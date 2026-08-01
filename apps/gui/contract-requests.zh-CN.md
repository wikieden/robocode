# GUI Core 契约请求

英文版：[contract-requests.md](contract-requests.md)

## GUI-CORE-008：所选 Lane 的上下文作用域

Core `0.3.5` 已暴露 `RuntimeViewState.context_budgets`，但 frontend-neutral
的 `viden-core` facade 尚未重新导出 `ContextBudgetRecord` 与
`ContextScope`。因此，GUI 无法在不重建私有序列化 schema 的前提下证明某个
budget 属于所选 Lane 的 task。

在 Core 通过 `viden-core` 导出 typed scope/budget 契约前，D1 将
`contextDock.context` 投影为 `null`。GUI 不得任意选择 budget、反序列化猜测的
scope 形状，也不得从展示文本推断用量。

当 facade 导出所需 frontend-neutral 类型，且规范 D1 fixture 覆盖两个具有不同
task-scoped budget 的 Lane 后，可关闭此请求。

## GUI-CORE-009：按 Owner 范围限定的类型化转录行

前端契约仅将 Lane 输出暴露为未类型化流，并暴露全局 assistant 流；它没有提供
按 Owner 范围限定且有序的 user/assistant 转录序列。因此 D1 仅为选中的精确
Owner 渲染类型化 Lane 输出，并将 user/assistant 行明确标为不可用；不得从展示
文本推断角色。

当 Core 发布包含稳定行 id、完整 `RuntimeOwner`、类型化 `user`/`assistant` 角色、
内容或不可变内容引用以及 replay/分页 cursor 的有序转录行时，关闭此请求。规范
D1 fixture 必须证明两个 Lane 的行不会跨 Owner 泄漏。

## GUI-CORE-010：按 Owner 范围限定的实时工作事实

在 frontend-contract-v1 中，`AgentTaskRecord`、活动工具调用、排队输入和证据视图
都不携带 `RuntimeOwner`。D1 不会依据时序或标签把这些全局事实归属给选中的 Lane，
而是省略它们。

当每一项实时工作事实都携带完整 `RuntimeOwner`，且规范 D1 fixture 证明两个并发
Owner 的选中 Lane 投影时，关闭此请求。
