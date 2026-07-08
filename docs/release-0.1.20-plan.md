# Viden 0.1.20 Plan - Usability Beta Gate

Chinese version: [release-0.1.20-plan.zh-CN.md](release-0.1.20-plan.zh-CN.md)

## Summary

`0.1.20` is the usability beta gate. The release should make Viden feel
usable by someone who is not sitting next to the maintainer:

> install Viden -> open the TUI -> configure provider/model -> run a daily
> coding task -> understand what is happening -> approve/apply work -> run
> tests -> delegate one review lane -> inspect evidence -> finish with
> confidence.

This is primarily an interaction and product-readiness release. It should
prefer fewer workflows that are obvious and dependable over adding new surfaces
that still require guessing command ids or reading implementation docs.

## Product Goal

By the end of `0.1.20`, a first-time or returning developer should be able to:

- start Viden from a clean install and understand the next action;
- configure provider/model from inside the TUI without memorizing commands;
- recover from missing keys, bad endpoints, model incompatibility, or provider
  failures through visible actions;
- complete a small real coding loop with approvals, file changes, tests, diff
  evidence, and final summary;
- start one delegated lane, observe it, inspect evidence, and accept/apply or
  discard the result;
- trust that visible footer actions, buttons, selectors, and side panels are
  real and not decorative placeholders.

## Baseline From 0.1.19

Already available on main:

- default TUI entry and fallback provider path;
- selector-first `/settings`, `/provider`, `/models`, `/permissions`, and
  `/theme`;
- `/provider` semantics separated from `/models`;
- `PROVIDER CONFIG` second-level provider page for key/endpoint/model actions;
- `NOW WORKING` runtime state projection;
- deterministic delegated shell/template lane smoke;
- TUI screenshot generation and regression evidence;
- GitHub release and Homebrew publishing flow.

The gap is product confidence: the user still sees rough layout, incomplete
mouse coverage, partial first-run setup, and settings paths that sometimes feel
like command completion rather than a real configuration flow.

## Implementation Checkpoints

- Setup wizard shell: `/setup` now opens a dedicated `SETUP WIZARD` selector
  with actionable rows for provider config, model choice, permissions, theme,
  current-provider doctor, fallback smoke, and saving defaults. Deterministic
  `main-setup-wizard` preview and regression artifacts are part of the visual
  evidence set.
- Missing-key startup: the main TUI now preloads `/setup` when the selected
  online provider has no detected API key, so clean installs land on an
  actionable setup surface instead of a passive transcript note.
- Provider failure recovery: provider/model errors now include a recovery class
  for missing key, auth, rate limit, timeout, context overflow, compatibility,
  or model unavailable, plus a concrete next action and commands for model
  switch, provider doctor, and fallback.
- Lane root selector: `/lane` now opens a centered action selector and includes
  id-specific inspect/timeline/diff/artifacts actions when lanes are tracked.

## P0 Scope

### 1. First-Run Setup Wizard

Replace the current command-guided setup with a real staged TUI wizard.

Acceptance:

- Clean install with no saved provider/model opens a visible setup state in the
  TUI instead of a passive transcript message.
- `/setup` opens the same wizard from any session.
- Stages are explicit: provider -> API key/env -> endpoint -> model -> probe ->
  save defaults.
- DeepSeek remains the default online path, with fallback as the offline escape
  hatch.
- API keys are not written to plain config by default. If a key is missing, the
  UI shows the exact env var and shell export hint.
- Probe result has an actionable next step: continue, switch model, edit
  endpoint, open doctor, or use fallback.
- A deterministic preview and screenshot cover the first-run wizard.

### 2. Settings Modal Unification

All user-decision settings should behave like one product system.

Acceptance:

- `/settings` is a real settings hub with visible sections:
  provider, model, permissions, theme, defaults, diagnostics.
- `/provider` opens a provider list, then `PROVIDER CONFIG`.
- `/models` opens provider-grouped model selection.
- `/permissions`, `/theme`, and future mode switches use the same centered
  selector/modal behavior.
- Every selector supports keyboard arrows, Enter, Esc, and mouse click.
- Long rows wrap or split into detail areas; no row should depend on horizontal
  clipping to be understandable.
- Footer text only names actions that actually work.

### 3. Composer, Cursor, IME, And Layout Reliability

Make the input area feel reliable in daily use.

Acceptance:

- Composer has enough height for one active input line plus mode/actions without
  feeling cramped.
- Cursor is visible even in terminals that do not render native blinking cursor
  style well. Add an app-owned high-contrast caret fallback if needed.
- CJK input and IME candidate windows stay close to the typed text in common
  macOS Terminal/iTerm2 layouts.
- Rapid resize does not leave stale lines, right-rail drift, panel misalignment,
  or old modal remnants.
- Add a resize stress regression that exercises composer, selector, approval,
  and lane-detail states.

### 4. Mouse And Focus Router

Move from ad hoc mouse handling to explicit focus targets.

Acceptance:

- Focus targets are explicit: `composer`, `selector`, `approval`, `right-rail`,
  `lane-detail`, `side-screen`, and `transcript`.
- Mouse click works for selector rows, provider config actions, approval
  buttons, right-rail tasks, lane controls, and side-screen route selection.
- Mouse wheel scrolls the currently focused scrollable panel.
- Esc behavior is consistent: close modal/selector first, then clear composer
  command, then exit only where appropriate.
- Focus state is visible enough that users know where keyboard actions apply.

### 5. Provider Failure Recovery

Provider/model failure should guide the user to a fix, not just show an API
error.

Acceptance:

- Classify common failures: missing key, bad endpoint, auth failure, rate
  limit, timeout, model unavailable, unsupported tool-call format, context
  overflow, and provider compatibility mismatch.
- Each class produces concrete actions: switch model, open `/models`, open
  provider config, run doctor, use fallback, or retry.
- If the selected model is known risky or fails compatibility checks, TUI shows
  a model recovery prompt.
- `/doctor` and `/provider doctor <id>` share the same provider readiness facts.
- Add focused tests for failure classification and recovery prompts.

### 6. Daily Coding Loop Evidence

The normal single-agent coding loop must be provable.

Acceptance:

- A deterministic daily-loop smoke covers:
  prompt -> provider turn -> approval -> write file -> shell/test -> diff/test
  evidence -> final summary.
- The main screen shows `NOW WORKING` during thinking, tool calls, approvals,
  shell/test execution, and completion.
- The right rail and recent files update from real runtime facts.
- Add screenshots for live thinking, approval, shell/test running, diff/test
  evidence, and final summary.

### 7. Delegated Review Loop Beta

Keep one delegated lane workflow usable while broader agent integrations stay
secondary.

Acceptance:

- `/lane` root opens an actionable selector, not only text help.
- Lane id operations can be selected without typing ids:
  inspect, timeline, diff, artifacts, accept, apply, discard, retry, stop,
  cleanup.
- Side-1 shows lane console/tail/transport state.
- Side-2 shows artifacts, changed files, context pressure, decision state, and
  apply/conflict status.
- A deterministic shell/template lane remains CI baseline.
- Codex/Claude remain optional probes unless their happy path is stable enough
  to be verified on the release machine.

## P1 Scope

- Favorite providers/models and last-known-good model history.
- Remote model discovery where provider APIs make this practical.
- Better tmux/PTY attach ergonomics and lane input forwarding.
- Provider-side token/context pressure warnings before a request is sent.
- More compact right-rail layouts for narrow terminals.

## Non-Goals

- Do not add a broad plugin marketplace in `0.1.20`.
- Do not turn ACP/MCP/skills into mutating runtime surfaces in this release.
- Do not make Codex/Claude write-capable by default.
- Do not save API keys in plaintext config as the default setup path.
- Do not treat attractive screenshots as completion without smoke evidence.

## Test Plan

Focused:

- setup wizard state transitions and persistence boundaries;
- provider config action selection and mouse hit testing;
- settings/provider/model/permissions/theme selector behavior;
- provider failure classification and recovery prompts;
- composer caret and CJK preview rendering;
- resize stress scenarios for selector, approval, lane detail, and composer;
- lane selector id/action flows;
- daily coding loop runtime state projection.

Regression:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-previews.sh docs/previews/generated
scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh
scripts/release-smoke.sh --version 0.1.20 --quick \
  --out-dir /tmp/viden-0120-release-smoke-local
```

Manual:

- macOS Terminal and iTerm2 first-run setup.
- Resize while selector, approval, lane detail, and provider turn are active.
- CJK input with IME candidate window.
- Mouse selection in provider config, model selector, approval, right rail, and
  lane controls.
- Fallback provider daily loop.
- DeepSeek live provider daily loop when credentials are available.
- Deterministic delegated lane review loop.

## Screenshot Evidence

Required deterministic or real-use screenshots:

- first-run setup wizard;
- provider list and `PROVIDER CONFIG`;
- provider-grouped model selector;
- settings hub;
- visible composer/caret/CJK input;
- resize stress state after redraw;
- live provider thinking / `NOW WORKING`;
- approval modal default action;
- shell/test running;
- diff/test evidence;
- lane selector;
- lane running side-1;
- lane evidence side-2;
- lane accept/apply/discard;
- final daily-loop summary.

## Release Standard

`0.1.20` is complete only when:

- clean-install setup can be completed from the TUI;
- fallback and live-provider recovery paths are documented and tested;
- daily coding loop smoke passes;
- delegated lane operator-loop smoke passes;
- interaction regression covers resize, mouse, selector, approval, and CJK
  input;
- screenshots are generated and referenced from docs;
- README and user guide describe only implemented behavior;
- GitHub release assets and Homebrew formula are published;
- post-publish smoke validates GitHub assets and Homebrew.
