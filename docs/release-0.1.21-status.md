# RoboCode 0.1.21 Status - Interaction System Completion

Chinese version: [release-0.1.21-status.zh-CN.md](release-0.1.21-status.zh-CN.md)

`0.1.21` is the Interaction System Completion release. It tightens the
provider/settings experience around actionable picker/form surfaces, keeps
provider configuration separate from model selection, and adds screenshot
evidence for the supplier-only provider list plus the provider detail form.

## Release State

- Workspace version: `0.1.21`
- Release commit: `07910a2f019f92127c322b18224aee5ec56b6348`
- Git tag: `v0.1.21`
- GitHub release:
  `https://github.com/wikieden/robocode/releases/tag/v0.1.21`
- Release workflow:
  `https://github.com/wikieden/robocode/actions/runs/26689608294`
- Homebrew tap commit: `wikieden/homebrew-tap@4108da0`
- Local package: `dist/robocode-v0.1.21-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `1f83e4dbf3f347d0dcbb4c67407f9bff4026f637e626a0782292260bb9505e55`

## Included Changes

- `/provider` now opens a supplier-only first-level picker: provider ids such
  as `deepseek`, `openrouter`, `anthropic`, and `openai` appear without API key,
  endpoint, model, or explanatory text on the first rows.
- Selecting a provider opens `PROVIDER CONFIG`, which shows the key env status,
  endpoint, candidate models, set-default action, session switch, doctor, and
  `/models` handoff.
- The TUI preview and regression matrix now includes a separate provider detail
  screenshot so product review can compare provider selection and provider
  configuration as two distinct states.
- README and user guides point to the 0.1.21 screenshot evidence set, including
  the new provider detail form.
- The release remains focused on interaction polish. ACP/MCP/plugin/skill
  mutation paths are still out of scope for this version.

## Validation

Focused and workspace checks:

```bash
cargo fmt --check
git diff --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p robocode-cli preview --quiet
cargo test -p robocode-cli command_palette --quiet
cargo test --workspace --quiet
scripts/daily-loop-smoke.sh /tmp/robocode-0121-daily-loop-smoke
```

TUI visual evidence:

```bash
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.21 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

Local release smoke:

```bash
scripts/release-smoke.sh --version 0.1.21 \
  --out-dir /tmp/robocode-0121-release-smoke-full-local
```

Result: passed with local package smoke. Skipped only opt-in live DeepSeek,
GitHub-release asset, GitHub Actions, and Homebrew checks that require
post-publish state.

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.21 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0121-postpublish-check
```

Result: passed. The smoke validated published GitHub release assets and the
Homebrew formula at `/tmp/robocode-0121-postpublish-check`.

Homebrew tap checks:

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula wikieden/tap/robocode
HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/robocode
```

Result: passed before pushing `wikieden/homebrew-tap@4108da0`.

## Screenshot Evidence

Deterministic 0.1.21 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.21-tui-side-2.svg`

Feature mapping:

- Provider selection: `0.1.21-tui-provider-selector.svg`
- Provider detail configuration: `0.1.21-tui-provider-detail.svg`
- Model selection: `0.1.21-tui-model-selector.svg`
- First-run setup: `0.1.21-tui-setup-wizard.svg`
- Work visibility: `0.1.21-tui-main.svg`, `0.1.21-tui-live-turn.svg`
- Composer/CJK/resize reliability: `0.1.21-tui-cjk-input.svg`,
  `0.1.21-tui-main-resize.svg`
- Lane operation and side screens: `0.1.21-tui-lane-selector.svg`,
  `0.1.21-tui-lane-detail.svg`, `0.1.21-tui-side-1.svg`,
  `0.1.21-tui-side-2.svg`

## Remaining Risks

- Provider detail is actionable and discoverable, but endpoint/key editing is
  still primarily command/env based. A later settings-form release should add
  direct text editing fields.
- Mouse/focus behavior is covered for selector rows and modal previews, but
  richer pane scrolling and right-rail click routing remain follow-up work.
- Live DeepSeek smoke was not run in this release turn; fallback, daily-loop,
  lane operator-loop, local package smoke, GitHub release asset validation,
  Homebrew validation, and deterministic TUI evidence passed.
