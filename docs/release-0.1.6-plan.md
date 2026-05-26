# RoboCode 0.1.6 Plan

Last updated: 2026-05-26

## Goal

`0.1.6` should move RoboCode from a terminal-first coding assistant toward a
multi-agent orchestration cockpit. The release should improve the live coding
experience, make background work observable in the main screen, and lay the
architecture for ACP, plugins, skills, and MCP without creating parallel
runtimes.

This plan follows three product findings from the 0.1.5 trial:

1. After the user submits input, the main TUI must make remote/model/lane work
   visible immediately.
2. Plugins, skills, and MCP need a coherent system design instead of isolated
   feature additions.
3. Tmux-launched tools are useful, but RoboCode should evolve toward an
   ACP-compatible multi-agent adapter model inspired by Zed.

## Development Target

The next version target is **make RoboCode feel alive and operator-grade during
real coding work**. The user should be able to submit a task, see what the main
agent is doing, see what side agents are doing, and decide the next action
without leaving the cockpit.

Version theme:

```text
0.1.6 = Live Coding Cockpit + Agent Extension Foundation
```

### P0: Must Ship

- Main-screen live activity:
  - show `Thinking...` immediately after a prompt is submitted;
  - show compact edit/tool status such as `Editing render.rs`;
  - show approval waiting state without hiding the rest of the session;
  - show active lane count and top lane progress in the main screen.
- Evidence-backed status:
  - every visible runtime status must come from transcript events, provider
    telemetry, pending approvals, lane artifacts, or workspace snapshots;
  - no placeholder metrics in normal runtime screens.
- Agent lane baseline:
  - keep template, tmux, and PTY lanes working;
  - expose transport and status in one common lane shape;
  - make `/lane inspect` the reliable debug surface for external agent work.
- Product/design documentation:
  - document the adapter model;
  - document plugin, skill, MCP, and ACP boundaries;
  - keep English and Chinese docs aligned.

### P1: Should Ship

- Agent registry:
  - `/agent list` for built-in and configured agents. **Status: initial
    read-only built-in registry shipped.**
  - `/agent doctor [id]` for binary, environment, template, tmux, and PTY
    readiness checks. **Status: initial local binary/template diagnostics
    shipped.**
- Extension surface:
  - `/extensions list` and `/extensions doctor` as read-only visibility first.
    **Status: initial extension visibility shipped.**
  - `/mcp list` and `/mcp doctor` as read-only visibility first. **Status:
    initial config-file visibility shipped.**
  - `/skills list` for local workflow/task recipes. **Status: initial local
    skill listing shipped with capped output and `--all`.**
- Side-screen improvement:
  - side-1 prioritizes agent lanes, transport, state, latest output, and next
    action. **Status: transport/state rows shipped for side-1 lanes.**
  - side-2 prioritizes tests, LSP, MCP/context, plugin health, and evidence.
    **Status: initial real ops panels shipped for tests/LSP, MCP/context,
    extensions, and recent evidence.**

### P2: Spike, Not Release-Critical

- ACP proof of concept:
  - launch one local ACP-compatible process;
  - complete a minimal handshake;
  - record ACP events into a JSONL debug log;
  - prove how ACP edit/tool/permission events map into RoboCode lanes.
- ACP adapter visibility:
  - `/agent list` and `/agent doctor acp` expose the experimental ACP adapter
    and its `ROBOCODE_AGENT_ACP_COMMAND` setup state. **Status: readiness
    visibility shipped; handshake remains a follow-up spike.**
- `/lane acp <agent> <task>` can remain experimental until the event model is
  clear.

### User-Facing Success Criteria

- After pressing Enter, the user never has to guess whether RoboCode is
  thinking, editing, waiting for approval, or supervising another agent.
- The main screen and side screens use the same language for agent state:
  `thinking`, `editing`, `testing`, `waiting approval`, `needs input`,
  `blocked`, `done`.
- Codex, Claude Code, DeepSeek lanes, shell jobs, and future ACP agents all look
  like peers in the cockpit instead of separate one-off integrations.
- Debugging an external agent run has a clear path: status row -> lane detail ->
  log/artifact/event replay.
- The release remains local-first: no cloud orchestration, account system, or
  remote registry is required for the core experience.

## Reference Signals

- Claude Code terminal UX uses a compact running row such as "Moseying...",
  elapsed time, token movement, and a contextual tip.
- Codex desktop UX makes the current operation legible with rows such as
  `Editing render.rs +129 -4` and `Thinking`.
- Zed's Agent Client Protocol direction is important because ACP standardizes
  communication between editors and coding agents, allowing clients to support
  many agents through one protocol boundary.
- Zed external agents run as separate processes over ACP. Zed forwards a small
  set of editor-owned settings such as model, mode, environment, MCP context
  servers, and project root, while external agents keep their own native
  configuration.
- Zed agent-server packaging shows the distribution shape RoboCode should learn
  from: per-platform targets, command/args, environment, archives, and SHA-256
  hashes.

Reference docs: [Zed ACP](https://zed.dev/acp),
[Zed external agents](https://zed.dev/docs/ai/external-agents.html), and
[Zed agent-server extensions](https://zed.dev/docs/extensions/agent-servers).

## Product Principles

- The main screen should always answer: "what is RoboCode doing right now?"
- External agents are collaborators behind adapters, not trusted authorities.
- ACP, tmux, PTY, CLI template lanes, plugins, skills, and MCP should all feed
  the same lane/status/approval/evidence model.
- User-visible panels must stay evidence-backed. Unknown runtime state should
  render as idle, unavailable, or setup required.
- RoboCode should be a local multi-agent cockpit first, not a cloud task runner
  or full editor replacement.

## Workstreams

### 1. Live Activity Strip

Goal: remove uncertainty after a prompt is submitted.

Deliverables:

- A fixed `LIVE ACTIVITY` strip in the main transcript area.
- Request state:
  - `Thinking...` while the latest submitted prompt is being processed.
  - provider/model detail while the call is in flight.
  - last assistant/tool result summary after completion.
- Tool state:
  - compact rows such as `Editing src/render.rs` for file mutation calls.
  - approval required state when a pending approval modal exists.
- Lane state:
  - number of active lanes and top lane status/progress/summary.
  - no invented data; all rows come from current TUI state, provider telemetry,
    pending approvals, transcript events, or lane store artifacts.

Acceptance checks:

- After pressing enter, the screen immediately shows `Thinking...`.
- Active tmux/PTY/background lanes are visible in the main screen without
  opening side screens.
- The strip remains visible in wide and compact layouts.
- TUI preview generation includes the strip.

### 2. Agent Adapter Model

Goal: make external coding agents a first-class lane backend.

Adapter families:

- `template`: current `ROBOCODE_LANE_<TOOL>_TEMPLATE` flow.
- `tmux`: current operator-controlled terminal session flow.
- `pty`: current embedded PTY bridge flow.
- `acp`: future JSON-RPC/ACP bridge for agents that speak Agent Client Protocol.

Common adapter contract:

```text
AgentAdapter
  id
  display_name
  transport: template | tmux | pty | acp
  launch
  send_task
  send_followup
  poll_status
  read_events
  stop
  capability_descriptor
```

The lane model should not care whether an agent is Codex through tmux, Claude
through a template, Gemini through ACP, or Kiro through ACP. It should record:

- objective
- workspace/worktree
- tool/agent id
- launch transport
- model/mode if available
- log/event paths
- permission requests
- edits/diffs
- tests/evidence
- next action

### 3. ACP Bridge Planning

Goal: add ACP support without breaking existing lane workflows.

Phases:

1. `robocode-acp` spike:
   - add a crate or module boundary for ACP message types and process transport;
   - use the official Rust ACP library if it fits, otherwise keep a minimal
     JSON-RPC transport wrapper until the protocol surface is clear.
2. `/agent list` and `/agent doctor`:
   - show configured template/tmux/pty/acp agents;
   - validate binary path, launch args, environment, and protocol handshake.
3. `/lane acp <agent> <task>`:
   - launch an ACP server process;
   - create a lane;
   - send the task as an ACP session/request;
   - record streamed text, tool/edit events, and permission prompts as lane
     artifacts.
4. Debug visibility:
   - write `.robocode/agents/<lane-id>.acp.jsonl`;
   - add `/agent logs <id>` or `/lane inspect <id>` ACP event replay.

Do not implement registry installation before a local custom-agent flow works.

### 4. Plugin, Skill, MCP System Shape

Goal: one extension model, multiple extension kinds.

Proposed hierarchy:

```text
robocode extensions
  providers: model provider plugins (already started)
  agents: template/tmux/pty/acp agent adapters
  tools: local tool plugins and MCP-backed tools
  skills: prompt/workflow/task templates
  context: MCP servers, repo context providers, docs/index providers
```

Rules:

- Provider plugins remain under `robocode-model` until the provider registry is
  hardened.
- Agent adapters should live near lane orchestration first, then move to
  `robocode-agents` or `robocode-workflows` when stable.
- MCP tools must enter through the existing permission/tool/transcript path.
  Do not create a separate MCP mutation runtime.
- Skills are not tools. They are reusable task envelopes, prompts, and workflow
  recipes that can create lanes, configure context, or guide the main agent.
- Every extension kind needs:
  - manifest;
  - discovery;
  - doctor output;
  - capability descriptor;
  - permission boundary;
  - debug logs.

### 5. Multi-Agent Cockpit UX

Goal: make RoboCode feel like an operator console for several agents.

Main screen:

- current live activity;
- pending approval;
- top active lane;
- recent edit/test evidence.

Side-1:

- agent lanes;
- agent transport (`tmux`, `pty`, `acp`, `template`);
- state (`thinking`, `editing`, `waiting approval`, `needs input`, `testing`);
- next action.

Side-2:

- tests/build/LSP;
- MCP/context status;
- plugin/agent health;
- recent evidence.

Commands:

- `/agent list`
- `/agent doctor [id]`
- `/agent logs <id>`
- `/lane acp <agent> <task>`
- `/lane followup <id> <message>`
- `/extensions list`
- `/extensions doctor`
- `/mcp list`
- `/mcp doctor`
- `/skills list`

## Implementation Order

1. Land `LIVE ACTIVITY` in the main TUI.
2. Document the adapter/extension architecture in English and Chinese.
3. Add agent registry data types and `/agent list` with built-in template/tmux
   agents. **Initial read-only version shipped.**
4. Add `/agent doctor` for Codex, Claude, custom templates, tmux, and PTY.
   **Initial read-only version shipped.**
5. Spike `robocode-acp` against one local ACP-compatible agent. **Readiness
   visibility shipped; handshake remains follow-up.**
6. Add `/lane acp <agent> <task>` as an experimental command.
7. Route ACP events into lane artifacts and `LIVE ACTIVITY`.
8. Expand side screens to show transport and agent state.

## Non-Goals

- Do not replace the existing tmux/PTY/template lane support.
- Do not expose MCP tools outside the existing permission system.
- Do not build a full editor UI.
- Do not build cloud delegation before local orchestration works.
- Do not support plugin registry installation before local manifest discovery
  and doctor checks are stable.

## Ready Criteria

- Main TUI makes active thinking/editing/lane work visible without ambiguity.
- At least one existing external-tool lane path reports transport and status in
  the same shape as future ACP lanes.
- Agent/plugin/skill/MCP architecture is documented and has command stubs or
  planned surfaces.
- ACP spike proves whether RoboCode can launch and exchange messages with one
  ACP-compatible agent process.
- Existing 0.1.5 release smoke still passes.
