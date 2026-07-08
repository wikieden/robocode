# Viden 0.1.9 状态

英文版： [release-0.1.9-status.md](release-0.1.9-status.md)

最后更新：2026-05-27

## 摘要

`0.1.9` 是 Verification Hardening + Screenshot-Gated UX 版本。版本目标见
[release-0.1.9-plan.zh-CN.md](release-0.1.9-plan.zh-CN.md)。

workspace package version 已 bump 到 `0.1.9`。本地 release-candidate 验证已通过，
包括 clippy 门禁、完整 workspace tests、TUI regression 截图、package smoke 和
DeepSeek live smoke。GitHub release、多平台资产和 Homebrew tap 也已发布并验证。

## 主要变化

- `scripts/release-smoke.sh` 成为标准 release gate，并写入结构化
  `release-evidence.json`。
- release gate 增加 `cargo clippy --workspace --all-targets -- -D warnings`。
- 发布后检查通过 opt-in 的 `--github-release-assets` 和 `--homebrew` 参数暴露。
- GitHub release metadata validation 对偶发 API fetch failure 增加重试，避免瞬时网络
  EOF 直接打断发布后检查。
- `scripts/tui-regression.sh` 导出可供产品侧确认的确定性截图产物。
- TUI preview evidence 已刷新到 `0.1.9`，README 系统截图也改为生成出来的主 cockpit SVG。
- CI PR/main 检查改为使用 quick release gate，不再是分散的 ad-hoc build/test。
- 清理 shared crates、model parsing、core runtime、config、session、LSP 和 TUI 代码中的
  既有 clippy warnings，让 clippy 可以作为 release blocker。

## 截图证据

已持久化的视觉证据：

- [主 cockpit](previews/generated/screenshots/0.1.9-tui-main.svg)
- [idle cockpit](previews/generated/screenshots/0.1.9-tui-main-idle.svg)
- [命令面板](previews/generated/screenshots/0.1.9-tui-command-palette.svg)
- [lane detail](previews/generated/screenshots/0.1.9-tui-lane-detail.svg)
- [side-1 lane screen](previews/generated/screenshots/0.1.9-tui-side-1.svg)
- [side-2 ops screen](previews/generated/screenshots/0.1.9-tui-side-2.svg)

结构化截图证据：

```text
docs/previews/generated/tui-regression-evidence.json
```

## 本地 Release Candidate 证据

完整本地 release smoke：

```bash
scripts/release-smoke.sh --version 0.1.9 --deepseek --out-dir /tmp/viden-019-release-smoke-full
```

结果：

- passed: 11
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-019-release-smoke-full/release-evidence.json`

通过的检查：

- `cargo-fmt`
- `cargo-clippy`
- `viden-cli-tests`
- `workspace-tests`
- `tui-regression`
- `fallback-cli-smoke`
- `codex-app-server-protocol-fixture`
- `codex-app-server-write-guard`
- `lane-operator-loop-smoke`
- `package-smoke`
- `deepseek-cli-smoke`

Package smoke 生成：

```text
dist/viden-v0.1.9-aarch64-apple-darwin.tar.gz
```

解压后的 binary 输出：

```text
viden-cli 0.1.9
```

## 发布状态

已发布并验证。

- GitHub release:
  [v0.1.9](https://github.com/wikieden/viden/releases/tag/v0.1.9)
- Release workflow:
  [run 26496859443](https://github.com/wikieden/viden/actions/runs/26496859443)
- Workflow status: `completed` / `success`
- Release published at: `2026-05-27T07:19:05Z`
- Main release commit: `99b3957`
- Homebrew tap commit: `6796da2`

已上传 release assets：

- `viden-v0.1.9-aarch64-apple-darwin.tar.gz`
- `viden-v0.1.9-aarch64-apple-darwin.tar.gz.sha256`
- `viden-v0.1.9-x86_64-apple-darwin.tar.gz`
- `viden-v0.1.9-x86_64-apple-darwin.tar.gz.sha256`
- `viden-v0.1.9-x86_64-pc-windows-msvc.tar.gz`
- `viden-v0.1.9-x86_64-pc-windows-msvc.tar.gz.sha256`
- `viden-v0.1.9-x86_64-unknown-linux-gnu.tar.gz`
- `viden-v0.1.9-x86_64-unknown-linux-gnu.tar.gz.sha256`

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.9 --quick --github-release-assets --homebrew --out-dir /tmp/viden-019-postpublish-check
```

结果：

- passed: 10
- failed: 0
- skipped: 3
- evidence: `/tmp/viden-019-postpublish-check/release-evidence.json`
- Homebrew formula: `wikieden/tap/viden 0.1.9`

## 剩余风险

- `0.1.9` 没有剩余 release-blocking 风险。
- 截图门禁当前使用确定性 preview SVG。输入法位置、真实光标闪烁和鼠标行为仍需要在具体 UI
  功能中补真实终端截图。
- Codex app-server write-capable turns 继续保持 experimental guard。
