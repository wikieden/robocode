use std::collections::VecDeque;
use std::time::Duration;

use serde::Deserialize;
use viden_core::{
    CoreClient, CoreClientError, CoreHandshake, EventCursor, FRONTEND_SCHEMA_V1, ReplayBatch,
    ReplayRequest, RuntimeCommandEnvelope, RuntimeEventEnvelope, RuntimeEventKind, RuntimeSnapshot,
    RuntimeSnapshotEnvelope, RuntimeViewState, RuntimeWireEvent, TranscriptPage,
    TranscriptPageRequest, frontend_capabilities, local_core_handshake,
};
use viden_gui::GuiCoreAdapter;

const INTERACTION_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/interaction-closed-loop.json"
);

#[derive(Clone, Deserialize)]
struct Fixture {
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
    expected_final_cursor: EventCursor,
}

struct FixtureCoreClient {
    view: RuntimeViewState,
    events: VecDeque<RuntimeEventEnvelope>,
    cursor: EventCursor,
}

impl FixtureCoreClient {
    fn new(fixture: &Fixture) -> Self {
        Self {
            view: RuntimeViewState::new(fixture.initial_snapshot.clone()),
            events: fixture.events.clone().into(),
            cursor: EventCursor {
                stream_id: fixture.expected_final_cursor.stream_id.clone(),
                sequence: 0,
            },
        }
    }

    fn envelope(&self) -> RuntimeSnapshotEnvelope {
        RuntimeSnapshotEnvelope {
            schema_version: FRONTEND_SCHEMA_V1,
            capabilities: frontend_capabilities(),
            cursor: self.cursor.clone(),
            snapshot: self.view.snapshot.clone(),
            view: self.view.clone(),
        }
    }
}

impl CoreClient for FixtureCoreClient {
    fn discover(&mut self) -> Result<CoreHandshake, CoreClientError> {
        Ok(local_core_handshake())
    }

    fn send(&mut self, _command: RuntimeCommandEnvelope) -> Result<(), CoreClientError> {
        Err(CoreClientError::Transport(
            "fixture projection test is read-only".to_string(),
        ))
    }

    fn recv(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<RuntimeEventEnvelope>, CoreClientError> {
        let Some(envelope) = self.events.pop_front() else {
            return Ok(None);
        };
        if let RuntimeWireEvent::Known(event) = &envelope.event {
            self.view.apply_event(event);
        }
        self.cursor = envelope.cursor.clone();
        Ok(Some(envelope))
    }

    fn snapshot(&mut self) -> Result<RuntimeSnapshotEnvelope, CoreClientError> {
        Ok(self.envelope())
    }

    fn replay(&mut self, _request: ReplayRequest) -> Result<ReplayBatch, CoreClientError> {
        Err(CoreClientError::Transport(
            "fixture events are already contiguous".to_string(),
        ))
    }

    fn transcript_page(
        &mut self,
        _request: TranscriptPageRequest,
    ) -> Result<TranscriptPage, CoreClientError> {
        Err(CoreClientError::Transport(
            "fixture has no transcript page".to_string(),
        ))
    }
}

#[test]
fn native_acp_fixture_projection() {
    let fixture: Fixture =
        serde_json::from_str(INTERACTION_FIXTURE).expect("parse interaction fixture");
    let mut adapter = GuiCoreAdapter::new(Box::new(FixtureCoreClient::new(&fixture)));
    adapter.connect().expect("connect fixture Core client");

    for _ in 0..fixture.events.len() {
        assert!(adapter.pump(Duration::ZERO).expect("pump fixture event"));
    }

    assert_eq!(
        adapter.projection().cursor(),
        Some(&fixture.expected_final_cursor)
    );
    assert_eq!(fixture.expected_final_cursor.sequence, 22);
    let view = adapter
        .projection()
        .view()
        .expect("Core-backed GUI runtime view");

    assert_eq!(view.lanes.len(), 1);
    assert_eq!(view.lanes[0].id, "lane-loop-coder");
    assert_eq!(view.starter_lane_receipts.len(), 1);
    assert_eq!(
        view.starter_lane_receipts[0].preview_id,
        "preview-loop-coder"
    );
    assert_eq!(view.starter_lane_receipts[0].lane.id, "lane-loop-coder");

    // Core's one-Lane/one-Agent rule retains the first execution identity.
    // The D1 projection must not manufacture an owner binding from either
    // session event when Core did not publish LaneRuntimeOwnerBound.
    assert_eq!(view.agent_sessions.len(), 1);
    assert_eq!(view.agent_sessions[0].session_id, "session-loop-built-in");
    let cockpit = adapter
        .d1_cockpit(Some("lane-loop-coder"))
        .expect("D1 cockpit projection");
    assert_eq!(cockpit.lanes.len(), 1);
    assert!(cockpit.context_dock.lane_agent.is_none());
    assert!(cockpit.agent_sessions.is_empty());

    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::AgentSessionStarted { session }
                    if session.session_id == "session-loop-built-in"
            )
    )));
    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::AgentSessionStarted { session }
                    if session.session_id == "session-loop-acp"
            )
    )));
    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::ToolCallStarted { tool_call_id, .. }
                    if tool_call_id == "tool-loop-test"
            )
    )));
    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::ToolCallFinished { tool_call_id, success, .. }
                    if tool_call_id == "tool-loop-test" && *success
            )
    )));
    assert!(view.active_tool_calls.is_empty());
    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::ApprovalRequested { approval }
                    if approval.id == "approval-loop-tool"
            )
    )));
    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::ApprovalResolved { request_id, .. }
                    if request_id == "approval-loop-tool"
            )
    )));
    assert!(view.pending_approvals.is_empty());

    assert_eq!(view.latest_evidence[0].id, "evidence-loop-test");
    assert_eq!(view.merge_gates[0].gate_id, "gate-loop-apply");
    assert_eq!(view.lane_conflicts[0].paths, ["src/lib.rs"]);
    assert_eq!(
        view.lane_recoveries[0].next_action,
        "action.revalidate_merge_conflict"
    );
    assert_eq!(
        view.agent_session_inputs[0].input_id,
        "agent-input-loop-follow-up"
    );
    assert!(fixture.events.iter().any(|envelope| matches!(
        &envelope.event,
        RuntimeWireEvent::Known(event)
            if matches!(
                &event.kind,
                RuntimeEventKind::AgentSessionStarted { session }
                    if session.session_id == "session-loop-acp"
                        && session.task == "task.loop.follow_up"
            )
    )));
    assert!(view.cost_usage.is_empty());
    assert!(view.token_cost.is_none());
}
