# Viden 0.1.11 计划

英文版： [release-0.1.11-plan.md](release-0.1.11-plan.md)

最后更新：2026-05-27

## 版本定位

`0.1.11` 是 TUI Cockpit Reliability + Orchestration Foundation 版本。

这个版本不把目标定成完整 `0.2.0` 多 Agent 编排系统，而是先完成两个基础条件：

- TUI cockpit 在真实终端里稳定、可信、可长时间使用。
- 多 Agent 编排和 token 效能优化需要的核心状态模型开始落地。

`0.2.0` 再作为 Agent Orchestration Runtime v1。

## 核心目标

### 1. 真实 TUI 使用可靠性

修复并验证用户已经遇到的关键交互问题：

- 窗口 resize、拖动、缩放后自动重绘，不残留旧边框和错位区域。
- 右侧面板边框、颜色、标题、内容不漂移。
- 输入区高度更舒适，光标明显并具备闪烁提示。
- 中文输入法候选窗尽量靠近输入行。
- 无弹窗态、approval 弹窗、命令提示列表使用同一主题风格。
- approval 弹窗默认聚焦通过，支持快捷键和鼠标点击，决策后立即关闭。
- `/quit` 和 `/exit` 稳定退出。

### 2. 主屏 Now Working 状态

主屏中央必须清楚显示 Viden 现在到底在做什么，而不是让用户猜远程是否还在工作。

状态来源统一进入 `AgentTask` 投影：

- provider 正在 thinking / streaming
- tool call 等待审批
- shell/test 正在运行
- 外部 lane 正在执行
- 最近一次失败、阻塞或需要用户决策的动作

显示内容至少包括：

- 当前动作
- 负责 agent/provider/lane
- elapsed time
- 证据来源
- 下一步可操作项

### 3. AgentTask / AgentLane 地基

为 `0.2.0` 多 Agent 编排做数据模型准备。

本版本需要完成：

- 统一 `AgentTask` lifecycle：pending、thinking、running、reviewing、blocked、completed、failed、cancelled。
- 引入或补齐 `AgentLane` 概念：main、side-1、side-2、shell、codex、claude、deepseek。
- 为每个 task/lane 记录 evidence：transcript event、tool call、diff、test、artifact、last output。
- 右栏、副屏、lane detail、Now Working 读取同一状态投影。

### 4. Token 效能基础设计

本版本不实现完整 token optimizer，但要把接口和可见性打出来。

必须设计并落文档：

- `ContextBundle` 的字段：task、selected files、diff、diagnostics、test results、facts、lane summaries。
- tool output compaction 策略：长日志 tail、重复输出去重、失败摘要优先。
- per-agent token budget 概念。
- cost/context pressure 在 TUI 中的展示位置。

如实现代码，优先只做最小可用：

- 当前 turn context summary。
- token/context pressure 更可信的显示。
- 长 tool output 的压缩展示，不影响 transcript 原始审计。

### 5. 真实截图验收

延续 0.1.10 规则：每个用户可见功能点完成后，都要给真实使用截图或确定性 TUI 视觉产物。

至少保留这些截图：

- idle cockpit
- live provider thinking
- Now Working active
- approval modal
- command palette
- resize 后主屏
- side-1 lanes
- side-2 ops
- lane detail
- 中文输入场景

## 非目标

- 不在 0.1.11 宣称完整 ACP host。
- 不在 0.1.11 做完整 MCP mutation runtime。
- 不把 plugin/skill 做成完整 marketplace。
- 不把 Web/Desktop/IDE 作为主入口。
- 不为了视觉效果增加没有真实状态支撑的面板。

## 验收标准

- 用户在真实终端里连续输入、审批、resize、打开命令提示、运行 provider turn，不出现明显错位和残留。
- 主屏中央可以持续显示当前工作状态，并且状态来自真实 `AgentTask` / `AgentLane`。
- 右栏、副屏和 Now Working 对同一个任务状态没有互相矛盾的信息。
- approval 弹窗可以用默认 Enter/快捷键/鼠标完成确认，并在确认后消失。
- token/context 展示不再只是装饰值，至少能说明当前 context 来源和压力。
- 文档明确 `0.2.0` 将承接多 Agent 编排 runtime，而 0.1.11 是基础版本。

## 验证

至少运行：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.11 --deepseek --out-dir /tmp/viden-0111-release-smoke-full
```

人工验证：

- macOS Terminal / iTerm2 至少各跑一次 TUI。
- 手动 resize、拖动窗口、全屏/退出全屏。
- 中文输入法输入一轮短中文 prompt。
- approval modal 通过键盘和鼠标各确认一次。
- DeepSeek provider 完成一次小型写文件 + 运行命令任务。

## 后续承接

`0.1.11` 完成后，进入 `0.2.0`：

- Agent Orchestration Runtime v1
- 默认 planner -> worker -> reviewer -> tester workflow
- Codex / Claude Code / DeepSeek / shell lane 的正式编排
- ContextBundle builder 和 token efficiency engine v1
