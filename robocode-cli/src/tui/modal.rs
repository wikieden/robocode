use super::{
    canvas::Frame,
    command_palette::render_command_suggestions,
    indicators::{progress_bar, status_dot},
    lane::{command_hint, interaction_hint, pid_hint, pty_label, terminal_label},
    panel::panel,
    state::{TerminalLane, TuiState, lane_runtime_evidence},
    text::{char_width, horizontal, pad, truncate},
};

const APPROVAL_FOCUS_APPLY_ALL: usize = 0;
const APPROVAL_FOCUS_DENY: usize = 1;
const APPROVAL_FOCUS_DIFF: usize = 2;
const APPROVAL_FOCUS_APPROVE: usize = 3;
pub(super) const DEFAULT_APPROVAL_FOCUS: usize = APPROVAL_FOCUS_APPROVE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalAction {
    ToggleApplyAll,
    Deny,
    Diff,
    Approve,
}

pub(super) fn render_overlays(frame: &mut Frame, state: &TuiState, right_rail_width: usize) {
    if let Some(lane) = focused_lane(state) {
        render_lane_modal(frame, state, lane, right_rail_width);
    }
    if let Some(approval) = latest_approval(state) {
        render_approval_modal(frame, approval, state, right_rail_width);
    } else {
        render_command_suggestions(frame, state);
    }
}

pub(super) fn has_pending_approval(state: &TuiState) -> bool {
    latest_approval(state).is_some()
}

pub(super) fn latest_approval(state: &TuiState) -> Option<&str> {
    for (index, entry) in state.entries.iter().enumerate().rev() {
        if entry.label != "approval" {
            continue;
        }
        if entry.body.contains("Press y") {
            return (!state.entries[index + 1..]
                .iter()
                .any(closes_pending_approval_modal))
            .then_some(entry.body.as_str());
        }
        if entry.body.contains("Approved") || entry.body.contains("Denied") {
            return None;
        }
    }
    None
}

fn closes_pending_approval_modal(entry: &super::state::TuiEntry) -> bool {
    matches!(
        entry.label.as_str(),
        "tool-result" | "assistant" | "command"
    ) || (entry.label == "approval" && !entry.body.contains("Press y"))
}

fn focused_lane(state: &TuiState) -> Option<&TerminalLane> {
    let id = state.focused_lane.as_deref()?;
    state
        .lanes
        .iter()
        .find(|lane| lane.id.eq_ignore_ascii_case(id))
}

fn render_lane_modal(
    frame: &mut Frame,
    state: &TuiState,
    lane: &TerminalLane,
    right_rail_width: usize,
) {
    let modal_width = frame
        .width
        .saturating_mul(2)
        .saturating_div(5)
        .clamp(54, 72);
    let modal_height = 16usize.min(frame.height.saturating_sub(4));
    let top = frame.height.saturating_sub(modal_height).saturating_div(2);
    let transcript_width = frame.width.saturating_sub(right_rail_width + 1);
    let centered_left = transcript_width
        .saturating_sub(modal_width)
        .saturating_div(2);
    let left = centered_left
        .max(22)
        .min(transcript_width.saturating_sub(modal_width));
    let mut rows = vec![
        format!(
            "{} {}  [{}]",
            lane.id,
            lane.tool,
            terminal_label(&lane.tool)
        ),
        format!(
            "PTY    {}  PID {}     ROUTE {}→{}",
            pty_label(&lane.tool),
            pid_hint(lane),
            truncate(&lane.target, 8),
            lane_screen_hint(lane)
        ),
        format!(
            "TASK   {}",
            truncate(&lane.title, modal_width.saturating_sub(11))
        ),
        format!(
            "STATE  {} {}",
            status_dot(&lane.status),
            progress_bar(lane.progress)
        ),
        format!(
            "CMD    {}",
            truncate(
                &command_hint(&lane.tool, &lane.title),
                modal_width.saturating_sub(11)
            )
        ),
        format!(
            "ATTACH {}",
            truncate(&interaction_hint(lane), modal_width.saturating_sub(11))
        ),
        scan_divider(modal_width),
        "LATEST OUTPUT".to_string(),
    ];
    rows.extend(lane_latest_output_rows(
        state,
        lane,
        modal_width.saturating_sub(6),
        3,
    ));
    rows.extend([
        scan_divider(modal_width),
        "CONTROL [stop] [tmux] [pty] [send] [inspect]".to_string(),
        "SIDE    --tui-screen side-1   live tail".to_string(),
    ]);
    let modal = panel(
        "LANE DETAIL",
        rows,
        modal_width,
        modal_height,
        Some("focus"),
    );
    clear_overlay_bounds(frame, top, modal_height, transcript_width);
    render_modal_shadow(frame, top, left, modal_width, modal_height);
    frame.write_block(top, left, &modal);
}

fn lane_latest_output_rows(
    state: &TuiState,
    lane: &TerminalLane,
    max_width: usize,
    max_lines: usize,
) -> Vec<String> {
    let tail = state
        .lane_store
        .as_deref()
        .and_then(|store| lane_runtime_evidence(store, &lane.id))
        .map(|evidence| evidence.log_tail)
        .unwrap_or_default();
    if tail.is_empty() {
        return vec![format!("  {}", truncate(&lane.summary, max_width))];
    }
    let keep_from = tail.len().saturating_sub(max_lines);
    tail.iter()
        .skip(keep_from)
        .map(|line| format!("  {}", truncate(line, max_width)))
        .collect()
}

fn lane_screen_hint(lane: &TerminalLane) -> &'static str {
    match lane.tool.as_str() {
        "codex" | "claude" => "side-1",
        "shell" | "run" => "side-2",
        _ => "main",
    }
}

fn render_approval_modal(
    frame: &mut Frame,
    approval: &str,
    state: &TuiState,
    right_rail_width: usize,
) {
    let details = ApprovalDetails::parse(approval);
    let bounds = approval_modal_bounds(frame.width, frame.height, right_rail_width);
    let mut rows = vec![
        format!(
            "APPROVAL REQUIRED: {:<14} ID: call_7f2a9c1e",
            truncate(details.tool, 14)
        ),
        format!(
            "PATH    {}",
            truncate(details.path, bounds.width.saturating_sub(12))
        ),
        "ACTION  Write (new content)  [MODIFIES FILE]".to_string(),
        "SIZE    +48 lines (2.1 KB)".to_string(),
        "PREVIEW (first 20 lines)".to_string(),
    ];
    rows.extend(code_preview_rows(&details, bounds.width));
    rows.extend([
        apply_all_row(state),
        format!(
            "{}{}{}",
            pad(
                &approval_button("[Deny (n)]", APPROVAL_FOCUS_DENY, state),
                20
            ),
            pad(&approval_button("[Diff]", APPROVAL_FOCUS_DIFF, state), 16),
            approval_button("[Approve (y)]", APPROVAL_FOCUS_APPROVE, state)
        ),
    ]);
    let modal = panel(
        "APPROVAL",
        rows,
        bounds.width,
        bounds.height,
        Some("tab/enter/click"),
    );
    clear_overlay_bounds(frame, bounds.top, bounds.height, bounds.transcript_width);
    render_modal_shadow(frame, bounds.top, bounds.left, bounds.width, bounds.height);
    frame.write_block(bounds.top, bounds.left, &modal);
}

pub(super) fn approval_action_at(
    state: &TuiState,
    column: u16,
    row: u16,
    frame_width: u16,
    frame_height: u16,
    right_rail_width: usize,
) -> Option<ApprovalAction> {
    latest_approval(state)?;
    let bounds = approval_modal_bounds(
        frame_width as usize,
        frame_height as usize,
        right_rail_width,
    );
    let column = column as usize;
    let row = row as usize;
    if row == bounds.apply_row() && column >= bounds.left + 2 && column < bounds.left + bounds.width
    {
        return Some(ApprovalAction::ToggleApplyAll);
    }
    if row != bounds.action_row() {
        return None;
    }
    let content_left = bounds.left + 2;
    if (content_left..content_left + 20).contains(&column) {
        Some(ApprovalAction::Deny)
    } else if (content_left + 20..content_left + 36).contains(&column) {
        Some(ApprovalAction::Diff)
    } else if (content_left + 36..bounds.left + bounds.width).contains(&column) {
        Some(ApprovalAction::Approve)
    } else {
        None
    }
}

pub(super) fn approval_focus_cursor(
    state: &TuiState,
    frame_width: u16,
    frame_height: u16,
    right_rail_width: usize,
) -> Option<(u16, u16)> {
    latest_approval(state)?;
    let bounds = approval_modal_bounds(
        frame_width as usize,
        frame_height as usize,
        right_rail_width,
    );
    let (column, row) = match focused_approval_action(state) {
        ApprovalAction::ToggleApplyAll => (bounds.left + 2, bounds.apply_row()),
        ApprovalAction::Deny => (bounds.left + 2, bounds.action_row()),
        ApprovalAction::Diff => (bounds.left + 22, bounds.action_row()),
        ApprovalAction::Approve => (bounds.left + 38, bounds.action_row()),
    };
    Some((column as u16, row as u16))
}

pub(super) fn move_approval_focus(state: &mut TuiState, delta: i8) {
    let current = state.approval_focus.min(APPROVAL_FOCUS_APPROVE);
    state.approval_focus = if delta < 0 {
        current.saturating_sub(1)
    } else {
        (current + 1).min(APPROVAL_FOCUS_APPROVE)
    };
}

pub(super) fn set_approval_focus_for_action(state: &mut TuiState, action: ApprovalAction) {
    state.approval_focus = match action {
        ApprovalAction::ToggleApplyAll => APPROVAL_FOCUS_APPLY_ALL,
        ApprovalAction::Deny => APPROVAL_FOCUS_DENY,
        ApprovalAction::Diff => APPROVAL_FOCUS_DIFF,
        ApprovalAction::Approve => APPROVAL_FOCUS_APPROVE,
    };
}

pub(super) fn focused_approval_action(state: &TuiState) -> ApprovalAction {
    match state.approval_focus {
        APPROVAL_FOCUS_DENY => ApprovalAction::Deny,
        APPROVAL_FOCUS_DIFF => ApprovalAction::Diff,
        APPROVAL_FOCUS_APPROVE => ApprovalAction::Approve,
        _ => ApprovalAction::ToggleApplyAll,
    }
}

fn apply_all_row(state: &TuiState) -> String {
    let checkbox = if state.approval_apply_all {
        "[x]"
    } else {
        "[ ]"
    };
    let marker = if state.approval_focus == APPROVAL_FOCUS_APPLY_ALL {
        "› "
    } else {
        "  "
    };
    format!("{marker}{checkbox} Apply to all write_file calls in this session")
}

fn approval_button(label: &str, focus: usize, state: &TuiState) -> String {
    if state.approval_focus == focus {
        format!("› {label}")
    } else {
        format!("  {label}")
    }
}

#[derive(Debug, Clone, Copy)]
struct ApprovalBounds {
    top: usize,
    left: usize,
    width: usize,
    height: usize,
    transcript_width: usize,
}

impl ApprovalBounds {
    fn apply_row(self) -> usize {
        self.top + self.height.saturating_sub(3)
    }

    fn action_row(self) -> usize {
        self.top + self.height.saturating_sub(2)
    }
}

fn approval_modal_bounds(
    frame_width: usize,
    frame_height: usize,
    right_rail_width: usize,
) -> ApprovalBounds {
    let width = frame_width.saturating_div(2).clamp(56, 64);
    let height = 15usize.min(frame_height.saturating_sub(4));
    let top = frame_height
        .saturating_sub(height)
        .saturating_div(3)
        .saturating_add(1)
        .min(frame_height.saturating_sub(height));
    let transcript_width = frame_width.saturating_sub(right_rail_width + 1);
    let centered_left = transcript_width.saturating_sub(width).saturating_div(2);
    let left = centered_left
        .max(22)
        .min(transcript_width.saturating_sub(width));
    ApprovalBounds {
        top,
        left,
        width,
        height,
        transcript_width,
    }
}

fn scan_divider(modal_width: usize) -> String {
    let width = modal_width.saturating_sub(4).min(64);
    "┄".repeat(width)
}

fn code_preview_rows(details: &ApprovalDetails<'_>, modal_width: usize) -> Vec<String> {
    let box_width = modal_width.saturating_sub(8).max(28);
    let label = format!(" {} ", truncate(details.path, box_width.saturating_sub(6)));
    let top_rule = horizontal(box_width.saturating_sub(char_width(&label) + 2));
    let bottom_rule = horizontal(box_width.saturating_sub(2));
    let line_width = box_width.saturating_sub(10);
    let preview_lines = code_preview_lines(details);
    let mut rows = vec![format!("  ┌{label}{top_rule}┐")];
    rows.extend(preview_lines.iter().enumerate().map(|(index, line)| {
        format!(
            "  │ +{:>2} │ {} │",
            index + 1,
            pad(&truncate(line, line_width), line_width)
        )
    }));
    rows.push(format!("  └{bottom_rule}┘"));
    rows
}

fn code_preview_lines(details: &ApprovalDetails<'_>) -> [&'static str; 4] {
    if details.tool == "write_file" {
        return [
            "use std::{fs, path::Path};",
            "use anyhow::{Context, Result};",
            "pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {",
            "    let content = fs::read_to_string(path.as_ref())?;",
        ];
    }
    [
        "let command = PermissionRequest::current();",
        "let result = workspace.apply(command)?;",
        "session.append_event(result.summary());",
        "Ok(())",
    ]
}

fn render_modal_shadow(frame: &mut Frame, top: usize, left: usize, width: usize, height: usize) {
    let _ = (frame, top, left, width, height);
}

fn clear_overlay_bounds(frame: &mut Frame, top: usize, height: usize, transcript_width: usize) {
    let clear_top = top.saturating_sub(1);
    let clear_left = 1;
    let clear_width = transcript_width
        .saturating_sub(2)
        .min(frame.width.saturating_sub(clear_left));
    let clear_height = (height + 1).min(frame.height.saturating_sub(clear_top));
    frame.fill_rect_pattern(
        clear_top,
        clear_left,
        clear_width,
        clear_height,
        |_x, _y| ' ',
    );
}

#[derive(Debug, Clone, Copy)]
struct ApprovalDetails<'a> {
    tool: &'a str,
    path: &'a str,
}

impl<'a> ApprovalDetails<'a> {
    fn parse(value: &'a str) -> Self {
        let mut lines = value.lines();
        let first = lines.next().unwrap_or("Permission request");
        let message = lines.next().unwrap_or("");
        let preview = lines.next().unwrap_or("");
        let tool = first
            .split('`')
            .nth(1)
            .filter(|value| !value.is_empty())
            .unwrap_or("tool action");
        let path = message
            .strip_prefix("path: ")
            .or_else(|| preview.strip_prefix("path: "))
            .unwrap_or("current session");
        Self { tool, path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{
        ProviderStatus, TerminalLane, TuiEntry, WorkspaceSnapshot, lane_store_path,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn approval_state() -> TuiState {
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
            approval_focus: DEFAULT_APPROVAL_FOCUS,
            approval_apply_all: false,
            pending_turn: None,
            entries: vec![TuiEntry {
                label: "approval".to_string(),
                body: "Permission request for `write_file`\npath: src/lib.rs\nPress y to allow, n/Esc to deny.".to_string(),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    #[test]
    fn latest_approval_stops_after_resolution_entry() {
        let mut state = approval_state();
        assert!(latest_approval(&state).is_some());

        state.entries.push(TuiEntry {
            label: "approval".to_string(),
            body: "Approved `write_file`.".to_string(),
        });

        assert!(latest_approval(&state).is_none());
    }

    #[test]
    fn latest_approval_stops_after_runtime_closure_event() {
        let mut state = approval_state();
        state.entries.push(TuiEntry {
            label: "command".to_string(),
            body: "Test result:\n  status: failed".to_string(),
        });

        assert!(latest_approval(&state).is_none());
    }

    #[test]
    fn approval_mouse_hit_testing_maps_footer_actions() {
        let state = approval_state();
        let bounds = approval_modal_bounds(140, 36, 38);

        assert_eq!(
            approval_action_at(
                &state,
                (bounds.left + 4) as u16,
                bounds.action_row() as u16,
                140,
                36,
                38,
            ),
            Some(ApprovalAction::Deny)
        );
        assert_eq!(
            approval_action_at(
                &state,
                (bounds.left + 40) as u16,
                bounds.action_row() as u16,
                140,
                36,
                38,
            ),
            Some(ApprovalAction::Approve)
        );
    }

    #[test]
    fn approval_focus_cursor_tracks_selected_action() {
        let mut state = approval_state();
        state.approval_focus = APPROVAL_FOCUS_APPROVE;
        let bounds = approval_modal_bounds(140, 36, 38);

        assert_eq!(
            approval_focus_cursor(&state, 140, 36, 38),
            Some(((bounds.left + 38) as u16, bounds.action_row() as u16))
        );
    }

    #[test]
    fn default_approval_focus_is_approve_for_fast_enter() {
        let state = approval_state();

        assert_eq!(focused_approval_action(&state), ApprovalAction::Approve);
    }

    #[test]
    fn focused_lane_latest_output_prefers_persisted_log_tail() {
        let root = temp_root("lane-modal-tail");
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".robocode").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "old line\ncargo test --workspace\nfinished cleanly\n",
        )
        .expect("lane log");
        let mut state = approval_state();
        state.lane_store = Some(lane_store);
        let lane = state.lanes.first().expect("preview lane");

        let rows = lane_latest_output_rows(&state, lane, 80, 2).join("\n");

        assert!(rows.contains("cargo test --workspace"));
        assert!(rows.contains("finished cleanly"));
        assert!(!rows.contains("patched failing tests"));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(suffix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("robocode-modal-test-{nanos}-{suffix}"))
    }
}
