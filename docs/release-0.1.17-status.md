# Viden 0.1.17 Status - Daily Coding Loop Baseline

Chinese version: [release-0.1.17-status.zh-CN.md](release-0.1.17-status.zh-CN.md)

## Status

`0.1.17` is the published daily coding loop baseline release.
It makes first-run setup DeepSeek-first, keeps fallback explicit for offline
testing, adds model recovery guidance, and adds a deterministic daily-loop
smoke to the release evidence path. It also lands the minimal task brief /
steering support promised in the 0.1.17 plan.

- Workspace version: `0.1.17`
- Main commit: `49ba8df`
- Git tag: `v0.1.17`
- GitHub release: https://github.com/wikieden/viden/releases/tag/v0.1.17
- Rust CI: https://github.com/wikieden/viden/actions/runs/26635714278
- Release artifacts workflow:
  https://github.com/wikieden/viden/actions/runs/26635986910
- Homebrew tap commit: `wikieden/homebrew-tap@2160d14`
- Local package: `dist/viden-v0.1.17-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `999edafa93e9c5863370a9857d1e96c430174572ab2b8b6f1e3c7106e7933ed1`

Published release asset sha256:

- `aarch64-apple-darwin`:
  `eaf0d02b6e7666b963ad20bb888616362e7bb02ba82504d729c2721db42c88b5`
- `x86_64-apple-darwin`:
  `c682906f44c4374d29f2e1a7ddc4b1587309b612ebd6617528b31cf15d9d492d`
- `x86_64-unknown-linux-gnu`:
  `cb4c51a491a2550ea8a0bfce85ddae5657428b2b3a612e9c32b50bcad7f84250`
- `x86_64-pc-windows-msvc`:
  `67e3efe36afd355e35860926d4ea194eb42326ea0c905a505ac1b329895679bf`

## Landed Scope

- Clean installs now resolve to `deepseek` as the default online provider.
  `fallback / test-local` remains available as an explicit offline and CI
  smoke path.
- `/setup` now renders an interactive provider/model setup guide in the TUI
  flow, including DeepSeek default setup, fallback setup, API-key status,
  provider choices, and command-palette operation hints.
- `/setup provider <id> [model]` and `/setup model <model>` reuse the saved
  provider/model path from `/settings`, so users can configure provider/model
  without storing API keys.
- Provider/model failures now include a switch-model recovery block when the
  error looks like an unavailable, unauthorized, unsupported, or incompatible
  model.
- `/brief <goal>` and `/spec <goal>` create an active task brief at
  `.viden/briefs/active.md`; `/brief show` renders it and `/brief clear`
  removes it.
- `/brief steering init` creates minimal project steering templates under
  `.viden/steering/`, and `/brief steering show` summarizes them.
- Provider ContextBundle construction and lane envelopes now reference the
  active brief and steering summaries when present; side-2 ops also surfaces
  the active brief id/title.
- `scripts/daily-loop-smoke.sh` runs a deterministic edit -> approval -> test
  -> diff -> status loop with active brief/steering evidence and writes
  transcript, diff, TUI ANSI preview, and summary evidence.
- `scripts/release-smoke.sh` now includes the `daily-loop-smoke` step.
- README and screenshot references were refreshed for the `0.1.17` daily-loop
  RC.

## Verification

Passed locally on 2026-05-29:

```bash
cargo fmt --check
git diff --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p viden-config --quiet
cargo test -p viden-model --quiet
cargo test -p viden-core --quiet -- --test-threads=1
cargo test -p viden-cli --quiet -- --test-threads=1
cargo test --workspace --quiet -- --test-threads=1
VIDEN_TUI_SCREENSHOT_VERSION=0.1.17 \
  VIDEN_TUI_PREVIEW_PROVIDER=deepseek \
  VIDEN_TUI_PREVIEW_MODEL=deepseek-v4-flash \
  scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh /tmp/viden-0117-daily-loop-smoke
scripts/daily-loop-smoke.sh /tmp/viden-0117-daily-loop-smoke-brief
scripts/release-smoke.sh --version 0.1.17 --quick \
  --out-dir /tmp/viden-0117-release-smoke-local
scripts/release-smoke.sh --version 0.1.17 --quick \
  --out-dir /tmp/viden-0117-release-smoke-local-brief
scripts/release-smoke.sh --version 0.1.17 --skip-package \
  --out-dir /tmp/viden-0117-release-smoke-full-nopackage
scripts/release-smoke.sh --version 0.1.17 --skip-package \
  --out-dir /tmp/viden-0117-release-smoke-full-nopackage-brief
scripts/release-smoke.sh --version 0.1.17 --quick --github-release-assets \
  --homebrew --out-dir /tmp/viden-0117-postpublish-check
scripts/package-release.sh 0.1.17 aarch64-apple-darwin
cd dist && shasum -a 256 -c viden-v0.1.17-aarch64-apple-darwin.tar.gz.sha256
```

Evidence directories:

```text
/tmp/viden-0117-daily-loop-smoke
/tmp/viden-0117-daily-loop-smoke-brief
/tmp/viden-0117-release-smoke-local
/tmp/viden-0117-release-smoke-local-brief
/tmp/viden-0117-release-smoke-full-nopackage
/tmp/viden-0117-release-smoke-full-nopackage-brief
/tmp/viden-0117-postpublish-check
```

## Visual Evidence

Deterministic 0.1.17 TUI screenshots:

- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-main.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.17-tui-side-2.svg`

Daily-loop smoke evidence:

- `/tmp/viden-0117-daily-loop-smoke/daily-loop-transcript.log`
- `/tmp/viden-0117-daily-loop-smoke/daily-loop.diff`
- `/tmp/viden-0117-daily-loop-smoke/daily-loop-tui-preview.ansi`
- `/tmp/viden-0117-daily-loop-smoke/summary.md`
- `/tmp/viden-0117-daily-loop-smoke-brief/workspace/.viden/briefs/active.md`
- `/tmp/viden-0117-daily-loop-smoke-brief/workspace/.viden/steering/conventions.md`

## Remaining Risks

- The `/setup` flow is command-palette guided rather than a full modal wizard.
  A richer picker remains a follow-up once the daily loop is stable.
- DeepSeek live smoke was not run in this local RC pass unless
  `DEEPSEEK_API_KEY` is supplied separately with `scripts/release-smoke.sh
  --deepseek`.
- Task brief / steering is intentionally minimal; it is a daily-loop context
  aid, not a full Kiro-style spec product yet.

## Next

Continue into the next version with deeper interaction polish and provider
configuration UX, using the 0.1.17 daily-loop smoke as a non-regression gate.
