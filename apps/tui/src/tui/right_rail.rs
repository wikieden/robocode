use super::{
    panel::panel,
    projection::CockpitProjection,
    state::{TuiState, agent_tasks},
    text::truncate,
};

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let projection = CockpitProjection::from(&state.runtime, &state.ui);
    let mut rows = Vec::new();
    if projection.provider.is_some()
        || projection.context.is_some()
        || projection.cost.total_tokens > 0
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
        rows.push(format!("COST {} tok", projection.cost.total_tokens));
    }

    let tasks = agent_tasks(state);
    if !tasks.is_empty() {
        rows.push("LANE".to_string());
        rows.extend(
            tasks
                .into_iter()
                .take(5)
                .map(|task| format!("{} {}", truncate(&task.id, 10), task.status)),
        );
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
        rows.extend(
            projection
                .evidence
                .iter()
                .take(2)
                .map(|evidence| format!("EVIDENCE {}", truncate(&evidence.summary, 16))),
        );
    }
    if rows.is_empty() {
        rows.push("No Core detail facts.".to_string());
    }
    panel("RUNTIME", rows, width, height, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::{ProviderHealthView, RuntimeErrorView};

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
}
