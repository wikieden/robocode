# RoboCode 0.1.8 Plan

Chinese version: [release-0.1.8-plan.zh-CN.md](release-0.1.8-plan.zh-CN.md)

Last updated: 2026-05-27

## Version Positioning

`0.1.7` shipped the GitHub release, Homebrew tap, Codex job runtime, initial
operation center, extension/MCP diagnostics, and side-screen foundation. `0.1.8`
builds on that release and focuses on the live programming experience.

Version theme:

```text
0.1.8 = AgentTask + Live Multi-Agent Cockpit
```

Goal: move RoboCode from "can launch and observe agents" to "can clearly
orchestrate multiple coding agents." After the user submits work, the main
screen should continuously show what is happening. Side screens should show
child agents, tests, diagnostics, MCP, extensions, and evidence. Codex, Claude
Code, DeepSeek, shell, tmux/PTY, and future ACP agents should all flow through
one `AgentTask` model.

## P0: Must Ship

### 1. Unified AgentTask Runtime Model

Goal: every active work item becomes one observable task model before reaching
the main screen, side screens, and command surfaces.

Deliverables:

- Define an `AgentTask` runtime view covering the primary RoboCode reply,
  provider turns, tool calls, approvals, test runs, shell jobs, Codex jobs,
  Claude/DeepSeek lanes, tmux/PTY bridges, and future ACP sessions.
- Minimum fields: `id`, `parent_id`, `agent`, `kind`, `transport`, `status`,
  `activity`, `summary`, `progress`, `started_at`, `updated_at`, `workspace`,
  `evidence`, `permissions`, `decision`, `result`, and `resume_handle`.
- Use one status set: `queued`, `thinking`, `streaming`, `editing`,
  `running_tool`, `testing`, `waiting_approval`, `needs_input`, `blocked`,
  `done`, `failed`, `cancelled`, and `archived`.
- `AgentTask` does not replace transcripts, lane artifacts, Codex job records,
  or test evidence. It normalizes those sources of truth and must not invent UI
  tasks.

Acceptance checks:

- The same Codex job appears in `/agent status`, the right rail, the main
  operation center, and side screens with the same id, status, and evidence
  source.
- The primary reply, tool call, approval, test run, and external lane can be
  sorted in one list, making the active blocker visible.

### 2. Main-Screen "What Is Happening Now"

Goal: the main screen always answers the current work state, so the user does
not have to guess whether the model, tool, or child agent is still running.

Deliverables:

- Derive the operation center state from the `AgentTask` view.
- Within 200ms after submit, show `thinking` or `streaming`.
- During tool calls show `running_tool <tool>`, during edits show
  `editing <file>`, during tests show `testing <command>`, and during approval
  blocks show `waiting approval`.
- While external lanes are running, show `supervising <n> agents` and surface
  the most important blocker or next action.
- Every status must include an evidence source such as provider request, tool
  call id, approval id, lane id, test artifact, or Codex thread/turn id.

Acceptance checks:

- Streaming provider responses, running tools, approval blocks, and background
  lanes render as distinct states.
- When nothing is running, the UI shows `Idle` or the latest completion summary,
  not fake progress.

### 3. Programming Feedback Loop

Goal: make "edit -> approve -> diff -> test -> fix -> confirm" smoother inside
the TUI.

Deliverables:

- `/test` results appear in main and side-2 evidence, including command,
  status, duration, failure summary, output tail, and related files.
- Edit/tool summaries aggregate files, line deltas, approval state, write
  result, and diff/review entry points.
- Approval overlays keep default focus on approve, but clear immediately after
  approval or denial.
- Composer remains visible: taller input well, blinking cursor, correct CJK IME
  placement, and correct redraw after resize.
- Fix right-rail, border, and multilingual rendering drift so long sessions do
  not push panels out of alignment.

Acceptance checks:

- A demo can complete "create file -> approve -> run test -> inspect result"
  without leaving the TUI.
- Every UI iteration keeps main, approval, side-1, and side-2 screenshots or
  text snapshots.

### 4. Agent Lane Operator Loop

Goal: external coding agents become observable, steerable collaborators rather
than terminal processes.

Deliverables:

- Codex, Claude Code, DeepSeek, shell, tmux, and PTY lanes all map into
  `AgentTask`.
- `/lane inspect` shows objective, transport, workspace, latest output, changed
  files, test evidence, next action, and decision history.
- `/lane send`, `/lane accept`, `/lane revise`, `/lane discard`, and
  `/lane apply` form a clear operator loop.
- Side-1 prioritizes real lane evidence, latest output, and next actions.

Acceptance checks:

- A tmux/PTY coding-agent lane can be started, observed, followed up, accepted
  or discarded, and leave audit evidence.

## P1: Should Ship

### 5. Codex Adapter Deepening

- Keep using the Claude Code Codex plugin as the reference for setup/doctor,
  review, adversarial review, task/rescue, status, result, cancel, and
  resume/follow-up.
- Keep the app-server task path opt-in until live smoke proves it is safe as a
  default protocol path.
- Continue mapping Codex app-server thread/turn/event evidence into
  `AgentTask`, lane evidence, and side-screen rows.

### 6. Plugin / Skill / MCP / Tool / Agent Foundation

- Define one extension descriptor: `id`, `kind`, `source`, `capabilities`,
  `permissions`, `health`, and `entrypoints`.
- `/extensions doctor`, `/mcp doctor`, and `/skills list` produce actionable
  diagnostics.
- MCP and extension invocation must enter the shared permission/runtime/
  transcript path and must not bypass approval or audit.

### 7. ACP Adapter Spike

- Keep Zed ACP as the long-term protocol reference and preserve process
  transport, handshake, JSONL event log, and event-to-AgentTask mapping.
- `0.1.8` does not need a full ACP editing loop, but it should define how text,
  edit, tool, permission, and completion events map into lane artifacts.

## Non-Goals

- No cloud agent registry.
- No marketplace.
- Do not turn RoboCode into a full IDE.
- Do not let plugins, skills, MCP, or ACP bypass permissions, transcript, and
  approval.
- Do not make automatic task splitting the default; planner exploration should
  require user confirmation.

## User-Facing Success Criteria

- After submitting a task, the user no longer has to guess whether the model or
  remote agent is working.
- The main screen shows the current action; side-1 shows child agents; side-2
  shows tests, LSP, MCP, extensions, and evidence.
- Codex, Claude Code, DeepSeek, shell jobs, and future ACP agents share one
  `AgentTask` / status / approval / evidence language.
- A real small-feature workflow can finish inside the TUI: enter the request,
  approve edits, run tests, inspect results, and accept or revise lane output.

## Suggested Build Order

1. `AgentTask` runtime view and reducer.
2. Main operation center consuming `AgentTask`.
3. Programming-loop evidence: edit/test/diff/approval.
4. Side-1 lane operator loop and side-2 ops evidence.
5. Codex adapter protocol-path hardening.
6. Extension descriptor / doctor.
7. ACP event-mapping spike.

## Verification Gate

- `cargo fmt --check`
- focused tests for touched crates
- `cargo test --workspace --quiet`
- TUI preview or screenshots: idle, thinking, tool call, approval, test result,
  side-1, side-2
- `scripts/smoke-codex-app-server.sh` when local Codex auth and rate limits are
  available
- `scripts/smoke-codex-app-server-protocol-fixture.sh` for deterministic
  command/file/approval/error protocol-event ingestion coverage
- `scripts/smoke-codex-app-server-write-guard.sh` for the default safety guard
  around experimental write-capable app-server turns
- `scripts/smoke-lane-operator-loop.sh` for focused lane inspect/send/
  accept/apply/conflict/cleanup/archive coverage
- at least one provider smoke covering:
  - prompt submit -> `AgentTask` -> operation center
  - file edit approval
  - `/test`
  - Codex status/result or mock app-server evidence
  - side-1 lane evidence
  - side-2 ops evidence
