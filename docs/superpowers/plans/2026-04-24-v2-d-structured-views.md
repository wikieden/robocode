# V2-D Structured Views Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first structured terminal presentation slice for RoboCode by improving LSP diagnostics, symbols, and references rendering without changing command semantics or runtime boundaries.

**Architecture:** Keep all behavior inside the existing `robocode-core` command path. Add a small `presentation.rs` helper module for text layout primitives, then update the existing LSP renderer functions in `robocode-core/src/lib.rs` to group output by file and render concise, stable lines. `robocode-cli` remains unchanged in this slice.

**Tech Stack:** Rust 2024 workspace, `robocode-core`, existing unit tests, plain terminal text output

---

## File Map

Create:

- `robocode-core/src/presentation.rs`
- `docs/superpowers/plans/2026-04-24-v2-d-structured-views.md`

Modify:

- `robocode-core/src/lib.rs`

No changes:

- `robocode-cli/src/main.rs`
- `robocode-lsp/*`
- `robocode-tools/*`
- `robocode-session/*`
- `robocode-permissions/*`

## Task 1: Add Presentation Module Skeleton

**Files:**
- Create: `robocode-core/src/presentation.rs`
- Modify: `robocode-core/src/lib.rs`

- [ ] **Step 1: Write the failing presentation module test**

Add to `robocode-core/src/presentation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_section_title_adds_consistent_header_spacing() {
        let rendered = render_section_title("Diagnostics");
        assert_eq!(rendered, "Diagnostics:\n");
    }

    #[test]
    fn join_lines_preserves_line_order() {
        let rendered = join_lines(&[
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
        ]);
        assert_eq!(rendered, "alpha\nbeta\ngamma");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p robocode-core render_section_title_adds_consistent_header_spacing
```

Expected: FAIL because `presentation.rs` is not wired into `robocode-core` yet and the helpers do not exist.

- [ ] **Step 3: Write minimal implementation**

Create `robocode-core/src/presentation.rs`:

```rust
pub fn render_section_title(title: &str) -> String {
    format!("{title}:\n")
}

pub fn join_lines(lines: &[String]) -> String {
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_section_title_adds_consistent_header_spacing() {
        let rendered = render_section_title("Diagnostics");
        assert_eq!(rendered, "Diagnostics:\n");
    }

    #[test]
    fn join_lines_preserves_line_order() {
        let rendered = join_lines(&[
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
        ]);
        assert_eq!(rendered, "alpha\nbeta\ngamma");
    }
}
```

Expose the module near the top of `robocode-core/src/lib.rs`:

```rust
mod presentation;
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p robocode-core presentation
```

Expected: PASS with the two presentation helper tests succeeding.

- [ ] **Step 5: Commit**

```bash
git add robocode-core/src/lib.rs robocode-core/src/presentation.rs
git commit -m "Introduce a presentation helper module for structured views"
```

## Task 2: Group Diagnostics by File

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-core/src/presentation.rs`

- [ ] **Step 1: Write the failing grouped diagnostics test**

Add to the existing `#[cfg(test)]` module in `robocode-core/src/lib.rs`:

```rust
#[test]
fn render_lsp_diagnostics_groups_entries_by_file() {
    let cwd = temp_dir("lsp_render_diagnostics_grouped");
    let rendered = render_lsp_diagnostics(
        &cwd,
        &[
            LspDiagnostic {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition { line: 2, character: 4 },
                    end: LspPosition { line: 2, character: 8 },
                },
                severity: Some(1),
                source: Some("rust-analyzer".to_string()),
                code: Some("E0001".to_string()),
                message: "first issue".to_string(),
            },
            LspDiagnostic {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition { line: 7, character: 1 },
                    end: LspPosition { line: 7, character: 5 },
                },
                severity: Some(2),
                source: Some("clippy".to_string()),
                code: None,
                message: "second issue".to_string(),
            },
        ],
    );

    assert!(rendered.contains("LSP diagnostics:"));
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  2:4 error [rust-analyzer/E0001] first issue"));
    assert!(rendered.contains("  7:1 warning [clippy] second issue"));
}
```

- [ ] **Step 2: Run test to verify current output fails**

Run:

```bash
cargo test -p robocode-core render_lsp_diagnostics_groups_entries_by_file
```

Expected: FAIL because the current renderer prints each diagnostic as a flat line with the file path repeated inline.

- [ ] **Step 3: Write minimal implementation**

Add helpers to `robocode-core/src/presentation.rs`:

```rust
pub fn render_subsection_title(title: &str) -> String {
    format!("{title}:")
}
```

Update `render_lsp_diagnostics` in `robocode-core/src/lib.rs` so it:

- starts with `render_section_title("LSP diagnostics").trim_end().to_string()`
- groups diagnostics by `render_lsp_path(cwd, &diagnostic.path)`
- emits one file header per group using `render_subsection_title`
- renders one indented line per diagnostic in this shape:

```rust
format!(
    "  {}:{} {} [{}] {}",
    diagnostic.range.start.line,
    diagnostic.range.start.character,
    severity_label(diagnostic.severity),
    diagnostic
        .source
        .as_ref()
        .map(|source| {
            diagnostic
                .code
                .as_ref()
                .map(|code| format!("{source}/{code}"))
                .unwrap_or_else(|| source.clone())
        })
        .or_else(|| diagnostic.code.as_ref().cloned())
        .unwrap_or_else(|| "unknown".to_string()),
    diagnostic.message
)
```

Preserve the empty case:

```rust
"LSP diagnostics:\n  <none>".to_string()
```

- [ ] **Step 4: Run diagnostics renderer tests**

Run:

```bash
cargo test -p robocode-core render_lsp_diagnostics
```

Expected: PASS for both the existing severity/source/code test and the new grouped-by-file test.

- [ ] **Step 5: Commit**

```bash
git add robocode-core/src/lib.rs robocode-core/src/presentation.rs
git commit -m "Group LSP diagnostics into structured file sections"
```

## Task 3: Make Symbols and References Scan Cleanly

**Files:**
- Modify: `robocode-core/src/lib.rs`
- Modify: `robocode-core/src/presentation.rs`

- [ ] **Step 1: Write failing tests for grouped symbols and compact references**

Add to the existing tests in `robocode-core/src/lib.rs`:

```rust
#[test]
fn render_lsp_symbols_groups_entries_under_file_headers() {
    let cwd = temp_dir("lsp_render_symbols_grouped");
    let rendered = render_lsp_symbols(
        &cwd,
        &[
            LspSymbol {
                name: "main".to_string(),
                kind: 12,
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition { line: 3, character: 1 },
                    end: LspPosition { line: 4, character: 1 },
                },
                selection_range: None,
                container_name: None,
            },
            LspSymbol {
                name: "value".to_string(),
                kind: 13,
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition { line: 4, character: 5 },
                    end: LspPosition { line: 4, character: 10 },
                },
                selection_range: None,
                container_name: Some("main".to_string()),
            },
        ],
    );

    assert!(rendered.contains("LSP symbols:"));
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  main [function] 3:1"));
    assert!(rendered.contains("  value [variable] 4:5 in main"));
}

#[test]
fn render_lsp_locations_keeps_relative_sorted_lines() {
    let cwd = temp_dir("lsp_render_locations_grouped");
    let rendered = render_lsp_locations(
        &cwd,
        &[
            LspLocation {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition { line: 4, character: 5 },
                    end: LspPosition { line: 4, character: 9 },
                },
            },
            LspLocation {
                path: cwd.join("src/engine.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition { line: 18, character: 9 },
                    end: LspPosition { line: 18, character: 13 },
                },
            },
        ],
    );

    assert!(rendered.contains("LSP references:"));
    assert!(rendered.contains("  src/lib.rs:4:5"));
    assert!(rendered.contains("  src/engine.rs:18:9"));
}
```

- [ ] **Step 2: Run tests to verify current output fails at least one new assertion**

Run:

```bash
cargo test -p robocode-core render_lsp_symbols_groups_entries_under_file_headers
cargo test -p robocode-core render_lsp_locations_keeps_relative_sorted_lines
```

Expected: FAIL because symbols currently render path inline per line and references do not use the target grouped shape.

- [ ] **Step 3: Write minimal implementation**

Update `render_lsp_symbols` in `robocode-core/src/lib.rs` so it:

- groups symbols by relative file path
- emits one file header per group
- renders entries as:

```rust
format!(
    "  {} [{}] {}:{}{}",
    symbol.name,
    symbol_kind_label(symbol.kind),
    symbol.range.start.line,
    symbol.range.start.character,
    symbol
        .container_name
        .as_ref()
        .map(|container| format!(" in {container}"))
        .unwrap_or_default()
)
```

Update `render_lsp_locations` in `robocode-core/src/lib.rs` to render:

```rust
format!(
    "  {}:{}:{}",
    render_lsp_path(cwd, &location.path),
    location.range.start.line,
    location.range.start.character
)
```

using `presentation::join_lines` for the final assembly.

- [ ] **Step 4: Run focused and full crate verification**

Run:

```bash
cargo test -p robocode-core render_lsp_
cargo test -p robocode-core
```

Expected: PASS, including existing LSP command tests and the new grouped rendering assertions.

- [ ] **Step 5: Run workspace regression and commit**

Run:

```bash
cargo test --workspace --quiet
```

Expected: PASS.

Then commit:

```bash
git add robocode-core/src/lib.rs robocode-core/src/presentation.rs
git commit -m "Render LSP results as structured terminal views"
```

## Self-Review

Spec coverage:

- `presentation.rs` module boundary: covered in Task 1
- grouped diagnostics: covered in Task 2
- readable symbols and compact references: covered in Task 3
- no CLI, tool, or transcript boundary changes: preserved by file scope

Placeholder scan:

- no unfinished markers or deferred code steps remain in the executable tasks

Type consistency:

- all helper names referenced in later tasks are defined in Task 1 or Task 2
- all modified functions already exist in `robocode-core/src/lib.rs`
