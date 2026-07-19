# NAMING-MAP · 产品名映射表（Viden ⇄ RoboCode · 唯一真源）

> **定稿(2026-07-02):产品名 = Viden,代码侧改名跟设计。**
> 本表是「设计词 ⇄ 引擎(RoboCode v0.1.30)现名」的唯一映射真源:编码侧照左列改名,
> 设计侧新稿只用左列。改名 = 改本表 + `i18n-dict.js` 的 `VIDEN_BRAND` 一处。
> 审查看板 #20(命名规范)/#21(gate type↔UI 标签)在本表收口。

## 1 · 品牌面（设计已定 → 代码改名目标）

| 项 | 设计名（唯一真源） | 引擎现名(v0.1.30) | 备注 |
|---|---|---|---|
| 产品名 | **Viden** | RoboCode | crate 前缀 `robocode-*` → `viden-*` |
| 字标 | `[◉] viden`（`[`/`]`=accent · `◉`=accent-bright · `v`=gold · `iden`=accent） | — | 结构在 DESIGN-REF「窗口标题栏 chrome」;字符串从 `VIDEN_BRAND.name/wm` 取 |
| `<title>` / 窗口题 | `viden Desktop` / `viden — <ctx>` | — | |
| bot / agent 头像 | `AgentLogo agent="viden"`（青括号眼+金瞳） | — | 内置 agent 的品牌徽标 |
| CLI 可执行名 | `viden`（`viden run …`） | `robocode-cli`(bin) | 产品方案 07-C 已按 `viden run` 写 |
| lane 分支前缀 | `vd/<slug>` | `codex/lane-<sessionId>-<laneId>` | 引擎 `tui/lane.rs` 硬编码,改名时一并换 |
| 项目配置目录 | `.viden/config.toml` | `.robocode/config.toml` | 键名不变(见 §3) |
| 全局配置 | `~/.config/viden/config.toml` | `~/.config/robocode/config.toml` | XDG/APPDATA 同理;macOS = `~/Library/Application Support/robocode/` |
| 任务简报 | `.viden/briefs/active.md` | `.robocode/briefs/active.md` | lane 启动时读入 |
| 风格指引 | `.viden/steering/`（如 `conventions.md`） | `.robocode/steering/` | 项目级 agent 行为约束 |
| 环境变量前缀 | `VIDEN_*`（`VIDEN_CONFIG` `VIDEN_PROVIDER` `VIDEN_<PROVIDER>_API_KEY` …） | `ROBOCODE_*` | 全量同形替换 |
| 闸策略/ownership 文件 | 仓库根 `viden.toml`（产品方案 Q1 定稿） | 引擎暂无对应 | **≠** `.viden/config.toml`:`viden.toml` 随仓库走管 gate/ownership;config.toml 管本机偏好 |
| mock 项目名 | `viden`（自举:用 viden 开发 viden） | ~~robocode~~ 泄漏已修 | D1/D6/D8/D13 mock 的 project scope |

## 2 · 门控类型 ⇄ UI 标签（看板 #21 收口）

**双层模型（SPEC `D-PERM`）**:permission 拦「能不能执行」(就地·阻塞·`.gperm`/TUI `.vgate`),gate 拦「批不批合入」(决策中心·异步)。

**PermissionMode（引擎枚举 `cli_name` → UI 标签）** — 设置屏/状态栏 PERM 段用 UI 标签,配置文件与 CLI 用 cli_name:

| cli_name(配置值) | UI 标签 | 语义 |
|---|---|---|
| `default` | **Ask** | 危险动作逐个就地审;安全读放行 |
| `acceptEdits` | **Auto Edit** | write/edit 放行;shell/越界仍审 |
| `plan` | **Read Only** | mutating 一律拒(plan mode) |
| `dontAsk` | **Full Access**（另加 `dontAsk` 小字注） | 不问直接执行(沙箱用)。⚠ 引擎 `from_legacy_mode` 把 dontAsk 与 bypassPermissions 都折叠为 `FullAccess`(label "Full Access") —— 无独立 label。状态栏 PERM 段显示 **Full Access**,悬停/设置屏用 cli_name `dontAsk` 区分档位 |
| `bypassPermissions` | **Full Access** | 绕过权限引擎;合入仍过 gate |

（引擎另有 `PermissionLevel` ask/auto_edit/auto/read_only/full_access。`from_legacy_mode` 是 **5→4 折叠**而非同构：Default→Ask · AcceptEdits→AutoEdit · Plan→ReadOnly · **Bypass|DontAsk→FullAccess**；另有 `Auto` 档无 legacy 对应。UI 标签取 PermissionLevel.label();dontAsk 与 bypass 的区分只在配置层(cli_name)存在。）

**PermissionDecision → UI**:`Allow`=放行(不打扰) · `Ask`=⏸ 金「需要人」(permission 卡) · `Deny`=✗ 红拒(理由:OutOfScopePath/RuleDeny/PlanMode…)。

**WorkMode**（`plan|build|review|explore`）→ 状态栏 MODE 段金色标签 Plan/Build/Review/Explore。

**viden.toml gate 类型 → UI 标签**（设计规约,引擎待实现）:

| gate type(toml) | UI 标签(zh) | UI label(en) |
|---|---|---|
| `merge` | 合入闸 | Merge gate |
| `replay-regression` | 回放回归闸 | Replay gate |
| `field` | 实机闸 | Field gate |
| `contract` | 契约闸 | Contract gate |

## 3 · 引擎配置键（不改名,两侧共用词汇）

顶层:`provider` `model` `api_base` `api_key` `api_key_env` `provider_plugin_dirs` `permission_mode` `session_home` `request_timeout_secs`(默认90) `max_retries`(默认1);`[providers.<id>]`:`api_base` `api_key(_env)` `default_model` `models` `favorite_models`。优先级 CLI > env > 项目 > 全局。
Provider registry 内置 id:`deepseek`(默认·`deepseek-v4-flash`) `deepseek-anthropic` `anthropic` `openai` `openai-compatible` `openrouter` `groq` `mistral` `together` `kimi` `qwen` `zhipu` `volcengine` `dashscope-*` `ollama`(免key) `fallback`;插件经 `provider_plugin_dirs`。
引擎无对应键(GUI 层实现,设置屏已标注):快捷键 · 遥测 · 通知/离桌兜底。

## 4 · 落地纪律

- **渲染文案禁提引擎旧名**：任何会渲染给用户/评审看的文案(产品 UI、设计稿 plead/note、mock 数据)一律用左列设计名(Viden / viden)指代引擎——"RoboCode" 只允许出现在**本表右列、代码注释、docs/ 的映射与决策记录**里。已渲染文案要指版本时用产品线版本(如 `viden 0.1.30`)，不得复述引擎 crate 版本号以外的臆造版本。(编码侧核查 2026-07-02·T5 案例收口)
- **设计稿取品牌字符串一律走 `i18n-dict.js` 的 `window.VIDEN_BRAND`**(name/wm/cli/cfgDir/cfgFile/gateFile/branchPrefix/envPrefix),别再散写字面量 —— 改名 = 改那一处。存量页已按左列写死 "viden" 的视为已合规(与真源一致),新稿必须走 `VIDEN_BRAND`。
- 引擎侧对齐动作(编码侧任务,列此备查):crate/bin 改名 · 配置目录/env 前缀 · 分支前缀 `vd/` · `PermissionMode` UI 标签用 §2 表。
- 本表变更须记 `CHANGELOG.md` 并同步 `DESIGN-REF.md` 索引。
