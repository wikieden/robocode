# ContextBundle 与 Token 效能设计

英文版： [context-bundle-token-efficiency.md](context-bundle-token-efficiency.md)

最后更新：2026-05-27

## 目标

RoboCode 的多 Agent 编排不能依赖“把完整 transcript 发给每个 agent”。
`ContextBundle` 是共享上下文模型：先作为设计落地，再于 `0.1.12` 首次接入 deterministic
lane envelope。

它的目标是：

- 让每个 agent 只拿到当前任务真正需要的上下文。
- 让主 TUI 能解释 context pressure、token budget 和上下文来源。
- 让多 agent 通过结构化 facts、artifacts 和 evidence 协作，而不是互相复制长对话。

## ContextBundle 字段

最小字段：

- `task`：当前用户目标或子任务描述。
- `selected_files`：被显式选中或语义召回的文件。
- `diff`：当前工作区 diff summary 和关键 hunk 摘要。
- `diagnostics`：LSP、编译器或测试诊断。
- `test_results`：命令、退出码、耗时、失败摘要和输出 tail。
- `facts`：用户约束、设计决策、项目约定和可复用记忆。
- `lane_summaries`：Codex、Claude、DeepSeek、shell 等 lane 的状态摘要和 artifacts。
- `permissions`：本轮可执行动作、需要审批的动作和禁止越界的边界。
- `budget`：当前 agent 的 token budget、模型路由、成本上限和 context pressure。

## Tool Output Compaction

原始 transcript 仍然作为审计事实保留，但发送给模型和显示在 TUI 中时要压缩：

- 长日志保留失败摘要、命令、退出码、最后 N 行 tail。
- 重复输出按 hash 或相邻重复块去重。
- 测试失败优先保留 failing file、line、error message 和 rerun command。
- 大 diff 先保留 file summary、风险文件和关键 hunk；需要时再按文件展开。
- lane output 只进入 `lane_summaries`，除非用户显式 inspect 某条 lane。

## Per-Agent Token Budget

每个 agent lane 都应有独立 budget：

- `planner`：小上下文，偏目标拆解和约束提取。
- `coder`：中高上下文，优先文件、diff、diagnostics。
- `reviewer`：中上下文，优先 diff、tests、risk、requirements。
- `tester`：小上下文，优先命令、失败、rerun。
- `researcher`：独立 budget，避免污染编码上下文。

当 context pressure 过高时，优先级是：

1. 当前任务和用户约束。
2. 当前 diff 和失败证据。
3. 相关文件片段。
4. 最近 lane summary。
5. 历史 transcript 摘要。

## 0.1.12 Runtime Slice

`0.1.12` 中，ContextBundle v0 已在 delegated lane envelope 中变成真实对象：

- `/lane run` 和 `/lane ask <tool> <task>` 会把 ContextBundle 元数据写入 lane envelope。
- lane envelope 包含 context sources、estimated tokens、largest sources、compaction notes、
  soft budget、hard limit 和 context pressure。
- 长 test/tool/lane output 做 summary + tail；原始 transcript 和 lane logs 继续作为审计事实保存。
- side-2 可从 lane evidence 展示 context pressure。
- 主 provider prompt 路径本版本仍只记录 context pressure 可见性，暂不使用 ContextBundle 改写 prompt construction。

0.2.0 再让每个 agent turn 都能输出：

- `bundle_id`
- included sources
- estimated tokens
- compaction decisions
- budget remaining
