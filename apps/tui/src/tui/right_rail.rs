use super::{
    panel::panel,
    projection::{CockpitProjection, ContextPressure, CostVisibility},
    state::{TuiState, agent_tasks},
    text::truncate,
};

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let projection = CockpitProjection::from(&state.runtime, &state.ui);
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
    rows.push(format!(
        "INBOX A:{} G:{} R:{}",
        projection.approvals.len(),
        projection.merge_gates.len(),
        projection.recovery_actions.len()
    ));

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
    let title = super::i18n::text(state, "rail.title");
    panel(&title, rows, width, height, None)
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
}
