# Native/ACP 交互夹具一致性

English version: [fixture-parity.md](fixture-parity.md)

## 结果

Core 0.3.5、TUI 0.3.3 与 GUI 0.1.0-rc.3 集成候选版本：通过。

唯一的源夹具是
`crates/types/tests/fixtures/frontend-contract-v1/interaction-closed-loop.json`。
前端没有维护副本。该夹具包含 22 个有序 runtime 事件，最终 cursor 为
`fixture:interaction-closed-loop` sequence `22`。Core canonical replay
测试会校验最终 `RuntimeViewState` digest：

```text
46db05abaaae36cf37cb7ffa0493a4ef8c158a2d5b4ffeef08d01dbf8e284ed0
```

## 投影事实

| 界面 | 已验证结果 |
| --- | --- |
| Core | 有序 replay、gap recovery、最终 cursor 和 canonical view digest 均与提交的夹具一致。 |
| TUI | `TuiClientDriver` 应用全部 22 个事件；Board、Gallery 和 Decisions 渲染出准确的 Lane、evidence、gate 和 recovery 事实。测试同时观察 built-in 与 ACP session start、tool start/finish、approval request/resolution 以及 follow-up/retry receipt。 |
| GUI | `GuiCoreAdapter` 消费相同的 22 个事件，并在 cursor 22 发布最终 D1 projection。投影包含准确的 Lane/receipt，且不会从 session 或 receipt 显示数据推测 Lane execution owner。 |

两个前端测试共同校验以下 ID：

- Lane `lane-loop-coder` 和 preview `preview-loop-coder`；
- built-in session `session-loop-built-in` 和 ACP session
  `session-loop-acp`；
- tool call `tool-loop-test`；
- approval `approval-loop-tool`；
- evidence `evidence-loop-test`；
- merge gate `gate-loop-apply`；
- conflict path `src/lib.rs`；
- recovery action `action.revalidate_merge_conflict`；
- accepted follow-up `agent-input-loop-follow-up`。

该夹具刻意不包含 cost 事件。因此本门禁记录 cost 事件为缺失（`0`），不声称
已经覆盖 cost projection 一致性。

夹具同时包含 built-in 与 ACP session-start receipt。Core 的
one-Lane/one-Agent reducer 会保留该 Lane 的第一个 execution identity。由于夹具没有
发布 `LaneRuntimeOwnerBound`，GUI 会正确地不显示 owner-scoped Agent 卡片，而不会从
任一 session 或 starter-Lane receipt 推测权限所有者。

## 复现

运行仓库门禁：

```bash
bash scripts/native-acp-fixture-parity.sh
```

门禁要求以下三个证明各有且仅有一个通过的测试：

```text
viden-core  interaction_closed_loop_fixture_replays_identically_after_a_gap
viden-tui   tui::app::tests::native_acp_fixture_render
viden-gui   native_acp_fixture_projection
```
