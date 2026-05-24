use super::text::{bottom_border, char_width, horizontal, pad, truncate};

pub(super) fn panel(
    title: &str,
    rows: Vec<String>,
    width: usize,
    height: usize,
    badge: Option<&str>,
) -> Vec<String> {
    let width = width.max(12);
    let height = height.max(3);
    let mut lines = Vec::with_capacity(height);
    lines.push(panel_top(title, width, badge));
    let content_height = height - 2;
    for index in 0..content_height {
        let row = rows.get(index).map(String::as_str).unwrap_or("");
        lines.push(bordered_row(row, width));
    }
    lines.push(bottom_border(width));
    lines
}

pub(super) fn panel_top(title: &str, width: usize, badge: Option<&str>) -> String {
    let label = format!(" {title} ");
    let mut line = format!("┌{}", label);
    let badge = badge.map(|value| format!(" {value} "));
    let used = 1 + char_width(&label) + badge.as_ref().map_or(0, |value| char_width(value)) + 1;
    line.push_str(&horizontal(width.saturating_sub(used)));
    if let Some(badge) = badge {
        line.push_str(&badge);
    }
    line.push('┐');
    truncate(&line, width)
}

pub(super) fn bordered_row(content: &str, width: usize) -> String {
    format!("│ {} │", pad(content, width.saturating_sub(4)))
}
