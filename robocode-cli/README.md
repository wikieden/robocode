# robocode-cli

## Purpose

`robocode-cli` owns the binary entrypoint and lightweight REPL. It turns CLI flags, config resolution, terminal input, and approval prompts into calls into `robocode-core`.

## Does Not Own

- Session orchestration, provider/tool loops, or workflow state.
- Permission decisions; it only renders approval prompts.
- Transcript, JSONL, or SQLite persistence.

## Public Surface

- CLI startup flags and environment handoff.
- `--provider-plugin-dir <dir>` for repeatable dynamic provider plugin
  discovery paths.
- Structured startup diagnostics for provider plugin loading failures.
- Runtime snapshot construction.
- REPL rendering of `EngineEvent` output.

## Invariants

- Never bypass `robocode-core` for commands or mutating actions.
- Preserve config precedence from `robocode-config`.
- Build the startup `ProviderHost` from resolved config, including explicit
  provider plugin directories.
- Render provider plugin loader errors with kind, path, message, and detail
  rather than collapsing them into an opaque string.
- Keep terminal output usable without a rich TUI.

## Reference Alignment

Behaviorally follows `.ref/claude-code-main/src/main.tsx` for startup and REPL wiring, without copying Bun/React/Ink internals.

## Test

```bash
cargo test -p robocode-cli
```
