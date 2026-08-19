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

/// Process execution a tool may perform.
pub trait ProcessCapability: Send + Sync {
    fn run(&self, invocation: &ProcessInvocation) -> Result<ProcessOutput, String>;
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
}
