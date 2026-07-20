use crate::{LocaleId, UiColorMode, UiDensity, UiMotion, UiSkin};

/// Partial personal UI preference update sent by a frontend.
///
/// Every field is a closed enum so serialized commands cannot carry free-form
/// theme names or secrets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiPreferencePatch {
    pub locale: Option<LocaleId>,
    pub skin: Option<UiSkin>,
    pub mode: Option<UiColorMode>,
    pub density: Option<UiDensity>,
    pub motion: Option<UiMotion>,
}

/// Bounded cross-project session inventory requested by a frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecentWorkQuery {
    pub limit: u16,
}

/// Project facts safe to expose outside the session persistence boundary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecentProjectSummary {
    pub canonical_root: String,
    pub display_name: String,
    pub last_updated_at: u64,
    pub latest_session_id: Option<String>,
}

/// Session facts safe to expose outside the session persistence boundary.
///
/// This intentionally omits transcript locations, titles, previews, arbitrary
/// metadata values, and command/tool/message bodies.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecentSessionSummary {
    pub canonical_root: String,
    pub session_id: String,
    pub created_at: u64,
    pub last_updated_at: u64,
    pub message_count: u64,
    pub tool_call_count: u64,
    pub command_count: u64,
}
