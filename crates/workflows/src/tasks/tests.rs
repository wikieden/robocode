use viden_types::{TaskPriority, TaskStatus};

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
