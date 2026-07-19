use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{AgentLaneId, AgentNextAction, AgentTaskId, AgentTaskStatus, EvidenceId, SessionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Planner,
    Coder,
    Reviewer,
    Tester,
    DocWriter,
    Researcher,
    ReleaseOperator,
}

impl AgentRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::Coder => "coder",
            Self::Reviewer => "reviewer",
            Self::Tester => "tester",
            Self::DocWriter => "doc_writer",
            Self::Researcher => "researcher",
            Self::ReleaseOperator => "release_operator",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().replace('-', "_").as_str() {
            "planner" => Some(Self::Planner),
            "coder" => Some(Self::Coder),
            "reviewer" => Some(Self::Reviewer),
            "tester" => Some(Self::Tester),
            "doc_writer" | "documenter" => Some(Self::DocWriter),
            "researcher" => Some(Self::Researcher),
            "release_operator" | "release" => Some(Self::ReleaseOperator),
            _ => None,
        }
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRoute {
    BuiltIn,
    Acp,
    Terminal,
    Tmux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStrength {
    Full,
    Cooperative,
    Containment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationPolicy {
    Autonomous,
    ProposeOnly,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneStatus {
    Draft,
    Queued,
    Starting,
    Running,
    WaitingApproval,
    NeedsInput,
    #[serde(alias = "apply_conflict")]
    Blocked,
    Attached,
    #[serde(alias = "stopped")]
    Detached,
    #[serde(alias = "completed", alias = "accepted", alias = "applied")]
    Done,
    Failed,
    #[serde(alias = "discarded", alias = "canceled")]
    Cancelled,
    Archived,
}

impl LaneStatus {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Draft
                | Self::Queued
                | Self::Starting
                | Self::Running
                | Self::WaitingApproval
                | Self::NeedsInput
                | Self::Blocked
                | Self::Attached
                | Self::Detached
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskKind {
    Provider,
    Tool,
    Shell,
    Test,
    Job,
    Agent,
}

impl std::fmt::Display for AgentTaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Provider => "provider",
            Self::Tool => "tool",
            Self::Shell => "shell",
            Self::Test => "test",
            Self::Job => "job",
            Self::Agent => "agent",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTarget {
    Local,
    Ssh { host: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataEgressPolicy {
    Deny,
    AllowProvider,
    AllowListed { domains: Vec<String> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneBudget {
    pub token_limit: Option<u64>,
    pub cost_limit_micro_usd: Option<u64>,
    pub wall_time_limit_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTaskRecord {
    pub id: AgentTaskId,
    pub parent_id: Option<AgentTaskId>,
    pub role: AgentRole,
    pub kind: AgentTaskKind,
    pub route: AgentRoute,
    pub title: String,
    pub status: AgentTaskStatus,
    pub activity: String,
    pub summary: String,
    pub progress: u8,
    pub started_at: Option<u64>,
    pub updated_at: Option<u64>,
    pub workspace: Option<String>,
    pub evidence: Vec<String>,
    pub permissions: Vec<String>,
    pub decision: Option<String>,
    pub result: Option<String>,
    pub resume_handle: Option<String>,
    pub pid: Option<u32>,
    pub next_action: Option<AgentNextAction>,
}

impl AgentTaskRecord {
    pub fn status_kind(&self) -> AgentTaskStatus {
        self.status
    }

    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub fn priority(&self) -> u8 {
        self.status.priority()
    }
}

impl Serialize for AgentTaskRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AgentTaskRecordWireRef::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentTaskRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AgentTaskRecordWire::deserialize(deserializer)?;
        let kind = legacy_task_kind(&wire.kind).map_err(serde::de::Error::custom)?;
        let role = match wire.role {
            Some(role) => role,
            None => {
                legacy_task_role(wire.agent.as_deref(), kind).map_err(serde::de::Error::custom)?
            }
        };
        let route = match wire.route {
            Some(route) => route,
            None => legacy_task_route(wire.transport.as_deref(), kind)
                .map_err(serde::de::Error::custom)?,
        };
        Ok(Self {
            id: wire.id,
            parent_id: wire.parent_id,
            role,
            kind,
            route,
            title: wire.title,
            status: wire.status,
            activity: wire.activity,
            summary: wire.summary,
            progress: wire.progress,
            started_at: wire.started_at,
            updated_at: wire.updated_at,
            workspace: wire.workspace,
            evidence: wire.evidence,
            permissions: wire.permissions,
            decision: wire.decision,
            result: wire.result,
            resume_handle: wire.resume_handle,
            pid: wire.pid,
            next_action: wire.next_action,
        })
    }
}

#[derive(Serialize)]
struct AgentTaskRecordWireRef<'a> {
    id: &'a AgentTaskId,
    parent_id: &'a Option<AgentTaskId>,
    #[serde(rename = "agent")]
    role: AgentRole,
    kind: AgentTaskKind,
    #[serde(rename = "transport")]
    route: AgentRoute,
    title: &'a str,
    status: AgentTaskStatus,
    activity: &'a str,
    summary: &'a str,
    progress: u8,
    started_at: &'a Option<u64>,
    updated_at: &'a Option<u64>,
    workspace: &'a Option<String>,
    evidence: &'a Vec<String>,
    permissions: &'a Vec<String>,
    decision: &'a Option<String>,
    result: &'a Option<String>,
    resume_handle: &'a Option<String>,
    pid: &'a Option<u32>,
    next_action: &'a Option<AgentNextAction>,
}

impl<'a> From<&'a AgentTaskRecord> for AgentTaskRecordWireRef<'a> {
    fn from(task: &'a AgentTaskRecord) -> Self {
        Self {
            id: &task.id,
            parent_id: &task.parent_id,
            role: task.role,
            kind: task.kind,
            route: task.route,
            title: &task.title,
            status: task.status,
            activity: &task.activity,
            summary: &task.summary,
            progress: task.progress,
            started_at: &task.started_at,
            updated_at: &task.updated_at,
            workspace: &task.workspace,
            evidence: &task.evidence,
            permissions: &task.permissions,
            decision: &task.decision,
            result: &task.result,
            resume_handle: &task.resume_handle,
            pid: &task.pid,
            next_action: &task.next_action,
        }
    }
}

#[derive(Deserialize)]
struct AgentTaskRecordWire {
    id: AgentTaskId,
    parent_id: Option<AgentTaskId>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    role: Option<AgentRole>,
    kind: String,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    route: Option<AgentRoute>,
    title: String,
    status: AgentTaskStatus,
    activity: String,
    summary: String,
    progress: u8,
    started_at: Option<u64>,
    updated_at: Option<u64>,
    workspace: Option<String>,
    evidence: Vec<String>,
    permissions: Vec<String>,
    decision: Option<String>,
    result: Option<String>,
    resume_handle: Option<String>,
    pid: Option<u32>,
    next_action: Option<AgentNextAction>,
}

// Legacy task names identify implementations, not roles. The migration keeps
// that ambiguity at the input boundary so reducers only see the typed role.
fn legacy_task_role(value: Option<&str>, kind: AgentTaskKind) -> Result<AgentRole, String> {
    let value = value.ok_or_else(|| "legacy task is missing agent/role".to_string())?;
    if let Some(role) = AgentRole::parse(value) {
        return Ok(role);
    }
    match value.trim() {
        "viden" | "codex" | "claude" | "shell" | "git" | "acp" => {
            if kind == AgentTaskKind::Test {
                Ok(AgentRole::Tester)
            } else {
                Ok(AgentRole::Coder)
            }
        }
        "external" => {
            Err("legacy task agent `external` is not a role; use route/source".to_string())
        }
        other => Err(format!("unknown legacy task agent `{other}`")),
    }
}

fn legacy_task_route(value: Option<&str>, kind: AgentTaskKind) -> Result<AgentRoute, String> {
    let value = value.ok_or_else(|| "legacy task is missing transport/route".to_string())?;
    let normalized = value.trim().to_ascii_lowercase();
    if kind == AgentTaskKind::Provider || matches!(normalized.as_str(), "runtime" | "core") {
        return Ok(AgentRoute::BuiltIn);
    }
    match normalized.as_str() {
        "built_in" => Ok(AgentRoute::BuiltIn),
        "acp" | "acp-session" => Ok(AgentRoute::Acp),
        "tmux" => Ok(AgentRoute::Tmux),
        "shell" | "local" | "pty" | "terminal" | "app-server" => Ok(AgentRoute::Terminal),
        value if value.starts_with("codex") || value.starts_with("claude") => {
            Ok(AgentRoute::Terminal)
        }
        other => Err(format!("unknown legacy task transport `{other}`")),
    }
}

// `runtime` and `lane` were v0 projection labels. They migrate to the single
// v1 supervised-agent kind; unrecognized labels still fail at the input edge.
fn legacy_task_kind(value: &str) -> Result<AgentTaskKind, String> {
    match value.trim() {
        "provider" => Ok(AgentTaskKind::Provider),
        "tool" => Ok(AgentTaskKind::Tool),
        "shell" => Ok(AgentTaskKind::Shell),
        "test" => Ok(AgentTaskKind::Test),
        "job" => Ok(AgentTaskKind::Job),
        "agent" | "runtime" | "lane" => Ok(AgentTaskKind::Agent),
        other => Err(format!("unknown task kind `{other}`")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLaneRecord {
    pub id: AgentLaneId,
    pub task_id: Option<AgentTaskId>,
    pub role: AgentRole,
    pub route: AgentRoute,
    pub gate_strength: GateStrength,
    pub mutation_policy: MutationPolicy,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub target: ExecutionTarget,
    pub data_egress: DataEgressPolicy,
    pub status: LaneStatus,
    pub budget: LaneBudget,
    pub active_session_ids: Vec<SessionId>,
    pub summary: String,
    pub evidence: Vec<EvidenceId>,
}

impl AgentLaneRecord {
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }
}

impl Serialize for AgentLaneRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AgentLaneRecordWireRef::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentLaneRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("role").is_some() || value.get("route").is_some() {
            let typed =
                AgentLaneRecordWire::deserialize(value).map_err(serde::de::Error::custom)?;
            return Ok(typed.into());
        }
        let legacy = LegacyAgentLaneRecord::deserialize(value).map_err(serde::de::Error::custom)?;
        legacy.try_into().map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize)]
struct AgentLaneRecordWireRef<'a> {
    id: &'a AgentLaneId,
    task_id: &'a Option<AgentTaskId>,
    role: AgentRole,
    route: AgentRoute,
    gate_strength: GateStrength,
    mutation_policy: MutationPolicy,
    worktree: &'a Option<String>,
    branch: &'a Option<String>,
    target: &'a ExecutionTarget,
    data_egress: &'a DataEgressPolicy,
    status: LaneStatus,
    budget: &'a LaneBudget,
    active_session_ids: &'a Vec<SessionId>,
    summary: &'a str,
    evidence: &'a Vec<EvidenceId>,
}

impl<'a> From<&'a AgentLaneRecord> for AgentLaneRecordWireRef<'a> {
    fn from(lane: &'a AgentLaneRecord) -> Self {
        Self {
            id: &lane.id,
            task_id: &lane.task_id,
            role: lane.role,
            route: lane.route,
            gate_strength: lane.gate_strength,
            mutation_policy: lane.mutation_policy,
            worktree: &lane.worktree,
            branch: &lane.branch,
            target: &lane.target,
            data_egress: &lane.data_egress,
            status: lane.status,
            budget: &lane.budget,
            active_session_ids: &lane.active_session_ids,
            summary: &lane.summary,
            evidence: &lane.evidence,
        }
    }
}

#[derive(Deserialize)]
struct AgentLaneRecordWire {
    id: AgentLaneId,
    task_id: Option<AgentTaskId>,
    role: AgentRole,
    route: AgentRoute,
    gate_strength: GateStrength,
    mutation_policy: MutationPolicy,
    worktree: Option<String>,
    branch: Option<String>,
    target: ExecutionTarget,
    data_egress: DataEgressPolicy,
    status: LaneStatus,
    budget: LaneBudget,
    active_session_ids: Vec<SessionId>,
    summary: String,
    evidence: Vec<EvidenceId>,
}

impl From<AgentLaneRecordWire> for AgentLaneRecord {
    fn from(lane: AgentLaneRecordWire) -> Self {
        Self {
            id: lane.id,
            task_id: lane.task_id,
            role: lane.role,
            route: lane.route,
            gate_strength: lane.gate_strength,
            mutation_policy: lane.mutation_policy,
            worktree: lane.worktree,
            branch: lane.branch,
            target: lane.target,
            data_egress: lane.data_egress,
            status: lane.status,
            budget: lane.budget,
            active_session_ids: lane.active_session_ids,
            summary: lane.summary,
            evidence: lane.evidence,
        }
    }
}

#[derive(Deserialize)]
struct LegacyAgentLaneRecord {
    id: AgentLaneId,
    task_id: AgentTaskId,
    agent: String,
    #[allow(dead_code)]
    screen: String,
    transport: String,
    status: String,
    summary: String,
    evidence: Vec<EvidenceId>,
}

impl TryFrom<LegacyAgentLaneRecord> for AgentLaneRecord {
    type Error = String;

    fn try_from(lane: LegacyAgentLaneRecord) -> Result<Self, Self::Error> {
        let route = legacy_lane_route(&lane.transport)?;
        Ok(Self {
            id: lane.id,
            task_id: Some(lane.task_id),
            role: legacy_lane_role(&lane.agent)?,
            route,
            gate_strength: default_gate_strength(route),
            mutation_policy: MutationPolicy::ProposeOnly,
            worktree: None,
            branch: None,
            target: ExecutionTarget::Local,
            data_egress: DataEgressPolicy::Deny,
            status: legacy_lane_status(&lane.status)?,
            budget: LaneBudget::default(),
            active_session_ids: Vec::new(),
            summary: lane.summary,
            evidence: lane.evidence,
        })
    }
}

pub fn legacy_lane_role(value: &str) -> Result<AgentRole, String> {
    if let Some(role) = AgentRole::parse(value) {
        return Ok(role);
    }
    match value.trim() {
        "codex" | "claude" | "shell" | "git" | "acp" | "tmux" | "viden" => Ok(AgentRole::Coder),
        "external" => {
            Err("legacy lane agent `external` is not a role; use route/source".to_string())
        }
        other => Err(format!("unknown legacy lane agent `{other}`")),
    }
}

pub fn legacy_lane_route(value: &str) -> Result<AgentRoute, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "built_in" | "runtime" | "core" => Ok(AgentRoute::BuiltIn),
        "acp" | "acp-session" => Ok(AgentRoute::Acp),
        "tmux" => Ok(AgentRoute::Tmux),
        "shell" | "local" | "pty" | "app-server" | "main" => Ok(AgentRoute::Terminal),
        value if value.starts_with("tmux") => Ok(AgentRoute::Tmux),
        value if value.starts_with("codex") || value.starts_with("claude") => {
            Ok(AgentRoute::Terminal)
        }
        other => Err(format!("unknown legacy lane transport `{other}`")),
    }
}

fn legacy_lane_status(value: &str) -> Result<LaneStatus, String> {
    if let Ok(status) = serde_json::from_value(serde_json::Value::String(value.to_string())) {
        return Ok(status);
    }
    match value.trim() {
        "thinking" => Ok(LaneStatus::Starting),
        "streaming" | "editing" | "running_tool" | "testing" => Ok(LaneStatus::Running),
        // v0's reviewing state meant the lane was waiting for an operator decision.
        "reviewing" => Ok(LaneStatus::NeedsInput),
        "manual" | "approval" => Ok(LaneStatus::WaitingApproval),
        other => Err(format!("unknown legacy lane status `{other}`")),
    }
}

pub fn default_gate_strength(route: AgentRoute) -> GateStrength {
    match route {
        AgentRoute::BuiltIn => GateStrength::Full,
        AgentRoute::Acp => GateStrength::Cooperative,
        AgentRoute::Terminal | AgentRoute::Tmux => GateStrength::Containment,
    }
}
