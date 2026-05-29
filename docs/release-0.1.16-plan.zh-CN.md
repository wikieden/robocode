# RoboCode 0.1.16 计划 - TUI 交互可靠性

英文版： [release-0.1.16-plan.md](release-0.1.16-plan.md)

## 摘要

`0.1.16` 插在轻量 spec/steering workflow 之前，因为当前 cockpit 还有一些会直接影响
真实编程信心的交互债。这个版本的目标是：

> 用户提交输入后，RoboCode 必须始终说明现在发生了什么，终端必须保持响应，并且所有
> 看得见的动作都必须真的能执行。

这是 operator loop 可靠性版本，不是视觉重做。它沿用当前无弹窗态视觉主题，重点修真实
使用中不可靠的地方：pending provider work、焦点、输入、命令提示、审批弹窗控制、鼠标
命中、resize 重绘，以及 footer 动作是否真实。

## 产品目标

- 让远程/provider 工作在运行中可见。
- 长 provider/tool turn 期间 TUI 仍能持续重绘。
- 显式建模 focus，让键盘和鼠标动作路由可预测。
- 每个展示出来的 footer action 都有真实行为。
- 降低 modal 摩擦：approval 要键盘优先、支持鼠标，决策后立即消失。
- 保持中文/IME 输入稳定，并在常见终端里有可见光标。
- 在进入 spec 或更大编排能力前，补上确定性和人工交互测试。

## P0 范围

### 1. 非阻塞 Provider Turn 外壳

问题：
Provider request 目前同步跑在主 TUI 路径里。请求开始前能显示 `pending_turn`，但
provider 阻塞时 elapsed time、`NOW WORKING`、取消入口和类似 streaming 的状态不能持续
重绘。

需求：

- 把 provider turn 放到 event/channel 边界之后，让 TUI event loop 能继续 tick 和 redraw。
- 主屏中部显示 working-state 行/卡片：
  - 当前动作，例如 `Thinking`、`Calling tool`、`Waiting for approval`、`Running shell`。
  - elapsed time。
  - provider/model。
  - 已知时显示 active task 或 lane id。
  - 下一步安全动作，例如 `wait`、`approve`、`deny`、`inspect` 或 `cancel`。
- worker 中的 permission approval 必须桥回 UI，不能绕过现有 permission path。
- worker state 写入共享 runtime snapshot，而不是新增 TUI-only status store。
- 如果真实 cancellation 在本版本风险太高，就不要把取消显示成已可用动作。

验收：

- 长 fallback/provider turn 不会冻结 clock/status repaint。
- approval request 仍要求同一套权限决策。
- turn 活跃时，主 cockpit 始终能回答“现在到底在干嘛”。

### 2. Interaction Router 与 Focus 模型

问题：
输入处理目前散落在 composer、command palette、approval、lane、side screen 和 modal
路径里。结果就是某些快捷键只在部分状态可用，行为不稳定。

需求：

- 引入显式 `TuiFocus`：
  - `composer`
  - `command_palette`
  - `approval`
  - `lane_detail`
  - `right_rail`
  - `side_screen`
- 键盘和鼠标事件都经过统一 interaction dispatcher。
- 保留当前 composer 快捷键：
  - `Enter` / `Ctrl-J`：发送。
  - `Ctrl-K`：清空。
  - `Ctrl-R`：重新载入最近一次用户输入。
  - `Ctrl-N`：开始 `/task add ...`。
  - `?`：仅在 composer 为空时打开帮助。
- 对不明显的状态转移，在代码里加简短注释说明。

验收：

- 单元测试覆盖各 focus target 的状态切换和快捷键行为。
- footer action 只在当前 focus 下有效时显示。

### 3. Command Palette 一致性

问题：
slash command palette 已经改善，但长列表和鼠标行为还不完整。

需求：

- 对超过可见行数的命令提示增加 scroll window。
- 键盘移动时保持 selected item 可见。
- 鼠标左键点击可见 suggestion 时支持选中/补全。
- palette 不得覆盖 composer 或终端 IME 区域。
- 命令描述在窄终端里要足够短。

验收：

- 有一张确定性截图证明 command palette 长列表滚动行为。
- 有 visible-window math 和 mouse hit testing 单元测试。

### 4. Approval Modal 控制

问题：
审批弹窗是信任核心，但当前 modal 会让人觉得“卡住”：焦点不清楚，`Diff` 收益不足，鼠标
和键盘控制也不完整。

需求：

- 默认选中动作保持 `Approve`。
- 键盘：
  - `y`：通过。
  - `n`：拒绝。
  - `d`：打开或聚焦真实 diff/evidence view。
  - `Tab` / 方向键：移动 action focus。
  - `Esc`：只有安全时关闭；否则说明必须先决策。
- 鼠标：
  - 左键点击 `Approve`、`Deny`、`Diff` 和 checkbox 区域。
  - 最终决策后立即移除 modal。
- Diff：
  - 要么显示真实 inline diff/evidence view，要么移除这个假 affordance。

验收：

- 有 default-approve、diff/evidence、post-action cleared state 三类 modal 截图证据。
- 测试覆盖键盘和鼠标 action mapping。

### 5. Resize、光标与 IME 稳定性

问题：
cockpit 信息密度高。边框错位、残留重绘、看不到光标、输入法候选框离输入区太远，都会立刻
降低信心。

需求：

- 增加 deterministic rapid-resize regression，覆盖：
  - idle main screen。
  - command palette。
  - approval modal。
  - active provider turn。
  - 中文输入。
- 输入区高度必须足够可读，并改善 IME 位置。
- 当终端不可靠地支持 `SetCursorStyle::BlinkingBar` 时，提供高对比 caret fallback 或
  app-owned pulse。
- right rail 和 side screens 边框颜色保持一致。
- 避免 semantic highlighter 把普通单词内部字母染色。

验收：

- macOS Terminal 和 iTerm2 人工截图证明输入/光标位置可读。
- regression artifacts 证明 resize 后边框仍对齐。

### 6. Footer Promise Audit

问题：
看起来能点、看起来有快捷键，但实际没行为的 footer action，会让整个 TUI 显得像假功能。

需求：

- 审计 main、side-1、side-2、lane detail、command palette 和 approval 状态下的所有
  footer label。
- 每个 action 必须实现、隐藏，或明确标记为不可用并说明原因。
- 为所有已实现的全局快捷键补测试。

验收：

- 文档和测试列出支持的 controls。
- 没有已知的“看得见但不能用”的 footer action。

## P1 范围

- right-rail tasks、recent files、diagnostics、provider health rows 的鼠标选择。
- transcript、palette 和 side panels 的鼠标滚轮。
- provider 支持时显示真实 streaming。
- 底层操作支持时，提供 app-level cancel/interrupt。
- 多屏操作的 focus breadcrumb。

## 明确不做

- 本版本不做轻量 spec/steering workflow；该能力顺延到 `0.1.17`。
- 不增加新的外部 Agent adapter 广度。
- 不引入 plugin marketplace 或 mutating MCP/ACP runtime。
- 不重做无弹窗态视觉主题；视觉变化只服务于交互清晰度。

## 测试计划

聚焦自动化检查：

- `cargo test -p robocode-cli tui::app --quiet`
- `cargo test -p robocode-cli tui::command_palette --quiet`
- `cargo test -p robocode-cli tui::terminal --quiet`
- 新增 focus-router 测试，覆盖 key 和 mouse routing。
- 新增 provider-worker 测试，覆盖 pending、approval、completion、failure events。
- 新增 palette scroll-window 测试。

回归检查：

- `cargo fmt --check`
- `cargo clippy -p robocode-cli --all-targets -- -D warnings`
- `cargo test -p robocode-cli --quiet -- --test-threads=1`
- `cargo test --workspace --quiet`
- `scripts/tui-regression.sh docs/previews/generated`
- 新增或扩展 interaction smoke，覆盖 rapid resize、pending turn repaint、modal action
  mapping 和 command-palette mouse behavior。

人工验收：

- macOS Terminal 和 iTerm2：
  - 普通输入。
  - 中文/IME 输入。
  - command palette 鼠标和键盘。
  - approval modal 鼠标和键盘。
  - turn 活跃时 resize。
  - side-1 / side-2 视觉一致性。
- 每个用户可见验收状态完成前，都要给真实截图。

## Release Evidence

必须生成截图或确定性视觉产物：

- `0.1.16-tui-working-state`
- `0.1.16-tui-command-palette-scroll`
- `0.1.16-tui-approval-default-approve`
- `0.1.16-tui-approval-diff`
- `0.1.16-tui-cjk-caret`
- `0.1.16-tui-resize-active-turn`
- `0.1.16-tui-side-rail-consistency`

## 文档更新

- 实现后更新 README controls。
- 更新 user guide controls 和 troubleshooting。
- 更新 TUI cockpit design，写清 focus 和 mouse 规则。
- 更新 staged / long-term roadmap。
- 本地 RC 形成后新增 `release-0.1.16-status`。

## 风险

- provider turn 从主 TUI 路径移出是最高风险项，因为 approval decision 对 tool loop 来说
  仍必须保持同步语义。
- 真实 cancellation 可能需要更深的 provider/tool runtime 改造；如果本版本做不到，就不要
  把它显示成可用动作。
- IME 候选框位置部分依赖终端行为；RoboCode 可以改善 composer 几何和 caret 位置，但不能
  完全控制所有终端的原生输入法 UI。
