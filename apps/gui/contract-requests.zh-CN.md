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

Core 状态：已在 `core-v0.3.2`（提交待定）交付
`RuntimeCommand::DecideReview { review_id, verdict, feedback, actor }`。只有独立
评审 Lane 可以做出决定；该结论结算评审事实，接受时为 gate validator 打上
`validated_at`；已结算的接受结论不会被后续 gate 决定覆盖；评审被驳回后
`AcceptMergeGate` 失败关闭。`ReviewRequestRecord` 新增可选且向后兼容的
`feedback` 字段。schema-1 扩展 fixture `review-decision.json` 证明
`Pending -> Accepted` 迁移。GUI 接线仍未完成：在客户端发送 `DecideReview` 之前，
D2 仍以该编码把接受/驳回动作置为禁用。

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

## GUI-CORE-018：检查点的捕获与恢复

D6 渲染了「恢复检查点」这一恢复动作，但 schema 1 完全没有建模检查点：没有任何
`RuntimeCommand` 能捕获或恢复检查点，`RuntimeViewState` 中没有检查点记录，也没有
事件报告恢复结果。其余 D6 动作现在都已接到真实 Core 命令——`restart` 发送
`RetryAgentSession`，`close_lane` 发送 `StopLane`——因此 `checkpoint` 成为唯一背后
无物的动作。

GUI 将其投影为 `available: false`、code 为 `GUI-CORE-003`，且不挂载任何处理器。它不
得用重放模拟恢复、不得把会话回退到更早的游标，也不得把重新读取快照伪装成检查点
恢复。

当 Core 发布带稳定 id 与归属 owner 的类型化检查点记录、提供恢复命令、发出携带恢复
结果的事件，且规范 `frontend-contract-v1` fixture 覆盖一次「捕获后恢复」时，关闭此
请求。

## GUI-CORE-019：Always 审批作用域与 Edit 决定

`ApprovalScope` 只建模了 `Once`、`Session` 与 `RepoAllowlist`。权限坞的设计还提供
「Always」与「Edit」：Always 是跨会话、跨仓库的长期决定，Edit 则返回一条经修改的
命令重新走审批，而不是接受或拒绝原提案。二者在 schema 1 中都不存在，因此都渲染为
fail-closed 的 `GUI-CORE-003` 占位，且 `PermissionChoice::Always` 与
`PermissionChoice::Edit` 在构造任何命令之前就被拒绝。

这也让一处键盘分歧保持开启。设计把 `Shift+A` 指派给「Always」；GUI 仍将 `Shift+A`
绑定在 `repo_allowlist`——Core 实际接受的最宽作用域——而不是把可用快捷键绑到一个失效
动作上。

当 Core 建模持久的 Always 作用域及使其可安全授予的撤销路径、建模能让修改后的命令
重新通过同一审批门禁的 Edit 决定，且规范 fixture 覆盖两者时，关闭此请求。届时 GUI
将恢复设计规定的 `Shift+A` 绑定。

## GUI-CORE-020：面向操作者的 git 动作

驾驶舱标题栏现在可以显示工作区源码管理的样子——分支、ahead/behind、dirty——因为
Core 会从工作区根采样 `WorkspaceSourceView` 并发布在
`RuntimeViewState.workspace_source` 上。但操作者无法对它做任何事。`RuntimeCommand`
没有建模 commit、push、pull、sync、fetch、stage 或切换分支；`crates/tools` 中面向
模型的 Git 工具是 `pub(crate)`：只有走权限门禁的 agent 轮次能触达它们，其他任何路径
都不能——这正是正确的边界，前端不得直接调用工具。

因此设计稿的 sync chip 以 `role=status` 元素而非按钮发布，设计稿的「提交或推送」
入口则完全不做。GUI 不得 shell out、不得驱动自己编造的工具调用，也不得呈现一个
落不到实处的 git 动作。

自然的接缝是 runtime 已经拥有的那一处：`LaneEffectExecutor`
（`crates/runtime/src/lane_runtime.rs`）上的类型化 effect，让操作者发起的 git 动作
经过同一道权限门禁、产出同样的证据，并落入同一份 append-only 会话事实。

当 Core 发布带逐命令权限门禁的类型化操作者 git 命令、发布携带每次结果（含拒绝与
冲突）的有序事件，且规范 `frontend-contract-v1` fixture 覆盖一次被拒与一次被接受的
动作时，关闭此请求。届时 GUI 会把 sync chip 升级为真实控件，并补上设计稿的
提交/推送入口。

## GUI-CORE-021：Pull request 与 forge 状态

设计稿的标题栏与 Lane 界面带有一行「Pull request status」：分支是否有开启的 PR、
其评审状态与检查结果。schema 1 完全没有建模 forge——没有 remote、没有 pull request
记录，也没有来自工作区之外的评审或检查状态。`CheckRunView` 是本地检查运行器，不是
forge 的 CI；`MergeGateView` 是 Viden 自己的门禁，不是远端的合并状态。

因此 GUI 不渲染 PR 行，也不渲染 forge 徽标。它不得从分支名推导 PR、不得用
`WorkspaceSourceView` 的 ahead/behind 计数推断 remote，也不得直接调用 forge API：
访问网络服务的前端已在客户端边界之外，而 forge 凭据属于 Core 已经拥有的那条凭据
路径。

当 Core 发布类型化的 forge 状态记录——Lane 分支与远端 pull request 的关联、其评审
状态与检查结果——连同使拉取安全的凭据与数据外发策略，且规范
`frontend-contract-v1` fixture 覆盖「有 PR 的分支」与「无 PR 的分支」时，关闭此请求。

## GUI-CORE-022：工作区文件清单

命令面板移植了 TUI jump 索引的 `~` 选择器，而 TUI 本身也有同样的缺口：
`RuntimeViewState` 承载 Lane、会话、合并闸与审批，却没有工作区文件的清单。既没有
类型化的路径列表，没有搜索索引，也没有任何一次读取能让前端在不自己遍历文件系统的
前提下枚举目录树——而自行遍历已在客户端边界之外，还会绕过管辖其余所有路径读取的
权限门禁。

因此 GUI 把「文件」分区渲染成恰好一行、点名本请求的永久禁用行，与
`apps/tui/src/tui/jump.rs` 渲染的那一行「诚实禁用」保持一致。GUI 不得遍历工作区、
不得 shell out 调用文件列举工具，也不得从证据记录或工具入参预览里偶然出现的路径拼
出一棵树：把不完整的清单当成清单呈现，比明说缺口更糟。

当 Core 发布类型化的工作区文件清单——路径列表连同 Core 自己拥有的作用域与忽略规则，
并与其他工作区读取走同一道权限门禁——且规范 `frontend-contract-v1` fixture 覆盖
「有清单」与「无清单」两种项目时，关闭此请求。届时 GUI 会在面板中启用 `~` 作用域，
TUI 也会启用同一选择器。

## GUI-CORE-023：并发多工作区托管

Core 一次只托管一个工作区。`LocalCoreHost::open_workspace`
（`crates/core/src/host.rs:166-234`）每次调用都会新建一个 `RuntimeSupervisor`，
而桌面宿主只是替换它那唯一的 `Mutex<Option<GuiCoreAdapter>>` 槽位
（`apps/gui/src-tauri/src/lib.rs:79-94`）。因此打开一个项目会**替换**当前打开的
项目：旧 supervisor 被 drop 时（`crates/runtime/src/runtime_supervisor.rs:1373-1404`）
会 join 其工作线程并关闭所有常驻 ACP 会话，旧工作区里正在运行的每条 Lane 与
Agent 都会停止。

这与设计稿画的不一致。`WorkspacePanel` 渲染了多个 `.wsroot` 项目分组以及一个跨项目
的「Global」lane 分区，`ProjectPicker` 的「工作区内」一列带项目计数和每项目 lane
计数，D13 展示的是跨项目的舰队视图。在单根 supervisor 之上，这些都无法表达。

在 Core 发布多根托管之前，GUI 守住以下边界：

- 侧栏只渲染一个分组——当前打开的项目——不伪造同级项目，也没有「Global」分区；
- 选择器的「工作区内」一列只有一行，标记为当前项且不可点击，因为工作区内部没有可
  切换的目标；
- 其他任何项目都是一次**切换**，必须先经过内联确认，确认文案点名将被拆除的正在运行
  的 Lane 与 Agent 会话数量；
- `克隆仓库…` 与 `新建空项目` 渲染为禁用并点名本请求：`frontend-contract-v1` 没有
  发布仓库克隆命令，也没有项目脚手架命令，而 GUI 不得自行 shell out 调用
  `git clone` 或写出项目骨架——这两者都是绕过 Core 权限门禁的变更。

当 Core 发布以下内容时，关闭此请求：

1. **N 根托管**——同时托管多个工作区，`open_workspace` 变为叠加语义（或获得显式的
   替换标志），而不是静默替换；
2. **项目注册表**——把已挂载的根集合作为 `RuntimeViewState` 中类型化、有序的事实
   发布，使侧栏分组来自 Core 而不是来自单一的 `environment.cwd`；
3. **跨项目 Lane 枚举**——可跨已挂载根读取 Lane、会话、闸门与审批事实，这正是设计稿
   的「Global」分区与 D13 舰队看板真正展示的内容；
4. **`RuntimeOwner.project_id` 推导规则**——从规范根到 `RuntimeOwner` 已携带的
   `project_id` 的稳定且有文档的映射，使客户端无需按路径做字符串匹配即可把
   owner 绑定事实归属到项目；
5. **项目初始化命令**——把「克隆进工作区」与「脚手架新建项目」作为 Core 命令发布，
   并与其他所有变更走同一道权限门禁，这才是解锁两行禁用选项的前提；
6. **规范 `frontend-contract-v1` fixture**，覆盖单根与双根两种情况，让分组侧栏与跨
   项目舰队拥有生成的证据而非手写投影。

最近工作清单（`runtime.recent_work`）**不是**这个缺口：它已经能让客户端列出可以打开
的项目。缺的是同时打开多个项目的能力。
