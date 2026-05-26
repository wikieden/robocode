# RoboCode 0.1.5 Plan

Last updated: 2026-05-25

## Theme

`0.1.5` is the programming-experience release. Version `0.1.4` proved that the
TUI cockpit, DeepSeek V4 Flash provider path, shell lanes, tmux lanes, and
release artifacts can work end to end. The next version should make those
pieces feel good enough for daily coding.

The north star:

> In a real Rust, JavaScript, or Python project, a user can stay inside RoboCode
> TUI for the loop: understand the request, edit code, review the diff, run
> tests, fix failures, and summarize the result.

Focused benchmark:

- `docs/code-agent-experience-benchmark-2026-05-25.md` compares Codex,
  Claude Code, DeepSeek-TUI / CodeWhale, and Zed, and is the benchmark source
  for this release plan.

## Product Principles

- Optimize for the coding loop, not surface area.
- Make every mutation explainable before approval.
- Keep raw evidence auditable, but render concise operator summaries.
- Prefer one reliable workflow over several half-wired shortcuts.
- Keep `/lane` TUI-first until the product decides to support it in the plain
  REPL.
- Treat screenshots, TUI snapshots, and smoke transcripts as release evidence.

## Non-Goals

- Do not start V3 platform expansion work such as MCP, remote bridge, or
  automation unless it directly unblocks the coding loop.
- Do not build a full terminal emulator in `0.1.5`; improve persisted log replay
  and lane controls first.
- Do not add new dependencies for UI polish unless an existing crate or local
  helper cannot solve the problem safely.
- Do not make side screens decorative; they must show useful coding evidence.

## P0: TUI Interaction Stability

These are release blockers because they affect every coding session.

- Composer:
  - make the input area taller and easier to see;
  - keep the cursor visible and blinking;
  - keep Chinese IME candidate windows near the active input line;
  - preserve multiline input without crowding the bottom bar.
- Resize and redraw:
  - handle terminal resize events without stale borders or shifted right panels;
  - keep transcript, right rail, composer, and bottom status aligned at common
    widths;
  - add snapshot/smoke coverage for compact, normal, wide, and short terminals.
- Approval modal:
  - default focus starts on approve;
  - keyboard shortcuts work even when the modal is open;
  - mouse clicks can choose approve, deny, diff, and apply-all where supported;
  - the modal clears immediately after a decision.
- Command palette:
  - `/` opens a useful suggestion list;
  - arrow keys select, Enter completes, Tab completes, Esc closes;
  - descriptions stay inside the palette and do not push the composer down.
- Rendering safety:
  - no ANSI, OSC, emoji, wide CJK text, or shell prompt noise can break panel
    alignment;
  - right-rail data stays clipped or wrapped inside its panel.

Acceptance evidence:

- `cargo test -p robocode-cli`.
- TUI preview generation passes and includes composer, command palette, modal,
  lane detail, side-1, side-2, and multiscreen snapshots.
- Manual or tmux-driven fallback TUI smoke covers typing, slash suggestions,
  approval, resize, and `/exit`.
- A screenshot or rendered preview is saved for the final visual checkpoint.

## P1: Coding Loop

This is the core value of the release.

- Diff review:
  - show changed files with added/modified/deleted status;
  - show a concise diff summary before approvals and after tool calls;
  - make the next action clear: approve, deny, inspect, run tests, or continue.
- Test workflow:
  - add an operator-friendly `/test` or `/run test` flow;
  - summarize exit code, failing command, failing files, and useful tail lines;
  - keep test evidence attached to the current task/session.
  - Current checkpoint: `/test <command>` is implemented through the shell
    permission path and `/status` reports the latest command, status, exit
    code, duration, and output tail. Failing-file extraction remains a follow-up
    refinement.
- Structured tool results:
  - compile errors, test failures, lint failures, and shell failures render as
    grouped evidence instead of raw log walls;
  - successful writes show file path, size, and a short effect summary.
- Task continuity:
  - the active task panel should reflect real `/task` and lane state;
  - resumed sessions should show what changed, what was tested, and what remains.
- Status clarity:
  - `/status` should tell the user provider, model, context, permissions, dirty
    files, active task, last test result, and recent lane state.

Acceptance evidence:

- Fallback provider coding smoke creates or edits a small file, reviews the
  diff, runs a test command, and exits cleanly.
- DeepSeek V4 Flash coding smoke performs a small code or script task with at
  least one tool call and one verification command.
- Session transcript proves the diff/test evidence is recorded.

## P1: Lane Operator Experience

Lanes become useful when they reduce context switching rather than adding
another place to watch logs.

- Lane creation:
  - keep `/lane run`, `/lane codex`, `/lane claude`, and `/lane ask` discoverable
    from the command palette;
  - make missing templates explain exactly what to configure.
- Lane inspect:
  - show command, status, pid/session, workspace, changed files, last output,
    exit code, decision artifacts, and recommended next action;
  - make tmux and PTY attach/send routes obvious.
- Lane apply and recovery:
  - make accept/apply/resolve/cleanup a guided sequence;
  - show conflict evidence without implying changes were reverted automatically.
- Side screens:
  - side-1 prioritizes agent lanes and live output;
  - side-2 prioritizes diagnostics, tests, repo state, and pressure indicators;
  - side screens remain useful even when no external agents are running.

Acceptance evidence:

- Shell lane smoke covers create, inspect, complete, archive or cleanup.
- Tmux lane smoke covers attach command generation, log capture, inspect, and
  cleanup.
- A template-lane dry run proves missing-template and configured-template paths
  are both understandable.

## P2: Project Context and Guidance

These deepen the experience once the core loop is stable.

- `/context` shows current files, dirty changes, task, recent tests, recent
  diagnostics, active provider, and lane state.
- LSP diagnostics can seed a fix task or lane.
- Recent files and workspace panels prefer files relevant to the current task.
- Provider health shows actionable failure messages, not only latency numbers.
- Release notes should clearly explain what is real, what is preview, and what
  remains intentionally deferred.

## Release Smoke Matrix

Before tagging `v0.1.5`, run:

- `cargo fmt --check`
- `cargo test -p robocode-cli`
- `cargo test --workspace --quiet`
- `scripts/tui-previews.sh /tmp/robocode-015-preview`
- fallback TUI coding smoke
- DeepSeek V4 Flash TUI coding smoke
- shell lane operator smoke
- tmux lane operator smoke
- package smoke for the host platform
- GitHub Actions release artifact validation for all configured targets with
  `upload_to_release=false`

## Development Slices

1. Composer, cursor, IME, and resize stability.
2. Approval modal and command palette ergonomics.
3. Diff and test evidence in the main coding loop.
4. Lane inspect/apply/side-screen operator flow.
5. Context/status polish and final release smoke.

## Benchmark-Aligned Product Decisions

- Keep RoboCode positioned as a local terminal cockpit, not a cloud task runner
  or full editor replacement.
- Match Codex and Zed expectations around central diff/review evidence.
- Match Claude Code expectations around a smooth terminal action loop,
  permissions, and eventually hooks, but keep `0.1.5` focused on the approval
  and test loop.
- Match DeepSeek-TUI's terminal density and DeepSeek V4 Flash provider
  visibility, but only for panels backed by real runtime state.
- Treat lanes as supervised work threads. Side screens should show agent lanes,
  tests, diagnostics, repo state, and recommended next actions.
- Defer MCP, remote automation, and a full terminal emulator unless they
  directly unblock the coding loop.

## Exit Criteria

`0.1.5` is ready when:

- the user can complete a small real coding task inside the TUI without
  switching to ad hoc shell usage for every step;
- the TUI remains visually stable during typing, resize, approvals, and lane
  updates;
- shell and tmux lanes provide useful evidence rather than raw noise;
- DeepSeek V4 Flash can complete a small live coding smoke;
- release artifacts build for every configured target.
