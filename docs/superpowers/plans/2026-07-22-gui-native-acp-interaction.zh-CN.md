# GUI 原生与 ACP 交互实施计划

> **面向智能体执行者：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 发布 GUI `0.1.0-rc.2`：用类似 Zed 的轻量 `+` 菜单创建原生 Lane 或委派 ACP，并在 D1 内完成 Core 管理的会话闭环。

**架构：** 与 TUI 一样从不可变 Core `0.3.4` SHA 开始。D1 始终是应用主壳；轻量菜单和任务输入替代 D4 作为常规入口，Tauri adapter 只翻译 typed Core 命令和投影。D4 代码保留为兼容入口，不再承担默认流程。

**技术栈：** TypeScript、DOM、CSS tokens、Tauri 2、Rust `viden-core` adapter、Vitest、Cargo 测试。

## 全局约束

- 目标版本严格为 GUI `0.1.0-rc.2`，基于不可变 Core `0.3.4` SHA。
- 菜单在 `NEW LANE` 下显示 `Viden Agent`，在 `DELEGATE TO CURRENT LANE` 下显示 Core adapter。
- `Open project` 只打开文件夹，不进入 D11，也不询问模型或 Lane。
- 没有选中 Lane 或 Core 未报告 `Ready` 时，ACP 选项不可启动。
- Lane receipt 与后续原生/ACP 启动成功相互独立。
- 所有 mutation 经 Tauri `viden-core` 客户端并等待 typed event；禁止 GUI 私有 reducer、进程启动、最近项目或 readiness 推断。
- 去除白色外框；支持键盘、可见焦点、CJK IME、读屏、语言、皮肤、密度、字号和减少动态效果。

---

### 任务 1：给 Tauri adapter 增加 Core 加法 intent

**文件：** `apps/gui/src-tauri/src/d1.rs`、`adapter.rs`、`projection.rs`、`apps/gui/tests/d1_cockpit.rs`。

**接口：** 增加 `preview_default_lane`、`send_agent_session_input`、`retry_agent_session` intent；投影 `workspaceEligibility` 与 `startability`。

- [ ] **步骤 1：先写 Rust 测试：默认 Lane intent 直译为 `PreviewDefaultStarterLane`；ACP 续聊保留精确 session id**
- [ ] **步骤 2：运行 `cargo test -p viden-gui d1_default_lane_intent_sends_core_generated_preview_command d1_acp_follow_up_preserves_exact_session_id`，确认新 intent 不存在**
- [ ] **步骤 3：实现 Serde tagged intent 和一对一命令映射；直接投影 Core `startability`，不得重算 `canStart`**
- [ ] **步骤 4：运行 `cargo test -p viden-gui --test d1_cockpit`**
- [ ] **步骤 5：提交 `feat(gui): expose native and ACP D1 intents`**

### 任务 2：实现类似 Zed 的轻量 Agent 菜单

**文件：** 新建 `components/agent_menu.ts`、`agent_menu.css`；修改 D1 TS/CSS；新建 `tests/agent_menu.spec.ts`。

**接口：**

```ts
type AgentMenuSelection = { kind: "native" } | { kind: "acp"; agentId: string };
```

- [ ] **步骤 1：写 DOM 测试：原生与 ACP 分组正确；无选中 Lane 时 ACP 行 `aria-disabled="true"`**
- [ ] **步骤 2：运行 `npm --prefix apps/gui test -- agent_menu.spec.ts`，确认模块不存在**
- [ ] **步骤 3：使用 `role="menu"`、分组标签、roving tabindex、Up/Down/Home/End/Enter/Escape、外部点击关闭和焦点恢复；不可用 adapter 显示 Core 状态，不放模型选择器**
- [ ] **步骤 4：再次运行 `npm --prefix apps/gui test -- agent_menu.spec.ts`**
- [ ] **步骤 5：提交 `feat(gui): add compact agent menu`**

### 任务 3：在 D1 内完成原生 Lane 创建

**文件：** 新建 `lane_task_prompt.ts/css`；修改 `d1_cockpit.ts`、`main.ts`、D1 测试。

**接口：** 任务输入只返回 `{ task: string }`，不包含 Lane id、分支、worktree、Provider 或模型。

- [ ] **步骤 1：写顺序测试：用户提交后先发送 `preview_default_lane`，收到 Core receipt 前不发 `submit_user_input`，receipt 后聚焦新 Lane 再提交**
- [ ] **步骤 2：运行 `npm --prefix apps/gui test -- d1_cockpit.spec.ts -t "waits for the Core Lane receipt"`**
- [ ] **步骤 3：按资格→preview→create→receipt→submit 推进；Provider 启动失败保留 Lane 并在 D1 显示恢复；D1 常规入口不再导航到 D4**
- [ ] **步骤 4：运行 `npm --prefix apps/gui test -- d1_cockpit.spec.ts`**
- [ ] **步骤 5：提交 `feat(gui): create native Lanes from D1`**

### 任务 4：在 Lane 内委派并续聊 ACP

**文件：** 新建 `agent_session_switcher.ts`；修改 D1 TS/CSS 与测试。

**接口：**

```ts
type FocusedConversation =
  | { kind: "native"; laneId: string }
  | { kind: "acp"; laneId: string; sessionId: string };
```

- [ ] **步骤 1：写测试：选择 Codex 后发送绑定当前 Lane 的 `start_agent_session`；聚焦 ACP 后 composer 发送精确 `send_agent_session_input`**
- [ ] **步骤 2：运行 `npm --prefix apps/gui test -- d1_cockpit.spec.ts -t "ACP"`，确认当前 D1 无 ACP 路由**
- [ ] **步骤 3：ready adapter 选择后复用任务输入；在 Lane 下显示活动/最近子会话；会话选择只切换已有 transcript；续聊、重试、取消均按精确 session；委派 ACP 时绝不创建 Lane**
- [ ] **步骤 4：运行 `npm --prefix apps/gui test -- d1_cockpit.spec.ts agent_menu.spec.ts`**
- [ ] **步骤 5：提交 `feat(gui): complete ACP conversations in D1`**

### 任务 5：对齐视觉、多语言、无障碍和恢复状态

**文件：** 中英文 i18n、`window_chrome.css`、`d1_cockpit.css`、D1 与视觉测试。

- [ ] **步骤 1：写断言：无白色外框；菜单/任务/会话控制有可访问名称；中文不裁切；两套皮肤焦点可见；reduced motion 禁用菜单动画**
- [ ] **步骤 2：运行 `npm --prefix apps/gui test -- visual_shell.spec.ts d1_cockpit.spec.ts`，确认新快照尚不存在**
- [ ] **步骤 3：只使用设计 token；按 Core 事实显示 connecting、需安装、需认证、Provider/工具/上下文错误、取消、重试、回放恢复**
- [ ] **步骤 4：运行 `npm --prefix apps/gui test && npm --prefix apps/gui run build`**
- [ ] **步骤 5：提交 `fix(gui): align agent interaction states`**

### 任务 6：发布 GUI 0.1.0-rc.2 证据

**文件：** `src-tauri/Cargo.toml`、`tauri.conf.json`、`Cargo.lock`、中英文 GUI 功能设计。

- [ ] **步骤 1：版本改为 `0.1.0-rc.2`，记录轻量菜单和 D4 兼容入口**
- [ ] **步骤 2：运行 `cargo test -p viden-gui && npm --prefix apps/gui test && npm --prefix apps/gui run build && cargo test --workspace --quiet && git diff --check`**
- [ ] **步骤 3：运行 `npm --prefix apps/gui run tauri build`，确认 `apps/gui/src-tauri/target/release/bundle/macos/Viden.app` 存在并打开 D1**
- [ ] **步骤 4：提交 `chore(gui): release native and ACP interaction rc.2`**
