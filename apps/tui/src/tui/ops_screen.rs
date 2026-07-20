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
        (
            super::i18n::text(state, "ops.section.runtime"),
            runtime_rows(state),
        ),
        (
            super::i18n::text(state, "ops.section.context"),
            context_rows(state, &projection),
        ),
        (
            super::i18n::text(state, "ops.section.provider"),
            provider_rows(state, &projection),
        ),
        (
            super::i18n::text(state, "ops.section.evidence"),
            evidence_rows(state, &projection),
        ),
    ];
    for (index, (title, rows)) in sections.into_iter().enumerate() {
        let block = panel(&title, rows, frame.width, section_height, None);
        frame.write_block(body_top + index * section_height, 0, &block);
    }
}

fn runtime_rows(state: &TuiState) -> Vec<String> {
    let root = truncate(&state.runtime.snapshot.cwd.display().to_string(), 60);
    let task_count = state.runtime.tasks.len().to_string();
    let lane_count = state.runtime.lanes.len().to_string();
    let error_count = state.runtime.errors.len().to_string();
    let mut rows = vec![
        super::i18n::translate(state, "ops.runtime.root", &[("root", root.as_str())]),
        super::i18n::translate(
            state,
            "ops.runtime.tasks",
            &[("count", task_count.as_str())],
        ),
        super::i18n::translate(
            state,
            "ops.runtime.lanes",
            &[("count", lane_count.as_str())],
        ),
        super::i18n::translate(
            state,
            "ops.runtime.errors",
            &[("count", error_count.as_str())],
        ),
    ];
    rows.extend(state.runtime.agent_dags.iter().flat_map(|dag| {
        let status = format!("{:?}", dag.status);
        let mut dag_rows = vec![super::i18n::translate(
            state,
            "ops.runtime.dag",
            &[("dag_id", dag.dag_id.as_str()), ("status", status.as_str())],
        )];
        dag_rows.extend(dag.tasks.iter().map(|task| {
            let dependencies = if task.dependencies.is_empty() {
                "-".to_string()
            } else {
                task.dependencies.join(",")
            };
            super::i18n::translate(
                state,
                "ops.runtime.blocker",
                &[
                    ("task_id", task.task_id.as_str()),
                    ("dependencies", dependencies.as_str()),
                ],
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
        super::i18n::translate(
            state,
            "ops.runtime.task",
            &[
                ("task_id", task.id.as_str()),
                ("status", task.status.as_str()),
                ("next", next.as_str()),
            ],
        )
    }));
    rows
}

fn context_rows(state: &TuiState, projection: &CockpitProjection) -> Vec<String> {
    let context = projection.context.as_ref();
    let bundle = context.map_or("-", |value| value.bundle_id.as_str());
    let tokens = context
        .map_or(0, |value| value.estimated_tokens)
        .to_string();
    let pressure = context
        .map_or(0, |value| value.pressure_percent())
        .to_string();
    let cost = projection.cost.total_tokens.to_string();
    vec![
        super::i18n::translate(state, "ops.context.bundle", &[("bundle", bundle)]),
        super::i18n::translate(state, "ops.context.tokens", &[("tokens", tokens.as_str())]),
        super::i18n::translate(
            state,
            "ops.context.pressure",
            &[("percent", pressure.as_str())],
        ),
        match projection.cost_visibility {
            CostVisibility::BlindUnmetered => super::i18n::translate(
                state,
                "ops.context.cost.blind",
                &[("tokens", cost.as_str())],
            ),
            CostVisibility::Metered => super::i18n::translate(
                state,
                "ops.context.cost.metered",
                &[("tokens", cost.as_str())],
            ),
            CostVisibility::Unavailable => super::i18n::text(state, "ops.context.cost.unavailable"),
        },
    ]
}

fn provider_rows(state: &TuiState, projection: &CockpitProjection) -> Vec<String> {
    let provider = projection.provider.as_ref();
    let approval_count = projection.approvals.len().to_string();
    let mut rows = vec![
        super::i18n::translate(
            state,
            "ops.provider.provider",
            &[(
                "provider",
                provider.map_or("-", |value| value.provider_id.as_str()),
            )],
        ),
        super::i18n::translate(
            state,
            "ops.provider.model",
            &[("model", provider.map_or("-", |value| value.model.as_str()))],
        ),
        super::i18n::translate(
            state,
            "ops.provider.health",
            &[(
                "health",
                provider.map_or("unknown", |value| value.status.as_str()),
            )],
        ),
        super::i18n::translate(
            state,
            "ops.provider.approval",
            &[("count", approval_count.as_str())],
        ),
    ];
    rows.extend(projection.merge_gates.iter().map(|gate| {
        let status = format!("{:?}", gate.status);
        let decision = format!("{:?}", gate.decision);
        super::i18n::translate(
            state,
            "ops.provider.gate",
            &[
                ("gate_id", gate.gate_id.as_str()),
                ("status", status.as_str()),
                ("decision", decision.as_str()),
            ],
        )
    }));
    rows.extend(projection.recovery_actions.iter().map(|recovery| {
        super::i18n::translate(
            state,
            "ops.provider.recover",
            &[
                ("reason", recovery.reason.as_str()),
                ("action", recovery.action.as_str()),
            ],
        )
    }));
    if let Some(pending) = projection.pending_command.as_ref() {
        let command_state = format!("{:?}", pending.state);
        rows.push(super::i18n::translate(
            state,
            "ops.provider.pending",
            &[
                ("command_id", pending.command_id.as_str()),
                ("state", command_state.as_str()),
            ],
        ));
    }
    rows.extend(projection.audit_ids.iter().map(|audit_id| {
        super::i18n::translate(
            state,
            "ops.provider.audit",
            &[("audit_id", audit_id.as_str())],
        )
    }));
    rows
}

fn evidence_rows(state: &TuiState, projection: &CockpitProjection) -> Vec<String> {
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
        rows.push(super::i18n::text(state, "ops.evidence.empty"));
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
            context_rows(&state, &projection)
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
            context_rows(&state, &projection),
            provider_rows(&state, &projection),
            evidence_rows(&state, &projection),
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

    #[test]
    fn ops_screen_follows_core_locale_without_translating_fact_values() {
        let mut state = TuiState::default();
        state.runtime.snapshot.ui_preferences.locale = viden_core::LocaleId::ZhCn;
        state.runtime.snapshot.cwd = "/workspace/raw-project".into();
        state.runtime.errors.push(RuntimeErrorView {
            message: "error-raw".to_string(),
            recoverable: true,
            hint: None,
        });
        let mut frame = Frame::new(120, 40);

        render_ops_body(&mut frame, &state);
        let rendered = frame.to_string();

        for expected in [
            "TESTS / LSP · 测试",
            "MCP / CONTEXT · 上下文",
            "PROVIDER / APPROVALS · 审批",
            "RECENT EVIDENCE · 最近证据",
            "/workspace/raw-project",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}:\n{rendered}"
            );
        }
    }
}
