use crate::{
    AgentDagRecord, AgentDagTaskSpec, AgentLaneRecord, AgentTaskId, AgentTaskRecord,
    ApprovalResponse, ContextBundleRecord, EvidenceId, MergeGateId, MergeGateRecord, MessageId,
    PermissionLevel, RuntimeSnapshot, ToolCallId, WorkMode, now_timestamp,
};

/// UI-independent command contract sent from a client surface into the runtime.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeCommand {
    SubmitUserInput {
        content: String,
    },
    QueueFollowUp {
        content: String,
    },
    CancelActiveTurn,
    SetWorkMode {
        mode: WorkMode,
    },
    SetPermissionLevel {
        level: PermissionLevel,
    },
    RespondToApproval {
        request_id: String,
        response: ApprovalResponse,
    },
    ConfigureProvider {
        provider_id: String,
        api_key_env: Option<String>,
        endpoint: Option<String>,
        default_model: Option<String>,
    },
    SelectModel {
        provider_id: String,
        model: String,
    },
    ActivateModel {
        provider_id: String,
        model: String,
    },
    DeactivateModel {
        provider_id: String,
        model: String,
    },
    StartAgentDag {
        goal: String,
        tasks: Vec<AgentDagTaskSpec>,
    },
    StartAgentTask {
        task_id: AgentTaskId,
    },
    CancelAgentTask {
        task_id: AgentTaskId,
    },
    AcceptMergeGate {
        gate_id: MergeGateId,
        decision: Option<String>,
    },
    RejectMergeGate {
        gate_id: MergeGateId,
        reason: String,
    },
    RecordAgentEvidence {
        gate_id: MergeGateId,
        evidence_id: Option<EvidenceId>,
        kind: String,
        summary: String,
        path: Option<String>,
        source: Option<String>,
    },
    AcceptAgentArtifact {
        gate_id: MergeGateId,
        evidence_id: EvidenceId,
        decision: Option<String>,
    },
    RejectAgentArtifact {
        gate_id: MergeGateId,
        evidence_id: EvidenceId,
        reason: String,
    },
    MergeAgentPatch {
        gate_id: MergeGateId,
        decision: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandAction {
    pub id: String,
    pub label: String,
    pub command: RuntimeCommand,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub shortcut: Option<String>,
    pub destructive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApprovalRequestView {
    pub id: String,
    pub tool_name: String,
    pub title: String,
    pub message: String,
    pub input_preview: String,
    pub is_mutating: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceView {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub path: Option<String>,
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCallView {
    pub tool_call_id: ToolCallId,
    pub name: String,
    pub input_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderHealthView {
    pub provider_id: String,
    pub model: String,
    pub status: String,
    pub request_count: u64,
    pub error_count: u64,
    pub last_latency_ms: Option<u64>,
    pub average_latency_ms: Option<u64>,
    pub tokens_per_second: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenCostView {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_micro_usd: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeErrorView {
    pub message: String,
    pub recoverable: bool,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeCommandReceipt {
    pub command_id: String,
    pub command: RuntimeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueuedInputView {
    pub id: String,
    pub content_preview: String,
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeEvent {
    pub sequence: u64,
    pub timestamp: Option<u64>,
    pub kind: RuntimeEventKind,
}

impl RuntimeEvent {
    pub fn new(sequence: u64, kind: RuntimeEventKind) -> Self {
        Self {
            sequence,
            timestamp: Some(now_timestamp()),
            kind,
        }
    }

    pub fn with_timestamp(sequence: u64, timestamp: Option<u64>, kind: RuntimeEventKind) -> Self {
        Self {
            sequence,
            timestamp,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum RuntimeEventKind {
    SnapshotUpdated {
        snapshot: RuntimeSnapshot,
    },
    AssistantDelta {
        message_id: MessageId,
        task_id: Option<AgentTaskId>,
        content: String,
    },
    ToolCallStarted {
        tool_call_id: ToolCallId,
        name: String,
        input_preview: String,
    },
    ToolCallFinished {
        tool_call_id: ToolCallId,
        name: String,
        success: bool,
        exit_code: Option<i32>,
        evidence: Option<EvidenceView>,
    },
    ApprovalRequested {
        approval: ApprovalRequestView,
    },
    ApprovalResolved {
        request_id: String,
        approved: bool,
    },
    CommandAccepted {
        command_id: String,
        command: RuntimeCommand,
    },
    CommandRejected {
        command_id: String,
        reason: String,
    },
    InputQueued {
        input: QueuedInputView,
    },
    InputDequeued {
        input_id: String,
    },
    TaskUpdated {
        task: AgentTaskRecord,
    },
    AgentDagUpdated {
        dag: AgentDagRecord,
    },
    LaneUpdated {
        lane: AgentLaneRecord,
    },
    EvidenceRecorded {
        evidence: EvidenceView,
    },
    ContextUpdated {
        context: ContextBundleRecord,
    },
    MergeGateUpdated {
        gate: MergeGateRecord,
    },
    ProviderHealthUpdated {
        provider: ProviderHealthView,
    },
    TokenCostUpdated {
        cost: TokenCostView,
    },
    Error {
        error: RuntimeErrorView,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeViewState {
    pub snapshot: RuntimeSnapshot,
    pub pending_approvals: Vec<ApprovalRequestView>,
    pub queued_inputs: Vec<QueuedInputView>,
    pub active_tool_calls: Vec<ToolCallView>,
    pub tasks: Vec<AgentTaskRecord>,
    pub agent_dags: Vec<AgentDagRecord>,
    pub lanes: Vec<AgentLaneRecord>,
    pub latest_evidence: Vec<EvidenceView>,
    pub assistant_stream: String,
    pub context: Option<ContextBundleRecord>,
    pub provider: Option<ProviderHealthView>,
    pub token_cost: Option<TokenCostView>,
    pub merge_gates: Vec<MergeGateRecord>,
    pub errors: Vec<RuntimeErrorView>,
    pub last_command: Option<RuntimeCommandReceipt>,
}

impl RuntimeViewState {
    pub fn new(snapshot: RuntimeSnapshot) -> Self {
        Self {
            snapshot,
            pending_approvals: Vec::new(),
            queued_inputs: Vec::new(),
            active_tool_calls: Vec::new(),
            tasks: Vec::new(),
            agent_dags: Vec::new(),
            lanes: Vec::new(),
            latest_evidence: Vec::new(),
            assistant_stream: String::new(),
            context: None,
            provider: None,
            token_cost: None,
            merge_gates: Vec::new(),
            errors: Vec::new(),
            last_command: None,
        }
    }

    pub fn apply_event(&mut self, event: &RuntimeEvent) {
        match &event.kind {
            RuntimeEventKind::SnapshotUpdated { snapshot } => {
                self.snapshot = snapshot.clone();
            }
            RuntimeEventKind::AssistantDelta { content, .. } => {
                self.assistant_stream.push_str(content);
            }
            RuntimeEventKind::ToolCallStarted {
                tool_call_id,
                name,
                input_preview,
            } => upsert_by_id(
                &mut self.active_tool_calls,
                ToolCallView {
                    tool_call_id: tool_call_id.clone(),
                    name: name.clone(),
                    input_preview: input_preview.clone(),
                },
                |tool| tool.tool_call_id == *tool_call_id,
            ),
            RuntimeEventKind::ToolCallFinished {
                tool_call_id,
                evidence,
                ..
            } => {
                self.active_tool_calls
                    .retain(|tool| tool.tool_call_id != *tool_call_id);
                if let Some(evidence) = evidence {
                    self.latest_evidence.push(evidence.clone());
                }
            }
            RuntimeEventKind::ApprovalRequested { approval } => {
                upsert_by_id(&mut self.pending_approvals, approval.clone(), |existing| {
                    existing.id == approval.id
                })
            }
            RuntimeEventKind::ApprovalResolved { request_id, .. } => {
                self.pending_approvals
                    .retain(|approval| approval.id != *request_id);
            }
            RuntimeEventKind::CommandAccepted {
                command_id,
                command,
            } => {
                self.last_command = Some(RuntimeCommandReceipt {
                    command_id: command_id.clone(),
                    command: command.clone(),
                });
            }
            RuntimeEventKind::CommandRejected { command_id, reason } => {
                self.last_command = None;
                self.errors.push(RuntimeErrorView {
                    message: format!("command {command_id} rejected: {reason}"),
                    recoverable: true,
                    hint: None,
                });
            }
            RuntimeEventKind::InputQueued { input } => {
                upsert_by_id(&mut self.queued_inputs, input.clone(), |existing| {
                    existing.id == input.id
                });
            }
            RuntimeEventKind::InputDequeued { input_id } => {
                self.queued_inputs.retain(|input| input.id != *input_id);
            }
            RuntimeEventKind::TaskUpdated { task } => {
                upsert_by_id(&mut self.tasks, task.clone(), |existing| {
                    existing.id == task.id
                });
            }
            RuntimeEventKind::AgentDagUpdated { dag } => {
                upsert_by_id(&mut self.agent_dags, dag.clone(), |existing| {
                    existing.dag_id == dag.dag_id
                });
            }
            RuntimeEventKind::LaneUpdated { lane } => {
                upsert_by_id(&mut self.lanes, lane.clone(), |existing| {
                    existing.id == lane.id
                });
            }
            RuntimeEventKind::EvidenceRecorded { evidence } => {
                self.latest_evidence.push(evidence.clone());
            }
            RuntimeEventKind::ContextUpdated { context } => {
                self.context = Some(context.clone());
            }
            RuntimeEventKind::MergeGateUpdated { gate } => {
                upsert_by_id(&mut self.merge_gates, gate.clone(), |existing| {
                    existing.gate_id == gate.gate_id
                });
            }
            RuntimeEventKind::ProviderHealthUpdated { provider } => {
                self.provider = Some(provider.clone());
            }
            RuntimeEventKind::TokenCostUpdated { cost } => {
                self.token_cost = Some(cost.clone());
            }
            RuntimeEventKind::Error { error } => {
                self.errors.push(error.clone());
            }
        }
    }
}

fn upsert_by_id<T, F>(items: &mut Vec<T>, item: T, matches: F)
where
    F: Fn(&T) -> bool,
{
    if let Some(existing) = items.iter_mut().find(|existing| matches(existing)) {
        *existing = item;
    } else {
        items.push(item);
    }
}
