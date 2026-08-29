# 主窗口交互 · 视觉证据

英文版：[EVIDENCE.md](EVIDENCE.md)

覆盖 `claude/gui-main-window-interactions` 上的主窗口交互工作：D6 恢复动作
（重启 / 关闭 Lane、纯展示的事实展开、以及被拒绝时的告警）、Composer 的
工作模式 / 权限级别 / 模型选择器与驾驶舱状态栏、活动栏齿轮后的设置面板、
D11 项目接入，以及 D12 合并闸的批准 / 退回动作栏及其必填理由输入框。

## harness 是什么

[`qa.html`](qa.html) 加 [`qa.ts`](qa.ts) 按 `?state=` 每次只渲染一个截图状态。
页面**只调用生产环境导出的渲染函数** —— `renderD1Cockpit`、`renderD6Recovery`
（经由驾驶舱自己的工作区挂载）、`renderD11Intake`、`renderD12IntegrationGate`
以及它们挂载的组件。这里不重新实现任何控件、文案或布局，因此 `src/**` 的回归
会直接反映在截图里，而不会被 harness 自带的 UI 副本掩盖。

渲染逻辑放在 `qa.ts` 而不是内联 `<script>`，这样 `tsc --noEmit` 会按生产渲染
签名对 harness 做类型检查。

### 确定性

- 首次渲染前把 `Date.now` 冻结在 `2026-01-01T00:00:00.000Z`。工作状态条打印
  `now() - startedAt`，时钟不冻结会让同一状态的两次截图不一致。
- `Math.random` 冻结为 `0`。
- 驾驶舱以 `poll: false` 挂载，操作者取景时不会有定时器刷新截图。
- 所有宿主回调都是永不 resolve 的 promise，harness 稳定后状态不再变化。唯一
  例外是 `d6-error`：它的 `sendD6Intent` 会以一句固定文本 reject —— 这次拒绝
  正是要截取的状态。
- 每个状态最后把自身名字写入 `document.documentElement.dataset.captureReady`。
  出现该属性后再截图。

## 投影来自哪里

| 状态组 | 来源 | 类型 |
| --- | --- | --- |
| `d1*`、`settings*`、`d6-*` | [`../../tests/support/d1_projection.ts`](../../tests/support/d1_projection.ts) | vitest 各套件共用的 D1 fixture |
| `d12-*` | [`../gui-screen-restore/projections/d12.json`](../gui-screen-restore/projections/d12.json) | 由 `tests/capture_projections.rs`**生成** |
| `d11`、`d11-recent` | 镜像 `tests/d11_intake.spec.ts` 里的 fixture，历史面板另用下文手写的 `RecentWorkResult` | 手写；D11 目前还没有生成的捕获投影 |
| `lane-rail`、`project-picker`、`project-switch-confirm` | 共享 D1 fixture 加一份手写的 `RecentWorkResult` | 手写；`frontend-contract-v1` 目前还没有规范的最近工作捕获投影，因此形状镜像 `tests/recent_work.rs` 与 `tests/project_picker.spec.ts` |

D12 投影从不手写。`tests/capture_projections.rs` 用真实 Rust 投影跑规范
`frontend-contract-v1` 的 `merge-gate.json` fixture 并序列化结果，因此截出来
的像素就是 Core 事实实际产出的。投影有任何改动后重新生成：

```bash
cargo test -p viden-gui --test capture_projections -- --ignored
```

W4 给 D12 闸动作加上了类型化的可用性码；已提交的 `d12.json` 现在在 accept 上
携带 `missing_evidence`、在 reject 上携带 `no_actor`，这正是 `d12-blocked`
要截取的内容。

### 在来源之上写的 delta

harness 补充的每个值都是上述来源之上的 delta，`qa.ts` 中每条都有行内注释指明
它镜像的是哪个 fixture。

| 状态 | Delta | 镜像自 |
| --- | --- | --- |
| 全部 `d1*` | `preferences.locale/skin/mode` 跟随 URL 参数 | 驾驶舱从投影而非文档元素读取语言 |
| 全部 `d1*` | 上下文坞的一条 `ContextUsageProjection`，以及共享 fixture 留空的三个状态栏字段（`context`、`diagnosticsCount`、`pendingGateCount`） | `tests/statusbar.spec.ts` 中已填满的状态栏 fixture |
| 全部 `d1*` | `agentAdapters[0].models` | `tests/composer_controls.spec.ts` 中的适配器 fixture |
| `d6-actions`、`d6-error` | 一个已停止的会话，其 `restart` 携带 session id、`close_lane` 携带 lane id | `tests/d6_recovery.spec.ts` 中的 `STOPPED` fixture |
| `d12-actions` | 已记录必需证据、验证方满足，两个动作都可用且 code 为 `null` | `tests/d12_integration_gate.spec.ts` 中的 `DECIDABLE` fixture |
| `d11` | 已探测的 `/workspace/demo` rust 项目，提供方处于凭据锁定状态 | `tests/d11_intake.spec.ts` 中的已探测项目 fixture |
| `d11`、`d11-recent` | 交给屏幕最近工作端口的同一份两项目 `RecentWorkResult` | `tests/d11_intake.spec.ts` 中的已加载行 fixture |
| `palette` | 交给 `loadPaletteCrossLane` 的一条跨 Lane 合并闸与一条询问 | `tests/d12_integration_gate.spec.ts` 的闸 fixture，以及 D1 fixture 本就带的那条 `liveWork.approvals` |
| `lane-rail`、`project-picker`、`project-switch-confirm` | 一份两项目的 `RecentWorkResult`，时间戳是相对冻结时钟的偏移，因此渲染出的相对时间稳定。当前打开的根目录被刻意包含在内——选择器必须把它从「最近」中剔除，而不是提供切换到已经打开的项目 | `tests/recent_work.rs` 断言的 `RecentWorkLoaded` 载荷 |

共享 D1 fixture 的 `topbarSource.project` 现在携带 `viden` 而不是 `null`。这是 Core
发布的名称，也正是标题栏选择器、侧栏 `.wsroot` 分组头与选择器「工作区内」一行共同
渲染的内容；回退到路径的分支仍由 `tests/cockpit_topbar.spec.ts` 与
`tests/lane_rail.spec.ts` 覆盖，它们显式把该字段覆写为 `null`。

## 如何运行

在拥有 `apps/gui/**` 的 worktree 里启动 dev server：

```bash
npm --prefix apps/gui run dev -- --port 4173 --strictPort
```

然后在授权的 Browser 运行时里按 1440x900 视口逐个打开下列 URL，等待
`data-capture-ready` 出现后截图。这与 `tools/capture-d1-visual.sh` 一致：只固化
URL 与尺寸，不在该运行时之外调用浏览器自动化。

所有 URL 共用前缀
`http://localhost:4173/evidence/main-window-interactions/qa.html`。

| 状态 | URL | 截图必须体现什么 |
| --- | --- | --- |
| `d1` | `…/qa.html?state=d1` | 完整驾驶舱；标题栏项目选择器带分支与 dirty 标记，旁边是 `↑/↓` 与工作树 chip；九个状态栏分段全部带事实，另加待决闸提示；三个 Composer 选择器胶囊 |
| `d1-mode-menu` | `…/qa.html?state=d1-mode-menu` | 工作模式弹层在 Composer 上方展开，当前模式标记为选中 |
| `d1-model-menu` | `…/qa.html?state=d1-model-menu` | 模型弹层展开，同时显示提供方分组与 Core 发布的适配器分组 |
| `settings` | `…/qa.html?state=settings` | 设置面板覆盖在驾驶舱上并带未保存草稿；取消与保存均可用 |
| `settings-unavailable` | `…/qa.html?state=settings-unavailable` | 同一面板只读，点名缺失的 `ui.preference_persistence` 能力；保存禁用 |
| `d6-actions` | `…/qa.html?state=d6-actions` | 恢复界面上「重启智能体」与「关闭 Lane」可用，并展开检查事实 |
| `d6-error` | `…/qa.html?state=d6-error` | 同一界面在重启被拒后，把 Core 的拒绝理由渲染成告警 |
| `d12-actions` | `…/qa.html?state=d12-actions` | 合并闸的批准可用，退回理由输入框已填写且可用 |
| `d12-blocked` | `…/qa.html?state=d12-blocked` | 同一闸的批准不可用并点名 `missing_evidence`，理由输入框禁用 |
| `d11` | `…/qa.html?state=d11` | 项目接入屏，显示已探测项目与提供方告警 |
| `d11-recent` | `…/qa.html?state=d11-recent` | 同一接入屏滚动到「最近工作」面板，显示 Core `QueryRecentWork` 行（名称、相对时间、会话数、规范根目录），替代已退役的静态不可用文案 |
| `palette` | `…/qa.html?state=palette` | 从标题栏按钮打开、覆盖在驾驶舱之上的 ⌘K 命令面板，四个分区全部可见——动作、跳转到（跨 Lane 的闸与询问，加上本 Lane）、设置，以及点名 `GUI-CORE-022` 的永久禁用「文件」行 |
| `lane-rail` | `…/qa.html?state=lane-rail` | 侧栏被固定展开（它默认自动隐藏），显示名为 `viden` 的唯一 `.wsroot` 项目分组、`▾` 折叠控件、Lane 计数、分组内 `＋`、嵌套其下的 Lane，以及 `＋ 添加项目…` 页脚；没有第二个分组，也没有「Global」分区 |
| `project-picker` | `…/qa.html?state=project-picker` | 选择器在标题栏 `▾` 之下展开，三列同时可见：可用的 `添加目录…` 与两行点名 `GUI-CORE-023` 的禁用行、当前打开项目的唯一「工作区内」行及其 lane 计数，以及一行带相对时间的「最近」 |
| `project-switch-confirm` | `…/qa.html?state=project-switch-confirm` | 同一选择器在点击最近项目后进入内联确认：目标根目录、点名 `GUI-CORE-023` 的替换说明、正在运行的工作计数，以及「取消」与「切换工作区」两个按钮 |

`mode=dark|light` 与 `locale=en|zh-CN` 每个状态都接受，并统一走共享的
`resolveTheme`，harness 不携带第二套配色。`mode=light` 同时选用 `ice` 皮肤，
与设计的搭配一致。建议的语言与皮肤验证：
`…/qa.html?state=d1&mode=light&locale=zh-CN`。

## 已捕获截图

2026-08-21 以 headless Chrome
(`--headless --window-size=1440,900 --virtual-time-budget=6000`)对 4173 端口
的 vite 开发服务器采集,随后人工目检(评审抽样 6/11;构建时 11 个状态均已做
DOM 级验证)。

| 文件 | 状态 | 视口 | 模式 | 语言 |
| --- | --- | --- | --- | --- |
| [d1-1440x900-dark-en.png](d1-1440x900-dark-en.png) | d1 | 1440x900 | dark | en |
| [d1-mode-menu-1440x900-dark-en.png](d1-mode-menu-1440x900-dark-en.png) | d1-mode-menu | 1440x900 | dark | en |
| [d1-model-menu-1440x900-dark-en.png](d1-model-menu-1440x900-dark-en.png) | d1-model-menu | 1440x900 | dark | en |
| [settings-1440x900-dark-en.png](settings-1440x900-dark-en.png) | settings | 1440x900 | dark | en |
| [settings-unavailable-1440x900-dark-en.png](settings-unavailable-1440x900-dark-en.png) | settings-unavailable | 1440x900 | dark | en |
| [d6-actions-1440x900-dark-en.png](d6-actions-1440x900-dark-en.png) | d6-actions | 1440x900 | dark | en |
| [d6-error-1440x900-dark-en.png](d6-error-1440x900-dark-en.png) | d6-error | 1440x900 | dark | en |
| [d12-actions-1440x900-dark-en.png](d12-actions-1440x900-dark-en.png) | d12-actions | 1440x900 | dark | en |
| [d12-blocked-1440x900-dark-en.png](d12-blocked-1440x900-dark-en.png) | d12-blocked | 1440x900 | dark | en |
| [d11-1440x900-dark-en.png](d11-1440x900-dark-en.png) | d11 | 1440x900 | dark | en |
| [d11-recent-1440x900-dark-en.png](d11-recent-1440x900-dark-en.png) | d11-recent | 1440x900 | dark | en |
| [d1-1440x900-light-zh-CN.png](d1-1440x900-light-zh-CN.png) | d1 | 1440x900 | light | zh-CN |

八张带标题栏的截图（`d1*`、`settings*`、`d6-*`）已于 2026-08-21 在标题栏 git 块
落地后重新采集，显示带分支与脏标记点的项目选择器及两个 `.gitops` chip
（`↑1 ↓0`、`⎇ 1 个工作树`）；独立屏 `d12*` 不含驾驶舱标题栏，原截图仍然有效。
`d11-recent` 于 2026-08-29 首次采集——历史面板自此渲染 Core 的最近工作行，
替代已退役的 `GUI-CORE-007` 文案；同日重采的 `d11` 与原图逐字节一致，
因为该面板位于此视口折叠线之下。

命令面板在 `.tbtools` 中新增了一个 `.tbtbtn` 按钮;八张带标题栏的截图与新的
`palette` 状态已于 2026-08-21 在 1440x900 下重新采集并人工目检(palette 截图
显示前缀图例、全部四个分组、kbd 提示与禁用的 Files 行)。

| 文件 | 状态 | 视口 | 模式 | 语言 |
| --- | --- | --- | --- | --- |
| [palette-1440x900-dark-en.png](palette-1440x900-dark-en.png) | palette | 1440x900 | dark | en |

## 项目选择器与分组侧栏截图

九张带标题栏的截图(标题栏选择器获得设计稿的 `▾` 与按钮外观;侧栏获得 `.wsroot`
分组头与 `＋ 添加项目…` 页脚)连同三个新状态已于 2026-08-21 在 1440x900 下重新
采集并人工目检(选择器三列含两行 `GUI-CORE-023` 禁用行、当前项目与最近项目行、
切换确认对话框明示替换语义与影响计数)。

| 文件 | 状态 | 视口 | 模式 | 语言 |
| --- | --- | --- | --- | --- |
| [project-picker-1440x900-dark-en.png](project-picker-1440x900-dark-en.png) | project-picker | 1440x900 | dark | en |
| [lane-rail-1440x900-dark-en.png](lane-rail-1440x900-dark-en.png) | lane-rail | 1440x900 | dark | en |
| [project-switch-confirm-1440x900-dark-en.png](project-switch-confirm-1440x900-dark-en.png) | project-switch-confirm | 1440x900 | dark | en |

## 已知限制

授权的 Browser 运行时能渲染并核对这些页面，但不能写 PNG 文件，因此在操作者实际
截图之前，本目录保存的是可复现的 harness 而不是已提交的图片。
`tools/capture-d1-visual.sh` 也刻意停在同一条边界上。
