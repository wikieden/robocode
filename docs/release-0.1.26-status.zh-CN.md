# RoboCode 0.1.26 状态 - TUI 回归包与模式稳定性

英文版： [release-0.1.26-status.md](release-0.1.26-status.md)

`0.1.26` 收口 TUI 可见层剩余的 Mode / Permission 工作，并把 active-turn 输入闭环做成
release gate。它也继续把真实 DeepSeek 开发 smoke 作为发布就绪的强制证据。

## 状态

- Workspace version：`0.1.26`
- Git tag：`v0.1.26`
- GitHub Release：等待发布
- Homebrew tap：等待同步发布

## 已实现

- `RuntimeSnapshot` 已经把 work mode 和 permission level 带入 TUI 状态。
- 顶栏和 composer 渲染真实 mode/permission，不再显示静态 `Build` / `Ask` 占位。
- `/plan on` 在同一轮命令里把可见 TUI 状态同步为 `Plan` / `Read Only`。
- active provider turn 期间，普通文本会作为下一条 prompt 排队；slash command 不会混入
  prompt 队列；`/cancel`、`/stop`、`/interrupt` 或 `/abort` 会请求取消。
- active-turn composer actions 会切换到 queue/cancel/history。
- TUI preview 检查改为断言 `LIVE WORK`、`input open`、queue 和 cancel 信号，不再依赖旧的
  单一句 thinking 文案。

## 验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo test -p robocode-cli --quiet`：通过，`283` 个测试
- `cargo test --workspace --quiet`：通过
- `scripts/tdd-testing-contract-smoke.sh`：通过
- `scripts/tui-turn-controller-smoke.sh`：通过
- `scripts/plan-mode-smoke.sh /tmp/robocode-0126-plan-mode-smoke`：通过
- `scripts/daily-loop-smoke.sh /tmp/robocode-0126-daily-loop-smoke`：通过
- `scripts/tui-regression.sh docs/previews/generated`：通过
- `scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash --out-dir /tmp/robocode-0126-deepseek-dev-smoke`：通过

## DeepSeek Smoke 证据

- Provider/model：`deepseek / deepseek-v4-flash`
- 场景：`python_add_module_with_test`
- 请求：`3` ok / `0` err
- Token：input `11382`，output `529`，total `11911`
- 估算费用：`¥0.012440 CNY`
- 计价依据：DeepSeek cache-miss estimate，input `¥1/1M`，output `¥2/1M`
- 证据：`/tmp/robocode-0126-release-gate/prepublish/deepseek-dev-scenario/summary.md`

## 发布完成门禁

`0.1.26` 只有在以下项目全部完成后才算完成：

- `scripts/release-gate.sh --version 0.1.26 --phase prepublish` 通过；已完成，
  证据在 `/tmp/robocode-0126-release-gate/prepublish`；
- GitHub Release `v0.1.26` assets 发布；
- `wikieden/homebrew-tap` 同步到 `0.1.26`；
- `scripts/release-gate.sh --version 0.1.26 --phase postpublish` 通过。
