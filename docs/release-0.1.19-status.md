# RoboCode 0.1.19 Status - Delegated Lane Usefulness

Chinese version: [release-0.1.19-status.zh-CN.md](release-0.1.19-status.zh-CN.md)

`0.1.19` is the Delegated Lane Usefulness release. It turns the 0.1.18
selector-first interaction rule into a more useful coding workflow: provider
configuration is separated from model selection, deterministic lane evidence is
visible in the cockpit, and release validation now includes the daily-loop and
lane operator-loop smoke gates.

## Release State

- Workspace version: `0.1.19`
- Git commit: `2319f26339c6403a6c280d4c1940179b55b79052`
- Git tag: `v0.1.19`
- GitHub release: https://github.com/wikieden/robocode/releases/tag/v0.1.19
- Release workflow: https://github.com/wikieden/robocode/actions/runs/26677360247
- Homebrew tap commit: `wikieden/homebrew-tap@d5fa2c143ae9967b9837104e163289ff3f924764`
- Local package: `dist/robocode-v0.1.19-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `2124d8ab31f73fe98113a70f9816cf23aa4b204ac68fa44f9612ff986d041937`

## Included Changes

- `/provider` is now a provider configuration surface. It shows API key/env
  status, endpoint source, provider doctor entry points, and known model
  candidates for the selected supplier.
- `/models` is now the cross-provider model picker. Models are grouped by
  provider and selecting a row switches both provider and model through the
  shared runtime command path.
- `/model` remains the current-provider quick switch for users who only want to
  change the model inside the active provider.
- `/settings provider` and `/setup provider` use the provider selector without
  pretending that provider selection is the same thing as model selection.
- Deterministic TUI previews now include provider and model selector screenshots
  so future interaction changes can be reviewed visually.
- Release smoke now keeps `daily-loop-smoke` and `lane-operator-loop-smoke` in
  the normal post-publish gate.

## Validation

Focused and workspace checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p robocode-cli command_palette --quiet
cargo test -p robocode-core provider --quiet
cargo test -p robocode-cli --quiet
cargo test --workspace --quiet
```

CLI and visual smoke:

```bash
printf '/provider\n/models\n/quit\n' | \
  cargo run -p robocode-cli -- --no-tui --provider fallback --model test-local

ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.19 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

Local release smoke:

```bash
scripts/release-smoke.sh --version 0.1.19 --quick \
  --out-dir /tmp/robocode-0119-release-smoke-local
```

Result: passed with 9 checks and 5 intentional skips (`package-smoke`, opt-in
`deepseek-cli-smoke`, GitHub release validation, GitHub asset validation, and
Homebrew validation).

GitHub release asset validation:

```bash
scripts/release-smoke.sh --version 0.1.19 --quick \
  --github-release-assets --skip-package \
  --out-dir /tmp/robocode-0119-github-release-check
```

Result: GitHub release assets passed checksum validation.

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.19 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0119-postpublish-check
```

Result: passed at `/tmp/robocode-0119-postpublish-check`, including GitHub
release assets and Homebrew validation.

Homebrew formula validation:

```bash
brew fetch --formula wikieden/tap/robocode
brew audit --formula wikieden/tap/robocode
```

Result: `brew fetch` resolved formula `robocode (0.1.19)` and audit produced no
errors.

## Screenshot Evidence

Deterministic 0.1.19 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-side-2.svg`

## Remaining Risks

- Provider configuration still edits through slash commands and environment
  variables. A later settings release should add first-use guided credential
  entry with secret-safe persistence rules.
- `/models` now makes cross-provider selection clear, but remote model discovery
  is still descriptor/static-list driven for most providers.
- Codex/Claude delegated lanes remain adapters on top of the shared lane/task
  model. The deterministic shell/template loop is the release blocker that is
  verified for 0.1.19.
