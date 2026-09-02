use std::path::{Path, PathBuf};

use viden_config::{
    default_user_config_path, preview_reset_user_ui_preferences_at, preview_user_ui_preferences_at,
    reset_user_ui_preferences_at, resolve_user_ui_preferences_at, save_user_ui_preferences_at,
};
use viden_types::{
    ApprovalResponse, AuditQuery, PermissionDecision, RecentWorkQuery, RuntimeCommand,
    RuntimeErrorView, RuntimeEvent, RuntimeEventKind, ToolInput, ToolSpec, UiPreferencePatch,
    UiPreferences, WorkspaceFileEntry, WorkspaceFileKind, WorkspaceFilePage, WorkspaceFilesQuery,
    resolve_ui_preferences,
};

use crate::SessionEngine;
use crate::presentation::render_permission_denial;

/// Permission tool name the workspace inventory read is gated under.
///
/// A named tool rather than a bare path check so an operator can write the
/// same allow/ask/deny rules for it that they write for every other workspace
/// read, and so a denial names something recognizable in the transcript.
pub(crate) const WORKSPACE_FILE_INVENTORY_TOOL: &str = "workspace_file_inventory";

/// Directories that never appear in the inventory, whatever the workspace's
/// `.gitignore` says.
///
/// These hold runtime and agent state — Git's own object store, Viden's
/// session/workflow state, the agent scratch directory, lane worktrees, and
/// reference material — not workspace content. A `.gitignore` that happens to
/// omit one of them must not turn thousands of internal files into palette
/// rows, so the exclusion is unconditional rather than inherited.
const EXCLUDED_STATE_DIRECTORIES: [&str; 5] = [".git", ".viden", ".omx", ".worktrees", ".ref"];

/// Builds the Error event for a refused inventory read.
///
/// Deliberately an `Error`, not an empty [`WorkspaceFilePage`]: a client that
/// received an empty page for a refusal would render "this workspace has no
/// files", which is a fabricated fact rather than a stated refusal.
fn workspace_files_refused(reason: &str, message: &str) -> RuntimeEvent {
    RuntimeEvent::new(
        1,
        RuntimeEventKind::Error {
            error: RuntimeErrorView {
                message: render_permission_denial(WORKSPACE_FILE_INVENTORY_TOOL, reason, message),
                recoverable: true,
                hint: Some(
                    "grant the workspace file inventory permission to list workspace paths"
                        .to_string(),
                ),
            },
        },
    )
}

/// Walks the workspace once, orders it, then cuts the requested page.
///
/// Ordering happens *before* the prefix filter, the `after` cursor, and the
/// limit, so [`WorkspaceFilePage::complete`] and
/// [`WorkspaceFilePage::next_after`] describe the filtered ordered inventory
/// rather than whatever the walker produced first (the audit-page precedent).
/// The whole tree is walked for every page: that is what makes two adjacent
/// pages tile the same inventory instead of drifting against a walker whose
/// order is not guaranteed to repeat.
fn read_workspace_file_page(
    root: &Path,
    query: &WorkspaceFilesQuery,
) -> Result<WorkspaceFilePage, String> {
    let mut walker = ignore::WalkBuilder::new(root);
    walker
        // Dotfiles are workspace content an operator wants to jump to
        // (`.gitignore`, `.github/workflows/...`); the state directories above
        // are removed explicitly instead.
        .hidden(false)
        // `.gitignore` is honored even outside a Git repository: the file
        // states the operator's intent about their own tree whether or not
        // `git init` has run.
        .require_git(false)
        // The inventory describes this workspace, not the machine. A global
        // gitignore or a parent directory's rules would make the same
        // workspace enumerate differently on two computers.
        .git_global(false)
        .parents(false)
        .follow_links(false);
    let mut entries = Vec::new();
    for entry in walker.build() {
        let entry = entry.map_err(|error| format!("workspace inventory walk failed: {error}"))?;
        // Depth 0 is the workspace root itself, which is not an entry in its
        // own inventory.
        if entry.depth() == 0 {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        let path = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if path.is_empty() || is_excluded_state_path(&path) {
            continue;
        }
        let file_type = entry.file_type();
        let kind = match file_type {
            Some(file_type) if file_type.is_dir() => WorkspaceFileKind::Dir,
            Some(file_type) if file_type.is_file() => WorkspaceFileKind::File,
            // Neither a plain file nor a directory (a symlink, socket, or
            // device). `WorkspaceFileKind` is `#[non_exhaustive]` precisely so
            // this build never has to mislabel one, so it is omitted rather
            // than published as a file.
            _ => continue,
        };
        let size_bytes = match kind {
            // Absence means "not known here", never zero: unreadable metadata
            // must not be published as an empty file.
            WorkspaceFileKind::File => entry.metadata().ok().map(|metadata| metadata.len()),
            _ => None,
        };
        entries.push(WorkspaceFileEntry {
            path,
            kind,
            size_bytes,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));

    let filtered = entries.into_iter().filter(|entry| {
        query
            .prefix
            .as_deref()
            .is_none_or(|prefix| entry.path.starts_with(prefix))
    });
    // Exclusive cursor: two adjacent pages tile the inventory without
    // repeating the boundary entry.
    let mut remaining = filtered
        .filter(|entry| {
            query
                .after
                .as_deref()
                .is_none_or(|after| entry.path.as_str() > after)
        })
        .collect::<Vec<_>>();
    let limit = query.clamped_limit();
    let complete = remaining.len() <= limit;
    remaining.truncate(limit);
    let next_after = if complete {
        None
    } else {
        remaining.last().map(|entry| entry.path.clone())
    };
    Ok(WorkspaceFilePage {
        entries: remaining,
        next_after,
        complete,
    })
}

/// Whether a workspace-relative path is inside an excluded state directory.
///
/// Matches the directory itself and everything under it, and only on a whole
/// path segment, so a legitimate `src/.gitkeep` or `docs/git-notes.md` is
/// never mistaken for state.
fn is_excluded_state_path(path: &str) -> bool {
    path.split('/')
        .next()
        .is_some_and(|first| EXCLUDED_STATE_DIRECTORIES.contains(&first))
}

impl SessionEngine {
    /// Loads bounded history through the Core-owned session inventory.
    ///
    /// The session crate returns whitelist DTOs only; frontend code never sees
    /// transcript paths, body previews, or arbitrary metadata values.
    pub(crate) fn query_recent_work(
        &self,
        query: RecentWorkQuery,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let inventory =
            viden_session::SessionStore::query_recent_work(self.store.home_dir(), query)?;
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::RecentWorkLoaded {
                projects: inventory.projects,
                sessions: inventory.sessions,
                diagnostics: inventory.diagnostics,
            },
        )])
    }

    /// Reads one newest-first page of the append-only audit timeline.
    ///
    /// Pure read: no permission prompt, no plan-mode gate, no mutation. The
    /// page is answered from the canonical JSONL, so it stays correct after
    /// the derived SQLite index is deleted.
    ///
    /// `command_id` is the id of the `QueryAudit` being answered and is
    /// published on the page, so a client can attribute the page to the exact
    /// read that asked instead of to whatever read it happened to have in
    /// flight. It is only ever the caller's own id: no other path may mint one.
    pub(crate) fn query_audit(
        &self,
        command_id: &str,
        query: AuditQuery,
    ) -> Result<Vec<RuntimeEvent>, String> {
        let page = self.workflows.query_audit(&query)?;
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::AuditPageLoaded {
                command_id: Some(command_id.to_string()),
                page,
            },
        )])
    }

    /// Reads one ordered page of the workspace file inventory (GUI-CORE-022).
    ///
    /// Unlike [`Self::query_audit`] this read touches the operator's working
    /// tree, so it goes through the permission engine *before* a single
    /// directory entry is read — "the same permission gate as other workspace
    /// reads" is the contract request's own close criterion. Three properties
    /// follow from that and are load-bearing:
    ///
    /// - **A refusal is an error, never an empty page.** "You may not read
    ///   this" and "this workspace has no files" are different facts, and a
    ///   client that received the second for the first would render a
    ///   fabricated absence.
    /// - **An unresolved `Ask` is also a refusal.** This is a read answering a
    ///   keystroke, not an interactive turn: blocking it on an approval prompt
    ///   would stall a client's palette behind a modal, so the gate is decided
    ///   non-interactively and an `Ask` surfaces as the same named refusal a
    ///   `Deny` does. `approver` is deliberately not consulted.
    /// - **Plan mode still answers.** The tool is non-mutating, so the
    ///   engine's `SafeRead` branch allows it while every mutation stays
    ///   blocked (`PermissionEngine::decide`).
    ///
    /// `command_id` is the id of the `QueryWorkspaceFiles` being answered and
    /// is published on the page. Unlike the audit page it is required, so a
    /// client never has to fall back to correlating by its own acceptance.
    pub(crate) fn query_workspace_files(
        &self,
        command_id: &str,
        query: WorkspaceFilesQuery,
    ) -> Result<Vec<RuntimeEvent>, String> {
        query.validate()?;

        // The gate runs first: nothing below this point has read the disk.
        let tool = ToolSpec {
            name: WORKSPACE_FILE_INVENTORY_TOOL.to_string(),
            description: "Read the workspace file inventory".to_string(),
            // Non-mutating, which is what keeps the read answerable in Plan
            // mode through the engine's SafeRead branch.
            is_mutating: false,
            input_schema_hint: "path".to_string(),
        };
        let mut input = ToolInput::new();
        // The workspace root is the path being read, so the engine's path
        // scope check applies to exactly the tree the walk will cover.
        input.insert("path".to_string(), self.cwd.display().to_string());
        match self.permissions.decide(&tool, &input) {
            PermissionDecision::Allow(_) => {}
            PermissionDecision::Deny(deny) => {
                return Ok(vec![workspace_files_refused(
                    &format!("{:?}", deny.decision_reason),
                    &deny.message,
                )]);
            }
            PermissionDecision::Ask(ask) => {
                return Ok(vec![workspace_files_refused(
                    "RequiresApproval",
                    &ask.message,
                )]);
            }
        }

        let page = read_workspace_file_page(&self.cwd, &query)?;
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::WorkspaceFilesLoaded {
                command_id: command_id.to_string(),
                page,
            },
        )])
    }

    /// Installs only the non-secret inputs required to re-resolve preferences.
    /// The complete CLI override object may contain provider credentials and is
    /// deliberately never retained by the engine.
    pub(crate) fn set_ui_preference_context(
        &mut self,
        cli_override: Option<UiPreferences>,
        config_path: Option<PathBuf>,
        system_context: UiPreferences,
    ) {
        self.ui_cli_override = cli_override.filter(|profile| {
            resolve_ui_preferences(Some(*profile), None, None, system_context)
                .diagnostics
                .is_empty()
        });
        self.ui_system_context = system_context;
        if let Some(path) = config_path {
            self.user_config_path_override = Some(path);
        }
    }

    pub(crate) fn ui_preference_mutation_descriptor(
        &self,
        command: &RuntimeCommand,
    ) -> Result<Option<(&'static str, String)>, String> {
        match command {
            RuntimeCommand::SetUiPreferences { patch } => {
                let path = self.ui_user_config_path()?;
                let state = preview_user_ui_preferences_at(&path, patch, self.ui_system_context)?;
                Ok(Some((
                    "ui_preferences_set",
                    preference_preview(state.persisted.as_ref()),
                )))
            }
            RuntimeCommand::ResetUiPreferences => {
                let path = self.ui_user_config_path()?;
                preview_reset_user_ui_preferences_at(
                    &path,
                    self.ui_cli_override,
                    self.ui_system_context,
                )?;
                Ok(Some(("ui_preferences_reset", "reset [ui]".to_string())))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn set_ui_preferences<F>(
        &mut self,
        patch: &UiPreferencePatch,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        let path = self.ui_user_config_path()?;
        let preview = preview_user_ui_preferences_at(&path, patch, self.ui_system_context)?;
        if let Some(denial) = self.ensure_workflow_permission(
            "ui_preferences_set",
            &preference_preview(preview.persisted.as_ref()),
            approver,
        )? {
            return Err(denial);
        }

        save_user_ui_preferences_at(&path, patch, self.ui_system_context)?;
        let state =
            resolve_user_ui_preferences_at(&path, self.ui_cli_override, self.ui_system_context)?;
        self.runtime_snapshot.ui_preferences = state.resolved.clone();
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::UiPreferencesUpdated {
                resolved: state.resolved,
                persisted: state.persisted,
                diagnostics: state.diagnostics,
            },
        )])
    }

    pub(crate) fn reset_ui_preferences<F>(
        &mut self,
        approver: &mut F,
    ) -> Result<Vec<RuntimeEvent>, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        let path = self.ui_user_config_path()?;
        preview_reset_user_ui_preferences_at(&path, self.ui_cli_override, self.ui_system_context)?;
        if let Some(denial) =
            self.ensure_workflow_permission("ui_preferences_reset", "reset [ui]", approver)?
        {
            return Err(denial);
        }

        reset_user_ui_preferences_at(&path, self.ui_cli_override, self.ui_system_context)?;
        let state =
            resolve_user_ui_preferences_at(&path, self.ui_cli_override, self.ui_system_context)?;
        self.runtime_snapshot.ui_preferences = state.resolved.clone();
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::UiPreferencesUpdated {
                resolved: state.resolved,
                persisted: state.persisted,
                diagnostics: state.diagnostics,
            },
        )])
    }

    fn ui_user_config_path(&self) -> Result<PathBuf, String> {
        self.user_config_path_override
            .clone()
            .map(Ok)
            .unwrap_or_else(default_user_config_path)
    }
}

fn preference_preview(profile: Option<&UiPreferences>) -> String {
    profile
        .map(|profile| {
            format!(
                "locale={:?} skin={:?} mode={:?} density={:?} motion={:?}",
                profile.locale, profile.skin, profile.mode, profile.density, profile.motion
            )
            .to_ascii_lowercase()
        })
        .unwrap_or_else(|| "reset [ui]".to_string())
}
