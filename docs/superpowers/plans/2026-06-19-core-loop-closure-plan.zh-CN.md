# 核心编程循环闭环补全计划（2026-06-19）

针对 RoboCode 核心 agent 循环
（`SessionEngine::process_input_with_approval_and_control`，
`robocode-core/src/runtime_loop.rs`）七个闭环缺口的补全计划。每个缺口映射到分阶段、
代码级、带 TDD 与发布闸的工作，纳入既有 `0.2.x` 路线图。

本计划为行为级，不逐文件移植 `.ref/claude-code-main`。

## 问题——七个缺口（证据）

| # | 缺口 | 证据 |
|---|------|------|
| 1 | 多步自治未闭：固定 8 次迭代上限 + break-on-text 启发式 | `runtime_loop.rs:79`（`for _ in 0..8`）、`:181`（`if !observed_tool_call \|\| observed_text { break }`） |
| 2 | token/成本未入循环：按字符压缩；`ContextBundleRecord` 的 token 字段仅展示；usage 仅 telemetry；无 tool-result 去重 | `runtime_loop.rs:20`、`:647` `fit_provider_request_budget`；`robocode-types/src/lib.rs:294-318`；`runtime_loop.rs:461` |
| 3 | 无验证闭环：仅 post-edit LSP 诊断；不自动 build/test/lint，无失败分类，无 done 闸 | `runtime_loop.rs:373-387`、`:185`（无条件置 Done）、`:286`（失败只变普通 Tool 消息） |
| 4 | 单体 coder 循环；无 planner/coder/reviewer/tester/doc 监督角色 | 缺失；lane 是委派终端，非角色参与者 |
| 5 | 事件/状态未统一：同步 `Vec<EngineEvent>` + 并行 `upsert_agent_task`；provider 事件整收（无真流） | `runtime_loop.rs:42`、`:100` `next_events_with_control` 返回 `Vec`、`:84/:161` upsert |
| 6 | Plan mode 仅是权限闸，非 plan→approve→execute 循环构造 | `robocode-permissions` plan-mode 测试；循环不产出计划工件 |
| 7 | 工具同步执行阻塞循环；request-too-large 单发重试 | `runtime_loop.rs:276`（同步 `tools.execute`）、`:78/:113-135`（`retried_request_too_large` bool） |

已闭（勿重造）：权限-先于-mutation、tool 结果回灌 transcript、post-edit LSP 诊断注入、
413 单次压缩重试、append-only JSONL 审计 + resume。

## 需保持的不变量（每阶段）

- 所有模型工具调用与命令副作用走共享 runtime 路径。
- 权限检查发生在 mutation 之前。
- JSONL transcript 为正本、append-only；SQLite 派生/可重建。
- Plan mode 阻断 mutating 的 workflow/file/shell/Git/memory/task 变更。
- 助手建议的 memory 需显式确认。
- 行为变更走 TDD；同一变更集内同步更新双语文档。

## 阶段

依赖顺序：**A → (B, C) → D → E**。A 解锁全部。映射路线图：
A、D ⊂ `0.2.0`；B ⊂ `0.2.1`；C、E ⊂ `0.2.2`；闸 ⊂ `0.2.3`。

### 阶段 A — 闭合迭代循环 + 事件接缝（缺口 #1，#5 的接缝）

改动最小、解锁最大；C/D/E 的前置。

**改动**
- A1. 替换终止条件。删除 `observed_text` break。新规则：只要刚处理的 provider 轮次含
  ≥1 个 `ModelEvent::ToolCall` 就继续；当某轮无工具调用（纯文本 / `Done`）才停。同一轮
  可同时有助手文本与工具调用（Anthropic 交错），文本不再终止轮次。
- A2. 用 `TurnBudget { max_tool_iterations, soft_token_budget, wall_clock }`（经
  `robocode-config` 解析）替换固定 `0..8`。耗尽时发 `TurnEvent::BudgetExhausted
  { reason }` 并暂停等显式继续，而非静默 break。默认 `max_tool_iterations` ≈ 25；token
  闸在阶段 B 接入。
- A3. 事件接缝：引入 `TurnEvent`（`EngineEvent` 超集）推入单一 sink；`AgentTask` upsert
  由该 sink 派生。完整统一见 D3。

**测试（TDD，`robocode-core`）**
- `loop_continues_past_eight_tool_iterations`
- `assistant_text_with_tool_call_does_not_end_turn`
- `turn_stops_when_no_tool_call_emitted`
- `budget_exhaustion_emits_event_not_silent_break`

**退出闸**：fixture 任务（read→edit→shell→edit→text-done，>8 步）跑到完成；
`cargo test -p robocode-core`；workspace 测试绿。

### 阶段 B — 真实 token + 成本核算（缺口 #2）

**改动**
- B1. `robocode-model` 加 `TokenCounter` trait（按 provider；含启发式回退）。用真实计数填
  `ContextBundleRecord.estimated_tokens`。
- B2. `fit_provider_request_budget` 改为按 bundle 的 `soft_token_budget` /
  `hard_token_limit` gating；48k 字符常量降为回退。单一预算源，非两套。
- B3. tool-result 去重 + 语义压缩：按内容哈希折叠重复的相同读取；用简短结构化摘要替换纯
  中段字符截断。
- B4. 成本：累计 `ModelUsage` → `RuntimeSnapshot` → 可见成本面板；软预算压力回灌阶段 A 的预算闸。

**测试**
- `estimated_tokens_within_tolerance_on_fixtures`
- `request_gating_uses_bundle_token_budget`
- `duplicate_file_reads_collapse_in_request`
- `cumulative_cost_surfaces_in_snapshot`

**退出闸**：DeepSeek 413 复现不再在原阈值触发；focused + workspace 测试绿。

### 阶段 C — 验证闭环 + 计划循环（缺口 #3、#6）

**改动**
- C1. 把 `post_edit_diagnostics_message` 泛化为 `post_action_verification`：一轮内 mutating
  工具落定后，可选运行配置/探测出的验证集（`cargo test`/`clippy`/`fmt`，项目探测），
  权限受控、附 evidence、结果作为下一轮输入回灌。
- C2. `FailureClass`（compile / test-fail / denied / not-found / timeout）打到
  `AgentTask` 与 ToolResult；附 next-action 提示，让循环放手让模型修复而非停滞。
- C3. Done 闸：若配置了验证且上次运行失败，轮次以 `NeedsAttention` 收尾，而非 `Done`
  （`runtime_loop.rs:185` 变为有条件）。
- C4. 计划循环：plan mode 下循环产出 `PlanArtifact`（有序步骤），发出并暂存；批准后经正常
  路径执行队列步骤。Plan mode 不再只是 deny。

**测试**
- `failed_test_feeds_failure_class_and_loop_continues`
- `done_gate_flips_to_needs_attention_on_failing_verify`
- `plan_artifact_produced_queued_and_executed_on_approval`
- `plan_mode_still_blocks_mutations_until_approved`

**退出闸**：确定性 daily-loop 冒烟（request→edit→approve→verify→diff→evidence）自动闭合；
plan-mode 冒烟通过。

### 阶段 D — 异步工具 job + 完整事件闭环（缺口 #7、#5 收尾）

**改动**
- D1. 长工具（shell、web、test）改 job 模型：spawn、流式 tail 事件、循环不阻塞；经
  `ToolJobControl`（仿 `ModelRequestControl`）中断在途任务。
- D2. 单 bool 重试换成有界 `RetryPolicy`（413→压缩、瞬时→退避、限流→等待），分类且封顶。
- D3. 定稿单一 `RuntimeSnapshot` 事件流；TUI 订阅；移除双通道。provider 支持时真流式
  `AssistantText` token。

**测试**
- `long_shell_emits_tail_events_and_is_cancelable`
- `retry_policy_respects_caps_and_classes`
- `snapshot_stream_drives_headless_consumer`

**退出闸**：30s shell 工具保持不阻塞 + 可取消；streaming/scrollback 冒烟绿；TUI 预览重生成
（`scripts/tui-previews.sh`）。

### 阶段 E — 监督角色（缺口 #4）

**改动**
- E1. `Role` 抽象（planner/coder/reviewer/tester/doc-writer）作为监督式循环参与者，带类型化
  inputs/outputs/evidence/failure-class/next-action——构建于 A（循环）、C2（分类）、D3（流）。
- E2. Orchestrator 路由任务过各角色；lane（外部 Codex/Claude/shell）成为角色后端而非独立路径。
  reviewer 驳回回灌给 coder。

**测试**
- `task_flows_plan_code_test_review_with_per_role_evidence`
- `reviewer_rejection_loops_back_to_coder`
- `external_lane_serves_as_reviewer_role_backend`

**退出闸**：一个任务在共享 runtime 上完成 plan→code→test→review 且每角色带 evidence；
更新真实开发场景闸（`0.2.3`）。

## 纳入发布

- `0.2.0` — 阶段 A（循环闭合 + 事件接缝）与阶段 D（异步 job、完整事件流）。即路线图所述
  runtime 分层 / 事件闭环。
- `0.2.1` — 阶段 B（context/token/成本引擎）。
- `0.2.2` — 阶段 C（验证 + 计划循环）后接阶段 E（监督角色）。
- `0.2.3` — 加入强制发布闸：多步自治冒烟（A）、token/成本摘要（B）、自动验证 daily-loop（C）、
  异步工具/取消冒烟（D）、角色流冒烟（E）。

## 风险

- A1 终止改动可能跑飞 → 由 A2 预算 + 显式继续兜底。
- B token 计数因 provider 而异 → 启发式回退 + 容差测试。
- C 自动验证须保持权限受控 → deny 模式下绝不擅自跑 shell。
- D 异步重构触碰同步内核 → 先落事件接缝（A3），再由 D3 移除旧通道。
- E 最大；A/B/C/D 契约稳定前不开工。

## 验证与闸（每变更集）

编辑文件 `cargo fmt` → focused crate 测试 → 共享/发布相关改动跑 `cargo test --workspace
--quiet` → 视觉改动跑 TUI 预览 → 同步双语文档。诚实说明未测项。

## 进度日志

- **2026-06-19 — 阶段 A1 + A2（首切片）落地。** `robocode-core`：
  - `runtime_loop.rs`：终止条件从 `if !observed_tool_call || observed_text` 改为
    `if !observed_tool_call`——助手文本与工具调用同轮不再结束轮次，每个 tool 结果都回灌。
    固定 `0..8` 上限换成 `SessionEngine::turn_budget.max_tool_iterations`（默认 25）；耗尽时发
    `EngineEvent::System("RoboCode turn budget exhausted …")` 并把 provider task 标为暂停，不再静默 break。
  - `lib.rs`：新增 `TurnBudget { max_tool_iterations }`（默认 25）字段 +
    `set_max_tool_iterations()` / `turn_budget()`。
  - 测试（`tests/runtime_loop_tests.rs`）：
    `assistant_text_with_tool_call_in_same_turn_continues_loop`、
    `tool_loop_respects_iteration_budget_and_emits_event`（+ `AlwaysToolCallProvider`）。
  - 验证：`cargo test -p robocode-core` 129 过 / 1 ignored（live-deepseek 无凭证）；
    `cargo clippy -p robocode-core` 干净；`cargo fmt` 干净；workspace 闸 586 过 / 4 ignored。
  - 阶段 A 仍开放的后续：经 `robocode-config` 解析预算（当前仅默认 + setter）；真正的 pause/resume
    “继续”轮（当前切片只发事件 + 结束，未持久化可恢复的暂停轮）；墙钟 + token 闸（token 闸随阶段 B）；A3 事件接缝。
- **2026-06-19 — 阶段 C2（首切片）落地。** `robocode-core/runtime_loop.rs`：
  - 工具失败时 `classify_tool_failure()` 推出 `ToolFailureClass`
    （not_found / directory_target / compile_error / test_failure / timeout / other）
    并配 next-action 提示，作为 `EngineEvent::System` / transcript 消息回灌给模型，
    类别也记到 tool task evidence（`failure_class <x>`）。
  - 测试：`failed_tool_result_includes_failure_classification_and_next_action`；既有
    `failed_tool_execution_is_returned_to_provider_without_ending_turn` 仍过（现也带 directory_target 提示）。
  - 验证：`cargo test -p robocode-core` 130 过 / 1 ignored；clippy 干净。
  - `ToolFailureClass` 暂为私有字符串枚举；阶段 E 角色需程序化分支时再提升到 `robocode-types`。
  - 切片 A1/A2 + C2 已提交到分支 `core-loop-closure`（`main` 未动）：
    `cargo test --workspace` 587 过 / 4 ignored。
- **2026-06-19 — 阶段 A2 后续：预算改为配置解析。** 轮次迭代上限不再仅默认：
  - `robocode-config`：`ResolvedConfig.max_tool_iterations`（默认 25），文件键
    `max_tool_iterations`，环境变量 `ROBOCODE_MAX_TOOL_ITERATIONS`（均钳制 >= 1）。
  - `robocode-cli/main.rs`：启动时把 `resolved_config.max_tool_iterations` 应用到引擎
    （唯一生产站点；`tui/app.rs` 7 处为测试）。
  - 测试：`max_tool_iterations_defaults_and_resolves_from_file_and_env`（默认 / 文件 / 环境覆盖文件）。
  - 未做：`ResolvedConfig::summary()` 暴露该值（避免改动 summary 断言）；CLI flag（`CliOverrides`，
    现由文件 + 环境覆盖）；新键/环境变量的面向用户配置文档。
- **2026-06-19 — 阶段 C3（首切片）：预算暂停的诚实 done 闸。**
  `robocode-core/runtime_loop.rs`：命中迭代上限的轮次现在以 provider-task 状态 `Blocked`
  （+ "paused" result）结束，而非 `Done`——它未完成、待继续。真正完成的轮次仍是 `Done`。
  - 测试：`tool_loop_respects_iteration_budget_and_emits_event` 扩展断言 `Blocked`。
    （`runtime_loop_tests` 20 过。）
  - 范围说明：这是 C3 无歧义的那半（预算暂停）。“最后工具失败后模型停下”的 done 闸推迟——
    它需要 C1（自动验证）才能成为可靠信号而非启发式。
