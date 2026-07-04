//! Viden GUI app scaffold.
//!
//! This crate is intentionally a contract consumer first. The future desktop
//! shell should render `RuntimeViewState` and send `RuntimeCommand` values
//! through `viden-core`, not call provider, tool, workflow, or permission
//! internals directly.

use viden_core::{RuntimeEvent, RuntimeViewState};

pub fn replay_runtime_events(events: &[RuntimeEvent]) -> Option<RuntimeViewState> {
    let first_snapshot = events.iter().find_map(|event| match &event.kind {
        viden_core::RuntimeEventKind::SnapshotUpdated { snapshot } => Some(snapshot.clone()),
        _ => None,
    })?;
    let mut state = RuntimeViewState::new(first_snapshot);
    for event in events {
        state.apply_event(event);
    }
    Some(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_runtime_contract_fixture() {
        let events: Vec<RuntimeEvent> = serde_json::from_str(include_str!(
            "../../../crates/types/tests/fixtures/runtime-contract-phase2.json"
        ))
        .expect("runtime fixture should parse");

        let state = replay_runtime_events(&events).expect("fixture should include snapshot");

        assert_eq!(state.snapshot.provider_family, "deepseek");
        assert_eq!(state.snapshot.model_label, "deepseek-reasoner");
        assert!(state.provider.is_some());
        assert!(state.token_cost.is_some());
        assert!(state.pending_approvals.is_empty());
        assert!(state.active_tool_calls.is_empty());
        assert_eq!(state.latest_evidence.len(), 1);
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.lanes.len(), 1);
    }
}
