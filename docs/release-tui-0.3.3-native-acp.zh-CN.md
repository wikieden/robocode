# Viden TUI 0.3.3 Native / ACP 交互

English version: [release-tui-0.3.3-native-acp.md](release-tui-0.3.3-native-acp.md)

TUI `0.3.3` 使用 Core `0.3.4` 不可变检查点
`54965464e87860f9c39a1fb656c2f528e354da94`。Lane identity、agent readiness、
session owner、进程执行、持久化与恢复仍全部由 Core 决定。

## 用户流程

- Normal 模式按 `n` 输入 Viden 原生 Lane 的首个任务。TUI 等待
  `WorkspaceEligibilityUpdated`，再发送 `PreviewDefaultStarterLane`；只有 Core
  依次发出 `StarterLanePreviewed` 和 `StarterLaneCreated` 后才提交任务。
- 先选择 Lane，再输入 `/acp` 打开键盘优先的 ACP 选择列表。已有 session 排在
  Core 发现的 Codex、Claude、Kiro 等 ACP adapter 之前。方向键移动、Enter 选择，
  Esc 会先从任务输入退回列表，再关闭列表。
- `Ready` adapter 进入任务输入；`ProbeRequired` 请求 Core 探测；需要安装、需要认证
  和不可用状态保持可见，但不会启动进程。
- 选择已有 ACP session 后，输入框通过 `SendAgentSessionInput` 续聊；`Ctrl-C` 使用
  Core 发布的精确 owner 发送 `CancelAgentSession`。在失败或取消的 session 行按 `r`
  发送 `RetryAgentSession`。

运行中和等待审批时输入框仍可编辑。中英文文案使用 Core 统一持久化的 locale
偏好；TUI 不另存语言或皮肤配置。
