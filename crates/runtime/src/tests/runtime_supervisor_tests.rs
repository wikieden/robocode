use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use std::{fs, path::Path};

use viden_lsp::{LspRuntime, LspServerConfig, LspServerRegistry};
use viden_provider::{ModelProvider, ModelRequestControl};
use viden_types::{
    AgentDagTaskSpec, AgentRole, AgentTaskStatus, ApprovalResponse, ContextBundleRecord,
    MergeGateStatus, ModelEvent, ModelRequest, RuntimeCommand, RuntimeEvent, RuntimeEventKind,
    ToolCall, ToolInput, WorkMode,
};
use viden_workflows::stores::WorkflowStore;

use crate::{RuntimeSupervisor, SessionEngine};

use super::{SequenceProvider, temp_dir};

static CUSTOM_ACP_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct BlockingProvider {
    entered: Arc<AtomicBool>,
}

impl ModelProvider for BlockingProvider {
    fn provider_name(&self) -> &str {
        "blocking"
    }

    fn model(&self) -> &str {
        "blocking-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Ok(Vec::new())
    }

    fn next_events_with_control(
        &mut self,
        _request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        self.entered.store(true, Ordering::SeqCst);
        loop {
            control.check_cancelled()?;
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

struct TimeoutUnlessCancelledProvider {
    entered: Arc<AtomicBool>,
}

impl ModelProvider for TimeoutUnlessCancelledProvider {
    fn provider_name(&self) -> &str {
        "cancel-aware"
    }

    fn model(&self) -> &str {
        "cancel-aware-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Err("provider called without cancellation control".to_string())
    }

    fn next_events_with_control(
        &mut self,
        _request: &ModelRequest,
        control: &ModelRequestControl,
    ) -> Result<Vec<ModelEvent>, String> {
        self.entered.store(true, Ordering::SeqCst);
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(500) {
            control.check_cancelled()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("provider did not receive cancellation".to_string())
    }
}

struct FailingProvider {
    error: String,
}

impl ModelProvider for FailingProvider {
    fn provider_name(&self) -> &str {
        "failing"
    }

    fn model(&self) -> &str {
        "failing-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Err(self.error.clone())
    }
}

struct RecordingProvider {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
    errors: Vec<String>,
}

impl RecordingProvider {
    fn success(requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
        Self {
            requests,
            errors: Vec::new(),
        }
    }

    fn with_errors(requests: Arc<Mutex<Vec<ModelRequest>>>, errors: Vec<String>) -> Self {
        Self { requests, errors }
    }
}

impl ModelProvider for RecordingProvider {
    fn provider_name(&self) -> &str {
        "recording"
    }

    fn model(&self) -> &str {
        "recording-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.requests.lock().unwrap().push(request.clone());
        if !self.errors.is_empty() {
            return Err(self.errors.remove(0));
        }
        Ok(vec![ModelEvent::AssistantText {
            content: "recorded".to_string(),
        }])
    }
}

#[test]
fn runtime_supervisor_cancels_active_provider_turn_and_keeps_worker_alive() {
    let cwd = temp_dir("runtime_supervisor_cancel_cwd");
    let home = temp_dir("runtime_supervisor_cancel_home");
    let entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(BlockingProvider {
        entered: Arc::clone(&entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_input",
            RuntimeCommand::SubmitUserInput {
                content: "start long provider turn".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));

    supervisor
        .send_command("cmd_cancel", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_cancel"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
        )
    }));

    supervisor
        .send_command(
            "cmd_mode",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        )
        .unwrap();
    let after_cancel = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == WorkMode::Plan
            )
        })
    });
    assert!(after_cancel.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Plan
        )
    }));
}

#[test]
fn runtime_supervisor_cancels_active_agent_task_and_keeps_worker_alive() {
    let cwd = temp_dir("runtime_supervisor_cancel_agent_cwd");
    let home = temp_dir("runtime_supervisor_cancel_agent_home");
    let entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(TimeoutUnlessCancelledProvider {
        entered: Arc::clone(&entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Cancel active role execution".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan cancellation".to_string(),
                    objective: "Enter provider turn and wait for cancellation".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();
    wait_until(|| entered.load(Ordering::SeqCst));

    supervisor
        .send_command("cmd_cancel_agent", RuntimeCommand::CancelActiveTurn)
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_planner" && task.status == "cancelled"
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_cancel_agent"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::Error { error } if error.message.contains("Model request cancelled")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_planner" && task.status == "cancelled"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    assert!(store.load_agent_events().unwrap().iter().any(|event| {
        event.event_type == "agent_task_cancelled"
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("error")
                .is_some_and(|error| error.contains("Model request cancelled"))
    }));

    supervisor
        .send_command(
            "cmd_mode_after_agent_cancel",
            RuntimeCommand::SetWorkMode {
                mode: WorkMode::Plan,
            },
        )
        .unwrap();
    let after_cancel = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == WorkMode::Plan
            )
        })
    });
    assert!(after_cancel.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Plan
        )
    }));
}

#[test]
fn runtime_supervisor_streams_async_acp_runtime_events_live() {
    let _guard = CUSTOM_ACP_ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("custom ACP env lock");
    let cwd = temp_dir("runtime_supervisor_acp_live_cwd");
    let home = temp_dir("runtime_supervisor_acp_live_home");
    let script = cwd.join("mock-acp-supervisor-live.sh");
    fs::write(
        &script,
        [
            "#!/bin/sh",
            "read _init",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":1,\"agentCapabilities\":{},\"agentInfo\":{\"name\":\"mock-supervisor-acp\",\"version\":\"0.5.0\"}}}'",
            "read _new_session",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"sessionId\":\"session_supervisor_live\"}}'",
            "read _prompt",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_supervisor_live\",\"update\":{\"type\":\"AgentMessageChunk\",\"content\":\"supervisor live delta\"}}}'",
            "sleep 2",
            "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"session_supervisor_live\",\"update\":{\"type\":\"TurnEnd\",\"status\":\"completed\"}}}'",
        ]
        .join("\n"),
    )
    .expect("write supervisor ACP mock");
    // The env lock serializes this process-wide override for the custom ACP descriptor.
    unsafe {
        std::env::set_var(
            "VIDEN_AGENT_ACP_COMMAND",
            format!("sh {}", script.display()),
        );
    }

    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);
    supervisor
        .send_command(
            "cmd_acp_async",
            RuntimeCommand::SubmitUserInput {
                content: "/agent run acp --async custom-acp stream live".to_string(),
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::AssistantDelta { content, .. }
                    if content == "supervisor live delta"
            )
        })
    });
    // Restore the process environment while still holding the env lock.
    unsafe {
        std::env::remove_var("VIDEN_AGENT_ACP_COMMAND");
    }

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "cmd_acp_async"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content == "supervisor live delta"
        )
    }));
}

#[test]
fn runtime_supervisor_resolves_tool_approval_without_tui_coupling() {
    let cwd = temp_dir("runtime_supervisor_approval_cwd");
    let home = temp_dir("runtime_supervisor_approval_home");
    let mut input = ToolInput::new();
    input.insert("command".to_string(), "printf approved".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_shell".to_string(),
            name: "shell".to_string(),
            input,
        }),
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_input",
            RuntimeCommand::SubmitUserInput {
                content: "run approved command".to_string(),
            },
        )
        .unwrap();
    let mut events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    });
    let request_id = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ApprovalRequested { approval } => Some(approval.id.clone()),
            _ => None,
        })
        .expect("approval request event");

    supervisor
        .send_command(
            "cmd_approval",
            RuntimeCommand::RespondToApproval {
                request_id,
                response: ApprovalResponse {
                    approved: true,
                    feedback: None,
                },
            },
        )
        .unwrap();
    events.extend(collect_events_until(
        &supervisor,
        Duration::from_secs(2),
        |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ToolCallFinished {
                        success: true,
                        evidence: Some(evidence),
                        ..
                    } if evidence.summary.contains("approved")
                )
            })
        },
    ));

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
fn runtime_supervisor_starts_agent_dag_without_provider_turn() {
    let cwd = temp_dir("runtime_supervisor_agent_dag_cwd");
    let home = temp_dir("runtime_supervisor_agent_dag_home");
    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Complete 0.2.2 role runtime".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_planner".to_string(),
                        role: AgentRole::Planner,
                        title: "Plan implementation".to_string(),
                        objective: "Split the work".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["plan".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_coder".to_string(),
                        role: AgentRole::Coder,
                        title: "Implement contracts".to_string(),
                        objective: "Add runtime contracts".to_string(),
                        dependencies: vec!["task_planner".to_string()],
                        workspace: None,
                        file_scope: vec!["crates/types".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string(), "test".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                ],
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .filter(|event| matches!(event.kind, RuntimeEventKind::TaskUpdated { .. }))
            .count()
            >= 2
            && events
                .iter()
                .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
            && events
                .iter()
                .filter(|event| matches!(event.kind, RuntimeEventKind::MergeGateUpdated { .. }))
                .count()
                >= 2
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_agent_dag"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AgentDagUpdated { dag }
                if dag.goal == "Complete 0.2.2 role runtime"
                    && dag.tasks.len() == 2
                    && dag.tasks[0].role == AgentRole::Planner
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_coder"
                    && task.agent == "coder"
                    && task.parent_id.as_deref() == Some("task_planner")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.task_id == "task_coder"
                    && gate.required_evidence == vec!["patch".to_string(), "test".to_string()]
        )
    }));
}

#[test]
fn runtime_supervisor_runs_agent_task_through_provider_and_merge_gate() {
    let cwd = temp_dir("runtime_supervisor_agent_task_cwd");
    let home = temp_dir("runtime_supervisor_agent_task_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "Plan: split runtime, workflow, and tests.".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Complete provider-backed role execution".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan role execution".to_string(),
                    objective: "Design the next implementation slice".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_planner"
                        && task.status == "done"
                        && task.result.as_deref().is_some_and(|result| {
                            result.contains("Plan: split runtime")
                        })
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::EvidenceRecorded { evidence }
                    if evidence.kind == "plan"
                        && evidence.summary.contains("Plan: split runtime")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ContextUpdated { context }
                    if context.task_id == "task_planner"
                        && context.policy.contains("agent-role")
                        && context.sources.iter().any(|source| source.name == "agent-role")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.task_id == "task_planner"
                        && !gate.status.is_open()
                        && gate.evidence_ids.iter().any(|id| id.contains("task_planner"))
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content.contains("Plan: split runtime")
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_agent_start"
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_planner"
                    && task.status == AgentTaskStatus::Done.as_str()
                    && task.result.as_deref().is_some_and(|result| {
                        result.contains("Plan: split runtime")
                    })
        )
    }));
    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    let dag_id = agent_events
        .iter()
        .find(|event| {
            event.event_type == "agent_task_completed"
                && event.task_id.as_deref() == Some("task_planner")
        })
        .map(|event| event.dag_id.clone())
        .unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_task_started"
            && event.dag_id == dag_id
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("role")
                .is_some_and(|role| role == "planner")
    }));
}

#[test]
fn runtime_supervisor_builds_role_specific_context_bundle_sources() {
    let cwd = temp_dir("runtime_supervisor_role_context_cwd");
    let home = temp_dir("runtime_supervisor_role_context_home");
    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Build role-specific contexts".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_plan".to_string(),
                        role: AgentRole::Planner,
                        title: "Plan context".to_string(),
                        objective: "Plan the architecture".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["plan".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_code".to_string(),
                        role: AgentRole::Coder,
                        title: "Code context".to_string(),
                        objective: "Implement the change".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_review".to_string(),
                        role: AgentRole::Reviewer,
                        title: "Review context".to_string(),
                        objective: "Review the change".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["review".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_test".to_string(),
                        role: AgentRole::Tester,
                        title: "Test context".to_string(),
                        objective: "Verify behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime/src/tests".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_docs".to_string(),
                        role: AgentRole::DocWriter,
                        title: "Docs context".to_string(),
                        objective: "Update documentation".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["doc_update".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let planner = start_agent_task_and_capture_context(&supervisor, "task_plan");
    assert_context_source(&planner, "role-planning-context", "role-guidance");
    assert_context_source(&planner, "agent-file-scope", "file-scope");
    assert_context_source(&planner, "agent-evidence-contract", "evidence-contract");

    let coder = start_agent_task_and_capture_context(&supervisor, "task_code");
    assert_context_source(&coder, "role-implementation-context", "role-guidance");
    assert_context_source(&coder, "agent-file-scope", "file-scope");
    assert_context_source(&coder, "agent-evidence-contract", "evidence-contract");

    let reviewer = start_agent_task_and_capture_context(&supervisor, "task_review");
    assert_context_source(&reviewer, "role-review-context", "role-guidance");
    assert_ne!(
        context_source_summary(&planner, "role-planning-context", "role-guidance"),
        context_source_summary(&reviewer, "role-review-context", "role-guidance")
    );

    let tester = start_agent_task_and_capture_context(&supervisor, "task_test");
    assert_context_source(&tester, "role-verification-context", "role-guidance");

    let doc_writer = start_agent_task_and_capture_context(&supervisor, "task_docs");
    assert_context_source(&doc_writer, "role-documentation-context", "role-guidance");
}

#[test]
fn agent_task_provider_request_uses_final_role_context_bundle() {
    let cwd = temp_dir("runtime_supervisor_provider_role_bundle_cwd");
    let home = temp_dir("runtime_supervisor_provider_role_bundle_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct RuntimeContractRoleBundle {}\n",
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::success(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(1_000, 8_000);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let _ = engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Build role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_plan_provider_bundle".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan provider bundle".to_string(),
                    objective: "Plan with role guidance".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_start_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_plan_provider_bundle".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let provider_manifest = requests[0]
        .messages
        .iter()
        .find(|message| message.content.contains("Viden ContextBundle"))
        .expect("provider context manifest")
        .content
        .clone();
    assert!(provider_manifest.contains("Bundle: ctx-agent-task_plan_provider_bundle"));
    assert!(provider_manifest.contains("Scope: task:task_plan_provider_bundle"));
    assert!(provider_manifest.contains("role-planning-context"));
    assert!(provider_manifest.contains("handle="));
    assert!(provider_manifest.contains("view="));
    assert!(provider_manifest.contains("quality="));
    assert!(!provider_manifest.contains("Focus on requirements"));
    assert!(!provider_manifest.contains(cwd.to_string_lossy().as_ref()));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextUpdated { context }
                if context.bundle_id == "ctx-agent-task_plan_provider_bundle"
                    && context.sources.iter().any(|source| {
                        source.name == "role-planning-context"
                            && source.handle_id.is_some()
                            && source.view_id.is_some()
                            && source.content_sha256.is_some()
                            && source.quality_id.is_some()
                    })
        )
    }));
}

#[test]
fn reviewer_agent_task_provider_request_uses_review_role_context() {
    let cwd = temp_dir("runtime_supervisor_reviewer_provider_bundle_cwd");
    let home = temp_dir("runtime_supervisor_reviewer_provider_bundle_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct ReviewRoleScopedBundle {}\n",
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::success(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(1_000, 8_000);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Review role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_review_provider_bundle".to_string(),
                    role: AgentRole::Reviewer,
                    title: "Review provider bundle".to_string(),
                    objective: "Review with role guidance".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["review".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    engine
        .handle_runtime_command(
            "cmd_start_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_review_provider_bundle".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let provider_manifest = provider_manifest(&requests[0]);
    assert!(provider_manifest.contains("Bundle: ctx-agent-task_review_provider_bundle"));
    assert!(provider_manifest.contains("Scope: task:task_review_provider_bundle"));
    assert!(provider_manifest.contains("role-review-context"));
    assert!(!provider_manifest.contains("role-planning-context"));
    assert!(!provider_manifest.contains("Focus on behavioral regressions"));
    assert!(!provider_manifest.contains(cwd.to_string_lossy().as_ref()));
}

#[test]
fn agent_task_context_overflow_retry_preserves_role_scoped_bundle() {
    let cwd = temp_dir("runtime_supervisor_agent_retry_role_bundle_cwd");
    let home = temp_dir("runtime_supervisor_agent_retry_role_bundle_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct RetryRoleScopedBundle {}\n",
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::with_errors(
        Arc::clone(&requests),
        vec!["context_overflow: current request exceeded provider context".to_string()],
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(2_000, 8_000);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Retry role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_retry_role_bundle".to_string(),
                    role: AgentRole::Planner,
                    title: "Retry planner".to_string(),
                    objective: "Plan while retrying context overflow".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_start_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_retry_role_bundle".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| {
        matches!(
            event,
            RuntimeEvent { kind: RuntimeEventKind::AssistantDelta { content, .. }, .. }
                if content.contains("recorded")
        )
    }));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let first_manifest = provider_manifest(&requests[0]);
    let second_manifest = provider_manifest(&requests[1]);
    assert!(first_manifest.contains("Bundle: ctx-agent-task_retry_role_bundle"));
    assert!(second_manifest.contains("Bundle: ctx-agent-task_retry_role_bundle"));
    assert!(first_manifest.contains("Policy: agent-role-planner-priority-budget"));
    assert!(second_manifest.contains("Policy: agent-role-planner-priority-budget-strict-retry"));
    assert!(first_manifest.contains("role-planning-context"));
    assert!(second_manifest.contains("role-planning-context"));
    assert!(first_manifest.contains("Scope: task:task_retry_role_bundle"));
    assert!(second_manifest.contains("Scope: task:task_retry_role_bundle"));
    assert!(second_manifest.contains("handle="));
    assert!(second_manifest.contains("view="));
    assert!(second_manifest.contains("strict-retry"));
}

#[test]
fn agent_task_hard_context_limit_rejects_before_provider_request() {
    let cwd = temp_dir("runtime_supervisor_agent_hard_budget_cwd");
    let home = temp_dir("runtime_supervisor_agent_hard_budget_home");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingProvider::success(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_context_budget_for_test(10, 20);
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let _ = engine
        .handle_runtime_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Reject huge role bundle".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_hard_budget_agent".to_string(),
                    role: AgentRole::Coder,
                    title: "Huge role context".to_string(),
                    objective: "x ".repeat(500),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "ask".to_string(),
                }],
            },
            &mut approver,
        )
        .unwrap();

    let events = engine
        .handle_runtime_command(
            "cmd_start_hard_budget_agent",
            RuntimeCommand::StartAgentTask {
                task_id: "task_hard_budget_agent".to_string(),
            },
            &mut approver,
        )
        .unwrap();

    assert!(requests.lock().unwrap().is_empty());
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ContextBudgetExceeded { budget }
                if budget.exceeded && budget.hard_token_limit == 20
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::Error { error }
                if error.message.contains("context hard limit")
                    && error.message.contains("task_hard_budget_agent")
        )
    }));
}

#[test]
fn runtime_supervisor_selects_role_specific_files_for_agent_context() {
    let cwd = temp_dir("runtime_supervisor_role_file_context_cwd");
    let home = temp_dir("runtime_supervisor_role_file_context_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "fn runtime_contract() {}",
    );
    write_test_file(
        &cwd.join("crates/runtime/src/tests/runtime_supervisor_tests.rs"),
        "fn runtime_supervisor_test() {}",
    );
    write_test_file(
        &cwd.join("crates/runtime/Cargo.toml"),
        "[package]\nname = \"viden-runtime\"",
    );
    write_test_file(&cwd.join("docs/architecture.md"), "# Architecture");
    write_test_file(&cwd.join("README.md"), "# Viden");

    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Select role files".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_code_files".to_string(),
                        role: AgentRole::Coder,
                        title: "Code file context".to_string(),
                        objective: "Implement runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_test_files".to_string(),
                        role: AgentRole::Tester,
                        title: "Test file context".to_string(),
                        objective: "Verify runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_doc_files".to_string(),
                        role: AgentRole::DocWriter,
                        title: "Doc file context".to_string(),
                        objective: "Update architecture docs".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["doc_update".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let coder = start_agent_task_and_capture_context(&supervisor, "task_code_files");
    let coder_files = context_source_summary(&coder, "role-selected-files", "selected-files");
    assert!(coder_files.contains("crates/runtime/src/runtime_contract.rs"));
    assert!(!coder_files.contains("docs/architecture.md"));

    let tester = start_agent_task_and_capture_context(&supervisor, "task_test_files");
    let tester_files = context_source_summary(&tester, "role-selected-files", "selected-files");
    assert!(tester_files.contains("crates/runtime/src/tests/runtime_supervisor_tests.rs"));
    assert!(tester_files.contains("crates/runtime/Cargo.toml"));

    let doc_writer = start_agent_task_and_capture_context(&supervisor, "task_doc_files");
    let doc_files = context_source_summary(&doc_writer, "role-selected-files", "selected-files");
    assert!(doc_files.contains("docs/architecture.md"));
    assert!(!doc_files.contains("README.md"));
}

#[test]
fn runtime_supervisor_selects_role_specific_symbols_for_agent_context() {
    let cwd = temp_dir("runtime_supervisor_role_symbol_context_cwd");
    let home = temp_dir("runtime_supervisor_role_symbol_context_home");
    write_test_file(
        &cwd.join("crates/runtime/src/runtime_contract.rs"),
        "pub struct RuntimeSupervisor {}\nimpl RuntimeSupervisor {\n    pub fn start_agent_task(&self) {}\n}\nfn helper() {}\n",
    );
    write_test_file(
        &cwd.join("crates/runtime/src/tests/runtime_supervisor_tests.rs"),
        "#[test]\nfn runtime_supervisor_starts_agent_task() {}\nfn helper_fixture() {}\n",
    );

    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::Done],
        vec![ModelEvent::Done],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Select role symbols".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_code_symbols".to_string(),
                        role: AgentRole::Coder,
                        title: "Code symbol context".to_string(),
                        objective: "Implement runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_test_symbols".to_string(),
                        role: AgentRole::Tester,
                        title: "Test symbol context".to_string(),
                        objective: "Verify runtime behavior".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let coder = start_agent_task_and_capture_context(&supervisor, "task_code_symbols");
    let coder_symbols = context_source_summary(&coder, "role-selected-symbols", "selected-symbols");
    assert!(
        coder_symbols.contains("crates/runtime/src/runtime_contract.rs::struct RuntimeSupervisor")
    );
    assert!(coder_symbols.contains("crates/runtime/src/runtime_contract.rs::fn start_agent_task"));

    let tester = start_agent_task_and_capture_context(&supervisor, "task_test_symbols");
    let tester_symbols =
        context_source_summary(&tester, "role-selected-symbols", "selected-symbols");
    assert!(
        tester_symbols
            .lines()
            .next()
            .is_some_and(|line| line.contains("runtime_supervisor_starts_agent_task")),
        "tester should prioritize test symbols: {tester_symbols}"
    );
}

#[test]
fn runtime_supervisor_adds_lsp_diagnostics_to_agent_context_bundle() {
    let cwd = temp_dir("runtime_supervisor_lsp_context_cwd");
    let home = temp_dir("runtime_supervisor_lsp_context_home");
    let fake_lsp_dir = temp_dir("runtime_supervisor_lsp_context_server");
    write_test_file(
        &cwd.join("crates/runtime/src/lib.rs"),
        "pub fn broken() {\n    let value = missing;\n}\n",
    );

    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::Done]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.lsp_runtime = Arc::new(LspRuntime::new(fake_lsp_registry(&fake_lsp_dir)));
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enrich role context with diagnostics".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_lsp_context".to_string(),
                    role: AgentRole::Coder,
                    title: "LSP context".to_string(),
                    objective: "Fix the diagnostic".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    let context = start_agent_task_and_capture_context(&supervisor, "task_lsp_context");
    let diagnostics = context_source_summary(&context, "role-lsp-diagnostics", "lsp-diagnostics");
    assert!(diagnostics.contains("LSP diagnostics:"));
    assert!(
        diagnostics.contains("crates/runtime/src/lib.rs"),
        "diagnostics summary should keep project-relative path, got: {diagnostics}"
    );
    assert!(diagnostics.contains("fake-lsp/E100"));
    assert!(diagnostics.contains("fake diagnostic"));
}

#[test]
fn runtime_supervisor_applies_read_only_role_policy_to_agent_tools() {
    let cwd = temp_dir("runtime_supervisor_role_policy_cwd");
    let home = temp_dir("runtime_supervisor_role_policy_home");
    let blocked_file = cwd.join("should_not_exist.txt");
    let mut shell_input = ToolInput::new();
    shell_input.insert(
        "command".to_string(),
        "printf blocked > should_not_exist.txt".to_string(),
    );
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::ToolCall(
        ToolCall {
            id: "tool_shell".to_string(),
            name: "shell".to_string(),
            input: shell_input,
        },
    )]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enforce role policy".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan without mutation".to_string(),
                    objective: "Planner must not run mutating tools".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start_read_only",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if evidence.summary.contains("reason: PlanMode")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::SnapshotUpdated { snapshot }
                    if snapshot.work_mode == WorkMode::Build
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::ToolCallFinished {
                success: false,
                evidence: Some(evidence),
                ..
            } if evidence.summary.contains("tool: shell")
                && evidence.summary.contains("reason: PlanMode")
        )
    }));
    assert!(
        !events
            .iter()
            .any(|event| { matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }) })
    );
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::SnapshotUpdated { snapshot }
                if snapshot.work_mode == WorkMode::Build
        )
    }));
    assert!(!blocked_file.exists());
}

#[test]
fn runtime_supervisor_applies_role_policy_matrix_to_tools() {
    let cwd = temp_dir("runtime_supervisor_role_policy_matrix_cwd");
    let home = temp_dir("runtime_supervisor_role_policy_matrix_home");
    write_test_file(&cwd.join("docs/guide.md"), "old docs");
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "old code");

    let mut test_shell = ToolInput::new();
    test_shell.insert("command".to_string(), "cargo test --help".to_string());
    let mut tester_write = ToolInput::new();
    tester_write.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    tester_write.insert("content".to_string(), "mutated".to_string());
    let mut docs_write = ToolInput::new();
    docs_write.insert("path".to_string(), "docs/guide.md".to_string());
    docs_write.insert("content".to_string(), "new docs".to_string());
    let mut code_write = ToolInput::new();
    code_write.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    code_write.insert("content".to_string(), "new code".to_string());

    let provider = Box::new(SequenceProvider::new(vec![
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_test_shell".to_string(),
                name: "shell".to_string(),
                input: test_shell,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_tester_write".to_string(),
                name: "write_file".to_string(),
                input: tester_write,
            }),
            ModelEvent::AssistantText {
                content: "tester policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_docs_write".to_string(),
                name: "write_file".to_string(),
                input: docs_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_code_write".to_string(),
                name: "write_file".to_string(),
                input: code_write,
            }),
            ModelEvent::AssistantText {
                content: "doc policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enforce role matrix".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_tester_policy".to_string(),
                        role: AgentRole::Tester,
                        title: "Tester matrix".to_string(),
                        objective: "Run verification without mutating files".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["test_result".to_string()],
                        permission_policy: "tester_verification".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_docs_policy".to_string(),
                        role: AgentRole::DocWriter,
                        title: "Doc writer matrix".to_string(),
                        objective: "Update docs only".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["doc_update".to_string()],
                        permission_policy: "docs_only".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_tester_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_tester_policy".to_string(),
            },
        )
        .unwrap();
    let tester_events = collect_events_until(&supervisor, Duration::from_secs(5), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "shell"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !tester_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        tester_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "shell"
            )
        }),
        "tester shell should run without approval: {tester_events:#?}"
    );
    assert!(
        tester_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "tester write should be denied by role policy: {tester_events:#?}"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("crates/runtime/src/lib.rs")).unwrap(),
        "old code"
    );

    supervisor
        .send_command(
            "cmd_docs_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_docs_policy".to_string(),
            },
        )
        .unwrap();
    let doc_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "write_file"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !doc_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        doc_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "write_file"
            )
        }),
        "doc writer should update docs without approval: {doc_events:#?}"
    );
    assert!(
        doc_events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "doc writer should be denied on code files: {doc_events:#?}"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/guide.md")).unwrap(),
        "new docs"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("crates/runtime/src/lib.rs")).unwrap(),
        "old code"
    );
}

#[test]
fn runtime_supervisor_applies_extended_agent_role_policy_matrix_to_tools() {
    let cwd = temp_dir("runtime_supervisor_extended_role_policy_matrix_cwd");
    let home = temp_dir("runtime_supervisor_extended_role_policy_matrix_home");
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "old code");
    write_test_file(&cwd.join("apps/tui/src/main.rs"), "old tui");
    write_test_file(&cwd.join("docs/release.md"), "old release docs");

    let mut coder_code_write = ToolInput::new();
    coder_code_write.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    coder_code_write.insert("content".to_string(), "coder code".to_string());
    let mut coder_docs_write = ToolInput::new();
    coder_docs_write.insert("path".to_string(), "docs/release.md".to_string());
    coder_docs_write.insert("content".to_string(), "bad coder docs".to_string());
    let mut release_test_shell = ToolInput::new();
    release_test_shell.insert("command".to_string(), "cargo test --help".to_string());
    let mut release_docs_write = ToolInput::new();
    release_docs_write.insert("path".to_string(), "docs/release.md".to_string());
    release_docs_write.insert("content".to_string(), "release docs".to_string());
    let mut release_push = ToolInput::new();
    release_push.insert("remote".to_string(), "origin".to_string());
    release_push.insert("branch".to_string(), "main".to_string());
    let mut external_write = ToolInput::new();
    external_write.insert("path".to_string(), "docs/release.md".to_string());
    external_write.insert("content".to_string(), "external write".to_string());
    let mut external_shell = ToolInput::new();
    external_shell.insert("command".to_string(), "printf external".to_string());

    let provider = Box::new(SequenceProvider::new(vec![
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_coder_code_write".to_string(),
                name: "write_file".to_string(),
                input: coder_code_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_coder_docs_write".to_string(),
                name: "write_file".to_string(),
                input: coder_docs_write,
            }),
            ModelEvent::AssistantText {
                content: "coder policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_release_test_shell".to_string(),
                name: "shell".to_string(),
                input: release_test_shell,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_release_docs_write".to_string(),
                name: "write_file".to_string(),
                input: release_docs_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_release_push".to_string(),
                name: "git_push".to_string(),
                input: release_push,
            }),
            ModelEvent::AssistantText {
                content: "release policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
        vec![
            ModelEvent::ToolCall(ToolCall {
                id: "tool_external_write".to_string(),
                name: "write_file".to_string(),
                input: external_write,
            }),
            ModelEvent::ToolCall(ToolCall {
                id: "tool_external_shell".to_string(),
                name: "shell".to_string(),
                input: external_shell,
            }),
            ModelEvent::AssistantText {
                content: "external policy checked".to_string(),
            },
            ModelEvent::Done,
        ],
    ]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_extended_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Enforce extended agent role matrix".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_coder_policy".to_string(),
                        role: AgentRole::Coder,
                        title: "Coder matrix".to_string(),
                        objective: "Mutate only declared code scope".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["crates/runtime".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "scoped_mutation".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_release_policy".to_string(),
                        role: AgentRole::ReleaseOperator,
                        title: "Release matrix".to_string(),
                        objective: "Run release verification without publishing".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["release_gate".to_string()],
                        permission_policy: "release_gate".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_external_policy".to_string(),
                        role: AgentRole::External,
                        title: "External matrix".to_string(),
                        objective: "Keep external agents read-only until explicitly promoted"
                            .to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: vec!["docs".to_string()],
                        context_bundle_id: None,
                        required_evidence: vec!["external_report".to_string()],
                        permission_policy: "external_agent".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_coder_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder_policy".to_string(),
            },
        )
        .unwrap();
    let coder_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "write_file"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "write_file"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !coder_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(
        fs::read_to_string(cwd.join("crates/runtime/src/lib.rs")).unwrap(),
        "coder code"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/release.md")).unwrap(),
        "old release docs"
    );

    supervisor
        .send_command(
            "cmd_release_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_release_policy".to_string(),
            },
        )
        .unwrap();
    let release_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "shell"
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "git_push"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        })
    });
    assert!(
        !release_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/release.md")).unwrap(),
        "release docs"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("apps/tui/src/main.rs")).unwrap(),
        "old tui"
    );

    supervisor
        .send_command(
            "cmd_external_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_external_policy".to_string(),
            },
        )
        .unwrap();
    let external_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .filter(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ToolCallFinished {
                        success: false,
                        evidence: Some(evidence),
                        ..
                    } if evidence.summary.contains("reason: RuleDeny")
                )
            })
            .count()
            >= 2
    });
    assert!(
        !external_events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert_eq!(
        fs::read_to_string(cwd.join("docs/release.md")).unwrap(),
        "release docs"
    );
}

#[test]
fn runtime_supervisor_applies_scoped_git_policy_to_agent_tasks() {
    let cwd = temp_dir("runtime_supervisor_scoped_git_policy_cwd");
    let home = temp_dir("runtime_supervisor_scoped_git_policy_home");
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "old code\n");
    write_test_file(&cwd.join("docs/release.md"), "old docs\n");
    init_git_repo(&cwd);
    write_test_file(&cwd.join("crates/runtime/src/lib.rs"), "new code\n");
    write_test_file(&cwd.join("docs/release.md"), "new docs\n");

    let mut scoped_git_add = ToolInput::new();
    scoped_git_add.insert("path".to_string(), "crates/runtime/src/lib.rs".to_string());
    let mut unscoped_git_add = ToolInput::new();
    unscoped_git_add.insert("path".to_string(), "docs/release.md".to_string());
    let mut git_commit = ToolInput::new();
    git_commit.insert("message".to_string(), "agent commit".to_string());

    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::ToolCall(ToolCall {
            id: "tool_scoped_git_add".to_string(),
            name: "git_add".to_string(),
            input: scoped_git_add,
        }),
        ModelEvent::ToolCall(ToolCall {
            id: "tool_unscoped_git_add".to_string(),
            name: "git_add".to_string(),
            input: unscoped_git_add,
        }),
        ModelEvent::ToolCall(ToolCall {
            id: "tool_git_commit".to_string(),
            name: "git_commit".to_string(),
            input: git_commit,
        }),
        ModelEvent::AssistantText {
            content: "git policy checked".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Apply scoped Git policy".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_scoped_git_policy".to_string(),
                    role: AgentRole::Coder,
                    title: "Scoped Git policy".to_string(),
                    objective: "Stage only files in task scope".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_scoped_git_policy",
            RuntimeCommand::StartAgentTask {
                task_id: "task_scoped_git_policy".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(5), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: true,
                    ..
                } if name == "git_add"
            )
        }) && events
            .iter()
            .filter(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::ToolCallFinished {
                        success: false,
                        evidence: Some(evidence),
                        ..
                    } if evidence.summary.contains("reason: RuleDeny")
                )
            })
            .count()
            >= 2
    });
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ApprovalRequested { .. }))
    );
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "git_add"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "unscoped git_add should be denied by role policy: {events:#?}"
    );
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished {
                    name,
                    success: false,
                    evidence: Some(evidence),
                    ..
                } if name == "git_commit"
                    && evidence.summary.contains("reason: RuleDeny")
            )
        }),
        "git_commit should be denied by role policy: {events:#?}"
    );

    let staged = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    let staged = String::from_utf8_lossy(&staged.stdout);
    assert!(staged.contains("crates/runtime/src/lib.rs"));
    assert!(!staged.contains("docs/release.md"));
}

#[test]
fn runtime_supervisor_accepts_and_rejects_merge_gate_decisions() {
    let cwd = temp_dir("runtime_supervisor_merge_gate_decision_cwd");
    let home = temp_dir("runtime_supervisor_merge_gate_decision_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "Plan: produce merge-gate evidence.".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Decide merge gate".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_planner".to_string(),
                    role: AgentRole::Planner,
                    title: "Plan merge decision".to_string(),
                    objective: "Create evidence for a gate decision".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["plan".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start",
            RuntimeCommand::StartAgentTask {
                task_id: "task_planner".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_planner"
                        && !gate.status.is_open()
            )
        })
    });

    supervisor
        .send_command(
            "cmd_reject_gate",
            RuntimeCommand::RejectMergeGate {
                gate_id: "gate-task_planner".to_string(),
                reason: "needs reviewer evidence".to_string(),
            },
        )
        .unwrap();
    let rejected = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_planner"
                        && gate.status == MergeGateStatus::NeedsChanges
                        && gate.decision.as_deref() == Some("needs reviewer evidence")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_planner"
                        && task.decision.as_deref() == Some("needs reviewer evidence")
            )
        })
    });
    assert!(rejected.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_reject_gate"
        )
    }));

    supervisor
        .send_command(
            "cmd_accept_gate",
            RuntimeCommand::AcceptMergeGate {
                gate_id: "gate-task_planner".to_string(),
                decision: Some("accepted after review".to_string()),
            },
        )
        .unwrap();
    let accepted = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_planner"
                        && gate.status == MergeGateStatus::Accepted
                        && gate.decision.as_deref() == Some("accepted after review")
            )
        })
    });
    assert!(accepted.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_accept_gate"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "merge_gate_rejected"
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("reason")
                .is_some_and(|reason| reason == "needs reviewer evidence")
    }));
    assert!(agent_events.iter().any(|event| {
        event.event_type == "merge_gate_accepted"
            && event.task_id.as_deref() == Some("task_planner")
            && event
                .payload
                .get("decision")
                .is_some_and(|decision| decision == "accepted after review")
    }));
}

#[test]
fn runtime_supervisor_rejects_unknown_agent_artifact_evidence() {
    let cwd = temp_dir("runtime_supervisor_unknown_artifact_evidence_cwd");
    let home = temp_dir("runtime_supervisor_unknown_artifact_evidence_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Reject unknown evidence".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_unknown_evidence".to_string(),
                    role: AgentRole::Coder,
                    title: "Produce patch evidence".to_string(),
                    objective: "Produce one patch artifact".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string(), "test_result".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_unknown_evidence",
            RuntimeCommand::StartAgentTask {
                task_id: "task_unknown_evidence".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_unknown_evidence"
                        && gate.status == MergeGateStatus::CollectingEvidence
            )
        })
    });

    supervisor
        .send_command(
            "cmd_accept_unknown_evidence",
            RuntimeCommand::AcceptAgentArtifact {
                gate_id: "gate-task_unknown_evidence".to_string(),
                evidence_id: "manual-test_result".to_string(),
                decision: Some("unknown evidence should not count".to_string()),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::CommandRejected { command_id, .. }
                    if command_id == "cmd_accept_unknown_evidence"
            )
        })
    });

    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == "cmd_accept_unknown_evidence"
                    && reason.contains("does not exist")
        )
    }));
    assert!(!events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::MergeGateUpdated { gate }
                if gate.gate_id == "gate-task_unknown_evidence"
                    && gate.status == MergeGateStatus::Accepted
        )
    }));
}

#[test]
fn runtime_supervisor_reduces_merge_gate_from_required_evidence_kinds() {
    let cwd = temp_dir("runtime_supervisor_evidence_reducer_cwd");
    let home = temp_dir("runtime_supervisor_evidence_reducer_home");
    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Collect required evidence".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_release_gate".to_string(),
                    role: AgentRole::ReleaseOperator,
                    title: "Verify release gate".to_string(),
                    objective: "Collect all merge evidence".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec![".".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec![
                        "test_result".to_string(),
                        "review".to_string(),
                        "doc_update".to_string(),
                        "release_artifact".to_string(),
                    ],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    for (command_id, evidence_id, kind, summary) in [
        (
            "cmd_record_test_result",
            "evidence-test",
            "test_result",
            "cargo test -p viden-runtime passed",
        ),
        (
            "cmd_record_review",
            "evidence-review",
            "review",
            "review found no blocking issues",
        ),
        (
            "cmd_record_doc_update",
            "evidence-doc",
            "doc_update",
            "frontend contract docs updated",
        ),
        (
            "cmd_record_release_artifact",
            "evidence-release",
            "release_artifact",
            "release checklist prepared",
        ),
    ] {
        supervisor
            .send_command(
                command_id,
                RuntimeCommand::RecordAgentEvidence {
                    gate_id: "gate-task_release_gate".to_string(),
                    evidence_id: Some(evidence_id.to_string()),
                    kind: kind.to_string(),
                    summary: summary.to_string(),
                    path: None,
                    source: Some("release-gate".to_string()),
                },
            )
            .unwrap();
    }

    let accepted = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_release_gate"
                        && gate.status == MergeGateStatus::Accepted
                        && gate.evidence_ids
                            == vec![
                                "evidence-test".to_string(),
                                "evidence-review".to_string(),
                                "evidence-doc".to_string(),
                                "evidence-release".to_string(),
                            ]
            )
        })
    });
    assert!(accepted.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.id == "evidence-release"
                    && evidence.kind == "release_artifact"
                    && evidence.source.as_deref() == Some("release-gate")
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    for kind in ["test_result", "review", "doc_update", "release_artifact"] {
        assert!(agent_events.iter().any(|event| {
            event.event_type == "agent_evidence_recorded"
                && event.task_id.as_deref() == Some("task_release_gate")
                && event
                    .payload
                    .get("evidence_kind")
                    .is_some_and(|recorded_kind| recorded_kind == kind)
        }));
    }
}

#[test]
fn runtime_supervisor_accepts_rejects_and_merges_agent_artifacts() {
    let cwd = temp_dir("runtime_supervisor_agent_artifact_gate_cwd");
    let home = temp_dir("runtime_supervisor_agent_artifact_gate_home");
    write_test_file(
        &cwd.join("src/lib.rs"),
        "pub const STATUS: &str = \"old\";\n",
    );
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub const STATUS: &str = \"old\";\n+pub const STATUS: &str = \"merged\";\n";
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: patch.to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Gate agent artifact".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_coder".to_string(),
                    role: AgentRole::Coder,
                    title: "Implement runtime artifact gate".to_string(),
                    objective: "Produce a patch artifact for the merge gate".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["crates/runtime".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string(), "test_result".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_coder",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::CollectingEvidence
                        && gate.evidence_ids.iter().any(|id| id == "evidence-task_coder-patch")
            )
        })
    });

    supervisor
        .send_command(
            "cmd_record_test_evidence",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
            },
        )
        .unwrap();
    let recorded_test_evidence =
        collect_events_until(&supervisor, Duration::from_secs(2), |events| {
            events.iter().any(|event| {
                matches!(
                    &event.kind,
                    RuntimeEventKind::MergeGateUpdated { gate }
                        if gate.gate_id == "gate-task_coder"
                            && gate.status == MergeGateStatus::Accepted
                            && gate.evidence_ids
                                == vec![
                                    "evidence-task_coder-patch".to_string(),
                                    "manual-test_result".to_string()
                                ]
                )
            })
        });
    assert!(recorded_test_evidence.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_record_test_evidence"
        )
    }));
    assert!(recorded_test_evidence.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.id == "manual-test_result" && evidence.kind == "test_result"
        )
    }));

    supervisor
        .send_command(
            "cmd_accept_test_artifact",
            RuntimeCommand::AcceptAgentArtifact {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: "manual-test_result".to_string(),
                decision: Some("focused tests passed".to_string()),
            },
        )
        .unwrap();
    let accepted_artifact = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::Accepted
                        && gate.decision.as_deref() == Some("focused tests passed")
            )
        })
    });
    assert!(accepted_artifact.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_accept_test_artifact"
        )
    }));

    supervisor
        .send_command(
            "cmd_reject_artifact",
            RuntimeCommand::RejectAgentArtifact {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: "manual-test_result".to_string(),
                reason: "test output was from the wrong package".to_string(),
            },
        )
        .unwrap();
    let rejected_artifact = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::NeedsChanges
                        && !gate.evidence_ids.iter().any(|id| id == "manual-test_result")
                        && gate.decision.as_deref()
                            == Some("test output was from the wrong package")
            )
        })
    });
    assert!(rejected_artifact.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_reject_artifact"
        )
    }));

    supervisor
        .send_command(
            "cmd_record_test_evidence_again",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "correct focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_merge_patch",
            RuntimeCommand::MergeAgentPatch {
                gate_id: "gate-task_coder".to_string(),
                decision: Some("merge accepted patch".to_string()),
            },
        )
        .unwrap();
    let merged = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder"
                        && gate.status == MergeGateStatus::Merged
                        && gate.decision.as_deref() == Some("merge accepted patch")
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_coder"
                        && task.status == AgentTaskStatus::Applied.as_str()
                        && task.decision.as_deref() == Some("merge accepted patch")
            )
        })
    });
    assert!(!merged.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. }
                if command_id == "cmd_merge_patch"
        )
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "pub const STATUS: &str = \"merged\";\n"
    );

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_artifact_accepted"
            && event.task_id.as_deref() == Some("task_coder")
            && event
                .payload
                .get("evidence_id")
                .is_some_and(|id| id == "manual-test_result")
    }));
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_artifact_rejected"
            && event.task_id.as_deref() == Some("task_coder")
            && event
                .payload
                .get("reason")
                .is_some_and(|reason| reason == "test output was from the wrong package")
    }));
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_patch_merged"
            && event.task_id.as_deref() == Some("task_coder")
            && event
                .payload
                .get("decision")
                .is_some_and(|decision| decision == "merge accepted patch")
    }));
}

#[test]
fn runtime_supervisor_applies_accepted_patch_evidence_to_workspace() {
    let cwd = temp_dir("runtime_supervisor_apply_patch_cwd");
    let home = temp_dir("runtime_supervisor_apply_patch_home");
    write_test_file(
        &cwd.join("src/lib.rs"),
        "pub fn name() -> &'static str {\n    \"old\"\n}\n",
    );
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn name() -> &'static str {\n-    \"old\"\n+    \"new\"\n }\n";
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: patch.to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Apply accepted patch".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_coder_apply".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch src lib".to_string(),
                    objective: "Produce a patch artifact".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string(), "test_result".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_coder_apply",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder_apply".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_apply"
                        && gate.status == MergeGateStatus::CollectingEvidence
                        && gate.evidence_ids.iter().any(|id| id == "evidence-task_coder_apply-patch")
            )
        })
    });

    supervisor
        .send_command(
            "cmd_accept_tests",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder_apply".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_apply"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_merge_patch",
            RuntimeCommand::MergeAgentPatch {
                gate_id: "gate-task_coder_apply".to_string(),
                decision: Some("apply accepted patch".to_string()),
            },
        )
        .unwrap();
    let merged = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_apply"
                        && gate.status == MergeGateStatus::Merged
            )
        })
    });
    assert!(merged.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_merge_patch"
        )
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "pub fn name() -> &'static str {\n    \"new\"\n}\n"
    );

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_patch_merged"
            && event.task_id.as_deref() == Some("task_coder_apply")
            && event
                .payload
                .get("evidence_id")
                .is_some_and(|id| id == "evidence-task_coder_apply-patch")
            && event
                .payload
                .get("changed_files")
                .is_some_and(|files| files == "src/lib.rs")
    }));
}

#[test]
fn runtime_supervisor_reports_patch_conflict_without_modifying_workspace() {
    let cwd = temp_dir("runtime_supervisor_patch_conflict_cwd");
    let home = temp_dir("runtime_supervisor_patch_conflict_home");
    write_test_file(
        &cwd.join("src/lib.rs"),
        "pub fn name() -> &'static str {\n    \"current\"\n}\n",
    );
    let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn name() -> &'static str {\n-    \"old\"\n+    \"new\"\n }\n";
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: patch.to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Report patch conflict".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_coder_conflict".to_string(),
                    role: AgentRole::Coder,
                    title: "Patch src lib".to_string(),
                    objective: "Produce a conflicting patch artifact".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["src".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["patch".to_string(), "test_result".to_string()],
                    permission_policy: "scoped_mutation".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_start_coder_conflict",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder_conflict".to_string(),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_conflict"
                        && gate.status == MergeGateStatus::CollectingEvidence
            )
        })
    });

    supervisor
        .send_command(
            "cmd_accept_tests",
            RuntimeCommand::RecordAgentEvidence {
                gate_id: "gate-task_coder_conflict".to_string(),
                evidence_id: Some("manual-test_result".to_string()),
                kind: "test_result".to_string(),
                summary: "focused tests passed".to_string(),
                path: Some("target/focused-tests.log".to_string()),
                source: Some("tester".to_string()),
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_conflict"
                        && gate.status == MergeGateStatus::Accepted
            )
        })
    });

    supervisor
        .send_command(
            "cmd_merge_patch",
            RuntimeCommand::MergeAgentPatch {
                gate_id: "gate-task_coder_conflict".to_string(),
                decision: Some("apply conflicting patch".to_string()),
            },
        )
        .unwrap();
    let conflicted = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::MergeGateUpdated { gate }
                    if gate.gate_id == "gate-task_coder_conflict"
                        && gate.status == MergeGateStatus::NeedsChanges
                        && gate.decision.as_deref().is_some_and(|decision| {
                            decision.contains("patch conflict")
                        })
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_coder_conflict"
                        && task.status == AgentTaskStatus::NeedsInput.as_str()
                        && task.next_action.as_ref().is_some_and(|action| {
                            action.command.as_deref() == Some("/agent start task_coder_conflict")
                        })
            )
        })
    });
    assert!(conflicted.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::TaskUpdated { task }
                if task.id == "task_coder_conflict"
                    && task.status == AgentTaskStatus::NeedsInput.as_str()
                    && task.next_action.as_ref().is_some_and(|action| {
                        action.command.as_deref() == Some("/agent start task_coder_conflict")
                    })
        )
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("src/lib.rs")).unwrap(),
        "pub fn name() -> &'static str {\n    \"current\"\n}\n"
    );

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_patch_conflict"
            && event.task_id.as_deref() == Some("task_coder_conflict")
            && event
                .payload
                .get("reason")
                .is_some_and(|reason| reason.contains("patch conflict"))
    }));
}

#[test]
fn runtime_supervisor_classifies_agent_task_provider_failures() {
    let cwd = temp_dir("runtime_supervisor_agent_failure_cwd");
    let home = temp_dir("runtime_supervisor_agent_failure_home");
    let provider = Box::new(FailingProvider {
        error: "API error (413): deepseek returned HTTP 413".to_string(),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Classify provider failure".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_tester".to_string(),
                    role: AgentRole::Tester,
                    title: "Run failing provider".to_string(),
                    objective: "Classify provider failure metadata".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: Vec::new(),
                    context_bundle_id: None,
                    required_evidence: vec!["test_result".to_string()],
                    permission_policy: "ask".to_string(),
                }],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start_failure",
            RuntimeCommand::StartAgentTask {
                task_id: "task_tester".to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_tester"
                        && task.status == AgentTaskStatus::Failed.as_str()
                        && task.next_action.as_ref().is_some_and(|action| {
                            action.label == "retry agent task"
                                && action.command.as_deref() == Some("/agent start task_tester")
                                && action.reason.as_deref().is_some_and(|reason| {
                                    reason.contains("request_too_large")
                                })
                        })
            )
        }) && events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::Error { error }
                    if error.message.contains("HTTP 413")
                        && error.hint.as_deref().is_some_and(|hint| {
                            hint.contains("compact provider context")
                        })
            )
        })
    });
    assert!(events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_agent_start_failure"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_task_failed"
            && event.task_id.as_deref() == Some("task_tester")
            && event
                .payload
                .get("failure_class")
                .is_some_and(|class| class == "request_too_large")
            && event
                .payload
                .get("recovery_suggestion")
                .is_some_and(|suggestion| suggestion.contains("compact provider context"))
    }));
}

#[test]
fn runtime_supervisor_cancels_queued_agent_task_with_durable_event() {
    let cwd = temp_dir("runtime_supervisor_cancel_queued_agent_cwd");
    let home = temp_dir("runtime_supervisor_cancel_queued_agent_home");
    let provider = Box::new(SequenceProvider::new(Vec::new()));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Cancel queued task".to_string(),
                tasks: vec![AgentDagTaskSpec {
                    task_id: "task_docs".to_string(),
                    role: AgentRole::DocWriter,
                    title: "Document cancellation".to_string(),
                    objective: "Prove explicit task cancellation is auditable".to_string(),
                    dependencies: Vec::new(),
                    workspace: None,
                    file_scope: vec!["docs".to_string()],
                    context_bundle_id: None,
                    required_evidence: vec!["doc_update".to_string()],
                    permission_policy: "read_only".to_string(),
                }],
            },
        )
        .unwrap();
    let dag_events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });
    let dag_id = dag_events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::AgentDagUpdated { dag } => Some(dag.dag_id.clone()),
            _ => None,
        })
        .unwrap();

    supervisor
        .send_command(
            "cmd_cancel_agent",
            RuntimeCommand::CancelAgentTask {
                task_id: "task_docs".to_string(),
            },
        )
        .unwrap();
    let cancelled = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_docs"
                        && task.status == AgentTaskStatus::Cancelled.as_str()
            )
        })
    });
    assert!(cancelled.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CommandAccepted { command_id, .. }
                if command_id == "cmd_cancel_agent"
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_task_cancelled"
            && event.dag_id == dag_id
            && event.task_id.as_deref() == Some("task_docs")
            && event
                .payload
                .get("reason")
                .is_some_and(|reason| reason == "cancelled by operator")
    }));
}

#[test]
fn runtime_supervisor_blocks_agent_task_until_dependencies_complete() {
    let cwd = temp_dir("runtime_supervisor_agent_deps_cwd");
    let home = temp_dir("runtime_supervisor_agent_deps_home");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "should not run".to_string(),
        },
        ModelEvent::Done,
    ]]));
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home.clone())).unwrap();
    let supervisor = RuntimeSupervisor::start(engine);

    supervisor
        .send_command(
            "cmd_agent_dag",
            RuntimeCommand::StartAgentDag {
                goal: "Respect dependencies".to_string(),
                tasks: vec![
                    AgentDagTaskSpec {
                        task_id: "task_planner".to_string(),
                        role: AgentRole::Planner,
                        title: "Plan first".to_string(),
                        objective: "Plan the work".to_string(),
                        dependencies: Vec::new(),
                        workspace: None,
                        file_scope: Vec::new(),
                        context_bundle_id: None,
                        required_evidence: vec!["plan".to_string()],
                        permission_policy: "read_only".to_string(),
                    },
                    AgentDagTaskSpec {
                        task_id: "task_coder".to_string(),
                        role: AgentRole::Coder,
                        title: "Code second".to_string(),
                        objective: "Implement after planning".to_string(),
                        dependencies: vec!["task_planner".to_string()],
                        workspace: None,
                        file_scope: Vec::new(),
                        context_bundle_id: None,
                        required_evidence: vec!["patch".to_string()],
                        permission_policy: "ask".to_string(),
                    },
                ],
            },
        )
        .unwrap();
    let _ = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::AgentDagUpdated { .. }))
    });

    supervisor
        .send_command(
            "cmd_agent_start_blocked",
            RuntimeCommand::StartAgentTask {
                task_id: "task_coder".to_string(),
            },
        )
        .unwrap();

    let events = collect_events_until(&supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::TaskUpdated { task }
                    if task.id == "task_coder"
                        && task.status == "blocked"
                        && task.activity.contains("waiting for dependency")
            )
        })
    });

    assert!(!events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { content, .. }
                if content.contains("should not run")
        )
    }));

    let store = WorkflowStore::new(home, &cwd).unwrap();
    let agent_events = store.load_agent_events().unwrap();
    let dag_id = agent_events
        .iter()
        .find(|event| event.event_type == "agent_dag_created")
        .map(|event| event.dag_id.clone())
        .expect("persisted agent DAG event");
    assert!(agent_events.iter().any(|event| {
        event.event_type == "agent_task_blocked"
            && event.dag_id == dag_id
            && event.task_id.as_deref() == Some("task_coder")
            && event
                .payload
                .get("dependency")
                .is_some_and(|dependency| dependency == "task_planner")
    }));
    assert!(
        !agent_events
            .iter()
            .any(|event| { event.event_type == "agent_task_blocked" && event.dag_id == "runtime" })
    );
}

fn wait_until(condition: impl Fn() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "condition did not become true before timeout"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn collect_events_until(
    supervisor: &RuntimeSupervisor,
    timeout: Duration,
    done: impl Fn(&[RuntimeEvent]) -> bool,
) -> Vec<RuntimeEvent> {
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < timeout {
        if let Some(event) = supervisor.recv_event_timeout(Duration::from_millis(50)) {
            events.push(event);
            if done(&events) {
                break;
            }
        }
    }
    events
}

fn start_agent_task_and_capture_context(
    supervisor: &RuntimeSupervisor,
    task_id: &str,
) -> ContextBundleRecord {
    supervisor
        .send_command(
            format!("cmd_start_{task_id}"),
            RuntimeCommand::StartAgentTask {
                task_id: task_id.to_string(),
            },
        )
        .unwrap();
    let events = collect_events_until(supervisor, Duration::from_secs(2), |events| {
        events.iter().any(|event| {
            matches!(
                &event.kind,
                RuntimeEventKind::ContextUpdated { context } if context.task_id == task_id
            )
        })
    });
    events
        .into_iter()
        .find_map(|event| match event.kind {
            RuntimeEventKind::ContextUpdated { context } if context.task_id == task_id => {
                Some(context)
            }
            _ => None,
        })
        .expect("agent task context event")
}

fn assert_context_source(context: &ContextBundleRecord, name: &str, kind: &str) {
    assert!(
        context
            .sources
            .iter()
            .any(|source| source.name == name && source.kind == kind),
        "missing source {name}/{kind} in {:?}",
        context.sources
    );
}

fn context_source_summary(context: &ContextBundleRecord, name: &str, kind: &str) -> String {
    context
        .sources
        .iter()
        .find(|source| source.name == name && source.kind == kind)
        .unwrap_or_else(|| panic!("missing context source {name}/{kind}"))
        .summary
        .clone()
}

fn provider_manifest(request: &ModelRequest) -> String {
    request
        .messages
        .iter()
        .find(|message| message.content.contains("Viden ContextBundle"))
        .expect("provider context manifest")
        .content
        .clone()
}

fn write_test_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("test file has parent")).unwrap();
    fs::write(path, contents).unwrap();
}

fn init_git_repo(cwd: &Path) {
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(init.status.success());
    for (key, value) in [
        ("user.email", "viden@example.com"),
        ("user.name", "Viden Test"),
    ] {
        let output = std::process::Command::new("git")
            .args(["config", key, value])
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    let add = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(add.status.success());
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(commit.status.success());
}

fn fake_lsp_registry(workdir: &Path) -> LspServerRegistry {
    let script_path = workdir.join("fake_lsp_server.py");
    fs::write(
        &script_path,
        r#"import json
import sys

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line == b"\r\n":
            break
        key, value = line.decode("utf-8").split(":", 1)
        headers[key.lower()] = value.strip()
    body = sys.stdin.buffer.read(int(headers["content-length"]))
    return json.loads(body.decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}})
    elif method == "initialized":
        continue
    elif method == "textDocument/didOpen":
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": message["params"]["textDocument"]["uri"],
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 16},
                        "end": {"line": 1, "character": 23}
                    },
                    "severity": 1,
                    "source": "fake-lsp",
                    "code": "E100",
                    "message": "fake diagnostic"
                }]
            }
        })
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": message["id"], "result": None})
    elif method == "exit":
        break
"#,
    )
    .unwrap();

    LspServerRegistry::new(vec![LspServerConfig {
        id: "fake-rust".to_string(),
        command: std::env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
        args: vec![script_path.to_string_lossy().to_string()],
        file_extensions: vec!["rs".to_string()],
    }])
}
