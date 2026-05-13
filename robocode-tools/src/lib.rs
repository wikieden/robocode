use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use robocode_types::{ToolCall, ToolInput, ToolResult, ToolSpec};

mod files;
mod git;
mod lsp;
mod shell;
mod web;

use files::{EditFileTool, ReadFileTool, WriteFileTool};
use git::{
    GitAddTool, GitBranchTool, GitCommitTool, GitDiffTool, GitPushTool, GitRestoreTool,
    GitStashDropTool, GitStashListTool, GitStashPopTool, GitStashPushTool, GitStatusTool,
    GitSwitchTool, GitWorktreeAddTool, GitWorktreeListTool, GitWorktreeRemoveTool,
};
use lsp::{LspDiagnosticsTool, LspReferencesTool, LspSymbolsTool};
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

struct GlobTool;
struct GrepTool;

impl BuiltinTool for GlobTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".to_string(),
            description: "Find files by wildcard pattern".to_string(),
            is_mutating: false,
            input_schema_hint: "pattern=src/*.rs path=optional/base".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let pattern = input
            .get("pattern")
            .ok_or_else(|| "glob requires `pattern`".to_string())?;
        let base = resolve_optional_base(ctx, input)?;
        let mut results = Vec::new();
        walk(&base, &mut |path| {
            let relative = path
                .strip_prefix(&base)
                .unwrap_or(path)
                .display()
                .to_string()
                .replace('\\', "/");
            if wildcard_match(pattern, &relative) {
                results.push(path.display().to_string());
            }
        })?;
        Ok(ToolExecutionOutput {
            output: if results.is_empty() {
                "No matches".to_string()
            } else {
                results.join("\n")
            },
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".to_string(),
            description: "Search files for a text pattern".to_string(),
            is_mutating: false,
            input_schema_hint: "pattern=needle path=optional/base".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let pattern = input
            .get("pattern")
            .ok_or_else(|| "grep requires `pattern`".to_string())?;
        let base = resolve_optional_base(ctx, input)?;
        let mut matches = Vec::new();
        walk(&base, &mut |path| {
            if path.is_dir() {
                return;
            }
            if let Ok(contents) = fs::read_to_string(path) {
                for (line_number, line) in contents.lines().enumerate() {
                    if line.contains(pattern) {
                        matches.push(format!(
                            "{}:{}:{}",
                            path.display(),
                            line_number + 1,
                            line.trim()
                        ));
                    }
                }
            }
        })?;
        Ok(ToolExecutionOutput {
            output: if matches.is_empty() {
                "No matches".to_string()
            } else {
                matches.join("\n")
            },
            diff: None,
            success: true,
        })
    }
}

fn resolve_optional_base(ctx: &ToolExecutionContext, input: &ToolInput) -> Result<PathBuf, String> {
    let raw = input.get("path").map(String::as_str).unwrap_or(".");
    let path = resolve_path(&ctx.cwd, raw);
    if !path.exists() {
        return Err(format!("Base path does not exist: {}", path.display()));
    }
    Ok(path)
}

fn resolve_path(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn walk(root: &Path, f: &mut dyn FnMut(&Path)) -> Result<(), String> {
    f(root);
    if root.is_dir() {
        for entry in fs::read_dir(root).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, f)?;
            } else {
                f(&path);
            }
        }
    }
    Ok(())
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    wildcard_match_inner(pattern.as_bytes(), candidate.as_bytes())
}

fn wildcard_match_inner(pattern: &[u8], candidate: &[u8]) -> bool {
    if pattern.is_empty() {
        return candidate.is_empty();
    }
    match pattern[0] {
        b'*' => {
            wildcard_match_inner(&pattern[1..], candidate)
                || (!candidate.is_empty() && wildcard_match_inner(pattern, &candidate[1..]))
        }
        b'?' => !candidate.is_empty() && wildcard_match_inner(&pattern[1..], &candidate[1..]),
        byte => {
            !candidate.is_empty()
                && byte == candidate[0]
                && wildcard_match_inner(&pattern[1..], &candidate[1..])
        }
    }
}

#[cfg(test)]
mod tests;
