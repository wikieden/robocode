# Viden 0.1.29 Plan - RC TUI Stability

Chinese version: [release-0.1.29-plan.zh-CN.md](release-0.1.29-plan.zh-CN.md)

`0.1.29` is the 0.1.x release-candidate stabilization slice. It does not add a
new UI surface. It turns the remaining P0/P1 TUI stability expectations into a
single release gate that can be audited before the final zero-bug exit.

## Goals

- Freeze feature expansion for the 0.1.x line and fix only release-blocking
  TUI stability issues.
- Add an RC TUI stability smoke that proves the known P0/P1 guardrails:
  fake-slow provider non-blocking behavior, approval non-blocking behavior,
  streaming scrollback preservation, focus/paste repaint policy, composer
  terminal-residue filtering, provider/model setup picker behavior, LIVE WORK
  preview contract, synthetic planning cleanup, and deterministic TUI previews.
- Record a release-visible P0/P1 TUI backlog summary.
- Make manual macOS Terminal/iTerm2 screenshot evidence explicit: release status
  must either link the real screenshots or record the remaining manual evidence
  risk before the final `0.1.30` zero-bug gate.

## Release Gate

Before publishing `0.1.29`, run:

```bash
scripts/rc-tui-stability-contract-smoke.sh
scripts/rc-tui-stability-smoke.sh /tmp/viden-0129-rc-tui-stability
scripts/release-gate.sh --version 0.1.29 --phase prepublish --out-dir /tmp/viden-0129-release-gate
```

The prepublish gate must include the live DeepSeek development scenario and
record token, elapsed-time, estimated-cost, and failure-class evidence.

After publishing GitHub assets and syncing Homebrew:

```bash
scripts/release-gate.sh --version 0.1.29 --phase postpublish --out-dir /tmp/viden-0129-release-gate
```

`0.1.29` is complete only when:

- RC TUI stability smoke passes and records P0/P1 backlog status;
- deterministic TUI regression, plan-mode smoke, daily-loop smoke, and lane
  operator smoke pass;
- prepublish gate passes with live DeepSeek smoke evidence;
- GitHub Release `v0.1.29` is published with assets and checksums;
- `wikieden/homebrew-tap` points to `0.1.29`;
- postpublish validation passes.
