use std::sync::{Arc, Mutex};

use viden_core::{
    AgentTaskStatus, RuntimeErrorView, RuntimeEvent, RuntimeEventKind, RuntimeSnapshot,
    RuntimeViewState,
};
use viden_gui::{D6State, GuiCoreAdapter};

mod support;
use support::TestCoreClient;

const D1_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

fn d1_view() -> RuntimeViewState {
    #[derive(serde::Deserialize)]
    struct Fixture {
        initial_snapshot: RuntimeSnapshot,
        events: Vec<viden_core::RuntimeEventEnvelope>,
    }
    let fixture: Fixture = serde_json::from_str(D1_FIXTURE).unwrap();
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let viden_core::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    view
}

fn connected(view: RuntimeViewState) -> GuiCoreAdapter {
    let mut adapter = GuiCoreAdapter::new(Box::new(TestCoreClient::new(
        view,
        Arc::new(Mutex::new(Vec::new())),
    )));
    adapter.connect().unwrap();
    adapter
}

#[test]
fn d6_projects_empty_provider_agent_context_and_queue_clear_from_core_facts() {
    let mut empty = d1_view();
    empty.lanes.clear();
    empty.tasks.clear();
    empty.pending_approvals.clear();
    assert_eq!(connected(empty).d6_recovery().state, D6State::Empty);

    let mut provider = d1_view();
    let provider_error: RuntimeEventKind = serde_json::from_value(serde_json::json!({
        "type": "provider_health_updated",
        "payload": {
            "provider": {
                "provider_id": "deepseek",
                "model": "deepseek-v4-flash",
                "status": "error",
                "request_count": 2,
                "error_count": 1,
                "last_latency_ms": 300,
                "average_latency_ms": 240,
                "tokens_per_second": 20
            }
        }
    }))
    .unwrap();
    provider.apply_event(&RuntimeEvent::new(98, provider_error));
    provider.errors.push(RuntimeErrorView {
        message: "provider unavailable".into(),
        recoverable: true,
        hint: Some("select another provider".into()),
    });
    let projection = connected(provider).d6_recovery();
    assert_eq!(projection.state, D6State::ProviderError);
    assert_eq!(projection.detail.as_deref(), Some("provider unavailable"));

    let mut stopped = d1_view();
    stopped.tasks[0].status = AgentTaskStatus::Failed;
    let projection = connected(stopped).d6_recovery();
    assert_eq!(projection.state, D6State::AgentStopped);
    assert!(
        projection
            .actions
            .iter()
            .filter(|action| action.kind != "inspect")
            .all(|action| !action.available)
    );
    assert!(
        projection
            .actions
            .iter()
            .any(|action| action.code == "GUI-CORE-003")
    );

    let mut overflow = d1_view();
    let budget: RuntimeEventKind = serde_json::from_value(serde_json::json!({
        "type": "context_budget_exceeded",
        "payload": {
            "budget": {
                "budget_id": "ctx-hard",
                "scope": { "type": "task", "id": "task-core" },
                "soft_token_limit": 8000,
                "hard_token_limit": 10000,
                "used_tokens": 10001,
                "remaining_tokens": 0,
                "exceeded": true,
                "updated_at": 1
            }
        }
    }))
    .unwrap();
    overflow.apply_event(&RuntimeEvent::new(99, budget));
    let projection = connected(overflow).d6_recovery();
    assert_eq!(projection.state, D6State::ContextOverflow);
    assert_eq!(projection.used_tokens, Some(10_001));
    assert_eq!(projection.hard_token_limit, Some(10_000));

    let mut clear = d1_view();
    clear.pending_approvals.clear();
    clear.merge_gates.clear();
    clear.errors.clear();
    clear.context_budgets.clear();
    clear
        .tasks
        .retain(|task| task.status != AgentTaskStatus::Failed);
    assert_eq!(
        connected(clear).d6_recovery().state,
        D6State::GateQueueClear
    );

    let mut open_gate = d1_view();
    open_gate.pending_approvals.clear();
    open_gate.merge_gates.push(
        serde_json::from_value(serde_json::json!({
            "gate_id": "gate-open",
            "task_id": "task-core",
            "status": "collecting_evidence",
            "required_evidence": ["test_result"],
            "evidence_ids": []
        }))
        .unwrap(),
    );
    assert_ne!(
        connected(open_gate).d6_recovery().state,
        D6State::GateQueueClear
    );
}

#[test]
fn d6_recovery_never_claims_restart_close_or_checkpoint_success_without_core_receipts() {
    let mut stopped = d1_view();
    stopped.tasks[0].status = AgentTaskStatus::Failed;
    let projection = connected(stopped).d6_recovery();
    for kind in ["restart", "close_lane", "checkpoint"] {
        let action = projection
            .actions
            .iter()
            .find(|action| action.kind == kind)
            .unwrap();
        assert!(!action.available);
        assert_eq!(action.code, "GUI-CORE-003");
    }
}
