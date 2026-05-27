use std::{sync::Arc, time::Instant};

use crate::{
    EngineEvent, PROVIDER_REASONING_CONTENT_KEY, SessionEngine,
    context_bundle::{context_evidence_rows, render_provider_context_message},
    lsp_tools::LspToolAdapter,
    lsp_tools::render_lsp_diagnostics,
    presentation::render_permission_denial,
};
use robocode_lsp::SemanticProvider;
use robocode_model::ModelRequestControl;
use robocode_permissions::PermissionEngine;
use robocode_tools::ToolExecutionContext;
use robocode_types::{
    AgentTaskStatus, ApprovalResponse, Message, ModelEvent, ModelRequest, PermissionDecision,
    PermissionLogEntry, Role, ToolCall, ToolResult, TranscriptEntry, fresh_id, now_timestamp,
};

impl SessionEngine {
    pub fn process_input_with_approval<F>(
        &mut self,
        input: &str,
        approver: &mut F,
    ) -> Result<Vec<EngineEvent>, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        self.process_input_with_approval_and_control(input, approver, &ModelRequestControl::new())
    }

    pub fn process_input_with_approval_and_control<F>(
        &mut self,
        input: &str,
        approver: &mut F,
        control: &ModelRequestControl,
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
        let context_bundle = self.build_main_context_bundle(trimmed);
        self.last_context_bundle = Some(context_bundle.clone());
        let mut provider_task = self.provider_task(
            trimmed,
            AgentTaskStatus::Thinking,
            "thinking through latest prompt",
            15,
        );
        provider_task
            .evidence
            .extend(context_evidence_rows(&context_bundle));
        self.upsert_agent_task(provider_task.clone());

        for _ in 0..8 {
            provider_task.status = AgentTaskStatus::Thinking.as_str().to_string();
            provider_task.activity = "waiting for provider response".to_string();
            provider_task.progress = provider_task.progress.max(20);
            provider_task.updated_at = Some(now_millis());
            self.upsert_agent_task(provider_task.clone());
            let mut request_messages = self.messages.clone();
            let context_message = Message::new(
                Role::System,
                render_provider_context_message(&context_bundle),
            );
            if request_messages
                .last()
                .is_some_and(|message| message.role == Role::User)
            {
                let insert_at = request_messages.len().saturating_sub(1);
                request_messages.insert(insert_at, context_message);
            } else {
                request_messages.push(context_message);
            }
            let request = ModelRequest {
                session_id: self.session_id().to_string(),
                model: self.provider.model().to_string(),
                messages: request_messages,
                tools: self.tools.specs(),
                permission_mode: self.permissions.mode(),
            };
            let request_started = Instant::now();
            let model_events = match self.provider.next_events_with_control(&request, control) {
                Ok(events) => {
                    let usage = aggregate_model_usage(&events);
                    self.provider_telemetry.record_success(
                        request_started.elapsed(),
                        events.len(),
                        usage,
                    );
                    events
                }
                Err(err) => {
                    self.provider_telemetry
                        .record_failure(request_started.elapsed(), &err);
                    provider_task.status = AgentTaskStatus::Failed.as_str().to_string();
                    provider_task.activity = format!("provider error: {err}");
                    provider_task.progress = 100;
                    provider_task.updated_at = Some(now_millis());
                    provider_task.result = Some(err.clone());
                    self.upsert_agent_task(provider_task);
                    return Err(err);
                }
            };
            let mut observed_tool_call = false;
            let mut observed_text = false;
            for model_event in model_events {
                match model_event {
                    ModelEvent::AssistantText { content } => {
                        if content.trim().is_empty() {
                            continue;
                        }
                        provider_task.status = AgentTaskStatus::Streaming.as_str().to_string();
                        provider_task.activity = "streaming assistant response".to_string();
                        provider_task.progress = provider_task.progress.max(65);
                        provider_task.updated_at = Some(now_millis());
                        self.upsert_agent_task(provider_task.clone());
                        observed_text = true;
                        let assistant = Message::new(Role::Assistant, &content);
                        self.messages.push(assistant.clone());
                        self.store_entry(TranscriptEntry::Message { message: assistant })?;
                        events.push(EngineEvent::Assistant(content));
                    }
                    ModelEvent::ToolCall(call) => {
                        observed_tool_call = true;
                        provider_task.status = AgentTaskStatus::RunningTool.as_str().to_string();
                        provider_task.activity = format!("requested tool `{}`", call.name);
                        provider_task.progress = provider_task.progress.max(75);
                        provider_task.updated_at = Some(now_millis());
                        self.upsert_agent_task(provider_task.clone());
                        let _ = self.handle_tool_call(call, approver, &mut events)?;
                    }
                    ModelEvent::Usage(_) => {}
                    ModelEvent::Done => {}
                }
            }
            if !observed_tool_call || observed_text {
                break;
            }
        }
        provider_task.status = AgentTaskStatus::Done.as_str().to_string();
        provider_task.activity = "provider turn complete".to_string();
        provider_task.progress = 100;
        provider_task.updated_at = Some(now_millis());
        provider_task.result = Some("turn complete".to_string());
        self.upsert_agent_task(provider_task);

        Ok(events)
    }

    fn handle_tool_call<F>(
        &mut self,
        call: ToolCall,
        approver: &mut F,
        events: &mut Vec<EngineEvent>,
    ) -> Result<ToolResult, String>
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
        let encoded_input = robocode_types::encode_tool_input(&call.input);
        let mut task = self.tool_task(
            &call.id,
            &call.name,
            &encoded_input,
            AgentTaskStatus::RunningTool,
            "checking permissions",
            15,
        );
        self.upsert_agent_task(task.clone());
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
            call.name, encoded_input
        )));

        let mut decision = self.permissions.decide(&tool_spec, &call.input);
        if let PermissionDecision::Ask(ask) = &decision {
            task.status = AgentTaskStatus::WaitingApproval.as_str().to_string();
            task.activity = format!("waiting for approval: `{}`", call.name);
            task.progress = 35;
            task.permissions
                .push("operator approval required".to_string());
            task.updated_at = Some(now_millis());
            self.upsert_agent_task(task.clone());
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
                task.status = AgentTaskStatus::RunningTool.as_str().to_string();
                task.activity = format!("running `{}`", call.name);
                task.progress = 55;
                task.decision = Some("allow".to_string());
                task.updated_at = Some(now_millis());
                self.upsert_agent_task(task.clone());
                let result = self.tools.execute(
                    &call,
                    &ToolExecutionContext {
                        cwd: self.cwd.clone(),
                        semantic: Some(Arc::new(LspToolAdapter {
                            runtime: Arc::clone(&self.lsp_runtime),
                        })),
                    },
                )?;
                let post_edit_diagnostics = if result.success {
                    self.post_edit_diagnostics_message(&call)
                } else {
                    None
                };
                self.persist_tool_result(&result)?;
                task.status = if result.success {
                    AgentTaskStatus::Done.as_str().to_string()
                } else {
                    AgentTaskStatus::Failed.as_str().to_string()
                };
                task.activity = if result.success {
                    format!("`{}` completed", call.name)
                } else {
                    format!("`{}` failed", call.name)
                };
                task.progress = 100;
                task.result = Some(result.output.clone());
                task.evidence.push(format!("success {}", result.success));
                if let Some(code) = result.exit_code {
                    task.evidence.push(format!("exit_code {code}"));
                }
                task.updated_at = Some(now_millis());
                self.upsert_agent_task(task);
                events.push(EngineEvent::ToolResult(result.output.clone()));
                if let Some(message) = post_edit_diagnostics {
                    let system_message = Message::new(Role::System, message.clone());
                    self.messages.push(system_message.clone());
                    self.store_entry(TranscriptEntry::Message {
                        message: system_message,
                    })?;
                    events.push(EngineEvent::System(message));
                }
                Ok(result)
            }
            PermissionDecision::Ask(_) => {
                unreachable!("ask decisions should be resolved before execution")
            }
            PermissionDecision::Deny(deny) => {
                let reason = format!("{:?}", deny.decision_reason);
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: call.name.clone(),
                        decision: "deny".to_string(),
                        reason: reason.clone(),
                        message: Some(deny.message.clone()),
                    },
                })?;
                task.status = AgentTaskStatus::Cancelled.as_str().to_string();
                task.activity = format!("denied `{}`", call.name);
                task.progress = 100;
                task.decision = Some("deny".to_string());
                task.result = Some(deny.message.clone());
                task.updated_at = Some(now_millis());
                self.upsert_agent_task(task);
                let rendered_denial = render_permission_denial(&call.name, &reason, &deny.message);
                let result = ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    output: rendered_denial.clone(),
                    diff: None,
                    success: false,
                    exit_code: None,
                };
                self.persist_tool_result(&result)?;
                events.push(EngineEvent::ToolResult(result.output.clone()));
                let system_message = Message::new(Role::System, rendered_denial.clone());
                self.messages.push(system_message.clone());
                self.store_entry(TranscriptEntry::Message {
                    message: system_message,
                })?;
                events.push(EngineEvent::System(rendered_denial));
                Ok(result)
            }
        }
    }

    fn post_edit_diagnostics_message(&self, call: &ToolCall) -> Option<String> {
        if !matches!(call.name.as_str(), "write_file" | "edit_file") {
            return None;
        }
        let path = std::path::Path::new(call.input.get("path")?);
        let diagnostics = self.lsp_runtime.diagnostics(&self.cwd, path).ok()?;
        if diagnostics.is_empty() {
            return None;
        }
        Some(format!(
            "Post-edit LSP diagnostics after `{}`:\n{}",
            call.name,
            render_lsp_diagnostics(&self.cwd, &diagnostics)
        ))
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

    pub(crate) fn run_named_tool<F>(
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
        let _ = self.handle_tool_call(call, approver, &mut events)?;
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

    pub(crate) fn run_named_tool_result<F>(
        &mut self,
        tool_name: &str,
        input: robocode_types::ToolInput,
        approver: &mut F,
    ) -> Result<ToolResult, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let mut events = Vec::new();
        let call = ToolCall {
            id: fresh_id("tool"),
            name: tool_name.to_string(),
            input,
        };
        self.handle_tool_call(call, approver, &mut events)
    }
}

fn aggregate_model_usage(events: &[ModelEvent]) -> Option<robocode_types::ModelUsage> {
    events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::Usage(usage) => Some(usage),
            _ => None,
        })
        .fold(None, |current, usage| {
            Some(match current {
                None => usage.clone(),
                Some(current) => robocode_types::ModelUsage {
                    input_tokens: add_optional(current.input_tokens, usage.input_tokens),
                    output_tokens: add_optional(current.output_tokens, usage.output_tokens),
                    total_tokens: add_optional(current.total_tokens, usage.total_tokens),
                    cost_micro_usd: add_optional(current.cost_micro_usd, usage.cost_micro_usd),
                },
            })
        })
}

fn add_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn now_millis() -> u128 {
    u128::from(now_timestamp()) * 1000
}
