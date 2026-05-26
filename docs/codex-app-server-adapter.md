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
6. Route server approval requests into the existing RoboCode permission path.
7. Retire text heuristics only after structured `threadId`, file, command, and
   test events are available in normal jobs.

## Current Boundary

Until app-server turn execution is wired into normal jobs, CLI-backed jobs
remain the stable fallback. They must keep:

- read-only default execution;
- explicit `/agent run codex --write <task>` for mutation;
- RoboCode permission approval before write-capable launch;
- `.robocode/agents/` job records, logs, results, baseline status, and evidence
  extraction.
