# GUI 0.1.0-rc.3 D1 Cockpit 集成状态

英文版：[release-gui-0.1.0-rc.3-status.md](release-gui-0.1.0-rc.3-status.md)

日期：2026-07-24

这是本地、未发布的集成候选。它不是 tag、push、main merge、签名或公证构建、
Homebrew 更新、release 或 live provider 认证。

## 当前候选

| 字段 | 值 |
| --- | --- |
| 分支 | `codex/d1-cockpit-closed-loop` |
| Worktree | `.worktrees/d1-cockpit-closed-loop` |
| 基线 | `origin/main` at `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core | `0.3.5`，来源 `f7fe1b31`，merge `76f7f8e3` |
| TUI | `0.3.3`，来源 `6260f183`，merge `026736dc` |
| GUI | `0.1.0-rc.3`，来源 `1c44094d`，merge `864966d0` |
| Fixture parity | `interaction-closed-loop`，22 个有序事件，PASS |

当前候选按 Core -> TUI -> GUI 的固定顺序从最新 `origin/main` 重建。原
`codex/d1-cockpit-integration` Core+GUI-only 检查点是历史 blocked 证据，不是
当前候选；它的 TUI 编译 drift、缺失 parity script 与 native Lane creation 结果
不得归到当前重建线。

## 确定性门禁

| 门禁 | 结果 |
| --- | --- |
| `bash scripts/native-acp-fixture-parity.sh` | PASS，Core、TUI、GUI 各一项精确证明 |
| `cargo test -p viden-types` | PASS，77 passed |
| `cargo test -p viden-runtime` | PASS，461 passed、1 ignored |
| `cargo test -p viden-core` | PASS，3 个手动 fixture refresh 测试 ignored |
| `cargo test -p viden-tui` | PASS，269 个 library 与 1 个 API test |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS，17 个文件、243 个测试 |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | 将过期 extension capability 断言从 15 对齐 Core 0.3.5 的 16 后 PASS |
| `scripts/rc-tui-stability-smoke.sh` | 同一 scoped gate 修复后 PASS |
| `cargo fmt --all -- --check` | PASS |

Parity 详情见
[fixture-parity.zh-CN.md](release-evidence/native-acp-interaction/fixture-parity.zh-CN.md)。

## 独立 macOS 构建

`npm --prefix apps/gui run tauri -- build --bundles app` 通过。

- bundle：`target/release/bundle/macos/Viden.app`
- executable：
  `target/release/bundle/macos/Viden.app/Contents/MacOS/viden-gui`
- bundle identifier：`dev.viden.gui`
- version：`0.1.0-rc.3`
- executable size：`27,826,752` bytes
- signature：ad-hoc linker-signed，无 TeamIdentifier

这不是 distribution-signed 候选；未运行项目签名和公证。

## Native App Smoke

状态：**等待解锁 Mac**。

已在 `/tmp/viden-native-smoke.Hmd3ak` 准备一个带最小 README 提交的安全临时
Git 仓库。首次 Computer Use 调用精确指向本次构建的 `.app`，但 macOS 桌面已
锁定且自动解锁失败。因此尚未声称 Welcome、Open Project、zero-Lane、New Lane、
composer、approval、ACP readiness、locale、skin 或 one-Lane/one-Agent 的任何
native 结果。确定性 fixture parity 不替代 native smoke。

Mac 解锁后无需重建即可继续。Smoke 只能使用 fallback `test-local` model，不得
输入凭证，关键截图必须保存到
`docs/release-evidence/gui-d1-cockpit/native-smoke/`。

## 决策

确定性集成与 standalone app build 门禁通过。构建 App 完成文档规定的 desktop
smoke 前，当前候选没有获得 native closed-loop 认证，也不能视为 main-ready。
未执行 live provider、ACP login、credential creation、push、merge、tag、签名、
公证、Homebrew mutation、release 或 publication。
