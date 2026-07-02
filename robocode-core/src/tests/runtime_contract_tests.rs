use std::fs;

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
    let lane_dir = cwd.join(".robocode");
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
