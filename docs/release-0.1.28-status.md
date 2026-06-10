# RoboCode 0.1.28 Status - Delegated Lane Visibility Cleanup

Chinese version: [release-0.1.28-status.zh-CN.md](release-0.1.28-status.zh-CN.md)

`0.1.28` is in progress. This release focuses on making delegated lane state,
evidence, next actions, and side-screen counts truthful before the final
`0.1.x` stability gate.

## Status

- Workspace version: `0.1.28`
- Git tag: pending
- GitHub Release: pending
- Homebrew tap: pending

## Implemented So Far

- Completed isolated lanes now surface `accept lane` with `/lane accept <id>`;
  accepted isolated lanes surface `apply lane` with `/lane apply <id>`.
- Side status now includes delegated lane counts split into active, review,
  blocked, and done buckets.
- Side-1 lane details now render closed operator states such as `applied`,
  `discarded`, `detached`, and `stopped` as done instead of active thinking work.

## Verification

- `cargo test -p robocode-cli agent_tasks_separate_completed_accept_and_accepted_apply_lane_actions -- --nocapture`: passed
- `cargo test -p robocode-cli side_status_rows_summarize_lane_background_counts -- --nocapture`: passed
- `cargo test -p robocode-cli side_lane_rows_render_closed_operator_states_as_done -- --nocapture`: passed
- `cargo test -p robocode-cli tui::side_screen::tests -- --nocapture`: passed
- `cargo test -p robocode-cli tui::state::tests::agent_tasks -- --nocapture`: passed
- `scripts/smoke-lane-operator-loop.sh`: passed
- `scripts/tui-turn-controller-smoke.sh`: passed
- `scripts/release-gate.sh --version 0.1.28 --phase prepublish --out-dir /tmp/robocode-0128-release-gate`: passed
  - PASS: cargo fmt, TDD contract, TUI TurnController smoke, clippy,
    robocode-cli tests, workspace tests, TUI regression, fallback CLI smoke,
    plan-mode smoke, daily-loop smoke, Codex app-server protocol/write guards,
    lane operator loop smoke, package smoke, DeepSeek dev scenario smoke.
  - Evidence: `/tmp/robocode-0128-release-gate/prepublish`

## Remaining Gate

- Publish GitHub Release `v0.1.28`.
- Sync `wikieden/homebrew-tap` to `0.1.28`.
- Run `scripts/release-gate.sh --version 0.1.28 --phase postpublish --out-dir /tmp/robocode-0128-release-gate`.

## DeepSeek Smoke Evidence

- Provider/model: `deepseek / deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok / `0` err
- Tokens: input `10990`, output `401`, total `11391`
- Elapsed seconds: `6`
- Estimated cost: `¥0.011792 CNY`
- Pricing basis: DeepSeek cache-miss estimate, input `¥1/1M`, output `¥2/1M`
- Failure class: none observed; live smoke passed on the first prepublish gate
  attempt.
- Evidence: `/tmp/robocode-0128-release-gate/prepublish/deepseek-dev-scenario/summary.md`
