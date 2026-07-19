use crate::{
    CostUsageRecord, Message, Role, RuntimeEvent, SessionId, ToolCall, ToolResult,
    decode_tool_input, encode_tool_input,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PermissionLogEntry {
    pub timestamp: u64,
    pub tool_name: String,
    pub decision: String,
    pub reason: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandLogEntry {
    pub timestamp: u64,
    pub name: String,
    pub args: Vec<String>,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    CostUsage { cost: Box<CostUsageRecord> },
    RuntimeEvent { event: Box<RuntimeEvent> },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscriptRowId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscriptCursor {
    pub session_id: SessionId,
    pub ordinal: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscriptRowKind {
    Message { message: Message },
    ToolCall { call: ToolCall },
    ToolResult { result: ToolResult },
    Permission { entry: PermissionLogEntry },
    Command { entry: CommandLogEntry },
    SessionMeta { entry: SessionMetaEntry },
    CostUsage { cost: Box<CostUsageRecord> },
    RuntimeEvent { event: Box<RuntimeEvent> },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscriptRow {
    pub id: TranscriptRowId,
    pub cursor: TranscriptCursor,
    pub timestamp: Option<u64>,
    pub kind: TranscriptRowKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscriptPageRequest {
    pub session_id: SessionId,
    pub before: Option<TranscriptCursor>,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscriptPage {
    pub rows: Vec<TranscriptRow>,
    pub older: Option<TranscriptCursor>,
    pub newer: Option<TranscriptCursor>,
    pub has_more: bool,
}

impl TranscriptRow {
    pub fn from_entry(session_id: &str, ordinal: u64, entry: &TranscriptEntry) -> Self {
        let cursor = TranscriptCursor {
            session_id: session_id.to_string(),
            ordinal,
        };
        Self {
            id: TranscriptRowId(format!("{session_id}:{ordinal}")),
            cursor,
            timestamp: entry.timestamp(),
            kind: TranscriptRowKind::from(entry),
        }
    }
}

impl TranscriptEntry {
    pub fn coalescing_message_id(&self) -> Option<&str> {
        match self {
            TranscriptEntry::Message { message } if message.role == Role::Assistant => {
                Some(&message.id)
            }
            _ => None,
        }
    }

    pub fn timestamp(&self) -> Option<u64> {
        match self {
            TranscriptEntry::Message { message } => Some(message.timestamp),
            TranscriptEntry::Permission { entry } => Some(entry.timestamp),
            TranscriptEntry::Command { entry } => Some(entry.timestamp),
            TranscriptEntry::SessionMeta { entry } => Some(entry.timestamp),
            TranscriptEntry::CostUsage { cost } => cost.recorded_at,
            TranscriptEntry::RuntimeEvent { event } => event.timestamp,
            TranscriptEntry::ToolCall { .. } | TranscriptEntry::ToolResult { .. } => None,
        }
    }
}

impl From<&TranscriptEntry> for TranscriptRowKind {
    fn from(entry: &TranscriptEntry) -> Self {
        match entry {
            TranscriptEntry::Message { message } => TranscriptRowKind::Message {
                message: message.clone(),
            },
            TranscriptEntry::ToolCall { call } => {
                TranscriptRowKind::ToolCall { call: call.clone() }
            }
            TranscriptEntry::ToolResult { result } => TranscriptRowKind::ToolResult {
                result: result.clone(),
            },
            TranscriptEntry::Permission { entry } => TranscriptRowKind::Permission {
                entry: entry.clone(),
            },
            TranscriptEntry::Command { entry } => TranscriptRowKind::Command {
                entry: entry.clone(),
            },
            TranscriptEntry::SessionMeta { entry } => TranscriptRowKind::SessionMeta {
                entry: entry.clone(),
            },
            TranscriptEntry::CostUsage { cost } => TranscriptRowKind::CostUsage {
                cost: Box::new((**cost).clone()),
            },
            TranscriptEntry::RuntimeEvent { event } => TranscriptRowKind::RuntimeEvent {
                event: Box::new((**event).clone()),
            },
        }
    }
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
                "{{\"type\":\"tool_result\",\"tool_call_id\":\"{}\",\"name\":\"{}\",\"output\":\"{}\",\"diff\":{},\"success\":{},\"exit_code\":{}}}",
                escape_json(&result.tool_call_id),
                escape_json(&result.name),
                escape_json(&result.output),
                optional_json_string(result.diff.as_deref()),
                if result.success { "true" } else { "false" },
                optional_i32(result.exit_code)
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
            TranscriptEntry::CostUsage { cost } => serde_json::json!({
                "type": "cost_usage",
                "cost": cost,
            })
            .to_string(),
            TranscriptEntry::RuntimeEvent { event } => serde_json::json!({
                "type": "runtime_event",
                "event": event,
            })
            .to_string(),
        }
    }

    pub fn from_json_line(line: &str) -> Result<Self, String> {
        let kind =
            extract_top_level_type_field(line).or_else(|_| extract_string_field(line, "type"))?;
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
                    exit_code: extract_optional_i32_field(line, "exit_code")?,
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
            "cost_usage" => cost_usage_entry_from_json_line(line),
            "runtime_event" => runtime_event_entry_from_json_line(line),
            _ => Err("Unknown transcript entry type".to_string()),
        }
    }
}

fn extract_top_level_type_field(line: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|_| "Malformed transcript entry")?;
    value
        .get("type")
        .and_then(|kind| kind.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| "Missing field `type`".to_string())
}

fn cost_usage_entry_from_json_line(line: &str) -> Result<TranscriptEntry, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|_| "Malformed cost usage transcript entry")?;
    let cost_value = value.get("cost").cloned().unwrap_or_else(|| value.clone());
    let cost =
        serde_json::from_value(cost_value).map_err(|_| "Malformed cost usage transcript entry")?;
    Ok(TranscriptEntry::CostUsage {
        cost: Box::new(cost),
    })
}

fn runtime_event_entry_from_json_line(line: &str) -> Result<TranscriptEntry, String> {
    #[derive(serde::Deserialize)]
    struct RuntimeEventEntry {
        event: RuntimeEvent,
    }

    let entry: RuntimeEventEntry = serde_json::from_str(line)
        .map_err(|err| format!("Malformed runtime event transcript entry: {err}"))?;
    Ok(TranscriptEntry::RuntimeEvent {
        event: Box::new(entry.event),
    })
}

fn optional_json_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_i32(value: Option<i32>) -> String {
    value
        .map(|value| value.to_string())
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

fn extract_optional_i32_field(line: &str, field: &str) -> Result<Option<i32>, String> {
    let marker = format!("\"{field}\":");
    let Some(start) = line.find(&marker).map(|index| index + marker.len()) else {
        return Ok(None);
    };
    let tail = &line[start..];
    if tail.starts_with("null") {
        return Ok(None);
    }
    let end = tail.find([',', '}']).unwrap_or(tail.len());
    tail[..end]
        .trim()
        .parse::<i32>()
        .map(Some)
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
