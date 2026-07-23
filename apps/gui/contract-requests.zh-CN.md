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
