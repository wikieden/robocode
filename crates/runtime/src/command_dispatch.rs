use crate::{EngineEvent, SessionEngine, presentation::render_diff_view};
use viden_types::{
    ApprovalResponse, CommandLogEntry, PermissionLevel, PermissionMode, TranscriptEntry, WorkMode,
    now_timestamp,
};

impl SessionEngine {
    pub(crate) fn handle_command<F>(
        &mut self,
        input: &str,
        approver: &mut F,
    ) -> Result<Vec<EngineEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or(input);
        let args: Vec<String> = parts.map(ToString::to_string).collect();
        let output = match command {
            "/help" => self.render_help(),
            "/model" => self.handle_model_command(&args)?,
            "/models" => self.handle_models_command(&args)?,
            "/connect" => self.handle_connect_command(&args)?,
            "/provider" => self.handle_provider_command(&args)?,
            "/settings" => self.handle_settings_command(&args)?,
            "/setup" => self.handle_setup_command(&args)?,
            "/brief" | "/spec" => self.handle_brief_command(&args, approver)?,
            "/agent" => self.handle_agent_command(&args, approver)?,
            "/extensions" => self.handle_extensions_command(&args)?,
            "/mcp" => self.handle_mcp_command(&args)?,
            "/skills" => self.handle_skills_command(&args)?,
            "/context" => self.render_context_command(),
            "/status" => self.render_status(),
            "/config" => self.render_config(),
            "/doctor" => self.render_doctor(),
            "/test" => self.handle_test_command(&args, approver)?,
            "/permissions" => {
                if let Some(mode) = args.first() {
                    let parsed = PermissionMode::parse_cli(mode)
                        .ok_or_else(|| format!("Unknown permission level `{mode}`"))?;
                    self.permissions.set_mode(parsed);
                    self.runtime_snapshot.permission_level =
                        PermissionLevel::from_legacy_mode(parsed);
                    if parsed == PermissionMode::Plan {
                        self.runtime_snapshot.work_mode = WorkMode::Plan;
                        self.persist_meta("work_mode", WorkMode::Plan.cli_name())?;
                    } else if self.runtime_snapshot.work_mode == WorkMode::Plan {
                        self.runtime_snapshot.work_mode = WorkMode::Build;
                        self.persist_meta("work_mode", WorkMode::Build.cli_name())?;
                    }
                    self.persist_meta("permission_mode", parsed.cli_name())?;
                    self.runtime_snapshot.permission_mode = parsed;
                    format!(
                        "Permission level set to {}",
                        self.permission_level().cli_name()
                    )
                } else {
                    format!(
                        "Current permission level: {}",
                        self.permission_level().cli_name()
                    )
                }
            }
            "/mode" => {
                if let Some(mode) = args.first() {
                    let parsed = WorkMode::parse_cli(mode)
                        .ok_or_else(|| format!("Unknown work mode `{mode}`"))?;
                    self.set_work_mode(parsed)?;
                    format!(
                        "Work mode set to {} / permission {}",
                        self.work_mode().cli_name(),
                        self.permission_level().cli_name()
                    )
                } else {
                    format!(
                        "Current work mode: {} / permission {}",
                        self.work_mode().cli_name(),
                        self.permission_level().cli_name()
                    )
                }
            }
            "/plan" => {
                let next_mode = match args.first().map(String::as_str) {
                    Some("on") => WorkMode::Plan,
                    Some("off") => WorkMode::Build,
                    _ if self.work_mode() == WorkMode::Plan => WorkMode::Build,
                    _ => WorkMode::Plan,
                };
                self.set_work_mode(next_mode)?;
                format!(
                    "Plan mode is now {}",
                    if next_mode == WorkMode::Plan {
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
                    render_diff_view("Latest diff", &diff)
                } else {
                    match self.run_named_tool("git_diff", Default::default(), approver) {
                        Ok(output) => render_diff_view("Latest diff", &output),
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
