# Independent Core, TUI, and GUI Release Integration Plan

Chinese: [2026-07-19-independent-release-integration.zh-CN.md](2026-07-19-independent-release-integration.zh-CN.md)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` for each integration gate and `superpowers:finishing-a-development-branch` before merging any component branch.

**Goal:** Coordinate independently versioned Core, TUI, and GUI branches through immutable checkpoints and prove I0 contract parity, I1 operability, and I2 trusted local-loop completion.

**Architecture:** Component branches never infer compatibility from SemVer alone. Machine-readable manifests pin schema, capabilities, source revisions, and the exact Core SHA. Shared fixtures, migrations, locale catalogs, generated visual tokens, and gate evidence are verified in a temporary integration worktree in the fixed order Core → TUI → GUI.

**Tech Stack:** Git worktrees, Rust/Cargo, TOML/JSON, shell/Python validation scripts, JSONL canonical logs, bilingual Markdown, design-package HTML/CSS, GitHub Actions.

## Global Constraints

- Use `codex/v3-core-runtime`, `codex/v3-tui-client`, and `codex/v3-gui-client`; implementation ownership is `crates/**`, `apps/tui/**`, and `apps/gui/**` respectively.
- TUI/GUI branches start from the immutable `frontend-contract-v1` commit, not from a moving branch name.
- Integrate and verify Core → TUI → GUI. Never merge a frontend to compensate for an absent Core fact.
- Component SemVer, wire schema, and Core Git SHA are independent identifiers and must all be recorded.
- Canonical logs remain append-only JSONL; SQLite remains a rebuildable index. Migration tests must run before new-client parity tests.
- Built-in locale support is `en` and `zh-CN`; appearance supports five skins, eight valid skin/mode combinations, three densities, and three motion policies.
- Design review order is global index → client index → component library → TUI unified prototype or GUI desktop cockpit. Reference shots are comparison aids only.
- Historical release evidence is not rewritten. Active docs are corrected; `docs/previews.old/**` is explicitly archival.
- No GitHub Release or Homebrew publication occurs in this plan. Publishing remains a separately authorized, coupled operation.

---

### Task 1: Add machine-readable component compatibility manifests

**Files:**
- Create: `crates/core/release-manifest.toml`
- Create: `apps/tui/release-manifest.toml`
- Create: `apps/gui/release-manifest.toml`
- Create: `scripts/check-component-manifests.py`
- Test: `scripts/tests/test_component_manifests.py`

**Interface:** Every manifest contains `component_version`, `min_core_version`, `supported_schema_versions`, `base_core_checkpoint`, `required_capabilities`, `design_source_revision`, `locale_catalog_revision`, and `token_registry_revision`. Core uses its own SHA for `base_core_checkpoint`; frontend values must be full 40-character SHAs.

- [ ] Write failing tests for missing fields, symbolic refs, malformed SemVer/SHA, unsorted capabilities, unsupported schemas, and a frontend minimum Core greater than the integrated Core.
- [ ] Run `python3 -m unittest scripts.tests.test_component_manifests`; expect FAIL because manifests and validator do not exist.
- [ ] Implement strict TOML validation and sample I0 manifests. Do not insert a fake SHA: the checkpoint task writes it after commit creation.
- [ ] Run the unit test and `python3 scripts/check-component-manifests.py crates/core/release-manifest.toml apps/tui/release-manifest.toml apps/gui/release-manifest.toml`; expect PASS.
- [ ] Commit with `git commit -m "build(release): validate independent component manifests"`.

### Task 2: Freeze the shared fixture catalog and digest contract

**Files:**
- Create: `crates/types/tests/fixtures/frontend-contract-v1/catalog.toml`
- Modify: `crates/types/tests/fixtures/frontend-contract-v1/*.json`
- Create: `scripts/check-frontend-fixtures.py`
- Create: `scripts/frontend-fixture-parity.sh`
- Test: `scripts/tests/test_frontend_fixtures.py`

**Interface:** The catalog records schema, required capabilities, expected final cursor, and normalized `RuntimeViewState` SHA-256 for every fixture, including `d1-vertical-slice.json` and later `local-operator-loop.json`.

- [ ] Write failing digest tests for missing entries, duplicate IDs, non-contiguous cursors, changed normalized state, unknown mandatory capabilities, and nondeterministic replay.
- [ ] Run `python3 -m unittest scripts.tests.test_frontend_fixtures`; expect FAIL.
- [ ] Implement canonical JSON normalization and a parity runner that invokes Core, TUI, and GUI fixture tests without copying fixtures into app directories.
- [ ] Run `python3 scripts/check-frontend-fixtures.py` and `scripts/frontend-fixture-parity.sh core`; expect PASS at I0.
- [ ] Commit with `git commit -m "test(contract): freeze the frontend parity corpus"`.

### Task 3: Gate legacy migration before schema-v1 replay

**Files:**
- Create: `scripts/check-frontend-migrations.sh`
- Modify: `crates/types/tests/fixtures/frontend-contract-v1/legacy-lanes.tsv`
- Create: `crates/types/tests/fixtures/frontend-contract-v1/legacy-runtime-events.json`
- Modify: `docs/frontend-integration-contract.md`
- Modify: `docs/frontend-integration-contract.zh-CN.md`

- [ ] Add failing tests proving v0 lane/task/approval/transcript data migrates to typed v1 facts and replaying the migration twice is idempotent.
- [ ] Run `scripts/check-frontend-migrations.sh`; expect FAIL until all migration entry points exist.
- [ ] Implement the ordered gate: parse legacy → migrate → replay v1 → rebuild SQLite → compare canonical facts.
- [ ] Run `scripts/check-frontend-migrations.sh && cargo test -p viden-types -p viden-session -p viden-workflows`; expect PASS.
- [ ] Commit with `git commit -m "test(contract): gate legacy frontend migrations"`.

### Task 4: Validate shared locale keys and UI preference semantics

**Files:**
- Create: `docs/locales/core-keys.json`
- Create: `scripts/check-locale-catalogs.py`
- Test: `scripts/tests/test_locale_catalogs.py`
- Modify: `crates/config/README.md`
- Modify: `crates/config/README.zh-CN.md`

**Interface:** Core facts expose stable key plus typed arguments. TUI/GUI catalogs must have identical key and argument sets for `en` and `zh-CN`. Resolution is explicit override → saved user preference → system locale → `en`; project config cannot force personal UI preferences.

- [ ] Write failing tests for missing/empty keys, argument drift, translated code/path/shortcut tokens, invalid locale aliases, fallback loops, and project preference override.
- [ ] Run `python3 -m unittest scripts.tests.test_locale_catalogs`; expect FAIL before catalogs are wired.
- [ ] Implement catalog discovery for both selected GUI layouts and TUI, visible-key fallback, and revision hashing.
- [ ] Run `python3 scripts/check-locale-catalogs.py apps/tui apps/gui docs/locales/core-keys.json`; expect PASS.
- [ ] Commit with `git commit -m "test(ui): enforce cross-client locale parity"`.

### Task 5: Generate and verify cross-client appearance tokens

**Files:**
- Create: `scripts/generate-ui-tokens.py`
- Create: `scripts/check-ui-token-parity.py`
- Test: `scripts/tests/test_ui_tokens.py`
- Source: `docs/viden-design/Viden/tokens.css`
- Generated: `apps/tui/src/tui/theme_tokens.rs`
- Generated: selected GUI token adapter under `apps/gui/src/ui/`

- [ ] Write failing tests for the eight valid skin/mode pairs, dark-only Amber/Phosphor, complete semantic roles, atomic Aurora dark/regular fallback, density geometry, reduced motion, contrast metadata, and stale generated output.
- [ ] Run `python3 -m unittest scripts.tests.test_ui_tokens`; expect FAIL.
- [ ] Implement deterministic generation with a registry revision; generated files carry a source digest and are never hand-edited.
- [ ] Run `python3 scripts/generate-ui-tokens.py --check && python3 scripts/check-ui-token-parity.py`; expect PASS.
- [ ] Commit with `git commit -m "build(ui): generate shared appearance adapters"`.

### Task 6: Clean active visual documentation and archive old evidence

**Files:**
- Modify pairs: `docs/viden-design-adoption*.md`, `docs/tui-cockpit-design*.md`, `docs/gui-version-functional-design*.md`, `docs/tui-interaction-flow-design*.md`, `docs/user-guide*.md`, `docs/parallel-development-plan*.md`, `docs/staged-roadmap*.md`
- Modify: `docs/ui-collaboration-guide.zh-CN.md`
- Modify: `docs/previews.old/README.md`
- Modify: `docs/previews.old/generated/README.md`
- Modify: `docs/previews.old/generated/screenshots/README.md`
- Create: `scripts/check-active-visual-sources.py`

- [ ] Write a failing checker that rejects active references to deleted `d1v2.png`, `s13.png`, `cockpit-final.png`, `welcome-watcher.png`, `lane-monitor-wide.png`, or `docs/previews/generated`, but exempts immutable historical release/status plans.
- [ ] Run `python3 scripts/check-active-visual-sources.py`; expect FAIL on current active docs.
- [ ] Update active docs to the canonical hierarchy and current reference shots. Separate D1 permission dock, D2 decisions, D12 conflict bounce, D14 audit, and Evidence. Mark `previews.old` as non-authoritative 0.1.x evidence.
- [ ] Run the visual checker, paired-doc/link checks, and `node docs/viden-design/Viden/tools/run-checks.node.js tokens icons changelog status`; expect PASS.
- [ ] Commit with `git commit -m "docs(design): align active visuals with the current package"`.

### Task 7: Certify I0 Contract

**Files:**
- Create: `docs/integration/i0-contract.md`
- Create: `docs/integration/i0-contract.zh-CN.md`
- Modify: all three component manifests

- [ ] From Core `0.3.0`, run types/runtime/core tests, migration, fixture digest, and dependency-boundary gates.
- [ ] Commit the Core freeze, resolve its payload SHA, write that SHA into the paired compatibility documents, and commit a separate evidence checkpoint. Verify every frontend manifest records the payload SHA and every frontend branch starts from the evidence checkpoint. Create an immutable `frontend-contract-v1` tag only with separate user authorization.
- [ ] Create TUI/GUI worktrees from that commit; run their alpha fixture consumers and framework gate without production migration shortcuts.
- [ ] Run `scripts/frontend-fixture-parity.sh all` plus manifest/doc checks; expect identical normalized state and cursor digests.
- [ ] Commit with `git commit -m "docs(integration): certify I0 frontend contract"`.

### Task 8: Certify I1 Operable

**Files:**
- Create: `docs/integration/i1-operable.md`
- Create: `docs/integration/i1-operable.zh-CN.md`
- Modify: component manifests for Core `0.3.1`, TUI `0.2.0`, GUI `0.1.0-beta.1`

- [ ] Integrate Core first and verify multi-lane ownership, project setup, owner-scoped queue/cancel/approval/error, and authoritative worktree/process/apply effects.
- [ ] Integrate TUI second; prove no `SessionEngine`, provider, Git, process, or direct persistence authority remains and run 0.1.30 stability regressions.
- [ ] Integrate the selected GUI third; prove D11 → D4 → D1, permission dock, D6 recovery, CJK/keyboard/a11y, locale, themes, density, and motion.
- [ ] Run migrations, full fixture parity, `cargo test --workspace --quiet`, TUI scripts, GUI selected-framework tests, and active visual checks.
- [ ] Commit with `git commit -m "docs(integration): certify I1 operable baseline"`.

### Task 9: Certify I2 Trusted Local Loop

**Files:**
- Create: `docs/integration/i2-local-loop.md`
- Create: `docs/integration/i2-local-loop.zh-CN.md`
- Create: `scripts/run-local-operator-loop.sh`
- Modify: component manifests for Core `0.3.2`, TUI `0.2.1`, GUI `0.1.0`

- [ ] Write the failing `local-operator-loop.json` expectations: request → work → test/review → evidence → gate → apply or conflict recovery, with stable owner/audit IDs and no transcript inference.
- [ ] Run the scenario independently through Core, TUI, and GUI; expect FAIL until all P0 surfaces exist.
- [ ] Complete only missing in-scope facts/surfaces in their owning branches, then re-integrate Core → TUI → GUI.
- [ ] Run `scripts/run-local-operator-loop.sh --clients core,tui,gui`; expect the same final projection/audit digest, with conflict returning to the owning lane and apply only after an accepted gate.
- [ ] Commit with `git commit -m "docs(integration): certify I2 trusted local loop"`.

### Task 10: Final merge evidence and main readiness

**Files:**
- Create: `docs/release-independent-train-status.md`
- Create: `docs/release-independent-train-status.zh-CN.md`
- Modify: `.github/workflows/rust.yml`
- Modify: `PLAN.md`
- Modify: `docs/staged-roadmap.md`
- Modify: `docs/staged-roadmap.zh-CN.md`

- [ ] Add CI jobs for manifest, migration, fixture parity, locale, token, active visual source, TUI boundary, selected GUI, and workspace tests.
- [ ] In a clean temporary integration worktree, merge the verified component heads Core → TUI → GUI and record base/head SHAs plus conflict resolutions.
- [ ] Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --quiet`, all scripts from Tasks 1–9, and `git diff --check`; expect PASS with a clean status except intentional evidence files.
- [ ] Review the final diff for required bilingual docs and comments on protocol, permission, recovery, and migration invariants.
- [ ] Commit with `git commit -m "docs(release): record independent train completion"`. Merge/push main only under the user's current authorization; do not create releases or update Homebrew here.

## Gate Summary

| Gate | Versions | Required evidence |
| --- | --- | --- |
| I0 | Core 0.3.0 / TUI alpha.1 / GUI alpha.1 | immutable SHA, schema/capabilities, migrations, three-client fixture parity, GUI framework decision |
| I1 | Core 0.3.1 / TUI 0.2.0 / GUI beta.1 | multi-lane authority in Core, usable unified cockpit/desktop cockpit, locale and appearance persistence |
| I2 | Core 0.3.2 / TUI 0.2.1 / GUI 0.1.0 | real trusted local loop, evidence/gate/apply/recovery, replay/audit parity, full visual/accessibility gates |
