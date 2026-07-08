# Viden 0.1.28 计划 - Delegated Lane 可见性收口

English version: [release-0.1.28-plan.md](release-0.1.28-plan.md)

`0.1.28` 收口 `0.1.27` 日常编程闭环之后剩下的 delegated lane 可见性问题。
本版本不新增新的 agent backend，而是让现有 lane 的状态、证据和下一步动作足够
可信，可以用于日常 operator 工作流。

## 目标

- lane 的下一步动作必须匹配真实 operator 生命周期：
  completed -> accept 或 revise，accepted -> apply，applied -> cleanup。
- side screen 里的 lane 状态必须真实，`applied`、`discarded`、`detached`、
  `stopped` 不能继续显示成还在 thinking 的活跃工作。
- 增加稳定的后台 delegated work 计数：active、review、blocked、done。
- lane 证据要能通过 task records、side screens、lane artifacts 和 release smoke
  输出看到。
- 保持 `0.1.27` 的非阻塞日常闭环：provider turn、approval、plan mode 和 lane
  work 都不能锁死 composer 输入。

## 实现范围

- 更新 `agent_tasks` 中的 lane next-action records，让隔离 worktree 的 completed
  lane 必须先显式 accept，再允许 apply。
- 为 side status 增加 delegated lane 后台状态分桶。
- 修正 side-1 详情中 closed operator lane 的状态归一化。
- 增加覆盖 lane 生命周期和 side status 输出的确定性 TUI 回归测试。
- 保持英文和中文 release 文档成对更新。

## 发布 Gate

发布 `0.1.28` 前必须运行：

```bash
cargo test -p viden-cli agent_tasks_separate_completed_accept_and_accepted_apply_lane_actions -- --nocapture
cargo test -p viden-cli side_status_rows_summarize_lane_background_counts -- --nocapture
cargo test -p viden-cli side_lane_rows_render_closed_operator_states_as_done -- --nocapture
scripts/smoke-lane-operator-loop.sh
scripts/tui-turn-controller-smoke.sh
scripts/release-gate.sh --version 0.1.28 --phase prepublish --out-dir /tmp/viden-0128-release-gate
```

prepublish gate 必须包含 live DeepSeek 开发场景，并记录 token、耗时、预估费用
和失败分类证据。

发布后必须运行：

```bash
scripts/release-gate.sh --version 0.1.28 --phase postpublish --out-dir /tmp/viden-0128-release-gate
```

`0.1.28` 只有在以下条件全部满足后才算完成：

- deterministic lane 和 daily-loop 回归测试通过；
- prepublish gate 通过，并包含 live DeepSeek smoke 证据；
- GitHub Release `v0.1.28` 已发布并包含 assets 和 checksums；
- `wikieden/homebrew-tap` 指向 `0.1.28`；
- postpublish validation 通过。
