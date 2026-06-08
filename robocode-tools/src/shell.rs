use std::{
    io::Write,
    process::{Command, Stdio},
};

use robocode_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput};

pub(crate) struct ShellTool;

const SHELL_STDIN_THRESHOLD: usize = 32 * 1024;

pub fn build_shell_invocation(command: &str, windows: bool) -> (String, Vec<String>) {
    if windows {
        (
            "powershell".to_string(),
            vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                command.to_string(),
            ],
        )
    } else {
        // Tool commands should inherit RoboCode's environment without sourcing
        // user startup files; stale profile hooks otherwise pollute tool output.
        (
            "sh".to_string(),
            vec!["-c".to_string(), command.to_string()],
        )
    }
}

pub fn shell_requires_stdin(command: &str) -> bool {
    command.len() > SHELL_STDIN_THRESHOLD
}

fn build_shell_stdin_invocation(windows: bool) -> (String, Vec<String>) {
    if windows {
        (
            "powershell".to_string(),
            vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "-".to_string(),
            ],
        )
    } else {
        ("sh".to_string(), vec!["-s".to_string()])
    }
}

fn run_shell_command(
    command: &str,
    cwd: &std::path::Path,
    windows: bool,
) -> Result<std::process::Output, String> {
    if !shell_requires_stdin(command) {
        let (program, args) = build_shell_invocation(command, windows);
        return Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|err| err.to_string());
    }

    let (program, args) = build_shell_stdin_invocation(windows);
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
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
        .write_all(command.as_bytes())
        .map_err(|err| format!("failed to write shell script to stdin: {err}"))?;
    stdin
        .write_all(b"\n")
        .map_err(|err| format!("failed to finish shell stdin script: {err}"))?;
    let _ = child.stdin.take();
    child.wait_with_output().map_err(|err| err.to_string())
}

impl BuiltinTool for ShellTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".to_string(),
            description: "Run a shell command".to_string(),
            is_mutating: true,
            input_schema_hint: "command='cargo test'".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let command = input
            .get("command")
            .ok_or_else(|| "shell requires `command`".to_string())?;
        let windows = cfg!(windows);
        let output = run_shell_command(command, &ctx.cwd, windows)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut rendered = String::new();
        if !stdout.trim().is_empty() {
            rendered.push_str(stdout.trim_end());
        }
        if !stderr.trim().is_empty() {
            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(stderr.trim_end());
        }
        if rendered.is_empty() {
            rendered = format!("Command exited with {}", output.status);
        }
        Ok(ToolExecutionOutput {
            output: rendered,
            diff: None,
            success: output.status.success(),
            exit_code: output.status.code(),
        })
    }
}
