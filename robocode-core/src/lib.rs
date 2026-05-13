use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

mod presentation;

use presentation::{join_lines, render_section_title, render_subsection_title};
use robocode_lsp::{LspRuntime, LspServerRegistry, SemanticProvider};
use robocode_model::ModelProvider;
use robocode_permissions::PermissionEngine;
use robocode_session::SessionStore;
use robocode_tools::{SemanticToolProvider, ToolExecutionContext, ToolRegistry};
use robocode_types::{
    ApprovalResponse, CommandLogEntry, LspDiagnostic, LspLocation, LspPosition, LspSymbol,
    MemoryKind, MemoryScope, MemorySource, Message, ModelEvent, ModelRequest, PermissionDecision,
    PermissionLogEntry, PermissionMode, ResumeContextSnapshot, Role, RuntimeSnapshot,
    SessionMetaEntry, SessionSummary, TaskPriority, TaskStatus, ToolCall, ToolInput, ToolResult,
    ToolSpec, TranscriptEntry, fresh_id, now_timestamp,
};
use robocode_workflows::memory::MemoryEvent;
use robocode_workflows::resume_context::{ResumeContextInput, build_resume_context};
use robocode_workflows::stores::WorkflowStore;
use robocode_workflows::tasks::{TaskBlocker, TaskEvent, TaskUpdate};

const PROVIDER_REASONING_CONTENT_KEY: &str = "__provider_reasoning_content";

#[derive(Debug, Clone)]
pub enum EngineEvent {
    System(String),
    Assistant(String),
    ToolCall(String),
    ToolResult(String),
    Command(String),
}

struct LspToolAdapter {
    runtime: Arc<LspRuntime>,
}

impl SemanticToolProvider for LspToolAdapter {
    fn diagnostics(&self, cwd: &std::path::Path, path: &std::path::Path) -> Result<String, String> {
        self.runtime
            .diagnostics(cwd, path)
            .map(|diagnostics| render_lsp_diagnostics(cwd, &diagnostics))
    }

    fn symbols(&self, cwd: &std::path::Path, path: &std::path::Path) -> Result<String, String> {
        self.runtime
            .symbols(cwd, path)
            .map(|symbols| render_lsp_symbols(cwd, &symbols))
    }

    fn references(
        &self,
        cwd: &std::path::Path,
        path: &std::path::Path,
        line: u32,
        character: u32,
    ) -> Result<String, String> {
        self.runtime
            .references(cwd, path, LspPosition { line, character })
            .map(|locations| render_lsp_locations(cwd, &locations))
    }
}

pub struct SessionEngine {
    cwd: PathBuf,
    provider: Box<dyn ModelProvider>,
    tools: ToolRegistry,
    permissions: PermissionEngine,
    store: SessionStore,
    workflows: WorkflowStore,
    lsp_runtime: Arc<LspRuntime>,
    messages: Vec<Message>,
    last_diff: Option<String>,
    runtime_snapshot: RuntimeSnapshot,
}

impl SessionEngine {
    pub fn new(cwd: impl Into<PathBuf>, provider: Box<dyn ModelProvider>) -> Result<Self, String> {
        Self::new_with_home(cwd, provider, Option::<PathBuf>::None)
    }

    pub fn new_with_home(
        cwd: impl Into<PathBuf>,
        provider: Box<dyn ModelProvider>,
        home_override: Option<PathBuf>,
    ) -> Result<Self, String> {
        let cwd = cwd.into();
        let default_snapshot = RuntimeSnapshot {
            cwd: cwd.clone(),
            provider_family: provider.provider_name().to_string(),
            model_label: provider.model().to_string(),
            permission_mode: PermissionMode::Default,
            config_summary: format!(
                "provider={} model={} permission_mode={} session_home=<default> timeout=<unknown> retries=<unknown>",
                provider.provider_name(),
                provider.model(),
                PermissionMode::Default.cli_name()
            ),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
        };
        Self::new_with_home_and_snapshot(cwd, provider, home_override, default_snapshot)
    }

    pub fn new_with_home_and_snapshot(
        cwd: impl Into<PathBuf>,
        provider: Box<dyn ModelProvider>,
        home_override: Option<PathBuf>,
        runtime_snapshot: RuntimeSnapshot,
    ) -> Result<Self, String> {
        let cwd = cwd.into();
        let store = match home_override {
            Some(home) => SessionStore::new_with_home(home, &cwd, None)?,
            None => SessionStore::new(&cwd, None)?,
        };
        let workflows = WorkflowStore::new(store.home_dir().to_path_buf(), &cwd)?;
        let engine = Self {
            cwd: cwd.clone(),
            provider,
            tools: ToolRegistry::builtin(),
            permissions: PermissionEngine::new(&cwd),
            store,
            workflows,
            lsp_runtime: Arc::new(LspRuntime::new(LspServerRegistry::default())),
            messages: Vec::new(),
            last_diff: None,
            runtime_snapshot,
        };
        engine.persist_meta("permission_mode", engine.permissions.mode().cli_name())?;
        let model = engine.provider.model().to_string();
        engine.persist_meta("model", &model)?;
        Ok(engine)
    }

    pub fn session_id(&self) -> &str {
        self.store.session_id()
    }

    pub fn provider_name(&self) -> &str {
        self.provider.provider_name()
    }

    pub fn model_name(&self) -> &str {
        self.provider.model()
    }

    pub fn mode(&self) -> PermissionMode {
        self.permissions.mode()
    }

    pub fn set_permission_mode(&mut self, mode: PermissionMode) -> Result<(), String> {
        self.permissions.set_mode(mode);
        self.persist_meta("permission_mode", mode.cli_name())
    }

    pub fn process_input_with_approval<F>(
        &mut self,
        input: &str,
        approver: &mut F,
    ) -> Result<Vec<EngineEvent>, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        if trimmed.starts_with('/') {
            return self.handle_command(trimmed, approver);
        }

        let mut events = Vec::new();
        let user_message = Message::new(Role::User, trimmed);
        self.messages.push(user_message.clone());
        self.store_entry(TranscriptEntry::Message {
            message: user_message,
        })?;

        for _ in 0..8 {
            let request = ModelRequest {
                session_id: self.session_id().to_string(),
                model: self.provider.model().to_string(),
                messages: self.messages.clone(),
                tools: self.tools.specs(),
                permission_mode: self.permissions.mode(),
            };
            let model_events = self.provider.next_events(&request)?;
            let mut observed_tool_call = false;
            let mut observed_text = false;
            for model_event in model_events {
                match model_event {
                    ModelEvent::AssistantText { content } => {
                        if content.trim().is_empty() {
                            continue;
                        }
                        observed_text = true;
                        let assistant = Message::new(Role::Assistant, &content);
                        self.messages.push(assistant.clone());
                        self.store_entry(TranscriptEntry::Message { message: assistant })?;
                        events.push(EngineEvent::Assistant(content));
                    }
                    ModelEvent::ToolCall(call) => {
                        observed_tool_call = true;
                        self.handle_tool_call(call, approver, &mut events)?;
                    }
                    ModelEvent::Done => {}
                }
            }
            if !observed_tool_call || observed_text {
                break;
            }
        }

        Ok(events)
    }

    fn handle_tool_call<F>(
        &mut self,
        call: ToolCall,
        approver: &mut F,
        events: &mut Vec<EngineEvent>,
    ) -> Result<(), String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let mut call = call;
        let reasoning_content = call.input.remove(PROVIDER_REASONING_CONTENT_KEY);
        let tool_spec = self
            .tools
            .spec(&call.name)
            .ok_or_else(|| format!("Model requested unknown tool `{}`", call.name))?;
        self.store_entry(TranscriptEntry::ToolCall { call: call.clone() })?;
        let mut assistant_input = call.input.clone();
        if let Some(reasoning_content) = reasoning_content {
            assistant_input.insert(
                PROVIDER_REASONING_CONTENT_KEY.to_string(),
                reasoning_content,
            );
        }
        let assistant_tool_call = Message {
            id: fresh_id("msg"),
            role: Role::Assistant,
            content: robocode_types::encode_tool_input(&assistant_input),
            timestamp: now_timestamp(),
            tool_name: Some(call.name.clone()),
            tool_call_id: Some(call.id.clone()),
        };
        self.messages.push(assistant_tool_call.clone());
        self.store_entry(TranscriptEntry::Message {
            message: assistant_tool_call,
        })?;
        events.push(EngineEvent::ToolCall(format!(
            "{} {}",
            call.name,
            robocode_types::encode_tool_input(&call.input)
        )));

        let mut decision = self.permissions.decide(&tool_spec, &call.input);
        if let PermissionDecision::Ask(ask) = &decision {
            let prompt = PermissionEngine::prompt_for(&call.name, ask, &call.input);
            let approval = approver(prompt);
            decision = self.permissions.apply_approval(approval, ask);
        }

        match decision {
            PermissionDecision::Allow(allow) => {
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: call.name.clone(),
                        decision: "allow".to_string(),
                        reason: format!("{:?}", allow.decision_reason),
                        message: allow.accept_feedback.clone(),
                    },
                })?;
                let result = self.tools.execute(
                    &call,
                    &ToolExecutionContext {
                        cwd: self.cwd.clone(),
                        semantic: Some(Arc::new(LspToolAdapter {
                            runtime: Arc::clone(&self.lsp_runtime),
                        })),
                    },
                )?;
                self.persist_tool_result(&result)?;
                events.push(EngineEvent::ToolResult(result.output.clone()));
            }
            PermissionDecision::Ask(_) => {
                unreachable!("ask decisions should be resolved before execution")
            }
            PermissionDecision::Deny(deny) => {
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: call.name.clone(),
                        decision: "deny".to_string(),
                        reason: format!("{:?}", deny.decision_reason),
                        message: Some(deny.message.clone()),
                    },
                })?;
                let system_message = Message::new(
                    Role::System,
                    format!("Permission denied for {}: {}", call.name, deny.message),
                );
                self.messages.push(system_message.clone());
                self.store_entry(TranscriptEntry::Message {
                    message: system_message,
                })?;
                events.push(EngineEvent::System(format!(
                    "Permission denied for {}: {}",
                    call.name, deny.message
                )));
            }
        }
        Ok(())
    }

    fn persist_tool_result(&mut self, result: &ToolResult) -> Result<(), String> {
        if let Some(diff) = &result.diff {
            self.last_diff = Some(diff.clone());
        }
        self.store_entry(TranscriptEntry::ToolResult {
            result: result.clone(),
        })?;
        let tool_message = Message {
            id: fresh_id("msg"),
            role: Role::Tool,
            content: result.output.clone(),
            timestamp: now_timestamp(),
            tool_name: Some(result.name.clone()),
            tool_call_id: Some(result.tool_call_id.clone()),
        };
        self.messages.push(tool_message);
        Ok(())
    }

    fn handle_command<F>(
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
            "/provider" => format!(
                "Current provider: {} ({})",
                self.provider.provider_name(),
                self.provider.model()
            ),
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

    fn handle_sessions(&self) -> Result<String, String> {
        let sessions = self.store.list_sessions_for_cwd()?;
        Ok(self.render_session_list(&sessions))
    }

    fn handle_tasks(&self) -> Result<String, String> {
        let state = self.workflows.load_task_state()?;
        let tasks = state.active_tasks();
        if tasks.is_empty() {
            return Ok("Project tasks:\n  <none>".to_string());
        }
        let mut lines = vec!["Project tasks:".to_string()];
        for task in tasks {
            lines.push(format!(
                "  {} [{} {}] {}",
                task.task_id,
                task.status.cli_name(),
                task.priority.cli_name(),
                task.title
            ));
        }
        Ok(lines.join("\n"))
    }

    fn handle_task_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_task_help());
        };
        match subcommand {
            "add" => {
                let title = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if title.trim().is_empty() {
                    return Err("Usage: /task add <title>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("task_add", &title, approver)?
                {
                    return Ok(denied);
                }
                let task_id = fresh_id("task");
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Created {
                        task_id: task_id.clone(),
                        title: title.clone(),
                        description: None,
                        priority: TaskPriority::Medium,
                        labels: Vec::new(),
                        assignee_hint: None,
                        parent_task_id: None,
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Created task {task_id} {title}"))
            }
            "view" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task view <task-id>".to_string())?;
                let state = self.workflows.load_task_state()?;
                let task = state
                    .task(task_id)
                    .ok_or_else(|| format!("No task found for `{task_id}`"))?;
                Ok(render_task_detail(task))
            }
            "update" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task update <task-id> <title>".to_string())?;
                let title = args.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
                if title.trim().is_empty() {
                    return Err("Usage: /task update <task-id> <title>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("task_update", &title, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Updated {
                        task_id: task_id.clone(),
                        update: TaskUpdate {
                            title: Some(title.clone()),
                            ..TaskUpdate::default()
                        },
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Updated task {task_id}: {title}"))
            }
            "status" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task status <task-id> <status>".to_string())?;
                let status = args
                    .get(2)
                    .and_then(|value| TaskStatus::parse_cli(value))
                    .ok_or_else(|| "Usage: /task status <task-id> <status>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_status", status.cli_name(), approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::StatusChanged {
                        task_id: task_id.clone(),
                        status,
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Set task {task_id} to {}", status.cli_name()))
            }
            "link" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task link <task-id> <depends-on-id>".to_string())?;
                let depends_on_id = args
                    .get(2)
                    .ok_or_else(|| "Usage: /task link <task-id> <depends-on-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_link", depends_on_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Linked {
                        task_id: task_id.clone(),
                        depends_on_id: depends_on_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!(
                    "Linked task {task_id} to dependency {depends_on_id}"
                ))
            }
            "block" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task block <task-id> <reason|task-id>".to_string())?;
                let reason = args.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
                if reason.trim().is_empty() {
                    return Err("Usage: /task block <task-id> <reason|task-id>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("task_block", &reason, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Blocked {
                        task_id: task_id.clone(),
                        blocker: TaskBlocker::Reason(reason.clone()),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Blocked task {task_id}: {reason}"))
            }
            "unblock" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task unblock <task-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_unblock", task_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Unblocked {
                        task_id: task_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Unblocked task {task_id}"))
            }
            "archive" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task archive <task-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_archive", task_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Archived {
                        task_id: task_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Archived task {task_id}"))
            }
            "restore" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task restore <task-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_restore", task_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Restored {
                        task_id: task_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Restored task {task_id}"))
            }
            "resume-context" => {
                let task_state = self.workflows.load_task_state()?;
                let memory_state = self.workflows.load_memory_state()?;
                let result = build_resume_context(ResumeContextInput {
                    task_state: &task_state,
                    memory_state: &memory_state,
                    current_session_id: Some(self.session_id().to_string()),
                    now: now_timestamp(),
                });
                for event in &result.derived_task_events {
                    self.workflows.append_task_domain_event_checked(event)?;
                }
                Ok(render_resume_context(&result.snapshot))
            }
            _ => Ok(format!(
                "Unknown task subcommand `{subcommand}`.\n\n{}",
                self.render_task_help()
            )),
        }
    }

    fn handle_memory_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let subcommand = args.first().map(String::as_str).unwrap_or("project");
        match subcommand {
            "project" => self.render_project_memory(),
            "session" => self.render_session_memory(),
            "suggest" if args.len() > 1 => {
                let content = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_suggest", &content, approver)?
                {
                    return Ok(denied);
                }
                let memory_id = fresh_id("mem");
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Suggested {
                        memory_id: memory_id.clone(),
                        kind: MemoryKind::Fact,
                        content: content.clone(),
                        source: MemorySource::AssistantSuggestion,
                        related_task_ids: Vec::new(),
                        confidence_hint: None,
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Suggested memory {memory_id} {content}"))
            }
            "suggest" => self.render_memory_suggestions(),
            "confirm" => {
                let memory_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /memory confirm <memory-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_confirm", memory_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Confirmed {
                        memory_id: memory_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Confirmed memory {memory_id}"))
            }
            "reject" => {
                let memory_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /memory reject <memory-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_reject", memory_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Rejected {
                        memory_id: memory_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Rejected memory {memory_id}"))
            }
            "prune" => {
                let memory_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /memory prune <memory-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_prune", memory_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Pruned {
                        memory_id: memory_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Pruned memory {memory_id}"))
            }
            "export" => self.render_memory_export(),
            "add" => {
                let content = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if content.trim().is_empty() {
                    return Err("Usage: /memory add <content>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_add", &content, approver)?
                {
                    return Ok(denied);
                }
                let memory_id = fresh_id("mem");
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Added {
                        memory_id: memory_id.clone(),
                        scope: MemoryScope::Session,
                        session_id: Some(self.session_id().to_string()),
                        kind: MemoryKind::Fact,
                        content: content.clone(),
                        source: MemorySource::Command,
                        related_task_ids: Vec::new(),
                        confidence_hint: None,
                        timestamp: now_timestamp(),
                    })?;
                Ok(format!("Added session memory {memory_id} {content}"))
            }
            _ => self.render_project_memory(),
        }
    }

    fn handle_lsp_command(&self, args: &[String]) -> Result<String, String> {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_lsp_help());
        };
        match subcommand {
            "help" => Ok(self.render_lsp_help()),
            "status" => Ok(self.render_lsp_status()),
            "diagnostics" => {
                let path = args
                    .get(1)
                    .ok_or_else(|| "Usage: /lsp diagnostics <path>".to_string())?;
                match self
                    .lsp_runtime
                    .diagnostics(&self.cwd, std::path::Path::new(path))
                {
                    Ok(diagnostics) => Ok(render_lsp_diagnostics(&self.cwd, &diagnostics)),
                    Err(error) => Ok(format!("LSP error: {error}")),
                }
            }
            "symbols" => {
                let path = args
                    .get(1)
                    .ok_or_else(|| "Usage: /lsp symbols <path>".to_string())?;
                match self
                    .lsp_runtime
                    .symbols(&self.cwd, std::path::Path::new(path))
                {
                    Ok(symbols) => Ok(render_lsp_symbols(&self.cwd, &symbols)),
                    Err(error) => Ok(format!("LSP error: {error}")),
                }
            }
            "references" => {
                let path = args.get(1).ok_or_else(|| {
                    "Usage: /lsp references <path> <line> <character>".to_string()
                })?;
                let line = parse_lsp_position_arg(args.get(2), "line")?;
                let character = parse_lsp_position_arg(args.get(3), "character")?;
                match self.lsp_runtime.references(
                    &self.cwd,
                    std::path::Path::new(path),
                    LspPosition { line, character },
                ) {
                    Ok(locations) => Ok(render_lsp_locations(&self.cwd, &locations)),
                    Err(error) => Ok(format!("LSP error: {error}")),
                }
            }
            _ => Ok(format!(
                "Unknown LSP subcommand `{subcommand}`.\n\n{}",
                self.render_lsp_help()
            )),
        }
    }

    fn render_project_memory(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.active_project_memory();
        if entries.is_empty() {
            return Ok("Project memory:\n  <none>".to_string());
        }
        let mut lines = vec!["Project memory:".to_string()];
        for entry in entries {
            lines.push(format!(
                "  {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        Ok(lines.join("\n"))
    }

    fn render_session_memory(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.active_session_memory(self.session_id());
        if entries.is_empty() {
            return Ok("Session memory:\n  <none>".to_string());
        }
        let mut lines = vec!["Session memory:".to_string()];
        for entry in entries {
            lines.push(format!(
                "  {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        Ok(lines.join("\n"))
    }

    fn render_memory_suggestions(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.pending_suggestions();
        if entries.is_empty() {
            return Ok("Pending memory suggestions:\n  <none>".to_string());
        }
        let mut lines = vec!["Pending memory suggestions:".to_string()];
        for entry in entries {
            lines.push(format!(
                "  {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        Ok(lines.join("\n"))
    }

    fn render_memory_export(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let mut lines = vec!["Memory export:".to_string(), "Project memory:".to_string()];
        for entry in state.active_project_memory() {
            lines.push(format!(
                "  - {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        lines.push("Session memory:".to_string());
        for entry in state.active_session_memory(self.session_id()) {
            lines.push(format!(
                "  - {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        if lines.len() == 3 {
            lines.push("  <none>".to_string());
        }
        Ok(lines.join("\n"))
    }

    fn ensure_workflow_permission<F>(
        &mut self,
        action: &str,
        preview: &str,
        approver: &mut F,
    ) -> Result<Option<String>, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let tool_name = format!("workflow_{action}");
        let tool = ToolSpec {
            name: tool_name.clone(),
            description: format!("Workflow mutation: {action}"),
            is_mutating: true,
            input_schema_hint: "workflow action".to_string(),
        };
        let mut input = ToolInput::new();
        input.insert("action".to_string(), action.to_string());
        input.insert("preview".to_string(), preview.to_string());
        let mut decision = self.permissions.decide(&tool, &input);
        if let PermissionDecision::Ask(ask) = &decision {
            let prompt = PermissionEngine::prompt_for(&tool_name, ask, &input);
            let approval = approver(prompt);
            decision = self.permissions.apply_approval(approval, ask);
        }
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
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: tool_name.clone(),
                        decision: "deny".to_string(),
                        reason: format!("{:?}", deny.decision_reason),
                        message: Some(deny.message.clone()),
                    },
                })?;
                Ok(Some(format!(
                    "Permission denied for {tool_name}: {}",
                    deny.message
                )))
            }
        }
    }

    fn handle_resume(&mut self, selector: Option<&str>) -> Result<String, String> {
        let Some(selector) = selector else {
            return self.handle_sessions();
        };
        if selector == "list" {
            return self.handle_sessions();
        }
        let loaded = match selector {
            "latest" => self.store.load_latest_for_cwd()?,
            other => self.resolve_resume_selector(other)?,
        };
        let Some((summary, entries)) = loaded else {
            return Ok("No resumable sessions found for the current project.".to_string());
        };
        let resumed_store = SessionStore::new_with_home(
            self.store.home_dir().to_path_buf(),
            self.cwd.clone(),
            Some(summary.session_id.clone()),
        )?;
        self.store = resumed_store;
        self.messages.clear();
        self.last_diff = None;
        self.permissions = PermissionEngine::new(&self.cwd);
        self.hydrate(entries);
        Ok(format!(
            "Resumed session {} ({})",
            summary.session_id,
            summary.title.unwrap_or_else(|| "untitled".to_string())
        ))
    }

    fn resolve_resume_selector(
        &self,
        selector: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        let sessions = self.store.list_sessions_for_cwd()?;
        if sessions.is_empty() {
            return Ok(None);
        }

        if let Some(loaded) = self.store.load_by_id_for_cwd(selector)? {
            return Ok(Some(loaded));
        }

        let matches: Vec<_> = sessions
            .iter()
            .filter(|summary| {
                summary.session_id != self.session_id()
                    && (summary.session_id.starts_with(selector)
                        || summary
                            .session_id
                            .trim_start_matches("session_")
                            .starts_with(selector))
            })
            .cloned()
            .collect();
        match matches.as_slice() {
            [] => self.resolve_resume_index(&sessions, selector),
            [summary] => {
                let entries = SessionStore::load_entries_from_path(std::path::Path::new(
                    &summary.transcript_path,
                ))?;
                Ok(Some((summary.clone(), entries)))
            }
            _ => Err(format!(
                "Session selector `{selector}` is ambiguous.\n\n{}",
                self.render_session_list(matches.as_slice())
            )),
        }
    }

    fn resolve_resume_index(
        &self,
        sessions: &[SessionSummary],
        selector: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        let index_selector = selector.strip_prefix('#').unwrap_or(selector);
        let Ok(index) = index_selector.parse::<usize>() else {
            return Ok(None);
        };
        if index == 0 {
            return Err("Session indexes start at 1.".to_string());
        }
        if let Some(summary) = sessions.get(index - 1) {
            let entries = SessionStore::load_entries_from_path(std::path::Path::new(
                &summary.transcript_path,
            ))?;
            return Ok(Some((summary.clone(), entries)));
        }
        Err(format!("No session found at index {index}."))
    }

    fn hydrate(&mut self, entries: Vec<TranscriptEntry>) {
        for entry in entries {
            match entry {
                TranscriptEntry::Message { message } => self.messages.push(message),
                TranscriptEntry::ToolResult { result } => {
                    if let Some(diff) = result.diff.clone() {
                        self.last_diff = Some(diff);
                    }
                    self.messages.push(Message {
                        id: fresh_id("msg"),
                        role: Role::Tool,
                        content: result.output,
                        timestamp: now_timestamp(),
                        tool_name: Some(result.name),
                        tool_call_id: Some(result.tool_call_id),
                    });
                }
                TranscriptEntry::SessionMeta { entry } => match entry.key.as_str() {
                    "permission_mode" => {
                        if let Some(mode) = PermissionMode::parse_cli(&entry.value) {
                            self.permissions.set_mode(mode);
                            self.runtime_snapshot.permission_mode = mode;
                        }
                    }
                    "model" => {
                        self.provider.set_model(entry.value.clone());
                        self.runtime_snapshot.model_label = self.provider.model().to_string();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn persist_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.store_entry(TranscriptEntry::SessionMeta {
            entry: SessionMetaEntry {
                timestamp: now_timestamp(),
                key: key.to_string(),
                value: value.to_string(),
            },
        })
    }

    fn store_entry(&self, entry: TranscriptEntry) -> Result<(), String> {
        self.store.append_entry(&entry)
    }

    fn run_named_tool<F>(
        &mut self,
        tool_name: &str,
        input: robocode_types::ToolInput,
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let mut events = Vec::new();
        let call = ToolCall {
            id: fresh_id("tool"),
            name: tool_name.to_string(),
            input,
        };
        self.handle_tool_call(call, approver, &mut events)?;
        let output = events
            .into_iter()
            .filter_map(|event| match event {
                EngineEvent::System(text)
                | EngineEvent::Assistant(text)
                | EngineEvent::ToolResult(text)
                | EngineEvent::Command(text) => Some(text),
                EngineEvent::ToolCall(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if output.trim().is_empty() {
            Ok("Tool completed".to_string())
        } else {
            Ok(output)
        }
    }

    fn handle_git_command<F>(&mut self, args: &[String], approver: &mut F) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_git_help());
        };
        match subcommand {
            "help" => Ok(self.render_git_help()),
            "status" => self.run_named_tool("git_status", Default::default(), approver),
            "diff" => {
                let mut input = robocode_types::ToolInput::new();
                if let Some(path) = args.get(1) {
                    input.insert("path".to_string(), path.clone());
                }
                self.run_named_tool("git_diff", input, approver)
            }
            "branch" => self.run_named_tool("git_branch", Default::default(), approver),
            "add" => {
                let all = args.iter().any(|arg| arg == "--all" || arg == "-A");
                let paths: Vec<String> = args
                    .iter()
                    .skip(1)
                    .filter(|arg| arg.as_str() != "--all" && arg.as_str() != "-A")
                    .cloned()
                    .collect();
                if paths.is_empty() && !all {
                    return Err("Usage: /git add [--all|-A] <path...>".to_string());
                }
                let mut input = robocode_types::ToolInput::new();
                if all {
                    input.insert("all".to_string(), "true".to_string());
                }
                if let Some(path) = paths.first() {
                    input.insert("path".to_string(), path.clone());
                }
                if paths.len() > 1 {
                    input.insert("paths".to_string(), paths.join("\n"));
                }
                self.run_named_tool("git_add", input, approver)
            }
            "restore" => {
                let staged = args.iter().any(|arg| arg == "--staged");
                let worktree = !args.iter().any(|arg| arg == "--worktree=false");
                let mut source = None;
                let mut paths = Vec::new();
                let mut iter = args.iter().skip(1);
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "--staged" | "--worktree" => {}
                        "--worktree=false" => {}
                        "--source" => {
                            source = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /git restore [--staged] [--source <ref>] <path...>"
                                    .to_string()
                            })?);
                        }
                        other if other.starts_with("--source=") => {
                            source = Some(other.trim_start_matches("--source=").to_string());
                        }
                        other => paths.push(other.to_string()),
                    }
                }
                if paths.is_empty() {
                    return Err(
                        "Usage: /git restore [--staged] [--source <ref>] <path...>".to_string()
                    );
                }
                let mut input = robocode_types::ToolInput::new();
                if staged {
                    input.insert("staged".to_string(), "true".to_string());
                }
                if !worktree {
                    input.insert("worktree".to_string(), "false".to_string());
                }
                if let Some(source) = source {
                    input.insert("source".to_string(), source);
                }
                if let Some(path) = paths.first() {
                    input.insert("path".to_string(), path.clone());
                }
                if paths.len() > 1 {
                    input.insert("paths".to_string(), paths.join("\n"));
                }
                self.run_named_tool("git_restore", input, approver)
            }
            "switch" | "checkout" => {
                let branch = args
                    .get(1)
                    .cloned()
                    .ok_or_else(|| "Usage: /git switch <branch> [--create]".to_string())?;
                let create = args.iter().any(|arg| arg == "--create" || arg == "-c");
                let mut input = robocode_types::ToolInput::new();
                input.insert("branch".to_string(), branch);
                if create {
                    input.insert("create".to_string(), "true".to_string());
                }
                self.run_named_tool("git_switch", input, approver)
            }
            "commit" => {
                let all = args.iter().any(|arg| arg == "--all" || arg == "-a");
                let message_parts: Vec<String> = args
                    .iter()
                    .skip(1)
                    .filter(|arg| arg.as_str() != "--all" && arg.as_str() != "-a")
                    .cloned()
                    .collect();
                if message_parts.is_empty() {
                    return Err("Usage: /git commit [--all] <message>".to_string());
                }
                let mut input = robocode_types::ToolInput::new();
                input.insert("message".to_string(), message_parts.join(" "));
                if all {
                    input.insert("all".to_string(), "true".to_string());
                }
                self.run_named_tool("git_commit", input, approver)
            }
            "push" => {
                let set_upstream = args
                    .iter()
                    .any(|arg| arg == "--set-upstream" || arg == "-u");
                let positional: Vec<String> = args
                    .iter()
                    .skip(1)
                    .filter(|arg| arg.as_str() != "--set-upstream" && arg.as_str() != "-u")
                    .cloned()
                    .collect();
                let mut input = robocode_types::ToolInput::new();
                if set_upstream {
                    input.insert("set_upstream".to_string(), "true".to_string());
                }
                match positional.as_slice() {
                    [] => {}
                    [branch] => {
                        input.insert("branch".to_string(), branch.clone());
                    }
                    [remote, branch] => {
                        input.insert("remote".to_string(), remote.clone());
                        input.insert("branch".to_string(), branch.clone());
                    }
                    _ => {
                        return Err(
                            "Usage: /git push [branch] | [remote branch] [--set-upstream|-u]"
                                .to_string(),
                        );
                    }
                }
                self.run_named_tool("git_push", input, approver)
            }
            "stash" => {
                let Some(action) = args.get(1).map(String::as_str) else {
                    return Ok(self.render_git_stash_help());
                };
                match action {
                    "help" => Ok(self.render_git_stash_help()),
                    "list" => self.run_named_tool("git_stash_list", Default::default(), approver),
                    "push" | "save" => {
                        let mut include_untracked = false;
                        let mut message = None;
                        let mut paths = Vec::new();
                        let mut iter = args.iter().skip(2);
                        while let Some(arg) = iter.next() {
                            match arg.as_str() {
                                "--include-untracked" | "-u" => include_untracked = true,
                                "--message" | "-m" => {
                                    message = Some(iter.next().cloned().ok_or_else(|| {
                                        "Usage: /git stash push [-m <message>] [-u] [path...]"
                                            .to_string()
                                    })?);
                                }
                                other if other.starts_with("--message=") => {
                                    message =
                                        Some(other.trim_start_matches("--message=").to_string());
                                }
                                other => paths.push(other.to_string()),
                            }
                        }
                        let mut input = robocode_types::ToolInput::new();
                        if include_untracked {
                            input.insert("include_untracked".to_string(), "true".to_string());
                        }
                        if let Some(message) = message {
                            input.insert("message".to_string(), message);
                        }
                        if let Some(path) = paths.first() {
                            input.insert("path".to_string(), path.clone());
                        }
                        if paths.len() > 1 {
                            input.insert("paths".to_string(), paths.join("\n"));
                        }
                        self.run_named_tool("git_stash_push", input, approver)
                    }
                    "pop" => {
                        let mut input = robocode_types::ToolInput::new();
                        if let Some(stash) = args.get(2) {
                            input.insert("stash".to_string(), stash.clone());
                        }
                        self.run_named_tool("git_stash_pop", input, approver)
                    }
                    "drop" => {
                        let mut input = robocode_types::ToolInput::new();
                        if let Some(stash) = args.get(2) {
                            input.insert("stash".to_string(), stash.clone());
                        }
                        self.run_named_tool("git_stash_drop", input, approver)
                    }
                    _ => Ok(format!(
                        "Unknown git stash subcommand `{action}`.\n\n{}",
                        self.render_git_stash_help()
                    )),
                }
            }
            "worktree" => {
                let Some(action) = args.get(1).map(String::as_str) else {
                    return Ok(self.render_git_worktree_help());
                };
                match action {
                    "help" => Ok(self.render_git_worktree_help()),
                    "list" => {
                        self.run_named_tool("git_worktree_list", Default::default(), approver)
                    }
                    "add" => {
                        let path = args.get(2).cloned().ok_or_else(|| {
                            "Usage: /git worktree add <path> [branch] [--create]".to_string()
                        })?;
                        let create = args.iter().any(|arg| arg == "--create" || arg == "-b");
                        let branch = args
                            .iter()
                            .skip(3)
                            .find(|arg| arg.as_str() != "--create" && arg.as_str() != "-b")
                            .cloned()
                            .or_else(|| {
                                args.get(3)
                                    .filter(|arg| {
                                        arg.as_str() != "--create" && arg.as_str() != "-b"
                                    })
                                    .cloned()
                            });
                        let mut input = robocode_types::ToolInput::new();
                        input.insert("path".to_string(), path);
                        if let Some(branch) = branch {
                            input.insert("branch".to_string(), branch);
                        }
                        if create {
                            input.insert("create".to_string(), "true".to_string());
                        }
                        self.run_named_tool("git_worktree_add", input, approver)
                    }
                    "remove" => {
                        let path = args.get(2).cloned().ok_or_else(|| {
                            "Usage: /git worktree remove <path> [--force]".to_string()
                        })?;
                        let force = args.iter().any(|arg| arg == "--force" || arg == "-f");
                        let mut input = robocode_types::ToolInput::new();
                        input.insert("path".to_string(), path);
                        if force {
                            input.insert("force".to_string(), "true".to_string());
                        }
                        self.run_named_tool("git_worktree_remove", input, approver)
                    }
                    _ => Ok(format!(
                        "Unknown git worktree subcommand `{action}`.\n\n{}",
                        self.render_git_worktree_help()
                    )),
                }
            }
            _ => Ok(format!(
                "Unknown git subcommand `{subcommand}`.\n\n{}",
                self.render_git_help()
            )),
        }
    }

    fn handle_web_command<F>(&mut self, args: &[String], approver: &mut F) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_web_help());
        };
        match subcommand {
            "help" => Ok(self.render_web_help()),
            "search" => {
                let mut limit = None;
                let mut site = None;
                let mut query_parts = Vec::new();
                let mut iter = args.iter().skip(1);
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "--limit" => {
                            limit = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /web search <query> [--limit <n>] [--site <domain>]"
                                    .to_string()
                            })?);
                        }
                        "--site" => {
                            site = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /web search <query> [--limit <n>] [--site <domain>]"
                                    .to_string()
                            })?);
                        }
                        other if other.starts_with("--limit=") => {
                            limit = Some(other.trim_start_matches("--limit=").to_string());
                        }
                        other if other.starts_with("--site=") => {
                            site = Some(other.trim_start_matches("--site=").to_string());
                        }
                        other => query_parts.push(other.to_string()),
                    }
                }
                if query_parts.is_empty() {
                    return Err(
                        "Usage: /web search <query> [--limit <n>] [--site <domain>]".to_string()
                    );
                }
                let mut input = robocode_types::ToolInput::new();
                input.insert("query".to_string(), query_parts.join(" "));
                if let Some(limit) = limit {
                    input.insert("limit".to_string(), limit);
                }
                if let Some(site) = site {
                    input.insert("site".to_string(), site);
                }
                self.run_named_tool("web_search", input, approver)
            }
            "fetch" => {
                let url = args.get(1).cloned().ok_or_else(|| {
                    "Usage: /web fetch <url> [--max-bytes <n>] [--raw]".to_string()
                })?;
                let mut max_bytes = None;
                let mut raw = false;
                let mut iter = args.iter().skip(2);
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "--raw" => raw = true,
                        "--max-bytes" => {
                            max_bytes = Some(iter.next().cloned().ok_or_else(|| {
                                "Usage: /web fetch <url> [--max-bytes <n>] [--raw]".to_string()
                            })?);
                        }
                        other if other.starts_with("--max-bytes=") => {
                            max_bytes = Some(other.trim_start_matches("--max-bytes=").to_string());
                        }
                        _ => {}
                    }
                }
                let mut input = robocode_types::ToolInput::new();
                input.insert("url".to_string(), url);
                if let Some(max_bytes) = max_bytes {
                    input.insert("max_bytes".to_string(), max_bytes);
                }
                if raw {
                    input.insert("raw".to_string(), "true".to_string());
                }
                self.run_named_tool("web_fetch", input, approver)
            }
            _ => Ok(format!(
                "Unknown web subcommand `{subcommand}`.\n\n{}",
                self.render_web_help()
            )),
        }
    }

    fn render_help(&self) -> String {
        [
            "RoboCode commands:",
            "",
            "Runtime:",
            "  /help                Show available commands",
            "  /provider            Show current provider and model",
            "  /model [name]        Show or change the active model label",
            "  /permissions [mode]  Show or change permission mode",
            "  /plan [on|off]       Toggle plan mode",
            "  /status              Show current runtime status",
            "  /config              Show resolved runtime configuration",
            "  /doctor              Check local dependency availability",
            "",
            "Sessions:",
            "  /sessions            List prior sessions for this project",
            "  /resume [selector]   List or resume by latest, #index, or id prefix",
            "  /diff                Show the latest file diff recorded in session",
            "",
            "Repository and web:",
            "  /git <subcommand>    Git status/diff/add/push/worktree flows",
            "  /web <subcommand>    Search or fetch web content",
            "",
            "Code intelligence:",
            "  /lsp status          Show language server configuration",
            "  /lsp diagnostics <path>",
            "  /lsp symbols <path>",
            "  /lsp references <path> <line> <character>",
            "",
            "Workflows:",
            "  /tasks               List active project tasks",
            "  /task <subcommand>   Manage tasks or render resume context",
            "  /memory <subcommand> Manage project/session memory",
            "",
            "Fallback tool syntax:",
            "  tool read_file path=Cargo.toml",
            "  tool grep pattern=fn path=src",
        ]
        .join("\n")
    }

    fn render_status(&self) -> String {
        [
            "Runtime status:".to_string(),
            format!("  Session: {}", self.session_id()),
            format!("  CWD: {}", self.cwd.display()),
            format!("  Provider: {}", self.provider.provider_name()),
            format!("  Model: {}", self.provider.model()),
            format!("  Permission mode: {}", self.permissions.mode().cli_name()),
            format!("  Transcript: {}", self.store.transcript_path().display()),
            format!("  Session home: {}", self.store.home_dir().display()),
            format!("  Index: {}", self.store.index_db_path().display()),
        ]
        .join("\n")
    }

    fn render_config(&self) -> String {
        let loaded_files = if self.runtime_snapshot.loaded_config_files.is_empty() {
            "<none>".to_string()
        } else {
            self.runtime_snapshot
                .loaded_config_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let overrides = if self.runtime_snapshot.startup_overrides.is_empty() {
            "<none>".to_string()
        } else {
            self.runtime_snapshot.startup_overrides.join(", ")
        };
        [
            "Runtime configuration:".to_string(),
            format!("  {}", self.runtime_snapshot.config_summary),
            format!("  Loaded config files: {}", loaded_files),
            format!("  Startup overrides: {}", overrides),
        ]
        .join("\n")
    }

    fn render_doctor(&self) -> String {
        DoctorReport::from_probe(system_dependency_status).render()
    }

    fn render_task_help(&self) -> String {
        [
            "Task commands:",
            "  /tasks",
            "  /task add <title>",
            "  /task resume-context",
        ]
        .join("\n")
    }

    fn render_git_help(&self) -> String {
        [
            "Git commands:",
            "  /git status",
            "  /git diff [path]",
            "  /git branch",
            "  /git add [--all|-A] <path...>",
            "  /git restore [--staged] [--source <ref>] <path...>",
            "  /git switch <branch> [--create]",
            "  /git commit [--all] <message>",
            "  /git push [branch] | [remote branch] [--set-upstream|-u]",
            "  /git stash <list|push|pop|drop>",
            "  /git worktree <list|add|remove>",
        ]
        .join("\n")
    }

    fn render_web_help(&self) -> String {
        [
            "Web commands:",
            "  /web search <query> [--limit <n>] [--site <domain>]",
            "  /web fetch <url> [--max-bytes <n>] [--raw]",
        ]
        .join("\n")
    }

    fn render_lsp_help(&self) -> String {
        [
            "LSP commands:",
            "  /lsp status",
            "  /lsp diagnostics <path>",
            "  /lsp symbols <path>",
            "  /lsp references <path> <line> <character>",
            "",
            "Positions are zero-based LSP line and character offsets.",
        ]
        .join("\n")
    }

    fn render_lsp_status(&self) -> String {
        let status = self.lsp_runtime.status();
        let configured = if status.configured_servers.is_empty() {
            "<none>".to_string()
        } else {
            status.configured_servers.join(", ")
        };
        let running = if status.running_servers.is_empty() {
            "<none>".to_string()
        } else {
            status.running_servers.join(", ")
        };
        [
            "LSP status:".to_string(),
            format!("  configured: {configured}"),
            format!("  running: {running}"),
            format!("  cached_sessions: {}", status.cached_sessions),
            format!("  open_documents: {}", status.open_documents),
            format!(
                "  last_error: {}",
                status.last_error.unwrap_or_else(|| "<none>".to_string())
            ),
        ]
        .join("\n")
    }

    fn render_git_stash_help(&self) -> String {
        [
            "Git stash commands:",
            "  /git stash list",
            "  /git stash push [-m <message>] [-u] [path...]",
            "  /git stash pop [stash@{0}]",
            "  /git stash drop [stash@{0}]",
        ]
        .join("\n")
    }

    fn render_git_worktree_help(&self) -> String {
        [
            "Git worktree commands:",
            "  /git worktree list",
            "  /git worktree add <path> [branch] [--create]",
            "  /git worktree remove <path> [--force]",
        ]
        .join("\n")
    }

    fn render_session_list(&self, sessions: &[SessionSummary]) -> String {
        if sessions.is_empty() {
            return "No resumable sessions found for the current project.".to_string();
        }
        let mut lines = vec![
            "Sessions for this project:".to_string(),
            "  Use `/resume latest`, `/resume #<index>`, or `/resume <session-id-prefix>`."
                .to_string(),
        ];
        for (index, summary) in sessions.iter().enumerate() {
            let title = summary
                .title
                .clone()
                .unwrap_or_else(|| "untitled".to_string());
            let preview = summary
                .last_preview
                .clone()
                .unwrap_or_else(|| "No preview available".to_string());
            let current = if summary.session_id == self.session_id() {
                " [current]"
            } else {
                ""
            };
            lines.push(format!(
                "  {}. {}{}  {}  {}",
                index + 1,
                summary.session_id,
                current,
                format_relative_age(summary.last_updated_at),
                title
            ));
            lines.push(format!(
                "     messages={} tools={} commands={} last={}",
                summary.message_count,
                summary.tool_call_count,
                summary.command_count,
                summary
                    .last_activity_kind
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            ));
            lines.push(format!("     {}", preview));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyStatus {
    Ok,
    Missing,
    #[cfg_attr(not(test), allow(dead_code))]
    NotRequired,
}

impl DependencyStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::NotRequired => "not required for current path",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DoctorReport {
    checks: Vec<(&'static str, DependencyStatus)>,
}

impl DoctorReport {
    fn from_probe<F>(mut probe: F) -> Self
    where
        F: FnMut(&str) -> DependencyStatus,
    {
        Self {
            checks: ["git", "rg", "sqlite3", "curl"]
                .into_iter()
                .map(|tool| (tool, probe(tool)))
                .collect(),
        }
    }

    fn render(&self) -> String {
        let mut lines = vec!["Environment diagnostics:".to_string()];
        lines.extend(
            self.checks
                .iter()
                .map(|(tool, status)| format!("  {tool}: {}", status.label())),
        );
        lines.join("\n")
    }
}

fn system_dependency_status(tool: &str) -> DependencyStatus {
    match Command::new(tool).arg("--version").output() {
        Ok(output) if output.status.success() => DependencyStatus::Ok,
        Ok(_) => DependencyStatus::Missing,
        Err(_) => DependencyStatus::Missing,
    }
}

fn parse_lsp_position_arg(raw: Option<&String>, name: &str) -> Result<u32, String> {
    raw.ok_or_else(|| "Usage: /lsp references <path> <line> <character>".to_string())?
        .parse::<u32>()
        .map_err(|_| {
            format!(
                "Usage: /lsp references <path> <line> <character>; line and character must be zero-based integers (`{name}` was invalid)"
            )
        })
}

fn render_lsp_diagnostics(cwd: &std::path::Path, diagnostics: &[LspDiagnostic]) -> String {
    if diagnostics.is_empty() {
        return "LSP diagnostics:\n  <none>".to_string();
    }

    let mut grouped = BTreeMap::<String, Vec<&LspDiagnostic>>::new();
    for diagnostic in diagnostics {
        grouped
            .entry(render_lsp_path(cwd, &diagnostic.path))
            .or_default()
            .push(diagnostic);
    }

    let mut lines = vec![
        render_section_title("LSP diagnostics")
            .trim_end()
            .to_string(),
    ];

    for (path, entries) in grouped {
        lines.push(render_subsection_title(&path));
        for diagnostic in entries {
            let source = diagnostic
                .source
                .as_ref()
                .map(|source| {
                    diagnostic
                        .code
                        .as_ref()
                        .map(|code| format!("{source}/{code}"))
                        .unwrap_or_else(|| source.clone())
                })
                .or_else(|| diagnostic.code.as_ref().cloned())
                .unwrap_or_else(|| "unknown".to_string());

            lines.push(format!(
                "  {}:{} {} [{}] {}",
                diagnostic.range.start.line,
                diagnostic.range.start.character,
                severity_label(diagnostic.severity),
                source,
                diagnostic.message
            ));
        }
    }

    join_lines(&lines)
}

fn render_lsp_symbols(cwd: &std::path::Path, symbols: &[LspSymbol]) -> String {
    if symbols.is_empty() {
        return "LSP symbols:\n  <none>".to_string();
    }

    let mut grouped = BTreeMap::<String, Vec<&LspSymbol>>::new();
    for symbol in symbols {
        grouped
            .entry(render_lsp_path(cwd, &symbol.path))
            .or_default()
            .push(symbol);
    }

    let mut lines = vec![render_section_title("LSP symbols").trim_end().to_string()];
    for (path, entries) in grouped {
        lines.push(render_subsection_title(&path));
        for symbol in entries {
            lines.push(format!(
                "  {} [{}] {}:{}{}",
                symbol.name,
                symbol_kind_label(symbol.kind),
                symbol.range.start.line,
                symbol.range.start.character,
                symbol
                    .container_name
                    .as_ref()
                    .map(|container| format!(" in {container}"))
                    .unwrap_or_default()
            ));
        }
    }

    join_lines(&lines)
}

fn render_lsp_locations(cwd: &std::path::Path, locations: &[LspLocation]) -> String {
    if locations.is_empty() {
        return "LSP references:\n  <none>".to_string();
    }
    let mut lines = vec![
        render_section_title("LSP references")
            .trim_end()
            .to_string(),
    ];
    for location in locations {
        lines.push(format!(
            "  {}:{}:{}",
            render_lsp_path(cwd, &location.path),
            location.range.start.line,
            location.range.start.character
        ));
    }
    join_lines(&lines)
}

fn render_lsp_path(cwd: &std::path::Path, path: &str) -> String {
    let path_buf = std::path::Path::new(path);
    path_buf
        .strip_prefix(cwd)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}

fn severity_label(severity: Option<u8>) -> &'static str {
    match severity {
        Some(1) => "error",
        Some(2) => "warning",
        Some(3) => "info",
        Some(4) => "hint",
        _ => "unknown",
    }
}

fn symbol_kind_label(kind: u32) -> &'static str {
    match kind {
        5 => "class",
        6 => "method",
        10 => "enum",
        11 => "interface",
        12 => "function",
        13 => "variable",
        19 => "namespace",
        22 => "field",
        23 => "struct",
        _ => "symbol",
    }
}

fn render_resume_context(snapshot: &ResumeContextSnapshot) -> String {
    let mut lines = vec!["Resume context:".to_string()];
    lines.push("  Active tasks:".to_string());
    if snapshot.active_tasks.is_empty() {
        lines.push("    <none>".to_string());
    } else {
        for task in &snapshot.active_tasks {
            lines.push(format!(
                "    {} [{}] {}",
                task.task_id, task.priority, task.title
            ));
        }
    }
    lines.push("  Blocked tasks:".to_string());
    if snapshot.blocked_tasks.is_empty() {
        lines.push("    <none>".to_string());
    } else {
        for task in &snapshot.blocked_tasks {
            lines.push(format!(
                "    {} blocked by {}",
                task.task_id,
                task.blocked_by
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            ));
        }
    }
    lines.push("  Project memory:".to_string());
    if snapshot.relevant_project_memory.is_empty() {
        lines.push("    <none>".to_string());
    } else {
        for entry in &snapshot.relevant_project_memory {
            lines.push(format!("    {} {}", entry.memory_id, entry.content));
        }
    }
    lines.push("Suggested next steps:".to_string());
    for step in &snapshot.suggested_next_steps {
        lines.push(format!("  - {step}"));
    }
    lines.join("\n")
}

fn render_task_detail(task: &robocode_types::TaskRecord) -> String {
    [
        "Task detail:".to_string(),
        format!("  ID: {}", task.task_id),
        format!("  Title: {}", task.title),
        format!("  Status: {}", task.status.cli_name()),
        format!("  Priority: {}", task.priority.cli_name()),
        format!(
            "  Blocked by: {}",
            task.blocked_by
                .clone()
                .unwrap_or_else(|| "<none>".to_string())
        ),
    ]
    .join("\n")
}

fn format_relative_age(timestamp: u64) -> String {
    let now = now_timestamp();
    if timestamp >= now {
        return "just now".to_string();
    }
    let delta = now - timestamp;
    if delta < 60 {
        format!("{delta}s ago")
    } else if delta < 60 * 60 {
        format!("{}m ago", delta / 60)
    } else if delta < 60 * 60 * 24 {
        format!("{}h ago", delta / 3_600)
    } else {
        format!("{}d ago", delta / 86_400)
    }
}

#[cfg(test)]
mod tests;
