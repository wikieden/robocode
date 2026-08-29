use std::path::PathBuf;

use viden_config::{
    default_user_config_path, preview_reset_user_ui_preferences_at, preview_user_ui_preferences_at,
    reset_user_ui_preferences_at, resolve_user_ui_preferences_at, save_user_ui_preferences_at,
};
use viden_types::{
    ApprovalResponse, AuditQuery, RecentWorkQuery, RuntimeCommand, RuntimeEvent, RuntimeEventKind,
    UiPreferencePatch, UiPreferences, resolve_ui_preferences,
};

use crate::SessionEngine;

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
    pub(crate) fn query_audit(&self, query: AuditQuery) -> Result<Vec<RuntimeEvent>, String> {
        let page = self.workflows.query_audit(&query)?;
        Ok(vec![RuntimeEvent::new(
            1,
            RuntimeEventKind::AuditPageLoaded { page },
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
