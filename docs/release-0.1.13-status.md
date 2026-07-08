# Viden 0.1.13 Status

Chinese version: [release-0.1.13-status.zh-CN.md](release-0.1.13-status.zh-CN.md)

Last updated: 2026-05-28

## Status

`0.1.13` is implemented, tagged, published, and post-publish verified for the
**Operator Loop Hardening** cut. GitHub Releases now carries the multi-platform
assets, and the `wikieden/homebrew-tap` formula points at the `v0.1.13`
artifacts.

Release:

- GitHub Release: [v0.1.13](https://github.com/wikieden/viden/releases/tag/v0.1.13)
- Release workflow: [26526332898](https://github.com/wikieden/viden/actions/runs/26526332898)
- Homebrew tap commit: `b8f9c1d`

## Landed

- Default `viden` entry now opens the TUI cockpit; `--no-tui` keeps the
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
- `VIDEN_TUI_SCREENSHOT_VERSION=0.1.13 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.13 --out-dir /tmp/viden-0113-release-smoke-full-local`
- `scripts/release-smoke.sh --version 0.1.13 --quick --github-release-assets --out-dir /tmp/viden-0113-github-release-check`
- `scripts/release-smoke.sh --version 0.1.13 --quick --github-release-assets --homebrew --out-dir /tmp/viden-0113-postpublish-check`
- `scripts/release-smoke.sh --version 0.1.13 --quick --deepseek --out-dir /tmp/viden-0113-deepseek-live-check`

Full local release smoke results:

- PASS `cargo-fmt`
- PASS `cargo-clippy`
- PASS `viden-cli-tests`
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

Post-publish smoke results:

- PASS `github-release-assets-validation`
- PASS `homebrew-validation`
- PASS `deepseek-cli-smoke`

Published assets:

- `viden-v0.1.13-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.13-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.13-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.13-x86_64-pc-windows-msvc.tar.gz`
- matching `.sha256` files for all four archives

Deterministic screenshots:

- [0.1.13 main](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.13-tui-main.svg)
- [0.1.13 command palette](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.13-tui-command-palette.svg)
- [0.1.13 side-2 ops](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.13-tui-side-2.svg)

## Remaining Follow-Up

- No release blocker remains for `0.1.13`.
- Windows and Linux assets are produced by GitHub Actions; local post-publish
  smoke validates asset presence and Homebrew on the current macOS host.
