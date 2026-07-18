use std::fs;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use viden_context::{ContextEngine, ContextPutRequest};
use viden_provider::ModelProvider;
use viden_types::{
    AgentDagTaskSpec, AgentRole, ApprovalResponse, ContextContentKind, ContextHandleRecord,
    ContextScope, EvidenceView, ModelEvent, ModelRequest, PermissionBehavior, PermissionLevel,
    PermissionRule, PermissionRuleSource, PermissionRuleValue, RuntimeCommand, RuntimeEvent,
    RuntimeEventKind, RuntimeViewState, ToolCall, ToolInput, WorkMode,
};

use crate::{EngineEvent, SessionEngine, context_bundle::ContextBuildMode};

use super::{SequenceProvider, temp_dir};

struct CountingProvider {
    request_count: Arc<AtomicU64>,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
    result: Result<Vec<ModelEvent>, String>,
}

impl CountingProvider {
    fn new(
        request_count: Arc<AtomicU64>,
        requests: Arc<Mutex<Vec<ModelRequest>>>,
        result: Result<Vec<ModelEvent>, String>,
    ) -> Self {
        Self {
            request_count,
            requests,
            result,
        }
    }
}

impl ModelProvider for CountingProvider {
    fn provider_name(&self) -> &str {
        "counting"
    }

    fn model(&self) -> &str {
        "counting-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap().push(request.clone());
        self.result.clone()
    }
}

fn assert_strictly_increasing_sequences(events: &[viden_types::RuntimeEvent]) {
    for pair in events.windows(2) {
        assert!(
            pair[0].sequence < pair[1].sequence,
            "runtime event sequence must increase: {} then {}",
            pair[0].sequence,
            pair[1].sequence
        );
    }
}

#[test]
fn core_exports_runtime_view_state_without_tui_dependencies() {
    let cwd = temp_dir("runtime_contract_cwd");
    let home = temp_dir("runtime_contract_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "hello from runtime".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let engine_events = engine
        .process_input_with_approval("say hello", &mut approver)
        .unwrap();
    let runtime_events = engine.runtime_events_for_engine_events(&engine_events);
    assert_strictly_increasing_sequences(&runtime_events);

    assert!(runtime_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.provider_family == "sequence"
                    && snapshot.model_label == "test-model"
        )
    }));
    assert!(runtime_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content.contains("hello from runtime")
        )
    }));
    assert!(runtime_events.iter().any(|event| {
        matches!(&event.kind, RuntimeEventKind::TaskUpdated { task } if task.kind == "provider")
    }));
    assert!(
        runtime_events
            .iter()
            .any(|event| matches!(&event.kind, RuntimeEventKind::ProviderHealthUpdated { .. }))
    );

    let mut view = RuntimeViewState::new(engine.runtime_snapshot());
    for event in &runtime_events {
        view.apply_event(event);
    }

    assert_eq!(view.snapshot.provider_family, "sequence");
    assert!(view.assistant_stream.contains("hello from runtime"));
    assert!(view.provider.is_some());
    assert!(view.tasks.iter().any(|task| task.kind == "provider"));
}

#[test]
fn core_runtime_bridge_records_tool_calls_and_results() {
    let cwd = temp_dir("runtime_contract_tool_cwd");
    let home = temp_dir("runtime_contract_tool_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf hello".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_1".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_engine_root_for_test(cwd.join(".viden/private-context-test"));
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let engine_events = engine
        .process_input_with_approval("run printf", &mut approver)
        .unwrap();
    assert!(engine_events.iter().any(
        |event| matches!(event, EngineEvent::ToolResult { output, .. } if output.contains("hello"))
    ));

    let runtime_events = engine.runtime_events_for_engine_events(&engine_events);
    assert_strictly_increasing_sequences(&runtime_events);
    assert!(runtime_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallStarted { name, .. } if name == "shell"
        )
    }));
    assert!(runtime_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                success: true,
                evidence: Some(evidence),
                ..
            } if evidence.summary.contains("hello")
        )
    }));
}

#[test]
fn core_runtime_bridge_does_not_fail_successful_tool_output_with_error_words() {
    let cwd = temp_dir("runtime_contract_tool_error_word_cwd");
    let home = temp_dir("runtime_contract_tool_error_word_home");
    let mut input = ToolInput::new();
    input.insert(
        "command".to_string(),
        "printf 'Error format documentation'".to_string(),
    );
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_error_word".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let engine_events = engine
        .process_input_with_approval("print help text", &mut approver)
        .unwrap();
    let runtime_events = engine.runtime_events_for_engine_events(&engine_events);

    assert!(runtime_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                success: true,
                evidence: Some(evidence),
                ..
            } if evidence.summary.contains("Error format documentation")
        )
    }));
}

#[test]
fn runtime_command_bus_switches_mode_and_submits_input() {
    let cwd = temp_dir("runtime_command_bus_cwd");
    let home = temp_dir("runtime_command_bus_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "planned response".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let mode_events = engine
        .handle_runtime_command(
            "cmd_mode",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
            &mut approver,
        )
        .unwrap();
    assert_strictly_increasing_sequences(&mode_events);

    assert!(mode_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_mode"
        )
    }));
    assert!(mode_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Plan
                    && snapshot.permission_level == PermissionLevel::ReadOnly
        )
    }));

    let input_events = engine
        .handle_runtime_command(
            "cmd_input",
            RuntimeCommand::SubmitUserInput {
                content: "write a plan".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert_strictly_increasing_sequences(&input_events);

    assert!(input_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_input"
        )
    }));
    assert!(input_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content.contains("planned response")
        )
    }));
}

#[test]
fn runtime_command_bus_covers_plan_build_review_permission_contract() {
    let cwd = temp_dir("runtime_command_mode_contract_cwd");
    let home = temp_dir("runtime_command_mode_contract_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    for (command_id, mode) in [
        ("cmd_plan", WorkMode::Plan),
        ("cmd_review", WorkMode::Review),
        ("cmd_explore", WorkMode::Explore),
    ] {
        let events = engine
            .handle_runtime_command(
                command_id,
                RuntimeCommand::SetWorkMode { mode },
                &mut approver,
            )
            .unwrap();

        assert_strictly_increasing_sequences(&events);
        assert!(events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == mode
                        && snapshot.permission_level == PermissionLevel::ReadOnly
            )
        }));
    }

    let build_events = engine
        .handle_runtime_command(
            "cmd_build",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Build,
            },
            &mut approver,
        )
        .unwrap();

    assert_strictly_increasing_sequences(&build_events);
    assert!(build_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Build
                    && snapshot.permission_level == PermissionLevel::Ask
        )
    }));
}

#[test]
fn runtime_command_bus_emits_approval_events_for_gated_tools() {
    let cwd = temp_dir("runtime_command_approval_cwd");
    let home = temp_dir("runtime_command_approval_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf approved".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_needs_approval".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .handle_runtime_command(
            "cmd_approval",
            RuntimeCommand::SubmitUserInput {
                content: "run approved command".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert_strictly_increasing_sequences(&events);
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalRequested { approval }
                if approval.tool_name == "shell"
                    && approval.input_preview.contains("printf approved")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ApprovalResolved { approved: true, .. }
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                success: true,
                evidence: Some(evidence),
                ..
            } if evidence.summary.contains("approved")
        )
    }));
}

#[test]
fn runtime_command_bus_queues_follow_up_input() {
    let cwd = temp_dir("runtime_command_queue_cwd");
    let home = temp_dir("runtime_command_queue_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .handle_runtime_command(
            "cmd_queue",
            RuntimeCommand::QueueFollowUp {
                content: "continue with tests".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert_strictly_increasing_sequences(&events);
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_queue"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::InputQueued { input }
                if input.content_preview == "continue with tests"
        )
    }));

    let view = engine.runtime_view_state();
    assert_eq!(view.queued_inputs.len(), 1);
    assert_eq!(view.queued_inputs[0].content_preview, "continue with tests");
}

#[test]
fn hard_context_limit_rejects_before_provider_request() {
    let cwd = temp_dir("runtime_contract_hard_budget_cwd");
    let home = temp_dir("runtime_contract_hard_budget_home");
    let request_count = Arc::new(AtomicU64::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(CountingProvider::new(
        Arc::clone(&request_count),
        Arc::clone(&requests),
        Ok(vec![ModelEvent::AssistantText {
            content: "should not be called".to_string(),
        }]),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(10, 20);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .handle_runtime_command(
            "cmd_hard_budget",
            RuntimeCommand::SubmitUserInput {
                content: "required evidence marker ".repeat(100),
            },
            &mut approver,
        )
        .unwrap();

    assert_eq!(request_count.load(Ordering::SeqCst), 0);
    assert!(requests.lock().unwrap().is_empty());
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextBudgetExceeded { budget }
                if budget.exceeded
                    && budget.soft_token_limit == 10
                    && budget.hard_token_limit == 20
                    && budget.used_tokens > budget.hard_token_limit
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_hard_budget"
                    && reason.contains("context hard limit")
                    && reason.contains("reduce input")
        )
    }));
}

#[test]
fn soft_context_budget_evicts_low_priority_sources_before_provider_request() {
    let cwd = temp_dir("runtime_contract_soft_budget_cwd");
    let home = temp_dir("runtime_contract_soft_budget_home");
    let request_count = Arc::new(AtomicU64::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(CountingProvider::new(
        Arc::clone(&request_count),
        Arc::clone(&requests),
        Ok(vec![ModelEvent::AssistantText {
            content: "soft budget ok".to_string(),
        }]),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(100, 1_000);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval(
            &format!("/memory add {}", "low-priority-memory ".repeat(300)),
            &mut approver,
        )
        .unwrap();
    let events = engine
        .handle_runtime_command(
            "cmd_soft_budget",
            RuntimeCommand::SubmitUserInput {
                content: "short task".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert_eq!(request_count.load(Ordering::SeqCst), 1);
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextUpdated { context }
                if context.estimated_tokens <= context.soft_token_budget
                    && context.hard_token_limit == 1_000
                    && context.omitted_sources.iter().any(|source| source.name == "memory")
                    && context.sources.iter().any(|source| source.name == "user-task")
        )
    }));
}

#[test]
fn context_engine_events_replay_without_raw_secret_or_paths() {
    let cwd = temp_dir("runtime_contract_context_replay_cwd");
    let home = temp_dir("runtime_contract_context_replay_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "context ready".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let secret = "sk-test-secret-1234567890";

    let events = engine
        .handle_runtime_command(
            "cmd_context_replay",
            RuntimeCommand::SubmitUserInput {
                content: format!("summarize without leaking {secret}"),
            },
            &mut approver,
        )
        .unwrap();

    let event_json = serde_json::to_string(&events).unwrap();
    assert!(!event_json.contains(secret));
    for event in &events {
        if matches!(
            event.kind,
            RuntimeEventKind::ContextItemStored { .. }
                | RuntimeEventKind::ContextViewDerived { .. }
                | RuntimeEventKind::ContextBundleBuilt { .. }
                | RuntimeEventKind::ContextBudgetExceeded { .. }
                | RuntimeEventKind::ContextUpdated { .. }
        ) {
            let context_event_json = serde_json::to_string(event).unwrap();
            assert!(!context_event_json.contains(cwd.to_string_lossy().as_ref()));
        }
    }
    assert!(events.iter().any(|event| {
        matches!(&event.kind, RuntimeEventKind::ContextItemStored { item }
            if !item.content_sha256.is_empty() && item.summary.contains("user-task"))
    }));
    assert!(events.iter().any(|event| {
        matches!(&event.kind, RuntimeEventKind::ContextViewDerived { view, handle }
            if view.token_count > 0
                && handle.item_id == view.item_id
                && !handle.content_sha256.is_empty()
                && !view.content_sha256.is_empty()
                && handle.preferred_view_id.as_deref() == Some(view.view_id.as_str()))
    }));
    assert!(events.iter().any(|event| {
        matches!(&event.kind, RuntimeEventKind::ContextBundleBuilt { handle_ids, estimated_tokens, .. }
            if !handle_ids.is_empty() && *estimated_tokens > 0)
    }));

    let mut view = RuntimeViewState::new(engine.runtime_snapshot());
    for event in &events {
        view.apply_event(event);
    }
    assert!(view.context.is_some());
    assert!(!view.context_items.is_empty());
    assert_eq!(view.context_items.len(), view.context_handles.len());
    assert_eq!(view.context_views.len(), view.context_handles.len());
}

#[test]
fn existing_context_source_hash_corruption_fails_visibly_on_rematerialization() {
    let cwd = temp_dir("runtime_contract_corrupt_context_cwd");
    let home = temp_dir("runtime_contract_corrupt_context_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "context ready".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .handle_runtime_command(
            "cmd_context",
            RuntimeCommand::SubmitUserInput {
                content: "build canonical context".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let mut bundle = engine.provider_context_bundle().expect("context bundle");
    let source = bundle
        .sources
        .iter_mut()
        .find(|source| source.name == "user-task")
        .expect("user-task source");
    source.content_sha256 = Some("ff".repeat(32));

    let rebuilt =
        engine.materialize_existing_context_bundle(&bundle, ContextBuildMode::RequestTooLargeRetry);

    assert!(rebuilt.events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextQualityFailed { quality }
                if quality.target_id == "user-task"
                    && quality.failure_reason.as_deref().is_some_and(|reason| {
                        reason.contains("hash mismatch") || reason.contains("context blob")
                    })
        )
    }));
}

#[test]
fn command_accepted_redacts_start_agent_dag_secrets_and_paths() {
    let cwd = temp_dir("runtime_contract_dag_redaction_cwd");
    let home = temp_dir("runtime_contract_dag_redaction_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let secret = "sk-agent-dag-secret-123";
    let raw_scope = cwd.join("secret/file.rs").to_string_lossy().to_string();

    let events = engine
        .handle_runtime_command(
            "cmd_redacted_dag",
            RuntimeCommand::StartAgentDag {
                goal: format!("Plan {secret}"),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_redacted".to_string(),
                    role: AgentRole::Planner,
                    title: format!("Title {secret}"),
                    objective: format!("Objective {secret}"),
                    dependencies: Vec::new(),
                    workspace: Some(cwd.to_string_lossy().to_string()),
                    file_scope: vec![raw_scope.clone()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let accepted = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::CommandAccepted { command, .. } => Some(command),
            _ => None,
        })
        .expect("command accepted");
    let json = serde_json::to_string(accepted).unwrap();
    assert!(!json.contains(secret));
    assert!(!json.contains(cwd.to_string_lossy().as_ref()));
    assert!(!json.contains(&raw_scope));
    let RuntimeCommand::StartAgentDag { goal, tasks } = accepted else {
        panic!("expected StartAgentDag");
    };
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].file_scope.len(), 1);
    assert_eq!(goal, "Plan [REDACTED]");
    assert_eq!(tasks[0].workspace.as_deref(), Some("[REDACTED]"));
}

#[test]
fn retrieve_context_returns_safe_bytes_and_event_metadata() {
    let cwd = temp_dir("runtime_command_retrieve_context_cwd");
    let home = temp_dir("runtime_command_retrieve_context_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let safe_content = "retrieve-context-safe-body";
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: safe_content.to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let view = engine.runtime_view_state();
    let context = view.context.as_ref().expect("runtime context bundle");
    let handle = context
        .sources
        .iter()
        .find(|source| source.name == "user-task")
        .and_then(|source| {
            context_handle_from_source(source, &ContextScope::Task(context.task_id.clone()))
        })
        .expect("user-task context handle");

    let events = engine
        .handle_runtime_command(
            "cmd_retrieve_context",
            RuntimeCommand::RetrieveContext {
                handle_id: handle.handle_id.clone(),
                reason: "hydrate context for review with sk-test-secret in reason".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                name,
                success: true,
                evidence: Some(evidence),
                ..
            } if name == "context_read" && evidence.summary.contains(safe_content)
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextRetrieved { retrieval }
                if retrieval.handle_id == handle.handle_id
                    && retrieval.item_id == handle.item_id
                    && retrieval.scope == handle.scope
                    && retrieval.byte_count == safe_content.len() as u64
                    && retrieval.token_count > 0
                    && retrieval.reason_category == "hydrate"
                    && !retrieval.reason.contains("sk-test-secret")
        )
    }));
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("sk-test-secret")
    );

    let mut projected = RuntimeViewState::new(engine.runtime_snapshot());
    for event in &events {
        projected.apply_event(event);
    }
    assert_eq!(projected.context_retrievals.len(), 1);
}

#[test]
fn retrieve_context_bounds_long_secret_and_path_reason_before_recording_event() {
    let cwd = temp_dir("runtime_command_retrieve_context_reason_cwd");
    let home = temp_dir("runtime_command_retrieve_context_reason_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: "bounded reason body".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let handle_id = engine
        .runtime_view_state()
        .context_handles
        .first()
        .unwrap()
        .handle_id
        .clone();
    let long_reason = format!(
        "hydrate {} /Users/wiki/private/context-store sk-test-secret-value {}",
        "安全".repeat(200),
        "tail".repeat(200)
    );

    let events = engine
        .handle_runtime_command(
            "cmd_retrieve_long_reason",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: long_reason,
            },
            &mut approver,
        )
        .unwrap();

    let retrieval = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ContextRetrieved { retrieval } => Some(retrieval),
            _ => None,
        })
        .expect("retrieval event");
    assert!(retrieval.reason.len() <= 256, "{}", retrieval.reason.len());
    assert!(retrieval.reason.chars().count() <= 160);
    assert!(!retrieval.reason.contains("/Users/wiki"));
    assert!(!retrieval.reason.contains("sk-test-secret"));
    assert!(std::str::from_utf8(retrieval.reason.as_bytes()).is_ok());
    assert_eq!(retrieval.permission_decision, "allow");
    assert_eq!(retrieval.reason_rule_category, "safe_read");
}

#[test]
fn retrieve_context_denies_unknown_and_cross_scope_handles_before_reading_bytes() {
    let cwd = temp_dir("runtime_command_retrieve_context_scope_cwd");
    let home = temp_dir("runtime_command_retrieve_context_scope_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let context_root = cwd.join(".viden/private-context-test");
    engine.set_context_engine_root_for_test(context_root.clone());
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: "owned task context".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let known_handle = engine
        .runtime_view_state()
        .context_handles
        .first()
        .cloned()
        .expect("known runtime handle");
    let mut unknown_cross_scope = known_handle;
    unknown_cross_scope.handle_id = "ctxh-cross-scope".to_string();
    unknown_cross_scope.scope = ContextScope::Task("task-other".to_string());
    {
        let mut store = ContextEngine::open(&context_root).unwrap();
        let stored = store
            .store(ContextPutRequest {
                scope: unknown_cross_scope.scope.clone(),
                kind: ContextContentKind::Text,
                content: b"cross scope bytes must never be read",
                evidence_id: None,
            })
            .unwrap();
        unknown_cross_scope.item_id = stored.handle.item_id;
        unknown_cross_scope.content_sha256 = stored.handle.content_sha256;
    }

    let events = engine
        .handle_runtime_command(
            "cmd_retrieve_cross",
            RuntimeCommand::RetrieveContext {
                handle_id: unknown_cross_scope.handle_id,
                reason: "hydrate foreign task".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let json = serde_json::to_string(&events).unwrap();
    assert!(!json.contains("cross scope bytes"));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. }
                if reason.contains("not known to the current runtime context")
        )
    }));
    assert!(events.iter().all(|event| {
        !matches!(
            event.kind,
            RuntimeEventKind::ContextRetrieved { .. } | RuntimeEventKind::ToolCallFinished { .. }
        )
    }));
}

#[test]
fn retrieve_context_rejects_prepare_failures_without_accepting_or_rewriting_command_ids() {
    let cwd = temp_dir("runtime_command_retrieve_context_prepare_order_cwd");
    let home = temp_dir("runtime_command_retrieve_context_prepare_order_home");
    let context_root = cwd.join(".viden/private-context-test");
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let mut unknown = engine_with_single_context(&cwd, &home, &context_root, "unknown body");
    let unknown_events = unknown
        .handle_runtime_command(
            "cmd_unknown_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id: "ctxh-does-not-exist".to_string(),
                reason: "hydrate unknown".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert_prepare_rejection_only(
        &unknown_events,
        unknown.runtime_snapshot(),
        "cmd_unknown_retrieve",
        "not known",
    );

    let mut denied = engine_with_single_context(&cwd, &home, &context_root, "denied body");
    let denied_handle_id = denied.runtime_view_state().context_handles[0]
        .handle_id
        .clone();
    denied.add_permission_rule_for_test(PermissionRule {
        source: PermissionRuleSource::Session,
        rule_behavior: PermissionBehavior::Deny,
        rule_value: PermissionRuleValue {
            tool_name: "context_read".to_string(),
            rule_content: None,
        },
    });
    let denied_events = denied
        .handle_runtime_command(
            "cmd_denied_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id: denied_handle_id,
                reason: "hydrate denied".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert_prepare_rejection_only(
        &denied_events,
        denied.runtime_snapshot(),
        "cmd_denied_retrieve",
        "Denied",
    );

    let mut expired = engine_with_single_context(&cwd, &home, &context_root, "expired body");
    let expired_handle_id = expired.runtime_view_state().context_handles[0]
        .handle_id
        .clone();
    expired.mutate_context_handle_for_test(&expired_handle_id, |handle| {
        handle.expires_at = Some(1);
    });
    let expired_events = expired
        .handle_runtime_command(
            "cmd_expired_retrieve",
            RuntimeCommand::RetrieveContext {
                handle_id: expired_handle_id,
                reason: "hydrate expired".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert_prepare_rejection_only(
        &expired_events,
        expired.runtime_snapshot(),
        "cmd_expired_retrieve",
        "expired",
    );
}

fn assert_prepare_rejection_only(
    events: &[RuntimeEvent],
    snapshot: viden_types::RuntimeSnapshot,
    command_id: &str,
    reason: &str,
) {
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected {
                    command_id: rejected_id,
                    reason: rejected_reason,
                } if rejected_id == command_id && rejected_reason.contains(reason)
            )
        }),
        "expected rejection for {command_id}: {events:#?}"
    );
    assert!(
        events.iter().all(|event| {
            !matches!(
                &event.kind,
                RuntimeEventKind::CommandAccepted {
                    command_id: accepted_id,
                    ..
                } if accepted_id == command_id
            )
        }),
        "prepare failure must not accept {command_id}: {events:#?}"
    );

    let mut projected = RuntimeViewState::new(snapshot);
    for event in events {
        projected.apply_event(event);
    }
    assert!(
        projected
            .errors
            .iter()
            .any(|error| error.message.contains(command_id) || error.message.contains(reason)),
        "replay should retain recoverable rejection evidence: {projected:#?}"
    );
}

#[test]
fn retrieve_context_uses_permission_policy_for_deny_ask_approve_and_plan_read() {
    let cwd = temp_dir("runtime_command_retrieve_context_permissions_cwd");
    let home = temp_dir("runtime_command_retrieve_context_permissions_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut allow = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: "permission gated context".to_string(),
            },
            &mut allow,
        )
        .unwrap();
    let handle_id = engine
        .runtime_view_state()
        .context_handles
        .first()
        .unwrap()
        .handle_id
        .clone();

    engine.add_permission_rule_for_test(PermissionRule {
        source: PermissionRuleSource::Session,
        rule_behavior: PermissionBehavior::Deny,
        rule_value: PermissionRuleValue {
            tool_name: "context_read".to_string(),
            rule_content: None,
        },
    });
    let denied = engine
        .handle_runtime_command(
            "cmd_retrieve_denied",
            RuntimeCommand::RetrieveContext {
                handle_id: handle_id.clone(),
                reason: "hydrate denied".to_string(),
            },
            &mut allow,
        )
        .unwrap();
    assert!(denied.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. }
                if reason.contains("Denied by permission rule")
        )
    }));
    assert!(
        denied
            .iter()
            .all(|event| !matches!(event.kind, RuntimeEventKind::ContextRetrieved { .. }))
    );

    engine.clear_permission_rules_for_test();
    engine.add_permission_rule_for_test(PermissionRule {
        source: PermissionRuleSource::Session,
        rule_behavior: PermissionBehavior::Ask,
        rule_value: PermissionRuleValue {
            tool_name: "context_read".to_string(),
            rule_content: None,
        },
    });
    let mut saw_prompt = false;
    let mut approve = |prompt: viden_types::PermissionPrompt| {
        saw_prompt = prompt.tool_name == "context_read";
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let approved = engine
        .handle_runtime_command(
            "cmd_retrieve_approved",
            RuntimeCommand::RetrieveContext {
                handle_id: handle_id.clone(),
                reason: "hydrate approved".to_string(),
            },
            &mut approve,
        )
        .unwrap();
    assert!(saw_prompt);
    assert!(
        approved
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ContextRetrieved { .. }))
    );

    engine.clear_permission_rules_for_test();
    engine
        .handle_runtime_command(
            "cmd_plan_mode",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
            &mut allow,
        )
        .unwrap();
    let plan_read = engine
        .handle_runtime_command(
            "cmd_retrieve_plan",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate plan read".to_string(),
            },
            &mut allow,
        )
        .unwrap();
    assert!(
        plan_read
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ContextRetrieved { .. }))
    );
}

#[test]
fn retrieve_context_redacts_secret_content_before_tool_result_and_events() {
    let cwd = temp_dir("runtime_command_retrieve_context_secret_cwd");
    let home = temp_dir("runtime_command_retrieve_context_secret_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: "api_key=raw-secret-value keep-safe-context".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let handle_id = engine
        .runtime_view_state()
        .context_handles
        .first()
        .unwrap()
        .handle_id
        .clone();

    let events = engine
        .handle_runtime_command(
            "cmd_retrieve_secret",
            RuntimeCommand::RetrieveContext {
                handle_id,
                reason: "hydrate secret-bearing context".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let json = serde_json::to_string(&events).unwrap();
    assert!(!json.contains("raw-secret-value"));
    assert!(json.contains("keep-safe-context"));
}

#[test]
fn retrieve_context_reports_expired_missing_item_missing_blob_and_hash_mismatch_safely() {
    let cwd = temp_dir("runtime_command_retrieve_context_errors_cwd");
    let home = temp_dir("runtime_command_retrieve_context_errors_home");
    let context_root = cwd.join(".viden/private-context-test");
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let mut expired = engine_with_single_context(&cwd, &home, &context_root, "expired bytes");
    let expired_handle = expired.runtime_view_state().context_handles[0].clone();
    expired.mutate_context_handle_for_test(&expired_handle.handle_id, |handle| {
        handle.expires_at = Some(1);
    });
    let expired_events = expired
        .handle_runtime_command(
            "cmd_expired",
            RuntimeCommand::RetrieveContext {
                handle_id: expired_handle.handle_id.clone(),
                reason: "hydrate expired".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(expired_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. } if reason.contains("expired")
        )
    }));

    let mut dag_mismatch = engine_with_single_context(&cwd, &home, &context_root, "dag bytes");
    let mismatch_handle = dag_mismatch.runtime_view_state().context_handles[0].clone();
    dag_mismatch.mutate_context_handle_for_test(&mismatch_handle.handle_id, |handle| {
        handle.scope = ContextScope::Dag("dag-other".to_string());
    });
    let mismatch_events = dag_mismatch
        .handle_runtime_command(
            "cmd_dag_mismatch",
            RuntimeCommand::RetrieveContext {
                handle_id: mismatch_handle.handle_id.clone(),
                reason: "hydrate mismatch".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(mismatch_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. }
                if reason.contains("outside the active context scope")
        )
    }));

    let mut missing_item = engine_with_single_context(&cwd, &home, &context_root, "missing item");
    let missing_item_handle = missing_item.runtime_view_state().context_handles[0].clone();
    missing_item.remove_context_item_for_test(&missing_item_handle.item_id);
    let missing_item_events = missing_item
        .handle_runtime_command(
            "cmd_missing_item",
            RuntimeCommand::RetrieveContext {
                handle_id: missing_item_handle.handle_id.clone(),
                reason: "hydrate missing item".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(missing_item_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. }
                if reason.contains("item") && reason.contains("missing")
        )
    }));

    let mut missing_blob = engine_with_single_context(&cwd, &home, &context_root, "missing blob");
    let missing_blob_handle = missing_blob.runtime_view_state().context_handles[0].clone();
    fs::remove_file(context_blob_path(
        &context_root,
        &missing_blob_handle.content_sha256,
    ))
    .unwrap();
    let missing_blob_events = missing_blob
        .handle_runtime_command(
            "cmd_missing_blob",
            RuntimeCommand::RetrieveContext {
                handle_id: missing_blob_handle.handle_id.clone(),
                reason: "hydrate missing blob".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    let missing_blob_json = serde_json::to_string(&missing_blob_events).unwrap();
    assert!(!missing_blob_json.contains(context_root.to_string_lossy().as_ref()));
    assert!(missing_blob_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. }
                if reason.contains("blob") && reason.contains("not")
        )
    }));

    let mut hash_mismatch = engine_with_single_context(&cwd, &home, &context_root, "hash before");
    let hash_handle = hash_mismatch.runtime_view_state().context_handles[0].clone();
    fs::write(
        context_blob_path(&context_root, &hash_handle.content_sha256),
        b"hash after",
    )
    .unwrap();
    let hash_events = hash_mismatch
        .handle_runtime_command(
            "cmd_hash_mismatch",
            RuntimeCommand::RetrieveContext {
                handle_id: hash_handle.handle_id,
                reason: "hydrate hash mismatch".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(hash_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { reason, .. } if reason.contains("hash mismatch")
        )
    }));
}

fn context_handle_from_source(
    source: &viden_types::ContextSourceRecord,
    scope: &ContextScope,
) -> Option<ContextHandleRecord> {
    Some(ContextHandleRecord {
        handle_id: source.handle_id.clone()?,
        item_id: source.item_id.clone()?,
        preferred_view_id: source.view_id.clone(),
        content_sha256: source.content_sha256.clone()?,
        scope: scope.clone(),
        expires_at: None,
    })
}

fn engine_with_single_context(
    cwd: &std::path::Path,
    home: &std::path::Path,
    context_root: &std::path::Path,
    content: &str,
) -> SessionEngine {
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(cwd, provider, Some(home.to_path_buf())).unwrap();
    engine.set_context_engine_root_for_test(context_root.to_path_buf());
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: content.to_string(),
            },
            &mut approver,
        )
        .unwrap();
    engine
}

fn context_blob_path(root: &std::path::Path, content_sha256: &str) -> std::path::PathBuf {
    root.join("blobs")
        .join(&content_sha256[..2])
        .join(content_sha256)
}

#[test]
fn runtime_command_bus_configures_provider_and_active_models() {
    let cwd = temp_dir("runtime_command_provider_cwd");
    let home = temp_dir("runtime_command_provider_home");
    let config_path = cwd.join("user-config.toml");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_user_config_path_override(config_path.clone());
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let configured = engine
        .handle_runtime_command(
            "cmd_provider_config",
            RuntimeCommand::ConfigureProvider {
                provider_id: "deepseek".to_string(),
                api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
                endpoint: Some("https://api.deepseek.com".to_string()),
                default_model: Some("deepseek-chat".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    assert_strictly_increasing_sequences(&configured);
    assert!(configured.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.kind == "provider_config"
                    && evidence.summary.contains("deepseek")
        )
    }));

    let activated = engine
        .handle_runtime_command(
            "cmd_activate_model",
            RuntimeCommand::ActivateModel {
                provider_id: "deepseek".to_string(),
                model: "deepseek-reasoner".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(activated.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.kind == "provider_model"
                    && evidence.summary.contains("deepseek-reasoner")
        )
    }));

    let deactivated = engine
        .handle_runtime_command(
            "cmd_deactivate_model",
            RuntimeCommand::DeactivateModel {
                provider_id: "deepseek".to_string(),
                model: "deepseek-chat".to_string(),
            },
            &mut approver,
        )
        .unwrap();
    assert!(deactivated.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.kind == "provider_model"
                    && evidence.summary.contains("removed")
        )
    }));

    let contents = fs::read_to_string(config_path).unwrap();
    assert!(contents.contains("[providers.deepseek]"));
    assert!(contents.contains(r#"api_key_env = "DEEPSEEK_API_KEY""#));
    assert!(contents.contains(r#"api_base = "https://api.deepseek.com""#));
    assert!(contents.contains(r#"default_model = "deepseek-chat""#));
    assert!(contents.contains(r#"models = ["deepseek-reasoner"]"#));
}

#[test]
fn runtime_view_state_emits_lane_facts_from_core_store() {
    let cwd = temp_dir("runtime_contract_lane_cwd");
    let home = temp_dir("runtime_contract_lane_home");
    let lane_dir = cwd.join(".viden");
    fs::create_dir_all(&lane_dir).unwrap();
    fs::write(
        lane_dir.join("lanes.tsv"),
        "L1\tcodex\tfix tests\trunning\tmain\t64\tpatched tests\t\n",
    )
    .unwrap();
    let provider = Box::new(SequenceProvider::new(vec![]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();

    let view = engine.runtime_view_state();

    assert_eq!(view.lanes.len(), 1);
    assert_eq!(view.lanes[0].id, "L1");
    assert_eq!(view.lanes[0].agent, "codex");
    assert_eq!(view.lanes[0].status, "running");
    assert_eq!(view.lanes[0].summary, "patched tests");
}

#[test]
fn runtime_view_state_emits_tracked_acp_session_jobs() {
    let cwd = temp_dir("runtime_contract_acp_job_cwd");
    let home = temp_dir("runtime_contract_acp_job_home");
    let agent_dir = cwd.join(".viden").join("agents");
    fs::create_dir_all(&agent_dir).unwrap();
    let result_path = agent_dir.join("acp-1.result.md");
    let log_path = agent_dir.join("acp-1.jsonl");
    let baseline_path = agent_dir.join("acp-1.baseline.status");
    fs::write(
        &result_path,
        "# ACP session result\n\nsession: session_1\nstatus: completed\n\nimplemented adapter",
    )
    .unwrap();
    fs::write(&log_path, "{\"method\":\"session/update\"}\n").unwrap();
    fs::write(&baseline_path, "").unwrap();
    fs::write(
        agent_dir.join("acp-1.runtime-events.jsonl"),
        [
            serde_json::to_string(&RuntimeEvent::new(
                1,
                RuntimeEventKind::AssistantDelta {
                    message_id: "acp-session-session_1".to_string(),
                    task_id: None,
                    content: "implemented adapter".to_string(),
                },
            ))
            .unwrap(),
            serde_json::to_string(&RuntimeEvent::new(
                2,
                RuntimeEventKind::EvidenceRecorded {
                    evidence: EvidenceView {
                        id: "acp-turn-session_1".to_string(),
                        kind: "acp_turn_end".to_string(),
                        summary: "ACP turn completed".to_string(),
                        path: Some(log_path.display().to_string()),
                        source: Some("acp".to_string()),
                        metadata: None,
                        timestamp: None,
                    },
                },
            ))
            .unwrap(),
        ]
        .join("\n"),
    )
    .unwrap();
    fs::write(
        agent_dir.join("codex-jobs.jsonl"),
        format!(
            "{{\"ts\":1783330000000,\"event\":\"completed\",\"id\":\"acp-1\",\"kind\":\"acp-session\",\"status\":\"finished\",\"pid\":null,\"command\":\"kiro-cli acp\",\"task\":\"implement adapter\",\"log\":\"{}\",\"result\":\"{}\",\"baseline\":\"{}\"}}\n",
            log_path.display(),
            result_path.display(),
            baseline_path.display()
        ),
    )
    .unwrap();
    let provider = Box::new(SequenceProvider::new(vec![]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();

    let view = engine.runtime_view_state();

    let task = view
        .tasks
        .iter()
        .find(|task| task.id == "acp-1")
        .expect("tracked ACP session job should become a runtime task");
    assert_eq!(task.agent, "acp");
    assert_eq!(task.kind, "job");
    assert_eq!(task.transport, "acp");
    assert_eq!(task.status, "done");
    assert_eq!(task.result, Some(result_path.display().to_string()));
    assert!(task.evidence.contains(&"session session_1".to_string()));
    assert_eq!(
        task.next_action
            .as_ref()
            .and_then(|action| action.command.as_deref()),
        Some("/agent result acp-1")
    );
    assert!(view.assistant_stream.contains("implemented adapter"));
    assert!(view.latest_evidence.iter().any(|evidence| {
        evidence.kind == "acp_turn_end" && evidence.summary.contains("completed")
    }));
}
