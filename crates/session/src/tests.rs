use super::*;
use viden_types::{
    CommandLogEntry, CostScope, CostUsageOutcome, CostUsageRecord, Message, TokenUsage, ToolCall,
    TranscriptEntry,
};

fn temp_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("viden_test_{name}_{}", fresh_id("tmp")));
    fs::create_dir_all(&dir).unwrap();
    dir
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
