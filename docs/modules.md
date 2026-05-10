# RoboCode Module Index

## Workspace Dependency Map

- `robocode-cli` depends on config, core, model, tools, and types to create the terminal runtime.
- `robocode-core` depends on LSP, model, permissions, session, tools, types, and workflows to orchestrate turns and commands.
- `robocode-lsp` depends on types and JSON serialization to provide read-only semantic code intelligence.
- `robocode-model`, `robocode-tools`, `robocode-permissions`, `robocode-session`, and `robocode-workflows` use `robocode-types` for shared contracts.
- `robocode-workflows` also uses `robocode-session` for shared project identity.

## Data Ownership Map

- Transcript/session facts: `robocode-session`.
- Project workflow state: `robocode-workflows`.
- Shared contracts: `robocode-types`.
- Permission policy: `robocode-permissions`.
- Tool implementation: `robocode-tools`.
- Provider host/runtime, protocol adaptation, and dynamic registry: `robocode-model`.
- Semantic code intelligence: `robocode-lsp`.
- CLI presentation: `robocode-cli`.

## Current Implementation Status

Mainline landed:

- V1 local CLI baseline is implemented: REPL, config, providers, permissions, transcripts, resume, file/search/shell/web/Git tools.
- V2-A session and command enhancement is implemented: `/status`, `/config`, `/doctor`, richer `/sessions`, grouped `/help`.
- V2-C workflow continuity is implemented: `robocode-workflows`, `/tasks`, `/task ...`, `/memory ...`, workflow JSONL logs, and resume context.
- V2-B LSP foundation is implemented: `robocode-lsp`, `lsp_*` tools, `/lsp ...` commands, real semantic queries, session reuse, and document sync.
- V2-D first structured terminal view slice is implemented: grouped diagnostics, grouped symbols, compact references, and `robocode-core` presentation helpers.
- Provider-plugin runtime and DeepSeek V4 are implemented on main. Mainline uses official DeepSeek model names: `deepseek-v4-flash` by default and `deepseek-v4-pro` when selected explicitly.

Next planned slice:

- harden the provider-plugin runtime slice with real dynamic loading paths, registry refresh coverage, descriptor validation, streaming/cancellation, and multi-agent provider binding tests.
- after that, later V2-D work extends structured views into sessions, tasks, memory, diff, and approvals.

## Gap vs `.ref/claude-code-main`

Covered: session engine shape, command families, permission modes, local tool registry, transcript/resume model, Git and web workflows.

Partial: task workflow depth, LSP runtime depth, terminal UI richness, provider streaming/cancellation maturity, dynamic provider loading, broader plugin hardening, DeepSeek Anthropic-compatible execution hardening, and long-session summarization.

Missing: MCP, general skills/plugins beyond provider plugins, multi-agent/team coordinator, bridge/remote/server mode, automation/cron, voice, managed settings, analytics, feature flags.

## Module Docs

- `robocode-cli/README.md`
- `robocode-config/README.md`
- `robocode-core/README.md`
- `robocode-lsp/README.md`
- `robocode-model/README.md`
- `robocode-tools/README.md`
- `robocode-permissions/README.md`
- `robocode-session/README.md`
- `robocode-types/README.md`
- `robocode-workflows/README.md`

See `PLAN.md`, `docs/product-requirements.md`, `docs/staged-roadmap.md`, and `docs/ref-gap-matrix.md` for full roadmap context.
