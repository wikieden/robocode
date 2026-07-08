# Agent Workflow Visibility

英文版：[agent-workflow-visibility.md](agent-workflow-visibility.md)

状态：产品与前端交互方案。

本文定义 Viden 如何把一个正在运行的 agent workflow 清晰解释给用户。目标不是让用户读原始
日志，而是让用户一眼看懂编排状态。

## 用户问题

每个 workflow 视图都必须回答六个问题：

1. 后续 agent 计划做什么？
2. 每个 agent 现在正在做什么？
3. 哪些工作已经做完？
4. 哪些已经完成验收，可以安全 merge、apply 或 publish？
5. 为什么这项工作分配给这个 agent、tool、MCP 能力或 skill？
6. 当前分工对成本和预算有什么影响？

这些是产品级问题。TUI、GUI、CLI status output 和未来 API clients 都应该从同一套
runtime facts 回答。

## 核心概念：Mission Control

把 workflow 表达成一个由 `AgentDagRecord`、`AgentTaskRecord`、`EvidenceView` 和
`MergeGateRecord` 支撑的 Mission Control board。

主视图分成五个区域：

| 区域 | 用户含义 | Runtime 来源 |
| --- | --- | --- |
| Assignment | 为什么这样拆任务，以及每部分由谁负责 | DAG planner output、agent capability/cost profile |
| Plan | 后续计划和依赖顺序 | queued `AgentTaskRecord`、DAG dependencies |
| Now | 正在工作的 agents 和当前步骤 | running task status、activity、active tool/provider events |
| Done | 已完成但可能仍需 review 的产物 | completed tasks、produced artifacts、evidence ids |
| Acceptance | 证据 checklist 和 merge/release decision | `MergeGateRecord`、required evidence、accepted artifacts |
| Blocked | 缺少审批、证据失败、冲突或等待用户输入 | blocked/failed tasks、errors、next actions |
| Cost | 预算、已花费和预计剩余成本 | token/cost records、provider/model metadata |

board 不应显示假的百分比。优先使用真实 phase、evidence counts 和 timestamps，而不是猜测
进度。

## 状态模型

使用小而稳定的状态词：

| 状态 | 含义 | 用户动作 |
| --- | --- | --- |
| `planned` | 任务已存在，但尚未准备运行 | 查看 scope、编辑计划、启动 |
| `queued` | 依赖或 scheduler 正在等待 | 调整顺序、取消、查看 blocker |
| `running` | agent 正在推理或调用工具 | 观察、取消、追加输入 |
| `waiting_approval` | 需要用户决策 | approve、deny、修改 scope |
| `collecting_evidence` | 已有输出，检查/review 仍在运行 | 等待、查看 checklist |
| `done` | agent 已完成分配任务 | 查看输出和 evidence |
| `needs_changes` | review、测试或用户拒绝结果 | 请求修改或 retry |
| `accepted` | required evidence 已满足 | 根据场景 merge/apply/publish |
| `merged` | accepted change 已应用 | 查看最终 diff 和 transcript |
| `failed` | 任务失败并有分类原因 | retry、换 provider、缩小 scope |
| `cancelled` | 用户或 runtime 取消任务 | resume、retry 或 archive |

## Task Card 契约

每张可见 task card 应展示：

- role 和 agent：`planner`、`coder`、`reviewer`、`tester`、`doc-writer`、
  `release-operator`、外部 ACP agent、MCP tool 或 skill；
- objective：一句话，不展示整段 prompt；
- scope：文件、worktree、repo 区域或外部系统；
- status 和 current activity；
- dependency blockers 和 upstream tasks；
- next action；
- evidence checklist 摘要，例如 `tests 1/1`、`review 0/1`、`patch 1/1`；
- 可用时显示 cost 和 duration；
- assignment reason：为什么选择这个 role/agent/tool/skill；
- cost profile：cheap/default/premium/manual、budget cap 和 current spend；
- artifact links：patch、docs、logs、screenshots、release assets 或 MCP output。

## 分工协作视图

workflow 必须在整体层面展示分工，而不只是展示单个 agent 的状态。用户需要理解多个
agents 如何在工程里协作。

Assignment view 应展示：

- task owner：role、具体 agent、provider/model、MCP server/tool 或 skill；
- assignment reason：专长匹配、文件归属、上下文局部性、已有 evidence、成本、延迟或用户偏好；
- collaboration pattern：sequential handoff、parallel fan-out、reviewer/tester fan-in
  或 manual approval gate；
- scope boundary：files、directories、worktree、external system 或 read-only research scope；
- expected output：plan、patch、test evidence、review findings、docs、release artifact
  或 diagnostic report；
- dependency links：解锁当前 task 的上游任务，以及等待它的下游任务；
- budget：预计 tokens/cost、已花费和最大允许花费。

示例：

```text
Assignment
  planner       Viden core      cheap model     split task, low mutation risk
  coder-a       Codex ACP       premium         Rust runtime patch, high code skill
  tester        local tools     free            run cargo tests after coder-a
  reviewer      Claude ACP      premium         architectural review, risk-focused
  doc-writer    Viden core      cheap model     update bilingual docs

Collaboration
  planner -> coder-a -> tester
                  \-> reviewer -> acceptance
  coder-a -> doc-writer after behavior is accepted

Budget
  estimated $0.42, spent $0.11, remaining $0.31
```

assignment reason 是一等产品文本。如果 Viden 无法解释为什么任务被分配给某个 agent，
scheduler 应显示 `not reported`，不能隐藏这个决策。

## 成本感知编排

分工必须同时考虑能力和成本。

调度输入：

- capability fit：coding、planning、review、tests、docs、release、research；
- context fit：哪个 agent 已经拥有相关上下文或 session；
- tool fit：某项工作是否用 local tool/MCP/skill 比再次调用 LLM 更便宜且更安全；
- risk：mutation level、permission scope、required evidence；
- latency：预期等待时间，以及并行是否值得；
- cost：model/provider price、token budget、免费的本地工具替代方案和 workflow 剩余预算。

成本展示规则：

- 可用时展示 workflow 的 estimated、spent 和 remaining cost；
- 区分 LLM/provider 花费与 local tool time；
- 解释节省成本的替代方案，例如“tester 使用 local cargo test，而不是 premium model”；
- 当 cheaper role/tool 可能足够时，标记 premium-agent use；
- 允许用户选择 `fast`、`balanced`、`cheap`、`high-confidence` 等策略预设。

成本不能覆盖安全。便宜 agent 如果缺少权限、上下文或必要能力，不能接收该任务。

## 详情视图

选择 task 后打开详情视图，包含六个 section：

1. **Objective**：原始目标、non-goals、role assignment、scope。
2. **Assignment**：owner、reason、capability fit、cost fit、dependencies。
3. **Plan**：planner 或 workflow template 生成的 substeps。
4. **Activity**：live provider/tool/MCP/skill events，合并节流后按顺序展示。
5. **Artifacts**：patches、docs、reports、logs、screenshots、release assets。
6. **Evidence**：tests、reviews、diagnostics、approvals、merge gate state。
7. **Next Action**：retry、revise、approve、merge、cancel、archive。

详情视图在完成后也必须保留历史。已完成 workflow 仍然是可 replay 的决策记录。

## 交互模式

### 启动 Workflow

当用户启动较大目标时，Viden 应先显示生成计划，再开始并行工作：

```text
Workflow: Refactor provider config
Plan
  1. planner: define scope and risks
  2. coder: update config loader
  3. tester: run config and runtime tests
  4. reviewer: inspect diff and missing cases
  5. doc-writer: update provider docs
Acceptance
  patch, test_result, review, doc_update
```

用户可以批准计划、编辑 scope 或只启动部分任务。

### 执行中

可见文字要具体但紧凑：

```text
Now
  coder      editing crates/config/src/lib.rs
  tester     queued, waiting for coder
  reviewer   planned

Blocked
  doc-writer needs accepted behavior summary
```

### 有输出后

完成和验收必须分开：

```text
Done
  coder      produced patch, 3 files changed
  tester     cargo test -p viden-config passed

Acceptance
  patch        1/1
  test_result  1/1
  review       0/1
  doc_update   0/1
  status       collecting_evidence
```

只有 Acceptance 区域能说明工作是否 ready。

## TUI 表达

TUI 应优先使用高密度、稳定的区域：

- 顶部 active strip：当前 workflow、active task 数、blocked 数、accepted gates、budget status；
- side rail：`Assignment`、`Plan`、`Now`、`Done`、`Acceptance`、`Blocked`、`Cost`
  counts 或 summaries；
- main transcript：重要 workflow events 和用户可读摘要；
- task detail panel：选中 card 的 evidence checklist 和 next action；
- workflow state 不能只存在于 modal 中；关闭 modal 不能隐藏 active work。

## GUI 表达

GUI 可以使用更完整的 Mission Control layout：

- 左侧：workflow/DAG tree 和 filters；
- 中间：Assignment、Plan、Now、Done、Acceptance、Blocked board columns；
- 右侧：选中 task detail，包含 activity timeline 和 evidence checklist；
- 底部：cost/time/status timeline 和 assignment rationale。

GUI 的 drill-down 不能修改 runtime state：filters、grouping、sort order、pinning 都只是本地
UI state。

## Runtime 要求

runtime 必须提供足够事实，让 UI 不需要推断：

- task status 和 current activity；
- task assignment owner 和 assignment reason；
- task plan steps 和 dependency ids；
- collaboration pattern 和 fan-out/fan-in edges；
- active provider/tool/MCP/skill step；
- evidence checklist requirements 和 collected evidence ids；
- acceptance state 和 latest decision；
- blocker classification 和 recovery suggestion；
- 可用时提供 token/cost/duration；
- 可用时提供 workflow budget、task estimate、task spend 和 cost strategy；
- 可 replay 的稳定 event ordering。

如果事实缺失，UI 应显示 `unknown` 或 `not reported`，不能编造进度。

## 验收标准

visibility model 可接受的条件：

- 用户不用读日志就能回答六个 workflow 问题；
- planned、running、completed、accepted 状态有清晰视觉区分；
- completed 不等于 accepted；
- workflow 和 task 级别都能看到 agent assignment reason 和 cost impact；
- blocked tasks 显示具体 next action；
- TUI 和 GUI 都从同一份 `RuntimeViewState` 渲染；
- replay workflow events 可以重建同一份 board state。
