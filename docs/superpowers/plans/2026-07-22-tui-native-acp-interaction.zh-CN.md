# TUI 原生与 ACP 交互实施计划

> **面向智能体执行者：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 发布 TUI `0.3.3`：常规方式创建原生 Lane，通过系统命令 `/acp` 选择、启动、恢复、聚焦和取消 Core 管理的 ACP 会话。

**架构：** 从不可变 Core `0.3.4` SHA 创建分支。选择器只保存 TUI 展示状态；可启动性、所有者、进度、对话、审批、结果和恢复全部来自 `RuntimeViewState`。

**技术栈：** Rust、crossterm、Viden 现有画布与组件、`viden-core`、JSON i18n、确定性 TUI 预览。

## 全局约束

- 目标版本严格为 TUI `0.3.3`，基于记录的 Core `0.3.4` SHA。
- `n` 创建 Viden 原生 Lane；`/acp` 委派给当前 Lane。
- 无当前 Lane 时 `/acp` 必须显示原因且不可启动。
- 列表同时显示 Core 发布的 adapter 与活动/最近 ACP 会话；方向键移动、Enter 选择、Esc 返回。
- 流式输出时输入框仍可编辑，忙时输入通过 Core 排队；`Ctrl-C` 只取消当前精确目标。
- 不允许 TUI 私有持久化、就绪推断、进程启动或 Lane reducer；中英文文案同步。

---

### 任务 1：注册 `/acp` 并与原生 Lane 创建分离

**文件：** `command_palette.rs`、`app.rs`、`state.rs` 及内联测试。

**接口：** 新增 `InteractionPanel::AcpPicker { selection, phase }`；`n` 使用独立 `NewLaneTask`。

- [ ] **步骤 1：先写测试：选中 Lane 后 `/acp` 打开 picker；`n` 只打开原生任务输入**
- [ ] **步骤 2：运行 `cargo test -p viden-tui acp_command_opens_picker_only_for_selected_lane native_new_lane_does_not_open_acp_picker`，确认新 variant 不存在**
- [ ] **步骤 3：`/acp` 发送 `QueryAgentAdapters`；`n` 后续发送 `PreviewDefaultStarterLane { Coder }`，不得复用 ACP picker**
- [ ] **步骤 4：运行 `cargo test -p viden-tui command_palette acp_command native_new_lane`**
- [ ] **步骤 5：提交 `feat(tui): separate native Lane and ACP commands`**

### 任务 2：实现键盘优先的 ACP adapter/会话选择器

**文件：** `modal.rs`、`input.rs`、`state.rs`、`i18n/en.json`、`i18n/zh-CN.json`。

**接口：** 稳定行 id 为 `session:<session_id>` 和 `adapter:<agent_id>`。

- [ ] **步骤 1：写测试：会话位于 adapter 前，认证/安装/不可用状态按 Core 文案显示**
- [ ] **步骤 2：运行 `cargo test -p viden-tui acp_picker_lists_sessions_before_adapters`，确认失败**
- [ ] **步骤 3：渲染 `ACTIVE / RECENT SESSIONS` 与 `AVAILABLE ACP AGENTS`；Ready 进入任务输入，ProbeRequired 发送 probe，其他状态只显示 Core 诊断；Esc 按任务输入→picker→关闭退出**
- [ ] **步骤 4：运行 `cargo test -p viden-tui acp_picker modal input`**
- [ ] **步骤 5：提交 `feat(tui): add ACP session picker`**

### 任务 3：完成原生 Lane 创建与首任务

**文件：** `app.rs`、`modal.rs`、`projection.rs`。

- [ ] **步骤 1：写命令顺序测试：先默认 preview；收到 receipt 之前绝不发送 `SubmitUserInput`；收到 receipt 后聚焦新 Lane 再提交任务**
- [ ] **步骤 2：运行 `cargo test -p viden-tui native_lane_task_waits_for_receipt_before_submitting`**
- [ ] **步骤 3：显示 Core 工作区资格；按 preview→create→receipt→submit 顺序推进。后续 Provider 失败只进入该 Lane 的恢复状态，不回滚 Lane**
- [ ] **步骤 4：运行 `cargo test -p viden-tui native_lane starter_lane`**
- [ ] **步骤 5：提交 `feat(tui): complete native Lane first task`**

### 任务 4：路由 ACP 任务、续聊、聚焦、重试与精确取消

**文件：** `app.rs`、`composer.rs`、`state.rs`。

**接口：** `FocusedConversation::{NativeLane(AgentLaneId), AcpSession(SessionId)}` 仅为展示状态。

- [ ] **步骤 1：写测试：聚焦 ACP 后输入发送 `SendAgentSessionInput`；`Ctrl-C` 发送精确 `CancelAgentSession`**
- [ ] **步骤 2：运行 `cargo test -p viden-tui focused_acp_composer ctrl_c_targets_focused_acp_session`，确认当前错误路由到原生 turn**
- [ ] **步骤 3：adapter 选择后收集任务并启动；会话选择只切换 transcript lens；续聊、重试、取消按当前精确会话发送；Running/WaitingApproval 时仍允许输入**
- [ ] **步骤 4：运行 `cargo test -p viden-tui agent_session composer cancel queue`**
- [ ] **步骤 5：提交 `feat(tui): control focused ACP conversations`**

### 任务 5：渲染完整状态、证据与恢复

**文件：** `render.rs`、`side_screen.rs`、`projection.rs`、中英文 i18n。

- [ ] **步骤 1：基于 Core 夹具写画布断言：Agent 名称、会话状态、工具进度、审批、结果证据、Provider 错误、回放恢复均可见**
- [ ] **步骤 2：运行 `cargo test -p viden-tui native_acp_fixture_render`，确认缺失标签失败**
- [ ] **步骤 3：复用现有面板与 glyph；审批动作留在审批面板；诊断限行；Cancelled 与 Failed 分开；恢复状态来自 Core cursor**
- [ ] **步骤 4：运行 `cargo test -p viden-tui && scripts/tui-previews.sh`**
- [ ] **步骤 5：提交 `feat(tui): render native and ACP lifecycle`**

### 任务 6：发布 TUI 0.3.3 证据

**文件：** `apps/tui/Cargo.toml`、`Cargo.lock`、中英文 TUI 功能设计文档。

- [ ] **步骤 1：版本改为 `0.3.3`，记录 `/acp`、`n`、聚焦、重试和取消**
- [ ] **步骤 2：运行 `cargo test -p viden-tui && scripts/tui-turn-controller-smoke.sh && scripts/rc-tui-stability-smoke.sh && scripts/tui-regression.sh && cargo test --workspace --quiet && git diff --check`**
- [ ] **步骤 3：提交 `chore(tui): release native and ACP interaction 0.3.3`**
