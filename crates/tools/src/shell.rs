use viden_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, ProcessInvocation, ToolExecutionContext, ToolExecutionOutput};

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
        // Tool commands should inherit Viden's environment without sourcing
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

/// Builds the invocation for the process capability seam. Oversized commands
/// travel as a stdin script; the capability backend owns the actual spawn.
fn build_process_invocation(
    command: &str,
    cwd: &std::path::Path,
    windows: bool,
) -> ProcessInvocation {
    if !shell_requires_stdin(command) {
        let (program, args) = build_shell_invocation(command, windows);
        return ProcessInvocation {
            program,
            args,
            cwd: cwd.to_path_buf(),
            stdin_script: None,
        };
    }
    let (program, args) = build_shell_stdin_invocation(windows);
    ProcessInvocation {
        program,
        args,
        cwd: cwd.to_path_buf(),
        stdin_script: Some(command.to_string()),
    }
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
        let invocation = build_process_invocation(command, &ctx.cwd, windows);
        let output = ctx.process.run(&invocation)?;
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
            rendered = format!("Command exited with {}", output.status_display);
        }
        Ok(ToolExecutionOutput {
            output: rendered,
            diff: None,
            success: output.success,
            exit_code: output.exit_code,
        })
    }
}
