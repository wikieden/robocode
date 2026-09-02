//! D1 command-palette file scope, projected from the Core workspace inventory
//! (GUI-CORE-022).
//!
//! The palette's `~` scope lists files. The client is forbidden from producing
//! that list itself — walking the workspace is outside the client boundary and
//! bypasses the permission gate every other path read passes — so every row
//! here comes from a `WorkspaceFilesLoaded` page Core published, or there is no
//! row.
//!
//! Absence, emptiness, and refusal stay three different facts on the wire:
//! `capability_available` says whether Core publishes an inventory at all,
//! `loaded` says whether a page has actually arrived, and an empty `entries`
//! on a loaded projection is a genuinely empty workspace. The frontend owns the
//! localized sentence for each; it must never collapse them into one empty
//! list.

use serde::Serialize;

use crate::D1OutcomeProjection;

/// The frontend-contract-v1 capability that carries the workspace inventory.
///
/// This is the id Core publishes in its handshake
/// (`FRONTEND_V1_EXTENSION_CAPABILITIES`); the client must not invent a
/// finer-grained one, because an unpublished id can never become available.
pub const WORKSPACE_FILES_CAPABILITY: &str = "runtime.workspace_files";

/// Page size for one `QueryWorkspaceFiles`.
///
/// Core clamps to `1..=MAX_WORKSPACE_FILE_PAGE_SIZE`, so this is a readability
/// choice, not a protocol bound: one page the palette can fuzzy-match over
/// without pausing on a workspace-sized payload.
pub const WORKSPACE_FILES_PAGE_LIMIT: u32 = 500;

/// One inventory entry, exactly as Core published it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileRowProjection {
    /// Workspace-relative, `/`-separated. Never an absolute path.
    pub path: String,
    /// `file`, `dir`, or `unknown` for a kind this build cannot name —
    /// `WorkspaceFileKind` is `#[non_exhaustive]`, and mislabelling an
    /// unmodeled kind as a file would be worse than saying so.
    pub kind: String,
    /// Byte size for a file Core could stat. Absent for a directory, and also
    /// absent for a file whose metadata Core could not read: absence means
    /// "not known", never zero.
    pub size_bytes: Option<u64>,
}

/// What the palette may render after one `QueryWorkspaceFiles`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFilesProjection {
    pub outcome: D1OutcomeProjection,
    /// Lexicographic by path, exactly as Core delivered.
    pub entries: Vec<WorkspaceFileRowProjection>,
    /// True when no further entry matches the read.
    pub complete: bool,
    /// Whether at least one page has arrived. Absence and emptiness are
    /// different facts: "nothing loaded yet" must never render as "no files".
    pub loaded: bool,
    pub pending_command_id: Option<String>,
    /// False when Core's handshake published no `runtime.workspace_files`.
    pub capability_available: bool,
}
