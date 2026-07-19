# CODEX HANDOFF · Viden UI 开发交接

> 给用 **Codex** 做 UI 开发的工程师/agent。本文件是**实现简报**,只做导航 + 交接约定,**不复制任何数值**(数值真源全在 `tokens.css` / `DESIGN-REF.md`,复制=漂移)。
> 先读 `AGENTS.md`(跨工具入口) → `CLAUDE.md`(项目说明 + 文档体系 + DoD) → 本文件。

---

## 0. 这批文件是什么(读之前必须懂)

这个包里的 `Core/` `TUI/` `GUI/` 下的 `.html` 都是 **HTML 高保真设计参考稿(design references)** —— 用来钉住「长什么样、怎么交互」的原型,**不是拿来直接上线的生产代码**。

你的任务 = **在目标工程环境里重建这些设计**,不是把 HTML 原样搬进去:
- **GUI(主实现目标)**:**Rust + Tauri**。Tauri 前端 = webview = HTML/CSS/JS,所以设计层**可以直接共享**部分文件(见 §2),不是手抄翻译。
- **TUI**:**Rust** 终端应用(等宽字符栅格);HTML 稿只表达布局/状态/字形语义,用 TUI 框架(ratatui 等)重建。
- **Core**:视觉真源 + 文档展示页,**不是要实现的产品屏**,是给你对齐配色/品牌/机制的参考。

**保真度 = 高保真(hifi)**:颜色、字体、间距、圆角、阴影、交互态都是最终稿,请像素级还原,但用工程环境自己的组件/框架实现。

---

## 1. 从哪开始找东西(冷启动地图)

| 要找 | 唯一真源 | 怎么取 |
|---|---|---|
| **token**(色/字号/间距/圆角/阴影/密度) | `tokens.css` | grep `--accent`;皮肤段看 `[data-skin][data-mode]` |
| **组件**(类名 + 最小 HTML) | `docs/DESIGN-REF.md` → `GUI/gui-kit.css` · `TUI/tui-kit.css` | grep 类名 `.wslane` / `.mgate` / `.vterm` |
| **GUI 图标** | `GUI/gui-icons.jsx`(`ICONS`/`GuiIcon`/`AgentLogo`) | grep key `chat`/`lock` |
| **屏 / 页**(画到哪 · 文件路径) | `docs/screens-status.js`(机读) | grep id / file |
| **设计决策 / 护栏 / 开放问题** | `docs/SPEC.md` | grep `@DECISION`(锚 `D-*`) / `@OPEN` |
| **命名映射**(旧名→新名) | `docs/NAMING-MAP.md` | — |

**铁律:先 grep 再写**。造任何 UI 元素前先读 `DESIGN-REF.md` / grep 现有 class,命中就抄类名直接用,别重造已沉淀的组件。

---

## 2. 翻译成 app 的三层(防漂移 · 详见 `CLAUDE.md`「翻译成 app」)

- **① 原样共享(零漂移)** — `tokens.css` + `GUI/gui-kit.css` + `brand-assets/*.svg`。Tauri 前端**直接 import**,组件渲染**同一套类名 + DOM 结构**(`.frame/.wslane/.mgate/.envp/.gperm`…)。改 token 两边同步。**视觉真源 = `GUI/Viden - 桌面驾驶舱 (GUI).html`(D1)**。
  - 建议:把 ① 的文件作为 git submodule / 共享包 vendore 进 app,`git pull` 即同步。**别在 app 留手改副本**(留副本 = 漂移源)。
- **② 脚本派生(单向)** — 原生侧(托盘/菜单/窗口 chrome)要色值时,用生成器 `tokens.css → tokens.json/.rs`,**禁手抄**。`.css` 永远是源。
- **③ 原生重写(本不共享)** — React+Babel 运行时转译、`chrome.js` 换肤器、窗口管理器、`tweaks-panel`、mock 数据 = **原型脚手架**,app 用正经构建/框架实现。**视觉不在这层,全在 ① 的共享 CSS 里。**

---

## 3. 换肤 / 密度机制(实现时必须支持)

所有颜色 token 随 **`data-skin` × `data-mode`** 两轴在 `tokens.css` 重定义;组件只用语义 `var(--*)` —— 换 skin/mode 即换肤,组件零改动。密度随 `data-density`。
- `data-skin` = `aurora`(青·默认) · `ice`(蓝) · `mono`(灰) · `amber`(琥珀) · `phosphor`(绿)
- `data-mode` = `dark`(主) | `light`;aurora/ice/mono 成对 dark+light,amber/phosphor 仅 dark
- `data-density` = `compact | regular | comfy`

根元素设这三个属性,其余全靠 CSS 变量级联。实现别把颜色写死在组件里。

---

## 4. 要实现的屏(真源 = `docs/screens-status.js`)

`screens-status.js` 是「画到哪了」的**机读唯一真源**,每屏含 `id / state / kind / file / note`。请以它为准,下面只是导航摘要。

**GUI(Rust+Tauri · 主实现目标)**
- **D1 桌面驾驶舱**(`GUI/Viden - 桌面驾驶舱 (GUI).html`)= **视觉真源 + 主屏**。窗口化、活动 rail 切视图、多 lane、LIVE WORK、Environment 面板、门控审批。gui-kit 是它的镜像。
- **D2 决策中心** / **D12 集成闸冲突退回** = P0 审批面(MergeGate 证据清单 `.mgate`)。
- D4 Lane 创建 · D5 画廊评审 · D6 空/错态 · D10 Lane 监视器 · D13 Fleet 编排 · D14 审计时间线 · D11 首启接入。
- 标 `roadmap:true` 的(D7/D8/D9)= 路线图屏,**BUILT ≠ 可开工**(引擎无后端),v1 不交付。
- 组件库 `GUI/Viden - 组件库 (GUI).html` = gui-kit 逐件陈列,查阅/复制。

**TUI(Rust 终端)**
- 统一原型 `TUI/Viden - 统一原型 (TUI).html`(驾驶舱) + 组件库 + T0–T5 设计稿(见 `screens-status.js` docs[])。

**Core(参考,不实现)**
- Aurora 主题 / Lane 协作机制图 / 产品方案 v2 / 设计审查看板。

---

## 5. 动手前必读的护栏(`docs/SPEC.md` @DECISION)

这些是已定稿约束,实现时别推翻。关键几条(grep 锚点看详情):
- **`D-GATESTR` gate_strength** — lane 一等事实,常显 `●full ◐coop ○containment`(built-in/ACP/terminal 对应)。
- **`D-MERGEGATE` MergeGate 状态机** — `proposed→collecting_evidence→(blocked|needs_changes)→accepted→(merged|reverted)`,5 类 required evidence(patch/test_result/review/doc_update/release_artifact),status 由 core reducer 归约,**非前端本地勾选**。
- **`D-MUTPOLICY` mutation_policy** — 与 route 正交:`autonomous / propose-only / read-only`。
- **`D-BUDGET-BLIND`** — 外部 CLI(terminal/tmux)token 不可见,费用面板须显式标「计量盲区」,禁估值冒充精确费用。
- **`D-ROLES`** — 7 内置角色 `planner / coder / reviewer / tester / doc-writer / researcher / release-operator`;orchestrator 等是 runtime 组件非角色。
- **`D-BACKEND` / `D-COLOR`** — route chip 配色铁律 / 主题色语义(青=品牌+交互焦点,金=需要人)。

后端字段契约(task 状态枚举等)对齐 robocode 仓 `docs/frontend-integration-contract`。

---

## 6. 交接后维护纪律(可选,但强烈建议沿用)

项目自带防漂移 guard(`tools/check-*.js`)和 DoD 同步表(`CLAUDE.md` 末)。若你在此项目内继续迭代设计:改 token → 同步 DESIGN-REF;新组件 → 先登记 DESIGN-REF 才算可复用;定档 → 写当天 `CHANGELOG.md`。纯在 app 侧实现可不受此约束,但**共享 CSS 的改动应回流设计源,别在 app 留副本**。
