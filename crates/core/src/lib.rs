//! Viden core facade.
//!
//! This crate is the stable import boundary for runtime clients. It re-exports
//! the core runtime and contract types without introducing any TUI or GUI
//! dependency.

mod client;
mod compatibility;
mod local_transport;

pub use client::{CoreClient, CoreClientError, CoreTransport, StatefulCoreClient};
pub use compatibility::{
    CORE_CLIENT_CAPABILITIES, CORE_CLIENT_VERSION, frontend_capabilities, local_core_handshake,
    validate_handshake, validate_schema_version,
};
pub use local_transport::LocalCoreTransport;
pub use viden_types::{
    ApprovalRequestView, ApprovalResponse, CommandAction, CoreHandshake, EventCursor, EvidenceView,
    FRONTEND_SCHEMA_V1, GapRecovery, ProviderHealthView, QueuedInputView, ReplayBatch,
    ReplayRequest, RuntimeCommand, RuntimeCommandEnvelope, RuntimeErrorView, RuntimeEvent,
    RuntimeEventEnvelope, RuntimeEventKind, RuntimeSnapshot, RuntimeSnapshotEnvelope,
    RuntimeViewState, RuntimeWireEvent, SchemaVersion, TokenCostView, ToolCallView, TranscriptPage,
    TranscriptPageRequest,
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
    }
}
