# Viden Module Index

## Workspace Dependency Map

- `apps/cli` depends on config, runtime, provider, tools, and types to start the product runtime.
- `apps/tui` owns terminal rendering, input orchestration, previews, and app-specific TUI state.
- `crates/core` is the stable facade that re-exports runtime and contract types for TUI, GUI, CLI, and future API surfaces.
- `crates/runtime` depends on LSP, provider, permissions, session, tools, types, and workflows to orchestrate turns and commands.
- `crates/lanes` owns lane lifecycle orchestration below the runtime; it depends only on permissions, tools, types, and workflows, and receives runtime policy through injected seams.
- `crates/agents` owns the external agent adapters below the runtime (ACP generic client, Codex app-server, shared spawn infrastructure); it depends only on permissions, plugin-api, plugin-host, tools, and types, and receives permission contexts, approvers, and event sinks through injected seams.
- `crates/plugin-api` defines shared plugin manifest, capability, permission, and provider descriptor contracts.
- `crates/plugin-host` owns shared plugin discovery/registry boundaries.
- `plugins/providers/deepseek` is the first first-party provider plugin using the plugin API.
- `viden-lsp` depends on types and JSON serialization to provide read-only semantic code intelligence.
- `viden-provider`, `viden-tools`, `viden-permissions`, `viden-session`, and `viden-workflows` use `viden-types` for shared contracts.
- `viden-workflows` also uses `viden-session` for shared project identity.

## Data Ownership Map

- Transcript/session facts: `viden-session`.
- Project workflow state: `viden-workflows`.
- Shared contracts: `viden-types`.
- Permission policy: `viden-permissions`.
- Tool implementation: `viden-tools`.
- Provider host/runtime, protocol adaptation, and dynamic registry: `viden-provider`.
- Plugin manifest and capability contracts: `viden-plugin-api`.
- Plugin registry/lifecycle boundary: `viden-plugin-host`.
- Semantic code intelligence: `viden-lsp`.
- App surfaces: `apps/cli`, `apps/tui`, and future `apps/gui`.

## Current Implementation Status

Mainline landed:

- V1 local CLI baseline is implemented: REPL, config, providers, permissions, transcripts, resume, file/search/shell/web/Git tools.
- V2-A session and command enhancement is implemented: `/status`, `/config`, `/doctor`, richer `/sessions`, grouped `/help`.
- V2-C workflow continuity is implemented: `viden-workflows`, `/tasks`, `/task ...`, `/memory ...`, workflow JSONL logs, and resume context.
- V2-B LSP foundation is implemented: `viden-lsp`, `lsp_*` tools, `/lsp ...` commands, real semantic queries, session reuse, and document sync.
- V2-D structured terminal view slices are implemented: grouped diagnostics, grouped symbols, compact references, structured sessions/tasks/memory, structured permission denials, structured `/git diff` and `/diff`, and shared `viden-runtime` presentation helpers.
- Provider-plugin runtime and DeepSeek V4 are implemented on main. Mainline uses official DeepSeek model names: `deepseek-v4-flash` by default and `deepseek-v4-pro` when selected explicitly.
- The provider descriptor matrix includes additional OpenAI-compatible gateway providers: `openrouter`, `groq`, `mistral`, `together`, `kimi`, `qwen`, `dashscope-coding-plan`, `dashscope-coding-plan-anthropic`, `dashscope-tokenplan`, `dashscope-tokenplan-anthropic`, `zhipu`, and `volcengine`.

Current published release:

- `docs/release-0.1.29-status.md` records the RC TUI stability release:
  RC TUI stability smoke, refreshed deterministic screenshots, live DeepSeek
  development smoke, GitHub Release assets, Homebrew tap, and post-publish
  validation.
- release validation now includes the RC stability smoke, daily-loop,
  lane operator-loop, local package, deterministic TUI screenshots, GitHub
  release assets, and Homebrew checks as normal gates.

Next planned slice:

- the current `0.1.30` slice is the final 0.1.x zero-bug gate: clear known
  P0/P1 TUI bugs, finish real-terminal screenshot evidence, keep all stability
  smokes green, and only then move toward the 0.2.x surface.
- keep every user-visible feature point backed by a real-use screenshot or
  deterministic visual artifact for product review.

## Gap vs `.ref/claude-code-main`

Covered: session engine shape, command families, permission modes, local tool registry, transcript/resume model, Git and web workflows.

Partial: task workflow depth, LSP runtime depth, richer interactive TUI behavior, provider streaming/cancellation maturity, dynamic provider loading, broader plugin hardening, DeepSeek Anthropic-compatible execution hardening, and long-session summarization.

Missing: MCP, general skills/plugins beyond provider plugins, multi-agent/team coordinator, bridge/remote/server mode, automation/cron, voice, managed settings, analytics, feature flags.

## Module Docs

- `apps/cli/README.md`
- `crates/config/README.md`
- `crates/runtime/README.md`
- `crates/lanes/README.md`
- `crates/agents/README.md`
- `crates/lsp/README.md`
- `crates/provider/README.md`
- `crates/tools/README.md`
- `crates/permissions/README.md`
- `crates/session/README.md`
- `crates/types/README.md`
- `crates/workflows/README.md`
- `docs/provider-live-matrix.md`
- `docs/provider-adapter-design.md`
- `docs/product-design-operator-loop.md`
- `docs/production-coding-loop-architecture.md`
- `docs/spec-review-0.1.24.md`

See `PLAN.md`, `docs/product-requirements.md`, `docs/staged-roadmap.md`, and `docs/ref-gap-matrix.md` for full roadmap context.
