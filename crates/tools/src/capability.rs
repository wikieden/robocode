//! OS capability seam for tool execution.
//!
//! Tools reach the operating system only through these traits so that one
//! provider swap (for example a sandboxed or remote backend) relocates every
//! consuming tool together, without touching tool code. `LocalFilesystem` and
//! `LocalProcess` preserve the direct `std::fs`/`std::process` behavior the
//! tools shipped with. File and shell tools consume the seam today; remaining
//! tools (search, git, patch, web, process lanes) migrate incrementally per
//! `docs/harness-direction.md`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

/// Filesystem operations a tool may perform. Mirrors the exact `std::fs`
/// surface tools used before the seam existed; keep additions minimal and
/// implementable by non-local backends.
pub trait FilesystemCapability: Send + Sync {
    fn is_dir(&self, path: &Path) -> bool;
    fn read(&self, path: &Path) -> Result<Vec<u8>, String>;
    fn read_to_string(&self, path: &Path) -> Result<String, String>;
    fn create_dir_all(&self, path: &Path) -> Result<(), String>;
    fn write(&self, path: &Path, contents: &str) -> Result<(), String>;
}

/// One process run: a program with arguments in a working directory, with an
/// optional script fed through stdin (used for oversized shell commands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub stdin_script: Option<String>,
}

/// The completed process result. `status_display` carries the platform status
/// rendering (for example `exit status: 0`) so output formatting stays
/// byte-identical to the pre-seam behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub status_display: String,
}

/// A long-lived interactive process launch: piped stdin/stdout/stderr, with
/// the caller writing input and polling output until it releases or kills the
/// process. Used for client-driven terminals (for example ACP
/// `terminal/create`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub envs: Vec<(String, String)>,
}

/// Control handle for a spawned interactive process.
pub trait InteractiveProcessControl: Send {
    /// Non-blocking exit probe: `Ok(Some(exit_code))` once the process has
    /// exited (the inner `Option` is `None` when the platform reports no
    /// code, for example signal termination), `Ok(None)` while running.
    fn try_wait(&mut self) -> Result<Option<Option<i32>>, String>;

    /// Forcefully terminate the process.
    fn kill(&mut self) -> Result<(), String>;
}

/// A spawned interactive process: stdin writer, asynchronously pumped
/// stdout/stderr byte chunks, and the control handle. The channels close when
/// the corresponding stream reaches end of file.
pub struct InteractiveProcess {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    pub stderr: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    pub control: Box<dyn InteractiveProcessControl>,
}

/// Process execution a tool may perform.
pub trait ProcessCapability: Send + Sync {
    fn run(&self, invocation: &ProcessInvocation) -> Result<ProcessOutput, String>;

    /// Spawn a long-lived interactive process. Defaults to unsupported so
    /// scripted or sandbox capabilities stay fail-closed unless they opt in.
    fn spawn_interactive(
        &self,
        invocation: &InteractiveInvocation,
    ) -> Result<InteractiveProcess, String> {
        let _ = invocation;
        Err("interactive process spawn is not supported by this process capability".to_string())
    }
}

/// Direct `std::fs` passthrough: the default local backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalFilesystem;

impl FilesystemCapability for LocalFilesystem {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        fs::read(path).map_err(|err| err.to_string())
    }

    fn read_to_string(&self, path: &Path) -> Result<String, String> {
        fs::read_to_string(path).map_err(|err| err.to_string())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|err| err.to_string())
    }

    fn write(&self, path: &Path, contents: &str) -> Result<(), String> {
        fs::write(path, contents).map_err(|err| err.to_string())
    }
}

/// Direct `std::process` passthrough: the default local backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalProcess;

/// Reads a stream on a background thread and forwards byte chunks until EOF.
fn pump_bytes_async(
    mut reader: impl std::io::Read + Send + 'static,
) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if sender.send(Ok(buffer[..size].to_vec())).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

/// Control handle over a real local child process.
struct LocalInteractiveControl {
    child: std::process::Child,
}

impl InteractiveProcessControl for LocalInteractiveControl {
    fn try_wait(&mut self) -> Result<Option<Option<i32>>, String> {
        match self.child.try_wait() {
            Ok(Some(status)) => Ok(Some(status.code())),
            Ok(None) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn kill(&mut self) -> Result<(), String> {
        self.child.kill().map_err(|error| error.to_string())
    }
}

impl ProcessCapability for LocalProcess {
    fn run(&self, invocation: &ProcessInvocation) -> Result<ProcessOutput, String> {
        let output = match &invocation.stdin_script {
            None => Command::new(&invocation.program)
                .args(&invocation.args)
                .current_dir(&invocation.cwd)
                .output()
                .map_err(|err| err.to_string())?,
            Some(script) => {
                let mut child = Command::new(&invocation.program)
                    .args(&invocation.args)
                    .current_dir(&invocation.cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|err| format!("failed to launch shell with stdin script: {err}"))?;
                let stdin = child
                    .stdin
                    .as_mut()
                    .ok_or_else(|| "failed to open shell stdin".to_string())?;
                stdin
                    .write_all(script.as_bytes())
                    .map_err(|err| format!("failed to write shell script to stdin: {err}"))?;
                stdin
                    .write_all(b"\n")
                    .map_err(|err| format!("failed to finish shell stdin script: {err}"))?;
                let _ = child.stdin.take();
                child.wait_with_output().map_err(|err| err.to_string())?
            }
        };
        Ok(ProcessOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            status_display: output.status.to_string(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn spawn_interactive(
        &self,
        invocation: &InteractiveInvocation,
    ) -> Result<InteractiveProcess, String> {
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .current_dir(&invocation.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in &invocation.envs {
            command.env(name, value);
        }
        let mut child = command.spawn().map_err(|err| err.to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "failed to capture stderr".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to capture stdin".to_string())?;
        Ok(InteractiveProcess {
            stdin: Box::new(stdin),
            stdout: pump_bytes_async(stdout),
            stderr: pump_bytes_async(stderr),
            control: Box::new(LocalInteractiveControl { child }),
        })
    }
}
