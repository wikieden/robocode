use crate::{EngineEvent, SessionEngine};
use robocode_types::ApprovalResponse;

use super::{SequenceProvider, temp_dir};

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
fn workflow_tasks_command_uses_structured_view_sections() {
    let home = temp_dir("workflow_task_structured_home");
    let cwd = temp_dir("workflow_task_structured_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let created = engine
        .process_input_with_approval("/task add Structured task rendering", &mut approver)
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

    let listed = engine
        .process_input_with_approval("/tasks", &mut approver)
        .unwrap();

    assert!(listed.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Summary: active=")
                && text.contains("Task entries:")
                && text.contains("title: Structured task rendering")
                && text.contains("status: in_progress")
                && text.contains("priority: medium")
                && text.contains("last session:")
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
fn workflow_memory_commands_use_structured_view_sections() {
    let home = temp_dir("workflow_memory_structured_home");
    let cwd = temp_dir("workflow_memory_structured_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse {
        approved: true,
        feedback: None,
    };

    let suggested = engine
        .process_input_with_approval(
            "/memory suggest Keep provider plugins explicit",
            &mut approver,
        )
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

    let suggestions = engine
        .process_input_with_approval("/memory suggest", &mut approver)
        .unwrap();
    assert!(suggestions.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Pending memory suggestions:")
                && text.contains("Summary: pending=1")
                && text.contains("Memory entries:")
                && text.contains("content: Keep provider plugins explicit")
                && text.contains("status: suggested")
                && text.contains("source: assistant_suggestion")
    )));

    engine
        .process_input_with_approval(&format!("/memory confirm {memory_id}"), &mut approver)
        .unwrap();
    engine
        .process_input_with_approval(
            "/memory add Remember session command context",
            &mut approver,
        )
        .unwrap();

    let project_memory = engine
        .process_input_with_approval("/memory project", &mut approver)
        .unwrap();
    assert!(project_memory.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Project memory:")
                && text.contains("Summary: active=1")
                && text.contains("Memory entries:")
                && text.contains("content: Keep provider plugins explicit")
                && text.contains("kind: fact")
                && text.contains("scope: project")
                && text.contains("status: active")
    )));

    let session_memory = engine
        .process_input_with_approval("/memory session", &mut approver)
        .unwrap();
    assert!(session_memory.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Session memory:")
                && text.contains("Summary: active=1")
                && text.contains("Memory entries:")
                && text.contains("content: Remember session command context")
                && text.contains("scope: session")
                && text.contains("session:")
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
        EngineEvent::Command(text)
            if text.contains("Memory export:")
                && text.contains("Summary: project=0 session=0")
                && text.contains("Project memory:")
                && text.contains("Session memory:")
    )));
}
