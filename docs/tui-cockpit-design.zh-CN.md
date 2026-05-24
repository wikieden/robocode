# RoboCode TUI Cockpit 设计

本文记录当前 TUI 目标，避免开发偏离前期生成的主视觉和终端 agent 工作流。

## 视觉基线

- 主视觉状态：**无弹窗态**。所有弹窗必须继承同一套 aurora-cyan cockpit
  主题，不再出现另一套配色。
- 主参考图：
  `docs/previews/tui-concept-holodeck-v1.png`。
- 布局目标：高密度终端 cockpit，不是介绍页。首屏应立即服务于编码、审查、
  审批和子 agent lane 监控。
- 配色方向：深蓝黑底、青色边框、绿色成功、黄色注意/权限、红色拒绝/错误。
  避免大块异常黑带或终端默认背景泄漏。

## 主屏幕

- 顶栏：产品、provider、model、session、context、Git 分支、权限模式、
  token/context 状态。
- Transcript：左侧主面板，时间线式消息，最近内容固定留在底部可见。
- 右侧栏：workspace、active tasks、diagnostics、provider health、recent files。
- Composer：始终在底部可见，输入光标位于输入行内，带 action hints 和
  approval-mode chips。
- 底部状态栏：连接状态、session、token、cost/time、theme/help 提示。

## 命令提示列表

当输入是单个 slash 前缀 token，比如 `/` 或 `/p`，命令提示列表显示在
composer 上方。

键盘契约：

- `Up` / `Down`：移动选中命令。
- `Tab`：把选中命令补全到 composer。
- `Enter`：补全部分命令；精确命令则提交。
- `Esc`：关闭当前 query 的提示列表。继续编辑 query 后重新打开。
- `/exit`、`/quit`、`exit`、`quit`：退出 TUI。

渲染契约：

- 提示列表使用与主 TUI 一致的 cockpit 边框、标题和行样式。
- 浮在 composer 正上方，不能遮挡输入光标。
- 展示命令、说明和选中行标记。

## 审批弹窗

审批弹窗是可交互 overlay，不是被动 transcript 卡片。

- `Tab` / `Shift-Tab` 和方向键在 apply-all、deny、diff、approve 间移动焦点。
- 默认焦点在 `Approve`，所以常见场景直接按 `Enter` 即可通过。
- `Enter` 执行当前焦点控件。
- checkbox 获得焦点时，`Space` 切换 apply-all。
- `y` 批准，`n` / `Esc` / `Ctrl-C` 拒绝。
- 鼠标点击可聚焦控件；在 deny 或 approve 上释放鼠标会完成审批。
- 批准或拒绝后，pending 弹窗必须立即消失，transcript 和右栏不能留下样式残影。

## 多屏方向

TUI 支持一个主屏幕，最多两个副屏幕：

- 主屏幕：transcript、审批、命令输入、高层状态。
- 副屏 1：子 agent / terminal lane 监控。
- 副屏 2：诊断、构建状态、文件和 ops 上下文。

核心需求不是“好看”，而是监督多个终端编程工具，例如 Codex、Claude Code、
shell job、DeepSeek lane。副屏需要暴露任务状态、最新输出、产物、进度和路由
提示，让主 agent 能判断后续动作。

## 当前实现备注

- 主屏和副屏都会响应 resize 事件并重绘。
- 行级 diff 渲染避免输入时整屏闪烁。
- composer 已按显示宽度处理中文等 CJK 输入。
- slash 提示列表是本地 UI 状态，不触发模型调用。

## 近期缺口

- 真正 PTY-backed lanes 仍由 lane snapshot 和 log tail 表示。
- 命令提示列表目前是固定命令注册表；命令参数和二级提示待后续扩展。
- 视觉还需要持续用截图和 holodeck 主参考图对比。
