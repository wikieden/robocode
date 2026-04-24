# RoboCode Architecture

## Workspace Layout

- `robocode-cli`: user-facing REPL and slash commands.
- `robocode-config`: config loading, merge precedence, and startup defaults.
- `robocode-core`: session engine and turn orchestration.
- `robocode-model`: provider abstraction, HTTP adapters, and tool-calling protocol translation.
- `robocode-tools`: builtin local tools and execution adapters.
- `robocode-permissions`: permission modes, rules, and approval decisions.
- `robocode-session`: JSONL transcripts plus SQLite indexing.
- `robocode-types`: shared domain types.
- `robocode-workflows`: project task, memory, resume-context, and workflow-log state.
- `robocode-lsp`: language-server configuration, protocol framing, semantic query execution, and result normalization.

The root workspace keeps `robocode-session` JSONL transcripts as the durable
source of truth. SQLite is a rebuildable index used for listing and resuming
sessions quickly.

## Configuration Model

Startup config is resolved through a fixed precedence chain:

1. CLI flags
2. Environment variables
3. Project-local `.robocode/config.toml`
4. Global config file
5. Built-in defaults

The resolved config currently covers:

- provider family
- model name
- API base URL
- API key
- permission mode
- session home
- request timeout
- retry count

This allows the engine and provider layer to stay free of ad hoc environment
lookups after startup.

## Main Execution Flow

1. CLI receives a line of user input.
2. `robocode-core` decides whether the line is a slash command, a direct tool
   request, or a normal model prompt.
3. Normal prompts are appended to the transcript and handed to the model
   provider.
4. Provider emits assistant text and/or tool calls.
5. Assistant tool calls are written into the in-memory conversation state so
   the next round-trip has a complete tool transcript.
6. Tool calls are routed through the permission engine.
7. If approval is required, the CLI prompts the user and returns the decision to
   the engine.
8. Tools execute through a shared registry.
9. Tool results are written to the transcript and reintroduced into the
   conversation history.
10. The engine loops until the provider finishes the turn.

This keeps every tool invocation on one shared path: validation, permission
decision, execution, transcript logging, and model reinjection all happen in
the same runtime flow.

## Transcript Schema

The canonical transcript is JSONL. Each line is one `TranscriptEntry` tagged by
type:

- `message`
- `tool_call`
- `tool_result`
- `permission`
- `command`
- `session_meta`

The transcript is append-only. SQLite stores derived summaries and can always be
rebuilt from JSONL.

Session metadata currently supports:

- project-scoped session listing
- `/sessions` output for the current repository
- `/resume latest`
- `/resume #<index>`
- `/resume <session-id-prefix>`

## Permission Model

Supported modes:

- `default`
- `acceptEdits`
- `bypassPermissions`
- `dontAsk`
- `plan`

Rules are grouped into allow, deny, and ask buckets. Additional working
directories expand the set of in-scope paths. File reads and searches can be
auto-allowed inside scope; mutations require approval unless mode or rule says
otherwise.

The permission engine also has a small set of behavior-specific exceptions. For
example, Git worktree operations can target paths outside the current repository
root, so those paths ask for approval instead of being treated as an automatic
out-of-scope deny.

## Provider Abstraction

The model layer exposes a provider trait that accepts:

- session id
- current model name
- conversation messages
- tool specs
- current permission mode

Providers return streamed or batched model events:

- assistant text
- tool calls
- end-of-turn

V1 includes a provider factory with these backend families:

- `anthropic`
- `openai`
- `openai-compatible`
- `ollama`
- `fallback`

The HTTP-backed providers use the system `curl` binary so the workspace remains
dependency-light and offline-compilable. The provider config includes request
timeouts and retry counts, and the HTTP path retries transient failures before
returning a structured error.

Current protocol support:

- Anthropic native `tool_use`
- OpenAI native `tool_calls`
- OpenAI-compatible tool calling using the same message shape
- Ollama text-only chat flow
- local `fallback` behavior for offline use and smoke testing

If credentials are missing, RoboCode can still run against deterministic local
fallback behavior instead of failing to start.

## Tool System

Builtin tools:

- `shell`
- `read_file`
- `write_file`
- `edit_file`
- `glob`
- `grep`
- `web_search`
- `web_fetch`
- `git_status`
- `git_diff`
- `git_branch`
- `git_switch`
- `git_add`
- `git_commit`
- `git_push`
- `git_restore`
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

Every tool declares:

- metadata
- mutability
- schema hint
- execution logic

All builtin tools return serializable results so their behavior is fully visible
in the transcript.

The CLI currently exposes these tool surfaces through slash commands as well:

- `/help`
- `/model`
- `/provider`
- `/permissions`
- `/plan`
- `/sessions`
- `/resume`
- `/diff`
- `/git ...`
- `/web ...`
- `/tasks`
- `/task ...`
- `/memory ...`
- `/lsp ...`

Current workflow/LSP notes:

- `robocode-workflows` keeps task and memory state outside the canonical transcript while remaining rebuildable from JSONL event logs.
- `robocode-lsp` currently supports query-driven semantic code intelligence through language-server stdio sessions.
- The current LSP runtime already covers real queries, session reuse, document synchronization, and normalized output, but it is still an early implementation rather than a fully mature long-lived LSP platform layer.

## Platform Notes

RoboCode keeps one shared engine across platforms and varies only the execution
adapter where necessary:

- POSIX shell adapter on macOS and Linux
- PowerShell adapter on Windows

Behavior is aligned at the tool contract level rather than by forcing identical
shell syntax across operating systems.
