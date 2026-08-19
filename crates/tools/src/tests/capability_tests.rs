use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use viden_types::{ToolCall, ToolInput};

use super::*;
use crate::{FilesystemCapability, ProcessCapability, ProcessInvocation, ProcessOutput};

/// In-memory filesystem: file and shell tools must reach the OS only through
/// the capability seam, so swapping this in must fully detach them from disk.
#[derive(Default)]
struct MemoryFilesystem {
    files: Mutex<BTreeMap<PathBuf, String>>,
}

impl MemoryFilesystem {
    fn with_file(path: impl Into<PathBuf>, contents: &str) -> Self {
        let fs = Self::default();
        fs.files
            .lock()
            .unwrap()
            .insert(path.into(), contents.to_string());
        fs
    }
}

impl FilesystemCapability for MemoryFilesystem {
    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        self.read_to_string(path).map(String::into_bytes)
    }

    fn read_to_string(&self, path: &Path) -> Result<String, String> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| format!("memory fs has no file at {}", path.display()))
    }

    fn create_dir_all(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }

    fn write(&self, path: &Path, contents: &str) -> Result<(), String> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), contents.to_string());
        Ok(())
    }
}

/// Scripted process runner recording every invocation instead of spawning.
struct ScriptedProcess {
    invocations: Mutex<Vec<ProcessInvocation>>,
    stdout: String,
}

impl ScriptedProcess {
    fn new(stdout: &str) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            stdout: stdout.to_string(),
        }
    }
}

impl ProcessCapability for ScriptedProcess {
    fn run(&self, invocation: &ProcessInvocation) -> Result<ProcessOutput, String> {
        self.invocations.lock().unwrap().push(invocation.clone());
        Ok(ProcessOutput {
            stdout: self.stdout.clone().into_bytes(),
            stderr: Vec::new(),
            success: true,
            exit_code: Some(0),
            status_display: "exit status: 0".to_string(),
        })
    }
}

#[test]
fn file_tools_reach_the_filesystem_only_through_the_capability_seam() {
    let cwd = PathBuf::from("/virtual/workspace");
    let memory_fs = Arc::new(MemoryFilesystem::with_file(
        "/virtual/workspace/notes.txt",
        "seam contents",
    ));
    let mut ctx = ToolExecutionContext::local(cwd);
    ctx.fs = memory_fs.clone();
    let registry = ToolRegistry::builtin();

    let mut read_input = ToolInput::new();
    read_input.insert("path".into(), "notes.txt".into());
    let read_result = registry
        .execute(
            &ToolCall {
                id: "tool_read".into(),
                name: "read_file".into(),
                input: read_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(read_result.success);
    assert_eq!(read_result.output, "seam contents");

    let mut edit_input = ToolInput::new();
    edit_input.insert("path".into(), "notes.txt".into());
    edit_input.insert("old".into(), "seam".into());
    edit_input.insert("new".into(), "capability".into());
    let edit_result = registry
        .execute(
            &ToolCall {
                id: "tool_edit".into(),
                name: "edit_file".into(),
                input: edit_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(edit_result.success);
    assert_eq!(
        memory_fs
            .read_to_string(Path::new("/virtual/workspace/notes.txt"))
            .unwrap(),
        "capability contents"
    );

    let mut write_input = ToolInput::new();
    write_input.insert("path".into(), "fresh/created.txt".into());
    write_input.insert("content".into(), "born virtual".into());
    let write_result = registry
        .execute(
            &ToolCall {
                id: "tool_write".into(),
                name: "write_file".into(),
                input: write_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(write_result.success);
    assert_eq!(
        memory_fs
            .read_to_string(Path::new("/virtual/workspace/fresh/created.txt"))
            .unwrap(),
        "born virtual"
    );
    // Nothing may have leaked onto the real disk.
    assert!(!Path::new("/virtual/workspace").exists());
}

#[test]
fn shell_tool_runs_commands_only_through_the_process_capability() {
    let cwd = PathBuf::from("/virtual/workspace");
    let scripted = Arc::new(ScriptedProcess::new("scripted output\n"));
    let mut ctx = ToolExecutionContext::local(cwd.clone());
    ctx.process = scripted.clone();
    let registry = ToolRegistry::builtin();

    let mut input = ToolInput::new();
    input.insert("command".into(), "echo hi".into());
    let result = registry
        .execute(
            &ToolCall {
                id: "tool_shell".into(),
                name: "shell".into(),
                input,
            },
            &ctx,
        )
        .unwrap();

    assert!(result.success);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.output, "scripted output");
    let invocations = scripted.invocations.lock().unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].cwd, cwd);
    assert!(
        invocations[0]
            .args
            .iter()
            .any(|arg| arg.contains("echo hi"))
            || invocations[0].stdin_script.as_deref() == Some("echo hi"),
        "the user command must reach the process capability verbatim: {:?}",
        invocations[0]
    );
}

#[test]
fn local_context_preserves_on_disk_behavior() {
    let cwd = temp_dir("capability_local");
    let ctx = ToolExecutionContext::local(cwd.clone());
    let registry = ToolRegistry::builtin();

    let mut write_input = ToolInput::new();
    write_input.insert("path".into(), "real.txt".into());
    write_input.insert("content".into(), "on disk".into());
    registry
        .execute(
            &ToolCall {
                id: "tool_write_local".into(),
                name: "write_file".into(),
                input: write_input,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(cwd.join("real.txt")).unwrap(),
        "on disk"
    );
}
