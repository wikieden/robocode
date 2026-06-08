use std::{
    env, fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use robocode_core::SessionEngine;

use super::input::should_exit;
use super::state::{
    CompanionScreen, ProviderOption, ProviderStatus, TerminalLane, TuiEntry, TuiState,
    WorkspaceSnapshot, lane_store_path, load_lanes, load_screens, refresh_lane_runtime, save_lanes,
    save_screens, screen_store_path,
};
use super::terminal::TerminalGuard;

const SCREEN_SHELL_SCRIPT_THRESHOLD: usize = 32 * 1024;

pub(crate) fn run_side_tui_with_theme(
    engine: &SessionEngine,
    startup_summary: &str,
    screen: SideScreen,
    theme_name: Option<&str>,
) -> Result<(), String> {
    let mut terminal = TerminalGuard::enter_with_theme(theme_name)?;
    let root = std::env::current_dir().ok();
    let lane_store = root.as_ref().map(|root| lane_store_path(root));
    let screen_store = root.as_ref().map(|root| screen_store_path(root));
    let mut state = TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        provider_catalog: engine
            .provider_descriptors()
            .iter()
            .map(ProviderOption::from_descriptor)
            .collect(),
        provider_status: ProviderStatus::from_telemetry(&engine.provider_telemetry()),
        theme_name: terminal.theme_name().to_string(),
        input: String::new(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        pending_turn: None,
        streaming_assistant: None,
        transcript_scroll: 0,
        entries: vec![TuiEntry {
            label: "system".to_string(),
            body: format!("RoboCode side monitor ready. Esc or Ctrl-C exits.\n{startup_summary}"),
        }],
        workspace: WorkspaceSnapshot::load_current(),
        tasks: engine.active_task_snapshot().unwrap_or_default(),
        runtime_tasks: engine.agent_task_snapshot(),
        memory: engine.memory_snapshot().unwrap_or_default(),
        screens: load_side_screens(screen_store.as_deref(), screen),
        lanes: load_side_lanes(lane_store.as_deref()),
        lane_store,
        focused_lane: None,
        interaction_panel: None,
    };
    if let Some(path) = screen_store.as_deref() {
        let _ = save_screens(path, &state.screens);
    }
    draw_side_screen(&mut terminal, &state, screen)?;

    loop {
        if event::poll(Duration::from_millis(750)).map_err(|err| err.to_string())? {
            let event = event::read().map_err(|err| err.to_string())?;
            let key = match event {
                Event::Key(key) => key,
                Event::Resize(_, _) => {
                    draw_side_screen(&mut terminal, &state, screen)?;
                    continue;
                }
                _ => continue,
            };
            if should_exit(key) {
                break;
            }
            if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
                let theme_name = terminal.cycle_theme();
                state.theme_name = theme_name.to_string();
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: format!("Switched TUI theme to `{theme_name}`."),
                });
            }
        }
        if let Some(path) = state.lane_store.as_deref() {
            state.lanes = load_lanes(path);
            refresh_lane_runtime(path, &mut state.lanes);
            let _ = save_lanes(path, &state.lanes);
        }
        if let Some(path) = screen_store.as_deref() {
            state.screens = load_side_screens(Some(path), screen);
            let _ = save_screens(path, &state.screens);
        }
        state.workspace = WorkspaceSnapshot::load_current();
        state.tasks = engine.active_task_snapshot().unwrap_or_default();
        state.memory = engine.memory_snapshot().unwrap_or_default();
        draw_side_screen(&mut terminal, &state, screen)?;
    }

    terminal.leave()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideScreen {
    Lanes,
    Ops,
}

impl SideScreen {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "side" | "side-1" => Some(Self::Lanes),
            "side-2" | "ops" => Some(Self::Ops),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Lanes => "side-1",
            Self::Ops => "side-2",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Lanes => "Agent lanes",
            Self::Ops => "Workspace ops",
        }
    }
}

pub(super) fn handle_screen_command(input: &str, state: &mut TuiState) -> bool {
    let mut parts = input.split_whitespace();
    if parts.next() != Some("/screen") {
        return false;
    }
    match parts.next() {
        Some("side-1") | Some("side") => launch_companion_screen(state, SideScreen::Lanes),
        Some("side-2") | Some("ops") => launch_companion_screen(state, SideScreen::Ops),
        Some("list") => push_screen_list(state),
        Some("close") => match parts.next() {
            Some(screen) => close_companion_screen(state, screen),
            None => push_screen_usage(state),
        },
        Some("main") => state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: "Screen `main` is this active cockpit. Use `/screen side-1` or `/screen side-2` to attach companions.".to_string(),
        }),
        _ => state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: screen_usage().to_string(),
        }),
    }
    true
}

fn draw_side_screen(
    terminal: &mut TerminalGuard,
    state: &TuiState,
    screen: SideScreen,
) -> Result<(), String> {
    match screen {
        SideScreen::Lanes => terminal.draw_side(state),
        SideScreen::Ops => terminal.draw_ops(state),
    }
}

fn load_side_lanes(lane_store: Option<&Path>) -> Vec<TerminalLane> {
    lane_store.map(load_lanes).unwrap_or_default()
}

fn load_side_screens(screen_store: Option<&Path>, current: SideScreen) -> Vec<CompanionScreen> {
    let mut screens = screen_store.map(load_screens).unwrap_or_default();
    upsert_screen(&mut screens, current_screen_record(current));
    screens
}

fn current_screen_record(screen: SideScreen) -> CompanionScreen {
    CompanionScreen {
        id: screen.id().to_string(),
        title: screen.title().to_string(),
        status: "attached".to_string(),
        pid: Some(std::process::id()),
        summary: "current side-screen process".to_string(),
    }
}

fn upsert_screen(screens: &mut Vec<CompanionScreen>, screen: CompanionScreen) {
    if let Some(existing) = screens
        .iter_mut()
        .find(|candidate| candidate.id == screen.id)
    {
        *existing = screen;
    } else {
        screens.push(screen);
    }
}

fn launch_companion_screen(state: &mut TuiState, screen: SideScreen) {
    let id = screen.id();
    if state.screens.iter().any(|candidate| candidate.id == id) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!(
                "Screen `{id}` is already tracked. Use `/screen list` or `/screen close {id}`."
            ),
        });
        return;
    }
    if state.screens.len() >= 2 {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: "RoboCode supports at most two companion side screens: side-1 and side-2."
                .to_string(),
        });
        return;
    }
    match spawn_companion_screen(state, id) {
        Ok(pid) => {
            state.screens.push(CompanionScreen {
                id: id.to_string(),
                title: screen.title().to_string(),
                status: "launched".to_string(),
                pid: Some(pid),
                summary: format!("provider={} model={}", state.provider, state.model),
            });
            persist_screens(state);
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!(
                    "Launched screen `{id}` as pid {pid}.\nUse `/screen list` to inspect or `/screen close {id}` to stop tracking it."
                ),
            });
        }
        Err(err) => state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to launch screen `{id}`: {err}"),
        }),
    }
}

fn spawn_companion_screen(state: &TuiState, screen: &str) -> Result<u32, String> {
    let launch = screen_launch_context(screen, state)?;
    if let Some((env_key, template)) = screen_launch_template(screen) {
        let command = expand_launch_template(&template, &launch);
        if command.trim().is_empty() {
            return Err(format!("{env_key} expanded to an empty command"));
        }
        return spawn_shell_command(&command);
    }
    let mut command = Command::new(&launch.binary);
    command
        .arg("--tui-screen")
        .arg(screen)
        .arg("--provider")
        .arg(&state.provider)
        .arg("--model")
        .arg(&state.model)
        .arg("--tui-theme")
        .arg(&state.theme_name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if launch.cwd.is_dir() {
        command.current_dir(&launch.cwd);
    }
    let child = command.spawn().map_err(|err| err.to_string())?;
    Ok(child.id())
}

fn screen_launch_template(screen: &str) -> Option<(String, String)> {
    let specific_key = match screen {
        "side-1" => Some("ROBOCODE_SCREEN_SIDE_1_LAUNCH_TEMPLATE"),
        "side-2" => Some("ROBOCODE_SCREEN_SIDE_2_LAUNCH_TEMPLATE"),
        _ => None,
    };
    specific_key
        .and_then(|key| {
            env::var(key)
                .ok()
                .map(|template| (key.to_string(), template))
        })
        .or_else(|| {
            env::var("ROBOCODE_SCREEN_LAUNCH_TEMPLATE")
                .ok()
                .map(|template| ("ROBOCODE_SCREEN_LAUNCH_TEMPLATE".to_string(), template))
        })
}

fn spawn_shell_command(command: &str) -> Result<u32, String> {
    let mut shell = platform_shell_command(command)?;
    shell
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = shell.spawn().map_err(|err| err.to_string())?;
    Ok(child.id())
}

#[cfg(windows)]
fn platform_shell_command(command: &str) -> Result<Command, String> {
    build_platform_shell_command(command, true)
}

#[cfg(not(windows))]
fn platform_shell_command(command: &str) -> Result<Command, String> {
    build_platform_shell_command(command, false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellCommandPlan {
    program: &'static str,
    inline_args: Vec<String>,
    script_extension: Option<&'static str>,
    script_body: Option<String>,
}

fn screen_shell_command_plan(command: &str, windows: bool) -> ShellCommandPlan {
    let requires_script = command.len() > SCREEN_SHELL_SCRIPT_THRESHOLD;
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

fn build_platform_shell_command(command: &str, windows: bool) -> Result<Command, String> {
    let plan = screen_shell_command_plan(command, windows);
    let mut shell = Command::new(plan.program);
    shell.args(plan.inline_args);
    if let Some(body) = plan.script_body {
        let extension = plan.script_extension.unwrap_or("cmd");
        let path = screen_shell_script_path(extension);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create screen shell script dir: {err}"))?;
        }
        fs::write(&path, body)
            .map_err(|err| format!("failed to write screen shell script: {err}"))?;
        shell.arg(path);
    }
    Ok(shell)
}

fn screen_shell_script_path(extension: &str) -> std::path::PathBuf {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    std::env::temp_dir()
        .join("robocode-screen-shell")
        .join(format!("screen-launch-{millis}.{extension}"))
}

#[derive(Debug, Clone)]
struct ScreenLaunchContext {
    screen: String,
    title: String,
    role: String,
    display: String,
    display_index: String,
    provider: String,
    model: String,
    theme: String,
    cwd: std::path::PathBuf,
    binary: std::path::PathBuf,
    args: String,
}

fn screen_launch_context(screen: &str, state: &TuiState) -> Result<ScreenLaunchContext, String> {
    let binary = env::current_exe().map_err(|err| err.to_string())?;
    let cwd = if state.workspace.root.is_dir() {
        state.workspace.root.clone()
    } else {
        env::current_dir().map_err(|err| err.to_string())?
    };
    let (title, role, display, display_index) = match screen {
        "side-1" => ("Agent lanes", "lanes", "secondary", "1"),
        "side-2" => ("Workspace ops", "ops", "tertiary", "2"),
        _ => ("Companion", "side", "secondary", "1"),
    };
    let args = [
        "--tui-screen".to_string(),
        screen.to_string(),
        "--provider".to_string(),
        state.provider.clone(),
        "--model".to_string(),
        state.model.clone(),
        "--tui-theme".to_string(),
        state.theme_name.clone(),
    ]
    .into_iter()
    .map(|arg| shell_quote(&arg))
    .collect::<Vec<_>>()
    .join(" ");
    Ok(ScreenLaunchContext {
        screen: screen.to_string(),
        title: title.to_string(),
        role: role.to_string(),
        display: display.to_string(),
        display_index: display_index.to_string(),
        provider: state.provider.clone(),
        model: state.model.clone(),
        theme: state.theme_name.clone(),
        cwd,
        binary,
        args,
    })
}

fn expand_launch_template(template: &str, launch: &ScreenLaunchContext) -> String {
    let cwd = launch.cwd.to_string_lossy().to_string();
    let binary = launch.binary.to_string_lossy().to_string();
    template
        .replace("{screen:q}", &shell_quote(&launch.screen))
        .replace("{title:q}", &shell_quote(&launch.title))
        .replace("{role:q}", &shell_quote(&launch.role))
        .replace("{display:q}", &shell_quote(&launch.display))
        .replace("{display_index:q}", &shell_quote(&launch.display_index))
        .replace("{provider:q}", &shell_quote(&launch.provider))
        .replace("{model:q}", &shell_quote(&launch.model))
        .replace("{theme:q}", &shell_quote(&launch.theme))
        .replace("{cwd:q}", &shell_quote(&cwd))
        .replace("{binary:q}", &shell_quote(&binary))
        .replace("{args:q}", &shell_quote(&launch.args))
        .replace("{screen}", &launch.screen)
        .replace("{title}", &launch.title)
        .replace("{role}", &launch.role)
        .replace("{display}", &launch.display)
        .replace("{display_index}", &launch.display_index)
        .replace("{provider}", &launch.provider)
        .replace("{model}", &launch.model)
        .replace("{theme}", &launch.theme)
        .replace("{cwd}", &cwd)
        .replace("{binary}", &binary)
        .replace("{args}", &launch.args)
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn push_screen_list(state: &mut TuiState) {
    let mut rows = vec![
        "Tracked screens:".to_string(),
        "main active pid=self".to_string(),
    ];
    if state.screens.is_empty() {
        rows.push("side-1 off".to_string());
        rows.push("side-2 off".to_string());
    } else {
        rows.extend(state.screens.iter().map(|screen| {
            format!(
                "{} {} pid={} {}",
                screen.id,
                screen.status,
                screen
                    .pid
                    .map(|pid| pid.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                screen.summary
            )
        }));
    }
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: rows.join("\n"),
    });
}

fn close_companion_screen(state: &mut TuiState, screen: &str) {
    let Some(index) = state
        .screens
        .iter()
        .position(|candidate| candidate.id == screen)
    else {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Screen `{screen}` is not tracked. Use `/screen list`."),
        });
        return;
    };
    let closed = state.screens.remove(index);
    persist_screens(state);
    let stop_note = closed
        .pid
        .map(stop_companion_process)
        .unwrap_or_else(|| "no pid recorded".to_string());
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: format!("Closed screen `{}`. {stop_note}", closed.id),
    });
}

fn persist_screens(state: &mut TuiState) {
    let path = screen_store_path(&state.workspace.root);
    if let Err(err) = save_screens(&path, &state.screens) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to persist screen registry: {err}"),
        });
    }
}

#[cfg(unix)]
fn stop_companion_process(pid: u32) -> String {
    match Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
    {
        Ok(status) if status.success() => format!("Sent TERM to pid {pid}."),
        Ok(status) => format!("Tried TERM for pid {pid}; kill exited with {status}."),
        Err(err) => format!("Could not TERM pid {pid}: {err}."),
    }
}

#[cfg(not(unix))]
fn stop_companion_process(pid: u32) -> String {
    format!("Stop tracking pid {pid}; process termination is not implemented on this platform yet.")
}

fn push_screen_usage(state: &mut TuiState) {
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: screen_usage().to_string(),
    });
}

fn screen_usage() -> &'static str {
    "Usage: /screen main | /screen side-1 | /screen side-2 | /screen list | /screen close <side-1|side-2>"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn state() -> TuiState {
        TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
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
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        }
    }

    #[test]
    fn screen_command_launches_side_process_and_tracks_it() {
        let _env = screen_template_env("exit 0");
        let mut state = state();

        assert!(handle_screen_command("/screen side-2", &mut state));

        assert!(state.entries[0].body.contains("Launched screen `side-2`"));
        assert_eq!(state.screens.len(), 1);
        assert_eq!(state.screens[0].id, "side-2");
        assert_eq!(state.screens[0].title, "Workspace ops");
        assert_eq!(state.screens[0].status, "launched");
        assert!(state.screens[0].pid.is_some());
    }

    #[test]
    fn screen_command_rejects_duplicate_screen() {
        let _env = screen_template_env("exit 0");
        let mut state = state();

        assert!(handle_screen_command("/screen side-1", &mut state));
        assert!(handle_screen_command("/screen side", &mut state));

        assert_eq!(state.screens.len(), 1);
        assert!(state.entries[1].body.contains("already tracked"));
    }

    #[test]
    fn screen_command_lists_and_closes_tracked_screen() {
        let _env = screen_template_env("exit 0");
        let mut state = state();

        assert!(handle_screen_command("/screen side-1", &mut state));
        assert!(handle_screen_command("/screen list", &mut state));
        assert!(state.entries[1].body.contains("side-1 launched"));

        assert!(handle_screen_command("/screen close side-1", &mut state));

        assert!(state.screens.is_empty());
        assert!(state.entries[2].body.contains("Closed screen `side-1`"));
    }

    #[test]
    fn screen_command_reports_usage_for_unknown_screen() {
        let mut state = state();

        assert!(handle_screen_command("/screen other", &mut state));

        assert!(state.entries[0].body.contains("Usage: /screen"));
    }

    #[test]
    fn screen_command_does_not_capture_other_slash_commands() {
        let mut state = state();

        assert!(!handle_screen_command("/screenshots", &mut state));
        assert!(state.entries.is_empty());
    }

    #[test]
    fn side_screen_lanes_do_not_fall_back_to_preview_data() {
        let root = std::env::temp_dir().join(format!(
            "robocode-side-lanes-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let missing_store = root.join(".robocode").join("lanes.tsv");

        let lanes = load_side_lanes(Some(&missing_store));

        assert!(lanes.is_empty());
    }

    #[test]
    fn side_screen_registry_merges_current_screen_with_persisted_siblings() {
        let root = std::env::temp_dir().join(format!(
            "robocode-side-screen-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = screen_store_path(&root);
        save_screens(
            &path,
            &[CompanionScreen {
                id: "side-2".to_string(),
                title: "Workspace ops".to_string(),
                status: "launched".to_string(),
                pid: Some(202),
                summary: "persisted sibling".to_string(),
            }],
        )
        .expect("save sibling screen");

        let screens = load_side_screens(Some(&path), SideScreen::Lanes);

        assert!(screens.iter().any(|screen| screen.id == "side-1"
            && screen.status == "attached"
            && screen.pid == Some(std::process::id())));
        assert!(
            screens
                .iter()
                .any(|screen| screen.id == "side-2" && screen.summary == "persisted sibling")
        );
    }

    #[test]
    fn screen_launch_template_quotes_values() {
        let mut state = state();
        state.model = "deepseek v4 flash".to_string();
        let launch = screen_launch_context("side-1", &state).expect("launch context");

        let command = expand_launch_template(
            "open {screen:q} {model:q} --display {display:q} --cwd {cwd:q}",
            &launch,
        );

        assert!(command.contains("open 'side-1' 'deepseek v4 flash'"));
        assert!(command.contains("--display 'secondary'"));
        assert!(command.contains("--cwd "));
    }

    #[test]
    fn screen_launch_template_exposes_side_specific_context() {
        let state = state();
        let launch = screen_launch_context("side-2", &state).expect("launch context");

        let command = expand_launch_template(
            "{binary:q} {args} --role {role} --display-index {display_index}",
            &launch,
        );

        assert!(command.contains("--tui-screen"));
        assert!(command.contains("'side-2'"));
        assert!(command.contains("--role ops"));
        assert!(command.contains("--display-index 2"));
    }

    #[test]
    fn screen_shell_command_uses_script_for_long_templates() {
        let command = format!("printf ok\n# {}", "x".repeat(40 * 1024));
        let plan = screen_shell_command_plan(&command, false);

        assert_eq!(plan.program, "sh");
        assert!(plan.inline_args.is_empty());
        assert_eq!(plan.script_extension, Some("sh"));
        assert_eq!(
            plan.script_body.as_deref(),
            Some(format!("set -eu\n{command}\n").as_str())
        );
    }

    #[test]
    fn screen_shell_command_keeps_short_templates_inline() {
        let plan = screen_shell_command_plan("exit 0", false);

        assert_eq!(plan.program, "sh");
        assert_eq!(
            plan.inline_args,
            vec!["-lc".to_string(), "exit 0".to_string()]
        );
        assert!(plan.script_extension.is_none());
        assert!(plan.script_body.is_none());
    }

    fn screen_template_env(template: &'static str) -> ScopedEnv {
        ScopedEnv::set_many(&[
            ("ROBOCODE_SCREEN_LAUNCH_TEMPLATE", Some(template)),
            ("ROBOCODE_SCREEN_SIDE_1_LAUNCH_TEMPLATE", None),
            ("ROBOCODE_SCREEN_SIDE_2_LAUNCH_TEMPLATE", None),
        ])
    }

    struct ScopedEnv {
        previous: Vec<(&'static str, Option<String>)>,
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedEnv {
        fn set_many(values: &[(&'static str, Option<&str>)]) -> Self {
            let guard = env_lock().lock().expect("env test lock");
            let previous = values
                .iter()
                .map(|(key, _)| (*key, std::env::var(key).ok()))
                .collect::<Vec<_>>();
            for (key, value) in values {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
            Self {
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            for (key, previous) in &self.previous {
                unsafe {
                    if let Some(value) = previous {
                        std::env::set_var(key, value);
                    } else {
                        std::env::remove_var(key);
                    }
                }
            }
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }
}
