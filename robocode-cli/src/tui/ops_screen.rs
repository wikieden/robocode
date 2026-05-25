use super::{
    canvas::Frame,
    indicators::progress_bar,
    lane::{command_hint, pty_label, status_badge},
    panel::panel,
    side_screen::side_status_rows,
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::truncate,
};

pub(super) fn render_ops_body(frame: &mut Frame, state: &TuiState) {
    let body_top = 3;
    let body_bottom = frame.height - BOTTOM_BAR_HEIGHT - 1;
    let body_height = body_bottom.saturating_sub(body_top) + 1;
    let workspace_height = body_height.saturating_mul(7).saturating_div(20).max(8);
    let diagnostics_height = body_height.saturating_mul(5).saturating_div(20).max(6);
    let activity_height = body_height.saturating_mul(4).saturating_div(20).max(6);
    let health_height = body_height
        .saturating_sub(workspace_height + diagnostics_height + activity_height)
        .max(5);

    let workspace = panel(
        "WORKSPACE",
        ops_workspace_rows(state),
        frame.width,
        workspace_height,
        Some("side-2"),
    );
    frame.write_block(body_top, 0, &workspace);

    let diagnostics_top = body_top + workspace_height;
    let diagnostics = panel(
        "LSP / BUILD",
        ops_diagnostic_rows(state),
        frame.width,
        diagnostics_height,
        Some(&state.workspace.diagnostics.len().to_string()),
    );
    frame.write_block(diagnostics_top, 0, &diagnostics);

    let activity_top = diagnostics_top + diagnostics_height;
    let activity = panel(
        "RECENT EVENTS",
        ops_activity_rows(state),
        frame.width,
        activity_height,
        Some("tail"),
    );
    frame.write_block(activity_top, 0, &activity);

    let health_top = activity_top + activity_height;
    let health = panel(
        "PROVIDER HEALTH",
        side_status_rows(state),
        frame.width,
        health_height,
        Some(state.provider_status.connection.as_str()),
    );
    frame.write_block(health_top, 0, &health);
}

fn ops_workspace_rows(state: &TuiState) -> Vec<String> {
    let mut rows = vec![
        format!("ROOT    {}", truncate(&state.workspace.display_root, 60)),
        format!("BRANCH  {}", state.workspace.git_branch),
        format!(
            "SCALE   files {}   lines {}",
            state.workspace.file_count, state.workspace.line_count
        ),
        "TOP FILES".to_string(),
    ];
    rows.extend(
        state
            .workspace
            .top_files
            .iter()
            .take(6)
            .map(|file| format!("  ├ {}", truncate(file, 68))),
    );
    rows
}

fn ops_diagnostic_rows(state: &TuiState) -> Vec<String> {
    if state.workspace.diagnostics.is_empty() {
        return vec![
            "diagnostics unavailable".to_string(),
            "waiting for auto-checks or /lsp diagnostics".to_string(),
        ];
    }
    state
        .workspace
        .diagnostics
        .iter()
        .take(8)
        .map(|diagnostic| truncate(diagnostic, 72))
        .collect()
}

fn ops_activity_rows(state: &TuiState) -> Vec<String> {
    let mut rows = Vec::new();
    rows.extend(state.lanes.iter().take(4).map(|lane| {
        format!(
            "{} {:<10} {:<10} {} {}",
            lane.id,
            truncate(terminal_label_for_ops(&lane.tool), 10),
            status_badge(&lane.status),
            progress_bar(lane.progress)
                .split_whitespace()
                .next()
                .unwrap_or("░░░░░"),
            truncate(
                &format!(
                    "{} :: {} :: {}",
                    pty_label(&lane.tool),
                    command_hint(&lane.tool, &lane.title),
                    lane.summary
                ),
                54
            )
        )
    }));
    if let Some(entry) = state.entries.last() {
        rows.push(format!(
            "main {:<8} {}",
            truncate(&entry.label, 8),
            truncate(&compact_activity(entry), 56)
        ));
    }
    rows
}

fn terminal_label_for_ops(tool: &str) -> &'static str {
    match tool {
        "codex" => "codex tty",
        "claude" => "claude tty",
        "shell" | "run" => "ops tty",
        _ => "agent tty",
    }
}

fn compact_activity(entry: &super::state::TuiEntry) -> String {
    if entry.label == "approval" && entry.body.contains("write_file") {
        let path = entry
            .body
            .lines()
            .find_map(|line| line.strip_prefix("path: "))
            .unwrap_or("workspace");
        return format!("[waiting] write_file {path}");
    }
    entry.body.replace('\n', " / ")
}
