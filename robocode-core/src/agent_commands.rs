use std::{
    collections::HashSet,
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{SessionEngine, presentation::render_permission_denial};
use robocode_permissions::PermissionEngine;
use robocode_types::{
    ApprovalResponse, PermissionDecision, PermissionLogEntry, ToolInput, ToolSpec, TranscriptEntry,
    now_timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AgentAdapterDescriptor {
    id: &'static str,
    display_name: &'static str,
    transport: &'static str,
    entrypoint: &'static str,
    binary: Option<&'static str>,
    config_env: Option<&'static str>,
    config_label: &'static str,
    config_required: bool,
}

const AGENT_ADAPTERS: [AgentAdapterDescriptor; 6] = [
    AgentAdapterDescriptor {
        id: "codex",
        display_name: "Codex CLI",
        transport: "app-server",
        entrypoint: "/agent run codex [--write] <task> | /lane codex <task>",
        binary: Some("codex"),
        config_env: Some("ROBOCODE_LANE_CODEX_TEMPLATE"),
        config_label: "template",
        config_required: false,
    },
    AgentAdapterDescriptor {
        id: "claude",
        display_name: "Claude Code",
        transport: "template",
        entrypoint: "/lane claude <task>",
        binary: Some("claude"),
        config_env: Some("ROBOCODE_LANE_CLAUDE_TEMPLATE"),
        config_label: "template",
        config_required: true,
    },
    AgentAdapterDescriptor {
        id: "custom-template",
        display_name: "Custom template agent",
        transport: "template",
        entrypoint: "/lane ask <tool> <task>",
        binary: None,
        config_env: Some("ROBOCODE_LANE_<TOOL>_TEMPLATE"),
        config_label: "template",
        config_required: false,
    },
    AgentAdapterDescriptor {
        id: "tmux",
        display_name: "Tmux lane",
        transport: "tmux",
        entrypoint: "/lane tmux <lane-id>",
        binary: Some("tmux"),
        config_env: None,
        config_label: "template",
        config_required: false,
    },
    AgentAdapterDescriptor {
        id: "pty",
        display_name: "Embedded PTY lane",
        transport: "pty",
        entrypoint: "/lane pty <lane-id>",
        binary: pty_binary(),
        config_env: Some("ROBOCODE_LANE_PTY_TEMPLATE"),
        config_label: "template",
        config_required: false,
    },
    AgentAdapterDescriptor {
        id: "acp",
        display_name: "ACP agent server",
        transport: "acp",
        entrypoint: "/lane acp <agent> <task> (experimental)",
        binary: None,
        config_env: Some("ROBOCODE_AGENT_ACP_COMMAND"),
        config_label: "command",
        config_required: true,
    },
];

impl SessionEngine {
    pub(super) fn handle_agent_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(render_agent_list()),
            "doctor" => Ok(render_agent_doctor(
                args.get(1).map(String::as_str),
                &self.cwd,
            )),
            "review" => handle_codex_review_command(&self.cwd, &args[1..]),
            "challenge" => handle_codex_challenge_command(&self.cwd, &args[1..]),
            "probe" => handle_codex_probe_command(&self.cwd, &args[1..]),
            "run" => self.handle_codex_run_command(&args[1..], approver),
            "status" => render_codex_job_status(&self.cwd),
            "result" => render_codex_job_result(&self.cwd, args.get(1).map(String::as_str)),
            "cancel" => cancel_codex_job(&self.cwd, args.get(1).map(String::as_str)),
            "logs" => Ok(render_agent_logs_help()),
            subcommand => Ok(format!(
                "Unknown agent subcommand `{subcommand}`.\n\n{}",
                self.render_agent_help()
            )),
        }
    }

    pub(super) fn render_agent_help(&self) -> String {
        [
            "Agent commands:",
            "  /agent list",
            "  /agent doctor [id]",
            "  /agent review codex [--base <ref>] [prompt]",
            "  /agent challenge codex [prompt]",
            "  /agent probe codex [--thread|--turn <task>]",
            "  /agent run codex [--write|--app-server] <task>",
            "  /agent status",
            "  /agent result <id>",
            "  /agent cancel <id>",
            "  /agent logs <id>",
            "",
            "Agent commands start and inspect tracked external agent jobs. Use `/lane ...` for terminal lane orchestration.",
        ]
        .join("\n")
    }

    fn handle_codex_run_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        ensure_codex_target(args.first().map(String::as_str))?;
        let parsed = parse_codex_run_args(&args[1..])?;
        if parsed.task.trim().is_empty() {
            return Err("Usage: /agent run codex [--write|--app-server] <task>".to_string());
        }
        if parsed.app_server && parsed.write {
            return Err(
                "`--app-server` currently supports read-only delegated tasks only.".to_string(),
            );
        }
        if parsed.app_server {
            return start_codex_app_server_job(&self.cwd, &codex_command(), parsed.task.clone());
        }
        if parsed.write {
            if let Some(denial) = self.ensure_codex_write_permission(&parsed.task, approver)? {
                return Ok(denial);
            }
        }
        let sandbox = if parsed.write {
            "workspace-write"
        } else {
            "read-only"
        };
        start_codex_job(
            &self.cwd,
            &codex_command(),
            CodexJobKind::Run,
            parsed.task.clone(),
            codex_run_command_args(&self.cwd, sandbox, parsed.task),
        )
    }

    fn ensure_codex_write_permission<F>(
        &mut self,
        task: &str,
        approver: &mut F,
    ) -> Result<Option<String>, String>
    where
        F: FnMut(robocode_types::PermissionPrompt) -> ApprovalResponse,
    {
        let tool_name = "agent_codex_write".to_string();
        let tool = ToolSpec {
            name: tool_name.clone(),
            description: "Start a write-capable Codex delegated task".to_string(),
            is_mutating: true,
            input_schema_hint: "agent task".to_string(),
        };
        let mut input = ToolInput::new();
        input.insert("agent".to_string(), "codex".to_string());
        input.insert("mode".to_string(), "workspace-write".to_string());
        input.insert("cwd".to_string(), self.cwd.display().to_string());
        input.insert("task".to_string(), task.to_string());
        let mut decision = self.permissions.decide(&tool, &input);
        if let PermissionDecision::Ask(ask) = &decision {
            let prompt = PermissionEngine::prompt_for(&tool_name, ask, &input);
            let approval = approver(prompt);
            decision = self.permissions.apply_approval(approval, ask);
        }
        match decision {
            PermissionDecision::Allow(allow) => {
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name,
                        decision: "allow".to_string(),
                        reason: format!("{:?}", allow.decision_reason),
                        message: allow.accept_feedback,
                    },
                })?;
                Ok(None)
            }
            PermissionDecision::Ask(_) => unreachable!("ask decisions should be resolved"),
            PermissionDecision::Deny(deny) => {
                let reason = format!("{:?}", deny.decision_reason);
                self.store_entry(TranscriptEntry::Permission {
                    entry: PermissionLogEntry {
                        timestamp: now_timestamp(),
                        tool_name: tool_name.clone(),
                        decision: "deny".to_string(),
                        reason: reason.clone(),
                        message: Some(deny.message.clone()),
                    },
                })?;
                Ok(Some(render_permission_denial(
                    &tool_name,
                    &reason,
                    &deny.message,
                )))
            }
        }
    }
}

fn codex_run_command_args(cwd: &Path, sandbox: &str, task: String) -> Vec<String> {
    vec![
        "exec".to_string(),
        "--cd".to_string(),
        cwd.display().to_string(),
        "--sandbox".to_string(),
        sandbox.to_string(),
        task,
    ]
}

fn render_agent_list() -> String {
    let mut lines = vec![
        "Agent adapters:".to_string(),
        "  id               transport  readiness      entrypoint".to_string(),
    ];
    lines.extend(AGENT_ADAPTERS.into_iter().map(|adapter| {
        format!(
            "  {:<16} {:<10} {:<14} {}",
            adapter.id,
            adapter.transport,
            adapter_readiness(adapter),
            adapter.entrypoint
        )
    }));
    lines.push(String::new());
    lines.push("Use `/agent doctor [id]` for binary, template, and transport details.".to_string());
    lines.join("\n")
}

fn render_agent_doctor(target: Option<&str>, cwd: &Path) -> String {
    let adapters = if let Some(id) = target {
        let Some(adapter) = AGENT_ADAPTERS.into_iter().find(|adapter| adapter.id == id) else {
            return format!(
                "Unknown agent adapter `{id}`. Use `/agent list` to see known adapters."
            );
        };
        vec![adapter]
    } else {
        AGENT_ADAPTERS.to_vec()
    };
    let mut lines = vec!["Agent diagnostics:".to_string()];
    for adapter in adapters {
        lines.push(format!("  {} ({})", adapter.id, adapter.display_name));
        lines.push(format!("    transport: {}", adapter.transport));
        lines.push(format!("    entrypoint: {}", adapter.entrypoint));
        match adapter.binary {
            Some(binary) => lines.push(format!(
                "    binary: {} ({binary})",
                if command_exists(binary) {
                    "ok"
                } else {
                    "missing"
                }
            )),
            None if adapter.id == "acp" => lines.push(
                "    binary: resolved from ROBOCODE_AGENT_ACP_COMMAND when configured".to_string(),
            ),
            None => {
                lines.push("    binary: not required until a custom tool is selected".to_string())
            }
        }
        match adapter.config_env {
            Some(env_key) if env_key.contains("<TOOL>") => lines.push(format!(
                "    {}: dynamic ({env_key}; resolved from `/lane ask <tool> ...`)",
                adapter.config_label
            )),
            Some(env_key) if adapter.config_required => lines.push(format!(
                "    {}: {} ({env_key})",
                adapter.config_label,
                if env_is_configured(env_key) {
                    "configured"
                } else {
                    "missing"
                }
            )),
            Some(env_key) => lines.push(format!(
                "    {}: optional override ({env_key})",
                adapter.config_label
            )),
            None => lines.push(format!("    {}: not required", adapter.config_label)),
        }
        if adapter.id == "acp" {
            lines.extend(render_acp_probe(cwd));
        } else if adapter.id == "codex" {
            lines.extend(render_codex_doctor(cwd));
        }
    }
    lines.join("\n")
}

fn render_agent_logs_help() -> String {
    [
        "Agent logs:",
        "  /agent result <id> shows tracked Codex job output.",
        "  Use `/lane inspect <id>` for lane logs, artifacts, decisions, and transport evidence.",
    ]
    .join("\n")
}

fn handle_codex_review_command(cwd: &Path, args: &[String]) -> Result<String, String> {
    ensure_codex_target(args.first().map(String::as_str))?;
    let parsed = parse_codex_review_args(&args[1..])?;
    let mut command_args = vec!["review".to_string(), "--uncommitted".to_string()];
    if let Some(base) = parsed.base {
        command_args.extend(["--base".to_string(), base]);
    }
    if !parsed.prompt.is_empty() {
        command_args.push(parsed.prompt.clone());
    }
    start_codex_job(
        cwd,
        &codex_command(),
        CodexJobKind::Review,
        if parsed.prompt.is_empty() {
            "Review current working tree".to_string()
        } else {
            parsed.prompt
        },
        command_args,
    )
}

fn handle_codex_challenge_command(cwd: &Path, args: &[String]) -> Result<String, String> {
    ensure_codex_target(args.first().map(String::as_str))?;
    let prompt = args[1..].join(" ");
    let challenge_prompt = if prompt.trim().is_empty() {
        "Run an adversarial code review. Challenge assumptions, look for regressions, missing tests, and unsafe implementation shortcuts.".to_string()
    } else {
        format!(
            "Run an adversarial code review focused on this request: {}. Challenge assumptions, look for regressions, missing tests, and unsafe implementation shortcuts.",
            prompt.trim()
        )
    };
    start_codex_job(
        cwd,
        &codex_command(),
        CodexJobKind::Challenge,
        challenge_prompt.clone(),
        vec![
            "review".to_string(),
            "--uncommitted".to_string(),
            challenge_prompt,
        ],
    )
}

fn handle_codex_probe_command(cwd: &Path, args: &[String]) -> Result<String, String> {
    ensure_codex_target(args.first().map(String::as_str))?;
    let mode = parse_codex_probe_args(&args[1..])?;
    let evidence = run_codex_app_server_probe(cwd, &codex_command(), mode.clone())?;
    let tracked_job = if let CodexProbeMode::Turn(task) = &mode {
        Some(record_codex_app_server_turn_probe(cwd, task, &evidence)?)
    } else {
        None
    };
    let notification_count = evidence.notifications.len();
    let notifications = if evidence.notifications.is_empty() {
        "none".to_string()
    } else {
        evidence.notifications.join(", ")
    };
    let thread = evidence
        .thread_id
        .as_ref()
        .map(|thread_id| format!("  thread: {thread_id}\n"))
        .unwrap_or_default();
    let turn = evidence
        .turn_id
        .as_ref()
        .map(|turn_id| {
            let status = evidence.turn_status.as_deref().unwrap_or("unknown");
            format!("  turn: {turn_id} ({status})\n")
        })
        .unwrap_or_default();
    let tracked = tracked_job
        .as_ref()
        .map(|id| format!("  tracked_job: {id}\n"))
        .unwrap_or_default();
    Ok(format!(
        "Codex app-server probe ok.\n  user_agent: {}\n  codex_home: {}\n  platform: {}\n{}{}{}  notifications: {} ({})\n  log: {}",
        evidence.user_agent,
        evidence.codex_home,
        evidence.platform,
        thread,
        turn,
        tracked,
        notification_count,
        notifications,
        evidence.log_path.display()
    ))
}

fn parse_codex_probe_args(args: &[String]) -> Result<CodexProbeMode, String> {
    let mut index = 0;
    let mut mode = CodexProbeMode::Initialize;
    while index < args.len() {
        match args[index].as_str() {
            "--thread" => mode = CodexProbeMode::Thread,
            "--turn" => {
                let task = args[index + 1..].join(" ");
                if task.trim().is_empty() {
                    return Err("Usage: /agent probe codex --turn <task>".to_string());
                }
                mode = CodexProbeMode::Turn(task);
                break;
            }
            other => {
                return Err(format!(
                    "Unknown Codex probe option `{other}`. Usage: /agent probe codex [--thread|--turn <task>]"
                ));
            }
        }
        index += 1;
    }
    Ok(mode)
}

fn adapter_readiness(adapter: AgentAdapterDescriptor) -> &'static str {
    let binary_ready = match adapter.binary {
        Some(binary) => command_exists(binary),
        None if adapter.config_label == "command" => {
            adapter.config_env.map(env_is_configured).unwrap_or(false)
        }
        None if adapter.config_required => false,
        None => true,
    };
    if adapter
        .config_env
        .is_some_and(|env_key| env_key.contains("<TOOL>"))
    {
        return "dynamic";
    }
    let config_ready =
        !adapter.config_required || adapter.config_env.map(env_is_configured).unwrap_or(true);
    if binary_ready && config_ready {
        "ready"
    } else if binary_ready || config_ready {
        "partial"
    } else {
        "setup needed"
    }
}

fn render_acp_probe(cwd: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    let Some(command) = env::var("ROBOCODE_AGENT_ACP_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        lines.push("    handshake: skipped (set ROBOCODE_AGENT_ACP_COMMAND)".to_string());
        return lines;
    };
    match run_acp_initialize_probe(cwd, &command) {
        Ok(evidence) => {
            lines.push("    handshake: ok (initialize)".to_string());
            lines.push(format!("    protocol: {}", evidence.protocol_version));
            lines.push(format!("    agent: {}", evidence.agent_label));
            lines.push(format!("    log: {}", evidence.log_path.display()));
        }
        Err(error) => lines.push(format!("    handshake: failed ({error})")),
    }
    lines
}

fn render_codex_doctor(cwd: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    let command = codex_command();
    lines.push(format!("    command: {command}"));

    match codex_diagnostics(cwd, &command) {
        CodexDiagnosticReport::Ready(report) => {
            lines.push(format!("    version: {}", report.version));
            lines.push(format!("    app-server: {}", report.app_server));
            lines.extend(render_codex_protocol_probe(cwd, &command));
            lines.push(format!("    auth: {}", report.auth));
            lines.push(format!("    config: {}", report.config_sources));
            lines.push(format!("    jobs: {}", report.job_store.display()));
            lines.push("    commands: /agent review codex | /agent challenge codex | /agent run codex [--write] <task>".to_string());
            lines.push(
                "    controls: /agent status | /agent result <id> | /agent cancel <id>".to_string(),
            );
        }
        CodexDiagnosticReport::Unavailable(reason) => {
            lines.push(format!("    version: unavailable ({reason})"));
            lines.push("    app-server: skipped".to_string());
            lines.push("    auth: skipped".to_string());
            lines.push("    next: install Codex with `npm install -g @openai/codex` or set ROBOCODE_AGENT_CODEX_COMMAND".to_string());
        }
    }
    lines
}

fn render_codex_protocol_probe(cwd: &Path, command: &str) -> Vec<String> {
    match codex_protocol_probe(cwd, command) {
        Ok(report) if report.missing.is_empty() => vec![format!(
            "    protocol: ok ({})",
            report.available.join(", ")
        )],
        Ok(report) => vec![format!(
            "    protocol: partial (available: {}; missing: {})",
            report.available.join(", "),
            report.missing.join(", ")
        )],
        Err(error) => vec![format!("    protocol: unavailable ({error})")],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCodexReviewArgs {
    base: Option<String>,
    prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCodexRunArgs {
    write: bool,
    app_server: bool,
    task: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CodexProbeMode {
    Initialize,
    Thread,
    Turn(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexJobKind {
    Review,
    Challenge,
    Run,
}

impl CodexJobKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Challenge => "challenge",
            Self::Run => "run",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexJobRecord {
    id: String,
    kind: String,
    status: String,
    pid: Option<u32>,
    command: String,
    task: String,
    log_path: PathBuf,
    result_path: PathBuf,
    baseline_path: PathBuf,
    updated_at: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CodexJobEvidence {
    session_id: Option<String>,
    files: Vec<String>,
}

fn ensure_codex_target(target: Option<&str>) -> Result<(), String> {
    match target {
        Some("codex") => Ok(()),
        Some(other) => Err(format!(
            "Unsupported agent `{other}` for this command. Use `codex`."
        )),
        None => Err("Usage: /agent <review|challenge|run> codex ...".to_string()),
    }
}

fn parse_codex_review_args(args: &[String]) -> Result<ParsedCodexReviewArgs, String> {
    let mut base = None;
    let mut prompt = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--base" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Usage: /agent review codex [--base <ref>] [prompt]".to_string());
                };
                base = Some(value.clone());
                index += 2;
            }
            value => {
                prompt.push(value.to_string());
                index += 1;
            }
        }
    }
    Ok(ParsedCodexReviewArgs {
        base,
        prompt: prompt.join(" "),
    })
}

fn parse_codex_run_args(args: &[String]) -> Result<ParsedCodexRunArgs, String> {
    let mut write = false;
    let mut app_server = false;
    let mut task = Vec::new();
    let mut end_of_options = false;
    for value in args {
        match value.as_str() {
            _ if end_of_options => task.push(value.clone()),
            "--write" => write = true,
            "--read-only" => write = false,
            "--app-server" => app_server = true,
            "--" => end_of_options = true,
            other if other.starts_with("--") && task.is_empty() => {
                return Err(format!(
                    "Unknown Codex run option `{other}`. Usage: /agent run codex [--write|--app-server] <task>"
                ));
            }
            _ => task.push(value.clone()),
        }
    }
    Ok(ParsedCodexRunArgs {
        write,
        app_server,
        task: task.join(" "),
    })
}

fn start_codex_job(
    cwd: &Path,
    command: &str,
    kind: CodexJobKind,
    task: String,
    mut args: Vec<String>,
) -> Result<String, String> {
    let id = format!("codex-{}", timestamp_millis());
    let log_path = codex_job_artifact_path(cwd, &id, "log");
    let result_path = codex_job_artifact_path(cwd, &id, "result.md");
    let baseline_path = codex_job_artifact_path(cwd, &id, "baseline.status");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    write_codex_status_baseline(cwd, &baseline_path)?;
    if kind == CodexJobKind::Run {
        let insert_at = args.len().saturating_sub(1);
        args.insert(insert_at, "-o".to_string());
        args.insert(insert_at + 1, result_path.display().to_string());
    }
    let stdout = fs::File::create(&log_path)
        .map_err(|err| format!("failed to create Codex job log: {err}"))?;
    let stderr = stdout
        .try_clone()
        .map_err(|err| format!("failed to clone Codex job log: {err}"))?;
    let mut child = Command::new(command)
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|err| {
            format!(
                "failed to launch `{}`: {err}",
                command_line_owned(command, &args)
            )
        })?;
    let pid = child.id();

    let record = CodexJobRecord {
        id: id.clone(),
        kind: kind.as_str().to_string(),
        status: "running".to_string(),
        pid: Some(pid),
        command: command_line_owned(command, &args),
        task,
        log_path: log_path.clone(),
        result_path: result_path.clone(),
        baseline_path: baseline_path.clone(),
        updated_at: timestamp_millis(),
    };
    append_codex_job_record(cwd, "started", &record)?;
    let monitor_cwd = cwd.to_path_buf();
    let mut monitor_record = record.clone();
    std::thread::spawn(move || {
        let status = child.wait();
        if find_codex_job(&monitor_cwd, &monitor_record.id)
            .ok()
            .flatten()
            .is_some_and(|job| job.status == "cancelled")
        {
            return;
        }
        monitor_record.status = match status {
            Ok(status) if status.success() => "finished".to_string(),
            Ok(_) | Err(_) => "failed".to_string(),
        };
        monitor_record.updated_at = timestamp_millis();
        let _ = append_codex_job_record(&monitor_cwd, "completed", &monitor_record);
    });

    Ok(format!(
        "Started Codex {} job `{}`.\n  pid: {}\n  log: {}\n  result: {}\n\nUse `/agent status` to watch it and `/agent result {}` to read output.",
        kind.as_str(),
        id,
        pid,
        log_path.display(),
        result_path.display(),
        id
    ))
}

fn start_codex_app_server_job(cwd: &Path, command: &str, task: String) -> Result<String, String> {
    let id = format!("codex-app-{}", timestamp_millis());
    let log_path = codex_job_artifact_path(cwd, &id, "jsonl");
    let result_path = codex_job_artifact_path(cwd, &id, "result.md");
    let baseline_path = codex_job_artifact_path(cwd, &id, "baseline.status");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    write_codex_status_baseline(cwd, &baseline_path)?;
    let record = CodexJobRecord {
        id: id.clone(),
        kind: "app-server-turn".to_string(),
        status: "running".to_string(),
        pid: None,
        command: "codex app-server turn/start".to_string(),
        task: task.clone(),
        log_path: log_path.clone(),
        result_path: result_path.clone(),
        baseline_path: baseline_path.clone(),
        updated_at: timestamp_millis(),
    };
    append_codex_job_record(cwd, "started", &record)?;

    let monitor_cwd = cwd.to_path_buf();
    let monitor_command = command.to_string();
    let monitor_task = task.clone();
    let mut monitor_record = record.clone();
    std::thread::spawn(move || {
        match run_codex_app_server_probe_with_log(
            &monitor_cwd,
            &monitor_command,
            CodexProbeMode::Turn(monitor_task),
            log_path,
        ) {
            Ok(evidence) => {
                let _ = write_codex_app_server_turn_result(&result_path, &evidence);
                monitor_record.status = codex_app_server_turn_job_status(&evidence);
            }
            Err(error) => {
                let _ = fs::write(
                    &result_path,
                    format!("# Codex app-server turn failed\n\n{error}\n"),
                );
                monitor_record.status = "failed".to_string();
            }
        }
        monitor_record.updated_at = timestamp_millis();
        let _ = append_codex_job_record(&monitor_cwd, "completed", &monitor_record);
    });

    Ok(format!(
        "Started Codex app-server job `{}`.\n  log: {}\n  result: {}\n\nUse `/agent status` to watch it and `/agent result {}` to read output.",
        id,
        record.log_path.display(),
        record.result_path.display(),
        id
    ))
}

fn render_codex_job_status(cwd: &Path) -> Result<String, String> {
    let jobs = latest_codex_jobs(cwd)?;
    if jobs.is_empty() {
        return Ok("Codex jobs:\n  no tracked jobs".to_string());
    }
    let mut lines = vec![
        "Codex jobs:".to_string(),
        "  id                    kind       status       pid     updated     task".to_string(),
    ];
    let mut observed = Vec::new();
    for mut job in jobs.into_iter().rev().take(8) {
        let observed_status = observed_codex_status(&job);
        if observed_status != job.status {
            job.status = observed_status;
            job.updated_at = timestamp_millis();
            observed.push(job.clone());
        }
        lines.push(format!(
            "  {:<21} {:<10} {:<12} {:<7} {:<11} {}",
            job.id,
            job.kind,
            job.status,
            job.pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            relative_millis(job.updated_at),
            truncate_for_line(&job.task, 72)
        ));
        let evidence = codex_job_evidence(cwd, &job);
        if let Some(session_id) = evidence.session_id {
            lines.push(format!("    resume: codex resume {session_id}"));
        }
        if !evidence.files.is_empty() {
            lines.push(format!(
                "    files: {}",
                evidence
                    .files
                    .into_iter()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    for job in observed {
        append_codex_job_record(cwd, "observed", &job)?;
    }
    Ok(lines.join("\n"))
}

fn render_codex_job_result(cwd: &Path, id: Option<&str>) -> Result<String, String> {
    let id = id.ok_or_else(|| "Usage: /agent result <id>".to_string())?;
    let job = find_codex_job(cwd, id)?.ok_or_else(|| format!("Unknown Codex job `{id}`"))?;
    let status = observed_codex_status(&job);
    let result = fs::read_to_string(&job.result_path)
        .ok()
        .filter(|content| !content.trim().is_empty())
        .or_else(|| tail_text(&job.log_path, 60).ok())
        .unwrap_or_else(|| "No output captured yet.".to_string());
    let evidence = codex_job_evidence_from_text(cwd, &job, &result);
    let resume = evidence
        .session_id
        .as_ref()
        .map(|session_id| format!("  resume: codex resume {session_id}\n"))
        .unwrap_or_default();
    let files = if evidence.files.is_empty() {
        String::new()
    } else {
        format!("  files: {}\n", evidence.files.join(", "))
    };
    Ok(format!(
        "Codex job `{}`\n  kind: {}\n  status: {}\n  pid: {}\n  command: {}\n  log: {}\n  result: {}\n{}{}\n{}",
        job.id,
        job.kind,
        status,
        job.pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "-".to_string()),
        job.command,
        job.log_path.display(),
        job.result_path.display(),
        resume,
        files,
        result.trim()
    ))
}

fn cancel_codex_job(cwd: &Path, id: Option<&str>) -> Result<String, String> {
    let id = id.ok_or_else(|| "Usage: /agent cancel <id>".to_string())?;
    let mut job = find_codex_job(cwd, id)?.ok_or_else(|| format!("Unknown Codex job `{id}`"))?;
    if matches!(job.status.as_str(), "cancelled" | "finished") {
        return Ok(format!("Codex job `{id}` is already {}.", job.status));
    }
    let Some(pid) = job.pid else {
        return Err(format!("Codex job `{id}` has no process id to cancel."));
    };
    job.status = "cancelled".to_string();
    job.updated_at = timestamp_millis();
    append_codex_job_record(cwd, "cancelled", &job)?;
    terminate_process(pid)?;
    Ok(format!("Cancelled Codex job `{id}` (pid {pid})."))
}

fn codex_command() -> String {
    env::var("ROBOCODE_AGENT_CODEX_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "codex".to_string())
}

fn codex_job_artifact_path(cwd: &Path, id: &str, ext: &str) -> PathBuf {
    cwd.join(".robocode")
        .join("agents")
        .join(format!("{id}.{ext}"))
}

fn append_codex_job_record(cwd: &Path, event: &str, record: &CodexJobRecord) -> Result<(), String> {
    let path = codex_job_store_path(cwd);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    writeln!(
        file,
        r#"{{"ts":{},"event":"{}","id":"{}","kind":"{}","status":"{}","pid":{},"command":"{}","task":"{}","log":"{}","result":"{}","baseline":"{}"}}"#,
        record.updated_at,
        escape_json_fragment(event),
        escape_json_fragment(&record.id),
        escape_json_fragment(&record.kind),
        escape_json_fragment(&record.status),
        record
            .pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "null".to_string()),
        escape_json_fragment(&record.command),
        escape_json_fragment(&record.task),
        escape_json_fragment(&record.log_path.display().to_string()),
        escape_json_fragment(&record.result_path.display().to_string()),
        escape_json_fragment(&record.baseline_path.display().to_string())
    )
    .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn latest_codex_jobs(cwd: &Path) -> Result<Vec<CodexJobRecord>, String> {
    let path = codex_job_store_path(cwd);
    let Ok(content) = fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let mut jobs = Vec::<CodexJobRecord>::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Some(record) = parse_codex_job_record(line) else {
            continue;
        };
        if let Some(existing) = jobs.iter_mut().find(|job| job.id == record.id) {
            *existing = record;
        } else {
            jobs.push(record);
        }
    }
    jobs.sort_by_key(|job| job.updated_at);
    Ok(jobs)
}

fn find_codex_job(cwd: &Path, id: &str) -> Result<Option<CodexJobRecord>, String> {
    Ok(latest_codex_jobs(cwd)?.into_iter().find(|job| job.id == id))
}

fn parse_codex_job_record(line: &str) -> Option<CodexJobRecord> {
    let id = json_string_field(line, "id")?;
    let log_path = PathBuf::from(json_string_field(line, "log")?);
    let result_path = PathBuf::from(json_string_field(line, "result")?);
    let baseline_path = json_string_field(line, "baseline")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            log_path
                .parent()
                .map(|parent| parent.join(format!("{id}.baseline.status")))
                .unwrap_or_else(|| PathBuf::from(format!("{id}.baseline.status")))
        });
    Some(CodexJobRecord {
        id,
        kind: json_string_field(line, "kind")?,
        status: json_string_field(line, "status")?,
        pid: json_number_field(line, "pid").and_then(|value| value.parse().ok()),
        command: json_string_field(line, "command").unwrap_or_default(),
        task: json_string_field(line, "task").unwrap_or_default(),
        log_path,
        result_path,
        baseline_path,
        updated_at: json_number_field(line, "ts")?.parse().ok()?,
    })
}

fn write_codex_status_baseline(cwd: &Path, path: &Path) -> Result<(), String> {
    let content = match git_status_snapshot(cwd) {
        Ok(lines) => lines.join("\n"),
        Err(error) => format!("# unavailable: {error}"),
    };
    fs::write(path, content).map_err(|err| {
        format!(
            "failed to write Codex job baseline {}: {err}",
            path.display()
        )
    })
}

fn codex_job_evidence(cwd: &Path, job: &CodexJobRecord) -> CodexJobEvidence {
    let result = fs::read_to_string(&job.result_path).unwrap_or_default();
    let log = tail_text(&job.log_path, 120).unwrap_or_default();
    codex_job_evidence_from_text(cwd, job, &format!("{result}\n{log}"))
}

fn record_codex_app_server_turn_probe(
    cwd: &Path,
    task: &str,
    evidence: &CodexAppServerProbeEvidence,
) -> Result<String, String> {
    let id = evidence
        .turn_id
        .as_ref()
        .map(|turn_id| format!("codex-app-{turn_id}"))
        .unwrap_or_else(|| format!("codex-app-{}", timestamp_millis()));
    let result_path = codex_job_artifact_path(cwd, &id, "result.md");
    let baseline_path = codex_job_artifact_path(cwd, &id, "baseline.status");
    if let Some(parent) = result_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    write_codex_status_baseline(cwd, &baseline_path)?;
    let command = "codex app-server turn/start".to_string();
    let mut record = CodexJobRecord {
        id: id.clone(),
        kind: "app-server-turn".to_string(),
        status: "running".to_string(),
        pid: None,
        command,
        task: task.to_string(),
        log_path: evidence.log_path.clone(),
        result_path: result_path.clone(),
        baseline_path,
        updated_at: timestamp_millis(),
    };
    append_codex_job_record(cwd, "started", &record)?;

    write_codex_app_server_turn_result(&result_path, evidence)?;
    record.status = codex_app_server_turn_job_status(evidence);
    record.updated_at = timestamp_millis();
    append_codex_job_record(cwd, "completed", &record)?;
    Ok(id)
}

fn write_codex_app_server_turn_result(
    result_path: &Path,
    evidence: &CodexAppServerProbeEvidence,
) -> Result<(), String> {
    fs::write(
        result_path,
        format!(
            "# Codex app-server turn\n\nthread: {}\nturn: {}\nstatus: {}\nlog: {}\n",
            evidence.thread_id.as_deref().unwrap_or("unknown"),
            evidence.turn_id.as_deref().unwrap_or("unknown"),
            evidence.turn_status.as_deref().unwrap_or("unknown"),
            evidence.log_path.display()
        ),
    )
    .map_err(|err| format!("failed to write {}: {err}", result_path.display()))
}

fn codex_app_server_turn_job_status(evidence: &CodexAppServerProbeEvidence) -> String {
    match evidence.turn_status.as_deref() {
        Some("completed") => "finished".to_string(),
        Some("failed" | "interrupted") => "failed".to_string(),
        _ => "observed".to_string(),
    }
}

fn codex_job_evidence_from_text(cwd: &Path, job: &CodexJobRecord, text: &str) -> CodexJobEvidence {
    let mut files = changed_files_since_codex_start(cwd, job);
    files.extend(extract_file_mentions(text));
    files.sort();
    files.dedup();
    CodexJobEvidence {
        session_id: extract_codex_session_id(text),
        files,
    }
}

fn changed_files_since_codex_start(cwd: &Path, job: &CodexJobRecord) -> Vec<String> {
    let Ok(current) = git_status_snapshot(cwd) else {
        return Vec::new();
    };
    let baseline = fs::read_to_string(&job.baseline_path)
        .ok()
        .filter(|content| !content.starts_with("# unavailable:"))
        .unwrap_or_default();
    let before = baseline.lines().map(str::to_string).collect::<HashSet<_>>();
    let mut changed = Vec::new();
    for line in current {
        if before.contains(&line) {
            continue;
        }
        if let Some(path) = git_status_path(&line) {
            changed.push(path);
        }
    }
    changed.sort();
    changed.dedup();
    changed
}

fn git_status_snapshot(cwd: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(cwd)
        .output()
        .map_err(|err| format!("failed to run git status: {err}"))?;
    if !output.status.success() {
        return Err(first_output_line(&join_output(
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
        .unwrap_or_else(|| format!("git status exited with {}", output.status)));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(git_status_line)
        .collect())
}

fn git_status_line(line: &str) -> Option<String> {
    let path = git_status_path(line)?;
    if path.starts_with(".robocode/") {
        return None;
    }
    Some(line.to_string())
}

fn git_status_path(line: &str) -> Option<String> {
    let path = line.get(3..)?.trim();
    let path = path.rsplit(" -> ").next().unwrap_or(path).trim();
    let path = path.trim_matches('"');
    if path.is_empty() || path.starts_with(".robocode/") {
        None
    } else {
        Some(path.to_string())
    }
}

fn extract_codex_session_id(text: &str) -> Option<String> {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower
            .find("codex resume ")
            .and_then(|index| line.get(index + "codex resume ".len()..))
        {
            if let Some(id) = first_identifier_token(rest) {
                return Some(id);
            }
        }
        for marker in [
            "session id:",
            "session:",
            "codex session:",
            "\"session_id\":",
            "\"thread_id\":",
        ] {
            if let Some(index) = lower.find(marker) {
                let rest = &line[index + marker.len()..];
                if let Some(id) = first_identifier_token(rest) {
                    return Some(id);
                }
            }
        }
    }
    None
}

fn first_identifier_token(value: &str) -> Option<String> {
    value
        .trim_start_matches(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\'' || ch == '=')
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | ')' | ']' | '}'))
        .find(|token| !token.trim().is_empty())
        .map(|token| {
            token
                .trim_matches(|ch: char| matches!(ch, '"' | '\'' | '`' | '.' | ',' | ';' | ':'))
                .to_string()
        })
        .filter(|token| !token.is_empty())
}

fn extract_file_mentions(text: &str) -> Vec<String> {
    let mut files = Vec::new();
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
            )
        });
        if looks_like_repo_file(token) {
            files.push(token.to_string());
        }
    }
    files.sort();
    files.dedup();
    files
}

fn looks_like_repo_file(token: &str) -> bool {
    if token.is_empty()
        || token.starts_with("http://")
        || token.starts_with("https://")
        || token.starts_with(".robocode/")
        || token.contains("://")
    {
        return false;
    }
    let has_path_shape = token.contains('/') || token.contains('.');
    let has_known_extension = [
        ".rs", ".toml", ".md", ".json", ".yaml", ".yml", ".js", ".ts", ".tsx", ".jsx", ".py",
        ".go", ".java", ".c", ".cc", ".cpp", ".h", ".hpp", ".html", ".css", ".sh",
    ]
    .iter()
    .any(|extension| token.ends_with(extension));
    has_path_shape
        && has_known_extension
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | '+'))
}

fn observed_codex_status(job: &CodexJobRecord) -> String {
    if matches!(job.status.as_str(), "cancelled" | "failed" | "finished") {
        return job.status.clone();
    }
    match job.pid {
        Some(pid) if process_is_running(pid) => "running".to_string(),
        Some(_) => "finished".to_string(),
        None => job.status.clone(),
    }
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
}

fn terminate_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .map_err(|err| format!("failed to run kill: {err}"))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("kill exited with {status}"))
                }
            })
    }
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|err| format!("failed to run taskkill: {err}"))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("taskkill exited with {status}"))
                }
            })
    }
}

fn tail_text(path: &Path, max_lines: usize) -> Result<String, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let lines = content.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

fn relative_millis(ts: u128) -> String {
    let now = timestamp_millis();
    let elapsed = now.saturating_sub(ts);
    if elapsed < 1_000 {
        "now".to_string()
    } else if elapsed < 60_000 {
        format!("{}s ago", elapsed / 1_000)
    } else {
        format!("{}m ago", elapsed / 60_000)
    }
}

fn truncate_for_line(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexReadyReport {
    version: String,
    app_server: String,
    auth: String,
    config_sources: String,
    job_store: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexProtocolProbeReport {
    available: Vec<String>,
    missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CodexDiagnosticReport {
    Ready(CodexReadyReport),
    Unavailable(String),
}

fn codex_diagnostics(cwd: &Path, command: &str) -> CodexDiagnosticReport {
    let version = match command_output(command, &["--version"], cwd, Duration::from_secs(4)) {
        Ok(output) => first_output_line(&output).unwrap_or_else(|| "unknown".to_string()),
        Err(error) => return CodexDiagnosticReport::Unavailable(error),
    };

    let app_server = match command_output(
        command,
        &["app-server", "--help"],
        cwd,
        Duration::from_secs(4),
    ) {
        Ok(output) if output.contains("app-server") => "ok (codex app-server)".to_string(),
        Ok(output) => format!(
            "unexpected help output ({})",
            first_output_line(&output).unwrap_or_else(|| "empty".to_string())
        ),
        Err(error) => format!("unavailable ({error})"),
    };

    let auth = match command_output(command, &["login", "status"], cwd, Duration::from_secs(5)) {
        Ok(output) => first_output_line(&output).unwrap_or_else(|| "unknown".to_string()),
        Err(error) => format!("setup needed ({error})"),
    };

    CodexDiagnosticReport::Ready(CodexReadyReport {
        version,
        app_server,
        auth,
        config_sources: codex_config_sources(cwd),
        job_store: codex_job_store_path(cwd),
    })
}

fn codex_protocol_probe(cwd: &Path, command: &str) -> Result<CodexProtocolProbeReport, String> {
    let schema_dir = codex_protocol_schema_dir(cwd);
    if schema_dir.exists() {
        fs::remove_dir_all(&schema_dir).map_err(|err| {
            format!(
                "failed to remove stale schema dir {}: {err}",
                schema_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&schema_dir).map_err(|err| {
        format!(
            "failed to create schema dir {}: {err}",
            schema_dir.display()
        )
    })?;
    let out = schema_dir.display().to_string();
    let probe = command_output(
        command,
        &[
            "app-server",
            "generate-json-schema",
            "--experimental",
            "--out",
            &out,
        ],
        cwd,
        Duration::from_secs(5),
    );
    let result = match probe {
        Ok(_) => codex_protocol_probe_from_dir(&schema_dir),
        Err(error) => Err(error),
    };
    let _ = fs::remove_dir_all(&schema_dir);
    result
}

fn codex_protocol_schema_dir(cwd: &Path) -> PathBuf {
    cwd.join(".robocode")
        .join("tmp")
        .join(format!("codex-schema-{}", timestamp_millis()))
}

fn codex_protocol_probe_from_dir(dir: &Path) -> Result<CodexProtocolProbeReport, String> {
    let client = read_schema_file(dir, "ClientRequest.json")?;
    let server_notifications = read_schema_file(dir, "ServerNotification.json")?;
    let server_requests = read_schema_file(dir, "ServerRequest.json")?;
    let checks: [(&str, &str, &[&str]); 6] = [
        (
            "thread lifecycle",
            &client,
            &["thread/start", "thread/resume", "thread/read"][..],
        ),
        ("review", &client, &["review/start"][..]),
        (
            "turn control",
            &client,
            &["turn/start", "turn/interrupt"][..],
        ),
        (
            "events",
            &server_notifications,
            &["thread/started", "turn/started", "turn/completed"][..],
        ),
        (
            "evidence",
            &server_notifications,
            &[
                "item/commandExecution/outputDelta",
                "item/fileChange/outputDelta",
                "turn/diff/updated",
            ][..],
        ),
        (
            "approvals",
            &server_requests,
            &[
                "item/commandExecution/requestApproval",
                "item/fileChange/requestApproval",
                "item/permissions/requestApproval",
            ][..],
        ),
    ];
    let mut available = Vec::new();
    let mut missing = Vec::new();
    for (label, haystack, needles) in checks {
        if needles.iter().all(|needle| haystack.contains(needle)) {
            available.push(label.to_string());
        } else {
            missing.push(label.to_string());
        }
    }
    Ok(CodexProtocolProbeReport { available, missing })
}

fn read_schema_file(dir: &Path, name: &str) -> Result<String, String> {
    fs::read_to_string(dir.join(name))
        .map_err(|err| format!("failed to read generated {name}: {err}"))
}

fn command_output(
    command: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let mut child = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to launch `{}`: {err}", command_line(command, args)))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let status = child.wait();
        let stdout = read_pipe(stdout);
        let stderr = read_pipe(stderr);
        let _ = sender.send((status, stdout, stderr));
    });

    match receiver.recv_timeout(timeout) {
        Ok((Ok(status), stdout, stderr)) if status.success() => Ok(join_output(stdout, stderr)),
        Ok((Ok(status), stdout, stderr)) => Err(format!(
            "`{}` exited with {}; {}",
            command_line(command, args),
            status,
            first_output_line(&join_output(stdout, stderr))
                .unwrap_or_else(|| "no output".to_string())
        )),
        Ok((Err(error), _, _)) => Err(format!(
            "`{}` wait failed: {error}",
            command_line(command, args)
        )),
        Err(_) => Err(format!("`{}` timed out", command_line(command, args))),
    }
}

fn read_pipe(mut pipe: impl std::io::Read) -> String {
    let mut output = String::new();
    let _ = pipe.read_to_string(&mut output);
    output
}

fn join_output(stdout: String, stderr: String) -> String {
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (false, false) => format!("{}\n{}", stdout.trim(), stderr.trim()),
        (false, true) => stdout.trim().to_string(),
        (true, false) => stderr.trim().to_string(),
        (true, true) => String::new(),
    }
}

fn first_output_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn command_line(command: &str, args: &[&str]) -> String {
    std::iter::once(command)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_line_owned(command: &str, args: &[String]) -> String {
    std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn codex_config_sources(cwd: &Path) -> String {
    let mut sources = Vec::new();
    let project_config = cwd.join(".codex").join("config.toml");
    if project_config.exists() {
        sources.push(format!("project {}", project_config.display()));
    }
    if let Some(home) = env::var_os("HOME") {
        let user_config = PathBuf::from(home).join(".codex").join("config.toml");
        if user_config.exists() {
            sources.push(format!("user {}", user_config.display()));
        }
    }
    if sources.is_empty() {
        "none found (Codex defaults apply)".to_string()
    } else {
        sources.join("; ")
    }
}

fn codex_job_store_path(cwd: &Path) -> PathBuf {
    cwd.join(".robocode")
        .join("agents")
        .join("codex-jobs.jsonl")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcpProbeEvidence {
    protocol_version: String,
    agent_label: String,
    log_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexAppServerProbeEvidence {
    user_agent: String,
    codex_home: String,
    platform: String,
    thread_id: Option<String>,
    turn_id: Option<String>,
    turn_status: Option<String>,
    notifications: Vec<String>,
    log_path: PathBuf,
}

fn run_codex_app_server_probe(
    cwd: &Path,
    command: &str,
    mode: CodexProbeMode,
) -> Result<CodexAppServerProbeEvidence, String> {
    run_codex_app_server_probe_with_log(cwd, command, mode, codex_app_server_probe_log_path(cwd))
}

fn run_codex_app_server_probe_with_log(
    cwd: &Path,
    command: &str,
    mode: CodexProbeMode,
    log_path: PathBuf,
) -> Result<CodexAppServerProbeEvidence, String> {
    let request = codex_app_server_initialize_request();
    let mut log_entries = vec![jsonl_event("client", &request)];
    let mut child = spawn_codex_app_server(cwd, command)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open Codex app-server stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to open Codex app-server stdout".to_string())?;
    stdin
        .write_all(request.as_bytes())
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .map_err(|err| format!("failed to write Codex app-server initialize: {err}"))?;

    // Codex app-server stdio is newline-delimited JSON: responses and
    // notifications can arrive in either order, so keep every line as evidence.
    let receiver = read_lines_async(stdout);
    let mut notifications = Vec::new();
    let response = match read_codex_app_server_response(
        &receiver,
        1,
        &mut log_entries,
        &mut notifications,
        Duration::from_secs(5),
    ) {
        Ok(response) => response,
        Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
    };

    let start_thread = !matches!(mode, CodexProbeMode::Initialize);
    let thread_id = if start_thread {
        let request = codex_app_server_thread_start_request(cwd);
        log_entries.push(jsonl_event("client", &request));
        if let Err(error) = stdin
            .write_all(request.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
        {
            return Err(finish_failed_probe(
                child,
                log_path,
                log_entries,
                format!("failed to write Codex app-server thread/start: {error}"),
            ));
        }
        let thread_response = match read_codex_app_server_response(
            &receiver,
            2,
            &mut log_entries,
            &mut notifications,
            Duration::from_secs(8),
        ) {
            Ok(response) => response,
            Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
        };
        json_object_string_field(&thread_response, "thread", "id")
    } else {
        None
    };

    let mut turn_id = None;
    let mut turn_status = None;
    if let CodexProbeMode::Turn(task) = &mode {
        let Some(thread_id) = thread_id.as_deref() else {
            return Err(finish_failed_probe(
                child,
                log_path.clone(),
                log_entries.clone(),
                "Codex app-server thread/start did not return a thread id".to_string(),
            ));
        };
        let request = codex_app_server_turn_start_request(cwd, thread_id, task);
        log_entries.push(jsonl_event("client", &request));
        if let Err(error) = stdin
            .write_all(request.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
        {
            return Err(finish_failed_probe(
                child,
                log_path,
                log_entries,
                format!("failed to write Codex app-server turn/start: {error}"),
            ));
        }
        let turn_response = match read_codex_app_server_response(
            &receiver,
            3,
            &mut log_entries,
            &mut notifications,
            Duration::from_secs(8),
        ) {
            Ok(response) => response,
            Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
        };
        turn_id = json_object_string_field(&turn_response, "turn", "id");
        turn_status = json_string_field(&turn_response, "status");
        if let Some(completed) = collect_codex_app_server_notifications(
            &receiver,
            &mut log_entries,
            &mut notifications,
            Some("turn/completed"),
            Duration::from_secs(30),
        ) {
            turn_status = json_string_field(&completed, "status").or(turn_status);
        }
    }

    while let Ok(Ok(line)) = receiver.recv_timeout(Duration::from_millis(150)) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        log_entries.push(jsonl_event("server", line));
        if let Some(method) = json_string_field(line, "method") {
            notifications.push(method);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    write_probe_log(&log_path, &log_entries)?;

    Ok(CodexAppServerProbeEvidence {
        user_agent: json_string_field(&response, "userAgent")
            .unwrap_or_else(|| "unknown".to_string()),
        codex_home: json_string_field(&response, "codexHome")
            .unwrap_or_else(|| "unknown".to_string()),
        platform: json_string_field(&response, "platformOs")
            .unwrap_or_else(|| "unknown".to_string()),
        thread_id,
        turn_id,
        turn_status,
        notifications,
        log_path,
    })
}

fn run_acp_initialize_probe(cwd: &Path, command: &str) -> Result<AcpProbeEvidence, String> {
    let log_path = acp_probe_log_path(cwd);
    let mut log_entries = Vec::new();
    let request = acp_initialize_request();
    log_entries.push(jsonl_event("client", &request));

    let mut child = spawn_acp_process(cwd, command)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open ACP stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to open ACP stdout".to_string())?;
    stdin
        .write_all(request.as_bytes())
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .map_err(|err| format!("failed to write initialize: {err}"))?;

    let response = match read_line_with_timeout(stdout, Duration::from_secs(8)) {
        Ok(response) => response,
        Err(error) => {
            return Err(finish_failed_probe(
                child,
                log_path.clone(),
                log_entries,
                error,
            ));
        }
    };
    log_entries.push(jsonl_event("agent", &response));
    let _ = child.kill();
    let _ = child.wait();
    write_probe_log(&log_path, &log_entries)?;

    if !response.contains("\"jsonrpc\"") || !response.contains("\"result\"") {
        return Err(format!(
            "unexpected initialize response; log {}",
            log_path.display()
        ));
    }
    Ok(AcpProbeEvidence {
        protocol_version: json_number_field(&response, "protocolVersion")
            .unwrap_or_else(|| "unknown".to_string()),
        agent_label: agent_label_from_response(&response),
        log_path,
    })
}

fn finish_failed_probe(
    mut child: Child,
    log_path: PathBuf,
    mut entries: Vec<String>,
    error: String,
) -> String {
    entries.push(jsonl_event("error", &error));
    let _ = write_probe_log(&log_path, &entries);
    let _ = child.kill();
    let _ = child.wait();
    format!("{error}; log {}", log_path.display())
}

fn acp_initialize_request() -> String {
    [
        r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"#,
        r#""protocolVersion":1,"#,
        r#""clientCapabilities":{"fs":{"readTextFile":true,"writeTextFile":true},"terminal":true},"#,
        r#""clientInfo":{"name":"robocode","version":"0.1.6"}}"#,
        r#"}"#,
    ]
    .join("")
}

fn spawn_acp_process(cwd: &Path, command: &str) -> Result<Child, String> {
    shell_command(command)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to launch ACP command: {err}"))
}

fn spawn_codex_app_server(cwd: &Path, command: &str) -> Result<Child, String> {
    Command::new(command)
        .args(["app-server", "--listen", "stdio://"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to launch Codex app-server: {err}"))
}

fn shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut process = Command::new("cmd");
        process.arg("/C").arg(command);
        process
    }
    #[cfg(not(windows))]
    {
        let mut process = Command::new("sh");
        process.arg("-lc").arg(command);
        process
    }
}

fn read_lines_async(
    stdout: impl std::io::Read + Send + 'static,
) -> mpsc::Receiver<std::io::Result<String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if sender.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

fn read_codex_app_server_response(
    receiver: &mpsc::Receiver<std::io::Result<String>>,
    request_id: u32,
    log_entries: &mut Vec<String>,
    notifications: &mut Vec<String>,
    timeout: Duration,
) -> Result<String, String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let remaining = timeout
            .checked_sub(start.elapsed())
            .unwrap_or_else(|| Duration::from_millis(1));
        let line = match receiver.recv_timeout(remaining) {
            Ok(Ok(line)) if !line.trim().is_empty() => line.trim().to_string(),
            Ok(Ok(_)) => continue,
            Ok(Err(error)) => {
                return Err(format!("failed to read Codex app-server response: {error}"));
            }
            Err(_) => break,
        };
        log_entries.push(jsonl_event("server", &line));
        if line.contains(&format!(r#""id":{request_id}"#)) {
            if line.contains(r#""result""#) {
                return Ok(line);
            }
            return Err(format!(
                "Codex app-server request {request_id} failed: {line}"
            ));
        }
        if let Some(method) = json_string_field(&line, "method") {
            notifications.push(method);
        }
    }
    Err(format!(
        "Codex app-server request {request_id} response timed out"
    ))
}

fn collect_codex_app_server_notifications(
    receiver: &mpsc::Receiver<std::io::Result<String>>,
    log_entries: &mut Vec<String>,
    notifications: &mut Vec<String>,
    until_method: Option<&str>,
    timeout: Duration,
) -> Option<String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let remaining = timeout
            .checked_sub(start.elapsed())
            .unwrap_or_else(|| Duration::from_millis(1));
        let line = match receiver.recv_timeout(remaining) {
            Ok(Ok(line)) if !line.trim().is_empty() => line.trim().to_string(),
            Ok(Ok(_)) => continue,
            Ok(Err(_)) | Err(_) => break,
        };
        log_entries.push(jsonl_event("server", &line));
        if let Some(method) = json_string_field(&line, "method") {
            notifications.push(method.clone());
            if until_method.is_some_and(|target| target == method) {
                return Some(line);
            }
        }
    }
    None
}

fn read_line_with_timeout(
    stdout: impl std::io::Read + Send + 'static,
    timeout: Duration,
) -> Result<String, String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    match receiver.recv_timeout(timeout) {
        Ok(Ok(line)) if !line.trim().is_empty() => Ok(line.trim().to_string()),
        Ok(Ok(_)) => Err("ACP command closed stdout without response".to_string()),
        Ok(Err(err)) => Err(format!("failed to read initialize response: {err}")),
        Err(_) => Err("initialize response timed out".to_string()),
    }
}

fn acp_probe_log_path(cwd: &Path) -> PathBuf {
    cwd.join(".robocode")
        .join("agents")
        .join(format!("acp-doctor-{}.jsonl", timestamp_millis()))
}

fn codex_app_server_probe_log_path(cwd: &Path) -> PathBuf {
    cwd.join(".robocode")
        .join("agents")
        .join(format!("codex-app-server-{}.jsonl", timestamp_millis()))
}

fn codex_app_server_initialize_request() -> String {
    [
        r#"{"id":1,"method":"initialize","params":{"clientInfo":{"name":"robocode","version":""#,
        env!("CARGO_PKG_VERSION"),
        r#""},"capabilities":{"experimentalApi":true,"requestAttestation":false,"optOutNotificationMethods":[]}}}"#,
    ]
    .join("")
}

fn codex_app_server_thread_start_request(cwd: &Path) -> String {
    let cwd = escape_json_fragment(&cwd.display().to_string());
    format!(
        r#"{{"id":2,"method":"thread/start","params":{{"model":null,"modelProvider":null,"cwd":"{cwd}","runtimeWorkspaceRoots":["{cwd}"],"approvalPolicy":"never","approvalsReviewer":"user","sandbox":"read-only","permissions":null,"config":null,"serviceName":"robocode","baseInstructions":null,"developerInstructions":null,"personality":null,"ephemeral":true,"sessionStartSource":"startup","threadSource":"subagent","environments":[],"dynamicTools":null,"experimentalRawEvents":false,"persistExtendedHistory":false}}}}"#
    )
}

fn codex_app_server_turn_start_request(cwd: &Path, thread_id: &str, task: &str) -> String {
    let cwd = escape_json_fragment(&cwd.display().to_string());
    let thread_id = escape_json_fragment(thread_id);
    let task = escape_json_fragment(task);
    format!(
        r#"{{"id":3,"method":"turn/start","params":{{"threadId":"{thread_id}","input":[{{"type":"text","text":"{task}","text_elements":[]}}],"responsesapiClientMetadata":null,"environments":[],"cwd":"{cwd}","runtimeWorkspaceRoots":["{cwd}"],"approvalPolicy":"never","approvalsReviewer":"user","sandboxPolicy":{{"type":"readOnly","networkAccess":false}},"permissions":null,"model":null,"serviceTier":null,"effort":null,"summary":null,"personality":null,"outputSchema":null,"collaborationMode":null}}}}"#
    )
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn write_probe_log(path: &Path, entries: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, entries.join("\n") + "\n").map_err(|err| err.to_string())
}

fn jsonl_event(direction: &str, payload: &str) -> String {
    format!(
        r#"{{"direction":"{}","payload":"{}"}}"#,
        escape_json_fragment(direction),
        escape_json_fragment(payload)
    )
}

fn escape_json_fragment(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

fn agent_label_from_response(response: &str) -> String {
    let name = json_string_field(response, "name").unwrap_or_else(|| "unknown".to_string());
    let version = json_string_field(response, "version");
    match version {
        Some(version) => format!("{name} {version}"),
        None => name,
    }
}

fn json_string_field(response: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = response.find(&marker)? + marker.len();
    let rest = response[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn json_object_string_field(response: &str, object: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{object}":"#);
    let start = response.find(&marker)? + marker.len();
    json_string_field(&response[start..], field)
}

fn json_number_field(response: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = response.find(&marker)? + marker.len();
    let value = response[start..]
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!value.is_empty()).then_some(value)
}

fn env_is_configured(key: &str) -> bool {
    env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn command_exists(command: &str) -> bool {
    let path = PathBuf::from(command);
    if path.components().count() > 1 {
        return is_executable_file(path);
    }
    env::var_os("PATH")
        .and_then(|paths| {
            env::split_paths(&paths)
                .map(|dir| dir.join(command))
                .find(|candidate| is_executable_file(candidate))
        })
        .is_some()
}

fn is_executable_file(path: impl Into<PathBuf>) -> bool {
    let path = path.into();
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

const fn pty_binary() -> Option<&'static str> {
    #[cfg(unix)]
    {
        Some("script")
    }
    #[cfg(not(unix))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn codex_run_args_default_to_read_only_and_require_explicit_write() {
        let read_only = parse_codex_run_args(&["summarize".into(), "repo".into()])
            .expect("parse read-only task");
        assert!(!read_only.write);
        assert!(!read_only.app_server);
        assert_eq!(read_only.task, "summarize repo");

        let write = parse_codex_run_args(&["--write".into(), "edit".into(), "file".into()])
            .expect("parse write task");
        assert!(write.write);
        assert_eq!(write.task, "edit file");

        let app_server =
            parse_codex_run_args(&["--app-server".into(), "summarize".into(), "status".into()])
                .expect("parse app-server task");
        assert!(app_server.app_server);
        assert_eq!(app_server.task, "summarize status");

        let args = codex_run_command_args(Path::new("/repo"), "workspace-write", write.task);
        assert_eq!(
            args,
            vec![
                "exec",
                "--cd",
                "/repo",
                "--sandbox",
                "workspace-write",
                "edit file"
            ]
        );
    }

    #[test]
    fn codex_protocol_probe_reads_generated_schema_surface() {
        let root = temp_root("codex_schema_probe");
        fs::write(
            root.join("ClientRequest.json"),
            r#"{"enum":["thread/start","thread/resume","thread/read","review/start","turn/start","turn/interrupt"]}"#,
        )
        .expect("write client schema");
        fs::write(
            root.join("ServerNotification.json"),
            r#"{"enum":["thread/started","turn/started","turn/completed","item/commandExecution/outputDelta","item/fileChange/outputDelta","turn/diff/updated"]}"#,
        )
        .expect("write server notification schema");
        fs::write(
            root.join("ServerRequest.json"),
            r#"{"enum":["item/commandExecution/requestApproval","item/fileChange/requestApproval","item/permissions/requestApproval"]}"#,
        )
        .expect("write server request schema");

        let report = codex_protocol_probe_from_dir(&root).expect("schema probe");

        assert!(report.missing.is_empty());
        assert_eq!(
            report.available,
            vec![
                "thread lifecycle",
                "review",
                "turn control",
                "events",
                "evidence",
                "approvals"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_app_server_initialize_probe_records_jsonl_evidence() {
        let root = temp_root("codex_app_server_probe");
        let script = root.join("mock-codex-app-server.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" != \"app-server\" ]; then exit 2; fi",
                "read _line",
                "printf '%s\\n' '{\"id\":1,\"result\":{\"userAgent\":\"Codex Desktop/mock (robocode; test)\",\"codexHome\":\"/tmp/codex-home\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}'",
                "printf '%s\\n' '{\"method\":\"remoteControl/status/changed\",\"params\":{\"status\":\"disabled\"}}'",
                "sleep 1",
            ]
            .join("\n"),
        )
        .expect("write mock codex app-server script");
        make_executable(&script);

        let evidence = run_codex_app_server_probe(
            &root,
            &script.to_string_lossy(),
            CodexProbeMode::Initialize,
        )
        .expect("probe succeeds");

        assert_eq!(evidence.user_agent, "Codex Desktop/mock (robocode; test)");
        assert_eq!(evidence.codex_home, "/tmp/codex-home");
        assert_eq!(evidence.platform, "macos");
        assert_eq!(evidence.thread_id, None);
        assert_eq!(
            evidence.notifications,
            vec!["remoteControl/status/changed".to_string()]
        );
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#""method\":\"initialize"#));
        assert!(log.contains("Codex Desktop/mock"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_app_server_thread_probe_records_thread_evidence() {
        let root = temp_root("codex_app_server_thread_probe");
        let script = root.join("mock-codex-thread.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" != \"app-server\" ]; then exit 2; fi",
                "read init",
                "case \"$init\" in *'\"experimentalApi\":true'*) ;; *) exit 3 ;; esac",
                "printf '%s\\n' '{\"id\":1,\"result\":{\"userAgent\":\"Codex Desktop/mock\",\"codexHome\":\"/tmp/codex-home\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}'",
                "read thread",
                "case \"$thread\" in *'\"method\":\"thread/start\"'*) ;; *) exit 4 ;; esac",
                "printf '%s\\n' '{\"id\":2,\"result\":{\"thread\":{\"id\":\"thread_123\",\"sessionId\":\"thread_123\",\"turns\":[]},\"model\":\"gpt-test\"}}'",
                "printf '%s\\n' '{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thread_123\"}}}'",
                "sleep 1",
            ]
            .join("\n"),
        )
        .expect("write mock codex thread script");
        make_executable(&script);

        let evidence =
            run_codex_app_server_probe(&root, &script.to_string_lossy(), CodexProbeMode::Thread)
                .expect("probe succeeds");

        assert_eq!(evidence.thread_id, Some("thread_123".to_string()));
        assert_eq!(evidence.notifications, vec!["thread/started".to_string()]);
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#"thread/start"#));
        assert!(log.contains("thread_123"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_app_server_turn_probe_records_turn_evidence() {
        let root = temp_root("codex_app_server_turn_probe");
        let script = root.join("mock-codex-turn.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" != \"app-server\" ]; then exit 2; fi",
                "read init",
                "printf '%s\\n' '{\"id\":1,\"result\":{\"userAgent\":\"Codex Desktop/mock\",\"codexHome\":\"/tmp/codex-home\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}'",
                "read thread",
                "printf '%s\\n' '{\"id\":2,\"result\":{\"thread\":{\"id\":\"thread_456\",\"sessionId\":\"thread_456\",\"turns\":[]},\"model\":\"gpt-test\"}}'",
                "printf '%s\\n' '{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thread_456\"}}}'",
                "read turn",
                "case \"$turn\" in *'\"method\":\"turn/start\"'*'summarize status'*) ;; *) exit 4 ;; esac",
                "printf '%s\\n' '{\"id\":3,\"result\":{\"turn\":{\"id\":\"turn_456\",\"items\":[],\"itemsView\":\"complete\",\"status\":\"inProgress\",\"error\":null,\"startedAt\":1,\"completedAt\":null,\"durationMs\":null}}}'",
                "printf '%s\\n' '{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thread_456\",\"turn\":{\"id\":\"turn_456\",\"status\":\"inProgress\"}}}'",
                "printf '%s\\n' '{\"method\":\"item/agentMessage/delta\",\"params\":{\"threadId\":\"thread_456\",\"turnId\":\"turn_456\",\"delta\":\"ok\"}}'",
                "printf '%s\\n' '{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread_456\",\"turn\":{\"id\":\"turn_456\",\"status\":\"completed\"}}}'",
                "sleep 1",
            ]
            .join("\n"),
        )
        .expect("write mock codex turn script");
        make_executable(&script);

        let evidence = run_codex_app_server_probe(
            &root,
            &script.to_string_lossy(),
            CodexProbeMode::Turn("summarize status".to_string()),
        )
        .expect("probe succeeds");

        assert_eq!(evidence.thread_id, Some("thread_456".to_string()));
        assert_eq!(evidence.turn_id, Some("turn_456".to_string()));
        assert_eq!(evidence.turn_status, Some("completed".to_string()));
        assert!(
            evidence
                .notifications
                .contains(&"item/agentMessage/delta".to_string())
        );
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#"turn/start"#));
        assert!(log.contains("summarize status"));
        assert!(log.contains("turn/completed"));

        let job_id = record_codex_app_server_turn_probe(&root, "summarize status", &evidence)
            .expect("record job");
        let status = render_codex_job_status(&root).expect("render job status");
        let result = render_codex_job_result(&root, Some(&job_id)).expect("render job result");
        assert!(status.contains(&job_id));
        assert!(status.contains("finished"));
        assert!(result.contains("thread_456"));
        assert!(result.contains("turn_456"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_app_server_job_records_async_status() {
        let root = temp_root("codex_app_server_job");
        let script = root.join("mock-codex-job-app-server.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" != \"app-server\" ]; then exit 2; fi",
                "read _init",
                "printf '%s\\n' '{\"id\":1,\"result\":{\"userAgent\":\"Codex Desktop/mock\",\"codexHome\":\"/tmp/codex-home\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}'",
                "read _thread",
                "printf '%s\\n' '{\"id\":2,\"result\":{\"thread\":{\"id\":\"thread_job\",\"sessionId\":\"thread_job\",\"turns\":[]},\"model\":\"gpt-test\"}}'",
                "printf '%s\\n' '{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thread_job\"}}}'",
                "read _turn",
                "printf '%s\\n' '{\"id\":3,\"result\":{\"turn\":{\"id\":\"turn_job\",\"items\":[],\"itemsView\":\"complete\",\"status\":\"inProgress\",\"error\":null,\"startedAt\":1,\"completedAt\":null,\"durationMs\":null}}}'",
                "printf '%s\\n' '{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread_job\",\"turn\":{\"id\":\"turn_job\",\"status\":\"completed\"}}}'",
                "sleep 1",
            ]
            .join("\n"),
        )
        .expect("write mock codex app-server job");
        make_executable(&script);

        let started = start_codex_app_server_job(
            &root,
            &script.to_string_lossy(),
            "summarize status".to_string(),
        )
        .expect("start app-server job");
        let id = started
            .lines()
            .find_map(|line| line.split('`').nth(1))
            .expect("job id in output")
            .to_string();

        wait_until(
            || {
                find_codex_job(&root, &id)
                    .ok()
                    .flatten()
                    .is_some_and(|job| job.status == "finished")
            },
            Duration::from_secs(5),
        );

        let status = render_codex_job_status(&root).expect("render job status");
        let result = render_codex_job_result(&root, Some(&id)).expect("render job result");
        assert!(status.contains(&id));
        assert!(status.contains("finished"));
        assert!(result.contains("thread_job"));
        assert!(result.contains("turn_job"));
    }

    #[test]
    fn acp_initialize_probe_records_jsonl_evidence() {
        let root = temp_root("acp_probe_ok");
        let script = root.join("mock-acp.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _line",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true},\"agentInfo\":{\"name\":\"mock-acp\",\"version\":\"0.1.0\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp script");
        make_executable(&script);

        let evidence =
            run_acp_initialize_probe(&root, &script.to_string_lossy()).expect("probe succeeds");

        assert_eq!(evidence.protocol_version, "1");
        assert_eq!(evidence.agent_label, "mock-acp 0.1.0");
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#""method\":\"initialize"#));
        assert!(log.contains("mock-acp"));
    }

    #[test]
    fn acp_initialize_probe_reports_timeout_with_log() {
        let root = temp_root("acp_probe_timeout");
        let script = root.join("silent-acp.sh");
        fs::write(&script, "#!/bin/sh\nsleep 10\n").expect("write silent acp script");
        make_executable(&script);

        let error = run_acp_initialize_probe(&root, &script.to_string_lossy())
            .expect_err("probe should time out");

        assert!(error.contains("timed out"));
        assert!(error.contains(".robocode/agents/acp-doctor-"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_diagnostics_reports_app_server_auth_and_job_store() {
        let root = temp_root("codex_doctor_ok");
        let script = root.join("mock-codex.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" = \"--version\" ]; then",
                "  echo 'codex-cli 9.9.9'",
                "elif [ \"$1\" = \"app-server\" ] && [ \"$2\" = \"--help\" ]; then",
                "  echo 'Usage: codex app-server [OPTIONS]'",
                "elif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then",
                "  echo 'Logged in using ChatGPT'",
                "else",
                "  echo unexpected \"$@\" >&2",
                "  exit 2",
                "fi",
            ]
            .join("\n"),
        )
        .expect("write mock codex script");
        make_executable(&script);

        let report = codex_diagnostics(&root, &script.to_string_lossy());

        let CodexDiagnosticReport::Ready(report) = report else {
            panic!("expected ready Codex report");
        };
        assert_eq!(report.version, "codex-cli 9.9.9");
        assert_eq!(report.app_server, "ok (codex app-server)");
        assert_eq!(report.auth, "Logged in using ChatGPT");
        assert!(
            report
                .job_store
                .ends_with(".robocode/agents/codex-jobs.jsonl")
        );
    }

    #[test]
    fn codex_diagnostics_reports_missing_command() {
        let root = temp_root("codex_doctor_missing");

        let report = codex_diagnostics(&root, "robocode-definitely-missing-codex");

        let CodexDiagnosticReport::Unavailable(reason) = report else {
            panic!("expected unavailable Codex report");
        };
        assert!(reason.contains("failed to launch"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_job_lifecycle_records_result_artifacts() {
        let root = temp_root("codex_job_lifecycle");
        let script = root.join("mock-codex-job.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "out=''",
                "while [ \"$#\" -gt 0 ]; do",
                "  if [ \"$1\" = \"-o\" ]; then",
                "    shift",
                "    out=\"$1\"",
                "  fi",
                "  shift || true",
                "done",
                "echo 'mock codex log'",
                "if [ -n \"$out\" ]; then",
                "  mkdir -p src",
                "  echo 'pub fn generated() {}' > src/generated.rs",
                "  echo 'mock codex result' > \"$out\"",
                "  echo 'Session ID: ses_test_123' >> \"$out\"",
                "  echo 'Changed files: src/generated.rs' >> \"$out\"",
                "fi",
            ]
            .join("\n"),
        )
        .expect("write mock codex job");
        make_executable(&script);

        let started = start_codex_job(
            &root,
            &script.to_string_lossy(),
            CodexJobKind::Run,
            "hello from test".to_string(),
            vec!["exec".to_string(), "hello from test".to_string()],
        )
        .expect("start codex job");
        let id = started
            .lines()
            .find_map(|line| line.split('`').nth(1))
            .expect("job id in output")
            .to_string();

        wait_until(
            || {
                find_codex_job(&root, &id)
                    .ok()
                    .flatten()
                    .is_some_and(|job| job.status == "finished")
            },
            Duration::from_secs(5),
        );

        let status = render_codex_job_status(&root).expect("render job status");
        let result = render_codex_job_result(&root, Some(&id)).expect("render job result");

        assert!(status.contains(&id));
        assert!(status.contains("finished"));
        assert!(status.contains("resume: codex resume ses_test_123"));
        assert!(status.contains("files: src/generated.rs"));
        assert!(result.contains("mock codex result"));
        assert!(result.contains("resume: codex resume ses_test_123"));
        assert!(result.contains("files: src/generated.rs"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_job_cancel_records_cancelled_status() {
        let root = temp_root("codex_job_cancel");
        let script = root.join("slow-codex-job.sh");
        fs::write(&script, "#!/bin/sh\nsleep 5\n").expect("write slow codex job");
        make_executable(&script);

        let started = start_codex_job(
            &root,
            &script.to_string_lossy(),
            CodexJobKind::Review,
            "slow review".to_string(),
            vec!["review".to_string()],
        )
        .expect("start codex job");
        let id = started
            .lines()
            .find_map(|line| line.split('`').nth(1))
            .expect("job id in output")
            .to_string();

        let output = cancel_codex_job(&root, Some(&id)).expect("cancel job");
        let job = find_codex_job(&root, &id)
            .expect("read job")
            .expect("job exists");

        assert!(output.contains("Cancelled Codex job"));
        assert_eq!(job.status, "cancelled");
    }

    fn wait_until(predicate: impl Fn() -> bool, timeout: Duration) {
        let start = SystemTime::now();
        while start.elapsed().unwrap_or_default() < timeout {
            if predicate() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "robocode-core-{name}-{}-{}-{}",
            std::process::id(),
            timestamp_millis(),
            TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}
}
