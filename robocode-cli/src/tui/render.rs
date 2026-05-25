use super::{
    canvas::Frame,
    composer::{COMPOSER_HEIGHT, render_composer},
    modal::render_overlays,
    ops_screen::render_ops_body,
    panel::panel,
    right_rail::right_rail,
    side_screen::render_side_body,
    state::TuiState,
    statusbar::{BOTTOM_BAR_HEIGHT, render_bottom_bar},
    topbar::{render_ops_top_bar, render_side_top_bar, render_top_bar},
    transcript::transcript_rows,
};

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

    let transcript_rows = recent_rows(
        transcript_rows(state, transcript_width.saturating_sub(4)),
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
    let transcript_rows = recent_rows(
        transcript_rows(state, frame.width.saturating_sub(4)),
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
        state::{ProviderStatus, TerminalLane, TuiEntry, TuiState, WorkspaceSnapshot},
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
        assert!(rendered.contains("L1 ● codex"));
        assert!(rendered.contains("L2 ◐ claude"));
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
                || first_content.contains("APPROVAL"),
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
        assert!(rendered.contains("patched failing tests"));
        assert!(rendered.contains("CONTROL [stop] [view] [route] [side-2]"));
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
            assert_eq!(line.chars().count(), 140, "{line}");
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
        assert!(ops.contains("diagnostics unavailable"));
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
        assert!(rendered.contains("TASK test fixes"));
        assert!(rendered.contains("└ CMD codex exec test fixes"));
        assert!(rendered.contains("│ TAIL patched failing tests"));
        assert!(rendered.contains("LANES tail persisted logs"));
        assert!(rendered.contains("CONTROL inspect stop route"));
        assert!(rendered.contains("pty/02 :: waiting for review terminal"));
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
        assert!(rendered.contains("WORKSPACE"));
        assert!(rendered.contains("LSP / BUILD"));
        assert!(rendered.contains("RECENT EVENTS"));
        assert!(rendered.contains("PROVIDER HEALTH"));
        assert!(rendered.contains("side-2"));
        assert!(rendered.contains("files 128"));
        assert!(rendered.contains("diagnostics unavailable"));
        assert!(rendered.contains("auto-checks or /lsp diagnostics"));
        assert!(rendered.contains("L1 codex tty"));
        assert!(rendered.contains("L2 claude tty"));
        assert!(rendered.contains("pty/01 :: codex exec test fixes"));
        assert!(rendered.contains("pty/02 :: claude -p review diff"));
        assert!(rendered.contains("[waiting] write_file"));
        assert!(!rendered.contains("Type instruction"));

        let lines = rendered.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 36);
        for line in lines {
            assert_eq!(line.chars().count(), 80, "{line}");
        }
    }
}
