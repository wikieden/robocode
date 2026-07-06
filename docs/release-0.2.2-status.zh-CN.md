# Viden 0.2.2 状态 - Agent DAG 与 Role Runtime 收口

English version: [release-0.2.2-status.md](release-0.2.2-status.md)

`0.2.2` 内容已在当前 working tree 中完成。这是核心 runtime checkpoint，
不是 GitHub/Homebrew 分发发布。

## 状态

- 范围：Agent DAG 与 role runtime。
- 产品名：Viden。
- Release publication：未发布。
- Distribution gates：只有真正创建 GitHub Release 时才适用。

## 已完成范围

- `StartAgentDag`、`StartAgentTask` 和 `CancelAgentTask` runtime commands。
- planner、coder、reviewer、tester、doc-writer、release-operator 和
  external-agent 的受监督 role records。
- 带 dependency gating 的 role task execution。
- AgentTask-bound ContextBundle events，包含 role guidance、file scope、
  evidence contract、scoped file candidates、symbol candidates 和 live LSP
  diagnostics。
- provider-backed task outputs 会写入 `task.result`，并链接到 role evidence。
- queued、started、blocked、completed、failed、cancelled agent tasks 的 durable
  workflow events。
- failure classification、recovery suggestions 和 retry next actions。
- MergeGate creation、accept/reject commands、artifact accept/reject commands、
  accepted patch merge command，以及 durable conflict reporting。
- scoped role permission policy，覆盖 read-only、docs-only、tester
  verification、scoped coder mutation、release-gate、external-agent denial、
  scoped `git_add`、越界 staging denial 和高风险 Git denial。
- 带 success 和 exit-code 字段的 structured tool-result runtime events。
- RuntimeSupervisor cancellation paths，取消后 worker 不会卡死。

## 验证

- PASS `cargo fmt --all --check`
- PASS `git diff --check`
- PASS `cargo test -p viden-runtime`
- PASS `cargo test --workspace --quiet`

live DeepSeek test 默认仍为 ignored，因为它需要 `DEEPSEEK_API_KEY`、网络访问和
实际计费 provider usage。

## 延后到后续 0.2.x

- Live LSP references enrichment。
- scoped staging 和高风险 mutation denial 之外的 release/publish Git rules。
- Evidence collection reducers。
- rename/delete/binary patch handling 和 three-way conflict resolution。
- external-agent plugin adapters。

这些内容作为 `0.2.3+` 工作记录在 staged roadmap 中，不阻塞 `0.2.2` Agent DAG
与 role runtime 收口。
