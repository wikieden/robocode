//! The palette's `~` file scope, read from the Core workspace inventory
//! (GUI-CORE-022).
//!
//! These tests cover the correlation machine, the capability gate, and the
//! refusal path. Correlation has only two cases here, not the three
//! `d14_audit_trail.rs` covers: `WorkspaceFilesLoaded.command_id` is a
//! *required* field, so a page either names this read or belongs to another
//! one. There is no legacy id-less page and therefore no acceptance-first
//! fallback — the residual limitation the audit read still documents does not
//! exist on this contract.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{
    EventCursor, FRONTEND_SCHEMA_V1, RuntimeCommand, RuntimeCommandEnvelope, RuntimeErrorView,
    RuntimeEvent, RuntimeEventEnvelope, RuntimeEventKind, RuntimeOwner, RuntimeSnapshot,
    RuntimeViewState, RuntimeWireEvent, WorkspaceFileEntry, WorkspaceFileKind, WorkspaceFilePage,
};
use viden_gui::{GuiCoreAdapter, WORKSPACE_FILES_CAPABILITY};

mod support;
use support::TestCoreClient;

const TIMEOUT: Duration = Duration::from_millis(10);

fn view() -> RuntimeViewState {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../crates/types/tests/fixtures/frontend-contract-v1/multi-lane.json"
    ))
    .expect("fixture json");
    let snapshot: RuntimeSnapshot =
        serde_json::from_value(fixture["initial_snapshot"].clone()).expect("fixture snapshot");
    RuntimeViewState::new(snapshot)
}

fn envelope(sequence: u64, kind: RuntimeEventKind) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: RuntimeOwner::default(),
        cursor: EventCursor {
            stream_id: "gui-test".to_string(),
            sequence,
        },
        event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
            sequence,
            Some(1_700_000_000 + sequence),
            kind,
        )),
    }
}

fn accepted(sequence: u64, command_id: &str) -> RuntimeEventEnvelope {
    envelope(
        sequence,
        RuntimeEventKind::CommandAccepted {
            command_id: command_id.to_string(),
            command: RuntimeCommand::QueryWorkspaceFiles {
                query: Default::default(),
            },
        },
    )
}

fn page(paths: &[(&str, WorkspaceFileKind)]) -> WorkspaceFilePage {
    WorkspaceFilePage {
        entries: paths
            .iter()
            .map(|(path, kind)| WorkspaceFileEntry {
                path: (*path).to_string(),
                kind: *kind,
                size_bytes: matches!(kind, WorkspaceFileKind::File).then_some(64),
            })
            .collect(),
        next_after: None,
        complete: true,
    }
}

fn loaded(sequence: u64, command_id: &str, page: WorkspaceFilePage) -> RuntimeEventEnvelope {
    envelope(
        sequence,
        RuntimeEventKind::WorkspaceFilesLoaded {
            command_id: command_id.to_string(),
            page,
        },
    )
}

fn refused(sequence: u64, message: &str) -> RuntimeEventEnvelope {
    envelope(
        sequence,
        RuntimeEventKind::Error {
            error: RuntimeErrorView {
                message: message.to_string(),
                recoverable: true,
                hint: None,
            },
        },
    )
}

struct Harness {
    adapter: GuiCoreAdapter,
    sent: Arc<Mutex<Vec<RuntimeCommandEnvelope>>>,
}

fn harness(events: Vec<RuntimeEventEnvelope>, with_capability: bool) -> Harness {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut client = TestCoreClient::new(view(), sent.clone());
    if !with_capability {
        client.capabilities.remove(WORKSPACE_FILES_CAPABILITY);
    }
    for event in events {
        client = client.with_envelope(event);
    }
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().expect("connect");
    Harness { adapter, sent }
}

fn file_queries(
    sent: &Arc<Mutex<Vec<RuntimeCommandEnvelope>>>,
) -> Vec<viden_core::WorkspaceFilesQuery> {
    sent.lock()
        .expect("sent lock")
        .iter()
        .filter_map(|envelope| match &envelope.command {
            RuntimeCommand::QueryWorkspaceFiles { query } => Some(query.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_confirming_page_becomes_the_palette_inventory_in_cores_own_order() {
    let mut harness = harness(
        vec![
            accepted(1, "gui-files-1"),
            loaded(
                2,
                "gui-files-1",
                page(&[
                    ("AGENTS.md", WorkspaceFileKind::File),
                    ("crates", WorkspaceFileKind::Dir),
                    ("crates/core/src/lib.rs", WorkspaceFileKind::File),
                ]),
            ),
        ],
        true,
    );
    let projection = harness
        .adapter
        .query_workspace_files_and_wait("gui-files-1", TIMEOUT)
        .expect("inventory read");

    assert!(projection.capability_available);
    assert!(projection.loaded);
    assert!(projection.complete);
    assert_eq!(projection.pending_command_id, None);
    assert_eq!(projection.outcome.state, "confirmed");
    // Core's lexicographic order, verbatim: the client never re-sorts.
    assert_eq!(
        projection
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        vec!["AGENTS.md", "crates", "crates/core/src/lib.rs"]
    );
    let dir = &projection.entries[1];
    assert_eq!(dir.kind, "dir");
    assert_eq!(dir.size_bytes, None, "a directory publishes no byte size");

    // The read asks for the whole tree with no prefix: the palette fuzzy-matches
    // locally, so it never sends a filter the operator did not type.
    let queries = file_queries(&harness.sent);
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].prefix, None);
    assert_eq!(queries[0].after, None);
}

#[test]
fn a_page_naming_another_read_is_ignored_with_no_acceptance_fallback() {
    let mut harness = harness(
        vec![
            accepted(1, "gui-files-1"),
            // Another client's answer, delivered while ours is outstanding.
            // The id is required on this event, so there is nothing to guess
            // with and the page is simply not ours.
            loaded(
                2,
                "gui-files-other",
                page(&[("leaked/from/another/read.rs", WorkspaceFileKind::File)]),
            ),
        ],
        true,
    );
    let projection = harness
        .adapter
        .query_workspace_files_and_wait("gui-files-1", TIMEOUT)
        .expect("inventory read");

    assert!(
        !projection.loaded,
        "another read's page must not settle ours"
    );
    assert!(projection.entries.is_empty());
    assert_eq!(
        projection.pending_command_id.as_deref(),
        Some("gui-files-1"),
        "our read is still outstanding"
    );
    assert_eq!(projection.outcome.state, "pending");
}

#[test]
fn a_refused_read_surfaces_cores_denial_and_never_an_empty_inventory() {
    let mut harness = harness(
        vec![
            accepted(1, "gui-files-1"),
            refused(
                2,
                "Permission decision: deny\n  tool: workspace_file_inventory",
            ),
        ],
        true,
    );
    let projection = harness
        .adapter
        .query_workspace_files_and_wait("gui-files-1", TIMEOUT)
        .expect("inventory read");

    assert_eq!(projection.outcome.state, "rejected");
    assert!(
        projection
            .outcome
            .reason
            .as_deref()
            .is_some_and(|detail| detail.contains("workspace_file_inventory")),
        "the refusal must name the permission, got {:?}",
        projection.outcome.reason
    );
    // A refusal and an empty workspace must never render the same.
    assert!(!projection.loaded);
    assert!(projection.entries.is_empty());
}

#[test]
fn the_file_scope_sends_nothing_without_the_core_capability() {
    let mut harness = harness(Vec::new(), false);
    let projection = harness
        .adapter
        .query_workspace_files_and_wait("gui-files-1", TIMEOUT)
        .expect("inventory read");

    assert!(!projection.capability_available);
    assert!(!projection.loaded);
    assert!(projection.entries.is_empty());
    assert_eq!(projection.outcome.state, "idle");
    assert!(
        file_queries(&harness.sent).is_empty(),
        "a missing capability must send no command at all"
    );
}

#[test]
fn a_second_read_is_refused_locally_while_one_is_in_flight() {
    let mut harness = harness(vec![accepted(1, "gui-files-1")], true);
    harness
        .adapter
        .query_workspace_files_and_wait("gui-files-1", TIMEOUT)
        .expect("first read");
    let error = harness
        .adapter
        .query_workspace_files_and_wait("gui-files-2", TIMEOUT)
        .expect_err("a second concurrent read must be refused");
    assert!(error.contains("gui-files-1"), "got {error}");
    // Refused locally means nothing was sent, so the in-flight read keeps its
    // correlation.
    assert_eq!(file_queries(&harness.sent).len(), 1);
}
