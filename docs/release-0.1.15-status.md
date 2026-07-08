# Viden 0.1.15 Status

Chinese version: [release-0.1.15-status.zh-CN.md](release-0.1.15-status.zh-CN.md)

Last updated: 2026-05-29

## Status

`0.1.15` is implemented and locally verified as the **Context Curator And
Budget Controls** release candidate. The workspace version is bumped to
`0.1.15`, ContextBundle v1 records are visible in provider turns and lane
envelopes, `/context` is available, and deterministic TUI screenshots are
refreshed.

Publishing is still pending: no `v0.1.15` tag, GitHub Release, multi-platform
release assets, or Homebrew tap update has been created from this workspace yet.

## Landed

- Shared `ContextBundleRecord` now records policy, source priority,
  include-reason, omitted sources, and omission reason.
- Main provider turns and lane envelopes use the same
  `v1-priority-budget` policy. High-priority task/workspace/test context stays
  ahead of lower-priority summaries when budget pressure rises.
- `/context` shows the latest provider ContextBundle with source ordering,
  token estimates, omitted sources, and compaction notes.
- `AgentTask` evidence now carries context policy and omitted-source counts, so
  side-2 ops can show `BUNDLE`, `POLICY`, and `OMIT` rows from the shared
  runtime task snapshot.
- The command palette advertises `/context`, and the user guide documents how
  to inspect provider-side context decisions.

## Evidence

- `cargo fmt --check`
- `cargo test -p viden-types --quiet`
- `cargo test -p viden-core context --quiet`
- `cargo test -p viden-cli tui::preview --quiet`
- `cargo test -p viden-cli tui::ops_screen --quiet`
- `cargo test -p viden-cli tui::command_palette --quiet`
- `cargo test -p viden-cli tui::lane::tests::lane_envelope_includes_context_bundle_sources_and_pressure --quiet`
- `cargo clippy -p viden-types -p viden-core -p viden-cli --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `VIDEN_TUI_SCREENSHOT_VERSION=0.1.15 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.15 --quick --out-dir /tmp/viden-0115-release-smoke-local`

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

- [0.1.15 main](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-main.svg)
- [0.1.15 command palette](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-command-palette.svg)
- [0.1.15 lane detail](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-lane-detail.svg)
- [0.1.15 side-1 lanes](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-side-1.svg)
- [0.1.15 side-2 ops](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-side-2.svg)

## Remaining Risks

- `0.1.15` still needs release publication: tag, GitHub Actions release assets,
  Homebrew formula update, and post-publish smoke.
- Pin/omit commands and per-lane budget overrides are still P1 design work; the
  current release only exposes the policy and evidence foundation.
- Main provider prompt assembly still uses the compact ContextBundle injection
  path. Full budget-aware prompt composition remains a later roadmap item.
