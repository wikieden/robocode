use crossterm::event::{KeyCode, KeyEvent};

use super::{canvas::Frame, panel::panel, state::TuiState, text::truncate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandSuggestion {
    pub(super) command: String,
    pub(super) summary: String,
}

pub(super) fn is_command_palette_query(input: &str) -> bool {
    input.starts_with('/')
}

pub(super) fn is_command_palette_visible(state: &TuiState) -> bool {
    is_command_palette_query(&state.input)
        && state.command_palette_hidden_for.as_deref() != Some(state.input.as_str())
        && !suggestions(state).is_empty()
}

pub(super) fn selected_command(state: &TuiState) -> Option<CommandSuggestion> {
    suggestions(state).get(state.command_selection).cloned()
}

pub(super) fn move_selection(state: &mut TuiState, delta: i8) -> bool {
    let len = suggestions(state).len();
    if len == 0 {
        return false;
    }
    state.command_selection = if delta < 0 {
        state.command_selection.saturating_sub(1)
    } else {
        (state.command_selection + 1).min(len - 1)
    };
    true
}

pub(super) fn reset_for_input_change(state: &mut TuiState) {
    state.command_selection = 0;
    state.command_palette_hidden_for = None;
}

pub(super) fn close_on_escape(key: KeyEvent, state: &mut TuiState) -> bool {
    if key.code != KeyCode::Esc || !is_command_palette_visible(state) {
        return false;
    }
    state.command_palette_hidden_for = Some(state.input.clone());
    true
}

pub(super) fn complete_selected(state: &mut TuiState) -> bool {
    let Some(selected) = selected_command(state) else {
        return false;
    };
    state.input = selected.command;
    reset_for_input_change(state);
    true
}

pub(super) fn select_suggestion_at(state: &mut TuiState, index: usize) -> bool {
    if index >= suggestions(state).len() {
        return false;
    }
    state.command_selection = index;
    true
}

pub(super) fn command_suggestion_index_at(
    state: &TuiState,
    _terminal_width: u16,
    _terminal_height: u16,
    _bottom_bar_height: usize,
    _column: u16,
    row: u16,
) -> Option<usize> {
    let index = usize::from(row.saturating_sub(3));
    (index < suggestions(state).len()).then_some(index)
}

pub(super) fn should_complete_on_enter(state: &TuiState) -> bool {
    selected_command(state).is_some_and(|selected| selected.command != state.input)
}

pub(super) fn render_command_suggestions(frame: &mut Frame, state: &TuiState) {
    if !is_command_palette_visible(state) {
        return;
    }
    let rows = suggestions(state)
        .into_iter()
        .enumerate()
        .take(8)
        .map(|(index, item)| {
            let marker = if index == state.command_selection {
                ">"
            } else {
                " "
            };
            format!(
                "{marker} {:<24} {}",
                truncate(&item.command, 24),
                truncate(&item.summary, 42)
            )
        })
        .collect::<Vec<_>>();
    let title = if state.input == "/setup" {
        "SETUP WIZARD"
    } else if state.input.starts_with("/lane") {
        "LANE ACTIONS"
    } else {
        "COMMANDS"
    };
    let block = panel(title, rows, frame.width.min(76), 11, None);
    frame.write_block(3, 0, &block);
}

fn suggestions(state: &TuiState) -> Vec<CommandSuggestion> {
    let query = state.input.as_str();
    if query == "/setup" {
        return [
            ("/connect", "Connect a provider"),
            ("/provider doctor", "Run provider doctor"),
            ("fallback test-local", "Offline fallback model"),
        ]
        .into_iter()
        .map(|(command, summary)| CommandSuggestion {
            command: command.to_string(),
            summary: summary.to_string(),
        })
        .collect();
    }
    let mut values = [
        ("/help", "Show commands"),
        ("/setup", "Open first-run setup"),
        ("/connect", "Configure a Core provider"),
        ("/models", "Select an available Core model"),
        ("/mode plan", "Request Plan work mode"),
        ("/mode build", "Request Build work mode"),
        ("/permissions ask", "Request ask permission level"),
        (
            "/permissions read-only",
            "Request read-only permission level",
        ),
        ("/status", "Inspect structured runtime status"),
    ]
    .into_iter()
    .map(|(command, summary)| CommandSuggestion {
        command: command.to_string(),
        summary: summary.to_string(),
    })
    .collect::<Vec<_>>();
    values.extend(state.runtime.lanes.iter().map(|lane| CommandSuggestion {
        command: format!("/lane inspect {}", lane.id),
        summary: format!("{} [{:?}]", lane.summary, lane.status),
    }));
    values
        .into_iter()
        .filter(|item| item.command.starts_with(query) || query == "/")
        .collect()
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn palette_uses_runtime_lanes() {
        let mut state = TuiState::default();
        state.input = "/lane".to_string();
        assert!(suggestions(&state).is_empty());
    }

    #[test]
    fn completion_never_executes_an_effect() {
        let mut state = TuiState::default();
        state.input = "/con".to_string();
        assert!(complete_selected(&mut state));
        assert_eq!(state.input, "/connect");
    }
}
