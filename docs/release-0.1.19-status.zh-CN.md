# RoboCode 0.1.19 状态 - Delegated Lane Usefulness

英文版： [release-0.1.19-status.md](release-0.1.19-status.md)

`0.1.19` 是 Delegated Lane Usefulness release。这个版本把 `0.1.18` 的
selector-first 交互规则进一步落到真实开发工作流：provider 配置和 model 选择分离，
deterministic lane evidence 在 cockpit 里可见，发布验证也把 daily-loop 和 lane
operator-loop smoke 纳入常规门禁。

## 发布状态

- Workspace version：`0.1.19`
- Git commit：`2319f26339c6403a6c280d4c1940179b55b79052`
- Git tag：`v0.1.19`
- GitHub release：https://github.com/wikieden/robocode/releases/tag/v0.1.19
- Release workflow：https://github.com/wikieden/robocode/actions/runs/26677360247
- Homebrew tap commit：`wikieden/homebrew-tap@d5fa2c143ae9967b9837104e163289ff3f924764`
- 本地包：`dist/robocode-v0.1.19-aarch64-apple-darwin.tar.gz`
- 本地包 sha256：
  `2124d8ab31f73fe98113a70f9816cf23aa4b204ac68fa44f9612ff986d041937`

## 本版变更

- `/provider` 现在是 provider 配置界面：展示 API key/env 状态、endpoint 来源、
  provider doctor 入口，以及该供应商已知模型候选。
- `/models` 现在是跨 provider 的模型选择器：按供应商分组展示模型，选中一项后通过
  共享 runtime command path 同时切换 provider 和 model。
- `/model` 保留为当前 provider 内的快速模型切换入口。
- `/settings provider` 和 `/setup provider` 使用 provider selector，不再把 provider
  选择误做成 model 选择。
- TUI 确定性预览新增 provider selector 和 model selector 截图，后续交互变更可以直接
  视觉回归。
- release smoke 把 `daily-loop-smoke` 和 `lane-operator-loop-smoke` 放进常规
  post-publish 门禁。

## 验证

聚焦测试和 workspace 检查：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p robocode-cli command_palette --quiet
cargo test -p robocode-core provider --quiet
cargo test -p robocode-cli --quiet
cargo test --workspace --quiet
```

CLI 和视觉 smoke：

```bash
printf '/provider\n/models\n/quit\n' | \
  cargo run -p robocode-cli -- --no-tui --provider fallback --model test-local

ROBOCODE_TUI_SCREENSHOT_VERSION=0.1.19 \
ROBOCODE_TUI_PREVIEW_PROVIDER=deepseek \
ROBOCODE_TUI_PREVIEW_MODEL=deepseek-v4-flash \
scripts/tui-regression.sh docs/previews/generated
```

本地 release smoke：

```bash
scripts/release-smoke.sh --version 0.1.19 --quick \
  --out-dir /tmp/robocode-0119-release-smoke-local
```

结果：9 个检查通过，5 个检查按设计跳过（`package-smoke`、可选
`deepseek-cli-smoke`、GitHub release validation、GitHub asset validation、
Homebrew validation）。

GitHub release asset 验证：

```bash
scripts/release-smoke.sh --version 0.1.19 --quick \
  --github-release-assets --skip-package \
  --out-dir /tmp/robocode-0119-github-release-check
```

结果：GitHub release assets checksum validation 通过。

发布后验证：

```bash
scripts/release-smoke.sh --version 0.1.19 --quick \
  --github-release-assets --homebrew --skip-package \
  --out-dir /tmp/robocode-0119-postpublish-check
```

结果：在 `/tmp/robocode-0119-postpublish-check` 通过，包含 GitHub release assets
和 Homebrew validation。

Homebrew formula 验证：

```bash
brew fetch --formula wikieden/tap/robocode
brew audit --formula wikieden/tap/robocode
```

结果：`brew fetch` 解析到 formula `robocode (0.1.19)`，audit 无错误输出。

## 截图证据

确定性 0.1.19 TUI 截图：

- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-main.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-main-idle.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-live-turn.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-main-resize.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-cjk-input.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-command-palette.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-provider-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-model-selector.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-lane-detail.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-side-1.svg`
- `/Users/wiki/Documents/GitHub/robocode/docs/previews/generated/screenshots/0.1.19-tui-side-2.svg`

## 剩余风险

- Provider 配置仍通过 slash command 和环境变量完成。后续 settings release 应加入首次
  使用向导式 credential 输入，并明确 secret-safe persistence 规则。
- `/models` 已把跨 provider 选择讲清楚，但多数 provider 的远程模型发现仍依赖 descriptor
  或静态列表。
- Codex/Claude delegated lanes 仍是 shared lane/task model 上的 adapter。本版真正阻塞
  发布的是 deterministic shell/template loop，并已完成验证。
