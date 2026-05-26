use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
            "doctor" => Ok(render_agent_doctor(
                args.get(1).map(String::as_str),
                &self.cwd,
            )),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcpProbeEvidence {
    protocol_version: String,
    agent_label: String,
    log_path: PathBuf,
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

    let response = match read_line_with_timeout(stdout, Duration::from_secs(2)) {
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
        let log = fs::read_to_string(evidence.log_path).expect("read jsonl log");
        assert!(log.contains(r#""method\":\"initialize"#));
        assert!(log.contains("mock-acp"));
    }

    #[test]
    fn acp_initialize_probe_reports_timeout_with_log() {
        let root = temp_root("acp_probe_timeout");
        let script = root.join("silent-acp.sh");
        fs::write(&script, "#!/bin/sh\nsleep 3\n").expect("write silent acp script");
        make_executable(&script);

        let error = run_acp_initialize_probe(&root, &script.to_string_lossy())
            .expect_err("probe should time out");

        assert!(error.contains("timed out"));
        assert!(error.contains(".robocode/agents/acp-doctor-"));
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "robocode-core-{name}-{}-{}",
            std::process::id(),
            timestamp_millis()
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
