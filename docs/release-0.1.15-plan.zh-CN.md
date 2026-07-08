# Viden 0.1.15 计划

English version: [release-0.1.15-plan.md](release-0.1.15-plan.md)

最后更新：2026-05-29

## 定位

`0.1.15` 是 **Context Curator And Budget Controls** 版本。

`0.1.14` 让 delegated lane 更可信：adapter capability doctor、timeline
evidence、isolation declaration、Codex 只读 review、Claude/tmux setup 安全检查。
下一步瓶颈是 context 控制。Viden 需要让操作者知道每个 provider 或 delegated
lane 到底看到了什么、哪些内容被省略、为什么省略，以及当前任务造成了多少预算压力。

本版本的产品问题是：

> 每个 agent 正在使用什么上下文，Viden 省略了什么，以及这次任务的 token/budget
> 压力有多大？

## 目标

- 把 ContextBundle 从简短状态字符串提升为受 policy 管理的事实模型。
- 让 source priority、budget pressure、omitted-source reason 和 compaction
  notes 在 provider turn 与 lane envelope 中可见。
- 为后续 pin/omit 控制打基础，但本版本不做复杂 UI。
- 即使 model-facing context 被压缩或省略，也保留原始 transcript、tool、test 和
  lane logs 审计数据。

## P0 范围

### ContextBundle v1 Policy Records

- 增加共享字段：
  - `policy`
  - source `priority`
  - source `include_reason`
  - `omitted_sources`
  - omission `reason`
- main provider turns 和 lane envelopes 共用 `v1-priority-budget` policy。
- 在 `AgentTask` evidence rows 和 provider context message 中显示 policy。

验收：

- JSON roundtrip 测试覆盖新的共享 record shape。
- Provider context message 显示 policy 和 omitted-source sections。
- Lane envelope 显示 `ContextBundle v1`、source priority、policy 和 omitted
  sources。

### Context Visibility Commands

- 增加一个紧凑的 operator command，用于查看最新 provider ContextBundle。
- 按 priority 和 token estimate 展示 sources。
- 展示 omitted sources 和 compaction notes。
- 保持只读。

验收：

- 用户不用读 raw transcript JSON，也能检查最新 provider bundle。
- 没有 bundle 时给出明确下一步。

### TUI Evidence Rows

- 在 side-2 ops context 中显示 context policy 和 omitted-source count。
- 保留现有 context pressure rows。
- visible rows 完成后补确定性截图证据。

验收：

- side-2 能一眼回答 policy、pressure、source count 和 omitted count。
- screenshot regression 包含更新后的 side-2 视图。

## P1 范围

- source pin/omit command 设计和文档。
- per-lane soft/hard budget overrides。
- retry lineage 记录 prior omissions 和 previous failure context。
- 长时间 delegated lane 的 budget stop evidence。

## 非目标

- 完整 spec workflow。
- hook runtime。
- mutating ACP/MCP/plugin/skill invocation。
- 自动 multi-agent task splitting。
- marketplace/install UX。

## 验证

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- ContextBundle focused tests：JSON shape、provider message rendering、lane
  envelope rendering、side-2 rows。
- `VIDEN_TUI_SCREENSHOT_VERSION=0.1.15 scripts/tui-regression.sh docs/previews/generated`
