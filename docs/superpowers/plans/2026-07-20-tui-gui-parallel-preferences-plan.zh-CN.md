# TUI 0.3 与 GUI 0.1 并发偏好系统实施计划

英文版：[2026-07-20-tui-gui-parallel-preferences-plan.md](2026-07-20-tui-gui-parallel-preferences-plan.md)

> **给 agentic workers：** 实施本计划时必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐任务执行。步骤使用 checkbox（`- [ ]`）语法追踪。

**目标：** 定义下一轮 Core、TUI、GUI 版本切片，让 TUI 和 GUI 基于同一个 Core checkpoint 并发开发，并共享多语言与外观偏好合同。

**架构：** Core 负责持久化 presentation preference、effective preference 解析和版本化事件。TUI、GUI 消费同一组 Core 事实，再在本地适配语言文案、skin token、density、motion 和颜色能力，不创建私有 palette 或私有 preference store。

**技术栈：** Rust workspace、`viden-core` frontend contract、`viden-types` DTO、Ratatui/Crossterm TUI、通过 framework gate 的 GUI、JSON/Serde fixtures、由 `docs/viden-design/Viden/tokens.css` 生成的 palette、双语 Markdown。

## 全局约束

- 设计审查顺序固定为：`docs/viden-design/Viden/index.html` -> 客户端设计稿索引 -> canonical prototype -> 组件库。
- TUI canonical prototype：`docs/viden-design/Viden/TUI/Viden - 统一原型 (TUI).html`。
- GUI canonical prototype：`docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html`。
- `tokens.css`、`i18n-dict.js`、`chrome.js`、`tui-kit.css`、`gui-kit.css` 和组件注册表是设计真源；归档页和孤立低层页面只作为补充证据。
- Core 版本线是 `core-v0.3.x`；TUI 版本线是 `tui-v0.3.x`；GUI 版本线是 `gui-v0.1.x`。
- 内置 locale 为 `en` 和 `zh-CN`；`system` 是解析输入，不是已渲染语言。
- 内置 skin 为 `aurora`、`ice`、`mono`、`amber`、`phosphor`；只有 `aurora`、`ice`、`mono` 支持 light mode。
- 有效外观组合为设计包定义的 8 组。无效值必须安全解析并给出可见诊断。
- 前端不得从 transcript 文本推断成功，也不得持久化第二套业务或偏好模型。

---

## 版本切片

| 集成门 | Core | TUI | GUI | 结果 |
| --- | --- | --- | --- | --- |
| P0 Contract | `core-v0.3.0` | `tui-v0.3.0-alpha.1` | `gui-v0.1.0-alpha.1` | `frontend-contract-v1` 包含 presentation preferences 与 fixtures。TUI/GUI 证明可消费 fixture。 |
| P1 Operable | `core-v0.3.1` | `tui-v0.3.0` | `gui-v0.1.0-beta.1` | TUI 统一 cockpit 与 GUI 桌面驾驶舱都暴露由 Core 支撑的语言/外观设置。 |
| P2 Local loop | `core-v0.3.2` | `tui-v0.3.1` | `gui-v0.1.0` | 一个本地任务在英文、简体中文和多外观变体下完成，并保持 Core/TUI/GUI 业务事实一致。 |

## 实施前要确认的设计主界面

| 轨道 | 主界面 | 辅助界面 | 确认重点 |
| --- | --- | --- | --- |
| TUI | `TUI/Viden - 统一原型 (TUI).html` | TUI 设计稿索引、TUI 组件库、T1/T1c/T1d/T3/T4 页面 | composer 语法、lane 布局、固定 approval、状态栏、终端色彩降级 |
| GUI | `GUI/Viden - 桌面驾驶舱 (GUI).html` | GUI 设计稿索引、GUI 组件库、D11、D4、D2、D6 页面 | 桌面 cockpit 外壳、项目接入、Lane 创建、permission/recovery、settings 入口 |
| Core/design | `index.html`、`Core/Viden - Aurora 主题 (Core).html` | `docs/SPEC.md`、`docs/DESIGN-REF.md`、`tokens.css` | 有效 skin、mode、density、语言切换、token ownership |

## 文件结构

| 路径 | 职责 |
| --- | --- |
| `crates/types/src/presentation.rs` | 共享 presentation preference DTO、枚举、effective values 和 validation errors。 |
| `crates/core/src/presentation.rs` | 偏好解析、持久化意图、Core events 和 snapshot 暴露。 |
| `crates/runtime/src/project.rs` 或现有 runtime 配置模块 | 通过 runtime commands 路由 project/user config 变更，不绕过 Plan mode。 |
| `docs/frontend-integration-contract.md` 和 `.zh-CN.md` | preference command、event、snapshot 与 frontend 义务的公开合同。 |
| `apps/tui/src/tui/i18n.rs` 和 `apps/tui/i18n/*.json` | TUI locale catalog、fallback、插值和 key parity 测试。 |
| `apps/tui/src/tui/preferences.rs` | TUI 对 Core preferences 的解析，以及终端 color-depth 适配。 |
| `apps/tui/src/tui/palette.rs` 或生成模块 | token-derived TUI palette，支持 truecolor、ANSI 256、ANSI 16 fallback。 |
| `apps/gui/**` | framework gate 之后的 GUI settings 入口、token adapter 和桌面 cockpit 集成。 |
| `apps/gui/release-manifest.toml` 和 `apps/tui/release-manifest.toml` | 独立版本元数据和所需 Core checkpoint。 |

---

### Task 1：Core Presentation Preference Contract

**文件：**
- 新建：`crates/types/src/presentation.rs`
- 修改：`crates/types/src/lib.rs`
- 修改：`crates/core/src/lib.rs`
- 修改：`crates/core/src/client.rs` 或当前 frontend contract 模块
- 修改：`docs/frontend-integration-contract.md`
- 修改：`docs/frontend-integration-contract.zh-CN.md`

**接口：**
- 产出：`UserPresentationPreferences`、`EffectivePresentationPreferences`、`PresentationPreferencePatch`、`PresentationPreferenceChanged`、`PresentationPreferenceError`。
- 消费：设计决策 `D-I18N`、`D-SKIN`、`D-SETTINGS`、`D-A11Y`，以及 `tokens.css` token 定义。

- [ ] **Step 1：写失败的 DTO 测试**

测试必须证明：

```rust
assert!(Skin::Aurora.supports(Mode::Light));
assert!(!Skin::Amber.supports(Mode::Light));
assert_eq!(Locale::resolve("system", Some("zh-CN")).id(), "zh-CN");
assert_eq!(Density::default(), Density::Compact);
```

- [ ] **Step 2：运行 RED 测试**

运行：`cargo test -p viden-types presentation -- --nocapture`

预期：失败，因为共享 preference contract 尚不存在。

- [ ] **Step 3：实现最小 typed contract**

定义 `LanguagePreference`、`LocaleId`、`Skin`、`ModePreference`、`EffectiveMode`、`Density`、`MotionPreference`、`TerminalColorCapability` 和 accessibility flags。未知外部值必须进入显式 invalid/fallback 路径，不能静默变成客户端直接使用的字符串。

- [ ] **Step 4：增加 Core commands 和 events**

增加读取 effective preferences 与应用 patch 的 command/event。Plan mode 必须在写配置前拒绝持久化 preference mutation，但允许只读 discovery。

- [ ] **Step 5：补双语合同文档**

更新两份 frontend integration contract，写明有效值、fallback 顺序、event 名称，以及前端不得建立私有 palette 的规则。

- [ ] **Step 6：验证并提交**

运行：

```bash
cargo test -p viden-types presentation
cargo test -p viden-core presentation
scripts/check-doc-pairs.sh docs/frontend-integration-contract.md docs/frontend-integration-contract.zh-CN.md
git diff --check
```

提交：`feat(core): define presentation preference contract`

### Task 2：语言与外观共享 Fixture 矩阵

**文件：**
- 新建或修改：现有 shared frontend fixture corpus
- 修改：`crates/core` fixture tests
- 修改：`docs/parallel-development-plan.md`
- 修改：`docs/parallel-development-plan.zh-CN.md`

**接口：**
- 消费：Task 1 preference DTO 和 Core event envelopes。
- 产出：覆盖 default、中文、light/dark、dark-only skin fallback、reduced motion、compact/regular/comfy density 和 terminal color fallback 的 fixtures。

- [ ] **Step 1：写失败的 fixture parity 测试**

增加以下 fixture 断言：

```text
en + aurora/dark + compact
zh-CN + aurora/dark + compact
en + ice/light + regular
zh-CN + mono/light + comfy
en + amber/light request -> amber/dark effective fallback
zh-CN + phosphor/light request -> phosphor/dark effective fallback
reduced motion
ansi16 terminal fallback
```

- [ ] **Step 2：运行 RED 测试**

运行：`cargo test -p viden-core frontend_preference_fixtures -- --nocapture`

预期：失败，直到 fixture 和 expected snapshots 建立。

- [ ] **Step 3：增加 fixture events 和 expected facts**

每个 fixture 必须包含 `schema_version`、preference source、effective values、fallback 发生时的 diagnostic，以及足够的 runtime facts，使 TUI/GUI 能证明它们渲染的是同一业务状态。

- [ ] **Step 4：验证并提交**

运行：

```bash
cargo test -p viden-core frontend_preference_fixtures
scripts/check-doc-pairs.sh docs/parallel-development-plan.md docs/parallel-development-plan.zh-CN.md
git diff --check
```

提交：`test(core): add presentation preference parity fixtures`

### Task 3：TUI 0.3.0 外观与语言切片

**文件：**
- 修改：`apps/tui/AGENTS.md`
- 修改：`apps/tui/release-manifest.toml`
- 新建或修改：`apps/tui/src/tui/i18n.rs`
- 新建或修改：`apps/tui/i18n/en.json`
- 新建或修改：`apps/tui/i18n/zh-CN.json`
- 新建或修改：`apps/tui/src/tui/preferences.rs`
- 新建或修改：`apps/tui/src/tui/palette.rs`
- 按需修改：TUI render/statusbar/composer/settings modules

**接口：**
- 消费：Core `EffectivePresentationPreferences` 和 shared fixtures。
- 产出：`tui-v0.3.0`，语言与外观设置由 Core 支撑。

- [ ] **Step 1：写失败的 TUI preference 测试**

测试必须证明 key parity、locale fallback、中文双宽布局稳定、truecolor/ANSI fallback 映射，以及拒绝私有 theme id。

- [ ] **Step 2：运行 RED 测试**

运行：`cargo test -p viden-tui preference i18n palette -- --nocapture`

预期：失败，直到 TUI 消费 Core preferences。

- [ ] **Step 3：实现 Core-backed TUI resolver**

TUI resolver 读取 Core effective values 并映射为终端样式。TUI 可以本地保存 terminal-only color capability detection，但 language、skin、mode、density、motion 的持久化必须回到 Core。

- [ ] **Step 4：更新用户可见控制**

通过 TUI overlay/settings 路径暴露语言与外观设置。label、approval 文案、status row 和窄屏 fallback 必须能以英文和简体中文渲染。

- [ ] **Step 5：验证并提交**

运行：

```bash
cargo test -p viden-tui
scripts/tui-regression.sh
scripts/tui-previews.sh
git diff --check
```

提交：`feat(tui): consume core presentation preferences`

### Task 4：GUI 0.1.0 外观与语言切片

**文件：**
- 修改：`apps/gui/AGENTS.md`
- 新建或修改：`apps/gui/release-manifest.toml`
- framework gate 后修改：GUI token adapter 和 settings modules
- 按需修改：GUI cockpit、D11 intake、D4 lane creation、D2 permission slice、D6 recovery surfaces
- framework selection 后修改：GUI screenshot/evidence scripts

**接口：**
- 消费：Core `EffectivePresentationPreferences`、GUI 组件库和桌面驾驶舱原型。
- 产出：`gui-v0.1.0-beta.1` / `gui-v0.1.0`，语言与外观由 settings 驱动。

- [ ] **Step 1：确认 GUI 入口路径**

按顺序打开：

```text
docs/viden-design/Viden/index.html
docs/viden-design/Viden/GUI/Viden - 设计稿索引 (GUI).html
docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html
docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html
```

实施前记录桌面驾驶舱、settings appearance 区、项目接入、Lane 创建、permission/decision 切片和 recovery state 截图。

- [ ] **Step 2：写失败的 GUI preference 测试**

测试必须证明 GUI 从 Core preferences 渲染、暴露语言与外观控制、拒绝无效 dark/light skin 组合，并且不交付第二套 palette registry。

- [ ] **Step 3：实现 GUI adapter**

从共享 tokens import 或生成。GUI 可以把 token 映射成框架原语，但有效 skin/mode/density/motion 值必须严格等于 Core contract。

- [ ] **Step 4：更新桌面驾驶舱路径**

桌面驾驶舱是 P0 入口。D11、D4、D2、D6 用于细化流程，但不能变成互相冲突的独立 shell。

- [ ] **Step 5：验证并提交**

运行所选 GUI 框架测试、screenshot parity 命令、CJK IME/manual evidence checklist、keyboard-only evidence，以及：

```bash
git diff --check
```

提交：`feat(gui): consume core presentation preferences`

### Task 5：集成与 Release Manifests

**文件：**
- 修改：`apps/tui/release-manifest.toml`
- 修改：`apps/gui/release-manifest.toml`
- 修改：Core release manifest 或 changelog 位置
- 修改：`docs/superpowers/plans/2026-07-19-independent-release-plan-index.md`
- 修改：`docs/superpowers/plans/2026-07-19-independent-release-plan-index.zh-CN.md`

**接口：**
- 消费：Task 1 到 Task 4 的提交。
- 产出：集成报告，明确 workspace candidate、Core version、TUI version、GUI version、Core checkpoint SHA、schema、capabilities 和 skipped gates。

- [ ] **Step 1：固定精确 Core checkpoint**

两个 frontend manifest 必须记录同一个 40 字符 Core checkpoint SHA 和支持的 schema versions。

- [ ] **Step 2：按固定顺序集成**

先跑 Core gates，再跑 TUI fixture/render gates，最后跑 GUI fixture/render gates。GUI 不能在 TUI 证据基于同一 Core checkpoint 通过前合入。

- [ ] **Step 3：验证 full workspace 或记录 scoped blocker**

运行：

```bash
cargo test --workspace --quiet
scripts/check-doc-pairs.sh
scripts/check-doc-links.sh
git diff --check
```

预期：通过，或记录明确 crate/script/owner 的 scoped blocker。

- [ ] **Step 4：提交集成元数据**

提交：`chore(release): pin independent frontend preference slice`

## 停止条件

当 Core、TUI、GUI 都有独立版本元数据，TUI/GUI 消费同一个 Core-owned presentation preference contract，英文与简体中文 key parity 被测试，所有有效 skin/mode/density 组合被 fixture 或 evidence 覆盖，并且集成报告写明两个前端使用的精确 Core checkpoint 时，本计划完成。
