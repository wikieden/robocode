# TUI Cockpit and Terminal Lanes Development Plan

Date: 2026-05-23

## Requirements Summary

RoboCode's TUI should evolve from the current lightweight alternate-screen shell into a coding-agent cockpit:

- implement the approved main-screen TUI from `DESIGN.md`;
- preserve the existing `SessionEngine` path for prompts, approvals, tool execution, and transcripts;
- add a screen model that can later support one main screen plus two companion workspaces;
- introduce supervised terminal lanes so companion screens can run real side work;
- integrate external terminal coding tools such as `codex` and `claude` through task envelopes and adapters only after the lane runtime is stable.

The current code shape supports an incremental path:

- `robocode-cli/src/main.rs:96` enters `tui::run_tui` when `--tui` is set.
- `robocode-cli/src/tui.rs:28` owns the current TUI event loop.
- `robocode-cli/src/tui.rs:67` already routes permission prompts through `prompt_for_tui_approval`.
- `robocode-cli/src/tui.rs:70` reuses `SessionEngine::process_input_with_approval`.
- `robocode-cli/src/tui.rs:187` currently renders the whole UI as one simple string frame.
- `robocode-core/src/lib.rs:31` exposes `EngineEvent`, which is enough for the first transcript timeline.
- `robocode-types/src/transcript.rs:28` defines the durable transcript entry model that companion views can later follow.

## Latest Product Design Requirements

The current product direction supersedes the older "rich terminal view" framing:

1. The primary product surface is the user-approved single-screen RoboCode cockpit, not the generated multi-screen concept sheet.
2. Companion screens are workspaces, not dashboards. Their core value is running and supervising side work.
3. Terminal lanes are the main primitive for side work:
   - run local commands;
   - later launch `codex`, `claude`, and other coding CLIs;
   - capture logs, status, exit code, diffs, and verification evidence;
   - require explicit inspect/accept/revise/discard decisions.
4. External tools are collaborators behind adapters, not native trusted agents.
5. Provider live compatibility remains important, but it should not block the TUI/lane product slice; provider matrix updates are a parallel quality track.

## Current Development Baseline

Latest `main` is at `bf41ffa` and added the provider live compatibility matrix. The next development batch should start from this baseline:

- `docs/provider-live-matrix.md` is now the evidence log for live provider checks.
- TUI work must keep provider/model/mode visible in the top rail, but does not need to complete provider live verification first.
- Any DeepSeek/OpenAI-compatible provider smoke used during TUI work should record only actual verified results; offline tests must not be described as live provider compatibility.

## Development Milestones

### Milestone A: Commit Design Baseline

Goal: preserve the design, benchmark, previews, architecture plan, and executable plan as a reviewable checkpoint before code work.

Files:

- `DESIGN.md`
- `docs/code-agent-benchmark.md`
- `docs/tui-lane-architecture-plan.md`
- `docs/tui-lane-architecture-plan.zh-CN.md`
- `docs/superpowers/plans/2026-05-23-tui-cockpit-terminal-lanes.md`
- `docs/superpowers/plans/2026-05-23-tui-cockpit-terminal-lanes.zh-CN.md`
- `docs/previews/`
- `PLAN.md`

Acceptance:

- docs are internally consistent with latest `main`;
- `PLAN.md` links to the new source docs;
- no source code behavior changes in this checkpoint.

### Milestone B: Main Cockpit Screen

Goal: replace the lightweight TUI frame with the approved main-screen structure while preserving `SessionEngine` behavior.

Deliver:

- top status rail;
- transcript timeline;
- right workspace/status rail;
- composer;
- bottom status bar;
- modal approval state;
- built-in `aurora-cyan` theme tokens.

Acceptance:

- prompt submission and approvals still flow through `SessionEngine`;
- render snapshots cover compact, normal, and wide terminals;
- fallback-provider TUI smoke still works;
- plain REPL remains untouched.

### Milestone C: Screen Registry Shell

Goal: introduce the `MAIN` / `AGENTS` / `OPS` registry without external process complexity.

Deliver:

- screen state model;
- max two companion workspaces;
- open/focus/close command parsing;
- companion render placeholders driven by real state where available.

Acceptance:

- third companion open attempt is rejected clearly;
- `AGENTS` and `OPS` have distinct layout priorities;
- main session remains usable while companion state changes.

### Milestone D: Terminal Lane Runtime MVP

Goal: make companion work real by running supervised non-interactive commands.

Deliver:

- `TerminalLane` state;
- durable lane logs;
- `/lane run <command>`;
- `/lane inspect <id>`;
- `/lane stop <id>`;
- active lane summary in the main right rail.

Acceptance:

- successful and failed commands capture status, output, and exit code;
- logs survive TUI exit;
- lane state transitions are unit-tested.

### Milestone E: Task Envelopes and External Tool Adapters

Goal: prepare safe interop with `codex`, `claude`, and user-defined coding CLIs.

Deliver:

- task-envelope rendering;
- adapter config shape;
- generic `/lane ask <tool> <task>`;
- `codex` and `claude` presets when binaries exist;
- conservative input modes first: `prompt-file`, `stdin`, `manual`;
- `/lane revise`, `/lane accept`, `/lane discard`.

Acceptance:

- missing binaries produce clear lane errors;
- every external task has an auditable envelope file;
- RoboCode recommends acceptance only after inspecting logs/diff/verification evidence.

### Milestone F: Isolation and Attach

Goal: make external coding lanes safe and interactive enough for real use.

Deliver:

- optional per-lane worktrees;
- diff review before integration;
- attach/detach prototype through tmux, OS terminal windows, or embedded PTY;
- cleanup/archive policy.

Acceptance:

- mutating external lanes can run outside the main worktree;
- `/lane attach` and `/lane detach` do not kill the lane;
- logs remain durable across attachment.

## Non-Goals

- Do not replace the plain REPL.
- Do not introduce a GUI/web client in this slice.
- Do not build embedded PTY/tmux attach before the lane metadata, logging, and inspection model works.
- Do not make external tools trusted authorities; RoboCode must inspect logs, exit codes, diffs, and verification evidence before accepting lane results.
- Do not copy `.ref/` implementations.

## Architecture Decision

Use two near-term implementation branches:

1. `codex/tui-main-screen`
   - refactor the single-file TUI into a small internal module set;
   - implement the approved main screen and approval modal;
   - keep all runtime behavior on `SessionEngine`.

2. `codex/tui-lane-runtime`
   - add terminal lane metadata, log capture, and `/lane run <command>`;
   - keep this first lane implementation non-interactive;
   - postpone `codex`/`claude` adapters until `/lane inspect` is reliable.

This splits product shape from side-work execution while keeping both branches small enough to review.

## Proposed Module Shape

Start within `robocode-cli/src/tui/`:

```text
robocode-cli/src/tui/
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

Keep this module private to `robocode-cli` initially. Promote durable lane records into `robocode-workflows` only after the state model proves useful outside TUI.

## Acceptance Criteria

### Main TUI

- `cargo run -p robocode-cli -- --tui --provider fallback --model test-local` still starts a working TUI.
- The main render includes:
  - top status rail;
  - transcript timeline;
  - right workspace/status rail;
  - composer;
  - bottom status bar;
  - approval modal state.
- Approval prompts still return `ApprovalResponse` through the existing shared approval path.
- Render tests cover compact, normal, and wide terminal sizes.
- Plain REPL behavior remains unchanged.

### Terminal Lane MVP

- `/lane run <command>` starts a supervised non-interactive command.
- A lane records:
  - id;
  - command;
  - cwd;
  - status;
  - start/update timestamps;
  - log path;
  - exit code when complete.
- Lane logs survive TUI exit.
- `/lane inspect <id>` summarizes last output, exit code, and status.
- Failed commands show failure status and exit code.
- Lane state transitions are unit-tested.

### External Tool Readiness

- Task envelope rendering is specified and testable before enabling `codex` or `claude` presets.
- Adapter config shape exists or is planned in a way that avoids hardcoding vendor-specific behavior.
- Missing external binaries produce clear lane errors, not panics.

## Implementation Steps

### Phase 0: Baseline and Branch Hygiene

1. Confirm `main` is up to date.
2. Create `codex/tui-main-screen` in `.worktrees/codex-tui-main-screen`.
3. Preserve existing untracked design docs and previews; do not edit `.ref/`.
4. Run the current focused TUI tests before editing:

```bash
cargo test -p robocode-cli tui --quiet
```

Expected result: current TUI tests pass or any rust-toolchain setup issue is documented.

### Phase 1: TUI Module Split

1. Move `robocode-cli/src/tui.rs` into `robocode-cli/src/tui/mod.rs`.
2. Extract pure rendering helpers into `render.rs`, `layout.rs`, and `panels.rs`.
3. Keep `run_tui` public as `pub(crate)` so `robocode-cli/src/main.rs:97` does not change behavior.
4. Add tests for layout splits before visual expansion.

Verification:

```bash
cargo test -p robocode-cli tui --quiet
```

### Phase 2: Main Screen Rendering

1. Add `TuiApp` state:
   - session id;
   - provider/model;
   - permission mode;
   - input buffer;
   - transcript entries;
   - right-rail snapshot placeholders;
   - optional approval modal.
2. Render the approved main-screen structure:
   - top rail;
   - transcript timeline;
   - right rail;
   - composer;
   - bottom status bar.
3. Keep right-rail data conservative at first:
   - cwd/workspace;
   - active task placeholder from workflow state if easily available;
   - provider/model;
   - recent file placeholder only if reliable.
4. Avoid overfitting generated art; prioritize stable terminal geometry and readable text.

Verification:

```bash
cargo test -p robocode-cli tui --quiet
cargo run -p robocode-cli -- --help | rg -- '--tui'
```

### Phase 3: Approval Modal

1. Change `prompt_for_tui_approval` from transcript-only text into a modal state.
2. Keep key behavior:
   - `y` approves;
   - `n` or `Esc` denies.
3. Add an "apply to all" visual placeholder only; do not add policy semantics unless the permission engine already supports it.

Verification:

- unit-test modal render includes tool name, message, input preview, approve and deny controls;
- smoke the fallback provider TUI path.

### Phase 4: Theme Tokens

1. Add built-in theme tokens:
   - `aurora-cyan`;
   - `ember-gold`;
   - `plasma-violet`;
   - `monochrome-ice`.
2. Use `aurora-cyan` as default.
3. Keep configuration loading for a later slice unless it is very small.

Verification:

- unit-test that all built-in themes define required tokens;
- render tests avoid color assertions unless stable.

### Phase 5: Merge First Slice

1. Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet
cargo run -p robocode-cli -- --provider fallback --model test-local --help
```

2. Update README/README.zh-CN only if user-facing flags or behavior changed.
3. Commit with the Lore Commit Protocol.
4. Open PR and merge after CI.

## Second Branch: Terminal Lane Runtime

### Phase 6: Lane State Model

Branch: `codex/tui-lane-runtime`.

Add a first CLI-local lane state model:

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

First implementation can store lane records under the session home, but if this becomes awkward, move records into `robocode-workflows`.

### Phase 7: `/lane run`

1. Add TUI command parsing for:

```text
/lane run <command>
/lane inspect <id>
/lane stop <id>
/lane archive <id>
```

2. Run commands non-interactively first.
3. Capture stdout/stderr into a durable log file.
4. Render active lanes in the main right rail.

Verification:

- unit-test command parsing;
- unit-test lane state transitions;
- integration-style test for a harmless command such as `printf hello`;
- inspect command includes last output and exit code.

### Phase 8: Lane Inspect and Review UX

1. `/lane inspect <id>` should show:
   - status;
   - command;
   - cwd;
   - log path;
   - exit code;
   - last N log lines.
2. Add placeholders for changed files and verification command, but do not claim support until implemented.
3. Design this as the future acceptance surface for `codex` and `claude`.

### Phase 9: External Tool Adapter Prep

Do not fully implement `codex` or `claude` yet. Prepare the adapter shape:

```rust
enum LaneInputMode {
    Argv,
    Stdin,
    PromptFile,
    Manual,
}
```

Add tests for task-envelope rendering without launching external tools.

## Risks and Mitigations

- Risk: TUI refactor grows too large.
  - Mitigation: branch 1 is rendering-only; no lane runtime.

- Risk: external tools mutate the main worktree unexpectedly.
  - Mitigation: adapter phases come after lane metadata and should default to clear cwd/mutation scope, then per-lane worktrees.

- Risk: embedded terminal/PTY complexity derails useful lane work.
  - Mitigation: start with non-interactive commands and durable logs.

- Risk: TUI breaks plain REPL behavior.
  - Mitigation: `main.rs` should continue to branch at `--tui`; existing REPL loop remains untouched.

- Risk: right rail shows stale or fake data.
  - Mitigation: use placeholders only when labeled; prefer actual engine/workflow data where available.

## Verification Plan

Minimum per branch:

```bash
cargo fmt --all -- --check
cargo test -p robocode-cli --quiet
cargo test --workspace --quiet
```

Smoke checks:

```bash
cargo run -p robocode-cli -- --help | rg -- '--tui'
cargo run -p robocode-cli -- --provider fallback --model test-local
```

Manual TUI check:

```bash
cargo run -p robocode-cli -- --tui --provider fallback --model test-local
```

Lane branch additional checks:

```bash
/lane run printf hello
/lane inspect <id>
```

## Recommended Execution Order

1. Commit current design/planning docs.
2. Implement `codex/tui-main-screen`.
3. Merge after CI.
4. Implement `codex/tui-lane-runtime`.
5. Merge after CI.
6. Start a third branch for `codex`/`claude` adapters only after lane inspect is reliable.

## Follow-Up Plan Candidates

- `codex/tui-screen-registry`: `MAIN` / `AGENTS` / `OPS` state and companion window lifecycle.
- `codex/tui-external-adapters`: task envelopes plus `codex`/`claude` presets.
- `codex/tui-lane-worktrees`: per-lane worktree isolation and acceptance flow.
- `codex/tui-terminal-attach`: tmux or PTY attach/detach prototype.
