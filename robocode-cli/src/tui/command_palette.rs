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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandSuggestion {
    pub(super) command: String,
    pub(super) summary: String,
}

pub(super) fn is_command_palette_query(input: &str) -> bool {
    if !input.starts_with('/') {
        return false;
    }
    !input.contains(char::is_whitespace) || is_lane_command_query(input)
}

pub(super) fn is_command_palette_visible(state: &TuiState) -> bool {
    if !is_command_palette_query(&state.input) {
        return false;
    }
    state
        .command_palette_hidden_for
        .as_ref()
        .is_none_or(|hidden| hidden != &state.input)
        && !command_suggestions_for_state(state).is_empty()
}

fn command_suggestions_for_state(state: &TuiState) -> Vec<CommandSuggestion> {
    lane_command_suggestions(&state.input, state)
        .unwrap_or_else(|| static_command_suggestions(&state.input))
}

fn static_command_suggestions(query: &str) -> Vec<CommandSuggestion> {
    COMMANDS
        .into_iter()
        .filter(|item| item.command.starts_with(query))
        .map(command_from_template)
        .collect()
}

pub(super) fn selected_command(state: &TuiState) -> Option<CommandSuggestion> {
    let suggestions = command_suggestions_for_state(state);
    let selected = state
        .command_selection
        .min(suggestions.len().saturating_sub(1));
    suggestions.get(selected).cloned()
}

pub(super) fn move_selection(state: &mut TuiState, delta: i8) -> bool {
    if !is_command_palette_visible(state) {
        return false;
    }
    let count = command_suggestions_for_state(state).len();
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
    let suggestions = command_suggestions_for_state(state);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandTemplate {
    command: &'static str,
    summary: &'static str,
}

const COMMANDS: [CommandTemplate; 16] = [
    CommandTemplate {
        command: "/help",
        summary: "Show commands",
    },
    CommandTemplate {
        command: "/provider",
        summary: "List or switch providers",
    },
    CommandTemplate {
        command: "/plan",
        summary: "Toggle planning mode",
    },
    CommandTemplate {
        command: "/git",
        summary: "Git status, diff, branch ops",
    },
    CommandTemplate {
        command: "/diff",
        summary: "Show latest diff",
    },
    CommandTemplate {
        command: "/lsp",
        summary: "Diagnostics and symbols",
    },
    CommandTemplate {
        command: "/task",
        summary: "Create or update tasks",
    },
    CommandTemplate {
        command: "/tasks",
        summary: "List active tasks",
    },
    CommandTemplate {
        command: "/memory",
        summary: "Project and session memory",
    },
    CommandTemplate {
        command: "/screen",
        summary: "Open side screen route",
    },
    CommandTemplate {
        command: "/lane",
        summary: "Run or inspect agent lanes",
    },
    CommandTemplate {
        command: "/status",
        summary: "Runtime status",
    },
    CommandTemplate {
        command: "/config",
        summary: "Show active config",
    },
    CommandTemplate {
        command: "/doctor",
        summary: "Check setup health",
    },
    CommandTemplate {
        command: "/exit",
        summary: "Exit RoboCode",
    },
    CommandTemplate {
        command: "/quit",
        summary: "Exit RoboCode",
    },
];

const LANE_COMMANDS: [CommandTemplate; 13] = [
    CommandTemplate {
        command: "/lane codex",
        summary: "Start Codex lane",
    },
    CommandTemplate {
        command: "/lane claude",
        summary: "Start Claude lane",
    },
    CommandTemplate {
        command: "/lane run",
        summary: "Run shell lane",
    },
    CommandTemplate {
        command: "/lane inspect",
        summary: "Inspect lane evidence",
    },
    CommandTemplate {
        command: "/lane stop",
        summary: "Stop running lane",
    },
    CommandTemplate {
        command: "/lane attach",
        summary: "Open lane terminal",
    },
    CommandTemplate {
        command: "/lane detach",
        summary: "Detach lane terminal",
    },
    CommandTemplate {
        command: "/lane accept",
        summary: "Accept lane result",
    },
    CommandTemplate {
        command: "/lane revise",
        summary: "Request revision",
    },
    CommandTemplate {
        command: "/lane discard",
        summary: "Discard lane result",
    },
    CommandTemplate {
        command: "/lane apply",
        summary: "Apply accepted patch",
    },
    CommandTemplate {
        command: "/lane cleanup",
        summary: "Archive worktree",
    },
    CommandTemplate {
        command: "/lane close",
        summary: "Close lane focus",
    },
];

const LANE_ID_COMMANDS: [&str; 9] = [
    "/lane inspect",
    "/lane stop",
    "/lane attach",
    "/lane detach",
    "/lane accept",
    "/lane revise",
    "/lane discard",
    "/lane apply",
    "/lane cleanup",
];

fn command_from_template(template: CommandTemplate) -> CommandSuggestion {
    CommandSuggestion {
        command: template.command.to_string(),
        summary: template.summary.to_string(),
    }
}

fn is_lane_command_query(input: &str) -> bool {
    input == "/lane " || input.starts_with("/lane ")
}

fn lane_command_suggestions(query: &str, state: &TuiState) -> Option<Vec<CommandSuggestion>> {
    if !is_lane_command_query(query) {
        return None;
    }
    let words = query.split_whitespace().collect::<Vec<_>>();
    let suggestions = if words.len() <= 1 || query.ends_with(' ') && words.len() == 1 {
        LANE_COMMANDS
            .into_iter()
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if words.len() == 2 && !query.ends_with(' ') {
        LANE_COMMANDS
            .into_iter()
            .filter(|item| item.command.starts_with(query))
            .map(command_from_template)
            .collect::<Vec<_>>()
    } else if should_suggest_lane_ids(query, &words) {
        lane_id_suggestions(query, &words, state)
    } else {
        Vec::new()
    };
    Some(suggestions)
}

fn should_suggest_lane_ids(query: &str, words: &[&str]) -> bool {
    if words.len() < 2 || words.len() > 3 {
        return false;
    }
    let base = format!("{} {}", words[0], words[1]);
    LANE_ID_COMMANDS.contains(&base.as_str())
        && (query.ends_with(' ')
            || words
                .get(2)
                .is_some_and(|value| value.starts_with('L') || value.starts_with('l')))
}

fn lane_id_suggestions(query: &str, words: &[&str], state: &TuiState) -> Vec<CommandSuggestion> {
    let base = format!("{} {}", words[0], words[1]);
    let partial_id = if query.ends_with(' ') {
        ""
    } else {
        words.get(2).copied().unwrap_or("")
    };
    state
        .lanes
        .iter()
        .filter(|lane| {
            lane.id
                .to_ascii_lowercase()
                .starts_with(&partial_id.to_ascii_lowercase())
        })
        .map(|lane| CommandSuggestion {
            command: format!("{base} {}", lane.id),
            summary: format!("{} [{}]", lane.title, lane.status),
        })
        .collect()
}

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
            pad(&suggestion.command, command_width),
            " ".repeat(2),
            pad(&suggestion.summary, detail_width)
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
            tasks: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    #[test]
    fn filters_slash_commands_by_prefix() {
        let suggestions = static_command_suggestions("/p");

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/provider", "/plan"]
        );
    }

    #[test]
    fn suggests_lane_subcommands_after_lane_space() {
        let state = state_with_input("/lane ");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(suggestions[0].command, "/lane codex");
        assert!(suggestions.iter().any(|item| item.command == "/lane apply"));
        assert!(is_command_palette_visible(&state));
    }

    #[test]
    fn filters_lane_subcommands_by_partial_argument() {
        let state = state_with_input("/lane a");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/lane attach", "/lane accept", "/lane apply"]
        );
    }

    #[test]
    fn suggests_lane_ids_for_lane_actions() {
        let state = state_with_input("/lane inspect l");

        let suggestions = command_suggestions_for_state(&state);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.command.as_str())
                .collect::<Vec<_>>(),
            vec!["/lane inspect L1", "/lane inspect L2", "/lane inspect L3"]
        );
        assert!(suggestions[0].summary.contains("[running]"));
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
        let mut state = state_with_input("/lane a");
        state.command_selection = 2;

        assert!(complete_selected(&mut state));

        assert_eq!(state.input, "/lane apply ");
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
