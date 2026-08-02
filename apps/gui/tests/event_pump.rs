use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{RuntimeEventEnvelope, RuntimeSnapshot, RuntimeViewState};
use viden_gui::GuiCoreAdapter;

mod support;
use support::TestCoreClient;

const MULTI_LANE_FIXTURE: &str =
    include_str!("../../../crates/types/tests/fixtures/frontend-contract-v1/multi-lane.json");

/// The fixture's initial snapshot plus its own ordered events, so the pump is
/// exercised against facts Core really publishes rather than a synthetic kind
/// the D1 projection is contractually allowed to ignore.
fn fixture_parts() -> (RuntimeViewState, Vec<RuntimeEventEnvelope>) {
    let fixture: serde_json::Value = serde_json::from_str(MULTI_LANE_FIXTURE).unwrap();
    let snapshot: RuntimeSnapshot =
        serde_json::from_value(fixture["initial_snapshot"].clone()).unwrap();
    let events: Vec<RuntimeEventEnvelope> = fixture["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| serde_json::from_value(event.clone()).unwrap())
        .collect();
    assert!(
        !events.is_empty(),
        "the fixture must publish ordered events"
    );
    (RuntimeViewState::new(snapshot), events)
}

#[test]
fn pump_advances_the_projection_when_core_publishes_an_ordered_event() {
    let (view, events) = fixture_parts();
    let mut client = TestCoreClient::new(view, Arc::new(Mutex::new(Vec::new())));
    for envelope in events {
        client = client.with_envelope(envelope);
    }
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().unwrap();
    let before = adapter.d1_cockpit(None).expect("cockpit projection");

    // The pump drains the ordered queue and refreshes the projection, so the
    // shell can be woken by a push instead of holding its own drain timer.
    assert!(adapter.pump_events(Duration::ZERO));

    let after = adapter.d1_cockpit(None).expect("cockpit projection");
    assert_ne!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&after).unwrap(),
        "the pumped events must advance the projection the shell renders"
    );
    assert!(
        !after.lanes.is_empty(),
        "the multi-lane fixture publishes lanes through its ordered events"
    );
}

#[test]
fn pump_reports_quiet_when_no_event_is_waiting() {
    let (view, _) = fixture_parts();
    let client = TestCoreClient::new(view, Arc::new(Mutex::new(Vec::new())));
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().unwrap();

    // A quiet pump must not report progress: the emitter uses this to decide
    // whether to wake the frontend, and false wakes recreate timer churn.
    assert!(!adapter.pump_events(Duration::ZERO));
}

#[test]
fn pump_drains_a_burst_in_one_pass() {
    let (view, events) = fixture_parts();
    let published = events.len();
    let mut client = TestCoreClient::new(view, Arc::new(Mutex::new(Vec::new())));
    for envelope in events {
        client = client.with_envelope(envelope);
    }
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().unwrap();

    assert!(adapter.pump_events(Duration::ZERO));
    // One pass consumes the whole queued burst, so a stream of deltas costs
    // one projection refresh and one frontend wake rather than one each.
    assert!(
        !adapter.pump_events(Duration::ZERO),
        "{published} events must drain in one pass"
    );
}
