use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use viden_provider::ModelRequestControl;
use viden_types::{
    ApprovalDecision, ApprovalDefaultAction, ApprovalRequestView, ApprovalResponse, ApprovalRisk,
    ApprovalScope, ApprovalTarget, PermissionPrompt, RuntimeCommand, RuntimeErrorView,
    RuntimeEvent, RuntimeEventKind, RuntimeOwner, fresh_id, now_timestamp,
};

use crate::{
    SessionEngine,
    runtime_contract::{
        ContextRetrievalJob, SupervisorContextRetrievalPreparation, execute_context_retrieval_job,
        redacted_runtime_command_for_event,
    },
};

struct PendingApproval {
    owner: RuntimeOwner,
    audit_id: String,
    #[cfg_attr(not(test), allow(dead_code))]
    expires_at: u64,
    target: PendingApprovalTarget,
}

enum PendingApprovalTarget {
    Channel(Sender<ApprovalResponse>),
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
    control: ModelRequestControl,
    state: ActiveJobState,
}

struct SupervisorShared<'a> {
    event_sender: &'a Sender<RuntimeEvent>,
    sequence: &'a Arc<AtomicU64>,
    active_control: &'a Arc<Mutex<Option<ActiveRuntimeControl>>>,
    pending_approvals: &'a Arc<Mutex<BTreeMap<String, PendingApproval>>>,
}

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
        decision: ApprovalDecision,
        job: Box<ContextRetrievalJob>,
    },
    Shutdown,
}

pub struct RuntimeSupervisor {
    commands: Sender<SupervisorMessage>,
    events: Receiver<RuntimeEvent>,
    event_sender: Sender<RuntimeEvent>,
    sequence: Arc<AtomicU64>,
    active_control: Arc<Mutex<Option<ActiveRuntimeControl>>>,
    pending_approvals: Arc<Mutex<BTreeMap<String, PendingApproval>>>,
    _worker: JoinHandle<()>,
}

impl RuntimeSupervisor {
    pub fn start(mut engine: SessionEngine) -> Self {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        let sequence = Arc::new(AtomicU64::new(1));
        let active_control = Arc::new(Mutex::new(None));
        let pending_approvals = Arc::new(Mutex::new(BTreeMap::new()));

        let sink_event_sender = event_sender.clone();
        let sink_sequence = Arc::clone(&sequence);
        engine.set_runtime_event_sink(Some(Arc::new(move |events| {
            emit_events(&sink_event_sender, &sink_sequence, events);
        })));

        let worker_event_sender = event_sender.clone();
        let worker_sequence = Arc::clone(&sequence);
        let worker_active_control = Arc::clone(&active_control);
        let worker_pending_approvals = Arc::clone(&pending_approvals);
        let worker = thread::spawn(move || {
            run_supervisor_worker(
                engine,
                command_receiver,
                worker_event_sender,
                worker_sequence,
                worker_active_control,
                worker_pending_approvals,
            );
        });

        Self {
            commands: command_sender,
            events: event_receiver,
            event_sender,
            sequence,
            active_control,
            pending_approvals,
            _worker: worker,
        }
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
        match command {
            RuntimeCommand::CancelActiveTurn => {
                let Some(control) = self
                    .active_control
                    .lock()
                    .map_err(|_| "active turn lock poisoned".to_string())?
                    .clone()
                else {
                    emit_event(
                        &self.event_sender,
                        &self.sequence,
                        RuntimeEventKind::CommandRejected {
                            command_id,
                            reason: "no active turn to cancel".to_string(),
                        },
                    );
                    return Ok(());
                };
                control.control.cancel();
                if let ActiveJobState::PendingApproval { request_id } = &control.state {
                    if let Ok(mut approvals) = self.pending_approvals.lock() {
                        approvals.remove(request_id);
                    }
                    clear_active_control(&self.active_control, &control.owner_id);
                    emit_event(
                        &self.event_sender,
                        &self.sequence,
                        approval_resolved_event(
                            request_id,
                            ApprovalDecision::Deny,
                            RuntimeOwner::default(),
                            fresh_id("audit"),
                        ),
                    );
                }
                emit_event(
                    &self.event_sender,
                    &self.sequence,
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
                let pending = {
                    let mut approvals = self
                        .pending_approvals
                        .lock()
                        .map_err(|_| "approval lock poisoned".to_string())?;
                    let Some(pending) = approvals.get(&request_id) else {
                        emit_event(
                            &self.event_sender,
                            &self.sequence,
                            RuntimeEventKind::CommandRejected {
                                command_id,
                                reason: format!("approval request `{request_id}` is not pending"),
                            },
                        );
                        return Ok(());
                    };
                    if pending.owner != owner {
                        emit_event(
                            &self.event_sender,
                            &self.sequence,
                            RuntimeEventKind::CommandRejected {
                                command_id,
                                reason: format!("approval request `{request_id}` owner mismatch"),
                            },
                        );
                        return Ok(());
                    }
                    approvals.remove(&request_id).expect("pending approval")
                };
                emit_event(
                    &self.event_sender,
                    &self.sequence,
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
                match pending.target {
                    PendingApprovalTarget::Channel(sender) => {
                        sender
                            .send(response.clone())
                            .map_err(|err| format!("failed to send approval response: {err}"))?;
                    }
                    PendingApprovalTarget::ContextRetrieval { owner_id, job } => {
                        if response.is_allowed() {
                            let mut job = *job;
                            job.permission_decision = "approved".to_string();
                            self.commands
                                .send(SupervisorMessage::ResumeContextRetrieval {
                                    owner_id,
                                    request_id,
                                    owner: pending.owner,
                                    audit_id: pending.audit_id,
                                    decision: response.decision.clone(),
                                    job: Box::new(job),
                                })
                                .map_err(|err| format!("runtime supervisor stopped: {err}"))?;
                        } else {
                            emit_event(
                                &self.event_sender,
                                &self.sequence,
                                approval_resolved_event(
                                    &request_id,
                                    response.decision.clone(),
                                    pending.owner,
                                    pending.audit_id,
                                ),
                            );
                            emit_error(
                                &self.event_sender,
                                &self.sequence,
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
                if let Some(owner_id) = active_owner_id(&self.active_control) {
                    emit_event(
                        &self.event_sender,
                        &self.sequence,
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
        self.events.recv_timeout(timeout).ok()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn expire_pending_approvals_for_test(&self, now: u64) {
        let expired = match self.pending_approvals.lock() {
            Ok(mut approvals) => {
                let expired_ids = approvals
                    .iter()
                    .filter(|(_, approval)| approval.expires_at <= now)
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>();
                expired_ids
                    .into_iter()
                    .filter_map(|id| approvals.remove(&id).map(|approval| (id, approval)))
                    .collect::<Vec<_>>()
            }
            Err(_) => Vec::new(),
        };
        for (request_id, pending) in expired {
            match pending.target {
                PendingApprovalTarget::Channel(sender) => {
                    let _ = sender.send(ApprovalResponse::deny(Some(
                        "approval expired; default action deny".to_string(),
                    )));
                }
                PendingApprovalTarget::ContextRetrieval { owner_id, .. } => {
                    emit_event(
                        &self.event_sender,
                        &self.sequence,
                        approval_resolved_event(
                            &request_id,
                            ApprovalDecision::Deny,
                            pending.owner,
                            pending.audit_id,
                        ),
                    );
                    emit_error(
                        &self.event_sender,
                        &self.sequence,
                        "Approval expired; default action deny".to_string(),
                    );
                    clear_active_control(&self.active_control, &owner_id);
                }
            }
        }
    }
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        let _ = self.commands.send(SupervisorMessage::Shutdown);
    }
}

fn run_supervisor_worker(
    mut engine: SessionEngine,
    command_receiver: Receiver<SupervisorMessage>,
    event_sender: Sender<RuntimeEvent>,
    sequence: Arc<AtomicU64>,
    active_control: Arc<Mutex<Option<ActiveRuntimeControl>>>,
    pending_approvals: Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    while let Ok(message) = command_receiver.recv() {
        match message {
            SupervisorMessage::Shutdown => break,
            SupervisorMessage::Command {
                owner,
                command_id,
                command,
            } => match command {
                RuntimeCommand::SubmitUserInput { content } => {
                    run_supervised_input(
                        &mut engine,
                        owner,
                        command_id,
                        content,
                        &event_sender,
                        &sequence,
                        &active_control,
                        &pending_approvals,
                    );
                }
                RuntimeCommand::StartAgentTask { task_id } => {
                    run_supervised_agent_task(
                        &mut engine,
                        owner,
                        command_id,
                        task_id,
                        &event_sender,
                        &sequence,
                        &active_control,
                        &pending_approvals,
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
                            event_sender: &event_sender,
                            sequence: &sequence,
                            active_control: &active_control,
                            pending_approvals: &pending_approvals,
                        },
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
                        Ok(events) => emit_events(&event_sender, &sequence, events),
                        Err(err) => emit_error(&event_sender, &sequence, err),
                    }
                }
            },
            SupervisorMessage::ResumeContextRetrieval {
                owner_id,
                request_id,
                owner,
                audit_id,
                decision,
                job,
            } => {
                resume_context_retrieval_after_approval(
                    &mut engine,
                    owner_id,
                    request_id,
                    owner,
                    audit_id,
                    decision,
                    *job,
                    &event_sender,
                    &sequence,
                    &active_control,
                );
            }
        }
    }
}

fn run_supervised_context_retrieval(
    engine: &mut SessionEngine,
    owner: RuntimeOwner,
    command_id: String,
    handle_id: String,
    reason: String,
    shared: SupervisorShared<'_>,
) {
    if let Some(owner_id) = active_owner_id(shared.active_control) {
        emit_event(
            shared.event_sender,
            shared.sequence,
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
                shared.event_sender,
                shared.sequence,
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
                control.clone(),
                ActiveJobState::Running,
            ) {
                emit_event(
                    shared.event_sender,
                    shared.sequence,
                    RuntimeEventKind::CommandRejected {
                        command_id,
                        reason: err,
                    },
                );
                return;
            }
            emit_event(
                shared.event_sender,
                shared.sequence,
                RuntimeEventKind::CommandAccepted {
                    command_id: command_id.clone(),
                    command: redacted_runtime_command_for_event(&RuntimeCommand::RetrieveContext {
                        handle_id: handle_id.clone(),
                        reason: reason.clone(),
                    }),
                },
            );
            emit_events(shared.event_sender, shared.sequence, prepared.pre_events);
            start_context_retrieval_worker(
                command_id,
                prepared.job,
                control,
                shared.event_sender,
                shared.sequence,
                shared.active_control,
            );
        }
        SupervisorContextRetrievalPreparation::PendingApproval { mut approval, job } => {
            approval.owner = owner;
            approval.audit_id = fresh_id("audit");
            approval.expires_at = now_timestamp().saturating_add(300);
            let control = ModelRequestControl::new();
            if let Err(err) = acquire_active_job(
                shared.active_control,
                command_id.clone(),
                control,
                ActiveJobState::PendingApproval {
                    request_id: approval.id.clone(),
                },
            ) {
                emit_event(
                    shared.event_sender,
                    shared.sequence,
                    RuntimeEventKind::CommandRejected {
                        command_id,
                        reason: err,
                    },
                );
                return;
            }
            emit_event(
                shared.event_sender,
                shared.sequence,
                RuntimeEventKind::CommandAccepted {
                    command_id: command_id.clone(),
                    command: redacted_runtime_command_for_event(&RuntimeCommand::RetrieveContext {
                        handle_id: handle_id.clone(),
                        reason: reason.clone(),
                    }),
                },
            );
            if let Ok(mut approvals) = shared.pending_approvals.lock() {
                approvals.insert(
                    approval.id.clone(),
                    PendingApproval {
                        owner: approval.owner.clone(),
                        audit_id: approval.audit_id.clone(),
                        expires_at: approval.expires_at,
                        target: PendingApprovalTarget::ContextRetrieval {
                            owner_id: command_id,
                            job: Box::new(job),
                        },
                    },
                );
            }
            emit_event(
                shared.event_sender,
                shared.sequence,
                RuntimeEventKind::ApprovalRequested { approval },
            );
        }
    }
}

fn start_context_retrieval_worker(
    owner_id: String,
    job: ContextRetrievalJob,
    control: ModelRequestControl,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
) {
    let worker_event_sender = event_sender.clone();
    let worker_sequence = Arc::clone(sequence);
    let worker_active_control = Arc::clone(active_control);
    thread::spawn(move || {
        let result = execute_context_retrieval_job(job, &control);
        clear_active_control(&worker_active_control, &owner_id);
        match result {
            Ok(events) => {
                if control.check_cancelled().is_ok() {
                    emit_events(&worker_event_sender, &worker_sequence, events);
                } else {
                    emit_error(
                        &worker_event_sender,
                        &worker_sequence,
                        "Model request cancelled".to_string(),
                    );
                }
            }
            Err(err) => emit_error(&worker_event_sender, &worker_sequence, err),
        }
    });
}

fn acquire_active_job(
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
    owner_id: String,
    control: ModelRequestControl,
    state: ActiveJobState,
) -> Result<(), String> {
    let mut slot = active_control
        .lock()
        .map_err(|_| "active turn lock poisoned".to_string())?;
    if let Some(active) = slot.as_ref() {
        return Err(format!(
            "active runtime job `{}` is already running",
            active.owner_id
        ));
    }
    *slot = Some(ActiveRuntimeControl {
        owner_id,
        control,
        state,
    });
    Ok(())
}

fn active_control_for_owner(
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
    owner_id: &str,
) -> Option<ModelRequestControl> {
    active_control.lock().ok().and_then(|slot| {
        slot.as_ref()
            .filter(|active| active.owner_id == owner_id)
            .map(|active| active.control.clone())
    })
}

fn mark_active_running(
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
    owner_id: &str,
) -> Result<(), String> {
    let mut slot = active_control
        .lock()
        .map_err(|_| "active turn lock poisoned".to_string())?;
    let Some(active) = slot.as_mut().filter(|active| active.owner_id == owner_id) else {
        return Err(format!(
            "active runtime job `{owner_id}` is no longer pending"
        ));
    };
    active.state = ActiveJobState::Running;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resume_context_retrieval_after_approval(
    engine: &mut SessionEngine,
    owner_id: String,
    request_id: String,
    owner: RuntimeOwner,
    audit_id: String,
    decision: ApprovalDecision,
    job: ContextRetrievalJob,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
) {
    let Some(control) = active_control_for_owner(active_control, &owner_id) else {
        emit_error(
            event_sender,
            sequence,
            format!("active runtime job `{owner_id}` is no longer pending"),
        );
        return;
    };
    if let Err(err) = engine.validate_context_retrieval_job_for_supervisor(&job) {
        clear_active_control(active_control, &owner_id);
        emit_event(
            event_sender,
            sequence,
            approval_resolved_event(
                &request_id,
                ApprovalDecision::Deny,
                owner.clone(),
                audit_id.clone(),
            ),
        );
        emit_error(event_sender, sequence, err);
        return;
    }
    if let Err(err) = mark_active_running(active_control, &owner_id) {
        emit_error(event_sender, sequence, err);
        return;
    }
    emit_event(
        event_sender,
        sequence,
        approval_resolved_event(&request_id, decision, owner, audit_id),
    );
    start_context_retrieval_worker(
        owner_id,
        job,
        control,
        event_sender,
        sequence,
        active_control,
    );
}

fn active_owner_id(active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>) -> Option<String> {
    active_control
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|active| active.owner_id.clone()))
}

fn clear_active_control(active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>, owner_id: &str) {
    if let Ok(mut slot) = active_control.lock()
        && slot
            .as_ref()
            .is_some_and(|active| active.owner_id == owner_id)
    {
        *slot = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn run_supervised_agent_task(
    engine: &mut SessionEngine,
    owner: RuntimeOwner,
    command_id: String,
    task_id: String,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    let control = ModelRequestControl::new();
    if let Err(err) = acquire_active_job(
        active_control,
        command_id.clone(),
        control.clone(),
        ActiveJobState::Running,
    ) {
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::CommandRejected {
                command_id,
                reason: err,
            },
        );
        return;
    }
    emit_event(
        event_sender,
        sequence,
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
        let approval = approval_request_view(&request_id, &prompt, owner.clone());
        if let Ok(mut approvals) = pending_approvals.lock() {
            approvals.insert(
                request_id.clone(),
                PendingApproval {
                    owner: approval.owner.clone(),
                    audit_id: approval.audit_id.clone(),
                    expires_at: approval.expires_at,
                    target: PendingApprovalTarget::Channel(approval_sender),
                },
            );
        }
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::ApprovalRequested {
                approval: approval.clone(),
            },
        );
        let response = approval_receiver
            .recv()
            .unwrap_or(ApprovalResponse::deny(Some(
                "approval response channel closed".to_string(),
            )));
        emit_event(
            event_sender,
            sequence,
            approval_resolved_event(
                &request_id,
                response.decision.clone(),
                approval.owner,
                approval.audit_id,
            ),
        );
        response
    };

    let result = engine.run_agent_task_with_control(&task_id, &mut approver, &control);
    clear_active_control(active_control, &command_id);
    match result {
        Ok(events) => emit_events(event_sender, sequence, events),
        Err(err) => emit_error(event_sender, sequence, err),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_supervised_input(
    engine: &mut SessionEngine,
    owner: RuntimeOwner,
    command_id: String,
    content: String,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ActiveRuntimeControl>>>,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    let control = ModelRequestControl::new();
    if let Err(err) = acquire_active_job(
        active_control,
        command_id.clone(),
        control.clone(),
        ActiveJobState::Running,
    ) {
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::CommandRejected {
                command_id,
                reason: err,
            },
        );
        return;
    }
    emit_event(
        event_sender,
        sequence,
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
        let approval = approval_request_view(&request_id, &prompt, owner.clone());
        if let Ok(mut approvals) = pending_approvals.lock() {
            approvals.insert(
                request_id.clone(),
                PendingApproval {
                    owner: approval.owner.clone(),
                    audit_id: approval.audit_id.clone(),
                    expires_at: approval.expires_at,
                    target: PendingApprovalTarget::Channel(approval_sender),
                },
            );
        }
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::ApprovalRequested {
                approval: approval.clone(),
            },
        );
        let response = approval_receiver
            .recv()
            .unwrap_or(ApprovalResponse::deny(Some(
                "approval response channel closed".to_string(),
            )));
        emit_event(
            event_sender,
            sequence,
            approval_resolved_event(
                &request_id,
                response.decision.clone(),
                approval.owner,
                approval.audit_id,
            ),
        );
        response
    };

    let result = engine.process_input_with_approval_and_control(&content, &mut approver, &control);
    clear_active_control(active_control, &command_id);
    match result {
        Ok(events) => emit_events(
            event_sender,
            sequence,
            engine.runtime_events_for_engine_events(&events),
        ),
        Err(err) => emit_error(event_sender, sequence, err),
    }
}

fn approval_request_view(
    request_id: &str,
    prompt: &PermissionPrompt,
    owner: RuntimeOwner,
) -> ApprovalRequestView {
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
            canonical_ref: None,
        },
        allowed_scopes: vec![ApprovalScope::Once],
        policy_reason_key: "permission.requires_approval".to_string(),
        policy_reason_args: BTreeMap::new(),
        expires_at: now_timestamp().saturating_add(300),
        default_action: ApprovalDefaultAction::Deny,
        audit_id: fresh_id("audit"),
    }
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

fn emit_events(
    sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    events: Vec<RuntimeEvent>,
) {
    for event in events {
        emit_event(sender, sequence, event.kind);
    }
}

fn emit_error(sender: &Sender<RuntimeEvent>, sequence: &Arc<AtomicU64>, message: String) {
    emit_event(
        sender,
        sequence,
        RuntimeEventKind::Error {
            error: RuntimeErrorView {
                message,
                recoverable: true,
                hint: None,
            },
        },
    );
}

fn emit_event(sender: &Sender<RuntimeEvent>, sequence: &Arc<AtomicU64>, kind: RuntimeEventKind) {
    let event = RuntimeEvent::new(sequence.fetch_add(1, Ordering::SeqCst), kind);
    let _ = sender.send(event);
}
