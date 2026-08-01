use std::sync::{Arc, Mutex};

use viden_core::{
    ConflictBounce, ConflictBounceStatus, MergeGateStatus, RevertRecord, RuntimeOwner,
    RuntimeSnapshot, RuntimeViewState,
};
use viden_gui::GuiCoreAdapter;

mod support;
use support::TestCoreClient;

const MERGE_GATE_FIXTURE: &str =
    include_str!("../../../crates/types/tests/fixtures/frontend-contract-v1/merge-gate.json");

fn owner(lane: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-viden".to_string(),
        project_id: "project-boss-rush".to_string(),
        lane_id: Some(lane.to_string()),
        task_id: Some(format!("task-{lane}")),
        ..Default::default()
    }
}

fn gate_view() -> RuntimeViewState {
    #[derive(serde::Deserialize)]
    struct Fixture {
        initial_snapshot: RuntimeSnapshot,
        events: Vec<viden_core::RuntimeEventEnvelope>,
    }
    let fixture: Fixture = serde_json::from_str(MERGE_GATE_FIXTURE).unwrap();
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let viden_core::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    assert!(
        !view.merge_gates.is_empty(),
        "the merge-gate fixture must publish a gate"
    );
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
fn d12_projects_each_core_merge_gate_with_its_policy() {
    let view = gate_view();
    let expected: Vec<String> = view
        .merge_gates
        .iter()
        .map(|gate| gate.gate_id.clone())
        .collect();
    let projection = connected(view).d12_integration_gate().expect("D12");

    assert_eq!(
        projection
            .gates
            .iter()
            .map(|gate| gate.gate_id.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        projection.selected_gate_id.as_deref(),
        expected.first().map(String::as_str)
    );
}

#[test]
fn d12_accept_stays_unavailable_until_every_required_evidence_id_is_present() {
    let mut view = gate_view();
    view.merge_gates[0].status = MergeGateStatus::CollectingEvidence;
    view.merge_gates[0].required_evidence = vec!["replay-regression".to_string()];
    view.merge_gates[0].evidence_ids.clear();

    let blocked = connected(view.clone()).d12_integration_gate().unwrap();
    let detail = blocked.detail.expect("detail");
    let accept = detail
        .actions
        .iter()
        .find(|action| action.kind == "accept")
        .expect("accept action");
    assert!(!accept.available, "a strong gate cannot be bypassed");
    assert_eq!(
        detail.missing_evidence,
        vec!["replay-regression".to_string()]
    );
    // The client offers no manual-merge escape hatch.
    assert!(
        detail
            .actions
            .iter()
            .all(|action| action.kind == "accept" || action.kind == "reject")
    );

    view.merge_gates[0].evidence_ids = vec!["replay-regression".to_string()];
    let ready = connected(view).d12_integration_gate().unwrap();
    let ready_detail = ready.detail.expect("detail");
    assert!(ready_detail.missing_evidence.is_empty());
    assert!(
        ready_detail
            .actions
            .iter()
            .find(|action| action.kind == "accept")
            .unwrap()
            .available
    );
}

#[test]
fn d12_scopes_the_recovery_timeline_to_the_selected_gate() {
    let mut view = gate_view();
    let gate_id = view.merge_gates[0].gate_id.clone();
    view.conflict_bounces.push(ConflictBounce {
        bounce_id: "bounce-1".to_string(),
        gate_id: gate_id.clone(),
        task_id: "task-lane-3".to_string(),
        original_lane_id: "lane-3".to_string(),
        owner: owner("lane-3"),
        reason: "src/player/dash.gd conflicts with the merged baseline".to_string(),
        status: ConflictBounceStatus::Revalidated,
        evidence_ids: vec![],
        baseline_evidence: vec![],
        revalidation_evidence: vec![],
        audit_id: "audit-bounce-1".to_string(),
        created_at: 1_700_000_700,
        revalidated_at: Some(1_700_000_800),
    });
    view.conflict_bounces.push(ConflictBounce {
        bounce_id: "bounce-other".to_string(),
        gate_id: "gate-unrelated".to_string(),
        task_id: "task-lane-9".to_string(),
        original_lane_id: "lane-9".to_string(),
        owner: owner("lane-9"),
        reason: "unrelated".to_string(),
        status: ConflictBounceStatus::Pending,
        evidence_ids: vec![],
        baseline_evidence: vec![],
        revalidation_evidence: vec![],
        audit_id: "audit-bounce-other".to_string(),
        created_at: 1_700_000_710,
        revalidated_at: None,
    });

    let detail = connected(view)
        .d12_integration_gate()
        .unwrap()
        .detail
        .expect("detail");
    assert_eq!(
        detail.bounces.len(),
        1,
        "another gate's bounce must not leak"
    );
    assert_eq!(detail.bounces[0].bounce_id, "bounce-1");
    assert_eq!(detail.bounces[0].original_lane_id, "lane-3");
    assert_eq!(detail.bounces[0].status, "revalidated");
}

#[test]
fn d12_surfaces_the_post_merge_revert_for_its_own_gate() {
    let mut view = gate_view();
    let gate_id = view.merge_gates[0].gate_id.clone();
    view.merge_gates[0].status = MergeGateStatus::Merged;
    view.reverts.push(RevertRecord {
        revert_id: "revert-1".to_string(),
        gate_id,
        applied_change_id: "change-1".to_string(),
        owner: owner("lane-3"),
        reason: "cancel window regressed".to_string(),
        restored_paths: vec!["src/player/dash.gd".to_string()],
        audit_id: "audit-revert-1".to_string(),
        reverted_at: 1_700_000_900,
    });

    let detail = connected(view)
        .d12_integration_gate()
        .unwrap()
        .detail
        .expect("detail");
    assert_eq!(detail.reverts.len(), 1);
    assert_eq!(detail.reverts[0].revert_id, "revert-1");
    assert_eq!(
        detail.reverts[0].restored_paths,
        vec!["src/player/dash.gd".to_string()]
    );
    assert_eq!(detail.reverts[0].audit_id, "audit-revert-1");
}

#[test]
fn d12_declares_the_conflict_hunk_unavailable_instead_of_rendering_one() {
    let projection = connected(gate_view()).d12_integration_gate().unwrap();
    let entry = projection
        .unavailable
        .iter()
        .find(|entry| entry.code == "GUI-CORE-015")
        .expect("the conflict hunk gap must be declared");
    assert_eq!(entry.key, "d12.conflict.noStructuredHunk");
}
