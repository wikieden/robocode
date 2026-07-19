use super::{
    panel::panel,
    projection::CockpitProjection,
    state::{TuiState, agent_tasks},
    text::truncate,
};

pub(super) fn right_rail(state: &TuiState, width: usize, height: usize) -> Vec<String> {
    let projection = CockpitProjection::from(&state.runtime, &state.ui);
    let mut rows = vec![
        format!(
            "PROVIDER {}",
            projection
                .provider
                .as_ref()
                .map_or("-", |provider| provider.status.as_str())
        ),
        format!("COST {} tok", projection.cost.total_tokens),
        format!("APPROVAL {}", projection.approvals.len()),
    ];
    rows.extend(
        agent_tasks(state)
            .into_iter()
            .take(5)
            .map(|task| format!("{} {}", truncate(&task.id, 10), task.status)),
    );
    rows.extend(
        projection
            .errors
            .iter()
            .rev()
            .take(2)
            .map(|error| format!("ERR {}", truncate(&error.message, 18))),
    );
    panel("RUNTIME", rows, width, height, None)
}
