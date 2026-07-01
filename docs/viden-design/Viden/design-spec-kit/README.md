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
