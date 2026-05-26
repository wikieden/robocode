# RoboCode 0.1.7 发布状态

英文版： [release-0.1.7-status.md](release-0.1.7-status.md)

最后更新：2026-05-26

## 目标

`0.1.7` 是 Codex Adapter 和 Agent Orchestration Backbone 版本。目标是让
RoboCode 不再只是 terminal launcher，而是一个本地 host cockpit：Codex 成为第一
个 protocol-aware delegate agent，主 TUI 显示实时工作状态，副屏展示 lane、
extension、MCP 和 evidence 状态。

## 阶段映射

1. Live operation center：本地实现已落地。
2. Codex job runtime：CLI jobs 和实验 app-server jobs 的本地实现已落地。
3. Cockpit 中的 agent evidence：本地实现已落地。
4. Extension 和 MCP diagnostics：本地实现已落地。
5. ACP 方向：protocol handshake/probe 仍为实验能力，并已有文档记录。
6. Release packaging 和外部发布：等待最终 smoke 与 release workflow。

## Candidate 证据

- workspace package version 已从 `0.1.6` 升到 `0.1.7`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.7`。
- README 安装示例和 release workflow 默认 tag 已改为 `v0.1.7`。
- 0.1.7 计划已经作为当前下一迭代核心：Host-Delegate Agent Bridge、Codex
  Adapter、live operation center、extension diagnostics 和 ACP adapter spike。
- `/agent doctor codex` 会检查 command、version、app-server support、auth
  status、config sources 和 job-store path。
- `/agent review codex`、`/agent challenge codex` 和
  `/agent run codex [--write] <task>` 会在 `.robocode/agents/` 下创建 tracked
  Codex job records 和 artifacts。
- `/agent status`、`/agent result <id>` 和 `/agent cancel <id>` 会展示 tracked
  Codex job lifecycle。
- TUI `OPERATION CENTER` 固定在 transcript 顶部，并为 provider turn、approval、
  lane、tool call 和 Codex job 标出 evidence source。
- TUI Codex job snapshot 会从 app-server result/log 中提取 thread ID、turn ID、
  turn status 和 approval requests。
- `/extensions doctor` 和 `/mcp doctor` 会按 surface 输出 readiness，包括 provider
  plugin dirs、MCP config files、skill roots 和 permission boundary 提醒。
- 稳定 subprocess-backed Codex、ACP 和 lane tests 后，默认
  `cargo test --workspace --quiet` 已通过。
- 带 DeepSeek 真实 provider validation 的完整本地 release smoke 已通过：
  `scripts/release-smoke.sh --version 0.1.7 --deepseek --out-dir /tmp/robocode-017-release-smoke-deepseek-local-2`。
- Evidence 目录：
  `/tmp/robocode-017-release-smoke-deepseek-local-2`。
- smoke matrix 已通过 `cargo-fmt`、`robocode-cli-tests`、
  `workspace-tests`、`tui-previews`、`fallback-cli-smoke`、
  `shell-lane-smoke`、`tmux-lane-smoke`、`package-smoke` 和
  `deepseek-cli-smoke`。
- DeepSeek V4 Flash live smoke 已通过；transcript 中包含
  `robocode-deepseek-smoke-ok`。
- `aarch64-apple-darwin` host package smoke 已通过；解压后的二进制输出
  `robocode-cli 0.1.7`。
- macOS arm64 archive SHA-256：
  `c9a17d5d4d3d36824616505a3abde659a6db173fffa21c22b3f60b83d988d1a2`。

## 验证门禁

发布前计划门禁：

- `cargo fmt --check`
- `git diff --check`
- `cargo test --workspace --quiet`
- `scripts/release-smoke.sh --version 0.1.7 --deepseek`
- `upload_to_release=false` 的 GitHub Actions release artifact validation
- `upload_to_release=true` 的最终 GitHub release artifact upload
- Homebrew tap update 和 fetch smoke

## 当前发现

### P0

- 本地源码验证和本地 release smoke 暂无已知 P0。
- 外部 release、GitHub artifacts 和 Homebrew tap validation 待执行。

### P1

- app-server task path 仍是实验能力；在 live smoke 证明普通 jobs 可以安全默认走
  protocol path 前，应保持 opt-in。
- 完整 ACP editing 仍是后续工作；0.1.7 保留 protocol boundary 和 evidence model。

### P2

- 自动任务拆分继续后置。
- 完整 cursor-addressed terminal replay 继续后置。
- 更多外部 coding-agent templates 继续按真实需求推进。

## Release 结果

待发布。
