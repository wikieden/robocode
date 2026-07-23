# Core 原生与 ACP 运行时实施计划

> **面向智能体执行者：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 发布 Core `0.3.4`，由 Core 唯一负责原生 Lane 创建，以及 ACP 的发现、精确会话启动、续聊、取消、持久化与恢复。

**架构：** 对冻结的前端合同只做加法扩展：Core 发布工作区可用性和 ACP 可启动状态，生成默认 Lane 标识，并只接受绑定到精确会话所有者的 ACP 后续输入。所有副作用由 runtime supervisor 执行并写入有序日志；TUI、GUI 只消费 `RuntimeViewState` 并发送命令。

**技术栈：** Rust、Serde 标签协议、JSONL 事件日志、`viden-types`、`viden-runtime`、`viden-core`、Cargo 测试。

## 全局约束

- 目标版本严格为 Core `0.3.4`。
- 保持 `RuntimeCommand -> ordered RuntimeEvent -> RuntimeViewState` 与 `frontend-contract-v1` 兼容。
- 每个 Lane 只有一个 Viden 原生主 Agent；ACP 是该 Lane 的委派子会话。
- Lane 创建成功不依赖后续原生 Provider 或 ACP 启动是否成功。
- 权限检查先于副作用；取消和续聊必须同时匹配 `RuntimeOwner.session_id` 与 `lane_id`。
- JSONL 是权威记录，SQLite 仍可重建；不得序列化凭据、ACP stderr 或环境变量值。
- 英文和中文合同文档同步更新。

---

### 任务 1：增加前端合同类型

**文件：**
- 修改：`crates/types/src/agent.rs`、`project.rs`、`runtime.rs`、`lib.rs`
- 测试：`crates/types/src/tests.rs`、`crates/core/tests/frontend_contract_v1.rs`

**接口：**
- 产出 `AgentStartability::{Ready, ProbeRequired, InstallRequired, AuthenticationRequired, Unavailable}`。
- 产出 `AgentSessionInput { session_id: SessionId, content: String }`。
- 产出 `WorkspaceEligibility { is_git_repository, has_head, can_create_lane, diagnostic }`。
- 增加 `PreviewDefaultStarterLane`、`SendAgentSessionInput`、`RetryAgentSession` 命令和 `WorkspaceEligibilityUpdated`、`AgentSessionInputAccepted` 事件。

- [ ] **步骤 1：先写失败的序列化和 reducer 测试**

```rust
let input = AgentSessionInput { session_id: SessionId("acp-7".into()), content: "继续失败测试".into() };
let command = RuntimeCommand::SendAgentSessionInput { input: input.clone() };
assert_eq!(serde_json::from_value(serde_json::to_value(&command).unwrap()).unwrap(), command);
```

- [ ] **步骤 2：运行并确认因新类型不存在而失败**

运行：`cargo test -p viden-types additive_agent_session_input_round_trips_and_reduces`

- [ ] **步骤 3：实现最小 DTO、Serde 默认值和 reducer**

`AgentAdapterView` 增加 `startability`；`RuntimeViewState` 增加 `workspace_eligibility` 和 `agent_session_inputs`，旧夹具必须仍可解码。

- [ ] **步骤 4：验证合同**

运行：`cargo test -p viden-types && cargo test -p viden-core --test frontend_contract_v1`

- [ ] **步骤 5：提交**

```bash
git add crates/types crates/core/tests/frontend_contract_v1.rs
git commit -m "feat(core): extend native and ACP frontend contract"
```

### 任务 2：发布真实工作区资格和 Core 生成的 Lane 默认值

**文件：** `crates/runtime/src/frontend_services.rs`、`runtime_contract.rs`、`starter_lane.rs` 及对应测试。

**接口：**
- `workspace_eligibility(cwd: &Path) -> WorkspaceEligibility`
- `default_starter_lane_request(cwd: &Path, preset: StarterLanePreset) -> Result<StarterLaneRequest, String>`

- [ ] **步骤 1：写非 Git 目录拒绝、有效 HEAD 通过、连续预览 id 唯一的失败测试**
- [ ] **步骤 2：运行 `cargo test -p viden-runtime default_lane_preview_ -- --nocapture`，确认命令尚未处理**
- [ ] **步骤 3：通过既有只读命令边界检查 `git rev-parse --is-inside-work-tree` 和 `git rev-parse --verify HEAD`；Core 生成 `lane-<12位小写十六进制>`、`viden/<lane-id>`、`.worktrees/<lane-id>`，不透传 stderr**
- [ ] **步骤 4：运行 `cargo test -p viden-runtime default_lane_preview_ workspace_eligibility_`**
- [ ] **步骤 5：提交 `git commit -m "feat(core): publish Lane workspace eligibility"`**

### 任务 3：让 ACP 发现和可启动状态真实可信

**文件：** `crates/runtime/src/agent_commands.rs`、`runtime_contract.rs`、ACP fixture 与测试。

- [ ] **步骤 1：写测试：initialize 成功必须为 `Available + Ready + AgentStartability::Ready`，未 probe 的已安装 Agent 必须是 `ProbeRequired`**
- [ ] **步骤 2：运行 `cargo test -p viden-runtime successful_initialize_probe_is_ready_to_start`，确认当前 `Unknown` 断言失败**
- [ ] **步骤 3：实现分类函数：Ready、ProbeRequired、InstallRequired、AuthenticationRequired、Unavailable；只在 initialize 成功并解析能力后发布 Ready**
- [ ] **步骤 4：运行 `cargo test -p viden-runtime agent_adapter probe_typed_agent_adapter`**
- [ ] **步骤 5：提交 `git commit -m "fix(core): publish truthful ACP startability"`**

### 任务 4：增加精确 ACP 会话续聊、重试和取消

**文件：** `crates/runtime/src/agent_commands.rs`、`runtime_supervisor.rs`、`event_journal.rs`、`runtime_supervisor_tests.rs`。

**接口：** `resume_typed_agent_session(cwd, session, content, sink, approver) -> Result<String, String>` 返回持久化 input id。

- [ ] **步骤 1：写测试：续聊复用精确 session、错误 Lane 所有者取消得到 `agent_session_owner_mismatch`**
- [ ] **步骤 2：运行 `cargo test -p viden-runtime follow_up_resumes_exact_acp_session cancel_rejects_session_not_owned`，确认失败**
- [ ] **步骤 3：加载持久会话记录，先验证 owner，再落 input 记录，通过 `AcpSessionOptions.load_session_id` 恢复；重试在同一逻辑会话下建立新 attempt，不原地改写终态 attempt**
- [ ] **步骤 4：运行 `cargo test -p viden-runtime runtime_supervisor agent_session event_journal`**
- [ ] **步骤 5：提交 `git commit -m "feat(core): resume exact ACP sessions"`**

### 任务 5：重启后恢复原生与 ACP 全部交互事实

**文件：** `runtime_contract.rs`、`agent_commands.rs`、`frontend_contract_v1.rs`，新建 `crates/core/tests/fixtures/native-acp-interaction-v1.jsonl`。

- [ ] **步骤 1：写 snapshot 与 ordered replay 业务投影完全一致的失败测试**
- [ ] **步骤 2：运行 `cargo test -p viden-core --test frontend_contract_v1 native_acp_fixture_snapshot_matches_ordered_replay`**
- [ ] **步骤 3：夹具覆盖工作区资格、Lane receipt、原生流/工具/成本/完成、ACP probe/start/follow-up/approval/result/cancel/retry/replay；恢复时不得把 Completed/Failed/Cancelled 改回 Started**
- [ ] **步骤 4：运行 `cargo test -p viden-types && cargo test -p viden-runtime && cargo test -p viden-core`**
- [ ] **步骤 5：提交 `git commit -m "test(core): certify native and ACP recovery parity"`**

### 任务 6：发布 Core 0.3.4 不可变检查点

**文件：** `crates/core/Cargo.toml`、`Cargo.lock`、中英文前端合同与并发开发计划。

- [ ] **步骤 1：版本改为 `0.3.4`，双语记录新增命令、事件、状态、精确取消、重试和回放语义**
- [ ] **步骤 2：运行 `cargo fmt --check && scripts/check-dependency-boundaries.sh && git diff --check`**
- [ ] **步骤 3：运行 `cargo test -p viden-types && cargo test -p viden-session && cargo test -p viden-workflows && cargo test -p viden-runtime && cargo test -p viden-core && cargo test --workspace --quiet`**
- [ ] **步骤 4：提交 `chore(core): release frontend contract 0.3.4`，用 `git rev-parse HEAD` 记录 TUI/GUI 唯一基线 SHA**
