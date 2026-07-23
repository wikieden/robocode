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
- Lane effect adapters for Git worktrees, local process groups, typed tmux/PTY
  terminal backends, and checked patch application.

## Invariants

- Mutating tools must be marked mutating in `ToolSpec`.
- Outputs must become serializable `ToolResult` values.
- Shell stays platform-aware: POSIX on Unix, PowerShell on Windows.
- Local lane processes never leave stdout or stderr in unread pipes: callers
  choose a durable combined log, otherwise output is explicitly discarded.
- `TerminalBackend` keeps typed tmux and PTY launch/input/stop semantics apart
  from plain `ProcessBackend` child-process effects.
- Patch adapters prepare every create, write, and delete before touching the
  filesystem. Standard `/dev/null` new-file and deleted-file diffs therefore
  participate in the same runtime rollback transaction.
- Git worktree tools delegate to the same lane worktree adapter used by Core
  lane orchestration.

## Reference Alignment

Reflects `.ref` `Tool.ts` and tool registry behavior using Rust traits and local adapters.

## Test

```bash
cargo test -p viden-tools
```
