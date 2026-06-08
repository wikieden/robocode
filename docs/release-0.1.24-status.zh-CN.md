# RoboCode 0.1.24 状态 - Provider 设置与非阻塞 Operator Loop

英文版： [release-0.1.24-status.md](release-0.1.24-status.md)

`0.1.24` 是 provider 设置与非阻塞 operator loop 版本。它保留 `0.1.23`
里接近 opencode 风格的 `/connect` 和 `/models` 设置体验，并把 live provider turn
移动到 TUI runtime worker 后面，让主事件循环继续负责重绘、输入、审批、取消和排队 prompt
恢复。

## 发布状态

- Workspace version：`0.1.24`
- Git tag：待发布
- GitHub release：待发布
- Release workflow：待运行
- Homebrew tap commit：待同步
- Prepublish evidence：`/tmp/robocode-0124-release-gate/prepublish`
- 本地 package：`dist/robocode-v0.1.24-aarch64-apple-darwin.tar.gz`
- 本地 package sha256：
  `5a9bd29040f071a0a4f623a9b9c9795ab8229025b13d04857461a2a9bd952a1b`
- Post-publish evidence：等待 GitHub assets 与 Homebrew 验证

## 已包含改动

- Provider turn 现在通过 `TuiRuntime` worker 派发，不再接管 TUI 输入循环。
- TUI 主循环通过同一条 controller event 路径接收 streaming delta、approval prompt、
  cancel signal、finish event 和 provider error。
- Plan mode 和 live provider turn 结束或失败后，不再把 composer 留在锁死状态。
- Active-turn queued input 通过共享 `AgentTask` projection 可见，包括排队 prompt 数量和
  next action 文案。
- Provider approval 由主事件循环处理，旧的嵌套 active-turn approval loop 已移除。
- Active turn 失败时 TUI 保持打开，恢复第一个排队 draft，并把剩余排队 prompt 保留在可见状态。
- `/connect` 仍是 provider 连接选择器；`/models` 仍是按 provider 分组的 active model
  选择器。
- 确定性 TUI regression preview 已重新生成，并使用 `0.1.24` 截图文件名。

## 验证

Focused 检查：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-turn-controller-smoke.sh
scripts/plan-mode-smoke.sh /tmp/robocode-0124-plan-mode-smoke
scripts/daily-loop-smoke.sh /tmp/robocode-0124-daily-loop-smoke
scripts/tui-regression.sh docs/previews/generated
```

Release gate：

```bash
scripts/release-gate.sh --version 0.1.24
```

结果：2026-06-08 prepublish 通过。证据目录：
`/tmp/robocode-0124-release-gate`。

Prepublish smoke 结果：

- `cargo-fmt`：通过
- `tdd-testing-contract-smoke`：通过
- `tui-turn-controller-smoke`：通过
- `cargo-clippy`：通过
- `robocode-cli-tests`：通过
- `workspace-tests`：通过
- `tui-regression`：通过
- `fallback-cli-smoke`：通过
- `plan-mode-smoke`：通过
- `daily-loop-smoke`：通过
- `codex-app-server-protocol-fixture`：通过
- `codex-app-server-write-guard`：通过
- `lane-operator-loop-smoke`：通过
- `package-smoke`：通过
- `deepseek-dev-scenario-smoke`：通过

DeepSeek 真实开发场景：

- Provider/model：`deepseek` / `deepseek-v4-flash`
- 场景：`python_add_module_with_test`
- 请求：`3` 次成功，`0` 次错误
- Token：input `11021`，output `427`，total `11448`
- 估算费用：`¥0.011875 CNY`
- 证据：`/tmp/robocode-0124-release-gate/prepublish/deepseek-dev-scenario`

发布后验证：

```bash
scripts/release-gate.sh --version 0.1.24 --phase postpublish
```

## 截图证据

确定性的 0.1.24 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-setup-wizard.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-provider-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-lane-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.24-tui-side-2.svg`

结构化 TUI 证据：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/tui-regression-evidence.json`

## 剩余风险

- Durable active-turn queue ownership 仍在 TUI 层。`0.1.24` 通过 `AgentTask` projection
  让 queue 状态可见，但真正 UI-agnostic 的 core/runtime queue 仍是后续工作。
- Live provider 行为仍依赖账号、模型和上游 provider 可用性。prepublish gate 已通过真实
  DeepSeek development smoke，但发布用户仍可能遇到账号、额度或 provider-specific 错误。
- GitHub release assets 和 Homebrew formula 验证都必须通过，才能把本版本视为完成。
