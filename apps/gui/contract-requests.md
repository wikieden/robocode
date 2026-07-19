# GUI Core Contract Requests

Chinese version: [contract-requests.zh-CN.md](contract-requests.zh-CN.md)

These requests are the GUI `0.1.0-alpha.1` inventory against Core `0.3.0`.
They are not GUI implementation workarounds. Until Core lands the missing
facts/commands, the named production screens remain blocked. Framework-neutral,
fixture-only Tasks 2-3 may still build replay harnesses and read-only
projections, but must not create private business reducers, direct runtime
calls, persisted preferences, or fake success states.

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
| `GUI-CORE-001` | P0 | Task 7 D11 project intake | Core Task 11 | Add the typed project-intake foundation already assigned to Task 11: project probe, onboarding provider/model health summary, `viden.toml` preview/confirm, and masked credential handles. | Core `0.3.0` has provider/model configuration and health facts, but no typed project probe or config preview/confirm flow. Recent history and starter-lane creation are deliberately split into `GUI-CORE-007` and `GUI-CORE-002`. |
| `GUI-CORE-002` | P0 | Task 7 D11 starter lane; Task 8 D4 lane creation; Task 9 D1 lane rail/worktree board | Core Task 10 | Add typed lane lifecycle commands/events: create a starter or operator lane from role/route/gate/mutation/target/budget, worktree preview/receipt, lane-created event, attach/pause/resume/cancel/close/restart/kill, and owner-scoped lane command receipts. | Core exports `AgentLaneRecord` and lane update events but no command that creates or manages a lane as a first-class operator intent. Starter-lane onboarding is a lane-creation preset, not a Core Task 11 project fact. |
| `GUI-CORE-003` | P0 | Task 10 D6 recovery | Core Task 10 follow-up | Add structured connection and lane-recovery facts/actions: connecting, disconnected, provider bridge dropped, agent stopped, restart from checkpoint, reconnect, close lane while keeping worktree, and owner-scoped safe recovery receipts. | CoreClient can recover stream gaps and Core can emit generic `RuntimeErrorView`, but Core Task 10 does not yet name the complete actionable D6 connection/recovery surface. Apply/conflict recovery remains with Core Task 12 in `GUI-CORE-006`. |
| `GUI-CORE-004` | P1 | Task 9 D1 audit pane; Task 12 history/audit | Core Task 13 | Add a stable append-only audit timeline: audit event id, source project/lane/task/session, permission decision, evidence/gate/applied-change links, and paginated query. | Core exposes approvals, evidence, merge gates, transcript rows, and generic command/tool records; the GUI still needs the Task 13 audit contract instead of parsing transcript display text. Diff/apply facts are split into `GUI-CORE-006`. |
| `GUI-CORE-005` | P0 | Task 6 production preference controls; Task 12 Settings | Core Task 4 follow-up | Add Core-owned locale/skin/mode/density/motion mutation and persistence commands/events, including restore defaults, resolved-preference confirmation, diagnostics, and precedence-safe durable storage. | Core Task 4 exports typed and resolved preferences, but `CoreClient` exposes no preference mutation/restore command or event-confirmed persistence receipt. GUI may render resolved preferences and use ephemeral spike controls only. |
| `GUI-CORE-006` | P0 | Task 9 D1 diff/test panes; Task 12 trusted local loop | Core Task 12 | Add structured diff/test/apply/conflict/retry facts and receipts linked to lane/task/session/evidence/gate/applied-change ids. | Core has evidence and MergeGate facts, but the Task 12 trust loop must export stable file/diff summaries, test results, apply outcomes, conflict bounce, and retry/revert facts through `viden-core`. |
| `GUI-CORE-007` | P1 | Task 7 D11 recent work; Task 12 history navigation | New Core history task required | Add paginated recent-project and recent-session summaries with stable project/session/lane ids, last-active ordering, availability state, and resume intent/receipt. | No current Core plan task owns the cross-project recent-history query. Core Task 11 covers project onboarding, while transcript paging covers rows inside a known session; neither provides this discovery surface. |

## Non-blocking partials

- Permission dock can use `ApprovalRequestView` and `RespondToApproval` now.
  GUI must display Core risk/target/scope/default/audit facts and must never
  execute the underlying tool.
- Locale and appearance can use `RuntimeSnapshot.ui_preferences` now. GUI
  renders the resolved preference; Tasks 2-3 may exercise ephemeral fixture
  controls, while production controls remain blocked on `GUI-CORE-005`.
- D1 fixture replay can start with existing `d1-vertical-slice` facts; any
  missing production command remains a Core request.

## Close criteria

A request can be closed only when Core exports the typed command/event/fact
through `viden-core`, the shared fixture corpus or compatibility docs record
the behavior, and GUI can consume it through `CoreClient` without importing
runtime/provider/tool/session/workflow internals.
