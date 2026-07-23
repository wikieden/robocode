use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{CoreClientError, RuntimeSnapshot, RuntimeViewState};
use viden_gui::{D6ConnectionState, D6State, GuiCoreAdapter};

mod support;
use support::TestCoreClient;

const D1_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

fn view() -> RuntimeViewState {
    let fixture: serde_json::Value = serde_json::from_str(D1_FIXTURE).unwrap();
    RuntimeViewState::new(
        serde_json::from_value::<RuntimeSnapshot>(fixture["initial_snapshot"].clone()).unwrap(),
    )
}

#[test]
fn event_gap_blocks_business_success_until_snapshot_recovery_is_published() {
    let client = TestCoreClient::new(view(), Arc::new(Mutex::new(Vec::new()))).with_recv_error(
        CoreClientError::SnapshotRequired {
            reason_code: "event_gap".into(),
        },
    );
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().unwrap();

    assert!(adapter.pump(Duration::ZERO).is_err());
    let recovering = adapter.d6_recovery();
    assert_eq!(recovering.connection, D6ConnectionState::Recovering);
    assert_eq!(recovering.state, D6State::EventGap);
    assert!(recovering.business_success_blocked);

    adapter
        .recover()
        .expect("validated Core snapshot completes recovery");
    let live = adapter.d6_recovery();
    assert_eq!(live.connection, D6ConnectionState::Live);
    assert!(!live.business_success_blocked);
}

#[test]
fn incompatible_schema_and_transport_disconnect_remain_explicit_non_live_states() {
    let incompatible = TestCoreClient::new(view(), Arc::new(Mutex::new(Vec::new())))
        .with_recv_error(CoreClientError::Compatibility(
            "schema 9 is unsupported".into(),
        ));
    let mut adapter = GuiCoreAdapter::new(Box::new(incompatible));
    adapter.connect().unwrap();
    assert!(adapter.pump(Duration::ZERO).is_err());
    assert_eq!(adapter.d6_recovery().state, D6State::IncompatibleSchema);

    let disconnected = TestCoreClient::new(view(), Arc::new(Mutex::new(Vec::new())))
        .with_recv_error(CoreClientError::Transport("bridge dropped".into()));
    let mut adapter = GuiCoreAdapter::new(Box::new(disconnected));
    adapter.connect().unwrap();
    assert!(adapter.pump(Duration::ZERO).is_err());
    assert_eq!(adapter.d6_recovery().state, D6State::Disconnected);
    assert!(adapter.d6_recovery().business_success_blocked);
}
