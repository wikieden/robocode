# Viden 0.1.9 Plan

Chinese version: [release-0.1.9-plan.zh-CN.md](release-0.1.9-plan.zh-CN.md)

Last updated: 2026-05-27

## Version Positioning

`0.1.8` published the live multi-agent cockpit foundation: unified task
projection, operation-center status, stronger `/test` evidence, side-screen
previews, release packaging, and Homebrew tap publication.

`0.1.9` should make that foundation trustworthy enough for broader hands-on
testing.

Version theme:

```text
0.1.9 = Verification Hardening + Screenshot-Gated UX
```

Goal: every important programming workflow should be covered by deterministic
tests, release smoke evidence, and a real-use screenshot or terminal capture
that the product owner can inspect before the feature is considered done.

## P0: Must Ship

### 1. Release Gate Hardening

Goal: one release command should prove that the build, package, smoke tests,
and install paths are healthy.

Deliverables:

- Extend `scripts/release-smoke.sh` into the canonical release gate.
- Include `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  focused crate tests, `cargo test --workspace`, package smoke, fallback CLI
  smoke, Codex app-server protocol fixture, app-server write guard, lane
  operator loop smoke, and TUI preview generation.
- Keep DeepSeek live smoke and GitHub release asset validation as explicit
  opt-in checks.
- Add Homebrew tap verification after a release candidate is tagged or
  published.
- Write a structured evidence summary such as `release-evidence.json` into the
  smoke output directory.
- Implement opt-in `--github-release-assets` and `--homebrew` release-smoke
  checks so post-publication validation has one command surface.

Acceptance checks:

- A maintainer can run one local release gate and know which sub-check failed.
- The evidence directory contains logs, package metadata, generated previews,
  and a concise summary.
- The release docs list the exact command and evidence directory for the
  accepted build.

### 2. TUI Regression Harness

Goal: stop visual regressions in layout, borders, colors, resize handling,
composer visibility, command palette, and approval overlays.

Deliverables:

- Add a dedicated TUI regression script that generates stable text/ANSI/SVG
  snapshots for main idle, main active, approval overlay, command palette,
  side-1, side-2, and theme variants.
- Cover representative terminal sizes: compact, default, wide desktop, and
  portrait side-screen layouts.
- Add assertions for panel bounds, right-rail width, composer height, command
  palette placement, and side-screen renderability.
- Add a color-integrity check so section labels and borders do not drift into
  unintended mixed colors.
- Keep screenshot artifacts under `docs/previews/generated/` or a clearly named
  evidence directory.
- The first regression entrypoint is `scripts/tui-regression.sh`, which wraps
  preview generation and exports 0.1.9-named screenshot artifacts.

Acceptance checks:

- Resizing no longer leaves stale panel fragments or broken right-rail borders.
- The composer is visibly taller, cursor state is represented, and CJK text
  input remains inside the composer.
- Every TUI-affecting change includes regenerated screenshots or text snapshots.

### 3. Screenshot Confirmation Gate

Goal: every user-visible feature point must end with real usage evidence that
can be reviewed visually.

Deliverables:

- For every feature task, capture at least one real-use screenshot, terminal
  recording frame, or deterministic TUI SVG that shows the feature in context.
- Store screenshots in `docs/previews/generated/` for canonical previews or
  under the release evidence directory for temporary validation artifacts.
- The final report for each feature must link the screenshot path and state what
  scenario it proves.
- Approval overlays, side screens, command palette, lane views, `/test`
  evidence, and install flows all require separate evidence images or captures.

Acceptance checks:

- No user-visible feature is marked complete without a screenshot or equivalent
  visual artifact.
- The screenshot shows the real screen state after running the feature, not only
  a mocked design image.
- The product owner can approve or reject each feature from its visual evidence.

### 4. AgentTask and Lane Verification

Goal: multi-agent orchestration should be testable without relying on live
Codex, Claude, or DeepSeek availability.

Deliverables:

- Add fixtures that normalize Viden primary turns, tool calls, approvals,
  `/test`, shell jobs, Codex jobs, Claude/DeepSeek lanes, tmux, PTY, and future
  ACP-style events into the same `AgentTask` view.
- Add tests for lifecycle transitions: queued, thinking, streaming, editing,
  running tool, testing, waiting approval, blocked, done, failed, cancelled, and
  archived.
- Expand lane smoke coverage for `/lane inspect`, `/lane send`, `/lane accept`,
  `/lane revise`, `/lane discard`, `/lane apply`, conflict handling, cleanup,
  archive, tmux, and PTY evidence paths.
- Verify that the main operation center always explains what the active task is
  doing and names the evidence source.

Acceptance checks:

- Mocked Codex, Claude, DeepSeek, shell, tmux, and PTY events produce stable
  `AgentTask` rows.
- The same active task appears consistently in the operation center, right rail,
  side-1, side-2, and command output.
- A lane can be started, observed, followed up, accepted or discarded, and then
  audited from artifacts.

### 5. Permission and Safety Regression Suite

Goal: testing must prove that new agent, plugin, MCP, lane, and app-server
paths do not bypass approval, transcript, or workspace safety.

Deliverables:

- Add regression tests for shell, file write, Git mutation, app-server write,
  lane mutation, plugin/MCP invocation, and plan-mode mutation blocking.
- Keep Codex app-server write-capable turns behind the explicit experimental
  guard until live protocol behavior is safe enough to default on.
- Add path-scope tests for workspace-external writes, path traversal, hidden
  files, and generated artifacts.
- Ensure approval decisions and denials are written to transcript or evidence
  logs.

Acceptance checks:

- Mutating paths fail closed when permission state is missing or denied.
- Plan mode blocks file, shell, Git, task, memory, lane, plugin, MCP, and
  app-server mutation paths.
- Safety checks are part of the default release gate.

## P1: Should Ship

### 6. CI Matrix Upgrade

- Split CI into PR fast, main full, and release full gates.
- PR fast should run fmt, clippy, focused tests, and quick smoke.
- Main full should run workspace tests, TUI regression, lane smoke, and release
  smoke quick.
- Release full should build all supported packages, upload artifacts, verify
  sha256, and validate Homebrew tap publication.

### 7. Provider Compatibility Matrix

- Keep fallback provider as the deterministic baseline.
- Keep DeepSeek live smoke opt-in and document required environment variables.
- Add provider fixture tests for OpenAI-compatible and Anthropic-style response
  shapes, including tool-call replay and non-null assistant tool content.

### 8. Documentation and Operator Playbooks

- Add a testing and validation guide that explains local, CI, live-provider,
  release, and screenshot-gated verification.
- Keep English and Chinese docs in sync.
- Add a release checklist that includes screenshot review and product-owner
  confirmation before tagging.

## Non-Goals

- No new marketplace or cloud registry.
- No broad ACP implementation beyond fixtures and mapping tests.
- No default app-server write path until live safety behavior is proven.
- No new dependencies unless a test gap cannot be closed with the existing
  Rust and shell stack.

## Screenshot Evidence Contract

Every feature completion report must include:

- feature name;
- command or workflow used to exercise it;
- screenshot or visual artifact path;
- what the artifact proves;
- remaining visual risk, if any.

Recommended artifact names:

```text
docs/previews/generated/0.1.9-<feature>-main.svg
docs/previews/generated/0.1.9-<feature>-approval.svg
docs/previews/generated/0.1.9-<feature>-side-1.svg
docs/previews/generated/0.1.9-<feature>-side-2.svg
```

Temporary release evidence may also live under:

```text
/tmp/viden-019-release-smoke-*/screenshots/
```

## Suggested Build Order

1. Add the testing and validation guide plus release checklist.
2. Upgrade `scripts/release-smoke.sh` to emit structured evidence.
3. Add the TUI regression script and screenshot evidence naming convention.
4. Expand `AgentTask` and lane fixtures.
5. Add permission and safety regression coverage.
6. Wire CI gates for PR, main, and release.
7. Run a full 0.1.9 release-candidate validation pass with screenshots.

## Verification Gate

Before 0.1.9 can ship:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- focused tests for touched crates
- `cargo test --workspace --quiet`
- `scripts/release-smoke.sh --quick`
- `scripts/release-smoke.sh --version 0.1.9 --deepseek` when live provider
  credentials are available
- TUI regression snapshots for all required visual states
- lane operator smoke with artifact inspection
- permission and app-server write-guard smoke
- package smoke for host platform
- GitHub release asset and sha256 validation after publication
- Homebrew tap fetch/install verification after tap update
- screenshot confirmation for every user-visible feature point
