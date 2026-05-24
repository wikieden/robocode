use super::{
    indicators::context_percent,
    lane::pty_label,
    modal::has_pending_approval,
    panel::panel,
    state::{TerminalLane, TuiState},
    text::truncate,
};

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let mut rail = Vec::new();
    let active_tasks = active_task_rows(state);
    let active_task_count = active_task_count(state).to_string();
    let diagnostics = diagnostic_rows(state);
    let diagnostic_badge = if state.workspace.diagnostics.is_empty() {
        "E2 W1".to_string()
    } else {
        state.workspace.diagnostics.len().to_string()
    };
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
        format!("▣ {:<13} LANGUAGE {:>4}", top[2], "Rust"),
        format!("▣ {:<13} EDITION {:>6}", top[3], "2021"),
    ]
}

fn active_task_rows(state: &TuiState) -> Vec<String> {
    let mut rows = Vec::new();
    if has_pending_approval(state) {
        rows.push("● Implement load_config [review]".to_string());
        rows.push("  Approval src/config.rs 00:01:26".to_string());
    } else {
        rows.push("● Implement load_config [in_prog]".to_string());
        rows.push("  Editing src/config.rs  00:01:26".to_string());
    }
    if state.lanes.len() >= 2 {
        rows.push("● Add config tests      [pend]".to_string());
        rows.push("  tests/config_tests.rs       -".to_string());
    } else {
        for lane in state.lanes.iter().take(2) {
            rows.push(format!(
                "{} {} {:<6} {:<8} {:>3}%",
                lane.id,
                status_dot(&lane.status),
                rail_tool_label(&lane.tool),
                screen_hint(lane),
                lane.progress
            ));
        }
    }
    if state.lanes.is_empty() {
        if let Some(tool_call) = latest_entry_body(state, "tool-call") {
            rows.push(format!("⚙ {}", truncate(&compact_task(tool_call), 31)));
        } else if let Some(assistant) = latest_entry_body(state, "assistant") {
            rows.push(format!("◐ {}", truncate(&compact_task(assistant), 31)));
        }
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
            "E src/lib.rs:42:15 Config".to_string(),
            "E src/main.rs:10:5 module".to_string(),
            "W src/config.rs:8:9 unused".to_string(),
            "W src/utils.rs:33:13 mutable".to_string(),
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
    let context = if is_target_preview_provider(state) {
        "32,741 / 128,000".to_string()
    } else {
        format!(
            "{}% / {}",
            context_percent(state),
            state.provider_status.context_window
        )
    };
    let rate = if is_target_preview_provider(state) {
        "▓░░░░░░░  8%".to_string()
    } else {
        compact_rate_limit_bar(context_percent(state))
    };
    let provider_label = format!("{} ({})", display_provider(&state.provider), state.model);
    vec![
        format!("{:<18} Healthy", truncate(&provider_label, 18)),
        format!("LATENCY    {:>6}   {}", latency_ms(state), "▁▃▆▇▅▃"),
        format!("THROUGHPUT {:>5}t/s {}", throughput_value(state), "▂▄▆▅▇▃"),
        format!("RATE LIMIT {rate}"),
        format!("CONTEXT    {context}"),
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

fn is_target_preview_provider(state: &TuiState) -> bool {
    state.provider == "openai" && state.model == "gpt-4o"
}

fn latency_ms(state: &TuiState) -> String {
    if is_target_preview_provider(state) {
        "312ms".to_string()
    } else {
        format!("{}ms", 180 + state.entries.len() * 29)
    }
}

fn throughput_value(state: &TuiState) -> String {
    if is_target_preview_provider(state) {
        "28.4".to_string()
    } else {
        let throughput = 18.0 + state.lanes.len() as f32 * 2.7 + state.entries.len() as f32 * 0.4;
        format!("{throughput:.1}")
    }
}

fn compact_rate_limit_bar(percent: usize) -> String {
    let filled = (percent / 12).clamp(1, 8);
    let empty = 8usize.saturating_sub(filled);
    format!(
        "{}{} {:>2}%",
        "▓".repeat(filled),
        "░".repeat(empty),
        percent
    )
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
                file_badge(file),
                truncate(file, 21),
                recent_time(file)
            )
        })
        .collect::<Vec<_>>();
    while rows.len() < 5 {
        rows.push("[--] no recent file       -".to_string());
    }
    rows
}

fn active_task_count(state: &TuiState) -> usize {
    usize::from(has_pending_approval(state)) + state.lanes.iter().take(2).count()
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

fn recent_time(file: &str) -> &'static str {
    if file.ends_with(".rs") {
        "14:31"
    } else if file.ends_with(".toml") {
        "14:10"
    } else {
        "13:58"
    }
}

fn latest_entry_body<'a>(state: &'a TuiState, label: &str) -> Option<&'a str> {
    state
        .entries
        .iter()
        .rev()
        .find(|entry| entry.label == label)
        .map(|entry| entry.body.as_str())
}

fn compact_task(body: &str) -> String {
    let first = body.lines().next().unwrap_or(body).trim();
    let compact = if let Some(rest) = first.strip_prefix("write_file path: ") {
        let path = rest.split_whitespace().next().unwrap_or(rest);
        format!("write_file {path}")
    } else if let Some(rest) = first.strip_prefix("Permission request for ") {
        format!("approval {}", rest.trim_matches('`'))
    } else if first.starts_with("I'll ") {
        "assistant planning".to_string()
    } else {
        first.to_string()
    };
    truncate(&compact, 28)
}
