# Codex App-Server Adapter 记录

英文版：[codex-app-server-adapter.md](codex-app-server-adapter.md)

最后检查：2026-05-26，使用 `codex-cli 0.133.0`。

## 为什么重要

RoboCode 0.1.7 把 Codex 当作第一个 host-delegate agent backend。目前实现已经可以
启动 Codex CLI jobs、跟踪 log/result、在 TUI 里显示 active work、提取 resume/file
evidence，并把 write-capable delegation 放到 RoboCode permission gate 后面。

下一步成熟化是用 Codex app-server protocol events 替代启发式 log/result 解析。本机
Codex CLI 已经暴露协议元数据：

```bash
codex app-server --help
codex app-server generate-json-schema --experimental --out <dir>
codex app-server generate-ts --experimental --out <dir>
```

## 已确认协议面

生成的 schema 已经包含 RoboCode 真正 adapter 所需的关键部分：

- Client requests：
  `initialize`、`thread/start`、`thread/resume`、`thread/read`、
  `thread/list`、`turn/start`、`turn/steer`、`turn/interrupt`、
  `review/start`、`thread/turns/list` 和 `thread/turns/items/list`。
- Server notifications：
  `thread/started`、`thread/status/changed`、`thread/tokenUsage/updated`、
  `thread/name/updated`、`thread/goal/updated`、`turn/started`、
  `turn/completed`、`turn/diff/updated`、`turn/plan/updated`、
  `item/started`、`item/completed`、`item/agentMessage/delta`、
  `item/commandExecution/outputDelta`、`item/fileChange/outputDelta`、
  `item/fileChange/patchUpdated`、`fs/changed` 和 `error`。
- Server approval requests：
  `item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、
  `item/permissions/requestApproval`、`execCommandApproval`、
  `applyPatchApproval` 和 `fileChange`。
- Thread identity fields：
  生成的 schema 在 approval、turn 和 notification payload 中包含 `threadId` 和
  `conversationId`。

## RoboCode 映射

RoboCode 应把 app-server 数据映射到现有 host-delegate lifecycle：

| Codex app-server signal | RoboCode artifact |
| --- | --- |
| `thread/started`、`thread/resume` | `AgentJobRecord` thread/resume handle |
| `thread/status/changed` | lane/job state |
| `turn/started`、`turn/completed` | operation-center state 和 job evidence |
| `item/agentMessage/delta` | transcript/lane output stream |
| `item/commandExecution/outputDelta` | command/test evidence stream |
| `item/fileChange/*`、`turn/diff/updated`、`fs/changed` | touched-file 和 diff evidence |
| approval requests | RoboCode permission prompt 和 transcript permission log |
| `error` | failed job/lane evidence |

## 实现顺序

1. 已完成：`/agent doctor codex` 现在会运行 app-server protocol probe，把 schema
   生成到临时目录，并报告关键 request、notification、evidence 和 approval
   protocol groups 是否可用。
2. 增加一个小 JSON-RPC transport wrapper，连接 `codex app-server --listen stdio://`
   或 `codex app-server proxy`。
3. 先做 `initialize` 加 read-only `review/start` 或 `thread/start` spike，写入
   `.robocode/agents/codex-app-server-*.jsonl`。
4. 把 notifications 映射为 `AgentJobRecord` 更新和 side-screen evidence。
5. 把 server approval requests 接入现有 RoboCode permission path。
6. 只有在普通 jobs 能拿到结构化 `threadId`、file、command 和 test events 后，
   再移除文本启发式解析。

## 当前边界

app-server transport 接入前，CLI-backed jobs 仍是稳定 fallback。必须保留：

- 默认 read-only execution；
- mutation 必须显式使用 `/agent run codex --write <task>`；
- write-capable launch 前必须经过 RoboCode permission approval；
- `.robocode/agents/` job records、logs、results、baseline status 和 evidence
  extraction。
