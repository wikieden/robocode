use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use std::{fs, path::Path};

use viden_lsp::{LspRuntime, LspServerConfig, LspServerRegistry};
use viden_provider::{ModelProvider, ModelRequestControl};
use viden_types::{
    AgentDagTaskSpec, AgentRole, AgentTaskStatus, ApprovalDecision, ApprovalResponse,
    ApprovalScope, ContextBundleRecord, EventCursor, FRONTEND_SCHEMA_V1, MergeGateStatus,
    ModelEvent, ModelRequest, PermissionBehavior, PermissionLevel, PermissionRule,
    PermissionRuleSource, PermissionRuleValue, ReplayRequest, RuntimeCommand,
    RuntimeCommandEnvelope, RuntimeEvent, RuntimeEventKind, RuntimeOwner, RuntimeWireEvent,
    ToolCall, ToolInput, TranscriptPageRequest, WorkMode,
};
use viden_workflows::stores::WorkflowStore;

use crate::{
    RuntimeSupervisor, SessionEngine,
    runtime_contract::{set_retrieve_context_publish_test_hook, set_retrieve_context_test_hook},
    runtime_supervisor::set_before_context_resume_enqueue_hook,
};

use super::{SequenceProvider, temp_dir};

static CUSTOM_ACP_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static RETRIEVE_CONTEXT_HOOK_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn assert_no_project_side_channel_events(events: &[viden_workflows::stores::WorkflowAgentEvent]) {
    let forbidden = [
        "agent_dag_created",
        "agent_task_queued",
        "agent_task_started",
        "agent_task_completed",
        "agent_task_cancelled",
        "agent_task_failed",
        "agent_task_blocked",
        "agent_evidence_recorded",
        "merge_gate_proposed",
        "merge_gate_accepted",
        "merge_gate_rejected",
        "agent_artifact_accepted",
        "agent_artifact_rejected",
        "agent_patch_merge_intent",
        "agent_patch_merged",
        "agent_patch_conflict",
    ];
    assert!(
        events
            .iter()
            .all(|event| !forbidden.contains(&event.event_type.as_str())),
        "new project commands must not emit legacy per-event agent facts: {events:?}"
    );
}

struct BlockingProvider {
    entered: Arc<AtomicBool>,
}

impl ModelProvider for BlockingProvider {
    fn provider_name(&self) -> &str {
        "blocking"
    }

    fn model(&self) -> &str {
        "blocking-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Ok(Vec::new())
    }

    fn next_events_with_control(
        &mut self,
        _request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        self.entered.store(true, Ordering::SeqCst);
        loop {
            control.check_cancelled()?;
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

struct TimeoutUnlessCancelledProvider {
    entered: Arc<AtomicBool>,
}

impl ModelProvider for TimeoutUnlessCancelledProvider {
    fn provider_name(&self) -> &str {
        "cancel-aware"
    }

    fn model(&self) -> &str {
        "cancel-aware-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Err("provider called without cancellation control".to_string())
    }

    fn next_events_with_control(
        &mut self,
        _request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        self.entered.store(true, Ordering::SeqCst);
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(500) {
            control.check_cancelled()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("provider did not receive cancellation".to_string())
    }
}

struct FailingProvider {
    error: String,
}

impl ModelProvider for FailingProvider {
    fn provider_name(&self) -> &str {
        "failing"
    }

    fn model(&self) -> &str {
        "failing-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Err(self.error.clone())
    }
}

struct RecordingProvider {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
    errors: Vec<String>,
}

impl RecordingProvider {
    fn success(requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
        Self {
            requests,
            errors: Vec::new(),
        }
    }

    fn with_errors(requests: Arc<Mutex<Vec<ModelRequest>>>, errors: Vec<String>) -> Self {
        Self { requests, errors }
    }
}

impl ModelProvider for RecordingProvider {
    fn provider_name(&self) -> &str {
        "recording"
    }

    fn model(&self) -> &str {
        "recording-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.requests.lock().unwrap().push(request.clone());
        if !self.errors.is_empty() {
            return Err(self.errors.remove(0));
        }
        Ok(vec![ModelEvent::AssistantText {
            content: "recorded".to_string(),
        }])
    }
}

#[test]
fn runtime_supervisor_envelopes_are_contiguous_owned_and_replayable_before_visibility() {
    let cwd = temp_dir("runtime_supervisor_envelope_cwd");
    let home = temp_dir("runtime_supervisor_envelope_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("envelope");

    supervisor
        .send_command_envelope(RuntimeCommandEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            client_id: "client-envelope".to_string(),
            command_id: "cmd-envelope".to_string(),
            owner: owner.clone(),
            command: RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        })
        .unwrap();

    let first = supervisor
        .recv_event_envelope_timeout(Duration::from_secs(2))
        .expect("first event envelope");
    assert_eq!(first.owner, owner);
    assert_eq!(first.cursor.sequence, 1);
    assert_eq!(
        match &first.event {
            RuntimeWireEvent::Known(event) => event.sequence,
            RuntimeWireEvent::Unknown { .. } => 0,
        },
        first.cursor.sequence
    );

    let replay = supervisor
        .replay_events(ReplayRequest {
            after: EventCursor {
                stream_id: first.cursor.stream_id.clone(),
                sequence: 0,
            },
            limit: 10,
        })
        .unwrap();
    assert_eq!(replay.events.first(), Some(&first));

    let second = supervisor
        .recv_event_envelope_timeout(Duration::from_secs(2))
        .expect("second event envelope");
    assert_eq!(second.owner, owner);
    assert_eq!(second.cursor.sequence, 2);
    assert_eq!(
        match &second.event {
            RuntimeWireEvent::Known(event) => event.sequence,
            RuntimeWireEvent::Unknown { .. } => 0,
        },
        second.cursor.sequence
    );
}

#[test]
fn runtime_supervisor_snapshot_pairs_transient_live_view_with_exact_cursor() {
    let cwd = temp_dir("runtime_supervisor_snapshot_boundary_cwd");
    let home = temp_dir("runtime_supervisor_snapshot_boundary_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command("cmd-no-active", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    let event = supervisor
        .recv_event_envelope_timeout(Duration::from_secs(2))
        .expect("transient command rejection");
    assert!(matches!(
        &event.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::CommandRejected { command_id, .. },
            ..
        }) if command_id == "cmd-no-active"
    ));

    let snapshot = supervisor.snapshot_envelope().unwrap();
    assert_eq!(snapshot.cursor, event.cursor);
    assert!(snapshot.view.errors.iter().any(|error| {
        error
            .message
            .contains("command cmd-no-active rejected: no active turn to cancel")
    }));
}

#[test]
fn runtime_supervisor_transcript_page_query_does_not_advance_event_journal() {
    let cwd = temp_dir("runtime_supervisor_transcript_page_cwd");
    let home = temp_dir("runtime_supervisor_transcript_page_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let session_id = engine.session_id().to_string();
    let supervisor = RuntimeSupervisor::start(engine);
    let before = supervisor.snapshot_envelope().unwrap();

    let page = supervisor
        .load_transcript_page(TranscriptPageRequest {
            session_id: session_id.clone(),
            before: None,
            limit: 20,
        })
        .unwrap();
    let after = supervisor.snapshot_envelope().unwrap();

    assert_eq!(after.cursor, before.cursor);
    assert!(
        page.rows
            .iter()
            .all(|row| row.cursor.session_id == session_id)
    );
}

#[test]
fn runtime_supervisor_transport_receive_distinguishes_timeout_from_stopped_worker() {
    let cwd = temp_dir("runtime_supervisor_receive_disconnect_cwd");
    let home = temp_dir("runtime_supervisor_receive_disconnect_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    assert_eq!(
        supervisor
            .recv_event_envelope(Duration::from_millis(1))
            .unwrap(),
        None
    );

    supervisor.stop_worker_for_test();
    let error = supervisor
        .recv_event_envelope(Duration::from_millis(1))
        .unwrap_err();
    assert!(error.contains("event stream stopped"));
    assert_eq!(
        supervisor.recv_event_envelope_timeout(Duration::from_millis(1)),
        None,
        "legacy callers retain Option semantics"
    );
}

#[test]
fn runtime_supervisor_cancels_active_provider_turn_and_keeps_worker_alive() {
    let cwd = temp_dir("runtime_supervisor_cancel_cwd");
    let home = temp_dir("runtime_supervisor_cancel_home");
    let entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(BlockingProvider {
        entered: Arc::clone(&entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_input",
            RuntimeCommand::SubmitUserInput {
                content: "start long provider turn".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));

    supervisor
        .send_command("cmd_cancel", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_cancel"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
        )
    }));

    supervisor
        .send_command(
            "cmd_mode",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        )
        .unwrap();
    let after_cancel = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == WorkMode::Plan
            )
        })
    });
    assert!(after_cancel.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Plan
        )
    }));
}

#[test]
fn runtime_supervisor_keeps_input_responsive_during_context_retrieval() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let cwd = temp_dir("runtime_supervisor_context_retrieve_cwd");
    let home = temp_dir("runtime_supervisor_context_retrieve_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: "retrieval worker body".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let handle_id = engine
        .runtime_view_state()
        .context_handles
        .first()
        .unwrap()
        .handle_id
        .clone();
    let entered = Arc::new(AtomicBool::new(false));
    let entered_for_hook = Arc::clone(&entered);
    set_retrieve_context_test_hook(Some(Arc::new(move |control| {
        entered_for_hook.store(true, Ordering::SeqCst);
        while control.check_cancelled().is_ok() {
            std::thread::sleep(Duration::from_millis(10));
        }
    })));

    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_retrieve_context",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate blocked worker".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));
    supervisor
        .send_command(
            "cmd_follow_up",
            RuntimeCommand::QueueFollowUp {
                content: "queued while retrieval is blocked".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command("cmd_cancel_retrieval", RuntimeCommand::CancelActiveTurn)
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        let queued_follow_up = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::InputQueued { input }
                    if input.content_preview.contains("queued while retrieval is blocked")
            )
        });
        let accepted_follow_up = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandAccepted { command_id, .. }
                    if command_id == "cmd_follow_up"
            )
        });
        let accepted_cancel = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandAccepted { command_id, .. }
                    if command_id == "cmd_cancel_retrieval"
            )
        });
        let cancelled = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
            )
        });
        queued_follow_up && accepted_follow_up && accepted_cancel && cancelled
    });
    set_retrieve_context_test_hook(None);

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::InputQueued { input }
                if input.content_preview.contains("queued while retrieval is blocked")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_cancel_retrieval"
        )
    }));
    assert!(events.iter().all(|event| match &event.kind {
        RuntimeEventKind::ContextRetrieved { .. } => false,
        RuntimeEventKind::ToolCallFinished { name, .. } if name == "context_read" => false,
        _ => true,
    }));
}

#[test]
fn runtime_supervisor_retrieve_context_waits_for_ask_approval_before_reading() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (mut engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_ask_approve",
        "ask approve body",
    );
    engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let read_started = Arc::new(AtomicBool::new(false));
    let read_started_for_hook = Arc::clone(&read_started);
    set_retrieve_context_test_hook(Some(Arc::new(move |_control| {
        read_started_for_hook.store(true, Ordering::SeqCst);
    })));
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_retrieve_context_ask",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate ask approve".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ApprovalRequested { approval }
                    if approval.tool_name == "context_read"
            )
        })
    });
    assert!(
        !read_started.load(Ordering::SeqCst),
        "retrieval read started before approval"
    );
    let request_id = approval_id(&events);

    supervisor
        .send_command(
            "cmd_retrieve_approval",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ContextRetrieved { retrieval }
                        if retrieval.permission_decision == "approved"
                            && retrieval.reason_rule_category == "rule_ask"
                )
            })
        },
    ));
    set_retrieve_context_test_hook(None);

    assert!(read_started.load(Ordering::SeqCst));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Allow { .. },
                ..
            } if resolved == &request_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                name,
                success: true,
                evidence: Some(evidence),
                ..
            } if name == "context_read" && evidence.summary.contains("ask approve body")
        )
    }));

    supervisor
        .send_command(
            "cmd_retrieve_approval_again",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let replay = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, reason }
                    if command_id == "cmd_retrieve_approval_again"
                        && reason.contains("is not pending")
            )
        })
    });
    assert!(replay.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. }
                if command_id == "cmd_retrieve_approval_again"
        )
    }));
}

#[test]
fn runtime_supervisor_retrieve_context_ask_denial_does_not_read() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (mut engine, handle_id) =
        supervisor_engine_with_context("runtime_supervisor_context_ask_deny", "ask deny body");
    engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let read_started = Arc::new(AtomicBool::new(false));
    let read_started_for_hook = Arc::clone(&read_started);
    set_retrieve_context_test_hook(Some(Arc::new(move |_control| {
        read_started_for_hook.store(true, Ordering::SeqCst);
    })));
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_retrieve_context_deny",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate ask deny".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = approval_id(&events);

    supervisor
        .send_command(
            "cmd_retrieve_denial",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::deny(Some("no".to_string())),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::Error { error }
                        if error.recoverable
                            && error.message.contains("User denied the permission request")
                )
            })
        },
    ));
    set_retrieve_context_test_hook(None);

    assert!(!read_started.load(Ordering::SeqCst));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Deny,
                ..
            } if resolved == &request_id
        )
    }));
    assert!(events.iter().all(|event| match &event.kind {
        RuntimeEventKind::ContextRetrieved { .. } => false,
        RuntimeEventKind::ToolCallFinished { name, .. } if name == "context_read" => false,
        _ => true,
    }));
}

#[test]
fn runtime_supervisor_context_retrieval_cancel_wins_before_success_publish() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_cancel_before_publish",
        "cancel fence body",
    );
    let paused = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let paused_for_hook = Arc::clone(&paused);
    let release_for_hook = Arc::clone(&release);
    set_retrieve_context_publish_test_hook(Some(Arc::new(move |control| {
        paused_for_hook.store(true, Ordering::SeqCst);
        while !release_for_hook.load(Ordering::SeqCst) && control.check_cancelled().is_ok() {
            std::thread::sleep(Duration::from_millis(10));
        }
    })));
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_retrieve_context_cancel_fence",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate cancel fence".to_string(),
            },
        )
        .unwrap();
    wait_until(|| paused.load(Ordering::SeqCst));
    supervisor
        .send_command("cmd_cancel_after_reduce", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    release.store(true, Ordering::SeqCst);

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("Model request cancelled")
            )
        })
    });
    set_retrieve_context_publish_test_hook(None);

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_cancel_after_reduce"
        )
    }));
    assert!(events.iter().all(|event| match &event.kind {
        RuntimeEventKind::ContextRetrieved { .. } => false,
        RuntimeEventKind::ToolCallFinished { name, .. } if name == "context_read" => false,
        _ => true,
    }));
}

#[test]
fn runtime_supervisor_rejects_invalid_retrieve_context_before_accepting_original_command_id() {
    let (engine, _handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_reject_before_accept",
        "unknown handle body",
    );
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_unknown_supervisor_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id: "ctxh-supervisor-unknown".to_string(),
                reason: "hydrate unknown supervisor handle".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, .. }
                    if command_id == "cmd_unknown_supervisor_retrieve"
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_unknown_supervisor_retrieve"
                    && reason.contains("not known")
        )
    }));
    assert!(events.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_unknown_supervisor_retrieve"
        )
    }));
    assert!(events.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. }
                if command_id == "retrieve_context"
        )
    }));
}

#[test]
fn runtime_supervisor_serializes_overlapping_context_retrieval_jobs() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_serialized_retrieval",
        "serialized retrieval body",
    );
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let worker_count = Arc::new(AtomicU64::new(0));
    let entered_for_hook = Arc::clone(&entered);
    let release_for_hook = Arc::clone(&release);
    let worker_count_for_hook = Arc::clone(&worker_count);
    set_retrieve_context_test_hook(Some(Arc::new(move |control| {
        worker_count_for_hook.fetch_add(1, Ordering::SeqCst);
        entered_for_hook.store(true, Ordering::SeqCst);
        while !release_for_hook.load(Ordering::SeqCst) && control.check_cancelled().is_ok() {
            std::thread::sleep(Duration::from_millis(10));
        }
    })));

    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_retrieve_first",
            RuntimeCommand::RetrieveContext {
                handle_id: handle_id.clone(),
                reason: "hydrate first serialized retrieval".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));

    supervisor
        .send_command(
            "cmd_retrieve_second",
            RuntimeCommand::RetrieveContext {
                handle_id: handle_id.clone(),
                reason: "hydrate second overlapping retrieval".to_string(),
            },
        )
        .unwrap();
    let second_rejected = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, .. }
                    if command_id == "cmd_retrieve_second"
            )
        })
    });
    assert_eq!(worker_count.load(Ordering::SeqCst), 1);
    assert!(second_rejected.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_retrieve_second"
                    && reason.contains("active")
        )
    }));
    assert!(second_rejected.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_retrieve_second"
        )
    }));

    supervisor
        .send_command("cmd_cancel_first", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    release.store(true, Ordering::SeqCst);
    let cancelled = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("Model request cancelled")
            )
        })
    });
    let late_events = collect_events_for(&supervisor, Duration::from_millis(200));
    set_retrieve_context_test_hook(None);
    assert!(
        cancelled
            .iter()
            .chain(late_events.iter())
            .all(|event| match &event.kind {
                RuntimeEventKind::ContextRetrieved { .. } => false,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "context_read" => false,
                _ => true,
            }),
        "cancelled retrieval must not publish success"
    );

    supervisor
        .send_command(
            "cmd_retrieve_third",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate third after first clears".to_string(),
            },
        )
        .unwrap();
    let third = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ContextRetrieved { retrieval }
                    if retrieval.reason_category == "hydrate"
            )
        })
    });
    assert!(third.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_retrieve_third"
        )
    }));
    assert!(third.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                name,
                success: true,
                evidence: Some(evidence),
                ..
            } if name == "context_read" && evidence.summary.contains("serialized retrieval body")
        )
    }));
}

#[test]
fn runtime_supervisor_rejects_input_and_agent_start_while_retrieval_owns_active_turn() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_reject_active_starts",
        "active retrieval owner body",
    );
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let entered_for_hook = Arc::clone(&entered);
    let release_for_hook = Arc::clone(&release);
    set_retrieve_context_test_hook(Some(Arc::new(move |control| {
        entered_for_hook.store(true, Ordering::SeqCst);
        while !release_for_hook.load(Ordering::SeqCst) && control.check_cancelled().is_ok() {
            std::thread::sleep(Duration::from_millis(10));
        }
    })));

    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_active_retrieve_owner",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate active owner".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));

    supervisor
        .send_command(
            "cmd_input_while_retrieve",
            RuntimeCommand::SubmitUserInput {
                content: "must not replace retrieval owner".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command(
            "cmd_agent_while_retrieve",
            RuntimeCommand::StartAgentTask {
                task_id: "task_missing_but_should_not_prepare".to_string(),
            },
        )
        .unwrap();
    supervisor
        .send_command(
            "cmd_followup_while_retrieve",
            RuntimeCommand::QueueFollowUp {
                content: "follow-up remains queueable".to_string(),
            },
        )
        .unwrap();

    let rejected = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        let input_rejected = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, reason }
                    if command_id == "cmd_input_while_retrieve"
                        && reason.contains("cmd_active_retrieve_owner")
            )
        });
        let agent_rejected = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, reason }
                    if command_id == "cmd_agent_while_retrieve"
                        && reason.contains("cmd_active_retrieve_owner")
            )
        });
        let followup_queued = events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::InputQueued { input }
                    if input.content_preview.contains("follow-up remains queueable")
            )
        });
        input_rejected && agent_rejected && followup_queued
    });
    assert!(rejected.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_input_while_retrieve"
                    || command_id == "cmd_agent_while_retrieve"
        )
    }));

    supervisor
        .send_command(
            "cmd_cancel_retrieve_owner",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    release.store(true, Ordering::SeqCst);
    let cancelled = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("Model request cancelled")
            )
        })
    });
    set_retrieve_context_test_hook(None);
    assert!(cancelled.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_cancel_retrieve_owner"
        )
    }));
    assert!(cancelled.iter().all(|event| {
        match &event.kind {
            RuntimeEventKind::ContextRetrieved { .. } => false,
            RuntimeEventKind::ToolCallFinished {
                name,
                success: true,
                ..
            } if name == "context_read" => false,
            _ => true,
        }
    }));
}

#[test]
fn runtime_supervisor_pending_retrieval_approval_reserves_owner_until_resolution() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (mut engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_pending_reserves_owner",
        "original pending approval body",
    );
    engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_pending_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate pending owner".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = approval_id(&events);

    supervisor
        .send_command(
            "cmd_replace_context_while_pending",
            RuntimeCommand::SubmitUserInput {
                content: "replacement context must be rejected".to_string(),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::CommandRejected { command_id, reason }
                        if command_id == "cmd_replace_context_while_pending"
                            && reason.contains("cmd_pending_retrieve")
                )
            })
        },
    ));

    supervisor
        .send_command(
            "cmd_approve_pending_retrieve",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ToolCallFinished {
                        name,
                        success: true,
                        evidence: Some(evidence),
                        ..
                    } if name == "context_read"
                        && evidence.summary.contains("original pending approval body")
                )
            })
        },
    ));
    let serialized = serde_json::to_string(&events).unwrap();
    assert!(!serialized.contains("replacement context must be rejected"));
}

#[test]
fn runtime_supervisor_pending_retrieval_deny_and_cancel_release_active_owner() {
    let (mut denied_engine, denied_handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_pending_deny_releases",
        "deny releases body",
    );
    denied_engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let denied_supervisor = RuntimeSupervisor::start(denied_engine);
    denied_supervisor
        .send_command(
            "cmd_pending_deny_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id: denied_handle_id,
                reason: "hydrate pending deny".to_string(),
            },
        )
        .unwrap();
    let deny_request_events =
        collect_events_until(&denied_supervisor, Duration::from_secs(2), |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
        });
    denied_supervisor
        .send_command(
            "cmd_deny_pending_retrieve",
            RuntimeCommand::RespondToApproval {
                request_id: approval_id(&deny_request_events),
                response: ApprovalResponse::deny(Some("deny".to_string())),
            },
        )
        .unwrap();
    let denied = collect_events_until(&denied_supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("User denied the permission request")
            )
        })
    });
    assert!(denied.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                decision: ApprovalDecision::Deny,
                ..
            }
        )
    }));
    denied_supervisor
        .send_command(
            "cmd_input_after_denied_pending",
            RuntimeCommand::SubmitUserInput {
                content: "input starts after denied pending retrieval".to_string(),
            },
        )
        .unwrap();
    let after_deny = collect_events_until(&denied_supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandAccepted { command_id, .. }
                    if command_id == "cmd_input_after_denied_pending"
            )
        })
    });
    assert!(after_deny.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. }
                if command_id == "cmd_input_after_denied_pending"
        )
    }));

    let (mut cancelled_engine, cancelled_handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_pending_cancel_releases",
        "cancel releases body",
    );
    cancelled_engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let cancelled_supervisor = RuntimeSupervisor::start(cancelled_engine);
    cancelled_supervisor
        .send_command(
            "cmd_pending_cancel_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id: cancelled_handle_id,
                reason: "hydrate pending cancel".to_string(),
            },
        )
        .unwrap();
    let cancel_request_events =
        collect_events_until(&cancelled_supervisor, Duration::from_secs(2), |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
        });
    assert!(!approval_id(&cancel_request_events).is_empty());
    cancelled_supervisor
        .send_command(
            "cmd_cancel_pending_retrieve",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    let cancelled = collect_events_until(&cancelled_supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandAccepted { command_id, .. }
                    if command_id == "cmd_cancel_pending_retrieve"
            )
        })
    });
    assert!(cancelled.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                decision: ApprovalDecision::Deny,
                ..
            }
        )
    }));
    cancelled_supervisor
        .send_command(
            "cmd_input_after_cancelled_pending",
            RuntimeCommand::SubmitUserInput {
                content: "input starts after cancelled pending retrieval".to_string(),
            },
        )
        .unwrap();
    let after_cancel =
        collect_events_until(&cancelled_supervisor, Duration::from_secs(2), |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::CommandAccepted { command_id, .. }
                        if command_id == "cmd_input_after_cancelled_pending"
                )
            })
        });
    assert!(after_cancel.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. }
                if command_id == "cmd_input_after_cancelled_pending"
        )
    }));
}

#[test]
fn runtime_supervisor_provider_and_agent_active_turns_reject_retrieve_context() {
    let cwd = temp_dir("runtime_supervisor_provider_active_rejects_retrieve_cwd");
    let home = temp_dir("runtime_supervisor_provider_active_rejects_retrieve_home");
    let provider_entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(BlockingProvider {
        entered: Arc::clone(&provider_entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let provider_supervisor = RuntimeSupervisor::start(engine);
    provider_supervisor
        .send_command(
            "cmd_blocking_provider_input",
            RuntimeCommand::SubmitUserInput {
                content: "block provider turn".to_string(),
            },
        )
        .unwrap();
    wait_until(|| provider_entered.load(Ordering::SeqCst));
    provider_supervisor
        .send_command(
            "cmd_retrieve_during_provider",
            RuntimeCommand::RetrieveContext {
                handle_id: "ctxh-provider-active".to_string(),
                reason: "hydrate during provider".to_string(),
            },
        )
        .unwrap();
    let provider_rejected =
        collect_events_until(&provider_supervisor, Duration::from_secs(2), |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::CommandRejected { command_id, reason }
                        if command_id == "cmd_retrieve_during_provider"
                            && reason.contains("cmd_blocking_provider_input")
                )
            })
        });
    assert!(provider_rejected.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_retrieve_during_provider"
                    && reason.contains("cmd_blocking_provider_input")
        )
    }));
    assert!(provider_rejected.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_retrieve_during_provider"
        )
    }));
    provider_supervisor
        .send_command(
            "cmd_cancel_provider_owner",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    let _ = collect_events_until(&provider_supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("Model request cancelled")
            )
        })
    });

    let cwd = temp_dir("runtime_supervisor_agent_active_rejects_retrieve_cwd");
    let home = temp_dir("runtime_supervisor_agent_active_rejects_retrieve_home");
    let agent_entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(BlockingProvider {
        entered: Arc::clone(&agent_entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let agent_supervisor = RuntimeSupervisor::start(engine);
    agent_supervisor
        .send_command(
            "cmd_agent_dag_for_active_owner",
            RuntimeCommand::StartAgentDag {
                goal: "Block agent active owner".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_active_owner".to_string(),
                    role: AgentRole::Planner,
                    title: "Block active owner".to_string(),
                    objective: "Enter provider turn".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&agent_supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });
    agent_supervisor
        .send_command(
            "cmd_blocking_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_active_owner".to_string(),
            },
        )
        .unwrap();
    wait_until(|| agent_entered.load(Ordering::SeqCst));
    agent_supervisor
        .send_command(
            "cmd_retrieve_during_agent",
            RuntimeCommand::RetrieveContext {
                handle_id: "ctxh-agent-active".to_string(),
                reason: "hydrate during agent".to_string(),
            },
        )
        .unwrap();
    let agent_rejected =
        collect_events_until(&agent_supervisor, Duration::from_secs(2), |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::CommandRejected { command_id, reason }
                        if command_id == "cmd_retrieve_during_agent"
                            && reason.contains("cmd_blocking_agent_start")
                )
            })
        });
    assert!(agent_rejected.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_retrieve_during_agent"
                    && reason.contains("cmd_blocking_agent_start")
        )
    }));
    assert!(agent_rejected.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_retrieve_during_agent"
        )
    }));
    agent_supervisor
        .send_command("cmd_cancel_agent_owner", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    let _ = collect_events_until(&agent_supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("Model request cancelled")
            )
        })
    });
}

#[test]
fn runtime_supervisor_redacts_fast_path_command_accepted_events() {
    let cwd = temp_dir("runtime_supervisor_fast_path_redaction_cwd");
    let home = temp_dir("runtime_supervisor_fast_path_redaction_home");
    let secret = "sk-fast-path-secret";
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_secret_input",
            RuntimeCommand::SubmitUserInput {
                content: format!("inspect {secret} under {}", cwd.display()),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandAccepted { command_id, command }
                    if command_id == "cmd_secret_input"
                        && matches!(command, RuntimeCommand::SubmitUserInput { .. })
            )
        })
    });

    let serialized = serde_json::to_string(&events).unwrap();
    assert!(!serialized.contains(secret));
    assert!(!serialized.contains(cwd.to_string_lossy().as_ref()));
    assert!(serialized.contains("[REDACTED]"));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, command }
                if command_id == "cmd_secret_input"
                    && matches!(command, RuntimeCommand::SubmitUserInput { .. })
        )
    }));
}

#[test]
fn runtime_supervisor_cancels_active_agent_task_and_keeps_worker_alive() {
    let cwd = temp_dir("runtime_supervisor_cancel_agent_cwd");
    let home = temp_dir("runtime_supervisor_cancel_agent_home");
    let entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(TimeoutUnlessCancelledProvider {
        entered: Arc::clone(&entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Cancel active role execution".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan cancellation".to_string(),
                    objective: "Enter provider turn and wait for cancellation".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));

    supervisor
        .send_command("cmd_cancel_agent", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_planner" && task.status == AgentTaskStatus::Cancelled
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_cancel_agent"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_planner" && task.status == AgentTaskStatus::Cancelled
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| kinds.contains("task_updated"))
    }));

    supervisor
        .send_command(
            "cmd_mode_after_agent_cancel",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        )
        .unwrap();
    let after_cancel = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == WorkMode::Plan
            )
        })
    });
    assert!(after_cancel.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Plan
        )
    }));
}

#[test]
fn runtime_supervisor_streams_async_acp_runtime_events_live() {
    let _guard = CUSTOM_ACP_ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("custom ACP env lock");
    let cwd = temp_dir("runtime_supervisor_acp_live_cwd");
    let home = temp_dir("runtime_supervisor_acp_live_home");
    let script = cwd.join("mock-acp-supervisor-live.sh");
    fs::write(
        &script,
        [
            "#!/bin/sh",
            "read _init",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-supervisor-acp\",\"version\":\"0.5.0\"}}}'",
            "read _new_session",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_supervisor_live\"}}'",
            "read _prompt",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_supervisor_live\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"supervisor live delta\"}}}'",
            "sleep 2",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_supervisor_live\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
        ]
        .join("\n"),
    )
    .expect("write supervisor ACP mock");
    // The env lock serializes this process-wide override for the custom ACP descriptor.
    unsafe {
        std::env::set_var(
            "VIDEN_AGENT_ACP_COMMAND",
            format!("sh {}", script.display()),
        );
    }

    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_acp_async",
            RuntimeCommand::SubmitUserInput {
                content: "/agent run acp --async custom-acp stream live".to_string(),
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::AssistantDelta { content, .. }
                    if content == "supervisor live delta"
            )
        })
    });
    // Restore the process environment while still holding the env lock.
    unsafe {
        std::env::remove_var("VIDEN_AGENT_ACP_COMMAND");
    }

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_acp_async"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content == "supervisor live delta"
        )
    }));
}

#[test]
fn runtime_supervisor_resolves_tool_approval_without_tui_coupling() {
    let cwd = temp_dir("runtime_supervisor_approval_cwd");
    let home = temp_dir("runtime_supervisor_approval_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf approved".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_input",
            RuntimeCommand::SubmitUserInput {
                content: "run approved command".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => Some(approval.id.clone()),
            _ => None,
        })
        .expect("approval request event");

    supervisor
        .send_command(
            "cmd_approval",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ToolCallFinished {
                        success: true,
                        evidence: Some(evidence),
                        ..
                    } if evidence.summary.contains("approved")
                )
            })
        },
    ));

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                decision: ApprovalDecision::Allow { .. },
                ..
            }
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                success: true,
                evidence: Some(evidence),
                ..
            } if evidence.summary.contains("approved")
        )
    }));
}

#[test]
fn runtime_supervisor_approval_request_and_resolution_share_owner_and_audit_id() {
    let cwd = temp_dir("runtime_supervisor_approval_audit_cwd");
    let home = temp_dir("runtime_supervisor_approval_audit_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf audit".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_audit".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-a");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_input_audit",
            RuntimeCommand::SubmitUserInput {
                content: "run audited command".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let (request_id, audit_id) = approval_identity(&events);

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_approval_audit",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalRequested { approval }
                if approval.id == request_id
                    && approval.audit_id == audit_id
                    && approval.owner == owner
                    && approval.expires_at > 0
                    && approval.allowed_scopes.contains(&ApprovalScope::Once)
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Allow {
                    scope: ApprovalScope::Once
                },
                audit_id: resolved_audit,
                owner: resolved_owner,
            } if resolved == &request_id && resolved_audit == &audit_id && resolved_owner == &owner
        )
    }));
}

#[test]
fn runtime_supervisor_drop_joins_pending_approval_timer_and_worker() {
    let cwd = temp_dir("runtime_supervisor_drop_join_cwd");
    let home = temp_dir("runtime_supervisor_drop_join_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf pending".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_pending_drop".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_pending_drop",
            RuntimeCommand::SubmitUserInput {
                content: "run pending command".to_string(),
            },
        )
        .unwrap();
    collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let started = Instant::now();
    drop(supervisor);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "supervisor drop left a worker or approval timer detached"
    );
}

#[test]
fn runtime_supervisor_approval_wrong_owner_is_rejected_without_resolving_pending_request() {
    let cwd = temp_dir("runtime_supervisor_approval_owner_cwd");
    let home = temp_dir("runtime_supervisor_approval_owner_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf owned".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_owner".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner_a = owner_for_lane("lane-a");
    let owner_b = owner_for_lane("lane-b");

    supervisor
        .send_command_from_owner(
            owner_a.clone(),
            "cmd_input_owner",
            RuntimeCommand::SubmitUserInput {
                content: "run owned command".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = approval_id(&events);

    supervisor
        .send_command_from_owner(
            owner_b,
            "cmd_wrong_owner",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::CommandRejected { command_id, reason }
                        if command_id == "cmd_wrong_owner" && reason.contains("owner mismatch")
                )
            })
        },
    ));
    assert!(events.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved { request_id: resolved, .. } if resolved == &request_id
        )
    }));

    supervisor
        .send_command_from_owner(
            owner_a,
            "cmd_right_owner",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));
}

#[test]
fn runtime_supervisor_approval_expiry_auto_denies_without_entering_effect() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (mut engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_context_expired_deny",
        "expired body must not be read",
    );
    engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let read_started = Arc::new(AtomicBool::new(false));
    let read_started_for_hook = Arc::clone(&read_started);
    set_retrieve_context_test_hook(Some(Arc::new(move |_control| {
        read_started_for_hook.store(true, Ordering::SeqCst);
    })));
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-expired");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_retrieve_expired",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate expired".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let (request_id, audit_id) = approval_identity(&events);

    supervisor.expire_pending_approvals_for_test(u64::MAX);
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));
    set_retrieve_context_test_hook(None);

    assert!(!read_started.load(Ordering::SeqCst));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Deny,
                audit_id: resolved_audit,
                owner: resolved_owner,
            } if resolved == &request_id && resolved_audit == &audit_id && resolved_owner == &owner
        )
    }));
}

#[test]
fn runtime_supervisor_approval_production_timer_auto_denies_pending_tool() {
    let cwd = temp_dir("runtime_supervisor_approval_timer_cwd");
    let home = temp_dir("runtime_supervisor_approval_timer_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf should-not-run".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_timer".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start_with_approval_ttl_for_test(engine, 0);
    let owner = owner_for_lane("lane-timer");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_input_timer",
            RuntimeCommand::SubmitUserInput {
                content: "run command that expires".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ApprovalResolved {
                    decision: ApprovalDecision::Deny,
                    ..
                }
            )
        })
    });
    let (request_id, audit_id) = approval_identity(&events);

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Deny,
                audit_id: resolved_audit,
                owner: resolved_owner,
            } if resolved == &request_id && resolved_audit == &audit_id && resolved_owner == &owner
        )
    }));
    assert!(events.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished { success: true, .. }
        )
    }));
}

#[test]
fn runtime_supervisor_permission_downgrade_precedes_pending_tool_allow() {
    let cwd = temp_dir("runtime_supervisor_permission_epoch_cwd");
    let home = temp_dir("runtime_supervisor_permission_epoch_home");
    let output = cwd.join("should-not-run.txt");
    let mut input = ToolInput::new();
    input.insert(
        "command".to_string(),
        format!("printf blocked > {}", output.display()),
    );
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_permission_epoch".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-permission-epoch");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_input_permission_epoch",
            RuntimeCommand::SubmitUserInput {
                content: "run one mutating tool".to_string(),
            },
        )
        .unwrap();
    let requested = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = approval_id(&requested);
    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_read_only_before_tool_allow",
            RuntimeCommand::SetPermissionLevel {
                level: PermissionLevel::ReadOnly,
            },
        )
        .unwrap();
    supervisor
        .send_command_from_owner(
            owner,
            "cmd_allow_after_read_only",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ApprovalResolved {
                    request_id: resolved,
                    decision: ApprovalDecision::Deny,
                    ..
                } if resolved == &request_id
            )
        })
    });

    assert!(
        !output.exists(),
        "permission downgrade must prevent tool mutation"
    );
    assert!(events.iter().all(|event| {
        !matches!(
            event.kind,
            RuntimeEventKind::ToolCallFinished { success: true, .. }
        )
    }));
}

#[test]
fn runtime_supervisor_rejects_tool_approval_after_permission_epoch_round_trip() {
    for (case, downgrade, restore) in [
        (
            "permission",
            RuntimeCommand::SetPermissionLevel {
                level: PermissionLevel::ReadOnly,
            },
            RuntimeCommand::SetPermissionLevel {
                level: PermissionLevel::Ask,
            },
        ),
        (
            "work_mode",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Build,
            },
        ),
    ] {
        let cwd = temp_dir(&format!("runtime_supervisor_{case}_epoch_cwd"));
        let home = temp_dir(&format!("runtime_supervisor_{case}_epoch_home"));
        let output = cwd.join("should-not-run.txt");
        let mut input = ToolInput::new();
        input.insert(
            "command".to_string(),
            format!("printf blocked > {}", output.display()),
        );
        let provider = Box::new(SequenceProvider::new(vec![vec![
            ModelEvent::ToolCall(ToolCall {
                id: format!("tool_shell_{case}_epoch"),
                name: "shell".to_string(),
                input,
            }),
            ModelEvent::Done,
        ]]));
        let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let supervisor = RuntimeSupervisor::start(engine);
        let owner = owner_for_lane(&format!("lane-{case}-epoch"));

        supervisor
            .send_command_from_owner(
                owner.clone(),
                format!("cmd_input_{case}_epoch"),
                RuntimeCommand::SubmitUserInput {
                    content: "run one mutating tool".to_string(),
                },
            )
            .unwrap();
        let requested = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
        });
        let request_id = approval_id(&requested);
        supervisor
            .send_command_from_owner(owner.clone(), format!("cmd_{case}_downgrade"), downgrade)
            .unwrap();
        supervisor
            .send_command_from_owner(owner.clone(), format!("cmd_{case}_restore"), restore)
            .unwrap();
        supervisor
            .send_command_from_owner(
                owner,
                format!("cmd_{case}_allow_stale"),
                RuntimeCommand::RespondToApproval {
                    request_id: request_id.clone(),
                    response: ApprovalResponse::allow_once(None),
                },
            )
            .unwrap();
        collect_events_until(&supervisor, Duration::from_secs(2), |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved {
                        request_id: resolved,
                        decision: ApprovalDecision::Deny,
                        ..
                    } if resolved == &request_id
                )
            })
        });

        assert!(
            !output.exists(),
            "{case} epoch round trip must invalidate the stale approval"
        );
    }
}

#[test]
fn runtime_supervisor_approval_cancel_pending_tool_resolves_once_with_stored_identity() {
    let cwd = temp_dir("runtime_supervisor_approval_cancel_tool_cwd");
    let home = temp_dir("runtime_supervisor_approval_cancel_tool_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf should-not-run".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_cancel".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-cancel-tool");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_input_cancel_tool",
            RuntimeCommand::SubmitUserInput {
                content: "run command then cancel approval".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let (request_id, audit_id) = approval_identity(&events);

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_cancel_tool",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));
    events.extend(collect_events_for(&supervisor, Duration::from_millis(150)));

    assert_eq!(
        approval_resolved_count(&events, &request_id),
        1,
        "cancel should emit exactly one approval resolution: {events:#?}"
    );
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Deny,
                audit_id: resolved_audit,
                owner: resolved_owner,
            } if resolved == &request_id && resolved_audit == &audit_id && resolved_owner == &owner
        )
    }));
    assert!(events.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished { success: true, .. }
        )
    }));
}

#[test]
fn runtime_supervisor_approval_cancel_pending_context_resolves_once_without_reading() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (mut engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_approval_cancel_context_cwd",
        "cancelled context body",
    );
    engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let read_started = Arc::new(AtomicBool::new(false));
    let read_started_for_hook = Arc::clone(&read_started);
    set_retrieve_context_test_hook(Some(Arc::new(move |_control| {
        read_started_for_hook.store(true, Ordering::SeqCst);
    })));
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-cancel-context");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_retrieve_cancel_context",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate then cancel".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let (request_id, audit_id) = approval_identity(&events);

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_cancel_context",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));
    events.extend(collect_events_for(&supervisor, Duration::from_millis(150)));
    set_retrieve_context_test_hook(None);

    assert!(!read_started.load(Ordering::SeqCst));
    assert_eq!(approval_resolved_count(&events, &request_id), 1);
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Deny,
                audit_id: resolved_audit,
                owner: resolved_owner,
            } if resolved == &request_id && resolved_audit == &audit_id && resolved_owner == &owner
        )
    }));
}

#[test]
fn runtime_supervisor_approval_wrong_owner_cancel_does_not_resolve_pending_tool() {
    let cwd = temp_dir("runtime_supervisor_approval_wrong_cancel_cwd");
    let home = temp_dir("runtime_supervisor_approval_wrong_cancel_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf should-not-run".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_wrong_cancel".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-cancel-owner");
    let wrong_owner = owner_for_lane("lane-cancel-other");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_input_wrong_cancel",
            RuntimeCommand::SubmitUserInput {
                content: "run command then wrong owner cancel".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = approval_id(&events);

    supervisor
        .send_command_from_owner(
            wrong_owner,
            "cmd_wrong_owner_cancel",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::CommandRejected { command_id, reason }
                        if command_id == "cmd_wrong_owner_cancel" && reason.contains("owner mismatch")
                )
            })
        },
    ));
    assert_eq!(approval_resolved_count(&events, &request_id), 0);

    supervisor
        .send_command_from_owner(
            owner,
            "cmd_correct_owner_cancel",
            RuntimeCommand::CancelActiveTurn,
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));
    assert_eq!(approval_resolved_count(&events, &request_id), 1);
}

#[test]
fn runtime_supervisor_approval_cancel_after_context_allow_does_not_re_deny_or_read() {
    let _guard = RETRIEVE_CONTEXT_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("retrieve context hook lock");
    let (mut engine, handle_id) = supervisor_engine_with_context(
        "runtime_supervisor_approval_cancel_after_allow_cwd",
        "cancel after allow body must not be read",
    );
    engine.add_permission_rule_for_test(context_read_rule(PermissionBehavior::Ask));
    let read_started = Arc::new(AtomicBool::new(false));
    let read_started_for_hook = Arc::clone(&read_started);
    set_retrieve_context_test_hook(Some(Arc::new(move |_control| {
        read_started_for_hook.store(true, Ordering::SeqCst);
    })));
    let hook_entered = Arc::new(AtomicBool::new(false));
    let hook_entered_for_hook = Arc::clone(&hook_entered);
    set_before_context_resume_enqueue_hook(Some(Arc::new(move |control| {
        hook_entered_for_hook.store(true, Ordering::SeqCst);
        control.cancel();
    })));
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-cancel-after-allow");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_retrieve_cancel_after_allow",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate then cancel after allow".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = approval_id(&events);

    supervisor
        .send_command_from_owner(
            owner,
            "cmd_allow_context_before_cancel",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved {
                        request_id: resolved,
                        decision: ApprovalDecision::Allow { .. },
                        ..
                    } if resolved == &request_id
                )
            })
        },
    ));
    events.extend(collect_events_for(&supervisor, Duration::from_millis(250)));
    set_before_context_resume_enqueue_hook(None);
    set_retrieve_context_test_hook(None);

    assert_eq!(approval_resolved_count(&events, &request_id), 1);
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Allow {
                    scope: ApprovalScope::Once
                },
                ..
            } if resolved == &request_id
        )
    }));
    assert!(events.iter().all(|event| {
        !matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved {
                request_id: resolved,
                decision: ApprovalDecision::Deny,
                ..
            } if resolved == &request_id
        )
    }));
    assert!(hook_entered.load(Ordering::SeqCst));
    assert!(!read_started.load(Ordering::SeqCst));
}

#[test]
fn runtime_supervisor_approval_rejects_unadvertised_or_wrong_scopes_without_resolving() {
    let cwd = temp_dir("runtime_supervisor_approval_scope_cwd");
    let home = temp_dir("runtime_supervisor_approval_scope_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf scoped".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell_scope".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-scope");

    supervisor
        .send_command_from_owner(
            owner.clone(),
            "cmd_input_scope",
            RuntimeCommand::SubmitUserInput {
                content: "run scoped command".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ApprovalRequested { approval }
                    if approval.allowed_scopes.contains(&ApprovalScope::Once)
                        && approval.allowed_scopes.contains(&ApprovalScope::Session {
                            session_id: "session-lane-scope".to_string()
                        })
                        && !approval.allowed_scopes.iter().any(|scope| matches!(
                            scope,
                            ApprovalScope::RepoAllowlist { .. }
                        ))
            )
        })
    });
    let request_id = approval_id(&events);

    for (command_id, scope) in [
        (
            "cmd_wrong_session_scope",
            ApprovalScope::Session {
                session_id: "session-other".to_string(),
            },
        ),
        (
            "cmd_repo_scope",
            ApprovalScope::RepoAllowlist {
                paths: vec!["src/lib.rs".to_string()],
            },
        ),
    ] {
        supervisor
            .send_command_from_owner(
                owner.clone(),
                command_id,
                RuntimeCommand::RespondToApproval {
                    request_id: request_id.clone(),
                    response: ApprovalResponse {
                        decision: ApprovalDecision::Allow { scope },
                        feedback: None,
                    },
                },
            )
            .unwrap();
        events.extend(collect_events_until(
            &supervisor,
            Duration::from_secs(2),
            |events| {
                events.iter().any(|event| {
                    matches!(
                        &event.kind,
                        RuntimeEventKind::CommandRejected { command_id: rejected, reason }
                            if rejected == command_id && reason.contains("scope is not allowed")
                    )
                })
            },
        ));
    }
    assert_eq!(approval_resolved_count(&events, &request_id), 0);

    supervisor
        .send_command_from_owner(
            owner,
            "cmd_allowed_scope",
            RuntimeCommand::RespondToApproval {
                request_id: request_id.clone(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                        if resolved == &request_id
                )
            })
        },
    ));
    assert_eq!(approval_resolved_count(&events, &request_id), 1);
}

#[test]
fn runtime_supervisor_approval_advertises_repo_allowlist_for_candidate_paths() {
    let cwd = temp_dir("runtime_supervisor_approval_repo_scope_cwd");
    let home = temp_dir("runtime_supervisor_approval_repo_scope_home");
    let mut input = ToolInput::new();
    input.insert("path".to_string(), "src/lib.rs".to_string());
    input.insert("content".to_string(), "updated".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_write_repo_scope".to_string(),
            name: "write_file".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    let owner = owner_for_lane("lane-repo-scope");

    supervisor
        .send_command_from_owner(
            owner,
            "cmd_input_repo_scope",
            RuntimeCommand::SubmitUserInput {
                content: "write path with repo scope".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ApprovalRequested { approval }
                    if approval.target.canonical_ref.as_deref() == Some("src/lib.rs")
                        && approval.allowed_scopes.contains(&ApprovalScope::RepoAllowlist {
                            paths: vec!["src/lib.rs".to_string()]
                        })
            )
        })
    });

    assert!(!approval_id(&events).is_empty());
}

#[test]
fn runtime_supervisor_starts_agent_dag_without_provider_turn() {
    let cwd = temp_dir("runtime_supervisor_agent_dag_cwd");
    let home = temp_dir("runtime_supervisor_agent_dag_home");
    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Complete 0.2.2 role runtime".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_planner".to_string(),
                        role: AgentRole::Planner,
                        title: "Plan implementation".to_string(),
                        objective: "Split the work".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["plan".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_coder".to_string(),
                        role: AgentRole::Coder,
                        title: "Implement contracts".to_string(),
                        objective: "Add runtime contracts".to_string(),
                        dependencies: vec!["task_planner".to_string()],
                        workspace: None,
                        file_scope: vec!["crates/types".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string(), "test".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                ],
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .filter(|event| matches!(event.kind, RuntimeEventKind::TaskUpdated { .. }))
            .count()
            >= 2
            && events
                .iter()
                .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
            && events
                .iter()
                .filter(|event| matches!(event.kind, RuntimeEventKind::MergeGateUpdated { .. }))
                .count()
                >= 2
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_agent_dag"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AgentDagUpdated { dag }
                if dag.goal == "Complete 0.2.2 role runtime"
                    && dag.tasks.len() == 2
                    && dag.tasks[0].role == AgentRole::Planner
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_coder"
                    && task.role == AgentRole::Coder
                    && task.parent_id.as_deref() == Some("task_planner")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.task_id == "task_coder"
                    && gate.required_evidence == vec!["patch".to_string(), "test".to_string()]
        )
    }));
}

#[test]
fn runtime_supervisor_runs_agent_task_through_provider_and_merge_gate() {
    let cwd = temp_dir("runtime_supervisor_agent_task_cwd");
    let home = temp_dir("runtime_supervisor_agent_task_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "Plan: split runtime, workflow, and tests.".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Complete provider-backed role execution".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan role execution".to_string(),
                    objective: "Design the next implementation slice".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_planner"
                        && task.status == AgentTaskStatus::Done
                        && task.result.as_deref().is_some_and(|result| {
                            result.contains("Plan: split runtime")
                        })
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::EvidenceRecorded { evidence }
                    if evidence.kind == "plan"
                        && evidence.summary.contains("canonical plan evidence")
                        && evidence.canonical.is_some()
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ContextUpdated { context }
                    if context.task_id == "task_planner"
                        && context.policy.contains("agent-role")
                        && context.sources.iter().any(|source| source.name == "agent-role")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.task_id == "task_planner"
                        && !gate.status.is_open()
                        && gate.evidence_ids.iter().any(|id| id.contains("task_planner"))
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content.contains("Plan: split runtime")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_agent_start"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_planner"
                    && task.status == AgentTaskStatus::Done
                    && task.result.as_deref().is_some_and(|result| {
                        result.contains("Plan: split runtime")
                    })
        )
    }));
    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| kinds.contains("task_updated"))
            && event
                .payload
                .get("runtime_events_json")
                .is_some_and(|json| json.contains("task_planner"))
    }));
}

#[test]
fn runtime_supervisor_builds_role_specific_context_bundle_sources() {
    let cwd = temp_dir("runtime_supervisor_role_context_cwd");
    let home = temp_dir("runtime_supervisor_role_context_home");
    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Build role-specific contexts".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_plan".to_string(),
                        role: AgentRole::Planner,
                        title: "Plan context".to_string(),
                        objective: "Plan the architecture".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["plan".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_code".to_string(),
                        role: AgentRole::Coder,
                        title: "Code context".to_string(),
                        objective: "Implement the change".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_review".to_string(),
                        role: AgentRole::Reviewer,
                        title: "Review context".to_string(),
                        objective: "Review the change".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["review".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_test".to_string(),
                        role: AgentRole::Tester,
                        title: "Test context".to_string(),
                        objective: "Verify behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime/src/tests".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_docs".to_string(),
                        role: AgentRole::DocWriter,
                        title: "Docs context".to_string(),
                        objective: "Update documentation".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["doc_update".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let planner = start_agent_task_and_capture_context(&supervisor, "task_plan");
    assert_context_source(&planner, "role-planning-context", "role-guidance");
    assert_context_source(&planner, "agent-file-scope", "file-scope");
    assert_context_source(&planner, "agent-evidence-contract", "evidence-contract");

    let coder = start_agent_task_and_capture_context(&supervisor, "task_code");
    assert_context_source(&coder, "role-implementation-context", "role-guidance");
    assert_context_source(&coder, "agent-file-scope", "file-scope");
    assert_context_source(&coder, "agent-evidence-contract", "evidence-contract");

    let reviewer = start_agent_task_and_capture_context(&supervisor, "task_review");
    assert_context_source(&reviewer, "role-review-context", "role-guidance");
    assert_ne!(
        context_source_summary(&planner, "role-planning-context", "role-guidance"),
        context_source_summary(&reviewer, "role-review-context", "role-guidance")
    );

    let tester = start_agent_task_and_capture_context(&supervisor, "task_test");
    assert_context_source(&tester, "role-verification-context", "role-guidance");

    let doc_writer = start_agent_task_and_capture_context(&supervisor, "task_docs");
    assert_context_source(&doc_writer, "role-documentation-context", "role-guidance");
}

#[test]
fn agent_task_provider_request_uses_final_role_context_bundle() {
    let cwd = temp_dir("runtime_supervisor_provider_role_bundle_cwd");
    let home = temp_dir("runtime_supervisor_provider_role_bundle_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct RuntimeContractRoleBundle {}\n",
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::success(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(1_000, 8_000);
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

    let _ = engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Build role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_plan_provider_bundle".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan provider bundle".to_string(),
                    objective: format!(
                        "Plan with role guidance without leaking sk-plan-secret from {}",
                        cwd.display()
                    ),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_start_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_plan_provider_bundle".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let provider_manifest = requests[0]
        .messages
        .iter()
        .find(|message| message.content.contains("Viden ContextBundle"))
        .expect("provider context manifest")
        .content
        .clone();
    assert!(provider_manifest.contains("Bundle: ctx-agent-task_plan_provider_bundle"));
    assert!(provider_manifest.contains("Scope: task:task_plan_provider_bundle"));
    assert!(provider_manifest.contains("role-planning-context"));
    assert!(provider_manifest.contains("Snippet:"));
    assert!(provider_manifest.contains("Focus on requirements"));
    assert!(provider_manifest.contains("crates/runtime/src/runtime_contract.rs"));
    assert!(provider_manifest.contains("handle="));
    assert!(provider_manifest.contains("view="));
    assert!(provider_manifest.contains("quality="));
    assert!(!provider_manifest.contains("sk-plan-secret"));
    assert!(!provider_manifest.contains(cwd.to_string_lossy().as_ref()));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextUpdated { context }
                if context.bundle_id == "ctx-agent-task_plan_provider_bundle"
                    && context.sources.iter().any(|source| {
                        source.name == "role-planning-context"
                            && source.handle_id.is_some()
                            && source.view_id.is_some()
                            && source.content_sha256.is_some()
                            && source.quality_id.is_some()
                    })
        )
    }));
}

#[test]
fn reviewer_agent_task_provider_request_uses_review_role_context() {
    let cwd = temp_dir("runtime_supervisor_reviewer_provider_bundle_cwd");
    let home = temp_dir("runtime_supervisor_reviewer_provider_bundle_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct ReviewRoleScopedBundle {}\n",
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::success(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(1_000, 8_000);
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Review role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_review_provider_bundle".to_string(),
                    role: AgentRole::Reviewer,
                    title: "Review provider bundle".to_string(),
                    objective: "Review with role guidance".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["review".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    engine
        .handle_runtime_command(
            "cmd_start_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_review_provider_bundle".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let provider_manifest = provider_manifest(&requests[0]);
    assert!(provider_manifest.contains("Bundle: ctx-agent-task_review_provider_bundle"));
    assert!(provider_manifest.contains("Scope: task:task_review_provider_bundle"));
    assert!(provider_manifest.contains("role-review-context"));
    assert!(provider_manifest.contains("Snippet:"));
    assert!(provider_manifest.contains("Focus on behavioral regressions"));
    assert!(!provider_manifest.contains("role-planning-context"));
    assert!(!provider_manifest.contains(cwd.to_string_lossy().as_ref()));
}

#[test]
fn agent_task_context_overflow_retry_preserves_role_scoped_bundle() {
    let cwd = temp_dir("runtime_supervisor_agent_retry_role_bundle_cwd");
    let home = temp_dir("runtime_supervisor_agent_retry_role_bundle_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct RetryRoleScopedBundle {}\n",
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::with_errors(
        Arc::clone(&requests),
        vec!["context_overflow: current request exceeded provider context".to_string()],
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Retry role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_retry_role_bundle".to_string(),
                    role: AgentRole::Planner,
                    title: "Retry planner".to_string(),
                    objective: "Plan while retrying context overflow".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_start_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_retry_role_bundle".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| {
        matches!(
            event,
            RuntimeEvent { kind: RuntimeEventKind::AssistantDelta { content, .. }, .. }
                if content.contains("recorded")
        )
    }));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let first_manifest = provider_manifest(&requests[0]);
    let second_manifest = provider_manifest(&requests[1]);
    assert!(first_manifest.contains("Bundle: ctx-agent-task_retry_role_bundle"));
    assert!(second_manifest.contains("Bundle: ctx-agent-task_retry_role_bundle"));
    assert!(first_manifest.contains("Policy: agent-role-planner-priority-budget"));
    assert!(second_manifest.contains("Policy: agent-role-planner-priority-budget-strict-retry"));
    assert!(first_manifest.contains("role-planning-context"));
    assert!(second_manifest.contains("role-planning-context"));
    assert!(first_manifest.contains("Scope: task:task_retry_role_bundle"));
    assert!(second_manifest.contains("Scope: task:task_retry_role_bundle"));
    assert!(second_manifest.contains("handle="));
    assert!(second_manifest.contains("view="));
    assert!(second_manifest.contains("strict-retry"));
    let first_role_refs = manifest_source_refs(&first_manifest, "role-planning-context");
    let second_role_refs = manifest_source_refs(&second_manifest, "role-planning-context");
    assert_eq!(first_role_refs.handle, second_role_refs.handle);
    assert_eq!(first_role_refs.item, second_role_refs.item);
    assert_eq!(first_role_refs.raw_hash, second_role_refs.raw_hash);
    assert_ne!(first_role_refs.view, second_role_refs.view);
    assert_ne!(first_role_refs.view_hash, second_role_refs.view_hash);
    assert!(second_manifest.contains("Soft budget: 12000"));
    assert!(second_manifest.contains("Hard limit: 32000"));
}

#[test]
fn agent_task_hard_context_limit_rejects_before_provider_request() {
    let cwd = temp_dir("runtime_supervisor_agent_hard_budget_cwd");
    let home = temp_dir("runtime_supervisor_agent_hard_budget_home");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::success(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(10, 20);
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

    let _ = engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Reject huge role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_hard_budget_agent".to_string(),
                    role: AgentRole::Coder,
                    title: "Huge role context".to_string(),
                    objective: "x ".repeat(500),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "ask".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_start_hard_budget_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_hard_budget_agent".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(requests.lock().unwrap().is_empty());
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextBudgetExceeded { budget }
                if budget.exceeded && budget.hard_token_limit == 20
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::Error { error }
                if error.message.contains("context hard limit")
                    && error.message.contains("task_hard_budget_agent")
        )
    }));
}

#[test]
fn runtime_supervisor_selects_role_specific_files_for_agent_context() {
    let cwd = temp_dir("runtime_supervisor_role_file_context_cwd");
    let home = temp_dir("runtime_supervisor_role_file_context_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "fn runtime_contract() {}",
    );
    write_test_file(
        &cwd.join("crates/runtime/src/tests/runtime_supervisor_tests.rs"),
        "fn runtime_supervisor_test() {}",
    );
    write_test_file(
        &cwd.join("crates/runtime/Cargo.toml"),
        "[package]\nname = \"viden-runtime\"",
    );
    write_test_file(&cwd.join("docs/architecture.md"), "# Architecture");
    write_test_file(&cwd.join("README.md"), "# Viden");

    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Select role files".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_code_files".to_string(),
                        role: AgentRole::Coder,
                        title: "Code file context".to_string(),
                        objective: "Implement runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_test_files".to_string(),
                        role: AgentRole::Tester,
                        title: "Test file context".to_string(),
                        objective: "Verify runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_doc_files".to_string(),
                        role: AgentRole::DocWriter,
                        title: "Doc file context".to_string(),
                        objective: "Update architecture docs".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["doc_update".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let coder = start_agent_task_and_capture_context(&supervisor, "task_code_files");
    let coder_files = context_source_summary(&coder, "role-selected-files", "selected-files");
    assert!(coder_files.contains("crates/runtime/src/runtime_contract.rs"));
    assert!(!coder_files.contains("docs/architecture.md"));

    let tester = start_agent_task_and_capture_context(&supervisor, "task_test_files");
    let tester_files = context_source_summary(&tester, "role-selected-files", "selected-files");
    assert!(tester_files.contains("crates/runtime/src/tests/runtime_supervisor_tests.rs"));
    assert!(tester_files.contains("crates/runtime/Cargo.toml"));

    let doc_writer = start_agent_task_and_capture_context(&supervisor, "task_doc_files");
    let doc_files = context_source_summary(&doc_writer, "role-selected-files", "selected-files");
    assert!(doc_files.contains("docs/architecture.md"));
    assert!(!doc_files.contains("README.md"));
}

#[test]
fn runtime_supervisor_selects_role_specific_symbols_for_agent_context() {
    let cwd = temp_dir("runtime_supervisor_role_symbol_context_cwd");
    let home = temp_dir("runtime_supervisor_role_symbol_context_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct RuntimeSupervisor {}\nimpl RuntimeSupervisor {\n    pub fn start_agent_task(&self) {}\n}\nfn helper() {}\n",
    );
    write_test_file(
        &cwd.join("crates/runtime/src/tests/runtime_supervisor_tests.rs"),
        "#[test]\nfn runtime_supervisor_starts_agent_task() {}\nfn helper_fixture() {}\n",
    );

    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Select role symbols".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_code_symbols".to_string(),
                        role: AgentRole::Coder,
                        title: "Code symbol context".to_string(),
                        objective: "Implement runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_test_symbols".to_string(),
                        role: AgentRole::Tester,
                        title: "Test symbol context".to_string(),
                        objective: "Verify runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let coder = start_agent_task_and_capture_context(&supervisor, "task_code_symbols");
    let coder_symbols = context_source_summary(&coder, "role-selected-symbols", "selected-symbols");
    assert!(
        coder_symbols.contains("crates/runtime/src/runtime_contract.rs::struct RuntimeSupervisor")
    );
    assert!(coder_symbols.contains("crates/runtime/src/runtime_contract.rs::fn start_agent_task"));

    let tester = start_agent_task_and_capture_context(&supervisor, "task_test_symbols");
    let tester_symbols =
        context_source_summary(&tester, "role-selected-symbols", "selected-symbols");
    assert!(
        tester_symbols
            .lines()
            .next()
            .is_some_and(|line| line.contains("runtime_supervisor_starts_agent_task")),
        "tester should prioritize test symbols: {tester_symbols}"
    );
}

#[test]
fn runtime_supervisor_adds_lsp_diagnostics_to_agent_context_bundle() {
    let cwd = temp_dir("runtime_supervisor_lsp_context_cwd");
    let home = temp_dir("runtime_supervisor_lsp_context_home");
    let fake_lsp_dir = temp_dir("runtime_supervisor_lsp_context_server");
    write_test_file(
        &cwd.join("crates/runtime/src/lib.rs"),
        "pub fn broken() {\n    let value = missing;\n}\n",
    );

    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.lsp_runtime = Arc::new(LspRuntime::new(fake_lsp_registry(&fake_lsp_dir)));
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enrich role context with diagnostics".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_lsp_context".to_string(),
                    role: AgentRole::Coder,
                    title: "LSP context".to_string(),
                    objective: "Fix the diagnostic".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let context = start_agent_task_and_capture_context(&supervisor, "task_lsp_context");
    let diagnostics = context_source_summary(&context, "role-lsp-diagnostics", "lsp-diagnostics");
    assert!(diagnostics.contains("LSP diagnostics:"));
    assert!(
        diagnostics.contains("crates/runtime/src/lib.rs"),
        "diagnostics summary should keep project-relative path, got: {diagnostics}"
    );
    assert!(diagnostics.contains("fake-lsp/E100"));
    assert!(diagnostics.contains("fake diagnostic"));
}

#[test]
fn runtime_supervisor_applies_read_only_role_policy_to_agent_tools() {
    let cwd = temp_dir("runtime_supervisor_role_policy_cwd");
    let home = temp_dir("runtime_supervisor_role_policy_home");
    let blocked_file = cwd.join("should_not_exist.txt");
    let mut shell_input = ToolInput::new();
    shell_input.insert(
        "command".to_string(),
        "printf blocked > should_not_exist.txt".to_string(),
    );
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::ToolCall(
        ToolCall {
            id: "tool_shell".to_string(),
            name: "shell".to_string(),
            input: shell_input,
        },
    )]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enforce role policy".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan without mutation".to_string(),
                    objective: "Planner must not run mutating tools".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start_read_only",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if evidence.summary.contains("reason: PlanMode")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == WorkMode::Build
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                success: false,
                evidence: Some(evidence),
                ..
            } if evidence.summary.contains("tool: shell")
                && evidence.summary.contains("reason: PlanMode")
        )
    }));
    assert!(
        !events
            .iter()
            .any(|event| { matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }) })
    );
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Build
        )
    }));
    assert!(!blocked_file.exists());
}

#[test]
fn runtime_supervisor_applies_role_policy_matrix_to_tools() {
    let cwd = temp_dir("runtime_supervisor_role_policy_matrix_cwd");
    let home = temp_dir("runtime_supervisor_role_policy_matrix_home");
    write_test_file(&cwd.join("docs/guide.md"), "old docs");
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "old code");

    let mut test_shell = ToolInput::new();
    test_shell.insert("command".to_string(), "cargo test --help".to_string());
    let mut tester_write = ToolInput::new();
    tester_write.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    tester_write.insert("content".to_string(), "mutated".to_string());
    let mut docs_write = ToolInput::new();
    docs_write.insert("path".to_string(), "docs/guide.md".to_string());
    docs_write.insert("content".to_string(), "new docs".to_string());
    let mut code_write = ToolInput::new();
    code_write.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    code_write.insert("content".to_string(), "new code".to_string());

    let provider = Box::new(SequenceProvider::new(vec![
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_test_shell".to_string(),
                name: "shell".to_string(),
                input: test_shell,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_tester_write".to_string(),
                name: "write_file".to_string(),
                input: tester_write,
            }),
            ModelEvent::AssistantText {
                content: "tester policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_docs_write".to_string(),
                name: "write_file".to_string(),
                input: docs_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_code_write".to_string(),
                name: "write_file".to_string(),
                input: code_write,
            }),
            ModelEvent::AssistantText {
                content: "doc policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enforce role matrix".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_tester_policy".to_string(),
                        role: AgentRole::Tester,
                        title: "Tester matrix".to_string(),
                        objective: "Run verification without mutating files".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "tester_verification".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_docs_policy".to_string(),
                        role: AgentRole::DocWriter,
                        title: "Doc writer matrix".to_string(),
                        objective: "Update docs only".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["doc_update".to_string()],
                        permission_policy: "docs_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_tester_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_tester_policy".to_string(),
            },
        )
        .unwrap();
    let tester_events = collect_events_until(&supervisor, Duration::from_secs(5), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "shell"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !tester_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        tester_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "shell"
            )
        }),
        "tester shell should run without approval: {tester_events:#?}"
    );
    assert!(
        tester_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "tester write should be denied by role policy: {tester_events:#?}"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("crates/runtime/src/lib.rs")).unwrap(),
        "old code"
    );

    supervisor
        .send_command(
            "cmd_docs_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_docs_policy".to_string(),
            },
        )
        .unwrap();
    let doc_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "write_file"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !doc_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        doc_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "write_file"
            )
        }),
        "doc writer should update docs without approval: {doc_events:#?}"
    );
    assert!(
        doc_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "doc writer should be denied on code files: {doc_events:#?}"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/guide.md")).unwrap(),
        "new docs"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("crates/runtime/src/lib.rs")).unwrap(),
        "old code"
    );
}

#[test]
fn runtime_supervisor_applies_extended_agent_role_policy_matrix_to_tools() {
    let cwd = temp_dir("runtime_supervisor_extended_role_policy_matrix_cwd");
    let home = temp_dir("runtime_supervisor_extended_role_policy_matrix_home");
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "old code");
    write_test_file(&cwd.join("apps/tui/src/main.rs"), "old tui");
    write_test_file(&cwd.join("docs/release.md"), "old release docs");

    let mut coder_code_write = ToolInput::new();
    coder_code_write.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    coder_code_write.insert("content".to_string(), "coder code".to_string());
    let mut coder_docs_write = ToolInput::new();
    coder_docs_write.insert("path".to_string(), "docs/release.md".to_string());
    coder_docs_write.insert("content".to_string(), "bad coder docs".to_string());
    let mut release_test_shell = ToolInput::new();
    release_test_shell.insert("command".to_string(), "cargo test --help".to_string());
    let mut release_docs_write = ToolInput::new();
    release_docs_write.insert("path".to_string(), "docs/release.md".to_string());
    release_docs_write.insert("content".to_string(), "release docs".to_string());
    let mut release_push = ToolInput::new();
    release_push.insert("remote".to_string(), "origin".to_string());
    release_push.insert("branch".to_string(), "main".to_string());
    let mut external_write = ToolInput::new();
    external_write.insert("path".to_string(), "docs/release.md".to_string());
    external_write.insert("content".to_string(), "external write".to_string());
    let mut external_shell = ToolInput::new();
    external_shell.insert("command".to_string(), "printf external".to_string());

    let provider = Box::new(SequenceProvider::new(vec![
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_coder_code_write".to_string(),
                name: "write_file".to_string(),
                input: coder_code_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_coder_docs_write".to_string(),
                name: "write_file".to_string(),
                input: coder_docs_write,
            }),
            ModelEvent::AssistantText {
                content: "coder policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_release_test_shell".to_string(),
                name: "shell".to_string(),
                input: release_test_shell,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_release_docs_write".to_string(),
                name: "write_file".to_string(),
                input: release_docs_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_release_push".to_string(),
                name: "git_push".to_string(),
                input: release_push,
            }),
            ModelEvent::AssistantText {
                content: "release policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_external_write".to_string(),
                name: "write_file".to_string(),
                input: external_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_external_shell".to_string(),
                name: "shell".to_string(),
                input: external_shell,
            }),
            ModelEvent::AssistantText {
                content: "external policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_extended_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enforce extended agent role matrix".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_coder_policy".to_string(),
                        role: AgentRole::Coder,
                        title: "Coder matrix".to_string(),
                        objective: "Mutate only declared code scope".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "scoped_mutation".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_release_policy".to_string(),
                        role: AgentRole::ReleaseOperator,
                        title: "Release matrix".to_string(),
                        objective: "Run release verification without publishing".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["release_gate".to_string()],
                        permission_policy: "release_gate".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_external_policy".to_string(),
                        role: AgentRole::Researcher,
                        title: "Research matrix".to_string(),
                        objective: "Keep research work read-only until explicitly promoted"
                            .to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["research".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_coder_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder_policy".to_string(),
            },
        )
        .unwrap();
    let coder_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "write_file"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !coder_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(
        fs::read_to_string(cwd.join("crates/runtime/src/lib.rs")).unwrap(),
        "coder code"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/release.md")).unwrap(),
        "old release docs"
    );

    supervisor
        .send_command(
            "cmd_release_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_release_policy".to_string(),
            },
        )
        .unwrap();
    let release_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "shell"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "git_push"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !release_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/release.md")).unwrap(),
        "release docs"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("apps/tui/src/main.rs")).unwrap(),
        "old tui"
    );

    supervisor
        .send_command(
            "cmd_external_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_external_policy".to_string(),
            },
        )
        .unwrap();
    let external_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_external_policy" && task.status == AgentTaskStatus::Done
            )
        })
    });
    // Plan mode's stable denial contract is structured permission evidence;
    // do not make this policy assertion depend on ToolCallFinished projection.
    for tool_name in ["write_file", "shell"] {
        assert!(external_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::EvidenceRecorded { evidence }
                    if evidence.summary.contains("Summary: decision=deny")
                        && evidence.summary.contains(&format!("tool: {tool_name}"))
                        && evidence.summary.contains("reason: PlanMode")
            )
        }));
    }
    assert!(
        !external_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/release.md")).unwrap(),
        "release docs"
    );
}

#[test]
fn runtime_supervisor_applies_scoped_git_policy_to_agent_tasks() {
    let cwd = temp_dir("runtime_supervisor_scoped_git_policy_cwd");
    let home = temp_dir("runtime_supervisor_scoped_git_policy_home");
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "old code\n");
    write_test_file(&cwd.join("docs/release.md"), "old docs\n");
    init_git_repo(&cwd);
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "new code\n");
    write_test_file(&cwd.join("docs/release.md"), "new docs\n");

    let mut scoped_git_add = ToolInput::new();
    scoped_git_add.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    let mut unscoped_git_add = ToolInput::new();
    unscoped_git_add.insert("path".to_string(), "docs/release.md".to_string());
    let mut git_commit = ToolInput::new();
    git_commit.insert("message".to_string(), "agent commit".to_string());

    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_scoped_git_add".to_string(),
            name: "git_add".to_string(),
            input: scoped_git_add,
        }),
        ModelEvent::ToolCall(ToolCall {
            id: "tool_unscoped_git_add".to_string(),
            name: "git_add".to_string(),
            input: unscoped_git_add,
        }),
        ModelEvent::ToolCall(ToolCall {
            id: "tool_git_commit".to_string(),
            name: "git_commit".to_string(),
            input: git_commit,
        }),
        ModelEvent::AssistantText {
            content: "git policy checked".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Apply scoped Git policy".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_scoped_git_policy".to_string(),
                    role: AgentRole::Coder,
                    title: "Scoped Git policy".to_string(),
                    objective: "Stage only files in task scope".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_scoped_git_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_scoped_git_policy".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(5), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "git_add"
            )
        }) && events
            .iter()
            .filter(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ToolCallFinished {
                        success: false,
                        evidence: Some(evidence),
                        ..
                    } if evidence.summary.contains("reason: RuleDeny")
                )
            })
            .count()
            >= 2
    });
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "git_add"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "unscoped git_add should be denied by role policy: {events:#?}"
    );
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "git_commit"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "git_commit should be denied by role policy: {events:#?}"
    );

    let staged = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    let staged = String::from_utf8_lossy(&staged.stdout);
    assert!(staged.contains("crates/runtime/src/lib.rs"));
    assert!(!staged.contains("docs/release.md"));
}

#[test]
fn runtime_supervisor_accepts_and_rejects_merge_gate_decisions() {
    let cwd = temp_dir("runtime_supervisor_merge_gate_decision_cwd");
    let home = temp_dir("runtime_supervisor_merge_gate_decision_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "Plan: produce merge-gate evidence.".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Decide merge gate".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan merge decision".to_string(),
                    objective: "Create evidence for a gate decision".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_planner"
                        && !gate.status.is_open()
            )
        })
    });

    supervisor
        .send_command(
            "cmd_reject_gate",
            RuntimeCommand::RejectMergeGate {
                gate_id: "gate-task_planner".to_string(),
                reason: "needs reviewer evidence".to_string(),
            },
        )
        .unwrap();
    let rejected = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_planner"
                        && gate.status == MergeGateStatus::NeedsChanges
                        && gate.decision.as_deref() == Some("needs reviewer evidence")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_planner"
                        && task.decision.as_deref() == Some("needs reviewer evidence")
            )
        })
    });
    assert!(rejected.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_reject_gate"
        )
    }));

    supervisor
        .send_command(
            "cmd_accept_gate",
            RuntimeCommand::AcceptMergeGate {
                gate_id: "gate-task_planner".to_string(),
                decision: Some("accepted after review".to_string()),
            },
        )
        .unwrap();
    let accepted = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_planner"
                        && gate.status == MergeGateStatus::Accepted
                        && gate.decision.as_deref() == Some("accepted after review")
            )
        })
    });
    assert!(accepted.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_accept_gate"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("command_id")
                .is_some_and(|id| id == "cmd_reject_gate")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| {
                    kinds.contains("merge_gate_updated") && kinds.contains("task_updated")
                })
    }));
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("command_id")
                .is_some_and(|id| id == "cmd_accept_gate")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| {
                    kinds.contains("merge_gate_updated") && kinds.contains("task_updated")
                })
    }));
}

#[test]
fn runtime_supervisor_rejects_unknown_agent_artifact_evidence() {
    let cwd = temp_dir("runtime_supervisor_unknown_artifact_evidence_cwd");
    let home = temp_dir("runtime_supervisor_unknown_artifact_evidence_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Reject unknown evidence".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_unknown_evidence".to_string(),
                    role: AgentRole::Coder,
                    title: "Produce patch evidence".to_string(),
                    objective: "Produce one patch artifact".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_unknown_evidence",
            RuntimeCommand::StartAgentTask {
                task_id: "task_unknown_evidence".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_unknown_evidence"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_accept_unknown_evidence",
            RuntimeCommand::AcceptAgentArtifact {
                gate_id: "gate-task_unknown_evidence".to_string(),
                evidence_id: "manual-test_result".to_string(),
                decision: Some("unknown evidence should not count".to_string()),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, .. }
                    if command_id == "cmd_accept_unknown_evidence"
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_accept_unknown_evidence"
                    && reason.contains("does not exist")
        )
    }));
    assert!(!events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-task_unknown_evidence"
                    && gate.status == MergeGateStatus::Accepted
        )
    }));
}

#[test]
fn runtime_supervisor_reduces_merge_gate_from_required_evidence_kinds() {
    let cwd = temp_dir("runtime_supervisor_evidence_reducer_cwd");
    let home = temp_dir("runtime_supervisor_evidence_reducer_home");
    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Collect required evidence".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_release_gate".to_string(),
                    role: AgentRole::ReleaseOperator,
                    title: "Verify release gate".to_string(),
                    objective: "Collect all merge evidence".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec![".".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec![
                        "test_result".to_string(),
                        "review".to_string(),
                        "doc_update".to_string(),
                        "release_artifact".to_string(),
                    ],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    for (command_id, evidence_id, kind, summary) in [
        (
            "cmd_record_test_result",
            "evidence-test",
            "test_result",
            "cargo test -p viden-runtime passed",
        ),
        (
            "cmd_record_review",
            "evidence-review",
            "review",
            "review found no blocking issues",
        ),
        (
            "cmd_record_doc_update",
            "evidence-doc",
            "doc_update",
            "frontend contract docs updated",
        ),
        (
            "cmd_record_release_artifact",
            "evidence-release",
            "release_artifact",
            "release checklist prepared",
        ),
    ] {
        supervisor
            .send_command(
                command_id,
                RuntimeCommand::RecordAgentEvidence {
                    gate_id: "gate-task_release_gate".to_string(),
                    evidence_id: Some(evidence_id.to_string()),
                    kind: kind.to_string(),
                    summary: summary.to_string(),
                    path: None,
                    source: Some("release-gate".to_string()),
                    canonical: None,
                },
            )
            .unwrap();
    }

    let collecting = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_release_gate"
                        && gate.status == MergeGateStatus::CollectingEvidence
                        && gate
                            .decision
                            .as_deref()
                            .is_some_and(|decision| decision.contains("missing_canonical"))
                        && gate.evidence_ids
                            == vec![
                                "evidence-test".to_string(),
                                "evidence-review".to_string(),
                                "evidence-doc".to_string(),
                                "evidence-release".to_string(),
                            ]
            )
        })
    });
    assert!(collecting.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.id == "evidence-release"
                    && evidence.kind == "release_artifact"
                    && evidence.source.as_deref() == Some("release-gate")
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    for kind in ["test_result", "review", "doc_update", "release_artifact"] {
        assert!(agent_events.iter().any(|event| {
            event.event_type == "runtime_projection_batch"
                && event.task_id.as_deref() == Some("task_release_gate")
                && event
                    .payload
                    .get("runtime_events_json")
                    .is_some_and(|json| json.contains(kind))
        }));
    }
}

#[test]
fn runtime_supervisor_accepts_rejects_and_merges_agent_artifacts() {
    let cwd = temp_dir("runtime_supervisor_agent_artifact_gate_cwd");
    let home = temp_dir("runtime_supervisor_agent_artifact_gate_home");
    write_test_file(
        &cwd.join("src/lib.rs"),
        "pub const STATUS: &str = \"old\";\n",
    );
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub const STATUS: &str = \"old\";\n+pub const STATUS: &str = \"merged\";\n";
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: patch.to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Gate agent artifact".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_coder".to_string(),
                    role: AgentRole::Coder,
                    title: "Implement runtime artifact gate".to_string(),
                    objective: "Produce a patch artifact for the merge gate".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_coder",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                        if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::Accepted
                        && gate.evidence_ids.iter().any(|id| id == "evidence-task_coder-patch")
            )
        })
    });

    supervisor
        .send_command(
            "cmd_record_test_evidence",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
                canonical: None,
            },
        )
        .unwrap();
    let recorded_test_evidence =
        collect_events_until(&supervisor, Duration::from_secs(2), |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                        if gate.gate_id == "gate-task_coder"
                            && gate.status == MergeGateStatus::Accepted
                            && gate.evidence_ids
                                == vec![
                                    "evidence-task_coder-patch".to_string(),
                                    "manual-test_result".to_string()
                                ]
                )
            })
        });
    assert!(recorded_test_evidence.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_record_test_evidence"
        )
    }));
    assert!(recorded_test_evidence.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.id == "manual-test_result" && evidence.kind == "test_result"
        )
    }));

    supervisor
        .send_command(
            "cmd_accept_test_artifact",
            RuntimeCommand::AcceptAgentArtifact {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: "manual-test_result".to_string(),
                decision: Some("focused tests passed".to_string()),
            },
        )
        .unwrap();
    let accepted_artifact = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, reason }
                    if command_id == "cmd_accept_test_artifact"
                        && reason.contains("missing_canonical")
            )
        })
    });
    assert!(!accepted_artifact.iter().any(|event| {
        matches!(&event.kind, RuntimeEventKind::CommandAccepted { command_id, .. }
            if command_id == "cmd_accept_test_artifact")
    }));

    let store = WorkflowStore::new(home.clone(), &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(!agent_events.iter().any(|event| {
        event.event_type == "agent_artifact_accepted"
            && event.task_id.as_deref() == Some("task_coder")
            && event
                .payload
                .get("evidence_id")
                .is_some_and(|id| id == "manual-test_result")
    }));

    supervisor
        .send_command(
            "cmd_reject_artifact",
            RuntimeCommand::RejectAgentArtifact {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: "manual-test_result".to_string(),
                reason: "test output was from the wrong package".to_string(),
            },
        )
        .unwrap();
    let rejected_artifact = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::NeedsChanges
                        && !gate.evidence_ids.iter().any(|id| id == "manual-test_result")
                        && gate.decision.as_deref()
                            == Some("test output was from the wrong package")
            )
        })
    });
    assert!(rejected_artifact.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_reject_artifact"
        )
    }));

    supervisor
        .send_command(
            "cmd_record_test_evidence_again",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "correct focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
                canonical: None,
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_merge_patch",
            RuntimeCommand::MergeAgentPatch {
                gate_id: "gate-task_coder".to_string(),
                decision: Some("merge accepted patch".to_string()),
            },
        )
        .unwrap();
    let merged = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::Merged
                        && gate.decision.as_deref() == Some("merge accepted patch")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_coder"
                        && task.status == AgentTaskStatus::Applied
                        && task.decision.as_deref() == Some("merge accepted patch")
            )
        })
    });
    assert!(!merged.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. }
                if command_id == "cmd_merge_patch"
        )
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "pub const STATUS: &str = \"merged\";\n"
    );

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_coder")
            && event
                .payload
                .get("command_id")
                .is_some_and(|id| id == "cmd_merge_patch")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| {
                    kinds.contains("merge_gate_updated") && kinds.contains("task_updated")
                })
    }));
}

#[test]
fn runtime_supervisor_applies_accepted_patch_evidence_to_workspace() {
    let cwd = temp_dir("runtime_supervisor_apply_patch_cwd");
    let home = temp_dir("runtime_supervisor_apply_patch_home");
    write_test_file(
        &cwd.join("src/lib.rs"),
        "pub fn name() -> &'static str {\n    \"old\"\n}\n",
    );
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn name() -> &'static str {\n-    \"old\"\n+    \"new\"\n }\n";
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: patch.to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Apply accepted patch".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_coder_apply".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch src lib".to_string(),
                    objective: "Produce a patch artifact".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_coder_apply",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder_apply".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_apply"
                        && gate.status == MergeGateStatus::Accepted
                        && gate.evidence_ids.iter().any(|id| id == "evidence-task_coder_apply-patch")
            )
        })
    });

    supervisor
        .send_command(
            "cmd_accept_tests",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder_apply".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
                canonical: None,
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_apply"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_merge_patch",
            RuntimeCommand::MergeAgentPatch {
                gate_id: "gate-task_coder_apply".to_string(),
                decision: Some("apply accepted patch".to_string()),
            },
        )
        .unwrap();
    let merged = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_apply"
                        && gate.status == MergeGateStatus::Merged
            )
        })
    });
    assert!(merged.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_merge_patch"
        )
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "pub fn name() -> &'static str {\n    \"new\"\n}\n"
    );

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_coder_apply")
            && event
                .payload
                .get("command_id")
                .is_some_and(|id| id == "cmd_merge_patch")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| {
                    kinds.contains("merge_gate_updated") && kinds.contains("task_updated")
                })
    }));
}

#[test]
fn runtime_supervisor_reports_patch_conflict_without_modifying_workspace() {
    let cwd = temp_dir("runtime_supervisor_patch_conflict_cwd");
    let home = temp_dir("runtime_supervisor_patch_conflict_home");
    write_test_file(
        &cwd.join("src/lib.rs"),
        "pub fn name() -> &'static str {\n    \"current\"\n}\n",
    );
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn name() -> &'static str {\n-    \"old\"\n+    \"new\"\n }\n";
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: patch.to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Report patch conflict".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_coder_conflict".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch src lib".to_string(),
                    objective: "Produce a conflicting patch artifact".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_coder_conflict",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder_conflict".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_conflict"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_accept_tests",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder_conflict".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
                canonical: None,
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_conflict"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_merge_patch",
            RuntimeCommand::MergeAgentPatch {
                gate_id: "gate-task_coder_conflict".to_string(),
                decision: Some("apply conflicting patch".to_string()),
            },
        )
        .unwrap();
    let conflicted = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_conflict"
                        && gate.status == MergeGateStatus::NeedsChanges
                        && gate.decision.as_deref().is_some_and(|decision| {
                            decision.contains("patch conflict")
                        })
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_coder_conflict"
                        && task.status == AgentTaskStatus::NeedsInput
                        && task.next_action.as_ref().is_some_and(|action| {
                            action.command.as_deref() == Some("/agent start task_coder_conflict")
                        })
            )
        })
    });
    assert!(conflicted.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_coder_conflict"
                    && task.status == AgentTaskStatus::NeedsInput
                    && task.next_action.as_ref().is_some_and(|action| {
                        action.command.as_deref() == Some("/agent start task_coder_conflict")
                    })
        )
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "pub fn name() -> &'static str {\n    \"current\"\n}\n"
    );

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_coder_conflict")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| {
                    kinds.contains("merge_gate_updated") && kinds.contains("task_updated")
                })
    }));
}

#[test]
fn runtime_supervisor_classifies_agent_task_provider_failures() {
    let cwd = temp_dir("runtime_supervisor_agent_failure_cwd");
    let home = temp_dir("runtime_supervisor_agent_failure_home");
    let provider = Box::new(FailingProvider {
        error: "API error (413): deepseek returned HTTP 413".to_string(),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Classify provider failure".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_tester".to_string(),
                    role: AgentRole::Tester,
                    title: "Run failing provider".to_string(),
                    objective: "Classify provider failure metadata".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["test_result".to_string()],
                    permission_policy: "ask".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start_failure",
            RuntimeCommand::StartAgentTask {
                task_id: "task_tester".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_tester"
                        && task.status == AgentTaskStatus::Failed
                        && task.next_action.as_ref().is_some_and(|action| {
                            action.label == "retry agent task"
                                && action.command.as_deref() == Some("/agent start task_tester")
                                && action.reason.as_deref().is_some_and(|reason| {
                                    reason.contains("request_too_large")
                                })
                        })
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("HTTP 413")
                        && error.hint.as_deref().is_some_and(|hint| {
                            hint.contains("compact provider context")
                        })
            )
        })
    });
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_agent_start_failure"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_tester")
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| kinds.contains("task_updated"))
    }));
}

#[test]
fn runtime_supervisor_cancels_queued_agent_task_with_durable_event() {
    let cwd = temp_dir("runtime_supervisor_cancel_queued_agent_cwd");
    let home = temp_dir("runtime_supervisor_cancel_queued_agent_home");
    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Cancel queued task".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_docs".to_string(),
                    role: AgentRole::DocWriter,
                    title: "Document cancellation".to_string(),
                    objective: "Prove explicit task cancellation is auditable".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["docs".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["doc_update".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let dag_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });
    let dag_id = dag_events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::AgentDagUpdated { dag } => Some(dag.dag_id.clone()),
            _ => None,
        })
        .unwrap();

    supervisor
        .send_command(
            "cmd_cancel_agent",
            RuntimeCommand::CancelAgentTask {
                task_id: "task_docs".to_string(),
            },
        )
        .unwrap();
    let cancelled = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_docs"
                        && task.status == AgentTaskStatus::Cancelled
            )
        })
    });
    assert!(cancelled.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_cancel_agent"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event.task_id.as_deref() == Some("task_docs")
            && event
                .payload
                .get("command_id")
                .is_some_and(|id| id == "cmd_cancel_agent")
            && event.dag_id == dag_id
    }));
}

#[test]
fn runtime_supervisor_blocks_agent_task_until_dependencies_complete() {
    let cwd = temp_dir("runtime_supervisor_agent_deps_cwd");
    let home = temp_dir("runtime_supervisor_agent_deps_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "should not run".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Respect dependencies".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_planner".to_string(),
                        role: AgentRole::Planner,
                        title: "Plan first".to_string(),
                        objective: "Plan the work".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: Vec::new(),
                        context_bundle_id: None,
                        required_evidence: vec!["plan".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_coder".to_string(),
                        role: AgentRole::Coder,
                        title: "Code second".to_string(),
                        objective: "Implement after planning".to_string(),
                        dependencies: vec!["task_planner".to_string()],
                        workspace: None,
                        file_scope: Vec::new(),
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start_blocked",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder".to_string(),
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_coder"
                        && task.status == AgentTaskStatus::Blocked
                        && task.activity.contains("waiting for dependency")
            )
        })
    });

    assert!(!events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content.contains("should not run")
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert_no_project_side_channel_events(&agent_events);
    assert!(agent_events.iter().any(|event| {
        event.event_type == "runtime_projection_batch"
            && event
                .payload
                .get("runtime_event_kinds")
                .is_some_and(|kinds| kinds.contains("task_updated"))
            && event
                .payload
                .get("runtime_events_json")
                .is_some_and(|json| json.contains("task_coder"))
    }));
}

fn wait_until(condition: impl Fn() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "condition did not become true before timeout"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn collect_events_until(
    supervisor: &RuntimeSupervisor,
    timeout: Duration,
    done: impl Fn(&[RuntimeEvent]) -> bool,
) -> Vec<RuntimeEvent> {
    let started = Instant::now();
    let timeout = timeout.max(Duration::from_secs(10));
    let mut events = Vec::new();
    let mut matched = false;
    while started.elapsed() < timeout {
        if let Some(event) = supervisor.recv_event_timeout(Duration::from_millis(50)) {
            events.push(event);
            if done(&events) {
                matched = true;
                break;
            }
        }
    }
    assert!(
        matched,
        "timed out waiting for runtime events matching predicate; collected: {events:#?}"
    );
    events
}

fn collect_events_for(supervisor: &RuntimeSupervisor, timeout: Duration) -> Vec<RuntimeEvent> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < timeout {
        if let Some(event) = supervisor.recv_event_timeout(Duration::from_millis(50)) {
            events.push(event);
        }
    }
    events
}

fn supervisor_engine_with_context(name: &str, content: &str) -> (SessionEngine, String) {
    let cwd = temp_dir(&format!("{name}_cwd"));
    let home = temp_dir(&format!("{name}_home"));
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: content.to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let handle_id = engine
        .runtime_view_state()
        .context_handles
        .first()
        .unwrap()
        .handle_id
        .clone();
    (engine, handle_id)
}

fn context_read_rule(rule_behavior: PermissionBehavior) -> PermissionRule {
    PermissionRule {
        source: PermissionRuleSource::Session,
        rule_behavior,
        rule_value: PermissionRuleValue {
            tool_name: "context_read".to_string(),
            rule_content: None,
        },
    }
}

fn approval_id(events: &[RuntimeEvent]) -> String {
    events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => Some(approval.id.clone()),
            _ => None,
        })
        .expect("approval request id")
}

fn approval_identity(events: &[RuntimeEvent]) -> (String, String) {
    events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => {
                Some((approval.id.clone(), approval.audit_id.clone()))
            }
            _ => None,
        })
        .expect("approval request identity")
}

fn approval_resolved_count(events: &[RuntimeEvent], request_id: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ApprovalResolved { request_id: resolved, .. }
                    if resolved == request_id
            )
        })
        .count()
}

fn owner_for_lane(lane: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-test".to_string(),
        project_id: "project-test".to_string(),
        lane_id: Some(lane.to_string()),
        session_id: Some(format!("session-{lane}")),
        task_id: Some(format!("task-{lane}")),
        turn_id: Some(format!("turn-{lane}")),
    }
}

fn start_agent_task_and_capture_context(
    supervisor: &RuntimeSupervisor,
    task_id: &str,
) -> ContextBundleRecord {
    supervisor
        .send_command(
            format!("cmd_start_{task_id}"),
            RuntimeCommand::StartAgentTask {
                task_id: task_id.to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ContextUpdated { context } if context.task_id == task_id
            )
        })
    });
    events
        .into_iter()
        .find_map(|event| match event.kind {
            RuntimeEventKind::ContextUpdated { context } if context.task_id == task_id => {
                Some(context)
            }
            _ => None,
        })
        .expect("agent task context event")
}

fn assert_context_source(context: &ContextBundleRecord, name: &str, kind: &str) {
    assert!(
        context
            .sources
            .iter()
            .any(|source| source.name == name && source.kind == kind),
        "missing source {name}/{kind} in {:?}",
        context.sources
    );
}

fn context_source_summary(context: &ContextBundleRecord, name: &str, kind: &str) -> String {
    context
        .sources
        .iter()
        .find(|source| source.name == name && source.kind == kind)
        .unwrap_or_else(|| panic!("missing context source {name}/{kind}"))
        .summary
        .clone()
}

fn provider_manifest(request: &ModelRequest) -> String {
    request
        .messages
        .iter()
        .find(|message| message.content.contains("Viden ContextBundle"))
        .expect("provider context manifest")
        .content
        .clone()
}

#[derive(Debug, PartialEq, Eq)]
struct ManifestSourceRefs {
    handle: String,
    item: String,
    view: String,
    raw_hash: String,
    view_hash: String,
}

fn manifest_source_refs(manifest: &str, source_name: &str) -> ManifestSourceRefs {
    let line = manifest
        .lines()
        .find(|line| line.contains(source_name) && line.contains("handle="))
        .unwrap_or_else(|| panic!("missing source refs for {source_name} in {manifest}"));
    ManifestSourceRefs {
        handle: manifest_ref_value(line, "handle="),
        item: manifest_ref_value(line, "item="),
        view: manifest_ref_value(line, "view="),
        raw_hash: manifest_ref_value(line, "raw_hash="),
        view_hash: manifest_ref_value(line, "view_hash="),
    }
}

fn manifest_ref_value(line: &str, key: &str) -> String {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(key))
        .unwrap_or_else(|| panic!("missing {key} in {line}"))
        .to_string()
}

fn write_test_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("test file has parent")).unwrap();
    fs::write(path, contents).unwrap();
}

fn init_git_repo(cwd: &Path) {
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(init.status.success());
    for (key, value) in [
        ("user.email", "viden@example.com"),
        ("user.name", "Viden Test"),
    ] {
        let output = std::process::Command::new("git")
            .args(["config", key, value])
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    let add = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(add.status.success());
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(commit.status.success());
}

fn fake_lsp_registry(workdir: &Path) -> LspServerRegistry {
    let script_path = workdir.join("fake_lsp_server.py");
    fs::write(
        &script_path,
        r#"import json
import sys

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line == b"\r\n":
            break
        key, value = line.decode("utf-8").split(":", 1)
        headers[key.lower()] = value.strip()
    body = sys.stdin.buffer.read(int(headers["content-length"]))
    return json.loads(body.decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}})
    elif method == "initialized":
        continue
    elif method == "textDocument/didOpen":
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": message["params"]["textDocument"]["uri"],
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 16},
                        "end": {"line": 1, "character": 23}
                    },
                    "severity": 1,
                    "source": "fake-lsp",
                    "code": "E100",
                    "message": "fake diagnostic"
                }]
            }
        })
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": message["id"], "result": None})
    elif method == "exit":
        break
"#,
    )
    .unwrap();

    LspServerRegistry::new(vec![LspServerConfig {
        id: "fake-rust".to_string(),
        command: std::env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
        args: vec![script_path.to_string_lossy().to_string()],
        file_extensions: vec!["rs".to_string()],
    }])
}
