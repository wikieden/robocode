# Viden — 项目说明（CLAUDE.md）

> 本文件是 **Viden** 的项目说明 + 文档体系 + 工作纪律,随每个会话自动加载。
> 底座来自 design-spec-kit;呈现层是 Viden 自己的「Aurora」终端/桌面皮肤。

## 快速检索（冷启动地图 · 找组件/样式/token/图标先看这里）
> 任何 agent 进来,先按此表定位真源再 grep,**别另造索引**(再造 = 漂移)。深细节走 `docs/DESIGN-REF.md`。

| 要找 | 唯一真源 | 怎么取 |
|---|---|---|
| **token**(色/字号/间距/圆角/阴影/密度) | `tokens.css` | grep `--accent`;皮肤段看 `[data-skin][data-mode]` |
| **组件**(类名 + 最小 HTML) | `docs/DESIGN-REF.md`「组件目录」→ `GUI/gui-kit.css` · `TUI/tui-kit.css` | grep 类名 `.wslane` / `.vterm` |
| **GUI 图标** | `GUI/gui-icons.jsx`(`ICONS` / `GuiIcon` / `AgentLogo`) | grep key `chat` / `lock` |
| **TUI 字形** | `docs/DESIGN-REF.md`「TUI 字形词表」 | grep 字形 / 语义 |
| **屏 / 页**(画到哪 · 文件路径) | `docs/screens-status.js`(机读) | grep id / file |
| **设计决策 / 护栏** | `docs/SPEC.md` | grep `@DECISION` / `@OPEN` / `@GREP` |
> 路由:`CLAUDE.md`(本文件·入口) → `docs/DESIGN-REF.md`(深目录) → grep 真源。非 Claude 工具入口见根 `AGENTS.md`(指针,不复制内容)。

## 产品
**Viden** 是一个 **AI agent 编排开发工具**:用 lane/会话编排多个智能体协作写代码,带工具门控(gate)与人审。
- **Core** —— 核心功能与视觉设计真源(Rust);品牌、Aurora 主题、协作机制图都在这里。
- **TUI** —— 命令行版本(终端驾驶舱,等宽字符栅格)。
- **GUI** —— 桌面客户端版本(Rust + Tauri,多栏窗口)。

技术栈:核心 **Rust**;GUI **Rust + Tauri**。UI 设计阶段,以 HTML 高保真稿沉淀规范。
`0.1.0-alpha.1` framework gate 已用同一 Core fixture 的 Tauri/GPUI spike 证据选择
**Tauri** 作为唯一 production baseline;GPUI 仅保留为对比 spike。该选择不豁免 Tauri
后续真实 IME/可访问性、三平台、性能、交付与恢复门禁。

核心场景:
- **会话主屏 / 多 lane**:在一个驾驶舱里编排多个 agent,看转录、实时工作(LIVE WORK)、活动任务。
- **工具门控与审批**:危险工具调用停在 gate 上等人审(approval),金色 = 「需要人」。
- **Lane 监视 / 集成闸**:并行 lane 进度、冲突退回、画廊评审。

## 设计基调
- 气质:**克制 · 工程/终端感**(cockpit 驾驶舱),信息承载优先,几何精确、不花哨。
- 模式:**深色为主**,Light 为可选变体;并提供多套配色皮肤(换肤)。
- 排版:UI/正文 **Inter + Noto Sans SC**;代码/终端 **JetBrains Mono**;终端态全程等宽对齐。
- 主题色:**多套可切换**,每套含 背景6档 / 前景4档 / 强调(青 `--accent` 主 + 金 `--gold` 注意)/ 语义4色(success/warning/error/progress)/ 边框 / 页面 chrome。
  - 主色 **青(robot logo 同源)**= 品牌 + 唯一交互焦点(边框激活/标题/选中行/focus)。
  - 次强调 **金**= viden 字标 + 工作模式 + 「需要人」(门控/待审批)。
  - 5 套皮肤 × 明暗两轴:`data-skin`=`aurora`(青·默认)·`ice`(蓝)·`mono`(灰)·`amber`(琥珀)·`phosphor`(绿);`data-mode`=`dark`|`light`。aurora/ice/mono 成对 dark+light;amber/phosphor 复古终端族仅 dark。
- 平台与密度:**桌面 · 高密度**(cockpit 多栏 + 滚动 ticker);密度可调 `data-density`=`compact|regular|comfy`。

## 设计 Token（单一真源）
所有 token 定义在根目录 **`tokens.css`**,是**唯一真源**。页面用 `<link rel="stylesheet" href="tokens.css">`(子目录用相对路径,如 `../tokens.css`),根元素设 `data-skin` / `data-mode` / `data-density`。
- **切勿凭空发明颜色 / 字号 / 间距**,一律引用 `var(--*)`。裸 `#hex` / `rgba()` 由 `check-tokens.js` 拦截。
- 间距 4px 基准(`--sp-*`),圆角 `--r-*`(TUI 方硬 3px → GUI 圆润 14px),阴影 `--shadow-*`,字体 `--font-mono`/`--font-sans`,字阶 `--fs-*`。
- 换肤机制:所有颜色 token 随 `data-skin`×`data-mode` 两轴重定义,组件只用语义 `var(--*)` —— 换 skin/mode 即换肤,组件零改动。密度随 `data-density`。

## 翻译成 app（Rust + Tauri · 防漂移）
> Tauri 前端 = webview = HTML/CSS/JS,所以设计层**直接共享、不要手抄翻译**(手抄必漂)。按「能否共享」分三层:
- **① 原样共享(零漂移)** —— `tokens.css` + `GUI/gui-kit.css` + `brand-assets/*.svg`。app 前端**直接 import**,组件渲染**同一套类名 + DOM 结构**(`.frame/.act/.wslane/.envp/.gperm`…);改 token 两边同步。视觉真源 = `GUI/Viden - 桌面驾驶舱 (GUI)`(D1)。
- **② 脚本派生(单向)** —— 原生侧(托盘/菜单/窗口 chrome)要色值时,用生成器 `tokens.css → tokens.json/.rs`,**禁手抄**;`.css` 永远是源,产物可重生成、可 gitignore。
- **③ 原生重写(本不共享)** —— React+Babel 运行时转译、`chrome.js` 换肤器、窗口管理器、`tweaks-panel`、mock 数据 = **原型脚手架**,app 用正经构建/框架实现。**注意:视觉不在这层,全在 ① 的共享 CSS 里。**
- **对齐更新**:① 的文件作单一真源,被 app vendore(git submodule / 共享包),`git pull` 即同步;**别在 app 留手改副本**(留副本 = 漂移源)。

## 交付物与文档
- **Core/「Viden - Aurora 主题」** —— 给人看的配色/层级/组件展示页(含主题切换)。
- `docs/DESIGN-REF.md` —— **AI / 开发速查手册**:token 全表 + 组件目录(类名 + 最小 HTML)。**复用组件前先读它。**
- `docs/SPEC.md` —— **设计决策护栏机读真源**(grep 锚点 @DECISION / @OPEN):动设计前查已定稿护栏 + 开放问题索引。
- `docs/CHANGELOG.md` —— 更新日志(按天 + 模块标签)。
- `docs/CHECKLIST.md` —— 收尾自查清单(开工前 / 写码时 / done 前 DoD)。
- `docs/screens-status.js` —— **屏幕状态唯一真源**(机读):每屏 id/state/kind/file/备注。门户 `index.html` 运行时直读渲染卡片 + 进度。**加/删/改屏只改这里**(见 DoD)。
- `docs/PROTO-STANDARD.md` —— **平台原型结构规范(TUI 为样板)**:目录布局 + 单一真源 + 入口/窗口约定 + **新平台(GUI)改造 7 步流程**。做新平台或大改造前先读它。
- 项目文档统一收纳在 `docs/`(CLAUDE.md 因需置于根目录而保留在根)。

## 约定
- 新设计 / 组件一律遵循上述 token 与基调,**保持克制**(少即是多,避免无意义的数字、图标、渐变堆砌)。
- 语言:界面中英双语(`i18n.js` 走 `data-zh` 切换);CJK 正文行高 1.55–1.7,等宽态中文宽字符占两格。
- TUI 细则:对齐到等宽单元格;无圆角/无阴影(TUI 内),边框用 box-drawing;每个色给 ANSI 256 近似,高亮行保留青色左竖条不丢焦点。
- GUI 细则:桌面窗口,鼠标命中目标 ≥ 28px(高密度);用 `--r-md`/`--r-lg`/`--shadow-lg` 做窗口与卡片。

## 工作纪律（来自 design-spec-kit · 换项目仍成立）
- **先 grep 再写**:造任何 UI 元素前先读 `docs/DESIGN-REF.md` / grep 现有 class——命中就抄类名直接用,**别重造已沉淀的组件**。这是防「页面漂移」的第一道闸。
- **按需披露**:`docs/` 索引按任务需要再打开,**不要预读**全部;深细节走对应 doc。
- **单一真源**:数值只在 tokens.css;改源不改副本,两处冲突以 tokens.css 为准。

## 单一真源 & 不腐化
- **tokens.css 是 token 唯一真源**;`DESIGN-REF.md` 只做索引与语义,不重复定义数值,冲突以 tokens.css 为准并立即修正 DESIGN-REF。
- **新组件准入**:组件只有在 `DESIGN-REF.md` 有条目(类名 + 最小 HTML)后才算「可复用」;没登记的视为临时草稿——**这是阻止「同一个东西长出十个样子」的关键纪律。**

## Changelog 维护
- 维护 `docs/CHANGELOG.md`,**按天 + 模块标签**记录(格式 `- [模块] 描述`)。新增模块同步补顶部「模块索引」。
- **定档即写**:仅把已定稿的工作写入当天 changelog;草稿 / 试验不记录。无需提醒,定档即写。
- **同日合并(硬规则)**:写条目前先 `grep '^## <今天日期>' docs/CHANGELOG.md`,命中就 append 到那段,**绝不新开第二个同日 `## YYYY-MM-DD`**。新的一天在文件**顶部**(模块索引下方)开新段——newest-first。
- **深度上限**:一条 = **1 行标题 + 最多 3 子 bullet**。根因 / 踩坑一句话带过,深内容分流到对应 doc 并指路。
- **滚动归档**:主文件只留最近约 2 个会话日;超 ~200 行就把窗口外**最旧整段(文件底部)**移到 `docs/_archive/CHANGELOG-YYYY-MM.md`(原样保真),底部留链接。

## 收尾同步表（DoD · `done` 前逐行过）
> 核心纪律:**任何影响产物的改动都带一个同步义务**,漏一项 = 文档 / 索引漂移。
> 标 🤖 的由 `tools/` 的 guard 机检(read_file 脚本 → 粘进 run_script 跑,看末行 `RESULT`)。

| 改了 | 必做 | 谁来守 |
|---|---|---|
| `tokens.css` 加 / 改 token | 同步 `DESIGN-REF.md` Token 速查表 | 人 |
| 新增 / 改 / 删可复用组件 | 登记 / 更新 `DESIGN-REF.md` 组件目录(类名 + 最小 HTML) | 人 |
| 改了**颜色相关**值 | 自查:禁裸 `#hex` / 禁裸 `rgba()` / 禁假 fallback,一律 `var(--*)` | 🤖 `check-tokens.js` |
| **GUI 加 / 改图标** | 走 `GUI/gui-icons.jsx`(`ICONS`/`GuiIcon`/`AgentLogo`)·禁页内联自造 rail/工具图标·禁 emoji·新图标先登记 DESIGN-REF | 🤖 `check-icons.js` |
| **TUI 加 / 改字形** | 用 DESIGN-REF「TUI 字形词表」登记字形(色=状态·锚 T4 §08)·禁 emoji·新字形先登记 | 🤖 `check-tui-glyphs.js` |
| 任意定档 | 写当天 `CHANGELOG.md`(先 grep 同日段,命中即 append;最新日期段置顶) | 🤖 `check-changelog.js` |
| 新增 / 改 TUI 或 GUI 屏 | 屏沿用 Aurora token + DESIGN-REF 组件;新组件先登记再用 | 人 |
| **加 / 删 / 改屏**(任何轨) | 同步 `docs/screens-status.js`(屏幕状态唯一真源:id/state/kind/file)→门户 index.html 运行时直读自动更新 | 🤖 `check-status.js` |
| **跨页复用文案** | 同一词多页出现 → 进 `i18n-dict.js`(集中词典),用 `tk()`/`data-i18n-key` 取;页面独有长句才用内联 `t(en,zh)` | 人 |
