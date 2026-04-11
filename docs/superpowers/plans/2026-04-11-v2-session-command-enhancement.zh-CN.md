# V2 Session 与命令面增强实现计划

英文版： [2026-04-11-v2-session-command-enhancement.md](2026-04-11-v2-session-command-enhancement.md)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保持当前 shared engine 和 transcript 模型不变的前提下，扩展 RoboCode 本地 CLI 的 session 元数据、运行时检查命令和更完整的命令面。

**Architecture:** 这一阶段只增强现有 V1 crates，不新增平台级子系统。实现重点是：把 startup/config 状态传入 `SessionEngine`，增强 `SessionSummary` 和 SQLite-backed session indexing，并新增 `/status`、`/config`、`/doctor`，同时改进 `/sessions` 输出，但不能绕开现有 command / transcript 主路径。

**Tech Stack:** Rust、现有 RoboCode workspace crates、SQLite fallback 索引、REPL 命令处理

---

## 范围

包含：

- 在 CLI 内暴露 runtime status 与 loaded config
- 丰富 session summaries 和 session-list rendering
- 添加轻量环境诊断
- 改进 help text，让命令面更像产品而不是 demo

不包含：

- LSP
- MCP
- 新建 remote / agent / plugin 相关 crates
- 全量 TUI 重写

## 目标行为

- `/status` 展示 session id、cwd、provider、model、permission mode、transcript path、session-home / index 位置。
- `/config` 展示 resolved runtime config summary，以及实际参与合并的配置文件。
- `/doctor` 运行轻量诊断，检查本地依赖是否齐全，并清晰报告缺失项。
- `/sessions` 展示比现有 title 和 preview 更丰富的摘要信息。
- session indexing 存储足够元数据，使列表渲染不需要每次都重新打开 transcript。
- 所有新增命令与现有 slash commands 一样，会写 transcript command entries。

## 文件映射

**修改：**

- `robocode-cli/src/main.rs`
- `robocode-core/src/lib.rs`
- `robocode-session/src/lib.rs`
- `robocode-types/src/lib.rs`
- `robocode-config/src/lib.rs`
- `README.md`

**创建：**

- `docs/superpowers/plans/2026-04-11-v2-session-command-enhancement.md`

## Task 1：增加 runtime startup snapshot 类型

**Files:**
- Modify: `robocode-types/src/lib.rs`
- Modify: `robocode-config/src/lib.rs`
- Modify: `robocode-cli/src/main.rs`
- Modify: `robocode-core/src/lib.rs`

- [ ] 在 `robocode-types` 中增加一个共享 runtime snapshot 类型，供 `SessionEngine` 渲染 `/status` 和 `/config`。
- [ ] 至少包含：
  - cwd
  - provider family
  - model label
  - permission mode
  - resolved config summary string
  - loaded config file list
  - session home override 或 effective home path
- [ ] 从 CLI startup 把这个 snapshot 传入 `SessionEngine`，不要在 `robocode-core` 里临时拼装。
- [ ] 除测试所需的最小补充外，保持现有 startup banner 行为不变。
- [ ] 增加单元测试，验证即使 provider 后续切换 model label，`SessionEngine` 也能稳定渲染保存下来的 startup snapshot。

## Task 2：增强 session summary 元数据

**Files:**
- Modify: `robocode-types/src/lib.rs`
- Modify: `robocode-session/src/lib.rs`

- [ ] 扩展 `SessionSummary`，增加：
  - message count
  - tool-call count
  - command count
  - last activity kind
  - last activity preview
- [ ] 更新 transcript summarization 逻辑，在单次扫描中派生这些字段。
- [ ] 更新 SQLite schema 和 upsert 路径，把这些字段落入索引。
- [ ] 保持向后兼容：
  - 容忍旧版 SQLite 表结构
  - 当索引缺失或过期时，回退到 project-directory transcript 扫描
- [ ] 增加测试覆盖：
  - 纯 JSONL fallback
  - 携带新字段的 SQLite index update
  - 混合 message / command / tool result 的 sessions

## Task 3：改进 `/sessions` 与 `/resume` 体验

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-session/src/lib.rs`

- [ ] 更新 session-list 渲染，让摘要信息更丰富，但不显得杂乱。
- [ ] 保持对以下 selector 的支持：
  - `/resume latest`
  - `/resume #<index>`
  - `/resume <session-id-prefix>`
- [ ] 让 `/sessions` 清楚标记当前 session，并显示 last activity kind。
- [ ] 保证 prefix 歧义时仍能返回有帮助的列表视图。
- [ ] 增加测试：
  - current-session marker 渲染
  - richer list formatting
  - ambiguous prefix error messaging

## Task 4：新增 `/status` 与 `/config`

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-cli/src/main.rs`
- Modify: `robocode-types/src/lib.rs`

- [ ] 新增只读命令 `/status`，完全由 engine state 渲染。
- [ ] 新增只读命令 `/config`，基于 startup snapshot 和 resolved config summary 渲染。
- [ ] `/status` 至少包含：
  - session id
  - cwd
  - provider family
  - model
  - permission mode
  - transcript path
  - session home
- [ ] `/config` 至少包含：
  - config summary string
  - loaded config files 或 `<none>`
  - 显式应用过的 startup overrides
- [ ] 增加命令测试，验证它们会出现在 `/help` 中，并写入 transcript command logs。

## Task 5：新增轻量 `/doctor`

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-cli/src/main.rs`

- [ ] 新增轻量命令 `/doctor`，报告这些依赖的可用性：
  - `git`
  - `rg`
  - `sqlite3`
  - `curl`
- [ ] 每个依赖只报告 `ok`、`missing` 或在确有必要时报告 `not required for current path`。
- [ ] 不做破坏性或联网诊断。
- [ ] 输出保持简单、适合终端。
- [ ] 增加测试，通过命令可用性 shim 或 helper functions 做确定性验证。

## Task 6：刷新 help 和文档

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `README.md`

- [ ] 更新 `/help` 输出，加入 `/status`、`/config`、`/doctor`。
- [ ] 让 help text 按用途分组，而不是平铺的混合列表。
- [ ] 更新 `README.md` 命令示例，纳入新的 runtime-inspection commands。

## Task 7：验证与收尾

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-session/src/lib.rs`
- Modify: `README.md`

- [ ] 实现过程中运行聚焦测试：
  - `cargo test -p robocode-session`
  - `cargo test -p robocode-core`
- [ ] 最终运行全量验证：
  - `cargo test --workspace`
- [ ] 在 CLI 中做 smoke-check：
  - `cargo run -p robocode-cli -- --provider fallback --model test-local`
  - `/status`
  - `/config`
  - `/doctor`
  - `/sessions`
  - `/resume latest`
- [ ] 只有在命令输出稳定后再更新文档。

## 验收标准

- CLI 暴露 `/status`、`/config`、`/doctor`
- session list 比当前 V1 输出拥有更丰富的元数据
- session indexing 仍然可以从 transcript files 重建
- 新命令没有绕开 transcript logging
- 所有现有和新增测试全部通过

## 后续工作

这份计划完成后，下一份详细计划应从以下中选择：

- LSP foundation
- memory 和 task workflows
- richer TUI 与 structured diff views
