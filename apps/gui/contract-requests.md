# GUI Core Contract Requests

Chinese version: [contract-requests.zh-CN.md](contract-requests.zh-CN.md)

These requests are the GUI `0.1.0-alpha.1` inventory against Core `0.3.0`.
They are not GUI implementation workarounds. Until Core lands the missing
facts/commands, GUI work may build replay harnesses and read-only projections
but must not create private business reducers, direct runtime calls, or fake
success states.

## Available Core surface

- Transport and compatibility: `CoreClient`, `CoreTransport`,
  `StatefulCoreClient`, `LocalCoreTransport`, `CoreHandshake`,
  `CORE_CLIENT_CAPABILITIES`, schema `1`, snapshot, replay, event receive, and
  transcript paging.
- Runtime projection: `RuntimeSnapshot`, `RuntimeEvent`,
  `RuntimeEventEnvelope`, `RuntimeViewState`, `RuntimeErrorView`, and
  `RuntimeSnapshot.ui_preferences: ResolvedUiPreferences`.
- Commands currently available: `SubmitUserInput`, `QueueFollowUp`,
  `CancelActiveTurn`, `SetWorkMode`, `SetPermissionLevel`,
  `RespondToApproval`, provider/model configuration and selection,
  `StartAgentDag`, `StartAgentTask`, `CancelAgentTask`, merge/evidence
  commands, `RetrieveContext`, and `LoadTranscriptPage`.
- Facts currently available: tool calls, approvals, queued inputs, typed tasks,
  typed lanes, evidence, merge gates, context/cost facts, provider health,
  token cost, UI preferences, transcript pages, and generic runtime errors.

## Open requests

| ID | Priority | Blocking GUI task | Core owner | Request | Current gap |
| --- | --- | --- | --- | --- | --- |
| `GUI-CORE-001` | P0 | Task 7 D11 project intake | Core Task 11 | Add typed project intake commands/events: project probe, recent projects/sessions, provider/model health summary for onboarding, `viden.toml` preview/confirm, masked credential handles, and starter-lane intent. | Core `0.3.0` has provider/model configuration and health facts, but no project probe, config preview/confirm, recent project/session surface, or starter-lane onboarding command. |
| `GUI-CORE-002` | P0 | Task 8 D4 lane creation; Task 9 D1 lane rail/worktree board | Core Task 10 | Add typed lane lifecycle commands/events: create lane from role/route/gate/mutation/target/budget, worktree preview/receipt, lane-created event, attach/pause/resume/cancel/close/restart/kill, and owner-scoped lane command receipts. | Core exports `AgentLaneRecord` and lane update events but no command that creates or manages a lane as a first-class operator intent. |
| `GUI-CORE-003` | P0 | Task 10 D6 recovery | Core Task 10/12 | Add structured connection and recovery facts/actions: connecting, disconnected, provider bridge dropped, agent stopped, restart from checkpoint, reconnect, close lane while keeping worktree, and safe recovery receipts. | CoreClient can recover stream gaps and Core can emit generic `RuntimeErrorView`, but GUI has no typed actionable recovery contract for D6 states. |
| `GUI-CORE-004` | P1 | Task 9 D1 cockpit; Task 10 permission/D6 evidence | Core Task 13 | Add stable audit timeline and diff/apply file facts for cockpit review panes: audit event id, source lane/task/session, file/diff summary, test result, permission decision, evidence link, and paginated query. | Core exposes approvals, evidence, merge gates, transcript rows, and generic command/tool records; D1 audit/review panes still need a stable timeline contract instead of parsing transcript display text. |

## Non-blocking partials

- Permission dock can use `ApprovalRequestView` and `RespondToApproval` now.
  GUI must display Core risk/target/scope/default/audit facts and must never
  execute the underlying tool.
- Locale and appearance can use `RuntimeSnapshot.ui_preferences` now. GUI
  renders the resolved preference and exposes local controls later only by
  sending Core-owned configuration intents once those are added.
- D1 fixture replay can start with existing `d1-vertical-slice` facts; any
  missing production command remains a Core request.

## Close criteria

A request can be closed only when Core exports the typed command/event/fact
through `viden-core`, the shared fixture corpus or compatibility docs record
the behavior, and GUI can consume it through `CoreClient` without importing
runtime/provider/tool/session/workflow internals.
