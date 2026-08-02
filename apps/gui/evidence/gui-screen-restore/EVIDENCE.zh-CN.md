# 还原 GUI 屏 · 视觉证据

英文版：[EVIDENCE.md](EVIDENCE.md)

覆盖 `codex/v3-gui-screen-restore` 上还原的五屏：D2 决策中心、D10 Lane 监视器、
D12 集成闸、D13 Fleet 编排与 Workflow、D14 审计与时间线。

## 投影为什么是生成的，不是手写的

捕获页从不手写投影。`tests/capture_projections.rs` 用真实 GUI 投影跑规范
`frontend-contract-v1` fixture，把结果序列化进 `projections/*.json`，页面再挂载它。
因此截出来的像素就是 Core 事实实际产出的；投影改坏了会直接反映在截图里，而不会被
手工调过的字面量掩盖。

投影有任何改动后重新生成：

```bash
cargo test -p viden-gui --test capture_projections -- --ignored
```

## 捕获步骤

在拥有 `apps/gui/**` 的 worktree 里启动 dev server：

```bash
npm --prefix apps/gui run dev -- --port 4173 --strictPort
```

然后在授权的 Browser 运行时里按 1440x900 视口逐个打开下列 URL。这与
`tools/capture-d1-visual.sh` 一致：只固化 URL 与尺寸，不在该运行时之外调用浏览器
自动化。

| 屏 | URL |
| --- | --- |
| D2 决策中心 | `http://localhost:4173/evidence/gui-screen-restore/screen-capture.html?screen=d2` |
| D10 Lane 监视器 | `…?screen=d10` |
| D12 集成闸 | `…?screen=d12` |
| D13 Fleet 编排 | `…?screen=d13` |
| D14 审计时间线 | `…?screen=d14` |
| 语言与皮肤验证 | `…?screen=d12&locale=zh-CN&mode=light` |

`locale`、`mode`、`density` 每屏都接受，并统一走共享的 `resolveTheme`，捕获页不携带
第二套配色。

## 每张截图必须体现什么

- **D2**：三组队列；选中闸项并显示 Core 风险档；动作由 `allowed_scopes` 生成；
  动作栏显示审计 id；上下文面板 `GUI-CORE-012`、契约组 `GUI-CORE-013`。
- **D10**：每条 Lane 一张卡；门控强度取自 `AgentLaneRecord`；已绑定 Lane 显示项目、
  未绑定 Lane 明确写无绑定；事件流位置显示 `GUI-CORE-014`。
- **D12**：冲突横幅带「强闸 · 不可绕过」；点名缺失的必需证据；缺证据时 `accept`
  可见且禁用；退回时间线与合入后回滚；`GUI-CORE-015`。
- **D13**：DAG 目标与状态；节点声明的依赖边；被阻塞节点点名 Core 依赖原因；
  「Core 未记录任何交接」。
- **D14**：按 Core cursor 顺序排列并标注 Core 自己的事件判别名；无法解码的行保留并
  高亮；批次未完时显示分页控件。

## 在 fixture 之上补充的事实

下列补充全部使用运行时会发布的同一套类型化 Core 记录；不编造字段，也不把展示文本
解析成事实。

| 屏 | Fixture | 补充 |
| --- | --- | --- |
| D2 | `approval-allow-deny.json` | 从 fixture 自身事件载荷回放的审批（该 fixture 的事件流会解决掉审批，导致待办队列为空）、一条 `ContractRecord`、一条待处理 `ReviewRequestRecord`、一条 `EvidenceView` |
| D10 | `multi-lane.json` | 一条 `LaneRuntimeOwnerBinding`，使已绑定与未绑定两条路径同时可见 |
| D12 | `merge-gate.json` | 闸状态 `needs_changes` 加一条必需证据 id、一条 `ConflictBounce`、一条 `RevertRecord` |
| D13 | `dag-blocker.json` | 一条 blocked 状态的 `DependencyRecord` |
| D14 | 无 | 回放契约没有 fixture，批次由类型化 Core 事件构造，并经同一条 `CoreClient::replay` 路径提供 |

## 已知限制

授权的 Browser 运行时能渲染并核对这些页面，但不能写 PNG 文件，因此本目录保存的是
可复现的 harness，而不是已提交的图片。要产出可提交的 PNG，需要项目明确认可的捕获
脚本；`tools/capture-d1-visual.sh` 也刻意停在同一条边界上。

原生 Tauri 窗口未被截图：它是开发期二进制而非注册的 application bundle，桌面截图工具
无法定位它。应用本身在本分支可以启动：端口 `1420` 空闲时 `npm run tauri -- dev` 即可，
或用显式覆盖
`--config '{"build":{"beforeDevCommand":"npm run dev -- --port 4173 --strictPort","devUrl":"http://localhost:4173"}}'`。
