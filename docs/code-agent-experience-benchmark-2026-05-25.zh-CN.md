# Code Agent 编程体验对标 - 2026-05-25

这份聚焦版对标比较 Codex、Claude Code、DeepSeek-TUI / CodeWhale 和 Zed
当前的编程体验，并把更大的 `docs/code-agent-benchmark.md` 收敛成 Viden
`0.1.5` 可执行的改进方向。

2026-05-25 检查的来源：

- [OpenAI Codex overview](https://platform.openai.com/docs/codex/overview)
  和 [OpenAI Codex product page](https://openai.com/codex/)
- [Claude Code overview](https://code.claude.com/docs/en/overview)、
  [Claude Code subagents](https://code.claude.com/docs/en/sub-agents) 和
  [Claude Code hooks](https://code.claude.com/docs/en/agent-sdk/hooks)
- [DeepSeek-TUI site](https://deepseek-tui.com/en) 和
  [DeepSeek-TUI GitHub repository](https://github.com/Hmbown/DeepSeek-TUI)
- [Zed AI overview](https://zed.dev/docs/ai/overview)、
  [Zed Agent Panel](https://zed.dev/docs/ai/agent-panel)、
  [Zed Parallel Agents](https://zed.dev/docs/ai/parallel-agents.html) 和
  [Zed External Agents](https://zed.dev/docs/ai/external-agents.html)

## 定位判断

Viden 不应该试图在每个产品的主场击败对方：

- Codex 的强项是 OpenAI 背书下跨 cloud、desktop、terminal 的高信任任务执行。
- Claude Code 的强项是成熟的 terminal 编程闭环，以及 permissions、hooks、
  subagents、MCP 和企业工作流。
- DeepSeek-TUI / CodeWhale 的强项是高密度 terminal-native DeepSeek V4 体验，
  包括 mode switching、长上下文预期、成本/provider 感知和快速 TUI 迭代。
- Zed 的强项是 editor-native agent 体验：agent panel、thread sidebar、
  inline code context、parallel threads、external agents 和 worktree isolation。

Viden 最强的路线应该更窄、更锋利：

> 一个 local-first terminal cockpit，让主 agent 监督代码修改、approval、测试、
> diagnostics，以及 `codex`、`claude`、shell job、DeepSeek-backed lane 等外部编程工具。

## 体验对比

| 产品 | 体验强项 | Viden 应该借鉴 | 不应直接照搬 |
| --- | --- | --- | --- |
| Codex | 委派任务完成、隔离环境、集中 diff/review 预期、多端连续性 | 把 diff/test evidence 作为一等结果；任务运行保持隔离和可 review；降低安装/发布摩擦 | `0.1.5` 不以 cloud delegation 为中心；Viden 近期优势是本地 TUI 监督 |
| Claude Code | terminal-native action loop、保守权限、hooks、subagents、MCP、清晰工作流自动化 | 打磨 approve/test/fix 闭环；approval 稳定后增加轻量 permission profiles 和 hooks；subagent 状态必须可见 | 不隐藏 invisible subagents；核心编程闭环稳定前不扩张 MCP |
| DeepSeek-TUI / CodeWhale | 高密度 terminal UI、DeepSeek V4 family 聚焦、长上下文/成本/provider 感知、快速 TUI 迭代 | 保持 DeepSeek V4 Flash 作为真实 smoke 目标；provider health 和 context pressure 可见；使用紧凑 side panels | 不追逐所有视觉花活；不增加没有真实状态支撑的 panel |
| Zed | editor-native agent panel、threads、parallel agents、ACP external agents、inline context selection、worktree isolation | 让 lanes 像 terminal threads；副屏展示 agent lanes、tests、diagnostics 和 next actions；external agents 可配置 | 不重做完整 editor；Viden 应该集成 editor，而不是替代 editor |

## Viden 0.1.5 产品差距

目前首要差距不是模型能力，而是操作者信心：

- 用户必须始终知道输入焦点在哪里。
- 用户必须看到什么在等待、改了什么、测了什么、什么可以安全批准。
- 外部 agents 必须是有 state、logs、changed files、next action 的受监督 lane，
  不能只是 opaque subprocess。
- 副屏必须承载工作证据，而不是装饰性 dashboard。

## 建议的 0.1.5 改进

### 1. 先稳定手感最直接的编程表面

先交付这些，再扩展更大的 workflow：

- 更高的 composer 和可见闪烁光标；
- 中文输入法候选窗靠近输入行；
- resize-safe layout，不出现旧边框或 right rail 漂移；
- 无弹窗态和弹窗态主题颜色统一；
- approval modal 默认 focus 在 approve，支持快捷键、鼠标目标、决策后立即消失，
  并显示当前 focus action。

原因：Codex、Claude Code、DeepSeek-TUI 和 Zed 都会让用户明确感知当前交互点。
Viden 只要输入光标、modal 或 panel 对齐不确定，用户信任就会掉。

### 2. 把 diff 和 test evidence 放到闭环中心

增加紧凑的 coding evidence model：

- changed files 按 added/modified/deleted 展示；
- approval 前和 mutation 后显示短 diff summary；
- `/test` 或 `/run test` 保存 command、exit code、duration、tail、失败文件和建议下一步；
- right rail 和 `/status` 都显示最新 diff/test state。

原因：Codex 和 Zed 让用户预期代码改动可以集中 review；Claude Code 让用户预期工具能在
一个闭环里修改并验证。Viden 应该让“改了什么、测过没有”无法被忽略。

### 3. 把 lane 从日志升级成受监督的工作线程

每个 lane 要渲染：

- lane id、tool、command/template、workspace/worktree、pid/session、status、elapsed time；
- ANSI/OSC/prompt 噪声清理后的最后有意义输出；
- changed files 和 artifacts；
- last test result 或 verification result；
- 推荐下一步：inspect、send、attach、test、accept、revise 或 cleanup。

原因：Zed 的 parallel threads 和 external agents 是最接近的心智模型。
Viden 可以提供 terminal 版本：少一点 editor-native，多一点 operations cockpit。

### 4. 做最小 permission automation，不做权限大爆炸

`0.1.5` 使用小型 profile layer：

- 默认允许低风险 read-only 命令；
- 可配置自动允许 test commands；
- file writes、deletes、network calls、workspace 外 shell commands 默认确认，
  除非用户选择更强模式；
- 每次 permission decision 都写入 transcript。

原因：Claude Code 的 permissions 和 hooks 有价值，是因为它们减少打断但不隐藏风险。
Viden 应该借鉴这个原则，而不是立刻复制完整表面积。

### 5. 保持 Zed 式显式上下文，但做成 terminal-native

增加 `/context`，作为可见 context bundle：

- current task；
- selected dirty files；
- recent diagnostics；
- latest test result；
- relevant lanes；
- provider/model/context pressure；
- 下一次 model turn 会发送的明确文件列表。

原因：Zed 允许用户把 editor selection 和 thread context 加入 agent。Viden 需要
terminal 等价物，让用户能判断 agent 到底看到了什么。

## 0.1.5 执行顺序

1. Composer、cursor、IME、resize 和 modal focus。
2. Slash command palette，以及 approval modal 的鼠标/键盘处理。
3. Diff/test evidence model 和 `/status` 集成。
4. Lane inspect 页面和 side-screen lane/test/diagnostic panels。
5. Light permission profiles 和 `/context`。

不要把 MCP、remote automation 或完整 terminal emulator 塞进 `0.1.5`，除非它们直接
阻塞上面某个条目。

## 验收证据

这个版本不能只靠截图判断，必须证明编程闭环真的可用：

- fallback-provider TUI smoke：编辑文件、approval、显示 diff、运行 test、退出；
- DeepSeek V4 Flash TUI smoke：完成一个小型 coding task，包含 tool calls 和 verification；
- shell lane smoke：create、inspect、capture output、cleanup；
- tmux/external lane smoke：attach command、捕获有意义输出、inspect lane state；
- no-modal、modal、command palette、lane inspect、side-1、side-2、compact、normal、
  wide layouts 的 snapshot/previews。

