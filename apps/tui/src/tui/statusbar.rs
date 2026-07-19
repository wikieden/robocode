use super::{
    canvas::Frame,
    input::effective_input_mode,
    state::{TuiState, has_active_work},
    text::{pad, truncate},
};

pub(super) const BOTTOM_BAR_HEIGHT: usize = 2;

pub(super) fn render_bottom_bar(frame: &mut Frame, state: &TuiState) {
    let row = frame.height.saturating_sub(BOTTOM_BAR_HEIGHT);
    let project = state
        .runtime
        .confirmed_project_config
        .as_ref()
        .and_then(|preview| preview.project_name.as_deref())
        .or_else(|| {
            state
                .runtime
                .project_probe
                .as_ref()
                .and_then(|probe| probe.project_name.as_deref())
        })
        .or_else(|| {
            state
                .runtime
                .snapshot
                .cwd
                .file_name()
                .and_then(|name| name.to_str())
        })
        .unwrap_or("-");
    let lane = state.ui.focused_lane.as_deref().unwrap_or("-");
    let mut identity = format!(" P:{} L:{}", truncate(project, 12), truncate(lane, 14));
    if frame.width >= 80 {
        identity.push_str(&format!(
            " M:{:?} {}",
            state.runtime.snapshot.work_mode,
            effective_input_mode(state).label()
        ));
    }
    if frame.width >= 112 {
        identity.push_str(&format!(" · {}", ambient_status(state)));
    }
    frame.write_line(row, &pad(&identity, frame.width));
    let mut pinned = format!(
        " PERM:{:?} A:{} G:{} E:{}",
        state.runtime.snapshot.permission_level,
        state.runtime.pending_approvals.len(),
        state.runtime.merge_gates.len(),
        state.runtime.errors.len()
    );
    if frame.width >= 80 {
        pinned.push_str(" · Ctrl-K commands · Ctrl-C cancel · Esc back");
    }
    frame.write_line(row + 1, &pad(&pinned, frame.width));
}

fn ambient_status(state: &TuiState) -> String {
    let activity = if has_active_work(state) {
        "ACTIVE"
    } else {
        "IDLE"
    };
    let provider = state
        .runtime
        .provider
        .as_ref()
        .map_or("-", |provider| provider.status.as_str());
    format!(
        "{activity} · EVENTS {} · TOKENS {} · PROVIDER {provider}",
        state.runtime.tasks.len() + state.runtime.lanes.len(),
        state.runtime.cost_ledger.total_tokens
    )
}

#[cfg(test)]
mod ambient_tests {
    use super::*;

    #[test]
    fn ambient_status_has_no_action_labels_or_shortcuts() {
        let status = ambient_status(&TuiState::default());

        for forbidden in ["GATE", "APPROVAL", "ERROR", "PERM", "Ctrl-"] {
            assert!(
                !status.contains(forbidden),
                "ambient ticker contains {forbidden}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{keymap::OverlayKind, state::OverlayState};

    #[test]
    fn status_bar_always_displays_the_explicit_input_mode() {
        let mut frame = Frame::new(100, 4);

        render_bottom_bar(&mut frame, &TuiState::default());

        assert!(frame.to_string().contains("NORMAL"));
    }

    #[test]
    fn status_bar_tracks_insert_and_overlay_ownership() {
        let mut state = TuiState::default();
        state.ui.input_mode = crate::tui::keymap::InputMode::Insert;
        let mut frame = Frame::new(100, 4);
        render_bottom_bar(&mut frame, &state);
        assert!(frame.to_string().contains("INSERT"));

        state.ui.overlay = Some(OverlayState::new(OverlayKind::ContextHelp));
        let mut frame = Frame::new(100, 4);
        render_bottom_bar(&mut frame, &state);
        assert!(frame.to_string().contains("OVERLAY"));
    }
}
