# TUI 0.3 and GUI 0.1 Parallel Preferences Implementation Plan

Chinese version: [2026-07-20-tui-gui-parallel-preferences-plan.zh-CN.md](2026-07-20-tui-gui-parallel-preferences-plan.zh-CN.md)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define the next Core, TUI, and GUI version slice so TUI and GUI can develop concurrently from one Core checkpoint while sharing multilingual and appearance preference contracts.

**Architecture:** Core owns the persisted presentation preference model, effective preference resolution, and versioned events. TUI and GUI consume the same Core facts, then adapt language strings, skin tokens, density, motion, and color capabilities locally without creating private palettes or private preference stores.

**Tech Stack:** Rust workspace, `viden-core` frontend contract, `viden-types` DTOs, Ratatui/Crossterm TUI, framework-gated GUI, JSON/Serde fixtures, generated palettes from `docs/viden-design/Viden/tokens.css`, bilingual Markdown.

## Global Constraints

- Review design in this order: `docs/viden-design/Viden/index.html` -> client design index -> canonical prototype -> component library.
- TUI canonical prototype: `docs/viden-design/Viden/TUI/Viden - 统一原型 (TUI).html`.
- GUI canonical prototype: `docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html`.
- `tokens.css`, `i18n-dict.js`, `chrome.js`, `tui-kit.css`, `gui-kit.css`, and component registries remain the design truth; archived pages and isolated lower-level pages are supporting evidence only.
- Core version line is `core-v0.3.x`; TUI version line is `tui-v0.3.x`; GUI version line is `gui-v0.1.x`.
- Built-in locales are `en` and `zh-CN`; `system` is a resolver input, not a stored rendered language.
- Built-in skins are `aurora`, `ice`, `mono`, `amber`, and `phosphor`; only `aurora`, `ice`, and `mono` support light mode.
- Valid appearance pairs are the eight combinations defined by the design package. Invalid values must resolve safely and emit a visible diagnostic.
- Frontends must not infer success from transcript text and must not persist a second business or preference model.

---

## Version Slice

| Integration gate | Core | TUI | GUI | Outcome |
| --- | --- | --- | --- | --- |
| P0 Contract | `core-v0.3.0` | `tui-v0.3.0-alpha.1` | `gui-v0.1.0-alpha.1` | `frontend-contract-v1` includes presentation preferences and fixtures. TUI/GUI prove fixture consumption. |
| P1 Operable | `core-v0.3.1` | `tui-v0.3.0` | `gui-v0.1.0-beta.1` | TUI unified cockpit and GUI desktop cockpit both expose language/appearance settings backed by Core. |
| P2 Local loop | `core-v0.3.2` | `tui-v0.3.1` | `gui-v0.1.0` | One local task completes with matching Core/TUI/GUI business facts under English, Simplified Chinese, and appearance variants. |

## Design Screens To Confirm Before Implementation

| Track | Primary screen | Supporting screens | Confirmation focus |
| --- | --- | --- | --- |
| TUI | `TUI/Viden - 统一原型 (TUI).html` | TUI design index, TUI component library, T1/T1c/T1d/T3/T4 pages | composer grammar, lane layout, pinned approval, statusbar, terminal color fallback |
| GUI | `GUI/Viden - 桌面驾驶舱 (GUI).html` | GUI design index, GUI component library, D11, D4, D2, D6 pages | desktop cockpit shell, project intake, lane creation, permission/recovery, settings entry |
| Core/design | `index.html`, `Core/Viden - Aurora 主题 (Core).html` | `docs/SPEC.md`, `docs/DESIGN-REF.md`, `tokens.css` | valid skins, modes, density, language switching, token ownership |

## File Structure

| Path | Responsibility |
| --- | --- |
| `crates/types/src/presentation.rs` | Shared presentation preference DTOs, enums, effective values, and validation errors. |
| `crates/core/src/presentation.rs` | Preference resolution, persistence intents, Core events, and snapshot exposure. |
| `crates/runtime/src/project.rs` or existing runtime config module | Route project/user config changes through runtime commands without bypassing Plan mode. |
| `docs/frontend-integration-contract.md` and `.zh-CN.md` | Public contract for preference commands, events, snapshots, and frontend obligations. |
| `apps/tui/src/tui/i18n.rs` and `apps/tui/i18n/*.json` | TUI locale catalogs, fallback, interpolation, and key parity tests. |
| `apps/tui/src/tui/preferences.rs` | TUI resolver for Core preferences plus terminal color-depth adaptation. |
| `apps/tui/src/tui/palette.rs` or generated module | Token-derived TUI palette with truecolor, ANSI 256, and ANSI 16 fallbacks. |
| `apps/gui/**` | GUI settings entry, token adapter, and desktop cockpit integration after framework gate. |
| `apps/gui/release-manifest.toml` and `apps/tui/release-manifest.toml` | Independent version metadata and required Core checkpoint. |

---

### Task 1: Core Presentation Preference Contract

**Files:**
- Create: `crates/types/src/presentation.rs`
- Modify: `crates/types/src/lib.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/client.rs` or the current frontend contract module
- Modify: `docs/frontend-integration-contract.md`
- Modify: `docs/frontend-integration-contract.zh-CN.md`

**Interfaces:**
- Produces: `UserPresentationPreferences`, `EffectivePresentationPreferences`, `PresentationPreferencePatch`, `PresentationPreferenceChanged`, and `PresentationPreferenceError`.
- Consumes: design decisions `D-I18N`, `D-SKIN`, `D-SETTINGS`, `D-A11Y`, and token definitions from `tokens.css`.

- [ ] **Step 1: Write failing DTO tests**

Create tests proving:

```rust
assert!(Skin::Aurora.supports(Mode::Light));
assert!(!Skin::Amber.supports(Mode::Light));
assert_eq!(Locale::resolve("system", Some("zh-CN")).id(), "zh-CN");
assert_eq!(Density::default(), Density::Compact);
```

- [ ] **Step 2: Run RED tests**

Run: `cargo test -p viden-types presentation -- --nocapture`

Expected: FAIL because the shared preference contract does not exist.

- [ ] **Step 3: Implement minimal typed contract**

Define enums for `LanguagePreference`, `LocaleId`, `Skin`, `ModePreference`, `EffectiveMode`, `Density`, `MotionPreference`, `TerminalColorCapability`, and accessibility flags. Unknown external values must deserialize into an explicit invalid/fallback path instead of silently becoming strings used by clients.

- [ ] **Step 4: Add Core commands and events**

Add command/event coverage for reading effective preferences and applying a patch. Plan mode must reject persisted preference mutation before writing config, while still allowing read-only discovery.

- [ ] **Step 5: Document bilingual contract**

Update both frontend integration contract files with the exact valid values, fallback order, event names, and frontend prohibition against private palettes.

- [ ] **Step 6: Verify and commit**

Run:

```bash
cargo test -p viden-types presentation
cargo test -p viden-core presentation
scripts/check-doc-pairs.sh docs/frontend-integration-contract.md docs/frontend-integration-contract.zh-CN.md
git diff --check
```

Commit: `feat(core): define presentation preference contract`

### Task 2: Shared Fixture Matrix For Language And Appearance

**Files:**
- Create or modify: shared frontend fixture corpus under the existing fixture location
- Modify: `crates/core` fixture tests
- Modify: `docs/parallel-development-plan.md`
- Modify: `docs/parallel-development-plan.zh-CN.md`

**Interfaces:**
- Consumes: Task 1 preference DTOs and Core event envelopes.
- Produces: fixtures for default, Chinese, light/dark, dark-only skin fallback, reduced motion, compact/regular/comfy density, and terminal color fallback.

- [ ] **Step 1: Write failing fixture parity tests**

Add fixture assertions for:

```text
en + aurora/dark + compact
zh-CN + aurora/dark + compact
en + ice/light + regular
zh-CN + mono/light + comfy
en + amber/light request -> amber/dark effective fallback
zh-CN + phosphor/light request -> phosphor/dark effective fallback
reduced motion
ansi16 terminal fallback
```

- [ ] **Step 2: Run RED tests**

Run: `cargo test -p viden-core frontend_preference_fixtures -- --nocapture`

Expected: FAIL until fixtures and expected snapshots exist.

- [ ] **Step 3: Add fixture events and expected facts**

Each fixture must include `schema_version`, preference source, effective values, diagnostic when fallback occurs, and enough runtime facts for both TUI and GUI to prove they render the same business state.

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo test -p viden-core frontend_preference_fixtures
scripts/check-doc-pairs.sh docs/parallel-development-plan.md docs/parallel-development-plan.zh-CN.md
git diff --check
```

Commit: `test(core): add presentation preference parity fixtures`

### Task 3: TUI 0.3.0 Appearance And Locale Slice

**Files:**
- Modify: `apps/tui/AGENTS.md`
- Modify: `apps/tui/release-manifest.toml`
- Create or modify: `apps/tui/src/tui/i18n.rs`
- Create or modify: `apps/tui/i18n/en.json`
- Create or modify: `apps/tui/i18n/zh-CN.json`
- Create or modify: `apps/tui/src/tui/preferences.rs`
- Create or modify: `apps/tui/src/tui/palette.rs`
- Modify: TUI render/statusbar/composer/settings modules as needed

**Interfaces:**
- Consumes: Core `EffectivePresentationPreferences` and shared fixtures.
- Produces: `tui-v0.3.0` with Core-backed language and appearance settings.

- [ ] **Step 1: Write failing TUI preference tests**

Tests must prove key parity, locale fallback, double-width Chinese layout stability, truecolor/ANSI fallback mapping, and rejection of private theme ids.

- [ ] **Step 2: Run RED tests**

Run: `cargo test -p viden-tui preference i18n palette -- --nocapture`

Expected: FAIL until the TUI consumes Core preferences.

- [ ] **Step 3: Implement Core-backed TUI resolver**

The TUI resolver reads the effective Core values and maps them to terminal styles. It may store terminal-only color capability detection locally, but language, skin, mode, density, and motion preference persistence must flow back through Core.

- [ ] **Step 4: Update user-visible controls**

Expose language and appearance through the TUI overlay/settings path. Labels, approval copy, status rows, and narrow-screen fallbacks must render in English and Simplified Chinese.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo test -p viden-tui
scripts/tui-regression.sh
scripts/tui-previews.sh
git diff --check
```

Commit: `feat(tui): consume core presentation preferences`

### Task 4: GUI 0.1.0 Appearance And Locale Slice

**Files:**
- Modify: `apps/gui/AGENTS.md`
- Create or modify: `apps/gui/release-manifest.toml`
- Modify: GUI token adapter and settings modules after the framework gate
- Modify: GUI cockpit, D11 intake, D4 lane creation, D2 permission slice, and D6 recovery surfaces as needed
- Modify: GUI screenshot/evidence scripts after framework selection

**Interfaces:**
- Consumes: Core `EffectivePresentationPreferences`, GUI component library, and desktop cockpit prototype.
- Produces: `gui-v0.1.0-beta.1` / `gui-v0.1.0` with settings-backed language and appearance.

- [ ] **Step 1: Confirm GUI entry path**

Open in order:

```text
docs/viden-design/Viden/index.html
docs/viden-design/Viden/GUI/Viden - 设计稿索引 (GUI).html
docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html
docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html
```

Record screenshots for the desktop cockpit, settings appearance area, project intake, lane creation, permission/decision slice, and recovery state before implementation.

- [ ] **Step 2: Write failing GUI preference tests**

Tests must prove GUI renders from Core preferences, exposes language and appearance controls, rejects invalid dark/light skin combinations, and does not ship a second palette registry.

- [ ] **Step 3: Implement GUI adapter**

Import or generate from shared tokens. GUI may map tokens to framework primitives, but valid skin/mode/density/motion values are exactly the Core contract values.

- [ ] **Step 4: Update desktop cockpit path**

The desktop cockpit is the P0 entry. D11, D4, D2, and D6 refine flows but must not become separate conflicting shells.

- [ ] **Step 5: Verify and commit**

Run the selected GUI framework tests, screenshot parity command, CJK IME/manual evidence checklist, keyboard-only evidence, and:

```bash
git diff --check
```

Commit: `feat(gui): consume core presentation preferences`

### Task 5: Integration And Release Manifests

**Files:**
- Modify: `apps/tui/release-manifest.toml`
- Modify: `apps/gui/release-manifest.toml`
- Modify: Core release manifest or changelog location
- Modify: `docs/superpowers/plans/2026-07-19-independent-release-plan-index.md`
- Modify: `docs/superpowers/plans/2026-07-19-independent-release-plan-index.zh-CN.md`

**Interfaces:**
- Consumes: Task 1 through Task 4 commits.
- Produces: integration report naming workspace candidate, Core version, TUI version, GUI version, Core checkpoint SHA, schema, capabilities, and skipped gates.

- [ ] **Step 1: Pin exact Core checkpoint**

Both frontend manifests must record the same 40-character Core checkpoint SHA and supported schema versions.

- [ ] **Step 2: Run fixed integration order**

Run Core gates first, then TUI fixture/render gates, then GUI fixture/render gates. Do not merge GUI before TUI evidence has passed against the same Core checkpoint.

- [ ] **Step 3: Verify full workspace or record scoped blocker**

Run:

```bash
cargo test --workspace --quiet
scripts/check-doc-pairs.sh
scripts/check-doc-links.sh
git diff --check
```

Expected: PASS, or a scoped blocker that names the exact crate/script and owner.

- [ ] **Step 4: Commit integration metadata**

Commit: `chore(release): pin independent frontend preference slice`

## Stop Condition

This plan is complete when Core, TUI, and GUI have independent version metadata, TUI and GUI consume one Core-owned presentation preference contract, English and Simplified Chinese key parity is tested, all valid skin/mode/density combinations are covered by fixtures or evidence, and integration reports name the exact Core checkpoint used by both frontends.
