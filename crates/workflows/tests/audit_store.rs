//! Append-only audit timeline store contract.
//!
//! The audit log is the operator-facing record of who changed what. Its
//! invariants are stronger than the fail-soft workflow projections: a record
//! is never silently dropped, never carries prose or secret bytes, and never
//! depends on the derived SQLite index for its answers.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use viden_types::{
    AuditActor, AuditActorFilter, AuditCursor, AuditObjectRef, AuditOutcome, AuditQuery,
    AuditRecord, RuntimeOwner, fresh_id,
};
use viden_workflows::stores::WorkflowStore;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("viden_audit_{name}_{}", fresh_id("tmp")));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn store(name: &str) -> (PathBuf, PathBuf, WorkflowStore) {
    let home = temp_dir(&format!("{name}_home"));
    let cwd = temp_dir(&format!("{name}_cwd"));
    let store = WorkflowStore::new(&home, &cwd).unwrap();
    (home, cwd, store)
}

fn owner(project_id: &str, lane_id: Option<&str>) -> RuntimeOwner {
    RuntimeOwner {
        workspace_id: "workspace-1".to_string(),
        project_id: project_id.to_string(),
        lane_id: lane_id.map(ToString::to_string),
        session_id: Some("session-1".to_string()),
        task_id: Some("task-1".to_string()),
        turn_id: None,
    }
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

fn record(
    audit_id: &str,
    timestamp: u64,
    owner: RuntimeOwner,
    action: &str,
    objects: Vec<AuditObjectRef>,
) -> AuditRecord {
    AuditRecord::sanitized(
        audit_id.to_string(),
        timestamp,
        owner,
        AuditActor::Operator,
        action.to_string(),
        objects,
        AuditOutcome::Success,
        args(&[("outcome", "success")]),
    )
    .unwrap()
}

fn object(kind: &str, id: &str) -> AuditObjectRef {
    AuditObjectRef {
        kind: kind.to_string(),
        id: id.to_string(),
    }
}

fn seed_interleaved(store: &WorkflowStore) -> Vec<AuditRecord> {
    let records = vec![
        record(
            "audit_0001",
            10,
            owner("project-1", Some("lane-a")),
            "lane.created",
            vec![object(AuditObjectRef::KIND_LANE, "lane-a")],
        ),
        record(
            "audit_0002",
            20,
            owner("project-1", None),
            "permission.decided",
            vec![
                object(AuditObjectRef::KIND_PERMISSION, "perm-1"),
                object(AuditObjectRef::KIND_LANE, "lane-a"),
            ],
        ),
        record(
            "audit_0003",
            30,
            owner("project-2", Some("lane-b")),
            "evidence.recorded",
            vec![object(AuditObjectRef::KIND_EVIDENCE, "evidence-1")],
        ),
        record(
            "audit_0004",
            40,
            owner("project-1", Some("lane-a")),
            "gate.decided",
            vec![
                object(AuditObjectRef::KIND_MERGE_GATE, "gate-1"),
                object(AuditObjectRef::KIND_TASK, "task-1"),
            ],
        ),
        record(
            "audit_0005",
            50,
            owner("project-1", Some("lane-a")),
            "change.applied",
            vec![object(AuditObjectRef::KIND_APPLIED_CHANGE, "change-1")],
        ),
        record(
            "audit_0006",
            60,
            owner("project-1", Some("lane-a")),
            "change.reverted",
            vec![
                object(AuditObjectRef::KIND_REVERT, "revert-1"),
                object(AuditObjectRef::KIND_APPLIED_CHANGE, "change-1"),
            ],
        ),
    ];
    for entry in &records {
        store.append_audit_record(entry).unwrap();
    }
    records
}

fn ids(page: &viden_types::AuditPage) -> Vec<String> {
    page.records
        .iter()
        .map(|record| record.audit_id.clone())
        .collect()
}

fn query(limit: u32) -> AuditQuery {
    AuditQuery {
        limit,
        ..AuditQuery::default()
    }
}

/// One record by a chosen actor, so the actor filter has something to separate.
fn actor_record(audit_id: &str, timestamp: u64, actor: AuditActor) -> AuditRecord {
    AuditRecord::sanitized(
        audit_id.to_string(),
        timestamp,
        owner("project-1", Some("lane-a")),
        actor,
        "gate.decided".to_string(),
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
        AuditOutcome::Success,
        args(&[("outcome", "success")]),
    )
    .unwrap()
}

/// Four records, one per actor shape the filter must distinguish.
fn seed_actors(store: &WorkflowStore) {
    for entry in [
        actor_record("audit_0001", 10, AuditActor::Operator),
        actor_record("audit_0002", 20, AuditActor::System),
        actor_record(
            "audit_0003",
            30,
            AuditActor::Agent {
                agent_id: "agent-planner".to_string(),
            },
        ),
        actor_record(
            "audit_0004",
            40,
            AuditActor::Agent {
                agent_id: "agent-coder".to_string(),
            },
        ),
    ] {
        store.append_audit_record(&entry).unwrap();
    }
}

#[test]
fn audit_query_filters_by_actor() {
    let (_home, _cwd, store) = store("actor_filter");
    seed_actors(&store);

    let operator = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::Operator),
            ..query(10)
        })
        .unwrap();
    assert_eq!(ids(&operator), vec!["audit_0001".to_string()]);

    let system = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::System),
            ..query(10)
        })
        .unwrap();
    assert_eq!(ids(&system), vec!["audit_0002".to_string()]);

    // `AnyAgent` is the "not a human, not the runtime" chip: it matches every
    // agent lane without the operator having to name one.
    let any_agent = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::AnyAgent),
            ..query(10)
        })
        .unwrap();
    assert_eq!(
        ids(&any_agent),
        vec!["audit_0004".to_string(), "audit_0003".to_string()]
    );

    let named_agent = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::Agent {
                agent_id: "agent-planner".to_string(),
            }),
            ..query(10)
        })
        .unwrap();
    assert_eq!(ids(&named_agent), vec!["audit_0003".to_string()]);

    // A named agent id matches exactly; it is never a prefix or substring test.
    let unknown_agent = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::Agent {
                agent_id: "agent-plan".to_string(),
            }),
            ..query(10)
        })
        .unwrap();
    assert!(unknown_agent.records.is_empty());
    assert!(unknown_agent.complete);
}

#[test]
fn audit_query_filters_by_a_half_open_time_range() {
    let (_home, _cwd, store) = store("time_filter");
    seed_interleaved(&store);

    // `from` is inclusive and `until` is exclusive, so two adjacent windows
    // tile the timeline without overlapping or dropping a record.
    let window = store
        .query_audit(&AuditQuery {
            from: Some(20),
            until: Some(50),
            ..query(10)
        })
        .unwrap();
    assert_eq!(
        ids(&window),
        vec![
            "audit_0004".to_string(),
            "audit_0003".to_string(),
            "audit_0002".to_string(),
        ]
    );

    let older = store
        .query_audit(&AuditQuery {
            until: Some(20),
            ..query(10)
        })
        .unwrap();
    assert_eq!(ids(&older), vec!["audit_0001".to_string()]);

    let newer = store
        .query_audit(&AuditQuery {
            from: Some(50),
            ..query(10)
        })
        .unwrap();
    assert_eq!(
        ids(&newer),
        vec!["audit_0006".to_string(), "audit_0005".to_string()]
    );
}

/// The whole point of a server-side filter: `complete` and `next_before` must
/// describe the *filtered* timeline. A client-side filter over an unfiltered
/// page reports completeness for records it never asked about.
#[test]
fn audit_filters_apply_before_pagination_so_completeness_is_the_filtered_timeline() {
    let (_home, _cwd, store) = store("filter_before_paging");
    seed_actors(&store);

    // Two agent records exist; a page of one is therefore incomplete and its
    // cursor names the agent record it stopped at, never the newest record of
    // the unfiltered timeline.
    let first = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::AnyAgent),
            ..query(1)
        })
        .unwrap();
    assert_eq!(ids(&first), vec!["audit_0004".to_string()]);
    assert!(!first.complete);
    assert_eq!(
        first.next_before,
        Some(AuditCursor {
            timestamp: 40,
            audit_id: "audit_0004".to_string(),
        })
    );

    let second = store
        .query_audit(&AuditQuery {
            actor: Some(AuditActorFilter::AnyAgent),
            before: first.next_before.clone(),
            ..query(1)
        })
        .unwrap();
    assert_eq!(ids(&second), vec!["audit_0003".to_string()]);
    assert!(
        second.complete,
        "the filtered timeline is exhausted even though older unfiltered records remain"
    );
    assert_eq!(second.next_before, None);

    // The unfiltered read over the same store still sees everything, so the
    // filtered completeness above is a filter fact, not a missing-record bug.
    let unfiltered = store.query_audit(&query(10)).unwrap();
    assert_eq!(unfiltered.records.len(), 4);
}

/// An inverted range is a caller bug. Answering it with an empty page would
/// read as "nothing happened in that window", so it is rejected instead.
#[test]
fn audit_query_rejects_an_inverted_time_range() {
    let (_home, _cwd, store) = store("inverted_range");
    seed_interleaved(&store);

    let error = store
        .query_audit(&AuditQuery {
            from: Some(50),
            until: Some(20),
            ..query(10)
        })
        .unwrap_err();
    assert!(error.contains("from"), "unexpected error: {error}");

    // An equal bound is legal and empty by construction: `until` is exclusive.
    let empty = store
        .query_audit(&AuditQuery {
            from: Some(20),
            until: Some(20),
            ..query(10)
        })
        .unwrap();
    assert!(empty.records.is_empty());
    assert!(empty.complete);
}

/// Additive fields must not break a page written by an older client.
#[test]
fn a_legacy_audit_query_without_filters_deserializes_to_no_filter() {
    let legacy = r#"{"project_id":null,"lane_id":null,"object":null,"before":null,"limit":10}"#;
    let query: AuditQuery = serde_json::from_str(legacy).unwrap();
    assert_eq!(query.actor, None);
    assert_eq!(query.from, None);
    assert_eq!(query.until, None);
    assert_eq!(
        query,
        AuditQuery {
            limit: 10,
            ..AuditQuery::default()
        }
    );
}

#[test]
fn audit_log_path_is_project_scoped_jsonl() {
    let (_home, _cwd, store) = store("paths");
    assert_eq!(store.paths().audit_log.file_name().unwrap(), "audit.jsonl");
    assert!(
        store
            .paths()
            .audit_log
            .starts_with(store.paths().project_dir.clone())
    );
}

#[test]
fn audit_query_returns_newest_first_across_record_kinds() {
    let (_home, _cwd, store) = store("newest_first");
    seed_interleaved(&store);

    let page = store.query_audit(&query(10)).unwrap();

    assert_eq!(
        ids(&page),
        vec![
            "audit_0006".to_string(),
            "audit_0005".to_string(),
            "audit_0004".to_string(),
            "audit_0003".to_string(),
            "audit_0002".to_string(),
            "audit_0001".to_string(),
        ]
    );
    assert!(page.complete);
    assert_eq!(page.next_before, None);
}

#[test]
fn audit_query_filters_by_project_lane_and_object() {
    let (_home, _cwd, store) = store("filters");
    seed_interleaved(&store);

    let by_project = store
        .query_audit(&AuditQuery {
            project_id: Some("project-2".to_string()),
            ..query(10)
        })
        .unwrap();
    assert_eq!(ids(&by_project), vec!["audit_0003".to_string()]);

    // `lane-a` matches through the owner lane AND through a linked lane object
    // on a record whose owner carries no lane.
    let by_lane = store
        .query_audit(&AuditQuery {
            lane_id: Some("lane-a".to_string()),
            ..query(10)
        })
        .unwrap();
    assert_eq!(
        ids(&by_lane),
        vec![
            "audit_0006".to_string(),
            "audit_0005".to_string(),
            "audit_0004".to_string(),
            "audit_0002".to_string(),
            "audit_0001".to_string(),
        ]
    );

    let by_object = store
        .query_audit(&AuditQuery {
            object: Some(object(AuditObjectRef::KIND_APPLIED_CHANGE, "change-1")),
            ..query(10)
        })
        .unwrap();
    assert_eq!(
        ids(&by_object),
        vec!["audit_0006".to_string(), "audit_0005".to_string()]
    );
}

#[test]
fn audit_pagination_walks_exact_boundaries_with_before_cursor() {
    let (_home, _cwd, store) = store("pagination");
    seed_interleaved(&store);

    let first = store.query_audit(&query(2)).unwrap();
    assert_eq!(
        ids(&first),
        vec!["audit_0006".to_string(), "audit_0005".to_string()]
    );
    assert!(!first.complete);
    assert_eq!(
        first.next_before,
        Some(AuditCursor {
            timestamp: 50,
            audit_id: "audit_0005".to_string(),
        })
    );

    let second = store
        .query_audit(&AuditQuery {
            before: first.next_before.clone(),
            ..query(2)
        })
        .unwrap();
    assert_eq!(
        ids(&second),
        vec!["audit_0004".to_string(), "audit_0003".to_string()]
    );
    assert!(!second.complete);

    let third = store
        .query_audit(&AuditQuery {
            before: second.next_before.clone(),
            ..query(2)
        })
        .unwrap();
    assert_eq!(
        ids(&third),
        vec!["audit_0002".to_string(), "audit_0001".to_string()]
    );
    assert!(third.complete);
    assert_eq!(third.next_before, None);
}

#[test]
fn audit_query_clamps_limit_to_the_page_maximum() {
    let (_home, _cwd, store) = store("clamp");
    for index in 0..520u32 {
        store
            .append_audit_record(&record(
                &format!("audit_{index:04}"),
                u64::from(index),
                owner("project-1", Some("lane-a")),
                "lane.created",
                vec![object(AuditObjectRef::KIND_LANE, "lane-a")],
            ))
            .unwrap();
    }

    let clamped_high = store.query_audit(&query(u32::MAX)).unwrap();
    assert_eq!(clamped_high.records.len(), 500);
    assert!(!clamped_high.complete);

    // A zero limit is clamped up rather than rejected, so a malformed client
    // request still gets a well-formed page.
    let clamped_low = store.query_audit(&query(0)).unwrap();
    assert_eq!(clamped_low.records.len(), 1);
}

#[test]
fn audit_results_do_not_depend_on_the_derived_sqlite_index() {
    let (_home, _cwd, store) = store("index_independent");
    seed_interleaved(&store);
    store.rebuild_index().unwrap();
    let before = store.query_audit(&query(500)).unwrap();

    fs::remove_file(&store.paths().index_db_path).unwrap();

    let after = store.query_audit(&query(500)).unwrap();
    assert_eq!(before, after);
}

#[test]
fn audit_records_reload_identically_from_a_fresh_store() {
    let home = temp_dir("audit_reload_home");
    let cwd = temp_dir("audit_reload_cwd");
    let first = WorkflowStore::new(&home, &cwd).unwrap();
    seed_interleaved(&first);
    let original = first.query_audit(&query(500)).unwrap();

    let reopened = WorkflowStore::new(&home, &cwd).unwrap();

    assert_eq!(reopened.query_audit(&query(500)).unwrap(), original);
}

#[test]
fn audit_query_fails_hard_on_a_malformed_line() {
    let (_home, _cwd, store) = store("malformed");
    seed_interleaved(&store);
    let mut contents = fs::read_to_string(&store.paths().audit_log).unwrap();
    contents.push_str("{\"audit_id\":\"truncated\"\n");
    fs::write(&store.paths().audit_log, contents).unwrap();

    let error = store.query_audit(&query(10)).unwrap_err();

    assert!(
        error.contains("audit"),
        "a malformed audit line must be a hard error, got: {error}"
    );
}

#[test]
fn audit_constructor_rejects_secret_shaped_argument_values() {
    let error = AuditRecord::sanitized(
        "audit_secret".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "gate.decided".to_string(),
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
        AuditOutcome::Success,
        args(&[("token", "sk-live-0123456789abcdef0123456789abcdef")]),
    )
    .unwrap_err();

    assert!(error.contains("secret"), "unexpected error: {error}");
}

#[test]
fn audit_constructor_rejects_path_traversal_and_control_characters() {
    let traversal = AuditRecord::sanitized(
        "audit_traversal".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "change.reverted".to_string(),
        vec![object(AuditObjectRef::KIND_REVERT, "revert-1")],
        AuditOutcome::Success,
        args(&[("path", "../../etc/passwd")]),
    )
    .unwrap_err();
    assert!(traversal.contains("audit argument"), "got: {traversal}");

    let control = AuditRecord::sanitized(
        "audit_control".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "change.reverted".to_string(),
        vec![object(AuditObjectRef::KIND_REVERT, "revert-1")],
        AuditOutcome::Success,
        args(&[("reason", "line one\nline two")]),
    )
    .unwrap_err();
    assert!(control.contains("audit argument"), "got: {control}");
}

#[test]
fn audit_constructor_rejects_unbounded_arguments_and_invalid_keys() {
    let oversized_value = "a".repeat(513);
    let oversized = AuditRecord::sanitized(
        "audit_oversized".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "gate.decided".to_string(),
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
        AuditOutcome::Success,
        args(&[("note", oversized_value.as_str())]),
    )
    .unwrap_err();
    assert!(oversized.contains("audit argument"), "got: {oversized}");

    let mut too_many = BTreeMap::new();
    for index in 0..33 {
        too_many.insert(format!("key_{index}"), "value".to_string());
    }
    let overflow = AuditRecord::sanitized(
        "audit_overflow".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "gate.decided".to_string(),
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
        AuditOutcome::Success,
        too_many,
    )
    .unwrap_err();
    assert!(overflow.contains("audit arguments"), "got: {overflow}");

    let bad_key = AuditRecord::sanitized(
        "audit_bad_key".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "gate.decided".to_string(),
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
        AuditOutcome::Success,
        args(&[("Gate Type!", "patch")]),
    )
    .unwrap_err();
    assert!(bad_key.contains("audit argument key"), "got: {bad_key}");
}

#[test]
fn audit_constructor_validates_object_kind_charset_but_allows_unknown_kinds() {
    let unknown = AuditRecord::sanitized(
        "audit_unknown_kind".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::Agent {
            agent_id: "agent-1".to_string(),
        },
        "plugin.invoked".to_string(),
        vec![object("plugin_capability", "cap-1")],
        AuditOutcome::Denied,
        BTreeMap::new(),
    );
    assert!(unknown.is_ok(), "unknown kinds stay forward compatible");

    let invalid = AuditRecord::sanitized(
        "audit_bad_kind".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "gate.decided".to_string(),
        vec![object("Merge Gate!", "gate-1")],
        AuditOutcome::Success,
        BTreeMap::new(),
    )
    .unwrap_err();
    assert!(invalid.contains("audit object kind"), "got: {invalid}");
}

#[test]
fn audit_constructor_rejects_localized_prose_action_keys() {
    let error = AuditRecord::sanitized(
        "audit_prose".to_string(),
        10,
        owner("project-1", Some("lane-a")),
        AuditActor::System,
        "The gate was decided by the reviewer".to_string(),
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
        AuditOutcome::Success,
        BTreeMap::new(),
    )
    .unwrap_err();

    assert!(error.contains("audit action"), "got: {error}");
}

#[test]
fn audit_append_rejects_an_unsanitized_record() {
    let (_home, _cwd, store) = store("append_unsanitized");
    let mut unsanitized = record(
        "audit_append",
        10,
        owner("project-1", Some("lane-a")),
        "gate.decided",
        vec![object(AuditObjectRef::KIND_MERGE_GATE, "gate-1")],
    );
    // Bypasses the constructor the same way a hand-built record would.
    unsanitized
        .args
        .insert("reason".to_string(), "reviewer said\u{7} no".to_string());

    let error = store.append_audit_record(&unsanitized).unwrap_err();

    assert!(error.contains("audit argument"), "got: {error}");
    assert!(
        !store.paths().audit_log.exists() || {
            let contents = fs::read_to_string(&store.paths().audit_log).unwrap();
            contents.trim().is_empty()
        }
    );
}
