# Viden 设计规范 · AI 速查手册（DESIGN-REF）

> 本文件是**给 AI / 开发快速复用的索引**,不是展示文档(展示见 `Core/Viden - Aurora 主题 (Core).html`)。
> **复用任何组件前先读本文件**:直接抄类名与最小 HTML,不必重读 CSS。
> 黄金规则:**只引用 `var(--*)`,绝不写死颜色 / 字号 / 间距**;改完若定档,按 `CHANGELOG.md` 规矩记录。

## 文件结构
```
项目根/
├── CLAUDE.md                 # 项目说明（根目录，自动加载）
├── tokens.css                # 所有设计变量（唯一真源）← 页面 <link> 它
├── i18n.js                   # data-zh 中英切换
├── tweaks-panel.jsx          # 主题/密度切换面板
├── docs/
│   ├── DESIGN-REF.md         # 本文件 · AI 速查
│   └── CHANGELOG.md          # 更新日志
├── tools/{check-tokens.js, check-changelog.js}
├── brand-assets/             # 图标 / favicon / OG（见下「品牌资产」）
├── Core/                     # 视觉真源：Aurora 主题、品牌、协作机制
├── TUI/                      # 终端版高保真稿
└── GUI/                      # 桌面(Tauri)版高保真稿
```

## 接入方式
页面 `<head>` 引 token 真源,根元素声明皮肤与密度:
```html
<link rel="stylesheet" href="tokens.css">      <!-- 子目录用 ../tokens.css -->
<html data-theme="dark" data-density="compact"> <!-- theme: dark|light|amber|phosphor|ice|mono -->
```

## 页面交互 chrome（每页统一）
> 三个 drop-in 脚本给每页同一套通用交互。放在 app 脚本之前(子目录各放一份副本):
```html
<script src="i18n.js"></script>    <!-- 浮动 EN/中 切换 + window.t(en,zh);data-zh 或 t() 翻译 -->
<script src="chrome.js"></script>  <!-- 浮动「皮肤+密度」切换器 + window.RC -->
```
- `chrome.js`:右下角浮动控件,SKIN(6 皮肤圆点,各显其 accent)+ DENS(紧/中/松)。写 `data-theme`/`data-density` 到 `<html>`,持久化到 `localStorage` 键 **`rc-scheme`/`rc-density`** → **全站任一页切换,其余页自动跟随**。
- `window.RC`:`setScheme(v)`/`setDensity(v)`/`cycle()`;派发 `rc-state` 事件供页面联动(如 Aurora 的 nav 按钮 + Tweaks 面板)。复用已存在的 `window.RC` 不覆盖。
- 例外:Aurora 页用自带 nav `#themeToggle` + Tweaks 面板(更丰富),不挂 chrome.js,但共用同一 localStorage 键。

## Token 速查
> 数值唯一真源在 `tokens.css`;下表只做语义索引。改了 tokens.css 必须同步本表。
> 全部颜色 token 随 `data-theme` 重定义 —— 组件只用语义名,换 theme 即换肤。

### 背景（6 档纵深 · 随 data-theme）
| token | 语义 |
|---|---|
| `--bg-void` | 最沉底(终端 chrome / 窗口外圈 / 输入区底) |
| `--bg-base` | 主背景(画布 / 终端屏) |
| `--bg-panel` | 面板底(侧栏 / 卡片) |
| `--bg-topbar` | 顶栏底 |
| `--bg-elev` | 抬升面(弹层 / 模态) |
| `--bg-sel` | **选中/高亮行底**(载重元素,配青色左竖条) |

### 前景（4 档 · 靠明度分级）
| token | 语义 |
|---|---|
| `--fg-primary` | 主文(承载内容) |
| `--fg-secondary` | 次文(标签 / 结构) |
| `--fg-muted` | 弱文(元数据) |
| `--fg-faint` | 极弱(可看可不看的提示 / 引导线) |

### 强调 & 语义
| token | 语义 |
|---|---|
| `--accent` / `--accent-bright` / `--accent-dim` | 品牌青 = 唯一交互焦点:边框激活 / 标题 / 选中 / focus |
| `--gold` / `--gold-bright` | 次强调金 = viden 字标 / 工作模式 / **「需要人」**(门控·待审批) |
| `--success` / `--warning` / `--error` / `--progress` | 成功绿 / 警告金 / 失败红 / 进行中蓝(刻意与品牌青拉开) |
| `--builtin` | 内置工具紫(GUI 第四类强调) |
| `--border` / `--border-soft` / `--border-active` | 主边框 / 弱分隔 / 激活边框 |
| `--page-bg` `--page-card` `--page-line` `--page-ink` `--page-ink-dim` `--page-accent` | 展示页 chrome(非应用界面用) |

### 字体 / 字阶 / 间距 / 圆角 / 阴影 / 密度
| 类别 | token | 说明 |
|---|---|---|
| 字体 | `--font-sans`(UI/正文 Inter+Noto SC) / `--font-mono`(JetBrains Mono) | 旧别名 `--sans` `--mono` 仍可用 |
| 字阶 | `--fs-kicker` `--fs-xs` `--fs-sm` `--fs-base` `--fs-lead` `--fs-h3/h2/h1` | 行高 `--lh-tight/ui/body`;字距 `--ls-kicker/label` |
| 间距 | `--sp-1`…`--sp-16` | 4px 基准 |
| 圆角 | `--r-xs`(3 TUI) `--r-sm` `--r-md` `--r-lg` `--r-xl`(14 GUI) `--r-full` | TUI 方硬 → GUI 圆润 |
| 阴影 | `--shadow-sm/md/lg` / `--shadow-pop`(TUI 硬投影) | |
| 密度 | `--c-gap` `--panel-pad-x/y` `--rail-mb` `--topbar-mb` | 随 `data-density`=compact/regular/comfy |
| 终端框 | `--term-screen` `--term-chrome` `--term-bar` `--term-edge` / `--win-edge` | 仿真实终端/桌面窗口外框 · **theme-independent 固定深色**,刻意区别于 Aurora 应用面 |
| 动效 | `--motion-fast/base` `--ease` | |

## 组件目录
> 每个可复用组件一条:类名 + 一句用途 + 最小 HTML。**没登记的视为临时草稿。**
> 颜色文字助手(TUI/展示通用):`.k-accent .k-bright .k-gold .k-goldb .k-success .k-warn .k-error .k-prog .k-pri .k-sec .k-mut .k-faint` + `.b`(加粗)。

### 展示页 chrome

**`.topnav` / `.brand` / `.navlinks` / `.toggle`** — 展示页顶栏(品牌 + 锚点 + 主题切换)。
```html
<nav class="topnav">
  <div class="brand"><span class="mono"><span class="c">[</span>◉<span class="c">]</span></span>
    <span class="mono"><span class="r">v</span><span class="c">iden</span></span></div>
  <div class="navlinks"><a href="#tokens">Color tokens</a></div>
  <div class="toggle"><span class="dot"></span><span>Aurora</span></div>
</nav>
```

**`.kicker` / `h1 .ac` / `.lead`** — 区块小标签(mono 大写+前导横线)/ 大标题(青字 highlight)/ 引言。
```html
<div class="kicker">Deliverable A · TUI theme skin</div>
<h1>Aurora — the <span class="ac">visual theme</span></h1>
<p class="lead">Skin only, no layout change.</p>
```

**`.rcard`(推理卡) · `.gcard`(状态画廊) · `.callout`(说明块) · `.toktable`(token 表) · `.ladder`/`.lrow`(层级阶梯)** — 展示页内容块,均 `var(--page-*)` 系。

### TUI canonical kit（`tui-kit.css` · 单一类名词汇 · 防漂移)
> **新 TUI 屏一律用 `tui-kit.css` 的 `.v*` 类,别再各页自造终端框/状态栏/红绿灯/后端 chip/lane 行/闸。**
> 规范源 = `T4 交互规则`。可交互整合参考 **`Viden - 统一原型 (TUI).html`**(切 lane ⌃L · 命令面板 ⌃P · 决策中心 ⌃G · 4 档审批闸)。
> 接入:`<link href="../tokens.css"><link href="tui-kit.css">`。
- **终端框**:`.vterm` + `.vterm-chrome`(`.vlights`>`i.a/.b/.c`=红/黄/绿 + `.vtitle`) + `.vterm-body`。
- **状态栏**:`.vstatus` > 左固定 `.vseg`(`.proj` 青底 / `.lane` / 后端段)+ **`.vticker`(中段自动滚动跑大量指标 `.vtk-item`>`.lbl`+`.v`,`.vtk-sep` 分隔,悬停暂停;内容双份无缝循环)** + 右固定 `.vseg.r` · `.vgate-badge`(金 ⏸N 点跳决策) · `.vled`(连接) · `.vhelp`。
- **后端 chip(铁律 ACP=`--accent` · built-in=`--builtin` · tmux=`--gold`)**:`.vbe.acp/.builtin/.tmux`,格式 `<类型>:<agent>`(如 `ACP:codex`);边框变体 `.vbe-pill.*`。
```html
<span class="vbe acp"><span class="k">ACP</span>:<span class="ag">codex</span></span>
```
- **lane 行**:`.vlane.s-{gate|run|wait|done|block}`(状态色 gate金/run蓝/wait灰/done绿/block红)+ `.on`(选中)·`.vled-st`·`.vlane-id/-role/-meta`·`.vlane-gate`(金 ⏸ 闸标)。
- **4 档审批闸**:`.vgate` > `.vgate-head`(`.badge`+`.risk`) · `.vgate-cmd` · `.vgate-opts`>`.vgate-opt`(`.n` 1–4,`.deny`,`.on`) · `.vgate-foot`(`.cd` 倒计时自动拒)。键:1–4 直达 / ↑↓ / ⏎。
- **快捷键提示**:`.vhints`>`.h b`(键金色)。**overlay**(命令面板/决策/lane 切换):`.vscrim`>`.voverlay`>`.voverlay-in`+`.voverlay-list`>`.vorow.on`+`.vogrp`+`.voverlay-foot`。
- 文字色助手:`.vk-accent/-bright/-gold/-success/-warn/-error/-prog/-builtin/-pri/-sec/-mut/-faint` + `.vb`。

### TUI（终端驾驶舱 · 旧展示页组件,逐步迁移到 tui-kit）

**`.tui` / `.tui.win` + `.tui-chrome` + `.tlights`** — 终端窗口框(红黄绿灯 + 标题)。
```html
<div class="tui win">
  <div class="tui-chrome"><div class="tlights"><i class="l1"></i><i class="l2"></i><i class="l3"></i></div>
    <span class="tlabel">viden — fish — 142×40</span></div>
  <div class="tui-screen">…</div>
</div>
```

**`.c-topbar` + `.chip`** — 顶部状态条(单下边线,不成框)。`.wm` 字标 / `.ver` 版本金 / `.chip .lbl` 标签青。
```html
<div class="c-topbar">
  <span class="wm"><span class="k-accent">[</span><span class="k-bright">◉</span><span class="k-accent">]</span>
    <span class="k-gold">v</span><span class="k-accent">iden</span></span>
  <span class="ver">v0.1.30</span>
  <span class="chip"><span class="lbl">MODEL</span> <b>deep~flash</b></span>
</div>
```

**`.panel` / `.panel.rail` + `.ptitle` + `.pcount`** — 精简面板:线分隔而非整框;`.railwrap` 给右栏加左竖线。
```html
<div class="panel rail"><span class="ptitle">ACTIVE TASKS</span><span class="pcount k-accent">4</span>…</div>
```

**`.sel-list` / `.sel-row` / `.sel-row.on`** — ★选择器高亮行(整个界面视觉锚点):选中行 `--bg-sel` + 青色 `inset 3px` 左竖条。
```html
<div class="sel-list">
  <div class="sel-row on"><span class="mk">›</span><span class="cmd">/model</span><span class="desc">switch model</span></div>
  <div class="sel-row"><span class="mk"> </span><span class="cmd">/lane</span><span class="desc">new lane</span></div>
</div>
```

**`.livework`** — LIVE WORK 进行中条(最亮层级,青边 + 青底 7%)。
```html
<div class="livework"><div class="lw-title">╭─ LIVE WORK •</div>
  <div class="lw-main"><span class="pulse">◉</span> Supervising 1 agent</div>
  <div class="lw-meta"><b>phase</b> testing · <span class="k-prog">64%</span></div></div>
```

**`.composer` + `.modepill` + `.mode-pop`** — 输入区(顶边线)+ 模式 chip + 模式选择弹层(金框 + 硬投影 + reverse-video)。`.cur` 闪烁光标。
```html
<div class="composer"><div class="cinput"><span class="cprompt">›</span>
  <span class="ctext">Add tests…<span class="cur"></span></span>
  <span class="modepill">MODE Build ▾<span class="mode-pop">…</span></span></div></div>
```

**`.c-statusbar` + `.sb-ticker` / `.sb-track` + `.sb-item`** — 合并式滚动状态 ticker(悬停暂停);左侧 `.sb-gear` 配置入口,右侧 `.sb-help`。

**`.approval` + `.ap-head .badge` + `.diffbox`(`.dl-add/.dl-del/.dl-ctx`) + `.ap-acts` / `.tact` / `.tact.focus`** — 审批模态(金/警告框 + box-drawing + 键盘优先动作菜单,reverse-video 焦点)。
```html
<div class="approval"><div class="ap-head"><span class="badge">GATE</span><span class="title">write_file</span></div>
  <div class="ap-body"><div class="diffbox"><span class="dl-add">+ added</span></div>
    <div class="ap-acts"><span class="tact focus"><span class="ky">a</span>Approve</span>
      <span class="tact deny"><span class="ky">d</span>Deny</span></div></div></div>
```

**`.welcome` + `.robot`(`.eye`/`.base`)** — 启动欢迎屏(ASCII 机器人 + 青色辉光)。
**Lane 监视模式 `.lm-banner` / `.lm-grid` / `.lm-item.on` / `.tbar`** — 全屏 lane 监控(同样 `.on` 高亮规则)。

### GUI（桌面 / Tauri）

**窗口标题栏 chrome（D1 同源 · 全 GUI 横屏页统一规范）** — 三套等价实现,**度量必须一致**:`.titlebar`(D1/召唤坞,含 `.tbtitle`)· `.vbar`(流程/内容页,共享壳)· `.winbar`(D10 监视墙,用 `.dots` 代 `.tl`)。规范:**栏高 46px · gap 13px · padding 0 14px · 交通灯点 12px(gap 8,红 `--error`/黄 `--warning`/绿 `--success`)· 字标 mono 700 13.5px**。字标 = `[◉]`(括号 `--accent` + `◉` `--accent-bright`)+ `v`(`.r`=`--gold`)+ `iden`(`.c`=`--accent`)。
```html
<div class="titlebar"><div class="tl"><i class="a"></i><i class="b"></i><i class="c"></i></div>
  <span class="wm"><span style="color:var(--accent)">[</span><span style="color:var(--accent-bright)">◉</span><span style="color:var(--accent)">]</span> <span class="r">v</span><span class="c">iden</span></span></div>
```

**`.frame`** — 桌面窗口框(`--r-xl` + `--shadow-lg`,固定高列布局)。
```html
<div class="frame"><div class="cmdbar">…</div><div class="body">…</div><div class="composer">…</div></div>
```

**`.cmdbar` + `.lg`(logo) + `.path` + `.cbtn` + `.dentry`(决策入口带 `.bdg` 红角标)** — 顶部命令栏。

**`.lanebar` + `.lrow` / `.lrow.on` + `.led` + `.gate` + `.tgtchip`** — lane 侧栏(选中 `--bg-sel` + 青左竖条;`.gate` 金角标待审;`.tgtchip` 远程目标)。
```html
<div class="lrow on"><span class="led"></span><div class="tx"><div class="t1"><span class="lid">L1</span> codex</div>
  <div class="t2">s1/pty/01</div></div></div>
```

**`.sesstabs` + `.stab` / `.stab.on`** — 会话标签页(底部青色 active 线)。
**`.targetbar`(`.ssh` 变体)** — 远程目标状态条(蓝色系)。
**`.work` 内转录:`.umsg`(用户) · `.amsg`/`.ahdr`/`.atext`(助手) · `.tool`(`.add/.del/.ok/.wn`) · `.gatemsg`+`.gobtn`(门控提示)** — 桌面版转录消息族。
**`.composer` + `.cline` + `.pm`(模式 pill)** — GUI 输入行(圆角框 + 真 input)。

## 品牌资产（`brand-assets/`）
- `app-icon-1024.png`(深色,Tauri 主图标)/ `app-icon-light-1024.png`(浅色变体)。
- `favicon-{512,192,64,32}.png`(网站/PWA 多档)。
- `og-banner-1200x630.png`(README / og:image)。
- `icon.svg`(92×92 矢量母版,任意尺寸无损导出/改色)。瞳孔金色是小尺寸识别点,不要去掉。
- 字标:`[◉]` glyph + `v`(金)`iden`(青);吉祥物机器人头青色线稿。

## 图标与贡献约定
- 图标:终端态用 box-drawing / Unicode 几何符(◉ ● ◆ ⚒ ♙ ▸ ╭─);避免 emoji。
- 新增组件先在本目录登记(类名 + 最小 HTML),再写进 CHANGELOG。**没登记 = 临时草稿。**
