use super::*;
use crate::{RuntimeSupervisor, SessionEngine};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use viden_config::CliOverrides;
use viden_types::{
    ApprovalResponse, EventCursor, LocaleId, Message, RecentWorkQuery, ReplayRequest, Role,
    RuntimeCommand, RuntimeEventKind, RuntimeWireEvent, SessionMetaEntry, TranscriptEntry,
    UiColorMode, UiDensity, UiMotion, UiPreferencePatch, UiPreferences, UiSkin, WorkMode,
};

fn append_recent_runtime_fixture(home: &Path, root: &Path, session_id: &str, timestamp: u64) {
    let root = root.canonicalize().unwrap();
    let store =
        viden_session::SessionStore::new_with_home(home, &root, Some(session_id.to_string()))
            .unwrap();
    store
        .append_entries_atomic(&[
            TranscriptEntry::SessionMeta {
                entry: SessionMetaEntry {
                    timestamp,
                    key: "canonical_root".to_string(),
                    value: root.display().to_string(),
                },
            },
            TranscriptEntry::SessionMeta {
                entry: SessionMetaEntry {
                    timestamp,
                    key: "session_created_at".to_string(),
                    value: timestamp.to_string(),
                },
            },
            TranscriptEntry::Message {
                message: Message {
                    id: format!("message-{session_id}"),
                    role: Role::User,
                    content: "sk-secret-runtime-body".to_string(),
                    timestamp: timestamp + 1,
                    tool_name: None,
                    tool_call_id: None,
                },
            },
        ])
        .unwrap();
}

#[test]
fn recent_work_command_is_read_only_in_plan_and_emits_exactly_accepted_then_loaded() {
    let cwd = temp_dir("recent_work_runtime_cwd");
    let other = temp_dir("recent_work_runtime_other");
    let home = temp_dir("recent_work_runtime_home");
    append_recent_runtime_fixture(&home, &other, "recent-runtime", 10);
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let mut approver = |_prompt| panic!("read-only recent work must not request approval");

    let events = engine
        .handle_runtime_command(
            "recent-work",
            RuntimeCommand::QueryRecentWork {
                query: RecentWorkQuery { limit: 10 },
            },
            &mut approver,
        )
        .unwrap();

    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0].kind,
        RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "recent-work"
    ));
    assert!(matches!(
        &events[1].kind,
        RuntimeEventKind::RecentWorkLoaded { sessions, .. }
            if sessions.len() == 1 && sessions[0].session_id == "recent-runtime"
    ));
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("sk-secret-runtime-body")
    );
    assert!(
        engine
            .workflow_store()
            .load_agent_events()
            .unwrap()
            .is_empty()
    );
    assert!(
        !fs::read_to_string(engine.store.transcript_path())
            .unwrap()
            .contains("recent_work_loaded")
    );
}

#[test]
fn recent_work_new_transcript_starts_with_canonical_root_and_stable_timestamp_metadata() {
    let cwd = temp_dir("recent_work_initial_metadata_cwd")
        .canonicalize()
        .unwrap();
    let home = temp_dir("recent_work_initial_metadata_home");
    let engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();

    let entries = engine.store.load_entries().unwrap();
    let root = entries.iter().find_map(|entry| match entry {
        TranscriptEntry::SessionMeta { entry } if entry.key == "canonical_root" => {
            Some((entry.timestamp, entry.value.clone()))
        }
        _ => None,
    });
    let created = entries.iter().find_map(|entry| match entry {
        TranscriptEntry::SessionMeta { entry } if entry.key == "session_created_at" => {
            Some((entry.timestamp, entry.value.clone()))
        }
        _ => None,
    });

    assert_eq!(root.as_ref().map(|(_, value)| value.as_str()), cwd.to_str());
    assert_eq!(
        created
            .as_ref()
            .and_then(|(_, value)| value.parse::<u64>().ok()),
        created.as_ref().map(|(timestamp, _)| *timestamp)
    );
    let raw = fs::read_to_string(engine.store.transcript_path()).unwrap();
    assert!(raw.starts_with("{\"type\":\"runtime_event_batch_begin\""));
}

#[test]
fn recent_work_supervisor_snapshot_and_replay_restore_the_last_loaded_result() {
    let cwd = temp_dir("recent_work_supervisor_cwd")
        .canonicalize()
        .unwrap();
    let other = temp_dir("recent_work_supervisor_other")
        .canonicalize()
        .unwrap();
    let home = temp_dir("recent_work_supervisor_home");
    append_recent_runtime_fixture(&home, &other, "other-session", 10);
    let engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "recent-work-supervisor",
            RuntimeCommand::QueryRecentWork {
                query: RecentWorkQuery { limit: 10 },
            },
        )
        .unwrap();

    let events = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::RecentWorkLoaded { .. }))
    });
    let loaded = events
        .iter()
        .find(|event| matches!(event.kind, RuntimeEventKind::RecentWorkLoaded { .. }))
        .unwrap();
    let snapshot = supervisor.snapshot_envelope().unwrap();
    assert_eq!(snapshot.view.recent_sessions.len(), 2);
    assert!(
        snapshot
            .view
            .recent_sessions
            .iter()
            .any(|session| session.session_id == "other-session")
    );

    let replay = supervisor
        .replay_events(ReplayRequest {
            after: EventCursor {
                stream_id: snapshot.cursor.stream_id.clone(),
                sequence: 0,
            },
            limit: 10,
        })
        .unwrap();
    assert!(replay.events.iter().any(|envelope| {
        matches!(
            &envelope.event,
            RuntimeWireEvent::Known(event)
                if event.sequence == loaded.sequence
                    && matches!(event.kind, RuntimeEventKind::RecentWorkLoaded { .. })
        )
    }));
}

fn preference_engine(slug: &str) -> (SessionEngine, PathBuf, PathBuf) {
    let cwd = temp_dir(slug);
    let config_path = cwd.join("user-config.toml");
    let home = cwd.join("session-home");
    let mut engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    engine.set_ui_preference_context(
        None,
        Some(config_path.clone()),
        UiPreferences::client_default(),
    );
    (engine, cwd, config_path)
}

#[test]
fn ui_preferences_command_set_emits_fact_and_reducer_syncs_view_and_snapshot() {
    let (mut engine, _cwd, config_path) = preference_engine("ui_preferences_set");
    let before = engine.runtime_view_state();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "set-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    density: Some(UiDensity::Compact),
                    motion: Some(UiMotion::Reduced),
                },
            },
            &mut approver,
        )
        .unwrap();

    let fact = events
        .iter()
        .find(|event| matches!(event.kind, RuntimeEventKind::UiPreferencesUpdated { .. }))
        .expect("successful preference command emits a fact");
    let mut replayed = before;
    replayed.apply_event(fact);
    assert_eq!(replayed.ui_preferences.locale, LocaleId::ZhCn);
    assert_eq!(replayed.ui_preferences, replayed.snapshot.ui_preferences);
    assert_eq!(
        engine.runtime_snapshot().ui_preferences,
        replayed.ui_preferences
    );
    assert!(fs::read_to_string(config_path).unwrap().contains("zh-CN"));
}

#[test]
fn ui_preferences_command_invalid_profile_is_rejected_before_approval() {
    let (mut engine, cwd, config_path) = preference_engine("ui_preferences_invalid");
    let original = b"[ui]\nskin = \"amber\"\nmode = \"dark\"\n";
    fs::write(&config_path, original).unwrap();
    let approvals = AtomicUsize::new(0);
    let mut approver = |_prompt| {
        approvals.fetch_add(1, Ordering::SeqCst);
        ApprovalResponse::allow_once(None)
    };
    let events = engine
        .handle_runtime_command(
            "invalid-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
            &mut approver,
        )
        .unwrap();

    assert_eq!(approvals.load(Ordering::SeqCst), 0);
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::CommandRejected { reason, .. }
            if reason.contains("ui.invalid_skin_mode_pair")
    )));
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert!(ui_temp_files(&cwd).is_empty());
}

#[test]
fn ui_preferences_command_cli_override_wins_after_persisting_user_profile() {
    let (mut engine, _cwd, config_path) = preference_engine("ui_preferences_cli_wins");
    let cli = UiPreferences {
        locale: LocaleId::En,
        skin: UiSkin::Mono,
        mode: UiColorMode::Dark,
        density: UiDensity::Regular,
        motion: UiMotion::Full,
    };
    engine.set_ui_preference_context(
        Some(cli),
        Some(config_path),
        UiPreferences::client_default(),
    );
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "cli-wins",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    density: Some(UiDensity::Compact),
                    motion: Some(UiMotion::Reduced),
                },
            },
            &mut approver,
        )
        .unwrap();

    let (resolved, persisted) = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::UiPreferencesUpdated {
                resolved,
                persisted,
                ..
            } => Some((resolved, persisted)),
            _ => None,
        })
        .unwrap();
    assert_eq!(resolved.locale, LocaleId::En);
    assert_eq!(resolved.skin, UiSkin::Mono);
    assert_eq!(persisted.as_ref().unwrap().locale, LocaleId::ZhCn);
}

#[test]
fn ui_preferences_command_bootstrap_retains_only_safe_reresolution_context() {
    let cwd = temp_dir("ui_preferences_bootstrap_context");
    let config_path = cwd.join("user-config.toml");
    let cli = UiPreferences {
        locale: LocaleId::En,
        skin: UiSkin::Mono,
        mode: UiColorMode::Dark,
        density: UiDensity::Regular,
        motion: UiMotion::Full,
    };
    let bootstrap = crate::bootstrap_runtime(crate::RuntimeBootstrapRequest::new(
        &cwd,
        CliOverrides {
            provider: Some("fallback".to_string()),
            model: Some("test-local".to_string()),
            api_key: Some("must-not-be-retained-for-ui-reresolution".to_string()),
            session_home: Some(cwd.join("session-home")),
            config_path: Some(config_path),
            ui: Some(cli),
            ..CliOverrides::default()
        },
    ))
    .unwrap();
    let mut engine = bootstrap.engine;
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "bootstrap-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
            &mut approver,
        )
        .unwrap();

    let serialized = serde_json::to_string(&events).unwrap();
    assert!(!serialized.contains("must-not-be-retained"));
    assert!(events.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::UiPreferencesUpdated { resolved, .. }
            if resolved.locale == LocaleId::En && resolved.skin == UiSkin::Mono
    )));
}

#[test]
fn ui_preferences_command_plan_mode_preserves_bytes_mtime_and_temp_state() {
    let (mut engine, cwd, config_path) = preference_engine("ui_preferences_plan");
    let original = b"[ui]\nskin = \"ice\"\nmode = \"dark\"\n";
    fs::write(&config_path, original).unwrap();
    let before_modified = fs::metadata(&config_path).unwrap().modified().unwrap();
    engine.set_work_mode(WorkMode::Plan).unwrap();
    let mut approver = |_prompt| panic!("Plan mode must deny before approval");
    let events = engine
        .handle_runtime_command("plan-ui", RuntimeCommand::ResetUiPreferences, &mut approver)
        .unwrap();

    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    );
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert_eq!(
        fs::metadata(&config_path).unwrap().modified().unwrap(),
        before_modified
    );
    assert!(ui_temp_files(&cwd).is_empty());
}

#[test]
fn ui_preferences_command_reset_removes_ui_and_emits_non_durable_projection_fact() {
    let (mut engine, _cwd, config_path) = preference_engine("ui_preferences_reset");
    fs::write(
        &config_path,
        "custom = 7\n[ui]\nskin = \"ice\"\nmode = \"dark\"\nfuture = \"gone\"\n",
    )
    .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let events = engine
        .handle_runtime_command(
            "reset-ui",
            RuntimeCommand::ResetUiPreferences,
            &mut approver,
        )
        .unwrap();

    let persisted = events.iter().find_map(|event| match &event.kind {
        RuntimeEventKind::UiPreferencesUpdated { persisted, .. } => Some(persisted),
        _ => None,
    });
    assert_eq!(persisted, Some(&None));
    let value = fs::read_to_string(&config_path).unwrap();
    assert!(!value.contains("[ui]"));
    assert!(value.contains("custom = 7"));
    assert!(
        engine
            .workflow_store()
            .load_agent_events()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ui_preferences_command_supervisor_routes_mutation_through_approval() {
    let (engine, _cwd, _config_path) = preference_engine("ui_preferences_supervisor");
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "supervised-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    skin: Some(UiSkin::Ice),
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
        )
        .unwrap();

    let events = collect_until(&supervisor, |events| {
        events.iter().any(|event| {
            matches!(
                event.kind,
                RuntimeEventKind::ApprovalRequested { .. }
                    | RuntimeEventKind::CommandRejected { .. }
            )
        })
    });
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    );
}

#[test]
fn ui_preferences_command_supervisor_success_updates_live_snapshot_and_replay() {
    let (engine, _cwd, config_path) = preference_engine("ui_preferences_supervisor_success");
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "supervised-ui-success",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Mono),
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
        )
        .unwrap();
    let pending = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = pending
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => Some(approval.id.clone()),
            _ => None,
        })
        .unwrap();
    supervisor
        .send_command(
            "approve-ui",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse::allow_once(None),
            },
        )
        .unwrap();

    let success = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::UiPreferencesUpdated { .. }))
    });
    assert!(success.iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::UiPreferencesUpdated { resolved, .. }
            if resolved.locale == LocaleId::ZhCn && resolved.mode == UiColorMode::Light
    )));
    let snapshot = supervisor.snapshot_envelope().unwrap();
    assert_eq!(
        snapshot.view.ui_preferences,
        snapshot.view.snapshot.ui_preferences
    );
    assert_eq!(snapshot.view.ui_preferences.locale, LocaleId::ZhCn);
    assert!(fs::read_to_string(config_path).unwrap().contains("zh-CN"));
}

#[test]
fn ui_preferences_command_supervisor_rejects_invalid_profile_without_approval() {
    let (engine, cwd, config_path) = preference_engine("ui_preferences_supervisor_invalid");
    let original = b"[ui]\nskin = \"amber\"\nmode = \"dark\"\n";
    fs::write(&config_path, original).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "supervised-invalid-ui",
            RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    mode: Some(UiColorMode::Light),
                    ..UiPreferencePatch::default()
                },
            },
        )
        .unwrap();

    let events = collect_until(&supervisor, |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::CommandRejected { .. }))
    });
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert!(ui_temp_files(&cwd).is_empty());
}

fn collect_until(
    supervisor: &RuntimeSupervisor,
    done: impl Fn(&[viden_types::RuntimeEvent]) -> bool,
) -> Vec<viden_types::RuntimeEvent> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < Duration::from_secs(10) {
        if let Some(event) = supervisor.recv_event_timeout(Duration::from_millis(50)) {
            events.push(event);
            if done(&events) {
                return events;
            }
        }
    }
    panic!("timed out waiting for UI preference supervisor events: {events:#?}");
}

fn ui_temp_files(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".ui-") && name.ends_with(".tmp"))
        })
        .collect()
}
