# Viden 0.1.21 计划 - 交互系统收口

英文版： [release-0.1.21-plan.md](release-0.1.21-plan.md)

## 摘要

`0.1.21` 要把 `0.1.18` 到 `0.1.20` 的交互优化收成一个统一产品系统。这个版本的目标不是继续增加更多面板或更多 agent 类型，而是让新用户不需要猜隐藏 slash command，也能完成配置、provider 故障恢复、日常 coding loop、delegated lane 操作，并明确知道焦点在哪里、下一步能按什么。

这个版本仍属于 V2 开发者增强层，同时为 V3 编排 runtime 做准备。MCP、ACP、plugin、skill 在本版本应停留在只读或 descriptor 层，除非它们复用这里定义的 settings、task、focus、permission 和 evidence 契约。

## 产品目标

到 `0.1.21` 结束，Viden 应该像一个可靠的终端产品：

- 所有设置/配置入口都打开可操作 picker 或 form；
- provider 配置和 model 选择在视觉与行为上明确分离；
- keyboard、mouse、Esc、Enter 在所有 modal 中行为一致；
- composer、command palette、approval modal、lane selector 和 side screens 共用同一套 focus model；
- 当前工作状态显示在主屏中央，并由共享 `AgentTask` snapshot 支撑；
- delegated lane 可以在不手写 lane id 的情况下 launch、inspect、review 和 resolve；
- 每个用户可见工作流都有确定性截图和至少一个 smoke 或 focused test。

## 0.1.20 基线

`0.1.20` 是 usability beta gate，已经引入或计划：

- 独立 `/setup` wizard selector；
- 在线 provider 缺少 API key 时，首次启动预填 `/setup`；
- 分离 `/provider` 供应商配置和 `/models` 模型选择；
- provider failure 分类，并给出恢复动作；
- `/lane` root action selector；
- setup、provider、model、lane、side screens、resize、中文输入、command palette、live-turn 等确定性截图。

剩余缺口：

- provider detail 还不像可编辑配置表单；
- settings 还没有完全统一到一套 modal/form 组件契约；
- mouse 和 focus 行为仍然偏零散；
- lane action 已经能发现，但还不是完整的一键/no-id 流程；
- `NOW WORKING`、right rail、side screens 还需要更严格地读取同一个 `AgentTask`；
- 截图已有，但真实使用的人工验收还需要正式进入 release checklist。

## P0 范围

### 1. 统一 Settings 与 Form Runtime

所有配置面都要使用同一套交互模型。

验收：

- `/settings` 打开 settings hub，包含 provider、model、permissions、theme、defaults、diagnostics、setup。
- `/setup` 使用同一套 selector/form runtime，不再是特殊渲染的一次性页面。
- `/provider` 一级只列供应商 id，例如 `deepseek`、`openrouter`，不在一级行里混入 key、endpoint 或 model 解释。Enter 或点击进入 provider detail。Provider detail 支持 inspect、设为默认、立即切换、编辑 endpoint、显示 key env hint、运行 doctor、probe model、打开过滤到当前 provider 的 `/models`。
- `/models` 按 provider 分组，并在数据存在时明确标记 current、configured、favorite、risky、unavailable。
- `/permissions`、`/theme` 和模式/default 设置使用相同 keyboard、mouse、footer 和 screenshot 契约。
- 长行使用 summary + detail pane，而不是把关键信息横向截断。

### 2. 首次启动 Setup 收口

把 clean-install path 做成真正 wizard。

验收：

- 缺少 API key 时打开 setup，并聚焦 provider/key step。
- wizard 显示所选 provider 的精确 env var，并以 transcript/evidence 形式给出可复制 shell export 命令。
- DeepSeek 仍然是默认在线路径；fallback 是离线路径。
- Probe 结果必须导向一个动作：continue、switch model、edit endpoint、doctor、fallback 或 retry。
- Save defaults 只写入非 secret 的 provider/model/default 设置。
- 为每个 setup state transition 和持久化边界添加测试。

### 3. Focus、Mouse 与 Modal Router

让 TUI 的键鼠行为可预测。

验收：

- 定义显式 focus targets：composer、command palette、selector/form、approval、transcript、right rail、lane detail、side-1、side-2。
- Esc 行为固定：先关闭 modal，再关闭 palette，再清空 command input，只有明确 exit action 才退出。
- Enter 行为固定：提交 composer、应用选中 modal row，或触发 focused approval control。
- Mouse click 在预期位置可以选择并触发行/按钮。
- Mouse wheel 滚动当前 focused scrollable pane。
- Focus state 视觉可见，并有确定性 preview 覆盖。
- 为 selector、provider detail、model selector、approval、lane selector 和 side-screen focus transitions 增加回归测试。

### 4. Composer 与 Command Palette 可靠性

让输入区在日常使用中稳定、安静。

验收：

- Composer 高度稳定，在窄终端和高终端中都可读。
- 光标始终可见，包括中文输入时。
- macOS Terminal 和 iTerm2 手工 smoke 中，IME candidate window 靠近输入文字。
- `/` command discovery 对决策命令只显示可操作行。
- `/provider`、`/models`、`/settings`、`/setup`、`/lane` 不允许退化成被动信息页。
- 增加 composer 与各 modal family 的 resize stress 覆盖。

### 5. 基于 AgentTask 的工作状态可视化

主屏中央必须回答“现在到底在干嘛”。

验收：

- provider thinking、streaming、tool call、approval、shell/test 执行、lane dispatch、lane review 和 completion 都写入/更新同一个共享 `AgentTask` snapshot。
- `NOW WORKING`、right rail active tasks、side-1 lane list、side-2 evidence、`/agent status`、`/lane inspect` 读取同一份 task facts。
- background count、blocked count、active lane count、latest evidence、next action 在多个 surface 上保持一致。
- 增加 focused tests，对比 main、right rail、side-1、side-2 preview 输出中的同一 task 状态。

### 6. Delegated Lane No-Guess Flow

Lane 编排不应该要求用户记住 id 或隐藏动词。

验收：

- `/lane` 列出 launch actions，并为每个 tracked lane 列出 id-specific actions。
- Lane detail 页面暴露 inspect、timeline、diff、artifacts、accept、apply、discard、retry、stop、cleanup 等可选择动作。
- Side-1 可以 focus lane 并打开 lane detail。
- Side-2 显示 evidence、artifacts、context pressure、changed files、conflict/apply state。
- 确定性 shell/template lane 仍然是 P0 baseline。
- Codex/Claude/tmux lane 可以保持 probe-level，但只要存在状态和 evidence，就必须映射到同一 `AgentTask` 与 lane UI。

### 7. Release Evidence Discipline

每个可见功能点都要有证据。

验收：

- 新增或更新 settings hub、provider detail、setup key step、setup probe result、model selector、approval focused states、lane action selector、lane detail、side-1、side-2、daily-loop final state 的确定性截图。
- 增加 macOS Terminal 和 iTerm2 手工 checklist，覆盖首次 setup、provider switch、model switch、approval、中文输入、resize、mouse selection 和 delegated lane review。
- Release status 必须列出每张截图证明了哪个功能点。

## P1 范围

- Favorite providers/models 和 last-known-good 恢复建议。
- Provider/model search 质量，包括 aliases 和 provider-scoped filters。
- 窄终端下 right rail 与 provider detail 的更紧凑布局。
- 使用统一 settings modal contract 的只读 MCP/plugin/skill capability browser。
- review、test、docs、shell task 等更多 lane templates。
- 发送 provider request 前的 token/context budget warning。

## 非目标

- `0.1.21` 不把 ACP 或 MCP 变成 mutating runtime。
- 不新增第三方 UI 依赖，除非现有 terminal widget 层无法满足 focus/form 要求。
- 默认不把 API key 保存到明文 config。
- Codex/Claude write-capable happy path 不作为 release blocker。
- 不增加没有 runtime facts 和 tests 的装饰性 panel。

## 测试计划

Focused：

- settings/form state transitions；
- provider detail actions 和 persistence boundaries；
- model selector grouping 与 switch commands；
- setup wizard key/probe/save states；
- focus router 与 Esc/Enter 行为；
- selector rows 与 action buttons 的 mouse hit testing；
- composer/caret/CJK preview rendering；
- composer、selector、provider detail、approval、lane detail 的 resize stress；
- TUI 多个 surface 上的共享 `AgentTask` 一致性；
- lane no-id action flows。

Regression：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-previews.sh docs/previews/generated
scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh
scripts/release-smoke.sh --version 0.1.21 --quick \
  --out-dir /tmp/viden-0121-release-smoke-local
```

Manual：

- macOS Terminal 和 iTerm2 首次 setup。
- Provider key missing、auth failure、timeout、model unavailable 和 fallback recovery。
- Provider detail editing 与 model switching。
- 中文输入和 IME candidate 位置。
- Provider detail、setup wizard、approval、lane selector、lane detail 激活时 resize。
- Provider detail、model selector、approval、right rail、lane controls 的 mouse selection。
- Fallback daily coding loop。
- 有凭证时跑 DeepSeek live daily coding loop。
- 确定性 delegated lane review loop。
