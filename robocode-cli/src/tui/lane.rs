use std::{
    fs,
    process::{Command, Stdio},
};

use crate::tui::state::{
    TerminalLane, TuiEntry, TuiState, lane_runtime_evidence, refresh_lane_runtime, save_lanes,
};

pub(super) fn status_badge(status: &str) -> &'static str {
    match status {
        "running" => "[in_prog]",
        "queued" => "[pending]",
        "completed" => "[done]",
        "failed" => "[failed]",
        _ => "[idle]",
    }
}

pub(super) fn terminal_label(tool: &str) -> &'static str {
    match tool {
        "codex" => "codex tty",
        "claude" => "claude tty",
        "shell" | "run" => "shell tty",
        _ => "agent tty",
    }
}

pub(super) fn pty_label(tool: &str) -> &'static str {
    match tool {
        "codex" => "pty/01",
        "claude" => "pty/02",
        "shell" | "run" => "pty/ops",
        _ => "pty/xx",
    }
}

pub(super) fn pid_hint(tool: &str) -> &'static str {
    match tool {
        "codex" => "4217",
        "claude" => "4380",
        "shell" | "run" => "4412",
        _ => "----",
    }
}

pub(super) fn command_hint(tool: &str, task: &str) -> String {
    match tool {
        "codex" => format!("codex exec {task}"),
        "claude" => format!("claude -p {task}"),
        "shell" | "run" => task.to_string(),
        _ => format!("{tool} {task}"),
    }
}

pub(super) fn handle_tui_command(input: &str, state: &mut TuiState) -> bool {
    if !input.starts_with("/lane") {
        return false;
    }
    let mut parts = input.split_whitespace();
    let _ = parts.next();
    match parts.next() {
        Some("close") => close_lane_focus(state),
        Some("inspect") => inspect_lane(parts.next(), state),
        Some("stop") => stop_lane(parts.next(), state),
        Some(_) => queue_lane(input, state),
        None => push_lane_usage(state),
    }
    true
}

fn close_lane_focus(state: &mut TuiState) {
    state.focused_lane = None;
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: "Closed lane detail focus.".to_string(),
    });
}

fn queue_lane(input: &str, state: &mut TuiState) {
    match TerminalLane::from_command(state.lanes.len() + 1, input) {
        Some(lane) => {
            let lane = maybe_start_lane_adapter(lane, state);
            let body = format!(
                "{} terminal lane `{}` using `{}` for `{}`.",
                if lane.status == "running" {
                    "Started"
                } else {
                    "Queued"
                },
                lane.id,
                lane.tool,
                lane.title
            );
            state.lanes.push(lane);
            persist_lanes(state);
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body,
            });
        }
        None => push_lane_usage(state),
    }
}

fn maybe_start_lane_adapter(mut lane: TerminalLane, state: &mut TuiState) -> TerminalLane {
    let command = match lane.tool.as_str() {
        "run" => lane.title.clone(),
        "codex" => match templated_agent_command("ROBOCODE_LANE_CODEX_TEMPLATE", &lane.title) {
            Some(command) => command,
            None => {
                lane.summary =
                    "queued; set ROBOCODE_LANE_CODEX_TEMPLATE to launch Codex".to_string();
                return lane;
            }
        },
        "claude" => match templated_agent_command("ROBOCODE_LANE_CLAUDE_TEMPLATE", &lane.title) {
            Some(command) => command,
            None => {
                lane.summary =
                    "queued; set ROBOCODE_LANE_CLAUDE_TEMPLATE to launch Claude".to_string();
                return lane;
            }
        },
        _ => return lane,
    };
    start_background_lane(lane, state, &command)
}

fn templated_agent_command(env_key: &str, task: &str) -> Option<String> {
    let template = std::env::var(env_key).ok()?;
    let command = expand_agent_template(&template, task);
    (!command.trim().is_empty()).then_some(command)
}

fn expand_agent_template(template: &str, task: &str) -> String {
    template
        .replace("{task}", task)
        .replace("{task:q}", &shell_quote_value(task))
}

fn start_background_lane(
    mut lane: TerminalLane,
    state: &mut TuiState,
    command: &str,
) -> TerminalLane {
    let Some(store) = state.lane_store.as_deref() else {
        lane.summary = "queued; no lane store available".to_string();
        return lane;
    };
    let Some(parent) = store.parent() else {
        lane.summary = "queued; lane store has no parent".to_string();
        return lane;
    };
    let artifact_dir = parent.join("lanes");
    if let Err(err) = fs::create_dir_all(&artifact_dir) {
        lane.status = "failed".to_string();
        lane.summary = format!("failed to create lane artifacts: {err}");
        return lane;
    }
    let log_path = artifact_dir.join(format!("{}.log", lane.id));
    let done_path = artifact_dir.join(format!("{}.done", lane.id));
    let shell = format!(
        "({command}) > {} 2>&1; status=$?; printf '%s\\n' \"$status\" > {}",
        shell_quote_path(&log_path),
        shell_quote_path(&done_path)
    );
    match Command::new("sh")
        .arg("-lc")
        .arg(shell)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            lane.status = "running".to_string();
            lane.progress = 10;
            lane.target = format!("pid {}", child.id());
            lane.summary = format!("running {}; log {}", lane.tool, log_path.display());
        }
        Err(err) => {
            lane.status = "failed".to_string();
            lane.progress = 100;
            lane.summary = format!("failed to start shell command: {err}");
        }
    }
    lane
}

fn shell_quote_path(path: &std::path::Path) -> String {
    shell_quote_value(&path.to_string_lossy())
}

fn shell_quote_value(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn inspect_lane(id: Option<&str>, state: &mut TuiState) {
    let Some(id) = id else {
        push_lane_usage(state);
        return;
    };
    refresh_lanes(state);
    let Some(lane) = state
        .lanes
        .iter()
        .find(|lane| lane.id.eq_ignore_ascii_case(id))
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("No terminal lane `{id}` found."),
        });
        return;
    };
    state.focused_lane = Some(lane.id.clone());
    let evidence = state
        .lane_store
        .as_deref()
        .and_then(|path| lane_runtime_evidence(path, &lane.id));
    let log_path = evidence
        .as_ref()
        .map(|evidence| evidence.log_path.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let done_path = evidence
        .as_ref()
        .map(|evidence| evidence.done_path.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let exit_code = evidence
        .as_ref()
        .and_then(|evidence| evidence.exit_code.as_deref())
        .unwrap_or("<pending>");
    let tail = evidence
        .as_ref()
        .map(|evidence| {
            if evidence.log_tail.is_empty() {
                "  <no log output>".to_string()
            } else {
                evidence
                    .log_tail
                    .iter()
                    .map(|line| format!("  {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        })
        .unwrap_or_else(|| "  <no lane store>".to_string());
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!(
            "Lane `{}`\nTool: {}\nStatus: {}\nTarget: {}\nProgress: {}%\nTask: {}\nLast output: {}\nLog: {log_path}\nDone: {done_path}\nExit: {exit_code}\nTail:\n{tail}",
            lane.id, lane.tool, lane.status, lane.target, lane.progress, lane.title, lane.summary
        ),
    });
}

fn stop_lane(id: Option<&str>, state: &mut TuiState) {
    let Some(id) = id else {
        push_lane_usage(state);
        return;
    };
    let Some(lane) = state
        .lanes
        .iter_mut()
        .find(|lane| lane.id.eq_ignore_ascii_case(id))
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("No terminal lane `{id}` found."),
        });
        return;
    };
    lane.status = "stopped".to_string();
    lane.progress = lane.progress.min(99);
    lane.summary = "stopped by operator".to_string();
    let lane_id = lane.id.clone();
    persist_lanes(state);
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!("Stopped terminal lane `{lane_id}`."),
    });
}

fn persist_lanes(state: &mut TuiState) {
    let Some(path) = state.lane_store.as_deref() else {
        return;
    };
    if let Err(err) = save_lanes(path, &state.lanes) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to persist terminal lanes: {err}"),
        });
    }
}

pub(super) fn refresh_lanes(state: &mut TuiState) {
    let Some(path) = state.lane_store.clone() else {
        return;
    };
    refresh_lane_runtime(&path, &mut state.lanes);
    persist_lanes(state);
}

fn push_lane_usage(state: &mut TuiState) {
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: "Usage: /lane codex <task> | /lane claude <task> | /lane run <command> | /lane inspect <id> | /lane stop <id> | /lane close"
            .to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{ProviderStatus, WorkspaceSnapshot, load_lanes, refresh_lane_runtime};
    use std::{
        fs, thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn test_state() -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            workspace: WorkspaceSnapshot::fixture(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            entries: Vec::new(),
        }
    }

    #[test]
    fn lane_command_adds_visible_lane_without_model_roundtrip() {
        let mut state = test_state();

        assert!(handle_tui_command(
            "/lane codex fix failing tests",
            &mut state
        ));

        assert_eq!(state.lanes.len(), 1);
        assert_eq!(state.lanes[0].id, "L1");
        assert_eq!(state.lanes[0].tool, "codex");
        assert_eq!(state.lanes[0].status, "queued");
        assert!(
            state.lanes[0]
                .summary
                .contains("ROBOCODE_LANE_CODEX_TEMPLATE")
        );
        assert!(state.entries[0].body.contains("Queued terminal lane"));
    }

    #[test]
    fn agent_template_quotes_task_placeholder() {
        let command = expand_agent_template("codex exec {task:q}", "fix 'quoted' task");

        assert_eq!(command, "codex exec 'fix '\\''quoted'\\'' task'");
    }

    #[test]
    fn lane_command_reports_usage_for_missing_task() {
        let mut state = test_state();

        assert!(handle_tui_command("/lane codex", &mut state));

        assert!(state.lanes.is_empty());
        assert!(state.entries[0].body.contains("Usage: /lane codex"));
    }

    #[test]
    fn lane_inspect_reports_existing_lane() {
        let mut state = test_state();
        state.lanes = TerminalLane::preview_lanes();

        assert!(handle_tui_command("/lane inspect L1", &mut state));

        assert_eq!(state.focused_lane.as_deref(), Some("L1"));
        assert!(state.entries[0].body.contains("Lane `L1`"));
        assert!(state.entries[0].body.contains("Tool: codex"));
        assert!(state.entries[0].body.contains("Progress: 64%"));
        assert!(state.entries[0].body.contains("Exit: <pending>"));
        assert!(state.entries[0].body.contains("Tail:\n  <no lane store>"));
        assert!(
            state.entries[0]
                .body
                .contains("patched failing tests; rerunning cargo")
        );
    }

    #[test]
    fn lane_close_clears_focused_lane() {
        let mut state = test_state();
        state.focused_lane = Some("L1".to_string());

        assert!(handle_tui_command("/lane close", &mut state));

        assert_eq!(state.focused_lane, None);
        assert!(state.entries[0].body.contains("Closed lane detail focus"));
    }

    #[test]
    fn lane_stop_marks_lane_stopped() {
        let mut state = test_state();
        state.lanes = TerminalLane::preview_lanes();

        assert!(handle_tui_command("/lane stop l1", &mut state));

        assert_eq!(state.lanes[0].status, "stopped");
        assert_eq!(state.lanes[0].summary, "stopped by operator");
        assert!(state.entries[0].body.contains("Stopped terminal lane `L1`"));
    }

    #[test]
    fn lane_commands_persist_created_and_stopped_lanes() {
        let root = temp_lane_root();
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane codex fix persistent state",
            &mut state
        ));
        assert!(handle_tui_command("/lane stop L1", &mut state));

        let lanes = load_lanes(&store);
        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].tool, "codex");
        assert_eq!(lanes[0].title, "fix persistent state");
        assert_eq!(lanes[0].status, "stopped");
        assert_eq!(lanes[0].summary, "stopped by operator");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lane_run_starts_shell_command_and_refreshes_output() {
        let root = temp_lane_root();
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command("/lane run printf lane-ok", &mut state));

        assert_eq!(state.lanes[0].tool, "run");
        assert_eq!(state.lanes[0].status, "running");
        assert!(state.entries[0].body.contains("Started terminal lane"));

        let mut lanes = Vec::new();
        for _ in 0..40 {
            lanes = load_lanes(&store);
            refresh_lane_runtime(&store, &mut lanes);
            if lanes.first().is_some_and(|lane| lane.status == "completed") {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(25));
        }

        assert_eq!(lanes[0].status, "completed");
        assert_eq!(lanes[0].progress, 100);
        assert!(lanes[0].summary.contains("lane-ok"));

        state.lanes = lanes;
        assert!(handle_tui_command("/lane inspect L1", &mut state));
        let inspect = state.entries.last().expect("inspect entry");
        assert!(inspect.body.contains("Exit: 0"));
        assert!(inspect.body.contains("Log:"));
        assert!(inspect.body.contains("Done:"));
        assert!(inspect.body.contains("lane-ok"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lane_run_refreshes_failed_exit_code_and_inspect_tail() {
        let root = temp_lane_root();
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane run printf fail-line && exit 7",
            &mut state
        ));

        let mut lanes = Vec::new();
        for _ in 0..40 {
            lanes = load_lanes(&store);
            refresh_lane_runtime(&store, &mut lanes);
            if lanes.first().is_some_and(|lane| lane.status == "failed") {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(25));
        }

        assert_eq!(lanes[0].status, "failed");
        assert_eq!(lanes[0].progress, 100);
        assert!(lanes[0].summary.contains("fail-line"));
        assert!(lanes[0].summary.contains("exit 7"));

        state.lanes = lanes;
        assert!(handle_tui_command("/lane inspect L1", &mut state));
        let inspect = state.entries.last().expect("inspect entry");
        assert!(inspect.body.contains("Exit: 7"));
        assert!(inspect.body.contains("fail-line"));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_lane_root() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("robocode-lane-test-{nanos}"))
    }
}
