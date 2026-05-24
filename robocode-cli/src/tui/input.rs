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
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            Some(resolve_approval(true, prompt, state, terminal))
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            Some(resolve_approval(false, prompt, state, terminal))
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(resolve_approval(false, prompt, state, terminal))
        }
        KeyCode::Char(' ') => {
            state.approval_apply_all = !state.approval_apply_all;
            let _ = terminal.draw(state);
            None
        }
        KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
            move_approval_focus(state, 1);
            let _ = terminal.draw(state);
            None
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Up => {
            move_approval_focus(state, -1);
            let _ = terminal.draw(state);
            None
        }
        KeyCode::Enter => {
            activate_approval_action(focused_approval_action(state), prompt, state, terminal)
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            set_approval_focus_for_action(state, ApprovalAction::Diff);
            let _ = terminal.draw(state);
            None
        }
        _ => None,
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
    match action {
        ApprovalAction::ToggleApplyAll => {
            state.approval_apply_all = !state.approval_apply_all;
            let _ = terminal.draw(state);
            None
        }
        ApprovalAction::Deny => Some(resolve_approval(false, prompt, state, terminal)),
        ApprovalAction::Approve => Some(resolve_approval(true, prompt, state, terminal)),
        ApprovalAction::Diff => {
            let _ = terminal.draw(state);
            None
        }
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
            provider_status: ProviderStatus::configured(),
            theme_name: "aurora-cyan".to_string(),
            input: String::new(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            entries: Vec::new(),
            workspace: WorkspaceSnapshot::fixture(),
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
}
