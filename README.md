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
- A provider abstraction with support for multiple API families and native tool-calling where available

## Workspace

- `robocode-cli`: command-line entrypoint and REPL
- `robocode-config`: config loading and precedence resolution
- `robocode-core`: session engine and orchestration
- `robocode-model`: model provider abstraction and implementations
- `robocode-tools`: built-in tools and execution adapters
- `robocode-permissions`: permission modes and decision logic
- `robocode-session`: transcript storage and resume support
- `robocode-types`: shared domain types

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

Supported provider families:

- `anthropic`
- `openai`
- `openai-compatible`
- `ollama`
- `fallback`

Native tool-calling currently maps:

- Anthropic `tool_use`
- OpenAI and OpenAI-compatible `tool_calls`
- `fallback` and `ollama` text-first local flows

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
```

The `/resume` command also supports `/resume #<index>` and `/resume <session-id-prefix>`.

Built-in tool families include:

- file and search tools: `read_file`, `write_file`, `edit_file`, `glob`, `grep`
- web tools: `web_search`, `web_fetch`
- Git tools: status, diff, branch, add, switch, commit, push, restore, stash, and worktree flows
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

This is an actively developing V1 implementation focused on a reliable local CLI core before broader platform features.
