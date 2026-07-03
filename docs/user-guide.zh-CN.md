# RoboCode 用户指南

英文版： [user-guide.md](user-guide.md)

本文档说明当前 RoboCode development line 已经真实支持的用户功能。

## 心智模型

RoboCode 分三层：

- CLI runtime：加载配置、选择 provider、记录 transcript、运行工具、执行权限检查。
- Cockpit TUI：展示对话、当前操作、审批弹窗、workspace 状态、任务、provider 健康度和副屏控制。
- Operator surfaces：slash commands、agent lanes、副屏、任务、记忆、诊断和发布证据。

所有 mutating action 都应该进入 shared runtime path，这样才能被记录、被权限检查，并在之后恢复上下文。

## 安装与启动

用 Homebrew 安装：

```bash
brew install wikieden/tap/robocode
viden-cli --help
```

从 release archive 运行：

```bash
viden-cli --version
viden-cli --provider fallback --model test-local
```

`viden-cli` 默认启动主 TUI。干净会话会先进入聚焦的 welcome 输入界面，
不会自动提交命令或自动打开 setup。需要配置 provider/model 时，用 `Ctrl-P`
打开命令，或直接提交下面这些入口。slash 设置命令执行后仍停留在 welcome；
提交第一个普通任务 prompt 后才进入完整 cockpit。

```text
/setup
/setup provider
/connect
/models
```

带显式启动参数进入主 cockpit：

```bash
viden-cli --provider deepseek --model deepseek-v4-flash
```

需要旧版行式 REPL 时使用 `--no-tui`。

直接启动副屏：

```bash
viden-cli --tui-screen side-1 --provider deepseek --model deepseek-v4-flash
viden-cli --tui-screen side-2 --provider deepseek --model deepseek-v4-flash
```

## 启动参数

常用 flags：

- `--provider <name>`：选择 provider family。
- `--model <name>`：覆盖 model label。
- `--api-base <url>` 和 `--api-key <value>`：覆盖 provider connection。
- `--provider-plugin-dir <dir>`：增加动态 provider plugin 目录。
- `--permissions <level>`：设置默认 permission level。
- `--session-home <dir>`：覆盖 transcript/index home。
- `--request-timeout <seconds>` 和 `--max-retries <n>`：调整 provider HTTP 行为。
- `--config <path>`：加载显式 TOML 配置。
- `--resume [id|latest]`：启动时恢复历史 session。
- `--tui`：启动主 cockpit。现在这是默认行为。
- `--no-tui`：启动旧版行式 REPL。
- `--tui-screen <main|side-1|side-2>`：启动指定屏幕。
- `--tui-theme <aurora-cyan|ember-gold|plasma-violet|monochrome-ice>`：选择内置主题。

用于视觉 review 的 preview flags：

- `--tui-preview`
- `--tui-preview-idle`：首次进入的 welcome 输入界面
- `--tui-preview-command-palette`
- `--tui-preview-live-turn`
- `--tui-preview-resize`
- `--tui-preview-cjk-input`
- `--tui-preview-lane`
- `--tui-preview-setup-wizard`
- `--tui-preview-provider-selector`
- `--tui-preview-model-selector`
- `--tui-preview-lane-selector`
- `--tui-preview-side`
- `--tui-preview-side-2`

每个 preview flag 都有对应的 `-ansi` 版本。

## 配置

RoboCode 按顺序加载：

1. 平台默认配置路径；
2. `.robocode/config.toml`；
3. 环境变量；
4. CLI overrides。

示例：

```toml
provider = "deepseek"
model = "deepseek-v4-flash"
permission_mode = "auto_edit"
request_timeout_secs = 120
max_retries = 2

[providers.deepseek]
api_base = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
default_model = "deepseek-v4-flash"
models = ["deepseek-v4-flash", "deepseek-v4-pro"]
favorite_models = ["deepseek-v4-pro"]
```

TUI 设置路径在提交命令后进入独立面板：`/connect`、`/provider`、`/setup provider`、`/settings provider` 会打开供应商选择面板；`Enter` 选择供应商，缺 key 时进入 API key 输入面板，然后进入 provider config 动作页。这个动作页可以更换 API key、清除当前进程里的 key、运行 provider doctor，或打开该 provider 的模型选择面板。在 provider-scoped 模型面板里选中模型后，会保存 provider/model、自动运行 provider doctor，并把 readiness 结果写回 transcript。`/models`、`/model`、`/setup model`、`/settings model` 会打开按供应商分组的模型选择面板，只显示已经配置过的 provider；对已配置 provider，会显示 active、favorite、default 和 known models。选中模型后会立即切换 provider/model。API key 在界面里脱敏展示，RoboCode 只保存环境变量名，不保存明文 key。直接命令 `/settings provider <provider-id> ...`、`/models <provider-id> <model>`、`/model <model>` 仍保留给脚本和高级用户。Provider 失败会被分类为 missing key、auth、rate limit、timeout、context overflow、compatibility 或 model unavailable 等 recovery class，并给出打开 doctor、切换 model/provider、稍后重试或使用 fallback 的具体命令。当 provider 因请求过大拒绝执行时，RoboCode 会记录压缩说明，用更小的 provider request view 自动重试一次，同时保留完整本地 transcript 作为审计记录。

```text
/settings
/setup
/connect
/provider
/model <model>
/models
/settings provider <provider-id> [model]
/settings provider <provider-id> key-env <ENV_NAME>
/settings provider <provider-id> endpoint <url>
/settings provider <provider-id> default-model <model>
/settings provider <provider-id> enable-model <model>
/settings provider <provider-id> favorite-model <model>
/settings provider <provider-id> models <model> [model...]
/settings permissions <level>
/settings theme <name>
/settings save
```

常用环境变量：

- `ROBOCODE_PROVIDER`、`ROBOCODE_MODEL`
- `ROBOCODE_API_BASE`、`ROBOCODE_API_KEY`
- `ROBOCODE_PROVIDER_PLUGIN_DIRS`
- `ROBOCODE_PERMISSION_MODE`、`ROBOCODE_SESSION_HOME`
- `ROBOCODE_REQUEST_TIMEOUT_SECS`、`ROBOCODE_MAX_RETRIES`
- `ROBOCODE_CONFIG`
- `ROBOCODE_SCREEN_LAUNCH_TEMPLATE`
- `ROBOCODE_LANE_CODEX_TEMPLATE`、`ROBOCODE_LANE_CLAUDE_TEMPLATE`
- `ANTHROPIC_API_KEY`、`OPENAI_API_KEY`、`DEEPSEEK_API_KEY`、`DEEPSEEK_API_BASE`

## Providers

用 `/provider list` 查看 runtime registry。当前内置 registry 包括：

- `anthropic`
- `deepseek`
- `deepseek-anthropic`
- `dashscope-coding-plan`
- `dashscope-coding-plan-anthropic`
- `dashscope-tokenplan`
- `dashscope-tokenplan-anthropic`
- `fallback`
- `groq`
- `kimi`
- `mistral`
- `ollama`
- `openai`
- `openai-compatible`
- `openrouter`
- `qwen`
- `together`
- `volcengine`
- `zhipu`

Provider commands：

```text
/provider
/connect
/provider <provider-id> [model]
/provider list
/provider doctor [provider-id]
/provider reload
/provider use <provider-id> [model]
/model [name]
/models
/models <provider-id> <model>
/settings
/settings permissions [mode]
/settings theme [name]
/setup
```

`fallback` 适合离线 smoke test，不会调用远程模型。

真实 provider smoke 会走和普通 non-TUI 请求相同的 runtime path，并保存 transcript 证据：

```bash
scripts/provider-live-smoke.sh --provider deepseek --model deepseek-v4-flash
scripts/provider-live-smoke.sh --provider dashscope-coding-plan --model qwen3.6-plus
scripts/provider-live-smoke.sh --provider dashscope-tokenplan --model qwen3.6-plus
scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash
```

`dashscope-coding-plan` 使用 `DASHSCOPE_CODING_PLAN_API_KEY`。
`dashscope-tokenplan` 使用 `DASHSCOPE_API_KEY`。
DeepSeek 开发场景 smoke 会产生真实费用，会写出 `usage.json` 和 `summary.md`，
记录 input/output/total token 以及按当前模型单价估算的 CNY 费用。

## TUI 屏幕

主 cockpit 展示：

- transcript 和 tool result stream；
- operation center 当前状态；
- approval modal；
- workspace snapshot；
- active tasks；
- diagnostics；
- provider health；
- recent files；
- composer 和 status bar。

副屏：

- `side-1`：Agent lane monitor，展示 lane state、attach hints、output tail 和 artifacts。
- `side-2`：ops/evidence monitor，展示最近 test、diff、provider、extension 和 task evidence。

在 TUI 中打开副屏：

```text
/screen side-1
/screen side-2
/screen list
/screen close side-1
```

可以通过 `ROBOCODE_SCREEN_SIDE_1_LAUNCH_TEMPLATE`、`ROBOCODE_SCREEN_SIDE_2_LAUNCH_TEMPLATE`
或 `ROBOCODE_SCREEN_LAUNCH_TEMPLATE`，把副屏交给终端应用、tmux 或显示器摆放脚本启动。

## TUI 操作

- `Enter`：提交输入区。
- `Ctrl-J`：显式发送。
- `Ctrl-K`：清空输入区。
- `Ctrl-R`：把最近一次用户输入重新放回输入区，便于重新生成。
- `Ctrl-N`：开始一个新的 `/task add ...` 命令。
- `?`：输入区为空时打开帮助。
- `Esc` 或 `Ctrl-C`：退出。
- `/quit` 或 `/exit`：从命令输入退出。
- `/`：打开命令提示。可以用 `Up` / `Down`、`Tab`、`Enter`，也可以点击可见提示行。
- 长命令提示列表会随着键盘选择滚动，保证选中行可见，鼠标点击和键盘选择看到的是同一组行。
- transcript 历史：用 `PageUp` / `PageDown` 或鼠标滚轮浏览更早的 transcript 行。
  `Ctrl-Home` 跳到最早可见历史，`Ctrl-End` 回到实时尾部。进入历史模式时，
  transcript 面板角标会从 `live session` 变成 `history N`。
- 审批弹窗：`y` 通过，`n` 拒绝，`d` 聚焦 diff，`Tab` / 方向键移动焦点。
  diff 焦点会优先展示本次审批 prompt 里的真实 evidence / preview lines，而不是装饰性占位内容。
- provider turn 运行中，TUI event loop 会继续工作；transcript live tail 会在最近
  对话内容下面显示更醒目的 `LIVE WORK` strip，展示 phase、signal 和下一步 guidance。
  Provider thinking 不再显示假进度百分比。状态栏、elapsed time、lane snapshot 和审批桥接
  都可以持续刷新。`Ctrl-C` 会请求取消；但已经发出的 HTTP provider 请求仍可能先完成。
- 支持 streaming 的 HTTP provider 在 TUI turn 中会请求 server-sent streaming。
  模型返回的 text delta 会先追加到临时 assistant transcript 行里；turn 完成后，
  再替换为正式持久化 transcript event。
- provider turn 尚未结束时，输入区仍可编辑。按 `Enter` 会把当前草稿排成下一条
  prompt，RoboCode 会立即清空输入区，并在当前 turn 结束后自动执行队列里的 prompt。
  如果当前 turn 失败，会把第一条已排队 prompt 放回输入区，方便你修改后重试。

## Slash Commands

Runtime：

```text
/help
/status
/config
/doctor
/mode [build|plan]
/permissions [level]
/plan [on|off]
/test <command>
```

`/plan` 是即时 TUI 命令：它切换到 Plan 模式，完成后会直接回到可输入状态，
不会启动一次 provider turn。在首次启动的 welcome 屏里，`/plan` 会继续停留在
welcome 输入界面，直到提交真正任务 prompt 后才进入完整会话页。

Plan 模式用于规划产品需求、架构、实现方案、测试策略和开发计划。它可以读取和分析项目，
但不会写代码、修改文件、执行 mutating shell/Git/workflow 操作，计划内容先输出在 transcript
里。确认要实现后，再切回 Build，并选择 `ask`、`auto_edit`、`read_only` 或
`full_access` 作为 permission level。

Sessions：

```text
/sessions
/resume latest
/resume #<index>
/resume <session-id-prefix>
/diff
```

Repository 和 Web：

```text
/git status
/git diff [path]
/git branch
/git add [--all|-A] <path...>
/git restore [--staged] [--source <ref>] <path...>
/git switch <branch> [--create]
/git commit [--all] <message>
/git push [branch] | [remote branch] [--set-upstream|-u]
/git stash <list|push|pop|drop>
/git worktree <list|add|remove>
/web search <query> [--limit <n>] [--site <domain>]
/web fetch <url> [--max-bytes <n>] [--raw]
```

Code intelligence：

```text
/lsp status
/lsp diagnostics <path>
/lsp symbols <path>
/lsp references <path> <line> <character>
```

任务和记忆：

```text
/tasks
/task add <title>
/task view <task-id>
/task update <task-id> <title>
/task status <task-id> <todo|in_progress|blocked|done|archived>
/task link <task-id> <depends-on-id>
/task block <task-id> <reason>
/task unblock <task-id>
/task archive <task-id>
/task restore <task-id>
/task resume-context
/brief <task goal>
/spec <task goal>
/brief show
/brief clear
/brief steering init
/brief steering show
/memory project
/memory session
/memory suggest <content>
/memory confirm <memory-id>
/memory reject <memory-id>
/memory prune <memory-id>
/memory add <content>
/memory export
/context
```

Agents 和 lanes：

```text
/agent list
/agent doctor [id]
/agent run codex [--write|--app-server] <task>
/agent review codex [--base <ref>] [prompt]
/agent challenge codex [prompt]
/agent status
/agent result <id>
/agent cancel <id>
/agent logs <id>
/lane codex <task>
/lane codex-review <task>
/lane claude <task>
/lane run <command>
/lane ask <tool> <task>
/lane inspect <id>
/lane timeline <id>
/lane diff <id>
/lane artifacts <id>
/lane stop <id>
/lane retry <id>
/lane attach <id>
/lane tmux <id>
/lane pty <id>
/lane send <id> <input>
/lane detach <id>
/lane accept <id>
/lane revise <id>
/lane discard <id>
/lane apply <id>
/lane resolve <id>
/lane archive <id>
/lane cleanup <id>
```

`/agent doctor [id]` 会输出每个 adapter 的 capability record：就绪状态、
变更模式、证据来源、配置来源和已知限制。把 Codex/Claude/template/tmux/ACP
lane 交出去之前，先用它确认这条 lane 是否可信。

在 TUI 中，`/lane` 会打开居中的动作 selector。它会列出 lane 启动命令；已有
lane 时，还会补充带 id 的 inspect、timeline、diff 和 artifacts 动作，这样不用
靠记忆手敲 lane id。

`/lane codex-review <task>` 是 P0 只读 Codex 信任闭环路径。它会写入
envelope；当 Codex 可用时启动 `codex review --uncommitted`，也支持
`ROBOCODE_LANE_CODEX_REVIEW_TEMPLATE` 覆盖；最终结果进入和其他 lane 一样的
log/timeline/evidence 模型。

`/lane tmux <id>` 现在会在标记 attached 之前检查默认 tmux/Claude 路径。
如果缺少 `tmux` 或 `claude`，会记录 setup-needed timeline 事件，而不是误报
attached；自定义路径可以通过 `ROBOCODE_LANE_TMUX_TEMPLATE` 和
`ROBOCODE_LANE_TMUX_COMMAND_TEMPLATE` 配置。

`/lane inspect <id>` 会输出 lane 状态、命令、退出码、日志、产物、envelope、
timeline、决策、下一步动作和变更文件。`/lane timeline <id>` 会打印有序事件流，
用于 review/apply/debug 证据追踪。

Extensions：

```text
/extensions list
/extensions doctor
/mcp list
/mcp doctor
/skills list
/skills list --all
```

## 内置工具

模型 tool calls 和 fallback `tool ...` syntax 可以使用：

- `read_file`
- `write_file`
- `edit_file`
- `glob`
- `grep`
- `shell`
- `web_search`
- `web_fetch`
- `git_status`
- `git_diff`
- `git_branch`
- `git_switch`
- `git_add`
- `git_restore`
- `git_commit`
- `git_push`
- `git_stash_list`
- `git_stash_push`
- `git_stash_pop`
- `git_stash_drop`
- `git_worktree_list`
- `git_worktree_add`
- `git_worktree_remove`
- `lsp_diagnostics`
- `lsp_symbols`
- `lsp_references`

Fallback tool syntax 示例：

```text
tool read_file path=Cargo.toml
tool grep pattern=SessionEngine path=viden-runtime/src
```

## 模式和权限

RoboCode 把工作意图和信任等级分开：

- Work Mode：`build` 用于实现，`plan` 用于产品需求、架构、实现方案、测试策略和开发计划。
- Permission Level：`ask`、`auto_edit`、`read_only` 或 `full_access`。

`/plan` 是进入 Plan work mode 并切到 Read Only permission level 的快捷入口，不写代码。
`/permissions` 只改变信任边界，不改变 provider/model，也不改变 work mode。

`default`、`acceptEdits`、`bypassPermissions` 和 `dontAsk` 等兼容 alias 仍会被 parser 接受，
用于旧配置和脚本；新的文档和 UI 使用 canonical permission-level 名称。

权限路径覆盖文件变更、shell、Git、workflow task/memory 变更，以及 write-capable delegated
Codex jobs。

## Sessions、Tasks 和 Memory

Sessions 以 JSONL transcript 保存，并用可重建的 SQLite index 加速查询。用 `/sessions` 和 `/resume` 恢复历史工作。

Tasks 和 memory 以 workflow events 保存。assistant suggested project memory 必须显式 confirm 后才会成为 active project memory。

`/task resume-context` 会把 task 和 memory 状态组合成可恢复的项目上下文快照。

`/brief <task goal>` 会在 `.robocode/briefs/active.md` 创建轻量 active task
brief；`/spec` 是别名。有 active brief 时，provider ContextBundle、lane
envelope 和 side-2 ops 都可以引用它。`/brief steering init` 会创建最小
`.robocode/steering/` 模板，用于项目约定、架构和工作流。

`/context` 会显示最近一次 provider turn 使用的 ContextBundle，包括 v1 policy、source priority、token 估算、被省略的 sources 和 compaction notes。它用于回答“这次请求到底带了哪些上下文、哪些被预算策略裁掉了”。

## Agent Lanes

Agent lanes 让 RoboCode 监督外部工具，而不是假装它们是原生模型调用。当前 adapters 包括：

- Codex CLI / app-server entrypoints。
- Claude Code command template。
- 自定义 template agents。
- Shell lanes。
- Tmux lane attachment。
- Embedded PTY lane。
- 实验性 ACP command surface。

Lane artifacts 会写入 `.robocode/lanes/`，这样主 TUI 和副屏可以展示 next actions、output tails、decision、apply result 和 conflicts。

## Extension 边界

RoboCode 现在能发现这些 extension surfaces：

- provider plugin directories；
- agent adapters；
- MCP config files；
- project/user/legacy skill roots 下的 local skills。

当前边界：MCP-backed tools 已可见，但尚未接入 mutation permission path。Skills 是任务 recipe，不是直接工具。

## Release Evidence

`0.1.16` 发布验证覆盖：

- clippy-as-gate；
- workspace tests；
- deterministic TUI screenshots；
- fallback CLI smoke；
- Codex app-server protocol fixture；
- app-server write guard；
- lane operator-loop smoke；
- GitHub release asset validation；
- Homebrew formula validation。

证据路径和 release asset 名称见 [0.1.16 状态](release-0.1.16-status.zh-CN.md)。

## 问题反馈

提交 GitHub issue 时建议包含：

- RoboCode 版本和安装方式；
- OS 和终端应用；
- provider/model；
- 命令和复现步骤；
- 已移除密钥的截图或日志。
