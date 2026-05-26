use std::{env, path::PathBuf};

use crate::SessionEngine;

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
        transport: "template",
        entrypoint: "/lane codex <task>",
        binary: Some("codex"),
        config_env: Some("ROBOCODE_LANE_CODEX_TEMPLATE"),
        config_label: "template",
        config_required: true,
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
    pub(super) fn handle_agent_command(&self, args: &[String]) -> Result<String, String> {
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => Ok(render_agent_list()),
            "doctor" => Ok(render_agent_doctor(args.get(1).map(String::as_str))),
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
            "  /agent logs <id>",
            "",
            "Agent commands are read-only operator views. Use `/lane ...` commands to launch or control lanes.",
        ]
        .join("\n")
    }
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

fn render_agent_doctor(target: Option<&str>) -> String {
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
    }
    lines.join("\n")
}

fn render_agent_logs_help() -> String {
    [
        "Agent logs:",
        "  /agent logs <id> is reserved for adapter-level logs.",
        "  Use `/lane inspect <id>` today for lane logs, artifacts, decisions, and transport evidence.",
    ]
    .join("\n")
}

fn adapter_readiness(adapter: AgentAdapterDescriptor) -> &'static str {
    let binary_ready = match adapter.binary {
        Some(binary) => command_exists(binary),
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
