use crate::{EngineEvent, SessionEngine};
use viden_provider::ProviderHost;
use viden_session::SessionStore;
use viden_types::{
    ApprovalResponse, Message, ModelEvent, ModelUsage, Role, RuntimeEventKind, RuntimeViewState,
    TranscriptEntry,
};

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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
fn resume_replays_provider_cost_usage_from_session_log() {
    let home = temp_dir("resume_provider_cost_home");
    let cwd = temp_dir("resume_provider_cost_cwd");
    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "priced reply".to_string(),
        },
        ModelEvent::Usage(ModelUsage {
            input_tokens: Some(19),
            output_tokens: Some(7),
            cached_input_tokens: Some(5),
            retrieval_tokens: None,
            total_tokens: Some(26),
            cost_micro_usd: None,
            actual_cost_micro_usd: None,
        }),
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let session_id = engine_a.session_id().to_string();
    let live_engine_events = engine_a
        .process_input_with_approval("track provider cost", &mut approver)
        .unwrap();
    let live_runtime_events = engine_a.runtime_events_for_engine_events(&live_engine_events);
    let live_usage_id = live_runtime_events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::CostUsageRecorded { cost } if cost.provider_id == "sequence" => {
                Some(cost.usage_id.clone())
            }
            _ => None,
        })
        .expect("live provider cost");

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let resume_events = engine_b
        .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
        .unwrap();
    let resumed_runtime_events = engine_b.runtime_events_for_engine_events(&resume_events);
    let mut view = RuntimeViewState::new(engine_b.runtime_snapshot());
    for event in &resumed_runtime_events {
        view.apply_event(event);
    }

    assert!(resumed_runtime_events.iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::CostUsageRecorded { cost }
                if cost.usage_id == live_usage_id && cost.tokens.input_tokens == Some(19)
        )
    }));
    assert_eq!(view.cost_usage.len(), 1);
    assert_eq!(view.cost_ledger.input_tokens, 19);
    assert_eq!(view.cost_ledger.output_tokens, 7);
    assert_eq!(view.cost_ledger.cached_input_tokens, 5);
}

#[test]
fn resume_replays_duplicate_cost_usage_id_once() {
    let home = temp_dir("resume_duplicate_cost_home");
    let cwd = temp_dir("resume_duplicate_cost_cwd");
    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "duplicate priced reply".to_string(),
        },
        ModelEvent::Usage(ModelUsage {
            input_tokens: Some(31),
            output_tokens: Some(11),
            cached_input_tokens: None,
            retrieval_tokens: None,
            total_tokens: Some(42),
            cost_micro_usd: None,
            actual_cost_micro_usd: None,
        }),
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let session_id = engine_a.session_id().to_string();
    engine_a
        .process_input_with_approval("track duplicate cost", &mut approver)
        .unwrap();
    let store = SessionStore::new_with_home(&home, &cwd, Some(session_id.clone())).unwrap();
    let duplicate_cost = store
        .load_entries()
        .unwrap()
        .into_iter()
        .find_map(|entry| match entry {
            viden_types::TranscriptEntry::CostUsage { cost } => Some(*cost),
            _ => None,
        })
        .expect("persisted cost usage");
    store
        .append_entry(&viden_types::TranscriptEntry::CostUsage {
            cost: Box::new(duplicate_cost.clone()),
        })
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let resume_events = engine_b
        .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
        .unwrap();
    let runtime_events = engine_b.runtime_events_for_engine_events(&resume_events);
    let mut view = RuntimeViewState::new(engine_b.runtime_snapshot());
    for event in &runtime_events {
        view.apply_event(event);
    }

    assert_eq!(view.cost_usage.len(), 1);
    assert_eq!(view.cost_usage[0].usage_id, duplicate_cost.usage_id);
    assert_eq!(view.cost_ledger.input_tokens, 31);
    assert_eq!(view.cost_ledger.output_tokens, 11);
}

#[test]
fn resume_session_without_cost_usage_keeps_empty_cost_ledger() {
    let home = temp_dir("resume_without_cost_home");
    let cwd = temp_dir("resume_without_cost_cwd");
    let session_id = "legacy_no_cost_session".to_string();
    let store = SessionStore::new_with_home(&home, &cwd, Some(session_id.clone())).unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: Message::new(Role::User, "old session before costs"),
        })
        .unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let resume_events = engine_b
        .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
        .unwrap();
    let runtime_events = engine_b.runtime_events_for_engine_events(&resume_events);
    let mut view = RuntimeViewState::new(engine_b.runtime_snapshot());
    for event in &runtime_events {
        view.apply_event(event);
    }

    assert!(view.cost_usage.is_empty());
    assert_eq!(view.cost_ledger.input_tokens, 0);
    assert_eq!(view.cost_ledger.output_tokens, 0);
}

#[test]
fn resume_restores_provider_runtime_selection() {
    let home = temp_dir("resume_provider_runtime_home");
    let cwd = temp_dir("resume_provider_runtime_cwd");
    let provider_a = Box::new(SequenceProvider::new(vec![]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    engine_a.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

    engine_a
        .process_input_with_approval("/provider use fallback resumed-model", &mut approver)
        .unwrap();
    let session_id = engine_a.session_id().to_string();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    engine_b.set_provider_runtime(ProviderHost::with_builtins(), Vec::new(), None, None, 90, 1);
    let output = engine_b
        .process_input_with_approval(&format!("/resume {session_id}"), &mut approver)
        .unwrap();

    assert_eq!(engine_b.provider_name(), "fallback");
    assert_eq!(engine_b.model_name(), "resumed-model");
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
fn sessions_command_uses_structured_view_sections() {
    let home = temp_dir("sessions_structured_home");
    let cwd = temp_dir("sessions_structured_cwd");

    let provider_a = Box::new(SequenceProvider::new(vec![vec![
        ModelEvent::AssistantText {
            content: "structured reply".to_string(),
        },
    ]]));
    let mut engine_a = SessionEngine::new_with_home(&cwd, provider_a, Some(home.clone())).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    engine_a
        .process_input_with_approval("inspect structured sessions", &mut approver)
        .unwrap();

    let provider_b = Box::new(SequenceProvider::new(vec![]));
    let mut engine_b = SessionEngine::new_with_home(&cwd, provider_b, Some(home)).unwrap();
    let output = engine_b
        .process_input_with_approval("/sessions", &mut approver)
        .unwrap();

    assert!(output.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Summary: total=")
                && text.contains("Session entries:")
                && text.contains("activity: messages=")
                && text.contains("preview:")
                && text.contains("last activity:")
    )));
}

#[test]
fn sessions_command_marks_current_session() {
    let home = temp_dir("sessions_current_home");
    let cwd = temp_dir("sessions_current_cwd");
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
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
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);

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
