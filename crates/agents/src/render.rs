use std::{env, path::Path};

use super::acp::*;
use super::codex::*;
use super::infra::*;
use viden_plugin_api::{
    AgentAuthMode, AgentPermissionProfile, AgentPluginCapability, AgentPluginDescriptor,
    AgentSource, AgentTransport,
};

/// Render the `/agent list` table of registered adapters and their transports.
pub fn render_agent_list() -> String {
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

/// Render `/agent doctor`: readiness, binary presence, auth hints, and
/// configuration for one adapter, or for all of them when `target` is `None`.
pub fn render_agent_doctor(target: Option<&str>, cwd: &Path) -> String {
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

pub(super) fn render_agent_descriptor_doctor(agent: &AgentPluginDescriptor) -> Vec<String> {
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

pub(super) fn agent_transport_label(agent: &AgentPluginDescriptor) -> &'static str {
    match agent.transport {
        AgentTransport::Acp => "acp",
        AgentTransport::AppServer => "app-server",
        AgentTransport::Template => "template",
        AgentTransport::Pty => "pty",
        AgentTransport::Tmux => "tmux",
    }
}

pub(super) fn agent_source_label(agent: &AgentPluginDescriptor) -> &'static str {
    match agent.source {
        AgentSource::Registry => "registry",
        AgentSource::LocalCommand => "local-command",
        AgentSource::Custom => "custom",
    }
}

pub(super) fn agent_permission_profile_label(agent: &AgentPluginDescriptor) -> &'static str {
    match agent.permission_profile {
        AgentPermissionProfile::ReadOnlyProbe => "read-only-probe",
        AgentPermissionProfile::RuntimeGated => "runtime-gated",
        AgentPermissionProfile::AgentNative => "agent-native",
    }
}

pub(super) fn agent_descriptor_readiness(agent: &AgentPluginDescriptor) -> &'static str {
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

pub(super) fn agent_auth_hint(agent: &AgentPluginDescriptor) -> &'static str {
    if agent.agent_id == "kiro-cli" {
        "agent-native; run `kiro-cli login` and `kiro-cli doctor`"
    } else if agent.auth_modes.contains(&AgentAuthMode::AgentNative) {
        "agent-native; use the agent's own login/config"
    } else {
        "none declared"
    }
}

pub(super) fn agent_command_line(agent: &AgentPluginDescriptor) -> String {
    let mut parts = vec![agent.command.command.clone()];
    parts.extend(acp_agent_command_args(agent));
    parts.join(" ")
}

pub(super) fn agent_capability_label(capability: &AgentPluginCapability) -> &'static str {
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

pub(super) fn agent_capability_id(capability: &AgentPluginCapability) -> &'static str {
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

/// Render the `/agent logs` help text.
pub fn render_agent_logs_help() -> String {
    [
        "Agent logs:",
        "  /agent result <id> shows tracked Codex or ACP agent job output.",
        "  Use `/lane inspect <id>` for lane logs, artifacts, decisions, and transport evidence.",
    ]
    .join("\n")
}

pub(super) fn render_acp_probe(cwd: &Path) -> Vec<String> {
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

pub(super) fn render_codex_doctor(cwd: &Path) -> Vec<String> {
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

pub(super) fn render_codex_protocol_probe(cwd: &Path, command: &str) -> Vec<String> {
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
