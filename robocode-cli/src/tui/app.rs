use std::{
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use robocode_core::SessionEngine;
use robocode_types::PermissionPrompt;

use super::command_palette::{
    close_on_escape, complete_selected, move_selection, reset_for_input_change,
    should_complete_on_enter,
};
use super::input::{close_focus_on_escape, prompt_for_tui_approval, should_exit};
use super::lane::{handle_tui_command, refresh_lanes};
use super::screen::handle_screen_command;
use super::state::{
    PendingTurn, ProviderOption, ProviderStatus, TuiEntry, TuiState, WorkspaceSnapshot,
    entry_from_event, lane_store_path, latest_lsp_diagnostics, load_lanes, load_screens,
    save_diagnostics, screen_store_path,
};
use super::terminal::TerminalGuard;

const BACKGROUND_DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(30);
const BACKGROUND_DIAGNOSTICS_PATH_LIMIT: usize = 4;

pub(crate) fn run_tui_with_theme(
    engine: &mut SessionEngine,
    startup_summary: &str,
    theme_name: Option<&str>,
) -> Result<(), String> {
    let mut terminal = TerminalGuard::enter_with_theme(theme_name)?;
    let lane_store = std::env::current_dir()
        .ok()
        .map(|root| lane_store_path(&root));
    let lanes = lane_store.as_deref().map(load_lanes).unwrap_or_default();
    let mut state = initial_state(
        engine,
        startup_summary,
        lane_store,
        lanes,
        terminal.theme_name(),
    );
    terminal.draw(&state)?;
    let mut background_diagnostics = None::<Receiver<Option<String>>>;
    let mut last_background_diagnostics = None::<Instant>;

    loop {
        poll_background_diagnostics(&mut state, &mut background_diagnostics);
        // Poll instead of blocking forever so background lane artifacts can
        // repaint completion, failure, and log-tail state without a keypress.
        if !event::poll(Duration::from_millis(750)).map_err(|err| err.to_string())? {
            refresh_lanes(&mut state);
            state.workspace.refresh_agent_jobs();
            maybe_start_background_diagnostics(
                engine,
                &state,
                &mut background_diagnostics,
                &mut last_background_diagnostics,
            );
            poll_background_diagnostics(&mut state, &mut background_diagnostics);
            terminal.draw(&state)?;
            continue;
        }
        let event = event::read().map_err(|err| err.to_string())?;
        let key = match event {
            Event::Key(key) => key,
            Event::Resize(_, _) => {
                terminal.draw(&state)?;
                continue;
            }
            _ => continue,
        };
        if close_focus_on_escape(key, &mut state) {
            terminal.draw(&state)?;
            continue;
        }
        if close_on_escape(key, &mut state) {
            terminal.draw(&state)?;
            continue;
        }
        if should_exit(key) {
            break;
        }
        match key.code {
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let theme_name = terminal.cycle_theme();
                state.theme_name = theme_name.to_string();
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: format!("Switched TUI theme to `{theme_name}`."),
                });
            }
            KeyCode::Up if !move_selection(&mut state, -1) => {
                continue;
            }
            KeyCode::Down if !move_selection(&mut state, 1) => {
                continue;
            }
            KeyCode::Tab if !complete_selected(&mut state) => {
                continue;
            }
            KeyCode::Enter => {
                if should_complete_on_enter(&state) {
                    complete_selected(&mut state);
                    terminal.draw(&state)?;
                    continue;
                }
                if handle_enter(engine, &mut state, &mut terminal)? {
                    break;
                }
            }
            KeyCode::Backspace => {
                state.input.pop();
                reset_for_input_change(&mut state);
            }
            KeyCode::Char(value) => {
                state.input.push(value);
                reset_for_input_change(&mut state);
            }
            _ => {}
        }
        terminal.draw(&state)?;
    }

    terminal.leave()
}

fn initial_state(
    engine: &SessionEngine,
    startup_summary: &str,
    lane_store: Option<std::path::PathBuf>,
    lanes: Vec<super::state::TerminalLane>,
    theme_name: &str,
) -> TuiState {
    let workspace = WorkspaceSnapshot::load_current();
    let screens = load_screens(&screen_store_path(&workspace.root));
    let tasks = engine.active_task_snapshot().unwrap_or_default();
    let memory = engine.memory_snapshot().unwrap_or_default();
    let provider_catalog = provider_catalog(engine);
    TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        provider_catalog,
        provider_status: ProviderStatus::from_telemetry(&engine.provider_telemetry()),
        theme_name: theme_name.to_string(),
        input: String::new(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        pending_turn: None,
        entries: vec![TuiEntry {
            label: "system".to_string(),
            body: format!(
                "RoboCode TUI ready. Enter submits. Esc or Ctrl-C exits.\n{startup_summary}"
            ),
        }],
        workspace,
        tasks,
        memory,
        screens,
        lanes,
        lane_store,
        focused_lane: None,
    }
}

fn handle_enter(
    engine: &mut SessionEngine,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    let input = state.input.trim().to_string();
    state.input.clear();
    if input.is_empty() {
        return Ok(false);
    }
    if is_exit_command(&input) {
        return Ok(true);
    }
    state.entries.push(TuiEntry {
        label: "user".to_string(),
        body: input.clone(),
    });
    if handle_tui_command(&input, state) || handle_screen_command(&input, state) {
        return Ok(false);
    }
    state.pending_turn = Some(PendingTurn::new(
        &state.session_id,
        &state.provider,
        &state.model,
        &input,
        &state.workspace.display_root,
    ));
    terminal.draw(state)?;
    let events = {
        let mut approver =
            |prompt: PermissionPrompt| prompt_for_tui_approval(prompt, state, terminal);
        engine.process_input_with_approval(&input, &mut approver)
    };
    state.pending_turn = None;
    let events = events?;
    state
        .entries
        .extend(events.into_iter().map(entry_from_event));
    refresh_diagnostics_cache(state);
    state.provider = engine.provider_name().to_string();
    state.model = engine.model_name().to_string();
    state.provider_catalog = provider_catalog(engine);
    state.provider_status = ProviderStatus::from_telemetry(&engine.provider_telemetry());
    state.tasks = engine.active_task_snapshot().unwrap_or_default();
    state.memory = engine.memory_snapshot().unwrap_or_default();
    state.workspace = WorkspaceSnapshot::load_current();
    Ok(false)
}

fn provider_catalog(engine: &SessionEngine) -> Vec<ProviderOption> {
    engine
        .provider_descriptors()
        .iter()
        .map(ProviderOption::from_descriptor)
        .collect()
}

fn maybe_start_background_diagnostics(
    engine: &SessionEngine,
    state: &TuiState,
    pending: &mut Option<Receiver<Option<String>>>,
    last_started: &mut Option<Instant>,
) {
    if pending.is_some() {
        return;
    }
    let now = Instant::now();
    if last_started
        .is_some_and(|started| now.duration_since(started) < BACKGROUND_DIAGNOSTICS_INTERVAL)
    {
        return;
    }
    let paths = background_diagnostic_paths(&state.workspace);
    if paths.is_empty() {
        return;
    }
    *last_started = Some(now);
    *pending = Some(engine.spawn_lsp_diagnostics_snapshot(paths));
}

fn background_diagnostic_paths(workspace: &WorkspaceSnapshot) -> Vec<String> {
    workspace
        .workspace_paths
        .iter()
        .filter(|path| path.ends_with(".rs"))
        .take(BACKGROUND_DIAGNOSTICS_PATH_LIMIT)
        .cloned()
        .collect()
}

fn poll_background_diagnostics(
    state: &mut TuiState,
    pending: &mut Option<Receiver<Option<String>>>,
) {
    let Some(receiver) = pending.take() else {
        return;
    };
    match receiver.try_recv() {
        Ok(Some(rendered)) => persist_rendered_diagnostics(state, &rendered),
        Ok(None) | Err(TryRecvError::Disconnected) => {}
        Err(TryRecvError::Empty) => *pending = Some(receiver),
    }
}

fn persist_rendered_diagnostics(state: &mut TuiState, rendered: &str) {
    let entry = TuiEntry {
        label: "command".to_string(),
        body: rendered.to_string(),
    };
    let Some(diagnostics) = latest_lsp_diagnostics(&[entry]) else {
        return;
    };
    if let Err(err) = save_diagnostics(&state.workspace.root, &diagnostics) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to persist background LSP diagnostics: {err}"),
        });
        return;
    }
    state.workspace = WorkspaceSnapshot::load(state.workspace.root.clone());
}

fn refresh_diagnostics_cache(state: &mut TuiState) {
    let Some(diagnostics) = latest_lsp_diagnostics(&state.entries) else {
        return;
    };
    if let Err(err) = save_diagnostics(&state.workspace.root, &diagnostics) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to persist LSP diagnostics: {err}"),
        });
    }
}

fn is_exit_command(input: &str) -> bool {
    matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "exit" | "quit" | "/exit" | "/quit"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        background_diagnostic_paths, is_exit_command, persist_rendered_diagnostics,
        refresh_diagnostics_cache,
    };
    use crate::tui::state::{ProviderStatus, TerminalLane, WorkspaceSnapshot};
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn exit_command_accepts_slash_aliases() {
        assert!(is_exit_command("exit"));
        assert!(is_exit_command("quit"));
        assert!(is_exit_command("/exit"));
        assert!(is_exit_command("/quit"));
        assert!(is_exit_command(" /QUIT "));
        assert!(!is_exit_command("/help"));
    }

    #[test]
    fn refresh_diagnostics_cache_persists_latest_lsp_output() {
        let root = temp_app_root();
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.root = root.clone();
        let mut state = super::TuiState {
            session_id: "session".to_string(),
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
            entries: vec![super::TuiEntry {
                label: "command".to_string(),
                body: "LSP diagnostics:\nsrc/main.rs:\n  1:2 error [fake/E1] broken".to_string(),
            }],
            workspace,
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::<TerminalLane>::new(),
            lane_store: None,
            focused_lane: None,
        };

        refresh_diagnostics_cache(&mut state);
        let persisted = fs::read_to_string(root.join(".robocode").join("diagnostics.txt"))
            .expect("diagnostics cache");

        assert!(persisted.contains("src/main.rs:1:2 error [fake/E1] broken"));
    }

    #[test]
    fn background_diagnostic_paths_prefers_rust_workspace_files() {
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.workspace_paths = vec![
            "README.md".to_string(),
            "src/lib.rs".to_string(),
            "src/main.rs".to_string(),
            "tests/config_tests.rs".to_string(),
            "Cargo.toml".to_string(),
            "src/config.rs".to_string(),
            "examples/demo.rs".to_string(),
        ];

        let paths = background_diagnostic_paths(&workspace);

        assert_eq!(
            paths,
            vec![
                "src/lib.rs",
                "src/main.rs",
                "tests/config_tests.rs",
                "src/config.rs"
            ]
        );
    }

    #[test]
    fn persist_rendered_diagnostics_updates_workspace_cache() {
        let root = temp_app_root();
        let mut workspace = WorkspaceSnapshot::fixture();
        workspace.root = root.clone();
        let mut state = super::TuiState {
            session_id: "session".to_string(),
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
            entries: Vec::new(),
            workspace,
            tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::<TerminalLane>::new(),
            lane_store: None,
            focused_lane: None,
        };

        persist_rendered_diagnostics(
            &mut state,
            "LSP diagnostics:\nsrc/lib.rs:\n  3:1 warning [fake/W1] note",
        );

        assert_eq!(
            state.workspace.diagnostics,
            vec!["src/lib.rs:3:1 warning [fake/W1] note".to_string()]
        );
    }

    fn temp_app_root() -> std::path::PathBuf {
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("robocode-tui-app-test-{nanos}-{suffix}"));
        fs::create_dir_all(&root).expect("temp root");
        root
    }
}
