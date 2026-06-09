# RoboCode 0.1.25 状态 - TUI 显示清理与 Idle 稳定性

英文版： [release-0.1.25-status.md](release-0.1.25-status.md)

`0.1.25` 是 TUI 显示清理版本。它把 `0.1.24` 的非阻塞 operator loop 继续稳住，
并修复长时间 idle 后重绘漂移、terminal focus/paste 后不重绘、composer 协议残片、
welcome/modal 清屏不完整等问题。

## 发布状态

- Workspace version: `0.1.25`
- Git tag: pending `v0.1.25`
- Release commit: pending
- GitHub release: pending
- Release workflow: pending
- Homebrew tap commit: pending
- Prepublish evidence: `/tmp/robocode-0125-release-gate/prepublish`
- Local package: `dist/robocode-v0.1.25-aarch64-apple-darwin.tar.gz`
- Local package sha256:
  `19a7423158910cb9fbab8823bf45122613593057c2e4bb6628c416e94b23a5e6`
- Post-publish evidence: pending
- Distribution state: pending GitHub Release assets and Homebrew validation

## 已包含变更

- Terminal drawing 现在会周期性强制 full redraw，避免 dirty-row cache 在长时间 idle、
  focus 或 sleep/wake 后误以为 alternate-screen 内容仍完整存在。
- focus 和 paste 事件会触发 TUI repaint，但不会进入 composer input。
- composer 会丢弃以 `m` 或 `M` 结尾的 terminal SGR residue，覆盖
  `2;28;95;132m` 这类常见 mouse/color protocol 尾巴。
- welcome-screen interaction modal 会按全屏清理，因为 welcome layout 没有 right rail。
- `/connect` provider selector preview 不再漏出底层 `commands /connect` hint 文本。
- TUI zero-bug gate 文档记录了 long-idle/focus/paste regression 和必须保留的 guardrails。
- `0.1.25` plan/spec/status 文档已版本化，并纳入 TDD release contract smoke。

## 验证

实现过程中已经使用的 focused checks：

```bash
cargo fmt --check
cargo test -p robocode-cli tui::app::tests -- --nocapture
cargo test -p robocode-cli tui::render::tests -- --nocapture
cargo test -p robocode-cli tui::terminal::tests -- --nocapture
cargo test -p robocode-cli --quiet
cargo clippy -p robocode-cli --all-targets -- -D warnings
scripts/tui-regression.sh /tmp/robocode-tui-regression-0125-idle-fix
```

Release gate：

```bash
scripts/release-gate.sh --version 0.1.25
```

结果：2026-06-09 prepublish 通过。证据：
`/tmp/robocode-0125-release-gate`。

Prepublish smoke result：

- `cargo-fmt`: passed
- `tdd-testing-contract-smoke`: passed
- `tui-turn-controller-smoke`: passed
- `cargo-clippy`: passed
- `robocode-cli-tests`: passed
- `workspace-tests`: passed
- `tui-regression`: passed
- `fallback-cli-smoke`: passed
- `plan-mode-smoke`: passed
- `daily-loop-smoke`: passed
- `codex-app-server-protocol-fixture`: passed
- `codex-app-server-write-guard`: passed
- `lane-operator-loop-smoke`: passed
- `package-smoke`: passed
- `deepseek-dev-scenario-smoke`: passed

Post-publish verification：

```bash
scripts/release-gate.sh --version 0.1.25 --phase postpublish
```

结果：pending。

## DeepSeek 真实开发场景

- Provider/model: `deepseek` / `deepseek-v4-flash`
- Scenario: `python_add_module_with_test`
- Requests: `3` ok，`0` errors
- Tokens: input `11269`，output `455`，total `11724`
- Estimated cost: `¥0.012179 CNY`
- Evidence: `/tmp/robocode-0125-release-gate/prepublish/deepseek-dev-scenario`

## 截图证据

已执行：

```bash
scripts/tui-regression.sh docs/previews/generated
```

结果：通过。结构化证据：
`docs/previews/generated/tui-regression-evidence.json`。

确定性截图集合：

- `docs/previews/generated/screenshots/0.1.25-tui-main.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-main-idle.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-live-turn.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-main-resize.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-cjk-input.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-command-palette.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-setup-wizard.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-provider-selector.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-provider-detail.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-model-selector.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-lane-selector.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-lane-detail.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-side-1.svg`
- `docs/previews/generated/screenshots/0.1.25-tui-side-2.svg`

## 剩余风险

- 真实 terminal focus/sleep 行为会因 emulator 不同而不同。自动 guard 已覆盖 redraw
  policy 和 deterministic preview；最终 0.1.x 仍应补 Terminal/iTerm2 人工验收证据。
- provider doctor/probe 和 durable active-turn queue ownership 仍是后续架构工作；
  它们不是本显示清理 patch 的新承诺。
