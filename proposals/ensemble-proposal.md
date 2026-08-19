# 协奏 · Ensemble 提案

> 基于 openai/codex 开源版魔改的团队级 Agent 编排系统——多模型（含本地）、产品/开发/测试三角色在同一 Agent 体系内实时协作，配套 Web 协作 GUI。
> 草案 v1 · 2026-08-16 · 依据：codex 主干全模块细读（见同目录《codex-arch-summary.md》与《codex-architecture-deep-read.md》）· Apache-2.0 许可允许修改分发

## 01 为什么基于 codex 魔改，而不是从零写

细读结论表明，codex 已经免费提供了这个产品最难的三块地基：

1. **前端契约已经是服务协议。** 它的 TUI/exec 本身就走 app-server JSON-RPC 客户端，进程内、本地守护进程（UDS）、远程（WebSocket）是同一接口的三种传输——把"单机工具"变"团队服务"不需要重构，只需要打开已有的 Remote 形态并加上身份层。连接↔线程本来就是多对多（多个客户端可订阅同一 thread），实时协作的管道是现成的。
2. **多代理原语齐备。** 一等子代理线程（fork 历史、深度限制）、代理间邮箱通信、agent-graph-store（父子拓扑持久化）、collab 工具族（spawn/send/wait/interrupt）、thread goals、持久化队列、collaboration-mode 模板——编排一个"产品→开发→测试"的代理流水线所需的底层动词都在。
3. **工程化质量极高且可继承。** schema fixture 契约测试、三平台沙箱、审批 fail-closed、JSONL+SQLite 自愈持久化、compact 策略族——这些是从零写至少一年的工作量。

许可上 codex 为 Apache-2.0：允许 fork、修改、商用，义务是保留 LICENSE/NOTICE 与修改声明，无传染性。

## 02 目标与非目标

- **目标 G1**：任何 OpenAI 之外的模型（Anthropic/DeepSeek/Qwen 及 Ollama、LM Studio、vLLM 本地端点）都能作为一等公民驱动完整 agent 循环。
- **目标 G2**：一个团队（产品、开发、测试）连接同一个 Ensemble 服务端，在共享的项目空间里创建、观察、转向、审批 agent 工作，实时互见。
- **目标 G3**：Web GUI 覆盖三角色的核心工作面：任务看板、线程时间线、审批收件箱、diff 评审、代理拓扑。
- **目标 G4**：角色化工作流可模板化沉淀（需求→实现→测试→证据门），而不是每次现场编排。
- **非目标**：不做通用 SaaS 多租户（先做单团队自部署）；不替代 Git 平台的 code review（对接而非重造）；不追求首版覆盖 codex 全部功能面（realtime 语音、cloud-tasks、remote-control 中继首版关闭）。

## 03 总体架构：保留 / 替换 / 新增

```
Web GUI（新增，React + 生成的 TS 类型）   TUI / exec（保留）
        │  WebSocket (ws://, JWT)              │ Embedded/UDS
        ▼                                      ▼
┌─ Ensemble Server（app-server 魔改：+身份 +项目空间 +角色路由）─┐
│                                                                │
│   core 引擎（保留） ── Provider Gateway（替换/扩展）           │
│   ├ 编排扩展（新增，ext/ 内部扩展形式：角色、流水线、门禁）    │
│   ├ sandboxing / permissions（保留，按角色配 profile）         │
│   └ rollout JSONL + SQLite（保留，+项目/组织维度）             │
└────────────────────────────────────────────────────────────────┘
```

| 层 | 处置 | 说明 |
| --- | --- | --- |
| core 引擎（Session/Turn/工具/审批/压缩） | 保留 | 不动核心循环；新能力以 `ext/extension-api` 内部扩展形式挂入，降低与上游的合并冲突面 |
| app-server 协议与传输 | 保留 + 新增 | 沿用宏表与 fixture 机制；新增方法走同一张表（org/project/role/pipeline 族），天然获得 TS 类型与契约测试 |
| 模型接入（client.rs，Responses API 耦合） | **替换** | 见 §04，最大的一块改造 |
| TUI / exec / SDK | 保留 | 开发者个人工位继续可用；exec 的 JSONL 事件流兼作 CI 集成面 |
| 身份 / 项目空间 / 角色 | 新增 | ws 传输已有 JWT 验证；加组织/项目/成员模型与按角色的 permission profile 映射 |
| Web 协作 GUI | 新增 | 见 §06；协议 TS 类型由现有 ts-rs 管线直接生成，前端零手写类型 |
| realtime 语音 / cloud-tasks / remote-control / ChatGPT connectors | 首版关闭 | feature-gate 掉，减小维护面；connectors 依赖 OpenAI 账号体系，与多模型目标冲突 |

## 04 改造一：Provider Gateway（多模型 + 本地模型）

codex 深度耦合 Responses API：WebSocket 增量流、`response_id` 续传、服务端压缩（remote compact v1/v2）、加密 reasoning 项。好消息是细读发现**它的降级路径全部现成**——这正是策略化组织的红利：

- 仓库里已有 `ollama`、`lmstudio`、`model-provider`、`models-manager` crate，说明多后端是上游认可的方向；
- 压缩按 `RemoteCompactionSupport::Unsupported` 自动落回**本地 Memento 摘要**，不支持服务端压缩的模型无需任何新代码；
- WebSocket 流本来就能回落 HTTP SSE；工具并行、审批、沙箱与模型层完全正交。

改造方案：在 `client.rs` 之下引入 **Provider Adapter trait**（Chat Completions / Anthropic Messages / OpenAI-compatible 本地端点三个实现），核心工作三件：

1. **能力矩阵下沉**：把"支持并行工具吗、支持严格 JSON 吗、支持推理内容吗、上下文多大"做成 provider capability（codex 已有该形状，扩充维度即可），所有上层按 capability 分派而非按厂商 if-else；
2. **流事件归一**：各家 SSE 归一为现有 `ResponseEvent`（OutputItemAdded/Delta/Done/Completed），复用全部下游管线；
3. **弱模型工具回退**：对无原生 function-calling 的本地模型，提供文本协议工具调用解析器（受 capability 门控），并把 `execpolicy` 决策收紧一档（弱模型 + 宽权限是最大风险组合）。

## 05 改造二：团队服务端（身份、项目空间、角色）

- **身份**：启用 ws 传输的 SignedBearerToken（JWT）路径；服务端加成员表（SSO 后置）。每个连接握手即绑定 user + role。
- **项目空间**：现有 thread 元数据（SQLite 投影）加 `project_id` / `org_id` 维度；rollout 目录按项目分层。工作区代码检出放服务端受控目录（沙箱三平台已就绪），或经 exec-server 接入成员自带 runner。
- **角色 = permission profile + 工具曝光 + 审批路由**：这是 codex 权限模型的自然延伸——产品角色默认 read-only profile + 计划/文档工具；开发角色 workspace-write + 全工具；测试角色 workspace-write（测试目录）+ 执行工具。审批请求（ServerRequest）按**动作类型路由到对应角色的收件箱**：网络逃逸给管理员、需求变更给产品、破坏性命令给开发 owner。fail-closed 语义原样继承。
- **实时互见**：多连接订阅同一 thread 是现成的；补 presence（谁在看哪个线程）与按项目的通知扇出。慢客户端断连、通知 opt-out 机制原样保留。

## 06 改造三：Web 协作 GUI

技术选型：React + 现有 ts-rs 管线生成的协议类型（655 个 .ts 文件的生成机制直接复用，新方法自动进入类型包），通过 ws 直连 Ensemble Server。五个核心界面：

| 界面 | 说明 |
| --- | --- |
| 任务看板 | 以 thread goals + 持久化队列为数据源的看板列；每卡片是一条 agent 流水线，状态来自 turn 生命周期通知 |
| 线程时间线 | ItemStarted/ItemCompleted 流的实时渲染（TurnItem 18 类各有卡片形态）；支持转向输入（Steer 是协议一等公民） |
| 审批收件箱 | 按角色路由的 ServerRequest 队列；每条带命令/补丁上下文与 execpolicy 裁决理由；超时即拒的剩余时间可见 |
| Diff 评审 | TurnDiff 事件（每 turn 净 diff，引擎已算好）+ 文件树；评审通过才放行合并门 |
| 代理拓扑 | agent-graph-store 的父子边实时图；点击进入任一子代理线程；异常子代理可定向中断 |

## 07 改造四：三角色工作流

以 `ext/` 内部扩展形式实现"流水线"编排器（不改 core 循环），角色间交接用现成的代理邮箱 + goal 机制：

| 阶段 | 执行者 | 机制映射 |
| --- | --- | --- |
| 需求 → 规格 | 产品 + 规格 Agent | thread goal 承载验收标准；collaboration-mode 模板固化提问式澄清流程；产出规格文档入库 |
| 规格 → 实现 | 开发 + 实现 Agent（可多个并行） | spawn 子代理各领子任务；父子拓扑入 graph-store；开发可随时 Steer / 接管终端（unified_exec 的长命 PTY） |
| 实现 → 验证 | 测试 + 测试 Agent | 复用 Review task 形态跑测试代理；hooks 的 PreToolUse/Stop 做**证据门**：测试未过、覆盖未达标即阻断"完成"声明 |
| 验证 → 交付 | 三角色会签 | Diff 评审 + 审批收件箱会签；全链路 rollout 留痕，任何结论可回放到具体 turn |

关键设计立场：**流水线是可选的编排层，不是强制的状态机**。底层永远是"可被人随时转向/中断的 thread"，避免把工具做成僵硬的工作流引擎。

## 08 与 Viden 的关系（必须诚实回答的问题）

Viden 与 codex 架构同构（core 拥有状态、瘦前端、JSONL+派生索引），三条路线比较：

- **A. fork codex（本提案）**：起点最高、最快见到团队协作产品；代价是接手约 50 万行外来代码与上游漂移。
- **B. 演进 Viden**：代码完全自主、与既有六条已接受方向连续；代价是团队协作与多代理原语要自建，到达本提案 M3 的时间预计 3–4 倍。
- **C. 混合**：Ensemble 走 fork 路线快速验证产品假设；Viden 按既定六方向继续演进，并把 Ensemble 中验证过的机制（契约 fixture、Provider Gateway 形状、角色权限映射）回灌为 Viden 的设计输入。两者共享的正是已接受的那六条方向。

**建议采纳 C**：把 Ensemble 定位为"产品假设验证 + 机制试验场"，Viden 定位为"长期自主内核"。若 M2 后 Ensemble 验证了团队协作需求为真，再决策是否将 Viden 的 core 逐步替入（两者前端契约思路一致，替换面可控）。

## 09 里程碑

| 阶段 | 范围 | 验收 |
| --- | --- | --- |
| **M0** · 2 周 | fork、rebrand、feature-gate 掉 realtime/cloud-tasks/connectors、CI 绿 | 全平台构建 + 既有测试套件通过 + NOTICE 合规 |
| **M1** · 4–6 周 | Provider Gateway：Anthropic + OpenAI-compatible 本地端点（Ollama/vLLM） | 本地模型跑通完整 agent 循环（工具、审批、本地压缩）；能力矩阵测试 |
| **M2** · 6–8 周 | 团队服务端（JWT + 项目空间）+ GUI 只读版（看板、时间线、拓扑） | 3 人同时在线观察同一项目的 agent 工作，实时事件延迟 < 1s |
| **M3** · 6–8 周 | GUI 可写（转向、审批收件箱、diff 评审）+ 角色权限映射 | 一个真实需求走完产品→开发→测试全流程，审批按角色路由 |
| **M4** · 持续 | 流水线模板、证据门、指标看板；上游 rebase 节奏化 | 两个以上团队自部署使用；月度上游同步无积压冲突 |

## 10 风险与对策

- **上游漂移**：codex 迭代极快。对策：所有新增以 `ext/` 扩展 + 新协议方法（同一宏表）实现，core 改动清单维持个位数文件；月度 rebase 节奏 + fixture 测试做合并安全网。
- **Responses 耦合深于预期**：resume 依赖 `response_id`、reasoning 加密项等。对策：M1 先做"无状态续传"路径（全量历史重放，codex 本地压缩路径已支持），性能优化后置。
- **弱模型 + 宽权限**：本地小模型的工具调用可靠性差。对策：capability 门控 + execpolicy 按 provider 收紧一档 + 默认 read-only profile 起步。
- **维护面**：~50 万行 Rust。对策：feature-gate 缩减编译面；首版明确"不修不碰"的模块清单；两名熟悉 Rust 异步的核心维护者为最低配置。
- **协作产品假设未验证**：三角色是否真愿意共用一个 Agent 界面。对策：M2 只读版即拿真实团队试用，M3 前设 go/no-go 评审。

## 11 成功指标

- M1：≥ 3 家模型（1 个本地）通过同一套 agent 循环回归；
- M2：真实团队周活 ≥ 3 角色，事件端到端延迟 P95 < 1s；
- M3：≥ 1 个真实需求全流程交付，人均审批响应 < 10 分钟；
- M4：上游月度同步平均冲突文件 < 10，自部署团队 ≥ 2。
