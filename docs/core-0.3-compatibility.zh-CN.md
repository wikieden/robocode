# Core 0.3 兼容性

English version: [core-0.3-compatibility.md](core-0.3-compatibility.md)

本文是冻结的 Core 0.3.0 `frontend-contract-v1` payload 及其向后兼容 Core 0.3.2
extension candidate 的人类可读兼容清单，记录前端 schema、handshake capabilities、
migration 顺序、确定性 fixture corpus、UI 偏好契约和设计入口层级。

## 冻结状态

```text
component = viden-core
component_version = 0.3.0
supported_schema_versions = [1]
active_schema_version = 1
contract_payload_sha: 5bd2b80b0953f4194d082940a7b9164c7231ca2d
```

这里记录的 40 字符 SHA 标识评审通过的 contract payload commit。本文通过单独的
evidence commit 落盘；该 evidence commit 是 TUI 与 GUI 的共同精确分支基线，并且它的
parent 必须等于这里记录的 payload SHA。本文不授权 tag、push、publish 或 Homebrew
变更。

## 冻结 Capability 集合

Core 以 `CORE_CLIENT_CAPABILITIES` 暴露并供 handshake 使用的冻结 capability 常量是
唯一真源。Core 0.3.0 公布以下精确、唯一且按字典序排列的集合：

```text
runtime.agent_dag
runtime.approvals
runtime.commands
runtime.context
runtime.cost
runtime.events
runtime.evidence
runtime.merge_gate
runtime.queued_input
runtime.replay
runtime.snapshot
runtime.transcript_page
runtime.typed_lanes
runtime.typed_tasks
ui.preferences
```

每份冻结 base fixture 的 requirement 都必须存在于该集合；单独登记的 extension fixture
只能要求已公布的 extension capabilities。任何 fixture 要求未知 mandatory capability 都会
在兼容性验证中失败；malformed 或 ambiguous legacy input 必须拒绝，不能猜测。

## Schema-1 冻结后的扩展候选

上面的 Core 0.3.0 冻结 capability 集合与原九份 fixture bytes 保持不变。Core 0.3.2
候选通过 `FRONTEND_V1_EXTENSION_CAPABILITIES` 和
`crates/core/frontend-contract-extensions.toml` 单独公布以下精确、按字典序排列的增量
capability：

```text
core.workspace_host
runtime.credential_handles
runtime.credential_staging
runtime.lane_lifecycle
runtime.lane_owner_projection
runtime.project_onboarding
runtime.recent_work
runtime.starter_lane_preview
runtime.trust_loop
ui.preference_persistence
```

schema 仍为 `1`。客户端只具备冻结 base 集合也可以连接；extension capability 缺失只
禁用对应功能，不得阻止无关启动。被禁用功能必须明确显示 unavailable，并且 command
零发送。具体而言：TUI stable Settings 要求 `ui.preference_persistence`；GUI D11 recent
work 要求 `core.workspace_host` 与 `runtime.recent_work`；TUI/GUI reviewed D4 创建要求
`runtime.starter_lane_preview`；精确 active-Lane cancel 还要求
`runtime.lane_owner_projection` 且恰好一条权威 binding。

基于 Core 0.3.0 编写的客户端仍然只要求冻结集合，并把不支持的 schema-1 事件保留为
`RuntimeWireEvent::Unknown`。新客户端只有在协商到 `runtime.lane_lifecycle` 后，才启用
13 个 Lane 生命周期命令以及 `LaneUpdated`、`LaneCommandAccepted`、
`LaneOutputAppended`、`LaneConflictDetected`、`LaneRecoveryRequired` 投影。Lane 命令回执
使用扩展专属的顶层事件，因此 0.3.0 客户端会把整个 payload 保留为 unknown，而不会因
内嵌的新命令变体导致解码失败。扩展投影为空时不会参与序列化，
因此重放冻结的 0.3.0 corpus 仍保持已记录的 canonical bytes 与 digest。

`runtime.trust_loop` 新增 typed handoff、review request、contract、dependency、
merge-gate policy/validator/decision、conflict bounce 与 revert facts。七个新增跨 Lane
command（含显式 `RevalidateMergeConflict`）及其 events 均经过权限门禁并由共享
reducer 重放。Schema 仍为 `1`：新增
record fields 提供默认值，未知字段可忽略；扩展前的 string merge decision 会读取为只读
`legacy` decision，新写入则始终序列化 typed decision。只有绑定真实 ContextStore bytes
与 Core 签发 permission receipt 的 evidence 才能产生 canonical acceptance；展示摘要不能
替代 evidence。指定 validator 必须绑定精确 id/hash 集，且所有 trust 纯 preflight 在
approval 前完成；`RequestReview` 本身由发起请求的 gate owner 授权。Dependency id 是稳定
edge id，不能重绑到不同端点。Merge 在改动文件前持久化私有 content-addressed recovery
snapshot 与 workflow precommit；重复 preimage blob 会复用，私有 recovery lock 拒绝
symlink traversal，从而在不把 raw preimage 写入 event log 的前提下支持重启后 audited revert。

Core 负责 Lane 权限判定，并在每个 Lane 命令前从当前 runtime mode 刷新权限状态。
所有有副作用的命令都使用 permission check 与 effect executor 共享的 canonical worktree
或 repository target 判定。已有 symlink 只能解析到仓库内；缺失 target 通过最近的真实父目录
解析，拒绝 symlink parent 与 `..`，并在本地 effect 前再次校验。审批预览会红写 command、
arguments、environment、input 和 diff payload。重启时，处于 starting、running 或等待审批
状态的 Lane 会恢复为 blocked recovery fact，并继续绑定其持久化 session owner。

项目接入对当前目录执行只读探测。`PreviewProjectConfig` 校验仓库根
`viden.toml` policy，并返回可审阅的精确 UTF-8 内容及 SHA-256，不写文件；
该 D11 parser 只接受已登记的 `project`、`gates`、`runner`、`budget`、`targets`
schema，拒绝未知 nested field；候选包含 secret field 或 credential-shaped value 时，
不会返回 exact contents。
`ConfirmProjectConfig` 只接受已缓存的 preview id/hash，重新核对目标文件的 base hash，
并在 Build 模式权限批准后写入同一组精确字节。Credential command 只携带 provider、
backend 与一次性 ingress 标识；这些标识采用有长度上限的 ASCII opaque-id grammar，
并拒绝 secret-like marker 与 path syntax。secret bytes 始终留在注入的 backend 中，replay/audit
只记录 `CredentialHandle` 安全元数据。

普通 tool 与 Lane 的审批响应都会按 supervisor 中 permission/mode 变更的命令顺序判定。
但两者刻意采用不同的 generation 语义：普通 tool 读取已提交的 permission control
reservation，因此 permission 或 work-mode 命令一经入队，即使 worker 尚未应用，也会立即
且永久地使阻塞中的审批失效。即使控制命令的 SessionMeta batch 随后持久化失败，已提交的
generation 也不会递减或复用；旧普通审批以 `Deny` 终结，不能恢复，用户必须重新触发 tool
以取得新审批。失败的 reservation 仍会从应用状态投影队列移除，避免其 policy 泄漏到后续
控制命令。Lane 请求则原子冻结 worker 已应用的 generation 及其所描述的 permission
engine；只有队列中的控制命令成功应用后，这一 generation 才会推进，因此 Lane 审批可以
在一次控制命令失败后继续有效。permission 与 work-mode 控制命令会先原子持久化完整的
session metadata batch，再发布新的 live snapshot 与 permission engine；batch 失败时，
engine、snapshot、Lane 配对和已应用 generation 都保持不变。审批等待期间只要已应用的
permission 或 work mode 代际发生过变化，即使可见 flags 随后恢复原值，
旧 Lane 审批也会失效。Lane 响应被接受后，supervisor
必须等待终态 `ApprovalResolved` 以及 effect/persistence 完成，才能处理或发布后续 permission
snapshot。审批产生的 session/repository allow rule 只保存在所属 Lane worker 内，因此会在
该 Lane 的常规权威权限刷新后保留，但不会授权另一条 Lane 或 owner；
Plan/ReadOnly 刷新会立即丢弃这些 rule。Create 与 Lane 状态迁移和其他持久化 effect 使用
同一 permission 与 mutation-policy gate。终态 worker 通过 completion reaper 自动注销并
join，不需要等待下一条 Lane 命令。

## Client 边界

前端 client 只能使用 `CoreClient` 和 `viden-core` 重导出的 protocol/view contracts。
Transport interface 仅包含 discovery、command send、event receive、snapshot、replay 和
transcript paging。前端禁止导入或调用 runtime、provider、tool、permission、session 或
workflow 内部模块。

`StatefulCoreClient` 在提交状态前验证 handshake 与 schema。它忽略 duplicate/older
cursor，只应用连续的 next event；gap replay 必须完整且验证通过后才提交；stream
mismatch 或 snapshot-required recovery 通过验证后的 snapshot 恢复。前端永远不能自行
合成 effect 成功状态。

`viden_core::legacy` 已 deprecated。它只为 pre-v3 TUI bootstrap 临时保留，新的 TUI、
GUI、CLI、API 或 plugin client 禁止使用。

## Schema-1 Fixture Corpus

Fixture 文件位于 `crates/types/tests/fixtures/frontend-contract-v1/`。下表 digest 单元是
经过测试的 fixture 值，必须与对应 fixture state 一起变更。

| Fixture id | 冻结场景 | Expected final view SHA-256 |
| --- | --- | --- |
| `stream-tool` | Assistant stream、tool start、成功 tool finish | `8478c7c0ce6f0adc3efdd3aa11497462e96b3aba50cf66e81b0ad9ddcd992eef` |
| `approval-allow-deny` | Structured scoped allow/deny，前端不拥有 effect | `7788f2f4b34ce54893ab8ed41beb6e37958ff5fda95642d045ef2d1dedbf7b39` |
| `queued-follow-up` | Active work 保持可见时 queue/dequeue follow-up | `eb1bc1a00185d5642f9a95a2cffde7a81f2bd4ac4417385c5c1b6e2aefa8354a` |
| `dag-blocker` | Typed DAG/task dependency blocker 与 recovery action | `a496d331e42f730d41565afe58a3308bf38a7b7e3b92e0279d198e9c407e7719` |
| `multi-lane` | 多条 typed lane，覆盖不同 role、route、gate、owner、target、budget、session facts | `e491d3bc547601b3c54eae05dc1b1259c9cc8ccac948908be8519d432b62fe38` |
| `merge-gate` | Typed evidence 与 Core-owned MergeGate reduction | `41f4d842a12356586a461b173d072d7e7efedb4d7707471c99eb77dd37533321` |
| `context-pressure-cost-blind` | Context pressure/omission 与明确 unknown/unmetered terminal cost | `2e39ec2e32fac56ae6279e8f681bcf4357701de51a6772bad14caee0ddb4ba5e` |
| `plan-denial` | Plan mode 拒绝 mutation，且没有成功 mutation fact | `fa1fa859af8f056686c06b30b789706539d9ed19e02756519757993d5ee31b2d` |
| `d1-vertical-slice` | D1 可见 transcript/tool、lane/task、decision、evidence/gate、context/cost、recovery、UI preferences | `7dd8faf04cca9f3013198e25823894eae91c2869e27087aa1eb0a34890cdf804` |

上表九行是冻结 base corpus。单独登记的 schema-1 extension fixture 为：

| Fixture id | 扩展场景 | 最终 view SHA-256 | Canonical fixture bytes SHA-256 |
| --- | --- | --- | --- |
| `frontend-host-services` | UI 偏好持久化、安全 recent work、reviewed starter-Lane preview/create/invalidation、精确 live Lane owner，以及一个可容忍的未来 optional event | `b118534bb0a568a6a1e781171cecf0512c7d987736c06e4f84d51b5835022a0e` | `96dd5fde9f1241eb50f9d8978cf478d0ac5d3327448dc6ccde9d0e5018ce1580` |

扩展 fixture 使用六个正式 known event：`UiPreferencesUpdated`、`RecentWorkLoaded`、
`StarterLanePreviewed`、`StarterLaneCreated`、`StarterLanePreviewInvalidated`、
`LaneRuntimeOwnerBound`；禁止用 transient error 或展示 placeholder 代替。正常内存
journal、snapshot 与 replay 路径必须把这些 facts 归约为相同 `RuntimeViewState`。未来
optional event 只推进 cursor，不修改该 state。

每个 JSON fixture envelope 包含：

- `fixture_id`；
- `schema_version: 1`；
- 唯一且按字典序排列的 `required_capabilities`；
- `initial_snapshot`；
- 非空、cursor 连续的 `RuntimeEventEnvelope`；
- `expected_final_cursor`；
- `expected_view_sha256`。

每个已知 event 的 event sequence 必须等于 cursor sequence。从同一 initial snapshot
把解析后的 fixture replay 两次，必须得到 byte-identical canonical state、相同 cursor 和
相同 digest。Fixture 值必须确定，且不能包含机器专属绝对路径或 secret。

## Canonical Digest

Final-view digest 是对 `RuntimeViewState` 递归排序所有 object key 后的 compact JSON 计算
SHA-256。Array 顺序保留，因为它具有语义。测试会把生成的 64 字符小写十六进制 digest
与 fixture 和本清单比较。

## Migration Gate 与顺序

Migration 必须在 schema-1 fixture replay 前运行，并且保持幂等：

1. 通过支持的 v0 lane input boundary 解析 `legacy-lanes.tsv`，得到 typed
   `AgentLaneRecord`。
2. 与 `typed-lanes.json` 比较，序列化 normalized typed values 后再次解析，并要求相等。
3. 把支持的 legacy flat cost shape 解析为 structured `CostUsageRecord`；unknown actual
   cost 必须保持 `None`，序列化 normalized record 后再次解析，并要求相等。
4. 把支持的 legacy approval boolean 解析为 structured `ApprovalResponse`，序列化时不再
   输出 legacy boolean，再次解析并要求相等。
5. 所有 migration 通过后，才将每个 schema-1 fixture replay 两次，并验证 identity、
   capabilities、cursor continuity、final state 和 digest。

未知 lane role/route/status、ambiguous cost shape、malformed approval record 和未知
mandatory fixture capability 都必须让 gate 失败。Migration 不得静默强转。

## UI 偏好兼容性

Schema-1 preference surface 与前端框架无关：

前端消费的 effective fact 是
`RuntimeSnapshot.ui_preferences: ResolvedUiPreferences`。Client 只渲染已解析值，不能在
本地重新执行优先级或 fallback policy。

| 维度 | 支持值 |
| --- | --- |
| 内置 locale | `en`、`zh-CN`（`system` 解析为其中之一） |
| Skin | `aurora`、`ice`、`mono`、`amber`、`phosphor` |
| Effective mode | `dark`、`light` |
| Density | `compact`、`regular`、`comfy` |
| Motion | `system`、`reduced`、`full` |

八组有效 effective skin/mode 是：

```text
aurora/dark
aurora/light
ice/dark
ice/light
mono/dark
mono/light
amber/dark
phosphor/dark
```

`amber` 与 `phosphor` 仅支持 dark。偏好优先级是 CLI、user、project、client default。
无效 effective pair 使用安全的 `aurora/dark` + regular density 回退，并记录
`ui.invalid_skin_mode_pair` diagnostic；locale 与 motion 保持解析结果。

## 设计入口层级

视觉验证遵循一条路径：

1. 全局索引：`docs/viden-design/Viden/index.html`；
2. client 索引：`TUI/Viden - 设计稿索引 (TUI).html` 或
   `GUI/Viden - 设计稿索引 (GUI).html`；
3. 组件库：`TUI/Viden - 组件库 (TUI).html` 或
   `GUI/Viden - 组件库 (GUI).html`；
4. canonical 产品入口：`TUI/Viden - 统一原型 (TUI).html` 或
   `GUI/Viden - 桌面驾驶舱 (GUI).html`（D1）。

GUI `pages/Viden - D11 首启与项目接入 (GUI).html` 是下级 onboarding，不是驾驶舱，
也不能替代 D1 成为 GUI baseline。以上所有相对路径均从
`docs/viden-design/Viden/` 起算。旧截图和 generated previews 只属于历史证据，不能
覆盖该层级。

## 历史兼容性

v0 lane TSV input、legacy flat cost shape、legacy approval boolean 和
`viden_core::legacy` bridge 都是 migration surface，不是新的 client API。Client 迁移到
schema `1` 与 CoreClient-only 边界时，保留历史 release evidence 不变。
