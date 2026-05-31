use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use robocode_core::SessionEngine;
use robocode_model::ModelRequestControl;
use robocode_types::{ApprovalResponse, PermissionPrompt};

use super::command_palette::{
    close_on_escape, command_suggestion_index_at, complete_selected, move_selection,
    reset_for_input_change, select_suggestion_at, selected_command, should_complete_on_enter,
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
const ACTIVE_TURN_REPAINT_INTERVAL: Duration = Duration::from_millis(150);

enum ProviderTurnEvent {
    Approval {
        prompt: PermissionPrompt,
        response: Sender<robocode_types::ApprovalResponse>,
    },
}

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
            Event::Mouse(mouse) => {
                if handle_mouse(mouse, &mut state) {
                    terminal.draw(&state)?;
                }
                continue;
            }
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
            KeyCode::Char('f')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && favorite_selected_model(engine, &mut state)? =>
            {
                terminal.draw(&state)?;
                continue;
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
            _ if is_send_key(key) => {
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
            _ if apply_composer_shortcut(key, &mut state) => {}
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

fn favorite_selected_model(
    engine: &mut SessionEngine,
    state: &mut TuiState,
) -> Result<bool, String> {
    if !state.input.trim_start().starts_with("/models") {
        return Ok(false);
    }
    let Some(suggestion) = selected_command(state) else {
        return Ok(false);
    };
    let Some((provider_id, model)) = parse_models_command(&suggestion.command) else {
        return Ok(false);
    };
    let command = format!("/settings provider {provider_id} favorite-model {model}");
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let events = engine.process_input_with_approval(&command, &mut approver)?;
    state.entries.push(TuiEntry {
        label: "settings".to_string(),
        body: format!(
            "Favorited `{model}` for `{provider_id}`. It will appear first in `/models`."
        ),
    });
    state
        .entries
        .extend(events.into_iter().map(entry_from_event));
    state.provider_catalog = provider_catalog(engine);
    state.command_selection = 0;
    Ok(true)
}

fn parse_models_command(command: &str) -> Option<(String, String)> {
    let mut words = command.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some("/models"), Some(provider), Some(model)) => {
            Some((provider.to_string(), model.to_string()))
        }
        _ => None,
    }
}

fn handle_mouse(mouse: MouseEvent, state: &mut TuiState) -> bool {
    if !matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
    ) {
        return false;
    }
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let Some(index) = command_suggestion_index_at(state, mouse.column, mouse.row, width, height)
    else {
        return false;
    };
    let selected = select_suggestion_at(state, index);
    if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
        complete_selected(state);
    }
    selected
}

fn is_send_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Enter
        || (key.code == KeyCode::Char('j') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn apply_composer_shortcut(key: KeyEvent, state: &mut TuiState) -> bool {
    match key.code {
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.input.clear();
            reset_for_input_change(state);
            true
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(input) = last_user_input(state) {
                state.input = input;
                reset_for_input_change(state);
                true
            } else {
                false
            }
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.input = "/task add ".to_string();
            reset_for_input_change(state);
            true
        }
        KeyCode::Char('?') if key.modifiers.is_empty() && state.input.is_empty() => {
            state.input = "/help ".to_string();
            reset_for_input_change(state);
            true
        }
        _ => false,
    }
}

fn last_user_input(state: &TuiState) -> Option<String> {
    state
        .entries
        .iter()
        .rev()
        .find(|entry| entry.label == "user" && !entry.body.trim().is_empty())
        .map(|entry| entry.body.trim().to_string())
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
    let mut entries = vec![TuiEntry {
        label: "system".to_string(),
        body: format!("RoboCode TUI ready. Enter submits. Esc or Ctrl-C exits.\n{startup_summary}"),
    }];
    let first_run_setup = first_run_setup_entry(engine, startup_summary, &provider_catalog);
    if let Some(entry) = first_run_setup.clone() {
        entries.push(entry);
    }
    let input = if first_run_setup.is_some() {
        "/setup".to_string()
    } else {
        String::new()
    };
    TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        provider_catalog,
        provider_status: ProviderStatus::from_telemetry(&engine.provider_telemetry()),
        theme_name: theme_name.to_string(),
        input,
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        pending_turn: None,
        entries,
        workspace,
        tasks,
        runtime_tasks: engine.agent_task_snapshot(),
        memory,
        screens,
        lanes,
        lane_store,
        focused_lane: None,
    }
}

fn first_run_setup_entry(
    engine: &SessionEngine,
    startup_summary: &str,
    provider_catalog: &[ProviderOption],
) -> Option<TuiEntry> {
    if engine.provider_name() == "fallback" || !startup_summary.contains("key=missing") {
        return None;
    }
    let default_model = provider_catalog
        .iter()
        .find(|provider| provider.provider_id == engine.provider_name())
        .and_then(|provider| provider.default_model.as_deref())
        .unwrap_or(engine.model_name());
    Some(TuiEntry {
        label: "setup".to_string(),
        body: format!(
            "First-run setup: `{}` is selected but no API key is detected.\nOpen `/setup` for the interactive provider/model flow, use `/setup provider deepseek {default_model}` for the DeepSeek default, or set the matching API key env var. Offline escape hatch: `/setup provider fallback test-local`.",
            engine.provider_name()
        ),
    })
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
    if handle_local_setting_command(&input, state, terminal)?
        || handle_tui_command(&input, state)
        || handle_screen_command(&input, state)
    {
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
    let events = run_provider_turn_interactive(engine, &input, state, terminal)?;
    state.pending_turn = None;
    state
        .entries
        .extend(events.into_iter().map(entry_from_event));
    refresh_diagnostics_cache(state);
    state.provider = engine.provider_name().to_string();
    state.model = engine.model_name().to_string();
    state.provider_catalog = provider_catalog(engine);
    state.provider_status = ProviderStatus::from_telemetry(&engine.provider_telemetry());
    state.tasks = engine.active_task_snapshot().unwrap_or_default();
    state.runtime_tasks = engine.agent_task_snapshot();
    state.memory = engine.memory_snapshot().unwrap_or_default();
    state.workspace = WorkspaceSnapshot::load_current();
    Ok(false)
}

fn handle_local_setting_command(
    input: &str,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    let mut parts = input.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("/theme"), Some(theme_name)) => {
            apply_theme_command(theme_name, state, terminal)?;
            Ok(true)
        }
        (Some("/settings" | "/setup"), Some("theme")) => {
            if let Some(theme_name) = parts.next() {
                apply_theme_command(theme_name, state, terminal)?;
            } else {
                state.entries.push(TuiEntry {
                    label: "settings".to_string(),
                    body: format!(
                        "Theme picker: type `/settings theme <name>`. Available: {}",
                        crate::tui::theme_names().join(", ")
                    ),
                });
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_theme_command(
    theme_name: &str,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<(), String> {
    let applied = terminal.set_theme(theme_name)?;
    state.theme_name = applied.to_string();
    state.entries.push(TuiEntry {
        label: "settings".to_string(),
        body: format!("TUI theme set to `{applied}`."),
    });
    Ok(())
}

fn run_provider_turn_interactive(
    engine: &mut SessionEngine,
    input: &str,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<Vec<robocode_core::EngineEvent>, String> {
    let (event_sender, event_receiver) = mpsc::channel::<ProviderTurnEvent>();
    let (result_sender, result_receiver) =
        mpsc::channel::<Result<Vec<robocode_core::EngineEvent>, String>>();
    let control = ModelRequestControl::new();

    std::thread::scope(|scope| {
        let worker_control = control.clone();
        let worker_input = input.to_string();
        scope.spawn(move || {
            let mut approver = |prompt: PermissionPrompt| {
                let (response_sender, response_receiver) = mpsc::channel();
                let event = ProviderTurnEvent::Approval {
                    prompt,
                    response: response_sender,
                };
                if event_sender.send(event).is_err() {
                    return robocode_types::ApprovalResponse {
                        approved: false,
                        feedback: Some("TUI approval channel closed".to_string()),
                    };
                }
                response_receiver
                    .recv()
                    .unwrap_or(robocode_types::ApprovalResponse {
                        approved: false,
                        feedback: Some("TUI approval response channel closed".to_string()),
                    })
            };
            let result = engine.process_input_with_approval_and_control(
                &worker_input,
                &mut approver,
                &worker_control,
            );
            let _ = result_sender.send(result);
        });

        loop {
            if let Some(result) = poll_provider_turn_result(&result_receiver)? {
                return Ok(result);
            }
            poll_provider_turn_events(&event_receiver, state, terminal)?;
            if let Some(result) = poll_provider_turn_result(&result_receiver)? {
                return Ok(result);
            }
            poll_active_turn_input(&control, state, terminal)?;
            refresh_lanes(state);
            state.workspace.refresh_agent_jobs();
            terminal.draw(state)?;
        }
    })
}

fn poll_provider_turn_result(
    receiver: &Receiver<Result<Vec<robocode_core::EngineEvent>, String>>,
) -> Result<Option<Vec<robocode_core::EngineEvent>>, String> {
    match receiver.try_recv() {
        Ok(result) => result.map(Some),
        Err(TryRecvError::Empty) => Ok(None),
        Err(TryRecvError::Disconnected) => Err("provider turn worker stopped unexpectedly".into()),
    }
}

fn poll_provider_turn_events(
    receiver: &Receiver<ProviderTurnEvent>,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<(), String> {
    loop {
        match receiver.try_recv() {
            Ok(ProviderTurnEvent::Approval { prompt, response }) => {
                mark_pending_turn_waiting_for_approval(state, &prompt);
                let approval = prompt_for_tui_approval(prompt, state, terminal);
                let _ = response.send(approval);
                mark_pending_turn_waiting_for_provider(state);
                terminal.draw(state)?;
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => return Ok(()),
        }
    }
}

fn poll_active_turn_input(
    control: &ModelRequestControl,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<(), String> {
    if !event::poll(ACTIVE_TURN_REPAINT_INTERVAL).map_err(|err| err.to_string())? {
        return Ok(());
    }
    match event::read().map_err(|err| err.to_string())? {
        Event::Key(key) => handle_active_turn_key(key, control, state, terminal),
        Event::Mouse(mouse) => {
            if handle_mouse(mouse, state) {
                terminal.draw(state)?;
            }
            Ok(())
        }
        Event::Resize(_, _) => terminal.draw(state),
        _ => Ok(()),
    }
}

fn handle_active_turn_key(
    key: KeyEvent,
    control: &ModelRequestControl,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<(), String> {
    if close_focus_on_escape(key, state) || close_on_escape(key, state) {
        return terminal.draw(state);
    }
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            control.cancel();
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: "Cancellation requested for the active provider turn. The current provider may finish its in-flight request before stopping.".to_string(),
            });
        }
        KeyCode::Esc => {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: "Provider turn is active; keeping the cockpit open until the turn finishes."
                    .to_string(),
            });
        }
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let theme_name = terminal.cycle_theme();
            state.theme_name = theme_name.to_string();
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!("Switched TUI theme to `{theme_name}`."),
            });
        }
        KeyCode::Up if move_selection(state, -1) => {}
        KeyCode::Down if move_selection(state, 1) => {}
        KeyCode::Tab if complete_selected(state) => {}
        _ if is_send_key(key) => {
            if should_complete_on_enter(state) {
                complete_selected(state);
            } else if !state.input.trim().is_empty() {
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body:
                        "Provider turn is still running; draft kept in composer for the next turn."
                            .to_string(),
                });
            }
        }
        KeyCode::Backspace => {
            state.input.pop();
            reset_for_input_change(state);
        }
        _ if apply_composer_shortcut(key, state) => {}
        KeyCode::Char(value) => {
            state.input.push(value);
            reset_for_input_change(state);
        }
        _ => {}
    }
    terminal.draw(state)
}

fn mark_pending_turn_waiting_for_approval(state: &mut TuiState, prompt: &PermissionPrompt) {
    if let Some(turn) = state.pending_turn.as_mut() {
        turn.phase = format!("Waiting for approval: {}", prompt.tool_name);
        turn.next_action = "approve / deny".to_string();
    }
}

fn mark_pending_turn_waiting_for_provider(state: &mut TuiState) {
    if let Some(turn) = state.pending_turn.as_mut() {
        turn.phase = "Waiting for provider response".to_string();
        turn.next_action = "wait".to_string();
    }
}

fn provider_catalog(engine: &SessionEngine) -> Vec<ProviderOption> {
    let ui_config = robocode_config::load_provider_ui_configs(engine.cwd()).unwrap_or_default();
    engine
        .provider_descriptors()
        .iter()
        .map(|descriptor| {
            let mut option = ProviderOption::from_descriptor(descriptor);
            if let Some(config) = ui_config.get(&option.provider_id) {
                if config.api_base.is_some() {
                    option.default_api_base = config.api_base.clone();
                }
                if config.api_key_env.is_some() {
                    option.api_key_env = config.api_key_env.clone();
                }
                if config.default_model.is_some() {
                    option.default_model = config.default_model.clone();
                }
                option.enabled_models = config.models.clone();
                option.favorite_models = config.favorite_models.clone();
            }
            if option.enabled_models.is_empty()
                && let Some(default_model) = option.default_model.clone()
            {
                option.enabled_models.push(default_model);
            }
            option
        })
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
        apply_composer_shortcut, background_diagnostic_paths, initial_state, is_exit_command,
        is_send_key, last_user_input, persist_rendered_diagnostics, refresh_diagnostics_cache,
    };
    use crate::tui::state::{ProviderStatus, TerminalLane, WorkspaceSnapshot};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use robocode_core::SessionEngine;
    use robocode_model::ModelProvider;
    use robocode_types::{ModelEvent, ModelRequest};
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
    fn composer_shortcuts_match_advertised_actions() {
        let mut state = test_state("draft input");
        state.entries.push(super::TuiEntry {
            label: "user".to_string(),
            body: "previous task".to_string(),
        });

        assert!(is_send_key(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::CONTROL
        )));
        assert!(apply_composer_shortcut(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            &mut state
        ));
        assert_eq!(state.input, "");
        assert!(apply_composer_shortcut(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            &mut state
        ));
        assert_eq!(state.input, "previous task");
        assert!(apply_composer_shortcut(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut state
        ));
        assert_eq!(state.input, "/task add ");
    }

    #[test]
    fn question_mark_opens_help_only_from_empty_composer() {
        let mut empty = test_state("");
        assert!(apply_composer_shortcut(
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()),
            &mut empty
        ));
        assert_eq!(empty.input, "/help ");

        let mut typing = test_state("what");
        assert!(!apply_composer_shortcut(
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()),
            &mut typing
        ));
        assert_eq!(typing.input, "what");
    }

    #[test]
    fn initial_state_preloads_setup_when_online_provider_key_is_missing() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(TestProvider {
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();

        let state = initial_state(
            &engine,
            "provider=deepseek model=deepseek-v4-flash key=missing",
            None,
            Vec::new(),
            "aurora-cyan",
        );

        assert_eq!(state.input, "/setup");
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.label == "setup" && entry.body.contains("First-run setup"))
        );
    }

    #[test]
    fn initial_state_does_not_preload_setup_for_fallback_provider() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(TestProvider {
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();

        let state = initial_state(
            &engine,
            "provider=fallback model=test-local key=missing",
            None,
            Vec::new(),
            "aurora-cyan",
        );

        assert_eq!(state.input, "");
        assert!(!state.entries.iter().any(|entry| entry.label == "setup"));
    }

    #[test]
    fn regenerate_uses_latest_user_turn() {
        let mut state = test_state("");
        assert_eq!(last_user_input(&state), None);
        state.entries.push(super::TuiEntry {
            label: "assistant".to_string(),
            body: "answer".to_string(),
        });
        state.entries.push(super::TuiEntry {
            label: "user".to_string(),
            body: "  retry this  ".to_string(),
        });

        assert_eq!(last_user_input(&state), Some("retry this".to_string()));
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
            runtime_tasks: Vec::new(),
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
            runtime_tasks: Vec::new(),
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

    fn test_state(input: &str) -> super::TuiState {
        super::TuiState {
            session_id: "session".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: input.to_string(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            pending_turn: None,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::<TerminalLane>::new(),
            lane_store: None,
            focused_lane: None,
        }
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

    struct TestProvider {
        provider: String,
        model: String,
    }

    impl ModelProvider for TestProvider {
        fn provider_name(&self) -> &str {
            &self.provider
        }

        fn model(&self) -> &str {
            &self.model
        }

        fn set_model(&mut self, model: String) {
            self.model = model;
        }

        fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
            Ok(Vec::new())
        }
    }
}
