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
| `d11` | 镜像 `tests/d11_intake.spec.ts` 里的 fixture | 手写；D11 目前还没有生成的捕获投影 |

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
| [d1-1440x900-light-zh-CN.png](d1-1440x900-light-zh-CN.png) | d1 | 1440x900 | light | zh-CN |

上表所有截图都早于 `claude/gui-titlebar-git` 上新增的标题栏 git 块。共享 D1
fixture 现在带 `topbarSource`，因此每个 `d1*`、`settings*` 与 `d6-*` 状态都会渲染
带分支的项目选择器与两个 `.gitops` chip；已提交的 PNG 仍是没有它们的旧标题栏，
需要操作者重新截图。

## 已知限制

授权的 Browser 运行时能渲染并核对这些页面，但不能写 PNG 文件，因此在操作者实际
截图之前，本目录保存的是可复现的 harness 而不是已提交的图片。
`tools/capture-d1-visual.sh` 也刻意停在同一条边界上。
