use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use viden_tools::InteractiveProcessControl;
use viden_types::AgentCapabilityRecord;

pub(super) const SHELL_SCRIPT_THRESHOLD: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AgentAdapterDescriptor {
    pub(super) id: &'static str,
    pub(super) display_name: &'static str,
    pub(super) transport: &'static str,
    pub(super) entrypoint: &'static str,
    pub(super) binary: Option<&'static str>,
    pub(super) config_env: Option<&'static str>,
    pub(super) config_label: &'static str,
    pub(super) config_required: bool,
}

pub(super) const AGENT_ADAPTERS: [AgentAdapterDescriptor; 6] = [
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

pub(super) fn env_flag_enabled(env_name: &str) -> bool {
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

pub(super) fn adapter_capability(adapter: AgentAdapterDescriptor) -> AgentCapabilityRecord {
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

pub(super) fn adapter_mutation_mode(adapter: AgentAdapterDescriptor) -> &'static str {
    match adapter.id {
        "codex" => "read-only by default; workspace-write requires approval",
        "claude" | "custom-template" => "template-defined; isolate before apply",
        "tmux" | "pty" => "interactive; operator-controlled",
        "acp" => "agent-native; mutating file/terminal requests require runtime bridge",
        _ => "unknown",
    }
}

pub(super) fn adapter_evidence_mode(adapter: AgentAdapterDescriptor) -> &'static str {
    match adapter.id {
        "codex" => "job result, protocol/app-server log, lane artifacts",
        "claude" | "custom-template" => "lane log, envelope, timeline, artifacts",
        "tmux" | "pty" => "terminal tail, lane log, timeline, artifacts",
        "acp" => "JSONL wire log, session result, permission decisions",
        _ => "unknown",
    }
}

pub(super) fn adapter_readiness(adapter: AgentAdapterDescriptor) -> &'static str {
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

pub(super) fn process_is_running(pid: u32) -> bool {
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

pub(super) fn terminate_process(pid: u32) -> Result<(), String> {
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
pub(super) fn send_unix_signal(pid: u32, signal: &str) -> Result<(), String> {
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
pub(super) fn wait_for_process_stop(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    !process_is_running(pid)
}

pub(super) fn tail_text(path: &Path, max_lines: usize) -> Result<String, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let lines = content.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

pub(super) fn relative_millis(ts: u128) -> String {
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

pub(super) fn truncate_for_line(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

pub(super) fn command_output(
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

pub(super) fn read_pipe(mut pipe: impl std::io::Read) -> String {
    let mut output = String::new();
    let _ = pipe.read_to_string(&mut output);
    output
}

pub(super) fn join_output(stdout: String, stderr: String) -> String {
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (false, false) => format!("{}\n{}", stdout.trim(), stderr.trim()),
        (false, true) => stdout.trim().to_string(),
        (true, false) => stderr.trim().to_string(),
        (true, true) => String::new(),
    }
}

pub(super) fn first_output_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn command_line(command: &str, args: &[&str]) -> String {
    std::iter::once(command)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn command_line_owned(command: &str, args: &[String]) -> String {
    std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Poll the interactive control handle until the process exits or the
/// timeout elapses; `Some(exit_code)` only when the exit was observed.
pub(super) fn wait_interactive_exit_timeout(
    control: &mut dyn InteractiveProcessControl,
    timeout: Duration,
) -> Option<Option<i32>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match control.try_wait() {
            Ok(Some(exit_code)) => return Some(exit_code),
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => return None,
        }
    }
    None
}

pub(super) fn child_stderr_tail(child: &mut Child) -> Option<String> {
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

pub(super) fn wait_child_timeout(child: &mut Child, timeout: Duration) -> bool {
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

pub(super) fn tail_lines(text: &str, max_lines: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ShellCommandPlan {
    pub(super) program: &'static str,
    pub(super) inline_args: Vec<String>,
    pub(super) script_extension: Option<&'static str>,
    pub(super) script_body: Option<String>,
}

pub(super) fn shell_command_plan(command: &str, windows: bool) -> ShellCommandPlan {
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

pub(super) fn shell_command(cwd: &Path, command: &str) -> Result<Command, String> {
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

pub(super) fn acp_shell_script_path(cwd: &Path, extension: &str) -> PathBuf {
    cwd.join(".viden")
        .join("tmp")
        .join(format!("acp-command-{}.{}", timestamp_millis(), extension))
}

pub(super) fn read_lines_async(
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

pub(super) fn read_line_with_timeout(
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

pub(super) fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(super) fn write_probe_log(path: &Path, entries: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, entries.join("\n") + "\n").map_err(|err| err.to_string())
}

pub(super) fn append_agent_job_log_event(
    path: &Path,
    direction: &str,
    payload: &str,
) -> Result<(), String> {
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

pub(super) fn jsonl_event(direction: &str, payload: &str) -> String {
    format!(
        r#"{{"direction":"{}","payload":"{}"}}"#,
        escape_json_fragment(direction),
        escape_json_fragment(payload)
    )
}

pub(super) fn escape_json_fragment(value: &str) -> String {
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

pub(super) fn json_string_field(response: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = response.find(&marker)? + marker.len();
    let rest = response[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub(super) fn json_object_string_field(
    response: &str,
    object: &str,
    field: &str,
) -> Option<String> {
    let marker = format!(r#""{object}":"#);
    let start = response.find(&marker)? + marker.len();
    json_string_field(&response[start..], field)
}

pub(super) fn json_number_field(response: &str, field: &str) -> Option<String> {
    let marker = format!(r#""{field}":"#);
    let start = response.find(&marker)? + marker.len();
    let value = response[start..]
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!value.is_empty()).then_some(value)
}

pub(super) fn env_is_configured(key: &str) -> bool {
    env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(super) fn command_exists(command: &str) -> bool {
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

pub(super) fn is_executable_file(path: impl Into<PathBuf>) -> bool {
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

pub(super) const fn pty_binary() -> Option<&'static str> {
    #[cfg(unix)]
    {
        Some("script")
    }
    #[cfg(not(unix))]
    {
        None
    }
}
