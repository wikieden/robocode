use std::path::PathBuf;
use std::sync::Arc;

mod doctor;
mod formatting;
mod git_commands;
mod lsp_tools;
mod presentation;
mod runtime_views;
mod web_commands;
mod workflow_commands;

#[cfg(test)]
pub(crate) use doctor::DependencyStatus;
pub(crate) use doctor::{DoctorReport, system_dependency_status};
use formatting::{format_relative_age, render_resume_context, render_task_detail};
use lsp_tools::LspToolAdapter;
use robocode_lsp::{LspRuntime, LspServerRegistry};
use robocode_model::ModelProvider;
use robocode_permissions::PermissionEngine;
use robocode_session::SessionStore;
use robocode_tools::{ToolExecutionContext, ToolRegistry};
use robocode_types::{
    ApprovalResponse, CommandLogEntry, Message, ModelEvent, ModelRequest, PermissionDecision,
    PermissionLogEntry, PermissionMode, Role, RuntimeSnapshot, SessionMetaEntry, SessionSummary,
    ToolCall, ToolResult, TranscriptEntry, fresh_id, now_timestamp,
};
use robocode_workflows::stores::WorkflowStore;

const PROVIDER_REASONING_CONTENT_KEY: &str = "__provider_reasoning_content";

#[derive(Debug, Clone)]
pub enum EngineEvent {
    System(String),
    Assistant(String),
    ToolCall(String),
    ToolResult(String),
    Command(String),
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
}

#[cfg(test)]
mod tests;
