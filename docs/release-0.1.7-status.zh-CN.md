# Viden 0.1.7 发布状态

英文版： [release-0.1.7-status.md](release-0.1.7-status.md)

最后更新：2026-05-26

## 目标

`0.1.7` 是 Codex Adapter 和 Agent Orchestration Backbone 版本。目标是让
Viden 不再只是 terminal launcher，而是一个本地 host cockpit：Codex 成为第一
个 protocol-aware delegate agent，主 TUI 显示实时工作状态，副屏展示 lane、
extension、MCP 和 evidence 状态。

## 阶段映射

1. Live operation center：本地实现已落地。
2. Codex job runtime：CLI jobs 和实验 app-server jobs 的本地实现已落地。
3. Cockpit 中的 agent evidence：本地实现已落地。
4. Extension 和 MCP diagnostics：本地实现已落地。
5. ACP 方向：protocol handshake/probe 仍为实验能力，并已有文档记录。
6. Release packaging 和外部发布：GitHub release 和 Homebrew tap validation 已通过。

## Candidate 证据

- workspace package version 已从 `0.1.6` 升到 `0.1.7`。
- `Cargo.lock` 中的 workspace package entries 已解析到 `0.1.7`。
- README 安装示例和 release workflow 默认 tag 已改为 `v0.1.7`。
- 0.1.7 计划是本次 release 的核心：Host-Delegate Agent Bridge、Codex
  Adapter、live operation center、extension diagnostics 和 ACP adapter spike。
  当前后续计划已转为 `docs/release-0.1.8-plan.zh-CN.md`。
- `/agent doctor codex` 会检查 command、version、app-server support、auth
  status、config sources 和 job-store path。
- `/agent review codex`、`/agent challenge codex` 和
  `/agent run codex [--write] <task>` 会在 `.viden/agents/` 下创建 tracked
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
  `scripts/release-smoke.sh --version 0.1.7 --deepseek --out-dir /tmp/viden-017-release-smoke-deepseek-local-2`。
- Evidence 目录：
  `/tmp/viden-017-release-smoke-deepseek-local-2`。
- smoke matrix 已通过 `cargo-fmt`、`viden-cli-tests`、
  `workspace-tests`、`tui-previews`、`fallback-cli-smoke`、
  `shell-lane-smoke`、`tmux-lane-smoke`、`package-smoke` 和
  `deepseek-cli-smoke`。
- DeepSeek V4 Flash live smoke 已通过；transcript 中包含
  `viden-deepseek-smoke-ok`。
- `aarch64-apple-darwin` host package smoke 已通过；解压后的二进制输出
  `viden-cli 0.1.7`。
- macOS arm64 archive SHA-256：
  `c9a17d5d4d3d36824616505a3abde659a6db173fffa21c22b3f60b83d988d1a2`。
- GitHub Actions release artifact validation 已以 `upload_to_release=false`
  跑通全部配置目标：`aarch64-apple-darwin`、`x86_64-apple-darwin`、
  `x86_64-unknown-linux-gnu` 和 `x86_64-pc-windows-msvc`。
  Run: https://github.com/wikieden/viden/actions/runs/26449257109。
- 最终 GitHub release workflow 已以 `upload_to_release=true` 通过，并上传全部
  配置 artifacts。
  Run: https://github.com/wikieden/viden/actions/runs/26449437778。
- Homebrew tap `wikieden/homebrew-tap` 中的 Viden formula URL 和 SHA-256
  已指向 `v0.1.7`。
  Commit: https://github.com/wikieden/homebrew-tap/commit/8e84a89。
- Homebrew fetch smoke 已通过：
  `brew fetch --force wikieden/tap/viden` 输出
  `Formula viden (0.1.7)`。

## 验证门禁

`0.1.7` 计划内验证门禁已全部通过：

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
- `0.1.7` release 无剩余 P0。

### P1

- app-server task path 仍是实验能力；在 live smoke 证明普通 jobs 可以安全默认走
  protocol path 前，应保持 opt-in。
- 完整 ACP editing 仍是后续工作；0.1.7 保留 protocol boundary 和 evidence model。

### P2

- 自动任务拆分继续后置。
- 完整 cursor-addressed terminal replay 继续后置。
- 更多外部 coding-agent templates 继续按真实需求推进。

## Release 结果

`v0.1.7` 已发布：

- https://github.com/wikieden/viden/releases/tag/v0.1.7

release 包含：

- `viden-v0.1.7-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.7-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.7-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.7-x86_64-pc-windows-msvc.tar.gz`
- 每个 archive 对应的 `.sha256` 文件。

GitHub 返回的 asset digests：

- `aarch64-apple-darwin`：
  `sha256:ec000b139ede57d27035e9ba2ed95f111e3f6d0e40fe2c2c648b63d6fbf7a2a9`
- `x86_64-apple-darwin`：
  `sha256:a50ceac337ffad807bb4ae6935ff5177a25f36e098eee10a91e0c1b9ce3b86bc`
- `x86_64-unknown-linux-gnu`：
  `sha256:758a38f4ef1a217e02b77647aa2ee22e049ce7bd214c55dbd8cd4e9b606065ae`
- `x86_64-pc-windows-msvc`：
  `sha256:f2c8e9d2247dd61cc81bd21d21ea48f92120d683e3e4bbfade9bb534e365b581`

Homebrew 安装路径：

```bash
brew tap wikieden/tap
brew install viden
```
