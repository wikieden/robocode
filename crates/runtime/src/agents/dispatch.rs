use super::acp::*;
use super::codex::*;
use super::render::*;
use crate::{SessionEngine, presentation::render_permission_denial};
use viden_types::{
    ApprovalResponse, PermissionDecision, PermissionLogEntry, ToolInput, ToolSpec, TranscriptEntry,
    now_timestamp,
};

impl SessionEngine {
    pub(crate) fn handle_agent_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(render_agent_list()),
            "doctor" => Ok(render_agent_doctor(
                args.get(1).map(String::as_str),
                &self.cwd,
            )),
            "review" => handle_codex_review_command(&self.cwd, &args[1..]),
            "challenge" => handle_codex_challenge_command(&self.cwd, &args[1..]),
            "probe" => handle_agent_probe_command(&self.cwd, &args[1..]),
            "auth" => handle_agent_auth_command(&self.cwd, &args[1..]),
            "smoke" => self.handle_agent_smoke_command(&args[1..], approver),
            "run" => self.handle_agent_run_command(&args[1..], approver),
            "status" => render_codex_job_status(&self.cwd),
            "result" => render_codex_job_result(&self.cwd, args.get(1).map(String::as_str)),
            "cancel" => cancel_codex_job(&self.cwd, args.get(1).map(String::as_str)),
            "logs" => Ok(render_agent_logs_help()),
            subcommand => Ok(format!(
                "Unknown agent subcommand `{subcommand}`.\n\n{}",
                self.render_agent_help()
            )),
        }
    }

    fn render_agent_help(&self) -> String {
        [
            "Agent commands:",
            "  /agent list",
            "  /agent doctor [id]",
            "  /agent review codex [--base <ref>] [prompt]",
            "  /agent challenge codex [prompt]",
            "  /agent probe codex [--thread|--turn <task>|--turn-write <task>]",
            "  /agent probe acp <agent-id>",
            "  /agent auth acp <agent-id> [method-id]",
            "  /agent smoke acp [--live]",
            "  /agent run acp [--async] [--load-session <id>] [--mode <mode-id>] [--model <model-id>] <agent-id> <task>",
            "  /agent run codex [--write|--app-server] <task>",
            "  /agent status",
            "  /agent result <id>",
            "  /agent cancel <id>",
            "  /agent logs <id>",
            "",
            "Agent commands start and inspect tracked external agent jobs. Use `/lane ...` for terminal lane orchestration.",
        ]
        .join("\n")
    }

    fn handle_agent_run_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str) {
            Some("acp") => {
                let parsed = parse_acp_run_args(&args[1..])?;
                handle_acp_agent_run_command(
                    &self.cwd,
                    parsed,
                    approver,
                    self.permissions.context_snapshot(),
                    self.runtime_event_sink(),
                )
            }
            _ => self.handle_codex_run_command(args, approver),
        }
    }

    fn handle_agent_smoke_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str) {
            Some("acp") => {
                let live = args.iter().any(|arg| arg == "--live");
                run_acp_smoke_gate(&self.cwd, live, approver)
            }
            _ => Err("Usage: /agent smoke acp [--live]".to_string()),
        }
    }

    fn handle_codex_run_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        ensure_codex_target(args.first().map(String::as_str))?;
        let parsed = parse_codex_run_args(&args[1..])?;
        if parsed.task.trim().is_empty() {
            return Err("Usage: /agent run codex [--write|--app-server] <task>".to_string());
        }
        if parsed.app_server && parsed.write {
            return Err(
                "`--app-server` currently supports read-only delegated tasks only.".to_string(),
            );
        }
        if parsed.app_server {
            return start_codex_app_server_job(&self.cwd, &codex_command(), parsed.task.clone());
        }
        if parsed.write
            && let Some(denial) = self.ensure_codex_write_permission(&parsed.task, approver)?
        {
            return Ok(denial);
        }
        let sandbox = if parsed.write {
            "workspace-write"
        } else {
            "read-only"
        };
        start_codex_job(
            &self.cwd,
            &codex_command(),
            CodexJobKind::Run,
            parsed.task.clone(),
            codex_run_command_args(&self.cwd, sandbox, parsed.task),
        )
    }

    fn ensure_codex_write_permission<F>(
        &mut self,
        task: &str,
        approver: &mut F,
    ) -> Result<Option<String>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        let tool_name = "agent_codex_write".to_string();
        let tool = ToolSpec {
            name: tool_name.clone(),
            description: "Start a write-capable Codex delegated task".to_string(),
            is_mutating: true,
            input_schema_hint: "agent task".to_string(),
        };
        let mut input = ToolInput::new();
        input.insert("agent".to_string(), "codex".to_string());
        input.insert("mode".to_string(), "workspace-write".to_string());
        input.insert("cwd".to_string(), self.cwd.display().to_string());
        input.insert("task".to_string(), task.to_string());
        let mut gate_permissions = self.permissions.clone();
        let decision =
            crate::permission_gate::resolve(&mut gate_permissions, &tool, &tool_name, &input, {
                |_ask, prompt| approver(prompt)
            });
        match decision {
            PermissionDecision::Allow(allow) => {
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name,
                        decision: "allow".to_string(),
                        reason: format!("{:?}", allow.decision_reason),
                        message: allow.accept_feedback,
                    },
                })?;
                Ok(None)
            }
            PermissionDecision::Ask(_) => unreachable!("ask decisions should be resolved"),
            PermissionDecision::Deny(deny) => {
                let reason = format!("{:?}", deny.decision_reason);
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: tool_name.clone(),
                        decision: "deny".to_string(),
                        reason: reason.clone(),
                        message: Some(deny.message.clone()),
                    },
                })?;
                Ok(Some(render_permission_denial(
                    &tool_name,
                    &reason,
                    &deny.message,
                )))
            }
        }
    }
}
