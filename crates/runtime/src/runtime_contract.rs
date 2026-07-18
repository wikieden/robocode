use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

use crate::agent_commands::{tracked_agent_job_runtime_events, tracked_agent_job_tasks};
use crate::context_bundle::{ContextBuildMode, redact_context_summary_for_event};
use crate::lsp_tools::render_lsp_diagnostics;
use crate::{EngineEvent, ProviderTelemetry, SessionEngine};
use viden_config::ProviderConfigUpdate;
use viden_lsp::SemanticProvider;
use viden_permissions::{PermissionContext, PermissionEngine};
use viden_provider::ModelRequestControl;
use viden_types::{
    AgentDagRecord, AgentDagStatus, AgentDagTaskSpec, AgentLaneRecord, AgentNextAction, AgentRole,
    AgentTaskRecord, AgentTaskStatus, ApprovalRequestView, ApprovalResponse, ContextSourceRecord,
    EvidenceView, MergeGateRecord, MergeGateStatus, PermissionBehavior, PermissionLevel,
    PermissionMode, PermissionPrompt, PermissionRule, PermissionRuleSource, PermissionRuleValue,
    ProviderHealthView, QueuedInputView, RuntimeCommand, RuntimeErrorView, RuntimeEvent,
    RuntimeEventKind, RuntimeSnapshot, RuntimeViewState, TokenCostView, ToolCallId, WorkMode,
    fresh_id, now_timestamp, truncate_for_preview,
};
use viden_workflows::stores::WorkflowAgentEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueuedRuntimeInput {
    id: String,
    content: String,
    created_at: u64,
}

#[derive(Debug, Clone)]
struct RuntimePermissionScope {
    work_mode: WorkMode,
    permission_mode: PermissionMode,
    permission_level: PermissionLevel,
    permission_context: PermissionContext,
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
                                metadata: None,
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
                EngineEvent::ToolResult {
                    output,
                    success,
                    exit_code,
                } => {
                    let (tool_call_id, name) = last_tool
                        .take()
                        .unwrap_or_else(|| (format!("tool-event-{sequence}"), "tool".to_string()));
                    out.push(RuntimeEvent::new(
                        sequence,
                        RuntimeEventKind::ToolCallFinished {
                            tool_call_id,
                            name,
                            success: *success,
                            exit_code: *exit_code,
                            evidence: Some(EvidenceView {
                                id: format!("tool-result-{sequence}"),
                                kind: "tool_result".to_string(),
                                summary: truncate_for_preview(output, 500),
                                path: None,
                                source: Some("engine".to_string()),
                                metadata: None,
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
                                metadata: None,
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
                command: redacted_runtime_command_for_event(&command),
            },
        );

        let mut events = vec![accepted];
        match command {
            RuntimeCommand::SubmitUserInput { content } => {
                match self.process_runtime_input_with_approval(&content, approver) {
                    Ok(input_events) => append_resequenced(&mut events, input_events),
                    Err(err) if err.contains("context hard limit") => {
                        append_resequenced(&mut events, self.runtime_state_events());
                        events.push(RuntimeEvent::new(
                            next_sequence(&events),
                            RuntimeEventKind::CommandRejected {
                                command_id,
                                reason: err,
                            },
                        ));
                        return Ok(events);
                    }
                    Err(err) => return Err(err),
                }
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
            RuntimeCommand::StartAgentDag { goal, tasks } => {
                append_resequenced(&mut events, self.start_agent_dag(goal, tasks)?);
            }
            RuntimeCommand::StartAgentTask { task_id } => {
                append_resequenced(&mut events, self.run_agent_task(&task_id, approver)?);
            }
            RuntimeCommand::CancelAgentTask { task_id } => {
                append_resequenced(&mut events, self.cancel_agent_task(&task_id)?);
            }
            RuntimeCommand::AcceptMergeGate { gate_id, decision } => {
                match self.decide_merge_gate(
                    &gate_id,
                    MergeGateStatus::Accepted,
                    decision.unwrap_or_else(|| "accepted".to_string()),
                    "merge_gate_accepted",
                    "decision",
                ) {
                    Ok(decision_events) => append_resequenced(&mut events, decision_events),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::RejectMergeGate { gate_id, reason } => {
                match self.decide_merge_gate(
                    &gate_id,
                    MergeGateStatus::NeedsChanges,
                    reason,
                    "merge_gate_rejected",
                    "reason",
                ) {
                    Ok(decision_events) => append_resequenced(&mut events, decision_events),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::RecordAgentEvidence {
                gate_id,
                evidence_id,
                kind,
                summary,
                path,
                source,
            } => {
                let record_result =
                    self.record_agent_evidence(&gate_id, evidence_id, kind, summary, path, source);
                match record_result {
                    Ok(evidence_events) => append_resequenced(&mut events, evidence_events),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::AcceptAgentArtifact {
                gate_id,
                evidence_id,
                decision,
            } => {
                match self.accept_agent_artifact(
                    &gate_id,
                    evidence_id,
                    decision.unwrap_or_else(|| "artifact accepted".to_string()),
                ) {
                    Ok(artifact_events) => append_resequenced(&mut events, artifact_events),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::RejectAgentArtifact {
                gate_id,
                evidence_id,
                reason,
            } => match self.reject_agent_artifact(&gate_id, &evidence_id, reason) {
                Ok(artifact_events) => append_resequenced(&mut events, artifact_events),
                Err(err) => return Ok(vec![command_rejected(command_id, err)]),
            },
            RuntimeCommand::MergeAgentPatch { gate_id, decision } => {
                match self.merge_agent_patch(
                    &gate_id,
                    decision.unwrap_or_else(|| "patch merged".to_string()),
                ) {
                    Ok(merge_events) => append_resequenced(&mut events, merge_events),
                    Err(err) => return Ok(vec![command_rejected(command_id, err)]),
                }
            }
            RuntimeCommand::CancelActiveTurn
            | RuntimeCommand::RespondToApproval { .. }
            | RuntimeCommand::RetrieveContext { .. } => {
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
        self.process_runtime_input_with_approval_and_control(
            input,
            approver,
            &ModelRequestControl::new(),
        )
    }

    pub(crate) fn process_runtime_input_with_approval_and_control<F>(
        &mut self,
        input: &str,
        approver: &mut F,
        control: &ModelRequestControl,
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
        let engine_events =
            self.process_input_with_approval_and_control(input, &mut capturing_approver, control)?;
        let runtime_events = self.runtime_events_for_engine_events(&engine_events);
        Ok(merge_approval_events(runtime_events, approval_events))
    }

    pub(crate) fn process_runtime_input_with_built_context_bundle_and_control<F>(
        &mut self,
        input: &str,
        approver: &mut F,
        control: &ModelRequestControl,
        built_context_bundle: crate::context_bundle::BuiltContextBundle,
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
        let engine_events = self.process_input_with_built_context_bundle_and_control(
            input,
            &mut capturing_approver,
            control,
            built_context_bundle,
        )?;
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
            for event in &self.last_context_runtime_events {
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    event.kind.clone(),
                ));
            }
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
        for dag in &self.runtime_agent_dags {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::AgentDagUpdated { dag: dag.clone() },
            ));
        }
        for lane in load_runtime_lanes(&self.runtime_snapshot.cwd.join(".viden").join("lanes.tsv"))
        {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::LaneUpdated { lane },
            ));
        }
        for task in tracked_agent_job_tasks(&self.runtime_snapshot.cwd) {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }
        for event in tracked_agent_job_runtime_events(&self.runtime_snapshot.cwd) {
            let RuntimeEvent {
                timestamp, kind, ..
            } = event;
            events.push(RuntimeEvent::with_timestamp(
                next_sequence(&events),
                timestamp,
                kind,
            ));
        }
        for task in self.agent_task_snapshot() {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated {
                    task: redacted_task_for_event(task),
                },
            ));
        }
        for evidence in &self.runtime_evidence {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::EvidenceRecorded {
                    evidence: evidence.clone(),
                },
            ));
        }
        for gate in &self.runtime_merge_gates {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::MergeGateUpdated { gate: gate.clone() },
            ));
        }
        events
    }

    fn start_agent_dag(
        &mut self,
        goal: String,
        tasks: Vec<AgentDagTaskSpec>,
    ) -> Result<Vec<RuntimeEvent>, String> {
        if tasks.is_empty() {
            return Err("agent DAG requires at least one task".to_string());
        }
        validate_agent_dag_tasks(&tasks)?;
        let now = now_timestamp();
        let dag = AgentDagRecord {
            dag_id: fresh_id("dag"),
            goal,
            status: AgentDagStatus::Active,
            tasks,
            created_at: Some(now),
            updated_at: Some(now),
        };

        self.persist_agent_event(
            &dag.dag_id,
            None,
            "agent_dag_created",
            &[("goal", &dag.goal)],
        )?;

        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::AgentDagUpdated { dag: dag.clone() },
        )];
        for spec in &dag.tasks {
            self.persist_agent_event(
                &dag.dag_id,
                Some(&spec.task_id),
                "agent_task_queued",
                &[
                    ("role", spec.role.as_str()),
                    ("title", spec.title.as_str()),
                    ("permission_policy", spec.permission_policy.as_str()),
                ],
            )?;
            let task = agent_task_record_from_spec(self, spec, now);
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
            // Create the merge gate with the task so every agent output has a
            // review/evidence target before implementation work starts.
            let gate = MergeGateRecord {
                gate_id: format!("gate-{}", spec.task_id),
                task_id: spec.task_id.clone(),
                status: MergeGateStatus::Proposed,
                required_evidence: spec.required_evidence.clone(),
                evidence_ids: Vec::new(),
                decision: None,
                updated_at: Some(now),
            };
            self.runtime_merge_gates.push(gate.clone());
            self.persist_agent_event(
                &dag.dag_id,
                Some(&spec.task_id),
                "merge_gate_proposed",
                &[("gate_id", gate.gate_id.as_str())],
            )?;
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::MergeGateUpdated { gate },
            ));
        }
        self.runtime_agent_dags.push(dag);
        Ok(events)
    }

    fn update_agent_task_status(
        &mut self,
        task_id: &str,
        status: AgentTaskStatus,
        activity: &str,
        progress: u8,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        else {
            return Err(format!("agent task `{task_id}` does not exist"));
        };
        task.status = status.as_str().to_string();
        task.activity = activity.to_string();
        task.progress = progress;
        task.updated_at = Some(u128::from(now_timestamp()) * 1000);
        self.upsert_agent_task(task.clone());
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::TaskUpdated { task },
        )])
    }

    fn cancel_agent_task(&mut self, task_id: &str) -> Result<Vec<RuntimeEvent>, String> {
        let dag_id = self
            .runtime_agent_dags
            .iter()
            .find(|dag| dag.tasks.iter().any(|task| task.task_id == task_id))
            .map(|dag| dag.dag_id.clone())
            .ok_or_else(|| format!("agent DAG for task `{task_id}` does not exist"))?;
        let events = self.update_agent_task_status(
            task_id,
            AgentTaskStatus::Cancelled,
            "cancelled by operator",
            100,
        )?;
        self.persist_agent_event(
            &dag_id,
            Some(task_id),
            "agent_task_cancelled",
            &[("reason", "cancelled by operator")],
        )?;
        Ok(events)
    }

    fn update_agent_task_failure(
        &mut self,
        task_id: &str,
        activity: &str,
        failure_class: &str,
        recovery_suggestion: &str,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        else {
            return Err(format!("agent task `{task_id}` does not exist"));
        };
        task.status = AgentTaskStatus::Failed.as_str().to_string();
        task.activity = activity.to_string();
        task.progress = 100;
        task.result = Some(format!("failed:{failure_class}"));
        task.updated_at = Some(u128::from(now_timestamp()) * 1000);
        task.next_action = Some(AgentNextAction {
            label: "retry agent task".to_string(),
            command: Some(format!("/agent start {task_id}")),
            reason: Some(format!("{failure_class}: {recovery_suggestion}")),
        });
        self.upsert_agent_task(task.clone());
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::TaskUpdated { task },
        )])
    }

    fn run_agent_task<F>(
        &mut self,
        task_id: &str,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(PermissionPrompt) -> ApprovalResponse,
    {
        self.run_agent_task_with_control(task_id, approver, &ModelRequestControl::new())
    }

    pub(crate) fn run_agent_task_with_control<F>(
        &mut self,
        task_id: &str,
        approver: &mut F,
        control: &ModelRequestControl,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(PermissionPrompt) -> ApprovalResponse,
    {
        let (dag_id, spec) = self
            .runtime_agent_dags
            .iter()
            .find_map(|dag| {
                dag.tasks
                    .iter()
                    .find(|task| task.task_id == task_id)
                    .cloned()
                    .map(|task| (dag.dag_id.clone(), task))
            })
            .ok_or_else(|| format!("agent task `{task_id}` does not exist"))?;

        if let Some(blocked_events) = self.blocked_by_unfinished_dependency(&dag_id, &spec)? {
            return Ok(blocked_events);
        }

        let mut events = self.update_agent_task_status(
            task_id,
            AgentTaskStatus::Running,
            "running supervised role task",
            10,
        )?;
        self.persist_agent_event(
            &dag_id,
            Some(task_id),
            "agent_task_started",
            &[
                ("role", spec.role.as_str()),
                ("permission_policy", spec.permission_policy.as_str()),
            ],
        )?;
        let prompt = agent_task_prompt(&spec);
        let seeded_context = self.agent_context_bundle(&spec, &prompt);
        let built_context =
            self.materialize_existing_context_bundle(&seeded_context, ContextBuildMode::Normal);
        let context = built_context.bundle.clone();
        self.last_context_bundle = Some(context.clone());
        self.last_context_runtime_events = built_context.events.clone();
        if built_context.hard_exceeded {
            append_resequenced(&mut events, built_context.events);
            append_resequenced(
                &mut events,
                self.update_agent_task_failure(
                    task_id,
                    "context hard limit exceeded before provider request",
                    "context_hard_limit",
                    "reduce input, narrow file scope, or split the task",
                )?,
            );
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::Error {
                    error: RuntimeErrorView {
                        message: format!(
                            "context hard limit exceeded for agent task `{task_id}` before provider request"
                        ),
                        recoverable: true,
                        hint: Some("reduce input, narrow file scope, or split the task".to_string()),
                    },
                },
            ));
            return Ok(events);
        }
        let previous_permissions = self.apply_agent_permission_policy(&spec);
        let provider_result = self.process_runtime_input_with_built_context_bundle_and_control(
            &prompt,
            approver,
            control,
            built_context,
        );
        self.restore_agent_permission_policy(previous_permissions);
        let provider_events = match provider_result {
            Ok(events) => events,
            Err(err) => {
                let cancelled = err.to_lowercase().contains("cancel");
                let activity = if cancelled {
                    "cancelled during supervised role task".to_string()
                } else {
                    format!("provider error: {}", truncate_for_preview(&err, 200))
                };
                let mut error_hint = "agent task stopped before evidence was accepted".to_string();
                if cancelled {
                    append_resequenced(
                        &mut events,
                        self.update_agent_task_status(
                            task_id,
                            AgentTaskStatus::Cancelled,
                            &activity,
                            100,
                        )?,
                    );
                    self.persist_agent_event(
                        &dag_id,
                        Some(task_id),
                        "agent_task_cancelled",
                        &[("role", spec.role.as_str()), ("error", err.as_str())],
                    )?;
                } else {
                    let failure = classify_agent_task_failure(&err);
                    error_hint = failure.recovery_suggestion.to_string();
                    append_resequenced(
                        &mut events,
                        self.update_agent_task_failure(
                            task_id,
                            &activity,
                            failure.class,
                            failure.recovery_suggestion,
                        )?,
                    );
                    self.persist_agent_event(
                        &dag_id,
                        Some(task_id),
                        "agent_task_failed",
                        &[
                            ("role", spec.role.as_str()),
                            ("error", err.as_str()),
                            ("failure_class", failure.class),
                            ("recovery_suggestion", failure.recovery_suggestion),
                        ],
                    )?;
                }
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    RuntimeEventKind::Error {
                        error: RuntimeErrorView {
                            message: err,
                            recoverable: true,
                            hint: Some(error_hint),
                        },
                    },
                ));
                events.push(RuntimeEvent::new(
                    next_sequence(&events),
                    RuntimeEventKind::SnapshotUpdated {
                        snapshot: self.runtime_snapshot(),
                    },
                ));
                return Ok(events);
            }
        };
        let assistant_output = assistant_output_from_events(&provider_events);
        append_resequenced(&mut events, provider_events);
        events.push(RuntimeEvent::new(
            next_sequence(&events),
            RuntimeEventKind::SnapshotUpdated {
                snapshot: self.runtime_snapshot(),
            },
        ));
        for event in &self.last_context_runtime_events {
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                event.kind.clone(),
            ));
        }
        events.push(RuntimeEvent::new(
            next_sequence(&events),
            RuntimeEventKind::ContextUpdated { context },
        ));

        let evidence_kind = evidence_kind_for_role(spec.role);
        let evidence_id = format!("evidence-{task_id}-{evidence_kind}");
        let summary = if assistant_output.trim().is_empty() {
            format!("{} completed without assistant text", spec.role)
        } else {
            assistant_output.clone()
        };
        let evidence = EvidenceView {
            id: evidence_id.clone(),
            kind: evidence_kind.to_string(),
            summary: summary.clone(),
            path: None,
            source: Some(spec.role.as_str().to_string()),
            metadata: None,
            timestamp: Some(now_timestamp()),
        };
        self.upsert_runtime_evidence(evidence.clone());
        events.push(RuntimeEvent::new(
            next_sequence(&events),
            RuntimeEventKind::EvidenceRecorded { evidence },
        ));

        let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        else {
            return Err(format!("agent task `{task_id}` does not exist"));
        };
        task.status = AgentTaskStatus::Done.as_str().to_string();
        task.activity = "supervised role task complete".to_string();
        task.progress = 100;
        task.result = Some(truncate_for_preview(&summary, 500));
        task.evidence.push(evidence_id.clone());
        task.updated_at = Some(u128::from(now_timestamp()) * 1000);
        self.upsert_agent_task(task.clone());
        events.push(RuntimeEvent::new(
            next_sequence(&events),
            RuntimeEventKind::TaskUpdated { task },
        ));

        let runtime_evidence = self.runtime_evidence.clone();
        if let Some(gate) = self
            .runtime_merge_gates
            .iter_mut()
            .find(|gate| gate.task_id == task_id)
        {
            if !gate.evidence_ids.contains(&evidence_id) {
                gate.evidence_ids.push(evidence_id);
            }
            gate.status = reduce_merge_gate_status(gate, &runtime_evidence);
            gate.updated_at = Some(now_timestamp());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::MergeGateUpdated { gate: gate.clone() },
            ));
        }

        self.persist_agent_event(
            &dag_id,
            Some(task_id),
            "agent_task_completed",
            &[
                ("role", spec.role.as_str()),
                ("evidence_kind", evidence_kind),
                ("summary", summary.as_str()),
            ],
        )?;

        Ok(events)
    }

    fn apply_agent_permission_policy(&mut self, spec: &AgentDagTaskSpec) -> RuntimePermissionScope {
        let previous = RuntimePermissionScope {
            work_mode: self.runtime_snapshot.work_mode,
            permission_mode: self.runtime_snapshot.permission_mode,
            permission_level: self.runtime_snapshot.permission_level,
            permission_context: self.permissions.context_snapshot(),
        };
        let scoped_mode =
            agent_permission_mode_for_policy(self.permissions.mode(), &spec.permission_policy);
        self.permissions.set_mode(scoped_mode);
        apply_agent_role_permission_rules(&mut self.permissions, spec);
        self.runtime_snapshot.permission_mode = scoped_mode;
        self.runtime_snapshot.permission_level = PermissionLevel::from_legacy_mode(scoped_mode);
        if scoped_mode == PermissionMode::Plan {
            self.runtime_snapshot.work_mode = WorkMode::Plan;
        }
        previous
    }

    fn restore_agent_permission_policy(&mut self, previous: RuntimePermissionScope) {
        self.permissions
            .restore_context(previous.permission_context);
        self.runtime_snapshot.work_mode = previous.work_mode;
        self.runtime_snapshot.permission_mode = previous.permission_mode;
        self.runtime_snapshot.permission_level = previous.permission_level;
    }

    fn agent_context_bundle(
        &self,
        spec: &AgentDagTaskSpec,
        prompt: &str,
    ) -> viden_types::ContextBundleRecord {
        let mut bundle = self.build_main_context_bundle(prompt);
        bundle.bundle_id = format!("ctx-agent-{}", spec.task_id);
        bundle.task_id = spec.task_id.clone();
        bundle.policy = format!("agent-role-{}-priority-budget", spec.role.as_str());
        let mut agent_sources = vec![ContextSourceRecord {
            name: "agent-role".to_string(),
            kind: "agent-task".to_string(),
            priority: 100,
            estimated_tokens: 160,
            summary: format!("{}: {}", spec.role.as_str(), spec.objective),
            include_reason: "role objective pins the ContextBundle to the AgentTask".to_string(),
            handle_id: None,
            item_id: None,
            view_id: None,
            content_sha256: None,
            view_sha256: None,
            quality_id: None,
        }];
        agent_sources.push(role_guidance_context_source(spec.role));
        if !spec.file_scope.is_empty() {
            agent_sources.push(agent_file_scope_context_source(spec));
        }
        if let Some(source) = agent_selected_files_context_source(&self.cwd, spec) {
            agent_sources.push(source);
        }
        if let Some(source) = agent_selected_symbols_context_source(&self.cwd, spec) {
            agent_sources.push(source);
        }
        if let Some(source) = self.agent_lsp_diagnostics_context_source(spec) {
            agent_sources.push(source);
        }
        if !spec.required_evidence.is_empty() {
            agent_sources.push(agent_evidence_contract_context_source(spec));
        }
        for (index, source) in agent_sources.into_iter().enumerate() {
            bundle.sources.insert(index, source);
        }
        bundle.estimated_tokens = bundle
            .sources
            .iter()
            .map(|source| source.estimated_tokens)
            .sum();
        bundle
            .compaction_notes
            .push(format!("agent task {}", spec.task_id));
        bundle
    }

    fn agent_lsp_diagnostics_context_source(
        &self,
        spec: &AgentDagTaskSpec,
    ) -> Option<ContextSourceRecord> {
        let mut diagnostics = Vec::new();
        for file in select_role_specific_files(&self.cwd, spec)
            .into_iter()
            .filter(|file| is_lsp_context_candidate(file))
            .take(4)
        {
            let Ok(mut file_diagnostics) =
                self.lsp_runtime.diagnostics(&self.cwd, Path::new(&file))
            else {
                continue;
            };
            diagnostics.append(&mut file_diagnostics);
            if diagnostics.len() >= 16 {
                break;
            }
        }
        if diagnostics.is_empty() {
            return None;
        }
        diagnostics.truncate(16);
        let rendered = render_lsp_diagnostics(&self.cwd, &diagnostics);
        Some(ContextSourceRecord {
            name: "role-lsp-diagnostics".to_string(),
            kind: "lsp-diagnostics".to_string(),
            priority: 93,
            estimated_tokens: diagnostics.len().saturating_mul(64).min(960) as u64,
            summary: truncate_for_preview(
                &redact_context_summary_for_event(&self.cwd, &rendered),
                1_500,
            ),
            include_reason: format!(
                "{} role receives live diagnostics from selected scoped files",
                spec.role.as_str()
            ),
            handle_id: None,
            item_id: None,
            view_id: None,
            content_sha256: None,
            view_sha256: None,
            quality_id: None,
        })
    }

    fn blocked_by_unfinished_dependency(
        &mut self,
        dag_id: &str,
        spec: &AgentDagTaskSpec,
    ) -> Result<Option<Vec<RuntimeEvent>>, String> {
        let Some(blocking_dependency) = spec.dependencies.iter().find(|dependency| {
            self.runtime_tasks
                .iter()
                .find(|task| task.id == **dependency)
                .map(|task| {
                    !matches!(
                        AgentTaskStatus::parse(&task.status),
                        Some(
                            AgentTaskStatus::Done
                                | AgentTaskStatus::Applied
                                | AgentTaskStatus::Archived
                        )
                    )
                })
                .unwrap_or(true)
        }) else {
            return Ok(None);
        };

        let mut task = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == spec.task_id)
            .cloned()
            .ok_or_else(|| format!("agent task `{}` does not exist", spec.task_id))?;
        task.status = AgentTaskStatus::Blocked.as_str().to_string();
        task.activity = format!("waiting for dependency `{blocking_dependency}`");
        task.progress = 0;
        task.updated_at = Some(u128::from(now_timestamp()) * 1000);
        self.upsert_agent_task(task.clone());
        self.persist_agent_event(
            dag_id,
            Some(&spec.task_id),
            "agent_task_blocked",
            &[("dependency", blocking_dependency.as_str())],
        )?;
        Ok(Some(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::TaskUpdated { task },
        )]))
    }

    fn decide_merge_gate(
        &mut self,
        gate_id: &str,
        status: MergeGateStatus,
        decision: String,
        event_type: &str,
        payload_key: &str,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let gate_index = self
            .runtime_merge_gates
            .iter()
            .position(|gate| gate.gate_id == gate_id)
            .ok_or_else(|| format!("merge gate `{gate_id}` does not exist"))?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        let dag_id = self
            .runtime_agent_dags
            .iter()
            .find(|dag| dag.tasks.iter().any(|task| task.task_id == task_id))
            .map(|dag| dag.dag_id.clone())
            .ok_or_else(|| format!("agent DAG for task `{task_id}` does not exist"))?;
        if status == MergeGateStatus::Accepted
            && self.runtime_merge_gates[gate_index].evidence_ids.is_empty()
        {
            return Err(format!(
                "merge gate `{gate_id}` cannot be accepted without evidence"
            ));
        }

        let now = now_timestamp();
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.status = status;
        gate.decision = Some(decision.clone());
        gate.updated_at = Some(now);
        let gate = gate.clone();

        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::MergeGateUpdated { gate },
        )];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            task.decision = Some(decision.clone());
            task.updated_at = Some(u128::from(now) * 1000);
            if status == MergeGateStatus::NeedsChanges {
                task.status = AgentTaskStatus::NeedsInput.as_str().to_string();
                task.activity = format!("merge gate requested changes: {decision}");
                task.next_action = Some(AgentNextAction {
                    label: "revise task".to_string(),
                    command: Some(format!("/agent start {}", task.id)),
                    reason: Some("merge gate rejected the current evidence".to_string()),
                });
            } else {
                task.activity = format!("merge gate accepted: {decision}");
                task.next_action = None;
            }
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }

        self.persist_agent_event(
            &dag_id,
            Some(&task_id),
            event_type,
            &[("gate_id", gate_id), (payload_key, decision.as_str())],
        )?;
        Ok(events)
    }

    fn record_agent_evidence(
        &mut self,
        gate_id: &str,
        evidence_id: Option<String>,
        kind: String,
        summary: String,
        path: Option<String>,
        source: Option<String>,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let kind = normalize_evidence_kind(&kind);
        if kind.is_empty() {
            return Err("agent evidence kind cannot be empty".to_string());
        }
        let summary = summary.trim().to_string();
        if summary.is_empty() {
            return Err("agent evidence summary cannot be empty".to_string());
        }

        let gate_index = self
            .runtime_merge_gates
            .iter()
            .position(|gate| gate.gate_id == gate_id)
            .ok_or_else(|| format!("merge gate `{gate_id}` does not exist"))?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        let dag_id = self.dag_id_for_task(&task_id)?;
        let evidence_id = evidence_id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| fresh_id(&format!("evidence-{task_id}-{kind}")));
        let now = now_timestamp();
        let evidence = EvidenceView {
            id: evidence_id.clone(),
            kind: kind.clone(),
            summary: truncate_for_preview(&summary, 500),
            path,
            source,
            metadata: None,
            timestamp: Some(now),
        };
        self.upsert_runtime_evidence(evidence.clone());

        let runtime_evidence = self.runtime_evidence.clone();
        let gate = &mut self.runtime_merge_gates[gate_index];
        if !gate.evidence_ids.contains(&evidence_id) {
            gate.evidence_ids.push(evidence_id.clone());
        }
        gate.status = reduce_merge_gate_status(gate, &runtime_evidence);
        gate.updated_at = Some(now);
        let gate = gate.clone();

        let mut events = vec![
            RuntimeEvent::new(
                1,
                RuntimeEventKind::EvidenceRecorded {
                    evidence: evidence.clone(),
                },
            ),
            RuntimeEvent::new(2, RuntimeEventKind::MergeGateUpdated { gate }),
        ];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            if !task.evidence.contains(&evidence_id) {
                task.evidence.push(evidence_id.clone());
            }
            task.activity = format!("evidence recorded: {kind}");
            task.updated_at = Some(u128::from(now) * 1000);
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }

        self.persist_agent_event(
            &dag_id,
            Some(&task_id),
            "agent_evidence_recorded",
            &[
                ("gate_id", gate_id),
                ("evidence_id", evidence_id.as_str()),
                ("evidence_kind", kind.as_str()),
                ("summary", summary.as_str()),
            ],
        )?;
        Ok(events)
    }

    fn accept_agent_artifact(
        &mut self,
        gate_id: &str,
        evidence_id: String,
        decision: String,
    ) -> Result<Vec<RuntimeEvent>, String> {
        if evidence_id.trim().is_empty() {
            return Err("agent artifact evidence id cannot be empty".to_string());
        }
        let gate_index = self
            .runtime_merge_gates
            .iter()
            .position(|gate| gate.gate_id == gate_id)
            .ok_or_else(|| format!("merge gate `{gate_id}` does not exist"))?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        let dag_id = self.dag_id_for_task(&task_id)?;
        let evidence = self
            .runtime_evidence
            .iter()
            .find(|evidence| evidence.id == evidence_id)
            .cloned()
            .ok_or_else(|| format!("agent artifact evidence `{evidence_id}` does not exist"))?;
        let now = now_timestamp();
        let runtime_evidence = self.runtime_evidence.clone();
        let gate = &mut self.runtime_merge_gates[gate_index];
        if !gate.evidence_ids.contains(&evidence_id) {
            gate.evidence_ids.push(evidence_id.clone());
        }
        gate.status = reduce_merge_gate_status(gate, &runtime_evidence);
        gate.decision = Some(decision.clone());
        gate.updated_at = Some(now);
        let gate = gate.clone();

        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::MergeGateUpdated { gate },
        )];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            if !task.evidence.contains(&evidence_id) {
                task.evidence.push(evidence_id.clone());
            }
            task.decision = Some(decision.clone());
            task.activity = format!("artifact accepted: {decision}");
            task.updated_at = Some(u128::from(now) * 1000);
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }

        self.persist_agent_event(
            &dag_id,
            Some(&task_id),
            "agent_artifact_accepted",
            &[
                ("gate_id", gate_id),
                ("evidence_id", evidence_id.as_str()),
                ("evidence_kind", evidence.kind.as_str()),
                ("decision", decision.as_str()),
            ],
        )?;
        Ok(events)
    }

    fn reject_agent_artifact(
        &mut self,
        gate_id: &str,
        evidence_id: &str,
        reason: String,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let gate_index = self
            .runtime_merge_gates
            .iter()
            .position(|gate| gate.gate_id == gate_id)
            .ok_or_else(|| format!("merge gate `{gate_id}` does not exist"))?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        let dag_id = self.dag_id_for_task(&task_id)?;
        let now = now_timestamp();
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.evidence_ids.retain(|id| id != evidence_id);
        gate.status = MergeGateStatus::NeedsChanges;
        gate.decision = Some(reason.clone());
        gate.updated_at = Some(now);
        let gate = gate.clone();

        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::MergeGateUpdated { gate },
        )];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            task.evidence.retain(|id| id != evidence_id);
            task.status = AgentTaskStatus::NeedsInput.as_str().to_string();
            task.activity = format!("artifact rejected: {reason}");
            task.decision = Some(reason.clone());
            task.next_action = Some(AgentNextAction {
                label: "revise artifact".to_string(),
                command: Some(format!("/agent start {}", task.id)),
                reason: Some("merge gate rejected an artifact".to_string()),
            });
            task.updated_at = Some(u128::from(now) * 1000);
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }

        self.persist_agent_event(
            &dag_id,
            Some(&task_id),
            "agent_artifact_rejected",
            &[
                ("gate_id", gate_id),
                ("evidence_id", evidence_id),
                ("reason", reason.as_str()),
            ],
        )?;
        Ok(events)
    }

    fn merge_agent_patch(
        &mut self,
        gate_id: &str,
        decision: String,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let gate_index = self
            .runtime_merge_gates
            .iter()
            .position(|gate| gate.gate_id == gate_id)
            .ok_or_else(|| format!("merge gate `{gate_id}` does not exist"))?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        let dag_id = self.dag_id_for_task(&task_id)?;
        if self.runtime_merge_gates[gate_index].status != MergeGateStatus::Accepted {
            return Err(format!(
                "merge gate `{gate_id}` must be accepted before patch merge"
            ));
        }
        let patch_evidence = self.patch_evidence_for_gate(gate_index).cloned();
        let Some(patch_evidence) = patch_evidence else {
            return self.mark_agent_patch_conflict(
                gate_index,
                &dag_id,
                &task_id,
                "patch conflict: accepted merge gate has no patch evidence".to_string(),
            );
        };
        let patch_application =
            match prepare_unified_diff_application(&self.cwd, &patch_evidence.summary) {
                Ok(application) => application,
                Err(err) => {
                    return self.mark_agent_patch_conflict(
                        gate_index,
                        &dag_id,
                        &task_id,
                        format!("patch conflict: {err}"),
                    );
                }
            };
        if let Err(err) = write_patch_application(&patch_application) {
            return self.mark_agent_patch_conflict(
                gate_index,
                &dag_id,
                &task_id,
                format!("patch conflict: {err}"),
            );
        }
        let changed_files = patch_application.changed_files.join(",");

        let now = now_timestamp();
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.status = MergeGateStatus::Merged;
        gate.decision = Some(decision.clone());
        gate.updated_at = Some(now);
        let gate = gate.clone();

        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::MergeGateUpdated { gate },
        )];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            task.status = AgentTaskStatus::Applied.as_str().to_string();
            task.activity = format!("patch merged: {decision}");
            task.decision = Some(decision.clone());
            task.next_action = None;
            task.updated_at = Some(u128::from(now) * 1000);
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }

        self.persist_agent_event(
            &dag_id,
            Some(&task_id),
            "agent_patch_merged",
            &[
                ("gate_id", gate_id),
                ("decision", decision.as_str()),
                ("evidence_id", patch_evidence.id.as_str()),
                ("changed_files", changed_files.as_str()),
            ],
        )?;
        Ok(events)
    }

    fn patch_evidence_for_gate(&self, gate_index: usize) -> Option<&EvidenceView> {
        let gate = &self.runtime_merge_gates[gate_index];
        gate.evidence_ids.iter().find_map(|id| {
            self.runtime_evidence
                .iter()
                .find(|evidence| evidence.id == *id && evidence.kind == "patch")
        })
    }

    fn mark_agent_patch_conflict(
        &mut self,
        gate_index: usize,
        dag_id: &str,
        task_id: &str,
        reason: String,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let now = now_timestamp();
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.status = MergeGateStatus::NeedsChanges;
        gate.decision = Some(reason.clone());
        gate.updated_at = Some(now);
        let gate = gate.clone();
        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::MergeGateUpdated { gate },
        )];

        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            task.status = AgentTaskStatus::NeedsInput.as_str().to_string();
            task.activity = reason.clone();
            task.decision = Some(reason.clone());
            task.next_action = Some(AgentNextAction {
                label: "revise patch".to_string(),
                command: Some(format!("/agent start {task_id}")),
                reason: Some("merge gate could not apply the accepted patch".to_string()),
            });
            task.updated_at = Some(u128::from(now) * 1000);
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(
                next_sequence(&events),
                RuntimeEventKind::TaskUpdated { task },
            ));
        }

        self.persist_agent_event(
            dag_id,
            Some(task_id),
            "agent_patch_conflict",
            &[("reason", reason.as_str())],
        )?;
        Ok(events)
    }

    fn dag_id_for_task(&self, task_id: &str) -> Result<String, String> {
        self.runtime_agent_dags
            .iter()
            .find(|dag| dag.tasks.iter().any(|task| task.task_id == task_id))
            .map(|dag| dag.dag_id.clone())
            .ok_or_else(|| format!("agent DAG for task `{task_id}` does not exist"))
    }

    fn persist_agent_event(
        &self,
        dag_id: &str,
        task_id: Option<&str>,
        event_type: &str,
        payload_fields: &[(&str, &str)],
    ) -> Result<(), String> {
        let payload = payload_fields
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        self.workflows.append_agent_event(&WorkflowAgentEvent {
            event_id: fresh_id("agent_evt"),
            dag_id: dag_id.to_string(),
            task_id: task_id.map(ToString::to_string),
            event_type: event_type.to_string(),
            timestamp: now_timestamp(),
            origin_session_id: Some(self.session_id().to_string()),
            payload,
        })
    }
}

fn agent_task_prompt(spec: &AgentDagTaskSpec) -> String {
    format!(
        "You are the {role} agent for a supervised Viden Agent DAG task.\n\
         Task: {title}\n\
         Objective: {objective}\n\
         File scope: {scope}\n\
         Permission policy: {permission}\n\
         Required evidence: {evidence}\n\
         Return concise output that can be recorded as {role} evidence.",
        role = spec.role.as_str(),
        title = spec.title,
        objective = spec.objective,
        scope = if spec.file_scope.is_empty() {
            "<none>".to_string()
        } else {
            spec.file_scope.join(", ")
        },
        permission = spec.permission_policy,
        evidence = if spec.required_evidence.is_empty() {
            "<none>".to_string()
        } else {
            spec.required_evidence.join(", ")
        }
    )
}

fn assistant_output_from_events(events: &[RuntimeEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::AssistantDelta { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn evidence_kind_for_role(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Planner => "plan",
        AgentRole::Coder => "patch",
        AgentRole::Reviewer => "review",
        AgentRole::Tester => "test_result",
        AgentRole::DocWriter => "doc_update",
        AgentRole::ReleaseOperator => "release_artifact",
        AgentRole::External => "agent_output",
    }
}

fn role_guidance_context_source(role: AgentRole) -> ContextSourceRecord {
    let (name, summary) = match role {
        AgentRole::Planner => (
            "role-planning-context",
            "Focus on requirements, architecture boundaries, sequencing, risks, and explicit non-goals before implementation.",
        ),
        AgentRole::Coder => (
            "role-implementation-context",
            "Focus on scoped files, existing local patterns, minimal implementation changes, and compile/test feedback.",
        ),
        AgentRole::Reviewer => (
            "role-review-context",
            "Focus on behavioral regressions, permission violations, missing tests, and evidence quality before acceptance.",
        ),
        AgentRole::Tester => (
            "role-verification-context",
            "Focus on focused checks, full gates, failure classification, reproducible logs, and release evidence.",
        ),
        AgentRole::DocWriter => (
            "role-documentation-context",
            "Focus on user-visible behavior, architecture contracts, bilingual docs, and keeping roadmap status current.",
        ),
        AgentRole::ReleaseOperator => (
            "role-release-context",
            "Focus on release gates, artifacts, version consistency, Homebrew sync, and post-publish validation.",
        ),
        AgentRole::External => (
            "role-external-agent-context",
            "Focus on adapter boundaries, evidence import, permission scope, and durable task handoff records.",
        ),
    };
    ContextSourceRecord {
        name: name.to_string(),
        kind: "role-guidance".to_string(),
        priority: 98,
        estimated_tokens: 220,
        summary: summary.to_string(),
        include_reason: format!(
            "{} role requires specialized context selection",
            role.as_str()
        ),
        handle_id: None,
        item_id: None,
        view_id: None,
        content_sha256: None,
        view_sha256: None,
        quality_id: None,
    }
}

fn agent_file_scope_context_source(spec: &AgentDagTaskSpec) -> ContextSourceRecord {
    ContextSourceRecord {
        name: "agent-file-scope".to_string(),
        kind: "file-scope".to_string(),
        priority: 94,
        estimated_tokens: 180,
        summary: spec.file_scope.join(", "),
        include_reason: "AgentTask file_scope limits which project areas this role should inspect"
            .to_string(),
        handle_id: None,
        item_id: None,
        view_id: None,
        content_sha256: None,
        view_sha256: None,
        quality_id: None,
    }
}

fn agent_evidence_contract_context_source(spec: &AgentDagTaskSpec) -> ContextSourceRecord {
    ContextSourceRecord {
        name: "agent-evidence-contract".to_string(),
        kind: "evidence-contract".to_string(),
        priority: 92,
        estimated_tokens: 160,
        summary: spec.required_evidence.join(", "),
        include_reason: "required_evidence defines the output contract for the merge gate"
            .to_string(),
        handle_id: None,
        item_id: None,
        view_id: None,
        content_sha256: None,
        view_sha256: None,
        quality_id: None,
    }
}

fn agent_selected_files_context_source(
    cwd: &Path,
    spec: &AgentDagTaskSpec,
) -> Option<ContextSourceRecord> {
    let selected_files = select_role_specific_files(cwd, spec);
    if selected_files.is_empty() {
        return None;
    }
    let estimated_tokens = selected_files.len().saturating_mul(48).min(640) as u64;
    Some(ContextSourceRecord {
        name: "role-selected-files".to_string(),
        kind: "selected-files".to_string(),
        priority: 96,
        estimated_tokens,
        summary: selected_files.join("\n"),
        include_reason: format!(
            "{} role gets deterministic file candidates from AgentTask file_scope",
            spec.role.as_str()
        ),
        handle_id: None,
        item_id: None,
        view_id: None,
        content_sha256: None,
        view_sha256: None,
        quality_id: None,
    })
}

fn agent_selected_symbols_context_source(
    cwd: &Path,
    spec: &AgentDagTaskSpec,
) -> Option<ContextSourceRecord> {
    let selected_symbols = select_role_specific_symbols(cwd, spec);
    if selected_symbols.is_empty() {
        return None;
    }
    let estimated_tokens = selected_symbols.len().saturating_mul(32).min(640) as u64;
    Some(ContextSourceRecord {
        name: "role-selected-symbols".to_string(),
        kind: "selected-symbols".to_string(),
        priority: 95,
        estimated_tokens,
        summary: selected_symbols.join("\n"),
        include_reason: format!(
            "{} role gets bounded symbol candidates from selected source files",
            spec.role.as_str()
        ),
        handle_id: None,
        item_id: None,
        view_id: None,
        content_sha256: None,
        view_sha256: None,
        quality_id: None,
    })
}

fn select_role_specific_files(cwd: &Path, spec: &AgentDagTaskSpec) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for scope in &spec.file_scope {
        let scoped_path = cwd.join(scope);
        collect_role_file_candidates(cwd, &scoped_path, 0, &mut seen, &mut candidates);
        if candidates.len() >= 512 {
            break;
        }
    }
    candidates.sort_by(|left, right| {
        role_file_score(spec.role, right)
            .cmp(&role_file_score(spec.role, left))
            .then_with(|| left.cmp(right))
    });
    candidates.truncate(8);
    candidates
}

fn select_role_specific_symbols(cwd: &Path, spec: &AgentDagTaskSpec) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for file in select_role_specific_files(cwd, spec) {
        let path = cwd.join(&file);
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        for symbol in extract_context_symbols_from_file(&file, &contents) {
            if seen.insert(symbol.clone()) {
                candidates.push(symbol);
            }
            if candidates.len() >= 128 {
                break;
            }
        }
        if candidates.len() >= 128 {
            break;
        }
    }
    candidates.sort_by(|left, right| {
        role_symbol_score(spec.role, right)
            .cmp(&role_symbol_score(spec.role, left))
            .then_with(|| left.cmp(right))
    });
    candidates.truncate(12);
    candidates
}

fn extract_context_symbols_from_file(relative_file: &str, contents: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    for raw_line in contents.lines().take(2_000) {
        let line = raw_line.trim();
        for (prefix, kind) in [
            ("pub async fn ", "fn"),
            ("async fn ", "fn"),
            ("pub fn ", "fn"),
            ("fn ", "fn"),
            ("pub struct ", "struct"),
            ("struct ", "struct"),
            ("pub enum ", "enum"),
            ("enum ", "enum"),
            ("pub trait ", "trait"),
            ("trait ", "trait"),
            ("impl ", "impl"),
        ] {
            if let Some(name) = parse_symbol_name(line, prefix) {
                symbols.push(format!("{relative_file}::{kind} {name}"));
                break;
            }
        }
    }
    symbols
}

fn parse_symbol_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name = rest
        .split(|character: char| {
            character == '('
                || character == '<'
                || character == '{'
                || character == ':'
                || character == ';'
                || character.is_whitespace()
        })
        .next()?
        .trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn role_symbol_score(role: AgentRole, symbol: &str) -> u8 {
    let lower = symbol.to_ascii_lowercase();
    let is_test_file =
        lower.contains("/tests/") || lower.contains("_test.") || lower.contains(".test.");
    let is_test_symbol = lower.contains("::fn test")
        || lower.contains("::fn should_")
        || lower.contains("::fn runtime_supervisor_")
        || lower.contains("::fn provider_")
        || lower.contains("::fn workflow_");
    let is_type = lower.contains("::struct ")
        || lower.contains("::enum ")
        || lower.contains("::trait ")
        || lower.contains("::impl ");
    match role {
        AgentRole::Tester => {
            if is_test_file && is_test_symbol {
                98
            } else if is_test_file {
                88
            } else if is_test_symbol {
                82
            } else {
                40
            }
        }
        AgentRole::Coder => {
            if !is_test_file && is_type {
                94
            } else if !is_test_file {
                88
            } else {
                45
            }
        }
        AgentRole::Reviewer => {
            if is_type || is_test_symbol {
                88
            } else {
                70
            }
        }
        AgentRole::Planner
        | AgentRole::DocWriter
        | AgentRole::ReleaseOperator
        | AgentRole::External => {
            if is_type {
                78
            } else {
                55
            }
        }
    }
}

fn is_lsp_context_candidate(file: &str) -> bool {
    let lower = file.to_ascii_lowercase();
    lower.ends_with(".rs")
        || lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".py")
        || lower.ends_with(".go")
}

fn collect_role_file_candidates(
    cwd: &Path,
    path: &Path,
    depth: usize,
    seen: &mut BTreeSet<String>,
    candidates: &mut Vec<String>,
) {
    if candidates.len() >= 512 || depth > 5 {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if let Some(relative) = normalized_relative_file(cwd, path)
            && seen.insert(relative.clone())
        {
            candidates.push(relative);
        }
        return;
    }
    if !metadata.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let entry_path = entry.path();
        if should_skip_context_path(&entry_path) {
            continue;
        }
        collect_role_file_candidates(cwd, &entry_path, depth + 1, seen, candidates);
        if candidates.len() >= 512 {
            break;
        }
    }
}

fn normalized_relative_file(cwd: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(cwd).ok()?;
    let normalized = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() || should_skip_context_path(Path::new(&normalized)) {
        None
    } else {
        Some(normalized)
    }
}

fn should_skip_context_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git"
                | ".viden"
                | ".worktrees"
                | ".ref"
                | "target"
                | "node_modules"
                | "dist"
                | "build"
        ) || (name.starts_with('.') && name != ".github")
    })
}

fn role_file_score(role: AgentRole, file: &str) -> u8 {
    let lower = file.to_ascii_lowercase();
    let is_doc = lower.ends_with(".md")
        || lower.contains("/docs/")
        || lower.ends_with("readme")
        || lower.contains("changelog");
    let is_test = lower.contains("/tests/")
        || lower.contains("_test.")
        || lower.contains(".test.")
        || lower.contains("_spec.")
        || lower.contains(".spec.");
    let is_source = lower.ends_with(".rs")
        || lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".py")
        || lower.ends_with(".go");
    let is_manifest = lower.ends_with("cargo.toml")
        || lower.ends_with("package.json")
        || lower.ends_with("pyproject.toml")
        || lower.ends_with("makefile");

    match role {
        AgentRole::Planner => {
            if is_doc {
                90
            } else if is_manifest {
                78
            } else {
                40
            }
        }
        AgentRole::Coder => {
            if is_source && !is_test {
                95
            } else if is_manifest {
                75
            } else if is_test {
                65
            } else {
                35
            }
        }
        AgentRole::Reviewer => {
            if is_source || is_test {
                88
            } else if is_doc || is_manifest {
                76
            } else {
                35
            }
        }
        AgentRole::Tester => {
            if is_test {
                96
            } else if is_manifest {
                82
            } else if is_source {
                62
            } else {
                30
            }
        }
        AgentRole::DocWriter => {
            if is_doc {
                96
            } else if is_manifest {
                48
            } else {
                25
            }
        }
        AgentRole::ReleaseOperator => {
            if lower.contains("release") || lower.contains("changelog") {
                96
            } else if is_manifest || is_doc {
                78
            } else {
                30
            }
        }
        AgentRole::External => {
            if is_source || is_test || is_doc || is_manifest {
                70
            } else {
                30
            }
        }
    }
}

fn reduce_merge_gate_status(gate: &MergeGateRecord, evidence: &[EvidenceView]) -> MergeGateStatus {
    // Merge gates are reduced from recorded evidence facts, not frontend-local
    // checklist state or evidence id naming conventions.
    if merge_gate_has_required_evidence(gate, evidence) {
        MergeGateStatus::Accepted
    } else {
        MergeGateStatus::CollectingEvidence
    }
}

fn merge_gate_has_required_evidence(gate: &MergeGateRecord, evidence: &[EvidenceView]) -> bool {
    let collected_kinds = gate
        .evidence_ids
        .iter()
        .filter_map(|evidence_id| evidence.iter().find(|item| item.id == *evidence_id))
        .map(|item| normalize_evidence_kind(&item.kind))
        .collect::<BTreeSet<_>>();
    gate.required_evidence
        .iter()
        .map(|required| normalize_evidence_kind(required))
        .all(|required| collected_kinds.contains(&required))
}

fn normalize_evidence_kind(kind: &str) -> String {
    kind.trim().replace('-', "_").to_ascii_lowercase()
}

#[derive(Debug, Clone)]
struct PatchApplication {
    writes: Vec<(PathBuf, String)>,
    changed_files: Vec<String>,
}

#[derive(Debug)]
struct PatchFile {
    path: String,
    hunks: Vec<PatchHunk>,
}

#[derive(Debug)]
struct PatchHunk {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

fn prepare_unified_diff_application(cwd: &Path, diff: &str) -> Result<PatchApplication, String> {
    let patch_files = parse_unified_diff(diff)?;
    if patch_files.is_empty() {
        return Err("no unified diff patch found".to_string());
    }

    let mut writes = Vec::new();
    let mut changed_files = Vec::new();
    for patch_file in patch_files {
        let relative_path = validate_patch_path(&patch_file.path)?;
        let full_path = cwd.join(&relative_path);
        let current = fs::read_to_string(&full_path)
            .map_err(|err| format!("{}: {err}", relative_path.display()))?;
        let updated = apply_patch_file(&current, &patch_file)
            .map_err(|err| format!("{}: {err}", relative_path.display()))?;
        changed_files.push(normalize_pathbuf(&relative_path));
        writes.push((full_path, updated));
    }

    Ok(PatchApplication {
        writes,
        changed_files,
    })
}

fn write_patch_application(application: &PatchApplication) -> Result<(), String> {
    for (path, contents) in &application.writes {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        fs::write(path, contents).map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(())
}

fn parse_unified_diff(diff: &str) -> Result<Vec<PatchFile>, String> {
    let mut files = Vec::new();
    let mut current_file: Option<PatchFile> = None;
    let mut current_hunk: Option<PatchHunk> = None;

    for raw_line in diff.split_inclusive('\n') {
        let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("diff --git ") {
            finish_patch_hunk(&mut current_file, &mut current_hunk);
            if let Some(file) = current_file.take() {
                files.push(file);
            }
            let path = rest
                .split_whitespace()
                .find_map(|part| part.strip_prefix("b/"))
                .or_else(|| rest.split_whitespace().nth(1))
                .ok_or_else(|| format!("invalid diff header `{line}`"))?;
            current_file = Some(PatchFile {
                path: path.to_string(),
                hunks: Vec::new(),
            });
            continue;
        }

        if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(file) = current_file.as_mut()
                && let Some(path) = path.trim().strip_prefix("b/")
            {
                file.path = path.to_string();
            }
            continue;
        }

        if line.starts_with("@@") {
            let Some(file) = current_file.as_mut() else {
                return Err("hunk appeared before file header".to_string());
            };
            if let Some(hunk) = current_hunk.take() {
                file.hunks.push(hunk);
            }
            current_hunk = Some(PatchHunk {
                old_lines: Vec::new(),
                new_lines: Vec::new(),
            });
            continue;
        }

        let Some(hunk) = current_hunk.as_mut() else {
            continue;
        };
        if line.starts_with('\\') {
            continue;
        }
        let Some((prefix, content)) = split_patch_line(raw_line) else {
            continue;
        };
        match prefix {
            ' ' => {
                hunk.old_lines.push(content.clone());
                hunk.new_lines.push(content);
            }
            '-' => hunk.old_lines.push(content),
            '+' => hunk.new_lines.push(content),
            _ => {}
        }
    }

    finish_patch_hunk(&mut current_file, &mut current_hunk);
    if let Some(file) = current_file {
        files.push(file);
    }
    files.retain(|file| !file.hunks.is_empty());
    Ok(files)
}

fn finish_patch_hunk(file: &mut Option<PatchFile>, hunk: &mut Option<PatchHunk>) {
    if let (Some(file), Some(hunk)) = (file.as_mut(), hunk.take()) {
        file.hunks.push(hunk);
    }
}

fn split_patch_line(raw_line: &str) -> Option<(char, String)> {
    let prefix = raw_line.chars().next()?;
    if !matches!(prefix, ' ' | '-' | '+') {
        return None;
    }
    Some((prefix, raw_line[prefix.len_utf8()..].to_string()))
}

fn apply_patch_file(current: &str, patch_file: &PatchFile) -> Result<String, String> {
    let mut lines = split_preserving_newlines(current);
    let mut cursor = 0usize;
    for hunk in &patch_file.hunks {
        let Some(index) = find_line_sequence(&lines, &hunk.old_lines, cursor) else {
            return Err("patch conflict: expected hunk context was not found".to_string());
        };
        lines.splice(index..index + hunk.old_lines.len(), hunk.new_lines.clone());
        cursor = index + hunk.new_lines.len();
    }
    Ok(lines.concat())
}

fn split_preserving_newlines(input: &str) -> Vec<String> {
    if input.is_empty() {
        Vec::new()
    } else {
        input
            .split_inclusive('\n')
            .map(ToString::to_string)
            .collect()
    }
}

fn find_line_sequence(lines: &[String], needle: &[String], start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(lines.len()));
    }
    if needle.len() > lines.len() {
        return None;
    }
    (start..=lines.len() - needle.len())
        .find(|&index| lines[index..index + needle.len()] == *needle)
}

fn validate_patch_path(path: &str) -> Result<PathBuf, String> {
    let normalized = path
        .trim()
        .trim_start_matches("a/")
        .trim_start_matches("b/");
    if normalized.is_empty() || normalized == "/dev/null" {
        return Err("patch path is empty or unsupported".to_string());
    }
    let candidate = Path::new(normalized);
    if candidate.is_absolute() {
        return Err(format!("absolute patch path `{normalized}` is not allowed"));
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!("unsafe patch path `{normalized}`"));
    }
    Ok(candidate.to_path_buf())
}

fn normalize_pathbuf(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

struct AgentTaskFailureClassification {
    class: &'static str,
    recovery_suggestion: &'static str,
}

fn classify_agent_task_failure(error: &str) -> AgentTaskFailureClassification {
    let lower = error.to_ascii_lowercase();
    if lower.contains("api error (413)")
        || lower.contains("http 413")
        || lower.contains("payload too large")
        || lower.contains("request entity too large")
    {
        AgentTaskFailureClassification {
            class: "request_too_large",
            recovery_suggestion: "compact provider context, retry with a smaller prompt, or switch provider",
        }
    } else if lower.contains("api key")
        || lower.contains("missing key")
        || lower.contains("key=missing")
        || lower.contains("no api key")
    {
        AgentTaskFailureClassification {
            class: "missing_key",
            recovery_suggestion: "open provider config, export the listed key env var, then retry",
        }
    } else if lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("permission")
        || lower.contains("api error (401)")
        || lower.contains("api error (403)")
    {
        AgentTaskFailureClassification {
            class: "auth",
            recovery_suggestion: "run provider doctor, verify key scope, or switch to fallback",
        }
    } else if lower.contains("rate limit") || lower.contains("too many requests") {
        AgentTaskFailureClassification {
            class: "rate_limit",
            recovery_suggestion: "retry later, switch model/provider, or use fallback",
        }
    } else if lower.contains("timeout") || lower.contains("timed out") {
        AgentTaskFailureClassification {
            class: "timeout",
            recovery_suggestion: "retry, increase request timeout, or switch provider",
        }
    } else if lower.contains("context_length")
        || lower.contains("context_overflow")
        || lower.contains("maximum context")
        || lower.contains("context overflow")
    {
        AgentTaskFailureClassification {
            class: "context_overflow",
            recovery_suggestion: "compact context or switch to a larger-context model",
        }
    } else if lower.contains("tool_calls")
        || lower.contains("tool call")
        || lower.contains("tool_choice")
        || lower.contains("unsupported")
    {
        AgentTaskFailureClassification {
            class: "compatibility",
            recovery_suggestion: "switch to a known-compatible model or inspect provider doctor",
        }
    } else if lower.contains("model")
        || lower.contains("not found")
        || lower.contains("does not exist")
        || lower.contains("unavailable")
        || lower.contains("api error (400)")
        || lower.contains("api error (404)")
    {
        AgentTaskFailureClassification {
            class: "model_unavailable",
            recovery_suggestion: "open /models and switch to a known model candidate",
        }
    } else {
        AgentTaskFailureClassification {
            class: "provider_error",
            recovery_suggestion: "inspect provider status, retry, or switch provider",
        }
    }
}

fn validate_agent_dag_tasks(tasks: &[AgentDagTaskSpec]) -> Result<(), String> {
    let ids = tasks
        .iter()
        .map(|task| task.task_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if ids.len() != tasks.len() {
        return Err("agent DAG task ids must be unique".to_string());
    }
    for task in tasks {
        for dependency in &task.dependencies {
            if !ids.contains(dependency.as_str()) {
                return Err(format!(
                    "agent task `{}` depends on missing task `{dependency}`",
                    task.task_id
                ));
            }
        }
    }
    Ok(())
}

fn agent_task_record_from_spec(
    engine: &SessionEngine,
    spec: &AgentDagTaskSpec,
    timestamp: u64,
) -> AgentTaskRecord {
    let now = u128::from(timestamp) * 1000;
    AgentTaskRecord {
        id: spec.task_id.clone(),
        parent_id: spec.dependencies.first().cloned(),
        agent: spec.role.as_str().to_string(),
        kind: "agent".to_string(),
        transport: "runtime".to_string(),
        title: spec.title.clone(),
        status: AgentTaskStatus::Queued.as_str().to_string(),
        activity: "queued for supervised execution".to_string(),
        summary: spec.objective.clone(),
        progress: 0,
        started_at: None,
        updated_at: Some(now),
        workspace: spec
            .workspace
            .clone()
            .or_else(|| Some(engine.cwd().to_string_lossy().to_string())),
        evidence: spec
            .required_evidence
            .iter()
            .map(|kind| format!("required {kind}"))
            .collect(),
        permissions: vec![format!("policy {}", spec.permission_policy)],
        decision: None,
        result: None,
        resume_handle: None,
        pid: None,
        next_action: Some(AgentNextAction {
            label: role_next_action_label(spec.role).to_string(),
            command: Some(format!("/agent start {}", spec.task_id)),
            reason: Some("task is queued in the supervised Agent DAG".to_string()),
        }),
    }
}

fn role_next_action_label(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Planner => "start planning",
        AgentRole::Coder => "start coding",
        AgentRole::Reviewer => "start review",
        AgentRole::Tester => "start testing",
        AgentRole::DocWriter => "start docs",
        AgentRole::ReleaseOperator => "start release gate",
        AgentRole::External => "start external agent",
    }
}

fn agent_permission_mode_for_policy(current: PermissionMode, policy: &str) -> PermissionMode {
    if current == PermissionMode::Plan {
        return PermissionMode::Plan;
    }
    match policy
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "read_only" | "readonly" | "plan" => PermissionMode::Plan,
        "tester_verification"
        | "tester"
        | "test_only"
        | "docs_only"
        | "doc_writer"
        | "documentation"
        | "reviewer"
        | "review_only"
        | "scoped_mutation"
        | "coder"
        | "code"
        | "release_gate"
        | "release_operator"
        | "release"
        | "external_agent"
        | "external"
        | "least_privilege" => PermissionMode::Plan,
        "ask" | "default" => PermissionMode::Default,
        "auto_edit" | "accept_edits" => {
            if matches!(
                current,
                PermissionMode::AcceptEdits
                    | PermissionMode::DontAsk
                    | PermissionMode::BypassPermissions
            ) {
                PermissionMode::AcceptEdits
            } else {
                PermissionMode::Default
            }
        }
        "full_access" | "bypass" | "bypass_permissions" => {
            if current == PermissionMode::BypassPermissions {
                PermissionMode::BypassPermissions
            } else {
                PermissionMode::Default
            }
        }
        _ => PermissionMode::Default,
    }
}

fn apply_agent_role_permission_rules(engine: &mut PermissionEngine, spec: &AgentDagTaskSpec) {
    match normalized_policy_name(&spec.permission_policy).as_str() {
        "tester_verification" | "tester" | "test_only" => {
            allow_verification_shell_commands(engine);
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Deny,
                "write_file",
                None,
            ));
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Deny,
                "edit_file",
                None,
            ));
        }
        "scoped_mutation" | "coder" | "code" => {
            allow_scoped_file_mutations(engine, &spec.file_scope);
            allow_scoped_git_staging(engine, &spec.file_scope);
            deny_unscoped_file_roots(engine, &spec.file_scope);
            deny_unscoped_git_roots(engine, &spec.file_scope);
            allow_verification_shell_commands(engine);
            deny_high_risk_git_mutations(engine);
            deny_release_publication_tools(engine);
        }
        "docs_only" | "doc_writer" | "documentation" => {
            allow_docs_file_mutations(engine, &spec.file_scope);
            for blocked_prefix in ["crates/", "apps/", "plugins/"] {
                engine.add_rule(agent_permission_rule(
                    PermissionBehavior::Deny,
                    "write_file",
                    Some(blocked_prefix),
                ));
                engine.add_rule(agent_permission_rule(
                    PermissionBehavior::Deny,
                    "edit_file",
                    Some(blocked_prefix),
                ));
            }
        }
        "release_gate" | "release_operator" | "release" => {
            allow_verification_shell_commands(engine);
            allow_docs_file_mutations(engine, &spec.file_scope);
            allow_scoped_git_staging(engine, &spec.file_scope);
            deny_code_file_roots(engine);
            deny_unscoped_git_roots(engine, &spec.file_scope);
            deny_high_risk_git_mutations(engine);
            deny_release_publication_tools(engine);
        }
        "reviewer" | "review_only" => {
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Deny,
                "write_file",
                None,
            ));
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Deny,
                "edit_file",
                None,
            ));
        }
        "external_agent" | "external" | "least_privilege" => {
            for tool_name in [
                "write_file",
                "edit_file",
                "shell",
                "git_add",
                "git_commit",
                "git_push",
                "git_restore",
                "git_stash_drop",
                "git_stash_pop",
                "git_stash_push",
                "git_switch",
                "git_worktree_add",
                "git_worktree_remove",
            ] {
                engine.add_rule(agent_permission_rule(
                    PermissionBehavior::Deny,
                    tool_name,
                    None,
                ));
            }
        }
        _ => {}
    }
}

fn allow_verification_shell_commands(engine: &mut PermissionEngine) {
    for command_prefix in [
        "cargo test",
        "cargo nextest",
        "cargo build",
        "cargo check",
        "pytest",
        "npm test",
        "npm run test",
    ] {
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Allow,
            "shell",
            Some(command_prefix),
        ));
    }
}

fn allow_scoped_file_mutations(engine: &mut PermissionEngine, file_scope: &[String]) {
    for scope in file_scope {
        let scope = scope.trim();
        if scope.is_empty() {
            continue;
        }
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Allow,
            "write_file",
            Some(scope),
        ));
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Allow,
            "edit_file",
            Some(scope),
        ));
    }
}

fn allow_scoped_git_staging(engine: &mut PermissionEngine, file_scope: &[String]) {
    for scope in file_scope {
        let scope = scope.trim();
        if scope.is_empty() {
            continue;
        }
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Allow,
            "git_add",
            Some(scope),
        ));
    }
}

fn allow_docs_file_mutations(engine: &mut PermissionEngine, file_scope: &[String]) {
    let mut allowed_docs_scope = false;
    for scope in file_scope {
        if is_docs_scope(scope) {
            allowed_docs_scope = true;
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Allow,
                "write_file",
                Some(scope),
            ));
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Allow,
                "edit_file",
                Some(scope),
            ));
        }
    }
    if !allowed_docs_scope {
        for scope in ["docs", "README", "CHANGELOG"] {
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Allow,
                "write_file",
                Some(scope),
            ));
            engine.add_rule(agent_permission_rule(
                PermissionBehavior::Allow,
                "edit_file",
                Some(scope),
            ));
        }
    }
}

fn deny_unscoped_git_roots(engine: &mut PermissionEngine, file_scope: &[String]) {
    for root in ["apps/", "crates/", "docs/", "plugins/"] {
        if file_scope
            .iter()
            .any(|scope| scope_overlaps_root(scope, root))
        {
            continue;
        }
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            "git_add",
            Some(root),
        ));
    }
}

fn deny_unscoped_file_roots(engine: &mut PermissionEngine, file_scope: &[String]) {
    for root in ["apps/", "crates/", "docs/", "plugins/"] {
        if file_scope
            .iter()
            .any(|scope| scope_overlaps_root(scope, root))
        {
            continue;
        }
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            "write_file",
            Some(root),
        ));
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            "edit_file",
            Some(root),
        ));
    }
}

fn deny_high_risk_git_mutations(engine: &mut PermissionEngine) {
    for tool_name in [
        "git_commit",
        "git_restore",
        "git_stash_drop",
        "git_stash_pop",
        "git_stash_push",
        "git_switch",
        "git_worktree_add",
        "git_worktree_remove",
    ] {
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            tool_name,
            None,
        ));
    }
}

fn deny_code_file_roots(engine: &mut PermissionEngine) {
    for root in ["apps/", "crates/", "plugins/"] {
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            "write_file",
            Some(root),
        ));
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            "edit_file",
            Some(root),
        ));
    }
}

fn deny_release_publication_tools(engine: &mut PermissionEngine) {
    for (tool_name, rule_content) in [
        ("git_push", None),
        ("shell", Some("git push")),
        ("shell", Some("gh release")),
        ("shell", Some("cargo publish")),
    ] {
        engine.add_rule(agent_permission_rule(
            PermissionBehavior::Deny,
            tool_name,
            rule_content,
        ));
    }
}

fn scope_overlaps_root(scope: &str, root: &str) -> bool {
    let normalized_scope = scope.trim().trim_start_matches("./");
    if normalized_scope.is_empty() {
        return false;
    }
    normalized_scope.starts_with(root) || root.starts_with(normalized_scope)
}

fn agent_permission_rule(
    behavior: PermissionBehavior,
    tool_name: &str,
    rule_content: Option<&str>,
) -> PermissionRule {
    PermissionRule {
        source: PermissionRuleSource::PolicySettings,
        rule_behavior: behavior,
        rule_value: PermissionRuleValue {
            tool_name: tool_name.to_string(),
            rule_content: rule_content.map(ToString::to_string),
        },
    }
}

fn normalized_policy_name(policy: &str) -> String {
    policy.trim().to_ascii_lowercase().replace('-', "_")
}

fn is_docs_scope(scope: &str) -> bool {
    let normalized = scope.trim().trim_start_matches("./");
    normalized == "docs"
        || normalized.starts_with("docs/")
        || normalized.ends_with(".md")
        || normalized.ends_with(".mdx")
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
        metadata: None,
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

pub(crate) fn redacted_runtime_command_for_event(command: &RuntimeCommand) -> RuntimeCommand {
    match command {
        RuntimeCommand::SubmitUserInput { content } => RuntimeCommand::SubmitUserInput {
            content: redact_command_text(content),
        },
        RuntimeCommand::QueueFollowUp { content } => RuntimeCommand::QueueFollowUp {
            content: redact_command_text(content),
        },
        RuntimeCommand::StartAgentDag { goal, tasks } => RuntimeCommand::StartAgentDag {
            goal: redact_command_text(goal),
            tasks: tasks
                .iter()
                .map(|task| AgentDagTaskSpec {
                    task_id: task.task_id.clone(),
                    role: task.role,
                    title: redact_command_text(&task.title),
                    objective: redact_command_text(&task.objective),
                    dependencies: task.dependencies.clone(),
                    workspace: task.workspace.as_ref().map(|_| "[REDACTED]".to_string()),
                    file_scope: task
                        .file_scope
                        .iter()
                        .map(|_| "[REDACTED]".to_string())
                        .collect(),
                    context_bundle_id: task.context_bundle_id.clone(),
                    required_evidence: task.required_evidence.clone(),
                    permission_policy: task.permission_policy.clone(),
                })
                .collect(),
        },
        other => other.clone(),
    }
}

fn redact_command_text(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if lower.starts_with("sk-")
                || lower.contains("secret")
                || lower.contains("token=")
                || lower.contains("api_key")
                || word.starts_with('/')
                || word.contains("/Users/")
                || word.contains("/tmp/")
                || word.contains("/var/")
            {
                "[REDACTED]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn redacted_task_for_event(mut task: AgentTaskRecord) -> AgentTaskRecord {
    task.title = redact_command_text(&task.title);
    task.activity = redact_command_text(&task.activity);
    task.summary = redact_command_text(&task.summary);
    if let Some(result) = task.result {
        task.result = Some(redact_command_text(&result));
    }
    task.evidence = task
        .evidence
        .into_iter()
        .map(|evidence| redact_command_text(&evidence))
        .collect();
    task
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
