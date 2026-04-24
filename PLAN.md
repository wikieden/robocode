# RoboCode Engineering Plan

## Current State

Mainline landed status:

- lightweight REPL and slash-command runtime
- layered config resolution
- multi-provider model abstraction
- native tool-calling support for Anthropic and OpenAI-style providers
- permission-aware local tool runtime
- JSONL transcripts plus rebuildable SQLite session index
- session listing and resume selectors
- file, search, shell, web, and Git tool families
- V2-A runtime/session commands: `/status`, `/config`, `/doctor`, richer `/sessions`, grouped `/help`
- V2-C workflow continuity: `robocode-workflows`, project tasks, project/session memory, `/tasks`, `/task ...`, `/memory ...`, `/task resume-context`, workflow JSONL logs

Current dev baseline on `codex/v2-lsp-foundation`:

- `robocode-lsp`
- LSP server registry and JSON-RPC framing
- semantic result contracts in `robocode-types`
- `lsp_diagnostics`, `lsp_symbols`, and `lsp_references` tools
- `/lsp status`, `/lsp diagnostics`, `/lsp symbols`, and `/lsp references` commands
- real semantic queries through language-server stdio
- initialized session reuse per workspace/server
- document version sync through `didChange`
- reference normalization and more readable terminal output

Implemented on preceding branch:

- `codex/v2-memory-task-workflows` contains the early V2-C workflow continuity slice
- V2-C remains partial relative to the full product target

Next planned slice:

- `codex/v2-d-structured-views` exists as the planning branch for structured terminal views
- V2-D implementation has not started yet

## Near-Term Plan

1. Merge or otherwise land the current V2-C memory/task workflow branch.
   - Keep project tasks and memory state separate from session transcripts.
   - Preserve permission and transcript invariants for all workflow commands.
   - Treat the branch as an early workflow layer, not a complete memory/task
     platform.

2. V2-B LSP Foundation.
   - Add semantic code intelligence without replacing file/search tools.
   - Keep LSP actions behind the same permission and transcript guarantees.
   - Keep maturing the runtime from an early implementation toward a stable merge target.
   - Focus next on robustness, output quality, and long-lived session behavior rather than broadening scope.

3. V2-D Richer TUI and Structured Views.
   - Improve task, memory, diff, session, and approval rendering.
   - Add structured views for diagnostics, symbols, and references.
   - Avoid a full UI rewrite until workflows are stable.
   - Keep text output usable in plain terminals.

4. V3 Platform Expansion.
   - MCP runtime and plugin loading.
   - Skills/workflow plugin model.
   - Multi-agent coordinator.
   - Bridge, remote, and server mode.
   - Automation only after workflow state is reliable.

## Gap vs `.ref/claude-code-main`

Completed or substantially covered:

- shared session engine pattern
- slash-command command families
- permission modes and approval path
- local file/search/shell tools
- Git and web command families
- transcript and resume model
- provider abstraction
- early task/memory workflow layer
- early LSP foundation with real semantic queries and normalized terminal output

Partial:

- command surface breadth
- LSP runtime execution depth
- provider streaming/cancellation maturity
- session summaries and long-history management
- task workflows compared with reference task/session model
- structured terminal UI

Missing:

- MCP
- skills/plugins
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
- `docs/superpowers/plans/2026-04-11-robocode-plan-index.md`

Current V2-C docs, when present:

- `docs/superpowers/specs/2026-04-11-v2-memory-task-workflows-design.md`
- `docs/superpowers/plans/2026-04-11-v2-memory-task-workflows.md`

Current V2-B docs, when present:

- `docs/superpowers/plans/2026-04-21-v2-lsp-foundation.md`

Current V2-D docs, when present:

- `docs/superpowers/plans/2026-04-23-v2-d-structured-views.md`
