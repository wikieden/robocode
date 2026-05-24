use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

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

pub(super) fn pid_hint(lane: &TerminalLane) -> String {
    lane_pid(lane)
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "----".to_string())
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
    let mut parts = input.split_whitespace();
    if parts.next() != Some("/lane") {
        return false;
    }
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
        "codex" => match templated_agent_command("ROBOCODE_LANE_CODEX_TEMPLATE", &lane, state) {
            Some(command) => command,
            None => {
                lane.summary = queued_adapter_summary(&lane, "ROBOCODE_LANE_CODEX_TEMPLATE", state);
                return lane;
            }
        },
        "claude" => match templated_agent_command("ROBOCODE_LANE_CLAUDE_TEMPLATE", &lane, state) {
            Some(command) => command,
            None => {
                lane.summary =
                    queued_adapter_summary(&lane, "ROBOCODE_LANE_CLAUDE_TEMPLATE", state);
                return lane;
            }
        },
        _ => return lane,
    };
    start_background_lane(lane, state, &command)
}

fn templated_agent_command(env_key: &str, lane: &TerminalLane, state: &TuiState) -> Option<String> {
    let template = std::env::var(env_key).ok()?;
    let envelope_path = write_lane_envelope(lane, state).ok()?;
    let command = expand_agent_template(&template, &lane.title, Some(envelope_path.as_path()));
    (!command.trim().is_empty()).then_some(command)
}

fn queued_adapter_summary(lane: &TerminalLane, env_key: &str, state: &TuiState) -> String {
    match write_lane_envelope(lane, state) {
        Ok(path) => format!(
            "queued; envelope {}; set {env_key} to launch {}",
            path.display(),
            lane.tool
        ),
        Err(err) => format!("queued; failed to write envelope: {err}; set {env_key}"),
    }
}

fn expand_agent_template(template: &str, task: &str, envelope_path: Option<&Path>) -> String {
    let envelope = envelope_path
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    template
        .replace("{task}", task)
        .replace("{task:q}", &shell_quote_value(task))
        .replace("{envelope}", &envelope)
        .replace("{envelope:q}", &shell_quote_value(&envelope))
}

fn write_lane_envelope(lane: &TerminalLane, state: &TuiState) -> Result<PathBuf, String> {
    let path = lane_artifact_path(state, &lane.id, "envelope.md")?;
    let content = render_lane_envelope(lane, state);
    fs::write(&path, content).map_err(|err| err.to_string())?;
    Ok(path)
}

fn lane_artifact_path(state: &TuiState, lane_id: &str, extension: &str) -> Result<PathBuf, String> {
    let store = state
        .lane_store
        .as_deref()
        .ok_or_else(|| "no lane store available".to_string())?;
    let parent = store
        .parent()
        .ok_or_else(|| "lane store has no parent".to_string())?;
    let artifact_dir = parent.join("lanes");
    fs::create_dir_all(&artifact_dir).map_err(|err| err.to_string())?;
    Ok(artifact_dir.join(format!("{lane_id}.{extension}")))
}

fn render_lane_envelope(lane: &TerminalLane, state: &TuiState) -> String {
    format!(
        "# RoboCode Lane Task\n\nLane: {}\nTool: {}\nWorkspace: {}\nSession: {}\nProvider: {}\nModel: {}\n\n## Task\n{}\n\n## Handoff\n- summary\n- files changed\n- tests run\n- remaining risks\n- suggested next step\n\n## Constraints\n- Do not assume access to the full RoboCode transcript.\n- Keep changes scoped to the task.\n- Report commands run and verification evidence.\n",
        lane.id,
        lane.tool,
        state.workspace.display_root,
        state.session_id,
        state.provider,
        state.model,
        lane.title
    )
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
    let mut command = Command::new("sh");
    command
        .arg("-lc")
        .arg(shell)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_lane_process_group(&mut command);
    match command.spawn() {
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
    let envelope_path = evidence
        .as_ref()
        .filter(|evidence| evidence.envelope_path.exists())
        .map(|evidence| evidence.envelope_path.display().to_string())
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
    let envelope = evidence
        .as_ref()
        .map(|evidence| {
            if evidence.envelope_preview.is_empty() {
                "  <no envelope>".to_string()
            } else {
                evidence
                    .envelope_preview
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
            "Lane `{}`\nTool: {}\nStatus: {}\nTarget: {}\nProgress: {}%\nTask: {}\nLast output: {}\nLog: {log_path}\nDone: {done_path}\nEnvelope: {envelope_path}\nExit: {exit_code}\nTail:\n{tail}\nEnvelope preview:\n{envelope}",
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
    let stop_result = stop_lane_process(lane);
    lane.status = "stopped".to_string();
    lane.progress = lane.progress.min(99);
    lane.summary = stop_result;
    let lane_id = lane.id.clone();
    let lane_summary = lane.summary.clone();
    persist_lanes(state);
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!("Stopped terminal lane `{lane_id}`: {lane_summary}"),
    });
}

#[cfg(unix)]
fn configure_lane_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_lane_process_group(_command: &mut Command) {}

fn stop_lane_process(lane: &TerminalLane) -> String {
    if !matches!(lane.status.as_str(), "running" | "queued") {
        return "stopped by operator; no running process recorded".to_string();
    }
    let Some(pid) = lane_pid(lane) else {
        return "stopped by operator; no process id recorded".to_string();
    };
    match terminate_process_group(pid) {
        Ok(()) => format!("stopped by operator; sent SIGTERM to process group {pid}"),
        Err(err) => format!("stopped by operator; failed to signal process group {pid}: {err}"),
    }
}

fn lane_pid(lane: &TerminalLane) -> Option<u32> {
    lane.target.strip_prefix("pid ")?.parse::<u32>().ok()
}

#[cfg(unix)]
fn terminate_process_group(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(format!("-{pid}"))
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("kill exited with {status}"))
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: u32) -> Result<(), String> {
    Err("process-group termination is unsupported on this platform".to_string())
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
        fs,
        sync::{Mutex, MutexGuard, OnceLock},
        thread,
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
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            entries: Vec::new(),
        }
    }

    #[test]
    fn lane_command_adds_visible_lane_without_model_roundtrip() {
        let _env = ScopedEnv::unset("ROBOCODE_LANE_CODEX_TEMPLATE");
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
        let envelope = std::path::Path::new("/tmp/task envelope.md");
        let command = expand_agent_template(
            "codex exec {task:q} --prompt-file {envelope:q}",
            "fix 'quoted' task",
            Some(envelope),
        );

        assert_eq!(
            command,
            "codex exec 'fix '\\''quoted'\\'' task' --prompt-file '/tmp/task envelope.md'"
        );
    }

    #[test]
    fn codex_lane_writes_auditable_envelope_when_adapter_is_not_configured() {
        let _env = ScopedEnv::unset("ROBOCODE_LANE_CODEX_TEMPLATE");
        let root = temp_lane_root();
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane codex fix persistent state",
            &mut state
        ));

        let envelope = root.join(".robocode").join("lanes").join("L1.envelope.md");
        let content = fs::read_to_string(&envelope).expect("lane envelope");
        assert!(content.contains("# RoboCode Lane Task"));
        assert!(content.contains("Lane: L1"));
        assert!(content.contains("Tool: codex"));
        assert!(content.contains("fix persistent state"));
        assert!(state.lanes[0].summary.contains("L1.envelope.md"));

        assert!(handle_tui_command("/lane inspect L1", &mut state));
        let inspect = state.entries.last().expect("inspect entry");
        assert!(inspect.body.contains("Envelope:"));
        assert!(inspect.body.contains("# RoboCode Lane Task"));
        assert!(inspect.body.contains("fix persistent state"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn codex_template_receives_envelope_path_and_runs_against_it() {
        let _env = ScopedEnv::set("ROBOCODE_LANE_CODEX_TEMPLATE", "cat {envelope:q}");
        let root = temp_lane_root();
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane codex summarize adapter",
            &mut state
        ));

        assert_eq!(state.lanes[0].tool, "codex");
        assert_eq!(state.lanes[0].status, "running");

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
        assert!(
            fs::read_to_string(root.join(".robocode").join("lanes").join("L1.log"))
                .expect("lane log")
                .contains("summarize adapter")
        );

        state.lanes = lanes;
        assert!(handle_tui_command("/lane inspect L1", &mut state));
        let inspect = state.entries.last().expect("inspect entry");
        assert!(inspect.body.contains("Exit: 0"));
        assert!(inspect.body.contains("Envelope preview:"));
        assert!(inspect.body.contains("summarize adapter"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lane_command_reports_usage_for_missing_task() {
        let mut state = test_state();

        assert!(handle_tui_command("/lane codex", &mut state));

        assert!(state.lanes.is_empty());
        assert!(state.entries[0].body.contains("Usage: /lane codex"));
    }

    #[test]
    fn lane_command_does_not_capture_other_slash_commands() {
        let mut state = test_state();

        assert!(!handle_tui_command("/lanes", &mut state));

        assert!(state.entries.is_empty());
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
        assert!(state.lanes[0].summary.contains("stopped by operator"));
        assert!(state.entries[0].body.contains("Stopped terminal lane `L1`"));
    }

    #[test]
    fn lane_commands_persist_created_and_stopped_lanes() {
        let _env = ScopedEnv::unset("ROBOCODE_LANE_CODEX_TEMPLATE");
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
        assert!(lanes[0].summary.contains("stopped by operator"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn lane_stop_terminates_running_process_group() {
        let root = temp_lane_root();
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane run sleep 5; printf should-not-finish",
            &mut state
        ));
        assert_eq!(state.lanes[0].status, "running");
        let done_path = root.join(".robocode").join("lanes").join("L1.done");

        assert!(handle_tui_command("/lane stop L1", &mut state));
        thread::sleep(std::time::Duration::from_millis(250));
        refresh_lanes(&mut state);

        assert_eq!(state.lanes[0].status, "stopped");
        assert!(state.lanes[0].summary.contains("SIGTERM"));
        assert!(
            !done_path.exists(),
            "stopped lane should not write normal completion marker"
        );

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

    struct ScopedEnv {
        key: &'static str,
        previous: Option<String>,
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedEnv {
        fn set(key: &'static str, value: &str) -> Self {
            let guard = env_lock().lock().expect("env test lock");
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self {
                key,
                previous,
                _guard: guard,
            }
        }

        fn unset(key: &'static str) -> Self {
            let guard = env_lock().lock().expect("env test lock");
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::remove_var(key);
            }
            Self {
                key,
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            unsafe {
                if let Some(value) = &self.previous {
                    std::env::set_var(self.key, value);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }
}
