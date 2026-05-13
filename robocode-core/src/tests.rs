use std::cell::Cell;
use std::collections::VecDeque;
use std::fs;

use robocode_model::ModelProvider;
use robocode_types::{
    ApprovalResponse, LspRange, ModelEvent, ModelRequest, PermissionMode, ToolCall, ToolInput,
};

use super::*;

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
fn resume_restores_previous_session() {
    let home = temp_dir("resume_home");
    let cwd = temp_dir("resume_cwd");
    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "first reply".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let session_id = engine_a.session_id().to_string();
    engine_a
        .process_input_with_approval("hello", &mut approver)
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let output = engine_b
        .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
        .unwrap();
    assert!(output.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("Resumed session"))
    ));
}

#[test]
fn sessions_command_lists_recent_sessions() {
    let home = temp_dir("sessions_home");
    let cwd = temp_dir("sessions_cwd");

    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "first reply".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine_a
        .process_input_with_approval("inspect the workspace", &mut approver)
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let output = engine_b
        .process_input_with_approval("/sessions", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Sessions for this project")
                && text.contains("inspect the workspace")
    )));
}

#[test]
fn sessions_command_includes_activity_metadata() {
    let home = temp_dir("sessions_meta_home");
    let cwd = temp_dir("sessions_meta_cwd");

    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "session reply".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine_a
        .process_input_with_approval("inspect metadata", &mut approver)
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let output = engine_b
        .process_input_with_approval("/sessions", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("messages=")
                && text.contains("commands=")
                && text.contains("last=")
    )));
}

#[test]
fn sessions_command_marks_current_session() {
    let home = temp_dir("sessions_current_home");
    let cwd = temp_dir("sessions_current_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    let output = engine
        .process_input_with_approval("/sessions", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("[current]")
    )));
}

#[test]
fn ambiguous_resume_prefix_returns_session_list() {
    let home = temp_dir("resume_ambiguous_home");
    let cwd = temp_dir("resume_ambiguous_cwd");
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "reply a".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    engine_a
        .process_input_with_approval("session one", &mut approver)
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "reply b".to_string(),
        },
    ]]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home.clone())).unwrap();
    engine_b
        .process_input_with_approval("session two", &mut approver)
        .unwrap();

    let provider_c = Box::new(SequenceProvider::new(vec![]));
    let mut engine_c = SessionEngine::new_with_home(&cwd, provider_c, Some(home)).unwrap();
    let result = engine_c.process_input_with_approval("/resume session_", &mut approver);
    let error = result.unwrap_err();
    assert!(error.contains("ambiguous"));
    assert!(error.contains("Sessions for this project"));
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
fn render_lsp_symbols_uses_relative_paths_and_kind_labels() {
    let cwd = temp_dir("lsp_render_symbols");
    let rendered = render_lsp_symbols(
        &cwd,
        &[LspSymbol {
            name: "main".to_string(),
            kind: 12,
            path: cwd.join("src/lib.rs").display().to_string(),
            range: LspRange {
                start: LspPosition {
                    line: 3,
                    character: 1,
                },
                end: LspPosition {
                    line: 4,
                    character: 1,
                },
            },
            selection_range: None,
            container_name: Some("impl SessionEngine".to_string()),
        }],
    );
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  main [function] 3:1 in impl SessionEngine"));
}

#[test]
fn render_lsp_symbols_groups_entries_under_file_headers() {
    let cwd = temp_dir("lsp_render_symbols_grouped");
    let rendered = render_lsp_symbols(
        &cwd,
        &[
            LspSymbol {
                name: "main".to_string(),
                kind: 12,
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 3,
                        character: 1,
                    },
                    end: LspPosition {
                        line: 4,
                        character: 1,
                    },
                },
                selection_range: None,
                container_name: None,
            },
            LspSymbol {
                name: "value".to_string(),
                kind: 13,
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 4,
                        character: 5,
                    },
                    end: LspPosition {
                        line: 4,
                        character: 10,
                    },
                },
                selection_range: None,
                container_name: Some("main".to_string()),
            },
            LspSymbol {
                name: "run".to_string(),
                kind: 12,
                path: cwd.join("src/engine.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 8,
                        character: 2,
                    },
                    end: LspPosition {
                        line: 9,
                        character: 2,
                    },
                },
                selection_range: None,
                container_name: Some("Engine".to_string()),
            },
        ],
    );

    assert_eq!(
        rendered,
        [
            "LSP symbols:",
            "src/engine.rs:",
            "  run [function] 8:2 in Engine",
            "src/lib.rs:",
            "  main [function] 3:1",
            "  value [variable] 4:5 in main",
        ]
        .join("\n")
    );
}

#[test]
fn render_lsp_locations_keeps_relative_sorted_lines() {
    let cwd = temp_dir("lsp_render_locations_grouped");
    let rendered = render_lsp_locations(
        &cwd,
        &[
            LspLocation {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 4,
                        character: 5,
                    },
                    end: LspPosition {
                        line: 4,
                        character: 9,
                    },
                },
            },
            LspLocation {
                path: cwd.join("src/engine.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 18,
                        character: 9,
                    },
                    end: LspPosition {
                        line: 18,
                        character: 13,
                    },
                },
            },
        ],
    );

    assert_eq!(
        rendered,
        [
            "LSP references:",
            "  src/lib.rs:4:5",
            "  src/engine.rs:18:9",
        ]
        .join("\n")
    );
}

#[test]
fn render_lsp_diagnostics_includes_severity_source_and_code() {
    let cwd = temp_dir("lsp_render_diagnostics");
    let rendered = render_lsp_diagnostics(
        &cwd,
        &[LspDiagnostic {
            path: cwd.join("src/lib.rs").display().to_string(),
            range: LspRange {
                start: LspPosition {
                    line: 7,
                    character: 2,
                },
                end: LspPosition {
                    line: 7,
                    character: 6,
                },
            },
            severity: Some(2),
            source: Some("rust-analyzer".to_string()),
            code: Some("E0308".to_string()),
            message: "mismatched types".to_string(),
        }],
    );
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  7:2 warning [rust-analyzer/E0308] mismatched types"));
}

#[test]
fn render_lsp_diagnostics_groups_entries_by_file() {
    let cwd = temp_dir("lsp_render_diagnostics_grouped");
    let rendered = render_lsp_diagnostics(
        &cwd,
        &[
            LspDiagnostic {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 2,
                        character: 4,
                    },
                    end: LspPosition {
                        line: 2,
                        character: 8,
                    },
                },
                severity: Some(1),
                source: Some("rust-analyzer".to_string()),
                code: Some("E0001".to_string()),
                message: "first issue".to_string(),
            },
            LspDiagnostic {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 7,
                        character: 1,
                    },
                    end: LspPosition {
                        line: 7,
                        character: 5,
                    },
                },
                severity: Some(2),
                source: Some("clippy".to_string()),
                code: None,
                message: "second issue".to_string(),
            },
        ],
    );

    assert!(rendered.contains("LSP diagnostics:"));
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  2:4 error [rust-analyzer/E0001] first issue"));
    assert!(rendered.contains("  7:1 warning [clippy] second issue"));
}

#[test]
fn workflow_task_commands_create_list_and_resume_context() {
    let home = temp_dir("workflow_task_home");
    let cwd = temp_dir("workflow_task_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let created = engine
        .process_input_with_approval("/task add Build workflow commands", &mut approver)
        .unwrap();
    assert!(created.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Created task")
                && text.contains("Build workflow commands")
    )));

    let listed = engine
        .process_input_with_approval("/tasks", &mut approver)
        .unwrap();
    assert!(listed.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Project tasks:")
                && text.contains("Build workflow commands")
    )));

    let context = engine
        .process_input_with_approval("/task resume-context", &mut approver)
        .unwrap();
    assert!(context.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Resume context:")
                && text.contains("Suggested next steps:")
    )));
}

#[test]
fn workflow_mutations_respect_plan_mode() {
    let home = temp_dir("workflow_plan_home");
    let cwd = temp_dir("workflow_plan_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    engine
        .process_input_with_approval("/plan on", &mut approver)
        .unwrap();
    let output = engine
        .process_input_with_approval("/task add Should not write", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Permission denied")
    )));
}

#[test]
fn workflow_memory_suggest_confirm_and_project_list() {
    let home = temp_dir("workflow_memory_home");
    let cwd = temp_dir("workflow_memory_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let suggested = engine
        .process_input_with_approval(
            "/memory suggest Keep project memory explicit",
            &mut approver,
        )
        .unwrap();
    let suggestion_text = suggested
        .iter()
        .find_map(|event| match event {
            EngineEvent::Command(text) if text.contains("Suggested memory") => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    let memory_id = suggestion_text
        .split_whitespace()
        .find(|part| part.starts_with("mem_"))
        .unwrap()
        .to_string();

    engine
        .process_input_with_approval(&format!("/memory confirm {memory_id}"), &mut approver)
        .unwrap();
    let project_memory = engine
        .process_input_with_approval("/memory project", &mut approver)
        .unwrap();
    assert!(project_memory.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Project memory:")
                && text.contains("Keep project memory explicit")
    )));
}

#[test]
fn workflow_task_mutation_subcommands_are_routed() {
    let home = temp_dir("workflow_task_mutations_home");
    let cwd = temp_dir("workflow_task_mutations_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let created = engine
        .process_input_with_approval("/task add Full task lifecycle", &mut approver)
        .unwrap();
    let task_id = created
        .iter()
        .find_map(|event| match event {
            EngineEvent::Command(text) => text
                .split_whitespace()
                .find(|part| part.starts_with("task_"))
                .map(ToString::to_string),
            _ => None,
        })
        .unwrap();

    engine
        .process_input_with_approval(
            &format!("/task status {task_id} in_progress"),
            &mut approver,
        )
        .unwrap();
    let viewed = engine
        .process_input_with_approval(&format!("/task view {task_id}"), &mut approver)
        .unwrap();
    assert!(viewed.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("in_progress") && text.contains("Full task lifecycle")
    )));

    engine
        .process_input_with_approval(
            &format!("/task block {task_id} waiting-review"),
            &mut approver,
        )
        .unwrap();
    engine
        .process_input_with_approval(&format!("/task unblock {task_id}"), &mut approver)
        .unwrap();
    engine
        .process_input_with_approval(&format!("/task archive {task_id}"), &mut approver)
        .unwrap();
    let restored = engine
        .process_input_with_approval(&format!("/task restore {task_id}"), &mut approver)
        .unwrap();

    assert!(restored.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Restored task")
    )));
}

#[test]
fn workflow_memory_reject_prune_and_export_are_routed() {
    let home = temp_dir("workflow_memory_mutations_home");
    let cwd = temp_dir("workflow_memory_mutations_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let suggested = engine
        .process_input_with_approval("/memory suggest Reject later", &mut approver)
        .unwrap();
    let memory_id = suggested
        .iter()
        .find_map(|event| match event {
            EngineEvent::Command(text) => text
                .split_whitespace()
                .find(|part| part.starts_with("mem_"))
                .map(ToString::to_string),
            _ => None,
        })
        .unwrap();
    let rejected = engine
        .process_input_with_approval(&format!("/memory reject {memory_id}"), &mut approver)
        .unwrap();
    assert!(rejected.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Rejected memory")
    )));

    let added = engine
        .process_input_with_approval("/memory add Prune later", &mut approver)
        .unwrap();
    let active_id = added
        .iter()
        .find_map(|event| match event {
            EngineEvent::Command(text) => text
                .split_whitespace()
                .find(|part| part.starts_with("mem_"))
                .map(ToString::to_string),
            _ => None,
        })
        .unwrap();
    engine
        .process_input_with_approval(&format!("/memory prune {active_id}"), &mut approver)
        .unwrap();
    let exported = engine
        .process_input_with_approval("/memory export", &mut approver)
        .unwrap();
    assert!(exported.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains("Memory export:")
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

#[test]
fn resume_without_selector_lists_sessions() {
    let home = temp_dir("resume_list_home");
    let cwd = temp_dir("resume_list_cwd");

    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "reply".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };
    engine_a
        .process_input_with_approval("draft a plan", &mut approver)
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let output = engine_b
        .process_input_with_approval("/resume", &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Use `/resume latest`")
                && text.contains("#<index>")
                && text.contains("draft a plan")
    )));
}

#[test]
fn resume_by_prefix_restores_matching_session() {
    let home = temp_dir("resume_prefix_home");
    let cwd = temp_dir("resume_prefix_cwd");
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "reply a".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    engine_a
        .process_input_with_approval("session alpha", &mut approver)
        .unwrap();
    let session_id = engine_a.session_id().to_string();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let prefix = session_id.trim_start_matches("session_");
    let prefix = &prefix[..prefix.len().min(10)];
    let output = engine_b
        .process_input_with_approval(&format!("/resume {prefix}"), &mut approver)
        .unwrap();
    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text) if text.contains(&session_id)
    )));
}

#[test]
fn git_status_command_uses_tool_runtime() {
    let home = temp_dir("git_status_home");
    let cwd = temp_dir("git_status_cwd");
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git status", &mut approver)
        .unwrap();
    assert_eq!(approvals, 0);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::Command(text) if text.contains("demo.txt")))
    );
}

#[test]
fn git_switch_requests_approval() {
    let home = temp_dir("git_switch_home");
    let cwd = temp_dir("git_switch_cwd");
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git switch feature/demo --create", &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("Switched") || text.contains("feature/demo"))
    ));
}

#[test]
fn git_add_requests_approval_and_stages_file() {
    let home = temp_dir("git_add_home");
    let cwd = temp_dir("git_add_cwd");
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git add demo.txt", &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("git add") || text.contains("demo.txt"))
    ));

    let output = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("A  demo.txt"));
}

#[test]
fn git_restore_requests_approval_and_reverts_file() {
    let home = temp_dir("git_restore_home");
    let cwd = temp_dir("git_restore_cwd");
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = std::process::Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = std::process::Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();
    let add = std::process::Command::new("git")
        .args(["add", "demo.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());
    std::fs::write(cwd.join("demo.txt"), "changed\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git restore demo.txt", &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("restore") || text.contains("demo.txt"))
    ));
    let contents = std::fs::read_to_string(cwd.join("demo.txt")).unwrap();
    assert_eq!(contents, "hello\n");
}

#[test]
fn git_stash_push_requests_approval_and_list_is_visible() {
    let home = temp_dir("git_stash_home");
    let cwd = temp_dir("git_stash_cwd");
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = std::process::Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = std::process::Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();
    let add = std::process::Command::new("git")
        .args(["add", "demo.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());
    std::fs::write(cwd.join("demo.txt"), "changed\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let approvals = Cell::new(0usize);
    let mut approver = |_prompt| {
        approvals.set(approvals.get() + 1);
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    engine
        .process_input_with_approval("/git stash push -m save-work", &mut approver)
        .unwrap();
    assert_eq!(approvals.get(), 1);
    let list_output = engine
        .process_input_with_approval("/git stash list", &mut approver)
        .unwrap();
    assert!(
        list_output
            .iter()
            .any(|event| matches!(event, EngineEvent::Command(text) if text.contains("save-work")))
    );
}

#[test]
fn git_worktree_add_requests_approval_and_creates_checkout() {
    let home = temp_dir("git_worktree_home");
    let cwd = temp_dir("git_worktree_cwd");
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = std::process::Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = std::process::Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();
    let add = std::process::Command::new("git")
        .args(["add", "demo.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());

    let worktree = cwd
        .parent()
        .unwrap()
        .join("robocode_core_worktree_checkout");
    if worktree.exists() {
        std::fs::remove_dir_all(&worktree).unwrap();
    }

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let command = format!(
        "/git worktree add {} feature/worktree --create",
        worktree.to_string_lossy()
    );
    let events = engine
        .process_input_with_approval(&command, &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Preparing worktree")
                || text.contains("feature/worktree")
                || text.contains("HEAD is now at")
    )));
    assert!(worktree.exists());
}
