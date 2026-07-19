use std::time::Duration;

use viden_core::{
    CoreClient, CoreClientError, CoreHandshake, EventCursor, RuntimeCommand,
    RuntimeCommandEnvelope, RuntimeEventEnvelope, RuntimeViewState, RuntimeWireEvent,
    validate_handshake,
};
use viden_types::{
    EventCursorOrder, FRONTEND_SCHEMA_V1, ReplayRequest, RuntimeOwner, RuntimeSnapshotEnvelope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PumpOutcome {
    Idle,
    Applied(EventCursor),
    Recovered(EventCursor),
    DuplicateIgnored(EventCursor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiClientError {
    Core(CoreClientError),
    Compatibility(String),
    MissingSnapshot,
    RecoveryBlocked(String),
}

impl std::fmt::Display for TuiClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => write!(formatter, "{error}"),
            Self::Compatibility(message) => write!(formatter, "TUI compatibility error: {message}"),
            Self::MissingSnapshot => write!(formatter, "Core did not provide a runtime snapshot"),
            Self::RecoveryBlocked(message) => {
                write!(formatter, "runtime recovery blocked: {message}")
            }
        }
    }
}

impl std::error::Error for TuiClientError {}

impl From<CoreClientError> for TuiClientError {
    fn from(value: CoreClientError) -> Self {
        Self::Core(value)
    }
}

pub struct TuiClientDriver<C: CoreClient> {
    client: C,
    handshake: CoreHandshake,
    view: RuntimeViewState,
    cursor: EventCursor,
    blocked_for_recovery: bool,
    next_command: u64,
    owner: RuntimeOwner,
}

impl<C: CoreClient> std::fmt::Debug for TuiClientDriver<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TuiClientDriver")
            .field("handshake", &self.handshake)
            .field("cursor", &self.cursor)
            .field("blocked_for_recovery", &self.blocked_for_recovery)
            .field("next_command", &self.next_command)
            .finish_non_exhaustive()
    }
}

impl<C: CoreClient> TuiClientDriver<C> {
    pub fn connect(mut client: C) -> Result<Self, TuiClientError> {
        let handshake = client.discover()?;
        validate_handshake(&handshake).map_err(TuiClientError::Compatibility)?;
        let snapshot = client.snapshot()?;
        validate_snapshot_envelope(&snapshot)?;
        Ok(Self {
            client,
            handshake,
            view: snapshot.view,
            cursor: snapshot.cursor,
            blocked_for_recovery: false,
            next_command: 1,
            owner: RuntimeOwner::default(),
        })
    }

    pub fn handshake(&self) -> &CoreHandshake {
        &self.handshake
    }

    pub fn view(&self) -> &RuntimeViewState {
        &self.view
    }

    pub fn cursor(&self) -> &EventCursor {
        &self.cursor
    }

    pub fn is_recovery_blocked(&self) -> bool {
        self.blocked_for_recovery
    }

    pub fn send(&mut self, command: RuntimeCommand) -> Result<String, TuiClientError> {
        let command_id = format!("tui-{}", self.next_command);
        self.next_command = self.next_command.saturating_add(1);
        self.client.send(RuntimeCommandEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            client_id: "viden-tui".to_string(),
            command_id: command_id.clone(),
            owner: self.owner.clone(),
            command,
        })?;
        Ok(command_id)
    }

    pub fn pump(&mut self) -> Result<PumpOutcome, TuiClientError> {
        self.pump_with_timeout(Duration::from_millis(0))
    }

    pub fn pump_with_timeout(&mut self, timeout: Duration) -> Result<PumpOutcome, TuiClientError> {
        let Some(envelope) = self.client.recv(timeout)? else {
            return Ok(PumpOutcome::Idle);
        };
        self.apply_or_recover(envelope)
    }

    fn apply_or_recover(
        &mut self,
        envelope: RuntimeEventEnvelope,
    ) -> Result<PumpOutcome, TuiClientError> {
        validate_event_envelope(&envelope)?;
        match self.cursor.classify_incoming(&envelope.cursor) {
            EventCursorOrder::DuplicateOrOld => {
                Ok(PumpOutcome::DuplicateIgnored(self.cursor.clone()))
            }
            EventCursorOrder::Next => {
                self.apply_envelope(&envelope)?;
                self.blocked_for_recovery = false;
                Ok(PumpOutcome::Applied(self.cursor.clone()))
            }
            EventCursorOrder::Gap => {
                self.blocked_for_recovery = true;
                self.recover_gap(envelope)?;
                self.blocked_for_recovery = false;
                Ok(PumpOutcome::Recovered(self.cursor.clone()))
            }
            EventCursorOrder::StreamMismatch => {
                self.blocked_for_recovery = true;
                Err(TuiClientError::RecoveryBlocked(format!(
                    "stream changed from {} to {}",
                    self.cursor.stream_id, envelope.cursor.stream_id
                )))
            }
        }
    }

    fn recover_gap(&mut self, incoming: RuntimeEventEnvelope) -> Result<(), TuiClientError> {
        let mut request = ReplayRequest {
            after: self.cursor.clone(),
            limit: 500,
        };
        let mut saw_incoming = false;
        loop {
            let batch = self.client.replay(request.clone())?;
            if batch.events.is_empty() && !batch.complete {
                return Err(TuiClientError::RecoveryBlocked(
                    "Core replay made no progress".to_string(),
                ));
            }
            for replayed in &batch.events {
                validate_event_envelope(replayed)?;
                match self.cursor.classify_incoming(&replayed.cursor) {
                    EventCursorOrder::DuplicateOrOld => {}
                    EventCursorOrder::Next => self.apply_envelope(replayed)?,
                    order => {
                        return Err(TuiClientError::RecoveryBlocked(format!(
                            "non-contiguous replay event {:?} at {}:{}",
                            order, replayed.cursor.stream_id, replayed.cursor.sequence
                        )));
                    }
                }
                saw_incoming |= replayed.cursor == incoming.cursor;
            }
            if batch.complete {
                break;
            }
            request = ReplayRequest {
                after: batch.next,
                limit: 500,
            };
        }
        if saw_incoming {
            Ok(())
        } else {
            Err(TuiClientError::RecoveryBlocked(format!(
                "complete replay did not include incoming cursor {}:{}",
                incoming.cursor.stream_id, incoming.cursor.sequence
            )))
        }
    }

    fn apply_envelope(&mut self, envelope: &RuntimeEventEnvelope) -> Result<(), TuiClientError> {
        if let RuntimeWireEvent::Known(event) = &envelope.event {
            self.view.apply_event(event);
        }
        self.cursor = envelope.cursor.clone();
        Ok(())
    }
}

fn validate_snapshot_envelope(snapshot: &RuntimeSnapshotEnvelope) -> Result<(), TuiClientError> {
    if snapshot.schema_version != FRONTEND_SCHEMA_V1 {
        return Err(TuiClientError::Compatibility(format!(
            "unsupported frontend schema {}",
            snapshot.schema_version.0
        )));
    }
    if snapshot.snapshot != snapshot.view.snapshot {
        return Err(TuiClientError::Compatibility(
            "snapshot and projected view disagree".to_string(),
        ));
    }
    if snapshot.cursor.stream_id.is_empty() {
        return Err(TuiClientError::Compatibility(
            "snapshot cursor stream id must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_event_envelope(envelope: &RuntimeEventEnvelope) -> Result<(), TuiClientError> {
    if envelope.schema_version != FRONTEND_SCHEMA_V1 {
        return Err(TuiClientError::Compatibility(format!(
            "unsupported frontend schema {}",
            envelope.schema_version.0
        )));
    }
    if let RuntimeWireEvent::Known(event) = &envelope.event
        && event.sequence != envelope.cursor.sequence
    {
        return Err(TuiClientError::Compatibility(format!(
            "event sequence {} does not match cursor sequence {}",
            event.sequence, envelope.cursor.sequence
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, path::PathBuf, time::Duration};
    use viden_core::{CoreClientError, frontend_capabilities, local_core_handshake};
    use viden_types::{
        PermissionLevel, PermissionMode, QueuedInputView, ReplayBatch, RuntimeEvent,
        RuntimeEventKind, RuntimeSnapshot, RuntimeWireEvent, TranscriptPage, TranscriptPageRequest,
        WorkMode,
    };

    #[derive(Default)]
    struct FakeCoreClient {
        handshake: Option<CoreHandshake>,
        snapshot: Option<RuntimeSnapshotEnvelope>,
        events: VecDeque<RuntimeEventEnvelope>,
        replay: VecDeque<ReplayBatch>,
        sent: Vec<RuntimeCommandEnvelope>,
    }

    impl FakeCoreClient {
        fn compatible() -> Self {
            let snapshot = runtime_snapshot();
            let cursor = EventCursor {
                stream_id: "fixture".to_string(),
                sequence: 0,
            };
            Self {
                handshake: Some(local_core_handshake()),
                snapshot: Some(RuntimeSnapshotEnvelope {
                    schema_version: FRONTEND_SCHEMA_V1,
                    capabilities: frontend_capabilities(),
                    cursor: cursor.clone(),
                    view: RuntimeViewState::new(snapshot.clone()),
                    snapshot,
                }),
                ..Self::default()
            }
        }
    }

    impl CoreClient for FakeCoreClient {
        fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
            self.handshake
                .clone()
                .ok_or(CoreClientError::HandshakeRequired)
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
            self.snapshot
                .clone()
                .ok_or(CoreClientError::MissingSnapshot)
        }

        fn replay(&mut self, _request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
            self.replay
                .pop_front()
                .ok_or_else(|| CoreClientError::Transport("missing replay fixture".to_string()))
        }

        fn transcript_page(
            &mut self,
            _request: TranscriptPageRequest,
        ) -> Result<TranscriptPage, CoreClientError> {
            Err(CoreClientError::Transport(
                "transcript page is not used by TUI client tests".to_string(),
            ))
        }
    }

    #[test]
    fn shared_frontend_fixtures_reduce_to_core_expected_facts() {
        let mut fake = FakeCoreClient::compatible();
        fake.events.push_back(event(
            1,
            RuntimeEventKind::AssistantDelta {
                message_id: "m1".to_string(),
                task_id: None,
                content: "hello".to_string(),
            },
        ));
        fake.events.push_back(event(
            2,
            RuntimeEventKind::InputQueued {
                input: QueuedInputView {
                    id: "q1".to_string(),
                    content_preview: "follow-up".to_string(),
                    created_at: Some(2),
                },
            },
        ));

        let mut driver = TuiClientDriver::connect(fake).expect("connect");

        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Applied(_)));
        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Applied(_)));
        assert_eq!(driver.view().assistant_stream, "hello");
        assert_eq!(driver.view().queued_inputs.len(), 1);
        assert_eq!(driver.cursor().sequence, 2);
    }

    #[test]
    fn incompatible_schema_fails_before_any_command_is_sent() {
        let mut fake = FakeCoreClient::compatible();
        let mut handshake = local_core_handshake();
        handshake.active_schema_version = viden_types::SchemaVersion(2);
        fake.handshake = Some(handshake);

        let error = TuiClientDriver::connect(fake).unwrap_err();

        assert!(matches!(error, TuiClientError::Compatibility(_)));
    }

    #[test]
    fn sequence_gap_requests_replay_before_success_is_visible() {
        let mut fake = FakeCoreClient::compatible();
        fake.events.push_back(event(
            3,
            RuntimeEventKind::AssistantDelta {
                message_id: "m3".to_string(),
                task_id: None,
                content: "late".to_string(),
            },
        ));
        fake.replay.push_back(ReplayBatch {
            events: vec![
                event(
                    1,
                    RuntimeEventKind::AssistantDelta {
                        message_id: "m1".to_string(),
                        task_id: None,
                        content: "a".to_string(),
                    },
                ),
                event(
                    2,
                    RuntimeEventKind::AssistantDelta {
                        message_id: "m2".to_string(),
                        task_id: None,
                        content: "b".to_string(),
                    },
                ),
                event(
                    3,
                    RuntimeEventKind::AssistantDelta {
                        message_id: "m3".to_string(),
                        task_id: None,
                        content: "c".to_string(),
                    },
                ),
            ],
            next: EventCursor {
                stream_id: "fixture".to_string(),
                sequence: 3,
            },
            complete: true,
        });

        let mut driver = TuiClientDriver::connect(fake).expect("connect");

        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Recovered(_)));
        assert_eq!(driver.view().assistant_stream, "abc");
        assert!(!driver.is_recovery_blocked());
    }

    #[test]
    fn duplicate_events_do_not_duplicate_view_facts() {
        let mut fake = FakeCoreClient::compatible();
        fake.events.push_back(event(
            1,
            RuntimeEventKind::AssistantDelta {
                message_id: "m1".to_string(),
                task_id: None,
                content: "once".to_string(),
            },
        ));
        fake.events.push_back(event(
            1,
            RuntimeEventKind::AssistantDelta {
                message_id: "m1".to_string(),
                task_id: None,
                content: "once".to_string(),
            },
        ));

        let mut driver = TuiClientDriver::connect(fake).expect("connect");

        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Applied(_)));
        assert!(matches!(
            driver.pump().unwrap(),
            PumpOutcome::DuplicateIgnored(_)
        ));
        assert_eq!(driver.view().assistant_stream, "once");
    }

    fn event(sequence: u64, kind: RuntimeEventKind) -> RuntimeEventEnvelope {
        RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: RuntimeOwner::default(),
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

    fn runtime_snapshot() -> RuntimeSnapshot {
        RuntimeSnapshot {
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
        }
    }
}
