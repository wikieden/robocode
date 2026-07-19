use std::time::Duration;

use viden_core::{
    CoreClient, CoreClientError, CoreHandshake, CoreTransport, EventCursor, RuntimeCommand,
    RuntimeCommandEnvelope, RuntimeViewState, StatefulCoreClient,
};
use viden_types::{FRONTEND_SCHEMA_V1, RuntimeOwner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PumpOutcome {
    Idle,
    Applied(EventCursor),
    Recovered(EventCursor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiClientError {
    Core(CoreClientError),
}

impl std::fmt::Display for TuiClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TuiClientError {}

impl From<CoreClientError> for TuiClientError {
    fn from(value: CoreClientError) -> Self {
        Self::Core(value)
    }
}

pub(super) struct TuiClientDriver<T: CoreTransport> {
    client: StatefulCoreClient<T>,
    handshake: CoreHandshake,
    next_command: u64,
    owner: RuntimeOwner,
}

impl<T: CoreTransport> std::fmt::Debug for TuiClientDriver<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TuiClientDriver")
            .field("handshake", &self.handshake)
            .field("cursor", &self.client.confirmed_cursor())
            .field("next_command", &self.next_command)
            .finish_non_exhaustive()
    }
}

impl<T: CoreTransport> TuiClientDriver<T> {
    pub(super) fn connect(mut client: StatefulCoreClient<T>) -> Result<Self, TuiClientError> {
        let handshake = client.discover()?;
        client.snapshot()?;
        Ok(Self {
            client,
            handshake,
            next_command: 1,
            owner: RuntimeOwner::default(),
        })
    }

    pub(super) fn view(&self) -> &RuntimeViewState {
        self.client
            .confirmed_view()
            .expect("TuiClientDriver loads a snapshot before construction")
    }

    pub(super) fn cursor(&self) -> &EventCursor {
        self.client
            .confirmed_cursor()
            .expect("TuiClientDriver loads a snapshot before construction")
    }

    pub(super) fn send(&mut self, command: RuntimeCommand) -> Result<String, TuiClientError> {
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

    pub(super) fn pump(&mut self) -> Result<PumpOutcome, TuiClientError> {
        self.pump_with_timeout(Duration::from_millis(0))
    }

    fn pump_with_timeout(&mut self, timeout: Duration) -> Result<PumpOutcome, TuiClientError> {
        let before = self.cursor().clone();
        let delivered = self.client.recv(timeout)?;
        let after = self.cursor().clone();
        if delivered.is_none() && after == before {
            return Ok(PumpOutcome::Idle);
        }
        if before.stream_id == after.stream_id
            && after.sequence == before.sequence.saturating_add(1)
        {
            Ok(PumpOutcome::Applied(after))
        } else {
            Ok(PumpOutcome::Recovered(after))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, path::PathBuf, time::Duration};
    use viden_core::{CoreClientError, CoreTransport, frontend_capabilities, local_core_handshake};
    use viden_types::{
        PermissionLevel, PermissionMode, ReplayBatch, ReplayRequest, RuntimeEvent,
        RuntimeEventEnvelope, RuntimeEventKind, RuntimeSnapshot, RuntimeSnapshotEnvelope,
        RuntimeWireEvent, TranscriptPage, TranscriptPageRequest, WorkMode,
    };

    #[derive(Default)]
    struct FakeCoreTransport {
        handshake: Option<CoreHandshake>,
        snapshot: Option<RuntimeSnapshotEnvelope>,
        events: VecDeque<RuntimeEventEnvelope>,
        replay: VecDeque<ReplayBatch>,
        sent: Vec<RuntimeCommandEnvelope>,
    }

    impl FakeCoreTransport {
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

    impl CoreTransport for FakeCoreTransport {
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
        #[derive(serde::Deserialize)]
        struct Fixture {
            initial_snapshot: RuntimeSnapshot,
            events: Vec<RuntimeEventEnvelope>,
            expected_final_cursor: EventCursor,
        }
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
        ))
        .expect("shared frontend fixture");
        let initial_cursor = EventCursor {
            stream_id: fixture.expected_final_cursor.stream_id.clone(),
            sequence: 0,
        };
        let snapshot = fixture.initial_snapshot;
        let fake = FakeCoreTransport {
            handshake: Some(local_core_handshake()),
            snapshot: Some(RuntimeSnapshotEnvelope {
                schema_version: FRONTEND_SCHEMA_V1,
                capabilities: frontend_capabilities(),
                cursor: initial_cursor,
                view: RuntimeViewState::new(snapshot.clone()),
                snapshot,
            }),
            events: fixture.events.into(),
            ..FakeCoreTransport::default()
        };
        let event_count = fake.events.len();

        let mut driver = TuiClientDriver::connect(StatefulCoreClient::new(fake)).expect("connect");

        for _ in 0..event_count {
            assert!(matches!(driver.pump().unwrap(), PumpOutcome::Applied(_)));
        }
        assert_eq!(driver.view().assistant_stream, "D1 cockpit state");
        assert_eq!(driver.view().tasks.len(), 1);
        assert!(!driver.view().errors.is_empty());
        assert!(driver.view().cost_ledger.total_tokens > 0);
        assert_eq!(driver.cursor(), &fixture.expected_final_cursor);
    }

    #[test]
    fn incompatible_schema_fails_before_any_command_is_sent() {
        let mut fake = FakeCoreTransport::compatible();
        let mut handshake = local_core_handshake();
        handshake.active_schema_version = viden_types::SchemaVersion(2);
        fake.handshake = Some(handshake);

        let error = TuiClientDriver::connect(StatefulCoreClient::new(fake)).unwrap_err();

        assert!(matches!(
            error,
            TuiClientError::Core(CoreClientError::Compatibility(_))
        ));
    }

    #[test]
    fn sequence_gap_requests_replay_before_success_is_visible() {
        let mut fake = FakeCoreTransport::compatible();
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

        let mut driver = TuiClientDriver::connect(StatefulCoreClient::new(fake)).expect("connect");

        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Recovered(_)));
        assert_eq!(driver.view().assistant_stream, "abc");
    }

    #[test]
    fn duplicate_events_do_not_duplicate_view_facts() {
        let mut fake = FakeCoreTransport::compatible();
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

        let mut driver = TuiClientDriver::connect(StatefulCoreClient::new(fake)).expect("connect");

        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Applied(_)));
        assert!(matches!(driver.pump().unwrap(), PumpOutcome::Idle));
        assert_eq!(driver.view().assistant_stream, "once");
    }

    #[test]
    fn failed_replay_does_not_publish_partial_view_or_cursor() {
        let mut fake = FakeCoreTransport::compatible();
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
                        content: "partial".to_string(),
                    },
                ),
                event(
                    3,
                    RuntimeEventKind::AssistantDelta {
                        message_id: "m3".to_string(),
                        task_id: None,
                        content: "invalid-gap".to_string(),
                    },
                ),
            ],
            next: EventCursor {
                stream_id: "fixture".to_string(),
                sequence: 3,
            },
            complete: true,
        });

        let mut driver = TuiClientDriver::connect(StatefulCoreClient::new(fake)).expect("connect");
        let error = driver.pump().expect_err("replay must fail");

        assert!(matches!(
            error,
            TuiClientError::Core(CoreClientError::Protocol(_))
        ));
        assert_eq!(driver.cursor().sequence, 0);
        assert!(driver.view().assistant_stream.is_empty());
    }

    #[test]
    fn complete_replay_without_incoming_rolls_back_all_staged_events() {
        let mut fake = FakeCoreTransport::compatible();
        fake.events.push_back(event(
            3,
            RuntimeEventKind::AssistantDelta {
                message_id: "m3".to_string(),
                task_id: None,
                content: "incoming".to_string(),
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
            ],
            next: EventCursor {
                stream_id: "fixture".to_string(),
                sequence: 2,
            },
            complete: true,
        });

        let mut driver = TuiClientDriver::connect(StatefulCoreClient::new(fake)).expect("connect");
        let error = driver.pump().expect_err("incoming must be present");

        assert!(matches!(
            error,
            TuiClientError::Core(CoreClientError::Protocol(_))
        ));
        assert_eq!(driver.cursor().sequence, 0);
        assert!(driver.view().assistant_stream.is_empty());
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
