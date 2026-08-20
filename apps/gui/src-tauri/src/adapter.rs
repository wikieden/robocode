use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_core::{
    AgentSessionInput, AgentSessionRequest, AgentSessionStatus, ApprovalDecision,
    ApprovalRequestView, ApprovalResponse, ApprovalScope, BoundCoreClient, CoreClient,
    CoreClientError, CoreHandshake, FRONTEND_SCHEMA_V1, LocalCoreHost, ReplayBatch, ReplayRequest,
    RuntimeCommand, RuntimeCommandEnvelope, RuntimeEventEnvelope, RuntimeEventKind, RuntimeOwner,
    RuntimeSnapshotEnvelope, RuntimeWireEvent, StarterLanePreset, StarterLaneRequest,
    TranscriptPage, TranscriptPageRequest, WorkMode, WorkspaceOpenRequest,
};

use crate::d1::{
    D1_OWNER_CAPABILITY, D1CockpitProjection, D1Intent, D1IntentResult, D1OutcomeProjection,
};
use crate::d4::{D4OutcomeProjection, PendingD4, ReviewedD4};
use crate::projection::exact_terminal_agent_session;
use crate::{
    D6ConnectionState, D6RecoveryProjection, D6State, D11IntakeProjection, PermissionChoice,
    PermissionDockProjection, PermissionIntent, PermissionIntentResult,
    PermissionOutcomeProjection, RuntimeProjection,
};

/// The sole command and state boundary between the desktop shell and Core.
pub struct GuiCoreAdapter {
    pub(crate) client: Box<dyn CoreClient>,
    pub(crate) projection: RuntimeProjection,
    capabilities: BTreeSet<String>,
    pending_d11: Option<PendingD11>,
    pub(crate) pending_d4: Option<PendingD4>,
    pub(crate) reviewed_d4: Option<ReviewedD4>,
    pub(crate) d4_receipt: Option<viden_core::StarterLaneReceipt>,
    pub(crate) d4_outcome: D4OutcomeProjection,
    pending_d1: Option<PendingD1>,
    d1_outcome: D1OutcomeProjection,
    pending_permission: Option<PendingPermission>,
    permission_outcome: PermissionOutcomeProjection,
    connection: D6ConnectionState,
    connection_detail: Option<String>,
    /// D2 queue selection. Presentation state only: it never gates a Core
    /// command and is dropped when Core stops publishing the decision.
    d2_selected: Option<String>,
}

struct HostedCoreClient {
    bound: BoundCoreClient,
    handshake: Option<CoreHandshake>,
    confirmed: Option<RuntimeSnapshotEnvelope>,
    confirmed_dirty: bool,
}

impl CoreClient for HostedCoreClient {
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
        let handshake = self.bound.client().discover()?;
        self.handshake = Some(handshake.clone());
        Ok(handshake)
    }

    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.bound.client().send(command)
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        let event = self.bound.client().recv(timeout)?;
        if event.is_some() {
            self.cache_confirmed_state();
            self.confirmed_dirty = true;
        }
        Ok(event)
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        if self.confirmed_dirty {
            self.confirmed_dirty = false;
            return self
                .confirmed
                .clone()
                .ok_or(CoreClientError::MissingSnapshot);
        }
        let snapshot = self.bound.client().snapshot()?;
        self.confirmed = Some(snapshot.clone());
        Ok(snapshot)
    }

    fn replay(&mut self, request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
        self.bound.client().replay(request)
    }

    fn transcript_page(
        &mut self,
        request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError> {
        self.bound.client().transcript_page(request)
    }
}

impl HostedCoreClient {
    fn new(bound: BoundCoreClient) -> Self {
        Self {
            bound,
            handshake: None,
            confirmed: None,
            confirmed_dirty: false,
        }
    }

    fn cache_confirmed_state(&mut self) {
        let Some(handshake) = &self.handshake else {
            return;
        };
        let client = self.bound.client();
        let (Some(view), Some(cursor)) = (client.confirmed_view(), client.confirmed_cursor())
        else {
            return;
        };
        self.confirmed = Some(RuntimeSnapshotEnvelope {
            schema_version: handshake.active_schema_version,
            capabilities: handshake.capabilities.clone(),
            cursor: cursor.clone(),
            snapshot: view.snapshot.clone(),
            view: view.clone(),
        });
    }
}

pub fn open_local_workspace(root: impl AsRef<Path>) -> Result<GuiCoreAdapter, String> {
    let host = LocalCoreHost::new();
    open_workspace_with_host(&host, root)
}

fn open_workspace_with_host(
    host: &LocalCoreHost,
    root: impl AsRef<Path>,
) -> Result<GuiCoreAdapter, String> {
    let bound = host
        .open_workspace(WorkspaceOpenRequest::new(root.as_ref()))
        .map_err(|error| error.to_string())?;
    let mut adapter = GuiCoreAdapter::new(Box::new(HostedCoreClient::new(bound)));
    adapter.connect().map_err(|error| error.to_string())?;
    // D1 needs Core's workspace-mode decision before it can enable New Lane.
    // A project probe is read-only and publishes whether Core will use Git
    // isolation or the opened directory without routing into D11.
    adapter.send_d11_intent_and_wait(
        "gui-open-workspace-probe",
        D11Intent::ProbeProject,
        Duration::from_millis(250),
    )?;
    // ProjectProbed completes the D11 request before the ordered eligibility
    // event that follows it, so drain the remaining facts into the projection.
    adapter.poll_d11(Duration::from_millis(250))?;
    Ok(adapter)
}

pub(crate) fn default_local_adapter() -> Result<Option<GuiCoreAdapter>, String> {
    let Some(root) = default_workspace_root() else {
        return Ok(None);
    };
    open_local_workspace(root).map(Some)
}

fn default_workspace_root() -> Option<PathBuf> {
    // A desktop launch does not imply that its process or bundle directory is
    // the user's project. Only an explicit launch binding may bypass Welcome.
    std::env::var_os("VIDEN_GUI_WORKSPACE").map(PathBuf::from)
}

struct PendingPermission {
    command_id: String,
    request_id: String,
    owner: RuntimeOwner,
    audit_id: String,
}

enum PermissionObservation {
    Continue,
    Confirmed,
    Rejected(String),
}

impl PendingPermission {
    fn observe(&self, envelope: &RuntimeEventEnvelope) -> PermissionObservation {
        let RuntimeWireEvent::Known(event) = &envelope.event else {
            return PermissionObservation::Continue;
        };
        match &event.kind {
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == &self.command_id =>
            {
                PermissionObservation::Rejected(reason.clone())
            }
            RuntimeEventKind::ApprovalResolved {
                request_id,
                owner,
                audit_id,
                ..
            } if request_id == &self.request_id
                && owner == &self.owner
                && audit_id == &self.audit_id =>
            {
                PermissionObservation::Confirmed
            }
            _ => PermissionObservation::Continue,
        }
    }
}

#[derive(Clone)]
enum PendingD1Kind {
    Submit,
    Queue,
    Cancel,
    QueryAgentAdapters,
    ProbeAgentAdapter {
        agent_id: String,
    },
    PreviewDefaultLane,
    CreateStarterLane {
        preview_id: String,
        content_sha256: String,
    },
    StartAgentSession {
        lane_id: String,
        agent_id: String,
        matching_sessions_before: usize,
    },
    SendAgentSessionInput {
        session_id: String,
    },
    RetryAgentSession {
        session_id: String,
    },
    CancelAgentSession {
        session_id: String,
    },
}

struct PendingD1 {
    command_id: String,
    owner: RuntimeOwner,
    kind: PendingD1Kind,
    accepted: bool,
}

enum D1Observation {
    Continue,
    Confirmed,
    Rejected(String),
}

impl PendingD1 {
    fn is_satisfied_by_agent_sessions(&self, sessions: &[viden_core::AgentSessionView]) -> bool {
        match &self.kind {
            PendingD1Kind::StartAgentSession {
                lane_id,
                agent_id,
                matching_sessions_before,
            } => {
                sessions
                    .iter()
                    .filter(|session| session.lane_id == *lane_id && session.agent_id == *agent_id)
                    .count()
                    > *matching_sessions_before
            }
            _ => false,
        }
    }

    fn observe(&mut self, envelope: &RuntimeEventEnvelope) -> D1Observation {
        let RuntimeWireEvent::Known(event) = &envelope.event else {
            return D1Observation::Continue;
        };
        match &event.kind {
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == &self.command_id =>
            {
                D1Observation::Rejected(reason.clone())
            }
            RuntimeEventKind::CommandAccepted {
                command_id,
                command,
            }
            | RuntimeEventKind::LaneCommandAccepted {
                command_id,
                command,
            } if command_id == &self.command_id
                && self.acceptance_owner_matches(envelope, command)
                && self.matches_command(command) =>
            {
                if matches!(self.kind, PendingD1Kind::PreviewDefaultLane)
                    && matches!(command, RuntimeCommand::PreviewStarterLane { .. })
                {
                    // Core expands the default request into a concrete Lane owner;
                    // subsequent business facts must match that authoritative owner.
                    self.owner = envelope.owner.clone();
                }
                self.accepted = true;
                D1Observation::Continue
            }
            kind if self.accepted
                && self.business_owner_matches(envelope)
                && self.matches_business_fact(kind) =>
            {
                D1Observation::Confirmed
            }
            _ => D1Observation::Continue,
        }
    }

    fn matches_command(&self, command: &RuntimeCommand) -> bool {
        matches!(
            (&self.kind, command),
            (
                PendingD1Kind::Submit,
                RuntimeCommand::SubmitUserInput { .. }
            ) | (PendingD1Kind::Queue, RuntimeCommand::QueueFollowUp { .. })
                | (PendingD1Kind::Cancel, RuntimeCommand::CancelActiveTurn)
                | (
                    PendingD1Kind::QueryAgentAdapters,
                    RuntimeCommand::QueryAgentAdapters
                )
                | (
                    PendingD1Kind::ProbeAgentAdapter { .. },
                    RuntimeCommand::ProbeAgentAdapter { .. }
                )
                | (
                    PendingD1Kind::PreviewDefaultLane,
                    RuntimeCommand::PreviewDefaultStarterLane { .. }
                        | RuntimeCommand::PreviewStarterLane { .. }
                )
                | (
                    PendingD1Kind::CreateStarterLane { .. },
                    RuntimeCommand::CreateStarterLane { .. }
                )
                | (
                    PendingD1Kind::StartAgentSession { .. },
                    RuntimeCommand::StartAgentSession { .. }
                )
                | (
                    PendingD1Kind::SendAgentSessionInput { .. },
                    RuntimeCommand::SendAgentSessionInput { .. }
                )
                | (
                    PendingD1Kind::RetryAgentSession { .. },
                    RuntimeCommand::RetryAgentSession { .. }
                )
                | (
                    PendingD1Kind::CancelAgentSession { .. },
                    RuntimeCommand::CancelAgentSession { .. }
                )
        )
    }

    fn acceptance_owner_matches(
        &self,
        envelope: &RuntimeEventEnvelope,
        command: &RuntimeCommand,
    ) -> bool {
        match (&self.kind, command) {
            (PendingD1Kind::PreviewDefaultLane, RuntimeCommand::PreviewStarterLane { request }) => {
                envelope.owner.lane_id.as_deref() == Some(request.lane_id.as_str())
            }
            _ => envelope.owner == self.owner,
        }
    }

    fn business_owner_matches(&self, envelope: &RuntimeEventEnvelope) -> bool {
        match self.kind {
            PendingD1Kind::Submit
            | PendingD1Kind::Queue
            | PendingD1Kind::Cancel
            | PendingD1Kind::PreviewDefaultLane => envelope.owner == self.owner,
            _ => true,
        }
    }

    fn matches_business_fact(&self, kind: &RuntimeEventKind) -> bool {
        match (&self.kind, kind) {
            (PendingD1Kind::Queue, RuntimeEventKind::InputQueued { .. }) => true,
            (
                PendingD1Kind::Submit,
                RuntimeEventKind::AssistantDelta { .. }
                | RuntimeEventKind::ToolCallStarted { .. }
                | RuntimeEventKind::TaskUpdated { .. },
            ) => true,
            (PendingD1Kind::Cancel, RuntimeEventKind::LaneUpdated { lane }) => {
                self.owner.lane_id.as_deref() == Some(lane.id.as_str()) && !lane.is_active()
            }
            (PendingD1Kind::QueryAgentAdapters, RuntimeEventKind::AgentAdaptersLoaded { .. }) => {
                true
            }
            (
                PendingD1Kind::ProbeAgentAdapter { agent_id },
                RuntimeEventKind::AgentAdapterProbed { adapter },
            ) => adapter.agent_id == *agent_id,
            (PendingD1Kind::PreviewDefaultLane, RuntimeEventKind::StarterLanePreviewed { .. }) => {
                true
            }
            (
                PendingD1Kind::CreateStarterLane {
                    preview_id,
                    content_sha256,
                },
                RuntimeEventKind::StarterLaneCreated { receipt },
            ) => receipt.preview_id == *preview_id && receipt.content_sha256 == *content_sha256,
            (
                PendingD1Kind::StartAgentSession {
                    lane_id, agent_id, ..
                },
                RuntimeEventKind::AgentSessionStarted { session },
            ) => session.lane_id == *lane_id && session.agent_id == *agent_id,
            (
                PendingD1Kind::SendAgentSessionInput { session_id },
                RuntimeEventKind::AgentSessionInputAccepted {
                    session_id: accepted,
                    ..
                },
            ) => accepted == session_id,
            (
                PendingD1Kind::RetryAgentSession { session_id },
                RuntimeEventKind::AgentSessionStarted { session },
            ) => session.session_id == *session_id,
            (
                PendingD1Kind::CancelAgentSession { session_id },
                RuntimeEventKind::AgentSessionUpdated { session }
                | RuntimeEventKind::AgentSessionCompleted { session }
                | RuntimeEventKind::AgentSessionFailed { session },
            ) => {
                session.session_id == *session_id && session.status == AgentSessionStatus::Cancelled
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum D11Intent {
    ProbeProject,
    PreviewProjectConfig {
        contents: String,
    },
    ConfirmProjectConfig,
    StoreCredentialHandle {
        provider_id: String,
        backend_id: String,
        credential_request_id: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D11IntentResult {
    pub projection: D11IntakeProjection,
    pub pending_command_id: Option<String>,
    pub pending_intent: Option<String>,
}

enum D11CompletionTarget {
    ProjectProbe,
    ProjectConfigPreview {
        content_sha256: String,
    },
    ProjectConfigConfirm {
        preview_id: String,
        content_sha256: String,
    },
    CredentialHandle {
        provider_id: String,
        backend_id: String,
    },
}

impl D11CompletionTarget {
    fn from_command(command: &RuntimeCommand) -> Result<Self, String> {
        match command {
            RuntimeCommand::ProbeProject => Ok(Self::ProjectProbe),
            RuntimeCommand::PreviewProjectConfig { contents } => Ok(Self::ProjectConfigPreview {
                content_sha256: sha256(contents.as_bytes()),
            }),
            RuntimeCommand::ConfirmProjectConfig {
                preview_id,
                content_sha256,
            } => Ok(Self::ProjectConfigConfirm {
                preview_id: preview_id.clone(),
                content_sha256: content_sha256.clone(),
            }),
            RuntimeCommand::StoreCredentialHandle {
                provider_id,
                backend_id,
                ..
            } => Ok(Self::CredentialHandle {
                provider_id: provider_id.clone(),
                backend_id: backend_id.clone(),
            }),
            _ => Err("unsupported D11 Core command".to_string()),
        }
    }

    fn intent_name(&self) -> &'static str {
        match self {
            Self::ProjectProbe => "probe_project",
            Self::ProjectConfigPreview { .. } => "preview_project_config",
            Self::ProjectConfigConfirm { .. } => "confirm_project_config",
            Self::CredentialHandle { .. } => "store_credential_handle",
        }
    }

    fn matches_fact(&self, kind: &RuntimeEventKind) -> bool {
        match kind {
            RuntimeEventKind::ProjectProbed { .. } => matches!(self, Self::ProjectProbe),
            RuntimeEventKind::ProjectConfigPreviewed { preview } => {
                matches!(self, Self::ProjectConfigPreview { content_sha256 } if preview.content_sha256 == *content_sha256)
            }
            RuntimeEventKind::ProjectConfigConfirmed { preview } => {
                matches!(self, Self::ProjectConfigConfirm { preview_id, content_sha256 }
                    if preview.preview_id == *preview_id && preview.content_sha256 == *content_sha256)
            }
            RuntimeEventKind::CredentialHandleStored { handle } => {
                matches!(self, Self::CredentialHandle { provider_id, backend_id }
                    if handle.provider_id == *provider_id && handle.backend_id == *backend_id)
            }
            _ => false,
        }
    }

    fn matches_approval(&self, approval: &ApprovalRequestView) -> bool {
        match self {
            Self::ProjectConfigConfirm {
                preview_id,
                content_sha256,
            } => {
                matches!(
                    approval.tool_name.as_str(),
                    "project_config_confirm" | "workflow_project_config_confirm"
                ) && approval_metadata_exact(approval, "sha256", content_sha256, is_lower_hex_64)
                    && (!approval_metadata_field_exists(approval, "preview_id")
                        || approval_metadata_exact(approval, "preview_id", preview_id, |_| true))
            }
            Self::CredentialHandle {
                provider_id,
                backend_id,
            } => {
                approval.tool_name.contains("credential_handle_store")
                    && approval.input_preview.contains(provider_id)
                    && approval.input_preview.contains(backend_id)
            }
            Self::ProjectProbe | Self::ProjectConfigPreview { .. } => false,
        }
    }
}

struct PendingD11 {
    command_id: String,
    target: D11CompletionTarget,
    accepted: bool,
    approval_request_id: Option<String>,
}

enum PendingObservation {
    Continue,
    ClearedContinue,
    ClearedStop,
}

impl PendingD11 {
    fn observe(&mut self, envelope: &RuntimeEventEnvelope) -> PendingObservation {
        let RuntimeWireEvent::Known(event) = &envelope.event else {
            return PendingObservation::Continue;
        };
        match &event.kind {
            RuntimeEventKind::CommandRejected {
                command_id: rejected,
                ..
            } if rejected == &self.command_id => PendingObservation::ClearedContinue,
            RuntimeEventKind::CommandAccepted {
                command_id: accepted,
                ..
            }
            | RuntimeEventKind::LaneCommandAccepted {
                command_id: accepted,
                ..
            } if accepted == &self.command_id => {
                self.accepted = true;
                PendingObservation::Continue
            }
            RuntimeEventKind::ApprovalRequested { approval }
                if self.accepted && self.target.matches_approval(approval) =>
            {
                self.approval_request_id = Some(approval.id.clone());
                PendingObservation::Continue
            }
            RuntimeEventKind::ApprovalResolved {
                request_id,
                decision,
                ..
            } if self.approval_request_id.as_deref() == Some(request_id.as_str()) => match decision
            {
                ApprovalDecision::Allow { .. } => PendingObservation::Continue,
                ApprovalDecision::Deny => PendingObservation::ClearedContinue,
            },
            kind if self.accepted && self.target.matches_fact(kind) => {
                PendingObservation::ClearedStop
            }
            _ => PendingObservation::Continue,
        }
    }
}

impl GuiCoreAdapter {
    pub fn new(client: Box<dyn CoreClient>) -> Self {
        Self {
            client,
            projection: RuntimeProjection::default(),
            capabilities: BTreeSet::new(),
            pending_d11: None,
            pending_d4: None,
            reviewed_d4: None,
            d4_receipt: None,
            d4_outcome: D4OutcomeProjection::idle(),
            pending_d1: None,
            d1_outcome: D1OutcomeProjection::idle(),
            pending_permission: None,
            permission_outcome: PermissionOutcomeProjection::idle(),
            connection: D6ConnectionState::Disconnected,
            connection_detail: None,
            d2_selected: None,
        }
    }

    pub fn connect(&mut self) -> Result<(), CoreClientError> {
        self.connection = D6ConnectionState::Connecting;
        self.connection_detail = None;
        let handshake = match self.client.discover() {
            Ok(handshake) => handshake,
            Err(error) => {
                self.record_connection_error(&error);
                return Err(error);
            }
        };
        self.capabilities = handshake
            .capabilities
            .into_iter()
            .map(|capability| capability.0)
            .collect();
        let snapshot = match self.client.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.record_connection_error(&error);
                return Err(error);
            }
        };
        self.publish(snapshot);
        self.connection = D6ConnectionState::Live;
        self.connection_detail = None;
        Ok(())
    }

    pub fn send_intent(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.client.send(command)
    }

    pub fn pump(&mut self, timeout: Duration) -> Result<bool, CoreClientError> {
        let event = self.receive_and_publish(timeout)?;
        if let Some(event) = &event {
            self.observe_pending(event);
        }
        Ok(event.is_some())
    }

    pub fn projection(&self) -> &RuntimeProjection {
        &self.projection
    }

    /// Workspace root Core probed, when Core has published one.
    ///
    /// The client keeps no second copy of where the workspace lives; reading
    /// Core's probe keeps a reopened workspace from resolving against the
    /// previous root.
    pub fn workspace_root(&self) -> Option<String> {
        self.projection
            .view()?
            .project_probe
            .as_ref()
            .map(|probe| probe.root.clone())
    }

    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn d1_cockpit(&self, selected_lane_id: Option<&str>) -> Option<D1CockpitProjection> {
        let mut projection = self.projection.d1_cockpit(selected_lane_id)?;
        if projection.permission_dock.request.is_none()
            && let Some(pending) = &self.pending_d1
            && pending.accepted
            && matches!(pending.kind, PendingD1Kind::CreateStarterLane { .. })
            && let Some(permission_dock) = self.projection.permission_dock_for_owner(&pending.owner)
        {
            // Lane creation approval precedes Lane registration, so no selected
            // owner exists yet. Route only the exact accepted Create owner;
            // never fall back to another owner's global pending approval.
            projection.permission_dock = permission_dock;
        }
        let transport_recovery = self.d6_recovery();
        if self.connection != D6ConnectionState::Live
            || projection.recovery.state != D6State::EventGap
        {
            projection.recovery = transport_recovery;
        }
        Some(projection)
    }

    pub fn permission_dock(&self) -> Option<PermissionDockProjection> {
        self.projection.permission_dock()
    }

    /// Pages the audit trail through the Core replay cursor.
    ///
    /// `after` is the opaque `stream:sequence` cursor returned by the previous
    /// page. A replay failure is reported rather than rendered as a shorter
    /// trail, because a truncated audit view reads as a complete one.
    pub fn d14_audit_timeline(
        &mut self,
        after: Option<&str>,
        limit: u32,
    ) -> Result<crate::D14AuditTimelineProjection, String> {
        let cursor = match after {
            Some(raw) => parse_audit_cursor(raw)?,
            None => self
                .projection
                .cursor()
                .cloned()
                .map(|cursor| viden_core::EventCursor {
                    stream_id: cursor.stream_id,
                    sequence: 0,
                })
                .unwrap_or(viden_core::EventCursor {
                    stream_id: String::new(),
                    sequence: 0,
                }),
        };
        let batch = self
            .client
            .replay(viden_core::ReplayRequest {
                after: cursor,
                limit,
            })
            .map_err(|error| format!("Core replay failed: {error}"))?;
        let rows = batch
            .events
            .iter()
            .map(|envelope| {
                let (kind, known, timestamp) = match &envelope.event {
                    viden_core::RuntimeWireEvent::Known(event) => (
                        // The canonical name is Core's own serde discriminant,
                        // so the client never keeps a second event vocabulary.
                        core_event_kind(&event.kind),
                        true,
                        event.timestamp,
                    ),
                    _ => ("unknown".to_string(), false, None),
                };
                crate::D14RowProjection {
                    sequence: envelope.cursor.sequence,
                    stream_id: envelope.cursor.stream_id.clone(),
                    kind,
                    known,
                    timestamp,
                    project_id: envelope.owner.project_id.clone(),
                    lane_id: envelope.owner.lane_id.clone(),
                    session_id: envelope.owner.session_id.clone(),
                    task_id: envelope.owner.task_id.clone(),
                }
            })
            .collect();
        Ok(crate::D14AuditTimelineProjection {
            rows,
            next_cursor: Some(format!("{}:{}", batch.next.stream_id, batch.next.sequence)),
            complete: batch.complete,
        })
    }

    pub fn d13_fleet_workflow(&self) -> Option<crate::D13FleetWorkflowProjection> {
        self.projection.d13_fleet_workflow()
    }

    pub fn d12_integration_gate(&self) -> Option<crate::D12IntegrationGateProjection> {
        self.projection.d12_integration_gate(None)
    }

    /// Projects the gate the shell selected without persisting the selection.
    pub fn d12_integration_gate_for(
        &self,
        gate_id: &str,
    ) -> Option<crate::D12IntegrationGateProjection> {
        self.projection.d12_integration_gate(Some(gate_id))
    }

    pub fn d10_lane_monitor(&self) -> Option<crate::D10LaneMonitorProjection> {
        self.projection.d10_lane_monitor()
    }

    pub fn d2_decisions(&self) -> Option<crate::D2DecisionsProjection> {
        self.projection.d2_decisions(self.d2_selected.as_deref())
    }

    /// Projects the queue with an explicit selection without persisting it.
    pub fn d2_decisions_for(&self, id: &str) -> Option<crate::D2DecisionsProjection> {
        self.projection.d2_decisions(Some(id))
    }

    /// Sends one D2 decision. Selection is local; every real decision travels
    /// as the Core command that owns that fact family.
    pub fn d2_send_intent(
        &mut self,
        command_id: &str,
        intent: crate::D2Intent,
    ) -> Result<crate::D2IntentResult, String> {
        let outcome = match intent {
            crate::D2Intent::Select { id } => {
                self.d2_selected = Some(id);
                PermissionOutcomeProjection::idle()
            }
            crate::D2Intent::RespondApproval {
                request_id,
                choice,
                feedback,
            } => {
                let envelope = self.permission_envelope(
                    command_id,
                    PermissionIntent::Respond {
                        request_id: request_id.clone(),
                        choice,
                        feedback,
                    },
                )?;
                self.client.send(envelope).map_err(|e| e.to_string())?;
                self.d2_selected = Some(request_id);
                PermissionOutcomeProjection::pending()
            }
            crate::D2Intent::DecideContract {
                contract_id,
                accept,
            } => {
                let envelope = self.d2_contract_envelope(command_id, &contract_id, accept)?;
                self.client.send(envelope).map_err(|e| e.to_string())?;
                self.d2_selected = Some(contract_id);
                PermissionOutcomeProjection::pending()
            }
        };
        let projection = self
            .d2_decisions()
            .ok_or_else(|| "Core has not published a decision projection".to_string())?;
        Ok(crate::D2IntentResult {
            projection,
            pending_command_id: match outcome.state {
                "pending" => Some(command_id.to_string()),
                _ => None,
            },
            outcome,
        })
    }

    /// Replays the contract identity Core recorded. The GUI never rebuilds an
    /// owner or task id from display text.
    fn d2_contract_envelope(
        &self,
        command_id: &str,
        contract_id: &str,
        accept: bool,
    ) -> Result<RuntimeCommandEnvelope, String> {
        let view = self
            .projection
            .view()
            .ok_or_else(|| "Core has not published a decision projection".to_string())?;
        if view.snapshot.work_mode == WorkMode::Plan {
            return Err("Plan mode blocks contract decisions".to_string());
        }
        let contract = view
            .contracts
            .iter()
            .find(|entry| entry.contract_id == contract_id)
            .ok_or_else(|| format!("contract `{contract_id}` is not published by Core"))?;
        Ok(RuntimeCommandEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            client_id: "viden-gui".to_string(),
            command_id: command_id.to_string(),
            owner: contract.owner.clone(),
            command: RuntimeCommand::ConfirmContract {
                contract_id: contract.contract_id.clone(),
                task_id: contract.task_id.clone(),
                owner: contract.owner.clone(),
                summary: contract.summary.clone(),
                decision: if accept {
                    viden_core::ContractDecision::Confirmed
                } else {
                    viden_core::ContractDecision::Rejected
                },
            },
        })
    }

    pub fn d6_recovery(&self) -> D6RecoveryProjection {
        self.projection
            .d6_recovery(
                self.connection,
                self.connection_detail.as_deref(),
                self.supports("runtime.approvals"),
            )
            .unwrap_or(D6RecoveryProjection {
                connection: self.connection,
                state: match self.connection {
                    D6ConnectionState::Connecting => crate::D6State::Connecting,
                    D6ConnectionState::Incompatible => crate::D6State::IncompatibleSchema,
                    D6ConnectionState::Recovering => crate::D6State::EventGap,
                    _ => crate::D6State::Disconnected,
                },
                detail: self.connection_detail.clone(),
                hint: None,
                recoverable: false,
                business_success_blocked: true,
                used_tokens: None,
                hard_token_limit: None,
                missing_capabilities: Vec::new(),
                actions: Vec::new(),
            })
    }

    /// Sends one D6 recovery action as the Core command that owns that fact
    /// family, then republishes the recovery projection.
    ///
    /// The target ids come from the projection the operator acted on, and are
    /// re-resolved against the current Core view here: a session or Lane that
    /// disappeared between render and click is rejected instead of being sent
    /// as a command Core would have to reject.
    pub fn send_d6_intent_and_wait(
        &mut self,
        command_id: &str,
        intent: crate::D6Intent,
        event_timeout: Duration,
    ) -> Result<crate::D6IntentResult, String> {
        let envelope = self.d6_envelope(command_id, &intent)?;
        self.client
            .send(envelope)
            .map_err(|error| error.to_string())?;
        // Drain the receipts Core already published so the returned projection
        // is not a turn stale; the ordered-event pump delivers the rest.
        self.pump_events(event_timeout);
        Ok(crate::D6IntentResult {
            projection: self.d6_recovery(),
            pending_command_id: Some(command_id.to_string()),
        })
    }

    fn d6_envelope(
        &self,
        command_id: &str,
        intent: &crate::D6Intent,
    ) -> Result<RuntimeCommandEnvelope, String> {
        let view = self
            .projection
            .view()
            .ok_or_else(|| "Core has not published a recovery projection".to_string())?;
        let (owner, command) = match intent {
            crate::D6Intent::Restart { session_id } => {
                let sessions = view
                    .agent_sessions
                    .iter()
                    .filter(|session| session.session_id == *session_id)
                    .collect::<Vec<_>>();
                let [session] = sessions.as_slice() else {
                    return Err(format!(
                        "Core has no unique ACP session `{session_id}` to restart"
                    ));
                };
                if !matches!(
                    session.status,
                    AgentSessionStatus::Failed | AgentSessionStatus::Cancelled
                ) {
                    return Err(format!("ACP session `{session_id}` is not retryable"));
                }
                (
                    session.owner.clone(),
                    RuntimeCommand::RetryAgentSession {
                        session_id: session_id.clone(),
                    },
                )
            }
            crate::D6Intent::CloseLane { lane_id } => {
                let lane = view
                    .lanes
                    .iter()
                    .find(|lane| lane.id == *lane_id)
                    .ok_or_else(|| format!("Core has no Lane `{lane_id}` to close"))?;
                if !lane.is_active() {
                    return Err(format!("Lane `{lane_id}` is not active"));
                }
                // Lane lifecycle is routed by the command's own lane id; the
                // owner still names the Lane so the audit trail keeps the
                // exact execution identity.
                (
                    self.exact_runtime_owner(lane_id).unwrap_or(RuntimeOwner {
                        lane_id: Some(lane_id.clone()),
                        ..RuntimeOwner::default()
                    }),
                    RuntimeCommand::StopLane {
                        lane_id: lane_id.clone(),
                    },
                )
            }
        };
        Ok(RuntimeCommandEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            client_id: "viden-gui".to_string(),
            command_id: command_id.to_string(),
            owner,
            command,
        })
    }

    pub fn recover(&mut self) -> Result<(), CoreClientError> {
        self.connection = D6ConnectionState::Recovering;
        let snapshot = match self.client.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.record_connection_error(&error);
                return Err(error);
            }
        };
        self.publish(snapshot);
        self.connection = D6ConnectionState::Live;
        self.connection_detail = None;
        Ok(())
    }

    pub fn send_permission_intent(
        &mut self,
        command_id: &str,
        intent: PermissionIntent,
    ) -> Result<(), String> {
        let envelope = self.permission_envelope(command_id, intent)?;
        self.client
            .send(envelope)
            .map_err(|error| error.to_string())
    }

    pub fn send_permission_intent_and_wait(
        &mut self,
        command_id: &str,
        intent: PermissionIntent,
        event_timeout: Duration,
    ) -> Result<PermissionIntentResult, String> {
        if let Some(pending) = &self.pending_permission {
            return Err(format!(
                "permission command `{}` is still pending",
                pending.command_id
            ));
        }
        let envelope = self.permission_envelope(command_id, intent)?;
        let RuntimeCommand::RespondToApproval { request_id, .. } = &envelope.command else {
            return Err("permission dock produced a non-approval command".to_string());
        };
        let request = self
            .projection
            .view()
            .and_then(|view| {
                view.pending_approvals
                    .iter()
                    .find(|item| item.id == *request_id)
            })
            .ok_or_else(|| format!("approval `{request_id}` is no longer pending"))?;
        self.pending_permission = Some(PendingPermission {
            command_id: command_id.to_string(),
            request_id: request_id.clone(),
            owner: request.owner.clone(),
            audit_id: request.audit_id.clone(),
        });
        self.client.send(envelope).map_err(|error| {
            self.pending_permission = None;
            error.to_string()
        })?;
        self.permission_outcome = PermissionOutcomeProjection::pending();
        self.poll_permission(event_timeout)
    }

    pub fn poll_permission(
        &mut self,
        event_timeout: Duration,
    ) -> Result<PermissionIntentResult, String> {
        let mut received = false;
        for _ in 0..8 {
            let event = match self
                .receive_event_until(event_timeout)
                .map_err(|error| error.to_string())?
            {
                Some(event) => event,
                None => break,
            };
            received = true;
            let permission_complete = self.observe_pending_permission(&event);
            self.observe_pending(&event);
            self.observe_d4(&event);
            if permission_complete {
                break;
            }
        }
        if received {
            self.refresh_projection()
                .map_err(|error| error.to_string())?;
        }
        Ok(PermissionIntentResult {
            projection: self
                .permission_dock()
                .ok_or_else(|| "Core has not published a permission projection".to_string())?,
            pending_command_id: self
                .pending_permission
                .as_ref()
                .map(|pending| pending.command_id.clone()),
            outcome: self.permission_outcome.clone(),
        })
    }

    fn permission_envelope(
        &self,
        command_id: &str,
        intent: PermissionIntent,
    ) -> Result<RuntimeCommandEnvelope, String> {
        let PermissionIntent::Respond {
            request_id,
            choice,
            feedback,
        } = intent;
        let view = self
            .projection
            .view()
            .ok_or_else(|| "Core has not published a permission projection".to_string())?;
        let request = view
            .pending_approvals
            .iter()
            .find(|request| request.id == request_id)
            .ok_or_else(|| format!("approval `{request_id}` is no longer pending"))?;
        if request.is_mutating && view.snapshot.work_mode == WorkMode::Plan {
            return Err("Plan mode blocks mutating approval responses".to_string());
        }
        let exact_scope = |kind: PermissionChoice| {
            let scopes = request
                .allowed_scopes
                .iter()
                .filter(|scope| {
                    matches!(
                        (kind, *scope),
                        (PermissionChoice::Once, ApprovalScope::Once)
                            | (PermissionChoice::Session, ApprovalScope::Session { .. })
                            | (
                                PermissionChoice::RepoAllowlist,
                                ApprovalScope::RepoAllowlist { .. }
                            )
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            match scopes.as_slice() {
                [scope] => Ok(scope.clone()),
                _ => Err("the selected approval scope is unavailable or ambiguous".to_string()),
            }
        };
        let decision = match choice {
            PermissionChoice::Once
            | PermissionChoice::Session
            | PermissionChoice::RepoAllowlist => ApprovalDecision::Allow {
                scope: exact_scope(choice)?,
            },
            PermissionChoice::Deny => ApprovalDecision::Deny,
            PermissionChoice::Always | PermissionChoice::Edit => {
                return Err("GUI-CORE-003: typed approval action is unavailable".to_string());
            }
        };
        Ok(RuntimeCommandEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            client_id: "viden-gui".to_string(),
            command_id: command_id.to_string(),
            owner: request.owner.clone(),
            command: RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse { decision, feedback },
            },
        })
    }

    /// Sends the single D1 composer/cancel intent through Core's shared path.
    pub fn send_d1_intent(&mut self, command_id: &str, intent: D1Intent) -> Result<(), String> {
        let (envelope, _) = self.d1_envelope(command_id, intent)?;
        self.client
            .send(envelope)
            .map_err(|error| error.to_string())
    }

    fn d1_envelope(
        &self,
        command_id: &str,
        intent: D1Intent,
    ) -> Result<(RuntimeCommandEnvelope, PendingD1Kind), String> {
        let (owner, command, kind) = match intent {
            D1Intent::Submit { lane_id, content } => {
                if content.trim().is_empty() {
                    return Err("D1 composer content is empty".to_string());
                }
                let projection = self
                    .d1_cockpit(Some(&lane_id))
                    .ok_or_else(|| "Core has not published a D1 projection".to_string())?;
                if projection.selected_lane_id.as_deref() != Some(lane_id.as_str()) {
                    return Err(format!("Core has no Lane `{lane_id}`"));
                }
                let owner = self.exact_submit_owner(&lane_id)?;
                let command = if projection.composer.busy {
                    RuntimeCommand::QueueFollowUp { content }
                } else {
                    RuntimeCommand::SubmitUserInput { content }
                };
                let kind = if matches!(command, RuntimeCommand::QueueFollowUp { .. }) {
                    PendingD1Kind::Queue
                } else {
                    PendingD1Kind::Submit
                };
                (owner, command, kind)
            }
            D1Intent::Cancel { lane_id } => {
                if !self.supports(D1_OWNER_CAPABILITY) {
                    return Err(format!("missing Core capability `{D1_OWNER_CAPABILITY}`"));
                }
                let view = self
                    .projection
                    .view()
                    .ok_or_else(|| "Core has not published a D1 projection".to_string())?;
                let active = view
                    .lanes
                    .iter()
                    .find(|lane| lane.id == lane_id)
                    .is_some_and(|lane| lane.is_active());
                if !active {
                    return Err(format!("Lane `{lane_id}` is not active"));
                }
                let owner = self.exact_runtime_owner(&lane_id)?;
                (
                    owner,
                    RuntimeCommand::CancelActiveTurn,
                    PendingD1Kind::Cancel,
                )
            }
            D1Intent::QueryAgentAdapters => (
                RuntimeOwner::default(),
                RuntimeCommand::QueryAgentAdapters,
                PendingD1Kind::QueryAgentAdapters,
            ),
            D1Intent::ProbeAgentAdapter { agent_id } => (
                RuntimeOwner::default(),
                RuntimeCommand::ProbeAgentAdapter {
                    agent_id: agent_id.clone(),
                },
                PendingD1Kind::ProbeAgentAdapter { agent_id },
            ),
            D1Intent::PreviewDefaultLane { preset } => {
                let preset = parse_starter_preset(&preset)?;
                (
                    RuntimeOwner::default(),
                    RuntimeCommand::PreviewDefaultStarterLane { preset },
                    PendingD1Kind::PreviewDefaultLane,
                )
            }
            D1Intent::CreateStarterLane {
                lane_id,
                preset,
                branch,
                preview_id,
                content_sha256,
            } => {
                let preset = parse_starter_preset(&preset)?;
                let preview = self
                    .projection
                    .view()
                    .and_then(|view| {
                        view.starter_lane_previews.iter().find(|preview| {
                            preview.preview_id == preview_id
                                && preview.content_sha256 == content_sha256
                                && preview.lane.id == lane_id
                                && preview.lane.branch == branch
                        })
                    })
                    .ok_or_else(|| "Core has not published the exact Lane preview".to_string())?;
                (
                    preview.owner.clone(),
                    RuntimeCommand::CreateStarterLane {
                        request: StarterLaneRequest {
                            lane_id,
                            preset,
                            branch,
                            worktree_path: None,
                        },
                        preview_id: preview_id.clone(),
                        content_sha256: content_sha256.clone(),
                    },
                    PendingD1Kind::CreateStarterLane {
                        preview_id,
                        content_sha256,
                    },
                )
            }
            D1Intent::StartAgentSession {
                lane_id,
                agent_id,
                model,
                task,
            } => {
                if task.trim().is_empty() {
                    return Err("ACP task is empty".to_string());
                }
                let view = self
                    .projection
                    .view()
                    .ok_or_else(|| "Core has not published a D1 projection".to_string())?;
                if !view.lanes.iter().any(|lane| lane.id == lane_id) {
                    return Err(format!("Core has no Lane `{lane_id}`"));
                }
                let ready = view.agent_adapters.iter().any(|adapter| {
                    adapter.agent_id == agent_id
                        && adapter.route == viden_core::AgentRoute::Acp
                        && adapter.startability == viden_core::AgentStartability::Ready
                });
                if !ready {
                    return Err(format!("ACP agent `{agent_id}` is not startable"));
                }
                let matching_sessions_before = view
                    .agent_sessions
                    .iter()
                    .filter(|session| session.lane_id == lane_id && session.agent_id == agent_id)
                    .count();
                (
                    RuntimeOwner {
                        lane_id: Some(lane_id.clone()),
                        ..RuntimeOwner::default()
                    },
                    RuntimeCommand::StartAgentSession {
                        request: AgentSessionRequest {
                            lane_id: lane_id.clone(),
                            agent_id: agent_id.clone(),
                            model,
                            load_session_id: None,
                            task,
                        },
                    },
                    PendingD1Kind::StartAgentSession {
                        lane_id,
                        agent_id,
                        matching_sessions_before,
                    },
                )
            }
            D1Intent::SendAgentSessionInput {
                lane_id,
                session_id,
                content,
            } => {
                if content.trim().is_empty() {
                    return Err("ACP input is empty".to_string());
                }
                let session = self.exact_agent_session(&lane_id, &session_id)?;
                (
                    session.owner.clone(),
                    RuntimeCommand::SendAgentSessionInput {
                        input: AgentSessionInput {
                            session_id: session_id.clone(),
                            content,
                        },
                    },
                    PendingD1Kind::SendAgentSessionInput { session_id },
                )
            }
            D1Intent::RetryAgentSession {
                lane_id,
                session_id,
            } => {
                let session = self.exact_agent_session(&lane_id, &session_id)?;
                if !matches!(
                    session.status,
                    AgentSessionStatus::Failed | AgentSessionStatus::Cancelled
                ) {
                    return Err(format!("ACP session `{session_id}` is not retryable"));
                }
                (
                    session.owner.clone(),
                    RuntimeCommand::RetryAgentSession {
                        session_id: session_id.clone(),
                    },
                    PendingD1Kind::RetryAgentSession { session_id },
                )
            }
            D1Intent::CancelAgentSession {
                lane_id,
                session_id,
            } => {
                let session = self.exact_agent_session(&lane_id, &session_id)?;
                (
                    session.owner.clone(),
                    RuntimeCommand::CancelAgentSession {
                        session_id: session_id.clone(),
                    },
                    PendingD1Kind::CancelAgentSession { session_id },
                )
            }
        };
        Ok((
            RuntimeCommandEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                client_id: "viden-gui".to_string(),
                command_id: command_id.to_string(),
                owner,
                command,
            },
            kind,
        ))
    }

    pub fn send_d1_intent_and_wait(
        &mut self,
        command_id: &str,
        intent: D1Intent,
        event_timeout: Duration,
    ) -> Result<D1IntentResult, String> {
        if let Some(pending) = &self.pending_d1 {
            return Err(format!(
                "D1 command `{}` is still pending",
                pending.command_id
            ));
        }
        let (envelope, kind) = self.d1_envelope(command_id, intent)?;
        let owner = envelope.owner.clone();
        let selected_lane_id = owner.lane_id.clone();
        self.client
            .send(envelope)
            .map_err(|error| error.to_string())?;
        self.pending_d1 = Some(PendingD1 {
            command_id: command_id.to_string(),
            owner,
            kind,
            accepted: false,
        });
        self.d1_outcome = D1OutcomeProjection::pending();
        self.poll_d1(selected_lane_id.as_deref(), event_timeout)
    }

    pub fn poll_d1(
        &mut self,
        selected_lane_id: Option<&str>,
        event_timeout: Duration,
    ) -> Result<D1IntentResult, String> {
        let mut received = false;
        let mut receive_failed = false;
        for _ in 0..8 {
            let event = match self.receive_event_until(event_timeout) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(_) => {
                    // The transport state is already classified by receive_event.
                    // Return the last canonical snapshot with D6 recovery facts attached.
                    receive_failed = true;
                    break;
                }
            };
            received = true;
            self.observe_pending_permission(&event);
            let observation = self
                .pending_d1
                .as_mut()
                .map_or(D1Observation::Continue, |pending| pending.observe(&event));
            match observation {
                D1Observation::Continue => {}
                D1Observation::Confirmed => {
                    self.pending_d1 = None;
                    self.d1_outcome = D1OutcomeProjection::confirmed();
                    break;
                }
                D1Observation::Rejected(reason) => {
                    self.pending_d1 = None;
                    self.d1_outcome = D1OutcomeProjection::rejected(reason);
                    break;
                }
            }
        }
        if received && !receive_failed {
            self.refresh_projection()
                .map_err(|error| error.to_string())?;
        }
        let pending_satisfied = self
            .pending_d1
            .as_ref()
            .zip(self.projection.view())
            .is_some_and(|(pending, view)| {
                pending.is_satisfied_by_agent_sessions(&view.agent_sessions)
            });
        if pending_satisfied {
            // The reducer can publish the typed session before this adapter
            // observes the matching business event. Reconcile the accepted
            // command from Core state so a completed start never blocks the
            // next composer turn until the desktop process restarts.
            self.pending_d1 = None;
            self.d1_outcome = D1OutcomeProjection::confirmed();
        }
        let selected = selected_lane_id.or_else(|| {
            self.pending_d1
                .as_ref()
                .and_then(|pending| pending.owner.lane_id.as_deref())
        });
        Ok(D1IntentResult {
            projection: self
                .d1_cockpit(selected)
                .ok_or_else(|| "Core has not published a D1 projection".to_string())?,
            pending_command_id: self
                .pending_d1
                .as_ref()
                .map(|pending| pending.command_id.clone()),
            outcome: self.d1_outcome.clone(),
        })
    }

    fn observe_pending_permission(&mut self, event: &RuntimeEventEnvelope) -> bool {
        let observation = self
            .pending_permission
            .as_ref()
            .map_or(PermissionObservation::Continue, |pending| {
                pending.observe(event)
            });
        match observation {
            PermissionObservation::Continue => false,
            PermissionObservation::Confirmed => {
                self.pending_permission = None;
                self.permission_outcome = PermissionOutcomeProjection::confirmed();
                true
            }
            PermissionObservation::Rejected(reason) => {
                self.pending_permission = None;
                self.permission_outcome = PermissionOutcomeProjection::rejected(reason);
                true
            }
        }
    }

    fn exact_runtime_owner(&self, lane_id: &str) -> Result<RuntimeOwner, String> {
        self.exact_lane_owner(lane_id, "runtime")
    }

    fn exact_submit_owner(&self, lane_id: &str) -> Result<RuntimeOwner, String> {
        self.exact_lane_owner(lane_id, "submit")
    }

    /// Raw same-Lane binding cardinality is authoritative; malformed duplicates block all routing.
    fn exact_lane_owner(&self, lane_id: &str, purpose: &str) -> Result<RuntimeOwner, String> {
        let view = self
            .projection
            .view()
            .ok_or_else(|| "Core has not published a D1 projection".to_string())?;
        let runtime = view
            .lane_runtime_owners
            .iter()
            .filter(|binding| binding.lane_id == lane_id)
            .collect::<Vec<_>>();
        match runtime.as_slice() {
            [binding] if binding.owner.lane_id.as_deref() == Some(lane_id) => {
                return Ok(binding.owner.clone());
            }
            _ => Err(format!(
                "Lane `{lane_id}` does not have one exact Core {purpose} owner"
            )),
        }
    }

    fn exact_agent_session(
        &self,
        lane_id: &str,
        session_id: &str,
    ) -> Result<&viden_core::AgentSessionView, String> {
        let view = self
            .projection
            .view()
            .ok_or_else(|| "Core has not published a D1 projection".to_string())?;
        let sessions = view
            .agent_sessions
            .iter()
            .filter(|session| session.lane_id == lane_id && session.session_id == session_id)
            .collect::<Vec<_>>();
        let [session] = sessions.as_slice() else {
            return Err(format!(
                "Core has no unique ACP session `{session_id}` for Lane `{lane_id}`"
            ));
        };
        let bindings = view
            .lane_runtime_owners
            .iter()
            .filter(|binding| binding.lane_id == lane_id)
            .collect::<Vec<_>>();
        match bindings.as_slice() {
            [binding]
                if binding.owner.lane_id.as_deref() == Some(lane_id)
                    && session.owner == binding.owner => {}
            [] if exact_terminal_agent_session(view, lane_id)
                .is_some_and(|restored| restored.session_id == session_id) => {}
            [binding] if binding.owner.lane_id.as_deref() == Some(lane_id) => {
                return Err(format!(
                    "ACP session `{session_id}` does not match the exact Core owner for Lane `{lane_id}`"
                ));
            }
            _ => {
                return Err(format!(
                    "Lane `{lane_id}` does not have one exact Core ACP owner"
                ));
            }
        }
        Ok(session)
    }

    pub fn send_d11_intent(&mut self, command_id: &str, intent: D11Intent) -> Result<(), String> {
        let envelope = self.d11_envelope(command_id, intent)?;
        self.client
            .send(envelope)
            .map_err(|error| error.to_string())
    }

    fn d11_envelope(
        &self,
        command_id: &str,
        intent: D11Intent,
    ) -> Result<RuntimeCommandEnvelope, String> {
        if matches!(&intent, D11Intent::StoreCredentialHandle { .. }) {
            return Err("GUI-CORE-001: platform credential intake is unavailable".to_string());
        }
        let required_capability = match intent {
            D11Intent::ProbeProject
            | D11Intent::PreviewProjectConfig { .. }
            | D11Intent::ConfirmProjectConfig => "runtime.project_onboarding",
            D11Intent::StoreCredentialHandle { .. } => "runtime.credential_handles",
        };
        if !self.supports(required_capability) {
            return Err(format!("missing Core capability `{required_capability}`"));
        }

        let command = self.d11_command(intent)?;
        let envelope = RuntimeCommandEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            client_id: "viden-gui".to_string(),
            command_id: command_id.to_string(),
            owner: Default::default(),
            command,
        };
        Ok(envelope)
    }

    pub fn send_d11_intent_and_wait(
        &mut self,
        command_id: &str,
        intent: D11Intent,
        event_timeout: Duration,
    ) -> Result<D11IntentResult, String> {
        if let Some(pending) = &self.pending_d11 {
            return Err(format!(
                "D11 command `{}` is still pending",
                pending.command_id
            ));
        }
        let envelope = self.d11_envelope(command_id, intent)?;
        let target = D11CompletionTarget::from_command(&envelope.command)?;
        self.client
            .send(envelope)
            .map_err(|error| error.to_string())?;
        self.pending_d11 = Some(PendingD11 {
            command_id: command_id.to_string(),
            target,
            accepted: false,
            approval_request_id: None,
        });
        // Command acceptance is not business success. Drain the ordered event
        // stream through CoreClient so the returned projection contains the
        // latest project/config/lane fact, rejection, or pending approval.
        self.poll_d11(event_timeout)
    }

    /// Polls Core without sending a command or creating GUI-owned runtime state.
    pub fn poll_d11(&mut self, event_timeout: Duration) -> Result<D11IntentResult, String> {
        let mut received = false;
        for _ in 0..8 {
            let event = match self
                .receive_event_until(event_timeout)
                .map_err(|error| error.to_string())?
            {
                Some(event) => event,
                None => break,
            };
            received = true;
            if self.observe_pending(&event) {
                break;
            }
        }
        if received {
            self.refresh_projection()
                .map_err(|error| error.to_string())?;
        }
        self.d11_result()
    }

    fn d11_result(&self) -> Result<D11IntentResult, String> {
        let mut projection = self
            .projection
            .d11_intake()
            .ok_or_else(|| "Core has not published a D11 projection".to_string())?;
        if let Some(pending) = &self.pending_d11 {
            if let Some(approval_request_id) = &pending.approval_request_id {
                projection.pending_approval =
                    self.projection.d11_approval_by_id(approval_request_id);
            } else {
                projection.pending_approval = None;
            }
        }
        Ok(D11IntentResult {
            projection,
            pending_command_id: self
                .pending_d11
                .as_ref()
                .map(|pending| pending.command_id.clone()),
            pending_intent: self
                .pending_d11
                .as_ref()
                .map(|pending| pending.target.intent_name().to_string()),
        })
    }

    /// Shutdown is intentionally non-mutating: dropping a GUI never sends a runtime command.
    pub fn shutdown(self) {}

    fn publish(&mut self, snapshot: RuntimeSnapshotEnvelope) {
        self.projection.replace(snapshot);
    }

    pub(crate) fn receive_and_publish(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        let event = self.receive_event(timeout)?;
        if event.is_some() {
            self.refresh_projection()?;
        }
        Ok(event)
    }

    pub(crate) fn receive_event(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        let event = match self.client.recv(timeout) {
            Ok(event) => event,
            Err(error) => {
                self.record_connection_error(&error);
                return Err(error);
            }
        };
        Ok(event)
    }

    /// Drains ordered Core events with one bounded wait and refreshes the
    /// projection when anything arrived.
    ///
    /// Returns whether observable state advanced. The desktop event pump uses
    /// this from a background thread to wake the frontend with a push instead
    /// of leaving every screen on its own drain timer; a quiet pump must
    /// therefore report `false` so idle loops stay silent.
    pub fn pump_events(&mut self, event_timeout: Duration) -> bool {
        let mut received = false;
        for _ in 0..16 {
            let event = match self.receive_event_until(if received {
                Duration::ZERO
            } else {
                event_timeout
            }) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(_) => {
                    // The transport state is already classified by
                    // receive_event; a connection change is progress too.
                    return true;
                }
            };
            received = true;
            self.observe_pending_permission(&event);
            self.observe_pending(&event);
            self.observe_d4(&event);
        }
        if received {
            let _ = self.refresh_projection();
        }
        received
    }

    fn receive_event_until(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        if timeout.is_zero() {
            return self.receive_event(timeout);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            if let Some(event) = self.receive_event(remaining)? {
                return Ok(Some(event));
            }
            // StatefulCoreClient also returns None for duplicate events already
            // covered by its confirmed snapshot. Keep waiting within the same
            // deadline so those stale envelopes cannot hide a later fact.
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    pub(crate) fn refresh_projection(&mut self) -> Result<(), CoreClientError> {
        let snapshot = match self.client.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.record_connection_error(&error);
                return Err(error);
            }
        };
        self.publish(snapshot);
        self.connection = D6ConnectionState::Live;
        self.connection_detail = None;
        Ok(())
    }

    fn record_connection_error(&mut self, error: &CoreClientError) {
        self.connection = match error {
            CoreClientError::Compatibility(_) => D6ConnectionState::Incompatible,
            CoreClientError::Transport(_) => D6ConnectionState::Disconnected,
            CoreClientError::Protocol(_)
            | CoreClientError::SnapshotRequired { .. }
            | CoreClientError::MissingSnapshot
            | CoreClientError::HandshakeRequired => D6ConnectionState::Recovering,
        };
        self.connection_detail = Some(error.to_string());
    }

    fn observe_pending(&mut self, event: &RuntimeEventEnvelope) -> bool {
        let observation = self
            .pending_d11
            .as_mut()
            .map_or(PendingObservation::Continue, |pending| {
                pending.observe(event)
            });
        match observation {
            PendingObservation::Continue => false,
            PendingObservation::ClearedContinue => {
                self.pending_d11 = None;
                false
            }
            PendingObservation::ClearedStop => {
                self.pending_d11 = None;
                true
            }
        }
    }

    fn d11_command(&self, intent: D11Intent) -> Result<RuntimeCommand, String> {
        Ok(match intent {
            D11Intent::ProbeProject => RuntimeCommand::ProbeProject,
            D11Intent::PreviewProjectConfig { contents } => {
                RuntimeCommand::PreviewProjectConfig { contents }
            }
            D11Intent::ConfirmProjectConfig => {
                let preview = self
                    .projection
                    .view()
                    .and_then(|view| view.project_config_preview.as_ref())
                    .filter(|preview| preview.is_valid() && preview.exact_contents.is_some())
                    .ok_or_else(|| "Core has no confirmable project config preview".to_string())?;
                RuntimeCommand::ConfirmProjectConfig {
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256.clone(),
                }
            }
            D11Intent::StoreCredentialHandle {
                provider_id,
                backend_id,
                credential_request_id,
            } => RuntimeCommand::StoreCredentialHandle {
                provider_id,
                backend_id,
                credential_request_id,
            },
        })
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse_starter_preset(value: &str) -> Result<StarterLanePreset, String> {
    match value {
        "coder" => Ok(StarterLanePreset::Coder),
        "reviewer" => Ok(StarterLanePreset::Reviewer),
        "tester" => Ok(StarterLanePreset::Tester),
        _ => Err(format!("unsupported starter Lane preset `{value}`")),
    }
}

fn approval_metadata_exact(
    approval: &ApprovalRequestView,
    field: &str,
    expected: &str,
    valid_value: impl Fn(&str) -> bool,
) -> bool {
    approval_metadata_values(approval).any(|(candidate_field, value)| {
        candidate_field == field && value == expected && valid_value(value)
    })
}

fn approval_metadata_field_exists(approval: &ApprovalRequestView, field: &str) -> bool {
    approval_metadata_values(approval).any(|(candidate_field, _)| candidate_field == field)
}

fn approval_metadata_values(approval: &ApprovalRequestView) -> impl Iterator<Item = (&str, &str)> {
    [&approval.input_preview[..], &approval.target.display[..]]
        .into_iter()
        .chain(approval.target.canonical_ref.as_deref())
        .flat_map(|metadata| metadata.split(is_metadata_separator))
        .filter_map(|token| token.split_once('='))
        .filter(|(field, value)| !field.is_empty() && !value.is_empty())
}

fn is_metadata_separator(character: char) -> bool {
    character.is_ascii_whitespace() || matches!(character, ',' | ';')
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use viden_core::{AgentSessionStatus, AgentSessionView, LocalCoreHost, RuntimeOwner};

    use crate::{D6ConnectionState, D11Intent, PermissionChoice, PermissionIntent};

    use super::{PendingD1, PendingD1Kind, open_workspace_with_host};

    fn temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("viden-gui-{label}-{unique}"));
        std::fs::create_dir_all(&path).expect("create temporary GUI directory");
        path
    }

    fn initialize_git_repository(project: &PathBuf) {
        std::fs::write(project.join("README.md"), "# GUI certification fixture\n")
            .expect("write fixture readme");
        for args in [
            vec!["init"],
            vec!["add", "README.md"],
            vec![
                "-c",
                "user.name=Viden GUI Test",
                "-c",
                "user.email=viden-gui-test@localhost",
                "commit",
                "-m",
                "Initialize fixture",
            ],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(project)
                .status()
                .expect("run git fixture command");
            assert!(status.success(), "git fixture command must succeed");
        }
    }

    #[test]
    fn agent_start_reconciles_from_the_projected_session() {
        let lane_id = "lane-reconciled".to_string();
        let agent_id = "codex-acp".to_string();
        let pending = PendingD1 {
            command_id: "start-reconciled".to_string(),
            owner: RuntimeOwner {
                lane_id: Some(lane_id.clone()),
                ..RuntimeOwner::default()
            },
            kind: PendingD1Kind::StartAgentSession {
                lane_id: lane_id.clone(),
                agent_id: agent_id.clone(),
                matching_sessions_before: 0,
            },
            accepted: false,
        };
        let sessions = vec![AgentSessionView {
            session_id: "session-reconciled".to_string(),
            lane_id,
            agent_id,
            model: None,
            status: AgentSessionStatus::Completed,
            owner: RuntimeOwner::default(),
            task: "reply".to_string(),
            diagnostic: None,
            output: Some("done".to_string()),
        }];

        assert!(pending.is_satisfied_by_agent_sessions(&sessions));
    }

    #[test]
    fn opening_git_workspace_projects_lane_eligibility_without_entering_d11() {
        let session_home = temp_dir("eligibility-session-home");
        let project = temp_dir("eligibility-project");
        initialize_git_repository(&project);
        let host = LocalCoreHost::with_session_home(&session_home);

        let adapter = open_workspace_with_host(&host, &project).expect("open local workspace");
        let cockpit = adapter.d1_cockpit(None).expect("D1 cockpit projection");
        let eligibility = cockpit
            .workspace_eligibility
            .expect("workspace eligibility is projected during open");

        assert!(eligibility.is_git_repository);
        assert!(eligibility.has_head);
        assert!(eligibility.can_create_lane);
        assert_eq!(eligibility.diagnostic, None);
    }

    #[test]
    fn local_workspace_adapter_previews_default_lane_through_live_core() {
        let session_home = temp_dir("preview-session-home");
        let project = temp_dir("preview-project");
        initialize_git_repository(&project);
        let host = LocalCoreHost::with_session_home(&session_home);
        let mut adapter = open_workspace_with_host(&host, &project).expect("open local workspace");

        let result = adapter
            .send_d1_intent_and_wait(
                "live-default-preview",
                crate::D1Intent::PreviewDefaultLane {
                    preset: "coder".to_string(),
                },
                Duration::from_millis(250),
            )
            .expect("preview default Lane through live Core");

        assert_eq!(result.pending_command_id, None);
        assert_eq!(result.outcome.state, "confirmed");
        assert_eq!(result.projection.starter_lane_previews.len(), 1);
        assert!(
            result.projection.starter_lane_previews[0]
                .diagnostics
                .is_empty()
        );

        drop(adapter);
        drop(host);
        std::fs::remove_dir_all(session_home).expect("remove temporary session home");
        std::fs::remove_dir_all(project).expect("remove temporary project");
    }

    #[test]
    fn local_workspace_adapter_creates_lane_in_non_git_directory_without_worktree() {
        let session_home = temp_dir("non-git-create-session-home");
        let project = temp_dir("non-git-create-project");
        let host = LocalCoreHost::with_session_home(&session_home);
        let mut adapter = open_workspace_with_host(&host, &project).expect("open local workspace");

        let eligibility = adapter
            .d1_cockpit(None)
            .expect("D1 cockpit projection")
            .workspace_eligibility
            .expect("workspace eligibility is projected during open");
        assert!(!eligibility.is_git_repository);
        assert!(!eligibility.has_head);
        assert!(eligibility.can_create_lane);
        assert_eq!(eligibility.diagnostic, None);

        let preview = adapter
            .send_d1_intent_and_wait(
                "live-non-git-preview",
                crate::D1Intent::PreviewDefaultLane {
                    preset: "coder".to_string(),
                },
                Duration::from_millis(250),
            )
            .expect("preview a non-Git Lane through live Core")
            .projection
            .starter_lane_previews
            .into_iter()
            .next()
            .expect("Core publishes a non-Git starter Lane preview");
        assert_eq!(preview.branch, None);

        let create = adapter
            .send_d1_intent_and_wait(
                "live-non-git-create",
                crate::D1Intent::CreateStarterLane {
                    lane_id: preview.lane_id.clone(),
                    preset: "coder".to_string(),
                    branch: None,
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256,
                },
                Duration::from_millis(250),
            )
            .expect("request non-Git starter Lane creation");
        let request_id = create
            .projection
            .permission_dock
            .request
            .expect("Core approval remains required without Git")
            .id;
        adapter
            .send_permission_intent_and_wait(
                "live-non-git-create-allow-once",
                PermissionIntent::Respond {
                    request_id,
                    choice: PermissionChoice::Once,
                    feedback: None,
                },
                Duration::from_millis(250),
            )
            .expect("allow non-Git starter Lane creation once");
        let created = adapter
            .poll_d1(None, Duration::from_millis(250))
            .expect("observe non-Git starter Lane creation");

        assert!(
            created
                .projection
                .lanes
                .iter()
                .any(|lane| { lane.id == preview.lane_id && lane.branch.is_none() })
        );
        assert!(!project.join(".git").exists());
        assert!(!project.join(".worktrees").exists());

        drop(adapter);
        drop(host);
        std::fs::remove_dir_all(session_home).expect("remove temporary session home");
        std::fs::remove_dir_all(project).expect("remove temporary project");
    }

    #[test]
    fn local_workspace_adapter_creates_previewed_lane_after_live_core_approval() {
        let session_home = temp_dir("create-session-home");
        let project = temp_dir("create-project");
        initialize_git_repository(&project);
        let host = LocalCoreHost::with_session_home(&session_home);
        let mut adapter = open_workspace_with_host(&host, &project).expect("open local workspace");

        let preview = adapter
            .send_d1_intent_and_wait(
                "live-create-preview",
                crate::D1Intent::PreviewDefaultLane {
                    preset: "coder".to_string(),
                },
                Duration::from_millis(250),
            )
            .expect("preview default Lane through live Core")
            .projection
            .starter_lane_previews
            .into_iter()
            .next()
            .expect("Core publishes a starter Lane preview");

        let create = adapter
            .send_d1_intent_and_wait(
                "live-create-lane",
                crate::D1Intent::CreateStarterLane {
                    lane_id: preview.lane_id.clone(),
                    preset: "coder".to_string(),
                    branch: preview.branch,
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256,
                },
                Duration::from_millis(250),
            )
            .expect("request starter Lane creation through live Core");
        assert!(
            create.projection.permission_dock.request.is_some(),
            "the exact pending Lane owner approval must be visible before Lane registration"
        );
        let (exact_owner, exact_kind) = adapter
            .pending_d1
            .as_ref()
            .map(|pending| (pending.owner.clone(), pending.kind.clone()))
            .expect("starter Lane creation remains pending approval");
        adapter
            .pending_d1
            .as_mut()
            .expect("pending starter creation")
            .owner = RuntimeOwner {
            lane_id: Some("different-lane-owner".to_string()),
            ..RuntimeOwner::default()
        };
        assert!(
            adapter
                .d1_cockpit(None)
                .expect("D1 projection for mismatched owner")
                .permission_dock
                .request
                .is_none(),
            "another owner must never inherit the pending creation approval"
        );
        {
            let pending = adapter
                .pending_d1
                .as_mut()
                .expect("pending starter creation");
            pending.owner = exact_owner;
            pending.kind = PendingD1Kind::QueryAgentAdapters;
        }
        assert!(
            adapter
                .d1_cockpit(None)
                .expect("D1 projection for non-create command")
                .permission_dock
                .request
                .is_none(),
            "non-create pending commands must not expose an unselected Lane approval"
        );
        adapter
            .pending_d1
            .as_mut()
            .expect("pending starter creation")
            .kind = exact_kind;
        let request_id = adapter
            .permission_dock()
            .expect("global permission projection")
            .request
            .expect("Core requests approval before creating a worktree")
            .id;

        adapter
            .send_permission_intent_and_wait(
                "live-create-allow-once",
                PermissionIntent::Respond {
                    request_id,
                    choice: PermissionChoice::Once,
                    feedback: None,
                },
                Duration::from_millis(250),
            )
            .expect("allow starter Lane creation once");
        let created = adapter
            .poll_d1(None, Duration::from_millis(250))
            .expect("observe starter Lane creation through live Core");

        assert_eq!(created.pending_command_id, None);
        assert_eq!(created.outcome.state, "confirmed");
        assert!(
            created
                .projection
                .lanes
                .iter()
                .any(|lane| lane.id == preview.lane_id)
        );
        assert!(
            created
                .projection
                .starter_lane_receipts
                .iter()
                .any(|receipt| receipt.preview_id == preview.preview_id)
        );

        drop(adapter);
        drop(host);
        std::fs::remove_dir_all(session_home).expect("remove temporary session home");
        std::fs::remove_dir_all(project).expect("remove temporary project");
    }

    #[test]
    fn local_workspace_adapter_reaches_live_core_and_can_probe_project() {
        let session_home = temp_dir("session-home");
        let project = temp_dir("project");
        let canonical_project = project.canonicalize().expect("canonical project path");
        let host = LocalCoreHost::with_session_home(&session_home);

        let mut adapter = open_workspace_with_host(&host, &project).expect("open local workspace");

        assert_eq!(adapter.d6_recovery().connection, D6ConnectionState::Live);
        assert!(adapter.supports("runtime.project_onboarding"));
        let initial = adapter
            .poll_d11(Duration::ZERO)
            .expect("poll initial D11 state");
        assert_eq!(initial.pending_command_id, None);
        assert_eq!(initial.pending_intent, None);

        let result = adapter
            .send_d11_intent_and_wait(
                "local-host-project-probe",
                D11Intent::ProbeProject,
                Duration::from_millis(250),
            )
            .expect("probe bound project through Core");
        assert_eq!(result.pending_command_id, None);
        assert_eq!(result.pending_intent, None);
        assert_eq!(
            result.projection.project.expect("Core project probe").root,
            canonical_project.to_string_lossy()
        );

        let preview = adapter
            .send_d11_intent_and_wait(
                "local-host-config-preview",
                D11Intent::PreviewProjectConfig {
                    contents: "[project]\nname = \"project\"\npack = \"rust\"\n".to_string(),
                },
                Duration::from_millis(250),
            )
            .expect("preview project config through Core");
        assert_eq!(preview.pending_command_id, None);
        assert_eq!(preview.pending_intent, None);
        assert!(
            preview
                .projection
                .preview
                .expect("Core config preview")
                .valid
        );

        let confirm = adapter
            .send_d11_intent_and_wait(
                "local-host-config-confirm",
                D11Intent::ConfirmProjectConfig,
                Duration::from_millis(250),
            )
            .expect("confirm project config through Core");
        let confirm = if confirm.projection.pending_approval.is_some() {
            confirm
        } else {
            adapter
                .poll_d11(Duration::from_millis(250))
                .expect("poll project config approval")
        };
        let request_id = confirm
            .projection
            .pending_approval
            .expect("Core project config approval")
            .id;
        adapter
            .send_permission_intent_and_wait(
                "local-host-config-allow",
                PermissionIntent::Respond {
                    request_id,
                    choice: PermissionChoice::Once,
                    feedback: None,
                },
                Duration::from_millis(250),
            )
            .expect("allow project config through Core");
        let confirmed = adapter
            .poll_d11(Duration::from_millis(250))
            .expect("poll confirmed project config");
        assert_eq!(confirmed.pending_command_id, None);
        assert_eq!(confirmed.pending_intent, None);
        assert!(confirmed.projection.confirmed_config.is_some());

        drop(adapter);
        drop(host);
        std::fs::remove_dir_all(session_home).expect("remove temporary session home");
        std::fs::remove_dir_all(project).expect("remove temporary project");
    }
}

/// Parses the opaque `stream:sequence` audit cursor the client handed out.
fn parse_audit_cursor(raw: &str) -> Result<viden_core::EventCursor, String> {
    let (stream_id, sequence) = raw
        .rsplit_once(':')
        .ok_or_else(|| format!("malformed audit cursor `{raw}`"))?;
    Ok(viden_core::EventCursor {
        stream_id: stream_id.to_string(),
        sequence: sequence
            .parse()
            .map_err(|_| format!("malformed audit cursor `{raw}`"))?,
    })
}

/// Reads the canonical Core discriminant for an event kind.
///
/// Core owns the event vocabulary; serializing the tag keeps the client from
/// mirroring a 117-variant enum that would drift on the next Core change.
fn core_event_kind(kind: &viden_core::RuntimeEventKind) -> String {
    match serde_json::to_value(kind) {
        Ok(serde_json::Value::Object(map)) => map
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string()),
        _ => "unknown".to_string(),
    }
}
