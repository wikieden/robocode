# RoboCode 0.1.27 Status - Daily Coding Loop Hardening

Chinese version: [release-0.1.27-status.zh-CN.md](release-0.1.27-status.zh-CN.md)

`0.1.27` is complete. This release closes daily coding-loop stability before
the next delegated-lane cleanup slice.

## Status

- Workspace version: `0.1.27`
- Git tag: `v0.1.27`
- GitHub Release: published at
  <https://github.com/wikieden/robocode/releases/tag/v0.1.27>
- Homebrew tap: synced in `wikieden/homebrew-tap` commit `eccca4c`

## Implemented So Far

- Added a full TUI controller regression for
  `prompt -> streaming -> approval -> write_file -> queued follow-up -> final`.
- Added a test-only terminal guard so TUI event-loop behavior can be verified
  without opening an alternate screen.
- Approval prompts no longer swallow ordinary composer typing; approval
  shortcuts still resolve the prompt.
- Bottom status bar now renders real work mode and permission level.
- Topbar status now uses real activity labels instead of static `auto` text.
- `scripts/tui-turn-controller-smoke.sh` now covers mode/permission sync,
  approval typing, active-turn queueing, and the full coding-loop controller
  path.

## Verification

- `cargo test -p robocode-cli provider_turn_streams_approves_tools_runs_queued_followup_and_releases_composer -- --nocapture`: passed
- `cargo test -p robocode-cli active_approval_does_not_swallow_composer_typing -- --nocapture`: passed
- `cargo test -p robocode-cli tui::statusbar::tests::bottom_bar_reflects_runtime_mode_and_permission_level -- --nocapture`: passed
- `cargo test -p robocode-cli tui::topbar::tests::top_bar_status_reflects_active_turn_instead_of_static_auto_text -- --nocapture`: passed
- `scripts/tui-turn-controller-smoke.sh`: passed
- `cargo fmt --check`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- `cargo test --workspace --quiet -- --test-threads=1`: passed after rerun outside
  the sandbox; the sandboxed run blocked a local HTTP test server with
  `Operation not permitted`.
- `scripts/release-gate.sh --version 0.1.27 --phase prepublish --out-dir /tmp/robocode-0127-release-gate`: passed
- `scripts/release-gate.sh --version 0.1.27 --phase postpublish --out-dir /tmp/robocode-0127-release-gate`: passed

## Release Gate

`0.1.27` is complete:

- prepublish gate passed, evidence at `/tmp/robocode-0127-release-gate/prepublish`;
- GitHub Release `v0.1.27` published with `8` assets;
- Homebrew tap synced to `0.1.27`, commit `eccca4c`;
- postpublish gate passed, evidence at `/tmp/robocode-0127-release-gate/postpublish`.

## DeepSeek Smoke Evidence

- Provider/model: `deepseek / deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok / `0` err
- Tokens: input `11542`, output `590`, total `12132`
- Elapsed seconds: `8`
- Estimated cost: `¥0.012722 CNY`
- Pricing basis: DeepSeek cache-miss estimate, input `¥1/1M`, output `¥2/1M`
- Failure class: none observed; live smoke passed on the first prepublish gate
  attempt.
- Evidence: `/tmp/robocode-0127-release-gate/prepublish/deepseek-dev-scenario/summary.md`
