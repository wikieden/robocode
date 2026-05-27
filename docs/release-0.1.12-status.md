# RoboCode 0.1.12 Status

Chinese version: [release-0.1.12-status.zh-CN.md](release-0.1.12-status.zh-CN.md)

Last updated: 2026-05-27

## Summary

`0.1.12` is the Agent Orchestration Operator Loop release. The version target is
documented in [release-0.1.12-plan.md](release-0.1.12-plan.md).

Workspace package version has been bumped to `0.1.12`. Local release-candidate
validation passed with 11 checks green, including DeepSeek smoke and the
deterministic lane operator loop smoke.

## Main Changes

- Added shared `AgentTaskRecord`, `AgentTaskStatus`, `AgentTaskEvidence`,
  `AgentNextAction`, `AgentLaneRecord`, and ContextBundle record types in
  `robocode-types`.
- Added `SessionEngine::agent_task_snapshot()` and runtime writes for provider
  turns, tool calls, permission approval waits, tool results, and `/test`
  commands.
- TUI `NOW WORKING`, right rail, side-1, and side-2 now consume the same shared
  `AgentTaskRecord` shape while preserving transcript fallback projections.
- `/lane run` and `/lane ask <tool> <task>` now generate a lane envelope with
  ContextBundle v0 sources, token estimate, pressure, largest sources,
  compaction notes, and budget metadata.
- Added `/lane retry <id>` so the deterministic lane operator loop can recover
  from failed/blocked work without manually reconstructing the task.
- Side-2 ops evidence surfaces context pressure when a lane envelope is present.
- README, user guide, module index, staged roadmap, and screenshots have been
  moved to the `0.1.12` line.

## Screenshot Evidence

Deterministic visual evidence:

- [main cockpit](previews/generated/screenshots/0.1.12-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.12-tui-main-idle.svg)
- [live provider turn](previews/generated/screenshots/0.1.12-tui-live-turn.svg)
- [resized redraw](previews/generated/screenshots/0.1.12-tui-main-resize.svg)
- [CJK input](previews/generated/screenshots/0.1.12-tui-cjk-input.svg)
- [command palette](previews/generated/screenshots/0.1.12-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.12-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.12-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.12-tui-side-2.svg)

Structured screenshot evidence:

```text
docs/previews/generated/tui-regression-evidence.json
```

## Local Verification

Focused implementation checks passed:

```bash
cargo test -p robocode-core -p robocode-cli -p robocode-types --quiet
```

Result:

- `robocode-cli`: 203 passed, 0 failed
- `robocode-core`: 93 passed, 0 failed
- `robocode-types`: 6 passed, 0 failed

Screenshot regression passed:

```bash
scripts/tui-regression.sh docs/previews/generated
```

## Release Candidate Evidence

```bash
scripts/release-smoke.sh --version 0.1.12 --deepseek --out-dir /tmp/robocode-0112-release-smoke-full
```

Result:

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-0112-release-smoke-full/release-evidence.json`

Passing checks:

- `cargo-fmt`
- `cargo-clippy`
- `robocode-cli-tests`
- `workspace-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`
- `package-smoke`
- `deepseek-cli-smoke`

Package smoke generated:

```text
dist/robocode-v0.1.12-aarch64-apple-darwin.tar.gz
```

Post-publish validation target:

```bash
scripts/release-smoke.sh --version 0.1.12 --quick --github-release-assets --homebrew --out-dir /tmp/robocode-0112-postpublish-check
```

Result:

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/robocode-0112-postpublish-check/release-evidence.json`

## Publish State

`v0.1.12` is published:

- GitHub release: https://github.com/wikieden/robocode/releases/tag/v0.1.12
- Release workflow: https://github.com/wikieden/robocode/actions/runs/26518796829
- Release workflow conclusion: `success`
- Release published at: `2026-05-27T14:49:11Z`
- Release assets uploaded at: `2026-05-27T14:51:33Z` - `2026-05-27T14:51:35Z`
- Homebrew tap commit: `3cb201c`

Published assets:

```text
robocode-v0.1.12-aarch64-apple-darwin.tar.gz
robocode-v0.1.12-aarch64-apple-darwin.tar.gz.sha256
robocode-v0.1.12-x86_64-apple-darwin.tar.gz
robocode-v0.1.12-x86_64-apple-darwin.tar.gz.sha256
robocode-v0.1.12-x86_64-pc-windows-msvc.tar.gz
robocode-v0.1.12-x86_64-pc-windows-msvc.tar.gz.sha256
robocode-v0.1.12-x86_64-unknown-linux-gnu.tar.gz
robocode-v0.1.12-x86_64-unknown-linux-gnu.tar.gz.sha256
```

## Remaining Risks

- Codex/Claude adapters reuse the shared AgentTask/lane shape, but their full
  happy path is intentionally not a `0.1.12` release blocker.
- ContextBundle v0 is wired into lane envelopes only; the main provider prompt
  path still records context pressure rather than using the bundle for prompt
  construction.
- MCP, skills, plugins, and ACP stay at descriptor/doctor/probe/capability/event
  mapping depth and do not yet mutate through a generalized plugin runtime.
