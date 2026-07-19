use crate::{
    AgentLaneId, AgentTaskId, RuntimeCommand, RuntimeEvent, RuntimeSnapshot, RuntimeViewState,
    SessionId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;

pub const FRONTEND_SCHEMA_V1: SchemaVersion = SchemaVersion(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeOwner {
    pub workspace_id: String,
    pub project_id: String,
    pub lane_id: Option<AgentLaneId>,
    pub session_id: Option<SessionId>,
    pub task_id: Option<AgentTaskId>,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCursor {
    pub stream_id: String,
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCursorOrder {
    DuplicateOrOld,
    Next,
    Gap,
    StreamMismatch,
}

impl EventCursor {
    pub fn classify_incoming(&self, incoming: &Self) -> EventCursorOrder {
        // Different streams are distinct logs and must never look contiguous to a client.
        if self.stream_id != incoming.stream_id {
            return EventCursorOrder::StreamMismatch;
        }

        match incoming.sequence {
            sequence if sequence <= self.sequence => EventCursorOrder::DuplicateOrOld,
            sequence if sequence == self.sequence.saturating_add(1) => EventCursorOrder::Next,
            _ => EventCursorOrder::Gap,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCommandEnvelope {
    pub schema_version: SchemaVersion,
    pub client_id: String,
    pub command_id: String,
    pub owner: RuntimeOwner,
    pub command: RuntimeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventEnvelope {
    pub schema_version: SchemaVersion,
    pub owner: RuntimeOwner,
    pub cursor: EventCursor,
    pub event: RuntimeWireEvent,
}

impl RuntimeEventEnvelope {
    fn validate_known_event_sequence(&self) -> Result<(), String> {
        let RuntimeWireEvent::Known(event) = &self.event else {
            return Ok(());
        };
        if self.cursor.sequence == event.sequence {
            return Ok(());
        }

        Err(format!(
            "runtime event cursor sequence {} does not match event sequence {}",
            self.cursor.sequence, event.sequence
        ))
    }
}

impl Serialize for RuntimeEventEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate_known_event_sequence()
            .map_err(serde::ser::Error::custom)?;
        RuntimeEventEnvelopeRef {
            schema_version: &self.schema_version,
            owner: &self.owner,
            cursor: &self.cursor,
            event: &self.event,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeEventEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let envelope = RuntimeEventEnvelopeWire::deserialize(deserializer)?;
        let envelope = Self {
            schema_version: envelope.schema_version,
            owner: envelope.owner,
            cursor: envelope.cursor,
            event: envelope.event,
        };
        envelope
            .validate_known_event_sequence()
            .map_err(serde::de::Error::custom)?;
        Ok(envelope)
    }
}

#[derive(Serialize)]
struct RuntimeEventEnvelopeRef<'a> {
    schema_version: &'a SchemaVersion,
    owner: &'a RuntimeOwner,
    cursor: &'a EventCursor,
    event: &'a RuntimeWireEvent,
}

#[derive(Deserialize)]
struct RuntimeEventEnvelopeWire {
    schema_version: SchemaVersion,
    owner: RuntimeOwner,
    cursor: EventCursor,
    event: RuntimeWireEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
// Boxing would change the frozen public construction shape without improving wire behavior.
#[allow(clippy::large_enum_variant)]
pub enum RuntimeWireEvent {
    Known(RuntimeEvent),
    Unknown {
        event_type: String,
        payload: serde_json::Value,
    },
}

impl Serialize for RuntimeWireEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Known(event) => event.serialize(serializer),
            Self::Unknown {
                event_type,
                payload,
            } => UnknownRuntimeEvent {
                kind: UnknownRuntimeEventKind {
                    event_type,
                    payload,
                },
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for RuntimeWireEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = serde_json::Value::deserialize(deserializer)?;
        let kind = raw
            .get("kind")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| serde::de::Error::custom("runtime event must contain kind"))?;
        let event_type = kind
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("runtime event kind must contain type"))?;

        if is_known_runtime_event_type(event_type) {
            return serde_json::from_value(raw)
                .map(Self::Known)
                .map_err(serde::de::Error::custom);
        }

        // Preserve forward-compatible payloads so older clients can inspect a stream event
        // they cannot yet reduce, instead of rejecting the entire stream.
        Ok(Self::Unknown {
            event_type: event_type.to_string(),
            payload: kind
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        })
    }
}

#[derive(Serialize)]
struct UnknownRuntimeEvent<'a> {
    kind: UnknownRuntimeEventKind<'a>,
}

#[derive(Serialize)]
struct UnknownRuntimeEventKind<'a> {
    #[serde(rename = "type")]
    event_type: &'a str,
    payload: &'a serde_json::Value,
}

fn is_known_runtime_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "snapshot_updated"
            | "assistant_delta"
            | "tool_call_started"
            | "tool_call_finished"
            | "approval_requested"
            | "approval_resolved"
            | "command_accepted"
            | "command_rejected"
            | "transcript_page_loaded"
            | "input_queued"
            | "input_dequeued"
            | "task_updated"
            | "agent_dag_updated"
            | "lane_updated"
            | "evidence_recorded"
            | "context_updated"
            | "context_bundle_built"
            | "context_item_stored"
            | "context_view_derived"
            | "context_reduction_recorded"
            | "context_retrieved"
            | "context_budget_exceeded"
            | "context_quality_failed"
            | "cost_usage_recorded"
            | "provider_cache_observed"
            | "evidence_canonicalized"
            | "merge_gate_updated"
            | "provider_health_updated"
            | "token_cost_updated"
            | "error"
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshotEnvelope {
    pub schema_version: SchemaVersion,
    pub capabilities: BTreeSet<CapabilityId>,
    pub cursor: EventCursor,
    pub snapshot: RuntimeSnapshot,
    pub view: RuntimeViewState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreHandshake {
    pub core_version: String,
    pub supported_schema_versions: Vec<SchemaVersion>,
    pub active_schema_version: SchemaVersion,
    pub capabilities: BTreeSet<CapabilityId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRequest {
    pub after: EventCursor,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayBatch {
    pub events: Vec<RuntimeEventEnvelope>,
    pub next: EventCursor,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapRecovery {
    Replay(ReplayRequest),
    SnapshotRequired { reason_code: String },
}
