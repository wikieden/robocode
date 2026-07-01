use robocode_types::{
    ApprovalResponse, ModelEvent, PermissionLevel, RuntimeCommand, RuntimeEventKind,
    RuntimeViewState, ToolCall, ToolInput, WorkMode,
};

use crate::{EngineEvent, SessionEngine};

use super::{SequenceProvider, temp_dir};

fn assert_strictly_increasing_sequences(events: &[robocode_types::RuntimeEvent]) {
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
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let engine_events = engine
        .process_input_with_approval("run printf", &mut approver)
        .unwrap();
    assert!(
        engine_events
            .iter()
            .any(|event| matches!(event, EngineEvent::ToolResult(text) if text.contains("hello")))
    );

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
