use super::state::{
    AgentJob, AgentTask, CompanionScreen, PendingTurn, ProviderOption, ProviderStatus,
    TerminalLane, TuiEntry, TuiState, WorkspaceSnapshot,
};
use super::{render, terminal};
use robocode_types::{
    MemoryEntry, MemoryKind, MemoryScope, MemorySource, MemoryStatus, TaskPriority, TaskRecord,
    TaskStatus,
};

pub(crate) fn render_preview(provider: &str, model: &str) -> String {
    let state = preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 140, 40)
}

pub(crate) fn render_idle_preview(provider: &str, model: &str) -> String {
    let state = idle_preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 140, 40)
}

pub(crate) fn render_command_palette_preview(provider: &str, model: &str) -> String {
    let state = command_palette_preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 140, 40)
}

pub(crate) fn render_live_turn_preview(provider: &str, model: &str) -> String {
    let state = live_turn_preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 140, 40)
}

pub(crate) fn render_resize_preview(provider: &str, model: &str) -> String {
    let state = resize_preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 100, 30)
}

pub(crate) fn render_cjk_input_preview(provider: &str, model: &str) -> String {
    let state = cjk_input_preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 100, 30)
}

pub(crate) fn render_ansi_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 140, 40),
        Some(theme_name),
    )
}

pub(crate) fn render_ansi_command_palette_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = command_palette_preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 140, 40),
        Some(theme_name),
    )
}

pub(crate) fn render_ansi_idle_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = idle_preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 140, 40),
        Some(theme_name),
    )
}

pub(crate) fn render_ansi_live_turn_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = live_turn_preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 140, 40),
        Some(theme_name),
    )
}

pub(crate) fn render_ansi_resize_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = resize_preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 100, 30),
        Some(theme_name),
    )
}

pub(crate) fn render_ansi_cjk_input_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = cjk_input_preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 100, 30),
        Some(theme_name),
    )
}

pub(crate) fn render_lane_preview(provider: &str, model: &str) -> String {
    let state = focused_lane_preview_state(provider, model, "aurora-cyan");
    render::render_frame(&state, 140, 40)
}

pub(crate) fn render_ansi_lane_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = focused_lane_preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_frame(&state, 140, 40),
        Some(theme_name),
    )
}

pub(crate) fn render_side_preview(provider: &str, model: &str) -> String {
    let state = preview_state(provider, model, "aurora-cyan");
    render::render_side_frame(&state, 80, 40)
}

pub(crate) fn render_ansi_side_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_side_frame(&state, 80, 40),
        Some(theme_name),
    )
}

pub(crate) fn render_ops_preview(provider: &str, model: &str) -> String {
    let state = preview_state(provider, model, "aurora-cyan");
    render::render_ops_frame(&state, 80, 40)
}

pub(crate) fn render_ansi_ops_preview_with_theme(
    provider: &str,
    model: &str,
    theme_name: Option<&str>,
) -> String {
    let theme_name = theme_name.unwrap_or("aurora-cyan");
    let state = preview_state(provider, model, theme_name);
    terminal::render_ansi_preview_with_theme(
        &render::render_ops_frame(&state, 80, 40),
        Some(theme_name),
    )
}

fn preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut workspace = WorkspaceSnapshot::fixture();
    workspace.agent_jobs = preview_agent_jobs();
    TuiState {
        session_id: "c4f2b7e".to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        provider_catalog: ProviderOption::fixture(),
        provider_status: ProviderStatus::configured(),
        theme_name: theme_name.to_string(),
        input: "Add tests for load_config and summarize the diff".to_string(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        pending_turn: None,
        workspace,
        tasks: preview_tasks(),
        runtime_tasks: preview_runtime_tasks(),
        memory: preview_memory(),
        screens: preview_screens(),
        lanes: preview_lanes(),
        lane_store: None,
        focused_lane: None,
        entries: vec![
            TuiEntry {
                label: "user".to_string(),
                body: "Add a new function `load_config` that reads a TOML config file and returns `Config`.".to_string(),
            },
            TuiEntry {
                label: "assistant".to_string(),
                body: "I'll add `load_config` to `src/config.rs`, then cover success and error cases with focused tests.".to_string(),
            },
            TuiEntry {
                label: "tool-call".to_string(),
                body: "write_file path: tests/config_tests.rs lines: 1-200".to_string(),
            },
            TuiEntry {
                label: "tool-result".to_string(),
                body: "write_file completed\nWrote 86 lines to tests/config_tests.rs (3.4 KB)".to_string(),
            },
            TuiEntry {
                label: "assistant".to_string(),
                body: "Tests are staged. I found one parser edge case and need to update `src/config.rs` before running the suite.".to_string(),
            },
            TuiEntry {
                label: "user".to_string(),
                body: "Good. Keep the change narrow and show me the diff before applying.".to_string(),
            },
            TuiEntry {
                label: "tool-call".to_string(),
                body: "write_file path: src/config.rs lines: 1-120".to_string(),
            },
            TuiEntry {
                label: "approval".to_string(),
                body: "Permission request for `write_file`\npath: src/config.rs\nPress y to allow, n/Esc to deny.".to_string(),
            },
            TuiEntry {
                label: "command".to_string(),
                body: [
                    "Test result:",
                    "  status: failed",
                    "  exit code: 101",
                    "  command: cargo test -p robocode-cli config_tests",
                    "  duration: 42ms",
                    "  failure summary:",
                    "    - assertion failed in config_tests",
                    "  failing files:",
                    "    - src/config.rs:42:15",
                    "  output tail:",
                    "    thread 'config_tests' panicked at src/config.rs:42:15",
                ]
                .join("\n"),
            },
        ],
    }
}

fn preview_agent_jobs() -> Vec<AgentJob> {
    vec![AgentJob {
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
        updated_at: 42,
    }]
}

fn preview_runtime_tasks() -> Vec<AgentTask> {
    vec![AgentTask {
        id: "turn-context-preview".to_string(),
        parent_id: None,
        agent: "deepseek".to_string(),
        kind: "provider".to_string(),
        transport: "api".to_string(),
        title: "ContextBundle v1 visibility".to_string(),
        status: "done".to_string(),
        activity: "context bundle recorded".to_string(),
        summary: "policy v1-priority-budget, 1 omitted source".to_string(),
        progress: 100,
        started_at: Some(1),
        updated_at: Some(2),
        workspace: Some("~/Documents/GitHub/robocode".to_string()),
        evidence: vec![
            "context_pressure 18% (23040/128000)".to_string(),
            "context_sources 6".to_string(),
            "context_policy v1-priority-budget".to_string(),
            "context_omitted 1".to_string(),
            "largest_context_source latest-diff 6400 tok".to_string(),
        ],
        permissions: Vec::new(),
        decision: Some("recorded".to_string()),
        result: Some("visible in side-2 ops".to_string()),
        resume_handle: None,
        pid: None,
        next_action: None,
    }]
}

fn preview_lanes() -> Vec<TerminalLane> {
    let mut lanes = TerminalLane::preview_lanes();
    lanes.truncate(1);
    lanes
}

fn preview_screens() -> Vec<CompanionScreen> {
    vec![
        CompanionScreen {
            id: "side-1".to_string(),
            title: "Agent lanes".to_string(),
            status: "launched".to_string(),
            pid: Some(4101),
            summary: "lane cockpit on companion display".to_string(),
        },
        CompanionScreen {
            id: "side-2".to_string(),
            title: "Workspace ops".to_string(),
            status: "launched".to_string(),
            pid: Some(4102),
            summary: "ops monitor on vertical display".to_string(),
        },
    ]
}

fn preview_tasks() -> Vec<TaskRecord> {
    vec![
        TaskRecord {
            task_id: "task_load_config".to_string(),
            title: "Implement load_config".to_string(),
            description: None,
            status: TaskStatus::InProgress,
            priority: TaskPriority::High,
            labels: vec!["tui".to_string()],
            assignee_hint: Some("main".to_string()),
            parent_task_id: None,
            dependency_ids: Vec::new(),
            blocked_by: None,
            notes: Vec::new(),
            created_at: 1,
            updated_at: 2,
            last_session_id: Some("c4f2b7e".to_string()),
            last_seen_at: None,
            archived_at: None,
        },
        TaskRecord {
            task_id: "task_review_tests".to_string(),
            title: "Review config tests".to_string(),
            description: None,
            status: TaskStatus::Blocked,
            priority: TaskPriority::Medium,
            labels: vec!["review".to_string()],
            assignee_hint: Some("side-1".to_string()),
            parent_task_id: None,
            dependency_ids: Vec::new(),
            blocked_by: Some("approval".to_string()),
            notes: Vec::new(),
            created_at: 1,
            updated_at: 2,
            last_session_id: Some("c4f2b7e".to_string()),
            last_seen_at: None,
            archived_at: None,
        },
    ]
}

fn preview_memory() -> Vec<MemoryEntry> {
    vec![
        MemoryEntry {
            memory_id: "mem_tui_standard".to_string(),
            scope: MemoryScope::Project,
            session_id: Some("c4f2b7e".to_string()),
            kind: MemoryKind::Convention,
            content: "Keep TUI docs and previews in the same change as UI behavior.".to_string(),
            source: MemorySource::AssistantSuggestion,
            status: MemoryStatus::Suggested,
            created_at: 1,
            updated_at: 2,
            related_task_ids: Vec::new(),
            confidence_hint: Some("preview".to_string()),
        },
        MemoryEntry {
            memory_id: "mem_theme".to_string(),
            scope: MemoryScope::Session,
            session_id: Some("c4f2b7e".to_string()),
            kind: MemoryKind::Preference,
            content: "Use aurora-cyan as the default cockpit theme.".to_string(),
            source: MemorySource::Command,
            status: MemoryStatus::Active,
            created_at: 1,
            updated_at: 2,
            related_task_ids: Vec::new(),
            confidence_hint: None,
        },
    ]
}

fn focused_lane_preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut state = preview_state(provider, model, theme_name);
    state.focused_lane = Some("L1".to_string());
    state
        .entries
        .retain(|entry| entry.label != "approval" && !entry.body.contains("Press y"));
    state.input = "/lane inspect L1".to_string();
    state
}

fn idle_preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut state = preview_state(provider, model, theme_name);
    state
        .entries
        .retain(|entry| entry.label != "approval" && !entry.body.contains("Press y"));
    state.entries.push(TuiEntry {
        label: "assistant".to_string(),
        body: "No approval is blocking right now. The cockpit stays open for transcript review, input, diagnostics, and lane status.".to_string(),
    });
    state.input = "Review current diff, then run tests when ready".to_string();
    state
}

fn command_palette_preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut state = idle_preview_state(provider, model, theme_name);
    state.input = "/lane a".to_string();
    state.command_selection = 1;
    state
}

fn live_turn_preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut state = idle_preview_state(provider, model, theme_name);
    state.entries = vec![
        TuiEntry {
            label: "system".to_string(),
            body: "RoboCode TUI ready. Enter submits. Esc or Ctrl-C exits.".to_string(),
        },
        TuiEntry {
            label: "user".to_string(),
            body: "Refactor the config loader, then run focused tests.".to_string(),
        },
    ];
    state.pending_turn = Some(PendingTurn {
        id: "turn-c4f2b7e-live".to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        prompt: "Refactor the config loader, then run focused tests.".to_string(),
        workspace: state.workspace.display_root.clone(),
        started_at: 1,
        phase: "Waiting for provider response".to_string(),
        next_action: "wait".to_string(),
    });
    state.workspace.agent_jobs.clear();
    state.lanes.clear();
    state.input = "Add a note about the validation result".to_string();
    state
}

fn resize_preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut state = live_turn_preview_state(provider, model, theme_name);
    state.input = "Resize-safe redraw check".to_string();
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: "Resize-safe redraw check: stale borders cleared; composer and panels reflow from one frame.".to_string(),
    });
    state
}

fn cjk_input_preview_state(provider: &str, model: &str, theme_name: &str) -> TuiState {
    let mut state = idle_preview_state(provider, model, theme_name);
    state.input = "你好，帮我检查当前变更".to_string();
    state.entries.push(TuiEntry {
        label: "user".to_string(),
        body: "中文输入法候选窗应该靠近 composer 光标，输入区要保持足够高。".to_string(),
    });
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_use_stable_demo_workspace_snapshot() {
        let main = render_preview("fallback", "test-local");
        let idle = render_idle_preview("fallback", "test-local");
        let live_turn = render_live_turn_preview("fallback", "test-local");
        let resize = render_resize_preview("fallback", "test-local");
        let cjk_input = render_cjk_input_preview("fallback", "test-local");
        let command_palette = render_command_palette_preview("fallback", "test-local");
        let side = render_side_preview("fallback", "test-local");
        let ops = render_ops_preview("fallback", "test-local");

        assert!(main.contains("~/projects/robocode"));
        assert!(idle.contains("~/projects/robocode"));
        assert!(live_turn.contains("Fallback is thinking"));
        assert!(live_turn.contains("live provider request"));
        assert!(resize.contains("NOW WORKING"));
        assert!(resize.contains("Resize-safe redraw check"));
        assert!(cjk_input.contains("你好，帮我检查当前变更"));
        assert!(main.contains("src/config.rs"));
        assert!(idle.contains("No approval is blocking right now"));
        assert!(command_palette.contains("COMMANDS"));
        assert!(command_palette.contains("› /lane artifacts"));
        assert!(command_palette.contains("/lane apply"));
        assert!(!idle.contains("APPROVAL REQUIRED"));
        assert!(!main.contains("APPROVAL REQUIRED"));
        assert!(main.contains("tests/config_tests.rs"));
        assert!(side.contains("~/projects/robocode"));
        assert!(ops.contains("files 128"));
        assert!(ops.contains("codex-app codex done"));
        assert!(ops.contains("evidence message ROBOCODE_APP_SERVER_SMOKE_OK"));
        assert!(!main.contains("docs/previews/generated"));
        assert!(!main.contains("scripts/tui-previews.sh"));
    }

    #[test]
    fn lane_preview_focuses_lane_detail_without_approval_overlay() {
        let preview = render_lane_preview("fallback", "test-local");

        assert!(preview.contains("LANE DETAIL"));
        assert!(preview.contains("ROUTE main→side-1"));
        assert!(preview.contains("CMD    codex exec test fixes"));
        assert!(preview.contains("ATTACH /lane tmux L1"));
        assert!(!preview.contains("APPROVAL REQUIRED"));
    }
}
