use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::sync::OnceLock;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use viden_provider::ModelRequestControl;
use viden_types::{
    ApprovalDecision, ApprovalDefaultAction, ApprovalRequestView, ApprovalResponse, ApprovalRisk,
    ApprovalScope, ApprovalTarget, CapabilityId, EventCursor, FRONTEND_SCHEMA_V1,
    FRONTEND_V1_CAPABILITIES, GapRecovery, PermissionPrompt, ReplayBatch, ReplayRequest,
    RuntimeCommand, RuntimeCommandEnvelope, RuntimeErrorView, RuntimeEvent, RuntimeEventEnvelope,
    RuntimeEventKind, RuntimeOwner, RuntimeSnapshotEnvelope, RuntimeViewState, RuntimeWireEvent,
    TranscriptPage, TranscriptPageRequest, fresh_id, now_timestamp,
};

use crate::{
    SessionEngine,
    event_journal::RuntimeEventJournal,
    lane_runtime::{LaneEffectExecutor, LocalLaneEffectExecutor},
    lane_supervisor::LaneSupervisor,
    runtime_contract::{
        ContextRetrievalJob, SupervisorContextRetrievalPreparation, execute_context_retrieval_job,
        redacted_runtime_command_for_event,
    },
};

struct PendingApproval {
    owner: RuntimeOwner,
    audit_id: String,
    expires_at: u64,
    allowed_scopes: Vec<ApprovalScope>,
    target: PendingApprovalTarget,
}

enum PendingApprovalTarget {
    Channel {
        owner_id: String,
        sender: Sender<ApprovalResponse>,
    },
    ContextRetrieval {
        owner_id: String,
        job: Box<ContextRetrievalJob>,
    },
}

#[derive(Clone)]
enum ActiveJobState {
    Running,
    PendingApproval { request_id: String },
}

#[derive(Clone)]
struct ActiveRuntimeControl {
    owner_id: String,
    owner: RuntimeOwner,
    control: ModelRequestControl,
    state: ActiveJobState,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeOwnerKey {
    workspace_id: String,
    project_id: String,
    lane_id: Option<String>,
    session_id: Option<String>,
    task_id: Option<String>,
    turn_id: Option<String>,
}

impl From<&RuntimeOwner> for RuntimeOwnerKey {
    fn from(owner: &RuntimeOwner) -> Self {
        Self {
            workspace_id: owner.workspace_id.clone(),
            project_id: owner.project_id.clone(),
            lane_id: owner.lane_id.clone(),
            session_id: owner.session_id.clone(),
            task_id: owner.task_id.clone(),
            turn_id: owner.turn_id.clone(),
        }
    }
}

type ActiveControlRegistry = Arc<Mutex<BTreeMap<RuntimeOwnerKey, ActiveRuntimeControl>>>;

struct SupervisorShared<'a> {
    event_bus: &'a RuntimeEventBus,
    active_control: &'a ActiveControlRegistry,
    pending_approvals: &'a Arc<Mutex<BTreeMap<String, PendingApproval>>>,
}

#[derive(Clone)]
struct RuntimeEventBus {
    sender: Sender<RuntimeEventEnvelope>,
    state: Arc<Mutex<RuntimeEventState>>,
}

struct RuntimeEventState {
    journal: RuntimeEventJournal,
    live_view: RuntimeViewState,
}

#[cfg(test)]
type BeforeContextResumeHook = Arc<dyn Fn(&ModelRequestControl) + Send + Sync>;

#[cfg(test)]
static BEFORE_CONTEXT_RESUME_ENQUEUE_HOOK: OnceLock<Mutex<Option<BeforeContextResumeHook>>> =
    OnceLock::new();

#[cfg(test)]
pub(crate) fn set_before_context_resume_enqueue_hook(hook: Option<BeforeContextResumeHook>) {
    if let Ok(mut slot) = BEFORE_CONTEXT_RESUME_ENQUEUE_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *slot = hook;
    }
}

#[cfg(test)]
fn before_context_resume_enqueue_for_test(control: &ModelRequestControl) {
    let hook = BEFORE_CONTEXT_RESUME_ENQUEUE_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(hook) = hook {
        hook(control);
    }
}

#[cfg(not(test))]
fn before_context_resume_enqueue_for_test(_control: &ModelRequestControl) {}

// Internal channel payload mirrors RuntimeCommand construction. Boxing command
// variants would add indirection at every supervisor send site without changing
// the protocol boundary.
#[allow(clippy::large_enum_variant)]
enum SupervisorMessage {
    Command {
        owner: RuntimeOwner,
        command_id: String,
        command: RuntimeCommand,
    },
    ResumeContextRetrieval {
        owner_id: String,
        request_id: String,
        owner: RuntimeOwner,
        audit_id: String,
        job: Box<ContextRetrievalJob>,
    },
    Snapshot {
        response: Sender<Result<RuntimeSnapshotEnvelope, String>>,
    },
    TranscriptPage {
        request: TranscriptPageRequest,
        response: Sender<Result<TranscriptPage, String>>,
    },
    Shutdown {
        response: Option<Sender<()>>,
    },
}

pub struct RuntimeSupervisor {
    commands: Sender<SupervisorMessage>,
    events: Receiver<RuntimeEventEnvelope>,
    event_bus: RuntimeEventBus,
    active_control: ActiveControlRegistry,
    pending_approvals: Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    lane_supervisor: Arc<LaneSupervisor>,
    worker_alive: Arc<AtomicBool>,
    _worker: JoinHandle<()>,
}

impl RuntimeSupervisor {
    pub fn start(engine: SessionEngine) -> Self {
        Self::start_with_approval_ttl(engine, 300)
    }

    fn start_with_approval_ttl(engine: SessionEngine, approval_ttl_secs: u64) -> Self {
        Self::start_with_effects(
            engine,
            approval_ttl_secs,
            Arc::new(LocalLaneEffectExecutor::default()),
        )
    }

    fn start_with_effects(
        mut engine: SessionEngine,
        approval_ttl_secs: u64,
        lane_effects: Arc<dyn LaneEffectExecutor>,
    ) -> Self {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        let active_control: ActiveControlRegistry = Arc::new(Mutex::new(BTreeMap::new()));
        let pending_approvals = Arc::new(Mutex::new(BTreeMap::new()));
        let worker_alive = Arc::new(AtomicBool::new(true));
        let event_bus = RuntimeEventBus {
            sender: event_sender,
            state: Arc::new(Mutex::new(RuntimeEventState {
                journal: RuntimeEventJournal::default_with_stream(fresh_id("runtime-stream")),
                live_view: engine.runtime_view_state(),
            })),
        };

        let lane_repo = engine.cwd().to_path_buf();
        let lane_workflows = engine.workflow_store();
        let lane_event_bus = event_bus.clone();
        let lane_events = Arc::new(move |owner, kind| {
            emit_event(&lane_event_bus, owner, kind);
        });
        let lane_mode_state = Arc::clone(&event_bus.state);
        let lane_mode = Arc::new(move || {
            lane_mode_state
                .lock()
                .map(|state| state.live_view.snapshot.work_mode)
                .unwrap_or(viden_types::WorkMode::Plan)
        });
        let lane_supervisor = Arc::new(LaneSupervisor::new(
            lane_repo,
            lane_workflows,
            lane_effects,
            lane_events,
            lane_mode,
        ));

        install_runtime_event_sink(&mut engine, event_bus.clone(), RuntimeOwner::default());

        let worker_event_bus = event_bus.clone();
        let worker_active_control = Arc::clone(&active_control);
        let worker_pending_approvals = Arc::clone(&pending_approvals);
        let worker_liveness = Arc::clone(&worker_alive);
        let worker = thread::spawn(move || {
            run_supervisor_worker(
                engine,
                command_receiver,
                worker_event_bus,
                worker_active_control,
                worker_pending_approvals,
                approval_ttl_secs,
                worker_liveness,
            );
        });

        Self {
            commands: command_sender,
            events: event_receiver,
            event_bus,
            active_control,
            pending_approvals,
            lane_supervisor,
            worker_alive,
            _worker: worker,
        }
    }

    #[cfg(test)]
    pub(crate) fn start_with_approval_ttl_for_test(
        engine: SessionEngine,
        approval_ttl_secs: u64,
    ) -> Self {
        Self::start_with_approval_ttl(engine, approval_ttl_secs)
    }

    #[cfg(test)]
    pub(crate) fn start_with_lane_effects_for_test(
        engine: SessionEngine,
        lane_effects: Arc<dyn LaneEffectExecutor>,
    ) -> Self {
        Self::start_with_effects(engine, 300, lane_effects)
    }

    pub fn send_command(
        &self,
        command_id: impl Into<String>,
        command: RuntimeCommand,
    ) -> Result<(), String> {
        self.send_command_inner(RuntimeOwner::default(), command_id.into(), command)
    }

    pub fn send_command_from_owner(
        &self,
        owner: RuntimeOwner,
        command_id: impl Into<String>,
        command: RuntimeCommand,
    ) -> Result<(), String> {
        let command_id = command_id.into();
        self.send_command_inner(owner, command_id, command)
    }

    fn send_command_inner(
        &self,
        owner: RuntimeOwner,
        command_id: String,
        command: RuntimeCommand,
    ) -> Result<(), String> {
        if LaneSupervisor::handles(&command) {
            return self.lane_supervisor.send(owner, command_id, command);
        }
        match command {
            RuntimeCommand::CancelActiveTurn => {
                if self.lane_supervisor.cancel(&owner, command_id.clone())? {
                    return Ok(());
                }
                let Some(control) = self
                    .active_control
                    .lock()
                    .map_err(|_| "active turn lock poisoned".to_string())?
                    .get(&RuntimeOwnerKey::from(&owner))
                    .cloned()
                else {
                    emit_event(
                        &self.event_bus,
                        owner.clone(),
                        RuntimeEventKind::CommandRejected {
                            command_id,
                            reason: "no active turn to cancel".to_string(),
                        },
                    );
                    return Ok(());
                };
                if control.owner != owner {
                    emit_event(
                        &self.event_bus,
                        owner.clone(),
                        RuntimeEventKind::CommandRejected {
                            command_id,
                            reason: format!(
                                "active runtime job `{}` owner mismatch",
                                control.owner_id
                            ),
                        },
                    );
                    return Ok(());
                }
                control.control.cancel();
                if let ActiveJobState::PendingApproval { request_id } = &control.state {
                    resolve_pending_approval_by_id(
                        request_id,
                        ApprovalDecision::Deny,
                        &self.event_bus,
                        &self.active_control,
                        &self.pending_approvals,
                        None,
                    );
                }
                emit_event(
                    &self.event_bus,
                    owner.clone(),
                    RuntimeEventKind::CommandAccepted {
                        command_id,
                        command: redacted_runtime_command_for_event(
                            &RuntimeCommand::CancelActiveTurn,
                        ),
                    },
                );
                Ok(())
            }
            RuntimeCommand::RespondToApproval {
                request_id,
                response,
            } => {
                if self.lane_supervisor.respond_to_approval(
                    &owner,
                    &command_id,
                    &request_id,
                    response.clone(),
                )? {
                    emit_event(
                        &self.event_bus,
                        owner.clone(),
                        RuntimeEventKind::CommandAccepted {
                            command_id,
                            command: redacted_runtime_command_for_event(
                                &RuntimeCommand::RespondToApproval {
                                    request_id,
                                    response,
                                },
                            ),
                        },
                    );
                    return Ok(());
                }
                let pending = {
                    let mut approvals = self
                        .pending_approvals
                        .lock()
                        .map_err(|_| "approval lock poisoned".to_string())?;
                    let Some(pending) = approvals.get(&request_id) else {
                        emit_event(
                            &self.event_bus,
                            owner.clone(),
                            RuntimeEventKind::CommandRejected {
                                command_id,
                                reason: format!("approval request `{request_id}` is not pending"),
                            },
                        );
                        return Ok(());
                    };
                    if pending.owner != owner {
                        emit_event(
                            &self.event_bus,
                            owner.clone(),
                            RuntimeEventKind::CommandRejected {
                                command_id,
                                reason: format!("approval request `{request_id}` owner mismatch"),
                            },
                        );
                        return Ok(());
                    }
                    if !approval_decision_is_allowed_by_request(&response.decision, pending) {
                        emit_event(
                            &self.event_bus,
                            owner.clone(),
                            RuntimeEventKind::CommandRejected {
                                command_id,
                                reason: format!(
                                    "approval request `{request_id}` scope is not allowed"
                                ),
                            },
                        );
                        return Ok(());
                    }
                    approvals.remove(&request_id).expect("pending approval")
                };
                emit_event(
                    &self.event_bus,
                    owner.clone(),
                    RuntimeEventKind::CommandAccepted {
                        command_id,
                        command: redacted_runtime_command_for_event(
                            &RuntimeCommand::RespondToApproval {
                                request_id: request_id.clone(),
                                response: response.clone(),
                            },
                        ),
                    },
                );
                let pending_owner = pending.owner.clone();
                let pending_audit_id = pending.audit_id.clone();
                match pending.target {
                    PendingApprovalTarget::Channel { owner_id, sender } => {
                        if response.is_allowed() {
                            let _ = mark_active_running(&self.active_control, &owner_id);
                        }
                        emit_event(
                            &self.event_bus,
                            pending_owner.clone(),
                            approval_resolved_event(
                                &request_id,
                                response.decision.clone(),
                                pending_owner,
                                pending_audit_id,
                            ),
                        );
                        sender
                            .send(response.clone())
                            .map_err(|err| format!("failed to send approval response: {err}"))?;
                    }
                    PendingApprovalTarget::ContextRetrieval { owner_id, job } => {
                        if response.is_allowed() {
                            let mut job = *job;
                            job.permission_decision = "approved".to_string();
                            if let Err(err) = mark_active_running(&self.active_control, &owner_id) {
                                emit_error(&self.event_bus, pending_owner.clone(), err);
                                return Ok(());
                            }
                            let Some(control) =
                                active_control_for_owner(&self.active_control, &owner_id)
                            else {
                                emit_error(
                                    &self.event_bus,
                                    pending_owner.clone(),
                                    format!("active runtime job `{owner_id}` is no longer running"),
                                );
                                return Ok(());
                            };
                            emit_event(
                                &self.event_bus,
                                pending_owner.clone(),
                                approval_resolved_event(
                                    &request_id,
                                    response.decision.clone(),
                                    pending_owner.clone(),
                                    pending_audit_id.clone(),
                                ),
                            );
                            before_context_resume_enqueue_for_test(&control);
                            if let Err(err) =
                                self.commands
                                    .send(SupervisorMessage::ResumeContextRetrieval {
                                        owner_id: owner_id.clone(),
                                        request_id,
                                        owner: pending_owner.clone(),
                                        audit_id: pending_audit_id.clone(),
                                        job: Box::new(job),
                                    })
                            {
                                clear_active_control(&self.active_control, &owner_id);
                                emit_error(
                                    &self.event_bus,
                                    pending_owner.clone(),
                                    format!("runtime supervisor stopped: {err}"),
                                );
                            }
                        } else {
                            emit_event(
                                &self.event_bus,
                                pending_owner.clone(),
                                approval_resolved_event(
                                    &request_id,
                                    response.decision.clone(),
                                    pending_owner.clone(),
                                    pending_audit_id,
                                ),
                            );
                            emit_error(
                                &self.event_bus,
                                pending_owner.clone(),
                                "User denied the permission request".to_string(),
                            );
                            clear_active_control(&self.active_control, &owner_id);
                        }
                    }
                }
                Ok(())
            }
            RuntimeCommand::SubmitUserInput { .. }
            | RuntimeCommand::StartAgentTask { .. }
            | RuntimeCommand::RetrieveContext { .. } => {
                if let Some(owner_id) = active_owner_id(&self.active_control, &owner) {
                    emit_event(
                        &self.event_bus,
                        owner.clone(),
                        RuntimeEventKind::CommandRejected {
                            command_id,
                            reason: format!("active runtime job `{owner_id}` is already running"),
                        },
                    );
                    return Ok(());
                }
                self.commands
                    .send(SupervisorMessage::Command {
                        owner,
                        command_id,
                        command,
                    })
                    .map_err(|err| format!("runtime supervisor stopped: {err}"))
            }
            command => self
                .commands
                .send(SupervisorMessage::Command {
                    owner,
                    command_id,
                    command,
                })
                .map_err(|err| format!("runtime supervisor stopped: {err}")),
        }
    }

    pub fn recv_event_timeout(&self, timeout: Duration) -> Option<RuntimeEvent> {
        self.recv_event_envelope_timeout(timeout)
            .and_then(|envelope| match envelope.event {
                RuntimeWireEvent::Known(event) => Some(event),
                RuntimeWireEvent::Unknown { .. } => None,
            })
    }

    pub fn send_command_envelope(&self, envelope: RuntimeCommandEnvelope) -> Result<(), String> {
        if envelope.schema_version != FRONTEND_SCHEMA_V1 {
            return Err(format!(
                "unsupported frontend schema {}",
                envelope.schema_version.0
            ));
        }
        self.send_command_from_owner(envelope.owner, envelope.command_id, envelope.command)
    }

    pub fn recv_event_envelope(
        &self,
        timeout: Duration,
    ) -> Result<Option<RuntimeEventEnvelope>, String> {
        match self.events.try_recv() {
            Ok(envelope) => return Ok(Some(envelope)),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("runtime supervisor event stream stopped".to_string());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if !self.worker_alive.load(Ordering::Acquire) {
            return Err("runtime supervisor event stream stopped".to_string());
        }

        match self.events.recv_timeout(timeout) {
            Ok(envelope) => Ok(Some(envelope)),
            Err(mpsc::RecvTimeoutError::Timeout) if self.worker_alive.load(Ordering::Acquire) => {
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                Err("runtime supervisor event stream stopped".to_string())
            }
        }
    }

    pub fn recv_event_envelope_timeout(&self, timeout: Duration) -> Option<RuntimeEventEnvelope> {
        self.recv_event_envelope(timeout).ok().flatten()
    }

    pub fn snapshot_envelope(&self) -> Result<RuntimeSnapshotEnvelope, String> {
        let (response, receiver) = mpsc::channel();
        self.commands
            .send(SupervisorMessage::Snapshot { response })
            .map_err(|err| format!("runtime supervisor stopped: {err}"))?;
        receiver
            .recv()
            .map_err(|err| format!("runtime supervisor snapshot failed: {err}"))?
    }

    pub fn replay_events(&self, request: ReplayRequest) -> Result<ReplayBatch, GapRecovery> {
        self.event_bus
            .state
            .lock()
            .map_err(|_| GapRecovery::SnapshotRequired {
                reason_code: "journal_unavailable".to_string(),
            })?
            .journal
            .replay(request)
    }

    pub fn load_transcript_page(
        &self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, String> {
        let (response, receiver) = mpsc::channel();
        self.commands
            .send(SupervisorMessage::TranscriptPage { request, response })
            .map_err(|err| format!("runtime supervisor stopped: {err}"))?;
        receiver
            .recv()
            .map_err(|err| format!("runtime supervisor transcript page failed: {err}"))?
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn expire_pending_approvals_for_test(&self, now: u64) {
        expire_pending_approvals_at(
            now,
            &self.event_bus,
            &self.active_control,
            &self.pending_approvals,
        );
    }

    #[cfg(test)]
    pub(crate) fn stop_worker_for_test(&self) {
        let (response, receiver) = mpsc::channel();
        self.commands
            .send(SupervisorMessage::Shutdown {
                response: Some(response),
            })
            .expect("runtime supervisor test shutdown");
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("runtime supervisor worker stopped");
    }
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        let _ = self
            .commands
            .send(SupervisorMessage::Shutdown { response: None });
    }
}

fn run_supervisor_worker(
    mut engine: SessionEngine,
    command_receiver: Receiver<SupervisorMessage>,
    event_bus: RuntimeEventBus,
    active_control: ActiveControlRegistry,
    pending_approvals: Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    approval_ttl_secs: u64,
    worker_alive: Arc<AtomicBool>,
) {
    let _liveness = WorkerLivenessGuard(Arc::clone(&worker_alive));
    while let Ok(message) = command_receiver.recv() {
        match message {
            SupervisorMessage::Shutdown { response } => {
                worker_alive.store(false, Ordering::Release);
                if let Some(response) = response {
                    let _ = response.send(());
                }
                break;
            }
            SupervisorMessage::Command {
                owner,
                command_id,
                command,
            } => {
                // Background engine work clones the installed sink, so bind
                // owner identity at command dispatch rather than consulting a
                // later active-owner value when the event finally arrives.
                install_runtime_event_sink(&mut engine, event_bus.clone(), owner.clone());
                match command {
                    RuntimeCommand::SubmitUserInput { content } => {
                        run_supervised_input(
                            &mut engine,
                            owner,
                            command_id,
                            content,
                            &event_bus,
                            &active_control,
                            &pending_approvals,
                            approval_ttl_secs,
                        );
                    }
                    RuntimeCommand::StartAgentTask { task_id } => {
                        run_supervised_agent_task(
                            &mut engine,
                            owner,
                            command_id,
                            task_id,
                            &event_bus,
                            &active_control,
                            &pending_approvals,
                            approval_ttl_secs,
                        );
                    }
                    RuntimeCommand::RetrieveContext { handle_id, reason } => {
                        run_supervised_context_retrieval(
                            &mut engine,
                            owner,
                            command_id,
                            handle_id,
                            reason,
                            SupervisorShared {
                                event_bus: &event_bus,
                                active_control: &active_control,
                                pending_approvals: &pending_approvals,
                            },
                            approval_ttl_secs,
                        );
                    }
                    command => {
                        let mut approver = |_prompt: PermissionPrompt| {
                            ApprovalResponse::deny(Some(
                                "runtime supervisor command path does not own this approval"
                                    .to_string(),
                            ))
                        };
                        match engine.handle_runtime_command(command_id, command, &mut approver) {
                            Ok(events) => emit_events(&event_bus, owner, events),
                            Err(err) => emit_error(&event_bus, owner, err),
                        }
                    }
                }
            }
            SupervisorMessage::ResumeContextRetrieval {
                owner_id,
                request_id,
                owner,
                audit_id,
                job,
            } => {
                resume_context_retrieval_after_approval(
                    &mut engine,
                    owner_id,
                    request_id,
                    owner,
                    audit_id,
                    *job,
                    &event_bus,
                    &active_control,
                );
            }
            SupervisorMessage::Snapshot { response } => {
                let envelope = match event_bus.state.lock() {
                    Ok(state) => {
                        // Cursor and live projection share one lock so a
                        // reconnect cannot pair a newer transient event with
                        // an older snapshot boundary.
                        Ok(RuntimeSnapshotEnvelope {
                            schema_version: FRONTEND_SCHEMA_V1,
                            capabilities: runtime_frontend_capabilities(),
                            cursor: state.journal.current_cursor(),
                            snapshot: state.live_view.snapshot.clone(),
                            view: state.live_view.clone(),
                        })
                    }
                    Err(_) => Err("runtime event journal lock poisoned".to_string()),
                };
                let _ = response.send(envelope);
            }
            SupervisorMessage::TranscriptPage { request, response } => {
                let _ = response.send(engine.load_transcript_page(&request));
            }
        }
    }
}

struct WorkerLivenessGuard(Arc<AtomicBool>);

impl Drop for WorkerLivenessGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn install_runtime_event_sink(
    engine: &mut SessionEngine,
    event_bus: RuntimeEventBus,
    owner: RuntimeOwner,
) {
    engine.set_runtime_event_sink(Some(Arc::new(move |events| {
        emit_events(&event_bus, owner.clone(), events);
    })));
}

fn runtime_frontend_capabilities() -> BTreeSet<CapabilityId> {
    FRONTEND_V1_CAPABILITIES
        .iter()
        .map(|capability| CapabilityId(capability.to_string()))
        .collect()
}

fn run_supervised_context_retrieval(
    engine: &mut SessionEngine,
    owner: RuntimeOwner,
    command_id: String,
    handle_id: String,
    reason: String,
    shared: SupervisorShared<'_>,
    approval_ttl_secs: u64,
) {
    if let Some(owner_id) = active_owner_id(shared.active_control, &owner) {
        emit_event(
            shared.event_bus,
            owner.clone(),
            RuntimeEventKind::CommandRejected {
                command_id,
                reason: format!("active runtime job `{owner_id}` is already running"),
            },
        );
        return;
    }
    let prepared = match engine.prepare_context_retrieval_for_supervisor(&handle_id, &reason) {
        Ok(prepared) => prepared,
        Err(err) => {
            emit_event(
                shared.event_bus,
                owner.clone(),
                RuntimeEventKind::CommandRejected {
                    command_id,
                    reason: err,
                },
            );
            return;
        }
    };
    match prepared {
        SupervisorContextRetrievalPreparation::Ready(prepared) => {
            let control = ModelRequestControl::new();
            if let Err(err) = acquire_active_job(
                shared.active_control,
                command_id.clone(),
                owner.clone(),
                control.clone(),
                ActiveJobState::Running,
            ) {
                emit_event(
                    shared.event_bus,
                    owner.clone(),
                    RuntimeEventKind::CommandRejected {
                        command_id,
                        reason: err,
                    },
                );
                return;
            }
            emit_event(
                shared.event_bus,
                owner.clone(),
                RuntimeEventKind::CommandAccepted {
                    command_id: command_id.clone(),
                    command: redacted_runtime_command_for_event(&RuntimeCommand::RetrieveContext {
                        handle_id: handle_id.clone(),
                        reason: reason.clone(),
                    }),
                },
            );
            emit_events(shared.event_bus, owner.clone(), prepared.pre_events);
            start_context_retrieval_worker(
                command_id,
                prepared.job,
                control,
                owner.clone(),
                shared.event_bus,
                shared.active_control,
            );
        }
        SupervisorContextRetrievalPreparation::PendingApproval { mut approval, job } => {
            approval.owner = owner.clone();
            approval.audit_id = fresh_id("audit");
            approval.expires_at = now_timestamp().saturating_add(approval_ttl_secs);
            approval.allowed_scopes = allowed_approval_scopes(&approval.owner, &[]);
            let control = ModelRequestControl::new();
            if let Err(err) = acquire_active_job(
                shared.active_control,
                command_id.clone(),
                owner.clone(),
                control,
                ActiveJobState::PendingApproval {
                    request_id: approval.id.clone(),
                },
            ) {
                emit_event(
                    shared.event_bus,
                    owner.clone(),
                    RuntimeEventKind::CommandRejected {
                        command_id,
                        reason: err,
                    },
                );
                return;
            }
            emit_event(
                shared.event_bus,
                owner.clone(),
                RuntimeEventKind::CommandAccepted {
                    command_id: command_id.clone(),
                    command: redacted_runtime_command_for_event(&RuntimeCommand::RetrieveContext {
                        handle_id: handle_id.clone(),
                        reason: reason.clone(),
                    }),
                },
            );
            insert_pending_approval(
                shared.pending_approvals,
                shared.event_bus,
                shared.active_control,
                approval.id.clone(),
                PendingApproval {
                    owner: approval.owner.clone(),
                    audit_id: approval.audit_id.clone(),
                    expires_at: approval.expires_at,
                    allowed_scopes: approval.allowed_scopes.clone(),
                    target: PendingApprovalTarget::ContextRetrieval {
                        owner_id: command_id,
                        job: Box::new(job),
                    },
                },
            );
            emit_event(
                shared.event_bus,
                owner,
                RuntimeEventKind::ApprovalRequested { approval },
            );
        }
    }
}

fn start_context_retrieval_worker(
    owner_id: String,
    job: ContextRetrievalJob,
    control: ModelRequestControl,
    owner: RuntimeOwner,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
) {
    let worker_event_bus = event_bus.clone();
    let worker_active_control = Arc::clone(active_control);
    thread::spawn(move || {
        let result = execute_context_retrieval_job(job, &control);
        clear_active_control(&worker_active_control, &owner_id);
        match result {
            Ok(events) => {
                if control.check_cancelled().is_ok() {
                    emit_events(&worker_event_bus, owner.clone(), events);
                } else {
                    emit_error(
                        &worker_event_bus,
                        owner.clone(),
                        "Model request cancelled".to_string(),
                    );
                }
            }
            Err(err) => emit_error(&worker_event_bus, owner, err),
        }
    });
}

fn acquire_active_job(
    active_control: &ActiveControlRegistry,
    owner_id: String,
    owner: RuntimeOwner,
    control: ModelRequestControl,
    state: ActiveJobState,
) -> Result<(), String> {
    let mut controls = active_control
        .lock()
        .map_err(|_| "active turn lock poisoned".to_string())?;
    let key = RuntimeOwnerKey::from(&owner);
    if let Some(active) = controls.get(&key) {
        return Err(format!(
            "active runtime job `{}` is already running",
            active.owner_id
        ));
    }
    controls.insert(
        key,
        ActiveRuntimeControl {
            owner_id,
            owner,
            control,
            state,
        },
    );
    Ok(())
}

fn active_control_for_owner(
    active_control: &ActiveControlRegistry,
    owner_id: &str,
) -> Option<ModelRequestControl> {
    active_control.lock().ok().and_then(|controls| {
        controls
            .values()
            .find(|active| active.owner_id == owner_id)
            .map(|active| active.control.clone())
    })
}

fn mark_active_running(
    active_control: &ActiveControlRegistry,
    owner_id: &str,
) -> Result<(), String> {
    let mut controls = active_control
        .lock()
        .map_err(|_| "active turn lock poisoned".to_string())?;
    let Some(active) = controls
        .values_mut()
        .find(|active| active.owner_id == owner_id)
    else {
        return Err(format!(
            "active runtime job `{owner_id}` is no longer pending"
        ));
    };
    active.state = ActiveJobState::Running;
    Ok(())
}

fn mark_active_pending(
    active_control: &ActiveControlRegistry,
    owner_id: &str,
    request_id: String,
) -> Result<(), String> {
    let mut controls = active_control
        .lock()
        .map_err(|_| "active turn lock poisoned".to_string())?;
    let Some(active) = controls
        .values_mut()
        .find(|active| active.owner_id == owner_id)
    else {
        return Err(format!(
            "active runtime job `{owner_id}` is no longer running"
        ));
    };
    active.state = ActiveJobState::PendingApproval { request_id };
    Ok(())
}

fn insert_pending_approval(
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    request_id: String,
    pending: PendingApproval,
) {
    let expires_at = pending.expires_at;
    if let Ok(mut approvals) = pending_approvals.lock() {
        approvals.insert(request_id.clone(), pending);
    }
    schedule_approval_expiry(
        request_id,
        expires_at,
        event_bus,
        active_control,
        pending_approvals,
    );
}

fn schedule_approval_expiry(
    request_id: String,
    expires_at: u64,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    let event_bus = event_bus.clone();
    let active_control = Arc::clone(active_control);
    let pending_approvals = Arc::clone(pending_approvals);
    thread::spawn(move || {
        let now = now_timestamp();
        if expires_at > now {
            thread::sleep(Duration::from_secs(expires_at - now));
        }
        resolve_pending_approval_by_id(
            &request_id,
            ApprovalDecision::Deny,
            &event_bus,
            &active_control,
            &pending_approvals,
            Some("Approval expired; default action deny".to_string()),
        );
    });
}

fn expire_pending_approvals_at(
    now: u64,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    let expired_ids = pending_approvals
        .lock()
        .map(|approvals| {
            approvals
                .iter()
                .filter(|(_, approval)| approval.expires_at <= now)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for request_id in expired_ids {
        resolve_pending_approval_by_id(
            &request_id,
            ApprovalDecision::Deny,
            event_bus,
            active_control,
            pending_approvals,
            Some("Approval expired; default action deny".to_string()),
        );
    }
}

fn resolve_pending_approval_by_id(
    request_id: &str,
    decision: ApprovalDecision,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    error_message: Option<String>,
) {
    let pending = pending_approvals
        .lock()
        .ok()
        .and_then(|mut approvals| approvals.remove(request_id));
    if let Some(pending) = pending {
        resolve_removed_pending_approval(
            request_id,
            pending,
            decision,
            event_bus,
            active_control,
            error_message,
        );
    }
}

fn resolve_removed_pending_approval(
    request_id: &str,
    pending: PendingApproval,
    decision: ApprovalDecision,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    error_message: Option<String>,
) {
    let owner_id = match &pending.target {
        PendingApprovalTarget::Channel { owner_id, .. } => owner_id.clone(),
        PendingApprovalTarget::ContextRetrieval { owner_id, .. } => owner_id.clone(),
    };
    let owner = pending.owner.clone();
    emit_event(
        event_bus,
        owner.clone(),
        approval_resolved_event(
            request_id,
            decision.clone(),
            pending.owner,
            pending.audit_id,
        ),
    );
    if !matches!(decision, ApprovalDecision::Allow { .. }) {
        clear_active_control(active_control, &owner_id);
    }
    match pending.target {
        PendingApprovalTarget::Channel { sender, .. } => {
            let _ = sender.send(ApprovalResponse {
                decision,
                feedback: error_message,
            });
        }
        PendingApprovalTarget::ContextRetrieval { .. } => {
            if let Some(message) = error_message {
                emit_error(event_bus, owner, message);
            }
        }
    }
}

fn approval_decision_is_allowed_by_request(
    decision: &ApprovalDecision,
    pending: &PendingApproval,
) -> bool {
    match decision {
        ApprovalDecision::Deny => true,
        ApprovalDecision::Allow { scope } => pending
            .allowed_scopes
            .iter()
            .any(|allowed| allowed == scope),
    }
}

#[allow(clippy::too_many_arguments)]
fn resume_context_retrieval_after_approval(
    engine: &mut SessionEngine,
    owner_id: String,
    _request_id: String,
    owner: RuntimeOwner,
    _audit_id: String,
    job: ContextRetrievalJob,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
) {
    let Some(control) = active_control_for_owner(active_control, &owner_id) else {
        emit_error(
            event_bus,
            owner,
            format!("active runtime job `{owner_id}` is no longer pending"),
        );
        return;
    };
    if let Err(err) = engine.validate_context_retrieval_job_for_supervisor(&job) {
        clear_active_control(active_control, &owner_id);
        emit_error(event_bus, owner, err);
        return;
    }
    if let Err(err) = mark_active_running(active_control, &owner_id) {
        emit_error(event_bus, owner, err);
        return;
    }
    start_context_retrieval_worker(owner_id, job, control, owner, event_bus, active_control);
}

fn active_owner_id(active_control: &ActiveControlRegistry, owner: &RuntimeOwner) -> Option<String> {
    active_control.lock().ok().and_then(|controls| {
        controls
            .get(&RuntimeOwnerKey::from(owner))
            .map(|active| active.owner_id.clone())
    })
}

fn clear_active_control(active_control: &ActiveControlRegistry, owner_id: &str) {
    if let Ok(mut controls) = active_control.lock() {
        controls.retain(|_, active| active.owner_id != owner_id);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_supervised_agent_task(
    engine: &mut SessionEngine,
    owner: RuntimeOwner,
    command_id: String,
    task_id: String,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    approval_ttl_secs: u64,
) {
    let control = ModelRequestControl::new();
    if let Err(err) = acquire_active_job(
        active_control,
        command_id.clone(),
        owner.clone(),
        control.clone(),
        ActiveJobState::Running,
    ) {
        emit_event(
            event_bus,
            owner.clone(),
            RuntimeEventKind::CommandRejected {
                command_id,
                reason: err,
            },
        );
        return;
    }
    emit_event(
        event_bus,
        owner.clone(),
        RuntimeEventKind::CommandAccepted {
            command_id: command_id.clone(),
            command: redacted_runtime_command_for_event(&RuntimeCommand::StartAgentTask {
                task_id: task_id.clone(),
            }),
        },
    );

    let mut approver = |prompt: PermissionPrompt| {
        let request_id = fresh_id("approval");
        let (approval_sender, approval_receiver) = mpsc::channel();
        let approval =
            approval_request_view(&request_id, &prompt, owner.clone(), approval_ttl_secs);
        let _ = mark_active_pending(active_control, &command_id, request_id.clone());
        insert_pending_approval(
            pending_approvals,
            event_bus,
            active_control,
            request_id.clone(),
            PendingApproval {
                owner: approval.owner.clone(),
                audit_id: approval.audit_id.clone(),
                expires_at: approval.expires_at,
                allowed_scopes: approval.allowed_scopes.clone(),
                target: PendingApprovalTarget::Channel {
                    owner_id: command_id.clone(),
                    sender: approval_sender,
                },
            },
        );
        emit_event(
            event_bus,
            owner.clone(),
            RuntimeEventKind::ApprovalRequested {
                approval: approval.clone(),
            },
        );
        approval_receiver
            .recv()
            .unwrap_or(ApprovalResponse::deny(Some(
                "approval response channel closed".to_string(),
            )))
    };

    let result = engine.run_agent_task_with_control(&task_id, &mut approver, &control);
    clear_active_control(active_control, &command_id);
    match result {
        Ok(events) => emit_events(event_bus, owner.clone(), events),
        Err(err) => emit_error(event_bus, owner, err),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_supervised_input(
    engine: &mut SessionEngine,
    owner: RuntimeOwner,
    command_id: String,
    content: String,
    event_bus: &RuntimeEventBus,
    active_control: &ActiveControlRegistry,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    approval_ttl_secs: u64,
) {
    let control = ModelRequestControl::new();
    if let Err(err) = acquire_active_job(
        active_control,
        command_id.clone(),
        owner.clone(),
        control.clone(),
        ActiveJobState::Running,
    ) {
        emit_event(
            event_bus,
            owner.clone(),
            RuntimeEventKind::CommandRejected {
                command_id,
                reason: err,
            },
        );
        return;
    }
    emit_event(
        event_bus,
        owner.clone(),
        RuntimeEventKind::CommandAccepted {
            command_id: command_id.clone(),
            command: redacted_runtime_command_for_event(&RuntimeCommand::SubmitUserInput {
                content: content.clone(),
            }),
        },
    );

    let mut approver = |prompt: PermissionPrompt| {
        let request_id = fresh_id("approval");
        let (approval_sender, approval_receiver) = mpsc::channel();
        let approval =
            approval_request_view(&request_id, &prompt, owner.clone(), approval_ttl_secs);
        let _ = mark_active_pending(active_control, &command_id, request_id.clone());
        insert_pending_approval(
            pending_approvals,
            event_bus,
            active_control,
            request_id.clone(),
            PendingApproval {
                owner: approval.owner.clone(),
                audit_id: approval.audit_id.clone(),
                expires_at: approval.expires_at,
                allowed_scopes: approval.allowed_scopes.clone(),
                target: PendingApprovalTarget::Channel {
                    owner_id: command_id.clone(),
                    sender: approval_sender,
                },
            },
        );
        emit_event(
            event_bus,
            owner.clone(),
            RuntimeEventKind::ApprovalRequested {
                approval: approval.clone(),
            },
        );
        approval_receiver
            .recv()
            .unwrap_or(ApprovalResponse::deny(Some(
                "approval response channel closed".to_string(),
            )))
    };

    let result = engine.process_input_with_approval_and_control(&content, &mut approver, &control);
    clear_active_control(active_control, &command_id);
    match result {
        Ok(events) => emit_events(
            event_bus,
            owner.clone(),
            engine.runtime_events_for_engine_events(&events),
        ),
        Err(err) => emit_error(event_bus, owner, err),
    }
}

fn approval_request_view(
    request_id: &str,
    prompt: &PermissionPrompt,
    owner: RuntimeOwner,
    approval_ttl_secs: u64,
) -> ApprovalRequestView {
    let allowed_scopes = allowed_approval_scopes(&owner, &prompt.candidate_paths);
    ApprovalRequestView {
        id: request_id.to_string(),
        tool_name: prompt.tool_name.clone(),
        title: format!("Approve {}", prompt.tool_name),
        message: prompt.message.clone(),
        input_preview: prompt.input_preview.clone(),
        is_mutating: true,
        reason: Some(prompt.message.clone()),
        owner,
        risk: ApprovalRisk::Medium,
        target: ApprovalTarget {
            kind: prompt.tool_name.clone(),
            display: prompt.input_preview.clone(),
            canonical_ref: prompt.candidate_paths.first().cloned(),
        },
        allowed_scopes,
        policy_reason_key: "permission.requires_approval".to_string(),
        policy_reason_args: BTreeMap::new(),
        expires_at: now_timestamp().saturating_add(approval_ttl_secs),
        default_action: ApprovalDefaultAction::Deny,
        audit_id: fresh_id("audit"),
    }
}

fn allowed_approval_scopes(owner: &RuntimeOwner, candidate_paths: &[String]) -> Vec<ApprovalScope> {
    let mut scopes = vec![ApprovalScope::Once];
    if let Some(session_id) = owner.session_id.clone()
        && !session_id.is_empty()
    {
        scopes.push(ApprovalScope::Session { session_id });
    }
    if !candidate_paths.is_empty() {
        scopes.push(ApprovalScope::RepoAllowlist {
            paths: candidate_paths.to_vec(),
        });
    }
    scopes
}

fn approval_resolved_event(
    request_id: &str,
    decision: ApprovalDecision,
    owner: RuntimeOwner,
    audit_id: String,
) -> RuntimeEventKind {
    RuntimeEventKind::ApprovalResolved {
        request_id: request_id.to_string(),
        decision,
        owner,
        audit_id,
    }
}

fn emit_events(bus: &RuntimeEventBus, owner: RuntimeOwner, events: Vec<RuntimeEvent>) {
    for event in events {
        emit_known_event(bus, owner.clone(), event);
    }
}

fn emit_error(bus: &RuntimeEventBus, owner: RuntimeOwner, message: String) {
    emit_event(
        bus,
        owner,
        RuntimeEventKind::Error {
            error: RuntimeErrorView {
                message,
                recoverable: true,
                hint: None,
            },
        },
    );
}

fn emit_event(bus: &RuntimeEventBus, owner: RuntimeOwner, kind: RuntimeEventKind) {
    let event = RuntimeEvent::new(0, kind);
    emit_known_event(bus, owner, event);
}

fn emit_known_event(bus: &RuntimeEventBus, owner: RuntimeOwner, event: RuntimeEvent) {
    let Ok(mut state) = bus.state.lock() else {
        return;
    };
    let envelope = state.journal.record(RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner,
        cursor: EventCursor {
            stream_id: String::new(),
            sequence: event.sequence,
        },
        event: RuntimeWireEvent::Known(event),
    });
    if let RuntimeWireEvent::Known(event) = &envelope.event {
        state.live_view.apply_event(event);
    }
    // The send remains inside the journal critical section so concurrent
    // producers cannot make a later envelope visible first.
    let _ = bus.sender.send(envelope);
}
