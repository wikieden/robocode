use std::{env, fs};

use serde_json::Value;
use viden_types::{AgentContentPart, RuntimeEventKind, RuntimeOwner, fresh_id};

use super::acp::*;
use super::glue::*;

fn chunk(text: &str) -> Value {
    serde_json::json!({
        "sessionUpdate": "agent_message_chunk",
        "content": { "type": "text", "text": text }
    })
}

/// GUI-CORE-016: every chunk of one prompt turn carries the same message id
/// and its session, so the reducer grows a single owner-scoped reply that a
/// client can render while it is still being produced.
#[test]
fn chunks_of_one_turn_share_a_stable_scoped_message_id() {
    let mut events = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    let turn_message_id = "acp-message-session_1-turn-7";

    for text in ["I will ", "draw ", "a cat."] {
        append_acp_update_runtime_events(
            &mut events,
            &mut sequence,
            "session_1",
            turn_message_id,
            &mut evidence_ids,
            None,
            None,
            &chunk(text),
        );
    }

    let deltas: Vec<(&str, Option<&str>, &str)> = events
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::AssistantDelta {
                message_id,
                session_id,
                content,
                ..
            } => Some((message_id.as_str(), session_id.as_deref(), content.as_str())),
            _ => None,
        })
        .collect();

    assert_eq!(deltas.len(), 3, "each chunk stays its own ordered event");
    assert!(
        deltas
            .iter()
            .all(|(id, session, _)| *id == turn_message_id && *session == Some("session_1")),
        "chunks of one turn must not fan out into separate messages: {deltas:?}"
    );
    assert_eq!(
        deltas
            .iter()
            .map(|(_, _, content)| *content)
            .collect::<String>(),
        "I will draw a cat.",
        "replaying the chunks reconstructs the reply exactly"
    );
}

/// GUI-CORE-017: an image block reaches the reply as a typed part instead
/// of being dropped while the text claims an image exists.
#[test]
fn an_image_block_becomes_a_typed_message_part() {
    let mut events = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    append_acp_update_runtime_events(
        &mut events,
        &mut sequence,
        "session_1",
        "turn-1",
        &mut evidence_ids,
        None,
        None,
        &serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "image",
                "mimeType": "image/png",
                "uri": "file:///tmp/cat.png",
                "alt": "an orange cat"
            }
        }),
    );

    let part = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::AgentMessagePart {
                message_id, part, ..
            } if message_id == "turn-1" => Some(part.clone()),
            _ => None,
        })
        .expect("the image block must publish a typed part");
    assert!(matches!(
        part,
        AgentContentPart::Image { ref media_type, ref reference, .. }
            if media_type == "image/png" && reference == "file:///tmp/cat.png"
    ));
}

/// Without a workspace to write into there is no reference Core has
/// persisted, so the block is kept verbatim rather than dropped or given
/// an invented reference.
#[test]
fn an_inline_image_block_without_a_workspace_is_preserved_as_unknown() {
    let mut events = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    append_acp_update_runtime_events(
        &mut events,
        &mut sequence,
        "session_1",
        "turn-1",
        &mut evidence_ids,
        None,
        None,
        &serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "image", "mimeType": "image/png", "data": "iVBORw0KGgo=" }
        }),
    );

    let part = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::AgentMessagePart { part, .. } => Some(part.clone()),
            _ => None,
        })
        .expect("the block must not disappear");
    assert!(matches!(part, AgentContentPart::Unknown { ref kind, .. } if kind == "image"));
}

/// GUI-CORE-017: inline Agent bytes become durable evidence with an
/// immutable reference, so a drawn image survives the turn and can be
/// rendered from the fact Core published rather than from the wire.
#[test]
fn inline_image_bytes_become_referenced_evidence() {
    let cwd = env::temp_dir().join(format!("viden_acp_inline_{}", fresh_id("tmp")));
    fs::create_dir_all(&cwd).expect("create workspace");
    let mut events = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    // "cat" in base64; the bytes only have to round-trip, not be a real PNG.
    append_acp_update_runtime_events(
        &mut events,
        &mut sequence,
        "session_1",
        "turn-1",
        &mut evidence_ids,
        Some(cwd.as_path()),
        None,
        &serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "image",
                "mimeType": "image/png",
                "data": "Y2F0",
                "alt": "an orange cat"
            }
        }),
    );

    let part = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::AgentMessagePart { part, .. } => Some(part.clone()),
            _ => None,
        })
        .expect("the block must publish a typed part");
    let AgentContentPart::Image {
        media_type,
        reference,
        alt,
    } = part
    else {
        panic!("persisted inline bytes must become a typed image part, got {part:?}");
    };
    assert_eq!(media_type, "image/png");
    assert_eq!(alt.as_deref(), Some("an orange cat"));
    assert!(
        reference.starts_with(".viden/agents/parts/") && reference.ends_with(".png"),
        "the reference must be a workspace path Core owns: {reference}"
    );
    assert_eq!(
        fs::read(cwd.join(&reference)).expect("persisted bytes"),
        b"cat",
        "the persisted file must hold exactly the bytes the Agent returned"
    );
    assert!(
        events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::EvidenceRecorded { evidence }
                if evidence.path.as_deref() == Some(reference.as_str())
        )),
        "the persisted bytes must also be published as evidence"
    );
    assert!(
        evidence_ids.iter().any(|id| id.contains("acp-content-")),
        "the content evidence must be available to a gate: {evidence_ids:?}"
    );
    let _ = fs::remove_dir_all(&cwd);
}

/// The same bytes must resolve to the same immutable file, so a replayed
/// event never points at a path that was rewritten underneath it.
#[test]
fn identical_inline_bytes_share_one_immutable_reference() {
    let cwd = env::temp_dir().join(format!("viden_acp_inline_{}", fresh_id("tmp")));
    fs::create_dir_all(&cwd).expect("create workspace");
    let update = serde_json::json!({
        "sessionUpdate": "agent_message_chunk",
        "content": { "type": "image", "mimeType": "image/png", "data": "Y2F0" }
    });
    let mut references = Vec::new();
    for turn in ["turn-1", "turn-2"] {
        let mut events = Vec::new();
        let mut sequence = 1;
        let mut evidence_ids = Vec::new();
        append_acp_update_runtime_events(
            &mut events,
            &mut sequence,
            "session_1",
            turn,
            &mut evidence_ids,
            Some(cwd.as_path()),
            None,
            &update,
        );
        references.extend(events.iter().filter_map(|event| match &event.kind {
            RuntimeEventKind::AgentMessagePart {
                part: AgentContentPart::Image { reference, .. },
                ..
            } => Some(reference.clone()),
            _ => None,
        }));
    }

    assert_eq!(references.len(), 2);
    assert_eq!(
        references[0], references[1],
        "content-addressed bytes must not fork into two references"
    );
    let _ = fs::remove_dir_all(&cwd);
}

/// GUI-CORE-016: facts are scoped by the session Core published, not by
/// the agent's own protocol handle.
///
/// Every other fact about an Agent session — its view, its accepted
/// inputs, its conversation — is keyed by the Viden session id. Scoping a
/// streamed reply by the ACP handle produced a parallel conversation no
/// client could see, which is exactly the "replies only appear when the
/// turn ends" symptom.
#[test]
fn a_streamed_reply_is_scoped_by_the_session_core_published() {
    assert_eq!(
        acp_scoped_session_id(Some("agent-session_17855"), "019fbc86-35da-7d33"),
        "agent-session_17855",
        "the Viden session id must win over the ACP protocol handle"
    );
    assert_eq!(
        acp_scoped_session_id(None, "019fbc86-35da-7d33"),
        "019fbc86-35da-7d33",
        "an ad-hoc probe with no Core session keeps the protocol handle"
    );
}

/// A resumed turn reuses the remote session and restarts ACP request ids,
/// so two turns would otherwise grow into one message.
#[test]
fn two_turns_of_one_session_never_share_a_message_id() {
    let first = acp_turn_message_id("session-1", Some("agent-input_1"), 6);
    let second = acp_turn_message_id("session-1", Some("agent-input_2"), 6);
    assert_ne!(first, second);
    assert_eq!(
        acp_turn_message_id("session-1", Some("agent-input_1"), 9),
        first,
        "chunks of one turn share a message id regardless of request id"
    );
    assert_eq!(
        acp_turn_message_id("session-1", None, 6),
        "acp-message-session-1-turn-6",
        "without a turn artifact the request id still separates turns"
    );
}

/// A later turn in the same session must not append to the earlier reply.
#[test]
fn a_later_turn_uses_its_own_message_id() {
    let mut events = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    append_acp_update_runtime_events(
        &mut events,
        &mut sequence,
        "session_1",
        "acp-message-session_1-turn-7",
        &mut evidence_ids,
        None,
        None,
        &chunk("first"),
    );
    append_acp_update_runtime_events(
        &mut events,
        &mut sequence,
        "session_1",
        "acp-message-session_1-turn-8",
        &mut evidence_ids,
        None,
        None,
        &chunk("second"),
    );

    let ids: Vec<&str> = events
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::AssistantDelta { message_id, .. } => Some(message_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1], "turns are separate replies");
}

/// GUI-CORE-010: an Agent session's tool calls and evidence carry the exact
/// owner Core published the session under, and stay unowned when Core
/// published no session for the turn.
#[test]
fn acp_live_work_facts_carry_the_published_session_owner() {
    let owner = RuntimeOwner {
        workspace_id: "workspace_acp".to_string(),
        project_id: "project_acp".to_string(),
        lane_id: Some("lane_acp".to_string()),
        session_id: Some("session_acp".to_string()),
        task_id: Some("task_acp".to_string()),
        turn_id: Some("turn_acp".to_string()),
    };
    let update = serde_json::json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "call_1",
        "title": "shell"
    });
    let finished = serde_json::json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "call_1",
        "title": "shell",
        "status": "completed",
        "content": "ok"
    });

    let mut owned = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    for value in [&update, &finished] {
        append_acp_update_runtime_events(
            &mut owned,
            &mut sequence,
            "session_acp",
            "acp-message-session_acp-turn-1",
            &mut evidence_ids,
            None,
            Some(&owner),
            value,
        );
    }
    let owners: Vec<Option<RuntimeOwner>> = owned
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::ToolCallStarted { owner, .. } => Some(owner.clone()),
            RuntimeEventKind::EvidenceRecorded { evidence } => Some(evidence.owner.clone()),
            RuntimeEventKind::ToolCallFinished {
                evidence: Some(evidence),
                ..
            } => Some(evidence.owner.clone()),
            _ => None,
        })
        .collect();
    assert!(owners.len() >= 3, "the turn must publish live-work facts");
    assert!(
        owners.iter().all(|value| value.as_ref() == Some(&owner)),
        "every ACP live-work fact carries the session owner: {owners:?}"
    );

    let mut unowned = Vec::new();
    let mut sequence = 1;
    let mut evidence_ids = Vec::new();
    for value in [&update, &finished] {
        append_acp_update_runtime_events(
            &mut unowned,
            &mut sequence,
            "session_acp",
            "acp-message-session_acp-turn-1",
            &mut evidence_ids,
            None,
            None,
            value,
        );
    }
    assert!(
        unowned.iter().all(|event| match &event.kind {
            RuntimeEventKind::ToolCallStarted { owner, .. } => owner.is_none(),
            RuntimeEventKind::EvidenceRecorded { evidence } => evidence.owner.is_none(),
            RuntimeEventKind::ToolCallFinished { evidence, .. } => evidence
                .as_ref()
                .is_none_or(|evidence| evidence.owner.is_none()),
            _ => true,
        }),
        "a turn Core published no session for stays unowned"
    );
}
