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
    let work_mode = state.provider_status.work_mode.label();
    let permission_level = state.provider_status.permission_level.label();
    let left = if frame.width >= 100 {
        format!(
            "● CONNECTED  ┆ MODE {work_mode:<5} ┆ PERM {permission_level:<9} ┆ SESSION {session:<8} ┆ EVENTS {events:<4} ┆ LANES {active_lanes:<2} ┆ CONTEXT {ctx:<5}",
            events = state.entries.len(),
            ctx = state.provider_status.context_window,
        )
    } else {
        format!(
            "● CONNECTED MODE {work_mode} PERM {permission_level} SES {session:<8} EVT {events:<3} L{active_lanes:<2}",
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

#[cfg(test)]
mod tests {
    use super::{BOTTOM_BAR_HEIGHT, render_bottom_bar};
    use crate::tui::{
        canvas::Frame,
        state::{ProviderOption, ProviderStatus, TuiState, WorkspaceSnapshot},
    };
    use viden_types::{PermissionLevel, WorkMode};

    #[test]
    fn bottom_bar_reflects_runtime_mode_and_permission_level() {
        let mut provider_status = ProviderStatus::configured();
        provider_status.work_mode = WorkMode::Plan;
        provider_status.permission_level = PermissionLevel::ReadOnly;
        let mut frame = Frame::new(140, BOTTOM_BAR_HEIGHT + 4);
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status,
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        render_bottom_bar(&mut frame, &state);
        let rendered = frame.to_string();

        assert!(rendered.contains("MODE Plan"));
        assert!(rendered.contains("PERM Read Only"));
    }
}
