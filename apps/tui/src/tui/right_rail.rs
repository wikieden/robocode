use super::{
    lane::pty_label,
    panel::panel,
    state::{AgentTask, TuiState, agent_tasks},
    text::truncate,
};

use std::time::SystemTime;
use viden_types::{TaskRecord, TaskStatus};

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let mut rail = Vec::new();
    let active_tasks = active_task_rows(state);
    let active_task_count = active_task_count(state).to_string();
    let diagnostics = diagnostic_rows(state);
    let diagnostic_badge = state.workspace.diagnostics.len().to_string();
    let compact = height < 28;
    let active_height = if compact { 5 } else { 6 };
    let diagnostic_height = if compact { 3 } else { 5 };
    let provider_height = 8;
    let reserved_height = 7 + active_height + diagnostic_height + provider_height;
    let recent_height = height.saturating_sub(reserved_height).max(3);
    let panels = [
        panel("WORKSPACE", workspace_rows(state), width, 7, None),
        panel(
            "ACTIVE TASKS",
            active_tasks,
            width,
            active_height,
            Some(&active_task_count),
        ),
        panel(
            "LSP DIAGNOSTICS",
            diagnostics,
            width,
            diagnostic_height,
            Some(&diagnostic_badge),
        ),
        panel(
            "PROVIDER HEALTH",
            provider_health_rows(state),
            width,
            provider_height,
            Some(state.provider_status.connection.as_str()),
        ),
        panel(
            "RECENT FILES",
            recent_file_rows(state),
            width,
            recent_height,
            Some("tail"),
        ),
    ];

    for panel_lines in panels {
        rail.extend(panel_lines);
    }
    while rail.len() < height {
        rail.push(" ".repeat(width));
    }
    rail.truncate(height);
    rail
}

fn workspace_rows(state: &TuiState) -> Vec<String> {
    let top = workspace_top_files(state);
    let root_label = state
        .workspace
        .display_root
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("workspace");
    vec![
        format!(
            "{:<12} {}",
            format!("{}/", truncate(root_label, 8)),
            truncate(&state.workspace.display_root, 19)
        ),
        format!("↳ {:<13} FILES {:>6}", top[0], state.workspace.file_count),
        format!("↳ {:<13} LINES {:>6}", top[1], state.workspace.line_count),
        format!(
            "▣ {:<13} LANGUAGE {:>4}",
            top[2],
            truncate(&state.workspace.primary_language, 4)
        ),
        format!(
            "▣ {:<13} EDITION {:>6}",
            top[3],
            state.workspace.rust_edition.as_deref().unwrap_or("-")
        ),
    ]
}

fn active_task_rows(state: &TuiState) -> Vec<String> {
    let mut rows = Vec::new();
    for agent_task in agent_tasks(state)
        .into_iter()
        .filter(AgentTask::is_active)
        .take(4)
    {
        rows.push(format!(
            "{} {} {:<6} {:<8} {:>3}%",
            agent_task.id,
            status_dot(&agent_task.status),
            truncate(&active_agent_label(&agent_task), 6),
            screen_hint(&agent_task),
            agent_task.progress
        ));
    }
    for task in state.tasks.iter().take(3) {
        rows.push(format!(
            "{} {:<11} {}",
            task_status_dot(task.status),
            task_badge(task),
            truncate(&task.title, 18)
        ));
    }
    if rows.is_empty() {
        rows.push("○ no active tasks".to_string());
    }
    rows.truncate(4);
    rows
}

fn screen_hint(task: &AgentTask) -> String {
    let screen = if task.agent == "robocode" && task.kind == "provider" {
        "main"
    } else {
        match task.agent.as_str() {
            "codex" | "claude" => "s1",
            "shell" => "s2",
            _ => task.transport.as_str(),
        }
    };
    format!("{screen}/{}", pty_label_for_agent(&task.agent))
}

fn active_agent_label(task: &AgentTask) -> String {
    if task.agent == "robocode" && task.kind == "provider" {
        "RoboCode".to_string()
    } else {
        task.agent.clone()
    }
}

fn pty_label_for_agent(agent: &str) -> &'static str {
    match agent {
        "robocode" => "robo",
        "codex" | "claude" | "shell" => pty_label(agent),
        _ => "pty/xx",
    }
}

fn diagnostic_rows(state: &TuiState) -> Vec<String> {
    if state.workspace.diagnostics.is_empty() {
        return vec![
            "diagnostics unavailable".to_string(),
            "auto-checks or /lsp diagnostics".to_string(),
        ];
    }
    state
        .workspace
        .diagnostics
        .iter()
        .take(4)
        .map(|diagnostic| truncate(diagnostic, 30))
        .collect()
}

fn provider_health_rows(state: &TuiState) -> Vec<String> {
    let provider_label = format!("{} ({})", display_provider(&state.provider), state.model);
    let mut rows = vec![
        truncate(&provider_label, 31),
        format!("STATUS     {}", state.provider_status.connection),
        format!(
            "REQUESTS   {} ok / {} err",
            state.provider_status.success_count, state.provider_status.failure_count
        ),
        format!(
            "LATENCY    last {} avg {}",
            format_latency(state.provider_status.last_latency_ms),
            format_latency(state.provider_status.average_latency_ms)
        ),
    ];
    if let Some(error) = &state.provider_status.last_error {
        rows.push(format!("ERROR      {}", truncate(error, 20)));
    } else if state.provider_status.request_count == 0 {
        rows.push("TELEMETRY  awaiting first request".to_string());
    } else {
        rows.extend(provider_usage_rows(state));
    }
    rows
}

fn provider_usage_rows(state: &TuiState) -> Vec<String> {
    let mut rows = Vec::new();
    if let Some(tokens) = state.provider_status.last_total_tokens {
        rows.push(format!(
            "TOKENS     last {} total {}",
            format_count(tokens),
            format_count(state.provider_status.total_tokens)
        ));
        if let Some(rate) = state.provider_status.last_tokens_per_second {
            rows.push(format!("RATE       {}/s", format_count(rate)));
        }
        if let Some(cost) = state.provider_status.total_cost_micro_usd {
            rows.push(format!("COST       {}", format_micro_usd(cost)));
        }
    } else {
        rows.push(format!(
            "EVENTS     {} ctx {}",
            state.provider_status.last_event_count, state.provider_status.context_window
        ));
    }
    rows
}

fn format_latency(value: Option<u128>) -> String {
    value
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_count(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}m", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn format_micro_usd(value: u64) -> String {
    format!("${:.4}", value as f64 / 1_000_000.0)
}

fn display_provider(provider: &str) -> String {
    match provider {
        "openai" => "OpenAI".to_string(),
        "deepseek" => "DeepSeek".to_string(),
        "anthropic" => "Anthropic".to_string(),
        value => value.to_string(),
    }
}

fn recent_file_rows(state: &TuiState) -> Vec<String> {
    let mut rows = state
        .workspace
        .recent_files
        .iter()
        .take(5)
        .map(|file| {
            format!(
                "[{}] {:<21} {}",
                file_badge(&file.path),
                truncate(&file.path, 21),
                recent_time(file.modified)
            )
        })
        .collect::<Vec<_>>();
    while rows.len() < 5 {
        rows.push("[--] no recent file       -".to_string());
    }
    rows
}

fn active_task_count(state: &TuiState) -> usize {
    state.tasks.len()
        + agent_tasks(state)
            .into_iter()
            .filter(AgentTask::is_active)
            .count()
}

fn workspace_top_files(state: &TuiState) -> [String; 4] {
    let mut files = state
        .workspace
        .top_files
        .iter()
        .map(|file| truncate(file, 13))
        .collect::<Vec<_>>();
    while files.len() < 4 {
        files.push("-".to_string());
    }
    [
        files[0].clone(),
        files[1].clone(),
        files[2].clone(),
        files[3].clone(),
    ]
}

fn status_dot(status: &str) -> &'static str {
    match status {
        "thinking" | "streaming" | "running_tool" => "●",
        "editing" => "✎",
        "testing" => "⛭",
        "waiting_approval" | "needs_input" | "blocked" => "◆",
        "running" => "●",
        "queued" => "◐",
        "attached" => "◆",
        "done" | "completed" => "✓",
        "failed" => "✕",
        "cancelled" => "·",
        _ => "○",
    }
}

fn task_status_dot(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "○",
        TaskStatus::InProgress => "●",
        TaskStatus::Blocked => "◆",
        TaskStatus::Done => "✓",
        TaskStatus::Archived => "·",
    }
}

fn task_badge(task: &TaskRecord) -> String {
    format!(
        "{}/{}",
        truncate(&task.task_id, 7),
        match task.status {
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "prog",
            TaskStatus::Blocked => "block",
            TaskStatus::Done => "done",
            TaskStatus::Archived => "arch",
        }
    )
}

fn file_badge(file: &str) -> &'static str {
    if file.ends_with(".rs") {
        "Rs"
    } else if file.ends_with(".toml") {
        "Tm"
    } else if file.ends_with(".md") {
        "Md"
    } else {
        "Fs"
    }
}

fn recent_time(modified: SystemTime) -> String {
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return "now".to_string();
    };
    let seconds = age.as_secs();
    if seconds < 60 {
        "now".to_string()
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}
