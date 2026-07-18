use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{EngineEvent, SessionEngine};
use viden_lsp::{LspRuntime, LspServerConfig, LspServerRegistry};
use viden_provider::{ModelProvider, ModelRequestControl};
use viden_types::{
    AgentTaskStatus, ApprovalResponse, ModelEvent, ModelRequest, ModelUsage, PermissionMode,
    ToolCall, ToolInput, WorkMode,
};

use super::{SequenceProvider, temp_dir};

#[test]
fn single_turn_text_response_is_recorded() {
    let home = temp_dir("single_home");
    let cwd = temp_dir("single_cwd");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "hello from test".to_string(),
        },
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let events = engine
        .process_input_with_approval("hi", &mut approver)
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::Assistant(text) if text.contains("hello")))
    );
    let snapshot = engine.agent_task_snapshot();
    assert!(snapshot.iter().any(|task| {
        task.kind == "provider"
            && task.status == AgentTaskStatus::Done.as_str()
            && task.title == "hi"
    }));
}

#[test]
fn provider_telemetry_records_successful_model_requests() {
    let home = temp_dir("telemetry_success_home");
    let cwd = temp_dir("telemetry_success_cwd");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "telemetry response".to_string(),
        },
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("measure provider", &mut approver)
        .unwrap();

    let telemetry = engine.provider_telemetry();
    assert_eq!(telemetry.request_count, 1);
    assert_eq!(telemetry.success_count, 1);
    assert_eq!(telemetry.failure_count, 0);
    assert_eq!(telemetry.last_event_count, 1);
    assert!(telemetry.last_latency_ms.is_some());
    assert!(telemetry.average_latency_ms.is_some());
    assert_eq!(telemetry.last_error, None);
}

#[test]
fn provider_telemetry_records_model_usage_when_provider_reports_it() {
    let home = temp_dir("telemetry_usage_home");
    let cwd = temp_dir("telemetry_usage_cwd");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "usage response".to_string(),
        },
        ModelEvent::Usage(ModelUsage {
            input_tokens: Some(13),
            output_tokens: Some(7),
            total_tokens: Some(20),
            cost_micro_usd: None,
        }),
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("measure usage", &mut approver)
        .unwrap();

    let telemetry = engine.provider_telemetry();
    assert_eq!(telemetry.last_input_tokens, Some(13));
    assert_eq!(telemetry.last_output_tokens, Some(7));
    assert_eq!(telemetry.last_total_tokens, Some(20));
    assert_eq!(telemetry.total_input_tokens, 13);
    assert_eq!(telemetry.total_output_tokens, 7);
    assert_eq!(telemetry.total_tokens, 20);
    assert_eq!(telemetry.last_cost_micro_usd, None);
}

#[test]
fn provider_turn_uses_ephemeral_context_bundle_without_transcript_mutation() {
    let home = temp_dir("context_bundle_home");
    let cwd = temp_dir("context_bundle_cwd");
    fs::write(cwd.join("notes.txt"), "draft context").unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingSequenceProvider::new(
        vec![vec![ModelEvent::AssistantText {
            content: "context-aware response".to_string(),
        }]],
        Arc::clone(&requests),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("/brief summarize workspace safely", &mut approver)
        .unwrap();
    engine
        .process_input_with_approval("/brief steering init", &mut approver)
        .unwrap();
    engine
        .process_input_with_approval("summarize the workspace", &mut approver)
        .unwrap();

    let requests = requests.lock().unwrap();
    let request = requests.first().expect("provider request");
    assert!(matches!(request.messages.last(), Some(message)
            if message.role == viden_types::Role::User
                && message.content == "summarize the workspace"));
    let context = request
        .messages
        .iter()
        .find(|message| message.content.contains("Viden ContextBundle"))
        .expect("ephemeral context message");
    assert_eq!(context.role, viden_types::Role::System);
    assert!(context.content.contains("Viden ContextBundle"));
    assert!(context.content.contains("Policy: v1-priority-budget"));
    assert!(context.content.contains("Omitted sources:"));
    assert!(context.content.contains("Context pressure:"));
    assert!(context.content.contains("workspace"));
    assert!(context.content.contains("active-brief"));
    assert!(context.content.contains("project-steering"));

    let bundle = engine
        .provider_context_bundle()
        .expect("provider context bundle");
    assert!(
        bundle
            .sources
            .iter()
            .any(|source| source.name == "user-task")
    );
    assert!(
        bundle
            .sources
            .iter()
            .any(|source| source.name == "workspace")
    );
    assert!(
        bundle
            .sources
            .iter()
            .any(|source| source.name == "active-brief"
                && source.summary.contains("summarize workspace safely"))
    );
    assert!(
        bundle
            .sources
            .iter()
            .any(|source| source.name == "project-steering")
    );
    assert_eq!(bundle.policy, "v1-priority-budget");
    assert!(bundle.sources.iter().all(|source| source.priority > 0));
    assert!(engine.agent_task_snapshot().iter().any(|task| {
        task.kind == "provider"
            && task
                .evidence
                .iter()
                .any(|row| row.starts_with("context_pressure "))
            && task
                .evidence
                .iter()
                .any(|row| row == "context_policy v1-priority-budget")
    }));

    let output = engine
        .process_input_with_approval("/context", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("ContextBundle:")
                && text.contains("Policy: v1-priority-budget")
                && text.contains("Sources by priority:")
                && text.contains("Omitted sources:")
    )));
}

#[test]
fn provider_turn_compacts_long_transcript_before_request() {
    let home = temp_dir("compact_provider_request_home");
    let cwd = temp_dir("compact_provider_request_cwd");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let turns = (0..26)
        .map(|index| {
            vec![ModelEvent::AssistantText {
                content: format!("assistant response {index} {}", "r".repeat(5_000)),
            }]
        })
        .collect::<Vec<_>>();
    let provider = Box::new(RecordingSequenceProvider::new(turns, Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    for index in 0..25 {
        engine
            .process_input_with_approval(
                &format!(
                    "older prompt {index} ancient-marker-{index} {}",
                    "u".repeat(5_000)
                ),
                &mut approver,
            )
            .unwrap();
    }
    engine
        .process_input_with_approval("final concise task", &mut approver)
        .unwrap();

    let requests = requests.lock().unwrap();
    let request = requests.last().expect("final provider request");
    let combined = request
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(matches!(request.messages.last(), Some(message)
            if message.role == viden_types::Role::User
                && message.content == "final concise task"));
    assert!(
        combined.len() < 60_000,
        "provider request should be compacted, got {} chars",
        combined.len()
    );
    assert!(combined.contains("Viden ContextBundle"));
    assert!(combined.contains("Viden compacted transcript summary"));
    assert!(
        !combined.contains("ancient-marker-0"),
        "oldest transcript details should not be replayed verbatim"
    );
}

#[test]
fn provider_turn_retries_request_too_large_with_smaller_context() {
    let home = temp_dir("retry_413_home");
    let cwd = temp_dir("retry_413_cwd");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RequestTooLargeOnceProvider::new(Arc::clone(&requests)));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .process_input_with_approval(
            &format!("summarize this oversized task {}", "detail ".repeat(6_000)),
            &mut approver,
        )
        .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::System(text)
            if text.contains("Provider request was too large")
                && text.contains("retrying with compacted context")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Assistant(text) if text.contains("retried successfully")
    )));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let first_chars = request_chars(&requests[0]);
    let second_chars = request_chars(&requests[1]);
    assert!(
        second_chars < first_chars,
        "retry request should shrink: first={first_chars} second={second_chars}"
    );
    assert!(requests[1].messages.iter().any(|message| {
        message
            .content
            .contains("Viden compacted provider request after a request-too-large error")
    }));
    let telemetry = engine.provider_telemetry();
    assert_eq!(telemetry.failure_count, 1);
    assert_eq!(telemetry.success_count, 1);
}

#[test]
fn provider_turn_retries_context_overflow_once() {
    let home = temp_dir("retry_context_overflow_home");
    let cwd = temp_dir("retry_context_overflow_cwd");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RequestTooLargeOnceProvider::with_error(
        Arc::clone(&requests),
        "maximum context length exceeded".to_string(),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .process_input_with_approval("trigger context overflow retry", &mut approver)
        .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Assistant(text) if text.contains("retried successfully")
    )));
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[test]
fn provider_turn_second_context_size_failure_does_not_loop() {
    let home = temp_dir("retry_second_context_failure_home");
    let cwd = temp_dir("retry_second_context_failure_cwd");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(AlwaysFailingRecordingProvider {
        error: "context_length exceeded".to_string(),
        requests: Arc::clone(&requests),
    });
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("trigger repeated context overflow", &mut approver)
        .unwrap_err();

    assert!(err.contains("context_length exceeded"));
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[test]
fn provider_turn_does_not_retry_unrelated_failure() {
    let home = temp_dir("retry_unrelated_failure_home");
    let cwd = temp_dir("retry_unrelated_failure_cwd");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(AlwaysFailingRecordingProvider {
        error: "provider down".to_string(),
        requests: Arc::clone(&requests),
    });
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("trigger unrelated failure", &mut approver)
        .unwrap_err();

    assert!(err.contains("provider down"));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn failed_tool_execution_is_returned_to_provider_without_ending_turn() {
    let home = temp_dir("failed_tool_result_home");
    let cwd = temp_dir("failed_tool_result_cwd");
    fs::create_dir_all(cwd.join("src")).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingSequenceProvider::new(
        vec![
            vec![ModelEvent::ToolCall(ToolCall {
                id: "tool_read_dir".to_string(),
                name: "read_file".to_string(),
                input: ToolInput::from([("path".to_string(), "src".to_string())]),
            })],
            vec![ModelEvent::AssistantText {
                content: "I should inspect files with glob instead.".to_string(),
            }],
        ],
        Arc::clone(&requests),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .process_input_with_approval("read the src directory", &mut approver)
        .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::ToolResult { output: text, .. }
            if text.contains("Tool `read_file` failed")
                && text.contains("is a directory")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Assistant(text) if text.contains("glob instead")
    )));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let replay = &requests[1].messages;
    assert!(replay.iter().any(|message| {
        message.role == viden_types::Role::Tool
            && message.tool_call_id.as_deref() == Some("tool_read_dir")
            && message.content.contains("Tool `read_file` failed")
    }));
}

fn request_chars(request: &ModelRequest) -> usize {
    request
        .messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum()
}

#[test]
fn provider_telemetry_records_failed_model_requests() {
    let home = temp_dir("telemetry_failure_home");
    let cwd = temp_dir("telemetry_failure_cwd");
    let provider = Box::new(FailingProvider);
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("measure provider failure", &mut approver)
        .unwrap_err();

    let telemetry = engine.provider_telemetry();
    assert_eq!(err, "provider down");
    assert_eq!(telemetry.request_count, 1);
    assert_eq!(telemetry.success_count, 0);
    assert_eq!(telemetry.failure_count, 1);
    assert_eq!(telemetry.last_event_count, 0);
    assert_eq!(telemetry.last_error.as_deref(), Some("provider down"));
}

#[test]
fn provider_model_failures_include_switch_model_recovery_prompt() {
    let home = temp_dir("model_recovery_home");
    let cwd = temp_dir("model_recovery_cwd");
    let provider = Box::new(ModelFailingProvider);
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.set_provider_runtime(
        viden_provider::ProviderHost::with_builtins(),
        Vec::new(),
        None,
        None,
        90,
        1,
    );
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval("trigger model failure", &mut approver)
        .unwrap_err();

    assert!(err.contains("Provider/model recovery:"), "{err}");
    assert!(err.contains("class: model_unavailable"), "{err}");
    assert!(err.contains("current: deepseek / made-up-model"), "{err}");
    assert!(err.contains("/models deepseek deepseek-v4-flash"), "{err}");
    assert!(err.contains("/connect deepseek"), "{err}");
    assert!(err.contains("/provider doctor deepseek"), "{err}");
    assert!(
        err.contains(
            "scripts/provider-live-smoke.sh --provider deepseek --model deepseek-v4-flash"
        ),
        "{err}"
    );
}

#[test]
fn cancelled_model_request_stops_before_provider_turn() {
    let home = temp_dir("cancel_home");
    let cwd = temp_dir("cancel_cwd");
    let provider = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "should not be observed".to_string(),
        },
    ]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let control = ModelRequestControl::new();
    control.cancel();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let err = engine
        .process_input_with_approval_and_control("hi", &mut approver, &control)
        .unwrap_err();

    assert_eq!(err, "Model request cancelled");
}

#[test]
fn tool_loop_executes_and_reinjects_result() {
    let home = temp_dir("tool_home");
    let cwd = temp_dir("tool_cwd");
    fs::write(cwd.join("sample.txt"), "hello").unwrap();
    let mut read_input = ToolInput::new();
    read_input.insert("path".to_string(), "sample.txt".to_string());
    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::ToolCall(ToolCall {
            id: "tool_read".to_string(),
            name: "read_file".to_string(),
            input: read_input,
        })],
        vec![ModelEvent::AssistantText {
            content: "Tool finished".to_string(),
        }],
    ]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let events = engine
        .process_input_with_approval("read it", &mut approver)
        .unwrap();
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::ToolResult { output, .. } if output.contains("hello"))
    ));
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Tool finished"))
    ));
    let snapshot = engine.agent_task_snapshot();
    assert!(snapshot.iter().any(|task| {
        task.id == "tool-tool_read"
            && task.kind == "tool"
            && task.status == AgentTaskStatus::Done.as_str()
            && task.evidence.iter().any(|item| item == "success true")
    }));
}

#[test]
fn post_edit_lsp_diagnostics_are_reinjected_after_file_writes() {
    let home = temp_dir("post_edit_lsp_home");
    let cwd = temp_dir("post_edit_lsp_cwd");
    let fake_lsp_dir = temp_dir("post_edit_lsp_server");
    let mut write_input = ToolInput::new();
    write_input.insert("path".to_string(), "src/main.rs".to_string());
    write_input.insert(
        "content".to_string(),
        "fn main() {\n    let value = missing;\n}\n".to_string(),
    );
    let provider = Box::new(SequenceProvider::new(vec![
        vec![ModelEvent::ToolCall(ToolCall {
            id: "tool_write".to_string(),
            name: "write_file".to_string(),
            input: write_input,
        })],
        vec![ModelEvent::AssistantText {
            content: "Saw diagnostics".to_string(),
        }],
    ]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.lsp_runtime = Arc::new(LspRuntime::new(fake_lsp_registry(&fake_lsp_dir)));
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let events = engine
        .process_input_with_approval("write broken rust", &mut approver)
        .unwrap();

    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::ToolResult { output, .. } if output.contains("src/main.rs"))
    ));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::System(text)
            if text.contains("Post-edit LSP diagnostics after `write_file`")
                && text.contains("LSP diagnostics:")
                && text.contains("fake-lsp/E100")
                && text.contains("fake diagnostic")))
    );
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Saw diagnostics"))
    ));
}

#[test]
fn background_lsp_diagnostics_snapshot_renders_successful_paths() {
    let home = temp_dir("background_lsp_home");
    let cwd = temp_dir("background_lsp_cwd");
    let fake_lsp_dir = temp_dir("background_lsp_server");
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/main.rs"), "fn main() {}\n").unwrap();
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    engine.lsp_runtime = Arc::new(LspRuntime::new(fake_lsp_registry(&fake_lsp_dir)));

    let receiver = engine.spawn_lsp_diagnostics_snapshot(vec!["src/main.rs".to_string()]);
    let rendered = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("background diagnostics result")
        .expect("rendered diagnostics");

    assert!(rendered.contains("LSP diagnostics:"));
    assert!(rendered.contains("fake-lsp/E100"));
    assert!(rendered.contains("fake diagnostic"));
}

#[test]
fn plan_mode_blocks_mutating_tools() {
    let home = temp_dir("plan_home");
    let cwd = temp_dir("plan_cwd");
    let mut write_input = ToolInput::new();
    write_input.insert("path".to_string(), "a.txt".to_string());
    write_input.insert("content".to_string(), "new".to_string());
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::ToolCall(
        ToolCall {
            id: "tool_write".to_string(),
            name: "write_file".to_string(),
            input: write_input,
        },
    )]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();
    assert_eq!(engine.mode(), PermissionMode::Plan);
    let events = engine
        .process_input_with_approval("write a file", &mut approver)
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::System(text)
            if text.contains("Permission decision:")
                && text.contains("Summary: decision=deny")
                && text.contains("tool: write_file")
                && text.contains("reason: PlanMode")
                && text.contains("message: write_file is blocked while plan mode is active")))
    );
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::ToolResult { output, .. }
            if output.contains("Permission decision:")
                && output.contains("decision=deny")
                && output.contains("tool: write_file"))
    ));
}

#[test]
fn plan_mode_provider_request_uses_planner_work_mode() {
    let home = temp_dir("plan_request_home");
    let cwd = temp_dir("plan_request_cwd");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingSequenceProvider::new(
        vec![vec![ModelEvent::AssistantText {
            content: "Here is the plan.".to_string(),
        }]],
        Arc::clone(&requests),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();
    engine
        .process_input_with_approval("规划这个功能", &mut approver)
        .unwrap();

    let requests = requests.lock().unwrap();
    let request = requests.first().expect("provider request");
    assert_eq!(request.work_mode, WorkMode::Plan);
    assert_eq!(request.permission_mode, PermissionMode::Plan);
}

#[test]
fn plan_mode_denies_long_shell_commands_before_spawn() {
    let home = temp_dir("plan_long_shell_home");
    let cwd = temp_dir("plan_long_shell_cwd");
    let mut shell_input = ToolInput::new();
    shell_input.insert(
        "command".to_string(),
        format!("printf ok\n# {}", "x".repeat(40 * 1024)),
    );
    let provider = Box::new(SequenceProvider::new(vec![vec![ModelEvent::ToolCall(
        ToolCall {
            id: "tool_shell".to_string(),
            name: "shell".to_string(),
            input: shell_input,
        },
    )]]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();

    let events = engine
        .process_input_with_approval("inspect without mutating", &mut approver)
        .unwrap();
    let rendered = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::System(text) => Some(text.as_str()),
            EngineEvent::ToolResult { output, .. } => Some(output.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("tool: shell"));
    assert!(rendered.contains("reason: PlanMode"));
    assert!(!rendered.contains("Argument list too long"));
}

#[test]
fn denied_tool_calls_are_followed_by_tool_result_messages() {
    let home = temp_dir("deny_tool_result_home");
    let cwd = temp_dir("deny_tool_result_cwd");
    let mut write_input = ToolInput::new();
    write_input.insert("path".to_string(), "a.txt".to_string());
    write_input.insert("content".to_string(), "new".to_string());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Box::new(RecordingSequenceProvider::new(
        vec![
            vec![ModelEvent::ToolCall(ToolCall {
                id: "tool_write".to_string(),
                name: "write_file".to_string(),
                input: write_input,
            })],
            vec![ModelEvent::AssistantText {
                content: "Denied safely".to_string(),
            }],
        ],
        Arc::clone(&requests),
    ));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();

    let events = engine
        .process_input_with_approval("write a file", &mut approver)
        .unwrap();

    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Denied safely"))
    ));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let replay = &requests[1].messages;
    let tool_call_index = replay
        .iter()
        .position(|message| {
            message.tool_call_id.as_deref() == Some("tool_write")
                && message.role == viden_types::Role::Assistant
        })
        .expect("assistant tool call message");
    let tool_result_index = replay
        .iter()
        .position(|message| {
            message.tool_call_id.as_deref() == Some("tool_write")
                && message.role == viden_types::Role::Tool
        })
        .expect("tool result message");
    assert_eq!(tool_result_index, tool_call_index + 1);
}

struct RecordingSequenceProvider {
    model: String,
    turns: std::collections::VecDeque<Vec<ModelEvent>>,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl RecordingSequenceProvider {
    fn new(turns: Vec<Vec<ModelEvent>>, requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
        Self {
            model: "test-model".to_string(),
            turns: turns.into(),
            requests,
        }
    }
}

impl ModelProvider for RecordingSequenceProvider {
    fn provider_name(&self) -> &str {
        "recording-sequence"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(self
            .turns
            .pop_front()
            .unwrap_or_else(|| vec![ModelEvent::Done]))
    }
}

struct FailingProvider;

impl ModelProvider for FailingProvider {
    fn provider_name(&self) -> &str {
        "failing"
    }

    fn model(&self) -> &str {
        "test-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Err("provider down".to_string())
    }
}

struct RequestTooLargeOnceProvider {
    failed_once: bool,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
    error: String,
}

impl RequestTooLargeOnceProvider {
    fn new(requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
        Self::with_error(
            requests,
            "API error (413): deepseek returned HTTP 413".to_string(),
        )
    }

    fn with_error(requests: Arc<Mutex<Vec<ModelRequest>>>, error: String) -> Self {
        Self {
            failed_once: false,
            requests,
            error,
        }
    }
}

impl ModelProvider for RequestTooLargeOnceProvider {
    fn provider_name(&self) -> &str {
        "deepseek"
    }

    fn model(&self) -> &str {
        "deepseek-v4-flash"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.requests.lock().unwrap().push(request.clone());
        if !self.failed_once {
            self.failed_once = true;
            return Err(self.error.clone());
        }
        Ok(vec![ModelEvent::AssistantText {
            content: "retried successfully".to_string(),
        }])
    }
}

struct AlwaysFailingRecordingProvider {
    error: String,
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl ModelProvider for AlwaysFailingRecordingProvider {
    fn provider_name(&self) -> &str {
        "deepseek"
    }

    fn model(&self) -> &str {
        "deepseek-v4-flash"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        self.requests.lock().unwrap().push(request.clone());
        Err(self.error.clone())
    }
}

struct ModelFailingProvider;

impl ModelProvider for ModelFailingProvider {
    fn provider_name(&self) -> &str {
        "deepseek"
    }

    fn model(&self) -> &str {
        "made-up-model"
    }

    fn set_model(&mut self, _model: String) {}

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Err("API error (400): model `made-up-model` not found".to_string())
    }
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
