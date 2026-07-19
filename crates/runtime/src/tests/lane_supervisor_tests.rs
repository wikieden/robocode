use std::sync::{
    Arc,
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
    lane_runtime::{LaneEffectExecutor, LaneEffectRequest, LaneEffectResult},
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

#[derive(Default)]
struct CountingLaneEffects {
    calls: AtomicUsize,
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
