use std::path::PathBuf;
use std::sync::Arc;

use robocode_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, SemanticToolProvider, ToolExecutionContext, ToolExecutionOutput};

pub(crate) struct LspDiagnosticsTool;
pub(crate) struct LspSymbolsTool;
pub(crate) struct LspReferencesTool;

impl BuiltinTool for LspDiagnosticsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "lsp_diagnostics".to_string(),
            description: "Read diagnostics for a source file from the semantic provider"
                .to_string(),
            is_mutating: false,
            input_schema_hint: "path=file".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let path = required_raw_path(input)?;
        let output = semantic_provider(ctx)?.diagnostics(&ctx.cwd, &path)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
            exit_code: None,
        })
    }
}

impl BuiltinTool for LspSymbolsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "lsp_symbols".to_string(),
            description: "Read document symbols for a source file from the semantic provider"
                .to_string(),
            is_mutating: false,
            input_schema_hint: "path=file".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let path = required_raw_path(input)?;
        let output = semantic_provider(ctx)?.symbols(&ctx.cwd, &path)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
            exit_code: None,
        })
    }
}

impl BuiltinTool for LspReferencesTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "lsp_references".to_string(),
            description: "Read references for a source location from the semantic provider"
                .to_string(),
            is_mutating: false,
            input_schema_hint: "path=file line=0 character=0".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let path = required_raw_path(input)?;
        let line = parse_required_u32(input, "line", "lsp_references")?;
        let character = parse_required_u32(input, "character", "lsp_references")?;
        let output = semantic_provider(ctx)?.references(&ctx.cwd, &path, line, character)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
            exit_code: None,
        })
    }
}

fn required_raw_path(input: &ToolInput) -> Result<PathBuf, String> {
    input
        .get("path")
        .map(PathBuf::from)
        .ok_or_else(|| "tool requires `path`".to_string())
}

fn parse_required_u32(input: &ToolInput, key: &str, tool_name: &str) -> Result<u32, String> {
    input
        .get(key)
        .ok_or_else(|| format!("{tool_name} requires `{key}`"))?
        .parse::<u32>()
        .map_err(|_| format!("{tool_name} requires numeric `{key}`"))
}

fn semantic_provider(ctx: &ToolExecutionContext) -> Result<&Arc<dyn SemanticToolProvider>, String> {
    ctx.semantic
        .as_ref()
        .ok_or_else(|| "LSP semantic provider is not available".to_string())
}
