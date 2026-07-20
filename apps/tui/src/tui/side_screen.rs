use super::{
    canvas::Frame,
    panel::panel,
    projection::CockpitProjection,
    state::{TuiState, agent_lanes, provider_status},
    statusbar::BOTTOM_BAR_HEIGHT,
    text::truncate,
};

pub(super) fn render_side_body(frame: &mut Frame, state: &TuiState) {
    let body_top = 3;
    let body_height = frame.height.saturating_sub(body_top + BOTTOM_BAR_HEIGHT);
    let lane_height = body_height.saturating_mul(2).saturating_div(3).max(8);
    frame.write_block(
        body_top,
        0,
        &panel(
            "AGENT LANES",
            lane_rows(state),
            frame.width,
            lane_height,
            None,
        ),
    );
    frame.write_block(
        body_top + lane_height,
        0,
        &panel(
            "SIDE STATUS",
            side_status_rows(state),
            frame.width,
            body_height.saturating_sub(lane_height).max(5),
            None,
        ),
    );
}

fn lane_rows(state: &TuiState) -> Vec<String> {
    let projection = CockpitProjection::from(&state.runtime, &state.ui);
    let mut rows = agent_lanes(state)
        .into_iter()
        .flat_map(|lane| {
            let mut lane_rows = vec![format!(
                "SWITCH {:<12} {:<10} {}",
                truncate(&lane.id, 12),
                lane.status,
                truncate(&lane.summary, 36)
            )];
            if projection
                .owner_actions
                .iter()
                .any(|action| action.target_lane_id == lane.id)
            {
                lane_rows.push(format!(
                    "CANCEL UNAVAILABLE {} · Core owner required",
                    lane.id
                ));
            }
            lane_rows
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push("no Core lanes".to_string());
    }
    rows
}

pub(super) fn side_status_rows(state: &TuiState) -> Vec<String> {
    let lanes = agent_lanes(state);
    let status = provider_status(state);
    let mut rows = vec![
        format!(
            "PROVIDER  {} / {}",
            state.runtime.snapshot.provider_family, state.runtime.snapshot.model_label
        ),
        format!(
            "WORKSPACE {}",
            truncate(&state.runtime.snapshot.cwd.display().to_string(), 28)
        ),
        format!(
            "LANES     active {}/{}",
            lanes.iter().filter(|lane| lane.is_active()).count(),
            lanes.len()
        ),
        format!("TELEMETRY {}", status.telemetry),
        format!("CONTEXT   {}", status.context_window),
        format!("THEME     {}", state.ui.theme_name),
    ];
    rows.extend(state.runtime.lane_recoveries.iter().map(|recovery| {
        format!(
            "RECOVERY  {} · {} · {}",
            recovery.lane_id, recovery.reason, recovery.next_action
        )
    }));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::LaneRecoveryView;

    #[test]
    fn side_rows_use_runtime_snapshot() {
        let mut state = TuiState::default();
        state.runtime.snapshot.provider_family = "structured-provider".to_string();
        state.runtime.snapshot.model_label = "structured-model".to_string();
        assert!(side_status_rows(&state)[0].contains("structured-provider / structured-model"));
    }

    #[test]
    fn typed_core_lane_states_drive_side_counts_and_state_rows() {
        let mut state = TuiState::default();
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lane fixture");

        let rows = lane_rows(&state);
        let status = side_status_rows(&state);

        assert!(rows.iter().any(|row| row.contains("L-start")));
        assert!(rows.iter().any(|row| row.contains("starting")));
        assert!(rows.iter().any(|row| row.contains("detached")));
        assert!(status.iter().any(|row| row.contains("active 4/4")));
    }

    #[test]
    fn side_screen_exposes_lane_switch_owner_cancel_and_recovery_without_inference() {
        let mut state = TuiState::default();
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lane fixture");
        state.runtime.lane_recoveries.push(LaneRecoveryView {
            lane_id: "L-conflict".to_string(),
            reason: "worker disconnected".to_string(),
            next_action: "reconnect and replay".to_string(),
            timestamp: Some(1),
        });

        let rows = lane_rows(&state)
            .into_iter()
            .chain(side_status_rows(&state))
            .collect::<Vec<_>>()
            .join("\n");

        for expected in [
            "SWITCH L-conflict",
            "CANCEL UNAVAILABLE L-conflict · Core owner required",
            "worker disconnected",
            "reconnect and replay",
        ] {
            assert!(rows.contains(expected), "missing {expected}:\n{rows}");
        }
    }
}
