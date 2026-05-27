# RoboCode 0.1.13 Status

Chinese version: [release-0.1.13-status.zh-CN.md](release-0.1.13-status.zh-CN.md)

Last updated: 2026-05-28

## Status

`0.1.13` is locally implemented and verified for the **Operator Loop
Hardening** cut. The full local release smoke, including package smoke, passed.
It has not yet been tagged, published to GitHub Releases, or published through
Homebrew in this workspace run.

## Landed

- Default `robocode` entry now opens the TUI cockpit; `--no-tui` keeps the
  legacy line REPL for scripts and smoke tests.
- `/settings` and `/setup` support provider/model discovery, key-status display,
  and explicit provider/model default persistence without storing secrets.
- Slash palette and approval tests now guard `/quit`, `/exit`, default approval,
  focus movement, direct shortcuts, and CJK/resize preview stability.
- Lane review has focused evidence commands:
  - `/lane diff <id>` writes and displays `L*.diff.patch`.
  - `/lane artifacts <id>` lists persisted lane artifacts.
- Main provider turns now build a ContextBundle and pass it as ephemeral request
  context before the final user message, preserving raw transcript audit data
  and provider compatibility.
- `/status`, runtime task evidence, and side-2 context rows expose context
  pressure, source count, largest sources, and compaction notes.
- Non-interactive smoke scripts explicitly pass `--no-tui`, so the default TUI
  launch does not break release automation.

## Evidence

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/smoke-lane-operator-loop.sh`
- `ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.13 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.13 --quick --out-dir /tmp/robocode-0113-release-smoke-quick`
- `scripts/release-smoke.sh --version 0.1.13 --out-dir /tmp/robocode-0113-release-smoke-full-local`

Full local release smoke results:

- PASS `cargo-fmt`
- PASS `cargo-clippy`
- PASS `robocode-cli-tests`
- PASS `workspace-tests`
- PASS `tui-regression`
- PASS `fallback-cli-smoke`
- PASS `codex-app-server-protocol-fixture`
- PASS `codex-app-server-write-guard`
- PASS `lane-operator-loop-smoke`
- PASS `package-smoke`
- SKIP `deepseek-cli-smoke` (opt-in live provider check)
- SKIP `github-actions-release-validation` (post-publish check)
- SKIP `github-release-assets-validation` (post-publish check)
- SKIP `homebrew-validation` (post-publish check)

Deterministic screenshots:

- [0.1.13 main](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.13-tui-main.svg)
- [0.1.13 command palette](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.13-tui-command-palette.svg)
- [0.1.13 side-2 ops](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.13-tui-side-2.svg)

## Remaining Before Public Release

- Run opt-in DeepSeek smoke with a live key if desired.
- Tag `v0.1.13`, publish GitHub release assets, update Homebrew tap, and run
  post-publish release validation.
