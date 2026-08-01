# Viden TUI 0.3.1 Client Boundary 认证

English version: [release-tui-0.3.1-status.md](release-tui-0.3.1-status.md)

当前分支已把 TUI `0.3.1` 认证为 CoreClient-only component candidate。本文件不声明
GitHub/Homebrew distribution release。

## Source 状态

- Branch：`codex/v3-tui-task12-certify`
- Worktree：`/Users/wiki/Documents/GitHub/viden/.worktrees/v3-tui-task12-certify`
- TUI 0.3.1 task base：`4fbe426cd0b1bff43ae94e1a87ad26f58632b8a1`
- certification evidence run 时的 HEAD：
  `4fbe426cd0b1bff43ae94e1a87ad26f58632b8a1`
- Final commit 规则：tracked 文件不能自引用所属 commit；创建 commit 后，由 handoff
  报告最终 SHA。
- Push、merge、tag、GitHub Release、Homebrew 与发布状态：未执行，且本认证任务未授权。

## Core 与 Fixture 证据

- 已评审 Core checkpoint：`a927e2f31d2cb9bb6015c30bc0ed0976e958c77e`
- Frontend schema：`1`
- 冻结 contract payload：`5bd2b80b0953f4194d082940a7b9164c7231ca2d`
- Capability inventory：`15` 个冻结 base capability 加 `10` 个独立协商的 feature
  extension capability；两组无交集。
- Extension fixture SHA-256：
  `96dd5fde9f1241eb50f9d8978cf478d0ac5d3327448dc6ccde9d0e5018ce1580`
- 九个 base fixture corpus aggregate SHA-256：
  `e272d7bee25af5d4a0e719aa7226f1b5bf22086e90f0d02224196c41ce67fcab`
- Base fixture 文件清单、各文件 digest 与 corpus aggregate 都固定在
  `apps/tui/release-manifest.toml`；任何 fixture 漂移都会令 regression 失败。
- Regression 会把 token/catalog SHA-256 与 `apps/tui/release-manifest.toml`
  逐项核对。
- TUI replay test 会把九个 frozen base fixture 与 extension fixture 的 canonical
  projected-view hash 对照独立 Core fixture oracle；extension replay 还会断言 exact
  lane owner。

结构化证据：

- `target/tui-regression/0.3.1/tui-0.3.1-certification.json`
- `target/tui-regression/0.3.1/shared-fixture-digests.sha256`
- `target/tui-regression/0.3.1/tui-boundary-report.txt`
- `target/tui-regression/0.3.1/tui-regression-evidence.json`
- `target/tui-stability/0.3.1/summary.md`

记录 run 的 certification JSON SHA-256 是
`8cadb425a118f8fce29bafaba9af8972e7967967376827d42bde69eb1f9dbae1`。
生成证据只在 `target/`；已接受的设计 HTML、TUI component/cockpit reference shot
只读，digest 已记录在 JSON 中。

## 已认证 Behavior 与 Presentation

结构化报告记录 `22` 个 exact passing test，并覆盖：

- stream、tool、approval 期间 composer 仍可编辑；
- Normal、Insert、Overlay ownership；
- bracketed paste 不发送、grapheme/CJK cursor geometry、可操作 approval，以及绑定
  exact live lane owner 的 cancel；
- `80`、`112`、`160` 列 deterministic render model；
- `en` 与 `zh-CN` catalog/projection parity；
- 八个已登记 palette × truecolor、ANSI 256、ANSI 16（共 `24` 组合）、三种 density
  与 reduced motion；
- Settings Apply/Reset 等待匹配 Core receipt；
- invalid appearance 会 atomic fallback 到 Aurora dark/regular；
- TUI 无 authoritative effect、runtime-internal dependency 或 private preference
  persistence。

## 验证

- PASS `cargo fmt --all --check`
- PASS `cargo test -p viden-tui --quiet`（`260` unit tests 加 `1` API test）
- PASS `cargo test -p viden-cli --quiet`（`34` unit tests；integration tests `3`
  passed、`2` live tests ignored）
- PASS `scripts/tui-turn-controller-smoke.sh`；`28` 个命名 filter 都必须严格匹配
  一个 passing test。
- PASS `scripts/rc-tui-stability-smoke.sh target/tui-stability/0.3.1`
- PASS `scripts/tui-regression.sh target/tui-regression/0.3.1`
- PASS sandbox 外 `cargo test --workspace --quiet`。首次 sandbox 内运行到
  `viden-plugin-host` 时，仅两个 process-reaping test 因 `ps` 返回
  `Operation not permitted` 失败；按要求提权重跑后 exit `0`，没有改代码绕过。
- PASS `cargo clippy --workspace --all-targets -- -D warnings`；先移除了一个仅在
  测试里的 `Copy` preference receipt 多余 `clone`。
- PASS `scripts/check-doc-pairs.sh` 与 `scripts/check-doc-links.sh`，显式检查六个已变更的
  双语 user/stability/status 文档。
- PASS `git diff --check`

## 人工证据、Contract Gap 与风险

- 未提供真实 macOS Terminal 与 iTerm2 截图。Deterministic preview 不能替代这项
  manual release evidence。
- `mouse_capture` 保持 `false`。已接受设计允许显式可选 mouse mode，但 TUI 0.3.1
  尚无 `mouse_capture=true` preference 或协商 contract path。所有已认证 action 都有
  keyboard path。
- Schema 1 没有 persisted color-depth field。Color depth 明确是 local、session-only
  terminal preview，不创建私有 TUI store。
- Core 只暴露 lane-advertised session id，不提供 global session enumeration。
- Trusted frontend secret ingress 仍不可用；Core 没有安全 ingress 时，provider detail
  只显示 handle 且保持只读。
- 未运行 live-provider、billable、publish、release、tag、push、merge 或 Homebrew gate，
  因为本任务没有授权。
