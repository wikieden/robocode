use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use viden_types::{LspDiagnostic, LspLocation, LspPosition, LspSymbol};

use crate::config::{LspServerConfig, LspServerRegistry};
use crate::framing::encode_message;
mod parsing;

use crate::protocol::{
    did_change_text_document, did_open_text_document, document_symbol_request, exit_notification,
    initialize_request, initialized_notification, references_request, shutdown_request,
};
use parsing::{
    file_uri, language_id_for_path, parse_diagnostics, parse_locations, parse_symbol_response,
};

const MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);

pub trait SemanticProvider: Send + Sync {
    fn diagnostics(&self, cwd: &Path, path: &Path) -> Result<Vec<LspDiagnostic>, String>;

    fn symbols(&self, cwd: &Path, path: &Path) -> Result<Vec<LspSymbol>, String>;

    fn references(
        &self,
        cwd: &Path,
        path: &Path,
        position: LspPosition,
    ) -> Result<Vec<LspLocation>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LspRuntimeStatus {
    pub configured_servers: Vec<String>,
    pub running_servers: Vec<String>,
    pub cached_sessions: usize,
    pub open_documents: usize,
    pub last_error: Option<String>,
}

pub struct LspRuntime {
    registry: LspServerRegistry,
    last_error: Arc<Mutex<Option<String>>>,
    sessions: Mutex<HashMap<String, LspSession>>,
}

impl LspRuntime {
    pub fn new(registry: LspServerRegistry) -> Self {
        Self {
            registry,
            last_error: Arc::new(Mutex::new(None)),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn status(&self) -> LspRuntimeStatus {
        let (running_servers, cached_sessions, open_documents) = self
            .sessions
            .lock()
            .ok()
            .map(|mut sessions| {
                let mut names = sessions
                    .values_mut()
                    .filter_map(|session| {
                        session
                            .is_dead()
                            .ok()
                            .is_some_and(|dead| !dead)
                            .then(|| session.server_id.clone())
                    })
                    .collect::<Vec<_>>();
                names.sort();
                names.dedup();
                let open_documents = sessions
                    .values()
                    .map(|session| session.open_documents.len())
                    .sum();
                (names, sessions.len(), open_documents)
            })
            .unwrap_or_default();
        LspRuntimeStatus {
            configured_servers: self
                .registry
                .all()
                .iter()
                .map(|server| server.id.clone())
                .collect(),
            running_servers,
            cached_sessions,
            open_documents,
            last_error: self.last_error.lock().ok().and_then(|guard| guard.clone()),
        }
    }

    fn set_last_error(&self, error: Option<String>) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = error;
        }
    }

    fn server_for_path<'a>(&'a self, path: &Path) -> Result<&'a LspServerConfig, String> {
        self.registry
            .for_path(path)
            .ok_or_else(|| format!("No configured language server for {}", path.display()))
    }

    fn with_open_document<T, F>(&self, cwd: &Path, path: &Path, action: F) -> Result<T, String>
    where
        F: Fn(&mut LspSession, &str) -> Result<T, String>,
    {
        let server = self.server_for_path(path)?;
        let absolute_path = resolve_query_path(cwd, path)?;
        let session_key = session_cache_key(cwd, server)?;
        let file_uri = file_uri(&absolute_path)?;
        let text = fs::read_to_string(&absolute_path).map_err(|err| err.to_string())?;
        for attempt in 0..2 {
            let result = {
                let mut sessions = self
                    .sessions
                    .lock()
                    .map_err(|_| "Failed to lock LSP session cache".to_string())?;
                let session = match sessions.entry(session_key.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        if entry.get_mut().is_dead()? {
                            let _ = entry.get_mut().shutdown();
                            let mut session = LspSession::start(server, cwd)?;
                            session.initialize(cwd)?;
                            session.notify(&initialized_notification())?;
                            let _ = entry.insert(session);
                        }
                        entry.into_mut()
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let mut session = LspSession::start(server, cwd)?;
                        session.initialize(cwd)?;
                        session.notify(&initialized_notification())?;
                        entry.insert(session)
                    }
                };
                session.sync_document(&file_uri, language_id_for_path(&absolute_path), &text)?;
                action(session, &file_uri)
            };

            match result {
                Ok(value) => {
                    self.set_last_error(None);
                    return Ok(value);
                }
                Err(error)
                    if attempt == 0 && error == "Language server closed the message stream" =>
                {
                    if let Ok(mut sessions) = self.sessions.lock()
                        && let Some(mut session) = sessions.remove(&session_key)
                    {
                        let _ = session.shutdown();
                    }
                    continue;
                }
                Err(error) => {
                    if let Ok(mut sessions) = self.sessions.lock()
                        && let Some(mut session) = sessions.remove(&session_key)
                    {
                        let _ = session.shutdown();
                    }
                    self.set_last_error(Some(error.clone()));
                    return Err(error);
                }
            }
        }
        Err("Language server retry loop exhausted unexpectedly".to_string())
    }
}

impl Drop for LspRuntime {
    fn drop(&mut self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            for (_, mut session) in sessions.drain() {
                let _ = session.shutdown();
            }
        }
    }
}

impl SemanticProvider for LspRuntime {
    fn diagnostics(&self, cwd: &Path, path: &Path) -> Result<Vec<LspDiagnostic>, String> {
        self.with_open_document(cwd, path, |session, file_uri| {
            session.wait_for_diagnostics(file_uri)
        })
    }

    fn symbols(&self, cwd: &Path, path: &Path) -> Result<Vec<LspSymbol>, String> {
        self.with_open_document(cwd, path, |session, file_uri| {
            let request_id = session.next_request_id();
            let response = session.request(document_symbol_request(request_id, file_uri))?;
            parse_symbol_response(&response, file_uri)
        })
    }

    fn references(
        &self,
        cwd: &Path,
        path: &Path,
        position: LspPosition,
    ) -> Result<Vec<LspLocation>, String> {
        self.with_open_document(cwd, path, |session, file_uri| {
            let request_id = session.next_request_id();
            let response = session.request(references_request(
                request_id,
                file_uri,
                position.line,
                position.character,
            ))?;
            parse_locations(response.get("result").unwrap_or(&Value::Null), file_uri)
        })
    }
}

struct LspSession {
    server_id: String,
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Result<Value, String>>,
    next_request_id: u64,
    open_documents: HashMap<String, i32>,
}

impl LspSession {
    fn start(server: &LspServerConfig, cwd: &Path) -> Result<Self, String> {
        let mut command = Command::new(&server.command);
        command
            .args(&server.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|err| {
            format!(
                "Failed to start language server `{}`: {err}",
                server.command
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture language server stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture language server stdout".to_string())?;
        Ok(Self {
            server_id: server.id.clone(),
            child,
            stdin,
            messages: spawn_reader(stdout),
            next_request_id: 1,
            open_documents: HashMap::new(),
        })
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    fn initialize(&mut self, cwd: &Path) -> Result<(), String> {
        let root_uri = file_uri(cwd)?;
        let request_id = self.next_request_id();
        let response = self.request(initialize_request(request_id, &root_uri))?;
        if response.get("result").is_none() {
            return Err("Language server initialize response missing result".to_string());
        }
        Ok(())
    }

    fn notify(&mut self, payload: &Value) -> Result<(), String> {
        self.send(payload)
    }

    fn sync_document(
        &mut self,
        file_uri: &str,
        language_id: &str,
        text: &str,
    ) -> Result<(), String> {
        if let Some(version) = self.open_documents.get_mut(file_uri) {
            *version += 1;
            let next_version = *version;
            return self.notify(&did_change_text_document(file_uri, next_version, text));
        }
        self.notify(&did_open_text_document(file_uri, language_id, text))?;
        self.open_documents.insert(file_uri.to_string(), 1);
        Ok(())
    }

    fn request(&mut self, payload: Value) -> Result<Value, String> {
        let id = payload
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "LSP request payload missing numeric id".to_string())?;
        self.send(&payload)?;
        loop {
            let message = self.recv_message(MESSAGE_TIMEOUT)?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(format!("Language server returned error: {error}"));
                }
                return Ok(message);
            }
        }
    }

    fn wait_for_diagnostics(&mut self, file_uri: &str) -> Result<Vec<LspDiagnostic>, String> {
        loop {
            match self.recv_message(MESSAGE_TIMEOUT) {
                Ok(message) => {
                    if message.get("method").and_then(Value::as_str)
                        == Some("textDocument/publishDiagnostics")
                    {
                        let params = message.get("params").unwrap_or(&Value::Null);
                        if params.get("uri").and_then(Value::as_str) == Some(file_uri) {
                            return parse_diagnostics(
                                params.get("diagnostics").unwrap_or(&Value::Null),
                                file_uri,
                            );
                        }
                    }
                }
                Err(error) if error == "Timed out waiting for language server message" => {
                    return Ok(Vec::new());
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), String> {
        let request_id = self.next_request_id();
        let _ = self.request(shutdown_request(request_id));
        let _ = self.notify(&exit_notification());
        match self.child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    fn is_dead(&mut self) -> Result<bool, String> {
        match self.child.try_wait() {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(err) => Err(err.to_string()),
        }
    }

    fn send(&mut self, payload: &Value) -> Result<(), String> {
        let bytes = encode_message(payload)?;
        self.stdin
            .write_all(&bytes)
            .map_err(|err| err.to_string())?;
        self.stdin.flush().map_err(|err| err.to_string())
    }

    fn recv_message(&self, timeout: Duration) -> Result<Value, String> {
        match self.messages.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err("Timed out waiting for language server message".to_string())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("Language server closed the message stream".to_string())
            }
        }
    }
}

fn spawn_reader(stdout: ChildStdout) -> Receiver<Result<Value, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_lsp_message(&mut reader) {
                Ok(Some(value)) => {
                    if sender.send(Ok(value)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

fn read_lsp_message(reader: &mut BufReader<ChildStdout>) -> Result<Option<Value>, String> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).map_err(|err| err.to_string())?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == "\r\n" {
            break;
        }
        if let Some(raw_length) = line.strip_prefix("Content-Length: ") {
            let length = raw_length
                .trim()
                .parse::<usize>()
                .map_err(|_| "Invalid Content-Length header".to_string())?;
            content_length = Some(length);
        }
    }
    let length = content_length.ok_or_else(|| "Missing Content-Length header".to_string())?;
    let mut body = vec![0_u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|err| err.to_string())?;
    serde_json::from_slice(&body)
        .map_err(|err| err.to_string())
        .map(Some)
}

fn resolve_query_path(cwd: &Path, path: &Path) -> Result<PathBuf, String> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    candidate.canonicalize().map_err(|err| err.to_string())
}

fn session_cache_key(cwd: &Path, server: &LspServerConfig) -> Result<String, String> {
    let absolute_cwd = cwd.canonicalize().map_err(|err| err.to_string())?;
    Ok(format!("{}::{}", absolute_cwd.display(), server.id))
}

#[cfg(test)]
mod runtime_tests;
