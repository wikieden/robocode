# TUI Cockpit 与 Terminal Lanes 开发计划

日期：2026-05-23

## 需求摘要

Viden 的 TUI 要从当前 lightweight alternate-screen shell，演进成 coding-agent cockpit：

- 实现 `DESIGN.md` 中已确认的主屏 TUI；
- 保留现有 `SessionEngine` 路径，继续负责 prompt、审批、工具执行和 transcript；
- 增加 screen model，为一个主屏加两个副屏工作舱做准备；
- 引入 supervised terminal lanes，让副屏能承载真实 side work；
- 在 lane runtime 稳定之后，再通过 task envelope 和 adapter 接入 `codex`、`claude` 等外部 terminal coding tools。

当前代码适合增量推进：

- `viden-cli/src/main.rs:96` 在 `--tui` 时进入 `tui::run_tui`。
- `viden-cli/src/tui.rs:28` 是当前 TUI 事件循环。
- `viden-cli/src/tui.rs:67` 已经把权限提示接到 `prompt_for_tui_approval`。
- `viden-cli/src/tui.rs:70` 复用 `SessionEngine::process_input_with_approval`。
- `viden-cli/src/tui.rs:187` 目前把整个 UI 渲染为一个简单字符串 frame。
- `viden-core/src/lib.rs:31` 暴露 `EngineEvent`，足够支撑第一版 transcript timeline。
- `viden-types/src/transcript.rs:28` 定义了 durable transcript entry，后续副屏可跟随它。

## 最新产品设计要求

当前产品方向已经覆盖并替代了之前较泛的 “rich terminal view” 说法：

1. 主产品界面是用户确认的单屏 Viden cockpit，不是生成的多屏概念图。
2. 副屏是工作舱，不是 dashboard。核心价值是运行和监督 side work。
3. Terminal lane 是 side work 的核心原语：
   - 运行本地命令；
   - 后续启动 `codex`、`claude` 等 coding CLI；
   - 捕获日志、状态、exit code、diff 和验证证据；
   - 必须经过 inspect/accept/revise/discard 的明确决策。
4. 外部工具是 adapter 后面的协作者，不是 Viden 原生可信 agent。
5. provider live compatibility 仍然重要，但不阻塞 TUI/lane 产品切片；provider matrix 是并行质量线。

## 当前开发基线

最新 `main` 位于 `bf41ffa`，并新增了 provider live compatibility matrix。下一批开发应从这个基线开始：

- `docs/provider-live-matrix.md` 现在是 live provider 检查的证据日志。
- TUI 顶部状态栏需要清楚展示 provider/model/mode，但不需要先完成 provider live verification。
- TUI 开发中如果使用 DeepSeek/OpenAI-compatible provider smoke，只能记录真实验证结果；offline test 不能描述成 live provider compatibility。

## 开发里程碑

### Milestone A: 提交设计基线

目标：先把设计、对标、预览图、架构计划和执行计划作为可 review checkpoint 保存下来。

文件：

- `DESIGN.md`
- `docs/code-agent-benchmark.md`
- `docs/tui-lane-architecture-plan.md`
- `docs/tui-lane-architecture-plan.zh-CN.md`
- `docs/superpowers/plans/2026-05-23-tui-cockpit-terminal-lanes.md`
- `docs/superpowers/plans/2026-05-23-tui-cockpit-terminal-lanes.zh-CN.md`
- `docs/previews/`
- `PLAN.md`

验收：

- 文档与最新 `main` 内部一致；
- `PLAN.md` 链接到新的 source docs；
- 这个 checkpoint 不改变源码行为。

### Milestone B: 主屏 Cockpit

目标：用已确认的主屏结构替换 lightweight TUI frame，同时保持 `SessionEngine` 行为。

交付：

- 顶部状态栏；
- transcript timeline；
- 右侧 workspace/status rail；
- composer；
- 底部状态栏；
- modal approval state；
- 内置 `aurora-cyan` theme tokens。

验收：

- prompt 提交和审批仍走 `SessionEngine`；
- render snapshots 覆盖 compact、normal、wide 终端；
- fallback-provider TUI smoke 仍可用；
- plain REPL 不受影响。

### Milestone C: Screen Registry Shell

目标：引入 `MAIN` / `AGENTS` / `OPS` registry，但暂不引入外部进程复杂度。

交付：

- screen state model；
- 最多两个 companion workspaces；
- open/focus/close command parsing；
- 基于真实状态的 companion render placeholders。

验收：

- 打开第三个副屏会清晰拒绝；
- `AGENTS` 和 `OPS` 有不同布局优先级；
- companion 状态变化时主会话仍可用。

### Milestone D: Terminal Lane Runtime MVP

目标：通过 supervised 非交互命令，让副屏工作舱真正能干活。

交付：

- `TerminalLane` state；
- durable lane logs；
- `/lane run <command>`；
- `/lane inspect <id>`；
- `/lane stop <id>`；
- 主屏右栏展示 active lane summary。

验收：

- 成功和失败命令都捕获 status、output 和 exit code；
- TUI 退出后日志仍保留；
- lane 状态流转有单元测试。

### Milestone E: Task Envelopes 与外部工具 Adapter

目标：为安全接入 `codex`、`claude` 和用户自定义 coding CLI 做准备。

交付：

- task-envelope rendering；
- adapter config shape；
- generic `/lane ask <tool> <task>`；
- 当二进制存在时启用 `codex` 和 `claude` preset；
- 优先支持保守输入模式：`prompt-file`、`stdin`、`manual`；
- `/lane revise`、`/lane accept`、`/lane discard`。

验收：

- 缺失二进制给出清晰 lane error；
- 每个外部任务都有可审计 envelope 文件；
- Viden 只有在检查 logs/diff/verification evidence 后才建议 accept。

### Milestone F: 隔离与 Attach

目标：让外部 coding lanes 对真实使用足够安全、也足够可交互。

交付：

- 可选 per-lane worktrees；
- 集成前 diff review；
- 通过 tmux、OS terminal windows 或 embedded PTY 做 attach/detach 原型；
- cleanup/archive policy。

验收：

- 会改文件的外部 lane 可以跑在主 worktree 外；
- `/lane attach` 和 `/lane detach` 不会杀掉 lane；
- attach 前后日志仍然持久。

## 非目标

- 不替换 plain REPL。
- 不在本阶段引入 GUI/web client。
- lane metadata、日志和 inspect 稳定前，不做 embedded PTY/tmux attach。
- 不把外部工具当成可信裁判；Viden 必须通过日志、exit code、diff 和验证证据来验收 lane 结果。
- 不复制 `.ref/` 实现。

## 架构决策

近期分两个实现分支：

1. `codex/tui-main-screen`
   - 把单文件 TUI 重构成小型内部模块；
   - 实现已确认的主屏和审批 modal；
   - 所有运行时行为继续走 `SessionEngine`。

2. `codex/tui-lane-runtime`
   - 增加 terminal lane metadata、日志捕获和 `/lane run <command>`；
   - 第一版 lane 只做非交互命令；
   - `/lane inspect` 可靠之前，不接 `codex`/`claude` adapter。

这样能把产品形态和 side-work 执行拆开，保持两个分支都足够小、可 review。

## 建议模块形态

先在 `viden-cli/src/tui/` 内部拆分：

```text
viden-cli/src/tui/
  mod.rs
  app.rs
  layout.rs
  render.rs
  theme.rs
  input.rs
  panels.rs
  screens.rs
  lanes.rs
```

初期保持为 `viden-cli` 私有模块。只有当 lane 状态需要被 TUI 外复用时，再把 durable lane records 下沉到 `viden-workflows`。

## 验收标准

### 主屏 TUI

- `cargo run -p viden-cli -- --tui --provider fallback --model test-local` 仍能启动可用 TUI。
- 主渲染包含：
  - 顶部状态栏；
  - transcript timeline；
  - 右侧 workspace/status rail；
  - composer；
  - 底部状态栏；
  - approval modal 状态。
- 审批仍通过现有共享 approval path 返回 `ApprovalResponse`。
- render tests 覆盖 compact、normal、wide 终端尺寸。
- plain REPL 行为不变。

### Terminal Lane MVP

- `/lane run <command>` 启动一个 supervised 非交互命令。
- lane 记录：
  - id；
  - command；
  - cwd；
  - status；
  - start/update timestamp；
  - log path；
  - 完成后的 exit code。
- lane 日志在 TUI 退出后仍保留。
- `/lane inspect <id>` 汇总最后输出、exit code 和状态。
- 失败命令展示失败状态和 exit code。
- lane 状态流转有单元测试。

### 外部工具准备度

- 在启用 `codex` 或 `claude` preset 前，task envelope 渲染已经可测试。
- adapter config shape 不硬编码单一 vendor 行为。
- 外部二进制缺失时给出清晰 lane error，不 panic。

## 实现步骤

### Phase 0: 基线与分支卫生

1. 确认 `main` 最新。
2. 在 `.worktrees/codex-tui-main-screen` 创建 `codex/tui-main-screen`。
3. 保留当前未提交设计文档和预览图；不编辑 `.ref/`。
4. 编辑前跑当前 TUI focused test：

```bash
cargo test -p viden-cli tui --quiet
```

预期：当前 TUI 测试通过；如果 Rust toolchain setup 有问题，记录清楚。

### Phase 1: TUI 模块拆分

1. 把 `viden-cli/src/tui.rs` 移到 `viden-cli/src/tui/mod.rs`。
2. 把纯渲染 helper 拆到 `render.rs`、`layout.rs`、`panels.rs`。
3. 保持 `run_tui` 为 `pub(crate)`，让 `viden-cli/src/main.rs:97` 行为不变。
4. 视觉扩展前先加 layout split 测试。

验证：

```bash
cargo test -p viden-cli tui --quiet
```

### Phase 2: 主屏渲染

1. 增加 `TuiApp` 状态：
   - session id；
   - provider/model；
   - permission mode；
   - input buffer；
   - transcript entries；
   - right-rail snapshot placeholders；
   - optional approval modal。
2. 渲染已确认主屏结构：
   - top rail；
   - transcript timeline；
   - right rail；
   - composer；
   - bottom status bar。
3. 右栏第一版保守取数：
   - cwd/workspace；
   - 如果方便，展示 workflow active task；
   - provider/model；
   - recent files 只有可靠时才显示。
4. 不过度贴合生成图，优先稳定终端几何和可读性。

验证：

```bash
cargo test -p viden-cli tui --quiet
cargo run -p viden-cli -- --help | rg -- '--tui'
```

### Phase 3: 审批 Modal

1. 把 `prompt_for_tui_approval` 从 transcript 文本提示改成 modal state。
2. 保持按键行为：
   - `y` approve；
   - `n` 或 `Esc` deny。
3. "apply to all" 先做视觉占位；除非 permission engine 已支持，否则不加策略语义。

验证：

- 单元测试 modal render 包含 tool name、message、input preview、approve/deny controls；
- smoke fallback provider TUI path。

### Phase 4: Theme Tokens

1. 增加内置 theme tokens：
   - `aurora-cyan`；
   - `ember-gold`；
   - `plasma-violet`；
   - `monochrome-ice`。
2. 默认使用 `aurora-cyan`。
3. 配置加载除非非常小，否则放到后续切片。

验证：

- 单元测试所有内置主题都有必需 token；
- render tests 尽量不依赖不稳定 color assertion。

### Phase 5: 合并第一刀

1. 运行：

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet
cargo run -p viden-cli -- --provider fallback --model test-local --help
```

2. 如果用户可见 flag 或行为变化，更新 README/README.zh-CN。
3. 按 Lore Commit Protocol 提交。
4. 开 PR，CI 通过后合并。

## 第二分支：Terminal Lane Runtime

### Phase 6: Lane 状态模型

分支：`codex/tui-lane-runtime`。

增加第一版 CLI-local lane state model：

```rust
enum LaneStatus {
    Queued,
    Starting,
    Running,
    Completed,
    Failed,
    Stopped,
    Archived,
}

struct TerminalLane {
    id: String,
    command: Vec<String>,
    cwd: PathBuf,
    status: LaneStatus,
    log_path: PathBuf,
    exit_code: Option<i32>,
    started_at: u64,
    updated_at: u64,
}
```

第一版可先把 lane records 放在 session home 下；如果别扭，再下沉到 `viden-workflows`。

### Phase 7: `/lane run`

1. 增加 TUI command parsing：

```text
/lane run <command>
/lane inspect <id>
/lane stop <id>
/lane archive <id>
```

2. 第一版只跑非交互命令。
3. stdout/stderr 捕获到 durable log file。
4. 主屏右栏展示 active lanes。

验证：

- command parsing 单元测试；
- lane state transition 单元测试；
- 对 `printf hello` 这类无害命令做 integration-style test；
- inspect command 展示最后输出和 exit code。

### Phase 8: Lane Inspect 与 Review UX

1. `/lane inspect <id>` 展示：
   - status；
   - command；
   - cwd；
   - log path；
   - exit code；
   - 最后 N 行日志。
2. changed files 和 verification command 可先留占位，但未实现前不要声称支持。
3. 把这个界面设计成未来 `codex`/`claude` 的验收入口。

### Phase 9: 外部工具 Adapter 准备

先不要完整实现 `codex` 或 `claude`。只准备 adapter shape：

```rust
enum LaneInputMode {
    Argv,
    Stdin,
    PromptFile,
    Manual,
}
```

增加 task-envelope rendering 测试，但不启动外部工具。

## 风险与缓解

- 风险：TUI refactor 过大。
  - 缓解：第一分支只做渲染，不做 lane runtime。

- 风险：外部工具意外修改主 worktree。
  - 缓解：adapter 阶段必须先有明确 cwd/mutation scope，再推进 per-lane worktree。

- 风险：embedded terminal/PTY 复杂度拖垮有用的 lane 工作。
  - 缓解：先做非交互命令和 durable logs。

- 风险：TUI 破坏 plain REPL。
  - 缓解：`main.rs` 继续只在 `--tui` 分支进入 TUI；现有 REPL loop 不动。

- 风险：右栏展示陈旧或假数据。
  - 缓解：占位必须标明；优先使用真实 engine/workflow 数据。

## 验证计划

每个分支最低要求：

```bash
cargo fmt --all -- --check
cargo test -p viden-cli --quiet
cargo test --workspace --quiet
```

Smoke checks：

```bash
cargo run -p viden-cli -- --help | rg -- '--tui'
cargo run -p viden-cli -- --provider fallback --model test-local
```

手动 TUI 检查：

```bash
cargo run -p viden-cli -- --tui --provider fallback --model test-local
```

Lane 分支额外检查：

```bash
/lane run printf hello
/lane inspect <id>
```

## 推荐执行顺序

1. 先提交当前设计/规划文档。
2. 实现 `codex/tui-main-screen`。
3. CI 通过后合并。
4. 实现 `codex/tui-lane-runtime`。
5. CI 通过后合并。
6. 只有当 lane inspect 可靠后，再开第三分支接 `codex`/`claude` adapter。

## 后续计划候选

- `codex/tui-screen-registry`：`MAIN` / `AGENTS` / `OPS` 状态与副屏窗口生命周期。
- `codex/tui-external-adapters`：task envelopes 与 `codex`/`claude` presets。
- `codex/tui-lane-worktrees`：per-lane worktree 隔离和验收流。
- `codex/tui-terminal-attach`：tmux 或 PTY attach/detach 原型。
