# RoboCode 0.1.22 Status - Provider Detail Usability Patch

Chinese version: [release-0.1.22-status.zh-CN.md](release-0.1.22-status.zh-CN.md)

`0.1.22` is a focused usability patch on top of `0.1.21`. It keeps the
interaction-system completion scope intact and tightens the provider detail page
so it behaves more like a compact settings form.

## Release State

- Workspace version: `0.1.22`
- Release commit: `d27b4e43209477ba93327148ca72c013ba8da945`
- Git tag: `v0.1.22`
- GitHub release:
  `https://github.com/wikieden/robocode/releases/tag/v0.1.22`
- Release workflow:
  `https://github.com/wikieden/robocode/actions/runs/26699720640`
- Homebrew tap commit: `wikieden/homebrew-tap@fece0ca`
- Local package: `dist/robocode-v0.1.22-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `e4c093d141ac6e13957f84a5196ea2e76e14af3cdea2590284e35f088ee02b89`

## Included Changes

- Provider detail now masks present API keys as prefix + `*` + suffix instead
  of showing only `present`.
- Provider detail action rows now show the current target value, such as the
  provider id or model name, instead of explanatory prose on every row.
- Model actions on the provider detail page no longer repeat long "save with
  model" descriptions.
- TUI preview generation now uses deterministic fake preview keys and unsets
  API-base overrides so screenshots do not capture local secret fragments or
  user-specific endpoints.
- README, user guide, TUI design docs, modules, and staged roadmap were updated
  to describe the masked-key behavior and the next editable-form direction.

## Validation

Focused checks:

```bash
cargo fmt --check
cargo check -p robocode-cli
cargo test -p robocode-cli command_palette --quiet
cargo test -p robocode-cli preview --quiet
git diff --check
```

TUI visual evidence:

```bash
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.22 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

Release smoke:

```bash
scripts/release-smoke.sh --version 0.1.22 \
  --out-dir /tmp/robocode-0122-release-smoke-full-local
```

Result: passed with local package smoke. Skipped only opt-in live DeepSeek,
GitHub-release asset, GitHub Actions, and Homebrew checks that require
post-publish state.

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.22 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0122-postpublish-check
```

Result: passed. The smoke validated published GitHub release assets and the
Homebrew formula at `/tmp/robocode-0122-postpublish-check`.

Homebrew tap checks:

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula wikieden/tap/robocode
HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/robocode
```

Result: passed before pushing `wikieden/homebrew-tap@fece0ca`.

## Screenshot Evidence

Deterministic 0.1.22 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.22-tui-side-2.svg`

## Remaining Risks

- Provider detail is still an actionable selector page, not a true editable
  form. The next interaction release should add focused field editing for key
  source, endpoint, default model, connection test, save, and cancel.
- Live DeepSeek smoke was not run in this release turn; fallback, daily-loop,
  lane operator-loop, local package smoke, GitHub release asset validation,
  Homebrew validation, and deterministic TUI evidence passed.
