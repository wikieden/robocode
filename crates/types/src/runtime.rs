use crate::{
    AgentDagRecord, AgentDagTaskSpec, AgentLaneRecord, AgentTaskId, AgentTaskRecord,
    ApprovalResponse, ContextBudgetRecord, ContextBundleRecord, ContextBundleSummaryRecord,
    ContextHandleRecord, ContextItemRecord, ContextQualityRecord, ContextRetrievalRecord,
    ContextScope, ContextViewRecord, CostLedgerTotals, CostUsageRecord,
    EvidenceCanonicalizationRecord, EvidenceId, MergeGateId, MergeGateRecord, MessageId,
    PermissionLevel, ProviderCacheObservationRecord, RuntimeSnapshot, ToolCallId, WorkMode,
    now_timestamp,
};
use std::collections::BTreeSet;

const RUNTIME_VIEW_COLLECTION_LIMIT: usize = 50;

/// UI-independent command contract sent from a client surface into the runtime.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// This is a stable serialized runtime protocol enum. Boxing large payloads
// would churn public construction semantics without changing wire format.
#[allow(clippy::large_enum_variant)]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        canonical: Option<CanonicalEvidenceReference>,
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
    RetrieveContext {
        handle_id: String,
        reason: String,
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
    pub canonical: Option<CanonicalEvidenceReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalEvidenceReference {
    pub item_id: String,
    pub bundle_id: String,
    pub source_hash: String,
    pub producer: EvidenceProducer,
    pub permission_snapshot_id: Option<String>,
    pub permission_scope: ContextScope,
    pub evidence_scope: ContextScope,
    pub verification: EvidenceVerificationState,
    pub quality: EvidenceQualityFacts,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceProducer {
    pub identity: String,
    pub role: String,
    pub task_id: AgentTaskId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerificationState {
    Unverified,
    Verified,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceQualityFacts {
    pub status: EvidenceQualityStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<EvidenceCanonicalReasonCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceQualityStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCanonicalStatus {
    Missing,
    Verified,
    Blocked,
    NeedsChanges,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCanonicalReasonCode {
    MissingCanonical,
    MissingRequiredKind,
    MissingSource,
    HashMismatch,
    ScopeMismatch,
    MissingPermissionSnapshot,
    InvalidPermissionSnapshot,
    MissingProducer,
    QualityFailed,
    VerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceCanonicalStatusReport {
    pub status: EvidenceCanonicalStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<EvidenceCanonicalReasonCode>,
}

pub fn canonical_evidence_status(evidence: &EvidenceView) -> EvidenceCanonicalStatus {
    match &evidence.canonical {
        None => EvidenceCanonicalStatus::Missing,
        Some(canonical) if canonical.quality.status == EvidenceQualityStatus::Fail => {
            EvidenceCanonicalStatus::NeedsChanges
        }
        Some(canonical) if canonical.verification == EvidenceVerificationState::Verified => {
            EvidenceCanonicalStatus::Verified
        }
        Some(_) => EvidenceCanonicalStatus::Blocked,
    }
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
// This is a stable serialized runtime protocol enum. Boxing large payloads
// would churn public construction semantics without changing wire format.
#[allow(clippy::large_enum_variant)]
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
    ContextBundleBuilt {
        bundle_id: String,
        scope: ContextScope,
        handle_ids: Vec<String>,
        estimated_tokens: u64,
    },
    ContextItemStored {
        item: ContextItemRecord,
    },
    ContextViewDerived {
        view: ContextViewRecord,
        handle: ContextHandleRecord,
    },
    ContextRetrieved {
        retrieval: ContextRetrievalRecord,
    },
    ContextBudgetExceeded {
        budget: ContextBudgetRecord,
    },
    ContextQualityFailed {
        quality: ContextQualityRecord,
    },
    CostUsageRecorded {
        cost: CostUsageRecord,
    },
    ProviderCacheObserved {
        provider_id: String,
        model: String,
        cached_input_tokens: u64,
        cache_hit_microunits: u64,
    },
    EvidenceCanonicalized {
        evidence_id: EvidenceId,
        item_id: String,
        content_sha256: String,
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
    #[serde(default)]
    pub context_bundles: Vec<ContextBundleSummaryRecord>,
    #[serde(default)]
    pub context_handles: Vec<ContextHandleRecord>,
    #[serde(default)]
    pub context_items: Vec<ContextItemRecord>,
    #[serde(default)]
    pub context_views: Vec<ContextViewRecord>,
    #[serde(default)]
    pub context_retrievals: Vec<ContextRetrievalRecord>,
    #[serde(default)]
    pub context_budgets: Vec<ContextBudgetRecord>,
    #[serde(default)]
    pub context_quality: Vec<ContextQualityRecord>,
    #[serde(default)]
    pub cost_usage: Vec<CostUsageRecord>,
    #[serde(default)]
    pub cost_ledger: CostLedgerTotals,
    #[serde(default, skip)]
    seen_cost_usage_ids: BTreeSet<String>,
    #[serde(default)]
    pub provider_cache_observations: Vec<ProviderCacheObservationRecord>,
    #[serde(default)]
    pub canonical_evidence: Vec<EvidenceCanonicalizationRecord>,
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
            context_bundles: Vec::new(),
            context_handles: Vec::new(),
            context_items: Vec::new(),
            context_views: Vec::new(),
            context_retrievals: Vec::new(),
            context_budgets: Vec::new(),
            context_quality: Vec::new(),
            cost_usage: Vec::new(),
            cost_ledger: CostLedgerTotals::default(),
            seen_cost_usage_ids: BTreeSet::new(),
            provider_cache_observations: Vec::new(),
            canonical_evidence: Vec::new(),
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
                    upsert_by_id(&mut self.latest_evidence, evidence.clone(), |existing| {
                        existing.id == evidence.id
                    });
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
                upsert_by_id(&mut self.latest_evidence, evidence.clone(), |existing| {
                    existing.id == evidence.id
                });
            }
            RuntimeEventKind::ContextUpdated { context } => {
                self.context = Some(context.clone());
            }
            RuntimeEventKind::ContextBundleBuilt {
                bundle_id,
                scope,
                handle_ids,
                estimated_tokens,
            } => {
                upsert_by_id(
                    &mut self.context_bundles,
                    ContextBundleSummaryRecord {
                        bundle_id: bundle_id.clone(),
                        scope: scope.clone(),
                        handle_ids: handle_ids.clone(),
                        estimated_tokens: *estimated_tokens,
                    },
                    |existing| existing.bundle_id == *bundle_id,
                );
                cap_vec(&mut self.context_bundles);
            }
            RuntimeEventKind::ContextItemStored { item } => {
                upsert_by_id(&mut self.context_items, item.clone(), |existing| {
                    existing.item_id == item.item_id
                });
                cap_vec(&mut self.context_items);
            }
            RuntimeEventKind::ContextViewDerived { view, handle } => {
                upsert_by_id(&mut self.context_views, view.clone(), |existing| {
                    existing.view_id == view.view_id
                });
                upsert_by_id(&mut self.context_handles, handle.clone(), |existing| {
                    existing.handle_id == handle.handle_id
                });
                cap_vec(&mut self.context_views);
                cap_vec(&mut self.context_handles);
            }
            RuntimeEventKind::ContextRetrieved { retrieval } => {
                self.context_retrievals.push(retrieval.clone());
                cap_vec(&mut self.context_retrievals);
            }
            RuntimeEventKind::ContextBudgetExceeded { budget } => {
                upsert_by_id(&mut self.context_budgets, budget.clone(), |existing| {
                    existing.budget_id == budget.budget_id
                });
                cap_vec(&mut self.context_budgets);
            }
            RuntimeEventKind::ContextQualityFailed { quality } => {
                upsert_by_id(&mut self.context_quality, quality.clone(), |existing| {
                    existing.quality_id == quality.quality_id
                });
                cap_vec(&mut self.context_quality);
            }
            RuntimeEventKind::CostUsageRecorded { cost } => {
                if self.seen_cost_usage_ids.insert(cost.usage_id.clone()) {
                    self.cost_ledger.record(cost);
                    self.cost_usage.push(cost.clone());
                    if self
                        .cost_usage
                        .iter()
                        .any(|record| record.actual_cost.is_none())
                    {
                        self.cost_ledger.actual_cost = None;
                        self.cost_ledger.total_actual_cost_micro_usd = None;
                    }
                }
                cap_vec(&mut self.cost_usage);
            }
            RuntimeEventKind::ProviderCacheObserved {
                provider_id,
                model,
                cached_input_tokens,
                cache_hit_microunits,
            } => {
                self.provider_cache_observations
                    .push(ProviderCacheObservationRecord {
                        provider_id: provider_id.clone(),
                        model: model.clone(),
                        cached_input_tokens: *cached_input_tokens,
                        cache_hit_microunits: *cache_hit_microunits,
                    });
                cap_vec(&mut self.provider_cache_observations);
            }
            RuntimeEventKind::EvidenceCanonicalized {
                evidence_id,
                item_id,
                content_sha256,
            } => {
                upsert_by_id(
                    &mut self.canonical_evidence,
                    EvidenceCanonicalizationRecord {
                        evidence_id: evidence_id.clone(),
                        item_id: item_id.clone(),
                        content_sha256: content_sha256.clone(),
                    },
                    |existing| existing.evidence_id == *evidence_id,
                );
                cap_vec(&mut self.canonical_evidence);
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

fn cap_vec<T>(items: &mut Vec<T>) {
    let excess = items.len().saturating_sub(RUNTIME_VIEW_COLLECTION_LIMIT);
    if excess > 0 {
        items.drain(0..excess);
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
