use super::{
    canvas::Frame,
    state::{Lens, TuiState, provider_status},
    text::{pad, truncate},
};

pub(super) fn render_top_bar(frame: &mut Frame, state: &TuiState) {
    let status = provider_status(state);
    let surface = match state.ui.lens {
        Lens::Welcome => "",
        Lens::Setup => " / SETUP",
        Lens::Board => " / LANES",
        Lens::Session => " / COCKPIT",
        Lens::Decisions => " / DECISIONS",
        Lens::Gallery => " / GALLERY",
    };
    frame.write_line(
        0,
        &pad(
            &format!(
                " VIDEN{surface}  {}  {}  {}",
                truncate(&state.runtime.snapshot.provider_family, 14),
                truncate(&state.runtime.snapshot.model_label, 18),
                status.connection
            ),
            frame.width,
        ),
    );
    frame.write_line(
        1,
        &pad(
            &format!(
                " {}  lane {}  session {}  approvals {}",
                truncate(&state.runtime.snapshot.cwd.display().to_string(), 32),
                state.ui.focused_lane.as_deref().unwrap_or("-"),
                if state.ui.session_id.is_empty() {
                    "-"
                } else {
                    state.ui.session_id.as_str()
                },
                state.runtime.pending_approvals.len()
            ),
            frame.width,
        ),
    );
}

pub(super) fn render_side_top_bar(frame: &mut Frame, state: &TuiState) {
    frame.write_line(0, &pad(" VIDEN / AGENT LANES", frame.width));
    frame.write_line(
        1,
        &pad(
            &format!(
                " {}  {} lanes",
                truncate(&state.runtime.snapshot.cwd.display().to_string(), 42),
                state.runtime.lanes.len()
            ),
            frame.width,
        ),
    );
}

pub(super) fn render_ops_top_bar(frame: &mut Frame, state: &TuiState) {
    frame.write_line(0, &pad(" VIDEN / OPS", frame.width));
    frame.write_line(
        1,
        &pad(
            &format!(
                " context {}  evidence {}  errors {}",
                state
                    .runtime
                    .context
                    .as_ref()
                    .map_or("-", |context| context.bundle_id.as_str()),
                state.runtime.latest_evidence.len(),
                state.runtime.errors.len()
            ),
            frame.width,
        ),
    );
}
