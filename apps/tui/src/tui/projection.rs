use viden_core::{
    AgentLaneRecord, ApprovalRequestView, ContextBundleRecord, CostLedgerTotals, EvidenceView,
    ProviderHealthView, RuntimeErrorView, RuntimeViewState,
};

use super::ui_state::TuiUiState;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct CockpitProjection {
    pub(super) lanes: Vec<AgentLaneRecord>,
    pub(super) evidence: Vec<EvidenceView>,
    pub(super) context: Option<ContextBundleRecord>,
    pub(super) cost: CostLedgerTotals,
    pub(super) provider: Option<ProviderHealthView>,
    pub(super) approvals: Vec<ApprovalRequestView>,
    pub(super) errors: Vec<RuntimeErrorView>,
}

impl CockpitProjection {
    pub(super) fn from(runtime: &RuntimeViewState, _ui: &TuiUiState) -> Self {
        Self {
            lanes: runtime.lanes.clone(),
            evidence: runtime.latest_evidence.clone(),
            context: runtime.context.clone(),
            cost: runtime.cost_ledger.clone(),
            provider: runtime.provider.clone(),
            approvals: runtime.pending_approvals.clone(),
            errors: runtime.errors.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use viden_core::{
        AgentLaneRecord, AgentRole, AgentRoute, ApprovalDefaultAction, ApprovalRisk,
        ApprovalTarget, DataEgressPolicy, ExecutionTarget, GateStrength, LaneBudget, LaneStatus,
        MutationPolicy, PermissionLevel, PermissionMode, ProviderHealthView, RuntimeSnapshot,
        RuntimeViewState, WorkMode,
    };

    use super::*;

    #[test]
    fn structured_runtime_fields_are_the_only_cockpit_fact_source() {
        let mut runtime = RuntimeViewState::new(runtime_snapshot());
        runtime.lanes.push(AgentLaneRecord {
            id: "lane-1".to_string(),
            task_id: Some("task-1".to_string()),
            role: AgentRole::Coder,
            route: AgentRoute::BuiltIn,
            gate_strength: GateStrength::Full,
            mutation_policy: MutationPolicy::ProposeOnly,
            worktree: None,
            branch: None,
            target: ExecutionTarget::Local,
            data_egress: DataEgressPolicy::Deny,
            status: LaneStatus::WaitingApproval,
            budget: LaneBudget::default(),
            active_session_ids: vec!["session-1".to_string()],
            summary: "structured lane".to_string(),
            evidence: vec!["evidence-1".to_string()],
        });
        runtime.latest_evidence.push(EvidenceView {
            id: "evidence-1".to_string(),
            kind: "test".to_string(),
            summary: "structured evidence".to_string(),
            path: None,
            source: Some("core".to_string()),
            canonical: None,
            metadata: None,
            timestamp: Some(1),
        });
        runtime.context = Some(ContextBundleRecord {
            bundle_id: "bundle-1".to_string(),
            task_id: "task-1".to_string(),
            policy: "structured-policy".to_string(),
            sources: Vec::new(),
            omitted_sources: Vec::new(),
            estimated_tokens: 700,
            largest_sources: Vec::new(),
            compaction_notes: Vec::new(),
            soft_token_budget: 800,
            hard_token_limit: 1_000,
        });
        runtime.cost_ledger.total_tokens = 321;
        runtime.provider = Some(ProviderHealthView {
            provider_id: "provider-1".to_string(),
            model: "model-1".to_string(),
            status: "healthy".to_string(),
            request_count: 3,
            error_count: 0,
            last_latency_ms: Some(12),
            average_latency_ms: Some(10),
            tokens_per_second: Some(50),
            credential: None,
        });
        runtime.pending_approvals.push(ApprovalRequestView {
            id: "approval-1".to_string(),
            tool_name: "shell".to_string(),
            title: "Run command".to_string(),
            message: "structured approval".to_string(),
            input_preview: "cargo test".to_string(),
            is_mutating: true,
            reason: Some("test".to_string()),
            owner: Default::default(),
            risk: ApprovalRisk::Medium,
            target: ApprovalTarget {
                kind: "command".to_string(),
                display: "cargo test".to_string(),
                canonical_ref: None,
            },
            allowed_scopes: Vec::new(),
            policy_reason_key: "policy.test".to_string(),
            policy_reason_args: Default::default(),
            expires_at: 0,
            default_action: ApprovalDefaultAction::Deny,
            audit_id: "audit-1".to_string(),
        });
        runtime.errors.push(RuntimeErrorView {
            message: "structured error".to_string(),
            recoverable: true,
            hint: Some("retry".to_string()),
        });
        let ui = TuiUiState {
            entries: vec![super::super::ui_state::TuiEntry {
                label: "assistant".to_string(),
                body: "fake lane done; zero cost; provider failed".to_string(),
            }],
            ..TuiUiState::default()
        };

        let projection = CockpitProjection::from(&runtime, &ui);

        assert_eq!(projection.lanes[0].status, LaneStatus::WaitingApproval);
        assert_eq!(projection.evidence[0].summary, "structured evidence");
        assert_eq!(projection.context.as_ref().unwrap().estimated_tokens, 700);
        assert_eq!(projection.cost.total_tokens, 321);
        assert_eq!(projection.provider.as_ref().unwrap().status, "healthy");
        assert_eq!(projection.approvals[0].id, "approval-1");
        assert_eq!(projection.errors[0].message, "structured error");
    }

    #[test]
    fn changing_transcript_copy_does_not_change_cockpit_facts() {
        let mut runtime = RuntimeViewState::new(runtime_snapshot());
        runtime.provider = Some(ProviderHealthView {
            provider_id: "provider-1".to_string(),
            model: "model-1".to_string(),
            status: "healthy".to_string(),
            request_count: 1,
            error_count: 0,
            last_latency_ms: None,
            average_latency_ms: None,
            tokens_per_second: None,
            credential: None,
        });
        let first = CockpitProjection::from(
            &runtime,
            &TuiUiState {
                entries: vec![super::super::ui_state::TuiEntry {
                    label: "assistant".to_string(),
                    body: "provider failed".to_string(),
                }],
                ..TuiUiState::default()
            },
        );
        let second = CockpitProjection::from(
            &runtime,
            &TuiUiState {
                entries: vec![super::super::ui_state::TuiEntry {
                    label: "assistant".to_string(),
                    body: "provider healthy".to_string(),
                }],
                ..TuiUiState::default()
            },
        );

        assert_eq!(first, second);
        assert_eq!(first.provider.unwrap().status, "healthy");
    }

    fn runtime_snapshot() -> RuntimeSnapshot {
        RuntimeSnapshot {
            cwd: PathBuf::from("/workspace"),
            provider_family: "provider-1".to_string(),
            model_label: "model-1".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "fixture".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        }
    }
}
