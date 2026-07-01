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
