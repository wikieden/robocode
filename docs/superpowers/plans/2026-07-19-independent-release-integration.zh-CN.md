# Core、TUI、GUI 独立版本集成实施计划

英文版：[2026-07-19-independent-release-integration.md](2026-07-19-independent-release-integration.md)

> **给 agentic workers：** 每个集成门必须使用 `superpowers:executing-plans`，合入任何组件分支前必须使用 `superpowers:finishing-a-development-branch`。

**目标：** 用不可变 checkpoint 协调独立版本的 Core、TUI、GUI 分支，依次证明 I0 合同一致、I1 可操作、I2 可信本地闭环完成。

**架构：** 组件不得只凭 SemVer 猜兼容性。机器可读 manifest 固定 schema、capabilities、source revision 和精确 Core SHA。共享 fixture、migration、locale catalog、生成式视觉 token 与 gate evidence 在临时集成 worktree 中按 Core → TUI → GUI 验证。

**技术栈：** Git worktree、Rust/Cargo、TOML/JSON、Shell/Python 校验、JSONL canonical log、双语 Markdown、设计 HTML/CSS、GitHub Actions。

## 全局约束

- 分支为 `codex/v3-core-runtime`、`codex/v3-tui-client`、`codex/v3-gui-client`；所有权分别是 `crates/**`、`apps/tui/**`、`apps/gui/**`。
- TUI/GUI 必须从不可变 `frontend-contract-v1` commit 创建，不能从移动分支名创建。
- 固定按 Core → TUI → GUI 集成验证；前端不得为缺失 Core 事实自行补合同。
- 组件 SemVer、wire schema、Core SHA 是三个独立标识，必须同时记录。
- JSONL 保持 canonical、append-only；SQLite 仅为可重建索引。先跑 migration，再跑新客户端 parity。
- 内置 locale 为 `en`、`zh-CN`；外观包含五种 skin、八个有效 skin/mode、三种 density、三种 motion。
- 设计审查顺序为全局 index → 客户端 index → 组件库 → TUI 统一原型或 GUI 桌面驾驶舱；reference shot 只用于比对。
- 不改写历史 release evidence；只修活跃文档，并明确 `docs/previews.old/**` 是 archive。
- 本计划不创建 GitHub Release 或更新 Homebrew；发布仍是另行授权且必须联动的动作。

---

### 任务 1：增加机器可读组件兼容 Manifest

**文件：** 新增 `crates/core/release-manifest.toml`、`apps/tui/release-manifest.toml`、`apps/gui/release-manifest.toml`、`scripts/check-component-manifests.py`、`scripts/tests/test_component_manifests.py`。

**接口：** 每个 manifest 包含 `component_version`、`min_core_version`、`supported_schema_versions`、`base_core_checkpoint`、`required_capabilities`、`design_source_revision`、`locale_catalog_revision`、`token_registry_revision`；前端 SHA 必须是完整 40 字符。

- [ ] 先写缺字段、符号 ref、非法 SemVer/SHA、未排序 capabilities、不支持 schema、min Core 超前的失败测试。
- [ ] 运行 `python3 -m unittest scripts.tests.test_component_manifests`，预期 FAIL。
- [ ] 实现严格 TOML 校验与 I0 manifest；checkpoint task 提交后才写真实 SHA，禁止伪造。
- [ ] 运行 unit test 与 `python3 scripts/check-component-manifests.py crates/core/release-manifest.toml apps/tui/release-manifest.toml apps/gui/release-manifest.toml`，预期 PASS。
- [ ] 提交 `git commit -m "build(release): validate independent component manifests"`。

### 任务 2：冻结共享 Fixture Catalog 与 Digest 合同

**文件：** 新增 `crates/types/tests/fixtures/frontend-contract-v1/catalog.toml`、`scripts/check-frontend-fixtures.py`、`scripts/frontend-fixture-parity.sh`、`scripts/tests/test_frontend_fixtures.py`；修改同目录 fixture。

**接口：** catalog 为每个 fixture 记录 schema、required capabilities、最终 cursor、normalized `RuntimeViewState` SHA-256，包含 `d1-vertical-slice.json` 与后续 `local-operator-loop.json`。

- [ ] 先写缺 entry、重复 ID、cursor 不连续、digest 变化、未知 mandatory capability、非确定重放测试。
- [ ] 运行 `python3 -m unittest scripts.tests.test_frontend_fixtures`，预期 FAIL。
- [ ] 实现 canonical JSON normalization 和调用三端测试的 parity runner，禁止复制 fixture 到 app 目录。
- [ ] 运行 `python3 scripts/check-frontend-fixtures.py && scripts/frontend-fixture-parity.sh core`，I0 预期 PASS。
- [ ] 提交 `git commit -m "test(contract): freeze the frontend parity corpus"`。

### 任务 3：在 Schema-v1 Replay 前验证旧数据 Migration

**文件：** 新增 `scripts/check-frontend-migrations.sh`、`crates/types/tests/fixtures/frontend-contract-v1/legacy-runtime-events.json`；修改 `legacy-lanes.tsv`、`docs/frontend-integration-contract.md` 与 `.zh-CN.md`。

- [ ] 先写 v0 lane/task/approval/transcript 到 typed v1 的失败测试，并验证重复 migration 幂等。
- [ ] 运行 `scripts/check-frontend-migrations.sh`，预期 FAIL。
- [ ] 实现 legacy parse → migrate → v1 replay → SQLite rebuild → canonical fact compare 的固定顺序。
- [ ] 运行脚本及 `cargo test -p viden-types -p viden-session -p viden-workflows`，预期 PASS。
- [ ] 提交 `git commit -m "test(contract): gate legacy frontend migrations"`。

### 任务 4：验证共享 Locale Key 与 UI Preference 语义

**文件：** 新增 `docs/locales/core-keys.json`、`scripts/check-locale-catalogs.py`、`scripts/tests/test_locale_catalogs.py`；修改 `crates/config/README.md` 与 `.zh-CN.md`。

**接口：** Core fact 发稳定 key 与 typed args；TUI/GUI 的 `en`、`zh-CN` key/参数集合一致。解析顺序为显式 override → 保存的用户偏好 → 系统 locale → `en`；项目配置不能强制个人偏好。

- [ ] 先写缺失/空 key、参数漂移、翻译 code/path/shortcut、错误 alias、fallback loop、项目覆盖个人偏好的失败测试。
- [ ] 运行 `python3 -m unittest scripts.tests.test_locale_catalogs`，预期 FAIL。
- [ ] 实现 TUI 与两种 GUI layout 的 catalog discovery、可见 key fallback、revision hash。
- [ ] 运行 `python3 scripts/check-locale-catalogs.py apps/tui apps/gui docs/locales/core-keys.json`，预期 PASS。
- [ ] 提交 `git commit -m "test(ui): enforce cross-client locale parity"`。

### 任务 5：生成并验证跨端外观 Token

**文件：** 新增 `scripts/generate-ui-tokens.py`、`scripts/check-ui-token-parity.py`、`scripts/tests/test_ui_tokens.py`；源为 `docs/viden-design/Viden/tokens.css`；生成 TUI `theme_tokens.rs` 与 GUI selected-framework adapter。

- [ ] 先写八个有效组合、Amber/Phosphor dark-only、完整 semantic role、Aurora dark/regular 原子回退、density geometry、reduced motion、contrast metadata、生成物过期测试。
- [ ] 运行 `python3 -m unittest scripts.tests.test_ui_tokens`，预期 FAIL。
- [ ] 实现带 source digest 和 registry revision 的确定性生成，生成物禁止手改。
- [ ] 运行 `python3 scripts/generate-ui-tokens.py --check && python3 scripts/check-ui-token-parity.py`，预期 PASS。
- [ ] 提交 `git commit -m "build(ui): generate shared appearance adapters"`。

### 任务 6：清理活跃视觉文档并归档旧证据

**文件：** 修改 `docs/viden-design-adoption*.md`、`tui-cockpit-design*.md`、`gui-version-functional-design*.md`、`tui-interaction-flow-design*.md`、`user-guide*.md`、`parallel-development-plan*.md`、`staged-roadmap*.md`、`ui-collaboration-guide.zh-CN.md`、三个 `docs/previews.old/**/README.md`；新增 `scripts/check-active-visual-sources.py`。

- [ ] 先写 checker：活跃文档不得引用已删除的 `d1v2.png`、`s13.png`、`cockpit-final.png`、`welcome-watcher.png`、`lane-monitor-wide.png`、`docs/previews/generated`；历史 release/status 豁免。
- [ ] 运行 checker，当前活跃文档预期 FAIL。
- [ ] 按 canonical hierarchy 更新活跃文档，拆开 D1 permission、D2 decision、D12 conflict、D14 audit 与 Evidence；将 `previews.old` 标为非真源的 0.1.x evidence。
- [ ] 运行 checker、双语/链接检查与 `node docs/viden-design/Viden/tools/run-checks.node.js tokens icons changelog status`，预期 PASS。
- [ ] 提交 `git commit -m "docs(design): align active visuals with the current package"`。

### 任务 7：认证 I0 Contract

**文件：** 新增 `docs/integration/i0-contract.md` 与 `.zh-CN.md`；修改三份 manifest。

- [ ] 对 Core 0.3.0 跑 types/runtime/core、migration、fixture digest、dependency boundary gate。
- [ ] 提交 Core freeze，解析 payload SHA，把它写入成对兼容文档，再提交单独的 evidence checkpoint。验证每个前端 manifest 都记录 payload SHA，且每个前端分支都从 evidence checkpoint 起步。只有另获用户授权时才创建不可变 `frontend-contract-v1` tag。
- [ ] 从该 SHA 创建 TUI/GUI worktree，运行 alpha fixture consumer 与 framework gate，不得用生产迁移捷径。
- [ ] 运行 `scripts/frontend-fixture-parity.sh all`、manifest/doc checks，三端 normalized state/cursor digest 必须一致。
- [ ] 提交 `git commit -m "docs(integration): certify I0 frontend contract"`。

### 任务 8：认证 I1 Operable

**文件：** 新增 `docs/integration/i1-operable.md` 与 `.zh-CN.md`；manifest 更新为 Core 0.3.1、TUI 0.2.0、GUI 0.1.0-beta.1。

- [ ] 先集成 Core，验证 multi-lane、project setup、owner-scoped queue/cancel/approval/error 与权威 worktree/process/apply。
- [ ] 再集成 TUI，证明无 `SessionEngine`、provider、Git、process、direct persistence authority，并运行 0.1.30 stability regression。
- [ ] 最后集成 selected GUI，证明 D11 → D4 → D1、permission dock、D6、CJK/keyboard/a11y、locale、theme、density、motion。
- [ ] 运行 migration、全 fixture parity、`cargo test --workspace --quiet`、TUI scripts、GUI tests、active visual checks。
- [ ] 提交 `git commit -m "docs(integration): certify I1 operable baseline"`。

### 任务 9：认证 I2 Trusted Local Loop

**文件：** 新增 `docs/integration/i2-local-loop.md` 与 `.zh-CN.md`、`scripts/run-local-operator-loop.sh`；manifest 更新为 Core 0.3.2、TUI 0.2.1、GUI 0.1.0。

- [ ] 先写 `local-operator-loop.json` 失败预期：request → work → test/review → evidence → gate → apply/recovery，owner/audit ID 稳定，不解析 transcript。
- [ ] 通过 Core、TUI、GUI 分别运行，P0 surface 未完成前预期 FAIL。
- [ ] 只在所属分支补齐缺失事实/表面，再按 Core → TUI → GUI 集成。
- [ ] 运行 `scripts/run-local-operator-loop.sh --clients core,tui,gui`，三端 final projection/audit digest 一致；冲突退回 owning lane，accepted gate 后才 apply。
- [ ] 提交 `git commit -m "docs(integration): certify I2 trusted local loop"`。

### 任务 10：最终合入证据与 Main Readiness

**文件：** 新增 `docs/release-independent-train-status.md` 与 `.zh-CN.md`；修改 `.github/workflows/rust.yml`、`PLAN.md`、`docs/staged-roadmap.md` 与 `.zh-CN.md`。

- [ ] CI 增加 manifest、migration、fixture parity、locale、token、active visual、TUI boundary、selected GUI、workspace jobs。
- [ ] 在干净临时 integration worktree 中按 Core → TUI → GUI 合入验证 head，记录 base/head SHA 与 conflict resolution。
- [ ] 运行 fmt、clippy、workspace tests、任务 1–9 全部脚本及 `git diff --check`，只允许 intentional evidence 文件。
- [ ] 审查最终 diff 的双语文档，以及 protocol/permission/recovery/migration invariant 注释。
- [ ] 提交 `git commit -m "docs(release): record independent train completion"`。只在用户当前授权范围内合并/推送 main；本任务不创建 Release 或更新 Homebrew。

## Gate 汇总

| Gate | 版本 | 必须证据 |
| --- | --- | --- |
| I0 | Core 0.3.0 / TUI alpha.1 / GUI alpha.1 | 不可变 SHA、schema/capabilities、migration、三端 fixture parity、GUI framework decision |
| I1 | Core 0.3.1 / TUI 0.2.0 / GUI beta.1 | Core 多 Lane 权威、可用 TUI/GUI 驾驶舱、locale/appearance persistence |
| I2 | Core 0.3.2 / TUI 0.2.1 / GUI 0.1.0 | 真实可信闭环、evidence/gate/apply/recovery、replay/audit parity、完整视觉/a11y gate |
