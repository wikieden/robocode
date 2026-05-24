use super::{
    canvas::Frame,
    indicators::{progress_bar, status_dot},
    lane::{command_hint, pid_hint, pty_label, status_badge, terminal_label},
    panel::panel,
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::{pad, truncate},
};

pub(super) fn render_side_body(frame: &mut Frame, state: &TuiState) {
    let body_top = 3;
    let body_bottom = frame.height - BOTTOM_BAR_HEIGHT - 1;
    let body_height = body_bottom.saturating_sub(body_top) + 1;
    let lane_height = body_height.saturating_mul(8).saturating_div(20).max(12);
    let output_height = body_height.saturating_mul(6).saturating_div(20).max(8);
    let provider_height = body_height
        .saturating_sub(lane_height + output_height)
        .max(6);

    let lane_panel = panel(
        "AGENT LANES",
        terminal_lane_detail_rows(state),
        frame.width,
        lane_height,
        Some("side-1"),
    );
    frame.write_block(body_top, 0, &lane_panel);

    let output_top = body_top + lane_height;
    let output_panel = panel(
        "LIVE OUTPUT",
        agent_output_rows(state),
        frame.width,
        output_height,
        Some("tail"),
    );
    frame.write_block(output_top, 0, &output_panel);

    let provider_top = output_top + output_height;
    let provider_panel = panel(
        "SIDE STATUS",
        side_status_rows(state),
        frame.width,
        provider_height,
        Some(state.provider_status.connection.as_str()),
    );
    frame.write_block(provider_top, 0, &provider_panel);
}

pub(super) fn side_status_rows(state: &TuiState) -> Vec<String> {
    let active_lanes = state
        .lanes
        .iter()
        .filter(|lane| lane.status == "running" || lane.status == "queued")
        .count();
    vec![
        format!("PROVIDER  {} / {}", state.provider, state.model),
        format!("WORKSPACE {}", truncate(&state.workspace.display_root, 28)),
        format!(
            "SCREENS   main online   side-1 live   side-2 ops   lanes {active_lanes}/{}",
            state.lanes.len()
        ),
        format!(
            "TELEMETRY {}   EVENTS {}",
            state.provider_status.telemetry,
            state.entries.len(),
        ),
        format!(
            "CONTEXT   {}   DIAGNOSTICS {}",
            state.provider_status.context_window,
            state.workspace.diagnostics.len(),
        ),
        format!("DIAG      ok   THEME {}", state.theme_name),
    ]
}

fn terminal_lane_detail_rows(state: &TuiState) -> Vec<String> {
    if state.lanes.is_empty() {
        return vec![
            "○ no terminal lanes attached".to_string(),
            "open main screen and run /lane codex <task>".to_string(),
        ];
    }
    let mut rows = Vec::new();
    for lane in &state.lanes {
        let badge = status_badge(&lane.status);
        let terminal = terminal_label(&lane.tool);
        rows.push(format!(
            "┌ {} {} {:<10} route {:<6} {}",
            status_dot(&lane.status),
            pad(&format!("{} {}", lane.id, terminal), 14),
            badge,
            truncate(&lane.target, 8),
            progress_bar(lane.progress)
        ));
        rows.push(format!(
            "│ PTY {}  PID {:<5}  TASK {}",
            pty_label(&lane.tool),
            pid_hint(&lane.tool),
            truncate(&lane.title, 43)
        ));
        rows.push(format!(
            "└ CMD {} │ TAIL {}",
            truncate(&command_hint(&lane.tool, &lane.title), 24),
            truncate(&lane.summary, 36)
        ));
    }
    rows
}

fn agent_output_rows(state: &TuiState) -> Vec<String> {
    let mut rows = vec!["LANES tail persisted logs │ CONTROL inspect stop route".to_string()];
    rows.extend(
        state
            .lanes
            .iter()
            .map(|lane| {
                let terminal = terminal_label(&lane.tool);
                format!(
                    "{} {:<10} {:<10} {} │ {}",
                    lane.id,
                    terminal,
                    status_badge(&lane.status),
                    progress_bar(lane.progress)
                        .split_whitespace()
                        .next()
                        .unwrap_or("░░░░░"),
                    truncate(
                        &format!("{} :: {}", pty_label(&lane.tool), lane.summary),
                        50
                    )
                )
            })
            .collect::<Vec<_>>(),
    );
    if rows.len() == 1 {
        rows.push("○ no lane output yet".to_string());
    }
    rows
}
