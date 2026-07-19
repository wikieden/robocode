use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputMode {
    Normal,
    Insert,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputIntent {
    None,
    EnterInsert,
    LeaveInsert,
    CloseOverlay,
    CancelCurrentWork,
    OpenCommandPalette,
    ContextHelp,
    Exit,
    InsertChar(char),
    Backspace,
    Submit,
    MoveSelection(i8),
    CompleteSelection,
    CompleteOrSubmit,
    Scroll(isize),
    ScrollToStart,
    ScrollToEnd,
}

pub(super) fn reduce_input(mode: InputMode, key: KeyEvent, has_active_work: bool) -> InputIntent {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return InputIntent::CancelCurrentWork;
    }
    if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return InputIntent::OpenCommandPalette;
    }
    match key.code {
        KeyCode::PageUp => return InputIntent::Scroll(12),
        KeyCode::PageDown => return InputIntent::Scroll(-12),
        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return InputIntent::ScrollToStart;
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return InputIntent::ScrollToEnd;
        }
        _ => {}
    }

    match (mode, key.code) {
        (InputMode::Normal, KeyCode::Char('i')) => InputIntent::EnterInsert,
        (InputMode::Normal, KeyCode::Char('?')) => InputIntent::ContextHelp,
        (InputMode::Normal, KeyCode::Esc) if has_active_work => InputIntent::None,
        (InputMode::Normal, KeyCode::Esc) => InputIntent::Exit,
        (InputMode::Insert, KeyCode::Esc) => InputIntent::LeaveInsert,
        (InputMode::Insert, KeyCode::Enter) | (InputMode::Insert, KeyCode::Char('j'))
            if key.modifiers.contains(KeyModifiers::CONTROL) || key.code == KeyCode::Enter =>
        {
            InputIntent::Submit
        }
        (InputMode::Insert, KeyCode::Backspace) => InputIntent::Backspace,
        (InputMode::Insert, KeyCode::Char(value))
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            InputIntent::InsertChar(value)
        }
        (InputMode::Overlay, KeyCode::Esc) => InputIntent::CloseOverlay,
        (InputMode::Overlay, KeyCode::Up | KeyCode::BackTab | KeyCode::Left) => {
            InputIntent::MoveSelection(-1)
        }
        (InputMode::Overlay, KeyCode::Down | KeyCode::Right) => InputIntent::MoveSelection(1),
        (InputMode::Overlay, KeyCode::Tab) => InputIntent::CompleteSelection,
        (InputMode::Overlay, KeyCode::Enter) => InputIntent::CompleteOrSubmit,
        (InputMode::Overlay, KeyCode::Backspace) => InputIntent::Backspace,
        (InputMode::Overlay, KeyCode::Char(value))
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            InputIntent::InsertChar(value)
        }
        _ => InputIntent::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn normal_insert_overlay_mode_matrix_is_reversible() {
        assert_eq!(
            reduce_input(InputMode::Normal, key(KeyCode::Char('i')), false),
            InputIntent::EnterInsert
        );
        assert_eq!(
            reduce_input(InputMode::Normal, key(KeyCode::Char('x')), false),
            InputIntent::None,
            "printable characters are not composer text in Normal mode"
        );
        assert_eq!(
            reduce_input(InputMode::Insert, key(KeyCode::Char('x')), false),
            InputIntent::InsertChar('x')
        );
        assert_eq!(
            reduce_input(InputMode::Insert, key(KeyCode::Esc), false),
            InputIntent::LeaveInsert
        );
        assert_eq!(
            reduce_input(InputMode::Overlay, key(KeyCode::Esc), false),
            InputIntent::CloseOverlay
        );
    }

    #[test]
    fn global_cancel_and_navigation_pierce_all_modes() {
        let cancel = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let palette = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        for mode in [InputMode::Normal, InputMode::Insert, InputMode::Overlay] {
            assert_eq!(
                reduce_input(mode, cancel, true),
                InputIntent::CancelCurrentWork
            );
            assert_eq!(
                reduce_input(mode, palette, false),
                InputIntent::OpenCommandPalette
            );
        }
    }

    #[test]
    fn overlay_owns_selector_navigation_and_submit() {
        assert_eq!(
            reduce_input(InputMode::Overlay, key(KeyCode::Down), false),
            InputIntent::MoveSelection(1)
        );
        assert_eq!(
            reduce_input(InputMode::Overlay, key(KeyCode::Tab), false),
            InputIntent::CompleteSelection
        );
        assert_eq!(
            reduce_input(InputMode::Overlay, key(KeyCode::Enter), false),
            InputIntent::CompleteOrSubmit
        );
    }

    #[test]
    fn active_work_keeps_normal_mode_escape_inside_the_cockpit() {
        assert_eq!(
            reduce_input(InputMode::Normal, key(KeyCode::Esc), true),
            InputIntent::None
        );
        assert_eq!(
            reduce_input(InputMode::Normal, key(KeyCode::Esc), false),
            InputIntent::Exit
        );
    }
}
