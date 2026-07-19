# AGENTS.md

> 跨工具 agent 入口指针(Codex / Cursor 等读 `AGENTS.md`,Claude Code 读 `CLAUDE.md`)。
> **本文件只做指针,不复制任何内容**——复制 = 漂移。真源全在下面指向的文件。

## 先读这两个
- **`CLAUDE.md`**(根) —— 项目说明 + 文档体系 + 工作纪律 + 收尾同步表(DoD)。**入口,先读它。**
- **`docs/DESIGN-REF.md`** —— 组件 / token / 图标速查(类名 + 最小 HTML)。复用组件前读它。

## 冷启动找东西
找 **组件 / 样式 / token / 图标 / 屏** → 看 `CLAUDE.md`「快速检索（冷启动地图）」一节,按表定位真源再 grep。

## 把设计翻译成 app（Rust + Tauri）
看 `CLAUDE.md`「翻译成 app（防漂移）」一节:① `tokens.css` + `GUI/gui-kit.css` + `brand-assets/` 直接共享(零漂移)· ② 原生侧色值走脚本派生 · ③ 原型脚手架(Babel 运行时 / chrome.js / 窗口管理器 / mock 数据)原生重写。

## 铁律
- **单一真源**:数值只在 `tokens.css`;组件登记在 `docs/DESIGN-REF.md` 才算可复用;改源不改副本。
- **先 grep 再写**:命中现有 class 就抄,别重造已沉淀的组件。
