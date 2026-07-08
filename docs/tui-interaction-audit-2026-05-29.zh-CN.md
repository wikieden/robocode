# TUI 交互审计 - 2026-05-29

English version: [tui-interaction-audit-2026-05-29.md](tui-interaction-audit-2026-05-29.md)

## 范围

这次审计不只看截图好不好看，而是看真实操作者会卡在哪里：输入、命令提示、鼠标、弹窗、重绘稳定性，以及“现在到底在干嘛”的信号。

## 发现

### 本轮已修

- Main TUI 启用了 mouse capture，但 approval loop 之外的 `Event::Mouse` 都被丢弃。现在 slash command palette 支持鼠标左键点击可见提示行，按下选中，松开补全。
- Composer 底部显示了 `Ctrl-K`、`Ctrl-R`、`Ctrl-N` 和 `?`，但主事件循环以前没有实现。现在这些快捷键有真实行为：
  - `Ctrl-K`：清空输入区。
  - `Ctrl-R`：把最近一次用户输入放回输入区，便于重新生成。
  - `Ctrl-N`：开始 `/task add ...`。
  - `?`：输入区为空时打开帮助。
- `Ctrl-J` 现在会作为显式 send key，和 footer action 对齐。
- 语义高亮里原来用单字母 `E` / `W` 做 metric span，容易把普通单词逐字染色。这个过宽的单字母匹配已移除，panel label 和边框颜色会更稳定。

### 仍有风险

- 鼠标支持仍然偏窄。目前覆盖 approval 和 command suggestions，但还不覆盖 right rail task selection、lane modal controls、transcript links 或 side-screen panels。
- 光标闪烁仍依赖终端自己的 `SetCursorStyle::BlinkingBar`。如果终端忽略该设置，还没有 app 自己控制的 blink pulse 或高对比 caret fallback。
- Resize 已经按 size change 全量重绘，但还缺“快速连续改尺寸 + 输入 + 弹窗”的压力测试。

### 0.1.16 RC 已更新

- Provider turn 已移到 worker/channel 边界后面。provider worker 运行时，主 event
  loop 会继续刷新 elapsed time、pending state、lane snapshot 和 approval prompt。
- Approval prompt 会桥接回 UI，并继续走原来的 permission path 处理。
- Command suggestion 长列表现在有可见滚动窗口，会保持选中行可见，鼠标 hit
  testing 也会映射到滚动窗口后的真实 suggestion index。
- Approval `Diff` 焦点现在会显示 prompt 携带的真实 evidence / preview lines。

### 剩余交互 backlog

- 真正取消仍是 best-effort。UI 可以请求取消，但已经发出的 provider request
  可能会在 worker 看到取消前正常返回。
- Provider token streaming 仍是后续能力；`0.1.16` 解决的是 turn 运行期间 UI
  不冻结，不是完整 streaming renderer。
- 鼠标覆盖应继续扩到右栏任务选择、lane modal controls、副屏滚动、transcript links
  和 wheel events。
- 光标闪烁和 IME 位置仍部分依赖终端行为。如果更多终端不渲染原生 blinking bar，
  Viden 需要补一个 app-owned 高对比 caret fallback。

## 建议下一片

这一片现在正式收敛为 `0.1.16`：TUI Interaction Reliability。它应该排在轻量
spec/steering 之前，因为更大的 workflow surface 会放大这些交互问题，而不是掩盖它们。

1. 把 provider turn 移到 background worker channel，让 TUI 能持续重绘 `NOW WORKING`、elapsed time、取消入口和 streaming 状态。
2. 增加统一 interaction router，明确 focus target：`composer`、`palette`、`approval`、`lane-detail`、`right-rail`、`side-screen`。
3. 把 mouse hit testing 从 command palette 扩到 right rail、lane controls 和 approval diff。
4. 增加 terminal-interaction regression：快速 resize、鼠标选择、快捷键、pending-turn repaint cadence。
5. 清理 footer action：显示出来的动作必须可执行；做不到就先去掉，不要像假功能。

详细版本需求见 [Viden 0.1.16 计划](release-0.1.16-plan.zh-CN.md)。
