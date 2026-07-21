use super::*;
use std::collections::BTreeSet;

fn runtime_snapshot_json() -> serde_json::Value {
    serde_json::json!({
        "cwd": "/tmp/viden",
        "provider_family": "deepseek",
        "model_label": "deepseek-v4-flash",
        "work_mode": "build",
        "permission_mode": "default",
        "permission_level": "ask",
        "config_summary": "provider=deepseek model=deepseek-v4-flash",
        "loaded_config_files": [],
        "startup_overrides": []
    })
}

#[test]
fn runtime_snapshot_without_ui_preferences_uses_safe_resolved_default() {
    let snapshot: RuntimeSnapshot = serde_json::from_value(runtime_snapshot_json()).unwrap();

    assert_eq!(
        snapshot.ui_preferences,
        ResolvedUiPreferences {
            locale: LocaleId::En,
            skin: UiSkin::Aurora,
            mode: UiColorMode::Dark,
            density: UiDensity::Regular,
            motion: UiMotion::System,
            diagnostics: Vec::new(),
        }
    );
}

#[test]
fn runtime_snapshot_serializes_exact_resolved_ui_preferences() {
    let mut encoded = runtime_snapshot_json();
    encoded["ui_preferences"] = serde_json::json!({
        "locale": "zh-CN",
        "skin": "aurora",
        "mode": "dark",
        "density": "regular",
        "motion": "reduced",
        "diagnostics": []
    });

    let snapshot: RuntimeSnapshot = serde_json::from_value(encoded).unwrap();
    let serialized = serde_json::to_value(snapshot).unwrap();

    assert_eq!(
        serialized["ui_preferences"],
        serde_json::json!({
            "locale": "zh-CN",
            "skin": "aurora",
            "mode": "dark",
            "density": "regular",
            "motion": "reduced",
            "diagnostics": []
        })
    );
}

#[test]
fn ui_preferences_serde_names_are_stable() {
    let cases = [
        (serde_json::to_value(LocaleId::System).unwrap(), "system"),
        (serde_json::to_value(LocaleId::En).unwrap(), "en"),
        (serde_json::to_value(LocaleId::ZhCn).unwrap(), "zh-CN"),
        (serde_json::to_value(UiSkin::Aurora).unwrap(), "aurora"),
        (serde_json::to_value(UiSkin::Ice).unwrap(), "ice"),
        (serde_json::to_value(UiSkin::Mono).unwrap(), "mono"),
        (serde_json::to_value(UiSkin::Amber).unwrap(), "amber"),
        (serde_json::to_value(UiSkin::Phosphor).unwrap(), "phosphor"),
        (serde_json::to_value(UiColorMode::System).unwrap(), "system"),
        (serde_json::to_value(UiColorMode::Dark).unwrap(), "dark"),
        (serde_json::to_value(UiColorMode::Light).unwrap(), "light"),
        (serde_json::to_value(UiDensity::Compact).unwrap(), "compact"),
        (serde_json::to_value(UiDensity::Regular).unwrap(), "regular"),
        (serde_json::to_value(UiDensity::Comfy).unwrap(), "comfy"),
        (serde_json::to_value(UiMotion::System).unwrap(), "system"),
        (serde_json::to_value(UiMotion::Reduced).unwrap(), "reduced"),
        (serde_json::to_value(UiMotion::Full).unwrap(), "full"),
        (
            serde_json::to_value(TuiColorDepth::Truecolor).unwrap(),
            "truecolor",
        ),
        (
            serde_json::to_value(TuiColorDepth::Ansi256).unwrap(),
            "ansi256",
        ),
        (
            serde_json::to_value(TuiColorDepth::Ansi16).unwrap(),
            "ansi16",
        ),
    ];

    for (encoded, expected) in cases {
        assert_eq!(encoded, serde_json::Value::String(expected.to_string()));
    }
}

#[test]
fn ui_preferences_valid_skin_mode_pairs_are_exactly_eight() {
    let pairs: Vec<_> = UiSkin::ALL
        .iter()
        .copied()
        .flat_map(|skin| {
            [UiColorMode::Dark, UiColorMode::Light]
                .into_iter()
                .map(move |mode| (skin, mode))
        })
        .filter(|(skin, mode)| UiPreferences::is_valid_effective_pair(*skin, *mode))
        .collect();

    assert_eq!(pairs.len(), 8);
    assert!(pairs.contains(&(UiSkin::Aurora, UiColorMode::Dark)));
    assert!(pairs.contains(&(UiSkin::Aurora, UiColorMode::Light)));
    assert!(pairs.contains(&(UiSkin::Ice, UiColorMode::Dark)));
    assert!(pairs.contains(&(UiSkin::Ice, UiColorMode::Light)));
    assert!(pairs.contains(&(UiSkin::Mono, UiColorMode::Dark)));
    assert!(pairs.contains(&(UiSkin::Mono, UiColorMode::Light)));
    assert!(pairs.contains(&(UiSkin::Amber, UiColorMode::Dark)));
    assert!(pairs.contains(&(UiSkin::Phosphor, UiColorMode::Dark)));
    assert!(!pairs.contains(&(UiSkin::Amber, UiColorMode::Light)));
    assert!(!pairs.contains(&(UiSkin::Phosphor, UiColorMode::Light)));
}

#[test]
fn ui_preferences_patch_serializes_as_schema_one_safe_values() {
    let patch = UiPreferencePatch {
        locale: Some(LocaleId::ZhCn),
        skin: Some(UiSkin::Ice),
        mode: Some(UiColorMode::Light),
        density: Some(UiDensity::Compact),
        motion: Some(UiMotion::Reduced),
    };

    assert_eq!(
        serde_json::to_value(patch).unwrap(),
        serde_json::json!({
            "locale": "zh-CN",
            "skin": "ice",
            "mode": "light",
            "density": "compact",
            "motion": "reduced"
        })
    );
    assert_eq!(
        serde_json::from_value::<UiPreferencePatch>(serde_json::json!({})).unwrap(),
        UiPreferencePatch::default()
    );
}

#[test]
fn ui_preferences_runtime_protocol_is_backward_compatible_schema_one_extension() {
    let command = RuntimeCommand::SetUiPreferences {
        patch: UiPreferencePatch {
            skin: Some(UiSkin::Mono),
            mode: Some(UiColorMode::Light),
            ..UiPreferencePatch::default()
        },
    };
    let encoded_command = serde_json::to_value(&command).unwrap();
    assert_eq!(encoded_command["type"], "set_ui_preferences");
    assert_eq!(encoded_command["patch"]["skin"], "mono");
    assert!(!encoded_command.to_string().contains("api_key"));

    let resolved = ResolvedUiPreferences {
        locale: LocaleId::En,
        skin: UiSkin::Mono,
        mode: UiColorMode::Light,
        density: UiDensity::Regular,
        motion: UiMotion::Reduced,
        diagnostics: Vec::new(),
    };
    let event = RuntimeEventKind::UiPreferencesUpdated {
        resolved: resolved.clone(),
        persisted: Some(UiPreferences {
            locale: LocaleId::En,
            skin: UiSkin::Mono,
            mode: UiColorMode::Light,
            density: UiDensity::Regular,
            motion: UiMotion::Reduced,
        }),
        diagnostics: Vec::new(),
    };
    let encoded_event = serde_json::to_value(&event).unwrap();
    assert_eq!(encoded_event["type"], "ui_preferences_updated");
    assert_eq!(
        serde_json::from_value::<RuntimeEventKind>(encoded_event).unwrap(),
        event
    );

    let snapshot = runtime_snapshot_for_contract();
    let live_view = RuntimeViewState::new(snapshot);
    assert_eq!(live_view.ui_preferences, live_view.snapshot.ui_preferences);
    let legacy_view = serde_json::to_value(live_view).unwrap();
    assert!(legacy_view.get("ui_preferences").is_none());
    let decoded: RuntimeViewState = serde_json::from_value(legacy_view).unwrap();
    assert_eq!(decoded.ui_preferences, ResolvedUiPreferences::default());
}

#[test]
fn recent_work_runtime_protocol_round_trips_safe_schema_one_payloads() {
    let command = RuntimeCommand::QueryRecentWork {
        query: RecentWorkQuery { limit: 501 },
    };
    let encoded_command = serde_json::to_value(&command).unwrap();
    assert_eq!(encoded_command["type"], "query_recent_work");
    assert_eq!(encoded_command["query"]["limit"], 501);
    assert_eq!(
        serde_json::from_value::<RuntimeCommand>(encoded_command).unwrap(),
        command
    );

    let sessions = vec![RecentSessionSummary {
        canonical_root: "/workspace/a".to_string(),
        session_id: "session-a".to_string(),
        created_at: 10,
        last_updated_at: 20,
        message_count: 2,
        tool_call_count: 1,
        command_count: 1,
    }];
    let projects = vec![RecentProjectSummary {
        canonical_root: "/workspace/a".to_string(),
        display_name: "a".to_string(),
        last_updated_at: 20,
        latest_session_id: Some("session-a".to_string()),
    }];
    let event = RuntimeEventKind::RecentWorkLoaded {
        projects: projects.clone(),
        sessions: sessions.clone(),
        diagnostics: vec!["recent.index_stale".to_string()],
    };
    let encoded_event = serde_json::to_value(&event).unwrap();
    assert_eq!(encoded_event["type"], "recent_work_loaded");
    assert_eq!(
        serde_json::from_value::<RuntimeEventKind>(encoded_event).unwrap(),
        event
    );

    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());
    view.apply_event(&RuntimeEvent::with_timestamp(1, Some(30), event));
    assert_eq!(view.recent_projects, projects);
    assert_eq!(view.recent_sessions, sessions);
    assert_eq!(view.recent_work_diagnostics, vec!["recent.index_stale"]);
}

#[test]
fn recent_work_serialized_event_and_view_exclude_private_session_fields() {
    let event = RuntimeEvent::with_timestamp(
        1,
        Some(30),
        RuntimeEventKind::RecentWorkLoaded {
            projects: vec![RecentProjectSummary {
                canonical_root: "/workspace/public".to_string(),
                display_name: "public".to_string(),
                last_updated_at: 20,
                latest_session_id: Some("session-public".to_string()),
            }],
            sessions: vec![RecentSessionSummary {
                canonical_root: "/workspace/public".to_string(),
                session_id: "session-public".to_string(),
                created_at: 10,
                last_updated_at: 20,
                message_count: 1,
                tool_call_count: 0,
                command_count: 0,
            }],
            diagnostics: Vec::new(),
        },
    );
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());
    view.apply_event(&event);
    let serialized = format!(
        "{}{}",
        serde_json::to_string(&event).unwrap(),
        serde_json::to_string(&view).unwrap()
    );

    for forbidden in [
        "transcript_path",
        "last_preview",
        "last_activity_preview",
        "credential_request_id",
        "backend_id",
        "sk-secret-message-body",
        "command output",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn ui_preferences_resolve_system_locale_density_and_reduced_motion() {
    let resolved = resolve_ui_preferences(
        None,
        None,
        Some(UiPreferences {
            locale: LocaleId::System,
            skin: UiSkin::Ice,
            mode: UiColorMode::System,
            density: UiDensity::Comfy,
            motion: UiMotion::Reduced,
        }),
        UiPreferences {
            locale: LocaleId::ZhCn,
            skin: UiSkin::Aurora,
            mode: UiColorMode::Light,
            density: UiDensity::Regular,
            motion: UiMotion::System,
        },
    );

    assert_eq!(resolved.locale, LocaleId::ZhCn);
    assert_eq!(resolved.skin, UiSkin::Ice);
    assert_eq!(resolved.mode, UiColorMode::Light);
    assert_eq!(resolved.density, UiDensity::Comfy);
    assert_eq!(resolved.motion, UiMotion::Reduced);
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn ui_preferences_invalid_dark_only_light_pair_falls_back_once() {
    let resolved = resolve_ui_preferences(
        Some(UiPreferences {
            locale: LocaleId::ZhCn,
            skin: UiSkin::Amber,
            mode: UiColorMode::Light,
            density: UiDensity::Compact,
            motion: UiMotion::Reduced,
        }),
        None,
        None,
        UiPreferences::client_default(),
    );

    assert_eq!(resolved.locale, LocaleId::ZhCn);
    assert_eq!(resolved.skin, UiSkin::Aurora);
    assert_eq!(resolved.mode, UiColorMode::Dark);
    assert_eq!(resolved.density, UiDensity::Regular);
    assert_eq!(resolved.motion, UiMotion::Reduced);
    assert_eq!(resolved.diagnostics.len(), 1);
    assert_eq!(resolved.diagnostics[0].code, "ui.invalid_skin_mode_pair");
    assert_eq!(resolved.diagnostics[0].field.as_deref(), Some("ui.mode"));
}

#[test]
fn ui_preferences_project_cannot_override_user_profile() {
    let resolved = resolve_ui_preferences(
        None,
        Some(UiPreferences {
            locale: LocaleId::En,
            skin: UiSkin::Mono,
            mode: UiColorMode::Dark,
            density: UiDensity::Compact,
            motion: UiMotion::Full,
        }),
        Some(UiPreferences {
            locale: LocaleId::ZhCn,
            skin: UiSkin::Ice,
            mode: UiColorMode::Light,
            density: UiDensity::Comfy,
            motion: UiMotion::Reduced,
        }),
        UiPreferences::client_default(),
    );

    assert_eq!(resolved.locale, LocaleId::En);
    assert_eq!(resolved.skin, UiSkin::Mono);
    assert_eq!(resolved.mode, UiColorMode::Dark);
    assert_eq!(resolved.density, UiDensity::Compact);
    assert_eq!(resolved.motion, UiMotion::Full);
}

#[test]
fn ui_preferences_chinese_locale_identifiers_map_to_builtin_zh_cn() {
    for raw in [
        "zh",
        "zh_CN",
        "zh-CN",
        "zh_CN.UTF-8",
        "zh-CN.UTF-8",
        "zh_TW",
        "zh-HK",
        "zh_Hant_TW.UTF-8",
        "zh.Hans",
    ] {
        assert_eq!(LocaleId::from_system_locale(raw), LocaleId::ZhCn, "{raw}");
    }
}

#[test]
fn ui_preferences_locale_detection_does_not_match_arbitrary_words() {
    for raw in ["zhuang", "zhfake", "english_zh", "en_US.UTF-8", ""] {
        assert_ne!(LocaleId::from_system_locale(raw), LocaleId::ZhCn, "{raw}");
    }
}

#[test]
fn work_modes_and_permission_levels_parse_cli_names() {
    assert_eq!(WorkMode::parse_cli("plan"), Some(WorkMode::Plan));
    assert_eq!(WorkMode::parse_cli("build"), Some(WorkMode::Build));
    assert_eq!(WorkMode::Plan.cli_name(), "plan");
    assert_eq!(WorkMode::default(), WorkMode::Build);

    assert_eq!(
        PermissionLevel::parse_cli("ask"),
        Some(PermissionLevel::Ask)
    );
    assert_eq!(
        PermissionLevel::parse_cli("auto_edit"),
        Some(PermissionLevel::AutoEdit)
    );
    assert_eq!(
        PermissionLevel::parse_cli("read-only"),
        Some(PermissionLevel::ReadOnly)
    );
    assert_eq!(
        PermissionLevel::from_legacy_mode(PermissionMode::Plan),
        PermissionLevel::ReadOnly
    );
    assert_eq!(
        PermissionLevel::from_legacy_mode(PermissionMode::AcceptEdits),
        PermissionLevel::AutoEdit
    );
}

#[test]
fn workflow_enums_roundtrip_through_cli_names_and_json() {
    assert_eq!(
        TaskStatus::parse_cli("in_progress"),
        Some(TaskStatus::InProgress)
    );
    assert_eq!(TaskStatus::Blocked.cli_name(), "blocked");
    assert_eq!(
        TaskPriority::parse_cli("critical"),
        Some(TaskPriority::Critical)
    );
    assert_eq!(
        MemoryScope::parse_cli("session"),
        Some(MemoryScope::Session)
    );
    assert_eq!(MemoryKind::Decision.cli_name(), "decision");
    assert_eq!(
        MemorySource::parse_cli("assistant_suggestion"),
        Some(MemorySource::AssistantSuggestion)
    );
    assert_eq!(MemoryStatus::Active.cli_name(), "active");

    let encoded = serde_json::to_string(&TaskStatus::InProgress).unwrap();
    assert_eq!(encoded, "\"in_progress\"");
    let decoded: TaskStatus = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, TaskStatus::InProgress);
}

#[test]
fn workflow_records_are_serializable() {
    let task = TaskRecord {
        task_id: "task_1".to_string(),
        title: "Design workflow state".to_string(),
        description: Some("Capture durable task state".to_string()),
        status: TaskStatus::Todo,
        priority: TaskPriority::High,
        labels: vec!["v2".to_string(), "workflow".to_string()],
        assignee_hint: Some("agent".to_string()),
        parent_task_id: None,
        dependency_ids: vec!["task_0".to_string()],
        blocked_by: Some("waiting on spec review".to_string()),
        notes: vec!["Use append-only logs".to_string()],
        created_at: 10,
        updated_at: 11,
        last_session_id: Some("session_1".to_string()),
        last_seen_at: Some(12),
        archived_at: None,
    };

    let memory = MemoryEntry {
        memory_id: "mem_1".to_string(),
        scope: MemoryScope::Project,
        session_id: None,
        kind: MemoryKind::Convention,
        content: "Use JSONL as canonical workflow storage".to_string(),
        source: MemorySource::User,
        status: MemoryStatus::Active,
        created_at: 20,
        updated_at: 21,
        related_task_ids: vec![task.task_id.clone()],
        confidence_hint: Some("high".to_string()),
    };

    let snapshot = ResumeContextSnapshot {
        active_tasks: vec![task],
        blocked_tasks: Vec::new(),
        recently_completed_tasks: Vec::new(),
        relevant_project_memory: vec![memory],
        recent_session_memory: Vec::new(),
        suggested_next_steps: vec!["Continue Task 1".to_string()],
        suggested_session_memory: vec!["Task 1 started".to_string()],
    };

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: ResumeContextSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.suggested_next_steps, vec!["Continue Task 1"]);
    assert_eq!(decoded.active_tasks[0].priority, TaskPriority::High);
}

#[test]
fn agent_task_status_maps_operator_priority() {
    assert_eq!(
        AgentTaskStatus::parse("reviewing"),
        Some(AgentTaskStatus::Reviewing)
    );
    assert_eq!(
        AgentTaskStatus::parse("apply_conflict"),
        Some(AgentTaskStatus::Blocked)
    );
    assert!(AgentTaskStatus::WaitingApproval.is_active());
    assert!(AgentTaskStatus::WaitingApproval.priority() > AgentTaskStatus::RunningTool.priority());
    assert!(!AgentTaskStatus::Done.is_active());
}

#[test]
fn agent_task_and_context_records_roundtrip_json() {
    let task = AgentTaskRecord {
        id: "lane-1".to_string(),
        parent_id: None,
        role: AgentRole::Coder,
        kind: AgentTaskKind::Agent,
        route: AgentRoute::Terminal,
        title: "cargo test".to_string(),
        status: AgentTaskStatus::Running,
        activity: "running cargo test".to_string(),
        summary: "operator lane".to_string(),
        progress: 42,
        started_at: Some(1),
        updated_at: Some(2),
        workspace: Some("/tmp/work".to_string()),
        evidence: vec!["log lane-1.log".to_string()],
        permissions: vec!["shell approval".to_string()],
        decision: None,
        result: None,
        resume_handle: Some("tmux attach -t rc-lane-1".to_string()),
        pid: Some(1234),
        next_action: Some(AgentNextAction {
            label: "inspect".to_string(),
            command: Some("/lane inspect lane-1".to_string()),
            reason: Some("running lane".to_string()),
        }),
    };
    assert!(task.is_active());
    assert_eq!(task.priority(), AgentTaskStatus::Running.priority());

    let bundle = ContextBundleRecord {
        bundle_id: "ctx-lane-1".to_string(),
        task_id: task.id.clone(),
        policy: "v1-priority-budget".to_string(),
        sources: vec![ContextSourceRecord {
            name: "latest-test".to_string(),
            kind: "test".to_string(),
            priority: 85,
            estimated_tokens: 200,
            summary: "tail compacted".to_string(),
            include_reason: "priority 85; selected by v1-priority-budget policy".to_string(),
            handle_id: Some("ctxh-test".to_string()),
            item_id: Some("ctxi-test".to_string()),
            view_id: Some("ctxv-test".to_string()),
            content_sha256: Some("ab".repeat(32)),
            view_sha256: Some("cd".repeat(32)),
            quality_id: Some("ctxq-test".to_string()),
        }],
        omitted_sources: vec![],
        estimated_tokens: 200,
        largest_sources: vec!["latest-test 200 tok".to_string()],
        compaction_notes: vec!["raw transcript preserved".to_string()],
        soft_token_budget: 800,
        hard_token_limit: 1000,
    };
    assert_eq!(bundle.pressure_percent(), 20);

    let encoded = serde_json::to_string(&(task, bundle)).unwrap();
    let (decoded_task, decoded_bundle): (AgentTaskRecord, ContextBundleRecord) =
        serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded_task.role, AgentRole::Coder);
    assert_eq!(decoded_bundle.sources[0].kind, "test");
    assert_eq!(
        decoded_bundle.sources[0].handle_id.as_deref(),
        Some("ctxh-test")
    );
    let legacy_json = serde_json::json!({
        "name": "legacy",
        "kind": "text",
        "priority": 1,
        "estimated_tokens": 2,
        "summary": "legacy summary",
        "include_reason": "legacy reason"
    });
    let legacy_source: ContextSourceRecord = serde_json::from_value(legacy_json).unwrap();
    assert!(legacy_source.handle_id.is_none());
    assert!(legacy_source.view_id.is_none());
}

#[test]
fn agent_lane_trust_loop_records_roundtrip_json() {
    let event = AgentLaneEventRecord {
        lane_id: "L1".to_string(),
        sequence: 1,
        timestamp: Some(10),
        kind: "lane.started".to_string(),
        summary: "started shell lane".to_string(),
        detail: Some("printf ok".to_string()),
        evidence_path: Some(".viden/lanes/L1.log".to_string()),
    };
    let isolation = AgentLaneIsolationRecord {
        lane_id: "L1".to_string(),
        workspace: "/tmp/project".to_string(),
        worktree: None,
        writable_scope: "current workspace".to_string(),
        env_vars: vec!["PATH".to_string()],
        cache_dirs: vec!["target/".to_string()],
        database_scope: None,
        service_ports: Vec::new(),
        setup_command: None,
        verification_command: Some("cargo test".to_string()),
        cleanup_command: None,
        risk_level: "medium".to_string(),
        warnings: vec!["shared workspace".to_string()],
    };
    let capability = AgentCapabilityRecord {
        id: "shell".to_string(),
        display_name: "Shell lane".to_string(),
        transport: "shell".to_string(),
        readiness: "ready".to_string(),
        entrypoint: "/lane run <command>".to_string(),
        mutation_mode: "permission-gated".to_string(),
        evidence_mode: "log+done+timeline".to_string(),
        config_source: None,
        known_limits: vec!["no process sandbox".to_string()],
    };

    let encoded = serde_json::to_string(&(event, isolation, capability)).unwrap();
    let (decoded_event, decoded_isolation, decoded_capability): (
        AgentLaneEventRecord,
        AgentLaneIsolationRecord,
        AgentCapabilityRecord,
    ) = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded_event.kind, "lane.started");
    assert_eq!(decoded_isolation.risk_level, "medium");
    assert_eq!(decoded_capability.evidence_mode, "log+done+timeline");
}

#[test]
fn typed_lane_records_use_the_frozen_v1_wire_names() {
    let lane = AgentLaneRecord {
        id: "lane_research".to_string(),
        task_id: Some("task_research".to_string()),
        role: AgentRole::Researcher,
        route: AgentRoute::Acp,
        gate_strength: GateStrength::Cooperative,
        mutation_policy: MutationPolicy::ReadOnly,
        worktree: Some(".worktrees/research".to_string()),
        branch: Some("codex/research".to_string()),
        target: ExecutionTarget::Ssh {
            host: "build.example.test".to_string(),
        },
        data_egress: DataEgressPolicy::AllowListed {
            domains: vec!["docs.example.test".to_string()],
        },
        status: LaneStatus::WaitingApproval,
        budget: LaneBudget {
            token_limit: Some(4_096),
            cost_limit_micro_usd: Some(250_000),
            wall_time_limit_secs: Some(300),
        },
        active_session_ids: vec!["session_research".to_string()],
        summary: "research is waiting for approval".to_string(),
        evidence: vec!["evidence_research".to_string()],
    };

    let encoded = serde_json::to_value(&lane).unwrap();
    assert_eq!(encoded["role"], "researcher");
    assert_eq!(encoded["route"], "acp");
    assert_eq!(encoded["gate_strength"], "cooperative");
    assert_eq!(encoded["mutation_policy"], "read_only");
    assert_eq!(encoded["status"], "waiting_approval");
    assert_eq!(
        encoded["target"],
        serde_json::json!({"ssh": {"host": "build.example.test"}})
    );
    assert_eq!(
        encoded["data_egress"],
        serde_json::json!({"allow_listed": {"domains": ["docs.example.test"]}})
    );
    assert_eq!(
        serde_json::from_value::<AgentLaneRecord>(encoded).unwrap(),
        lane
    );
}

#[test]
fn typed_lane_enums_use_explicit_v1_json_names() {
    let roles = [
        (AgentRole::Planner, "planner"),
        (AgentRole::Coder, "coder"),
        (AgentRole::Reviewer, "reviewer"),
        (AgentRole::Tester, "tester"),
        (AgentRole::DocWriter, "doc_writer"),
        (AgentRole::Researcher, "researcher"),
        (AgentRole::ReleaseOperator, "release_operator"),
    ];
    for (value, wire) in roles {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }
    assert!(serde_json::from_str::<AgentRole>("\"external\"").is_err());

    let routes = [
        (AgentRoute::BuiltIn, "built_in"),
        (AgentRoute::Acp, "acp"),
        (AgentRoute::Terminal, "terminal"),
        (AgentRoute::Tmux, "tmux"),
    ];
    for (value, wire) in routes {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }

    let gates = [
        (GateStrength::Full, "full"),
        (GateStrength::Cooperative, "cooperative"),
        (GateStrength::Containment, "containment"),
    ];
    for (value, wire) in gates {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }

    let mutation_policies = [
        (MutationPolicy::Autonomous, "autonomous"),
        (MutationPolicy::ProposeOnly, "propose_only"),
        (MutationPolicy::ReadOnly, "read_only"),
    ];
    for (value, wire) in mutation_policies {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }

    let statuses = [
        (LaneStatus::Draft, "draft"),
        (LaneStatus::Queued, "queued"),
        (LaneStatus::Starting, "starting"),
        (LaneStatus::Running, "running"),
        (LaneStatus::WaitingApproval, "waiting_approval"),
        (LaneStatus::NeedsInput, "needs_input"),
        (LaneStatus::Blocked, "blocked"),
        (LaneStatus::Attached, "attached"),
        (LaneStatus::Detached, "detached"),
        (LaneStatus::Done, "done"),
        (LaneStatus::Failed, "failed"),
        (LaneStatus::Cancelled, "cancelled"),
        (LaneStatus::Archived, "archived"),
    ];
    for (value, wire) in statuses {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }

    let task_kinds = [
        (AgentTaskKind::Provider, "provider"),
        (AgentTaskKind::Tool, "tool"),
        (AgentTaskKind::Shell, "shell"),
        (AgentTaskKind::Test, "test"),
        (AgentTaskKind::Job, "job"),
        (AgentTaskKind::Agent, "agent"),
    ];
    for (value, wire) in task_kinds {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }
    assert_eq!(
        serde_json::to_value(ExecutionTarget::Local).unwrap(),
        "local"
    );
    assert_eq!(
        serde_json::to_value(DataEgressPolicy::AllowProvider).unwrap(),
        "allow_provider"
    );
}

#[test]
fn typed_lane_fixture_replays_as_the_frozen_v1_record() {
    let lanes: Vec<AgentLaneRecord> = serde_json::from_str(include_str!(
        "../tests/fixtures/frontend-contract-v1/typed-lanes.json"
    ))
    .unwrap();
    assert_eq!(lanes.len(), 4);
    assert_eq!(lanes[0].id, "L-start");
    assert_eq!(lanes[0].role, AgentRole::Coder);
    assert_eq!(lanes[0].route, AgentRoute::Terminal);
    assert_eq!(lanes[0].status, LaneStatus::Starting);
    assert_eq!(lanes[1].status, LaneStatus::Blocked);
    assert_eq!(lanes[2].route, AgentRoute::Tmux);
    assert_eq!(lanes[2].status, LaneStatus::Detached);
    assert_eq!(lanes[3].status, LaneStatus::Detached);
}

#[test]
fn legacy_lane_json_migrates_only_at_the_record_input_edge() {
    let legacy = serde_json::json!({
        "id": "lane_legacy",
        "task_id": "task_legacy",
        "agent": "codex",
        "screen": "lane",
        "transport": "tmux",
        "status": "stopped",
        "summary": "legacy lane stopped by operator",
        "evidence": ["log lane_legacy.log"]
    });
    let lane: AgentLaneRecord = serde_json::from_value(legacy).unwrap();
    assert_eq!(lane.role, AgentRole::Coder);
    assert_eq!(lane.route, AgentRoute::Tmux);
    assert_eq!(lane.status, LaneStatus::Detached);
    assert_eq!(lane.mutation_policy, MutationPolicy::ProposeOnly);
    assert_eq!(lane.target, ExecutionTarget::Local);
}

#[test]
fn legacy_lane_route_accepts_terminal_alias() {
    assert_eq!(legacy_lane_route("terminal").unwrap(), AgentRoute::Terminal);
}

#[test]
fn typed_task_records_preserve_v0_names_and_migrate_legacy_values() {
    let legacy = serde_json::json!({
        "id": "task_test",
        "parent_id": null,
        "agent": "shell",
        "kind": "test",
        "transport": "shell",
        "title": "cargo test",
        "status": "starting",
        "activity": "starting test",
        "summary": "test task",
        "progress": 0,
        "started_at": null,
        "updated_at": null,
        "workspace": null,
        "evidence": [],
        "permissions": [],
        "decision": null,
        "result": null,
        "resume_handle": null,
        "pid": null,
        "next_action": null
    });
    let task: AgentTaskRecord = serde_json::from_value(legacy).unwrap();
    assert_eq!(task.role, AgentRole::Tester);
    assert_eq!(task.kind, AgentTaskKind::Test);
    assert_eq!(task.route, AgentRoute::Terminal);
    assert_eq!(task.status, AgentTaskStatus::Thinking);

    let encoded = serde_json::to_value(&task).unwrap();
    assert_eq!(encoded["agent"], "tester");
    assert_eq!(encoded["transport"], "terminal");
    assert!(encoded.get("role").is_none());
    assert!(encoded.get("route").is_none());

    let mut external = encoded;
    external["agent"] = serde_json::json!("external");
    assert!(serde_json::from_value::<AgentTaskRecord>(external).is_err());
}

#[test]
fn legacy_task_transport_values_migrate_by_task_kind_not_key_renaming() {
    let cases = [
        (
            "viden",
            "provider",
            "deepseek",
            AgentTaskKind::Provider,
            AgentRoute::BuiltIn,
        ),
        (
            "viden",
            "provider",
            "anthropic",
            AgentTaskKind::Provider,
            AgentRoute::BuiltIn,
        ),
        (
            "shell",
            "shell",
            "terminal",
            AgentTaskKind::Shell,
            AgentRoute::Terminal,
        ),
        (
            "viden",
            "tool",
            "local",
            AgentTaskKind::Tool,
            AgentRoute::Terminal,
        ),
        (
            "acp",
            "job",
            "acp-session",
            AgentTaskKind::Job,
            AgentRoute::Acp,
        ),
    ];

    for (agent, kind, transport, expected_kind, expected_route) in cases {
        let task: AgentTaskRecord =
            serde_json::from_value(legacy_task_json(agent, kind, transport)).unwrap();
        assert_eq!(task.kind, expected_kind);
        assert_eq!(task.route, expected_route);

        let encoded = serde_json::to_value(task).unwrap();
        // v0 names remain, but their values are the v1 typed classifications.
        assert!(encoded.get("role").is_none());
        assert!(encoded.get("route").is_none());
        assert_eq!(encoded["agent"], "coder");
        assert_eq!(
            encoded["transport"],
            serde_json::to_value(expected_route).unwrap()
        );
    }

    assert!(
        serde_json::from_value::<AgentTaskRecord>(legacy_task_json(
            "viden",
            "tool",
            "unrecognized-transport",
        ))
        .is_err()
    );
}

fn legacy_task_json(agent: &str, kind: &str, transport: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "task_legacy",
        "parent_id": null,
        "agent": agent,
        "kind": kind,
        "transport": transport,
        "title": "legacy task",
        "status": "queued",
        "activity": "queued",
        "summary": "legacy task",
        "progress": 0,
        "started_at": null,
        "updated_at": null,
        "workspace": null,
        "evidence": [],
        "permissions": [],
        "decision": null,
        "result": null,
        "resume_handle": null,
        "pid": null,
        "next_action": null
    })
}

#[test]
fn agent_dag_context_evidence_and_merge_gate_roundtrip_json() {
    let task = AgentDagTaskSpec {
        task_id: "agent_task_1".to_string(),
        role: AgentRole::Planner,
        title: "Plan runtime split".to_string(),
        objective: "Create architecture and implementation plan".to_string(),
        dependencies: Vec::new(),
        workspace: Some("/tmp/viden".to_string()),
        file_scope: vec!["crates/runtime".to_string(), "crates/types".to_string()],
        context_bundle_id: Some("ctx_agent_task_1".to_string()),
        required_evidence: vec!["plan".to_string(), "architecture".to_string()],
        permission_policy: "read_only".to_string(),
    };
    let dag = AgentDagRecord {
        dag_id: "dag_1".to_string(),
        goal: "Complete 0.2.2 agent role runtime".to_string(),
        status: AgentDagStatus::Active,
        tasks: vec![task.clone()],
        created_at: Some(10),
        updated_at: Some(11),
    };
    let evidence = EvidenceRecord {
        evidence_id: "ev_1".to_string(),
        task_id: task.task_id.clone(),
        kind: EvidenceKind::Plan,
        summary: "planner produced a scoped DAG".to_string(),
        path: Some("docs/multi-agent-core-orchestration.md".to_string()),
        source: Some("planner".to_string()),
        created_at: Some(12),
    };
    let gate = MergeGateRecord {
        gate_id: "gate_1".to_string(),
        task_id: task.task_id.clone(),
        status: MergeGateStatus::CollectingEvidence,
        required_evidence: vec!["plan".to_string(), "review".to_string()],
        evidence_ids: vec![evidence.evidence_id.clone()],
        gate_type: MergeGateType::Artifact,
        owner: RuntimeOwner::default(),
        validator: None,
        policy_snapshot: MergeGatePolicySnapshot::default(),
        decision: None,
        conflict: None,
        applied_change_id: None,
        recovery_snapshot: None,
        audit_ids: Vec::new(),
        updated_at: Some(13),
    };

    assert_eq!(AgentRole::parse("doc-writer"), Some(AgentRole::DocWriter));
    assert_eq!(AgentRole::ReleaseOperator.as_str(), "release_operator");
    assert!(AgentDagStatus::Active.is_active());
    assert!(!MergeGateStatus::Merged.is_open());

    let encoded = serde_json::to_string(&(dag, evidence, gate)).unwrap();
    let (decoded_dag, decoded_evidence, decoded_gate): (
        AgentDagRecord,
        EvidenceRecord,
        MergeGateRecord,
    ) = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded_dag.tasks[0].role, AgentRole::Planner);
    assert_eq!(decoded_evidence.kind, EvidenceKind::Plan);
    assert_eq!(decoded_gate.status, MergeGateStatus::CollectingEvidence);
}

#[test]
fn schema_one_merge_gate_accepts_legacy_decisions_and_unknown_fields() {
    let gate: MergeGateRecord = serde_json::from_value(serde_json::json!({
        "gate_id": "gate_legacy",
        "task_id": "task_legacy",
        "status": "accepted",
        "required_evidence": ["patch"],
        "evidence_ids": ["evidence_patch"],
        "decision": "accepted before typed trust records",
        "updated_at": 13,
        "future_schema_one_field": {"ignored": true}
    }))
    .unwrap();

    let decision = gate
        .decision
        .clone()
        .expect("legacy decision should migrate");
    assert_eq!(decision.outcome, MergeGateDecisionOutcome::Legacy);
    assert_eq!(decision.reason, "accepted before typed trust records");
    assert_eq!(gate.gate_type, MergeGateType::Artifact);
    assert_eq!(gate.owner, RuntimeOwner::default());
    let legacy_encoded = serde_json::to_value(&gate).unwrap();
    assert_eq!(
        legacy_encoded["decision"],
        "accepted before typed trust records"
    );
    assert!(legacy_encoded.get("gate_type").is_none());
    assert!(legacy_encoded.get("owner").is_none());

    let encoded = serde_json::to_value(MergeGateRecord {
        gate_id: "gate_typed".to_string(),
        task_id: "task_typed".to_string(),
        status: MergeGateStatus::Accepted,
        required_evidence: vec!["patch".to_string()],
        evidence_ids: vec!["evidence_patch".to_string()],
        gate_type: MergeGateType::Patch,
        owner: RuntimeOwner {
            workspace_id: "workspace".to_string(),
            project_id: "project".to_string(),
            task_id: Some("task_typed".to_string()),
            ..RuntimeOwner::default()
        },
        validator: None,
        policy_snapshot: MergeGatePolicySnapshot::default(),
        decision: Some(MergeGateDecision {
            outcome: MergeGateDecisionOutcome::Accepted,
            reason: "typed acceptance".to_string(),
            owner: RuntimeOwner::default(),
            evidence_ids: vec!["evidence_patch".to_string()],
            reviewed_evidence: Vec::new(),
            review_request_id: None,
            audit_id: "audit_typed".to_string(),
            decided_at: 14,
        }),
        conflict: None,
        applied_change_id: None,
        recovery_snapshot: None,
        audit_ids: vec!["audit_typed".to_string()],
        updated_at: Some(14),
    })
    .unwrap();

    assert_eq!(encoded["decision"]["outcome"], "accepted");
    assert!(encoded["decision"].is_object());
}

#[test]
fn schema_one_trust_loop_commands_roundtrip_as_additive_typed_variants() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-trust".to_string(),
        project_id: "project-trust".to_string(),
        lane_id: Some("lane-reviewer".to_string()),
        session_id: Some("session-reviewer".to_string()),
        task_id: Some("task-trust".to_string()),
        turn_id: None,
    };
    let commands = vec![
        RuntimeCommand::CreateHandoff {
            handoff_id: "handoff-trust".to_string(),
            task_id: "task-trust".to_string(),
            from_lane_id: "lane-coder".to_string(),
            to_lane_id: "lane-reviewer".to_string(),
            owner: owner.clone(),
            summary: "ready for review".to_string(),
            acceptance: HandoffAcceptance::Accepted,
        },
        RuntimeCommand::RequestReview {
            review_id: "review-trust".to_string(),
            gate_id: "gate-trust".to_string(),
            requester_lane_id: "lane-coder".to_string(),
            reviewer_lane_id: "lane-reviewer".to_string(),
            owner: owner.clone(),
            evidence_ids: vec!["evidence-trust".to_string()],
        },
        RuntimeCommand::ConfirmContract {
            contract_id: "contract-trust".to_string(),
            task_id: "task-trust".to_string(),
            owner: owner.clone(),
            summary: "contract confirmed".to_string(),
            decision: ContractDecision::Confirmed,
        },
        RuntimeCommand::SetDependency {
            dependency_id: "dependency-trust".to_string(),
            task_id: "task-trust".to_string(),
            depends_on_task_id: "task-base".to_string(),
            owner: owner.clone(),
            state: DependencyState::Blocked,
            reason: "waiting for base".to_string(),
        },
        RuntimeCommand::BounceMergeConflict {
            gate_id: "gate-trust".to_string(),
            original_lane_id: "lane-coder".to_string(),
            owner: owner.clone(),
            reason: "context mismatch".to_string(),
        },
        RuntimeCommand::RevalidateMergeConflict {
            gate_id: "gate-trust".to_string(),
            bounce_id: "bounce-trust".to_string(),
            actor: owner.clone(),
            evidence: ReviewedEvidenceBinding {
                evidence_id: "evidence-trust".to_string(),
                source_hash: "ab".repeat(32),
            },
        },
        RuntimeCommand::RevertAppliedChange {
            gate_id: "gate-trust".to_string(),
            owner,
            reason: "verification failed".to_string(),
        },
    ];

    let encoded = serde_json::to_string(&commands).unwrap();
    let decoded: Vec<RuntimeCommand> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, commands);
    for command_type in [
        "create_handoff",
        "request_review",
        "confirm_contract",
        "set_dependency",
        "bounce_merge_conflict",
        "revalidate_merge_conflict",
        "revert_applied_change",
    ] {
        assert!(encoded.contains(command_type));
    }
}

#[test]
fn schema_one_reject_commands_default_missing_actor_for_legacy_json() {
    let reject_gate: RuntimeCommand = serde_json::from_value(serde_json::json!({
        "type": "reject_merge_gate",
        "gate_id": "gate-legacy",
        "reason": "legacy rejection"
    }))
    .unwrap();
    assert!(matches!(
        reject_gate,
        RuntimeCommand::RejectMergeGate { actor, .. } if actor == RuntimeOwner::default()
    ));

    let reject_artifact: RuntimeCommand = serde_json::from_value(serde_json::json!({
        "type": "reject_agent_artifact",
        "gate_id": "gate-legacy",
        "evidence_id": "evidence-legacy",
        "reason": "legacy artifact rejection"
    }))
    .unwrap();
    assert!(matches!(
        reject_artifact,
        RuntimeCommand::RejectAgentArtifact { actor, .. } if actor == RuntimeOwner::default()
    ));
}

#[test]
fn trust_decisions_bind_actor_reviewed_hashes_and_durable_recovery_reference() {
    let actor = RuntimeOwner {
        workspace_id: "workspace-trust".to_string(),
        project_id: "project-trust".to_string(),
        lane_id: Some("lane-reviewer".to_string()),
        session_id: Some("session-reviewer".to_string()),
        task_id: Some("task-trust".to_string()),
        turn_id: None,
    };
    let binding = ReviewedEvidenceBinding {
        evidence_id: "evidence-patch".to_string(),
        source_hash: "ab".repeat(32),
    };
    let command = RuntimeCommand::AcceptMergeGate {
        gate_id: "gate-trust".to_string(),
        actor: actor.clone(),
        reviewed_evidence: vec![binding.clone()],
        decision: Some("reviewed exact patch bytes".to_string()),
    };
    let recovery = RecoverySnapshotReference {
        snapshot_id: "recovery-change-1".to_string(),
        manifest_sha256: "cd".repeat(32),
    };

    let command_json = serde_json::to_value(&command).unwrap();
    assert_eq!(command_json["actor"]["lane_id"], "lane-reviewer");
    assert_eq!(
        command_json["reviewed_evidence"][0]["source_hash"],
        "ab".repeat(32)
    );
    assert_eq!(
        serde_json::from_value::<RuntimeCommand>(command_json).unwrap(),
        command
    );
    assert_eq!(
        serde_json::from_value::<RecoverySnapshotReference>(
            serde_json::to_value(&recovery).unwrap()
        )
        .unwrap(),
        recovery
    );
}

#[test]
fn canonical_evidence_status_preserves_legacy_summary_only_shape() {
    let legacy_json = r#"{
        "id":"evidence-legacy",
        "kind":"patch",
        "summary":"patch was generated from an older ACP flow",
        "path":null,
        "source":"acp",
        "timestamp":12
    }"#;
    let evidence: EvidenceView = serde_json::from_str(legacy_json).unwrap();

    assert_eq!(evidence.canonical, None);
    assert_eq!(
        canonical_evidence_status(&evidence),
        EvidenceCanonicalStatus::Missing
    );
    assert!(
        !serde_json::to_string(&evidence)
            .unwrap()
            .contains("canonical")
    );
}

#[test]
fn canonical_evidence_status_reports_verified_and_quality_failed_states() {
    let mut evidence = EvidenceView {
        id: "evidence-canonical".to_string(),
        kind: "test_result".to_string(),
        summary: "cargo test -p viden-runtime passed".to_string(),
        path: None,
        source: Some("cargo".to_string()),
        canonical: Some(CanonicalEvidenceReference {
            item_id: "ctxi-test".to_string(),
            bundle_id: "bundle-test".to_string(),
            source_hash: "ab".repeat(32),
            producer: EvidenceProducer {
                identity: "executor".to_string(),
                role: "tester".to_string(),
                task_id: "task-test".to_string(),
            },
            permission_snapshot_id: Some("perm-task-test".to_string()),
            permission_scope: ContextScope::Task("task-test".to_string()),
            evidence_scope: ContextScope::Task("task-test".to_string()),
            verification: EvidenceVerificationState::Verified,
            quality: EvidenceQualityFacts {
                status: EvidenceQualityStatus::Pass,
                reason_codes: Vec::new(),
            },
        }),
        metadata: None,
        timestamp: Some(12),
    };

    assert_eq!(
        canonical_evidence_status(&evidence),
        EvidenceCanonicalStatus::Verified
    );

    let canonical = evidence.canonical.as_mut().unwrap();
    canonical.quality.status = EvidenceQualityStatus::Fail;
    canonical.quality.reason_codes = vec![EvidenceCanonicalReasonCode::QualityFailed];
    assert_eq!(
        canonical_evidence_status(&evidence),
        EvidenceCanonicalStatus::NeedsChanges
    );
}

#[test]
fn lsp_diagnostic_roundtrips_json() {
    let diagnostic = LspDiagnostic {
        path: "src/lib.rs".to_string(),
        range: LspRange {
            start: LspPosition {
                line: 1,
                character: 2,
            },
            end: LspPosition {
                line: 1,
                character: 5,
            },
        },
        severity: Some(2),
        source: Some("rust-analyzer".to_string()),
        code: Some("E0308".to_string()),
        message: "mismatched types".to_string(),
    };

    let encoded = serde_json::to_string(&diagnostic).unwrap();
    let decoded: LspDiagnostic = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, diagnostic);
}

#[test]
fn transcript_tool_result_preserves_exit_code() {
    let entry = TranscriptEntry::ToolResult {
        result: ToolResult {
            tool_call_id: "tool_1".to_string(),
            name: "shell".to_string(),
            output: "failed".to_string(),
            diff: None,
            success: false,
            exit_code: Some(7),
        },
    };

    let line = entry.to_json_line();
    assert!(line.contains("\"exit_code\":7"));
    assert_eq!(TranscriptEntry::from_json_line(&line).unwrap(), entry);
}

#[test]
fn transcript_cost_usage_roundtrips_canonical_and_legacy_shapes() {
    let cost = CostUsageRecord {
        usage_id: "usage-canonical-1".to_string(),
        provider_id: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        scopes: vec![CostScope::Request("req-1".to_string())],
        tokens: TokenUsage {
            input_tokens: Some(11),
            output_tokens: Some(3),
            cached_input_tokens: Some(2),
            retrieval_tokens: None,
            total_tokens: Some(14),
        },
        estimate: None,
        actual_cost: None,
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(123),
    };
    let entry = TranscriptEntry::CostUsage {
        cost: Box::new(cost.clone()),
    };
    let line = entry.to_json_line();

    assert_eq!(TranscriptEntry::from_json_line(&line).unwrap(), entry);

    let legacy = r#"{"type":"cost_usage","request_id":"req-legacy-1","provider_id":"legacy-provider","model":"legacy-model","scope":{"type":"task","id":"task-legacy-1"},"input_tokens":5,"output_tokens":7,"estimated_cost_micro_usd":9}"#;
    let TranscriptEntry::CostUsage { cost: legacy_cost } =
        TranscriptEntry::from_json_line(legacy).unwrap()
    else {
        panic!("expected cost usage entry");
    };
    assert_eq!(legacy_cost.tokens.input_tokens, Some(5));
    assert_eq!(legacy_cost.tokens.output_tokens, Some(7));
    assert_eq!(legacy_cost.tokens.cached_input_tokens, Some(0));
    assert_eq!(legacy_cost.actual_cost, None);
    assert!(
        legacy_cost
            .scopes
            .contains(&CostScope::Request("req-legacy-1".to_string()))
    );
    assert!(
        legacy_cost
            .scopes
            .contains(&CostScope::AgentTask("task-legacy-1".to_string()))
    );
}

#[test]
fn transcript_row_kinds_roundtrip_with_stable_serde_names() {
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "cargo test".to_string());
    let cost = CostUsageRecord {
        usage_id: "usage-row".to_string(),
        provider_id: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        scopes: vec![CostScope::Request("request-row".to_string())],
        tokens: TokenUsage {
            input_tokens: Some(10),
            output_tokens: Some(2),
            cached_input_tokens: Some(1),
            retrieval_tokens: None,
            total_tokens: Some(12),
        },
        estimate: None,
        actual_cost: None,
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(44),
    };
    let runtime_event = RuntimeEvent::with_timestamp(
        7,
        Some(55),
        RuntimeEventKind::CostUsageRecorded { cost: cost.clone() },
    );
    let kinds = vec![
        TranscriptRowKind::Message {
            message: Message {
                id: "msg-row".to_string(),
                role: Role::Assistant,
                content: "hello".to_string(),
                timestamp: 11,
                tool_name: None,
                tool_call_id: None,
            },
        },
        TranscriptRowKind::ToolCall {
            call: ToolCall {
                id: "tool-call-row".to_string(),
                name: "shell".to_string(),
                input,
            },
        },
        TranscriptRowKind::ToolResult {
            result: ToolResult {
                tool_call_id: "tool-call-row".to_string(),
                name: "shell".to_string(),
                output: "ok".to_string(),
                diff: None,
                success: true,
                exit_code: Some(0),
            },
        },
        TranscriptRowKind::Permission {
            entry: PermissionLogEntry {
                timestamp: 12,
                tool_name: "shell".to_string(),
                decision: "allow".to_string(),
                reason: "test".to_string(),
                message: None,
            },
        },
        TranscriptRowKind::Command {
            entry: CommandLogEntry {
                timestamp: 13,
                name: "status".to_string(),
                args: vec!["--json".to_string()],
                output: "{}".to_string(),
            },
        },
        TranscriptRowKind::SessionMeta {
            entry: SessionMetaEntry {
                timestamp: 14,
                key: "model".to_string(),
                value: "deepseek".to_string(),
            },
        },
        TranscriptRowKind::CostUsage {
            cost: Box::new(cost),
        },
        TranscriptRowKind::RuntimeEvent {
            event: Box::new(runtime_event),
        },
    ];

    let encoded = serde_json::to_value(&kinds).unwrap();
    let names = encoded
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
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
    let decoded: Vec<TranscriptRowKind> = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, kinds);
}

#[test]
fn transcript_row_page_request_and_loaded_event_roundtrip_json() {
    let page = TranscriptPage {
        rows: vec![TranscriptRow {
            id: TranscriptRowId("session-a:0".to_string()),
            cursor: TranscriptCursor {
                session_id: "session-a".to_string(),
                ordinal: 0,
            },
            timestamp: Some(1),
            kind: TranscriptRowKind::Message {
                message: Message {
                    id: "msg-a".to_string(),
                    role: Role::User,
                    content: "hello".to_string(),
                    timestamp: 1,
                    tool_name: None,
                    tool_call_id: None,
                },
            },
        }],
        older: None,
        newer: None,
        has_more: false,
    };
    let command = RuntimeCommand::LoadTranscriptPage {
        request: TranscriptPageRequest {
            session_id: "session-a".to_string(),
            before: None,
            limit: 25,
        },
    };
    let event = RuntimeEventKind::TranscriptPageLoaded {
        page: Box::new(page),
    };

    assert_eq!(
        serde_json::to_value(&command).unwrap()["type"],
        "load_transcript_page"
    );
    assert_eq!(
        serde_json::to_value(&event).unwrap()["type"],
        "transcript_page_loaded"
    );
    assert!(serde_json::to_string(&event).unwrap().contains("\"rows\""));
}

fn runtime_snapshot_for_contract() -> RuntimeSnapshot {
    RuntimeSnapshot {
        cwd: PathBuf::from("/tmp/viden"),
        provider_family: "deepseek".to_string(),
        model_label: "deepseek-v4-flash".to_string(),
        work_mode: WorkMode::Build,
        permission_mode: PermissionMode::Default,
        permission_level: PermissionLevel::Ask,
        config_summary: "provider=deepseek model=deepseek-v4-flash".to_string(),
        loaded_config_files: vec![PathBuf::from("/tmp/viden/config.toml")],
        startup_overrides: vec!["--provider=deepseek".to_string()],
        ui_preferences: ResolvedUiPreferences::default(),
    }
}

fn starter_lane_for_contract(lane_id: &str) -> AgentLaneRecord {
    AgentLaneRecord {
        id: lane_id.to_string(),
        task_id: None,
        role: AgentRole::Coder,
        route: AgentRoute::BuiltIn,
        gate_strength: GateStrength::Full,
        mutation_policy: MutationPolicy::ProposeOnly,
        worktree: Some(format!("/tmp/viden/.worktrees/{lane_id}")),
        branch: Some(format!("codex/{lane_id}")),
        target: ExecutionTarget::Local,
        data_egress: DataEgressPolicy::Deny,
        status: LaneStatus::Draft,
        budget: LaneBudget::default(),
        active_session_ids: Vec::new(),
        summary: "coder starter lane".to_string(),
        evidence: Vec::new(),
    }
}

#[test]
fn runtime_v1_envelopes_roundtrip_owner_cursor_and_capabilities() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-a".to_string(),
        project_id: "project-a".to_string(),
        lane_id: Some("lane-a".to_string()),
        session_id: Some("session-a".to_string()),
        task_id: Some("task-a".to_string()),
        turn_id: Some("turn-a".to_string()),
    };
    let cursor = EventCursor {
        stream_id: "stream-a".to_string(),
        sequence: 7,
    };
    let capabilities = BTreeSet::from([
        CapabilityId("runtime.replay".to_string()),
        CapabilityId("runtime.snapshot".to_string()),
    ]);
    let event = RuntimeEvent::with_timestamp(
        cursor.sequence,
        Some(17),
        RuntimeEventKind::AssistantDelta {
            message_id: "message-a".to_string(),
            task_id: owner.task_id.clone(),
            content: "hello".to_string(),
        },
    );
    let command = RuntimeCommandEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        client_id: "tui-a".to_string(),
        command_id: "command-a".to_string(),
        owner: owner.clone(),
        command: RuntimeCommand::QueueFollowUp {
            content: "continue".to_string(),
        },
    };
    let event = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: owner.clone(),
        cursor: cursor.clone(),
        event: RuntimeWireEvent::Known(event),
    };
    let snapshot = RuntimeSnapshotEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        capabilities: capabilities.clone(),
        cursor: cursor.clone(),
        snapshot: runtime_snapshot_for_contract(),
        view: RuntimeViewState::new(runtime_snapshot_for_contract()),
    };
    let handshake = CoreHandshake {
        core_version: "0.3.0".to_string(),
        supported_schema_versions: vec![FRONTEND_SCHEMA_V1],
        active_schema_version: FRONTEND_SCHEMA_V1,
        capabilities,
    };

    let encoded = serde_json::to_string(&(command, event, snapshot, handshake)).unwrap();
    let (command, event, snapshot, handshake): (
        RuntimeCommandEnvelope,
        RuntimeEventEnvelope,
        RuntimeSnapshotEnvelope,
        CoreHandshake,
    ) = serde_json::from_str(&encoded).unwrap();

    assert_eq!(command.schema_version, SchemaVersion(1));
    assert_eq!(command.owner, owner);
    assert_eq!(event.cursor, cursor);
    assert!(matches!(event.event, RuntimeWireEvent::Known(ref event) if event.sequence == 7));
    assert_eq!(snapshot.capabilities, handshake.capabilities);
}

#[test]
fn starter_lane_events_roundtrip_as_known_with_exact_owner_and_cursor() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-starter".to_string(),
        project_id: "project-starter".to_string(),
        lane_id: Some("lane-starter".to_string()),
        session_id: Some("session-starter".to_string()),
        task_id: None,
        turn_id: Some("turn-starter".to_string()),
    };
    let lane = starter_lane_for_contract("lane-starter");
    let preview = StarterLanePreview {
        preview_id: "preview-starter".to_string(),
        content_sha256: "ab".repeat(32),
        owner: owner.clone(),
        lane: lane.clone(),
        branch: "codex/lane-starter".to_string(),
        worktree_path: "/tmp/viden/.worktrees/lane-starter".to_string(),
        base_revision: "cd".repeat(20),
        diagnostics: Vec::new(),
    };
    let cases = [
        (
            "starter_lane_previewed",
            RuntimeEventKind::StarterLanePreviewed {
                preview: preview.clone(),
            },
        ),
        (
            "starter_lane_created",
            RuntimeEventKind::StarterLaneCreated {
                receipt: StarterLaneReceipt {
                    preview_id: preview.preview_id.clone(),
                    content_sha256: preview.content_sha256.clone(),
                    lane,
                    branch: preview.branch.clone(),
                    worktree_path: preview.worktree_path.clone(),
                    base_revision: preview.base_revision.clone(),
                    owner: owner.clone(),
                },
            },
        ),
        (
            "starter_lane_preview_invalidated",
            RuntimeEventKind::StarterLanePreviewInvalidated {
                owner: owner.clone(),
                preview_id: preview.preview_id,
                reason: StarterLanePreviewInvalidationReason::HashMismatch,
            },
        ),
    ];

    for (index, (expected_type, kind)) in cases.into_iter().enumerate() {
        let sequence = index as u64 + 1;
        let envelope = RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: owner.clone(),
            cursor: EventCursor {
                stream_id: "stream-starter".to_string(),
                sequence,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(sequence, Some(17), kind)),
        };
        let encoded = serde_json::to_value(&envelope).unwrap();
        assert_eq!(encoded["event"]["kind"]["type"], expected_type);
        let decoded: RuntimeEventEnvelope = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, envelope);
        assert!(
            matches!(
                &decoded.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewed { preview },
                    ..
                }) if preview.owner == decoded.owner
            ) || matches!(
                &decoded.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLaneCreated { receipt },
                    ..
                }) if receipt.owner == decoded.owner
            ) || matches!(
                &decoded.event,
                RuntimeWireEvent::Known(RuntimeEvent {
                    kind: RuntimeEventKind::StarterLanePreviewInvalidated { owner, .. },
                    ..
                }) if owner == &decoded.owner
            )
        );
        assert_eq!(decoded.owner, owner);
        assert_eq!(decoded.cursor.stream_id, "stream-starter");
        assert_eq!(decoded.cursor.sequence, sequence);
    }
}

#[test]
fn lane_runtime_owner_binding_reduces_by_lane_and_clears_only_terminal_lane() {
    let owner = |lane_id: &str, turn_id: &str| RuntimeOwner {
        workspace_id: "workspace-owner".to_string(),
        project_id: "project-owner".to_string(),
        lane_id: Some(lane_id.to_string()),
        session_id: Some(format!("session-{lane_id}")),
        task_id: Some(format!("task-{lane_id}")),
        turn_id: Some(turn_id.to_string()),
    };
    let binding_a = LaneRuntimeOwnerBinding {
        lane_id: "lane-a".to_string(),
        owner: owner("lane-a", "turn-a"),
    };
    let replacement_a = LaneRuntimeOwnerBinding {
        lane_id: "lane-a".to_string(),
        owner: owner("lane-a", "turn-a-next"),
    };
    let binding_b = LaneRuntimeOwnerBinding {
        lane_id: "lane-b".to_string(),
        owner: owner("lane-b", "turn-b"),
    };

    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());
    for binding in [binding_a.clone(), replacement_a.clone(), binding_b.clone()] {
        view.apply_event(&RuntimeEvent::new(
            1,
            RuntimeEventKind::LaneRuntimeOwnerBound { binding },
        ));
    }
    assert_eq!(
        view.lane_runtime_owners,
        vec![replacement_a.clone(), binding_b.clone()]
    );

    let mismatched = LaneRuntimeOwnerBinding {
        lane_id: "lane-a".to_string(),
        owner: owner("lane-other", "turn-invalid"),
    };
    view.apply_event(&RuntimeEvent::new(
        2,
        RuntimeEventKind::LaneRuntimeOwnerBound {
            binding: mismatched,
        },
    ));
    assert_eq!(
        view.lane_runtime_owners,
        vec![replacement_a.clone(), binding_b.clone()]
    );

    let mut running_lane = starter_lane_for_contract("lane-a");
    running_lane.status = LaneStatus::Running;
    view.apply_event(&RuntimeEvent::new(
        3,
        RuntimeEventKind::LaneUpdated { lane: running_lane },
    ));
    assert_eq!(
        view.lane_runtime_owners,
        vec![replacement_a.clone(), binding_b.clone()]
    );

    for status in [
        LaneStatus::Done,
        LaneStatus::Failed,
        LaneStatus::Cancelled,
        LaneStatus::Archived,
    ] {
        let mut terminal_view = RuntimeViewState::new(runtime_snapshot_for_contract());
        for binding in [replacement_a.clone(), binding_b.clone()] {
            terminal_view.apply_event(&RuntimeEvent::new(
                1,
                RuntimeEventKind::LaneRuntimeOwnerBound { binding },
            ));
        }
        let mut lane = starter_lane_for_contract("lane-a");
        lane.status = status;
        terminal_view.apply_event(&RuntimeEvent::new(
            2,
            RuntimeEventKind::LaneUpdated { lane },
        ));
        assert_eq!(terminal_view.lane_runtime_owners, vec![binding_b.clone()]);
    }
}

#[test]
fn lane_runtime_owner_event_roundtrips_as_known_while_future_event_is_inert() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-owner".to_string(),
        project_id: "project-owner".to_string(),
        lane_id: Some("lane-a".to_string()),
        session_id: Some("session-a".to_string()),
        task_id: Some("task-a".to_string()),
        turn_id: Some("turn-a".to_string()),
    };
    let binding = LaneRuntimeOwnerBinding {
        lane_id: "lane-a".to_string(),
        owner: owner.clone(),
    };
    let envelope = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner,
        cursor: EventCursor {
            stream_id: "stream-owner".to_string(),
            sequence: 1,
        },
        event: RuntimeWireEvent::Known(RuntimeEvent::new(
            1,
            RuntimeEventKind::LaneRuntimeOwnerBound {
                binding: binding.clone(),
            },
        )),
    };

    let encoded = serde_json::to_value(&envelope).unwrap();
    assert_eq!(encoded["event"]["kind"]["type"], "lane_runtime_owner_bound");
    let decoded: RuntimeEventEnvelope = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, envelope);
    assert!(matches!(
        decoded.event,
        RuntimeWireEvent::Known(RuntimeEvent {
            kind: RuntimeEventKind::LaneRuntimeOwnerBound { binding: decoded },
            ..
        }) if decoded == binding
    ));

    let future: RuntimeWireEvent = serde_json::from_value(serde_json::json!({
        "sequence": 2,
        "timestamp": null,
        "kind": {
            "type": "future_lane_runtime_owner_rotated",
            "payload": {"lane_id": "lane-a"}
        }
    }))
    .unwrap();
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());
    view.apply_event(&RuntimeEvent::new(
        1,
        RuntimeEventKind::LaneRuntimeOwnerBound { binding },
    ));
    let before = view.clone();
    if let RuntimeWireEvent::Known(event) = future {
        view.apply_event(&event);
    }
    assert_eq!(view, before);
}

#[test]
fn lane_runtime_owner_event_transport_rejects_payload_owner_mismatch() {
    let payload_owner = RuntimeOwner {
        workspace_id: "workspace-owner".to_string(),
        project_id: "project-owner".to_string(),
        lane_id: Some("lane-a".to_string()),
        session_id: Some("session-payload".to_string()),
        task_id: Some("task-a".to_string()),
        turn_id: Some("turn-payload".to_string()),
    };
    let envelope_owner = RuntimeOwner {
        session_id: Some("session-envelope".to_string()),
        turn_id: Some("turn-envelope".to_string()),
        ..payload_owner.clone()
    };
    let envelope = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: envelope_owner,
        cursor: EventCursor {
            stream_id: "stream-owner".to_string(),
            sequence: 1,
        },
        event: RuntimeWireEvent::Known(RuntimeEvent::new(
            1,
            RuntimeEventKind::LaneRuntimeOwnerBound {
                binding: LaneRuntimeOwnerBinding {
                    lane_id: "lane-a".to_string(),
                    owner: payload_owner,
                },
            },
        )),
    };

    assert!(serde_json::to_value(envelope).is_err());
}

#[test]
fn starter_lane_view_keys_previews_receipts_and_invalidation_by_owner_and_preview_id() {
    let owner_a = RuntimeOwner {
        workspace_id: "workspace-a".to_string(),
        project_id: "project".to_string(),
        lane_id: Some("lane-a".to_string()),
        ..RuntimeOwner::default()
    };
    let owner_b = RuntimeOwner {
        workspace_id: "workspace-b".to_string(),
        project_id: "project".to_string(),
        lane_id: Some("lane-b".to_string()),
        ..RuntimeOwner::default()
    };
    let preview = |owner: RuntimeOwner, lane_id: &str| StarterLanePreview {
        preview_id: "colliding-preview".to_string(),
        content_sha256: "ab".repeat(32),
        owner,
        lane: starter_lane_for_contract(lane_id),
        branch: format!("codex/{lane_id}"),
        worktree_path: format!("/tmp/viden/.worktrees/{lane_id}"),
        base_revision: "cd".repeat(20),
        diagnostics: Vec::new(),
    };
    let preview_a = preview(owner_a.clone(), "lane-a");
    let preview_b = preview(owner_b.clone(), "lane-b");
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());
    for preview in [preview_a.clone(), preview_b.clone()] {
        view.apply_event(&RuntimeEvent::new(
            1,
            RuntimeEventKind::StarterLanePreviewed { preview },
        ));
    }
    assert_eq!(view.starter_lane_previews.len(), 2);

    view.apply_event(&RuntimeEvent::new(
        2,
        RuntimeEventKind::StarterLanePreviewInvalidated {
            owner: owner_a.clone(),
            preview_id: preview_a.preview_id.clone(),
            reason: StarterLanePreviewInvalidationReason::HashMismatch,
        },
    ));
    assert_eq!(view.starter_lane_previews, vec![preview_b.clone()]);

    view.apply_event(&RuntimeEvent::new(
        3,
        RuntimeEventKind::StarterLanePreviewed {
            preview: preview_a.clone(),
        },
    ));
    for preview in [preview_a, preview_b] {
        view.apply_event(&RuntimeEvent::new(
            4,
            RuntimeEventKind::StarterLaneCreated {
                receipt: StarterLaneReceipt {
                    preview_id: preview.preview_id,
                    content_sha256: preview.content_sha256,
                    lane: preview.lane,
                    branch: preview.branch,
                    worktree_path: preview.worktree_path,
                    base_revision: preview.base_revision,
                    owner: preview.owner,
                },
            },
        ));
    }
    assert!(view.starter_lane_previews.is_empty());
    assert_eq!(view.starter_lane_receipts.len(), 2);
    assert!(
        view.starter_lane_receipts
            .iter()
            .any(|receipt| receipt.owner == owner_a)
    );
    assert!(
        view.starter_lane_receipts
            .iter()
            .any(|receipt| receipt.owner == owner_b)
    );
}

#[test]
fn starter_lane_event_transport_rejects_payload_owner_mismatch() {
    let envelope_owner = RuntimeOwner {
        workspace_id: "workspace-envelope".to_string(),
        project_id: "project".to_string(),
        ..RuntimeOwner::default()
    };
    let payload_owner = RuntimeOwner {
        workspace_id: "workspace-payload".to_string(),
        project_id: "project".to_string(),
        ..RuntimeOwner::default()
    };
    let events = [
        RuntimeEventKind::StarterLaneCreated {
            receipt: StarterLaneReceipt {
                preview_id: "preview-mismatch".to_string(),
                content_sha256: "ab".repeat(32),
                lane: starter_lane_for_contract("lane-mismatch"),
                branch: "codex/lane-mismatch".to_string(),
                worktree_path: "/tmp/viden/.worktrees/lane-mismatch".to_string(),
                base_revision: "cd".repeat(20),
                owner: payload_owner.clone(),
            },
        },
        RuntimeEventKind::StarterLanePreviewInvalidated {
            owner: payload_owner,
            preview_id: "preview-mismatch".to_string(),
            reason: StarterLanePreviewInvalidationReason::PermissionDenied,
        },
    ];

    for kind in events {
        let envelope = RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: envelope_owner.clone(),
            cursor: EventCursor {
                stream_id: "stream-mismatch".to_string(),
                sequence: 1,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::new(1, kind)),
        };
        assert!(serde_json::to_value(envelope).is_err());
    }
}

#[test]
fn runtime_v1_unknown_event_is_preserved() {
    let raw = r#"{
        "schema_version": 1,
        "owner": {
            "workspace_id": "workspace-a",
            "project_id": "project-a",
            "lane_id": "lane-a",
            "session_id": "session-a",
            "task_id": "task-a",
            "turn_id": "turn-a"
        },
        "cursor": {"stream_id": "stream-a", "sequence": 9},
        "event": {
            "sequence": 9,
            "timestamp": 17,
            "kind": {"type": "future_event", "payload": {"x": 1}}
        }
    }"#;

    let decoded: RuntimeEventEnvelope = serde_json::from_str(raw).unwrap();
    assert_eq!(decoded.schema_version, FRONTEND_SCHEMA_V1);
    assert_eq!(decoded.cursor.stream_id, "stream-a");
    assert!(matches!(
        decoded.event,
        RuntimeWireEvent::Unknown { ref event_type, ref payload }
            if event_type == "future_event" && payload == &serde_json::json!({"x": 1})
    ));
    assert_eq!(
        EventCursor {
            stream_id: "stream-a".to_string(),
            sequence: 8,
        }
        .classify_incoming(&decoded.cursor),
        EventCursorOrder::Next
    );

    let encoded = serde_json::to_string(&decoded).unwrap();
    let replayed: RuntimeEventEnvelope = serde_json::from_str(&encoded).unwrap();
    assert_eq!(replayed, decoded);
}

#[test]
fn frontend_host_capabilities_known_wire_events_roundtrip_without_placeholders() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-host-fixture".to_string(),
        project_id: "project-host-fixture".to_string(),
        lane_id: Some("lane-host-fixture".to_string()),
        session_id: Some("session-host-fixture".to_string()),
        task_id: Some("task_host_fixture".to_string()),
        turn_id: Some("turn-host-fixture".to_string()),
    };
    let lane = starter_lane_for_contract("lane-host-fixture");
    let preview = StarterLanePreview {
        preview_id: "preview-host-fixture".to_string(),
        content_sha256: "ab".repeat(32),
        owner: owner.clone(),
        lane: lane.clone(),
        branch: "codex/lane-host-fixture".to_string(),
        worktree_path: "workspace/.worktrees/lane-host-fixture".to_string(),
        base_revision: "cd".repeat(20),
        diagnostics: Vec::new(),
    };
    let resolved = ResolvedUiPreferences {
        locale: LocaleId::ZhCn,
        skin: UiSkin::Ice,
        mode: UiColorMode::Dark,
        density: UiDensity::Compact,
        motion: UiMotion::Reduced,
        diagnostics: Vec::new(),
    };
    let cases = [
        RuntimeEventKind::UiPreferencesUpdated {
            resolved,
            persisted: None,
            diagnostics: Vec::new(),
        },
        RuntimeEventKind::RecentWorkLoaded {
            projects: vec![RecentProjectSummary {
                canonical_root: "workspace/project".to_string(),
                display_name: "project".to_string(),
                last_updated_at: 20,
                latest_session_id: Some("session-host-fixture".to_string()),
            }],
            sessions: vec![RecentSessionSummary {
                canonical_root: "workspace/project".to_string(),
                session_id: "session-host-fixture".to_string(),
                created_at: 10,
                last_updated_at: 20,
                message_count: 1,
                tool_call_count: 0,
                command_count: 1,
            }],
            diagnostics: Vec::new(),
        },
        RuntimeEventKind::StarterLanePreviewed {
            preview: preview.clone(),
        },
        RuntimeEventKind::StarterLaneCreated {
            receipt: StarterLaneReceipt {
                preview_id: preview.preview_id.clone(),
                content_sha256: preview.content_sha256.clone(),
                lane,
                branch: preview.branch.clone(),
                worktree_path: preview.worktree_path.clone(),
                base_revision: preview.base_revision.clone(),
                owner: owner.clone(),
            },
        },
        RuntimeEventKind::StarterLanePreviewInvalidated {
            owner: owner.clone(),
            preview_id: preview.preview_id,
            reason: StarterLanePreviewInvalidationReason::BaseRevisionChanged,
        },
        RuntimeEventKind::LaneRuntimeOwnerBound {
            binding: LaneRuntimeOwnerBinding {
                lane_id: "lane-host-fixture".to_string(),
                owner: owner.clone(),
            },
        },
    ];

    for (index, kind) in cases.into_iter().enumerate() {
        let sequence = index as u64 + 1;
        let envelope = RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: owner.clone(),
            cursor: EventCursor {
                stream_id: "fixture:frontend-host-services".to_string(),
                sequence,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
                sequence,
                Some(1_700_000_000 + sequence),
                kind,
            )),
        };
        let encoded = serde_json::to_value(&envelope).unwrap();
        let decoded: RuntimeEventEnvelope = serde_json::from_value(encoded).unwrap();
        assert!(
            matches!(decoded.event, RuntimeWireEvent::Known(_)),
            "event {sequence} must use a real known schema-1 wire fact"
        );
    }
}

#[test]
fn agent_adapter_and_session_commands_roundtrip_as_typed_schema_v1_intents() {
    let request = AgentSessionRequest {
        lane_id: "lane-agent".to_string(),
        agent_id: "claude-acp".to_string(),
        model: Some("sonnet".to_string()),
        load_session_id: Some("remote-session".to_string()),
        task: "review the runtime contract".to_string(),
    };
    let commands = [
        RuntimeCommand::QueryAgentAdapters,
        RuntimeCommand::ProbeAgentAdapter {
            agent_id: "claude-acp".to_string(),
        },
        RuntimeCommand::StartAgentSession {
            request: request.clone(),
        },
        RuntimeCommand::CancelAgentSession {
            session_id: "agent-session-1".to_string(),
        },
    ];

    for command in commands {
        let encoded = serde_json::to_string(&command).unwrap();
        let decoded: RuntimeCommand = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, command);
    }
}

#[test]
fn agent_adapter_and_session_events_roundtrip_as_known_owner_scoped_facts() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-agent".to_string(),
        project_id: "project-agent".to_string(),
        lane_id: Some("lane-agent".to_string()),
        session_id: Some("agent-session-1".to_string()),
        ..RuntimeOwner::default()
    };
    let adapter = AgentAdapterView {
        agent_id: "claude-acp".to_string(),
        display_name: "Claude Agent".to_string(),
        route: AgentRoute::Acp,
        source: AgentAdapterSource::Registry,
        availability: AgentAvailability::Available,
        auth_state: AgentAuthState::Ready,
        capabilities: vec![CapabilityId("agent.session.prompt".to_string())],
        models: Vec::new(),
        diagnostics: Vec::new(),
    };
    let session = AgentSessionView {
        session_id: "agent-session-1".to_string(),
        lane_id: "lane-agent".to_string(),
        agent_id: adapter.agent_id.clone(),
        model: None,
        status: AgentSessionStatus::Running,
        owner: owner.clone(),
        task: "review the runtime contract".to_string(),
        diagnostic: None,
    };
    let cases = [
        RuntimeEventKind::AgentAdaptersLoaded {
            adapters: vec![adapter.clone()],
        },
        RuntimeEventKind::AgentAdapterProbed { adapter },
        RuntimeEventKind::AgentSessionStarted {
            session: session.clone(),
        },
        RuntimeEventKind::AgentSessionUpdated {
            session: session.clone(),
        },
        RuntimeEventKind::AgentSessionCompleted {
            session: session.clone(),
        },
        RuntimeEventKind::AgentSessionFailed { session },
    ];

    for (index, kind) in cases.into_iter().enumerate() {
        let sequence = index as u64 + 1;
        let envelope = RuntimeEventEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            owner: owner.clone(),
            cursor: EventCursor {
                stream_id: "stream-agent".to_string(),
                sequence,
            },
            event: RuntimeWireEvent::Known(RuntimeEvent::new(sequence, kind)),
        };
        let encoded = serde_json::to_string(&envelope).unwrap();
        let decoded: RuntimeEventEnvelope = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(decoded.event, RuntimeWireEvent::Known(_)));
    }

    let mismatched = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: RuntimeOwner {
            lane_id: Some("other-lane".to_string()),
            ..owner
        },
        cursor: EventCursor {
            stream_id: "stream-agent".to_string(),
            sequence: 1,
        },
        event: RuntimeWireEvent::Known(RuntimeEvent::new(
            1,
            RuntimeEventKind::AgentSessionStarted {
                session: AgentSessionView {
                    session_id: "agent-session-2".to_string(),
                    lane_id: "lane-agent".to_string(),
                    agent_id: "claude-acp".to_string(),
                    model: None,
                    status: AgentSessionStatus::Starting,
                    owner: RuntimeOwner {
                        workspace_id: "workspace-agent".to_string(),
                        project_id: "project-agent".to_string(),
                        lane_id: Some("lane-agent".to_string()),
                        session_id: Some("agent-session-2".to_string()),
                        ..RuntimeOwner::default()
                    },
                    task: "review".to_string(),
                    diagnostic: None,
                },
            },
        )),
    };
    assert!(serde_json::to_string(&mismatched).is_err());

    let embedded_owner = RuntimeOwner {
        workspace_id: "workspace-agent".to_string(),
        project_id: "project-agent".to_string(),
        lane_id: Some("lane-agent".to_string()),
        session_id: Some("agent-session-3".to_string()),
        ..RuntimeOwner::default()
    };
    let inconsistent_identity = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: embedded_owner.clone(),
        cursor: EventCursor {
            stream_id: "stream-agent".to_string(),
            sequence: 1,
        },
        event: RuntimeWireEvent::Known(RuntimeEvent::new(
            1,
            RuntimeEventKind::AgentSessionStarted {
                session: AgentSessionView {
                    session_id: "different-session".to_string(),
                    lane_id: "different-lane".to_string(),
                    agent_id: "claude-acp".to_string(),
                    model: None,
                    status: AgentSessionStatus::Starting,
                    owner: embedded_owner,
                    task: "review".to_string(),
                    diagnostic: None,
                },
            },
        )),
    };
    assert!(serde_json::to_string(&inconsistent_identity).is_err());
}

#[test]
fn agent_adapter_and_session_events_reduce_by_stable_identity_and_owner() {
    let owner = RuntimeOwner {
        workspace_id: "workspace-agent".to_string(),
        project_id: "project-agent".to_string(),
        lane_id: Some("lane-agent".to_string()),
        session_id: Some("agent-session-1".to_string()),
        task_id: None,
        turn_id: Some("turn-agent".to_string()),
    };
    let adapter = AgentAdapterView {
        agent_id: "claude-acp".to_string(),
        display_name: "Claude Agent".to_string(),
        route: AgentRoute::Acp,
        source: AgentAdapterSource::Registry,
        availability: AgentAvailability::NeedsAuth,
        auth_state: AgentAuthState::LoggedOut,
        capabilities: vec![CapabilityId("agent.session.prompt".to_string())],
        models: vec!["sonnet".to_string()],
        diagnostics: vec!["run claude auth login".to_string()],
    };
    let running = AgentSessionView {
        session_id: "agent-session-1".to_string(),
        lane_id: "lane-agent".to_string(),
        agent_id: adapter.agent_id.clone(),
        model: Some("sonnet".to_string()),
        status: AgentSessionStatus::Running,
        owner: owner.clone(),
        task: "review the runtime contract".to_string(),
        diagnostic: None,
    };
    let completed = AgentSessionView {
        status: AgentSessionStatus::Completed,
        diagnostic: Some("evidence recorded".to_string()),
        ..running.clone()
    };
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());

    view.apply_event(&RuntimeEvent::new(
        1,
        RuntimeEventKind::AgentAdaptersLoaded {
            adapters: vec![adapter.clone()],
        },
    ));
    view.apply_event(&RuntimeEvent::new(
        2,
        RuntimeEventKind::AgentAdapterProbed {
            adapter: AgentAdapterView {
                availability: AgentAvailability::Available,
                auth_state: AgentAuthState::Ready,
                diagnostics: Vec::new(),
                ..adapter
            },
        },
    ));
    view.apply_event(&RuntimeEvent::new(
        3,
        RuntimeEventKind::AgentSessionStarted {
            session: running.clone(),
        },
    ));
    view.apply_event(&RuntimeEvent::new(
        4,
        RuntimeEventKind::AgentSessionCompleted {
            session: completed.clone(),
        },
    ));

    assert_eq!(view.agent_adapters.len(), 1);
    assert_eq!(
        view.agent_adapters[0].availability,
        AgentAvailability::Available
    );
    assert_eq!(view.agent_sessions, vec![completed]);
}

#[test]
fn legacy_runtime_view_without_agent_extensions_defaults_to_empty_collections() {
    let view = RuntimeViewState::new(runtime_snapshot_for_contract());
    let mut encoded = serde_json::to_value(view).unwrap();
    encoded.as_object_mut().unwrap().remove("agent_adapters");
    encoded.as_object_mut().unwrap().remove("agent_sessions");

    let decoded: RuntimeViewState = serde_json::from_value(encoded).unwrap();

    assert!(decoded.agent_adapters.is_empty());
    assert!(decoded.agent_sessions.is_empty());
}

#[test]
fn runtime_v1_known_event_with_unknown_nested_command_is_preserved() {
    let raw = r#"{
        "sequence": 9,
        "timestamp": 17,
        "kind": {
            "type": "command_accepted",
            "payload": {
                "command_id": "command-future",
                "command": {"type": "future_lane_command", "lane_id": "lane-a"}
            }
        }
    }"#;

    let decoded: RuntimeWireEvent = serde_json::from_str(raw).unwrap();
    assert!(matches!(
        decoded,
        RuntimeWireEvent::Unknown { ref event_type, ref payload }
            if event_type == "command_accepted"
                && payload["command_id"] == "command-future"
    ));
}

#[test]
fn runtime_v1_lane_event_with_unknown_nested_command_is_preserved() {
    let raw = r#"{
        "sequence": 10,
        "timestamp": 18,
        "kind": {
            "type": "lane_command_accepted",
            "payload": {
                "command_id": "command-future-lane",
                "command": {"type": "future_lane_command", "lane_id": "lane-a"}
            }
        }
    }"#;

    let decoded: RuntimeWireEvent = serde_json::from_str(raw).unwrap();
    assert!(matches!(
        decoded,
        RuntimeWireEvent::Unknown { ref event_type, ref payload }
            if event_type == "lane_command_accepted"
                && payload["command_id"] == "command-future-lane"
    ));
}

#[test]
fn approval_response_legacy_bool_decodes_but_structured_serialization_omits_approved() {
    let legacy_allow: ApprovalResponse =
        serde_json::from_str(r#"{"approved":true,"feedback":"ok"}"#).unwrap();
    assert_eq!(
        legacy_allow.decision,
        ApprovalDecision::Allow {
            scope: ApprovalScope::Once
        }
    );
    assert_eq!(legacy_allow.feedback.as_deref(), Some("ok"));

    let legacy_deny: ApprovalResponse = serde_json::from_str(r#"{"approved":false}"#).unwrap();
    assert_eq!(legacy_deny.decision, ApprovalDecision::Deny);

    let encoded = serde_json::to_value(ApprovalResponse::allow_once(None)).unwrap();
    assert_eq!(encoded["decision"]["allow"]["scope"], "once");
    assert!(encoded.get("approved").is_none());
}

#[test]
fn runtime_v1_known_event_sequence_mismatch_is_rejected_at_wire_boundary() {
    let mismatched = RuntimeEventEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        owner: RuntimeOwner::default(),
        cursor: EventCursor {
            stream_id: "stream-a".to_string(),
            sequence: 7,
        },
        event: RuntimeWireEvent::Known(RuntimeEvent::with_timestamp(
            8,
            None,
            RuntimeEventKind::InputDequeued {
                input_id: "input-a".to_string(),
            },
        )),
    };
    assert!(serde_json::to_string(&mismatched).is_err());

    let raw_known = r#"{
        "schema_version": 1,
        "owner": {
            "workspace_id": "workspace-a",
            "project_id": "project-a",
            "lane_id": null,
            "session_id": null,
            "task_id": null,
            "turn_id": null
        },
        "cursor": {"stream_id": "stream-a", "sequence": 7},
        "event": {
            "sequence": 8,
            "timestamp": null,
            "kind": {"type": "input_dequeued", "payload": {"input_id": "input-a"}}
        }
    }"#;
    assert!(serde_json::from_str::<RuntimeEventEnvelope>(raw_known).is_err());

    let raw_unknown = raw_known.replace("input_dequeued", "future_event");
    let unknown: RuntimeEventEnvelope = serde_json::from_str(&raw_unknown).unwrap();
    assert!(matches!(unknown.event, RuntimeWireEvent::Unknown { .. }));
}

#[test]
fn event_cursor_classifies_incoming_stream_order() {
    let current = EventCursor {
        stream_id: "stream-a".to_string(),
        sequence: 7,
    };

    assert_eq!(
        current.classify_incoming(&EventCursor {
            stream_id: "stream-a".to_string(),
            sequence: 7,
        }),
        EventCursorOrder::DuplicateOrOld
    );
    assert_eq!(
        current.classify_incoming(&EventCursor {
            stream_id: "stream-a".to_string(),
            sequence: 6,
        }),
        EventCursorOrder::DuplicateOrOld
    );
    assert_eq!(
        current.classify_incoming(&EventCursor {
            stream_id: "stream-a".to_string(),
            sequence: 8,
        }),
        EventCursorOrder::Next
    );
    assert_eq!(
        current.classify_incoming(&EventCursor {
            stream_id: "stream-a".to_string(),
            sequence: 9,
        }),
        EventCursorOrder::Gap
    );
    assert_eq!(
        current.classify_incoming(&EventCursor {
            stream_id: "stream-b".to_string(),
            sequence: 8,
        }),
        EventCursorOrder::StreamMismatch
    );
}

#[test]
fn runtime_commands_and_actions_roundtrip_json_without_ui_state() {
    let action = CommandAction {
        id: "mode.plan".to_string(),
        label: "Plan".to_string(),
        command: RuntimeCommand::SetWorkMode {
            mode: WorkMode::Plan,
        },
        enabled: true,
        disabled_reason: None,
        shortcut: Some("ctrl+p".to_string()),
        destructive: false,
    };

    let encoded = serde_json::to_value(&action).unwrap();
    assert_eq!(encoded["command"]["type"], "set_work_mode");
    assert_eq!(encoded["command"]["mode"], "plan");

    let decoded: CommandAction = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.command, action.command);
    assert!(decoded.enabled);
}

#[test]
fn agent_dag_runtime_command_roundtrips_json() {
    let commands = vec![
        RuntimeCommand::StartAgentDag {
            goal: "Complete role runtime".to_string(),
            tasks: vec![AgentDagTaskSpec {
                task_id: "task_planner".to_string(),
                role: AgentRole::Planner,
                title: "Plan implementation".to_string(),
                objective: "Split work into safe tasks".to_string(),
                dependencies: Vec::new(),
                workspace: None,
                file_scope: vec!["crates/runtime".to_string()],
                context_bundle_id: Some("ctx_planner".to_string()),
                required_evidence: vec!["plan".to_string()],
                permission_policy: "read_only".to_string(),
            }],
        },
        RuntimeCommand::AcceptMergeGate {
            gate_id: "gate-task_planner".to_string(),
            actor: RuntimeOwner::default(),
            reviewed_evidence: Vec::new(),
            decision: Some("required evidence complete".to_string()),
        },
        RuntimeCommand::RejectMergeGate {
            gate_id: "gate-task_planner".to_string(),
            actor: RuntimeOwner::default(),
            reason: "missing test evidence".to_string(),
        },
        RuntimeCommand::RecordAgentEvidence {
            gate_id: "gate-task_planner".to_string(),
            evidence_id: Some("manual-test_result".to_string()),
            kind: "test_result".to_string(),
            summary: "focused tests passed".to_string(),
            path: Some("target/test.log".to_string()),
            source: Some("tester".to_string()),
            canonical: None,
        },
        RuntimeCommand::AcceptAgentArtifact {
            gate_id: "gate-task_planner".to_string(),
            evidence_id: "evidence-task_planner-plan".to_string(),
            actor: RuntimeOwner::default(),
            source_hash: String::new(),
            decision: Some("artifact evidence accepted".to_string()),
        },
        RuntimeCommand::RejectAgentArtifact {
            gate_id: "gate-task_planner".to_string(),
            evidence_id: "evidence-task_planner-plan".to_string(),
            actor: RuntimeOwner::default(),
            reason: "artifact is stale".to_string(),
        },
        RuntimeCommand::MergeAgentPatch {
            gate_id: "gate-task_planner".to_string(),
            actor: RuntimeOwner::default(),
            decision: Some("merge accepted artifact".to_string()),
        },
    ];

    let encoded = serde_json::to_value(&commands).unwrap();
    assert_eq!(encoded[0]["type"], "start_agent_dag");
    assert_eq!(encoded[0]["tasks"][0]["role"], "planner");
    assert_eq!(encoded[1]["type"], "accept_merge_gate");
    assert_eq!(encoded[2]["type"], "reject_merge_gate");
    assert_eq!(encoded[3]["type"], "record_agent_evidence");
    assert_eq!(encoded[3]["kind"], "test_result");
    assert_eq!(encoded[4]["type"], "accept_agent_artifact");
    assert_eq!(encoded[5]["type"], "reject_agent_artifact");
    assert_eq!(encoded[6]["type"], "merge_agent_patch");

    let decoded: Vec<RuntimeCommand> = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, commands);
}

#[test]
fn context_contracts_round_trip_without_exposing_storage_paths() {
    let handle = ContextHandleRecord {
        handle_id: "ctxh-1".into(),
        item_id: "ctxi-1".into(),
        preferred_view_id: Some("ctxv-1".into()),
        content_sha256: "ab".repeat(32),
        scope: ContextScope::Task("task-1".into()),
        expires_at: None,
    };
    let item = ContextItemRecord {
        item_id: "ctxi-1".into(),
        scope: ContextScope::Dag("dag-1".into()),
        kind: ContextContentKind::Diff,
        content_sha256: "cd".repeat(32),
        title: "runtime contract patch".into(),
        summary: "Diff summary only".into(),
        token_count: 120,
        evidence_id: Some("evidence-1".into()),
        created_at: Some(100),
    };
    let view_record = ContextViewRecord {
        view_id: "ctxv-1".into(),
        item_id: "ctxi-1".into(),
        kind: ContextContentKind::Text,
        derivation: "summary".into(),
        content_sha256: "ef".repeat(32),
        token_count: 24,
        quality_id: Some("ctxq-1".into()),
        created_at: Some(101),
    };
    let retrieval = ContextRetrievalRecord {
        retrieval_id: "ctxr-1".into(),
        handle_id: "ctxh-1".into(),
        item_id: "ctxi-1".into(),
        view_id: Some("ctxv-1".into()),
        scope: ContextScope::Task("task-1".into()),
        byte_count: 256,
        token_count: 64,
        reason_category: "hydrate".into(),
        permission_decision: "allow".into(),
        reason_rule_category: "safe_read".into(),
        reason: "answer follow-up".into(),
        requester: "runtime".into(),
        retrieved_at: Some(102),
    };
    let quality = ContextQualityRecord {
        quality_id: "ctxq-1".into(),
        target_id: "ctxv-1".into(),
        passed: true,
        score_microunits: Some(920_000),
        checks: vec!["sha256_match".into(), "evidence_present".into()],
        failure_reason: None,
        checked_at: Some(103),
    };
    let budget = ContextBudgetRecord {
        budget_id: "ctxb-1".into(),
        scope: ContextScope::Workflow("wf-1".into()),
        soft_token_limit: 8_000,
        hard_token_limit: 16_000,
        used_tokens: 1_200,
        remaining_tokens: 6_800,
        exceeded: false,
        updated_at: Some(104),
    };
    let cost = CostUsageRecord {
        usage_id: "cost-1".into(),
        provider_id: "deepseek".into(),
        model: "deepseek-reasoner".into(),
        scopes: vec![CostScope::AgentTask("task-1".into())],
        tokens: TokenUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(250),
            cached_input_tokens: Some(500),
            retrieval_tokens: Some(0),
            total_tokens: Some(1_250),
        },
        estimate: Some(CostEstimate {
            amount: CostAmount {
                currency: "USD".into(),
                micro_units: 1_234,
            },
            provider_id: "deepseek".into(),
            model: "deepseek-reasoner".into(),
            price_table_version: "test".into(),
            estimated: true,
        }),
        actual_cost: None,
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(105),
    };

    let handle_json = serde_json::to_value(&handle).unwrap();
    assert_eq!(handle_json["scope"]["type"], "task");
    assert!(handle_json.get("storage_path").is_none());
    assert_eq!(
        serde_json::from_value::<ContextHandleRecord>(handle_json).unwrap(),
        handle
    );

    let records = serde_json::json!({
        "item": item,
        "view": view_record,
        "retrieval": retrieval,
        "quality": quality,
        "budget": budget,
        "cost": cost,
    });
    assert_eq!(records["item"]["kind"], "diff");
    assert_eq!(records["view"]["kind"], "text");
    assert_eq!(records["retrieval"]["reason"], "answer follow-up");
    assert_eq!(records["retrieval"]["scope"]["type"], "task");
    assert_eq!(records["retrieval"]["byte_count"], 256);
    assert_eq!(records["retrieval"]["reason_category"], "hydrate");
    assert_eq!(records["retrieval"]["permission_decision"], "allow");
    assert_eq!(records["retrieval"]["reason_rule_category"], "safe_read");
    assert_eq!(records["quality"]["score_microunits"], 920_000);
    assert_eq!(records["budget"]["scope"]["type"], "workflow");
    assert_eq!(records["cost"]["actual_cost"], serde_json::Value::Null);
    assert!(
        !serde_json::to_string(&records)
            .unwrap()
            .contains("storage_path")
    );
}

#[test]
fn context_and_cost_runtime_events_project_bounded_summaries() {
    let handle = ContextHandleRecord {
        handle_id: "ctxh-1".into(),
        item_id: "ctxi-1".into(),
        preferred_view_id: Some("ctxv-1".into()),
        content_sha256: "ab".repeat(32),
        scope: ContextScope::Task("task-1".into()),
        expires_at: None,
    };
    let item = ContextItemRecord {
        item_id: "ctxi-1".into(),
        scope: ContextScope::Task("task-1".into()),
        kind: ContextContentKind::Code,
        content_sha256: "cd".repeat(32),
        title: "runtime.rs".into(),
        summary: "Runtime contract definitions".into(),
        token_count: 320,
        evidence_id: None,
        created_at: Some(200),
    };
    let view_record = ContextViewRecord {
        view_id: "ctxv-1".into(),
        item_id: "ctxi-1".into(),
        kind: ContextContentKind::Text,
        derivation: "bounded_summary".into(),
        content_sha256: "ef".repeat(32),
        token_count: 80,
        quality_id: Some("ctxq-1".into()),
        created_at: Some(201),
    };
    let retrieval = ContextRetrievalRecord {
        retrieval_id: "ctxr-1".into(),
        handle_id: "ctxh-1".into(),
        item_id: "ctxi-1".into(),
        view_id: Some("ctxv-1".into()),
        scope: ContextScope::Task("task-1".into()),
        byte_count: 512,
        token_count: 80,
        reason_category: "hydrate".into(),
        permission_decision: "allow".into(),
        reason_rule_category: "safe_read".into(),
        reason: "hydrate evidence".into(),
        requester: "runtime".into(),
        retrieved_at: Some(202),
    };
    let budget = ContextBudgetRecord {
        budget_id: "ctxb-1".into(),
        scope: ContextScope::Task("task-1".into()),
        soft_token_limit: 1_000,
        hard_token_limit: 1_200,
        used_tokens: 1_300,
        remaining_tokens: 0,
        exceeded: true,
        updated_at: Some(203),
    };
    let quality = ContextQualityRecord {
        quality_id: "ctxq-1".into(),
        target_id: "ctxv-1".into(),
        passed: false,
        score_microunits: Some(400_000),
        checks: vec!["missing_canonical_evidence".into()],
        failure_reason: Some("canonical evidence was not attached".into()),
        checked_at: Some(204),
    };
    let cost = CostUsageRecord {
        usage_id: "cost-1".into(),
        provider_id: "deepseek".into(),
        model: "deepseek-reasoner".into(),
        scopes: vec![CostScope::AgentTask("task-1".into())],
        tokens: TokenUsage {
            input_tokens: Some(700),
            output_tokens: Some(300),
            cached_input_tokens: Some(200),
            retrieval_tokens: Some(0),
            total_tokens: Some(1_000),
        },
        estimate: Some(CostEstimate {
            amount: CostAmount {
                currency: "USD".into(),
                micro_units: 900,
            },
            provider_id: "deepseek".into(),
            model: "deepseek-reasoner".into(),
            price_table_version: "test".into(),
            estimated: true,
        }),
        actual_cost: Some(CostAmount {
            currency: "USD".into(),
            micro_units: 950,
        }),
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(205),
    };
    let events = vec![
        RuntimeEvent::new(
            1,
            RuntimeEventKind::ContextBundleBuilt {
                bundle_id: "bundle-1".into(),
                scope: ContextScope::Task("task-1".into()),
                handle_ids: vec![handle.handle_id.clone()],
                estimated_tokens: 400,
            },
        ),
        RuntimeEvent::new(2, RuntimeEventKind::ContextItemStored { item }),
        RuntimeEvent::new(
            3,
            RuntimeEventKind::ContextViewDerived {
                view: view_record,
                handle: handle.clone(),
            },
        ),
        RuntimeEvent::new(4, RuntimeEventKind::ContextRetrieved { retrieval }),
        RuntimeEvent::new(5, RuntimeEventKind::ContextBudgetExceeded { budget }),
        RuntimeEvent::new(6, RuntimeEventKind::ContextQualityFailed { quality }),
        RuntimeEvent::new(7, RuntimeEventKind::CostUsageRecorded { cost }),
        RuntimeEvent::new(
            8,
            RuntimeEventKind::ProviderCacheObserved {
                provider_id: "deepseek".into(),
                model: "deepseek-reasoner".into(),
                cached_input_tokens: 200,
                cache_hit_microunits: 160_000,
            },
        ),
        RuntimeEvent::new(
            9,
            RuntimeEventKind::EvidenceCanonicalized {
                evidence_id: "evidence-1".into(),
                item_id: "ctxi-1".into(),
                content_sha256: "cd".repeat(32),
            },
        ),
    ];

    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());
    for event in &events {
        view.apply_event(event);
    }

    assert_eq!(
        serde_json::to_value(&events[0]).unwrap()["kind"]["type"],
        "context_bundle_built"
    );
    assert_eq!(
        serde_json::to_value(RuntimeCommand::RetrieveContext {
            handle_id: handle.handle_id.clone(),
            reason: "answer follow-up".into(),
        })
        .unwrap()["type"],
        "retrieve_context"
    );
    assert_eq!(view.context_handles[0], handle);
    assert_eq!(view.context_items[0].item_id, "ctxi-1");
    assert_eq!(view.context_views[0].view_id, "ctxv-1");
    assert_eq!(view.context_retrievals[0].handle_id, "ctxh-1");
    assert_eq!(view.context_budgets[0].used_tokens, 1_300);
    assert!(!view.context_quality[0].passed);
    assert_eq!(view.cost_ledger.total_tokens, 1_000);
    assert_eq!(view.cost_ledger.total_estimated_cost_micro_usd, 900);
    assert_eq!(view.cost_ledger.total_actual_cost_micro_usd, Some(950));
    assert_eq!(view.provider_cache_observations[0].cached_input_tokens, 200);
    assert_eq!(view.canonical_evidence[0].evidence_id, "evidence-1");

    for index in 0..55 {
        view.apply_event(&RuntimeEvent::new(
            100 + index,
            RuntimeEventKind::ContextRetrieved {
                retrieval: ContextRetrievalRecord {
                    retrieval_id: format!("ctxr-extra-{index}"),
                    handle_id: "ctxh-1".into(),
                    item_id: "ctxi-1".into(),
                    view_id: Some("ctxv-1".into()),
                    scope: ContextScope::Task("task-1".into()),
                    byte_count: 64,
                    token_count: 16,
                    reason_category: "replay".into(),
                    permission_decision: "allow".into(),
                    reason_rule_category: "safe_read".into(),
                    reason: "bounded replay".into(),
                    requester: "runtime".into(),
                    retrieved_at: Some(300 + index),
                },
            },
        ));
    }
    assert_eq!(view.context_retrievals.len(), 50);
    assert_eq!(view.context_retrievals[0].retrieval_id, "ctxr-extra-5");

    let public_json = serde_json::to_string(&events).unwrap()
        + &serde_json::to_string(&view.context_bundles).unwrap()
        + &serde_json::to_string(&view.context_handles).unwrap()
        + &serde_json::to_string(&view.context_items).unwrap()
        + &serde_json::to_string(&view.context_views).unwrap()
        + &serde_json::to_string(&view.context_retrievals).unwrap()
        + &serde_json::to_string(&view.context_budgets).unwrap()
        + &serde_json::to_string(&view.context_quality).unwrap()
        + &serde_json::to_string(&view.cost_usage).unwrap()
        + &serde_json::to_string(&view.provider_cache_observations).unwrap()
        + &serde_json::to_string(&view.canonical_evidence).unwrap();
    assert!(!public_json.contains("storage_path"));
    assert!(!public_json.contains("/tmp/viden"));
}

#[test]
fn runtime_view_state_does_not_double_count_duplicate_cost_usage_id() {
    let cost = CostUsageRecord {
        usage_id: "provider-attempt-1".into(),
        provider_id: "deepseek".into(),
        model: "deepseek-v4-flash".into(),
        scopes: vec![
            CostScope::Request("provider-attempt-1".into()),
            CostScope::AgentTask("task-1".into()),
            CostScope::Workflow("wf-1".into()),
        ],
        tokens: TokenUsage {
            input_tokens: Some(10),
            output_tokens: Some(5),
            cached_input_tokens: Some(3),
            retrieval_tokens: Some(0),
            total_tokens: Some(15),
        },
        estimate: None,
        actual_cost: None,
        attempt_index: 0,
        outcome: CostUsageOutcome::Success,
        recorded_at: Some(500),
    };
    let events = vec![
        RuntimeEvent::new(
            1,
            RuntimeEventKind::CostUsageRecorded { cost: cost.clone() },
        ),
        RuntimeEvent::new(2, RuntimeEventKind::CostUsageRecorded { cost }),
    ];
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());

    for event in &events {
        view.apply_event(event);
    }

    assert_eq!(view.cost_usage.len(), 1);
    assert_eq!(view.cost_ledger.input_tokens, 10);
    assert_eq!(view.cost_ledger.output_tokens, 5);
    assert_eq!(view.cost_ledger.cached_input_tokens, 3);
    assert_eq!(view.cost_ledger.total_tokens, 15);
}

#[test]
fn legacy_flat_cost_usage_events_replay_with_unknown_actual_preserved() {
    let fixture = include_str!("../tests/fixtures/runtime-contract-legacy-cost.json");
    let events: Vec<RuntimeEvent> = serde_json::from_str(fixture).unwrap();
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());

    for event in &events {
        view.apply_event(event);
    }

    assert_eq!(view.cost_usage.len(), 3);
    assert_ne!(view.cost_usage[0].usage_id, view.cost_usage[1].usage_id);
    assert_ne!(view.cost_usage[1].usage_id, view.cost_usage[2].usage_id);
    assert!(view.cost_usage[1].usage_id.contains("legacy-request-2"));
    assert!(view.cost_usage[2].usage_id.contains("legacy-request-3"));
    assert_eq!(view.cost_ledger.input_tokens, 50);
    assert_eq!(view.cost_ledger.output_tokens, 19);
    assert_eq!(view.cost_ledger.cached_input_tokens, 5);
    assert_eq!(view.cost_ledger.retrieval_tokens, 0);
    assert_eq!(view.cost_ledger.total_tokens, 69);
    assert_eq!(view.cost_ledger.total_estimated_cost_micro_usd, 1100);
    assert_eq!(view.cost_ledger.total_actual_cost_micro_usd, None);
    assert_eq!(view.cost_usage[0].attempt_index, 0);
    assert_eq!(view.cost_usage[0].outcome, CostUsageOutcome::Success);
    assert!(
        view.cost_usage[0]
            .scopes
            .contains(&CostScope::AgentTask("legacy-task".into()))
    );
    assert!(
        !view.cost_usage[0]
            .scopes
            .iter()
            .any(|scope| matches!(scope, CostScope::Request(_)))
    );
    assert!(
        view.cost_usage[1]
            .scopes
            .contains(&CostScope::Request("legacy-request-2".into()))
    );
    assert!(
        view.cost_usage[1]
            .scopes
            .contains(&CostScope::Workflow("legacy-workflow".into()))
    );
    assert_eq!(
        view.cost_usage[0]
            .estimate
            .as_ref()
            .unwrap()
            .price_table_version,
        "legacy-flat-cost-v1"
    );

    let serialized = serde_json::to_value(&view.cost_usage[0]).unwrap();
    assert!(serialized.get("scope").is_none());
    assert!(serialized.get("input_tokens").is_none());
    assert!(serialized.get("estimated_cost_micro_usd").is_none());
    assert!(serialized.get("scopes").is_some());
    assert!(serialized.get("tokens").is_some());
}

#[test]
fn legacy_cost_usage_rejects_ambiguous_or_malformed_shapes_without_raw_payload() {
    let ambiguous = serde_json::json!({
        "usage_id": "ambiguous",
        "provider_id": "deepseek",
        "model": "deepseek",
        "scope": {"type": "task", "id": "legacy-task"},
        "scopes": [{"type": "request", "id": "new-request"}],
        "input_tokens": 1,
        "output_tokens": 1,
        "estimated_cost_micro_usd": 1,
        "tokens": {"input_tokens": 1, "output_tokens": 1, "cached_input_tokens": 0, "retrieval_tokens": 0, "total_tokens": 2},
        "recorded_at": 1
    });
    let err = serde_json::from_value::<CostUsageRecord>(ambiguous)
        .expect_err("ambiguous legacy/new shape must be rejected")
        .to_string();
    assert!(err.contains("ambiguous cost usage"));
    assert!(!err.contains("sk-"));
    assert!(!err.contains("legacy-task"));

    let malformed = serde_json::json!({
        "provider_id": "deepseek",
        "model": "deepseek",
        "scope": {"type": "task", "id": "sk-secret-task"},
        "input_tokens": 1
    });
    let err = serde_json::from_value::<CostUsageRecord>(malformed)
        .expect_err("incomplete legacy shape must be rejected")
        .to_string();
    assert!(err.contains("malformed legacy cost usage"));
    assert!(!err.contains("sk-secret-task"));
}

#[test]
fn runtime_events_replay_into_ui_independent_view_state() {
    let snapshot = runtime_snapshot_for_contract();
    let mut view = RuntimeViewState::new(RuntimeSnapshot {
        provider_family: "fallback".to_string(),
        model_label: "test-local".to_string(),
        ..snapshot.clone()
    });

    let approval = ApprovalRequestView {
        id: "approval_1".to_string(),
        tool_name: "shell".to_string(),
        title: "Run cargo test".to_string(),
        message: "Shell command requires approval".to_string(),
        input_preview: "cargo test -p viden-types".to_string(),
        is_mutating: false,
        reason: Some("permission level is ask".to_string()),
        owner: RuntimeOwner::default(),
        risk: ApprovalRisk::Medium,
        target: ApprovalTarget {
            kind: "shell".to_string(),
            display: "cargo test -p viden-types".to_string(),
            canonical_ref: None,
        },
        allowed_scopes: vec![ApprovalScope::Once],
        policy_reason_key: "permission.requires_approval".to_string(),
        policy_reason_args: std::collections::BTreeMap::new(),
        expires_at: 1,
        default_action: ApprovalDefaultAction::Deny,
        audit_id: "audit_1".to_string(),
    };
    let evidence = EvidenceView {
        id: "evidence_1".to_string(),
        kind: "test".to_string(),
        summary: "viden-types tests passed".to_string(),
        path: Some("target/test.log".to_string()),
        source: Some("cargo".to_string()),
        canonical: None,
        metadata: None,
        timestamp: Some(42),
    };
    let task = AgentTaskRecord {
        id: "task_1".to_string(),
        parent_id: None,
        role: AgentRole::Planner,
        kind: AgentTaskKind::Agent,
        route: AgentRoute::BuiltIn,
        title: "Build runtime contract".to_string(),
        status: AgentTaskStatus::Thinking,
        activity: "designing contract".to_string(),
        summary: "phase 0 contract".to_string(),
        progress: 15,
        started_at: Some(1),
        updated_at: Some(2),
        workspace: Some("/tmp/viden".to_string()),
        evidence: vec![evidence.id.clone()],
        permissions: Vec::new(),
        decision: None,
        result: None,
        resume_handle: None,
        pid: None,
        next_action: None,
    };
    let queued = QueuedInputView {
        id: "queued_1".to_string(),
        content_preview: "follow-up question".to_string(),
        created_at: Some(43),
    };

    let events = vec![
        RuntimeEvent::new(
            1,
            RuntimeEventKind::SnapshotUpdated {
                snapshot: snapshot.clone(),
            },
        ),
        RuntimeEvent::new(
            2,
            RuntimeEventKind::AssistantDelta {
                message_id: "msg_1".to_string(),
                task_id: Some(task.id.clone()),
                content: "Working on the contract.".to_string(),
            },
        ),
        RuntimeEvent::new(3, RuntimeEventKind::ApprovalRequested { approval }),
        RuntimeEvent::new(4, RuntimeEventKind::EvidenceRecorded { evidence }),
        RuntimeEvent::new(5, RuntimeEventKind::TaskUpdated { task }),
        RuntimeEvent::new(
            6,
            RuntimeEventKind::InputQueued {
                input: queued.clone(),
            },
        ),
        RuntimeEvent::new(
            7,
            RuntimeEventKind::InputDequeued {
                input_id: queued.id.clone(),
            },
        ),
        RuntimeEvent::new(
            8,
            RuntimeEventKind::ApprovalResolved {
                request_id: "approval_1".to_string(),
                decision: ApprovalDecision::Allow {
                    scope: ApprovalScope::Once,
                },
                owner: RuntimeOwner::default(),
                audit_id: "audit_1".to_string(),
            },
        ),
    ];

    for event in &events {
        view.apply_event(event);
    }

    assert_eq!(view.snapshot.provider_family, "deepseek");
    assert_eq!(view.assistant_stream, "Working on the contract.");
    assert!(view.pending_approvals.is_empty());
    assert!(view.queued_inputs.is_empty());
    assert_eq!(view.latest_evidence[0].summary, "viden-types tests passed");
    assert_eq!(view.tasks[0].status_kind(), AgentTaskStatus::Thinking);

    let encoded = serde_json::to_string(&events).unwrap();
    let decoded: Vec<RuntimeEvent> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, events);
}

#[test]
fn runtime_view_state_replays_agent_dag_and_merge_gate_events() {
    let snapshot = runtime_snapshot_for_contract();
    let dag = AgentDagRecord {
        dag_id: "dag_runtime".to_string(),
        goal: "Coordinate implementation".to_string(),
        status: AgentDagStatus::Active,
        tasks: vec![AgentDagTaskSpec {
            task_id: "task_runtime_planner".to_string(),
            role: AgentRole::Planner,
            title: "Plan work".to_string(),
            objective: "Split implementation".to_string(),
            dependencies: Vec::new(),
            workspace: Some("/tmp/viden".to_string()),
            file_scope: vec!["crates/runtime".to_string()],
            context_bundle_id: Some("ctx_runtime_planner".to_string()),
            required_evidence: vec!["plan".to_string()],
            permission_policy: "read_only".to_string(),
        }],
        created_at: Some(1),
        updated_at: Some(1),
    };
    let gate = MergeGateRecord {
        gate_id: "gate_runtime".to_string(),
        task_id: "task_runtime_planner".to_string(),
        status: MergeGateStatus::Proposed,
        required_evidence: vec!["plan".to_string()],
        evidence_ids: Vec::new(),
        gate_type: MergeGateType::Artifact,
        owner: RuntimeOwner::default(),
        validator: None,
        policy_snapshot: MergeGatePolicySnapshot::default(),
        decision: None,
        conflict: None,
        applied_change_id: None,
        recovery_snapshot: None,
        audit_ids: Vec::new(),
        updated_at: Some(2),
    };
    let mut view = RuntimeViewState::new(snapshot);

    view.apply_event(&RuntimeEvent::new(
        1,
        RuntimeEventKind::AgentDagUpdated { dag: dag.clone() },
    ));
    view.apply_event(&RuntimeEvent::new(
        2,
        RuntimeEventKind::MergeGateUpdated { gate: gate.clone() },
    ));

    assert_eq!(view.agent_dags[0], dag);
    assert_eq!(view.merge_gates[0], gate);
}

#[test]
fn runtime_view_state_sanitizes_context_reduction_records_from_raw_jsonl() {
    let raw = r#"
    {
      "sequence": 1,
      "timestamp": 1,
      "kind": {
        "type": "context_reduction_recorded",
        "payload": {
          "reduction": {
            "reduction_id": "ctxr/../../bad",
            "item_id": "ctxi_/Users/wiki/private/sk-test-secret",
            "view_id": "ctxv_/Users/wiki/private",
            "reducer_id": "adapter/../../sk-test-secret",
            "reducer_version": "../0.1.0",
            "status": "timeout\n/Users/wiki/private",
            "reason": "stderr /Users/wiki/private sk-test-secret ".repeat(10),
            "fallback": true,
            "host_latency_ms": 999999999999,
            "created_at": 1
          }
        }
      }
    }
    "#;
    let raw = raw.replace(
        "\"stderr /Users/wiki/private sk-test-secret \".repeat(10)",
        &format!(
            "{:?}",
            "stderr /Users/wiki/private sk-test-secret ".repeat(10)
        ),
    );
    let event: RuntimeEvent = serde_json::from_str(&raw).unwrap();
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());

    view.apply_event(&event);

    let reduction = view.context_reductions.first().unwrap();
    assert_eq!(
        reduction.reducer_id,
        "adapter_.._.._sk_redacted_test-secret"
    );
    assert_eq!(reduction.reducer_version, ".._0.1.0");
    assert_eq!(reduction.status, "timeout__Users_wiki_private");
    assert!(reduction.reason.as_ref().unwrap().len() <= 160);
    let replayed = serde_json::to_string(&view.context_reductions).unwrap();
    assert!(!replayed.contains("/Users/"));
    assert!(!replayed.contains("sk-test-secret "));
}

#[test]
fn runtime_contract_fixture_replays_phase2_cross_frontend_facts() {
    let fixture = include_str!("../tests/fixtures/runtime-contract-phase2.json");
    let events: Vec<RuntimeEvent> = serde_json::from_str(fixture).unwrap();
    let mut view = RuntimeViewState::new(runtime_snapshot_for_contract());

    for event in &events {
        view.apply_event(event);
    }

    assert_eq!(view.snapshot.provider_family, "deepseek");
    assert_eq!(view.snapshot.model_label, "deepseek-reasoner");
    assert_eq!(view.snapshot.work_mode, WorkMode::Review);
    assert_eq!(view.provider.as_ref().unwrap().model, "deepseek-reasoner");
    assert_eq!(view.token_cost.as_ref().unwrap().total_tokens, 3456);
    assert_eq!(view.token_cost.as_ref().unwrap().cost_micro_usd, Some(1234));
    assert_eq!(view.context_handles[0].handle_id, "ctxh_runtime_1");
    assert_eq!(view.context_items[0].kind, ContextContentKind::Code);
    assert_eq!(view.context_views[0].derivation, "bounded_summary");
    assert_eq!(view.context_bundles[0].bundle_id, "ctx_bundle_runtime_2");
    assert_eq!(view.context_retrievals[0].retrieval_id, "ctxr_runtime_1");
    assert!(view.context_budgets[0].exceeded);
    assert!(!view.context_quality[0].passed);
    assert_eq!(view.cost_ledger.total_tokens, 3456);
    assert_eq!(view.provider_cache_observations[0].cached_input_tokens, 600);
    assert_eq!(view.canonical_evidence[0].item_id, "ctxi_runtime_1");
    assert!(!fixture.contains("storage_path"));
    assert!(view.pending_approvals.is_empty());
    assert!(view.active_tool_calls.is_empty());
    assert_eq!(view.tasks[0].id, "task_runtime_1");
    assert_eq!(view.lanes[0].id, "lane_runtime_1");
    assert!(view.latest_evidence.iter().any(|evidence| {
        evidence.kind == "test" && evidence.summary.contains("workspace tests passed")
    }));
    assert!(matches!(
        view.last_command.as_ref().map(|receipt| &receipt.command),
        Some(RuntimeCommand::SelectModel { provider_id, model })
            if provider_id == "deepseek" && model == "deepseek-reasoner"
    ));
}
