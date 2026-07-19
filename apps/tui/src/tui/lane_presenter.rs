use super::state::TerminalLane;

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
        "applied" => "[applied]",
        "apply_conflict" => "[conflict]",
        "attached" => "[attach]",
        "detached" => "[detach]",
        _ => "[idle]",
    }
}

pub(super) fn terminal_label(tool: &str) -> &'static str {
    match tool {
        "codex" => "codex tty",
        "codex-review" => "codex review",
        "claude" => "claude tty",
        "shell" | "run" => "shell tty",
        _ => "agent tty",
    }
}

pub(super) fn pty_label(tool: &str) -> &'static str {
    match tool {
        "codex" => "pty/01",
        "codex-review" => "pty/rev",
        "claude" => "pty/02",
        "shell" | "run" => "pty/ops",
        _ => "pty/xx",
    }
}

pub(super) fn pid_hint(lane: &TerminalLane) -> String {
    lane.target
        .strip_prefix("pty pid ")
        .or_else(|| lane.target.strip_prefix("attach pid "))
        .map(str::to_string)
        .unwrap_or_else(|| "----".to_string())
}

pub(super) fn command_hint(tool: &str, task: &str) -> String {
    match tool {
        "codex" => format!("codex exec {task}"),
        "codex-review" => format!("codex review --uncommitted {task}"),
        "claude" => format!("claude -p {task}"),
        "shell" | "run" => task.to_string(),
        _ => format!("{tool} {task}"),
    }
}

pub(super) fn interaction_hint(lane: &TerminalLane) -> String {
    if let Some(session) = lane.target.strip_prefix("tmux ") {
        return format!("tmux attach -t {session}");
    }
    if lane.target.starts_with("pty pid ") {
        return format!("/lane send {} <text>", lane.id);
    }
    if let Some(pid) = lane.target.strip_prefix("attach pid ") {
        return format!("external terminal pid {pid}");
    }
    format!("/lane tmux {}", lane.id)
}

pub(super) fn lane_next_action(lane: &TerminalLane) -> String {
    match lane.status.as_str() {
        "queued" | "running" | "attached" => format!(
            "watch or attach with `{}`; stop with `/lane stop {}` if it is no longer useful",
            interaction_hint(lane),
            lane.id
        ),
        "completed" if lane.worktree.is_some() => format!(
            "review changes, then `/lane accept {}` or `/lane revise {} <notes>`",
            lane.id, lane.id
        ),
        "completed" => format!("review evidence, then `/lane archive {}`", lane.id),
        "failed" => format!(
            "review the tail, then `/lane revise {} <notes>` or `/lane discard {} <reason>`",
            lane.id, lane.id
        ),
        "accepted" if lane.worktree.is_some() => {
            format!("apply isolated changes with `/lane apply {}`", lane.id)
        }
        "accepted" => format!("archive accepted evidence with `/lane archive {}`", lane.id),
        "apply_conflict" => format!(
            "resolve main/lane conflicts, then retry with `/lane resolve {}`",
            lane.id
        ),
        "applied" => format!(
            "review the main workspace diff, then `/lane cleanup {}` when evidence is no longer needed",
            lane.id
        ),
        "detached" => format!(
            "reattach with `/lane attach {}` or archive when done",
            lane.id
        ),
        "stopped" => format!(
            "inspect preserved evidence, then `/lane archive {}`",
            lane.id
        ),
        "archived" | "discarded" => {
            "no active action; evidence remains under `.viden/lanes/`".to_string()
        }
        "revise" => format!(
            "send revision notes to a fresh lane or archive with `/lane archive {}`",
            lane.id
        ),
        _ => format!(
            "inspect artifacts, then decide with `/lane accept {}`",
            lane.id
        ),
    }
}
