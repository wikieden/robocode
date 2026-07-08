use super::{
    canvas::Frame,
    panel::{bordered_row, panel_top},
    state::TuiState,
    text::{bottom_border, char_width, pad, truncate},
};

pub(super) const COMPOSER_HEIGHT: usize = 6;
const WELCOME_BOX_MAX_WIDTH: usize = 96;
const WELCOME_BOX_MIN_WIDTH: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ComposerAnchor {
    pub(super) left: usize,
    pub(super) input_row: usize,
    pub(super) width: usize,
}

pub(super) fn should_render_welcome(state: &TuiState) -> bool {
    state.focused_lane.is_none()
        && state.pending_turn.is_none()
        && !state.entries.iter().any(is_session_entry)
}

pub(super) fn render_welcome(frame: &mut Frame, state: &TuiState) {
    let width = frame.width;
    let height = frame.height;
    let box_width = welcome_box_width(width);
    let box_left = width.saturating_sub(box_width) / 2;
    let composer_top = welcome_composer_top(width, height);
    let logo_top = composer_top.saturating_sub(if width >= 100 { 8 } else { 4 });

    let logo = welcome_logo(width);
    for (offset, line) in logo.iter().enumerate() {
        frame.write_line(logo_top + offset, &centered_line(width, line));
    }

    frame.write_line(
        composer_top + 1,
        &positioned_line(width, box_left, &welcome_input_row(state, box_width)),
    );
    frame.write_line(
        composer_top + 2,
        &positioned_line(width, box_left, &welcome_spacer_row(box_width)),
    );
    frame.write_line(
        composer_top + 3,
        &positioned_line(width, box_left, &welcome_context_row(state, box_width)),
    );
    frame.write_line(
        composer_top + 5,
        &positioned_line(width, box_left, &welcome_hint_row(box_width)),
    );

    let bottom = welcome_status_row(state, width);
    frame.write_line(height.saturating_sub(2), &bottom);
}

pub(super) fn render_composer(frame: &mut Frame, state: &TuiState, bottom_bar_height: usize) {
    let top = frame.height - COMPOSER_HEIGHT - bottom_bar_height;
    frame.write_line(top, &panel_top("Viden >_", frame.width, None));
    frame.write_line(top + 1, &composer_input_spacer_row(frame.width));
    frame.write_line(top + 2, &composer_input_row(state, frame.width));
    frame.write_line(top + 3, &composer_input_spacer_row(frame.width));
    frame.write_line(
        top + 4,
        &bordered_row(
            &composer_actions(state, frame.width.saturating_sub(4)),
            frame.width,
        ),
    );
    frame.write_line(top + 5, &bottom_border(frame.width));
}

fn composer_input_row(state: &TuiState, width: usize) -> String {
    let value = if !state.input.is_empty() {
        state.input.clone()
    } else if let Some(turn) = &state.pending_turn {
        if turn.queued_inputs.is_empty() {
            "Type next prompt while Viden works...".to_string()
        } else {
            format!(
                "{} queued; type another prompt...",
                queued_prompt_count_label(turn.queued_inputs.len())
            )
        }
    } else {
        "Type your instruction...".to_string()
    };
    let content_width = width.saturating_sub(26);
    let input = format!(
        "› {}",
        pad(
            &truncate(&value, content_width.saturating_sub(6)),
            content_width.saturating_sub(6)
        )
    );
    let mode_chip = format!("MODE {} ▾", state.provider_status.work_mode.label());
    format!(
        "│ {}│ {} │",
        pad(&input, width.saturating_sub(18)),
        pad(&truncate(&mode_chip, 12), 12)
    )
}

fn queued_prompt_count_label(count: usize) -> String {
    if count == 1 {
        "1 prompt".to_string()
    } else {
        format!("{count} prompts")
    }
}

fn composer_input_spacer_row(width: usize) -> String {
    format!("│ {}│ {} │", pad("", width.saturating_sub(18)), pad("", 12))
}

pub(super) fn composer_cursor_position(
    state: &TuiState,
    terminal_width: u16,
    terminal_height: u16,
    bottom_bar_height: usize,
) -> (u16, u16) {
    let width = usize::from(terminal_width);
    let height = usize::from(terminal_height);
    if should_render_welcome(state) {
        return welcome_cursor_position(state, width, height);
    }
    let input_width = width.saturating_sub(24);
    let visible_input_width = input_width.saturating_sub(4);
    let input_len = char_width(&state.input).min(visible_input_width);
    let column = 4 + input_len;
    let row = height
        .saturating_sub(bottom_bar_height)
        .saturating_sub(COMPOSER_HEIGHT)
        + 2;
    (column as u16, row as u16)
}

pub(super) fn composer_anchor(
    state: &TuiState,
    width: usize,
    height: usize,
    bottom_bar_height: usize,
) -> ComposerAnchor {
    if should_render_welcome(state) {
        let box_width = welcome_box_width(width);
        let box_left = width.saturating_sub(box_width) / 2;
        return ComposerAnchor {
            left: box_left,
            input_row: welcome_composer_top(width, height) + 1,
            width: box_width,
        };
    }

    ComposerAnchor {
        left: 0,
        input_row: height
            .saturating_sub(bottom_bar_height)
            .saturating_sub(COMPOSER_HEIGHT)
            + 2,
        width,
    }
}

fn composer_actions(state: &TuiState, width: usize) -> String {
    let left = format!(
        "MODE [{}]  PERM [{}]",
        state.provider_status.work_mode.label(),
        state.provider_status.permission_level.label()
    );
    let right = if state.pending_turn.is_some() {
        "ACTIONS: [^J Queue] [^C Cancel] [PgUp History] [? Help]"
    } else {
        "ACTIONS: [^J Send] [^K Clr] [^R Regenerate] [^N New Task] [? Help]"
    };
    let left_width = char_width(&left);
    let right_width = char_width(right);
    if left_width + right_width + 3 <= width {
        return format!(
            "{}{} {right}",
            left,
            " ".repeat(width.saturating_sub(left_width + right_width + 1))
        );
    }
    if left_width <= width {
        return left;
    }
    truncate(
        &format!(
            "MODE {}  PERMISSIONS {}",
            state.provider_status.work_mode.label(),
            state.provider_status.permission_level.label()
        ),
        width,
    )
}

fn is_session_entry(entry: &super::state::TuiEntry) -> bool {
    match entry.label.as_str() {
        "user" => !entry.body.trim_start().starts_with('/'),
        "command" => is_work_command_entry(&entry.body),
        "assistant" | "tool-call" | "tool-result" | "approval" => true,
        _ => false,
    }
}

fn is_work_command_entry(body: &str) -> bool {
    body.contains("Latest diff:")
        || body.contains("Test result:")
        || body.contains("Tool result:")
        || body.contains("Shell result:")
}

fn welcome_box_width(width: usize) -> usize {
    width
        .saturating_sub(10)
        .min(WELCOME_BOX_MAX_WIDTH)
        .max(WELCOME_BOX_MIN_WIDTH.min(width))
}

fn welcome_composer_top(width: usize, height: usize) -> usize {
    let target = if width >= 100 {
        height.saturating_mul(55) / 100
    } else {
        height.saturating_mul(45) / 100
    };
    target.min(height.saturating_sub(8)).max(6)
}

fn welcome_logo(width: usize) -> Vec<&'static str> {
    if width < 100 {
        return vec!["Viden"];
    }
    vec![
        "        /\\        ",
        "   ____/  \\____   ",
        "  / __  /\\  __ \\  ",
        " /_/  |/__\\|  \\_\\ ",
        " |  []  /\\  []  | ",
        " |_____/  \\_____| ",
        "    /_VIDEN_\\  ",
    ]
}

fn welcome_cursor_position(state: &TuiState, width: usize, height: usize) -> (u16, u16) {
    let box_width = welcome_box_width(width);
    let box_left = width.saturating_sub(box_width) / 2;
    let input_width = box_width.saturating_sub(6);
    let input_len = char_width(&state.input).min(input_width);
    let column = box_left + 2 + input_len;
    let row = welcome_composer_top(width, height) + 1;
    (column as u16, row as u16)
}

fn welcome_input_row(state: &TuiState, box_width: usize) -> String {
    let (value, left_pad) = if state.input.is_empty() {
        ("Ask anything... \"Fix broken tests\"", " ")
    } else {
        (state.input.as_str(), "")
    };
    let value_width = box_width.saturating_sub(4 + char_width(left_pad));
    format!(
        "▌ {left_pad}{}",
        pad(
            &truncate(value, value_width),
            box_width - 2 - char_width(left_pad)
        )
    )
}

fn welcome_spacer_row(box_width: usize) -> String {
    format!("▌ {}", pad("", box_width - 2))
}

fn welcome_context_row(state: &TuiState, box_width: usize) -> String {
    let provider = provider_display_name(state);
    let content = format!("▌ Viden - Operator · {} {}", state.model, provider);
    truncate(&pad(&content, box_width), box_width)
}

fn welcome_hint_row(box_width: usize) -> String {
    let hint = "tab agents   ctrl+p commands   /connect";
    format!(
        "{}{}",
        " ".repeat(box_width.saturating_sub(char_width(hint))),
        hint
    )
}

fn welcome_status_row(state: &TuiState, width: usize) -> String {
    let left = truncate(
        &format!(
            "{}:{}",
            state.workspace.display_root, state.workspace.git_branch
        ),
        width.saturating_mul(45) / 100,
    );
    let middle = format!("◎ {} lanes  /status", state.lanes.len());
    let right = format!("v{}", env!("CARGO_PKG_VERSION"));
    let left_width = char_width(&left);
    let middle_width = char_width(&middle);
    let right_width = char_width(&right);
    if left_width + middle_width + right_width + 6 > width {
        return pad(&format!("{left}  {right}"), width);
    }
    let middle_col = width.saturating_sub(middle_width) / 2;
    let right_col = width.saturating_sub(right_width).saturating_sub(2);
    let first_gap = middle_col.saturating_sub(left_width);
    let second_gap = right_col.saturating_sub(middle_col + middle_width);
    pad(
        &format!(
            "{left}{}{middle}{}{right}",
            " ".repeat(first_gap),
            " ".repeat(second_gap)
        ),
        width,
    )
}

fn provider_display_name(state: &TuiState) -> String {
    state
        .provider_catalog
        .iter()
        .find(|provider| provider.provider_id == state.provider)
        .map(|provider| provider.display_name.clone())
        .unwrap_or_else(|| state.provider.clone())
}

fn centered_line(width: usize, value: &str) -> String {
    let left = width.saturating_sub(char_width(value)) / 2;
    positioned_line(width, left, value)
}

fn positioned_line(width: usize, left: usize, value: &str) -> String {
    let mut line = " ".repeat(left.min(width));
    line.push_str(value);
    pad(&line, width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{ProviderStatus, TerminalLane, WorkspaceSnapshot};
    use viden_types::{PermissionLevel, WorkMode};

    fn state_with_input(input: &str) -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: input.to_string(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![super::super::state::TuiEntry {
                label: "assistant".to_string(),
                body: "hello".to_string(),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        }
    }

    #[test]
    fn cursor_position_tracks_visible_input_end() {
        assert_eq!(
            composer_cursor_position(&state_with_input(""), 120, 40, 1),
            (4, 35)
        );
        assert_eq!(
            composer_cursor_position(&state_with_input("abc"), 120, 40, 1),
            (7, 35)
        );
    }

    #[test]
    fn composer_uses_taller_input_area() {
        let mut frame = Frame::new(120, 40);
        render_composer(&mut frame, &state_with_input("hello"), 1);
        let rendered = frame.to_string();
        let lines = rendered.lines().collect::<Vec<_>>();

        assert_eq!(COMPOSER_HEIGHT, 6);
        assert!(lines[33].contains("Viden >_"));
        assert!(lines[34].contains("│"));
        assert!(lines[35].contains("› hello"));
        assert!(lines[36].contains("│"));
        assert!(lines[37].contains("MODE"));
        assert!(lines[37].contains("PERM"));
    }

    #[test]
    fn composer_actions_use_runtime_mode_and_permission_state() {
        let mut state = state_with_input("hello");
        state.provider_status.work_mode = WorkMode::Plan;
        state.provider_status.permission_level = PermissionLevel::ReadOnly;

        let mut frame = Frame::new(120, 40);
        render_composer(&mut frame, &state, 1);
        let rendered = frame.to_string();

        assert!(rendered.contains("MODE [Plan]"));
        assert!(rendered.contains("PERM [Read Only]"));
        assert!(rendered.contains("MODE Plan"));
        assert!(!rendered.contains("MODE Code"));
    }

    #[test]
    fn composer_invites_next_prompt_during_active_turn() {
        let mut state = state_with_input("");
        state.pending_turn = Some(super::super::state::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "first task",
            "/tmp/project",
        ));

        let mut frame = Frame::new(120, 40);
        render_composer(&mut frame, &state, 1);
        let rendered = frame.to_string();

        assert!(rendered.contains("Type next prompt while Viden works"));

        state
            .pending_turn
            .as_mut()
            .expect("pending turn")
            .queued_inputs
            .push("follow up".to_string());
        let mut frame = Frame::new(120, 40);
        render_composer(&mut frame, &state, 1);
        let rendered = frame.to_string();

        assert!(rendered.contains("1 prompt queued; type another prompt"));
    }

    #[test]
    fn composer_actions_show_queue_and_cancel_during_active_turn() {
        let mut state = state_with_input("");
        state.pending_turn = Some(super::super::state::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "first task",
            "/tmp/project",
        ));

        let mut frame = Frame::new(120, 40);
        render_composer(&mut frame, &state, 1);
        let rendered = frame.to_string();

        assert!(rendered.contains("[^J Queue]"));
        assert!(rendered.contains("[^C Cancel]"));
        assert!(!rendered.contains("[^R Regenerate]"));
    }

    #[test]
    fn cursor_sits_on_middle_input_row_for_ime_candidate_placement() {
        assert_eq!(
            composer_cursor_position(&state_with_input("你好"), 120, 40, 1),
            (8, 35)
        );
    }

    #[test]
    fn welcome_cursor_tracks_centered_input() {
        let mut state = state_with_input("hello");
        state.entries = vec![super::super::state::TuiEntry {
            label: "system".to_string(),
            body: "Viden TUI ready. Enter submits.".to_string(),
        }];

        let (column, row) = composer_cursor_position(&state, 140, 40, 1);

        assert!(column > 20);
        assert_eq!(row, 23);
    }

    #[test]
    fn welcome_empty_placeholder_starts_after_cursor_cell() {
        let mut state = state_with_input("");
        state.entries = vec![super::super::state::TuiEntry {
            label: "system".to_string(),
            body: "Viden TUI ready. Enter submits.".to_string(),
        }];
        let mut frame = Frame::new(140, 40);
        render_welcome(&mut frame, &state);
        let rendered = frame.to_string();
        let (column, row) = composer_cursor_position(&state, 140, 40, 1);
        let line = rendered.lines().nth(row as usize).expect("cursor row");

        assert_eq!(line.chars().nth(column as usize), Some(' '));
        assert_eq!(line.chars().nth(column as usize + 1), Some('A'));
    }

    #[test]
    fn welcome_renders_mecha_logo_on_wide_terminals() {
        let mut state = state_with_input("");
        state.entries = vec![super::super::state::TuiEntry {
            label: "system".to_string(),
            body: "Viden TUI ready. Enter submits.".to_string(),
        }];
        let mut frame = Frame::new(140, 40);
        render_welcome(&mut frame, &state);
        let rendered = frame.to_string();

        assert!(rendered.contains("/_VIDEN_\\"));
        assert!(rendered.contains("[]"));
    }

    #[test]
    fn welcome_survives_slash_setup_entries_until_user_starts_session() {
        let mut state = state_with_input("");
        state.entries = vec![
            super::super::state::TuiEntry {
                label: "system".to_string(),
                body: "Viden TUI ready. Enter submits.".to_string(),
            },
            super::super::state::TuiEntry {
                label: "user".to_string(),
                body: "/connect".to_string(),
            },
            super::super::state::TuiEntry {
                label: "settings".to_string(),
                body: "Provider switched to deepseek.".to_string(),
            },
            super::super::state::TuiEntry {
                label: "setup".to_string(),
                body: "Provider setup completed.".to_string(),
            },
        ];

        assert!(should_render_welcome(&state));

        state.entries.push(super::super::state::TuiEntry {
            label: "user".to_string(),
            body: "fix broken tests".to_string(),
        });

        assert!(!should_render_welcome(&state));
    }
}
