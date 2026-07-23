# GUI D1 Cockpit 集成检查点

英文版：[checkpoints.md](checkpoints.md)

日期：2026-07-24

本证据只描述本地候选，不是已发布、签名、公证、push、merge、tag 或 live
provider 认证的 release。

## 候选线

| 项目 | SHA / 路径 |
| --- | --- |
| 基线 `origin/main` | `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core `0.3.5` 来源 / merge | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` / `76f7f8e3a84ff38846023dda7dead0c50bfb2b68` |
| TUI `0.3.3` 来源 / merge | `6260f183d19da27e61fdf068d67a9c481c68d829` / `026736dc4c16b1d039b80e77b9fe8ff99788d51b` |
| GUI `0.1.0-rc.3` 来源 / merge | `1c44094dd29674e1cc585ff6c83302581440aeb0` / `864966d0677e9d958396fac150f4701b2d14b0a1` |
| Integration fix | `cb9baaf7ff212655d3b1ea8dd3cb4684ae40f7d0` |
| Fixture parity | `d4fe33fb0510bf05fb4586ddf2ec4cd7718f185d` |
| Task 14 base | `d4fe33fb0510bf05fb4586ddf2ec4cd7718f185d` |
| 分支 | `codex/d1-cockpit-closed-loop` |
| Worktree | `.worktrees/d1-cockpit-closed-loop` |

旧 `codex/d1-cockpit-integration` Core+GUI-only 线只保留为历史 blocked 证据；
上表才是当前候选。

## 确定性证据

| 命令 | 结果 |
| --- | --- |
| `bash scripts/native-acp-fixture-parity.sh` | PASS：精确 Core replay/hash、TUI render 与 GUI projection 证明 |
| `cargo test -p viden-types` | PASS，77 |
| `cargo test -p viden-runtime` | PASS，461 + 1 ignored |
| `cargo test -p viden-core` | PASS |
| `cargo test -p viden-tui` | PASS，269 + 1 API |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui test -- --run` | PASS，17 files / 243 tests |
| `npm --prefix apps/gui run build` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | PASS |
| `scripts/tui-turn-controller-smoke.sh` | PASS |
| `scripts/tui-regression.sh` | Core 0.3.5 extension count 修复后 PASS |
| `scripts/rc-tui-stability-smoke.sh` | 同一修复后 PASS |
| `cargo fmt --all -- --check` | PASS |

Canonical parity fixture 含 22 个事件，终点为
`fixture:interaction-closed-loop@22`，Core view SHA-256 为
`46db05abaaae36cf37cb7ffa0493a4ef8c158a2d5b4ffeef08d01dbf8e284ed0`。
详见
[fixture-parity.zh-CN.md](../native-acp-interaction/fixture-parity.zh-CN.md)。

## App 构建证据

`npm --prefix apps/gui run tauri -- build --bundles app` 成功完成：

- App：`target/release/bundle/macos/Viden.app`
- executable：
  `target/release/bundle/macos/Viden.app/Contents/MacOS/viden-gui`
- `CFBundleIdentifier`：`dev.viden.gui`
- `CFBundleExecutable`：`viden-gui`
- `CFBundleShortVersionString`：`0.1.0-rc.3`
- executable size：`27,826,752` bytes
- signature：ad-hoc linker-signed，无 TeamIdentifier

未运行项目 distribution-signing 或公证门禁。

## Native Smoke 边界

状态：**等待解锁 Mac**。

Smoke fixture 是 `/tmp/viden-native-smoke.Hmd3ak`，它是仅包含一个已提交 README
的干净 Git 仓库。Computer Use 精确指向上述 App，但报告 Mac 已锁定且自动解锁
失败。目前没有 native interaction 或 screenshot 证据，因此本检查点不声称
Welcome、project selection、zero-Lane、Lane creation、one-Lane/one-Agent、
composer input、runtime output、approval、ACP readiness、locale 或 skin 通过。

解锁后必须针对同一 bundle、使用 fallback `test-local` 恢复 smoke，把关键截图
保存到 `native-smoke/`，并记录 typed completion 或精确 typed
recovery/rejection。不得输入凭证，也不得运行 live provider/ACP turn。
