use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    state::{TuiEntry, TuiState},
    text::{char_width, truncate, wrap_words},
};

pub(super) fn transcript_rows(state: &TuiState, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let visible_entries = state
        .entries
        .iter()
        .filter(|entry| !(entry.label == "approval" && entry.body.contains("Press y")))
        .collect::<Vec<_>>();
    let total = visible_entries.len();
    for (index, entry) in visible_entries.iter().enumerate() {
        let (icon, label) = transcript_role(entry.label.as_str());
        let offset = total.saturating_sub(index + 1) as u64 * 2;
        rows.push(header_row(icon, label, &clock_label(offset), width));
        rows.extend(body_rows(entry, width));
        if index + 1 < total {
            rows.push(separator_row(width));
        }
    }
    rows
}

fn header_row(icon: &str, label: &str, time: &str, width: usize) -> String {
    let left = format!("  {icon}  ┊  {label}");
    let left_width = char_width(&left);
    let time_width = char_width(time);
    if left_width + time_width + 1 >= width {
        return truncate(&format!("{left} {time}"), width);
    }
    format!(
        "{left}{}{}",
        " ".repeat(width.saturating_sub(left_width + time_width)),
        time
    )
}

fn body_rows(entry: &TuiEntry, width: usize) -> Vec<String> {
    if entry.label == "tool-call" {
        return tool_call_rows(&entry.body, width);
    }
    if entry.label == "approval" && entry.body.contains("Press y") {
        return Vec::new();
    }
    entry
        .body
        .lines()
        .flat_map(|line| body_rows_wrapped(entry, line, width))
        .collect()
}

fn body_rows_wrapped(entry: &TuiEntry, line: &str, width: usize) -> Vec<String> {
    let content = match entry.label.as_str() {
        "tool-result" => compact_tool_result(line),
        _ => line.to_string(),
    };
    wrap_words(&content, width.saturating_sub(8))
        .into_iter()
        .map(|line| format!("     ┊  {line}"))
        .map(|line| truncate(&line, width))
        .collect()
}

fn tool_call_rows(body: &str, width: usize) -> Vec<String> {
    body.lines()
        .flat_map(|line| structured_tool_call(line, width))
        .collect()
}

fn structured_tool_call(line: &str, width: usize) -> Vec<String> {
    let Some((tool, rest)) = line.split_once(" path: ") else {
        return vec![format!(
            "  ┊    {}",
            truncate(line, width.saturating_sub(7))
        )];
    };
    let (path, lines) = rest.split_once(" lines: ").unwrap_or((rest, "-"));
    let content_width = width.saturating_sub(5);
    vec![
        format!(
            "     ┊  [{:<10}] path: {}",
            truncate(tool, 10),
            truncate(path, content_width.saturating_sub(char_width(tool) + 18))
        ),
        format!(
            "     ┊  {:>12} lines: {:<9} gate: waiting",
            "",
            truncate(lines, 9)
        ),
    ]
    .into_iter()
    .map(|line| truncate(&line, width))
    .collect()
}

fn compact_tool_result(line: &str) -> String {
    if line.ends_with("completed") {
        return format!("✓ {line}");
    }
    if line.starts_with("Wrote ") {
        return format!("• {line}");
    }
    line.to_string()
}

fn transcript_role(label: &str) -> (&'static str, &'static str) {
    match label {
        "user" => ("♙", "USER"),
        "assistant" => ("✣", "ASSISTANT"),
        "tool-call" => ("⚒", "TOOL CALL"),
        "tool-result" => ("✓", "TOOL RESULT"),
        "approval" => ("◆", "APPROVAL"),
        _ => ("·", "SYSTEM"),
    }
}

fn separator_row(width: usize) -> String {
    let rule_width = width.saturating_sub(8).min(88);
    truncate(&format!("     ┊  {}", "┄".repeat(rule_width)), width)
}

fn clock_label(offset_secs: u64) -> String {
    let seconds = now_seconds().saturating_sub(offset_secs) % 86_400;
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{ProviderStatus, TerminalLane, TuiEntry, TuiState, WorkspaceSnapshot};

    fn state(entries: Vec<TuiEntry>) -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            entries,
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    #[test]
    fn transcript_rows_render_status_badges_and_tool_cards() {
        let rendered = transcript_rows(
            &state(vec![
                TuiEntry {
                    label: "user".to_string(),
                    body: "change config loader".to_string(),
                },
                TuiEntry {
                    label: "tool-call".to_string(),
                    body: "write_file path: src/config.rs lines: 1-120".to_string(),
                },
                TuiEntry {
                    label: "tool-result".to_string(),
                    body: "write_file completed\npath: src/config.rs\nsize: 2.1 KB\neffect: wrote 48 lines".to_string(),
                },
            ]),
            96,
        )
        .join("\n");

        assert!(rendered.contains("USER"));
        assert!(rendered.contains("TOOL CALL"));
        assert!(rendered.contains("[write_file]"));
        assert!(rendered.contains("path: src/config.rs"));
        assert!(rendered.contains("gate: waiting"));
        assert!(rendered.contains("TOOL RESULT"));
        assert!(!rendered.contains("[done]"));
        assert!(rendered.contains("✓ write_file completed"));
        assert!(rendered.contains("path: src/config.rs"));
        assert!(rendered.contains("size: 2.1 KB"));
        assert!(rendered.contains("effect: wrote 48 lines"));
    }

    #[test]
    fn transcript_rows_wrap_long_messages_without_hard_cutoff() {
        let rendered = transcript_rows(
            &state(vec![TuiEntry {
                label: "assistant".to_string(),
                body: "This message should wrap across multiple transcript rows instead of disappearing at the panel edge.".to_string(),
            }]),
            58,
        );

        assert!(rendered.iter().any(|line| line.contains("wrap across")));
        assert!(rendered.iter().any(|line| line.contains("panel edge.")));
        assert!(rendered.len() > 2);
    }
}
