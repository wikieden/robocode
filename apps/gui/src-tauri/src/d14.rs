//! D14 audit and timeline projections.
//!
//! The audit trail is the Core replay stream, not the current view state.
//! `RuntimeViewState` publishes present facts only, so D14 pages ordered
//! events through the Core replay cursor and renders them in cursor order.
//! An event the client cannot decode still occupies a row: an audit trail
//! that silently drops entries is worse than one that shows an unknown row.

use serde::Serialize;

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
