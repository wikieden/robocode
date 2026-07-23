use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use viden_lsp::LspRuntime;
use viden_types::{
    CheckRunStatus, CheckRunView, RuntimeEvent, RuntimeEventKind, RuntimeOwner,
    RuntimeServiceHealthView, ToolCall, ToolResult, WorkspaceChangeKind, WorkspaceChangeView,
    WorkspaceSourceStatus, WorkspaceSourceView,
};

use crate::SessionEngine;

/// Cockpit reducers retain at most 50 rows. Producers use the same limit so
/// source inspection cannot create an unbounded intermediate fact set.
pub(crate) const MAX_COCKPIT_ROWS: usize = 50;
/// Individual captured patches are bounded independently from the row cap.
pub(crate) const MAX_COCKPIT_PATCH_BYTES: usize = 64 * 1024;
const MAX_GIT_OUTPUT_BYTES: usize = 64 * 1024;
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn sample_workspace_source(cwd: &Path) -> WorkspaceSourceView {
    sample_workspace_source_with_git(cwd, Path::new("git"), GIT_COMMAND_TIMEOUT)
}

pub(crate) fn sample_workspace_source_with_git(
    cwd: &Path,
    git_program: &Path,
    timeout: Duration,
) -> WorkspaceSourceView {
    let worktree =
        match run_git_bounded(cwd, git_program, &["rev-parse", "--show-toplevel"], timeout) {
            GitOutput::Complete(output) => first_non_empty_line(&output).map(str::to_string),
            GitOutput::Truncated => return truncated_source(None, None),
            GitOutput::Failed | GitOutput::Unavailable => return unavailable_source(),
        };
    let Some(worktree) = worktree else {
        return unavailable_source();
    };
    let branch = match run_git_bounded(
        cwd,
        git_program,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        timeout,
    ) {
        GitOutput::Complete(output) => first_non_empty_line(&output).map(str::to_string),
        GitOutput::Truncated => return truncated_source(None, Some(worktree)),
        // Detached HEAD is a valid source state and makes symbolic-ref fail.
        GitOutput::Failed => None,
        GitOutput::Unavailable => return unavailable_source(),
    };
    let status = match run_git_bounded(
        cwd,
        git_program,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
        timeout,
    ) {
        GitOutput::Complete(output) => output,
        GitOutput::Truncated => return truncated_source(branch, Some(worktree)),
        GitOutput::Failed | GitOutput::Unavailable => return unavailable_source(),
    };
    let dirty = status.lines().next().is_some();
    let (ahead, behind) = match run_git_bounded(
        cwd,
        git_program,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        timeout,
    ) {
        GitOutput::Complete(output) => parse_ahead_behind(&output).unwrap_or((0, 0)),
        GitOutput::Truncated => return truncated_source(branch, Some(worktree)),
        // A repository without an upstream has no ahead/behind relation.
        GitOutput::Failed => (0, 0),
        GitOutput::Unavailable => return unavailable_source(),
    };
    let diff = match run_git_bounded(
        cwd,
        git_program,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--numstat",
            "HEAD",
            "--",
        ],
        timeout,
    ) {
        GitOutput::Complete(output) => output,
        GitOutput::Truncated => return truncated_source(branch, Some(worktree)),
        GitOutput::Failed | GitOutput::Unavailable => return unavailable_source(),
    };
    let Some((added, deleted)) = parse_numstat(&diff) else {
        return truncated_source(branch, Some(worktree));
    };

    WorkspaceSourceView {
        status: WorkspaceSourceStatus::Ready,
        branch,
        worktree: Some(worktree),
        ahead,
        behind,
        added,
        deleted,
        dirty,
    }
}

pub(crate) fn runtime_service_health(
    cwd: &Path,
    lsp_runtime: &LspRuntime,
) -> Vec<RuntimeServiceHealthView> {
    let mut services = crate::extension_commands::mcp_runtime_service_health(cwd);
    services.extend(crate::lsp_tools::lsp_runtime_service_health(lsp_runtime));
    services.truncate(MAX_COCKPIT_ROWS);
    services
}

pub(crate) fn lifecycle_events(cwd: &Path, lsp_runtime: &LspRuntime) -> Vec<RuntimeEvent> {
    let mut events = vec![RuntimeEvent::new(
        0,
        RuntimeEventKind::WorkspaceSourceUpdated {
            source: sample_workspace_source(cwd),
        },
    )];
    events.extend(
        runtime_service_health(cwd, lsp_runtime)
            .into_iter()
            .map(|service| {
                RuntimeEvent::new(0, RuntimeEventKind::RuntimeServiceHealthUpdated { service })
            }),
    );
    events
}

impl SessionEngine {
    pub(crate) fn frontend_status_lifecycle_events(&self) -> Vec<RuntimeEvent> {
        lifecycle_events(&self.cwd, self.lsp_runtime.as_ref())
    }
}

pub(crate) fn workspace_changes_from_tool_result(
    call: &ToolCall,
    result: &ToolResult,
    owner: &RuntimeOwner,
) -> Vec<WorkspaceChangeView> {
    if !result.success || !matches!(call.name.as_str(), "write_file" | "edit_file") {
        return Vec::new();
    }
    let Some(path) = call
        .input
        .get("path")
        .filter(|path| !path.trim().is_empty())
    else {
        return Vec::new();
    };
    let (additions, deletions) = result
        .diff
        .as_deref()
        .map(count_patch_lines)
        .unwrap_or((0, 0));
    let kind = result
        .diff
        .as_deref()
        .map(classify_structured_patch)
        .unwrap_or(WorkspaceChangeKind::Modified);
    let patch = result
        .diff
        .as_deref()
        .map(|patch| truncate_utf8_bytes(patch, MAX_COCKPIT_PATCH_BYTES));
    vec![WorkspaceChangeView {
        id: format!("{}:{path}", result.tool_call_id),
        owner: owner.clone(),
        path: path.clone(),
        kind,
        patch,
        additions,
        deletions,
    }]
}

pub(crate) fn check_run_from_tool_result(
    call: &ToolCall,
    result: &ToolResult,
    owner: &RuntimeOwner,
) -> Option<CheckRunView> {
    if call.name != "shell" {
        return None;
    }
    let command = call
        .input
        .get("command")
        .map(String::as_str)
        .filter(|command| is_check_command(command))?;
    let status = if result.success {
        CheckRunStatus::Passed
    } else if result.exit_code == Some(130) {
        CheckRunStatus::Cancelled
    } else {
        CheckRunStatus::Failed
    };
    let summary = match (status, result.exit_code) {
        (CheckRunStatus::Passed, _) => "passed".to_string(),
        (CheckRunStatus::Cancelled, Some(code)) => format!("cancelled (exit code {code})"),
        (CheckRunStatus::Failed, Some(code)) => format!("failed (exit code {code})"),
        (CheckRunStatus::Failed, None) => "failed (exit code unavailable)".to_string(),
        _ => status_label(status).to_string(),
    };
    Some(CheckRunView {
        id: result.tool_call_id.clone(),
        owner: owner.clone(),
        label: truncate_utf8_bytes(command, 120),
        command: truncate_utf8_bytes(command, 512),
        status,
        summary,
        failing_location: None,
    })
}

pub(crate) fn bind_fact_owner(kind: &mut RuntimeEventKind, owner: &RuntimeOwner) {
    match kind {
        RuntimeEventKind::WorkspaceChangeUpdated { change } => change.owner = owner.clone(),
        RuntimeEventKind::CheckRunUpdated { check } => check.owner = owner.clone(),
        _ => {}
    }
}

enum GitOutput {
    Complete(String),
    Truncated,
    Failed,
    Unavailable,
}

fn run_git_bounded(cwd: &Path, program: &Path, args: &[&str], timeout: Duration) -> GitOutput {
    let mut child = match Command::new(program.as_os_str())
        .arg("--no-optional-locks")
        .args(["-c", "core.fsmonitor=false"])
        .args(["-c", "core.untrackedCache=false"])
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return GitOutput::Unavailable,
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return GitOutput::Unavailable;
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut bytes = Vec::with_capacity(MAX_GIT_OUTPUT_BYTES.min(8192));
        let mut truncated = false;
        let mut chunk = [0_u8; 8192];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => break,
                Ok(count) => {
                    let remaining = MAX_GIT_OUTPUT_BYTES.saturating_sub(bytes.len());
                    let keep = remaining.min(count);
                    bytes.extend_from_slice(&chunk[..keep]);
                    if keep < count {
                        truncated = true;
                        break;
                    }
                }
                Err(_) => {
                    let _ = sender.send(None);
                    return;
                }
            }
        }
        let _ = sender.send(Some((bytes, truncated)));
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Err(_) => break None,
        }
    };
    let Some(status) = status else {
        // A descendant can inherit stdout after the timed-out process is
        // killed. Detach the reader instead of turning that pipe into an
        // unbounded wait on the status path.
        return GitOutput::Truncated;
    };
    let read = receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()
        .flatten();
    if read.is_some() {
        let _ = reader.join();
    }
    let Some((bytes, truncated)) = read else {
        return GitOutput::Truncated;
    };
    if truncated {
        return GitOutput::Truncated;
    }
    if !status.success() {
        return GitOutput::Failed;
    }
    GitOutput::Complete(String::from_utf8_lossy(&bytes).into_owned())
}

fn unavailable_source() -> WorkspaceSourceView {
    WorkspaceSourceView {
        status: WorkspaceSourceStatus::Unavailable,
        branch: None,
        worktree: None,
        ahead: 0,
        behind: 0,
        added: 0,
        deleted: 0,
        dirty: false,
    }
}

fn truncated_source(branch: Option<String>, worktree: Option<String>) -> WorkspaceSourceView {
    WorkspaceSourceView {
        status: WorkspaceSourceStatus::Truncated,
        branch,
        worktree,
        ahead: 0,
        behind: 0,
        added: 0,
        deleted: 0,
        dirty: false,
    }
}

fn first_non_empty_line(output: &str) -> Option<&str> {
    output.lines().map(str::trim).find(|line| !line.is_empty())
}

fn parse_ahead_behind(output: &str) -> Option<(u32, u32)> {
    let mut fields = output.split_whitespace();
    let ahead = fields.next()?.parse::<u32>().ok()?;
    let behind = fields.next()?.parse::<u32>().ok()?;
    Some((ahead, behind))
}

fn parse_numstat(output: &str) -> Option<(u32, u32)> {
    let mut added = 0_u32;
    let mut deleted = 0_u32;
    for line in output.lines() {
        let mut fields = line.split('\t');
        let line_added = parse_numstat_count(fields.next()?)?;
        let line_deleted = parse_numstat_count(fields.next()?)?;
        added = added.checked_add(line_added)?;
        deleted = deleted.checked_add(line_deleted)?;
    }
    Some((added, deleted))
}

fn parse_numstat_count(value: &str) -> Option<u32> {
    if value == "-" {
        Some(0)
    } else {
        value.parse().ok()
    }
}

fn count_patch_lines(patch: &str) -> (u32, u32) {
    patch
        .lines()
        .fold((0_u32, 0_u32), |(added, deleted), line| {
            if line.starts_with('+') && !line.starts_with("+++") {
                (added.saturating_add(1), deleted)
            } else if line.starts_with('-') && !line.starts_with("---") {
                (added, deleted.saturating_add(1))
            } else {
                (added, deleted)
            }
        })
}

fn classify_structured_patch(patch: &str) -> WorkspaceChangeKind {
    if patch.lines().any(|line| line == "--- /dev/null") {
        WorkspaceChangeKind::Added
    } else if patch.lines().any(|line| line == "+++ /dev/null") {
        WorkspaceChangeKind::Deleted
    } else if patch.lines().any(|line| line.starts_with("rename from "))
        && patch.lines().any(|line| line.starts_with("rename to "))
    {
        WorkspaceChangeKind::Renamed
    } else {
        WorkspaceChangeKind::Modified
    }
}

fn truncate_utf8_bytes(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    let mut end = max_bytes;
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_string()
}

fn is_check_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    [
        "cargo test",
        "cargo check",
        "cargo clippy",
        "cargo fmt",
        "pytest",
        "python -m pytest",
        "go test",
        "npm test",
        "npm run test",
        "pnpm test",
        "pnpm run test",
        "yarn test",
        "make test",
        "just test",
    ]
    .iter()
    .any(|prefix| {
        normalized == *prefix
            || normalized
                .strip_prefix(prefix)
                .and_then(|rest| rest.chars().next())
                .is_some_and(char::is_whitespace)
    }) || normalized.starts_with("scripts/")
        && normalized.split_whitespace().next().is_some_and(|script| {
            script.contains("test") || script.contains("check") || script.contains("smoke")
        })
}

fn status_label(status: CheckRunStatus) -> &'static str {
    match status {
        CheckRunStatus::Queued => "queued",
        CheckRunStatus::Running => "running",
        CheckRunStatus::Passed => "passed",
        CheckRunStatus::Failed => "failed",
        CheckRunStatus::Cancelled => "cancelled",
    }
}
