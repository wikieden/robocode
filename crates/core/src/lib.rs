//! Viden core facade.
//!
//! This crate is the stable import boundary for runtime clients. It re-exports
//! the core runtime and contract types without introducing any TUI or GUI
//! dependency.

pub use viden_provider::{
    ModelProvider, ModelRequestControl, ProviderAuthMode, ProviderDescriptor,
};
pub use viden_runtime::{EngineEvent, ProviderTelemetry, RuntimeSupervisor, SessionEngine};
pub use viden_types::{
    ApprovalRequestView, ApprovalResponse, CommandAction, EvidenceView, ProviderHealthView,
    QueuedInputView, RuntimeCommand, RuntimeErrorView, RuntimeEvent, RuntimeEventKind,
    RuntimeSnapshot, RuntimeViewState, TokenCostView, ToolCallView,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_exports_runtime_contract_types() {
        assert!(std::any::type_name::<RuntimeSupervisor>().contains("RuntimeSupervisor"));
        assert!(std::any::type_name::<SessionEngine>().contains("SessionEngine"));
        assert!(std::any::type_name::<EngineEvent>().contains("EngineEvent"));
        assert!(std::any::type_name::<ProviderTelemetry>().contains("ProviderTelemetry"));
        assert!(std::any::type_name::<ModelRequestControl>().contains("ModelRequestControl"));
        assert!(std::any::type_name::<ProviderAuthMode>().contains("ProviderAuthMode"));
        assert!(std::any::type_name::<ProviderDescriptor>().contains("ProviderDescriptor"));
        assert!(std::any::type_name::<&dyn ModelProvider>().contains("ModelProvider"));
        assert!(std::any::type_name::<RuntimeEvent>().contains("RuntimeEvent"));
        assert!(std::any::type_name::<RuntimeCommand>().contains("RuntimeCommand"));
        assert!(std::any::type_name::<RuntimeViewState>().contains("RuntimeViewState"));
        assert!(std::any::type_name::<ApprovalRequestView>().contains("ApprovalRequestView"));
        assert!(std::any::type_name::<EvidenceView>().contains("EvidenceView"));
        assert!(std::any::type_name::<QueuedInputView>().contains("QueuedInputView"));
    }
}
