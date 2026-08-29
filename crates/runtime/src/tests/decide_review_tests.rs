//! `DecideReview`: the independent reviewer lane's verdict on a pending review.
//!
//! The review fact and the merge-gate decision are deliberately separate
//! commands. These tests pin the split: a review decision settles the review
//! (and stamps the validator on accept), while the gate decision stays the
//! operator's own permission-gated action and fails closed against a rejected
//! review.

use viden_types::{
    ApprovalResponse, AuditObjectRef, MergeGateStatus, ReviewRequestStatus, ReviewVerdict,
    ReviewedEvidenceBinding, RuntimeCommand, RuntimeEvent, RuntimeEventKind, RuntimeOwner,
};

use super::audit_runtime_tests::{
    all_audit_records, links, only_record, owner, record_canonical_patch, start_gate,
};
use super::{SequenceProvider, temp_dir};
use crate::SessionEngine;

const PATCH: &[u8] =
    b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
const REVISED_PATCH: &[u8] =
    b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+revised\n";

fn engine_for(name: &str) -> (std::path::PathBuf, SessionEngine) {
    let cwd = temp_dir(&format!("{name}_cwd"));
    let home = temp_dir(&format!("{name}_home"));
    let engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    (cwd, engine)
}

/// Builds a gate holding one canonical patch with a pending independent review.
fn gate_with_pending_review(
    cwd: &std::path::Path,
    engine: &mut SessionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    task_id: &str,
    review_id: &str,
) -> ReviewedEvidenceBinding {
    start_gate(engine, approver, task_id, vec!["patch".to_string()]);
    // The gate owner is the requester lane, which is what `RequestReview`
    // authorizes against.
    engine
        .handle_runtime_command(
            format!("handoff-{task_id}"),
            RuntimeCommand::CreateHandoff {
                handoff_id: format!("handoff-{task_id}"),
                task_id: task_id.to_string(),
                from_lane_id: "lane-planner".to_string(),
                to_lane_id: "lane-origin".to_string(),
                owner: owner("lane-origin", task_id),
                summary: "origin owns the patch".to_string(),
                acceptance: viden_types::HandoffAcceptance::Accepted,
            },
            approver,
        )
        .unwrap();
    let binding = record_canonical_patch(cwd, engine, approver, task_id, PATCH);
    engine
        .handle_runtime_command(
            format!("request-{review_id}"),
            RuntimeCommand::RequestReview {
                review_id: review_id.to_string(),
                gate_id: format!("gate-{task_id}"),
                requester_lane_id: "lane-origin".to_string(),
                reviewer_lane_id: "lane-reviewer".to_string(),
                owner: owner("lane-origin", task_id),
                evidence_ids: vec![binding.evidence_id.clone()],
            },
            approver,
        )
        .unwrap();
    binding
}

fn rejection_reason(events: &[RuntimeEvent]) -> String {
    events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::CommandRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a CommandRejected event, got {events:?}"))
}

fn review_updates(events: &[RuntimeEvent]) -> Vec<&viden_types::ReviewRequestRecord> {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::ReviewRequestUpdated { review } => Some(review),
            _ => None,
        })
        .collect()
}

#[test]
fn decide_review_accepts_a_pending_review_and_stamps_the_gate_validator() {
    let (cwd, mut engine) = engine_for("decide_review_accept");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-accept";
    gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-accept");

    let events = engine
        .handle_runtime_command(
            "decide-accept",
            RuntimeCommand::DecideReview {
                review_id: "review-accept".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: Some("canonical evidence matches the request".to_string()),
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let updated = review_updates(&events);
    assert_eq!(updated.len(), 1, "expected one ReviewRequestUpdated");
    assert_eq!(updated[0].status, ReviewRequestStatus::Accepted);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::MergeGateUpdated { .. })),
        "an accepted review must republish the gate it validated"
    );

    let view = engine.runtime_view_state();
    let review = &view.review_requests[0];
    assert_eq!(review.status, ReviewRequestStatus::Accepted);
    assert_eq!(
        review.feedback.as_deref(),
        Some("canonical evidence matches the request")
    );
    let validator = view.merge_gates[0]
        .validator
        .as_ref()
        .expect("gate keeps its independent validator");
    assert_eq!(validator.review_request_id, "review-accept");
    assert!(
        validator.validated_at.is_some(),
        "an accepted independent review stamps the validator"
    );

    let records = all_audit_records(&engine);
    let decided = only_record(&records, "review.decided");
    assert_eq!(decided.audit_id, review.audit_id);
    assert_eq!(
        decided.args.get("verdict").map(String::as_str),
        Some("accepted")
    );
    assert!(links(
        decided,
        AuditObjectRef::KIND_REVIEW_REQUEST,
        "review-accept"
    ));
    assert!(links(
        decided,
        AuditObjectRef::KIND_MERGE_GATE,
        &format!("gate-{task_id}")
    ));
    assert!(links(decided, AuditObjectRef::KIND_TASK, task_id));
    assert!(links(decided, AuditObjectRef::KIND_LANE, "lane-origin"));
    assert!(links(decided, AuditObjectRef::KIND_LANE, "lane-reviewer"));
    assert!(
        view.merge_gates[0].audit_ids.contains(&review.audit_id),
        "the gate timeline must join the review decision"
    );

    // Reviewer prose stays on the review fact; the audit timeline keeps stable
    // tokens only.
    let serialized = serde_json::to_string(&records).unwrap();
    assert!(!serialized.contains("canonical evidence matches the request"));
}

#[test]
fn decide_review_rejects_without_deciding_the_merge_gate() {
    let (cwd, mut engine) = engine_for("decide_review_reject");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-reject";
    gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-reject");
    let gate_status_before = engine.runtime_view_state().merge_gates[0].status;

    engine
        .handle_runtime_command(
            "decide-reject",
            RuntimeCommand::DecideReview {
                review_id: "review-reject".to_string(),
                verdict: ReviewVerdict::Rejected,
                feedback: Some("missing regression coverage".to_string()),
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let view = engine.runtime_view_state();
    assert_eq!(
        view.review_requests[0].status,
        ReviewRequestStatus::Rejected
    );
    assert_eq!(
        view.merge_gates[0].status, gate_status_before,
        "a review verdict must not decide the gate on its own"
    );
    assert!(
        view.merge_gates[0]
            .validator
            .as_ref()
            .expect("validator")
            .validated_at
            .is_none(),
        "a rejected review must never stamp the validator"
    );

    let records = all_audit_records(&engine);
    let decided = only_record(&records, "review.decided");
    assert_eq!(
        decided.args.get("verdict").map(String::as_str),
        Some("rejected")
    );
}

#[test]
fn decide_review_refuses_the_requester_lane() {
    let (cwd, mut engine) = engine_for("decide_review_requester");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-requester";
    gate_with_pending_review(
        &cwd,
        &mut engine,
        &mut approver,
        task_id,
        "review-requester",
    );

    let events = engine
        .handle_runtime_command(
            "decide-requester",
            RuntimeCommand::DecideReview {
                review_id: "review-requester".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: owner("lane-origin", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains("review decision requires the independent reviewer lane"),
        "got: {reason}"
    );
    assert_eq!(
        engine.runtime_view_state().review_requests[0].status,
        ReviewRequestStatus::Pending
    );
}

#[test]
fn decide_review_refuses_a_foreign_workspace_actor() {
    let (cwd, mut engine) = engine_for("decide_review_workspace");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-workspace";
    gate_with_pending_review(
        &cwd,
        &mut engine,
        &mut approver,
        task_id,
        "review-workspace",
    );

    let mut actor = owner("lane-reviewer", task_id);
    actor.workspace_id = "workspace-other".to_string();
    let events = engine
        .handle_runtime_command(
            "decide-workspace",
            RuntimeCommand::DecideReview {
                review_id: "review-workspace".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor,
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains("review decision actor does not match the review workspace scope"),
        "got: {reason}"
    );
    assert_eq!(
        engine.runtime_view_state().review_requests[0].status,
        ReviewRequestStatus::Pending
    );
}

#[test]
fn decide_review_refuses_a_default_actor() {
    let (cwd, mut engine) = engine_for("decide_review_default_actor");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-default-actor";
    gate_with_pending_review(
        &cwd,
        &mut engine,
        &mut approver,
        task_id,
        "review-default-actor",
    );

    let events = engine
        .handle_runtime_command(
            "decide-default-actor",
            RuntimeCommand::DecideReview {
                review_id: "review-default-actor".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: RuntimeOwner::default(),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains("review decision requires an actor"),
        "got: {reason}"
    );
}

#[test]
fn decide_review_refuses_an_already_decided_review() {
    let (cwd, mut engine) = engine_for("decide_review_twice");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-twice";
    gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-twice");

    engine
        .handle_runtime_command(
            "decide-once",
            RuntimeCommand::DecideReview {
                review_id: "review-twice".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();
    let events = engine
        .handle_runtime_command(
            "decide-again",
            RuntimeCommand::DecideReview {
                review_id: "review-twice".to_string(),
                verdict: ReviewVerdict::Rejected,
                feedback: None,
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains("review `review-twice` is already decided"),
        "got: {reason}"
    );
    assert_eq!(
        engine.runtime_view_state().review_requests[0].status,
        ReviewRequestStatus::Accepted,
        "a settled review is never reopened by a second decision"
    );
}

#[test]
fn decide_review_refuses_evidence_that_drifted_since_the_request() {
    let (cwd, mut engine) = engine_for("decide_review_drift");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-drift";
    let original =
        gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-drift");

    // Re-record the same evidence id with different canonical bytes: the gate
    // binding moves while the review still points at the reviewed receipt.
    let revised = record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, REVISED_PATCH);
    assert_ne!(original.source_hash, revised.source_hash);

    let events = engine
        .handle_runtime_command(
            "decide-drift",
            RuntimeCommand::DecideReview {
                review_id: "review-drift".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains("review evidence drifted since the request"),
        "got: {reason}"
    );
    assert_eq!(
        engine.runtime_view_state().review_requests[0].status,
        ReviewRequestStatus::Pending
    );
}

#[test]
fn accept_merge_gate_fails_closed_after_a_rejected_review_decision() {
    let (cwd, mut engine) = engine_for("decide_review_blocks_accept");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-blocks";
    let binding =
        gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-blocks");
    engine
        .handle_runtime_command(
            "decide-blocks",
            RuntimeCommand::DecideReview {
                review_id: "review-blocks".to_string(),
                verdict: ReviewVerdict::Rejected,
                feedback: Some("needs another pass".to_string()),
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "accept-after-reject",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-reviewer", task_id),
                reviewed_evidence: vec![binding],
                decision: Some("try to accept anyway".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains(&format!(
            "cannot accept merge gate `gate-{task_id}`: independent review `review-blocks` was rejected"
        )),
        "got: {reason}"
    );
    assert_ne!(
        engine.runtime_view_state().merge_gates[0].status,
        MergeGateStatus::Accepted
    );
}

#[test]
fn accept_merge_gate_still_settles_a_review_left_pending() {
    // Legacy path: a gate decided without an explicit review decision still
    // auto-flips its pending review, so existing operators keep working.
    let (cwd, mut engine) = engine_for("decide_review_legacy_autoflip");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-legacy";
    let binding =
        gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-legacy");

    engine
        .handle_runtime_command(
            "accept-legacy",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-reviewer", task_id),
                reviewed_evidence: vec![binding],
                decision: Some("reviewer accepted the patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    let view = engine.runtime_view_state();
    assert_eq!(view.merge_gates[0].status, MergeGateStatus::Accepted);
    assert_eq!(
        view.review_requests[0].status,
        ReviewRequestStatus::Accepted
    );
}

#[test]
fn accept_merge_gate_succeeds_after_an_accepted_review_decision() {
    let (cwd, mut engine) = engine_for("decide_review_then_accept_gate");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-then-accept";
    let binding = gate_with_pending_review(
        &cwd,
        &mut engine,
        &mut approver,
        task_id,
        "review-then-accept",
    );
    engine
        .handle_runtime_command(
            "decide-then-accept",
            RuntimeCommand::DecideReview {
                review_id: "review-then-accept".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "accept-after-decide",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-reviewer", task_id),
                reviewed_evidence: vec![binding],
                decision: Some("operator accepted the reviewed patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. })),
        "an accepted review must not block the gate decision: {events:?}"
    );

    let view = engine.runtime_view_state();
    assert_eq!(view.merge_gates[0].status, MergeGateStatus::Accepted);
    // The settled review keeps the verdict DecideReview recorded.
    assert_eq!(
        view.review_requests[0].status,
        ReviewRequestStatus::Accepted
    );
}

#[test]
fn decide_review_is_blocked_in_plan_mode_before_any_mutation() {
    let (cwd, mut engine) = engine_for("decide_review_plan_mode");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-plan";
    gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-plan");
    engine
        .process_input_with_approval("/mode plan", &mut approver)
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "decide-plan",
            RuntimeCommand::DecideReview {
                review_id: "review-plan".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(reason.contains("plan mode"), "got: {reason}");
    assert_eq!(
        engine.runtime_view_state().review_requests[0].status,
        ReviewRequestStatus::Pending
    );
    assert!(
        !all_audit_records(&engine)
            .iter()
            .any(|record| record.action == "review.decided")
    );
}

#[test]
fn decide_review_fails_when_the_audit_append_fails() {
    let (cwd, mut engine) = engine_for("decide_review_append_failure");
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-decide-append";
    gate_with_pending_review(&cwd, &mut engine, &mut approver, task_id, "review-append");

    engine.fail_next_workflow_append_for_test();
    let events = engine
        .handle_runtime_command(
            "decide-append",
            RuntimeCommand::DecideReview {
                review_id: "review-append".to_string(),
                verdict: ReviewVerdict::Accepted,
                feedback: None,
                actor: owner("lane-reviewer", task_id),
            },
            &mut approver,
        )
        .unwrap();

    let reason = rejection_reason(&events);
    assert!(
        reason.contains("injected workflow append failure"),
        "got: {reason}"
    );
    assert!(
        review_updates(&events).is_empty(),
        "a failed audit append must not publish the review verdict"
    );
    let view = engine.runtime_view_state();
    assert_eq!(view.review_requests[0].status, ReviewRequestStatus::Pending);
    assert_eq!(view.review_requests[0].feedback, None);
    assert!(
        view.merge_gates[0]
            .validator
            .as_ref()
            .expect("validator")
            .validated_at
            .is_none()
    );
    assert!(
        !all_audit_records(&engine)
            .iter()
            .any(|record| record.action == "review.decided")
    );
}
