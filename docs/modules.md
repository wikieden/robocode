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
- V2-D structured terminal view slices are implemented: grouped diagnostics, grouped symbols, compact references, structured sessions/tasks/memory, structured permission denials, structured `/git diff` and `/diff`, and shared `robocode-core` presentation helpers.
- Provider-plugin runtime and DeepSeek V4 are implemented on main. Mainline uses official DeepSeek model names: `deepseek-v4-flash` by default and `deepseek-v4-pro` when selected explicitly.
- The provider descriptor matrix includes additional OpenAI-compatible gateway providers: `openrouter`, `groq`, `mistral`, `together`, `kimi`, `qwen`, `zhipu`, and `volcengine`.

Current published release:

- `docs/release-0.1.19-status.md` records the tagged, published,
  Homebrew-updated, and post-publish-verified Delegated Lane Usefulness release.
- `0.1.19` separates provider configuration from model selection: `/provider`
  inspects supplier credentials/endpoints/model candidates, while `/models`
  selects provider-grouped models.
- release validation now includes daily-loop and lane operator-loop smoke as
  normal gates.

Next planned slice:

- target the `0.1.20` usability beta gate: clean install, first-use setup,
  provider/model recovery, daily coding loop, and one delegated review loop that
  can be trusted in real work.
- keep every user-visible feature point backed by a real-use screenshot or
  deterministic visual artifact for product review.
- keep `0.1.20` as the usability beta gate for clean install, daily coding
  loop, and delegated review loop evidence.

## Gap vs `.ref/claude-code-main`

Covered: session engine shape, command families, permission modes, local tool registry, transcript/resume model, Git and web workflows.

Partial: task workflow depth, LSP runtime depth, richer interactive TUI behavior, provider streaming/cancellation maturity, dynamic provider loading, broader plugin hardening, DeepSeek Anthropic-compatible execution hardening, and long-session summarization.

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
- `docs/provider-live-matrix.md`

See `PLAN.md`, `docs/product-requirements.md`, `docs/staged-roadmap.md`, and `docs/ref-gap-matrix.md` for full roadmap context.
