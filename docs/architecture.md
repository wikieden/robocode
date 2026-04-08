# RoboCode Architecture

## Workspace Layout

- `robocode-cli`: user-facing REPL and slash commands.
- `robocode-core`: session engine and turn orchestration.
- `robocode-model`: provider abstraction and Anthropic-oriented scaffold.
- `robocode-tools`: builtin local tools and execution adapters.
- `robocode-permissions`: permission modes, rules, and approval decisions.
- `robocode-session`: JSONL transcripts plus SQLite indexing.
- `robocode-types`: shared domain types.

## Main Execution Flow

1. CLI receives a line of user input.
2. Slash commands are resolved locally by `robocode-core`.
3. Normal prompts are appended to the transcript and handed to the model
   provider.
4. Provider emits assistant text and/or tool calls.
5. Tool calls are routed through the permission engine.
6. If approval is required, the CLI prompts the user and returns the decision to
   the engine.
7. Tools execute through a shared registry.
8. Tool results are written to the transcript and reintroduced into the
   conversation history.
9. The engine loops until the provider finishes the turn.

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
dependency-light and offline-compilable. If credentials are missing, RoboCode
falls back to deterministic local behavior rather than failing to start.

## Tool System

Builtin tools:

- `shell`
- `read_file`
- `write_file`
- `edit_file`
- `glob`
- `grep`

Every tool declares:

- metadata
- mutability
- schema hint
- execution logic

All builtin tools return serializable results so their behavior is fully visible
in the transcript.
