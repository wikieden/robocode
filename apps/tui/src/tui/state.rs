use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use viden_provider::{ProviderAuthMode, ProviderDescriptor};
use viden_runtime::{EngineEvent, ProviderTelemetry};
use viden_types::{
    AgentLaneRecord, AgentNextAction, AgentTaskRecord, MemoryEntry, PermissionLevel, TaskRecord,
    WorkMode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiEntry {
    pub(super) label: String,
    pub(super) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiState {
    pub(super) session_id: String,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) provider_catalog: Vec<ProviderOption>,
    pub(super) provider_status: ProviderStatus,
    pub(super) theme_name: String,
    pub(super) input: String,
    pub(super) command_selection: usize,
    pub(super) command_palette_hidden_for: Option<String>,
    pub(super) approval_focus: usize,
    pub(super) approval_apply_all: bool,
    pub(super) pending_turn: Option<PendingTurn>,
    pub(super) streaming_assistant: Option<String>,
    pub(super) transcript_scroll: usize,
    pub(super) entries: Vec<TuiEntry>,
    pub(super) workspace: WorkspaceSnapshot,
    pub(super) tasks: Vec<TaskRecord>,
    pub(super) runtime_tasks: Vec<AgentTask>,
    pub(super) memory: Vec<MemoryEntry>,
    pub(super) screens: Vec<CompanionScreen>,
    pub(super) lanes: Vec<TerminalLane>,
    pub(super) lane_store: Option<PathBuf>,
    pub(super) focused_lane: Option<String>,
    pub(super) interaction_panel: Option<InteractionPanel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InteractionPanel {
    ConnectProvider {
        search: String,
        selected: usize,
    },
    ProviderConfig {
        provider_id: String,
        selected: usize,
    },
    ProviderApiKey {
        provider_id: String,
        input: String,
    },
    ModelPicker {
        provider_id: Option<String>,
        search: String,
        selected: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingTurn {
    pub(super) id: String,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) prompt: String,
    pub(super) workspace: String,
    pub(super) started_at: u128,
    pub(super) phase: String,
    pub(super) next_action: String,
    pub(super) queued_inputs: Vec<String>,
}

impl PendingTurn {
    pub(super) fn new(
        session_id: &str,
        provider: &str,
        model: &str,
        prompt: &str,
        workspace: &str,
    ) -> Self {
        let started_at = now_millis();
        Self {
            id: format!("turn-{}-{started_at}", compact_session_id(session_id)),
            provider: provider.to_string(),
            model: model.to_string(),
            prompt: first_line(prompt),
            workspace: workspace.to_string(),
            started_at,
            phase: "Waiting for provider response".to_string(),
            next_action: "wait".to_string(),
            queued_inputs: Vec::new(),
        }
    }
}

pub(super) type AgentTask = AgentTaskRecord;
pub(super) type AgentLane = AgentLaneRecord;

fn agent_lane_from_task(task: &AgentTask) -> AgentLane {
    AgentLane {
        id: format!("{}:{}", agent_screen(task), task.id),
        task_id: task.id.clone(),
        agent: agent_lane_label(task),
        screen: agent_screen(task).to_string(),
        transport: task.transport.clone(),
        status: task.status.clone(),
        summary: if task.summary.is_empty() {
            task.activity.clone()
        } else {
            task.summary.clone()
        },
        evidence: task.evidence.clone(),
    }
}

fn agent_task_from_lane(lane: &TerminalLane, lane_store: Option<&Path>) -> AgentTask {
    let status = normalized_lane_status(lane);
    let activity = lane_activity(lane, &status);
    let decision = lane_decision(lane);
    let result = lane_result(lane);
    AgentTask {
        id: lane.id.clone(),
        parent_id: None,
        agent: agent_label(&lane.tool),
        kind: "lane".to_string(),
        transport: lane_transport(&lane.tool, &lane.target).to_string(),
        title: lane.title.clone(),
        status,
        activity,
        summary: lane.summary.clone(),
        progress: lane.progress,
        started_at: None,
        updated_at: None,
        workspace: lane
            .worktree
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        evidence: lane_evidence(lane, lane_store),
        permissions: lane_permissions(lane),
        decision,
        result,
        resume_handle: lane_resume_handle(lane),
        pid: lane_pid(&lane.target),
        next_action: lane_next_action_record(lane),
    }
}

fn agent_task_from_codex_job(job: &AgentJob) -> AgentTask {
    AgentTask {
        id: job.id.clone(),
        parent_id: None,
        agent: agent_job_agent(job).to_string(),
        kind: "job".to_string(),
        transport: agent_job_transport(job).to_string(),
        title: job.task.clone(),
        status: normalized_codex_job_status(&job.status),
        activity: codex_job_activity(job),
        summary: job.task.clone(),
        progress: codex_job_progress(job),
        started_at: None,
        updated_at: Some(job.updated_at),
        workspace: None,
        evidence: job.evidence.clone(),
        permissions: codex_job_permissions(job),
        decision: None,
        result: job
            .result_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        resume_handle: codex_resume_handle(job),
        pid: job.pid,
        next_action: Some(AgentNextAction {
            label: "inspect agent".to_string(),
            command: Some(format!("/agent result {}", job.id)),
            reason: Some("agent job state is available".to_string()),
        }),
    }
}

fn agent_job_agent(job: &AgentJob) -> &'static str {
    if job.kind == "acp-session" {
        "acp"
    } else {
        "codex"
    }
}

fn agent_job_transport(job: &AgentJob) -> &'static str {
    if job.kind == "acp-session" {
        "acp"
    } else {
        "app-server"
    }
}

fn agent_task_from_pending_turn(turn: &PendingTurn) -> AgentTask {
    let queued_count = turn.queued_inputs.len();
    let queued_suffix = if queued_count == 0 {
        String::new()
    } else if queued_count == 1 {
        " · 1 prompt queued".to_string()
    } else {
        format!(" · {queued_count} prompts queued")
    };
    AgentTask {
        id: turn.id.clone(),
        parent_id: None,
        agent: "viden".to_string(),
        kind: "provider".to_string(),
        transport: turn.provider.clone(),
        title: turn.prompt.clone(),
        status: "thinking".to_string(),
        activity: turn.phase.clone(),
        summary: format!("Viden is processing the request{queued_suffix}"),
        progress: 0,
        started_at: Some(turn.started_at),
        updated_at: Some(now_millis()),
        workspace: Some(turn.workspace.clone()),
        evidence: vec![
            "live provider request".to_string(),
            format!("provider {}", turn.provider),
            format!("model {}", turn.model),
            format!("next_action {}", turn.next_action),
            format!("queued_inputs {}", turn.queued_inputs.len()),
        ],
        permissions: Vec::new(),
        decision: None,
        result: None,
        resume_handle: None,
        pid: None,
        next_action: Some(AgentNextAction {
            label: format!("{}{}", turn.next_action, queued_suffix),
            command: (turn.next_action != "wait").then(|| "/status".to_string()),
            reason: Some("provider turn is active".to_string()),
        }),
    }
}

pub(super) fn agent_lanes(state: &TuiState) -> Vec<AgentLane> {
    agent_tasks(state)
        .into_iter()
        .map(|task| agent_lane_from_task(&task))
        .collect()
}

fn agent_screen(task: &AgentTask) -> &'static str {
    match task.agent.as_str() {
        "codex" | "claude" => "side-1",
        "shell" => "side-2",
        _ if task.kind == "test" || task.kind == "diff" => "side-2",
        _ => "main",
    }
}

fn agent_lane_label(task: &AgentTask) -> String {
    if task.agent == "viden" && task.kind == "provider" {
        task.transport.clone()
    } else {
        task.agent.clone()
    }
}

pub(super) fn agent_tasks(state: &TuiState) -> Vec<AgentTask> {
    let mut tasks = Vec::new();
    if let Some(turn) = &state.pending_turn {
        tasks.push(agent_task_from_pending_turn(turn));
    }
    tasks.extend(state.runtime_tasks.iter().cloned());
    tasks.extend(transcript_agent_tasks(state));
    tasks.extend(
        state
            .lanes
            .iter()
            .map(|lane| agent_task_from_lane(lane, state.lane_store.as_deref())),
    );
    tasks.extend(
        state
            .workspace
            .agent_jobs
            .iter()
            .map(agent_task_from_codex_job),
    );
    tasks.sort_by(|left, right| {
        right
            .is_active()
            .cmp(&left.is_active())
            .then(right.priority().cmp(&left.priority()))
            .then(right.updated_at.cmp(&left.updated_at))
            .then(left.id.cmp(&right.id))
    });
    tasks
}

fn agent_label(tool: &str) -> String {
    match tool {
        "codex" => "codex".to_string(),
        "claude" => "claude".to_string(),
        "shell" | "run" => "shell".to_string(),
        other => other.to_string(),
    }
}

fn lane_transport(tool: &str, target: &str) -> &'static str {
    if target.starts_with("tmux ") {
        "tmux"
    } else if target.starts_with("pty ") || target.contains(" pty ") {
        "pty"
    } else if matches!(tool, "run" | "shell") {
        "shell"
    } else {
        "template"
    }
}

fn normalized_lane_status(lane: &TerminalLane) -> String {
    match lane.status.as_str() {
        "queued" => "queued".to_string(),
        "starting" => "thinking".to_string(),
        "running" => infer_work_status(&lane.title, &lane.summary),
        "attached" | "needs_input" => "needs_input".to_string(),
        "reviewing" => "waiting_approval".to_string(),
        "completed" if lane.worktree.is_some() => "waiting_approval".to_string(),
        "completed" | "done" | "accepted" | "applied" => "done".to_string(),
        "failed" => "failed".to_string(),
        "apply_conflict" => "blocked".to_string(),
        "discarded" | "detached" | "stopped" => "cancelled".to_string(),
        "archived" => "archived".to_string(),
        _ => lane.status.clone(),
    }
}

fn lane_activity(lane: &TerminalLane, status: &str) -> String {
    match status {
        "queued" => format!("queued: {}", lane.summary),
        "thinking" | "editing" | "testing" => infer_running_activity(&lane.title, &lane.summary),
        "needs_input" => "waiting for operator input".to_string(),
        "waiting_approval" => "waiting for review decision".to_string(),
        "done" => "result ready".to_string(),
        "failed" | "blocked" => lane.summary.clone(),
        status => format!("{status}: {}", lane.summary),
    }
}

fn lane_evidence(lane: &TerminalLane, lane_store: Option<&Path>) -> Vec<String> {
    let mut evidence = Vec::new();
    if !lane.target.is_empty() {
        evidence.push(format!("target {}", lane.target));
    }
    if lane.worktree.is_some() {
        evidence.push("isolated worktree".to_string());
    }
    evidence.push(format!("summary {}", lane.summary));
    evidence.extend(lane_artifact_evidence(lane, lane_store));
    evidence
}

fn lane_artifact_evidence(lane: &TerminalLane, lane_store: Option<&Path>) -> Vec<String> {
    let Some(artifact_dir) = lane_store
        .and_then(|path| path.parent())
        .map(|path| path.join("lanes"))
    else {
        return Vec::new();
    };
    let mut evidence = Vec::new();
    let apply_path = artifact_dir.join(format!("{}.apply.md", lane.id));
    let envelope_path = artifact_dir.join(format!("{}.envelope.md", lane.id));
    if envelope_path.exists() {
        evidence.extend(lane_context_evidence(&envelope_path));
    }
    if apply_path.exists() {
        evidence.extend(lane_markdown_evidence(&apply_path, false));
    }
    let conflict_path = artifact_dir.join(format!("{}.apply-conflict.md", lane.id));
    if conflict_path.exists() {
        evidence.extend(lane_markdown_evidence(&conflict_path, true));
    }
    evidence
}

fn lane_context_evidence(path: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut evidence = Vec::new();
    if let Some(pressure) = markdown_field(&content, "Context pressure") {
        evidence.push(format!("context_pressure {pressure}"));
    }
    if let Some(tokens) = markdown_field(&content, "Estimated tokens") {
        evidence.push(format!("context_tokens {tokens}"));
    }
    evidence
}

fn lane_markdown_evidence(path: &Path, conflict: bool) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut evidence = Vec::new();
    if let Some(patch) = markdown_field(&content, "Patch") {
        evidence.push(format!("patch {patch}"));
    }
    let changed_section = if conflict {
        "Lane worktree changed files"
    } else {
        "Workspace changed files after apply"
    };
    evidence.extend(
        markdown_section_items(&content, changed_section)
            .into_iter()
            .take(3)
            .map(|item| format!("changed {item}")),
    );
    if conflict
        && let Some(line) = markdown_section_items(&content, "Direct apply check")
            .into_iter()
            .find(|line| !line.eq_ignore_ascii_case("clean"))
    {
        evidence.push(format!("conflict {line}"));
    }
    evidence
}

fn markdown_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    content
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .map(|value| value.trim_matches('`').to_string())
        .filter(|value| !value.is_empty())
}

fn markdown_section_items(content: &str, heading: &str) -> Vec<String> {
    let target = format!("## {heading}");
    content
        .lines()
        .skip_while(|line| line.trim() != target)
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with("## "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("- ").to_string())
        .collect()
}

fn lane_permissions(lane: &TerminalLane) -> Vec<String> {
    if matches!(lane.tool.as_str(), "codex" | "claude" | "deepseek") || lane.worktree.is_some() {
        vec!["workspace-write after approval".to_string()]
    } else if matches!(lane.tool.as_str(), "run" | "shell") {
        vec!["shell approval".to_string()]
    } else {
        Vec::new()
    }
}

fn lane_decision(lane: &TerminalLane) -> Option<String> {
    match lane.status.as_str() {
        "accepted" => Some("accepted".to_string()),
        "revise" => Some("revise requested".to_string()),
        "discarded" => Some("discarded".to_string()),
        "applied" => Some("applied".to_string()),
        "apply_conflict" => Some("resolve conflicts".to_string()),
        _ => None,
    }
}

fn lane_result(lane: &TerminalLane) -> Option<String> {
    matches!(
        lane.status.as_str(),
        "completed" | "done" | "accepted" | "applied" | "failed" | "apply_conflict"
    )
    .then(|| lane.summary.clone())
}

fn lane_next_action_record(lane: &TerminalLane) -> Option<AgentNextAction> {
    let (label, command, reason) = match lane.status.as_str() {
        "queued" | "running" | "attached" => (
            "watch lane",
            Some(format!("/lane inspect {}", lane.id)),
            "lane is still active",
        ),
        "completed" if lane.worktree.is_some() => (
            "accept lane",
            Some(format!("/lane accept {}", lane.id)),
            "isolated lane needs operator acceptance before apply",
        ),
        "accepted" if lane.worktree.is_some() => (
            "apply lane",
            Some(format!("/lane apply {}", lane.id)),
            "accepted isolated lane has reviewable changes",
        ),
        "completed" => (
            "archive lane",
            Some(format!("/lane archive {}", lane.id)),
            "shell lane completed without isolated changes",
        ),
        "failed" => (
            "retry lane",
            Some(format!("/lane retry {}", lane.id)),
            "lane failed and can be replayed",
        ),
        "apply_conflict" => (
            "resolve lane",
            Some(format!("/lane resolve {}", lane.id)),
            "main workspace and lane patch conflict",
        ),
        "applied" => (
            "cleanup lane",
            Some(format!("/lane cleanup {}", lane.id)),
            "applied lane artifacts can be cleaned",
        ),
        _ => return None,
    };
    Some(AgentNextAction {
        label: label.to_string(),
        command,
        reason: Some(reason.to_string()),
    })
}

fn lane_resume_handle(lane: &TerminalLane) -> Option<String> {
    lane.target
        .strip_prefix("tmux ")
        .map(|session| format!("tmux attach -t {session}"))
        .or_else(|| {
            lane.target
                .contains(" pty ")
                .then(|| format!("/lane send {} <text>", lane.id))
        })
}

fn codex_job_activity(job: &AgentJob) -> String {
    match job.status.as_str() {
        "queued" => format!("queued: {}", job.task),
        "running" => job
            .evidence
            .first()
            .cloned()
            .unwrap_or_else(|| format!("running {}", job.kind)),
        "finished" | "observed" => "result ready".to_string(),
        "failed" => job
            .evidence
            .first()
            .cloned()
            .unwrap_or_else(|| "failed; inspect result".to_string()),
        status => format!("{status}: {}", job.task),
    }
}

fn codex_job_progress(job: &AgentJob) -> u8 {
    match job.status.as_str() {
        "queued" => 10,
        "running" => 65,
        "finished" | "observed" => 100,
        "failed" => 100,
        _ => 0,
    }
}

fn normalized_codex_job_status(status: &str) -> String {
    match status {
        "queued" => "queued",
        "running" => "thinking",
        "finished" | "observed" => "done",
        "failed" => "failed",
        "cancelled" | "canceled" => "cancelled",
        other => other,
    }
    .to_string()
}

fn codex_job_permissions(job: &AgentJob) -> Vec<String> {
    if job.kind == "acp-session" {
        vec!["agent permission gated".to_string()]
    } else if job.kind.contains("write") || job.kind.contains("rescue") {
        vec!["workspace-write approval".to_string()]
    } else {
        vec!["read-only".to_string()]
    }
}

fn codex_resume_handle(job: &AgentJob) -> Option<String> {
    job.evidence
        .iter()
        .find_map(|item| item.strip_prefix("resume ").map(ToOwned::to_owned))
}

fn transcript_agent_tasks(state: &TuiState) -> Vec<AgentTask> {
    let mut tasks = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some((index, entry)) = latest_approval_entry(&state.entries) {
        tasks.push(agent_task_from_approval(index, entry));
        seen.insert(index);
    }
    for predicate in [
        is_diff_entry as fn(&TuiEntry) -> bool,
        is_test_entry,
        is_tool_entry,
    ] {
        if let Some((index, entry)) = latest_entry_matching(&state.entries, predicate)
            && seen.insert(index)
        {
            tasks.push(agent_task_from_entry(index, entry, state));
        }
    }
    if state.pending_turn.is_none()
        && let Some((index, entry)) = latest_entry_matching(&state.entries, is_provider_entry)
        && provider_entry_can_drive_activity(&state.entries, index, entry)
        && !provider_turn_failed_after(&state.entries, index)
        && seen.insert(index)
    {
        tasks.push(agent_task_from_entry(index, entry, state));
    }
    tasks
}

fn latest_approval_entry(entries: &[TuiEntry]) -> Option<(usize, &TuiEntry)> {
    entries.iter().enumerate().rev().find(|(index, entry)| {
        entry.label == "approval"
            && entry.body.contains("Permission request for")
            && !entries[*index + 1..].iter().any(closes_pending_approval)
    })
}

fn closes_pending_approval(entry: &TuiEntry) -> bool {
    matches!(
        entry.label.as_str(),
        "tool-result" | "assistant" | "command"
    ) || (entry.label == "approval" && !entry.body.contains("Permission request for"))
}

fn latest_entry_matching(
    entries: &[TuiEntry],
    predicate: fn(&TuiEntry) -> bool,
) -> Option<(usize, &TuiEntry)> {
    entries
        .iter()
        .enumerate()
        .rev()
        .find(|(_, entry)| predicate(entry))
}

fn is_diff_entry(entry: &TuiEntry) -> bool {
    is_diff_view(&entry.body)
}

fn is_test_entry(entry: &TuiEntry) -> bool {
    entry.body.contains("Test result:")
}

fn is_tool_entry(entry: &TuiEntry) -> bool {
    matches!(entry.label.as_str(), "tool-call" | "tool-result")
}

fn is_provider_entry(entry: &TuiEntry) -> bool {
    matches!(entry.label.as_str(), "user" | "assistant")
}

fn provider_entry_can_drive_activity(
    entries: &[TuiEntry],
    provider_index: usize,
    entry: &TuiEntry,
) -> bool {
    entry.label != "user" || provider_index + 1 == entries.len()
}

fn provider_turn_failed_after(entries: &[TuiEntry], provider_index: usize) -> bool {
    entries[provider_index + 1..].iter().any(|entry| {
        entry.label == "system"
            && entry
                .body
                .contains("Provider turn failed, but Viden kept the TUI open.")
    })
}

fn agent_task_from_approval(index: usize, entry: &TuiEntry) -> AgentTask {
    let tool = approval_tool(&entry.body);
    let scope = approval_scope(&entry.body);
    AgentTask {
        id: format!("approval-{}", index + 1),
        parent_id: None,
        agent: "viden".to_string(),
        kind: "approval".to_string(),
        transport: "permission".to_string(),
        title: format!("{tool} approval"),
        status: "waiting_approval".to_string(),
        activity: format!("waiting approval for {tool}"),
        summary: scope.clone(),
        progress: 0,
        started_at: None,
        updated_at: None,
        workspace: None,
        evidence: vec!["transcript approval".to_string(), scope],
        permissions: vec![tool],
        decision: None,
        result: None,
        resume_handle: None,
        pid: None,
        next_action: Some(AgentNextAction {
            label: "approve or deny".to_string(),
            command: None,
            reason: Some("approval modal is waiting".to_string()),
        }),
    }
}

fn agent_task_from_entry(index: usize, entry: &TuiEntry, state: &TuiState) -> AgentTask {
    match entry.label.as_str() {
        "user" => AgentTask {
            id: format!("reply-{}", index + 1),
            parent_id: None,
            agent: "viden".to_string(),
            kind: "provider".to_string(),
            transport: state.provider.clone(),
            title: first_line(&entry.body),
            status: "thinking".to_string(),
            activity: "thinking through latest prompt".to_string(),
            summary: format!("{} / {} is processing", state.provider, state.model),
            progress: 0,
            started_at: None,
            updated_at: None,
            workspace: Some(state.workspace.display_root.clone()),
            evidence: vec!["latest user turn".to_string()],
            permissions: Vec::new(),
            decision: None,
            result: None,
            resume_handle: None,
            pid: None,
            next_action: Some(AgentNextAction {
                label: "watch provider".to_string(),
                command: Some("/status".to_string()),
                reason: Some("latest user turn is active".to_string()),
            }),
        },
        "tool-call" => tool_call_task(index, entry),
        "tool-result" => tool_result_task(index, entry),
        "assistant" => AgentTask {
            id: format!("reply-{}", index + 1),
            parent_id: None,
            agent: "viden".to_string(),
            kind: "provider".to_string(),
            transport: state.provider.clone(),
            title: first_line(&entry.body),
            status: "done".to_string(),
            activity: "reply ready".to_string(),
            summary: first_line(&entry.body),
            progress: 100,
            started_at: None,
            updated_at: None,
            workspace: Some(state.workspace.display_root.clone()),
            evidence: vec!["latest assistant reply".to_string()],
            permissions: Vec::new(),
            decision: None,
            result: Some(first_line(&entry.body)),
            resume_handle: None,
            pid: None,
            next_action: None,
        },
        _ if entry.body.contains("Test result:") => test_result_task(index, entry),
        _ if is_diff_view(&entry.body) => diff_view_task(index, entry),
        _ => AgentTask {
            id: format!("event-{}", index + 1),
            parent_id: None,
            agent: "viden".to_string(),
            kind: "event".to_string(),
            transport: "transcript".to_string(),
            title: first_line(&entry.body),
            status: "done".to_string(),
            activity: compact_entry_activity(entry),
            summary: first_line(&entry.body),
            progress: 100,
            started_at: None,
            updated_at: None,
            workspace: None,
            evidence: vec![format!("transcript {}", entry.label)],
            permissions: Vec::new(),
            decision: None,
            result: Some(first_line(&entry.body)),
            resume_handle: None,
            pid: None,
            next_action: None,
        },
    }
}

fn diff_view_task(index: usize, entry: &TuiEntry) -> AgentTask {
    let files = summary_field(&entry.body, "files").unwrap_or("0");
    let additions = summary_field(&entry.body, "additions").unwrap_or("0");
    let deletions = summary_field(&entry.body, "deletions").unwrap_or("0");
    let has_diff = files != "0";
    let mut evidence = vec![
        "transcript diff".to_string(),
        format!("files {files}"),
        format!("additions {additions}"),
        format!("deletions {deletions}"),
    ];
    evidence.extend(
        diff_paths(&entry.body)
            .into_iter()
            .map(|path| format!("path {path}")),
    );
    AgentTask {
        id: format!("diff-{}", index + 1),
        parent_id: None,
        agent: "shell".to_string(),
        kind: "diff".to_string(),
        transport: "local-git".to_string(),
        title: first_line(&entry.body),
        status: if has_diff { "needs_input" } else { "done" }.to_string(),
        activity: if has_diff {
            format!("review diff: {files} file(s) +{additions} -{deletions}")
        } else {
            "diff clean".to_string()
        },
        summary: format!("{files} file(s), +{additions} -{deletions}"),
        progress: 100,
        started_at: None,
        updated_at: None,
        workspace: None,
        evidence,
        permissions: Vec::new(),
        decision: None,
        result: Some(if has_diff {
            "diff needs review".to_string()
        } else {
            "no diff".to_string()
        }),
        resume_handle: Some("/diff".to_string()),
        pid: None,
        next_action: Some(AgentNextAction {
            label: "review diff".to_string(),
            command: Some("/diff".to_string()),
            reason: Some("diff evidence exists".to_string()),
        }),
    }
}

fn tool_call_task(index: usize, entry: &TuiEntry) -> AgentTask {
    let tool = entry.body.split_whitespace().next().unwrap_or("tool");
    let title = first_line(&entry.body);
    let activity = tool_call_activity(&entry.body);
    let status = if activity.starts_with("Editing") {
        "editing"
    } else if activity.starts_with("Testing") {
        "testing"
    } else {
        "running_tool"
    };
    let mut evidence = vec!["transcript tool-call".to_string(), format!("tool {tool}")];
    evidence.extend(tool_call_evidence(&entry.body));
    AgentTask {
        id: format!("tool-{}", index + 1),
        parent_id: None,
        agent: "viden".to_string(),
        kind: "tool".to_string(),
        transport: "local-tool".to_string(),
        title,
        status: status.to_string(),
        activity,
        summary: tool.to_string(),
        progress: 50,
        started_at: None,
        updated_at: None,
        workspace: None,
        evidence,
        permissions: tool_permissions(tool),
        decision: None,
        result: None,
        resume_handle: None,
        pid: None,
        next_action: None,
    }
}

fn tool_result_task(index: usize, entry: &TuiEntry) -> AgentTask {
    let failed = entry.body.to_ascii_lowercase().contains("error")
        || entry.body.to_ascii_lowercase().contains("failed");
    let mut evidence = vec!["transcript tool-result".to_string()];
    evidence.extend(tool_result_evidence(&entry.body));
    AgentTask {
        id: format!("tool-{}", index + 1),
        parent_id: None,
        agent: "viden".to_string(),
        kind: "tool".to_string(),
        transport: "local-tool".to_string(),
        title: first_line(&entry.body),
        status: if failed { "failed" } else { "done" }.to_string(),
        activity: if failed {
            "tool failed".to_string()
        } else {
            "tool result ready".to_string()
        },
        summary: first_line(&entry.body),
        progress: 100,
        started_at: None,
        updated_at: None,
        workspace: None,
        evidence,
        permissions: Vec::new(),
        decision: None,
        result: Some(first_line(&entry.body)),
        resume_handle: None,
        pid: None,
        next_action: None,
    }
}

fn test_result_task(index: usize, entry: &TuiEntry) -> AgentTask {
    let status = rendered_field(&entry.body, "status").unwrap_or("unknown");
    let command = rendered_field(&entry.body, "command").unwrap_or("<unknown command>");
    let duration = rendered_field(&entry.body, "duration").unwrap_or("-");
    let normalized = if matches!(status, "passed" | "ok" | "success") {
        "done"
    } else if status == "running" {
        "testing"
    } else {
        "failed"
    };
    let mut evidence = vec![
        "transcript test result".to_string(),
        format!("command {command}"),
        format!("status {status}"),
        format!("duration {duration}"),
    ];
    for failure in rendered_section_items(&entry.body, "failure summary:", 2) {
        evidence.push(format!("failure {failure}"));
    }
    for file in rendered_section_items(&entry.body, "failing files:", 3) {
        evidence.push(format!("failing-file {file}"));
    }
    for tail in rendered_section_items(&entry.body, "output tail:", 2) {
        evidence.push(format!("tail {tail}"));
    }
    if normalized == "failed" {
        evidence.push(format!("rerun {command}"));
    }
    AgentTask {
        id: format!("test-{}", index + 1),
        parent_id: None,
        agent: "shell".to_string(),
        kind: "test".to_string(),
        transport: "local-shell".to_string(),
        title: command.to_string(),
        status: normalized.to_string(),
        activity: format!("testing {command}"),
        summary: format!("{status} in {duration}"),
        progress: 100,
        started_at: None,
        updated_at: None,
        workspace: None,
        evidence,
        permissions: vec!["shell approval".to_string()],
        decision: None,
        result: Some(status.to_string()),
        resume_handle: None,
        pid: None,
        next_action: Some(AgentNextAction {
            label: "rerun test".to_string(),
            command: Some(format!("/test {command}")),
            reason: Some("test evidence can be refreshed".to_string()),
        }),
    }
}

fn approval_tool(body: &str) -> String {
    body.split('`').nth(1).unwrap_or("tool").to_string()
}

fn approval_scope(body: &str) -> String {
    body.lines()
        .skip(1)
        .find(|line| !line.trim().is_empty() && !line.contains("Press y"))
        .unwrap_or("waiting for decision")
        .trim()
        .to_string()
}

fn first_line(body: &str) -> String {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| clean_display_fragment(line.trim(), 120))
        .unwrap_or_else(|| "no detail available".to_string())
}

fn compact_session_id(session_id: &str) -> String {
    session_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn tool_call_activity(body: &str) -> String {
    let detail = first_line(body);
    if let Some((tool, rest)) = detail.split_once(" path: ") {
        let path = rest.split_whitespace().next().unwrap_or(rest);
        if matches!(tool, "write_file" | "edit_file") {
            return format!("Editing {path}");
        }
    }
    if detail.contains("cargo test")
        || detail.contains("pytest")
        || detail.contains("npm test")
        || detail.contains(" test ")
    {
        format!("Testing {detail}")
    } else {
        format!("Running tool {detail}")
    }
}

fn tool_call_evidence(body: &str) -> Vec<String> {
    let detail = first_line(body);
    let mut evidence = Vec::new();
    if let Some(path) = field_after_marker(&detail, " path: ") {
        evidence.push(format!("path {path}"));
    }
    if let Some(lines) = field_after_marker(&detail, " lines: ") {
        evidence.push(format!("lines {lines}"));
    }
    if let Some(command) = field_after_marker(&detail, " command=") {
        evidence.push(format!("command {command}"));
    }
    evidence
}

fn tool_result_evidence(body: &str) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(path) = key_value_line(body, "path") {
        evidence.push(format!("path {path}"));
    } else if let Some(path) = wrote_path(body) {
        evidence.push(format!("path {path}"));
    }
    if let Some(lines) = wrote_line_count(body) {
        evidence.push(format!("lines {lines}"));
    }
    if let Some(files) = changed_files_summary(body) {
        evidence.push(format!("changed {files}"));
    }
    evidence
}

fn field_after_marker(detail: &str, marker: &str) -> Option<String> {
    detail
        .split_once(marker)
        .map(|(_, value)| {
            value
                .split_whitespace()
                .next()
                .unwrap_or(value)
                .trim_matches('`')
                .trim_matches(',')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn key_value_line(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    body.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .map(|value| value.trim_matches('`').to_string())
        .filter(|value| !value.is_empty())
}

fn wrote_path(body: &str) -> Option<String> {
    first_line(body)
        .split_once(" to ")
        .map(|(_, rest)| rest.split_whitespace().next().unwrap_or(rest).to_string())
        .map(|path| path.trim_matches('`').to_string())
        .filter(|path| path.contains('/'))
}

fn wrote_line_count(body: &str) -> Option<String> {
    let line = first_line(body);
    let (_, after_wrote) = line.split_once("Wrote ")?;
    let (count, _) = after_wrote.split_once(" lines")?;
    Some(count.trim().to_string()).filter(|count| !count.is_empty())
}

fn changed_files_summary(body: &str) -> Option<String> {
    body.lines()
        .skip_while(|line| !line.trim().eq_ignore_ascii_case("Changed files:"))
        .skip(1)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("- ").to_string())
}

fn rendered_section_items(body: &str, heading: &str, max: usize) -> Vec<String> {
    body.lines()
        .skip_while(|line| !line.trim().eq_ignore_ascii_case(heading))
        .skip(1)
        .map(str::trim)
        .take_while(|line| !is_rendered_section_heading(line))
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("- ").trim_matches('`').to_string())
        .filter(|line| !line.is_empty())
        .take(max)
        .collect()
}

fn is_rendered_section_heading(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.ends_with(':') && !trimmed.starts_with('-') && !trimmed.starts_with("error:")
}

fn tool_permissions(tool: &str) -> Vec<String> {
    match tool {
        "write_file" | "edit_file" | "shell" => vec!["approval required".to_string()],
        _ => Vec::new(),
    }
}

fn compact_entry_activity(entry: &TuiEntry) -> String {
    match entry.label.as_str() {
        "command" => "command result".to_string(),
        "system" => "system event".to_string(),
        _ => "transcript event".to_string(),
    }
}

fn rendered_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("  {key}: ");
    body.lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
}

fn is_diff_view(body: &str) -> bool {
    (body.starts_with("Latest diff:") || body.starts_with("Git diff:"))
        && body.contains("  Summary: files=")
}

fn summary_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let summary = body
        .lines()
        .find_map(|line| line.trim().strip_prefix("Summary: "))?;
    summary.split_whitespace().find_map(|part| {
        let (label, value) = part.split_once('=')?;
        (label == key && !value.is_empty()).then_some(value)
    })
}

fn diff_paths(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("diff --git ")?;
            let mut parts = rest.split_whitespace();
            let _before = parts.next()?;
            let after = parts.next()?;
            Some(after.trim_start_matches("b/").to_string())
        })
        .take(4)
        .collect()
}

fn infer_work_status(title: &str, summary: &str) -> String {
    let lower = format!("{title} {summary}").to_ascii_lowercase();
    if ["test", "cargo", "pytest", "npm test"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        "testing".to_string()
    } else if ["edit", "write", "patch", "diff", "file"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        "editing".to_string()
    } else {
        "thinking".to_string()
    }
}

fn infer_running_activity(title: &str, summary: &str) -> String {
    let lower = format!("{title} {summary}").to_ascii_lowercase();
    if ["test", "cargo", "pytest", "npm test"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        format!("Running tests: {summary}")
    } else if ["edit", "write", "patch", "diff", "file"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        format!("Editing: {summary}")
    } else {
        format!("Thinking: {summary}")
    }
}

fn lane_pid(target: &str) -> Option<u32> {
    target
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|window| {
            (window[0] == "pid")
                .then(|| window[1].parse().ok())
                .flatten()
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderOption {
    pub(super) provider_id: String,
    pub(super) display_name: String,
    pub(super) default_api_base: Option<String>,
    pub(super) default_model: Option<String>,
    pub(super) known_models: Vec<String>,
    pub(super) enabled_models: Vec<String>,
    pub(super) favorite_models: Vec<String>,
    pub(super) api_key_env: Option<String>,
    pub(super) api_base_env: Option<String>,
    pub(super) auth_modes: Vec<ProviderAuthMode>,
}

impl ProviderOption {
    pub(super) fn from_descriptor(descriptor: &ProviderDescriptor) -> Self {
        Self {
            provider_id: descriptor.provider_id.clone(),
            display_name: descriptor.display_name.clone(),
            default_api_base: descriptor.default_api_base.clone(),
            default_model: descriptor.default_model.clone(),
            known_models: descriptor.known_models.clone(),
            enabled_models: Vec::new(),
            favorite_models: Vec::new(),
            api_key_env: descriptor.env_mappings.api_key_env.clone(),
            api_base_env: descriptor.env_mappings.api_base_env.clone(),
            auth_modes: descriptor.auth_modes.clone(),
        }
    }

    pub(super) fn fixture() -> Vec<Self> {
        vec![
            Self {
                provider_id: "anthropic".to_string(),
                display_name: "Anthropic".to_string(),
                default_api_base: Some("https://api.anthropic.com".to_string()),
                default_model: Some("claude-sonnet-4-5".to_string()),
                known_models: vec![
                    "claude-opus-4-5".to_string(),
                    "claude-sonnet-4-5".to_string(),
                    "claude-haiku-4-5".to_string(),
                ],
                enabled_models: vec!["claude-sonnet-4-5".to_string()],
                favorite_models: Vec::new(),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                api_base_env: Some("VIDEN_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            Self {
                provider_id: "deepseek".to_string(),
                display_name: "DeepSeek".to_string(),
                default_api_base: Some("https://api.deepseek.com".to_string()),
                default_model: Some("deepseek-v4-flash".to_string()),
                known_models: vec![
                    "deepseek-v4-flash".to_string(),
                    "deepseek-v4-pro".to_string(),
                    "deepseek-chat".to_string(),
                    "deepseek-reasoner".to_string(),
                ],
                enabled_models: vec![
                    "deepseek-v4-flash".to_string(),
                    "deepseek-v4-pro".to_string(),
                ],
                favorite_models: vec!["deepseek-v4-pro".to_string()],
                api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
                api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            Self {
                provider_id: "dashscope-coding-plan".to_string(),
                display_name: "DashScope Coding Plan".to_string(),
                default_api_base: Some("https://coding.dashscope.aliyuncs.com/v1".to_string()),
                default_model: Some("qwen3.6-plus".to_string()),
                known_models: vec![
                    "qwen3.6-plus".to_string(),
                    "qwen3.5-plus".to_string(),
                    "qwen3-max-2026-01-23".to_string(),
                    "qwen3-coder-next".to_string(),
                    "qwen3-coder-plus".to_string(),
                    "kimi-k2.5".to_string(),
                    "glm-5".to_string(),
                    "glm-4.7".to_string(),
                    "MiniMax-M2.5".to_string(),
                ],
                enabled_models: Vec::new(),
                favorite_models: Vec::new(),
                api_key_env: Some("DASHSCOPE_CODING_PLAN_API_KEY".to_string()),
                api_base_env: Some("DASHSCOPE_CODING_PLAN_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            Self {
                provider_id: "dashscope-tokenplan".to_string(),
                display_name: "DashScope TokenPlan".to_string(),
                default_api_base: Some(
                    "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1"
                        .to_string(),
                ),
                default_model: Some("qwen3.6-plus".to_string()),
                known_models: vec![
                    "qwen3.7-max".to_string(),
                    "qwen3.6-plus".to_string(),
                    "qwen3.6-flash".to_string(),
                    "deepseek-v4-flash".to_string(),
                    "kimi-k2.6".to_string(),
                    "kimi-k2.5".to_string(),
                    "glm-5.1".to_string(),
                    "MiniMax-M2.5".to_string(),
                ],
                enabled_models: Vec::new(),
                favorite_models: Vec::new(),
                api_key_env: Some("DASHSCOPE_API_KEY".to_string()),
                api_base_env: Some("DASHSCOPE_TOKENPLAN_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            Self {
                provider_id: "openrouter".to_string(),
                display_name: "OpenRouter".to_string(),
                default_api_base: Some("https://openrouter.ai/api/v1".to_string()),
                default_model: None,
                known_models: vec![
                    "openai/gpt-5.2".to_string(),
                    "anthropic/claude-sonnet-4.5".to_string(),
                    "qwen/qwen3-coder-plus".to_string(),
                ],
                enabled_models: vec!["deepseek/deepseek-v4-flash".to_string()],
                favorite_models: Vec::new(),
                api_key_env: Some("OPENROUTER_API_KEY".to_string()),
                api_base_env: Some("OPENROUTER_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            Self {
                provider_id: "fallback".to_string(),
                display_name: "Fallback".to_string(),
                default_api_base: None,
                default_model: Some("fallback-local".to_string()),
                known_models: vec!["fallback-local".to_string(), "test-local".to_string()],
                enabled_models: vec!["fallback-local".to_string(), "test-local".to_string()],
                favorite_models: Vec::new(),
                api_key_env: None,
                api_base_env: None,
                auth_modes: vec![ProviderAuthMode::Local],
            },
            Self {
                provider_id: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                default_api_base: Some("https://api.openai.com/v1".to_string()),
                default_model: Some("gpt-5.2".to_string()),
                known_models: vec![
                    "gpt-5.2".to_string(),
                    "gpt-5.2-codex".to_string(),
                    "gpt-5.1".to_string(),
                ],
                enabled_models: vec!["gpt-5.2".to_string()],
                favorite_models: Vec::new(),
                api_key_env: Some("OPENAI_API_KEY".to_string()),
                api_base_env: Some("VIDEN_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::WebLogin, ProviderAuthMode::ApiKey],
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompanionScreen {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) pid: Option<u32>,
    pub(super) summary: String,
}

impl CompanionScreen {
    fn to_tsv(&self) -> String {
        [
            escape_tsv(&self.id),
            escape_tsv(&self.title),
            escape_tsv(&self.status),
            self.pid.map(|pid| pid.to_string()).unwrap_or_default(),
            escape_tsv(&self.summary),
        ]
        .join("\t")
    }

    fn from_tsv(value: &str) -> Option<Self> {
        let fields = value.split('\t').map(unescape_tsv).collect::<Vec<_>>();
        if fields.len() != 5 {
            return None;
        }
        let pid = fields[3].parse::<u32>().ok();
        Some(Self {
            id: fields[0].clone(),
            title: fields[1].clone(),
            status: fields[2].clone(),
            pid,
            summary: fields[4].clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TerminalLane {
    pub(super) id: String,
    pub(super) tool: String,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) target: String,
    pub(super) progress: u8,
    pub(super) summary: String,
    pub(super) worktree: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LaneRuntimeEvidence {
    pub(super) log_path: PathBuf,
    pub(super) done_path: PathBuf,
    pub(super) envelope_path: PathBuf,
    pub(super) exit_code: Option<String>,
    pub(super) log_tail: Vec<String>,
    pub(super) envelope_preview: Vec<String>,
}

impl TerminalLane {
    pub(super) fn from_command(index: usize, command: &str) -> Option<Self> {
        let mut parts = command.split_whitespace();
        let slash = parts.next()?;
        if slash != "/lane" {
            return None;
        }
        let tool = parts.next()?;
        let (tool, title) = if tool == "ask" {
            let tool = parts.next()?;
            let title = parts.collect::<Vec<_>>().join(" ");
            (tool, title)
        } else {
            (tool, parts.collect::<Vec<_>>().join(" "))
        };
        if title.is_empty() {
            return None;
        }
        let status = match tool {
            "codex" | "codex-review" | "claude" | "run" => "queued",
            _ => "manual",
        };
        Some(Self {
            id: format!("L{index}"),
            tool: tool.to_string(),
            title,
            status: status.to_string(),
            target: "main".to_string(),
            progress: 0,
            summary: "waiting for terminal adapter".to_string(),
            worktree: None,
        })
    }

    pub(super) fn preview_lanes() -> Vec<Self> {
        vec![
            Self {
                id: "L1".to_string(),
                tool: "codex".to_string(),
                title: "test fixes".to_string(),
                status: "running".to_string(),
                target: "main".to_string(),
                progress: 64,
                summary: "patched failing tests; rerunning cargo".to_string(),
                worktree: None,
            },
            Self {
                id: "L2".to_string(),
                tool: "claude".to_string(),
                title: "review diff".to_string(),
                status: "attached".to_string(),
                target: "tmux viden-c4f2b7e-l2".to_string(),
                progress: 32,
                summary: "tmux session ready; reviewing config architecture".to_string(),
                worktree: None,
            },
            Self {
                id: "L3".to_string(),
                tool: "shell".to_string(),
                title: "cargo test".to_string(),
                status: "idle".to_string(),
                target: "ops".to_string(),
                progress: 100,
                summary: "last run green; no failures cached".to_string(),
                worktree: None,
            },
        ]
    }

    fn to_tsv(&self) -> String {
        [
            escape_tsv(&self.id),
            escape_tsv(&self.tool),
            escape_tsv(&self.title),
            escape_tsv(&self.status),
            escape_tsv(&self.target),
            self.progress.to_string(),
            escape_tsv(&clean_display_fragment(&self.summary, 120)),
            escape_tsv(
                self.worktree
                    .as_ref()
                    .map(|path| path.to_string_lossy())
                    .as_deref()
                    .unwrap_or_default(),
            ),
        ]
        .join("\t")
    }

    fn from_tsv(value: &str) -> Option<Self> {
        let fields = value.split('\t').map(unescape_tsv).collect::<Vec<_>>();
        if fields.len() != 5 && fields.len() != 7 && fields.len() != 8 {
            return None;
        }
        let progress = fields
            .get(5)
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0)
            .min(100);
        let summary = fields
            .get(6)
            .map(|value| clean_display_fragment(value, 120))
            .unwrap_or_else(|| "restored from lane store".to_string());
        let worktree = fields
            .get(7)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Some(Self {
            id: fields[0].clone(),
            tool: fields[1].clone(),
            title: fields[2].clone(),
            status: fields[3].clone(),
            target: fields[4].clone(),
            progress,
            summary,
            worktree,
        })
    }
}

pub(super) fn lane_store_path(root: &Path) -> PathBuf {
    root.join(".viden").join("lanes.tsv")
}

pub(super) fn screen_store_path(root: &Path) -> PathBuf {
    root.join(".viden").join("screens.tsv")
}

pub(super) fn diagnostics_store_path(root: &Path) -> PathBuf {
    root.join(".viden").join("diagnostics.txt")
}

pub(super) fn save_diagnostics(root: &Path, diagnostics: &[String]) -> Result<(), String> {
    let path = diagnostics_store_path(root);
    if diagnostics.is_empty() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.to_string()),
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, format!("{}\n", diagnostics.join("\n"))).map_err(|err| err.to_string())
}

pub(super) fn load_lanes(path: &Path) -> Vec<TerminalLane> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content.lines().filter_map(TerminalLane::from_tsv).collect()
}

pub(super) fn save_lanes(path: &Path, lanes: &[TerminalLane]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let content = lanes
        .iter()
        .map(TerminalLane::to_tsv)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, format!("{content}\n")).map_err(|err| err.to_string())
}

pub(super) fn load_screens(path: &Path) -> Vec<CompanionScreen> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(CompanionScreen::from_tsv)
        .collect()
}

pub(super) fn save_screens(path: &Path, screens: &[CompanionScreen]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let content = screens
        .iter()
        .map(CompanionScreen::to_tsv)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, format!("{content}\n")).map_err(|err| err.to_string())
}

pub(super) fn refresh_lane_runtime(path: &Path, lanes: &mut [TerminalLane]) {
    for lane in lanes {
        // Operator decisions are durable states; runtime artifacts must not downgrade them.
        if matches!(
            lane.status.as_str(),
            "accepted"
                | "revise"
                | "discarded"
                | "applied"
                | "apply_conflict"
                | "archived"
                | "detached"
                | "stopped"
        ) {
            continue;
        }
        let Some(evidence) = lane_runtime_evidence(path, &lane.id) else {
            continue;
        };
        if let Some(summary) = evidence.log_tail.last().cloned() {
            lane.summary = summary;
            lane.progress = lane.progress.clamp(35, 95);
        }
        let Some(exit_code) = evidence.exit_code else {
            continue;
        };
        lane.progress = 100;
        if exit_code == "0" {
            lane.status = "completed".to_string();
            if lane.summary.is_empty() {
                lane.summary = "completed successfully".to_string();
            }
            append_lane_timeline_once(
                path,
                &lane.id,
                "lane.completed",
                &format!("completed with exit 0: {}", lane.summary),
            );
        } else {
            lane.status = "failed".to_string();
            lane.summary = if lane.summary.is_empty() {
                format!("exited with status {exit_code}")
            } else {
                format!("{} (exit {exit_code})", lane.summary)
            };
            append_lane_timeline_once(
                path,
                &lane.id,
                "lane.failed",
                &format!("failed with exit {exit_code}: {}", lane.summary),
            );
        }
    }
}

fn append_lane_timeline_once(store: &Path, lane_id: &str, kind: &str, summary: &str) {
    let Some(parent) = store.parent() else {
        return;
    };
    let artifact_dir = parent.join("lanes");
    let path = artifact_dir.join(format!("{lane_id}.timeline.md"));
    if fs::read_to_string(&path)
        .ok()
        .is_some_and(|content| content.contains(&format!("Kind: {kind}")))
    {
        return;
    }
    if fs::create_dir_all(&artifact_dir).is_err() {
        return;
    }
    let timestamp = now_millis();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(
            file,
            "## {timestamp} {kind}\nKind: {kind}\nSummary: {summary}\n"
        );
    }
}

pub(super) fn lane_runtime_evidence(path: &Path, lane_id: &str) -> Option<LaneRuntimeEvidence> {
    let artifact_dir = path.parent()?.join("lanes");
    let log_path = artifact_dir.join(format!("{lane_id}.log"));
    let done_path = artifact_dir.join(format!("{lane_id}.done"));
    let envelope_path = artifact_dir.join(format!("{lane_id}.envelope.md"));
    let log_tail = log_tail(&log_path, 5);
    let envelope_preview = file_head(&envelope_path, 12);
    let exit_code = fs::read_to_string(&done_path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Some(LaneRuntimeEvidence {
        log_path,
        done_path,
        envelope_path,
        exit_code,
        log_tail,
        envelope_preview,
    })
}

fn log_tail(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = content
        .lines()
        .map(|line| clean_display_fragment(line, 120))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .filter(|line| !is_terminal_prompt_noise(line))
        .collect::<Vec<_>>();
    let keep_from = lines.len().saturating_sub(max_lines);
    lines.drain(0..keep_from);
    lines
}

fn clean_display_fragment(value: &str, max_chars: usize) -> String {
    sanitize_terminal_controls(value)
        .chars()
        .take(max_chars)
        .collect()
}

fn sanitize_terminal_controls(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            skip_escape_sequence(&mut chars);
            continue;
        }
        match ch {
            // Terminal logs can include carriage-return redraws from shells and
            // progress UIs. Keep only the final visible segment for summaries.
            '\r' => output.clear(),
            '\u{8}' => {
                output.pop();
            }
            '\t' => output.push(' '),
            _ if ch.is_control() => {}
            _ => output.push(ch),
        }
    }
    output
}

fn is_terminal_prompt_noise(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "%"
        || trimmed == "$"
        || trimmed == "#"
        || trimmed.starts_with("➜ ")
        || trimmed.starts_with("➜\u{a0}")
}

fn skip_escape_sequence<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            for ch in chars.by_ref() {
                if ('@'..='~').contains(&ch) {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            let mut saw_escape = false;
            for ch in chars.by_ref() {
                if ch == '\u{7}' || (saw_escape && ch == '\\') {
                    break;
                }
                saw_escape = ch == '\u{1b}';
            }
        }
        Some('P' | '^' | '_' | 'X') => {
            chars.next();
            let mut saw_escape = false;
            for ch in chars.by_ref() {
                if saw_escape && ch == '\\' {
                    break;
                }
                saw_escape = ch == '\u{1b}';
            }
        }
        Some('(' | ')' | '*' | '+' | '-' | '.' | '/') => {
            chars.next();
            let _ = chars.next();
        }
        Some(_) => {
            let _ = chars.next();
        }
        None => {}
    }
}

fn file_head(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .take(max_lines)
        .map(|line| line.chars().take(120).collect::<String>())
        .collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_tsv(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some('\\') => output.push('\\'),
            Some(other) => {
                output.push('\\');
                output.push(other);
            }
            None => output.push('\\'),
        }
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderStatus {
    pub(super) connection: String,
    pub(super) telemetry: String,
    pub(super) context_window: String,
    pub(super) work_mode: WorkMode,
    pub(super) permission_level: PermissionLevel,
    pub(super) request_count: u64,
    pub(super) success_count: u64,
    pub(super) failure_count: u64,
    pub(super) last_latency_ms: Option<u128>,
    pub(super) average_latency_ms: Option<u128>,
    pub(super) last_event_count: usize,
    pub(super) last_error: Option<String>,
    pub(super) last_input_tokens: Option<u64>,
    pub(super) last_output_tokens: Option<u64>,
    pub(super) last_total_tokens: Option<u64>,
    pub(super) total_tokens: u64,
    pub(super) last_tokens_per_second: Option<u64>,
    pub(super) last_cost_micro_usd: Option<u64>,
    pub(super) total_cost_micro_usd: Option<u64>,
}

impl ProviderStatus {
    pub(super) fn configured() -> Self {
        Self::from_telemetry(&ProviderTelemetry::default())
    }

    pub(super) fn from_telemetry(telemetry: &ProviderTelemetry) -> Self {
        let connection = if telemetry.last_error.is_some() {
            "Error"
        } else if telemetry.request_count > 0 {
            "Healthy"
        } else {
            "Configured"
        };
        let telemetry_label = if telemetry.request_count == 0 {
            "not sampled".to_string()
        } else {
            format!(
                "{} req / {} ok / {} err",
                telemetry.request_count, telemetry.success_count, telemetry.failure_count
            )
        };
        Self {
            connection: connection.to_string(),
            telemetry: telemetry_label,
            context_window: "128k".to_string(),
            work_mode: WorkMode::Build,
            permission_level: PermissionLevel::Ask,
            request_count: telemetry.request_count,
            success_count: telemetry.success_count,
            failure_count: telemetry.failure_count,
            last_latency_ms: telemetry.last_latency_ms,
            average_latency_ms: telemetry.average_latency_ms,
            last_event_count: telemetry.last_event_count,
            last_error: telemetry.last_error.clone(),
            last_input_tokens: telemetry.last_input_tokens,
            last_output_tokens: telemetry.last_output_tokens,
            last_total_tokens: telemetry.last_total_tokens,
            total_tokens: telemetry.total_tokens,
            last_tokens_per_second: telemetry.last_tokens_per_second,
            last_cost_micro_usd: telemetry.last_cost_micro_usd,
            total_cost_micro_usd: telemetry.total_cost_micro_usd,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspaceSnapshot {
    pub(super) root: PathBuf,
    pub(super) display_root: String,
    pub(super) git_branch: String,
    pub(super) git_branches: Vec<String>,
    pub(super) git_remotes: Vec<String>,
    pub(super) git_remote_branches: Vec<GitRemoteBranchEntry>,
    pub(super) git_stashes: Vec<GitStashEntry>,
    pub(super) git_worktrees: Vec<GitWorktreeEntry>,
    pub(super) file_count: usize,
    pub(super) line_count: usize,
    pub(super) recent_files: Vec<RecentFile>,
    pub(super) top_files: Vec<String>,
    pub(super) workspace_paths: Vec<String>,
    pub(super) diagnostics: Vec<String>,
    pub(super) agent_jobs: Vec<AgentJob>,
    pub(super) primary_language: String,
    pub(super) rust_edition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentJob {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) status: String,
    pub(super) task: String,
    pub(super) pid: Option<u32>,
    pub(super) log_path: Option<PathBuf>,
    pub(super) result_path: Option<PathBuf>,
    pub(super) evidence: Vec<String>,
    pub(super) updated_at: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RecentFile {
    pub(super) path: String,
    pub(super) modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitStashEntry {
    pub(super) reference: String,
    pub(super) summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitRemoteBranchEntry {
    pub(super) remote: String,
    pub(super) branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitWorktreeEntry {
    pub(super) path: String,
    pub(super) branch: Option<String>,
}

impl WorkspaceSnapshot {
    pub(super) fn load_current() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::load(root)
    }

    pub(super) fn load(root: PathBuf) -> Self {
        let git_branch = git_branch(&root).unwrap_or_else(|| "main".to_string());
        let git_branches = git_branches(&root).unwrap_or_else(|| vec![git_branch.clone()]);
        let git_remotes = git_remotes(&root).unwrap_or_default();
        let git_remote_branches = git_remote_branches(&root).unwrap_or_default();
        let git_stashes = git_stashes(&root).unwrap_or_default();
        let git_worktrees = git_worktrees(&root).unwrap_or_default();
        let display_root = display_path(&root);
        let mut files = Vec::new();
        collect_files(&root, &root, &mut files, 0);
        files.sort_by_key(|file| std::cmp::Reverse(file.modified));
        let file_count = files.len();
        let line_count = files.iter().map(|file| file.lines).sum();
        let primary_language = primary_language(&files);
        let rust_edition = rust_edition(&root);
        let recent_files = files
            .iter()
            .take(3)
            .map(|file| RecentFile {
                path: file.path.clone(),
                modified: file.modified,
            })
            .collect::<Vec<_>>();
        let top_files = files
            .iter()
            .filter(|file| visible_top_file(&file.path))
            .take(4)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let workspace_paths = files
            .iter()
            .take(96)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let diagnostics = load_diagnostics(&root);
        let agent_jobs = load_agent_jobs(&root);

        Self {
            root,
            display_root,
            git_branch,
            git_branches,
            git_remotes,
            git_remote_branches,
            git_stashes,
            git_worktrees,
            file_count,
            line_count,
            recent_files,
            top_files,
            workspace_paths,
            diagnostics,
            agent_jobs,
            primary_language,
            rust_edition,
        }
    }

    pub(super) fn refresh_agent_jobs(&mut self) {
        self.agent_jobs = load_agent_jobs(&self.root);
    }

    pub(super) fn fixture() -> Self {
        Self {
            root: PathBuf::from("/tmp/viden"),
            display_root: "~/projects/viden".to_string(),
            git_branch: "main".to_string(),
            git_branches: vec![
                "main".to_string(),
                "codex/tui-cockpit".to_string(),
                "release/v0.1.4".to_string(),
            ],
            git_remotes: vec!["origin".to_string(), "upstream".to_string()],
            git_remote_branches: vec![
                GitRemoteBranchEntry {
                    remote: "origin".to_string(),
                    branch: "main".to_string(),
                },
                GitRemoteBranchEntry {
                    remote: "origin".to_string(),
                    branch: "release/v0.1.4".to_string(),
                },
                GitRemoteBranchEntry {
                    remote: "upstream".to_string(),
                    branch: "main".to_string(),
                },
            ],
            git_stashes: vec![
                GitStashEntry {
                    reference: "stash@{0}".to_string(),
                    summary: "WIP on main: tune cockpit palette".to_string(),
                },
                GitStashEntry {
                    reference: "stash@{1}".to_string(),
                    summary: "On codex/tui-cockpit: checkpoint preview assets".to_string(),
                },
            ],
            git_worktrees: vec![
                GitWorktreeEntry {
                    path: "/tmp/viden".to_string(),
                    branch: Some("main".to_string()),
                },
                GitWorktreeEntry {
                    path: "/tmp/viden/.worktrees/codex-tui-cockpit".to_string(),
                    branch: Some("codex/tui-cockpit".to_string()),
                },
            ],
            file_count: 128,
            line_count: 24_531,
            recent_files: vec![
                RecentFile::fixture("src/config.rs", 0),
                RecentFile::fixture("tests/config_tests.rs", 60),
                RecentFile::fixture("src/lib.rs", 120),
                RecentFile::fixture("src/main.rs", 180),
                RecentFile::fixture("Cargo.toml", 240),
            ],
            top_files: vec![
                "src/".to_string(),
                "tests/".to_string(),
                "Cargo.toml".to_string(),
                "README.md".to_string(),
            ],
            workspace_paths: vec![
                "src/config.rs".to_string(),
                "src/lib.rs".to_string(),
                "src/main.rs".to_string(),
                "tests/config_tests.rs".to_string(),
                "Cargo.toml".to_string(),
                "README.md".to_string(),
            ],
            diagnostics: Vec::new(),
            agent_jobs: Vec::new(),
            primary_language: "Rust".to_string(),
            rust_edition: Some("2024".to_string()),
        }
    }
}

impl RecentFile {
    fn fixture(path: &str, offset_seconds: u64) -> Self {
        Self {
            path: path.to_string(),
            modified: SystemTime::now()
                .checked_sub(Duration::from_secs(offset_seconds))
                .unwrap_or_else(SystemTime::now),
        }
    }
}

pub(super) fn entry_from_event(event: EngineEvent) -> TuiEntry {
    match event {
        EngineEvent::System(text) => TuiEntry {
            label: "system".to_string(),
            body: text,
        },
        EngineEvent::Assistant(text) => TuiEntry {
            label: "assistant".to_string(),
            body: text,
        },
        EngineEvent::ToolCall(text) => TuiEntry {
            label: "tool-call".to_string(),
            body: text,
        },
        EngineEvent::ToolResult { output, .. } => TuiEntry {
            label: "tool-result".to_string(),
            body: output,
        },
        EngineEvent::Command(text) => TuiEntry {
            label: "command".to_string(),
            body: text,
        },
    }
}

pub(super) fn latest_lsp_diagnostics(entries: &[TuiEntry]) -> Option<Vec<String>> {
    entries
        .iter()
        .rev()
        .find_map(|entry| parse_lsp_diagnostics(&entry.body))
}

fn parse_lsp_diagnostics(body: &str) -> Option<Vec<String>> {
    let mut lines = body.lines().skip_while(|line| {
        !line
            .trim_end_matches(':')
            .trim()
            .eq_ignore_ascii_case("LSP diagnostics")
    });
    lines.next()?;

    let mut current_path = None::<String>;
    let mut diagnostics = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "<none>" {
            return Some(Vec::new());
        }
        if !line.starts_with(' ') && trimmed.ends_with(':') {
            current_path = Some(trimmed.trim_end_matches(':').to_string());
            continue;
        }
        if line.starts_with("  ") {
            let rendered = current_path
                .as_ref()
                .map(|path| format!("{path}:{trimmed}"))
                .unwrap_or_else(|| trimmed.to_string());
            diagnostics.push(rendered);
        }
    }
    (!diagnostics.is_empty()).then_some(diagnostics)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    path: String,
    lines: usize,
    modified: SystemTime,
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<FileSnapshot>, depth: usize) {
    if depth > 4 || files.len() > 512 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if should_skip(&name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_files(root, &path, files, depth + 1);
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let lines = count_lines(&path, metadata.len());
            files.push(FileSnapshot {
                path: relative,
                lines,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
}

fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".ref" | ".worktrees" | ".omx" | ".codegraph" | "target" | "node_modules"
    )
}

fn visible_top_file(path: &str) -> bool {
    !path.starts_with('.') && path.split('/').count() <= 2
}

fn primary_language(files: &[FileSnapshot]) -> String {
    let rust_files = files
        .iter()
        .filter(|file| file.path.ends_with(".rs"))
        .count();
    if rust_files > 0 {
        "Rust".to_string()
    } else {
        "mixed".to_string()
    }
}

fn rust_edition(root: &Path) -> Option<String> {
    let content = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("edition = "))
        .map(|value| value.trim_matches('"').to_string())
}

fn count_lines(path: &Path, size: u64) -> usize {
    if size > 256 * 1024 || !looks_text(path) {
        return 0;
    }
    fs::read_to_string(path)
        .map(|content| content.lines().count())
        .unwrap_or(0)
}

fn looks_text(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        extension,
        "rs" | "toml" | "md" | "txt" | "json" | "yaml" | "yml" | "sh" | "lock"
    )
}

fn git_branch(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

fn git_branches(root: &Path) -> Option<Vec<String>> {
    let output = Command::new("git")
        .arg("branch")
        .arg("--format=%(refname:short)")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!branches.is_empty()).then_some(branches)
}

fn git_remotes(root: &Path) -> Option<Vec<String>> {
    let output = Command::new("git")
        .arg("remote")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let remotes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!remotes.is_empty()).then_some(remotes)
}

fn git_remote_branches(root: &Path) -> Option<Vec<GitRemoteBranchEntry>> {
    let output = Command::new("git")
        .arg("branch")
        .arg("-r")
        .arg("--format=%(refname:short)")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // `git branch -r` includes symbolic refs such as
            // `origin/HEAD -> origin/main`; suggestions should target real
            // remote branch names only.
            if line.is_empty() || line.contains(" -> ") {
                return None;
            }
            let (remote, branch) = line.split_once('/')?;
            Some(GitRemoteBranchEntry {
                remote: remote.to_string(),
                branch: branch.to_string(),
            })
        })
        .collect::<Vec<_>>();
    (!branches.is_empty()).then_some(branches)
}

fn git_stashes(root: &Path) -> Option<Vec<GitStashEntry>> {
    let output = Command::new("git")
        .arg("stash")
        .arg("list")
        .arg("--format=%gd%x09%gs")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stashes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (reference, summary) = line.split_once('\t')?;
            let reference = reference.trim();
            if reference.is_empty() {
                return None;
            }
            Some(GitStashEntry {
                reference: reference.to_string(),
                summary: summary.trim().to_string(),
            })
        })
        .collect::<Vec<_>>();
    (!stashes.is_empty()).then_some(stashes)
}

fn git_worktrees(root: &Path) -> Option<Vec<GitWorktreeEntry>> {
    let output = Command::new("git")
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut entries = Vec::new();
    let mut current_path = None::<String>;
    let mut current_branch = None::<String>;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.is_empty() {
            if let Some(path) = current_path.take() {
                entries.push(GitWorktreeEntry {
                    path,
                    branch: current_branch.take(),
                });
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(previous_path) = current_path.replace(path.to_string()) {
                entries.push(GitWorktreeEntry {
                    path: previous_path,
                    branch: current_branch.take(),
                });
            }
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = Some(branch.trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(path) = current_path {
        entries.push(GitWorktreeEntry {
            path,
            branch: current_branch,
        });
    }
    (!entries.is_empty()).then_some(entries)
}

fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        if path.starts_with(home.as_ref()) {
            return path.replacen(home.as_ref(), "~", 1);
        }
    }
    path.to_string()
}

fn load_diagnostics(root: &Path) -> Vec<String> {
    fs::read_to_string(diagnostics_store_path(root))
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn load_agent_jobs(root: &Path) -> Vec<AgentJob> {
    let path = root.join(".viden").join("agents").join("codex-jobs.jsonl");
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut jobs = Vec::<AgentJob>::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Some(job) = parse_agent_job(line) else {
            continue;
        };
        if let Some(existing) = jobs.iter_mut().find(|existing| existing.id == job.id) {
            *existing = job;
        } else {
            jobs.push(job);
        }
    }
    jobs.sort_by_key(|job| job.updated_at);
    jobs
}

fn parse_agent_job(line: &str) -> Option<AgentJob> {
    let log_path = json_string_field(line, "log").map(PathBuf::from);
    let result_path = json_string_field(line, "result").map(PathBuf::from);
    let evidence = agent_job_evidence(log_path.as_deref(), result_path.as_deref());
    Some(AgentJob {
        id: json_string_field(line, "id")?,
        kind: json_string_field(line, "kind")?,
        status: json_string_field(line, "status")?,
        task: json_string_field(line, "task").unwrap_or_default(),
        pid: json_number_field(line, "pid").and_then(|value| value.parse().ok()),
        log_path,
        result_path,
        evidence,
        updated_at: json_number_field(line, "ts")?.parse().ok()?,
    })
}

fn agent_job_evidence(log_path: Option<&Path>, result_path: Option<&Path>) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(result_path) = result_path {
        for line in file_head(result_path, 16) {
            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim();
                if value.is_empty() || value == "unknown" || value == "none" {
                    continue;
                }
                match key.trim() {
                    "thread" => push_unique_evidence(&mut evidence, format!("thread {value}")),
                    "turn" => push_unique_evidence(&mut evidence, format!("turn {value}")),
                    "status" => {
                        push_unique_evidence(&mut evidence, format!("turn status {value}"));
                    }
                    "approvals" => {
                        push_unique_evidence(&mut evidence, format!("approvals {value}"));
                    }
                    "resume" => push_unique_evidence(&mut evidence, format!("resume {value}")),
                    "command" => push_unique_evidence(&mut evidence, format!("command {value}")),
                    "message" => push_unique_evidence(&mut evidence, format!("message {value}")),
                    "changed" | "file" | "files" => {
                        push_unique_evidence(&mut evidence, format!("changed {value}"));
                    }
                    "signals" => {
                        push_unique_evidence(&mut evidence, format!("signals {value}"));
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(log_path) = log_path {
        let log = log_head(log_path, 240).join("\n");
        for (needle, label) in [
            ("thread/resume", "resume available"),
            ("thread/started", "thread started"),
            ("turn/started", "turn started"),
            ("turn/completed", "turn completed"),
            (
                "item/commandExecution/outputDelta",
                "command output captured",
            ),
            ("item/fileChange/outputDelta", "file change captured"),
            ("item/fileChange/patchUpdated", "file patch captured"),
            ("turn/diff/updated", "diff updated"),
            ("fs/changed", "fs changed"),
            ("requestApproval", "approval request captured"),
            ("\"method\":\"error\"", "app-server error captured"),
        ] {
            if log.contains(needle) {
                push_unique_evidence(&mut evidence, label.to_string());
            }
        }
        if let Some(message) = app_server_agent_message(&log) {
            push_unique_evidence(&mut evidence, format!("message {message}"));
        }
    }
    evidence.truncate(16);
    evidence
}

fn push_unique_evidence(evidence: &mut Vec<String>, item: String) {
    if !evidence.iter().any(|existing| existing == &item) {
        evidence.push(item);
    }
}

fn log_head(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(max_lines)
        .map(ToOwned::to_owned)
        .collect()
}

fn app_server_agent_message(log: &str) -> Option<String> {
    log.lines().rev().find_map(|line| {
        if !line.contains("agentMessage") || !line.contains("text") {
            return None;
        }
        let normalized = line.replace("\\\"", "\"");
        json_string_field(&normalized, "text")
            .map(|message| clean_display_fragment(&message, 96))
            .filter(|message| !message.is_empty())
    })
}

fn json_string_field(value: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = value.find(&marker)? + marker.len();
    let rest = value[start..].trim_start().strip_prefix('"')?;
    let mut output = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(output),
            '\\' => match chars.next() {
                Some('n') => output.push('\n'),
                Some('r') => output.push('\r'),
                Some('t') => output.push('\t'),
                Some('"') => output.push('"'),
                Some('\\') => output.push('\\'),
                Some(other) => output.push(other),
                None => return None,
            },
            other => output.push(other),
        }
    }
    None
}

fn json_number_field(value: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = value.find(&marker)? + marker.len();
    let number = value[start..]
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!number.is_empty()).then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use viden_runtime::EngineEvent;

    static TEMP_STATE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn entry_from_event_preserves_command_output() {
        let entry = entry_from_event(EngineEvent::Command("Provider registry:".to_string()));

        assert_eq!(entry.label, "command");
        assert_eq!(entry.body, "Provider registry:");
    }

    #[test]
    fn latest_lsp_diagnostics_extracts_real_rendered_diagnostics() {
        let entries = vec![
            TuiEntry {
                label: "system".to_string(),
                body: "older".to_string(),
            },
            TuiEntry {
                label: "command".to_string(),
                body: "LSP diagnostics:\nsrc/lib.rs:\n  7:2 warning [rust-analyzer/E0308] mismatched types\n".to_string(),
            },
        ];

        let diagnostics = latest_lsp_diagnostics(&entries).expect("diagnostics");

        assert_eq!(
            diagnostics,
            vec!["src/lib.rs:7:2 warning [rust-analyzer/E0308] mismatched types"]
        );
    }

    #[test]
    fn latest_lsp_diagnostics_clears_cache_on_empty_lsp_result() {
        let entries = vec![TuiEntry {
            label: "command".to_string(),
            body: "LSP diagnostics:\n  <none>".to_string(),
        }];

        let diagnostics = latest_lsp_diagnostics(&entries).expect("empty diagnostics");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn workspace_snapshot_loads_persisted_diagnostics_cache() {
        let root = temp_state_root();
        save_diagnostics(
            &root,
            &["src/main.rs:1:2 error [fake/E1] broken value".to_string()],
        )
        .expect("save diagnostics");

        let workspace = WorkspaceSnapshot::load(root);

        assert_eq!(
            workspace.diagnostics,
            vec!["src/main.rs:1:2 error [fake/E1] broken value"]
        );
    }

    #[test]
    fn save_empty_diagnostics_removes_persisted_cache() {
        let root = temp_state_root();
        save_diagnostics(
            &root,
            &["src/main.rs:1:2 error [fake/E1] broken value".to_string()],
        )
        .expect("save diagnostics");
        save_diagnostics(&root, &[]).expect("clear diagnostics");

        let workspace = WorkspaceSnapshot::load(root);

        assert!(workspace.diagnostics.is_empty());
    }

    #[test]
    fn terminal_lane_tsv_loads_legacy_five_field_rows() {
        let lane = TerminalLane::from_tsv("L1\tcodex\tfix tests\tqueued\tmain")
            .expect("legacy lane row should load");

        assert_eq!(lane.progress, 0);
        assert_eq!(lane.summary, "restored from lane store");
    }

    #[test]
    fn terminal_lane_tsv_round_trips_progress_and_summary() {
        let mut lane = TerminalLane::preview_lanes()
            .into_iter()
            .next()
            .expect("preview lane");
        lane.worktree = Some(PathBuf::from("/tmp/viden-lane"));
        let loaded = TerminalLane::from_tsv(&lane.to_tsv()).expect("lane row should load");

        assert_eq!(loaded.progress, 64);
        assert_eq!(loaded.summary, "patched failing tests; rerunning cargo");
        assert_eq!(loaded.worktree, Some(PathBuf::from("/tmp/viden-lane")));
    }

    #[test]
    fn terminal_lane_tsv_sanitizes_control_sequences_in_summary() {
        let lane = TerminalLane {
            id: "L1".to_string(),
            tool: "run".to_string(),
            title: "printf ok".to_string(),
            status: "attached".to_string(),
            target: "tmux session".to_string(),
            progress: 35,
            summary: "\u{1b}]697;PreExec\u{7}\u{1b}[31mold\rvisible\u{8}!".to_string(),
            worktree: None,
        };

        let loaded = TerminalLane::from_tsv(&lane.to_tsv()).expect("lane row should load");

        assert_eq!(loaded.summary, "visibl!");
        assert!(!loaded.summary.contains('\u{1b}'));
        assert!(!loaded.summary.contains('\u{7}'));
    }

    #[test]
    fn refresh_lane_runtime_updates_attached_lane_from_log_tail() {
        let root = temp_state_root();
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".viden").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "tmux booted\nlive pane output\n",
        )
        .expect("runtime log");
        let mut lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "claude".to_string(),
            title: "review interactively".to_string(),
            status: "attached".to_string(),
            target: "tmux viden-session-l1".to_string(),
            progress: 10,
            summary: "tmux session ready".to_string(),
            worktree: None,
        }];

        refresh_lane_runtime(&lane_store, &mut lanes);

        assert_eq!(lanes[0].status, "attached");
        assert_eq!(lanes[0].summary, "live pane output");
        assert_eq!(lanes[0].progress, 35);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_lane_runtime_sanitizes_tmux_log_tail() {
        let root = temp_state_root();
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".viden").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.log"),
            "printf smoke\r\u{1b}]697;PreExec\u{7}\u{1b}[32msmoke-ok\u{1b}[0m\n\u{1b}[01;32m➜  \u{1b}[36mwork\u{1b}[00m \n",
        )
        .expect("runtime log");
        let mut lanes = vec![TerminalLane {
            id: "L1".to_string(),
            tool: "run".to_string(),
            title: "printf smoke".to_string(),
            status: "attached".to_string(),
            target: "tmux viden-session-l1".to_string(),
            progress: 10,
            summary: "tmux session ready".to_string(),
            worktree: None,
        }];

        refresh_lane_runtime(&lane_store, &mut lanes);

        assert_eq!(lanes[0].summary, "smoke-ok");
        assert!(!lanes[0].summary.contains('\u{1b}'));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn companion_screen_store_round_trips_registry_rows() {
        let root = temp_state_root();
        let path = screen_store_path(&root);
        let screens = vec![CompanionScreen {
            id: "side-1".to_string(),
            title: "Agent lanes".to_string(),
            status: "launched".to_string(),
            pid: Some(4242),
            summary: "provider=deepseek model=deepseek-v4-flash".to_string(),
        }];

        save_screens(&path, &screens).expect("save screens");
        let loaded = load_screens(&path);

        assert_eq!(loaded, screens);
    }

    #[test]
    fn workspace_snapshot_loads_latest_codex_agent_jobs() {
        let root = temp_state_root();
        let agents = root.join(".viden").join("agents");
        fs::create_dir_all(&agents).expect("agent dir");
        let codex_2_log = agents.join("codex-2.jsonl");
        let codex_2_result = agents.join("codex-2.result.md");
        fs::write(
            &codex_2_log,
            r#"{"direction":"server","payload":"{\"method\":\"turn/started\"}"}"#,
        )
        .expect("codex log");
        fs::write(
            &codex_2_result,
            "# Codex app-server turn\n\nthread: thread_2\nturn: turn_2\nstatus: completed\n",
        )
        .expect("codex result");
        fs::write(
            agents.join("codex-jobs.jsonl"),
            format!(
                "{}\n{}\n{}\n",
                r#"{"ts":10,"event":"started","id":"codex-1","kind":"run","status":"running","pid":4242,"command":"codex exec","task":"first task","log":"a","result":"b"}"#,
                r#"{"ts":20,"event":"completed","id":"codex-1","kind":"run","status":"finished","pid":4242,"command":"codex exec","task":"first task done","log":"a","result":"b"}"#,
                format_args!(
                    r#"{{"ts":30,"event":"started","id":"codex-2","kind":"review","status":"running","pid":5252,"command":"codex review","task":"review diff","log":"{}","result":"{}"}}"#,
                    codex_2_log.display(),
                    codex_2_result.display()
                )
            ),
        )
        .expect("jobs jsonl");

        let workspace = WorkspaceSnapshot::load(root);

        assert_eq!(workspace.agent_jobs.len(), 2);
        assert_eq!(workspace.agent_jobs[0].id, "codex-1");
        assert_eq!(workspace.agent_jobs[0].status, "finished");
        assert_eq!(workspace.agent_jobs[1].id, "codex-2");
        assert_eq!(workspace.agent_jobs[1].status, "running");
        assert_eq!(workspace.agent_jobs[1].task, "review diff");
        assert_eq!(
            workspace.agent_jobs[1].evidence,
            vec![
                "thread thread_2".to_string(),
                "turn turn_2".to_string(),
                "turn status completed".to_string(),
                "turn started".to_string()
            ]
        );
    }

    #[test]
    fn workspace_snapshot_loads_app_server_command_file_approval_and_resume_evidence() {
        let root = temp_state_root();
        let agents = root.join(".viden").join("agents");
        fs::create_dir_all(&agents).expect("agent dir");
        let log_path = agents.join("codex-app-server.jsonl");
        let result_path = agents.join("codex-app-server.result.md");
        fs::write(
            &log_path,
            [
                r#"{"direction":"server","payload":"{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thread_app\"}}}"}"#,
                r#"{"direction":"server","payload":"{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thread_app\",\"turn\":{\"id\":\"turn_app\"}}}"}"#,
                r#"{"direction":"server","payload":"{\"method\":\"item/commandExecution/outputDelta\",\"params\":{\"command\":\"cargo test\",\"delta\":\"running\"}}"}"#,
                r#"{"direction":"server","payload":"{\"method\":\"item/fileChange/patchUpdated\",\"params\":{\"path\":\"src/config.rs\"}}"}"#,
                r#"{"direction":"server","payload":"{\"id\":9,\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"command\":\"cargo test\"}}"}"#,
                r#"{"direction":"server","payload":"{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"agentMessage\",\"text\":\"VIDEN_APP_SERVER_SMOKE_OK\"}}}"}"#,
                r#"{"direction":"server","payload":"{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread_app\",\"turn\":{\"id\":\"turn_app\",\"status\":\"completed\"}}}"}"#,
            ]
            .join("\n"),
        )
        .expect("codex app-server log");
        fs::write(
            &result_path,
            "# Codex app-server turn\n\nthread: thread_app\nturn: turn_app\nstatus: completed\nresume: thread_app\nmessage: VIDEN_APP_SERVER_SMOKE_OK\napprovals: item/commandExecution/requestApproval\nsignals: command-output, file-patch\n",
        )
        .expect("codex app-server result");
        fs::write(
            agents.join("codex-jobs.jsonl"),
            format!(
                r#"{{"ts":40,"event":"completed","id":"codex-app","kind":"app-server-turn","status":"finished","pid":6262,"command":"codex app-server turn/start","task":"run tests","log":"{}","result":"{}"}}"#,
                log_path.display(),
                result_path.display()
            ),
        )
        .expect("jobs jsonl");

        let workspace = WorkspaceSnapshot::load(root);
        let job = workspace.agent_jobs.first().expect("agent job");

        assert_eq!(job.evidence[0], "thread thread_app");
        assert!(job.evidence.contains(&"turn turn_app".to_string()));
        assert!(job.evidence.contains(&"turn status completed".to_string()));
        assert!(job.evidence.contains(&"resume thread_app".to_string()));
        assert!(
            job.evidence
                .contains(&"command output captured".to_string())
        );
        assert!(job.evidence.contains(&"file patch captured".to_string()));
        assert!(
            job.evidence
                .contains(&"approval request captured".to_string())
        );
        assert!(
            job.evidence
                .contains(&"message VIDEN_APP_SERVER_SMOKE_OK".to_string())
        );
        assert!(
            job.evidence
                .contains(&"signals command-output, file-patch".to_string())
        );

        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace,
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };
        let tasks = agent_tasks(&state);
        let task = tasks
            .iter()
            .find(|task| task.id == "codex-app")
            .expect("task");

        assert_eq!(task.transport, "app-server");
        assert_eq!(task.resume_handle, Some("thread_app".to_string()));
        assert!(task.evidence.contains(&"file patch captured".to_string()));
        assert!(
            task.evidence
                .contains(&"command output captured".to_string())
        );
        assert!(
            task.evidence
                .contains(&"signals command-output, file-patch".to_string())
        );
    }

    #[test]
    fn agent_tasks_unify_lanes_and_codex_jobs() {
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.agent_jobs = vec![AgentJob {
            id: "codex-1".to_string(),
            kind: "run".to_string(),
            status: "running".to_string(),
            task: "review diff".to_string(),
            pid: Some(4242),
            log_path: None,
            result_path: None,
            evidence: vec!["thread thread_1".to_string()],
            updated_at: 123,
        }];
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace,
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: vec![TerminalLane {
                id: "L1".to_string(),
                tool: "claude".to_string(),
                title: "review config".to_string(),
                status: "attached".to_string(),
                target: "tmux viden-l1".to_string(),
                progress: 40,
                summary: "reviewing config architecture".to_string(),
                worktree: None,
            }],
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "L1");
        assert_eq!(tasks[0].agent, "claude");
        assert_eq!(tasks[0].kind, "lane");
        assert_eq!(tasks[0].transport, "tmux");
        assert_eq!(tasks[0].status, "needs_input");
        assert!(tasks[0].is_active());
        assert_eq!(tasks[1].id, "codex-1");
        assert_eq!(tasks[1].agent, "codex");
        assert_eq!(tasks[1].kind, "job");
        assert_eq!(tasks[1].transport, "app-server");
        assert_eq!(tasks[1].status, "thinking");
        assert_eq!(tasks[1].activity, "thread thread_1");
    }

    #[test]
    fn agent_tasks_project_acp_session_jobs_as_acp_agents() {
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.agent_jobs = vec![AgentJob {
            id: "acp-1".to_string(),
            kind: "acp-session".to_string(),
            status: "running".to_string(),
            task: "review architecture".to_string(),
            pid: Some(5151),
            log_path: None,
            result_path: None,
            evidence: vec!["session session_1".to_string()],
            updated_at: 456,
        }];
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace,
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "acp-1");
        assert_eq!(tasks[0].agent, "acp");
        assert_eq!(tasks[0].transport, "acp");
        assert_eq!(tasks[0].status, "thinking");
        assert_eq!(
            tasks[0].permissions,
            vec!["agent permission gated".to_string()]
        );
        assert_eq!(
            tasks[0]
                .next_action
                .as_ref()
                .and_then(|action| action.command.as_deref()),
            Some("/agent result acp-1")
        );
    }

    #[test]
    fn agent_tasks_project_lane_apply_and_conflict_artifacts() {
        let root = temp_state_root();
        let lane_store = lane_store_path(&root);
        let artifact_dir = root.join(".viden").join("lanes");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(
            artifact_dir.join("L1.apply.md"),
            [
                "# Viden Lane Apply",
                "",
                "Patch: /tmp/L1.apply.patch",
                "",
                "## Workspace changed files after apply",
                "M src/config.rs",
                "A tests/config_tests.rs",
            ]
            .join("\n"),
        )
        .expect("apply artifact");
        fs::write(
            artifact_dir.join("L2.apply-conflict.md"),
            [
                "# Viden Lane Apply Conflict",
                "",
                "Patch: /tmp/L2.apply.patch",
                "",
                "## Direct apply check",
                "error: patch failed: src/config.rs:42",
                "",
                "## Lane worktree changed files",
                "M src/config.rs",
            ]
            .join("\n"),
        )
        .expect("conflict artifact");
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: vec![
                TerminalLane {
                    id: "L1".to_string(),
                    tool: "codex".to_string(),
                    title: "apply config loader".to_string(),
                    status: "applied".to_string(),
                    target: "main".to_string(),
                    progress: 100,
                    summary: "applied patch /tmp/L1.apply.patch; cleanup remains separate"
                        .to_string(),
                    worktree: Some(root.join(".worktrees").join("L1")),
                },
                TerminalLane {
                    id: "L2".to_string(),
                    tool: "claude".to_string(),
                    title: "review config loader".to_string(),
                    status: "apply_conflict".to_string(),
                    target: "main".to_string(),
                    progress: 100,
                    summary: "apply conflict; report /tmp/L2.apply-conflict.md".to_string(),
                    worktree: Some(root.join(".worktrees").join("L2")),
                },
            ],
            lane_store: Some(lane_store),
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);
        let applied = tasks.iter().find(|task| task.id == "L1").expect("L1 task");
        let conflict = tasks.iter().find(|task| task.id == "L2").expect("L2 task");

        assert_eq!(applied.status, "done");
        assert!(
            applied
                .evidence
                .contains(&"patch /tmp/L1.apply.patch".to_string())
        );
        assert!(
            applied
                .evidence
                .contains(&"changed M src/config.rs".to_string())
        );
        assert!(
            conflict
                .evidence
                .contains(&"conflict error: patch failed: src/config.rs:42".to_string())
        );
        assert!(
            conflict
                .evidence
                .contains(&"changed M src/config.rs".to_string())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn agent_tasks_separate_completed_accept_and_accepted_apply_lane_actions() {
        let root = temp_state_root();
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: vec![
                TerminalLane {
                    id: "L1".to_string(),
                    tool: "codex".to_string(),
                    title: "implement config loader".to_string(),
                    status: "completed".to_string(),
                    target: "main".to_string(),
                    progress: 100,
                    summary: "lane worktree has reviewable changes".to_string(),
                    worktree: Some(root.join(".worktrees").join("L1")),
                },
                TerminalLane {
                    id: "L2".to_string(),
                    tool: "claude".to_string(),
                    title: "apply accepted cleanup".to_string(),
                    status: "accepted".to_string(),
                    target: "main".to_string(),
                    progress: 100,
                    summary: "operator accepted lane changes".to_string(),
                    worktree: Some(root.join(".worktrees").join("L2")),
                },
            ],
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);
        let completed = tasks.iter().find(|task| task.id == "L1").expect("L1 task");
        let accepted = tasks.iter().find(|task| task.id == "L2").expect("L2 task");

        let completed_action = completed.next_action.as_ref().expect("completed action");
        assert_eq!(completed_action.label, "accept lane");
        assert_eq!(completed_action.command.as_deref(), Some("/lane accept L1"));
        assert_eq!(
            completed_action.reason.as_deref(),
            Some("isolated lane needs operator acceptance before apply")
        );

        let accepted_action = accepted.next_action.as_ref().expect("accepted action");
        assert_eq!(accepted_action.label, "apply lane");
        assert_eq!(accepted_action.command.as_deref(), Some("/lane apply L2"));
        assert_eq!(
            accepted_action.reason.as_deref(),
            Some("accepted isolated lane has reviewable changes")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn agent_tasks_surface_pending_turn_without_duplicate_provider_task() {
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.display_root = "/tmp/project".to_string();
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: Some(PendingTurn {
                id: "turn-session-42".to_string(),
                provider: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                prompt: "create hello world".to_string(),
                workspace: "/tmp/project".to_string(),
                started_at: 42,
                phase: "Waiting for provider response".to_string(),
                next_action: "wait".to_string(),
                queued_inputs: vec![
                    "then run tests".to_string(),
                    "summarize the diff".to_string(),
                ],
            }),
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![TuiEntry {
                label: "user".to_string(),
                body: "create hello world".to_string(),
            }],
            workspace,
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "turn-session-42");
        assert_eq!(tasks[0].status, "thinking");
        assert_eq!(tasks[0].transport, "deepseek");
        assert_eq!(
            tasks[0].summary,
            "Viden is processing the request · 2 prompts queued"
        );
        assert!(
            tasks[0]
                .evidence
                .contains(&"live provider request".to_string())
        );
        assert!(tasks[0].evidence.contains(&"queued_inputs 2".to_string()));
        assert_eq!(
            tasks[0].next_action.as_ref().expect("next action").label,
            "wait · 2 prompts queued"
        );
    }

    #[test]
    fn agent_tasks_do_not_keep_failed_provider_turn_active() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![
                TuiEntry {
                    label: "user".to_string(),
                    body: "plan an embodied AI project".to_string(),
                },
                TuiEntry {
                    label: "system".to_string(),
                    body: "Provider turn failed, but Viden kept the TUI open.\nerror: Argument list too long (os error 7)".to_string(),
                },
            ],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert!(tasks.iter().all(|task| task.kind != "provider"));
    }

    #[test]
    fn agent_tasks_project_transcript_runtime_events() {
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.display_root = "/tmp/project".to_string();
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![
                TuiEntry {
                    label: "user".to_string(),
                    body: "create hello world".to_string(),
                },
                TuiEntry {
                    label: "tool-call".to_string(),
                    body: "write_file path: hello.py lines: 1-2".to_string(),
                },
                TuiEntry {
                    label: "approval".to_string(),
                    body: "Permission request for `write_file`\npath: hello.py\nPress y to allow, n/Esc to deny.".to_string(),
                },
            ],
            workspace,
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks[0].kind, "approval");
        assert_eq!(tasks[0].status, "waiting_approval");
        assert_eq!(tasks[0].permissions, vec!["write_file".to_string()]);
        assert!(
            tasks[0]
                .evidence
                .contains(&"transcript approval".to_string())
        );
        assert_eq!(tasks[1].kind, "tool");
        assert_eq!(tasks[1].status, "editing");
        assert_eq!(tasks[1].activity, "Editing hello.py");
        assert!(tasks[1].evidence.contains(&"path hello.py".to_string()));
        assert!(tasks[1].evidence.contains(&"lines 1-2".to_string()));
    }

    #[test]
    fn agent_tasks_do_not_keep_stale_approval_after_closure_event() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![
                TuiEntry {
                    label: "approval".to_string(),
                    body: "Permission request for `write_file`\npath: hello.py\nPress y to allow, n/Esc to deny.".to_string(),
                },
                TuiEntry {
                    label: "approval".to_string(),
                    body: "Approved `write_file`.".to_string(),
                },
                TuiEntry {
                    label: "tool-result".to_string(),
                    body: "write_file completed\npath=hello.py\nWrote 2 lines to hello.py".to_string(),
                },
            ],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert!(tasks.iter().all(|task| task.kind != "approval"));
        assert_eq!(tasks[0].kind, "tool");
        assert_eq!(tasks[0].status, "done");
        assert!(tasks[0].evidence.contains(&"path hello.py".to_string()));
    }

    #[test]
    fn agent_tasks_project_test_result_evidence() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![TuiEntry {
                label: "command".to_string(),
                body: [
                    "Test result:",
                    "  status: failed",
                    "  exit code: 101",
                    "  command: cargo test -p viden-cli",
                    "  duration: 42ms",
                    "  failure summary:",
                    "    - assertion failed in ops_screen",
                    "  failing files:",
                    "    - src/tui/ops_screen.rs:42:9",
                    "  output tail:",
                    "    thread 'ops_screen' panicked at src/tui/ops_screen.rs:42:9",
                ]
                .join("\n"),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks[0].kind, "test");
        assert_eq!(tasks[0].agent, "shell");
        assert_eq!(tasks[0].status, "failed");
        assert_eq!(tasks[0].summary, "failed in 42ms");
        assert!(tasks[0].evidence.contains(&"status failed".to_string()));
        assert!(
            tasks[0]
                .evidence
                .contains(&"command cargo test -p viden-cli".to_string())
        );
        assert!(
            tasks[0]
                .evidence
                .contains(&"failure assertion failed in ops_screen".to_string())
        );
        assert!(
            tasks[0]
                .evidence
                .contains(&"failing-file src/tui/ops_screen.rs:42:9".to_string())
        );
        assert!(tasks[0].evidence.contains(
            &"tail thread 'ops_screen' panicked at src/tui/ops_screen.rs:42:9".to_string()
        ));
        assert!(
            tasks[0]
                .evidence
                .contains(&"rerun cargo test -p viden-cli".to_string())
        );
    }

    #[test]
    fn agent_tasks_project_diff_review_evidence() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![TuiEntry {
                label: "command".to_string(),
                body: [
                    "Latest diff:",
                    "  Summary: files=2 additions=12 deletions=3",
                    "",
                    "Diff:",
                    "diff --git a/src/config.rs b/src/config.rs",
                    "--- a/src/config.rs",
                    "+++ b/src/config.rs",
                    "@@",
                    "-old",
                    "+new",
                    "diff --git a/tests/config_tests.rs b/tests/config_tests.rs",
                ]
                .join("\n"),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks[0].kind, "diff");
        assert_eq!(tasks[0].status, "needs_input");
        assert_eq!(tasks[0].activity, "review diff: 2 file(s) +12 -3");
        assert!(tasks[0].is_active());
        assert!(tasks[0].evidence.contains(&"files 2".to_string()));
        assert!(tasks[0].evidence.contains(&"additions 12".to_string()));
        assert!(tasks[0].evidence.contains(&"deletions 3".to_string()));
        assert!(
            tasks[0]
                .evidence
                .contains(&"path src/config.rs".to_string())
        );
        assert_eq!(tasks[0].resume_handle, Some("/diff".to_string()));
    }

    #[test]
    fn agent_tasks_keep_recent_diff_test_tool_and_provider_entries() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![
                TuiEntry {
                    label: "assistant".to_string(),
                    body: "I patched the parser.".to_string(),
                },
                TuiEntry {
                    label: "tool-result".to_string(),
                    body: "write_file completed\npath=src/config.rs".to_string(),
                },
                TuiEntry {
                    label: "command".to_string(),
                    body: "Test result:\n  status: passed\n  command: cargo test\n  duration: 1s"
                        .to_string(),
                },
                TuiEntry {
                    label: "command".to_string(),
                    body: "Latest diff:\n  Summary: files=1 additions=2 deletions=0\n\nDiff:\ndiff --git a/src/config.rs b/src/config.rs".to_string(),
                },
            ],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert!(tasks.iter().any(|task| task.kind == "diff"));
        assert!(tasks.iter().any(|task| task.kind == "test"));
        assert!(tasks.iter().any(|task| task.kind == "tool"));
        assert!(tasks.iter().any(|task| task.kind == "provider"));
    }

    #[test]
    fn agent_tasks_project_tool_result_structured_evidence() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: vec![TuiEntry {
                label: "tool-result".to_string(),
                body: "Wrote 48 lines to src/config.rs (2.1 KB)\npath=src/config.rs".to_string(),
            }],
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let tasks = agent_tasks(&state);

        assert_eq!(tasks[0].kind, "tool");
        assert_eq!(tasks[0].status, "done");
        assert!(
            tasks[0]
                .evidence
                .contains(&"path src/config.rs".to_string())
        );
        assert!(tasks[0].evidence.contains(&"lines 48".to_string()));
    }

    fn temp_state_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let counter = TEMP_STATE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("viden-tui-state-test-{nanos}-{counter}"));
        fs::create_dir_all(&root).expect("temp root");
        root
    }
}
