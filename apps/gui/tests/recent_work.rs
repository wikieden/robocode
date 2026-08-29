//! Cross-project recent work flows through the Core inventory contract.
//!
//! `QueryRecentWork` is read-only and is answered by exactly `CommandAccepted`
//! followed by `RecentWorkLoaded` (`docs/frontend-integration-contract.md`,
//! "Recent Work Contract"). The GUI never scans `<session-home>/projects`,
//! never reads a transcript, and never treats acceptance alone as the answer:
//! only the ordered `RecentWorkLoaded` fact populates the rows the Welcome
//! screen and the project picker render.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{
    RecentProjectSummary, RecentSessionSummary, RecentWorkQuery, RuntimeCommand,
    RuntimeEventEnvelope, RuntimeEventKind, RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent,
};
use viden_gui::{GuiCoreAdapter, RECENT_WORK_CAPABILITY};

mod support;
use support::TestCoreClient;

const D1_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

#[derive(serde::Deserialize)]
struct Fixture {
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
}

fn d1_view() -> RuntimeViewState {
    let fixture: Fixture = serde_json::from_str(D1_FIXTURE).expect("parse D1 fixture");
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    view
}

fn connected(client: TestCoreClient) -> GuiCoreAdapter {
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().expect("connect recent-work client");
    adapter
}

fn sent_commands(
    sent: &Arc<Mutex<Vec<viden_core::RuntimeCommandEnvelope>>>,
) -> Vec<viden_core::RuntimeCommandEnvelope> {
    sent.lock().expect("sent commands").clone()
}

fn project(root: &str, name: &str, updated: u64) -> RecentProjectSummary {
    RecentProjectSummary {
        canonical_root: root.to_string(),
        display_name: name.to_string(),
        last_updated_at: updated,
        latest_session_id: Some(format!("session-{name}")),
    }
}

fn session(root: &str, id: &str, updated: u64) -> RecentSessionSummary {
    RecentSessionSummary {
        canonical_root: root.to_string(),
        session_id: id.to_string(),
        created_at: updated - 60,
        last_updated_at: updated,
        message_count: 12,
        tool_call_count: 3,
        command_count: 1,
    }
}

#[test]
fn query_sends_the_exact_query_recent_work_command() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    let result = adapter
        .query_recent_work_and_wait("gui-recent-1", 8, Duration::ZERO)
        .expect("recent-work query dispatches");

    let commands = sent_commands(&sent);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command_id, "gui-recent-1");
    // The inventory is user-scoped, never Lane-scoped.
    assert_eq!(commands[0].owner, viden_core::RuntimeOwner::default());
    assert_eq!(
        commands[0].command,
        RuntimeCommand::QueryRecentWork {
            query: RecentWorkQuery { limit: 8 }
        }
    );
    // Acceptance has not arrived, so nothing is renderable yet.
    assert_eq!(result.outcome.state, "pending");
    assert_eq!(result.pending_command_id.as_deref(), Some("gui-recent-1"));
    assert!(result.projects.is_empty());
    assert!(result.sessions.is_empty());
    assert!(result.capability_available);
}

#[test]
fn only_recent_work_loaded_after_acceptance_confirms_the_query() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent))
        .with_event(RuntimeEventKind::CommandAccepted {
            command_id: "gui-recent-1".into(),
            command: RuntimeCommand::QueryRecentWork {
                query: RecentWorkQuery { limit: 8 },
            },
        })
        // A republished snapshot is not an inventory answer.
        .with_event(RuntimeEventKind::SnapshotUpdated {
            snapshot: d1_view().snapshot.clone(),
        })
        .with_event(RuntimeEventKind::RecentWorkLoaded {
            projects: vec![
                project("/workspace/viden", "viden", 1_700_000_200),
                project("/workspace/spatial-lm", "spatial-lm", 1_700_000_100),
            ],
            sessions: vec![session("/workspace/viden", "session-viden", 1_700_000_200)],
            diagnostics: vec!["recent.index_stale".into()],
        });
    let mut adapter = connected(client);

    let result = adapter
        .query_recent_work_and_wait("gui-recent-1", 8, Duration::from_millis(10))
        .expect("recent-work query confirms");

    assert_eq!(result.outcome.state, "confirmed");
    assert_eq!(result.pending_command_id, None);
    // Ordering is Core's; the client re-serializes without re-sorting.
    assert_eq!(
        result
            .projects
            .iter()
            .map(|entry| entry.canonical_root.as_str())
            .collect::<Vec<_>>(),
        vec!["/workspace/viden", "/workspace/spatial-lm"]
    );
    assert_eq!(result.projects[0].display_name, "viden");
    assert_eq!(result.projects[0].last_updated_at, 1_700_000_200);
    assert_eq!(
        result.projects[0].latest_session_id.as_deref(),
        Some("session-viden")
    );
    assert_eq!(result.sessions.len(), 1);
    assert_eq!(result.sessions[0].session_id, "session-viden");
    assert_eq!(result.sessions[0].message_count, 12);
    // Core's diagnostics are rendered verbatim, never reworded.
    assert_eq!(result.diagnostics, vec!["recent.index_stale".to_string()]);
}

#[test]
fn recent_work_loaded_without_acceptance_does_not_confirm_the_query() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    // Another client's inventory answer arrives first. It carries no command
    // id, so it must not be adopted as this command's receipt.
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent)).with_event(
        RuntimeEventKind::RecentWorkLoaded {
            projects: vec![project("/workspace/other", "other", 1_700_000_000)],
            sessions: Vec::new(),
            diagnostics: Vec::new(),
        },
    );
    let mut adapter = connected(client);

    let result = adapter
        .query_recent_work_and_wait("gui-recent-1", 8, Duration::from_millis(10))
        .expect("recent-work query dispatches");

    assert_eq!(result.outcome.state, "pending");
    assert!(result.projects.is_empty());
}

#[test]
fn a_later_poll_confirms_a_query_the_send_call_left_pending() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent))
        .with_gap()
        .with_event(RuntimeEventKind::CommandAccepted {
            command_id: "gui-recent-slow".into(),
            command: RuntimeCommand::QueryRecentWork {
                query: RecentWorkQuery { limit: 5 },
            },
        })
        .with_event(RuntimeEventKind::RecentWorkLoaded {
            projects: vec![project("/workspace/viden", "viden", 1_700_000_300)],
            sessions: Vec::new(),
            diagnostics: Vec::new(),
        });
    let mut adapter = connected(client);

    let pending = adapter
        .query_recent_work_and_wait("gui-recent-slow", 5, Duration::ZERO)
        .expect("recent-work query dispatches");
    assert_eq!(pending.outcome.state, "pending");

    let confirmed = adapter
        .poll_recent_work(Duration::from_millis(10))
        .expect("the later poll drains the inventory fact");

    assert_eq!(confirmed.outcome.state, "confirmed");
    assert_eq!(confirmed.projects.len(), 1);
    // Polling never re-sends the read.
    assert_eq!(sent_commands(&sent).len(), 1);
}

#[test]
fn a_core_rejection_carries_the_reason_through_to_the_client() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent)).with_event(
        RuntimeEventKind::CommandRejected {
            command_id: "gui-recent-1".into(),
            reason: "recent work inventory is unavailable".into(),
        },
    );
    let mut adapter = connected(client);

    let result = adapter
        .query_recent_work_and_wait("gui-recent-1", 8, Duration::from_millis(10))
        .expect("a rejection is a projected outcome, not a transport error");

    assert_eq!(result.outcome.state, "rejected");
    assert_eq!(
        result.outcome.reason.as_deref(),
        Some("recent work inventory is unavailable")
    );
    assert_eq!(result.pending_command_id, None);
    assert!(result.projects.is_empty());
}

#[test]
fn a_second_query_is_refused_while_one_is_still_pending() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    adapter
        .query_recent_work_and_wait("gui-recent-1", 8, Duration::ZERO)
        .expect("first recent-work query dispatches");
    let error = adapter
        .query_recent_work_and_wait("gui-recent-2", 8, Duration::ZERO)
        .expect_err("a second in-flight read is refused");

    assert!(error.contains("gui-recent-1"), "{error}");
    assert_eq!(sent_commands(&sent).len(), 1);
}

#[test]
fn an_absent_capability_blocks_the_query_and_is_readable_by_the_client() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut client = TestCoreClient::new(d1_view(), Arc::clone(&sent));
    client.capabilities.remove(RECENT_WORK_CAPABILITY);
    let mut adapter = connected(client);

    assert!(!adapter.supports_recent_work());
    let error = adapter
        .query_recent_work_and_wait("gui-recent-1", 8, Duration::ZERO)
        .expect_err("a missing capability fails closed");
    assert!(error.contains(RECENT_WORK_CAPABILITY), "{error}");
    assert!(sent_commands(&sent).is_empty());
}

#[test]
fn the_capability_is_read_from_the_handshake_when_core_publishes_it() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));
    assert!(adapter.supports_recent_work());
}
