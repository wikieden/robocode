use super::{
    canvas::Frame,
    indicators::{progress_bar, status_dot},
    lane::{command_hint, interaction_hint, pid_hint, pty_label, status_badge, terminal_label},
    panel::panel,
    state::{TuiState, lane_runtime_evidence},
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
        .filter(|lane| matches!(lane.status.as_str(), "running" | "queued" | "attached"))
        .count();
    vec![
        format!("PROVIDER  {} / {}", state.provider, state.model),
        format!("WORKSPACE {}", truncate(&state.workspace.display_root, 28)),
        format!(
            "SCREENS   main online   {}   {}   lanes {active_lanes}/{}",
            companion_screen_label(state, "side-1"),
            companion_screen_label(state, "side-2"),
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

fn companion_screen_label(state: &TuiState, id: &str) -> String {
    let Some(screen) = state.screens.iter().find(|screen| screen.id == id) else {
        return format!("{id} off");
    };
    let pid = screen.pid.map(|pid| format!(":{pid}")).unwrap_or_default();
    format!("{} {}{}", screen.id, screen.status, pid)
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
            "┌ {} {} {:<10} route {:<8} {}",
            status_dot(&lane.status),
            pad(&format!("{} {}", lane.id, terminal), 14),
            badge,
            truncate(&lane.target, 10),
            progress_bar(lane.progress)
        ));
        rows.push(format!(
            "│ PTY {}  PID {:<5}  ATTACH {}",
            pty_label(&lane.tool),
            pid_hint(lane),
            truncate(&interaction_hint(lane), 40)
        ));
        rows.push(format!(
            "└ CMD {} │ TASK {}",
            truncate(&command_hint(&lane.tool, &lane.title), 24),
            truncate(&lane.title, 36)
        ));
        rows.push(format!("  tail {}", truncate(&lane.summary, 66)));
    }
    rows
}

fn agent_output_rows(state: &TuiState) -> Vec<String> {
    let mut rows = vec!["LANES tail persisted logs │ CONTROL inspect stop route".to_string()];
    for lane in &state.lanes {
        let terminal = terminal_label(&lane.tool);
        rows.push(format!(
            "{} {:<10} {:<10} {} │ {}",
            lane.id,
            terminal,
            status_badge(&lane.status),
            progress_bar(lane.progress)
                .split_whitespace()
                .next()
                .unwrap_or("░░░░░"),
            truncate(
                &format!("{} :: {}", interaction_hint(lane), lane.summary),
                50
            )
        ));
        rows.extend(lane_log_tail_rows(state, &lane.id, 70, 2));
    }
    if rows.len() == 1 {
        rows.push("○ no lane output yet".to_string());
    }
    rows
}

fn lane_log_tail_rows(
    state: &TuiState,
    lane_id: &str,
    max_width: usize,
    max_lines: usize,
) -> Vec<String> {
    let Some(evidence) = state
        .lane_store
        .as_deref()
        .and_then(|store| lane_runtime_evidence(store, lane_id))
    else {
        return Vec::new();
    };
    let keep_from = evidence.log_tail.len().saturating_sub(max_lines);
    evidence
        .log_tail
        .iter()
        .skip(keep_from)
        .map(|line| format!("  │ {}", truncate(line, max_width)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{
        CompanionScreen, ProviderStatus, TerminalLane, TuiEntry, WorkspaceSnapshot, lane_store_path,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn side_status_rows_reflect_tracked_companion_screens() {
        let state = TuiState {
            session_id: "session".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::<TuiEntry>::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: vec![CompanionScreen {
                id: "side-1".to_string(),
                title: "Agent lanes".to_string(),
                status: "launched".to_string(),
                pid: Some(4242),
                summary: "test screen".to_string(),
            }],
            lanes: Vec::<TerminalLane>::new(),
            lane_store: None,
            focused_lane: None,
        };

        let rows = side_status_rows(&state);
        let screen_row = rows
            .iter()
            .find(|row| row.contains("SCREENS"))
            .expect("screen row");

        assert!(screen_row.contains("side-1 launched:4242"));
        assert!(screen_row.contains("side-2 off"));
    }

    #[test]
    fn side_lane_rows_surface_tmux_attach_command() {
        let mut state = TuiState {
            session_id: "session".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::<TuiEntry>::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        };
        state.lanes[0].target = "tmux robocode-session-l1".to_string();

        let rendered = terminal_lane_detail_rows(&state).join("\n");

        assert!(rendered.contains("ATTACH tmux attach -t robocode-session-l1"));
        assert!(rendered.contains("tail patched failing tests"));
    }

    #[test]
    fn side_output_rows_replay_persisted_lane_log_tail() {
        let root = temp_root("side-output-tail");
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".robocode").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "compile started\nrunning cargo test\nall tests green\n",
        )
        .expect("lane log");
        let mut state = TuiState {
            session_id: "session".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::<TuiEntry>::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: Some(lane_store),
            focused_lane: None,
        };
        state.lanes.truncate(1);

        let rendered = agent_output_rows(&state).join("\n");

        assert!(rendered.contains("L1"));
        assert!(rendered.contains("running cargo test"));
        assert!(rendered.contains("all tests green"));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(suffix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("robocode-side-screen-test-{nanos}-{suffix}"))
    }
}
