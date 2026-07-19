use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::lane::LaneEffectError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnProcess {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneProcessHandle {
    pub id: String,
}

pub trait ProcessBackend: Send + Sync {
    fn spawn(&self, request: &SpawnProcess) -> Result<LaneProcessHandle, LaneEffectError>;
    fn send(&self, handle: &LaneProcessHandle, input: &[u8]) -> Result<(), LaneEffectError>;
    fn stop(&self, handle: &LaneProcessHandle) -> Result<(), LaneEffectError>;
}

#[derive(Debug, Default)]
pub struct LocalProcessBackend {
    children: Mutex<BTreeMap<String, Child>>,
}

impl ProcessBackend for LocalProcessBackend {
    fn spawn(&self, request: &SpawnProcess) -> Result<LaneProcessHandle, LaneEffectError> {
        let mut command = Command::new(&request.command);
        command
            .args(&request.args)
            .current_dir(&request.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &request.env {
            command.env(key, value);
        }
        let child = command
            .spawn()
            .map_err(|err| LaneEffectError::Io(err.to_string()))?;
        let id = format!("process-{}", child.id());
        self.children
            .lock()
            .map_err(|_| LaneEffectError::Io("process registry poisoned".to_string()))?
            .insert(id.clone(), child);
        Ok(LaneProcessHandle { id })
    }

    fn send(&self, handle: &LaneProcessHandle, input: &[u8]) -> Result<(), LaneEffectError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| LaneEffectError::Io("process registry poisoned".to_string()))?;
        let child = children.get_mut(&handle.id).ok_or_else(|| {
            LaneEffectError::Io(format!("unknown process handle `{}`", handle.id))
        })?;
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            LaneEffectError::Io(format!("process handle `{}` has no stdin", handle.id))
        })?;
        stdin
            .write_all(input)
            .and_then(|_| stdin.flush())
            .map_err(|err| LaneEffectError::Io(err.to_string()))
    }

    fn stop(&self, handle: &LaneProcessHandle) -> Result<(), LaneEffectError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| LaneEffectError::Io("process registry poisoned".to_string()))?;
        let Some(mut child) = children.remove(&handle.id) else {
            return Err(LaneEffectError::Io(format!(
                "unknown process handle `{}`",
                handle.id
            )));
        };
        child
            .kill()
            .map_err(|err| LaneEffectError::Io(err.to_string()))?;
        let _ = child.wait();
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeTerminalSession {
    pub handle: LaneProcessHandle,
    pub request: SpawnProcess,
    pub inputs: Vec<Vec<u8>>,
    pub stopped: bool,
}

#[derive(Debug, Default, Clone)]
pub struct FakeTerminalBackend {
    sessions: Arc<Mutex<Vec<FakeTerminalSession>>>,
}

impl FakeTerminalBackend {
    pub fn sessions(&self) -> Vec<FakeTerminalSession> {
        self.sessions
            .lock()
            .expect("fake terminal poisoned")
            .clone()
    }
}

impl ProcessBackend for FakeTerminalBackend {
    fn spawn(&self, request: &SpawnProcess) -> Result<LaneProcessHandle, LaneEffectError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("fake terminal poisoned".to_string()))?;
        let handle = LaneProcessHandle {
            id: format!("fake-terminal-{}", sessions.len() + 1),
        };
        sessions.push(FakeTerminalSession {
            handle: handle.clone(),
            request: request.clone(),
            inputs: Vec::new(),
            stopped: false,
        });
        Ok(handle)
    }

    fn send(&self, handle: &LaneProcessHandle, input: &[u8]) -> Result<(), LaneEffectError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("fake terminal poisoned".to_string()))?;
        let Some(session) = sessions
            .iter_mut()
            .find(|session| session.handle == *handle)
        else {
            return Err(LaneEffectError::Io(format!(
                "unknown process handle `{}`",
                handle.id
            )));
        };
        session.inputs.push(input.to_vec());
        Ok(())
    }

    fn stop(&self, handle: &LaneProcessHandle) -> Result<(), LaneEffectError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("fake terminal poisoned".to_string()))?;
        let Some(session) = sessions
            .iter_mut()
            .find(|session| session.handle == *handle)
        else {
            return Err(LaneEffectError::Io(format!(
                "unknown process handle `{}`",
                handle.id
            )));
        };
        session.stopped = true;
        Ok(())
    }
}
