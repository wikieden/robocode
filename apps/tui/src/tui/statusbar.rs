use super::{
    canvas::Frame,
    input::effective_input_mode,
    state::{TuiState, has_active_work},
    text::pad,
};

pub(super) const BOTTOM_BAR_HEIGHT: usize = 2;

pub(super) fn render_bottom_bar(frame: &mut Frame, state: &TuiState) {
    let row = frame.height.saturating_sub(BOTTOM_BAR_HEIGHT);
    let activity = if has_active_work(state) {
        "ACTIVE"
    } else {
        "IDLE"
    };
    let input_mode = effective_input_mode(state).label();
    frame.write_line(
        row,
        &pad(
            &format!(
                " {input_mode}  {activity}  {:?}  {:?}  TELEMETRY  EVENTS  LANES {}",
                state.runtime.snapshot.work_mode,
                state.runtime.snapshot.permission_level,
                state.runtime.lanes.len()
            ),
            frame.width,
        ),
    );
    frame.write_line(
        row + 1,
        &pad(" Ctrl-K commands  Ctrl-C cancel  Esc back", frame.width),
    );
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
