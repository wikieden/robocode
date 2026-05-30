# RoboCode 0.1.21 Plan - Interaction System Completion

Chinese version: [release-0.1.21-plan.zh-CN.md](release-0.1.21-plan.zh-CN.md)

## Summary

`0.1.21` should turn the interaction work from `0.1.18` through `0.1.20`
into one coherent product system. The release goal is not to add more panels or
more agent types. The goal is that a new user can configure RoboCode, recover
from provider errors, run a coding loop, use delegated lanes, and understand
where focus and actions live without guessing hidden slash commands.

This release remains inside the V2 developer enhancement layer, while preparing
the V3 orchestration runtime. MCP, ACP, plugins, and skills should stay mostly
read-only or descriptor-level unless they reuse the same settings, task, focus,
permission, and evidence contracts defined here.

## Product Goal

By the end of `0.1.21`, RoboCode should feel like a dependable terminal product:

- every settings/configuration entry opens an actionable picker or form;
- provider configuration and model selection are visually and behaviorally
  distinct;
- keyboard, mouse, Esc, and Enter behavior are consistent across modals;
- composer, command palette, approval modal, lane selector, and side screens
  share one focus model;
- active work is visible in the center of the main screen and backed by the
  shared `AgentTask` snapshot;
- delegated lanes can be launched, inspected, reviewed, and resolved without
  typing lane ids by hand;
- each user-visible workflow has deterministic screenshots and at least one
  smoke or focused test.

## Baseline From 0.1.20

`0.1.20` is the usability beta gate and introduces or plans the following:

- dedicated `/setup` wizard selector;
- first-run setup preload when an online provider is missing an API key;
- separated `/provider` supplier configuration and `/models` model selection;
- provider failure classification with concrete recovery hints;
- `/lane` root action selector;
- deterministic screenshots for setup, provider, model, lane, side screens,
  resize, CJK input, command palette, and live-turn states.

Remaining gaps:

- provider detail pages still do not feel like editable configuration forms;
- settings surfaces are not all driven by one modal/form component contract;
- mouse and focus behavior is still too ad hoc;
- lane actions are discoverable but not yet fully one-click/no-id flows;
- `NOW WORKING`, right rail, and side screens still need stricter shared
  `AgentTask` consistency checks;
- screenshots exist, but manual real-use acceptance still needs to become part
  of the release checklist.

## P0 Scope

### 1. Unified Settings And Form Runtime

All configuration surfaces should use one interaction model.

Acceptance:

- `/settings` opens a hub with provider, model, permissions, theme, defaults,
  diagnostics, and setup entries.
- `/setup` is a staged wizard backed by the same selector/form runtime, not a
  special one-off renderer.
- `/provider` lists suppliers. Enter or click opens provider detail. Provider
  detail supports inspect, set as default, switch now, edit endpoint, show key
  env hint, run doctor, probe model, and open `/models` filtered to that
  provider.
- `/models` groups models by provider and clearly marks current, configured,
  favorite, risky, and unavailable states when the data exists.
- `/permissions`, `/theme`, and mode/default settings use the same keyboard,
  mouse, footer, and screenshot contracts.
- Long rows render summary plus detail panes instead of clipping the most
  important information.

### 2. First-Run Setup Completion

Finish the clean-install path as a real wizard.

Acceptance:

- Missing API key opens setup and focuses the provider/key step.
- The wizard shows the exact env var for the selected provider and a copyable
  shell export command in transcript/evidence form.
- DeepSeek is the default online path; fallback remains the offline path.
- Probe results lead to one of: continue, switch model, edit endpoint, doctor,
  fallback, or retry.
- Saving defaults writes only non-secret provider/model/default settings.
- Add tests for each setup state transition and persistence boundary.

### 3. Focus, Mouse, And Modal Router

Make the TUI predictable under keyboard and mouse use.

Acceptance:

- Define explicit focus targets: composer, command palette, selector/form,
  approval, transcript, right rail, lane detail, side-1, and side-2.
- Esc behavior is deterministic: close modal, then close palette, then clear
  command input, then exit only for direct exit actions.
- Enter behavior is deterministic: submit composer, apply selected modal row,
  or activate focused approval control.
- Mouse click selects and activates rows/buttons where expected.
- Mouse wheel scrolls the focused scrollable pane.
- Focus state is visually visible and covered by deterministic preview states.
- Add regression tests for selector, provider detail, model selector,
  approval, lane selector, and side-screen focus transitions.

### 4. Composer And Command Palette Reliability

Keep the input area calm during daily use.

Acceptance:

- Composer has stable height and remains readable in narrow and tall terminals.
- The caret remains visible, including during CJK input.
- IME candidate windows stay near the typed text in macOS Terminal and iTerm2
  manual smoke runs.
- `/` command discovery shows only actionable rows for decision commands.
- `/provider`, `/models`, `/settings`, `/setup`, and `/lane` never degrade into
  passive status pages.
- Add resize stress coverage for composer plus each modal family.

### 5. AgentTask-Backed Work Visibility

The center of the main screen must answer "what is happening now?"

Acceptance:

- Provider thinking, streaming, tool calls, approvals, shell/test execution,
  lane dispatch, lane review, and completion all write/update one shared
  `AgentTask` snapshot.
- `NOW WORKING`, right rail active tasks, side-1 lane list, side-2 evidence,
  `/agent status`, and `/lane inspect` read the same task facts.
- Background count, blocked count, active lane count, latest evidence, and next
  action are consistent across surfaces.
- Add focused tests that compare the same task state across main, right rail,
  side-1, and side-2 preview output.

### 6. Delegated Lane No-Guess Flow

Lane orchestration should not require remembering ids or hidden verbs.

Acceptance:

- `/lane` lists launch actions and id-specific actions for every tracked lane.
- Lane detail pages expose inspect, timeline, diff, artifacts, accept, apply,
  discard, retry, stop, and cleanup as selectable actions.
- Side-1 can focus a lane and open lane detail.
- Side-2 shows evidence, artifacts, context pressure, changed files, and
  conflict/apply state.
- Deterministic shell/template lanes remain the P0 baseline.
- Codex/Claude/tmux lanes can stay probe-level, but their status and evidence
  must map into the same `AgentTask` and lane UI when present.

### 7. Release Evidence Discipline

Every visible feature needs proof.

Acceptance:

- Add or update deterministic screenshots for settings hub, provider detail,
  setup key step, setup probe result, model selector, approval focused states,
  lane action selector, lane detail, side-1, side-2, and daily-loop final state.
- Add a manual checklist for macOS Terminal and iTerm2 covering first-run setup,
  provider switch, model switch, approval, CJK input, resize, mouse selection,
  and delegated lane review.
- Release status must list which screenshots prove which feature.

## P1 Scope

- Favorite providers/models and last-known-good recovery suggestions.
- Provider/model search quality, including aliases and provider-scoped filters.
- Better narrow-terminal layouts for right rail and provider detail pages.
- Read-only MCP/plugin/skill capability browser that uses the unified settings
  modal contract.
- More lane templates for review, test, docs, and shell task flavors.
- Token/context budget warnings before provider requests.

## Non-Goals

- Do not make ACP or MCP a mutating runtime in `0.1.21`.
- Do not add new third-party UI dependencies unless the existing terminal
  widget layer cannot satisfy the focus/form requirements.
- Do not save API keys into plaintext config by default.
- Do not treat Codex/Claude write-capable happy paths as release blockers.
- Do not add new decorative panels without runtime facts and tests.

## Test Plan

Focused:

- settings/form state transitions;
- provider detail actions and persistence boundaries;
- model selector grouping and switch commands;
- setup wizard key/probe/save states;
- focus router and Esc/Enter behavior;
- mouse hit testing for selector rows and action buttons;
- composer/caret/CJK preview rendering;
- resize stress for composer, selector, provider detail, approval, and lane
  detail;
- shared `AgentTask` consistency across TUI surfaces;
- lane no-id action flows.

Regression:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-previews.sh docs/previews/generated
scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh
scripts/release-smoke.sh --version 0.1.21 --quick \
  --out-dir /tmp/robocode-0121-release-smoke-local
```

Manual:

- macOS Terminal and iTerm2 first-run setup.
- Provider key missing, auth failure, timeout, model unavailable, and fallback
  recovery.
- Provider detail editing and model switching.
- CJK input and IME candidate placement.
- Resize while provider detail, setup wizard, approval, lane selector, and lane
  detail are active.
- Mouse selection in provider detail, model selector, approval, right rail, and
  lane controls.
- Fallback daily coding loop.
- DeepSeek live daily coding loop when credentials are available.
- Deterministic delegated lane review loop.
