use std::time::{Duration, Instant};

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use viden_core::{RuntimeSupervisor, SessionEngine};
use viden_types::{
    ApprovalRequestView, ApprovalResponse, RuntimeCommand, RuntimeEvent, RuntimeEventKind,
    RuntimeViewState,
};

use super::command_palette::{
    close_on_escape, command_suggestion_index_at, complete_selected, move_selection,
    reset_for_input_change, select_suggestion_at, selected_command, should_complete_on_enter,
};
use super::input::{
    ApprovalKeyEffect, apply_approval_action, apply_approval_key, close_focus_on_escape,
    should_exit,
};
use super::lane::{handle_tui_command, refresh_lanes};
use super::modal::{
    DEFAULT_APPROVAL_FOCUS, approval_action_at, interaction_panel_index_at,
    set_approval_focus_for_action,
};
use super::screen::handle_screen_command;
use super::state::{
    InteractionPanel, PendingTurn, ProviderOption, ProviderStatus, TuiEntry, TuiState,
    WorkspaceSnapshot, lane_store_path, latest_lsp_diagnostics, load_lanes, load_screens,
    save_diagnostics, screen_store_path,
};
use super::terminal::TerminalGuard;

const BACKGROUND_DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(30);
const BACKGROUND_DIAGNOSTICS_PATH_LIMIT: usize = 4;

struct RuntimeClient {
    supervisor: RuntimeSupervisor,
    view_state: RuntimeViewState,
    next_command: u64,
    active_turn_requested: bool,
}

impl RuntimeClient {
    fn start(engine: SessionEngine) -> Self {
        let view_state = engine.runtime_view_state();
        let supervisor = RuntimeSupervisor::start(engine);
        Self {
            supervisor,
            view_state,
            next_command: 1,
            active_turn_requested: false,
        }
    }

    fn start_provider_turn(&mut self, input: String) -> Result<(), String> {
        if self.is_turn_active() {
            return Err("provider turn is already active".to_string());
        }
        self.send_text_input(input)?;
        self.active_turn_requested = true;
        Ok(())
    }

    fn send_text_input(&mut self, input: String) -> Result<String, String> {
        let command_id = self.next_command_id("input");
        self.supervisor.send_command(
            command_id.clone(),
            RuntimeCommand::SubmitUserInput { content: input },
        )?;
        Ok(command_id)
    }

    fn send_command(&mut self, command: RuntimeCommand) -> Result<String, String> {
        let command_id = self.next_command_id("cmd");
        self.supervisor.send_command(command_id.clone(), command)?;
        Ok(command_id)
    }

    fn cancel_active_turn(&mut self) {
        let _ = self.send_command(RuntimeCommand::CancelActiveTurn);
    }

    fn respond_to_approval(&mut self, request_id: String, response: ApprovalResponse) {
        let _ = self.send_command(RuntimeCommand::RespondToApproval {
            request_id,
            response,
        });
    }

    fn is_turn_active(&self) -> bool {
        self.active_turn_requested || self.supervisor.is_turn_active()
    }

    fn poll_event(&mut self) -> Option<RuntimeEvent> {
        let event = self.supervisor.try_recv_event()?;
        self.view_state.apply_event(&event);
        Some(event)
    }

    fn note_turn_finished(&mut self) {
        self.active_turn_requested = false;
        self.view_state.assistant_stream.clear();
    }

    fn next_command_id(&mut self, prefix: &str) -> String {
        let id = self.next_command;
        self.next_command += 1;
        format!("{prefix}-{id}")
    }
}

struct ActiveApproval {
    approval: ApprovalRequestView,
}

pub fn run_tui_with_theme(
    engine: SessionEngine,
    startup_summary: &str,
    theme_name: Option<&str>,
) -> Result<(), String> {
    let mut terminal = TerminalGuard::enter_with_theme(theme_name)?;
    let lane_store = std::env::current_dir()
        .ok()
        .map(|root| lane_store_path(&root));
    let lanes = lane_store.as_deref().map(load_lanes).unwrap_or_default();
    let mut state = initial_state(
        &engine,
        startup_summary,
        lane_store,
        lanes,
        terminal.theme_name(),
    );
    let mut runtime = RuntimeClient::start(engine);
    terminal.draw(&state)?;
    let mut last_background_diagnostics = None::<Instant>;
    let mut active_approval = None::<ActiveApproval>;

    loop {
        if poll_turn_controller_events(
            &mut runtime,
            &mut state,
            &mut active_approval,
            &mut terminal,
        )? {
            break;
        }
        // Poll instead of blocking forever so background lane artifacts can
        // repaint completion, failure, and log-tail state without a keypress.
        if !event::poll(Duration::from_millis(750)).map_err(|err| err.to_string())? {
            if poll_turn_controller_events(
                &mut runtime,
                &mut state,
                &mut active_approval,
                &mut terminal,
            )? {
                break;
            }
            refresh_lanes(&mut state);
            state.workspace.refresh_agent_jobs();
            maybe_start_background_diagnostics(
                &mut runtime,
                &state,
                &mut last_background_diagnostics,
            );
            terminal.draw(&state)?;
            continue;
        }
        let event = event::read().map_err(|err| err.to_string())?;
        let key = match event {
            Event::Key(key) => key,
            Event::Mouse(mouse) => {
                if handle_active_approval_mouse(
                    mouse,
                    &mut active_approval,
                    &mut runtime,
                    &mut state,
                    &mut terminal,
                )? {
                    terminal.draw(&state)?;
                    continue;
                }
                if handle_interaction_panel_mouse(&mut runtime, mouse, &mut state)? {
                    terminal.draw(&state)?;
                    continue;
                }
                if handle_mouse(mouse, &mut state) {
                    terminal.draw(&state)?;
                }
                continue;
            }
            Event::Resize(_, _) => {
                terminal.draw(&state)?;
                continue;
            }
            event if event_requires_repaint(&event) => {
                terminal.draw(&state)?;
                continue;
            }
            _ => continue,
        };
        if handle_active_approval_key(
            key,
            &mut active_approval,
            &mut runtime,
            &mut state,
            &mut terminal,
        )? {
            terminal.draw(&state)?;
            continue;
        }
        if state.interaction_panel.is_some() {
            handle_interaction_panel_key(&mut runtime, key, &mut state)?;
            terminal.draw(&state)?;
            continue;
        }
        if close_focus_on_escape(key, &mut state) {
            terminal.draw(&state)?;
            continue;
        }
        if close_on_escape(key, &mut state) {
            terminal.draw(&state)?;
            continue;
        }
        if runtime.is_turn_active()
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            runtime.cancel_active_turn();
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: "Cancellation requested for the active provider turn. The current provider may finish its in-flight request before stopping.".to_string(),
            });
            terminal.draw(&state)?;
            continue;
        }
        if runtime.is_turn_active() && key.code == KeyCode::Esc {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: "Provider turn is active; keeping the cockpit open until the turn finishes."
                    .to_string(),
            });
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
                    && favorite_selected_model(&mut runtime, &mut state)? =>
            {
                terminal.draw(&state)?;
                continue;
            }
            KeyCode::PageUp => scroll_transcript(&mut state, 12),
            KeyCode::PageDown => scroll_transcript(&mut state, -12),
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.transcript_scroll = usize::MAX / 2;
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.transcript_scroll = 0;
            }
            KeyCode::Up if !move_selection(&mut state, -1) => continue,
            KeyCode::Down if !move_selection(&mut state, 1) => continue,
            KeyCode::Tab if !complete_selected(&mut state) => {
                continue;
            }
            _ if is_send_key(key) => {
                if should_complete_on_enter(&state) {
                    complete_selected(&mut state);
                    terminal.draw(&state)?;
                    continue;
                }
                if handle_enter(&mut runtime, &mut state, &mut terminal)? {
                    break;
                }
            }
            KeyCode::Backspace => {
                state.input.pop();
                reset_for_input_change(&mut state);
            }
            _ if apply_composer_shortcut(key, &mut state) => {}
            KeyCode::Char(value) => push_composer_char(&mut state, value),
            _ => {}
        }
        terminal.draw(&state)?;
    }

    terminal.leave()
}

fn favorite_selected_model(
    runtime: &mut RuntimeClient,
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
    if runtime.is_turn_active() {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: "A provider turn is active. Finish or cancel it before changing favorite models."
                .to_string(),
        });
        return Ok(true);
    }
    run_settings_command(runtime, state, &command)?;
    state.entries.push(TuiEntry {
        label: "settings".to_string(),
        body: format!(
            "Favorited `{model}` for `{provider_id}`. It will appear first in `/models`."
        ),
    });
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
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            scroll_transcript(state, 4);
            return true;
        }
        MouseEventKind::ScrollDown => {
            scroll_transcript(state, -4);
            return true;
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {}
        _ => return false,
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

fn scroll_transcript(state: &mut TuiState, delta: isize) {
    if delta > 0 {
        state.transcript_scroll = state.transcript_scroll.saturating_add(delta as usize);
    } else {
        state.transcript_scroll = state.transcript_scroll.saturating_sub(delta.unsigned_abs());
    }
}

fn handle_interaction_panel_mouse(
    runtime: &mut RuntimeClient,
    mouse: MouseEvent,
    state: &mut TuiState,
) -> Result<bool, String> {
    if state.interaction_panel.is_none()
        || !matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
        )
    {
        return Ok(false);
    }
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let Some(index) = interaction_panel_index_at(state, mouse.column, mouse.row, width, height, 38)
    else {
        return Ok(false);
    };
    set_interaction_panel_selected(state, index);
    if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
        apply_interaction_panel_selection(runtime, state)?;
    }
    Ok(true)
}

fn handle_interaction_panel_key(
    runtime: &mut RuntimeClient,
    key: KeyEvent,
    state: &mut TuiState,
) -> Result<(), String> {
    match key.code {
        KeyCode::Esc => state.interaction_panel = None,
        KeyCode::Up => move_interaction_selection(state, -1),
        KeyCode::Down => move_interaction_selection(state, 1),
        KeyCode::Enter => apply_interaction_panel_selection(runtime, state)?,
        KeyCode::Backspace => edit_interaction_panel_text(state, None),
        KeyCode::Char(value)
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            edit_interaction_panel_text(state, Some(value));
        }
        _ => {}
    }
    Ok(())
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

fn push_composer_char(state: &mut TuiState, value: char) {
    state.input.push(value);
    // Some terminals can leak SGR mouse/color escape tails as printable chars;
    // clear those protocol residues instead of rendering them in the composer.
    if looks_like_terminal_escape_residue(&state.input) {
        state.input.clear();
    }
    reset_for_input_change(state);
}

fn looks_like_terminal_escape_residue(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.len() < 6 || !(trimmed.ends_with('m') || trimmed.ends_with('M')) {
        return false;
    }
    let body = trimmed[..trimmed.len() - 1].trim_start_matches(['\u{1b}', '[', '<', '?']);
    let mut parts = body.split(';').collect::<Vec<_>>();
    if parts.len() < 3 || parts.len() > 5 {
        return false;
    }
    if parts[0].is_empty() {
        parts.remove(0);
    }
    let all_numeric = parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()));
    all_numeric && parts.len() >= 3
}

fn event_requires_repaint(event: &Event) -> bool {
    matches!(
        event,
        Event::FocusGained | Event::FocusLost | Event::Paste(_)
    )
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
    let mut provider_status = ProviderStatus::from_telemetry(&engine.provider_telemetry());
    provider_status.work_mode = engine.work_mode();
    provider_status.permission_level = engine.permission_level();
    let entries = vec![TuiEntry {
        label: "system".to_string(),
        body: format!("Viden TUI ready. Enter submits. Esc or Ctrl-C exits.\n{startup_summary}"),
    }];
    TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        provider_catalog,
        provider_status,
        theme_name: theme_name.to_string(),
        input: String::new(),
        command_selection: 0,
        command_palette_hidden_for: None,
        approval_focus: 0,
        approval_apply_all: false,
        pending_turn: None,
        streaming_assistant: None,
        transcript_scroll: 0,
        entries,
        workspace,
        tasks,
        runtime_tasks: engine.agent_task_snapshot(),
        memory,
        screens,
        lanes,
        lane_store,
        focused_lane: None,
        interaction_panel: None,
    }
}

fn handle_enter(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    let input = state.input.trim().to_string();
    state.input.clear();
    handle_submitted_input(runtime, state, terminal, input)
}

fn handle_submitted_input(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
    input: String,
) -> Result<bool, String> {
    if input.is_empty() {
        return Ok(false);
    }
    if is_exit_command(&input) {
        return Ok(true);
    }
    if state.pending_turn.is_some() {
        if handle_local_setting_command(&input, state, terminal)?
            || handle_tui_command(&input, state)
            || handle_screen_command(&input, state)
        {
            return Ok(false);
        }
        match active_turn_input_intent(&input) {
            ActiveTurnInputIntent::Cancel => {
                runtime.cancel_active_turn();
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: "Cancellation requested for the active provider turn. The composer stays available for your next prompt.".to_string(),
                });
            }
            ActiveTurnInputIntent::ImmediateCommand => {
                handle_immediate_runtime_command(runtime, state, &input)?;
            }
            ActiveTurnInputIntent::Command => {
                state.input = input;
                reset_for_input_change(state);
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: "Command kept in the composer while the active turn runs. Press Enter after it finishes, or type /cancel to stop the turn.".to_string(),
                });
            }
            ActiveTurnInputIntent::Prompt => {
                state.input = input;
                queue_active_turn_input(state);
            }
        }
        return Ok(false);
    }
    state.entries.push(TuiEntry {
        label: "user".to_string(),
        body: input.clone(),
    });
    state.transcript_scroll = 0;
    state.streaming_assistant = None;
    if handle_local_setting_command(&input, state, terminal)?
        || handle_tui_command(&input, state)
        || handle_screen_command(&input, state)
    {
        return Ok(false);
    }
    if handle_immediate_runtime_command(runtime, state, &input)? {
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
    if let Err(err) = runtime.start_provider_turn(input) {
        let queued_inputs = take_queued_inputs(state);
        state.pending_turn = None;
        state.streaming_assistant = None;
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: render_provider_turn_error(&err),
        });
        restore_first_queued_input(state, queued_inputs);
    }
    Ok(false)
}

fn take_queued_inputs(state: &mut TuiState) -> Vec<String> {
    state
        .pending_turn
        .as_mut()
        .map(|turn| std::mem::take(&mut turn.queued_inputs))
        .unwrap_or_default()
}

fn restore_first_queued_input(state: &mut TuiState, queued_inputs: Vec<String>) {
    let queued_count = queued_inputs.len();
    let mut queued_inputs = queued_inputs.into_iter();
    if let Some(first) = queued_inputs.next() {
        state.input = first;
        reset_for_input_change(state);
        if queued_count > 1 {
            let remaining = queued_inputs.collect::<Vec<_>>();
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: format!(
                    "{} were not run because the active turn failed. The first draft was restored to the composer; preserved queued drafts:\n{}",
                    queued_prompt_count_label(queued_count),
                    remaining
                        .iter()
                        .enumerate()
                        .map(|(index, input)| format!("  {}. {}", index + 2, input))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            });
        }
    }
}

fn poll_turn_controller_events(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    active_approval: &mut Option<ActiveApproval>,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    let mut should_exit = false;
    let mut saw_event = false;
    let mut turn_error = None::<String>;
    while let Some(event) = runtime.poll_event() {
        saw_event = true;
        if let RuntimeEventKind::Error { error } = &event.kind {
            turn_error = Some(error.message.clone());
        }
        match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => {
                if active_approval.is_some() {
                    runtime.respond_to_approval(
                        approval.id.clone(),
                        ApprovalResponse {
                            approved: false,
                            feedback: Some("another approval is already pending".to_string()),
                        },
                    );
                } else {
                    *active_approval = Some(begin_active_approval(approval.clone(), state));
                }
            }
            RuntimeEventKind::AssistantDelta {
                message_id,
                content,
                ..
            } if message_id.starts_with("stream-") => {
                append_streaming_assistant_delta(state, content);
            }
            _ => {}
        }
        apply_runtime_event_to_tui(state, &event, &runtime.view_state);
    }
    if saw_event && state.pending_turn.is_some() && !runtime.supervisor.is_turn_active() {
        let queued_inputs = take_queued_inputs(state);
        state.streaming_assistant = None;
        state.pending_turn = None;
        *active_approval = None;
        runtime.note_turn_finished();
        sync_state_from_runtime_view(state, &runtime.view_state);
        if let Some(error) = turn_error {
            state.entries.push(TuiEntry {
                label: "system".to_string(),
                body: render_provider_turn_error(&error),
            });
            restore_first_queued_input(state, queued_inputs);
            return Ok(false);
        }
        let mut queued_inputs = queued_inputs.into_iter();
        while let Some(queued_input) = queued_inputs.next() {
            if handle_submitted_input(runtime, state, terminal, queued_input)? {
                should_exit = true;
                break;
            }
            if runtime.is_turn_active() {
                if let Some(turn) = state.pending_turn.as_mut() {
                    turn.queued_inputs.extend(queued_inputs);
                }
                break;
            }
        }
    }
    Ok(should_exit)
}

fn render_provider_turn_error(err: &str) -> String {
    if err.contains("Tool `") || err.contains("tool failed") {
        return format!(
            "Tool execution failed, but Viden kept the TUI open.\n  error: {err}\n  next: inspect the tool path/input, then retry or ask Viden to use another tool."
        );
    }
    format!(
        "Provider turn failed, but Viden kept the TUI open.\n  error: {err}\n  next: try /status, /provider doctor, /models, or retry with a smaller prompt."
    )
}

fn handle_immediate_runtime_command(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    input: &str,
) -> Result<bool, String> {
    if !is_immediate_runtime_command(input) {
        return Ok(false);
    }
    if runtime.is_turn_active() {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("`{input}` will be available after the current provider turn finishes."),
        });
        return Ok(true);
    }
    run_settings_command(runtime, state, input)?;
    Ok(true)
}

fn is_immediate_runtime_command(input: &str) -> bool {
    matches!(
        input.split_whitespace().collect::<Vec<_>>().as_slice(),
        ["/plan"]
            | ["/plan", "on"]
            | ["/plan", "off"]
            | ["/mode"]
            | ["/mode", "build"]
            | ["/mode", "plan"]
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTurnInputIntent {
    Cancel,
    ImmediateCommand,
    Command,
    Prompt,
}

fn active_turn_input_intent(input: &str) -> ActiveTurnInputIntent {
    if is_active_turn_cancel_command(input) {
        ActiveTurnInputIntent::Cancel
    } else if is_immediate_runtime_command(input) {
        ActiveTurnInputIntent::ImmediateCommand
    } else if is_slash_command(input) {
        ActiveTurnInputIntent::Command
    } else {
        ActiveTurnInputIntent::Prompt
    }
}

fn is_active_turn_cancel_command(input: &str) -> bool {
    matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "/cancel" | "/stop" | "/interrupt" | "/abort"
    )
}

fn is_slash_command(input: &str) -> bool {
    input.trim_start().starts_with('/')
}

fn sync_state_from_runtime_view(state: &mut TuiState, view: &RuntimeViewState) {
    state.provider = view.snapshot.provider_family.clone();
    state.model = view.snapshot.model_label.clone();
    state.provider_status.work_mode = view.snapshot.work_mode;
    state.provider_status.permission_level = view.snapshot.permission_level;
    state.runtime_tasks = view.tasks.clone();
    if let Some(provider) = &view.provider {
        state.provider_status.connection = provider.status.clone();
        state.provider_status.request_count = provider.request_count;
        state.provider_status.failure_count = provider.error_count;
        state.provider_status.success_count =
            provider.request_count.saturating_sub(provider.error_count);
        state.provider_status.last_latency_ms = provider.last_latency_ms.map(u128::from);
        state.provider_status.average_latency_ms = provider.average_latency_ms.map(u128::from);
        state.provider_status.last_tokens_per_second = provider.tokens_per_second;
        state.provider_status.telemetry = format!(
            "{} req / {} ok / {} err",
            state.provider_status.request_count,
            state.provider_status.success_count,
            state.provider_status.failure_count
        );
    }
    if let Some(cost) = &view.token_cost {
        state.provider_status.last_input_tokens = Some(cost.input_tokens);
        state.provider_status.last_output_tokens = Some(cost.output_tokens);
        state.provider_status.last_total_tokens = Some(cost.total_tokens);
        state.provider_status.total_tokens = cost.total_tokens;
        state.provider_status.last_cost_micro_usd = cost.cost_micro_usd;
        state.provider_status.total_cost_micro_usd = cost.cost_micro_usd;
    }
    if let Some(error) = view.errors.last() {
        state.provider_status.last_error = Some(error.message.clone());
    }
    state.workspace = WorkspaceSnapshot::load_current();
}

fn apply_runtime_event_to_tui(state: &mut TuiState, event: &RuntimeEvent, view: &RuntimeViewState) {
    let defer_streaming_entry = state.pending_turn.is_some()
        && matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { message_id, .. }
                if message_id.starts_with("stream-")
        );
    if !defer_streaming_entry && let Some(entry) = entry_from_runtime_event(event) {
        state.entries.push(entry);
    }
    refresh_diagnostics_cache(state);
    sync_state_from_runtime_view(state, view);
}

fn entry_from_runtime_event(event: &RuntimeEvent) -> Option<TuiEntry> {
    match &event.kind {
        RuntimeEventKind::AssistantDelta { content, .. } => Some(TuiEntry {
            label: "assistant".to_string(),
            body: content.clone(),
        }),
        RuntimeEventKind::ToolCallStarted {
            name,
            input_preview,
            ..
        } => Some(TuiEntry {
            label: "tool-call".to_string(),
            body: format!("{name} {input_preview}").trim().to_string(),
        }),
        RuntimeEventKind::ToolCallFinished {
            name,
            success,
            exit_code,
            evidence,
            ..
        } => Some(TuiEntry {
            label: "tool-result".to_string(),
            body: evidence
                .as_ref()
                .map(|evidence| evidence.summary.clone())
                .unwrap_or_else(|| {
                    format!(
                        "{name} completed success={success}{}",
                        exit_code
                            .map(|code| format!(" exit_code={code}"))
                            .unwrap_or_default()
                    )
                }),
        }),
        RuntimeEventKind::ApprovalResolved {
            request_id,
            approved,
        } => Some(TuiEntry {
            label: "approval".to_string(),
            body: format!(
                "{} `{request_id}`.",
                if *approved { "Approved" } else { "Denied" }
            ),
        }),
        RuntimeEventKind::CommandRejected { command_id, reason } => Some(TuiEntry {
            label: "system".to_string(),
            body: format!("Command `{command_id}` rejected: {reason}"),
        }),
        RuntimeEventKind::EvidenceRecorded { evidence } => Some(TuiEntry {
            label: evidence.kind.clone(),
            body: evidence.summary.clone(),
        }),
        RuntimeEventKind::Error { error } => Some(TuiEntry {
            label: "system".to_string(),
            body: error.message.clone(),
        }),
        RuntimeEventKind::SnapshotUpdated { .. }
        | RuntimeEventKind::ApprovalRequested { .. }
        | RuntimeEventKind::CommandAccepted { .. }
        | RuntimeEventKind::InputQueued { .. }
        | RuntimeEventKind::InputDequeued { .. }
        | RuntimeEventKind::TaskUpdated { .. }
        | RuntimeEventKind::LaneUpdated { .. }
        | RuntimeEventKind::ContextUpdated { .. }
        | RuntimeEventKind::ProviderHealthUpdated { .. }
        | RuntimeEventKind::TokenCostUpdated { .. } => None,
    }
}

fn handle_local_setting_command(
    input: &str,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    if open_local_picker_command(input, state) {
        return Ok(true);
    }
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

fn open_local_picker_command(input: &str, state: &mut TuiState) -> bool {
    state.interaction_panel = match input.trim() {
        "/connect" | "/provider" | "/settings provider" | "/setup provider" => {
            Some(InteractionPanel::ConnectProvider {
                search: String::new(),
                selected: 0,
            })
        }
        "/models" | "/model" | "/settings model" | "/setup model" => {
            Some(InteractionPanel::ModelPicker {
                provider_id: None,
                search: String::new(),
                selected: 0,
            })
        }
        _ => return false,
    };
    state.input.clear();
    reset_for_input_change(state);
    true
}

fn move_interaction_selection(state: &mut TuiState, delta: i8) {
    let count = interaction_choice_count(state);
    if count == 0 {
        set_interaction_panel_selected(state, 0);
        return;
    }
    let current = interaction_selected(state).min(count.saturating_sub(1));
    let next = if delta < 0 {
        current.saturating_sub(1)
    } else {
        (current + 1).min(count - 1)
    };
    set_interaction_panel_selected(state, next);
}

fn edit_interaction_panel_text(state: &mut TuiState, value: Option<char>) {
    match state.interaction_panel.as_mut() {
        Some(InteractionPanel::ConnectProvider { search, selected })
        | Some(InteractionPanel::ModelPicker {
            search, selected, ..
        }) => {
            match value {
                Some(value) => search.push(value),
                None => {
                    search.pop();
                }
            }
            *selected = 0;
        }
        Some(InteractionPanel::ProviderConfig { .. }) => {}
        Some(InteractionPanel::ProviderApiKey { input, .. }) => match value {
            Some(value) => input.push(value),
            None => {
                input.pop();
            }
        },
        None => {}
    }
}

fn apply_interaction_panel_selection(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
) -> Result<(), String> {
    match state.interaction_panel.clone() {
        Some(InteractionPanel::ConnectProvider { search, selected }) => {
            let providers = filtered_interaction_providers(state, &search);
            let Some(provider) = providers.get(selected.min(providers.len().saturating_sub(1)))
            else {
                return Ok(());
            };
            let provider_id = provider.provider_id.clone();
            if provider_needs_api_key(provider) {
                state.interaction_panel = Some(InteractionPanel::ProviderApiKey {
                    provider_id,
                    input: String::new(),
                });
            } else {
                open_provider_config_panel(state, provider_id);
            }
        }
        Some(InteractionPanel::ProviderConfig {
            provider_id,
            selected,
        }) => match selected.min(PROVIDER_CONFIG_ACTION_COUNT - 1) {
            PROVIDER_CONFIG_CHOOSE_MODEL => open_provider_model_panel(state, provider_id),
            PROVIDER_CONFIG_CHANGE_KEY => {
                state.interaction_panel = Some(InteractionPanel::ProviderApiKey {
                    provider_id,
                    input: String::new(),
                });
            }
            PROVIDER_CONFIG_CLEAR_SESSION_KEY => {
                if let Some(provider) = state
                    .provider_catalog
                    .iter()
                    .find(|provider| provider.provider_id == provider_id)
                    && let Some(key_env) = provider.api_key_env.as_deref()
                {
                    // Only clear the current process environment. Viden does
                    // not persist raw API keys, so deleting the saved env-var
                    // name would make future setup less discoverable.
                    unsafe {
                        std::env::remove_var(key_env);
                    }
                    state.entries.push(TuiEntry {
                        label: "setup".to_string(),
                        body: format!(
                            "Cleared `{key_env}` for this Viden process. The config still records the env var name; enter a new key from `/connect {}` or export `{key_env}` in your shell.",
                            provider.provider_id
                        ),
                    });
                }
                if runtime.is_turn_active() {
                    state.entries.push(TuiEntry {
                        label: "system".to_string(),
                        body: "Provider key changes are paused until the active turn finishes."
                            .to_string(),
                    });
                }
                state.interaction_panel = Some(InteractionPanel::ProviderConfig {
                    provider_id,
                    selected: PROVIDER_CONFIG_CHANGE_KEY,
                });
            }
            PROVIDER_CONFIG_DOCTOR => {
                run_settings_command(runtime, state, &format!("/provider doctor {provider_id}"))?;
                state.interaction_panel = Some(InteractionPanel::ProviderConfig {
                    provider_id,
                    selected: PROVIDER_CONFIG_DOCTOR,
                });
            }
            _ => {}
        },
        Some(InteractionPanel::ProviderApiKey { provider_id, input }) => {
            let key = input.trim();
            if key.is_empty() {
                return Ok(());
            }
            let Some(provider) = state
                .provider_catalog
                .iter()
                .find(|provider| provider.provider_id == provider_id)
                .cloned()
            else {
                state.interaction_panel = None;
                return Ok(());
            };
            let key_env = provider.api_key_env.clone().unwrap_or_else(|| {
                format!("{}_API_KEY", provider.provider_id.to_ascii_uppercase())
            });
            // TUI entry should never persist the raw API key. It is injected only
            // into the current process environment, then the env var name is
            // saved so future shells can provide the value explicitly.
            unsafe {
                std::env::set_var(&key_env, key);
            }
            run_settings_command(
                runtime,
                state,
                &format!("/settings provider {provider_id} key-env {key_env}"),
            )?;
            open_provider_config_panel(state, provider_id);
        }
        Some(InteractionPanel::ModelPicker {
            provider_id,
            search,
            selected,
        }) => {
            let provider_scoped_setup = provider_id.clone();
            let choices = filtered_interaction_models(state, provider_id.as_deref(), &search);
            let Some(choice) = choices.get(selected.min(choices.len().saturating_sub(1))) else {
                return Ok(());
            };
            let command = format!("/models {} {}", choice.provider_id, choice.model);
            run_settings_command(runtime, state, &command)?;
            if provider_scoped_setup.is_some() {
                run_settings_command(
                    runtime,
                    state,
                    &format!("/provider doctor {}", choice.provider_id),
                )?;
                state.entries.push(TuiEntry {
                    label: "setup".to_string(),
                    body: format!(
                        "Provider setup completed for `{}` / `{}`.\nDoctor output is above. For a real request smoke, run `scripts/provider-live-smoke.sh --provider {} --model {}` when the key env is available.",
                        choice.provider_id, choice.model, choice.provider_id, choice.model
                    ),
                });
            }
            state.interaction_panel = None;
        }
        None => {}
    }
    Ok(())
}

fn run_settings_command(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    command: &str,
) -> Result<(), String> {
    if runtime.is_turn_active() {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("`{command}` is paused until the active provider turn finishes."),
        });
        return Ok(());
    }
    let command_id = runtime.send_text_input(command.to_string())?;
    drain_runtime_events_for_command(runtime, state, &command_id);
    Ok(())
}

fn drain_runtime_events_for_command(
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    command_id: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_command_result = false;
    while Instant::now() < deadline {
        let Some(event) = runtime.poll_event() else {
            if saw_command_result && !runtime.supervisor.is_turn_active() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        match &event.kind {
            RuntimeEventKind::CommandAccepted {
                command_id: accepted,
                ..
            }
            | RuntimeEventKind::CommandRejected {
                command_id: accepted,
                ..
            } if accepted == command_id => {
                saw_command_result = true;
            }
            RuntimeEventKind::ApprovalRequested { approval } => {
                runtime.respond_to_approval(
                    approval.id.clone(),
                    ApprovalResponse {
                        approved: false,
                        feedback: Some("settings commands cannot prompt for approval".to_string()),
                    },
                );
            }
            _ => {}
        }
        apply_runtime_event_to_tui(state, &event, &runtime.view_state);
    }
    if !saw_command_result {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!(
                "`{command_id}` is still running; the TUI will update from runtime events."
            ),
        });
    }
}

fn open_provider_model_panel(state: &mut TuiState, provider_id: String) {
    state.interaction_panel = Some(InteractionPanel::ModelPicker {
        provider_id: Some(provider_id),
        search: String::new(),
        selected: 0,
    });
}

fn interaction_selected(state: &TuiState) -> usize {
    match state.interaction_panel.as_ref() {
        Some(InteractionPanel::ConnectProvider { selected, .. })
        | Some(InteractionPanel::ProviderConfig { selected, .. })
        | Some(InteractionPanel::ModelPicker { selected, .. }) => *selected,
        _ => 0,
    }
}

fn set_interaction_panel_selected(state: &mut TuiState, index: usize) {
    match state.interaction_panel.as_mut() {
        Some(InteractionPanel::ConnectProvider { selected, .. })
        | Some(InteractionPanel::ProviderConfig { selected, .. })
        | Some(InteractionPanel::ModelPicker { selected, .. }) => *selected = index,
        _ => {}
    }
}

fn interaction_choice_count(state: &TuiState) -> usize {
    match state.interaction_panel.as_ref() {
        Some(InteractionPanel::ConnectProvider { search, .. }) => {
            filtered_interaction_providers(state, search).len()
        }
        Some(InteractionPanel::ProviderConfig { .. }) => PROVIDER_CONFIG_ACTION_COUNT,
        Some(InteractionPanel::ModelPicker {
            provider_id,
            search,
            ..
        }) => filtered_interaction_models(state, provider_id.as_deref(), search).len(),
        _ => 0,
    }
}

const PROVIDER_CONFIG_CHOOSE_MODEL: usize = 0;
const PROVIDER_CONFIG_CHANGE_KEY: usize = 1;
const PROVIDER_CONFIG_CLEAR_SESSION_KEY: usize = 2;
const PROVIDER_CONFIG_DOCTOR: usize = 3;
const PROVIDER_CONFIG_ACTION_COUNT: usize = 4;

fn open_provider_config_panel(state: &mut TuiState, provider_id: String) {
    state.interaction_panel = Some(InteractionPanel::ProviderConfig {
        provider_id,
        selected: PROVIDER_CONFIG_CHOOSE_MODEL,
    });
}

fn provider_needs_api_key(provider: &ProviderOption) -> bool {
    provider.api_key_env.as_deref().is_some_and(|key| {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_none()
    })
}

fn filtered_interaction_providers<'a>(
    state: &'a TuiState,
    search: &str,
) -> Vec<&'a ProviderOption> {
    let needle = search.trim().to_ascii_lowercase();
    let mut providers = state
        .provider_catalog
        .iter()
        .filter(|provider| provider.provider_id != "fallback")
        .filter(|provider| {
            needle.is_empty()
                || provider.provider_id.to_ascii_lowercase().contains(&needle)
                || provider.display_name.to_ascii_lowercase().contains(&needle)
        })
        .collect::<Vec<_>>();
    providers.sort_by_key(|provider| {
        (
            provider.provider_id != state.provider,
            !matches!(
                provider.provider_id.as_str(),
                "deepseek" | "dashscope-coding-plan" | "openrouter" | "openai" | "anthropic"
            ),
            provider.display_name.to_ascii_lowercase(),
        )
    });
    providers
}

#[derive(Debug, Clone)]
struct InteractionModelChoice {
    provider_id: String,
    model: String,
}

fn filtered_interaction_models(
    state: &TuiState,
    provider_filter: Option<&str>,
    search: &str,
) -> Vec<InteractionModelChoice> {
    let needle = search.trim().to_ascii_lowercase();
    let mut choices = Vec::new();
    for provider in &state.provider_catalog {
        if provider_filter.is_some_and(|filter| filter != provider.provider_id) {
            continue;
        }
        let models = if provider_filter.is_some() {
            provider_interaction_models(provider)
        } else if provider_is_available_for_model_picker(provider) {
            active_interaction_models(provider)
        } else {
            Vec::new()
        };
        for model in models {
            if needle.is_empty()
                || model.to_ascii_lowercase().contains(&needle)
                || provider.display_name.to_ascii_lowercase().contains(&needle)
            {
                choices.push(InteractionModelChoice {
                    provider_id: provider.provider_id.clone(),
                    model,
                });
            }
        }
    }
    choices.sort_by_key(|choice| {
        (
            choice.provider_id != state.provider,
            choice.provider_id.to_ascii_lowercase(),
            choice.model.to_ascii_lowercase(),
        )
    });
    choices
}

fn provider_interaction_models(provider: &ProviderOption) -> Vec<String> {
    let mut models = provider.favorite_models.clone();
    for model in &provider.enabled_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    if let Some(default_model) = &provider.default_model
        && !models.contains(default_model)
    {
        models.push(default_model.clone());
    }
    for model in &provider.known_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    models
}

fn active_interaction_models(provider: &ProviderOption) -> Vec<String> {
    let mut models = provider.favorite_models.clone();
    for model in &provider.enabled_models {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    models
}

fn provider_is_available_for_model_picker(provider: &ProviderOption) -> bool {
    !active_interaction_models(provider).is_empty()
        || provider.api_key_env.as_deref().is_some_and(|env| {
            std::env::var(env)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        })
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

fn begin_active_approval(approval: ApprovalRequestView, state: &mut TuiState) -> ActiveApproval {
    state.approval_focus = DEFAULT_APPROVAL_FOCUS;
    state.approval_apply_all = false;
    state.entries.push(TuiEntry {
        label: "approval".to_string(),
        body: format!(
            "Permission request for `{}`\n{}\n{}\nPress y to allow, n/Esc to deny. Tab/arrows move, Enter activates, click buttons.",
            approval.tool_name, approval.message, approval.input_preview
        ),
    });
    mark_pending_turn_waiting_for_approval(state, &approval);
    ActiveApproval { approval }
}

fn handle_active_approval_key(
    key: KeyEvent,
    active_approval: &mut Option<ActiveApproval>,
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    if active_approval.is_none() {
        return Ok(false);
    }
    match apply_approval_key(key, state) {
        ApprovalKeyEffect::Resolve(approved) => {
            resolve_active_approval(runtime, approved, active_approval, state);
            terminal.draw(state)?;
            Ok(true)
        }
        ApprovalKeyEffect::Redraw => {
            terminal.draw(state)?;
            Ok(true)
        }
        ApprovalKeyEffect::None => match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                runtime.cancel_active_turn();
                resolve_active_approval(runtime, false, active_approval, state);
                state.entries.push(TuiEntry {
                    label: "system".to_string(),
                    body: "Cancellation requested while approval was pending.".to_string(),
                });
                terminal.draw(state)?;
                Ok(true)
            }
            KeyCode::PageUp => {
                scroll_transcript(state, 12);
                terminal.draw(state)?;
                Ok(true)
            }
            KeyCode::PageDown => {
                scroll_transcript(state, -12);
                terminal.draw(state)?;
                Ok(true)
            }
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.transcript_scroll = usize::MAX / 2;
                terminal.draw(state)?;
                Ok(true)
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.transcript_scroll = 0;
                terminal.draw(state)?;
                Ok(true)
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let theme_name = terminal.cycle_theme();
                state.theme_name = theme_name.to_string();
                terminal.draw(state)?;
                Ok(true)
            }
            KeyCode::Backspace | KeyCode::Char(_) => Ok(false),
            _ => Ok(true),
        },
    }
}

fn handle_active_approval_mouse(
    mouse: MouseEvent,
    active_approval: &mut Option<ActiveApproval>,
    runtime: &mut RuntimeClient,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Result<bool, String> {
    if active_approval.is_none() {
        return Ok(false);
    }
    if matches!(
        mouse.kind,
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
    ) {
        if handle_mouse(mouse, state) {
            terminal.draw(state)?;
        }
        return Ok(true);
    }
    if !matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
    ) {
        return Ok(true);
    }
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let Some(action) = approval_action_at(state, mouse.column, mouse.row, width, height, 38) else {
        return Ok(true);
    };
    set_approval_focus_for_action(state, action);
    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        terminal.draw(state)?;
        return Ok(true);
    }
    match apply_approval_action(action, state) {
        ApprovalKeyEffect::Resolve(approved) => {
            resolve_active_approval(runtime, approved, active_approval, state);
        }
        ApprovalKeyEffect::Redraw | ApprovalKeyEffect::None => {}
    }
    terminal.draw(state)?;
    Ok(true)
}

fn resolve_active_approval(
    runtime: &mut RuntimeClient,
    approved: bool,
    active_approval: &mut Option<ActiveApproval>,
    state: &mut TuiState,
) {
    let Some(active) = active_approval.take() else {
        return;
    };
    let verb = if approved { "Approved" } else { "Denied" };
    let apply_all = if state.approval_apply_all {
        " apply_all=true"
    } else {
        ""
    };
    state.entries.push(TuiEntry {
        label: "approval".to_string(),
        body: format!("{verb} `{}`.{apply_all}", active.approval.tool_name),
    });
    state.approval_focus = DEFAULT_APPROVAL_FOCUS;
    state.approval_apply_all = false;
    mark_pending_turn_waiting_for_provider(state);
    runtime.respond_to_approval(
        active.approval.id,
        ApprovalResponse {
            approved,
            feedback: None,
        },
    );
}

fn queue_active_turn_input(state: &mut TuiState) {
    let draft = state.input.trim().to_string();
    if draft.is_empty() {
        return;
    }
    if let Some(turn) = state.pending_turn.as_mut() {
        turn.queued_inputs.push(draft);
        let count = turn.queued_inputs.len();
        state.input.clear();
        reset_for_input_change(state);
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!(
                "{} queued. Viden will run it after the current turn finishes.",
                queued_prompt_count_label(count)
            ),
        });
    }
}

fn queued_prompt_count_label(count: usize) -> String {
    if count == 1 {
        "1 prompt".to_string()
    } else {
        format!("{count} prompts")
    }
}

fn mark_pending_turn_waiting_for_approval(state: &mut TuiState, approval: &ApprovalRequestView) {
    if let Some(turn) = state.pending_turn.as_mut() {
        turn.phase = format!("Waiting for approval: {}", approval.tool_name);
        turn.next_action = "approve / deny".to_string();
    }
}

fn append_streaming_assistant_delta(state: &mut TuiState, delta: &str) {
    if delta.is_empty() {
        return;
    }
    let body = state.streaming_assistant.get_or_insert_with(String::new);
    body.push_str(delta);
    if let Some(turn) = state.pending_turn.as_mut() {
        turn.phase = "Streaming assistant response".to_string();
        turn.next_action = "read streaming response".to_string();
    }
}

fn mark_pending_turn_waiting_for_provider(state: &mut TuiState) {
    if let Some(turn) = state.pending_turn.as_mut() {
        turn.phase = "Waiting for provider response".to_string();
        turn.next_action = "wait".to_string();
    }
}

fn provider_catalog(engine: &SessionEngine) -> Vec<ProviderOption> {
    let ui_config = viden_core::load_provider_ui_configs(engine.cwd()).unwrap_or_default();
    engine
        .provider_descriptors()
        .iter()
        .map(|descriptor| {
            let mut option = ProviderOption::from_descriptor(descriptor);
            let mut has_configured_default_model = false;
            if let Some(config) = ui_config.get(&option.provider_id) {
                if config.api_base.is_some() {
                    option.default_api_base = config.api_base.clone();
                }
                if config.api_key_env.is_some() {
                    option.api_key_env = config.api_key_env.clone();
                }
                if config.default_model.is_some() {
                    has_configured_default_model = true;
                    option.default_model = config.default_model.clone();
                }
                option.enabled_models = config.models.clone();
                option.favorite_models = config.favorite_models.clone();
            }
            if option.provider_id == engine.provider_name() && option.enabled_models.is_empty() {
                option.enabled_models.push(engine.model_name().to_string());
            }
            if has_configured_default_model
                && option.enabled_models.is_empty()
                && let Some(default_model) = option.default_model.clone()
            {
                option.enabled_models.push(default_model);
            }
            option
        })
        .collect()
}

fn maybe_start_background_diagnostics(
    runtime: &mut RuntimeClient,
    state: &TuiState,
    last_started: &mut Option<Instant>,
) {
    if runtime.is_turn_active() {
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
    let _ = runtime.send_command(RuntimeCommand::RefreshDiagnostics { paths });
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

fn refresh_diagnostics_cache(state: &mut TuiState) {
    let Some(diagnostics) = latest_lsp_diagnostics(&state.entries) else {
        return;
    };
    if let Err(err) = save_diagnostics(&state.workspace.root, &diagnostics) {
        state.entries.push(TuiEntry {
            label: "system".to_string(),
            body: format!("Failed to persist LSP diagnostics: {err}"),
        });
        return;
    }
    state.workspace = WorkspaceSnapshot::load(state.workspace.root.clone());
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
        ActiveTurnInputIntent, active_turn_input_intent, append_streaming_assistant_delta,
        apply_composer_shortcut, background_diagnostic_paths, begin_active_approval,
        event_requires_repaint, filtered_interaction_models, handle_immediate_runtime_command,
        initial_state, is_exit_command, is_immediate_runtime_command, is_send_key, last_user_input,
        open_local_picker_command, push_composer_char, queue_active_turn_input,
        refresh_diagnostics_cache, render_provider_turn_error, resolve_active_approval,
        restore_first_queued_input, scroll_transcript,
    };
    use crate::tui::state::{
        InteractionPanel, ProviderOption, ProviderStatus, TerminalLane, WorkspaceSnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::{
        collections::VecDeque,
        fs,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use viden_core::{ModelProvider, ModelRequestControl, ProviderAuthMode, SessionEngine};
    use viden_types::{
        ApprovalRequestView, EvidenceView, ModelEvent, ModelRequest, PermissionLevel, ToolCall,
        ToolInput, WorkMode,
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
    fn plan_command_is_immediate_not_provider_turn() {
        assert!(is_immediate_runtime_command("/plan"));
        assert!(is_immediate_runtime_command("/plan on"));
        assert!(is_immediate_runtime_command("/plan off"));
        assert!(!is_immediate_runtime_command("/plan now"));
        assert!(!is_immediate_runtime_command("write a plan"));
    }

    #[test]
    fn immediate_plan_command_does_not_leave_input_locked() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(TestProvider {
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();
        let mut runtime = super::RuntimeClient::start(engine);
        let mut state = test_state("");

        let handled =
            handle_immediate_runtime_command(&mut runtime, &mut state, "/plan on").unwrap();

        assert!(handled);
        assert_eq!(state.input, "");
        assert!(state.pending_turn.is_none());
        assert!(state.streaming_assistant.is_none());
        assert_eq!(state.provider_status.work_mode, WorkMode::Plan);
        assert_eq!(
            state.provider_status.permission_level,
            PermissionLevel::ReadOnly
        );
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.label == "command" && entry.body.contains("Plan mode is now on"))
        );
        assert!(
            state
                .runtime_tasks
                .iter()
                .all(|task| !task.activity.contains("provider"))
        );
    }

    #[test]
    fn mode_and_permission_commands_immediately_sync_tui_runtime_status() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(TestProvider {
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();
        let mut runtime = super::RuntimeClient::start(engine);
        let mut state = test_state("");

        super::run_settings_command(&mut runtime, &mut state, "/mode plan").unwrap();
        assert_eq!(state.provider_status.work_mode, WorkMode::Plan);
        assert_eq!(
            state.provider_status.permission_level,
            PermissionLevel::ReadOnly
        );

        super::run_settings_command(&mut runtime, &mut state, "/mode build").unwrap();
        assert_eq!(state.provider_status.work_mode, WorkMode::Build);
        assert_eq!(state.provider_status.permission_level, PermissionLevel::Ask);

        super::run_settings_command(&mut runtime, &mut state, "/permissions auto_edit").unwrap();
        assert_eq!(state.provider_status.work_mode, WorkMode::Build);
        assert_eq!(
            state.provider_status.permission_level,
            PermissionLevel::AutoEdit
        );

        super::run_settings_command(&mut runtime, &mut state, "/permissions ask").unwrap();
        assert_eq!(state.provider_status.work_mode, WorkMode::Build);
        assert_eq!(state.provider_status.permission_level, PermissionLevel::Ask);
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
    fn composer_discards_terminal_escape_residue_instead_of_rendering_it() {
        let mut state = test_state("");
        for ch in "2;28;95;132m".chars() {
            push_composer_char(&mut state, ch);
        }

        assert_eq!(state.input, "");

        for ch in "<2;28;95;132m".chars() {
            push_composer_char(&mut state, ch);
        }

        assert_eq!(state.input, "");
    }

    #[test]
    fn composer_discards_uppercase_sgr_mouse_residue() {
        let mut state = test_state("");
        for ch in "0;105;25M".chars() {
            push_composer_char(&mut state, ch);
        }

        assert_eq!(state.input, "");
    }

    #[test]
    fn composer_keeps_normal_digit_prompts() {
        let mut state = test_state("");
        for ch in "2 failing tests; fix both".chars() {
            push_composer_char(&mut state, ch);
        }

        assert_eq!(state.input, "2 failing tests; fix both");
    }

    #[test]
    fn focus_and_paste_events_force_repaint_without_becoming_input() {
        assert!(event_requires_repaint(&Event::FocusGained));
        assert!(event_requires_repaint(&Event::FocusLost));
        assert!(event_requires_repaint(&Event::Paste("draft".to_string())));
    }

    #[test]
    fn active_turn_enter_queues_next_prompt_and_keeps_composer_editable() {
        let mut state = test_state("write the follow-up tests");
        state.pending_turn = Some(super::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "first task",
            "/tmp/project",
        ));

        queue_active_turn_input(&mut state);

        let pending = state.pending_turn.as_ref().expect("pending turn");
        assert_eq!(pending.queued_inputs, vec!["write the follow-up tests"]);
        assert_eq!(state.input, "");
        assert!(state.entries.iter().any(|entry| entry.label == "system"
            && entry.body.contains("1 prompt queued. Viden will run it")));

        state.input = "and summarize the risk".to_string();
        queue_active_turn_input(&mut state);

        let pending = state.pending_turn.as_ref().expect("pending turn");
        assert_eq!(
            pending.queued_inputs,
            vec!["write the follow-up tests", "and summarize the risk"]
        );
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.body.contains("2 prompts queued"))
        );
    }

    #[test]
    fn active_turn_input_intent_keeps_commands_out_of_prompt_queue() {
        assert_eq!(
            active_turn_input_intent("/cancel"),
            ActiveTurnInputIntent::Cancel
        );
        assert_eq!(
            active_turn_input_intent("/mode plan"),
            ActiveTurnInputIntent::ImmediateCommand
        );
        assert_eq!(
            active_turn_input_intent("/status"),
            ActiveTurnInputIntent::Command
        );
        assert_eq!(
            active_turn_input_intent("continue with the next step"),
            ActiveTurnInputIntent::Prompt
        );
    }

    #[test]
    fn runtime_provider_turn_starts_without_blocking_ui_thread() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(SlowProvider {
            delay: Duration::from_millis(180),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();
        let mut runtime = super::RuntimeClient::start(engine);

        let started = Instant::now();
        runtime
            .start_provider_turn("explain the project".to_string())
            .unwrap();

        assert!(
            started.elapsed() < Duration::from_millis(75),
            "starting a provider turn should return control to the TUI immediately"
        );
        assert!(runtime.is_turn_active());
        assert!(runtime.poll_event().is_none());

        let mut saw_assistant = false;
        for _ in 0..20 {
            if let Some(event) = runtime.poll_event()
                && matches!(
                    event.kind,
                    viden_types::RuntimeEventKind::AssistantDelta { ref content, .. }
                        if content.contains("slow provider done")
                )
            {
                saw_assistant = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(saw_assistant);
        runtime.note_turn_finished();
        assert!(!runtime.is_turn_active());
    }

    #[test]
    fn provider_turn_streams_approves_tools_runs_queued_followup_and_releases_composer() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(CodingLoopProvider::new());
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();
        let mut runtime = super::RuntimeClient::start(engine);
        let mut terminal = super::TerminalGuard::test();
        let mut state = test_state("");
        let mut active_approval = None;

        super::handle_submitted_input(
            &mut runtime,
            &mut state,
            &mut terminal,
            "create a tiny python file".to_string(),
        )
        .unwrap();
        assert!(runtime.is_turn_active());

        super::handle_submitted_input(
            &mut runtime,
            &mut state,
            &mut terminal,
            "then summarize what changed".to_string(),
        )
        .unwrap();
        assert_eq!(state.input, "");
        assert_eq!(
            state
                .pending_turn
                .as_ref()
                .expect("pending turn")
                .queued_inputs,
            vec!["then summarize what changed"]
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            super::poll_turn_controller_events(
                &mut runtime,
                &mut state,
                &mut active_approval,
                &mut terminal,
            )
            .unwrap();
            if active_approval.is_some() {
                resolve_active_approval(&mut runtime, true, &mut active_approval, &mut state);
            }
            if !runtime.is_turn_active()
                && state.pending_turn.is_none()
                && state.entries.iter().any(|entry| {
                    entry.label == "assistant" && entry.body.contains("Queued follow-up complete")
                })
            {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(root.join("hello.py").exists());
        assert_eq!(state.input, "");
        assert!(state.pending_turn.is_none());
        assert!(state.streaming_assistant.is_none());
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.label == "user" && entry.body == "then summarize what changed")
        );
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.label == "approval" && entry.body == "Approved `write_file`.")
        );
        assert!(state.entries.iter().any(|entry| {
            entry.label == "tool-result" && entry.body.contains("write_file completed")
        }));
        assert!(state.entries.iter().any(|entry| {
            entry.label == "assistant" && entry.body.contains("Queued follow-up complete")
        }));
    }

    #[test]
    fn failed_active_turn_restores_first_queued_prompt_to_composer() {
        let mut state = test_state("");
        state.pending_turn = Some(super::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "first task",
            "/tmp/project",
        ));
        let pending = state.pending_turn.as_mut().expect("pending turn");
        pending.queued_inputs = vec!["retry with smaller context".to_string()];

        restore_first_queued_input(&mut state, vec!["retry with smaller context".to_string()]);

        assert_eq!(state.input, "retry with smaller context");
        assert!(
            state
                .entries
                .iter()
                .all(|entry| !entry.body.contains("More queued prompts"))
        );
    }

    #[test]
    fn failed_active_turn_preserves_all_queued_prompts_visibly() {
        let mut state = test_state("");

        restore_first_queued_input(
            &mut state,
            vec![
                "retry with smaller context".to_string(),
                "then run tests".to_string(),
                "summarize risks".to_string(),
            ],
        );

        assert_eq!(state.input, "retry with smaller context");
        let notice = state
            .entries
            .iter()
            .find(|entry| entry.label == "system")
            .expect("preserved queue notice");
        assert!(notice.body.contains("3 prompts were not run"));
        assert!(notice.body.contains("2. then run tests"));
        assert!(notice.body.contains("3. summarize risks"));
    }

    #[test]
    fn active_approval_resolves_through_channel_without_nested_event_loop() {
        let mut state = test_state("");
        state.pending_turn = Some(super::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "edit file",
            "/tmp/project",
        ));
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(TestProvider {
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();
        let mut runtime = super::RuntimeClient::start(engine);
        let mut active = Some(begin_active_approval(test_approval_view(), &mut state));

        resolve_active_approval(&mut runtime, true, &mut active, &mut state);

        assert!(active.is_none());
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.label == "approval" && entry.body == "Approved `write_file`.")
        );
        assert_eq!(
            state
                .pending_turn
                .as_ref()
                .expect("pending turn")
                .next_action,
            "wait"
        );
    }

    #[test]
    fn active_approval_does_not_swallow_composer_typing() {
        let root = temp_app_root();
        let home = root.join("session-home");
        let provider = Box::new(TestProvider {
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
        });
        let engine = SessionEngine::new_with_home(&root, provider, Some(home)).unwrap();
        let mut runtime = super::RuntimeClient::start(engine);
        let mut terminal = super::TerminalGuard::test();
        let mut state = test_state("");
        state.pending_turn = Some(super::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "edit file",
            "/tmp/project",
        ));
        let mut active = Some(begin_active_approval(test_approval_view(), &mut state));

        let handled = super::handle_active_approval_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()),
            &mut active,
            &mut runtime,
            &mut state,
            &mut terminal,
        )
        .unwrap();

        assert!(!handled);
        assert!(active.is_some());
        assert_eq!(state.input, "");
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
    fn exact_provider_and_model_commands_expand_to_local_pickers() {
        let mut connect = test_state("");
        assert!(open_local_picker_command("/connect", &mut connect));
        assert_eq!(connect.input, "");
        assert!(matches!(
            connect.interaction_panel,
            Some(InteractionPanel::ConnectProvider { .. })
        ));

        let mut models = test_state("");
        assert!(open_local_picker_command("/models", &mut models));
        assert_eq!(models.input, "");
        assert!(matches!(
            models.interaction_panel,
            Some(InteractionPanel::ModelPicker { .. })
        ));

        let mut provider = test_state("");
        assert!(open_local_picker_command(
            "/settings provider",
            &mut provider
        ));
        assert_eq!(provider.input, "");
        assert!(matches!(
            provider.interaction_panel,
            Some(InteractionPanel::ConnectProvider { .. })
        ));

        let mut explicit = test_state("");
        assert!(!open_local_picker_command(
            "/connect deepseek",
            &mut explicit
        ));
        assert_eq!(explicit.input, "");
    }

    #[test]
    fn model_picker_omits_unconfigured_provider_models() {
        let mut state = test_state("");
        state.provider = "deepseek".to_string();
        state.provider_catalog = vec![
            ProviderOption {
                provider_id: "deepseek".to_string(),
                display_name: "DeepSeek".to_string(),
                default_api_base: Some("https://api.deepseek.com".to_string()),
                default_model: Some("deepseek-v4-flash".to_string()),
                known_models: vec![
                    "deepseek-v4-flash".to_string(),
                    "deepseek-v4-pro".to_string(),
                ],
                enabled_models: vec!["deepseek-v4-flash".to_string()],
                favorite_models: Vec::new(),
                api_key_env: None,
                api_base_env: None,
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            ProviderOption {
                provider_id: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                default_api_base: Some("https://api.openai.com/v1".to_string()),
                default_model: Some("gpt-5.2".to_string()),
                known_models: vec!["gpt-5.2".to_string(), "gpt-5.2-codex".to_string()],
                enabled_models: Vec::new(),
                favorite_models: Vec::new(),
                api_key_env: Some("__ROBOCODE_TEST_MISSING_OPENAI_KEY__".to_string()),
                api_base_env: None,
                auth_modes: vec![ProviderAuthMode::ApiKey],
            },
            ProviderOption {
                provider_id: "fallback".to_string(),
                display_name: "Fallback".to_string(),
                default_api_base: None,
                default_model: Some("fallback-local".to_string()),
                known_models: vec!["fallback-local".to_string(), "test-local".to_string()],
                enabled_models: Vec::new(),
                favorite_models: Vec::new(),
                api_key_env: None,
                api_base_env: None,
                auth_modes: vec![ProviderAuthMode::Local],
            },
        ];

        let choices = filtered_interaction_models(&state, None, "");
        assert!(
            choices
                .iter()
                .any(|choice| choice.provider_id == "deepseek")
        );
        assert!(
            !choices
                .iter()
                .any(|choice| choice.model == "deepseek-v4-pro")
        );
        assert!(!choices.iter().any(|choice| choice.provider_id == "openai"));
        assert!(
            !choices
                .iter()
                .any(|choice| choice.provider_id == "fallback")
        );

        let provider_scoped = filtered_interaction_models(&state, Some("fallback"), "");
        assert!(
            provider_scoped
                .iter()
                .any(|choice| choice.model == "fallback-local")
        );
        let deepseek_scoped = filtered_interaction_models(&state, Some("deepseek"), "");
        assert!(
            deepseek_scoped
                .iter()
                .any(|choice| choice.model == "deepseek-v4-pro")
        );
    }

    #[test]
    fn initial_state_keeps_clean_welcome_when_online_provider_key_is_missing() {
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

        assert_eq!(state.input, "");
        assert!(!state.entries.iter().any(|entry| entry.label == "setup"));
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.label == "system" && entry.body.contains("Viden TUI ready"))
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
    fn provider_turn_errors_are_rendered_without_exiting_tui() {
        let rendered = render_provider_turn_error("Argument list too long (os error 7)");

        assert!(rendered.contains("kept the TUI open"));
        assert!(rendered.contains("Argument list too long"));
        assert!(rendered.contains("/provider doctor"));
    }

    #[test]
    fn transcript_scroll_moves_without_affecting_input() {
        let mut state = test_state("draft");

        scroll_transcript(&mut state, 12);
        assert_eq!(state.transcript_scroll, 12);
        assert_eq!(state.input, "draft");

        scroll_transcript(&mut state, -5);
        assert_eq!(state.transcript_scroll, 7);
        scroll_transcript(&mut state, -20);
        assert_eq!(state.transcript_scroll, 0);
    }

    #[test]
    fn streaming_delta_updates_visible_assistant_draft() {
        let mut state = test_state("");
        state.pending_turn = Some(super::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "hello",
            "/tmp/project",
        ));

        append_streaming_assistant_delta(&mut state, "Hel");
        append_streaming_assistant_delta(&mut state, "lo");

        assert_eq!(state.streaming_assistant.as_deref(), Some("Hello"));
        assert_eq!(state.transcript_scroll, 0);
        let turn = state.pending_turn.as_ref().expect("pending turn");
        assert_eq!(turn.phase, "Streaming assistant response");
        assert_eq!(turn.next_action, "read streaming response");
    }

    #[test]
    fn streaming_delta_does_not_steal_scrollback_when_user_scrolled_up() {
        let mut state = test_state("");
        state.pending_turn = Some(super::PendingTurn::new(
            "session",
            "deepseek",
            "deepseek-v4-flash",
            "hello",
            "/tmp/project",
        ));
        state.transcript_scroll = 18;

        append_streaming_assistant_delta(&mut state, "new token");

        assert_eq!(state.streaming_assistant.as_deref(), Some("new token"));
        assert_eq!(state.transcript_scroll, 18);
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
            streaming_assistant: None,
            transcript_scroll: 0,
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
            interaction_panel: None,
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
    fn runtime_lsp_diagnostics_evidence_updates_workspace_cache() {
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
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace,
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::<TerminalLane>::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
        };

        let event = viden_types::RuntimeEvent::new(
            1,
            viden_types::RuntimeEventKind::EvidenceRecorded {
                evidence: EvidenceView {
                    id: "diagnostics-test".to_string(),
                    kind: "lsp_diagnostics".to_string(),
                    summary: "LSP diagnostics:\nsrc/lib.rs:\n  3:1 warning [fake/W1] note"
                        .to_string(),
                    path: None,
                    source: Some("runtime".to_string()),
                    timestamp: None,
                },
            },
        );
        let entry = super::entry_from_runtime_event(&event).expect("diagnostics evidence entry");
        state.entries.push(entry);
        refresh_diagnostics_cache(&mut state);

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
            streaming_assistant: None,
            transcript_scroll: 0,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: Vec::<TerminalLane>::new(),
            lane_store: None,
            focused_lane: None,
            interaction_panel: None,
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

    fn test_approval_view() -> ApprovalRequestView {
        ApprovalRequestView {
            id: "approval-test".to_string(),
            tool_name: "write_file".to_string(),
            title: "Approve write_file".to_string(),
            message: "Allow write?".to_string(),
            input_preview: "path=src/lib.rs".to_string(),
            is_mutating: true,
            reason: Some("Allow write?".to_string()),
        }
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

    struct SlowProvider {
        delay: Duration,
    }

    impl ModelProvider for SlowProvider {
        fn provider_name(&self) -> &str {
            "fallback"
        }

        fn model(&self) -> &str {
            "test-local"
        }

        fn set_model(&mut self, _model: String) {}

        fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
            thread::sleep(self.delay);
            Ok(vec![
                ModelEvent::AssistantText {
                    content: "slow provider done".to_string(),
                },
                ModelEvent::Done,
            ])
        }
    }

    struct CodingLoopProvider {
        turns: VecDeque<Vec<ModelEvent>>,
    }

    impl CodingLoopProvider {
        fn new() -> Self {
            let mut write_input = ToolInput::new();
            write_input.insert("path".to_string(), "hello.py".to_string());
            write_input.insert(
                "content".to_string(),
                "print('daily-loop-ok')\n".to_string(),
            );
            Self {
                turns: VecDeque::from([
                    vec![
                        ModelEvent::ToolCall(ToolCall {
                            id: "call-write".to_string(),
                            name: "write_file".to_string(),
                            input: write_input,
                        }),
                        ModelEvent::AssistantText {
                            content: "Created hello.py.".to_string(),
                        },
                        ModelEvent::Done,
                    ],
                    vec![
                        ModelEvent::AssistantText {
                            content: "Queued follow-up complete.".to_string(),
                        },
                        ModelEvent::Done,
                    ],
                ]),
            }
        }
    }

    impl ModelProvider for CodingLoopProvider {
        fn provider_name(&self) -> &str {
            "fallback"
        }

        fn model(&self) -> &str {
            "test-local"
        }

        fn set_model(&mut self, _model: String) {}

        fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
            Ok(self.turns.pop_front().unwrap_or_else(|| {
                vec![
                    ModelEvent::AssistantText {
                        content: "No scripted turn remaining.".to_string(),
                    },
                    ModelEvent::Done,
                ]
            }))
        }

        fn next_events_with_control(
            &mut self,
            request: &ModelRequest,
            control: &ModelRequestControl,
        ) -> Result<Vec<ModelEvent>, String> {
            control.emit_stream_delta(format!("drafting from {}...", request.model));
            self.next_events(request)
        }
    }
}
