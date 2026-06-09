# RoboCode User Guide

Chinese version: [user-guide.zh-CN.md](user-guide.zh-CN.md)

This guide describes the user-facing features that are available in the current
RoboCode development line.

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

`robocode-cli` starts the main TUI by default. A clean session opens on the
focused welcome composer first; it does not auto-submit or auto-open setup. Use
`Ctrl-P` for commands, or submit one of these entries when you want
provider/model setup. Slash setup commands keep the welcome surface active; the
full cockpit appears after the first normal task prompt.

```text
/setup
/setup provider
/connect
/models
```

Start the main cockpit with explicit startup overrides:

```bash
robocode-cli --provider deepseek --model deepseek-v4-flash
```

Use `--no-tui` when you need the legacy line REPL.

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
- `--permissions <level>`: set default permission level.
- `--session-home <dir>`: override transcript/index home.
- `--request-timeout <seconds>` and `--max-retries <n>`: tune provider HTTP
  behavior.
- `--config <path>`: load an explicit TOML config.
- `--resume [id|latest]`: resume a prior session at startup.
- `--tui`: start the main cockpit. This is the default.
- `--no-tui`: start the legacy line REPL.
- `--tui-screen <main|side-1|side-2>`: start a specific screen.
- `--tui-theme <aurora-cyan|ember-gold|plasma-violet|monochrome-ice>`: select
  a built-in theme.

Preview flags for visual review:

- `--tui-preview`
- `--tui-preview-idle` for the first-launch welcome composer
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

The TUI setup path is panel-first after you submit the command. `/connect`,
`/provider`, `/setup provider`, and `/settings provider` open a provider picker;
`Enter` selects the highlighted supplier, opens API-key entry when that supplier
needs one, and then opens the provider config action panel. That panel can
change the API key, clear the current process key, run provider doctor, or open
the provider-scoped model picker. Selecting a model from that provider-scoped
picker saves the provider/model, runs provider doctor, and writes the readiness
result back into the transcript. `/models`, `/model`, `/setup model`, and
`/settings model` open a provider-grouped model picker; it only shows providers
that have been configured/activated in provider settings. For configured
providers, the picker includes active, favorite, default, and known models.
Choosing a model applies the provider/model switch immediately. API keys are
masked in the TUI and RoboCode saves the env var name, not the raw key.

Typed commands still use compact completion while you are editing, so a large
selector does not steal the composer before you press Enter. Direct commands
such as `/settings provider <provider-id> ...`, `/models <provider-id> <model>`,
and `/model <model>` remain available for scripts and advanced users. Provider
failures are classified
into recovery classes such as missing key, auth, rate limit, timeout, context
overflow, compatibility, and model unavailable; the recovery prompt includes
concrete commands to open doctor, switch model/provider, retry later, or use
fallback. When a provider rejects a request as too large, RoboCode records a
compaction note, retries once with a smaller provider request view, and keeps the
full local transcript intact for audit.

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

Provider commands:

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

`fallback` is useful for offline smoke tests. It does not call a remote model.

Live provider smoke tests use the same runtime path as a normal non-TUI request
and store transcript evidence:

```bash
scripts/provider-live-smoke.sh --provider deepseek --model deepseek-v4-flash
scripts/provider-live-smoke.sh --provider dashscope-coding-plan --model qwen3.6-plus
scripts/provider-live-smoke.sh --provider dashscope-tokenplan --model qwen3.6-plus
scripts/deepseek-dev-scenario-smoke.sh --model deepseek-v4-flash
```

`dashscope-coding-plan` uses `DASHSCOPE_CODING_PLAN_API_KEY`.
`dashscope-tokenplan` uses `DASHSCOPE_API_KEY`.
The DeepSeek development scenario is billable. It writes `usage.json` and
`summary.md` with input/output/total token counts and an estimated CNY cost
using the configured model price.

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
- `Ctrl-R`: reload the latest user prompt into the composer for regeneration.
- `Ctrl-N`: start a new `/task add ...` command.
- `?`: open help when the composer is empty.
- `Esc` or `Ctrl-C`: exit.
- `/quit` or `/exit`: exit from command input.
- `/`: open command suggestions. Use `Up` / `Down`, `Tab`, `Enter`, or click a
  visible suggestion row. Long suggestion lists scroll as the selection moves
  so keyboard and mouse behavior stay aligned.
- Transcript history: use `PageUp` / `PageDown` or the mouse wheel to browse
  older transcript rows. `Ctrl-Home` jumps to the oldest visible history and
  `Ctrl-End` returns to the live tail. When history mode is active, the
  transcript panel badge changes from `live session` to `history N`.
- Approval modal: `y` approve, `n` deny, `d` focus diff, `Tab` / arrows move
  focus. Diff focus now shows the prompt's actual evidence/preview lines when
  they are available instead of a decorative placeholder.
- Active provider turns keep the TUI event loop alive. The transcript live tail
  shows a prominent `LIVE WORK` strip directly below the latest conversation
  content, with phase, signal, and next-action guidance while RoboCode works.
  Provider thinking does not show fake progress percentages. The status bar,
  elapsed time, lane snapshots, and pending approval bridge can repaint while
  the provider worker runs. `Ctrl-C` requests cancellation; an already in-flight
  HTTP request may still complete before the provider returns.
- Streaming-capable HTTP providers request server-sent streaming during TUI
  turns. Text deltas are appended to a temporary assistant transcript row while
  the provider is still responding, then replaced by the canonical transcript
  event when the turn completes.
- During an active provider turn, the composer stays editable. Press `Enter` to
  queue the draft as the next prompt; RoboCode clears the composer immediately
  and runs queued prompts after the current turn finishes. If the active turn
  fails, the first queued prompt is restored to the composer.

## Slash Commands

Runtime:

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

`/plan` is an immediate TUI command: it switches to Plan mode and returns the
composer to normal input without starting a provider turn. On the first-launch
welcome screen, `/plan` keeps the welcome composer visible until a real task
prompt starts the session.

Plan mode is for planning product requirements, architecture, implementation
approach, test strategy, and development steps. It may read and inspect the
project, but it does not write code, modify files, run mutating shell/Git/workflow
actions, or persist the plan. Plan output stays in the transcript until you
confirm implementation and switch back to Build with `ask`, `auto_edit`,
`read_only`, or `full_access`.

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

`/agent doctor [id]` reports each adapter capability record: readiness,
mutation mode, evidence mode, config source, and known limits. Use it before
trusting a delegated Codex/Claude/template/tmux/ACP lane.

Inside the TUI, `/lane` opens a centered action selector. It lists lane launch
commands and adds id-specific inspect, timeline, diff, and artifacts actions for
active lanes, so you can operate lanes without memorizing their ids.

`/lane codex-review <task>` is the P0 read-only Codex trust-loop path. It writes
an envelope, launches `codex review --uncommitted` when Codex is available (or a
`ROBOCODE_LANE_CODEX_REVIEW_TEMPLATE` override when configured), and stores the
result in the same log/timeline/evidence model as other lanes.

`/lane tmux <id>` now preflights the default tmux/Claude path before marking a
lane attached. Missing `tmux` or `claude` produces a setup-needed timeline event
instead of a false attached state; custom templates can be supplied with
`ROBOCODE_LANE_TMUX_TEMPLATE` and `ROBOCODE_LANE_TMUX_COMMAND_TEMPLATE`.

`/lane inspect <id>` reports the lane status, command, exit code, log, artifacts,
envelope, timeline, decision, next action, and changed files. `/lane timeline
<id>` prints the ordered event stream for review/apply/debug evidence.

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

## Modes And Permissions

RoboCode separates work intent from trust level:

- Work Mode: `build` for implementation, `plan` for requirements,
  architecture, implementation approach, test strategy, and development plans.
- Permission Level: `ask`, `auto_edit`, `read_only`, or `full_access`.

`/plan` is a shortcut to Plan work mode with Read Only permission level. It
does not write code. `/permissions` changes the trust boundary only; it does not
change provider/model or the work mode.

Compatibility aliases such as `default`, `acceptEdits`, `bypassPermissions`,
and `dontAsk` are still accepted by the parser for older configs and scripts,
but new docs and UI use the canonical permission-level names.

The permission path covers file mutation, shell, Git, workflow task/memory
changes, and write-capable delegated Codex jobs.

## Sessions, Tasks, And Memory

Sessions are stored as JSONL transcripts with a rebuildable SQLite index. Use
`/sessions` and `/resume` to continue previous work.

Tasks and memory are stored as workflow events. Assistant-suggested project
memory must be confirmed before it becomes active project memory.

`/task resume-context` combines task and memory state into a resumable project
context snapshot.

`/brief <task goal>` creates a lightweight active task brief under
`.robocode/briefs/active.md`; `/spec` is an alias. When an active brief exists,
provider ContextBundles, lane envelopes, and side-2 ops can reference it.
`/brief steering init` creates minimal `.robocode/steering/` templates for
project conventions, architecture, and workflows.

`/context` shows the latest provider ContextBundle, including the v1 policy,
source priorities, token estimates, omitted sources, and compaction notes. It is
read-only and does not expose raw secret values.

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

For `0.1.16`, the release was verified with:

- clippy-as-gate;
- workspace tests;
- deterministic TUI screenshots;
- fallback CLI smoke;
- Codex app-server protocol fixture;
- app-server write guard;
- lane operator-loop smoke;
- GitHub release asset validation;
- Homebrew formula validation.

See [0.1.16 Status](release-0.1.16-status.md) for evidence paths and release
asset names.

## Feedback

Open a GitHub issue with:

- RoboCode version and install method;
- OS and terminal app;
- provider/model;
- command and reproduction steps;
- screenshots or logs with secrets redacted.
