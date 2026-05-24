use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use robocode_core::SessionEngine;
use robocode_types::PermissionPrompt;

use super::command_palette::{
    close_on_escape, complete_selected, move_selection, reset_for_input_change,
    should_complete_on_enter,
};
use super::input::{close_focus_on_escape, prompt_for_tui_approval, should_exit};
use super::lane::{handle_tui_command, refresh_lanes};
use super::screen::handle_screen_command;
use super::state::{
    ProviderStatus, TuiEntry, TuiState, WorkspaceSnapshot, entry_from_event, lane_store_path,
    load_lanes,
};
use super::terminal::TerminalGuard;

pub(crate) fn run_tui_with_theme(
    engine: &mut SessionEngine,
    startup_summary: &str,
    theme_name: Option<&str>,
) -> Result<(), String> {
    let mut terminal = TerminalGuard::enter_with_theme(theme_name)?;
    let lane_store = std::env::current_dir()
        .ok()
        .map(|root| lane_store_path(&root));
    let lanes = lane_store.as_deref().map(load_lanes).unwrap_or_default();
    let mut state = initial_state(
        engine,
        startup_summary,
        lane_store,
        lanes,
        terminal.theme_name(),
    );
    terminal.draw(&state)?;

    loop {
        // Poll instead of blocking forever so background lane artifacts can
        // repaint completion, failure, and log-tail state without a keypress.
        if !event::poll(Duration::from_millis(750)).map_err(|err| err.to_string())? {
            refresh_lanes(&mut state);
            terminal.draw(&state)?;
            continue;
        }
        let event = event::read().map_err(|err| err.to_string())?;
        let key = match event {
            Event::Key(key) => key,
            Event::Resize(_, _) => {
                terminal.draw(&state)?;
                continue;
            }
            _ => continue,
        };
        if close_focus_on_escape(key, &mut state) {
            terminal.draw(&state)?;
            continue;
        }
        if close_on_escape(key, &mut state) {
            terminal.draw(&state)?;
            continue;
        }
        if should_exit(key) {
            break;
        }
        match key.code {
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let theme_name = terminal.cycle_theme();
                state.theme_name = theme_name.to_string();
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: format!("Switched TUI theme to `{theme_name}`."),
                });
            }
            KeyCode::Up => {
                if !move_selection(&mut state, -1) {
                    continue;
                }
            }
            KeyCode::Down => {
                if !move_selection(&mut state, 1) {
                    continue;
                }
            }
            KeyCode::Tab => {
                if !complete_selected(&mut state) {
                    continue;
                }
            }
            KeyCode::Enter => {
                if should_complete_on_enter(&state) {
                    complete_selected(&mut state);
                    terminal.draw(&state)?;
                    continue;
                }
                if handle_enter(engine, &mut state, &mut terminal)? {
                    break;
                }
            }
            KeyCode::Backspace => {
                state.input.pop();
                reset_for_input_change(&mut state);
            }
            KeyCode::Char(value) => {
                state.input.push(value);
                reset_for_input_change(&mut state);
            }
            _ => {}
        }
        terminal.draw(&state)?;
    }

    terminal.leave()
}

fn initial_state(
    engine: &SessionEngine,
    startup_summary: &str,
    lane_store: Option<std::path::PathBuf>,
    lanes: Vec<super::state::TerminalLane>,
    theme_name: &str,
) -> TuiState {
    TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        provider_status: ProviderStatus::configured(),
        theme_name: theme_name.to_string(),
        input: String::new(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        entries: vec![TuiEntry {
            label: "system".to_string(),
            body: format!(
                "RoboCode TUI ready. Enter submits. Esc or Ctrl-C exits.\n{startup_summary}"
            ),
        }],
        workspace: WorkspaceSnapshot::load_current(),
        lanes,
        lane_store,
        focused_lane: None,
    }
}

fn handle_enter(
    engine: &mut SessionEngine,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    let input = state.input.trim().to_string();
    state.input.clear();
    if input.is_empty() {
        return Ok(false);
    }
    if is_exit_command(&input) {
        return Ok(true);
    }
    state.entries.push(TuiEntry {
        label: "user".to_string(),
        body: input.clone(),
    });
    if handle_tui_command(&input, state) || handle_screen_command(&input, state) {
        return Ok(false);
    }
    terminal.draw(state)?;
    let mut approver = |prompt: PermissionPrompt| prompt_for_tui_approval(prompt, state, terminal);
    let events = engine.process_input_with_approval(&input, &mut approver)?;
    state
        .entries
        .extend(events.into_iter().map(entry_from_event));
    state.provider = engine.provider_name().to_string();
    state.model = engine.model_name().to_string();
    state.provider_status = ProviderStatus::configured();
    state.workspace = WorkspaceSnapshot::load_current();
    Ok(false)
}

fn is_exit_command(input: &str) -> bool {
    matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "exit" | "quit" | "/exit" | "/quit"
    )
}

#[cfg(test)]
mod tests {
    use super::is_exit_command;

    #[test]
    fn exit_command_accepts_slash_aliases() {
        assert!(is_exit_command("exit"));
        assert!(is_exit_command("quit"));
        assert!(is_exit_command("/exit"));
        assert!(is_exit_command("/quit"));
        assert!(is_exit_command(" /QUIT "));
        assert!(!is_exit_command("/help"));
    }
}
