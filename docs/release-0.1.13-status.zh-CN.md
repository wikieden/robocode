# RoboCode 0.1.13 状态

English version: [release-0.1.13-status.md](release-0.1.13-status.md)

最后更新：2026-05-28

## 状态

`0.1.13` 已完成 **Operator Loop Hardening** 的实现、打 tag、发布和发布后验证。
GitHub Releases 已包含多平台资产，`wikieden/homebrew-tap` formula 也已指向
`v0.1.13` artifacts。

发布信息：

- GitHub Release: [v0.1.13](https://github.com/wikieden/robocode/releases/tag/v0.1.13)
- Release workflow: [26526332898](https://github.com/wikieden/robocode/actions/runs/26526332898)
- Homebrew tap commit: `b8f9c1d`

## 已落地

- 默认 `robocode` 入口现在打开 TUI cockpit；`--no-tui` 保留给脚本和 smoke
  tests 使用的 legacy line REPL。
- `/settings` 和 `/setup` 支持 provider/model 发现、API key 状态展示，以及显式
  保存 provider/model 默认值，不保存 secret。
- Slash palette 和 approval 测试已保护 `/quit`、`/exit`、默认 approval、focus
  移动、直接快捷键，以及中文输入/resize preview 稳定性。
- Lane review 增加聚焦证据命令：
  - `/lane diff <id>` 写入并展示 `L*.diff.patch`。
  - `/lane artifacts <id>` 列出持久化 lane artifacts。
- 主 provider turn 会构造 ContextBundle，并作为临时 request context 放在最终
  user message 前面，保留 raw transcript audit 数据，也不破坏 provider 兼容。
- `/status`、runtime task evidence 和 side-2 context 行展示 context pressure、
  source count、largest sources 和 compaction notes。
- 非交互 smoke scripts 已显式传入 `--no-tui`，默认 TUI 不再破坏 release
  automation。

## 验证证据

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `scripts/smoke-lane-operator-loop.sh`
- `ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.13 scripts/tui-regression.sh docs/previews/generated`
- `scripts/release-smoke.sh --version 0.1.13 --out-dir /tmp/robocode-0113-release-smoke-full-local`
- `scripts/release-smoke.sh --version 0.1.13 --quick --github-release-assets --out-dir /tmp/robocode-0113-github-release-check`
- `scripts/release-smoke.sh --version 0.1.13 --quick --github-release-assets --homebrew --out-dir /tmp/robocode-0113-postpublish-check`
- `scripts/release-smoke.sh --version 0.1.13 --quick --deepseek --out-dir /tmp/robocode-0113-deepseek-live-check`

完整本地 release smoke 结果：

- PASS `cargo-fmt`
- PASS `cargo-clippy`
- PASS `robocode-cli-tests`
- PASS `workspace-tests`
- PASS `tui-regression`
- PASS `fallback-cli-smoke`
- PASS `codex-app-server-protocol-fixture`
- PASS `codex-app-server-write-guard`
- PASS `lane-operator-loop-smoke`
- PASS `package-smoke`
- SKIP `deepseek-cli-smoke`（需要 opt-in live provider 验证）
- SKIP `github-actions-release-validation`（发布后验证）
- SKIP `github-release-assets-validation`（发布后验证）
- SKIP `homebrew-validation`（发布后验证）

发布后 smoke 结果：

- PASS `github-release-assets-validation`
- PASS `homebrew-validation`
- PASS `deepseek-cli-smoke`

已发布资产：

- `robocode-v0.1.13-aarch64-apple-darwin.tar.gz`
- `robocode-v0.1.13-x86_64-apple-darwin.tar.gz`
- `robocode-v0.1.13-x86_64-unknown-linux-gnu.tar.gz`
- `robocode-v0.1.13-x86_64-pc-windows-msvc.tar.gz`
- 四个平台 archive 对应的 `.sha256` 文件

确定性截图：

- [0.1.13 main](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.13-tui-main.svg)
- [0.1.13 command palette](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.13-tui-command-palette.svg)
- [0.1.13 side-2 ops](/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.13-tui-side-2.svg)

## 后续

- `0.1.13` 已无发布 blocker。
- Windows 和 Linux assets 由 GitHub Actions 产出；本地 post-publish smoke
  验证 asset 存在性，以及当前 macOS host 上的 Homebrew 安装路径。
