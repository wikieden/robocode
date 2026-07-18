use std::{sync::Arc, time::Instant};

use crate::{
    EngineEvent, PROVIDER_REASONING_CONTENT_KEY, SessionEngine,
    context_bundle::{ContextBuildMode, context_evidence_rows, render_provider_context_message},
    lsp_tools::LspToolAdapter,
    lsp_tools::render_lsp_diagnostics,
    presentation::render_permission_denial,
};
use viden_lsp::SemanticProvider;
use viden_permissions::PermissionEngine;
use viden_provider::ModelRequestControl;
use viden_tools::ToolExecutionContext;
use viden_types::{
    AgentTaskStatus, ApprovalResponse, ContextBundleRecord, CostAmount, CostEstimate, CostScope,
    CostUsageOutcome, CostUsageRecord, Message, ModelEvent, ModelRequest, ModelUsage,
    PermissionDecision, PermissionLogEntry, Role, TokenUsage, ToolCall, ToolResult,
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
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
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
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        self.process_input_with_optional_context_bundle(input, approver, control, None)
    }

    pub(crate) fn process_input_with_built_context_bundle_and_control<F>(
        &mut self,
        input: &str,
        approver: &mut F,
        control: &ModelRequestControl,
        built_context_bundle: crate::context_bundle::BuiltContextBundle,
    ) -> Result<Vec<EngineEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        self.process_input_with_optional_context_bundle(
            input,
            approver,
            control,
            Some(built_context_bundle),
        )
    }

    fn process_input_with_optional_context_bundle<F>(
        &mut self,
        input: &str,
        approver: &mut F,
        control: &ModelRequestControl,
        built_context_bundle: Option<crate::context_bundle::BuiltContextBundle>,
    ) -> Result<Vec<EngineEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
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
        let built_context_bundle = built_context_bundle.unwrap_or_else(|| {
            self.build_main_context_bundle_with_mode(trimmed, ContextBuildMode::Normal)
        });
        let mut context_bundle = built_context_bundle.bundle;
        self.last_context_runtime_events = built_context_bundle.events;
        self.last_context_bundle = Some(context_bundle.clone());
        if built_context_bundle.hard_exceeded {
            return Err(format!(
                "context hard limit exceeded before provider request: used {} tokens, hard limit {}. reduce input, narrow file scope, or split the task.",
                context_bundle.estimated_tokens, context_bundle.hard_token_limit
            ));
        }
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
        for attempt_index in 0..8 {
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
                    self.provider_cost_usage.push(provider_attempt_cost_record(
                        &provider_task.id,
                        self.provider_name(),
                        self.model_name(),
                        attempt_index,
                        usage.as_ref(),
                        CostUsageOutcome::Success,
                    ));
                    self.provider_telemetry.record_success(
                        request_started.elapsed(),
                        events.len(),
                        usage,
                    );
                    events
                }
                Err(err) => {
                    self.provider_cost_usage.push(provider_attempt_cost_record(
                        &provider_task.id,
                        self.provider_name(),
                        self.model_name(),
                        attempt_index,
                        None,
                        CostUsageOutcome::Failure,
                    ));
                    self.provider_telemetry
                        .record_failure(request_started.elapsed(), &err);
                    if crate::provider_commands::is_request_too_large_provider_failure(&err)
                        && !retried_request_too_large
                    {
                        retried_request_too_large = true;
                        let retry_context = self.materialize_existing_context_bundle(
                            &context_bundle,
                            ContextBuildMode::RequestTooLargeRetry,
                        );
                        context_bundle = retry_context.bundle;
                        self.last_context_runtime_events = retry_context.events;
                        self.last_context_bundle = Some(context_bundle.clone());
                        if retry_context.hard_exceeded {
                            return Err(format!(
                                "context hard limit exceeded during request-too-large recovery: used {} tokens, hard limit {}. reduce input, narrow file scope, or split the task.",
                                context_bundle.estimated_tokens, context_bundle.hard_token_limit
                            ));
                        }
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
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        let mut call = call;
        let reasoning_content = call.input.remove(PROVIDER_REASONING_CONTENT_KEY);
        let tool_spec = self
            .tools
            .spec(&call.name)
            .ok_or_else(|| format!("Model requested unknown tool `{}`", call.name))?;
        self.store_entry(TranscriptEntry::ToolCall { call: call.clone() })?;
        let encoded_input = viden_types::encode_tool_input(&call.input);
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
            content: viden_types::encode_tool_input(&assistant_input),
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
                events.push(EngineEvent::ToolResult {
                    output: result.output.clone(),
                    success: result.success,
                    exit_code: result.exit_code,
                });
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
                events.push(EngineEvent::ToolResult {
                    output: result.output.clone(),
                    success: result.success,
                    exit_code: result.exit_code,
                });
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
        input: viden_types::ToolInput,
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
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
                | EngineEvent::Command(text) => Some(text),
                EngineEvent::ToolResult { output, .. } => Some(output),
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
        input: viden_types::ToolInput,
        approver: &mut F,
    ) -> Result<ToolResult, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
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

fn aggregate_model_usage(events: &[ModelEvent]) -> Option<viden_types::ModelUsage> {
    events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::Usage(usage) => Some(usage),
            _ => None,
        })
        .fold(None, |current, usage| {
            Some(match current {
                None => usage.clone(),
                Some(current) => viden_types::ModelUsage {
                    input_tokens: add_optional(current.input_tokens, usage.input_tokens),
                    output_tokens: add_optional(current.output_tokens, usage.output_tokens),
                    cached_input_tokens: add_optional(
                        current.cached_input_tokens,
                        usage.cached_input_tokens,
                    ),
                    retrieval_tokens: add_optional(
                        current.retrieval_tokens,
                        usage.retrieval_tokens,
                    ),
                    total_tokens: add_optional(current.total_tokens, usage.total_tokens),
                    cost_micro_usd: add_optional(current.cost_micro_usd, usage.cost_micro_usd),
                    actual_cost_micro_usd: add_optional(
                        current.actual_cost_micro_usd,
                        usage.actual_cost_micro_usd,
                    ),
                },
            })
        })
}

fn provider_attempt_cost_record(
    task_id: &str,
    provider_id: &str,
    model: &str,
    attempt_index: u32,
    usage: Option<&ModelUsage>,
    outcome: CostUsageOutcome,
) -> CostUsageRecord {
    let usage_id = format!("{task_id}-provider-attempt-{attempt_index}");
    let tokens = usage
        .map(|usage| TokenUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            retrieval_tokens: usage.retrieval_tokens,
            total_tokens: usage.total_tokens,
        })
        .unwrap_or(TokenUsage {
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            retrieval_tokens: None,
            total_tokens: None,
        });
    CostUsageRecord {
        usage_id: usage_id.clone(),
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        scopes: vec![
            CostScope::Request(usage_id),
            CostScope::AgentTask(task_id.to_string()),
            CostScope::Workflow("interactive".to_string()),
        ],
        estimate: usage.and_then(|usage| estimate_provider_cost(provider_id, model, usage)),
        actual_cost: usage
            .and_then(|usage| usage.actual_cost_micro_usd)
            .map(|micro_units| CostAmount {
                currency: "USD".to_string(),
                micro_units,
            }),
        tokens,
        attempt_index,
        outcome,
        recorded_at: Some(now_timestamp()),
    }
}

fn estimate_provider_cost(
    provider_id: &str,
    model: &str,
    usage: &ModelUsage,
) -> Option<CostEstimate> {
    let price = price_table(provider_id, model)?;
    let billable_input = usage
        .input_tokens
        .unwrap_or(0)
        .saturating_sub(usage.cached_input_tokens.unwrap_or(0));
    let cached = usage.cached_input_tokens.unwrap_or(0);
    let output = usage.output_tokens.unwrap_or(0);
    let micro_units = multiply_per_million(billable_input, price.input_micro_usd_per_million)
        .saturating_add(multiply_per_million(
            cached,
            price
                .cached_input_micro_usd_per_million
                .unwrap_or(price.input_micro_usd_per_million),
        ))
        .saturating_add(multiply_per_million(
            output,
            price.output_micro_usd_per_million,
        ));
    Some(CostEstimate {
        amount: CostAmount {
            currency: "USD".to_string(),
            micro_units,
        },
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        price_table_version: price.version.to_string(),
        estimated: true,
    })
}

fn multiply_per_million(tokens: u64, micro_usd_per_million: u64) -> u64 {
    let amount = u128::from(tokens).saturating_mul(u128::from(micro_usd_per_million)) / 1_000_000;
    amount.min(u128::from(u64::MAX)) as u64
}

#[derive(Clone, Copy)]
struct PriceRow {
    version: &'static str,
    input_micro_usd_per_million: u64,
    cached_input_micro_usd_per_million: Option<u64>,
    output_micro_usd_per_million: u64,
}

fn price_table(provider_id: &str, model: &str) -> Option<PriceRow> {
    match (provider_id, model) {
        ("sequence", "test-model") => Some(PriceRow {
            version: "test-2026-07-18",
            input_micro_usd_per_million: 1_000_000,
            cached_input_micro_usd_per_million: Some(100_000),
            output_micro_usd_per_million: 2_000_000,
        }),
        ("deepseek", "deepseek-v4-flash" | "deepseek-chat") => Some(PriceRow {
            version: "deepseek-2026-07-18",
            input_micro_usd_per_million: 140_000,
            cached_input_micro_usd_per_million: Some(14_000),
            output_micro_usd_per_million: 280_000,
        }),
        ("deepseek", "deepseek-v4-pro" | "deepseek-reasoner") => Some(PriceRow {
            version: "deepseek-2026-07-18",
            input_micro_usd_per_million: 550_000,
            cached_input_micro_usd_per_million: Some(55_000),
            output_micro_usd_per_million: 2_190_000,
        }),
        _ => None,
    }
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
            "Viden compacted provider request after a request-too-large error. The full transcript remains available in local audit storage.",
        ));
    }
    request.extend(recent_plain_history(history, limits));
    request.push(context_message);
    request.extend(
        live_turn
            .iter()
            .map(|message| compact_live_turn_message(message, limits)),
    );
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
        "Viden compacted transcript summary\nOmitted durable messages: {omitted}\nRecent omitted facts:\n{lines}\n\nThe full transcript remains in Viden storage; this summary is only the provider request view."
    ))
}

fn compact_live_turn_message(message: &Message, limits: ProviderRequestLimits) -> Message {
    let mut compacted = message.clone();
    compacted.content = compact_chars(&compacted.content, limits.tail_message_char_limit);
    compacted
}

fn fit_provider_request_budget(
    mut messages: Vec<Message>,
    request_char_budget: usize,
) -> Vec<Message> {
    while total_message_chars(&messages) > request_char_budget && messages.len() > 2 {
        let removable_recent_index = messages
            .iter()
            .position(|message| message.content.contains("Viden ContextBundle"))
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
        .contains("Viden compacted transcript summary")
        || message.content.contains("Viden ContextBundle")
        || message
            .content
            .contains("Viden compacted provider request after a request-too-large error")
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
        "{start}\n...[Viden compacted {} chars for provider request budget]...\n{end}",
        input.chars().count().saturating_sub(keep)
    )
}

fn one_line(input: &str, max_chars: usize) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    compact_chars(&collapsed, max_chars)
}
