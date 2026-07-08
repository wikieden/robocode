use crate::lsp_tools::{render_lsp_diagnostics, render_lsp_locations, render_lsp_symbols};
use viden_types::{LspDiagnostic, LspLocation, LspPosition, LspRange, LspSymbol};

use super::temp_dir;

#[test]
fn render_lsp_symbols_uses_relative_paths_and_kind_labels() {
    let cwd = temp_dir("lsp_render_symbols");
    let rendered = render_lsp_symbols(
        &cwd,
        &[LspSymbol {
            name: "main".to_string(),
            kind: 12,
            path: cwd.join("src/lib.rs").display().to_string(),
            range: LspRange {
                start: LspPosition {
                    line: 3,
                    character: 1,
                },
                end: LspPosition {
                    line: 4,
                    character: 1,
                },
            },
            selection_range: None,
            container_name: Some("impl SessionEngine".to_string()),
        }],
    );
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  main [function] 3:1 in impl SessionEngine"));
}

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
                    start: LspPosition {
                        line: 3,
                        character: 1,
                    },
                    end: LspPosition {
                        line: 4,
                        character: 1,
                    },
                },
                selection_range: None,
                container_name: None,
            },
            LspSymbol {
                name: "value".to_string(),
                kind: 13,
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 4,
                        character: 5,
                    },
                    end: LspPosition {
                        line: 4,
                        character: 10,
                    },
                },
                selection_range: None,
                container_name: Some("main".to_string()),
            },
            LspSymbol {
                name: "run".to_string(),
                kind: 12,
                path: cwd.join("src/engine.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 8,
                        character: 2,
                    },
                    end: LspPosition {
                        line: 9,
                        character: 2,
                    },
                },
                selection_range: None,
                container_name: Some("Engine".to_string()),
            },
        ],
    );

    assert_eq!(
        rendered,
        [
            "LSP symbols:",
            "src/engine.rs:",
            "  run [function] 8:2 in Engine",
            "src/lib.rs:",
            "  main [function] 3:1",
            "  value [variable] 4:5 in main",
        ]
        .join("\n")
    );
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
                    start: LspPosition {
                        line: 4,
                        character: 5,
                    },
                    end: LspPosition {
                        line: 4,
                        character: 9,
                    },
                },
            },
            LspLocation {
                path: cwd.join("src/engine.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 18,
                        character: 9,
                    },
                    end: LspPosition {
                        line: 18,
                        character: 13,
                    },
                },
            },
        ],
    );

    assert_eq!(
        rendered,
        [
            "LSP references:",
            "  src/lib.rs:4:5",
            "  src/engine.rs:18:9",
        ]
        .join("\n")
    );
}

#[test]
fn render_lsp_diagnostics_includes_severity_source_and_code() {
    let cwd = temp_dir("lsp_render_diagnostics");
    let rendered = render_lsp_diagnostics(
        &cwd,
        &[LspDiagnostic {
            path: cwd.join("src/lib.rs").display().to_string(),
            range: LspRange {
                start: LspPosition {
                    line: 7,
                    character: 2,
                },
                end: LspPosition {
                    line: 7,
                    character: 6,
                },
            },
            severity: Some(2),
            source: Some("rust-analyzer".to_string()),
            code: Some("E0308".to_string()),
            message: "mismatched types".to_string(),
        }],
    );
    assert!(rendered.contains("src/lib.rs:"));
    assert!(rendered.contains("  7:2 warning [rust-analyzer/E0308] mismatched types"));
}

#[test]
fn render_lsp_diagnostics_groups_entries_by_file() {
    let cwd = temp_dir("lsp_render_diagnostics_grouped");
    let rendered = render_lsp_diagnostics(
        &cwd,
        &[
            LspDiagnostic {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 2,
                        character: 4,
                    },
                    end: LspPosition {
                        line: 2,
                        character: 8,
                    },
                },
                severity: Some(1),
                source: Some("rust-analyzer".to_string()),
                code: Some("E0001".to_string()),
                message: "first issue".to_string(),
            },
            LspDiagnostic {
                path: cwd.join("src/lib.rs").display().to_string(),
                range: LspRange {
                    start: LspPosition {
                        line: 7,
                        character: 1,
                    },
                    end: LspPosition {
                        line: 7,
                        character: 5,
                    },
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
