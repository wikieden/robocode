# GUI Core 合同请求

英文版：[contract-requests.md](contract-requests.md)

这些请求来自 GUI `0.1.0-alpha.1` 对 Core `0.3.0` 的清单盘点。它们不是 GUI
绕行方案。在 Core 补齐缺失事实/命令前，GUI 可以做 replay harness 和只读 projection，
但不得创建私有业务 reducer、直接调用 runtime，或伪造成功状态。

## 当前可用 Core 表面

- Transport 与兼容性：`CoreClient`、`CoreTransport`、`StatefulCoreClient`、
  `LocalCoreTransport`、`CoreHandshake`、`CORE_CLIENT_CAPABILITIES`、schema `1`、
  snapshot、replay、event receive、transcript paging。
- Runtime projection：`RuntimeSnapshot`、`RuntimeEvent`、`RuntimeEventEnvelope`、
  `RuntimeViewState`、`RuntimeErrorView`、
  `RuntimeSnapshot.ui_preferences: ResolvedUiPreferences`。
- 当前可用命令：`SubmitUserInput`、`QueueFollowUp`、`CancelActiveTurn`、
  `SetWorkMode`、`SetPermissionLevel`、`RespondToApproval`、provider/model 配置和选择、
  `StartAgentDag`、`StartAgentTask`、`CancelAgentTask`、merge/evidence commands、
  `RetrieveContext`、`LoadTranscriptPage`。
- 当前可用事实：tool calls、approvals、queued inputs、typed tasks、typed lanes、
  evidence、merge gates、context/cost facts、provider health、token cost、UI preferences、
  transcript pages、generic runtime errors。

## 开放请求

| ID | 优先级 | 阻塞 GUI task | Core owner | 请求 | 当前缺口 |
| --- | --- | --- | --- | --- | --- |
| `GUI-CORE-001` | P0 | Task 7 D11 项目接入 | Core Task 11 | 增加 typed project intake commands/events：project probe、recent projects/sessions、onboarding provider/model health summary、`viden.toml` preview/confirm、masked credential handles、starter-lane intent。 | Core `0.3.0` 已有 provider/model 配置和 health facts，但没有 project probe、config preview/confirm、recent project/session surface 或 starter-lane onboarding command。 |
| `GUI-CORE-002` | P0 | Task 8 D4 Lane 创建；Task 9 D1 lane rail/worktree board | Core Task 10 | 增加 typed lane lifecycle commands/events：按 role/route/gate/mutation/target/budget 创建 lane、worktree preview/receipt、lane-created event、attach/pause/resume/cancel/close/restart/kill、owner-scoped lane command receipts。 | Core 已导出 `AgentLaneRecord` 和 lane update events，但没有把创建/管理 lane 作为一等 operator intent 的命令。 |
| `GUI-CORE-003` | P0 | Task 10 D6 恢复 | Core Task 10/12 | 增加结构化 connection/recovery facts/actions：connecting、disconnected、provider bridge dropped、agent stopped、restart from checkpoint、reconnect、close lane while keeping worktree、safe recovery receipts。 | CoreClient 可恢复 stream gap，Core 可发 generic `RuntimeErrorView`，但 GUI 没有 D6 可操作恢复态的 typed contract。 |
| `GUI-CORE-004` | P1 | Task 9 D1 驾驶舱；Task 10 permission/D6 evidence | Core Task 13 | 增加稳定 audit timeline 与 diff/apply file facts：audit event id、source lane/task/session、file/diff summary、test result、permission decision、evidence link、分页查询。 | Core 已有 approvals、evidence、merge gates、transcript rows 和 generic command/tool records；D1 audit/review panes 仍需要稳定 timeline contract，不能解析 transcript 展示文本。 |

## 非阻塞 partials

- Permission dock 现在可以使用 `ApprovalRequestView` 和 `RespondToApproval`。GUI 必须展示
  Core 的 risk/target/scope/default/audit facts，且不得直接执行底层 tool。
- Locale 与外观现在可以使用 `RuntimeSnapshot.ui_preferences`。GUI 渲染已解析偏好；
  以后开放本地控件时，也必须通过 Core-owned configuration intents 持久化。
- D1 fixture replay 可以先基于现有 `d1-vertical-slice` facts 开始；任何生产命令缺口仍回到
  Core request。

## 关闭标准

只有当 Core 通过 `viden-core` 导出 typed command/event/fact、共享 fixture corpus 或兼容文档
记录该行为，并且 GUI 能仅通过 `CoreClient` 消费而不导入 runtime/provider/tool/session/workflow
internals 时，请求才能关闭。
