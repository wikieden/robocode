use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use viden_provider::ModelProvider;
use viden_types::{
    AgentLaneRecord, AgentRole, AgentRoute, DataEgressPolicy, EventCursor, ExecutionTarget,
    FRONTEND_SCHEMA_V1, GateStrength, LaneBudget, LaneStatus, MutationPolicy, RuntimeCommand,
    RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind, RuntimeOwner, RuntimeSnapshot,
    RuntimeViewState, RuntimeWireEvent, WorkMode,
};

use crate::{
    RuntimeSupervisor, SessionEngine,
    lane_runtime::{
        LaneEffectExecutor, LaneEffectRequest, LaneEffectResult, resolve_lane_output_log,
    },
};

use super::{SequenceProvider, temp_dir};

#[test]
fn lane_supervisor_protocol_round_trips_all_lifecycle_commands() {
    let commands = vec![
        RuntimeCommand::CreateLane {
            lane: lane("lane-a"),
        },
        RuntimeCommand::StartLane {
            lane_id: "lane-a".to_string(),
            command: "worker".to_string(),
            args: vec!["--run".to_string()],
            env: vec![("VIDEN_LANE".to_string(), "lane-a".to_string())],
            output_log: Some(".viden/lanes/lane-a.log".to_string()),
        },
        RuntimeCommand::StopLane {
            lane_id: "lane-a".to_string(),
        },
        RuntimeCommand::AttachLane {
            lane_id: "lane-a".to_string(),
        },
        RuntimeCommand::DetachLane {
            lane_id: "lane-a".to_string(),
        },
        RuntimeCommand::SendLaneInput {
            lane_id: "lane-a".to_string(),
            input: "continue\n".to_string(),
        },
        RuntimeCommand::AcceptLaneOutput {
            lane_id: "lane-a".to_string(),
            summary: "accepted".to_string(),
        },
        RuntimeCommand::ReviseLaneOutput {
            lane_id: "lane-a".to_string(),
            feedback: "add tests".to_string(),
        },
        RuntimeCommand::DiscardLaneOutput {
            lane_id: "lane-a".to_string(),
            reason: "superseded".to_string(),
        },
        RuntimeCommand::ApplyLaneChanges {
            lane_id: "lane-a".to_string(),
            unified_diff: "diff --git a/a b/a\n".to_string(),
        },
        RuntimeCommand::ResolveLaneConflict {
            lane_id: "lane-a".to_string(),
            unified_diff: "diff --git a/a b/a\n".to_string(),
        },
        RuntimeCommand::ArchiveLane {
            lane_id: "lane-a".to_string(),
            summary: "archived evidence".to_string(),
        },
        RuntimeCommand::CleanupLane {
            lane_id: "lane-a".to_string(),
            force: false,
        },
    ];
    let expected_types = [
        "create_lane",
        "start_lane",
        "stop_lane",
        "attach_lane",
        "detach_lane",
        "send_lane_input",
        "accept_lane_output",
        "revise_lane_output",
        "discard_lane_output",
        "apply_lane_changes",
        "resolve_lane_conflict",
        "archive_lane",
        "cleanup_lane",
    ];

    for (command, expected_type) in commands.into_iter().zip(expected_types) {
        let encoded = serde_json::to_value(&command).unwrap();
        assert_eq!(encoded["type"], expected_type);
        let decoded: RuntimeCommand = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, command);
    }
}

#[test]
fn lane_supervisor_protocol_round_trips_owner_scoped_events_and_projects_view() {
    let owner = owner("lane-a");
    let event_kinds = vec![
        RuntimeEventKind::LaneUpdated {
            lane: lane("lane-a"),
        },
        RuntimeEventKind::LaneOutputAppended {
            lane_id: "lane-a".to_string(),
            stream: "stdout".to_string(),
            content: "tests passed".to_string(),
        },
        RuntimeEventKind::LaneConflictDetected {
            lane_id: "lane-a".to_string(),
            summary: "patch conflict".to_string(),
            paths: vec!["src/lib.rs".to_string()],
        },
        RuntimeEventKind::LaneRecoveryRequired {
            lane_id: "lane-a".to_string(),
            reason: "terminal exited".to_string(),
            next_action: "restart lane".to_string(),
        },
    ];
    let mut view = RuntimeViewState::new(snapshot());

    for (offset, kind) in event_kinds.into_iter().enumerate() {
        let sequence = offset as u64 + 1;
        let envelope = RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: owner.clone(),
            cursor: EventCursor {
                stream_id: "lane-stream".to_string(),
                sequence,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
                sequence,
                Some(100 + sequence),
                kind,
            )),
        };
        let encoded = serde_json::to_vec(&envelope).unwrap();
        let decoded: RuntimeEventEnvelope = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.owner, owner);
        let RuntimeWireEvent::Known(event) = decoded.event else {
            panic!("lane lifecycle event decoded as unknown");
        };
        view.apply_event(&event);
    }

    assert_eq!(view.lanes, vec![lane("lane-a")]);
    assert_eq!(view.lane_outputs.len(), 1);
    assert_eq!(view.lane_outputs[0].lane_id, "lane-a");
    assert_eq!(view.lane_outputs[0].content, "tests passed");
    assert_eq!(view.lane_conflicts.len(), 1);
    assert_eq!(view.lane_conflicts[0].paths, vec!["src/lib.rs"]);
    assert_eq!(view.lane_recoveries.len(), 1);
    assert_eq!(view.lane_recoveries[0].next_action, "restart lane");
}

#[test]
fn lane_supervisor_plan_mode_rejects_effectful_commands_before_effects() {
    let cwd = temp_dir("lane_supervisor_plan_gate_cwd");
    let home = temp_dir("lane_supervisor_plan_gate_home");
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| viden_types::ApprovalResponse::allow_once(None);
    engine
        .handle_runtime_command(
            "cmd_plan",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
            &mut approver,
        )
        .unwrap();
    let effects = Arc::new(CountingLaneEffects::default());
    let supervisor = RuntimeSupervisor::start_with_lane_effects_for_test(
        engine,
        effects.clone() as Arc<dyn LaneEffectExecutor>,
    );
    let lane_owner = owner("lane-plan");

    let commands = [
        (
            "cmd_create_plan",
            RuntimeCommand::CreateLane {
                lane: lane("lane-plan"),
            },
        ),
        (
            "cmd_start_plan",
            RuntimeCommand::StartLane {
                lane_id: "lane-plan".to_string(),
                command: "worker".to_string(),
                args: Vec::new(),
                env: Vec::new(),
                output_log: None,
            },
        ),
        (
            "cmd_apply_plan",
            RuntimeCommand::ApplyLaneChanges {
                lane_id: "lane-plan".to_string(),
                unified_diff: "diff --git a/a b/a\n".to_string(),
            },
        ),
        (
            "cmd_cleanup_plan",
            RuntimeCommand::CleanupLane {
                lane_id: "lane-plan".to_string(),
                force: true,
            },
        ),
    ];
    for (command_id, command) in commands {
        supervisor
            .send_command_from_owner(lane_owner.clone(), command_id, command)
            .unwrap();
    }

    let envelopes = collect_envelopes_until(&supervisor, |events| {
        events
            .iter()
            .filter(|envelope| {
                matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::CommandRejected { reason, .. },
                        ..
                    }) if reason.contains("Plan mode")
                )
            })
            .count()
            == 4
    });
    assert_eq!(effects.calls.load(Ordering::SeqCst), 0);
    assert!(
        envelopes
            .iter()
            .all(|envelope| envelope.owner == lane_owner)
    );
}

#[test]
fn lane_supervisor_output_log_is_confined_to_canonical_lane_root() {
    let root = temp_dir("lane_output_log_scope");
    let lane_root = root.join("lane");
    std::fs::create_dir_all(lane_root.join("logs")).unwrap();
    let resolved = resolve_lane_output_log(&lane_root, Some("logs/worker.log"), "lane-a")
        .expect("scoped relative log");
    assert_eq!(
        resolved,
        lane_root.canonicalize().unwrap().join("logs/worker.log")
    );
    assert!(resolve_lane_output_log(&lane_root, Some("../escape.log"), "lane-a").is_err());
    assert!(resolve_lane_output_log(&lane_root, Some("/tmp/escape.log"), "lane-a").is_err());

    #[cfg(unix)]
    {
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, lane_root.join("logs/link")).unwrap();
        assert!(
            resolve_lane_output_log(&lane_root, Some("logs/link/escape.log"), "lane-a").is_err()
        );
    }
}

#[test]
fn lane_supervisor_apply_enforces_state_and_mutation_policy_before_effect() {
    let cwd = temp_dir("lane_apply_policy_cwd");
    let home = temp_dir("lane_apply_policy_home");
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine
        .set_permission_mode(viden_types::PermissionMode::DontAsk)
        .unwrap();
    let effects = Arc::new(RecordingLaneEffects::default());
    let supervisor = RuntimeSupervisor::start_with_lane_effects_for_test(
        engine,
        effects.clone() as Arc<dyn LaneEffectExecutor>,
    );
    let owner = owner("lane-policy");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create_policy",
            RuntimeCommand::CreateLane {
                lane: lane("lane-policy"),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "apply_wrong_state",
            RuntimeCommand::ApplyLaneChanges {
                lane_id: "lane-policy".to_string(),
                unified_diff: "diff --git a/a b/a\n".to_string(),
            },
        )
        .unwrap();
    let wrong_state = collect_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { command_id, .. },
                    ..
                }) if command_id == "apply_wrong_state"
            )
        })
    });
    assert!(wrong_state.iter().any(|envelope| envelope.owner == owner));
    assert!(!effects.calls.lock().unwrap().contains(&"apply".to_string()));

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "accept_policy",
            RuntimeCommand::AcceptLaneOutput {
                lane_id: "lane-policy".to_string(),
                summary: "ready to apply".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "apply_propose_only",
            RuntimeCommand::ApplyLaneChanges {
                lane_id: "lane-policy".to_string(),
                unified_diff: "diff --git a/a b/a\n".to_string(),
            },
        )
        .unwrap();
    let approval = collect_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { approval },
                    ..
                }) if approval.tool_name == "lane_apply"
            )
        })
    });
    assert!(approval.iter().any(|envelope| envelope.owner == owner));
    assert!(!effects.calls.lock().unwrap().contains(&"apply".to_string()));
}

#[test]
fn lane_supervisor_expired_approval_reuses_audit_and_never_applies() {
    let cwd = temp_dir("lane_approval_expiry_cwd");
    let home = temp_dir("lane_approval_expiry_home");
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine
        .set_permission_mode(viden_types::PermissionMode::DontAsk)
        .unwrap();
    let effects = Arc::new(RecordingLaneEffects::default());
    let supervisor = RuntimeSupervisor::start_with_lane_effects_and_approval_ttl_for_test(
        engine,
        effects.clone() as Arc<dyn LaneEffectExecutor>,
        0,
    );
    let owner = owner("lane-expired");
    create_and_mark_done(&supervisor, &owner, "lane-expired");
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "apply_expired",
            RuntimeCommand::ApplyLaneChanges {
                lane_id: "lane-expired".to_string(),
                unified_diff: "diff --git a/a b/a\n".to_string(),
            },
        )
        .unwrap();
    let requested = collect_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let approval = requested
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "respond_expired",
            RuntimeCommand::RespondToApproval {
                request_id: approval.id.clone(),
                response: viden_types::ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let resolved = collect_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { reason, .. },
                    ..
                }) if reason.contains("expired")
            )
        })
    });
    assert!(resolved.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalResolved { audit_id, decision: viden_types::ApprovalDecision::Deny, .. },
                ..
            }) if audit_id == &approval.audit_id
        )
    }));
    assert!(!effects.calls.lock().unwrap().contains(&"apply".to_string()));
}

#[test]
fn lane_supervisor_plan_transition_precedes_queued_apply_effect() {
    let cwd = temp_dir("lane_plan_order_cwd");
    let home = temp_dir("lane_plan_order_home");
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine
        .set_permission_mode(viden_types::PermissionMode::DontAsk)
        .unwrap();
    let effects = Arc::new(RecordingLaneEffects::default());
    let supervisor = RuntimeSupervisor::start_with_lane_effects_for_test(
        engine,
        effects.clone() as Arc<dyn LaneEffectExecutor>,
    );
    let owner = owner("lane-plan-order");
    let mut autonomous = lane("lane-plan-order");
    autonomous.mutation_policy = MutationPolicy::Autonomous;
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "create_plan_order",
            RuntimeCommand::CreateLane { lane: autonomous },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "accept_plan_order",
            RuntimeCommand::AcceptLaneOutput {
                lane_id: "lane-plan-order".to_string(),
                summary: "ready".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "set_plan_order",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "apply_after_plan",
            RuntimeCommand::ApplyLaneChanges {
                lane_id: "lane-plan-order".to_string(),
                unified_diff: "diff --git a/a b/a\n".to_string(),
            },
        )
        .unwrap();
    let events = collect_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { command_id, reason },
                    ..
                }) if command_id == "apply_after_plan" && reason.contains("Plan mode")
            )
        })
    });
    let plan_sequence = events
        .iter()
        .find(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::SnapshotUpdated { snapshot },
                    ..
                }) if snapshot.work_mode == WorkMode::Plan
            )
        })
        .unwrap()
        .cursor
        .sequence;
    let rejection_sequence = events
        .iter()
        .find(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { command_id, .. },
                    ..
                }) if command_id == "apply_after_plan"
            )
        })
        .unwrap()
        .cursor
        .sequence;
    assert!(plan_sequence < rejection_sequence);
    assert!(!effects.calls.lock().unwrap().contains(&"apply".to_string()));
}

#[test]
fn lane_supervisor_keeps_waiting_lane_isolated_and_routes_owner_events() {
    let cwd = temp_dir("lane_supervisor_owner_routing_cwd");
    let home = temp_dir("lane_supervisor_owner_routing_home");
    let provider: Box<dyn ModelProvider> = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine
        .set_permission_mode(viden_types::PermissionMode::DontAsk)
        .unwrap();
    let effects = Arc::new(RecordingLaneEffects::default());
    let supervisor = RuntimeSupervisor::start_with_lane_effects_for_test(
        engine,
        effects.clone() as Arc<dyn LaneEffectExecutor>,
    );
    let owner_a = owner("lane-a");
    let owner_b = owner("lane-b");
    let mut lane_b = lane("lane-b");
    lane_b.mutation_policy = MutationPolicy::Autonomous;

    supervisor
        .send_command_from_owner(
            owner_a.clone(),
            "cmd_create_a",
            RuntimeCommand::CreateLane {
                lane: lane("lane-a"),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "cmd_create_b",
            RuntimeCommand::CreateLane { lane: lane_b },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_a.clone(),
            "cmd_start_a",
            RuntimeCommand::StartLane {
                lane_id: "lane-a".to_string(),
                command: "worker-a".to_string(),
                args: Vec::new(),
                env: Vec::new(),
                output_log: None,
            },
        )
        .unwrap();

    let mut envelopes = collect_envelopes_until(&supervisor, |events| {
        events.iter().any(|envelope| {
            matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::ApprovalRequested { .. },
                    ..
                })
            )
        })
    });
    let approval_id = envelopes
        .iter()
        .find_map(|envelope| match &envelope.event {
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::ApprovalRequested { approval },
                ..
            }) => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();

    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "cmd_wrong_owner_approval",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id,
                response: viden_types::ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "cmd_start_b",
            RuntimeCommand::StartLane {
                lane_id: "lane-b".to_string(),
                command: "worker-b".to_string(),
                args: Vec::new(),
                env: Vec::new(),
                output_log: None,
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "cmd_input_b",
            RuntimeCommand::SendLaneInput {
                lane_id: "lane-b".to_string(),
                input: "continue\n".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_b.clone(),
            "cmd_accept_b",
            RuntimeCommand::AcceptLaneOutput {
                lane_id: "lane-b".to_string(),
                summary: "lane B complete".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner_a.clone(),
            "cmd_cancel_a",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();

    envelopes.extend(collect_envelopes_until(&supervisor, |events| {
        let has_b_done = events.iter().any(|envelope| {
            envelope.owner == owner_b
                && matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::LaneUpdated { lane },
                        ..
                    }) if lane.id == "lane-b" && lane.status == LaneStatus::Done
                )
        });
        let has_a_cancelled = events.iter().any(|envelope| {
            envelope.owner == owner_a
                && matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::LaneUpdated { lane },
                        ..
                    }) if lane.id == "lane-a" && lane.status == LaneStatus::Cancelled
                )
        });
        has_b_done && has_a_cancelled
    }));

    let calls = effects.calls.lock().unwrap().clone();
    assert!(calls.contains(&"create:lane-a".to_string()));
    assert!(calls.contains(&"create:lane-b".to_string()));
    assert!(calls.contains(&"start:lane-b".to_string()));
    assert!(calls.contains(&"input:lane-b".to_string()));
    assert!(!calls.contains(&"start:lane-a".to_string()));
    assert!(!calls.contains(&"stop:lane-a".to_string()));

    assert!(envelopes.iter().any(|envelope| {
        envelope.owner == owner_b
            && matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::CommandRejected { command_id, reason },
                    ..
                }) if command_id == "cmd_wrong_owner_approval" && reason.contains("owner mismatch")
            )
    }));
    assert!(!envelopes.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(RuntimeEvent {
                kind: RuntimeEventKind::CommandAccepted { command_id, .. },
                ..
            }) if command_id == "cmd_wrong_owner_approval"
        )
    }));
    let queued_sequence = envelopes
        .iter()
        .find(|envelope| {
            envelope.owner == owner_b
                && matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::InputQueued { .. },
                        ..
                    })
                )
        })
        .unwrap()
        .cursor
        .sequence;
    let dequeued_sequence = envelopes
        .iter()
        .find(|envelope| {
            envelope.owner == owner_b
                && matches!(
                    &envelope.event,
                    RuntimeWireEvent::Known(RuntimeEvent {
                        kind: RuntimeEventKind::InputDequeued { .. },
                        ..
                    })
                )
        })
        .unwrap()
        .cursor
        .sequence;
    assert!(queued_sequence < dequeued_sequence);
    assert!(
        supervisor
            .snapshot_envelope()
            .unwrap()
            .view
            .queued_inputs
            .is_empty()
    );
    assert!(envelopes.iter().any(|envelope| {
        envelope.owner == owner_b
            && matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::InputQueued { .. },
                    ..
                })
            )
    }));
    assert!(envelopes.iter().any(|envelope| {
        envelope.owner == owner_b
            && matches!(
                &envelope.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::Error { .. },
                    ..
                })
            )
    }));
    assert!(envelopes.iter().all(|envelope| match &envelope.event {
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::ApprovalRequested { approval },
            ..
        }) => envelope.owner == owner_a && approval.owner == owner_a,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::ApprovalResolved { owner, .. },
            ..
        }) => envelope.owner == owner_a && owner == &owner_a,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::LaneUpdated { lane },
            ..
        }) => envelope.owner.lane_id.as_deref() == Some(lane.id.as_str()),
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::LaneOutputAppended { lane_id, .. },
            ..
        }) => envelope.owner.lane_id.as_deref() == Some(lane_id.as_str()),
        _ => true,
    }));
}

#[derive(Default)]
struct CountingLaneEffects {
    calls: AtomicUsize,
}

#[derive(Default)]
struct RecordingLaneEffects {
    calls: Mutex<Vec<String>>,
}

impl LaneEffectExecutor for RecordingLaneEffects {
    fn execute(&self, request: LaneEffectRequest) -> Result<LaneEffectResult, String> {
        let call = match request {
            LaneEffectRequest::Create { lane, .. } => format!("create:{}", lane.id),
            LaneEffectRequest::Start { lane, .. } => format!("start:{}", lane.id),
            LaneEffectRequest::Stop { lane_id } => format!("stop:{lane_id}"),
            LaneEffectRequest::SendInput { lane_id, .. } => {
                self.calls.lock().unwrap().push(format!("input:{lane_id}"));
                return Err(format!("lane `{lane_id}` input channel closed"));
            }
            LaneEffectRequest::Apply { .. } => "apply".to_string(),
            LaneEffectRequest::Cleanup { lane, .. } => format!("cleanup:{}", lane.id),
        };
        self.calls.lock().unwrap().push(call);
        Ok(LaneEffectResult::success("effect completed"))
    }
}

impl LaneEffectExecutor for CountingLaneEffects {
    fn execute(&self, _request: LaneEffectRequest) -> Result<LaneEffectResult, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(LaneEffectResult::success("effect completed"))
    }
}

fn collect_envelopes_until(
    supervisor: &RuntimeSupervisor,
    done: impl Fn(&[RuntimeEventEnvelope]) -> bool,
) -> Vec<RuntimeEventEnvelope> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < Duration::from_secs(5) {
        if let Some(event) = supervisor.recv_event_envelope_timeout(Duration::from_millis(50)) {
            events.push(event);
            if done(&events) {
                return events;
            }
        }
    }
    panic!("timed out waiting for lane envelopes: {events:#?}");
}

fn owner(lane_id: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-test".to_string(),
        project_id: "project-test".to_string(),
        lane_id: Some(lane_id.to_string()),
        session_id: Some(format!("session-{lane_id}")),
        task_id: None,
        turn_id: Some(format!("turn-{lane_id}")),
    }
}

fn lane(id: &str) -> AgentLaneRecord {
    AgentLaneRecord {
        id: id.to_string(),
        task_id: None,
        role: AgentRole::Coder,
        route: AgentRoute::Terminal,
        gate_strength: GateStrength::Containment,
        mutation_policy: MutationPolicy::ProposeOnly,
        worktree: Some(format!(".worktrees/{id}")),
        branch: Some(format!("codex/{id}")),
        target: ExecutionTarget::Local,
        data_egress: DataEgressPolicy::Deny,
        status: LaneStatus::Draft,
        budget: LaneBudget::default(),
        active_session_ids: Vec::new(),
        summary: "ready".to_string(),
        evidence: Vec::new(),
    }
}

fn snapshot() -> RuntimeSnapshot {
    RuntimeSnapshot {
        cwd: std::env::temp_dir(),
        provider_family: "test".to_string(),
        model_label: "test".to_string(),
        work_mode: WorkMode::Build,
        permission_mode: viden_types::PermissionMode::Default,
        permission_level: viden_types::PermissionLevel::Ask,
        config_summary: String::new(),
        loaded_config_files: Vec::new(),
        startup_overrides: Vec::new(),
        ui_preferences: viden_types::ResolvedUiPreferences::default(),
    }
}

fn create_and_mark_done(supervisor: &RuntimeSupervisor, owner: &RuntimeOwner, lane_id: &str) {
    supervisor
        .send_command_from_owner(
            owner.clone(),
            format!("create_{lane_id}"),
            RuntimeCommand::CreateLane {
                lane: lane(lane_id),
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner.clone(),
            format!("accept_{lane_id}"),
            RuntimeCommand::AcceptLaneOutput {
                lane_id: lane_id.to_string(),
                summary: "ready".to_string(),
            },
        )
        .unwrap();
}
