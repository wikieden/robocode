//! Viden core facade.
//!
//! This crate is the stable import boundary for runtime clients. It re-exports
//! the core runtime and contract types without introducing any TUI or GUI
//! dependency.

mod client;
mod compatibility;
mod host;
mod local_transport;

pub use client::{CoreClient, CoreClientError, CoreTransport, StatefulCoreClient};
pub use compatibility::{
    CORE_CLIENT_CAPABILITIES, CORE_CLIENT_VERSION, CORE_EXTENSION_CAPABILITIES,
    frontend_capabilities, local_core_handshake, validate_handshake, validate_schema_version,
};
pub use host::{
    BoundCoreClient, CoreHostError, LocalCoreHost, SecretBytes, WorkspaceBinding,
    WorkspaceOpenOverrides, WorkspaceOpenRequest,
};
pub use local_transport::LocalCoreTransport;
pub use viden_types::{
    AgentDagRecord, AgentDagStatus, AgentDagTaskSpec, AgentLaneRecord, AgentRole, AgentRoute,
    AgentTaskKind, AgentTaskRecord, AgentTaskStatus, ApprovalDecision, ApprovalDefaultAction,
    ApprovalRequestView, ApprovalResponse, ApprovalRisk, ApprovalScope, ApprovalTarget,
    CapabilityId, CommandAction, ContextBundleRecord, ContextOmittedSourceRecord,
    ContextSourceRecord, CoreHandshake, CostLedgerTotals, CostUsageRecord, CredentialHandle,
    CredentialRequestId, CredentialStatus, DataEgressPolicy, EventCursor, EvidenceView,
    ExecutionTarget, FRONTEND_SCHEMA_V1, GapRecovery, GateStrength, LaneBudget, LaneStatus,
    LocaleId, MergeGateRecord, MergeGateStatus, MutationPolicy, PermissionLevel, PermissionMode,
    ProjectConfigPreview, ProjectConfigState, ProjectProbe, ProviderHealthView, QueuedInputView,
    RecentProjectSummary, RecentSessionSummary, RecentWorkQuery, ReplayBatch, ReplayRequest,
    ResolvedUiPreferences, RuntimeCommand, RuntimeCommandEnvelope, RuntimeErrorView, RuntimeEvent,
    RuntimeEventEnvelope, RuntimeEventKind, RuntimeOwner, RuntimeSnapshot, RuntimeSnapshotEnvelope,
    RuntimeViewState, RuntimeWireEvent, SchemaVersion, StarterLanePreset, StarterLanePreview,
    StarterLanePreviewInvalidationReason, StarterLaneReceipt, StarterLaneRequest, TokenCostView,
    ToolCallView, TranscriptPage, TranscriptPageRequest, TranscriptRow, TranscriptRowId,
    TranscriptRowKind, TuiColorDepth, UiColorMode, UiDensity, UiMotion, UiPreferenceDiagnostic,
    UiPreferencePatch, UiPreferences, UiSkin, WorkMode,
};

/// Temporary compatibility imports for the pre-v3 TUI bootstrap.
///
/// Frontend clients must use [`CoreClient`] and must not import this module.
/// It can be removed after the legacy TUI has migrated to the frozen contract.
#[deprecated(note = "use CoreClient and protocol/view contracts from viden-core")]
pub mod legacy {
    pub use viden_provider::{
        ModelProvider, ModelRequestControl, ProviderAuthMode, ProviderDescriptor,
    };
    pub use viden_runtime::{EngineEvent, ProviderTelemetry, RuntimeSupervisor, SessionEngine};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_exports_runtime_contract_types() {
        assert!(std::any::type_name::<RuntimeEvent>().contains("RuntimeEvent"));
        assert!(std::any::type_name::<RuntimeCommand>().contains("RuntimeCommand"));
        assert!(std::any::type_name::<RuntimeViewState>().contains("RuntimeViewState"));
        assert!(
            std::any::type_name::<StatefulCoreClient<LocalCoreTransport>>()
                .contains("StatefulCoreClient")
        );
        assert!(std::any::type_name::<ApprovalRequestView>().contains("ApprovalRequestView"));
        assert!(std::any::type_name::<EvidenceView>().contains("EvidenceView"));
        assert!(std::any::type_name::<QueuedInputView>().contains("QueuedInputView"));
        assert!(std::any::type_name::<AgentLaneRecord>().contains("AgentLaneRecord"));
        assert!(std::any::type_name::<AgentTaskStatus>().contains("AgentTaskStatus"));
        assert!(std::any::type_name::<MergeGateStatus>().contains("MergeGateStatus"));
        assert!(std::any::type_name::<ResolvedUiPreferences>().contains("ResolvedUiPreferences"));
        assert!(std::any::type_name::<UiPreferencePatch>().contains("UiPreferencePatch"));
        assert!(std::any::type_name::<UiSkin>().contains("UiSkin"));
        assert!(std::any::type_name::<WorkMode>().contains("WorkMode"));
        assert!(std::any::type_name::<ProjectProbe>().contains("ProjectProbe"));
        assert!(std::any::type_name::<ProjectConfigPreview>().contains("ProjectConfigPreview"));
        assert!(std::any::type_name::<CredentialHandle>().contains("CredentialHandle"));
        assert!(std::any::type_name::<RecentWorkQuery>().contains("RecentWorkQuery"));
        assert!(std::any::type_name::<RecentProjectSummary>().contains("RecentProjectSummary"));
        assert!(std::any::type_name::<RecentSessionSummary>().contains("RecentSessionSummary"));
    }

    #[test]
    fn facade_exports_starter_lane_frontend_types() {
        let capability = CapabilityId("runtime.lane_lifecycle".to_string());
        let owner = RuntimeOwner {
            workspace_id: "workspace-test".to_string(),
            project_id: "project-test".to_string(),
            ..RuntimeOwner::default()
        };
        let handshake = CoreHandshake {
            core_version: "test-core".to_string(),
            supported_schema_versions: vec![FRONTEND_SCHEMA_V1],
            active_schema_version: FRONTEND_SCHEMA_V1,
            capabilities: std::collections::BTreeSet::from([capability]),
        };
        let request = StarterLaneRequest {
            lane_id: "lane-coder".to_string(),
            preset: StarterLanePreset::Coder,
            branch: Some("codex/lane-coder".to_string()),
            worktree_path: Some(".worktrees/lane-coder".to_string()),
        };
        let command = RuntimeCommand::PreviewStarterLane {
            request: request.clone(),
        };

        assert_eq!(handshake.active_schema_version, FRONTEND_SCHEMA_V1);
        assert_eq!(owner.workspace_id, "workspace-test");
        assert_eq!(command, RuntimeCommand::PreviewStarterLane { request });
        assert!(std::any::type_name::<StarterLanePreview>().contains("StarterLanePreview"));
        assert!(std::any::type_name::<StarterLaneReceipt>().contains("StarterLaneReceipt"));
        assert!(
            std::any::type_name::<StarterLanePreviewInvalidationReason>()
                .contains("StarterLanePreviewInvalidationReason")
        );
    }
}
