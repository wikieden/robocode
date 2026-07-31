# GUI ACP Lane 交互闭环规格

## 问题

D1 的新建 Lane 弹层会在 ACP Agent 仍处于探测阶段时默认选择 Viden
原生 Agent。此时点击被禁用的 Codex 选项不会产生效果，提交任务会静默创建
原生 Lane。另一方面，ACP session 已成功完成时，界面仍可能被旧的全局
`agent_stopped` 恢复页面覆盖，而且当前 GUI 类型化投影没有 ACP 回复正文。

## 已确认行为

1. 新建 Lane 默认不选择任何 Agent。
2. 用户必须明确选择 Viden Agent 或已就绪的 ACP Agent。
3. Agent 探测中、未选择 Agent 或任务为空时，禁止创建 Lane。
4. 创建操作明确显示当前选择的 Agent。
5. 一个 Lane 始终只绑定一个 Agent。
6. 当前选中 ACP session 处于 `starting`、`running`、`waiting_approval`
   或 `completed` 时，不得被无关的旧 `agent_stopped` 页面覆盖。
7. Core 将最近一次完成的 ACP 助手回复作为可选、带 owner 的 session
   事实发布，GUI 在对应 Lane 中展示。
8. ACP session 失败或取消时继续显示明确的状态/恢复界面，不得伪装成功。

## 合同

`AgentSessionView` 增加带 serde 默认值的可选 `output` 字段。这是
`frontend-contract-v1` 的向后兼容扩展；旧快照缺少该字段时解析为不存在。
ACP runtime 只使用同一个 Core owner session 的协议
`AgentMessageChunk` 内容填写此字段。

D1 adapter 将可选字段映射到 `D1AgentSessionProjection`。Web 客户端不得
直接读取 `.viden` JSONL/result 文件，也不得从诊断或展示文本推断回复。

## 验收证据

- 组件测试：默认无选择；探测完成且明确选择就绪 Agent 后才能创建。
- D1 测试：选择 Codex 后发送带 `codex-acp` 的 `start_agent_session`。
- Core 测试：ACP 完成事件发布回复；缺少 `output` 的旧 session JSON 仍可解析。
- 投影测试：当前 ACP 输出进入 GUI 投影。
- D1 测试：完成的 ACP 回复可见，旧 `agent_stopped` 不再覆盖。
- macOS 真机测试：创建 Codex Lane，并在 D1 中看到其精确回复。

