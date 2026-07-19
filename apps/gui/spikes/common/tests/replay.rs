use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use viden_core::{
    CoreClientError, CoreHandshake, CoreTransport, EventCursor, FRONTEND_SCHEMA_V1, ReplayBatch,
    ReplayRequest, RuntimeCommand, RuntimeCommandEnvelope, RuntimeEvent, RuntimeEventEnvelope,
    RuntimeEventKind, RuntimeSnapshot, RuntimeSnapshotEnvelope, RuntimeViewState, RuntimeWireEvent,
    SchemaVersion, TranscriptPage, TranscriptPageRequest, TranscriptRow, TranscriptRowId,
    frontend_capabilities,
};
use viden_gui_spike_common::{GuiConnectionState, GuiCoreAdapter, TranscriptViewport};

const D1_FIXTURE: &str = include_str!(
    "../../../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

#[derive(Clone, Deserialize)]
struct D1Fixture {
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
    expected_final_cursor: EventCursor,
}

#[derive(Clone)]
struct FixtureTransport {
    state: Arc<Mutex<FixtureTransportState>>,
}

struct FixtureTransportState {
    handshake: Result<CoreHandshake, CoreClientError>,
    sent: Vec<RuntimeCommandEnvelope>,
    receives: VecDeque<Result<Option<RuntimeEventEnvelope>, CoreClientError>>,
    snapshots: VecDeque<Result<RuntimeSnapshotEnvelope, CoreClientError>>,
    replays: VecDeque<Result<ReplayBatch, CoreClientError>>,
    replay_requests: Vec<ReplayRequest>,
}

impl FixtureTransport {
    fn new() -> (Self, Arc<Mutex<FixtureTransportState>>) {
        let state = Arc::new(Mutex::new(FixtureTransportState {
            handshake: Ok(CoreHandshake {
                core_version: "0.3.0-fixture".to_string(),
                supported_schema_versions: vec![FRONTEND_SCHEMA_V1],
                active_schema_version: FRONTEND_SCHEMA_V1,
                capabilities: frontend_capabilities(),
            }),
            sent: Vec::new(),
            receives: VecDeque::new(),
            snapshots: VecDeque::new(),
            replays: VecDeque::new(),
            replay_requests: Vec::new(),
        }));
        (
            Self {
                state: Arc::clone(&state),
            },
            state,
        )
    }
}

impl CoreTransport for FixtureTransport {
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
        self.state.lock().unwrap().handshake.clone()
    }

    fn send(&mut self, command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        self.state.lock().unwrap().sent.push(command);
        Ok(())
    }

    fn recv(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        self.state
            .lock()
            .unwrap()
            .receives
            .pop_front()
            .unwrap_or(Ok(None))
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        self.state
            .lock()
            .unwrap()
            .snapshots
            .pop_front()
            .unwrap_or_else(|| {
                Err(CoreClientError::Transport(
                    "missing fixture snapshot".into(),
                ))
            })
    }

    fn replay(&mut self, request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
        let mut state = self.state.lock().unwrap();
        state.replay_requests.push(request);
        state
            .replays
            .pop_front()
            .unwrap_or_else(|| Err(CoreClientError::Transport("missing fixture replay".into())))
    }

    fn transcript_page(
        &mut self,
        _request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError> {
        Err(CoreClientError::Transport(
            "fixture transport has no transcript page".into(),
        ))
    }
}

fn fixture() -> D1Fixture {
    serde_json::from_str(D1_FIXTURE).expect("committed D1 fixture should parse")
}

fn snapshot(
    initial: &RuntimeSnapshot,
    cursor: EventCursor,
    mutate: impl FnOnce(&mut RuntimeViewState),
) -> RuntimeSnapshotEnvelope {
    let mut view = RuntimeViewState::new(initial.clone());
    mutate(&mut view);
    RuntimeSnapshotEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        capabilities: frontend_capabilities(),
        cursor,
        snapshot: initial.clone(),
        view,
    }
}

fn initial_snapshot(fixture: &D1Fixture) -> RuntimeSnapshotEnvelope {
    snapshot(
        &fixture.initial_snapshot,
        EventCursor {
            stream_id: "fixture:d1-vertical-slice".to_string(),
            sequence: 0,
        },
        |_| {},
    )
}

fn connected_adapter(
    fixture: &D1Fixture,
    transport: FixtureTransport,
    state: &Arc<Mutex<FixtureTransportState>>,
) -> GuiCoreAdapter<FixtureTransport> {
    state
        .lock()
        .unwrap()
        .snapshots
        .push_back(Ok(initial_snapshot(fixture)));
    let mut adapter = GuiCoreAdapter::new(transport);
    adapter.connect().unwrap();
    adapter
}

#[test]
fn duplicate_event_does_not_advance_or_mutate_the_confirmed_projection_twice() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    let event = fixture.events[0].clone();
    {
        let mut state = state.lock().unwrap();
        state.receives.push_back(Ok(Some(event.clone())));
        state.receives.push_back(Ok(Some(event)));
    }
    let mut adapter = connected_adapter(&fixture, transport, &state);

    adapter.pump(Duration::ZERO).unwrap();
    let first = adapter.projection().clone();
    adapter.pump(Duration::ZERO).unwrap();

    assert_eq!(adapter.projection(), &first);
    assert_eq!(adapter.metrics().confirmed_event_count, 1);
}

#[test]
fn incompatible_handshake_enters_the_terminal_incompatible_state() {
    let (transport, state) = FixtureTransport::new();
    state.lock().unwrap().handshake = Ok(CoreHandshake {
        core_version: "future-core".to_string(),
        supported_schema_versions: vec![SchemaVersion(2)],
        active_schema_version: SchemaVersion(2),
        capabilities: frontend_capabilities(),
    });
    let mut adapter = GuiCoreAdapter::new(transport);

    let error = adapter.connect().unwrap_err();

    assert!(matches!(error, CoreClientError::Compatibility(_)));
    assert!(matches!(
        adapter.connection_state(),
        GuiConnectionState::Incompatible { .. }
    ));
}

#[test]
fn out_of_order_replay_is_rejected_without_publishing_staged_core_state() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    {
        let mut state = state.lock().unwrap();
        state
            .receives
            .push_back(Ok(Some(fixture.events[2].clone())));
        state.replays.push_back(Ok(ReplayBatch {
            events: vec![
                fixture.events[1].clone(),
                fixture.events[0].clone(),
                fixture.events[2].clone(),
            ],
            next: fixture.events[2].cursor.clone(),
            complete: true,
        }));
    }
    let mut adapter = connected_adapter(&fixture, transport, &state);
    let initial = adapter.projection().clone();

    let error = adapter.pump(Duration::ZERO).unwrap_err();

    assert!(matches!(error, CoreClientError::Protocol(_)));
    assert_eq!(adapter.projection(), &initial);
    assert_eq!(
        adapter.connection_state(),
        &GuiConnectionState::Recovering {
            expected: EventCursor {
                stream_id: "fixture:d1-vertical-slice".to_string(),
                sequence: 1,
            },
            received: fixture.events[2].cursor.clone(),
        }
    );
}

#[test]
fn failed_gap_replay_enters_recovering_and_snapshot_recovery_returns_live() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    {
        let mut state = state.lock().unwrap();
        state
            .receives
            .push_back(Ok(Some(fixture.events[2].clone())));
        state
            .replays
            .push_back(Err(CoreClientError::Transport("replay offline".into())));
    }
    let mut adapter = connected_adapter(&fixture, transport, &state);

    assert!(adapter.pump(Duration::ZERO).is_err());
    assert!(matches!(
        adapter.connection_state(),
        GuiConnectionState::Recovering { .. }
    ));

    let replacement = snapshot(
        &fixture.initial_snapshot,
        EventCursor {
            stream_id: "fixture:d1-recovered".to_string(),
            sequence: 9,
        },
        |view| view.assistant_stream = "snapshot recovered".to_string(),
    );
    state.lock().unwrap().snapshots.push_back(Ok(replacement));
    adapter.recover().unwrap();

    assert!(matches!(
        adapter.connection_state(),
        GuiConnectionState::Live { cursor } if cursor.stream_id == "fixture:d1-recovered" && cursor.sequence == 9
    ));
    assert_eq!(
        adapter.projection().view().unwrap().assistant_stream,
        "snapshot recovered"
    );
}

#[test]
fn stream_mismatch_replaces_the_projection_from_core_snapshot() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    let mut mismatched = fixture.events[0].clone();
    mismatched.cursor.stream_id = "fixture:new-stream".to_string();
    let replacement = snapshot(
        &fixture.initial_snapshot,
        EventCursor {
            stream_id: "fixture:new-stream".to_string(),
            sequence: 7,
        },
        |view| view.assistant_stream = "rebuilt from snapshot".to_string(),
    );
    {
        let mut state = state.lock().unwrap();
        state.receives.push_back(Ok(Some(mismatched)));
    }
    let mut adapter = connected_adapter(&fixture, transport, &state);
    state.lock().unwrap().snapshots.push_back(Ok(replacement));

    adapter.pump(Duration::ZERO).unwrap();

    assert_eq!(adapter.projection().cursor().unwrap().sequence, 7);
    assert_eq!(
        adapter.projection().view().unwrap().assistant_stream,
        "rebuilt from snapshot"
    );
    assert_eq!(adapter.metrics().snapshot_replacements, 1);
}

#[test]
fn multi_batch_gap_replay_syncs_every_intermediate_fact_from_confirmed_core_view() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    {
        let mut state = state.lock().unwrap();
        state
            .receives
            .push_back(Ok(Some(fixture.events[3].clone())));
        state.replays.push_back(Ok(ReplayBatch {
            events: fixture.events[0..2].to_vec(),
            next: fixture.events[1].cursor.clone(),
            complete: false,
        }));
        state.replays.push_back(Ok(ReplayBatch {
            events: fixture.events[2..4].to_vec(),
            next: fixture.events[3].cursor.clone(),
            complete: true,
        }));
    }
    let mut adapter = connected_adapter(&fixture, transport, &state);

    adapter.pump(Duration::ZERO).unwrap();

    let view = adapter.projection().view().unwrap();
    assert_eq!(view.assistant_stream, "D1 cockpit state");
    assert!(view.active_tool_calls.is_empty());
    assert_eq!(view.latest_evidence[0].id, "ev_d1_test");
    assert_eq!(view.lanes[0].id, "lane_d1_core");
    assert_eq!(
        adapter.projection().cursor(),
        Some(&fixture.events[3].cursor)
    );
    assert_eq!(adapter.metrics().gap_recoveries, 1);
    assert_eq!(adapter.metrics().replay_batches_observed, 2);
}

#[test]
fn command_rejection_is_rendered_only_from_the_core_confirmed_view() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    let mut rejected = fixture.events[0].clone();
    rejected.event = RuntimeWireEvent::Known(RuntimeEvent::new(
        1,
        RuntimeEventKind::CommandRejected {
            command_id: "command-rejected".to_string(),
            reason: "policy denied".to_string(),
        },
    ));
    state.lock().unwrap().receives.push_back(Ok(Some(rejected)));
    let mut adapter = connected_adapter(&fixture, transport, &state);

    adapter.pump(Duration::ZERO).unwrap();

    let view = adapter.projection().view().unwrap();
    assert!(view.last_command.is_none());
    assert!(view.errors.iter().any(|error| {
        error.message.contains("command-rejected") && error.message.contains("policy denied")
    }));
}

#[test]
fn send_intent_delegates_the_unchanged_envelope_to_core_transport() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    let mut adapter = connected_adapter(&fixture, transport, &state);
    let command = RuntimeCommandEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        client_id: "gui-spike".to_string(),
        command_id: "queue-1".to_string(),
        owner: fixture.events[0].owner.clone(),
        command: RuntimeCommand::QueueFollowUp {
            content: "continue".to_string(),
        },
    };

    adapter.send_intent(command.clone()).unwrap();

    assert_eq!(state.lock().unwrap().sent, vec![command]);
}

#[test]
fn ten_thousand_fixture_events_preserve_cursor_and_content_identity() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    {
        let mut state = state.lock().unwrap();
        for sequence in 1..=10_000 {
            let mut event = fixture.events[0].clone();
            event.cursor.sequence = sequence;
            let RuntimeWireEvent::Known(runtime_event) = &mut event.event else {
                panic!("D1 event 1 should be known");
            };
            runtime_event.sequence = sequence;
            runtime_event.kind = RuntimeEventKind::AssistantDelta {
                message_id: format!("d1-{sequence}"),
                task_id: None,
                content: "x".to_string(),
            };
            state.receives.push_back(Ok(Some(event)));
        }
    }
    let mut adapter = connected_adapter(&fixture, transport, &state);

    for _ in 0..10_000 {
        adapter.pump(Duration::ZERO).unwrap();
    }

    assert_eq!(adapter.projection().cursor().unwrap().sequence, 10_000);
    assert_eq!(
        adapter.projection().view().unwrap().assistant_stream,
        "x".repeat(10_000)
    );
    assert_eq!(adapter.metrics().confirmed_event_count, 10_000);
}

#[test]
fn full_d1_fixture_reaches_the_committed_final_cursor() {
    let fixture = fixture();
    let (transport, state) = FixtureTransport::new();
    state
        .lock()
        .unwrap()
        .receives
        .extend(fixture.events.iter().cloned().map(|event| Ok(Some(event))));
    let mut adapter = connected_adapter(&fixture, transport, &state);

    for _ in &fixture.events {
        adapter.pump(Duration::ZERO).unwrap();
    }

    assert_eq!(
        adapter.projection().cursor(),
        Some(&fixture.expected_final_cursor)
    );
    let view = adapter.projection().view().unwrap();
    assert_eq!(view.assistant_stream, "D1 cockpit state");
    assert_eq!(view.lanes.len(), 1);
    assert_eq!(view.tasks.len(), 1);
    assert_eq!(view.merge_gates.len(), 1);
}

#[test]
fn fifty_thousand_rows_keep_a_stable_bounded_scroll_anchor() {
    let rows = transcript_rows(50_000);
    let anchor = TranscriptRowId("session-d1:25000".to_string());
    let mut viewport = TranscriptViewport::new(240);

    viewport.replace_rows(rows, Some(&anchor));

    assert_eq!(viewport.anchor(), Some(&anchor));
    assert_eq!(viewport.anchor_offset(), Some(120));
    assert_eq!(viewport.rows().len(), 240);
    assert_eq!(viewport.rows()[120].id, anchor);
}

fn transcript_rows(count: u64) -> Vec<TranscriptRow> {
    (0..count)
        .map(|ordinal| {
            serde_json::from_value(serde_json::json!({
                "id": format!("session-d1:{ordinal}"),
                "cursor": {
                    "session_id": "session-d1",
                    "ordinal": ordinal,
                },
                "timestamp": ordinal,
                "kind": {
                    "type": "session_meta",
                    "entry": {
                        "timestamp": ordinal,
                        "key": "fixture-row",
                        "value": ordinal.to_string(),
                    }
                }
            }))
            .expect("generated transcript row should match the Core contract")
        })
        .collect()
}
