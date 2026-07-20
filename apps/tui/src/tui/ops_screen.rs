use super::{
    canvas::Frame,
    panel::panel,
    projection::{CockpitProjection, CostVisibility},
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::truncate,
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
    let mut rows = vec![
        format!(
            "ROOT     {}",
            truncate(&state.runtime.snapshot.cwd.display().to_string(), 60)
        ),
        format!("TASKS    {}", state.runtime.tasks.len()),
        format!("LANES    {}", state.runtime.lanes.len()),
        format!("ERRORS   {}", state.runtime.errors.len()),
    ];
    rows.extend(state.runtime.agent_dags.iter().flat_map(|dag| {
        let mut dag_rows = vec![format!("DAG      {} · {:?}", dag.dag_id, dag.status)];
        dag_rows.extend(dag.tasks.iter().map(|task| {
            format!(
                "BLOCKER  {} <- {}",
                task.task_id,
                if task.dependencies.is_empty() {
                    "-".to_string()
                } else {
                    task.dependencies.join(",")
                }
            )
        }));
        dag_rows
    }));
    rows.extend(state.runtime.tasks.iter().map(|task| {
        let next = task
            .next_action
            .as_ref()
            .map(|action| format!(" · {}", action.label))
            .unwrap_or_default();
        format!("TASK     {} · {}{next}", task.id, task.status)
    }));
    rows
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
        match projection.cost_visibility {
            CostVisibility::BlindUnmetered => format!(
                "COST     {} tokens · blind / unmetered",
                projection.cost.total_tokens
            ),
            CostVisibility::Metered => format!("COST     {} tokens", projection.cost.total_tokens),
            CostVisibility::Unavailable => "COST     unavailable".to_string(),
        },
    ]
}

fn provider_rows(projection: &CockpitProjection) -> Vec<String> {
    let provider = projection.provider.as_ref();
    let mut rows = vec![
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
    ];
    rows.extend(projection.merge_gates.iter().map(|gate| {
        format!(
            "GATE     {} · {:?} · {:?}",
            gate.gate_id, gate.status, gate.decision
        )
    }));
    rows.extend(
        projection
            .recovery_actions
            .iter()
            .map(|recovery| format!("RECOVER  {} · {}", recovery.reason, recovery.action)),
    );
    if let Some(pending) = projection.pending_command.as_ref() {
        rows.push(format!(
            "PENDING  {} · {:?}",
            pending.command_id, pending.state
        ));
    }
    rows.extend(
        projection
            .audit_ids
            .iter()
            .map(|audit_id| format!("AUDIT    {audit_id}")),
    );
    rows
}

fn evidence_rows(projection: &CockpitProjection) -> Vec<String> {
    let mut rows = projection
        .evidence
        .iter()
        .rev()
        .take(6)
        .map(|evidence| format!("{}  {}", evidence.kind, truncate(&evidence.summary, 60)))
        .collect::<Vec<_>>();
    rows.extend(projection.evidence_decisions.iter().map(|decision| {
        format!(
            "{}  {} · {:?}",
            decision.evidence_id, decision.gate_id, decision.decision
        )
    }));
    if rows.is_empty() {
        rows.push("no structured evidence yet".to_string());
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_types::{
        AgentDagRecord, AgentTaskRecord, MergeGateRecord, RuntimeErrorView, TokenCostView,
    };

    #[test]
    fn ops_rows_ignore_transcript_copy() {
        let mut state = TuiState::default();
        state.runtime.cost_ledger.total_tokens = 42;
        state.ui.entries.push(super::super::state::TuiEntry {
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

    #[test]
    fn ops_screen_renders_dag_gate_cost_and_recovery_from_core_facts() {
        let mut state = TuiState::default();
        state.runtime.tasks.push(
            serde_json::from_value::<AgentTaskRecord>(serde_json::json!({
                "id": "task-blocked",
                "parent_id": null,
                "role": "coder",
                "kind": "agent",
                "route": "terminal",
                "title": "resolve blocker",
                "status": "blocked",
                "activity": "dependency missing",
                "summary": "waiting retry",
                "progress": 40,
                "started_at": 1,
                "updated_at": 2,
                "workspace": ".worktrees/core",
                "evidence": ["ev-blocked"],
                "permissions": ["ask"],
                "decision": null,
                "result": null,
                "resume_handle": "resume-blocked",
                "pid": null,
                "next_action": {"label":"Retry blocker","command":"retry","reason":"dependency recovered"}
            }))
            .expect("task"),
        );
        state.runtime.agent_dags.push(
            serde_json::from_value::<AgentDagRecord>(serde_json::json!({
                "dag_id":"dag-blocked",
                "goal":"local supervision",
                "status":"blocked",
                "tasks":[{"task_id":"task-blocked","role":"coder","title":"resolve blocker","objective":"recover","dependencies":["task-dependency"],"workspace":null,"file_scope":[],"context_bundle_id":null,"required_evidence":["test_result"],"permission_policy":"ask"}],
                "created_at":1,
                "updated_at":2
            }))
            .expect("dag"),
        );
        state.runtime.merge_gates.push(
            serde_json::from_value::<MergeGateRecord>(serde_json::json!({
                "gate_id":"gate-conflict",
                "task_id":"task-blocked",
                "status":"blocked",
                "required_evidence":["test_result"],
                "evidence_ids":["ev-blocked"],
                "decision":{"outcome":"conflict","reason":"overlap","owner":{"workspace_id":"workspace","project_id":"project","lane_id":null,"session_id":null,"task_id":"task-blocked","turn_id":null},"evidence_ids":["ev-blocked"],"audit_id":"audit-conflict","decided_at":2},
                "updated_at":2
            }))
            .expect("gate"),
        );
        state.runtime.token_cost = Some(TokenCostView {
            input_tokens: 10,
            output_tokens: 2,
            total_tokens: 12,
            cost_micro_usd: None,
        });
        state.runtime.errors.push(RuntimeErrorView {
            message: "provider disconnected".to_string(),
            recoverable: true,
            hint: Some("reconnect and replay".to_string()),
        });

        let projection = CockpitProjection::from(&state.runtime, &state.ui);
        let rows = [
            runtime_rows(&state),
            context_rows(&projection),
            provider_rows(&projection),
            evidence_rows(&projection),
        ]
        .concat()
        .join("\n");

        for expected in [
            "dag-blocked",
            "task-blocked",
            "task-dependency",
            "Retry blocker",
            "gate-conflict",
            "Conflict",
            "blind / unmetered",
            "provider disconnected",
            "reconnect and replay",
            "audit-conflict",
        ] {
            assert!(rows.contains(expected), "missing {expected}:\n{rows}");
        }
    }
}
