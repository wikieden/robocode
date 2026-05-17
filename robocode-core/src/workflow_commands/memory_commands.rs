use super::SessionEngine;
use robocode_types::{
    ApprovalResponse, MemoryKind, MemoryScope, MemorySource, fresh_id, now_timestamp,
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
            return Ok("Project memory:\n  <none>".to_string());
        }
        let mut lines = vec!["Project memory:".to_string()];
        for entry in entries {
            lines.push(format!(
                "  {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        Ok(lines.join("\n"))
    }

    fn render_session_memory(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.active_session_memory(self.session_id());
        if entries.is_empty() {
            return Ok("Session memory:\n  <none>".to_string());
        }
        let mut lines = vec!["Session memory:".to_string()];
        for entry in entries {
            lines.push(format!(
                "  {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        Ok(lines.join("\n"))
    }

    fn render_memory_suggestions(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let entries = state.pending_suggestions();
        if entries.is_empty() {
            return Ok("Pending memory suggestions:\n  <none>".to_string());
        }
        let mut lines = vec!["Pending memory suggestions:".to_string()];
        for entry in entries {
            lines.push(format!(
                "  {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        Ok(lines.join("\n"))
    }

    fn render_memory_export(&self) -> Result<String, String> {
        let state = self.workflows.load_memory_state()?;
        let mut lines = vec!["Memory export:".to_string(), "Project memory:".to_string()];
        for entry in state.active_project_memory() {
            lines.push(format!(
                "  - {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        lines.push("Session memory:".to_string());
        for entry in state.active_session_memory(self.session_id()) {
            lines.push(format!(
                "  - {} [{}] {}",
                entry.memory_id,
                entry.kind.cli_name(),
                entry.content
            ));
        }
        if lines.len() == 3 {
            lines.push("  <none>".to_string());
        }
        Ok(lines.join("\n"))
    }
}
