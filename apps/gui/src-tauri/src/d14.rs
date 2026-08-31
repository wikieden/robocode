//! D14 audit and timeline projections.
//!
//! D14 has two host-computed modes over two different Core surfaces:
//!
//! - **Audit mode** (primary) is the Core audit contract:
//!   `RuntimeCommand::QueryAudit` -> `RuntimeEventKind::AuditPageLoaded`
//!   (`crates/types/src/audit.rs`). It answers "who changed what, on which
//!   objects, with what outcome" from the append-only audit store.
//! - **Raw replay mode** (diagnostic) is the Core replay stream. It answers
//!   "which ordered events did Core actually emit", which is a different
//!   question and stays available when the audit capability is absent.
//!
//! The raw mode's rules are unchanged: `RuntimeViewState` publishes present
//! facts only, so it pages ordered events through the Core replay cursor and
//! renders them in cursor order, and an event the client cannot decode still
//! occupies a row — an audit trail that silently drops entries is worse than
//! one that shows an unknown row.

use serde::{Deserialize, Serialize};

use crate::d1::D1OutcomeProjection;

/// The frontend-contract-v1 capability that carries the audit timeline.
///
/// This is the id Core publishes in its handshake
/// (`FRONTEND_V1_EXTENSION_CAPABILITIES`); the client must not invent a
/// finer-grained one, because an unpublished id can never become available.
pub const AUDIT_CAPABILITY: &str = "runtime.audit";

/// Page size for one `QueryAudit`.
///
/// Core clamps to `1..=MAX_AUDIT_PAGE_SIZE`, so this is a readability choice,
/// not a protocol bound: a page the operator can scroll, with older records
/// reachable through the load-older control.
pub const D14_AUDIT_PAGE_LIMIT: u32 = 100;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14RowProjection {
    /// Core stream cursor sequence; the only ordering key.
    pub sequence: u64,
    pub stream_id: String,
    /// Canonical Core event discriminant, or `unknown` for an undecodable one.
    pub kind: String,
    pub known: bool,
    pub timestamp: Option<u64>,
    pub project_id: String,
    pub lane_id: Option<String>,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditTimelineProjection {
    pub rows: Vec<D14RowProjection>,
    /// Opaque `stream:sequence` cursor for the next page.
    pub next_cursor: Option<String>,
    pub complete: bool,
}

/// One object an audit record linked, as Core published it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditObjectProjection {
    pub kind: String,
    pub id: String,
}

/// One bounded audit argument, kept in Core's own `BTreeMap` order so two
/// captures of the same record render identically.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditArgProjection {
    pub key: String,
    pub value: String,
}

/// One audit record, projected for rendering and nothing more.
///
/// `action` is Core's stable dotted vocabulary (`gate.decided`,
/// `change.reverted`, ...). The contract keeps it deliberately free of prose so
/// two timelines can be diffed; localizing it here would destroy exactly that
/// property, so it travels raw and the chrome around it is localized instead.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditRowProjection {
    pub audit_id: String,
    pub timestamp: u64,
    /// `operator`, `agent`, `system`, or `unknown` for an actor a newer Core
    /// published that this build cannot name. `AuditActor` is
    /// `#[non_exhaustive]`, and borrowing a known label would be a lie.
    pub actor_kind: String,
    /// Present only when the actor is an agent lane.
    pub agent_id: Option<String>,
    pub action: String,
    pub objects: Vec<D14AuditObjectProjection>,
    /// `success`, `denied`, `failed`, or `unknown`; see `actor_kind`.
    pub outcome: String,
    pub args: Vec<D14AuditArgProjection>,
}

/// The object filter one audit view is scoped to, reported back so the screen
/// can render (and drop) the scope chip without keeping its own copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditScopeProjection {
    pub kind: String,
    pub id: String,
}

/// The scope a caller asks a query to use.
///
/// Deserialized from the webview so a screen can navigate into a scoped audit
/// view; the host still builds the typed `AuditObjectRef` itself.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditScopeInput {
    pub kind: String,
    pub id: String,
}

/// What the client may render after one `QueryAudit`.
///
/// `rows`, `next_before`, and `complete` are populated only from a confirming
/// `AuditPageLoaded`, so a pending or rejected read never leaves D14 showing
/// records Core did not publish for this query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D14AuditProjection {
    pub outcome: D1OutcomeProjection,
    /// Newest first, exactly as Core delivered. Older pages append at the end.
    pub rows: Vec<D14AuditRowProjection>,
    /// Opaque `timestamp:audit_id` cursor for the next (older) page.
    pub next_before: Option<String>,
    pub complete: bool,
    /// Whether at least one page has arrived. Absence and emptiness are
    /// different facts: "nothing loaded yet" must never render as "no records".
    pub loaded: bool,
    pub pending_command_id: Option<String>,
    /// False when Core's handshake published no `runtime.audit`.
    pub capability_available: bool,
    pub scope: Option<D14AuditScopeProjection>,
}
