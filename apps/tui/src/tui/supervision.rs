//! Pure supervision intent builders.
//!
//! Every function here returns a frozen `RuntimeCommand` and performs no
//! effect: no dispatch, no persistence, no local business state. Identities,
//! owners, evidence bindings, and hashes are always passed in from Core-owned
//! records so the TUI never manufactures an actor or a source hash from display
//! text. This mirrors `super::lane` for the lane lifecycle commands.

use viden_core::{
    ContractDecision, DependencyState, HandoffAcceptance, ReviewedEvidenceBinding, RuntimeCommand,
    RuntimeOwner,
};
use viden_types::ReviewVerdict;

pub(super) fn accept_merge_gate_intent(
    gate_id: impl Into<String>,
    actor: RuntimeOwner,
    reviewed_evidence: Vec<ReviewedEvidenceBinding>,
    decision: Option<String>,
) -> RuntimeCommand {
    RuntimeCommand::AcceptMergeGate {
        gate_id: gate_id.into(),
        actor,
        reviewed_evidence,
        decision,
    }
}

pub(super) fn reject_merge_gate_intent(
    gate_id: impl Into<String>,
    actor: RuntimeOwner,
    reason: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::RejectMergeGate {
        gate_id: gate_id.into(),
        actor,
        reason: reason.into(),
    }
}

/// Records the independent reviewer lane's verdict. `ReviewVerdict` is narrower
/// than `ReviewRequestStatus` on purpose: a decision can settle a review but
/// never reopen it.
pub(super) fn decide_review_intent(
    review_id: impl Into<String>,
    verdict: ReviewVerdict,
    feedback: Option<String>,
    actor: RuntimeOwner,
) -> RuntimeCommand {
    RuntimeCommand::DecideReview {
        review_id: review_id.into(),
        verdict,
        feedback,
        actor,
    }
}

#[allow(dead_code)]
pub(super) fn request_review_intent(
    review_id: impl Into<String>,
    gate_id: impl Into<String>,
    requester_lane_id: impl Into<String>,
    reviewer_lane_id: impl Into<String>,
    owner: RuntimeOwner,
    evidence_ids: Vec<String>,
) -> RuntimeCommand {
    RuntimeCommand::RequestReview {
        review_id: review_id.into(),
        gate_id: gate_id.into(),
        requester_lane_id: requester_lane_id.into(),
        reviewer_lane_id: reviewer_lane_id.into(),
        owner,
        evidence_ids,
    }
}

#[allow(dead_code)]
pub(super) fn create_handoff_intent(
    handoff_id: impl Into<String>,
    task_id: impl Into<String>,
    from_lane_id: impl Into<String>,
    to_lane_id: impl Into<String>,
    owner: RuntimeOwner,
    summary: impl Into<String>,
    acceptance: HandoffAcceptance,
) -> RuntimeCommand {
    RuntimeCommand::CreateHandoff {
        handoff_id: handoff_id.into(),
        task_id: task_id.into(),
        from_lane_id: from_lane_id.into(),
        to_lane_id: to_lane_id.into(),
        owner,
        summary: summary.into(),
        acceptance,
    }
}

#[allow(dead_code)]
pub(super) fn confirm_contract_intent(
    contract_id: impl Into<String>,
    task_id: impl Into<String>,
    owner: RuntimeOwner,
    summary: impl Into<String>,
    decision: ContractDecision,
) -> RuntimeCommand {
    RuntimeCommand::ConfirmContract {
        contract_id: contract_id.into(),
        task_id: task_id.into(),
        owner,
        summary: summary.into(),
        decision,
    }
}

#[allow(dead_code)]
pub(super) fn set_dependency_intent(
    dependency_id: impl Into<String>,
    task_id: impl Into<String>,
    depends_on_task_id: impl Into<String>,
    owner: RuntimeOwner,
    state: DependencyState,
    reason: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::SetDependency {
        dependency_id: dependency_id.into(),
        task_id: task_id.into(),
        depends_on_task_id: depends_on_task_id.into(),
        owner,
        state,
        reason: reason.into(),
    }
}

pub(super) fn bounce_merge_conflict_intent(
    gate_id: impl Into<String>,
    original_lane_id: impl Into<String>,
    owner: RuntimeOwner,
    reason: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::BounceMergeConflict {
        gate_id: gate_id.into(),
        original_lane_id: original_lane_id.into(),
        owner,
        reason: reason.into(),
    }
}

/// Revalidation carries exactly one reviewed-evidence binding replayed from
/// Core's bounce record; the caller never recomputes the source hash locally.
pub(super) fn revalidate_merge_conflict_intent(
    gate_id: impl Into<String>,
    bounce_id: impl Into<String>,
    actor: RuntimeOwner,
    evidence: ReviewedEvidenceBinding,
) -> RuntimeCommand {
    RuntimeCommand::RevalidateMergeConflict {
        gate_id: gate_id.into(),
        bounce_id: bounce_id.into(),
        actor,
        evidence,
    }
}

pub(super) fn revert_applied_change_intent(
    gate_id: impl Into<String>,
    owner: RuntimeOwner,
    reason: impl Into<String>,
) -> RuntimeCommand {
    RuntimeCommand::RevertAppliedChange {
        gate_id: gate_id.into(),
        owner,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> RuntimeOwner {
        RuntimeOwner {
            workspace_id: "workspace".to_string(),
            project_id: "project".to_string(),
            lane_id: Some("lane-a".to_string()),
            session_id: Some("session-a".to_string()),
            task_id: Some("task-1".to_string()),
            turn_id: Some("turn-1".to_string()),
        }
    }

    fn binding() -> ReviewedEvidenceBinding {
        ReviewedEvidenceBinding {
            evidence_id: "ev-1".to_string(),
            source_hash: "hash-ev-1".to_string(),
        }
    }

    #[test]
    fn merge_gate_builders_carry_the_actor_and_reviewed_evidence_verbatim() {
        assert_eq!(
            accept_merge_gate_intent(
                "gate-1",
                owner(),
                vec![binding()],
                Some("apply reviewed patch".to_string())
            ),
            RuntimeCommand::AcceptMergeGate {
                gate_id: "gate-1".to_string(),
                actor: owner(),
                reviewed_evidence: vec![binding()],
                decision: Some("apply reviewed patch".to_string()),
            }
        );
        assert_eq!(
            reject_merge_gate_intent("gate-1", owner(), "evidence missing"),
            RuntimeCommand::RejectMergeGate {
                gate_id: "gate-1".to_string(),
                actor: owner(),
                reason: "evidence missing".to_string(),
            }
        );
    }

    #[test]
    fn review_builders_separate_requesting_from_deciding() {
        assert_eq!(
            request_review_intent(
                "review-1",
                "gate-1",
                "lane-a",
                "lane-b",
                owner(),
                vec!["ev-1".to_string()]
            ),
            RuntimeCommand::RequestReview {
                review_id: "review-1".to_string(),
                gate_id: "gate-1".to_string(),
                requester_lane_id: "lane-a".to_string(),
                reviewer_lane_id: "lane-b".to_string(),
                owner: owner(),
                evidence_ids: vec!["ev-1".to_string()],
            }
        );
        assert_eq!(
            decide_review_intent(
                "review-1",
                ReviewVerdict::Rejected,
                Some("needs a regression test".to_string()),
                owner()
            ),
            RuntimeCommand::DecideReview {
                review_id: "review-1".to_string(),
                verdict: ReviewVerdict::Rejected,
                feedback: Some("needs a regression test".to_string()),
                actor: owner(),
            }
        );
    }

    #[test]
    fn handoff_contract_and_dependency_builders_return_frozen_core_commands() {
        assert_eq!(
            create_handoff_intent(
                "handoff-1",
                "task-1",
                "lane-a",
                "lane-b",
                owner(),
                "ready for review",
                HandoffAcceptance::Accepted
            ),
            RuntimeCommand::CreateHandoff {
                handoff_id: "handoff-1".to_string(),
                task_id: "task-1".to_string(),
                from_lane_id: "lane-a".to_string(),
                to_lane_id: "lane-b".to_string(),
                owner: owner(),
                summary: "ready for review".to_string(),
                acceptance: HandoffAcceptance::Accepted,
            }
        );
        assert_eq!(
            confirm_contract_intent(
                "contract-1",
                "task-1",
                owner(),
                "frontend contract v1",
                ContractDecision::Confirmed
            ),
            RuntimeCommand::ConfirmContract {
                contract_id: "contract-1".to_string(),
                task_id: "task-1".to_string(),
                owner: owner(),
                summary: "frontend contract v1".to_string(),
                decision: ContractDecision::Confirmed,
            }
        );
        assert_eq!(
            set_dependency_intent(
                "dep-1",
                "task-1",
                "task-0",
                owner(),
                DependencyState::Unblocked,
                "upstream landed"
            ),
            RuntimeCommand::SetDependency {
                dependency_id: "dep-1".to_string(),
                task_id: "task-1".to_string(),
                depends_on_task_id: "task-0".to_string(),
                owner: owner(),
                state: DependencyState::Unblocked,
                reason: "upstream landed".to_string(),
            }
        );
    }

    #[test]
    fn conflict_and_revert_builders_return_frozen_core_commands() {
        assert_eq!(
            bounce_merge_conflict_intent("gate-1", "lane-a", owner(), "base moved"),
            RuntimeCommand::BounceMergeConflict {
                gate_id: "gate-1".to_string(),
                original_lane_id: "lane-a".to_string(),
                owner: owner(),
                reason: "base moved".to_string(),
            }
        );
        assert_eq!(
            revalidate_merge_conflict_intent("gate-1", "bounce-1", owner(), binding()),
            RuntimeCommand::RevalidateMergeConflict {
                gate_id: "gate-1".to_string(),
                bounce_id: "bounce-1".to_string(),
                actor: owner(),
                evidence: binding(),
            }
        );
        assert_eq!(
            revert_applied_change_intent("gate-1", owner(), "regression in main"),
            RuntimeCommand::RevertAppliedChange {
                gate_id: "gate-1".to_string(),
                owner: owner(),
                reason: "regression in main".to_string(),
            }
        );
    }
}
