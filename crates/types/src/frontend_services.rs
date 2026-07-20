use crate::{AgentLaneRecord, LocaleId, RuntimeOwner, UiColorMode, UiDensity, UiMotion, UiSkin};

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

/// Closed starter templates offered by first-run and Lane-creation clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterLanePreset {
    Coder,
    Reviewer,
    Tester,
}

/// User-controlled inputs whose resolved result must be reviewed before creation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StarterLaneRequest {
    pub lane_id: String,
    pub preset: StarterLanePreset,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
}

/// Read-only resolution of a starter Lane request against one repository revision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StarterLanePreview {
    pub preview_id: String,
    pub content_sha256: String,
    pub owner: RuntimeOwner,
    pub lane: AgentLaneRecord,
    pub branch: String,
    pub worktree_path: String,
    pub base_revision: String,
    pub diagnostics: Vec<String>,
}

/// Owner-bound proof that the reviewed Lane was durably created.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StarterLaneReceipt {
    pub preview_id: String,
    pub content_sha256: String,
    pub lane: AgentLaneRecord,
    pub branch: String,
    pub worktree_path: String,
    pub base_revision: String,
    pub owner: RuntimeOwner,
}

/// Stable reasons why a reviewed preview stopped being creatable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterLanePreviewInvalidationReason {
    PlanModeDenied,
    RequestChanged,
    HashMismatch,
    BaseRevisionChanged,
    WorktreeUnavailable,
    BranchUnavailable,
    LaneAlreadyRegistered,
    PermissionDenied,
    EffectFailed,
}
