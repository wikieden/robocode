# RoboCode 0.1.23 Status - Provider And Model Setup Patch

Chinese version: [release-0.1.23-status.zh-CN.md](release-0.1.23-status.zh-CN.md)

`0.1.23` is a focused usability release for provider/model setup. It separates
provider connection from model selection, moves `/connect` and `/models` closer
to the opencode picker pattern, and makes provider authentication modes visible
without treating every provider as API-key-only.

## Release State

- Workspace version: `0.1.23`
- Release commit: `ec608e62d94bde511f2c25b6a1322baa873c7b76`
- Git tag: `v0.1.23`
- GitHub release:
  `https://github.com/wikieden/robocode/releases/tag/v0.1.23`
- Release workflow:
  `https://github.com/wikieden/robocode/actions/runs/26711635516`
- Homebrew tap commit: `wikieden/homebrew-tap@708cef1`
- Local package: `dist/robocode-v0.1.23-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `5c20394e27a68187ebfe095d9a5afb6e80e562c9fa7f7d577aa125b41f77ed61`

## Included Changes

- `/connect` is the provider connection picker. First-level rows are suppliers,
  not provider/model combinations.
- `/connect <provider>` opens a provider-scoped detail surface with masked key
  status, endpoint source, default model, active model list, favorite model
  actions, and provider doctor entry points.
- Provider descriptors now expose auth modes. OpenAI can advertise web login or
  API key, gateway providers advertise API key, and local providers advertise
  local auth.
- `/models` shows Favorites first, then Recent, then provider-grouped active
  model rows.
- Model favorites are provider/model pairs. Favorite rows are not repeated in
  later provider groups, and `Ctrl-F` favorites the selected model row.
- Provider-scoped config writes persist active models and favorite models without
  storing plaintext API keys.
- README, user guide, module index, staged roadmap, and deterministic TUI
  preview evidence were updated for the 0.1.23 setup flow.

## Validation

Focused checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
git diff --check
```

TUI visual evidence:

```bash
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.23 \
scripts/tui-regression.sh docs/previews/generated
```

Release smoke:

```bash
scripts/release-smoke.sh --version 0.1.23 \
  --out-dir /tmp/robocode-0123-release-smoke-full-local
```

Result: passed with local package smoke. Skipped only opt-in live DeepSeek,
GitHub-release asset, GitHub Actions, and Homebrew checks that require
post-publish state.

Post-publish verification:

```bash
scripts/release-smoke.sh --version 0.1.23 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0123-postpublish-check
```

Result: passed. The smoke validated published GitHub release assets and the
Homebrew formula at `/tmp/robocode-0123-postpublish-check`.

Homebrew tap checks:

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --formula wikieden/tap/robocode
HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/robocode
```

Result: passed before pushing `wikieden/homebrew-tap@708cef1`.

## Screenshot Evidence

Deterministic 0.1.23 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.23-tui-side-2.svg`

## Remaining Risks

- OpenAI web-login is descriptor-visible only in this release; the runtime
  browser-login flow remains future work.
- Provider detail is still a selector/detail surface, not a full field editor
  with save/cancel transaction semantics.
- Live provider availability is model/account dependent; local fallback,
  deterministic TUI evidence, release assets, and Homebrew smoke remain the
  required release gates.
