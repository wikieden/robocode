use crossterm::event::{KeyCode, KeyEvent};

use super::{
    canvas::Frame,
    composer::COMPOSER_HEIGHT,
    panel::{bordered_row, panel_top},
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::{bottom_border, pad},
};

const MAX_VISIBLE_COMMANDS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CommandSuggestion {
    pub(super) command: &'static str,
    pub(super) summary: &'static str,
}

pub(super) fn is_command_palette_query(input: &str) -> bool {
    input.starts_with('/') && !input.contains(char::is_whitespace)
}

pub(super) fn is_command_palette_visible(state: &TuiState) -> bool {
    if !is_command_palette_query(&state.input) {
        return false;
    }
    state
        .command_palette_hidden_for
        .as_ref()
        .is_none_or(|hidden| hidden != &state.input)
        && !command_suggestions(&state.input).is_empty()
}

pub(super) fn command_suggestions(query: &str) -> Vec<CommandSuggestion> {
    COMMANDS
        .into_iter()
        .filter(|item| item.command.starts_with(query))
        .collect()
}

pub(super) fn selected_command(state: &TuiState) -> Option<CommandSuggestion> {
    let suggestions = command_suggestions(&state.input);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    suggestions.get(selected).copied()
}

pub(super) fn move_selection(state: &mut TuiState, delta: i8) -> bool {
    if !is_command_palette_visible(state) {
        return false;
    }
    let count = command_suggestions(&state.input).len();
    if count == 0 {
        state.command_selection = 0;
        return false;
    }
    state.command_selection = if delta < 0 {
        state.command_selection.saturating_sub(1)
    } else {
        (state.command_selection + 1).min(count - 1)
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
    let Some(suggestion) = selected_command(state) else {
        return false;
    };
    state.input = format!("{} ", suggestion.command);
    state.command_selection = 0;
    state.command_palette_hidden_for = None;
    true
}

pub(super) fn should_complete_on_enter(state: &TuiState) -> bool {
    let Some(selected) = selected_command(state) else {
        return false;
    };
    state.input != selected.command && is_command_palette_visible(state)
}

pub(super) fn render_command_suggestions(frame: &mut Frame, state: &TuiState) {
    if !is_command_palette_visible(state) {
        return;
    }
    let suggestions = command_suggestions(&state.input);
    if suggestions.is_empty() {
        return;
    }

    let width = frame.width.saturating_sub(8).clamp(48, 104);
    let visible = suggestions.len().min(MAX_VISIBLE_COMMANDS);
    let height = visible + 2;
    let left = 2usize.min(frame.width.saturating_sub(width));
    let composer_top = frame
        .height
        .saturating_sub(BOTTOM_BAR_HEIGHT)
        .saturating_sub(COMPOSER_HEIGHT);
    let top = composer_top.saturating_sub(height);
    let detail_width = width.saturating_mul(2).saturating_div(5).max(22);
    let command_width = width.saturating_sub(detail_width + 4);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    let mut rows = Vec::with_capacity(height);
    rows.push(panel_top("COMMANDS", width, Some("↑↓ tab enter esc")));
    for (index, suggestion) in suggestions.iter().take(visible).enumerate() {
        rows.push(command_suggestion_row(
            suggestion,
            index == selected,
            width,
            command_width,
            detail_width,
        ));
    }
    rows.push(bottom_border(width));
    frame.fill_rect_pattern(top, 0, frame.width, rows.len(), |_x, _y| ' ');
    frame.write_block(top, left, &rows);
}

const COMMANDS: [CommandSuggestion; 16] = [
    CommandSuggestion {
        command: "/help",
        summary: "Show commands",
    },
    CommandSuggestion {
        command: "/provider",
        summary: "List or switch providers",
    },
    CommandSuggestion {
        command: "/plan",
        summary: "Toggle planning mode",
    },
    CommandSuggestion {
        command: "/git",
        summary: "Git status, diff, branch ops",
    },
    CommandSuggestion {
        command: "/diff",
        summary: "Show latest diff",
    },
    CommandSuggestion {
        command: "/lsp",
        summary: "Diagnostics and symbols",
    },
    CommandSuggestion {
        command: "/task",
        summary: "Create or update tasks",
    },
    CommandSuggestion {
        command: "/tasks",
        summary: "List active tasks",
    },
    CommandSuggestion {
        command: "/memory",
        summary: "Project and session memory",
    },
    CommandSuggestion {
        command: "/screen",
        summary: "Open side screen route",
    },
    CommandSuggestion {
        command: "/lane",
        summary: "Run or inspect agent lanes",
    },
    CommandSuggestion {
        command: "/status",
        summary: "Runtime status",
    },
    CommandSuggestion {
        command: "/config",
        summary: "Show active config",
    },
    CommandSuggestion {
        command: "/doctor",
        summary: "Check setup health",
    },
    CommandSuggestion {
        command: "/exit",
        summary: "Exit RoboCode",
    },
    CommandSuggestion {
        command: "/quit",
        summary: "Exit RoboCode",
    },
];

fn command_suggestion_row(
    suggestion: &CommandSuggestion,
    selected: bool,
    width: usize,
    command_width: usize,
    detail_width: usize,
) -> String {
    let marker = if selected { "›" } else { " " };
    bordered_row(
        &format!(
            "{marker} {}{}{}",
            pad(suggestion.command, command_width),
            " ".repeat(2),
            pad(suggestion.summary, detail_width)
        ),
        width,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{ProviderStatus, TerminalLane, WorkspaceSnapshot};

    fn state_with_input(input: &str) -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: input.to_string(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    #[test]
    fn filters_slash_commands_by_prefix() {
        let suggestions = command_suggestions("/p");

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command)
                .collect::<Vec<_>>(),
            vec!["/provider", "/plan"]
        );
    }

    #[test]
    fn moves_and_clamps_selection() {
        let mut state = state_with_input("/p");

        assert!(move_selection(&mut state, 1));
        assert_eq!(state.command_selection, 1);
        assert!(move_selection(&mut state, 1));
        assert_eq!(state.command_selection, 1);
        assert!(move_selection(&mut state, -1));
        assert_eq!(state.command_selection, 0);
    }

    #[test]
    fn completes_selected_command_with_trailing_space() {
        let mut state = state_with_input("/p");
        state.command_selection = 1;

        assert!(complete_selected(&mut state));

        assert_eq!(state.input, "/plan ");
        assert_eq!(state.command_selection, 0);
    }

    #[test]
    fn escape_hides_until_query_changes() {
        let mut state = state_with_input("/p");

        assert!(is_command_palette_visible(&state));
        assert!(close_on_escape(
            KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::empty()),
            &mut state
        ));
        assert!(!is_command_palette_visible(&state));

        state.input.push('r');
        reset_for_input_change(&mut state);

        assert!(is_command_palette_visible(&state));
    }

    #[test]
    fn enter_only_completes_partial_commands() {
        let partial = state_with_input("/p");
        let exact = state_with_input("/help");

        assert!(should_complete_on_enter(&partial));
        assert!(!should_complete_on_enter(&exact));
    }
}
