use std::fs;
use std::time::{Duration, Instant};

use viden_context::{ContextEngine, ContextPutRequest};
use viden_types::{
    AgentDagTaskSpec, AgentRole, ApprovalResponse, CanonicalEvidenceReference,
    ConflictBounceStatus, ContextContentKind, ContextScope, ContractDecision, DependencyState,
    EvidenceProducer, EvidenceQualityFacts, EvidenceQualityStatus, EvidenceVerificationState,
    HandoffAcceptance, MergeGateDecisionOutcome, MergeGateStatus, RuntimeCommand, RuntimeEvent,
    RuntimeEventKind, RuntimeOwner, RuntimeViewState,
};
use viden_workflows::stores::WorkflowStore;

use super::{SequenceProvider, temp_dir};
use crate::{RuntimeSupervisor, SessionEngine};

fn owner(lane_id: &str, task_id: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-1".to_string(),
        project_id: "project-1".to_string(),
        lane_id: Some(lane_id.to_string()),
        session_id: Some(format!("session-{lane_id}")),
        task_id: Some(task_id.to_string()),
        turn_id: None,
    }
}

fn start_gate(
    engine: &mut SessionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    task_id: &str,
    required_evidence: Vec<String>,
) -> Vec<RuntimeEvent> {
    engine
        .handle_runtime_command(
            format!("start-{task_id}"),
            RuntimeCommand::StartAgentDag {
                goal: format!("trust loop for {task_id}"),
                tasks: vec![AgentDagTaskSpec {
                    task_id: task_id.to_string(),
                    role: AgentRole::Coder,
                    title: "Trust-loop task".to_string(),
                    objective: "produce canonical evidence".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence,
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
            approver,
        )
        .unwrap()
}

#[test]
fn trust_loop_cross_lane_records_preserve_owner_and_dependency_state_through_replay() {
    let cwd = temp_dir("trust_loop_cross_lane_records_cwd");
    let home = temp_dir("trust_loop_cross_lane_records_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home.clone()),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-cross-lane";
    let mut events = start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );

    let handoff_events = engine
        .handle_runtime_command(
            "handoff",
            RuntimeCommand::CreateHandoff {
                handoff_id: "handoff-1".to_string(),
                task_id: task_id.to_string(),
                from_lane_id: "lane-planner".to_string(),
                to_lane_id: "lane-coder".to_string(),
                owner: owner("lane-coder", task_id),
                summary: "planner hands accepted scope to coder".to_string(),
                acceptance: HandoffAcceptance::Accepted,
            },
            &mut approver,
        )
        .unwrap();
    assert!(handoff_events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::MergeGateUpdated { gate }
            if gate.owner == owner("lane-coder", task_id)
    )));
    events.extend(handoff_events);
    let rejected_handoff = engine
        .handle_runtime_command(
            "handoff-rejected",
            RuntimeCommand::CreateHandoff {
                handoff_id: "handoff-rejected".to_string(),
                task_id: task_id.to_string(),
                from_lane_id: "lane-coder".to_string(),
                to_lane_id: "lane-docs".to_string(),
                owner: owner("lane-docs", task_id),
                summary: "docs lane rejected incomplete scope".to_string(),
                acceptance: HandoffAcceptance::Rejected,
            },
            &mut approver,
        )
        .unwrap();
    assert!(rejected_handoff.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::HandoffUpdated { handoff }
            if handoff.acceptance == HandoffAcceptance::Rejected
    )));
    assert!(rejected_handoff.iter().all(|event| !matches!(
        &event.kind,
        RuntimeEventKind::MergeGateUpdated { gate }
            if gate.owner == owner("lane-docs", task_id)
    )));
    events.extend(rejected_handoff);

    for (command_id, command) in [
        (
            "review",
            RuntimeCommand::RequestReview {
                review_id: "review-1".to_string(),
                gate_id: format!("gate-{task_id}"),
                requester_lane_id: "lane-coder".to_string(),
                reviewer_lane_id: "lane-reviewer".to_string(),
                owner: owner("lane-reviewer", task_id),
                evidence_ids: Vec::new(),
            },
        ),
        (
            "contract",
            RuntimeCommand::ConfirmContract {
                contract_id: "contract-1".to_string(),
                task_id: task_id.to_string(),
                owner: owner("lane-reviewer", task_id),
                summary: "src scope and canonical patch evidence".to_string(),
                decision: ContractDecision::Confirmed,
            },
        ),
        (
            "block",
            RuntimeCommand::SetDependency {
                dependency_id: "dependency-1".to_string(),
                task_id: task_id.to_string(),
                depends_on_task_id: "task-contract".to_string(),
                owner: owner("lane-coder", task_id),
                state: DependencyState::Blocked,
                reason: "contract confirmation pending".to_string(),
            },
        ),
        (
            "unblock",
            RuntimeCommand::SetDependency {
                dependency_id: "dependency-1".to_string(),
                task_id: task_id.to_string(),
                depends_on_task_id: "task-contract".to_string(),
                owner: owner("lane-coder", task_id),
                state: DependencyState::Unblocked,
                reason: "contract confirmed".to_string(),
            },
        ),
    ] {
        events.extend(
            engine
                .handle_runtime_command(command_id, command, &mut approver)
                .unwrap(),
        );
    }

    let live = engine.runtime_view_state();
    assert_eq!(live.handoffs.len(), 2);
    assert_eq!(live.handoffs[0].acceptance, HandoffAcceptance::Accepted);
    assert_eq!(live.handoffs[0].owner, owner("lane-coder", task_id));
    assert_eq!(live.review_requests.len(), 1);
    assert_eq!(live.review_requests[0].reviewer_lane_id, "lane-reviewer");
    assert_eq!(live.contracts[0].decision, ContractDecision::Confirmed);
    assert_eq!(live.dependencies[0].state, DependencyState::Unblocked);
    assert!(live.handoffs[0].audit_id.starts_with("audit"));
    assert!(live.review_requests[0].audit_id.starts_with("audit"));

    let mut replayed = RuntimeViewState::new(live.snapshot.clone());
    for event in &events {
        replayed.apply_event(event);
    }
    assert_eq!(replayed.handoffs, live.handoffs);
    assert_eq!(replayed.review_requests, live.review_requests);
    assert_eq!(replayed.contracts, live.contracts);
    assert_eq!(replayed.dependencies, live.dependencies);
    assert_eq!(replayed.merge_gates, live.merge_gates);
}

#[test]
fn trust_loop_summary_only_evidence_never_produces_typed_acceptance() {
    let cwd = temp_dir("trust_loop_summary_only_cwd");
    let home = temp_dir("trust_loop_summary_only_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-summary-only";
    start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );

    let recorded = engine
        .handle_runtime_command(
            "summary-only",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: format!("gate-{task_id}"),
                evidence_id: Some("summary-only-patch".to_string()),
                kind: "patch".to_string(),
                summary: "diff looked good in chat".to_string(),
                path: None,
                source: Some("summary".to_string()),
                canonical: None,
            },
            &mut approver,
        )
        .unwrap();
    assert!(recorded.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::MergeGateUpdated { gate }
            if gate.status == MergeGateStatus::CollectingEvidence
                && gate.decision.as_ref().is_some_and(|decision| {
                    decision.outcome == MergeGateDecisionOutcome::AwaitingEvidence
                })
    )));

    let accepted = engine
        .handle_runtime_command(
            "accept-summary-only",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                decision: Some("summary is sufficient".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    assert!(accepted.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("missing_canonical")
    )));
    assert_ne!(
        engine.runtime_view_state().merge_gates[0]
            .decision
            .as_ref()
            .map(|decision| decision.outcome),
        Some(MergeGateDecisionOutcome::Accepted)
    );
}

#[test]
fn trust_loop_conflict_bounces_to_origin_then_revalidates_merges_and_reverts() {
    let cwd = temp_dir("trust_loop_conflict_revert_cwd");
    let home = temp_dir("trust_loop_conflict_revert_home");
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/lib.rs"), "current\n").unwrap();
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-conflict";
    start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );
    engine
        .handle_runtime_command(
            "handoff-conflict",
            RuntimeCommand::CreateHandoff {
                handoff_id: "handoff-conflict".to_string(),
                task_id: task_id.to_string(),
                from_lane_id: "lane-planner".to_string(),
                to_lane_id: "lane-origin".to_string(),
                owner: owner("lane-origin", task_id),
                summary: "origin owns the patch".to_string(),
                acceptance: HandoffAcceptance::Accepted,
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "review-conflict",
            RuntimeCommand::RequestReview {
                review_id: "review-conflict".to_string(),
                gate_id: format!("gate-{task_id}"),
                requester_lane_id: "lane-origin".to_string(),
                reviewer_lane_id: "lane-reviewer".to_string(),
                owner: owner("lane-reviewer", task_id),
                evidence_ids: Vec::new(),
            },
            &mut approver,
        )
        .unwrap();
    let patch = b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
    record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, patch);
    let pending_review = engine.runtime_view_state().merge_gates[0].clone();
    assert_eq!(pending_review.status, MergeGateStatus::CollectingEvidence);
    assert!(pending_review.decision.as_ref().is_some_and(|decision| {
        decision.outcome == MergeGateDecisionOutcome::AwaitingEvidence
            && decision.reason == "independent_review_required"
    }));
    engine
        .handle_runtime_command(
            "accept-before-conflict",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                decision: Some("independent reviewer accepted initial patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    let conflicted = engine
        .handle_runtime_command(
            "merge-conflict",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                decision: Some("apply reviewed patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    assert!(conflicted.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::MergeConflictBounced { conflict }
            if conflict.original_lane_id == "lane-origin"
                && conflict.status == ConflictBounceStatus::Pending
    )));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "current\n"
    );

    fs::write(cwd.join("src/lib.rs"), "old\n").unwrap();
    record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, patch);
    let gate = engine.runtime_view_state().merge_gates[0].clone();
    assert_eq!(
        gate.conflict.as_ref().map(|conflict| conflict.status),
        Some(ConflictBounceStatus::Revalidated)
    );

    let accepted = engine
        .handle_runtime_command(
            "accept-revalidated",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                decision: Some("independent reviewer accepted revalidation".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    assert!(accepted.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ReviewRequestUpdated { review }
            if review.review_id == "review-conflict"
                && review.status == viden_types::ReviewRequestStatus::Accepted
                && review.evidence_ids.iter().any(|id| id == "evidence-task-conflict")
    )));
    assert_eq!(
        engine.runtime_view_state().review_requests[0].status,
        viden_types::ReviewRequestStatus::Accepted
    );
    let merged = engine
        .handle_runtime_command(
            "merge-revalidated",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                decision: Some("apply revalidated patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    assert!(merged.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::MergeGateUpdated { gate }
            if gate.status == MergeGateStatus::Merged
                && gate.applied_change_id.is_some()
                && gate.decision.as_ref().is_some_and(|decision| {
                    decision.outcome == MergeGateDecisionOutcome::Merged
                })
    )));
    assert_eq!(
        engine.runtime_view_state().conflict_bounces[0].status,
        ConflictBounceStatus::Resolved
    );
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "merged\n"
    );

    let reverted = engine
        .handle_runtime_command(
            "revert-applied",
            RuntimeCommand::RevertAppliedChange {
                gate_id: format!("gate-{task_id}"),
                owner: owner("lane-reviewer", task_id),
                reason: "post-merge verification failed".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(reverted.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::RevertRecorded { revert }
            if revert.owner == owner("lane-reviewer", task_id)
                && revert.audit_id.starts_with("audit")
    )));
    assert_eq!(fs::read_to_string(cwd.join("src/lib.rs")).unwrap(), "old\n");
    assert_eq!(
        engine.runtime_view_state().merge_gates[0].status,
        MergeGateStatus::Reverted
    );
}

#[test]
fn trust_loop_revert_audit_failure_restores_applied_bytes_and_facts() {
    let cwd = temp_dir("trust_loop_revert_audit_failure_cwd");
    let home = temp_dir("trust_loop_revert_audit_failure_home");
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/lib.rs"), "old\n").unwrap();
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home.clone()),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-revert-failure";
    start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );
    let patch = b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
    record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, patch);
    engine
        .handle_runtime_command(
            "merge-before-revert-failure",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                decision: Some("merge before revert".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    let before = engine.runtime_view_state();
    // The revert intent precommit succeeds, then the final durable fact fails
    // after bytes have changed so compensation must restore both domains.
    engine.fail_after_workflow_appends_for_test(1);

    let rejected = engine
        .handle_runtime_command(
            "revert-audit-failure",
            RuntimeCommand::RevertAppliedChange {
                gate_id: format!("gate-{task_id}"),
                owner: owner("lane-reviewer", task_id),
                reason: "force workflow failure".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(rejected.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("injected workflow append failure")
    )));
    assert!(rejected.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::Error { error }
            if error.recoverable
                && error.hint.as_deref().is_some_and(|hint| hint.contains("restored"))
    )));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "merged\n"
    );
    assert_eq!(engine.runtime_view_state().merge_gates, before.merge_gates);
    assert_eq!(engine.runtime_view_state().reverts, before.reverts);
    let workflow_events = WorkflowStore::new(home, &cwd)
        .unwrap()
        .load_agent_events()
        .unwrap();
    assert!(workflow_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event
                .payload
                .get("runtime_events_json")
                .is_some_and(|json| json.contains("audit-revert-precommit"))
    }));
}

#[test]
fn trust_loop_supervisor_resumes_approved_mutation_with_the_original_owner() {
    let cwd = temp_dir("trust_loop_supervisor_approval_cwd");
    let home = temp_dir("trust_loop_supervisor_approval_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let task_id = "task-supervised-trust";
    start_gate(
        &mut engine,
        &mut |_prompt| ApprovalResponse::allow_once(None),
        task_id,
        vec!["review".to_string()],
    );
    let supervisor = RuntimeSupervisor::start(engine);
    let command_owner = owner("lane-reviewer", task_id);
    supervisor
        .send_command_from_owner(
            command_owner.clone(),
            "record-supervised-review",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: format!("gate-{task_id}"),
                evidence_id: Some("evidence-supervised-review".to_string()),
                kind: "review".to_string(),
                summary: "review completed with canonical evidence pending".to_string(),
                path: None,
                source: Some("lane-reviewer".to_string()),
                canonical: None,
            },
        )
        .unwrap();
    let requested = collect_supervisor_events(&supervisor, |events| {
        events.iter().any(|event| {
            matches!(
                &event.event,
                viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let request_id = requested
        .iter()
        .find_map(|event| match &event.event {
            viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) if approval.owner == command_owner => Some(approval.id.clone()),
            _ => None,
        })
        .expect("trust mutation should request approval for the submitting owner");

    supervisor
        .send_command_from_owner(
            command_owner.clone(),
            "approve-supervised-review",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let resumed = collect_supervisor_events(&supervisor, |events| {
        events.iter().any(|event| {
            matches!(
                &event.event,
                viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::EvidenceRecorded { evidence },
                    ..
                }) if evidence.id == "evidence-supervised-review"
            )
        })
    });
    assert!(resumed.iter().any(|event| {
        event.owner == command_owner
            && matches!(
                &event.event,
                viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::EvidenceRecorded { evidence },
                    ..
                }) if evidence.id == "evidence-supervised-review"
            )
    }));

    supervisor
        .send_command_from_owner(
            command_owner.clone(),
            "deny-supervised-contract",
            RuntimeCommand::ConfirmContract {
                contract_id: "contract-supervised-denied".to_string(),
                task_id: task_id.to_string(),
                owner: command_owner.clone(),
                summary: "this contract must not survive denial".to_string(),
                decision: ContractDecision::Confirmed,
            },
        )
        .unwrap();
    let requested = collect_supervisor_events(&supervisor, |events| {
        events.iter().any(|event| {
            matches!(
                &event.event,
                viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let request_id = requested
        .iter()
        .find_map(|event| match &event.event {
            viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .expect("contract mutation should request approval");
    supervisor
        .send_command_from_owner(
            command_owner,
            "reject-supervised-contract",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::deny(Some("contract rejected".to_string())),
            },
        )
        .unwrap();
    collect_supervisor_events(&supervisor, |events| {
        events.iter().any(|event| {
            matches!(
                &event.event,
                viden_types::RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalResolved {
                        decision: viden_types::ApprovalDecision::Deny,
                        ..
                    },
                    ..
                })
            )
        })
    });
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .contracts
            .is_empty()
    );
}

fn collect_supervisor_events(
    supervisor: &RuntimeSupervisor,
    done: impl Fn(&[viden_types::RuntimeEventEnvelope]) -> bool,
) -> Vec<viden_types::RuntimeEventEnvelope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        if let Some(event) = supervisor.recv_event_envelope_timeout(Duration::from_millis(25)) {
            events.push(event);
            if done(&events) {
                return events;
            }
        }
    }
    panic!("timed out waiting for supervised trust-loop events: {events:#?}");
}

fn record_canonical_patch(
    cwd: &std::path::Path,
    engine: &mut SessionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    task_id: &str,
    patch: &[u8],
) {
    let evidence_id = format!("evidence-{task_id}");
    let bundle_id = format!("bundle-{task_id}");
    let mut store = ContextEngine::open(cwd.join(".viden/context-engine")).unwrap();
    let stored = store
        .store(ContextPutRequest {
            scope: ContextScope::Task(task_id.to_string()),
            kind: ContextContentKind::Diff,
            content: patch,
            evidence_id: Some(evidence_id.clone()),
        })
        .unwrap();
    let canonical = CanonicalEvidenceReference {
        item_id: stored.item.item_id.clone(),
        bundle_id: bundle_id.clone(),
        source_hash: stored.item.content_sha256.clone(),
        producer: EvidenceProducer {
            identity: "lane-origin".to_string(),
            role: "coder".to_string(),
            task_id: task_id.to_string(),
        },
        permission_snapshot_id: Some(format!("permission-{task_id}")),
        permission_scope: ContextScope::Task(task_id.to_string()),
        evidence_scope: ContextScope::Task(task_id.to_string()),
        verification: EvidenceVerificationState::Verified,
        quality: EvidenceQualityFacts {
            status: EvidenceQualityStatus::Pass,
            reason_codes: Vec::new(),
        },
    };
    engine.set_merge_gate_context_facts_for_test(&bundle_id, stored.item);
    let events = engine
        .handle_runtime_command(
            format!("record-{evidence_id}"),
            RuntimeCommand::RecordAgentEvidence {
                gate_id: format!("gate-{task_id}"),
                evidence_id: Some(evidence_id),
                kind: "patch".to_string(),
                summary: "canonical patch".to_string(),
                path: None,
                source: Some("lane-origin".to_string()),
                canonical: Some(canonical),
            },
            approver,
        )
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::MergeGateUpdated { .. }))
    );
}
