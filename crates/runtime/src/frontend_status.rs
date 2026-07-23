use std::path::Path;
use std::process::Command;

use viden_lsp::LspRuntime;
use viden_types::{
    CheckRunStatus, CheckRunView, RuntimeEvent, RuntimeEventKind, RuntimeOwner,
    RuntimeServiceHealthView, ToolCall, ToolResult, WorkspaceChangeKind, WorkspaceChangeView,
    WorkspaceSourceView,
};

use crate::SessionEngine;

/// Cockpit reducers retain at most 50 rows. Producers use the same limit so
/// source inspection cannot create an unbounded intermediate fact set.
pub(crate) const MAX_COCKPIT_ROWS: usize = 50;
/// Individual captured patches are bounded independently from the row cap.
pub(crate) const MAX_COCKPIT_PATCH_BYTES: usize = 64 * 1024;
const MAX_SOURCE_LINE_COUNT: u32 = 1_000_000;

pub(crate) fn sample_workspace_source(cwd: &Path) -> Option<WorkspaceSourceView> {
    let worktree = run_git(cwd, &["rev-parse", "--show-toplevel"])?;
    let worktree = first_non_empty_line(&worktree)?.to_string();
    let branch = run_git(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .and_then(|output| first_non_empty_line(&output).map(str::to_string));
    let status = run_git(
        cwd,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )?;
    let dirty = status.lines().next().is_some();
    let (ahead, behind) = run_git(
        cwd,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )
    .and_then(|output| parse_ahead_behind(&output))
    .unwrap_or((0, 0));
    let (added, deleted) = run_git(cwd, &["diff", "--numstat", "HEAD", "--"])
        .or_else(|| run_git(cwd, &["diff", "--numstat", "--"]))
        .map(|output| parse_numstat(&output))
        .unwrap_or((0, 0));

    Some(WorkspaceSourceView {
        branch,
        worktree: Some(worktree),
        ahead,
        behind,
        added,
        deleted,
        dirty,
    })
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
    let mut events = Vec::new();
    if let Some(source) = sample_workspace_source(cwd) {
        events.push(RuntimeEvent::new(
            0,
            RuntimeEventKind::WorkspaceSourceUpdated { source },
        ));
    }
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
    let patch = result
        .diff
        .as_deref()
        .map(|patch| truncate_utf8_bytes(patch, MAX_COCKPIT_PATCH_BYTES));
    let (additions, deletions) = patch.as_deref().map(count_patch_lines).unwrap_or((0, 0));
    let kind = match call.name.as_str() {
        "write_file" if deletions == 0 => WorkspaceChangeKind::Added,
        _ => WorkspaceChangeKind::Modified,
    };
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

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
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

fn parse_numstat(output: &str) -> (u32, u32) {
    output
        .lines()
        .take(MAX_COCKPIT_ROWS)
        .fold((0_u32, 0_u32), |(added, deleted), line| {
            let mut fields = line.split('\t');
            let line_added = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let line_deleted = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            (
                added.saturating_add(line_added).min(MAX_SOURCE_LINE_COUNT),
                deleted
                    .saturating_add(line_deleted)
                    .min(MAX_SOURCE_LINE_COUNT),
            )
        })
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
