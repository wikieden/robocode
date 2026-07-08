use super::*;

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
        agent: "shell".to_string(),
        kind: "lane".to_string(),
        transport: "template".to_string(),
        title: "cargo test".to_string(),
        status: AgentTaskStatus::Running.as_str().to_string(),
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
    assert_eq!(decoded_task.agent, "shell");
    assert_eq!(decoded_bundle.sources[0].kind, "test");
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
        decision: None,
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
    }
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
            decision: Some("required evidence complete".to_string()),
        },
        RuntimeCommand::RejectMergeGate {
            gate_id: "gate-task_planner".to_string(),
            reason: "missing test evidence".to_string(),
        },
        RuntimeCommand::RecordAgentEvidence {
            gate_id: "gate-task_planner".to_string(),
            evidence_id: Some("manual-test_result".to_string()),
            kind: "test_result".to_string(),
            summary: "focused tests passed".to_string(),
            path: Some("target/test.log".to_string()),
            source: Some("tester".to_string()),
        },
        RuntimeCommand::AcceptAgentArtifact {
            gate_id: "gate-task_planner".to_string(),
            evidence_id: "evidence-task_planner-plan".to_string(),
            decision: Some("artifact evidence accepted".to_string()),
        },
        RuntimeCommand::RejectAgentArtifact {
            gate_id: "gate-task_planner".to_string(),
            evidence_id: "evidence-task_planner-plan".to_string(),
            reason: "artifact is stale".to_string(),
        },
        RuntimeCommand::MergeAgentPatch {
            gate_id: "gate-task_planner".to_string(),
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
    };
    let evidence = EvidenceView {
        id: "evidence_1".to_string(),
        kind: "test".to_string(),
        summary: "viden-types tests passed".to_string(),
        path: Some("target/test.log".to_string()),
        source: Some("cargo".to_string()),
        metadata: None,
        timestamp: Some(42),
    };
    let task = AgentTaskRecord {
        id: "task_1".to_string(),
        parent_id: None,
        agent: "planner".to_string(),
        kind: "runtime".to_string(),
        transport: "core".to_string(),
        title: "Build runtime contract".to_string(),
        status: AgentTaskStatus::Thinking.as_str().to_string(),
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
                approved: true,
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
        decision: None,
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
