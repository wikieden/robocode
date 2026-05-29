# RoboCode 0.1.16 Status - TUI Interaction Reliability

Chinese version: [release-0.1.16-status.zh-CN.md](release-0.1.16-status.zh-CN.md)

## Status

`0.1.16` is a local release candidate for the inserted TUI interaction
reliability slice. It is intentionally placed before the lightweight
spec/steering workflow so the cockpit stays trustworthy before more workflow
surface is added.

GitHub Release assets and the Homebrew tap update are not published yet.

## Landed Scope

- Workspace version bumped to `0.1.16`.
- Provider turns now run behind a worker/channel boundary. The TUI keeps
  repainting `NOW WORKING`, elapsed time, status bars, lane snapshots, and
  approval prompts while the provider worker runs.
- Approval prompts are bridged back from the provider worker to the existing
  permission path. Approval still defaults to `Approve`.
- Active-turn composer shortcuts are real: `Ctrl-J` sends, `Ctrl-K` clears,
  `Ctrl-R` reloads the latest user prompt, `Ctrl-N` starts `/task add ...`,
  and `?` opens help only from an empty composer.
- Command suggestions now keep long lists windowed and selected rows visible.
  Mouse hit testing maps visible rows back to the underlying suggestion index.
- Approval `Diff` focus now renders prompt evidence / preview lines when
  present instead of a decorative-only affordance.
- The TUI interaction audit, user guide, cockpit design, roadmap, README, and
  screenshot references were updated for the inserted interaction version.

## Verification

Passed locally on 2026-05-29:

```bash
cargo fmt --check
git diff --check
cargo clippy -p robocode-types -p robocode-core -p robocode-cli --all-targets -- -D warnings
cargo test -p robocode-cli tui::render::tests::render_frame_overlays_approval_modal --quiet
cargo test --workspace --quiet
scripts/release-smoke.sh --version 0.1.16 --quick --out-dir /tmp/robocode-0116-release-smoke-local
```

The quick release smoke passed:

- `cargo-fmt`
- `cargo-clippy`
- `robocode-cli-terminal-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`

Smoke evidence directory:

```text
/tmp/robocode-0116-release-smoke-local
```

## Visual Evidence

Deterministic 0.1.16 TUI screenshots:

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.16-tui-side-2.svg`

Structured screenshot evidence:

```text
/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/tui-regression-evidence.json
```

## Remaining Risks

- Cancellation is best-effort. `Ctrl-C` can request cancellation, but an
  already in-flight provider request may still finish before the worker sees
  the cancellation state.
- This release keeps the TUI responsive during provider work, but it is not full
  token-by-token provider streaming.
- Mouse support is still narrow: approval and command suggestions are covered,
  while right-rail selection, side-panel scrolling, lane-modal controls,
  transcript links, and mouse wheel handling remain follow-up work.
- Cursor blink and IME candidate-window placement still depend partly on the
  host terminal.
- GitHub Release packaging, multi-platform assets, post-publish validation, and
  Homebrew tap update remain outside this local RC.

## Next

`0.1.17` should resume the lightweight spec/steering workflow, but only after
carrying forward the remaining interaction backlog as acceptance criteria:
mouse coverage, true cancellability where supported, streaming display, and
more manual Terminal / iTerm2 acceptance screenshots.
