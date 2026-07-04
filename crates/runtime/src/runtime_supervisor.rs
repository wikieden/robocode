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

use crate::SessionEngine;

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
    pending_approvals: Arc<Mutex<BTreeMap<String, Sender<ApprovalResponse>>>>,
    _worker: JoinHandle<()>,
}

impl RuntimeSupervisor {
    pub fn start(engine: SessionEngine) -> Self {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        let sequence = Arc::new(AtomicU64::new(1));
        let active_control = Arc::new(Mutex::new(None));
        let pending_approvals = Arc::new(Mutex::new(BTreeMap::new()));

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
                        command: RuntimeCommand::CancelActiveTurn,
                    },
                );
                Ok(())
            }
            RuntimeCommand::RespondToApproval {
                request_id,
                response,
            } => {
                let Some(sender) = self
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
                sender
                    .send(response.clone())
                    .map_err(|err| format!("failed to send approval response: {err}"))?;
                emit_event(
                    &self.event_sender,
                    &self.sequence,
                    RuntimeEventKind::CommandAccepted {
                        command_id,
                        command: RuntimeCommand::RespondToApproval {
                            request_id,
                            response,
                        },
                    },
                );
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

    pub fn try_recv_event(&self) -> Option<RuntimeEvent> {
        self.events.try_recv().ok()
    }

    pub fn is_turn_active(&self) -> bool {
        self.active_control
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
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
    pending_approvals: Arc<Mutex<BTreeMap<String, Sender<ApprovalResponse>>>>,
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

fn run_supervised_input(
    engine: &mut SessionEngine,
    command_id: String,
    content: String,
    event_sender: &Sender<RuntimeEvent>,
    sequence: &Arc<AtomicU64>,
    active_control: &Arc<Mutex<Option<ModelRequestControl>>>,
    pending_approvals: &Arc<Mutex<BTreeMap<String, Sender<ApprovalResponse>>>>,
) {
    emit_event(
        event_sender,
        sequence,
        RuntimeEventKind::CommandAccepted {
            command_id,
            command: RuntimeCommand::SubmitUserInput {
                content: content.clone(),
            },
        },
    );

    let stream_message_id = fresh_id("stream");
    let stream_sender = event_sender.clone();
    let stream_sequence = Arc::clone(sequence);
    let control = ModelRequestControl::with_streaming_sink(true, move |delta| {
        emit_event(
            &stream_sender,
            &stream_sequence,
            RuntimeEventKind::AssistantDelta {
                message_id: stream_message_id.clone(),
                task_id: None,
                content: delta,
            },
        );
    });
    if let Ok(mut slot) = active_control.lock() {
        *slot = Some(control.clone());
    }

    let mut approver = |prompt: PermissionPrompt| {
        let request_id = fresh_id("approval");
        let (approval_sender, approval_receiver) = mpsc::channel();
        if let Ok(mut approvals) = pending_approvals.lock() {
            approvals.insert(request_id.clone(), approval_sender);
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
