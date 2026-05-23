use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use robocode_core::{EngineEvent, SessionEngine};
use robocode_types::{ApprovalResponse, PermissionPrompt};

#[derive(Debug, Clone, PartialEq, Eq)]
struct TuiEntry {
    label: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TuiState {
    session_id: String,
    provider: String,
    model: String,
    input: String,
    entries: Vec<TuiEntry>,
}

pub(crate) fn run_tui(engine: &mut SessionEngine, startup_summary: &str) -> Result<(), String> {
    let mut terminal = TerminalGuard::enter()?;
    let mut state = TuiState {
        session_id: engine.session_id().to_string(),
        provider: engine.provider_name().to_string(),
        model: engine.model_name().to_string(),
        input: String::new(),
        entries: vec![TuiEntry {
            label: "system".to_string(),
            body: format!(
                "RoboCode TUI ready. Enter submits. Esc or Ctrl-C exits.\n{startup_summary}"
            ),
        }],
    };
    terminal.draw(&state)?;

    loop {
        let Event::Key(key) = event::read().map_err(|err| err.to_string())? else {
            continue;
        };
        if should_exit(key) {
            break;
        }
        match key.code {
            KeyCode::Enter => {
                let input = state.input.trim().to_string();
                state.input.clear();
                if input.is_empty() {
                    terminal.draw(&state)?;
                    continue;
                }
                if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                    break;
                }
                state.entries.push(TuiEntry {
                    label: "user".to_string(),
                    body: input.clone(),
                });
                terminal.draw(&state)?;
                let mut approver = |prompt: PermissionPrompt| {
                    prompt_for_tui_approval(prompt, &mut state, &mut terminal)
                };
                let events = engine.process_input_with_approval(&input, &mut approver)?;
                state
                    .entries
                    .extend(events.into_iter().map(entry_from_event));
                state.provider = engine.provider_name().to_string();
                state.model = engine.model_name().to_string();
            }
            KeyCode::Backspace => {
                state.input.pop();
            }
            KeyCode::Char(value) => {
                state.input.push(value);
            }
            _ => {}
        }
        terminal.draw(&state)?;
    }

    terminal.leave()
}

fn prompt_for_tui_approval(
    prompt: PermissionPrompt,
    state: &mut TuiState,
    terminal: &mut TerminalGuard,
) -> ApprovalResponse {
    state.entries.push(TuiEntry {
        label: "approval".to_string(),
        body: format!(
            "Permission request for `{}`\n{}\n{}\nPress y to allow, n/Esc to deny.",
            prompt.tool_name, prompt.message, prompt.input_preview
        ),
    });
    let _ = terminal.draw(state);
    loop {
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                state.entries.push(TuiEntry {
                    label: "approval".to_string(),
                    body: format!("Approved `{}`.", prompt.tool_name),
                });
                return ApprovalResponse {
                    approved: true,
                    feedback: None,
                };
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                state.entries.push(TuiEntry {
                    label: "approval".to_string(),
                    body: format!("Denied `{}`.", prompt.tool_name),
                });
                return ApprovalResponse {
                    approved: false,
                    feedback: None,
                };
            }
            _ => {}
        }
    }
}

fn should_exit(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn enter() -> Result<Self, String> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode().map_err(|err| err.to_string())?;
        if let Err(err) = execute!(stdout, EnterAlternateScreen, cursor::Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(err.to_string());
        }
        Ok(Self { active: true })
    }

    fn draw(&mut self, state: &TuiState) -> Result<(), String> {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let frame = render_frame(state, width, height);
        let mut stdout = io::stdout();
        queue!(
            stdout,
            cursor::MoveTo(0, 0),
            Clear(ClearType::All),
            SetForegroundColor(Color::Cyan),
            Print(frame),
            ResetColor
        )
        .map_err(|err| err.to_string())?;
        stdout.flush().map_err(|err| err.to_string())
    }

    fn leave(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut stdout = io::stdout();
        execute!(stdout, cursor::Show, LeaveAlternateScreen).map_err(|err| err.to_string())?;
        terminal::disable_raw_mode().map_err(|err| err.to_string())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

fn render_frame(state: &TuiState, width: u16, height: u16) -> String {
    let width = width.max(24) as usize;
    let height = height.max(8) as usize;
    let inner_width = width.saturating_sub(4).max(1);
    let transcript_height = height.saturating_sub(6);
    let mut lines = Vec::new();
    lines.push(format!("RoboCode TUI | session {}", state.session_id));
    lines.push(format!("provider={} model={}", state.provider, state.model));
    lines.push("-".repeat(width));
    for line in visible_transcript_lines(state, inner_width, transcript_height) {
        lines.push(line);
    }
    while lines.len() < height.saturating_sub(2) {
        lines.push(String::new());
    }
    lines.push("-".repeat(width));
    lines.push(format!("> {}", truncate_line(&state.input, inner_width)));
    lines.join("\n")
}

fn visible_transcript_lines(state: &TuiState, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for entry in &state.entries {
        lines.push(format!("[{}]", entry.label));
        lines.extend(entry.body.lines().map(|line| truncate_line(line, width)));
    }
    if lines.len() > max_lines {
        lines.split_off(lines.len() - max_lines)
    } else {
        lines
    }
}

fn truncate_line(value: &str, width: usize) -> String {
    value.chars().take(width).collect()
}

fn entry_from_event(event: EngineEvent) -> TuiEntry {
    match event {
        EngineEvent::System(text) => TuiEntry {
            label: "system".to_string(),
            body: text,
        },
        EngineEvent::Assistant(text) => TuiEntry {
            label: "assistant".to_string(),
            body: text,
        },
        EngineEvent::ToolCall(text) => TuiEntry {
            label: "tool-call".to_string(),
            body: text,
        },
        EngineEvent::ToolResult(text) => TuiEntry {
            label: "tool-result".to_string(),
            body: text,
        },
        EngineEvent::Command(text) => TuiEntry {
            label: "command".to_string(),
            body: text,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_frame_includes_status_transcript_and_input() {
        let state = TuiState {
            session_id: "session_123".to_string(),
            provider: "fallback".to_string(),
            model: "test-local".to_string(),
            input: "/help".to_string(),
            entries: vec![TuiEntry {
                label: "assistant".to_string(),
                body: "hello".to_string(),
            }],
        };

        let rendered = render_frame(&state, 48, 10);

        assert!(rendered.contains("RoboCode TUI | session session_123"));
        assert!(rendered.contains("provider=fallback model=test-local"));
        assert!(rendered.contains("[assistant]"));
        assert!(rendered.contains("hello"));
        assert!(rendered.contains("> /help"));
    }

    #[test]
    fn entry_from_event_preserves_command_output() {
        let entry = entry_from_event(EngineEvent::Command("Provider registry:".to_string()));

        assert_eq!(entry.label, "command");
        assert_eq!(entry.body, "Provider registry:");
    }
}
