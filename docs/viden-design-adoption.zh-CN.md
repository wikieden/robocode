# Viden 设计接入与视觉真源索引

英文版：[viden-design-adoption.md](viden-design-adoption.md)

最后更新：2026-07-19

## 决策

`docs/viden-design/Viden/` 下的最新设计目录，是 Viden 产品视觉与交互行为的接受真源。
生效中的产品、TUI 和 GUI 文档必须从这里派生，不能各自保留另一套视觉目标。

这项决策本身不负责重命名 Rust crates、binary、配置路径、release artifacts 或兼容命令；
这些仍属于迁移工作。

## 真源优先级

发生冲突时按以下顺序处理：

1. `docs/viden-design/Viden/docs/SPEC.md` 与
   `docs/viden-design/Viden/docs/screens-status.js` 定义已接受决策、开放问题、roadmap
   状态和屏幕注册表。
2. `docs/viden-design/Viden/tokens.css` 定义视觉数值。
3. 活体原型和 `tui-kit.css` / `gui-kit.css` 定义当前布局、组件、状态与交互行为。
4. `docs/viden-design/reference-shots/` 是这些活体真源的评审快照。
5. 功能设计和路线图负责把设计翻译成开发需求。
6. 生成式 preview 和 release screenshot 只属于实现证据或历史证据。

参考图与活体原型冲突时，以活体原型为准；功能文档与 `SPEC.md` 冲突时，以
`SPEC.md` 为准。

已经删除的 `docs/design/canvas-export`、`docs/viden-design/Viden/screenshots/` 下的旧文件，
以及 `docs/previews/`，都不能继续作为当前视觉目标。

## TUI 目标映射

| 用途 | 权威源 | 评审快照 |
| --- | --- | --- |
| 统一驾驶舱与 Welcome | `docs/viden-design/Viden/TUI/Viden - 统一原型 (TUI).html` | `docs/viden-design/reference-shots/TUI-统一原型驾驶舱.png` |
| 可复用组件与状态 | `docs/viden-design/Viden/TUI/Viden - 组件库 (TUI).html` 和 `docs/viden-design/Viden/TUI/tui-kit.css` | `docs/viden-design/reference-shots/TUI-组件库.png` |
| 输入、焦点、overlay 与审批行为 | `docs/viden-design/Viden/TUI/pages/Viden - T4 交互规则 (TUI).html` | 统一原型和组件库快照结合评审 |
| 屏幕清单 | `docs/viden-design/Viden/TUI.html` 和 `docs/viden-design/Viden/docs/screens-status.js` | 不设置独立目标图 |

交互契约是 Normal / Insert / Overlay 三态，键盘优先、鼠标可选；`Esc` 逐层退出，
`Ctrl-C` 只负责打断活动工作。四档审批闸使用 `1`–`4`、方向键和 `Enter`，`Esc` 或超时
安全拒绝。

## GUI 目标映射

| 用途 | 权威源 | 评审快照 |
| --- | --- | --- |
| D1 桌面驾驶舱外壳 | `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html` | `docs/viden-design/reference-shots/GUI-D1-桌面驾驶舱.png` |
| GUI 组件词汇 | `docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html` 和 `docs/viden-design/Viden/GUI/gui-kit.css` | `docs/viden-design/reference-shots/GUI-KIT-组件库.png` |
| D2 决策中心 | `docs/viden-design/Viden/GUI/pages/Viden - D2 决策中心 (GUI).html` | `docs/viden-design/reference-shots/GUI-D2-决策中心.png` |
| D4 Lane 创建 | `docs/viden-design/Viden/GUI/pages/Viden - D4 Lane创建流程 (GUI).html` | `docs/viden-design/reference-shots/GUI-D4-Lane创建流程.png` |
| D10 Lane 监视器 | `docs/viden-design/Viden/GUI/pages/Viden - D10 Lane监视器 (GUI).html` | `docs/viden-design/reference-shots/GUI-D10-Lane监视器.png` |
| D11 首启与接入 | `docs/viden-design/Viden/GUI/pages/Viden - D11 首启与项目接入 (GUI).html` | `docs/viden-design/reference-shots/GUI-D11-首启与项目接入.png` |
| D12 冲突退回 | `docs/viden-design/Viden/GUI/pages/Viden - D12 集成闸冲突退回 (GUI).html` | `docs/viden-design/reference-shots/GUI-D12-集成闸冲突退回.png` |
| D13 Fleet 与 Workflow | `docs/viden-design/Viden/GUI/pages/Viden - D13 Fleet 编排与 Workflow (GUI).html` | `docs/viden-design/reference-shots/GUI-D13-Fleet编排.png` |
| D14 审计时间线 | `docs/viden-design/Viden/GUI/pages/Viden - D14 审计与时间线 (GUI).html` | `docs/viden-design/reference-shots/GUI-D14-审计与时间线.png` |
| D5 画廊和 D6 系统态 | `docs/viden-design/Viden/GUI/pages/` 下对应文件 | 对应的 `GUI-D5-*` 与 `GUI-D6-*` 快照 |

D7、D8、D9 是 roadmap 屏；D2h、D3 和 Pip 是概念或装饰性扩展。设计产物存在，不等于自动
进入首发需求；具体状态以 `screens-status.js` 和 `SPEC.md` 为准。

D1 是 GUI 外壳：固定 activity rail、浮动或固定 lane rail、中央工作区、
Environment/context rail，以及按需出现的 dock 或 inspector。Permission 是执行前的 inline
dock；D2 负责异步 gate 与 review 决策，D12 负责 merge conflict recovery，D14 负责
append-only audit trail。Evidence 是与审计记录关联的产物，不等同于 audit log。

## 实现规则

- TUI 与 GUI 消费同一套 Core facts、commands、events、snapshots、replay、tasks、lanes、
  permissions、context、cost、evidence 和 audit identity。
- 前端不能虚构业务状态，也不能创建第二条执行路径。
- 视觉数值来自 `tokens.css`；新增局部样式前，先使用已经登记的组件词汇。
- 当前实现 preview 可以与目标快照做回归比对，但不能因此升级成新的设计源。
- 历史 release 文档要保留原有证据含义，不能把历史截图替换成当前目标图。
- 接受视觉偏差时必须记录影响源、原因、owner 和后续 gate。

## 治理

修改设计目录时，需要同步相关活体源；必要时同时更新 `DESIGN-REF.md`、`SPEC.md`、
`screens-status.js`、token baseline、设计检查和设计 changelog。消费侧文档应链接回本索引，
不要再复制另一套视觉基线。
