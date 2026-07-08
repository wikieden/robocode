# Viden 0.1.28 Status - Delegated Lane Visibility Cleanup

Chinese version: [release-0.1.28-status.zh-CN.md](release-0.1.28-status.zh-CN.md)

`0.1.28` is complete. This release focuses on making delegated lane state,
evidence, next actions, and side-screen counts truthful before the final
`0.1.x` stability gate.

## Status

- Workspace version: `0.1.28`
- Git tag: `v0.1.28`
- GitHub Release: published at
  <https://github.com/wikieden/viden/releases/tag/v0.1.28>
- Homebrew tap: synced in `wikieden/homebrew-tap` commit `5561846`

## Implemented So Far

- Completed isolated lanes now surface `accept lane` with `/lane accept <id>`;
  accepted isolated lanes surface `apply lane` with `/lane apply <id>`.
- Side status now includes delegated lane counts split into active, review,
  blocked, and done buckets.
- Side-1 lane details now render closed operator states such as `applied`,
  `discarded`, `detached`, and `stopped` as done instead of active thinking work.

## Verification

- `cargo test -p viden-cli agent_tasks_separate_completed_accept_and_accepted_apply_lane_actions -- --nocapture`: passed
- `cargo test -p viden-cli side_status_rows_summarize_lane_background_counts -- --nocapture`: passed
- `cargo test -p viden-cli side_lane_rows_render_closed_operator_states_as_done -- --nocapture`: passed
- `cargo test -p viden-cli tui::side_screen::tests -- --nocapture`: passed
- `cargo test -p viden-cli tui::state::tests::agent_tasks -- --nocapture`: passed
- `scripts/smoke-lane-operator-loop.sh`: passed
- `scripts/tui-turn-controller-smoke.sh`: passed
- `scripts/release-gate.sh --version 0.1.28 --phase prepublish --out-dir /tmp/viden-0128-release-gate`: passed
  - PASS: cargo fmt, TDD contract, TUI TurnController smoke, clippy,
    viden-cli tests, workspace tests, TUI regression, fallback CLI smoke,
    plan-mode smoke, daily-loop smoke, Codex app-server protocol/write guards,
    lane operator loop smoke, package smoke, DeepSeek dev scenario smoke.
  - Evidence: `/tmp/viden-0128-release-gate/prepublish`
- GitHub Release workflow run `27277703367`: passed and uploaded `8` assets.
- `scripts/release-smoke.sh --version 0.1.28 --quick --skip-package --github-release-assets --out-dir /tmp/viden-0128-github-release-check`: passed
- `HOMEBREW_NO_AUTO_UPDATE=1 brew fetch --force --formula wikieden/tap/viden`: passed, formula `viden (0.1.28)`
- `HOMEBREW_NO_AUTO_UPDATE=1 brew audit --formula wikieden/tap/viden`: passed
- `scripts/release-gate.sh --version 0.1.28 --phase postpublish --out-dir /tmp/viden-0128-release-gate`: passed
  - PASS: GitHub release asset validation and Homebrew validation.
  - Evidence: `/tmp/viden-0128-release-gate/postpublish`

## Release Gate

`0.1.28` is complete:

- prepublish gate passed, evidence at `/tmp/viden-0128-release-gate/prepublish`;
- GitHub Release `v0.1.28` published with `8` assets;
- Homebrew tap synced to `0.1.28`, commit `5561846`;
- postpublish gate passed, evidence at `/tmp/viden-0128-release-gate/postpublish`.

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
- Evidence: `/tmp/viden-0128-release-gate/prepublish/deepseek-dev-scenario/summary.md`
