# Codex 架构细读报告（openai/codex，2026-08 主干）

> 来源：浅克隆于 scratchpad 的 openai/codex 主干。8 个并行子代理分模块细读，主会话交叉核对汇总。
> 注意：该 checkout 比公开文档新很多（`docs/protocol_v1.md` 自述已过时；仓库含 agent-graph-store、memories、分页 thread-store 等公开版未见的 crate）。所有 file:line 以该克隆为准。
> 本报告服务于 Viden 已接受的六条架构方向（契约 fixture / 协议演进纪律 / 权限决策-执行分层 / core daemon 化 / headless 契约客户端 / compact 策略化组织）。

---

## 0. 总分层（实际形态，非文档形态）

```
前端（全部只是协议客户端）
  TUI ─┐
  exec ─┼── app-server-client（in-process / UDS daemon / ws remote，同一接口）
  IDE ─┘         │
                 ▼
  app-server（JSON-RPC 翻译层：v2 wire 类型 ≠ core 域类型）
                 │  ClientRequest / ServerNotification / ServerRequest
                 ▼
  core（引擎：Session/Turn/Task，SQ=Op / EQ=EventMsg，进程内 channel）
                 │
  ├─ sandboxing（SandboxManager：决策→执行的接缝）
  ├─ rollout JSONL（事实源）+ SQLite（可重建投影）
  ├─ compact 策略族 / context_manager
  └─ tools / MCP / skills / hooks / ext 扩展体系
```

**最重要的一个事实**：TUI 和 headless exec 自己也不直接调 core——它们通过 `InProcessAppServerClient` 走完整的 app-server 请求/通知协议（`app-server-client/src/lib.rs:306-309` 的注释明说：故意保留 server 的 request/notification 模型而不暴露 core 运行时句柄）。`AppServerTarget` 三种模式：`Embedded`（进程内）、`LocalDaemon`（Unix socket）、`Remote`（ws/wss）——同一接口，只换传输（`tui/src/lib.rs:273-277`）。

---

## 1. 引擎协议层（`protocol` crate：SQ/EQ 契约）

### 1.1 Op（提交队列，UI→core）
- `Op` 是 **进程内 Rust 枚举，非 serde wire 类型**（部分 variant 携带 `oneshot::Sender`），`#[non_exhaustive]`（`protocol.rs:540`）。
- 已从文档里的 `Op::UserTurn` 演进为：`TurnInput { request, mode, reply }` / `RecoverTurn` / `ThreadSettings`，per-turn 上下文（cwd、model、sandbox、approval、permission_profile、effort、collaboration_mode…）统一收在 `ThreadSettingsOverrides`（`protocol.rs:469-521`）。
- `TurnInputMode = StartOrSteer | StartIfIdle | Steer{expected_turn_id}`；提交结果为 `Started/Steered/NotSubmitted{reason}`，`NotSubmittedReason` 枚举了 NotIdle / PlanMode / ExpectedTurnMismatch / 非可转向任务（Review、Compact）等失败原因——**"转向一个正在跑的 turn"是协议一等公民**。
- 其余 Op：审批回执（Exec/Patch/Permissions/UserInput/DynamicTool/Elicitation）、`Compact`、`ThreadRollback{num_turns}`、`Review`、`RunUserShellCommand`、realtime 会话一族、`InterAgentCommunication`。

### 1.2 EventMsg（事件队列，core→UI）
- `EventMsg` 是真正的 wire 契约：`#[serde(tag="type", snake_case)]` + `JsonSchema` + `TS` 派生（`protocol.rs:1281`）。
- 新旧双模型并存：新模型是 `ItemStarted/ItemCompleted{ item: TurnItem }`（18 种 TurnItem），旧模型是平铺的 `ExecCommandBegin/End`、`McpToolCallBegin/End` 等。兼容靠 `HasLegacyEvent` trait（`legacy_events.rs:67`）：一个 Item 事件按需扇出为零到多个 legacy 事件，delta 类事件刻意不产生 legacy 事件。
- 事件类别：生命周期（TurnStarted/TurnComplete/TurnAborted/SessionConfigured…）、流式 delta（AgentMessageContentDelta/ReasoningContentDelta/ExecCommandOutputDelta…）、工具进度、审批请求、错误/警告/元信息（TokenCount、ModelReroute、RateLimits）、review 模式、多代理 collab 事件。

### 1.3 演进纪律（可直接抄的规则清单）
1. `#[non_exhaustive]` 用在会增长的枚举上（`Op`、`UserInput`）。
2. 改名走 `#[serde(rename = 旧名, alias = 新名)]` 双标签：`TurnStarted` 序列化仍是 `task_started`，反序列化两个都收（`protocol.rs:1328,1337`）；同样手法用于 `on-failure→on-request`、`none→deny`、`guardian_subagent→auto_review` 等。
3. 后加的"逻辑必填"字段一律 `#[serde(default)]`（多个事件的 `turn_id`），带注释说明是为老 rollout 兼容；语义默认值不对时手写 Deserialize（`RequestUserInputEvent.is_blocking` 缺省应为 true，`request_user_input.rs:73-98`）。
4. 多形状兼容用 untagged 中间枚举（`FileSystemPermissionsDe::{Canonical, Legacy}`）。
5. **前向兼容 catch-all**：`FileSystemSpecialPath::Unknown{...}` 显式保留未知 token 而非拒绝——注释里引用了 0.112.0 因拒绝未知值而破坏前向兼容的事故。
6. ID 设计：`SessionId`/`ThreadId` 共享同一 UUID 空间（构造保证）；`RolloutId = ThreadId`，但 `thread/revert` 换 rollout 文件不换 ThreadId——回滚不清掉原始转录。

---

## 2. app-server 契约层（对 Viden 方向 #1、#4 最关键）

### 2.1 协议形态
- JSON-RPC 2.0 **去掉 `jsonrpc` 字段**（`rpc.rs:1-2` 注释明说）；请求带非标 `trace`（W3C traceparent）。stdio 用 NDJSON 帧，UDS/ws 用 WebSocket 文本帧。
- 四个方向的完整目录**由一个声明式宏表生成**（`common.rs`）：约 150 个 client→server 方法、11 个 server→client 请求（审批/elicitation/动态工具/token 刷新）、约 80 个 server→client 通知、1 个 client 通知（initialized）。
- 每行声明五件事：wire 方法名、params 类型、response 类型、`inspect_params`（字段级实验门控）、**serialization scope（并发键）**——例如 `thread/resume` 声明按 `thread_id` 串行。同一张表同时生成：运行时枚举与 TryFrom、实验门控元数据、schema 导出函数。**没有独立 IDL 文件可漂移。**

### 2.2 与 core 协议的关系
- **不是包装而是有状态翻译**。wire 的 v2 类型与 core 域类型刻意分离（`auth_mode.rs:4-9` 注释："类型保持分离，使 app-server 协议所有权不泄漏进域 crate"）。
- 纯映射部分：`event_mapping.rs`（无状态 1:1 投影，注释明确划界）。
- 有状态部分：`bespoke_event_handling.rs`（4115 行单个 match）：合成 turn 对象、在 turn 边界中止悬挂的 server 请求、把 core 审批事件转为 server→client 请求并把回答转回 `Op`。

### 2.3 schema fixture 机制（完整配方，Viden #1 直接可移植）
1. **cfg 交换的 no-op 派生**：协议类型都写 `#[derive(..., JsonSchema, TS)]`，但 release 构建里这两个名字解析到一个 20 行的 no-op proc-macro crate；只有 `#[cfg(test)]` 才解析到真的 schemars/ts-rs——注解零成本，重依赖全在 dev-dependencies（`lib.rs:64-71`）。
2. **生成产物入库 + 压缩内嵌**：655 个 `.ts` + 288 个 `.json` fixture checked-in（PR 里可人审 diff），同时打成 zstd blob `include_bytes!` 进二进制，让发布的 `codex app-server generate-ts` 不需要生成器本体。
3. **三角等式测试**：生成器输出 == 入库 fixture 树 == 内嵌 blob，三方 byte-level 一致（`schema_fixtures_tests.rs` 四个测试）。
4. **归一化比较**：JSON 键排序 + 有守卫的数组排序（只有每个元素都能给出稳定排序键才排）、CRLF 归一、剥生成 banner——断言语义而非格式，消灭 Windows CI 抖动。
5. **失败信息自带修复命令**：`Run 'just write-app-server-schema' to overwrite...` + unified diff。
6. **stable/experimental 双生成 + 金丝雀类型**：`mock/experimentalMethod` 与 `mockExperimentalField` 专为验证过滤器存在，断言"stable 必不含、experimental 必含、runtime 必拒"三个方向——防止过滤器坏掉但 fixture 同步坏掉时测试假绿。
7. **风格规则测试化**：`?: T | null` 只允许出现在 `*Params` 类型——把 AGENTS.md 里的书面规则变成 fixture 测试。
8. 再生成入口是 `#[ignore]` 测试 + 环境变量，无独立二进制、无重复构建图。

### 2.4 传输与生命周期
- 监听模式：`stdio:// | unix://PATH | ws://IP:PORT | off`；UDS 权限 0600、接受后升级为 WebSocket；ws 模式拒绝任何带 Origin 的请求；另有 remote-control 反向 ws（NAT 穿透）和 `stdio-to-uds` 字节泵代理。
- 连接↔线程**多对多**（多个 IDE 窗口订阅同一 thread），`ThreadStateManager` 维护映射；慢客户端出站队列满即断连；入口饱和回 `-32001 Server overloaded`。
- 握手：`initialize{clientInfo, capabilities}` + `initialized` 通知；**没有 protocolVersion 字段**——兼容靠 v1/v2 命名空间、additive-only 纪律（fixture 测试强制）、experimental capability 三件套。字段级实验门控一处注解四处生效（方法门/字段门/出站过滤/codegen 过滤）。
- 客户端可在握手时按方法名精确 opt-out 通知；`deprecationNotice` 是带内一等通知。

### 2.5 契约回归的两半
- 结构半：上述 fixture 测试。
- 行为半：`tests/common/test_app_server.rs`（2125 行）spawn **真实生产二进制**（临时 CODEX_HOME、结构化日志断言、~200 个 typed send_* 方法），v2 suite 约 85 个文件。

---

## 3. core 引擎（`core` crate，~52k LOC 非测试代码）

### 3.1 对象模型
```
ThreadManager（进程级注册表）
  └─ Arc<CodexThread>（薄门面：submit/next_event/steer）
       ├─ Arc<Session>（全部运行态）
       └─ SessionIo（channels + agent_status watch）
Session
  ├─ SessionServices（DI 袋：ModelClient/McpRuntime/ExecPolicyManager/ApprovalStore/…）
  ├─ Mutex<SessionState>
  └─ Mutex<Option<ActiveTurn>>   ← 单任务不变量的结构化承载
ActiveTurn → RunningTask（JoinHandle+CancellationToken）+ TurnState（悬挂审批、pending input）
TurnContext（turn 级，趋于不可变）→ StepContext（每次采样请求级，含定稿的 ToolRouter）
```
- **一个 Session 同时最多一个 Task**（doc comment + `debug_assert`），新任务 spawn 无条件以 `Replaced` 中止旧任务。并行靠多线程实例 + `AgentExecutionLimiter`（原子计数上限，turn 生命周期持有 slot）。
- `StepContext` 是比 Turn 更细的新单位：把该次请求实际通告与执行的工具集（ToolRouter）、审批策略、MCP 绑定一起钉死——**同一请求内"模型看到的"与"实际能执行的"由同一对象保证一致**。

### 3.2 一个 turn 的端到端
提交 → `submission_loop`（每 session 一个 tokio task，大 match）→ `spawn_task` → `run_turn` 外层循环：
pre-turn compact → 捕获 StepContext → 注入 skills/plugins → hooks 记录输入 → 循环 { 组 prompt（history.for_prompt + model_visible_specs）→ `run_sampling_request`（重试包装：退避、ws→http 降级换传输后重置计数）→ SSE 消费 }。
- 模型接入：Responses API，优先 WebSocket（增量 item 发送、连接复用），可回落 HTTP SSE；401 自动走 auth 恢复重试一次。
- 工具调用在流内即时入 history/rollout（先记录后执行，防取消丢失一致性），futures 以 `FuturesOrdered` 排队，流结束后**按序** drain。
- 并行门控：可并行工具拿读锁、串行工具拿写锁（`tools/parallel.rs:153-157`）——一个 RwLock 解决混合并行。
- 取消是令牌树：root → task → sampling → 每个工具调用；SSE 读用 `.or_cancel()`，中断能掐断半路的流。
- 转向（steer）不打断在途采样，落在下一次模型请求边界。

### 3.3 审批（fail-closed 全链路）
- 先注册 oneshot 再发事件，`rx.await.unwrap_or(ReviewDecision::Abort)`——通道断开=拒绝；`ReviewDecision` 的 `Default` 也是 Denied。
- 决策优先级明文注释：**1. hooks，2. Guardian（模型自动审查者，严格 JSON、90s 超时、拒绝熔断、超时即拒），3. 用户**。
- 审批决定永不产生于 core 内部（除策略/hooks/Guardian），一律从提交队列进来。

### 3.4 工具系统
- 每个 step 重建 `ToolRegistry`：shell（unified_exec 双工具或 legacy shell）、MCP 资源工具、实用工具（plan/request_user_input/request_permissions/view_image/apply_patch…）、多代理 collab 工具、扩展/动态/hosted（web search 由服务端执行）工具，最后 `finalize` 做 code-mode 包装与冲突检测。
- `ToolExposure = Direct | Deferred（tool_search 可发现）| Hidden（可调度不通告）`——与 Claude Code 的 deferred tools 同构。
- `unified_exec` 管理**长命 PTY 进程**（数字进程 id、write_stdin、跨 turn 存活、后台终端列表/清理），与一次性 exec 共享同一套审批/沙箱/升级编排（`ToolOrchestrator`：审批→选沙箱→尝试→被沙箱拒绝时申请免沙箱重试）。
- `FunctionCallError::RespondToModel vs Fatal` 是核心控制信号：前者变成模型可见的工具输出，后者杀 turn。
- `TurnDiffTracker` 从 apply_patch 增量累计每 turn 净 diff，不重读文件系统。

### 3.5 前端无关性的机制（不只是纪律）
- `#![deny(clippy::print_stdout, print_stderr)]`。
- 唯一出口 `send_event`：**先持久化 RolloutItem 再投递**；同时按旧客户端需要扇出 legacy 事件。
- `event_mapping.rs` 把 core 自己注入的上下文（`<environments_instructions>`、`<skills_instructions>`、`<context_window>` 等一整张前缀表）**从前端可见事件流里过滤掉**——前端永远不渲染引擎管道内容。

### 3.6 子代理（三种机制，不可混淆）
1. `codex_delegate`：进程内委托 Session（继承父服务、强制 `AskForApproval::Never`、禁递归 spawn、不注册进 ThreadManager）——只有 review 和 Guardian 两个消费者。
2. `agent/` + `spawn_subagent`：一等子代理线程（fork 父历史、注册进 ThreadManager、spawn 深度限制、`AgentControl` 随后代克隆、v1/v2 两代 collab 工具）。
3. `agent_communication.rs`：纯可观测性（OTel 打点），真正的传输是 `Op::InterAgentCommunication` → 邮箱 → `MailboxDeliveryPhase` 状态机决定并入当前 turn 还是下一 turn。

---

## 4. 沙箱与策略栈（Viden 方向 #3 的参照）

### 4.1 决策层
- **execpolicy**：Starlark DSL（`prefix_rule` / `host_executable` / `network_rule`），裁决 `Allow | Prompt | Forbidden`，多规则命中取最严；规则文件按配置层（项目→用户→托管）合并，ArcSwap 热更新；批准可持久化为新 Allow 规则追加进 `default.rules`。
- 命令先拆解（`bash -lc "a && b"` 拆成子命令逐个判），未命中走危险/安全启发式 + 当前审批模式合成裁决；**只有全部子命令都命中显式 Allow 规则才允许绕过沙箱**。
- 无平台沙箱时决策层自动收紧（未知命令强制 Prompt/Forbidden）——**决策强度与执行能力联动**。

### 4.2 接缝（最值得抄的形状）
- `SandboxManager`（`sandboxing/src/manager.rs`）：`should_sandbox()` → `select_initial()` 选 `SandboxType{None|MacosSeatbelt|LinuxSeccomp|WindowsRestrictedToken}` → `transform(SandboxTransformRequest)` 产出 **`SandboxExecRequest`{command, cwd, env, network, permission_profile, ...}**——结构化的"决策结果→执行约束"对象，再降级为最终 `ExecRequest` 并注入 `CODEX_SANDBOX*` 信息性环境变量（明文注释：informational，非 enforcement 证明）。
- 运行时权限形状：`PermissionProfile = Managed{file_system, network} | Disabled | External{network}`，文件系统条目支持 Path/Glob/Special（含 `Unknown` 前向兼容）。

### 4.3 执行层
- macOS：动态拼 `.sbpl`（default-deny 基线 + 生成的读写子路径 + deny-regex glob + 动态网络策略），走 `/usr/bin/sandbox-exec` 硬编码绝对路径防 PATH 注入。
- Linux：默认 bubblewrap（根只读 bind + 可写根分层 + `.git`/`.codex` 再关闭 + unshare user/pid/net），两段式：外层 bwrap 建文件系统视图后 re-exec 自身，内层进程内上 `no_new_privs` + seccomp-bpf（永远拒 ptrace/io_uring；受限网络模式拒 connect 族、socket 仅 AF_UNIX）；托管代理模式用 TCP↔UDS 桥 + seccomp 收紧。旧 Landlock 是显式 opt-in 回退。
- Windows：受限令牌 + 作业对象 + ACL 拒读写 + WFP 防火墙按进程 SID 控出站。
- 网络四层：execpolicy network_rule → 用户态代理（HTTP/SOCKS5，allowlist-first、limited 只读模式 MITM 验方法）→ OS 层强制只许到代理回环端口 → 环境变量信号位。
- 辅助：shell-escalation（打补丁的 zsh 拦截 execve，socket 协商"沙箱内跑/升级到沙箱外/拒绝"，传 FD 保持 stdio）；process-hardening（禁 ptrace attach、禁 core dump、清 LD_/DYLD_）。

---

## 5. 持久化（印证并加固 Viden 现有设计）

- **JSONL 追加日志为事实源**：`sessions/YYYY/MM/DD/rollout-<ts>-<thread_id>.jsonl[.zst]`，行格式 `RolloutLine{timestamp, ordinal, RolloutItem}`；冷压缩 zstd，追加前透明解压回 `.jsonl`。持久化策略过滤器决定哪些 item 落盘。写入走后台任务 + mpsc 命令通道，新线程延迟建文件到首次 persist。
- **SQLite 全部是可重建投影**，且工程化了三道防线：
  1. 启动 backfill 门禁（`BackfillStatus::Complete` 之前拒发 DB 句柄）；
  2. read-repair（文件扫描与 DB 不一致时以 rollout 文件头为准修 DB 行）;
  3. 每条 DB 读路径都有纯文件系统扫描兜底 + 回退遥测打点。
- 多个专用 DB 分文件（state_5 / logs_2 / goals_1 / memories_1 / queue_1 / thread_history_1）而非一个大库；分页历史模式下 SQLite 兼作读模型，但 JSONL 写路径不变。
- `ThreadStore` trait 是存储中立边界（local 文件+SQLite 实现 / in-memory 测试实现）。
- 压缩即日志检查点：resume 从最新带 `replacement_history` 的 `Compacted` 记录起步，只重放其后的条目；原始转录留盘可审计。
- 凭据：OS keyring 只存一个口令，真实秘密 age 加密落盘 `secrets/*.age`，按 global/环境 scope 键控，原子写。
- memories：两阶段后台管道（SQLite job 表租约认领 → raw_memories.md 渲染 → git 仓库做 diff 基线的整合 agent 产出 MEMORY.md），限流门禁。

---

## 6. 上下文压缩（Viden 方向 #6 的组织配方）

四种策略并存：**本地 Memento**（摘要 prompt 跑一个合成 turn，重建为"近期用户消息+摘要"）、**远端 v1**（`/responses/compact` 服务端压缩返回替换历史）、**远端 v2**（`CompactionTrigger` 触发模型输出一个加密 `Compaction` 桥接项 + 本地 64k 保留预算）、**token-budget**（不摘要，直接开新窗口重渲染 WorldState）。

组织方式（可直接照抄）：
1. 每策略一个文件、**同形入口函数对**（manual + inline auto）；加第五种策略=加一个文件+一个 match 臂。
2. 选择逻辑只存在于两个调用点（手动 /compact 与 auto），按 provider capability + feature flag 分派；策略互不知晓。
3. 共享生命周期脚手架（初始上下文重注入、分析枚举、hook 触发）定义一次被各策略 use。
4. 模型回退（换模型重试一次）与策略选择正交，判据函数独立。
5. 遥测按 Trigger/Reason/Implementation/Phase/Strategy 五轴独立命名，可自由交叉分析。
6. rollout 只认 `Compacted` 检查点类型不认策略——新策略不动 resume 路径。
7. 触发点三处：pre-turn、mid-turn（needs_follow_up 且超限）、换模型时（compat hash 变化或降档到更小窗口）。
8. 阈值：默认窗口 90% 再叠加回退缓冲；估算器明示是"粗略下界，非精确 tokenizer"。

配套：`context/world_state/` 把 AGENTS.md/权限/环境等做成可 diff 的结构化 section（快照+增量渲染，压缩后重置为全量）；`context-fragments` 小 crate 统一"可识别、可合并、带标记的注入块"契约。

---

## 7. 工具与扩展生态

- **双层扩展模型**：对内 `ext/extension-api` 是编译期 Rust contributor trait 体系（ToolContributor/ConfigContributor/TurnLifecycleContributor…），codex 自家功能（guardian、goal、memories、web-search、image-gen）全部以内部扩展组合；对外 `plugin` 只是 **skills + MCP servers + hooks 的打包分发格式**（manifest + marketplace），不是运行时扩展 API。
- **MCP 双向**：作为客户端（rmcp-client，stdio/HTTP/OAuth，工具曝光策略、elicitation、"always allow"写回配置）；作为服务端只暴露 `codex` 和 `codex-reply` 两个工具，审批转为 MCP elicitation。
- **skills**：SKILL.md + YAML frontmatter；注入分两级——目录级（每技能一行，预算=窗口 2%，超预算截断描述并告警）与选中级（全文，8KB 截断）；`$name` / `skill://` mention 语法与 connector（`app://`）、plugin mention 共用一套解析。
- **hooks**：11 个生命周期事件；handler 四类（Command 子进程 / McpTool / Prompt / Agent）；PreToolUse 可改写工具入参或阻断，Post 只观测；来源与信任分级（System/User/Project/Mdm/Plugin/Cloud）。
- **code-mode**：模型写 JS 一次调多工具。裸 V8（可 jitless）、剥除危险全局、`tools.*` 每次调用阻塞回程到宿主执行器；host/protocol/runtime 三 crate 分离使 V8 可进程内或独立进程运行。

---

## 8. 前端面与分发

- TUI：ratatui；`AppEvent` 内部总线（~200 变体）；**已完成的历史直接写终端原生 scrollback（escape 序列），ratatui 每帧只重绘底部活动区**；独立提交动画线程按目标帧率驱动流式动画；审批 overlay 保证 Esc 永远映射为显式 Cancel（防"关掉弹窗=默认继续"）。
- exec：`--json` 输出稳定 JSONL `ThreadEvent` 流（thread.started/turn.completed/item.* …），已从 experimental 转正；退出码把 turn 失败/中断映射为 1；支持 resume/fork/review 子命令。
- **TS SDK 驱动的是 `codex exec --json` 子进程而非 app-server**——Thread/Turn 对象 + AsyncGenerator 事件流；事件类型是手工镜像 Rust 的 `exec_events.rs`；outputSchema 经临时文件传 `--output-schema`。
- 单二进制多路复用：clap 子命令树 + **arg0 技巧**（按 argv[0] basename 分派 codex-linux-sandbox/apply_patch；启动时在受锁临时目录创建指回自身的符号链接并前置 PATH，"用一个二进制模拟部署多个可执行"）。
- npm 分发：无 postinstall；`optionalDependencies` 按平台包（os/cpu 约束筛选安装），入口 JS resolve 平台包内 `vendor/<triple>/bin/codex` 并 spawn（stdio inherit、信号转发、退出码镜像）。

---

## 9. 映射回 Viden 六条已接受方向（细读后的修正与落点）

1. **契约 fixture（第一优先）**——采用 §2.3 完整配方，而不只是"序列化样本"：宏表单一来源、cfg 交换 no-op 派生、产物入库+三角等式、归一化比较、金丝雀类型、失败信息带修复命令。Viden 落点：`crates/types`（或新 `crates/contract`）为 frontend-contract-v1 建 fixture 树，TUI/GUI parity corpus 挂同一套。
2. **演进纪律**——§1.3 的六条规则写进 `crates/AGENTS.md`/`crates/types` 说明；额外学习点：**无协议版本号**路线可行的前提正是 fixture 测试 + additive-only + experimental capability 三件套齐备。
3. **权限接缝**——`SandboxSpec` 的形状照 `SandboxTransformRequest → SandboxExecRequest`；同时抄"无执行层时决策自动收紧"的联动逻辑；信息性环境变量与强制机制明确分开标注。
4. **core daemon 化（认知升级）**——不是"再加一个 daemon 形态"，而是 codex 展示的终局：**前端一律走同一协议客户端，Embedded/LocalDaemon/Remote 只是传输选项**。Viden 的 `local_transport` 应演进为这个三态客户端的 in-process 实现；wire 类型与域类型分离（翻译层允许有状态、集中在一处 bespoke 文件）；连接↔线程多对多、慢客户端断连、serialization scope 声明在方法表里。
5. **headless 契约客户端**——codex 的 exec 证明该路线成立且已转正为稳定接口；注意它的 JSONL 事件 schema（`exec_events.rs`）是独立于内部 EventMsg 的**面向消费者的第三套稳定 schema**，SDK 镜像的是它。Viden 的 `--no-tui` 输出应同样定义为独立的稳定事件 schema 而非直接倾倒内部事件。
6. **compact 策略化**——照 §6 的八条组织方式；Viden `crates/context` 从第一天起按"每策略一文件 + 两个调用点分派 + 检查点持久化与策略解耦"布局。

另有两个此前未列入但值得记录的候选方向：
- **审批 fail-closed 模式**（oneshot 先注册后发事件、断开即 Abort、Default=Denied）——Viden permissions/runtime 交互可对照检查。
- **投影自愈三防线**（backfill 门禁 / read-repair / 文件系统兜底+遥测）——`crates/session` 的 SQLite 索引可按此加固。
