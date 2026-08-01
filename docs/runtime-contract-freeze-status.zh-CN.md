# Runtime Contract Freeze 状态

英文版：[runtime-contract-freeze-status.md](runtime-contract-freeze-status.md)

本文记录 Viden Runtime-first 重构的 core-only Phase 0-2 checkpoint。本阶段刻意不实现
新的 TUI 或 GUI 界面。

## 范围

本阶段包含：

- 前端无关 runtime schema；
- core runtime bridge 和 command bus；
- submit/cancel/approval 的非 UI supervisor 边界；
- process-plugin protocol 草案；
- 跨前端 fixture replay；
- 后续 UI 分支 gate 的文档化。

本分支不包含：

- TUI rendering 重写；
- GUI 实现；
- 视觉 parity 截图。

## 证据

| 要求 | 当前证据 | 状态 |
| --- | --- | --- |
| `viden-core` facade | `viden-core/src/lib.rs` 重导出 `RuntimeSupervisor`、`SessionEngine` 和 runtime contract 类型 | 已完成 |
| Runtime schema | `viden-types/src/runtime.rs` 定义 `RuntimeCommand`、`RuntimeEventKind`、`RuntimeViewState`、approval、evidence、provider health、cost、tool call、task、lane | 已完成 |
| Runtime replay reducer | `RuntimeViewState::apply_event` 和 `viden-types` 测试覆盖 snapshot、approval、task、queued input、lane、evidence、provider、cost facts | 已完成 |
| Core bridge | `SessionEngine::runtime_snapshot`、`runtime_view_state`、`runtime_events_for_engine_events`、`handle_runtime_command`、`process_runtime_input_with_approval` | 已完成 |
| Command bus | 测试覆盖 user input、queued follow-up、mode 切换、permission-level 切换、provider config、model selection、active model activate/deactivate | 已完成 |
| Plan mode mutation safety | 现有 permission 和 workflow 测试覆盖 plan mode 下 mutating tool denial 与 workflow task mutation denial | 已完成 |
| Supervisor boundary | `RuntimeSupervisor` 测试覆盖 active provider cancellation 和不耦合 TUI 的 approval response delivery | 已完成 |
| Permission/mode contract | `runtime_command_bus_covers_plan_build_review_permission_contract` 覆盖 plan/review/explore read-only，以及 build 恢复 ask | 已完成 |
| Core 发出 lane facts | `runtime_view_state_emits_lane_facts_from_core_store_legacy_lane_statuses` 与 `legacy_lane_migration_runs_once_at_resume_and_runtime_replays_typed_state` 证明 TSV 只迁移一次，随后由 `lanes.jsonl` replay 成 `LaneUpdated` runtime facts，Core 不再读取 TUI TSV | 已完成 |
| Provider/model、approval、lane、task、cost、evidence fixture | `crates/types/tests/fixtures/runtime-contract-phase2.json` 和 fixture replay 测试 | 已完成 |
| Process-plugin protocol 草案 | `docs/process-plugin-protocol.zh-CN.md` 和英文 counterpart | 已完成 |
| Thin TUI client proof | 因本阶段约束延后到 TUI client 分支；当前分支只证明共享 fixture 和 API 边界 | 延后 |
| GUI API proof | 已通过 runtime schema、fixture、GUI functional design 和 process-plugin 草案文档化；可执行 GUI client tests 等 GUI 分支 | 延后 |

## 验证快照

本分支最近本地检查：

```bash
cargo test -p viden-core
cargo test -p viden-types runtime_contract_fixture_replays_phase2_cross_frontend_facts -- --nocapture
cargo test -p viden-runtime runtime_command_bus_covers_plan_build_review_permission_contract -- --nocapture
cargo test -p viden-runtime runtime_view_state_emits_lane_facts_from_core_store -- --nocapture
cargo fmt --check
git diff --check
RUST_TEST_THREADS=1 cargo test --workspace --quiet
```

默认并发的 `cargo test --workspace --quiet` 也已通过。此前的 TUI lane test timing
flake 已通过测试 harness 修复：测试会等 failed lane 的 log tail 进入 summary 后再断言；
没有改变 TUI 产品行为。

## 下一步交接

后续分支从这个 contract boundary 继续：

1. Core 继续实现 context/token/cost 和 plugin runtime。
2. TUI client 分支消费 `viden-core` 和 runtime fixture，不再直接调用 core internals。
3. GUI 分支只在共享 fixture 足够支撑 parity tests 后启动。
