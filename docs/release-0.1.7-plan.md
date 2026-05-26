# RoboCode 0.1.7 Plan

Chinese version: [release-0.1.7-plan.zh-CN.md](release-0.1.7-plan.zh-CN.md)

Last updated: 2026-05-26

## Goal

`0.1.7` should continue improving the real programming experience and turn the
0.1.6 TUI cockpit, lanes, and extension visibility into a practical
multi-agent orchestration workbench.

Version theme:

```text
0.1.7 = Programming Experience + Agent Orchestration Backbone
```

RoboCode should not be just a polished TUI, and it should not simply launch
Codex, Claude Code, DeepSeek, or other tools in terminals. It should become a
local multi-agent cockpit: the user gives the primary goal, RoboCode can split,
dispatch, observe, approve, and converge work while different coding agents
collaborate through one shared mechanism.

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

### 3. Agent Lane Lifecycle

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

### 4. Usable Extension System V1

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

### 5. ACP Adapter Spike

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

### 6. Real Side-2 Ops Screen

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
- Plugin, skill, and MCP problems are diagnosable instead of appearing as
  silent no-ops.
- A real small-feature workflow can finish inside the TUI: enter the request,
  approve edits, run tests, inspect results, and accept or revise lane output.

## Suggested Build Order

1. Main-screen operation center: first solve the user's uncertainty about
   whether RoboCode is doing work.
2. Side-2 evidence panel: give tests, LSP, MCP, and extensions one observation
   surface.
3. Lane lifecycle polish: turn tmux, PTY, and template tools into an operator
   loop.
4. Extension descriptor and doctor: build diagnostics and boundaries before
   complex execution.
5. ACP spike: validate the protocol direction in parallel without blocking the
   daily programming experience.

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
  - side-1 lane evidence
  - side-2 ops evidence
