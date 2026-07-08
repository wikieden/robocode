# Viden 0.1.10 Plan

Chinese version: [release-0.1.10-plan.zh-CN.md](release-0.1.10-plan.zh-CN.md)

Last updated: 2026-05-27

## Target

`0.1.10` is the Programming Cockpit Feedback release. It tightens the loop
between user input, provider work, delegated agent lanes, and visual evidence.

The release should make one thing obvious at all times: what Viden is doing
right now, what produced that signal, and what the operator can do next.

## Scope

- Show live provider requests as first-class `AgentTask` records instead of
  inferring activity from transcript history.
- Keep the main operation center, right rail, side screens, and ops screen on
  the same normalized task model.
- Refresh deterministic TUI screenshots under `0.1.10` artifact names.
- Keep plugin, skill, MCP, and ACP work as visible design direction unless a
  concrete integration is actually wired through permission and runtime paths.
- Preserve the 0.1.9 release gate: format, clippy, tests, TUI regression,
  package smoke, optional DeepSeek smoke, GitHub release asset verification, and
  Homebrew verification.

## Acceptance Criteria

- Submitting a provider request in the TUI immediately surfaces a live
  `thinking` task with provider/model/workspace evidence.
- When the provider returns, the pending task clears and transcript-derived
  tool, approval, test, diff, or assistant tasks take over.
- The operation center shows the live task summary and evidence before the
  provider response completes.
- Right rail and side screens consume the same `AgentTask` projection.
- README and user guide reference `0.1.10` screenshots and installation assets.
- Release status records local and post-publish evidence before the release is
  called complete.

## Verification

Run at minimum:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
scripts/tui-regression.sh docs/previews/generated
scripts/release-smoke.sh --version 0.1.10 --deepseek --out-dir /tmp/viden-0110-release-smoke-full
```

After publishing:

```bash
scripts/release-smoke.sh --version 0.1.10 --quick --github-release-assets --homebrew --out-dir /tmp/viden-0110-postpublish-check
```

## Deferred

- Full ACP host integration for third-party coding agents.
- Mutation-capable MCP tools through the shared permission path.
- User-installable plugin and skill lifecycle commands beyond current visibility
  and planning docs.
