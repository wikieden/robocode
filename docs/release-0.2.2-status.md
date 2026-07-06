# Viden 0.2.2 Status - Agent DAG and Role Runtime Closure

Chinese version: [release-0.2.2-status.zh-CN.md](release-0.2.2-status.zh-CN.md)

`0.2.2` content is complete in the current working tree. This is a core
runtime checkpoint, not a GitHub/Homebrew distribution release.

## Status

- Scope: Agent DAG and role runtime.
- Product name: Viden.
- Release publication: not published.
- Distribution gates: not applicable until a GitHub Release is cut.

## Completed Scope

- `StartAgentDag`, `StartAgentTask`, and `CancelAgentTask` runtime commands.
- Supervised planner, coder, reviewer, tester, doc-writer, release-operator,
  and external-agent role records.
- Dependency-gated role task execution.
- AgentTask-bound ContextBundle events with role guidance, file scope,
  evidence contract, scoped file candidates, symbol candidates, and live LSP
  diagnostics.
- Provider-backed task outputs written to `task.result` and linked to role
  evidence.
- Durable workflow events for queued, started, blocked, completed, failed, and
  cancelled agent tasks.
- Failure classification with recovery suggestions and retry next actions.
- MergeGate creation, accept/reject commands, artifact accept/reject commands,
  accepted patch merge command, and durable conflict reporting.
- Scoped role permission policy covering read-only, docs-only, tester
  verification, scoped coder mutation, release-gate, external-agent denial,
  scoped `git_add`, unscoped staging denial, and high-risk Git denial.
- Structured tool-result runtime events with success and exit-code fields.
- RuntimeSupervisor cancellation paths that keep the worker alive.

## Verification

- PASS `cargo fmt --all --check`
- PASS `git diff --check`
- PASS `cargo test -p viden-runtime`
- PASS `cargo test --workspace --quiet`

The live DeepSeek test remains ignored by default because it requires
`DEEPSEEK_API_KEY`, network access, and billable provider usage.

## Deferred To Later 0.2.x Work

- Live LSP references enrichment.
- Release/publish Git rules beyond scoped staging and high-risk mutation denial.
- Evidence collection reducers.
- Rename/delete/binary patch handling and three-way conflict resolution.
- External-agent plugin adapters.

These are tracked as `0.2.3+` work in the staged roadmap and do not block the
`0.2.2` Agent DAG and role runtime closure.
