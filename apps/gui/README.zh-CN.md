# Viden GUI

英文版：[README.md](README.md)

本目录是 Viden 的 GUI 实施线。`0.1.0-alpha.1` 阶段只冻结合同输入并准备
framework gate，还不是生产桌面应用。Tauri/GPUI 必须用同一 Core fixture、可访问性、
IME、转录和打包证据决出胜者后，GUI 才能进入生产框架实现。

## 冻结输入

| 字段 | 值 |
| --- | --- |
| GUI 组件版本 | `0.1.0-alpha.1` |
| 最低 Core 版本 | `0.3.0` |
| 支持 frontend schema | `[1]` |
| 共同分支基线 | `afd6fcc9aaf3039ba79bb4588ed33bf1547209f5` |
| 合同 payload | `5bd2b80b0953f4194d082940a7b9164c7231ca2d` |
| 必需 Core capabilities | 来自 `CORE_CLIENT_CAPABILITIES` 的 15 项 |
| 内置 locale | `en`、`zh-CN` |
| 外观系统 | 5 套 skin、8 组有效 skin/mode、3 档 density、3 种 motion |

当前机器可读 manifest 是 [release-manifest.toml](release-manifest.toml)。
不可变 alpha 快照是
[manifests/0.1.0-alpha.1.toml](manifests/0.1.0-alpha.1.toml)；此版本 checkpoint
下两者必须逐字节一致。

## 设计真源顺序

视觉和交互盘点固定从以下层级进入：

1. `docs/viden-design/Viden/index.html`
2. `docs/viden-design/Viden/GUI/Viden - 设计稿索引 (GUI).html`
3. `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html`
4. `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html`（D1）

D11 首启接入、D4 Lane 创建、D6 恢复/空态都是
`docs/viden-design/Viden/GUI/pages/` 下的从属屏。它们定义操作闭环，但不能替代 D1
作为桌面驾驶舱基线。

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

## 对照 Core `0.3.0` 的清单

| GUI 区域 | 设计意图 | Core `0.3.0` 状态 | GUI 处理 |
| --- | --- | --- | --- |
| D11 项目接入 | project probe、recent project/session、provider health、config preview/confirm、starter lanes | provider/model 配置和 provider health 已有；project probe/config preview/recent project/starter-lane commands 缺失 | 阻塞生产 D11，等待 Core request `GUI-CORE-001` |
| D4 Lane 创建 | typed role、route、gate strength、mutation policy、target、budget、worktree preview、lane receipt | typed lane records 已有；未导出 create-lane/worktree-preview/lane-created command | 阻塞生产 D4，等待 `GUI-CORE-002` |
| D1 驾驶舱 | activity rail、lane rail、streaming transcript/tool rows、permission dock、worktree board、evidence/gate/context/cost panels、settings entry | stream/tool/approval/queue/task/lane/evidence/merge/context/cost/preferences facts 已有；worktree board、lane lifecycle、diff/apply file facts、稳定 audit timeline 不完整 | Task 2/3 可做 fixture replay；生产 D1 等待 `GUI-CORE-002`、`GUI-CORE-003`、`GUI-CORE-004` |
| Permission dock | scoped approve/deny、risk、target、expiry、default action、audit id | `ApprovalRequestView` 和 `RespondToApproval` 已有 | 可经 Core 使用；GUI 不得直接执行 tool |
| D6 恢复 | 空 cockpit、连接中、断连、agent stopped、budget exhausted、gate queue clear、reconnect/restart/close actions | Runtime errors、CoreClient recovery、context budget facts、queue/gate facts 已有；结构化 connection/lane lifecycle recovery commands 缺失 | 可先做只读/错误呈现；可操作恢复等待 `GUI-CORE-003` |
| Locale 与换肤 | `en`/`zh-CN`、Aurora/Ice/Mono/Amber/Phosphor、明暗约束、density、motion | 已有 `RuntimeSnapshot.ui_preferences: ResolvedUiPreferences` 和安全回退诊断 | GUI 渲染 Core resolved preferences，仅保留本地显示状态 |

开放请求记录在 [contract-requests.md](contract-requests.md) 和
[contract-requests.zh-CN.md](contract-requests.zh-CN.md)。GUI 不得用私有 reducer 或直接访问
runtime 来绕开这些缺口。

## 下一实施门

Task 2 会在 `CoreClient` 和共享 `d1-vertical-slice` fixture 之上建立 framework-neutral
replay harness。它必须先证明有序 replay、snapshot recovery、transcript paging anchor
和 projection parity，之后才能引入 Tauri 或 GPUI 的生产代码。
