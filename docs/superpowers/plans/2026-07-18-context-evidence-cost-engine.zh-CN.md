# Context、Evidence 与 Cost Engine 实施计划

> **供 Agent 执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐任务执行本计划。所有步骤使用 checkbox（`- [ ]`）追踪。

**目标：** 建设 Rust 原生 Context、Evidence 与 Cost Engine，为每个 AgentTask 提供按角色裁剪、可回溯的上下文，同时统计成本并保护 canonical evidence。

**架构：** 在 `crates/types` 稳定契约后增加中性的 `crates/context` 实现 crate。`crates/runtime` 负责 bundle 构造、预算执行、事件和 provider usage 集成；workflow/session JSONL 继续作为 durable canonical history。Headroom 仅能作为可选 plugin/MCP/benchmark adapter，不能成为原生执行依赖。

**技术栈：** Rust 2024、serde/serde_json、sha2、现有 JSONL workflow/session stores、`RuntimeSupervisor`、provider telemetry、shell/Python release-smoke scripts。

**设计文档：** [Context、Evidence 与 Cost Engine 设计](../specs/2026-07-18-context-evidence-cost-engine-design.zh-CN.md)

**英文详细计划：** [2026-07-18-context-evidence-cost-engine.md](2026-07-18-context-evidence-cost-engine.md)

---

## 交付顺序

1. `0.2.1-a`：稳定 contracts、canonical store、deterministic reducers 和 retrieval。
2. `0.2.1-b`：runtime bundle、budget enforcement、event replay 和 cost ledger。
3. `0.2.3`：Merge Gate canonical evidence verification。
4. `0.2.4` / `0.2.5`：可选 Headroom adapter boundary 和 DeepSeek A/B release gate。

本计划不实现 TUI/GUI。客户端工作只包括稳定 `RuntimeViewState` 和对接文档。

### 集成前置条件

当前 main 的 `apps/tui/Cargo.toml` 仍直接依赖 `viden-provider` 和
`viden-runtime`。Core/context tasks 可以并行推进，但 Task 10 和 release acceptance
会一直阻塞，直到 0.2.0 runtime-facade dependency cut 移除这些 UI 直连依赖。禁止为了
兼容当前状态而放宽 dependency guard。

## 文件边界

| 路径 | 职责 |
| --- | --- |
| `crates/types/src/context.rs` | 稳定 context、retrieval、quality 和 cost records。 |
| `crates/types/src/runtime.rs` | Runtime commands/events/view-state projection。 |
| `crates/context/src/store.rs` | Content-addressed canonical raw storage。 |
| `crates/context/src/reducer.rs` | Content routing、deterministic reducers、derivation records。 |
| `crates/context/src/cost.rs` | Append-only usage entries 和精确聚合。 |
| `crates/runtime/src/context_bundle.rs` | Role policy、bundle assembly、budget enforcement。 |
| `crates/runtime/src/runtime_contract.rs` | Commands、events、retrieval、cost projection 和 Merge Gate 集成。 |
| `crates/provider/src/transport.rs` | Provider usage/cache facts。 |
| `crates/plugin-api/src/lib.rs` | 可选 context-adapter capability descriptor。 |
| `scripts/context-engine-benchmark.sh` | Deterministic/live A/B benchmark。 |

## Task 1：稳定 Context 与 Cost Contracts

**文件：** `crates/types/src/context.rs`、`lib.rs`、`runtime.rs`、`tests.rs`、runtime contract fixture。

- [ ] 先写 `ContextItemRecord`、`ContextViewRecord`、`ContextHandleRecord`、
  `ContextRetrievalRecord`、`ContextQualityRecord`、`ContextBudgetRecord` 和
  `CostUsageRecord` 的 serialization/invariant failing tests。
- [ ] 运行 `cargo test -p viden-types context_contracts_round_trip_without_exposing_storage_paths -- --exact`，确认因类型缺失而失败。
- [ ] 增加 `ContextScope`、`ContextContentKind` 和上述 records；金额使用整数 micro-unit，未知 actual cost 使用 `None`。
- [ ] 增加 `RetrieveContext` command，以及 bundle/item/view/retrieval/budget/quality/cost/cache/evidence events。
- [ ] 扩展 `RuntimeViewState`，只投影受限摘要和聚合值，不暴露 raw secret 或 storage path。
- [ ] 运行 `cargo test -p viden-types`，确认全绿并更新 fixture。
- [ ] 提交：`feat: add context and cost runtime contracts`。

## Task 2：实现 Canonical Context Store

**文件：** 新增 `crates/context/Cargo.toml`、`src/lib.rs`、`src/store.rs`，更新 workspace manifest/lock。

- [ ] 先写重复内容 dedup、byte-identical retrieval、restart/reopen、hash mismatch 和 scope denial tests。
- [ ] 运行 `cargo test -p viden-context repeated_content_reuses_one_canonical_blob -- --exact`，确认 RED。
- [ ] 使用 SHA-256 content-addressed blobs；metadata append 到 `context-items.jsonl`；通过临时文件和 atomic rename 写入。
- [ ] Handle 不能包含 API key、credential 或 local storage path。
- [ ] 运行 `cargo test -p viden-context store::tests`，验证 corruption 返回 `HashMismatch`，跨 task 返回 `ScopeDenied`。
- [ ] 运行 `cargo test -p viden-types -p viden-context`。
- [ ] 提交：`feat: add canonical context store`。

## Task 3：实现确定性 Content Routing 与 Reducers

**文件：** `crates/context/src/reducer.rs`、`lib.rs`、crate manifest。

- [ ] 为 JSON、Rust source、unified diff、失败 test log 和 transcript 编写 golden failing tests。
- [ ] 测试必须覆盖 deterministic output、required markers、size bound、omissions 和 reducer version。
- [ ] 运行 `cargo test -p viden-context reducer::tests`，确认 RED。
- [ ] 实现 `native-v1` reducers：JSON 使用 `serde_json`；diff/log 使用结构化 line scanner；code v1 保留 imports/declarations/signatures/task-selected ranges；text 保留约束、决策和未决问题。
- [ ] `ReductionResult` 必须记录原始/压缩 token estimate、omissions、retained markers、reducer id/version 和 quality result。
- [ ] Parse 失败回退到 bounded raw；required marker 缺失返回 `QualityFailed`。
- [ ] 连续运行两次 reducer tests，确认 byte-identical。
- [ ] 提交：`feat: add deterministic context reducers`。

## Task 4：构造 Role-Scoped Bundles 并执行 Budget

**文件：** `crates/runtime/Cargo.toml`、`lib.rs`、`context_bundle.rs`、`runtime_contract.rs` 和 runtime tests。

- [ ] 编写 planner/coder/reviewer/tester source 差异、shared-handle dedup、hard-limit preflight rejection 和 replay tests。
- [ ] 使用 counting provider 证明 hard limit 时 provider request count 为 0。
- [ ] 将现有 `build_main_context_bundle` 接入 `ContextEngine`；保留迁移期 UI source summaries。
- [ ] Soft budget 执行 deterministic reduction 和 priority eviction；hard limit 在 transport 前拒绝。
- [ ] Provider 413 只允许使用 stricter policy 重建一次，第二次失败必须可见且不能继续 retry。
- [ ] 运行 `cargo test -p viden-runtime runtime_contract_tests runtime_supervisor_tests`。
- [ ] 提交：`feat: build reversible role context bundles`。

## Task 5：增加 Permission-Gated Retrieval

**文件：** runtime contract/supervisor tests 和 tools facade。

- [ ] 编写有效 retrieval、跨 task denial、secret exclusion、expired handle、missing item 和 cancellation tests。
- [ ] 运行 `cargo test -p viden-runtime retrieve_context`，确认 RED。
- [ ] Handle 只能在 runtime 内解析，读 bytes 前执行 scope 和 permission checks。
- [ ] Tool result 返回 bounded content；runtime event 只记录 reason/counts，不记录 raw body。
- [ ] 测试 retrieval 阻塞期间 `QueueFollowUp` 和 `CancelActiveTurn` 仍被接受，command loop 不锁死。
- [ ] 运行 `cargo test -p viden-runtime retrieve_context -- --nocapture`。
- [ ] 提交：`feat: add scoped context retrieval`。

## Task 6：实现 Provider-Aware Cost Ledger

**文件：** `crates/context/src/cost.rs`、provider transport/parse、runtime contract、live DeepSeek test。

- [ ] 编写 exact token sum、actual/estimated labels、cached tokens、retry attribution 和 task/DAG/workflow rollup tests。
- [ ] 证明未知 actual cost 保持 `None`，不能转换成 0。
- [ ] Provider telemetry 增加 optional cached-input 和 provider-reported cost facts。
- [ ] Pricing estimate 与协议解析分离，并记录 provider/model、price-table version、currency 和 `estimated=true`。
- [ ] 每次 provider attempt/retrieval append 一条 ledger entry，发送 cost/cache events。
- [ ] 运行 `cargo test -p viden-context -p viden-provider -p viden-runtime cost`。
- [ ] 提交：`feat: add task and workflow cost ledger`。

## Task 7：Merge Gate 强制 Canonical Evidence

**文件：** types、runtime contract/tests、workflow task reducer/tests。

- [ ] 编写 summary-only rejection、verified canonical acceptance、hash mismatch、missing source 和 restart replay tests。
- [ ] 运行 `cargo test -p viden-runtime merge_gate_rejects_summary_only_patch_evidence -- --exact`，确认 RED。
- [ ] Evidence 增加 canonical item id、bundle id、source hash、producer、permission snapshot id 和 verification state。
- [ ] Patch/test/review/doc/release evidence 只有在 canonical source 存在、hash 正确、scope 有效且 quality pass 时计入 gate。
- [ ] 失败进入 `blocked` 或 `needs_changes`，并带 machine-readable reason。
- [ ] 运行 `cargo test -p viden-workflows -p viden-runtime merge_gate`。
- [ ] 提交：`feat: verify canonical merge gate evidence`。

## Task 8：定义可选 Context Adapter Boundary

**文件：** plugin-api、plugin-host 及双语 plugin architecture docs。

- [ ] 编写 capability registration、version negotiation、timeout、malformed response、process absence 和 native fallback tests。
- [ ] 增加 `context_reducer` capability envelope；内容包括 kind、canonical hash、policy、reduced content、omissions、reducer id/version 和 quality facts。
- [ ] External reducer 默认关闭，必须显式配置，不能读取 credential/storage path。
- [ ] Timeout/crash/hash mismatch/quality failure 生成 health evidence 并回退 native reducer。
- [ ] 运行 `cargo tree | rg -i 'headroom|pyo3|python'`，production dependency 应无输出。
- [ ] 运行 `cargo test -p viden-plugin-api -p viden-plugin-host`。
- [ ] 提交：`feat: add optional context reducer capability`。

## Task 9：增加 Deterministic 与 DeepSeek A/B Gate

**文件：** 新增 benchmark script，修改 DeepSeek smoke、release gate、live test 和双语 testing plan。

- [ ] 先实现 fixture dry-run contract tests，缺字段、success/evidence 不一致或 token reduction 小于 20% 必须失败。
- [ ] 同一 disposable development scenario 分别运行 `VIDEN_CONTEXT_ENGINE=off/on`，release candidate 各三次。
- [ ] 每次记录 prompt version、provider/model、task/test result、evidence hashes、input/output/cached tokens、cost、first-token/total latency、retrieval、retry、compression ratio 和 failure class。
- [ ] Deterministic gate：`scripts/context-engine-benchmark.sh --fixtures crates/runtime/src/tests/fixtures/context-benchmark --out-dir /tmp/viden-context-benchmark`。
- [ ] Live gate：`scripts/context-engine-benchmark.sh --provider deepseek --model "${VIDEN_LIVE_DEEPSEEK_MODEL:-deepseek-v4-flash}" --runs 3 --out-dir /tmp/viden-context-live`。
- [ ] Live gate 结束必须汇总六次运行总 token、费用和耗时。
- [ ] 提交：`test: add context engine release benchmark`。

## Task 10：同步产品、架构与客户端契约

**文件：** `PLAN.md`、双语 PRD/architecture/roadmap/orchestration/UI collaboration docs。

- [ ] 每份 overview 只写 native-core 决策、版本归属、canonical evidence invariant、UI dependency rule 和专题文档链接。
- [ ] 对 TUI/GUI 写清 commands/events/view-state fields，禁止 UI 直接依赖 context/provider/tool/workflow internals。
- [ ] 新增 `scripts/check-doc-pairs.sh`、`scripts/check-doc-links.sh` 和
  `scripts/check-dependency-boundaries.sh`，支持显式路径/manifest 输入，并使用临时
  invalid/valid fixtures 验证失败和成功退出码。
- [ ] 对本次修改的双语 docs 运行 pair/link checks。
- [ ] 运行 `cargo fmt --all -- --check` 和 `cargo test --workspace --quiet`。
- [ ] 运行 `scripts/check-dependency-boundaries.sh`，确认 UI 依赖守卫通过。
- [ ] 提交：`docs: define context evidence and cost rollout`。

## 阻塞验收矩阵

| 领域 | 必须通过的验收标准 |
| --- | --- |
| 正确性 | Canonical retrieval byte-identical；hash corruption 和 scope violation 被拒绝。 |
| Context | 每个 provider-backed AgentTask 有 bundle id、role policy、handles、omissions 和 hard-limit result。 |
| Reduction | JSON/code/diff/log/transcript deterministic golden tests 全过；每个 view 有 reducer/version/omissions/quality。 |
| Cost | Provider token total 精确一致；未知 actual cost 保持 unknown；estimate 有 label/version。 |
| Evidence | Summary-only evidence 不能接受 Merge Gate；必须有 canonical source 和 permission snapshot。 |
| Runtime | 新状态可通过 events replay；build/retrieval 期间 composer/command loop 保持响应。 |
| Security | Secret/excluded source 不投影且不能跨 scope retrieve；不能绕过 permission。 |
| Architecture | TUI/GUI 不直接依赖 context/provider/tool/workflow internals；Headroom 不在 mandatory dependencies。 |
| Reliability | Missing adapter、corrupt store、reducer failure、provider 413、cancel 和 restart 都有 deterministic recovery tests。 |
| Performance | Release machine 上 fixture bundle build p95 不超过 200 ms，canonical retrieval p95 不超过 50 ms。 |
| Live quality | DeepSeek on/off 各三次，input-token 中位数至少下降 20%，task/test success 相同，evidence 完整，无新增 failure class。 |
| Release | Workspace tests、docs checks、dependency guard、deterministic/live benchmark、release gate、GitHub assets 和同步 Homebrew validation 全过。 |

## 完成证据

Release status 必须包含：精确 commit/branch、workspace test log、deterministic benchmark、
六次 DeepSeek A/B token/cost/duration summary、event replay fixture、dependency-boundary
结果、剩余风险和用户批准的 metric waiver。

任何阈值都不能静默豁免。豁免必须记录 owner、reason、affected metric、expiry release
和 follow-up task。
