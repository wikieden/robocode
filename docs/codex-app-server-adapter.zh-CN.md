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
2. 已完成：`/agent probe codex` 会启动 `codex app-server --listen stdio://`，
   发送 `initialize`，并把 response/notification evidence 记录到
   `.robocode/agents/codex-app-server-*.jsonl`。
3. 已完成：`/agent probe codex --thread` 会声明 `experimentalApi`，启动一个
   ephemeral read-only Codex thread，捕获结构化 `threadId`，并记录
   `thread/started` evidence，但不会运行 model turn。
4. 已完成：`/agent probe codex --turn <task>` 会启动 read-only turn，并把
   `turn/started`、streamed item notifications 和 `turn/completed` evidence 写入
   `.robocode/agents/codex-app-server-*.jsonl`。
5. 已完成：completed turn probes 现在会把结构化 `threadId`、`turnId` 和
   completion status 映射为 tracked Codex job records 和 result summaries。
   Result summaries 也会把最终 `agentMessage` text 持久化为 `message:`。
6. 已完成：result summaries 现在会把 protocol `signals:` 持久化，用于摘要
   command output、file change、patch update、diff update、filesystem change、
   MCP tool call、MCP file write 和 app-server error。这些摘要来自 app-server
   notifications，并且仍由原始 JSONL log 支撑。
7. 已完成：`/agent probe codex --turn-write <task>` 只作为带环境变量保护的
   disposable-workspace 实验路径存在。它默认禁用，因为 live safety trial 证明
   workspace-write turn 可能在 RoboCode 收到 approval request 前直接修改文件。
8. 已完成：`/agent run codex --app-server <task>` 会启动异步 read-only
   app-server turn job，同时默认 `/agent run codex` 仍走 CLI fallback。
9. 已完成：approval-like server requests 会写入 JSONL evidence，并返回
   decline/no-grant responses，避免 app-server work 卡住或绕过 RoboCode
   permission boundaries。但这对 write-capable turn 还不够，因为部分
   workspace-write mutation 可能在 request 发出前就已经发生。
10. 已完成：TUI `AgentTask` projection 现在会读取 app-server result/log
   artifacts，提取 thread、turn、status、approval、resume、command-output、
   file-change、patch、diff、filesystem、MCP tool-call、MCP file-write、error 和
   final-message evidence。
11. 在 live smoke coverage 证明普通 jobs 可以安全使用 protocol path 后，再通过
   config flag/default 推广 app-server execution。
12. 只有在普通 jobs 能拿到结构化 `threadId`、file、command 和 test events 后，
   再移除文本启发式解析。

## 当前边界

app-server turn execution 接入普通 jobs 前，CLI-backed jobs 仍是稳定 fallback。必须保留：

- 默认 read-only execution；
- mutation 必须显式使用 `/agent run codex --write <task>`；
- write-capable launch 前必须经过 RoboCode permission approval；
- `.robocode/agents/` job records、logs、results、baseline status 和 evidence
  extraction。

可重复本地 smoke：

```bash
scripts/smoke-codex-app-server.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-codex-app-server-write-guard.sh
```

live smoke 依赖本机 Codex auth 和 rate limit 可用。它验证真实 text turn、tracked
job completion、result `thread` / `turn` / `resume` / `message` 字段，以及
final-message evidence。protocol-fixture smoke 使用 mock app-server，但走同一套
CLI/probe/result 路径，确定性覆盖 command、file、approval、MCP tool-call / MCP
file-write 和 error event 类。

command、file、approval、MCP 和 error 的 live event smoke 仍是 app-server 成为默认路径前
的后续工作。fixture 证明的是 RoboCode ingestion/display path，不代表真实模型每个
live turn 都会稳定发出这些事件。2026-05-27 的 disposable live write probe 证明
Codex Desktop 可以通过 `mcpToolCall` 在没有先发出 RoboCode approval request 的情况下修改
workspace；RoboCode 现在会把它记录为 `mcp-tool-call`、`mcp-tool-completed` 和
`mcp-fs-write`，但这条路径必须继续保持 opt-in。write-guard smoke 会验证 write-capable
app-server probe 默认在启动前被拦截；只有在 disposable workspace 显式设置
`ROBOCODE_EXPERIMENTAL_CODEX_APP_SERVER_WRITE=1` 时才允许实验。
