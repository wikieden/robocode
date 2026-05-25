use super::{
    canvas::Frame,
    panel::bordered_row,
    state::TuiState,
    text::{char_width, compact_middle, truncate},
};

pub(super) const BOTTOM_BAR_HEIGHT: usize = 1;

pub(super) fn render_bottom_bar(frame: &mut Frame, state: &TuiState) {
    let session = compact_middle(&state.session_id, 8);
    let active_lanes = state
        .lanes
        .iter()
        .filter(|lane| matches!(lane.status.as_str(), "running" | "queued" | "attached"))
        .count();
    let left = if frame.width >= 100 {
        format!(
            "● CONNECTED  ┆ SESSION {session:<8} ┆ EVENTS {events:<4} ┆ LANES {active_lanes:<2} ┆ CONTEXT {ctx:<5}",
            events = state.entries.len(),
            ctx = state.provider_status.context_window,
        )
    } else {
        format!(
            "● CONNECTED SES {session:<8} EVT {events:<3} L{active_lanes:<2}",
            events = state.entries.len(),
        )
    };
    let right = if frame.width >= 160 {
        format!("THEME {} ┆ HELP ?", state.theme_name,)
    } else if frame.width >= 120 {
        "Press ? for help".to_string()
    } else if frame.width >= 100 {
        "Press ?".to_string()
    } else {
        "? help".to_string()
    };
    frame.write_line(
        frame.height - 1,
        &bordered_row(
            &status_content(&left, &right, frame.width.saturating_sub(4)),
            frame.width,
        ),
    );
}

fn status_content(left: &str, right: &str, width: usize) -> String {
    let left_width = char_width(left);
    let right_width = char_width(right);
    if left_width + right_width + 3 <= width {
        return format!(
            "{left}{} {right}",
            " ".repeat(width.saturating_sub(left_width + right_width + 1))
        );
    }
    if right_width + 3 >= width {
        return truncate(left, width);
    }
    let left_width = width.saturating_sub(right_width + 3);
    format!("{}   {right}", truncate(left, left_width))
}
