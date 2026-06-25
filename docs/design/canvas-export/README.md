# Viden 设计系统 · Canvas 导出（权威原件）

来源:Claude Design 项目 **「Robocode」**(`e9da4ee8-858c-470b-9eb2-58b3ed3795fb`,普通设计项目,owner wiki)。
品牌名 **Viden**。本目录是该项目网页导出的**忠实镜像**(原结构),供代码评审与实现参考。

## 结构

```
canvas-export/
├── CLAUDE.md                 # Viden 项目契约（定位/基调/token 纪律/DoD）
├── tokens.css                # Token 单一真源 + 6 套换肤主题
├── i18n.js                   # 中英切换（根副本）
├── tweaks-panel.jsx          # 主题/密度面板（刻意非 token 化的浅色独立皮肤）
├── docs/
│   ├── DESIGN-REF.md         # ★设计系统 AI 速查总纲（token 表 + 全组件目录）
│   ├── CHANGELOG.md          # 全项目设计演进史
│   └── previews/manual/0.1.30/readme/*.png   # 0.1.30 真机 TUI 截图
├── Core/   7 HTML            # Aurora 主题 / 品牌 / Lane 机制 / 产品方案 v1·v2 / 审查看板
├── TUI/   13 HTML + 源       # T0–T5 + 借鉴研究 + 统一原型；tui-kit.css / tui-screens.jsx / tui-gate-timeline.jsx
├── GUI/   13 HTML            # D1–D13 驾驶舱族 + 宠物 Pip
├── design-spec-kit/          # 与平台无关的设计纪律方法套件（契约/DoD/两个 guard）
├── tools/                    # check-tokens.js / check-changelog.js / baseline.json（Viden 应用版）
└── screenshots/  90 PNG      # 迭代过程截图（设计演进记录）
```

> 每个 `Core/` `TUI/` `GUI/` 子目录各带一份 `i18n.js` / `chrome.js` / `tweaks-panel.jsx` 副本(页面 drop-in)。

## 品牌资产（已提升到 `docs/brand/`）
`icon.svg`(92×92 母版)· `app-icon-1024.png`(+ light)· `og-banner-1200x630.png` · `favicon-{32,64,192,512}.png`

## 未纳入（留在导出文件夹 `~/Documents/viden-deign/`）
- `uploads/`(38M,~30 张粘贴大图)—— 体积过大,不进 git
- `support.js`、`Canvas.dc.html` —— Claude Design 平台 canvas 运行时(`GENERATED · do not edit`),非 Viden 内容

## 与 design-system/ 的关系
仓库另有 `design-system/`(`@viden/design-system`,storybook 形态 React 组件库)= 本设计的**代码化实现**,
用于 design-sync 同步到专用 DS 项目 `9c2c6c63-...`(非本「Robocode」项目)。
本目录是**设计稿/规格原件**;`design-system/` 是**可编译实现**。两者同源,改动需同步。

## 项目身份提示
`e9da4ee8`(Robocode)类型 `PROJECT_TYPE_PROJECT`(普通设计项目),**design-sync 无法往它同步组件库**。
