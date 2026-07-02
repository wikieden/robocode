# viden-cli

## Purpose

`viden-cli` owns the binary entrypoint and lightweight REPL. It turns CLI flags, config resolution, terminal input, and approval prompts into calls into `viden-runtime`.

## Does Not Own

- Session orchestration, provider/tool loops, or workflow state.
- Permission decisions; it only renders approval prompts.
- Transcript, JSONL, or SQLite persistence.

## Public Surface

- CLI startup flags and environment handoff.
- `--provider-plugin-dir <dir>` for repeatable dynamic provider plugin
  discovery paths.
- Structured startup diagnostics for provider plugin loading failures.
- REPL handoff of provider runtime state so `/provider list` and
  `/provider reload` can inspect and refresh plugin descriptors, and
  `/provider use <id> [model]` can switch through the same registry.
- Runtime snapshot construction.
- REPL rendering of `EngineEvent` output.

## Invariants

- Never bypass `viden-runtime` for commands or mutating actions.
- Preserve config precedence from `viden-config`.
- Build the startup `ProviderHost` from resolved config, including explicit
  provider plugin directories.
- Pass the startup provider host and plugin directories into `viden-runtime`;
  the CLI must not implement provider reload behavior itself.
- Pass provider request defaults into `viden-runtime` so provider switching uses
  the same timeout, retry, API base, and API key defaults as startup.
- Render provider plugin loader errors with kind, path, message, and detail
  rather than collapsing them into an opaque string.
- Keep terminal output usable without a rich TUI.

## Reference Alignment

Behaviorally follows `.ref/claude-code-main/src/main.tsx` for startup and REPL wiring, without copying Bun/React/Ink internals.

## Test

```bash
cargo test -p viden-cli
```
