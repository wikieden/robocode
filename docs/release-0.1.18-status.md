# Viden 0.1.18 Status - Interaction Hardening

Chinese version: [release-0.1.18-status.zh-CN.md](release-0.1.18-status.zh-CN.md)

`0.1.18` is the interaction-hardening release that finishes the provider/model
setup work after the published `0.1.17` baseline. It avoids moving the already
published `v0.1.17` tag and ships the newer settings selector behavior as a
fresh release.

## Release State

- Workspace version: `0.1.18`
- Git commit: `e0e40537094d6384e2dd08b8454d1d6c5be3a750`
- Git tag: `v0.1.18`
- GitHub release: https://github.com/wikieden/viden/releases/tag/v0.1.18
- Release workflow: https://github.com/wikieden/viden/actions/runs/26651202650
- Homebrew tap commit: `wikieden/homebrew-tap@651c7a7`
- Local package: `dist/viden-v0.1.18-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `8a3f199b716708ae84a7531928cbe2f4804afb30f63a81f767cdcd8b79e85bc0`

## Included Changes

- `/settings` now opens an actionable settings selector instead of a read-only
  status page.
- `/provider`, `/models`, `/permissions`, and `/theme` share the same centered
  selector interaction model: search, keyboard selection, mouse selection, and
  Enter-to-apply.
- `/settings permissions <mode>` changes the permission mode through the shared
  runtime command path.
- `/settings theme <name>` changes the live TUI theme inside the cockpit.
- The TUI design contract now states that settings surfaces are selector-first;
  diagnostics/details commands such as `/config` and `/provider doctor` remain
  information-oriented by design.
- README, user guide, release screenshots, and preview assertions now use the
  `/settings` selector as the canonical configuration screenshot.

## Validation

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
VIDEN_TUI_PREVIEW_PROVIDER=deepseek \
VIDEN_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-previews.sh docs/previews/generated
VIDEN_TUI_SCREENSHOT_VERSION=0.1.18 \
VIDEN_TUI_PREVIEW_PROVIDER=deepseek \
VIDEN_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.18 --quick \
  --github-release-assets --homebrew \
  --out-dir /tmp/viden-0118-postpublish-check
```

Result: passed at `2026-05-29T17:19:43Z` with 11 passed checks, 0 failed, and
3 intentionally skipped checks (`package-smoke`, opt-in `deepseek-cli-smoke`,
and explicit `github-actions-release-validation`).

Live DeepSeek smoke:

```bash
scripts/release-smoke.sh --version 0.1.18 --quick --deepseek \
  --out-dir /tmp/viden-0118-deepseek-smoke
```

Result: passed at `2026-05-29T17:29:16Z`, including `deepseek-cli-smoke`.

## Screenshot Evidence

Deterministic 0.1.18 TUI screenshots:

- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-main.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.18-tui-side-2.svg`

## Remaining Risks

- Theme selection is live in the current TUI session. Persistent theme defaults
  still require CLI/config usage and should become a later settings option.
- Manual macOS Terminal/iTerm2 mouse validation should still be repeated during
  release acceptance even though selector hit testing is covered by unit tests.
