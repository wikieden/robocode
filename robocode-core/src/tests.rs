use std::collections::VecDeque;
use std::fs;

use robocode_model::ModelProvider;
use robocode_types::{
    ApprovalResponse, ModelEvent, ModelRequest, PermissionMode, ToolCall, ToolInput,
};

use super::*;

mod git_command_tests;
mod lsp_render_tests;
mod session_command_tests;
mod workflow_command_tests;

struct SequenceProvider {
    model: String,
    turns: VecDeque<Vec<ModelEvent>>,
}

impl SequenceProvider {
    fn new(turns: Vec<Vec<ModelEvent>>) -> Self {
        Self {
            model: "test-model".to_string(),
            turns: turns.into(),
        }
    }
}

impl ModelProvider for SequenceProvider {
    fn provider_name(&self) -> &str {
        "sequence"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn next_events(&mut self, _request: &ModelRequest) -> Result<Vec<ModelEvent>, String> {
        Ok(self
            .turns
            .pop_front()
            .unwrap_or_else(|| vec![ModelEvent::Done]))
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("robocode_core_{name}_{}", fresh_id("tmp")));
    fs::create_dir_all(&dir).unwrap();
    dir
}

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
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::ToolResult(text) if text.contains("hello")))
    );
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Assistant(text) if text.contains("Tool finished"))
    ));
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
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::System(text) if text.contains("Permission denied"))
    ));
}

#[test]
fn web_help_command_is_available() {
    let home = temp_dir("web_help_home");
    let cwd = temp_dir("web_help_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/web", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Web commands:")
    )));
}

#[test]
fn status_command_reports_current_runtime_state() {
    let home = temp_dir("status_home");
    let cwd = temp_dir("status_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/status", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Session:")
                && text.contains("Provider:")
                && text.contains("Permission mode:")
                && text.contains("Transcript:")
                && text.contains("Index:")
    )));
}

#[test]
fn config_command_reports_runtime_configuration_summary() {
    let home = temp_dir("config_home");
    let cwd = temp_dir("config_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/config", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Runtime configuration:")
                && text.contains("provider=")
                && text.contains("Loaded config files:")
    )));
}

#[test]
fn doctor_command_reports_dependency_checks() {
    let home = temp_dir("doctor_home");
    let cwd = temp_dir("doctor_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/doctor", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Environment diagnostics:")
                && text.contains("git:")
                && text.contains("rg:")
                && text.contains("sqlite3:")
                && text.contains("curl:")
    )));
}

#[test]
fn doctor_report_renders_injected_dependency_probe_results() {
    let report = DoctorReport::from_probe(|tool| match tool {
        "git" => DependencyStatus::Ok,
        "rg" => DependencyStatus::Missing,
        "sqlite3" => DependencyStatus::NotRequired,
        "curl" => DependencyStatus::Ok,
        other => panic!("unexpected dependency probe for {other}"),
    });

    let rendered = report.render();

    assert!(rendered.contains("Environment diagnostics:"));
    assert!(rendered.contains("git: ok"));
    assert!(rendered.contains("rg: missing"));
    assert!(rendered.contains("sqlite3: not required for current path"));
    assert!(rendered.contains("curl: ok"));
}

#[test]
fn help_output_lists_lsp_commands() {
    let home = temp_dir("lsp_help_home");
    let cwd = temp_dir("lsp_help_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/help", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("/lsp status")
                && text.contains("/lsp diagnostics")
                && text.contains("/lsp symbols")
                && text.contains("/lsp references")
    )));
}

#[test]
fn lsp_status_reports_configured_servers() {
    let home = temp_dir("lsp_status_home");
    let cwd = temp_dir("lsp_status_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/lsp status", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("LSP status:")
                && text.contains("configured: rust-analyzer")
                && text.contains("cached_sessions: 0")
                && text.contains("open_documents: 0")
    )));
}

#[test]
fn lsp_diagnostics_unconfigured_path_fails_cleanly() {
    let home = temp_dir("lsp_diagnostics_home");
    let cwd = temp_dir("lsp_diagnostics_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/lsp diagnostics README.md", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("LSP error:")
                && text.contains("No configured language server for README.md")
    )));
}

#[test]
fn lsp_references_validates_position_arguments() {
    let home = temp_dir("lsp_refs_home");
    let cwd = temp_dir("lsp_refs_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let error = engine
        .process_input_with_approval("/lsp references src/lib.rs abc 1", &mut approver)
        .unwrap_err();
    assert!(error.contains("line and character must be zero-based integers"));
}

#[test]
fn lsp_command_entries_are_written_to_transcript() {
    let home = temp_dir("lsp_transcript_home");
    let cwd = temp_dir("lsp_transcript_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine
        .process_input_with_approval("/lsp status", &mut approver)
        .unwrap();
    let entries = engine.store.load_entries().unwrap();
    assert!(entries.iter().any(|entry| matches!(
        entry,
        TranscriptEntry::Command { entry } if entry.name == "lsp"
    )));
}

#[test]
fn help_output_lists_runtime_inspection_commands() {
    let home = temp_dir("help_home");
    let cwd = temp_dir("help_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/help", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("/status")
                && text.contains("/config")
                && text.contains("/doctor")
    )));
}

#[test]
fn help_output_groups_commands_by_purpose() {
    let home = temp_dir("help_groups_home");
    let cwd = temp_dir("help_groups_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/help", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Runtime:")
                && text.contains("Sessions:")
                && text.contains("Repository and web:")
    )));
}
