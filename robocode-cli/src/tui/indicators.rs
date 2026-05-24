use super::state::TuiState;

pub(super) fn progress_bar(progress: u8) -> String {
    let filled = (progress as usize / 20).clamp(0, 5);
    let empty = 5usize.saturating_sub(filled);
    format!(
        "{}{} {:>3}%",
        "▓".repeat(filled),
        "░".repeat(empty),
        progress
    )
}

pub(super) fn status_dot(status: &str) -> &'static str {
    match status {
        "running" => "●",
        "queued" => "◐",
        "completed" => "✓",
        "failed" => "E",
        "stopped" => "■",
        _ => "○",
    }
}

pub(super) fn latency_label(state: &TuiState) -> String {
    let latency = 180 + state.entries.len() * 29;
    format!("{latency} ms")
}

pub(super) fn throughput_label(state: &TuiState) -> String {
    let throughput = 18.0 + state.lanes.len() as f32 * 2.7 + state.entries.len() as f32 * 0.4;
    format!("{throughput:.1}")
}

pub(super) fn rate_limit_bar(state: &TuiState) -> String {
    let percent = rate_limit_percent(state);
    let filled = (percent / 12).clamp(1, 8);
    let empty = 8usize.saturating_sub(filled);
    format!(
        "{}{} {:>2}%",
        "▓".repeat(filled),
        "░".repeat(empty),
        percent
    )
}

pub(super) fn rate_limit_percent(state: &TuiState) -> usize {
    (28 + state.entries.len() * 5 + state.lanes.len() * 3).min(96)
}

pub(super) fn context_percent(state: &TuiState) -> usize {
    (state.entries.len() * 3 + state.lanes.len()).clamp(1, 99)
}
