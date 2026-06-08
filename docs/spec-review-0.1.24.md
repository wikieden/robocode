# RoboCode 0.1.24 Spec Review

Chinese version: [spec-review-0.1.24.zh-CN.md](spec-review-0.1.24.zh-CN.md)

Last updated: 2026-06-07

## Purpose

This document reviews the gap between the current code, product documentation,
and release plan using a spec-first rule. A spec describes intended behavior;
the code must implement it, or the documentation must explicitly mark it as
planned. Future behavior should not be described as already shipped.

This pass focuses on:

- TUI provider turns, Plan mode input, approval, streaming, and scrollback;
- `/connect`, `/provider`, `/models`, and `/setup` interaction semantics;
- ContextBundle, provider error recovery, and live provider smoke;
- AgentTask, side panels, lane/delegate evidence, and runtime layering;
- release gates, screenshot evidence, and test strategy.

## Solid Ground

- Provider, tool, permission, and transcript flow already share the runtime
  path. Plan mode can block mutating tool calls and shell-backed `/test` at the
  core layer.
- `ContextBundle` is included in provider requests and has request compaction
  tests for large histories.
- The TUI has local interaction panels for `/connect` and `/models`, so it no
  longer depends only on command palette completion.
- Provider turns now dispatch through a `TuiRuntime` worker and return control
  to the TUI main loop immediately. Streaming deltas, approval requests,
  cancel, resize, scroll, and finish/error events are consumed by the same main
  event loop instead of the former `run_provider_turn_interactive` loop.
- `/quit`, `/exit`, resize redraw, CJK display-width handling, provider
  telemetry, recent files, LSP snapshots, lane artifacts, and Codex job
  evidence all have implementation foundations.
- Release gate, daily-loop smoke, plan-mode smoke, and DeepSeek development
  scenario smoke scripts exist.

## P0 Gaps

| Priority | Gap | Code Area | Impact | Spec Target |
| --- | --- | --- | --- | --- |
| P0 | Active-turn queue still needs durable core/runtime ownership | `robocode-cli/src/tui/state.rs` `PendingTurn`, `robocode-cli/src/tui/app.rs` `queue_active_turn_input` | The TUI main loop can keep accepting input while a provider turn runs, queued prompts are preserved across success/failure, and queued count is visible through the shared `AgentTask` projection; durable core/no-TUI queue ownership is still future work | Queue remains visible in `AgentTask` with cancel, retry, restore-all behavior, and side-panel evidence; durable queue ownership can move into core after 0.1.24 if non-TUI surfaces need it |
| P0 | Non-blocking provider-turn smoke needs stronger terminal coverage | `robocode-cli/src/tui/app.rs`, `scripts/plan-mode-smoke.sh` | Unit tests prove a fake slow provider starts without blocking the UI thread, but manual Terminal/iTerm2 evidence and a broader input/resize/models smoke are still required | While a fake slow provider runs, users can type, edit, scroll, resize, open `/models`, cancel, and see queued count |
| P0 | Streaming deltas no longer force bottom scroll, and the transcript badge now marks new output while viewing history | `robocode-cli/src/tui/app.rs`, `robocode-cli/src/tui/render.rs` | Users inspecting history are no longer pulled back to the bottom and now see `history N · new output`; richer jump-to-latest affordances still belong in the zero-bug pass | Auto-follow only when the user is already at the bottom; otherwise show a new-output marker |
| P0 | 413 / argument-too-long recovery is partially automated, but still needs full launch-path audit | `robocode-core/src/runtime_loop.rs`, `robocode-tools/src/shell.rs`, `robocode-core/src/agent_commands.rs`, `robocode-cli/src/tui/screen.rs` | Provider 413 now retries once with compacted context, and known shell/ACP/screen long-payload paths avoid huge argv; remaining future launch surfaces still need verification before the zero-bug gate | Provider error classifier plus shrink/retry; every shell, lane, ACP, and screen launch path uses tempfile/stdin with audit evidence |

## P1 Gaps

| Priority | Gap | Code Area | Impact | Spec Target |
| --- | --- | --- | --- | --- |
| P1 | Provider/model interaction is split between TUI panels and core command text fallback | `robocode-cli/src/tui/app.rs`, `robocode-core/src/provider_commands.rs` | Some paths still show command instructions instead of actionable forms | TUI uses direct forms and pickers; core command output is only a no-TUI fallback |
| P1 | Global `/models` is narrowed to active/favorite models, but recent-model persistence is still local/UI-light | `robocode-cli/src/tui/app.rs` model picker construction | Global picker no longer pulls every descriptor known model; provider-scoped setup still exposes known candidates. Recent persistence and richer favorite management remain thin | Global `/models` reads active/favorite/recent only; provider-scoped setup shows known candidates |
| P1 | Provider doctor/probe can still run synchronously through `run_settings_command` | `robocode-cli/src/tui/app.rs` settings command path | The panel can freeze once doctor becomes a real network probe | Doctor/probe are background jobs with tail, status, evidence, and cancel |
| P1 | The product view model boundary is not formal | `robocode-cli/src/tui/state.rs` `agent_tasks` projection | TUI still mixes runtime snapshots, transcript-derived tasks, workspace scans, and local pending state | Core exposes `RuntimeViewSnapshot`; TUI renders a product view model only |
| P1 | Provider capability differences do not yet have a complete adapter layer | `robocode-model/src/providers.rs`, `robocode-model/src/adapters.rs` | DeepSeek, DashScope, OpenRouter, Anthropic, and OpenAI-compatible differences can leak into UI behavior | Each provider descriptor declares auth, endpoint, models, tool semantics, stream fields, error mapping, and retry policy |

## P2 Gaps

- `docs/tui-cockpit-design*.md` should now describe the real boundary:
  provider turns run in the `TuiRuntime` worker and feed the main event loop,
  while runtime-visible queue state and full smoke evidence remain open.
- The main TUI design docs now use `RoboCode` or internal roles, such as
  `RoboCode is planning`, `Operator is reviewing context`, or `Tool runner is
  waiting for approval`. Older release status files, historical screenshots, or
  audit docs may still contain `DeepSeek is thinking`; keep those as history,
  but do not use them as new UI copy.
- The docs are mostly bilingual, but the older `docs/code-agent-benchmark.md`
  is English-only. If it remains user-facing, add a `zh-CN` counterpart or move
  it into internal research.

## Spec Correction Rules

1. Anything described as "implemented", "current", or "available" must map to
   code, tests, screenshots, or smoke evidence.
2. Target behavior in roadmap or release plans must include an acceptance gate.
3. TUI must not enter nested input loops for provider turns, approval, doctor,
   context building, lanes, or tool execution.
4. Provider/model setup must be direct interaction: select, edit, Enter applies,
   Esc cancels. Command text is only a no-TUI fallback.
5. Every user-visible feature needs a real terminal screenshot or deterministic
   preview before completion.

## Priority Development Plan

### P0-A TurnController And One Main Event Loop

- Add `TurnController` or equivalent state for active turns, queued turns,
  cancel, streaming deltas, approval requests, tool/lane jobs, and final
  result.
- The TUI main loop only receives terminal input, worker events, timers, and
  resize, then updates state/render.
- Remove active-turn-specific keyboard loops. `handle_submitted_input` should
  return after dispatching work, not after the provider turn completes.
- Current progress: `TuiRuntime` now owns the `SessionEngine` on a worker
  thread. `handle_submitted_input` starts a provider turn and returns
  immediately; the main loop consumes stream, approval, cancel, and
  finish/error events. The old `run_provider_turn_interactive` /
  `poll_active_turn_input` path has been removed. A fake slow provider unit test
  proves turn dispatch returns without waiting for provider completion.

Acceptance:

- While a fake slow provider runs for 30 seconds, users can type, edit, scroll,
  resize, open `/models`, and see the queued count.
- After `/plan on`, submitting a long planning task does not lock or drop the
  next input.

### P0-B Non-blocking Approval

- Convert approval prompts into `InteractionPanel::Approval` or a dedicated
  modal state.
- Remove direct `event::read()` from approval flow.
- Keyboard, mouse, resize, and scroll go through the same main event loop.
- Current progress: the blocking approval reader and active-turn event pump
  were removed. Approval now uses an `ActiveApproval` callback object handled by
  the TUI main loop. A later cleanup can convert it into a first-class
  `InteractionPanel::Approval`, but it no longer requires a nested input loop.

Acceptance:

- During approval, mouse click, `y/n/Enter/Esc`, resize, and scroll all work.
- After approval resolves, the modal disappears immediately and the pending
  task becomes accepted or denied.

### P0-C Core-visible Queue, Error Recovery, And Scrollback

- Queued follow-ups move into runtime snapshots instead of only
  `PendingTurn.queued_inputs`.
- On failure, restore all queued drafts and record which are preserved and which
  are waiting to retry.
- Streaming gains follow-mode state; when the user scrolls away from the
  bottom, new tokens no longer steal the viewport.
- Current progress: streaming deltas no longer reset `transcript_scroll` to the
  bottom, failed active turns restore the first queued draft while listing the
  remaining preserved drafts, successful turns preserve remaining queued prompts
  behind the next automatically started turn, and queued count appears in
  `AgentTask.summary`, evidence, and next action. Durable core/no-TUI queue
  ownership remains future work.

Acceptance:

- Type 3 follow-ups while a provider is active; after provider failure, all 3
  are restored or remain visibly queued.
- Scroll up during streaming; the viewport does not jump to the bottom, and a
  `history N · new output` marker appears in the transcript badge.

### P0-D Context Failure Recovery

- Add a provider error classifier for 413, 429, 401, 404 model missing, timeout,
  network, unsupported tools, and invalid tool result sequence.
- 413 now retries once after shrinking the provider request view and records a
  compaction note in transcript/events.
- The builtin shell tool uses stdin for long commands. ACP shell startup and TUI
  side-screen shell templates now write oversized launch commands to temporary
  scripts, preserving protocol stdin where needed.
- Remaining future launch surfaces, including new lane adapters, must keep the
  same no-large-argv invariant.

Acceptance:

- A deterministic 413 provider fixture triggers shrink/retry.
- Long shell payloads no longer trigger OS `Argument list too long` on the
  builtin shell tool, ACP startup path, or TUI side-screen template path.

### P1-A Provider Setup Forms

- `/connect` first lists provider names only; Enter opens provider-specific
  setup.
- API-key providers open key input; web-login providers show a login URL/action;
  local providers show local health/action.
- After save, open the provider-scoped model picker. Global `/models` shows
  only configured providers and active/favorite/recent models.
- Current progress: global `/models` now uses active/favorite model rows only;
  provider-scoped setup still shows descriptor default and known model
  candidates so a configured provider can activate additional models without
  polluting the global picker.

Acceptance:

- DeepSeek key can be updated, deleted, and set again. The key is displayed only
  with prefix/suffix masking.
- Unconfigured providers do not appear in global `/models`, but their known
  candidates remain available inside `/connect` provider-scoped setup.

### P1-B Async Doctor, RuntimeViewSnapshot, And Provider Adapter Matrix

- Doctor/probe run as background jobs.
- Core exposes `RuntimeViewSnapshot`; right rail, side-1, side-2, and NOW
  WORKING read the same data.
- Provider descriptors gain a capability matrix: auth, models, stream fields,
  tool behavior, context limit, and error recovery.

Acceptance:

- Provider doctor does not freeze the UI.
- Side panels and main status show the same task id/status/evidence for the same
  turn.

## Test And Release Gates

- `cargo fmt --check`
- TDD testing contract smoke: `scripts/tdd-testing-contract-smoke.sh`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/tui-turn-controller-smoke.sh`
- `scripts/tui-regression.sh docs/previews/generated`
- `scripts/plan-mode-smoke.sh /tmp/robocode-0124-plan-mode-smoke`
- `scripts/daily-loop-smoke.sh /tmp/robocode-0124-daily-loop-smoke`
- fake slow provider non-blocking TUI smoke
- deterministic approval non-blocking smoke
- deterministic 413 shrink/retry smoke
- `scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash`
- `scripts/release-gate.sh --version 0.1.24`
- `scripts/release-gate.sh --version 0.1.24 --phase postpublish`

## Documentation Actions

- `docs/release-0.1.24-plan*.md` must reference this spec review as a release
  gate.
- `docs/testing-validation-plan*.md` must add a spec drift gate so docs do not
  lead implementation again.
- `docs/tui-cockpit-design*.md` now describes the real implementation boundary;
  future reviews should prevent target behavior from being written as landed.
- `docs/modules*.md` must list this file as a roadmap/reference document.
