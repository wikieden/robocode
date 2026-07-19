use std::sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
};
use std::thread::{self, JoinHandle};

use viden_permissions::PermissionEngine;
use viden_types::{
    AgentLaneRecord, ApprovalDecision, ApprovalRequestView, ApprovalResponse, ApprovalScope,
    LaneStatus, PermissionAskDecision, PermissionDecision, RuntimeCommand, RuntimeErrorView,
    RuntimeEventKind, RuntimeOwner, ToolInput, ToolSpec, fresh_id, now_timestamp,
};
use viden_workflows::lanes::LaneEvent;

use crate::lane_runtime::{LaneEffectExecutor, LaneEffectRequest};
use crate::lane_supervisor::LanePersistence;

pub(crate) type LaneEventSink = Arc<dyn Fn(RuntimeOwner, RuntimeEventKind) + Send + Sync>;

pub(crate) enum LaneWorkerMessage {
    Command {
        command_id: String,
        command: Box<RuntimeCommand>,
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
    worker: Option<JoinHandle<()>>,
}

impl LaneWorkerHandle {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn(
        owner: RuntimeOwner,
        lane: AgentLaneRecord,
        repo: std::path::PathBuf,
        persistence: Arc<dyn LanePersistence>,
        permissions: Arc<Mutex<PermissionEngine>>,
        effects: Arc<dyn LaneEffectExecutor>,
        events: LaneEventSink,
        approval_ttl_secs: u64,
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
                persistence,
                permissions,
                effects,
                events,
                pending_mutation: None,
                pending_approval: worker_pending_approval,
                approval_ttl_secs,
                runtime_active: false,
            };
            while let Ok(message) = receiver.recv() {
                if matches!(message, LaneWorkerMessage::Shutdown) {
                    break;
                }
                runtime.handle(message);
            }
            if runtime.runtime_active {
                let _ = runtime.effects.shutdown_lane(&runtime.lane.id);
            }
        });
        Self {
            owner,
            pending_approval,
            sender,
            worker: Some(worker),
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
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct PendingMutation {
    request_id: String,
    command_id: String,
    audit_id: String,
    expires_at: u64,
    allowed_scopes: Vec<ApprovalScope>,
    permission_ask: Option<PermissionAskDecision>,
    tool: ToolSpec,
    input: ToolInput,
    previous_status: LaneStatus,
    operation: PendingOperation,
}

enum PendingOperation {
    Start(LaneEffectRequest),
    Apply(LaneEffectRequest),
}

struct LaneWorker {
    owner: RuntimeOwner,
    lane: AgentLaneRecord,
    repo: std::path::PathBuf,
    persistence: Arc<dyn LanePersistence>,
    permissions: Arc<Mutex<PermissionEngine>>,
    effects: Arc<dyn LaneEffectExecutor>,
    events: LaneEventSink,
    pending_mutation: Option<PendingMutation>,
    pending_approval: Arc<Mutex<Option<String>>>,
    approval_ttl_secs: u64,
    runtime_active: bool,
}

impl LaneWorker {
    fn handle(&mut self, message: LaneWorkerMessage) {
        match message {
            LaneWorkerMessage::Command {
                command_id,
                command,
            } => self.handle_command(command_id, *command),
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
                if !matches!(
                    self.lane.status,
                    LaneStatus::Draft | LaneStatus::Queued | LaneStatus::Detached
                ) {
                    self.reject(
                        command_id,
                        format!("lane cannot start while status is {:?}", self.lane.status),
                    );
                    return;
                }
                self.dispatch_mutation(command_id, "lane_start", PendingOperation::Start(request));
            }
            RuntimeCommand::StopLane { .. } => {
                if let Err(error) = self.effects.shutdown_lane(&self.lane.id) {
                    self.fail(error);
                } else {
                    self.runtime_active = false;
                    let _ = self.change_status(LaneStatus::Detached, "lane stopped");
                }
            }
            RuntimeCommand::AttachLane { .. } => {
                let _ = self.change_status(LaneStatus::Attached, "lane attached");
            }
            RuntimeCommand::DetachLane { .. } => {
                let _ = self.change_status(LaneStatus::Detached, "lane detached");
            }
            RuntimeCommand::SendLaneInput { input, .. } => {
                let input_id = fresh_id("lane-input");
                self.emit(RuntimeEventKind::InputQueued {
                    input: viden_types::QueuedInputView {
                        id: input_id.clone(),
                        content_preview: input.chars().take(160).collect(),
                        created_at: Some(now_timestamp()),
                    },
                });
                let result = self.effects.execute(LaneEffectRequest::SendInput {
                    lane_id: self.lane.id.clone(),
                    input,
                });
                self.emit(RuntimeEventKind::InputDequeued { input_id });
                match result {
                    Ok(result) => {
                        self.output("receipt", result.output);
                        let _ = self.change_status(self.lane.status, "lane input delivered");
                    }
                    Err(error) => self.fail(error),
                }
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
            RuntimeCommand::ApplyLaneChanges { unified_diff, .. } => {
                if self.lane.status != LaneStatus::Done {
                    self.reject(
                        command_id,
                        format!(
                            "lane changes require done status, found {:?}",
                            self.lane.status
                        ),
                    );
                    return;
                }
                self.dispatch_mutation(
                    command_id,
                    "lane_apply",
                    PendingOperation::Apply(LaneEffectRequest::Apply {
                        cwd: self.repo.clone(),
                        unified_diff,
                    }),
                );
            }
            RuntimeCommand::ResolveLaneConflict { unified_diff, .. } => {
                if self.lane.status != LaneStatus::Blocked {
                    self.reject(
                        command_id,
                        format!(
                            "lane conflict resolution requires blocked status, found {:?}",
                            self.lane.status
                        ),
                    );
                    return;
                }
                self.dispatch_mutation(
                    command_id,
                    "lane_resolve_conflict",
                    PendingOperation::Apply(LaneEffectRequest::Apply {
                        cwd: self.repo.clone(),
                        unified_diff,
                    }),
                );
            }
            RuntimeCommand::ArchiveLane { summary, .. } => {
                if let Err(error) = self.effects.shutdown_lane(&self.lane.id) {
                    self.fail(error);
                    return;
                }
                self.runtime_active = false;
                let _ = self.change_status(LaneStatus::Archived, summary);
            }
            RuntimeCommand::CleanupLane { force, .. } => {
                // Persist cleanup intent before the irreversible worktree removal. If the
                // completion append fails, replay leaves the lane in Starting with this
                // recovery summary instead of claiming that removed bytes were restored.
                if self
                    .change_status(LaneStatus::Starting, "lane cleanup intent persisted")
                    .is_err()
                {
                    return;
                }
                if let Err(error) = self.effects.shutdown_lane(&self.lane.id) {
                    self.fail(error);
                    return;
                }
                self.runtime_active = false;
                match self.effects.execute(LaneEffectRequest::Cleanup {
                    repo: self.repo.clone(),
                    lane: self.lane.clone(),
                    force,
                }) {
                    Ok(result) => {
                        self.output("receipt", result.output);
                        if self
                            .change_status(LaneStatus::Archived, "lane cleaned up")
                            .is_err()
                        {
                            self.emit(RuntimeEventKind::LaneRecoveryRequired {
                                lane_id: self.lane.id.clone(),
                                reason: "cleanup completed but durable completion append failed"
                                    .to_string(),
                                next_action: "replay cleanup intent and reconcile the worktree"
                                    .to_string(),
                            });
                        }
                    }
                    Err(error) => self.fail(error),
                }
            }
            _ => self.reject(command_id, "command is not a lane lifecycle command"),
        }
    }

    fn dispatch_mutation(
        &mut self,
        command_id: String,
        tool_name: &str,
        operation: PendingOperation,
    ) {
        if self.lane.mutation_policy == viden_types::MutationPolicy::ReadOnly {
            self.reject(
                command_id,
                format!("read-only lane cannot execute `{tool_name}`"),
            );
            return;
        }
        let tool = ToolSpec {
            name: tool_name.to_string(),
            description: format!("Core-owned mutation for lane `{}`", self.lane.id),
            is_mutating: true,
            input_schema_hint: "path=<canonical lane scope>".to_string(),
        };
        let mut input = ToolInput::new();
        input.insert("path".to_string(), self.repo.to_string_lossy().to_string());
        let permission = match self.permissions.lock() {
            Ok(permissions) => permissions.decide(&tool, &input),
            Err(_) => {
                self.reject(command_id, "lane permission registry poisoned");
                return;
            }
        };
        match permission {
            PermissionDecision::Deny(denial) => self.reject(command_id, denial.message),
            PermissionDecision::Allow(_)
                if self.lane.mutation_policy == viden_types::MutationPolicy::Autonomous =>
            {
                self.execute_pending_operation(operation);
            }
            PermissionDecision::Allow(_) => {
                self.queue_mutation_approval(command_id, tool, input, None, operation)
            }
            PermissionDecision::Ask(ask) => {
                self.queue_mutation_approval(command_id, tool, input, Some(ask), operation)
            }
        }
    }

    fn queue_mutation_approval(
        &mut self,
        command_id: String,
        tool: ToolSpec,
        input: ToolInput,
        permission_ask: Option<PermissionAskDecision>,
        operation: PendingOperation,
    ) {
        let previous_status = self.lane.status;
        let request_id = fresh_id("lane-approval");
        let audit_id = fresh_id("audit");
        let expires_at = now_timestamp().saturating_add(self.approval_ttl_secs);
        let mut allowed_scopes = vec![ApprovalScope::Once];
        if let Some(session_id) = self.owner.session_id.clone() {
            allowed_scopes.push(ApprovalScope::Session { session_id });
        }
        allowed_scopes.push(ApprovalScope::RepoAllowlist {
            paths: vec![self.repo.to_string_lossy().to_string()],
        });
        if self
            .change_status(LaneStatus::WaitingApproval, "lane mutation awaits approval")
            .is_err()
        {
            return;
        }
        let approval = lane_approval(
            &request_id,
            &audit_id,
            expires_at,
            allowed_scopes.clone(),
            &self.owner,
            &self.lane,
            &tool,
            &input,
        );
        self.pending_mutation = Some(PendingMutation {
            request_id: request_id.clone(),
            command_id,
            audit_id,
            expires_at,
            allowed_scopes,
            permission_ask,
            tool,
            input,
            previous_status,
            operation,
        });
        if let Ok(mut pending) = self.pending_approval.lock() {
            *pending = Some(request_id);
        }
        self.emit(RuntimeEventKind::ApprovalRequested { approval });
    }

    fn execute_pending_operation(&mut self, operation: PendingOperation) {
        match operation {
            PendingOperation::Start(request) => self.start(request),
            PendingOperation::Apply(request) => self.apply(request),
        }
    }

    fn resume_approval(&mut self, request_id: String, response: ApprovalResponse) {
        let Some(pending) = self.pending_mutation.take() else {
            return;
        };
        if pending.request_id != request_id {
            self.pending_mutation = Some(pending);
            return;
        }
        if let Ok(mut approval) = self.pending_approval.lock() {
            *approval = None;
        }
        let valid_scope = match &response.decision {
            ApprovalDecision::Deny => true,
            ApprovalDecision::Allow { scope } => pending.allowed_scopes.contains(scope),
        };
        let unexpired = now_timestamp() < pending.expires_at;
        let permission_allowed = if valid_scope && unexpired && response.is_allowed() {
            if let Some(ask) = &pending.permission_ask {
                self.permissions
                    .lock()
                    .map(|mut permissions| {
                        matches!(
                            permissions.apply_approval(
                                response.clone(),
                                ask,
                                &pending.tool,
                                &pending.input,
                            ),
                            PermissionDecision::Allow(_)
                        )
                    })
                    .unwrap_or(false)
            } else {
                true
            }
        } else {
            false
        };
        let decision = if permission_allowed {
            response.decision.clone()
        } else {
            ApprovalDecision::Deny
        };
        self.emit(RuntimeEventKind::ApprovalResolved {
            request_id,
            decision,
            owner: self.owner.clone(),
            audit_id: pending.audit_id.clone(),
        });
        if permission_allowed {
            self.execute_pending_operation(pending.operation);
        } else {
            let _ = self.change_status(pending.previous_status, "lane mutation approval denied");
            let reason = if !unexpired {
                "lane mutation approval expired"
            } else if !valid_scope {
                "lane mutation approval scope is not allowed"
            } else {
                "lane mutation approval denied"
            };
            self.reject(pending.command_id, reason);
        }
    }

    fn cancel(&mut self, command_id: String) {
        if let Some(pending) = self.pending_mutation.take() {
            if let Ok(mut approval) = self.pending_approval.lock() {
                *approval = None;
            }
            self.emit(RuntimeEventKind::ApprovalResolved {
                request_id: pending.request_id,
                decision: ApprovalDecision::Deny,
                owner: self.owner.clone(),
                audit_id: pending.audit_id,
            });
            let _ = self.change_status(LaneStatus::Cancelled, "lane mutation cancelled");
        } else {
            if let Err(error) = self.effects.shutdown_lane(&self.lane.id) {
                self.fail(error);
            } else {
                self.runtime_active = false;
                let _ = self.change_status(LaneStatus::Cancelled, "lane cancelled");
            }
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
        match self.effects.execute(request) {
            Ok(result) => {
                self.runtime_active = true;
                self.output("receipt", result.output);
                if self
                    .change_status(LaneStatus::Running, "lane running")
                    .is_err()
                {
                    let _ = self.effects.shutdown_lane(&self.lane.id);
                    self.runtime_active = false;
                }
            }
            Err(error) => self.fail(error),
        }
    }

    fn apply(&mut self, request: LaneEffectRequest) {
        let event = LaneEvent::status_changed(
            fresh_id("lane-event"),
            self.lane.id.clone(),
            LaneStatus::Done,
            "lane changes applied",
            now_timestamp(),
            self.owner.session_id.clone(),
        );
        let persistence = Arc::clone(&self.persistence);
        let mut persist = || persistence.append(&event);
        match self.effects.apply_transactionally(request, &mut persist) {
            Ok(result) if result.conflict_paths.is_empty() => {
                self.output("receipt", result.output);
                self.lane.status = LaneStatus::Done;
                self.lane.summary = "lane changes applied".to_string();
                self.emit(RuntimeEventKind::LaneUpdated {
                    lane: self.lane.clone(),
                });
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
        if let Err(error) = self.persistence.append(&event) {
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

#[allow(clippy::too_many_arguments)]
fn lane_approval(
    request_id: &str,
    audit_id: &str,
    expires_at: u64,
    allowed_scopes: Vec<ApprovalScope>,
    owner: &RuntimeOwner,
    lane: &AgentLaneRecord,
    tool: &ToolSpec,
    input: &ToolInput,
) -> ApprovalRequestView {
    ApprovalRequestView {
        id: request_id.to_string(),
        tool_name: tool.name.clone(),
        title: format!("Approve {}", tool.name),
        message: format!("Approve `{}` for lane `{}`", tool.name, lane.id),
        input_preview: input
            .iter()
            .map(|(key, value)| format!("{key}: {value}"))
            .collect::<Vec<_>>()
            .join("\n"),
        is_mutating: true,
        reason: Some("lane mutation policy requires approval".to_string()),
        owner: owner.clone(),
        risk: viden_types::ApprovalRisk::Medium,
        target: viden_types::ApprovalTarget {
            kind: "lane".to_string(),
            display: lane.id.clone(),
            canonical_ref: lane.worktree.clone(),
        },
        allowed_scopes,
        policy_reason_key: "lane.requires_approval".to_string(),
        policy_reason_args: Default::default(),
        expires_at,
        default_action: viden_types::ApprovalDefaultAction::Deny,
        audit_id: audit_id.to_string(),
    }
}
