use super::{
    canvas::Frame,
    panel::{bordered_row, panel_top},
    state::TuiState,
    text::{bottom_border, char_width, pad, truncate},
};

pub(super) const COMPOSER_HEIGHT: usize = 4;

pub(super) fn render_composer(frame: &mut Frame, state: &TuiState, bottom_bar_height: usize) {
    let top = frame.height - COMPOSER_HEIGHT - bottom_bar_height;
    frame.write_line(top, &panel_top("RoboCode >_", frame.width, None));
    frame.write_line(top + 1, &composer_input_top_row(state, frame.width));
    frame.write_line(
        top + 2,
        &bordered_row(
            &composer_actions(frame.width.saturating_sub(4)),
            frame.width,
        ),
    );
    frame.write_line(top + 3, &bottom_border(frame.width));
}

fn composer_input_top_row(state: &TuiState, width: usize) -> String {
    let value = if state.input.is_empty() {
        "Type your instruction..."
    } else {
        state.input.as_str()
    };
    let content_width = width.saturating_sub(26);
    let input = format!(
        "› {}",
        pad(
            &truncate(value, content_width.saturating_sub(6)),
            content_width.saturating_sub(6)
        )
    );
    format!(
        "│ {}│ {} │",
        pad(&input, width.saturating_sub(18)),
        pad("MODE Code ▾", 12)
    )
}

pub(super) fn composer_cursor_position(
    state: &TuiState,
    terminal_width: u16,
    terminal_height: u16,
    bottom_bar_height: usize,
) -> (u16, u16) {
    let width = usize::from(terminal_width);
    let height = usize::from(terminal_height);
    let input_width = width.saturating_sub(24);
    let visible_input_width = input_width.saturating_sub(4);
    let input_len = char_width(&state.input).min(visible_input_width);
    let column = 4 + input_len;
    let row = height
        .saturating_sub(bottom_bar_height)
        .saturating_sub(COMPOSER_HEIGHT)
        + 1;
    (column as u16, row as u16)
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
    fn cursor_position_tracks_visible_input_end() {
        assert_eq!(
            composer_cursor_position(&state_with_input(""), 120, 40, 1),
            (4, 36)
        );
        assert_eq!(
            composer_cursor_position(&state_with_input("abc"), 120, 40, 1),
            (7, 36)
        );
    }
}

fn composer_actions(width: usize) -> String {
    let left = "APPROVAL MODE: [Suggest] [Auto Edit] [Plan] [Manual]";
    let right = "ACTIONS: [^J Send] [^K Clr] [^R Regenerate] [^N New Task] [? Help]";
    let left_width = char_width(left);
    let right_width = char_width(right);
    if left_width + right_width + 3 <= width {
        return format!(
            "{left}{} {right}",
            " ".repeat(width.saturating_sub(left_width + right_width + 1))
        );
    }
    if left_width <= width {
        return left.to_string();
    }
    truncate(&format!("{left}   {right}"), width)
}
