# RoboCode 0.1.17 Plan - Daily Coding Loop Baseline

Chinese version: [release-0.1.17-plan.zh-CN.md](release-0.1.17-plan.zh-CN.md)

## Summary

`0.1.17` should shift the next release from "more surfaces" to "real daily
usability." The goal is not to make RoboCode look more complete. The goal is to
make one normal programming loop dependable enough to use on a real project:

> install -> configure provider -> ask for a scoped change -> approve edits ->
> run tests -> inspect failures -> fix or delegate -> review diff -> preserve
> evidence and resume later.

The earlier plan put lightweight spec/steering directly after `0.1.16`. That
still matters, but it should serve this daily loop rather than become another
feature island. For `0.1.17`, spec/steering becomes a small task brief and
project-convention layer that makes real coding work safer and easier to
repeat.

## Product Goal

By the end of `0.1.17`, a developer should be able to use RoboCode for a
single-repo coding task without constantly dropping to another terminal to
understand what happened.

## 0.20 Usability North Star

RoboCode should feel "truly usable" before `0.1.20` if these are true:

- First run is understandable: provider/model/API-key status is visible, and
  the user can fix setup from the TUI.
- DeepSeek is the default online path. Fallback/test-local stays available as
  an explicit offline/testing path, not as the silent default for real use.
- Model failure is recoverable from inside the product: unsupported model,
  unavailable model, auth, and provider compatibility errors should suggest a
  concrete provider/model switch.
- The main coding loop works: change request, file edits, approval, shell/test,
  diff review, and final summary.
- Failure recovery is obvious: test failures, apply conflicts, provider errors,
  and cancelled turns show next safe actions.
- Context is controllable: the user can see what context is included, what was
  omitted, and why.
- Delegated lanes are useful in one real path: Codex/Claude/shell lanes can
  run, produce evidence, and be accepted/discarded/applied without guesswork.
- The TUI remains responsive during long turns, resize, CJK input, approvals,
  and side-screen monitoring.
- Documentation explains the workflows users actually perform, not only the
  architecture.

## P0 Scope

### 1. Daily Coding Loop Smoke

Create a deterministic smoke scenario that proves RoboCode can complete the
normal coding loop:

1. Start with the offline fallback/test provider for deterministic CI, and run
   the same path with DeepSeek credentials during manual acceptance.
2. Ask for a small code change in a fixture workspace.
3. Trigger a file edit through the permission path.
4. Approve the edit.
5. Run a test command.
6. Show failure or success evidence in transcript, right rail, and side-2.
7. Run `/diff` or `/git diff`.
8. Produce a final summary with changed files, tests, and next action.

Acceptance:

- Add `scripts/daily-loop-smoke.sh` or extend `scripts/release-smoke.sh` with a
  named daily-loop step.
- Evidence must include transcript, changed file, test output, diff summary,
  and deterministic TUI screenshot or ANSI capture.
- The smoke must run without a live provider key, while the real-use acceptance
  path must also prove the DeepSeek-first launch path.

### 2. Task Brief And Steering Files

Implement the minimum useful spec/steering layer:

- `/brief` or `/spec` creates a task brief from the current request.
- `/brief show` displays:
  - goal
  - constraints
  - files likely involved
  - acceptance checks
  - risk notes
- Project steering files live under `.robocode/steering/`:
  - `conventions.md`
  - `architecture.md`
  - `workflows.md`
- The ContextBundle can reference the active brief and steering summaries.

Acceptance:

- A task brief can be created, shown, and attached to a lane envelope.
- Steering files are never invented as active project facts without an explicit
  user command.
- The TUI shows the active brief id/title in `NOW WORKING` or side-2 when a
  brief is active.

### 3. DeepSeek-First Setup And Model Recovery

The tool should help a new user become productive:

- Clean installs should default to DeepSeek as the online provider target when
  no explicit provider is configured.
- `/setup` should open an interactive TUI configuration flow, not only print
  instructions:
  - choose provider
  - choose model from provider defaults/candidates
  - show API key env var and whether it is present
  - show config path and save target
  - test provider reachability when credentials are available
- `/settings` keeps the command-based path for scripting:
  `/settings provider <id> [model]`, `/settings model <model>`, and
  `/settings save`.
- `/doctor` should include TUI, provider, git workspace, release version, and
  lane prerequisites.
- Missing provider credentials should produce a fix command or env-var hint.
- If the current model does not work, RoboCode should show an actionable switch
  prompt instead of a raw provider error. Covered cases include unknown model,
  unavailable model, auth/model permission errors, context-limit mismatch, and
  provider protocol incompatibility.
- The model recovery prompt should suggest:
  - the provider's default model
  - known compatible alternatives
  - `/settings model <candidate>`
  - `/settings provider <provider> <model>`
  - `/provider doctor <provider>`

Acceptance:

- `robocode-cli --provider fallback --model test-local` remains the offline
  escape hatch.
- Launching with no saved provider enters the DeepSeek-first setup path.
- DeepSeek setup path is documented and visible in TUI, `/setup`, `/settings`,
  and `/doctor`.
- The interactive provider/model configuration flow can save provider/model
  defaults without storing API keys.
- A deliberately invalid model produces a visible "switch model" action with at
  least one compatible DeepSeek candidate.
- `doctor` output is captured in daily-loop or release smoke evidence.

### 4. Reviewable Diff And Test Evidence

Improve the "is this safe to accept?" path:

- `/diff` should summarize files changed, additions/deletions, and likely test
  commands.
- `/test <command>` failures should expose:
  - failing command
  - exit code
  - duration
  - top failure lines
  - likely file paths
  - suggested rerun command
- Side-2 should prioritize latest diff/test evidence over lower-signal rows.

Acceptance:

- A failed test in the daily-loop smoke produces actionable next action text.
- A successful test produces a clear "ready for review" state.

### 5. Real-Use Screenshot Set

For every P0 feature, produce one deterministic artifact or real screenshot:

- setup/doctor
- interactive provider/model setup
- model failure and switch-model prompt
- active brief
- edit approval
- test failure or success
- diff review
- final ready state
- side-2 evidence

## P1 Scope

- Better mouse coverage for right rail, side panels, and lane modal controls.
- True provider cancellation for providers/runtime paths that can observe
  cancellation.
- First streaming token renderer when a provider exposes streaming events.
- A `robocode doctor --json` output mode for automation.
- A `--daily-loop-smoke` preview fixture for release screenshots.
- Provider/model favorites and last-known-good model history.

## Non-Goals

- Do not broaden ACP/MCP/plugin mutation yet.
- Do not add a marketplace or install UX.
- Do not make Codex/Claude write-capable by default.
- Do not build a full spec product; keep task brief and steering minimal.

## Test Plan

Focused:

- brief/steering command tests
- ContextBundle includes active brief/steering summaries
- setup/doctor provider diagnostics tests
- default DeepSeek provider resolution tests
- interactive provider/model setup reducer tests
- provider/model failure classification and recovery-prompt tests
- diff/test evidence reducers
- TUI render tests for active brief and side-2 evidence

Regression:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.17 --quick --out-dir /tmp/robocode-0117-release-smoke-local
```

Manual:

- Install from Homebrew.
- Launch with no saved config and confirm the DeepSeek-first setup path.
- Launch with fallback provider as the explicit offline path.
- Launch with DeepSeek credentials and a valid model.
- Launch with DeepSeek credentials and an invalid/unavailable model, then
  confirm the switch-model prompt.
- Save provider/model from the interactive setup flow and restart.
- Run the daily coding loop on a small real repository.
- Capture at least one real terminal screenshot for the loop.

## 0.18-0.20 Bridge

- `0.1.18`: failure recovery and review gates. Make test failure, diff review,
  and apply/rollback the strongest path in the product.
- `0.1.19`: delegated lane usefulness. Make one Codex/Claude/shell delegated
  workflow reliable enough for real review tasks.
- `0.1.20`: usability beta. A clean install should complete the daily coding
  loop and one delegated review loop with documented evidence.
