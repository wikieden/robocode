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
    ApprovalRequestView, ApprovalResponse, PermissionPrompt, RuntimeCommand, RuntimeErrorView,
    RuntimeEvent, RuntimeEventKind, fresh_id,
};

use crate::{
    SessionEngine,
    runtime_contract::{
        ContextRetrievalJob, SupervisorContextRetrievalPreparation, execute_context_retrieval_job,
        redacted_runtime_command_for_event,
    },
};

enum PendingApproval {
    Channel(Sender<ApprovalResponse>),
    ContextRetrieval(Box<ContextRetrievalJob>),
}

struct SupervisorShared<'a> {
    event_sender: &'a Sender<RuntimeEvent>,
    sequence: &'a Arc<AtomicU64>,
    active_control: &'a Arc<Mutex<Option<ModelRequestControl>>>,
    pending_approvals: &'a Arc<Mutex<BTreeMap<String, PendingApproval>>>,
}

enum SupervisorMessage {
    Command {
        command_id: String,
        command: RuntimeCommand,
    },
    Shutdown,
}

pub struct RuntimeSupervisor {
    commands: Sender<SupervisorMessage>,
    events: Receiver<RuntimeEvent>,
    event_sender: Sender<RuntimeEvent>,
    sequence: Arc<AtomicU64>,
    active_control: Arc<Mutex<Option<ModelRequestControl>>>,
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
        let command_id = command_id.into();
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
                control.cancel();
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
                let Some(pending) = self
                    .pending_approvals
                    .lock()
                    .map_err(|_| "approval lock poisoned".to_string())?
                    .remove(&request_id)
                else {
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
                match pending {
                    PendingApproval::Channel(sender) => {
                        sender
                            .send(response.clone())
                            .map_err(|err| format!("failed to send approval response: {err}"))?;
                    }
                    PendingApproval::ContextRetrieval(job) => {
                        emit_event(
                            &self.event_sender,
                            &self.sequence,
                            RuntimeEventKind::ApprovalResolved {
                                request_id,
                                approved: response.approved,
                            },
                        );
                        if response.approved {
                            let mut job = *job;
                            job.permission_decision = "approved".to_string();
                            start_context_retrieval_worker(
                                job,
                                &self.event_sender,
                                &self.sequence,
                                &self.active_control,
                            );
                        } else {
                            emit_error(
                                &self.event_sender,
                                &self.sequence,
                                "User denied the permission request".to_string(),
                            );
                        }
                    }
                }
                Ok(())
            }
            command => self
                .commands
                .send(SupervisorMessage::Command {
                    command_id,
                    command,
                })
                .map_err(|err| format!("runtime supervisor stopped: {err}")),
        }
    }

    pub fn recv_event_timeout(&self, timeout: Duration) -> Option<RuntimeEvent> {
        self.events.recv_timeout(timeout).ok()
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
    active_control: Arc<Mutex<Option<ModelRequestControl>>>,
    pending_approvals: Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    while let Ok(message) = command_receiver.recv() {
        match message {
            SupervisorMessage::Shutdown => break,
            SupervisorMessage::Command {
                command_id,
                command,
            } => match command {
                RuntimeCommand::SubmitUserInput { content } => {
                    run_supervised_input(
                        &mut engine,
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
                    let mut approver = |_prompt: PermissionPrompt| ApprovalResponse {
                        approved: false,
                        feedback: Some(
                            "runtime supervisor command path does not own this approval"
                                .to_string(),
                        ),
                    };
                    match engine.handle_runtime_command(command_id, command, &mut approver) {
                        Ok(events) => emit_events(&event_sender, &sequence, events),
                        Err(err) => emit_error(&event_sender, &sequence, err),
                    }
                }
            },
        }
    }
}

fn run_supervised_context_retrieval(
    engine: &mut SessionEngine,
    command_id: String,
    handle_id: String,
    reason: String,
    shared: SupervisorShared<'_>,
) {
    emit_event(
        shared.event_sender,
        shared.sequence,
        RuntimeEventKind::CommandAccepted {
            command_id,
            command: redacted_runtime_command_for_event(&RuntimeCommand::RetrieveContext {
                handle_id: handle_id.clone(),
                reason: reason.clone(),
            }),
        },
    );
    let prepared = match engine.prepare_context_retrieval_for_supervisor(&handle_id, &reason) {
        Ok(prepared) => prepared,
        Err(err) => {
            emit_event(
                shared.event_sender,
                shared.sequence,
                RuntimeEventKind::CommandRejected {
                    command_id: "retrieve_context".to_string(),
                    reason: err,
                },
            );
            return;
        }
    };
    match prepared {
        SupervisorContextRetrievalPreparation::Ready(prepared) => {
            emit_events(shared.event_sender, shared.sequence, prepared.pre_events);
            start_context_retrieval_worker(
                prepared.job,
                shared.event_sender,
                shared.sequence,
                shared.active_control,
            );
        }
        SupervisorContextRetrievalPreparation::PendingApproval { approval, job } => {
            if let Ok(mut approvals) = shared.pending_approvals.lock() {
                approvals.insert(
                    approval.id.clone(),
                    PendingApproval::ContextRetrieval(Box::new(job)),
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
    job: ContextRetrievalJob,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ModelRequestControl>>>,
) {
    let control = ModelRequestControl::new();
    if let Ok(mut slot) = active_control.lock() {
        *slot = Some(control.clone());
    }
    let worker_event_sender = event_sender.clone();
    let worker_sequence = Arc::clone(sequence);
    let worker_active_control = Arc::clone(active_control);
    thread::spawn(move || {
        let result = execute_context_retrieval_job(job, &control);
        if let Ok(mut slot) = worker_active_control.lock() {
            *slot = None;
        }
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

fn run_supervised_agent_task(
    engine: &mut SessionEngine,
    command_id: String,
    task_id: String,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ModelRequestControl>>>,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    emit_event(
        event_sender,
        sequence,
        RuntimeEventKind::CommandAccepted {
            command_id,
            command: redacted_runtime_command_for_event(&RuntimeCommand::StartAgentTask {
                task_id: task_id.clone(),
            }),
        },
    );

    let control = ModelRequestControl::new();
    if let Ok(mut slot) = active_control.lock() {
        *slot = Some(control.clone());
    }

    let mut approver = |prompt: PermissionPrompt| {
        let request_id = fresh_id("approval");
        let (approval_sender, approval_receiver) = mpsc::channel();
        if let Ok(mut approvals) = pending_approvals.lock() {
            approvals.insert(
                request_id.clone(),
                PendingApproval::Channel(approval_sender),
            );
        }
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::ApprovalRequested {
                approval: approval_request_view(&request_id, &prompt),
            },
        );
        let response = approval_receiver.recv().unwrap_or(ApprovalResponse {
            approved: false,
            feedback: Some("approval response channel closed".to_string()),
        });
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::ApprovalResolved {
                request_id,
                approved: response.approved,
            },
        );
        response
    };

    let result = engine.run_agent_task_with_control(&task_id, &mut approver, &control);
    if let Ok(mut slot) = active_control.lock() {
        *slot = None;
    }
    match result {
        Ok(events) => emit_events(event_sender, sequence, events),
        Err(err) => emit_error(event_sender, sequence, err),
    }
}

fn run_supervised_input(
    engine: &mut SessionEngine,
    command_id: String,
    content: String,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ModelRequestControl>>>,
    pending_approvals: &Arc<Mutex<BTreeMap<String, PendingApproval>>>,
) {
    emit_event(
        event_sender,
        sequence,
        RuntimeEventKind::CommandAccepted {
            command_id,
            command: redacted_runtime_command_for_event(&RuntimeCommand::SubmitUserInput {
                content: content.clone(),
            }),
        },
    );

    let control = ModelRequestControl::new();
    if let Ok(mut slot) = active_control.lock() {
        *slot = Some(control.clone());
    }

    let mut approver = |prompt: PermissionPrompt| {
        let request_id = fresh_id("approval");
        let (approval_sender, approval_receiver) = mpsc::channel();
        if let Ok(mut approvals) = pending_approvals.lock() {
            approvals.insert(
                request_id.clone(),
                PendingApproval::Channel(approval_sender),
            );
        }
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::ApprovalRequested {
                approval: approval_request_view(&request_id, &prompt),
            },
        );
        let response = approval_receiver.recv().unwrap_or(ApprovalResponse {
            approved: false,
            feedback: Some("approval response channel closed".to_string()),
        });
        emit_event(
            event_sender,
            sequence,
            RuntimeEventKind::ApprovalResolved {
                request_id,
                approved: response.approved,
            },
        );
        response
    };

    let result = engine.process_input_with_approval_and_control(&content, &mut approver, &control);
    if let Ok(mut slot) = active_control.lock() {
        *slot = None;
    }
    match result {
        Ok(events) => emit_events(
            event_sender,
            sequence,
            engine.runtime_events_for_engine_events(&events),
        ),
        Err(err) => emit_error(event_sender, sequence, err),
    }
}

fn approval_request_view(request_id: &str, prompt: &PermissionPrompt) -> ApprovalRequestView {
    ApprovalRequestView {
        id: request_id.to_string(),
        tool_name: prompt.tool_name.clone(),
        title: format!("Approve {}", prompt.tool_name),
        message: prompt.message.clone(),
        input_preview: prompt.input_preview.clone(),
        is_mutating: true,
        reason: Some(prompt.message.clone()),
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
