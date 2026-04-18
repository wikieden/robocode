//! Project task domain.

use std::collections::BTreeMap;

use robocode_types::{TaskId, TaskPriority, TaskRecord, TaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskBlocker {
    Task(TaskId),
    Reason(String),
}

impl TaskBlocker {
    fn as_record_value(&self) -> String {
        match self {
            Self::Task(task_id) => task_id.clone(),
            Self::Reason(reason) => reason.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<TaskPriority>,
    pub labels: Option<Vec<String>>,
    pub assignee_hint: Option<Option<String>>,
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Created {
        task_id: TaskId,
        title: String,
        description: Option<String>,
        priority: TaskPriority,
        labels: Vec<String>,
        assignee_hint: Option<String>,
        parent_task_id: Option<TaskId>,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Updated {
        task_id: TaskId,
        update: TaskUpdate,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    StatusChanged {
        task_id: TaskId,
        status: TaskStatus,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Linked {
        task_id: TaskId,
        depends_on_id: TaskId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Blocked {
        task_id: TaskId,
        blocker: TaskBlocker,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Unblocked {
        task_id: TaskId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Archived {
        task_id: TaskId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Restored {
        task_id: TaskId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Seen {
        task_id: TaskId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskState {
    tasks: BTreeMap<TaskId, TaskRecord>,
}

impl TaskState {
    pub fn task(&self, task_id: &str) -> Option<&TaskRecord> {
        self.tasks.get(task_id)
    }

    pub fn active_tasks(&self) -> Vec<&TaskRecord> {
        self.tasks
            .values()
            .filter(|task| task.status != TaskStatus::Archived && task.status != TaskStatus::Done)
            .collect()
    }

    pub fn blocked_tasks(&self) -> Vec<&TaskRecord> {
        self.tasks
            .values()
            .filter(|task| task.status == TaskStatus::Blocked)
            .collect()
    }

    pub fn archived_tasks(&self) -> Vec<&TaskRecord> {
        self.tasks
            .values()
            .filter(|task| task.status == TaskStatus::Archived)
            .collect()
    }

    pub fn completed_tasks(&self) -> Vec<&TaskRecord> {
        self.tasks
            .values()
            .filter(|task| task.status == TaskStatus::Done)
            .collect()
    }

    pub fn child_tasks(&self, parent_task_id: &str) -> Vec<&TaskRecord> {
        self.tasks
            .values()
            .filter(|task| task.parent_task_id.as_deref() == Some(parent_task_id))
            .collect()
    }
}

pub fn reduce_task_events(events: &[TaskEvent]) -> Result<TaskState, String> {
    let mut state = TaskState::default();
    for event in events {
        apply_event(&mut state, event)?;
    }
    Ok(state)
}

fn apply_event(state: &mut TaskState, event: &TaskEvent) -> Result<(), String> {
    match event {
        TaskEvent::Created {
            task_id,
            title,
            description,
            priority,
            labels,
            assignee_hint,
            parent_task_id,
            timestamp,
            origin_session_id,
        } => {
            if state.tasks.contains_key(task_id) {
                return Err(format!("task `{task_id}` already exists"));
            }
            if let Some(parent_id) = parent_task_id {
                require_task(state, parent_id, "missing parent task")?;
            }
            state.tasks.insert(
                task_id.clone(),
                TaskRecord {
                    task_id: task_id.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    status: TaskStatus::Todo,
                    priority: *priority,
                    labels: labels.clone(),
                    assignee_hint: assignee_hint.clone(),
                    parent_task_id: parent_task_id.clone(),
                    dependency_ids: Vec::new(),
                    blocked_by: None,
                    notes: Vec::new(),
                    created_at: *timestamp,
                    updated_at: *timestamp,
                    last_session_id: origin_session_id.clone(),
                    last_seen_at: None,
                    archived_at: None,
                },
            );
        }
        TaskEvent::Updated {
            task_id,
            update,
            timestamp,
            origin_session_id,
        } => {
            let task = require_task_mut(state, task_id)?;
            if let Some(title) = &update.title {
                task.title = title.clone();
            }
            if let Some(description) = &update.description {
                task.description = description.clone();
            }
            if let Some(priority) = update.priority {
                task.priority = priority;
            }
            if let Some(labels) = &update.labels {
                task.labels = labels.clone();
            }
            if let Some(assignee_hint) = &update.assignee_hint {
                task.assignee_hint = assignee_hint.clone();
            }
            if let Some(notes) = &update.notes {
                task.notes = notes.clone();
            }
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::StatusChanged {
            task_id,
            status,
            timestamp,
            origin_session_id,
        } => {
            let task = require_task_mut(state, task_id)?;
            task.status = *status;
            if *status != TaskStatus::Archived {
                task.archived_at = None;
            }
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::Linked {
            task_id,
            depends_on_id,
            timestamp,
            origin_session_id,
        } => {
            require_task(state, depends_on_id, "missing dependency")?;
            let task = require_task_mut(state, task_id)?;
            if !task.dependency_ids.contains(depends_on_id) {
                task.dependency_ids.push(depends_on_id.clone());
            }
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::Blocked {
            task_id,
            blocker,
            timestamp,
            origin_session_id,
        } => {
            if let TaskBlocker::Task(blocker_id) = blocker {
                require_task(state, blocker_id, "missing blocker task")?;
            }
            let task = require_task_mut(state, task_id)?;
            task.status = TaskStatus::Blocked;
            task.blocked_by = Some(blocker.as_record_value());
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::Unblocked {
            task_id,
            timestamp,
            origin_session_id,
        } => {
            let task = require_task_mut(state, task_id)?;
            task.blocked_by = None;
            if task.status == TaskStatus::Blocked {
                task.status = TaskStatus::Todo;
            }
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::Archived {
            task_id,
            timestamp,
            origin_session_id,
        } => {
            let task = require_task_mut(state, task_id)?;
            if task.status == TaskStatus::Archived {
                return Err(format!("task `{task_id}` is already archived"));
            }
            task.status = TaskStatus::Archived;
            task.archived_at = Some(*timestamp);
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::Restored {
            task_id,
            timestamp,
            origin_session_id,
        } => {
            let task = require_task_mut(state, task_id)?;
            if task.status != TaskStatus::Archived {
                return Err(format!("task `{task_id}` is not archived"));
            }
            task.status = TaskStatus::Todo;
            task.archived_at = None;
            touch_task(task, *timestamp, origin_session_id);
        }
        TaskEvent::Seen {
            task_id,
            timestamp,
            origin_session_id,
        } => {
            let task = require_task_mut(state, task_id)?;
            task.last_seen_at = Some(*timestamp);
            touch_task(task, *timestamp, origin_session_id);
        }
    }
    Ok(())
}

fn require_task<'a>(
    state: &'a TaskState,
    task_id: &str,
    message: &str,
) -> Result<&'a TaskRecord, String> {
    state
        .tasks
        .get(task_id)
        .ok_or_else(|| format!("{message}: `{task_id}`"))
}

fn require_task_mut<'a>(
    state: &'a mut TaskState,
    task_id: &str,
) -> Result<&'a mut TaskRecord, String> {
    state
        .tasks
        .get_mut(task_id)
        .ok_or_else(|| format!("missing task: `{task_id}`"))
}

fn touch_task(task: &mut TaskRecord, timestamp: u64, origin_session_id: &Option<String>) {
    task.updated_at = timestamp;
    if let Some(session_id) = origin_session_id {
        task.last_session_id = Some(session_id.clone());
    }
}

#[cfg(test)]
mod tests {
    use robocode_types::{TaskPriority, TaskStatus};

    use super::*;

    #[test]
    fn task_reducer_creates_and_updates_tasks() {
        let state = reduce_task_events(&[
            TaskEvent::Created {
                task_id: "task_1".to_string(),
                title: "Build workflow store".to_string(),
                description: None,
                priority: TaskPriority::High,
                labels: vec!["v2".to_string()],
                assignee_hint: None,
                parent_task_id: None,
                timestamp: 10,
                origin_session_id: Some("session_1".to_string()),
            },
            TaskEvent::Updated {
                task_id: "task_1".to_string(),
                update: TaskUpdate {
                    title: Some("Build workflow event store".to_string()),
                    description: Some(Some("Canonical JSONL plus index".to_string())),
                    priority: Some(TaskPriority::Critical),
                    labels: Some(vec!["v2".to_string(), "storage".to_string()]),
                    assignee_hint: Some(Some("agent".to_string())),
                    notes: Some(vec!["Keep transcript separate".to_string()]),
                },
                timestamp: 11,
                origin_session_id: Some("session_2".to_string()),
            },
        ])
        .unwrap();

        let task = state.task("task_1").unwrap();
        assert_eq!(task.title, "Build workflow event store");
        assert_eq!(
            task.description.as_deref(),
            Some("Canonical JSONL plus index")
        );
        assert_eq!(task.priority, TaskPriority::Critical);
        assert_eq!(task.labels, vec!["v2", "storage"]);
        assert_eq!(task.assignee_hint.as_deref(), Some("agent"));
        assert_eq!(task.notes, vec!["Keep transcript separate"]);
        assert_eq!(task.last_session_id.as_deref(), Some("session_2"));
        assert_eq!(task.updated_at, 11);
    }

    #[test]
    fn task_reducer_links_blocks_and_unblocks_tasks() {
        let state = reduce_task_events(&[
            create_event("task_1", "Parent", 10),
            create_event("task_2", "Dependency", 11),
            TaskEvent::Linked {
                task_id: "task_1".to_string(),
                depends_on_id: "task_2".to_string(),
                timestamp: 12,
                origin_session_id: None,
            },
            TaskEvent::Blocked {
                task_id: "task_1".to_string(),
                blocker: TaskBlocker::Task("task_2".to_string()),
                timestamp: 13,
                origin_session_id: None,
            },
            TaskEvent::Unblocked {
                task_id: "task_1".to_string(),
                timestamp: 14,
                origin_session_id: None,
            },
        ])
        .unwrap();

        let task = state.task("task_1").unwrap();
        assert_eq!(task.dependency_ids, vec!["task_2"]);
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.blocked_by, None);
    }

    #[test]
    fn task_reducer_archives_and_restores_tasks() {
        let state = reduce_task_events(&[
            create_event("task_1", "Archive me", 10),
            TaskEvent::Archived {
                task_id: "task_1".to_string(),
                timestamp: 11,
                origin_session_id: None,
            },
            TaskEvent::Restored {
                task_id: "task_1".to_string(),
                timestamp: 12,
                origin_session_id: None,
            },
        ])
        .unwrap();

        let task = state.task("task_1").unwrap();
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.archived_at, None);
    }

    #[test]
    fn task_reducer_reconstructs_hierarchy_and_validates_links() {
        let state = reduce_task_events(&[
            create_event("task_parent", "Parent", 10),
            TaskEvent::Created {
                task_id: "task_child".to_string(),
                title: "Child".to_string(),
                description: None,
                priority: TaskPriority::Medium,
                labels: Vec::new(),
                assignee_hint: None,
                parent_task_id: Some("task_parent".to_string()),
                timestamp: 11,
                origin_session_id: None,
            },
        ])
        .unwrap();

        assert_eq!(
            state
                .child_tasks("task_parent")
                .iter()
                .map(|task| task.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["task_child"]
        );

        let error = reduce_task_events(&[
            create_event("task_a", "A", 10),
            TaskEvent::Linked {
                task_id: "task_a".to_string(),
                depends_on_id: "missing".to_string(),
                timestamp: 11,
                origin_session_id: None,
            },
        ])
        .unwrap_err();
        assert!(error.contains("missing dependency"));
    }

    fn create_event(task_id: &str, title: &str, timestamp: u64) -> TaskEvent {
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
