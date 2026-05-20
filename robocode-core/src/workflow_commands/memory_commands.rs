use super::SessionEngine;
use crate::presentation::{
    join_lines, render_empty_section, render_entry_heading, render_field, render_section_title,
    render_subsection_title, render_summary_fields,
};
use robocode_types::{
    ApprovalResponse, MemoryEntry, MemoryKind, MemoryScope, MemorySource, fresh_id, now_timestamp,
};
use robocode_workflows::memory::MemoryEvent;

impl SessionEngine {
    pub(crate) fn handle_memory_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let subcommand = args.first().map(String::as_str).unwrap_or("project");
        match subcommand {
            "project" => self.render_project_memory(),
            "session" => self.render_session_memory(),
            "suggest" if args.len() > 1 => {
                let content = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_suggest", &content, approver)?
                {
                    return Ok(denied);
                }
                let memory_id = fresh_id("mem");
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Suggested {
                        memory_id: memory_id.clone(),
                        kind: MemoryKind::Fact,
                        content: content.clone(),
                        source: MemorySource::AssistantSuggestion,
                        related_task_ids: Vec::new(),
                        confidence_hint: None,
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Suggested memory {memory_id} {content}"))
            }
            "suggest" => self.render_memory_suggestions(),
            "confirm" => {
                let memory_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /memory confirm <memory-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_confirm", memory_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Confirmed {
                        memory_id: memory_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Confirmed memory {memory_id}"))
            }
            "reject" => {
                let memory_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /memory reject <memory-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_reject", memory_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Rejected {
                        memory_id: memory_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Rejected memory {memory_id}"))
            }
            "prune" => {
                let memory_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /memory prune <memory-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_prune", memory_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Pruned {
                        memory_id: memory_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Pruned memory {memory_id}"))
            }
            "export" => self.render_memory_export(),
            "add" => {
                let content = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if content.trim().is_empty() {
                    return Err("Usage: /memory add <content>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("memory_add", &content, approver)?
                {
                    return Ok(denied);
                }
                let memory_id = fresh_id("mem");
                self.workflows
                    .append_memory_domain_event_checked(&MemoryEvent::Added {
                        memory_id: memory_id.clone(),
                        scope: MemoryScope::Session,
                        session_id: Some(self.session_id().to_string()),
                        kind: MemoryKind::Fact,
                        content: content.clone(),
                        source: MemorySource::Command,
                        related_task_ids: Vec::new(),
                        confidence_hint: None,
                        timestamp: now_timestamp(),
                    })?;
                Ok(format!("Added session memory {memory_id} {content}"))
            }
            _ => self.render_project_memory(),
        }
    }

    fn render_project_memory(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.active_project_memory();
        if entries.is_empty() {
            return Ok(render_empty_section("Project memory"));
        }
        let mut lines = memory_summary_lines("Project memory", "active", entries.len());
        for entry in entries {
            lines.extend(render_memory_entry(entry));
        }
        Ok(join_lines(&lines))
    }

    fn render_session_memory(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.active_session_memory(self.session_id());
        if entries.is_empty() {
            return Ok(render_empty_section("Session memory"));
        }
        let mut lines = memory_summary_lines("Session memory", "active", entries.len());
        for entry in entries {
            lines.extend(render_memory_entry(entry));
        }
        Ok(join_lines(&lines))
    }

    fn render_memory_suggestions(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.pending_suggestions();
        if entries.is_empty() {
            return Ok(render_empty_section("Pending memory suggestions"));
        }
        let mut lines =
            memory_summary_lines("Pending memory suggestions", "pending", entries.len());
        for entry in entries {
            lines.extend(render_memory_entry(entry));
        }
        Ok(join_lines(&lines))
    }

    fn render_memory_export(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let project_entries = state.active_project_memory();
        let session_entries = state.active_session_memory(self.session_id());
        let mut lines = vec![
            render_section_title("Memory export").trim_end().to_string(),
            render_summary_fields(&[
                ("project", project_entries.len().to_string()),
                ("session", session_entries.len().to_string()),
            ]),
            String::new(),
            render_subsection_title("Project memory"),
        ];
        for entry in &project_entries {
            lines.extend(render_memory_entry(entry));
        }
        if project_entries.is_empty() {
            lines.push("  <none>".to_string());
        }

        lines.push(String::new());
        lines.push(render_subsection_title("Session memory"));
        for entry in &session_entries {
            lines.extend(render_memory_entry(entry));
        }
        if session_entries.is_empty() {
            lines.push("  <none>".to_string());
        }
        Ok(join_lines(&lines))
    }
}

fn memory_summary_lines(title: &str, count_label: &str, count: usize) -> Vec<String> {
    vec![
        render_section_title(title).trim_end().to_string(),
        render_summary_fields(&[(count_label, count.to_string())]),
        String::new(),
        render_subsection_title("Memory entries"),
    ]
}

fn render_memory_entry(entry: &MemoryEntry) -> Vec<String> {
    let mut lines = vec![
        render_entry_heading(&entry.memory_id),
        render_field("content", &entry.content),
        render_field("kind", entry.kind.cli_name()),
        render_field("scope", entry.scope.cli_name()),
        render_field("status", entry.status.cli_name()),
        render_field("source", entry.source.cli_name()),
    ];
    if let Some(session_id) = &entry.session_id {
        lines.push(render_field("session", session_id));
    }
    if !entry.related_task_ids.is_empty() {
        lines.push(render_field(
            "related tasks",
            entry.related_task_ids.join(", "),
        ));
    }
    if let Some(confidence_hint) = &entry.confidence_hint {
        lines.push(render_field("confidence", confidence_hint));
    }
    lines
}
