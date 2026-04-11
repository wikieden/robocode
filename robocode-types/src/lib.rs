use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub type SessionId = String;
pub type MessageId = String;
pub type ToolCallId = String;
pub type ToolInput = BTreeMap<String, String>;

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
pub enum ModelEvent {
    AssistantText { content: String },
    ToolCall(ToolCall),
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
pub struct PermissionLogEntry {
    pub timestamp: u64,
    pub tool_name: String,
    pub decision: String,
    pub reason: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLogEntry {
    pub timestamp: u64,
    pub name: String,
    pub args: Vec<String>,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetaEntry {
    pub timestamp: u64,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptEntry {
    Message { message: Message },
    ToolCall { call: ToolCall },
    ToolResult { result: ToolResult },
    Permission { entry: PermissionLogEntry },
    Command { entry: CommandLogEntry },
    SessionMeta { entry: SessionMetaEntry },
}

impl TranscriptEntry {
    pub fn to_json_line(&self) -> String {
        match self {
            TranscriptEntry::Message { message } => format!(
                "{{\"type\":\"message\",\"id\":\"{}\",\"role\":\"{}\",\"content\":\"{}\",\"timestamp\":{},\"tool_name\":{},\"tool_call_id\":{}}}",
                escape_json(&message.id),
                message.role.as_str(),
                escape_json(&message.content),
                message.timestamp,
                optional_json_string(message.tool_name.as_deref()),
                optional_json_string(message.tool_call_id.as_deref())
            ),
            TranscriptEntry::ToolCall { call } => format!(
                "{{\"type\":\"tool_call\",\"id\":\"{}\",\"name\":\"{}\",\"input\":\"{}\"}}",
                escape_json(&call.id),
                escape_json(&call.name),
                escape_json(&encode_tool_input(&call.input))
            ),
            TranscriptEntry::ToolResult { result } => format!(
                "{{\"type\":\"tool_result\",\"tool_call_id\":\"{}\",\"name\":\"{}\",\"output\":\"{}\",\"diff\":{},\"success\":{}}}",
                escape_json(&result.tool_call_id),
                escape_json(&result.name),
                escape_json(&result.output),
                optional_json_string(result.diff.as_deref()),
                if result.success { "true" } else { "false" }
            ),
            TranscriptEntry::Permission { entry } => format!(
                "{{\"type\":\"permission\",\"timestamp\":{},\"tool_name\":\"{}\",\"decision\":\"{}\",\"reason\":\"{}\",\"message\":{}}}",
                entry.timestamp,
                escape_json(&entry.tool_name),
                escape_json(&entry.decision),
                escape_json(&entry.reason),
                optional_json_string(entry.message.as_deref())
            ),
            TranscriptEntry::Command { entry } => format!(
                "{{\"type\":\"command\",\"timestamp\":{},\"name\":\"{}\",\"args\":\"{}\",\"output\":\"{}\"}}",
                entry.timestamp,
                escape_json(&entry.name),
                escape_json(&entry.args.join("\t")),
                escape_json(&entry.output)
            ),
            TranscriptEntry::SessionMeta { entry } => format!(
                "{{\"type\":\"session_meta\",\"timestamp\":{},\"key\":\"{}\",\"value\":\"{}\"}}",
                entry.timestamp,
                escape_json(&entry.key),
                escape_json(&entry.value)
            ),
        }
    }

    pub fn from_json_line(line: &str) -> Result<Self, String> {
        let kind = extract_string_field(line, "type")?;
        match kind.as_str() {
            "message" => Ok(TranscriptEntry::Message {
                message: Message {
                    id: extract_string_field(line, "id")?,
                    role: Role::parse(&extract_string_field(line, "role")?)
                        .ok_or_else(|| "Unknown role".to_string())?,
                    content: extract_string_field(line, "content")?,
                    timestamp: extract_u64_field(line, "timestamp")?,
                    tool_name: extract_optional_string_field(line, "tool_name")?,
                    tool_call_id: extract_optional_string_field(line, "tool_call_id")?,
                },
            }),
            "tool_call" => Ok(TranscriptEntry::ToolCall {
                call: ToolCall {
                    id: extract_string_field(line, "id")?,
                    name: extract_string_field(line, "name")?,
                    input: decode_tool_input(&extract_string_field(line, "input")?),
                },
            }),
            "tool_result" => Ok(TranscriptEntry::ToolResult {
                result: ToolResult {
                    tool_call_id: extract_string_field(line, "tool_call_id")?,
                    name: extract_string_field(line, "name")?,
                    output: extract_string_field(line, "output")?,
                    diff: extract_optional_string_field(line, "diff")?,
                    success: extract_bool_field(line, "success")?,
                },
            }),
            "permission" => Ok(TranscriptEntry::Permission {
                entry: PermissionLogEntry {
                    timestamp: extract_u64_field(line, "timestamp")?,
                    tool_name: extract_string_field(line, "tool_name")?,
                    decision: extract_string_field(line, "decision")?,
                    reason: extract_string_field(line, "reason")?,
                    message: extract_optional_string_field(line, "message")?,
                },
            }),
            "command" => Ok(TranscriptEntry::Command {
                entry: CommandLogEntry {
                    timestamp: extract_u64_field(line, "timestamp")?,
                    name: extract_string_field(line, "name")?,
                    args: extract_string_field(line, "args")?
                        .split('\t')
                        .filter(|part| !part.is_empty())
                        .map(ToString::to_string)
                        .collect(),
                    output: extract_string_field(line, "output")?,
                },
            }),
            "session_meta" => Ok(TranscriptEntry::SessionMeta {
                entry: SessionMetaEntry {
                    timestamp: extract_u64_field(line, "timestamp")?,
                    key: extract_string_field(line, "key")?,
                    value: extract_string_field(line, "value")?,
                },
            }),
            _ => Err("Unknown transcript entry type".to_string()),
        }
    }
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

fn optional_json_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn escape_json(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn extract_string_field(line: &str, field: &str) -> Result<String, String> {
    let marker = format!("\"{field}\":\"");
    let start = line
        .find(&marker)
        .ok_or_else(|| format!("Missing field `{field}`"))?
        + marker.len();
    parse_json_string_from(line, start)
}

fn extract_optional_string_field(line: &str, field: &str) -> Result<Option<String>, String> {
    let marker = format!("\"{field}\":");
    let start = line
        .find(&marker)
        .ok_or_else(|| format!("Missing field `{field}`"))?
        + marker.len();
    if line[start..].starts_with("null") {
        Ok(None)
    } else if line[start..].starts_with('"') {
        Ok(Some(parse_json_string_from(line, start + 1)?))
    } else {
        Err(format!("Invalid optional string field `{field}`"))
    }
}

fn parse_json_string_from(line: &str, start: usize) -> Result<String, String> {
    let bytes = line.as_bytes();
    let mut index = start;
    let mut escaped = false;
    let mut out = String::new();
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Ok(out);
        } else {
            out.push(ch);
        }
        index += 1;
    }
    Err("Unterminated JSON string".to_string())
}

fn extract_u64_field(line: &str, field: &str) -> Result<u64, String> {
    let marker = format!("\"{field}\":");
    let start = line
        .find(&marker)
        .ok_or_else(|| format!("Missing field `{field}`"))?
        + marker.len();
    let tail = &line[start..];
    let end = tail.find([',', '}']).unwrap_or(tail.len());
    tail[..end]
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("Invalid number in `{field}`"))
}

fn extract_bool_field(line: &str, field: &str) -> Result<bool, String> {
    let marker = format!("\"{field}\":");
    let start = line
        .find(&marker)
        .ok_or_else(|| format!("Missing field `{field}`"))?
        + marker.len();
    let tail = &line[start..];
    if tail.starts_with("true") {
        Ok(true)
    } else if tail.starts_with("false") {
        Ok(false)
    } else {
        Err(format!("Invalid bool in `{field}`"))
    }
}
