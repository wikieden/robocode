# RoboCode 0.1.30 Plan - Final Zero-Bug TUI Gate

Chinese version: [release-0.1.30-plan.zh-CN.md](release-0.1.30-plan.zh-CN.md)

`0.1.30` is the final 0.1.x zero-bug gate. It closes the Mode/Permission UI
surface that was scoped from the earlier 0.1.26 work and turns the remaining
TUI stability expectations into a hard prepublish requirement.

## Goals

- Wire topbar, composer, status, mode, permission, and provider/model labels to
  real runtime evidence rather than static display text.
- Keep the composer usable while provider turns, plan mode, approvals, and tool
  jobs are active: typing, queueing, cancel, and history scroll must not lock.
- Treat Plan mode as a planning-only mode: requirements, architecture,
  implementation plans, and task lists are allowed; file, shell, Git, workflow,
  memory, and task mutations remain blocked.
- Require a live DeepSeek development scenario smoke for every release and
  record tokens, elapsed time, estimated cost, and failure classification.
- Require the final zero-bug gate with deterministic TUI screenshots plus real
  macOS Terminal and iTerm2 evidence before publishing.

## Release Gate

Before publishing `0.1.30`, run:

```bash
export ROBOCODE_TUI_MANUAL_EVIDENCE_DIR=docs/previews/manual/0.1.30
scripts/final-zero-bug-contract-smoke.sh
scripts/final-zero-bug-smoke.sh /tmp/robocode-0130-final-zero-bug
scripts/release-gate.sh --version 0.1.30 --phase prepublish --out-dir /tmp/robocode-0130-release-gate
```

The prepublish gate automatically runs `scripts/final-zero-bug-smoke.sh` for
`0.1.30`. The final zero-bug gate must fail when manual Terminal/iTerm2
screenshots are missing.

After publishing GitHub assets and syncing Homebrew:

```bash
scripts/release-gate.sh --version 0.1.30 --phase postpublish --out-dir /tmp/robocode-0130-release-gate
```

`0.1.30` is complete only when:

- P0/P1 TUI backlog is zero;
- deterministic TUI regression, final zero-bug smoke, plan-mode smoke,
  daily-loop smoke, and RC TUI stability smoke pass;
- prepublish gate passes with live DeepSeek token/cost evidence;
- GitHub Release `v0.1.30` is published with assets and checksums;
- `wikieden/homebrew-tap` points to `0.1.30`;
- postpublish validation passes.
