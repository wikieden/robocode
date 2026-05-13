use std::process::Command;

use robocode_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput};

pub(crate) struct ShellTool;

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
        (
            "sh".to_string(),
            vec!["-lc".to_string(), command.to_string()],
        )
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
        let (program, args) = build_shell_invocation(command, windows);
        let output = Command::new(program)
            .args(args)
            .current_dir(&ctx.cwd)
            .output()
            .map_err(|err| err.to_string())?;
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
        })
    }
}
