use std::fs;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use viden_context::{ContextEngine, ContextPutRequest};
use viden_provider::ModelProvider;
use viden_types::{
    AgentDagTaskSpec, AgentRole, ApprovalResponse, CanonicalEvidenceReference, ContextContentKind,
    ContextHandleRecord, ContextItemRecord, ContextScope, CostScope, EvidenceProducer,
    EvidenceQualityFacts, EvidenceQualityStatus, EvidenceVerificationState, EvidenceView,
    MergeGateStatus, ModelEvent, ModelRequest, ModelUsage, PermissionBehavior, PermissionLevel,
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

fn single_cost<'a>(
    events: &'a [viden_types::RuntimeEvent],
    provider_id: &str,
) -> &'a viden_types::CostUsageRecord {
    let costs = events
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::CostUsageRecorded { cost } if cost.provider_id == provider_id => {
                Some(cost)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(costs.len(), 1, "expected one {provider_id} cost event");
    costs[0]
}

fn assert_scope_once(cost: &viden_types::CostUsageRecord, scope: CostScope) {
    assert_eq!(
        cost.scopes
            .iter()
            .filter(|candidate| **candidate == scope)
            .count(),
        1,
        "scope {scope:?} should appear exactly once in {:?}",
        cost.scopes
    );
}

fn cost_provider_task_id(cost: &viden_types::CostUsageRecord) -> String {
    cost.scopes
        .iter()
        .find_map(|scope| match scope {
            CostScope::AgentTask(id) => Some(id.clone()),
            _ => None,
        })
        .expect("provider cost should include task scope")
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
fn provider_cost_attribution_uses_explicit_workflow_and_smoke_without_duplicate_scopes() {
    let cwd = temp_dir("cost_attr_main_cwd");
    let home = temp_dir("cost_attr_main_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "cost attribution".to_string(),
        },
        ModelEvent::Usage(ModelUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            cached_input_tokens: Some(3),
            retrieval_tokens: None,
            total_tokens: Some(14),
            cost_micro_usd: None,
            actual_cost_micro_usd: None,
        }),
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_cost_workflow_id_for_test(Some("workflow-main-1"));
    engine.set_cost_smoke_run_id_for_test(Some("smoke-main-1"));
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let engine_events = engine
        .process_input_with_approval("attribute main turn", &mut approver)
        .unwrap();
    let runtime_events = engine.runtime_events_for_engine_events(&engine_events);
    let mut view = RuntimeViewState::new(engine.runtime_snapshot());
    for event in &runtime_events {
        view.apply_event(event);
    }
    let cost = single_cost(&runtime_events, "sequence");

    assert_scope_once(cost, CostScope::Request(cost.usage_id.clone()));
    assert_scope_once(cost, CostScope::AgentTask(cost_provider_task_id(cost)));
    assert_scope_once(cost, CostScope::Workflow("workflow-main-1".to_string()));
    assert_scope_once(cost, CostScope::SmokeRun("smoke-main-1".to_string()));
    assert!(
        !cost
            .scopes
            .contains(&CostScope::Workflow("interactive".to_string()))
    );
    assert_eq!(view.cost_ledger.input_tokens, 10);
    assert_eq!(view.cost_ledger.cached_input_tokens, 3);
    assert_eq!(
        view.cost_usage
            .iter()
            .filter(|record| record
                .scopes
                .contains(&CostScope::SmokeRun("smoke-main-1".to_string())))
            .count(),
        1
    );
}

#[test]
fn agent_task_provider_cost_includes_dag_workflow_and_smoke_scopes() {
    let cwd = temp_dir("cost_attr_agent_cwd");
    let home = temp_dir("cost_attr_agent_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "agent done".to_string(),
        },
        ModelEvent::Usage(ModelUsage {
            input_tokens: Some(21),
            output_tokens: Some(8),
            cached_input_tokens: None,
            retrieval_tokens: None,
            total_tokens: Some(29),
            cost_micro_usd: None,
            actual_cost_micro_usd: None,
        }),
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_cost_workflow_id_for_test(Some("workflow-agent-1"));
    engine.set_cost_smoke_run_id_for_test(Some("smoke-agent-1"));
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let task_id = "agent-cost-task".to_string();
    let dag_id = "agent-cost-dag".to_string();

    engine
        .handle_runtime_command(
            "cmd_dag",
            RuntimeCommand::StartAgentDag {
                goal: "cost attribution dag".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: task_id.clone(),
                    role: AgentRole::Coder,
                    title: "cost task".to_string(),
                    objective: "return usage".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "read_write".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();
    engine.set_runtime_dag_id_for_test(&task_id, &dag_id);
    let events = engine
        .handle_runtime_command(
            "cmd_task",
            RuntimeCommand::StartAgentTask {
                task_id: task_id.clone(),
            },
            &mut approver,
        )
        .unwrap();
    let cost = single_cost(&events, "sequence");

    assert_scope_once(cost, CostScope::AgentTask(task_id));
    assert_scope_once(cost, CostScope::Dag(dag_id));
    assert_scope_once(cost, CostScope::Workflow("workflow-agent-1".to_string()));
    assert_scope_once(cost, CostScope::SmokeRun("smoke-agent-1".to_string()));
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
    let context_task_id = context.task_id.clone();
    engine.set_cost_workflow_id_for_test(Some("workflow-retrieval-1"));
    engine.set_cost_smoke_run_id_for_test(Some("smoke-retrieval-1"));
    engine
        .handle_runtime_command(
            "cmd_retrieval_dag",
            RuntimeCommand::StartAgentDag {
                goal: "retrieval ancestry".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: context_task_id.clone(),
                    role: AgentRole::Reviewer,
                    title: "retrieval task".to_string(),
                    objective: "retrieve context".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["review".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();
    engine.set_runtime_dag_id_for_test(&context_task_id, "dag-retrieval-1");
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
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CostUsageRecorded { cost }
                if cost.provider_id == "context"
                    && cost.model == "retrieval"
                    && cost.tokens.retrieval_tokens.is_some()
                    && cost.tokens.total_tokens == cost.tokens.retrieval_tokens
                    && cost.scopes.contains(&CostScope::AgentTask(context_task_id.clone()))
                    && cost.scopes.contains(&CostScope::Dag("dag-retrieval-1".to_string()))
                    && cost.scopes.contains(&CostScope::Workflow("workflow-retrieval-1".to_string()))
                    && cost.scopes.contains(&CostScope::SmokeRun("smoke-retrieval-1".to_string()))
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
fn retrieved_context_cost_survives_session_resume() {
    let cwd = temp_dir("runtime_command_retrieve_resume_cwd");
    let home = temp_dir("runtime_command_retrieve_resume_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .handle_runtime_command(
            "cmd_build_context",
            RuntimeCommand::SubmitUserInput {
                content: "resume-safe-retrieval-body".to_string(),
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
    let session_id = engine.session_id().to_string();

    engine
        .handle_runtime_command(
            "cmd_retrieve_context",
            RuntimeCommand::RetrieveContext {
                handle_id: handle.handle_id,
                reason: "hydrate resume-safe context".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let resumed_provider = Box::new(SequenceProvider::new(vec![]));
    let mut resumed =
        SessionEngine::new_with_home(&cwd, resumed_provider, Some(home.clone())).unwrap();
    let resume_engine_events = resumed
        .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
        .unwrap();
    let resume_runtime_events = resumed.runtime_events_for_engine_events(&resume_engine_events);
    let mut resumed_view = RuntimeViewState::new(resumed.runtime_snapshot());
    for event in &resume_runtime_events {
        resumed_view.apply_event(event);
    }
    let retrieval_cost = resumed_view
        .cost_usage
        .iter()
        .find(|cost| cost.provider_id == "context" && cost.model == "retrieval")
        .expect("retrieval cost survives resume");

    assert_eq!(
        retrieval_cost.tokens.retrieval_tokens,
        retrieval_cost.tokens.total_tokens
    );
    assert!(retrieval_cost.tokens.retrieval_tokens.unwrap_or(0) > 0);
    assert_eq!(
        resumed_view.cost_ledger.retrieval_tokens,
        retrieval_cost.tokens.retrieval_tokens.unwrap()
    );
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
fn merge_gate_rejects_summary_only_patch_evidence() {
    let cwd = temp_dir("runtime_contract_summary_only_gate_cwd");
    let home = temp_dir("runtime_contract_summary_only_gate_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Require canonical evidence".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_canonical_patch".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch with canonical evidence".to_string(),
                    objective: "Record a patch".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_summary_evidence",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_canonical_patch".to_string(),
                evidence_id: Some("evidence-summary-patch".to_string()),
                kind: "patch".to_string(),
                summary: "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
                path: None,
                source: Some("legacy-agent".to_string()),
                canonical: None,
            },
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-task_canonical_patch"
                    && gate.status == MergeGateStatus::CollectingEvidence
                    && gate.decision.as_deref().is_some_and(|decision| decision.contains("missing_canonical"))
        )
    }));
    let store = viden_workflows::stores::WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_evidence_recorded"
            && event.task_id.as_deref() == Some("task_canonical_patch")
            && event
                .payload
                .get("gate_status")
                .is_some_and(|status| status == "collecting_evidence")
            && event
                .payload
                .get("canonical_reasons")
                .is_some_and(|reasons| reasons.contains("missing_canonical"))
    }));
}

#[test]
fn accept_merge_gate_command_cannot_bypass_invalid_evidence() {
    let cwd = temp_dir("runtime_contract_accept_bypass_gate_cwd");
    let home = temp_dir("runtime_contract_accept_bypass_gate_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Block direct accept".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_accept_bypass".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch with summary evidence".to_string(),
                    objective: "Record an incomplete patch".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();
    engine
        .handle_runtime_command(
            "cmd_summary_evidence",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_accept_bypass".to_string(),
                evidence_id: Some("evidence-summary-patch".to_string()),
                kind: "patch".to_string(),
                summary: "summary-only patch".to_string(),
                path: None,
                source: Some("legacy-agent".to_string()),
                canonical: None,
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_accept_gate",
            RuntimeCommand::AcceptMergeGate {
                gate_id: "gate-task_accept_bypass".to_string(),
                decision: Some("force accept".to_string()),
            },
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_accept_gate"
                    && reason.contains("missing_canonical")
        )
    }));
    assert_eq!(
        engine
            .runtime_view_state()
            .merge_gates
            .iter()
            .find(|gate| gate.gate_id == "gate-task_accept_bypass")
            .unwrap()
            .status,
        MergeGateStatus::CollectingEvidence
    );
}

#[test]
fn merge_gate_accepts_fully_verified_canonical_evidence() {
    let cwd = temp_dir("runtime_contract_verified_canonical_gate_cwd");
    let home = temp_dir("runtime_contract_verified_canonical_gate_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Accept canonical evidence".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_verified_patch".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch with canonical source".to_string(),
                    objective: "Record a verified patch".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();
    let item = canonical_context_item("task_verified_patch", "ctxi-patch", "ab");
    engine.set_merge_gate_context_facts_for_test("bundle-patch", item.clone());

    let events = engine
        .handle_runtime_command(
            "cmd_canonical_evidence",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_verified_patch".to_string(),
                evidence_id: Some("evidence-canonical-patch".to_string()),
                kind: "patch".to_string(),
                summary: "canonical patch evidence".to_string(),
                path: None,
                source: Some("executor".to_string()),
                canonical: Some(canonical_reference(
                    "task_verified_patch",
                    "ctxi-patch",
                    "bundle-patch",
                    "ab",
                )),
            },
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-task_verified_patch"
                    && gate.status == MergeGateStatus::Accepted
                    && gate.decision.is_none()
        )
    }));
}

#[test]
fn merge_gate_accepts_all_required_canonical_evidence_kinds_idempotently_after_replay() {
    let cwd = temp_dir("runtime_contract_all_canonical_kinds_cwd");
    let home = temp_dir("runtime_contract_all_canonical_kinds_home");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let mut recorded_events = engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Require all canonical evidence kinds".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_all_kinds".to_string(),
                    role: AgentRole::ReleaseOperator,
                    title: "Release gate".to_string(),
                    objective: "Collect all gate evidence".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec![".".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec![
                        "patch".to_string(),
                        "test".to_string(),
                        "review".to_string(),
                        "doc".to_string(),
                        "release".to_string(),
                    ],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    for (index, (kind, prefix)) in [
        ("patch", "aa"),
        ("test_result", "bb"),
        ("review", "cc"),
        ("doc_update", "dd"),
        ("release_artifact", "ee"),
    ]
    .into_iter()
    .enumerate()
    {
        let item_id = format!("ctxi-{kind}");
        let bundle_id = format!("bundle-{kind}");
        let evidence_id = format!("evidence-{kind}");
        let item = canonical_context_item("task_all_kinds", &item_id, prefix);
        engine.set_merge_gate_context_facts_for_test(&bundle_id, item);
        let mut events = engine
            .handle_runtime_command(
                format!("cmd_canonical_{kind}"),
                RuntimeCommand::RecordAgentEvidence {
                    gate_id: "gate-task_all_kinds".to_string(),
                    evidence_id: Some(evidence_id.clone()),
                    kind: kind.to_string(),
                    summary: format!("canonical {kind} evidence"),
                    path: None,
                    source: Some("executor".to_string()),
                    canonical: Some(canonical_reference(
                        "task_all_kinds",
                        &item_id,
                        &bundle_id,
                        prefix,
                    )),
                },
                &mut approver,
            )
            .unwrap();
        if index == 4 {
            assert!(events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                        if gate.gate_id == "gate-task_all_kinds"
                            && gate.status == MergeGateStatus::Accepted
                            && gate.evidence_ids.len() == 5
                )
            }));
        }
        recorded_events.append(&mut events);
    }

    let duplicate_events = engine
        .handle_runtime_command(
            "cmd_duplicate_patch",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_all_kinds".to_string(),
                evidence_id: Some("evidence-patch".to_string()),
                kind: "patch".to_string(),
                summary: "duplicate canonical patch evidence".to_string(),
                path: None,
                source: Some("executor".to_string()),
                canonical: Some(canonical_reference(
                    "task_all_kinds",
                    "ctxi-patch",
                    "bundle-patch",
                    "aa",
                )),
            },
            &mut approver,
        )
        .unwrap();
    recorded_events.extend(duplicate_events);

    let live = engine.runtime_view_state();
    let gate = live
        .merge_gates
        .iter()
        .find(|gate| gate.gate_id == "gate-task_all_kinds")
        .unwrap();
    assert_eq!(gate.status, MergeGateStatus::Accepted);
    assert_eq!(gate.evidence_ids.len(), 5);
    assert_eq!(
        live.latest_evidence
            .iter()
            .filter(|evidence| evidence.id == "evidence-patch")
            .count(),
        1
    );
    assert_eq!(live.canonical_evidence.len(), 5);
    let live_json = serde_json::to_string(&live).unwrap();
    assert!(!live_json.contains("storage_path"));

    let mut replayed = RuntimeViewState::new(live.snapshot.clone());
    for event in &recorded_events {
        replayed.apply_event(event);
    }
    assert_eq!(replayed.merge_gates, live.merge_gates);
    assert_eq!(replayed.latest_evidence, live.latest_evidence);
}

#[test]
fn merge_gate_reports_stable_canonical_failure_reasons() {
    for (case, expected_status, expected_reason, configure) in canonical_failure_cases() {
        let cwd = temp_dir(&format!("runtime_contract_canonical_failure_{case}_cwd"));
        let home = temp_dir(&format!("runtime_contract_canonical_failure_{case}_home"));
        let provider = Box::new(SequenceProvider::new(vec![]));
        let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
        let mut approver = |_prompt| ApprovalResponse {
            approved: true,
            feedback: None,
        };
        let task_id = format!("task_{case}");
        engine
            .handle_runtime_command(
                "cmd_agent_dag",
                RuntimeCommand::StartAgentDag {
                    goal: format!("Canonical failure {case}"),
                    tasks: vec![AgentDagTaskSpec {
                        task_id: task_id.clone(),
                        role: AgentRole::Tester,
                        title: format!("Canonical failure {case}"),
                        objective: "Record canonical test evidence".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test".to_string()],
                        permission_policy: "read_only".to_string(),
                    }],
                },
                &mut approver,
            )
            .unwrap();

        let mut canonical = canonical_reference(&task_id, "ctxi-test", "bundle-test", "ab");
        let mut item = canonical_context_item(&task_id, "ctxi-test", "ab");
        let should_seed_source = configure(&mut canonical, &mut item);
        if should_seed_source {
            engine.set_merge_gate_context_facts_for_test("bundle-test", item);
        }

        let events = engine
            .handle_runtime_command(
                "cmd_canonical_evidence",
                RuntimeCommand::RecordAgentEvidence {
                    gate_id: format!("gate-{task_id}"),
                    evidence_id: Some(format!("evidence-{case}")),
                    kind: "test_result".to_string(),
                    summary: "canonical test evidence".to_string(),
                    path: None,
                    source: Some("executor".to_string()),
                    canonical: Some(canonical),
                },
                &mut approver,
            )
            .unwrap();

        assert!(
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                        if gate.gate_id == format!("gate-{task_id}")
                            && gate.status == expected_status
                            && gate
                                .decision
                                .as_deref()
                                .is_some_and(|decision| decision.contains(expected_reason))
                )
            }),
            "case {case} did not report {expected_status:?}/{expected_reason}: {events:#?}"
        );
    }
}

type CanonicalFailureConfigurator =
    fn(&mut CanonicalEvidenceReference, &mut ContextItemRecord) -> bool;

fn canonical_failure_cases() -> Vec<(
    &'static str,
    MergeGateStatus,
    &'static str,
    CanonicalFailureConfigurator,
)> {
    vec![
        (
            "hash_mismatch",
            MergeGateStatus::Blocked,
            "hash_mismatch",
            |canonical, _item| {
                canonical.source_hash = "cd".repeat(32);
                true
            },
        ),
        (
            "missing_source",
            MergeGateStatus::Blocked,
            "missing_source",
            |_canonical, _item| false,
        ),
        (
            "wrong_scope",
            MergeGateStatus::Blocked,
            "scope_mismatch",
            |canonical, item| {
                canonical.evidence_scope = ContextScope::Task("task-other".to_string());
                canonical.permission_scope = ContextScope::Task("task-other".to_string());
                item.scope = ContextScope::Task("task-other".to_string());
                true
            },
        ),
        (
            "missing_permission",
            MergeGateStatus::Blocked,
            "missing_permission_snapshot",
            |canonical, _item| {
                canonical.permission_snapshot_id = None;
                true
            },
        ),
        (
            "invalid_permission",
            MergeGateStatus::Blocked,
            "invalid_permission_snapshot",
            |canonical, _item| {
                canonical.permission_scope = ContextScope::Task("task-other".to_string());
                true
            },
        ),
        (
            "missing_producer",
            MergeGateStatus::Blocked,
            "missing_producer",
            |canonical, _item| {
                canonical.producer.identity.clear();
                true
            },
        ),
        (
            "quality_fail",
            MergeGateStatus::NeedsChanges,
            "quality_failed",
            |canonical, _item| {
                canonical.quality.status = EvidenceQualityStatus::Fail;
                true
            },
        ),
    ]
}

fn canonical_context_item(task_id: &str, item_id: &str, hash_prefix: &str) -> ContextItemRecord {
    ContextItemRecord {
        item_id: item_id.to_string(),
        scope: ContextScope::Task(task_id.to_string()),
        kind: ContextContentKind::Diff,
        content_sha256: hash_prefix.repeat(32),
        title: "canonical evidence".to_string(),
        summary: "bounded canonical evidence summary".to_string(),
        token_count: 10,
        evidence_id: None,
        created_at: Some(1),
    }
}

fn canonical_reference(
    task_id: &str,
    item_id: &str,
    bundle_id: &str,
    hash_prefix: &str,
) -> CanonicalEvidenceReference {
    CanonicalEvidenceReference {
        item_id: item_id.to_string(),
        bundle_id: bundle_id.to_string(),
        source_hash: hash_prefix.repeat(32),
        producer: EvidenceProducer {
            identity: "executor".to_string(),
            role: "coder".to_string(),
            task_id: task_id.to_string(),
        },
        permission_snapshot_id: Some(format!("perm-{task_id}")),
        permission_scope: ContextScope::Task(task_id.to_string()),
        evidence_scope: ContextScope::Task(task_id.to_string()),
        verification: EvidenceVerificationState::Verified,
        quality: EvidenceQualityFacts {
            status: EvidenceQualityStatus::Pass,
            reason_codes: Vec::new(),
        },
    }
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
                        canonical: None,
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
