# RoboCode 用户指南

英文版： [user-guide.md](user-guide.md)

本文档说明 RoboCode `0.1.10` 已经真实支持的用户功能。

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
robocode --help
```

从 release archive 运行：

```bash
robocode-cli --version
robocode-cli --provider fallback --model test-local
```

启动主 cockpit：

```bash
robocode-cli --tui --provider deepseek --model deepseek-v4-flash
```

直接启动副屏：

```bash
robocode-cli --tui-screen side-1 --provider deepseek --model deepseek-v4-flash
robocode-cli --tui-screen side-2 --provider deepseek --model deepseek-v4-flash
```

## 启动参数

常用 flags：

- `--provider <name>`：选择 provider family。
- `--model <name>`：覆盖 model label。
- `--api-base <url>` 和 `--api-key <value>`：覆盖 provider connection。
- `--provider-plugin-dir <dir>`：增加动态 provider plugin 目录。
- `--permissions <mode>`：设置默认权限模式。
- `--session-home <dir>`：覆盖 transcript/index home。
- `--request-timeout <seconds>` 和 `--max-retries <n>`：调整 provider HTTP 行为。
- `--config <path>`：加载显式 TOML 配置。
- `--resume [id|latest]`：启动时恢复历史 session。
- `--tui`：启动主 cockpit。
- `--tui-screen <main|side-1|side-2>`：启动指定屏幕。
- `--tui-theme <aurora-cyan|ember-gold|plasma-violet|monochrome-ice>`：选择内置主题。

用于视觉 review 的 preview flags：

- `--tui-preview`
- `--tui-preview-idle`
- `--tui-preview-command-palette`
- `--tui-preview-lane`
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
permission_mode = "acceptEdits"
request_timeout_secs = 120
max_retries = 2

[providers.deepseek]
api_base = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
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
/provider list
/provider doctor [provider-id]
/provider reload
/provider use <provider-id> [model]
/model [name]
```

`fallback` 适合离线 smoke test，不会调用远程模型。

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
- `Ctrl-R`：重新生成。
- `Ctrl-N`：新任务。
- `?`：打开帮助。
- `Esc` 或 `Ctrl-C`：退出。
- `/quit` 或 `/exit`：从命令输入退出。
- `/`：打开命令提示。
- 审批弹窗：`y` 通过，`n` 拒绝，`d` 聚焦 diff，`Tab` / 方向键移动焦点。

## Slash Commands

Runtime：

```text
/help
/status
/config
/doctor
/permissions [mode]
/plan [on|off]
/test <command>
```

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
/memory project
/memory session
/memory suggest <content>
/memory confirm <memory-id>
/memory reject <memory-id>
/memory prune <memory-id>
/memory add <content>
/memory export
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
/lane claude <task>
/lane run <command>
/lane ask <tool> <task>
/lane inspect <id>
/lane stop <id>
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
tool grep pattern=SessionEngine path=robocode-core/src
```

## 权限模式

Permission modes：

- `default`：safe reads 允许，mutating actions 询问。
- `acceptEdits`：文件编辑更积极地接受，shell/Git mutation 仍走策略。
- `plan`：拒绝 mutating actions，适合只读规划。
- `dontAsk` 和 `bypassPermissions`：用于可信本地工作流。

权限路径覆盖文件变更、shell、Git、workflow task/memory 变更，以及 write-capable delegated Codex jobs。

## Sessions、Tasks 和 Memory

Sessions 以 JSONL transcript 保存，并用可重建的 SQLite index 加速查询。用 `/sessions` 和 `/resume` 恢复历史工作。

Tasks 和 memory 以 workflow events 保存。assistant suggested project memory 必须显式 confirm 后才会成为 active project memory。

`/task resume-context` 会把 task 和 memory 状态组合成可恢复的项目上下文快照。

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

`0.1.10` 通过了：

- full local release smoke with DeepSeek；
- clippy-as-gate；
- workspace tests；
- deterministic TUI screenshots；
- fallback CLI smoke；
- Codex app-server protocol fixture；
- app-server write guard；
- lane operator-loop smoke；
- package archive smoke；
- GitHub release asset validation；
- Homebrew formula validation。

证据路径和 release asset 名称见 [0.1.10 状态](release-0.1.10-status.zh-CN.md)。

## 问题反馈

提交 GitHub issue 时建议包含：

- RoboCode 版本和安装方式；
- OS 和终端应用；
- provider/model；
- 命令和复现步骤；
- 已移除密钥的截图或日志。
