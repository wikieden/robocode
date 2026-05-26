use robocode_permissions::PermissionEngine;
use robocode_session::SessionStore;
use robocode_types::{
    Message, PermissionMode, Role, SessionMetaEntry, SessionSummary, TranscriptEntry, fresh_id,
    now_timestamp,
};

use crate::SessionEngine;

impl SessionEngine {
    pub(super) fn handle_sessions(&self) -> Result<String, String> {
        let sessions = self.store.list_sessions_for_cwd()?;
        Ok(self.render_session_list(&sessions))
    }

    pub(super) fn handle_resume(&mut self, selector: Option<&str>) -> Result<String, String> {
        let Some(selector) = selector else {
            return self.handle_sessions();
        };
        if selector == "list" {
            return self.handle_sessions();
        }
        let loaded = match selector {
            "latest" => self.store.load_latest_for_cwd()?,
            other => self.resolve_resume_selector(other)?,
        };
        let Some((summary, entries)) = loaded else {
            return Ok("No resumable sessions found for the current project.".to_string());
        };
        let resumed_store = SessionStore::new_with_home(
            self.store.home_dir().to_path_buf(),
            self.cwd.clone(),
            Some(summary.session_id.clone()),
        )?;
        self.store = resumed_store;
        self.messages.clear();
        self.last_diff = None;
        self.last_test = None;
        self.permissions = PermissionEngine::new(&self.cwd);
        self.hydrate(entries)?;
        Ok(format!(
            "Resumed session {} ({})",
            summary.session_id,
            summary.title.unwrap_or_else(|| "untitled".to_string())
        ))
    }

    fn resolve_resume_selector(
        &self,
        selector: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        let sessions = self.store.list_sessions_for_cwd()?;
        if sessions.is_empty() {
            return Ok(None);
        }

        if let Some(loaded) = self.store.load_by_id_for_cwd(selector)? {
            return Ok(Some(loaded));
        }

        let matches: Vec<_> = sessions
            .iter()
            .filter(|summary| {
                summary.session_id != self.session_id()
                    && (summary.session_id.starts_with(selector)
                        || summary
                            .session_id
                            .trim_start_matches("session_")
                            .starts_with(selector))
            })
            .cloned()
            .collect();
        match matches.as_slice() {
            [] => self.resolve_resume_index(&sessions, selector),
            [summary] => {
                let entries = SessionStore::load_entries_from_path(std::path::Path::new(
                    &summary.transcript_path,
                ))?;
                Ok(Some((summary.clone(), entries)))
            }
            _ => Err(format!(
                "Session selector `{selector}` is ambiguous.\n\n{}",
                self.render_session_list(matches.as_slice())
            )),
        }
    }

    fn resolve_resume_index(
        &self,
        sessions: &[SessionSummary],
        selector: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        let index_selector = selector.strip_prefix('#').unwrap_or(selector);
        let Ok(index) = index_selector.parse::<usize>() else {
            return Ok(None);
        };
        if index == 0 {
            return Err("Session indexes start at 1.".to_string());
        }
        if let Some(summary) = sessions.get(index - 1) {
            let entries = SessionStore::load_entries_from_path(std::path::Path::new(
                &summary.transcript_path,
            ))?;
            return Ok(Some((summary.clone(), entries)));
        }
        Err(format!("No session found at index {index}."))
    }

    fn hydrate(&mut self, entries: Vec<TranscriptEntry>) -> Result<(), String> {
        let mut provider_meta = None;
        let mut model_meta = None;
        for entry in entries {
            match entry {
                TranscriptEntry::Message { message } => self.messages.push(message),
                TranscriptEntry::ToolResult { result } => {
                    if let Some(diff) = result.diff.clone() {
                        self.last_diff = Some(diff);
                    }
                    self.messages.push(Message {
                        id: fresh_id("msg"),
                        role: Role::Tool,
                        content: result.output,
                        timestamp: now_timestamp(),
                        tool_name: Some(result.name),
                        tool_call_id: Some(result.tool_call_id),
                    });
                }
                TranscriptEntry::SessionMeta { entry } => match entry.key.as_str() {
                    "permission_mode" => {
                        if let Some(mode) = PermissionMode::parse_cli(&entry.value) {
                            self.permissions.set_mode(mode);
                            self.runtime_snapshot.permission_mode = mode;
                        }
                    }
                    "model" => {
                        model_meta = Some(entry.value.clone());
                    }
                    "provider" => {
                        provider_meta = Some(entry.value.clone());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        self.restore_provider_meta(provider_meta.as_deref(), model_meta.as_deref())?;
        Ok(())
    }

    fn restore_provider_meta(
        &mut self,
        provider_id: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), String> {
        if let Some(provider_id) = provider_id {
            if self.provider_host.is_some() {
                let next_provider = self.create_provider_from_runtime(provider_id, model)?;
                self.provider = next_provider;
                self.runtime_snapshot.provider_family = provider_id.to_string();
                self.runtime_snapshot.model_label = self.provider.model().to_string();
                return Ok(());
            }
        }
        if let Some(model) = model {
            self.provider.set_model(model.to_string());
            self.runtime_snapshot.model_label = self.provider.model().to_string();
        }
        self.runtime_snapshot.provider_family = self.provider.provider_name().to_string();
        Ok(())
    }

    pub(super) fn persist_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.store_entry(TranscriptEntry::SessionMeta {
            entry: SessionMetaEntry {
                timestamp: now_timestamp(),
                key: key.to_string(),
                value: value.to_string(),
            },
        })
    }

    pub(super) fn store_entry(&self, entry: TranscriptEntry) -> Result<(), String> {
        self.store.append_entry(&entry)
    }
}
