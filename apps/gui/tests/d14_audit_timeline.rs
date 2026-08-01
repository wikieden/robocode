use std::sync::{Arc, Mutex};

use viden_core::{
    EventCursor, FRONTEND_SCHEMA_V1, ReplayBatch, RuntimeEvent, RuntimeEventEnvelope,
    RuntimeEventKind, RuntimeOwner, RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent,
};
use viden_gui::GuiCoreAdapter;

mod support;
use support::TestCoreClient;

fn owner(lane: &str) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-viden".to_string(),
        project_id: "project-viden".to_string(),
        lane_id: Some(lane.to_string()),
        ..Default::default()
    }
}

fn envelope(sequence: u64, lane: &str, kind: RuntimeEventKind) -> RuntimeEventEnvelope {
    let mut event = RuntimeEvent::new(sequence, kind);
    event.timestamp = Some(1_700_000_000 + sequence);
    RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: owner(lane),
        cursor: EventCursor {
            stream_id: "gui-test".to_string(),
            sequence,
        },
        event: RuntimeWireEvent::Known(event),
    }
}

fn batch(events: Vec<RuntimeEventEnvelope>, next: u64, complete: bool) -> ReplayBatch {
    ReplayBatch {
        events,
        next: EventCursor {
            stream_id: "gui-test".to_string(),
            sequence: next,
        },
        complete,
    }
}

fn adapter_with(batches: Vec<ReplayBatch>) -> GuiCoreAdapter {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../crates/types/tests/fixtures/frontend-contract-v1/multi-lane.json"
    ))
    .expect("fixture json");
    let snapshot: RuntimeSnapshot =
        serde_json::from_value(fixture["initial_snapshot"].clone()).expect("fixture snapshot");
    let view = RuntimeViewState::new(snapshot);
    let mut client = TestCoreClient::new(view, Arc::new(Mutex::new(Vec::new())));
    for batch in batches {
        client = client.with_replay_batch(batch);
    }
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().unwrap();
    adapter
}

#[test]
fn d14_pages_the_audit_timeline_through_the_core_replay_cursor() {
    let mut adapter = adapter_with(vec![
        batch(
            vec![
                envelope(
                    1,
                    "lane-1",
                    RuntimeEventKind::Error {
                        error: viden_core::RuntimeErrorView {
                            message: "provider unavailable".to_string(),
                            recoverable: true,
                            hint: None,
                        },
                    },
                ),
                envelope(
                    2,
                    "lane-2",
                    RuntimeEventKind::AssistantDelta {
                        message_id: "message-1".to_string(),
                        task_id: None,
                        content: "hello".to_string(),
                    },
                ),
            ],
            2,
            false,
        ),
        batch(vec![], 2, true),
    ]);

    let first = adapter.d14_audit_timeline(None, 50).expect("first page");
    assert_eq!(first.rows.len(), 2);
    assert_eq!(first.next_cursor.as_deref(), Some("gui-test:2"));
    assert!(!first.complete);

    // Ordering and identity come from the Core cursor, never from timestamps.
    assert_eq!(first.rows[0].sequence, 1);
    assert_eq!(first.rows[0].lane_id.as_deref(), Some("lane-1"));
    assert_eq!(first.rows[1].sequence, 2);
    assert_eq!(first.rows[1].lane_id.as_deref(), Some("lane-2"));

    let second = adapter
        .d14_audit_timeline(Some("gui-test:2"), 50)
        .expect("second page");
    assert!(second.rows.is_empty());
    assert!(second.complete);
}

#[test]
fn d14_labels_each_row_with_the_canonical_core_event_kind() {
    let mut adapter = adapter_with(vec![batch(
        vec![envelope(
            7,
            "lane-1",
            RuntimeEventKind::AssistantDelta {
                message_id: "message-7".to_string(),
                task_id: None,
                content: "chunk".to_string(),
            },
        )],
        7,
        true,
    )]);
    let page = adapter.d14_audit_timeline(None, 10).unwrap();
    // The label is Core's own serde discriminant, not a client-side rename.
    assert_eq!(page.rows[0].kind, "assistant_delta");
    assert_eq!(page.rows[0].timestamp, Some(1_700_000_007));
    assert!(page.rows[0].known);
}

#[test]
fn d14_keeps_an_unknown_event_in_the_timeline_instead_of_dropping_it() {
    let unknown = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: owner("lane-9"),
        cursor: EventCursor {
            stream_id: "gui-test".to_string(),
            sequence: 11,
        },
        event: RuntimeWireEvent::Unknown {
            event_type: "core.future_fact".to_string(),
            payload: serde_json::Value::Null,
        },
    };
    let mut adapter = adapter_with(vec![batch(vec![unknown], 11, true)]);
    let page = adapter.d14_audit_timeline(None, 10).unwrap();

    assert_eq!(
        page.rows.len(),
        1,
        "an audit trail must not silently drop a row"
    );
    assert!(!page.rows[0].known);
    assert_eq!(page.rows[0].kind, "unknown");
    assert_eq!(page.rows[0].sequence, 11);
}

#[test]
fn d14_reports_a_replay_failure_instead_of_rendering_a_partial_trail() {
    let mut adapter = adapter_with(Vec::new());
    let error = adapter
        .d14_audit_timeline(None, 10)
        .expect_err("replay is unavailable in this fixture");
    assert!(
        error.contains("replay"),
        "the failure must name the replay transport, got {error}"
    );
}
