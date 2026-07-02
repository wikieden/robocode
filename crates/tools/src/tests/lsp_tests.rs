use super::*;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
struct MockSemanticProvider;

impl SemanticToolProvider for MockSemanticProvider {
    fn diagnostics(&self, _cwd: &Path, path: &Path) -> Result<String, String> {
        Ok(format!("diagnostics for {}", path.display()))
    }

    fn symbols(&self, _cwd: &Path, path: &Path) -> Result<String, String> {
        Ok(format!("symbols for {}", path.display()))
    }

    fn references(
        &self,
        _cwd: &Path,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<String, String> {
        Ok(format!(
            "references for {}:{line}:{character}",
            path.display()
        ))
    }
}

#[test]
fn lsp_tool_specs_are_read_only() {
    let registry = ToolRegistry::builtin();
    for name in ["lsp_diagnostics", "lsp_symbols", "lsp_references"] {
        let spec = registry.spec(name).unwrap();
        assert!(!spec.is_mutating, "{name} must be read-only");
    }
}

#[test]
fn lsp_diagnostics_requires_semantic_provider() {
    let ctx = ToolExecutionContext {
        cwd: temp_dir("lsp_missing_provider"),
        semantic: None,
    };
    let mut input = ToolInput::new();
    input.insert("path".into(), "src/lib.rs".into());

    let error = ToolRegistry::builtin()
        .execute(
            &ToolCall {
                id: "tool_lsp_diagnostics".into(),
                name: "lsp_diagnostics".into(),
                input,
            },
            &ctx,
        )
        .unwrap_err();
    assert_eq!(error, "LSP semantic provider is not available");
}

#[test]
fn lsp_references_validates_line_and_character() {
    let ctx = ToolExecutionContext {
        cwd: temp_dir("lsp_bad_position"),
        semantic: Some(Arc::new(MockSemanticProvider)),
    };
    let mut input = ToolInput::new();
    input.insert("path".into(), "src/lib.rs".into());
    input.insert("line".into(), "abc".into());
    input.insert("character".into(), "0".into());

    let error = ToolRegistry::builtin()
        .execute(
            &ToolCall {
                id: "tool_lsp_references".into(),
                name: "lsp_references".into(),
                input,
            },
            &ctx,
        )
        .unwrap_err();
    assert_eq!(error, "lsp_references requires numeric `line`");
}

#[test]
fn lsp_symbols_returns_mock_semantic_output() {
    let ctx = ToolExecutionContext {
        cwd: temp_dir("lsp_symbols"),
        semantic: Some(Arc::new(MockSemanticProvider)),
    };
    let mut input = ToolInput::new();
    input.insert("path".into(), "src/lib.rs".into());

    let result = ToolRegistry::builtin()
        .execute(
            &ToolCall {
                id: "tool_lsp_symbols".into(),
                name: "lsp_symbols".into(),
                input,
            },
            &ctx,
        )
        .unwrap();
    assert!(result.success);
    assert_eq!(result.output, "symbols for src/lib.rs");
}
