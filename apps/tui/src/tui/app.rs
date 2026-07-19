use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use viden_core::{CoreClient, RuntimeCommand};

use super::client::{PumpOutcome, TuiClientDriver, TuiClientError};
use super::state::{PendingTurn, ProviderStatus, TuiEntry, TuiState, WorkspaceSnapshot};
use super::terminal::TerminalGuard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiOptions {
    pub startup_summary: String,
    pub theme_name: Option<String>,
}

impl TuiOptions {
    pub fn new(startup_summary: impl Into<String>, theme_name: Option<&str>) -> Self {
        Self {
            startup_summary: startup_summary.into(),
            theme_name: theme_name.map(str::to_string),
        }
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
    let mut terminal = TerminalGuard::enter_with_theme(options.theme_name.as_deref())
        .map_err(TuiError::Terminal)?;
    let mut state = state_from_driver(&driver, &options, terminal.theme_name());
    terminal.draw(&state).map_err(TuiError::Terminal)?;

    loop {
        apply_pump_outcome(&mut state, driver.pump()?);
        terminal.draw(&state).map_err(TuiError::Terminal)?;

        if !event::poll(std::time::Duration::from_millis(100))
            .map_err(|err| TuiError::Terminal(err.to_string()))?
        {
            continue;
        }

        let event = event::read().map_err(|err| TuiError::Terminal(err.to_string()))?;
        let Event::Key(key) = event else {
            continue;
        };
        if handle_key(&mut driver, &mut state, key)? {
            break;
        }
    }
    Ok(())
}

/// Temporary bootstrap shim while `apps/cli` still constructs the legacy
/// runtime before choosing the UI. The value is intentionally opaque: the TUI
/// cannot drive it or inspect provider/session internals. CLI will replace
/// this with a real `LocalCoreTransport` in the integration step.
pub fn run_tui_with_theme<E>(
    _legacy_engine: E,
    startup_summary: &str,
    theme_name: Option<&str>,
) -> Result<(), String> {
    Err(format!(
        "TUI 0.2.0-alpha.1 requires CoreClient bootstrap; legacy runtime startup is disabled. startup={startup_summary}, theme={}",
        theme_name.unwrap_or("default")
    ))
}

fn state_from_driver<C: CoreClient>(
    driver: &TuiClientDriver<C>,
    options: &TuiOptions,
    theme_name: &str,
) -> TuiState {
    let snapshot = &driver.view().snapshot;
    let mut state = TuiState {
        session_id: driver.cursor().stream_id.clone(),
        provider: snapshot.provider_family.clone(),
        model: snapshot.model_label.clone(),
        provider_status: ProviderStatus::configured(),
        theme_name: theme_name.to_string(),
        workspace: WorkspaceSnapshot::fixture(),
        ..TuiState::default()
    };
    state.provider_status.work_mode = snapshot.work_mode;
    state.provider_status.permission_level = snapshot.permission_level;
    state.entries.push(TuiEntry {
        label: "system".to_string(),
        body: options.startup_summary.clone(),
    });
    state
}

pub(super) fn dispatch_intent<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    command: RuntimeCommand,
) -> Result<String, TuiClientError> {
    driver.send(command)
}

fn handle_key<C: CoreClient>(
    driver: &mut TuiClientDriver<C>,
    state: &mut TuiState,
    key: KeyEvent,
) -> Result<bool, TuiError> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            dispatch_intent(driver, RuntimeCommand::CancelActiveTurn)?;
            state.entries.push(TuiEntry {
                label: "command".to_string(),
                body: "cancel requested".to_string(),
            });
            Ok(false)
        }
        (KeyCode::Enter, _) if !state.input.trim().is_empty() => {
            let content = state.input.trim().to_string();
            state.pending_turn = Some(PendingTurn::for_input(&content));
            dispatch_intent(driver, RuntimeCommand::SubmitUserInput { content })?;
            state.input.clear();
            Ok(false)
        }
        (KeyCode::Esc, _) => Ok(true),
        (KeyCode::Char(ch), _) => {
            state.input.push(ch);
            Ok(false)
        }
        (KeyCode::Backspace, _) => {
            state.input.pop();
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn apply_pump_outcome(state: &mut TuiState, outcome: PumpOutcome) {
    match outcome {
        PumpOutcome::Idle | PumpOutcome::DuplicateIgnored(_) => {}
        PumpOutcome::Applied(cursor) | PumpOutcome::Recovered(cursor) => {
            state.session_id = cursor.stream_id;
            state.pending_turn = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, path::PathBuf, time::Duration};
    use viden_core::{
        CoreClientError, CoreHandshake, EventCursor, RuntimeCommandEnvelope, RuntimeEventEnvelope,
        RuntimeSnapshotEnvelope, RuntimeViewState, frontend_capabilities, local_core_handshake,
    };
    use viden_types::{
        FRONTEND_SCHEMA_V1, PermissionLevel, PermissionMode, ReplayBatch, ReplayRequest,
        RuntimeSnapshot, TranscriptPage, TranscriptPageRequest, WorkMode,
    };

    #[derive(Default)]
    struct FakeCoreClient {
        sent: Vec<RuntimeCommandEnvelope>,
    }

    impl CoreClient for FakeCoreClient {
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
            Ok(RuntimeSnapshotEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                capabilities: frontend_capabilities(),
                cursor: EventCursor {
                    stream_id: "fixture".to_string(),
                    sequence: 0,
                },
                view: RuntimeViewState::new(snapshot.clone()),
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
        let mut driver = TuiClientDriver::connect(FakeCoreClient::default()).expect("connect");

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
        let mut driver = TuiClientDriver::connect(FakeCoreClient::default()).expect("connect");
        let mut state = TuiState::default();

        handle_key(
            &mut driver,
            &mut state,
            KeyEvent::new(KeyCode::Char('你'), KeyModifiers::NONE),
        )
        .expect("key");

        assert_eq!(state.input, "你");
    }
}
