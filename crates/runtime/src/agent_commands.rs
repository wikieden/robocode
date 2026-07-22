use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{RuntimeEventSink, SessionEngine, presentation::render_permission_denial};
use serde_json::{Value, json};
use viden_permissions::{PermissionContext, PermissionEngine};
use viden_plugin_api::{
    AgentAuthMode, AgentCommandSpec, AgentPermissionProfile, AgentPluginCapability,
    AgentPluginDescriptor, AgentProtocolVersion, AgentSource, AgentTransport,
};
use viden_plugin_host::builtin_agent_descriptors;
use viden_types::{
    AgentAdapterSource, AgentAdapterView, AgentAuthState, AgentAvailability, AgentCapabilityRecord,
    AgentNextAction, AgentRole, AgentRoute, AgentSessionRequest, AgentSessionStatus,
    AgentSessionView, AgentStartability, AgentTaskKind, AgentTaskRecord, AgentTaskStatus,
    ApprovalResponse, CapabilityId, EvidenceView, MergeGateDecisionOutcome,
    MergeGatePolicySnapshot, MergeGateRecord, MergeGateStatus, MergeGateType, PermissionDecision,
    PermissionLogEntry, RuntimeEvent, RuntimeEventKind, RuntimeOwner, ToolInput, ToolSpec,
    TranscriptEntry, fresh_id, now_timestamp, truncate_for_preview,
};

const SHELL_SCRIPT_THRESHOLD: usize = 32 * 1024;
const DEFAULT_LOCAL_ACP_HANDSHAKE_TIMEOUT_SECS: u64 = 30;
const DEFAULT_REGISTRY_ACP_HANDSHAKE_TIMEOUT_SECS: u64 = 90;
const DEFAULT_LOCAL_ACP_SESSION_TIMEOUT_SECS: u64 = 60;
const DEFAULT_KIRO_ACP_SESSION_TIMEOUT_SECS: u64 = 120;
const ACP_SESSION_CANCEL_REQUEST_ID: u64 = 90;

pub(crate) type AgentSessionApprover =
    Box<dyn FnMut(viden_types::PermissionPrompt) -> ApprovalResponse + Send + 'static>;

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
        entrypoint: "/agent review codex [prompt] | /lane codex-review <task> | /lane codex <task>",
        binary: Some("codex"),
        config_env: Some("VIDEN_LANE_CODEX_TEMPLATE"),
        config_label: "template",
        config_required: false,
    },
    AgentAdapterDescriptor {
        id: "claude",
        display_name: "Claude Code",
        transport: "template",
        entrypoint: "/lane claude <task>",
        binary: Some("claude"),
        config_env: Some("VIDEN_LANE_CLAUDE_TEMPLATE"),
        config_label: "template",
        config_required: true,
    },
    AgentAdapterDescriptor {
        id: "custom-template",
        display_name: "Custom template agent",
        transport: "template",
        entrypoint: "/lane ask <tool> <task>",
        binary: None,
        config_env: Some("VIDEN_LANE_<TOOL>_TEMPLATE"),
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
        config_env: Some("VIDEN_LANE_PTY_TEMPLATE"),
        config_label: "template",
        config_required: false,
    },
    AgentAdapterDescriptor {
        id: "acp",
        display_name: "ACP agent server",
        transport: "acp",
        entrypoint: "/lane acp <agent> <task> (experimental)",
        binary: None,
        config_env: Some("VIDEN_AGENT_ACP_COMMAND"),
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
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(render_agent_list()),
            "doctor" => Ok(render_agent_doctor(
                args.get(1).map(String::as_str),
                &self.cwd,
            )),
            "review" => handle_codex_review_command(&self.cwd, &args[1..]),
            "challenge" => handle_codex_challenge_command(&self.cwd, &args[1..]),
            "probe" => handle_agent_probe_command(&self.cwd, &args[1..]),
            "auth" => handle_agent_auth_command(&self.cwd, &args[1..]),
            "smoke" => self.handle_agent_smoke_command(&args[1..], approver),
            "run" => self.handle_agent_run_command(&args[1..], approver),
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
            "  /agent probe codex [--thread|--turn <task>|--turn-write <task>]",
            "  /agent probe acp <agent-id>",
            "  /agent auth acp <agent-id> [method-id]",
            "  /agent smoke acp [--live]",
            "  /agent run acp [--async] [--load-session <id>] [--mode <mode-id>] [--model <model-id>] <agent-id> <task>",
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

    fn handle_agent_run_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str) {
            Some("acp") => {
                let parsed = parse_acp_run_args(&args[1..])?;
                handle_acp_agent_run_command(
                    &self.cwd,
                    parsed,
                    approver,
                    self.permissions.context_snapshot(),
                    self.runtime_event_sink(),
                )
            }
            _ => self.handle_codex_run_command(args, approver),
        }
    }

    fn handle_agent_smoke_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    {
        match args.first().map(String::as_str) {
            Some("acp") => {
                let live = args.iter().any(|arg| arg == "--live");
                run_acp_smoke_gate(&self.cwd, live, approver)
            }
            _ => Err("Usage: /agent smoke acp [--live]".to_string()),
        }
    }

    fn handle_codex_run_command<F>(
        &mut self,
        args: &[String],
        approver: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
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
        if parsed.write
            && let Some(denial) = self.ensure_codex_write_permission(&parsed.task, approver)?
        {
            return Ok(denial);
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
        F: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
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
            decision = self
                .permissions
                .apply_approval(approval, ask, &tool, &input);
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

fn handle_agent_probe_command(cwd: &Path, args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("codex") => handle_codex_probe_command(cwd, args),
        Some("acp") => handle_acp_agent_probe_command(cwd, args.get(1).map(String::as_str)),
            Some(id) if acp_agent_descriptors().iter().any(|agent| agent.agent_id == id) => {
                handle_acp_agent_probe_command(cwd, Some(id))
            }
        Some(other) => Err(format!(
            "Unsupported probe target `{other}`. Use `codex` or `acp <agent-id>`."
        )),
        None => Err(
            "Usage: /agent probe codex [--thread|--turn <task>|--turn-write <task>] | /agent probe acp <agent-id>"
                .to_string(),
        ),
    }
}

fn handle_agent_auth_command(cwd: &Path, args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("acp") => {
            let agent_id = args.get(1).map(String::as_str);
            let method_id = args.get(2).map(String::as_str);
            handle_acp_agent_auth_command(cwd, agent_id, method_id)
        }
        _ => Err("Usage: /agent auth acp <agent-id> [method-id]".to_string()),
    }
}

fn handle_acp_agent_probe_command(cwd: &Path, target: Option<&str>) -> Result<String, String> {
    let id = target.ok_or_else(|| "Usage: /agent probe acp <agent-id>".to_string())?;
    let agents = acp_agent_descriptors();
    let Some(agent) = agents.iter().find(|agent| agent.agent_id == id) else {
        let known = agents
            .iter()
            .map(|agent| agent.agent_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Unknown ACP agent `{id}`. Known ACP agents: {known}"
        ));
    };
    if !matches!(agent.transport, AgentTransport::Acp) {
        return Err(format!("Agent `{id}` does not use ACP transport."));
    }
    let evidence = run_acp_initialize_probe_for_agent(cwd, agent)?;
    let auth = if evidence.auth_methods.is_empty() {
        "none advertised".to_string()
    } else {
        evidence.auth_methods.join(", ")
    };
    let capabilities = if evidence.capabilities.is_empty() {
        "none advertised".to_string()
    } else {
        evidence.capabilities.join(", ")
    };
    Ok(format!(
        "ACP initialize probe ok.\n  agent: {} ({})\n  command: {}\n  protocol: {}\n  remote: {}\n  auth: {}\n  capabilities: {}\n  log: {}",
        agent.agent_id,
        agent.display_name,
        agent_command_line(agent),
        evidence.protocol_version,
        evidence.agent_label,
        auth,
        capabilities,
        evidence.log_path.display()
    ))
}

fn handle_acp_agent_auth_command(
    cwd: &Path,
    target: Option<&str>,
    method_id: Option<&str>,
) -> Result<String, String> {
    let id = target.ok_or_else(|| "Usage: /agent auth acp <agent-id> [method-id]".to_string())?;
    let agents = acp_agent_descriptors();
    let Some(agent) = agents.iter().find(|agent| agent.agent_id == id) else {
        let known = agents
            .iter()
            .map(|agent| agent.agent_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Unknown ACP agent `{id}`. Known ACP agents: {known}"
        ));
    };
    if agent.agent_id == "kiro-cli" {
        return Ok(render_kiro_native_auth_instructions(agent));
    }
    let evidence = run_acp_authenticate_for_agent(cwd, agent, method_id)?;
    Ok(format!(
        "ACP authenticate completed.\n  agent: {} ({})\n  method: {}\n  status: {}\n  log: {}",
        agent.agent_id,
        agent.display_name,
        evidence.method_id,
        evidence.status,
        evidence.log_path.display()
    ))
}

fn render_kiro_native_auth_instructions(agent: &AgentPluginDescriptor) -> String {
    format!(
        "Kiro CLI uses native authentication.\n  agent: {} ({})\n  command: {}\n  login: kiro-cli login --use-device-flow\n  verify: kiro-cli doctor\n  gate: /agent smoke acp --live\n  note: Viden does not store Kiro credentials; Kiro owns auth, billing, and agent configuration.",
        agent.agent_id,
        agent.display_name,
        agent_command_line(agent),
    )
}

fn acp_agent_descriptors() -> Vec<AgentPluginDescriptor> {
    let mut agents = builtin_agent_descriptors();
    if let Some(custom) = custom_acp_agent_descriptor_from_env() {
        agents.push(custom);
    }
    agents
}

pub(crate) fn typed_agent_adapter_views() -> Vec<AgentAdapterView> {
    acp_agent_descriptors()
        .iter()
        .map(typed_agent_adapter_view)
        .collect()
}

pub(crate) fn probe_typed_agent_adapter(
    cwd: &Path,
    agent_id: &str,
) -> Result<AgentAdapterView, String> {
    let agents = acp_agent_descriptors();
    let Some(agent) = agents.iter().find(|agent| agent.agent_id == agent_id) else {
        let known = agents
            .iter()
            .map(|agent| agent.agent_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Unknown ACP agent `{agent_id}`. Known ACP agents: {known}"
        ));
    };
    let mut view = typed_agent_adapter_view(agent);
    match run_acp_initialize_probe_for_agent(cwd, agent) {
        Ok(evidence) => {
            view.availability = AgentAvailability::Available;
            view.auth_state = AgentAuthState::Unknown;
            view.diagnostics = if evidence.auth_methods.is_empty() {
                Vec::new()
            } else {
                vec![format!(
                    "agent-native authentication methods advertised: {}",
                    evidence.auth_methods.join(", ")
                )]
            };
        }
        Err(error) => {
            let lower = error.to_ascii_lowercase();
            if lower.contains("not logged in")
                || lower.contains("not authenticated")
                || lower.contains("please log in")
            {
                view.availability = AgentAvailability::NeedsAuth;
                view.auth_state = AgentAuthState::LoggedOut;
            } else if !command_exists(&agent.command.command) {
                view.availability = if matches!(agent.source, AgentSource::Registry) {
                    AgentAvailability::NeedsInstall
                } else {
                    AgentAvailability::Unavailable
                };
                view.auth_state = AgentAuthState::Unknown;
            } else {
                view.availability = AgentAvailability::Unavailable;
                view.auth_state = AgentAuthState::Error;
            }
            // Agent-native stderr may contain credential material. The typed
            // frontend projection carries only a classified diagnostic.
            view.diagnostics = vec![if view.availability == AgentAvailability::NeedsAuth {
                "agent-native authentication is required".to_string()
            } else if view.availability == AgentAvailability::NeedsInstall {
                format!("agent command `{}` is not installed", agent.command.command)
            } else {
                "agent initialize probe failed; inspect local agent logs".to_string()
            }];
        }
    }
    view.startability = classify_agent_startability(view.availability, view.auth_state);
    Ok(view)
}

fn classify_agent_startability(
    availability: AgentAvailability,
    auth_state: AgentAuthState,
) -> AgentStartability {
    match (availability, auth_state) {
        (AgentAvailability::Available, AgentAuthState::Ready) => AgentStartability::Ready,
        (AgentAvailability::Available, AgentAuthState::Unknown) => AgentStartability::ProbeRequired,
        (AgentAvailability::NeedsInstall, _) => AgentStartability::InstallRequired,
        (AgentAvailability::NeedsAuth, _) | (_, AgentAuthState::LoggedOut) => {
            AgentStartability::AuthenticationRequired
        }
        _ => AgentStartability::Unavailable,
    }
}

fn typed_agent_adapter_view(agent: &AgentPluginDescriptor) -> AgentAdapterView {
    let installed = command_exists(&agent.command.command);
    let availability = if installed {
        AgentAvailability::Available
    } else if matches!(agent.source, AgentSource::Registry) {
        AgentAvailability::NeedsInstall
    } else {
        AgentAvailability::Unavailable
    };
    let diagnostics = if installed && agent.auth_modes.contains(&AgentAuthMode::AgentNative) {
        vec![agent_auth_hint(agent).to_string()]
    } else if installed {
        Vec::new()
    } else {
        vec![format!(
            "command `{}` is not installed",
            agent.command.command
        )]
    };
    AgentAdapterView {
        agent_id: agent.agent_id.clone(),
        display_name: agent.display_name.clone(),
        route: AgentRoute::Acp,
        source: match agent.source {
            AgentSource::Registry => AgentAdapterSource::Registry,
            AgentSource::LocalCommand | AgentSource::Custom => AgentAdapterSource::LocalCommand,
        },
        availability,
        auth_state: AgentAuthState::Unknown,
        startability: classify_agent_startability(availability, AgentAuthState::Unknown),
        capabilities: agent
            .capabilities
            .iter()
            .map(|capability| CapabilityId(agent_capability_id(capability).to_string()))
            .collect(),
        models: Vec::new(),
        diagnostics,
    }
}

fn custom_acp_agent_descriptor_from_env() -> Option<AgentPluginDescriptor> {
    let command = env::var("VIDEN_AGENT_ACP_COMMAND")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    Some(custom_acp_agent_descriptor(&command))
}

fn custom_acp_agent_descriptor(command: &str) -> AgentPluginDescriptor {
    let (program, args) = shell_descriptor_command(command);
    AgentPluginDescriptor {
        agent_id: "custom-acp".to_string(),
        display_name: "Custom ACP agent".to_string(),
        version: "custom".to_string(),
        transport: AgentTransport::Acp,
        source: AgentSource::LocalCommand,
        command: AgentCommandSpec {
            command: program,
            args,
            env: Vec::new(),
        },
        registry_package: None,
        protocol_versions: vec![AgentProtocolVersion::AcpV1],
        auth_modes: vec![AgentAuthMode::AgentNative],
        capabilities: vec![
            AgentPluginCapability::SessionPrompt,
            AgentPluginCapability::StreamingUpdates,
            AgentPluginCapability::ToolCalls,
            AgentPluginCapability::SessionCancel,
        ],
        permission_profile: AgentPermissionProfile::RuntimeGated,
        experimental_methods: Vec::new(),
        config_schema_version: 1,
    }
}

fn shell_descriptor_command(command: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        (
            "cmd".to_string(),
            vec!["/C".to_string(), command.to_string()],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-lc".to_string(), command.to_string()],
        )
    }
}

fn run_acp_smoke_gate(
    cwd: &Path,
    live: bool,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
) -> Result<String, String> {
    let agents = acp_agent_descriptors();
    run_acp_smoke_gate_for_agents(cwd, &agents, live, approver)
}

fn run_acp_smoke_gate_for_agents(
    cwd: &Path,
    agents: &[AgentPluginDescriptor],
    live: bool,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
) -> Result<String, String> {
    let mut lines = vec![format!(
        "ACP smoke gate ({})",
        if live { "live session" } else { "initialize" }
    )];
    let mut failed = 0usize;
    let mut blocked = 0usize;
    for agent in agents {
        if !matches!(agent.transport, AgentTransport::Acp) {
            continue;
        }
        let result = if live {
            run_acp_session_prompt_for_agent(
                cwd,
                agent,
                "Reply with OK only.",
                AcpSessionOptions::default(),
                approver,
            )
            .map(|evidence| {
                format!(
                    "ok session={} status={} usage={}",
                    evidence.session_id,
                    evidence.final_status,
                    evidence
                        .usage_summary
                        .unwrap_or_else(|| "unavailable".to_string())
                )
            })
        } else {
            run_acp_initialize_probe_for_agent(cwd, agent).map(|evidence| {
                let auth = if evidence.auth_methods.is_empty() {
                    "none advertised".to_string()
                } else {
                    evidence.auth_methods.join(", ")
                };
                format!(
                    "ok protocol={} remote={} auth={}",
                    evidence.protocol_version, evidence.agent_label, auth
                )
            })
        };
        match result {
            Ok(summary) => lines.push(format!("  PASS {}: {}", agent.agent_id, summary)),
            Err(error) => {
                let classification = classify_acp_smoke_error(agent, &error);
                if classification == "blocked-auth" {
                    blocked += 1;
                } else {
                    failed += 1;
                }
                lines.push(format!(
                    "  {} {}: {}",
                    if classification == "blocked-auth" {
                        "BLOCKED"
                    } else {
                        "FAIL"
                    },
                    agent.agent_id,
                    smoke_error_summary(&error)
                ));
            }
        }
    }
    lines.push(format!(
        "summary: {} failed, {} blocked-auth",
        failed, blocked
    ));
    if failed > 0 || blocked > 0 {
        return Err(lines.join("\n"));
    }
    Ok(lines.join("\n"))
}

fn classify_acp_smoke_error(_agent: &AgentPluginDescriptor, error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not authenticated")
        || lower.contains("please log in")
    {
        "blocked-auth"
    } else if lower.contains("timed out") {
        "timeout"
    } else {
        "failed"
    }
}

fn smoke_error_summary(error: &str) -> String {
    let mut summary = truncate_for_line(error.lines().next().unwrap_or(error), 220);
    if summary.contains("timed out") {
        summary
            .push_str(" (increase VIDEN_ACP_SESSION_TIMEOUT_SECS or run provider-native doctor)");
    }
    summary
}

fn handle_acp_agent_run_command(
    cwd: &Path,
    run: AcpRunArgs,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    permission_context: PermissionContext,
    runtime_event_sink: Option<RuntimeEventSink>,
) -> Result<String, String> {
    let agents = acp_agent_descriptors();
    handle_acp_agent_run_command_with_agents(
        cwd,
        &agents,
        run,
        approver,
        permission_context,
        runtime_event_sink,
    )
}

fn handle_acp_agent_run_command_with_agents(
    cwd: &Path,
    agents: &[AgentPluginDescriptor],
    run: AcpRunArgs,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    permission_context: PermissionContext,
    runtime_event_sink: Option<RuntimeEventSink>,
) -> Result<String, String> {
    let Some(agent) = agents.iter().find(|agent| agent.agent_id == run.agent_id) else {
        let known = agents
            .iter()
            .map(|agent| agent.agent_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Unknown ACP agent `{}`. Known ACP agents: {known}",
            run.agent_id
        ));
    };
    if !matches!(agent.transport, AgentTransport::Acp) {
        return Err(format!(
            "Agent `{}` does not use ACP transport.",
            run.agent_id
        ));
    }
    if run.async_job {
        return Err(
            "asynchronous ACP sessions must be submitted through RuntimeSupervisor so approvals, cancellation, and replay remain owner-scoped"
                .to_string(),
        );
    }
    let evidence = run_acp_session_prompt_for_agent_with_permissions(
        cwd,
        agent,
        &run.task,
        run.session,
        approver,
        permission_context,
        runtime_event_sink,
    )?;
    let tool_calls = if evidence.tool_calls.is_empty() {
        "none".to_string()
    } else {
        evidence.tool_calls.join(", ")
    };
    let message = if evidence.message.trim().is_empty() {
        "<empty>".to_string()
    } else {
        evidence.message
    };
    let usage = evidence
        .usage_summary
        .unwrap_or_else(|| "unavailable".to_string());
    Ok(format!(
        "ACP session completed.\n  agent: {} ({})\n  session: {}\n  status: {}\n  message: {}\n  tool_calls: {}\n  usage: {}\n  log: {}",
        agent.agent_id,
        agent.display_name,
        evidence.session_id,
        evidence.final_status,
        message,
        tool_calls,
        usage,
        evidence.log_path.display()
    ))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AcpRunArgs {
    async_job: bool,
    agent_id: String,
    task: String,
    session: AcpSessionOptions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AcpSessionOptions {
    load_session_id: Option<String>,
    mode_id: Option<String>,
    model_id: Option<String>,
}

struct AcpSessionPromptRunContext<'a, A, P>
where
    A: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    P: FnMut(u32),
{
    approver: &'a mut A,
    log_path: PathBuf,
    cancel_path: Option<PathBuf>,
    runtime_event_log_path: Option<PathBuf>,
    permission_context: PermissionContext,
    runtime_event_sink: Option<RuntimeEventSink>,
    on_pid: P,
}

fn parse_acp_run_args(args: &[String]) -> Result<AcpRunArgs, String> {
    let usage = "Usage: /agent run acp [--async] [--load-session <id>] [--mode <mode-id>] [--model <model-id>] <agent-id> <task>";
    let mut parsed = AcpRunArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--async" => {
                parsed.async_job = true;
                i += 1;
            }
            "--load-session" => {
                parsed.session.load_session_id = Some(
                    args.get(i + 1)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| usage.to_string())?
                        .clone(),
                );
                i += 2;
            }
            "--mode" => {
                parsed.session.mode_id = Some(
                    args.get(i + 1)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| usage.to_string())?
                        .clone(),
                );
                i += 2;
            }
            "--model" => {
                parsed.session.model_id = Some(
                    args.get(i + 1)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| usage.to_string())?
                        .clone(),
                );
                i += 2;
            }
            value if value.starts_with("--") => {
                return Err(format!("Unknown ACP run option `{value}`.\n{usage}"));
            }
            _ => break,
        }
    }
    parsed.agent_id = args
        .get(i)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| usage.to_string())?
        .clone();
    parsed.task = args.get(i + 1..).unwrap_or_default().join(" ");
    if parsed.task.trim().is_empty() {
        return Err(usage.to_string());
    }
    Ok(parsed)
}

pub(crate) fn typed_agent_session_request_from_compat_input(
    input: &str,
    lane_id: Option<&str>,
) -> Option<Result<AgentSessionRequest, String>> {
    let args = input
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("/agent")
        || args.get(1).map(String::as_str) != Some("run")
        || args.get(2).map(String::as_str) != Some("acp")
    {
        return None;
    }
    let parsed = match parse_acp_run_args(&args[3..]) {
        Ok(parsed) if parsed.async_job => parsed,
        Ok(_) => return None,
        Err(error) => return Some(Err(error)),
    };
    let Some(lane_id) = lane_id else {
        return Some(Err(
            "asynchronous ACP sessions require a lane-scoped runtime owner".to_string(),
        ));
    };
    if parsed.session.mode_id.is_some() {
        return Some(Err(
            "typed asynchronous ACP sessions do not yet accept --mode; select policy through the lane contract"
                .to_string(),
        ));
    }
    Some(Ok(AgentSessionRequest {
        lane_id: lane_id.to_string(),
        agent_id: parsed.agent_id,
        model: parsed.session.model_id,
        load_session_id: parsed.session.load_session_id,
        task: parsed.task,
    }))
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
        "  id               transport  readiness      mutation".to_string(),
    ];
    lines.extend(AGENT_ADAPTERS.into_iter().map(|adapter| {
        let capability = adapter_capability(adapter);
        format!(
            "  {:<16} {:<10} {:<14} {}",
            capability.id, capability.transport, capability.readiness, capability.mutation_mode
        )
    }));
    lines.extend(acp_agent_descriptors().into_iter().map(|agent| {
        format!(
            "  {:<16} {:<10} {:<14} {}",
            agent.agent_id,
            agent_transport_label(&agent),
            agent_descriptor_readiness(&agent),
            "agent-native; permission-gated by ACP events"
        )
    }));
    lines.push(String::new());
    lines.push(
        "Use `/agent doctor [id]` for binary, capability, evidence, and transport details."
            .to_string(),
    );
    lines.join("\n")
}

fn render_agent_doctor(target: Option<&str>, cwd: &Path) -> String {
    let agent_descriptors = acp_agent_descriptors();
    let adapters = if let Some(id) = target {
        AGENT_ADAPTERS
            .into_iter()
            .find(|adapter| adapter.id == id)
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        AGENT_ADAPTERS.to_vec()
    };
    let agents = if let Some(id) = target {
        agent_descriptors
            .into_iter()
            .find(|agent| agent.agent_id == id)
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        agent_descriptors
    };
    if target.is_some() && adapters.is_empty() && agents.is_empty() {
        let id = target.unwrap_or_default();
        return format!("Unknown agent adapter `{id}`. Use `/agent list` to see known adapters.");
    }
    let mut lines = vec!["Agent diagnostics:".to_string()];
    for adapter in adapters {
        let capability = adapter_capability(adapter);
        lines.push(format!("  {} ({})", capability.id, capability.display_name));
        lines.push(format!("    transport: {}", capability.transport));
        lines.push(format!("    readiness: {}", capability.readiness));
        lines.push(format!("    entrypoint: {}", capability.entrypoint));
        lines.push(format!("    mutation: {}", capability.mutation_mode));
        lines.push(format!("    evidence: {}", capability.evidence_mode));
        if let Some(config_source) = capability.config_source.as_deref() {
            lines.push(format!("    config source: {config_source}"));
        }
        if !capability.known_limits.is_empty() {
            lines.push(format!(
                "    limits: {}",
                capability.known_limits.join("; ")
            ));
        }
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
                "    binary: resolved from VIDEN_AGENT_ACP_COMMAND when configured".to_string(),
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
    for agent in agents {
        lines.extend(render_agent_descriptor_doctor(&agent));
    }
    lines.join("\n")
}

fn render_agent_descriptor_doctor(agent: &AgentPluginDescriptor) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("  {} ({})", agent.agent_id, agent.display_name));
    lines.push(format!("    transport: {}", agent_transport_label(agent)));
    lines.push(format!("    source: {}", agent_source_label(agent)));
    lines.push(format!(
        "    permission: {}",
        agent_permission_profile_label(agent)
    ));
    lines.push(format!(
        "    readiness: {}",
        agent_descriptor_readiness(agent)
    ));
    lines.push(format!("    auth: {}", agent_auth_hint(agent)));
    lines.push(format!("    command: {}", agent_command_line(agent)));
    if let Some(package) = &agent.registry_package {
        lines.push(format!(
            "    registry: {}@{}",
            package.package, package.version
        ));
    }
    if !agent.capabilities.is_empty() {
        lines.push(format!(
            "    capabilities: {}",
            agent
                .capabilities
                .iter()
                .map(agent_capability_label)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !agent.experimental_methods.is_empty() {
        lines.push(format!(
            "    experimental: {}",
            agent.experimental_methods.join(", ")
        ));
    }
    lines.push("    mutation: agent-native; Viden must gate local tool effects".to_string());
    lines.push("    evidence: ACP stream, tool updates, turn end, session log".to_string());
    lines
}

fn agent_transport_label(agent: &AgentPluginDescriptor) -> &'static str {
    match agent.transport {
        AgentTransport::Acp => "acp",
        AgentTransport::AppServer => "app-server",
        AgentTransport::Template => "template",
        AgentTransport::Pty => "pty",
        AgentTransport::Tmux => "tmux",
    }
}

fn agent_source_label(agent: &AgentPluginDescriptor) -> &'static str {
    match agent.source {
        AgentSource::Registry => "registry",
        AgentSource::LocalCommand => "local-command",
        AgentSource::Custom => "custom",
    }
}

fn agent_permission_profile_label(agent: &AgentPluginDescriptor) -> &'static str {
    match agent.permission_profile {
        AgentPermissionProfile::ReadOnlyProbe => "read-only-probe",
        AgentPermissionProfile::RuntimeGated => "runtime-gated",
        AgentPermissionProfile::AgentNative => "agent-native",
    }
}

fn agent_descriptor_readiness(agent: &AgentPluginDescriptor) -> &'static str {
    if command_exists(&agent.command.command) {
        if agent.auth_modes.contains(&AgentAuthMode::AgentNative) {
            "installed; auth unknown"
        } else {
            "ready"
        }
    } else if matches!(agent.source, AgentSource::Registry) {
        "setup needed"
    } else {
        "missing"
    }
}

fn agent_auth_hint(agent: &AgentPluginDescriptor) -> &'static str {
    if agent.agent_id == "kiro-cli" {
        "agent-native; run `kiro-cli login` and `kiro-cli doctor`"
    } else if agent.auth_modes.contains(&AgentAuthMode::AgentNative) {
        "agent-native; use the agent's own login/config"
    } else {
        "none declared"
    }
}

fn agent_command_line(agent: &AgentPluginDescriptor) -> String {
    let mut parts = vec![agent.command.command.clone()];
    parts.extend(acp_agent_command_args(agent));
    parts.join(" ")
}

fn acp_agent_command_args(agent: &AgentPluginDescriptor) -> Vec<String> {
    let mut args = agent.command.args.clone();
    if is_kiro_agent(agent) {
        push_env_arg(&mut args, "--agent", "VIDEN_KIRO_AGENT");
        push_env_arg(&mut args, "--model", "VIDEN_KIRO_MODEL");
        push_env_arg(&mut args, "--effort", "VIDEN_KIRO_EFFORT");
        if env_flag_enabled("VIDEN_KIRO_TRUST_ALL_TOOLS")
            && !args.iter().any(|arg| arg == "--trust-all-tools")
        {
            args.push("--trust-all-tools".to_string());
        } else {
            push_env_arg(&mut args, "--trust-tools", "VIDEN_KIRO_TRUST_TOOLS");
        }
        push_env_arg(&mut args, "--agent-engine", "VIDEN_KIRO_AGENT_ENGINE");
    }
    args
}

fn is_kiro_agent(agent: &AgentPluginDescriptor) -> bool {
    agent.agent_id == "kiro-cli" || agent.command.command == "kiro-cli"
}

fn push_env_arg(args: &mut Vec<String>, flag: &str, env_name: &str) {
    if args.iter().any(|arg| arg == flag) {
        return;
    }
    let Ok(value) = env::var(env_name) else {
        return;
    };
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    args.push(flag.to_string());
    args.push(value.to_string());
}

fn env_flag_enabled(env_name: &str) -> bool {
    env::var(env_name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn agent_capability_label(capability: &AgentPluginCapability) -> &'static str {
    match capability {
        AgentPluginCapability::SessionPrompt => "session/prompt",
        AgentPluginCapability::SessionLoad => "session/load",
        AgentPluginCapability::SessionCancel => "session/cancel",
        AgentPluginCapability::SessionSetMode => "session/set_mode",
        AgentPluginCapability::SessionSetModel => "session/set_model",
        AgentPluginCapability::StreamingUpdates => "streaming",
        AgentPluginCapability::ToolCalls => "tool_calls",
        AgentPluginCapability::ImageInput => "image_input",
        AgentPluginCapability::SlashCommands => "slash_commands",
        AgentPluginCapability::McpEvents => "mcp_events",
    }
}

fn agent_capability_id(capability: &AgentPluginCapability) -> &'static str {
    match capability {
        AgentPluginCapability::SessionPrompt => "agent.session.prompt",
        AgentPluginCapability::SessionLoad => "agent.session.load",
        AgentPluginCapability::SessionCancel => "agent.session.cancel",
        AgentPluginCapability::SessionSetMode => "agent.session.set-mode",
        AgentPluginCapability::SessionSetModel => "agent.session.set-model",
        AgentPluginCapability::StreamingUpdates => "agent.streaming",
        AgentPluginCapability::ToolCalls => "agent.tool-calls",
        AgentPluginCapability::ImageInput => "agent.image-input",
        AgentPluginCapability::SlashCommands => "agent.slash-commands",
        AgentPluginCapability::McpEvents => "agent.mcp-events",
    }
}

fn adapter_capability(adapter: AgentAdapterDescriptor) -> AgentCapabilityRecord {
    let known_limits = match adapter.id {
        "codex" => vec![
            "write-capable runs require Viden approval".to_string(),
            "app-server write probe is guarded by an experimental flag".to_string(),
        ],
        "claude" => vec![
            "requires a configured lane template before execution".to_string(),
            "review/apply evidence is captured through lane artifacts".to_string(),
        ],
        "custom-template" => vec![
            "resolved dynamically from the selected tool name".to_string(),
            "template output must emit inspectable artifacts for review".to_string(),
        ],
        "tmux" => vec![
            "operator-controlled interactive lane".to_string(),
            "requires tmux and terminal-side task discipline".to_string(),
        ],
        "pty" => vec![
            "embedded PTY lane is platform/template dependent".to_string(),
            "review/apply still flows through lane evidence".to_string(),
        ],
        "acp" => vec![
            "descriptor probe, minimal session run, and tracked async session jobs".to_string(),
            "ACP file/terminal mutation is still gated until runtime bridging lands".to_string(),
        ],
        _ => Vec::new(),
    };
    AgentCapabilityRecord {
        id: adapter.id.to_string(),
        display_name: adapter.display_name.to_string(),
        transport: adapter.transport.to_string(),
        readiness: adapter_readiness(adapter).to_string(),
        entrypoint: adapter.entrypoint.to_string(),
        mutation_mode: adapter_mutation_mode(adapter).to_string(),
        evidence_mode: adapter_evidence_mode(adapter).to_string(),
        config_source: adapter.config_env.map(str::to_string),
        known_limits,
    }
}

fn adapter_mutation_mode(adapter: AgentAdapterDescriptor) -> &'static str {
    match adapter.id {
        "codex" => "read-only by default; workspace-write requires approval",
        "claude" | "custom-template" => "template-defined; isolate before apply",
        "tmux" | "pty" => "interactive; operator-controlled",
        "acp" => "agent-native; mutating file/terminal requests require runtime bridge",
        _ => "unknown",
    }
}

fn adapter_evidence_mode(adapter: AgentAdapterDescriptor) -> &'static str {
    match adapter.id {
        "codex" => "job result, protocol/app-server log, lane artifacts",
        "claude" | "custom-template" => "lane log, envelope, timeline, artifacts",
        "tmux" | "pty" => "terminal tail, lane log, timeline, artifacts",
        "acp" => "JSONL wire log, session result, permission decisions",
        _ => "unknown",
    }
}

fn render_agent_logs_help() -> String {
    [
        "Agent logs:",
        "  /agent result <id> shows tracked Codex or ACP agent job output.",
        "  Use `/lane inspect <id>` for lane logs, artifacts, decisions, and transport evidence.",
    ]
    .join("\n")
}

pub(crate) fn tracked_agent_job_tasks(cwd: &Path) -> Vec<AgentTaskRecord> {
    latest_codex_jobs(cwd)
        .unwrap_or_default()
        .into_iter()
        .map(|job| agent_task_from_job_record(cwd, job))
        .collect()
}

pub(crate) fn tracked_agent_job_sessions(cwd: &Path) -> Vec<AgentSessionView> {
    latest_codex_jobs(cwd)
        .unwrap_or_default()
        .into_iter()
        .filter(|job| job.kind == "acp-session")
        .filter_map(|mut job| {
            let interrupted_status = matches!(job.status.as_str(), "running" | "waiting_approval")
                .then(|| job.status.clone());
            let recovery_error = if interrupted_status.is_some() {
                // A replacement Core cannot reattach stdio or an approval
                // receiver. Stop the orphan before publishing recovery state.
                match cancel_codex_job(cwd, Some(&job.id)) {
                    Ok(_) => {
                        job.status = "failed".to_string();
                        job.updated_at = timestamp_millis();
                        let _ = append_codex_job_record(cwd, "recovered_after_restart", &job);
                        None
                    }
                    Err(error) => Some(error),
                }
            } else {
                None
            };
            let metadata = job.agent.take()?;
            let lane_id = metadata.owner.lane_id.clone()?;
            let stored_status = interrupted_status
                .clone()
                .unwrap_or_else(|| observed_codex_status(&job));
            let status = if recovery_error.is_some() {
                if stored_status == "waiting_approval" {
                    AgentSessionStatus::WaitingApproval
                } else {
                    AgentSessionStatus::Running
                }
            } else {
                match stored_status.as_str() {
                "cancelled" => AgentSessionStatus::Cancelled,
                "failed" => AgentSessionStatus::Failed,
                "finished" => AgentSessionStatus::Completed,
                // A new Core cannot resume the old process' stdio or approval
                // channel. Surface an explicit recoverable failure instead of
                // pretending the external session is still controllable.
                "waiting_approval" | "running" => AgentSessionStatus::Failed,
                _ => AgentSessionStatus::Failed,
                }
            };
            Some(AgentSessionView {
                session_id: job.id,
                lane_id,
                agent_id: metadata.agent_id,
                model: metadata.model,
                status,
                owner: metadata.owner,
                task: job.task,
                diagnostic: recovery_error.map(|error| {
                    format!("Core restart could not stop the orphaned ACP process; retry cancel: {error}")
                }).or_else(|| (status == AgentSessionStatus::Failed).then(|| {
                    if stored_status == "waiting_approval" {
                        "Core restarted while ACP approval was pending; start a new session"
                            .to_string()
                    } else if stored_status == "running" {
                        "Core restarted while the ACP session was running; start a new session"
                            .to_string()
                    } else {
                        "restored failed ACP session".to_string()
                    }
                })),
            })
        })
        .collect()
}

fn agent_task_from_job_record(cwd: &Path, mut job: CodexJobRecord) -> AgentTaskRecord {
    job.status = observed_codex_status(&job);
    let evidence = codex_job_evidence(cwd, &job);
    let mut evidence_lines = Vec::new();
    if let Some(session_id) = &evidence.session_id {
        if job.kind == "acp-session" {
            evidence_lines.push(format!("session {session_id}"));
        } else {
            evidence_lines.push(format!("resume {session_id}"));
        }
    }
    evidence_lines.extend(
        evidence
            .files
            .into_iter()
            .take(8)
            .map(|file| format!("file {file}")),
    );
    AgentTaskRecord {
        id: job.id.clone(),
        parent_id: None,
        role: AgentRole::Coder,
        kind: AgentTaskKind::Job,
        route: agent_job_route(&job),
        title: job.task.clone(),
        status: agent_job_task_status(&job.status),
        activity: agent_job_activity(&job, &evidence_lines),
        summary: job.task.clone(),
        progress: agent_job_progress(&job.status),
        started_at: None,
        updated_at: Some(job.updated_at.min(u128::from(u64::MAX)) as u64),
        workspace: None,
        evidence: evidence_lines,
        permissions: agent_job_permissions(&job),
        decision: None,
        result: Some(job.result_path.display().to_string()),
        resume_handle: evidence.session_id.filter(|_| job.kind != "acp-session"),
        pid: job.pid,
        next_action: Some(AgentNextAction {
            label: "inspect agent".to_string(),
            command: Some(format!("/agent result {}", job.id)),
            reason: Some("tracked agent job state is available".to_string()),
        }),
    }
}

fn agent_job_route(job: &CodexJobRecord) -> AgentRoute {
    if job.kind == "acp-session" {
        AgentRoute::Acp
    } else {
        AgentRoute::Terminal
    }
}

fn agent_job_task_status(status: &str) -> AgentTaskStatus {
    match status {
        "queued" => AgentTaskStatus::Queued,
        "running" => AgentTaskStatus::Thinking,
        "finished" | "observed" => AgentTaskStatus::Done,
        "failed" => AgentTaskStatus::Failed,
        "cancelled" | "canceled" => AgentTaskStatus::Cancelled,
        _ => AgentTaskStatus::Done,
    }
}

fn agent_job_progress(status: &str) -> u8 {
    match status {
        "queued" => 10,
        "running" => 65,
        "finished" | "observed" | "failed" | "cancelled" | "canceled" => 100,
        _ => 0,
    }
}

fn agent_job_activity(job: &CodexJobRecord, evidence: &[String]) -> String {
    match job.status.as_str() {
        "queued" => format!("queued: {}", job.task),
        "running" => evidence
            .first()
            .cloned()
            .unwrap_or_else(|| format!("running {}", job.kind)),
        "finished" | "observed" => "result ready".to_string(),
        "failed" => evidence
            .first()
            .cloned()
            .unwrap_or_else(|| "failed; inspect result".to_string()),
        status => format!("{status}: {}", job.task),
    }
}

fn agent_job_permissions(job: &CodexJobRecord) -> Vec<String> {
    if job.kind == "acp-session" {
        vec!["agent permission gated".to_string()]
    } else if job.kind.contains("write") || job.kind.contains("rescue") {
        vec!["workspace-write approval".to_string()]
    } else {
        vec!["read-only".to_string()]
    }
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
    if matches!(mode, CodexProbeMode::Turn { write: true, .. })
        && !env_is_configured("VIDEN_EXPERIMENTAL_CODEX_APP_SERVER_WRITE")
    {
        return Err(
            "`/agent probe codex --turn-write` is disabled by default because Codex app-server workspace-write turns can mutate files before Viden receives an approval request. Set VIDEN_EXPERIMENTAL_CODEX_APP_SERVER_WRITE=1 only in a disposable workspace."
                .to_string(),
        );
    }
    let evidence = run_codex_app_server_probe(cwd, &codex_command(), mode.clone())?;
    let tracked_job = if let CodexProbeMode::Turn { task, .. } = &mode {
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
                mode = CodexProbeMode::Turn { task, write: false };
                break;
            }
            "--turn-write" => {
                let task = args[index + 1..].join(" ");
                if task.trim().is_empty() {
                    return Err("Usage: /agent probe codex --turn-write <task>".to_string());
                }
                mode = CodexProbeMode::Turn { task, write: true };
                break;
            }
            other => {
                return Err(format!(
                    "Unknown Codex probe option `{other}`. Usage: /agent probe codex [--thread|--turn <task>|--turn-write <task>]"
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
    let Some(command) = env::var("VIDEN_AGENT_ACP_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        lines.push("    handshake: skipped (set VIDEN_AGENT_ACP_COMMAND)".to_string());
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
            lines.push("    next: install Codex with `npm install -g @openai/codex` or set VIDEN_AGENT_CODEX_COMMAND".to_string());
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
    Turn { task: String, write: bool },
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
    agent: Option<AgentJobMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentJobMetadata {
    agent_id: String,
    model: Option<String>,
    owner: RuntimeOwner,
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
        agent: None,
    };
    append_codex_job_record(cwd, "started", &record)?;
    let monitor_cwd = cwd.to_path_buf();
    let monitor_cancel_path = acp_job_cancel_path(cwd, &id);
    let mut monitor_record = record.clone();
    std::thread::spawn(move || {
        let status = child.wait();
        if monitor_cancel_path.exists()
            || find_codex_job(&monitor_cwd, &monitor_record.id)
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
        agent: None,
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
            CodexProbeMode::Turn {
                task: monitor_task,
                write: false,
            },
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

pub(crate) fn start_typed_agent_session(
    cwd: &Path,
    session_id: String,
    request: AgentSessionRequest,
    owner: RuntimeOwner,
    runtime_event_sink: RuntimeEventSink,
    mut approver: AgentSessionApprover,
) -> Result<AgentSessionView, String> {
    validate_typed_agent_session_request(&request)?;
    let agents = acp_agent_descriptors();
    let agent = agents
        .iter()
        .find(|agent| agent.agent_id == request.agent_id)
        .ok_or_else(|| {
            let known = agents
                .iter()
                .map(|agent| agent.agent_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Unknown ACP agent `{}`. Known ACP agents: {known}",
                request.agent_id
            )
        })?;
    if owner.lane_id.as_deref() != Some(request.lane_id.as_str())
        || owner.session_id.as_deref() != Some(session_id.as_str())
    {
        return Err("agent session owner must match the requested lane and session".to_string());
    }

    let log_path = codex_job_artifact_path(cwd, &session_id, "jsonl");
    let result_path = codex_job_artifact_path(cwd, &session_id, "result.md");
    let runtime_event_path = acp_job_runtime_events_path(cwd, &session_id);
    let baseline_path = codex_job_artifact_path(cwd, &session_id, "baseline.status");
    let cancel_path = acp_job_cancel_path(cwd, &session_id);
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    write_codex_status_baseline(cwd, &baseline_path)?;
    let record = CodexJobRecord {
        id: session_id.clone(),
        kind: "acp-session".to_string(),
        status: "running".to_string(),
        pid: None,
        command: agent_command_line(agent),
        task: request.task.clone(),
        log_path: log_path.clone(),
        result_path: result_path.clone(),
        baseline_path,
        updated_at: timestamp_millis(),
        agent: Some(AgentJobMetadata {
            agent_id: request.agent_id.clone(),
            model: request.model.clone(),
            owner: owner.clone(),
        }),
    };
    append_codex_job_record(cwd, "started", &record)?;

    let session = AgentSessionView {
        session_id: session_id.clone(),
        lane_id: request.lane_id,
        agent_id: request.agent_id,
        model: request.model.clone(),
        status: AgentSessionStatus::Starting,
        owner,
        task: request.task,
        diagnostic: None,
    };
    runtime_event_sink(vec![RuntimeEvent::new(
        0,
        RuntimeEventKind::AgentSessionStarted {
            session: session.clone(),
        },
    )]);

    let monitor_cwd = cwd.to_path_buf();
    let monitor_cancel_path = cancel_path.clone();
    let monitor_agent = agent.clone();
    let monitor_session = AcpSessionOptions {
        load_session_id: request.load_session_id,
        mode_id: None,
        model_id: request.model,
    };
    let monitor_runtime_event_path = runtime_event_path;
    let mut monitor_record = record;
    let monitor_view = session.clone();
    let terminal_sink = Arc::clone(&runtime_event_sink);
    let protocol_sink = Arc::clone(&runtime_event_sink);
    let pid_slot = Arc::new(Mutex::new(None::<u32>));
    let pid_slot_for_thread = Arc::clone(&pid_slot);
    std::thread::spawn(move || {
        let result = run_acp_session_prompt_for_agent_with_log(
            &monitor_cwd,
            &monitor_agent,
            &monitor_view.task,
            monitor_session,
            AcpSessionPromptRunContext {
                approver: &mut approver,
                log_path: log_path.clone(),
                cancel_path: Some(cancel_path),
                runtime_event_log_path: Some(monitor_runtime_event_path.clone()),
                permission_context: PermissionContext::default(),
                runtime_event_sink: Some(protocol_sink),
                on_pid: |pid| {
                    if let Ok(mut slot) = pid_slot_for_thread.lock() {
                        *slot = Some(pid);
                    }
                    let mut pid_record = monitor_record.clone();
                    pid_record.pid = Some(pid);
                    pid_record.updated_at = timestamp_millis();
                    let _ = append_codex_job_record(&monitor_cwd, "pid", &pid_record);
                },
            },
        );
        // The marker expresses owner intent, but cancellation becomes terminal
        // only here, after the ACP runner and its child process have stopped.
        let was_cancelled = monitor_cancel_path.exists()
            || find_codex_job(&monitor_cwd, &monitor_record.id)
                .ok()
                .flatten()
                .is_some_and(|job| job.status == "cancelled");
        monitor_record.pid = pid_slot.lock().ok().and_then(|slot| *slot);
        let mut terminal = monitor_view;
        let kind = match result {
            Ok(evidence) => {
                let _ =
                    write_acp_runtime_events(&monitor_runtime_event_path, &evidence.runtime_events);
                let _ = write_acp_session_result(&result_path, &evidence);
                monitor_record.status = acp_session_job_status(&evidence);
                if was_cancelled {
                    monitor_record.status = "cancelled".to_string();
                    terminal.status = AgentSessionStatus::Cancelled;
                    RuntimeEventKind::AgentSessionUpdated { session: terminal }
                } else if monitor_record.status == "failed" {
                    terminal.status = AgentSessionStatus::Failed;
                    terminal.diagnostic = Some("ACP session reported a failed status".to_string());
                    RuntimeEventKind::AgentSessionFailed { session: terminal }
                } else {
                    terminal.status = AgentSessionStatus::Completed;
                    RuntimeEventKind::AgentSessionCompleted { session: terminal }
                }
            }
            Err(error) => {
                if was_cancelled {
                    monitor_record.status = "cancelled".to_string();
                    terminal.status = AgentSessionStatus::Cancelled;
                    terminal.diagnostic = Some("cancelled by owner".to_string());
                    RuntimeEventKind::AgentSessionUpdated { session: terminal }
                } else {
                    let _ = fs::write(&result_path, format!("# ACP session failed\n\n{error}\n"));
                    monitor_record.status = "failed".to_string();
                    terminal.status = AgentSessionStatus::Failed;
                    terminal.diagnostic = Some(truncate_for_preview(&error, 320));
                    RuntimeEventKind::AgentSessionFailed { session: terminal }
                }
            }
        };
        monitor_record.updated_at = timestamp_millis();
        let _ = append_codex_job_record(&monitor_cwd, "completed", &monitor_record);
        terminal_sink(vec![RuntimeEvent::new(0, kind)]);
    });

    Ok(session)
}

pub(crate) fn validate_typed_agent_session_request(
    request: &AgentSessionRequest,
) -> Result<(), String> {
    let agents = acp_agent_descriptors();
    let agent = agents
        .iter()
        .find(|agent| agent.agent_id == request.agent_id)
        .ok_or_else(|| {
            let known = agents
                .iter()
                .map(|agent| agent.agent_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Unknown ACP agent `{}`. Known ACP agents: {known}",
                request.agent_id
            )
        })?;
    if !matches!(agent.transport, AgentTransport::Acp) {
        return Err(format!(
            "Agent `{}` does not use ACP transport.",
            request.agent_id
        ));
    }
    if !command_exists(&agent.command.command) {
        return Err(format!(
            "Agent `{}` is unavailable because command `{}` is not installed",
            request.agent_id, agent.command.command
        ));
    }
    Ok(())
}

#[cfg(test)]
fn start_acp_session_job(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
    task: String,
    session: AcpSessionOptions,
    runtime_event_sink: Option<RuntimeEventSink>,
) -> Result<String, String> {
    let id = format!("acp-{}", timestamp_millis());
    let log_path = codex_job_artifact_path(cwd, &id, "jsonl");
    let result_path = codex_job_artifact_path(cwd, &id, "result.md");
    let runtime_event_path = acp_job_runtime_events_path(cwd, &id);
    let baseline_path = codex_job_artifact_path(cwd, &id, "baseline.status");
    let cancel_path = acp_job_cancel_path(cwd, &id);
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    write_codex_status_baseline(cwd, &baseline_path)?;
    let record = CodexJobRecord {
        id: id.clone(),
        kind: "acp-session".to_string(),
        status: "running".to_string(),
        pid: None,
        command: agent_command_line(agent),
        task: task.clone(),
        log_path: log_path.clone(),
        result_path: result_path.clone(),
        baseline_path: baseline_path.clone(),
        updated_at: timestamp_millis(),
        agent: None,
    };
    append_codex_job_record(cwd, "started", &record)?;

    let monitor_cwd = cwd.to_path_buf();
    let monitor_cancel_path = cancel_path.clone();
    let monitor_agent = agent.clone();
    let monitor_task = task.clone();
    let monitor_session = session.clone();
    let monitor_runtime_event_path = runtime_event_path.clone();
    let mut monitor_record = record.clone();
    let pid_slot = Arc::new(Mutex::new(None::<u32>));
    let pid_slot_for_thread = Arc::clone(&pid_slot);
    std::thread::spawn(move || {
        // Legacy job mechanics are exercised only in unit tests; production
        // async ACP work is routed through the supervisor-owned typed session.
        let mut background_approver = |_prompt: viden_types::PermissionPrompt| {
            ApprovalResponse::allow_once(Some("test-only async job approval".to_string()))
        };
        let result = run_acp_session_prompt_for_agent_with_log(
            &monitor_cwd,
            &monitor_agent,
            &monitor_task,
            monitor_session.clone(),
            AcpSessionPromptRunContext {
                approver: &mut background_approver,
                log_path: log_path.clone(),
                cancel_path: Some(cancel_path.clone()),
                runtime_event_log_path: Some(monitor_runtime_event_path.clone()),
                permission_context: PermissionContext::default(),
                runtime_event_sink,
                on_pid: |pid| {
                    if let Ok(mut slot) = pid_slot_for_thread.lock() {
                        *slot = Some(pid);
                    }
                    let mut pid_record = monitor_record.clone();
                    pid_record.pid = Some(pid);
                    pid_record.updated_at = timestamp_millis();
                    let _ = append_codex_job_record(&monitor_cwd, "pid", &pid_record);
                },
            },
        );
        let was_cancelled = monitor_cancel_path.exists()
            || find_codex_job(&monitor_cwd, &monitor_record.id)
                .ok()
                .flatten()
                .is_some_and(|job| job.status == "cancelled");
        if was_cancelled && result.is_err() {
            let _ = append_agent_job_log_event(
                &log_path,
                "system",
                &format!(
                    "observed cancellation for ACP session job `{}` after agent process stopped",
                    monitor_record.id
                ),
            );
            return;
        }
        monitor_record.pid = pid_slot.lock().ok().and_then(|slot| *slot);
        match result {
            Ok(evidence) => {
                let _ =
                    write_acp_runtime_events(&monitor_runtime_event_path, &evidence.runtime_events);
                let _ = write_acp_session_result(&result_path, &evidence);
                monitor_record.status = acp_session_job_status(&evidence);
                if was_cancelled && monitor_record.status != "cancelled" {
                    monitor_record.status = "cancelled".to_string();
                }
            }
            Err(error) => {
                let _ = fs::write(&result_path, format!("# ACP session failed\n\n{error}\n"));
                monitor_record.status = "failed".to_string();
            }
        }
        monitor_record.updated_at = timestamp_millis();
        let _ = append_codex_job_record(&monitor_cwd, "completed", &monitor_record);
    });

    Ok(format!(
        "Started ACP session job `{}`.\n  agent: {} ({})\n  log: {}\n  result: {}\n\nUse `/agent status` to watch it, `/agent result {}` to read output, and `/agent cancel {}` to stop it.",
        id,
        agent.agent_id,
        agent.display_name,
        record.log_path.display(),
        record.result_path.display(),
        id,
        id
    ))
}

fn render_codex_job_status(cwd: &Path) -> Result<String, String> {
    let jobs = latest_codex_jobs(cwd)?;
    if jobs.is_empty() {
        return Ok("Agent jobs:\n  no tracked jobs".to_string());
    }
    let mut lines = vec![
        "Agent jobs:".to_string(),
        "  id                    kind          status       pid     updated     task".to_string(),
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
            "  {:<21} {:<13} {:<12} {:<7} {:<11} {}",
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
            lines.push(agent_job_session_line(&job, &session_id));
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
    let job = find_codex_job(cwd, id)?.ok_or_else(|| format!("Unknown agent job `{id}`"))?;
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
        .map(|session_id| format!("{}\n", agent_job_session_line(&job, session_id)))
        .unwrap_or_default();
    let files = if evidence.files.is_empty() {
        String::new()
    } else {
        format!("  files: {}\n", evidence.files.join(", "))
    };
    Ok(format!(
        "{} `{}`\n  kind: {}\n  status: {}\n  pid: {}\n  command: {}\n  log: {}\n  result: {}\n{}{}\n{}",
        agent_job_label(&job),
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

// Keep the liveness check and termination result as separate branches: the
// cancellation monitor races this path and depends on the original ordering.
#[allow(clippy::collapsible_if)]
fn cancel_codex_job(cwd: &Path, id: Option<&str>) -> Result<String, String> {
    let id = id.ok_or_else(|| "Usage: /agent cancel <id>".to_string())?;
    let mut job = find_codex_job(cwd, id)?.ok_or_else(|| format!("Unknown agent job `{id}`"))?;
    let label = agent_job_label(&job);
    if matches!(job.status.as_str(), "cancelled" | "finished") {
        return Ok(format!("{label} `{id}` is already {}.", job.status));
    }
    // Every process-backed job receives a durable intent marker so its monitor
    // cannot race a confirmed cancellation with a later nonterminal record.
    let cancel_path = Some(acp_job_cancel_path(cwd, &job.id));
    if let Some(path) = &cancel_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(
            path,
            format!("cancel requested at {}\n", timestamp_millis()),
        )
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }
    // Preserve a nonterminal durable state until process termination is
    // confirmed. Restart recovery must never hide a still-live child behind a
    // terminal `cancelled` record.
    job.updated_at = timestamp_millis();
    append_codex_job_record(cwd, "cancel_requested", &job)?;
    let Some(pid) = job.pid else {
        append_agent_job_log_event(
            &job.log_path,
            "system",
            &format!("cancel requested for {label} `{id}` before process startup"),
        )?;
        return Err(format!(
            "Cancellation requested for {label} `{id}`, but process termination is not confirmed yet; retry after startup completes."
        ));
    };
    if job.kind == "acp-session" {
        let _ =
            wait_for_agent_job_text(&job.log_path, "session/cancel", Duration::from_millis(1500));
    }
    if process_is_running(pid) {
        if let Err(error) = terminate_process(pid) {
            job.updated_at = timestamp_millis();
            append_codex_job_record(cwd, "cancel_failed", &job)?;
            return Err(error);
        }
    }
    if process_is_running(pid) {
        job.updated_at = timestamp_millis();
        append_codex_job_record(cwd, "cancel_failed", &job)?;
        return Err(format!(
            "process {pid} is still running after cancellation; retry cancellation"
        ));
    }
    job.status = "cancelled".to_string();
    job.updated_at = timestamp_millis();
    append_codex_job_record(cwd, "cancelled", &job)?;
    write_agent_job_cancel_result(&job, pid)?;
    Ok(format!("Cancelled {label} `{id}` (pid {pid})."))
}

pub(crate) fn cancel_typed_agent_session(cwd: &Path, session_id: &str) -> Result<(), String> {
    cancel_codex_job(cwd, Some(session_id)).map(|_| ())
}

pub(crate) fn mark_typed_agent_session_status(
    cwd: &Path,
    session_id: &str,
    status: &str,
) -> Result<(), String> {
    let mut job = find_codex_job(cwd, session_id)?
        .ok_or_else(|| format!("Unknown agent session `{session_id}`"))?;
    if job.kind != "acp-session" {
        return Err(format!("Job `{session_id}` is not an ACP session"));
    }
    job.status = status.to_string();
    job.updated_at = timestamp_millis();
    append_codex_job_record(cwd, "status", &job)
}

fn write_agent_job_cancel_result(job: &CodexJobRecord, pid: u32) -> Result<(), String> {
    let note = if job.kind == "acp-session" {
        "Viden requested ACP session/cancel when the live session was available, then used process termination as a bounded fallback if the agent did not stop promptly."
    } else {
        "Agent job was stopped by terminating the process."
    };
    if let Some(parent) = job.result_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if job.kind == "acp-session" {
        // The ACP monitor owns the richer protocol result. Give it a bounded
        // opportunity to publish that evidence and never overwrite it with the
        // process-level fallback summary.
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if fs::metadata(&job.result_path).is_ok_and(|metadata| metadata.len() > 0) {
                return append_agent_job_log_event(
                    &job.log_path,
                    "system",
                    &format!(
                        "cancelled {} `{}` via pid {}",
                        agent_job_label(job),
                        job.id,
                        pid
                    ),
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fs::write(
        &job.result_path,
        format!(
            "# {} cancelled\n\nstatus: cancelled\npid: {}\ncommand: {}\ntask: {}\nlog: {}\n\n{}\n",
            agent_job_label(job),
            pid,
            job.command,
            job.task,
            job.log_path.display(),
            note
        ),
    )
    .map_err(|err| format!("failed to write {}: {err}", job.result_path.display()))?;
    append_agent_job_log_event(
        &job.log_path,
        "system",
        &format!(
            "cancelled {} `{}` via pid {}",
            agent_job_label(job),
            job.id,
            pid
        ),
    )
}

fn agent_job_label(job: &CodexJobRecord) -> &'static str {
    if job.kind == "acp-session" {
        "ACP session job"
    } else {
        "Codex job"
    }
}

fn agent_job_session_line(job: &CodexJobRecord, session_id: &str) -> String {
    if job.kind == "acp-session" {
        format!("    session: {session_id}")
    } else {
        format!("    resume: codex resume {session_id}")
    }
}

fn codex_command() -> String {
    env::var("VIDEN_AGENT_CODEX_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "codex".to_string())
}

fn codex_job_artifact_path(cwd: &Path, id: &str, ext: &str) -> PathBuf {
    cwd.join(".viden")
        .join("agents")
        .join(format!("{id}.{ext}"))
}

fn acp_job_cancel_path(cwd: &Path, id: &str) -> PathBuf {
    codex_job_artifact_path(cwd, id, "cancel")
}

fn acp_job_runtime_events_path(cwd: &Path, id: &str) -> PathBuf {
    codex_job_artifact_path(cwd, id, "runtime-events.jsonl")
}

pub(crate) fn tracked_agent_job_runtime_events(cwd: &Path) -> Vec<RuntimeEvent> {
    latest_codex_jobs(cwd)
        .unwrap_or_default()
        .into_iter()
        .filter(|job| job.kind == "acp-session")
        .flat_map(|job| read_acp_runtime_events(&acp_job_runtime_events_path(cwd, &job.id)))
        .collect()
}

fn read_acp_runtime_events(path: &Path) -> Vec<RuntimeEvent> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<RuntimeEvent>(line).ok())
        .collect()
}

fn write_acp_runtime_events(path: &Path, events: &[RuntimeEvent]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut content = String::new();
    for event in events {
        let line = serde_json::to_string(event)
            .map_err(|err| format!("failed to encode ACP runtime event: {err}"))?;
        content.push_str(&line);
        content.push('\n');
    }
    fs::write(path, content).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn append_acp_runtime_events(path: &Path, events: &[RuntimeEvent]) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    for event in events {
        let line = serde_json::to_string(event)
            .map_err(|err| format!("failed to encode ACP runtime event: {err}"))?;
        writeln!(file, "{line}")
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }
    Ok(())
}

fn wait_for_agent_job_text(path: &Path, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if fs::read_to_string(path).is_ok_and(|text| text.contains(needle)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
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
        r#"{{"ts":{},"event":"{}","id":"{}","kind":"{}","status":"{}","pid":{},"command":"{}","task":"{}","log":"{}","result":"{}","baseline":"{}","agent_id":"{}","model":"{}","owner_workspace_id":"{}","owner_project_id":"{}","owner_lane_id":"{}","owner_session_id":"{}","owner_task_id":"{}","owner_turn_id":"{}"}}"#,
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
        escape_json_fragment(&record.baseline_path.display().to_string()),
        escape_json_fragment(record.agent.as_ref().map(|agent| agent.agent_id.as_str()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().and_then(|agent| agent.model.as_deref()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().map(|agent| agent.owner.workspace_id.as_str()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().map(|agent| agent.owner.project_id.as_str()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().and_then(|agent| agent.owner.lane_id.as_deref()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().and_then(|agent| agent.owner.session_id.as_deref()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().and_then(|agent| agent.owner.task_id.as_deref()).unwrap_or("")),
        escape_json_fragment(record.agent.as_ref().and_then(|agent| agent.owner.turn_id.as_deref()).unwrap_or(""))
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
    let optional = |field: &str| json_string_field(line, field).filter(|value| !value.is_empty());
    let agent = optional("agent_id").map(|agent_id| AgentJobMetadata {
        agent_id,
        model: optional("model"),
        owner: RuntimeOwner {
            workspace_id: optional("owner_workspace_id").unwrap_or_default(),
            project_id: optional("owner_project_id").unwrap_or_default(),
            lane_id: optional("owner_lane_id"),
            session_id: optional("owner_session_id"),
            task_id: optional("owner_task_id"),
            turn_id: optional("owner_turn_id"),
        },
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
        agent,
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
        agent: None,
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
    let signals = codex_app_server_signal_summary(&evidence.notifications);
    fs::write(
        result_path,
        format!(
            "# Codex app-server turn\n\nthread: {}\nturn: {}\nstatus: {}\nlog: {}\nresume: {}\nmessage: {}\napprovals: {}\nsignals: {}\n",
            evidence.thread_id.as_deref().unwrap_or("unknown"),
            evidence.turn_id.as_deref().unwrap_or("unknown"),
            evidence.turn_status.as_deref().unwrap_or("unknown"),
            evidence.log_path.display(),
            evidence.thread_id.as_deref().unwrap_or("unknown"),
            evidence.final_message.as_deref().unwrap_or("none"),
            if evidence.approval_requests.is_empty() {
                "none".to_string()
            } else {
                evidence.approval_requests.join(", ")
            },
            signals
        ),
    )
    .map_err(|err| format!("failed to write {}: {err}", result_path.display()))
}

fn codex_app_server_signal_summary(notifications: &[String]) -> String {
    let mut signals = Vec::new();
    for (method, label) in [
        ("item/commandExecution/outputDelta", "command-output"),
        ("item/fileChange/outputDelta", "file-change"),
        ("item/fileChange/patchUpdated", "file-patch"),
        ("turn/diff/updated", "diff-updated"),
        ("fs/changed", "fs-changed"),
        ("item/mcpToolCall", "mcp-tool-call"),
        ("item/mcpToolCall/completed", "mcp-tool-completed"),
        ("item/mcpToolCall/fs-write", "mcp-fs-write"),
        ("error", "app-server-error"),
    ] {
        if notifications.iter().any(|item| item == method) {
            signals.push(label);
        }
    }
    if signals.is_empty() {
        "none".to_string()
    } else {
        signals.join(", ")
    }
}

fn codex_app_server_turn_job_status(evidence: &CodexAppServerProbeEvidence) -> String {
    match evidence.turn_status.as_deref() {
        Some("completed") => "finished".to_string(),
        Some("failed" | "interrupted") => "failed".to_string(),
        _ => "observed".to_string(),
    }
}

fn acp_session_job_status(evidence: &AcpSessionPromptEvidence) -> String {
    match evidence.final_status.as_str() {
        "completed" | "end_turn" => "finished".to_string(),
        "cancelled" => "cancelled".to_string(),
        "failed" | "interrupted" => "failed".to_string(),
        _ => "observed".to_string(),
    }
}

fn write_acp_session_result(
    path: &Path,
    evidence: &AcpSessionPromptEvidence,
) -> Result<(), String> {
    let tool_calls = if evidence.tool_calls.is_empty() {
        "none".to_string()
    } else {
        evidence.tool_calls.join(", ")
    };
    fs::write(
        path,
        format!(
            "# ACP session result\n\nsession: {}\nstatus: {}\ntool_calls: {}\nusage: {}\nlog: {}\n\n{}",
            evidence.session_id,
            evidence.final_status,
            tool_calls,
            evidence.usage_summary.as_deref().unwrap_or("unavailable"),
            evidence.log_path.display(),
            evidence.message.trim()
        ),
    )
    .map_err(|err| format!("failed to write ACP session result: {err}"))
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
    if path.starts_with(".viden/") {
        return None;
    }
    Some(line.to_string())
}

fn git_status_path(line: &str) -> Option<String> {
    let path = line.get(3..)?.trim();
    let path = path.rsplit(" -> ").next().unwrap_or(path).trim();
    let path = path.trim_matches('"');
    if path.is_empty() || path.starts_with(".viden/") {
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
            && let Some(id) = first_identifier_token(rest)
        {
            return Some(id);
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
        || token.starts_with(".viden/")
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
        let signallable = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !signallable {
            return false;
        }
        // A killed child remains signallable as a zombie until its monitor
        // reaps it, but it can no longer execute ACP side effects.
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .map(|output| {
                let state = String::from_utf8_lossy(&output.stdout);
                let state = state.trim();
                !state.is_empty() && !state.starts_with('Z')
            })
            .unwrap_or(signallable)
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
        send_unix_signal(pid, "-TERM")?;
        if wait_for_process_stop(pid, Duration::from_millis(300)) {
            return Ok(());
        }
        send_unix_signal(pid, "-KILL")?;
        if wait_for_process_stop(pid, Duration::from_millis(700)) {
            Ok(())
        } else {
            Err(format!("process {pid} remained alive after TERM and KILL"))
        }
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

#[cfg(unix)]
fn send_unix_signal(pid: u32, signal: &str) -> Result<(), String> {
    Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("failed to run kill {signal}: {err}"))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("kill {signal} exited with {status}"))
            }
        })
}

#[cfg(unix)]
fn wait_for_process_stop(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    !process_is_running(pid)
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
    cwd.join(".viden")
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
    cwd.join(".viden").join("agents").join("codex-jobs.jsonl")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcpProbeEvidence {
    protocol_version: String,
    agent_label: String,
    auth_methods: Vec<String>,
    auth_method_ids: Vec<String>,
    capabilities: Vec<String>,
    log_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcpAuthEvidence {
    method_id: String,
    status: String,
    log_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcpSessionPromptEvidence {
    session_id: String,
    final_status: String,
    message: String,
    tool_calls: Vec<String>,
    usage_summary: Option<String>,
    runtime_events: Vec<RuntimeEvent>,
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
    final_message: Option<String>,
    notifications: Vec<String>,
    approval_requests: Vec<String>,
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
    let mut approval_requests = Vec::new();
    let response = match read_codex_app_server_response(
        &receiver,
        &mut stdin,
        1,
        &mut log_entries,
        &mut notifications,
        &mut approval_requests,
        Duration::from_secs(5),
    ) {
        Ok(response) => response,
        Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
    };

    let start_thread = !matches!(mode, CodexProbeMode::Initialize);
    let write_turn = matches!(mode, CodexProbeMode::Turn { write: true, .. });
    let thread_id = if start_thread {
        let request = codex_app_server_thread_start_request(cwd, write_turn);
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
            &mut stdin,
            2,
            &mut log_entries,
            &mut notifications,
            &mut approval_requests,
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
    if let CodexProbeMode::Turn { task, write } = &mode {
        let Some(thread_id) = thread_id.as_deref() else {
            return Err(finish_failed_probe(
                child,
                log_path.clone(),
                log_entries.clone(),
                "Codex app-server thread/start did not return a thread id".to_string(),
            ));
        };
        let request = codex_app_server_turn_start_request(cwd, thread_id, task, *write);
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
            &mut stdin,
            3,
            &mut log_entries,
            &mut notifications,
            &mut approval_requests,
            Duration::from_secs(8),
        ) {
            Ok(response) => response,
            Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
        };
        turn_id = json_object_string_field(&turn_response, "turn", "id");
        turn_status = json_string_field(&turn_response, "status");
        if let Some(completed) = collect_codex_app_server_notifications(
            &receiver,
            &mut stdin,
            &mut log_entries,
            &mut notifications,
            &mut approval_requests,
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
            if is_codex_app_server_request(&method) {
                if let Some(response) =
                    codex_app_server_request_denial_response(line, &method, &mut approval_requests)
                {
                    log_entries.push(jsonl_event("client", &response));
                    let _ = stdin
                        .write_all(response.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush());
                }
            } else {
                record_codex_app_server_notification(line, &method, &mut notifications);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let final_message = codex_app_server_final_message(&log_entries);
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
        final_message,
        notifications,
        approval_requests,
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
    Ok(acp_probe_evidence_from_response(&response, log_path))
}

fn run_acp_initialize_probe_for_agent(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
) -> Result<AcpProbeEvidence, String> {
    let log_path = acp_probe_log_path(cwd);
    let mut log_entries = Vec::new();
    let request = acp_initialize_request();
    log_entries.push(jsonl_event("client", &request));

    let mut child = spawn_acp_agent_process(cwd, agent)?;
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

    let response = match read_line_with_timeout(stdout, acp_agent_handshake_timeout(agent)) {
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
    Ok(acp_probe_evidence_from_response(&response, log_path))
}

fn run_acp_authenticate_for_agent(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
    method_id: Option<&str>,
) -> Result<AcpAuthEvidence, String> {
    let log_path = acp_probe_log_path(cwd);
    let mut log_entries = Vec::new();
    let mut child = spawn_acp_agent_process(cwd, agent)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open ACP stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to open ACP stdout".to_string())?;
    let receiver = read_lines_async(stdout);

    let initialize = acp_initialize_request();
    if let Err(error) = write_acp_request(&mut stdin, &initialize, &mut log_entries) {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }
    let initialize_response = match read_acp_response_line(
        &receiver,
        0,
        &mut log_entries,
        acp_agent_handshake_timeout(agent),
    ) {
        Ok(response) => response,
        Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
    };
    let probe = acp_probe_evidence_from_response(&initialize_response, log_path.clone());
    let method = match method_id {
        Some(method) => method.to_string(),
        None if probe.auth_method_ids.len() == 1 => probe.auth_method_ids[0].clone(),
        None if probe.auth_method_ids.is_empty() => {
            return Err(finish_expected_acp_stop(
                child,
                log_path,
                log_entries,
                "ACP agent did not advertise authentication methods".to_string(),
            ));
        }
        None => {
            let methods = probe.auth_methods.join(", ");
            return Err(finish_expected_acp_stop(
                child,
                log_path,
                log_entries,
                format!("choose an auth method: {methods}"),
            ));
        }
    };
    if !probe.auth_method_ids.is_empty() && !probe.auth_method_ids.iter().any(|id| id == &method) {
        let methods = probe.auth_methods.join(", ");
        return Err(finish_expected_acp_stop(
            child,
            log_path,
            log_entries,
            format!("unknown ACP auth method `{method}`. Available methods: {methods}"),
        ));
    }

    let authenticate = acp_authenticate_request(&method);
    if let Err(error) = write_acp_request(&mut stdin, &authenticate, &mut log_entries) {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }
    let response = match read_acp_response_line(
        &receiver,
        1,
        &mut log_entries,
        acp_agent_handshake_timeout(agent),
    ) {
        Ok(response) => response,
        Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
    };
    let _ = child.kill();
    let _ = child.wait();
    write_probe_log(&log_path, &log_entries)?;
    if response.contains(r#""error""#) {
        return Err(format!(
            "ACP authenticate failed for method `{method}`; log {}",
            log_path.display()
        ));
    }
    Ok(AcpAuthEvidence {
        method_id: method,
        status: acp_auth_status_from_response(&response),
        log_path,
    })
}

fn run_acp_session_prompt_for_agent(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
    prompt: &str,
    session: AcpSessionOptions,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
) -> Result<AcpSessionPromptEvidence, String> {
    run_acp_session_prompt_for_agent_with_permissions(
        cwd,
        agent,
        prompt,
        session,
        approver,
        PermissionContext::default(),
        None,
    )
}

fn run_acp_session_prompt_for_agent_with_permissions(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
    prompt: &str,
    session: AcpSessionOptions,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    permission_context: PermissionContext,
    runtime_event_sink: Option<RuntimeEventSink>,
) -> Result<AcpSessionPromptEvidence, String> {
    let log_path = acp_session_log_path(cwd);
    run_acp_session_prompt_for_agent_with_log(
        cwd,
        agent,
        prompt,
        session,
        AcpSessionPromptRunContext {
            approver,
            log_path,
            cancel_path: None,
            runtime_event_log_path: None,
            permission_context,
            runtime_event_sink,
            on_pid: |_| {},
        },
    )
}

fn run_acp_session_prompt_for_agent_with_log<A, P>(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
    prompt: &str,
    session: AcpSessionOptions,
    context: AcpSessionPromptRunContext<'_, A, P>,
) -> Result<AcpSessionPromptEvidence, String>
where
    A: FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    P: FnMut(u32),
{
    let AcpSessionPromptRunContext {
        approver,
        log_path,
        cancel_path,
        runtime_event_log_path,
        permission_context,
        runtime_event_sink,
        mut on_pid,
    } = context;
    let mut log_entries = Vec::new();
    let mut permission_engine = PermissionEngine::new(cwd);
    permission_engine.restore_context(permission_context);
    let mut child = spawn_acp_agent_process(cwd, agent)?;
    on_pid(child.id());
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open ACP stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to open ACP stdout".to_string())?;
    let receiver = read_lines_async(stdout);

    let initialize = acp_initialize_request();
    if let Err(error) = write_acp_request(&mut stdin, &initialize, &mut log_entries) {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }
    if let Err(error) = read_acp_response_line(
        &receiver,
        0,
        &mut log_entries,
        acp_agent_handshake_timeout(agent),
    ) {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }

    let session_request = if let Some(session_id) = session.load_session_id.as_deref() {
        acp_session_load_request(cwd, session_id)
    } else {
        acp_session_new_request(cwd)
    };
    if let Err(error) = write_acp_request(&mut stdin, &session_request, &mut log_entries) {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }
    let session_response = match read_acp_response_line(
        &receiver,
        1,
        &mut log_entries,
        acp_agent_handshake_timeout(agent),
    ) {
        Ok(response) => response,
        Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
    };
    let Some(session_id) =
        acp_session_id_from_response(&session_response).or_else(|| session.load_session_id.clone())
    else {
        return Err(finish_failed_probe(
            child,
            log_path.clone(),
            log_entries.clone(),
            "ACP session creation did not return a session id".to_string(),
        ));
    };

    let mut next_request_id = 2;
    if let Some(mode_id) = session.mode_id.as_deref() {
        let set_mode = acp_session_set_mode_request(&session_id, mode_id, next_request_id);
        if let Err(error) = write_acp_request(&mut stdin, &set_mode, &mut log_entries) {
            return Err(finish_failed_probe(child, log_path, log_entries, error));
        }
        let response = match read_acp_response_line(
            &receiver,
            next_request_id,
            &mut log_entries,
            acp_agent_handshake_timeout(agent),
        ) {
            Ok(response) => response,
            Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
        };
        if acp_response_has_error(&response) {
            return Err(finish_failed_probe(
                child,
                log_path,
                log_entries,
                acp_response_error_message_for(
                    "ACP session/set_mode failed",
                    &serde_json::from_str(&response).unwrap_or(Value::Null),
                ),
            ));
        }
        next_request_id += 1;
    }
    if let Some(model_id) = session.model_id.as_deref() {
        let set_model = acp_session_set_model_request(&session_id, model_id, next_request_id);
        if let Err(error) = write_acp_request(&mut stdin, &set_model, &mut log_entries) {
            return Err(finish_failed_probe(child, log_path, log_entries, error));
        }
        let response = match read_acp_response_line(
            &receiver,
            next_request_id,
            &mut log_entries,
            acp_agent_handshake_timeout(agent),
        ) {
            Ok(response) => response,
            Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
        };
        next_request_id += 1;
        if acp_response_is_method_not_found(&response) {
            let legacy_set_model =
                acp_legacy_session_set_model_request(&session_id, model_id, next_request_id);
            if let Err(error) = write_acp_request(&mut stdin, &legacy_set_model, &mut log_entries) {
                return Err(finish_failed_probe(child, log_path, log_entries, error));
            }
            let legacy_response = match read_acp_response_line(
                &receiver,
                next_request_id,
                &mut log_entries,
                acp_agent_handshake_timeout(agent),
            ) {
                Ok(response) => response,
                Err(error) => return Err(finish_failed_probe(child, log_path, log_entries, error)),
            };
            if acp_response_has_error(&legacy_response) {
                return Err(finish_failed_probe(
                    child,
                    log_path,
                    log_entries,
                    acp_response_error_message_for(
                        "ACP session/set_model failed",
                        &serde_json::from_str(&legacy_response).unwrap_or(Value::Null),
                    ),
                ));
            }
            next_request_id += 1;
        } else if acp_response_has_error(&response) {
            return Err(finish_failed_probe(
                child,
                log_path,
                log_entries,
                acp_response_error_message_for(
                    "ACP session/set_config_option failed",
                    &serde_json::from_str(&response).unwrap_or(Value::Null),
                ),
            ));
        }
    }

    let prompt_request_id = next_request_id;
    let prompt_request = acp_session_prompt_request(agent, &session_id, prompt, prompt_request_id);
    if let Err(error) = write_acp_request(&mut stdin, &prompt_request, &mut log_entries) {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }
    let mut message = String::new();
    let mut tool_calls = Vec::new();
    let mut runtime_events = Vec::new();
    let mut runtime_sequence = 1;
    let mut acp_gate_evidence_ids = Vec::new();
    push_acp_runtime_event(
        &mut runtime_events,
        &mut runtime_sequence,
        RuntimeEventKind::MergeGateUpdated {
            gate: acp_session_merge_gate(
                &session_id,
                MergeGateStatus::Proposed,
                &acp_gate_evidence_ids,
            ),
        },
    );
    if let Some(path) = runtime_event_log_path.as_deref()
        && let Err(error) = append_acp_runtime_events(path, &runtime_events)
    {
        return Err(finish_failed_probe(child, log_path, log_entries, error));
    }
    if let Some(sink) = runtime_event_sink.as_ref() {
        sink(runtime_events.clone());
    }
    let mut terminals = AcpTerminalStore::default();
    let mut final_status = None;
    let mut usage_summary = None;
    let mut cancel_sent = false;
    let mut cancel_deadline = None;
    let session_timeout = acp_session_prompt_timeout(agent);
    let deadline = Instant::now() + session_timeout;
    while Instant::now() < deadline {
        if !cancel_sent && cancel_path.as_ref().is_some_and(|path| path.exists()) {
            let cancel_request = acp_session_cancel_request(&session_id);
            if let Err(error) = write_acp_request(&mut stdin, &cancel_request, &mut log_entries) {
                return Err(finish_failed_probe(child, log_path, log_entries, error));
            }
            cancel_sent = true;
            cancel_deadline = Some(Instant::now() + Duration::from_secs(2));
            continue;
        }
        if cancel_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            final_status = Some("cancelled".to_string());
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_secs(1))) {
            Ok(Ok(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                log_entries.push(jsonl_event("agent", line));
                let Ok(value) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                if value.get("id").and_then(Value::as_u64) == Some(prompt_request_id) {
                    if value.get("error").is_some() {
                        return Err(finish_failed_probe(
                            child,
                            log_path,
                            log_entries,
                            acp_response_error_message(&value),
                        ));
                    }
                    final_status = Some(acp_prompt_response_status(&value));
                    usage_summary = acp_usage_summary(&value);
                    break;
                }
                if value.get("id").and_then(Value::as_u64) == Some(ACP_SESSION_CANCEL_REQUEST_ID) {
                    final_status = Some(
                        value
                            .pointer("/result/status")
                            .and_then(Value::as_str)
                            .unwrap_or("cancelled")
                            .to_string(),
                    );
                    break;
                }
                if value.get("method").and_then(Value::as_str) == Some("session/request_permission")
                {
                    let prompt = acp_permission_prompt(&value);
                    let approval = approver(prompt);
                    let response = acp_permission_response(&value, approval.is_allowed());
                    if let Err(error) = write_acp_request(&mut stdin, &response, &mut log_entries) {
                        return Err(finish_failed_probe(child, log_path, log_entries, error));
                    }
                    continue;
                }
                if let Some(response) = acp_filesystem_client_request_response(
                    cwd,
                    &mut permission_engine,
                    approver,
                    &value,
                ) {
                    if let Err(error) = write_acp_request(&mut stdin, &response, &mut log_entries) {
                        return Err(finish_failed_probe(child, log_path, log_entries, error));
                    }
                    continue;
                }
                if let Some(response) = acp_terminal_client_request_response(
                    cwd,
                    &mut permission_engine,
                    approver,
                    &mut terminals,
                    &value,
                ) {
                    if let Err(error) = write_acp_request(&mut stdin, &response, &mut log_entries) {
                        return Err(finish_failed_probe(child, log_path, log_entries, error));
                    }
                    continue;
                }
                if let Some(response) = acp_unsupported_client_request_response(&value) {
                    if let Err(error) = write_acp_request(&mut stdin, &response, &mut log_entries) {
                        return Err(finish_failed_probe(child, log_path, log_entries, error));
                    }
                    continue;
                }
                let method = value.get("method").and_then(Value::as_str);
                if method != Some("session/update") && method != Some("session/notification") {
                    continue;
                }
                let Some(update) = value.pointer("/params/update") else {
                    continue;
                };
                let before_event_count = runtime_events.len();
                append_acp_update_runtime_events(
                    &mut runtime_events,
                    &mut runtime_sequence,
                    &session_id,
                    &mut acp_gate_evidence_ids,
                    update,
                );
                if let Some(path) = runtime_event_log_path.as_deref()
                    && before_event_count < runtime_events.len()
                    && let Err(error) =
                        append_acp_runtime_events(path, &runtime_events[before_event_count..])
                {
                    return Err(finish_failed_probe(child, log_path, log_entries, error));
                }
                if before_event_count < runtime_events.len()
                    && let Some(sink) = runtime_event_sink.as_ref()
                {
                    sink(runtime_events[before_event_count..].to_vec());
                }
                match acp_update_kind(update).as_deref() {
                    Some("AgentMessageChunk") | Some("agent_message_chunk") => {
                        if let Some(text) = acp_message_chunk_text(update) {
                            message.push_str(&text);
                        }
                    }
                    Some("ToolCall")
                    | Some("tool_call")
                    | Some("ToolCallUpdate")
                    | Some("tool_call_update") => {
                        tool_calls.push(acp_tool_call_summary(update));
                    }
                    Some("TurnEnd") | Some("turn_end") => {
                        final_status = update
                            .get("status")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| Some("completed".to_string()));
                        break;
                    }
                    _ => {}
                }
            }
            Ok(Err(error)) => {
                return Err(finish_failed_probe(
                    child,
                    log_path,
                    log_entries,
                    format!("failed to read ACP session update: {error}"),
                ));
            }
            Err(_) => {}
        }
    }
    let Some(final_status) = final_status else {
        return Err(finish_failed_probe(
            child,
            log_path,
            log_entries,
            format!(
                "ACP session/prompt timed out before TurnEnd or final response after {}s",
                session_timeout.as_secs()
            ),
        ));
    };
    let _ = child.kill();
    let _ = child.wait();
    write_probe_log(&log_path, &log_entries)?;

    Ok(AcpSessionPromptEvidence {
        session_id,
        final_status,
        message,
        tool_calls,
        usage_summary,
        runtime_events,
        log_path,
    })
}

fn write_acp_request(
    stdin: &mut impl Write,
    request: &str,
    log_entries: &mut Vec<String>,
) -> Result<(), String> {
    log_entries.push(jsonl_event("client", request));
    stdin
        .write_all(request.as_bytes())
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .map_err(|err| format!("failed to write ACP request: {err}"))
}

fn read_acp_response_line(
    receiver: &mpsc::Receiver<std::io::Result<String>>,
    id: u64,
    log_entries: &mut Vec<String>,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_secs(1))) {
            Ok(Ok(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                log_entries.push(jsonl_event("agent", line));
                let Ok(value) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                if value.get("id").and_then(Value::as_u64) == Some(id) {
                    return Ok(line.to_string());
                }
            }
            Ok(Err(error)) => return Err(format!("failed to read ACP response: {error}")),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(format!("ACP command closed stdout before response id {id}"));
            }
        }
    }
    Err(format!("ACP response id {id} timed out"))
}

fn acp_agent_handshake_timeout(agent: &AgentPluginDescriptor) -> Duration {
    if let Ok(raw) = env::var("VIDEN_ACP_HANDSHAKE_TIMEOUT_SECS")
        && let Ok(seconds) = raw.parse::<u64>()
        && seconds > 0
    {
        return Duration::from_secs(seconds);
    }
    if matches!(agent.source, AgentSource::Registry) {
        Duration::from_secs(DEFAULT_REGISTRY_ACP_HANDSHAKE_TIMEOUT_SECS)
    } else {
        Duration::from_secs(DEFAULT_LOCAL_ACP_HANDSHAKE_TIMEOUT_SECS)
    }
}

fn acp_session_prompt_timeout(agent: &AgentPluginDescriptor) -> Duration {
    if let Ok(raw) = env::var("VIDEN_ACP_SESSION_TIMEOUT_SECS")
        && let Ok(seconds) = raw.parse::<u64>()
        && seconds > 0
    {
        return Duration::from_secs(seconds);
    }
    if is_kiro_agent(agent) {
        Duration::from_secs(DEFAULT_KIRO_ACP_SESSION_TIMEOUT_SECS)
    } else {
        Duration::from_secs(DEFAULT_LOCAL_ACP_SESSION_TIMEOUT_SECS)
    }
}

fn acp_update_kind(update: &Value) -> Option<String> {
    update
        .get("type")
        .or_else(|| update.get("sessionUpdate"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn acp_message_chunk_text(update: &Value) -> Option<String> {
    match update.get("content") {
        Some(Value::String(content)) => Some(content.clone()),
        Some(Value::Object(content)) => content
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => update
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn acp_patch_text(update: &Value) -> Option<String> {
    [
        "/diff",
        "/patch",
        "/unifiedDiff",
        "/unified_diff",
        "/content/diff",
        "/content/patch",
        "/content/unifiedDiff",
        "/content/unified_diff",
        "/fileChange/patch",
        "/fileChange/diff",
        "/file_change/patch",
        "/file_change/diff",
    ]
    .iter()
    .filter_map(|pointer| update.pointer(pointer).and_then(Value::as_str))
    .find(|text| looks_like_unified_diff(text))
    .map(str::to_string)
}

fn acp_patch_path(update: &Value) -> Option<String> {
    [
        "/path",
        "/file",
        "/filePath",
        "/file_path",
        "/content/path",
        "/content/file",
        "/content/filePath",
        "/content/file_path",
        "/fileChange/path",
        "/fileChange/filePath",
        "/file_change/path",
        "/file_change/file_path",
    ]
    .iter()
    .find_map(|pointer| update.pointer(pointer).and_then(Value::as_str))
    .map(str::to_string)
}

fn acp_patch_summary(patch: &str, explicit_path: Option<&str>) -> String {
    let metadata = acp_patch_metadata(patch, explicit_path, &Value::Null);
    let file_count = metadata
        .get("fileCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let additions = metadata
        .get("additions")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let deletions = metadata
        .get("deletions")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let path = explicit_path
        .map(str::to_string)
        .or_else(|| {
            metadata
                .pointer("/files/0/path")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<unknown>".to_string());
    format!("ACP patch: {file_count} file(s), +{additions}/-{deletions}, first {path}")
}

fn acp_patch_metadata(patch: &str, explicit_path: Option<&str>, update: &Value) -> Value {
    let (files, additions, deletions, hunks) = acp_patch_files(patch, explicit_path);
    json!({
        "schema": "acp.patch.v1",
        "format": "unified_diff",
        "fileCount": files.len(),
        "additions": additions,
        "deletions": deletions,
        "hunks": hunks,
        "files": files,
        "diff": patch,
        "origin": {
            "updateType": acp_update_kind(update),
            "toolCallId": update
                .get("toolCallId")
                .or_else(|| update.get("tool_call_id"))
                .and_then(Value::as_str),
        }
    })
}

fn acp_patch_files(patch: &str, explicit_path: Option<&str>) -> (Vec<Value>, u64, u64, u64) {
    #[derive(Default)]
    struct FileStats {
        old_path: Option<String>,
        new_path: Option<String>,
        additions: u64,
        deletions: u64,
        hunks: u64,
    }

    fn push_file(files: &mut Vec<Value>, file: &mut Option<FileStats>) {
        let Some(stats) = file.take() else {
            return;
        };
        let path = stats
            .new_path
            .as_deref()
            .filter(|path| *path != "/dev/null")
            .or(stats
                .old_path
                .as_deref()
                .filter(|path| *path != "/dev/null"))
            .unwrap_or("<unknown>");
        files.push(json!({
            "path": path,
            "oldPath": stats.old_path,
            "newPath": stats.new_path,
            "additions": stats.additions,
            "deletions": stats.deletions,
            "hunks": stats.hunks,
        }));
    }

    let mut files = Vec::new();
    let mut current: Option<FileStats> = None;
    let mut total_additions = 0;
    let mut total_deletions = 0;
    let mut total_hunks = 0;

    for line in patch.lines() {
        if let Some((old_path, new_path)) = parse_diff_git_paths(line) {
            push_file(&mut files, &mut current);
            current = Some(FileStats {
                old_path: Some(old_path),
                new_path: Some(new_path),
                ..FileStats::default()
            });
            continue;
        }
        if let Some(old_path) = line.strip_prefix("--- ") {
            current.get_or_insert_with(FileStats::default).old_path =
                Some(normalize_diff_path(old_path));
            continue;
        }
        if let Some(new_path) = line.strip_prefix("+++ ") {
            current.get_or_insert_with(FileStats::default).new_path =
                Some(normalize_diff_path(new_path));
            continue;
        }
        if line.starts_with("@@") {
            let stats = current.get_or_insert_with(FileStats::default);
            stats.hunks += 1;
            total_hunks += 1;
            continue;
        }
        if line.starts_with('+') && !line.starts_with("+++") {
            let stats = current.get_or_insert_with(FileStats::default);
            stats.additions += 1;
            total_additions += 1;
            continue;
        }
        if line.starts_with('-') && !line.starts_with("---") {
            let stats = current.get_or_insert_with(FileStats::default);
            stats.deletions += 1;
            total_deletions += 1;
        }
    }
    push_file(&mut files, &mut current);

    if files.is_empty()
        && let Some(path) = explicit_path
    {
        files.push(json!({
            "path": path,
            "oldPath": path,
            "newPath": path,
            "additions": total_additions,
            "deletions": total_deletions,
            "hunks": total_hunks,
        }));
    }

    (files, total_additions, total_deletions, total_hunks)
}

fn parse_diff_git_paths(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("diff --git ")?;
    let mut parts = rest.split_whitespace();
    let old_path = normalize_diff_path(parts.next()?);
    let new_path = normalize_diff_path(parts.next()?);
    Some((old_path, new_path))
}

fn normalize_diff_path(path: &str) -> String {
    path.trim()
        .trim_matches('"')
        .strip_prefix("a/")
        .or_else(|| path.trim().trim_matches('"').strip_prefix("b/"))
        .unwrap_or_else(|| path.trim().trim_matches('"'))
        .to_string()
}

fn looks_like_unified_diff(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("diff --git ") || (trimmed.contains("\n--- ") && trimmed.contains("\n+++ "))
}

fn acp_prompt_response_status(response: &Value) -> String {
    response
        .pointer("/result/stopReason")
        .or_else(|| response.pointer("/result/status"))
        .and_then(Value::as_str)
        .unwrap_or("completed")
        .to_string()
}

fn acp_usage_summary(response: &Value) -> Option<String> {
    let usage = response.pointer("/result/usage")?;
    let total = usage.get("totalTokens").and_then(Value::as_u64);
    let input = usage.get("inputTokens").and_then(Value::as_u64);
    let output = usage.get("outputTokens").and_then(Value::as_u64);
    match (total, input, output) {
        (Some(total), Some(input), Some(output)) => {
            Some(format!("total={total} input={input} output={output}"))
        }
        (Some(total), _, _) => Some(format!("total={total}")),
        _ => None,
    }
}

fn append_acp_update_runtime_events(
    events: &mut Vec<RuntimeEvent>,
    sequence: &mut u64,
    session_id: &str,
    gate_evidence_ids: &mut Vec<String>,
    update: &Value,
) {
    match acp_update_kind(update).as_deref() {
        Some("AgentMessageChunk") | Some("agent_message_chunk") => {
            if let Some(text) = acp_message_chunk_text(update) {
                push_acp_runtime_event(
                    events,
                    sequence,
                    RuntimeEventKind::AssistantDelta {
                        message_id: format!("acp-message-{session_id}-{sequence}"),
                        task_id: Some(format!("acp-session-{session_id}")),
                        content: text,
                    },
                );
            }
        }
        Some("ToolCall") | Some("tool_call") => {
            let tool_call_id = acp_tool_call_id(update);
            let title = acp_tool_call_title(update);
            push_acp_runtime_event(
                events,
                sequence,
                RuntimeEventKind::ToolCallStarted {
                    tool_call_id,
                    name: title,
                    input_preview: truncate_for_preview(&update.to_string(), 500),
                },
            );
        }
        Some("ToolCallUpdate") | Some("tool_call_update") => {
            let tool_call_id = acp_tool_call_id(update);
            let title = acp_tool_call_title(update);
            let status = update
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed");
            let content = update
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or(status);
            if let Some(patch) = acp_patch_text(update) {
                let path = acp_patch_path(update);
                let patch_evidence = EvidenceView {
                    id: format!("acp-patch-{tool_call_id}-{sequence}"),
                    kind: "patch".to_string(),
                    summary: acp_patch_summary(&patch, path.as_deref()),
                    path: path.clone(),
                    source: Some("acp:patch.v1".to_string()),
                    canonical: None,
                    metadata: Some(acp_patch_metadata(&patch, path.as_deref(), update)),
                    timestamp: None,
                };
                push_unique_evidence_id(gate_evidence_ids, &patch_evidence.id);
                push_acp_runtime_event(
                    events,
                    sequence,
                    RuntimeEventKind::EvidenceRecorded {
                        evidence: patch_evidence,
                    },
                );
            }
            let evidence = EvidenceView {
                id: format!("acp-tool-{tool_call_id}-{sequence}"),
                kind: "tool_log".to_string(),
                summary: truncate_for_preview(content, 500),
                path: None,
                source: Some("acp".to_string()),
                canonical: None,
                metadata: None,
                timestamp: None,
            };
            push_acp_runtime_event(
                events,
                sequence,
                RuntimeEventKind::ToolCallFinished {
                    tool_call_id: tool_call_id.clone(),
                    name: title,
                    success: !matches!(status, "failed" | "error" | "cancelled" | "canceled"),
                    exit_code: None,
                    evidence: Some(evidence.clone()),
                },
            );
            push_unique_evidence_id(gate_evidence_ids, &evidence.id);
            push_acp_runtime_event(
                events,
                sequence,
                RuntimeEventKind::EvidenceRecorded { evidence },
            );
            push_acp_runtime_event(
                events,
                sequence,
                RuntimeEventKind::MergeGateUpdated {
                    gate: acp_session_merge_gate(
                        session_id,
                        MergeGateStatus::CollectingEvidence,
                        gate_evidence_ids,
                    ),
                },
            );
        }
        Some("Diff")
        | Some("diff")
        | Some("Patch")
        | Some("patch")
        | Some("FileChange")
        | Some("file_change")
        | Some("fileChange")
        | Some("file_change_patch")
        | Some("file-patch")
        | Some("diff-updated") => {
            if let Some(patch) = acp_patch_text(update) {
                let path = acp_patch_path(update);
                let evidence = EvidenceView {
                    id: format!("acp-patch-{session_id}-{sequence}"),
                    kind: "patch".to_string(),
                    summary: acp_patch_summary(&patch, path.as_deref()),
                    path: path.clone(),
                    source: Some("acp:patch.v1".to_string()),
                    canonical: None,
                    metadata: Some(acp_patch_metadata(&patch, path.as_deref(), update)),
                    timestamp: None,
                };
                push_unique_evidence_id(gate_evidence_ids, &evidence.id);
                push_acp_runtime_event(
                    events,
                    sequence,
                    RuntimeEventKind::EvidenceRecorded { evidence },
                );
                push_acp_runtime_event(
                    events,
                    sequence,
                    RuntimeEventKind::MergeGateUpdated {
                        gate: acp_session_merge_gate(
                            session_id,
                            MergeGateStatus::CollectingEvidence,
                            gate_evidence_ids,
                        ),
                    },
                );
            }
        }
        Some("TurnEnd") | Some("turn_end") => {
            let status = update
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed");
            let evidence = EvidenceView {
                id: format!("acp-turn-end-{session_id}-{sequence}"),
                kind: "acp_turn_end".to_string(),
                summary: format!("ACP session {session_id} ended with status {status}"),
                path: None,
                source: Some("acp".to_string()),
                canonical: None,
                metadata: None,
                timestamp: None,
            };
            push_unique_evidence_id(gate_evidence_ids, &evidence.id);
            push_acp_runtime_event(
                events,
                sequence,
                RuntimeEventKind::EvidenceRecorded { evidence },
            );
            push_acp_runtime_event(
                events,
                sequence,
                RuntimeEventKind::MergeGateUpdated {
                    gate: acp_session_merge_gate(
                        session_id,
                        MergeGateStatus::CollectingEvidence,
                        gate_evidence_ids,
                    ),
                },
            );
        }
        _ => {}
    }
}

fn acp_session_merge_gate(
    session_id: &str,
    status: MergeGateStatus,
    evidence_ids: &[String],
) -> MergeGateRecord {
    let now = now_timestamp();
    let task_id = format!("acp-session-{session_id}");
    let required_evidence = acp_session_required_evidence(evidence_ids);
    MergeGateRecord {
        gate_id: format!("gate-acp-session-{session_id}"),
        task_id: task_id.clone(),
        status,
        required_evidence: required_evidence.clone(),
        evidence_ids: evidence_ids.to_vec(),
        gate_type: MergeGateType::Artifact,
        owner: RuntimeOwner {
            task_id: Some(task_id),
            ..RuntimeOwner::default()
        },
        validator: None,
        policy_snapshot: MergeGatePolicySnapshot {
            required_evidence,
            permission_snapshot_id: None,
            requires_independent_validator: false,
            captured_at: Some(now),
        },
        decision: if status == MergeGateStatus::CollectingEvidence && !evidence_ids.is_empty() {
            Some(crate::trust_loop::merge_gate_decision(
                MergeGateDecisionOutcome::AwaitingEvidence,
                "missing_canonical".to_string(),
                RuntimeOwner::default(),
                evidence_ids.to_vec(),
                fresh_id("audit"),
            ))
        } else {
            None
        },
        conflict: None,
        applied_change_id: None,
        recovery_snapshot: None,
        audit_ids: Vec::new(),
        updated_at: Some(now),
    }
}

fn acp_session_required_evidence(evidence_ids: &[String]) -> Vec<String> {
    let mut required = Vec::new();
    if evidence_ids.iter().any(|id| id.starts_with("acp-patch-")) {
        required.push("patch".to_string());
    }
    required.push("acp_turn_end".to_string());
    required
}

fn push_unique_evidence_id(evidence_ids: &mut Vec<String>, evidence_id: &str) {
    if !evidence_ids.iter().any(|id| id == evidence_id) {
        evidence_ids.push(evidence_id.to_string());
    }
}

fn push_acp_runtime_event(
    events: &mut Vec<RuntimeEvent>,
    sequence: &mut u64,
    kind: RuntimeEventKind,
) {
    events.push(RuntimeEvent::new(*sequence, kind));
    *sequence += 1;
}

fn acp_response_error_message(response: &Value) -> String {
    acp_response_error_message_for("ACP session/prompt failed", response)
}

fn acp_response_error_message_for(context: &str, response: &Value) -> String {
    response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(|message| format!("{context}: {message}"))
        .unwrap_or_else(|| context.to_string())
}

fn acp_response_has_error(response: &str) -> bool {
    serde_json::from_str::<Value>(response)
        .ok()
        .is_some_and(|value| value.get("error").is_some())
}

fn acp_response_is_method_not_found(response: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(response) else {
        return false;
    };
    value.pointer("/error/code").and_then(Value::as_i64) == Some(-32601)
}

fn acp_session_new_request(cwd: &Path) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "session/new",
        "params": {
            "cwd": cwd.display().to_string(),
            "mcpServers": []
        }
    })
    .to_string()
}

fn acp_session_load_request(cwd: &Path, session_id: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "session/load",
        "params": {
            "cwd": cwd.display().to_string(),
            "mcpServers": [],
            "sessionId": session_id
        }
    })
    .to_string()
}

fn acp_authenticate_request(method_id: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "authenticate",
        "params": {
            "methodId": method_id
        }
    })
    .to_string()
}

fn acp_auth_status_from_response(response: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(response) else {
        return "unknown".to_string();
    };
    value
        .pointer("/result/status")
        .or_else(|| value.pointer("/result/outcome/status"))
        .or_else(|| value.pointer("/result/outcome"))
        .and_then(Value::as_str)
        .unwrap_or("ok")
        .to_string()
}

fn acp_session_prompt_request(
    agent: &AgentPluginDescriptor,
    session_id: &str,
    prompt: &str,
    id: u64,
) -> String {
    let text_blocks = json!([
        {
            "type": "text",
            "text": prompt
        }
    ]);
    let mut params = serde_json::Map::new();
    params.insert(
        "sessionId".to_string(),
        Value::String(session_id.to_string()),
    );
    if acp_agent_uses_content_prompt(agent) {
        params.insert("content".to_string(), text_blocks);
    } else {
        params.insert("prompt".to_string(), text_blocks);
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/prompt",
        "params": Value::Object(params)
    })
    .to_string()
}

fn acp_session_set_mode_request(session_id: &str, mode_id: &str, id: u64) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/set_mode",
        "params": {
            "sessionId": session_id,
            "modeId": mode_id
        }
    })
    .to_string()
}

fn acp_session_set_model_request(session_id: &str, model_id: &str, id: u64) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/set_config_option",
        "params": {
            "sessionId": session_id,
            "configId": "model",
            "value": model_id
        }
    })
    .to_string()
}

fn acp_legacy_session_set_model_request(session_id: &str, model_id: &str, id: u64) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/set_model",
        "params": {
            "sessionId": session_id,
            "modelId": model_id
        }
    })
    .to_string()
}

fn acp_session_cancel_request(session_id: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": ACP_SESSION_CANCEL_REQUEST_ID,
        "method": "session/cancel",
        "params": {
            "sessionId": session_id
        }
    })
    .to_string()
}

fn acp_agent_uses_content_prompt(_agent: &AgentPluginDescriptor) -> bool {
    false
}

fn acp_session_id_from_response(response: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(response).ok()?;
    value
        .pointer("/result/sessionId")
        .or_else(|| value.pointer("/result/session/id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn acp_permission_prompt(request: &Value) -> viden_types::PermissionPrompt {
    let tool_call = request.pointer("/params/toolCall");
    let tool_id = tool_call
        .and_then(|tool_call| {
            tool_call
                .get("toolCallId")
                .or_else(|| tool_call.get("id"))
                .and_then(Value::as_str)
        })
        .unwrap_or("permission");
    let title = tool_call
        .and_then(|tool_call| {
            tool_call
                .get("title")
                .or_else(|| tool_call.get("name"))
                .and_then(Value::as_str)
        })
        .unwrap_or("ACP permission request");
    let kind = tool_call
        .and_then(|tool_call| tool_call.get("kind").and_then(Value::as_str))
        .unwrap_or("external-agent");
    viden_types::PermissionPrompt {
        tool_name: format!("acp:{tool_id}"),
        message: format!("{title} ({kind})"),
        input_preview: request
            .pointer("/params")
            .map(Value::to_string)
            .unwrap_or_else(|| "{}".to_string()),
        candidate_paths: acp_request_path(request).into_iter().collect(),
    }
}

fn acp_permission_response(request: &Value, approved: bool) -> String {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    match acp_permission_option_id(request, approved) {
        Some(option_id) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "outcome": {
                    "type": "selected",
                    "optionId": option_id
                }
            }
        })
        .to_string(),
        None => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "outcome": {
                    "type": "cancelled"
                }
            }
        })
        .to_string(),
    }
}

fn acp_filesystem_client_request_response(
    cwd: &Path,
    permission_engine: &mut PermissionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    request: &Value,
) -> Option<String> {
    let method = request.get("method").and_then(Value::as_str)?;
    match method {
        "fs/read_text_file" => Some(acp_read_text_file_response(cwd, permission_engine, request)),
        "fs/write_text_file" => Some(acp_write_text_file_response(
            cwd,
            permission_engine,
            approver,
            request,
        )),
        _ => None,
    }
}

fn acp_read_text_file_response(
    cwd: &Path,
    permission_engine: &PermissionEngine,
    request: &Value,
) -> String {
    let id = acp_request_id(request);
    let Some(path) = acp_request_path(request) else {
        return acp_client_error_response(id, -32602, "fs/read_text_file requires params.path");
    };
    let input = acp_file_tool_input(&path, None);
    let tool = acp_file_tool_spec("read_file", false);
    match permission_engine.decide(&tool, &input) {
        PermissionDecision::Allow(_) => match read_acp_text_file(cwd, &path, request) {
            Ok(content) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": content
                }
            })
            .to_string(),
            Err(error) => acp_client_error_response(id, -32002, &error),
        },
        PermissionDecision::Ask(ask) => acp_client_error_response(id, -32003, &ask.message),
        PermissionDecision::Deny(deny) => acp_client_error_response(id, -32003, &deny.message),
    }
}

fn acp_write_text_file_response(
    cwd: &Path,
    permission_engine: &mut PermissionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    request: &Value,
) -> String {
    let id = acp_request_id(request);
    let Some(path) = acp_request_path(request) else {
        return acp_client_error_response(id, -32602, "fs/write_text_file requires params.path");
    };
    let Some(content) = request.pointer("/params/content").and_then(Value::as_str) else {
        return acp_client_error_response(id, -32602, "fs/write_text_file requires params.content");
    };
    let input = acp_file_tool_input(&path, Some(content));
    let tool = acp_file_tool_spec("write_file", true);
    let mut decision = permission_engine.decide(&tool, &input);
    if let PermissionDecision::Ask(ask) = &decision {
        let prompt = PermissionEngine::prompt_for("write_file", ask, &input);
        let approval = approver(prompt);
        decision = permission_engine.apply_approval(approval, ask, &tool, &input);
    }
    match decision {
        PermissionDecision::Allow(_) => match write_acp_text_file(cwd, &path, content) {
            Ok(()) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            })
            .to_string(),
            Err(error) => acp_client_error_response(id, -32002, &error),
        },
        PermissionDecision::Ask(_) => unreachable!("ask decisions should be resolved"),
        PermissionDecision::Deny(deny) => acp_client_error_response(id, -32003, &deny.message),
    }
}

fn acp_request_id(request: &Value) -> Value {
    request.get("id").cloned().unwrap_or(Value::Null)
}

fn acp_request_path(request: &Value) -> Option<String> {
    request
        .pointer("/params/path")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn acp_file_tool_input(path: &str, content: Option<&str>) -> ToolInput {
    let mut input = ToolInput::new();
    input.insert("path".to_string(), path.to_string());
    if let Some(content) = content {
        input.insert("content".to_string(), content.to_string());
    }
    input
}

fn acp_file_tool_spec(name: &str, is_mutating: bool) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: format!("ACP {name} client request"),
        is_mutating,
        input_schema_hint: "path=absolute/or/relative content=optional".to_string(),
    }
}

#[derive(Default)]
struct AcpTerminalStore {
    next_id: u64,
    records: BTreeMap<String, AcpTerminalRecord>,
}

struct AcpTerminalRecord {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    stderr: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    output: String,
    output_byte_limit: Option<u64>,
    truncated: bool,
    exit_code: Option<i32>,
    signal: Option<String>,
    released: bool,
    killed: bool,
}

fn acp_terminal_client_request_response(
    cwd: &Path,
    permission_engine: &mut PermissionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    terminals: &mut AcpTerminalStore,
    request: &Value,
) -> Option<String> {
    let method = request.get("method").and_then(Value::as_str)?;
    match method {
        "terminal/create" => Some(acp_terminal_create_response(
            cwd,
            permission_engine,
            approver,
            terminals,
            request,
        )),
        "terminal/input" | "terminal/write" => {
            Some(acp_terminal_input_response(terminals, request))
        }
        "terminal/output" => Some(acp_terminal_output_response(terminals, request)),
        "terminal/wait_for_exit" => Some(acp_terminal_wait_for_exit_response(terminals, request)),
        "terminal/release" => Some(acp_terminal_release_response(terminals, request)),
        "terminal/kill" => Some(acp_terminal_kill_response(terminals, request)),
        _ => None,
    }
}

fn acp_terminal_create_response(
    cwd: &Path,
    permission_engine: &mut PermissionEngine,
    approver: &mut impl FnMut(viden_types::PermissionPrompt) -> ApprovalResponse,
    terminals: &mut AcpTerminalStore,
    request: &Value,
) -> String {
    let id = acp_request_id(request);
    let Some(command) = request.pointer("/params/command").and_then(Value::as_str) else {
        return acp_client_error_response(id, -32602, "terminal/create requires params.command");
    };
    let args = acp_terminal_args(request);
    let exec_cwd = match acp_terminal_cwd(cwd, request) {
        Ok(path) => path,
        Err(error) => return acp_client_error_response(id, -32602, &error),
    };
    let command_preview = acp_terminal_command_preview(command, &args);
    let mut input = ToolInput::new();
    input.insert("command".to_string(), command_preview.clone());
    input.insert("path".to_string(), exec_cwd.display().to_string());
    let tool = ToolSpec {
        name: "shell".to_string(),
        description: "ACP terminal/create client request".to_string(),
        is_mutating: true,
        input_schema_hint: "command='cargo test' path=/workspace".to_string(),
    };
    let mut decision = permission_engine.decide(&tool, &input);
    if let PermissionDecision::Ask(ask) = &decision {
        let prompt = PermissionEngine::prompt_for("shell", ask, &input);
        let approval = approver(prompt);
        decision = permission_engine.apply_approval(approval, ask, &tool, &input);
    }
    match decision {
        PermissionDecision::Allow(_) => match spawn_acp_terminal_command(
            command,
            &args,
            &exec_cwd,
            acp_terminal_env(request),
            request
                .pointer("/params/outputByteLimit")
                .and_then(Value::as_u64),
        ) {
            Ok(record) => {
                terminals.next_id += 1;
                let terminal_id = format!("acp-terminal-{}", terminals.next_id);
                terminals.records.insert(terminal_id.clone(), record);
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "terminalId": terminal_id
                    }
                })
                .to_string()
            }
            Err(error) => acp_client_error_response(id, -32002, &error),
        },
        PermissionDecision::Ask(_) => unreachable!("ask decisions should be resolved"),
        PermissionDecision::Deny(deny) => acp_client_error_response(id, -32003, &deny.message),
    }
}

fn acp_terminal_input_response(terminals: &mut AcpTerminalStore, request: &Value) -> String {
    let id = acp_request_id(request);
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("terminal/input");
    let Some(terminal_id) = acp_request_terminal_id(request) else {
        return acp_client_error_response(
            id,
            -32602,
            &format!("{method} requires params.terminalId"),
        );
    };
    let Some(input) = acp_terminal_input_text(request) else {
        return acp_client_error_response(
            id,
            -32602,
            &format!("{method} requires params.input, params.text, params.content, or params.data"),
        );
    };
    let Some(record) = terminals.records.get_mut(&terminal_id) else {
        return acp_client_error_response(id, -32004, "unknown ACP terminal id");
    };
    acp_terminal_refresh(record);
    if record.child.is_none() {
        return acp_client_error_response(id, -32004, "ACP terminal is not running");
    }
    let Some(stdin) = record.stdin.as_mut() else {
        return acp_client_error_response(id, -32004, "ACP terminal stdin is unavailable");
    };
    if let Err(error) = stdin
        .write_all(input.as_bytes())
        .and_then(|_| stdin.flush())
    {
        record.stdin = None;
        return acp_client_error_response(
            id,
            -32002,
            &format!("failed to write ACP terminal input: {error}"),
        );
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "bytesWritten": input.len()
        }
    })
    .to_string()
}

fn acp_terminal_output_response(terminals: &mut AcpTerminalStore, request: &Value) -> String {
    let id = acp_request_id(request);
    let Some(terminal_id) = acp_request_terminal_id(request) else {
        return acp_client_error_response(id, -32602, "terminal/output requires params.terminalId");
    };
    let Some(record) = terminals.records.get_mut(&terminal_id) else {
        return acp_client_error_response(id, -32004, "unknown ACP terminal id");
    };
    acp_terminal_refresh(record);
    if record.output.is_empty() && record.child.is_some() {
        acp_terminal_poll_output(record, Duration::from_millis(150));
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "output": record.output,
            "truncated": record.truncated,
            "exitStatus": acp_terminal_exit_status(record)
        }
    })
    .to_string()
}

fn acp_terminal_wait_for_exit_response(
    terminals: &mut AcpTerminalStore,
    request: &Value,
) -> String {
    let id = acp_request_id(request);
    let Some(terminal_id) = acp_request_terminal_id(request) else {
        return acp_client_error_response(
            id,
            -32602,
            "terminal/wait_for_exit requires params.terminalId",
        );
    };
    let Some(record) = terminals.records.get_mut(&terminal_id) else {
        return acp_client_error_response(id, -32004, "unknown ACP terminal id");
    };
    acp_terminal_wait_for_exit(record);
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "exitCode": record.exit_code,
            "signal": record.signal
        }
    })
    .to_string()
}

fn acp_terminal_release_response(terminals: &mut AcpTerminalStore, request: &Value) -> String {
    let id = acp_request_id(request);
    let Some(terminal_id) = acp_request_terminal_id(request) else {
        return acp_client_error_response(
            id,
            -32602,
            "terminal/release requires params.terminalId",
        );
    };
    let Some(record) = terminals.records.get_mut(&terminal_id) else {
        return acp_client_error_response(id, -32004, "unknown ACP terminal id");
    };
    acp_terminal_terminate(record, "released");
    record.released = true;
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {}
    })
    .to_string()
}

fn acp_terminal_kill_response(terminals: &mut AcpTerminalStore, request: &Value) -> String {
    let id = acp_request_id(request);
    let Some(terminal_id) = acp_request_terminal_id(request) else {
        return acp_client_error_response(id, -32602, "terminal/kill requires params.terminalId");
    };
    let Some(record) = terminals.records.get_mut(&terminal_id) else {
        return acp_client_error_response(id, -32004, "unknown ACP terminal id");
    };
    acp_terminal_terminate(record, "killed");
    record.killed = true;
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {}
    })
    .to_string()
}

fn spawn_acp_terminal_command(
    command: &str,
    args: &[String],
    cwd: &Path,
    envs: Vec<(String, String)>,
    output_byte_limit: Option<u64>,
) -> Result<AcpTerminalRecord, String> {
    let mut process = Command::new(command);
    process
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in envs {
        process.env(name, value);
    }
    let mut child = process
        .spawn()
        .map_err(|err| format!("failed to run ACP terminal command `{command}`: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("failed to capture stdout for ACP terminal command `{command}`"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("failed to capture stderr for ACP terminal command `{command}`"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("failed to capture stdin for ACP terminal command `{command}`"))?;
    Ok(AcpTerminalRecord {
        child: Some(child),
        stdin: Some(stdin),
        stdout: read_bytes_async(stdout),
        stderr: read_bytes_async(stderr),
        output: String::new(),
        output_byte_limit,
        truncated: false,
        exit_code: None,
        signal: None,
        released: false,
        killed: false,
    })
}

fn acp_terminal_wait_timeout() -> Duration {
    env::var("VIDEN_ACP_TERMINAL_TIMEOUT_SECS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(30))
}

fn acp_terminal_refresh(record: &mut AcpTerminalRecord) {
    acp_terminal_drain_output(record);
    if let Some(child) = record.child.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                record.exit_code = status.code();
                record.stdin = None;
                record.child = None;
                acp_terminal_drain_output_after_exit(record);
            }
            Ok(None) => {}
            Err(error) => {
                record.signal = Some(format!("wait_error:{error}"));
                record.stdin = None;
                record.child = None;
            }
        }
    }
    acp_terminal_drain_output(record);
}

fn acp_terminal_drain_output_after_exit(record: &mut AcpTerminalRecord) {
    for _ in 0..10 {
        acp_terminal_drain_output(record);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn acp_terminal_wait_for_exit(record: &mut AcpTerminalRecord) {
    let deadline = Instant::now() + acp_terminal_wait_timeout();
    loop {
        acp_terminal_refresh(record);
        if record.child.is_none() {
            break;
        }
        if Instant::now() >= deadline {
            acp_terminal_terminate(record, "timeout");
            acp_terminal_append_output(
                record,
                format!(
                    "\nACP terminal command timed out after {}s",
                    acp_terminal_wait_timeout().as_secs()
                )
                .as_bytes(),
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    for _ in 0..10 {
        acp_terminal_drain_output(record);
        std::thread::sleep(Duration::from_millis(5));
    }
    acp_terminal_drain_output(record);
}

fn acp_terminal_poll_output(record: &mut AcpTerminalRecord, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline && record.output.is_empty() && record.child.is_some() {
        acp_terminal_refresh(record);
        if !record.output.is_empty() || record.child.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    acp_terminal_refresh(record);
}

fn acp_terminal_terminate(record: &mut AcpTerminalRecord, signal: &str) {
    record.stdin = None;
    if let Some(child) = record.child.as_mut() {
        let _ = child.kill();
        let exited = wait_child_timeout(child, Duration::from_secs(1));
        if exited && let Ok(Some(status)) = child.try_wait() {
            record.exit_code = status.code();
        }
        record.signal = Some(signal.to_string());
        record.child = None;
    }
    acp_terminal_drain_output(record);
}

fn acp_terminal_drain_output(record: &mut AcpTerminalRecord) {
    while let Ok(chunk) = record.stdout.try_recv() {
        match chunk {
            Ok(bytes) => acp_terminal_append_output(record, &bytes),
            Err(error) => {
                acp_terminal_append_output(record, format!("\nstdout error: {error}").as_bytes())
            }
        }
    }
    while let Ok(chunk) = record.stderr.try_recv() {
        match chunk {
            Ok(bytes) => acp_terminal_append_output(record, &bytes),
            Err(error) => {
                acp_terminal_append_output(record, format!("\nstderr error: {error}").as_bytes())
            }
        }
    }
}

fn acp_terminal_append_output(record: &mut AcpTerminalRecord, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    record.output.push_str(&String::from_utf8_lossy(bytes));
    let (output, truncated) =
        truncate_terminal_output(record.output.clone(), record.output_byte_limit);
    record.output = output;
    record.truncated |= truncated;
}

fn acp_terminal_args(request: &Value) -> Vec<String> {
    request
        .pointer("/params/args")
        .and_then(Value::as_array)
        .map(|args| {
            args.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn acp_terminal_input_text(request: &Value) -> Option<String> {
    [
        "/params/input",
        "/params/text",
        "/params/content",
        "/params/data",
    ]
    .iter()
    .find_map(|path| {
        request
            .pointer(path)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn acp_terminal_env(request: &Value) -> Vec<(String, String)> {
    request
        .pointer("/params/env")
        .and_then(Value::as_array)
        .map(|envs| {
            envs.iter()
                .filter_map(|entry| {
                    let name = entry.get("name").and_then(Value::as_str)?;
                    let value = entry.get("value").and_then(Value::as_str)?;
                    Some((name.to_string(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn acp_terminal_cwd(cwd: &Path, request: &Value) -> Result<PathBuf, String> {
    let raw = request
        .pointer("/params/cwd")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let path = raw
        .map(|raw| resolve_acp_path(cwd, raw))
        .unwrap_or_else(|| cwd.to_path_buf());
    if !path.exists() {
        return Err(format!("terminal cwd does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!(
            "terminal cwd is not a directory: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn acp_terminal_command_preview(command: &str, args: &[String]) -> String {
    std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn acp_request_terminal_id(request: &Value) -> Option<String> {
    request
        .pointer("/params/terminalId")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn acp_terminal_exit_status(record: &AcpTerminalRecord) -> Value {
    json!({
        "exitCode": record.exit_code,
        "signal": record.signal
    })
}

fn truncate_terminal_output(output: String, limit: Option<u64>) -> (String, bool) {
    let Some(limit) = limit.map(|value| value as usize) else {
        return (output, false);
    };
    if output.len() <= limit {
        return (output, false);
    }
    let mut start = output.len().saturating_sub(limit);
    while start < output.len() && !output.is_char_boundary(start) {
        start += 1;
    }
    (output[start..].to_string(), true)
}

fn read_acp_text_file(cwd: &Path, raw_path: &str, request: &Value) -> Result<String, String> {
    let path = resolve_acp_path(cwd, raw_path);
    if path.is_dir() {
        return Err(format!("`{}` is a directory", path.display()));
    }
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(slice_acp_file_content(&content, request))
}

fn write_acp_text_file(cwd: &Path, raw_path: &str, content: &str) -> Result<(), String> {
    let path = resolve_acp_path(cwd, raw_path);
    if path.is_dir() {
        return Err(format!("`{}` is a directory", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&path, content).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn resolve_acp_path(cwd: &Path, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn slice_acp_file_content(content: &str, request: &Value) -> String {
    let start_line = request
        .pointer("/params/startLine")
        .or_else(|| request.pointer("/params/line"))
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .saturating_sub(1) as usize;
    let limit = request
        .pointer("/params/limit")
        .or_else(|| request.pointer("/params/lineCount"))
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    let lines = content.lines().skip(start_line);
    match limit {
        Some(limit) => lines.take(limit).collect::<Vec<_>>().join("\n"),
        None => lines.collect::<Vec<_>>().join("\n"),
    }
}

fn acp_client_error_response(id: Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}

fn acp_unsupported_client_request_response(request: &Value) -> Option<String> {
    let method = request.get("method").and_then(Value::as_str)?;
    if !method.starts_with("fs/") && !method.starts_with("terminal/") {
        return None;
    }
    let id = request.get("id")?.clone();
    Some(
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32001,
                "message": format!("ACP client method {method} is not available through the Viden ACP runtime bridge")
            }
        })
        .to_string(),
    )
}

fn acp_permission_option_id(request: &Value, approved: bool) -> Option<String> {
    let options = request.pointer("/params/options")?.as_array()?;
    let preferred = options.iter().find(|option| {
        let kind = option
            .get("kind")
            .or_else(|| option.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        let option_id = option
            .get("optionId")
            .or_else(|| option.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        if approved {
            kind.contains("allow") || kind.contains("approve") || option_id.contains("allow")
        } else {
            kind.contains("reject") || kind.contains("deny") || option_id.contains("deny")
        }
    });
    preferred
        .or_else(|| options.first())
        .and_then(|option| {
            option
                .get("optionId")
                .or_else(|| option.get("id"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

fn acp_tool_call_summary(update: &Value) -> String {
    let id = acp_tool_call_id(update);
    let status = update
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("updated");
    let content = update
        .get("content")
        .and_then(Value::as_str)
        .or_else(|| update.get("title").and_then(Value::as_str))
        .unwrap_or("");
    if content.is_empty() {
        format!("{id}:{status}")
    } else {
        format!("{id}:{status}:{content}")
    }
}

fn acp_tool_call_id(update: &Value) -> String {
    update
        .get("toolCallId")
        .or_else(|| update.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("tool")
        .to_string()
}

fn acp_tool_call_title(update: &Value) -> String {
    update
        .get("title")
        .or_else(|| update.get("name"))
        .or_else(|| update.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("acp_tool")
        .to_string()
}

fn finish_failed_probe(
    mut child: Child,
    log_path: PathBuf,
    mut entries: Vec<String>,
    error: String,
) -> String {
    entries.push(jsonl_event("error", &error));
    let _ = child.kill();
    let exited = wait_child_timeout(&mut child, Duration::from_secs(2));
    let stderr = exited.then(|| child_stderr_tail(&mut child)).flatten();
    if let Some(stderr) = &stderr {
        entries.push(jsonl_event("stderr", stderr));
    }
    if !exited {
        entries.push(jsonl_event(
            "error",
            "ACP child did not exit within cleanup timeout after termination",
        ));
    }
    let _ = write_probe_log(&log_path, &entries);
    if let Some(stderr) = stderr {
        return format!("{error}; stderr: {stderr}; log {}", log_path.display());
    }
    format!("{error}; log {}", log_path.display())
}

fn finish_expected_acp_stop(
    mut child: Child,
    log_path: PathBuf,
    mut entries: Vec<String>,
    message: String,
) -> String {
    entries.push(jsonl_event("error", &message));
    let _ = write_probe_log(&log_path, &entries);
    let _ = child.kill();
    let _ = wait_child_timeout(&mut child, Duration::from_secs(1));
    format!("{message}; log {}", log_path.display())
}

fn child_stderr_tail(child: &mut Child) -> Option<String> {
    let mut stderr = child.stderr.take()?;
    let mut text = String::new();
    stderr.read_to_string(&mut text).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(tail_lines(text, 8))
    }
}

fn wait_child_timeout(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => return false,
        }
    }
    false
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

fn acp_initialize_request() -> String {
    [
        r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"#,
        r#""protocolVersion":1,"#,
        r#""clientCapabilities":{"fs":{"readTextFile":true,"writeTextFile":true},"terminal":true},"#,
        r#""clientInfo":{"name":"viden","version":"0.1.6"}}"#,
        r#"}"#,
    ]
    .join("")
}

fn spawn_acp_process(cwd: &Path, command: &str) -> Result<Child, String> {
    let mut command = shell_command(cwd, command)?;
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to launch ACP command: {err}"))
}

fn spawn_acp_agent_process(cwd: &Path, agent: &AgentPluginDescriptor) -> Result<Child, String> {
    let mut command = Command::new(&agent.command.command);
    command
        .args(acp_agent_command_args(agent))
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_acp_agent_process_env(cwd, agent, &mut command)?;
    command.spawn().map_err(|err| {
        format!(
            "failed to launch ACP agent `{}` with `{}`: {err}",
            agent.agent_id,
            agent_command_line(agent)
        )
    })
}

fn configure_acp_agent_process_env(
    cwd: &Path,
    agent: &AgentPluginDescriptor,
    command: &mut Command,
) -> Result<(), String> {
    if matches!(agent.source, AgentSource::Registry) {
        let cache_dir = cwd.join(".viden").join("cache").join("npm");
        fs::create_dir_all(&cache_dir).map_err(|err| {
            format!(
                "failed to create ACP registry npm cache {}: {err}",
                cache_dir.display()
            )
        })?;
        command
            .env("npm_config_cache", &cache_dir)
            .env("NPM_CONFIG_CACHE", &cache_dir)
            .env("npm_config_audit", "false")
            .env("npm_config_fund", "false")
            .env("npm_config_update_notifier", "false");
    }
    Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellCommandPlan {
    program: &'static str,
    inline_args: Vec<String>,
    script_extension: Option<&'static str>,
    script_body: Option<String>,
}

fn shell_command_plan(command: &str, windows: bool) -> ShellCommandPlan {
    let requires_script = command.len() > SHELL_SCRIPT_THRESHOLD;
    if windows {
        return ShellCommandPlan {
            program: "cmd",
            inline_args: if requires_script {
                vec!["/C".to_string()]
            } else {
                vec!["/C".to_string(), command.to_string()]
            },
            script_extension: requires_script.then_some("cmd"),
            script_body: requires_script.then(|| command.to_string()),
        };
    }

    ShellCommandPlan {
        program: "sh",
        inline_args: if requires_script {
            Vec::new()
        } else {
            vec!["-lc".to_string(), command.to_string()]
        },
        script_extension: requires_script.then_some("sh"),
        script_body: requires_script.then(|| format!("set -eu\n{command}\n")),
    }
}

fn shell_command(cwd: &Path, command: &str) -> Result<Command, String> {
    let plan = shell_command_plan(command, cfg!(windows));
    let mut process = Command::new(plan.program);
    process.args(plan.inline_args);
    if let Some(body) = plan.script_body {
        let extension = plan.script_extension.unwrap_or("cmd");
        let path = acp_shell_script_path(cwd, extension);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create ACP shell script dir: {err}"))?;
        }
        fs::write(&path, body).map_err(|err| format!("failed to write ACP shell script: {err}"))?;
        process.arg(path);
    }
    Ok(process)
}

fn acp_shell_script_path(cwd: &Path, extension: &str) -> PathBuf {
    cwd.join(".viden")
        .join("tmp")
        .join(format!("acp-command-{}.{}", timestamp_millis(), extension))
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

fn read_bytes_async(
    mut reader: impl std::io::Read + Send + 'static,
) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if sender.send(Ok(buffer[..size].to_vec())).is_err() {
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
    stdin: &mut impl Write,
    request_id: u32,
    log_entries: &mut Vec<String>,
    notifications: &mut Vec<String>,
    approval_requests: &mut Vec<String>,
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
            if is_codex_app_server_request(&method) {
                if let Some(response) =
                    codex_app_server_request_denial_response(&line, &method, approval_requests)
                {
                    log_entries.push(jsonl_event("client", &response));
                    stdin
                        .write_all(response.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush())
                        .map_err(|err| {
                            format!("failed to answer Codex app-server request: {err}")
                        })?;
                }
            } else {
                record_codex_app_server_notification(&line, &method, notifications);
            }
        }
    }
    Err(format!(
        "Codex app-server request {request_id} response timed out"
    ))
}

fn collect_codex_app_server_notifications(
    receiver: &mpsc::Receiver<std::io::Result<String>>,
    stdin: &mut impl Write,
    log_entries: &mut Vec<String>,
    notifications: &mut Vec<String>,
    approval_requests: &mut Vec<String>,
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
            if is_codex_app_server_request(&method) {
                if let Some(response) =
                    codex_app_server_request_denial_response(&line, &method, approval_requests)
                {
                    log_entries.push(jsonl_event("client", &response));
                    let _ = stdin
                        .write_all(response.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush());
                }
            } else {
                record_codex_app_server_notification(&line, &method, notifications);
            }
            if until_method.is_some_and(|target| target == method) {
                return Some(line);
            }
        }
    }
    None
}

fn codex_app_server_final_message(log_entries: &[String]) -> Option<String> {
    log_entries.iter().rev().find_map(|entry| {
        if !entry.contains("agentMessage") || !entry.contains("text") {
            return None;
        }
        let normalized = entry.replace("\\\"", "\"");
        json_string_field(&normalized, "text")
            .map(|message| message.chars().take(500).collect::<String>())
            .filter(|message| !message.trim().is_empty())
    })
}

fn record_codex_app_server_notification(line: &str, method: &str, notifications: &mut Vec<String>) {
    notifications.push(method.to_string());
    // Some current Codex app-server write-capable turns perform work through
    // MCP tool items instead of emitting fileChange/fs notifications. Preserve
    // those as explicit signals so operator evidence does not collapse to
    // `signals: none` after a real workspace mutation.
    if line.contains(r#""type":"mcpToolCall""#) {
        notifications.push("item/mcpToolCall".to_string());
        if line.contains(r#""status":"completed""#) {
            notifications.push("item/mcpToolCall/completed".to_string());
        }
        if line.contains("writeFile") || line.contains("node:fs") {
            notifications.push("item/mcpToolCall/fs-write".to_string());
        }
    }
}

fn is_codex_app_server_request(method: &str) -> bool {
    matches!(
        method,
        "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
            | "execCommandApproval"
            | "applyPatchApproval"
    )
}

fn codex_app_server_request_denial_response(
    line: &str,
    method: &str,
    approval_requests: &mut Vec<String>,
) -> Option<String> {
    let id = json_number_field(line, "id")?;
    approval_requests.push(method.to_string());
    let result = match method {
        "item/commandExecution/requestApproval" => r#"{"decision":"decline"}"#,
        "item/fileChange/requestApproval" => r#"{"decision":"decline"}"#,
        "item/permissions/requestApproval" => {
            r#"{"permissions":{},"scope":"turn","strictAutoReview":true}"#
        }
        "execCommandApproval" | "applyPatchApproval" => r#"{"decision":"denied"}"#,
        _ => return None,
    };
    Some(format!(r#"{{"id":{id},"result":{result}}}"#))
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
    cwd.join(".viden")
        .join("agents")
        .join(format!("acp-doctor-{}.jsonl", timestamp_millis()))
}

fn acp_session_log_path(cwd: &Path) -> PathBuf {
    cwd.join(".viden")
        .join("agents")
        .join(format!("acp-session-{}.jsonl", timestamp_millis()))
}

fn codex_app_server_probe_log_path(cwd: &Path) -> PathBuf {
    cwd.join(".viden")
        .join("agents")
        .join(format!("codex-app-server-{}.jsonl", timestamp_millis()))
}

fn codex_app_server_initialize_request() -> String {
    [
        r#"{"id":1,"method":"initialize","params":{"clientInfo":{"name":"viden","version":""#,
        env!("CARGO_PKG_VERSION"),
        r#""},"capabilities":{"experimentalApi":true,"requestAttestation":false,"optOutNotificationMethods":[]}}}"#,
    ]
    .join("")
}

fn codex_app_server_thread_start_request(cwd: &Path, write: bool) -> String {
    let cwd = escape_json_fragment(&cwd.display().to_string());
    let approval_policy = if write { "on-request" } else { "never" };
    let sandbox = if write {
        "workspace-write"
    } else {
        "read-only"
    };
    format!(
        r#"{{"id":2,"method":"thread/start","params":{{"model":null,"modelProvider":null,"cwd":"{cwd}","runtimeWorkspaceRoots":["{cwd}"],"approvalPolicy":"{approval_policy}","approvalsReviewer":"user","sandbox":"{sandbox}","permissions":null,"config":null,"serviceName":"viden","baseInstructions":null,"developerInstructions":null,"personality":null,"ephemeral":true,"sessionStartSource":"startup","threadSource":"subagent","environments":[],"dynamicTools":null,"experimentalRawEvents":false,"persistExtendedHistory":false}}}}"#
    )
}

fn codex_app_server_turn_start_request(
    cwd: &Path,
    thread_id: &str,
    task: &str,
    write: bool,
) -> String {
    let cwd = escape_json_fragment(&cwd.display().to_string());
    let thread_id = escape_json_fragment(thread_id);
    let task = escape_json_fragment(task);
    let approval_policy = if write { "on-request" } else { "never" };
    let sandbox_policy = if write {
        format!(r#"{{"type":"workspaceWrite","writableRoots":["{cwd}"],"networkAccess":false}}"#)
    } else {
        r#"{"type":"readOnly","networkAccess":false}"#.to_string()
    };
    format!(
        r#"{{"id":3,"method":"turn/start","params":{{"threadId":"{thread_id}","input":[{{"type":"text","text":"{task}","text_elements":[]}}],"responsesapiClientMetadata":null,"environments":[],"cwd":"{cwd}","runtimeWorkspaceRoots":["{cwd}"],"approvalPolicy":"{approval_policy}","approvalsReviewer":"user","sandboxPolicy":{sandbox_policy},"permissions":null,"model":null,"serviceTier":null,"effort":null,"summary":null,"personality":null,"outputSchema":null,"collaborationMode":null}}}}"#
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

fn append_agent_job_log_event(path: &Path, direction: &str, payload: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    writeln!(file, "{}", jsonl_event(direction, payload))
        .map_err(|err| format!("failed to append {}: {err}", path.display()))
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

fn acp_probe_evidence_from_response(response: &str, log_path: PathBuf) -> AcpProbeEvidence {
    let Ok(value) = serde_json::from_str::<Value>(response) else {
        return AcpProbeEvidence {
            protocol_version: json_number_field(response, "protocolVersion")
                .unwrap_or_else(|| "unknown".to_string()),
            agent_label: agent_label_from_response(response),
            auth_methods: Vec::new(),
            auth_method_ids: Vec::new(),
            capabilities: Vec::new(),
            log_path,
        };
    };
    let protocol_version = value
        .pointer("/result/protocolVersion")
        .and_then(Value::as_i64)
        .map(|version| version.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let name = value
        .pointer("/result/agentInfo/name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let version = value
        .pointer("/result/agentInfo/version")
        .and_then(Value::as_str);
    let agent_label = match version {
        Some(version) => format!("{name} {version}"),
        None => name.to_string(),
    };
    let (auth_methods, auth_method_ids) = acp_auth_methods_from_value(&value);
    AcpProbeEvidence {
        protocol_version,
        agent_label,
        auth_methods,
        auth_method_ids,
        capabilities: acp_capabilities_from_value(&value),
        log_path,
    }
}

fn acp_auth_methods_from_value(value: &Value) -> (Vec<String>, Vec<String>) {
    let Some(methods) = value
        .pointer("/result/authMethods")
        .and_then(Value::as_array)
    else {
        return (Vec::new(), Vec::new());
    };
    let mut labels = Vec::new();
    let mut ids = Vec::new();
    for method in methods {
        let Some(id) = method.get("id").and_then(Value::as_str) else {
            continue;
        };
        ids.push(id.to_string());
        let name = method
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .trim();
        if name == id {
            labels.push(id.to_string());
        } else {
            labels.push(format!("{id} ({name})"));
        }
    }
    (labels, ids)
}

fn acp_capabilities_from_value(value: &Value) -> Vec<String> {
    let mut capabilities = Vec::new();
    if let Some(object) = value
        .pointer("/result/agentCapabilities")
        .and_then(Value::as_object)
    {
        for (key, item) in object {
            match item {
                Value::Bool(true) => capabilities.push(key.clone()),
                Value::Object(nested) if nested.is_empty() => capabilities.push(key.clone()),
                Value::Object(nested) => {
                    for nested_key in nested.keys() {
                        capabilities.push(format!("{key}.{nested_key}"));
                    }
                }
                _ => {}
            }
        }
    }
    capabilities.sort();
    capabilities
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
    use std::cell::{Cell, RefCell};
    use std::sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    };
    use viden_plugin_api::{AgentAuthMode, AgentCommandSpec, AgentProtocolVersion};

    static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);
    static SUBPROCESS_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
    fn codex_probe_args_support_opt_in_write_turns() {
        assert_eq!(
            parse_codex_probe_args(&["--turn".into(), "summarize".into()])
                .expect("parse read-only turn"),
            CodexProbeMode::Turn {
                task: "summarize".to_string(),
                write: false,
            }
        );
        assert_eq!(
            parse_codex_probe_args(&["--turn-write".into(), "edit".into(), "file".into()])
                .expect("parse write turn"),
            CodexProbeMode::Turn {
                task: "edit file".to_string(),
                write: true,
            }
        );
    }

    #[test]
    fn codex_app_server_write_probe_requests_workspace_write_with_approval() {
        let cwd = Path::new("/tmp/viden-write-probe");
        let thread = codex_app_server_thread_start_request(cwd, true);
        let turn = codex_app_server_turn_start_request(cwd, "thread_1", "edit file", true);

        assert!(thread.contains(r#""approvalPolicy":"on-request""#));
        assert!(thread.contains(r#""sandbox":"workspace-write""#));
        assert!(turn.contains(r#""approvalPolicy":"on-request""#));
        assert!(turn.contains(r#""type":"workspaceWrite""#));
        assert!(turn.contains(r#""writableRoots":["/tmp/viden-write-probe"]"#));
        assert!(turn.contains(r#""networkAccess":false"#));
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

    #[test]
    fn acp_shell_command_uses_script_for_long_commands() {
        let long_command = format!("printf ok\n# {}", "x".repeat(40 * 1024));
        let plan = shell_command_plan(&long_command, false);

        assert_eq!(plan.program, "sh");
        assert!(plan.inline_args.is_empty());
        assert_eq!(plan.script_extension, Some("sh"));
        assert_eq!(
            plan.script_body.as_deref(),
            Some(format!("set -eu\n{long_command}\n").as_str())
        );
    }

    #[test]
    fn acp_shell_command_keeps_short_commands_inline() {
        let plan = shell_command_plan("printf ok", false);

        assert_eq!(plan.program, "sh");
        assert_eq!(
            plan.inline_args,
            vec!["-lc".to_string(), "printf ok".to_string()]
        );
        assert!(plan.script_extension.is_none());
        assert!(plan.script_body.is_none());
    }

    #[test]
    fn acp_shell_command_writes_long_command_script() {
        let cwd = temp_root("acp_shell_script");
        let long_command = format!("printf ok\n# {}", "x".repeat(40 * 1024));

        let _command = shell_command(&cwd, &long_command).expect("build shell command");

        let tmp_dir = cwd.join(".viden").join("tmp");
        let scripts = fs::read_dir(&tmp_dir)
            .expect("read tmp dir")
            .map(|entry| entry.expect("script entry").path())
            .collect::<Vec<_>>();
        assert_eq!(scripts.len(), 1);
        let script = fs::read_to_string(&scripts[0]).expect("read script");
        assert!(script.starts_with("set -eu\nprintf ok"));
        assert!(script.ends_with('\n'));
    }

    #[cfg(unix)]
    #[test]
    fn codex_app_server_initialize_probe_records_jsonl_evidence() {
        let _guard = subprocess_test_guard();
        let root = temp_root("codex_app_server_probe");
        let script = root.join("mock-codex-app-server.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" != \"app-server\" ]; then exit 2; fi",
                "read _line",
                "printf '%s\\n' '{\"id\":1,\"result\":{\"userAgent\":\"Codex Desktop/mock (viden; test)\",\"codexHome\":\"/tmp/codex-home\",\"platformFamily\":\"unix\",\"platformOs\":\"macos\"}}'",
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

        assert_eq!(evidence.user_agent, "Codex Desktop/mock (viden; test)");
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
        let _guard = subprocess_test_guard();
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
        let _guard = subprocess_test_guard();
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
                "printf '%s\\n' '{\"method\":\"item/started\",\"params\":{\"threadId\":\"thread_456\",\"turnId\":\"turn_456\",\"item\":{\"type\":\"mcpToolCall\",\"id\":\"call_1\",\"server\":\"node_repl\",\"tool\":\"js\",\"status\":\"inProgress\",\"arguments\":{\"code\":\"await fs.writeFile(\\\\\"live.txt\\\\\", \\\\\"ok\\\\\")\"}}}}'",
                "printf '%s\\n' '{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread_456\",\"turnId\":\"turn_456\",\"item\":{\"type\":\"mcpToolCall\",\"id\":\"call_1\",\"server\":\"node_repl\",\"tool\":\"js\",\"status\":\"completed\",\"arguments\":{\"code\":\"await fs.writeFile(\\\\\"live.txt\\\\\", \\\\\"ok\\\\\")\"},\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}}}'",
                "printf '%s\\n' '{\"id\":9,\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"threadId\":\"thread_456\",\"turnId\":\"turn_456\",\"itemId\":\"item_1\",\"startedAtMs\":1,\"command\":\"cargo test\",\"cwd\":\"/tmp\"}}'",
                "read approval",
                "case \"$approval\" in *'\"id\":9'*'\"decision\":\"decline\"'*) ;; *) exit 5 ;; esac",
                "printf '%s\\n' '{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread_456\",\"turnId\":\"turn_456\",\"item\":{\"type\":\"agentMessage\",\"text\":\"turn probe complete\"}}}'",
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
            CodexProbeMode::Turn {
                task: "summarize status".to_string(),
                write: false,
            },
        )
        .expect("probe succeeds");

        assert_eq!(evidence.thread_id, Some("thread_456".to_string()));
        assert_eq!(evidence.turn_id, Some("turn_456".to_string()));
        assert_eq!(evidence.turn_status, Some("completed".to_string()));
        assert_eq!(
            evidence.final_message,
            Some("turn probe complete".to_string())
        );
        assert_eq!(
            evidence.approval_requests,
            vec!["item/commandExecution/requestApproval".to_string()]
        );
        assert!(
            evidence
                .notifications
                .contains(&"item/agentMessage/delta".to_string())
        );
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#"turn/start"#));
        assert!(log.contains(r#"\"decision\":\"decline\""#));
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
        assert!(result.contains("resume: thread_456"));
        assert!(result.contains("message: turn probe complete"));
        assert!(result.contains("approvals: item/commandExecution/requestApproval"));
        assert!(result.contains("signals: mcp-tool-call, mcp-tool-completed, mcp-fs-write"));
    }

    #[test]
    fn codex_app_server_signal_summary_reports_protocol_evidence() {
        let notifications = vec![
            "thread/started".to_string(),
            "item/commandExecution/outputDelta".to_string(),
            "item/fileChange/outputDelta".to_string(),
            "item/fileChange/patchUpdated".to_string(),
            "turn/diff/updated".to_string(),
            "fs/changed".to_string(),
            "item/mcpToolCall".to_string(),
            "item/mcpToolCall/completed".to_string(),
            "item/mcpToolCall/fs-write".to_string(),
            "error".to_string(),
        ];

        assert_eq!(
            codex_app_server_signal_summary(&notifications),
            "command-output, file-change, file-patch, diff-updated, fs-changed, mcp-tool-call, mcp-tool-completed, mcp-fs-write, app-server-error"
        );
        assert_eq!(codex_app_server_signal_summary(&[]), "none");
    }

    #[cfg(unix)]
    #[test]
    fn codex_app_server_job_records_async_status() {
        let _guard = subprocess_test_guard();
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
                "printf '%s\\n' '{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread_job\",\"turnId\":\"turn_job\",\"item\":{\"type\":\"agentMessage\",\"text\":\"async job complete\"}}}'",
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
            Duration::from_secs(15),
        );

        let status = render_codex_job_status(&root).expect("render job status");
        let result = render_codex_job_result(&root, Some(&id)).expect("render job result");
        assert!(status.contains(&id));
        assert!(status.contains("finished"));
        assert!(result.contains("thread_job"));
        assert!(result.contains("turn_job"));
        assert!(result.contains("resume: thread_job"));
        assert!(result.contains("message: async job complete"));
        assert!(result.contains("signals: none"));
    }

    #[test]
    fn acp_initialize_probe_records_jsonl_evidence() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_probe_ok");
        let script = root.join("mock-acp.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _line",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true,\"promptCapabilities\":{\"image\":true}},\"agentInfo\":{\"name\":\"mock-acp\",\"version\":\"0.1.0\"},\"authMethods\":[{\"id\":\"api-key\",\"name\":\"API Key\"},{\"id\":\"browser\",\"name\":\"Browser Login\"}]}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp script");
        make_executable(&script);

        let evidence =
            run_acp_initialize_probe(&root, &script.to_string_lossy()).expect("probe succeeds");

        assert_eq!(evidence.protocol_version, "1");
        assert_eq!(evidence.agent_label, "mock-acp 0.1.0");
        assert_eq!(
            evidence.auth_methods,
            vec!["api-key (API Key)", "browser (Browser Login)"]
        );
        assert_eq!(evidence.auth_method_ids, vec!["api-key", "browser"]);
        assert!(evidence.capabilities.contains(&"loadSession".to_string()));
        assert!(
            evidence
                .capabilities
                .contains(&"promptCapabilities.image".to_string())
        );
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#""method\":\"initialize"#));
        assert!(log.contains("mock-acp"));
    }

    #[test]
    fn acp_auth_command_lists_methods_when_choice_required() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_auth_choose");
        let script = root.join("mock-acp-auth-choose.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"auth\":{\"logout\":{}}},\"agentInfo\":{\"name\":\"mock-auth\",\"version\":\"0.1.0\"},\"authMethods\":[{\"id\":\"api-key\",\"name\":\"API Key\"},{\"id\":\"browser\",\"name\":\"Browser Login\"}]}}'",
                "sleep 1",
            ]
            .join("\n"),
        )
        .expect("write mock acp auth choose script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-auth-choose", &script);

        let error = run_acp_authenticate_for_agent(&root, &descriptor, None)
            .expect_err("multiple methods require explicit choice");

        assert!(error.contains("choose an auth method"));
        assert!(error.contains("api-key (API Key)"));
        assert!(error.contains("browser (Browser Login)"));
    }

    #[test]
    fn acp_auth_command_sends_authenticate_method() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_auth_method");
        let script = root.join("mock-acp-auth-method.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"auth\":{\"logout\":{}}},\"agentInfo\":{\"name\":\"mock-auth\",\"version\":\"0.1.0\"},\"authMethods\":[{\"id\":\"browser\",\"name\":\"Browser Login\"}]}}'",
                "read auth",
                "case \"$auth\" in *'\"method\":\"authenticate\"'*'\"methodId\":\"browser\"'*) ;; *) echo \"$auth\" >&2; exit 5 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"status\":\"ok\"}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp auth method script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-auth-method", &script);

        let evidence = run_acp_authenticate_for_agent(&root, &descriptor, Some("browser"))
            .expect("auth succeeds");

        assert_eq!(evidence.method_id, "browser");
        assert_eq!(evidence.status, "ok");
        let log = fs::read_to_string(&evidence.log_path).expect("read auth log");
        assert!(log.contains("authenticate"));
        assert!(log.contains("browser"));
    }

    #[test]
    fn acp_session_new_uses_mcp_server_array() {
        let request = acp_session_new_request(Path::new("/repo"));
        let value: Value = serde_json::from_str(&request).expect("valid session/new json");

        assert_eq!(
            value.get("method").and_then(Value::as_str),
            Some("session/new")
        );
        assert!(
            value
                .pointer("/params/mcpServers")
                .is_some_and(Value::is_array)
        );
    }

    #[test]
    fn acp_run_args_parse_session_configuration() {
        let args = vec![
            "--async".to_string(),
            "--load-session".to_string(),
            "session_old".to_string(),
            "--mode".to_string(),
            "plan".to_string(),
            "--model".to_string(),
            "claude-sonnet".to_string(),
            "kiro-cli".to_string(),
            "continue".to_string(),
            "work".to_string(),
        ];

        let parsed = parse_acp_run_args(&args).expect("parse acp run args");

        assert!(parsed.async_job);
        assert_eq!(parsed.agent_id, "kiro-cli");
        assert_eq!(parsed.task, "continue work");
        assert_eq!(
            parsed.session.load_session_id.as_deref(),
            Some("session_old")
        );
        assert_eq!(parsed.session.mode_id.as_deref(), Some("plan"));
        assert_eq!(parsed.session.model_id.as_deref(), Some("claude-sonnet"));
    }

    #[test]
    fn acp_session_load_uses_required_schema_fields() {
        let request = acp_session_load_request(Path::new("/repo"), "session_old");
        let value: Value = serde_json::from_str(&request).expect("valid session/load json");

        assert_eq!(
            value.get("method").and_then(Value::as_str),
            Some("session/load")
        );
        assert_eq!(
            value.pointer("/params/sessionId").and_then(Value::as_str),
            Some("session_old")
        );
        assert!(
            value
                .pointer("/params/mcpServers")
                .is_some_and(Value::is_array)
        );
    }

    #[test]
    fn acp_session_configuration_requests_use_schema_shapes() {
        let set_mode = acp_session_set_mode_request("session_1", "plan", 2);
        let set_model = acp_session_set_model_request("session_1", "claude-sonnet", 3);
        let legacy_set_model =
            acp_legacy_session_set_model_request("session_1", "claude-sonnet", 4);
        let set_mode: Value = serde_json::from_str(&set_mode).expect("valid set_mode json");
        let set_model: Value = serde_json::from_str(&set_model).expect("valid set_model json");
        let legacy_set_model: Value =
            serde_json::from_str(&legacy_set_model).expect("valid legacy set_model json");

        assert_eq!(
            set_mode.get("method").and_then(Value::as_str),
            Some("session/set_mode")
        );
        assert_eq!(
            set_mode.pointer("/params/modeId").and_then(Value::as_str),
            Some("plan")
        );
        assert_eq!(
            set_model.get("method").and_then(Value::as_str),
            Some("session/set_config_option")
        );
        assert_eq!(
            set_model
                .pointer("/params/configId")
                .and_then(Value::as_str),
            Some("model")
        );
        assert_eq!(
            set_model.pointer("/params/value").and_then(Value::as_str),
            Some("claude-sonnet")
        );
        assert_eq!(
            legacy_set_model.get("method").and_then(Value::as_str),
            Some("session/set_model")
        );
        assert_eq!(
            legacy_set_model
                .pointer("/params/modelId")
                .and_then(Value::as_str),
            Some("claude-sonnet")
        );
    }

    #[test]
    fn acp_session_prompt_uses_prompt_array() {
        let descriptor = mock_acp_descriptor("mock-codex-style", Path::new("mock"));
        let request = acp_session_prompt_request(&descriptor, "session_1", "hello", 2);
        let value: Value = serde_json::from_str(&request).expect("valid session/prompt json");

        assert_eq!(
            value.get("method").and_then(Value::as_str),
            Some("session/prompt")
        );
        assert!(value.pointer("/params/prompt").is_some_and(Value::is_array));
        assert!(value.pointer("/params/content").is_none());
    }

    #[test]
    fn acp_response_reader_reports_closed_stdout_before_timeout() {
        let (_sender, receiver) = mpsc::channel::<std::io::Result<String>>();
        drop(_sender);
        let mut log_entries = Vec::new();

        let error =
            read_acp_response_line(&receiver, 1, &mut log_entries, Duration::from_millis(50))
                .expect_err("closed ACP stdout should not wait for timeout");

        assert!(error.contains("closed stdout before response id 1"));
    }

    #[test]
    fn acp_kiro_session_prompt_uses_prompt_array() {
        let mut descriptor = mock_acp_descriptor("kiro-cli", Path::new("kiro-cli"));
        descriptor.source = AgentSource::LocalCommand;
        descriptor.command.command = "kiro-cli".to_string();
        descriptor.command.args = vec!["acp".to_string()];

        let request = acp_session_prompt_request(&descriptor, "session_1", "hello", 2);
        let value: Value = serde_json::from_str(&request).expect("valid Kiro session/prompt json");

        assert_eq!(
            value.get("method").and_then(Value::as_str),
            Some("session/prompt")
        );
        assert!(value.pointer("/params/prompt").is_some_and(Value::is_array));
        assert!(value.pointer("/params/content").is_none());
    }

    #[test]
    fn acp_kiro_agent_env_adds_agent_selector_arg() {
        let _guard = subprocess_test_guard();
        let mut descriptor = mock_acp_descriptor("kiro-cli", Path::new("kiro-cli"));
        descriptor.source = AgentSource::LocalCommand;
        descriptor.command.command = "kiro-cli".to_string();
        descriptor.command.args = vec!["acp".to_string()];
        unsafe {
            env::set_var("VIDEN_KIRO_AGENT", "team-agent");
        }

        let args = acp_agent_command_args(&descriptor);

        unsafe {
            env::remove_var("VIDEN_KIRO_AGENT");
        }
        assert_eq!(args, vec!["acp", "--agent", "team-agent"]);
    }

    #[test]
    fn acp_kiro_env_maps_official_acp_launch_options() {
        let _guard = subprocess_test_guard();
        let mut descriptor = mock_acp_descriptor("kiro-cli", Path::new("kiro-cli"));
        descriptor.source = AgentSource::LocalCommand;
        descriptor.command.command = "kiro-cli".to_string();
        descriptor.command.args = vec!["acp".to_string()];
        unsafe {
            env::set_var("VIDEN_KIRO_MODEL", "claude-sonnet-4");
            env::set_var("VIDEN_KIRO_EFFORT", "high");
            env::set_var(
                "VIDEN_KIRO_TRUST_TOOLS",
                "fs/read_text_file,terminal/create",
            );
            env::set_var("VIDEN_KIRO_AGENT_ENGINE", "v3");
        }

        let args = acp_agent_command_args(&descriptor);

        unsafe {
            env::remove_var("VIDEN_KIRO_MODEL");
            env::remove_var("VIDEN_KIRO_EFFORT");
            env::remove_var("VIDEN_KIRO_TRUST_TOOLS");
            env::remove_var("VIDEN_KIRO_AGENT_ENGINE");
        }
        assert_eq!(
            args,
            vec![
                "acp",
                "--model",
                "claude-sonnet-4",
                "--effort",
                "high",
                "--trust-tools",
                "fs/read_text_file,terminal/create",
                "--agent-engine",
                "v3"
            ]
        );
    }

    #[test]
    fn acp_kiro_trust_all_tools_overrides_trust_tools() {
        let _guard = subprocess_test_guard();
        let mut descriptor = mock_acp_descriptor("kiro-cli", Path::new("kiro-cli"));
        descriptor.source = AgentSource::LocalCommand;
        descriptor.command.command = "kiro-cli".to_string();
        descriptor.command.args = vec!["acp".to_string()];
        unsafe {
            env::set_var("VIDEN_KIRO_TRUST_ALL_TOOLS", "true");
            env::set_var("VIDEN_KIRO_TRUST_TOOLS", "fs/read_text_file");
        }

        let args = acp_agent_command_args(&descriptor);

        unsafe {
            env::remove_var("VIDEN_KIRO_TRUST_ALL_TOOLS");
            env::remove_var("VIDEN_KIRO_TRUST_TOOLS");
        }
        assert_eq!(args, vec!["acp", "--trust-all-tools"]);
    }

    #[test]
    fn acp_initialize_probe_uses_agent_descriptor_command_args() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_descriptor_probe_ok");
        let script = root.join("mock-acp-agent.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "if [ \"$1\" != \"acp\" ]; then exit 2; fi",
                "read _line",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true,\"promptCapabilities\":{\"image\":true}},\"agentInfo\":{\"name\":\"mock-descriptor-acp\",\"version\":\"0.2.0\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp agent script");
        make_executable(&script);
        let descriptor = AgentPluginDescriptor {
            agent_id: "mock-acp".to_string(),
            display_name: "Mock ACP".to_string(),
            version: "0.2.0".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: script.display().to_string(),
                args: vec!["acp".to_string()],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::StreamingUpdates,
            ],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec![],
            config_schema_version: 1,
        };

        let evidence =
            run_acp_initialize_probe_for_agent(&root, &descriptor).expect("probe succeeds");

        assert_eq!(evidence.protocol_version, "1");
        assert_eq!(evidence.agent_label, "mock-descriptor-acp 0.2.0");
        let log = fs::read_to_string(&evidence.log_path).expect("read jsonl log");
        assert!(log.contains("mock-descriptor-acp"));
    }

    #[test]
    fn acp_session_prompt_collects_streamed_updates() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_prompt");
        let script = root.join("mock-acp-session.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read init",
                "case \"$init\" in *'\"method\":\"initialize\"'*) ;; *) exit 2 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true},\"agentInfo\":{\"name\":\"mock-session-acp\",\"version\":\"0.3.0\"}}}'",
                "read new_session",
                "case \"$new_session\" in *'\"method\":\"session/new\"'*'\"cwd\"'*) ;; *) exit 3 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_123\"}}'",
                "read prompt",
                "case \"$prompt\" in *'\"method\":\"session/prompt\"'*'build a plan'*) ;; *) exit 4 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_123\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"Planning\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_123\",\"update\":{\"type\":\"ToolCall\",\"toolCallId\":\"tool_1\",\"title\":\"Read files\",\"status\":\"pending\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_123\",\"update\":{\"type\":\"ToolCallUpdate\",\"toolCallId\":\"tool_1\",\"status\":\"completed\",\"content\":\"README.md\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_123\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp session script");
        make_executable(&script);
        let descriptor = AgentPluginDescriptor {
            agent_id: "mock-acp-session".to_string(),
            display_name: "Mock ACP Session".to_string(),
            version: "0.3.0".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: script.display().to_string(),
                args: vec![],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec![],
            config_schema_version: 1,
        };

        let mut approver = |_prompt| ApprovalResponse::allow_once(None);
        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "build a plan",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .unwrap();

        assert_eq!(evidence.session_id, "session_123");
        assert_eq!(evidence.final_status, "completed");
        assert_eq!(evidence.message, "Planning");
        assert_eq!(
            evidence.tool_calls,
            vec!["tool_1:pending:Read files", "tool_1:completed:README.md"]
        );
        let log = fs::read_to_string(&evidence.log_path).expect("read acp session log");
        assert!(log.contains("session/new"));
        assert!(log.contains("session/prompt"));
        assert!(log.contains("TurnEnd"));
    }

    #[test]
    fn acp_session_prompt_can_load_and_configure_existing_session() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_load_configure");
        let script = root.join("mock-acp-load-configure.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true,\"sessionCapabilities\":{\"setMode\":true}},\"agentInfo\":{\"name\":\"mock-load-configure\",\"version\":\"0.7.0\"}}}'",
                "read load",
                "case \"$load\" in *'\"method\":\"session/load\"'*'\"sessionId\":\"session_existing\"'*) ;; *) echo \"$load\" >&2; exit 5 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_existing\"}}'",
                "read mode",
                "case \"$mode\" in *'\"method\":\"session/set_mode\"'*'\"modeId\":\"plan\"'*) ;; *) echo \"$mode\" >&2; exit 6 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{}}'",
                "read model",
                "case \"$model\" in *'\"method\":\"session/set_config_option\"'*'\"configId\":\"model\"'*'\"value\":\"claude-sonnet\"'*) ;; *) echo \"$model\" >&2; exit 7 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"configOptions\":[]}}'",
                "read prompt",
                "case \"$prompt\" in *'\"method\":\"session/prompt\"'*) ;; *) echo \"$prompt\" >&2; exit 8 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_existing\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"configured\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{\"stopReason\":\"end_turn\"}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp load configure script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-load-configure", &script);
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);
        let session = AcpSessionOptions {
            load_session_id: Some("session_existing".to_string()),
            mode_id: Some("plan".to_string()),
            model_id: Some("claude-sonnet".to_string()),
        };

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "continue",
            session,
            &mut approver,
        )
        .expect("configured acp session succeeds");

        assert_eq!(evidence.session_id, "session_existing");
        assert_eq!(evidence.final_status, "end_turn");
        assert_eq!(evidence.message, "configured");
        let log = fs::read_to_string(&evidence.log_path).expect("read acp session log");
        assert!(log.contains("session/load"));
        assert!(log.contains("session/set_mode"));
        assert!(log.contains("session/set_config_option"));
        assert!(log.contains("session/prompt"));
    }

    #[test]
    fn acp_session_prompt_fails_when_set_mode_is_rejected() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_set_mode_error");
        let script = root.join("mock-acp-set-mode-error.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"sessionCapabilities\":{\"setMode\":true}},\"agentInfo\":{\"name\":\"mock-set-mode-error\",\"version\":\"0.7.0\"}}}'",
                "read _new",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_123\"}}'",
                "read mode",
                "case \"$mode\" in *'\"method\":\"session/set_mode\"'*'\"modeId\":\"plan\"'*) ;; *) echo \"$mode\" >&2; exit 5 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000,\"message\":\"mode unavailable\"}}'",
                "read maybe_prompt || exit 0",
                "echo \"unexpected prompt: $maybe_prompt\" >&2",
                "exit 9",
            ]
            .join("\n"),
        )
        .expect("write mock acp set-mode error script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-set-mode-error", &script);
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);

        let err = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "continue",
            AcpSessionOptions {
                load_session_id: None,
                mode_id: Some("plan".to_string()),
                model_id: None,
            },
            &mut approver,
        )
        .expect_err("set_mode errors should stop before prompting");

        assert!(err.contains("mode unavailable"), "{err}");
    }

    #[test]
    fn acp_run_can_use_custom_command_descriptor_from_env() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_custom_command_run");
        let script = root.join("mock-custom-acp.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentInfo\":{\"name\":\"custom-acp\",\"version\":\"0.1.0\"}}}'",
                "read _new",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_custom\"}}'",
                "read prompt",
                "case \"$prompt\" in *'\"method\":\"session/prompt\"'*'hello custom'*) ;; *) echo \"$prompt\" >&2; exit 5 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_custom\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"custom ok\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_custom\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock custom acp script");
        make_executable(&script);
        let descriptor = custom_acp_agent_descriptor(&script.display().to_string());
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);

        let output = handle_acp_agent_run_command_with_agents(
            &root,
            &[descriptor],
            AcpRunArgs {
                async_job: false,
                agent_id: "custom-acp".to_string(),
                task: "hello custom".to_string(),
                session: AcpSessionOptions::default(),
            },
            &mut approver,
            PermissionContext::default(),
            None,
        )
        .expect("custom ACP descriptor should run");

        assert!(output.contains("agent: custom-acp"));
        assert!(output.contains("message: custom ok"));
    }

    #[test]
    fn acp_session_prompt_accepts_codex_style_final_response() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_prompt_codex_style");
        let script = root.join("mock-acp-codex-style.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true},\"agentInfo\":{\"name\":\"mock-codex-style\",\"version\":\"0.1.0\"}}}'",
                "read new_session",
                "case \"$new_session\" in *'\"mcpServers\":[]'*) ;; *) echo \"$new_session\" >&2; exit 3 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_codex\"}}'",
                "read prompt",
                "case \"$prompt\" in *'\"prompt\"'*'Reply'*) ;; *) echo \"$prompt\" >&2; exit 4 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_codex\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"OK\"}}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"stopReason\":\"end_turn\",\"usage\":{\"totalTokens\":9,\"inputTokens\":7,\"outputTokens\":2}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock codex-style acp script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-codex-style", &script);
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "Reply",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .unwrap();

        assert_eq!(evidence.session_id, "session_codex");
        assert_eq!(evidence.final_status, "end_turn");
        assert_eq!(evidence.message, "OK");
        assert_eq!(
            evidence.usage_summary.as_deref(),
            Some("total=9 input=7 output=2")
        );
        assert_eq!(acp_session_job_status(&evidence), "finished");
    }

    #[test]
    fn acp_session_prompt_accepts_kiro_notifications_and_tool_calls() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_prompt_kiro_style");
        let script = root.join("mock-acp-kiro-style.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true,\"promptCapabilities\":{\"image\":true}},\"agentInfo\":{\"name\":\"kiro-cli\",\"version\":\"1.5.0\"}}}'",
                "read new_session",
                "case \"$new_session\" in *'\"mcpServers\":[]'*) ;; *) echo \"$new_session\" >&2; exit 3 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_kiro\"}}'",
                "read prompt",
                "case \"$prompt\" in *'\"prompt\"'*'Explain'*) ;; *) echo \"$prompt\" >&2; exit 4 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/notification\",\"params\":{\"sessionId\":\"session_kiro\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"Working\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/notification\",\"params\":{\"sessionId\":\"session_kiro\",\"update\":{\"type\":\"ToolCall\",\"toolCallId\":\"tool_1\",\"title\":\"Inspect project\",\"status\":\"pending\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/notification\",\"params\":{\"sessionId\":\"session_kiro\",\"update\":{\"type\":\"ToolCallUpdate\",\"toolCallId\":\"tool_1\",\"status\":\"completed\",\"content\":\"Cargo.toml\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/notification\",\"params\":{\"sessionId\":\"session_kiro\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock kiro-style acp script");
        make_executable(&script);
        let mut descriptor = mock_acp_descriptor("kiro-cli", &script);
        descriptor.source = AgentSource::LocalCommand;
        descriptor.command.args = vec![];
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "Explain",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .unwrap();

        assert_eq!(evidence.session_id, "session_kiro");
        assert_eq!(evidence.final_status, "completed");
        assert_eq!(evidence.message, "Working");
        assert_eq!(
            evidence.tool_calls,
            vec![
                "tool_1:pending:Inspect project",
                "tool_1:completed:Cargo.toml"
            ]
        );
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. } if content == "Working"
        )));
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::ToolCallStarted { tool_call_id, name, .. }
                if tool_call_id == "tool_1" && name == "Inspect project"
        )));
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished { tool_call_id, success, evidence, .. }
                if tool_call_id == "tool_1" && *success && evidence.is_some()
        )));
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.kind == "tool_log" && evidence.summary.contains("Cargo.toml")
        )));
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.kind == "acp_turn_end" && evidence.summary.contains("completed")
        )));
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-acp-session-session_kiro"
                    && gate.status == MergeGateStatus::CollectingEvidence
                    && gate.decision.as_deref() == Some("missing_canonical")
                    && gate.required_evidence == vec!["acp_turn_end".to_string()]
                    && gate.evidence_ids.iter().any(|id| id.starts_with("acp-tool-tool_1"))
                    && gate.evidence_ids.iter().any(|id| id.starts_with("acp-turn-end-session_kiro"))
        )));
    }

    #[test]
    fn acp_session_prompt_maps_diff_updates_to_patch_evidence() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_prompt_patch_evidence");
        let script = root.join("mock-acp-patch-evidence.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"loadSession\":true},\"agentInfo\":{\"name\":\"mock-acp-patch\",\"version\":\"0.1.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_patch\"}}'",
                "read _prompt",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_patch\",\"update\":{\"type\":\"ToolCallUpdate\",\"toolCallId\":\"tool_patch\",\"status\":\"completed\",\"content\":\"generated patch\",\"diff\":\"diff --git a/src/lib.rs b/src/lib.rs\\n--- a/src/lib.rs\\n+++ b/src/lib.rs\\n@@ -1 +1 @@\\n-old\\n+new\\n\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_patch\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp patch script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-patch", &script);
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "Generate a patch",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .unwrap();

        let patch_id = evidence
            .runtime_events
            .iter()
            .find_map(|event| match &event.kind {
                RuntimeEventKind::EvidenceRecorded { evidence } if evidence.kind == "patch" => {
                    let metadata = evidence.metadata.as_ref()?;
                    assert_eq!(
                        metadata.get("schema").and_then(Value::as_str),
                        Some("acp.patch.v1")
                    );
                    assert_eq!(
                        metadata.get("format").and_then(Value::as_str),
                        Some("unified_diff")
                    );
                    assert_eq!(metadata.get("fileCount").and_then(Value::as_u64), Some(1));
                    assert_eq!(metadata.get("additions").and_then(Value::as_u64), Some(1));
                    assert_eq!(metadata.get("deletions").and_then(Value::as_u64), Some(1));
                    assert_eq!(
                        metadata.pointer("/files/0/path").and_then(Value::as_str),
                        Some("src/lib.rs")
                    );
                    assert_eq!(
                        metadata
                            .pointer("/origin/toolCallId")
                            .and_then(Value::as_str),
                        Some("tool_patch")
                    );
                    assert!(
                        metadata.get("diff").and_then(Value::as_str).is_some_and(
                            |diff| diff.contains("diff --git a/src/lib.rs b/src/lib.rs")
                        )
                    );
                    assert_eq!(evidence.source.as_deref(), Some("acp:patch.v1"));
                    assert!(evidence.summary.contains("ACP patch: 1 file(s), +1/-1"));
                    Some(evidence.id.clone())
                }
                _ => None,
            });
        let patch_id = patch_id.expect("ACP diff update should record patch evidence");
        assert!(evidence.runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-acp-session-session_patch"
                    && gate.required_evidence == vec![
                        "patch".to_string(),
                        "acp_turn_end".to_string(),
                    ]
                    && gate.evidence_ids.iter().any(|id| id == &patch_id)
                    && gate.status == MergeGateStatus::CollectingEvidence
                    && gate.decision.as_deref() == Some("missing_canonical")
        )));
    }

    #[test]
    fn acp_smoke_gate_reports_pass_and_blocked_auth() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_smoke_gate");
        let ok = root.join("mock-acp-ok.sh");
        fs::write(
            &ok,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-ok\",\"version\":\"0.1.0\"},\"authMethods\":[]}}'",
            ]
            .join("\n"),
        )
        .expect("write ok smoke script");
        make_executable(&ok);
        let blocked = root.join("mock-acp-blocked.sh");
        fs::write(
            &blocked,
            [
                "#!/bin/sh",
                "echo 'error: You are not logged in, please log in' >&2",
                "exit 3",
            ]
            .join("\n"),
        )
        .expect("write blocked smoke script");
        make_executable(&blocked);
        let agents = vec![
            mock_acp_descriptor("mock-ok", &ok),
            mock_acp_descriptor("mock-blocked", &blocked),
        ];
        let mut approver = |_prompt| ApprovalResponse::allow_once(None);

        let report =
            run_acp_smoke_gate_for_agents(&root, &agents, false, &mut approver).unwrap_err();

        assert!(report.contains("PASS mock-ok"));
        assert!(report.contains("BLOCKED mock-blocked"));
        assert!(report.contains("summary: 0 failed, 1 blocked-auth"));
    }

    #[test]
    fn acp_smoke_gate_classifies_timeout_as_failure_not_auth_block() {
        let descriptor = mock_acp_descriptor("kiro-cli", Path::new("kiro-cli"));

        assert_eq!(
            classify_acp_smoke_error(
                &descriptor,
                "ACP session/prompt timed out before TurnEnd or final response after 120s",
            ),
            "timeout"
        );
        assert_eq!(
            classify_acp_smoke_error(&descriptor, "You are not logged in, please log in"),
            "blocked-auth"
        );
    }

    #[test]
    fn acp_session_prompt_timeout_is_agent_aware_and_env_overridable() {
        let _guard = subprocess_test_guard();
        let mut kiro = mock_acp_descriptor("kiro-cli", Path::new("kiro-cli"));
        kiro.command.command = "kiro-cli".to_string();
        let codex = mock_acp_descriptor("codex-acp", Path::new("npx"));

        unsafe {
            env::remove_var("VIDEN_ACP_SESSION_TIMEOUT_SECS");
        }
        assert_eq!(
            acp_session_prompt_timeout(&kiro),
            Duration::from_secs(DEFAULT_KIRO_ACP_SESSION_TIMEOUT_SECS)
        );
        assert_eq!(
            acp_session_prompt_timeout(&codex),
            Duration::from_secs(DEFAULT_LOCAL_ACP_SESSION_TIMEOUT_SECS)
        );

        unsafe {
            env::set_var("VIDEN_ACP_SESSION_TIMEOUT_SECS", "7");
        }
        assert_eq!(acp_session_prompt_timeout(&kiro), Duration::from_secs(7));
        unsafe {
            env::remove_var("VIDEN_ACP_SESSION_TIMEOUT_SECS");
        }
    }

    #[test]
    fn acp_session_permission_request_routes_through_approver() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_permission");
        let script = root.join("mock-acp-permission.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-permission-acp\",\"version\":\"0.4.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_perm\"}}'",
                "read _prompt",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"session/request_permission\",\"params\":{\"sessionId\":\"session_perm\",\"toolCall\":{\"toolCallId\":\"tool_2\",\"title\":\"Edit file\",\"kind\":\"edit\"},\"options\":[{\"optionId\":\"deny\",\"kind\":\"reject_once\",\"name\":\"Deny\"},{\"optionId\":\"allow\",\"kind\":\"allow_once\",\"name\":\"Allow\"}]}}'",
                "read approval",
                "case \"$approval\" in *'\"id\":9'*'\"optionId\":\"allow\"'*) ;; *) exit 5 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_perm\",\"update\":{\"type\":\"ToolCallUpdate\",\"toolCallId\":\"tool_2\",\"status\":\"completed\",\"content\":\"approved\"}}}'",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_perm\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp permission script");
        make_executable(&script);
        let descriptor = AgentPluginDescriptor {
            agent_id: "mock-acp-permission".to_string(),
            display_name: "Mock ACP Permission".to_string(),
            version: "0.4.0".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: script.display().to_string(),
                args: vec![],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec![],
            config_schema_version: 1,
        };
        let approvals = Cell::new(0usize);
        let prompts = RefCell::new(Vec::new());
        let mut approver = |prompt: viden_types::PermissionPrompt| {
            approvals.set(approvals.get() + 1);
            prompts.borrow_mut().push(prompt);
            ApprovalResponse::allow_once(Some("ok".to_string()))
        };

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "edit the file",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .unwrap();

        assert_eq!(approvals.get(), 1);
        assert_eq!(prompts.borrow()[0].tool_name, "acp:tool_2");
        assert!(prompts.borrow()[0].message.contains("Edit file"));
        assert_eq!(evidence.tool_calls, vec!["tool_2:completed:approved"]);
        let log = fs::read_to_string(&evidence.log_path).expect("read acp session log");
        assert!(log.contains("session/request_permission"));
        assert!(log.contains(r#"optionId\":\"allow"#));
    }

    #[test]
    fn first_party_acp_permission_and_tool_update_fixtures_project_consistently() {
        let fixtures = [
            include_str!("tests/fixtures/acp-v1/claude-acp.json"),
            include_str!("tests/fixtures/acp-v1/codex-acp.json"),
            include_str!("tests/fixtures/acp-v1/kiro-acp.json"),
        ];
        let known = builtin_agent_descriptors()
            .into_iter()
            .map(|descriptor| descriptor.agent_id)
            .collect::<HashSet<_>>();

        for raw in fixtures {
            let fixture: Value = serde_json::from_str(raw).expect("valid ACP fixture");
            let agent_id = fixture["agent_id"].as_str().expect("fixture agent id");
            assert!(
                known.contains(agent_id),
                "unknown fixture adapter {agent_id}"
            );
            let permission = &fixture["permission_request"];
            let prompt = acp_permission_prompt(permission);
            assert!(prompt.tool_name.starts_with("acp:"), "{agent_id}");
            assert!(!prompt.message.trim().is_empty(), "{agent_id}");
            let allow: Value = serde_json::from_str(&acp_permission_response(permission, true))
                .expect("allow response");
            let deny: Value = serde_json::from_str(&acp_permission_response(permission, false))
                .expect("deny response");
            assert_ne!(
                allow.pointer("/result/outcome/optionId"),
                deny.pointer("/result/outcome/optionId"),
                "{agent_id} must map distinct allow and deny options"
            );

            let mut events = Vec::new();
            let mut sequence = 1;
            let mut evidence_ids = Vec::new();
            append_acp_update_runtime_events(
                &mut events,
                &mut sequence,
                agent_id,
                &mut evidence_ids,
                &fixture["tool_update"],
            );
            assert!(events.iter().any(|event| matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished { success: true, .. }
            )));
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event.kind, RuntimeEventKind::EvidenceRecorded { .. }))
            );
        }
    }

    #[test]
    fn acp_session_permission_denial_selects_reject_option() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_permission_denied");
        let script = root.join("mock-acp-permission-denied.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-deny-acp\",\"version\":\"0.4.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_deny\"}}'",
                "read _prompt",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"session/request_permission\",\"params\":{\"sessionId\":\"session_deny\",\"toolCall\":{\"toolCallId\":\"tool_3\",\"title\":\"Run command\",\"kind\":\"terminal\"},\"options\":[{\"optionId\":\"approve\",\"kind\":\"allow_once\",\"name\":\"Allow\"},{\"optionId\":\"reject\",\"kind\":\"reject_once\",\"name\":\"Reject\"}]}}'",
                "read approval",
                "case \"$approval\" in *'\"id\":10'*'\"optionId\":\"reject\"'*) ;; *) exit 5 ;; esac",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_deny\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp permission denied script");
        make_executable(&script);
        let descriptor = AgentPluginDescriptor {
            agent_id: "mock-acp-deny".to_string(),
            display_name: "Mock ACP Deny".to_string(),
            version: "0.4.0".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: script.display().to_string(),
                args: vec![],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![AgentPluginCapability::SessionPrompt],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec![],
            config_schema_version: 1,
        };
        let mut approver =
            |_prompt: viden_types::PermissionPrompt| ApprovalResponse::deny(Some("no".to_string()));

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "run a command",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .unwrap();

        assert_eq!(evidence.final_status, "completed");
        let log = fs::read_to_string(&evidence.log_path).expect("read acp session log");
        assert!(log.contains(r#"optionId\":\"reject"#));
    }

    #[test]
    fn acp_session_handles_permission_gated_file_read_requests() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_file_request_read");
        let target = root.join("notes.txt");
        fs::write(&target, "hello from acp\nsecond line\n").expect("write target file");
        let script = root.join("mock-acp-file-request.sh");
        fs::write(
            &script,
            vec![
                "#!/bin/sh".to_string(),
                "read _init".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-file-acp\",\"version\":\"0.5.0\"}}}'".to_string(),
                "read _new_session".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_file\"}}'".to_string(),
                "read _prompt".to_string(),
                format!(
                    "printf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":17,\"method\":\"fs/read_text_file\",\"params\":{{\"path\":\"{}\",\"startLine\":1,\"limit\":1}}}}'",
                    target.display()
                ),
                "read file_response".to_string(),
                "case \"$file_response\" in".to_string(),
                "  *'\"content\":\"hello from acp\"'*) ;;".to_string(),
                "  *) echo \"$file_response\" >&2; exit 3 ;;".to_string(),
                "esac".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_file\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'".to_string(),
            ]
            .join("\n"),
        )
        .expect("write mock acp file request script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-file-request", &script);
        let mut approver =
            |_prompt: viden_types::PermissionPrompt| ApprovalResponse::allow_once(None);

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "read a file",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .expect("session should continue after file read request");

        assert_eq!(evidence.final_status, "completed");
        let log = fs::read_to_string(&evidence.log_path).expect("read acp session log");
        assert!(log.contains("fs/read_text_file"));
        assert!(log.contains("hello from acp"));
    }

    #[test]
    fn acp_session_file_write_requests_require_approval() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_file_request_write");
        let target = root.join("written.txt");
        let script = root.join("mock-acp-file-write.sh");
        fs::write(
            &script,
            vec![
                "#!/bin/sh".to_string(),
                "read _init".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-file-write-acp\",\"version\":\"0.5.0\"}}}'".to_string(),
                "read _new_session".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_file_write\"}}'".to_string(),
                "read _prompt".to_string(),
                format!(
                    "printf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":18,\"method\":\"fs/write_text_file\",\"params\":{{\"path\":\"{}\",\"content\":\"written by acp\"}}}}'",
                    target.display()
                ),
                "read file_response".to_string(),
                "case \"$file_response\" in".to_string(),
                "  *'\"result\":{}'*) ;;".to_string(),
                "  *) echo \"$file_response\" >&2; exit 3 ;;".to_string(),
                "esac".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_file_write\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'".to_string(),
            ]
            .join("\n"),
        )
        .expect("write mock acp file write script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-file-write", &script);
        let approvals = Cell::new(0usize);
        let mut approver = |prompt: viden_types::PermissionPrompt| {
            approvals.set(approvals.get() + 1);
            assert_eq!(prompt.tool_name, "write_file");
            assert!(prompt.input_preview.contains("written.txt"));
            ApprovalResponse::allow_once(None)
        };

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "write a file",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .expect("session should continue after approved file write request");

        assert_eq!(approvals.get(), 1);
        assert_eq!(evidence.final_status, "completed");
        assert_eq!(fs::read_to_string(&target).unwrap(), "written by acp");
    }

    #[test]
    fn acp_filesystem_bridge_denies_out_of_scope_reads() {
        let root = temp_root("acp_file_out_of_scope");
        let engine = PermissionEngine::new(&root);
        let response = acp_read_text_file_response(
            &root,
            &engine,
            &json!({
                "jsonrpc": "2.0",
                "id": 19,
                "method": "fs/read_text_file",
                "params": {"path": "/tmp/outside-viden.txt"}
            }),
        );

        assert!(response.contains(r#""id":19"#));
        assert!(response.contains("Path is outside the allowed working directory scope"));
    }

    #[test]
    fn acp_session_terminal_requests_run_through_permission_bridge() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_session_terminal_request");
        let script = root.join("mock-acp-terminal.sh");
        fs::write(
            &script,
            vec![
                "#!/bin/sh".to_string(),
                "read _init".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-terminal-acp\",\"version\":\"0.6.0\"}}}'".to_string(),
                "read _new_session".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_terminal\"}}'".to_string(),
                "read _prompt".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"terminal/create\",\"params\":{\"sessionId\":\"session_terminal\",\"command\":\"printf\",\"args\":[\"terminal-ok\"]}}'".to_string(),
                "read create_response".to_string(),
                "case \"$create_response\" in".to_string(),
                "  *'\"terminalId\":\"acp-terminal-1\"'*) ;;".to_string(),
                "  *) echo \"$create_response\" >&2; exit 3 ;;".to_string(),
                "esac".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":43,\"method\":\"terminal/output\",\"params\":{\"sessionId\":\"session_terminal\",\"terminalId\":\"acp-terminal-1\"}}'".to_string(),
                "read output_response".to_string(),
                "case \"$output_response\" in".to_string(),
                "  *'terminal-ok'*) ;;".to_string(),
                "  *) echo \"$output_response\" >&2; exit 4 ;;".to_string(),
                "esac".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":44,\"method\":\"terminal/wait_for_exit\",\"params\":{\"sessionId\":\"session_terminal\",\"terminalId\":\"acp-terminal-1\"}}'".to_string(),
                "read wait_response".to_string(),
                "case \"$wait_response\" in".to_string(),
                "  *'\"exitCode\":0'*) ;;".to_string(),
                "  *) echo \"$wait_response\" >&2; exit 5 ;;".to_string(),
                "esac".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":45,\"method\":\"terminal/release\",\"params\":{\"sessionId\":\"session_terminal\",\"terminalId\":\"acp-terminal-1\"}}'".to_string(),
                "read release_response".to_string(),
                "case \"$release_response\" in".to_string(),
                "  *'\"result\":{}'*) ;;".to_string(),
                "  *) echo \"$release_response\" >&2; exit 6 ;;".to_string(),
                "esac".to_string(),
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_terminal\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'".to_string(),
            ]
            .join("\n"),
        )
        .expect("write mock acp terminal script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-terminal", &script);
        let approvals = Cell::new(0usize);
        let mut approver = |prompt: viden_types::PermissionPrompt| {
            approvals.set(approvals.get() + 1);
            assert_eq!(prompt.tool_name, "shell");
            assert!(prompt.input_preview.contains("printf terminal-ok"));
            ApprovalResponse::allow_once(None)
        };

        let evidence = run_acp_session_prompt_for_agent(
            &root,
            &descriptor,
            "run a terminal command",
            AcpSessionOptions::default(),
            &mut approver,
        )
        .expect("session should continue after terminal requests");

        assert_eq!(approvals.get(), 1);
        assert_eq!(evidence.final_status, "completed");
        let log = fs::read_to_string(&evidence.log_path).expect("read acp session log");
        assert!(log.contains("terminal/create"));
        assert!(log.contains("terminal-ok"));
    }

    #[test]
    fn acp_terminal_bridge_supports_long_running_output_polling() {
        let root = temp_root("acp_terminal_long_running");
        let mut engine = PermissionEngine::new(&root);
        let mut terminals = AcpTerminalStore::default();
        let mut approver =
            |_prompt: viden_types::PermissionPrompt| ApprovalResponse::allow_once(None);

        let started = Instant::now();
        let create = acp_terminal_create_response(
            &root,
            &mut engine,
            &mut approver,
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 47,
                "method": "terminal/create",
                "params": {
                    "sessionId": "session_terminal",
                    "command": "sh",
                    "args": ["-c", "printf started; sleep 1; printf done"]
                }
            }),
        );
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "terminal/create should return before the command exits"
        );
        assert!(create.contains(r#""terminalId":"acp-terminal-1""#));

        std::thread::sleep(Duration::from_millis(100));
        let output = acp_terminal_output_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 48,
                "method": "terminal/output",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(output.contains("started"));
        assert!(output.contains(r#""exitCode":null"#));

        let wait = acp_terminal_wait_for_exit_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 49,
                "method": "terminal/wait_for_exit",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(wait.contains(r#""exitCode":0"#));

        let final_output = acp_terminal_output_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 50,
                "method": "terminal/output",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(final_output.contains("started"));
        assert!(final_output.contains("done"));
    }

    #[test]
    fn acp_terminal_bridge_can_kill_long_running_processes() {
        let root = temp_root("acp_terminal_kill_long_running");
        let mut engine = PermissionEngine::new(&root);
        let mut terminals = AcpTerminalStore::default();
        let mut approver =
            |_prompt: viden_types::PermissionPrompt| ApprovalResponse::allow_once(None);

        let create = acp_terminal_create_response(
            &root,
            &mut engine,
            &mut approver,
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 51,
                "method": "terminal/create",
                "params": {
                    "sessionId": "session_terminal",
                    "command": "sh",
                    "args": ["-c", "printf started; sleep 5; printf never"]
                }
            }),
        );
        assert!(create.contains(r#""terminalId":"acp-terminal-1""#));

        std::thread::sleep(Duration::from_millis(100));
        let kill = acp_terminal_kill_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 52,
                "method": "terminal/kill",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(kill.contains(r#""result":{}"#));

        let wait = acp_terminal_wait_for_exit_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 53,
                "method": "terminal/wait_for_exit",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(wait.contains(r#""signal":"killed""#));

        let output = acp_terminal_output_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 54,
                "method": "terminal/output",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(output.contains("started"));
        assert!(!output.contains("never"));
    }

    #[test]
    fn acp_terminal_bridge_supports_stdin_input() {
        let root = temp_root("acp_terminal_stdin_input");
        let mut engine = PermissionEngine::new(&root);
        let mut terminals = AcpTerminalStore::default();
        let mut approver =
            |_prompt: viden_types::PermissionPrompt| ApprovalResponse::allow_once(None);

        let create = acp_terminal_create_response(
            &root,
            &mut engine,
            &mut approver,
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 55,
                "method": "terminal/create",
                "params": {
                    "sessionId": "session_terminal",
                    "command": "sh",
                    "args": ["-c", "read line; printf 'got:%s' \"$line\""]
                }
            }),
        );
        assert!(create.contains(r#""terminalId":"acp-terminal-1""#));

        let input = acp_terminal_input_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 56,
                "method": "terminal/input",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1",
                    "input": "hello\n"
                }
            }),
        );
        assert!(input.contains(r#""bytesWritten":6"#));

        let wait = acp_terminal_wait_for_exit_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 57,
                "method": "terminal/wait_for_exit",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(wait.contains(r#""exitCode":0"#));

        let output = acp_terminal_output_response(
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 58,
                "method": "terminal/output",
                "params": {
                    "sessionId": "session_terminal",
                    "terminalId": "acp-terminal-1"
                }
            }),
        );
        assert!(output.contains("got:hello"));
    }

    #[test]
    fn acp_terminal_bridge_respects_plan_mode() {
        let root = temp_root("acp_terminal_plan_mode");
        let mut engine = PermissionEngine::new(&root);
        let context = PermissionContext {
            mode: viden_types::PermissionMode::Plan,
            ..Default::default()
        };
        engine.restore_context(context);
        let mut terminals = AcpTerminalStore::default();
        let approvals = Cell::new(0usize);
        let mut approver = |_prompt: viden_types::PermissionPrompt| {
            approvals.set(approvals.get() + 1);
            ApprovalResponse::allow_once(None)
        };

        let response = acp_terminal_create_response(
            &root,
            &mut engine,
            &mut approver,
            &mut terminals,
            &json!({
                "jsonrpc": "2.0",
                "id": 46,
                "method": "terminal/create",
                "params": {
                    "sessionId": "session_terminal",
                    "command": "printf",
                    "args": ["blocked"]
                }
            }),
        );

        assert_eq!(approvals.get(), 0);
        assert!(terminals.records.is_empty());
        assert!(response.contains(r#""id":46"#));
        assert!(response.contains("blocked while plan mode is active"));
    }

    #[test]
    fn acp_agent_handshake_timeout_allows_registry_cold_start() {
        let mut registry = mock_acp_descriptor("mock-registry-acp", Path::new("npx"));
        registry.source = AgentSource::Registry;
        let mut local = mock_acp_descriptor("mock-local-acp", Path::new("kiro-cli"));
        local.source = AgentSource::LocalCommand;

        assert_eq!(
            acp_agent_handshake_timeout(&registry),
            Duration::from_secs(DEFAULT_REGISTRY_ACP_HANDSHAKE_TIMEOUT_SECS)
        );
        assert_eq!(
            acp_agent_handshake_timeout(&local),
            Duration::from_secs(DEFAULT_LOCAL_ACP_HANDSHAKE_TIMEOUT_SECS)
        );
    }

    #[test]
    fn acp_registry_agent_uses_project_scoped_npm_cache() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_registry_npm_cache");
        let script = root.join("mock-registry-acp.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "case \"$npm_config_cache\" in",
                "  */.viden/cache/npm) ;;",
                "  *) echo \"unexpected npm_config_cache=$npm_config_cache\" >&2; exit 7 ;;",
                "esac",
                "test \"$NPM_CONFIG_CACHE\" = \"$npm_config_cache\" || exit 8",
                "test \"$npm_config_audit\" = \"false\" || exit 9",
                "test \"$npm_config_fund\" = \"false\" || exit 10",
                "read _line",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-registry-cache\",\"version\":\"0.1.0\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock registry acp script");
        make_executable(&script);
        let mut descriptor = mock_acp_descriptor("mock-registry-cache", &script);
        descriptor.source = AgentSource::Registry;

        let evidence = run_acp_initialize_probe_for_agent(&root, &descriptor)
            .expect("registry probe succeeds");

        assert_eq!(evidence.agent_label, "mock-registry-cache 0.1.0");
        assert!(root.join(".viden/cache/npm").is_dir());
    }

    #[test]
    fn acp_initialize_probe_records_stderr_on_agent_exit() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_probe_stderr");
        let script = root.join("mock-acp-stderr.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "echo 'Auth: Not authenticated. Please run login' >&2",
                "exit 3",
            ]
            .join("\n"),
        )
        .expect("write mock acp stderr script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-stderr", &script);

        let error = run_acp_initialize_probe_for_agent(&root, &descriptor)
            .expect_err("probe should fail with stderr");

        assert!(error.contains("ACP command closed stdout without response"));
        assert!(error.contains("Auth: Not authenticated"));
        let log_path = error
            .split("log ")
            .last()
            .expect("log path in error")
            .trim();
        let log = fs::read_to_string(log_path).expect("read probe log");
        assert!(log.contains(r#""direction":"stderr"#));
        assert!(log.contains("Auth: Not authenticated"));
    }

    #[cfg(unix)]
    #[test]
    fn acp_child_cleanup_timeout_is_bounded() {
        let _guard = subprocess_test_guard();
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("trap '' TERM; sleep 5")
            .spawn()
            .expect("spawn sleep child");

        let exited = wait_child_timeout(&mut child, Duration::from_millis(100));

        assert!(!exited);
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn acp_async_job_records_status_and_result() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_async_job");
        let script = root.join("mock-acp-async.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-async-acp\",\"version\":\"0.5.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_async\"}}'",
                "read _prompt",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_async\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"async done\"}}}'",
                "sleep 3",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_async\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp async script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-async", &script);

        let started = start_acp_session_job(
            &root,
            &descriptor,
            "finish quickly".to_string(),
            AcpSessionOptions::default(),
            None,
        )
        .expect("start acp job");
        let id = started
            .lines()
            .find_map(|line| line.split('`').nth(1))
            .expect("job id in output")
            .to_string();
        let runtime_events_path = acp_job_runtime_events_path(&root, &id);

        let live_deadline = Instant::now() + Duration::from_secs(2);
        let mut saw_live_event_while_running = false;
        while Instant::now() < live_deadline {
            let running = find_codex_job(&root, &id)
                .ok()
                .flatten()
                .is_some_and(|job| job.status == "running");
            let runtime_events = read_acp_runtime_events(&runtime_events_path);
            let has_assistant_event = runtime_events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::AssistantDelta { content, .. } if content == "async done"
                )
            });
            let has_turn_end_evidence = runtime_events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::EvidenceRecorded { evidence } if evidence.kind == "acp_turn_end"
                )
            });
            if running && has_assistant_event {
                assert!(!has_turn_end_evidence);
                saw_live_event_while_running = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            saw_live_event_while_running,
            "async ACP job should persist assistant runtime events while still running"
        );
        let live_runtime_events =
            fs::read_to_string(&runtime_events_path).expect("read live ACP runtime events");
        assert!(live_runtime_events.contains("async done"));
        let parsed_live_runtime_events = read_acp_runtime_events(&runtime_events_path);
        assert!(!parsed_live_runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence } if evidence.kind == "acp_turn_end"
        )));

        wait_until(
            || {
                find_codex_job(&root, &id)
                    .ok()
                    .flatten()
                    .is_some_and(|job| job.status == "finished")
            },
            Duration::from_secs(10),
        );

        let status = render_codex_job_status(&root).expect("render job status");
        let result = render_codex_job_result(&root, Some(&id)).expect("render job result");
        assert!(status.contains("acp-session"));
        assert!(status.contains("finished"));
        assert!(status.contains("session: session_async"));
        assert!(!status.contains("codex resume session_async"));
        assert!(result.contains("session_async"));
        assert!(result.contains("session: session_async"));
        assert!(!result.contains("codex resume session_async"));
        assert!(result.contains("async done"));

        let runtime_events_log =
            fs::read_to_string(&runtime_events_path).expect("read ACP runtime events");
        assert!(runtime_events_log.contains("assistant_delta"));
        assert!(runtime_events_log.contains("acp_turn_end"));
        let runtime_events = tracked_agent_job_runtime_events(&root);
        assert!(runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. } if content == "async done"
        )));
        assert!(runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.kind == "acp_turn_end"
                    && evidence.summary.contains("completed")
        )));
        assert!(runtime_events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-acp-session-session_async"
                    && gate.status == MergeGateStatus::CollectingEvidence
                    && gate.decision.as_deref() == Some("missing_canonical")
                    && gate.evidence_ids.iter().any(|id| id.starts_with("acp-turn-end-session_async"))
        )));
    }

    #[test]
    fn acp_async_job_pushes_runtime_events_to_live_sink() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_async_live_sink");
        let script = root.join("mock-acp-live-sink.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-live-acp\",\"version\":\"0.5.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_live\"}}'",
                "read _prompt",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_live\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"live sink delta\"}}}'",
                "sleep 2",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_live\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
            ]
            .join("\n"),
        )
        .expect("write mock acp live sink script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-live-sink", &script);
        let (sender, receiver) = mpsc::channel();
        let live_sink: RuntimeEventSink = Arc::new(move |events| {
            for event in events {
                let _ = sender.send(event);
            }
        });

        let started = start_acp_session_job(
            &root,
            &descriptor,
            "stream while running".to_string(),
            AcpSessionOptions::default(),
            Some(live_sink),
        )
        .expect("start acp job");
        let id = started
            .lines()
            .find_map(|line| line.split('`').nth(1))
            .expect("job id in output")
            .to_string();

        let live_events = wait_for_channel_events(&receiver, Duration::from_secs(10), |events| {
            let has_proposed_gate = events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                        if gate.gate_id == "gate-acp-session-session_live"
                            && gate.status == MergeGateStatus::Proposed
                )
            });
            let has_assistant_delta = events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::AssistantDelta { content, .. } if content == "live sink delta"
                )
            });
            has_proposed_gate && has_assistant_delta
        });
        assert!(live_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-acp-session-session_live"
                        && gate.status == MergeGateStatus::Proposed
            )
        }));
        let job = find_codex_job(&root, &id)
            .expect("find job")
            .expect("job exists");
        assert_eq!(job.status, "running");
    }

    #[test]
    fn acp_async_job_can_be_cancelled_by_pid() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_async_cancel");
        let script = root.join("mock-acp-cancel.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "trap 'exit 0' TERM",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-cancel-acp\",\"version\":\"0.5.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_cancel\"}}'",
                "read _prompt",
                "sleep 20",
            ]
            .join("\n"),
        )
        .expect("write mock acp cancel script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-cancel", &script);

        let started = start_acp_session_job(
            &root,
            &descriptor,
            "wait".to_string(),
            AcpSessionOptions::default(),
            None,
        )
        .expect("start acp job");
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
                    .is_some_and(|job| job.pid.is_some() && job.status == "running")
            },
            Duration::from_secs(10),
        );

        let cancelled = cancel_codex_job(&root, Some(&id)).expect("cancel acp job");

        assert!(cancelled.contains("Cancelled"));
        let job = find_codex_job(&root, &id)
            .expect("find job")
            .expect("job exists");
        assert_eq!(job.status, "cancelled");
        wait_until(
            || {
                fs::read_to_string(&job.result_path)
                    .is_ok_and(|result| result.contains("# ACP session result"))
            },
            Duration::from_secs(5),
        );
        let result = fs::read_to_string(&job.result_path).expect("read cancellation result");
        assert!(result.contains("# ACP session result"));
        assert!(result.contains("status: cancelled"));
        assert!(result.contains("session: session_cancel"));
        assert!(result.contains("tool_calls: none"));
        wait_until(
            || fs::read_to_string(&job.log_path).is_ok_and(|log| log.contains("session/cancel")),
            Duration::from_secs(5),
        );
        let log = fs::read_to_string(&job.log_path).expect("read cancellation log");
        assert!(log.contains("session/cancel"));
    }

    #[test]
    fn cancellation_before_process_start_keeps_durable_job_nonterminal() {
        let root = temp_root("acp_cancel_before_pid");
        let id = "agent-session-before-pid";
        let record = CodexJobRecord {
            id: id.to_string(),
            kind: "acp-session".to_string(),
            status: "running".to_string(),
            pid: None,
            command: "mock-acp".to_string(),
            task: "wait for process startup".to_string(),
            log_path: codex_job_artifact_path(&root, id, "jsonl"),
            result_path: codex_job_artifact_path(&root, id, "result.md"),
            baseline_path: codex_job_artifact_path(&root, id, "baseline.status"),
            updated_at: timestamp_millis(),
            agent: None,
        };
        append_codex_job_record(&root, "started", &record).expect("record pending job");

        let error = cancel_codex_job(&root, Some(id)).expect_err("termination is not confirmed");
        let persisted = find_codex_job(&root, id)
            .expect("read pending job")
            .expect("pending job exists");

        assert!(error.contains("termination is not confirmed"));
        assert_eq!(persisted.status, "running");
        assert!(acp_job_cancel_path(&root, id).exists());
    }

    #[test]
    fn acp_async_job_sends_session_cancel_when_agent_supports_it() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_async_session_cancel");
        let script = root.join("mock-acp-session-cancel.sh");
        fs::write(
            &script,
            [
                "#!/bin/sh",
                "read _init",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{\"sessionCancel\":true},\"agentInfo\":{\"name\":\"mock-session-cancel-acp\",\"version\":\"0.6.0\"}}}'",
                "read _new_session",
                "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_explicit_cancel\"}}'",
                "read _prompt",
                "while IFS= read -r line; do",
                "  case \"$line\" in",
                "    *'\"method\":\"session/cancel\"'*)",
                "      printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"status\":\"cancelled\"}}'",
                "      printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/notification\",\"params\":{\"sessionId\":\"session_explicit_cancel\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"cancelled\"}}}'",
                "      exit 0",
                "      ;;",
                "  esac",
                "done",
            ]
            .join("\n"),
        )
        .expect("write mock acp session cancel script");
        make_executable(&script);
        let descriptor = mock_acp_descriptor("mock-acp-session-cancel", &script);

        let started = start_acp_session_job(
            &root,
            &descriptor,
            "wait".to_string(),
            AcpSessionOptions::default(),
            None,
        )
        .expect("start acp job");
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
                    .is_some_and(|job| job.pid.is_some() && job.status == "running")
            },
            Duration::from_secs(10),
        );

        let cancelled = cancel_codex_job(&root, Some(&id)).expect("cancel acp job");

        assert!(cancelled.contains("Cancelled"));
        wait_until(
            || {
                find_codex_job(&root, &id)
                    .ok()
                    .flatten()
                    .is_some_and(|job| job.status == "cancelled")
            },
            Duration::from_secs(10),
        );
        let job = find_codex_job(&root, &id)
            .expect("find job")
            .expect("job exists");
        let log = fs::read_to_string(&job.log_path).expect("read cancellation log");
        assert!(log.contains("session/cancel"));
        let result = fs::read_to_string(&job.result_path).expect("read cancellation result");
        assert!(result.contains("status: cancelled"));
        assert!(result.contains("session_explicit_cancel"));
    }

    #[test]
    fn acp_initialize_probe_reports_timeout_with_log() {
        let _guard = subprocess_test_guard();
        let root = temp_root("acp_probe_timeout");
        let script = root.join("silent-acp.sh");
        fs::write(&script, "#!/bin/sh\nsleep 10\n").expect("write silent acp script");
        make_executable(&script);

        let error = run_acp_initialize_probe(&root, &script.to_string_lossy())
            .expect_err("probe should time out");

        assert!(error.contains("timed out"));
        assert!(error.contains(".viden/agents/acp-doctor-"));
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
        assert!(report.job_store.ends_with(".viden/agents/codex-jobs.jsonl"));
    }

    #[test]
    fn codex_diagnostics_reports_missing_command() {
        let root = temp_root("codex_doctor_missing");

        let report = codex_diagnostics(&root, "viden-definitely-missing-codex");

        let CodexDiagnosticReport::Unavailable(reason) = report else {
            panic!("expected unavailable Codex report");
        };
        assert!(reason.contains("failed to launch"));
    }

    #[cfg(unix)]
    #[test]
    fn codex_job_lifecycle_records_result_artifacts() {
        let _guard = subprocess_test_guard();
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
            Duration::from_secs(15),
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
        let _guard = subprocess_test_guard();
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

    #[test]
    #[should_panic(expected = "timed out waiting for runtime event condition")]
    fn channel_event_wait_fails_at_unmet_condition_boundary() {
        let (_sender, receiver) = mpsc::channel::<RuntimeEvent>();

        let _ = wait_for_channel_events(&receiver, Duration::ZERO, |_| false);
    }

    fn wait_for_channel_events(
        receiver: &mpsc::Receiver<RuntimeEvent>,
        timeout: Duration,
        predicate: impl Fn(&[RuntimeEvent]) -> bool,
    ) -> Vec<RuntimeEvent> {
        let deadline = Instant::now() + timeout;
        let mut events = Vec::new();
        while Instant::now() < deadline {
            if predicate(&events) {
                return events;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            match receiver.recv_timeout(remaining.min(Duration::from_millis(20))) {
                Ok(event) => events.push(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(
            predicate(&events),
            "timed out waiting for runtime event condition; observed events: {events:#?}"
        );
        events
    }

    fn subprocess_test_guard() -> MutexGuard<'static, ()> {
        // Mock app-server and Codex job tests exchange lines with subprocesses;
        // serialize them so default parallel test runs do not starve timeout paths.
        SUBPROCESS_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "viden-runtime-{name}-{}-{}-{}",
            std::process::id(),
            timestamp_millis(),
            TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn mock_acp_descriptor(agent_id: &str, script: &Path) -> AgentPluginDescriptor {
        AgentPluginDescriptor {
            agent_id: agent_id.to_string(),
            display_name: "Mock ACP".to_string(),
            version: "test".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: script.display().to_string(),
                args: vec![],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::StreamingUpdates,
            ],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec![],
            config_schema_version: 1,
        }
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
