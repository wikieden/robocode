use std::path::PathBuf;

use robocode_model::ModelProvider;
use robocode_permissions::PermissionEngine;
use robocode_session::SessionStore;
use robocode_tools::{ToolExecutionContext, ToolRegistry};
use robocode_types::{
    ApprovalResponse, CommandLogEntry, Message, ModelEvent, ModelRequest, PermissionDecision,
    PermissionLogEntry, PermissionMode, Role, SessionMetaEntry, ToolCall, ToolResult,
    TranscriptEntry, fresh_id, now_timestamp,
};

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
    messages: Vec<Message>,
    last_diff: Option<String>,
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
        let store = match home_override {
            Some(home) => SessionStore::new_with_home(home, &cwd, None)?,
            None => SessionStore::new(&cwd, None)?,
        };
        let engine = Self {
            cwd: cwd.clone(),
            provider,
            tools: ToolRegistry::builtin(),
            permissions: PermissionEngine::new(&cwd),
            store,
            messages: Vec::new(),
            last_diff: None,
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
        let tool_spec = self
            .tools
            .spec(&call.name)
            .ok_or_else(|| format!("Model requested unknown tool `{}`", call.name))?;
        self.store_entry(TranscriptEntry::ToolCall { call: call.clone() })?;
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
            "/permissions" => {
                if let Some(mode) = args.first() {
                    let parsed = PermissionMode::parse_cli(mode)
                        .ok_or_else(|| format!("Unknown permission mode `{mode}`"))?;
                    self.permissions.set_mode(parsed);
                    self.persist_meta("permission_mode", parsed.cli_name())?;
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
                format!(
                    "Plan mode is now {}",
                    if next_mode == PermissionMode::Plan {
                        "on"
                    } else {
                        "off"
                    }
                )
            }
            "/resume" => self.handle_resume(args.first().map(String::as_str))?,
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
            "/git" => self.handle_git_command(&args, approver)?,
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

    fn handle_resume(&mut self, selector: Option<&str>) -> Result<String, String> {
        let loaded = match selector {
            Some("latest") | None => self.store.load_latest_for_cwd()?,
            Some(session_id) => self.store.load_by_id_for_cwd(session_id)?,
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
                        }
                    }
                    "model" => self.provider.set_model(entry.value),
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
            _ => Ok(format!(
                "Unknown git subcommand `{subcommand}`.\n\n{}",
                self.render_git_help()
            )),
        }
    }

    fn render_help(&self) -> String {
        [
            "RoboCode commands:",
            "  /help                Show available commands",
            "  /provider            Show current provider and model",
            "  /model [name]        Show or change the active model label",
            "  /permissions [mode]  Show or change permission mode",
            "  /plan [on|off]       Toggle plan mode",
            "  /resume [id|latest]  Resume a prior session for this project",
            "  /diff                Show the latest file diff recorded in session",
            "  /git <subcommand>    Git status/diff/branch/switch/commit",
            "",
            "Fallback tool syntax:",
            "  tool read_file path=Cargo.toml",
            "  tool grep pattern=fn path=src",
        ]
        .join("\n")
    }

    fn render_git_help(&self) -> String {
        [
            "Git commands:",
            "  /git status",
            "  /git diff [path]",
            "  /git branch",
            "  /git switch <branch> [--create]",
            "  /git commit [--all] <message>",
        ]
        .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;

    use robocode_model::ModelProvider;
    use robocode_types::{
        ApprovalResponse, ModelEvent, ModelRequest, PermissionMode, ToolCall, ToolInput,
    };

    use super::*;

    struct SequenceProvider {
        model: String,
        turns: VecDeque<Vec<ModelEvent>>,
    }

    impl SequenceProvider {
        fn new(turns: Vec<Vec<ModelEvent>>) -> Self {
            Self {
                model: "test-model".to_string(),
                turns: turns.into(),
            }
        }
    }

    impl ModelProvider for SequenceProvider {
        fn provider_name(&self) -> &str {
            "sequence"
        }

        fn model(&self) -> &str {
            &self.model
        }

        fn set_model(&mut self, model: String) {
            self.model = model;
        }

        fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
            Ok(self
                .turns
                .pop_front()
                .unwrap_or_else(|| vec![ModelEvent::Done]))
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("robocode_core_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn single_turn_text_response_is_recorded() {
        let home = temp_dir("single_home");
        let cwd = temp_dir("single_cwd");
        let provider = Box::new(SequenceProvider::new(vec![vec![
            ModelEvent::AssistantText {
                content: "hello from test".to_string(),
            },
        ]]));
        let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let mut approver = |_prompt| ApprovalResponse {
            approved: true,
            feedback: None,
        };
        let events = engine
            .process_input_with_approval("hi", &mut approver)
            .unwrap();
        assert!(
            events.iter().any(
                |event| matches!(event, EngineEvent::Assistant(text) if text.contains("hello"))
            )
        );
    }

    #[test]
    fn tool_loop_executes_and_reinjects_result() {
        let home = temp_dir("tool_home");
        let cwd = temp_dir("tool_cwd");
        fs::write(cwd.join("sample.txt"), "hello").unwrap();
        let mut read_input = ToolInput::new();
        read_input.insert("path".to_string(), "sample.txt".to_string());
        let provider = Box::new(SequenceProvider::new(vec![
            vec![ModelEvent::ToolCall(ToolCall {
                id: "tool_read".to_string(),
                name: "read_file".to_string(),
                input: read_input,
            })],
            vec![ModelEvent::AssistantText {
                content: "Tool finished".to_string(),
            }],
        ]));
        let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let mut approver = |_prompt| ApprovalResponse {
            approved: true,
            feedback: None,
        };
        let events = engine
            .process_input_with_approval("read it", &mut approver)
            .unwrap();
        assert!(
            events.iter().any(
                |event| matches!(event, EngineEvent::ToolResult(text) if text.contains("hello"))
            )
        );
        assert!(events.iter().any(
            |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Tool finished"))
        ));
    }

    #[test]
    fn plan_mode_blocks_mutating_tools() {
        let home = temp_dir("plan_home");
        let cwd = temp_dir("plan_cwd");
        let mut write_input = ToolInput::new();
        write_input.insert("path".to_string(), "a.txt".to_string());
        write_input.insert("content".to_string(), "new".to_string());
        let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::ToolCall(
            ToolCall {
                id: "tool_write".to_string(),
                name: "write_file".to_string(),
                input: write_input,
            },
        )]]));
        let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let mut approver = |_prompt| ApprovalResponse {
            approved: true,
            feedback: None,
        };
        engine
            .process_input_with_approval("/plan on", &mut approver)
            .unwrap();
        assert_eq!(engine.mode(), PermissionMode::Plan);
        let events = engine
            .process_input_with_approval("write a file", &mut approver)
            .unwrap();
        assert!(events.iter().any(
            |event| matches!(event, EngineEvent::System(text) if text.contains("Permission denied"))
        ));
    }

    #[test]
    fn resume_restores_previous_session() {
        let home = temp_dir("resume_home");
        let cwd = temp_dir("resume_cwd");
        let provider_a = Box::new(SequenceProvider::new(vec![vec![
            ModelEvent::AssistantText {
                content: "first reply".to_string(),
            },
        ]]));
        let mut engine_a =
            SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
        let mut approver = |_prompt| ApprovalResponse {
            approved: true,
            feedback: None,
        };
        let session_id = engine_a.session_id().to_string();
        engine_a
            .process_input_with_approval("hello", &mut approver)
            .unwrap();

        let provider_b = Box::new(SequenceProvider::new(vec![]));
        let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
        let output = engine_b
            .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
            .unwrap();
        assert!(output.iter().any(
            |event| matches!(event, EngineEvent::Command(text) if text.contains("Resumed session"))
        ));
    }

    #[test]
    fn git_status_command_uses_tool_runtime() {
        let home = temp_dir("git_status_home");
        let cwd = temp_dir("git_status_cwd");
        let init = std::process::Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(init.success());
        std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();

        let provider = Box::new(SequenceProvider::new(vec![]));
        let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let mut approvals = 0usize;
        let mut approver = |_prompt| {
            approvals += 1;
            ApprovalResponse {
                approved: true,
                feedback: None,
            }
        };
        let events = engine
            .process_input_with_approval("/git status", &mut approver)
            .unwrap();
        assert_eq!(approvals, 0);
        assert!(
            events.iter().any(
                |event| matches!(event, EngineEvent::Command(text) if text.contains("demo.txt"))
            )
        );
    }

    #[test]
    fn git_switch_requests_approval() {
        let home = temp_dir("git_switch_home");
        let cwd = temp_dir("git_switch_cwd");
        let init = std::process::Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(init.success());

        let provider = Box::new(SequenceProvider::new(vec![]));
        let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let mut approvals = 0usize;
        let mut approver = |_prompt| {
            approvals += 1;
            ApprovalResponse {
                approved: true,
                feedback: None,
            }
        };
        let events = engine
            .process_input_with_approval("/git switch feature/demo --create", &mut approver)
            .unwrap();
        assert_eq!(approvals, 1);
        assert!(events.iter().any(
            |event| matches!(event, EngineEvent::Command(text) if text.contains("Switched") || text.contains("feature/demo"))
        ));
    }
}
