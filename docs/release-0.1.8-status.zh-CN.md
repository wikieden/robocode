# RoboCode 0.1.8 状态

英文版： [release-0.1.8-status.md](release-0.1.8-status.md)

最后更新：2026-05-27

## 当前阶段

`0.1.8` 已发布。版本目标见
[release-0.1.8-plan.zh-CN.md](release-0.1.8-plan.zh-CN.md)。

workspace package version 已经 bump 到 `0.1.8`；本地 packaging 已通过，GitHub
release 也已经包含跨平台 artifacts。

本 checkpoint 聚焦 P0 的第一段：统一 `AgentTask` runtime view，并让主屏
operation center、右侧 `ACTIVE TASKS` 面板和 side-2 `RECENT EVIDENCE`
开始读取同一套状态模型。

## 已完成

- 新增 `AgentTask` runtime view 字段，覆盖计划里的核心概念：
  `id`、`parent_id`、`agent`、`kind`、`transport`、`status`、`activity`、
  `summary`、`progress`、`started_at`、`updated_at`、`workspace`、`evidence`、
  `permissions`、`decision`、`result`、`resume_handle` 和 `pid`。
- `AgentTask` 现在从多个 source of truth 归一化：
  - transcript 中的主回复状态；
  - pending approval；
  - tool call / tool result；
  - `/test` command evidence；
  - terminal lanes；
  - Codex job records。
- lane 和 Codex job 状态开始映射到 0.1.8 的统一状态语言，例如 `thinking`、
  `editing`、`testing`、`waiting_approval`、`needs_input`、`blocked`、`done`、
  `failed`、`cancelled` 和 `archived`。
- 主屏 operation center 改为优先读取 `AgentTask`，并显示 `waiting approval`、
  `thinking through latest prompt`、`supervising <n> agent(s)` 等状态。
- 主屏 operation center 的状态文案进一步转成 operator-facing 语言：例如
  `DeepSeek is thinking`、`Approval needed: waiting approval for write_file`、
  `Supervising 2 agents: claude needs input`，细节行仍保留
  `AgentTask id / agent / status / progress / activity`。
- operation center 现在会从 `AgentTask.evidence` 中提升可操作的 blocker/proof
  signal：失败测试显示 `Tests failed: <command>` 和
  `next open failure, patch, rerun tests`；lane conflict 显示
  `blocked on <conflict/summary>` 和冲突处理 next action。
- 历史 approval request 在后续 approval resolution、tool result、assistant reply
  或 `/test` command result 闭环后，不再继续占用主屏状态或 approval modal。
- `/diff` 和 `/git diff` command output 现在会投影成 `AgentTask kind=diff`：
  非空 diff 会成为 `needs_input` review task，包含 files/additions/deletions/path
  evidence，并在 operation center 显示 `review diff: 2 file(s) +12 -3` 这类文案，
  在 side-2 显示 `review diff, then test or commit` next action。
- transcript projection 现在会把最近的 diff、test、tool 和 provider entry
  分别保留为独立 `AgentTask` row，而不是只保留一个 latest runtime event。
- 右侧 `ACTIVE TASKS` 面板改为读取 active `AgentTask`，approval、tool、lane 和
  Codex job 不再各自拼一套状态。
- side-2 `RECENT EVIDENCE` 改为读取 `AgentTask` runtime view，并展示
  `id / agent / status / progress / activity`，同时把 `evidence`、`decision`、
  `result` 和下一步 operator action 作为二级证据行输出。
- `side-1` 预览开始显示 normalized lane state：`testing`、`needs input`、`done`。
- 增加 focused tests，覆盖 transcript approval/tool/test 到 `AgentTask` 的投影。
- 增加 focused tests，覆盖 side-2 复用 approval、tool、lane 和 Codex job 的
  `AgentTask` evidence。
- edit/test/tool-result evidence 已开始结构化：tool call 会提取 `path` 和
  `lines`，tool result 会提取写入 path / line count / changed files，`/test`
  结果会提取 command、status、duration 和 failure summary。
- 失败 `/test` 结果现在会在 `AgentTask.evidence` 里带完整恢复线索：
  `failure`、`failing-file`、`tail` 和 `rerun <command>` 会进入 side-2 和主屏
  operation-center next action，operator 可以直接打开失败点、修补并重跑同一条命令，
  不需要从原始 transcript 里翻找。
- side-2 对 failed/blocked task 会优先展示 command/failure/path 这类可行动证据，
  避免被泛化的 `result failed` 或 `transcript ...` 挤掉。
- lane apply/conflict artifact 已接入 `AgentTask.evidence`：从
  `.robocode/lanes/L*.apply.md` 和 `L*.apply-conflict.md` 提取 patch path、
  changed files 和 direct apply conflict 摘要；blocked lane 在 side-2 会展示
  conflict / changed / patch 这类可恢复证据。
- Codex app-server job artifacts 现在会向 TUI 提供更完整的 `AgentTask`
  evidence：result 文件暴露 thread、turn、status、approval 和 resume handle，
  JSONL log 暴露 command-output、file-change、patch、diff、filesystem、
  approval 和 error protocol signal。
- Codex app-server turn result 文件现在会写入 `resume:` handle，所以真实 opt-in
  app-server job 可以展示后续继续上下文，不再只依赖测试 fixture 手写结果。
- Codex app-server JSONL log 现在会把最终 agent-message text 作为
  `AgentTask` evidence 暴露出来，所以 side-2 除了协议 thread/turn id，也能展示
  delegate 最后回答了什么。
- Codex app-server result 文件现在会把最终 `agentMessage` 写成 `message:`，
  因此 `/agent result`、TUI `AgentTask` 和 side-2 evidence 会读取同一个
  delegate answer。
- Codex app-server result 文件现在也会写入 `signals:`，用于摘要 command output、
  file change、patch、diff update、filesystem change、MCP tool call、MCP file
  write 和 app-server error 等 protocol evidence。TUI `AgentTask` evidence
  也读取同一行，所以 protocol fixture 不需要手动打开 JSONL 也能审计。
- side-2 `RECENT EVIDENCE` 现在会把 app-server `message ...` evidence 和
  command evidence 放到同一优先级，所以 completed text-turn smoke 会先展示
  delegate answer，再展示低信号的协议 id。
- TUI preview fixture 现在包含一个 completed Codex app-server job，并且
  `docs/previews/generated/side-2.txt` 可以直接看到
  `evidence message ROBOCODE_APP_SERVER_SMOKE_OK`，方便截图验收。
- 新增 `scripts/smoke-codex-app-server.sh`，用于可重复 live smoke：它会在临时
  workspace 启动真实 Codex app-server text turn，并检查 `thread`、`turn`、
  `resume`、tracked job `finished` 和 final-message evidence。
- 新增 `scripts/smoke-codex-app-server-protocol-fixture.sh`，用于确定性 mock
  app-server smoke：它走正常 CLI/probe/result 路径，并覆盖 command-output、
  file-change、file-patch、diff、filesystem-change、approval request/denial 和
  MCP tool-call / MCP file-write、error signal。
- 新增带保护的 `/agent probe codex --turn-write <task>` protocol path，仅用于
  disposable workspace 实验。它默认禁用，因为 live safety trial 证明 Codex
  app-server workspace-write turn 可能在 RoboCode 收到 approval request 前直接修改文件。
- 新增 `scripts/smoke-codex-app-server-write-guard.sh`，验证默认 guard 会在启动
  app-server 前拦截 write probe，并保持 workspace 不被修改。
- 新增 `scripts/smoke-lane-operator-loop.sh`，用于 focused operator-loop smoke：
  覆盖 shell lane、`/lane inspect`、decision evidence、embedded PTY send、tmux
  attach evidence、accept/apply、conflict review/resolve、discard/cleanup 和
  archive。
- 刷新了 `docs/previews/generated/` 下的 TUI preview 文本、ANSI 和 SVG。

## 验证

已通过：

```bash
cargo fmt
cargo fmt --check
git diff --check
cargo test -p robocode-cli --quiet
cargo test -p robocode-core --quiet
cargo test --workspace --quiet
scripts/tui-previews.sh docs/previews/generated
scripts/smoke-codex-app-server.sh
scripts/smoke-codex-app-server-protocol-fixture.sh
scripts/smoke-codex-app-server-write-guard.sh
scripts/smoke-lane-operator-loop.sh
scripts/release-smoke.sh --quick --skip-package
scripts/release-smoke.sh --quick --skip-package --deepseek --out-dir /tmp/robocode-018-release-smoke-deepseek-latest
scripts/release-smoke.sh --version 0.1.8 --deepseek --out-dir /tmp/robocode-018-release-smoke-full
gh workflow run release.yml --repo wikieden/robocode -f tag=v0.1.8 -f upload_to_release=true
```

结果：

- `robocode-cli` tests：197 passed，binary tests 2 passed / 2 ignored。
- `robocode-core` tests：93 passed。
- workspace tests：通过。
- TUI previews：`scripts/tui-previews.sh docs/previews/generated` 已生成。
  `main.txt` 现在展示 active test lane 的 operation-center next action，且不会被
  stale approval modal 遮挡；`side-2.txt` 包含 `codex-app codex done` 和
  app-server final message evidence 行。
- live Codex app-server text-turn smoke：通过；本机 `codex-cli 0.133.0` /
  `Codex Desktop/0.133.0` 生成了 completed thread/turn、tracked job、resume
  handle、result `message: ROBOCODE_APP_SERVER_SMOKE_OK` 和 final-message
  evidence。
- mock Codex app-server protocol-fixture smoke：通过；同一套 `/agent probe`
  -> tracked job -> `/agent result` surface 里出现了 `signals:
  command-output, file-change, file-patch, diff-updated, fs-changed,
  mcp-tool-call, mcp-tool-completed, mcp-fs-write, app-server-error`，并记录了
  被拒绝的 command approval request。
- live Codex app-server read-only command trial：已完成，但 Codex Desktop 回报
  当前 app-server session 没有 shell tool，因此没有发出 live command approval
  signal。
- live Codex app-server write trial：在 disposable workspace 中完成，并通过
  `mcpToolCall` 创建了 `live-write.txt`，但没有 approval request。RoboCode 现在会把
  这类事件分类为 `mcp-tool-call`、`mcp-tool-completed` 和 `mcp-fs-write`；
  write-capable probe 继续默认禁用。
- Codex app-server write-guard smoke：通过；`/agent probe codex --turn-write`
  默认被拦截，只有在 disposable workspace 显式设置
  `ROBOCODE_EXPERIMENTAL_CODEX_APP_SERVER_WRITE=1` 时才允许实验。
- lane operator-loop smoke：通过；覆盖本地 runtime/operator 路径，从 shell lane
  到 inspect、PTY send、tmux evidence、accept/apply、conflict review/resolve、
  discard/cleanup 和 archive。
- release smoke quick matrix：通过；覆盖 formatting、terminal tests、TUI previews、
  fallback CLI smoke、protocol fixture、write guard 和 lane operator-loop smoke。
- DeepSeek release smoke matrix：通过，证据目录为
  `/tmp/robocode-018-release-smoke-deepseek-latest`；`deepseek-v4-flash` 返回了
  `robocode-deepseek-smoke-ok`。
- full 0.1.8 release smoke matrix：通过，证据目录为
  `/tmp/robocode-018-release-smoke-full`。覆盖 `robocode-cli` tests、workspace
  tests、previews、fallback CLI、app-server protocol fixture、write-guard、lane
  operator loop、package archive smoke 和 DeepSeek smoke。
- package smoke 已生成并验证
  `dist/robocode-v0.1.8-aarch64-apple-darwin.tar.gz`；解压后的 binary 输出
  `robocode-cli 0.1.8`。
- GitHub release workflow
  [26494175931](https://github.com/wikieden/robocode/actions/runs/26494175931)
  已通过，并上传了四个 target archive 和对应 SHA-256 文件。

## 已发布版本

`v0.1.8` 已发布：

- https://github.com/wikieden/robocode/releases/tag/v0.1.8

Release assets：

- `robocode-v0.1.8-aarch64-apple-darwin.tar.gz`
- `robocode-v0.1.8-aarch64-apple-darwin.tar.gz.sha256`
- `robocode-v0.1.8-x86_64-apple-darwin.tar.gz`
- `robocode-v0.1.8-x86_64-apple-darwin.tar.gz.sha256`
- `robocode-v0.1.8-x86_64-pc-windows-msvc.tar.gz`
- `robocode-v0.1.8-x86_64-pc-windows-msvc.tar.gz.sha256`
- `robocode-v0.1.8-x86_64-unknown-linux-gnu.tar.gz`
- `robocode-v0.1.8-x86_64-unknown-linux-gnu.tar.gz.sha256`

## 剩余 P0

`0.1.8` release 无剩余 P0。

## 后续风险

- 主屏 operation center 仍需要更多真实运行样本验证，尤其是长 tool output 和
  多步 review session 时的摘要压缩。
- `AgentTask` 还没有持久化为独立 artifact；目前仍是 runtime projection。
- side-2 的 `TESTS / LSP`、`MCP / CONTEXT` 和 `EXTENSIONS` 面板仍保留各自的
  source-specific 视图；下一步需要把更多真实 diff/review 入口接到主编程闭环。
- 编程闭环已有一等 diff review 可见性，也有结构化失败测试恢复线索；仍需要在更长的
  真实 review session 里继续验证。
- lane operator loop 现在已有确定性的 focused smoke coverage，但 release sign-off
  前仍需要在真实 DeepSeek/Codex/Claude/tmux TUI 副屏流程里做一轮人工验证并截图。
- Codex app-server path 仍保持 opt-in。live text-turn 和 disposable write-turn
  probe 已通过，确定性 protocol-fixture coverage 也已经覆盖 command/file/approval/MCP/error
  evidence。live write probe 确认 workspace-write turn 可以通过 MCP tool call
  在没有 RoboCode approval request 的情况下修改文件，所以 write-capable app-server
  probe 必须继续默认禁用，只允许 disposable workspace 实验。

## 下一步

1. 继续让 app-server execution 保持 opt-in 且默认 read-only。在 MCP/file mutation
   能做到 mutation 前 mediation 之前，不要推广 write-capable app-server turn。
2. 继续收敛 side-1 / side-2 / right-rail 的状态词和颜色优先级。
3. 在真实 DeepSeek/Codex/Claude/tmux 流程里做一轮手工 TUI 验证并截图。确定性
   lane operator-loop smoke 已覆盖命令路径；剩余是跨真实副屏的视觉/runtime sign-off。
4. 把 0.1.8 的后续风险纳入下一版本计划。
