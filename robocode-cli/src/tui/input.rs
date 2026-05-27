use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use robocode_types::{ApprovalResponse, PermissionPrompt};

use super::modal::{
    ApprovalAction, DEFAULT_APPROVAL_FOCUS, approval_action_at, focused_approval_action,
    move_approval_focus, set_approval_focus_for_action,
};
use super::state::{TuiEntry, TuiState};
use super::terminal::TerminalGuard;

const APPROVAL_RIGHT_RAIL_WIDTH: usize = 38;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalKeyEffect {
    None,
    Redraw,
    Resolve(bool),
}

pub(super) fn close_focus_on_escape(key: KeyEvent, state: &mut TuiState) -> bool {
    if key.code != KeyCode::Esc || state.focused_lane.is_none() {
        return false;
    }
    state.focused_lane = None;
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: "Closed lane detail focus.".to_string(),
    });
    true
}

pub(super) fn prompt_for_tui_approval(
    prompt: PermissionPrompt,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> ApprovalResponse {
    state.approval_focus = DEFAULT_APPROVAL_FOCUS;
    state.approval_apply_all = false;
    state.entries.push(TuiEntry {
        label: "approval".to_string(),
        body: format!(
            "Permission request for `{}`\n{}\n{}\nPress y to allow, n/Esc to deny. Tab/arrows move, Enter activates, click buttons.",
            prompt.tool_name, prompt.message, prompt.input_preview
        ),
    });
    let _ = terminal.draw(state);
    loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                if let Some(response) = handle_approval_key(key, &prompt, state, terminal) {
                    return response;
                }
            }
            Ok(Event::Mouse(mouse)) => {
                if let Some(response) = handle_approval_mouse(mouse, &prompt, state, terminal) {
                    return response;
                }
            }
            Ok(Event::Resize(_, _)) => {
                let _ = terminal.draw(state);
            }
            _ => continue,
        }
    }
}

fn handle_approval_key(
    key: KeyEvent,
    prompt: &PermissionPrompt,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Option<ApprovalResponse> {
    match apply_approval_key(key, state) {
        ApprovalKeyEffect::None => None,
        ApprovalKeyEffect::Redraw => {
            let _ = terminal.draw(state);
            None
        }
        ApprovalKeyEffect::Resolve(approved) => {
            Some(resolve_approval(approved, prompt, state, terminal))
        }
    }
}

fn apply_approval_key(key: KeyEvent, state: &mut TuiState) -> ApprovalKeyEffect {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => ApprovalKeyEffect::Resolve(true),
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => ApprovalKeyEffect::Resolve(false),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            ApprovalKeyEffect::Resolve(false)
        }
        KeyCode::Char(' ') => {
            if focused_approval_action(state) != ApprovalAction::ToggleApplyAll {
                return ApprovalKeyEffect::None;
            }
            state.approval_apply_all = !state.approval_apply_all;
            ApprovalKeyEffect::Redraw
        }
        KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
            move_approval_focus(state, 1);
            ApprovalKeyEffect::Redraw
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Up => {
            move_approval_focus(state, -1);
            ApprovalKeyEffect::Redraw
        }
        KeyCode::Enter => apply_approval_action(focused_approval_action(state), state),
        KeyCode::Char('d') | KeyCode::Char('D') => {
            set_approval_focus_for_action(state, ApprovalAction::Diff);
            ApprovalKeyEffect::Redraw
        }
        _ => ApprovalKeyEffect::None,
    }
}

fn handle_approval_mouse(
    mouse: MouseEvent,
    prompt: &PermissionPrompt,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Option<ApprovalResponse> {
    if !matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
    ) {
        return None;
    }
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let action = approval_action_at(
        state,
        mouse.column,
        mouse.row,
        width,
        height,
        APPROVAL_RIGHT_RAIL_WIDTH,
    )?;
    set_approval_focus_for_action(state, action);
    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        let _ = terminal.draw(state);
        return None;
    }
    activate_approval_action(action, prompt, state, terminal)
}

fn activate_approval_action(
    action: ApprovalAction,
    prompt: &PermissionPrompt,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> Option<ApprovalResponse> {
    match apply_approval_action(action, state) {
        ApprovalKeyEffect::None => None,
        ApprovalKeyEffect::Redraw => {
            let _ = terminal.draw(state);
            None
        }
        ApprovalKeyEffect::Resolve(approved) => {
            Some(resolve_approval(approved, prompt, state, terminal))
        }
    }
}

fn apply_approval_action(action: ApprovalAction, state: &mut TuiState) -> ApprovalKeyEffect {
    match action {
        ApprovalAction::ToggleApplyAll => {
            state.approval_apply_all = !state.approval_apply_all;
            ApprovalKeyEffect::Redraw
        }
        ApprovalAction::Deny => ApprovalKeyEffect::Resolve(false),
        ApprovalAction::Approve => ApprovalKeyEffect::Resolve(true),
        ApprovalAction::Diff => ApprovalKeyEffect::Redraw,
    }
}

fn resolve_approval(
    approved: bool,
    prompt: &PermissionPrompt,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> ApprovalResponse {
    let verb = if approved { "Approved" } else { "Denied" };
    let apply_all = if state.approval_apply_all {
        " apply_all=true"
    } else {
        ""
    };
    state.entries.push(TuiEntry {
        label: "approval".to_string(),
        body: format!("{verb} `{}`.{apply_all}", prompt.tool_name),
    });
    state.approval_focus = DEFAULT_APPROVAL_FOCUS;
    state.approval_apply_all = false;
    let _ = terminal.draw(state);
    ApprovalResponse {
        approved,
        feedback: None,
    }
}

pub(super) fn should_exit(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[cfg(test)]
mod tests {
    use super::super::state::{ProviderStatus, TerminalLane, WorkspaceSnapshot};
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn state_with_focus() -> TuiState {
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
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
            tasks: Vec::new(),
            runtime_tasks: Vec::new(),
            memory: Vec::new(),
            screens: Vec::new(),
            lanes: TerminalLane::preview_lanes(),
            lane_store: None,
            focused_lane: Some("L1".to_string()),
        }
    }

    #[test]
    fn escape_closes_focus_before_exit() {
        let mut state = state_with_focus();

        assert!(close_focus_on_escape(key(KeyCode::Esc), &mut state));

        assert_eq!(state.focused_lane, None);
        assert!(state.entries[0].body.contains("Closed lane detail focus"));
    }

    #[test]
    fn escape_without_focus_keeps_exit_behavior_available() {
        let mut state = state_with_focus();
        state.focused_lane = None;

        assert!(!close_focus_on_escape(key(KeyCode::Esc), &mut state));
        assert!(should_exit(key(KeyCode::Esc)));
    }

    #[test]
    fn approval_space_toggles_apply_all_only_when_checkbox_is_focused() {
        let mut state = state_with_focus();
        state.approval_focus = DEFAULT_APPROVAL_FOCUS;

        assert_eq!(
            apply_approval_key(key(KeyCode::Char(' ')), &mut state),
            ApprovalKeyEffect::None
        );
        assert!(!state.approval_apply_all);

        set_approval_focus_for_action(&mut state, ApprovalAction::ToggleApplyAll);

        assert_eq!(
            apply_approval_key(key(KeyCode::Char(' ')), &mut state),
            ApprovalKeyEffect::Redraw
        );
        assert!(state.approval_apply_all);
    }

    #[test]
    fn approval_enter_activates_default_approve_focus() {
        let mut state = state_with_focus();
        state.approval_focus = DEFAULT_APPROVAL_FOCUS;

        assert_eq!(
            apply_approval_key(key(KeyCode::Enter), &mut state),
            ApprovalKeyEffect::Resolve(true)
        );
    }

    #[test]
    fn approval_keyboard_focus_reaches_deny_diff_and_approve() {
        let mut state = state_with_focus();
        state.approval_focus = DEFAULT_APPROVAL_FOCUS;

        assert_eq!(
            focused_approval_action(&state),
            ApprovalAction::Approve,
            "approval should default to the low-friction approve action"
        );

        assert_eq!(
            apply_approval_key(key(KeyCode::Left), &mut state),
            ApprovalKeyEffect::Redraw
        );
        assert_eq!(focused_approval_action(&state), ApprovalAction::Diff);

        assert_eq!(
            apply_approval_key(key(KeyCode::Left), &mut state),
            ApprovalKeyEffect::Redraw
        );
        assert_eq!(focused_approval_action(&state), ApprovalAction::Deny);

        assert_eq!(
            apply_approval_key(key(KeyCode::Enter), &mut state),
            ApprovalKeyEffect::Resolve(false)
        );

        set_approval_focus_for_action(&mut state, ApprovalAction::Approve);
        assert_eq!(
            apply_approval_key(key(KeyCode::Enter), &mut state),
            ApprovalKeyEffect::Resolve(true)
        );
    }

    #[test]
    fn approval_shortcuts_resolve_without_focus_hopping() {
        let mut state = state_with_focus();
        set_approval_focus_for_action(&mut state, ApprovalAction::Deny);

        assert_eq!(
            apply_approval_key(key(KeyCode::Char('y')), &mut state),
            ApprovalKeyEffect::Resolve(true)
        );
        assert_eq!(focused_approval_action(&state), ApprovalAction::Deny);

        assert_eq!(
            apply_approval_key(key(KeyCode::Char('n')), &mut state),
            ApprovalKeyEffect::Resolve(false)
        );
        assert_eq!(
            apply_approval_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut state
            ),
            ApprovalKeyEffect::Resolve(false)
        );
    }
}
