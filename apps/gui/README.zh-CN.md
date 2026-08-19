# Viden GUI

英文版：[README.md](README.md)

本目录是 Viden 的 GUI 实施线。Alpha 证据门禁已经选择 Tauri，
`0.1.0-rc.3` 使用 Core `0.3.5` 的同状态 fixture 认证规范 D1 驾驶舱。
`0.1.0-beta.1` 建立唯一 production desktop bootstrap。原生启动器会通过
frontend-safe `LocalCoreHost` 打开工作区，并把它的 `CoreClient` 注入
`GuiCoreAdapter`。应用始终先显示 D1 驾驶舱外壳。未绑定工作区时，“打开项目”只会
打开系统文件夹选择器，并通过 `LocalCoreHost::open_workspace` 重绑；它不会进入 D11，
也不会要求选择模型。已打开但没有 Lane 的项目仍显示项目驾驶舱及“新建 Lane”，
并打开 D1 的 New Lane 弹层，用于快速启动原生或 ACP Lane；精确 Core Lane receipt
会把焦点带回 D1。

## 本地运行桌面客户端

进入 `apps/gui`，安装锁定版本的前端依赖并启动原生 Tauri 开发窗口：

```bash
npm ci
npm run tauri -- dev
```

Vite 与 Tauri 固定共用 `http://localhost:1420`；如果端口被占用，启动会直接失败，
不会静默切换到其他端口。只有显式设置 `VIDEN_GUI_WORKSPACE` 时，原生 bootstrap 才会
预先绑定工作区；否则 D1 欢迎页使用系统文件夹选择器。真正的 Core 启动失败才显示明确
D6 断开状态。

macOS 仍保留原生红黄绿窗口按钮，但标题栏使用 overlay，由 HTML 外壳提供深色可拖拽区域；
不再渲染原型窗口边框、圆角内框或白色原生标题条。

构建并打开不依赖 Vite 开发服务的独立 macOS debug App：

```bash
npm run tauri -- build --debug --bundles app
open ../../target/debug/bundle/macos/Viden.app
```

直接运行 binary 时，可以显式绑定项目：

```bash
VIDEN_GUI_WORKSPACE=/absolute/project/path \
  ../../target/debug/bundle/macos/Viden.app/Contents/MacOS/viden-gui
```

桌面宿主会在 Core bootstrap 前，将实际存在的标准用户工具目录（如 `~/.local/bin`、Bun、
Volta、asdf、mise/fnm 与 Homebrew）加入继承的 `PATH` 前部。这样从 Finder 打开 App 时，
Agent 可用性探测与后续 ACP spawn 使用同一命令路径；整个过程不执行 login shell，也不写死
当前机器的绝对路径。

## 冻结输入

| 字段 | 值 |
| --- | --- |
| GUI 组件版本 | `0.1.0-rc.3` |
| 最低 Core 版本 | `0.3.5` |
| 支持 frontend schema | `[1]` |
| 共同分支基线 | `3a7740ea72e58f4a22248a80f9e7324c49bb0f73` |
| Core 最终 checkpoint | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core code checkpoint | `17fa2071398d5eaf30045257163d57d22d99177b` |
| 合同 payload | `5bd2b80b0953f4194d082940a7b9164c7231ca2d` |
| 规范 D1 fixture | `d1-main-cockpit.json`，SHA-256 `f96ba30cc6e80aa52cb15a2fd1f03c082487a3cd4779c25f61e42ee1548e1e3b` |
| 必需 Core capabilities | 15 项冻结能力加 additive extension capabilities，包括 `runtime.cockpit_context_v1` |
| 内置 locale | `en`、`zh-CN` |
| 外观系统 | 5 套 skin、8 组有效 skin/mode、3 档 density、3 种 motion |

当前机器可读 manifest 是 [release-manifest.toml](release-manifest.toml)。
不可变 rc.3 快照是
[manifests/0.1.0-rc.3.toml](manifests/0.1.0-rc.3.toml)；此版本 checkpoint
下两者必须逐字节一致。更早的 alpha、beta 与 rc.2 快照继续作为历史证据保留，不会被重写。

## 设计真源顺序

视觉和交互盘点固定从以下层级进入：

1. `docs/viden-design/Viden/index.html`
2. `docs/viden-design/Viden/GUI/Viden - 设计稿索引 (GUI).html`
3. `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html`
4. `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html`（D1）

D11 项目配置、D4 Lane 创建、D6 运行期恢复都是
`docs/viden-design/Viden/GUI/pages/` 下的从属屏。它们定义操作闭环，但不能替代 D1
作为桌面驾驶舱基线。

可复现的 design revision 还必须覆盖已登记的组件语义和这些屏幕实际消费的本地源：
`docs/DESIGN-REF.md`、`GUI/gui-kit.css`、`GUI/gui-icons.jsx`、
`GUI/gui-titlebar.jsx`、`GUI/gui-statusbar.jsx`、`GUI/gui-inbox.jsx`、
`GUI/gui-settings.jsx`。Manifest 记录精确有序列表；archive 和 mock 源不计入。

## Core 边界

GUI 代码只能依赖 `viden-core` 和 GUI 自有框架/平台代码。允许使用的 Core 入口是：

- `CoreClient`、`CoreTransport`、`StatefulCoreClient`、`LocalCoreTransport`；
- `CoreHandshake`、schema/capability 常量、command/event envelope、snapshot、replay、
  transcript paging、`RuntimeViewState`；
- `viden-core` 重新导出的 frontend-neutral domain records。

GUI 禁止导入 `viden_core::legacy`、`viden-runtime`、`viden-provider`、
`viden-tools`、`viden-permissions`、`viden-session`、`viden-workflows`、
`viden-context` 或 config internals。所有 mutation 都发送 `RuntimeCommand`；
只有收到 `CommandAccepted` 和后续有序 state event 后才能显示成功。

前端侧的同一纪律收敛为单一宿主接缝：`src/host/core_client.ts` 定义传输中立的
`GuiCoreClient` 接口（区别于上文 Rust 侧的 `CoreClient`），
`src/host/tauri_core_client.ts` 是前端唯一允许导入 `@tauri-apps/*` 的模块。
屏幕和 shell 只消费注入的接口，因此更换桌面宿主意味着提供另一个
`GuiCoreClient` 实现，而不是修改屏幕代码。

## 对照 Core `0.3.5` 的清单

| GUI 区域 | 设计意图 | Core `0.3.5` 状态 | GUI 处理 |
| --- | --- | --- | --- |
| 打开项目 / D11 接入 | 原生文件夹打开，以及 project probe、provider health、config preview/confirm、credential handles | `LocalCoreHost::open_workspace` 已提供可信文件夹重绑；安全 credential ingress 与 GUI recent-work adapter 仍不完整 | Welcome 直接使用原生选择器和 host rebind；D11 只作为项目内显式配置流程，不接管打开文件夹 |
| D4 Lane 创建 | typed role、route、gate strength、mutation policy、target、budget、worktree preview、lane receipt | 已有 `PreviewStarterLane`/`CreateStarterLane`、Core 解析 preview、invalidation、approval、精确 receipt 与 `runtime.starter_lane_preview` 广告 | Task 8 渲染四步复核流程；连接旧版 Core 时仍以可见 unavailable 和零发送 fail closed |
| D1 驾驶舱 | 无项目欢迎中心、零 Lane 项目驾驶舱、activity/lane rails、streaming transcript/tool rows、Environment、Live Work、composer、evidence/context/cost facts | stream/tool/approval/queue/task/lane/owner/evidence/context/cost/preferences facts 已有；diff/apply、稳定 audit timeline、可操作 Lane recovery 与 GUI recent-work projection 尚不完整 | 未绑定 host 才显示 Welcome；已绑定空项目仍留在 D1 并提供“新建 Lane”；实时工作从 `RuntimeViewState` 渲染 |
| Permission dock | scoped approve/deny、risk、target、expiry、default action、audit id | `ApprovalRequestView` 和 `RespondToApproval` 已有 | 可经 Core 使用；GUI 不得直接执行 tool |
| D2 决策中心 | 跨 Lane 的统一决策队列：闸审批、lane 问询、契约确认共用「上下文 / 证据 / 动作栏」一套卡片骨架 | `pending_approvals` + `RespondToApproval`、`review_requests` + `ReviewRequestStatus`、`contracts` + `ConfirmContract` 已有；评审决定命令、审批的结构化 diff、待确认契约事实缺失 | 入口 `?screen=d2`；闸与契约决定发出 Core 命令，评审以 `GUI-CORE-011` 只读，审批 diff 以 `GUI-CORE-012` 声明不可用，契约分组以 `GUI-CORE-013` 标注为已决历史 |
| D10 Lane 监视器 | 跨项目每条 Lane 一张卡：门控强度、状态、进度、证据与「等你」计数 | `lanes`、`lane_runtime_owners`、`tasks`、`agent_sessions`、`latest_evidence` 已有；视图状态没有有序事件日志 | 入口 `?screen=d10`；只读，门控强度取自 `AgentLaneRecord.gate_strength` 而非 agent 标签，未绑定 Lane 不显示项目，无 Core 任务的 Lane 不显示进度，事件流以 `GUI-CORE-014` 声明不可用 |
| D12 集成闸 | 冲突横幅、闸策略、退回原 Lane 的恢复时间线、合入后回滚，且不提供手动 merge | `merge_gates`、`conflict_bounces`、`reverts`、`check_runs` 已有；不发布结构化冲突内容 | 入口 `?screen=d12`；只有闸策略要求的证据 id 全部就位时 `accept` 才开放，时间线与回滚按选中闸限定，冲突 hunk 以 `GUI-CORE-015` 声明不可用 |
| D14 审计与时间线 | 跨工作区的有序审计轨迹，支持分页 | `CoreClient::replay` 的 `ReplayRequest`/`ReplayBatch` 与 `EventCursor` 已有；视图状态没有事件日志（`GUI-CORE-014`） | 入口 `?screen=d14`；行按 Core 回放 cursor 顺序取得，行标签用 Core 自己的 serde 判别名而非客户端改名，无法解码的事件仍占一行，回放失败显式提示而不是给出更短但看起来完整的轨迹 |
| D13 Fleet 编排与 Workflow | 每个 workflow DAG 一块看板：声明的依赖边、节点运行状态、阻塞原因与 Lane 交接 | `agent_dags`（含 `AgentDagTaskSpec`）、`tasks`、`dependencies`、`handoffs` 已有 | 入口 `?screen=d13`；只读，依赖边取自任务规格自身的 dependencies，节点只有在 Core 真正跑该任务时才显示状态，阻塞只来自 Core 的 `DependencyState::Blocked` 记录，交接绝不由依赖边推导 |
| D6 恢复 | 连接中、断连、agent stopped、budget exhausted、gate queue clear、reconnect/restart/close actions | Runtime errors、CoreClient snapshot recovery、context budget facts、queue/gate facts 已有；结构化 Lane lifecycle recovery commands 缺失 | Task 10 渲染运行期 Core-owned 恢复状态；无项目 `empty` 状态由 D1 Welcome Center 承担，restart/close/checkpoint 仍以 `GUI-CORE-003` 明确禁用 |
| Locale 与换肤 | `en`/`zh-CN`、Aurora/Ice/Mono/Amber/Phosphor、明暗约束、density、motion | 已有 `RuntimeSnapshot.ui_preferences`、`SetUiPreferences`、`ResetUiPreferences`、`UiPreferencesUpdated`、持久化和安全回退诊断 | GUI 渲染 Core resolved preferences；正式 Settings 控件仍是前端实现任务，并且必须等待有序 Core event |

开放请求记录在 [contract-requests.md](contract-requests.md) 和
[contract-requests.zh-CN.md](contract-requests.zh-CN.md)。GUI 不得用私有 reducer 或直接访问
runtime 来绕开这些缺口。

剩余开放请求只阻塞各自点名的生产屏，不阻塞 framework-neutral、fixture-only 的
Task 2-3 及其证据；spike 结果不能授权生产 mutation 或 persistence。

## D11 项目接入

Task 7 在固定 Core `0.3.2` integration checkpoint 之后实现 D11 项目内显式配置流程。
`GuiCoreAdapter` 会按 advertised extension capabilities 分别 gate project onboarding 与
credential-handle intents。Probe、preview 与 confirm 保持为不同 Core command；D11 的
starter 选择只形成有序本地复核队列，绝不发送旧 `CreateLane`。只读 `d11_poll` command
会在首次有界等待之后继续接收迟到事实；adapter 会串行化
pending intake command，并在清除 pending 前匹配 preview hash、confirm id/hash、Lane id
和 active approval request id。Project-config approval 必须先绑定精确 Core metadata
token `sha256=<64 lowercase hex>`，GUI 才接受它的 request id；如果存在有边界的
`preview_id=` token，也必须等于 pending preview id。hash substring、非 lowercase
hash、free text 和非 `sha256` 字段都不能 retarget 或清除当前 pending command。
Allow 决策后继续等待 matching business fact；deny/expiry 决策会
清除 pending，并继续 drain 后续 Core error projection。瞬时 poll 失败会保留 local draft
与 pending identity，然后按有界 backoff 重试。等待期间的 Core 中间投影变化仍会显示。
Cancel 只清除内存导航状态，不发生 Core mutation，并返回 D1。Welcome 不进入该流程：
文件夹选择和 host rebind 会先独立完成。

独立 host 会先消费有序 command events，再刷新权威 snapshot，因此 acceptance 不会吞掉
后续 probe、preview 或 confirmation 事实。新项目 draft 默认包含必填的 `name` 与 `pack`
字段。若确认需要 Core 批准，D11 会嵌入与 D1 相同的 typed Permission Dock；
`Allow once` 或 `Deny` 仍是显式 Core command，不会成为 GUI 侧绕过。

## D4 起始 Lane 复核创建

Task 8 从项目驾驶舱的“新建 Lane”进入 D4，每次只复核一个 seed。创建前 Cancel/Skip
会零 mutation 返回 D1；创建发出后，界面只提供精确 Core
approval 的 allow/deny，不伪造 cancel command。每个完整
`StarterLaneCreated.receipt` 推进一项，最后一个 receipt 发出 typed D1 导航请求，并聚焦
最后创建的 Lane。

Adapter 发送 `PreviewStarterLane`、保留原始请求，并且只接受同 owner 的
`StarterLanePreviewed` 事实。branch、worktree、base revision、route、gate、target、
mutation policy 与 budget 都是只读 Core 事实。Build 模式只能创建未变更的已复核请求；
Plan 模式可预览但不可创建。`CommandAccepted` 与 `LaneUpdated` 都只是中间事实，绝不触发
导航。请求变更、rejection、approval deny 和 typed preview invalidation 会保留 webview
draft，并要求重新预览。只有 owner/id/hash/Lane/branch/worktree/base/config 全量匹配的
`StarterLaneCreated` 才能推进队列。

Core `0.3.5` production handshake 已包含精确 additive capabilities
`runtime.starter_lane_preview` 与 `runtime.cockpit_context_v1`，因此 D4 与 D1
Context Dock 可使用已复核 typed flow。连接旧版或不完整 Core 时仍明确展示门禁并保持零发送；
`runtime.lane_lifecycle` 不能作为替代。

rc.3 的确定性浏览器证据位于 `evidence/0.1.0-rc.3/`。完整 8 组主题矩阵、3 档
density、两套 catalog 和 reduced-motion 行为仍由自动化测试覆盖。

配置 rail 只渲染 Core 返回的 `viden.toml` 精确复核内容，confirm 的 preview id 与 SHA
也从当前 Core projection 复制。Credential 行只显示 masked handle。由于尚无
frontend-safe 平台 credential staging channel，raw credential 输入与 webview
`StoreCredentialHandle` 路径以 `GUI-CORE-001` 明确禁用。跨项目 recent work 继续以
`GUI-CORE-007` typed unavailable 呈现；GUI 不扫描 local storage、JSONL 或 SQLite。
project switching 现通过 Core-owned `LocalCoreHost::open_workspace` 完成；
安全 raw credential staging 仍是 `GUI-CORE-001` 的未完成部分。

## D1 流式驾驶舱

Task 9 让 D1 成为常驻应用外壳和唯一主工作面。它先显示 D6 连接中/断开状态，或
host-owned 无项目欢迎页。已绑定但没有 Lane 的项目仍是 D1；D4 只从项目内创建入口进入，
D11 只用于显式配置。Activity rail、Lane rail、Environment、Live Work、
transcript/tool rows、排队状态、evidence 与 composer 都是 Core 最新
`RuntimeViewState` 的 transport-safe 投影。Webview 只持有焦点、draft、布局、有界行窗口
与滚动锚点，不解析显示字符串，不持久化第二套 workspace 模型，也不会把 command
acceptance 当作业务成功。有序 Core 刷新会原位更新 activity 与 Lane rail，让它们的
hover 根节点在易变的 Lane/Agent 状态变化期间保持挂载；这样既不会隐藏最新 Core 事实，
也不会让浮动侧栏反复闪烁。

“新建 Lane”会打开一个紧凑的锚定弹层，默认选中内置 Viden Agent，并包含已发现的 ACP
Agents、品牌身份、任务 draft、Core 投影的 eligibility/probe 诊断，以及只作呈现的
isolation 提示。“完整设置…”进入既有 D4 兼容流程，不在快速创建器里继续堆叠选项。
Git 工作区预览由任务文本派生的 branch/worktree；非 Git 目录明确提示 Lane
直接在已打开工作区运行，不创建二者。
选择 Agent 不会关闭弹层，任务 textarea 会获得焦点，任务非空前“创建 Lane”保持禁用。
该 textarea 持有 IME 组合态时，会推迟 Core 或 Agent 探测引发的界面重绘，避免 macOS
候选输入过程中节点被移除。
创建时沿用现有有序路径：`preview_default_lane`、`create_starter_lane`，并且只有在精确
Core Lane 已投影后才发送原生 `submit` 或 ACP `start_agent_session`。Transport 或 Core
拒绝会保留 draft，并使用 D1 已有 typed rejection surface。ACP 发现只在当前驾驶舱生命
周期内自动执行一次；重新打开弹层会复用结果。发现失败会结束忙碌态，在弹层中显示精确
诊断，并且只有用户点击“重试 ACP 检测”才再次执行。ACP 启动被拒绝时，其 Lane 创建后仍
会在 D1 typed rejection surface 上显示原因。

当 assistant stream、tool、task、approval 或 queued input 仍活跃时，composer 仍可编辑。
此时 Enter 发送 `QueueFollowUp`，空闲时发送 `SubmitUserInput`；Shift+Enter 保留多行输入，
CJK IME 组合阶段绝不提前提交；streaming Core 重绘同样会等到 `compositionend` 后再替换
输入节点。两种命令都使用 Core 发布的精确 Lane owner，来源只能是
live owner binding 或 D4 receipt。Cancel 更严格：只有选中 Lane 仍 active、Core 宣告
`runtime.lane_owner_projection`，并且 `lane_runtime_owners` 中恰好有一个完整匹配 owner 时，
控件才可见且允许传输。owner 缺失、错配或歧义都 fail closed，保持零发送。
若另一个有序 Core command 或 Agent probe 仍占用客户端命令槽，一条 composer 输入会以
`Queue follow-up` 可见等待，同时禁用重复“发送”；槽位释放后自动投递，绝不静默丢弃输入。
桌面端重启后，Core 恢复出的唯一 terminal ACP Session 即使已经没有进程内 Lane binding，
仍可使用其精确、持久的 Session owner 接收后续输入。Continuation 启动前，Core 会为同一
持久 owner 发布新的 `LaneRuntimeOwnerBound` 事实，因此 ACP 响应运行期间 Lane 会持续显示
busy，而不会短暂掉入 Agent Stopped；重复 Session、owner 错配以及非 ACP 恢复仍然 fail closed。
在同一次 App/Core 生命周期内，已完成 ACP turn 会保留健康的进程与远端 Session；下一次
“发送”因此直接进入 `session/prompt`。常驻连接已退出或不兼容时，Core 会在发送 prompt 前
回退到持久化 `session/load` 路径。GUI 不持有这份缓存，仍只渲染 Core 的有序事实。即时
Starting/busy 反馈衡量 Core dispatch；首个 assistant 内容的等待时间还包含冷启动时的 agent
启动、上下文处理和模型推理。
Core 将常驻池限制为八个 Session，并回收空闲 15 分钟的连接；之后的“发送”会透明走持久化
reload 路径。Core 关闭时也会回收该工作区的全部常驻连接。
取消内置 Agent 的活跃模型推理时，现在会先终止该 turn，再考虑 Lane 生命周期取消，因此
Lane 与其精确 owner 仍可继续路由。对于已经没有 owner 的旧 terminal 原生 Lane，composer
和“发送”会显示为禁用，而不是保留一个点击后无反应的控件。

Transcript 最多保留 240 行。离开最新输出边缘后会设为 `follow_latest=false`、保留当前锚点，
并递增可见的新输出计数，而不会强制滚动。Rust 与 webview 测试覆盖 10,000-event burst、
50,000 行、resize/idle 读取、CJK composition、多行 paste/undo、键盘遍历、ARIA region 与
可见焦点。`evidence/0.1.0-rc.3/` 下的 Browser-controlled 证据包含 Aurora
dark/regular 英文、Ice light/regular 英文、Aurora dark/regular 中文、compact density
和 responsive drawer 状态，并包含一个由 `d1-main-cockpit.json` 填充的独立同状态设计
reference。它还包含一个补充 Context Dock bottom-state capture，用于证明下方事实可通过内部
滚动到达。Diff、apply、audit 与未类型化 recovery actions 始终是明确 unavailable facts；D1
不会伪造成功占位。

## Permission Dock 与 D6 恢复

Task 10 把规范 `.gperm.dock` 紧贴放在 D1 composer 上方。它精确展示 Core approval 的
risk、target、allowed scopes、reason、input preview、expiry、default action 与 audit id。
Once、Session、仓库 allowlist 与 Deny 只映射到 `RespondToApproval`；Always 与 Edit 以
`GUI-CORE-003` 保持禁用。Plan 模式下的 mutation response 在 transport 前 fail closed。
Command acceptance 不代表成功：只有 owner/request/audit 全部匹配的有序
`ApprovalResolved` 事实才能清除 pending。

D6 是 D1 中央工作面的从属状态，不建立第二套 cockpit shell。Empty、connection、provider、
agent stopped、context overflow、capability、incompatible schema、queue clear 与 event gap
只来自 Core projection 或 CoreClient error。Event gap 的 reconnect 走 CoreClient snapshot
路径，并在已验证 live snapshot 发布前保持 busy。Restart、close Lane 与 checkpoint 控件仍
可见但以 `GUI-CORE-003` 禁用；GUI 不伪造 recovery receipt。

## Production bootstrap

`src-tauri` 是 root Rust workspace 中唯一 GUI member，并显式声明独立
`0.1.0-rc.3` 版本。`GuiCoreAdapter`、其 D4 adapter extension 与
`RuntimeProjection` 是 production source 中仅有的 Core contract 边界模块；
`GuiPreferences`、`WorkspaceSelection`、
`ComposerDraft` 与 `TranscriptViewport` 只保存 presentation state。关闭窗口只会丢弃
注入的 client，不会发送 mutation。

## Locale 与外观

Production webview 读取 `RuntimeViewState.snapshot.ui_preferences` 的 transport-safe
投影。只有这份 Core resolved state 能设置 document language、skin、effective mode、
density 和 motion 属性。内置 `en`、`zh-CN` catalog 会检查 key 与 placeholder parity；
shortcut、路径和代码保持原样，不进入翻译流程。

有效外观矩阵固定为 8 组：Aurora、Ice、Mono 各自支持 dark/light，Amber 与
Phosphor 仅支持 dark。非法或损坏值采用确定性的安全回退，同时保留 diagnostic。
Tauri CSS adapter 直接 import `docs/viden-design/Viden/tokens.css`。运行
`tools/check-generated-tokens.sh` 会检查 SHA-256、semantic roles、theme/density
矩阵、adapter import 和 generated metadata；production GUI source 不手抄 token 值。

Preference 控件可以保留未保存的内存 draft。Save/restore 必须使用
`SetUiPreferences` 或 `ResetUiPreferences`：GUI 不写 browser storage、文件、config，
也不建立私有 preference authority；只有 `UiPreferencesUpdated` 提供新的 resolved
projection 后才能改变渲染状态。

默认原生 binary 在未显式设置 `VIDEN_GUI_WORKSPACE` 时不预绑工作区。D1 Welcome 打开
系统文件夹选择器，`LocalCoreHost` 在注入的 frontend-safe `CoreClient` 背后构造并持有
runtime。真正的 host/bootstrap 失败显示 D6 disconnected，不进入 D11。D11 adapter 仍
需要注入 frontend-safe `CoreClient`。GUI 不直接导入 runtime ownership，也不自行构造
`SessionEngine`/`RuntimeSupervisor` 或增加私有 reducer。
Task 6 现已负责 resolved locale/appearance projection 与未保存 draft contract；Task 7
负责 D11，Task 9 负责 D1。

## rc.3 视觉、元数据与 bundle 门禁

Task 11 新增 framework-neutral 组件画廊，以及面向 D1、D11、D4、D6 和 gallery 的
deterministic pairwise case inventory/DOM contract。它枚举中英文、全部有效 skin/mode
组合、3 档 density、system/reduced motion，以及桌面、窄屏和放大字体要求。当前已复核
视觉证据包含代表性 desktop gate 和精确尺寸的 D1 同状态 QA；gallery、窄屏与放大字体
capture 仍明确标为 partial。D1 的 pass/fail 视觉 QA 使用独立 canonical-state design
reference 与 production canonical capture 对比；旧 accepted desktop cockpit 截图仅作为历史视觉
lineage 保留。

机器可读的可访问性、有界本地性能记录、Browser-controlled 同状态截图、side-by-side QA、
精确方法和明确的原生审计/profile skip 都在
[evidence/0.1.0-rc.3](evidence/0.1.0-rc.3/README.md)。active manifest 与
immutable rc.3 snapshot 记录相同证据路径并保持逐字节一致。macOS `.app` bundle
只是本地构建产物；未安装、签名、公证、发布、打 tag 或 release。
