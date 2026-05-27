# RoboCode User Guide

Chinese version: [user-guide.zh-CN.md](user-guide.zh-CN.md)

This guide describes the user-facing features that are available in RoboCode
`0.1.10`.

## Mental Model

RoboCode has three layers:

- CLI runtime: loads config, selects a provider, records transcripts, runs tools,
  and applies permissions.
- Cockpit TUI: renders the conversation, current operation, approval prompts,
  workspace state, tasks, provider health, and side-screen controls.
- Operator surfaces: slash commands, agent lanes, side screens, tasks, memory,
  diagnostics, and release evidence.

All mutating actions should enter through the shared runtime path so they can be
recorded and permission-checked.

## Install And Start

Install with Homebrew:

```bash
brew install wikieden/tap/robocode
robocode --help
```

Run from a release archive:

```bash
robocode-cli --version
robocode-cli --provider fallback --model test-local
```

Start the main cockpit:

```bash
robocode-cli --tui --provider deepseek --model deepseek-v4-flash
```

Start a side screen directly:

```bash
robocode-cli --tui-screen side-1 --provider deepseek --model deepseek-v4-flash
robocode-cli --tui-screen side-2 --provider deepseek --model deepseek-v4-flash
```

## Startup Flags

Common flags:

- `--provider <name>`: choose provider family.
- `--model <name>`: override model label.
- `--api-base <url>` and `--api-key <value>`: override provider connection.
- `--provider-plugin-dir <dir>`: add a dynamic provider plugin directory.
- `--permissions <mode>`: set default permission mode.
- `--session-home <dir>`: override transcript/index home.
- `--request-timeout <seconds>` and `--max-retries <n>`: tune provider HTTP
  behavior.
- `--config <path>`: load an explicit TOML config.
- `--resume [id|latest]`: resume a prior session at startup.
- `--tui`: start the main cockpit.
- `--tui-screen <main|side-1|side-2>`: start a specific screen.
- `--tui-theme <aurora-cyan|ember-gold|plasma-violet|monochrome-ice>`: select
  a built-in theme.

Preview flags for visual review:

- `--tui-preview`
- `--tui-preview-idle`
- `--tui-preview-command-palette`
- `--tui-preview-lane`
- `--tui-preview-side`
- `--tui-preview-side-2`

Each preview also has an ANSI variant ending in `-ansi`.

## Configuration

RoboCode loads:

1. platform config path;
2. `.robocode/config.toml`;
3. environment variables;
4. CLI overrides.

Example:

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

Useful environment variables:

- `ROBOCODE_PROVIDER`, `ROBOCODE_MODEL`
- `ROBOCODE_API_BASE`, `ROBOCODE_API_KEY`
- `ROBOCODE_PROVIDER_PLUGIN_DIRS`
- `ROBOCODE_PERMISSION_MODE`, `ROBOCODE_SESSION_HOME`
- `ROBOCODE_REQUEST_TIMEOUT_SECS`, `ROBOCODE_MAX_RETRIES`
- `ROBOCODE_CONFIG`
- `ROBOCODE_SCREEN_LAUNCH_TEMPLATE`
- `ROBOCODE_LANE_CODEX_TEMPLATE`, `ROBOCODE_LANE_CLAUDE_TEMPLATE`
- `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`,
  `DEEPSEEK_API_BASE`

## Providers

Use `/provider list` to inspect the runtime registry. The current built-in
registry includes:

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

Provider commands:

```text
/provider
/provider list
/provider doctor [provider-id]
/provider reload
/provider use <provider-id> [model]
/model [name]
```

`fallback` is useful for offline smoke tests. It does not call a remote model.

## TUI Screens

Main cockpit:

- transcript and tool result stream;
- operation center for current activity;
- approval modal;
- workspace snapshot;
- active tasks;
- diagnostics;
- provider health;
- recent files;
- composer and status bar.

Side screens:

- `side-1`: agent lane monitor with lane state, attach hints, output tail, and
  artifacts.
- `side-2`: ops/evidence monitor with recent test, diff, provider, extension,
  and task evidence.

Open side screens from the TUI:

```text
/screen side-1
/screen side-2
/screen list
/screen close side-1
```

Route side screens through a terminal app, tmux, or monitor placement script by
setting `ROBOCODE_SCREEN_SIDE_1_LAUNCH_TEMPLATE`,
`ROBOCODE_SCREEN_SIDE_2_LAUNCH_TEMPLATE`, or `ROBOCODE_SCREEN_LAUNCH_TEMPLATE`.

## TUI Controls

- `Enter`: submit composer.
- `Ctrl-J`: explicit send action.
- `Ctrl-K`: clear composer.
- `Ctrl-R`: regenerate.
- `Ctrl-N`: new task.
- `?`: open help.
- `Esc` or `Ctrl-C`: exit.
- `/quit` or `/exit`: exit from command input.
- `/`: open command suggestions.
- Approval modal: `y` approve, `n` deny, `d` focus diff, `Tab` / arrows move
  focus.

## Slash Commands

Runtime:

```text
/help
/status
/config
/doctor
/permissions [mode]
/plan [on|off]
/test <command>
```

Sessions:

```text
/sessions
/resume latest
/resume #<index>
/resume <session-id-prefix>
/diff
```

Repository and web:

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

Code intelligence:

```text
/lsp status
/lsp diagnostics <path>
/lsp symbols <path>
/lsp references <path> <line> <character>
```

Tasks and memory:

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

Agents and lanes:

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

Extensions:

```text
/extensions list
/extensions doctor
/mcp list
/mcp doctor
/skills list
/skills list --all
```

## Built-In Tools

Model tool calls and fallback `tool ...` syntax can use:

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

Example fallback tool syntax:

```text
tool read_file path=Cargo.toml
tool grep pattern=SessionEngine path=robocode-core/src
```

## Permissions

Permission modes:

- `default`: safe reads are allowed; mutating actions ask.
- `acceptEdits`: file edits can be accepted more aggressively while shell/Git
  mutation still follows policy.
- `plan`: mutating actions are denied; useful for read-only planning.
- `dontAsk` and `bypassPermissions`: available for trusted local workflows.

The permission path covers file mutation, shell, Git, workflow task/memory
changes, and write-capable delegated Codex jobs.

## Sessions, Tasks, And Memory

Sessions are stored as JSONL transcripts with a rebuildable SQLite index. Use
`/sessions` and `/resume` to continue previous work.

Tasks and memory are stored as workflow events. Assistant-suggested project
memory must be confirmed before it becomes active project memory.

`/task resume-context` combines task and memory state into a resumable project
context snapshot.

## Agent Lanes

Agent lanes let RoboCode supervise external tools without pretending they are
native model calls. Current adapters include:

- Codex CLI / app-server entrypoints.
- Claude Code via command template.
- Custom template agents.
- Shell lanes.
- Tmux lane attachment.
- Embedded PTY lane.
- Experimental ACP command surface.

Lane artifacts are written under `.robocode/lanes/` so the main TUI and side
screens can show next actions, output tails, decisions, apply results, and
conflicts.

## Extension Boundaries

RoboCode can discover extension surfaces today:

- provider plugin directories;
- agent adapters;
- MCP config files;
- local skills under project, user, and legacy skill roots.

Current boundary: MCP-backed tools are visible but not yet wired into the
mutation permission path. Skills are task recipes, not direct tools.

## Release Evidence

For `0.1.10`, the release was verified with:

- full local release smoke with DeepSeek;
- clippy-as-gate;
- workspace tests;
- deterministic TUI screenshots;
- fallback CLI smoke;
- Codex app-server protocol fixture;
- app-server write guard;
- lane operator-loop smoke;
- package archive smoke;
- GitHub release asset validation;
- Homebrew formula validation.

See [0.1.10 Status](release-0.1.10-status.md) for evidence paths and release
asset names.

## Feedback

Open a GitHub issue with:

- RoboCode version and install method;
- OS and terminal app;
- provider/model;
- command and reproduction steps;
- screenshots or logs with secrets redacted.
