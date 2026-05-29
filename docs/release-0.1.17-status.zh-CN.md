# RoboCode 0.1.17 状态 - 日常编码闭环基线

英文版： [release-0.1.17-status.md](release-0.1.17-status.md)

## 状态

`0.1.17` 是日常编码闭环基线的本地 RC。它把首次使用路径调整为
DeepSeek-first，fallback 只作为显式离线测试路径，并加入模型失败恢复提示和确定性
daily-loop smoke。同时，本版本也落地了 0.1.17 计划里的最小 task brief /
steering 支撑层。

- Workspace version：`0.1.17`
- 本地 package：`dist/robocode-v0.1.17-aarch64-apple-darwin.tar.gz`
- 本地 package sha256：
  `999edafa93e9c5863370a9857d1e96c430174572ab2b8b6f1e3c7106e7933ed1`

## 已落地范围

- 干净安装现在默认解析到 `deepseek` 作为在线 provider。
  `fallback / test-local` 保留为显式离线和 CI smoke 路径。
- `/setup` 现在会在 TUI flow 中展示交互式 provider/model 配置指南，包含
  DeepSeek 默认配置、fallback 配置、API-key 状态、provider choices 和 command
  palette 操作提示。
- `/setup provider <id> [model]` 和 `/setup model <model>` 复用 `/settings` 的
  provider/model 保存路径，因此用户可以配置 provider/model，但不会保存 API key。
- 当错误看起来像 model unavailable、unauthorized、unsupported 或 incompatible 时，
  provider/model failure 会附加换模型恢复提示。
- `/brief <goal>` 和 `/spec <goal>` 会在 `.robocode/briefs/active.md`
  创建 active task brief；`/brief show` 展示，`/brief clear` 清理。
- `/brief steering init` 会在 `.robocode/steering/` 下创建最小 project
  steering 模板；`/brief steering show` 展示摘要。
- Provider ContextBundle 和 lane envelope 现在会在存在 active brief / steering
  时引用它们；side-2 ops 也会显示 active brief id/title。
- `scripts/daily-loop-smoke.sh` 会跑确定性的 edit -> approval -> test -> diff ->
  status 闭环，并带上 active brief / steering evidence，写入 transcript、diff、
  TUI ANSI preview 和 summary evidence。
- `scripts/release-smoke.sh` 已包含 `daily-loop-smoke` step。
- README 和截图引用已刷新到 `0.1.17` daily-loop RC。

## 验证

2026-05-29 本地通过：

```bash
cargo fmt --check
git diff --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p robocode-config --quiet
cargo test -p robocode-model --quiet
cargo test -p robocode-core --quiet -- --test-threads=1
cargo test -p robocode-cli --quiet -- --test-threads=1
cargo test --workspace --quiet -- --test-threads=1
ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.17 \
  ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
  ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
  scripts/tui-regression.sh docs/previews/generated
scripts/daily-loop-smoke.sh /tmp/robocode-0117-daily-loop-smoke
scripts/daily-loop-smoke.sh /tmp/robocode-0117-daily-loop-smoke-brief
scripts/release-smoke.sh --version 0.1.17 --quick \
  --out-dir /tmp/robocode-0117-release-smoke-local
scripts/release-smoke.sh --version 0.1.17 --quick \
  --out-dir /tmp/robocode-0117-release-smoke-local-brief
scripts/release-smoke.sh --version 0.1.17 --skip-package \
  --out-dir /tmp/robocode-0117-release-smoke-full-nopackage
scripts/release-smoke.sh --version 0.1.17 --skip-package \
  --out-dir /tmp/robocode-0117-release-smoke-full-nopackage-brief
scripts/package-release.sh 0.1.17 aarch64-apple-darwin
cd dist && shasum -a 256 -c robocode-v0.1.17-aarch64-apple-darwin.tar.gz.sha256
```

Evidence 目录：

```text
/tmp/robocode-0117-daily-loop-smoke
/tmp/robocode-0117-daily-loop-smoke-brief
/tmp/robocode-0117-release-smoke-local
/tmp/robocode-0117-release-smoke-local-brief
/tmp/robocode-0117-release-smoke-full-nopackage
/tmp/robocode-0117-release-smoke-full-nopackage-brief
```

## 视觉证据

0.1.17 确定性 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.17-tui-side-2.svg`

Daily-loop smoke evidence：

- `/tmp/robocode-0117-daily-loop-smoke/daily-loop-transcript.log`
- `/tmp/robocode-0117-daily-loop-smoke/daily-loop.diff`
- `/tmp/robocode-0117-daily-loop-smoke/daily-loop-tui-preview.ansi`
- `/tmp/robocode-0117-daily-loop-smoke/summary.md`
- `/tmp/robocode-0117-daily-loop-smoke-brief/workspace/.robocode/briefs/active.md`
- `/tmp/robocode-0117-daily-loop-smoke-brief/workspace/.robocode/steering/conventions.md`

## 剩余风险

- `/setup` 目前是 command-palette guided flow，还不是完整 modal wizard。
  等日常闭环稳定后再做更丰富的 picker。
- 本地 RC 没有默认运行 DeepSeek live smoke；需要单独提供 `DEEPSEEK_API_KEY` 并执行
  `scripts/release-smoke.sh --deepseek`。
- 本地 package check 覆盖当前 host target。多平台 release assets 仍依赖 GitHub
  release workflow。
- Task brief / steering 当前刻意保持最小，只作为 daily-loop context aid，
  还不是完整的 Kiro-style spec product。

## 下一步

完成 `v0.1.17` 发布验证：推送 release tag，等待 GitHub release workflow 产出资产，
更新 Homebrew tap，并运行包含 GitHub assets 和 Homebrew validation 的 post-publish smoke。
