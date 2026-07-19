# Core、TUI、GUI 独立版本列车设计

英文版：[2026-07-19-independent-core-tui-gui-release-train-design.md](2026-07-19-independent-core-tui-gui-release-train-design.md)

## 1. 目标

Viden 后续按 Core、TUI、GUI 三条独立 SemVer 版本线开发和发布。Core 根据 TUI、GUI 的产品需求以及自身的可靠性、安全和演进需求，先定义兼容合同与不可变 checkpoint；TUI、GUI 再基于该合同并发开发。三条线不要求版本号对齐，也不要求每次同步发布，但每个前端版本都必须声明并验证其 Core 兼容范围。

本轮交付目标是一个可运行的本地操作闭环：

- Core 提供 `frontend-contract-v1`、多 Lane runtime、CoreClient、恢复语义和共享验证样本。
- TUI 完成薄客户端迁移、稳定交互和统一原型主 Cockpit。
- GUI 完成框架门禁，并把桌面驾驶舱的 P0 主路径做成可运行产品。
- 英文和简体中文、多皮肤、明暗模式与密度从第一轮就作为系统能力建模；配置入口按版本逐步开放，不以散落常量实现。

本轮不包含完整 P1 Beta、团队协作、Fleet、远程目标和插件贡献 UI。

## 2. 已选方案

### 2.1 三条独立版本线

采用“独立发版、协同集成门”：

- Core、TUI、GUI 各自维护 SemVer 和变更日志。
- Core 可以为合同或 runtime 能力独立发版。
- TUI、GUI 可以按各自需求独立发版。
- TUI、GUI 的每个构建必须记录：最低 Core 版本、支持的 wire schema、所基于的 Core checkpoint SHA、所需 capabilities。
- 阶段性集成基线统一验收，但不强制三条线同时发版。

没有采用统一产品版本号，因为它会把 UI 发布节奏绑在 Core 内部演进上；也没有采用完全松散的三线开发，因为缺少兼容矩阵和 checkpoint 会使前端只能靠猜测合同工作。

### 2.2 三种版本标识彼此分离

必须区分：

1. 组件 SemVer，例如 Core `0.3.0`、TUI `0.2.0`、GUI `0.1.0`。
2. wire/schema 版本，例如 `frontend-contract-v1`、`schema_version = 1`。
3. 不可变实现 checkpoint，即精确 Git SHA。

客户端先做 capability discovery，不根据 SemVer 字符串猜测命令或事件是否存在。breaking wire 变更必须提升 schema major，并同时提供 migration、legacy fixtures 和三端 parity evidence。

## 3. 设计真源与检查顺序

所有 TUI、GUI 需求分析、开发验收和视觉审查按以下顺序进行：

1. `docs/viden-design/Viden/index.html`：全局设计入口和 Core/TUI/GUI 分组。
2. `TUI/Viden - 设计稿索引 (TUI).html` 与 `GUI/Viden - 设计稿索引 (GUI).html`：确定屏幕层级、状态和 roadmap 边界。
3. `TUI/Viden - 组件库 (TUI).html` 与 `GUI/Viden - 组件库 (GUI).html`：确定可复用组件和交互词汇。
4. TUI 以 `TUI/Viden - 统一原型 (TUI).html` 为主产品原型。
5. GUI 以 `GUI/Viden - 桌面驾驶舱 (GUI).html` 为主入口和视觉母版。

`tokens.css` 是数值与视觉 token 单一真源；`tui-kit.css` 和 `gui-kit.css` 分别是组件真源。`reference-shots/` 只用于快速比对，不覆盖活体 HTML、组件库或设计登记。

## 4. 本轮独立版本定义

| 集成基线 | Core | TUI | GUI | 共同结果 |
| --- | --- | --- | --- | --- |
| `I0 · Contract` | `0.3.0` | `0.2.0-alpha.1` | `0.1.0-alpha.1` | 冻结 `frontend-contract-v1`；两端可重放同一 fixture；GUI 完成框架闸，TUI 完成 client spike |
| `I1 · Operable` | `0.3.1` | `0.2.0` | `0.1.0-beta.1` | Core 多 Lane 与副作用上收；TUI 主 Cockpit 可用；GUI 桌面驾驶舱主壳和 D11/D4/D1/permission/recovery 垂直路径可用 |
| `I2 · Local Loop` | `0.3.2` | `0.2.1` | `0.1.0` | request → work → test/review → evidence → gate → apply/recovery 的本地闭环，三端事实一致 |

这些版本号是本轮计划的目标线。某条线因修复产生 patch 版本时，不需要推动另外两条线改号；只更新兼容矩阵与集成基线记录。

## 5. Core 版本包

### Core 0.3.0：前端合同 v1

- 为 command、event、snapshot 增加版本化 envelope、stream/session identity、cursor 和 capabilities。
- 定义 sequence、duplicate、out-of-order、gap、snapshot、replay 和 reconnect 语义。
- 提供 transport-neutral `CoreClient`，前端不创建或修改 `SessionEngine`。
- 把 task、lane、role、route、gate strength、mutation policy、target、budget 等字段改为 typed contract。
- 丰富 approval：risk、target、scope、policy reason、expiry、default action、stable audit id。
- 定义分页/流式 transcript rows 与稳定 scroll anchor。
- 提供 migration 和共享 parity corpus。

### Core 0.3.1：多 Lane runtime

- 将 worktree、terminal/tmux/PTY、accept/apply、conflict recovery 等权威副作用上收 Core。
- 实现 lane/session/task keyed supervisor；一条 Lane 的等待、审批、取消或错误不阻塞其他 Lane。
- 为 queue、cancel、approval、error 和 command receipt 增加明确 owner。
- 提供项目探测、配置预览/确认、provider/model health、credential handle 和 Lane 生命周期命令。

### Core 0.3.2：本地可信闭环

- 完成 evidence、MergeGate、apply/recovery、history/replay 和 append-only audit 的 P0 语义。
- 提供 `handoff`、`review_request`、`contract`、`dependency` 的最小本地合同，复杂跨团队编排延后。
- 用真实本地任务验证结构化事实链，不允许前端从 transcript 文案推断成功。

## 6. TUI 版本包

### TUI 0.2.0-alpha.1：CoreClient 验证

- 从 Core 0.3.0 checkpoint 创建分支。
- 重放共享 fixtures，并证明可归约出与 Core 相同的 `RuntimeViewState`。
- 不做生产视觉迁移，不新增 TUI 私有业务事实。

### TUI 0.2.0：统一原型主 Cockpit

- TUI 只发送 `RuntimeCommand`、消费 `RuntimeEvent`、渲染 `RuntimeViewState`。
- 移除对 engine、provider、permission store、Git、process 和 Lane 权威副作用的直接调用。
- 落实 welcome → `/setup` 或 `/lanes` → Cockpit 的主路径。
- 落实 Normal / Insert / Overlay、`Esc` 逐层退出、`Ctrl-C` 仅中断当前工作。
- 支持多行 composer、内部滚动、bracketed paste、CJK 双宽和独立 scrollback。
- 保留 0.1.30 的 zero-bug 回归基线。

### TUI 0.2.1：本地监督闭环

- 完成 lane/session 切换、selector-first、全局 jump、approval、task/DAG、evidence、MergeGate、context/cost 和 recovery action。
- Changes、Evidence、Context 是核心详情面；Inbox/Fleet 只显示本轮可支持的摘要。
- 所有成功状态由 Core 事实事件确认。

## 7. GUI 版本包

### GUI 0.1.0-alpha.1：框架门禁

- Tauri 与 GPUI 使用同一 Core fixture 和同一 D1 垂直切片比较。
- 比较输入延迟、event-to-visible、frame work、10,000 events、50,000 transcript rows、CJK IME、键盘、可访问性、三平台、签名/更新/凭据/崩溃恢复和长期维护成本。
- 任一 IME、可访问性、三平台、有界 transcript 或长期 fork 门失败，选择 Tauri。

### GUI 0.1.0-beta.1：桌面驾驶舱 P0

- GUI 主入口是 `Viden - 桌面驾驶舱 (GUI).html` 对应的 D1 驾驶舱。
- 无项目时进入 D11，完成项目探测、模式选择、`viden.toml` 预览/确认和首批 Lane 创建，再进入 D1。
- 从 D1 接入 D4 Lane 创建、Conversation、Activity Rail、Environment、composer、stream/tool cards、permission dock 和 D6 recovery states。
- D2 在本轮只承载完成本地闭环所需的最小 decision/permission slice；完整 Decision Center 属于下一轮 P1。
- GUI 只依赖 CoreClient 和前端中立合同。

### GUI 0.1.0：本地操作闭环

- 完成 project → lane → session → task 的导航和恢复。
- 完成 diff/test/evidence、MergeGate、apply/recovery、history/replay 的 P0 表面。
- 完成一次可审计真实本地开发任务。
- 达到视觉、CJK、键盘、可访问性和性能门，暂不要求团队/Fleet/远程能力。

## 8. 多语言系统

### 8.1 责任边界

- Core 只发结构化事实、稳定 message key、参数和错误码，不发需要前端解析的英文句子。
- TUI、GUI 各自渲染本地化文本，但共享 locale id、fallback 规则和 key parity 测试。
- 业务日志保留原始事实；切换语言不重写 transcript、event log 或 audit log。

### 8.2 本轮支持

- 内置 `en` 和 `zh-CN`。
- 启动优先级：显式 CLI/配置 → 已保存用户偏好 → 系统 locale → `en`。
- fallback：请求 locale → 同语言默认变体 → `en` → 可见 key；禁止静默空白。
- 时间、数字、token、成本、快捷键和路径按语义格式化，代码、命令、标识符不翻译。
- TUI/GUI 的同一 Core 事实使用同一 key 与参数集合。

### 8.3 配置开放策略

- I0 定义 `ui.locale` 配置 schema、解析和持久化，不承诺完整设置界面。
- I1 在两端提供设计中已有的 EN/中快速切换，并持久化用户偏好。
- I2 在 Settings 中开放稳定配置项，并保留 CLI override。
- 新 locale 必须通过 key completeness、layout/CJK、snapshot 和 fallback 测试后登记。

## 9. 皮肤、配色和密度系统

### 9.1 配置模型

共享 UI 偏好包含：

- `skin`: `aurora | ice | mono | amber | phosphor`
- `mode`: `dark | light | system`
- `density`: `compact | regular | comfy`
- `motion`: `system | reduced | full`
- TUI-only `color_depth`: `auto | truecolor | ansi256 | ansi16`

有效皮肤组合为：Aurora、Ice、Mono 支持 dark/light；Amber、Phosphor 仅支持 dark，共 8 种。无效组合必须给出可见原因并安全回退，不能产生半套 token。

### 9.2 单一真源与适配

- `tokens.css` 保持设计数值真源。
- 构建期从登记 token 生成 TUI palette 和 GUI framework adapter；生成物可校验，不手工复制颜色。
- `tui-kit.css`、`gui-kit.css` 和组件登记决定组件语义；颜色只表达状态和层级。
- GUI 禁 emoji；TUI 使用单一 glyph registry，并支持 truecolor → 256 → 16 降级。
- reduced-motion、对比度、可见焦点和不可只靠颜色表达状态是所有皮肤的硬门。

### 9.3 配置开放策略

- I0 冻结 `ui.skin/ui.mode/ui.density/ui.motion` schema 和 token registry。
- I1 保留原型中的皮肤、明暗和密度快速切换，并实现持久化。
- I2 在 Settings 中开放完整选项、系统模式和恢复默认值。
- 后续插件只能通过登记 descriptor 提供皮肤，不得注入任意 CSS 或覆盖核心状态色。

## 10. 配置优先级与数据流

配置优先级为：

```text
CLI override
  → user preferences
  → project-safe defaults
  → client capability defaults
```

项目配置不能强制改变个人语言、皮肤、明暗、密度或 reduced-motion。Core 可以返回推荐值和 capability，但最终 UI 偏好属于本地用户状态。两端切换 UI 偏好时不产生业务 mutation；如需审计，只记录配置 key 变化，不记录颜色值或翻译文本。

## 11. 兼容矩阵与集成流程

每个 TUI/GUI release manifest 至少记录：

```text
component_version
min_core_version
supported_schema_versions
base_core_checkpoint
required_capabilities
design_source_revision
locale_catalog_revision
token_registry_revision
```

开发顺序：

1. Core 从同步 main 创建版本分支并发布不可变 checkpoint。
2. TUI、GUI 从同一 checkpoint 建独立 worktree 并发开发。
3. Core 后续变更保持向后兼容；breaking change 进入下一 schema。
4. 集成固定按 Core → TUI → GUI 串行验证。
5. 三端共享 fixture、migration 和设计基线全部通过后签发集成基线。

写范围继续限制为 Core `crates/**`、TUI `apps/tui/**`、GUI `apps/gui/**`；缺失合同必须提交 Core contract request，不能在客户端私建事实。

## 12. 错误与恢复

- schema 不兼容：启动前阻止连接，显示客户端版本、Core 版本、schema 和升级建议。
- capability 缺失：禁用对应动作并显示缺失能力，不渲染可点击但必失败的控件。
- sequence gap：请求 snapshot/replay；恢复前不宣布成功。
- locale key 缺失：按 fallback 链显示，并在测试/诊断中记录 key。
- 皮肤组合无效或 token 不完整：回退 Aurora dark/regular，并显示一次非阻塞诊断。
- 配置损坏：保留原文件、使用安全默认值，并给出可定位错误；禁止静默覆盖用户配置。

## 13. 验收与测试

### Core

- schema、capability、cursor/replay/gap、unknown event、migration 和多 Lane 非阻塞测试。
- Plan mode 在任何 mutation 前拒绝。
- JSONL replay 重建相同业务事实。
- Core 不依赖 UI crate。

### TUI

- 共享 fixture replay、输入模式、CJK、paste、scrollback、resize、approval 和多 Lane 测试。
- 80/112/160 列与 truecolor/256/16 截图。
- 两种 locale、8 种有效皮肤组合、3 种密度和 reduced-motion 关键快照。
- `apps/tui/**` 不包含权威 runtime/Git/process/Lane 副作用。

### GUI

- 框架闸指标、共享 fixture replay、D1 主路径、D11/D4/permission/recovery、crash/reconnect。
- 英文/中文、CJK IME、键盘、screen reader、可见焦点和三平台。
- component gallery 与接受的 HTML 主界面截图比对。
- 8 种有效皮肤组合、3 种密度、system/reduced-motion 和无效组合回退测试。

### 集成

- 同一 fixture 在 Core、TUI、GUI 得到相同业务事实。
- 同一项目/Lane/Session 可在 TUI 和 GUI 中恢复。
- 一条真实本地开发任务完成完整 P0 闭环。
- `cargo test --workspace --quiet` 通过。

## 14. 下一轮而非本轮

- 完整 Decision Center、D10 Lane Monitor、D12 conflict bounce、D14 audit 的 P1 深度体验。
- 可信交付 Beta、三平台正式打包和发布门。
- D13 Fleet、D7/D8 团队能力、D9 远程目标。
- plugin/domain UI contributions、自定义 locale 包和第三方皮肤 descriptor。

这些能力必须基于本轮稳定合同另立版本，不得反向扩大本轮 P0 范围。
