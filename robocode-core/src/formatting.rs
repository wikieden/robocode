use robocode_types::{ResumeContextSnapshot, now_timestamp};

pub(crate) fn render_resume_context(snapshot: &ResumeContextSnapshot) -> String {
    let mut lines = vec!["Resume context:".to_string()];
    lines.push("  Active tasks:".to_string());
    if snapshot.active_tasks.is_empty() {
        lines.push("    <none>".to_string());
    } else {
        for task in &snapshot.active_tasks {
            lines.push(format!(
                "    {} [{}] {}",
                task.task_id, task.priority, task.title
            ));
        }
    }
    lines.push("  Blocked tasks:".to_string());
    if snapshot.blocked_tasks.is_empty() {
        lines.push("    <none>".to_string());
    } else {
        for task in &snapshot.blocked_tasks {
            lines.push(format!(
                "    {} blocked by {}",
                task.task_id,
                task.blocked_by
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            ));
        }
    }
    lines.push("  Project memory:".to_string());
    if snapshot.relevant_project_memory.is_empty() {
        lines.push("    <none>".to_string());
    } else {
        for entry in &snapshot.relevant_project_memory {
            lines.push(format!("    {} {}", entry.memory_id, entry.content));
        }
    }
    lines.push("Suggested next steps:".to_string());
    for step in &snapshot.suggested_next_steps {
        lines.push(format!("  - {step}"));
    }
    lines.join("\n")
}

pub(crate) fn render_task_detail(task: &robocode_types::TaskRecord) -> String {
    [
        "Task detail:".to_string(),
        format!("  ID: {}", task.task_id),
        format!("  Title: {}", task.title),
        format!("  Status: {}", task.status.cli_name()),
        format!("  Priority: {}", task.priority.cli_name()),
        format!(
            "  Blocked by: {}",
            task.blocked_by
                .clone()
                .unwrap_or_else(|| "<none>".to_string())
        ),
    ]
    .join("\n")
}

pub(crate) fn format_relative_age(timestamp: u64) -> String {
    let now = now_timestamp();
    if timestamp >= now {
        return "just now".to_string();
    }
    let delta = now - timestamp;
    if delta < 60 {
        format!("{delta}s ago")
    } else if delta < 60 * 60 {
        format!("{}m ago", delta / 60)
    } else if delta < 60 * 60 * 24 {
        format!("{}h ago", delta / 3_600)
    } else {
        format!("{}d ago", delta / 86_400)
    }
}
