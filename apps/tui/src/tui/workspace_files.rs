//! TUI-local state for the Core workspace file inventory (GUI-CORE-022).
//!
//! The TUI is a client of `RuntimeCommand::QueryWorkspaceFiles` ->
//! `RuntimeEventKind::WorkspaceFilesLoaded`. Two rules shape this module:
//!
//! - **Nothing is discovered locally.** The client never walks the filesystem,
//!   never shells out to a file lister, and never reconstructs a tree from
//!   paths that happen to appear in evidence records or tool previews. Every
//!   row comes from a page Core published, or there is no row.
//! - **Absence, emptiness, and refusal are three different facts.** "Core does
//!   not publish an inventory", "the read is still in flight", "the workspace
//!   is empty", and "the read was refused" each render their own sentence.
//!   Collapsing any of them into an empty list would show an operator a
//!   fabricated inventory.
//!
//! Correlation is exact and has no fallback: unlike `AuditPageLoaded`, whose
//! `command_id` is an addition and therefore optional, `WorkspaceFilesLoaded`
//! carries the id as a required field. A page naming another reader's id is
//! ignored outright — there is no acceptance-gated guess to fall back to,
//! which is precisely the residual limitation this contract was designed
//! without.
//!
//! A refusal is attributable for the same reason. Core rejects a refused
//! inventory read with `CommandRejected` naming this exact read, so this module
//! never inspects `RuntimeEventKind::Error`: an `Error` carries no command id,
//! and treating one as "our refusal" because a read happened to be outstanding
//! would let an unrelated lane or provider failure fabricate a refusal Core
//! never issued.

use viden_core::{RuntimeEvent, RuntimeEventKind, WorkspaceFileEntry, WorkspaceFilePage};

use super::text::truncate;

/// The frontend-contract-v1 capability that carries the workspace inventory.
///
/// This is the id Core publishes in its handshake
/// (`FRONTEND_V1_EXTENSION_CAPABILITIES`); the client must not invent a
/// finer-grained one, because an unpublished id can never become available.
pub(super) const WORKSPACE_FILES_CAPABILITY: &str = "runtime.workspace_files";

/// Page size for one `QueryWorkspaceFiles`.
///
/// Core clamps to `1..=MAX_WORKSPACE_FILE_PAGE_SIZE`, so this is a readability
/// choice rather than a protocol bound: one page an operator can fuzzy-search
/// without the palette pausing on a workspace-sized payload.
pub(super) const WORKSPACE_FILE_PAGE_LIMIT: u32 = 500;

/// Display width one file row's context column is truncated to.
const FILE_CONTEXT_WIDTH: usize = 24;

/// What the client knows about the workspace inventory right now.
///
/// Presentation only: it holds no authoritative record, just a command id this
/// client issued and bytes Core published back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct WorkspaceFileIndex {
    /// Whether Core's handshake and snapshot both advertised
    /// `runtime.workspace_files`. False means the client sends nothing and
    /// says so, rather than showing an empty file list.
    available: bool,
    /// Command id of the in-flight read, or `None` when idle.
    awaiting: Option<String>,
    /// Exactly as Core delivered: lexicographic by path.
    entries: Vec<WorkspaceFileEntry>,
    /// Whether at least one page has arrived. Absence and emptiness are
    /// different facts: "nothing loaded yet" must never render as "no files".
    loaded: bool,
    /// Core's rejection reason, verbatim. Never a locally composed sentence.
    error: Option<String>,
}

impl WorkspaceFileIndex {
    pub(super) fn mark_available(&mut self, available: bool) {
        self.available = available;
    }

    pub(super) fn is_available(&self) -> bool {
        self.available
    }

    pub(super) fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub(super) fn is_reading(&self) -> bool {
        self.awaiting.is_some()
    }

    pub(super) fn entries(&self) -> &[WorkspaceFileEntry] {
        &self.entries
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether a fresh read should be dispatched.
    ///
    /// One read at a time: a second concurrent read would leave two pages
    /// racing for one slot, and the exact command id only tells them apart
    /// after the fact.
    pub(super) fn should_read(&self) -> bool {
        self.available && !self.loaded && self.awaiting.is_none() && self.error.is_none()
    }

    /// The whole workspace inventory in one page. The palette fuzzy-matches
    /// locally over what Core sent, so it asks for the tree rather than a
    /// prefix the operator has not typed yet.
    pub(super) fn next_query(&self) -> viden_core::WorkspaceFilesQuery {
        viden_core::WorkspaceFilesQuery {
            prefix: None,
            limit: Some(WORKSPACE_FILE_PAGE_LIMIT),
            after: None,
        }
    }

    /// Registers the in-flight read this index is waiting for.
    pub(super) fn begin(&mut self, command_id: impl Into<String>) {
        self.awaiting = Some(command_id.into());
        self.error = None;
    }

    /// Records Core's verbatim rejection of the in-flight read.
    ///
    /// Only ever called for a `CommandRejected` naming this read, or for a
    /// local transport failure on the send itself.
    pub(super) fn fail(&mut self, reason: impl Into<String>) {
        self.awaiting = None;
        self.error = Some(reason.into());
    }

    /// Applies a page if and only if it answers the read in flight.
    ///
    /// Returns whether it was applied. `command_id` is a required field on
    /// this event, so the match is exact with no fallback: a page carrying
    /// another reader's id belongs to that reader, and a page arriving with no
    /// read in flight belongs to nobody here.
    pub(super) fn apply_page(&mut self, command_id: &str, page: &WorkspaceFilePage) -> bool {
        if self.awaiting.as_deref() != Some(command_id) {
            return false;
        }
        self.entries = page.entries.clone();
        self.awaiting = None;
        self.error = None;
        self.loaded = true;
        true
    }

    /// Reconciles one ordered Core event against the in-flight read.
    ///
    /// Returns whether this event changed the index.
    pub(super) fn observe_event(&mut self, event: &RuntimeEvent) -> bool {
        match &event.kind {
            RuntimeEventKind::CommandRejected { command_id, reason }
                if self.awaiting.as_deref() == Some(command_id.as_str()) =>
            {
                self.fail(reason.clone());
                true
            }
            RuntimeEventKind::WorkspaceFilesLoaded { command_id, page } => {
                self.apply_page(command_id, page)
            }
            _ => false,
        }
    }
}

/// The column shown beside a file row: the entry kind, and the byte size when
/// Core published one. A directory has no size and shows none, rather than a
/// zero that would read as an empty file.
pub(super) fn file_row_context(entry: &WorkspaceFileEntry) -> String {
    let kind = match entry.kind {
        viden_core::WorkspaceFileKind::Dir => "dir",
        viden_core::WorkspaceFileKind::File => "file",
        // `WorkspaceFileKind` is `#[non_exhaustive]`: a newer Core may publish
        // a kind this build cannot name. Saying so is honest; calling it a
        // file would not be.
        _ => "unknown",
    };
    match entry.size_bytes {
        Some(size) => truncate(&format!("{kind} · {size} B"), FILE_CONTEXT_WIDTH),
        None => kind.to_string(),
    }
}
