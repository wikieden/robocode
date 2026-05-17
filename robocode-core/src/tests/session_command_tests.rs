use crate::{EngineEvent, SessionEngine};
use robocode_types::{ApprovalResponse, ModelEvent};

use super::{SequenceProvider, temp_dir};

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
