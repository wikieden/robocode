# RoboCode 0.1.7 Plan

Chinese version: [release-0.1.7-plan.zh-CN.md](release-0.1.7-plan.zh-CN.md)

Last updated: 2026-05-26

Related adapter note:
[codex-app-server-adapter.md](codex-app-server-adapter.md)

## Goal

`0.1.7` should continue improving the real programming experience and turn the
0.1.6 TUI cockpit, lanes, and extension visibility into a practical
multi-agent orchestration workbench.

Version theme:

```text
0.1.7 = Codex Adapter + Agent Orchestration Backbone
```

RoboCode should not be just a polished TUI, and it should not simply launch
Codex, Claude Code, DeepSeek, or other tools in terminals. It should become a
local multi-agent cockpit: the user gives the primary goal, RoboCode can split,
dispatch, observe, approve, and converge work while different coding agents
collaborate through one shared mechanism.

The core reference for this iteration is OpenAI's Codex plugin for Claude Code:
[`openai/codex-plugin-cc`](https://github.com/openai/codex-plugin-cc). That
plugin proves the product pattern we want: a host coding agent can call Codex
through a plugin/command/subagent surface, keep background jobs observable, and
resume or inspect Codex work without forcing the user to switch tools. RoboCode
should turn that pattern into a first-class local agent adapter instead of
treating Codex as just another terminal command.

## Next Iteration Core: Host-Delegate Agent Bridge

The next implementation pass should center on one product loop:

```text
RoboCode host -> delegate agent -> observable job -> evidence -> operator decision
```

In this loop, RoboCode is the host cockpit and Codex is the first delegate
agent. The Claude Code Codex plugin shows the shape, but RoboCode should make
the pattern generic enough for Claude Code, DeepSeek TUI, tmux/PTY agents, and
future ACP-compatible agents.

Core design rules:

- A delegate agent is not a raw terminal. It has a descriptor, readiness
  doctor, launch command, job record, event/evidence stream, cancel path, and
  optional resume handle.
- Every delegate task enters the same lane lifecycle: queued, running,
  waiting for approval, blocked, done, failed, archived.
- The main TUI must show active delegate work in the operation center within
  one refresh tick, so the user never has to guess whether the remote agent is
  thinking, editing, testing, or stuck.
- Results are actionable evidence, not transcript decoration: changed files,
  commands, tests, final output, errors, and thread/session IDs should be
  queryable through `/agent status`, `/agent result`, `/lane inspect`, and the
  side-screen evidence panels.
- Write-capable delegate work stays behind RoboCode permissions and approval.
  Read-only review can be lightweight, but mutation must not bypass the shared
  tool/runtime/transcript path.

Implementation priority for this core:

1. Finish the Codex job/event adapter until it exposes thread IDs, touched
   files, command/test evidence, and resume hints.
2. Make the main operation center consume the same job/evidence model as the
   side screens.
3. Generalize the adapter contract so plugin, skill, MCP, tmux/PTY, and ACP
   agents can reuse the same lifecycle instead of each command inventing a
   separate status format.
4. Add one real write-capable delegated task path with explicit approval, then
   use it as the template for Claude/DeepSeek/ACP backends.

## Problem Statement

The next trial-focused issues fall into three groups:

- Runtime state is not strong enough: after the user submits input, the center
  of the main screen should continuously say whether RoboCode is thinking,
  editing, testing, waiting for approval, or supervising lanes.
- The extension system is still mostly read-only: plugins, skills, and MCP now
  have visibility, but they do not yet form a developer-experience-oriented
  loading, diagnostics, invocation, and permission model.
- Multi-agent work is still terminal-integration-first: tmux, PTY, and template
  adapters can launch external tools, but RoboCode should move toward the Zed
  ACP direction and make different coding agents first-class lane backends
  behind a unified adapter boundary.
- The strongest immediate adapter target is Codex itself: Claude Code's Codex
  plugin exposes `/codex:review`, `/codex:rescue`, `/codex:status`,
  `/codex:result`, `/codex:cancel`, and `/codex:setup`, backed by a companion
  runtime and Codex app-server integration. RoboCode should support the same
  operator loop natively.

## Release Definition

`0.1.7` is successful when RoboCode feels useful during the live programming
loop, not only after a run finishes. The user should be able to submit a task,
see what the primary agent is doing, watch side-agent lanes progress, approve
or reject changes, run tests, and understand extension/MCP failures without
leaving the cockpit.

Hard release gates:

- Codex is a first-class agent backend with setup/doctor, review, task,
  status, result, cancel, and resume-style flows.
- The main screen has a real operation center backed by runtime evidence.
- The composer, approval overlay, and resize behavior are stable enough for
  daily interactive use.
- Side-1 and side-2 use real lane, test, LSP, MCP, extension, and evidence
  state instead of preview-only placeholders.
- External agents share one lane lifecycle and one operator decision language.
- ACP has a documented adapter boundary plus a working handshake/event-log
  spike, even if full ACP editing is still experimental.
- Plugin, skill, MCP, tool, and agent extension kinds have a documented
  descriptor shape, diagnostics path, and permission boundary.

Cut line:

- If time is tight, keep full ACP task execution and automatic task splitting
  experimental.
- Do not cut the Codex adapter, live operation center, composer usability, lane
  lifecycle, or extension diagnostics. Those are the programming-experience
  foundation.

## Reference Model: Codex Plugin for Claude Code

RoboCode should explicitly learn from `openai/codex-plugin-cc`, not copy its
Node implementation. The reference design has five pieces worth preserving:

- Plugin/command surface:
  `/codex:review`, `/codex:adversarial-review`, `/codex:rescue`,
  `/codex:status`, `/codex:result`, `/codex:cancel`, and `/codex:setup`.
- Thin local runtime:
  a companion script checks Codex availability/auth, launches Codex work, stores
  job records, and renders status/result output.
- Protocol-backed integration:
  the plugin uses the local `codex` binary and Codex app-server rather than
  only scraping terminal text.
- Background job model:
  long-running reviews and rescue tasks can continue in the background while
  the host tool shows status and final results later.
- Safety posture:
  review defaults to read-only, write-capable rescue is explicit, and optional
  review gates are visible because they can create loops and consume usage.

RoboCode translation:

- `/agent doctor codex` replaces `/codex:setup`.
- `/agent review codex [--base <ref>]` replaces `/codex:review`.
- `/agent challenge codex ...` replaces `/codex:adversarial-review`.
- `/agent run codex [--write] <task>` or `/lane codex <task>` replaces
  `/codex:rescue`.
- `/agent status`, `/agent result <id>`, and `/agent cancel <id>` cover
  background job management.
- Codex app-server events become RoboCode lane events, evidence records, and
  side-screen rows.

## Milestones

### M1: Live Cockpit Stability

Focus: fix the everyday feel before adding more orchestration surface.

- Main-screen operation center.
- Composer height, blinking cursor, and CJK IME placement.
- Resize redraw and border alignment.
- Approval overlay focus, dismissal, and post-action cleanup.

Exit criteria:

- A user can type, approve, reject, resize, and continue without visual drift or
  hidden state.
- Screenshots/previews cover idle, thinking, tool call, approval, and test
  result states.

### M2: Evidence-Driven Programming Loop

Focus: make edit/test/review visible and actionable.

- `/test` evidence model and side-2 rendering.
- Edit summaries with file, delta, approval, and write result.
- Diff/review entry points for the current round of changes.
- Recent evidence timeline for tools, tests, lanes, and approvals.

Exit criteria:

- The demo workflow "create file -> approve -> run test -> inspect result" can
  complete inside TUI.
- The latest failure summary and changed files are visible without reading raw
  transcript history.

### M3: Codex Adapter Core

Focus: make Codex the first protocol-backed external coding agent in RoboCode.

- Codex availability/auth doctor.
- Codex app-server process or broker boundary.
- Review and adversarial-review flows.
- Task/rescue flow with read-only and write-capable modes.
- Background job records with status/result/cancel/resume.
- Mapping from Codex thread/turn/events to RoboCode lane/evidence records.

Exit criteria:

- A user can run a Codex review from RoboCode, see progress, fetch the result,
  and resume the Codex session if needed.
- A user can hand a bounded task to Codex, observe it as an agent lane, and see
  changed files, commands, tests, and final output as RoboCode evidence.

### M4: Agent Lane Operator Loop

Focus: turn external tools into supervised collaborators.

- Normalized lane states.
- `/lane inspect`, `/lane send`, `/lane revise`, `/lane accept`,
  `/lane discard`, and `/lane apply` decision loop.
- Side-1 real lane evidence and next-action priority.
- tmux/PTY/template lane observation hardening.

Exit criteria:

- A tmux or PTY coding-agent lane can be started, observed, followed up, and
  accepted or discarded with visible evidence.

### M5: Extension Foundation

Focus: make plugin, skill, MCP, tool, and agent surfaces diagnosable before
making them powerful.

- Unified extension descriptor.
- `/extensions doctor` and richer `/skills list`.
- MCP context/config status in side-2.
- Shared permission/runtime/transcript path for extension invocation.

Exit criteria:

- Missing MCP config, missing binaries, disabled skills, and failed extension
  health checks produce actionable diagnostics.

### M6: ACP Bridge Spike

Focus: prove the future multi-agent protocol direction without destabilizing
the local cockpit.

- ACP process transport boundary.
- Handshake and JSONL event log.
- Event mapping design for text, edit, tool, permission, and completion events.
- `/agent doctor acp` readiness and protocol evidence.

Exit criteria:

- A mock ACP-compatible process can handshake, emit events, and leave replayable
  evidence that maps cleanly to lane artifacts.

## P0: Must Ship

### 1. Main-Screen Operation Center

Goal: the main screen should always answer "what is RoboCode doing right now?"

Deliverables:

- Show the current primary operation in the transcript center or fixed live
  activity area: `Thinking`, `Editing <file>`, `Running tests`,
  `Waiting approval`, `Supervising <n> lanes`, or `Idle`.
- Attach evidence to each state. Valid sources include provider requests, tool
  calls, pending approvals, test events, lane artifacts, and transcript events.
- For long operations, show duration and useful context such as the active
  file, command, lane ID, token movement, or event movement.
- Keep the state visible on the main screen, not only in side screens, so the
  user can decide whether to wait, approve, interrupt, or switch lanes.

Acceptance checks:

- Within 200ms after Enter, the main screen shows visible work state.
- Streaming provider responses, running tools, approval blocks, and background
  lanes render as distinct states.
- When nothing is running, the UI shows `Idle` or the latest completion
  summary, not fake progress.

### 2. Programming Feedback Loop

Goal: make the edit, diff, test, fix, and confirm loop smooth.

Deliverables:

- `/test` results appear in the main screen and side-2 evidence instead of
  remaining only as transcript text.
- File-edit tool calls aggregate into scan-friendly summaries: files, line
  delta, approval status, and whether the write landed.
- Add or improve diff/review entry points so the user can jump from the TUI to
  the current round of changes.
- Approval overlays keep default focus on approve, while the main screen clearly
  explains why work is blocked.
- The composer remains highly visible: blinking cursor, correct CJK IME
  placement, and enough height for longer inputs.

Acceptance checks:

- A demo can complete "create file -> approve -> run test -> inspect result"
  without leaving the TUI.
- The user can find the latest test output, failure summary, and related files
  from the main screen or side-2.

### 3. Codex Adapter and Job Runtime

Goal: make Codex the first-class external agent backend, using the Claude Code
Codex plugin as the reference workflow.

Deliverables:

- `/agent doctor codex` checks `codex` binary availability, app-server support,
  auth readiness, config source, and workspace trust/setup state.
- `/agent review codex` runs a read-only Codex review of the working tree or a
  base branch, with foreground/background modes.
- `/agent challenge codex` runs a steerable adversarial review focused on
  assumptions, tradeoffs, and failure modes.
- `/agent run codex [--write] <task>` starts a tracked Codex task; write-capable
  runs must be explicit and permission-gated.
- `/agent status`, `/agent result <id>`, `/agent cancel <id>`, and
  resume/follow-up handling work for Codex jobs.
- Codex app-server notifications, final output, touched files, command
  executions, test evidence, and thread IDs are persisted as RoboCode evidence.

Current implementation status:

- Landed: `/agent doctor codex` checks the Codex command, version,
  `app-server` availability, auth status, config sources, and job-store path.
- Landed: `/agent review codex`, `/agent challenge codex`, and
  `/agent run codex [--write] <task>` start tracked Codex CLI jobs with per-job
  log and result artifacts under `.robocode/agents/`.
- Landed: `/agent run codex --write <task>` is the explicit write-capable
  delegate path. It asks through RoboCode's mutating permission path before
  launch and uses Codex `workspace-write` sandbox only after approval.
- Landed: `/agent status`, `/agent result <id>`, and `/agent cancel <id>` read
  and control the tracked job records in `.robocode/agents/codex-jobs.jsonl`.
- Landed: Codex jobs now keep a start-time Git status baseline and extract
  resume/session hints plus touched-file evidence from job output, so
  `/agent status` and `/agent result <id>` can show `codex resume ...` and
  related files when available.
- Landed: the TUI workspace snapshot reads tracked Codex jobs so the main
  `LIVE ACTIVITY` strip and right-rail `ACTIVE TASKS` panel show active Codex
  work instead of leaving the user guessing after submission.
- Landed: `/extensions doctor` and `/mcp doctor` now report readiness by
  surface, including provider plugin dirs, MCP config files and server names,
  project/user/legacy skill roots, and permission boundary reminders.
- Landed: `/agent doctor codex` probes the experimental app-server JSON schema
  surface and reports whether thread lifecycle, review, turn control, event,
  evidence, and approval protocol groups are available.
- Landed: `/agent probe codex` performs a live app-server `initialize` handshake
  over stdio and writes replayable JSONL response/notification evidence.
- Landed: `/agent probe codex --thread` starts an ephemeral read-only Codex
  app-server thread and captures structured `threadId` / `thread/started`
  evidence without running a model turn.
- Landed: `/agent probe codex --turn <task>` starts a read-only app-server turn
  and captures structured turn/item/completion event evidence.
- Landed: completed app-server turn probes now write tracked Codex job records
  and result summaries so `/agent status`, `/agent result`, and the TUI job rail
  can surface the structured thread/turn evidence.
- Remaining: make normal `/agent run codex` jobs use the app-server event path
  asynchronously, with approval request routing, while keeping CLI fallback.
- App-server protocol findings are captured in
  [codex-app-server-adapter.md](codex-app-server-adapter.md).

Acceptance checks:

- A read-only Codex review can be started, monitored, and rendered in the TUI.
- A background Codex task can be checked through `/agent status` and fetched
  through `/agent result`.
- Write-capable Codex work never bypasses RoboCode permissions, transcript, or
  approval.

### 4. Agent Lane Lifecycle

Goal: external coding agents should have lifecycle, evidence, and operator
decisions, not merely a launched terminal.

Deliverables:

- Normalize lane states to: `queued`, `thinking`, `editing`, `testing`,
  `needs input`, `waiting approval`, `blocked`, `done`, `failed`, and
  `archived`.
- `/lane inspect <id>` shows objective, transport, workspace, latest output,
  changed files, test evidence, next action, and decision history.
- `/lane send <id> <text>`, `/lane accept`, `/lane revise`, `/lane discard`,
  and `/lane apply` form a clear operator loop.
- Side-1 remains the agent-lanes cockpit and prioritizes real lane evidence
  over preview data.

Acceptance checks:

- After starting a Codex/Claude/DeepSeek-like external tool through tmux or PTY,
  RoboCode can keep observing latest output and suggest the next operator
  action.
- When a lane completes, the result evidence is visible and the user can accept,
  revise, discard, or apply it.

## P1: Should Ship

### 5. Usable Extension System V1

Goal: plugins, skills, and MCP should be diagnosable, invokable, and
extensible, not just visible.

Deliverables:

- Define one extension descriptor:
  - `id`
  - `kind: plugin | skill | mcp | tool | agent`
  - `source`
  - `capabilities`
  - `permissions`
  - `health`
  - `entrypoints`
- `/extensions doctor` produces actionable diagnostics for missing binaries,
  missing config, insufficient permissions, schema errors, and incompatible
  versions.
- `/skills list` includes skill summaries and trigger hints instead of only
  paths.
- MCP configuration enters runtime context: side-2 shows enabled MCP servers,
  config sources, and errors.
- All extension invocation must go through the shared permission and runtime
  path, never around transcript recording.

Acceptance checks:

- The user can run `/extensions doctor` and understand why an MCP server or
  skill is unavailable.
- Side-2 distinguishes configured, ready, failed, and disabled extensions.

### 6. ACP Adapter Spike

Goal: establish the protocol boundary for supporting more coding agents.

Deliverables:

- Add an ACP adapter design document or module boundary that explains how ACP
  maps into RoboCode lane events.
- Complete a minimal process transport spike:
  - launch an agent server;
  - perform handshake;
  - record JSONL events;
  - map text, edit, tool, and permission events into lane artifacts.
- `/agent doctor <id>` can distinguish template, tmux, PTY, and ACP readiness.

Acceptance checks:

- A mock ACP server or minimal compatible server can complete handshake and
  event logging.
- 0.1.7 does not need a full ACP editing loop, but the event model must be
  clear.

### 7. Real Side-2 Ops Screen

Goal: side-2 becomes the operations panel for tests, LSP, MCP, extensions, and
evidence.

Deliverables:

- `TESTS / LSP`: latest test command, status, duration, failure summary, and
  LSP diagnostics.
- `MCP / CONTEXT`: MCP config sources, context window, and workspace snapshot.
- `EXTENSIONS`: provider, agent, skill, and MCP ready/failed/disabled counts.
- `RECENT EVIDENCE`: latest tool, test, and lane artifacts instead of generic
  chat summaries.

Acceptance checks:

- After `/test`, side-2 shows test results.
- Missing or invalid MCP configuration produces a clear side-2 message.

## P2: Exploratory

- Agent task planner: split a large objective into lane tasks, still requiring
  user confirmation.
- Lane scheduling policy: concurrency limits, agent choice, and
  workspace/worktree choice.
- Remote/desktop companion: reserve a state protocol for future desktop or
  editor integrations.
- Optional task graph view: show dependencies between the main agent and side
  agents.

## Non-Goals

- No cloud agent registry.
- No account system or remote task hosting.
- Do not turn RoboCode into a full IDE.
- Do not add a marketplace before the extension system is stable.
- Do not let plugins, skills, MCP, or ACP bypass permissions, transcript, and
  approval.

## User-Facing Success Criteria

- After submitting a task, the user no longer has to guess whether the model is
  working.
- The user can see current action on the main screen, side agents on side-1,
  and evidence plus diagnostics on side-2.
- Codex, Claude Code, DeepSeek, shell jobs, and future ACP agents use the same
  lane/status/approval/evidence language in the TUI.
- Codex specifically feels native: setup, review, rescue/task, background
  status, result replay, cancellation, and resume are available without opening
  a separate terminal.
- Plugin, skill, and MCP problems are diagnosable instead of appearing as
  silent no-ops.
- A real small-feature workflow can finish inside the TUI: enter the request,
  approve edits, run tests, inspect results, and accept or revise lane output.

## Suggested Build Order

1. Codex adapter core: implement the concrete external-agent workflow first,
   using the Claude Code Codex plugin as the reference product shape.
2. Main-screen operation center: make Codex and RoboCode activity visible while
   work is running.
3. Side-2 evidence panel: give tests, LSP, MCP, extensions, and Codex job
   evidence one observation surface.
4. Lane lifecycle polish: turn Codex, tmux, PTY, and template tools into one
   operator loop.
5. Extension descriptor and doctor: build diagnostics and boundaries before
   complex execution.
6. ACP spike: validate the general protocol direction after Codex proves the
   concrete adapter model.

## Verification Gate

- `cargo fmt --check`
- focused tests for touched crates
- `cargo test --workspace --quiet`
- TUI previews for main screen, side-1, and side-2 as screenshots or text
  snapshots
- at least one fallback/provider smoke covering:
  - prompt submit -> live activity
  - file edit approval
  - `/test`
  - Codex setup/review/status/result or a mock Codex app-server equivalent
  - side-1 lane evidence
  - side-2 ops evidence
