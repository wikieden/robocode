use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use viden_core::{
    ApprovalResponse, CoreClient, EventCursor, RuntimeCommand, RuntimeViewState, TuiColorDepth,
};

use super::client::{PumpOutcome, TuiClientDriver, TuiClientError};
use super::command_palette::{
    close_on_escape, complete_selected, move_selection, reset_for_input_change,
    should_complete_on_enter,
};
use super::composer::composer_content_width;
use super::input::{
    ApprovalKeyEffect, apply_approval_key, close_focus_on_escape, effective_input_mode, input_focus,
};
use super::keymap::{InputIntent, InputMode, OverlayKind, RuntimeFacts, reduce_input};
use super::modal::{interaction_panel_choice_count, selected_interaction_command};
use super::state::{InteractionPanel, OverlayState, TuiEntry, TuiState};
use super::terminal::TerminalGuard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiOptions {
    pub startup_summary: String,
    pub startup_check: bool,
    pub color_depth: TuiColorDepth,
}

impl TuiOptions {
    pub fn new(startup_summary: impl Into<String>) -> Self {
        Self {
            startup_summary: startup_summary.into(),
            startup_check: false,
            color_depth: detect_color_depth(),
        }
    }

    pub fn with_startup_check(mut self) -> Self {
        self.startup_check = true;
        self
    }

    pub fn with_color_depth(mut self, color_depth: TuiColorDepth) -> Self {
        self.color_depth = color_depth;
        self
    }
}

#[derive(Debug)]
pub enum TuiError {
    Client(TuiClientError),
    Terminal(String),
}

impl std::fmt::Display for TuiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(error) => write!(formatter, "{error}"),
            Self::Terminal(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TuiError {}

impl From<TuiClientError> for TuiError {
    fn from(value: TuiClientError) -> Self {
        Self::Client(value)
    }
}

pub fn run_tui<C: CoreClient>(client: C, options: TuiOptions) -> Result<(), TuiError> {
    let mut driver = TuiClientDriver::connect(client)?;
    if options.startup_check {
        let _state = state_from_driver(&driver, &options);
        return Ok(());
    }
    let mut terminal = TerminalGuard::enter_with_preferences(
        &driver.view().snapshot.ui_preferences,
        options.color_depth,
    )
    .map_err(TuiError::Terminal)?;
    let mut state = state_from_driver(&driver, &options);
    terminal.draw(&state).map_err(TuiError::Terminal)?;

    loop {
        apply_pump_outcome(&mut state, driver.pump()?);
        project_runtime_view(&mut state, driver.view(), driver.cursor());
        terminal.draw(&state).map_err(TuiError::Terminal)?;

        if !event::poll(std::time::Duration::from_millis(100))
            .map_err(|err| TuiError::Terminal(err.to_string()))?
        {
            continue;
        }

        let event = event::read().map_err(|err| TuiError::Terminal(err.to_string()))?;
        let size = crossterm::terminal::size().unwrap_or((80, 24));
        if handle_ui_event(&mut driver, &mut state, event, size)? == UiEventOutcome::Exit {
            break;
        }
    }
    Ok(())
}

fn detect_color_depth() -> TuiColorDepth {
    if std::env::var("COLORTERM")
        .ok()
        .is_some_and(|value| value.contains("truecolor") || value.contains("24bit"))
    {
        TuiColorDepth::Truecolor
    } else if std::env::var("TERM")
        .ok()
        .is_some_and(|value| value.contains("256color"))
    {
        TuiColorDepth::Ansi256
    } else {
        TuiColorDepth::Ansi16
    }
}

fn state_from_driver<C: CoreClient>(driver: &TuiClientDriver<C>, options: &TuiOptions) -> TuiState {
    let mut state = TuiState::new(driver.view().clone());
    state.ui.session_id = driver.cursor().stream_id.clone();
    state.ui.theme_name = ui_profile_label(&state.runtime.snapshot.ui_preferences);
    state.ui.entries.push(TuiEntry {
        label: "system".to_string(),
        body: options.startup_summary.clone(),
    });
    project_runtime_view(&mut state, driver.view(), driver.cursor());
    state
}

fn ui_profile_label(preferences: &viden_core::ResolvedUiPreferences) -> String {
    let locale = match preferences.locale {
        viden_core::LocaleId::System => "system",
        viden_core::LocaleId::En => "en",
        viden_core::LocaleId::ZhCn => "zh-CN",
    };
    let skin = match preferences.skin {
        viden_core::UiSkin::Aurora => "aurora",
        viden_core::UiSkin::Ice => "ice",
        viden_core::UiSkin::Mono => "mono",
        viden_core::UiSkin::Amber => "amber",
        viden_core::UiSkin::Phosphor => "phosphor",
    };
    let mode = match preferences.mode {
        viden_core::UiColorMode::System => "system",
        viden_core::UiColorMode::Dark => "dark",
        viden_core::UiColorMode::Light => "light",
    };
    let density = match preferences.density {
        viden_core::UiDensity::Compact => "compact",
        viden_core::UiDensity::Regular => "regular",
        viden_core::UiDensity::Comfy => "comfy",
    };
    let motion = match preferences.motion {
        viden_core::UiMotion::System => "system",
        viden_core::UiMotion::Reduced => "reduced",
        viden_core::UiMotion::Full => "full",
    };
    format!("{skin}/{mode} · {locale} · {density} · {motion}")
}

/// Replaces TUI runtime presentation from the Core-owned projection while
/// preserving only local input/layout state and the startup/user transcript.
fn project_runtime_view(state: &mut TuiState, view: &RuntimeViewState, cursor: &EventCursor) {
    state.runtime = view.clone();
    state.ui.session_id = cursor.stream_id.clone();
}

pub(super) fn dispatch_intent<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    command: RuntimeCommand,
) -> Result<String, TuiClientError> {
    driver.send(command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiEventOutcome {
    Redraw,
    Exit,
}

fn handle_ui_event<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    state: &mut TuiState,
    event: Event,
    terminal_size: (u16, u16),
) -> Result<UiEventOutcome, TuiError> {
    match event {
        Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => Ok(UiEventOutcome::Redraw),
        Event::Paste(text) => {
            let approval_pending = !driver.view().pending_approvals.is_empty();
            if let Some(overlay) = state.ui.overlay.as_mut() {
                overlay.filter.push_str(&text);
                overlay.selected = 0;
            } else if approval_pending {
                // Approval remains pinned while the composer stays editable;
                // pasted content must never resolve the approval.
                state.ui.input.paste(&text);
                reset_for_input_change(state);
            } else if state.ui.interaction_panel.is_some() {
                for value in text.chars() {
                    edit_interaction_panel_text(state, Some(value));
                }
            } else if matches!(
                effective_input_mode(state),
                InputMode::Insert | InputMode::Overlay
            ) {
                state.ui.input.paste(&text);
                reset_for_input_change(state);
            }
            Ok(UiEventOutcome::Redraw)
        }
        Event::Mouse(_) => Ok(UiEventOutcome::Redraw),
        Event::Key(key) => handle_ui_key(driver, state, key, terminal_size),
    }
}

fn handle_ui_key<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    state: &mut TuiState,
    key: KeyEvent,
    terminal_size: (u16, u16),
) -> Result<UiEventOutcome, TuiError> {
    let mode = effective_input_mode(state);
    let focus = input_focus(state);
    let facts = RuntimeFacts {
        current_work_owner: current_work_owner(driver, state),
    };
    if !(key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)) {
        state.ui.idle_ctrl_c_armed = false;
    }
    let intent = reduce_input(mode, focus, key, facts);
    apply_input_intent(driver, state, key, intent, terminal_size)
}

fn apply_input_intent<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    state: &mut TuiState,
    key: KeyEvent,
    intent: InputIntent,
    terminal_size: (u16, u16),
) -> Result<UiEventOutcome, TuiError> {
    if let Some(outcome) = apply_pending_approval_intent(driver, state, key, &intent)? {
        return Ok(outcome);
    }
    match intent {
        InputIntent::None => {}
        InputIntent::EnterInsert => state.ui.input_mode = InputMode::Insert,
        InputIntent::LeaveInsert => state.ui.input_mode = InputMode::Normal,
        InputIntent::OpenOverlay(kind) => {
            state.ui.overlay = Some(OverlayState::new(kind));
            state.ui.idle_ctrl_c_armed = false;
        }
        InputIntent::CloseOverlay => {
            if state.ui.overlay.take().is_none() && state.ui.interaction_panel.take().is_none() {
                close_on_escape(key, state);
            }
        }
        InputIntent::ClearSelection => {
            close_focus_on_escape(key, state);
        }
        InputIntent::ArmExitConfirmation => state.ui.idle_ctrl_c_armed = true,
        InputIntent::CancelCurrentWork { owner } => {
            state.ui.idle_ctrl_c_armed = false;
            driver.send_for_owner(owner, RuntimeCommand::CancelActiveTurn)?;
            state.ui.entries.push(TuiEntry {
                label: "command".to_string(),
                body: "cancel requested".to_string(),
            });
        }
        InputIntent::CycleAgentFocus => cycle_agent_focus(state),
        InputIntent::Exit => return Ok(UiEventOutcome::Exit),
        InputIntent::InsertChar(value) => {
            if let Some(overlay) = state.ui.overlay.as_mut() {
                overlay.filter.push(value);
                overlay.selected = 0;
            } else if state.ui.interaction_panel.is_some() {
                edit_interaction_panel_text(state, Some(value));
            } else {
                push_composer_char(state, value);
            }
        }
        InputIntent::InsertNewline => {
            state.ui.input.insert_newline();
            reset_for_input_change(state);
        }
        InputIntent::Backspace => {
            if let Some(overlay) = state.ui.overlay.as_mut() {
                overlay.filter.pop();
                overlay.selected = 0;
            } else if state.ui.interaction_panel.is_some() {
                edit_interaction_panel_text(state, None);
            } else {
                state.ui.input.backspace();
                reset_for_input_change(state);
            }
        }
        InputIntent::MoveCursorLeft => state.ui.input.move_left(),
        InputIntent::MoveCursorRight => state.ui.input.move_right(),
        InputIntent::MoveCursorUp => state
            .ui
            .input
            .move_up(composer_content_width(usize::from(terminal_size.0))),
        InputIntent::MoveCursorDown => state
            .ui
            .input
            .move_down(composer_content_width(usize::from(terminal_size.0))),
        InputIntent::Submit if state.ui.input.has_unclosed_code_fence() => {
            state.ui.input.insert_newline();
            reset_for_input_change(state);
        }
        InputIntent::Submit => submit_composer(driver, state)?,
        InputIntent::MoveSelection(delta) => {
            if let Some(overlay) = state.ui.overlay.as_mut() {
                overlay.selected = if delta < 0 {
                    overlay.selected.saturating_sub(1)
                } else {
                    overlay.selected.saturating_add(1)
                };
            } else if state.ui.interaction_panel.is_some() {
                move_interaction_selection(state, delta);
            } else {
                move_selection(state, delta);
            }
        }
        InputIntent::CompleteSelection => {
            if state.ui.overlay.is_none() && state.ui.interaction_panel.is_none() {
                complete_selected(state);
            }
        }
        InputIntent::CompleteOrSubmit => {
            if state
                .ui
                .overlay
                .as_ref()
                .is_some_and(|overlay| overlay.kind == OverlayKind::ExitConfirm)
            {
                if current_work_owner(driver, state).is_some() {
                    state.ui.overlay = None;
                    state.ui.idle_ctrl_c_armed = false;
                    return Ok(UiEventOutcome::Redraw);
                }
                return Ok(UiEventOutcome::Exit);
            } else if state.ui.overlay.take().is_some() {
                // Task 4 owns navigation and filtering. Core-backed actions for
                // future screens remain contract requests rather than TUI effects.
            } else if state.ui.interaction_panel.is_some() {
                if apply_interaction_panel_selection(state) {
                    submit_composer(driver, state)?;
                }
            } else if should_complete_on_enter(state) {
                complete_selected(state);
            } else {
                submit_composer(driver, state)?;
            }
        }
        InputIntent::Scroll(delta) => scroll_transcript(state, delta),
        InputIntent::ScrollToStart => state.ui.transcript_scroll = usize::MAX / 2,
        InputIntent::ScrollToEnd => state.ui.transcript_scroll = 0,
    }
    Ok(UiEventOutcome::Redraw)
}

fn current_work_owner<C: CoreClient>(
    driver: &TuiClientDriver<C>,
    state: &TuiState,
) -> Option<viden_types::RuntimeOwner> {
    state
        .runtime
        .pending_approvals
        .first()
        .map(|approval| approval.owner.clone())
        .or_else(|| runtime_has_active_work(&state.runtime).then(|| driver.owner().clone()))
}

fn apply_pending_approval_intent<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    state: &mut TuiState,
    key: KeyEvent,
    intent: &InputIntent,
) -> Result<Option<UiEventOutcome>, TuiError> {
    if state.runtime.pending_approvals.is_empty()
        || state.ui.overlay.is_some()
        || !matches!(
            intent,
            InputIntent::CloseOverlay
                | InputIntent::MoveSelection(_)
                | InputIntent::CompleteSelection
                | InputIntent::CompleteOrSubmit
                | InputIntent::InsertChar(_)
        )
    {
        return Ok(None);
    }

    match apply_approval_key(key, state) {
        ApprovalKeyEffect::Resolve(allow) => {
            if let Some(approval) = driver.view().pending_approvals.first()
                && let Some(command) = approval_command(driver.view(), allow)
            {
                driver.send_for_owner(approval.owner.clone(), command)?;
            }
            Ok(Some(UiEventOutcome::Redraw))
        }
        ApprovalKeyEffect::Redraw => Ok(Some(UiEventOutcome::Redraw)),
        ApprovalKeyEffect::None => Ok(None),
    }
}

fn submit_composer<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    state: &mut TuiState,
) -> Result<(), TuiError> {
    let content = state.ui.input.as_str().trim().to_string();
    if content.is_empty() || open_local_picker_command(&content, state) {
        return Ok(());
    }
    let command = command_for_composer(state, &content);
    state.ui.entries.push(TuiEntry {
        label: "user".to_string(),
        body: content,
    });
    dispatch_intent(driver, command)?;
    state.ui.input.clear();
    reset_for_input_change(state);
    Ok(())
}

fn open_local_picker_command(input: &str, state: &mut TuiState) -> bool {
    state.ui.interaction_panel = match input.trim() {
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
    state.ui.input.clear();
    reset_for_input_change(state);
    true
}

fn move_interaction_selection(state: &mut TuiState, delta: i8) {
    let count = interaction_panel_choice_count(state);
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

fn interaction_selected(state: &TuiState) -> usize {
    match state.ui.interaction_panel.as_ref() {
        Some(InteractionPanel::ConnectProvider { selected, .. })
        | Some(InteractionPanel::ProviderConfig { selected, .. })
        | Some(InteractionPanel::ModelPicker { selected, .. }) => *selected,
        _ => 0,
    }
}

fn cycle_agent_focus(state: &mut TuiState) {
    if state.runtime.lanes.is_empty() {
        state.ui.focused_lane = None;
        return;
    }
    let next = state
        .ui
        .focused_lane
        .as_deref()
        .and_then(|focused| {
            state
                .runtime
                .lanes
                .iter()
                .position(|lane| lane.id == focused)
        })
        .map(|index| (index + 1) % state.runtime.lanes.len())
        .unwrap_or(0);
    state.ui.focused_lane = Some(state.runtime.lanes[next].id.clone());
}

fn set_interaction_panel_selected(state: &mut TuiState, index: usize) {
    match state.ui.interaction_panel.as_mut() {
        Some(InteractionPanel::ConnectProvider { selected, .. })
        | Some(InteractionPanel::ProviderConfig { selected, .. })
        | Some(InteractionPanel::ModelPicker { selected, .. }) => *selected = index,
        _ => {}
    }
}

fn edit_interaction_panel_text(state: &mut TuiState, value: Option<char>) {
    match state.ui.interaction_panel.as_mut() {
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
        Some(InteractionPanel::ProviderApiKey { input, .. }) => match value {
            Some(value) => input.push(value),
            None => {
                input.pop();
            }
        },
        Some(InteractionPanel::ProviderConfig { .. }) | None => {}
    }
}

fn apply_interaction_panel_selection(state: &mut TuiState) -> bool {
    let command = selected_interaction_command(state);
    state.ui.interaction_panel = None;
    if let Some(command) = command {
        // Provider/model activation remains a Core command. The overlay only
        // selects the command; it never mutates provider authority directly.
        state.ui.input.replace(command);
        reset_for_input_change(state);
        true
    } else {
        false
    }
}

fn scroll_transcript(state: &mut TuiState, delta: isize) {
    if delta > 0 {
        state.ui.transcript_scroll = state.ui.transcript_scroll.saturating_add(delta as usize);
    } else {
        state.ui.transcript_scroll = state
            .ui
            .transcript_scroll
            .saturating_sub(delta.unsigned_abs());
    }
}

fn push_composer_char(state: &mut TuiState, value: char) {
    let mut encoded = [0; 4];
    state.ui.input.insert(value.encode_utf8(&mut encoded));
    if looks_like_terminal_escape_residue(state.ui.input.as_str()) {
        state.ui.input.clear();
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
    parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        && parts.len() >= 3
}

fn command_for_composer(state: &TuiState, content: &str) -> RuntimeCommand {
    if runtime_has_active_work(&state.runtime) {
        RuntimeCommand::QueueFollowUp {
            content: content.to_string(),
        }
    } else {
        RuntimeCommand::SubmitUserInput {
            content: content.to_string(),
        }
    }
}

fn runtime_has_active_work(view: &RuntimeViewState) -> bool {
    !view.active_tool_calls.is_empty()
        || !view.pending_approvals.is_empty()
        || !view.assistant_stream.is_empty()
        || view.tasks.iter().any(|task| task.is_active())
        || view.lanes.iter().any(|lane| lane.is_active())
        || !view.queued_inputs.is_empty()
}

fn approval_command(view: &RuntimeViewState, allow: bool) -> Option<RuntimeCommand> {
    let approval = view.pending_approvals.first()?;
    Some(RuntimeCommand::RespondToApproval {
        request_id: approval.id.clone(),
        response: if allow {
            ApprovalResponse::allow_once(None)
        } else {
            ApprovalResponse::deny(None)
        },
    })
}

fn apply_pump_outcome(state: &mut TuiState, outcome: PumpOutcome) {
    match outcome {
        PumpOutcome::Idle => {}
        PumpOutcome::Applied(cursor) | PumpOutcome::Recovered(cursor) => {
            state.ui.session_id = cursor.stream_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::Duration,
    };
    use viden_core::{
        CoreClient, CoreClientError, CoreHandshake, CoreTransport, EventCursor,
        RuntimeCommandEnvelope, RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind,
        RuntimeSnapshotEnvelope, RuntimeViewState, RuntimeWireEvent, StatefulCoreClient,
        frontend_capabilities, local_core_handshake,
    };
    use viden_types::{
        AgentLaneRecord, FRONTEND_SCHEMA_V1, LaneStatus, PermissionLevel, PermissionMode,
        ReplayBatch, ReplayRequest, RuntimeOwner, RuntimeSnapshot, ToolCallView, TranscriptPage,
        TranscriptPageRequest, WorkMode,
    };

    #[derive(Default)]
    struct FakeCoreTransport {
        sent: Vec<RuntimeCommandEnvelope>,
        events: VecDeque<RuntimeEventEnvelope>,
        view: Option<RuntimeViewState>,
    }

    impl CoreTransport for FakeCoreTransport {
        fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
            Ok(local_core_handshake())
        }

        fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
            self.sent.push(command);
            Ok(())
        }

        fn recv(
            &mut self,
            _timeout: Duration,
        ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
            Ok(self.events.pop_front())
        }

        fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
            let snapshot = RuntimeSnapshot {
                cwd: PathBuf::from("/workspace"),
                provider_family: "fallback".to_string(),
                model_label: "test-local".to_string(),
                work_mode: WorkMode::Build,
                permission_mode: PermissionMode::Default,
                permission_level: PermissionLevel::Ask,
                config_summary: "fixture".to_string(),
                loaded_config_files: Vec::new(),
                startup_overrides: Vec::new(),
                ui_preferences: Default::default(),
            };
            let view = self
                .view
                .clone()
                .unwrap_or_else(|| RuntimeViewState::new(snapshot.clone()));
            let snapshot = view.snapshot.clone();
            Ok(RuntimeSnapshotEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                capabilities: frontend_capabilities(),
                cursor: EventCursor {
                    stream_id: "fixture".to_string(),
                    sequence: 0,
                },
                view,
                snapshot,
            })
        }

        fn replay(&mut self, _request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
            Ok(ReplayBatch {
                events: VecDeque::new().into(),
                next: EventCursor {
                    stream_id: "fixture".to_string(),
                    sequence: 0,
                },
                complete: true,
            })
        }

        fn transcript_page(
            &mut self,
            _request: TranscriptPageRequest,
        ) -> Result<TranscriptPage, CoreClientError> {
            Err(CoreClientError::Transport("unused".to_string()))
        }
    }

    #[derive(Default)]
    struct FakeCoreClient {
        transport: FakeCoreTransport,
        sent: Arc<Mutex<Vec<RuntimeCommandEnvelope>>>,
    }

    impl CoreClient for FakeCoreClient {
        fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
            CoreTransport::discover(&mut self.transport)
        }

        fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
            self.sent.lock().expect("sent commands").push(command);
            Ok(())
        }

        fn recv(
            &mut self,
            timeout: Duration,
        ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
            CoreTransport::recv(&mut self.transport, timeout)
        }

        fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
            CoreTransport::snapshot(&mut self.transport)
        }

        fn replay(&mut self, request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
            CoreTransport::replay(&mut self.transport, request)
        }

        fn transcript_page(
            &mut self,
            request: TranscriptPageRequest,
        ) -> Result<TranscriptPage, CoreClientError> {
            CoreTransport::transcript_page(&mut self.transport, request)
        }
    }

    #[test]
    fn submit_queue_cancel_and_approval_use_runtime_commands() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");

        let submit_id = dispatch_intent(
            &mut driver,
            RuntimeCommand::SubmitUserInput {
                content: "first".to_string(),
            },
        )
        .expect("submit");
        let queue_id = dispatch_intent(
            &mut driver,
            RuntimeCommand::QueueFollowUp {
                content: "next".to_string(),
            },
        )
        .expect("queue");
        let cancel_id =
            dispatch_intent(&mut driver, RuntimeCommand::CancelActiveTurn).expect("cancel");
        let approval_id = dispatch_intent(
            &mut driver,
            RuntimeCommand::RespondToApproval {
                request_id: "approval-1".to_string(),
                response: ApprovalResponse::allow_once(None),
            },
        )
        .expect("approval");

        assert_eq!(
            [submit_id, queue_id, cancel_id, approval_id],
            ["tui-1", "tui-2", "tui-3", "tui-4"]
        );
        let sent = sent.lock().expect("sent commands");
        assert!(matches!(
            sent[0].command,
            RuntimeCommand::SubmitUserInput { .. }
        ));
        assert!(matches!(
            sent[1].command,
            RuntimeCommand::QueueFollowUp { .. }
        ));
        assert!(matches!(sent[2].command, RuntimeCommand::CancelActiveTurn));
        assert!(matches!(
            sent[3].command,
            RuntimeCommand::RespondToApproval { .. }
        ));
    }

    #[test]
    fn bootstrap_accepts_direct_core_client() {
        let options = TuiOptions::new("startup").with_startup_check();

        run_tui(FakeCoreClient::default(), options).expect("direct CoreClient bootstrap");
    }

    #[test]
    fn command_accepted_does_not_synthesize_success() {
        let client = FakeCoreClient {
            transport: FakeCoreTransport {
                events: VecDeque::from([event(
                    1,
                    RuntimeEventKind::CommandAccepted {
                        command_id: "command-1".to_string(),
                        command: RuntimeCommand::SubmitUserInput {
                            content: "hello".to_string(),
                        },
                    },
                )]),
                ..FakeCoreTransport::default()
            },
            ..FakeCoreClient::default()
        };
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        state.ui.entries.push(TuiEntry {
            label: "user".to_string(),
            body: "hello".to_string(),
        });

        let outcome = driver.pump().expect("command receipt");
        apply_pump_outcome(&mut state, outcome);

        assert_eq!(
            state.ui.entries.len(),
            1,
            "receipt must not invent transcript facts"
        );
        assert!(driver.view().last_command.is_some());
    }

    #[test]
    fn command_rejected_reason_is_rendered() {
        let client = FakeCoreClient {
            transport: FakeCoreTransport {
                events: VecDeque::from([event(
                    1,
                    RuntimeEventKind::CommandRejected {
                        command_id: "command-1".to_string(),
                        reason: "forbidden".to_string(),
                    },
                )]),
                ..FakeCoreTransport::default()
            },
            ..FakeCoreClient::default()
        };
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        driver.pump().expect("command rejection");
        let mut state = TuiState::default();
        project_runtime_view(&mut state, driver.view(), driver.cursor());

        assert!(
            state
                .runtime
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden"))
        );
    }

    #[test]
    fn composer_stays_editable_while_events_stream() {
        let client = FakeCoreClient {
            transport: FakeCoreTransport {
                events: VecDeque::from([event(
                    1,
                    RuntimeEventKind::AssistantDelta {
                        message_id: "assistant-1".to_string(),
                        task_id: None,
                        content: "working".to_string(),
                    },
                )]),
                ..FakeCoreTransport::default()
            },
            ..FakeCoreClient::default()
        };
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        driver.pump().expect("stream event");
        project_runtime_view(&mut state, driver.view(), driver.cursor());

        state.ui.input_mode = InputMode::Insert;
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('你'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("key");

        assert_eq!(state.ui.input, "你");
        assert_eq!(driver.view().assistant_stream, "working");
    }

    fn event(sequence: u64, kind: RuntimeEventKind) -> RuntimeEventEnvelope {
        RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: Default::default(),
            cursor: EventCursor {
                stream_id: "fixture".to_string(),
                sequence,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
                sequence,
                Some(sequence),
                kind,
            )),
        }
    }

    #[test]
    fn focus_and_paste_events_force_repaint_without_becoming_input() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        assert_eq!(
            handle_ui_event(
                &mut driver,
                &mut state,
                Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
                (120, 40),
            )
            .expect("insert mode"),
            UiEventOutcome::Redraw
        );
        assert_eq!(state.ui.input_mode, InputMode::Insert);
        assert_eq!(
            handle_ui_event(
                &mut driver,
                &mut state,
                Event::Paste("first\nsecond".to_string()),
                (120, 40),
            )
            .expect("paste"),
            UiEventOutcome::Redraw
        );
        assert_eq!(state.ui.input, "first\nsecond");
        assert!(
            !super::super::state::has_active_work(&state),
            "paste must never submit"
        );

        for event in [Event::FocusLost, Event::FocusGained, Event::Resize(100, 30)] {
            assert_eq!(
                handle_ui_event(&mut driver, &mut state, event, (100, 30)).expect("repaint event"),
                UiEventOutcome::Redraw
            );
        }
        assert_eq!(state.ui.input, "first\nsecond");
    }

    #[test]
    fn paste_normalizes_crlf_preserves_leading_slash_and_never_submits() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Paste("/status\r\nnext\rline".to_string()),
            (120, 40),
        )
        .expect("paste");

        assert_eq!(state.ui.input, "/status\nnext\nline");
        assert!(sent.lock().expect("sent commands").is_empty());
    }

    #[test]
    fn enter_inside_unclosed_code_fence_inserts_newline_without_scrollback_effects() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "```rust\nfn main() {}".into();
        state.ui.transcript_scroll = 17;

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("enter");

        assert_eq!(state.ui.input, "```rust\nfn main() {}\n");
        assert_eq!(state.ui.transcript_scroll, 17);
        assert!(sent.lock().expect("sent commands").is_empty());
    }

    #[test]
    fn shift_and_alt_enter_insert_newlines_without_submitting() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "first".into();

        for modifiers in [KeyModifiers::SHIFT, KeyModifiers::ALT] {
            handle_ui_event(
                &mut driver,
                &mut state,
                Event::Key(KeyEvent::new(KeyCode::Enter, modifiers)),
                (120, 40),
            )
            .expect("modified enter");
        }

        assert_eq!(state.ui.input, "first\n\n");
        assert!(sent.lock().expect("sent commands").is_empty());
    }

    #[test]
    fn modified_enter_does_not_complete_command_palette_or_interaction_filter() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "/con".into();

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
            (120, 40),
        )
        .expect("command palette modified enter");

        assert_eq!(state.ui.input, "/con");
        assert!(state.ui.interaction_panel.is_none());

        state.ui.provider_catalog = crate::tui::state::ProviderOption::fixture();
        state.ui.interaction_panel = Some(InteractionPanel::ConnectProvider {
            search: "deep".to_string(),
            selected: 0,
        });
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)),
            (120, 40),
        )
        .expect("interaction modified enter");

        assert!(matches!(
            state.ui.interaction_panel,
            Some(InteractionPanel::ConnectProvider { ref search, selected })
                if search == "deep" && selected == 0
        ));
        assert!(sent.lock().expect("sent commands").is_empty());
    }

    #[test]
    fn modified_enter_edits_approval_composer_without_resolving_approval() {
        let mut view = RuntimeViewState::new(RuntimeSnapshot {
            cwd: PathBuf::from("/workspace"),
            provider_family: "fallback".to_string(),
            model_label: "test-local".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "fixture".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        });
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/approval-allow-deny.json"
        ))
        .expect("approval fixture");
        let envelope: RuntimeEventEnvelope =
            serde_json::from_value(fixture["events"][0].clone()).expect("approval event");
        if let viden_types::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
        let client = FakeCoreClient {
            transport: FakeCoreTransport {
                view: Some(view),
                ..FakeCoreTransport::default()
            },
            ..FakeCoreClient::default()
        };
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        project_runtime_view(&mut state, driver.view(), driver.cursor());
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "explain".into();

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
            (120, 40),
        )
        .expect("approval modified enter");

        assert_eq!(state.ui.input, "explain\n");
        assert_eq!(driver.view().pending_approvals.len(), 1);
        assert!(sent.lock().expect("sent commands").is_empty());
    }

    #[test]
    fn plain_enter_submits_a_closed_composer() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "review this".into();

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("submit");

        assert!(state.ui.input.is_empty());
        let sent = sent.lock().expect("sent commands");
        assert!(matches!(
            sent.first().map(|command| &command.command),
            Some(RuntimeCommand::SubmitUserInput { content }) if content == "review this"
        ));
    }

    #[test]
    fn composer_discards_terminal_escape_residue_instead_of_rendering_it() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");

        for value in "2;28;95;132m".chars() {
            handle_ui_event(
                &mut driver,
                &mut state,
                Event::Key(KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE)),
                (120, 40),
            )
            .expect("composer key");
        }

        assert!(state.ui.input.is_empty());
    }

    #[test]
    fn transcript_scroll_and_normal_insert_escape_survive_core_projection() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.transcript_scroll = 18;

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("scroll");
        assert_eq!(state.ui.transcript_scroll, 30);

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('草'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("draft");
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("leave insert");
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("normal key");

        assert_eq!(state.ui.input_mode, InputMode::Normal);
        assert_eq!(
            state.ui.input, "草",
            "Esc preserves the draft and Normal ignores x"
        );
        project_runtime_view(&mut state, driver.view(), driver.cursor());
        assert_eq!(
            state.ui.transcript_scroll, 30,
            "Core projection keeps scrollback"
        );
    }

    #[test]
    fn streaming_delta_does_not_steal_scrollback_when_user_scrolled_up() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.transcript_scroll = 18;
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");

        project_runtime_view(&mut state, driver.view(), driver.cursor());

        assert_eq!(state.ui.transcript_scroll, 18);
        assert_eq!(state.ui.input_mode, InputMode::Insert);
    }

    #[test]
    fn runtime_provider_turn_starts_without_blocking_ui_thread() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.runtime.active_tool_calls.push(ToolCallView {
            tool_call_id: "tool-1".to_string(),
            name: "slow".to_string(),
            input_preview: "request".to_string(),
        });
        state.ui.input_mode = InputMode::Insert;

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('继'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("composer remains responsive");

        assert_eq!(state.ui.input, "继");
        assert!(super::super::state::has_active_work(&state));
    }

    #[test]
    fn active_approval_does_not_swallow_composer_typing() {
        let mut view = RuntimeViewState::new(RuntimeSnapshot {
            cwd: PathBuf::from("/workspace"),
            provider_family: "fallback".to_string(),
            model_label: "test-local".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "fixture".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        });
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/approval-allow-deny.json"
        ))
        .expect("approval fixture");
        let envelope: RuntimeEventEnvelope =
            serde_json::from_value(fixture["events"][0].clone()).expect("approval event");
        if let viden_types::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
        let mut driver = TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport {
            view: Some(view),
            ..FakeCoreTransport::default()
        }))
        .expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("approval-time typing");

        assert_eq!(state.ui.input, "x");
        assert_eq!(driver.view().pending_approvals.len(), 1);

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Paste("\nsecond line".to_string()),
            (120, 40),
        )
        .expect("approval-time bracketed paste");

        assert_eq!(state.ui.input, "x\nsecond line");
        assert_eq!(driver.view().pending_approvals.len(), 1);
    }

    #[test]
    fn ctrl_c_cancels_the_current_work_owner_without_denying_approval() {
        let mut view = RuntimeViewState::new(RuntimeSnapshot {
            cwd: PathBuf::from("/workspace"),
            provider_family: "fallback".to_string(),
            model_label: "test-local".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "fixture".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        });
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/approval-allow-deny.json"
        ))
        .expect("approval fixture");
        let envelope: RuntimeEventEnvelope =
            serde_json::from_value(fixture["events"][0].clone()).expect("approval event");
        if let viden_types::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
        let owner = RuntimeOwner {
            workspace_id: "workspace".to_string(),
            project_id: "viden".to_string(),
            lane_id: Some("lane-review".to_string()),
            session_id: Some("session-review".to_string()),
            task_id: Some("task-review".to_string()),
            turn_id: Some("turn-review".to_string()),
        };
        view.pending_approvals[0].owner = owner.clone();
        let client = FakeCoreClient {
            transport: FakeCoreTransport {
                view: Some(view),
                ..FakeCoreTransport::default()
            },
            ..FakeCoreClient::default()
        };
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::new(driver.view().clone());
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            (120, 40),
        )
        .expect("cancel current work");

        let sent = sent.lock().expect("sent commands");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].owner, owner);
        assert!(matches!(sent[0].command, RuntimeCommand::CancelActiveTurn));
    }

    #[test]
    fn idle_ctrl_c_does_not_send_cancel_or_exit_directly() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(
            handle_ui_event(&mut driver, &mut state, ctrl_c.clone(), (120, 40))
                .expect("first idle Ctrl-C"),
            UiEventOutcome::Redraw
        );
        assert!(state.ui.idle_ctrl_c_armed);
        assert!(state.ui.overlay.is_none());
        assert_eq!(
            handle_ui_event(&mut driver, &mut state, ctrl_c, (120, 40))
                .expect("second idle Ctrl-C"),
            UiEventOutcome::Redraw
        );
        assert!(matches!(
            state.ui.overlay.as_ref().map(|overlay| overlay.kind),
            Some(OverlayKind::ExitConfirm)
        ));

        assert!(sent.lock().expect("sent commands").is_empty());
    }

    #[test]
    fn active_work_cancel_clears_stale_idle_ctrl_c_arm() {
        let client = FakeCoreClient::default();
        let sent = Arc::clone(&client.sent);
        let mut driver = TuiClientDriver::connect(client).expect("connect");
        let mut state = TuiState::default();
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

        handle_ui_event(&mut driver, &mut state, ctrl_c.clone(), (120, 40))
            .expect("arm idle Ctrl-C");
        assert!(state.ui.idle_ctrl_c_armed);

        state.runtime.active_tool_calls.push(ToolCallView {
            tool_call_id: "tool-active".to_string(),
            name: "shell".to_string(),
            input_preview: "cargo test".to_string(),
        });
        handle_ui_event(&mut driver, &mut state, ctrl_c.clone(), (120, 40))
            .expect("cancel active work");
        assert!(
            !state.ui.idle_ctrl_c_armed,
            "active-work Ctrl-C must invalidate an earlier idle arm"
        );
        assert_eq!(sent.lock().expect("sent commands").len(), 1);

        state.runtime.active_tool_calls.clear();
        handle_ui_event(&mut driver, &mut state, ctrl_c, (120, 40)).expect("new first idle Ctrl-C");
        assert!(state.ui.idle_ctrl_c_armed);
        assert!(
            state.ui.overlay.is_none(),
            "one idle Ctrl-C after cancellation must not open exit confirmation"
        );
    }

    #[test]
    fn exit_confirmation_rechecks_work_that_arrived_before_enter() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        for event in [ctrl_c.clone(), ctrl_c] {
            handle_ui_event(&mut driver, &mut state, event, (120, 40))
                .expect("open exit confirmation");
        }
        assert!(matches!(
            state.ui.overlay.as_ref().map(|overlay| overlay.kind),
            Some(OverlayKind::ExitConfirm)
        ));

        state.runtime.active_tool_calls.push(ToolCallView {
            tool_call_id: "tool-arrived".to_string(),
            name: "shell".to_string(),
            input_preview: "cargo test".to_string(),
        });
        let outcome = handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("reject stale exit confirmation");

        assert_eq!(outcome, UiEventOutcome::Redraw);
        assert!(state.ui.overlay.is_none());
    }

    #[test]
    fn escape_closes_overlay_then_selection_then_insert_and_preserves_draft() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "keep this draft".into();
        state.ui.focused_lane = Some("lane-selected".to_string());
        state.ui.overlay = Some(OverlayState::new(OverlayKind::ContextHelp));
        let escape = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        handle_ui_event(&mut driver, &mut state, escape.clone(), (120, 40)).expect("close overlay");
        assert!(state.ui.overlay.is_none());
        assert_eq!(state.ui.focused_lane.as_deref(), Some("lane-selected"));
        assert_eq!(state.ui.input_mode, InputMode::Insert);

        handle_ui_event(&mut driver, &mut state, escape.clone(), (120, 40))
            .expect("clear selection");
        assert!(state.ui.focused_lane.is_none());
        assert_eq!(state.ui.input_mode, InputMode::Insert);

        handle_ui_event(&mut driver, &mut state, escape, (120, 40)).expect("leave insert");
        assert_eq!(state.ui.input_mode, InputMode::Normal);
        assert_eq!(state.ui.input, "keep this draft");
    }

    #[test]
    fn overlay_owns_filter_navigation_and_enter_without_touching_insert_draft() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "draft stays".into();

        for event in [
            Event::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
            Event::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
            Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        ] {
            handle_ui_event(&mut driver, &mut state, event, (120, 40)).expect("overlay input");
        }

        let overlay = state.ui.overlay.as_ref().expect("lane overlay");
        assert_eq!(overlay.kind, OverlayKind::Lane);
        assert_eq!(overlay.filter, "f");
        assert_eq!(overlay.selected, 1);
        assert_eq!(state.ui.input, "draft stays");

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("overlay enter");
        assert!(state.ui.overlay.is_none());
        assert_eq!(state.ui.input_mode, InputMode::Insert);
        assert_eq!(state.ui.input, "draft stays");
    }

    #[test]
    fn paste_in_global_overlay_updates_filter_without_touching_hidden_draft() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.input_mode = InputMode::Insert;
        state.ui.input = "hidden draft".into();
        state.ui.overlay = Some(OverlayState::new(OverlayKind::Lane));

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Paste("review".to_string()),
            (120, 40),
        )
        .expect("overlay paste");

        let overlay = state.ui.overlay.as_ref().expect("lane overlay");
        assert_eq!(overlay.filter, "review");
        assert_eq!(overlay.selected, 0);
        assert_eq!(state.ui.input, "hidden draft");
    }

    #[test]
    fn paste_in_interaction_overlay_updates_its_filter_not_composer() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.input = "hidden draft".into();
        state.ui.interaction_panel = Some(InteractionPanel::ConnectProvider {
            search: String::new(),
            selected: 3,
        });

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Paste("deep".to_string()),
            (120, 40),
        )
        .expect("interaction paste");

        assert!(matches!(
            state.ui.interaction_panel,
            Some(InteractionPanel::ConnectProvider { ref search, selected })
                if search == "deep" && selected == 0
        ));
        assert_eq!(state.ui.input, "hidden draft");
    }

    #[test]
    fn provider_and_model_selector_paths_are_reachable_from_core_client_loop() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.ui.provider_catalog = crate::tui::state::ProviderOption::fixture();
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");
        state.ui.input = "/models".into();

        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("open models");

        assert!(matches!(
            state.ui.interaction_panel,
            Some(InteractionPanel::ModelPicker { .. })
        ));
        assert_eq!(effective_input_mode(&state), InputMode::Overlay);
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("close selector");
        assert!(state.ui.interaction_panel.is_none());
        assert_eq!(state.ui.input_mode, InputMode::Insert);

        state.ui.input = "/models".into();
        for _ in 0..2 {
            handle_ui_event(
                &mut driver,
                &mut state,
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                (120, 40),
            )
            .expect("open and select model");
        }
        assert!(state.ui.interaction_panel.is_none());
        assert!(state.ui.interaction_panel.is_none());
        assert!(
            state
                .ui
                .entries
                .iter()
                .all(|entry| entry.label != "assistant")
        );
    }

    #[test]
    fn rendered_shortcut_hints_match_command_and_agent_handlers() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        state.runtime.lanes = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed lanes");
        handle_ui_event(
            &mut driver,
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            (140, 40),
        )
        .expect("command shortcut");
        assert!(matches!(
            state.ui.overlay.as_ref().map(|overlay| overlay.kind),
            Some(OverlayKind::CommandPalette)
        ));

        let mut agent_state = state.clone();
        agent_state.ui.focused_lane = None;
        agent_state.ui.overlay = None;
        handle_ui_event(
            &mut driver,
            &mut agent_state,
            Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            (140, 40),
        )
        .expect("agent shortcut");
        assert_eq!(agent_state.ui.focused_lane.as_deref(), Some("L-start"));
    }

    #[test]
    fn runtime_view_projects_authoritative_frontend_facts_without_workspace_fixture() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            initial_snapshot: RuntimeSnapshot,
            events: Vec<RuntimeEventEnvelope>,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
        ))
        .expect("shared fixture");
        let mut view = RuntimeViewState::new(fixture.initial_snapshot);
        let mut approval_event = None;
        for envelope in fixture.events {
            if let viden_types::RuntimeWireEvent::Known(event) = envelope.event {
                if matches!(
                    event.kind,
                    viden_types::RuntimeEventKind::ApprovalRequested { .. }
                ) {
                    approval_event = Some(event.clone());
                }
                view.apply_event(&event);
            }
        }
        view.apply_event(&approval_event.expect("approval fixture event"));
        view.queued_inputs.push(viden_types::QueuedInputView {
            id: "queue-1".to_string(),
            content_preview: "continue with tests".to_string(),
            created_at: Some(1),
        });
        let client = StatefulCoreClient::new(FakeCoreTransport {
            view: Some(view),
            ..FakeCoreTransport::default()
        });
        let driver = TuiClientDriver::connect(client).expect("connect");

        let state = state_from_driver(&driver, &TuiOptions::new("startup"));

        assert_eq!(state.runtime.snapshot.cwd, PathBuf::from("workspace/viden"));
        assert_eq!(state.runtime.assistant_stream, "D1 cockpit state");
        assert!(!state.runtime.pending_approvals.is_empty());
        assert!(!state.runtime.errors.is_empty());
        assert_eq!(
            state.runtime.queued_inputs[0].content_preview,
            "continue with tests"
        );
        assert_eq!(state.runtime.tasks.len(), 1);
        assert!(state.runtime.cost_ledger.total_tokens > 0);
    }

    #[test]
    fn runtime_view_projects_multi_lane_fixture_facts() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            initial_snapshot: RuntimeSnapshot,
            events: Vec<RuntimeEventEnvelope>,
        }

        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/multi-lane.json"
        ))
        .expect("multi-lane shared fixture");
        let mut view = RuntimeViewState::new(fixture.initial_snapshot);
        for envelope in fixture.events {
            if let viden_types::RuntimeWireEvent::Known(event) = envelope.event {
                view.apply_event(&event);
            }
        }
        let driver = TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport {
            view: Some(view),
            ..FakeCoreTransport::default()
        }))
        .expect("connect");

        let state = state_from_driver(&driver, &TuiOptions::new("startup"));

        assert_eq!(state.runtime.lanes.len(), 2);
        let core = state
            .runtime
            .lanes
            .iter()
            .find(|lane| lane.id == "lane_core")
            .expect("core lane");
        assert_eq!(core.status, LaneStatus::Running);
        assert_eq!(
            core.role.to_string(),
            "coder",
            "role is the visible lane owner"
        );
        assert_eq!(core.task_id.as_deref(), Some("task_core"));
        assert_eq!(core.worktree.as_deref(), Some(".worktrees/lane_core"));

        let review = state
            .runtime
            .lanes
            .iter()
            .find(|lane| lane.id == "lane_review")
            .expect("review lane");
        assert_eq!(review.status, LaneStatus::WaitingApproval);
        assert_eq!(review.role.to_string(), "reviewer");
    }

    #[test]
    fn runtime_view_projects_representative_typed_lane_fixture() {
        let lanes: Vec<AgentLaneRecord> = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed-lanes shared fixture");
        let snapshot = RuntimeSnapshot {
            cwd: PathBuf::from("workspace/viden"),
            provider_family: "fallback".to_string(),
            model_label: "test-local".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "typed lanes".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        };
        let mut view = RuntimeViewState::new(snapshot);
        view.lanes = lanes;
        let driver = TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport {
            view: Some(view),
            ..FakeCoreTransport::default()
        }))
        .expect("connect");

        let state = state_from_driver(&driver, &TuiOptions::new("startup"));

        assert_eq!(state.runtime.lanes.len(), 4);
        let detached = state
            .runtime
            .lanes
            .iter()
            .find(|lane| lane.id == "L-detached")
            .expect("detached lane");
        assert_eq!(detached.status, LaneStatus::Detached);
        assert_eq!(
            detached.role.to_string(),
            "coder",
            "role is the visible lane owner"
        );
        assert_eq!(detached.task_id.as_deref(), Some("task_detached"));
        assert_eq!(detached.summary, "legacy detached lane");
    }

    #[test]
    fn typed_done_review_and_blocked_lanes_project_into_rendered_statuses() {
        let mut lanes: Vec<AgentLaneRecord> = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .expect("typed-lanes shared fixture");
        lanes.truncate(3);
        for (lane, (id, status, summary)) in lanes.iter_mut().zip([
            ("L-done", LaneStatus::Done, "result ready"),
            ("L-review", LaneStatus::WaitingApproval, "approval pending"),
            ("L-blocked", LaneStatus::Blocked, "dependency blocker"),
        ]) {
            lane.id = id.to_string();
            lane.status = status;
            lane.summary = summary.to_string();
            lane.worktree = None;
        }
        let snapshot = RuntimeSnapshot {
            cwd: PathBuf::from("workspace/viden"),
            provider_family: "fallback".to_string(),
            model_label: "test-local".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "typed lane rendering".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        };
        let mut view = RuntimeViewState::new(snapshot);
        view.lanes = lanes;
        let driver = TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport {
            view: Some(view),
            ..FakeCoreTransport::default()
        }))
        .expect("connect");
        let state = state_from_driver(&driver, &TuiOptions::new("startup"));

        let rendered = crate::tui::render::render_side_frame(&state, 100, 70);

        assert!(rendered.contains("L-done"));
        assert!(rendered.contains("done"));
        assert!(rendered.contains("L-review"));
        assert!(rendered.contains("waitingapproval"));
        assert!(rendered.contains("approval pending"));
        assert!(rendered.contains("L-blocked"));
        assert!(rendered.contains("blocked"));
        assert!(rendered.contains("blocker"));
    }

    #[test]
    fn release_manifest_declares_requested_and_effective_presentation_inputs() {
        let manifest = include_str!("../../release-manifest.toml");

        assert!(manifest.contains("locales = [\"system\", \"en\", \"zh-CN\"]"));
        assert!(manifest.contains("effective_locales = [\"en\", \"zh-CN\"]"));
        assert!(manifest.contains("modes = [\"system\", \"dark\", \"light\"]"));
        assert!(manifest.contains("effective_modes = [\"dark\", \"light\"]"));
        assert!(manifest.contains("densities = [\"compact\", \"regular\", \"comfy\"]"));
        assert!(manifest.contains("motion = [\"system\", \"reduced\", \"full\"]"));
        assert!(
            manifest
                .contains("tui_color_depth = [\"auto\", \"truecolor\", \"ansi256\", \"ansi16\"]")
        );
        assert!(
            manifest
                .contains("effective_tui_color_depth = [\"truecolor\", \"ansi256\", \"ansi16\"]")
        );
        assert!(manifest.contains("mouse_capture = false"));
    }

    #[test]
    fn startup_check_connects_core_client_without_entering_terminal() {
        let client = StatefulCoreClient::new(FakeCoreTransport::default());
        let options = TuiOptions::new("startup").with_startup_check();

        run_tui(client, options).expect("startup check");
    }

    #[test]
    fn active_turn_enter_queues_follow_up_instead_of_submitting_second_turn() {
        let mut state = TuiState::default();
        state.runtime.active_tool_calls.push(ToolCallView {
            tool_call_id: "tool-1".to_string(),
            name: "first".to_string(),
            input_preview: "{}".to_string(),
        });

        assert!(matches!(
            command_for_composer(&state, "second"),
            RuntimeCommand::QueueFollowUp { content } if content == "second"
        ));
    }

    #[test]
    fn approval_shortcut_builds_response_for_core_request_id() {
        let mut view = RuntimeViewState::new(RuntimeSnapshot {
            cwd: PathBuf::from("/workspace"),
            provider_family: "fallback".to_string(),
            model_label: "test-local".to_string(),
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: "fixture".to_string(),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: Default::default(),
        });
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/approval-allow-deny.json"
        ))
        .expect("approval fixture");
        let envelope: RuntimeEventEnvelope =
            serde_json::from_value(fixture["events"][0].clone()).expect("approval event");
        if let viden_types::RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }

        assert!(matches!(
            approval_command(&view, true),
            Some(RuntimeCommand::RespondToApproval { request_id, response })
                if request_id == "approval_allow" && response.is_allowed()
        ));
    }
}
