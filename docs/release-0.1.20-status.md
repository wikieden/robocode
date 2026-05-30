# RoboCode 0.1.20 Status - Usability Beta Gate

Chinese version: [release-0.1.20-status.zh-CN.md](release-0.1.20-status.zh-CN.md)

`0.1.20` is the Usability Beta Gate release. It focuses on making the TUI
usable without guessing hidden commands: first-run setup opens an actionable
wizard, provider/model recovery is explicit, and lane operation has centered
selectors plus deterministic screenshot evidence.

## Release State

- Workspace version: `0.1.20`
- Release commit: `320de3318bb0e53727497ee0b23cec4e9cc40a41`
- Git tag: `v0.1.20`
- GitHub release: https://github.com/wikieden/robocode/releases/tag/v0.1.20
- Release workflow: https://github.com/wikieden/robocode/actions/runs/26686753200
- Homebrew tap commit: `wikieden/homebrew-tap@2f57fb1f8526afcea293f86377a584a212000201`
- Local package: `dist/robocode-v0.1.20-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `7cc2eeb04ceebcf67926d0aeb843d538065bf0150e374c17f3a0b175ac9fa8b2`

## Included Changes

- `/setup` now opens a dedicated `SETUP WIZARD` selector with actionable rows
  for provider configuration, model selection, permissions, theme, provider
  doctor, fallback smoke, and saving defaults.
- Clean TUI startup preloads `/setup` when the selected online provider is
  missing its API key, while keeping fallback sessions offline and direct.
- `/provider` remains the supplier configuration surface. Selecting a provider
  opens `PROVIDER CONFIG` with key/env, endpoint, model candidates, doctor, and
  switch/save actions.
- `/models` remains the provider-grouped model selector for cross-provider
  model switching.
- Provider/model failures are classified into recovery classes such as missing
  key, auth, rate limit, timeout, context overflow, compatibility, and model
  unavailable, each with a concrete next action.
- `/lane` now opens a centered `LANE ACTIONS` selector and includes
  id-specific inspect, timeline, diff, and artifacts actions for tracked lanes.
- TUI preview and regression fixtures now cover setup wizard and lane selector
  states, in addition to the existing main cockpit, resize, CJK input,
  provider/model selector, lane detail, and side-screen states.
- `docs/release-0.1.21-plan.md` records the next interaction-system completion
  plan so future work continues from the same usability findings.

## Validation

Focused and workspace checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/daily-loop-smoke.sh /tmp/robocode-0120-daily-loop-smoke
```

TUI visual evidence:

```bash
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-previews.sh docs/previews/generated

ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.20 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

Local release smoke:

```bash
scripts/release-smoke.sh --version 0.1.20 \
  --out-dir /tmp/robocode-0120-release-smoke-full-local
```

Result: passed with `package-smoke`; skipped only opt-in live DeepSeek,
GitHub-release, and Homebrew checks that require post-publish state.

GitHub release asset validation:

```bash
scripts/release-smoke.sh --version 0.1.20 --quick \
  --github-release-assets --skip-package \
  --out-dir /tmp/robocode-0120-github-release-check
```

Result: GitHub release assets passed checksum validation.

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.20 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0120-postpublish-check
```

Result: passed at `/tmp/robocode-0120-postpublish-check`, including GitHub
release asset validation and Homebrew validation.

Homebrew formula validation:

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula wikieden/tap/robocode
HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/robocode
```

Result: `brew fetch` resolved formula `robocode (0.1.20)` and `brew audit`
produced no errors.

## Screenshot Evidence

Deterministic 0.1.20 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.20-tui-side-2.svg`

## Remaining Risks

- Provider configuration is now discoverable, but endpoint/key editing is still
  mostly command/env based. `0.1.21` should finish the unified settings/form
  runtime.
- Mouse/focus behavior is improved for selectors but still needs one explicit
  focus router across composer, approval, selectors, lane detail, side screens,
  and right rail.
- Live DeepSeek smoke was not run in this release turn; fallback, deterministic
  daily-loop, lane operator-loop, GitHub assets, and Homebrew were verified.
- Codex/Claude/tmux lanes remain supervised adapter paths. The release blocker
  remains the deterministic shell/template lane loop.
