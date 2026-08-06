use std::sync::{Arc, Mutex};

use viden_core::{
    ApprovalRequestView, ApprovalScope, ContractDecision, ContractRecord, EvidenceView,
    ReviewRequestRecord, ReviewRequestStatus, RuntimeCommand, RuntimeCommandEnvelope, RuntimeOwner,
    RuntimeSnapshot, RuntimeViewState,
};
use viden_gui::{D2Intent, GuiCoreAdapter, PermissionChoice};

mod support;
use support::TestCoreClient;

const APPROVAL_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/approval-allow-deny.json"
);

fn owner(lane: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-viden".to_string(),
        project_id: "project-viden".to_string(),
        lane_id: Some(lane.to_string()),
        session_id: Some(format!("session-{lane}")),
        task_id: Some(format!("task-{lane}")),
        turn_id: None,
    }
}

/// Builds a decision queue that carries one fact per D2 group so the
/// projection cannot pass by reusing a single fact family.
fn decision_view() -> (RuntimeViewState, ApprovalRequestView) {
    let fixture: serde_json::Value = serde_json::from_str(APPROVAL_FIXTURE).unwrap();
    let snapshot: RuntimeSnapshot =
        serde_json::from_value(fixture["initial_snapshot"].clone()).unwrap();
    let mut view = RuntimeViewState::new(snapshot);

    let mut approval: ApprovalRequestView = serde_json::from_value(
        fixture["events"][0]["event"]["kind"]["payload"]["approval"].clone(),
    )
    .unwrap();
    approval.owner = owner("lane-gate");
    approval.allowed_scopes = vec![
        ApprovalScope::Once,
        ApprovalScope::Session {
            session_id: "session-lane-gate".to_string(),
        },
    ];
    view.pending_approvals.push(approval.clone());

    view.contracts.push(ContractRecord {
        contract_id: "contract-feel-v1-1".to_string(),
        task_id: "task-lane-contract".to_string(),
        owner: owner("lane-contract"),
        summary: "feel param ranges v1.0 -> v1.1".to_string(),
        decision: ContractDecision::Confirmed,
        audit_id: "audit-contract".to_string(),
        updated_at: 1_700_000_100,
    });

    view.review_requests.push(ReviewRequestRecord {
        review_id: "review-jump-feel".to_string(),
        gate_id: "gate-integration".to_string(),
        task_id: "task-lane-review".to_string(),
        requester_lane_id: "lane-review".to_string(),
        reviewer_lane_id: "lane-owner".to_string(),
        owner: owner("lane-review"),
        evidence_ids: vec!["evidence-playtest".to_string()],
        evidence_bindings: Vec::new(),
        status: ReviewRequestStatus::Pending,
        audit_id: "audit-review".to_string(),
        updated_at: 1_700_000_200,
    });

    view.latest_evidence.push(EvidenceView {
        id: "evidence-playtest".to_string(),
        kind: "playtest".to_string(),
        summary: "jump feel validated".to_string(),
        path: Some("evidence/playtest.json".to_string()),
        source: Some("lane-review".to_string()),
        canonical: None,
        metadata: None,
        timestamp: Some(1_700_000_150),
    });

    (view, approval)
}

fn connected(view: RuntimeViewState) -> (GuiCoreAdapter, Arc<Mutex<Vec<RuntimeCommandEnvelope>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = GuiCoreAdapter::new(Box::new(TestCoreClient::new(view, Arc::clone(&sent))));
    adapter.connect().unwrap();
    (adapter, sent)
}

#[test]
fn d2_projects_one_queue_from_gate_contract_and_review_facts() {
    let (view, approval) = decision_view();
    let (adapter, _) = connected(view);
    let projection = adapter.d2_decisions().expect("D2 projection");

    let gate = projection.group("gate").expect("gate group");
    let contract = projection.group("contract").expect("contract group");
    let review = projection.group("review").expect("review group");

    assert_eq!(gate.items.len(), 1, "one pending approval is one gate item");
    assert_eq!(gate.items[0].id, approval.id);
    assert_eq!(gate.items[0].lane_id.as_deref(), Some("lane-gate"));
    // The risk bucket is the fixture's Core fact, never a GUI heuristic.
    assert_eq!(gate.items[0].risk.as_deref(), Some("high"));

    assert_eq!(contract.items.len(), 1);
    assert_eq!(contract.items[0].id, "contract-feel-v1-1");
    assert_eq!(contract.items[0].lane_id.as_deref(), Some("lane-contract"));
    // ContractRecord carries a recorded decision and Core has no
    // "awaiting confirmation" contract fact, so the group must declare that
    // it is decided history rather than a human backlog.
    assert_eq!(contract.items[0].status, "confirmed");
    assert_eq!(
        contract.unavailable.as_ref().map(|entry| entry.code),
        Some("GUI-CORE-013")
    );

    assert_eq!(review.items.len(), 1);
    assert_eq!(review.items[0].id, "review-jump-feel");
    assert_eq!(review.items[0].lane_id.as_deref(), Some("lane-review"));
    assert_eq!(review.items[0].status, "pending");

    // The command bar count is the queue total of things actually awaiting a
    // human: pending approvals and pending reviews only.
    assert_eq!(projection.pending_total, 2);
}

#[test]
fn d2_selects_the_first_gate_item_and_renders_only_core_owned_context() {
    let (view, approval) = decision_view();
    let (adapter, _) = connected(view);
    let projection = adapter.d2_decisions().expect("D2 projection");
    let detail = projection.detail.expect("detail for the selected item");

    assert_eq!(detail.id, approval.id);
    assert_eq!(detail.kind, "gate");
    // The design shows a line-level diff. Core exposes only an opaque input
    // preview, so D2 renders the preview verbatim and declares the structured
    // diff unavailable instead of synthesizing diff rows.
    assert_eq!(detail.context.source, "approval_input_preview");
    assert_eq!(detail.context.text, approval.input_preview);
    assert!(
        detail.context.unavailable.is_some(),
        "structured diff must be declared unavailable, not invented"
    );
    assert_eq!(detail.audit_id, approval.audit_id);
}

#[test]
fn d2_gate_actions_come_from_core_allowed_scopes_and_send_respond_to_approval() {
    let (view, approval) = decision_view();
    let (mut adapter, sent) = connected(view);

    let projection = adapter.d2_decisions().expect("D2 projection");
    let detail = projection.detail.expect("detail");
    let kinds: Vec<&str> = detail
        .actions
        .iter()
        .filter(|action| action.available)
        .map(|action| action.kind.as_str())
        .collect();
    assert_eq!(kinds, vec!["once", "session", "deny"]);

    adapter
        .d2_send_intent(
            "gui-d2-test",
            D2Intent::RespondApproval {
                request_id: approval.id.clone(),
                choice: PermissionChoice::Once,
                feedback: None,
            },
        )
        .expect("respond to approval");

    let commands = sent.lock().unwrap();
    let last = commands.last().expect("one command was sent");
    assert!(
        matches!(
            &last.command,
            RuntimeCommand::RespondToApproval { request_id, .. } if request_id == &approval.id
        ),
        "gate decisions must travel as RespondToApproval, got {:?}",
        last.command
    );
}

#[test]
fn d2_contract_decision_sends_confirm_contract_with_the_core_owned_identity() {
    let (view, _) = decision_view();
    let (mut adapter, sent) = connected(view);

    adapter
        .d2_send_intent(
            "gui-d2-contract",
            D2Intent::DecideContract {
                contract_id: "contract-feel-v1-1".to_string(),
                accept: true,
            },
        )
        .expect("confirm contract");

    let commands = sent.lock().unwrap();
    let last = commands.last().expect("one command was sent");
    match &last.command {
        RuntimeCommand::ConfirmContract {
            contract_id,
            task_id,
            owner,
            decision,
            ..
        } => {
            assert_eq!(contract_id, "contract-feel-v1-1");
            // Identity is replayed from the Core record; the GUI never
            // reconstructs the owner from display strings.
            assert_eq!(task_id, "task-lane-contract");
            assert_eq!(owner.lane_id.as_deref(), Some("lane-contract"));
            assert_eq!(decision, &ContractDecision::Confirmed);
        }
        other => panic!("expected ConfirmContract, got {other:?}"),
    }
}

#[test]
fn d2_review_items_expose_evidence_but_declare_the_decision_action_unavailable() {
    let (view, _) = decision_view();
    let (adapter, _) = connected(view);
    let projection = adapter
        .d2_decisions_for("review-jump-feel")
        .expect("review detail");
    let detail = projection.detail.expect("detail");

    assert_eq!(detail.kind, "review");
    assert_eq!(detail.evidence.len(), 1);
    assert_eq!(detail.evidence[0].id, "evidence-playtest");
    assert_eq!(detail.evidence[0].summary, "jump feel validated");

    // frontend-contract-v1 has RequestReview but no review-decision command,
    // so the action must fail closed with its contract-request code.
    assert!(detail.actions.iter().all(|action| !action.available));
    assert!(
        detail
            .actions
            .iter()
            .any(|action| action.code == Some("GUI-CORE-011")),
        "the blocked review decision must name its contract request"
    );
}
