use std::{
    env, fs,
    path::{Path, PathBuf},
};

use super::{
    canvas::Frame,
    indicators::progress_bar,
    panel::panel,
    state::{AgentTask, TuiState, agent_lanes, agent_tasks},
    statusbar::BOTTOM_BAR_HEIGHT,
    text::truncate,
};

pub(super) fn render_ops_body(frame: &mut Frame, state: &TuiState) {
    let body_top = 3;
    let body_bottom = frame.height - BOTTOM_BAR_HEIGHT - 1;
    let body_height = body_bottom.saturating_sub(body_top) + 1;
    let tests_height = body_height.saturating_mul(6).saturating_div(20).max(7);
    let context_height = body_height.saturating_mul(5).saturating_div(20).max(7);
    let extensions_height = body_height.saturating_mul(4).saturating_div(20).max(6);
    let evidence_height = body_height
        .saturating_sub(tests_height + context_height + extensions_height)
        .max(5);

    let tests = panel(
        "TESTS / LSP",
        ops_tests_lsp_rows(state),
        frame.width,
        tests_height,
        Some(&format!("{} diag", state.workspace.diagnostics.len())),
    );
    frame.write_block(body_top, 0, &tests);

    let context_top = body_top + tests_height;
    let context = panel(
        "MCP / CONTEXT",
        ops_context_rows(state),
        frame.width,
        context_height,
        Some("workspace"),
    );
    frame.write_block(context_top, 0, &context);

    let extensions_top = context_top + context_height;
    let extensions = panel(
        "EXTENSIONS",
        ops_extension_rows(state),
        frame.width,
        extensions_height,
        Some(extension_health_label(state)),
    );
    frame.write_block(extensions_top, 0, &extensions);

    let evidence_top = extensions_top + extensions_height;
    let evidence = panel(
        "RECENT EVIDENCE",
        ops_evidence_rows(state),
        frame.width,
        evidence_height,
        Some("tail"),
    );
    frame.write_block(evidence_top, 0, &evidence);
}

fn ops_tests_lsp_rows(state: &TuiState) -> Vec<String> {
    let mut rows = vec![
        latest_test_summary(state).unwrap_or_else(|| "TEST    no /test evidence yet".to_string()),
        format!(
            "LSP     {} diagnostic(s)",
            state.workspace.diagnostics.len()
        ),
    ];
    if state.workspace.diagnostics.is_empty() {
        rows.push("        waiting for auto-checks or /lsp diagnostics".to_string());
    } else {
        rows.extend(
            state
                .workspace
                .diagnostics
                .iter()
                .take(5)
                .map(|diagnostic| format!("        {}", truncate(diagnostic, 64))),
        );
    }
    rows
}

fn latest_test_summary(state: &TuiState) -> Option<String> {
    let body = state.entries.iter().rev().find_map(|entry| {
        entry
            .body
            .contains("Test result:")
            .then_some(entry.body.as_str())
    })?;
    let status = rendered_field(body, "status").unwrap_or("unknown");
    let command = rendered_field(body, "command").unwrap_or("<unknown command>");
    let duration = rendered_field(body, "duration").unwrap_or("-");
    Some(truncate(
        &format!("TEST    {status}  {duration}  {command}"),
        72,
    ))
}

fn rendered_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("  {key}: ");
    body.lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
}

fn ops_context_rows(state: &TuiState) -> Vec<String> {
    let mcp_configs = mcp_config_statuses(&state.workspace.root);
    let configured_mcp_count = mcp_configs
        .iter()
        .filter(|config| matches!(config.status, ConfigStatus::Found))
        .count();
    let mut rows = vec![
        format!("ROOT    {}", truncate(&state.workspace.display_root, 60)),
        format!("BRANCH  {}", state.workspace.git_branch),
        format!(
            "SCALE   files {}   lines {}",
            state.workspace.file_count, state.workspace.line_count
        ),
        format!(
            "CTX     {}   MCP configs {}",
            state.provider_status.context_window, configured_mcp_count
        ),
    ];
    if let Some(pressure) = agent_tasks(state)
        .into_iter()
        .flat_map(|task| task.evidence)
        .find_map(|item| item.strip_prefix("context_pressure ").map(str::to_string))
    {
        rows.push(format!("BUNDLE  pressure {pressure}"));
    }
    let context_evidence = agent_tasks(state)
        .into_iter()
        .flat_map(|task| task.evidence)
        .collect::<Vec<_>>();
    if let Some(policy) = context_evidence
        .iter()
        .find_map(|item| item.strip_prefix("context_policy ").map(str::to_string))
    {
        rows.push(format!("POLICY  {}", truncate(&policy, 60)));
    }
    if let Some(omitted) = context_evidence
        .iter()
        .find_map(|item| item.strip_prefix("context_omitted ").map(str::to_string))
    {
        rows.push(format!("OMIT    {omitted} source(s)"));
    }
    rows.extend(mcp_configs.iter().take(3).map(|config| {
        format!(
            "MCP     {:<10} {:<7} {}",
            config.source,
            config.status.as_str(),
            truncate(&display_path(&config.path), 42)
        )
    }));
    rows
}

fn ops_extension_rows(state: &TuiState) -> Vec<String> {
    let mcp_ready = mcp_config_statuses(&state.workspace.root)
        .iter()
        .filter(|config| matches!(config.status, ConfigStatus::Found))
        .count();
    let skill_count = count_skills(&state.workspace.root);
    let lanes = agent_lanes(state);
    let active_lanes = lanes.iter().filter(|lane| lane.is_active()).count();
    let primary_lane = lanes
        .iter()
        .find(|lane| lane.is_active())
        .map(|lane| {
            let evidence = lane
                .evidence
                .first()
                .map(String::as_str)
                .unwrap_or("no evidence");
            format!(
                "{} {} {} {} {}",
                lane.screen,
                lane.agent,
                lane.task_id,
                truncate(&lane.summary, 18),
                truncate(evidence, 28)
            )
        })
        .unwrap_or_else(|| "main idle no evidence".to_string());
    let mut rows = vec![
        format!(
            "PROVIDER {:<10} {}",
            state.provider_status.connection, state.provider_status.telemetry
        ),
        format!("CATALOG  {} provider(s)", state.provider_catalog.len()),
        format!("AGENTS   {} active / {} lane(s)", active_lanes, lanes.len()),
        format!("LANE     {}", truncate(&primary_lane, 62)),
        format!("MCP      {} configured config(s)", mcp_ready),
        format!("SKILLS   {} discovered recipe(s)", skill_count),
    ];
    if let Some(error) = &state.provider_status.last_error {
        rows.push(format!("ERROR    {}", truncate(error, 60)));
    }
    rows
}

fn extension_health_label(state: &TuiState) -> &'static str {
    if state.provider_status.last_error.is_some() {
        "attention"
    } else {
        "ready"
    }
}

fn ops_evidence_rows(state: &TuiState) -> Vec<String> {
    let mut rows = Vec::new();
    let mut tasks = agent_tasks(state);
    tasks.sort_by_key(|task| std::cmp::Reverse(ops_evidence_task_priority(task)));
    for task in tasks.into_iter().take(6) {
        rows.push(agent_task_evidence_row(&task));
        let detail_limit =
            if task.kind == "diff" || matches!(task.status.as_str(), "failed" | "blocked") {
                6
            } else {
                2
            };
        rows.extend(agent_task_detail_rows(&task).into_iter().take(detail_limit));
    }
    if rows.is_empty() {
        rows.push("no tool/test/lane evidence yet".to_string());
    }
    rows
}

fn ops_evidence_task_priority(task: &AgentTask) -> u8 {
    if task
        .evidence
        .iter()
        .any(|item| item.starts_with("message "))
    {
        return 97;
    }
    match task.status.as_str() {
        "waiting_approval" => 100,
        "failed" | "blocked" => 95,
        "testing" => 85,
        "editing" | "running_tool" => 80,
        "needs_input" => 75,
        "thinking" | "streaming" => 60,
        "done" => 40,
        _ => 10,
    }
}

fn agent_task_evidence_row(task: &AgentTask) -> String {
    let progress = progress_bar(task.progress);
    let bar = progress.split_whitespace().next().unwrap_or("░░░░░");
    format!(
        "{} {} {} {} {}",
        truncate(&task.id, 12),
        truncate(&task.agent, 10),
        task.status,
        bar,
        truncate(&task.activity, 52)
    )
}

fn agent_task_detail_rows(task: &AgentTask) -> Vec<String> {
    let mut rows = Vec::new();
    let evidence = prioritized_task_evidence(task);
    if matches!(task.status.as_str(), "failed" | "blocked") {
        rows.extend(evidence.iter().take(4).map(|item| evidence_row(item)));
    }
    if let Some(decision) = &task.decision {
        rows.push(format!("  decision {}", truncate(decision, 64)));
    }
    if let Some(next) = agent_task_next_action(task) {
        rows.push(format!("  next {}", truncate(next, 70)));
    }
    if let Some(result) = &task.result {
        rows.push(format!("  result {}", truncate(result, 66)));
    }
    if matches!(task.status.as_str(), "failed" | "blocked") {
        rows.extend(evidence.iter().skip(4).map(|item| evidence_row(item)));
    }
    if !matches!(task.status.as_str(), "failed" | "blocked") {
        rows.extend(evidence.iter().map(|item| evidence_row(item)));
    }
    rows
}

fn prioritized_task_evidence(task: &AgentTask) -> Vec<String> {
    let mut evidence = task
        .evidence
        .iter()
        .filter(|item| !item.starts_with("transcript "))
        .cloned()
        .collect::<Vec<_>>();
    evidence.sort_by_key(|item| evidence_priority(item));
    if evidence.is_empty() {
        evidence = task.evidence.clone();
    }
    evidence
}

fn evidence_priority(item: &str) -> u8 {
    if item.starts_with("failure ") || item.starts_with("conflict ") {
        0
    } else if item.starts_with("failing-file ")
        || item.starts_with("files ")
        || item.starts_with("additions ")
        || item.starts_with("deletions ")
        || item.starts_with("command ")
        || item.starts_with("message ")
    {
        1
    } else if item.starts_with("tail ")
        || item.starts_with("rerun ")
        || item.starts_with("signals ")
        || item.starts_with("path ")
    {
        2
    } else if item.starts_with("lines ") || item.starts_with("changed ") {
        3
    } else if item.starts_with("patch ") {
        4
    } else {
        9
    }
}

fn evidence_row(item: &str) -> String {
    format!("  evidence {}", truncate(item, 64))
}

fn agent_task_next_action(task: &AgentTask) -> Option<&'static str> {
    match (
        task.kind.as_str(),
        task.status.as_str(),
        task.decision.as_deref(),
    ) {
        ("lane", "done", Some("accepted")) if task.workspace.is_some() => {
            Some("apply isolated changes")
        }
        ("lane", "blocked", _) => Some("resolve lane conflicts"),
        ("diff", "needs_input", _) => Some("review diff, then test or commit"),
        ("approval", "waiting_approval", _) => Some("approve, deny, or inspect diff"),
        ("test", "failed", _) => Some("open failure, patch, rerun tests"),
        ("tool", "failed", _) => Some("inspect failure evidence"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigStatus {
    Found,
    Missing,
}

impl ConfigStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigStatusRow {
    source: &'static str,
    path: PathBuf,
    status: ConfigStatus,
}

fn mcp_config_statuses(root: &Path) -> Vec<ConfigStatusRow> {
    mcp_config_candidates(root)
        .into_iter()
        .map(|(source, path)| {
            let status = if path.exists() {
                ConfigStatus::Found
            } else {
                ConfigStatus::Missing
            };
            ConfigStatusRow {
                source,
                path,
                status,
            }
        })
        .collect()
}

fn mcp_config_candidates(root: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut paths = vec![
        ("workspace", root.join(".mcp.json")),
        ("cursor", root.join(".cursor").join("mcp.json")),
    ];
    if let Some(home) = home_dir() {
        paths.push(("user", home.join(".codex").join("mcp.json")));
    }
    paths
}

fn count_skills(root: &Path) -> usize {
    skill_roots(root)
        .into_iter()
        .map(|root| count_skill_root(&root))
        .sum()
}

fn skill_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.join(".codex").join("skills")];
    if let Some(home) = home_dir() {
        roots.push(home.join(".codex").join("skills"));
        roots.push(home.join(".agents").join("skills"));
    }
    roots
}

fn count_skill_root(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .count()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(home) = env::var_os("HOME") {
        let home = home.to_string_lossy();
        if path.starts_with(home.as_ref()) {
            return path.replacen(home.as_ref(), "~", 1);
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{
        AgentJob, ProviderOption, ProviderStatus, TerminalLane, TuiEntry, WorkspaceSnapshot,
        lane_store_path,
    };

    fn test_state() -> TuiState {
        TuiState {
            session_id: "session".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            entries: Vec::<TuiEntry>::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
        }
    }

    #[test]
    fn ops_evidence_rows_surface_lane_next_actions() {
        let mut state = test_state();
        state.lanes.truncate(1);
        state.lanes[0].status = "accepted".to_string();
        state.lanes[0].worktree = Some(std::path::PathBuf::from("/tmp/robocode-lane"));

        let rendered = ops_evidence_rows(&state).join("\n");

        assert!(rendered.contains("L1 codex done"));
        assert!(rendered.contains("next apply"));
    }

    #[test]
    fn ops_evidence_rows_reuse_agent_task_runtime_view() {
        let mut state = test_state();
        state.entries.push(TuiEntry {
            label: "approval".to_string(),
            body: "Permission request for `write_file`\npath: src/lib.rs".to_string(),
        });
        state.entries.push(TuiEntry {
            label: "tool-call".to_string(),
            body: "shell command=cargo test -p robocode-cli".to_string(),
        });
        state.lanes.truncate(1);
        state.lanes[0].status = "running".to_string();
        state.lanes[0].title = "cargo test --workspace".to_string();
        state.lanes[0].summary = "Running test suite".to_string();
        state.workspace.agent_jobs = vec![AgentJob {
            id: "codex-ops".to_string(),
            kind: "run".to_string(),
            status: "running".to_string(),
            task: "review diagnostics".to_string(),
            pid: Some(4242),
            log_path: None,
            result_path: None,
            evidence: vec!["thread turn-123".to_string()],
            updated_at: 99,
        }];

        let rendered = ops_evidence_rows(&state).join("\n");

        assert!(rendered.contains("approval-1 robocode"));
        assert!(rendered.contains("waiting_approval"));
        assert!(rendered.contains("tool-2 robocode"));
        assert!(rendered.contains("testing"));
        assert!(rendered.contains("L1 codex testing"));
        assert!(rendered.contains("codex-ops codex thinking"));
        assert!(rendered.contains("evidence thread turn-123"));
    }

    #[test]
    fn ops_tests_lsp_rows_surface_test_and_diagnostics_evidence() {
        let mut state = test_state();
        state.entries.push(TuiEntry {
            label: "command".to_string(),
            body: [
                "Test result:",
                "  status: failed",
                "  exit code: 101",
                "  command: cargo test -p robocode-cli ops_",
                "  duration: 42ms",
                "  failure summary:",
                "    - assertion failed",
            ]
            .join("\n"),
        });
        state.workspace.diagnostics = vec!["src/render.rs:7:2 warning unused".to_string()];

        let rendered = ops_tests_lsp_rows(&state).join("\n");

        assert!(rendered.contains("TEST    failed  42ms  cargo test -p robocode-cli ops_"));
        assert!(rendered.contains("LSP     1 diagnostic(s)"));
        assert!(rendered.contains("src/render.rs:7:2 warning unused"));
    }

    #[test]
    fn ops_evidence_rows_prioritize_test_command_and_failure() {
        let mut state = test_state();
        state.entries.push(TuiEntry {
            label: "command".to_string(),
            body: [
                "Test result:",
                "  status: failed",
                "  exit code: 101",
                "  command: cargo test -p robocode-cli ops_",
                "  duration: 42ms",
                "  failure summary:",
                "    - assertion failed in ops_screen",
                "  failing files:",
                "    - src/tui/ops_screen.rs:42:9",
                "  output tail:",
                "    expected operation evidence",
            ]
            .join("\n"),
        });

        let rendered = ops_evidence_rows(&state).join("\n");

        assert!(rendered.find("test-1 shell failed") < rendered.find("L2 claude needs_input"));
        assert!(rendered.contains("test-1 shell failed"));
        assert!(rendered.contains("evidence command cargo test -p robocode-cli ops_"));
        assert!(rendered.contains("evidence failure assertion failed in ops_screen"));
        assert!(rendered.contains("evidence failing-file src/tui/ops_screen.rs:42:9"));
        assert!(rendered.contains("evidence tail expected operation evidence"));
        assert!(rendered.contains("next open failure, patch, rerun tests"));
    }

    #[test]
    fn ops_evidence_rows_surface_diff_review_action() {
        let mut state = test_state();
        state.lanes.clear();
        state.entries.push(TuiEntry {
            label: "command".to_string(),
            body: [
                "Git diff:",
                "  Summary: files=1 additions=5 deletions=1",
                "",
                "Diff:",
                "diff --git a/src/render.rs b/src/render.rs",
            ]
            .join("\n"),
        });

        let rendered = ops_evidence_rows(&state).join("\n");

        assert!(rendered.contains("diff-1 shell needs_input"));
        assert!(rendered.contains("next review diff, then test or commit"));
        assert!(rendered.contains("evidence files 1"));
        assert!(rendered.contains("evidence path src/render.rs"));
    }

    #[test]
    fn ops_evidence_rows_surface_lane_conflict_artifacts() {
        let root = temp_root();
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".robocode").join("lanes");
        std::fs::create_dir_all(&artifact_dir).expect("artifact dir");
        std::fs::write(
            artifact_dir.join("L1.apply-conflict.md"),
            [
                "# RoboCode Lane Apply Conflict",
                "",
                "Patch: /tmp/L1.apply.patch",
                "",
                "## Direct apply check",
                "error: patch failed: src/config.rs:42",
                "",
                "## Lane worktree changed files",
                "M src/config.rs",
            ]
            .join("\n"),
        )
        .expect("conflict artifact");
        let mut state = test_state();
        state.lane_store = Some(lane_store);
        state.lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "codex".to_string(),
            title: "apply config loader".to_string(),
            status: "apply_conflict".to_string(),
            target: "main".to_string(),
            progress: 100,
            summary: "apply conflict; report /tmp/L1.apply-conflict.md".to_string(),
            worktree: Some(root.join(".worktrees").join("L1")),
        }];

        let rendered = ops_evidence_rows(&state).join("\n");

        assert!(rendered.contains("L1 codex blocked"));
        assert!(rendered.contains("evidence conflict error: patch failed: src/config.rs:42"));
        assert!(rendered.contains("evidence changed M src/config.rs"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ops_evidence_rows_prioritize_app_server_final_message() {
        let mut state = test_state();
        state.lanes.truncate(1);
        state.lanes[0].status = "running".to_string();
        state.workspace.agent_jobs = vec![AgentJob {
            id: "codex-app".to_string(),
            kind: "app-server-turn".to_string(),
            status: "finished".to_string(),
            task: "text smoke".to_string(),
            pid: None,
            log_path: None,
            result_path: None,
            evidence: vec![
                "thread thread_app".to_string(),
                "turn turn_app".to_string(),
                "turn status completed".to_string(),
                "resume thread_app".to_string(),
                "message ROBOCODE_APP_SERVER_SMOKE_OK".to_string(),
            ],
            updated_at: 100,
        }];

        let rendered = ops_evidence_rows(&state).join("\n");

        assert!(rendered.contains("codex-app codex done"));
        assert!(rendered.find("codex-app codex done") < rendered.find("L1 codex testing"));
        assert!(rendered.contains("evidence message ROBOCODE_APP_SERVER_SMOKE_OK"));
        assert!(
            rendered.find("evidence message ROBOCODE_APP_SERVER_SMOKE_OK")
                < rendered.find("evidence thread thread_app")
        );
    }

    #[test]
    fn ops_context_and_extensions_rows_surface_real_config_counts() {
        let root = temp_root();
        std::fs::write(root.join(".mcp.json"), "{}").expect("write mcp config");
        std::fs::create_dir_all(root.join(".codex").join("skills").join("demo"))
            .expect("create skill dir");
        std::fs::write(
            root.join(".codex")
                .join("skills")
                .join("demo")
                .join("SKILL.md"),
            "# demo",
        )
        .expect("write skill");

        let mut state = test_state();
        state.workspace.root = root.clone();
        state.workspace.display_root = root.display().to_string();
        state.runtime_tasks.push(AgentTask {
            id: "turn-context".to_string(),
            parent_id: None,
            agent: "deepseek".to_string(),
            kind: "provider".to_string(),
            transport: "api".to_string(),
            title: "ContextBundle visibility".to_string(),
            status: "done".to_string(),
            activity: "context recorded".to_string(),
            summary: "provider context bundle".to_string(),
            progress: 100,
            started_at: None,
            updated_at: None,
            workspace: Some(root.display().to_string()),
            evidence: vec![
                "context_pressure 12% (1536/128000)".to_string(),
                "context_policy v1-priority-budget".to_string(),
                "context_omitted 2".to_string(),
            ],
            permissions: Vec::new(),
            decision: None,
            result: None,
            resume_handle: None,
            pid: None,
            next_action: None,
        });

        let context = ops_context_rows(&state).join("\n");
        let extensions = ops_extension_rows(&state).join("\n");

        assert!(context.contains("MCP configs 1"));
        assert!(context.contains("BUNDLE  pressure 12%"));
        assert!(context.contains("POLICY  v1-priority-budget"));
        assert!(context.contains("OMIT    2 source(s)"));
        assert!(context.contains("workspace  found"));
        assert!(context.contains("found"));
        assert!(extensions.contains("MCP      1 configured config(s)"));
        assert!(extensions.contains("SKILLS"));
        assert!(extensions.contains("discovered recipe(s)"));
    }

    #[test]
    fn render_ops_body_uses_0_1_6_ops_panel_names() {
        let state = test_state();
        let mut frame = Frame::new(80, 36);

        render_ops_body(&mut frame, &state);
        let rendered = frame.to_string();

        assert!(rendered.contains("TESTS / LSP"));
        assert!(rendered.contains("MCP / CONTEXT"));
        assert!(rendered.contains("EXTENSIONS"));
        assert!(rendered.contains("RECENT EVIDENCE"));
        assert!(!rendered.contains("RECENT EVENTS"));
        assert!(!rendered.contains("PROVIDER HEALTH"));
    }

    fn temp_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "robocode-ops-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
