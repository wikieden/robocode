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
