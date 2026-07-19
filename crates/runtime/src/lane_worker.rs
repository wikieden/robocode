use std::sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
};
use std::thread::{self, JoinHandle};

use viden_types::{
    AgentLaneRecord, ApprovalDecision, ApprovalRequestView, ApprovalResponse, LaneStatus,
    RuntimeCommand, RuntimeErrorView, RuntimeEventKind, RuntimeOwner, fresh_id, now_timestamp,
};
use viden_workflows::{lanes::LaneEvent, stores::WorkflowStore};

use crate::lane_runtime::{LaneEffectExecutor, LaneEffectRequest};

pub(crate) type LaneEventSink = Arc<dyn Fn(RuntimeOwner, RuntimeEventKind) + Send + Sync>;

pub(crate) enum LaneWorkerMessage {
    Command {
        command_id: String,
        command: RuntimeCommand,
    },
    ResumeApproval {
        request_id: String,
        response: ApprovalResponse,
    },
    Cancel {
        command_id: String,
    },
    Shutdown,
}

pub(crate) struct LaneWorkerHandle {
    pub(crate) owner: RuntimeOwner,
    pending_approval: Arc<Mutex<Option<String>>>,
    sender: Sender<LaneWorkerMessage>,
    _worker: JoinHandle<()>,
}

impl LaneWorkerHandle {
    pub(crate) fn spawn(
        owner: RuntimeOwner,
        lane: AgentLaneRecord,
        repo: std::path::PathBuf,
        workflows: WorkflowStore,
        effects: Arc<dyn LaneEffectExecutor>,
        events: LaneEventSink,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let pending_approval = Arc::new(Mutex::new(None));
        let worker_pending_approval = Arc::clone(&pending_approval);
        let worker_owner = owner.clone();
        let worker_lane = lane.clone();
        let worker = thread::spawn(move || {
            let mut runtime = LaneWorker {
                owner: worker_owner,
                lane: worker_lane,
                repo,
                workflows,
                effects,
                events,
                pending_start: None,
                pending_approval: worker_pending_approval,
            };
            while let Ok(message) = receiver.recv() {
                if matches!(message, LaneWorkerMessage::Shutdown) {
                    break;
                }
                runtime.handle(message);
            }
        });
        Self {
            owner,
            pending_approval,
            sender,
            _worker: worker,
        }
    }

    pub(crate) fn send(&self, message: LaneWorkerMessage) -> Result<(), String> {
        self.sender
            .send(message)
            .map_err(|error| format!("lane worker stopped: {error}"))
    }

    pub(crate) fn owns_pending_approval(&self, request_id: &str) -> bool {
        self.pending_approval
            .lock()
            .is_ok_and(|pending| pending.as_deref() == Some(request_id))
    }
}

impl Drop for LaneWorkerHandle {
    fn drop(&mut self) {
        let _ = self.sender.send(LaneWorkerMessage::Shutdown);
    }
}

struct PendingStart {
    request_id: String,
    command_id: String,
    request: LaneEffectRequest,
}

struct LaneWorker {
    owner: RuntimeOwner,
    lane: AgentLaneRecord,
    repo: std::path::PathBuf,
    workflows: WorkflowStore,
    effects: Arc<dyn LaneEffectExecutor>,
    events: LaneEventSink,
    pending_start: Option<PendingStart>,
    pending_approval: Arc<Mutex<Option<String>>>,
}

impl LaneWorker {
    fn handle(&mut self, message: LaneWorkerMessage) {
        match message {
            LaneWorkerMessage::Command {
                command_id,
                command,
            } => self.handle_command(command_id, command),
            LaneWorkerMessage::ResumeApproval {
                request_id,
                response,
            } => self.resume_approval(request_id, response),
            LaneWorkerMessage::Cancel { command_id } => self.cancel(command_id),
            LaneWorkerMessage::Shutdown => {}
        }
    }

    fn handle_command(&mut self, command_id: String, command: RuntimeCommand) {
        match command {
            RuntimeCommand::StartLane {
                command,
                args,
                env,
                output_log,
                ..
            } => {
                let request = LaneEffectRequest::Start {
                    repo: self.repo.clone(),
                    lane: self.lane.clone(),
                    command,
                    args,
                    env,
                    output_log,
                };
                match self.lane.mutation_policy {
                    viden_types::MutationPolicy::Autonomous => self.start(request),
                    viden_types::MutationPolicy::ProposeOnly => {
                        let request_id = fresh_id("lane-approval");
                        if self
                            .change_status(
                                LaneStatus::WaitingApproval,
                                "lane start awaits approval",
                            )
                            .is_err()
                        {
                            return;
                        }
                        let approval = lane_approval(&request_id, &self.owner, &self.lane);
                        self.pending_start = Some(PendingStart {
                            request_id: request_id.clone(),
                            command_id,
                            request,
                        });
                        if let Ok(mut pending) = self.pending_approval.lock() {
                            *pending = Some(request_id);
                        }
                        self.emit(RuntimeEventKind::ApprovalRequested { approval });
                    }
                    viden_types::MutationPolicy::ReadOnly => {
                        self.reject(command_id, "read-only lane cannot start a mutating runtime")
                    }
                }
            }
            RuntimeCommand::StopLane { .. } => {
                self.execute_effect(
                    LaneEffectRequest::Stop {
                        lane_id: self.lane.id.clone(),
                    },
                    LaneStatus::Detached,
                    "lane stopped",
                );
            }
            RuntimeCommand::AttachLane { .. } => {
                let _ = self.change_status(LaneStatus::Attached, "lane attached");
            }
            RuntimeCommand::DetachLane { .. } => {
                let _ = self.change_status(LaneStatus::Detached, "lane detached");
            }
            RuntimeCommand::SendLaneInput { input, .. } => {
                self.emit(RuntimeEventKind::InputQueued {
                    input: viden_types::QueuedInputView {
                        id: fresh_id("lane-input"),
                        content_preview: input.chars().take(160).collect(),
                        created_at: Some(now_timestamp()),
                    },
                });
                self.execute_effect(
                    LaneEffectRequest::SendInput {
                        lane_id: self.lane.id.clone(),
                        input,
                    },
                    self.lane.status,
                    "lane input delivered",
                );
            }
            RuntimeCommand::AcceptLaneOutput { summary, .. } => {
                let _ = self.change_status(LaneStatus::Done, summary);
            }
            RuntimeCommand::ReviseLaneOutput { feedback, .. } => {
                let _ = self.change_status(LaneStatus::NeedsInput, feedback);
            }
            RuntimeCommand::DiscardLaneOutput { reason, .. } => {
                let _ = self.change_status(LaneStatus::Cancelled, reason);
            }
            RuntimeCommand::ApplyLaneChanges { unified_diff, .. }
            | RuntimeCommand::ResolveLaneConflict { unified_diff, .. } => {
                self.apply(unified_diff);
            }
            RuntimeCommand::ArchiveLane { summary, .. } => {
                let _ = self.change_status(LaneStatus::Archived, summary);
            }
            RuntimeCommand::CleanupLane { force, .. } => {
                self.execute_effect(
                    LaneEffectRequest::Cleanup {
                        repo: self.repo.clone(),
                        lane: self.lane.clone(),
                        force,
                    },
                    LaneStatus::Archived,
                    "lane cleaned up",
                );
            }
            _ => self.reject(command_id, "command is not a lane lifecycle command"),
        }
    }

    fn resume_approval(&mut self, request_id: String, response: ApprovalResponse) {
        let Some(pending) = self.pending_start.take() else {
            return;
        };
        if pending.request_id != request_id {
            self.pending_start = Some(pending);
            return;
        }
        if let Ok(mut approval) = self.pending_approval.lock() {
            *approval = None;
        }
        self.emit(RuntimeEventKind::ApprovalResolved {
            request_id,
            decision: response.decision.clone(),
            owner: self.owner.clone(),
            audit_id: fresh_id("audit"),
        });
        if response.is_allowed() {
            self.start(pending.request);
        } else {
            let _ = self.change_status(LaneStatus::Cancelled, "lane start denied");
            self.reject(pending.command_id, "lane start approval denied");
        }
    }

    fn cancel(&mut self, command_id: String) {
        if let Some(pending) = self.pending_start.take() {
            if let Ok(mut approval) = self.pending_approval.lock() {
                *approval = None;
            }
            self.emit(RuntimeEventKind::ApprovalResolved {
                request_id: pending.request_id,
                decision: ApprovalDecision::Deny,
                owner: self.owner.clone(),
                audit_id: fresh_id("audit"),
            });
            let _ = self.change_status(
                LaneStatus::Cancelled,
                "lane cancelled while awaiting approval",
            );
        } else {
            self.execute_effect(
                LaneEffectRequest::Stop {
                    lane_id: self.lane.id.clone(),
                },
                LaneStatus::Cancelled,
                "lane cancelled",
            );
        }
        self.emit(RuntimeEventKind::CommandAccepted {
            command_id,
            command: RuntimeCommand::CancelActiveTurn,
        });
    }

    fn start(&mut self, request: LaneEffectRequest) {
        if self
            .change_status(LaneStatus::Starting, "lane starting")
            .is_err()
        {
            return;
        }
        self.execute_effect(request, LaneStatus::Running, "lane running");
    }

    fn apply(&mut self, unified_diff: String) {
        match self.effects.execute(LaneEffectRequest::Apply {
            cwd: self.repo.clone(),
            unified_diff,
        }) {
            Ok(result) if result.conflict_paths.is_empty() => {
                self.output("receipt", result.output);
                let _ = self.change_status(LaneStatus::Done, "lane changes applied");
            }
            Ok(result) => {
                self.emit(RuntimeEventKind::LaneConflictDetected {
                    lane_id: self.lane.id.clone(),
                    summary: result.output,
                    paths: result.conflict_paths,
                });
                let _ = self.change_status(LaneStatus::Blocked, "lane patch conflict");
            }
            Err(error) => self.fail(error),
        }
    }

    fn execute_effect(&mut self, request: LaneEffectRequest, status: LaneStatus, summary: &str) {
        match self.effects.execute(request) {
            Ok(result) => {
                self.output("receipt", result.output);
                let _ = self.change_status(status, summary);
            }
            Err(error) => self.fail(error),
        }
    }

    fn change_status(&mut self, status: LaneStatus, summary: impl Into<String>) -> Result<(), ()> {
        let summary = summary.into();
        let event = LaneEvent::status_changed(
            fresh_id("lane-event"),
            self.lane.id.clone(),
            status,
            summary.clone(),
            now_timestamp(),
            self.owner.session_id.clone(),
        );
        if let Err(error) = self.workflows.append_lane_event_checked(&event) {
            self.emit(RuntimeEventKind::LaneRecoveryRequired {
                lane_id: self.lane.id.clone(),
                reason: error.clone(),
                next_action: "reload lane state and retry".to_string(),
            });
            self.error(error);
            return Err(());
        }
        self.lane.status = status;
        self.lane.summary = summary;
        self.emit(RuntimeEventKind::LaneUpdated {
            lane: self.lane.clone(),
        });
        Ok(())
    }

    fn output(&self, stream: &str, content: String) {
        self.emit(RuntimeEventKind::LaneOutputAppended {
            lane_id: self.lane.id.clone(),
            stream: stream.to_string(),
            content,
        });
    }

    fn fail(&mut self, error: String) {
        let _ = self.change_status(LaneStatus::Failed, error.clone());
        self.error(error);
    }

    fn reject(&self, command_id: String, reason: impl Into<String>) {
        self.emit(RuntimeEventKind::CommandRejected {
            command_id,
            reason: reason.into(),
        });
    }

    fn error(&self, message: String) {
        self.emit(RuntimeEventKind::Error {
            error: RuntimeErrorView {
                message,
                recoverable: true,
                hint: Some("inspect the lane receipt and retry".to_string()),
            },
        });
    }

    fn emit(&self, kind: RuntimeEventKind) {
        (self.events)(self.owner.clone(), kind);
    }
}

fn lane_approval(
    request_id: &str,
    owner: &RuntimeOwner,
    lane: &AgentLaneRecord,
) -> ApprovalRequestView {
    ApprovalRequestView {
        id: request_id.to_string(),
        tool_name: "lane_start".to_string(),
        title: "Approve lane start".to_string(),
        message: format!("Start lane `{}`", lane.id),
        input_preview: lane.summary.clone(),
        is_mutating: true,
        reason: Some("lane mutation policy requires approval".to_string()),
        owner: owner.clone(),
        risk: viden_types::ApprovalRisk::Medium,
        target: viden_types::ApprovalTarget {
            kind: "lane".to_string(),
            display: lane.id.clone(),
            canonical_ref: lane.worktree.clone(),
        },
        allowed_scopes: vec![viden_types::ApprovalScope::Once],
        policy_reason_key: "lane.requires_approval".to_string(),
        policy_reason_args: Default::default(),
        expires_at: now_timestamp().saturating_add(300),
        default_action: viden_types::ApprovalDefaultAction::Deny,
        audit_id: fresh_id("audit"),
    }
}
