# GUI ACP Lane Closed-Loop Specification

## Problem

The D1 New Lane popover currently defaults to the native Viden Agent while ACP
adapter discovery is still running. Clicking a disabled Codex option has no
effect, so submitting the task can silently create a native Lane. Separately,
a successfully completed ACP session can remain hidden behind a stale global
`agent_stopped` recovery surface, and the typed GUI projection does not expose
the ACP assistant response.

## Accepted behavior

1. A new Lane has no Agent selected by default.
2. The operator must explicitly choose Viden Agent or a ready ACP Agent.
3. Lane creation is disabled while discovery is probing, when no Agent is
   selected, or when the task is empty.
4. The create action visibly names the selected Agent.
5. Exactly one Agent remains bound to one Lane.
6. A selected ACP session in `starting`, `running`, `waiting_approval`, or
   `completed` state is not replaced by an unrelated stale `agent_stopped`
   recovery surface.
7. Core publishes the latest completed ACP assistant response as an optional,
   owner-scoped session fact. GUI renders that response in the selected Lane.
8. Failed and cancelled ACP sessions continue to use explicit recovery/status
   UI and never appear successful.

## Contract

`AgentSessionView` gains an optional `output` field with serde defaults. This is
a backward-compatible extension of `frontend-contract-v1`; legacy snapshots
decode it as absent. The ACP runtime assigns the field only from protocol
`AgentMessageChunk` content collected for that exact Core-owned session.

The D1 adapter maps the optional field into `D1AgentSessionProjection`. The web
client does not read `.viden` JSONL/result files directly and does not infer
output from diagnostics or display text.

## Acceptance evidence

- Component test: no selection by default; create remains disabled until an
  explicit ready Agent is chosen and discovery completes.
- D1 test: choosing Codex sends `start_agent_session` with `codex-acp`.
- Core tests: ACP completion publishes the collected response and legacy
  session JSON without `output` still decodes.
- Projection test: selected ACP output reaches the GUI projection.
- D1 test: completed ACP output is visible while stale `agent_stopped` does not
  cover it.
- Live macOS app test: create a Codex Lane and see its exact response in D1.
