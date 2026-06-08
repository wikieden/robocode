# TUI Stability Zero-Bug Gate

英文版： [tui-stability-zero-bug-gate.md](tui-stability-zero-bug-gate.md)

最后更新：2026-06-07

## 目的

0.1.x 的最后阶段必须把 TUI 稳定性作为最高优先级。RoboCode 不能带着已知显示错位、
输入卡死、弹窗残影、状态漂移或 scrollback 错误进入 0.2.x。

这里的 “0 bug” 定义为：

- 已知 P0/P1 TUI 显示和交互 bug 为 0；
- 所有用户可见 TUI 功能都有截图或 deterministic preview 证据；
- 所有 release-blocking TUI regression 都有可复跑测试；
- P2 视觉瑕疵必须记录、降级说明清楚，且不影响日常开发闭环。

## Bug 分级

### P0：必须立即阻断 release

- 输入被锁死，用户无法继续输入、退出或取消。
- provider turn、Plan 模式、approval、doctor、lane、context build 或 tool job 卡住主 UI。
- approval 弹窗无法操作、无法消失或批准/拒绝后状态错误。
- resize 后布局不可用、边框错位导致内容不可读。
- streaming 抢 scrollback，用户无法查看历史。
- 状态栏、right rail、side screen 显示虚假数据或 stale blocking 状态。
- 真实任务完成后仍显示 running/thinking/waiting approval。

### P1：必须在 0.1.x final 前清零

- 光标位置错误、IME 候选框明显偏离输入区。
- command palette、provider/model picker、settings/setup modal 遮挡输入或脱离 composer。
- 主屏和 side panels 对同一个任务显示不同状态。
- 边框、竖线、分隔线、颜色在常见终端尺寸下明显错位。
- welcome/config flow 跳转错误：配置 provider/model 后不该进入 cockpit 却进入了，或真实任务开始后仍停留 welcome。
- 错误提示过于突兀、覆盖主要内容，且没有明确 recovery action。

### P2：可以带记录进入后续版本

- 不影响可读性的轻微 spacing 差异。
- 少数不常见 terminal/font 下的非阻断视觉偏差。
- 不影响工作流判断的低优先级文案 polish。

## 0.1.x Final 退出标准

在宣布 0.1.x 最后一版完成前，必须满足：

- P0/P1 TUI bug backlog 为 0。
- `scripts/tui-regression.sh docs/previews/generated` 通过。
- `scripts/plan-mode-smoke.sh` 通过。
- `scripts/daily-loop-smoke.sh` 通过。
- fake slow provider non-blocking TUI smoke 通过。
- deterministic approval non-blocking smoke 通过。
- streaming scrollback smoke 通过。
- provider/model setup smoke 通过。
- 至少覆盖 macOS Terminal 和 iTerm2 的人工截图验收。
- 每个核心 TUI 状态都有截图证据：welcome、main idle、thinking/streaming、
  approval、provider setup、model picker、command palette、side-1、side-2、
  error recovery、resize 后布局。
- release status 明确列出 TUI bug backlog、截图路径、失败用例、已知 P2 和剩余风险。

## 建议版本节奏

- `0.1.24`：启动非阻塞主循环 gate。解决 Plan 模式、approval、streaming、provider turn
  卡输入的根因。
- `0.1.25`：TUI display cleanup。集中清理边框、竖线、颜色、IME、cursor、modal
  position、right rail drift 和提示框位置。
- `0.1.26`：TUI regression pack。把所有历史显示 bug 做成 deterministic preview、
  terminal smoke 或人工截图 checklist。
- `0.1.27`：Daily coding loop hardening。用真实开发任务验证输入、审批、测试、diff、
  error recovery、scrollback 和 provider setup。
- `0.1.28`：Delegated lane visibility cleanup。确保 side screens、lane evidence、
  Codex/Claude/shell job 状态一致且不假显示。
- `0.1.29`：0.1.x RC stabilization。停止扩大新 UI surface，只修 P0/P1 TUI bug。
- `0.1.30`：0.1.x final zero-bug gate。P0/P1 清零后才允许进入 0.2.x spec/context/evidence runtime。

## 执行规则

- 0.1.x 后半段不再因为“新 agent surface”牺牲 TUI 稳定性。
- 每个 TUI bug 必须有复现步骤、影响等级、截图或 transcript、修复 PR/commit、验证证据。
- 每次修 bug 必须优先走 TDD：先补能复现问题的测试或 deterministic preview，再改实现。
- 如果 bug 无法自动化，必须补人工验收 checklist 和真实终端截图。
- 不能把“已知显示错误”标成 polish；只要影响用户判断、输入、审批、滚动或状态理解，就是 P0/P1。
