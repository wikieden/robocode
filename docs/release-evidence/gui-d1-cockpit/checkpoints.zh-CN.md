# GUI D1 Cockpit 集成检查点

英文版：[checkpoints.md](checkpoints.md)

日期：2026-07-24

本文记录 D1 cockpit 的本地 Core+GUI 集成检查点。它只作为证据，不是已发布 release。

## 检查点

| 项目 | SHA / 路径 |
| --- | --- |
| 基线 `origin/main` | `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb` |
| Core 来源 | `f7fe1b31dfb237e4062209767a7051c2b2c68b93` |
| Core merge | `6d0094a11cc64c097a2f48ee6122ec8bc95a2d23` |
| GUI 来源 | `4cb2a498c5b091f62f554ff407acdf162f96cc1e` |
| GUI merge | `d27dc09fd230e3c2fb3ae79fbf3d10a45f400226` |
| 分支 | `codex/d1-cockpit-integration` |
| Worktree | `/Users/wiki/Documents/GitHub/viden/.worktrees/d1-cockpit-integration` |
| App bundle | `target/release/bundle/macos/Viden.app` |
| DMG bundle | `target/release/bundle/dmg/Viden_0.1.0-rc.3_aarch64.dmg` |

## 版本

| 组件 | 版本 | 证据 |
| --- | --- | --- |
| Core | `0.3.5` | `crates/core/release-manifest.toml` |
| GUI | `0.1.0-rc.3` | `apps/gui/manifests/0.1.0-rc.3.toml`、app `Info.plist` |
| TUI | `0.3.3` | 本任务未合并；workspace 仍包含继承的 TUI/Core API drift。 |

## 确定性证据

| 命令 | 结果 |
| --- | --- |
| `cargo test -p viden-types` | PASS，77 passed |
| `cargo test -p viden-runtime` | 最终重跑 PASS，461 passed、1 ignored；首次完整运行有一次 transient cancellation-test failure |
| `cargo test -p viden-core` | PASS |
| `cargo test -p viden-gui` | PASS |
| `npm --prefix apps/gui ci` | PASS，0 vulnerabilities |
| `npm --prefix apps/gui test -- tests/d1_cockpit.spec.ts --run` | PASS，native-menu 修复后 54 个 D1 focused tests |
| `npm --prefix apps/gui test -- --run` | PASS，native-menu 修复后 17 个文件、239 个测试 |
| `npm --prefix apps/gui run build` | native-menu 修复后 PASS |
| `bash scripts/native-acp-fixture-parity.sh` | BLOCKED，脚本不存在 |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `cargo test --workspace --quiet` | BLOCKED，`viden-tui` 无法针对当前 Core facade 编译 |
| `npm --prefix apps/gui run tauri build` | native-menu 修复后 PASS |

## Hash 证据

| Artifact | SHA-256 |
| --- | --- |
| `crates/types/tests/fixtures/frontend-contract-v1/d1-main-cockpit.json` | `f96ba30cc6e80aa52cb15a2fd1f03c082487a3cd4779c25f61e42ee1548e1e3b` |
| `apps/gui/evidence/0.1.0-rc.3/d1-design-reference-canonical.png` | `f9209057b5538278da861e04bb43b891438802d9a41dcb5f1476b341b93dc11c` |
| `apps/gui/evidence/0.1.0-rc.3/d1-context-dock-bottom-1280x800.png` | `0179f20ac53a484dfb0194392d206d7e182eae1d33d0fd0e94f43c1e2fcc6c30` |
| `apps/gui/evidence/0.1.0-rc.3/d1-design-reference-vs-actual.png` | `d27302d81afaeadfc156513eed30d251ff09194b1b3392010baeac5602ced5e8` |
| `apps/gui/evidence/0.1.0-rc.3/accepted-target-dark-cockpit.png` | `d4c97aa4ebe603eddd290785a0e632fd41b72a94de5e7ccb6206352bb0f37e36` |

## Native Smoke 边界

已构建并验证 app bundle 元数据：

- `CFBundleExecutable`：`viden-gui`
- `CFBundleIdentifier`：`dev.viden.gui`
- `CFBundleName`：`Viden`
- `CFBundleShortVersionString`：`0.1.0-rc.3`
- `CFBundleVersion`：`0.1.0-rc.3`
- app size：`27M`
- DMG size：`9.5M`
- signature：ad-hoc，无 TeamIdentifier

精确 bundle 启动后产生 PID `49949`，但进程级 desktop path 显示该 PID 有 0 个
Accessibility window，同时旧的同名 Viden 进程持有 1 个 window。

独立 native-app desktop control 在集成 bundle 上验证了 Welcome、native Open
Project、选择安全 temp project、D1 shell retention，以及打开紧凑 `+ New Lane`
popup。

本检查点包含一个 scoped GUI 修复：ACP probe 运行时，native `Viden Agent` Lane
creation 仍保持 enabled；ACP 选项仍正确由 ACP readiness 控制。
修复后 native smoke 证明菜单可以 resolve：`Viden Agent` enabled 且可选，`Codex`
Ready，`Kiro` Ready，`Claude` disabled 并显示 initialize-probe failure。选择
`Viden Agent` 会关闭菜单，但 5 秒后没有新 Lane 出现。当前 native smoke 阻塞点
是 native Lane creation/Core owner binding。已有选中 Lane 也仍没有唯一 Core
execution owner，因此发送保持 disabled。该修复已由 `npm --prefix apps/gui test --
tests/d1_cockpit.spec.ts --run` 和 full GUI npm suite 覆盖。
