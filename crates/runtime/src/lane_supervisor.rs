use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use viden_permissions::PermissionEngine;
use viden_types::{
    AgentLaneRecord, ApprovalResponse, RuntimeCommand, RuntimeEventKind, RuntimeOwner, WorkMode,
    fresh_id, now_timestamp,
};
use viden_workflows::{
    lanes::{LaneEvent, LaneEventKind},
    stores::WorkflowStore,
};

use crate::lane_runtime::LaneEffectExecutor;
use crate::lane_worker::{LaneEventSink, LaneTerminalKind, LaneWorkerHandle, LaneWorkerMessage};
use crate::runtime_contract::redacted_runtime_command_for_event;

pub(crate) trait LanePersistence: Send + Sync {
    fn append(&self, event: &LaneEvent) -> Result<(), String>;
    fn load_lanes(&self) -> Result<BTreeMap<String, AgentLaneRecord>, String>;
}

pub(crate) struct WorkflowLanePersistence(pub(crate) WorkflowStore);

impl LanePersistence for WorkflowLanePersistence {
    fn append(&self, event: &LaneEvent) -> Result<(), String> {
        self.0.append_lane_event_checked(event)
    }

    fn load_lanes(&self) -> Result<BTreeMap<String, AgentLaneRecord>, String> {
        let mut lanes = self.0.load_lane_state()?.lanes().clone();
        for event in self.0.load_lane_events()? {
            let Some(origin_session_id) = event.origin_session_id else {
                continue;
            };
            let lane_ids = match event.kind {
                LaneEventKind::LegacyImported { lanes, .. } => {
                    lanes.into_iter().map(|lane| lane.id).collect::<Vec<_>>()
                }
                _ => vec![event.lane_id],
            };
            for lane_id in lane_ids {
                let Some(lane) = lanes.get_mut(&lane_id) else {
                    continue;
                };
                if lane.active_session_ids.is_empty()
                    && !lane.active_session_ids.contains(&origin_session_id)
                {
                    lane.active_session_ids.push(origin_session_id.clone());
                }
            }
        }
        Ok(lanes)
    }
}

pub(crate) struct LaneSupervisor {
    repo: PathBuf,
    persistence: Arc<dyn LanePersistence>,
    permissions: Arc<Mutex<PermissionEngine>>,
    effects: Arc<dyn LaneEffectExecutor>,
    events: LaneEventSink,
    work_mode: Arc<dyn Fn() -> WorkMode + Send + Sync>,
    approval_ttl_secs: u64,
    lanes: Mutex<BTreeMap<String, LaneWorkerHandle>>,
    hydrated_lanes: Mutex<BTreeMap<String, AgentLaneRecord>>,
    terminal_lanes: Mutex<BTreeMap<String, LaneTerminalKind>>,
    hydration_recoveries: Vec<AgentLaneRecord>,
    hydration_error: Option<String>,
}

impl LaneSupervisor {
    pub(crate) fn new(
        repo: PathBuf,
        persistence: Arc<dyn LanePersistence>,
        permissions: Arc<Mutex<PermissionEngine>>,
        effects: Arc<dyn LaneEffectExecutor>,
        events: LaneEventSink,
        work_mode: Arc<dyn Fn() -> WorkMode + Send + Sync>,
        approval_ttl_secs: u64,
    ) -> Self {
        let (mut hydrated_lanes, mut hydration_error) = match persistence.load_lanes() {
            Ok(lanes) => (lanes, None),
            Err(error) => (
                BTreeMap::new(),
                Some(format!("lane hydration failed: {error}")),
            ),
        };
        let mut hydration_recoveries = Vec::new();
        if hydration_error.is_none() {
            for lane in hydrated_lanes.values_mut().filter(|lane| {
                matches!(
                    lane.status,
                    viden_types::LaneStatus::Starting
                        | viden_types::LaneStatus::Running
                        | viden_types::LaneStatus::WaitingApproval
                )
            }) {
                let summary = "lane requires recovery after runtime restart";
                let event = LaneEvent::status_changed(
                    fresh_id("lane-event"),
                    lane.id.clone(),
                    viden_types::LaneStatus::Blocked,
                    summary,
                    now_timestamp(),
                    lane.active_session_ids.first().cloned(),
                );
                if let Err(error) = persistence.append(&event) {
                    hydration_error = Some(format!(
                        "lane hydration recovery failed for `{}`: {error}",
                        lane.id
                    ));
                    break;
                }
                lane.status = viden_types::LaneStatus::Blocked;
                lane.summary = summary.to_string();
                hydration_recoveries.push(lane.clone());
            }
        }
        let terminal_lanes = hydrated_lanes
            .iter()
            .filter(|(_, lane)| lane.status == viden_types::LaneStatus::Archived)
            .map(|(lane_id, lane)| {
                let kind = if lane.summary == "lane cleaned up" {
                    LaneTerminalKind::Cleaned
                } else {
                    LaneTerminalKind::Archived
                };
                (lane_id.clone(), kind)
            })
            .collect();
        Self {
            repo,
            persistence,
            permissions,
            effects,
            events,
            work_mode,
            approval_ttl_secs,
            lanes: Mutex::new(BTreeMap::new()),
            hydrated_lanes: Mutex::new(hydrated_lanes),
            terminal_lanes: Mutex::new(terminal_lanes),
            hydration_recoveries,
            hydration_error,
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

    pub(crate) fn hydration_recoveries(&self) -> &[AgentLaneRecord] {
        &self.hydration_recoveries
    }

    pub(crate) fn sync_permissions(&self, permissions: PermissionEngine) -> Result<(), String> {
        *self
            .permissions
            .lock()
            .map_err(|_| "lane permission registry poisoned".to_string())? = permissions;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn worker_finished_for_test(&self, lane_id: &str) -> bool {
        self.lanes
            .lock()
            .ok()
            .and_then(|lanes| lanes.get(lane_id).map(LaneWorkerHandle::is_finished))
            .unwrap_or(true)
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
        if Self::handles(&command) && (self.work_mode)() != WorkMode::Build {
            self.reject(
                owner,
                command_id,
                "Plan mode blocks lane mutations".to_string(),
            );
            return Ok(());
        }
        if let Some(error) = &self.hydration_error {
            self.reject(owner.clone(), command_id, error.clone());
            self.error(owner, error.clone());
            return Ok(());
        }
        let mut lanes = self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?;
        self.reap_terminal_lanes(&mut lanes)?;
        if let RuntimeCommand::CreateLane { lane } = command {
            drop(lanes);
            return self.create(owner, command_id, lane);
        }
        let terminal = self
            .terminal_lanes
            .lock()
            .map_err(|_| "terminal lane registry poisoned".to_string())?
            .get(&lane_id)
            .copied();
        if let Some(terminal) = terminal {
            let archive_to_cleanup = terminal == LaneTerminalKind::Archived
                && matches!(command, RuntimeCommand::CleanupLane { .. });
            if archive_to_cleanup {
                self.terminal_lanes
                    .lock()
                    .map_err(|_| "terminal lane registry poisoned".to_string())?
                    .remove(&lane_id);
            } else {
                self.reject(
                    owner,
                    command_id,
                    format!("lane `{lane_id}` already reached a durable terminal state"),
                );
                return Ok(());
            }
        }
        if !lanes.contains_key(&lane_id) {
            let hydrated = self
                .hydrated_lanes
                .lock()
                .map_err(|_| "hydrated lane registry poisoned".to_string())?
                .get(&lane_id)
                .cloned();
            if let Some(lane) = hydrated {
                if !lane.active_session_ids.is_empty()
                    && !owner
                        .session_id
                        .as_ref()
                        .is_some_and(|session_id| lane.active_session_ids.contains(session_id))
                {
                    self.reject(
                        owner,
                        command_id,
                        format!("lane `{lane_id}` owner mismatch"),
                    );
                    return Ok(());
                }
                lanes.insert(
                    lane_id.clone(),
                    LaneWorkerHandle::spawn(
                        owner.clone(),
                        lane,
                        self.repo.clone(),
                        Arc::clone(&self.persistence),
                        Arc::clone(&self.permissions),
                        Arc::clone(&self.effects),
                        Arc::clone(&self.events),
                        self.approval_ttl_secs,
                        true,
                    ),
                );
            }
        }
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
            RuntimeEventKind::LaneCommandAccepted {
                command_id: command_id.clone(),
                command: redacted_runtime_command_for_event(&command),
            },
        );
        worker.send(LaneWorkerMessage::Command {
            command_id,
            command: Box::new(command),
        })?;
        Ok(())
    }

    pub(crate) fn respond_to_approval(
        &self,
        owner: &RuntimeOwner,
        command_id: &str,
        request_id: &str,
        response: ApprovalResponse,
    ) -> Result<Option<bool>, String> {
        let lanes = self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?;
        let Some((lane_id, worker)) = lanes
            .iter()
            .find(|(_, worker)| worker.owns_pending_approval(request_id))
        else {
            return Ok(None);
        };
        if &worker.owner != owner {
            self.reject(
                owner.clone(),
                command_id.to_string(),
                format!("approval request `{request_id}` owner mismatch for lane `{lane_id}`"),
            );
            return Ok(Some(false));
        }
        worker.send(LaneWorkerMessage::ResumeApproval {
            request_id: request_id.to_string(),
            response,
        })?;
        Ok(Some(true))
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
        mut lane: AgentLaneRecord,
    ) -> Result<(), String> {
        if let Some(error) = &self.hydration_error {
            self.reject(owner.clone(), command_id, error.clone());
            self.error(owner, error.clone());
            return Ok(());
        }
        if let Some(session_id) = owner.session_id.clone()
            && !lane.active_session_ids.contains(&session_id)
        {
            lane.active_session_ids.push(session_id);
        }
        let mut lanes = self
            .lanes
            .lock()
            .map_err(|_| "lane registry poisoned".to_string())?;
        if let Some(worker) = lanes.get(&lane.id) {
            if worker.owner == owner && !worker.is_registered() {
                self.emit(
                    owner.clone(),
                    RuntimeEventKind::LaneCommandAccepted {
                        command_id: command_id.clone(),
                        command: redacted_runtime_command_for_event(&RuntimeCommand::CreateLane {
                            lane: lane.clone(),
                        }),
                    },
                );
                return worker.send(LaneWorkerMessage::Command {
                    command_id,
                    command: Box::new(RuntimeCommand::CreateLane { lane }),
                });
            }
            self.reject(
                owner,
                command_id,
                format!("lane `{}` is already registered", lane.id),
            );
            return Ok(());
        }
        if self
            .hydrated_lanes
            .lock()
            .map_err(|_| "hydrated lane registry poisoned".to_string())?
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
            RuntimeEventKind::LaneCommandAccepted {
                command_id: command_id.clone(),
                command: redacted_runtime_command_for_event(&RuntimeCommand::CreateLane {
                    lane: lane.clone(),
                }),
            },
        );
        let worker = LaneWorkerHandle::spawn(
            owner.clone(),
            lane.clone(),
            self.repo.clone(),
            Arc::clone(&self.persistence),
            Arc::clone(&self.permissions),
            Arc::clone(&self.effects),
            Arc::clone(&self.events),
            self.approval_ttl_secs,
            false,
        );
        worker.send(LaneWorkerMessage::Command {
            command_id,
            command: Box::new(RuntimeCommand::CreateLane { lane: lane.clone() }),
        })?;
        lanes.insert(lane.id, worker);
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

    fn reap_terminal_lanes(
        &self,
        lanes: &mut BTreeMap<String, LaneWorkerHandle>,
    ) -> Result<(), String> {
        let completions = lanes
            .iter()
            .filter_map(|(lane_id, worker)| {
                worker
                    .take_terminal_completion()
                    .map(|completion| (lane_id.clone(), completion))
            })
            .collect::<Vec<_>>();
        if completions.is_empty() {
            return Ok(());
        }
        let mut hydrated = self
            .hydrated_lanes
            .lock()
            .map_err(|_| "hydrated lane registry poisoned".to_string())?;
        let mut terminal = self
            .terminal_lanes
            .lock()
            .map_err(|_| "terminal lane registry poisoned".to_string())?;
        for (lane_id, completion) in completions {
            hydrated.insert(lane_id.clone(), completion.lane);
            terminal.insert(lane_id.clone(), completion.kind);
            lanes.remove(&lane_id);
        }
        Ok(())
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
