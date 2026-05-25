use std::collections::BTreeMap;
use std::sync::{Arc, mpsc};
use std::thread;

use robocode_lsp::{LspRuntime, SemanticProvider};
use robocode_tools::SemanticToolProvider;
use robocode_types::{LspDiagnostic, LspLocation, LspPosition, LspSymbol};

use crate::SessionEngine;
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

impl SessionEngine {
    pub fn spawn_lsp_diagnostics_snapshot(
        &self,
        paths: Vec<String>,
    ) -> mpsc::Receiver<Option<String>> {
        let cwd = self.cwd.clone();
        let runtime = Arc::clone(&self.lsp_runtime);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let rendered = render_lsp_diagnostics_snapshot(runtime, &cwd, &paths);
            let _ = sender.send(rendered);
        });
        receiver
    }

    pub(super) fn handle_lsp_command(&self, args: &[String]) -> Result<String, String> {
        let Some(subcommand) = args.first().map(String::as_str) else {
            return Ok(self.render_lsp_help());
        };
        match subcommand {
            "help" => Ok(self.render_lsp_help()),
            "status" => Ok(self.render_lsp_status()),
            "diagnostics" => {
                let path = args
                    .get(1)
                    .ok_or_else(|| "Usage: /lsp diagnostics <path>".to_string())?;
                match self
                    .lsp_runtime
                    .diagnostics(&self.cwd, std::path::Path::new(path))
                {
                    Ok(diagnostics) => Ok(render_lsp_diagnostics(&self.cwd, &diagnostics)),
                    Err(error) => Ok(format!("LSP error: {error}")),
                }
            }
            "symbols" => {
                let path = args
                    .get(1)
                    .ok_or_else(|| "Usage: /lsp symbols <path>".to_string())?;
                match self
                    .lsp_runtime
                    .symbols(&self.cwd, std::path::Path::new(path))
                {
                    Ok(symbols) => Ok(render_lsp_symbols(&self.cwd, &symbols)),
                    Err(error) => Ok(format!("LSP error: {error}")),
                }
            }
            "references" => {
                let path = args.get(1).ok_or_else(|| {
                    "Usage: /lsp references <path> <line> <character>".to_string()
                })?;
                let line = parse_lsp_position_arg(args.get(2), "line")?;
                let character = parse_lsp_position_arg(args.get(3), "character")?;
                match self.lsp_runtime.references(
                    &self.cwd,
                    std::path::Path::new(path),
                    LspPosition { line, character },
                ) {
                    Ok(locations) => Ok(render_lsp_locations(&self.cwd, &locations)),
                    Err(error) => Ok(format!("LSP error: {error}")),
                }
            }
            _ => Ok(format!(
                "Unknown LSP subcommand `{subcommand}`.\n\n{}",
                self.render_lsp_help()
            )),
        }
    }

    fn render_lsp_help(&self) -> String {
        [
            "LSP commands:",
            "  /lsp status",
            "  /lsp diagnostics <path>",
            "  /lsp symbols <path>",
            "  /lsp references <path> <line> <character>",
            "",
            "Positions are zero-based LSP line and character offsets.",
        ]
        .join("\n")
    }

    fn render_lsp_status(&self) -> String {
        let status = self.lsp_runtime.status();
        let configured = if status.configured_servers.is_empty() {
            "<none>".to_string()
        } else {
            status.configured_servers.join(", ")
        };
        let running = if status.running_servers.is_empty() {
            "<none>".to_string()
        } else {
            status.running_servers.join(", ")
        };
        [
            "LSP status:".to_string(),
            format!("  configured: {configured}"),
            format!("  running: {running}"),
            format!("  cached_sessions: {}", status.cached_sessions),
            format!("  open_documents: {}", status.open_documents),
            format!(
                "  last_error: {}",
                status.last_error.unwrap_or_else(|| "<none>".to_string())
            ),
        ]
        .join("\n")
    }
}

fn render_lsp_diagnostics_snapshot(
    runtime: Arc<LspRuntime>,
    cwd: &std::path::Path,
    paths: &[String],
) -> Option<String> {
    let mut had_success = false;
    let mut diagnostics = Vec::new();
    for path in paths {
        match runtime.diagnostics(cwd, std::path::Path::new(path)) {
            Ok(mut path_diagnostics) => {
                had_success = true;
                diagnostics.append(&mut path_diagnostics);
            }
            Err(_) => continue,
        }
    }
    had_success.then(|| render_lsp_diagnostics(cwd, &diagnostics))
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
