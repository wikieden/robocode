# Native/ACP Interaction Fixture Parity

Chinese version:
[fixture-parity.zh-CN.md](fixture-parity.zh-CN.md)

## Result

PASS on the Core 0.3.5, TUI 0.3.3, and GUI 0.1.0-rc.3 integration
candidate.

The only source fixture is
`crates/types/tests/fixtures/frontend-contract-v1/interaction-closed-loop.json`.
No frontend copy is maintained. It contains 22 ordered runtime events and
finishes at `fixture:interaction-closed-loop` sequence `22`. Core's canonical
replay test verifies the final `RuntimeViewState` digest:

```text
46db05abaaae36cf37cb7ffa0493a4ef8c158a2d5b4ffeef08d01dbf8e284ed0
```

## Projected Facts

| Surface | Verified result |
| --- | --- |
| Core | Ordered replay, gap recovery, final cursor, and canonical view digest match the committed fixture. |
| TUI | `TuiClientDriver` applies all 22 events. Board, Gallery, and Decisions rendering exposes the exact Lane, evidence, gate, and recovery facts. The test also observes built-in and ACP session starts, tool start/finish, approval request/resolution, and follow-up/retry receipts. |
| GUI | `GuiCoreAdapter` consumes the same 22 events and publishes the final D1 projection at cursor 22. The projection contains the exact Lane/receipt and never invents a Lane execution owner from session or receipt display data. |

The shared IDs checked by both frontend tests are:

- Lane `lane-loop-coder` and preview `preview-loop-coder`;
- built-in session `session-loop-built-in` and ACP session
  `session-loop-acp`;
- tool call `tool-loop-test`;
- approval `approval-loop-tool`;
- evidence `evidence-loop-test`;
- merge gate `gate-loop-apply`;
- conflict path `src/lib.rs`;
- recovery action `action.revalidate_merge_conflict`;
- accepted follow-up `agent-input-loop-follow-up`.

The fixture deliberately contains no cost event. Consequently this gate records
cost events as absent (`0`) and makes no claim about cost projection parity.

The fixture contains both built-in and ACP session-start receipts. Core's
one-Lane/one-Agent reducer keeps the first execution identity for the Lane.
Because the fixture does not publish `LaneRuntimeOwnerBound`, GUI correctly
shows no owner-scoped Agent card instead of inferring authority from either
session or the starter-Lane receipt.

## Reproduction

Run the repository gate:

```bash
bash scripts/native-acp-fixture-parity.sh
```

It requires exactly one passing test for each proof:

```text
viden-core  interaction_closed_loop_fixture_replays_identically_after_a_gap
viden-tui   tui::app::tests::native_acp_fixture_render
viden-gui   native_acp_fixture_projection
```
