use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use viden_types::{
    CommandLogEntry, CostScope, CostUsageOutcome, CostUsageRecord, Message, PermissionLogEntry,
    Role, RuntimeEvent, RuntimeEventKind, SessionMetaEntry, TokenUsage, ToolCall, ToolResult,
    TranscriptCursor, TranscriptEntry, TranscriptPageRequest, TranscriptRowKind,
};

fn temp_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("viden_test_{name}_{}", fresh_id("tmp")));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn open_existing_for_query_does_not_create_pristine_home() {
    let base = temp_home("readonly_query_pristine_base");
    let home = base.join("missing-home");
    let cwd = base.join("workspace");
    fs::create_dir_all(&cwd).unwrap();

    let query = SessionStore::open_existing_for_query(&home, &cwd).unwrap();

    assert!(query.is_none());
    assert!(
        !home.exists(),
        "read-only session query must not create the home directory"
    );
}

#[test]
fn open_existing_for_query_preserves_existing_empty_home() {
    let home = temp_home("readonly_query_existing_home");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let sentinel = home.join("sentinel.txt");
    fs::write(&sentinel, "unchanged").unwrap();
    let before = fs::metadata(&sentinel).unwrap();

    let query = SessionStore::open_existing_for_query(&home, &cwd).unwrap();

    assert!(query.is_none());
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "unchanged");
    assert_eq!(fs::metadata(&sentinel).unwrap().len(), before.len());
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&sentinel).unwrap().permissions().mode(),
        before.permissions().mode()
    );
    assert_eq!(
        fs::metadata(&sentinel).unwrap().permissions().readonly(),
        before.permissions().readonly()
    );
    assert!(!home.join("projects").exists());
    assert!(!home.join("index.sqlite3").exists());
}

#[test]
fn jsonl_round_trip_works() {
    let home = temp_home("jsonl");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_roundtrip".into())).unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: Message::new(Role::User, "hello"),
        })
        .unwrap();
    let entries = store.load_entries().unwrap();
    assert_eq!(entries.len(), 1);
}

#[test]
fn replay_discards_trailing_partial_batch() {
    let home = temp_home("partial_batch_legacy");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store =
        SessionStore::new_with_home(&home, &cwd, Some("session_partial_batch".into())).unwrap();
    fs::write(
        store.transcript_path(),
        "{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"batch-a\",\"count\":1}\n{\"type\":\"message\",\"id\":\"msg_partial\",\"role\":\"user\",\"content\":\"discarded\",\"timestamp\":1,\"tool_name\":null,\"tool_call_id\":null}\n",
    )
    .unwrap();

    assert_eq!(store.load_entries().unwrap(), Vec::<TranscriptEntry>::new());
}

#[test]
fn replay_discards_malformed_uncommitted_batch_then_loads_later_committed_batch() {
    let home = temp_home("malformed_uncommitted_then_committed");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store =
        SessionStore::new_with_home(&home, &cwd, Some("session_malformed_uncommitted".into()))
            .unwrap();
    let committed = TranscriptEntry::Message {
        message: Message::new(Role::Assistant, "committed survives"),
    };
    fs::write(
        store.transcript_path(),
        format!(
            "{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"bad\",\"count\":2}}\n{{not-json}}\n{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"bad\",\"count\":2}}\n{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"good\",\"count\":1}}\n{}\n{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"good\",\"count\":1}}\n",
            committed.to_json_line()
        ),
    )
    .unwrap();

    assert_eq!(store.load_entries().unwrap(), vec![committed]);
}

#[test]
fn replay_discards_mismatched_id_and_count_batches() {
    let home = temp_home("mismatched_batch_markers");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_mismatch".into())).unwrap();
    let discarded_id = TranscriptEntry::Message {
        message: Message::new(Role::User, "wrong id"),
    };
    let discarded_count = TranscriptEntry::Message {
        message: Message::new(Role::User, "wrong count"),
    };
    let kept = TranscriptEntry::Message {
        message: Message::new(Role::User, "kept"),
    };
    fs::write(
        store.transcript_path(),
        format!(
            "{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"a\",\"count\":1}}\n{}\n{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"b\",\"count\":1}}\n{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"c\",\"count\":2}}\n{}\n{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"c\",\"count\":2}}\n{}\n",
            discarded_id.to_json_line(),
            discarded_count.to_json_line(),
            kept.to_json_line()
        ),
    )
    .unwrap();

    assert_eq!(store.load_entries().unwrap(), vec![kept]);
}

#[test]
fn replay_discards_nested_batch_without_hiding_later_valid_batch() {
    let home = temp_home("nested_batch");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_nested".into())).unwrap();
    let nested = TranscriptEntry::Message {
        message: Message::new(Role::User, "nested discarded"),
    };
    let kept = TranscriptEntry::Message {
        message: Message::new(Role::Assistant, "later valid"),
    };
    fs::write(
        store.transcript_path(),
        format!(
            "{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"outer\",\"count\":1}}\n{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"inner\",\"count\":1}}\n{}\n{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"outer\",\"count\":1}}\n{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"later\",\"count\":1}}\n{}\n{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"later\",\"count\":1}}\n",
            nested.to_json_line(),
            kept.to_json_line()
        ),
    )
    .unwrap();

    assert_eq!(store.load_entries().unwrap(), vec![kept]);
}

#[test]
fn replay_loads_multiple_committed_batches_written_under_append_lock() {
    let home = temp_home("multiple_committed_batches");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store =
        SessionStore::new_with_home(&home, &cwd, Some("session_multiple_batches".into())).unwrap();
    let first = TranscriptEntry::Message {
        message: Message::new(Role::User, "first batch"),
    };
    let second = TranscriptEntry::Message {
        message: Message::new(Role::Assistant, "second batch"),
    };

    store
        .append_entries_atomic(std::slice::from_ref(&first))
        .unwrap();
    store
        .append_entries_atomic(std::slice::from_ref(&second))
        .unwrap();

    assert_eq!(store.load_entries().unwrap(), vec![first, second]);
}

#[test]
fn replay_keeps_strict_errors_for_malformed_legacy_entries() {
    let home = temp_home("malformed_legacy");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store =
        SessionStore::new_with_home(&home, &cwd, Some("session_malformed_legacy".into())).unwrap();
    fs::write(store.transcript_path(), "{not-json}\n").unwrap();

    let error = store.load_entries().unwrap_err();
    assert!(
        error.contains("Malformed") || error.contains("Missing field"),
        "{error}"
    );
}

#[test]
fn jsonl_round_trip_preserves_cost_usage_entries() {
    let home = temp_home("jsonl_cost_usage");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store =
        SessionStore::new_with_home(&home, &cwd, Some("session_cost_roundtrip".into())).unwrap();
    let cost = CostUsageRecord {
        usage_id: "usage-session-1".to_string(),
        provider_id: "context".to_string(),
        model: "retrieval".to_string(),
        scopes: vec![CostScope::Request("req-session-1".to_string())],
        tokens: TokenUsage {
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            retrieval_tokens: Some(42),
            total_tokens: Some(42),
        },
        estimate: None,
        actual_cost: None,
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(now_timestamp()),
    };
    store
        .append_entry(&TranscriptEntry::CostUsage {
            cost: Box::new(cost.clone()),
        })
        .unwrap();

    let entries = store.load_entries().unwrap();
    assert_eq!(
        entries,
        vec![TranscriptEntry::CostUsage {
            cost: Box::new(cost)
        }]
    );
}

#[test]
fn sqlite_index_is_updated() {
    let home = temp_home("sqlite");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_index".into())).unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: Message::new(Role::User, "hello"),
        })
        .unwrap();
    let sessions = store.list_sessions_for_cwd().unwrap();
    assert!(!sessions.is_empty());
}

#[test]
fn summary_metadata_counts_messages_commands_and_tool_calls() {
    let home = temp_home("summary_meta");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_summary".into())).unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: Message::new(Role::User, "inspect summary"),
        })
        .unwrap();
    store
        .append_entry(&TranscriptEntry::ToolCall {
            call: ToolCall {
                id: "tool_1".into(),
                name: "read_file".into(),
                input: Default::default(),
            },
        })
        .unwrap();
    store
        .append_entry(&TranscriptEntry::Command {
            entry: CommandLogEntry {
                timestamp: now_timestamp(),
                name: "status".into(),
                args: vec![],
                output: "status output".into(),
            },
        })
        .unwrap();
    let summary = store
        .list_sessions_for_cwd()
        .unwrap()
        .into_iter()
        .find(|item| item.session_id == "session_summary")
        .unwrap();
    assert_eq!(summary.message_count, 1);
    assert_eq!(summary.tool_call_count, 1);
    assert_eq!(summary.command_count, 1);
    assert_eq!(summary.last_activity_kind.as_deref(), Some("command"));
    assert_eq!(
        summary.last_activity_preview.as_deref(),
        Some("status output")
    );
}

#[test]
fn sqlite_index_preserves_sessions_with_multiline_command_previews() {
    let home = temp_home("multiline_command_preview");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_multiline".into())).unwrap();
    store
        .append_entry(&TranscriptEntry::Command {
            entry: CommandLogEntry {
                timestamp: now_timestamp(),
                name: "provider".into(),
                args: vec!["deepseek".into(), "deepseek-v4-flash".into()],
                output: "Provider set to deepseek (deepseek-v4-flash)\nSaved default provider/model to /tmp/config.toml\nNext live turn uses deepseek / deepseek-v4-flash.".into(),
            },
        })
        .unwrap();

    let summary = store
        .list_sessions_for_cwd()
        .unwrap()
        .into_iter()
        .find(|item| item.session_id == "session_multiline")
        .unwrap();
    assert_eq!(summary.command_count, 1);
    assert_eq!(summary.last_activity_kind.as_deref(), Some("command"));
    assert_eq!(
        summary.last_activity_preview.as_deref(),
        Some("Provider set to deepseek (deepseek-v4-flash) Saved default provider/model to /tm...")
    );
    assert!(
        store
            .load_by_id_for_cwd("session_multiline")
            .unwrap()
            .is_some()
    );
}

#[test]
fn falls_back_to_project_scan_when_sqlite_index_has_old_schema() {
    let home = temp_home("sqlite_fallback");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_fallback".into())).unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: Message::new(Role::User, "fallback session"),
        })
        .unwrap();

    if sqlite_available() {
        let legacy_sql = "DROP TABLE IF EXISTS sessions;
            CREATE TABLE sessions (
                session_id TEXT PRIMARY KEY,
                cwd TEXT NOT NULL,
                project_key TEXT NOT NULL,
                title TEXT,
                last_preview TEXT,
                transcript_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_updated_at INTEGER NOT NULL
            );";
        run_sql(&home.join("index.sqlite3"), legacy_sql).unwrap();
    }

    let sessions = store.list_sessions_for_cwd().unwrap();
    assert!(
        sessions
            .iter()
            .any(|item| item.session_id == "session_fallback")
    );
}

fn message_with_id(id: &str, role: Role, content: &str, timestamp: u64) -> Message {
    Message {
        id: id.to_string(),
        role,
        content: content.to_string(),
        timestamp,
        tool_name: None,
        tool_call_id: None,
    }
}

#[test]
fn transcript_page_newest_older_newer_limits_and_order_are_stable() {
    let home = temp_home("transcript_page_newest");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_page".into())).unwrap();
    for index in 0..5 {
        store
            .append_entry(&TranscriptEntry::Message {
                message: message_with_id(
                    &format!("msg-{index}"),
                    Role::User,
                    &format!("content {index}"),
                    index + 1,
                ),
            })
            .unwrap();
    }

    let newest = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_page".to_string(),
            before: None,
            limit: 2,
        })
        .unwrap();
    assert_eq!(
        newest
            .rows
            .iter()
            .map(|row| row.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["session_page:3", "session_page:4"]
    );
    assert!(newest.has_more);
    assert_eq!(newest.older.as_ref().unwrap().ordinal, 3);
    assert_eq!(newest.newer, None);

    let previous = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_page".to_string(),
            before: newest.older.clone(),
            limit: 2,
        })
        .unwrap();
    assert_eq!(
        previous
            .rows
            .iter()
            .map(|row| row.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["session_page:1", "session_page:2"]
    );
    assert!(previous.has_more);
    assert_eq!(previous.older.as_ref().unwrap().ordinal, 1);
    assert_eq!(previous.newer.as_ref().unwrap().ordinal, 2);

    let oldest = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_page".to_string(),
            before: previous.older.clone(),
            limit: 0,
        })
        .unwrap();
    assert_eq!(oldest.rows.len(), 1);
    assert_eq!(oldest.rows[0].id.0, "session_page:0");
    assert!(!oldest.has_more);
    assert_eq!(oldest.older, None);
    assert_eq!(oldest.newer.as_ref().unwrap().ordinal, 0);

    let all = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_page".to_string(),
            before: None,
            limit: 900,
        })
        .unwrap();
    assert_eq!(all.rows.len(), 5);
    assert!(!all.has_more);
}

#[test]
fn transcript_page_rejects_wrong_session_cursor_and_loads_exact_other_session() {
    let home = temp_home("transcript_page_other_session");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store_a = SessionStore::new_with_home(&home, &cwd, Some("session_a".into())).unwrap();
    let store_b = SessionStore::new_with_home(&home, &cwd, Some("session_b".into())).unwrap();
    store_a
        .append_entry(&TranscriptEntry::Message {
            message: message_with_id("msg-a", Role::User, "a", 1),
        })
        .unwrap();
    store_b
        .append_entry(&TranscriptEntry::Message {
            message: message_with_id("msg-b", Role::User, "b", 2),
        })
        .unwrap();

    let err = store_a
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_a".to_string(),
            before: Some(TranscriptCursor {
                session_id: "session_b".to_string(),
                ordinal: 0,
            }),
            limit: 25,
        })
        .unwrap_err();
    assert!(err.contains("cursor session"));

    let page = store_a
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_b".to_string(),
            before: None,
            limit: 25,
        })
        .unwrap();
    assert_eq!(page.rows[0].id.0, "session_b:0");
}

#[test]
fn transcript_page_exact_lookup_recovers_project_jsonl_when_sqlite_index_is_stale() {
    let home = temp_home("transcript_page_stale_index");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store_a = SessionStore::new_with_home(&home, &cwd, Some("session_a".into())).unwrap();
    store_a
        .append_entry(&TranscriptEntry::Message {
            message: message_with_id("msg-a", Role::User, "indexed", 1),
        })
        .unwrap();
    if !sqlite_available() {
        return;
    }
    let store_b = SessionStore::new_with_home(&home, &cwd, Some("session_b".into())).unwrap();
    fs::write(
        store_b.transcript_path(),
        TranscriptEntry::Message {
            message: message_with_id("msg-b", Role::User, "jsonl only", 2),
        }
        .to_json_line()
            + "\n",
    )
    .unwrap();

    let indexed = store_a.list_sessions_from_sqlite().unwrap();
    assert!(
        indexed
            .iter()
            .any(|summary| summary.session_id == "session_a")
    );
    assert!(
        !indexed
            .iter()
            .any(|summary| summary.session_id == "session_b")
    );

    let page = store_a
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_b".to_string(),
            before: None,
            limit: 25,
        })
        .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].id.0, "session_b:0");
}

#[test]
fn transcript_page_rebuild_and_reconnect_preserve_ids_order_and_anchors() {
    let home = temp_home("transcript_page_reconnect");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_rebuild".into())).unwrap();
    for index in 0..3 {
        store
            .append_entry(&TranscriptEntry::Message {
                message: message_with_id(
                    &format!("msg-rebuild-{index}"),
                    Role::Assistant,
                    &format!("rebuild {index}"),
                    index + 1,
                ),
            })
            .unwrap();
    }
    let request = TranscriptPageRequest {
        session_id: "session_rebuild".to_string(),
        before: None,
        limit: 2,
    };
    let before = store.load_transcript_page(&request).unwrap();
    store.rebuild_index_for_current().unwrap();
    let reconnected =
        SessionStore::new_with_home(&home, &cwd, Some("session_rebuild".into())).unwrap();
    let after = reconnected.load_transcript_page(&request).unwrap();

    assert_eq!(after, before);
}

#[test]
fn transcript_page_includes_committed_batch_and_keeps_uncommitted_behavior() {
    let home = temp_home("transcript_page_committed_batch");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_batch".into())).unwrap();
    let committed = TranscriptEntry::Message {
        message: message_with_id("msg-committed", Role::Assistant, "committed", 1),
    };
    let discarded = TranscriptEntry::Message {
        message: message_with_id("msg-uncommitted", Role::Assistant, "discarded", 2),
    };
    store
        .append_entries_atomic(std::slice::from_ref(&committed))
        .unwrap();
    assert!(
        store
            .append_entries_uncommitted_for_test(std::slice::from_ref(&discarded), 1)
            .is_err()
    );

    let page = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_batch".to_string(),
            before: None,
            limit: 10,
        })
        .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].id.0, "session_batch:0");
    assert!(matches!(
        &page.rows[0].kind,
        TranscriptRowKind::Message { message } if message.id == "msg-committed"
    ));
}

#[test]
fn transcript_page_coalesces_repeated_assistant_message_id_to_first_ordinal_latest_content() {
    let home = temp_home("transcript_page_coalesce");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_stream".into())).unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: message_with_id("assistant-stream", Role::Assistant, "partial", 10),
        })
        .unwrap();
    store
        .append_entry(&TranscriptEntry::ToolCall {
            call: ToolCall {
                id: "tool-1".to_string(),
                name: "shell".to_string(),
                input: Default::default(),
            },
        })
        .unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: message_with_id("assistant-stream", Role::Assistant, "complete", 11),
        })
        .unwrap();

    let page = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_stream".to_string(),
            before: None,
            limit: 10,
        })
        .unwrap();
    assert_eq!(
        page.rows
            .iter()
            .map(|row| row.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["session_stream:0", "session_stream:1"]
    );
    assert!(matches!(
        &page.rows[0].kind,
        TranscriptRowKind::Message { message }
            if message.content == "complete" && page.rows[0].cursor.ordinal == 0
    ));
}

#[test]
fn transcript_page_converts_every_entry_kind() {
    let home = temp_home("transcript_page_entry_kinds");
    let cwd = home.join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some("session_kinds".into())).unwrap();
    let cost = CostUsageRecord {
        usage_id: "usage-kind".to_string(),
        provider_id: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        scopes: vec![CostScope::Request("request-kind".to_string())],
        tokens: TokenUsage {
            input_tokens: Some(1),
            output_tokens: Some(1),
            cached_input_tokens: Some(0),
            retrieval_tokens: None,
            total_tokens: Some(2),
        },
        estimate: None,
        actual_cost: None,
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(22),
    };
    let entries = vec![
        TranscriptEntry::Message {
            message: message_with_id("msg-kind", Role::User, "msg", 1),
        },
        TranscriptEntry::ToolCall {
            call: ToolCall {
                id: "tool-kind".to_string(),
                name: "shell".to_string(),
                input: Default::default(),
            },
        },
        TranscriptEntry::ToolResult {
            result: ToolResult {
                tool_call_id: "tool-kind".to_string(),
                name: "shell".to_string(),
                output: "ok".to_string(),
                diff: None,
                success: true,
                exit_code: Some(0),
            },
        },
        TranscriptEntry::Permission {
            entry: PermissionLogEntry {
                timestamp: 2,
                tool_name: "shell".to_string(),
                decision: "allow".to_string(),
                reason: "test".to_string(),
                message: None,
            },
        },
        TranscriptEntry::Command {
            entry: CommandLogEntry {
                timestamp: 3,
                name: "status".to_string(),
                args: Vec::new(),
                output: "ok".to_string(),
            },
        },
        TranscriptEntry::SessionMeta {
            entry: SessionMetaEntry {
                timestamp: 4,
                key: "model".to_string(),
                value: "deepseek".to_string(),
            },
        },
        TranscriptEntry::CostUsage {
            cost: Box::new(cost.clone()),
        },
        TranscriptEntry::RuntimeEvent {
            event: Box::new(RuntimeEvent::with_timestamp(
                1,
                Some(5),
                RuntimeEventKind::CostUsageRecorded { cost },
            )),
        },
    ];
    for entry in &entries {
        store.append_entry(entry).unwrap();
    }

    let page = store
        .load_transcript_page(&TranscriptPageRequest {
            session_id: "session_kinds".to_string(),
            before: None,
            limit: 20,
        })
        .unwrap();
    let kind_names = page
        .rows
        .iter()
        .map(|row| match &row.kind {
            TranscriptRowKind::Message { .. } => "message",
            TranscriptRowKind::ToolCall { .. } => "tool_call",
            TranscriptRowKind::ToolResult { .. } => "tool_result",
            TranscriptRowKind::Permission { .. } => "permission",
            TranscriptRowKind::Command { .. } => "command",
            TranscriptRowKind::SessionMeta { .. } => "session_meta",
            TranscriptRowKind::CostUsage { .. } => "cost_usage",
            TranscriptRowKind::RuntimeEvent { .. } => "runtime_event",
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kind_names,
        vec![
            "message",
            "tool_call",
            "tool_result",
            "permission",
            "command",
            "session_meta",
            "cost_usage",
            "runtime_event"
        ]
    );
}
