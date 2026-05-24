use std::{
    env, fs,
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
        "accepted" => "[accepted]",
        "revise" => "[revise]",
        "discarded" => "[discard]",
        "archived" => "[archive]",
        "attached" => "[attach]",
        "detached" => "[detach]",
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
        Some("accept") => decide_lane("accepted", parts.next(), parts.collect(), state),
        Some("revise") => decide_lane("revise", parts.next(), parts.collect(), state),
        Some("discard") => decide_lane("discarded", parts.next(), parts.collect(), state),
        Some("cleanup") => cleanup_lane(parts.next(), parts.collect(), state),
        Some("attach") => attach_lane(parts.next(), state),
        Some("detach") => detach_lane(parts.next(), state),
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
        "codex" => {
            match templated_agent_command("ROBOCODE_LANE_CODEX_TEMPLATE", &mut lane, state) {
                Ok(Some(command)) => command,
                Ok(None) => {
                    lane.summary =
                        queued_adapter_summary(&lane, "ROBOCODE_LANE_CODEX_TEMPLATE", state);
                    return lane;
                }
                Err(err) => return failed_lane(lane, err),
            }
        }
        "claude" => {
            match templated_agent_command("ROBOCODE_LANE_CLAUDE_TEMPLATE", &mut lane, state) {
                Ok(Some(command)) => command,
                Ok(None) => {
                    lane.summary =
                        queued_adapter_summary(&lane, "ROBOCODE_LANE_CLAUDE_TEMPLATE", state);
                    return lane;
                }
                Err(err) => return failed_lane(lane, err),
            }
        }
        _ => return lane,
    };
    start_background_lane(lane, state, &command)
}

fn templated_agent_command(
    env_key: &str,
    lane: &mut TerminalLane,
    state: &TuiState,
) -> Result<Option<String>, String> {
    let Ok(template) = std::env::var(env_key) else {
        return Ok(None);
    };
    prepare_lane_worktree(lane, state)?;
    let envelope_path = write_lane_envelope(lane, state)
        .map_err(|err| format!("failed to write lane envelope: {err}"))?;
    let cwd = lane_workspace(lane, state);
    let command = expand_agent_template(&template, &lane.title, Some(envelope_path.as_path()), cwd);
    Ok((!command.trim().is_empty()).then_some(command))
}

fn failed_lane(mut lane: TerminalLane, summary: String) -> TerminalLane {
    lane.status = "failed".to_string();
    lane.progress = 100;
    lane.summary = summary;
    lane
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

fn expand_agent_template(
    template: &str,
    task: &str,
    envelope_path: Option<&Path>,
    cwd: &Path,
) -> String {
    let envelope = envelope_path
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let cwd = cwd.to_string_lossy().to_string();
    template
        .replace("{task}", task)
        .replace("{task:q}", &shell_quote_value(task))
        .replace("{envelope}", &envelope)
        .replace("{envelope:q}", &shell_quote_value(&envelope))
        .replace("{cwd}", &cwd)
        .replace("{cwd:q}", &shell_quote_value(&cwd))
        .replace("{worktree}", &cwd)
        .replace("{worktree:q}", &shell_quote_value(&cwd))
}

fn expand_attach_template(
    template: &str,
    lane: &TerminalLane,
    state: &TuiState,
    attach_log: &Path,
) -> String {
    let cwd = lane_workspace(lane, state).to_string_lossy().to_string();
    let lane_id = lane.id.as_str();
    let task = lane.title.as_str();
    let tool = lane.tool.as_str();
    let log = attach_log.to_string_lossy().to_string();
    template
        .replace("{lane:q}", &shell_quote_value(lane_id))
        .replace("{task:q}", &shell_quote_value(task))
        .replace("{tool:q}", &shell_quote_value(tool))
        .replace("{cwd:q}", &shell_quote_value(&cwd))
        .replace("{worktree:q}", &shell_quote_value(&cwd))
        .replace("{log:q}", &shell_quote_value(&log))
        .replace("{lane}", lane_id)
        .replace("{task}", task)
        .replace("{tool}", tool)
        .replace("{cwd}", &cwd)
        .replace("{worktree}", &cwd)
        .replace("{log}", &log)
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
    let workspace = lane_workspace(lane, state).to_string_lossy().to_string();
    let mutation_scope = if lane.worktree.is_some() {
        "isolated per-lane worktree"
    } else {
        "current workspace"
    };
    format!(
        "# RoboCode Lane Task\n\nLane: {}\nTool: {}\nWorkspace: {workspace}\nMutation scope: {mutation_scope}\nSession: {}\nProvider: {}\nModel: {}\n\n## Task\n{}\n\n## Handoff\n- summary\n- files changed\n- tests run\n- remaining risks\n- suggested next step\n\n## Constraints\n- Do not assume access to the full RoboCode transcript.\n- Keep changes scoped to the task.\n- Report commands run and verification evidence.\n",
        lane.id, lane.tool, state.session_id, state.provider, state.model, lane.title
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
        .current_dir(lane_workspace(&lane, state))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_lane_process_group(&mut command);
    match command.spawn() {
        Ok(child) => {
            lane.status = "running".to_string();
            lane.progress = 10;
            lane.target = format!("pid {}", child.id());
            lane.summary = format!(
                "running {}; cwd {}; log {}",
                lane.tool,
                lane_workspace(&lane, state).display(),
                log_path.display()
            );
        }
        Err(err) => {
            lane.status = "failed".to_string();
            lane.progress = 100;
            lane.summary = format!("failed to start shell command: {err}");
        }
    }
    lane
}

fn prepare_lane_worktree(lane: &mut TerminalLane, state: &TuiState) -> Result<(), String> {
    if lane.worktree.is_some() {
        return Ok(());
    }
    let Some(store) = state.lane_store.as_deref() else {
        return Err(
            "failed to prepare isolated lane worktree: no lane store available".to_string(),
        );
    };
    let Some(parent) = store.parent() else {
        return Err(
            "failed to prepare isolated lane worktree: lane store has no parent".to_string(),
        );
    };
    let worktree_dir = parent.join("worktrees").join(format!(
        "{}-{}",
        sanitize_ref(&state.session_id),
        lane.id.to_ascii_lowercase()
    ));
    if worktree_dir.exists() {
        lane.worktree = Some(worktree_dir);
        return Ok(());
    }
    fs::create_dir_all(
        worktree_dir
            .parent()
            .ok_or_else(|| "worktree path has no parent".to_string())?,
    )
    .map_err(|err| format!("failed to create worktree parent: {err}"))?;
    let branch = format!(
        "codex/lane-{}-{}",
        sanitize_ref(&state.session_id),
        lane.id.to_ascii_lowercase()
    );
    let output = Command::new("git")
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(&branch)
        .arg(&worktree_dir)
        .arg("HEAD")
        .current_dir(&state.workspace.root)
        .output()
        .map_err(|err| format!("failed to run git worktree add: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "failed to create isolated worktree `{}` on `{branch}`: {}",
            worktree_dir.display(),
            if stderr.is_empty() {
                output.status.to_string()
            } else {
                stderr
            }
        ));
    }
    lane.worktree = Some(worktree_dir);
    lane.summary = format!("isolated worktree {branch}");
    Ok(())
}

fn sanitize_ref(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "session".to_string()
    } else {
        sanitized
    }
}

fn lane_workspace<'a>(lane: &'a TerminalLane, state: &'a TuiState) -> &'a Path {
    lane.worktree
        .as_deref()
        .unwrap_or(state.workspace.root.as_path())
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
    let changed_files = changed_file_rows_for_lane(lane, state);
    let verification = verification_rows(evidence.as_ref());
    let decision = decision_rows(state, &lane.id);
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
            "Lane `{}`\nTool: {}\nStatus: {}\nTarget: {}\nWorktree: {}\nProgress: {}%\nTask: {}\nLast output: {}\nLog: {log_path}\nDone: {done_path}\nEnvelope: {envelope_path}\nExit: {exit_code}\nChanged files:\n{changed_files}\nVerification:\n{verification}\nDecision:\n{decision}\nTail:\n{tail}\nEnvelope preview:\n{envelope}",
            lane.id,
            lane.tool,
            lane.status,
            lane.target,
            lane.worktree
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            lane.progress,
            lane.title,
            lane.summary
        ),
    });
}

fn changed_file_rows_for_lane(lane: &TerminalLane, state: &TuiState) -> String {
    changed_file_rows(lane_workspace(lane, state))
}

fn changed_file_rows(root: &Path) -> String {
    match workspace_changed_files(root) {
        Ok(files) if files.is_empty() => "  <none>".to_string(),
        Ok(files) => files
            .into_iter()
            .take(8)
            .map(|file| format!("  {file}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Err(err) => format!("  unavailable: {err}"),
    }
}

fn workspace_changed_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("status")
        .arg("--short")
        .current_dir(root)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(ToString::to_string)
        .collect())
}

fn verification_rows(evidence: Option<&crate::tui::state::LaneRuntimeEvidence>) -> String {
    let Some(evidence) = evidence else {
        return "  <no lane store>".to_string();
    };
    let exit = evidence.exit_code.as_deref().unwrap_or("<pending>");
    let tail = evidence
        .log_tail
        .last()
        .map(String::as_str)
        .unwrap_or("<no log output>");
    format!(
        "  exit: {exit}\n  log: {}\n  tail: {tail}",
        evidence.log_path.display()
    )
}

fn decision_rows(state: &TuiState, lane_id: &str) -> String {
    let Some(path) = decision_artifact_path(state, lane_id) else {
        return "  <none>".to_string();
    };
    let lines = fs::read_to_string(path)
        .ok()
        .map(|content| {
            content
                .lines()
                .filter(|line| line.starts_with("Decision:") || line.starts_with("Summary:"))
                .map(|line| format!("  {line}"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        "  <none>".to_string()
    } else {
        lines.join("\n")
    }
}

fn decision_artifact_path(state: &TuiState, lane_id: &str) -> Option<PathBuf> {
    let store = state.lane_store.as_deref()?;
    Some(
        store
            .parent()?
            .join("lanes")
            .join(format!("{lane_id}.decision.md")),
    )
}

fn decide_lane(action: &str, id: Option<&str>, feedback: Vec<&str>, state: &mut TuiState) {
    let Some(id) = id else {
        push_lane_usage(state);
        return;
    };
    refresh_lanes(state);
    let Some(index) = state
        .lanes
        .iter()
        .position(|lane| lane.id.eq_ignore_ascii_case(id))
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("No terminal lane `{id}` found."),
        });
        return;
    };
    let lane = state.lanes[index].clone();
    let summary = if feedback.is_empty() {
        format!("operator marked lane {}", action)
    } else {
        feedback.join(" ")
    };
    let content = render_lane_decision(action, &summary, &lane, state);
    let path = match lane_artifact_path(state, &lane.id, "decision.md") {
        Ok(path) => path,
        Err(err) => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Failed to prepare lane decision artifact: {err}"),
            });
            return;
        }
    };
    if let Err(err) = fs::write(&path, content) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to write lane decision: {err}"),
        });
        return;
    }
    state.lanes[index].status = action.to_string();
    state.lanes[index].summary = format!("decision: {summary}");
    state.lanes[index].progress = 100;
    persist_lanes(state);
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!(
            "Recorded `{action}` decision for lane `{}`.\nDecision: {}",
            lane.id,
            path.display()
        ),
    });
}

fn render_lane_decision(
    action: &str,
    summary: &str,
    lane: &TerminalLane,
    state: &TuiState,
) -> String {
    let changed_files = changed_file_rows_for_lane(lane, state);
    let evidence = state
        .lane_store
        .as_deref()
        .and_then(|path| lane_runtime_evidence(path, &lane.id));
    let verification = verification_rows(evidence.as_ref());
    format!(
        "# RoboCode Lane Decision\n\nLane: {}\nTool: {}\nDecision: {action}\nSummary: {summary}\n\n## Task\n{}\n\n## Changed files\n{changed_files}\n\n## Verification\n{verification}\n",
        lane.id, lane.tool, lane.title
    )
}

fn attach_lane(id: Option<&str>, state: &mut TuiState) {
    let Some(id) = id else {
        push_lane_usage(state);
        return;
    };
    refresh_lanes(state);
    let Some(index) = state
        .lanes
        .iter()
        .position(|lane| lane.id.eq_ignore_ascii_case(id))
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("No terminal lane `{id}` found."),
        });
        return;
    };
    let lane = state.lanes[index].clone();
    let attach_path = match lane_artifact_path(state, &lane.id, "attach.md") {
        Ok(path) => path,
        Err(err) => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Failed to prepare lane attach artifact: {err}"),
            });
            return;
        }
    };
    let attach_log = match lane_artifact_path(state, &lane.id, "attach.log") {
        Ok(path) => path,
        Err(err) => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Failed to prepare lane attach log: {err}"),
            });
            return;
        }
    };
    let command = match lane_attach_command(&lane, state, &attach_log) {
        Ok(command) => command,
        Err(err) => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Cannot attach lane `{}`: {err}", lane.id),
            });
            return;
        }
    };
    if let Err(err) = fs::write(
        &attach_path,
        render_lane_attach(&lane, state, &command, &attach_log),
    ) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to write lane attach artifact: {err}"),
        });
        return;
    }
    match spawn_shell_command(&command) {
        Ok(pid) => {
            state.lanes[index].status = "attached".to_string();
            state.lanes[index].target = format!("attach pid {pid}");
            state.lanes[index].summary = format!(
                "attached interactive terminal; artifact {}",
                attach_path.display()
            );
            state.focused_lane = Some(lane.id.clone());
            persist_lanes(state);
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!(
                    "Attached lane `{}` as pid {pid}.\nDetach with `/lane detach {}`; logs and lane artifacts remain in `.robocode/lanes/`.",
                    lane.id, lane.id
                ),
            });
        }
        Err(err) => state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to attach lane `{}`: {err}", lane.id),
        }),
    }
}

fn detach_lane(id: Option<&str>, state: &mut TuiState) {
    let Some(id) = id else {
        push_lane_usage(state);
        return;
    };
    let Some(index) = state
        .lanes
        .iter()
        .position(|lane| lane.id.eq_ignore_ascii_case(id))
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("No terminal lane `{id}` found."),
        });
        return;
    };
    let lane_id = state.lanes[index].id.clone();
    state.lanes[index].status = "detached".to_string();
    state.lanes[index].summary =
        "detached from interactive terminal; external process not killed".to_string();
    state.focused_lane = None;
    persist_lanes(state);
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!("Detached lane `{lane_id}` without killing its terminal process."),
    });
}

fn lane_attach_command(
    lane: &TerminalLane,
    state: &TuiState,
    attach_log: &Path,
) -> Result<String, String> {
    if let Ok(template) = env::var("ROBOCODE_LANE_ATTACH_TEMPLATE") {
        let command = expand_attach_template(&template, lane, state, attach_log);
        return (!command.trim().is_empty())
            .then_some(command)
            .ok_or_else(|| {
                "ROBOCODE_LANE_ATTACH_TEMPLATE expanded to an empty command".to_string()
            });
    }
    platform_lane_attach_command(lane, state, attach_log)
}

#[cfg(target_os = "macos")]
fn platform_lane_attach_command(
    lane: &TerminalLane,
    state: &TuiState,
    attach_log: &Path,
) -> Result<String, String> {
    let cwd = lane_workspace(lane, state).to_string_lossy().to_string();
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let terminal_script = format!(
        "cd {} && printf '%s\\n' {} >> {} && exec {} -l",
        shell_quote_value(&cwd),
        shell_quote_value(&format!(
            "RoboCode attached lane {}: {}",
            lane.id, lane.title
        )),
        shell_quote_path(attach_log),
        shell_quote_value(&shell)
    );
    Ok(format!(
        "osascript -e {}",
        shell_quote_value(&format!(
            "tell application \"Terminal\" to do script {}",
            applescript_string(&terminal_script)
        ))
    ))
}

#[cfg(not(target_os = "macos"))]
fn platform_lane_attach_command(
    _lane: &TerminalLane,
    _state: &TuiState,
    _attach_log: &Path,
) -> Result<String, String> {
    Err(
        "set ROBOCODE_LANE_ATTACH_TEMPLATE to open a terminal for this platform, e.g. `tmux new-session -A -s robocode-{lane:q} -c {cwd:q}`"
            .to_string(),
    )
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn render_lane_attach(
    lane: &TerminalLane,
    state: &TuiState,
    command: &str,
    attach_log: &Path,
) -> String {
    format!(
        "# RoboCode Lane Attach\n\nLane: {}\nTool: {}\nStatus before attach: {}\nWorkspace: {}\nAttach log: {}\n\n## Task\n{}\n\n## Command\n{}\n\n## Detach\nUse `/lane detach {}` to return RoboCode tracking to detached state without killing the external terminal process.\n",
        lane.id,
        lane.tool,
        lane.status,
        lane_workspace(lane, state).display(),
        attach_log.display(),
        lane.title,
        command,
        lane.id
    )
}

fn cleanup_lane(id: Option<&str>, args: Vec<&str>, state: &mut TuiState) {
    let Some(id) = id else {
        push_lane_usage(state);
        return;
    };
    refresh_lanes(state);
    let Some(index) = state
        .lanes
        .iter()
        .position(|lane| lane.id.eq_ignore_ascii_case(id))
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("No terminal lane `{id}` found."),
        });
        return;
    };
    let lane = state.lanes[index].clone();
    if matches!(lane.status.as_str(), "running" | "queued") {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!(
                "Lane `{}` is {}; stop or finish it before cleanup.",
                lane.id, lane.status
            ),
        });
        return;
    }
    let Some(worktree) = lane.worktree.clone() else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Lane `{}` has no isolated worktree to clean.", lane.id),
        });
        return;
    };
    let force = args.iter().any(|arg| *arg == "--force" || *arg == "-f");
    let changed_files = workspace_changed_files(&worktree)
        .unwrap_or_else(|err| vec![format!("unavailable: {err}")]);
    if !changed_files.is_empty() && !force {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!(
                "Refused to clean lane `{}` because its worktree has changes.\nRun `/lane inspect {}` first, then `/lane cleanup {} --force` if you want to delete them.",
                lane.id, lane.id, lane.id
            ),
        });
        return;
    }
    let cleanup_content = render_lane_cleanup(&lane, &worktree, &changed_files, force);
    let cleanup_path = match lane_artifact_path(state, &lane.id, "cleanup.md") {
        Ok(path) => path,
        Err(err) => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Failed to prepare cleanup artifact: {err}"),
            });
            return;
        }
    };
    if let Err(err) = fs::write(&cleanup_path, cleanup_content) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to write cleanup artifact: {err}"),
        });
        return;
    }
    match remove_lane_worktree(&state.workspace.root, &worktree, force) {
        Ok(()) => {
            state.lanes[index].worktree = None;
            state.lanes[index].status = "archived".to_string();
            state.lanes[index].summary = format!(
                "cleaned worktree {}; cleanup {}",
                worktree.display(),
                cleanup_path.display()
            );
            persist_lanes(state);
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!(
                    "Cleaned lane `{}` worktree.\nCleanup: {}",
                    lane.id,
                    cleanup_path.display()
                ),
            });
        }
        Err(err) => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Failed to clean lane `{}` worktree: {err}", lane.id),
            });
        }
    }
}

fn render_lane_cleanup(
    lane: &TerminalLane,
    worktree: &Path,
    changed_files: &[String],
    force: bool,
) -> String {
    let changed_files = if changed_files.is_empty() {
        "  <none>".to_string()
    } else {
        changed_files
            .iter()
            .map(|file| format!("  {file}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "# RoboCode Lane Cleanup\n\nLane: {}\nTool: {}\nStatus before cleanup: {}\nWorktree: {}\nForced: {force}\n\n## Changed files before cleanup\n{changed_files}\n",
        lane.id,
        lane.tool,
        lane.status,
        worktree.display()
    )
}

fn remove_lane_worktree(root: &Path, worktree: &Path, force: bool) -> Result<(), String> {
    if !worktree.exists() {
        return Ok(());
    }
    let mut command = Command::new("git");
    command.arg("worktree").arg("remove");
    if force {
        command.arg("--force");
    }
    let output = command
        .arg(worktree)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git worktree remove: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            output.status.to_string()
        } else {
            stderr
        })
    }
}

fn spawn_shell_command(command: &str) -> Result<u32, String> {
    let mut shell = platform_shell_command(command);
    shell
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = shell.spawn().map_err(|err| err.to_string())?;
    Ok(child.id())
}

#[cfg(windows)]
fn platform_shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

#[cfg(not(windows))]
fn platform_shell_command(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-lc").arg(command);
    shell
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
        body: "Usage: /lane codex <task> | /lane claude <task> | /lane run <command> | /lane inspect <id> | /lane stop <id> | /lane attach <id> | /lane detach <id> | /lane accept <id> [note] | /lane revise <id> [note] | /lane discard <id> [note] | /lane cleanup <id> [--force] | /lane close"
            .to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{ProviderStatus, WorkspaceSnapshot, load_lanes, refresh_lane_runtime};
    use std::{
        fs,
        sync::{
            Mutex, MutexGuard, OnceLock,
            atomic::{AtomicU64, Ordering},
        },
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
            tasks: Vec::new(),
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
            "codex exec {task:q} --prompt-file {envelope:q} --cwd {cwd:q}",
            "fix 'quoted' task",
            Some(envelope),
            Path::new("/tmp/lane cwd"),
        );

        assert_eq!(
            command,
            "codex exec 'fix '\\''quoted'\\'' task' --prompt-file '/tmp/task envelope.md' --cwd '/tmp/lane cwd'"
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
        init_git_repo(&root);
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.workspace.root = root.clone();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane codex summarize adapter",
            &mut state
        ));

        assert_eq!(state.lanes[0].tool, "codex");
        assert_eq!(state.lanes[0].status, "running");
        assert!(state.lanes[0].worktree.is_some());

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
        assert!(inspect.body.contains("Worktree:"));
        assert!(inspect.body.contains("Envelope preview:"));
        assert!(inspect.body.contains("summarize adapter"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn codex_template_runs_inside_isolated_worktree() {
        let _env = ScopedEnv::set(
            "ROBOCODE_LANE_CODEX_TEMPLATE",
            "printf isolated > isolated.txt; printf '%s' {worktree:q}",
        );
        let root = temp_lane_root();
        init_git_repo(&root);
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.workspace.root = root.clone();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane codex create isolated artifact",
            &mut state
        ));

        let mut lanes = Vec::new();
        for _ in 0..40 {
            lanes = load_lanes(&store);
            refresh_lane_runtime(&store, &mut lanes);
            if lanes.first().is_some_and(|lane| lane.status == "completed") {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(25));
        }

        let lane = lanes.first().expect("lane");
        let worktree = lane.worktree.as_ref().expect("lane worktree").clone();
        assert_eq!(lane.status, "completed");
        assert!(worktree.join("isolated.txt").exists());
        assert!(!root.join("isolated.txt").exists());

        state.lanes = lanes;
        assert!(handle_tui_command("/lane inspect L1", &mut state));
        let inspect = state.entries.last().expect("inspect entry");
        assert!(inspect.body.contains(&worktree.display().to_string()));
        assert!(inspect.body.contains("?? isolated.txt"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lane_cleanup_requires_force_for_dirty_worktree_and_preserves_artifacts() {
        let _env = ScopedEnv::set("ROBOCODE_LANE_CODEX_TEMPLATE", "printf dirty > dirty.txt");
        let root = temp_lane_root();
        init_git_repo(&root);
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.workspace.root = root.clone();
        state.lane_store = Some(store.clone());

        assert!(handle_tui_command(
            "/lane codex create dirty file",
            &mut state
        ));

        let mut lanes = Vec::new();
        for _ in 0..40 {
            lanes = load_lanes(&store);
            refresh_lane_runtime(&store, &mut lanes);
            if lanes.first().is_some_and(|lane| lane.status == "completed") {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(25));
        }
        state.lanes = lanes;
        let worktree = state.lanes[0]
            .worktree
            .as_ref()
            .expect("lane worktree")
            .clone();

        assert!(handle_tui_command(
            "/lane discard L1 not needed",
            &mut state
        ));
        assert!(worktree.exists(), "discard must preserve worktree changes");

        assert!(handle_tui_command("/lane cleanup L1", &mut state));
        assert!(
            worktree.exists(),
            "plain cleanup must preserve dirty worktree"
        );
        assert!(
            state
                .entries
                .last()
                .expect("cleanup refusal")
                .body
                .contains("Refused to clean")
        );

        assert!(handle_tui_command("/lane cleanup L1 --force", &mut state));

        assert!(!worktree.exists());
        assert_eq!(state.lanes[0].status, "archived");
        let cleanup = fs::read_to_string(root.join(".robocode/lanes/L1.cleanup.md"))
            .expect("cleanup artifact");
        assert!(cleanup.contains("Forced: true"));
        assert!(cleanup.contains("?? dirty.txt"));
        assert!(root.join(".robocode/lanes/L1.decision.md").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lane_attach_uses_template_and_detach_preserves_process() {
        let _env = ScopedEnv::set(
            "ROBOCODE_LANE_ATTACH_TEMPLATE",
            "printf attached-{lane} > {log:q}",
        );
        let root = temp_lane_root();
        fs::create_dir_all(&root).expect("temp root");
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.workspace.root = root.clone();
        state.lane_store = Some(store);
        state.lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "codex".to_string(),
            title: "inspect interactively".to_string(),
            status: "completed".to_string(),
            target: "main".to_string(),
            progress: 100,
            summary: "completed successfully".to_string(),
            worktree: None,
        }];

        assert!(handle_tui_command("/lane attach L1", &mut state));

        assert_eq!(state.lanes[0].status, "attached");
        assert!(state.lanes[0].target.starts_with("attach pid "));
        assert_eq!(state.focused_lane.as_deref(), Some("L1"));
        let attach =
            fs::read_to_string(root.join(".robocode/lanes/L1.attach.md")).expect("attach artifact");
        assert!(attach.contains("RoboCode Lane Attach"));
        assert!(attach.contains("printf attached-L1"));

        assert!(handle_tui_command("/lane detach L1", &mut state));

        assert_eq!(state.lanes[0].status, "detached");
        assert_eq!(state.focused_lane, None);
        assert!(
            state
                .entries
                .last()
                .expect("detach entry")
                .body
                .contains("without killing")
        );

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

    #[test]
    fn lane_decision_records_changed_files_and_inspect_evidence() {
        let root = temp_lane_root();
        fs::create_dir_all(&root).expect("temp root");
        Command::new("git")
            .arg("init")
            .current_dir(&root)
            .status()
            .expect("git init");
        fs::write(root.join("changed.txt"), "changed\n").expect("changed file");
        let store = root.join(".robocode").join("lanes.tsv");
        let mut state = test_state();
        state.workspace.root = root.clone();
        state.lane_store = Some(store);
        state.lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "codex".to_string(),
            title: "review generated patch".to_string(),
            status: "completed".to_string(),
            target: "main".to_string(),
            progress: 100,
            summary: "completed successfully".to_string(),
            worktree: None,
        }];

        assert!(handle_tui_command("/lane accept L1 looks good", &mut state));

        let decision = fs::read_to_string(root.join(".robocode/lanes/L1.decision.md"))
            .expect("decision artifact");
        assert!(decision.contains("Decision: accepted"));
        assert!(decision.contains("Summary: looks good"));
        assert!(decision.contains("?? changed.txt"));
        assert_eq!(state.lanes[0].status, "accepted");

        assert!(handle_tui_command("/lane inspect L1", &mut state));
        let inspect = state.entries.last().expect("inspect entry");
        assert!(inspect.body.contains("Changed files:"));
        assert!(inspect.body.contains("?? changed.txt"));
        assert!(inspect.body.contains("Decision: accepted"));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_lane_root() -> std::path::PathBuf {
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_nanos();
        let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("robocode-lane-test-{nanos}-{suffix}"))
    }

    fn init_git_repo(root: &Path) {
        fs::create_dir_all(root).expect("repo root");
        assert!(
            Command::new("git")
                .arg("init")
                .current_dir(root)
                .status()
                .expect("git init")
                .success()
        );
        fs::write(root.join("README.md"), "fixture\n").expect("fixture file");
        assert!(
            Command::new("git")
                .arg("add")
                .arg("README.md")
                .current_dir(root)
                .status()
                .expect("git add")
                .success()
        );
        assert!(
            Command::new("git")
                .args(["-c", "user.email=robot@example.invalid"])
                .args(["-c", "user.name=RoboCode Test"])
                .args(["commit", "-m", "initial"])
                .current_dir(root)
                .status()
                .expect("git commit")
                .success()
        );
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
