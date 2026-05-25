use super::state::{
    CompanionScreen, ProviderStatus, TerminalLane, TuiEntry, TuiState, WorkspaceSnapshot,
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
    TuiState {
        session_id: "c4f2b7e".to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        provider_status: ProviderStatus::configured(),
        theme_name: theme_name.to_string(),
        input: "Add tests for load_config and summarize the diff".to_string(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        workspace: WorkspaceSnapshot::fixture(),
        tasks: preview_tasks(),
        memory: preview_memory(),
        screens: preview_screens(),
        lanes: TerminalLane::preview_lanes(),
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
        ],
    }
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
    state.input = "/git push origin rel".to_string();
    state.command_selection = 0;
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_use_stable_demo_workspace_snapshot() {
        let main = render_preview("fallback", "test-local");
        let idle = render_idle_preview("fallback", "test-local");
        let command_palette = render_command_palette_preview("fallback", "test-local");
        let side = render_side_preview("fallback", "test-local");
        let ops = render_ops_preview("fallback", "test-local");

        assert!(main.contains("~/projects/robocode"));
        assert!(idle.contains("~/projects/robocode"));
        assert!(main.contains("src/config.rs"));
        assert!(idle.contains("No approval is blocking right now"));
        assert!(command_palette.contains("COMMANDS"));
        assert!(command_palette.contains("› /git push origin release/v0.1.3"));
        assert!(command_palette.contains("Remote branch origin/release/v0.1.3"));
        assert!(!idle.contains("APPROVAL REQUIRED"));
        assert!(main.contains("tests/config_tests.rs"));
        assert!(side.contains("~/projects/robocode"));
        assert!(ops.contains("files 128"));
        assert!(!main.contains("docs/previews/generated"));
        assert!(!main.contains("scripts/tui-previews.sh"));
    }

    #[test]
    fn lane_preview_focuses_lane_detail_without_approval_overlay() {
        let preview = render_lane_preview("fallback", "test-local");

        assert!(preview.contains("LANE DETAIL"));
        assert!(preview.contains("ROUTE main→side-1"));
        assert!(preview.contains("CMD    codex exec test fixes"));
        assert!(!preview.contains("APPROVAL REQUIRED"));
    }
}
