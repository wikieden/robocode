//! Workflow-oriented resume context derivation.

use robocode_types::{MemoryEntry, ResumeContextSnapshot, TaskRecord, TaskStatus};

use crate::memory::MemoryState;
use crate::tasks::{TaskEvent, TaskState};

#[derive(Debug, Clone)]
pub struct ResumeContextInput<'a> {
    pub task_state: &'a TaskState,
    pub memory_state: &'a MemoryState,
    pub current_session_id: Option<String>,
    pub now: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextBuild {
    pub snapshot: ResumeContextSnapshot,
    pub derived_task_events: Vec<TaskEvent>,
}

pub fn build_resume_context(input: ResumeContextInput<'_>) -> ResumeContextBuild {
    let active_tasks = input
        .task_state
        .active_tasks()
        .into_iter()
        .filter(|task| task.status != TaskStatus::Blocked)
        .cloned()
        .collect::<Vec<_>>();
    let blocked_tasks = input
        .task_state
        .blocked_tasks()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let recently_completed_tasks = input
        .task_state
        .completed_tasks()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let relevant_project_memory = input
        .memory_state
        .active_project_memory()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let recent_session_memory = input
        .current_session_id
        .as_deref()
        .map(|session_id| {
            input
                .memory_state
                .active_session_memory(session_id)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let suggested_next_steps = suggest_next_steps(&active_tasks, &blocked_tasks);
    let suggested_session_memory =
        suggest_session_memory(&active_tasks, &blocked_tasks, &relevant_project_memory);
    let derived_task_events = active_tasks
        .iter()
        .chain(blocked_tasks.iter())
        .map(|task| TaskEvent::Seen {
            task_id: task.task_id.clone(),
            timestamp: input.now,
            origin_session_id: input.current_session_id.clone(),
        })
        .collect();

    ResumeContextBuild {
        snapshot: ResumeContextSnapshot {
            active_tasks,
            blocked_tasks,
            recently_completed_tasks,
            relevant_project_memory,
            recent_session_memory,
            suggested_next_steps,
            suggested_session_memory,
        },
        derived_task_events,
    }
}

fn suggest_next_steps(active_tasks: &[TaskRecord], blocked_tasks: &[TaskRecord]) -> Vec<String> {
    if let Some(task) = active_tasks.first() {
        return vec![format!("Continue {}: {}", task.task_id, task.title)];
    }
    if let Some(task) = blocked_tasks.first() {
        return vec![format!(
            "Resolve blocker for {}: {}",
            task.task_id,
            task.blocked_by
                .clone()
                .unwrap_or_else(|| "unknown blocker".to_string())
        )];
    }
    vec!["No active tasks. Add or restore a task before continuing.".to_string()]
}

fn suggest_session_memory(
    active_tasks: &[TaskRecord],
    blocked_tasks: &[TaskRecord],
    project_memory: &[MemoryEntry],
) -> Vec<String> {
    vec![format!(
        "Resume context generated with {} active task(s), {} blocked task(s), and {} project memory item(s).",
        active_tasks.len(),
        blocked_tasks.len(),
        project_memory.len()
    )]
}

#[cfg(test)]
mod tests {
    use robocode_types::{MemoryKind, MemoryScope, MemorySource, TaskPriority, TaskStatus};

    use crate::memory::{reduce_memory_events, MemoryEvent};
    use crate::tasks::{reduce_task_events, TaskBlocker, TaskEvent};

    use super::*;

    #[test]
    fn resume_context_selects_tasks_memory_and_next_steps() {
        let task_events = vec![
            create_task("task_active", "Continue workflows", 10),
            create_task("task_blocked", "Wait for review", 11),
            TaskEvent::Blocked {
                task_id: "task_blocked".to_string(),
                blocker: TaskBlocker::Reason("review pending".to_string()),
                timestamp: 12,
                origin_session_id: None,
            },
            create_task("task_done", "Finish skeleton", 13),
            TaskEvent::StatusChanged {
                task_id: "task_done".to_string(),
                status: TaskStatus::Done,
                timestamp: 14,
                origin_session_id: None,
            },
        ];
        let task_state = reduce_task_events(&task_events).unwrap();
        let memory_state = reduce_memory_events(&[
            MemoryEvent::Added {
                memory_id: "mem_project".to_string(),
                scope: MemoryScope::Project,
                session_id: None,
                kind: MemoryKind::Convention,
                content: "Keep transcript separate from workflow state".to_string(),
                source: MemorySource::User,
                related_task_ids: vec!["task_active".to_string()],
                confidence_hint: Some("high".to_string()),
                timestamp: 20,
            },
            MemoryEvent::Added {
                memory_id: "mem_session".to_string(),
                scope: MemoryScope::Session,
                session_id: Some("session_1".to_string()),
                kind: MemoryKind::Decision,
                content: "Implement resume context next".to_string(),
                source: MemorySource::Command,
                related_task_ids: vec!["task_active".to_string()],
                confidence_hint: None,
                timestamp: 21,
            },
        ])
        .unwrap();

        let result = build_resume_context(ResumeContextInput {
            task_state: &task_state,
            memory_state: &memory_state,
            current_session_id: Some("session_1".to_string()),
            now: 30,
        });

        assert_eq!(result.snapshot.active_tasks[0].task_id, "task_active");
        assert_eq!(result.snapshot.blocked_tasks[0].task_id, "task_blocked");
        assert_eq!(result.snapshot.recently_completed_tasks[0].task_id, "task_done");
        assert_eq!(result.snapshot.relevant_project_memory[0].memory_id, "mem_project");
        assert_eq!(result.snapshot.recent_session_memory[0].memory_id, "mem_session");
        assert!(
            result.snapshot.suggested_next_steps[0]
                .contains("Continue task_active: Continue workflows")
        );
        assert!(
            result.snapshot.suggested_session_memory[0].contains("Resume context generated")
        );
    }

    #[test]
    fn resume_context_derived_events_do_not_change_task_status() {
        let task_events = vec![
            create_task("task_blocked", "Blocked work", 10),
            TaskEvent::Blocked {
                task_id: "task_blocked".to_string(),
                blocker: TaskBlocker::Reason("needs input".to_string()),
                timestamp: 11,
                origin_session_id: None,
            },
        ];
        let task_state = reduce_task_events(&task_events).unwrap();
        let memory_state = reduce_memory_events(&[]).unwrap();
        let result = build_resume_context(ResumeContextInput {
            task_state: &task_state,
            memory_state: &memory_state,
            current_session_id: Some("session_2".to_string()),
            now: 50,
        });

        let mut replay = task_events;
        replay.extend(result.derived_task_events);
        let updated = reduce_task_events(&replay).unwrap();
        let task = updated.task("task_blocked").unwrap();

        assert_eq!(task.status, TaskStatus::Blocked);
        assert_eq!(task.last_seen_at, Some(50));
        assert_eq!(task.last_session_id.as_deref(), Some("session_2"));
    }

    fn create_task(task_id: &str, title: &str, timestamp: u64) -> TaskEvent {
        TaskEvent::Created {
            task_id: task_id.to_string(),
            title: title.to_string(),
            description: None,
            priority: TaskPriority::Medium,
            labels: Vec::new(),
            assignee_hint: None,
            parent_task_id: None,
            timestamp,
            origin_session_id: None,
        }
    }
}
