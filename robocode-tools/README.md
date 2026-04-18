# robocode-tools

## Purpose

`robocode-tools` owns built-in local tools and execution adapters.

## Does Not Own

- Permission decisions.
- Model planning.
- Transcript or workflow state.

## Public Surface

- `BuiltinTool`
- `ToolRegistry`
- Built-ins for shell, files, glob, grep, web, and Git.

## Invariants

- Mutating tools must be marked mutating in `ToolSpec`.
- Outputs must become serializable `ToolResult` values.
- Shell stays platform-aware: POSIX on Unix, PowerShell on Windows.

## Reference Alignment

Reflects `.ref` `Tool.ts` and tool registry behavior using Rust traits and local adapters.

## Test

```bash
cargo test -p robocode-tools
```
