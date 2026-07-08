# Viden 0.1.27 状态 - 日常编程闭环加固

English version: [release-0.1.27-status.md](release-0.1.27-status.md)

`0.1.27` 已完成。本版本在下一轮 delegated lane 清理前，先收口日常编程
闭环稳定性。

## 状态

- Workspace version：`0.1.27`
- Git tag：`v0.1.27`
- GitHub Release：已发布：
  <https://github.com/wikieden/viden/releases/tag/v0.1.27>
- Homebrew tap：已同步到 `wikieden/homebrew-tap` commit `eccca4c`

## 当前已实现

- 增加完整 TUI controller 回归：
  `prompt -> streaming -> approval -> write_file -> queued follow-up -> final`。
- 增加 test-only terminal guard，不打开 alternate screen 也能验证 TUI event
  loop 行为。
- approval 弹窗不再吞掉普通 composer 输入；approval 快捷键仍然能确认或拒绝。
- 底部状态栏现在展示真实 work mode 和 permission level。
- 顶栏状态使用真实活动标签，不再显示静态 `auto` 文案。
- `scripts/tui-turn-controller-smoke.sh` 覆盖 mode/permission 同步、approval
  输入、active-turn queue，以及完整 coding-loop controller 路径。

## 验证

- `cargo test -p viden-cli provider_turn_streams_approves_tools_runs_queued_followup_and_releases_composer -- --nocapture`：通过
- `cargo test -p viden-cli active_approval_does_not_swallow_composer_typing -- --nocapture`：通过
- `cargo test -p viden-cli tui::statusbar::tests::bottom_bar_reflects_runtime_mode_and_permission_level -- --nocapture`：通过
- `cargo test -p viden-cli tui::topbar::tests::top_bar_status_reflects_active_turn_instead_of_static_auto_text -- --nocapture`：通过
- `scripts/tui-turn-controller-smoke.sh`：通过
- `cargo fmt --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo test --workspace --quiet -- --test-threads=1`：提权后通过；sandbox
  内本地 HTTP test server 被 `Operation not permitted` 阻止。
- `scripts/release-gate.sh --version 0.1.27 --phase prepublish --out-dir /tmp/viden-0127-release-gate`：通过
- `scripts/release-gate.sh --version 0.1.27 --phase postpublish --out-dir /tmp/viden-0127-release-gate`：通过

## 发布 Gate

`0.1.27` 已完成：

- prepublish gate 已通过，证据在 `/tmp/viden-0127-release-gate/prepublish`；
- GitHub Release `v0.1.27` 已发布，包含 `8` 个 assets；
- Homebrew tap 已同步到 `0.1.27`，commit `eccca4c`；
- postpublish gate 已通过，证据在 `/tmp/viden-0127-release-gate/postpublish`。

## DeepSeek Smoke 证据

- Provider/model：`deepseek / deepseek-v4-flash`
- 场景：`python_add_module_with_test`
- 请求：`3` ok / `0` err
- Tokens：input `11542`，output `590`，total `12132`
- 耗时：`8` 秒
- 预估费用：`¥0.012722 CNY`
- 计费依据：DeepSeek cache-miss estimate，input `¥1/1M`，output `¥2/1M`
- 失败分类：未观察到失败；live smoke 在本次 prepublish gate 首次通过。
- 证据：`/tmp/viden-0127-release-gate/prepublish/deepseek-dev-scenario/summary.md`
