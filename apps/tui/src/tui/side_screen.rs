use super::{
    canvas::Frame,
    panel::panel,
    projection::{CancelOwnerProjection, CancelUnavailableReason, CockpitProjection},
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
            &super::i18n::text(state, "side.title.lanes"),
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
            &super::i18n::text(state, "side.title.status"),
            side_status_rows(state),
            frame.width,
            body_height.saturating_sub(lane_height).max(5),
            None,
        ),
    );
}

fn lane_rows(state: &TuiState) -> Vec<String> {
    let projection =
        CockpitProjection::from_with_capabilities(&state.runtime, &state.ui, &state.capabilities);
    let mut rows = agent_lanes(state)
        .into_iter()
        .flat_map(|lane| {
            let lane_id = truncate(&lane.id, 12);
            let status = lane.status.to_string();
            let summary = truncate(&lane.summary, 36);
            let mut lane_rows = vec![super::i18n::translate(
                state,
                "side.lane.switch",
                &[
                    ("lane_id", lane_id.as_str()),
                    ("status", status.as_str()),
                    ("summary", summary.as_str()),
                ],
            )];
            let cancel_key = match projection.cancel_owner_for_lane(&lane.id) {
                CancelOwnerProjection::Available(_) => "side.lane.cancel_available",
                CancelOwnerProjection::Unavailable(CancelUnavailableReason::MissingCapability) => {
                    "side.lane.cancel_unavailable_capability"
                }
                CancelOwnerProjection::Unavailable(CancelUnavailableReason::AmbiguousOwner) => {
                    "side.lane.cancel_unavailable_ambiguous"
                }
                CancelOwnerProjection::Unavailable(CancelUnavailableReason::OwnerLaneMismatch) => {
                    "side.lane.cancel_unavailable_mismatch"
                }
                CancelOwnerProjection::Unavailable(
                    CancelUnavailableReason::CoreOwnerRequired
                    | CancelUnavailableReason::LaneNotActive,
                ) => "side.lane.cancel_unavailable",
            };
            lane_rows.push(super::i18n::translate(
                state,
                cancel_key,
                &[("lane_id", lane.id.as_str())],
            ));
            lane_rows
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(super::i18n::text(state, "side.empty"));
    }
    rows
}

pub(super) fn side_status_rows(state: &TuiState) -> Vec<String> {
    let lanes = agent_lanes(state);
    let status = provider_status(state);
    let workspace = truncate(&state.runtime.snapshot.cwd.display().to_string(), 28);
    let active = lanes
        .iter()
        .filter(|lane| lane.is_active())
        .count()
        .to_string();
    let total = lanes.len().to_string();
    let mut rows = vec![
        super::i18n::translate(
            state,
            "side.status.provider",
            &[
                ("provider", state.runtime.snapshot.provider_family.as_str()),
                ("model", state.runtime.snapshot.model_label.as_str()),
            ],
        ),
        super::i18n::translate(
            state,
            "side.status.workspace",
            &[("workspace", workspace.as_str())],
        ),
        super::i18n::translate(
            state,
            "side.status.lanes",
            &[("active", active.as_str()), ("total", total.as_str())],
        ),
        super::i18n::translate(
            state,
            "side.status.telemetry",
            &[("telemetry", status.telemetry.as_str())],
        ),
        super::i18n::translate(
            state,
            "side.status.context",
            &[("context", status.context_window.as_str())],
        ),
        super::i18n::translate(
            state,
            "side.status.theme",
            &[("theme", state.ui.theme_name.as_str())],
        ),
    ];
    rows.extend(state.runtime.lane_recoveries.iter().map(|recovery| {
        super::i18n::translate(
            state,
            "side.status.recovery",
            &[
                ("lane_id", recovery.lane_id.as_str()),
                ("reason", recovery.reason.as_str()),
                ("next_action", recovery.next_action.as_str()),
            ],
        )
    }));
    rows.extend(state.runtime.agent_sessions.iter().map(|session| {
        let task = truncate(&session.task, 24);
        let status = format!("{:?}", session.status).to_ascii_lowercase();
        super::i18n::translate(
            state,
            "side.status.agent_session",
            &[
                ("agent", session.agent_id.as_str()),
                ("status", status.as_str()),
                ("task", task.as_str()),
            ],
        )
    }));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::{CapabilityId, LaneRecoveryView, LaneRuntimeOwnerBinding, RuntimeOwner};

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
        state
            .capabilities
            .insert(CapabilityId("runtime.lane_owner_projection".to_string()));
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

    #[test]
    fn lane_runtime_owner_copy_is_bilingual_and_preserves_ids_and_shortcuts() {
        let mut state = TuiState::default();
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lane fixture");
        state
            .capabilities
            .insert(CapabilityId("runtime.lane_owner_projection".to_string()));
        state.runtime.lane_runtime_owners = vec![LaneRuntimeOwnerBinding {
            lane_id: "L-start".to_string(),
            owner: RuntimeOwner {
                lane_id: Some("L-start".to_string()),
                ..RuntimeOwner::default()
            },
        }];

        let english = lane_rows(&state).join("\n");
        assert!(english.contains("CANCEL L-start · Ctrl-C"));
        assert!(english.contains("CANCEL UNAVAILABLE L-conflict · Core owner required"));

        state.runtime.snapshot.ui_preferences.locale = viden_core::LocaleId::ZhCn;
        let chinese = lane_rows(&state).join("\n");
        assert!(chinese.contains("取消 L-start · Ctrl-C"));
        assert!(chinese.contains("无法取消 L-conflict · 需要 Core owner"));
    }

    #[test]
    fn side_screen_follows_core_locale_without_translating_runtime_facts() {
        let mut state = TuiState::default();
        state.runtime.snapshot.ui_preferences.locale = viden_core::LocaleId::ZhCn;
        state.runtime.snapshot.provider_family = "provider-raw".to_string();
        state.runtime.snapshot.model_label = "model-raw".to_string();
        state.runtime.lane_recoveries.push(LaneRecoveryView {
            lane_id: "lane-raw".to_string(),
            reason: "reason-raw".to_string(),
            next_action: "action-raw".to_string(),
            timestamp: None,
        });
        let mut frame = Frame::new(100, 30);

        render_side_body(&mut frame, &state);
        let rendered = frame.to_string();

        for expected in [
            "AGENT 通道",
            "侧栏状态",
            "provider-raw",
            "model-raw",
            "lane-raw",
            "reason-raw",
            "action-raw",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}:\n{rendered}"
            );
        }
    }
}
