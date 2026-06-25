# Viden 更新日志（Changelog）

> 按**天**记录更新,每条标注涉及的**模块**便于筛选。**最新日期段在最上面**(newest-first)。
> 格式:`- [模块] 变更描述`。一天内可有多条,按模块归类。
> **定档即写**:仅把已定稿的工作写入;草稿 / 试验不记录。模块索引随新模块扩充。
>
> **卫生三条**(由 `tools/check-changelog.js` 机检):
> 1. **同日合并**——写前先 `grep '^## <今天>'`,命中就 append,绝不开第二个同日 `## YYYY-MM-DD` 段。
> 2. **深度上限**——一条 = 1 行标题 + 最多 3 子 bullet;根因 / 踩坑一句话带过,深内容分流到对应 doc。
> 3. **滚动归档**——主文件只留最近约 2 个会话日;超 ~200 行把窗口外**最旧整段(文件底部)**移到 `_archive/CHANGELOG-YYYY-MM.md`,底部留链接。

## 模块索引
- **设计规范** — tokens / 组件 / 展示文档
- **文档** — CLAUDE.md / DESIGN-REF / 本文件
- **TUI** — 终端版屏与组件
- **GUI** — 桌面(Tauri)版屏与组件
- **品牌** — logo / favicon / OG / 图标

---

## 2026-06-24
- [GUI] **横屏页窗口标题栏统一对齐 D1 规范**:把全部 11 个 GUI 横屏页的标题栏度量校准到 D1 同一套(栏高 46px · gap 13 · padding 14 · 交通灯点 12px · 字标 mono 13.5px)。① 8 个 `.vbar` 流程/内容页(D2决策/D4/D5/D7/D9/D11/D12/D13):44→46 高、gap 12→13、字标 13→13.5;② D2横屏/D3竖屏 `.titlebar`:补齐高/gap/padding + 灯点 11→12 + 字标 13→13.5;③ D10 `.winbar`:34→46 高、灯点 11→12、点距 6→8、字标 12.5→13.5。
  - [文档] DESIGN-REF GUI 段登记**窗口标题栏 chrome**条目(三套等价实现 .titlebar/.vbar/.winbar + 统一度量 + 字标最小 HTML)。
- [GUI] D1 内部「屏幕适配」三个分辨率示意图(k4 4K / uw 超宽 / ptd 竖屏)的迷你标题栏对齐主壳规范:交通灯裸 `#hex` → `--error/--warning/--success` token;字标 `[◉]` 单色 → `[`青 / `◉`亮青 / `]`青 标准拆分(与 `.tl`+`.wm` 同源);k4/uw 交通灯点 8→12px、点距 5→8 与 ptd/主壳统一。
- [GUI] 同三个示意图的迷你状态栏(`.k4sb/.uwsb/.ptdsb`)对齐主 `.statusbar` 规范:补 `font-family:var(--mono)`;统一 `⚙` 金齿轮起头 + `┆` 分隔符(uwsb 原 `●`+`·`、ptdsb 原 `·` 已校正)。
- [GUI] k4/uw 示意图 activity rail 基本元素对齐主驾驶舱:ASCII 字符(◉⎇±▤⚠)→ 主壳 SVG 图标(IChat/IWorktree/IReview/IEvid/IDiag,`svg.ic` 缩到 13–14px),随 `.k4act/.uwact` currentColor 着色 + `.on` 高亮;ptd 竖屏沿用带标签侧栏(与主 ThreadSidebar 同模式)。

## 2026-06-19
- [Core][GUI] **Lane ≡ Session(1:1)定调**:lane 即一个会话,层级压成 `Workspace → Project → Lane(=会话) → Subagent`;并明确 lane 归属二选一(挂某 project / 跨项目全局 lane 直属 workspace)。① 产品方案 v2 层级图删 Session 子层、顶补 Workspace + 全局 Lane;② D1 驾驶舱去掉中间区会话 tab 栏改为单条 **lane 上下文头**(名字优先 flex);③ 左栏"＋新建 lane"弹窗改 `position:fixed` 逃出 `.side` overflow 裁剪。
  - [GUI] D1 **Workspace 面板重构为 lane 归属视图**:顶部「全局 Lane(跨项目)」直列 + 每个 project 展开列其名下 lane(lane 数 badge,0 lane 显占位);THREADS 增 `proj` 字段(L1–L3→robocode,L4→全局),原文件树/Worktrees 小列表移除。`.wsglobal/.wsghd/.wslane/.wslanes(.glob)/.wsseclabel/.lct`。
- [GUI] 新增 **D13 Fleet 编排与 Workflow** 页:把产品逻辑(workspace→主 agent 规划 fleet→project/lane→subagent + 群会话通道)可视化成横向流水线 DAG(Orchestrate→Plan→Implement→Verify→Review→Integrate),节点按角色聚合(×数量 + run/done/gate/idle 状态段),点群组下钻个体样本(+N more);右侧群会话通道(广播/回报/闸事件流 + 分派 composer),顶部统计条体现 1,284 agent 规模。沿用统一窗口壳(vbar+vrail)。
- [GUI] 7 个无壳流程/内容页统一包进 **D1 窗口壳**(交通灯 + `[◉] viden` 字标 + 项目选择器 + 工具按钮):D2决策中心/D5画廊/D7收件箱/D12集成闸 另加左侧 **ActivityRail**(对应视图图标高亮 + 角标);D11首启/D4 Lane创建/D9远程 因自带向导/lane 导航,仅加身份 titlebar。复用 `.vbar/.vbtn/.vrail/.vrailbtn` 与 `VBar`/`VRail` 组件。
- [GUI] D10 Lane监视器 窗口栏对齐 D1 身份:灰点改 macOS 红/黄/绿交通灯 + 补 `[◉] viden` 字标(dash/strip 两个 winbar 同步);D2横/D3竖召唤坞经核已与 D1 同源(`.tl .a/.b/.c`+`.wm`)。
- [GUI] D1 驾驶舱 Environment 面板参照 OpenCode 右栏扩充:新增 **Context**(token/进度条/花费)、**MCP**(服务连接列表)、**LSP**(rust-analyzer 状态)、**Todo**(当前 lane 任务勾选,active 金色高亮)四段 + 底部 cwd/分支/版本 footer(`LANE_ENV`/`MCP_SERVERS` 数据,`.envctx/.mcprow/.todorow/.envfoot`);composer hint 补 token%/花费/⌃P。
- [GUI] D1 驾驶舱 点击右边栏文件 / subagent 行,从中间窗口**分出 Inspector 分栏**(介于对话与 Environment 之间):文件显示 File/Diff 代码视图,subagent 显示 Output/Evidence + 操作;可拖宽(`.centerwrap`/`.inspector`/`InspectorPane`),max-width 56% 保证对话不被挤没。
- [GUI] D1 驾驶舱 Environment 面板三段(Environment/Subagents/Sources)均可点标题**折叠**(caret 旋转),内容超高时面板上下滚动;subagent 行可点击(选中高亮 + 跳 Lane 监控)。
- [GUI] D1 驾驶舱 右边栏 **Subagents 面板重构为「Environment」复合信息栏**(Codex 风):Environment(Changes/Local/分支/Commit·push/PR 状态)+ Subagents(像素机器人图标 `IRobot` 按 lane 色 + diff 统计)+ Sources 三段,随当前 lane 切换;新增 `.envp/.envsec/.envrow` 组件、dockbody 改可滚动。
- [GUI] D1 驾驶舱 右边栏 ContextDock 面板改为**可开关**:每个 tab 带 × 关闭,+ 按钮下拉「打开面板」补回已关闭项,默认只开 Subagents;全关时显示空态快捷开按钮。
- [GUI] D1 驾驶舱 lane 列表项支持双击 / 三角展开,内联列出该 lane 的 subagents(`.thsubs`/`.twirl`)。
- [GUI] D1 驾驶舱 右边栏 ContextDock 改 subagents→env tab,默认开 Environment;dtadd `+` 弹菜单开新 tab(已关闭面板)。

## 2026-06-18
- [GUI] D1 驾驶舱 侧栏 lane 卡**双击展开**(或点 ▸ 三角)显示该 lane 的 subagent 列表(读 `LANE_SUBAGENTS[lid]`),新增 `.thsubs`/`.thsub`/`.twirl` 样式。
- [GUI] D1 驾驶舱 ＋ 按钮点击弹出**已配置智能体选择菜单**(`AGENT_PRESETS`:ACP 外部 CLI + 内置智能体 + 模板),新增 `.addmenu` 样式。
- [GUI] D1 驾驶舱 Workspace 支持**多项目文件夹**:新增 `WorkspacePanel` + `WS_PROJECTS`(robocode/viden-web/infra 各自可折叠树)+「添加项目」入口;Lanes/Workspace 切换移到侧栏顶部并右挂 ＋ 新建 lane,去掉旧 navtop/sidehead/页脚说明。
- [GUI] D1 驾驶舱 左侧栏新增 **Lanes / Workspace** 分段切换:Workspace 态显示工程资源管理器(robocode 仓库头 + crates/tests 文件树 + 各 lane worktree 列表),新增 `.lseg` 分段控件 + `WS_TREE` 数据,复用 `.wsrow`/`.wtmini`/`.sidesection`。
- [GUI] D1 驾驶舱 下边坞 ▾ 按钮改为直接关闭整个下边栏(连同 tab 行,不留折叠条),重开走顶栏 ⌘J。
- [GUI] D1 驾驶舱 PROV(provider/model)与 PERM(权限)选择器从 titlebar 移到对话窗口下方 composer 的 cmeta 行,下拉改为向上弹出(`.tbmenu.up`);titlebar 右侧只留工具按钮。
- [GUI] D1 驾驶舱 Lane 监控改为**按 lane 各自的 subagent 树**:新增 `LANE_SUBAGENTS` 映射(L1–L4 各自 orchestrator 派生多个子代理 + 每节点输出/证据/stats),点节点切换 inspector,切 lane 自动重置选中。
- [GUI] D1 驾驶舱 下边坞空态改为一排可点的召唤按钮(`.sdquick`/`.sdqbtn`)直接开 Terminal/Files/Review/Browser/Side chat,替掉原纯文字提示。
- [GUI] D1 驾驶舱 右边栏(ContextDock)宽度可拖动:新增 `rightW` state + `.lresize` 右侧把手,范围 240–480px(下边栏 DockSD 已有 `sdgrip` 高度拖动)。
- [GUI] D1 驾驶舱 titlebar 右侧重排:面板开关收到最右端——`tbdiv` 分隔后依次 2 个 `tbtbtn.ghost` 占位按钮(虚线·待定义)+ 下边栏开关 + 右边栏开关(最右);cmd/focus/popout 移到分隔左侧。

## 2026-06-16
- [TUI] 统一原型 More 栏加**皮肤切换**:6 套 SKIN swatch(dark/light/amber/phosphor/ice/mono · 预览底色 + accent/gold 点)+ DENSITY 段控,写 `data-theme`/`data-density` 并 localStorage 持久化。
- [TUI] 统一原型驾驶舱终端底色**随皮肤换肤**:`#cockpit` 把固定 `--term-screen/chrome/edge` 别名到当前主题的 `--bg-void/--bg-topbar/--border-soft`(零裸色值)——修绿/单色主题下终端底不变的问题,light 主题终端转浅可读。
- [TUI] 统一原型右栏可隐藏:状态栏 `? help` 旁加 `▦ rail ⌃B` 开关(点击切换 + ⌃B 快捷键),关时 `.c-body` 转单列、转录铺满,带 .18s 过渡。
- [TUI] 统一原型 lane 行**点击内联展开**名下在跑的后台 agent(单源 `LANE_AGENTS`:run 转圈 / done ✓ / wait 灰 + backend 色标签 + 进度),行尾运行数 + 折叠箭头;固定底部 BACKGROUND 条移入 **More 栏**(按当前 lane 显示,与 lane 展开共用同源)。
- [TUI] 统一原型 composer 输入改 textarea:默认 2 排,随内容自增,最多 5 排(~97px),再多内部滚动。`›` 前缀置顶第一排(`align-self:flex-start`)。
- [TUI] 统一原型(TUI):chrome 快捷键提示改为 lane/cmd/decisions 三段切换 tab(青高亮 + 待审金点);lane 标签行去重为单行上下文头;状态栏删版本号 + lane 重复段,舱高提到 `min(760px,vh-40)`。
- [TUI] 统一原型(TUI)新增**首屏/启动欢迎屏**:复用 Aurora `.welcome`+`.robot` 语汇(ASCII 机器人 + 命令选择器),`view` 态切换,enter/点选进驾驶舱,chrome ⌂ home 返回。
- [TUI] 新增**会话页(opencode 借鉴)**:右「项目状况」栏(Context 计量 + MCP 连接表 + Todo + Modified Files 全量增删 + LSP) + 主转录流(thought 折叠 / tool / 命令块 / 绿增 diff + 左右分栏 diff(红删) / tool-error / PostToolUse hook / markdown 表格 / 双色小标题 + ✅ 完成态 / 列表 / 用户消息回显 / Todo 勾选 / 统计汇总行 / Skill 行 / permission 动作条 / composer)。全程 mono + tokens,screen-local 类未入 kit。
- [TUI] 会话页侧栏加**顶部 tab**(Project ⌃1 / Lane ⌃2 / More ⌃3,名字 + 快捷键,点选或 ⌃1/2/3 切换)+ **区块可折叠**(▾/▸ 点头折叠);新增 Lane 面板(并行研究轨道 + Activity)与 More 面板(Session / Usage / Keybindings)。
- [TUI] 统一原型(TUI)同步侧栏改造:`Rail` 组件加顶部 tab(Lane ⌥1 / Project ⌥2 / More ⌥3)+ 可折叠区块;Project 面板补 Context/Backends(按 ACP/builtin/tmux 着色)/Modified,More 面板补 Session/Keybindings。
- [TUI] 侧栏 tab 快捷键 ⌃1/2/3 → **⌥1/2/3**(避开浏览器 ⌘-数字切标签;handler 用 `e.code` 跨平台);会话页与统一原型同步。
- [TUI] 加 **HUD 余光层**(会话页 + 统一原型):转录右上「新信息」toast(金=notice/青=update,可关)+ 右栏底「后台任务」常驻条(spinner/进度/✓ done)。color=state,不阻断主任务。
- [TUI] 统一原型转录并入会话页**全套富转录块**:Thought 折叠 / 命令块(可展开)/ 左右分栏 diff(红删)/ PostToolUse hook / markdown 表格 / tool-error / ✅ 完成态 + Todo / Skill 行 / 统计行 / model 署名,均按 boss-rush(HITSTUN)叙事改写,沿用 `oc-*` 类。

## 2026-06-15
- [文档] 从 design-spec-kit 起步,落地 CLAUDE.md 契约 + DESIGN-REF 索引 + DoD guard。
- [设计规范] 建 `tokens.css` 单一真源:收编 Aurora 6 套皮肤(dark/light/amber/phosphor/ice/mono)+ 密度 3 档 + 字阶/间距/圆角/阴影。
  - 颜色随 `data-theme` 换肤、密度随 `data-density`;组件只用语义 `var(--*)`。
  - 青 `--accent`=唯一交互焦点,金 `--gold`=「需要人」语义,选择器高亮行(`--bg-sel`+青左竖条)为视觉锚点。
- [设计规范] DESIGN-REF 登记组件目录:展示 chrome + TUI(panel/sel-row/livework/composer/approval)+ GUI(frame/lanebar/work)。
- [Core] 7 个 Core 屏接入 `../tokens.css` 单一真源:删本地 `:root`/主题副本,语义色 rgba 微底→`color-mix(var(--*))`,阴影/红绿灯→token。
  - 品牌资产页旧别名(`--bg/--card/--line/--ink/--dim`)映射到 `page-*` 真源;onlight/stage.lite 浅色预览为刻意保留的主题无关固定值。
  - 已知:`check-tokens` 沙箱 readFile 读不了含空格/中文/括号的文件名 → Core/GUI/TUI 的 HTML 屏不被扫描,迁移以人工 grep + 视觉核验为准。
- [TUI] 11 个 TUI 屏接入 `../tokens.css`:删本地 `:root` 主题副本(T1 保留 `--acp/--tmux` 别名映射到 token;T0/T5 保留 `.depth256` ANSI-256 降级演示为内容)。
  - 余留:组件级语义色 rgba 微底 + 真终端模拟器外框近黑(`#06090e` 等)未 token 化——稳定且镜像 token、非漂移源,留作下一轮 de-hex。
- [GUI] 12 个 GUI 屏接入 `../tokens.css`:删本地 `:root` 主题副本(D1 原版连 6 套 `html[data-theme]` 皮肤一并收归;D1 v2/D11 含 `--builtin` 变体)。
  - D1 红绿灯 → `var(--error/warning/success)`;窗口/宠物外框近黑(`#1c2c3a/#11212e/#070d13`)同 TUI 终端框,留作下一轮 de-hex。
- [设计规范] de-hex 一轮:TUI/GUI 23 屏语义色 rgba 微底 → `color-mix(var(--*))`(128 处,经 copy→ASCII 临时名 →script→copy-back 批处理绕开沙箱文件名限制);`i18n.js`×3 字标切换器 token 化(随主题换肤);`tui-screens.jsx` 语义 SVG 标记/红绿灯 → `var()`(地形分类色保留)。
  - `tweaks-panel.jsx` 刻意保留:独立浅色面板皮肤、非 Aurora 调色板,不映射。
  - baseline 重同步 195→170;近黑外框/scrim/黑阴影留作可选 `--term-*` 收编(下一轮)。
- [设计规范] ③ 终端/窗口外框近黑收编:tokens.css 新增 `--term-*` 组(`--term-screen/chrome/bar/edge` + `--win-edge`,**theme-independent 固定深色**,刻意区别于 Aurora 应用面=真实终端保真基线)。
  - TUI 9 屏 + GUI(D1 窗口框/Pip 宠物)+ D5 画廊 SVG(改 `style` 吃 `var`)全部 token 化;DESIGN-REF Token 表同步登记。
  - 至此 Core/TUI/GUI 设计面**零裸色值**;剩余仅 `tweaks-panel.jsx`(独立浅色皮肤)+ 黑阴影(中性),均有意保留。
- [设计规范] 交互对齐:新增共享 `chrome.js` —— 通用「皮肤+密度」浮动切换器(右下角;6 皮肤圆点用 `[data-theme]` 各显其色 + 紧/中/松;`window.RC` + localStorage `rc-scheme`/`rc-density` → **全站任一页切换其余页自动跟随**)。
  - 注入全部 Core(除 Aurora,已有等价 nav+Tweaks)+ TUI 共 17 屏;缺 `i18n.js` 的 5 个 Core 静态页一并补齐语言切换。
  - chrome.js 全 `var(--*)` 上色、零裸色值;DESIGN-REF 新增「页面交互 chrome」节登记。
- [TUI] 一致性收敛:建 `tui-kit.css` canonical 组件库 + `Viden - 统一原型 (TUI).html`(可交互),把终端框/状态栏/红绿灯/后端 chip/lane 行/4 档审批闸/快捷键提示/overlay 统一成一套 `.v*` 类名。
  - 后端铁律 ACP=`--accent`·built-in=`--builtin`·tmux=`--gold`、格式 `类型:agent`(如 `ACP:codex`);权限闸统一 **4 档+超时拒**;原型 ⌃L/⌃P/⌃G + 点 ⏸ 触发闸,均实测通过。
  - DESIGN-REF 新增「TUI canonical kit」节登记;旧各页终端组件标注「逐步迁移到 tui-kit」。

<!-- 新的一天在「模块索引」下方、本段之上开新 `## YYYY-MM-DD`（newest-first）。 -->
