use crate::{EngineEvent, SessionEngine};
use robocode_types::{
    ApprovalResponse, CommandLogEntry, PermissionMode, TranscriptEntry, now_timestamp,
};

impl SessionEngine {
    pub(crate) fn handle_command<F>(
        &mut self,
        input: &str,
        approver: &mut F,
    ) -> Result<Vec<EngineEvent>, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or(input);
        let args: Vec<String> = parts.map(ToString::to_string).collect();
        let output = match command {
            "/help" => self.render_help(),
            "/model" => {
                if let Some(model) = args.first() {
                    self.provider.set_model(model.clone());
                    self.persist_meta("model", model)?;
                    format!("Model set to {}", self.provider.model())
                } else {
                    format!("Current model: {}", self.provider.model())
                }
            }
            "/provider" => self.handle_provider_command(&args)?,
            "/status" => self.render_status(),
            "/config" => self.render_config(),
            "/doctor" => self.render_doctor(),
            "/permissions" => {
                if let Some(mode) = args.first() {
                    let parsed = PermissionMode::parse_cli(mode)
                        .ok_or_else(|| format!("Unknown permission mode `{mode}`"))?;
                    self.permissions.set_mode(parsed);
                    self.persist_meta("permission_mode", parsed.cli_name())?;
                    self.runtime_snapshot.permission_mode = parsed;
                    format!("Permission mode set to {}", parsed.cli_name())
                } else {
                    format!(
                        "Current permission mode: {}",
                        self.permissions.mode().cli_name()
                    )
                }
            }
            "/plan" => {
                let next_mode = match args.first().map(String::as_str) {
                    Some("on") => PermissionMode::Plan,
                    Some("off") => PermissionMode::Default,
                    _ if self.permissions.mode() == PermissionMode::Plan => PermissionMode::Default,
                    _ => PermissionMode::Plan,
                };
                self.permissions.set_mode(next_mode);
                self.persist_meta("permission_mode", next_mode.cli_name())?;
                self.runtime_snapshot.permission_mode = next_mode;
                format!(
                    "Plan mode is now {}",
                    if next_mode == PermissionMode::Plan {
                        "on"
                    } else {
                        "off"
                    }
                )
            }
            "/sessions" => self.handle_sessions()?,
            "/resume" => self.handle_resume(args.first().map(String::as_str))?,
            "/tasks" => self.handle_tasks()?,
            "/task" => self.handle_task_command(&args, approver)?,
            "/memory" => self.handle_memory_command(&args, approver)?,
            "/diff" => {
                if let Some(diff) = self.last_diff.clone() {
                    diff
                } else {
                    match self.run_named_tool("git_diff", Default::default(), approver) {
                        Ok(output) => output,
                        Err(_) => "No diffs recorded in this session yet.".to_string(),
                    }
                }
            }
            "/web" => self.handle_web_command(&args, approver)?,
            "/git" => self.handle_git_command(&args, approver)?,
            "/lsp" => self.handle_lsp_command(&args)?,
            _ => format!("Unknown command `{command}`. Use /help."),
        };
        self.store_entry(TranscriptEntry::Command {
            entry: CommandLogEntry {
                timestamp: now_timestamp(),
                name: command.trim_start_matches('/').to_string(),
                args,
                output: output.clone(),
            },
        })?;
        Ok(vec![EngineEvent::Command(output)])
    }
}
