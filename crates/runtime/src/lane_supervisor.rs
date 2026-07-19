use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use viden_types::{
    AgentLaneRecord, ApprovalResponse, RuntimeCommand, RuntimeEventKind, RuntimeOwner, WorkMode,
    fresh_id, now_timestamp,
};
use viden_workflows::{lanes::LaneEvent, stores::WorkflowStore};

use crate::lane_runtime::{LaneEffectExecutor, LaneEffectRequest};
use crate::lane_worker::{LaneEventSink, LaneWorkerHandle, LaneWorkerMessage};
use crate::runtime_contract::redacted_runtime_command_for_event;

pub(crate) struct LaneSupervisor {
    repo: PathBuf,
    workflows: WorkflowStore,
    effects: Arc<dyn LaneEffectExecutor>,
    events: LaneEventSink,
    work_mode: Arc<dyn Fn() -> WorkMode + Send + Sync>,
    lanes: Mutex<BTreeMap<String, LaneWorkerHandle>>,
}

impl LaneSupervisor {
    pub(crate) fn new(
        repo: PathBuf,
        workflows: WorkflowStore,
        effects: Arc<dyn LaneEffectExecutor>,
        events: LaneEventSink,
        work_mode: Arc<dyn Fn() -> WorkMode + Send + Sync>,
    ) -> Self {
        Self {
            repo,
            workflows,
            effects,
            events,
            work_mode,
            lanes: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn handles(command: &RuntimeCommand) -> bool {
        matches!(
            command,
            RuntimeCommand::CreateLane { .. }
                | RuntimeCommand::StartLane { .. }
                | RuntimeCommand::StopLane { .. }
                | RuntimeCommand::AttachLane { .. }
                | RuntimeCommand::DetachLane { .. }
                | RuntimeCommand::SendLaneInput { .. }
                | RuntimeCommand::AcceptLaneOutput { .. }
                | RuntimeCommand::ReviseLaneOutput { .. }
                | RuntimeCommand::DiscardLaneOutput { .. }
                | RuntimeCommand::ApplyLaneChanges { .. }
                | RuntimeCommand::ResolveLaneConflict { .. }
                | RuntimeCommand::ArchiveLane { .. }
                | RuntimeCommand::CleanupLane { .. }
        )
    }

    pub(crate) fn send(
        &self,
        owner: RuntimeOwner,
        command_id: String,
        command: RuntimeCommand,
    ) -> Result<(), String> {
        let lane_id = command_lane_id(&command).to_string();
        if owner.lane_id.as_deref() != Some(lane_id.as_str()) {
            self.reject(
                owner,
                command_id,
                format!("lane `{lane_id}` owner mismatch"),
            );
            return Ok(());
        }
        // This gate deliberately precedes registry lookup, persistence, and effect dispatch.
        if is_effectful(&command) && (self.work_mode)() != WorkMode::Build {
            self.reject(
                owner,
                command_id,
                "Plan mode blocks lane effects".to_string(),
            );
            return Ok(());
        }
        if let RuntimeCommand::CreateLane { lane } = command {
            return self.create(owner, command_id, lane);
        }
        let lanes = self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?;
        let Some(worker) = lanes.get(&lane_id) else {
            self.reject(
                owner,
                command_id,
                format!("lane `{lane_id}` is not registered"),
            );
            return Ok(());
        };
        if worker.owner != owner {
            self.reject(
                owner,
                command_id,
                format!("lane `{lane_id}` owner mismatch"),
            );
            return Ok(());
        }
        self.emit(
            owner.clone(),
            RuntimeEventKind::CommandAccepted {
                command_id: command_id.clone(),
                command: redacted_runtime_command_for_event(&command),
            },
        );
        worker.send(LaneWorkerMessage::Command {
            command_id,
            command,
        })
    }

    pub(crate) fn respond_to_approval(
        &self,
        owner: &RuntimeOwner,
        request_id: &str,
        response: ApprovalResponse,
    ) -> Result<bool, String> {
        let Some(lane_id) = owner.lane_id.as_deref() else {
            return Ok(false);
        };
        let lanes = self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?;
        let Some(worker) = lanes.get(lane_id) else {
            return Ok(false);
        };
        if &worker.owner != owner {
            return Err(format!("lane `{lane_id}` owner mismatch"));
        }
        worker.send(LaneWorkerMessage::ResumeApproval {
            request_id: request_id.to_string(),
            response,
        })?;
        Ok(true)
    }

    pub(crate) fn cancel(&self, owner: &RuntimeOwner, command_id: String) -> Result<bool, String> {
        let Some(lane_id) = owner.lane_id.as_deref() else {
            return Ok(false);
        };
        let lanes = self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?;
        let Some(worker) = lanes.get(lane_id) else {
            return Ok(false);
        };
        if &worker.owner != owner {
            return Err(format!("lane `{lane_id}` owner mismatch"));
        }
        worker.send(LaneWorkerMessage::Cancel { command_id })?;
        Ok(true)
    }

    fn create(
        &self,
        owner: RuntimeOwner,
        command_id: String,
        lane: AgentLaneRecord,
    ) -> Result<(), String> {
        if self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?
            .contains_key(&lane.id)
        {
            self.reject(
                owner,
                command_id,
                format!("lane `{}` is already registered", lane.id),
            );
            return Ok(());
        }
        self.emit(
            owner.clone(),
            RuntimeEventKind::CommandAccepted {
                command_id: command_id.clone(),
                command: redacted_runtime_command_for_event(&RuntimeCommand::CreateLane {
                    lane: lane.clone(),
                }),
            },
        );
        let effect = self.effects.execute(LaneEffectRequest::Create {
            repo: self.repo.clone(),
            lane: lane.clone(),
        });
        let result = match effect {
            Ok(result) => result,
            Err(error) => {
                self.error(owner, error);
                return Ok(());
            }
        };
        let event = LaneEvent::created(
            fresh_id("lane-event"),
            lane.clone(),
            now_timestamp(),
            owner.session_id.clone(),
        );
        if let Err(error) = self.workflows.append_lane_event_checked(&event) {
            self.emit(
                owner.clone(),
                RuntimeEventKind::LaneRecoveryRequired {
                    lane_id: lane.id.clone(),
                    reason: error.clone(),
                    next_action: "remove the orphan worktree or retry lane registration"
                        .to_string(),
                },
            );
            self.error(owner, error);
            return Ok(());
        }
        let worker = LaneWorkerHandle::spawn(
            owner.clone(),
            lane.clone(),
            self.repo.clone(),
            self.workflows.clone(),
            Arc::clone(&self.effects),
            Arc::clone(&self.events),
        );
        self.lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?
            .insert(lane.id.clone(), worker);
        self.emit(
            owner.clone(),
            RuntimeEventKind::LaneOutputAppended {
                lane_id: lane.id.clone(),
                stream: "receipt".to_string(),
                content: result.output,
            },
        );
        self.emit(owner, RuntimeEventKind::LaneUpdated { lane });
        Ok(())
    }

    fn emit(&self, owner: RuntimeOwner, kind: RuntimeEventKind) {
        (self.events)(owner, kind);
    }
    fn reject(&self, owner: RuntimeOwner, command_id: String, reason: String) {
        self.emit(
            owner,
            RuntimeEventKind::CommandRejected { command_id, reason },
        );
    }
    fn error(&self, owner: RuntimeOwner, message: String) {
        self.emit(
            owner,
            RuntimeEventKind::Error {
                error: viden_types::RuntimeErrorView {
                    message,
                    recoverable: true,
                    hint: Some("inspect lane recovery state".to_string()),
                },
            },
        );
    }
}

fn command_lane_id(command: &RuntimeCommand) -> &str {
    match command {
        RuntimeCommand::CreateLane { lane } => &lane.id,
        RuntimeCommand::StartLane { lane_id, .. }
        | RuntimeCommand::StopLane { lane_id }
        | RuntimeCommand::AttachLane { lane_id }
        | RuntimeCommand::DetachLane { lane_id }
        | RuntimeCommand::SendLaneInput { lane_id, .. }
        | RuntimeCommand::AcceptLaneOutput { lane_id, .. }
        | RuntimeCommand::ReviseLaneOutput { lane_id, .. }
        | RuntimeCommand::DiscardLaneOutput { lane_id, .. }
        | RuntimeCommand::ApplyLaneChanges { lane_id, .. }
        | RuntimeCommand::ResolveLaneConflict { lane_id, .. }
        | RuntimeCommand::ArchiveLane { lane_id, .. }
        | RuntimeCommand::CleanupLane { lane_id, .. } => lane_id,
        _ => "",
    }
}

fn is_effectful(command: &RuntimeCommand) -> bool {
    matches!(
        command,
        RuntimeCommand::CreateLane { .. }
            | RuntimeCommand::StartLane { .. }
            | RuntimeCommand::StopLane { .. }
            | RuntimeCommand::SendLaneInput { .. }
            | RuntimeCommand::ApplyLaneChanges { .. }
            | RuntimeCommand::ResolveLaneConflict { .. }
            | RuntimeCommand::CleanupLane { .. }
    )
}
