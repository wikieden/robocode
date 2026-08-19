# DeepSeek Harness 架构总结

> 基于 deepseek-ai/deepseek-harness pinned `47f94385`（2026-08-13，`0.1.0-rc.5`，
> MIT，developer preview——官方声明会有破坏性变更）的全模块细读（6 个并行子代理
> 分模块阅读，主会话交叉核对）· 2026-08-17
> 完整 file:line 版本见同目录《deepseek-harness-deep-read.md》。
> 与《codex-arch-summary.md》构成 Viden 参考架构双基准。

DeepSeek Harness（`dsh`）是 Node.js/TS 的 agent harness，~219 个包全部是 Cordis
插件（"everything is a plugin"），Cordis 全家被 vendored 并重命名进
`@deepseek-ai` scope。本文按层总结实际架构，末尾给出可迁移与不可迁移清单。

## 00 总分层：组合即配置

应用 = 空插件表 ← bundle patch（YAML）← profile patch ← home patch ← --patch。
bundle 没有代码只有 `cordis.patch.yml`；patch 按 row id **整体替换** config。
web-app 关掉 base 的 23 个 agent-plane 行、换成按会话挂载的 agent-presets（每
session 一个 scope，preset 服务须包 `isolate` realm）。浏览器里跑第二棵 Cordis
树（服务端插件经 `dsh.client` 双面包自带前端模块）。第三方插件 = pnpm 真装 +
主进程 import，**零隔离**（"插件即可信代码"是明确立场）。

## 01 插件契约

服务契约与事件契约写在同一个 TS declaration-merge 块（事件带 `@mode
waterfall|emit` 标签）；能力接缝三段式 Definition/Provider/Consumer + companion
policy 插件（如 read-before-edit 是可插拔事件门，不装即退化为无条件写）；换
`ctx.fs`+`ctx.subprocess` 两个 adapter 即把 bash/PTY/LSP 整体搬进远程沙箱
（e2b）。每包强制一个 `./invariant` 导出（219/219 覆盖，gate 强制：真检查或
"无不变式+理由"）。近 30 个生成文档全部配对 `--check` CI 断言。

## 02 agent 循环与事件

`driver → turn → step → tool batch`；inbox 两目标（next-turn/next-step），
followup/steer/inject 是 send 的糖。**每个流式 chunk 都落盘**（token 级重放），
每次请求从 session log 重新派生。事件双平面：`session/event`（durable 事实源）
vs `agent/*`（live、scope-filtered）。**surface 是独立的"模型可见历史"维度**：
`append | replace{start,end}` + `sourceEventSeqs`——append-only 日志与压缩由此兼
容；整值事件规则（状态事件必须带完整后状态）。前向兼容 = 生成的 KNOWN 闭包 +
逐事件 `ignorable` 逃生门 + 拒绝不迁移（版本钉 0）。system prompt 是有序注册表，
section（稳定 system 前缀）/context（动态 user-role 快照）二分保 KV cache。

## 03 持久化

JSONL 唯一事实源（zstd + chunk-run 打包 ~60%，读 layout-blind）；SQLite 三种角色
互不相干：可替换持久化后端（**实现了但无组合挂载**）、通用 KV 后端、可丢弃 FTS
读模型（版本不符 DROP 重建）。checkpoint 三语义点全 fail-closed（llm 请求前 /
顶层工具前 / pre-step 前），与批窗分离。崩溃恢复合成 `turn/end{interrupted}` 不
截断。投影：纯同步 init/apply/view + 引用不变零下游 + 共享水位；缓存 fail-soft
+ `identity={createdAt,cwd}` 防陈旧投毒。凭据明文 YAML + 0600 + 启动权限位不合
即拒启。golden 会话日志测试（record/replay/refresh 三模式，
`session.expected.jsonl`）。

## 04 工具、审批、沙箱

工具契约四件套：schema 白名单（只发 name/description/parameters）、强制 output
JSON Schema + 纯 render（live 与回放同一函数）、timeoutMs/isConcurrencySafe 永
不发模型、presentCall/Result 纯函数。executionMode 默认独占 fail-closed；工具错
误归一为 `isError` 回模型不杀回合；`concludesTurn` 数据驱动收束。
**审批只在沙箱升级点问人，不逐调用**——日常防线是内核沙箱（bwrap/landlock/
seatbelt/Windows ACL），审批 fail-closed（只有 allowed-once 是授权）、guards 单
调只拒不允。升级语法：denial 标记 + 同轮单次重试提示放在决策点。**plan-mode 是
软引导不阻断变异**（与 Viden invariant 相反）。沙箱 `confine(argv)` 纯包装，
enforcement full/partial 是上报的事实（windows-acl 永远 partial）；**网络完全不
管**。

## 05 子代理与外部 CLI（本仓独有价值）

4 成员 provider 接口（name / 4 个 start-time 能力布尔 / inheritsParentContext 措
辞旗 / start）；能力校验先于 provider、fail loud。SessionId 即子代理地址；血缘
进 session header（depth 是单调下界）。result never-reject，stopReason 词表含
refusal。每个外部 CLI 一种策略：**Claude Code 走官方 Agent SDK（钉 0.3.220，只
接管进程 spawn，读宿主原生 settings，不桥接审批）；Codex 走自研最小 app-server
JSON-RPC（钉 0.147.0，审批硬编码无人值守拒绝，未知请求即 fatal）；ACP 作通用客
户端（部署级 allow/reject，绝不弹人类）；dsh-sdk 驱动完整的第二个 dsh**。共性：
无人值守、fail-closed、单 turn 交付、外部 CLI 的递归预算父方不设防。转录归属三
分（coordinator/report/settled）。hooks 包是 CC/Codex 方言翻译层，与 subagent
驱动零配合。

## 06 Web 操作台与 SDK

上行 = 自定义四象限信封 over POST（业务错误走 `{ok:false}` 带内，不走 HTTP 状态
码）；下行 = downlink-only WebSocket 帧流。多标签一致性靠**全量快照帧**（审批
rpcId 原样复用，新标签可应答同一审批）+ **host 计算的投影推送**（客户端零业务
reducer——与 Viden 立场一致，机制不同）。**无认证**：loopback/Origin 三关信任栅
栏 + `--host 0.0.0.0` 显式拒绝 + 16 个特权方法钉 loopback。无协议版本号（前提：
client 与 host 同发）。`packages/sdk` 是第三套协议（stdio JSON-RPC 2.0，仅
initialize/session.prompt/shutdown），与 `/api` 零共享。

## 07 防御模式（七条实战教训）

正交结果各报各的（timedOut 与 exitCode 独立）；公共契约两侧归一化错误形态；异
步状态不当同步结果用（显式定义等待区间 + "无可等待"分支）；dispose 抵达静默
（kill → await done，先关监听器）；dispatcher 收容回调异常；擦洗 env + 0700/wx
私有临时文件；link 用 unlink 删、递归 rm 只给已知真实目录。

## 08 与 codex 的跨栈共识（采信度最高）

审批 fail-closed 默认拒绝；JSONL 追加日志为事实源、派生索引可重建/可丢弃；压缩
即日志检查点且与策略解耦；工具错误回模型而非杀回合；headless/SDK 面是独立稳定
schema。

## 09 可迁移清单

| 设计 | 一句话 | Viden 落点 |
| --- | --- | --- |
| 子代理 provider 形状 | 4 成员接口 + SessionId 地址 + never-reject + 每 CLI 一策略 | `crates/runtime` agent adapter + `plugin-api` |
| surface 机制 | append/replace + sourceEventSeqs，append-only 与压缩兼容 | `crates/session`/`crates/context` |
| KNOWN 闭包 + ignorable | 未知事件拒绝重建而非静默丢弃 | `crates/types`/`crates/session` |
| 投影三层 | 纯函数单元 + 共享水位 + fail-soft 缓存带 identity | session SQLite 索引 + 前端快照契约 |
| checkpoint 三语义点 | 请求前/工具前/pre-step 前 fail-closed | `crates/session` 写路径 |
| 工具契约收紧 | 强制 output schema + 纯 render 共用 = TUI/GUI parity | `crates/tools` + 契约 fixture |
| 升级语法 | denial 标记 + 同轮单次重试 + 依赖倒置的 approver | `crates/permissions` |
| skills 目录注入 | 耐久 user-role 替换目录 + digest 变更检测 | `crates/context` |
| 工程治理三件 | invariant 全覆盖 gate / 生成文档配 --check / golden 会话日志 | `scripts/` + CI + fixture 计划 |
| 防御模式七条 | 写进编码标准，作为 tools/runtime 审计项 | `docs/development-standards.md` |
| 信任栅栏 | loopback 三关 + 拒 0.0.0.0 + 特权方法钉 loopback | 任何本地 HTTP 面 |

## 10 不可迁移 / 显式分歧

| 分歧 | dsh 立场 | Viden 决定 |
| --- | --- | --- |
| 运行期插件树 | Cordis 动态组合、插件零隔离 | 保持 Rust 静态 + 描述符；只抄接缝纪律 |
| plan mode | 软引导不阻断 | **维持硬阻断**；可取 exit_plan_mode 常驻工具表 |
| 审批位置 | 只在沙箱升级点 | 沙箱执行层落地前维持逐调用决策，防保护真空 |
| 协议版本 | 无版本号（client/host 同发） | 保持 schema 1 + 能力握手，不退回 |
| 凭据 | 明文 + 0600 硬校验 | 权限位校验可补；目标形态仍对照 codex |
| token 计量 | 4 字符/token 启发式 | 保持 provider 精确计量 |
| 前端插件化 | 浏览器第二棵 Cordis 树 + ui-slots | GUI 保持契约客户端；ui-slots 仅远期参考 |
| 网络沙箱 | 明确不管 | 该维度以 codex 四层栈为参照 |
