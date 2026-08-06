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

## GUI-CORE-011：评审决定命令

`frontend-contract-v1` 发布了带 `Pending` 状态的 `ReviewRequestRecord`，也提供
`RuntimeCommand::RequestReview`，但没有任何命令用于记录评审决定。因此 D2 会列出
待处理评审及其 Core 证据，并把接受/驳回动作以该编码置为禁用；不得用
`AcceptLaneOutput` 或审批响应冒充评审结论。

当 Core 发布携带评审 id、结论、可选反馈与审计 id 的评审决定命令，且规范 fixture
证明 `ReviewRequestStatus` 的状态迁移时，关闭此请求。

## GUI-CORE-012：审批的结构化决策上下文

`ApprovalRequestView` 只携带 `input_preview` 这一不透明展示字符串。D2 设计稿要求
按行渲染待执行变更的 diff。D2 原样渲染该预览并声明 diff 不可用，而不是把展示文本
解析成 diff 行。

当 Core 发布审批的类型化决策上下文（带文件路径、行号与变更类型的有序 hunk，或
客户端可解析的不可变内容引用），且规范审批 fixture 覆盖多文件变更时，关闭此请求。

## GUI-CORE-013：待确认契约事实

`ContractRecord.decision` 只有 `Confirmed` 与 `Rejected`，因此发布出来的契约都已
决定。D2 设计稿展示的是等待人确认的契约队列。D2 把契约记录列为已决历史并给该分组
打上此编码；不得把已决记录当成待办积压。

当 Core 发布携带提议方、目标契约版本、订阅方与审计 id 的待确认契约事实，且规范
fixture 证明待确认契约经 `ConfirmContract` 转为已决时，关闭此请求。

## GUI-CORE-014：视图状态中的有序事件日志

`RuntimeViewState` 只发布当前事实，没有有序事件日志。D10 设计稿展示跨项目的书记官
汇总事件流，D14 也需要同一份有序历史。D10 不渲染任何 ticker，以该编码声明缺口；
不得通过比对相邻快照重建时间线。

当 Core 发布有界、有序、按 Owner 限定的事件日志（或客户端可分页的 replay cursor），
携带稳定事件 id、类型、Owner 与时间戳，且规范 fixture 证明跨两个项目的顺序时，
关闭此请求。

## GUI-CORE-015：结构化合并冲突内容

`MergeGateRecord` 与 `ConflictBounce` 给出闸、原 Lane 与理由，但不携带冲突内容。
D12 设计稿要求并排展示两条 Lane 的 hunk 与冲突标记。D12 只渲染 Core 的理由文本并
声明 hunk 不可用；不得读取 worktree，也不得把理由字符串解析成 diff 行。

当 Core 为被退回的闸发布结构化冲突内容（文件路径、带行号的 ours/theirs hunk、以及
计算冲突所依据的基线），且规范 merge-gate fixture 覆盖单文件两 Lane 冲突时，
关闭此请求。

## GUI-CORE-016：Agent 消息的流式分片

ACP 适配器已经收到 `agent_message_chunk` 更新，但
`crates/runtime/src/agent_commands.rs` 只是把它们累加进一个局部字符串，在轮次结束
时发布单条 `AgentConversationMessageView`。没有任何有序事件承载部分消息，因此 GUI
无法边生成边渲染，只能显示已完成的整段。

D1 因此按整条消息渲染，并用工作状态条表达「仍在进行」。不得对一条已完成的消息伪造
打字机效果。

当 Core 发布携带会话 id、所属消息 id、追加文本与终止标记的有序分片事件，且规范
fixture 证明重放分片可精确重建最终消息时，关闭此请求。

状态：Core 已实现，fixture 仍缺。`AssistantDelta` 现在携带可选会话 id，ACP 适配器在
整个提示轮次内保持同一个消息 id，reducer 因此增长单条归属明确的消息。crate 测试证明
重放分片可精确重建回复，且完成事件不会重复它。在规范 `frontend-contract-v1` fixture
覆盖一次流式轮次之前，此请求保持开启。

## GUI-CORE-017：Agent 消息的非文本内容

`AgentConversationMessageView.content` 是单个 `String`，且 `acp_message_chunk_text`
只提取 `content.type == "text"`。因此当 ACP agent 返回图像块时，到达客户端的只是一段
声称「图已画好」的文字，背后没有任何图像事实——这正是操作者看到的「agent 说画了，
但什么都没有」。

D1 只渲染 Core 发布的文本，不合成附件。

当会话消息携带类型化内容分部（文本，加上带媒体类型与客户端可解析的不可变内容引用的
图像/文件分部），且规范 fixture 覆盖一次返回图像分部与文本的 ACP 轮次时，关闭此请求。

状态：Core 已实现，fixture 仍缺。会话消息现在携带类型化内容分部，`AgentMessagePart`
把分部挂到它所属的消息上；内联字节按内容摘要写入 `.viden/agents/parts/`，因此引用不可
变，字节同时作为证据发布。Core 未建模的分部类型无损往返，而不是被丢弃。桌面外壳通过
`agent_content` 命令解析工作区引用——webview 无法直接打开工作区路径——并拒绝 parts 目录
之外的任何引用。在规范 `frontend-contract-v1` fixture 覆盖一次返回图像分部与文本的 ACP
轮次之前，此请求保持开启。
