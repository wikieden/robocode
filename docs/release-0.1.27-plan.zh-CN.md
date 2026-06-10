# RoboCode 0.1.27 计划 - 日常编程闭环加固

English version: [release-0.1.27-plan.md](release-0.1.27-plan.md)

`0.1.27` 收口真实编程过程中最影响体验的交互可靠性问题。本版本不扩张新的
agent 能力，重点是让主编程闭环真实、响应及时、可测试。

## 目标

- Runtime 状态不能只是静态装饰。顶栏、输入框和底部状态栏都读取真实
  `RuntimeSnapshot` 的 mode 和 permission。
- `/mode plan`、`/mode build`、`/permissions ask` 在同一个命令回合内立刻
  更新 TUI。
- provider turn、plan turn、工具执行和 approval 弹窗期间，composer 仍然能
  输入、排队、取消和滚动历史。
- 活跃工作提示在完成后清理干净，不能残留过期的 `planning` 或 `thinking`
  行。
- 发布 smoke 同时包含确定性的 daily-loop 测试，以及一次真实 DeepSeek 开发
  场景，记录 token、耗时、费用和失败分类证据。

## 必做实现

- 增加完整 TUI controller 回归：
  `prompt -> streaming -> approval -> write_file -> queued follow-up -> final`。
- approval 请求可见时，普通 composer 输入不能被吞掉；approval 快捷键仍然能
  正常确认或拒绝。
- 顶栏静态状态文案改成真实活动状态，例如 `idle`、`working`、`approval`、
  `check`。
- 底部状态栏展示真实 work mode 和 permission level。
- `scripts/tui-turn-controller-smoke.sh` 作为快速 gate，覆盖输入、approval、
  queue、mode、permission、scrollback 和 stale-status 回归。

## 验证 Gate

发布 `0.1.27` 前必须运行：

```bash
scripts/release-gate.sh --version 0.1.27 --phase prepublish
```

gate 必须包含：

- `cargo fmt --check`；
- `cargo clippy --workspace --all-targets -- -D warnings`；
- focused TUI turn-controller smoke；
- TUI regression preview；
- plan-mode smoke；
- daily-loop smoke；
- 带 token 和费用摘要的真实 DeepSeek development smoke。

GitHub assets 发布并同步 Homebrew 后运行：

```bash
scripts/release-gate.sh --version 0.1.27 --phase postpublish
```

## 完成标准

`0.1.27` 只有在以下条件全部满足后才算完成：

- prepublish gate 全部通过；
- GitHub Release `v0.1.27` 已发布并包含 assets 和 checksums；
- `wikieden/homebrew-tap` 指向 `0.1.27`；
- postpublish Homebrew 和 GitHub asset 验证通过；
- status 文档记录 DeepSeek token、预估费用、证据路径，以及观察到的失败分类。
