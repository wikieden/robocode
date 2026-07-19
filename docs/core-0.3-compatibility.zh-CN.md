# Core 0.3 兼容性

English version: [core-0.3-compatibility.md](core-0.3-compatibility.md)

本文是 Core 0.3.0 `frontend-contract-v1` payload 的人类可读兼容清单，记录前端
schema、handshake capabilities、migration 顺序、确定性 fixture corpus、UI 偏好契约
和设计入口层级。

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

每个 fixture 的 requirement 都必须存在于该集合。要求未知 mandatory capability 的
fixture 会在兼容性验证中失败；malformed 或 ambiguous legacy input 必须拒绝，不能猜测。

## Schema-1 冻结后的扩展候选

上面的 Core 0.3.0 冻结 capability 集合与 fixture digest 保持不变。Core 0.3.1 候选
通过 `FRONTEND_V1_EXTENSION_CAPABILITIES` 和
`crates/core/frontend-contract-extensions.toml` 单独公布增量 capability
`runtime.lane_lifecycle`。

基于 Core 0.3.0 编写的客户端仍然只要求冻结集合，并把不支持的 schema-1 事件保留为
`RuntimeWireEvent::Unknown`。新客户端只有在协商到 `runtime.lane_lifecycle` 后，才启用
13 个 Lane 生命周期命令以及 `LaneUpdated`、`LaneCommandAccepted`、
`LaneOutputAppended`、`LaneConflictDetected`、`LaneRecoveryRequired` 投影。Lane 命令回执
使用扩展专属的顶层事件，因此 0.3.0 客户端会把整个 payload 保留为 unknown，而不会因
内嵌的新命令变体导致解码失败。扩展投影为空时不会参与序列化，
因此重放冻结的 0.3.0 corpus 仍保持已记录的 canonical bytes 与 digest。

Core 负责 Lane 权限判定，并在每个 Lane 命令前从当前 runtime mode 刷新权限状态。
所有有副作用的命令都按真实 worktree 或 repository target 判定；审批预览会红写 command、
arguments、environment、input 和 diff payload。重启时，处于 starting、running 或等待
审批状态的 Lane 会恢复为 blocked recovery fact，并继续绑定其持久化 session owner。

审批响应与 permission/mode 变更共用 supervisor command queue，因此排队中的权限降级会
先于 Lane mutation 恢复生效。审批产生的 session/repository allow rule 会在常规权威权限
刷新后保留，但 Plan/ReadOnly 刷新会立即丢弃这些 rule。Create 与 Lane 状态迁移和其他
持久化 effect 使用同一 permission 与 mutation-policy gate。终态 worker 通过 completion
reaper 自动注销并 join，不需要等待下一条 Lane 命令。

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

每个 JSON envelope 包含：

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
