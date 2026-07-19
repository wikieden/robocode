# viden-tools

## Purpose

`viden-tools` owns built-in local tools and execution adapters.

## Does Not Own

- Permission decisions.
- Model planning.
- Transcript or workflow state.
- Merge-gate orchestration or workflow persistence.

## Public Surface

- `BuiltinTool`
- `ToolRegistry`
- Built-ins for shell, files, glob, grep, web, and Git.
- Lane effect adapters for Git worktrees, local processes or terminal
  backends, and checked patch application.

## Invariants

- Mutating tools must be marked mutating in `ToolSpec`.
- Outputs must become serializable `ToolResult` values.
- Shell stays platform-aware: POSIX on Unix, PowerShell on Windows.
- Patch adapters prepare all writes before touching the filesystem so runtime
  transactions can roll back safely.
- Git worktree tools delegate to the same lane worktree adapter used by Core
  lane orchestration.

## Reference Alignment

Reflects `.ref` `Tool.ts` and tool registry behavior using Rust traits and local adapters.

## Test

```bash
cargo test -p viden-tools
```
