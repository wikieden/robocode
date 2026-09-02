//! Read-only workspace file inventory contract (GUI-CORE-022).
//!
//! A frontend needs the list of files in the open workspace to offer a file
//! jump target, but it must not produce that list itself: walking the
//! filesystem from a client is outside the client boundary and bypasses the
//! permission gate that governs every other path read. So the inventory is a
//! Core-owned query, and three invariants make its answer trustworthy:
//!
//! 1. **Permission-gated at the source.** Core consults the permission engine
//!    before it reads a single directory entry. A denial is published as an
//!    error, never as an empty page: an empty inventory and a refused
//!    inventory are different facts and must never render the same.
//! 2. **Ordered, then paged.** Entries are sorted lexicographically and the
//!    prefix filter and cursor are applied to that ordered inventory, so
//!    [`WorkspaceFilePage::complete`] and [`WorkspaceFilePage::next_after`]
//!    describe the filtered ordered tree — not whatever the walker happened to
//!    hand back first (the audit-page precedent).
//! 3. **Attributable.** [`crate::RuntimeEventKind::WorkspaceFilesLoaded`]
//!    carries the exact command id of the read it answers, required from the
//!    first byte the type ever shipped. GUI-CORE-024 established what an
//!    optional correlation id costs; this contract is new, so it has no legacy
//!    `None` case to accommodate and never gains one.

use serde::{Deserialize, Serialize};

/// Largest page a single [`WorkspaceFilesQuery`] may return. Mirrors
/// `MAX_AUDIT_PAGE_SIZE`: a read bounded the same way the other paginated read
/// on this contract is bounded.
pub const MAX_WORKSPACE_FILE_PAGE_SIZE: u32 = 500;

/// Page size used when a query names no limit.
///
/// A query with no limit means "a page", never "the whole tree": an unbounded
/// answer would put a workspace-sized payload on the event stream, and a client
/// that wants more already has the `after` cursor to ask for it.
pub const DEFAULT_WORKSPACE_FILE_PAGE_SIZE: u32 = 200;

/// What one inventory entry is.
///
/// `#[non_exhaustive]` because the filesystem has more kinds than these two
/// (symlinks, sockets, devices). A newer Core that starts publishing one must
/// not force this build to mislabel it as a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkspaceFileKind {
    File,
    Dir,
}

/// One entry of the workspace inventory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceFileEntry {
    /// Workspace-relative, `/`-separated, with no leading separator. Always
    /// relative: an absolute path would leak the operator's home directory
    /// onto the wire and into fixtures.
    pub path: String,
    pub kind: WorkspaceFileKind,
    /// Byte size of a file. `None` for a directory, and also `None` for a file
    /// whose metadata Core could not read — absence means "not known here",
    /// never zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// Read-only workspace inventory query.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorkspaceFilesQuery {
    /// Workspace-relative `/`-separated prefix filter; `None` is the whole
    /// tree. Matching is on path prefix, so `crates/ty` keeps
    /// `crates/types/...`.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Page size, clamped to `1..=MAX_WORKSPACE_FILE_PAGE_SIZE`. `None` uses
    /// [`DEFAULT_WORKSPACE_FILE_PAGE_SIZE`].
    #[serde(default)]
    pub limit: Option<u32>,
    /// Exclusive resume cursor: the last `path` of the previous page.
    ///
    /// Exclusive rather than inclusive so two adjacent pages tile the
    /// inventory without repeating the boundary entry.
    #[serde(default)]
    pub after: Option<String>,
}

impl WorkspaceFilesQuery {
    /// Page size actually used, clamped rather than rejected so a malformed
    /// client request still gets a well-formed page.
    pub fn clamped_limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_WORKSPACE_FILE_PAGE_SIZE)
            .clamp(1, MAX_WORKSPACE_FILE_PAGE_SIZE) as usize
    }

    /// Rejects a query that cannot mean what it says.
    ///
    /// A prefix that escapes the workspace is rejected, not clamped: clamping
    /// it to the root would answer a question the caller did not ask, and
    /// answering it with an empty page would be indistinguishable from an
    /// empty subtree. Backslashes are rejected for the same reason the path is
    /// documented as `/`-separated — one separator, so a prefix means the same
    /// thing on every platform.
    pub fn validate(&self) -> Result<(), String> {
        let Some(prefix) = self.prefix.as_deref() else {
            return Ok(());
        };
        if prefix.starts_with('/') || prefix.starts_with('\\') {
            return Err(format!(
                "workspace file prefix `{prefix}` must be workspace-relative"
            ));
        }
        if prefix.contains('\\') {
            return Err(format!(
                "workspace file prefix `{prefix}` must use `/` separators"
            ));
        }
        if prefix.len() > 1 && prefix.as_bytes()[1] == b':' {
            return Err(format!(
                "workspace file prefix `{prefix}` must be workspace-relative"
            ));
        }
        if prefix.split('/').any(|segment| segment == "..") {
            return Err(format!(
                "workspace file prefix `{prefix}` leaves the workspace"
            ));
        }
        if prefix.contains('\0') {
            return Err(format!(
                "workspace file prefix `{prefix}` contains a null byte"
            ));
        }
        Ok(())
    }
}

/// One page of the workspace inventory, ordered lexicographically by `path`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorkspaceFilePage {
    /// Lexicographic by `path`, ascending.
    pub entries: Vec<WorkspaceFileEntry>,
    /// Cursor to pass as the next query's `after`. `None` when complete.
    #[serde(default)]
    pub next_after: Option<String>,
    /// True when no further entry matches the query. Describes the *filtered*
    /// ordered inventory, because Core applies the prefix before cutting the
    /// page — a client filtering a page it already holds could not know whether
    /// a matching path sits on a page it never loaded.
    pub complete: bool,
}
