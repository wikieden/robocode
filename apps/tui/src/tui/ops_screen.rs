use super::{
    canvas::Frame, panel::panel, projection::CockpitProjection, state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT, text::truncate,
};

pub(super) fn render_ops_body(frame: &mut Frame, state: &TuiState) {
    let projection = CockpitProjection::from(&state.runtime, &state.ui);
    let body_top = 3;
    let body_height = frame.height.saturating_sub(body_top + BOTTOM_BAR_HEIGHT);
    let section_height = body_height.saturating_div(4).max(4);
    let sections = [
        ("TESTS / LSP", runtime_rows(state)),
        ("MCP / CONTEXT", context_rows(&projection)),
        ("PROVIDER / APPROVALS", provider_rows(&projection)),
        ("RECENT EVIDENCE", evidence_rows(&projection)),
    ];
    for (index, (title, rows)) in sections.into_iter().enumerate() {
        let block = panel(title, rows, frame.width, section_height, None);
        frame.write_block(body_top + index * section_height, 0, &block);
    }
}

fn runtime_rows(state: &TuiState) -> Vec<String> {
    vec![
        format!(
            "ROOT     {}",
            truncate(&state.runtime.snapshot.cwd.display().to_string(), 60)
        ),
        format!("TASKS    {}", state.runtime.tasks.len()),
        format!("LANES    {}", state.runtime.lanes.len()),
        format!("ERRORS   {}", state.runtime.errors.len()),
    ]
}

fn context_rows(projection: &CockpitProjection) -> Vec<String> {
    let context = projection.context.as_ref();
    vec![
        format!(
            "BUNDLE   {}",
            context.map_or("-", |value| value.bundle_id.as_str())
        ),
        format!(
            "TOKENS   {}",
            context.map_or(0, |value| value.estimated_tokens)
        ),
        format!(
            "PRESSURE {}%",
            context.map_or(0, |value| value.pressure_percent())
        ),
        format!("COST     {} tokens", projection.cost.total_tokens),
    ]
}

fn provider_rows(projection: &CockpitProjection) -> Vec<String> {
    let provider = projection.provider.as_ref();
    vec![
        format!(
            "PROVIDER {}",
            provider.map_or("-", |value| value.provider_id.as_str())
        ),
        format!(
            "MODEL    {}",
            provider.map_or("-", |value| value.model.as_str())
        ),
        format!(
            "HEALTH   {}",
            provider.map_or("unknown", |value| value.status.as_str())
        ),
        format!("APPROVAL {} pending", projection.approvals.len()),
    ]
}

fn evidence_rows(projection: &CockpitProjection) -> Vec<String> {
    let mut rows = projection
        .evidence
        .iter()
        .rev()
        .take(6)
        .map(|evidence| format!("{}  {}", evidence.kind, truncate(&evidence.summary, 60)))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push("no structured evidence yet".to_string());
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_rows_ignore_transcript_copy() {
        let mut state = TuiState::default();
        state.runtime.cost_ledger.total_tokens = 42;
        state.entries.push(super::super::state::TuiEntry {
            label: "assistant".to_string(),
            body: "cost zero".to_string(),
        });
        let projection = CockpitProjection::from(&state.runtime, &state.ui);
        assert!(
            context_rows(&projection)
                .iter()
                .any(|row| row.contains("42 tokens"))
        );
    }
}
