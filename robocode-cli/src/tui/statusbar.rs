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
        .filter(|lane| lane.status == "running" || lane.status == "queued")
        .count();
    let left = if frame.width >= 100 {
        format!(
            "● CONNECTED  ┆ SESSION {session:<8} ┆ TOKENS {tokens:<7}/ {ctx:<5} ┆ COST {cost:<7} ┆ TIME {time:<8}",
            tokens = token_estimate(state),
            ctx = state.provider_status.context_window,
            cost = cost_estimate(state),
            time = elapsed_label(state),
        )
    } else {
        format!(
            "● CONNECTED SES {session:<8} TOK {tokens:<7} L{active_lanes:<2}",
            tokens = token_estimate(state),
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

fn elapsed_label(state: &TuiState) -> String {
    let seconds = state.entries.len().max(1) as u64 * 13 + state.lanes.len() as u64 * 7;
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("00:{minutes:02}:{seconds:02}")
}

fn token_estimate(state: &TuiState) -> String {
    let chars = state
        .entries
        .iter()
        .map(|entry| entry.body.chars().count())
        .sum::<usize>();
    let tokens = (chars / 4).max(1);
    format!("{:.1}k", tokens as f32 / 1000.0)
}

fn cost_estimate(state: &TuiState) -> String {
    let tokens = state
        .entries
        .iter()
        .map(|entry| entry.body.chars().count())
        .sum::<usize>() as f32
        / 4.0;
    format!("${:.4}", tokens * 0.0000004)
}
