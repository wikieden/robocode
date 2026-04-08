# RoboCode

RoboCode is a Rust-first reimplementation of the core local agent CLI patterns from the reference Claude Code project.

This repository currently includes:

- A multi-crate Rust workspace
- A lightweight REPL CLI
- Session persistence with JSONL transcripts and a SQLite index
- A permission-aware tool runtime
- Built-in local tools for shell, files, search, and basic Git workflows
- A provider abstraction with support for multiple API families

## Workspace

- `robocode-cli`: command-line entrypoint and REPL
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

## Status

This is an actively developing V1 implementation focused on a reliable local CLI core before broader platform features.
