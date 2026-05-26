# RoboCode Architecture

## Target Architecture

RoboCode is organized as a local-first developer agent runtime. The CLI is the
entrypoint; `robocode-core` owns the agent loop; durable state, tools,
permissions, workflows, LSP, and model providers stay behind explicit subsystem
boundaries.

```mermaid
flowchart TB
    User["User / Developer"] --> CLI["robocode-cli<br/>REPL / Slash Commands / Terminal Views"]

    CLI --> Core["robocode-core<br/>SessionEngine / Command Router / Agent Loop"]

    Core --> Config["robocode-config<br/>Layered Config / Provider-Scoped Config"]
    Core --> Perm["robocode-permissions<br/>Permission Modes / Approval Gate"]
    Core --> Session["robocode-session<br/>JSONL Transcript / SQLite Index / Resume"]
    Core --> Workflows["robocode-workflows<br/>Tasks / Memory / Resume Context"]
    Core --> Tools["robocode-tools<br/>File / Search / Shell / Web / Git / LSP Tools"]
    Core --> Model["robocode-model<br/>ProviderHost / Registry / Protocol Adapters"]

    Tools --> LSP["robocode-lsp<br/>Diagnostics / Symbols / References"]
    Tools --> LocalOS["Local OS<br/>Filesystem / Shell / Git / Network"]

    Model --> ProviderSDK["robocode-provider-sdk<br/>Plugin ABI / Descriptor Contract"]
    Model --> Builtins["Built-in Providers<br/>Anthropic / OpenAI / Ollama / Fallback"]
    Model --> DeepSeek["robocode-provider-deepseek<br/>DeepSeek Plugin"]

    ProviderSDK --> DynamicPlugins["Dynamic Provider Plugins<br/>Native dylib / so / dll now<br/>WASM later"]

    Builtins --> APIs["Model APIs"]
    DeepSeek --> APIs
    DynamicPlugins --> APIs

    APIs --> Anthropic["Anthropic-style<br/>tool_use"]
    APIs --> OpenAI["OpenAI-style<br/>tool_calls"]
    APIs --> DeepSeekAPI["DeepSeek<br/>deepseek-v4-flash / deepseek-v4-pro<br/>OpenAI + Anthropic endpoints"]
```

## Workspace Layout

- `robocode-cli`: user-facing REPL and slash commands.
- `robocode-config`: config loading, merge precedence, and startup defaults.
- `robocode-core`: session engine and turn orchestration.
- `robocode-model`: provider host/runtime, HTTP adapters, dynamic provider registry, and tool-calling protocol translation.
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
- provider-scoped config with generic fallback
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

## Terminal Presentation

`robocode-core` owns plain-text terminal presentation helpers so slash-command
views stay consistent without requiring a full-screen TUI. Current structured
views cover:

- LSP diagnostics, symbols, and references
- session lists
- project tasks and memory
- permission denials and approval outcomes that block execution
- `/git diff` and `/diff` summaries with file/addition/deletion counts

The renderer keeps section titles, summaries, entry headings, field rows, empty
states, and diff summaries in one place. Future TUI work should reuse these
same output contracts rather than creating a second command-result model.

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

## Provider Runtime

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
- `deepseek` as an independent provider family using the official OpenAI-style API surface
- `deepseek-anthropic` for DeepSeek's official Anthropic-compatible API surface
- OpenAI-compatible gateway descriptors for `openrouter`, `groq`, `mistral`, `together`, `kimi`, `qwen`, `zhipu`, and `volcengine`
- `ollama`
- `fallback`

The provider runtime on main has evolved from a small built-in factory into a
provider host/runtime with:

- built-in provider descriptors
- dynamic provider registry
- a compatibility matrix for built-in descriptors, including protocol family, default model, streaming capability, and tool-call capability
- protocol adapters separated from provider identity
- a plugin contract designed for native dynamic loading first and WASM
  migration later
- instance-scoped provider binding so different sessions/agents can use
  different providers concurrently in the same process

The HTTP-backed providers use the system `curl` binary so the workspace remains
dependency-light and offline-compilable. The provider config includes request
timeouts and retry counts, and the HTTP path retries transient failures before
returning a structured error.

Current protocol support:

- Anthropic native `tool_use`
- OpenAI native `tool_calls`
- OpenAI-compatible tool calling using the same message shape
- OpenAI-compatible gateway providers through shared descriptor-backed HTTP providers
- DeepSeek as an independent provider identity with:
  - `deepseek` bound to the OpenAI-style adapter family at `https://api.deepseek.com`
  - `deepseek-anthropic` bound to the Anthropic-style adapter family at `https://api.deepseek.com/anthropic`
- DeepSeek V4 defaults to `deepseek-v4-flash`; `deepseek-v4-pro` is selectable explicitly
- Ollama text-only chat flow
- local `fallback` behavior for offline use and smoke testing

If credentials are missing, RoboCode can still run against deterministic local
fallback behavior instead of failing to start.

Runtime provider loading target:

- the registry can be refreshed while the process is running
- newly loaded providers become available to newly created provider instances
- active sessions keep their bound provider instances instead of hot-swapping in
  place
- built-in and dynamically discovered descriptors flow through one registry,
  while full plugin-backed execution, streaming, cancellation, and broader
  provider compatibility continue to harden

### Provider Plugin Runtime

The provider runtime separates provider identity from protocol behavior. The
registry answers "what providers exist"; the host creates instance-scoped
providers for each session or agent.

```mermaid
flowchart TB
    Core["robocode-core<br/>SessionEngine / Agent Runtime"] --> Host["robocode-model::ProviderHost"]

    Host --> Registry["ProviderRegistry<br/>provider lookup / reload / collision checks"]
    Host --> Factory["Provider Factory<br/>per-session provider instances"]

    Registry --> Builtin["Built-in descriptors<br/>anthropic / openai / ollama / fallback / deepseek"]
    Registry --> PluginLoader["Dynamic Plugin Loader<br/>scan plugin dirs"]
    PluginLoader --> NativeLib["Native plugins<br/>dylib / so / dll"]
    NativeLib --> Descriptor["PluginDescriptor JSON<br/>stable ABI boundary"]

    Factory --> AdapterChoice["Protocol Adapter Binding"]
    AdapterChoice --> AnthropicAdapter["Anthropic-style adapter<br/>tool_use"]
    AdapterChoice --> OpenAIAdapter["OpenAI-style adapter<br/>tool_calls"]

    Builtin --> DeepSeekOpenAI["deepseek<br/>OpenAI-style<br/>https://api.deepseek.com"]
    Builtin --> DeepSeekAnthropic["deepseek-anthropic<br/>Anthropic-style<br/>https://api.deepseek.com/anthropic"]

    OpenAIAdapter --> APIs["External Model APIs"]
    AnthropicAdapter --> APIs

    APIs --> DeepSeekAPI["DeepSeek<br/>deepseek-v4-flash / deepseek-v4-pro"]
    APIs --> OpenAIAPI["OpenAI / compatible"]
    APIs --> AnthropicAPI["Anthropic / compatible"]
```

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
- `/test <command>`
- `/git ...`
- `/web ...`
- `/tasks`
- `/task ...`
- `/memory ...`
- `/lsp ...`

Current workflow/LSP notes:

- `robocode-workflows` keeps task and memory state outside the canonical transcript while remaining rebuildable from JSONL event logs.
- `/test <command>` reuses the shell tool permission path and stores the latest
  test evidence in `SessionEngine` so `/status` can report the most recent
  verification command, exit code, likely failing-file count, and output tail
  without creating a second execution path. The command output also includes a
  small parser for common Rust/cargo and pytest failure-summary/file patterns.
- `/status` also acts as a read-only cockpit snapshot: it collects git dirty
  files, active workflow tasks, and lane state from `.robocode/lanes.tsv`, with
  each collector degrading independently if that source is unavailable.
- Successful `write_file` and `edit_file` results are structured as `path`,
  `size`, and `effect` lines so transcript and TUI surfaces can summarize file
  changes without parsing free-form prose.
- Lane inspect/apply/recovery commands store auditable artifacts under
  `.robocode/lanes/` and render a recommended next action so the operator can
  move from evidence review to accept/apply/resolve/cleanup without guessing
  the command sequence.
- Side screens reuse the same lane next-action language and artifact hints:
  side-1 emphasizes lane supervision plus persisted log tails, while side-2
  carries compact ops activity rows for the same command sequence.
- `robocode-lsp` currently supports query-driven semantic code intelligence through language-server stdio sessions.
- The current LSP runtime already covers real queries, session reuse, document synchronization, and normalized output, but it is still an early implementation rather than a fully mature long-lived LSP platform layer.

## Platform Notes

RoboCode keeps one shared engine across platforms and varies only the execution
adapter where necessary:

- POSIX shell adapter on macOS and Linux
- PowerShell adapter on Windows

Behavior is aligned at the tool contract level rather than by forcing identical
shell syntax across operating systems.
