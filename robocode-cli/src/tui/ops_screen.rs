use std::{
    env, fs,
    path::{Path, PathBuf},
};

use super::{
    canvas::Frame,
    indicators::progress_bar,
    lane::{command_hint, lane_next_action, pty_label, status_badge},
    panel::panel,
    state::TuiState,
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
    let active_lanes = state
        .lanes
        .iter()
        .filter(|lane| !matches!(lane.status.as_str(), "idle" | "done" | "archived"))
        .count();
    let mut rows = vec![
        format!(
            "PROVIDER {:<10} {}",
            state.provider_status.connection, state.provider_status.telemetry
        ),
        format!("CATALOG  {} provider(s)", state.provider_catalog.len()),
        format!(
            "AGENTS   {} active / {} lane(s)",
            active_lanes,
            state.lanes.len()
        ),
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
    for lane in state.lanes.iter().take(4) {
        rows.push(format!(
            "{} {:<10} {:<10} {} {}",
            lane.id,
            truncate(terminal_label_for_ops(&lane.tool), 10),
            status_badge(&lane.status),
            progress_bar(lane.progress)
                .split_whitespace()
                .next()
                .unwrap_or("░░░░░"),
            truncate(
                &format!(
                    "{} :: {} :: next {}",
                    pty_label(&lane.tool),
                    command_hint(&lane.tool, &lane.title),
                    lane_next_action(lane)
                ),
                54
            )
        ));
    }
    rows.extend(recent_evidence_entries(state).into_iter().take(3));
    if rows.is_empty() {
        rows.push("no tool/test/lane evidence yet".to_string());
    }
    rows
}

fn recent_evidence_entries(state: &TuiState) -> Vec<String> {
    state
        .entries
        .iter()
        .rev()
        .filter(|entry| {
            matches!(
                entry.label.as_str(),
                "tool-call" | "tool-result" | "approval" | "command"
            ) || entry.body.contains("Test result:")
        })
        .map(|entry| {
            format!(
                "main {:<8} {}",
                truncate(&entry.label, 8),
                truncate(&compact_activity(entry), 56)
            )
        })
        .collect()
}

fn terminal_label_for_ops(tool: &str) -> &'static str {
    match tool {
        "codex" => "codex tty",
        "claude" => "claude tty",
        "shell" | "run" => "ops tty",
        _ => "agent tty",
    }
}

fn compact_activity(entry: &super::state::TuiEntry) -> String {
    if entry.label == "approval" && entry.body.contains("write_file") {
        let path = entry
            .body
            .lines()
            .find_map(|line| line.strip_prefix("path: "))
            .unwrap_or("workspace");
        return format!("[waiting] write_file {path}");
    }
    entry.body.replace('\n', " / ")
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
        ProviderOption, ProviderStatus, TerminalLane, TuiEntry, WorkspaceSnapshot,
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
            entries: Vec::<TuiEntry>::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
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

        assert!(rendered.contains("L1 codex tty"));
        assert!(rendered.contains("next apply"));
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

        let context = ops_context_rows(&state).join("\n");
        let extensions = ops_extension_rows(&state).join("\n");

        assert!(context.contains("MCP configs 1"));
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
