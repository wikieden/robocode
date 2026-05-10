# RoboCode

RoboCode is a Rust-first reimplementation of the core local agent CLI patterns from the reference Claude Code project.

Chinese version: [README.zh-CN.md](README.zh-CN.md)

This repository currently includes:

- A multi-crate Rust workspace
- A lightweight REPL CLI
- Layered startup configuration with project and global config support
- Session persistence with JSONL transcripts and a SQLite index
- A permission-aware tool runtime
- Built-in local tools for shell, files, search, web access, and Git workflows including worktrees and stash/restore flows
- Project-level workflow state for tasks, session memory, project memory suggestions, and resume context
- A provider abstraction with support for multiple API families, native tool-calling where available, and a provider-plugin runtime with DeepSeek as the first independent provider family

## Workspace

- `robocode-cli`: command-line entrypoint and REPL
- `robocode-config`: config loading and precedence resolution
- `robocode-core`: session engine and orchestration
- `robocode-model`: provider host/runtime, protocol adapters, and model implementations
- `robocode-tools`: built-in tools and execution adapters
- `robocode-permissions`: permission modes and decision logic
- `robocode-session`: transcript storage and resume support
- `robocode-types`: shared domain types
- `robocode-workflows`: project tasks, memory, resume-context, and workflow event storage

## Development

Run the test suite:

```bash
cargo test --workspace
```

Start the CLI:

```bash
cargo run -p robocode-cli -- --provider fallback --model test-local
```

Start the CLI with an explicit config file:

```bash
cargo run -p robocode-cli -- --config .robocode/config.toml
```

Configuration can come from:

- a global config file
- a project-local `.robocode/config.toml`
- environment variables
- CLI flags

Priority is `CLI > environment > project config > global config > defaults`.

Example config:

```toml
provider = "openai"
model = "gpt-5.2"
permission_mode = "acceptEdits"
request_timeout_secs = 120
max_retries = 2
```

Provider-scoped config can override generic API fields:

```toml
provider = "deepseek"
model = "deepseek-v4-flash"
api_base = "https://generic.example"

[providers.deepseek]
api_base = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
```

DeepSeek V4 model names follow the official API docs:

- `deepseek-v4-flash` is RoboCode's default DeepSeek model
- `deepseek-v4-pro` can be selected explicitly for the higher-capability V4 model
- legacy `deepseek-chat` and `deepseek-reasoner` remain compatibility names only and are scheduled for DeepSeek-side deprecation

DeepSeek precedence for API fields is:

- CLI `--api-key` / `--api-base`
- `[providers.deepseek]` config
- `DEEPSEEK_API_KEY` / `DEEPSEEK_API_BASE`
- generic `api_key` / `api_base`

Supported provider families:

- `anthropic`
- `openai`
- `openai-compatible`
- `deepseek` as an independent provider family using the OpenAI-style protocol
- `deepseek-anthropic` for DeepSeek's Anthropic-compatible endpoint at `https://api.deepseek.com/anthropic`
- `ollama`
- `fallback`

Current protocol families and tool-calling mappings:

- Anthropic `tool_use`
- OpenAI-style `tool_calls` for OpenAI-compatible providers, including DeepSeek
- DeepSeek Anthropic-compatible `tool_use` through `deepseek-anthropic`
- `fallback` and `ollama` text-first local flows

Provider runtime status and direction:

- built-in providers remain supported
- provider descriptors now flow through the provider host/runtime registry
- provider bindings are session/agent scoped rather than process-global
- the next hardening work is real dynamic plugin loading, registry refresh coverage, streaming, and cancellation
- the runtime is designed for native dynamic loading first and WASM migration later

Useful commands:

```text
/help
/provider
/status
/config
/doctor
/permissions
/sessions
/resume latest
/git status
/git worktree list
/git stash list
/web search rust language --limit 3
/web fetch https://www.rust-lang.org --max-bytes 500
/task add Build workflow commands
/tasks
/task resume-context
/memory suggest Keep project memory explicit
/memory confirm mem_<id>
/memory export
```

The `/resume` command also supports `/resume #<index>` and `/resume <session-id-prefix>`.

Built-in tool families include:

- file and search tools: `read_file`, `write_file`, `edit_file`, `glob`, `grep`
- web tools: `web_search`, `web_fetch`
- Git tools: status, diff, branch, add, switch, commit, push, restore, stash, and worktree flows
- workflow commands: project tasks, task lifecycle changes, session memory, project memory suggestions, and resume context
- shell execution with platform-specific adapters for POSIX and PowerShell

Project docs:

- `docs/architecture.md`
- `docs/architecture.zh-CN.md`
- `docs/documentation-localization.md`
- `docs/documentation-localization.zh-CN.md`
- `docs/reference-analysis.md`
- `docs/reference-analysis.zh-CN.md`
- `docs/product-requirements.md`
- `docs/product-requirements.zh-CN.md`
- `docs/staged-roadmap.md`
- `docs/staged-roadmap.zh-CN.md`
- `docs/ref-gap-matrix.md`
- `docs/ref-gap-matrix.zh-CN.md`
- `docs/superpowers/plans/2026-04-11-robocode-plan-index.md`
- `docs/superpowers/plans/2026-04-11-robocode-plan-index.zh-CN.md`
- `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.md`
- `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.zh-CN.md`

## Status

This is an actively developing local-first CLI platform. Mainline already includes V1 plus core V2 session/workflow/LSP slices, the provider-plugin runtime, and DeepSeek v4 as the first independent provider target. The next provider-platform work is hardening dynamic loading, registry refresh, streaming, cancellation, and broader plugin compatibility.
