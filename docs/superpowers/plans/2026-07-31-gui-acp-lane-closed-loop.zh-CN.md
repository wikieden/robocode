# GUI ACP Lane 交互闭环实施计划

本文件是
[`2026-07-31-gui-acp-lane-closed-loop.md`](./2026-07-31-gui-acp-lane-closed-loop.md)
的中文执行摘要。完整的逐步测试命令、接口和文件清单以英文主计划为准。

## 目标

让用户明确选择 Viden 或 Codex Agent，并让 ACP 的启动、运行、完成状态及最终
回复通过 Core 类型化合同进入 GUI，形成可见且可信的交互闭环。

## 执行顺序

1. Agent 默认不选中；探测中、未选择或任务为空时禁止创建。
2. Core 的 `AgentSessionView` 增加向后兼容的可选 `output`，只接收对应
   ACP session 的协议回复。
3. Tauri D1 adapter 投影 `output`，GUI 在当前 Lane 展示 ACP 回复。
4. 当前 ACP session 活跃或完成时，仅屏蔽无关的旧 `agent_stopped`；
   当前 session 失败/取消仍显示恢复状态。
5. 运行 TypeScript、Rust、依赖边界、workspace 测试与 macOS 构建。
6. 在真实 GUI 中选择 Codex 创建 Lane，确认精确回复可见且不再显示
   `AGENT STOPPED`。
