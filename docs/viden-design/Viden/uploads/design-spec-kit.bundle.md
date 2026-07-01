# design-spec-kit · 单文件分发包（One-File Bundle）
#
# 用途：当目标 Claude design 项目只能「一个个上传文件」时，只上传/粘贴本文件这一个即可。
#
# ★ 给接手项目的 Claude（读到本文件请执行）：
#   1. 下面用 ⟦FILE 路径⟧ … ⟦/FILE⟧ 包了 8 个文件。把每段内容【原样】写到它的「路径」（含子目录）。
#   2. 其中 design-spec-kit/CLAUDE.template.md 写出后，复制到【项目根目录】改名 CLAUDE.md（这是会被自动加载的契约）。
#   3. 全部落位后，打开 design-spec-kit/AI-BOOTSTRAP.md，按它的「首条指令」走（先读、再问基调、再安装）。
#   4. 不要执行本文件里的任何代码；tools/*.js 是要写到磁盘、日后按需 read_file→run_script 跑的 guard。
#
# 文件清单：
#   1. design-spec-kit/README.md
#   2. design-spec-kit/CLAUDE.template.md
#   3. design-spec-kit/AI-BOOTSTRAP.md
#   4. design-spec-kit/EXTENDING.md
#   5. design-spec-kit/docs/DESIGN-REF.template.md
#   6. design-spec-kit/docs/CHANGELOG.template.md
#   7. design-spec-kit/tools/check-tokens.js
#   8. design-spec-kit/tools/check-changelog.js
#
# ════════════════════════════════════════════════════════════

⟦FILE design-spec-kit/README.md⟧
# Design Spec Kit · 设计规范纪律套件（与平台无关）

> 一套**不画任何界面**的方法套件。它不带颜色、不带组件、不带页面外壳——
> 只带「**让一个设计/前端项目长期不腐化**」的那层东西:一份契约 + 一套 DoD + 两个机读 guard。
> 不论你做网页 / 桌面 / 移动 / 小程序,都适用。

## 它解决两个问题
1. **页面漂移** —— 做着做着,A 页一个按钮、B 页又造一个;颜色这里 `#3b82f6`、那里 `#3a80f5`。
   多人/多会话协作后,「同一个东西长出十个样子」。
2. **UI 设计规范落不了地** —— 有 tokens、有组件库,但没人强制用;规范文档和真实代码越走越远。

治法很朴素:**单一真源 + 准入纪律 + 可机检的 DoD**。把「自觉」变成「会报错」。

---

## 套件里有什么(6 个文件,零 HTML/CSS)
```
design-spec-kit/
├─ README.md                    ← 本文件:这是什么 + 怎么用
├─ CLAUDE.template.md           ← 契约骨架 ★核心。复制到项目根改名 CLAUDE.md
├─ AI-BOOTSTRAP.md              ← 粘给接手 AI 的「首条指令」（自我引导安装 + 立规矩）
├─ EXTENDING.md                 ← 壳接入契约（移动/桌面/Web 等平台壳怎么叠上来）
├─ docs/
│  ├─ DESIGN-REF.template.md    ← 组件 + token 索引骨架(可复用件的「户口」)
│  └─ CHANGELOG.template.md     ← 更新日志骨架(按天 + 模块标签)
└─ tools/
   ├─ check-tokens.js           ← guard①:禁裸 hex/rgba/假 fallback —— 直接防视觉漂移
   └─ check-changelog.js        ← guard②:CHANGELOG 卫生(同日合并/长度/深度)
```

> ★ `CLAUDE.md` 是每个会话**自动加载**的项目说明。它是让「每个 Claude / 协作者」都守同一套纪律的总开关——套件的心脏在这。

---

## 怎么用(START HERE · 零猜测)
1. **立契约** —— 把 `CLAUDE.template.md` 复制到**新项目根目录**,改名 `CLAUDE.md`,把所有〈尖括号〉替换为真实内容,删掉不适用的小节。
2. **搬文档** —— `docs/DESIGN-REF.template.md` / `CHANGELOG.template.md` 复制进项目 `docs/`,去掉 `.template`。
3. **搬 guard** —— `tools/` 两个脚本原样复制进项目 `tools/`。
4. **建 token 真源** —— 在你项目里建 `tokens.css`(放哪都行),所有颜色/字号/间距/圆角/阴影定义在这一处,组件**只引用 `var(--*)`**。这是「单一真源」的物理落点。
5. **画屏/写组件** —— 造任何 UI 元素前先 `grep` 一下 `DESIGN-REF.md`:命中就抄类名直接用,没命中再造、造完登记。**没登记的组件 = 临时草稿。**
6. **`done` 前跑 guard** —— 见下。

---

## 两个 guard 怎么跑
> 都是纯只读脚本,用本环境的 `run_script`:**`read_file` 脚本全文 → 整段粘进 `run_script` 执行**。末行打印 `RESULT: PASS|FAIL`。

| guard | 守什么 | 首次跑 | 之后 |
|---|---|---|---|
| `check-tokens.js` | 代码里有没有裸 `#hex` / `rgba()` / 假 `var(--x,#fff)` fallback | 自动生成 baseline(接受现状) | 只报**新增**违规,FAIL 就修或收编进 tokens.css |
| `check-changelog.js` | CHANGELOG 同日是否开了两段、是否超长、单条是否过深 | 直接扫 | 同上,HARD FAIL = 重复同日段 |

**接手第一件事**:打开 `tools/check-tokens.js` 顶部 `SCAN_ROOTS`,改成你项目放样式/页面的目录(默认已列了 `styles/css/src/components/pages` 等常见名,不存在的会自动跳过,通常不用改)。

---

## 一句话边界
- **它管**:纪律、单一真源、可机检的 DoD、组件准入。
- **它不管**:你长什么样(颜色/字体/组件/布局全是你的)、用什么框架、做什么平台。
- **要做移动 App 原型**(iPhone 框 + 画布 + iOS chrome + 路由栈)?那是**另一个可选包**,跟本套件解耦——本套件是它的「方法底座」,但不依赖它。**任何平台壳(移动/桌面/Web)怎么叠到本底座上,见 [`EXTENDING.md`](EXTENDING.md)。**

> 核心理念:**任何影响产物的改动都带一个同步义务**。漏一项 = 漂移。把同步义务写进 `CLAUDE.md` 的「收尾同步表」,再用 guard 守住能机检的那几条。

⟦/FILE⟧
⟦FILE design-spec-kit/CLAUDE.template.md⟧
# 〈项目名〉— 项目说明（CLAUDE.md）

> 本文件随 **design-spec-kit** 提供,是**项目说明 + 文档体系 + 工作纪律**的骨架(与平台无关)。
> 复制到新项目根目录后改名为 `CLAUDE.md`,把所有〈尖括号占位〉替换为真实内容,删掉本引言与不适用的小节。
> CLAUDE.md 必须放在**项目根目录**(会被每个会话自动加载);其余文档统一收进 `docs/`。

## 产品
**〈产品名〉** 是〈一句话定位:平台 / 关键特性 / 对标对象〉。
技术栈:〈框架 / 语言 / 数据层〉。〈当前阶段说明,如「UI 属新建阶段」〉。

核心场景:
- **〈场景一〉**:〈说明〉。
- **〈场景二〉**:〈说明〉。

## 设计基调（与用户确认后填写）
- 气质:〈如 温润 / 克制 / 锐利 / 活泼〉,一个明确方向。
- 模式:〈浅色 / 深色 / 双模式〉。
- 排版:〈正文字体 + UI 字体 + 语言主次〉。
- 主题色:〈单套 or 多套可切换;每套包含哪些角色色 accent / strong / soft / ink / on-accent〉。
- 平台与密度:〈移动 / 桌面 / 响应式;信息密度高低〉。

## 设计 Token（单一真源）
所有 token 定义在 `〈你的 tokens.css 路径〉`,是**唯一真源**。
- **切勿凭空发明颜色 / 字号 / 间距**,一律引用 `var(--*)`。裸 `#hex` / `rgba()` 由 `check-tokens.js` 拦截。
- 间距 4px 基准(`--sp-*`),圆角 `--r-*`,阴影 `--shadow-*`,字体 `--font-*`。
- 〈若多主题/明暗:中性色随 `data-mode`,强调色随 `data-theme + data-mode`——组件只用 `var(--*)`,换 token 即换肤。〉

## 交付物与文档
- `〈设计规范展示页〉` —— 给人看的展示文档(色板 / 字阶 / 组件总览),〈含主题/明暗切换,可选〉。
- `docs/DESIGN-REF.md` —— **AI / 开发速查手册**(机器友好):token 全表 + 组件目录(类名 + 最小 HTML)。**复用组件前先读它。**
- `docs/CHANGELOG.md` —— 更新日志(按天 + 模块标签)。
- 项目文档统一收纳在 `docs/`(CLAUDE.md 因需置于根目录而保留在根)。

## 约定
- 新设计 / 组件一律遵循上述 token 与基调,**保持克制**(少即是多,避免无意义的数字、图标、渐变堆砌)。
- 〈语言 / 排版细则,如 CJK 正文行高 1.55–1.7〉。
- 〈平台细则,如移动端点击目标 ≥ 44px——按你的平台填,非移动可删。〉

## 工作纪律（来自 design-spec-kit · 换项目仍成立）
- **先 grep 再写**:造任何 UI 元素前先读 `DESIGN-REF.md` / grep 现有 class——命中就抄类名直接用,**别重造已沉淀的组件**。这是防「页面漂移」的第一道闸。
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
| 任意定档 | 写当天 `CHANGELOG.md`(先 grep 同日段,命中即 append;最新日期段置顶) | 🤖 `check-changelog.js` |
| 〈接了某平台外壳 / 模块,按需追加行〉 | 〈对应同步义务〉 | 〈人 / guard〉 |

⟦/FILE⟧

⟦FILE design-spec-kit/AI-BOOTSTRAP.md⟧
# 给 AI 的首条指令（粘这一段给接手项目的 Claude）

> 用法:把下面 `===` 之间的整段,连同 `design-spec-kit/` 文件夹一起交给新项目的 Claude(或粘进对方项目的第一条消息)。它会自我引导完成安装 + 立规矩。

===

我给你一套 **design-spec-kit**(在项目里的 `design-spec-kit/` 文件夹)。它是一套**与平台无关的设计纪律套件**,目的是让这个项目长期**不发生页面漂移、UI 规范能落地**。请你按下面步骤接管它,做完跟我确认。

**第一步 · 先读不动手**
读这三份,建立认知,先别改任何东西:
- `design-spec-kit/README.md`(这是什么 + 怎么用)
- `design-spec-kit/CLAUDE.template.md`(契约骨架)
- `design-spec-kit/docs/DESIGN-REF.template.md`(组件/token 索引骨架)

**第二步 · 跟我对齐基调(提问,别自己定)**
用一轮问题跟我确认,把答案准备好填进 CLAUDE.md:产品一句话定位、技术栈、气质方向(温润/克制/锐利/活泼…)、浅色/深色/双模式、正文+UI 字体、主题色(单套还是多套、各含哪些角色色)、平台与信息密度。**没对齐前不要建 token、不要画屏。**

**第三步 · 安装套件(落地到项目)**
- 把 `design-spec-kit/CLAUDE.template.md` 复制到**项目根目录**,改名 `CLAUDE.md`,用第二步的答案替换所有〈尖括号〉,删掉不适用的小节。
- 把 `design-spec-kit/docs/` 两个模板复制进项目 `docs/`,去掉 `.template` 后缀。
- 把 `design-spec-kit/tools/` 两个脚本原样复制进项目 `tools/`。
- 打开 `tools/check-tokens.js` 顶部 `SCAN_ROOTS`,确认覆盖了本项目放样式/页面的目录(默认已列常见目录名,不存在的会自动跳过)。

**第四步 · 建设计真源(token 先行)**
建一个 `tokens.css`(放哪都行,跟 CLAUDE.md 里写的路径一致):把第二步定的颜色/字号/间距/圆角/阴影**全部定义在这一处**,用 `:root` 变量。之后所有组件**只引用 `var(--*)`**,绝不写裸 `#hex`/`rgba()`。同步在 `docs/DESIGN-REF.md` 的 Token 速查表登记。

**第五步 · 此后每次干活都守这三条纪律**
1. **先 grep 再写**:造任何 UI 元素前先读/grep `docs/DESIGN-REF.md`——命中就抄类名直接用,**别重造已有组件**;没命中再造,造完**立刻登记**(类名 + 最小 HTML)。没登记的组件 = 临时草稿,不算可复用。
2. **单一真源**:数值只在 `tokens.css`;DESIGN-REF 只做索引,冲突以 tokens.css 为准。
3. **定档即写 CHANGELOG**:每定稿一件事,在 `docs/CHANGELOG.md` 当天段落 append 一条 `- [模块] 描述`(先 grep 同日段,绝不开第二个同日段)。

**第六步 · 每次 `done` 前跑 DoD guard**
- `read_file` 读 `tools/check-tokens.js` 全文 → 整段粘进 `run_script` 执行;看末行 `RESULT`。首跑会自动建 baseline,之后只报**新增**违规,`FAIL` 就把颜色收编进 tokens.css(或确需保留则按脚本提示写 baseline 并在 CHANGELOG 注明)。
- 同样跑 `tools/check-changelog.js`,确保没有重复同日段、没超长、单条不过深。

请先做第一、二步:读完三份文件,然后开始问我基调问题。**不要跳过提问直接安装。**

===

⟦/FILE⟧

⟦FILE design-spec-kit/EXTENDING.md⟧
# 扩展:给底座叠一层「平台壳」（EXTENDING）

> design-spec-kit 是**方法底座**(契约 + DoD + token 纪律),它**不规定你怎么呈现界面**。
> 「壳」= 一层可插拔的呈现方案:移动 App 原型(iPhone 框 + 画布 + iOS chrome + 路由栈)、桌面窗口、Web 多栏……
> 底座对壳一无所知;**壳单向依赖底座**。本文讲一个壳怎么干净地接进来,以及怎么自己造一个新壳。

---

## 一句话扩展契约
> 一个「壳」= **消费底座的 token 真源 + 自带平台 DoD 行(可选 guard)+ 自带平台 CLAUDE 小节与架构 doc**。
> 装/卸一个壳 = 加/减下面三块。底座永不依赖壳。

## 接入只有三个挂钩点

### ① 共用同一个 token 真源(不复制)
壳里的屏 `link` 的是底座管的那份 `tokens.css` + 组件 CSS,全走 `var(--*)`。换 token 自动换肤。
- **铁律:壳不带自己的颜色。** 壳自带的占位 token 仅供它独立 demo 跑;真接进项目就指向项目的 `tokens.css`。
- 把壳的目录(如 `mobile-shell/`、`pages/`)加进 `tools/check-tokens.js` 的 `SCAN_ROOTS`——**漂移防线自动覆盖到壳**,壳里冒出裸 hex 照样 FAIL。

### ② 往 DoD 表追加平台行 +(可选)平台 guard
`CLAUDE.template.md` 的收尾同步表最后一行是预留扩展位:
```
| 〈接了某平台外壳 / 模块,按需追加行〉 | 〈对应同步义务〉 | 〈人 / guard〉 |
```
比如接移动壳时实化成:

| 改了 | 必做 | 谁来守 |
|---|---|---|
| 加 / 删 / 改屏 | 同步壳的屏清单(如 `PROTO_CONFIG.screens`);必要时更新架构 doc | 人 |
| 改了外壳机制(路由 / 转场 / 画布) | 跑壳自带的 `check-kit-drift.js` 守外壳同源 | 🤖 |

> `check-kit-drift.js` 是**壳专属的第三个 guard**,只在「复制式复用了壳」的项目里需要。
> ⚠ 若项目是**引用**壳(屏直接 `link ../<壳>/assets/*`、不复制),就**没有副本→没有副本漂移**,这个 guard 自动退役——底座的两个 guard 仍照常守。

### ③ 往 CLAUDE.md 补一节平台纪律 + 一份架构 doc
壳把自己的「别自造清单」(如 iOS chrome / `data-nav` / 底部弹层 / 画布外壳都现成)补进 CLAUDE.md 的工作纪律,并把它的架构说明(如 `PROTOTYPE-ARCH.md`)放进 `docs/`。
- 这些平台专属内容**只在装了壳时才出现**,不污染底座。卸壳 = 删这一节 + 删 doc + 删 DoD 平台行。

---

## 多壳并存
一个项目可以同时挂多个壳(如 `mobile-shell/` + `desktop-shell/`):它们**共用同一份 `tokens.css` + `DESIGN-REF.md`**,只是「怎么摆」不同。底座保证它们说的是同一套设计语言,壳只负责各自平台的呈现与导航。

## 造一个新壳的最小清单
1. 壳目录里所有 CSS/组件**只用 `var(--*)`**,自带一份占位 `tokens.css` 仅供独立 demo。
2. 写一份壳 README:它解决什么平台、屏怎么登记、有哪些现成能力(别让人重画)。
3. 若是复制式复用,带一个 `check-kit-drift.js`;若是引用式,不需要。
4. 给出要追加到底座的:DoD 平台行 + CLAUDE.md 平台小节 + 架构 doc。
5. 确认壳目录已进 `check-tokens.js` 的 `SCAN_ROOTS`——纳入漂移防线。

> 核心:壳负责「怎么呈现」,底座负责「不腐化」。两者通过 token 真源 + DoD 表 + CLAUDE 小节这三个挂钩点对接,各自可独立替换。

⟦/FILE⟧

⟦FILE design-spec-kit/docs/DESIGN-REF.template.md⟧
# 〈项目名〉设计规范 · AI 速查手册（DESIGN-REF）

> 本文件是**给 AI / 开发快速复用的索引**,不是给人看的展示文档(展示见〈设计规范展示页〉)。
> **复用任何组件前先读本文件**:直接抄类名与最小 HTML 片段,不必重读 CSS。
> 黄金规则:**只引用 `var(--*)`,绝不写死颜色 / 字号 / 间距**;改完若定档,按 `CHANGELOG.md` 规矩记录。

## 文件结构（按你的项目填实）
```
项目根/
├── CLAUDE.md             # 项目说明（必须在根目录，自动加载）
├── docs/
│   ├── DESIGN-REF.md     # 本文件 · AI 速查
│   └── CHANGELOG.md      # 更新日志（按天 + 模块标签）
├── tools/
│   ├── check-tokens.js
│   └── check-changelog.js
└── 〈样式目录〉/
    ├── tokens.css        # 所有设计变量（唯一真源）
    └── 〈组件样式〉.css    # 布局 + 组件样式（只用 var(--*)）
```

## Token 速查
> 数值唯一真源在 `tokens.css`;下表只做语义索引。改了 tokens.css 必须同步本表。
> 下面是**建议骨架**,按你的实际 token 增删。

### 颜色（中性 · 〈随 data-mode,可选〉）
| token | 语义 |
|---|---|
| `--bg` / `--bg-2` | 页面底 / 次级底 |
| `--surface` / `--surface-2` | 卡片面 / 次级面 |
| `--ink` / `--ink-2` / `--ink-3` | 正文 / 次要 / 占位 |
| `--hairline` | 分隔线 |

### 颜色（强调 · 〈随 data-theme + data-mode,可选〉）
| token | 语义 |
|---|---|
| `--accent` | 主强调(按钮 / 选中 / 链接) |
| `--accent-strong` | 加重强调 |
| `--accent-soft` | 浅底 |
| `--on-accent` | 实色强调上的文字 |

### 字体 / 间距 / 圆角 / 阴影
| 类别 | token | 说明 |
|---|---|---|
| 字体 | `--font-sans` / `--font-mono` | UI / 等宽 |
| 间距 | `--sp-1`…`--sp-16` | 4px 基准 |
| 圆角 | `--r-xs`…`--r-xl` / `--r-full` | |
| 阴影 | `--shadow-sm` / `--shadow-md` / `--shadow-lg` | |

## 组件目录
> 每个可复用组件一条:类名 + 一句用途 + 最小 HTML。**没登记的组件视为临时草稿。**

### 〈组件名示例:按钮〉
〈用途〉
```html
<button class="〈类名〉">…</button>
```

<!-- 按此格式继续追加组件。新增组件先在此登记，再写进 CHANGELOG。 -->

## 图标与贡献约定
- 图标:〈线性 / 填充、stroke-width、来源库〉。
- 新增组件先在本目录登记(类名 + 最小 HTML),再写进 CHANGELOG。

⟦/FILE⟧

⟦FILE design-spec-kit/docs/CHANGELOG.template.md⟧
# 〈项目名〉更新日志（Changelog）

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
- 〈按你的项目追加模块标签〉

---

## 〈YYYY-MM-DD〉
- [文档] 从 design-spec-kit 起步,立 CLAUDE.md 契约 + DESIGN-REF 索引 + DoD guard。

<!-- 新的一天在「模块索引」下方、本段之上开新 `## YYYY-MM-DD`（newest-first）。 -->

⟦/FILE⟧

⟦FILE design-spec-kit/tools/check-tokens.js⟧
/**
 * check-tokens.js · token 纪律防漂移扫描（design-spec-kit · 项目通用）
 *
 * ★ 拿到本脚本：按你的目录调下方 SCAN_ROOTS（默认已列常见样式/页面目录，不存在的自动跳过）。
 *   首次跑会自动生成 baseline（接受现状），之后只报新增违规。
 *
 * 用途：扫描项目里的 .css/.js/.jsx/.ts/.tsx/.html/.vue，找违反 token 纪律的代码：
 *   ❌ 裸 hex          `#abc` / `#abcdef` / `#abcdef88`
 *   ❌ 裸 rgba         `rgba(0,0,0,.5)`
 *   ❌ 假 fallback     `var(--x, #fff)` / `var(--x, rgba(...))`
 *                     （允许 `var(--x, var(--y))` token→token fallback）
 *
 * CLAUDE.md 约定：颜色一律 `var(--*)`，数值唯一真源在 tokens.css。
 * 本脚本把这条「自觉纪律」变成可机检的 DoD 守卫——直接挡住「页面漂移」里最常见的一种：
 * 这页 #3b82f6、那页 #3a80f5，颜色越走越散。
 *
 * ─────────────────────────────────────────────────────────────
 *  扫哪些 / 跳哪些
 * ─────────────────────────────────────────────────────────────
 *  自动遍历 SCAN_ROOTS（不硬编码文件名 → 新增页不漏扫、不需维护清单）。
 *  跳过：
 *    · 任何 `tokens.css`  —— token 唯一真源，hex/rgba 合法定义于此
 *    · SKIP_DIRS 里的目录 —— 依赖 / 构建产物 / 归档 / 工具本身
 *    · 非代码文件         —— 按扩展名只收下面 CODE_EXT
 *
 * ─────────────────────────────────────────────────────────────
 *  baseline 机制
 * ─────────────────────────────────────────────────────────────
 *  tools/check-tokens.baseline.json 列出「已认证保留」的违规快照。
 *  脚本只报增量：清掉旧违规 = OK / 新增违规 = FAIL。
 *  要把当前所有违规重新固化为 baseline，把下方 args 设成 ['--write-baseline']。
 *
 * ─────────────────────────────────────────────────────────────
 *  怎么跑：read_file 本文件 → 整个粘到 run_script。
 *  只用沙箱 helper：readFile / saveFile / ls / log。末行 `RESULT: PASS|FAIL`。
 * ═════════════════════════════════════════════════════════════*/

// ─── 配置（接手第一件事：按你的项目改这里）──────────────────────

const args = [];   // 例：['--write-baseline'] 把当前扫描结果固化为新 baseline

// 放样式 / 组件 / 页面的目录。多列无妨——不存在的目录会被自动跳过。
const SCAN_ROOTS = ['styles', 'css', 'src', 'components', 'pages', 'app', 'design-system'];
const ROOT_FILES = ['index.html'];          // 项目根的散件
const CODE_EXT   = /\.(css|scss|less|js|jsx|ts|tsx|vue|svelte|html)$/i;

// 整目录级 skip（依赖 / 构建产物 / 归档 / 工具 / 版本库）
const SKIP_DIRS = new Set(['node_modules', 'dist', 'build', '.git', '_archive', 'tools', 'uploads', 'vendor']);
// 整文件级 skip：token 唯一真源（hex/rgba 合法定义于此）
const isSkipFile = p => /(^|\/)tokens\.css$/i.test(p);

const BASELINE_PATH = 'tools/check-tokens.baseline.json';

// ─── 规则 ──────────────────────────────────────────────────────

// 单一组合 regex，按 alternative 顺序匹配：fake-fallback 优先 → 裸 hex/rgba 兜底
const RE = /var\(\s*--[a-z0-9-]+\s*,\s*#[0-9A-Fa-f]{3,8}\s*\)|var\(\s*--[a-z0-9-]+\s*,\s*rgba?\([^)]*\)\s*\)|#[0-9A-Fa-f]{3,8}\b|\brgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*(?:,\s*[\d.]+\s*)?\)/gi;

function classify(m) {
  if (m.startsWith('var(')) return m.includes('#') ? 'fake-fallback-hex' : 'fake-fallback-rgba';
  return m.startsWith('#') ? 'bare-hex' : 'bare-rgba';
}

// 用空格替换注释内容（保留位置，方便行号反查）
const stripCss  = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length));
const stripHtml = s => s.replace(/<!--[\s\S]*?-->/g, m => ' '.repeat(m.length));
const stripJs   = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length))
                       .replace(/\/\/[^\n]*/g, m => ' '.repeat(m.length));
const extOf  = p => p.slice(p.lastIndexOf('.')).toLowerCase();
const strip  = (s, ext) => ext === '.css' || ext === '.scss' || ext === '.less' ? stripCss(s)
                         : ext === '.html' || ext === '.vue' || ext === '.svelte' ? stripHtml(s)
                         : stripJs(s);

function lineOf(src, idx) {
  let l = 1;
  for (let i = 0; i < idx; i++) if (src.charCodeAt(i) === 10) l++;
  return l;
}

// ─── 收集文件（递归遍历 SCAN_ROOTS）─────────────────────────────

async function walk(dir, out) {
  let entries;
  try { entries = await ls(dir); } catch { return; }
  if (!entries || entries.length === 0) return;   // 文件 ls → []，自然终止
  for (const name of entries) {
    const path = dir ? dir + '/' + name : name;
    if (CODE_EXT.test(name)) {
      if (!isSkipFile(path)) out.push(path);
    } else if (!name.includes('.') && !SKIP_DIRS.has(name)) {
      // 无扩展名 → 当目录递归（dotfiles / 图片等被扩展名过滤天然排除）
      await walk(path, out);
    }
  }
}

async function collectFiles() {
  const out = [];
  for (const r of SCAN_ROOTS) await walk(r, out);
  for (const f of ROOT_FILES) if (!isSkipFile(f)) out.push(f);
  return [...new Set(out)];   // SCAN_ROOTS 可能重叠，去重
}

// ─── 扫描 ──────────────────────────────────────────────────────

const PARALLEL_BATCH = 24;

async function scanAll(files) {
  const allHits = [];
  for (let i = 0; i < files.length; i += PARALLEL_BATCH) {
    const batch = files.slice(i, i + PARALLEL_BATCH);
    const contents = await Promise.all(batch.map(async f => {
      try { return { f, src: await readFile(f) }; }
      catch { return { f, src: null }; }
    }));
    for (const { f, src } of contents) {
      if (!src) continue;
      const ext = extOf(f);
      const cleaned = strip(src, ext);
      let m; RE.lastIndex = 0;
      while ((m = RE.exec(cleaned)) !== null) {
        allHits.push({ file: f, line: lineOf(src, m.index), kind: classify(m[0]), match: m[0] });
      }
    }
  }
  return allHits;
}

// ─── Baseline diff ─────────────────────────────────────────────

function keyOf(h) { return `${h.file}::${h.kind}::${h.match}`; }

function baselineKeys(b) {
  const s = new Set();
  if (!b || !b.files) return s;
  for (const [f, arr] of Object.entries(b.files)) {
    for (const e of arr) s.add(`${f}::${e.kind}::${e.match}`);
  }
  return s;
}

function buildBaseline(hits, reason) {
  const grouped = {};
  for (const h of hits) (grouped[h.file] = grouped[h.file] || []).push({
    line: h.line, kind: h.kind, match: h.match
  });
  for (const f of Object.keys(grouped)) {
    grouped[f].sort((a, b) => a.line - b.line || a.match.localeCompare(b.match));
  }
  return {
    note: '已认证保留的 token 违规清单。新增违规需修代码或显式加到这里。',
    generatedAt: new Date().toISOString().slice(0, 10),
    reason: reason || 'baseline write',
    totalEntries: hits.length,
    files: grouped,
  };
}

// ─── Main（top-level await — run_script 直接执行）──────────────

const writeBaseline = args.includes('--write-baseline');

const files = await collectFiles();
const hits = await scanAll(files);
log(`scanned ${files.length} files · ${hits.length} violations`);

if (writeBaseline) {
  await saveFile(BASELINE_PATH, JSON.stringify(buildBaseline(hits, 'manual --write-baseline'), null, 2) + '\n');
  log(`✓ baseline rewritten: ${BASELINE_PATH} (${hits.length} entries)`);
} else {
  let baseline = null;
  try { baseline = JSON.parse(await readFile(BASELINE_PATH)); } catch { /* no baseline */ }

  if (!baseline) {
    await saveFile(BASELINE_PATH, JSON.stringify(buildBaseline(hits, 'first run'), null, 2) + '\n');
    log(`✓ baseline created: ${BASELINE_PATH} (${hits.length} entries) — 复查后再跑一次进入 diff 模式`);
  } else {
    const allowed = baselineKeys(baseline);
    const news    = hits.filter(h => !allowed.has(keyOf(h)));
    const removed = [...allowed].filter(k => !hits.some(h => keyOf(h) === k));

    log(`baseline: ${allowed.size} entries · removed: ${removed.length} · new: ${news.length}`);

    if (removed.length > 0) {
      log(`\n✓ ${removed.length} 处 baseline 违规已被清理（干得漂亮）`);
      for (const k of removed.slice(0, 20)) log('    cleaned: ' + k);
      if (removed.length > 20) log(`    ... 还有 ${removed.length - 20} 处`);
      log(`  → 跑一次 args=['--write-baseline'] 同步 baseline\n`);
    }

    if (news.length > 0) {
      log(`\n✗ ${news.length} 处新增违规：`);
      const byFile = {};
      for (const h of news) (byFile[h.file] = byFile[h.file] || []).push(h);
      for (const [f, arr] of Object.entries(byFile)) {
        log(`  ${f}`);
        for (const h of arr) log(`    L${h.line}  [${h.kind}]  ${h.match}`);
      }
      log(`\n修法：`);
      log(`  1. 优先把 hex / rgba 收编进 tokens.css（推荐）`);
      log(`  2. 确实必须保留：args=['--write-baseline'] 并在 CHANGELOG 写明理由`);
      log(`\nRESULT: FAIL`);
    } else if (removed.length === 0) {
      log('✓ check-tokens: 0 新增 · 0 减少 · baseline 保持不变');
      log(`\nRESULT: PASS`);
    } else {
      log(`\nRESULT: PASS`);
    }
  }
}

⟦/FILE⟧

⟦FILE design-spec-kit/tools/check-changelog.js⟧
/**
 * check-changelog.js · CHANGELOG 卫生防漂移扫描（design-spec-kit · 项目通用）
 *
 * 把 CLAUDE.md「Changelog 维护」里可机判的三条约定变成 DoD 守卫
 * （改 docs/CHANGELOG.md 后必跑）：
 *
 *   ❌ HARD FAIL  同一日期出现 >1 个 `## YYYY-MM-DD` 段
 *                 （硬规则：写前先 grep 同日段、命中就 append，绝不新开第二段）
 *   ⚠  WARN       文件总行数 > WARN_LINES → 把窗口外早期整段移到
 *                 docs/_archive/CHANGELOG-YYYY-MM.md（主文件留最近约 2 个会话日）
 *   ⚠  WARN       单条目子 bullet > MAX_SUB → 验尸报告化，细节该分流到
 *                 docs/ 下对应 doc（只点名，不 fail）
 *
 * ─────────────────────────────────────────────────────────────
 *  怎么跑：read_file 本文件 → 整个粘到 run_script。
 *  只用沙箱 helper：readFile / log。无 baseline、无写盘（纯只读扫描）。
 *  退出语义：有 HARD FAIL 时末行打印 `RESULT: FAIL`，否则 `RESULT: PASS`。
 * ═════════════════════════════════════════════════════════════*/

// ─── 配置 ──────────────────────────────────────────────────────

const CHANGELOG_PATH = 'docs/CHANGELOG.md';   // ← 你的 CHANGELOG 路径
const WARN_LINES = 200;   // 超过此行数 → 提示归档（CLAUDE.md：留最近 ~2 会话日 / 超 ~200 行归档）
const MAX_SUB    = 3;     // 单条目允许的子 bullet 上限（深度上限：1 行标题 + 最多 3 子 bullet）

// ─── 解析 ──────────────────────────────────────────────────────

const RE_DATE_H  = /^##\s+(\d{4}-\d{2}-\d{2})\b/;   // 真实日期段（## YYYY-MM-DD）
const RE_ANY_H2  = /^##\s+/;                         // 任意 H2（模块索引 / 约定段等）
const RE_TOP_LI  = /^-\s+\S/;                        // 顶层条目（- 开头）
const RE_SUB_LI  = /^\s+-\s+\S/;                     // 子 bullet（缩进 - 开头）

const src = await readFile(CHANGELOG_PATH);
const lines = src.split('\n');
const totalLines = lines.length;

// 1) 重复同日段
const dateHits = {};   // date -> [lineNo,...]
lines.forEach((ln, i) => {
  const m = ln.match(RE_DATE_H);
  if (m) (dateHits[m[1]] = dateHits[m[1]] || []).push(i + 1);
});
const dupDates = Object.entries(dateHits).filter(([, ls]) => ls.length > 1);

// 2) 条目深度：只扫真实日期段内的条目
const entries = [];   // {date, line, title, subCount}
let inDated = false, curDate = null, cur = null;
const pushCur = () => { if (cur) { entries.push(cur); cur = null; } };
lines.forEach((ln, i) => {
  const dm = ln.match(RE_DATE_H);
  if (dm) { pushCur(); inDated = true; curDate = dm[1]; return; }
  if (RE_ANY_H2.test(ln)) { pushCur(); inDated = false; curDate = null; return; }  // 非日期 H2（模块索引 / 约定段）
  if (!inDated) return;
  if (RE_TOP_LI.test(ln)) {
    pushCur();
    cur = { date: curDate, line: i + 1, title: ln.replace(/^-\s+/, '').replace(/\*\*/g, '').slice(0, 64), subCount: 0 };
  } else if (cur && RE_SUB_LI.test(ln)) {
    cur.subCount++;
  }
});
pushCur();

const fatEntries = entries.filter(e => e.subCount > MAX_SUB).sort((a, b) => b.subCount - a.subCount);

// ─── 报告 ──────────────────────────────────────────────────────

let fail = false;
log(`scanned ${CHANGELOG_PATH} · ${totalLines} 行 · ${entries.length} 条目 · ${Object.keys(dateHits).length} 个日期段`);

// 重复同日段（HARD FAIL）
if (dupDates.length > 0) {
  fail = true;
  log(`\n✗ ${dupDates.length} 个日期出现重复 \`## YYYY-MM-DD\` 段（硬规则：同日只能一段）：`);
  for (const [d, ls] of dupDates) log(`    ${d}  ×${ls.length}  → 行 ${ls.join(', ')}`);
  log(`  修法：把这些段的条目合并到第一段（最上方那个），删掉多余 \`## ${dupDates[0][0]}\` 标题与其间的 \`---\` 分隔。`);
} else {
  log('✓ 同日合并：每个日期仅一段');
}

// 文件超长（WARN）
if (totalLines > WARN_LINES) {
  log(`\n⚠ 文件 ${totalLines} 行 > ${WARN_LINES} 行阈值 → 建议归档`);
  const dates = Object.keys(dateHits).sort();
  log(`    当前日期段（旧→新）：${dates.join(' · ')}`);
  log(`    把最旧的几段整段移到 docs/_archive/CHANGELOG-YYYY-MM.md（原样保真），主文件留最近约 2 个会话日 + 底部「更早条目」链接。`);
} else {
  log(`✓ 文件长度：${totalLines} 行（≤ ${WARN_LINES}）`);
}

// 条目深度（WARN，只点名）
if (fatEntries.length > 0) {
  log(`\n⚠ ${fatEntries.length} 条条目子 bullet > ${MAX_SUB}（验尸报告化，细节该分流到 docs/）：`);
  for (const e of fatEntries.slice(0, 8)) log(`    L${e.line}  [${e.date}]  ${e.subCount} bullets  ·  ${e.title}…`);
  if (fatEntries.length > 8) log(`    …还有 ${fatEntries.length - 8} 条`);
  log(`    深内容指向 DESIGN-REF.md 等 doc，条目里只留一句话 + 指路。`);
} else {
  log(`✓ 条目深度：均 ≤ ${MAX_SUB} 子 bullet`);
}

log(`\nRESULT: ${fail ? 'FAIL' : 'PASS'}`);

⟦/FILE⟧
