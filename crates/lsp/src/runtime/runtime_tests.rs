use std::env;
use std::path::Path;

use super::*;
use crate::LspServerRegistry;

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("viden_lsp_{name}_{}", viden_types::fresh_id("tmp")));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_fake_server(
    workdir: &Path,
    stats_path: Option<&Path>,
    exit_after_symbol: bool,
) -> PathBuf {
    let script_path = workdir.join("fake_lsp_server.py");
    let counter_path = stats_path
        .map(|path| format!("STATS_PATH = {:?}\n", path.display().to_string()))
        .unwrap_or_else(|| "STATS_PATH = None\n".to_string());
    let exit_after_symbol_line = if exit_after_symbol {
        "EXIT_AFTER_SYMBOL = True\n"
    } else {
        "EXIT_AFTER_SYMBOL = False\n"
    };
    let script_template = r#"__COUNTER_PATH____EXIT_AFTER_SYMBOL__
import json
import sys
from pathlib import Path

def update_stats(key):
    if not STATS_PATH:
        return
    stats_file = Path(STATS_PATH)
    if stats_file.exists():
        stats = json.loads(stats_file.read_text())
    else:
        stats = {}
    stats[key] = stats.get(key, 0) + 1
    stats_file.write_text(json.dumps(stats))

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line == b"\r\n":
            break
        key, value = line.decode("utf-8").split(":", 1)
        headers[key.lower()] = value.strip()
    length = int(headers["content-length"])
    body = sys.stdin.buffer.read(length)
    return json.loads(body.decode("utf-8"))

def send(payload):
    body = json.dumps(payload).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        update_stats("initialize")
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}})
    elif method == "initialized":
        continue
    elif method == "textDocument/didOpen":
        update_stats("didOpen")
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 8}
                    },
                    "severity": 1,
                    "source": "fake-lsp",
                    "code": "E100",
                    "message": "fake diagnostic"
                }]
            }
        })
    elif method == "textDocument/didChange":
        update_stats("didChange")
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": [{
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 8}
                    },
                    "severity": 2,
                    "source": "fake-lsp-change",
                    "code": "E200",
                    "message": "changed diagnostic"
                }]
            }
        })
    elif method == "textDocument/documentSymbol":
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": [{
                "name": "main",
                "kind": 12,
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 2, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 3},
                    "end": {"line": 0, "character": 7}
                },
                "children": [{
                    "name": "value",
                    "kind": 13,
                    "range": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 9}
                    },
                    "selectionRange": {
                        "start": {"line": 1, "character": 4},
                        "end": {"line": 1, "character": 9}
                    }
                }]
            }]
        })
        if EXIT_AFTER_SYMBOL:
            break
    elif method == "textDocument/references":
        uri = message["params"]["textDocument"]["uri"]
        send({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": [{
                "uri": uri,
                "range": {
                    "start": {"line": 1, "character": 4},
                    "end": {"line": 1, "character": 9}
                }
            }]
        })
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": message["id"], "result": None})
    elif method == "exit":
        break
"#;
    let script = script_template
        .replace("__COUNTER_PATH__", &counter_path)
        .replace("__EXIT_AFTER_SYMBOL__", exit_after_symbol_line);
    fs::write(&script_path, script).unwrap();
    script_path
}

fn fake_registry(workdir: &Path) -> LspServerRegistry {
    let script_path = write_fake_server(workdir, None, false);
    LspServerRegistry::new(vec![LspServerConfig {
        id: "fake-rust".to_string(),
        command: env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
        args: vec![script_path.to_string_lossy().to_string()],
        file_extensions: vec!["rs".to_string()],
    }])
}

fn fake_registry_with_counter(workdir: &Path, counter: &Path) -> LspServerRegistry {
    let script_path = write_fake_server(workdir, Some(counter), false);
    LspServerRegistry::new(vec![LspServerConfig {
        id: "fake-rust".to_string(),
        command: env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
        args: vec![script_path.to_string_lossy().to_string()],
        file_extensions: vec!["rs".to_string()],
    }])
}

fn fake_registry_exits_after_symbol(workdir: &Path, counter: &Path) -> LspServerRegistry {
    let script_path = write_fake_server(workdir, Some(counter), true);
    LspServerRegistry::new(vec![LspServerConfig {
        id: "fake-rust".to_string(),
        command: env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string()),
        args: vec![script_path.to_string_lossy().to_string()],
        file_extensions: vec!["rs".to_string()],
    }])
}

#[test]
fn status_reports_configured_servers() {
    let runtime = LspRuntime::new(LspServerRegistry::default());
    let status = runtime.status();
    assert_eq!(status.configured_servers, vec!["rust-analyzer"]);
    assert!(status.running_servers.is_empty());
    assert_eq!(status.cached_sessions, 0);
    assert_eq!(status.open_documents, 0);
    assert!(status.last_error.is_none());
}

#[test]
fn diagnostics_returns_clean_error_for_unconfigured_path() {
    let runtime = LspRuntime::new(LspServerRegistry::default());
    let error = runtime
        .diagnostics(Path::new("."), Path::new("README.md"))
        .unwrap_err();
    assert_eq!("No configured language server for README.md", error);
}

#[test]
fn diagnostics_collect_publish_diagnostics_from_language_server() {
    let cwd = temp_dir("diagnostics");
    let source = cwd.join("sample.rs");
    fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
    let runtime = LspRuntime::new(fake_registry(&cwd));

    let diagnostics = runtime.diagnostics(&cwd, Path::new("sample.rs")).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].source.as_deref(), Some("fake-lsp"));
    assert_eq!(diagnostics[0].message, "fake diagnostic");
}

#[test]
fn symbols_query_language_server_and_flatten_children() {
    let cwd = temp_dir("symbols");
    let source = cwd.join("sample.rs");
    fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
    let runtime = LspRuntime::new(fake_registry(&cwd));

    let symbols = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].name, "main");
    assert_eq!(symbols[1].container_name.as_deref(), Some("main"));
    assert_eq!(symbols[1].name, "value");
}

#[test]
fn references_query_language_server_and_parse_locations() {
    let cwd = temp_dir("references");
    let source = cwd.join("sample.rs");
    fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
    let runtime = LspRuntime::new(fake_registry(&cwd));

    let locations = runtime
        .references(
            &cwd,
            Path::new("sample.rs"),
            LspPosition {
                line: 1,
                character: 4,
            },
        )
        .unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].range.start.line, 1);
    assert_eq!(locations[0].range.start.character, 4);
}

#[test]
fn runtime_reuses_initialized_session_for_multiple_queries() {
    let cwd = temp_dir("reuse");
    let source = cwd.join("sample.rs");
    let counter = cwd.join("stats.json");
    fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
    let runtime = LspRuntime::new(fake_registry_with_counter(&cwd, &counter));

    let _ = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();
    let _ = runtime
        .references(
            &cwd,
            Path::new("sample.rs"),
            LspPosition {
                line: 1,
                character: 4,
            },
        )
        .unwrap();

    let status = runtime.status();
    assert_eq!(status.running_servers, vec!["fake-rust"]);
    assert_eq!(status.cached_sessions, 1);
    assert_eq!(status.open_documents, 1);
    let stats: Value = serde_json::from_str(&fs::read_to_string(counter).unwrap()).unwrap();
    assert_eq!(stats["initialize"], 1);
    assert_eq!(stats["didOpen"], 1);
}

#[test]
fn runtime_uses_did_change_for_repeated_document_sync() {
    let cwd = temp_dir("did_change");
    let source = cwd.join("sample.rs");
    let counter = cwd.join("stats.json");
    fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
    let runtime = LspRuntime::new(fake_registry_with_counter(&cwd, &counter));

    let first = runtime.diagnostics(&cwd, Path::new("sample.rs")).unwrap();
    fs::write(&source, "fn main() {\n    let value = 2;\n}\n").unwrap();
    let second = runtime.diagnostics(&cwd, Path::new("sample.rs")).unwrap();

    assert_eq!(first[0].source.as_deref(), Some("fake-lsp"));
    assert_eq!(second[0].source.as_deref(), Some("fake-lsp-change"));
    let stats: Value = serde_json::from_str(&fs::read_to_string(counter).unwrap()).unwrap();
    assert_eq!(stats["initialize"], 1);
    assert_eq!(stats["didOpen"], 1);
    assert_eq!(stats["didChange"], 1);
}

#[test]
fn runtime_restarts_dead_session_before_reuse() {
    let cwd = temp_dir("restart_dead");
    let source = cwd.join("sample.rs");
    let counter = cwd.join("stats.json");
    fs::write(&source, "fn main() {\n    let value = 1;\n}\n").unwrap();
    let runtime = LspRuntime::new(fake_registry_exits_after_symbol(&cwd, &counter));

    let first = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();
    let second = runtime.symbols(&cwd, Path::new("sample.rs")).unwrap();

    assert_eq!(first[0].name, "main");
    assert_eq!(second[0].name, "main");
    let stats: Value = serde_json::from_str(&fs::read_to_string(counter).unwrap()).unwrap();
    assert_eq!(stats["initialize"], 2);
    assert_eq!(stats["didOpen"], 2);
}

#[test]
fn parse_locations_supports_location_links_and_dedups() {
    let payload = serde_json::json!([
        {
            "targetUri": "file:///tmp/project/src/lib.rs",
            "targetSelectionRange": {
                "start": {"line": 3, "character": 2},
                "end": {"line": 3, "character": 6}
            }
        },
        {
            "uri": "file:///tmp/project/src/lib.rs",
            "range": {
                "start": {"line": 3, "character": 2},
                "end": {"line": 3, "character": 6}
            }
        },
        {
            "uri": "file:///tmp/project/src/main.rs",
            "range": {
                "start": {"line": 1, "character": 0},
                "end": {"line": 1, "character": 4}
            }
        }
    ]);

    let locations = parse_locations(&payload, "file:///tmp/fallback.rs").unwrap();
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0].path, "/tmp/project/src/lib.rs");
    assert_eq!(locations[0].range.start.line, 3);
    assert_eq!(locations[1].path, "/tmp/project/src/main.rs");
}
