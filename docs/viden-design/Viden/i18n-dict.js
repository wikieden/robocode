/* ============================================================
   Viden · 集中 i18n 词典【共享文案唯一真源】
   ------------------------------------------------------------
   跨页复用的 chrome / 通用术语只在这里定义一次，按 key 取词，
   避免「同一个词在每页各翻一遍 → 越走越散」（审查看板 A-1 语言不统一）。

   用法（页面 <head>，必须在 i18n.js 之前）：
     <script src="i18n-dict.js"></script>   <!-- 子目录用 ../i18n-dict.js -->
     <script src="i18n.js"></script>
   取词：
     · HTML：<span data-i18n-key="open"></span>  → i18n.js 挂载时自动填入
     · JS  ：window.tk('open')                    → 返回当前语言字符串
   页面专属、一次性的长句仍可用 window.t('English','中文') 内联（向后兼容）。

   ★ 新增共享词条加在这里（不要散落各页）；只放真正跨页复用的术语，
     页面独有文案别塞进来（保持词典精简）。
   ============================================================ */
window.VIDEN_DICT = {
  /* —— 通用动作 / 导航 —— */
  open:        { en: "Open →",            zh: "打开 →" },
  back:        { en: "← Back",            zh: "← 返回" },
  backIndex:   { en: "← Prototype index", zh: "← 原型总览" },
  openNew:     { en: "Open in new tab ↗", zh: "新标签打开 ↗" },
  loading:     { en: "Loading…",          zh: "加载中……" },

  /* —— 屏幕状态枚举（screens-status.js · 门户徽标） —— */
  st_PLANNED:  { en: "PLANNED",   zh: "规划中" },
  st_WIP:      { en: "WIP",       zh: "在画" },
  st_BUILT:    { en: "BUILT",     zh: "已建" },
  st_REVIEWED: { en: "REVIEWED",  zh: "已评审" },
  st_ARCHIVED: { en: "ARCHIVED",  zh: "存档" },
  st_DROPPED:  { en: "DROPPED",   zh: "已废弃" },

  /* —— 构建载体 kind（门户卡片右上角徽标） —— */
  kind_kit:    { en: "KIT",            zh: "已接套件" },
  kind_inline: { en: "INLINE",         zh: "内联待迁移" },
  kind_doc:    { en: "DOC",            zh: "文档" },
  kind_arch:   { en: "ARCHIVED",       zh: "存档" },

  /* —— 门户图例 —— */
  lg_kit:    { en: "uses kit (single source)", zh: "已接套件(单一真源)" },
  lg_inline: { en: "inline — migration target", zh: "内联 — 迁移目标" },
  lg_doc:    { en: "doc / showcase",            zh: "文档 / 展示页" },
  lg_arch:   { en: "archived",                  zh: "存档" },

  /* —— 进度汇总 —— */
  progress:  { en: "built",   zh: "已建" },
  of:        { en: "of",      zh: "/" },

  /* —— 单一真源面板 —— */
  singleSource: { en: "SINGLE SOURCE", zh: "单一真源" },
};

/* —— 品牌字符串单一真源（docs/NAMING-MAP.md §1/§4）——
   产品名定档 Viden(2026-07-02)，代码侧(RoboCode)改名跟设计。
   新稿取品牌字符串一律走这里 —— 改名 = 改此一处。 */
window.VIDEN_BRAND = {
  name: "Viden",            /* 产品名 */
  wm: ["v", "iden"],        /* 字标拆字：wm[0]=gold · wm[1]=accent（glyph [◉] 见 DESIGN-REF） */
  cli: "viden",             /* CLI 可执行名（viden run …） */
  cfgDir: ".viden",         /* 项目配置目录 .viden/config.toml */
  cfgFile: "config.toml",
  gateFile: "viden.toml",   /* 仓库根 gate/ownership 真源 */
  branchPrefix: "vd/",      /* lane 分支前缀 */
  envPrefix: "VIDEN_",      /* 环境变量前缀 */
};
