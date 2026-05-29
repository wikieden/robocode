# RoboCode 0.1.14 状态

English version: [release-0.1.14-status.md](release-0.1.14-status.md)

最后更新：2026-05-29

## 状态

`0.1.14` 已作为 **Delegated Agent Trust Loop** 本地 RC 完成实现和验证。
workspace version 已升到 `0.1.14`，文档和确定性 TUI 截图已刷新，本地测试为绿色。

发布动作仍未执行：当前工作区还没有创建 `v0.1.14` tag、GitHub Release、多平台
release assets 或 Homebrew tap 更新。

## 已完成

- 共享 adapter capability record 已接入 `/agent list` 和 `/agent doctor`。
  doctor 输出包含 Codex、Claude、自定义 template、tmux、PTY、ACP 的就绪状态、
  变更模式、证据模式、配置来源和已知限制。
- Lane timeline 与 isolation declaration 已持久化为证据。`/lane timeline <id>`
  显示有序事件流，`/lane inspect <id>` 会关联 envelope、log、done、timeline、
  terminal artifacts、changed files、verification、decision 和 next action。
- `/lane codex-review <task>` 是 P0 只读 Codex review 信任闭环。它会写入 lane
  envelope；Codex 可用时运行 `codex review --uncommitted`；支持
  `ROBOCODE_LANE_CODEX_REVIEW_TEMPLATE`；setup blocker 会写入 timeline evidence。
- Claude/tmux 信任闭环更安全。`/lane tmux <id>` 会在标记 attached 前检查默认
  tmux/Claude 路径；缺少 `tmux` 或 `claude` 时记录 setup-needed timeline 事件，
  不再误报 attached。
- 新增长期 roadmap 和 HN demand radar 文档，用于把当前 TUI 工作对齐到更长期的
  多 Agent 编排与 token 效能定位。

## 验证证据

- `cargo fmt --check`
- `cargo test -p robocode-types --quiet`
- `cargo test -p robocode-core agent_ --quiet`
- `cargo test -p robocode-cli tui::lane --quiet`
- `cargo test -p robocode-cli tui::command_palette --quiet`
- `cargo clippy -p robocode-cli -p robocode-core -p robocode-types --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.14 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.14 --quick --out-dir /tmp/robocode-0114-release-smoke-local`
- `git diff --check`

本地 quick release smoke 结果：

- PASS `cargo-fmt`
- PASS `cargo-clippy`
- PASS `robocode-cli-terminal-tests`
- PASS `tui-regression`
- PASS `fallback-cli-smoke`
- PASS `codex-app-server-protocol-fixture`
- PASS `codex-app-server-write-guard`
- PASS `lane-operator-loop-smoke`
- SKIP `package-smoke`（quick mode）
- SKIP `deepseek-cli-smoke`（需显式开启真实 provider 检查）
- SKIP `github-actions-release-validation`（发布后检查）
- SKIP `github-release-assets-validation`（发布后检查）
- SKIP `homebrew-validation`（发布后检查）

确定性截图：

- [0.1.14 main](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.14-tui-main.svg)
- [0.1.14 command palette](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.14-tui-command-palette.svg)
- [0.1.14 lane detail](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.14-tui-lane-detail.svg)
- [0.1.14 side-1 lanes](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.14-tui-side-1.svg)
- [0.1.14 side-2 ops](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.14-tui-side-2.svg)

## 剩余风险

- `0.1.14` 仍需发布闭环：tag、GitHub Actions release assets、Homebrew formula
  更新和 post-publish smoke。
- Codex/Claude happy path 当前有确定性 template 和 setup blocker 测试；公开发布前
  还应该补真实已认证 Codex/Claude terminal 运行证据。
- ACP 仍停留在 descriptor/probe 范围。mutating ACP/plugin/MCP/skill runtime 支持不在
  本版本范围内。
