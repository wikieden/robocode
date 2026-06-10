# RoboCode 0.1.28 状态 - Delegated Lane 可见性收口

English version: [release-0.1.28-status.md](release-0.1.28-status.md)

`0.1.28` 正在进行。本版本聚焦 delegated lane 的状态、证据、下一步动作和 side
screen 计数，在最终 `0.1.x` 稳定性 gate 前把这些显示做真实。

## 状态

- Workspace version：`0.1.28`
- Git tag：待发布
- GitHub Release：待发布
- Homebrew tap：待同步

## 当前已实现

- 隔离 worktree 的 completed lane 现在显示 `accept lane` 和 `/lane accept <id>`；
  accepted lane 才显示 `apply lane` 和 `/lane apply <id>`。
- side status 现在把 delegated lane 分为 active、review、blocked、done 计数。
- side-1 lane 详情现在把 `applied`、`discarded`、`detached`、`stopped` 等关闭
  状态显示为 done，不再伪装成仍在 thinking 的活跃工作。

## 验证

- `cargo test -p robocode-cli agent_tasks_separate_completed_accept_and_accepted_apply_lane_actions -- --nocapture`：通过
- `cargo test -p robocode-cli side_status_rows_summarize_lane_background_counts -- --nocapture`：通过
- `cargo test -p robocode-cli side_lane_rows_render_closed_operator_states_as_done -- --nocapture`：通过
- `cargo test -p robocode-cli tui::side_screen::tests -- --nocapture`：通过
- `cargo test -p robocode-cli tui::state::tests::agent_tasks -- --nocapture`：通过
- `scripts/smoke-lane-operator-loop.sh`：通过
- `scripts/tui-turn-controller-smoke.sh`：通过
- `scripts/release-gate.sh --version 0.1.28 --phase prepublish --out-dir /tmp/robocode-0128-release-gate`：通过
  - PASS：cargo fmt、TDD contract、TUI TurnController smoke、clippy、
    robocode-cli tests、workspace tests、TUI regression、fallback CLI smoke、
    plan-mode smoke、daily-loop smoke、Codex app-server protocol/write guards、
    lane operator loop smoke、package smoke、DeepSeek dev scenario smoke。
  - 证据：`/tmp/robocode-0128-release-gate/prepublish`

## 剩余 Gate

- 发布 GitHub Release `v0.1.28`。
- 同步 `wikieden/homebrew-tap` 到 `0.1.28`。
- 运行 `scripts/release-gate.sh --version 0.1.28 --phase postpublish --out-dir /tmp/robocode-0128-release-gate`。

## DeepSeek Smoke 证据

- Provider/model：`deepseek / deepseek-v4-flash`
- 场景：`python_add_module_with_test`
- 请求：`3` ok / `0` err
- Tokens：input `10990`，output `401`，total `11391`
- 耗时：`6` 秒
- 预估费用：`¥0.011792 CNY`
- 计费依据：DeepSeek cache-miss estimate，input `¥1/1M`，output `¥2/1M`
- 失败分类：未观察到失败；live smoke 在本次 prepublish gate 首次通过。
- 证据：`/tmp/robocode-0128-release-gate/prepublish/deepseek-dev-scenario/summary.md`
