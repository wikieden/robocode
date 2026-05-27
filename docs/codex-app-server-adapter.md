# Codex App-Server Adapter Notes

Chinese version: [codex-app-server-adapter.zh-CN.md](codex-app-server-adapter.zh-CN.md)

Last checked: 2026-05-26 with `codex-cli 0.133.0`.

## Why This Matters

RoboCode 0.1.7 treats Codex as the first host-delegate agent backend. The
current implementation can launch Codex CLI jobs, track logs/results, show
active work in the TUI, extract resume/file evidence, and gate write-capable
delegation behind RoboCode permissions.

The next maturity step is to replace heuristic log/result parsing with Codex
app-server protocol events. The local Codex CLI already exposes protocol
metadata through:

```bash
codex app-server --help
codex app-server generate-json-schema --experimental --out <dir>
codex app-server generate-ts --experimental --out <dir>
```

## Confirmed Protocol Surface

The generated schema exposes the pieces RoboCode needs for a real adapter:

- Client requests:
  `initialize`, `thread/start`, `thread/resume`, `thread/read`,
  `thread/list`, `turn/start`, `turn/steer`, `turn/interrupt`,
  `review/start`, `thread/turns/list`, and `thread/turns/items/list`.
- Server notifications:
  `thread/started`, `thread/status/changed`, `thread/tokenUsage/updated`,
  `thread/name/updated`, `thread/goal/updated`, `turn/started`,
  `turn/completed`, `turn/diff/updated`, `turn/plan/updated`,
  `item/started`, `item/completed`, `item/agentMessage/delta`,
  `item/commandExecution/outputDelta`, `item/fileChange/outputDelta`,
  `item/fileChange/patchUpdated`, `fs/changed`, and `error`.
- Server approval requests:
  `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`,
  `item/permissions/requestApproval`, `execCommandApproval`,
  `applyPatchApproval`, and `fileChange`.
- Thread identity fields:
  generated schemas include `threadId` and `conversationId` across approval,
  turn, and notification payloads.

## RoboCode Mapping

RoboCode should map app-server data into the existing host-delegate lifecycle:

| Codex app-server signal | RoboCode artifact |
| --- | --- |
| `thread/started`, `thread/resume` | `AgentJobRecord` thread/resume handle |
| `thread/status/changed` | lane/job state |
| `turn/started`, `turn/completed` | operation-center state and job evidence |
| `item/agentMessage/delta` | transcript/lane output stream |
| `item/commandExecution/outputDelta` | command/test evidence stream |
| `item/fileChange/*`, `turn/diff/updated`, `fs/changed` | touched-file and diff evidence |
| approval requests | RoboCode permission prompt, transcript permission log |
| `error` | failed job/lane evidence |

## Implementation Sequence

1. Done: `/agent doctor codex` now runs an app-server protocol probe that
   generates schema into a temporary directory and reports key request,
   notification, evidence, and approval protocol groups.
2. Done: `/agent probe codex` starts `codex app-server --listen stdio://`,
   sends `initialize`, and records response/notification evidence in
   `.robocode/agents/codex-app-server-*.jsonl`.
3. Done: `/agent probe codex --thread` declares `experimentalApi`, starts an
   ephemeral read-only Codex thread, captures the structured `threadId`, and
   records `thread/started` evidence without running a model turn.
4. Done: `/agent probe codex --turn <task>` starts a read-only turn, records
   `turn/started`, streamed item notifications, and `turn/completed` evidence in
   `.robocode/agents/codex-app-server-*.jsonl`.
5. Done: completed turn probes now map structured `threadId`, `turnId`, and
   completion status into tracked Codex job records and result summaries.
   Result summaries also persist final `agentMessage` text as `message:`.
6. Done: result summaries now persist protocol `signals:` for command output,
   file changes, patch updates, diff updates, filesystem changes, MCP tool
   calls, MCP file writes, and app-server errors. These summaries are derived
   from notifications and remain backed by the raw JSONL log.
7. Done: `/agent probe codex --turn-write <task>` exists only as an
   environment-gated disposable-workspace experiment. It is disabled by default
   because a live safety trial showed workspace-write turns can mutate files
   before RoboCode receives an approval request.
8. Done: `/agent run codex --app-server <task>` starts an asynchronous
   read-only app-server turn job and keeps default `/agent run codex` on the CLI
   fallback.
9. Done: approval-like server requests are captured in the JSONL evidence and
   answered with decline/no-grant responses so app-server work cannot hang or
   bypass RoboCode permission boundaries. This is not yet enough for
   write-capable turns because some workspace-write mutations can happen before
   a request is emitted.
10. Done: TUI `AgentTask` projection now reads app-server result/log artifacts
   for thread, turn, status, approval, resume, command-output, file-change,
   patch, diff, filesystem, MCP tool-call, MCP file-write, error, and
   final-message evidence.
11. Promote app-server execution behind a config flag/default after live smoke
   coverage proves normal jobs can use the protocol path safely.
12. Retire text heuristics only after structured `threadId`, file, command, and
   test events are available in normal jobs.

## Current Boundary

Until app-server turn execution is wired into normal jobs, CLI-backed jobs
remain the stable fallback. They must keep:

- read-only default execution;
- explicit `/agent run codex --write <task>` for mutation;
- RoboCode permission approval before write-capable launch;
- `.robocode/agents/` job records, logs, results, baseline status, and evidence
  extraction.

Repeatable local smoke:

```bash
scripts/smoke-codex-app-server.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-codex-app-server-write-guard.sh
```

The live smoke requires local Codex auth and rate-limit availability. It
verifies a real text turn, tracked job completion, result `thread` / `turn` /
`resume` / `message` fields, and final-message evidence. The protocol-fixture
smoke uses a mock app-server through the same CLI/probe/result path to
deterministically cover command, file, approval, MCP tool-call / MCP file-write,
and error event classes.

Command, file, approval, MCP, and error live event smokes remain follow-up
before making app-server the default path. The fixture proves RoboCode's
ingestion and display path, not the live model's willingness to emit each event
in every turn. A disposable live write probe on 2026-05-27 showed that Codex
Desktop can mutate a workspace through an `mcpToolCall` without first emitting a
RoboCode approval request; RoboCode now records that as `mcp-tool-call`,
`mcp-tool-completed`, and `mcp-fs-write`, but the path must stay opt-in. The
write-guard smoke verifies that write-capable app-server probes are blocked
before launch unless
`ROBOCODE_EXPERIMENTAL_CODEX_APP_SERVER_WRITE=1` is set in a disposable
workspace.
