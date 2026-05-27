use super::{
    canvas::Frame,
    composer::{COMPOSER_HEIGHT, render_composer},
    modal::render_overlays,
    ops_screen::render_ops_body,
    panel::panel,
    right_rail::right_rail,
    side_screen::render_side_body,
    state::{AgentTask, TuiState, agent_tasks},
    statusbar::{BOTTOM_BAR_HEIGHT, render_bottom_bar},
    text::truncate,
    topbar::{render_ops_top_bar, render_side_top_bar, render_top_bar},
    transcript::transcript_rows,
};
use std::time::{SystemTime, UNIX_EPOCH};

const MIN_WIDTH: usize = 80;
const MIN_HEIGHT: usize = 24;
const RIGHT_RAIL_WIDTH: usize = 38;
pub(super) fn render_frame(state: &TuiState, width: u16, height: u16) -> String {
    let width = (width as usize).max(MIN_WIDTH);
    let height = (height as usize).max(MIN_HEIGHT);
    let mut frame = Frame::new(width, height);

    render_top_bar(&mut frame, state);
    if width >= 112 {
        render_landscape_body(&mut frame, state);
    } else {
        render_compact_body(&mut frame, state);
    }
    render_composer(&mut frame, state, BOTTOM_BAR_HEIGHT);
    render_bottom_bar(&mut frame, state);
    render_overlays(&mut frame, state, RIGHT_RAIL_WIDTH);

    frame.to_string()
}

pub(super) fn render_side_frame(state: &TuiState, width: u16, height: u16) -> String {
    let width = (width as usize).max(MIN_WIDTH);
    let height = (height as usize).max(MIN_HEIGHT);
    let mut frame = Frame::new(width, height);

    render_side_top_bar(&mut frame, state);
    render_side_body(&mut frame, state);
    render_bottom_bar(&mut frame, state);

    frame.to_string()
}

pub(super) fn render_ops_frame(state: &TuiState, width: u16, height: u16) -> String {
    let width = (width as usize).max(MIN_WIDTH);
    let height = (height as usize).max(MIN_HEIGHT);
    let mut frame = Frame::new(width, height);

    render_ops_top_bar(&mut frame, state);
    render_ops_body(&mut frame, state);
    render_bottom_bar(&mut frame, state);

    frame.to_string()
}

fn render_landscape_body(frame: &mut Frame, state: &TuiState) {
    let body_top = 3;
    let body_bottom = frame.height - COMPOSER_HEIGHT - BOTTOM_BAR_HEIGHT - 1;
    let body_height = body_bottom.saturating_sub(body_top) + 1;
    let rail_left = frame.width - RIGHT_RAIL_WIDTH;
    let transcript_width = rail_left.saturating_sub(1);

    let transcript_rows = main_transcript_rows(
        state,
        transcript_width.saturating_sub(4),
        body_height.saturating_sub(2),
    );
    let transcript = panel(
        "TRANSCRIPT",
        transcript_rows,
        transcript_width,
        body_height,
        Some("live session"),
    );
    frame.write_block(body_top, 0, &transcript);

    let rail = right_rail(state, RIGHT_RAIL_WIDTH, body_height);
    frame.write_block(body_top, rail_left, &rail);
}

fn render_compact_body(frame: &mut Frame, state: &TuiState) {
    let body_top = 3;
    let body_bottom = frame.height - COMPOSER_HEIGHT - BOTTOM_BAR_HEIGHT - 1;
    let body_height = body_bottom.saturating_sub(body_top) + 1;
    let transcript_rows = main_transcript_rows(
        state,
        frame.width.saturating_sub(4),
        body_height.saturating_sub(2),
    );
    let transcript = panel(
        "TRANSCRIPT",
        transcript_rows,
        frame.width,
        body_height,
        None,
    );
    frame.write_block(body_top, 0, &transcript);
}

fn main_transcript_rows(state: &TuiState, width: usize, max_rows: usize) -> Vec<String> {
    let activity = operation_center_rows(state, width);
    let activity_rows = activity.len() + 1;
    let transcript_limit = max_rows.saturating_sub(activity_rows).max(1);
    let mut rows = activity;
    rows.push(activity_separator(width));
    rows.extend(recent_rows(transcript_rows(state, width), transcript_limit));
    rows
}

fn operation_center_rows(state: &TuiState, width: usize) -> Vec<String> {
    let status = live_activity_status(state);
    let mut rows = vec![truncate(
        &format!("  ◎ NOW WORKING  {}", status.summary),
        width,
    )];
    rows.push(truncate(
        &format!("     ┊  evidence: {}", status.evidence),
        width,
    ));
    rows.extend(
        status
            .details
            .iter()
            .take(2)
            .map(|detail| truncate(&format!("     ┊  {detail}"), width)),
    );
    rows
}

fn live_activity_status(state: &TuiState) -> LiveActivityStatus {
    // Priority mirrors operator urgency and reads from the normalized AgentTask
    // view so every panel describes the same runtime state.
    let active_agent_tasks = agent_tasks(state)
        .into_iter()
        .filter(AgentTask::is_active)
        .collect::<Vec<_>>();
    if !active_agent_tasks.is_empty() {
        let primary = &active_agent_tasks[0];
        let delegated_count = active_agent_tasks
            .iter()
            .filter(|task| matches!(task.kind.as_str(), "lane" | "job"))
            .count();
        let summary = operator_summary(primary, delegated_count);
        return LiveActivityStatus {
            summary,
            evidence: format!(
                "AgentTask {} from {}",
                primary.id,
                primary_task_signal(primary)
                    .as_deref()
                    .or_else(|| primary.evidence.first().map(String::as_str))
                    .unwrap_or("runtime view")
            ),
            details: active_agent_tasks
                .into_iter()
                .map(|task| operator_detail(&task))
                .collect(),
        };
    }

    if let Some(task) = agent_tasks(state).into_iter().rev().find(|task| {
        matches!(
            task.status.as_str(),
            "done" | "failed" | "cancelled" | "finished" | "observed" | "completed"
        ) && !task.evidence.is_empty()
    }) {
        return LiveActivityStatus {
            summary: historical_task_summary(&task),
            evidence: primary_task_signal(&task).unwrap_or_else(|| "agent task result".to_string()),
            details: vec![operator_detail(&task)],
        };
    }

    if let Some(entry) = state.entries.last() {
        return LiveActivityStatus {
            summary: compact_activity_label(entry.label.as_str()).to_string(),
            evidence: "latest transcript event".to_string(),
            details: vec![compact_activity_detail(&entry.body)],
        };
    }

    let provider_state = if state.provider_status.request_count == 0 {
        "idle; no provider request yet".to_string()
    } else {
        format!(
            "idle; last provider status {}",
            state.provider_status.connection
        )
    };
    LiveActivityStatus {
        summary: provider_state,
        evidence: "provider telemetry".to_string(),
        details: vec![format!("{} / {}", state.provider, state.model)],
    }
}

fn operator_summary(task: &AgentTask, delegated_count: usize) -> String {
    if delegated_count > 0 && matches!(task.kind.as_str(), "lane" | "job") {
        if task.status == "blocked" {
            return format!(
                "Supervising {} agent{}: blocked on {}",
                delegated_count,
                if delegated_count == 1 { "" } else { "s" },
                primary_task_signal(task).unwrap_or_else(|| task.activity.clone())
            );
        }
        return format!(
            "Supervising {} agent{}: {} {}",
            delegated_count,
            if delegated_count == 1 { "" } else { "s" },
            operator_agent_label(task),
            operator_status_label(task)
        );
    }
    match task.status.as_str() {
        "waiting_approval" => format!("Approval needed: {}", task.activity),
        "needs_input" if task.kind == "diff" => task.activity.clone(),
        "testing" => {
            if let Some(command) = evidence_value(task, "command ") {
                format!("Testing: {command}")
            } else {
                task.activity.clone()
            }
        }
        "editing" | "running_tool" => task.activity.clone(),
        "thinking" | "streaming" => format!("{} is thinking", operator_agent_label(task)),
        "needs_input" => format!("Needs input: {}", operator_agent_label(task)),
        "blocked" => format!(
            "Blocked: {}",
            primary_task_signal(task).unwrap_or_else(|| task.activity.clone())
        ),
        _ => task.activity.clone(),
    }
}

fn operator_detail(task: &AgentTask) -> String {
    let mut detail = format!(
        "{} {} {} {}%",
        task.id,
        operator_agent_label(task),
        operator_status_label(task),
        task.progress
    );
    if let Some(next) = next_operator_action(task) {
        detail.push_str(&format!(" :: next {next}"));
    } else if !task.title.is_empty() {
        detail.push_str(&format!(" :: {}", truncate(&task.title, 32)));
    }
    if let Some(signal) = primary_task_signal(task) {
        detail.push_str(&format!(" :: signal {signal}"));
    } else if !task.activity.is_empty() {
        detail.push_str(&format!(" :: {}", truncate(&task.activity, 32)));
    } else if !task.summary.is_empty() {
        detail.push_str(&format!(" :: {}", truncate(&task.summary, 32)));
    }
    if let Some(updated_at) = task.updated_at {
        detail.push_str(&format!(" :: updated {}", relative_millis(updated_at)));
    }
    detail
}

fn historical_task_summary(task: &AgentTask) -> String {
    match (task.kind.as_str(), task.status.as_str()) {
        ("test", "failed") => evidence_value(task, "command ")
            .map(|command| format!("Tests failed: {command}"))
            .unwrap_or_else(|| "Tests failed".to_string()),
        ("test", "done") => evidence_value(task, "command ")
            .map(|command| format!("Tests passed: {command}"))
            .unwrap_or_else(|| "Tests passed".to_string()),
        (_, "failed") => format!(
            "Latest {} failed: {}",
            operator_agent_label(task),
            primary_task_signal(task).unwrap_or_else(|| task.activity.clone())
        ),
        (_, "cancelled") => format!("Latest {} task cancelled", operator_agent_label(task)),
        _ => format!("Latest {} task {}", operator_agent_label(task), task.status),
    }
}

fn primary_task_signal(task: &AgentTask) -> Option<String> {
    // Operation center copy should surface the blocker or proof, not the lowest
    // level source label like "transcript tool-result".
    for prefix in [
        "failure ",
        "failing-file ",
        "conflict ",
        "approval ",
        "message ",
        "command ",
        "tail ",
        "rerun ",
        "summary ",
        "files ",
        "additions ",
        "deletions ",
        "changed ",
        "path ",
        "resume ",
        "thread ",
        "turn ",
    ] {
        if let Some(value) = evidence_value(task, prefix) {
            return Some(format!("{} {value}", prefix.trim()));
        }
    }
    task.evidence
        .iter()
        .find(|item| !item.starts_with("transcript "))
        .cloned()
}

fn evidence_value(task: &AgentTask, prefix: &str) -> Option<String> {
    task.evidence
        .iter()
        .find_map(|item| item.strip_prefix(prefix).map(str::trim))
        .map(|value| truncate(value, 88))
        .filter(|value| !value.is_empty())
}

fn next_operator_action(task: &AgentTask) -> Option<&'static str> {
    match task.status.as_str() {
        "waiting_approval" => Some("approve, diff, or deny"),
        "blocked" => Some("inspect conflict and revise/apply manually"),
        "failed" if task.kind == "test" => Some("open failure, patch, rerun tests"),
        "failed" => Some("inspect result and retry or discard"),
        "needs_input" if task.kind == "diff" => Some("review diff, then test or commit"),
        "needs_input" => Some("send follow-up to lane"),
        "testing" => Some("wait for test result"),
        "editing" | "running_tool" => Some("wait for tool result"),
        "done" => Some("review result or continue"),
        _ => None,
    }
}

fn operator_agent_label(task: &AgentTask) -> String {
    if task.agent == "robocode" && task.kind == "provider" {
        provider_display_name(&task.transport)
    } else {
        task.agent.clone()
    }
}

fn provider_display_name(provider: &str) -> String {
    match provider {
        "deepseek" => "DeepSeek".to_string(),
        "openai" => "OpenAI".to_string(),
        "anthropic" => "Anthropic".to_string(),
        "ollama" => "Ollama".to_string(),
        "fallback" => "Fallback".to_string(),
        other => other.to_string(),
    }
}

fn operator_status_label(task: &AgentTask) -> &'static str {
    match task.status.as_str() {
        "waiting_approval" => "waiting approval",
        "running_tool" => "using tool",
        "needs_input" => "needs input",
        "cancelled" => "cancelled",
        "archived" => "archived",
        "queued" => "queued",
        "thinking" => "thinking",
        "streaming" => "streaming",
        "editing" => "editing",
        "testing" => "testing",
        "blocked" => "blocked",
        "done" => "done",
        "failed" => "failed",
        _ => "active",
    }
}

fn compact_activity_label(label: &str) -> &'static str {
    match label {
        "assistant" => "reply ready",
        "tool-call" => "reply using tool",
        "tool-result" => "tool result ready",
        "system" => "system idle",
        _ => "session idle",
    }
}

fn compact_activity_detail(body: &str) -> String {
    let detail = body
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .unwrap_or_else(|| "no detail available".to_string());
    if let Some((tool, rest)) = detail.split_once(" path: ") {
        let path = rest.split_whitespace().next().unwrap_or(rest);
        if matches!(tool, "write_file" | "edit_file") {
            return format!("Editing {path}");
        }
    }
    detail
}

fn activity_separator(width: usize) -> String {
    truncate(
        &format!("     ┊  {}", "┄".repeat(width.saturating_sub(8).min(88))),
        width,
    )
}

struct LiveActivityStatus {
    summary: String,
    evidence: String,
    details: Vec<String>,
}

fn relative_millis(updated_at: u128) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(updated_at);
    let elapsed = now.saturating_sub(updated_at);
    if elapsed < 1_000 {
        "now".to_string()
    } else if elapsed < 60_000 {
        format!("{}s ago", elapsed / 1_000)
    } else if elapsed < 3_600_000 {
        format!("{}m ago", elapsed / 60_000)
    } else {
        format!("{}h ago", elapsed / 3_600_000)
    }
}

fn recent_rows(mut rows: Vec<String>, max_rows: usize) -> Vec<String> {
    if rows.len() > max_rows {
        rows = rows.split_off(rows.len() - max_rows);
    }
    while rows
        .first()
        .is_some_and(|row| is_loose_timeline_connector(row))
    {
        rows.remove(0);
    }
    rows
}

fn is_loose_timeline_connector(row: &str) -> bool {
    let trimmed = row.trim();
    trimmed == "│" || trimmed == "│  ·" || trimmed == "│ ·"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{
        state::{
            AgentJob, PendingTurn, ProviderStatus, TerminalLane, TuiEntry, TuiState,
            WorkspaceSnapshot,
        },
        text::char_width,
    };
    use robocode_core::ProviderTelemetry;
    use robocode_types::{TaskPriority, TaskRecord, TaskStatus};

    fn render_state() -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            entries: vec![TuiEntry {
                label: "assistant".to_string(),
                body: "hello".to_string(),
            }],
        }
    }

    fn preview_like_state() -> TuiState {
        let mut state = render_state();
        state.session_id = "c4f2b7e".to_string();
        state.lanes = TerminalLane::preview_lanes();
        state.input = "Add tests for load_config and summarize the diff".to_string();
        state.entries = vec![
            TuiEntry {
                label: "assistant".to_string(),
                body: "Tests are staged. I found one parser edge case.".to_string(),
            },
            TuiEntry {
                label: "tool-call".to_string(),
                body: "write_file path: src/config.rs lines: 1-120".to_string(),
            },
            TuiEntry {
                label: "approval".to_string(),
                body: "Permission request for `write_file`\npath: src/config.rs\nPress y to allow, n/Esc to deny.".to_string(),
            },
        ];
        state
    }

    fn assert_no_visual_regressions(rendered: &str) {
        let forbidden = [
            "SIDE MONITOR",
            "OPS MONITOR",
            "TERMINAL LANES DETAIL",
            "AGENT OUTPUT",
            "SYSTEM STATUS",
            "WORKSPACE MAP",
            "RECENT ACTIVITY",
            "PROVIDER / LIMITS",
            "Permission request for `write_file`",
        ];
        for fragment in forbidden {
            assert!(!rendered.contains(fragment), "{fragment}");
        }
        for line in rendered.lines() {
            assert_eq!(
                line.matches('[').count(),
                line.matches(']').count(),
                "{line}"
            );
        }
    }

    #[test]
    fn render_frame_includes_status_transcript_and_input() {
        let mut state = render_state();
        state.input = "/help".to_string();

        let rendered = render_frame(&state, 48, 10);

        assert!(rendered.contains("RoboCode"));
        assert!(rendered.contains("TRANSCRIPT"));
        assert!(rendered.contains("ASSISTANT"));
        assert!(rendered.contains("hello"));
        assert!(rendered.contains("› /help"));
        assert!(rendered.contains("/help"));
        assert!(rendered.contains("APPROVAL MODE:"));
        assert!(rendered.contains("CONNECTED"));
        for line in rendered.lines() {
            assert_eq!(
                line.matches('[').count(),
                line.matches(']').count(),
                "{line}"
            );
        }
    }

    #[test]
    fn render_frame_uses_cockpit_right_rail_when_wide() {
        let mut state = render_state();
        state.session_id = "session_123456789".to_string();
        state.provider = "deepseek".to_string();
        state.model = "deepseek-v4-flash".to_string();
        state.lanes = TerminalLane::preview_lanes();
        state.entries = vec![
            TuiEntry {
                label: "assistant".to_string(),
                body: "I'll update the renderer and keep the layout stable.".to_string(),
            },
            TuiEntry {
                label: "tool-call".to_string(),
                body: "write_file path: src/config.rs".to_string(),
            },
        ];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("Suggest"));
        assert!(rendered.contains("PERMISSIONS"));
        assert!(rendered.contains("WORKSPACE"));
        assert!(rendered.contains("ACTIVE TASKS"));
        assert!(rendered.contains("LSP DIAGNOSTICS"));
        assert!(rendered.contains("diagnostics unavailable"));
        assert!(rendered.contains("PROVIDER HEALTH"));
        assert!(rendered.contains("LATENCY"));
        assert!(rendered.contains("unavailable"));
        assert!(rendered.contains("TELEMETRY"));
        assert!(rendered.contains("CONTEXT"));
        assert!(!rendered.contains("312 ms"));
        assert!(!rendered.contains("28.4 t/s"));
        assert!(!rendered.contains("Implement load_config"));
        assert!(rendered.contains("L1 ⛭ codex"));
        assert!(rendered.contains("L2 ◆ claude"));
        assert!(rendered.contains("TOOL CALL"));
        assert!(rendered.contains("FILES    128"));
        assert!(rendered.contains("robocode/"));
        assert!(rendered.contains("LANGUAGE Rust"));
        assert!(rendered.contains("EDITION   2024"));
        assert!(rendered.contains("[GIT main"));
        assert!(!rendered.contains("[SYNC"));
        assert!(rendered.contains("EVENTS"));
        assert!(rendered.contains("LANES"));
        assert!(!rendered.contains("COST"));
        assert!(!rendered.contains("TIME"));
        assert!(rendered.contains("CONNECTED"));
        assert!(rendered.contains("Press ? for help"));
        assert!(rendered.contains("ACTIVE TASKS"));
        assert!(rendered.contains("[^R Regenerate]"));
        assert!(rendered.contains("[^N New Task]"));
        assert!(rendered.contains("APPROVAL MODE: [Suggest]"));

        let lines = rendered.lines().collect::<Vec<_>>();
        let recent_index = lines
            .iter()
            .position(|line| line.contains("RECENT FILES"))
            .expect("recent files panel");
        let composer_index = lines
            .iter()
            .position(|line| line.contains("RoboCode >_"))
            .expect("composer panel");
        assert!(composer_index > recent_index);
        assert!(lines[composer_index - 1].contains('└'));
        assert!(rendered.contains("[Rs] src/config.rs"));
    }

    #[test]
    fn render_frame_keeps_live_activity_visible_for_running_request() {
        let mut state = render_state();
        state.provider = "deepseek".to_string();
        state.model = "deepseek-v4-flash".to_string();
        state.entries.push(TuiEntry {
            label: "user".to_string(),
            body: "add tests and summarize".to_string(),
        });

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("NOW WORKING"));
        assert!(rendered.contains("DeepSeek is thinking"));
        assert!(rendered.contains("evidence: AgentTask reply-"));
    }

    #[test]
    fn render_frame_keeps_live_activity_visible_for_lanes_and_tool_calls() {
        let mut state = render_state();
        state.lanes = TerminalLane::preview_lanes();
        state.entries.push(TuiEntry {
            label: "tool-call".to_string(),
            body: "write_file path: src/render.rs lines: 1-20".to_string(),
        });

        let lane_rendered = render_frame(&state, 140, 36);

        assert!(lane_rendered.contains("NOW WORKING"));
        assert!(lane_rendered.contains("Supervising 2 agents: claude needs input"));
        assert!(lane_rendered.contains("evidence: AgentTask"));
        assert!(lane_rendered.contains("L1 codex testing 64%"));

        state.lanes.clear();
        let tool_rendered = render_frame(&state, 140, 36);

        assert!(tool_rendered.contains("Editing src/render.rs"));
    }

    #[test]
    fn render_frame_surfaces_failed_tests_after_approval_is_closed() {
        let mut state = render_state();
        state.entries = vec![
            TuiEntry {
                label: "approval".to_string(),
                body: "Permission request for `write_file`\npath: src/config.rs\nPress y to allow, n/Esc to deny.".to_string(),
            },
            TuiEntry {
                label: "approval".to_string(),
                body: "Approved `write_file`.".to_string(),
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
                    "    - src/config.rs:12:5",
                    "  output tail:",
                    "    thread 'config_tests' panicked at src/config.rs:12:5",
                ]
                .join("\n"),
            },
        ];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("Tests failed: cargo test -p robocode-cli config_tests"));
        assert!(rendered.contains("failure assertion failed in config_tests"));
        assert!(rendered.contains("next open failure, patch, rerun tests"));
        assert!(!rendered.contains("Approval needed"));
        assert!(!rendered.contains("APPROVAL REQUIRED"));
    }

    #[test]
    fn render_frame_surfaces_lane_conflict_as_operator_blocker() {
        let mut state = render_state();
        state.lanes = vec![TerminalLane {
            id: "L9".to_string(),
            tool: "codex".to_string(),
            title: "apply config loader".to_string(),
            status: "apply_conflict".to_string(),
            target: "main".to_string(),
            progress: 78,
            summary: "conflict error: patch failed: src/config.rs:42".to_string(),
            worktree: None,
        }];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("Supervising 1 agent: blocked on"));
        assert!(rendered.contains("summary conflict error: patch failed"));
        assert!(rendered.contains("next inspect conflict and revise/apply manually"));
    }

    #[test]
    fn render_frame_surfaces_diff_as_review_action() {
        let mut state = render_state();
        state.entries = vec![TuiEntry {
            label: "command".to_string(),
            body: [
                "Latest diff:",
                "  Summary: files=2 additions=12 deletions=3",
                "",
                "Diff:",
                "diff --git a/src/config.rs b/src/config.rs",
                "diff --git a/tests/config_tests.rs b/tests/config_tests.rs",
            ]
            .join("\n"),
        }];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("review diff: 2 file(s) +12 -3"));
        assert!(rendered.contains("next review diff, then test or commit"));
        assert!(rendered.contains("signal files 2"));
    }

    #[test]
    fn render_frame_surfaces_active_codex_jobs() {
        let mut state = render_state();
        state.workspace.agent_jobs = vec![AgentJob {
            id: "codex-123".to_string(),
            kind: "run".to_string(),
            status: "running".to_string(),
            task: "review payment refactor".to_string(),
            pid: Some(4242),
            log_path: None,
            result_path: None,
            evidence: vec!["thread thread_123".to_string(), "turn turn_123".to_string()],
            updated_at: 1,
        }];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("Supervising 1 agent: codex thinking"));
        assert!(rendered.contains("evidence: AgentTask codex-123"));
        assert!(rendered.contains("codex-123 codex thinking 65%"));
        assert!(rendered.contains("thread thread_123"));
        assert!(rendered.contains("codex"));
        assert!(rendered.contains("review payment"));
    }

    #[test]
    fn render_frame_surfaces_pending_provider_turn() {
        let mut state = render_state();
        state.provider = "deepseek".to_string();
        state.model = "deepseek-v4-flash".to_string();
        state.pending_turn = Some(PendingTurn {
            id: "turn-session-42".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            prompt: "implement config loader".to_string(),
            workspace: "/tmp/project".to_string(),
            started_at: 42,
        });

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("DeepSeek is thinking"));
        assert!(rendered.contains("evidence: AgentTask turn-session-42"));
        assert!(rendered.contains("live provider request"));
        assert!(rendered.contains("turn-session-42 DeepSeek thinking 15%"));
    }

    #[test]
    fn render_provider_health_uses_real_request_telemetry() {
        let mut state = render_state();
        state.provider_status = ProviderStatus::from_telemetry(&ProviderTelemetry {
            request_count: 2,
            success_count: 1,
            failure_count: 1,
            last_latency_ms: Some(42),
            average_latency_ms: Some(21),
            last_event_count: 3,
            last_error: Some("provider timeout".to_string()),
            ..ProviderTelemetry::default()
        });

        let rendered = render_frame(&state, 180, 36);

        assert!(rendered.contains("STATUS     Error"));
        assert!(rendered.contains("REQUESTS   1 ok / 1 err"));
        assert!(rendered.contains("LATENCY    last 42ms avg 21ms"));
        assert!(rendered.contains("ERROR      provider timeout"));
        assert!(!rendered.contains("312 ms"));
        assert!(!rendered.contains("28.4 t/s"));
    }

    #[test]
    fn render_provider_health_shows_real_usage_when_available() {
        let mut state = render_state();
        state.provider_status = ProviderStatus::from_telemetry(&ProviderTelemetry {
            request_count: 1,
            success_count: 1,
            last_latency_ms: Some(500),
            average_latency_ms: Some(500),
            last_event_count: 2,
            last_total_tokens: Some(1200),
            total_tokens: 2400,
            last_tokens_per_second: Some(2400),
            ..ProviderTelemetry::default()
        });

        let rendered = render_frame(&state, 180, 36);

        assert!(rendered.contains("TOKENS     last 1.2k total 2.4k"));
        assert!(rendered.contains("RATE       2.4k/s"));
    }

    #[test]
    fn render_right_rail_uses_real_workflow_tasks() {
        let mut state = render_state();
        state.tasks = vec![task_record(
            "task_active",
            "Ship active task panel",
            TaskStatus::InProgress,
        )];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("ACTIVE TASKS"));
        assert!(rendered.contains("task_ac/prog"));
        assert!(rendered.contains("Ship active task"));
        assert!(!rendered.contains("○ no active tasks"));
    }

    #[test]
    fn render_right_rail_uses_real_cached_lsp_diagnostics() {
        let mut state = render_state();
        state.workspace.diagnostics =
            vec!["src/lib.rs:7:2 warning [rust-analyzer/E0308] mismatched types".to_string()];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("LSP DIAGNOSTICS"));
        assert!(rendered.contains("src/lib.rs:7:2 warning"));
        assert!(!rendered.contains("diagnostics unavailable"));
    }

    #[test]
    fn render_frame_keeps_recent_transcript_rows_visible() {
        let mut state = render_state();
        state.entries = (0..12)
            .map(|index| TuiEntry {
                label: "assistant".to_string(),
                body: format!("event {index}"),
            })
            .collect();

        let rendered = render_frame(&state, 90, 24);

        assert!(rendered.contains("event 11"));
        assert!(!rendered.contains("event 0"));
    }

    fn task_record(task_id: &str, title: &str, status: TaskStatus) -> TaskRecord {
        TaskRecord {
            task_id: task_id.to_string(),
            title: title.to_string(),
            description: None,
            status,
            priority: TaskPriority::Medium,
            labels: Vec::new(),
            assignee_hint: None,
            parent_task_id: None,
            dependency_ids: Vec::new(),
            blocked_by: None,
            notes: Vec::new(),
            created_at: 1,
            updated_at: 2,
            last_session_id: None,
            last_seen_at: None,
            archived_at: None,
        }
    }

    #[test]
    fn render_frame_does_not_start_visible_transcript_on_connector() {
        let state = preview_like_state();

        let rendered = render_frame(&state, 140, 36);
        let lines = rendered.lines().collect::<Vec<_>>();
        let transcript_top = lines
            .iter()
            .position(|line| line.contains("TRANSCRIPT"))
            .expect("transcript panel");
        let first_content = lines[transcript_top + 1];

        assert!(!first_content.contains("│   │  ·"), "{first_content}");
        assert!(
            first_content.contains("USER")
                || first_content.contains("ASSISTANT")
                || first_content.contains("TOOL")
                || first_content.contains("APPROVAL")
                || first_content.contains("NOW WORKING"),
            "{first_content}"
        );
    }

    #[test]
    fn render_frame_keeps_wide_transcript_text_inside_terminal_width() {
        let mut state = render_state();
        state.entries = vec![TuiEntry {
            label: "assistant".to_string(),
            body: "我是 **RoboCode**，一个运行在终端里的 AI 编程助手 🤖\n有什么需要帮忙的吗？"
                .to_string(),
        }];

        let width = 202usize;
        let rendered = render_frame(&state, width as u16, 58);

        for line in rendered.lines() {
            assert!(
                char_width(line) <= width,
                "line display width {} exceeded {width}: {line}",
                char_width(line)
            );
        }
        assert!(rendered.contains("PROVIDER HEALTH"));
        assert!(rendered.contains("RECENT FILES"));
    }

    #[test]
    fn render_frame_keeps_unicode_rows_at_terminal_cell_width() {
        let mut state = render_state();
        state.provider = "deepseek".to_string();
        state.model = "deepseek-v4-flash-中文👋🏻".to_string();
        state.input = "你好，帮我检查右侧栏是否会跑偏".to_string();
        state.workspace.display_root = "~/项目/robocode".to_string();
        state.workspace.git_branch = "feature/中文-ui".to_string();
        state.workspace.recent_files[0].path = "src/界面/输入法.rs".to_string();
        state.workspace.top_files[0] = "src/界面/".to_string();
        state.workspace.primary_language = "Rust🦀".to_string();
        state.entries = vec![TuiEntry {
            label: "assistant".to_string(),
            body: "可以，我会先检查中文输入、emoji 👋🏻 和长路径是否把右侧栏挤歪。这里是一段没有空格的中文文本用于触发按显示宽度换行。".to_string(),
        }];

        let width = 202usize;
        let rendered = render_frame(&state, width as u16, 58);

        for line in rendered.lines() {
            assert_eq!(
                char_width(line),
                width,
                "line display width {} differed from {width}: {line}",
                char_width(line)
            );
        }
        assert!(rendered.contains("PROVIDER HEALTH"));
        assert!(rendered.contains("RECENT FILES"));
    }

    #[test]
    fn render_frame_overlays_approval_modal() {
        let mut state = render_state();
        state.entries = vec![TuiEntry {
            label: "approval".to_string(),
            body: "Permission request for `write_file`\npath: src/lib.rs\nPress y to allow, n/Esc to deny.".to_string(),
        }];

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("APPROVAL REQUIRED"));
        assert!(rendered.contains("ID: call_7f2a9c1e"));
        assert!(rendered.contains("ACTION  Write (new content)"));
        assert!(rendered.contains("MODIFIES FILE"));
        assert!(rendered.contains("PREVIEW (first 20 lines)"));
        assert!(rendered.contains("SIZE    +48 lines"));
        assert!(rendered.contains("│ + 1 │"));
        assert!(rendered.contains("load_config"));
        assert!(rendered.contains("Apply to all write_file calls"));
        assert!(rendered.contains("write_file"));
        assert!(rendered.contains("src/lib.rs"));
        assert!(rendered.contains("[Approve (y)]"));
        assert!(rendered.contains("[Deny (n)]"));
        assert!(rendered.contains("[Diff]"));
        assert!(rendered.contains("APPROVAL MODE: [Suggest]"));
    }

    #[test]
    fn render_frame_overlays_focused_lane_modal() {
        let mut state = render_state();
        state.lanes = TerminalLane::preview_lanes();
        state.focused_lane = Some("L1".to_string());

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("LANE DETAIL"));
        assert!(rendered.contains("L1 codex"));
        assert!(rendered.contains("[codex tty]"));
        assert!(rendered.contains("PTY    pty/01"));
        assert!(rendered.contains("PID ----"));
        assert!(rendered.contains("ROUTE main→side-1"));
        assert!(rendered.contains("STATE"));
        assert!(rendered.contains("CMD    codex exec test fixes"));
        assert!(rendered.contains("ATTACH /lane tmux L1"));
        assert!(rendered.contains("patched failing tests"));
        assert!(rendered.contains("CONTROL [stop] [tmux] [pty] [send] [inspect]"));
        assert!(rendered.contains("--tui-screen side-1"));
        for line in rendered.lines() {
            assert_eq!(
                line.matches('[').count(),
                line.matches(']').count(),
                "{line}"
            );
        }
    }

    #[test]
    fn render_frame_shows_slash_command_suggestions_above_composer() {
        let mut state = render_state();
        state.input = "/p".to_string();

        let rendered = render_frame(&state, 140, 36);

        assert!(rendered.contains("COMMANDS"));
        assert!(rendered.contains("↑↓ tab enter esc"));
        assert!(rendered.contains("› /provider"));
        assert!(rendered.contains("/plan"));
        assert!(rendered.contains("List or switch providers"));
    }

    #[test]
    fn render_frame_keeps_fixed_width_outer_edges() {
        let mut state = render_state();
        state.input = "Add tests for load_config and summarize the diff".to_string();

        let rendered = render_frame(&state, 140, 36);
        let lines = rendered.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 36);
        for line in lines {
            assert_eq!(char_width(line), 140, "{line}");
        }
    }

    #[test]
    fn render_cockpit_screens_avoid_legacy_or_half_chip_text() {
        let state = preview_like_state();
        let main = render_frame(&state, 140, 36);
        let side = render_side_frame(&state, 80, 36);
        let ops = render_ops_frame(&state, 80, 36);

        assert_no_visual_regressions(&main);
        assert_no_visual_regressions(&side);
        assert_no_visual_regressions(&ops);
        assert!(main.contains("[PERMISSIONS"));
        assert!(side.contains("[FOCUS tail]"));
        assert!(ops.contains("TESTS / LSP"));
        assert!(ops.contains("LSP     0 diagnostic(s)"));
    }

    #[test]
    fn render_side_frame_focuses_on_lane_monitoring() {
        let mut state = render_state();
        state.lanes = TerminalLane::preview_lanes();

        let rendered = render_side_frame(&state, 80, 36);

        assert!(rendered.contains("SIDE-1"));
        assert!(rendered.contains("[LINK main]"));
        assert!(rendered.contains("AGENT LANES"));
        assert!(rendered.contains("LIVE OUTPUT"));
        assert!(rendered.contains("SIDE STATUS"));
        assert!(rendered.contains("┌ ● L1 codex"));
        assert!(rendered.contains("PTY pty/01"));
        assert!(rendered.contains("PID ----"));
        assert!(rendered.contains("ATTACH /lane tmux L1"));
        assert!(rendered.contains("└ CMD codex exec test fixes"));
        assert!(rendered.contains("TASK test fixes"));
        assert!(rendered.contains("tail patched failing tests"));
        assert!(rendered.contains("LANES tail persisted logs"));
        assert!(rendered.contains("CONTROL inspect stop route"));
        assert!(rendered.contains("tmux attach -t robocode-c4f2b7e-l2"));
        assert!(rendered.contains("tmux session ready"));
        assert!(rendered.contains("patched failing tests"));
        assert!(!rendered.contains("approval write_file"));
        assert!(rendered.contains("CONTEXT"));
        assert!(!rendered.contains("Type instruction"));

        let lines = rendered.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 36);
        for line in lines {
            assert_eq!(line.chars().count(), 80, "{line}");
        }
    }

    #[test]
    fn render_ops_frame_focuses_on_workspace_and_diagnostics() {
        let mut state = render_state();
        state.lanes = TerminalLane::preview_lanes();
        state.entries = vec![TuiEntry {
            label: "approval".to_string(),
            body: "Permission request for `write_file`\npath: src/config.rs\nPress y to allow, n/Esc to deny.".to_string(),
        }];

        let rendered = render_ops_frame(&state, 80, 36);

        assert!(rendered.contains("SIDE-2"));
        assert!(rendered.contains("[LINK side-1]"));
        assert!(rendered.contains("TESTS / LSP"));
        assert!(rendered.contains("MCP / CONTEXT"));
        assert!(rendered.contains("EXTENSIONS"));
        assert!(rendered.contains("RECENT EVIDENCE"));
        assert!(rendered.contains("workspace"));
        assert!(rendered.contains("files 128"));
        assert!(rendered.contains("TEST    no /test evidence yet"));
        assert!(rendered.contains("LSP     0 diagnostic(s)"));
        assert!(rendered.contains("auto-checks or /lsp diagnostics"));
        assert!(rendered.contains("approval-1 robocode waiting_approval"));
        assert!(rendered.contains("next approve, deny, or inspect diff"));
        assert!(rendered.contains("L1 codex testing"));
        assert!(rendered.contains("L2 claude needs_input"));
        assert!(rendered.contains("evidence path: src/config.rs"));
        assert!(!rendered.contains("Type instruction"));

        let lines = rendered.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 36);
        for line in lines {
            assert_eq!(line.chars().count(), 80, "{line}");
        }
    }
}
