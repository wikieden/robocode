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
    AgentTaskStatus, ApprovalResponse, ContextBundleRecord, Message, ModelEvent, ModelRequest,
    PermissionDecision, PermissionLogEntry, PermissionMode, Role, ToolCall, ToolResult,
    TranscriptEntry, fresh_id, now_timestamp,
};

const PROVIDER_REQUEST_CHAR_BUDGET: usize = 48_000;
const PROVIDER_RECENT_HISTORY_LIMIT: usize = 8;
const PROVIDER_HISTORY_MESSAGE_CHAR_LIMIT: usize = 1_500;
const PROVIDER_TAIL_MESSAGE_CHAR_LIMIT: usize = 4_000;
const PROVIDER_SUMMARY_LINE_LIMIT: usize = 180;
const PROVIDER_SUMMARY_LINE_COUNT: usize = 20;
const PROVIDER_RETRY_REQUEST_CHAR_BUDGET: usize = 16_000;
const PROVIDER_RETRY_HISTORY_MESSAGE_CHAR_LIMIT: usize = 800;
const PROVIDER_RETRY_TAIL_MESSAGE_CHAR_LIMIT: usize = 1_200;

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

        let mut retried_request_too_large = false;
        let max_tool_iterations = self.turn_budget.max_tool_iterations.max(1);
        // Stays true only if the loop exhausts its iteration ceiling while tool
        // calls are still pending — i.e. the turn was paused, not completed.
        let mut paused_on_budget = true;
        // Whether a mutating edit succeeded this turn, gating post-edit verification.
        let mut edited_this_turn = false;
        for _ in 0..max_tool_iterations {
            provider_task.status = AgentTaskStatus::Thinking.as_str().to_string();
            provider_task.activity = "waiting for provider response".to_string();
            provider_task.progress = provider_task.progress.max(20);
            provider_task.updated_at = Some(now_millis());
            self.upsert_agent_task(provider_task.clone());
            let request_messages = if retried_request_too_large {
                build_provider_retry_request_messages(&self.messages, &context_bundle)
            } else {
                build_provider_request_messages(&self.messages, &context_bundle)
            };
            let request = ModelRequest {
                session_id: self.session_id().to_string(),
                model: self.provider.model().to_string(),
                messages: request_messages,
                tools: self.tools.specs(),
                work_mode: self.runtime_snapshot.work_mode,
                permission_mode: self.permissions.mode(),
                permission_level: self.runtime_snapshot.permission_level,
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
                    if crate::provider_commands::is_request_too_large_provider_failure(&err)
                        && !retried_request_too_large
                    {
                        retried_request_too_large = true;
                        let note =
                            "Provider request was too large; retrying with compacted context.";
                        let system_message = Message::new(Role::System, note);
                        self.messages.push(system_message.clone());
                        self.store_entry(TranscriptEntry::Message {
                            message: system_message,
                        })?;
                        events.push(EngineEvent::System(note.to_string()));
                        provider_task.status = AgentTaskStatus::Thinking.as_str().to_string();
                        provider_task.activity =
                            "compacting provider context after request-too-large".to_string();
                        provider_task.progress = provider_task.progress.max(35);
                        provider_task
                            .evidence
                            .push("provider_retry request_too_large compacted".to_string());
                        provider_task.updated_at = Some(now_millis());
                        self.upsert_agent_task(provider_task.clone());
                        continue;
                    }
                    let rendered_error = self
                        .provider_model_recovery_prompt(&err)
                        .map(|hint| format!("{err}\n\n{hint}"))
                        .unwrap_or_else(|| err.clone());
                    provider_task.status = AgentTaskStatus::Failed.as_str().to_string();
                    provider_task.activity = format!("provider error: {err}");
                    provider_task.progress = 100;
                    provider_task.updated_at = Some(now_millis());
                    provider_task.result = Some(rendered_error.clone());
                    self.upsert_agent_task(provider_task);
                    return Err(rendered_error);
                }
            };
            let mut observed_tool_call = false;
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
                        let tool_result = self.handle_tool_call(call, approver, &mut events)?;
                        if tool_result.success
                            && matches!(tool_result.name.as_str(), "write_file" | "edit_file")
                        {
                            edited_this_turn = true;
                        }
                    }
                    ModelEvent::Usage(_) => {}
                    ModelEvent::Done => {}
                }
            }
            // A turn ends only when the model stops requesting tools. Assistant
            // text alongside a tool call does NOT end the turn, so every tool
            // result is fed back for a follow-up turn.
            if !observed_tool_call {
                paused_on_budget = false;
                break;
            }
        }
        if paused_on_budget {
            let note = format!(
                "RoboCode turn budget exhausted after {max_tool_iterations} tool iterations; pausing this turn. Send another message to continue."
            );
            let system_message = Message::new(Role::System, &note);
            self.messages.push(system_message.clone());
            self.store_entry(TranscriptEntry::Message {
                message: system_message,
            })?;
            provider_task
                .evidence
                .push(format!("turn_budget_exhausted {max_tool_iterations}"));
            events.push(EngineEvent::System(note));
        }
        // Post-edit verification: once per turn, only if an edit succeeded and the
        // turn completed (not budget-paused). Feeds the result into the transcript
        // and gates the turn's done-status.
        let verification = if edited_this_turn && !paused_on_budget {
            self.run_verify_command()
        } else {
            None
        };
        if let Some((ok, message)) = &verification {
            let system_message = Message::new(Role::System, message.clone());
            self.messages.push(system_message.clone());
            self.store_entry(TranscriptEntry::Message {
                message: system_message,
            })?;
            provider_task.evidence.push(format!(
                "post_edit_verification {}",
                if *ok { "passed" } else { "failed" }
            ));
            events.push(EngineEvent::System(message.clone()));
        }
        let verification_failed = matches!(&verification, Some((false, _)));
        let needs_attention = paused_on_budget || verification_failed;

        provider_task.status = if needs_attention {
            AgentTaskStatus::Blocked.as_str().to_string()
        } else {
            AgentTaskStatus::Done.as_str().to_string()
        };
        provider_task.activity = if paused_on_budget {
            format!("turn paused: tool budget exhausted after {max_tool_iterations} iterations")
        } else if verification_failed {
            "turn ended: post-edit verification failed".to_string()
        } else {
            "provider turn complete".to_string()
        };
        provider_task.progress = 100;
        provider_task.updated_at = Some(now_millis());
        provider_task.result = Some(if paused_on_budget {
            format!("paused: tool budget exhausted after {max_tool_iterations} iterations")
        } else if verification_failed {
            "post-edit verification failed".to_string()
        } else {
            "turn complete".to_string()
        });
        provider_task.evidence.push(format!(
            "session_spend tokens={} cost_micro_usd={}",
            self.provider_telemetry.total_tokens,
            self.provider_telemetry.total_cost_micro_usd.unwrap_or(0)
        ));
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
                let result = match self.tools.execute(
                    &call,
                    &ToolExecutionContext {
                        cwd: self.cwd.clone(),
                        semantic: Some(Arc::new(LspToolAdapter {
                            runtime: Arc::clone(&self.lsp_runtime),
                        })),
                    },
                ) {
                    Ok(result) => result,
                    Err(error) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        output: format!("Tool `{}` failed: {error}", call.name),
                        diff: None,
                        success: false,
                        exit_code: None,
                    },
                };
                let post_edit_diagnostics = if result.success {
                    self.post_edit_diagnostics_message(&call)
                } else {
                    None
                };
                let failure_guidance = (!result.success).then(|| {
                    let class = classify_tool_failure(&call.name, &result.output, result.exit_code);
                    (
                        class,
                        format!(
                            "Tool `{}` failure class: {} — {}",
                            call.name,
                            class.as_str(),
                            class.next_action()
                        ),
                    )
                });
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
                if let Some((class, _)) = failure_guidance.as_ref() {
                    task.evidence
                        .push(format!("failure_class {}", class.as_str()));
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
                if let Some((_, message)) = failure_guidance {
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
                task.evidence.push("failure_class denied".to_string());
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
                let denial_guidance = format!(
                    "Tool `{}` failure class: {} — {}",
                    call.name,
                    ToolFailureClass::Denied.as_str(),
                    ToolFailureClass::Denied.next_action()
                );
                let guidance_message = Message::new(Role::System, denial_guidance.clone());
                self.messages.push(guidance_message.clone());
                self.store_entry(TranscriptEntry::Message {
                    message: guidance_message,
                })?;
                events.push(EngineEvent::System(denial_guidance));
                Ok(result)
            }
        }
    }

    /// Run the configured post-edit verification command (e.g. `cargo test`) and
    /// summarize the outcome. Returns `(success, message)`, or `None` when no
    /// command is configured or plan mode forbids running it. Synchronous for now;
    /// async/non-blocking verification is a later (gap #7) increment.
    fn run_verify_command(&self) -> Option<(bool, String)> {
        let command = self.verify_command.as_deref()?;
        if self.permissions.mode() == PermissionMode::Plan {
            return None;
        }
        let mut input = robocode_types::ToolInput::new();
        input.insert("command".to_string(), command.to_string());
        let verify_call = ToolCall {
            id: fresh_id("verify"),
            name: "shell".to_string(),
            input,
        };
        let ctx = ToolExecutionContext {
            cwd: self.cwd.clone(),
            semantic: None,
        };
        match self.tools.execute(&verify_call, &ctx) {
            Ok(result) => {
                let status = if result.success { "passed" } else { "FAILED" };
                let exit = result
                    .exit_code
                    .map(|code| format!(" (exit {code})"))
                    .unwrap_or_default();
                let tail = compact_chars(&result.output, 1_500);
                Some((
                    result.success,
                    format!("Post-edit verification (`{command}`) {status}{exit}:\n{tail}"),
                ))
            }
            Err(error) => Some((
                false,
                format!("Post-edit verification (`{command}`) could not run: {error}"),
            )),
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

fn build_provider_request_messages(
    transcript: &[Message],
    context_bundle: &ContextBundleRecord,
) -> Vec<Message> {
    build_provider_request_messages_with_limits(
        transcript,
        context_bundle,
        ProviderRequestLimits::normal(),
        false,
    )
}

fn build_provider_retry_request_messages(
    transcript: &[Message],
    context_bundle: &ContextBundleRecord,
) -> Vec<Message> {
    build_provider_request_messages_with_limits(
        transcript,
        context_bundle,
        ProviderRequestLimits::request_too_large_retry(),
        true,
    )
}

#[derive(Debug, Clone, Copy)]
struct ProviderRequestLimits {
    request_char_budget: usize,
    recent_history_limit: usize,
    history_message_char_limit: usize,
    tail_message_char_limit: usize,
    summary_line_limit: usize,
    summary_line_count: usize,
}

impl ProviderRequestLimits {
    fn normal() -> Self {
        Self {
            request_char_budget: PROVIDER_REQUEST_CHAR_BUDGET,
            recent_history_limit: PROVIDER_RECENT_HISTORY_LIMIT,
            history_message_char_limit: PROVIDER_HISTORY_MESSAGE_CHAR_LIMIT,
            tail_message_char_limit: PROVIDER_TAIL_MESSAGE_CHAR_LIMIT,
            summary_line_limit: PROVIDER_SUMMARY_LINE_LIMIT,
            summary_line_count: PROVIDER_SUMMARY_LINE_COUNT,
        }
    }

    fn request_too_large_retry() -> Self {
        Self {
            request_char_budget: PROVIDER_RETRY_REQUEST_CHAR_BUDGET,
            recent_history_limit: PROVIDER_RECENT_HISTORY_LIMIT / 2,
            history_message_char_limit: PROVIDER_RETRY_HISTORY_MESSAGE_CHAR_LIMIT,
            tail_message_char_limit: PROVIDER_RETRY_TAIL_MESSAGE_CHAR_LIMIT,
            summary_line_limit: PROVIDER_SUMMARY_LINE_LIMIT / 2,
            summary_line_count: PROVIDER_SUMMARY_LINE_COUNT / 2,
        }
    }
}

fn build_provider_request_messages_with_limits(
    transcript: &[Message],
    context_bundle: &ContextBundleRecord,
    limits: ProviderRequestLimits,
    request_too_large_retry: bool,
) -> Vec<Message> {
    let context_message = Message::new(
        Role::System,
        render_provider_context_message(context_bundle),
    );
    let Some(latest_user_index) = transcript
        .iter()
        .rposition(|message| message.role == Role::User)
    else {
        return vec![context_message];
    };

    let history = &transcript[..latest_user_index];
    let live_turn = &transcript[latest_user_index..];
    let mut request = Vec::new();
    if let Some(summary) = compact_transcript_summary(history, limits) {
        request.push(Message::new(Role::System, summary));
    }
    if request_too_large_retry {
        request.push(Message::new(
            Role::System,
            "RoboCode compacted provider request after a request-too-large error. The full transcript remains available in local audit storage.",
        ));
    }
    request.extend(recent_plain_history(history, limits));
    request.push(context_message);
    request.extend(
        live_turn
            .iter()
            .map(|message| compact_live_turn_message(message, limits)),
    );
    dedupe_repeated_tool_outputs(&mut request);
    fit_provider_request_budget(request, limits.request_char_budget)
}

fn recent_plain_history(history: &[Message], limits: ProviderRequestLimits) -> Vec<Message> {
    let mut recent = Vec::new();
    for message in history.iter().rev() {
        if recent.len() >= limits.recent_history_limit {
            break;
        }
        if message.tool_name.is_some()
            || message.tool_call_id.is_some()
            || message.role == Role::Tool
        {
            continue;
        }
        let mut compacted = message.clone();
        compacted.content = compact_chars(&compacted.content, limits.history_message_char_limit);
        recent.push(compacted);
    }
    recent.reverse();
    recent
}

fn compact_transcript_summary(
    history: &[Message],
    limits: ProviderRequestLimits,
) -> Option<String> {
    if history.is_empty() {
        return None;
    }
    let omitted = history.len();
    let start = history.len().saturating_sub(limits.summary_line_count);
    let lines = history[start..]
        .iter()
        .map(|message| {
            let tool = message
                .tool_name
                .as_deref()
                .map(|name| format!(" tool={name}"))
                .unwrap_or_default();
            format!(
                "- {}{}: {}",
                message.role.as_str(),
                tool,
                one_line(&message.content, limits.summary_line_limit)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "RoboCode compacted transcript summary\nOmitted durable messages: {omitted}\nRecent omitted facts:\n{lines}\n\nThe full transcript remains in RoboCode storage; this summary is only the provider request view."
    ))
}

fn compact_live_turn_message(message: &Message, limits: ProviderRequestLimits) -> Message {
    let mut compacted = message.clone();
    compacted.content = compact_chars(&compacted.content, limits.tail_message_char_limit);
    compacted
}

/// Collapse identical repeated tool outputs in a provider request: when the same
/// tool output appears more than once (e.g. the model re-read an unchanged file),
/// every occurrence but the last is replaced with a short stub. Saves request
/// budget without losing content — the latest copy stays verbatim, and message
/// order / tool_call_id pairing are preserved.
fn dedupe_repeated_tool_outputs(messages: &mut [Message]) {
    use std::collections::HashMap;
    let mut last_index_for_content: HashMap<String, usize> = HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        if message.role == Role::Tool && !message.content.is_empty() {
            last_index_for_content.insert(message.content.clone(), index);
        }
    }
    let stub_targets: Vec<(usize, usize)> = messages
        .iter()
        .enumerate()
        .filter(|(index, message)| {
            message.role == Role::Tool
                && !message.content.is_empty()
                && last_index_for_content.get(&message.content) != Some(index)
        })
        .map(|(index, message)| (index, message.content.chars().count()))
        .collect();
    for (index, original_len) in stub_targets {
        messages[index].content = format!(
            "[RoboCode: identical tool output elided to save context; {original_len} chars repeated in a later read]"
        );
    }
}

fn fit_provider_request_budget(
    mut messages: Vec<Message>,
    request_char_budget: usize,
) -> Vec<Message> {
    while total_message_chars(&messages) > request_char_budget && messages.len() > 2 {
        let removable_recent_index = messages
            .iter()
            .position(|message| message.content.contains("RoboCode ContextBundle"))
            .filter(|context_index| *context_index > 1)
            .and_then(|context_index| {
                (1..context_index).find(|index| !is_provider_request_protected(&messages[*index]))
            });
        if let Some(index) = removable_recent_index {
            messages.remove(index);
        } else if let Some(index) = messages.iter().position(|message| {
            !is_provider_request_protected(message)
                && message.role != Role::User
                && message.tool_call_id.is_none()
        }) {
            messages.remove(index);
        } else {
            break;
        }
    }
    messages
}

fn is_provider_request_protected(message: &Message) -> bool {
    message
        .content
        .contains("RoboCode compacted transcript summary")
        || message.content.contains("RoboCode ContextBundle")
        || message
            .content
            .contains("RoboCode compacted provider request after a request-too-large error")
}

fn total_message_chars(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum()
}

fn compact_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let keep = max_chars.saturating_sub(96);
    let head = keep / 2;
    let tail = keep.saturating_sub(head);
    let start = input.chars().take(head).collect::<String>();
    let end = input
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!(
        "{start}\n...[RoboCode compacted {} chars for provider request budget]...\n{end}",
        input.chars().count().saturating_sub(keep)
    )
}

fn one_line(input: &str, max_chars: usize) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    compact_chars(&collapsed, max_chars)
}

/// Heuristic classification of a failed tool result, used to feed the model a
/// concrete next action instead of a bare error string. Best-effort and string
/// based; promote to a shared `robocode-types` contract when supervised roles
/// (Phase E) need to branch on it programmatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolFailureClass {
    NotFound,
    DirectoryTarget,
    CompileError,
    TestFailure,
    Timeout,
    Denied,
    Other,
}

impl ToolFailureClass {
    fn as_str(&self) -> &'static str {
        match self {
            ToolFailureClass::NotFound => "not_found",
            ToolFailureClass::DirectoryTarget => "directory_target",
            ToolFailureClass::CompileError => "compile_error",
            ToolFailureClass::TestFailure => "test_failure",
            ToolFailureClass::Timeout => "timeout",
            ToolFailureClass::Denied => "denied",
            ToolFailureClass::Other => "other",
        }
    }

    fn next_action(&self) -> &'static str {
        match self {
            ToolFailureClass::NotFound => {
                "Target not found. Use `glob`/`grep` to locate it or correct the path before retrying."
            }
            ToolFailureClass::DirectoryTarget => {
                "Path is a directory. Use `glob` or `grep` to inspect its contents instead."
            }
            ToolFailureClass::CompileError => {
                "Compilation failed. Read the cited diagnostics and fix those lines before re-running."
            }
            ToolFailureClass::TestFailure => {
                "Tests failed. Inspect the failing cases and fix the cause before re-running."
            }
            ToolFailureClass::Timeout => {
                "The command timed out. Narrow its scope or raise the limit, then retry."
            }
            ToolFailureClass::Denied => {
                "Blocked by permission policy. Adjust the request or narrow its scope; in plan mode, mutating tools require build mode, or ask the operator to approve."
            }
            ToolFailureClass::Other => {
                "The tool failed. Review the error output and adjust inputs before retrying."
            }
        }
    }
}

fn classify_tool_failure(
    tool_name: &str,
    output: &str,
    exit_code: Option<i32>,
) -> ToolFailureClass {
    let lower = output.to_ascii_lowercase();
    if lower.contains("no such file")
        || lower.contains("not found")
        || lower.contains("does not exist")
    {
        return ToolFailureClass::NotFound;
    }
    if lower.contains("is a directory") {
        return ToolFailureClass::DirectoryTarget;
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return ToolFailureClass::Timeout;
    }
    if lower.contains("error[e") || (lower.contains("error:") && lower.contains(".rs")) {
        return ToolFailureClass::CompileError;
    }
    let nonzero_exit = exit_code.map(|code| code != 0).unwrap_or(false);
    if tool_name == "shell"
        && nonzero_exit
        && (lower.contains("test result: failed")
            || lower.contains("failures:")
            || lower.contains("assertion"))
    {
        return ToolFailureClass::TestFailure;
    }
    ToolFailureClass::Other
}
