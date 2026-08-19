//! Contract: a recovered session never presents an unanswered tool call.
//!
//! The write path persists the assistant tool-call message before the tool
//! runs and persists the result only after it finishes. A crash or kill in
//! between leaves a durable assistant message whose `tool_call_id` no
//! `Role::Tool` message answers, which several providers reject outright.
//! Recovery must close those calls in memory while leaving the append-only
//! JSONL exactly as the crash left it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use viden_types::{ApprovalResponse, Message, ModelEvent, Role, ToolCall};

use crate::{RuntimeResumeRequest, SessionEngine};

use super::{SequenceProvider, temp_dir};

const RECOVERY_NOTE_MARKER: &str = "closed during session recovery";
const CLOSURE_MARKER: &str = "was interrupted before completion";

fn model_visible_projection(
    messages: &[Message],
) -> Vec<(Role, String, Option<String>, Option<String>)> {
    messages
        .iter()
        .map(|message| {
            (
                message.role.clone(),
                message.content.clone(),
                message.tool_name.clone(),
                message.tool_call_id.clone(),
            )
        })
        .collect()
}

fn transcript_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn rewrite_transcript(path: &Path, lines: &[String]) {
    let mut contents = lines.join("\n");
    contents.push('\n');
    std::fs::write(path, contents).unwrap();
}

fn tool_call_events(call_id: &str) -> Vec<ModelEvent> {
    let mut glob_input = BTreeMap::new();
    glob_input.insert("pattern".to_string(), "*.txt".to_string());
    vec![ModelEvent::ToolCall(ToolCall {
        id: call_id.to_string(),
        name: "glob".to_string(),
        input: glob_input,
    })]
}

fn assistant_events(content: &str) -> Vec<ModelEvent> {
    vec![ModelEvent::AssistantText {
        content: content.to_string(),
    }]
}

/// Runs `inputs` against `turns` and returns the session id plus the transcript
/// path, so a test can corrupt the JSONL the way a crash would.
fn run_live_session(
    home: &Path,
    cwd: &Path,
    turns: Vec<Vec<ModelEvent>>,
    inputs: &[&str],
) -> (String, PathBuf) {
    std::fs::write(cwd.join("note.txt"), "hello recovery\n").unwrap();
    let mut approver = |_prompt| ApprovalResponse::allow_once(None);
    let provider = Box::new(SequenceProvider::new(turns));
    let mut live_engine =
        SessionEngine::new_with_home(cwd, provider, Some(home.to_path_buf())).unwrap();
    let session_id = live_engine.session_id().to_string();
    for input in inputs {
        live_engine
            .process_input_with_approval(input, &mut approver)
            .unwrap();
    }
    let transcript_path = live_engine.store.transcript_path().to_path_buf();
    (session_id, transcript_path)
}

fn resume_engine(home: &Path, cwd: &Path, session_id: &str) -> SessionEngine {
    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(cwd, provider, Some(home.to_path_buf())).unwrap();
    engine
        .resume_session(RuntimeResumeRequest::exact_session_id(session_id))
        .unwrap();
    engine
}

fn assistant_call_index(messages: &[Message], call_id: &str) -> usize {
    messages
        .iter()
        .position(|message| {
            message.role == Role::Assistant && message.tool_call_id.as_deref() == Some(call_id)
        })
        .unwrap_or_else(|| panic!("missing assistant tool-call message for {call_id}"))
}

fn recovery_notes(messages: &[Message]) -> Vec<&Message> {
    messages
        .iter()
        .filter(|message| {
            message.role == Role::System && message.content.contains(RECOVERY_NOTE_MARKER)
        })
        .collect()
}

#[test]
fn interrupted_tail_tool_call_is_closed_during_session_recovery() {
    let home = temp_dir("interrupted_tail_home");
    let cwd = temp_dir("interrupted_tail_cwd");
    let (session_id, transcript_path) = run_live_session(
        &home,
        &cwd,
        vec![
            tool_call_events("call_interrupted_tail"),
            assistant_events("found the note"),
        ],
        &["list text files"],
    );

    // Crash shape: the assistant tool-call message was durable, nothing after.
    let lines = transcript_lines(&transcript_path);
    let call_line = lines
        .iter()
        .position(|line| {
            line.starts_with("{\"type\":\"message\"") && line.contains("call_interrupted_tail")
        })
        .expect("assistant tool-call message must be persisted before the tool runs");
    rewrite_transcript(&transcript_path, &lines[..=call_line]);
    let truncated = std::fs::read_to_string(&transcript_path).unwrap();

    let recovered = resume_engine(&home, &cwd, &session_id);
    let messages = &recovered.messages;

    let note = messages
        .last()
        .expect("recovered history must not be empty");
    assert_eq!(note.role, Role::System, "messages: {messages:?}");
    assert!(
        note.content.contains("1 interrupted tool call(s)")
            && note.content.contains(RECOVERY_NOTE_MARKER),
        "expected a single recovery note, got: {}",
        note.content
    );

    let closure = &messages[messages.len() - 2];
    assert_eq!(closure.role, Role::Tool, "messages: {messages:?}");
    assert_eq!(
        closure.tool_call_id.as_deref(),
        Some("call_interrupted_tail")
    );
    assert_eq!(closure.tool_name.as_deref(), Some("glob"));
    assert!(
        closure.content.contains(CLOSURE_MARKER),
        "the closure must mark the call as interrupted: {}",
        closure.content
    );

    // Loads stay read-only: the synthesis exists only in memory.
    assert_eq!(
        std::fs::read_to_string(&transcript_path).unwrap(),
        truncated,
        "recovery must not append to the append-only transcript"
    );
}

#[test]
fn interrupted_tool_call_recovery_is_deterministic_across_loads() {
    let home = temp_dir("interrupted_determinism_home");
    let cwd = temp_dir("interrupted_determinism_cwd");
    let (session_id, transcript_path) = run_live_session(
        &home,
        &cwd,
        vec![
            tool_call_events("call_interrupted_twice"),
            assistant_events("found the note"),
        ],
        &["list text files"],
    );
    let lines = transcript_lines(&transcript_path);
    let call_line = lines
        .iter()
        .position(|line| {
            line.starts_with("{\"type\":\"message\"") && line.contains("call_interrupted_twice")
        })
        .unwrap();
    rewrite_transcript(&transcript_path, &lines[..=call_line]);

    let first = model_visible_projection(&resume_engine(&home, &cwd, &session_id).messages);
    let second = model_visible_projection(&resume_engine(&home, &cwd, &session_id).messages);
    assert_eq!(
        first, second,
        "recovery synthesis must be a pure function of the transcript"
    );
}

#[test]
fn completed_tool_call_is_not_closed_during_session_recovery() {
    let home = temp_dir("completed_call_home");
    let cwd = temp_dir("completed_call_cwd");
    let (session_id, _) = run_live_session(
        &home,
        &cwd,
        vec![
            tool_call_events("call_completed"),
            assistant_events("found the note"),
        ],
        &["list text files"],
    );

    let recovered = resume_engine(&home, &cwd, &session_id);
    assert!(
        recovered
            .messages
            .iter()
            .all(|message| !message.content.contains(CLOSURE_MARKER)),
        "an answered tool call must not be synthesized: {:?}",
        recovered.messages
    );
    assert!(
        recovery_notes(&recovered.messages).is_empty(),
        "a healthy transcript must not carry a recovery note"
    );
}

#[test]
fn two_interrupted_tool_calls_are_closed_under_one_recovery_note() {
    let home = temp_dir("two_interrupted_home");
    let cwd = temp_dir("two_interrupted_cwd");
    let (session_id, transcript_path) = run_live_session(
        &home,
        &cwd,
        vec![
            tool_call_events("call_dangling_one"),
            assistant_events("first turn done"),
            tool_call_events("call_dangling_two"),
            assistant_events("second turn done"),
        ],
        &["list text files", "list them again"],
    );

    // Both results were lost; both assistant tool-call messages survive.
    let lines: Vec<String> = transcript_lines(&transcript_path)
        .into_iter()
        .filter(|line| !line.starts_with("{\"type\":\"tool_result\""))
        .collect();
    rewrite_transcript(&transcript_path, &lines);

    let recovered = resume_engine(&home, &cwd, &session_id);
    let messages = &recovered.messages;

    for call_id in ["call_dangling_one", "call_dangling_two"] {
        let call_index = assistant_call_index(messages, call_id);
        let closure = &messages[call_index + 1];
        assert_eq!(closure.role, Role::Tool, "messages: {messages:?}");
        assert_eq!(closure.tool_call_id.as_deref(), Some(call_id));
        assert!(closure.content.contains(CLOSURE_MARKER));
    }

    let notes = recovery_notes(messages);
    assert_eq!(
        notes.len(),
        1,
        "expected exactly one recovery note: {notes:?}"
    );
    assert!(
        notes[0].content.contains("2 interrupted tool call(s)"),
        "the note must count every closed call: {}",
        notes[0].content
    );
}

#[test]
fn interrupted_tool_call_in_the_middle_of_history_is_closed_in_place() {
    // Reachable today: a resumed session appends to the same transcript, so an
    // interrupted call from an earlier run stays embedded before the turns the
    // user ran afterwards, and every later load must close it again in place.
    let home = temp_dir("interrupted_middle_home");
    let cwd = temp_dir("interrupted_middle_cwd");
    let (session_id, transcript_path) = run_live_session(
        &home,
        &cwd,
        vec![
            tool_call_events("call_middle_dangling"),
            assistant_events("first turn done"),
            tool_call_events("call_middle_completed"),
            assistant_events("second turn done"),
        ],
        &["list text files", "list them again"],
    );

    // Only the first result was lost.
    let mut dropped = false;
    let lines: Vec<String> = transcript_lines(&transcript_path)
        .into_iter()
        .filter(|line| {
            let drop = !dropped && line.starts_with("{\"type\":\"tool_result\"");
            dropped |= drop;
            !drop
        })
        .collect();
    assert!(dropped, "the fixture must drop one tool result");
    rewrite_transcript(&transcript_path, &lines);

    let recovered = resume_engine(&home, &cwd, &session_id);
    let messages = &recovered.messages;

    let call_index = assistant_call_index(messages, "call_middle_dangling");
    let closure = &messages[call_index + 1];
    assert_eq!(closure.role, Role::Tool, "messages: {messages:?}");
    assert_eq!(
        closure.tool_call_id.as_deref(),
        Some("call_middle_dangling")
    );
    assert!(closure.content.contains(CLOSURE_MARKER));

    let completed_index = assistant_call_index(messages, "call_middle_completed");
    assert!(
        call_index + 1 < completed_index,
        "the closure must sit inside the interrupted turn, not at the tail: {messages:?}"
    );
    assert!(
        messages[completed_index + 1..]
            .iter()
            .any(|message| message.role == Role::Tool
                && message.tool_call_id.as_deref() == Some("call_middle_completed")
                && !message.content.contains(CLOSURE_MARKER)),
        "the answered call must keep its real result: {messages:?}"
    );

    let notes = recovery_notes(messages);
    assert_eq!(
        notes.len(),
        1,
        "expected exactly one recovery note: {notes:?}"
    );
    assert!(notes[0].content.contains("1 interrupted tool call(s)"));
}
