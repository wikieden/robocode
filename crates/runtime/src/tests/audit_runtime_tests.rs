//! Runtime emission and read-back for the append-only audit timeline.
//!
//! Every accepted trust mutation must leave exactly one durable audit record
//! that reuses the audit id already stored on the trust fact, so an operator
//! can join the timeline to the gate/lane/task facts without guessing.

use std::fs;

use viden_context::{ContextEngine, ContextPutRequest};
use viden_types::{
    AgentDagTaskSpec, AgentRole, ApprovalResponse, AuditObjectRef, AuditQuery, AuditRecord,
    CanonicalEvidenceReference, ContextContentKind, ContextScope, ContractDecision,
    DependencyState, EvidenceProducer, EvidenceQualityFacts, EvidenceQualityStatus,
    EvidenceVerificationState, HandoffAcceptance, ReviewedEvidenceBinding, RuntimeCommand,
    RuntimeEventKind, RuntimeOwner, WorkMode,
};

use super::{SequenceProvider, temp_dir};
use crate::SessionEngine;

pub(super) fn owner(lane_id: &str, task_id: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-1".to_string(),
        project_id: "project-1".to_string(),
        lane_id: Some(lane_id.to_string()),
        session_id: Some(format!("session-{lane_id}")),
        task_id: Some(task_id.to_string()),
        turn_id: None,
    }
}

pub(super) fn start_gate(
    engine: &mut SessionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    task_id: &str,
    required_evidence: Vec<String>,
) {
    engine
        .handle_runtime_command(
            format!("start-{task_id}"),
            RuntimeCommand::StartAgentDag {
                goal: format!("audit timeline for {task_id}"),
                tasks: vec![AgentDagTaskSpec {
                    task_id: task_id.to_string(),
                    role: AgentRole::Coder,
                    title: "Audited task".to_string(),
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
        .unwrap();
}

pub(super) fn record_canonical_patch(
    cwd: &std::path::Path,
    engine: &mut SessionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    task_id: &str,
    patch: &[u8],
) -> ReviewedEvidenceBinding {
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
    let binding = ReviewedEvidenceBinding {
        evidence_id: evidence_id.clone(),
        source_hash: canonical.source_hash.clone(),
    };
    engine.set_merge_gate_context_facts_for_test(&bundle_id, stored.item);
    engine
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
    binding
}

pub(super) fn all_audit_records(engine: &SessionEngine) -> Vec<AuditRecord> {
    engine
        .workflow_store()
        .query_audit(&AuditQuery {
            project_id: None,
            lane_id: None,
            object: None,
            before: None,
            limit: 500,
        })
        .unwrap()
        .records
}

pub(super) fn only_record<'a>(records: &'a [AuditRecord], action: &str) -> &'a AuditRecord {
    let matching = records
        .iter()
        .filter(|record| record.action == action)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one `{action}` audit record, got {}",
        matching.len()
    );
    matching[0]
}

pub(super) fn links(record: &AuditRecord, kind: &str, id: &str) -> bool {
    record
        .objects
        .iter()
        .any(|object| object.kind == kind && object.id == id)
}

#[test]
fn audit_runtime_records_handoff_review_contract_and_dependency_trust_facts() {
    let cwd = temp_dir("audit_runtime_trust_facts_cwd");
    let home = temp_dir("audit_runtime_trust_facts_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-audited";
    start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );
    start_gate(
        &mut engine,
        &mut approver,
        "task-blocker",
        vec!["patch".to_string()],
    );

    engine
        .handle_runtime_command(
            "handoff",
            RuntimeCommand::CreateHandoff {
                handoff_id: "handoff-1".to_string(),
                task_id: task_id.to_string(),
                from_lane_id: "lane-planner".to_string(),
                to_lane_id: "lane-origin".to_string(),
                owner: owner("lane-origin", task_id),
                summary: "planner hands accepted scope to origin".to_string(),
                acceptance: HandoffAcceptance::Accepted,
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "contract",
            RuntimeCommand::ConfirmContract {
                contract_id: "contract-1".to_string(),
                task_id: task_id.to_string(),
                owner: owner("lane-origin", task_id),
                summary: "scope confirmed".to_string(),
                decision: ContractDecision::Confirmed,
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "dependency",
            RuntimeCommand::SetDependency {
                dependency_id: "dependency-1".to_string(),
                task_id: task_id.to_string(),
                depends_on_task_id: "task-blocker".to_string(),
                owner: owner("lane-origin", task_id),
                state: DependencyState::Blocked,
                reason: "blocked on upstream task".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let patch = b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
    let binding = record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, patch);
    engine
        .handle_runtime_command(
            "review",
            RuntimeCommand::RequestReview {
                review_id: "review-1".to_string(),
                gate_id: format!("gate-{task_id}"),
                requester_lane_id: "lane-origin".to_string(),
                reviewer_lane_id: "lane-reviewer".to_string(),
                owner: owner("lane-origin", task_id),
                evidence_ids: vec![binding.evidence_id.clone()],
            },
            &mut approver,
        )
        .unwrap();

    let view = engine.runtime_view_state();
    let records = all_audit_records(&engine);

    let handoff = only_record(&records, "handoff.created");
    assert_eq!(handoff.audit_id, view.handoffs[0].audit_id);
    assert!(links(handoff, AuditObjectRef::KIND_HANDOFF, "handoff-1"));
    assert!(links(handoff, AuditObjectRef::KIND_TASK, task_id));
    assert!(links(handoff, AuditObjectRef::KIND_LANE, "lane-origin"));
    assert!(links(handoff, AuditObjectRef::KIND_LANE, "lane-planner"));

    let contract = only_record(&records, "contract.confirmed");
    assert_eq!(contract.audit_id, view.contracts[0].audit_id);
    assert!(links(contract, AuditObjectRef::KIND_CONTRACT, "contract-1"));
    assert!(links(contract, AuditObjectRef::KIND_TASK, task_id));

    let dependency = only_record(&records, "dependency.set");
    assert_eq!(dependency.audit_id, view.dependencies[0].audit_id);
    assert!(links(
        dependency,
        AuditObjectRef::KIND_DEPENDENCY,
        "dependency-1"
    ));
    assert!(links(dependency, AuditObjectRef::KIND_TASK, "task-blocker"));

    let review = only_record(&records, "review.requested");
    assert_eq!(review.audit_id, view.review_requests[0].audit_id);
    assert!(links(
        review,
        AuditObjectRef::KIND_REVIEW_REQUEST,
        "review-1"
    ));
    assert!(links(
        review,
        AuditObjectRef::KIND_MERGE_GATE,
        &format!("gate-{task_id}")
    ));
    assert!(links(
        review,
        AuditObjectRef::KIND_EVIDENCE,
        &binding.evidence_id
    ));

    // The AwaitingEvidence stamp `RequestReview` writes onto the gate is
    // evidence-collection bookkeeping, not an operator decision, so it is
    // deliberately unaudited: `gate.decided` is reserved for accept/reject.
    assert!(
        !records.iter().any(|record| record.action == "gate.decided"),
        "auto evidence-collection stamps must not be audited as gate decisions"
    );

    // Audit arguments stay stable keys, never the free-text trust summaries.
    let serialized = serde_json::to_string(&records).unwrap();
    assert!(!serialized.contains("planner hands accepted scope"));
    assert!(!serialized.contains("blocked on upstream task"));
}

#[test]
fn audit_runtime_records_conflict_bounce_and_revert_with_trust_audit_ids() {
    let cwd = temp_dir("audit_runtime_conflict_revert_cwd");
    let home = temp_dir("audit_runtime_conflict_revert_home");
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
    let patch = b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
    let original = record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, patch);
    engine
        .handle_runtime_command(
            "review-conflict",
            RuntimeCommand::RequestReview {
                review_id: "review-conflict".to_string(),
                gate_id: format!("gate-{task_id}"),
                requester_lane_id: "lane-origin".to_string(),
                reviewer_lane_id: "lane-reviewer".to_string(),
                owner: owner("lane-origin", task_id),
                evidence_ids: vec![original.evidence_id.clone()],
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "accept-before-conflict",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-reviewer", task_id),
                reviewed_evidence: vec![original.clone()],
                decision: Some("independent reviewer accepted initial patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    // The first apply conflicts and bounces back to the origin lane.
    engine
        .handle_runtime_command(
            "merge-conflict",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-origin", task_id),
                decision: Some("apply reviewed patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    let pending = engine.runtime_view_state().merge_gates[0]
        .conflict
        .clone()
        .expect("conflict bounce should be durable");

    fs::write(cwd.join("src/lib.rs"), "old\n").unwrap();
    let revised_patch = b"diff --git a/src/lib.rs b/src/lib.rs\nindex 1111111..2222222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
    let revised = record_canonical_patch(&cwd, &mut engine, &mut approver, task_id, revised_patch);
    engine
        .handle_runtime_command(
            "revalidate-conflict",
            RuntimeCommand::RevalidateMergeConflict {
                gate_id: format!("gate-{task_id}"),
                bounce_id: pending.bounce_id.clone(),
                actor: owner("lane-origin", task_id),
                evidence: revised.clone(),
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "review-revalidated-conflict",
            RuntimeCommand::RequestReview {
                review_id: "review-conflict-2".to_string(),
                gate_id: format!("gate-{task_id}"),
                requester_lane_id: "lane-origin".to_string(),
                reviewer_lane_id: "lane-reviewer".to_string(),
                owner: owner("lane-origin", task_id),
                evidence_ids: vec![revised.evidence_id.clone()],
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "accept-revalidated",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-reviewer", task_id),
                reviewed_evidence: vec![revised],
                decision: Some("independent reviewer accepted revalidation".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "merge-revalidated",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                actor: owner("lane-origin", task_id),
                decision: Some("apply revalidated patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    engine
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

    let view = engine.runtime_view_state();
    let records = all_audit_records(&engine);

    // The origin lane's permission-gated revalidation claim is its own fact.
    let revalidated = only_record(&records, "conflict.revalidated");
    assert!(links(
        revalidated,
        AuditObjectRef::KIND_CONFLICT,
        &pending.bounce_id
    ));
    assert!(links(
        revalidated,
        AuditObjectRef::KIND_MERGE_GATE,
        &format!("gate-{task_id}")
    ));
    assert!(links(revalidated, AuditObjectRef::KIND_TASK, task_id));

    let bounced = only_record(&records, "conflict.bounced");
    assert_eq!(bounced.audit_id, view.conflict_bounces[0].audit_id);
    assert!(links(
        bounced,
        AuditObjectRef::KIND_MERGE_GATE,
        &format!("gate-{task_id}")
    ));
    assert!(links(bounced, AuditObjectRef::KIND_LANE, "lane-origin"));
    assert!(links(bounced, AuditObjectRef::KIND_TASK, task_id));

    let reverted = only_record(&records, "change.reverted");
    assert_eq!(reverted.audit_id, view.reverts[0].audit_id);
    assert!(links(
        reverted,
        AuditObjectRef::KIND_REVERT,
        &view.reverts[0].revert_id
    ));
    assert!(links(
        reverted,
        AuditObjectRef::KIND_APPLIED_CHANGE,
        &view.reverts[0].applied_change_id
    ));

    // The lane filter joins owner lanes and linked lane objects.
    let lane_page = engine
        .workflow_store()
        .query_audit(&AuditQuery {
            project_id: None,
            lane_id: Some("lane-origin".to_string()),
            object: None,
            before: None,
            limit: 500,
        })
        .unwrap();
    assert!(
        lane_page
            .records
            .iter()
            .any(|record| record.action == "conflict.bounced")
    );
}

#[test]
fn audit_runtime_query_command_round_trips_the_page_in_plan_mode() {
    let cwd = temp_dir("audit_runtime_query_cwd");
    let home = temp_dir("audit_runtime_query_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-query";
    start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );
    engine
        .handle_runtime_command(
            "handoff-query",
            RuntimeCommand::CreateHandoff {
                handoff_id: "handoff-query".to_string(),
                task_id: task_id.to_string(),
                from_lane_id: "lane-planner".to_string(),
                to_lane_id: "lane-origin".to_string(),
                owner: owner("lane-origin", task_id),
                summary: "planner hands scope to origin".to_string(),
                acceptance: HandoffAcceptance::Accepted,
            },
            &mut approver,
        )
        .unwrap();

    // The read path is read-only: it must answer identically in Plan mode and
    // must never reach the approval prompt.
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let mut denier = |_prompt| panic!("read-only audit query must not request approval");
    let events = engine
        .handle_runtime_command(
            "audit-query",
            RuntimeCommand::QueryAudit {
                // An out-of-range limit is clamped, not rejected.
                query: AuditQuery {
                    project_id: None,
                    lane_id: None,
                    object: None,
                    before: None,
                    limit: u32::MAX,
                },
            },
            &mut denier,
        )
        .unwrap();

    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0].kind,
        RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "audit-query"
    ));
    let RuntimeEventKind::AuditPageLoaded { page } = &events[1].kind else {
        panic!("expected AuditPageLoaded, got {:?}", events[1].kind);
    };
    assert!(page.complete);
    assert_eq!(page.next_before, None);
    assert!(
        page.records
            .iter()
            .any(|record| record.action == "handoff.created")
    );
}

#[test]
fn audit_runtime_append_failure_fails_the_trust_action_without_publishing_it() {
    let cwd = temp_dir("audit_runtime_append_failure_cwd");
    let home = temp_dir("audit_runtime_append_failure_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-append-failure";
    start_gate(
        &mut engine,
        &mut approver,
        task_id,
        vec!["patch".to_string()],
    );

    engine.fail_next_workflow_append_for_test();
    let rejected = engine
        .handle_runtime_command(
            "contract-append-failure",
            RuntimeCommand::ConfirmContract {
                contract_id: "contract-append-failure".to_string(),
                task_id: task_id.to_string(),
                owner: owner("lane-origin", task_id),
                summary: "scope confirmed".to_string(),
                decision: ContractDecision::Confirmed,
            },
            &mut approver,
        )
        .unwrap();

    assert!(rejected.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("injected workflow append failure")
    )));
    assert!(
        !rejected
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ContractUpdated { .. })),
        "a failed audit append must not publish the trust fact"
    );
    assert!(engine.runtime_view_state().contracts.is_empty());
    assert!(all_audit_records(&engine).is_empty());
}

/// Sets up a gate holding one canonical patch, ready for an operator decision.
fn gate_with_canonical_patch(
    cwd: &std::path::Path,
    engine: &mut SessionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    task_id: &str,
) -> ReviewedEvidenceBinding {
    start_gate(engine, approver, task_id, vec!["patch".to_string()]);
    let patch = b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+merged\n";
    record_canonical_patch(cwd, engine, approver, task_id, patch)
}

#[test]
fn audit_runtime_records_the_operator_merge_gate_acceptance() {
    let cwd = temp_dir("audit_runtime_accept_gate_cwd");
    let home = temp_dir("audit_runtime_accept_gate_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-accept";
    let binding = gate_with_canonical_patch(&cwd, &mut engine, &mut approver, task_id);
    let actor = engine.runtime_view_state().merge_gates[0].owner.clone();

    engine
        .handle_runtime_command(
            "accept-gate",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: actor.clone(),
                reviewed_evidence: vec![binding],
                decision: Some("operator accepted the reviewed patch".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    let view = engine.runtime_view_state();
    let records = all_audit_records(&engine);
    let decided = only_record(&records, "gate.decided");
    assert_eq!(
        decided.audit_id,
        view.merge_gates[0]
            .decision
            .as_ref()
            .expect("gate decision")
            .audit_id
    );
    assert!(links(
        decided,
        AuditObjectRef::KIND_MERGE_GATE,
        &format!("gate-{task_id}")
    ));
    assert!(links(decided, AuditObjectRef::KIND_TASK, task_id));
    assert_eq!(
        decided.args.get("outcome").map(String::as_str),
        Some("accepted")
    );
    // The operator's free-text decision never reaches the timeline.
    assert!(
        !serde_json::to_string(&records)
            .unwrap()
            .contains("operator accepted the reviewed patch")
    );
}

#[test]
fn audit_runtime_records_the_operator_merge_gate_rejection() {
    let cwd = temp_dir("audit_runtime_reject_gate_cwd");
    let home = temp_dir("audit_runtime_reject_gate_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-reject";
    gate_with_canonical_patch(&cwd, &mut engine, &mut approver, task_id);
    let actor = engine.runtime_view_state().merge_gates[0].owner.clone();

    engine
        .handle_runtime_command(
            "reject-gate",
            RuntimeCommand::RejectMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor,
                reason: "operator wants a different approach".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let view = engine.runtime_view_state();
    let records = all_audit_records(&engine);
    let decided = only_record(&records, "gate.decided");
    assert_eq!(
        decided.audit_id,
        view.merge_gates[0]
            .decision
            .as_ref()
            .expect("gate decision")
            .audit_id
    );
    assert_eq!(
        decided.args.get("outcome").map(String::as_str),
        Some("needs_changes")
    );
    assert!(
        !serde_json::to_string(&records)
            .unwrap()
            .contains("operator wants a different approach")
    );
}

#[test]
fn audit_runtime_records_a_rejected_agent_artifact() {
    let cwd = temp_dir("audit_runtime_reject_artifact_cwd");
    let home = temp_dir("audit_runtime_reject_artifact_home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-reject-artifact";
    let binding = gate_with_canonical_patch(&cwd, &mut engine, &mut approver, task_id);
    let actor = engine.runtime_view_state().merge_gates[0].owner.clone();

    engine
        .handle_runtime_command(
            "reject-artifact",
            RuntimeCommand::RejectAgentArtifact {
                gate_id: format!("gate-{task_id}"),
                evidence_id: binding.evidence_id.clone(),
                actor,
                reason: "artifact does not match the objective".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let view = engine.runtime_view_state();
    let records = all_audit_records(&engine);
    let rejected = only_record(&records, "evidence.rejected");
    assert_eq!(
        rejected.audit_id,
        view.merge_gates[0]
            .decision
            .as_ref()
            .expect("gate decision")
            .audit_id
    );
    assert!(links(
        rejected,
        AuditObjectRef::KIND_EVIDENCE,
        &binding.evidence_id
    ));
    assert!(links(
        rejected,
        AuditObjectRef::KIND_MERGE_GATE,
        &format!("gate-{task_id}")
    ));
    assert!(links(rejected, AuditObjectRef::KIND_TASK, task_id));
    assert_eq!(
        rejected.args.get("outcome").map(String::as_str),
        Some("rejected")
    );
}

#[test]
fn audit_runtime_revert_append_failure_leaves_the_applied_bytes_untouched() {
    let cwd = temp_dir("audit_runtime_revert_untouched_cwd");
    let home = temp_dir("audit_runtime_revert_untouched_home");
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/lib.rs"), "old\n").unwrap();
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-revert-untouched";
    let binding = gate_with_canonical_patch(&cwd, &mut engine, &mut approver, task_id);
    let actor = engine.runtime_view_state().merge_gates[0].owner.clone();
    engine
        .handle_runtime_command(
            "accept-before-revert",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: actor.clone(),
                reviewed_evidence: vec![binding],
                decision: Some("accept before revert".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "merge-before-revert",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                actor,
                decision: Some("merge before revert".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    let applied_bytes = fs::read(cwd.join("src/lib.rs")).unwrap();
    assert_eq!(applied_bytes, b"merged\n");

    // The audit append is the first durable write in the revert, so a failure
    // must abort before any byte is restored.
    engine.fail_next_workflow_append_for_test();
    let rejected = engine
        .handle_runtime_command(
            "revert-audit-append-failure",
            RuntimeCommand::RevertAppliedChange {
                gate_id: format!("gate-{task_id}"),
                owner: owner("lane-reviewer", task_id),
                reason: "force audit append failure".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(rejected.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("injected workflow append failure")
    )));
    assert!(
        !rejected
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::RevertRecorded { .. })),
        "a failed audit append must not publish a revert"
    );
    // The decisive assertion: no silent, untraceable disk mutation.
    assert_eq!(fs::read(cwd.join("src/lib.rs")).unwrap(), applied_bytes);
    assert!(engine.runtime_view_state().reverts.is_empty());
    assert!(
        !all_audit_records(&engine)
            .iter()
            .any(|record| record.action == "change.reverted")
    );
}

#[test]
fn audit_runtime_revert_records_the_audit_fact_before_touching_bytes() {
    let cwd = temp_dir("audit_runtime_revert_order_cwd");
    let home = temp_dir("audit_runtime_revert_order_home");
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/lib.rs"), "old\n").unwrap();
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let task_id = "task-revert-order";
    let binding = gate_with_canonical_patch(&cwd, &mut engine, &mut approver, task_id);
    let actor = engine.runtime_view_state().merge_gates[0].owner.clone();
    engine
        .handle_runtime_command(
            "accept-before-order",
            RuntimeCommand::AcceptMergeGate {
                gate_id: format!("gate-{task_id}"),
                actor: actor.clone(),
                reviewed_evidence: vec![binding],
                decision: Some("accept before ordering probe".to_string()),
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "merge-before-order",
            RuntimeCommand::MergeAgentPatch {
                gate_id: format!("gate-{task_id}"),
                actor,
                decision: Some("merge before ordering probe".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    // Let the FIRST durable write succeed and fail the second. This pins the
    // audit-before-mutation ordering: the surviving `change.reverted` record
    // proves the audit fact landed before `restore_file_rollbacks` ran. Under
    // the reverse ordering the first append would be the precommit and the
    // audit record would never exist, leaving a byte change with no trail.
    engine.fail_after_workflow_appends_for_test(1);
    let rejected = engine
        .handle_runtime_command(
            "revert-order-probe",
            RuntimeCommand::RevertAppliedChange {
                gate_id: format!("gate-{task_id}"),
                owner: owner("lane-reviewer", task_id),
                reason: "force failure after the audit fact".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(rejected.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("injected workflow append failure")
    )));
    let records = all_audit_records(&engine);
    assert!(
        records
            .iter()
            .any(|record| record.action == "change.reverted"),
        "the audit fact must be durable before any byte is restored"
    );
    // The revert itself did not complete, so the record is deliberately
    // orphaned: detectable evidence of the attempt rather than a silent change.
    assert!(engine.runtime_view_state().reverts.is_empty());
}
