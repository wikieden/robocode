# Viden 设计接入决策

英文版： [viden-design-adoption.md](viden-design-adoption.md)

最后更新：2026-06-26

## 决策

Viden 是当前有效的产品方向。旧的 RoboCode 产品框架视为 legacy implementation plan，
不再作为产品、TUI 或 GUI 决策依据。

这是产品和设计决策，不等于立即重命名 Rust crates、binary、package artifacts、
transcript path 或兼容命令。这些名称属于实现迁移工作，需要单独的 rename plan。

## 接受的设计源

接受的设计源是：

- `docs/viden-design/Viden/CLAUDE.md`
- `docs/viden-design/Viden/docs/DESIGN-REF.md`
- `docs/viden-design/Viden/tokens.css`
- `docs/viden-design/Viden/TUI/tui-kit.css`
- `docs/viden-design/Viden/screenshots/`
- `docs/viden-design/Viden/Core/`
- `docs/viden-design/Viden/TUI/`
- `docs/viden-design/Viden/GUI/`

旧的 `docs/design/canvas-export` 导入已删除，不能继续作为设计源。

## 产品映射

| 旧术语 | Viden 方向 |
| --- | --- |
| RoboCode product | Viden product |
| RoboCode cockpit | Viden cockpit |
| RoboCode TUI / GUI | Viden TUI / GUI |
| RoboCode visual identity | Viden Aurora identity |
| Generated canvas export | Reviewed Viden design source |

crates、binary、config path、release artifacts 等实现名称可以先保持现状，直到迁移计划
覆盖 backward compatibility、Homebrew、GitHub releases、config migration 和 user data
migration。

## 目标屏

主要 TUI 目标：

- `docs/viden-design/Viden/screenshots/cockpit-final.png`
- `docs/viden-design/Viden/screenshots/welcome-watcher.png`
- `docs/viden-design/Viden/screenshots/lane-monitor-wide.png`

主要 GUI 目标：

- `docs/viden-design/Viden/screenshots/d1v2.png`
- `docs/viden-design/Viden/screenshots/s13.png`

这些图片定义视觉方向和信息架构，不等于单独构成像素级实现合同。实现仍必须通过组件、token、
截图和 runtime-state 验收 gate。

## 实现规则

- TUI 和 GUI 必须消费共享 runtime facts：`RuntimeSnapshot`、event stream、task、lane、
  approval、provider health、context、cost 和 evidence。
- UI 不得虚构 runtime 无法 replay 的业务状态。
- 新 UI 工作必须优先使用 Viden token 源和组件词汇，再考虑新增样式。
- 当设计源和当前实现冲突时，产品方向以 Viden 设计源为准；兼容性问题在迁移计划完成前以当前
  实现为准。
- 用户可见设计和规划文档的产品名应使用 Viden。RoboCode 只应出现在 legacy implementation
  name 或 migration compatibility 语境中。

## 待迁移工作

1. 决定是否以及何时重命名 binary、crates、config directories、release artifacts 和
   Homebrew formula。
2. 定义现有 `robocode` 命令和 `.robocode` 用户数据的兼容策略。
3. 把活跃 PRD、roadmap、TUI 和 GUI 文档从 RoboCode 框架迁移到 Viden 框架。
4. 从已接受的 Viden 目标图建立 screenshot baselines。
5. 增加 release gate：当 UI 截图偏离 Viden 目标且没有记录 deviation 时失败。
