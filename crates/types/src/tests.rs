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
    assert_eq!(decoded_task.agent, "shell");
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
            canonical: None,
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
