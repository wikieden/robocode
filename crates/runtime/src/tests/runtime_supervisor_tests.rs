use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use viden_provider::{ModelProvider, ModelRequestControl};
use viden_types::{
    ApprovalResponse, ModelEvent, ModelRequest, RuntimeCommand, RuntimeEvent, RuntimeEventKind,
    ToolCall, ToolInput, WorkMode,
};

use crate::{RuntimeSupervisor, SessionEngine};

use super::{SequenceProvider, temp_dir};

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

#[test]
fn runtime_supervisor_cancels_active_provider_turn_and_keeps_worker_alive() {
    let cwd = temp_dir("runtime_supervisor_cancel_cwd");
    let home = temp_dir("runtime_supervisor_cancel_home");
    let entered = Arc::new(AtomicBool::new(false));
    let provider = Box::new(BlockingProvider {
        entered: Arc::clone(&entered),
    });
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
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
    let engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
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
