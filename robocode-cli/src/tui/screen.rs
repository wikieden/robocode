use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use robocode_core::SessionEngine;

use super::input::should_exit;
use super::state::{
    ProviderStatus, TerminalLane, TuiEntry, TuiState, WorkspaceSnapshot, lane_store_path,
    load_lanes, refresh_lane_runtime, save_lanes,
};
use super::terminal::TerminalGuard;

pub(crate) fn run_side_tui_with_theme(
    engine: &SessionEngine,
    startup_summary: &str,
    screen: SideScreen,
    theme_name: Option<&str>,
) -> Result<(), String> {
    let mut terminal = TerminalGuard::enter_with_theme(theme_name)?;
    let lane_store = std::env::current_dir()
        .ok()
        .map(|root| lane_store_path(&root));
    let mut state = TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        provider_status: ProviderStatus::from_telemetry(&engine.provider_telemetry()),
        theme_name: terminal.theme_name().to_string(),
        input: String::new(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        entries: vec![TuiEntry {
            label: "system".to_string(),
            body: format!("RoboCode side monitor ready. Esc or Ctrl-C exits.\n{startup_summary}"),
        }],
        workspace: WorkspaceSnapshot::load_current(),
        lanes: lane_store
            .as_deref()
            .map(load_lanes)
            .unwrap_or_else(TerminalLane::preview_lanes),
        lane_store,
        focused_lane: None,
    };
    draw_side_screen(&mut terminal, &state, screen)?;

    loop {
        if event::poll(Duration::from_millis(750)).map_err(|err| err.to_string())? {
            let event = event::read().map_err(|err| err.to_string())?;
            let key = match event {
                Event::Key(key) => key,
                Event::Resize(_, _) => {
                    draw_side_screen(&mut terminal, &state, screen)?;
                    continue;
                }
                _ => continue,
            };
            if should_exit(key) {
                break;
            }
            if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
                let theme_name = terminal.cycle_theme();
                state.theme_name = theme_name.to_string();
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: format!("Switched TUI theme to `{theme_name}`."),
                });
            }
        }
        if let Some(path) = state.lane_store.as_deref() {
            state.lanes = load_lanes(path);
            refresh_lane_runtime(path, &mut state.lanes);
            let _ = save_lanes(path, &state.lanes);
        }
        state.workspace = WorkspaceSnapshot::load_current();
        draw_side_screen(&mut terminal, &state, screen)?;
    }

    terminal.leave()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideScreen {
    Lanes,
    Ops,
}

impl SideScreen {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "side" | "side-1" => Some(Self::Lanes),
            "side-2" | "ops" => Some(Self::Ops),
            _ => None,
        }
    }
}

pub(super) fn handle_screen_command(input: &str, state: &mut TuiState) -> bool {
    if !input.starts_with("/screen") {
        return false;
    }
    let mut parts = input.split_whitespace();
    let _ = parts.next();
    match parts.next() {
        Some("side-1") | Some("side") => push_screen_launch(
            state,
            "side-1",
            "cargo run -p robocode-cli -- --tui-screen side-1",
        ),
        Some("side-2") | Some("ops") => push_screen_launch(
            state,
            "side-2",
            "cargo run -p robocode-cli -- --tui-screen side-2",
        ),
        Some("main") => push_screen_launch(state, "main", "cargo run -p robocode-cli -- --tui"),
        _ => state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: "Usage: /screen main | /screen side-1 | /screen side-2".to_string(),
        }),
    }
    true
}

fn draw_side_screen(
    terminal: &mut TerminalGuard,
    state: &TuiState,
    screen: SideScreen,
) -> Result<(), String> {
    match screen {
        SideScreen::Lanes => terminal.draw_side(state),
        SideScreen::Ops => terminal.draw_ops(state),
    }
}

fn push_screen_launch(state: &mut TuiState, screen: &str, command: &str) {
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!("Screen `{screen}` launch command:\n{command}"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    #[test]
    fn screen_command_reports_side_launch_command() {
        let mut state = state();

        assert!(handle_screen_command("/screen side-2", &mut state));

        assert!(state.entries[0].body.contains("Screen `side-2`"));
        assert!(state.entries[0].body.contains("--tui-screen side-2"));
    }

    #[test]
    fn screen_command_reports_usage_for_unknown_screen() {
        let mut state = state();

        assert!(handle_screen_command("/screen other", &mut state));

        assert!(state.entries[0].body.contains("Usage: /screen"));
    }
}
