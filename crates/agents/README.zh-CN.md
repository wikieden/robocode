# viden-agents

## 目的

`viden-agents` 负责外部 agent adapters：Viden 驱动在另一个进程中运行的 agent 的
全部方式。它为每个外部 CLI 保存一种策略——通用 ACP JSON-RPC 客户端、Codex CLI 与
app-server 客户端——以及两者共享的进程启动、探测和输出捕获基础设施，还有把已跟踪
agent job 投影为 typed 记录的 session glue。

它位于 `viden-runtime` 之下，不知道 session、lane、provider、前端或 runtime 的
trust loop。

## 不负责

- Session、lane、provider 或前端状态。
- Permission 决策序列本身；它调用 `viden-permissions` 中的共享 gate，由调用方提供
  engine 和 approver。
- Runtime 事件记录在何处；由 runtime 提供 `RuntimeEventSink`。
- Transcript 记录与 `/agent` 命令界面，这些留在 `viden-runtime`
  （`agent_dispatch.rs`）。
- 直接访问操作系统。所有 process、文件和 terminal 副作用都经过 `viden-tools`
  capabilities。

## 公共接口

- Typed agent sessions：`start_typed_agent_session`、
  `resume_typed_agent_session`、`retry_typed_agent_session`、
  `cancel_typed_agent_session`、`mark_typed_agent_session_status`、
  `validate_typed_agent_session_request` 和
  `typed_agent_session_request_from_compat_input`。
- Adapter 发现：`typed_agent_adapter_views` 和 `probe_typed_agent_adapter`。
- 已跟踪 job 投影：`tracked_agent_job_tasks`、`tracked_agent_job_sessions` 和
  `tracked_agent_job_runtime_events`。
- 注入的 runtime 策略：`RuntimeEventSink` 和 `AgentSessionApprover`。
- ACP band 的命令入口：`handle_agent_probe_command`、
  `handle_agent_auth_command`、`handle_acp_agent_run_command`、
  `run_acp_smoke_gate`、`parse_acp_run_args` 和 `AcpRunArgs`。
- Codex band 的命令入口：`handle_codex_review_command`、
  `handle_codex_challenge_command`、`start_codex_job`、
  `start_codex_app_server_job`、`render_codex_job_status`、
  `render_codex_job_result`、`cancel_codex_job`、`ensure_codex_target`、
  `parse_codex_run_args`、`ParsedCodexRunArgs`、`CodexJobKind`、
  `codex_command` 和 `codex_run_command_args`。
- `/agent` 展示辅助：`render_agent_list`、`render_agent_doctor` 和
  `render_agent_logs_help`。
- `shutdown_resident_acp_sessions`：不让任何 agent 进程比它的 project 活得更久。

## 不变量

- Permission checks 先于 effects。agent 请求的每次 reverse-RPC 文件系统和 terminal
  mutation，都先经 `viden_permissions::resolve_permission`——共享的
  decide -> ask -> apply_approval gate——解析，然后才调用 `viden-tools` capability。
- Runtime 拥有的策略只能注入，不能导入。Permission context、approver 和 event sink
  都作为参数传入，这保持依赖边单向，并由
  `scripts/check-dependency-boundaries.sh` 强制。
- 前端只能通过 Core 访问这些 adapters。`viden-agents` 不得出现在 TUI 或 GUI 的
  manifest 中。
- 常驻 ACP session 按 project 缓存，并随 project 一起拆除。

## 测试

```bash
cargo test -p viden-agents
```

Agent 行为另有 runtime 套件的端到端覆盖，它通过 `RuntimeSupervisor` 和 `/agent`
命令界面驱动 adapters：

```bash
cargo test -p viden-runtime
```
