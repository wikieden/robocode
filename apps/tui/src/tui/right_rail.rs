use super::preferences::UI_PREFERENCE_PERSISTENCE_CAPABILITY;
use super::{
    glyphs::Glyph,
    panel::panel,
    projection::{CockpitProjection, ContextPressure, CostVisibility},
    state::{TuiState, agent_tasks},
    text::truncate,
};

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let projection =
        CockpitProjection::from_with_capabilities(&state.runtime, &state.ui, &state.capabilities);
    let mut rows = vec!["CHANGES · EVIDENCE · CONTEXT".to_string()];
    if projection.provider.is_some()
        || projection.context.is_some()
        || projection.cost.total_tokens > 0
        || projection.token_cost.is_some()
    {
        rows.push("ENV".to_string());
        rows.push(format!(
            "PROVIDER {}",
            projection
                .provider
                .as_ref()
                .map_or("-", |provider| provider.status.as_str())
        ));
        if let Some(context) = projection.context.as_ref() {
            rows.push(format!(
                "CONTEXT {}/{}",
                context.estimated_tokens, context.hard_token_limit
            ));
        }
        rows.push(format!("CONTEXT {:?}", projection.context_pressure));
        rows.push(match projection.cost_visibility {
            CostVisibility::BlindUnmetered => "COST blind / unmetered".to_string(),
            CostVisibility::Metered => format!("COST {} tok", projection.cost.total_tokens),
            CostVisibility::Unavailable => "COST unavailable".to_string(),
        });
    }

    let tasks = agent_tasks(state);
    if !tasks.is_empty() {
        rows.push("LANE".to_string());
        rows.extend(tasks.into_iter().take(4).map(|task| {
            let next = task
                .next_action
                .as_ref()
                .map(|action| format!(" · {}", action.label))
                .unwrap_or_default();
            format!("{} {}{next}", truncate(&task.id, 16), task.status)
        }));
    }

    rows.push(format!(
        "FLEET active {}/{}",
        projection
            .lanes
            .iter()
            .filter(|lane| lane.is_active())
            .count(),
        projection.lanes.len()
    ));
    let counts = projection.supervision_counts();
    // Registered gold `⏸` gate badge; the terminal layer downgrades it to the
    // registered ASCII fallback when the terminal cannot render Unicode.
    rows.push(format!(
        "INBOX A:{} {} {} R:{}",
        projection.approvals.len(),
        Glyph::Gate.unicode(),
        counts.merge_gates,
        projection.recovery_actions.len()
    ));
    if counts.has_non_gate_records() {
        rows.push(super::i18n::translate(
            state,
            "rail.supervision.trust",
            &[
                ("review", &counts.review_requests.to_string()),
                ("handoff", &counts.handoffs.to_string()),
                ("contract", &counts.contracts.to_string()),
            ],
        ));
        rows.push(super::i18n::translate(
            state,
            "rail.supervision.integration",
            &[
                ("dependency", &counts.dependencies.to_string()),
                ("bounce", &counts.conflict_bounces.to_string()),
                ("revert", &counts.reverts.to_string()),
            ],
        ));
    }

    if !projection.approvals.is_empty()
        || !projection.errors.is_empty()
        || !projection.evidence.is_empty()
    {
        rows.push("MORE".to_string());
        if !projection.approvals.is_empty() {
            rows.push(format!("APPROVAL {}", projection.approvals.len()));
        }
        rows.extend(
            projection
                .errors
                .iter()
                .rev()
                .take(2)
                .map(|error| format!("ERR {}", truncate(&error.message, 18))),
        );
        rows.extend(projection.evidence.iter().take(2).map(|evidence| {
            format!(
                "EVIDENCE {} {}",
                truncate(&evidence.id, 14),
                truncate(&evidence.summary, 14)
            )
        }));
    }
    if rows.len() == 3
        && projection.lanes.is_empty()
        && projection.provider.is_none()
        && projection.context_pressure == ContextPressure::Unavailable
    {
        rows.push(super::i18n::text(state, "rail.empty"));
    }
    let preferences = &state.runtime.snapshot.ui_preferences;
    rows.push(
        if state.has_capability(UI_PREFERENCE_PERSISTENCE_CAPABILITY) {
            super::i18n::text(state, "settings.rail.available")
        } else {
            super::i18n::text(state, "settings.rail.unavailable")
        },
    );
    rows.push(super::i18n::translate(
        state,
        "settings.rail.profile",
        &[
            ("skin", skin_name(preferences.skin)),
            ("mode", mode_name(preferences.mode)),
            ("locale", locale_name(preferences.locale)),
            ("density", density_name(preferences.density)),
            ("motion", motion_name(preferences.motion)),
        ],
    ));
    rows.push(super::i18n::text(state, "settings.rail.authority"));
    let mut diagnostic_codes = preferences
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .chain(state.ui.preference_diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    diagnostic_codes.sort();
    diagnostic_codes.dedup();
    rows.extend(diagnostic_codes.iter().map(|diagnostic| {
        super::i18n::translate(
            state,
            "settings.rail.diagnostic",
            &[("diagnostic", diagnostic.as_str())],
        )
    }));
    if let Some(super::state::InteractionPanel::Settings(panel)) =
        state.ui.interaction_panel.as_ref()
    {
        if panel.is_pending() {
            rows.push(super::i18n::text(state, "settings.pending"));
        } else if let Some(reason) = panel.rejection_reason() {
            rows.push(super::i18n::translate(
                state,
                "settings.rejected",
                &[("reason", reason)],
            ));
        } else if panel.has_succeeded() {
            rows.push(super::i18n::text(state, "settings.saved"));
        }
    }
    let title = super::i18n::text(state, "rail.title");
    panel(&title, rows, width, height, None)
}

fn locale_name(value: viden_core::LocaleId) -> &'static str {
    match value {
        viden_core::LocaleId::System => "system",
        viden_core::LocaleId::En => "en",
        viden_core::LocaleId::ZhCn => "zh-CN",
    }
}

fn skin_name(value: viden_core::UiSkin) -> &'static str {
    match value {
        viden_core::UiSkin::Aurora => "aurora",
        viden_core::UiSkin::Ice => "ice",
        viden_core::UiSkin::Mono => "mono",
        viden_core::UiSkin::Amber => "amber",
        viden_core::UiSkin::Phosphor => "phosphor",
    }
}

fn mode_name(value: viden_core::UiColorMode) -> &'static str {
    match value {
        viden_core::UiColorMode::System => "system",
        viden_core::UiColorMode::Dark => "dark",
        viden_core::UiColorMode::Light => "light",
    }
}

fn density_name(value: viden_core::UiDensity) -> &'static str {
    match value {
        viden_core::UiDensity::Compact => "compact",
        viden_core::UiDensity::Regular => "regular",
        viden_core::UiDensity::Comfy => "comfy",
    }
}

fn motion_name(value: viden_core::UiMotion) -> &'static str {
    match value {
        viden_core::UiMotion::System => "system",
        viden_core::UiMotion::Reduced => "reduced",
        viden_core::UiMotion::Full => "full",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::{
        AgentTaskKind, AgentTaskRecord, AgentTaskStatus, ProviderHealthView, RuntimeErrorView,
    };

    #[test]
    fn env_lane_more_groups_only_exist_for_core_facts() {
        let empty = right_rail(&TuiState::default(), 38, 20).join("\n");
        assert!(!empty.contains("ENV"));
        assert!(!empty.contains("LANE"));
        assert!(!empty.contains("MORE"));

        let mut state = TuiState::default();
        state.runtime.provider = Some(ProviderHealthView {
            provider_id: "fallback".to_string(),
            model: "test-local".to_string(),
            status: "healthy".to_string(),
            request_count: 1,
            error_count: 0,
            last_latency_ms: None,
            average_latency_ms: None,
            tokens_per_second: None,
            credential: None,
        });
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lanes");
        state.runtime.lanes.truncate(1);
        state.runtime.errors.push(RuntimeErrorView {
            message: "core error".to_string(),
            recoverable: true,
            hint: None,
        });

        let rendered = right_rail(&state, 38, 20).join("\n");
        assert!(rendered.contains("ENV"));
        assert!(rendered.contains("PROVIDER healthy"));
        assert!(rendered.contains("LANE"));
        assert!(rendered.contains(&state.runtime.lanes[0].id));
        assert!(rendered.contains("MORE"));
        assert!(rendered.contains("ERR core error"));
    }

    #[test]
    fn empty_runtime_rail_uses_core_resolved_chinese_locale() {
        let mut state = TuiState::default();
        state.runtime.snapshot.ui_preferences.locale = viden_core::LocaleId::ZhCn;

        let rendered = right_rail(&state, 38, 20).join("\n");

        assert!(rendered.contains("RUNTIME · 运行时"));
        assert!(rendered.contains("Core 暂无详细事实。"));
    }

    #[test]
    fn local_supervision_rail_exposes_detail_lenses_and_compact_summaries() {
        let mut state = TuiState::default();
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lanes");
        state.runtime.tasks.push(AgentTaskRecord {
            id: "task-blocked".to_string(),
            parent_id: None,
            role: viden_types::AgentRole::Coder,
            kind: AgentTaskKind::Agent,
            route: viden_types::AgentRoute::Terminal,
            title: "blocked task".to_string(),
            status: AgentTaskStatus::Blocked,
            activity: "dependency missing".to_string(),
            summary: "waiting retry".to_string(),
            progress: 40,
            started_at: None,
            updated_at: None,
            workspace: None,
            evidence: vec!["ev-review".to_string()],
            permissions: vec!["ask".to_string()],
            decision: None,
            result: None,
            resume_handle: None,
            pid: None,
            next_action: Some(viden_types::AgentNextAction {
                label: "Retry blocker".to_string(),
                command: Some("retry".to_string()),
                reason: Some("dependency recovered".to_string()),
            }),
        });
        state.runtime.token_cost = Some(viden_types::TokenCostView {
            input_tokens: 100,
            output_tokens: 20,
            total_tokens: 120,
            cost_micro_usd: None,
        });
        state
            .runtime
            .latest_evidence
            .push(viden_types::EvidenceView {
                id: "ev-review".to_string(),
                kind: "review".to_string(),
                summary: "review needs changes".to_string(),
                path: None,
                source: Some("core".to_string()),
                canonical: None,
                metadata: None,
                timestamp: Some(1),
            });

        let rendered = right_rail(&state, 42, 28).join("\n");

        for expected in [
            "CHANGES",
            "EVIDENCE",
            "CONTEXT",
            "INBOX",
            "FLEET",
            "blind / unmetered",
            "task-blocked",
            "Retry blocker",
            "ev-review",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}:\n{rendered}"
            );
        }
    }

    #[test]
    fn rail_inbox_uses_the_registered_gate_glyph_and_hides_empty_supervision_rows() {
        let mut state = TuiState::default();
        let empty = right_rail(&state, 48, 28).join("\n");
        assert!(empty.contains("INBOX A:0 ⏸ 0 R:0"));
        assert!(!empty.contains("G:0"));
        assert!(!empty.contains("TRUST"));

        state
            .runtime
            .review_requests
            .push(viden_types::ReviewRequestRecord {
                review_id: "review-1".to_string(),
                gate_id: "gate-1".to_string(),
                task_id: "task-1".to_string(),
                requester_lane_id: "lane-a".to_string(),
                reviewer_lane_id: "lane-b".to_string(),
                owner: Default::default(),
                evidence_ids: Vec::new(),
                evidence_bindings: Vec::new(),
                status: viden_types::ReviewRequestStatus::Pending,
                feedback: None,
                audit_id: "audit-review".to_string(),
                updated_at: 1,
            });
        state
            .runtime
            .dependencies
            .push(viden_types::DependencyRecord {
                dependency_id: "dep-1".to_string(),
                task_id: "task-1".to_string(),
                depends_on_task_id: "task-0".to_string(),
                owner: Default::default(),
                state: viden_types::DependencyState::Blocked,
                reason: "upstream pending".to_string(),
                audit_id: "audit-dep".to_string(),
                updated_at: 2,
            });

        let rendered = right_rail(&state, 48, 28).join("\n");
        assert!(rendered.contains("TRUST review 1"), "missing:\n{rendered}");
        assert!(
            rendered.contains("INTEGRATION dependency 1"),
            "missing:\n{rendered}"
        );

        state.runtime.snapshot.ui_preferences.locale = viden_core::LocaleId::ZhCn;
        let chinese = right_rail(&state, 48, 28).join("\n");
        assert!(chinese.contains("信任 复审 1"), "missing:\n{chinese}");
        assert!(chinese.contains("集成 依赖 1"), "missing:\n{chinese}");
    }

    #[test]
    fn right_rail_exposes_preference_authority_and_feature_gate_status() {
        let mut state = TuiState::default();
        let unavailable = right_rail(&state, 52, 28).join("\n");
        assert!(unavailable.contains("SETTINGS unavailable"));

        state.capabilities.insert(viden_types::CapabilityId(
            "ui.preference_persistence".to_string(),
        ));
        state.runtime.snapshot.ui_preferences.skin = viden_core::UiSkin::Ice;
        state.runtime.snapshot.ui_preferences.mode = viden_core::UiColorMode::Light;
        let available = right_rail(&state, 52, 28).join("\n");
        assert!(available.contains("SETTINGS available"));
        assert!(available.contains("ice/light"));
        assert!(available.contains("Core resolved"));
    }
}
