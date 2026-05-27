# RoboCode 0.1.8 Status

Chinese version: [release-0.1.8-status.zh-CN.md](release-0.1.8-status.zh-CN.md)

Last updated: 2026-05-27

## Current Phase

`0.1.8` is published. The version target is documented in
[release-0.1.8-plan.md](release-0.1.8-plan.md).

The workspace package version has been bumped to `0.1.8`, local packaging
passes, and the GitHub release now contains cross-platform artifacts.

This checkpoint focuses on the first P0 slice: the unified `AgentTask` runtime
view, plus making the main operation center, right-rail `ACTIVE TASKS` panel,
and side-2 `RECENT EVIDENCE` consume the same status model.

## Completed

- Added the `AgentTask` runtime-view fields that cover the core plan concepts:
  `id`, `parent_id`, `agent`, `kind`, `transport`, `status`, `activity`,
  `summary`, `progress`, `started_at`, `updated_at`, `workspace`, `evidence`,
  `permissions`, `decision`, `result`, `resume_handle`, and `pid`.
- `AgentTask` is now projected from multiple sources of truth:
  - primary transcript reply state;
  - pending approval;
  - tool call / tool result;
  - `/test` command evidence;
  - terminal lanes;
  - Codex job records.
- Lane and Codex job states now map into the 0.1.8 status vocabulary such as
  `thinking`, `editing`, `testing`, `waiting_approval`, `needs_input`,
  `blocked`, `done`, `failed`, `cancelled`, and `archived`.
- The main operation center now prioritizes `AgentTask` and can show states
  such as `waiting approval`, `thinking through latest prompt`, and
  `supervising <n> agent(s)`.
- The main operation-center copy now uses operator-facing language such as
  `DeepSeek is thinking`, `Approval needed: waiting approval for write_file`,
  and `Supervising 2 agents: claude needs input`, while detail rows still keep
  `AgentTask id / agent / status / progress / activity`.
- The operation center now promotes actionable blocker/proof signals from
  `AgentTask.evidence`: failed tests show `Tests failed: <command>` plus a
  `next open failure, patch, rerun tests` action, and lane conflicts show
  `blocked on <conflict/summary>` plus a conflict-resolution next action.
- Historical approval requests no longer keep blocking the main screen or
  approval modal after a later approval resolution, tool result, assistant
  reply, or `/test` command result closes that request.
- `/diff` and `/git diff` command output now project into `AgentTask
  kind=diff`: non-empty diffs become `needs_input` review tasks with
  files/additions/deletions/path evidence, operation-center copy such as
  `review diff: 2 file(s) +12 -3`, and side-2 next action
  `review diff, then test or commit`.
- Transcript projection now keeps the latest diff, test, tool, and provider
  entries as separate `AgentTask` rows instead of collapsing everything into
  only one latest runtime event.
- The right-rail `ACTIVE TASKS` panel now reads active `AgentTask` entries
  instead of separately stitching approval, tool, lane, and Codex job status.
- Side-2 `RECENT EVIDENCE` now reads the `AgentTask` runtime view and renders
  `id / agent / status / progress / activity`, with `evidence`, `decision`,
  `result`, and the next operator action as secondary evidence rows.
- The side-1 preview now shows normalized lane state: `testing`, `needs input`,
  and `done`.
- Added focused tests for projecting transcript approval/tool/test events into
  `AgentTask`.
- Added focused tests for side-2 reusing approval, tool, lane, and Codex job
  `AgentTask` evidence.
- Edit/test/tool-result evidence is now more structured: tool calls extract
  `path` and `lines`, tool results extract write path / line count / changed
  files, and `/test` results extract command, status, duration, and failure
  summary.
- Failed `/test` results now carry a full recovery trail in `AgentTask.evidence`:
  `failure`, `failing-file`, `tail`, and `rerun <command>` rows feed side-2 and
  the main operation-center next action, so the operator can open the failure,
  patch it, and rerun the same command without digging through raw transcript
  output.
- Side-2 failed/blocked tasks prioritize actionable command/failure/path
  evidence so generic `result failed` or `transcript ...` rows do not hide the
  useful signal.
- Lane apply/conflict artifacts now feed `AgentTask.evidence`: RoboCode extracts
  patch path, changed files, and direct-apply conflict summaries from
  `.robocode/lanes/L*.apply.md` and `L*.apply-conflict.md`; blocked lanes expose
  conflict / changed / patch evidence in side-2.
- Codex app-server job artifacts now feed richer `AgentTask` evidence in the
  TUI: result files expose thread, turn, status, approval, and resume handles,
  while JSONL logs expose command-output, file-change, patch, diff, filesystem,
  approval, and error protocol signals.
- Codex app-server turn result files now write a `resume:` handle, so real
  opt-in app-server jobs can surface follow-up context instead of relying on
  hand-written test fixtures.
- Codex app-server JSONL logs now expose final agent-message text as
  `AgentTask` evidence, so side-2 can show the delegate's answer in addition to
  protocol thread/turn ids.
- Codex app-server result files now persist `message:` from the final
  `agentMessage`, so `/agent result`, TUI `AgentTask`, and side-2 evidence read
  the same delegate answer.
- Codex app-server result files now also persist `signals:` for protocol
  evidence classes such as command output, file changes, patches, diff updates,
  filesystem changes, MCP tool calls, MCP file writes, and app-server errors.
  TUI `AgentTask` evidence reads the same line so protocol fixtures can be
  audited without manually opening JSONL.
- Side-2 `RECENT EVIDENCE` now prioritizes app-server `message ...` evidence
  alongside command evidence, so a completed text-turn smoke shows the delegate
  answer before lower-signal protocol ids.
- TUI preview fixtures now include a completed Codex app-server job, and
  `docs/previews/generated/side-2.txt` visibly shows
  `evidence message ROBOCODE_APP_SERVER_SMOKE_OK` for screenshot review.
- Added `scripts/smoke-codex-app-server.sh`, a repeatable live smoke that
  starts a real Codex app-server text turn in a temporary workspace and checks
  `thread`, `turn`, `resume`, tracked-job `finished`, and final-message
  evidence.
- Added `scripts/smoke-codex-app-server-protocol-fixture.sh`, a deterministic
  mock app-server smoke that drives the normal CLI/probe/result path and covers
  command-output, file-change, file-patch, diff, filesystem-change, approval
  request/denial, MCP tool-call / MCP file-write, and error signals.
- Added a guarded `/agent probe codex --turn-write <task>` protocol path for
  disposable-workspace experiments. It is disabled by default because a live
  safety trial showed Codex app-server workspace-write turns can mutate files
  before RoboCode receives an approval request.
- Added `scripts/smoke-codex-app-server-write-guard.sh` to prove the default
  guard blocks app-server write probes before launch and leaves the workspace
  untouched.
- Added `scripts/smoke-lane-operator-loop.sh`, a focused operator-loop smoke
  for shell lanes, `/lane inspect`, decision evidence, embedded PTY send,
  tmux attach evidence, accept/apply, conflict review/resolve, discard/cleanup,
  and archive.
- Refreshed TUI preview text, ANSI, and SVG artifacts under
  `docs/previews/generated/`.

## Verification

Passed:

```bash
cargo fmt
cargo fmt --check
git diff --check
cargo test -p robocode-cli --quiet
cargo test -p robocode-core --quiet
cargo test --workspace --quiet
scripts/tui-previews.sh docs/previews/generated
scripts/smoke-codex-app-server.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-codex-app-server-write-guard.sh
scripts/smoke-lane-operator-loop.sh
scripts/release-smoke.sh --quick --skip-package
scripts/release-smoke.sh --quick --skip-package --deepseek --out-dir /tmp/robocode-018-release-smoke-deepseek-latest
scripts/release-smoke.sh --version 0.1.8 --deepseek --out-dir /tmp/robocode-018-release-smoke-full
gh workflow run release.yml --repo wikieden/robocode -f tag=v0.1.8 -f upload_to_release=true
```

Result:

- `robocode-cli` tests: 197 passed, binary tests 2 passed / 2 ignored.
- `robocode-core` tests: 93 passed.
- workspace tests: passed.
- TUI previews: `scripts/tui-previews.sh docs/previews/generated` generated.
  `main.txt` now shows the operation-center next action for an active test
  lane without a stale approval modal, and `side-2.txt` includes
  `codex-app codex done` plus the app-server final message evidence row.
- live Codex app-server text-turn smoke: passed with `codex-cli 0.133.0`
  through `Codex Desktop/0.133.0`, producing a completed thread/turn, tracked
  job, resume handle, result `message: ROBOCODE_APP_SERVER_SMOKE_OK`, and
  final-message evidence.
- mock Codex app-server protocol-fixture smoke: passed. It produced
  `signals: command-output, file-change, file-patch, diff-updated, fs-changed,
  mcp-tool-call, mcp-tool-completed, mcp-fs-write, app-server-error` plus a
  declined command approval request through the same `/agent probe` -> tracked
  job -> `/agent result` surfaces.
- live Codex app-server read-only command trial: completed, but Codex Desktop
  reported no shell tool was available in that app-server session, so no live
  command approval signal was emitted.
- live Codex app-server write trial in a disposable workspace: completed and
  created `live-write.txt` through an `mcpToolCall` without an approval request.
  RoboCode now classifies that event as `mcp-tool-call`,
  `mcp-tool-completed`, and `mcp-fs-write`, and the write-capable probe remains
  disabled by default.
- Codex app-server write-guard smoke: passed. `/agent probe codex --turn-write`
  is blocked by default unless
  `ROBOCODE_EXPERIMENTAL_CODEX_APP_SERVER_WRITE=1` is set in a disposable
  workspace.
- lane operator-loop smoke: passed. It exercises the local runtime/operator
  path from shell lanes through inspect, PTY send, tmux evidence, accept/apply,
  conflict review/resolve, discard/cleanup, and archive.
- release smoke quick matrix: passed. It covered formatting, terminal tests,
  TUI previews, fallback CLI smoke, protocol fixture, write guard, and lane
  operator-loop smoke.
- DeepSeek release smoke matrix: passed at
  `/tmp/robocode-018-release-smoke-deepseek-latest`; `deepseek-v4-flash` returned
  `robocode-deepseek-smoke-ok`.
- full 0.1.8 release smoke matrix: passed at
  `/tmp/robocode-018-release-smoke-full`. It covered `robocode-cli` tests,
  workspace tests, previews, fallback CLI, app-server protocol fixture,
  write-guard, lane operator loop, package archive smoke, and DeepSeek smoke.
- package smoke produced and verified
  `dist/robocode-v0.1.8-aarch64-apple-darwin.tar.gz`; extracted binary reports
  `robocode-cli 0.1.8`.
- GitHub release workflow
  [26494175931](https://github.com/wikieden/robocode/actions/runs/26494175931)
  passed and uploaded all four target archives plus SHA-256 files.

## Published Release

`v0.1.8` is published at:

- https://github.com/wikieden/robocode/releases/tag/v0.1.8

Release assets:

- `robocode-v0.1.8-aarch64-apple-darwin.tar.gz`
- `robocode-v0.1.8-aarch64-apple-darwin.tar.gz.sha256`
- `robocode-v0.1.8-x86_64-apple-darwin.tar.gz`
- `robocode-v0.1.8-x86_64-apple-darwin.tar.gz.sha256`
- `robocode-v0.1.8-x86_64-pc-windows-msvc.tar.gz`
- `robocode-v0.1.8-x86_64-pc-windows-msvc.tar.gz.sha256`
- `robocode-v0.1.8-x86_64-unknown-linux-gnu.tar.gz`
- `robocode-v0.1.8-x86_64-unknown-linux-gnu.tar.gz.sha256`

## Remaining P0

None for the `0.1.8` release.

## Follow-Up Risks

- Main operation-center summaries still need more real-runtime sample
  validation, especially for long tool output and multi-step review sessions.
- `AgentTask` is not persisted as a standalone artifact yet; it is currently a
  runtime projection.
- Side-2 `TESTS / LSP`, `MCP / CONTEXT`, and `EXTENSIONS` still keep their
  source-specific views; the next step is connecting more real diff/review
  entry points to the main programming loop.
- The programming loop now has first-class diff review visibility and a
  structured failed-test recovery trail; it still needs more real-runtime
  validation across long review sessions.
- The lane operator loop now has deterministic focused smoke coverage, but a
  full human TUI pass with DeepSeek/Codex/Claude/tmux side screens and captured
  screenshots is still needed before release sign-off.
- The Codex app-server path remains opt-in. Live text-turn and disposable
  write-turn probes now pass, and deterministic protocol-fixture coverage
  exercises command/file/approval/MCP/error evidence. The live write probe
  confirmed workspace-write turns can mutate through MCP tool calls without a
  RoboCode approval request, so write-capable app-server probes stay disabled by
  default and must remain disposable-workspace-only.

## Next Steps

1. Keep app-server execution opt-in and read-only by default. Do not promote
   write-capable app-server turns until MCP/file mutations can be mediated
   before mutation.
2. Keep aligning side-1, side-2, and right-rail state wording and color
   priority.
3. Run a real DeepSeek/Codex/Claude/tmux TUI pass and capture screenshots.
   The deterministic lane operator-loop smoke now covers the command path; the
   remaining work is visual/runtime sign-off across real side screens.
4. Use the 0.1.8 follow-up risks as input to the next release plan.
