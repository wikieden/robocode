# 原生 Agent 与 ACP Lane 闭环设计

英文版：[2026-07-22-native-acp-lane-flow-design.md](2026-07-22-native-acp-lane-flow-design.md)

## 状态

下一轮统一里程碑已确认采用本交互方向：

- Core `0.3.4`；
- TUI `0.3.3`；
- GUI `0.1.0-rc.2`。

本文定义产品与合同目标，不代表当前候选版本已经实现这些行为。

## 目标

让 TUI 和 GUI 都能完成最短且真实的 Viden 工作流：

1. 打开 Git 项目；
2. 创建一个由 Viden 原生主 Agent 驱动的 Lane；
3. 通过 Core 向该 Agent 发送首个任务；
4. 按需委派一个或多个 ACP Agent；
5. 在任一前端观察、审批、取消和恢复工作。

Viden 原生 Agent 由 Core 当前选择的 DeepSeek 或 OpenAI provider 驱动。Codex、Claude、
Kiro 和自定义 ACP server 是外部委派 Agent，不是 model provider，也不是 Lane 状态的另一套
权威来源。

## 产品模型

- 一个 Lane 恰好拥有一个 Viden 原生主 Agent runtime。
- 主 Agent 使用 Core 统一提供的 provider、model、tool、permission、session、transcript、
  task 和 evidence 服务。
- ACP Agent 必须作为委派子会话归属于现有 Lane。
- 一个 Lane 可以拥有多个 ACP 子会话。
- Lane 创建与 Agent 启动是两个独立结果。Core 一旦给出 `StarterLaneCreated` receipt，
  后续原生或 ACP 启动失败都不能回滚或隐藏 Lane 成功状态。
- Core 仍是唯一业务状态权威；前端只保留 draft、焦点、菜单状态等展示状态。

## 统一 Core 流程

### 项目资格预检

Core 在开放创建入口前发布：当前绑定 workspace 是否为有有效 `HEAD` 的 Git 仓库。
不符合条件时返回 typed reason，并保持零 starter-Lane mutation。前端必须在创建入口直接展示
原因，不能等到流程后段才给出笼统失败。

### 原生 Lane

规范顺序为：

1. 前端发送已复核的 starter-Lane 请求；
2. Core 发布 preview facts；
3. 需要时由用户批准 branch/worktree mutation；
4. Core 发布精确 Lane receipt；
5. 前端立即聚焦已确认 Lane；
6. 用户提交第一条任务，启动该 Lane 的 Viden 原生主 Agent；
7. Core 持续发布 task、turn、tool、approval、transcript、usage 和 evidence facts。

Provider 和 model 是 Core 管理的项目/会话选择。普通 Lane 创建直接继承当前选择；切换入口位于
composer 或 settings，不进入新建菜单。

### ACP 委派

ACP 只能从现有 Lane 发起。规范顺序为：

1. 查询 Core 的 typed ACP adapter 列表；
2. 选择 adapter；
3. 必要时探测安装与 Agent 自有认证；
4. 输入委派任务；
5. 启动 owner-scoped ACP session；
6. 流式展示状态、消息、工具请求、审批、结果和证据；
7. 每个子会话独立完成、失败或取消。

ACP 失败不得改变已确认 Lane receipt，也不得终止 Viden 原生主 Agent。Core 必须发布真实的
可启动状态；测试不得注入生产 probe 永远无法产生的 `auth_state=ready` 来制造通过。

## GUI 交互

正常创建路径移除 D4 四步向导。`+` 按钮只打开一个类似 Zed 的紧凑菜单：

```text
NEW LANE
  Viden Agent

DELEGATE TO CURRENT LANE
  Codex
  Claude
  Kiro
  Custom ACP...
```

规则：

- `Viden Agent` 使用 Core 默认值自动生成 Lane id、branch 和 worktree。
- 只有 mutation 需要确认时，紧凑菜单才切换成小型 Core preview/approval 状态。
- 收到 Lane receipt 后，GUI 立即打开该 Lane composer；用户第一条消息就是原生 Agent 初始任务。
- Provider/model 保留在 composer footer 和 settings。
- 未选择 Lane 时，委派入口禁用。
- 选择 ACP 后只打开委派任务输入；安装、认证或启动错误归属该子会话。
- branch、worktree、budget、policy 等高级选项移到 Lane settings，不阻塞默认路径。

GUI 主路径因此只有：

`+ -> Viden Agent -> 输入任务`。

委派路径为：

`选择 Lane -> + -> ACP Agent -> 输入委派任务`。

## TUI 交互

TUI 使用常规终端交互，不复制 GUI 菜单。

- `n` 或现有新建 Lane 命令创建默认 Viden 原生 Lane，并聚焦 composer。
- ACP 委派加入系统命令列表，命令为 `/acp`。
- 选择 `/acp` 后打开由 Core adapter facts 驱动、可用键盘操作的 ACP Agent 列表。
- 选择 adapter 后，TUI 请求委派任务并通过统一 Core command 启动子会话。
- 没有活动 Lane 时，`/acp` 明确禁用并显示原因。
- 方向键移动、Enter 确认、Escape 取消；窄终端下仍可完成全流程。
- Provider/model 保持为状态栏、settings 或 slash command 选项，不增加新建 Lane 步骤。

TUI 主路径为：

`新建 Lane -> 输入任务`。

委派路径为：

`/acp -> 选择 Agent -> 输入委派任务`。

## 错误与恢复语义

- 非 Git 或缺少 HEAD：preview 前阻塞，并给出直接说明。
- branch/worktree 冲突：保留创建入口，允许重新生成默认值或显式重试。
- provider 不可用或未认证：保留 Lane，显示原生 Agent 启动失败与 settings 动作。
- ACP 未安装：分类为需要安装。
- ACP 未登录：分类为需要认证，并显示 Agent 自有登录指引。
- ACP probe 或 session 失败：保留子会话记录，允许重试或取消。
- event gap 或重连：通过 snapshot/replay 恢复后才重新开放 mutation。

所有错误都必须来自 typed Core facts。前端不得解析显示字符串来推断成功、owner、认证或可重试性。

## 合同与所有权影响

Core 负责：

- Git workspace 资格和自动生成的 Lane 默认值；
- Lane preview、approval、receipt 与原生主 Agent task lifecycle；
- 活动 provider/model 与 provider health；
- ACP discovery、probe、startability、session lifecycle、cancel 和 evidence；
- snapshot/replay 恢复与 owner 关系。

TUI 只负责系统命令发现、`/acp` 选择展示、draft 和焦点。GUI 只负责紧凑 `+` 菜单、委派任务
展示、draft 和焦点。两端都不得持久化第二套 adapter registry、recent list、provider choice、
Lane record 或 ACP session record。

## 验证

统一 fixture corpus 必须覆盖：

1. 有效 Git 项目 -> Lane receipt -> 原生 DeepSeek 任务；
2. 有效 Git 项目 -> Lane receipt -> 原生 OpenAI 任务；
3. 非 Git 与缺少 HEAD 的预检拒绝；
4. Lane 成功后 provider 启动失败；
5. Codex、Claude、Kiro discovery/probe 分类；
6. 从配置发现自定义 ACP；
7. ACP 成功、需要认证、需要安装、失败与取消；
8. 一个 Lane 下多个并发 ACP 子会话；
9. event gap 与 snapshot/replay 恢复；
10. TUI、GUI 消费同一 Core facts 的 parity。

测试必须包含真实 LocalCoreHost/runtime integration。只靠 fixture 注入生产不存在的 readiness
状态不够。发布证据至少包含一次真实 DeepSeek 原生 turn 和一次真实 ACP session；只有发布门禁
明确提供 OpenAI 凭据时，才要求 OpenAI live execution。

## 不在范围内

- 一个 Lane 内多个 Viden 原生主 Agent。
- 没有归属 Lane 的 ACP session。
- 在新建 Lane 菜单内选择 provider/model。
- 创建前强制配置自定义 branch/worktree/budget。
- 前端自建 Agent registry、认证状态或 session persistence。

## 验收

用户能在同一 Core build 上分别用 TUI 和 GUI 完成原生与 ACP 两条路径，并且每个可见成功都有
有序 Core receipt 或 lifecycle fact 支撑时，本里程碑完成。后续原生或 ACP 启动失败时，Lane
创建仍必须保持明确成功。
