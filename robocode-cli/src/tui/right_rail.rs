use super::{
    lane::pty_label,
    modal::latest_approval,
    panel::panel,
    state::{TerminalLane, TuiState},
    text::truncate,
};

use std::time::SystemTime;

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let mut rail = Vec::new();
    let active_tasks = active_task_rows(state);
    let active_task_count = active_task_count(state).to_string();
    let diagnostics = diagnostic_rows(state);
    let diagnostic_badge = state.workspace.diagnostics.len().to_string();
    let recent_height = height.saturating_sub(25).max(3);
    let panels = [
        panel("WORKSPACE", workspace_rows(state), width, 7, None),
        panel(
            "ACTIVE TASKS",
            active_tasks,
            width,
            6,
            Some(&active_task_count),
        ),
        panel(
            "LSP DIAGNOSTICS",
            diagnostics,
            width,
            5,
            Some(&diagnostic_badge),
        ),
        panel(
            "PROVIDER HEALTH",
            provider_health_rows(state),
            width,
            7,
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
    if let Some(approval) = latest_approval(state) {
        rows.push(format!(
            "◆ approval {:<8} [review]",
            truncate(&approval_tool(approval), 8)
        ));
        rows.push(format!("  {}", truncate(&approval_scope(approval), 31)));
    }
    for lane in state
        .lanes
        .iter()
        .filter(|lane| is_active_lane(lane))
        .take(4)
    {
        rows.push(format!(
            "{} {} {:<6} {:<8} {:>3}%",
            lane.id,
            status_dot(&lane.status),
            rail_tool_label(&lane.tool),
            screen_hint(lane),
            lane.progress
        ));
    }
    if rows.is_empty() {
        rows.push("○ no active tasks".to_string());
    }
    rows.truncate(4);
    rows
}

fn rail_tool_label(tool: &str) -> &'static str {
    match tool {
        "codex" => "codex",
        "claude" => "claude",
        "shell" | "run" => "ops",
        _ => "agent",
    }
}

fn screen_hint(lane: &TerminalLane) -> String {
    let screen = match lane.tool.as_str() {
        "codex" | "claude" => "s1",
        "shell" | "run" => "s2",
        _ => lane.target.as_str(),
    };
    format!("{screen}/{}", pty_label(&lane.tool))
}

fn diagnostic_rows(state: &TuiState) -> Vec<String> {
    if state.workspace.diagnostics.is_empty() {
        return vec![
            "diagnostics unavailable".to_string(),
            "run /lsp or cargo check".to_string(),
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
    vec![
        truncate(&provider_label, 31),
        format!("STATUS     {}", state.provider_status.connection),
        format!("TELEMETRY  {}", state.provider_status.telemetry),
        "LATENCY    unavailable".to_string(),
        format!("CONTEXT    {}", state.provider_status.context_window),
    ]
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
    usize::from(latest_approval(state).is_some())
        + state
            .lanes
            .iter()
            .filter(|lane| is_active_lane(lane))
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
        "running" => "●",
        "queued" => "◐",
        "completed" => "✓",
        "failed" => "✕",
        _ => "○",
    }
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

fn is_active_lane(lane: &TerminalLane) -> bool {
    matches!(lane.status.as_str(), "running" | "queued")
}

fn approval_tool(approval: &str) -> String {
    approval.split('`').nth(1).unwrap_or("tool").to_string()
}

fn approval_scope(approval: &str) -> String {
    approval
        .lines()
        .skip(1)
        .find(|line| !line.trim().is_empty() && !line.contains("Press y"))
        .unwrap_or("waiting for decision")
        .trim()
        .to_string()
}
