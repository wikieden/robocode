# RoboCode Engineering Plan

## Current State

Mainline landed status:

- lightweight REPL and slash-command runtime
- layered config resolution
- multi-provider model abstraction
- native tool-calling support for Anthropic and OpenAI-style providers
- provider-plugin runtime and DeepSeek v4 support, including `deepseek`, `deepseek-anthropic`, expanded OpenAI-compatible gateway descriptors, provider-scoped config, and official `deepseek-v4-flash` / `deepseek-v4-pro` model names
- permission-aware local tool runtime
- JSONL transcripts plus rebuildable SQLite session index
- session listing and resume selectors
- file, search, shell, web, and Git tool families
- V2-A runtime/session commands: `/status`, `/config`, `/doctor`, richer `/sessions`, grouped `/help`
- V2-C workflow continuity: `robocode-workflows`, project tasks, project/session memory, `/tasks`, `/task ...`, `/memory ...`, `/task resume-context`, workflow JSONL logs
- V2-B LSP foundation: `robocode-lsp`, `lsp_*` tools, `/lsp ...` commands, real semantic queries, session reuse, and document synchronization
- V2-D structured view slices: grouped diagnostics, grouped symbols, compact references, structured sessions/tasks/memory, structured permission denials, structured `/git diff` and `/diff`, and shared `robocode-core` presentation helpers
- TUI cockpit and terminal-lane runtime: main cockpit layout, theme variants,
  companion screen registry, `/lane run` lifecycle, external Codex/Claude and
  generic lane adapters, per-lane worktree isolation, explicit accept/apply
  decisions, external terminal/tmux/PTY attach, `/lane send`, and lane log-tail
  replay while reusing the shared `SessionEngine`
- provider runtime hardening checkpoints: descriptor validation, registry refresh coverage, blank-key handling, provider-scoped diagnostics, and offline/live smoke harnesses
- DeepSeek V4 compatibility flags: reasoning-content replay, non-null assistant tool-call content, explicit `tool_choice` capability, and `high`/`max` reasoning-effort metadata

Next planned slice (`0.1.12`):

- make the `0.1.11` TUI/AgentTask foundation into a real Agent Orchestration
  Operator Loop, as specified in `docs/release-0.1.12-plan.md`
- promote provider turns, tool calls, shell/test runs, external lanes, and
  approvals into one runtime `AgentTask` fact layer
- make `NOW WORKING`, side-1, side-2, lane detail, and recent evidence agree on
  the same active/background task state
- ship a minimal dispatch -> observe -> review -> apply loop for at least one
  stable external-agent/lane path
- move `ContextBundle` and token-efficiency work from design into v0
  implementation: context sources, long-output compaction, token estimates, and
  per-lane budget fields
- keep plugin, skill, MCP, and ACP work at the adapter/descriptor/doctor
  foundation level until the shared permission, transcript, evidence, and token
  boundaries are solid

## Near-Term Plan

1. TUI Cockpit and Terminal Lanes.
   - Phase 1-7 of the current TUI/lane plan are landed on `main`: cockpit
     layout, theme variants, companion screens, lane runtime, external-tool
     adapters, isolation, and attachable terminal panes.
   - Continue with the `0.1.12` operator-loop goals: live `NOW WORKING`, shared
     `AgentTask` state, side-screen controls, tmux/PTY log capture, `/lane send`,
     and review/apply loops.
   - Keep lane completion separate from acceptance, and keep apply/cleanup as
     explicit operator actions.

2. Programming Evidence and Terminal Hardening.
   - Promote test output, edit summaries, approval state, and changed-file
     evidence into the main screen and side-2.
   - Improve apply-conflict recovery beyond the current audited retry path.
   - Evaluate whether full cursor-addressed terminal replay is worth the added
     parser/rendering complexity after log-tail replay has covered the first
     cockpit observation need.

3. Codex Adapter as the Reference Agent Backend.
   - Implement Codex setup/doctor, review, adversarial review, task/rescue,
     status, result, cancel, and resume/follow-up flows as the first native
     external-agent adapter.
   - Prefer Codex app-server / protocol events over terminal scraping where
     available; persist thread IDs, touched files, command executions, final
     output, and reasoning/evidence summaries into RoboCode lane evidence.
   - Use read-only as the default review posture; require explicit permission
     boundaries for write-capable Codex work.
   - In `0.1.12`, keep this scoped to one stable operator-loop path rather than
     trying to complete every Codex workflow at once.

4. External Coding Tool Adapter Expansion.
   - Keep Codex, Claude Code, DeepSeek, shell, tmux, PTY, and future ACP agents
     behind one lane lifecycle and one evidence model.
   - Add more templates and docs for tools such as Gemini, Junie, Kiro, and
     local coding CLIs as real operator workflows demand them.
   - Promote durable lane primitives out of `robocode-cli` only when non-TUI
     surfaces need the same model.

5. Extension Foundation.
   - Define plugin, skill, MCP, tool, and agent descriptor boundaries.
   - Make extension health visible through doctor commands and side-2 before
     adding complex invocation or marketplace-like installation flows.
   - Keep every extension invocation on the shared permission, runtime, and
     transcript path.

6. Provider Compatibility Completion.
   - Keep the provider live matrix documented and aligned with built-in descriptors.
   - Validate real API compatibility across built-in and descriptor-backed OpenAI-compatible providers when credentials are available.
   - Keep dynamic loading, registry refresh, descriptor compatibility flags, and collision tests covered.
   - Keep provider binding instance-scoped so multiple sessions or agents can use different providers in the same process.
   - Harden OpenAI-style and Anthropic-style protocol compatibility, including DeepSeek reasoning/tool-call turns.

7. V3 Platform Expansion.
   - MCP runtime and plugin loading.
   - Skills/workflow plugin model.
   - Multi-agent coordinator.
   - Bridge, remote, and server mode.
   - Automation only after workflow state is reliable.

## Landed Phase Notes

1. V2-C Memory and Task Workflows.
   - Keep project tasks and memory state separate from session transcripts.
   - Preserve permission and transcript invariants for all workflow commands.
   - Treat the branch as an early workflow layer, not a complete memory/task
     platform.

2. V2-B LSP Foundation.
   - Add semantic code intelligence without replacing file/search tools.
   - Keep LSP actions behind the same permission and transcript guarantees.
   - Keep maturing the runtime from an early implementation toward a stable merge target.
   - Focus next on robustness, output quality, and long-lived session behavior rather than broadening scope.

3. Provider Plugin Runtime + DeepSeek v4.
   - Add a dynamic provider registry and provider host/runtime.
   - Support runtime registry refresh for newly loaded providers.
   - Keep provider binding instance-scoped so multiple agents can use different providers in the same process.
   - Land DeepSeek as the first independent plugin-backed provider using the official OpenAI-style and Anthropic-compatible API surfaces.

## Gap vs `.ref/claude-code-main`

Completed or substantially covered:

- shared session engine pattern
- slash-command command families
- permission modes and approval path
- local file/search/shell tools
- Git and web command families
- transcript and resume model
- provider abstraction
- provider plugin runtime and dynamic registry
- DeepSeek v4 as an independent provider family
- early task/memory workflow layer
- early LSP foundation with real semantic queries and normalized terminal output
- structured terminal views for diagnostics, symbols, references, sessions, tasks, memory, diff, and permission denials

Partial:

- command surface breadth
- LSP runtime execution depth
- provider streaming/cancellation maturity
- dynamic provider loading and plugin hardening
- session summaries and long-history management
- task workflows compared with reference task/session model
- richer interactive TUI behavior beyond structured plain-text sections

Missing:

- MCP
- general skills/plugins beyond provider plugins
- multi-agent/team coordinator
- bridge/remote/server mode
- cron/automation
- voice
- managed settings, analytics, feature flag platform

Deferred intentionally:

- Bun/React/Ink internals
- reference product operations machinery
- remote-first flows before local CLI stability

## Implementation Policy

- Build from small written plans in `docs/superpowers/plans/`.
- Keep every feature on a dedicated `codex/*` branch/worktree.
- Prefer behavior-level compatibility with `.ref`, not direct code translation.
- Keep JSONL canonical and SQLite derived.
- Keep mutations permission-gated.
- Keep transcript entries sufficient for audit and resume.
- Update English and Chinese docs together for user-facing docs.
- Commit checkpoints after focused test passes.

## Source Docs

Primary planning docs:

- `docs/product-requirements.md`
- `docs/staged-roadmap.md`
- `docs/ref-gap-matrix.md`
- `docs/reference-analysis.md`
- `docs/architecture.md`
- `DESIGN.md`
- `docs/code-agent-benchmark.md`
- `docs/tui-lane-architecture-plan.md`
- `docs/tui-lane-architecture-plan.zh-CN.md`
- `docs/superpowers/plans/2026-05-23-tui-cockpit-terminal-lanes.md`
- `docs/superpowers/plans/2026-05-23-tui-cockpit-terminal-lanes.zh-CN.md`
- `docs/superpowers/plans/2026-04-11-robocode-plan-index.md`

Current V2-C docs, when present:

- `docs/superpowers/specs/2026-04-11-v2-memory-task-workflows-design.md`
- `docs/superpowers/plans/2026-04-11-v2-memory-task-workflows.md`

Current V2-B docs, when present:

- `docs/superpowers/plans/2026-04-21-v2-lsp-foundation.md`

Current V2-D docs, when present:

- `docs/superpowers/plans/2026-04-23-v2-d-structured-views.md`
