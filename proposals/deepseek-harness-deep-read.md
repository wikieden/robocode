# DeepSeek Harness 架构细读报告（deepseek-ai/deepseek-harness，pinned 47f94385）

> 来源：浅克隆于 scratchpad 的 deepseek-harness 主干，pinned commit
> `47f943859bef60e4160492346772ded9b24f765a`（2026-08-13，`0.1.0-rc.5`，MIT）。
> 官方自述 developer preview："THERE WILL BE COMPATIBILITY-BREAKING CHANGES"——
> 所有 file:line 只对该 SHA 有效，引用前先核对。
> 6 个并行子代理分模块细读（组合模型 / agent 循环 / 持久化 / 子代理委托 / Web 面 /
> 工具与沙箱），主会话交叉核对汇总。
> 本报告与《codex-architecture-deep-read.md》构成 Viden 参考架构双基准
> （用户 2026-08-17 指令：需求/架构决策同时咨询 openai/codex 与 deepseek-harness）。

---

## 0. 总分层（实际形态）

Node.js/TS pnpm monorepo，~219 个包（`packages/<family>/<pkg>`），跑在 vendored 并
重命名到 `@deepseek-ai` scope 的 Cordis 上（`docs/rescope.md:11-19`）。

```
apps/cli（launcher：只解析 --profile/--patch/--dump-config，其余 argv 冻结转交）
  └─ boot()（packages/boot/app-boot/src/index.ts:757-802）
       Context + Loader ← cordis.patch.yml 层叠：
         空表 ← bundle patch（按序）← profile patch ← $DSH_HOME patch ← --patch
       ├─ host 平面（一棵 Cordis 树，一切能力都是插件）
       │   ├─ 能力接缝：ctx.fs / ctx.shell / ctx.sandbox / ctx.subprocess /
       │   │   ctx.llm / ctx.subagents / ctx.skills / ctx.jobs / ctx.terminals …
       │   ├─ core：ctx.agents（契约）+ ctx.agentLoop（默认驱动）
       │   │   + ctx.sessions / ctx.tools / ctx.systemPrompt
       │   └─ web host：webserver / apiproxy（遗留 BFF）/ api-gateway（typert）
       └─ client 平面：浏览器里的第二棵 Cordis 树
           （dsh.client 双面包 + ui-slots 插槽 + host 注入 boot 图）
```

**三个最重要的结构事实**：

1. **组合即配置**：bundle 不是代码是 YAML patch（`dsh.bundle.patch` 指向
   `cordis.patch.yml`，`packages/boot/app-boot/src/profile.ts:41-62`）；patch 按
   row id 命中并**整体替换该行 config（不合并）**。web-app bundle 靠 `disabled:
   true` 关掉 base 的 23 个 agent-plane 行、换成按会话挂载的 agent-presets
   （`packages/bundle/web-app/cordis.patch.yml:420-424`）——同一产品的两种形态只是
   两份 patch。
2. **`agent`（契约）与 `agent-loop`（默认实现）是分开的包**：扩展插件依赖 seam，
   循环驱动本身可替换（`packages/core/README.md`）。
3. **事件双平面**：`session/event` 是 durable、可重放的事实源；`agent/*` 是 live
   协调事件（scope-filtered dispatch，监听器只收自己 agent 的事件）。二者词表、
   投递、消费者完全分离（`packages/core/agent/src/runtime-types.ts:1-3`）。

---

## 1. 插件/组合模型（Cordis 的实际用法）

### 1.1 插件形态与契约声明

- 服务包 = default-export 一个 `Service` 子类；函数插件 = 具名导出
  `name`/`inject`/`Config`/`apply` 且**不得有 default export**（混用会让 Loader 丢
  namespace，`packages/AGENTS.md:5` + `docs/postmortem/0001`）。
- **服务契约与事件契约写在同一个 declaration-merge 块里**：`packages/fs/fs/src/index.ts:44-77`
  同时声明 `Context.fs` 与 `fs/write-intent`/`fs/edit-intent`/`fs/observed` 三个事
  件，每个事件带 `@mode waterfall|emit` JSDoc。插件间同进程契约就是 TS 声明合并
  ——**typert 不是插件间契约机制**，只覆盖 Host service → 浏览器 Remote 这一条边
  （构建期从 ts.Program 生成 wire 描述符，产物不入库）。
- 可选服务必须 `ctx.get(name)`；`ctx.<name>` 只留给声明过的 inject（property
  proxy 对加载拓扑敏感，`packages/AGENTS.md:6`）。
- 配置校验用 schemastery/zod，装载时失败即整行 fail loud。

### 1.2 scope：per-agent 作用域（纯库，无服务 key）

`packages/core/scope`：`createScope(ctx, key)` 用 no-op 插件开 fiber 再
`extend({[kScope]: key})`，key 是任意 object 身份（`src/index.ts:137-147`）。一条
父链同时驱动两个反向语义（`:33-39`）：**注册视图向下继承**（子 scope 看到祖先
layer），**事件准入向上扩展**（祖先上的 listener 收得到后代事件）。建 scope 的只
有两处：每个 Agent 一个（`agent-loop/src/agent.ts:94`）、每个 preset 一个常驻
scope（`agent-presets/src/index.ts:515`），用 `bindScopeParent` 串起来——"preset
挂一次、每个 session 经 scope 父子关系加入"。tools/system-prompt/skills/jobs/
commands 全部用 `ScopedLayers`（global + 祖先链 overlay，最近者胜）。

### 1.3 装配：bundle / profile / preset 三层

- profile = `$DSH_HOME/profiles/<name>`：`dsh.profile.bundles` 有序列表 + 用户
  patch；出厂模板 `web = [dsh-base, dsh-web-app]`（`profile.ts:114-117`）。
- base bundle 一次 insert 挂 ~78 行（`packages/bundle/base/cordis.patch.yml`）：
  llm 族、session 族、agent 族、执行世界（subprocess/sandbox/fs/shell）、策略
  （approval/presets/timeout/spill/repeat-reminder）、全部 tool-*、subagent、
  goal/plan/compaction、settings/credentials。
- preset = 一份 agent 平面的 cordis.yml（`apps/cli/config/agent-presets/*/`）。
  preset 内 provide 服务的行必须包在 `isolate` realm 的 `cordis:group` 里，否则
  publish 进 root realm 变进程全局，挂载时直接拒绝
  （`agent-presets/src/mount.ts:181-191`）。`isolate` 是命名可见性 realm，
  **不是安全边界**。
- 第三方插件：`dsh plugin add` 转发真 pnpm + `dsh.profile.bundles` 对账
  （`apps/cli/src/plugin.ts:120-137`）。**加载即主进程 import()，拿完整
  Context，与 host 同权限——没有任何隔离**；文档明说 git 依赖的 prepare 脚本
  在"agent 沙箱之外"执行（`docs/user/develop/basic/publish.md:173`）。
  `plugin-inventory` 名不副实：72 行的 Loader entries 只读投影，不发现不加载。

### 1.4 工程治理（比机制本身更可抄）

- **每包一个 `./invariant` 导出，覆盖率 219/219，由 gate 强制**
  （`scripts/verify-package-invariants.ts`）：要么真检查一条运行期关系（例：
  scope 的 invariant 挂 `internal/dispatch`，强制作用域事件必须用 scopeTarget 派
  发），要么空 installer 附 `No runtime invariant: <理由>`——无理由的空实现判失败。
- **文档是带 CI 断言的产物**：近 30 个 `gen-*`/`verify-*` 脚本配对
  （module-graph / tool-catalog / persistence-catalog / capability-seams /
  event-producer-consumer …），`docs/graph-atlas.md` 给每份文档标注维护模式
  （generated / hybrid / curated）。tool-catalog 生成器**真的 boot 每个插件读
  `ctx.tools.schemas()`**（schema 不可静态知），并 glob `packages/*/tool-*` 做完
  整性守卫。
- **fail-loud 启动审计**：entry 未激活时区分"模块解析失败 / FAILED 取原始
  reason / PENDING 列出未解析服务名"（`app-boot/src/index.ts:658-725`）；迟到的
  unhandledRejection 变成一行 stderr + exit(1)。
- 漂移示例（阅读时需防）：出厂 preset 注释引用已删除的 `base.cordis.yml`/
  `web.ts`；HMR 在两条出厂链路上都 `disabled: true`（真正存活的只有配置层热重
  载）；`docs/persistence-catalog.md` 实为会话事件类型目录而非持久化全景。

---

## 2. 核心 agent 循环与事件模型

### 2.1 回合状态机

驱动是 `ReactLoopAgent`（`packages/core/agent-loop/src/agent.ts:64`）。相位三态
`idle | maintenance | running`（maintenance 对外映射成 idle）。层次
**driver(kick) → turn → step → tool batch**：

- inbox 两目标：`next-turn` / `next-step`；`claim` 排空整个 next-step + 至多 1 条
  next-turn（`agent/src/inbox.ts:71-78`）。`followup`=next-turn+wake、
  `steer`=next-step+wake、`inject`=next-step+不 wake——三个动词是 `send` 的糖
  （`agent.ts:122-132`）。
- `turn()`：append `turn/start` → step 循环。pre-step waterfall 返回 reject →
  turn 以 `blocked` 收束且不消耗 step；`max-tokens` 粘性（一旦触顶后续 completed
  不得降级 turn 结果）。停止检查点 `agent/turn-stopping`（serial）在 dispatch
  **前后各查一次** inbox——"数据决定停止，监听器顺序不影响结果；要反对就
  steer"（`agent.ts:295-299`）。
- `step()`：`buildRequest` → `llm.stream` → **每个 chunk 都 append
  `assistant/chunk` 落盘**（token 级重放保真）→ `BlockAssembler` 折成
  `assistant/message`（`sourceEventSeqs` 引用组成它的 chunk seq）→ 工具批 →
  `concluded ? completed : null`（null = turn 未结束，继续下一 step）。
- **每次请求都从 session log 重新派生**（`session.deriveMessages()`，
  `agent.ts:341`）；`request/header` 仅在首次/变更时落盘
  （reason `initial|resume|change`）。取消是单一 AbortSignal 贯穿；abort 时已开
  始的调用排空提交、未开始的补写合成 `isError` 结果，保证重放合法
  （`tool-calls.ts:240-259`）。

### 2.2 事件与 surface

- `SessionEvent = {type, seq, time, data, ignorable?}`，seq 单调连续、全量
  lossless JSON。`SessionEventMap` 是 merge-extensible 接口，插件 `declare
  module` 合并自己的事件类型（`core/session/src/types.ts:236`）。
- **前向兼容策略 = 闭包白名单 + 逐事件逃生门**：`KNOWN_SESSION_EVENT_TYPES` 生成
  闭包（`known-event-types.ts:19`）；读者遇到不认识且无 `ignorable` 标记的事件
  **必须拒绝重建而非静默丢弃**。`SESSION_FORMAT_VERSION = 0` 钉死：发布前
  **拒绝而非迁移**，是否 bump 由写方决定（`types.ts:35-56`）。
- **surface 是独立于事件日志的"模型可见历史"维度**：只有
  `user/message | assistant/message | tool/result` 三类可上 surface；
  `SurfaceOp = 'append' | {op:'replace', start, end}`，replace 用于压缩且必须在
  `sourceEventSeqs` 列全被遮蔽节点；条件类型强制非 surface 事件不得携带 surface
  元数据（`types.ts:343-436`）。**append-only 日志与上下文压缩由此兼容**。
- **整值事件规则**：状态类事件必须携带完整变更后状态、绝不携带裸 delta——这是
  todo/goal/plan/title 全部是快照事件的原因（`session-projection/src/index.ts:13-16`）。

### 2.3 模型接入 / 重试 / 压缩

- adapter 抽象只有一个必实现 `stream(): AsyncIterable<StreamChunk>`；
  `registerAdapter` 返回可原子替换路由的 handle；`prepareCall` 把调用绑定到解析
  出其精确模型默认值的那次注册（`packages/llm/llm/src/index.ts:155-256`）。
  `llm/stream` waterfall 是唯一按调用拦截点。
- `llm-retry` **不包裹 stream()**，挂 `agent/request-error` waterfall；每次重试是
  新编号 turn；无跨 provider 降级（policy 随注册捕获）。
- compaction 接缝三操作（`compactIfNeeded/compactNow/compactRegion`），范围是
  surface 位置区间而非 seq 区间，导出 tool 配对平衡检查。`compaction-basic` 两触
  发点：pre-step 压力检查、request-error 只认 canonical overflow 且**只有
  `surface.replaceGeneration` 前进才授权重试**。摘要请求**逐字重放会话自身
  system prompt + tools + 被遮蔽消息**以复用 provider KV 前缀缓存；事务
  bracket-first：`compaction/start` 同步落盘 → 摘要 → 重校验 → `summary` + 替换。
  `compaction-tool-result-pruner`：无模型纯剪枝，新 append 一条带
  `surfaceOp:replace` 的 `tool/result`，原事件在日志里完整保留。
- `spill`：超量工具输出外溢到 0700 私有目录 + `wx`/0600 独占写；`spill-policy`
  是 `tools/post-execute` 变换器，best-effort（存储失败只 warn 绝不把成功调用变
  isError），跳过 `read` 工具防 read→spill→read 死循环。
- system prompt 是有序注册表：`section`（进 system slot，name 唯一、scoped 可
  shadow 同名全局——preset 覆盖 persona 的机制）与 `context`（进**动态 user-role
  快照**，经 `RuntimeContextProjection` 仅在与已保留快照不同时才产生新消息）二
  分——稳定前缀留 system、易变状态走可追加 user 快照，KV cache 友好
  （`core/system-prompt/src/index.ts:381-446`；order 约定 -100 identity /
  0 persona / 100-199 工具引导）。

---

## 3. 会话与持久化

### 3.1 事实源与后端

- **JSONL 事件日志是唯一事实源**。`session-persistence` 是抽象缝（
  `locate/create/append/prepare/load/inspect/readFrom/list/readRaw`），JSONL 与
  SQLite 是**可替换后端**跑同一套 contract 测试——但 grep 全部 shipped 组合，
  **没有任何组合挂载 SQLite 后端**，生产事实上只有 JSONL。
- 磁盘记录层 ≠ 事件层：连续 `assistant/chunk` 打包成 chunk-run 行（实测日志缩小
  ~60%），**读永远 layout-blind**（`core/session/src/chunk-rows.ts`）。路径
  `~/.dsh/sessions/--<项目slug>--/<encodeSegment(sessionId)>/session.jsonl.zstd`；
  `encodeSegment` 对全部 UTF-16 码元单射编码，中和 `../`/NUL/分隔符（SessionId
  是未校验 branded string，落盘前必须过安全编码，`format.ts:121`）。
- **checkpoint 是三个语义点、全部 fail-closed**（检查点失败 ⇒ 下游不执行）：
  `llm/stream` 前、顶层 `tools/execute` 前、`agent/pre-step` 前
  （`session-checkpoint-policy/src/index.ts:63-81`）。耐久性归 checkpoint，批窗
  归 write-behind——职责显式分离。
- 损坏恢复：崩溃在 turn 中间**不截断**——冷加载合成
  `turn/end {reason: interrupted}`（唯一 loop 不会发出的 reason）；只有物理撕裂
  尾部被丢弃；拒绝分两类错误（`FormatUnsupported` ≠ `Corruption`）。

### 3.2 投影与查询

- `session-projection` 是能力缝：域包只给三个纯同步函数 `init/apply/view` + zod
  schema + stateVersion；`apply` 未命中必须返回同一引用（引用不变 = 零下游工
  作）；`ProjectionSnapshot.asOfSeq` 是共享水位给出一致读切面。
- `session-projection-cache` 是折叠捷径**绝非权威**：ver 不匹配即丢弃不迁移，全
  写路径 fail-soft；`identity = {createdAt, cwd}` 绑定日志生命周期——session id
  只是槽位，删除重建不能让旧缓存行喂进无关日志（防陈旧缓存投毒）。
- `session-query-sqlite` 是**可丢弃派生 FTS 读模型**：版本不符直接 DROP 重建
  （与持久化"拒绝不迁移"形成对照——派生数据可重算所以能推倒）；application_id
  与持久化库刻意差一。默认 `:memory:`，全文搜索默认关闭但精确读可用。
- 通用 KV：`storage` 三段式 hub/backend/domain；读同步（内存权威）、写走每域单
  链"先后端耐久→再改内存→再发 changed"；域版本不符 = 拒绝，无迁移。
- 凭据：**明文 YAML + 0600 + 启动时 `assertOwnerOnly` 检查权限位，有 group/other
  位直接拒绝启动**；无 keyring。四层优先级：继承环境（只读最高）>
  `.credentials.yaml`（唯一可写）> cwd `.env` > home `.env`。配置只带
  `CredentialRef` 引用，消费者每次操作重新解析（这就是热更新机制）。
- `atomic-write`：`wx` 独占创建兄弟临时文件（拒符号链接投毒）+ 新 inode 直接带
  mode + rename 提交；**明确不做 fsync**（TODO 注明）。锁：竞争者永不删除既有锁
  （文件年龄证明不了持有者已死）。

### 3.3 golden 会话日志测试

`vitest.snapshot.config.ts` 三模式：`replay`（默认无 key，从录制的模型响应跑真实
路径，diff 组装请求/归一化输出/**持久化日志期望输出**）、`record`（真调 API 更新
fixture）、`refresh`（重放脚本只更新期望）。`session.expected.jsonl` 是会话日志
的黄金文件——任何事件形状/打包布局/顺序变化都在 snapshot 层炸出。replay 并行、
record/refresh 强制串行。

---

## 4. 工具、权限/审批、沙箱

### 4.1 工具契约四件套

`ToolDefinition`（`packages/core/tools/src/index.ts:222-288`）：

1. `ToolSchema`——**只有 name/description/parameters 发给模型**（白名单）；
2. `output`（强制）：canonical JSON Schema + 纯 `render(args,value)` 投影 + 可选
   presentationMeta——工具体只返回无损 JSON，渲染是纯函数（**实时流式与日志回放
   两处调用同一函数**，天然保证 live/replay 呈现一致）；
3. 执行元数据：`timeoutMs`（由独立 timeout-policy 插件执行）、
   `isConcurrencySafe(args)`——**都永不发给模型**；
4. `presentCall/presentResult` 纯函数。

**没有权限需求字段**——权限不是工具自声明属性（见 4.2）。并行门控：
`executionMode` **默认 exclusive**（未声明/抛异常/非 true 全部独占，fail-closed）；
parallel 进有界滚动池（默认 10），**dispatch 可重叠但 policy、结果提交、结果上下
文严格保持模型顺序**（`commitReady` 只沿连续就绪槽位推进）。工具错误一律归一化
为 `isError` 的 `tool/result` 回给模型，不终止回合；提前收束只有数据驱动的
`concludesTurn: true`。`additionalContexts` 是工具/守卫向模型追加上下文的通道
（缓冲进 next-step，不新增事件类型）。

### 4.2 审批：只在升级点，不逐调用（与 Claude Code / Viden 现行都不同的路线）

- `ctx.approval` 彻底 fail-closed：`ApprovalOutcome` 只有 `allowed-once` 是授权且
  只授权那一次；缺失/异常/非法值归一为 `unavailable` = 拒绝；无 approval 服务时
  `ask` 一律降级 deny（`tools/src/index.ts:1693-1725`）。Guards 单调：只有拒绝没
  有 allow，后注册者无法翻案。
- **全仓 `approval.request` 的消费方只有沙箱升级与 hooks 桥**——日常防护由内核级
  沙箱承担，审批只在模型请求**放宽沙箱**时出现。升级语法
  （`sandbox/src/escalation.ts`）：denial 标记
  `[sandbox: file access denied under <mode> mode]` + 同轮提示"retry this exact
  command once with sandbox_permissions"**放在决策点**（不依赖模型回忆工具描
  述）；`approveEscalation` 先查严格更宽（不更宽直接抛、不打扰人类）→ 查服务存
  在 → 问 → 逐 outcome 映射逐字错误文本。该包不 import approval，只要一个结构化
  `EscalationApprover` 闭包传入——干净依赖倒置。
- presets **只捆绑不执行**：把 `sandbox/mode` + `approval/policy` 两个正交旋钮打
  包成 UI 选择器，`current()` 从旋钮反推而非读自己的事件。库默认保守
  （`read-only`）、出厂组合放宽（`workspace-write`+`ask`）——**library-default
  fail-safe / bundle-default 产品化**的双层默认值。
- **plan-mode 是软引导，不阻断变异**（`docs/subsystems/plan.md:5`）：只注入
  `plan:policy` 提示段；sandbox 与 approval 独立执行限制、不读写 plan 状态；
  `exit_plan_mode` 常驻工具表（保 KV cache 稳定）仅执行期拒绝。
  ——与 Viden"Plan mode rejects all mutation paths"invariant **相反**，是需要显
  式决策的分歧点。

### 4.3 沙箱：决策/执行接缝

- 决策：`ctx.sandboxPolicy` 唯一拥有默认/覆盖解析（已批准显式 mode > session
  log 最后一条 `sandbox/mode` > 部署默认）。执行：`ctx.sandbox.confine(argv,
  policy) -> ConfinedArgv`——**纯 argv 包装**，返回替换 argv + `enforcement
  (full|partial)` + `denialSignatures` + `runnerFailureRules`；禁止静默透传，无
  后端抛 `SANDBOX_UNAVAILABLE`。**enforcement 是被上报的事实不是承诺**；
  windows-acl 永远 `partial`（Everyone 可写对象 + NTFS 硬链接别名，已知不可修
  复）——要求绝对边界的消费方必须显式拒绝 partial。
- 平台链：linux bwrap→landlock（自带 native launcher）、darwin seatbelt（唯一候
  选不探测）、win32 受限令牌+ACL。探测是**功能性探测**（真跑一次 `true`）。
  stderr 双分类器按后端方言（denial vs runner-failure），消费方先判 runner
  failure 再判 denial。
- **网络完全不管**：`SandboxMode` 词表明确排除网络与进程可见性——要网络管控换整
  块能力（e2b 远程沙箱），不是 sandbox provider 的事。（codex 的四层网络栈仍是
  该维度唯一参照。）
- 单一可写根集合 `writableRoots()` 被 seatbelt profile 与进程内 fs 围栏共用（防
  漂移）——但 bwrap 分支实际给 `--tmpfs /tmp`（全新空 tmpfs），同一
  `workspace-write` 语义跨 rung 不等价（已核实的真实漂移）。
- 进程内 fs 围栏（`fs-sandbox`）定位诚实："可信代码里对模型控制路径的策略检查，
  不是内核边界"；canonicalize-then-contain + `checkedTarget()` 返回被检查的
  target 本身给 mutation 用（杜绝 check-here-write-there）。
- **read-before-edit 是可插拔事件门不是工具内建**：`fs-observation-policy` 纯事
  件插件（不注册服务、状态在自己的 WeakMap），不装它工具退化为无条件写。

### 4.4 长命进程 / MCP / skills / workflow

- 两条独立路径：`ctx.jobs`（kind 无关后台作业注册表，owner 授权而非 id 保密；
  bash/terminal/subagent 共用 `job_list/output/kill`）与 `ctx.terminals`（真 PTY
  持久会话）。`terminal-bash` 不变式：owner 有开着的 PTY 时，`sandbox/mode` 切换
  在事件提交前被拒——防"宽模式开的终端活过降级"。
- MCP **只有客户端**：每个 server 一个插件实例，工具名 `mcp__<server>__<name>`
  （自述与 Claude Code/Codex 同形状）；`failOnStartupError` 默认 false（server
  挂了不阻断启动，工具静默缺席）。
- skills：目录级注入**不是塞 system prompt**，而是 `agent.inject()` 追加耐久的
  user-role 替换式目录（只含 name + 转义 description，不含 body/路径）；每步前
  重算 digest（覆盖结构化 entries 而非渲染散文），变了才追加完整替换；技能正文
  只在 `skill({name})` 调用时载入。目录预算 `catalogDescriptionMaxLength` 默认
  500 字符/条。
- `workflow`：模型写的编排脚本在 worker_thread 里跑，`meta` 先按 schema 校验再
  执行任何脚本文本。`ralph` = fresh-agent 循环的固定前台 workflow（不可变
  objective、每轮全新 child、共享 workspace 即记忆、轮间只传有界结构化
  handoff）——**专用编排策略只是普通插件，agent-loop 里没有 Ralph 模式**。
- `code-runtime` 定性："Containment, not a security boundary"——信任姿态等同
  bash，但多了 bash 没有的收容（独立 isolate、空环境、堆上限、硬终止）。

---

## 5. 子代理委托与外部 CLI（本次调研重点）

### 5.1 核心抽象：4 成员 provider 接口

`packages/subagent/subagent/src/types.ts:285-324`：`name` + `capabilities`（仅 4
个 start-time 布尔：`outputSchema/depthLimit/toolFilter/persona`）+
`inheritsParentContext`（**描述性**，只用于生成模型侧措辞）+
`start(request): Promise<SubagentRun>`；可选 `prepareContinuable`——**方法存在性
即能力**。能力校验在 provider 之前发生、失败即抛（"fail loud, no silent
degradation"）。

- **SessionId 即子代理地址**：本地 run 的 id 必须等于已发布子 session id；远程
  provider 在父命名空间铸 `SessionId(randomUUID())`。
- 血缘持久化在子 session header：`parentSession / origin:'subagent' /
  delegationDepth / seedLength`；深度取
  `max(header.delegationDepth, options.subagentDepth)`（持久化值是单调下界，防
  resume 后当 top-level）。身份另有 log-only `subagent/descriptor` 事件（不进模
  型历史、抗 compaction）。
- `SubagentRun.result` **发布后永不 reject**：子级失败摊平成
  `stopReason ∈ {completed, aborted, error, max-tokens, refusal}`；`dispose()`
  幂等且必须抵达进程静默。
- 模型侧三工具：`tool-subagent`（入参只有 description/prompt/run_in_background
  ——persona/toolFilter/maxDepth 全是部署配置模型不可见；每实例绑一个 provider，
  可多实例不同 toolName）；`tool-subagent-control`（`send_message` 成为子代理下
  一个 FIFO turn、`interrupt_agent` 只停当前 turn）；`tool-subagent-report`
  （只装进 continuable 进程内子代理 scope；**子代理自身是凭据**，不能指定收件
  人；report 不结束 turn）。
- 转录归属三分（`continuation.ts:57-96`）：`coordinator`（父→子转发）/
  `subagent-report`（子选的内容）/ `subagent-settled`（**runtime 自己**对结局的
  陈述）——"合并它们的转录会把子代理没说过的话记在它头上"。

### 5.2 四个外部 provider：每个 CLI 一种策略

| provider | 协议 | 版本钉死 | 审批桥接 | 部分输出 |
| --- | --- | --- | --- | --- |
| `subagent-claude-code` | 官方 `@anthropic-ai/claude-agent-sdk` 的 `query()`，harness 只接管进程 spawn | SDK 0.3.220 | **不桥接**：读宿主原生 Claude settings；禁 `AskUserQuestion`；无 canUseTool 回调，无人值守交互直接失败 | **丢弃**（只消费 result 消息） |
| `subagent-codex` | 自研最小 app-server JSON-RPC（`codex app-server --stdio`），强制 `thread.ephemeral===true` | codex 0.147.0 | **硬编码无人值守拒绝**：approval→cancel/decline、permissions→空集、elicitation→decline；未知 server request → fatal | 保留（AssistantOutputFold） |
| `subagent-acp` | 官方 ACP SDK ClientSideConnection；故意不宣告任何 client 能力（无 fs/terminal） | — | 部署级固定策略 `allow\|reject`（默认 reject），绝不弹人类 | 保留；唯一能产出 `refusal` |
| `subagent-dsh-sdk` | dsh 驱动**另一个完整 dsh**（stdio JSON-RPC，子进程有自己的 cordis.yml） | — | 子 runtime 自己的部署负责 | 保留 |

共性（`out-of-process.ts`）：全部 `NO_START_CAPABILITIES`（父方数值 maxDepth 在
mount 期直接 fail loud，组合必须写 `maxDepth: 'provider-managed'`——**外部 CLI 的
递归预算父方不设防**）；cwd = 配置覆盖 > 父 session cwd，无则 fail loud；
`settleRunResult` never-reject 摊平；进程早退包成 `processFailure: Promise<never>`
与每步 race（静默退出不会被误读成完成）。

**关键结论**：对外部 CLI 的委托是**无人值守、fail-closed、单 turn 交付**的形状
——权限审批从不冒泡回宿主；交互性需求（审批/提问）在设计上直接失败而非挂起。
需要交互桥接的场景（如 Viden GUI 的 resident ACP lane）超出了它的目标形态。

### 5.3 ACP 的对称性与 hooks 的真实角色

- `packages/acp/acp` 是 **Agent 侧**（把 harness 会话暴露给自动化客户端），
  `subagent-acp` 是 Client 侧——dsh 可把另一个 dsh 当 ACP 子代理。
- `packages/hooks`（hook-protocol / hooks-claude-code / hooks-codex）是**方言翻
  译层**：让用户把已有的 CC/Codex hooks.json 指过来跑；与 subagent 驱动**零配
  合**（子进程内部 hooks 由其原生 settings 决定）。唯一耦合：hooks-claude-code
  把 `subagent/start|end` 映射成 CC 的 SubagentStart/Stop hook 点。

---

## 6. Web 操作台、API 与 SDK

### 6.1 协议：不是 JSON-RPC

- 上行 unary = **自定义四象限信封 over HTTP POST**（`ClientRequest/ServerResponse/
  ServerRequest/ClientResponse` 判别联合，`apiproxy/src/api/rpc.ts:151-186`）：
  `POST /api/<method>`，`result` 是 `{ok:true,value}|{ok:false,error}`——**业务错
  误从不走异常也不走 HTTP 状态码**。
- 下行事件 = **downlink-only WebSocket**（`/api/events.mux` + `/api/events.host`）；
  客户端发任何消息即协议违规 `close(1008)`。SSE 是同一契约的另一物理载体（留给
  其他 client 形态）。
- 帧目录：`MuxFrame`（session/event 带宿主计算的 view、approval/question 请求与
  解决、queue/jobs **全量快照帧**、projection、stream/error）+ `HostFrame`
  （session/workspace 增删、白名单转发事件）。
- 方法目录：53 个遗留 apiproxy 方法（TS interface 是唯一真源）+ typert gateway
  5 个 namespace。`docs/api-gateway.md` 只覆盖 typert 一条路径——以它为完整入口
  会严重低估面积（已核实漂移）。

### 6.2 多客户端语义（与 Viden snapshot/replay 契约同题异构）

- **没有 per-tab 身份**：一个 `broadcast()` 把帧发给所有 mux 消费者；一致性靠
  **全量快照帧**而非增量对账（打开 mux 时重放 pending approval/question——
  **rpcId 原样复用，刷新后的新标签仍能应答同一个审批**；queue/jobs 推全量）。
- **投影是"客户端不算业务状态"的核心机制**：host 计算 →
  `session/projection{key,value,seq}` 推送 → 客户端按 higher-seq-wins 维护通用
  值表，`session.history` 尾页携带 projections 基线。goal/plan/permissions/
  stats/contextMeter 全走这条路。（与 Viden"Core 唯一权威、前端零业务
  reducer"立场一致，机制不同：Viden 是版本化 snapshot/replay，dsh 是全量帧 +
  投影推送。）
- 连接握手严格：mux+host 两条流 onOpen 且 `host.describe` 成功才 onConnected。

### 6.3 无认证 + 信任栅栏

- **没有任何认证层**。每个 `/api` 请求过三关：Host 头必须 loopback 或
  trustedHosts（DNS rebinding 防护）；`sec-fetch-site: cross-site` 拒；Origin 存
  在须严格等于本 authority。`--host 0.0.0.0` 被 CLI **显式拒绝**（"would expose
  remote code execution to the network"）。
- 16 个 `PRIVILEGED_METHODS`（settings/credentials/pickDirectory…）钉死
  loopback；typert gateway 整体是 `trusted-host` 级（注释自认"能起 session 的调
  用方本来就能跑 bash"）。
- **无协议版本号且是刻意的**："client and host ship together; introduce
  protocolVersion only when an independently released client appears"
  （`api/host.ts:1-3`）——前提是无独立发布客户端，Viden 已有独立前端版本线，不适用。

### 6.4 客户端也是插件树；SDK 是第三套协议

- 浏览器里跑第二棵 Cordis 树：host 扫描 Loader 条目的 `package.json dsh.client`
  字段组装 `window.__DSH_BOOT__`，`webServer.tapIndex` 注入 index.html；一个 npm
  包双面（`exports["."]` Node 半边 + `exports["./client"]` 浏览器半边）——**服务
  端插件自带自己的前端模块**。`ui-slots`：SlotMap 空接口 + 声明合并，插槽两轴
  （kind: single|list|keyed|chain × scope: root|session-maybe|session）。
- `packages/sdk` 与 `/api` **零共享**：stdio 换行分隔 JSON-RPC 2.0，方法目录仅
  `initialize / session.prompt / shutdown` + 4 个通知；无 schema 生成、无版本协
  商；信任边界 = "你 spawn 了这个子进程"。它是 `subagent-dsh-sdk` 的底座。

---

## 7. 防御模式（docs/defensive-patterns.md，33 行全是实战教训）

1. **正交结果各报各的**：进程可同时 `timedOut && exitCode===0`（trap 了信号）；
   绝不把一个标志的报告嵌进另一个的分支。
2. **公共契约两侧都守**：多形态错误（抛出 vs finish chunk）在穿过公共 API 前归
   一化，归一化契约写在类型定义处并用真实消费方跑通每种来源。
3. **异步状态不是同步状态**：不要把 `agent/status` 或 `whenIdle()` 当某次
   follow-up 的结果；自动化调用方必须显式定义自己的区间，并处理"无可等待"分支。
4. **dispose 必须抵达静默**：kill → await done；先关监听器再 kill，让迟到完成保
   持静默。
5. **dispatcher 里收容回调异常**：坏订阅者永不打断核心生命周期。
6. **擦洗 env + 私有路径**：spawn 的命令拿擦洗过的 env（丢 `*KEY*/*SECRET*/
   *TOKEN*/*PASSWORD*`）；临时文件 0700 目录 + 随机名 + `wx`/0600。
7. **link 形状的路径用 unlink 删**：递归 rmSync 只保留给已知真实目录（Windows
   junction 穿透教训）。

---

## 8. 与 codex 的对照（双基准速查）

| 维度 | codex（Rust） | deepseek-harness（Node/Cordis） |
| --- | --- | --- |
| 前端边界 | 协议客户端（app-server，三种传输） | 同进程插件树 + Web 全量帧/投影推送 |
| 组合方式 | 编译期 crate + 内部 contributor trait | 运行期 YAML patch 插件树 |
| 契约保真 | fixture 三角等式（宏表→入库→内嵌） | typert 生成 + `session.expected.jsonl` golden + 生成文档带 --check |
| 事件演进 | rename+alias、serde(default)、Unknown 保留 | KNOWN 闭包 + `ignorable` 逃生门 + 拒绝不迁移 |
| 压缩 | 四策略并存、检查点解耦 | surface replace 单机制 + pruner/summary 两级 |
| 审批 | fail-closed，hooks→Guardian→用户 | fail-closed，**只在沙箱升级点**问人 |
| 沙箱 | 决策 DSL + 四层网络栈 | confine() argv 包装、enforcement 上报、**无网络** |
| 子代理 | 进程内三机制（delegate/一等线程/邮箱） | 命名 provider 注册表 + **外部 CLI 驱动**（SDK/app-server/ACP/dsh-sdk） |
| plan mode | （审批/沙箱联动） | 软引导，不阻断变异 |
| 凭据 | keyring 口令 + age 加密落盘 | 明文 YAML + 0600 + 启动权限位硬校验 |

两者共同点（可视为跨栈共识，采信度最高）：审批 fail-closed 且默认拒绝；JSONL
追加日志为事实源、派生索引可重建/可丢弃；压缩即日志检查点、resume 不依赖策略；
工具错误回模型而非杀回合；headless/SDK 面是独立稳定 schema 而非内部事件倾倒。

---

## 9. 映射回 Viden：可迁移 / 不可迁移方向

### 9.1 直接可迁移（建议采纳）

1. **子代理 provider 形状**（落点：`crates/runtime/src/agent_commands.rs` 的
   agent adapter 层 + `crates/plugin-api` 的 agent 描述符）。Viden 已有 ACP
   resident session、6 个内建 AgentAdapterDescriptor、AgentSessionApprover 桥。
   照 §5 收敛：① 4 成员 provider 接口 + start-time 能力布尔 + 启动前校验 fail
   loud；② SessionId 即地址、血缘进 session header（parent/origin/depth 单调下
   界）；③ result never-reject + `stopReason` 词表（补 `refusal`）；④ 每个外部
   CLI 一种策略（CC 走官方 SDK 只接管 spawn、codex 走 app-server 最小面、ACP 通
   用）而不是一套万能协议；⑤ 转录归属三分（coordinator/report/settled），不把
   runtime 的结算陈述记在子代理头上。**Viden 与 dsh 的差异要保留**：Viden 的
   lane 是交互式的（审批经 AgentSessionApprover 冒泡给用户），dsh 是无人值守
   fail-closed——两种模式都应显式建模（无人值守模式照抄 dsh 的硬编码拒绝表）。
2. **surface 机制**（落点：`crates/session` + `crates/context`）：
   `SurfaceOp = append | replace{start,end}` + `sourceEventSeqs` 完整性 + "只有三
   类事件可上 surface" + 整值事件规则。这让 Viden 的 append-only JSONL 与压缩
   /剪枝天然兼容：pruner 新 append 一条 replace 的 tool/result，原事件留盘。
3. **前向兼容双件套**（落点：`crates/types`/`crates/session`）：生成的
   `KNOWN_*` 事件闭包 + 逐事件 `ignorable` 逃生门；"读者遇到未知且无标记必须拒
   绝重建而非静默丢弃"。与 codex 的六条演进规则互补（codex 管改名/加字段，dsh
   管插件扩表）。
4. **投影三层**（落点：`crates/session` SQLite 索引 + frontend snapshot 契约）：
   纯同步 `init/apply/view` 单元 + 引用不变即零下游 + 共享水位 `asOfSeq`；缓存
   fail-soft + `identity` 绑定防陈旧投毒；派生 FTS 索引"版本不符即 DROP 重建"
   （与持久化"拒绝不迁移"分开对待）。
5. **checkpoint 三语义点**（落点：`crates/session` 写路径）：模型请求前 / 顶层
   工具执行前 / pre-step 前，全部 fail-closed；耐久性与批窗分离。
6. **工具契约收紧**（落点：`crates/tools` + frontend 契约）：强制 output JSON
   Schema + 纯 render/present 函数（live 与回放共用一个函数保证 TUI/GUI parity
   ——正好接 Viden 的 parity corpus）；`timeoutMs`/`isConcurrencySafe` 不进模型
   面；executionMode 默认独占 fail-closed；`concludesTurn` 数据驱动收束；
   `additionalContexts` 通道。
7. **升级语法**（落点：`crates/permissions`）：denial 标记与"同轮重试一次"提示
   放在决策点、`EscalationApprover` 依赖倒置、"不更宽直接抛不打扰人"。可作为
   Viden 现有 allow/ask/deny 的 escalation 扩展，不必替换整个模型。
8. **skills 目录注入**（落点：`crates/context`）：耐久 user-role 替换式目录 +
   结构化 digest 变更检测 + 正文按需载入——比每次重渲染 system prompt 更省且 KV
   友好。
9. **工程治理三件**（落点：`scripts/` + CI）：① 每 crate 一个 invariant 声明
   （真检查或"无不变式+理由"，gate 强制全覆盖）；② 生成文档配对 `--check`（
   Viden 的 doc-pairs/doc-links 已有骨架，可扩展到 module-graph/tool-catalog 类
   生成物）；③ `session.expected.jsonl` 式 golden 会话日志测试（record/replay/
   refresh 三模式）——直接服务 frontend-contract fixture 方向。
10. **防御模式清单**（§7 七条）：可整体写进 `docs/development-standards.md` 的
    编码标准；其中"正交结果各报各的""dispose 抵达静默""擦洗 env"对
    `crates/tools`/`crates/runtime` 是立即可审计的检查项。
11. **信任栅栏**（落点：未来任何本地 HTTP 面 / GUI dev server）：loopback +
    Origin 三关、拒绝 0.0.0.0、特权方法钉 loopback、trustedHosts 载入期规范化断
    言。哪怕 Tauri IPC 为主，evidence/预览等本地 HTTP 端点也应套用。

### 9.2 不可迁移 / 需显式决策的分歧

1. **Cordis 式运行期插件树**：Node 专属的动态组合；Viden 保持 Rust 静态组合 +
   `plugin-api` 描述符路线。可抄的是**接缝纪律**（Definition/Provider/Consumer
   三段式、companion policy 插件、"换两个 adapter 搬整个执行世界"），不是机制。
   尤其注意 dsh 的"插件=可信代码、零隔离"立场与 Viden plugin-host 的边界意图相
   反——不要因参照它而放松 Viden 的插件信任模型。
2. **plan-mode 软引导**：与 Viden"plan 模式阻断一切变异路径"invariant 直接冲
   突。dsh 的论据（沙箱/审批旋钮独立执行、prompt 引导 + 工具表稳定保 KV cache）
   值得记录，但 Viden 应**维持硬阻断**——除非未来沙箱强度达到 dsh 的前提。折中可
   取处：`exit_plan_mode` 常驻工具表、仅执行期拒绝的做法可解决 Viden plan 模式
   下工具表抖动问题。
3. **审批只在升级点**：dsh 路线的前提是"内核沙箱是日常防线"。Viden 目前沙箱执行
   层尚未到位（权限是决策层），照抄会造成保护真空。方向应是 codex 式"决策强度与
   执行能力联动"，待沙箱落地后再考虑把逐调用审批收敛到升级点。
4. **无协议版本号 / client-host 同发**：仅在无独立发布客户端时成立。Viden 已冻结
   frontend schema 1 + 能力集握手，**保持现有版本化路线**，不要退回。
5. **明文凭据**：dsh 的 0600+启动权限位硬校验是低成本地板，可以补进 Viden；但目
   标形态仍应对照 codex（keyring 口令 + age 加密落盘）。
6. **token 计量 4 字符/token 启发式**：Viden `crates/context` 的 cost 引擎按
   provider 精确计量，不要退化。可借鉴的只有"provider 真实 usage 仅在请求信封匹
   配时复用"的防错配规则。
7. **浏览器第二棵插件树 / ui-slots**：Viden GUI 是 Tauri + CoreClient 契约，前端
   保持契约客户端而非插件宿主。ui-slots 的声明合并插槽可作为**远期** GUI 扩展点
   参考，当前不引入。
8. **网络管控缺位**：dsh 显式把网络排除出沙箱词表。Viden 该维度仍以 codex 四层
   栈为唯一参照。

### 9.3 新增候选方向（此前两份基准都未覆盖）

- **外部 CLI 委托作为一等能力面**：codex 无此形态、dsh 有完整参照。Viden 的
  agent adapter 已在契约里（`runtime.agent_adapters` 等扩展能力），建议按 §9.1-1
  升级为正式方向并补 parity fixture。
- **golden 会话日志（record/replay/refresh）**：两个参照里 dsh 的这套与 Viden
  的 JSONL-canonical 架构最契合，建议纳入 frontend-contract fixture 计划。
