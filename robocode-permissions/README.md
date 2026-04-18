# robocode-permissions

## Purpose

`robocode-permissions` owns permission modes, path scope checks, and allow/ask/deny decisions.

## Does Not Own

- Prompt rendering.
- Tool execution.
- Transcript or workflow storage.

## Public Surface

- `PermissionEngine`
- `PermissionContext`
- `PermissionDecision`

## Invariants

- `plan` mode denies mutation.
- Safe reads inside scope can auto-allow.
- Out-of-scope paths deny unless explicitly special-cased.
- Workflow writes are treated as mutating actions by core.

## Reference Alignment

Based on `.ref/src/types/permissions.ts`: modes, scoped rules, and approval outcomes.

## Test

```bash
cargo test -p robocode-permissions
```
