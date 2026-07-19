use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::lane::LaneEffectError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnProcess {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// When present, both stdout and stderr append to this durable lane log.
    /// When absent, they are sent to the null device rather than unread pipes.
    pub output_log: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneProcessHandle {
    pub id: String,
}

pub trait ProcessBackend: Send + Sync {
    fn spawn(&self, request: &SpawnProcess) -> Result<LaneProcessHandle, LaneEffectError>;
    fn send(&self, handle: &LaneProcessHandle, input: &[u8]) -> Result<(), LaneEffectError>;
    fn stop(&self, handle: &LaneProcessHandle) -> Result<(), LaneEffectError>;
}

#[derive(Debug, Default)]
pub struct LocalProcessBackend {
    children: Mutex<BTreeMap<String, Child>>,
}

impl ProcessBackend for LocalProcessBackend {
    fn spawn(&self, request: &SpawnProcess) -> Result<LaneProcessHandle, LaneEffectError> {
        validate_environment(&request.env)?;
        let mut command = Command::new(&request.command);
        command
            .args(&request.args)
            .current_dir(&request.cwd)
            .stdin(Stdio::piped());
        configure_process_output(&mut command, request.output_log.as_deref())?;
        configure_process_group(&mut command);
        for (key, value) in &request.env {
            command.env(key, value);
        }
        let child = command
            .spawn()
            .map_err(|err| LaneEffectError::Io(err.to_string()))?;
        let id = format!("process-{}", child.id());
        self.children
            .lock()
            .map_err(|_| LaneEffectError::Io("process registry poisoned".to_string()))?
            .insert(id.clone(), child);
        Ok(LaneProcessHandle { id })
    }

    fn send(&self, handle: &LaneProcessHandle, input: &[u8]) -> Result<(), LaneEffectError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| LaneEffectError::Io("process registry poisoned".to_string()))?;
        let child = children.get_mut(&handle.id).ok_or_else(|| {
            LaneEffectError::Io(format!("unknown process handle `{}`", handle.id))
        })?;
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            LaneEffectError::Io(format!("process handle `{}` has no stdin", handle.id))
        })?;
        stdin
            .write_all(input)
            .and_then(|_| stdin.flush())
            .map_err(|err| LaneEffectError::Io(err.to_string()))
    }

    fn stop(&self, handle: &LaneProcessHandle) -> Result<(), LaneEffectError> {
        let mut child = self
            .children
            .lock()
            .map_err(|_| LaneEffectError::Io("process registry poisoned".to_string()))?
            .remove(&handle.id)
            .ok_or_else(|| {
                LaneEffectError::Io(format!("unknown process handle `{}`", handle.id))
            })?;
        let leader_exited = child
            .try_wait()
            .map_err(|err| LaneEffectError::Io(err.to_string()))?
            .is_some();
        stop_process_group(&mut child, leader_exited)?;
        if !leader_exited {
            child
                .wait()
                .map_err(|err| LaneEffectError::Io(err.to_string()))?;
        }
        Ok(())
    }
}

fn configure_process_output(
    command: &mut Command,
    output_log: Option<&Path>,
) -> Result<(), LaneEffectError> {
    let Some(output_log) = output_log else {
        command.stdout(Stdio::null()).stderr(Stdio::null());
        return Ok(());
    };
    if let Some(parent) = output_log.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| LaneEffectError::Io(format!("{}: {err}", parent.display())))?;
    }
    let output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_log)
        .map_err(|err| LaneEffectError::Io(format!("{}: {err}", output_log.display())))?;
    let stderr = output
        .try_clone()
        .map_err(|err| LaneEffectError::Io(format!("{}: {err}", output_log.display())))?;
    command
        .stdout(Stdio::from(output))
        .stderr(Stdio::from(stderr));
    Ok(())
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn stop_process_group(child: &mut Child, leader_exited: bool) -> Result<(), LaneEffectError> {
    let process_group = format!("-{}", child.id());
    let term = Command::new("kill")
        .args(["-TERM", "--", &process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| LaneEffectError::Io(err.to_string()))?;
    if !term.success() {
        if leader_exited {
            return Ok(());
        }
        child
            .kill()
            .map_err(|err| LaneEffectError::Io(err.to_string()))?;
        return Ok(());
    }
    if leader_exited {
        return Ok(());
    }
    for _ in 0..10 {
        if child
            .try_wait()
            .map_err(|err| LaneEffectError::Io(err.to_string()))?
            .is_some()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = Command::new("kill")
        .args(["-KILL", "--", &process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

#[cfg(not(unix))]
fn stop_process_group(child: &mut Child, leader_exited: bool) -> Result<(), LaneEffectError> {
    if leader_exited {
        return Ok(());
    }
    child
        .kill()
        .map_err(|err| LaneEffectError::Io(err.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKind {
    Tmux,
    Pty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnTerminal {
    pub kind: TerminalKind,
    pub session_name: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub output_log: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneTerminalHandle {
    pub id: String,
    pub kind: TerminalKind,
    pub attach_command: Option<String>,
}

/// Terminal effects are distinct from plain child processes because tmux and
/// PTY sessions have route-specific launch, input, attachment, and stop rules.
pub trait TerminalBackend: Send + Sync {
    fn spawn(&self, request: &SpawnTerminal) -> Result<LaneTerminalHandle, LaneEffectError>;
    fn send(&self, handle: &LaneTerminalHandle, input: &[u8]) -> Result<(), LaneEffectError>;
    fn stop(&self, handle: &LaneTerminalHandle) -> Result<(), LaneEffectError>;
}

#[derive(Debug)]
struct LocalTerminalSession {
    handle: LaneTerminalHandle,
    process: Option<LaneProcessHandle>,
    tmux_session: Option<String>,
}

#[derive(Debug, Default)]
pub struct LocalTerminalBackend {
    processes: LocalProcessBackend,
    sessions: Mutex<BTreeMap<String, LocalTerminalSession>>,
}

impl TerminalBackend for LocalTerminalBackend {
    fn spawn(&self, request: &SpawnTerminal) -> Result<LaneTerminalHandle, LaneEffectError> {
        validate_environment(&request.env)?;
        let session = match request.kind {
            TerminalKind::Tmux => self.spawn_tmux(request)?,
            TerminalKind::Pty => self.spawn_pty(request)?,
        };
        let handle = session.handle.clone();
        self.sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("terminal registry poisoned".to_string()))?
            .insert(handle.id.clone(), session);
        Ok(handle)
    }

    fn send(&self, handle: &LaneTerminalHandle, input: &[u8]) -> Result<(), LaneEffectError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("terminal registry poisoned".to_string()))?;
        let session = sessions.get(&handle.id).ok_or_else(|| {
            LaneEffectError::Io(format!("unknown terminal handle `{}`", handle.id))
        })?;
        match session.handle.kind {
            TerminalKind::Tmux => send_tmux_input(
                session
                    .tmux_session
                    .as_deref()
                    .expect("tmux session invariant"),
                input,
            ),
            TerminalKind::Pty => self.processes.send(
                session.process.as_ref().expect("pty process invariant"),
                input,
            ),
        }
    }

    fn stop(&self, handle: &LaneTerminalHandle) -> Result<(), LaneEffectError> {
        let session = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("terminal registry poisoned".to_string()))?
            .remove(&handle.id)
            .ok_or_else(|| {
                LaneEffectError::Io(format!("unknown terminal handle `{}`", handle.id))
            })?;
        match session.handle.kind {
            TerminalKind::Tmux => run_tmux([
                "kill-session".to_string(),
                "-t".to_string(),
                session.tmux_session.expect("tmux session invariant"),
            ]),
            TerminalKind::Pty => self
                .processes
                .stop(&session.process.expect("pty process invariant")),
        }
    }
}

impl LocalTerminalBackend {
    fn spawn_tmux(&self, request: &SpawnTerminal) -> Result<LocalTerminalSession, LaneEffectError> {
        let session_name = request
            .session_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| LaneEffectError::Io("tmux requires a session name".to_string()))?;
        if let Some(parent) = request.output_log.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| LaneEffectError::Io(format!("{}: {err}", parent.display())))?;
        }
        run_tmux([
            "new-session".to_string(),
            "-d".to_string(),
            "-s".to_string(),
            session_name.to_string(),
            "-c".to_string(),
            request.cwd.to_string_lossy().to_string(),
        ])?;
        let configure = (|| {
            run_tmux([
                "pipe-pane".to_string(),
                "-o".to_string(),
                "-t".to_string(),
                session_name.to_string(),
                format!(
                    "cat >> {}",
                    shell_quote(&request.output_log.to_string_lossy())
                ),
            ])?;
            let command =
                shell_command_line_with_env(&request.env, &request.command, &request.args)?;
            run_tmux([
                "send-keys".to_string(),
                "-t".to_string(),
                session_name.to_string(),
                "--".to_string(),
                command,
                "Enter".to_string(),
            ])
        })();
        if let Err(error) = configure {
            let cleanup = run_tmux([
                "kill-session".to_string(),
                "-t".to_string(),
                session_name.to_string(),
            ]);
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(LaneEffectError::Io(format!(
                    "{error}; tmux cleanup failed: {cleanup_error}"
                ))),
            };
        }
        Ok(LocalTerminalSession {
            handle: LaneTerminalHandle {
                id: format!("tmux-{session_name}"),
                kind: TerminalKind::Tmux,
                attach_command: Some(format!("tmux attach -t {}", shell_quote(session_name))),
            },
            process: None,
            tmux_session: Some(session_name.to_string()),
        })
    }

    fn spawn_pty(&self, request: &SpawnTerminal) -> Result<LocalTerminalSession, LaneEffectError> {
        let process_request = platform_pty_process(request)?;
        let process = self.processes.spawn(&process_request)?;
        Ok(LocalTerminalSession {
            handle: LaneTerminalHandle {
                id: format!("pty-{}", process.id),
                kind: TerminalKind::Pty,
                attach_command: None,
            },
            process: Some(process),
            tmux_session: None,
        })
    }
}

fn run_tmux(args: impl IntoIterator<Item = String>) -> Result<(), LaneEffectError> {
    let args = args.into_iter().collect::<Vec<_>>();
    let status = Command::new("tmux")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| LaneEffectError::Io(format!("failed to run tmux: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(LaneEffectError::Io(format!(
            "tmux {} exited with {status}",
            args.join(" ")
        )))
    }
}

fn send_tmux_input(session_name: &str, input: &[u8]) -> Result<(), LaneEffectError> {
    let mut load = Command::new("tmux")
        .args(["load-buffer", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| LaneEffectError::Io(format!("failed to run tmux: {err}")))?;
    load.stdin
        .take()
        .expect("piped tmux stdin invariant")
        .write_all(input)
        .map_err(|err| LaneEffectError::Io(err.to_string()))?;
    let status = load
        .wait()
        .map_err(|err| LaneEffectError::Io(err.to_string()))?;
    if !status.success() {
        return Err(LaneEffectError::Io(format!(
            "tmux load-buffer exited with {status}"
        )));
    }
    run_tmux([
        "paste-buffer".to_string(),
        "-d".to_string(),
        "-t".to_string(),
        session_name.to_string(),
    ])
}

#[cfg(target_os = "macos")]
fn platform_pty_process(request: &SpawnTerminal) -> Result<SpawnProcess, LaneEffectError> {
    let mut args = vec![
        "-q".to_string(),
        "-a".to_string(),
        "-F".to_string(),
        request.output_log.to_string_lossy().to_string(),
    ];
    args.push(request.command.clone());
    args.extend(request.args.clone());
    Ok(SpawnProcess {
        command: "script".to_string(),
        args,
        cwd: request.cwd.clone(),
        env: request.env.clone(),
        output_log: None,
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_pty_process(request: &SpawnTerminal) -> Result<SpawnProcess, LaneEffectError> {
    Ok(SpawnProcess {
        command: "script".to_string(),
        args: vec![
            "-q".to_string(),
            "-a".to_string(),
            "-f".to_string(),
            "-c".to_string(),
            shell_command_line(&request.command, &request.args),
            request.output_log.to_string_lossy().to_string(),
        ],
        cwd: request.cwd.clone(),
        env: request.env.clone(),
        output_log: None,
    })
}

#[cfg(not(unix))]
fn platform_pty_process(_request: &SpawnTerminal) -> Result<SpawnProcess, LaneEffectError> {
    Err(LaneEffectError::Io(
        "PTY terminal backend is unsupported on this platform".to_string(),
    ))
}

fn shell_command_line(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_command_line_with_env(
    environment: &[(String, String)],
    command: &str,
    args: &[String],
) -> Result<String, LaneEffectError> {
    validate_environment(environment)?;
    if environment.is_empty() {
        return Ok(shell_command_line(command, args));
    }
    let assignments = environment
        .iter()
        .map(|(key, value)| shell_quote(&format!("{key}={value}")))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(format!(
        "env {assignments} {}",
        shell_command_line(command, args)
    ))
}

fn validate_environment(environment: &[(String, String)]) -> Result<(), LaneEffectError> {
    for (key, value) in environment {
        if key.is_empty() || key.contains(['=', '\0']) || value.contains('\0') {
            return Err(LaneEffectError::Io(format!(
                "invalid environment entry `{key}`"
            )));
        }
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod process_tests {
    use super::*;

    #[test]
    fn tmux_command_line_carries_validated_environment() {
        let command = shell_command_line_with_env(
            &[("VIDEN_LANE".into(), "lane a".into())],
            "worker",
            &["--arg".into(), "value with space".into()],
        )
        .unwrap();

        assert_eq!(
            command,
            "env 'VIDEN_LANE=lane a' 'worker' '--arg' 'value with space'"
        );
        assert!(shell_command_line_with_env(&[("BAD=KEY".into(), "x".into())], "w", &[]).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn pty_script_appends_and_flushes_on_macos() {
        let request = terminal_fixture();
        let process = platform_pty_process(&request).unwrap();

        assert_eq!(&process.args[..3], ["-q", "-a", "-F"]);
        assert_eq!(process.env, request.env);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn pty_script_appends_and_flushes_on_linux() {
        let request = terminal_fixture();
        let process = platform_pty_process(&request).unwrap();

        assert_eq!(&process.args[..3], ["-q", "-a", "-f"]);
        assert_eq!(process.env, request.env);
    }

    #[cfg(unix)]
    #[test]
    fn stop_terminates_process_group_after_leader_exits() {
        let cwd = std::env::temp_dir().join(format!(
            "viden-process-group-stop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&cwd).unwrap();
        let background_pid_path = cwd.join("background.pid");
        let backend = LocalProcessBackend::default();
        let handle = backend
            .spawn(&SpawnProcess {
                command: "sh".into(),
                args: vec![
                    "-c".into(),
                    format!(
                        "sleep 30 & echo $! > {}",
                        shell_quote(&background_pid_path.to_string_lossy())
                    ),
                ],
                cwd: cwd.clone(),
                env: Vec::new(),
                output_log: None,
            })
            .unwrap();

        let leader_exited = (0..100).any(|_| {
            let exited = backend
                .children
                .lock()
                .unwrap()
                .get_mut(&handle.id)
                .unwrap()
                .try_wait()
                .unwrap()
                .is_some();
            if !exited {
                thread::sleep(Duration::from_millis(10));
            }
            exited
        });
        assert!(leader_exited, "shell leader did not exit in time");
        let background_pid = fs::read_to_string(&background_pid_path).unwrap();
        let background_pid = background_pid.trim();
        assert!(
            Command::new("kill")
                .args(["-0", background_pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success(),
            "background process should be alive before stop"
        );

        backend.stop(&handle).unwrap();

        let stopped = (0..100).any(|_| {
            let stopped = !Command::new("kill")
                .args(["-0", background_pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success();
            if !stopped {
                thread::sleep(Duration::from_millis(10));
            }
            stopped
        });
        assert!(stopped, "background process survived process-group stop");
        fs::remove_dir_all(cwd).unwrap();
    }

    #[cfg(unix)]
    fn terminal_fixture() -> SpawnTerminal {
        SpawnTerminal {
            kind: TerminalKind::Pty,
            session_name: None,
            command: "worker".into(),
            args: vec!["--lane".into()],
            cwd: std::env::temp_dir(),
            env: vec![("VIDEN_LANE".into(), "lane-a".into())],
            output_log: std::env::temp_dir().join("viden-process-test.log"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeTerminalSession {
    pub handle: LaneTerminalHandle,
    pub request: SpawnTerminal,
    pub inputs: Vec<Vec<u8>>,
    pub stopped: bool,
}

#[derive(Debug, Default, Clone)]
pub struct FakeTerminalBackend {
    sessions: Arc<Mutex<Vec<FakeTerminalSession>>>,
}

impl FakeTerminalBackend {
    pub fn sessions(&self) -> Vec<FakeTerminalSession> {
        self.sessions
            .lock()
            .expect("fake terminal poisoned")
            .clone()
    }
}

impl TerminalBackend for FakeTerminalBackend {
    fn spawn(&self, request: &SpawnTerminal) -> Result<LaneTerminalHandle, LaneEffectError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("fake terminal poisoned".to_string()))?;
        let handle = LaneTerminalHandle {
            id: format!("fake-terminal-{}", sessions.len() + 1),
            kind: request.kind,
            attach_command: request
                .session_name
                .as_ref()
                .map(|session| format!("tmux attach -t {}", shell_quote(session))),
        };
        sessions.push(FakeTerminalSession {
            handle: handle.clone(),
            request: request.clone(),
            inputs: Vec::new(),
            stopped: false,
        });
        Ok(handle)
    }

    fn send(&self, handle: &LaneTerminalHandle, input: &[u8]) -> Result<(), LaneEffectError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("fake terminal poisoned".to_string()))?;
        let Some(session) = sessions
            .iter_mut()
            .find(|session| session.handle == *handle)
        else {
            return Err(LaneEffectError::Io(format!(
                "unknown terminal handle `{}`",
                handle.id
            )));
        };
        session.inputs.push(input.to_vec());
        Ok(())
    }

    fn stop(&self, handle: &LaneTerminalHandle) -> Result<(), LaneEffectError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| LaneEffectError::Io("fake terminal poisoned".to_string()))?;
        let Some(session) = sessions
            .iter_mut()
            .find(|session| session.handle == *handle)
        else {
            return Err(LaneEffectError::Io(format!(
                "unknown terminal handle `{}`",
                handle.id
            )));
        };
        session.stopped = true;
        Ok(())
    }
}
