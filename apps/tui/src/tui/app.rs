use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use viden_core::{
    AgentLaneRecord, AgentRoute, ApprovalResponse, CoreTransport, EventCursor, ExecutionTarget,
    GateStrength, LaneStatus, RuntimeCommand, RuntimeViewState, StatefulCoreClient, TuiColorDepth,
};

use super::client::{PumpOutcome, TuiClientDriver, TuiClientError};
use super::command_palette::{
    close_on_escape, command_suggestion_index_at, complete_selected, is_command_palette_visible,
    move_selection, reset_for_input_change, select_suggestion_at, should_complete_on_enter,
};
use super::input::{ApprovalKeyEffect, apply_approval_key, close_focus_on_escape};
use super::keymap::{InputIntent, InputMode, reduce_input};
use super::modal::{
    interaction_panel_choice_count, interaction_panel_index_at, selected_interaction_command,
};
use super::state::{
    AgentTask, InteractionPanel, PendingTurn, ProviderStatus, TerminalLane, TuiEntry, TuiState,
    WorkspaceSnapshot,
};
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

pub fn run_tui<T: CoreTransport>(
    client: StatefulCoreClient<T>,
    options: TuiOptions,
) -> Result<(), TuiError> {
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
    let mut input = TuiInputController::default();
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
        if handle_ui_event(&mut driver, &mut state, &mut input, event, size)?
            == UiEventOutcome::Exit
        {
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

fn state_from_driver<T: CoreTransport>(
    driver: &TuiClientDriver<T>,
    options: &TuiOptions,
) -> TuiState {
    let snapshot = &driver.view().snapshot;
    let mut state = TuiState {
        session_id: driver.cursor().stream_id.clone(),
        provider: snapshot.provider_family.clone(),
        model: snapshot.model_label.clone(),
        provider_status: ProviderStatus::configured(),
        theme_name: ui_profile_label(&snapshot.ui_preferences),
        workspace: WorkspaceSnapshot::from_core_cwd(snapshot.cwd.clone()),
        ..TuiState::default()
    };
    state.provider_status.work_mode = snapshot.work_mode;
    state.provider_status.permission_level = snapshot.permission_level;
    state.entries.push(TuiEntry {
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
    state.session_id = cursor.stream_id.clone();
    state.provider = view.snapshot.provider_family.clone();
    state.model = view.snapshot.model_label.clone();
    state.workspace = WorkspaceSnapshot::from_core_cwd(view.snapshot.cwd.clone());
    state.provider_status.work_mode = view.snapshot.work_mode;
    state.provider_status.permission_level = view.snapshot.permission_level;

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
    let token_cost = view.token_cost.as_ref();
    state.provider_status.last_input_tokens = token_cost.map(|cost| cost.input_tokens);
    state.provider_status.last_output_tokens = token_cost.map(|cost| cost.output_tokens);
    state.provider_status.last_total_tokens = token_cost.map(|cost| cost.total_tokens);
    state.provider_status.last_cost_micro_usd = token_cost.and_then(|cost| cost.cost_micro_usd);
    state.provider_status.total_tokens = view.cost_ledger.total_tokens;
    state.provider_status.total_cost_micro_usd = view
        .cost_ledger
        .total_actual_cost_micro_usd
        .or(Some(view.cost_ledger.total_estimated_cost_micro_usd));
    state.provider_status.last_event_count = cursor.sequence as usize;
    state.provider_status.context_window = view
        .context
        .as_ref()
        .map(|context| format!("{}/{}", context.estimated_tokens, context.hard_token_limit))
        .unwrap_or_else(|| "-".to_string());

    state.streaming_assistant =
        (!view.assistant_stream.is_empty()).then(|| view.assistant_stream.clone());
    state.runtime_tasks = view.tasks.iter().map(agent_task_from_core).collect();
    state.lanes = view.lanes.iter().map(terminal_lane_from_core).collect();

    state.entries.retain(|entry| {
        matches!(entry.label.as_str(), "system" | "user") && !entry.body.starts_with("runtime:")
    });
    state
        .entries
        .extend(view.active_tool_calls.iter().map(|tool| TuiEntry {
            label: "tool-call".to_string(),
            body: format!("{}\n{}", tool.name, tool.input_preview),
        }));
    state
        .entries
        .extend(view.latest_evidence.iter().map(|evidence| TuiEntry {
            label: "tool-result".to_string(),
            body: evidence.summary.clone(),
        }));
    state
        .entries
        .extend(view.pending_approvals.iter().map(|approval| TuiEntry {
            label: "approval".to_string(),
            body: format!(
                "Permission request for `{}`\npath: {}\n{}\n{}\nPress y to approve or n to deny.",
                approval.tool_name,
                approval.target.display,
                approval.message,
                approval.input_preview
            ),
        }));
    state
        .entries
        .extend(view.errors.iter().map(|error| TuiEntry {
            label: "error".to_string(),
            body: format!(
            "{}{}",
            error.message,
            error
                .hint
                .as_deref()
                .map(|hint| format!("\n{hint}"))
                .unwrap_or_default()
        ),
        }));
    state
        .entries
        .extend(view.merge_gates.iter().map(|gate| TuiEntry {
            label: "system".to_string(),
            body: format!("runtime: merge gate {} {:?}", gate.gate_id, gate.status),
        }));

    let has_active_runtime = !view.active_tool_calls.is_empty()
        || !view.pending_approvals.is_empty()
        || view.tasks.iter().any(|task| task.is_active())
        || view.lanes.iter().any(AgentLaneRecord::is_active)
        || !view.queued_inputs.is_empty()
        || !view.assistant_stream.is_empty();
    if has_active_runtime {
        let mut turn = state.pending_turn.take().unwrap_or_else(|| {
            PendingTurn::new(
                &state.session_id,
                &state.provider,
                &state.model,
                "runtime activity",
                &state.workspace.display_root,
            )
        });
        turn.queued_inputs = view
            .queued_inputs
            .iter()
            .map(|input| input.content_preview.clone())
            .collect();
        if !view.pending_approvals.is_empty() {
            turn.phase = "approval required".to_string();
            turn.next_action = "approve or deny".to_string();
        } else if !view.active_tool_calls.is_empty() {
            turn.phase = "running tool".to_string();
            turn.next_action = "wait".to_string();
        } else {
            turn.phase = "streaming".to_string();
            turn.next_action = "wait".to_string();
        }
        state.pending_turn = Some(turn);
    } else {
        state.pending_turn = None;
    }
}

fn terminal_lane_from_core(lane: &AgentLaneRecord) -> TerminalLane {
    TerminalLane {
        id: lane.id.clone(),
        tool: lane.role.to_string(),
        title: lane
            .task_id
            .clone()
            .unwrap_or_else(|| format!("{} lane", lane.role)),
        status: lane_status_name(lane.status).to_string(),
        target: format!(
            "{}/{} · {}",
            lane_route_name(lane.route),
            execution_target_name(&lane.target),
            gate_strength_name(lane.gate_strength)
        ),
        progress: u8::from(matches!(lane.status, LaneStatus::Done)) * 100,
        summary: lane.summary.clone(),
        worktree: lane.worktree.as_deref().map(std::path::PathBuf::from),
    }
}

fn lane_status_name(status: LaneStatus) -> &'static str {
    match status {
        LaneStatus::Draft => "draft",
        LaneStatus::Queued => "queued",
        LaneStatus::Starting => "starting",
        LaneStatus::Running => "running",
        LaneStatus::WaitingApproval => "waiting_approval",
        LaneStatus::NeedsInput => "needs_input",
        LaneStatus::Blocked => "blocked",
        LaneStatus::Attached => "attached",
        LaneStatus::Detached => "detached",
        LaneStatus::Done => "done",
        LaneStatus::Failed => "failed",
        LaneStatus::Cancelled => "cancelled",
        LaneStatus::Archived => "archived",
    }
}

fn lane_route_name(route: AgentRoute) -> &'static str {
    match route {
        AgentRoute::BuiltIn => "built_in",
        AgentRoute::Acp => "acp",
        AgentRoute::Terminal => "terminal",
        AgentRoute::Tmux => "tmux",
    }
}

fn execution_target_name(target: &ExecutionTarget) -> String {
    match target {
        ExecutionTarget::Local => "local".to_string(),
        ExecutionTarget::Ssh { host } => format!("ssh:{host}"),
    }
}

fn gate_strength_name(gate: GateStrength) -> &'static str {
    match gate {
        GateStrength::Full => "full",
        GateStrength::Cooperative => "cooperative",
        GateStrength::Containment => "containment",
    }
}

fn agent_task_from_core(task: &viden_core::AgentTaskRecord) -> AgentTask {
    AgentTask {
        id: task.id.clone(),
        parent_id: task.parent_id.clone(),
        agent: task.role.to_string(),
        kind: task.kind.to_string(),
        transport: format!("{:?}", task.route).to_ascii_lowercase(),
        title: task.title.clone(),
        status: task.status.as_str().to_string(),
        progress: task.progress,
        activity: task.activity.clone(),
        summary: task.summary.clone(),
        evidence: task.evidence.clone(),
        next_action: task.next_action.clone(),
        started_at: task.started_at,
        updated_at: task.updated_at,
        workspace: task.workspace.clone(),
        permissions: task.permissions.clone(),
        decision: task.decision.clone(),
        result: task.result.clone(),
        resume_handle: task.resume_handle.clone(),
        pid: task.pid,
    }
}

pub(super) fn dispatch_intent<T: CoreTransport>(
    driver: &mut TuiClientDriver<T>,
    command: RuntimeCommand,
) -> Result<String, TuiClientError> {
    driver.send(command)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiEventOutcome {
    Redraw,
    Exit,
}

#[derive(Debug, Clone, Copy)]
struct TuiInputController {
    mode: InputMode,
}

impl Default for TuiInputController {
    fn default() -> Self {
        Self {
            mode: InputMode::Normal,
        }
    }
}

impl TuiInputController {
    #[cfg(test)]
    fn mode(self) -> InputMode {
        self.mode
    }
}

fn effective_input_mode(controller: &TuiInputController, state: &TuiState) -> InputMode {
    if state.interaction_panel.is_some()
        || state.focused_lane.is_some()
        || is_command_palette_visible(state)
    {
        InputMode::Overlay
    } else {
        controller.mode
    }
}

fn handle_ui_event<T: CoreTransport>(
    driver: &mut TuiClientDriver<T>,
    state: &mut TuiState,
    controller: &mut TuiInputController,
    event: Event,
    terminal_size: (u16, u16),
) -> Result<UiEventOutcome, TuiError> {
    match event {
        Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => Ok(UiEventOutcome::Redraw),
        Event::Paste(text) => {
            if driver.view().pending_approvals.is_empty()
                && matches!(
                    effective_input_mode(controller, state),
                    InputMode::Insert | InputMode::Overlay
                )
            {
                if state.interaction_panel.is_some() {
                    for value in text.chars() {
                        edit_interaction_panel_text(state, Some(value));
                    }
                } else {
                    state.input.push_str(&text);
                    reset_for_input_change(state);
                }
            }
            Ok(UiEventOutcome::Redraw)
        }
        Event::Mouse(mouse) => {
            handle_mouse(mouse, state, terminal_size);
            Ok(UiEventOutcome::Redraw)
        }
        Event::Key(key) => handle_ui_key(driver, state, controller, key),
    }
}

fn handle_ui_key<T: CoreTransport>(
    driver: &mut TuiClientDriver<T>,
    state: &mut TuiState,
    controller: &mut TuiInputController,
    key: KeyEvent,
) -> Result<UiEventOutcome, TuiError> {
    let has_active_work = state.pending_turn.is_some();
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        dispatch_intent(driver, RuntimeCommand::CancelActiveTurn)?;
        state.entries.push(TuiEntry {
            label: "command".to_string(),
            body: "cancel requested".to_string(),
        });
        return Ok(UiEventOutcome::Redraw);
    }

    if !driver.view().pending_approvals.is_empty() {
        match apply_approval_key(key, state) {
            ApprovalKeyEffect::Resolve(allow) => {
                if let Some(command) = approval_command(driver.view(), allow) {
                    dispatch_intent(driver, command)?;
                }
                return Ok(UiEventOutcome::Redraw);
            }
            ApprovalKeyEffect::Redraw => return Ok(UiEventOutcome::Redraw),
            ApprovalKeyEffect::None => {}
        }
    }

    let mode = if driver.view().pending_approvals.is_empty() {
        effective_input_mode(controller, state)
    } else {
        InputMode::Overlay
    };
    let intent = reduce_input(mode, key, has_active_work);
    apply_input_intent(driver, state, controller, key, intent)
}

fn apply_input_intent<T: CoreTransport>(
    driver: &mut TuiClientDriver<T>,
    state: &mut TuiState,
    controller: &mut TuiInputController,
    key: KeyEvent,
    intent: InputIntent,
) -> Result<UiEventOutcome, TuiError> {
    match intent {
        InputIntent::None => {}
        InputIntent::EnterInsert => controller.mode = InputMode::Insert,
        InputIntent::LeaveInsert => controller.mode = InputMode::Normal,
        InputIntent::CloseOverlay => {
            if state.interaction_panel.take().is_none()
                && !close_focus_on_escape(key, state)
                && !close_on_escape(key, state)
            {
                controller.mode = InputMode::Normal;
            }
        }
        InputIntent::CancelCurrentWork => unreachable!("global cancel is handled first"),
        InputIntent::OpenCommandPalette => {
            controller.mode = InputMode::Insert;
            state.input = "/".to_string();
            reset_for_input_change(state);
        }
        InputIntent::ContextHelp => {
            controller.mode = InputMode::Insert;
            state.input = "/help ".to_string();
            reset_for_input_change(state);
        }
        InputIntent::Exit => return Ok(UiEventOutcome::Exit),
        InputIntent::InsertChar(value) => {
            if state.interaction_panel.is_some() {
                edit_interaction_panel_text(state, Some(value));
            } else {
                push_composer_char(state, value);
            }
        }
        InputIntent::Backspace => {
            if state.interaction_panel.is_some() {
                edit_interaction_panel_text(state, None);
            } else {
                state.input.pop();
                reset_for_input_change(state);
            }
        }
        InputIntent::Submit => submit_composer(driver, state)?,
        InputIntent::MoveSelection(delta) => {
            if state.interaction_panel.is_some() {
                move_interaction_selection(state, delta);
            } else {
                move_selection(state, delta);
            }
        }
        InputIntent::CompleteSelection => {
            if state.interaction_panel.is_none() {
                complete_selected(state);
            }
        }
        InputIntent::CompleteOrSubmit => {
            if state.interaction_panel.is_some() {
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
        InputIntent::ScrollToStart => state.transcript_scroll = usize::MAX / 2,
        InputIntent::ScrollToEnd => state.transcript_scroll = 0,
    }
    Ok(UiEventOutcome::Redraw)
}

fn handle_mouse(mouse: MouseEvent, state: &mut TuiState, terminal_size: (u16, u16)) -> bool {
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
    if state.interaction_panel.is_some() {
        let Some(index) = interaction_panel_index_at(
            state,
            mouse.column,
            mouse.row,
            terminal_size.0,
            terminal_size.1,
            38,
        ) else {
            return false;
        };
        set_interaction_panel_selected(state, index);
        if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
            apply_interaction_panel_selection(state);
        }
        return true;
    }
    let Some(index) = command_suggestion_index_at(
        state,
        mouse.column,
        mouse.row,
        terminal_size.0,
        terminal_size.1,
    ) else {
        return false;
    };
    let selected = select_suggestion_at(state, index);
    if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
        complete_selected(state);
    }
    selected
}

fn submit_composer<T: CoreTransport>(
    driver: &mut TuiClientDriver<T>,
    state: &mut TuiState,
) -> Result<(), TuiError> {
    let content = state.input.trim().to_string();
    if content.is_empty() || open_local_picker_command(&content, state) {
        return Ok(());
    }
    let command = command_for_composer(state, &content);
    if state.pending_turn.is_none() {
        state.pending_turn = Some(PendingTurn::for_input(&content));
    }
    state.entries.push(TuiEntry {
        label: "user".to_string(),
        body: content,
    });
    dispatch_intent(driver, command)?;
    state.input.clear();
    reset_for_input_change(state);
    Ok(())
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
    state.interaction_panel = None;
    if let Some(command) = command {
        // Provider/model activation remains a Core command. The overlay only
        // selects the command; it never mutates provider authority directly.
        state.input = command;
        reset_for_input_change(state);
        true
    } else {
        false
    }
}

fn scroll_transcript(state: &mut TuiState, delta: isize) {
    if delta > 0 {
        state.transcript_scroll = state.transcript_scroll.saturating_add(delta as usize);
    } else {
        state.transcript_scroll = state.transcript_scroll.saturating_sub(delta.unsigned_abs());
    }
}

fn push_composer_char(state: &mut TuiState, value: char) {
    state.input.push(value);
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
    parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        && parts.len() >= 3
}

fn command_for_composer(state: &TuiState, content: &str) -> RuntimeCommand {
    if state.pending_turn.is_some() {
        RuntimeCommand::QueueFollowUp {
            content: content.to_string(),
        }
    } else {
        RuntimeCommand::SubmitUserInput {
            content: content.to_string(),
        }
    }
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
            state.session_id = cursor.stream_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{MouseEvent, MouseEventKind};
    use std::{collections::VecDeque, path::PathBuf, time::Duration};
    use viden_core::{
        CoreClientError, CoreHandshake, CoreTransport, EventCursor, RuntimeCommandEnvelope,
        RuntimeEventEnvelope, RuntimeSnapshotEnvelope, RuntimeViewState, StatefulCoreClient,
        frontend_capabilities, local_core_handshake,
    };
    use viden_types::{
        AgentLaneRecord, FRONTEND_SCHEMA_V1, PermissionLevel, PermissionMode, ReplayBatch,
        ReplayRequest, RuntimeSnapshot, TranscriptPage, TranscriptPageRequest, WorkMode,
    };

    #[derive(Default)]
    struct FakeCoreTransport {
        sent: Vec<RuntimeCommandEnvelope>,
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
            Ok(None)
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

    #[test]
    fn submit_queue_cancel_and_approval_use_runtime_commands() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");

        let id = dispatch_intent(
            &mut driver,
            RuntimeCommand::QueueFollowUp {
                content: "next".to_string(),
            },
        )
        .expect("send");

        assert_eq!(id, "tui-1");
    }

    #[test]
    fn command_accepted_does_not_synthesize_success() {
        let mut state = TuiState {
            pending_turn: Some(PendingTurn::for_input("hello")),
            ..TuiState::default()
        };

        apply_pump_outcome(&mut state, PumpOutcome::Idle);

        assert!(state.pending_turn.is_some());
    }

    #[test]
    fn command_rejected_reason_is_rendered() {
        let mut state = TuiState::default();

        state.entries.push(TuiEntry {
            label: "error".to_string(),
            body: "command rejected: forbidden".to_string(),
        });

        assert!(state.entries[0].body.contains("forbidden"));
    }

    #[test]
    fn composer_stays_editable_while_events_stream() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();

        let mut controller = TuiInputController {
            mode: InputMode::Insert,
        };
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('你'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("key");

        assert_eq!(state.input, "你");
    }

    #[test]
    fn focus_and_paste_events_force_repaint_without_becoming_input() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        let mut controller = TuiInputController::default();

        assert_eq!(
            handle_ui_event(
                &mut driver,
                &mut state,
                &mut controller,
                Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
                (120, 40),
            )
            .expect("insert mode"),
            UiEventOutcome::Redraw
        );
        assert_eq!(controller.mode(), InputMode::Insert);
        assert_eq!(
            handle_ui_event(
                &mut driver,
                &mut state,
                &mut controller,
                Event::Paste("first\nsecond".to_string()),
                (120, 40),
            )
            .expect("paste"),
            UiEventOutcome::Redraw
        );
        assert_eq!(state.input, "first\nsecond");
        assert!(state.pending_turn.is_none(), "paste must never submit");

        for event in [Event::FocusLost, Event::FocusGained, Event::Resize(100, 30)] {
            assert_eq!(
                handle_ui_event(&mut driver, &mut state, &mut controller, event, (100, 30),)
                    .expect("repaint event"),
                UiEventOutcome::Redraw
            );
        }
        assert_eq!(state.input, "first\nsecond");
    }

    #[test]
    fn composer_discards_terminal_escape_residue_instead_of_rendering_it() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        let mut controller = TuiInputController::default();
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");

        for value in "2;28;95;132m".chars() {
            handle_ui_event(
                &mut driver,
                &mut state,
                &mut controller,
                Event::Key(KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE)),
                (120, 40),
            )
            .expect("composer key");
        }

        assert!(state.input.is_empty());
    }

    #[test]
    fn transcript_scroll_and_normal_insert_escape_survive_core_projection() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        let mut controller = TuiInputController::default();
        state.transcript_scroll = 18;

        let scroll = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Mouse(scroll),
            (120, 40),
        )
        .expect("scroll");
        assert_eq!(state.transcript_scroll, 22);

        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('草'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("draft");
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("leave insert");
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("normal key");

        assert_eq!(controller.mode(), InputMode::Normal);
        assert_eq!(
            state.input, "草",
            "Esc preserves the draft and Normal ignores x"
        );
        project_runtime_view(&mut state, driver.view(), driver.cursor());
        assert_eq!(
            state.transcript_scroll, 22,
            "Core projection keeps scrollback"
        );
    }

    #[test]
    fn streaming_delta_does_not_steal_scrollback_when_user_scrolled_up() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState::default();
        let mut controller = TuiInputController::default();
        state.transcript_scroll = 18;
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");

        project_runtime_view(&mut state, driver.view(), driver.cursor());

        assert_eq!(state.transcript_scroll, 18);
        assert_eq!(controller.mode(), InputMode::Insert);
    }

    #[test]
    fn runtime_provider_turn_starts_without_blocking_ui_thread() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState {
            pending_turn: Some(PendingTurn::for_input("slow request")),
            ..TuiState::default()
        };
        let mut controller = TuiInputController {
            mode: InputMode::Insert,
        };

        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('继'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("composer remains responsive");

        assert_eq!(state.input, "继");
        assert!(state.pending_turn.is_some());
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
        let mut controller = TuiInputController {
            mode: InputMode::Insert,
        };

        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("approval-time typing");

        assert_eq!(state.input, "x");
        assert_eq!(driver.view().pending_approvals.len(), 1);
    }

    #[test]
    fn provider_and_model_selector_paths_are_reachable_from_core_client_loop() {
        let mut driver =
            TuiClientDriver::connect(StatefulCoreClient::new(FakeCoreTransport::default()))
                .expect("connect");
        let mut state = TuiState {
            provider_catalog: crate::tui::state::ProviderOption::fixture(),
            ..TuiState::default()
        };
        let mut controller = TuiInputController::default();
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("insert mode");
        state.input = "/models".to_string();

        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("open models");

        assert!(matches!(
            state.interaction_panel,
            Some(InteractionPanel::ModelPicker { .. })
        ));
        assert_eq!(
            effective_input_mode(&controller, &state),
            InputMode::Overlay
        );
        handle_ui_event(
            &mut driver,
            &mut state,
            &mut controller,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            (120, 40),
        )
        .expect("close selector");
        assert!(state.interaction_panel.is_none());
        assert_eq!(controller.mode(), InputMode::Insert);

        state.input = "/models".to_string();
        for _ in 0..2 {
            handle_ui_event(
                &mut driver,
                &mut state,
                &mut controller,
                Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                (120, 40),
            )
            .expect("open and select model");
        }
        assert!(state.interaction_panel.is_none());
        assert!(
            state
                .entries
                .iter()
                .any(|entry| { entry.label == "user" && entry.body.starts_with("/models ") })
        );
        assert!(
            state.pending_turn.is_some(),
            "selection dispatches through Core"
        );
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

        assert_eq!(state.workspace.root, PathBuf::from("workspace/viden"));
        assert_eq!(
            state.streaming_assistant.as_deref(),
            Some("D1 cockpit state")
        );
        assert!(state.entries.iter().any(|entry| entry.label == "approval"));
        assert!(state.entries.iter().any(|entry| entry.label == "error"));
        assert_eq!(
            state
                .pending_turn
                .as_ref()
                .expect("active runtime facts")
                .queued_inputs,
            vec!["continue with tests".to_string()]
        );
        assert_eq!(state.runtime_tasks.len(), 1);
        assert!(state.provider_status.total_tokens > 0);
        assert!(state.provider_status.total_cost_micro_usd.is_some());
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

        assert_eq!(state.lanes.len(), 2);
        let core = state
            .lanes
            .iter()
            .find(|lane| lane.id == "lane_core")
            .expect("core lane");
        assert_eq!(core.status, "running");
        assert_eq!(core.tool, "coder", "role is the visible lane owner");
        assert_eq!(core.title, "task_core");
        assert_eq!(core.target, "terminal/local · containment");
        assert_eq!(
            core.worktree.as_deref(),
            Some(std::path::Path::new(".worktrees/lane_core"))
        );

        let review = state
            .lanes
            .iter()
            .find(|lane| lane.id == "lane_review")
            .expect("review lane");
        assert_eq!(review.status, "waiting_approval");
        assert_eq!(review.tool, "reviewer");
        assert_eq!(review.target, "acp/ssh:review.example.test · cooperative");
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

        assert_eq!(state.lanes.len(), 4);
        let detached = state
            .lanes
            .iter()
            .find(|lane| lane.id == "L-detached")
            .expect("detached lane");
        assert_eq!(detached.status, "detached");
        assert_eq!(detached.tool, "coder", "role is the visible lane owner");
        assert_eq!(detached.title, "task_detached");
        assert_eq!(detached.target, "tmux/local · containment");
        assert_eq!(detached.summary, "legacy detached lane");
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
    }

    #[test]
    fn startup_check_connects_core_client_without_entering_terminal() {
        let client = StatefulCoreClient::new(FakeCoreTransport::default());
        let options = TuiOptions::new("startup").with_startup_check();

        run_tui(client, options).expect("startup check");
    }

    #[test]
    fn active_turn_enter_queues_follow_up_instead_of_submitting_second_turn() {
        let state = TuiState {
            pending_turn: Some(PendingTurn::for_input("first")),
            ..TuiState::default()
        };

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
