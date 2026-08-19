# Codex 架构总结

> 基于 openai/codex 2026-08 主干浅克隆的全模块细读（8 个并行子代理分模块阅读，主会话交叉核对）· 2026-08-16
> 完整 file:line 版本见同目录《codex-architecture-deep-read.md》。仓库许可 Apache-2.0。

Codex 是一个 100+ crate 的 Rust workspace（`codex-rs/`），外围是 npm 分发壳、TS/Python SDK 和 Bazel/just 双构建。本文按层总结其实际架构（比公开文档新一代：`protocol_v1.md` 自述已过时），末尾给出可迁移设计清单。

## 00 总分层：一切前端都是协议客户端

```
TUI ──┐
exec ─┼─ app-server-client（Embedded / LocalDaemon / Remote，同一接口）
IDE ──┘        │
               ▼
      app-server（JSON-RPC 翻译层：wire 类型 ≠ 域类型）
               ▼
      core 引擎（Session/Turn/Task；SQ=Op / EQ=EventMsg）
       ├─ sandboxing（决策→执行接缝）
       ├─ rollout JSONL（事实源）+ SQLite（可重建投影）
       ├─ compact 策略族 / context_manager
       └─ tools / MCP / skills / hooks / ext 扩展体系
```

**最重要的一个事实**：TUI 和 headless exec 自己也不直接调 core，而是走 `InProcessAppServerClient`，完整使用 app-server 的 request/notification 协议。进程内（Embedded）、本地守护进程（UDS）、远程（WebSocket）只是同一接口的三种传输。app-server 协议就是唯一前端契约。

## 01 引擎协议（SQ/EQ）

- `Op`（提交队列）是**进程内枚举**（部分 variant 携带 oneshot），非 wire 类型；per-turn 上下文统一收在 `ThreadSettingsOverrides`。转向正在运行的 turn（Steer）是协议一等公民，失败原因是显式枚举。
- `EventMsg`（事件队列）是真正的 wire 契约（serde tag + JsonSchema + TS 派生）。新旧双模型并存：新的 `ItemStarted/ItemCompleted{TurnItem}` 通过 `HasLegacyEvent` 按需扇出为旧的平铺事件。
- 演进纪律：`non_exhaustive` 增长枚举；改名走 rename+alias 双标签（`task_started`/`turn_started`）；后加字段一律 `serde(default)`；多形状用 untagged 中间枚举；未知值保留不拒绝（`FileSystemSpecialPath::Unknown`，注释引用过真实前向兼容事故）。

## 02 app-server 契约层

- JSON-RPC 2.0 去掉 `jsonrpc` 字段；请求带 W3C trace。四个方向约 150/11/80/1 个方法，全部由**一张声明式宏表**生成——同一张表同时产出运行时枚举、实验门控元数据、schema 导出函数和**并发键**（serialization scope 声明在方法行里）。没有独立 IDL 可漂移。
- 与 core 的关系是**有状态翻译**而非包装：无状态 1:1 投影在 `event_mapping.rs`，有状态的集中在 `bespoke_event_handling.rs`（4115 行单个 match）。wire 类型与域类型刻意分离。
- **schema fixture 机制**（完整配方）：① cfg 交换的 no-op 派生（release 零成本，schemars/ts-rs 全在 dev-deps）；② 655 个 .ts + 288 个 .json fixture 入库 + zstd blob 内嵌二进制；③ 三角等式测试（生成器 == fixture 树 == blob）；④ 归一化比较（键排序、有守卫的数组排序、剥 banner）；⑤ 金丝雀类型双向断言防过滤器假绿；⑥ 失败信息自带修复命令；⑦ 风格规则写成 fixture 测试。
- **无协议版本号**：兼容靠 v1/v2 命名空间 + additive-only（fixture 强制）+ experimental capability 三件套。连接↔线程多对多（多窗口订阅同一 thread）；慢客户端队列满即断连。

## 03 core 引擎

- 所有权链：`ThreadManager → CodexThread → Session → ActiveTurn → TurnContext → StepContext`。**一个 Session 同时最多一个 Task**（结构化承载 + debug_assert），并行靠多实例 + 原子计数上限。
- `StepContext` 把该次采样请求"模型看到的工具集"与"实际可执行的工具集"钉在同一对象上。
- 模型接入 Responses API：优先 WebSocket（增量 item、连接复用）可回落 HTTP SSE；重试带退避与传输降级。工具调用**先入 history/rollout 再执行**；并行门控用一把 RwLock（并行工具读锁、串行工具写锁）；取消是令牌树，SSE 读可被中途掐断。
- **审批 fail-closed 全链路**：先注册 oneshot 再发事件，`await.unwrap_or(Abort)`，`ReviewDecision` 默认值即 Denied。决策优先级：hooks → Guardian（模型自动审查者，超时即拒、拒绝熔断）→ 用户。
- 前端无关性有机制保障：`deny(print_stdout)`；唯一出口 send_event 先持久化再投递；`event_mapping` 把引擎注入的上下文从前端可见流中过滤。
- 子代理三机制：进程内委托（强制 Never 审批、只供 review/Guardian）；一等子代理线程（fork 历史、深度限制、collab 工具 v1/v2）；邮箱通信 + 投递相位状态机。

## 04 沙箱与策略

- 决策层：Starlark 规则语言（execpolicy），裁决 Allow/Prompt/Forbidden、最严者胜；命令先拆解逐个判；**只有全部子命令命中显式 Allow 才可绕过沙箱**；无平台沙箱时决策自动收紧。
- 接缝：`SandboxManager::transform` 把 `PermissionProfile` 转成结构化 `SandboxExecRequest`（命令、cwd、env、网络、可写根）——决策与执行的清晰边界。
- 执行层：macOS 动态拼 Seatbelt profile；Linux bwrap 两段式 + seccomp（永远拒 ptrace/io_uring）；Windows 受限令牌 + WFP 防火墙。网络四层：规则 → 用户态代理（allowlist-first）→ OS 强制只达代理端口 → 信息性环境变量。

## 05 持久化

- JSONL 追加日志为事实源（按日期分目录、zstd 冷压缩、后台写入任务）；SQLite 全部是**可重建投影**，且有三道防线：启动 backfill 门禁、read-repair 自愈、每条读路径的文件系统兜底 + 回退遥测。
- 压缩即日志检查点：resume 从最新 `Compacted{replacement_history}` 起步只重放其后条目，原始转录留盘可审计。凭据：OS keyring 只存口令，秘密 age 加密落盘。

## 06 上下文压缩

- 四策略并存：本地摘要（Memento）、远端 v1（服务端压缩）、远端 v2（加密桥接项）、token-budget（直接开新窗口）。**每策略一个文件、同形入口、选择只在两个调用点**，按 provider capability + feature flag 分派；rollout 只认检查点类型不认策略，新策略不动 resume 路径。
- 触发三处（pre-turn / mid-turn / 换模型时），阈值默认窗口 90% + 回退缓冲。`world_state` 把 AGENTS.md、权限、环境做成可 diff 的结构化 section。

## 07 工具与扩展生态

- **双层扩展模型**：对内 `ext/extension-api` 是编译期 contributor trait 体系（自家功能全部以内部扩展组合）；对外 plugin 只是 skills+MCP+hooks 的打包分发格式。
- 工具曝光三态 Direct/Deferred/Hidden；unified_exec 管理长命 PTY 进程；`RespondToModel vs Fatal` 是核心控制信号。MCP 双向（客户端全功能；服务端只暴露 codex/codex-reply 两个工具）。skills 两级注入（目录行预算 2% 窗口 / 选中全文 8KB）；hooks 11 个生命周期事件、PreToolUse 可改写入参或阻断。

## 08 前端与分发

- TUI：已完成历史直接写终端原生 scrollback（escape 序列），ratatui 只重绘底部活动区；审批 overlay 保证 Esc 永远等于显式 Cancel。
- exec 的 `--json` 是**独立于内部 EventMsg 的第三套面向消费者的稳定事件 schema**，TS SDK 镜像并驱动它（子进程方式）。单二进制 arg0 分派；npm 用 optionalDependencies 平台包分发、无 postinstall。

## 09 可迁移设计清单

| 设计 | 一句话 |
| --- | --- |
| 协议即前端契约 | 所有前端走同一协议客户端，进程内外只是传输差异 |
| 契约 fixture 三角等式 | 宏表单一来源 + 产物入库 + 生成器/fixture/内嵌 blob 三方一致测试 |
| 演进六规则 | non_exhaustive、双标签、serde(default)、untagged 兼容、未知值保留、无版本号靠纪律 |
| 决策↔执行接缝 | 权限裁决产出结构化 SandboxExecRequest，执行层按平台各自实现 |
| 审批 fail-closed | 先注册回执通道再发事件，任何异常路径都落到拒绝 |
| 投影自愈三防线 | backfill 门禁 / read-repair / 文件系统兜底 + 遥测 |
| 压缩策略化组织 | 每策略一文件、同形入口、两个分派点、检查点与策略解耦 |
| 消费者事件 schema | headless 输出独立稳定 schema，不倾倒内部事件 |
