# Viden 0.1.15 状态

English version: [release-0.1.15-status.md](release-0.1.15-status.md)

最后更新：2026-05-29

## 状态

`0.1.15` 已实现并完成本地验证，定位为 **Context Curator And Budget
Controls** release candidate。Workspace version 已提升到 `0.1.15`，
ContextBundle v1 records 已在 provider turn 和 lane envelope 中可见，`/context`
命令可用，确定性 TUI 截图也已刷新。

发布仍未完成：当前工作区还没有创建 `v0.1.15` tag、GitHub Release、多平台
release assets 或 Homebrew tap update。

## 已落地

- 共享 `ContextBundleRecord` 现在记录 policy、source priority、include-reason、
  omitted sources 和 omission reason。
- Main provider turns 和 lane envelopes 共用 `v1-priority-budget` policy。预算压力上升时，task/workspace/test 这类高优先级上下文会优先保留，低优先级 summaries 先被裁剪。
- `/context` 展示最近一次 provider ContextBundle，包括 source ordering、token
  estimates、omitted sources 和 compaction notes。
- `AgentTask` evidence 现在携带 context policy 和 omitted-source count，所以
  side-2 ops 可以从 shared runtime task snapshot 显示 `BUNDLE`、`POLICY` 和
  `OMIT` 行。
- Command palette 已加入 `/context`，用户指南也说明了如何检查 provider-side
  context 决策。

## 验证证据

- `cargo fmt --check`
- `cargo test -p viden-types --quiet`
- `cargo test -p viden-core context --quiet`
- `cargo test -p viden-cli tui::preview --quiet`
- `cargo test -p viden-cli tui::ops_screen --quiet`
- `cargo test -p viden-cli tui::command_palette --quiet`
- `cargo test -p viden-cli tui::lane::tests::lane_envelope_includes_context_bundle_sources_and_pressure --quiet`
- `cargo clippy -p viden-types -p viden-core -p viden-cli --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `VIDEN_TUI_SCREENSHOT_VERSION=0.1.15 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.15 --quick --out-dir /tmp/viden-0115-release-smoke-local`

本地 quick release smoke 结果：

- PASS `cargo-fmt`
- PASS `cargo-clippy`
- PASS `viden-cli-terminal-tests`
- PASS `tui-regression`
- PASS `fallback-cli-smoke`
- PASS `codex-app-server-protocol-fixture`
- PASS `codex-app-server-write-guard`
- PASS `lane-operator-loop-smoke`
- SKIP `package-smoke`（quick mode）
- SKIP `deepseek-cli-smoke`（需要 opt-in live provider check）
- SKIP `github-actions-release-validation`（发布后检查）
- SKIP `github-release-assets-validation`（发布后检查）
- SKIP `homebrew-validation`（发布后检查）

确定性截图：

- [0.1.15 main](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-main.svg)
- [0.1.15 command palette](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-command-palette.svg)
- [0.1.15 lane detail](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-lane-detail.svg)
- [0.1.15 side-1 lanes](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-side-1.svg)
- [0.1.15 side-2 ops](/Users/wiki/Documents/GitHub/viden/docs/previews/generated/screenshots/0.1.15-tui-side-2.svg)

## 剩余风险

- `0.1.15` 仍需要发布闭环：tag、GitHub Actions release assets、Homebrew formula
  update 和 post-publish smoke。
- Pin/omit commands 与 per-lane budget overrides 仍是 P1 设计；当前版本只落地
  policy 与 evidence foundation。
- Main provider prompt assembly 仍使用 compact ContextBundle injection path。
  完整 budget-aware prompt composition 放到后续 roadmap。
