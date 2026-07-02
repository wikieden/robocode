use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod lsp;
mod runtime;
mod transcript;
mod workflow;

pub use lsp::{LspDiagnostic, LspLocation, LspPosition, LspRange, LspSymbol};
pub use runtime::{
    ApprovalRequestView, CommandAction, EvidenceView, ProviderHealthView, QueuedInputView,
    RuntimeCommand, RuntimeCommandReceipt, RuntimeErrorView, RuntimeEvent, RuntimeEventKind,
    RuntimeViewState, TokenCostView, ToolCallView,
};
pub use transcript::{CommandLogEntry, PermissionLogEntry, SessionMetaEntry, TranscriptEntry};
pub use workflow::{
    MemoryEntry, MemoryKind, MemoryScope, MemorySource, MemoryStatus, ResumeContextSnapshot,
    TaskPriority, TaskRecord, TaskStatus,
};

pub type SessionId = String;
pub type MessageId = String;
pub type ToolCallId = String;
pub type ToolInput = BTreeMap<String, String>;
pub type TaskId = String;
pub type MemoryId = String;
pub type LspServerId = String;
pub type AgentTaskId = String;
pub type AgentLaneId = String;

pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn fresh_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{nanos}")
}

pub fn truncate_for_preview(input: &str, max_chars: usize) -> String {
    let mut collected = String::new();
    for ch in input.chars().take(max_chars) {
        collected.push(ch);
    }
    if input.chars().count() > max_chars {
        collected.push_str("...");
    }
    collected
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Queued,
    Thinking,
    Streaming,
    Editing,
    RunningTool,
    Testing,
    WaitingApproval,
    NeedsInput,
    Blocked,
    Reviewing,
    Running,
    Attached,
    Done,
    Applied,
    Discarded,
    Failed,
    Cancelled,
    Archived,
}

impl AgentTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Thinking => "thinking",
            Self::Streaming => "streaming",
            Self::Editing => "editing",
            Self::RunningTool => "running_tool",
            Self::Testing => "testing",
            Self::WaitingApproval => "waiting_approval",
            Self::NeedsInput => "needs_input",
            Self::Blocked => "blocked",
            Self::Reviewing => "reviewing",
            Self::Running => "running",
            Self::Attached => "attached",
            Self::Done => "done",
            Self::Applied => "applied",
            Self::Discarded => "discarded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Archived => "archived",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input.trim() {
            "queued" => Some(Self::Queued),
            "thinking" | "starting" => Some(Self::Thinking),
            "streaming" => Some(Self::Streaming),
            "editing" => Some(Self::Editing),
            "running_tool" | "tool" => Some(Self::RunningTool),
            "testing" => Some(Self::Testing),
            "waiting_approval" | "approval" => Some(Self::WaitingApproval),
            "needs_input" | "input" => Some(Self::NeedsInput),
            "blocked" | "apply_conflict" => Some(Self::Blocked),
            "reviewing" => Some(Self::Reviewing),
            "running" => Some(Self::Running),
            "attached" => Some(Self::Attached),
            "done" | "completed" | "accepted" => Some(Self::Done),
            "applied" => Some(Self::Applied),
            "discarded" | "detached" | "stopped" => Some(Self::Discarded),
            "failed" => Some(Self::Failed),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued
                | Self::Thinking
                | Self::Streaming
                | Self::Editing
                | Self::RunningTool
                | Self::Testing
                | Self::WaitingApproval
                | Self::NeedsInput
                | Self::Blocked
                | Self::Reviewing
                | Self::Running
                | Self::Attached
        )
    }

    pub fn priority(self) -> u8 {
        match self {
            Self::WaitingApproval => 100,
            Self::Blocked => 95,
            Self::NeedsInput => 90,
            Self::Reviewing => 85,
            Self::Testing => 80,
            Self::Editing => 70,
            Self::RunningTool | Self::Running | Self::Attached => 60,
            Self::Thinking | Self::Streaming => 50,
            Self::Queued => 40,
            Self::Failed => 30,
            Self::Done | Self::Applied | Self::Discarded => 20,
            Self::Cancelled => 10,
            Self::Archived => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTaskEvidence {
    pub kind: String,
    pub summary: String,
    pub path: Option<String>,
    pub timestamp: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentNextAction {
    pub label: String,
    pub command: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTaskRecord {
    pub id: AgentTaskId,
    pub parent_id: Option<AgentTaskId>,
    pub agent: String,
    pub kind: String,
    pub transport: String,
    pub title: String,
    pub status: String,
    pub activity: String,
    pub summary: String,
    pub progress: u8,
    pub started_at: Option<u128>,
    pub updated_at: Option<u128>,
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
        AgentTaskStatus::parse(&self.status).unwrap_or(AgentTaskStatus::Done)
    }

    pub fn is_active(&self) -> bool {
        self.status_kind().is_active()
    }

    pub fn priority(&self) -> u8 {
        self.status_kind().priority()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentLaneRecord {
    pub id: AgentLaneId,
    pub task_id: AgentTaskId,
    pub agent: String,
    pub screen: String,
    pub transport: String,
    pub status: String,
    pub summary: String,
    pub evidence: Vec<String>,
}

impl AgentLaneRecord {
    pub fn is_active(&self) -> bool {
        AgentTaskStatus::parse(&self.status)
            .map(AgentTaskStatus::is_active)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentLaneEventRecord {
    pub lane_id: AgentLaneId,
    pub sequence: u64,
    pub timestamp: Option<u128>,
    pub kind: String,
    pub summary: String,
    pub detail: Option<String>,
    pub evidence_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentLaneIsolationRecord {
    pub lane_id: AgentLaneId,
    pub workspace: String,
    pub worktree: Option<String>,
    pub writable_scope: String,
    pub env_vars: Vec<String>,
    pub cache_dirs: Vec<String>,
    pub database_scope: Option<String>,
    pub service_ports: Vec<String>,
    pub setup_command: Option<String>,
    pub verification_command: Option<String>,
    pub cleanup_command: Option<String>,
    pub risk_level: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentCapabilityRecord {
    pub id: String,
    pub display_name: String,
    pub transport: String,
    pub readiness: String,
    pub entrypoint: String,
    pub mutation_mode: String,
    pub evidence_mode: String,
    pub config_source: Option<String>,
    pub known_limits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextSourceRecord {
    pub name: String,
    pub kind: String,
    pub priority: u8,
    pub estimated_tokens: u64,
    pub summary: String,
    pub include_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextOmittedSourceRecord {
    pub name: String,
    pub kind: String,
    pub estimated_tokens: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextBundleRecord {
    pub bundle_id: String,
    pub task_id: AgentTaskId,
    pub policy: String,
    pub sources: Vec<ContextSourceRecord>,
    pub omitted_sources: Vec<ContextOmittedSourceRecord>,
    pub estimated_tokens: u64,
    pub largest_sources: Vec<String>,
    pub compaction_notes: Vec<String>,
    pub soft_token_budget: u64,
    pub hard_token_limit: u64,
}

impl ContextBundleRecord {
    pub fn pressure_percent(&self) -> u8 {
        if self.hard_token_limit == 0 {
            return 0;
        }
        let percent = self
            .estimated_tokens
            .saturating_mul(100)
            .saturating_div(self.hard_token_limit);
        percent.min(100) as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            "system" => Some(Self::System),
            "tool" => Some(Self::Tool),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub content: String,
    pub timestamp: u64,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<ToolCallId>,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            id: fresh_id("msg"),
            role,
            content: content.into(),
            timestamp: now_timestamp(),
            tool_name: None,
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
    DontAsk,
    Plan,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkMode {
    Plan,
    #[default]
    Build,
    Review,
    Explore,
}

impl WorkMode {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "plan" => Some(Self::Plan),
            "build" | "code" | "coding" => Some(Self::Build),
            "review" => Some(Self::Review),
            "explore" | "inspect" => Some(Self::Explore),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Build => "build",
            Self::Review => "review",
            Self::Explore => "explore",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::Build => "Build",
            Self::Review => "Review",
            Self::Explore => "Explore",
        }
    }
}

impl Display for WorkMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    #[default]
    Ask,
    AutoEdit,
    Auto,
    ReadOnly,
    FullAccess,
}

impl PermissionLevel {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "ask" | "suggest" | "default" => Some(Self::Ask),
            "auto_edit" | "auto-edit" | "acceptEdits" | "accept_edits" => Some(Self::AutoEdit),
            "auto" => Some(Self::Auto),
            "read_only" | "read-only" | "readonly" | "plan" => Some(Self::ReadOnly),
            "full_access" | "full-access" | "full" | "bypassPermissions" | "bypass_permissions" => {
                Some(Self::FullAccess)
            }
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::AutoEdit => "auto_edit",
            Self::Auto => "auto",
            Self::ReadOnly => "read_only",
            Self::FullAccess => "full_access",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::AutoEdit => "Auto Edit",
            Self::Auto => "Auto",
            Self::ReadOnly => "Read Only",
            Self::FullAccess => "Full Access",
        }
    }

    pub fn from_legacy_mode(mode: PermissionMode) -> Self {
        match mode {
            PermissionMode::Default => Self::Ask,
            PermissionMode::AcceptEdits => Self::AutoEdit,
            PermissionMode::BypassPermissions | PermissionMode::DontAsk => Self::FullAccess,
            PermissionMode::Plan => Self::ReadOnly,
        }
    }
}

impl Display for PermissionLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

impl PermissionMode {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "default" | "ask" | "suggest" => Some(Self::Default),
            "acceptEdits" | "accept_edits" | "auto_edit" | "auto-edit" => Some(Self::AcceptEdits),
            "bypassPermissions" | "bypass_permissions" | "full_access" | "full-access" | "full" => {
                Some(Self::BypassPermissions)
            }
            "dontAsk" | "dont_ask" => Some(Self::DontAsk),
            "plan" | "read_only" | "read-only" | "readonly" => Some(Self::Plan),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
            Self::DontAsk => "dontAsk",
            Self::Plan => "plan",
        }
    }
}

impl Display for PermissionMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRuleSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    CliArg,
    Command,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRuleValue {
    pub tool_name: String,
    pub rule_content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRule {
    pub source: PermissionRuleSource,
    pub rule_behavior: PermissionBehavior,
    pub rule_value: PermissionRuleValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalWorkingDirectory {
    pub path: String,
    pub source: PermissionRuleSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecisionReason {
    RuleAllow,
    RuleDeny,
    RuleAsk,
    SafeRead,
    RequiresApproval,
    OutOfScopePath,
    BypassMode,
    DontAskMode,
    PlanMode,
    AcceptEditsMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionAllowDecision {
    pub updated_input: Option<ToolInput>,
    pub user_modified: bool,
    pub decision_reason: Option<PermissionDecisionReason>,
    pub accept_feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionAskDecision {
    pub message: String,
    pub updated_input: Option<ToolInput>,
    pub decision_reason: Option<PermissionDecisionReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenyDecision {
    pub message: String,
    pub decision_reason: PermissionDecisionReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow(PermissionAllowDecision),
    Ask(PermissionAskDecision),
    Deny(PermissionDenyDecision),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub is_mutating: bool,
    pub input_schema_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub input: ToolInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub tool_call_id: ToolCallId,
    pub name: String,
    pub output: String,
    pub diff: Option<String>,
    pub success: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProgress {
    pub tool_call_id: ToolCallId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost_micro_usd: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelEvent {
    AssistantText { content: String },
    ToolCall(ToolCall),
    Usage(ModelUsage),
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    pub session_id: SessionId,
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub work_mode: WorkMode,
    pub permission_mode: PermissionMode,
    pub permission_level: PermissionLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub cwd: String,
    pub transcript_path: String,
    pub title: Option<String>,
    pub last_preview: Option<String>,
    pub message_count: usize,
    pub tool_call_count: usize,
    pub command_count: usize,
    pub last_activity_kind: Option<String>,
    pub last_activity_preview: Option<String>,
    pub created_at: u64,
    pub last_updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeSnapshot {
    pub cwd: PathBuf,
    pub provider_family: String,
    pub model_label: String,
    pub work_mode: WorkMode,
    pub permission_mode: PermissionMode,
    pub permission_level: PermissionLevel,
    pub config_summary: String,
    pub loaded_config_files: Vec<PathBuf>,
    pub startup_overrides: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPrompt {
    pub tool_name: String,
    pub message: String,
    pub input_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApprovalResponse {
    pub approved: bool,
    pub feedback: Option<String>,
}

pub fn parse_tool_input(input: &str) -> ToolInput {
    let mut out = BTreeMap::new();
    for segment in input.split_whitespace() {
        if let Some((key, value)) = segment.split_once('=') {
            let cleaned = value.trim_matches('"').trim_matches('\'').to_string();
            out.insert(key.to_string(), cleaned);
        }
    }
    out
}

pub fn encode_tool_input(input: &ToolInput) -> String {
    input
        .iter()
        .map(|(key, value)| format!("{key}={}", value.replace('\t', "\\t")))
        .collect::<Vec<_>>()
        .join("\t")
}

pub fn decode_tool_input(input: &str) -> ToolInput {
    let mut out = BTreeMap::new();
    for part in input.split('\t').filter(|part| !part.is_empty()) {
        if let Some((key, value)) = part.split_once('=') {
            out.insert(key.to_string(), value.replace("\\t", "\t"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
