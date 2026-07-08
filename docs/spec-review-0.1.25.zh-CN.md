# Viden 0.1.25 Spec Review

英文版： [spec-review-0.1.25.md](spec-review-0.1.25.md)

最后更新：2026-06-09

## 目的

本 spec review 审查 `0.1.25` 显示稳定性版本与当前代码、文档和 release gates 的一致性。
它沿用 `0.1.24` 的 spec-first 原则：当前行为必须能对应到实现、测试、截图或 smoke
证据；未来能力必须标为 future work。

本版本保留 `0.1.24` 已经落地的非阻塞 operator loop，重点处理可见 TUI 正确性：

- 长时间 idle、focus/sleep 后的重绘稳定性；
- composer 中混入 terminal protocol residue；
- welcome modal 清屏和 popup placement；
- transcript scrollback 和 live activity 文案；
- release 截图证据和 TDD 覆盖。

本版本的 TDD testing contract smoke 是 `scripts/tdd-testing-contract-smoke.sh`。

## 已经稳住的能力

- Provider turn 通过 `TuiRuntime` dispatch，主循环仍可处理键盘、鼠标、resize、
  scroll、approval、streaming 和 cancellation。
- 用户查看历史时，streaming delta 不再抢回底部；transcript label 会显示
  `history N · new output`。
- terminal renderer 已有周期性 full-redraw policy，dirty-row cache 不再假设终端永远保留
  alternate-screen 全部行。
- focus 和 paste 事件对 renderer 可见：它们只触发 repaint，不会变成 composer input。
- composer 会过滤以 `m` 或 `M` 结尾的 terminal SGR residue，覆盖常见 mouse/color
  protocol 尾巴。
- welcome-screen interaction modal 按全屏清理，因为 welcome layout 没有 right rail。

## P0 差异

| 优先级 | 差异 | 代码位置 | 影响 | Spec 目标 |
| --- | --- | --- | --- | --- |
| P0 | manual long-idle terminal acceptance 仍需要真实 Terminal/iTerm2 证据 | `viden-cli/src/tui/terminal.rs`、人工验收 | 自动测试覆盖 redraw policy、focus/paste repaint policy 和 preview output，但真实 terminal 的 sleep/focus 行为会因 emulator 不同而不同 | 在最终 0.1.x zero-bug gate 前补 macOS Terminal 和 iTerm2 的人工截图/记录 |
| P0 | active-turn queue ownership 仍停留在 TUI 层 | `viden-cli/src/tui/state.rs`、`viden-cli/src/tui/app.rs` | queued prompts 已经可见且可保留，但 no-TUI/core queue ownership 尚未正式化 | 0.1.25 保持 UI 行为稳定；后续架构切片再把 durable runtime queue ownership 下沉 |

## P1 差异

| 优先级 | 差异 | 代码位置 | 影响 | Spec 目标 |
| --- | --- | --- | --- | --- |
| P1 | provider doctor/probe 仍有同步命令路径 | `viden-cli/src/tui/app.rs`、`viden-core/src/provider_commands.rs` | 如果未来 doctor 做真实网络探测，同步路径仍可能冻结 UI | doctor/probe 改成 background job，展示 status、tail、evidence 和 cancel |
| P1 | provider capability 差异仍需要完整 adapter matrix | `viden-model/src/providers.rs`、`viden-model/src/adapters.rs` | DeepSeek、DashScope、OpenRouter、Anthropic/OpenAI-compatible 的差异可能泄漏到 UI 和恢复逻辑 | provider descriptor 明确 auth、endpoint、model catalog、stream field、tool semantics、context limit、retry policy 和 error mapping |
| P1 | recent/favorite model 管理仍偏轻 | `viden-cli/src/tui/app.rs` model picker | global `/models` 已不显示未配置 provider，但 recent persistence 和 favorite editing 仍不够完整 | 持久化 recent model choices，并提供不重复的 favorite toggle |

## P2 差异

- 历史文档和截图里仍可能出现旧的 `DeepSeek is thinking` 文案。新的 TUI 文案应使用
  Viden 或内部角色，例如 `Viden is planning`。
- release preview set 已是确定性的，但最终 0.1.x zero-bug gate 仍应加入真实终端截图验收。

## 验收门禁

- `cargo fmt --check`
- `scripts/tdd-testing-contract-smoke.sh`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/tui-turn-controller-smoke.sh`
- `scripts/tui-regression.sh docs/previews/generated`
- `scripts/plan-mode-smoke.sh /tmp/viden-0125-plan-mode-smoke`
- `scripts/daily-loop-smoke.sh /tmp/viden-0125-daily-loop-smoke`
- `scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash`
- `scripts/release-gate.sh --version 0.1.25`
- `scripts/release-gate.sh --version 0.1.25 --phase postpublish`

## 发布决策规则

只有自动化 release gates 全部通过、确定性 TUI screenshots 以 `0.1.25` 命名重新生成、
release status 记录 DeepSeek token/费用证据，并且 GitHub Release 和 Homebrew validation
都通过后，`0.1.25` 才能算发布完成。
