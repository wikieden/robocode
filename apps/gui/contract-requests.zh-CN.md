# GUI Core 合同请求

英文版：[contract-requests.md](contract-requests.md)

这些请求来自 GUI `0.1.0-alpha.1` 对 Core `0.3.0` 的清单盘点。它们不是 GUI
绕行方案。在 Core 补齐缺失事实/命令前，请求中点名的生产屏保持阻塞。Framework-neutral、
fixture-only 的 Task 2-3 仍可做 replay harness 和只读 projection，但不得创建私有业务
reducer、直接调用 runtime、独立持久化偏好，或伪造成功状态。

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
| `GUI-CORE-001` | P0 | Task 7 D11 项目接入 | Core Task 11 | 增加 Task 11 已归属的 typed 项目接入基础：project probe、onboarding provider/model health summary、`viden.toml` preview/confirm、masked credential handles。 | Core `0.3.0` 已有 provider/model 配置和 health facts，但没有 typed project probe 或 config preview/confirm 流程。Recent history 与 starter-lane 创建分别拆到 `GUI-CORE-007`、`GUI-CORE-002`。 |
| `GUI-CORE-002` | P0 | Task 7 D11 starter lane；Task 8 D4 Lane 创建；Task 9 D1 lane rail/worktree board | Core Task 10 | 增加 typed lane lifecycle commands/events：按 role/route/gate/mutation/target/budget 创建 starter/operator lane、worktree preview/receipt、lane-created event、attach/pause/resume/cancel/close/restart/kill、owner-scoped lane command receipts。 | Core 已导出 `AgentLaneRecord` 和 lane update events，但没有把创建/管理 lane 作为一等 operator intent 的命令。Starter-lane onboarding 是 lane-creation preset，不是 Core Task 11 的 project fact。 |
| `GUI-CORE-003` | P0 | Task 10 D6 恢复 | Core Task 10 follow-up | 增加结构化 connection/lane-recovery facts/actions：connecting、disconnected、provider bridge dropped、agent stopped、restart from checkpoint、reconnect、close lane while keeping worktree、owner-scoped safe recovery receipts。 | CoreClient 可恢复 stream gap，Core 可发 generic `RuntimeErrorView`，但 Core Task 10 尚未点名完整的 D6 可操作 connection/recovery surface。Apply/conflict recovery 由 `GUI-CORE-006` 归 Core Task 12。 |
| `GUI-CORE-004` | P1 | Task 9 D1 audit pane；Task 12 history/audit | Core Task 13 | 增加稳定 append-only audit timeline：audit event id、source project/lane/task/session、permission decision、evidence/gate/applied-change links、分页查询。 | Core 已有 approvals、evidence、merge gates、transcript rows 和 generic command/tool records；GUI 仍需 Task 13 audit contract，不能解析 transcript 展示文本。Diff/apply facts 拆到 `GUI-CORE-006`。 |
| `GUI-CORE-005` | P0 | Task 6 生产偏好控件；Task 12 Settings | Core Task 4 follow-up | 增加 Core-owned locale/skin/mode/density/motion mutation 与 persistence commands/events，包括 restore defaults、resolved-preference confirmation、diagnostics 和遵守 precedence 的持久化存储。 | Core Task 4 已导出 typed/resolved preferences，但 `CoreClient` 没有 preference mutation/restore command 或事件确认的 persistence receipt。GUI 只能渲染 resolved preferences，并在 spike 使用 ephemeral 控件。 |
| `GUI-CORE-006` | P0 | Task 9 D1 diff/test panes；Task 12 可信本地闭环 | Core Task 12 | 增加结构化 diff/test/apply/conflict/retry facts 与 receipts，并链接 lane/task/session/evidence/gate/applied-change ids。 | Core 已有 evidence/MergeGate facts，但 Task 12 trust loop 仍需通过 `viden-core` 导出稳定 file/diff summary、test result、apply outcome、conflict bounce、retry/revert facts。 |
| `GUI-CORE-007` | P1 | Task 7 D11 recent work；Task 12 history navigation | 需新增 Core history task | 增加分页 recent-project/recent-session summary，包含稳定 project/session/lane ids、last-active order、availability state、resume intent/receipt。 | 当前 Core 计划没有 task 负责跨项目 recent-history query。Core Task 11 管项目接入，transcript paging 管已知 session 内的 rows；两者都不提供该 discovery surface。 |

## 非阻塞 partials

- Permission dock 现在可以使用 `ApprovalRequestView` 和 `RespondToApproval`。GUI 必须展示
  Core 的 risk/target/scope/default/audit facts，且不得直接执行底层 tool。
- Locale 与外观现在可以使用 `RuntimeSnapshot.ui_preferences`。GUI 渲染已解析偏好；
  Task 2-3 可使用 ephemeral fixture 控件，生产控件继续等待 `GUI-CORE-005`。
- D1 fixture replay 可以先基于现有 `d1-vertical-slice` facts 开始；任何生产命令缺口仍回到
  Core request。

## 关闭标准

只有当 Core 通过 `viden-core` 导出 typed command/event/fact、共享 fixture corpus 或兼容文档
记录该行为，并且 GUI 能仅通过 `CoreClient` 消费而不导入 runtime/provider/tool/session/workflow
internals 时，请求才能关闭。
