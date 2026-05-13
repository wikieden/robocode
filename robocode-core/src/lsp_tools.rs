use std::collections::BTreeMap;
use std::sync::Arc;

use robocode_lsp::{LspRuntime, SemanticProvider};
use robocode_tools::SemanticToolProvider;
use robocode_types::{LspDiagnostic, LspLocation, LspPosition, LspSymbol};

use crate::presentation::{join_lines, render_section_title, render_subsection_title};

pub(crate) struct LspToolAdapter {
    pub(crate) runtime: Arc<LspRuntime>,
}

impl SemanticToolProvider for LspToolAdapter {
    fn diagnostics(&self, cwd: &std::path::Path, path: &std::path::Path) -> Result<String, String> {
        self.runtime
            .diagnostics(cwd, path)
            .map(|diagnostics| render_lsp_diagnostics(cwd, &diagnostics))
    }

    fn symbols(&self, cwd: &std::path::Path, path: &std::path::Path) -> Result<String, String> {
        self.runtime
            .symbols(cwd, path)
            .map(|symbols| render_lsp_symbols(cwd, &symbols))
    }

    fn references(
        &self,
        cwd: &std::path::Path,
        path: &std::path::Path,
        line: u32,
        character: u32,
    ) -> Result<String, String> {
        self.runtime
            .references(cwd, path, LspPosition { line, character })
            .map(|locations| render_lsp_locations(cwd, &locations))
    }
}

pub(crate) fn parse_lsp_position_arg(raw: Option<&String>, name: &str) -> Result<u32, String> {
    raw.ok_or_else(|| "Usage: /lsp references <path> <line> <character>".to_string())?
        .parse::<u32>()
        .map_err(|_| {
            format!(
                "Usage: /lsp references <path> <line> <character>; line and character must be zero-based integers (`{name}` was invalid)"
            )
        })
}

pub(crate) fn render_lsp_diagnostics(
    cwd: &std::path::Path,
    diagnostics: &[LspDiagnostic],
) -> String {
    if diagnostics.is_empty() {
        return "LSP diagnostics:\n  <none>".to_string();
    }

    let mut grouped = BTreeMap::<String, Vec<&LspDiagnostic>>::new();
    for diagnostic in diagnostics {
        grouped
            .entry(render_lsp_path(cwd, &diagnostic.path))
            .or_default()
            .push(diagnostic);
    }

    let mut lines = vec![
        render_section_title("LSP diagnostics")
            .trim_end()
            .to_string(),
    ];

    for (path, entries) in grouped {
        lines.push(render_subsection_title(&path));
        for diagnostic in entries {
            let source = diagnostic
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
                .unwrap_or_else(|| "unknown".to_string());

            lines.push(format!(
                "  {}:{} {} [{}] {}",
                diagnostic.range.start.line,
                diagnostic.range.start.character,
                severity_label(diagnostic.severity),
                source,
                diagnostic.message
            ));
        }
    }

    join_lines(&lines)
}

pub(crate) fn render_lsp_symbols(cwd: &std::path::Path, symbols: &[LspSymbol]) -> String {
    if symbols.is_empty() {
        return "LSP symbols:\n  <none>".to_string();
    }

    let mut grouped = BTreeMap::<String, Vec<&LspSymbol>>::new();
    for symbol in symbols {
        grouped
            .entry(render_lsp_path(cwd, &symbol.path))
            .or_default()
            .push(symbol);
    }

    let mut lines = vec![render_section_title("LSP symbols").trim_end().to_string()];
    for (path, entries) in grouped {
        lines.push(render_subsection_title(&path));
        for symbol in entries {
            lines.push(format!(
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
            ));
        }
    }

    join_lines(&lines)
}

pub(crate) fn render_lsp_locations(cwd: &std::path::Path, locations: &[LspLocation]) -> String {
    if locations.is_empty() {
        return "LSP references:\n  <none>".to_string();
    }
    let mut lines = vec![
        render_section_title("LSP references")
            .trim_end()
            .to_string(),
    ];
    for location in locations {
        lines.push(format!(
            "  {}:{}:{}",
            render_lsp_path(cwd, &location.path),
            location.range.start.line,
            location.range.start.character
        ));
    }
    join_lines(&lines)
}

fn render_lsp_path(cwd: &std::path::Path, path: &str) -> String {
    let path_buf = std::path::Path::new(path);
    path_buf
        .strip_prefix(cwd)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}

fn severity_label(severity: Option<u8>) -> &'static str {
    match severity {
        Some(1) => "error",
        Some(2) => "warning",
        Some(3) => "info",
        Some(4) => "hint",
        _ => "unknown",
    }
}

fn symbol_kind_label(kind: u32) -> &'static str {
    match kind {
        5 => "class",
        6 => "method",
        10 => "enum",
        11 => "interface",
        12 => "function",
        13 => "variable",
        19 => "namespace",
        22 => "field",
        23 => "struct",
        _ => "symbol",
    }
}
