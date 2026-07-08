# Viden 0.1.14 Status

Chinese version: [release-0.1.14-status.zh-CN.md](release-0.1.14-status.zh-CN.md)

Last updated: 2026-05-29

## Status

`0.1.14` is implemented and locally verified as the **Delegated Agent Trust
Loop** release candidate. The workspace version is bumped to `0.1.14`, docs and
deterministic TUI screenshots are refreshed, and the local test suite is green.

Publishing is still pending: no `v0.1.14` tag, GitHub Release, multi-platform
release assets, or Homebrew tap update has been created from this workspace yet.

## Landed

- Shared adapter capability records now back `/agent list` and `/agent doctor`.
  Doctor output includes readiness, mutation mode, evidence mode, config source,
  and known limits for Codex, Claude, custom template, tmux, PTY, and ACP
  surfaces.
- Lane timelines and isolation declarations are persisted as evidence. `/lane
  timeline <id>` exposes ordered lane events, while `/lane inspect <id>` links
  envelope, log, done, timeline, terminal artifacts, changed files, verification,
  decision, and next action.
- `/lane codex-review <task>` is the P0 read-only Codex review trust loop. It
  writes a lane envelope, runs `codex review --uncommitted` when Codex is
  available, supports `VIDEN_LANE_CODEX_REVIEW_TEMPLATE`, and records setup
  blockers as timeline evidence.
- Claude/tmux trust-loop setup is safer. `/lane tmux <id>` now preflights the
  default tmux/Claude path before marking a lane attached, and missing `tmux` or
  `claude` produces a setup-needed timeline event instead of a false attached
  state.
- Long-term roadmap and HN demand radar docs are added to keep the current TUI
  work aligned with the larger multi-agent orchestration and token-efficiency
  positioning.

## Evidence

- `cargo fmt --check`
- `cargo test -p viden-types --quiet`
- `cargo test -p viden-core agent_ --quiet`
- `cargo test -p viden-cli tui::lane --quiet`
- `cargo test -p viden-cli tui::command_palette --quiet`
- `cargo clippy -p viden-cli -p viden-core -p viden-types --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `VIDEN_TUI_SCREENSHOT_VERSION=0.1.14 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.14 --quick --out-dir /tmp/viden-0114-release-smoke-local`
- `git diff --check`

Local quick release smoke results:

- PASS `cargo-fmt`
- PASS `cargo-clippy`
- PASS `viden-cli-terminal-tests`
- PASS `tui-regression`
- PASS `fallback-cli-smoke`
- PASS `codex-app-server-protocol-fixture`
- PASS `codex-app-server-write-guard`
- PASS `lane-operator-loop-smoke`
- SKIP `package-smoke` (quick mode)
- SKIP `deepseek-cli-smoke` (opt-in live provider check)
- SKIP `github-actions-release-validation` (post-publish check)
- SKIP `github-release-assets-validation` (post-publish check)
- SKIP `homebrew-validation` (post-publish check)

Deterministic screenshots:

- [0.1.14 main](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.14-tui-main.svg)
- [0.1.14 command palette](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.14-tui-command-palette.svg)
- [0.1.14 lane detail](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.14-tui-lane-detail.svg)
- [0.1.14 side-1 lanes](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.14-tui-side-1.svg)
- [0.1.14 side-2 ops](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.14-tui-side-2.svg)

## Remaining Risks

- `0.1.14` still needs release publication: tag, GitHub Actions release assets,
  Homebrew formula update, and post-publish smoke.
- Codex and Claude happy paths are covered by deterministic templates and setup
  blockers locally; real authenticated Codex/Claude terminal runs should still
  be captured before calling the public release fully published.
- ACP remains descriptor/probe only. Mutating ACP/plugin/MCP/skill runtime
  support stays out of this release.
