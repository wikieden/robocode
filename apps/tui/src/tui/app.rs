use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use viden_core::{
    ApprovalResponse, CoreTransport, EventCursor, RuntimeCommand, RuntimeViewState,
    StatefulCoreClient, TuiColorDepth,
};

use super::client::{PumpOutcome, TuiClientDriver, TuiClientError};
use super::state::{AgentTask, PendingTurn, ProviderStatus, TuiEntry, TuiState, WorkspaceSnapshot};
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
        let Event::Key(key) = event else {
            continue;
        };
        if handle_key(&mut driver, &mut state, key)? {
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

fn handle_key<T: CoreTransport>(
    driver: &mut TuiClientDriver<T>,
    state: &mut TuiState,
    key: KeyEvent,
) -> Result<bool, TuiError> {
    if let KeyCode::Char(decision @ ('y' | 'n')) = key.code
        && let Some(command) = approval_command(driver.view(), decision == 'y')
    {
        dispatch_intent(driver, command)?;
        return Ok(false);
    }
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
    use std::{collections::VecDeque, path::PathBuf, time::Duration};
    use viden_core::{
        CoreClientError, CoreHandshake, CoreTransport, EventCursor, RuntimeCommandEnvelope,
        RuntimeEventEnvelope, RuntimeSnapshotEnvelope, RuntimeViewState, StatefulCoreClient,
        frontend_capabilities, local_core_handshake,
    };
    use viden_types::{
        FRONTEND_SCHEMA_V1, PermissionLevel, PermissionMode, ReplayBatch, ReplayRequest,
        RuntimeSnapshot, TranscriptPage, TranscriptPageRequest, WorkMode,
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

        handle_key(
            &mut driver,
            &mut state,
            KeyEvent::new(KeyCode::Char('你'), KeyModifiers::NONE),
        )
        .expect("key");

        assert_eq!(state.input, "你");
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
