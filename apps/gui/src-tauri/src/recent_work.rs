//! Cross-project recent work, as a client of the Core inventory contract.
//!
//! Core alone scans the shared session home, validates each project key, and
//! bounds the answer (`docs/frontend-integration-contract.md`, "Recent Work
//! Contract"). This module is only the transport-safe vocabulary the webview
//! and the shell share; the typed Core command is built in `adapter.rs`.
//!
//! The GUI never reads a transcript, a project directory, or the session index,
//! and never orders, re-ranks, or truncates the answer: Core already ordered
//! sessions by `(last_updated_at DESC, canonical_root ASC, session_id ASC)` and
//! aggregated projects from that bounded list.

use serde::Serialize;

use crate::d1::D1OutcomeProjection;

/// The frontend-contract-v1 capability that carries the recent-work inventory.
///
/// This is the id Core publishes in its handshake
/// (`FRONTEND_V1_EXTENSION_CAPABILITIES`); the client must not invent a
/// finer-grained one, because an unpublished id can never become available.
pub const RECENT_WORK_CAPABILITY: &str = "runtime.recent_work";

/// One project exactly as `RecentProjectSummary` published it.
///
/// The DTO is a Core whitelist: canonical root, derived display name, latest
/// stable timestamp, latest session id. No transcript path, title, preview, or
/// metadata value exists here to leak, and the client adds none.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProjectProjection {
    pub canonical_root: String,
    pub display_name: String,
    pub last_updated_at: u64,
    pub latest_session_id: Option<String>,
}

/// One session exactly as `RecentSessionSummary` published it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSessionProjection {
    pub canonical_root: String,
    pub session_id: String,
    pub created_at: u64,
    pub last_updated_at: u64,
    pub message_count: u64,
    pub tool_call_count: u64,
    pub command_count: u64,
}

/// What the client may render after one `QueryRecentWork`.
///
/// `projects`, `sessions`, and `diagnostics` are populated only from the
/// confirming `RecentWorkLoaded` fact, so a pending or rejected read never
/// leaves the Welcome screen or the project picker showing rows Core did not
/// publish.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWorkResult {
    pub outcome: D1OutcomeProjection,
    pub projects: Vec<RecentProjectProjection>,
    pub sessions: Vec<RecentSessionProjection>,
    /// Core's own inventory diagnostics, rendered verbatim by code.
    pub diagnostics: Vec<String>,
    pub pending_command_id: Option<String>,
    pub capability_available: bool,
}
