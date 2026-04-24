use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use robocode_types::{LspDiagnostic, LspLocation, LspPosition, LspRange, LspSymbol};

use crate::config::{LspServerConfig, LspServerRegistry};
use crate::framing::encode_message;
use crate::protocol::{
    did_change_text_document, did_open_text_document, document_symbol_request,
    exit_notification, initialize_request, initialized_notification, references_request,
    shutdown_request,
};

const MESSAGE_TIMEOUT: Duration = Duration::from_secs(2);

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
        let running_servers = self
            .sessions
            .lock()
            .ok()
            .map(|sessions| {
                let mut names = sessions
                    .values()
                    .map(|session| session.server_id.clone())
                    .collect::<Vec<_>>();
                names.sort();
                names.dedup();
                names
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
                session.sync_document(
                    &file_uri,
                    language_id_for_path(&absolute_path),
                    &text,
                )?;
                action(session, &file_uri)
            };

            match result {
                Ok(value) => {
                    self.set_last_error(None);
                    return Ok(value);
                }
                Err(error)
                    if attempt == 0
                        && error == "Language server closed the message stream" =>
                {
                    if let Ok(mut sessions) = self.sessions.lock() {
                        if let Some(mut session) = sessions.remove(&session_key) {
                            let _ = session.shutdown();
                        }
                    }
                    continue;
                }
                Err(error) => {
                    if let Ok(mut sessions) = self.sessions.lock() {
                        if let Some(mut session) = sessions.remove(&session_key) {
                            let _ = session.shutdown();
                        }
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
                            return parse_diagnostics(params.get("diagnostics").unwrap_or(&Value::Null), file_uri);
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
        self.stdin.write_all(&bytes).map_err(|err| err.to_string())?;
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
    reader.read_exact(&mut body).map_err(|err| err.to_string())?;
    serde_json::from_slice(&body).map_err(|err| err.to_string()).map(Some)
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

fn language_id_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("py") => "python",
        _ => "plaintext",
    }
}

fn file_uri(path: &Path) -> Result<String, String> {
    let absolute = path.canonicalize().map_err(|err| err.to_string())?;
    let rendered = absolute.to_string_lossy().replace(' ', "%20");
    #[cfg(windows)]
    {
        Ok(format!("file:///{}", rendered.replace('\\', "/")))
    }
    #[cfg(not(windows))]
    {
        Ok(format!("file://{rendered}"))
    }
}

fn parse_diagnostics(value: &Value, file_uri: &str) -> Result<Vec<LspDiagnostic>, String> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let path = uri_to_path_string(file_uri);
    let mut diagnostics = Vec::new();
    for item in items {
        diagnostics.push(LspDiagnostic {
            path: path.clone(),
            range: parse_range(item.get("range").unwrap_or(&Value::Null))?,
            severity: item
                .get("severity")
                .and_then(Value::as_u64)
                .map(|severity| severity as u8),
            source: item
                .get("source")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            code: parse_optional_code(item.get("code")),
            message: item
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    Ok(diagnostics)
}

fn parse_symbol_response(response: &Value, file_uri: &str) -> Result<Vec<LspSymbol>, String> {
    parse_symbols(response.get("result").unwrap_or(&Value::Null), &uri_to_path_string(file_uri), None)
}

fn parse_symbols(
    value: &Value,
    path: &str,
    container_name: Option<String>,
) -> Result<Vec<LspSymbol>, String> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let mut symbols = Vec::new();
    for item in items {
        if let Some(location) = item.get("location") {
            symbols.push(LspSymbol {
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                kind: item.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32,
                path: uri_to_path_string(
                    location
                        .get("uri")
                        .and_then(Value::as_str)
                        .unwrap_or(path),
                ),
                range: parse_range(location.get("range").unwrap_or(&Value::Null))?,
                selection_range: None,
                container_name: item
                    .get("containerName")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            });
            continue;
        }

        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let selection_range = item
            .get("selectionRange")
            .map(parse_range)
            .transpose()?;
        symbols.push(LspSymbol {
            name: name.clone(),
            kind: item.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32,
            path: path.to_string(),
            range: parse_range(item.get("range").unwrap_or(&Value::Null))?,
            selection_range,
            container_name: container_name.clone(),
        });
        if let Some(children) = item.get("children") {
            symbols.extend(parse_symbols(children, path, Some(name))?);
        }
    }
    Ok(symbols)
}

fn parse_locations(value: &Value, fallback_uri: &str) -> Result<Vec<LspLocation>, String> {
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let mut locations = Vec::new();
    for item in items {
        let uri = item
            .get("uri")
            .and_then(Value::as_str)
            .or_else(|| item.get("targetUri").and_then(Value::as_str))
            .unwrap_or(fallback_uri);
        let range_value = item
            .get("range")
            .or_else(|| item.get("targetSelectionRange"))
            .or_else(|| item.get("targetRange"))
            .unwrap_or(&Value::Null);
        locations.push(LspLocation {
            path: uri_to_path_string(uri),
            range: parse_range(range_value)?,
        });
    }
    locations.sort_by(|left, right| {
        (
            left.path.as_str(),
            left.range.start.line,
            left.range.start.character,
        )
            .cmp(&(
                right.path.as_str(),
                right.range.start.line,
                right.range.start.character,
            ))
    });
    locations.dedup_by(|left, right| {
        left.path == right.path
            && left.range.start.line == right.range.start.line
            && left.range.start.character == right.range.start.character
            && left.range.end.line == right.range.end.line
            && left.range.end.character == right.range.end.character
    });
    Ok(locations)
}

fn parse_range(value: &Value) -> Result<LspRange, String> {
    Ok(LspRange {
        start: parse_position(value.get("start").unwrap_or(&Value::Null))?,
        end: parse_position(value.get("end").unwrap_or(&Value::Null))?,
    })
}

fn parse_position(value: &Value) -> Result<LspPosition, String> {
    Ok(LspPosition {
        line: value
            .get("line")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Missing LSP position line".to_string())? as u32,
        character: value
            .get("character")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Missing LSP position character".to_string())? as u32,
    })
}

fn parse_optional_code(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

fn uri_to_path_string(uri: &str) -> String {
    uri.strip_prefix("file://")
        .unwrap_or(uri)
        .replace("%20", " ")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    use super::*;
    use crate::LspServerRegistry;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "robocode_lsp_{name}_{}",
            robocode_types::fresh_id("tmp")
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_fake_server(
        workdir: &Path,
        stats_path: Option<&Path>,
        exit_after_symbol: bool,
    ) -> PathBuf {
        let script_path = workdir.join("fake_lsp_server.py");
        let counter_path = stats_path
            .map(|path| format!("STATS_PATH = {:?}\n", path.display().to_string()))
            .unwrap_or_else(|| "STATS_PATH = None\n".to_string());
        let exit_after_symbol_line = if exit_after_symbol {
            "EXIT_AFTER_SYMBOL = True\n"
        } else {
            "EXIT_AFTER_SYMBOL = False\n"
        };
        let script_template = r#"__COUNTER_PATH____EXIT_AFTER_SYMBOL__
import json
import sys
from pathlib import Path

def update_stats(key):
    if not STATS_PATH:
        return
    stats_file = Path(STATS_PATH)
    if stats_file.exists():
        stats = json.loads(stats_file.read_text())
    else:
        stats = {}
    stats[key] = stats.get(key, 0) + 1
    stats_file.write_text(json.dumps(stats))

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line == b"\r\n":
            break
        key, value = line.decode("utf-8").split(":", 1)
        headers[key.lower()] = value.strip()
    length = int(headers["content-length"])
    body = sys.stdin.buffer.read(length)
    return json.loads(body.decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        update_stats("initialize")
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}})
    elif method == "initialized":
        continue
    elif method == "textDocument/didOpen":
        update_stats("didOpen")
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 8}
                    },
                    "severity": 1,
                    "source": "fake-lsp",
                    "code": "E100",
                    "message": "fake diagnostic"
                }]
            }
        })
    elif method == "textDocument/didChange":
        update_stats("didChange")
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 8}
                    },
                    "severity": 2,
                    "source": "fake-lsp-change",
                    "code": "E200",
                    "message": "changed diagnostic"
                }]
            }
        })
    elif method == "textDocument/documentSymbol":
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": [{
                "name": "main",
                "kind": 12,
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 2, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 3},
                    "end": {"line": 0, "character": 7}
                },
                "children": [{
                    "name": "value",
                    "kind": 13,
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 9}
                    },
                    "selectionRange": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 9}
                    }
                }]
            }]
        })
        if EXIT_AFTER_SYMBOL:
            break
    elif method == "textDocument/references":
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": [{
                "uri": uri,
                "range": {
                    "start": {"line": 1, "character": 4},
                    "end": {"line": 1, "character": 9}
                }
            }]
        })
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": message["id"], "result": None})
    elif method == "exit":
        break
"#;
        let script = script_template
            .replace("__COUNTER_PATH__", &counter_path)
            .replace("__EXIT_AFTER_SYMBOL__", exit_after_symbol_line);
        fs::write(&script_path, script).unwrap();
        script_path
    }

    fn fake_registry(workdir: &Path) -> LspServerRegistry {
        let script_path = write_fake_server(workdir, None, false);
        LspServerRegistry::new(vec![LspServerConfig {
            id: "fake-rust".to_string(),
            command: env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
            args: vec![script_path.to_string_lossy().to_string()],
            file_extensions: vec!["rs".to_string()],
        }])
    }

    fn fake_registry_with_counter(workdir: &Path, counter: &Path) -> LspServerRegistry {
        let script_path = write_fake_server(workdir, Some(counter), false);
        LspServerRegistry::new(vec![LspServerConfig {
            id: "fake-rust".to_string(),
            command: env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
            args: vec![script_path.to_string_lossy().to_string()],
            file_extensions: vec!["rs".to_string()],
        }])
    }

    fn fake_registry_exits_after_symbol(workdir: &Path, counter: &Path) -> LspServerRegistry {
        let script_path = write_fake_server(workdir, Some(counter), true);
        LspServerRegistry::new(vec![LspServerConfig {
            id: "fake-rust".to_string(),
            command: env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
            args: vec![script_path.to_string_lossy().to_string()],
            file_extensions: vec!["rs".to_string()],
        }])
    }

    #[test]
    fn status_reports_configured_servers() {
        let runtime = LspRuntime::new(LspServerRegistry::default());
        let status = runtime.status();
        assert_eq!(status.configured_servers, vec!["rust-analyzer"]);
        assert!(status.running_servers.is_empty());
        assert!(status.last_error.is_none());
    }

    #[test]
    fn diagnostics_returns_clean_error_for_unconfigured_path() {
        let runtime = LspRuntime::new(LspServerRegistry::default());
        let error = runtime
            .diagnostics(Path::new("."), Path::new("README.md"))
            .unwrap_err();
        assert_eq!("No configured language server for README.md", error);
    }

    #[test]
    fn diagnostics_collect_publish_diagnostics_from_language_server() {
        let cwd = temp_dir("diagnostics");
        let source = cwd.join("sample.rs");
        fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
        let runtime = LspRuntime::new(fake_registry(&cwd));

        let diagnostics = runtime.diagnostics(&cwd, Path::new("sample.rs")).unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("fake-lsp"));
        assert_eq!(diagnostics[0].message, "fake diagnostic");
    }

    #[test]
    fn symbols_query_language_server_and_flatten_children() {
        let cwd = temp_dir("symbols");
        let source = cwd.join("sample.rs");
        fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
        let runtime = LspRuntime::new(fake_registry(&cwd));

        let symbols = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "main");
        assert_eq!(symbols[1].container_name.as_deref(), Some("main"));
        assert_eq!(symbols[1].name, "value");
    }

    #[test]
    fn references_query_language_server_and_parse_locations() {
        let cwd = temp_dir("references");
        let source = cwd.join("sample.rs");
        fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
        let runtime = LspRuntime::new(fake_registry(&cwd));

        let locations = runtime
            .references(
                &cwd,
                Path::new("sample.rs"),
                LspPosition {
                    line: 1,
                    character: 4,
                },
            )
            .unwrap();
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].range.start.line, 1);
        assert_eq!(locations[0].range.start.character, 4);
    }

    #[test]
    fn runtime_reuses_initialized_session_for_multiple_queries() {
        let cwd = temp_dir("reuse");
        let source = cwd.join("sample.rs");
        let counter = cwd.join("stats.json");
        fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
        let runtime = LspRuntime::new(fake_registry_with_counter(&cwd, &counter));

        let _ = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();
        let _ = runtime.references(
            &cwd,
            Path::new("sample.rs"),
            LspPosition {
                line: 1,
                character: 4,
            },
        ).unwrap();

        let status = runtime.status();
        assert_eq!(status.running_servers, vec!["fake-rust"]);
        let stats: Value = serde_json::from_str(&fs::read_to_string(counter).unwrap()).unwrap();
        assert_eq!(stats["initialize"], 1);
        assert_eq!(stats["didOpen"], 1);
    }

    #[test]
    fn runtime_uses_did_change_for_repeated_document_sync() {
        let cwd = temp_dir("did_change");
        let source = cwd.join("sample.rs");
        let counter = cwd.join("stats.json");
        fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
        let runtime = LspRuntime::new(fake_registry_with_counter(&cwd, &counter));

        let first = runtime.diagnostics(&cwd, Path::new("sample.rs")).unwrap();
        fs::write(&source, "fn main() {\n    let value = 2;\n}\n").unwrap();
        let second = runtime.diagnostics(&cwd, Path::new("sample.rs")).unwrap();

        assert_eq!(first[0].source.as_deref(), Some("fake-lsp"));
        assert_eq!(second[0].source.as_deref(), Some("fake-lsp-change"));
        let stats: Value = serde_json::from_str(&fs::read_to_string(counter).unwrap()).unwrap();
        assert_eq!(stats["initialize"], 1);
        assert_eq!(stats["didOpen"], 1);
        assert_eq!(stats["didChange"], 1);
    }

    #[test]
    fn runtime_restarts_dead_session_before_reuse() {
        let cwd = temp_dir("restart_dead");
        let source = cwd.join("sample.rs");
        let counter = cwd.join("stats.json");
        fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
        let runtime = LspRuntime::new(fake_registry_exits_after_symbol(&cwd, &counter));

        let first = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();
        let second = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();

        assert_eq!(first[0].name, "main");
        assert_eq!(second[0].name, "main");
        let stats: Value = serde_json::from_str(&fs::read_to_string(counter).unwrap()).unwrap();
        assert_eq!(stats["initialize"], 2);
        assert_eq!(stats["didOpen"], 2);
    }

    #[test]
    fn parse_locations_supports_location_links_and_dedups() {
        let payload = serde_json::json!([
            {
                "targetUri": "file:///tmp/project/src/lib.rs",
                "targetSelectionRange": {
                    "start": {"line": 3, "character": 2},
                    "end": {"line": 3, "character": 6}
                }
            },
            {
                "uri": "file:///tmp/project/src/lib.rs",
                "range": {
                    "start": {"line": 3, "character": 2},
                    "end": {"line": 3, "character": 6}
                }
            },
            {
                "uri": "file:///tmp/project/src/main.rs",
                "range": {
                    "start": {"line": 1, "character": 0},
                    "end": {"line": 1, "character": 4}
                }
            }
        ]);

        let locations = parse_locations(&payload, "file:///tmp/fallback.rs").unwrap();
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].path, "/tmp/project/src/lib.rs");
        assert_eq!(locations[0].range.start.line, 3);
        assert_eq!(locations[1].path, "/tmp/project/src/main.rs");
    }
}
