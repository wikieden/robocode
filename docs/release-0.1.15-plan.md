# Viden 0.1.15 Plan

Chinese version: [release-0.1.15-plan.zh-CN.md](release-0.1.15-plan.zh-CN.md)

Last updated: 2026-05-29

## Positioning

`0.1.15` is the **Context Curator And Budget Controls** release.

`0.1.14` made delegated lanes more trustworthy: adapter capability doctor,
timeline evidence, isolation declarations, Codex read-only review, and safer
Claude/tmux setup. The next bottleneck is context control. Viden should help
the operator understand and shape what each provider or delegated lane sees,
what was omitted, and why.

The product question for this release is:

> What context is each agent using, what did Viden leave out, and how much
> budget pressure is this task creating?

## Goals

- Promote ContextBundle from a compact status string to a policy-governed fact
  model.
- Make source priority, budget pressure, omitted-source reasons, and compaction
  notes visible in provider turns and lane envelopes.
- Add the foundation for future pin/omit controls without building a large UI
  yet.
- Preserve raw transcript, tool, test, and lane logs even when model-facing
  context is compacted or omitted.

## P0 Scope

### ContextBundle v1 Policy Records

- Add shared fields for:
  - `policy`
  - source `priority`
  - source `include_reason`
  - `omitted_sources`
  - omission `reason`
- Apply the same `v1-priority-budget` policy to main provider turns and lane
  envelopes.
- Include the policy in `AgentTask` evidence rows and provider context messages.

Acceptance:

- JSON roundtrip tests cover the new shared record shape.
- Provider context messages show policy and omitted-source sections.
- Lane envelopes show `ContextBundle v1`, source priority, policy, and omitted
  sources.

### Context Visibility Commands

- Add a compact operator command for the latest provider ContextBundle.
- Show sources ordered by priority and token estimate.
- Show omitted sources and compaction notes.
- Keep it read-only.

Acceptance:

- A user can inspect the latest provider bundle without reading raw transcript
  JSON.
- Empty/missing bundle reports a clear next step.

### TUI Evidence Rows

- Surface context policy and omitted-source count in side-2 ops context.
- Keep current context pressure rows.
- Add deterministic screenshot evidence after visible rows land.

Acceptance:

- Side-2 can answer policy, pressure, source count, and omitted count at a
  glance.
- Screenshot regression includes the updated side-2 view.

## P1 Scope

- Source pin/omit command design and docs.
- Per-lane soft/hard budget overrides.
- Retry lineage that records prior omissions and previous failure context.
- Budget stop evidence for long-running delegated lanes.

## Non-Goals

- Full spec workflow.
- Hook runtime.
- Mutating ACP/MCP/plugin/skill invocation.
- Automatic multi-agent task splitting.
- Marketplace/install UX.

## Verification

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- Focused ContextBundle tests for JSON shape, provider message rendering, lane
  envelope rendering, and side-2 rows.
- `VIDEN_TUI_SCREENSHOT_VERSION=0.1.15 scripts/tui-regression.sh docs/previews/generated`
