use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use robocode_types::{ToolCall, ToolInput, ToolResult, ToolSpec};

mod files;
mod git;
mod lsp;
mod search;
mod shell;
mod web;

use files::{EditFileTool, ReadFileTool, WriteFileTool};
use git::{
    GitAddTool, GitBranchTool, GitCommitTool, GitDiffTool, GitPushTool, GitRestoreTool,
    GitStashDropTool, GitStashListTool, GitStashPopTool, GitStashPushTool, GitStatusTool,
    GitSwitchTool, GitWorktreeAddTool, GitWorktreeListTool, GitWorktreeRemoveTool,
};
use lsp::{LspDiagnosticsTool, LspReferencesTool, LspSymbolsTool};
use search::{GlobTool, GrepTool};
use shell::ShellTool;
pub use shell::build_shell_invocation;
use web::{WebFetchTool, WebSearchTool};
#[cfg(test)]
pub(crate) use web::{html_to_text, parse_duckduckgo_results, url_encode};

#[derive(Clone)]
pub struct ToolExecutionContext {
    pub cwd: PathBuf,
    pub semantic: Option<Arc<dyn SemanticToolProvider>>,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionOutput {
    pub output: String,
    pub diff: Option<String>,
    pub success: bool,
}

pub trait SemanticToolProvider: Send + Sync {
    fn diagnostics(&self, cwd: &Path, path: &Path) -> Result<String, String>;

    fn symbols(&self, cwd: &Path, path: &Path) -> Result<String, String>;

    fn references(
        &self,
        cwd: &Path,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<String, String>;
}

pub trait BuiltinTool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String>;
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn BuiltinTool>>,
}

impl ToolRegistry {
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        registry.register(ReadFileTool);
        registry.register(WriteFileTool);
        registry.register(EditFileTool);
        registry.register(GlobTool);
        registry.register(GrepTool);
        registry.register(ShellTool);
        registry.register(WebSearchTool);
        registry.register(WebFetchTool);
        registry.register(GitStatusTool);
        registry.register(GitDiffTool);
        registry.register(GitBranchTool);
        registry.register(GitSwitchTool);
        registry.register(GitAddTool);
        registry.register(GitRestoreTool);
        registry.register(GitCommitTool);
        registry.register(GitPushTool);
        registry.register(GitStashListTool);
        registry.register(GitStashPushTool);
        registry.register(GitStashPopTool);
        registry.register(GitStashDropTool);
        registry.register(GitWorktreeListTool);
        registry.register(GitWorktreeAddTool);
        registry.register(GitWorktreeRemoveTool);
        registry.register(LspDiagnosticsTool);
        registry.register(LspSymbolsTool);
        registry.register(LspReferencesTool);
        registry
    }

    pub fn register<T>(&mut self, tool: T)
    where
        T: BuiltinTool + 'static,
    {
        self.tools.insert(tool.spec().name.clone(), Arc::new(tool));
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|tool| tool.spec()).collect()
    }

    pub fn spec(&self, name: &str) -> Option<ToolSpec> {
        self.tools.get(name).map(|tool| tool.spec())
    }

    pub fn execute(
        &self,
        call: &ToolCall,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult, String> {
        let tool = self
            .tools
            .get(&call.name)
            .ok_or_else(|| format!("Unknown tool: {}", call.name))?;
        let output = tool.run(ctx, &call.input)?;
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            output: output.output,
            diff: output.diff,
            success: output.success,
        })
    }
}

fn resolve_path(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests;
