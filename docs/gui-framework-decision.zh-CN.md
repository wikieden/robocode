# Viden GUI 框架选型决策

English version: [gui-framework-decision.md](gui-framework-decision.md)

决策日期：2026-07-20

组件：`viden-gui 0.1.0-alpha.1`

状态：已选定 production baseline；正式 Tauri 客户端此后已完成 bootstrap
（见 [`apps/gui/README.zh-CN.md`](../apps/gui/README.zh-CN.md)）

## 记录恢复说明（2026-08-16）

本记录于 2026-07-20 写在 `gui-v0.1` alpha 线上，但没有随其余工作合入 mainline；
2026-08-16 从原始 commit `d9d5b26b` 恢复。下文 Reproduce 命令依赖的 alpha spike
harness（`apps/gui/spikes/**`）此后已被 `apps/gui/` 下的正式 Tauri 客户端取代，
完整重跑门禁需要 checkout 该 commit 的历史 alpha 树。机读证据记录、
`apps/gui/tools/` 下的 comparator 工具与 `apps/gui/framework-gate.toml`
选型记录作为不可变门禁证据保留。

## 决策

Viden 选择 **Tauri** 作为唯一正式 GUI 框架。GPUI 保留为 alpha 对比 spike，
不再启动第二套 production client。

本次选择严格执行既定 hard rule：GPUI 只有在所有必需 measurement 与 hard gate
全部通过时才可以成为正式框架；missing、partial 或 failed 都不等于 pass。可复现
comparator 共发现 16 个 GPUI blocker，因此选择 Tauri。

本决策不代表 Tauri 已达到发布条件。Tauri 同样保留下面列出的性能、可访问性、
跨平台、soak、打包、credential 与恢复证据缺口。Task 5 只能启动 Tauri 正式客户端；
后续 release gate 必须用真实证据关闭这些缺口。

## 证据摘要

| 门禁 | Tauri | GPUI | 实际验证范围 |
| --- | --- | --- | --- |
| 等价 D1 slice | Pass | Pass | 候选测试覆盖共享 roles、action log、projection hash、queue/cancel、approval、history、theme 与 focus 行为。 |
| 有序 events | Pass | Pass | 共享 adapter 对恰好 10,000 个 events 保持 identity 与顺序。 |
| Transcript 规模 | Partial | Partial | 共享 paging 覆盖恰好 50,000 rows；两个 framework renderer 都没有 bounded virtualization instrumentation。 |
| macOS build 与 launch | Pass | Pass | 两个 debug binary 均在 Darwin arm64 构建成功并保持运行 5 秒。 |
| Composer input p95 `< 50 ms` | Unavailable | Unavailable | 当前没有 input timing collector。 |
| Event-to-visible p95 `< 100 ms` | Unavailable | Unavailable | 当前没有原生 event-to-paint instrumentation。 |
| Frame work p95 `< 16 ms` | Unavailable | Unavailable | 当前没有 frame timing collector。 |
| 原生 CJK IME | Partial | Partial | Framework composition tests 通过，但没有采集操作系统 IME 注入。 |
| Keyboard-only | Partial | Partial | Focus traversal tests 通过，但没有完整 native-window 操作记录。 |
| Screen reader | Unavailable | Unavailable | 没有采集辅助技术运行证据。 |
| Linux 与 Windows build/launch | Unavailable | Unavailable | 本轮只实测本机 macOS。 |
| Bounded soak 与 near-zero idle CPU | Unavailable | Unavailable | 当前没有 soak 或 CPU sampler。 |
| 无长期 framework fork | Pass | Pass | 两个 spike 都使用发布版 framework package，仓库内没有 patch/fork。 |
| Visual parity | Unavailable | Unavailable | 没有采集可重复的 live D1 screenshot comparison。 |
| Signing、updater、credential storage、crash recovery | Unavailable | Unavailable | Alpha spike 尚未实现这些正式交付路径。 |

机读候选记录见 [Tauri evidence](../apps/gui/evidence/framework-gate/tauri.json)
与 [GPUI evidence](../apps/gui/evidence/framework-gate/gpui.json)。生成的 blocker 清单见
[framework gate decision evidence](../apps/gui/evidence/framework-gate/decision.md)，当前选择
记录在 [`apps/gui/framework-gate.toml`](../apps/gui/framework-gate.toml)。

## 复现

在仓库根目录运行：

```bash
apps/gui/tools/run-framework-gate.sh tauri crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json
apps/gui/tools/run-framework-gate.sh gpui crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json
python3 apps/gui/tools/compare-framework-gate.py \
  apps/gui/evidence/framework-gate/tauri.json \
  apps/gui/evidence/framework-gate/gpui.json
```

每个候选记录都包含 fixture digest、host/tool version、精确命令、exit code 与 output
tail。每条 D1 test command 会先验证传入文件与仓库已提交 fixture 逐字节一致，且
fixture ID 与 projection digest 相同；原生 launch 只有在完整 5 秒内持续存活才算
pass。Comparator 会拒绝任何引用失败命令的 pass 记录，Runner 会把未取得的证据
记录为 unavailable，不会用估算值替代。

## 影响

- Task 5 只创建 Tauri production shell。
- GPUI 保留为对比 spike，不再并行增加 production feature。
- Tauri 可以直接复用已接受的 `tokens.css` 与 GUI 组件资产。
- Core command/event/snapshot/replay 边界继续保持 framework-neutral；选型不会把业务状态
  移入 frontend。
- Tauri 仍缺失的门禁继续作为 release blocker，必须在 beta 或 stable 声明前于所需平台实测。

后续路线见 [GUI 功能设计](gui-version-functional-design.zh-CN.md) 与
[并发开发计划](parallel-development-plan.zh-CN.md)。
