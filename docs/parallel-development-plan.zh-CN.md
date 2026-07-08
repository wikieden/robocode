# Viden 并发开发计划

英文版：[parallel-development-plan.md](parallel-development-plan.md)

## 目的

这份计划定义 Viden 如何进入大型 Runtime-first 重构，并支持未来最多三个人或 agent
并发开发。

核心规则：

> 先做结构重构。只有 runtime contract 足够稳定、能避免 TUI/GUI 复制业务逻辑后，
> 才开始并行开发 TUI 和 GUI。

## 目标开发形态

Viden 正在迁移为 Runtime-first 平台：

- `viden-core` 是公开核心 facade，承载 runtime、编排、context、permissions、
  evidence、cost、tasks、lanes 和 extension contracts。
- TUI 和 GUI 是产品客户端。它们渲染状态并发送命令，不拥有 provider loop、tool
  execution、permission decision 或 task state。
- 扩展通过声明式 plugin boundary 接入，不能绕过 runtime 的 permission/evidence 路径。
- Viden 在迁移期保留为 legacy compatibility 名称；当前产品、文档、UI 和新架构方向统一为 Viden。

## 当前 Core Contract 基线

第一段 Phase 0-1 contract 切片只落在 core 层：

- `viden-types` 拥有前端无关 schema：
  - `RuntimeSnapshot`
  - `RuntimeEvent` / `RuntimeEventKind`
  - `RuntimeCommand`
  - `CommandAction`
  - `ApprovalRequestView`
  - `EvidenceView`
  - `ProviderHealthView`
  - `TokenCostView`
  - `RuntimeViewState`
- `RuntimeViewState::apply_event` 是 contract tests 使用的 replay reducer。
  后续 TUI 和 GUI 必须消费这种可 replay 的事实流，而不是重新创建私有业务状态。
- `viden-runtime` 暴露第一版 compatibility bridge：
  - `viden-core`
  - `SessionEngine::runtime_snapshot()`
  - `SessionEngine::runtime_view_state()`
  - `SessionEngine::runtime_events_for_engine_events(...)`
  - `SessionEngine::handle_runtime_command(...)`
  - `RuntimeSupervisor`
- `viden-core` 是稳定的客户端导入边界，目前重导出 runtime/control contract，
  不增加 UI 依赖。这个 bridge 会把现有 `EngineEvent` 输出，以及 provider health、context、
  token/cost、task facts 投影到共享 runtime contract。
- 第一版 process-plugin protocol 草案是
  [process-plugin-protocol.zh-CN.md](process-plugin-protocol.zh-CN.md)。第一版
  跨前端 runtime fixture 是
  `crates/types/tests/fixtures/runtime-contract-phase2.json`。
- 当前 core-only Phase 0-2 审计见
  [runtime-contract-freeze-status.zh-CN.md](runtime-contract-freeze-status.zh-CN.md)。
- command bus 当前支持提交用户输入、排队 follow-up 输入、切换 work mode、切换
  permission level、在当前 provider 下选择 model、provider configuration，以及
  active-model 编辑。提交输入过程中触发的 approval prompt 会被捕获为
  `ApprovalRequested` / `ApprovalResolved` 事件。`RuntimeSupervisor` 提供第一版异步
  core path，可以在不耦合 TUI 的情况下取消运行中的 provider turn，并投递 approval response。
- 本阶段不实现新的 TUI 或 GUI 界面，只建立后续界面必须使用的 API 边界。

## 阶段计划

### Phase 0：架构切分

在大规模改动前，冻结目标 workspace 结构、依赖方向、公开 runtime contract、plugin
protocol 形态和迁移策略。

交付物：

- `viden-core` facade 设计
- UI model contract：`RuntimeSnapshot`、event stream、command actions、approval
  requests、evidence views 和 UI contribution model
- process-plugin protocol 草案
- Viden rename 与 Viden compatibility 迁移计划
- TUI/GUI contract-test fixture 计划

### Phase 1：核心结构重构

先做结构和边界。这个阶段避免大规模 TUI 视觉重写，也不启动 GUI 实现。

交付物：

- 引入 `viden-core` facade，或先通过 compatibility exports 分阶段落地
- 从 TUI-owned state 中抽出 runtime supervisor 和 event stream
- 为用户输入、mode 切换、approval、provider/model setup、cancel、queued
  follow-up、tool/lane facts 建立 command bus
- mutation 前的 permission check 统一进入 core
- task、lane、evidence、cost、context、provider health、transcript facts 全部由 core runtime 发出
- core-only Phase 0-2 分支不实现 TUI 或 GUI；后续 TUI client 分支必须消费 runtime
  facts，而不是自己拥有业务状态

### Phase 2：契约冻结

在并发 UI 开发前，冻结第一版可用的跨前端 contract。

必须满足：

- runtime snapshots 和 events 的 core replay tests 通过
- permission/mode contract tests 覆盖 plan/build/review 行为
- provider/model setup、approval、lane、task、cost、evidence fixtures 存在
- 薄 TUI client 可以只依赖共享 contract 运行，不直接调用业务内部；在本次 core-only
  Phase 0-2 分支里，这一点用 runtime fixture replay 表达，真正薄 TUI client 在后续
  TUI client 分支实现
- GUI 需要的 API 已文档化，并有 schema 或 fixture tests 覆盖

### Phase 3：TUI 与 GUI 并发开发

contract freeze 后，把工作拆到独立 branch/worktree。

推荐分支归属：

| Branch | Owner | Scope |
| --- | --- | --- |
| `codex/viden-core-runtime` | Core owner | Runtime contracts、plugin protocol、migration、bugfix |
| `codex/viden-tui-client` | TUI owner | Terminal rendering、keyboard/input、panes、scrollback、status、errors |
| `codex/viden-gui-tauri-client` | GUI owner | Tauri + Web cockpit、settings、agent board、evidence、approval、provider/model |

规则：

- TUI 和 GUI 分支不能直接调用 provider、tool、permission、transcript 或 workflow 内部 API。
- 共享 contract 变化必须先在 core 分支加测试，再让 UI 分支 rebase 或 merge。
- TUI 和 GUI 可以在布局和交互细节上不同，但同一 fixture 必须显示同一套 runtime facts。
- UI plugin contribution 必须是声明式的。插件可以贡献 panels、settings、commands 和
  cards，但不能修改 UI 内部状态。

### Phase 4：集成与发版

合并顺序：

1. Core/runtime branch
2. TUI client branch
3. GUI client branch

发版闸门：

- full workspace tests
- runtime replay 和 permission/mode tests
- plugin manifest/capability tests
- TUI/GUI parity fixture tests
- deterministic TUI previews 和 GUI screenshots
- 真实 DeepSeek development smoke，并记录 token、费用、耗时和失败分类
- Viden binary/config migration tests
- Viden compatibility shim tests
- GitHub Release 与 Homebrew tap 作为一个发版单元验证

## 并发规则

- 使用 `.worktrees/<branch-name>` 下的独立 worktree。
- 架构层最多三个 active owner：core、TUI、GUI。
- 大文件拆分应在 Phase 1 完成，避免 UI 分支分叉后冲突放大。
- 共享 contract 先改测试，再改实现。
- UI 分支必须频繁 rebase 或 merge core 分支；长期 UI 分支不能为了推进而发明私有 runtime state。
- 文档变更跟随行为 owner。面向用户的文档需要英文和中文一起更新。

## 版本映射

- `0.2.0`：架构切分与核心结构重构。
- `0.2.1`：Context、token/cost、evidence 和 runtime fact model。
- `0.2.2`：受监督多 Agent 执行闭环。
- `0.2.3`：plugin runtime、process-plugin protocol 和真实开发 gate。
- `0.3.0`：多前端 contract freeze 与 Viden migration plan。
- `0.3.1`：TUI 与 GUI 并行实现分支。
- `0.3.2`：TUI/GUI parity 集成候选版。
- `0.3.3`：可操作 GUI beta 与 Viden compatibility migration hardening。
- `0.3.4`：视觉保真和生产发版 gate。
