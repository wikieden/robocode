use viden_types::{
    AgentNextAction, AgentTaskStatus, ApprovalResponse, ConflictBounce, ConflictBounceStatus,
    ContractDecision, ContractRecord, DependencyRecord, DependencyState, HandoffAcceptance,
    HandoffRecord, MergeGateDecision, MergeGateDecisionOutcome, MergeGateValidator, RevertRecord,
    ReviewRequestRecord, ReviewRequestStatus, RuntimeEvent, RuntimeEventKind, RuntimeOwner,
    fresh_id, now_timestamp, truncate_for_preview,
};

use crate::SessionEngine;

impl SessionEngine {
    pub(crate) fn trust_mutation_permission_descriptor(
        &self,
        command: &viden_types::RuntimeCommand,
    ) -> Result<Option<(&'static str, String)>, String> {
        let descriptor = match command {
            viden_types::RuntimeCommand::CreateHandoff {
                handoff_id,
                task_id,
                from_lane_id,
                to_lane_id,
                owner,
                ..
            } => {
                validate_trust_id("handoff_id", handoff_id)?;
                validate_trust_id("task_id", task_id)?;
                validate_trust_id("from_lane_id", from_lane_id)?;
                validate_trust_id("to_lane_id", to_lane_id)?;
                validate_owner(owner, Some(to_lane_id), task_id)?;
                (
                    "create_handoff",
                    format!("task={task_id} from={from_lane_id} to={to_lane_id}"),
                )
            }
            viden_types::RuntimeCommand::RequestReview {
                review_id,
                gate_id,
                requester_lane_id,
                reviewer_lane_id,
                owner,
                evidence_ids,
            } => {
                validate_trust_id("review_id", review_id)?;
                validate_trust_id("gate_id", gate_id)?;
                validate_trust_id("requester_lane_id", requester_lane_id)?;
                validate_trust_id("reviewer_lane_id", reviewer_lane_id)?;
                let gate_index = self.require_merge_gate_index(gate_id)?;
                validate_review_requester(
                    &self.runtime_merge_gates[gate_index],
                    requester_lane_id,
                )?;
                validate_owner(
                    owner,
                    Some(reviewer_lane_id),
                    &self.runtime_merge_gates[gate_index].task_id,
                )?;
                for evidence_id in evidence_ids {
                    validate_trust_id("evidence_id", evidence_id)?;
                    if !self
                        .runtime_evidence
                        .iter()
                        .any(|evidence| evidence.id == *evidence_id)
                    {
                        return Err(format!("review evidence `{evidence_id}` does not exist"));
                    }
                }
                (
                    "request_review",
                    format!("gate={gate_id} reviewer={reviewer_lane_id}"),
                )
            }
            viden_types::RuntimeCommand::ConfirmContract {
                contract_id,
                task_id,
                owner,
                ..
            } => {
                validate_trust_id("contract_id", contract_id)?;
                validate_trust_id("task_id", task_id)?;
                validate_owner(owner, owner.lane_id.as_deref(), task_id)?;
                (
                    "confirm_contract",
                    format!("task={task_id} contract={contract_id}"),
                )
            }
            viden_types::RuntimeCommand::SetDependency {
                dependency_id,
                task_id,
                depends_on_task_id,
                owner,
                state,
                ..
            } => {
                validate_trust_id("dependency_id", dependency_id)?;
                validate_trust_id("task_id", task_id)?;
                validate_trust_id("depends_on_task_id", depends_on_task_id)?;
                validate_owner(owner, owner.lane_id.as_deref(), task_id)?;
                (
                    "set_dependency",
                    format!("task={task_id} dependency={depends_on_task_id} state={state:?}"),
                )
            }
            viden_types::RuntimeCommand::AcceptMergeGate { gate_id, .. } => {
                self.require_merge_gate_index(gate_id)?;
                ("accept_merge_gate", format!("gate={gate_id}"))
            }
            viden_types::RuntimeCommand::RejectMergeGate { gate_id, .. } => {
                self.require_merge_gate_index(gate_id)?;
                ("reject_merge_gate", format!("gate={gate_id}"))
            }
            viden_types::RuntimeCommand::RecordAgentEvidence { gate_id, kind, .. } => {
                self.require_merge_gate_index(gate_id)?;
                if kind.trim().is_empty() {
                    return Err("agent evidence kind cannot be empty".to_string());
                }
                (
                    "record_agent_evidence",
                    format!("gate={gate_id} kind={}", kind.trim()),
                )
            }
            viden_types::RuntimeCommand::AcceptAgentArtifact {
                gate_id,
                evidence_id,
                ..
            } => {
                self.require_merge_gate_index(gate_id)?;
                validate_trust_id("evidence_id", evidence_id)?;
                if !self
                    .runtime_evidence
                    .iter()
                    .any(|evidence| evidence.id == *evidence_id)
                {
                    return Err(format!(
                        "agent artifact evidence `{evidence_id}` does not exist"
                    ));
                }
                (
                    "accept_agent_artifact",
                    format!("gate={gate_id} evidence={evidence_id}"),
                )
            }
            viden_types::RuntimeCommand::RejectAgentArtifact {
                gate_id,
                evidence_id,
                ..
            } => {
                self.require_merge_gate_index(gate_id)?;
                validate_trust_id("evidence_id", evidence_id)?;
                if !self
                    .runtime_evidence
                    .iter()
                    .any(|evidence| evidence.id == *evidence_id)
                {
                    return Err(format!(
                        "agent artifact evidence `{evidence_id}` does not exist"
                    ));
                }
                (
                    "reject_agent_artifact",
                    format!("gate={gate_id} evidence={evidence_id}"),
                )
            }
            viden_types::RuntimeCommand::MergeAgentPatch { gate_id, .. } => {
                self.require_merge_gate_index(gate_id)?;
                ("merge_agent_patch", format!("gate={gate_id}"))
            }
            viden_types::RuntimeCommand::BounceMergeConflict {
                gate_id,
                original_lane_id,
                owner,
                ..
            } => {
                let gate_index = self.require_merge_gate_index(gate_id)?;
                validate_trust_id("original_lane_id", original_lane_id)?;
                validate_owner(
                    owner,
                    owner.lane_id.as_deref(),
                    &self.runtime_merge_gates[gate_index].task_id,
                )?;
                (
                    "bounce_merge_conflict",
                    format!("gate={gate_id} origin={original_lane_id}"),
                )
            }
            viden_types::RuntimeCommand::RevertAppliedChange { gate_id, owner, .. } => {
                let gate_index = self.require_merge_gate_index(gate_id)?;
                validate_owner(
                    owner,
                    owner.lane_id.as_deref(),
                    &self.runtime_merge_gates[gate_index].task_id,
                )?;
                let change_id = self.runtime_merge_gates[gate_index]
                    .applied_change_id
                    .as_deref()
                    .ok_or_else(|| {
                        format!("merge gate `{gate_id}` is missing applied change identity")
                    })?;
                (
                    "revert_applied_change",
                    format!("gate={gate_id} change={change_id}"),
                )
            }
            _ => return Ok(None),
        };
        Ok(Some(descriptor))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_handoff<F>(
        &mut self,
        handoff_id: String,
        task_id: String,
        from_lane_id: String,
        to_lane_id: String,
        owner: RuntimeOwner,
        summary: String,
        acceptance: HandoffAcceptance,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        validate_trust_id("handoff_id", &handoff_id)?;
        validate_trust_id("task_id", &task_id)?;
        validate_trust_id("from_lane_id", &from_lane_id)?;
        validate_trust_id("to_lane_id", &to_lane_id)?;
        validate_owner(&owner, Some(&to_lane_id), &task_id)?;
        if from_lane_id == to_lane_id {
            return Err("handoff requires distinct source and destination lanes".to_string());
        }
        self.require_runtime_task(&task_id)?;
        let summary = validate_trust_text("handoff summary", summary, 500)?;
        ensure_unique(
            self.runtime_handoffs
                .iter()
                .any(|record| record.handoff_id == handoff_id),
            "handoff",
            &handoff_id,
        )?;
        self.require_trust_permission(
            "create_handoff",
            &format!("task={task_id} from={from_lane_id} to={to_lane_id}"),
            approver,
        )?;

        let now = now_timestamp();
        let audit_id = fresh_id("audit");
        let handoff = HandoffRecord {
            handoff_id,
            task_id: task_id.clone(),
            from_lane_id,
            to_lane_id,
            owner: owner.clone(),
            summary,
            acceptance,
            audit_id: audit_id.clone(),
            updated_at: now,
        };
        self.runtime_handoffs.push(handoff.clone());
        let mut gate_update = None;
        if acceptance == HandoffAcceptance::Accepted
            && let Some(gate) = self
                .runtime_merge_gates
                .iter_mut()
                .find(|gate| gate.task_id == task_id)
        {
            gate.owner = owner;
            gate.audit_ids.push(audit_id);
            gate.updated_at = Some(now);
            gate_update = Some(gate.clone());
        }
        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::HandoffUpdated { handoff },
        )];
        if let Some(gate) = gate_update {
            events.push(RuntimeEvent::new(
                2,
                RuntimeEventKind::MergeGateUpdated { gate },
            ));
        }
        Ok(events)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn request_review<F>(
        &mut self,
        review_id: String,
        gate_id: String,
        requester_lane_id: String,
        reviewer_lane_id: String,
        owner: RuntimeOwner,
        evidence_ids: Vec<String>,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        validate_trust_id("review_id", &review_id)?;
        validate_trust_id("gate_id", &gate_id)?;
        validate_trust_id("requester_lane_id", &requester_lane_id)?;
        validate_trust_id("reviewer_lane_id", &reviewer_lane_id)?;
        if requester_lane_id == reviewer_lane_id {
            return Err("review requires an independent lane".to_string());
        }
        ensure_unique(
            self.runtime_review_requests
                .iter()
                .any(|record| record.review_id == review_id),
            "review request",
            &review_id,
        )?;
        let gate_index = self.require_merge_gate_index(&gate_id)?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        validate_review_requester(&self.runtime_merge_gates[gate_index], &requester_lane_id)?;
        validate_owner(&owner, Some(&reviewer_lane_id), &task_id)?;
        for evidence_id in &evidence_ids {
            validate_trust_id("evidence_id", evidence_id)?;
            if !self
                .runtime_evidence
                .iter()
                .any(|evidence| evidence.id == *evidence_id)
            {
                return Err(format!("review evidence `{evidence_id}` does not exist"));
            }
        }
        self.require_trust_permission(
            "request_review",
            &format!("gate={gate_id} reviewer={reviewer_lane_id}"),
            approver,
        )?;

        let now = now_timestamp();
        let audit_id = fresh_id("audit");
        let review = ReviewRequestRecord {
            review_id: review_id.clone(),
            gate_id,
            task_id,
            requester_lane_id,
            reviewer_lane_id,
            owner: owner.clone(),
            evidence_ids,
            status: ReviewRequestStatus::Pending,
            audit_id: audit_id.clone(),
            updated_at: now,
        };
        self.runtime_review_requests.push(review.clone());
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.validator = Some(MergeGateValidator {
            owner,
            review_request_id: review_id,
            independent: true,
            validated_at: None,
        });
        gate.policy_snapshot.requires_independent_validator = true;
        gate.audit_ids.push(audit_id);
        gate.updated_at = Some(now);
        Ok(vec![
            RuntimeEvent::new(1, RuntimeEventKind::ReviewRequestUpdated { review }),
            RuntimeEvent::new(2, RuntimeEventKind::MergeGateUpdated { gate: gate.clone() }),
        ])
    }

    pub(crate) fn confirm_contract<F>(
        &mut self,
        contract_id: String,
        task_id: String,
        owner: RuntimeOwner,
        summary: String,
        decision: ContractDecision,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        validate_trust_id("contract_id", &contract_id)?;
        validate_trust_id("task_id", &task_id)?;
        validate_owner(&owner, owner.lane_id.as_deref(), &task_id)?;
        self.require_runtime_task(&task_id)?;
        ensure_unique(
            self.runtime_contracts
                .iter()
                .any(|record| record.contract_id == contract_id),
            "contract",
            &contract_id,
        )?;
        let summary = validate_trust_text("contract summary", summary, 500)?;
        self.require_trust_permission(
            "confirm_contract",
            &format!("task={task_id} contract={contract_id}"),
            approver,
        )?;
        let now = now_timestamp();
        let contract = ContractRecord {
            contract_id,
            task_id,
            owner,
            summary,
            decision,
            audit_id: fresh_id("audit"),
            updated_at: now,
        };
        self.runtime_contracts.push(contract.clone());
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::ContractUpdated { contract },
        )])
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_dependency<F>(
        &mut self,
        dependency_id: String,
        task_id: String,
        depends_on_task_id: String,
        owner: RuntimeOwner,
        state: DependencyState,
        reason: String,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        validate_trust_id("dependency_id", &dependency_id)?;
        validate_trust_id("task_id", &task_id)?;
        validate_trust_id("depends_on_task_id", &depends_on_task_id)?;
        validate_owner(&owner, owner.lane_id.as_deref(), &task_id)?;
        self.require_runtime_task(&task_id)?;
        let reason = validate_trust_text("dependency reason", reason, 240)?;
        self.require_trust_permission(
            "set_dependency",
            &format!("task={task_id} dependency={depends_on_task_id} state={state:?}"),
            approver,
        )?;

        let now = now_timestamp();
        let dependency = DependencyRecord {
            dependency_id,
            task_id: task_id.clone(),
            depends_on_task_id,
            owner,
            state,
            reason: reason.clone(),
            audit_id: fresh_id("audit"),
            updated_at: now,
        };
        upsert_dependency(&mut self.runtime_dependencies, dependency.clone());
        let mut events = vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::DependencyUpdated { dependency },
        )];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            match state {
                DependencyState::Blocked => {
                    task.status = AgentTaskStatus::Blocked;
                    task.activity = format!("dependency blocked: {reason}");
                }
                DependencyState::Unblocked if task.status == AgentTaskStatus::Blocked => {
                    task.status = AgentTaskStatus::Queued;
                    task.activity = format!("dependency unblocked: {reason}");
                }
                DependencyState::Unblocked => {}
            }
            task.updated_at = Some(now.saturating_mul(1000));
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(2, RuntimeEventKind::TaskUpdated { task }));
        }
        Ok(events)
    }

    pub(crate) fn bounce_merge_conflict<F>(
        &mut self,
        gate_id: String,
        original_lane_id: String,
        owner: RuntimeOwner,
        reason: String,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        validate_trust_id("gate_id", &gate_id)?;
        validate_trust_id("original_lane_id", &original_lane_id)?;
        let gate_index = self.require_merge_gate_index(&gate_id)?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        validate_owner(&owner, owner.lane_id.as_deref(), &task_id)?;
        let reason = validate_trust_text("conflict reason", reason, 500)?;
        self.require_trust_permission(
            "bounce_merge_conflict",
            &format!("gate={gate_id} origin={original_lane_id}"),
            approver,
        )?;
        Ok(self.record_conflict_bounce(gate_index, original_lane_id, owner, reason))
    }

    pub(crate) fn record_conflict_bounce(
        &mut self,
        gate_index: usize,
        original_lane_id: String,
        owner: RuntimeOwner,
        reason: String,
    ) -> Vec<RuntimeEvent> {
        let now = now_timestamp();
        let gate_id = self.runtime_merge_gates[gate_index].gate_id.clone();
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        let audit_id = fresh_id("audit");
        let conflict = ConflictBounce {
            bounce_id: fresh_id("conflict"),
            gate_id,
            task_id: task_id.clone(),
            original_lane_id,
            owner: owner.clone(),
            reason: reason.clone(),
            status: ConflictBounceStatus::Pending,
            evidence_ids: self.runtime_merge_gates[gate_index].evidence_ids.clone(),
            audit_id: audit_id.clone(),
            created_at: now,
            revalidated_at: None,
        };
        self.runtime_conflict_bounces.push(conflict.clone());
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.status = viden_types::MergeGateStatus::NeedsChanges;
        gate.conflict = Some(conflict.clone());
        gate.decision = Some(merge_gate_decision(
            MergeGateDecisionOutcome::Conflict,
            reason.clone(),
            owner,
            gate.evidence_ids.clone(),
            audit_id.clone(),
        ));
        gate.audit_ids.push(audit_id);
        gate.updated_at = Some(now);
        let gate = gate.clone();
        let mut events = vec![
            RuntimeEvent::new(
                1,
                RuntimeEventKind::MergeConflictBounced {
                    conflict: conflict.clone(),
                },
            ),
            RuntimeEvent::new(2, RuntimeEventKind::MergeGateUpdated { gate }),
        ];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            task.status = AgentTaskStatus::NeedsInput;
            task.activity = reason;
            task.next_action = Some(AgentNextAction {
                label: "resolve merge conflict".to_string(),
                command: Some(format!("/lane attach {}", conflict.original_lane_id)),
                reason: Some("merge conflict returned to the originating lane".to_string()),
            });
            task.updated_at = Some(now.saturating_mul(1000));
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(3, RuntimeEventKind::TaskUpdated { task }));
        }
        events
    }

    pub(crate) fn mark_conflict_revalidated(
        &mut self,
        gate_index: usize,
    ) -> Option<ConflictBounce> {
        let now = now_timestamp();
        let conflict = self.runtime_merge_gates[gate_index].conflict.as_mut()?;
        conflict.status = ConflictBounceStatus::Revalidated;
        conflict.revalidated_at = Some(now);
        let conflict = conflict.clone();
        upsert_conflict(&mut self.runtime_conflict_bounces, conflict.clone());
        Some(conflict)
    }

    pub(crate) fn mark_conflict_resolved(&mut self, gate_index: usize) -> Option<ConflictBounce> {
        let conflict = self.runtime_merge_gates[gate_index].conflict.as_mut()?;
        conflict.status = ConflictBounceStatus::Resolved;
        let conflict = conflict.clone();
        upsert_conflict(&mut self.runtime_conflict_bounces, conflict.clone());
        Some(conflict)
    }

    pub(crate) fn revert_applied_change<F>(
        &mut self,
        gate_id: String,
        owner: RuntimeOwner,
        reason: String,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        validate_trust_id("gate_id", &gate_id)?;
        let gate_index = self.require_merge_gate_index(&gate_id)?;
        let task_id = self.runtime_merge_gates[gate_index].task_id.clone();
        validate_owner(&owner, owner.lane_id.as_deref(), &task_id)?;
        if self.runtime_merge_gates[gate_index].status != viden_types::MergeGateStatus::Merged {
            return Err(format!(
                "merge gate `{gate_id}` has no applied change to revert"
            ));
        }
        let applied_change_id = self.runtime_merge_gates[gate_index]
            .applied_change_id
            .clone()
            .ok_or_else(|| format!("merge gate `{gate_id}` is missing applied change identity"))?;
        let original = self
            .applied_change_rollbacks
            .get(&applied_change_id)
            .cloned()
            .ok_or_else(|| {
                format!("applied change `{applied_change_id}` has no local recovery snapshot")
            })?;
        let reason = validate_trust_text("revert reason", reason, 500)?;
        self.require_trust_permission(
            "revert_applied_change",
            &format!("gate={gate_id} change={applied_change_id}"),
            approver,
        )?;

        let paths = original
            .iter()
            .map(|rollback| rollback.path.clone())
            .collect::<Vec<_>>();
        self.stage_rollback_paths(&paths)?;
        self.persist_merge_gate_precommit(gate_index, "audit-revert-precommit")?;
        self.restore_file_rollbacks(&original)?;

        let now = now_timestamp();
        let audit_id = fresh_id("audit");
        let restored_paths = paths
            .iter()
            .filter_map(|path| path.strip_prefix(&self.cwd).ok())
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>();
        let revert = RevertRecord {
            revert_id: fresh_id("revert"),
            gate_id,
            applied_change_id: applied_change_id.clone(),
            owner: owner.clone(),
            reason: reason.clone(),
            restored_paths,
            audit_id: audit_id.clone(),
            reverted_at: now,
        };
        self.runtime_reverts.push(revert.clone());
        self.applied_change_rollbacks.remove(&applied_change_id);
        let gate = &mut self.runtime_merge_gates[gate_index];
        gate.status = viden_types::MergeGateStatus::Reverted;
        gate.decision = Some(merge_gate_decision(
            MergeGateDecisionOutcome::Reverted,
            reason.clone(),
            owner,
            gate.evidence_ids.clone(),
            audit_id.clone(),
        ));
        gate.audit_ids.push(audit_id);
        gate.updated_at = Some(now);
        let gate = gate.clone();

        let mut events = vec![
            RuntimeEvent::new(1, RuntimeEventKind::RevertRecorded { revert }),
            RuntimeEvent::new(2, RuntimeEventKind::MergeGateUpdated { gate }),
        ];
        if let Some(mut task) = self
            .runtime_tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
        {
            task.status = AgentTaskStatus::NeedsInput;
            task.activity = format!("applied change reverted: {reason}");
            task.next_action = Some(AgentNextAction {
                label: "revise reverted change".to_string(),
                command: Some(format!("/agent start {task_id}")),
                reason: Some("the applied change was reverted by an audited command".to_string()),
            });
            task.updated_at = Some(now.saturating_mul(1000));
            self.upsert_agent_task(task.clone());
            events.push(RuntimeEvent::new(3, RuntimeEventKind::TaskUpdated { task }));
        }
        Ok(events)
    }

    pub(crate) fn require_trust_permission<F>(
        &mut self,
        action: &str,
        preview: &str,
        approver: &mut F,
    ) -> Result<(), String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        if let Some(denial) = self.ensure_workflow_permission(action, preview, approver)? {
            return Err(denial);
        }
        Ok(())
    }

    fn require_runtime_task(&self, task_id: &str) -> Result<(), String> {
        if self.runtime_tasks.iter().any(|task| task.id == task_id) {
            Ok(())
        } else {
            Err(format!("agent task `{task_id}` does not exist"))
        }
    }

    pub(crate) fn require_merge_gate_index(&self, gate_id: &str) -> Result<usize, String> {
        self.runtime_merge_gates
            .iter()
            .position(|gate| gate.gate_id == gate_id)
            .ok_or_else(|| format!("merge gate `{gate_id}` does not exist"))
    }
}

pub(crate) fn merge_gate_decision(
    outcome: MergeGateDecisionOutcome,
    reason: String,
    owner: RuntimeOwner,
    evidence_ids: Vec<String>,
    audit_id: String,
) -> MergeGateDecision {
    MergeGateDecision {
        outcome,
        reason,
        owner,
        evidence_ids,
        audit_id,
        decided_at: now_timestamp(),
    }
}

fn validate_owner(
    owner: &RuntimeOwner,
    lane_id: Option<&str>,
    task_id: &str,
) -> Result<(), String> {
    validate_trust_id("owner.workspace_id", &owner.workspace_id)
        .map_err(|_| "trust-loop owner requires valid workspace identity".to_string())?;
    validate_trust_id("owner.project_id", &owner.project_id)
        .map_err(|_| "trust-loop owner requires valid project identity".to_string())?;
    for (name, value) in [
        ("owner.lane_id", owner.lane_id.as_deref()),
        ("owner.session_id", owner.session_id.as_deref()),
        ("owner.task_id", owner.task_id.as_deref()),
        ("owner.turn_id", owner.turn_id.as_deref()),
    ] {
        if let Some(value) = value {
            validate_trust_id(name, value)?;
        }
    }
    if owner.task_id.as_deref() != Some(task_id) {
        return Err("trust-loop owner task identity mismatch".to_string());
    }
    if let Some(lane_id) = lane_id
        && owner.lane_id.as_deref() != Some(lane_id)
    {
        return Err("trust-loop owner lane identity mismatch".to_string());
    }
    Ok(())
}

fn validate_review_requester(
    gate: &viden_types::MergeGateRecord,
    requester_lane_id: &str,
) -> Result<(), String> {
    if let Some(owner_lane_id) = gate.owner.lane_id.as_deref()
        && owner_lane_id != requester_lane_id
    {
        return Err("review requester does not own the merge gate".to_string());
    }
    Ok(())
}

fn validate_trust_id(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
        || value.contains("..")
        || value.starts_with(['.', ':', '-', '_'])
        || value.ends_with(['.', ':', '-', '_'])
    {
        return Err(format!("invalid trust-loop {name}"));
    }
    Ok(())
}

fn validate_trust_text(name: &str, value: String, max_chars: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(format!(
            "{name} cannot be empty or contain control characters"
        ));
    }
    Ok(truncate_for_preview(value, max_chars))
}

fn ensure_unique(exists: bool, kind: &str, id: &str) -> Result<(), String> {
    if exists {
        Err(format!("{kind} `{id}` already exists"))
    } else {
        Ok(())
    }
}

fn upsert_dependency(records: &mut Vec<DependencyRecord>, record: DependencyRecord) {
    if let Some(existing) = records
        .iter_mut()
        .find(|existing| existing.dependency_id == record.dependency_id)
    {
        *existing = record;
    } else {
        records.push(record);
    }
}

fn upsert_conflict(records: &mut Vec<ConflictBounce>, record: ConflictBounce) {
    if let Some(existing) = records
        .iter_mut()
        .find(|existing| existing.bounce_id == record.bounce_id)
    {
        *existing = record;
    } else {
        records.push(record);
    }
}
