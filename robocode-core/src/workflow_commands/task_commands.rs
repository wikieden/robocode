use super::SessionEngine;
use crate::{render_resume_context, render_task_detail};
use robocode_types::{ApprovalResponse, TaskPriority, TaskStatus, fresh_id, now_timestamp};
use robocode_workflows::resume_context::{ResumeContextInput, build_resume_context};
use robocode_workflows::tasks::{TaskBlocker, TaskEvent, TaskUpdate};

impl SessionEngine {
    pub(crate) fn handle_tasks(&self) -> Result<String, String> {
        let state = self.workflows.load_task_state()?;
        let tasks = state.active_tasks();
        if tasks.is_empty() {
            return Ok("Project tasks:\n  <none>".to_string());
        }
        let mut lines = vec!["Project tasks:".to_string()];
        for task in tasks {
            lines.push(format!(
                "  {} [{} {}] {}",
                task.task_id,
                task.status.cli_name(),
                task.priority.cli_name(),
                task.title
            ));
        }
        Ok(lines.join("\n"))
    }

    pub(crate) fn handle_task_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_task_help());
        };
        match subcommand {
            "add" => {
                let title = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if title.trim().is_empty() {
                    return Err("Usage: /task add <title>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("task_add", &title, approver)?
                {
                    return Ok(denied);
                }
                let task_id = fresh_id("task");
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Created {
                        task_id: task_id.clone(),
                        title: title.clone(),
                        description: None,
                        priority: TaskPriority::Medium,
                        labels: Vec::new(),
                        assignee_hint: None,
                        parent_task_id: None,
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Created task {task_id} {title}"))
            }
            "view" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task view <task-id>".to_string())?;
                let state = self.workflows.load_task_state()?;
                let task = state
                    .task(task_id)
                    .ok_or_else(|| format!("No task found for `{task_id}`"))?;
                Ok(render_task_detail(task))
            }
            "update" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task update <task-id> <title>".to_string())?;
                let title = args.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
                if title.trim().is_empty() {
                    return Err("Usage: /task update <task-id> <title>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("task_update", &title, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Updated {
                        task_id: task_id.clone(),
                        update: TaskUpdate {
                            title: Some(title.clone()),
                            ..TaskUpdate::default()
                        },
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Updated task {task_id}: {title}"))
            }
            "status" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task status <task-id> <status>".to_string())?;
                let status = args
                    .get(2)
                    .and_then(|value| TaskStatus::parse_cli(value))
                    .ok_or_else(|| "Usage: /task status <task-id> <status>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_status", status.cli_name(), approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::StatusChanged {
                        task_id: task_id.clone(),
                        status,
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Set task {task_id} to {}", status.cli_name()))
            }
            "link" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task link <task-id> <depends-on-id>".to_string())?;
                let depends_on_id = args
                    .get(2)
                    .ok_or_else(|| "Usage: /task link <task-id> <depends-on-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_link", depends_on_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Linked {
                        task_id: task_id.clone(),
                        depends_on_id: depends_on_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!(
                    "Linked task {task_id} to dependency {depends_on_id}"
                ))
            }
            "block" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task block <task-id> <reason|task-id>".to_string())?;
                let reason = args.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
                if reason.trim().is_empty() {
                    return Err("Usage: /task block <task-id> <reason|task-id>".to_string());
                }
                if let Some(denied) =
                    self.ensure_workflow_permission("task_block", &reason, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Blocked {
                        task_id: task_id.clone(),
                        blocker: TaskBlocker::Reason(reason.clone()),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Blocked task {task_id}: {reason}"))
            }
            "unblock" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task unblock <task-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_unblock", task_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Unblocked {
                        task_id: task_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Unblocked task {task_id}"))
            }
            "archive" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task archive <task-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_archive", task_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Archived {
                        task_id: task_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Archived task {task_id}"))
            }
            "restore" => {
                let task_id = args
                    .get(1)
                    .ok_or_else(|| "Usage: /task restore <task-id>".to_string())?;
                if let Some(denied) =
                    self.ensure_workflow_permission("task_restore", task_id, approver)?
                {
                    return Ok(denied);
                }
                self.workflows
                    .append_task_domain_event_checked(&TaskEvent::Restored {
                        task_id: task_id.clone(),
                        timestamp: now_timestamp(),
                        origin_session_id: Some(self.session_id().to_string()),
                    })?;
                Ok(format!("Restored task {task_id}"))
            }
            "resume-context" => {
                let task_state = self.workflows.load_task_state()?;
                let memory_state = self.workflows.load_memory_state()?;
                let result = build_resume_context(ResumeContextInput {
                    task_state: &task_state,
                    memory_state: &memory_state,
                    current_session_id: Some(self.session_id().to_string()),
                    now: now_timestamp(),
                });
                for event in &result.derived_task_events {
                    self.workflows.append_task_domain_event_checked(event)?;
                }
                Ok(render_resume_context(&result.snapshot))
            }
            _ => Ok(format!(
                "Unknown task subcommand `{subcommand}`.\n\n{}",
                self.render_task_help()
            )),
        }
    }
}
