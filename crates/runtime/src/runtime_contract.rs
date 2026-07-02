use std::fs;

use crate::{EngineEvent, ProviderTelemetry, SessionEngine};
use viden_config::ProviderConfigUpdate;
use viden_types::{
    AgentLaneRecord, ApprovalRequestView, ApprovalResponse, EvidenceView, PermissionLevel,
    PermissionMode, PermissionPrompt, ProviderHealthView, QueuedInputView, RuntimeCommand,
    RuntimeEvent, RuntimeEventKind, RuntimeSnapshot, RuntimeViewState, TokenCostView, ToolCallId,
    fresh_id, now_timestamp, truncate_for_preview,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueuedRuntimeInput {
    id: String,
    content: String,
    created_at: u64,
}

impl QueuedRuntimeInput {
    fn new(content: String) -> Self {
        Self {
            id: fresh_id("queued"),
            content,
            created_at: now_timestamp(),
        }
    }

    fn view(&self) -> QueuedInputView {
        QueuedInputView {
            id: self.id.clone(),
            content_preview: truncate_for_preview(&self.content, 500),
            created_at: Some(self.created_at),
        }
    }
}

impl SessionEngine {
    pub fn runtime_snapshot(&self) -> RuntimeSnapshot {
        self.runtime_snapshot.clone()
    }

    pub fn runtime_view_state(&self) -> RuntimeViewState {
        let mut view = RuntimeViewState::new(self.runtime_snapshot());
        for event in self.runtime_state_events() {
            view.apply_event(&event);
        }
        view
    }

    pub fn runtime_events_for_engine_events(&self, events: &[EngineEvent]) -> Vec<RuntimeEvent> {
        let mut out = self.runtime_state_events();
        let mut last_tool: Option<(ToolCallId, String)> = None;

        for event in events {
            let sequence = next_sequence(&out);
            match event {
                EngineEvent::System(text) => {
                    out.push(RuntimeEvent::new(
                        sequence,
                        RuntimeEventKind::EvidenceRecorded {
                            evidence: EvidenceView {
                                id: format!("system-{sequence}"),
                                kind: "system".to_string(),
                                summary: truncate_for_preview(text, 500),
                                path: None,
                                source: Some("engine".to_string()),
                                timestamp: None,
                            },
                        },
                    ));
                }
                EngineEvent::Assistant(content) => {
                    out.push(RuntimeEvent::new(
                        sequence,
                        RuntimeEventKind::AssistantDelta {
                            message_id: format!("assistant-{sequence}"),
                            task_id: None,
                            content: content.clone(),
                        },
                    ));
                }
                EngineEvent::ToolCall(text) => {
                    let (name, input_preview) = parse_legacy_tool_call(text);
                    let tool_call_id = format!("tool-event-{sequence}");
                    last_tool = Some((tool_call_id.clone(), name.clone()));
                    out.push(RuntimeEvent::new(
                        sequence,
                        RuntimeEventKind::ToolCallStarted {
                            tool_call_id,
                            name,
                            input_preview,
                        },
                    ));
                }
                EngineEvent::ToolResult(text) => {
                    let (tool_call_id, name) = last_tool
                        .take()
                        .unwrap_or_else(|| (format!("tool-event-{sequence}"), "tool".to_string()));
                    out.push(RuntimeEvent::new(
                        sequence,
                        RuntimeEventKind::ToolCallFinished {
                            tool_call_id,
                            name,
                            success: legacy_tool_result_success(text),
                            exit_code: None,
                            evidence: Some(EvidenceView {
                                id: format!("tool-result-{sequence}"),
                                kind: "tool_result".to_string(),
                                summary: truncate_for_preview(text, 500),
                                path: None,
                                source: Some("engine".to_string()),
                                timestamp: None,
                            }),
                        },
                    ));
                }
                EngineEvent::Command(text) => {
                    out.push(RuntimeEvent::new(
                        sequence,
                        RuntimeEventKind::EvidenceRecorded {
                            evidence: EvidenceView {
                                id: format!("command-{sequence}"),
                                kind: "command".to_string(),
                                summary: truncate_for_preview(text, 500),
                                path: None,
                                source: Some("engine".to_string()),
                                timestamp: None,
                            },
                        },
                    ));
                }
            }
        }

        out
    }

    pub fn handle_runtime_command<F>(
        &mut self,
        command_id: impl Into<String>,
        command: RuntimeCommand,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(PermissionPrompt) -> ApprovalResponse,
    {
        let command_id = command_id.into();
        let accepted = RuntimeEvent::new(
            1,
            RuntimeEventKind::CommandAccepted {
                command_id: command_id.clone(),
                command: command.clone(),
            },
        );

        let mut events = vec![accepted];
        match command {
            RuntimeCommand::SubmitUserInput { content } => {
                append_resequenced(
                    &mut events,
                    self.process_runtime_input_with_approval(&content, approver)?,
                );
            }
            RuntimeCommand::QueueFollowUp { content } => {
                let queued = QueuedRuntimeInput::new(content);
                let input = queued.view();
                self.queued_runtime_inputs.push(queued);
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    RuntimeEventKind::InputQueued { input },
                ));
            }
            RuntimeCommand::SetWorkMode { mode } => {
                self.set_work_mode(mode)?;
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    RuntimeEventKind::SnapshotUpdated {
                        snapshot: self.runtime_snapshot(),
                    },
                ));
            }
            RuntimeCommand::SetPermissionLevel { level } => {
                self.set_permission_mode(permission_mode_for_level(level))?;
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    RuntimeEventKind::SnapshotUpdated {
                        snapshot: self.runtime_snapshot(),
                    },
                ));
            }
            RuntimeCommand::SelectModel { provider_id, model } => {
                if provider_id != self.provider_name() {
                    return Ok(vec![command_rejected(
                        command_id,
                        format!(
                            "provider `{provider_id}` is not the active provider `{}`",
                            self.provider_name()
                        ),
                    )]);
                }
                self.provider.set_model(model);
                self.runtime_snapshot.model_label = self.provider.model().to_string();
                self.persist_meta("model", self.provider.model())?;
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    RuntimeEventKind::SnapshotUpdated {
                        snapshot: self.runtime_snapshot(),
                    },
                ));
            }
            RuntimeCommand::ConfigureProvider {
                provider_id,
                api_key_env,
                endpoint,
                default_model,
            } => match self.save_provider_config_update(
                &provider_id,
                ProviderConfigUpdate {
                    api_base: endpoint.clone(),
                    api_key_env: api_key_env.clone(),
                    default_model: default_model.clone(),
                    ..ProviderConfigUpdate::default()
                },
                provider_config_summary(&api_key_env, &endpoint, &default_model),
            ) {
                Ok(output) => push_evidence_event(&mut events, "provider_config", output),
                Err(err) => return Ok(vec![command_rejected(command_id, err)]),
            },
            RuntimeCommand::ActivateModel { provider_id, model } => {
                match self.add_provider_model(&provider_id, &model) {
                    Ok(output) => push_evidence_event(&mut events, "provider_model", output),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::DeactivateModel { provider_id, model } => {
                match self.remove_provider_model(&provider_id, &model) {
                    Ok(output) => push_evidence_event(&mut events, "provider_model", output),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::CancelActiveTurn | RuntimeCommand::RespondToApproval { .. } => {
                return Ok(vec![command_rejected(
                    command_id,
                    "runtime command is declared but not implemented in core yet".to_string(),
                )]);
            }
        }

        Ok(events)
    }

    pub fn process_runtime_input_with_approval<F>(
        &mut self,
        input: &str,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(PermissionPrompt) -> ApprovalResponse,
    {
        let mut approval_events = Vec::new();
        let mut approval_counter = 0_u64;
        let mut capturing_approver = |prompt: PermissionPrompt| {
            approval_counter += 1;
            let request_id = format!("approval-{approval_counter}");
            approval_events.push(RuntimeEvent::new(
                approval_counter,
                RuntimeEventKind::ApprovalRequested {
                    approval: approval_request_view(&request_id, &prompt),
                },
            ));
            let response = approver(prompt);
            approval_events.push(RuntimeEvent::new(
                approval_counter + 1,
                RuntimeEventKind::ApprovalResolved {
                    request_id,
                    approved: response.approved,
                },
            ));
            response
        };
        let engine_events = self.process_input_with_approval(input, &mut capturing_approver)?;
        let runtime_events = self.runtime_events_for_engine_events(&engine_events);
        Ok(merge_approval_events(runtime_events, approval_events))
    }

    fn runtime_state_events(&self) -> Vec<RuntimeEvent> {
        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::SnapshotUpdated {
                snapshot: self.runtime_snapshot(),
            },
        )];
        if let Some(context) = &self.last_context_bundle {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::ContextUpdated {
                    context: context.clone(),
                },
            ));
        }
        events.push(RuntimeEvent::new(
            next_sequence(&events),
            RuntimeEventKind::ProviderHealthUpdated {
                provider: provider_health_view(
                    self.provider_name(),
                    self.model_name(),
                    &self.provider_telemetry,
                ),
            },
        ));
        if let Some(cost) = token_cost_view(&self.provider_telemetry) {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TokenCostUpdated { cost },
            ));
        }
        for input in &self.queued_runtime_inputs {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::InputQueued {
                    input: input.view(),
                },
            ));
        }
        for lane in load_runtime_lanes(
            &self
                .runtime_snapshot
                .cwd
                .join(".robocode")
                .join("lanes.tsv"),
        ) {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::LaneUpdated { lane },
            ));
        }
        for task in self.agent_task_snapshot() {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }
        events
    }
}

fn push_evidence_event(events: &mut Vec<RuntimeEvent>, kind: &str, summary: String) {
    let sequence = next_sequence(events);
    events.push(RuntimeEvent::new(
        sequence,
        RuntimeEventKind::EvidenceRecorded {
            evidence: runtime_evidence(sequence, kind, summary),
        },
    ));
}

fn runtime_evidence(sequence: u64, kind: &str, summary: String) -> EvidenceView {
    EvidenceView {
        id: format!("{kind}-{sequence}"),
        kind: kind.to_string(),
        summary: truncate_for_preview(&summary, 500),
        path: None,
        source: Some("runtime_command".to_string()),
        timestamp: None,
    }
}

fn provider_config_summary(
    api_key_env: &Option<String>,
    endpoint: &Option<String>,
    default_model: &Option<String>,
) -> String {
    let mut fields = Vec::new();
    if let Some(api_key_env) = api_key_env {
        fields.push(format!("key env {api_key_env}"));
    }
    if let Some(endpoint) = endpoint {
        fields.push(format!("endpoint {endpoint}"));
    }
    if let Some(default_model) = default_model {
        fields.push(format!("default model {default_model}"));
    }
    if fields.is_empty() {
        "no fields".to_string()
    } else {
        fields.join(", ")
    }
}

fn approval_request_view(request_id: &str, prompt: &PermissionPrompt) -> ApprovalRequestView {
    ApprovalRequestView {
        id: request_id.to_string(),
        tool_name: prompt.tool_name.clone(),
        title: format!("Approve {}", prompt.tool_name),
        message: prompt.message.clone(),
        input_preview: prompt.input_preview.clone(),
        is_mutating: true,
        reason: Some(prompt.message.clone()),
    }
}

fn command_rejected(command_id: String, reason: String) -> RuntimeEvent {
    RuntimeEvent::new(1, RuntimeEventKind::CommandRejected { command_id, reason })
}

fn permission_mode_for_level(level: PermissionLevel) -> PermissionMode {
    match level {
        PermissionLevel::Ask => PermissionMode::Default,
        PermissionLevel::AutoEdit | PermissionLevel::Auto => PermissionMode::AcceptEdits,
        PermissionLevel::ReadOnly => PermissionMode::Plan,
        PermissionLevel::FullAccess => PermissionMode::BypassPermissions,
    }
}

fn next_sequence(events: &[RuntimeEvent]) -> u64 {
    events.last().map(|event| event.sequence + 1).unwrap_or(1)
}

fn append_resequenced(target: &mut Vec<RuntimeEvent>, source: Vec<RuntimeEvent>) {
    for mut event in source {
        event.sequence = next_sequence(target);
        target.push(event);
    }
}

fn merge_approval_events(
    runtime_events: Vec<RuntimeEvent>,
    approval_events: Vec<RuntimeEvent>,
) -> Vec<RuntimeEvent> {
    if approval_events.is_empty() {
        return runtime_events;
    }

    let mut merged = Vec::with_capacity(runtime_events.len() + approval_events.len());
    let mut approvals = Some(approval_events);
    for event in runtime_events {
        if approvals.is_some()
            && matches!(event.kind, RuntimeEventKind::ToolCallFinished { .. })
            && let Some(approval_events) = approvals.take()
        {
            append_resequenced(&mut merged, approval_events);
        }
        append_resequenced(&mut merged, vec![event]);
    }
    if let Some(approval_events) = approvals {
        append_resequenced(&mut merged, approval_events);
    }
    merged
}

fn provider_health_view(
    provider_id: &str,
    model: &str,
    telemetry: &ProviderTelemetry,
) -> ProviderHealthView {
    let status = if telemetry.failure_count > telemetry.success_count {
        "degraded"
    } else {
        "healthy"
    };
    ProviderHealthView {
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        status: status.to_string(),
        request_count: telemetry.request_count,
        error_count: telemetry.failure_count,
        last_latency_ms: telemetry.last_latency_ms.and_then(clamp_u128_to_u64),
        average_latency_ms: telemetry.average_latency_ms.and_then(clamp_u128_to_u64),
        tokens_per_second: telemetry.last_tokens_per_second,
    }
}

fn token_cost_view(telemetry: &ProviderTelemetry) -> Option<TokenCostView> {
    if telemetry.total_tokens == 0 && telemetry.total_cost_micro_usd.is_none() {
        return None;
    }
    Some(TokenCostView {
        input_tokens: telemetry.total_input_tokens,
        output_tokens: telemetry.total_output_tokens,
        total_tokens: telemetry.total_tokens,
        cost_micro_usd: telemetry.total_cost_micro_usd,
    })
}

fn clamp_u128_to_u64(value: u128) -> Option<u64> {
    Some(value.min(u128::from(u64::MAX)) as u64)
}

fn parse_legacy_tool_call(text: &str) -> (String, String) {
    let trimmed = text.trim();
    match trimmed.split_once(' ') {
        Some((name, input)) => (name.to_string(), input.to_string()),
        None => (trimmed.to_string(), String::new()),
    }
}

fn legacy_tool_result_success(text: &str) -> bool {
    let lower = text.to_lowercase();
    !lower.contains("failed") && !lower.contains("error") && !lower.contains("denied")
}

fn load_runtime_lanes(path: &std::path::Path) -> Vec<AgentLaneRecord> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content.lines().filter_map(parse_runtime_lane).collect()
}

fn parse_runtime_lane(line: &str) -> Option<AgentLaneRecord> {
    let fields = line
        .split('\t')
        .map(unescape_runtime_tsv)
        .collect::<Vec<_>>();
    if fields.len() != 5 && fields.len() != 7 && fields.len() != 8 {
        return None;
    }
    let id = fields[0].clone();
    let tool = fields[1].clone();
    let title = fields[2].clone();
    let status = fields[3].clone();
    let target = fields[4].clone();
    let summary = fields
        .get(6)
        .filter(|summary| !summary.trim().is_empty())
        .cloned()
        .unwrap_or(title);
    Some(AgentLaneRecord {
        id: id.clone(),
        task_id: id,
        agent: tool,
        screen: "lane".to_string(),
        transport: target,
        status,
        summary,
        evidence: Vec::new(),
    })
}

fn unescape_runtime_tsv(value: &str) -> String {
    value.replace("\\t", "\t").replace("\\n", "\n")
}
