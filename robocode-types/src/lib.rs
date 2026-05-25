use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod lsp;
mod transcript;
mod workflow;

pub use lsp::{LspDiagnostic, LspLocation, LspPosition, LspRange, LspSymbol};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    DontAsk,
    Plan,
}

impl PermissionMode {
    pub fn parse_cli(input: &str) -> Option<Self> {
        match input.trim() {
            "default" => Some(Self::Default),
            "acceptEdits" | "accept_edits" => Some(Self::AcceptEdits),
            "bypassPermissions" | "bypass_permissions" => Some(Self::BypassPermissions),
            "dontAsk" | "dont_ask" => Some(Self::DontAsk),
            "plan" => Some(Self::Plan),
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

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
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
    pub permission_mode: PermissionMode,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub cwd: PathBuf,
    pub provider_family: String,
    pub model_label: String,
    pub permission_mode: PermissionMode,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
