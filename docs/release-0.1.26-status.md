# RoboCode 0.1.26 Status - TUI Regression Pack And Mode Stability

Chinese version: [release-0.1.26-status.zh-CN.md](release-0.1.26-status.zh-CN.md)

`0.1.26` closes the remaining TUI-facing Mode / Permission work and turns the
active-turn input loop into a release gate. It also keeps the live DeepSeek
development smoke as mandatory evidence for release readiness.

## Status

- Workspace version: `0.1.26`
- Git tag: `v0.1.26`
- GitHub Release: pending publish
- Homebrew tap: pending publish sync

## Implemented

- `RuntimeSnapshot` now carries work mode and permission level into TUI state.
- The top bar and composer render real mode/permission values instead of static
  `Build` / `Ask` placeholders.
- `/plan on` updates visible TUI state to `Plan` / `Read Only` in the same
  command turn.
- During active provider turns, normal text queues as the next prompt, slash
  commands stay out of the prompt queue, and `/cancel`, `/stop`, `/interrupt`,
  or `/abort` request cancellation.
- Active-turn composer actions switch to queue/cancel/history affordances.
- TUI preview checks now assert the `LIVE WORK`, `input open`, queue, and cancel
  signals instead of old single-phrase thinking text.

## Verification

- `cargo fmt --all --check`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- `cargo test -p robocode-cli --quiet`: passed, `283` tests
- `cargo test --workspace --quiet`: passed
- `scripts/tdd-testing-contract-smoke.sh`: passed
- `scripts/tui-turn-controller-smoke.sh`: passed
- `scripts/plan-mode-smoke.sh /tmp/robocode-0126-plan-mode-smoke`: passed
- `scripts/daily-loop-smoke.sh /tmp/robocode-0126-daily-loop-smoke`: passed
- `scripts/tui-regression.sh docs/previews/generated`: passed
- `scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash --out-dir /tmp/robocode-0126-deepseek-dev-smoke`: passed

## DeepSeek Smoke Evidence

- Provider/model: `deepseek / deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok / `0` err
- Tokens: input `11382`, output `529`, total `11911`
- Estimated cost: `¥0.012440 CNY`
- Pricing basis: DeepSeek cache-miss estimate, input `¥1/1M`, output `¥2/1M`
- Evidence: `/tmp/robocode-0126-release-gate/prepublish/deepseek-dev-scenario/summary.md`

## Release Completion Gate

`0.1.26` is complete only after:

- `scripts/release-gate.sh --version 0.1.26 --phase prepublish` passes; done,
  evidence at `/tmp/robocode-0126-release-gate/prepublish`;
- GitHub Release `v0.1.26` assets are published;
- `wikieden/homebrew-tap` is updated to `0.1.26`;
- `scripts/release-gate.sh --version 0.1.26 --phase postpublish` passes.
