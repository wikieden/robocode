# Viden 0.1.12 Plan

Chinese version: [release-0.1.12-plan.zh-CN.md](release-0.1.12-plan.zh-CN.md)

Last updated: 2026-05-27

## Positioning

`0.1.12` is the Agent Orchestration Operator Loop release.

`0.1.11` established the TUI reliability baseline, `NOW WORKING`,
`AgentTask` / `AgentLane` projections, screenshot regression, and the
token/context design. `0.1.12` should turn that foundation into a real
programming workflow:

- the user gives Viden a development task;
- Viden can dispatch supervised agent/lane work;
- the main screen explains who is working, what they are doing, where the
  evidence is, and what decision is needed next;
- side screens become real observe/control/review/apply surfaces;
- token/context inputs and outputs start to be budgeted and compacted.

This remains a `0.1.x` release. The target is a usable operator loop, not a full
`0.2.0` multi-agent runtime claim.

## Previous Release Retrospective

### 0.1.8: AgentTask Was The Right Foundation, But The Slice Was Wide

`0.1.8` introduced the `AgentTask` runtime view, operation center, side-2
evidence, Codex app-server fixtures, and lane operator-loop smoke. That proved
the direction: Viden needs one fact layer instead of each panel inventing its
own status.

Lessons:

- The release covered provider, tool, lane, Codex, diff, test, and approval
  surfaces, but the user-facing "complete programming loop" was still not crisp
  enough.
- `AgentTask` had useful fields, but runtime writes, task priority, and next
  actions need to behave more like a product workflow than a projection.

### 0.1.9: Verification Became A Product Asset

`0.1.9` standardized release smoke, clippy gating, TUI regression screenshots,
and post-publish checks.

Lessons:

- Deterministic screenshots are excellent layout-regression evidence, but they
  do not replace real terminal interaction checks.
- Future TUI features need both snapshot evidence and a manual terminal
  checklist.

### 0.1.10: Users Need To Know Whether The Remote Is Working

`0.1.10` created a pending `AgentTask` before provider calls, so the main screen
could show provider thinking.

Lessons:

- This is high-value feedback: `NOW WORKING` should first reduce uncertainty
  around "is anything happening?"
- Provider turns are only one remote-work class. Shell/test, lanes, approval,
  and review/apply must enter the same mechanism.

### 0.1.11: TUI Reliability Is Necessary, Not Sufficient

`0.1.11` strengthened resize, CJK input, `NOW WORKING` naming, `AgentLane`
projection, token/context design, and the full GitHub/Homebrew release loop.

Lessons:

- TUI reliability is the gate for multi-agent work, but the core user value is
  still orchestrating multiple coding tools to finish tasks.
- Side screens must become operator control surfaces, not status showcases.
- Token/context work cannot stay documentation-only; it should affect at least
  one provider/lane prompt.

## Release Cutline

The `0.1.12` cutline is: **one usable operator-loop vertical slice**.

Priorities:

1. **P0: unified runtime fact layer.** Provider, tool, shell/test, lane, and
   approval actions must write `AgentTask` state that `NOW WORKING`, side-1, and
   side-2 read consistently.
2. **P0: one stable lane loop.** Use the deterministic `shell/template` lane as
   the testable baseline for dispatch -> observe -> review -> apply/discard.
   Codex/Claude should reuse the same model, but completing every external
   agent is not P0.
3. **P0: ContextBundle v0 affects a real prompt.** At least one provider turn or
   delegated lane should record context sources, estimate tokens, and compact
   long tool output.
4. **P1: deepen Codex/Claude adapters.** After the P0 loop is stable, map
   Codex/Claude status, tail, result, and review/apply into the same operator
   loop.
5. **P1: real terminal acceptance.** Terminal/iTerm2 CJK input, resize,
   approval, and mouse behavior should keep manual screenshots or screen
   evidence.
6. **P2: extension/ACP/MCP expansion.** Keep descriptor, doctor, probe,
   capability, and event mapping work from taking focus away from the P0 loop.

## Goals

### 1. Make AgentTask The Runtime Fact Layer

`0.1.11` made the TUI read from a shared projection. `0.1.12` should move core
runtime actions into the same `AgentTask` lifecycle.

Required coverage:

- provider turns: thinking, streaming, tool-call, completed, failed;
- tool calls: approval required, running, result, failed;
- shell/tests: command, pid/exit, duration, tail, artifact;
- external lanes: Codex, Claude, DeepSeek, shell/template, tmux/PTY start, tail,
  review, apply, stop;
- approvals: pending, approved, denied, default action, decision evidence.

Every `AgentTask` should include at least:

- id, agent/provider/lane, transport, status, started_at, updated_at;
- objective / current_action;
- evidence rows: transcript event, tool call, diff, test result, artifact, log
  tail;
- next_action: wait, approve, inspect, attach, send, review, apply, stop, retry.

### 2. Turn `NOW WORKING` Into The Operation Center

After input submission, the main screen must immediately answer what Viden is
doing now.

Required behavior:

- provider waiting shows thinking/streaming state, elapsed time, provider/model;
- approval waits show approval type, default action, and shortcut hints;
- shell/test execution shows command, elapsed time, and last-output summary;
- external agent/lane work shows lane, transport, stage, and latest evidence;
- when multiple tasks exist, show the highest-priority active task and the
  number of background tasks.

The area must be backed by real `AgentTask` state, not decorative copy.

### 3. Make Side Screens Real Agent Consoles

`side-1` and `side-2` should become operator consoles, not extra dashboards.

`side-1` focuses on agents/lanes:

- list active / completed / failed lanes;
- expose inspect, attach, send, stop, and retry command entry points;
- show each lane's status, elapsed time, last output, artifact, and next action;
- use the same `AgentTask` data as the main `NOW WORKING` area.

`side-2` focuses on evidence/ops:

- summarize tests, diffs, diagnostics, git, tool output, and artifacts;
- show recent failures and blocking reasons;
- support review/apply decisions with evidence instead of placeholders.

### 4. Close The Programming Loop

`0.1.12` should ship the first minimal multi-agent programming loop.

Target flow:

1. The user submits a development task.
2. Viden generates or accepts a small plan.
3. The user dispatches a subtask to a lane such as Codex, Claude,
   shell/template, or DeepSeek.
4. The lane is observed through `AgentTask`.
5. The result enters review with touched files, diff, tests, and evidence.
6. The user can accept/apply/discard/retry.
7. Final state is written to transcript, workflow events, and recent evidence.

Prefer one stable happy path before broadening the agent matrix.

### 5. Define The Extension/Adapter Foundation

This release should shape plugin, skill, MCP, and ACP boundaries so later
implementation is straightforward.

Required work:

- unified descriptor docs for provider plugins, agent adapters, skills, MCP
  servers, and tool surfaces;
- keep `/extensions doctor`, `/mcp doctor`, and `/skills list` grounded in real
  diagnostics;
- keep ACP experimental, prioritizing probe, capabilities, job envelopes, and
  event mapping over a full editor-grade host;
- route every extension invocation through shared permission, transcript,
  evidence, and token-budget boundaries.

### 6. Keep Real Terminal Experience Moving

`0.1.11` added deterministic previews for resize and CJK input. `0.1.12` should
continue tracking real terminal behavior as part of the release.

Required coverage:

- resize redraw in macOS Terminal and iTerm2;
- CJK input candidate placement and visible input cursor;
- approval modal default approve, shortcuts, and mouse clicks;
- side-screen open/close, focus switching, and lane attach/send usability.

SVG snapshots are not enough for this area. Keep real terminal screenshots or
manual verification notes.

### 7. Implement ContextBundle / Token Efficiency v0

`0.1.12` should move from design into a minimal implementation.

Required work:

- build `ContextBundle` v0 for provider turns or delegated lanes;
- record context sources: user task, selected files, diff, diagnostics, tests,
  memory, lane summaries;
- send long tool output as summary + tail while preserving raw output in the
  transcript/audit trail;
- show context pressure, estimated tokens, and largest sources in status/side
  surfaces;
- give each agent/lane budget fields: soft budget, hard limit, current estimate.

### 8. Keep Screenshot Evidence

Every user-visible feature should leave a screenshot or deterministic TUI visual
artifact.

Required evidence:

- provider-thinking `NOW WORKING`;
- shell/test-running `NOW WORKING`;
- approval pending/default approve;
- side-1 active lane controls;
- side-2 evidence/ops;
- lane review/apply decision;
- ContextBundle/token pressure;
- multi-task background state.

## Suggested Implementation Order

### Milestone A: AgentTask Runtime Write Path

- Map current projection-only status sources.
- Add one shared `AgentTask` update path or reducer.
- Connect provider, tool, shell/test, and approval first.
- Add focused tests proving the same task appears consistently in
  `NOW WORKING`, the right panel, and side-2.

### Milestone B: Deterministic Lane Operator Loop

- Use `shell/template` lane as the P0 baseline.
- Implement dispatch, observe, tail, review, apply/discard, retry, and stop.
- Generate TUI screenshots for active lane, review, apply result, and side-2
  evidence.

### Milestone C: ContextBundle v0

- Define and implement the smallest `ContextBundle` construction path.
- Compact long tool output into summary + tail.
- Show context sources, estimated tokens, and pressure in the TUI.
- Test that raw transcript/audit data is preserved while prompt input is
  compacted.

### Milestone D: Codex/Claude Adapter Reuse

- Do not build a second UI. Map Codex/Claude status/result/tail into the same
  `AgentTask` and lane APIs.
- Add adapter doctor/probe and one reproducible smoke.
- Keep unstable real-terminal checks as manual evidence items.

## Non-Goals

- Do not claim the full `0.2.0` multi-agent runtime in `0.1.12`.
- Do not build marketplace, remote collaboration, web, desktop, or IDE entry
  points.
- Do not add the full MCP mutating tool runtime unless it passes through the
  permission/evidence boundary.
- Do not turn ACP into a full Zed-grade host; keep it to adapter/probe/job/event
  foundations.
- Do not add fake panels that only make the UI look more multi-agent.
- Do not broaden every external coding agent at once; first make one operator
  loop truly close.

## Acceptance Criteria

- The user can dispatch a small programming task to at least one agent/lane and
  observe the whole process in the TUI.
- `NOW WORKING`, side-1, side-2, and recent evidence agree on task state.
- Provider/tool/shell/lane actions map into `AgentTask`.
- Review/apply/retry/stop has at least one stable happy path.
- ContextBundle v0 explains which context entered the prompt, the estimated
  token cost, and the largest pressure sources.
- Every user-visible interaction has screenshot evidence.
- Docs clearly distinguish real functionality from experimental adapters and
  read-only diagnostics.
- P0 functionality is covered by deterministic smoke. External-agent unstable
  items are scoped as P1/P2 or manual verification items.

## Verification

Run at minimum:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.12 --deepseek --out-dir /tmp/viden-0112-release-smoke-full
```

Manual checks:

- run the TUI in macOS Terminal and iTerm2;
- run a real provider turn and observe thinking -> streaming -> completed;
- run a real shell/test task and observe running -> result;
- run at least one external lane through dispatch -> observe -> review ->
  apply/discard;
- regress CJK input, resize, approval, and the command palette.

## Follow-Up

After `0.1.12`, choose between `0.1.13` and `0.2.0`:

- if the operator loop is still unstable, ship `0.1.13` for reliability and
  review/apply hardening;
- if the operator loop is stable, move to `0.2.0`: Agent Orchestration Runtime
  v1, the default planner -> worker -> reviewer -> tester workflow, and a
  fuller ContextBundle/token-efficiency engine.
