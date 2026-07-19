use super::{
    canvas::Frame,
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
    frame.write_line(
        row,
        &pad(
            &format!(
                " {activity}  {:?}  {:?}  TELEMETRY  EVENTS  LANES {}",
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
