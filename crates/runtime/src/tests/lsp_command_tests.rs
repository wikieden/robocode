use crate::{EngineEvent, SessionEngine};
use viden_types::{ApprovalResponse, TranscriptEntry};

use super::{SequenceProvider, temp_dir};

#[test]
fn help_output_lists_lsp_commands() {
    let home = temp_dir("lsp_help_home");
    let cwd = temp_dir("lsp_help_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    engine
        .process_input_with_approval("/lsp status", &mut approver)
        .unwrap();
    let entries = engine.store.load_entries().unwrap();
    assert!(entries.iter().any(|entry| matches!(
        entry,
        TranscriptEntry::Command { entry } if entry.name == "lsp"
    )));
}
