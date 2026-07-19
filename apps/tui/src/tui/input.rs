use crossterm::event::{KeyCode, KeyEvent};

use super::command_palette::is_command_palette_visible;
use super::keymap::{InputFocus, InputMode, OverlayKind};
use super::modal::{
    ApprovalAction, focused_approval_action, move_approval_focus, set_approval_focus_for_action,
};
use super::state::{TuiEntry, TuiState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalKeyEffect {
    None,
    Redraw,
    Resolve(bool),
}

pub(super) fn effective_input_mode(state: &TuiState) -> InputMode {
    if active_overlay_kind(state).is_some() {
        InputMode::Overlay
    } else {
        state.ui.input_mode
    }
}

pub(super) fn input_focus(state: &TuiState) -> InputFocus {
    InputFocus {
        overlay: active_overlay_kind(state),
        selection_active: state.ui.focused_lane.is_some(),
        idle_ctrl_c_armed: state.ui.idle_ctrl_c_armed,
    }
}

fn active_overlay_kind(state: &TuiState) -> Option<OverlayKind> {
    if let Some(overlay) = state.ui.overlay.as_ref() {
        Some(overlay.kind)
    } else if state.ui.interaction_panel.is_some() {
        Some(OverlayKind::InteractionPanel)
    } else if is_command_palette_visible(state) {
        Some(OverlayKind::ComposerCommands)
    } else {
        None
    }
}

pub(super) fn close_focus_on_escape(key: KeyEvent, state: &mut TuiState) -> bool {
    if key.code != KeyCode::Esc || state.ui.focused_lane.is_none() {
        return false;
    }
    state.ui.focused_lane = None;
    state.ui.entries.push(TuiEntry {
        label: "system".to_string(),
        body: "Closed lane detail focus.".to_string(),
    });
    true
}

pub(super) fn apply_approval_key(key: KeyEvent, state: &mut TuiState) -> ApprovalKeyEffect {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => ApprovalKeyEffect::Resolve(true),
        KeyCode::Char('n') | KeyCode::Char('N') => ApprovalKeyEffect::Resolve(false),
        KeyCode::Char(' ') => {
            if focused_approval_action(state) != ApprovalAction::ToggleApplyAll {
                return ApprovalKeyEffect::None;
            }
            state.ui.approval_apply_all = !state.ui.approval_apply_all;
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

pub(super) fn apply_approval_action(
    action: ApprovalAction,
    state: &mut TuiState,
) -> ApprovalKeyEffect {
    match action {
        ApprovalAction::ToggleApplyAll => {
            state.ui.approval_apply_all = !state.ui.approval_apply_all;
            ApprovalKeyEffect::Redraw
        }
        ApprovalAction::Deny => ApprovalKeyEffect::Resolve(false),
        ApprovalAction::Approve => ApprovalKeyEffect::Resolve(true),
        ApprovalAction::Diff => ApprovalKeyEffect::Redraw,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::super::modal::DEFAULT_APPROVAL_FOCUS;
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn state_with_focus() -> TuiState {
        let mut state = TuiState::default();
        state.ui.session_id = "session_123".to_string();
        state.ui.provider_catalog = crate::tui::state::ProviderOption::fixture();
        state.ui.theme_name = "aurora-cyan".to_string();
        state.ui.focused_lane = Some("L1".to_string());
        state
    }

    #[test]
    fn escape_closes_focus_before_exit() {
        let mut state = state_with_focus();

        assert!(close_focus_on_escape(key(KeyCode::Esc), &mut state));

        assert_eq!(state.ui.focused_lane, None);
        assert!(
            state.ui.entries[0]
                .body
                .contains("Closed lane detail focus")
        );
    }

    #[test]
    fn escape_without_focus_leaves_selection_handler_idle() {
        let mut state = state_with_focus();
        state.ui.focused_lane = None;

        assert!(!close_focus_on_escape(key(KeyCode::Esc), &mut state));
    }

    #[test]
    fn approval_space_toggles_apply_all_only_when_checkbox_is_focused() {
        let mut state = state_with_focus();
        state.ui.approval_focus = DEFAULT_APPROVAL_FOCUS;

        assert_eq!(
            apply_approval_key(key(KeyCode::Char(' ')), &mut state),
            ApprovalKeyEffect::None
        );
        assert!(!state.ui.approval_apply_all);

        set_approval_focus_for_action(&mut state, ApprovalAction::ToggleApplyAll);

        assert_eq!(
            apply_approval_key(key(KeyCode::Char(' ')), &mut state),
            ApprovalKeyEffect::Redraw
        );
        assert!(state.ui.approval_apply_all);
    }

    #[test]
    fn approval_enter_activates_default_approve_focus() {
        let mut state = state_with_focus();
        state.ui.approval_focus = DEFAULT_APPROVAL_FOCUS;

        assert_eq!(
            apply_approval_key(key(KeyCode::Enter), &mut state),
            ApprovalKeyEffect::Resolve(true)
        );
    }

    #[test]
    fn approval_keyboard_focus_reaches_deny_diff_and_approve() {
        let mut state = state_with_focus();
        state.ui.approval_focus = DEFAULT_APPROVAL_FOCUS;

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
            ApprovalKeyEffect::None,
            "Ctrl-C belongs to current-work cancellation and must never deny approval"
        );
    }
}
