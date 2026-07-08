# V2-D Structured Views Design

## Purpose

This document defines the first V2-D slice for Viden's terminal
presentation layer. The goal is to make existing command results easier to
scan and reason about without changing command semantics, tool execution,
permission behavior, or transcript storage.

The confirmed direction for this slice is:

- start with LSP-facing output only
- keep the existing REPL and command pipeline
- add a presentation-focused module inside `viden-core`
- improve structure, not feature scope
- defer full-screen TUI work until the text-rendering surface is stable

## Product Goal

Viden should not only produce correct developer-facing information, it
should present that information in a form that is easy to act on during
everyday terminal use.

For this slice, a user running LSP commands should be able to answer:

- which file a result belongs to
- where an issue or reference is located
- what kind of symbol is being shown
- whether a line belongs to a container such as a function or module

The output should remain plain terminal text, but it should read like an
intentional structured view rather than a raw line dump.

## Scope

In scope:

- structured rendering for:
  - `/lsp diagnostics`
  - `/lsp symbols`
  - `/lsp references`
- a reusable presentation helper module in `viden-core`
- renderer-focused tests that lock output shape
- preserving relative path rendering where possible

Out of scope:

- full-screen TUI
- new UI crates or dependencies
- changing slash command names or arguments
- changing tool contracts
- changing transcript schema
- tasks, memory, sessions, diff, or approval rendering in this first slice

## Architecture

### Module Boundary

Add a presentation-focused internal module:

- `viden-core/src/presentation.rs`

Responsibilities:

- small text-rendering helpers
- section and subsection formatting
- grouped line assembly for structured command output

Non-responsibilities:

- command parsing
- business logic
- tool execution
- transcript writes
- ANSI-heavy terminal behavior

`viden-core/src/lib.rs` remains the command routing surface. It continues to
own command dispatch and domain-specific rendering decisions, but delegates
common formatting behavior to `presentation.rs`.

### Responsibility Split

- `viden-core`
  owns command handling, domain-aware formatting, and final command output
- `presentation.rs`
  owns reusable text layout helpers
- `viden-cli`
  remains a thin printing surface and should not gain command-specific view
  logic in this slice

This preserves the existing boundary that command output is produced inside the
engine and emitted through the shared runtime path.

## Target Behaviors

### Diagnostics

`/lsp diagnostics` should:

- group diagnostics by relative file path
- render one file header followed by indented entries
- show location in a stable `line:character` form
- retain severity, source, and code when present
- stay readable when multiple diagnostics belong to the same file

Example shape:

```text
Diagnostics:
src/lib.rs:
  2:4 error [rust-analyzer/clippy] unused variable
  8:1 warning [rust-analyzer] dead code
```

### Symbols

`/lsp symbols` should:

- keep one symbol per line
- use readable kind labels instead of raw numeric codes
- show relative paths
- show `in <container>` when a container is known
- prefer compact scanning over verbose multi-line symbol blocks

Example shape:

```text
Symbols:
src/lib.rs:
  main [function] 3:1
  value [variable] 4:5 in main
```

### References

`/lsp references` should:

- preserve stable ordering and deduplicated results
- use relative paths where possible
- render one reference per line with compact location formatting
- avoid repeating unnecessary wording per entry

Example shape:

```text
References:
  src/lib.rs:4:5
  src/engine.rs:18:9
```

## Data and API Impact

This slice does not add new persisted state and does not change public tool
contracts.

Expected code-level impact:

- new internal helper functions in `presentation.rs`
- existing `render_lsp_diagnostics`
- existing `render_lsp_symbols`
- existing `render_lsp_locations`

No changes are required to:

- `viden-tools`
- `viden-lsp`
- `viden-session`
- `viden-permissions`

## Testing Strategy

Tests should lock view shape before implementation changes.

Required coverage:

- diagnostics render grouped by file
- diagnostics retain severity/source/code and relative path formatting
- symbols show readable kind labels
- symbols show container context when present
- references remain relative and stable
- presentation helper tests for section title and basic line assembly

Verification commands for this slice:

```bash
cargo test -p viden-core render_lsp_
cargo test -p viden-core presentation
```

Before completion, run:

```bash
cargo test -p viden-core
cargo test --workspace --quiet
```

## Risks and Constraints

Constraints:

- no new dependency for rendering
- keep output transcript-safe plain text
- keep the diff small and reversible

Risks:

- over-abstracting presentation too early
- accidentally mixing business logic into formatter helpers
- making output prettier but less stable for tests

Mitigations:

- keep helper functions narrow
- keep grouping decisions close to the existing LSP renderers
- lock output shape with focused tests before broader refactors

## Follow-on Work

If this slice succeeds, later V2-D work can extend the same presentation layer
to:

- `/sessions`
- `/tasks`
- `/memory`
- `/diff`
- approval prompts

That later work should reuse the same boundary:

- `viden-core` owns structured text output
- `viden-cli` remains a thin terminal shell
