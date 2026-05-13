use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use robocode_types::{ToolCall, ToolInput, ToolResult, ToolSpec};

mod shell;

use shell::ShellTool;
pub use shell::build_shell_invocation;

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

struct ReadFileTool;
struct WriteFileTool;
struct EditFileTool;
struct GlobTool;
struct GrepTool;
struct WebSearchTool;
struct WebFetchTool;
struct GitStatusTool;
struct GitDiffTool;
struct GitBranchTool;
struct GitSwitchTool;
struct GitAddTool;
struct GitRestoreTool;
struct GitCommitTool;
struct GitPushTool;
struct GitStashListTool;
struct GitStashPushTool;
struct GitStashPopTool;
struct GitStashDropTool;
struct GitWorktreeListTool;
struct GitWorktreeAddTool;
struct GitWorktreeRemoveTool;
struct LspDiagnosticsTool;
struct LspSymbolsTool;
struct LspReferencesTool;

impl BuiltinTool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".to_string(),
            description: "Read a UTF-8 text file".to_string(),
            is_mutating: false,
            input_schema_hint: "path=relative/or/absolute/path max_bytes=8192".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let path = resolve_required_path(ctx, input)?;
        let max_bytes = input
            .get("max_bytes")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(16 * 1024);
        let bytes = fs::read(&path).map_err(|err| err.to_string())?;
        let slice = &bytes[..bytes.len().min(max_bytes)];
        Ok(ToolExecutionOutput {
            output: String::from_utf8_lossy(slice).to_string(),
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".to_string(),
            description: "Create or overwrite a file".to_string(),
            is_mutating: true,
            input_schema_hint: "path=file content='new contents'".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let path = resolve_required_path(ctx, input)?;
        let content = input
            .get("content")
            .ok_or_else(|| "write_file requires `content`".to_string())?;
        let before = fs::read_to_string(&path).unwrap_or_default();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&path, content).map_err(|err| err.to_string())?;
        Ok(ToolExecutionOutput {
            output: format!("Wrote {}", path.display()),
            diff: Some(render_diff(&before, content)),
            success: true,
        })
    }
}

impl BuiltinTool for EditFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".to_string(),
            description: "Replace text inside a file".to_string(),
            is_mutating: true,
            input_schema_hint: "path=file old='find' new='replace'".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let path = resolve_required_path(ctx, input)?;
        let old = input
            .get("old")
            .ok_or_else(|| "edit_file requires `old`".to_string())?;
        let new = input
            .get("new")
            .ok_or_else(|| "edit_file requires `new`".to_string())?;
        let before = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        if !before.contains(old) {
            return Err("edit_file could not find the target text".to_string());
        }
        let after = before.replacen(old, new, 1);
        fs::write(&path, &after).map_err(|err| err.to_string())?;
        Ok(ToolExecutionOutput {
            output: format!("Edited {}", path.display()),
            diff: Some(render_diff(&before, &after)),
            success: true,
        })
    }
}

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

impl BuiltinTool for WebSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search".to_string(),
            description: "Search the web and return the top results".to_string(),
            is_mutating: false,
            input_schema_hint: "query='rust http client' limit=5 site=optional/domain".to_string(),
        }
    }

    fn run(
        &self,
        _ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let query = input
            .get("query")
            .ok_or_else(|| "web_search requires `query`".to_string())?;
        let limit = input
            .get("limit")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(5)
            .clamp(1, 10);
        let site = input.get("site").map(String::as_str);
        let scoped_query = if let Some(site) = site {
            format!("site:{site} {query}")
        } else {
            query.clone()
        };
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            url_encode(&scoped_query)
        );
        let html = fetch_url(&url, 30)?;
        let results = parse_duckduckgo_results(&html, limit);
        Ok(ToolExecutionOutput {
            output: if results.is_empty() {
                "No search results found.".to_string()
            } else {
                results
                    .iter()
                    .enumerate()
                    .map(|(index, result)| {
                        format!(
                            "{}. {}\n   {}\n   {}",
                            index + 1,
                            result.title,
                            result.url,
                            result.snippet
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for WebFetchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_fetch".to_string(),
            description: "Fetch a web page and return extracted text".to_string(),
            is_mutating: false,
            input_schema_hint: "url=https://example.com max_bytes=20000 raw=false".to_string(),
        }
    }

    fn run(
        &self,
        _ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let url = input
            .get("url")
            .ok_or_else(|| "web_fetch requires `url`".to_string())?;
        let max_bytes = input
            .get("max_bytes")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(20_000);
        let raw = input
            .get("raw")
            .map(|value| value == "true")
            .unwrap_or(false);
        let response = fetch_url(url, 30)?;
        let output = if raw {
            truncate_bytes(&response, max_bytes)
        } else {
            html_to_text(&response, max_bytes)
        };
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitStatusTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_status".to_string(),
            description: "Show git status for the current repository".to_string(),
            is_mutating: false,
            input_schema_hint: "path=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let output = run_git_capture(&repo, &["status", "--short", "--branch"])?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitDiffTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_diff".to_string(),
            description: "Show git diff for the current repository".to_string(),
            is_mutating: false,
            input_schema_hint: "path=optional/file/or/repo staged=false".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let staged = input
            .get("staged")
            .map(|value| value == "true")
            .unwrap_or(false);
        let mut args = vec!["diff".to_string()];
        if staged {
            args.push("--cached".to_string());
        }
        if let Some(path) = input.get("path") {
            let path = resolve_path(&ctx.cwd, path);
            if path.exists() && !path.is_dir() {
                let relative = path
                    .strip_prefix(&repo)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                args.push("--".to_string());
                args.push(relative);
            }
        }
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output: if output.trim().is_empty() {
                "No diff".to_string()
            } else {
                output.clone()
            },
            diff: if output.trim().is_empty() {
                None
            } else {
                Some(output)
            },
            success: true,
        })
    }
}

impl BuiltinTool for GitBranchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_branch".to_string(),
            description: "List local git branches".to_string(),
            is_mutating: false,
            input_schema_hint: "path=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let output = run_git_capture(&repo, &["branch", "--list"])?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitSwitchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_switch".to_string(),
            description: "Switch to or create a git branch".to_string(),
            is_mutating: true,
            input_schema_hint: "branch=name create=false path=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let branch = input
            .get("branch")
            .ok_or_else(|| "git_switch requires `branch`".to_string())?;
        let create = input
            .get("create")
            .map(|value| value == "true")
            .unwrap_or(false);
        let args: Vec<&str> = if create {
            vec!["switch", "-c", branch.as_str()]
        } else {
            vec!["switch", branch.as_str()]
        };
        let output = run_git_capture(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitCommitTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_commit".to_string(),
            description: "Create a git commit".to_string(),
            is_mutating: true,
            input_schema_hint: "message='commit message' all=false path=optional/repo/root"
                .to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let message = input
            .get("message")
            .ok_or_else(|| "git_commit requires `message`".to_string())?;
        let all = input
            .get("all")
            .map(|value| value == "true")
            .unwrap_or(false);
        let output = if all {
            run_git_capture(&repo, &["commit", "-am", message.as_str()])?
        } else {
            run_git_capture(&repo, &["commit", "-m", message.as_str()])?
        };
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitAddTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_add".to_string(),
            description: "Stage files in git".to_string(),
            is_mutating: true,
            input_schema_hint: "path=file paths='a\\nb' all=false path=optional/repo/root"
                .to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let all = input
            .get("all")
            .map(|value| value == "true")
            .unwrap_or(false);
        let mut args = vec!["add".to_string()];
        if all {
            args.push("--all".to_string());
        }
        for path in collect_git_paths(input) {
            args.push(path_relative_to_repo(&repo, &ctx.cwd, &path)?);
        }
        if args.len() == 1 {
            return Err("git_add requires at least one path or `all=true`".to_string());
        }
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitPushTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_push".to_string(),
            description: "Push the current branch to a git remote".to_string(),
            is_mutating: true,
            input_schema_hint:
                "remote=origin branch=current set_upstream=false path=optional/repo/root"
                    .to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let remote = input
            .get("remote")
            .cloned()
            .unwrap_or_else(|| "origin".to_string());
        let branch = input
            .get("branch")
            .cloned()
            .unwrap_or(current_git_branch(&repo)?);
        let set_upstream = input
            .get("set_upstream")
            .map(|value| value == "true")
            .unwrap_or(false);
        let mut args = vec!["push".to_string()];
        if set_upstream {
            args.push("--set-upstream".to_string());
        }
        args.push(remote);
        args.push(branch);
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitRestoreTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_restore".to_string(),
            description: "Restore files from git HEAD or another source".to_string(),
            is_mutating: true,
            input_schema_hint:
                "path=file paths='a\\nb' staged=false worktree=true source=HEAD path=optional/repo/root"
                    .to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let staged = input
            .get("staged")
            .map(|value| value == "true")
            .unwrap_or(false);
        let worktree = input
            .get("worktree")
            .map(|value| value != "false")
            .unwrap_or(true);
        if !staged && !worktree {
            return Err("git_restore requires `staged=true` or `worktree=true`".to_string());
        }
        let paths = collect_git_paths(input);
        if paths.is_empty() {
            return Err("git_restore requires at least one path".to_string());
        }
        let mut args = vec!["restore".to_string()];
        if staged {
            args.push("--staged".to_string());
        }
        if worktree && staged {
            args.push("--worktree".to_string());
        }
        if let Some(source) = input.get("source") {
            args.push("--source".to_string());
            args.push(source.clone());
        }
        args.push("--".to_string());
        for path in paths {
            args.push(path_relative_to_repo(&repo, &ctx.cwd, &path)?);
        }
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitStashListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_stash_list".to_string(),
            description: "List git stashes".to_string(),
            is_mutating: false,
            input_schema_hint: "path=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let output = run_git_capture(&repo, &["stash", "list"])?;
        Ok(ToolExecutionOutput {
            output: if output.trim().is_empty() {
                "No stashes".to_string()
            } else {
                output
            },
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitStashPushTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_stash_push".to_string(),
            description: "Create a git stash".to_string(),
            is_mutating: true,
            input_schema_hint:
                "message='stash message' include_untracked=false path=file paths='a\\nb' path=optional/repo/root"
                    .to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let mut args = vec!["stash".to_string(), "push".to_string()];
        if input
            .get("include_untracked")
            .map(|value| value == "true")
            .unwrap_or(false)
        {
            args.push("--include-untracked".to_string());
        }
        if let Some(message) = input.get("message") {
            args.push("-m".to_string());
            args.push(message.clone());
        }
        let paths = collect_git_paths(input);
        if !paths.is_empty() {
            args.push("--".to_string());
            for path in paths {
                args.push(path_relative_to_repo(&repo, &ctx.cwd, &path)?);
            }
        }
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitStashPopTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_stash_pop".to_string(),
            description: "Apply and drop a git stash".to_string(),
            is_mutating: true,
            input_schema_hint: "stash=stash@{0} path=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let mut args = vec!["stash".to_string(), "pop".to_string()];
        if let Some(stash) = input.get("stash") {
            args.push(stash.clone());
        }
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitStashDropTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_stash_drop".to_string(),
            description: "Drop a git stash without applying it".to_string(),
            is_mutating: true,
            input_schema_hint: "stash=stash@{0} path=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base(ctx, input)?;
        let stash = input
            .get("stash")
            .cloned()
            .unwrap_or_else(|| "stash@{0}".to_string());
        let args = vec!["stash".to_string(), "drop".to_string(), stash];
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitWorktreeListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_worktree_list".to_string(),
            description: "List git worktrees for the current repository".to_string(),
            is_mutating: false,
            input_schema_hint: "repo=optional/repo/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base_by_key(ctx, input, "repo")?;
        let output = run_git_capture(&repo, &["worktree", "list"])?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitWorktreeAddTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_worktree_add".to_string(),
            description: "Create a git worktree".to_string(),
            is_mutating: true,
            input_schema_hint: "path=../checkout branch=name create=false repo=optional/root"
                .to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base_by_key(ctx, input, "repo")?;
        let target = input
            .get("path")
            .ok_or_else(|| "git_worktree_add requires `path`".to_string())?;
        let target_path = resolve_path(&ctx.cwd, target);
        let branch = input.get("branch").cloned();
        let create = input
            .get("create")
            .map(|value| value == "true")
            .unwrap_or(false);
        let mut args = vec!["worktree".to_string(), "add".to_string()];
        if create {
            let branch = branch.clone().ok_or_else(|| {
                "git_worktree_add with `create=true` requires `branch`".to_string()
            })?;
            args.push("-b".to_string());
            args.push(branch);
        }
        args.push(target_path.to_string_lossy().to_string());
        if let Some(branch) = branch.filter(|_| !create) {
            args.push(branch);
        }
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

impl BuiltinTool for GitWorktreeRemoveTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_worktree_remove".to_string(),
            description: "Remove a git worktree".to_string(),
            is_mutating: true,
            input_schema_hint: "path=../checkout force=false repo=optional/root".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let repo = resolve_git_base_by_key(ctx, input, "repo")?;
        let target = input
            .get("path")
            .ok_or_else(|| "git_worktree_remove requires `path`".to_string())?;
        let target_path = resolve_path(&ctx.cwd, target);
        let force = input
            .get("force")
            .map(|value| value == "true")
            .unwrap_or(false);
        let mut args = vec!["worktree".to_string(), "remove".to_string()];
        if force {
            args.push("--force".to_string());
        }
        args.push(target_path.to_string_lossy().to_string());
        let output = run_git_capture_owned(&repo, &args)?;
        Ok(ToolExecutionOutput {
            output,
            diff: None,
            success: true,
        })
    }
}

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

fn resolve_required_path(ctx: &ToolExecutionContext, input: &ToolInput) -> Result<PathBuf, String> {
    let raw = input
        .get("path")
        .ok_or_else(|| "tool requires `path`".to_string())?;
    Ok(resolve_path(&ctx.cwd, raw))
}

fn resolve_optional_base(ctx: &ToolExecutionContext, input: &ToolInput) -> Result<PathBuf, String> {
    let raw = input.get("path").map(String::as_str).unwrap_or(".");
    let path = resolve_path(&ctx.cwd, raw);
    if !path.exists() {
        return Err(format!("Base path does not exist: {}", path.display()));
    }
    Ok(path)
}

fn resolve_git_base(ctx: &ToolExecutionContext, input: &ToolInput) -> Result<PathBuf, String> {
    resolve_git_base_by_key(ctx, input, "path")
}

fn resolve_git_base_by_key(
    ctx: &ToolExecutionContext,
    input: &ToolInput,
    key: &str,
) -> Result<PathBuf, String> {
    let candidate = input
        .get(key)
        .map(|path| resolve_path(&ctx.cwd, path))
        .unwrap_or_else(|| ctx.cwd.clone());
    let probe = if candidate.is_dir() {
        candidate
    } else {
        candidate
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| ctx.cwd.clone())
    };
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&probe)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Not a git repository".to_string()
        } else {
            stderr
        });
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn resolve_path(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn collect_git_paths(input: &ToolInput) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path) = input.get("path") {
        paths.push(path.clone());
    }
    if let Some(raw_paths) = input.get("paths") {
        for path in raw_paths.lines() {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                paths.push(trimmed.to_string());
            }
        }
    }
    paths
}

fn path_relative_to_repo(repo: &Path, cwd: &Path, raw: &str) -> Result<String, String> {
    let resolved = normalize_path_for_repo(resolve_path(cwd, raw));
    let repo = normalize_path_for_repo(repo.to_path_buf());
    let relative = resolved
        .strip_prefix(&repo)
        .map_err(|_| format!("Path is outside the repository: {}", resolved.display()))?;
    let rendered = relative.to_string_lossy().replace('\\', "/");
    Ok(if rendered.is_empty() {
        ".".to_string()
    } else {
        rendered
    })
}

fn normalize_path_for_repo(path: PathBuf) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(&path) {
        return canonical;
    }
    if let Some(parent) = path
        .parent()
        .and_then(|parent| fs::canonicalize(parent).ok())
    {
        if let Some(name) = path.file_name() {
            return parent.join(name);
        }
        return parent;
    }
    path
}

fn current_git_branch(repo: &Path) -> Result<String, String> {
    let branch = run_git_capture(repo, &["branch", "--show-current"])?;
    if branch.trim().is_empty() {
        Err("Could not determine the current branch".to_string())
    } else {
        Ok(branch)
    }
}

fn run_git_capture(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        if stdout.is_empty() && stderr.is_empty() {
            Ok(format!("git {} completed", args.join(" ")))
        } else if stdout.is_empty() {
            Ok(stderr)
        } else if stderr.is_empty() {
            Ok(stdout)
        } else {
            Ok(format!("{stdout}\n{stderr}"))
        }
    } else if !stderr.is_empty() {
        Err(stderr)
    } else if !stdout.is_empty() {
        Err(stdout)
    } else {
        Err(format!("git {} failed", args.join(" ")))
    }
}

fn run_git_capture_owned(repo: &Path, args: &[String]) -> Result<String, String> {
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git_capture(repo, &borrowed)
}

fn fetch_url(url: &str, timeout_secs: u64) -> Result<String, String> {
    let output = Command::new("curl")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("--max-time")
        .arg(timeout_secs.to_string())
        .arg("--user-agent")
        .arg("RoboCode/0.1 (+https://github.com/wikieden/robocode)")
        .arg(url)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("curl failed with status {}", output.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn url_encode(input: &str) -> String {
    let mut output = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char)
            }
            b' ' => output.push('+'),
            other => output.push_str(&format!("%{:02X}", other)),
        }
    }
    output
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &input[index + 1..index + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    output.push(value as char);
                    index += 3;
                } else {
                    output.push('%');
                    index += 1;
                }
            }
            byte => {
                output.push(byte as char);
                index += 1;
            }
        }
    }
    output
}

fn parse_duckduckgo_results(html: &str, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut offset = 0;
    while results.len() < limit {
        let Some(anchor_start_rel) = html[offset..].find("result__a") else {
            break;
        };
        let anchor_start = offset + anchor_start_rel;
        let href_search_start = html[..anchor_start].rfind("<a").unwrap_or(anchor_start);
        let Some(href_rel) = html[href_search_start..].find("href=\"") else {
            offset = anchor_start + 8;
            continue;
        };
        let href_start = href_search_start + href_rel + 6;
        let Some(href_end_rel) = html[href_start..].find('"') else {
            break;
        };
        let href_end = href_start + href_end_rel;
        let raw_href = &html[href_start..href_end];
        let Some(title_end_rel) = html[href_end..].find("</a>") else {
            break;
        };
        let title_end = href_end + title_end_rel;
        let title_html = html[href_end + 2..title_end].trim_start_matches('>');
        let title = clean_html_fragment(title_html);
        let snippet = extract_result_snippet(&html[title_end..]);
        let url = normalize_search_result_url(raw_href);
        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
        offset = title_end + 4;
    }
    results
}

fn extract_result_snippet(html: &str) -> String {
    let Some(snippet_rel) = html.find("result__snippet") else {
        return String::new();
    };
    let snippet_start = snippet_rel;
    let Some(tag_end_rel) = html[snippet_start..].find('>') else {
        return String::new();
    };
    let content_start = snippet_start + tag_end_rel + 1;
    let Some(content_end_rel) = html[content_start..].find("</") else {
        return String::new();
    };
    clean_html_fragment(&html[content_start..content_start + content_end_rel])
}

fn normalize_search_result_url(raw_href: &str) -> String {
    if let Some(uddg_index) = raw_href.find("uddg=") {
        let encoded = &raw_href[uddg_index + 5..];
        let encoded = encoded.split('&').next().unwrap_or(encoded);
        return percent_decode(encoded);
    }
    percent_decode(raw_href)
}

fn html_to_text(html: &str, max_bytes: usize) -> String {
    let stripped = strip_html_tags(&remove_html_noise(html));
    let decoded = decode_html_entities(&stripped);
    let normalized = normalize_whitespace(&decoded);
    truncate_bytes(&normalized, max_bytes)
}

fn remove_html_noise(html: &str) -> String {
    let without_script = remove_tag_block(html, "script");
    let without_style = remove_tag_block(&without_script, "style");
    remove_tag_block(&without_style, "noscript")
}

fn remove_tag_block(input: &str, tag: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    let start_marker = format!("<{tag}");
    let end_marker = format!("</{tag}>");
    while let Some(start_rel) = input[cursor..].to_ascii_lowercase().find(&start_marker) {
        let start = cursor + start_rel;
        output.push_str(&input[cursor..start]);
        if let Some(end_rel) = input[start..].to_ascii_lowercase().find(&end_marker) {
            cursor = start + end_rel + end_marker.len();
        } else {
            cursor = input.len();
        }
    }
    output.push_str(&input[cursor..]);
    output
}

fn strip_html_tags(input: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn normalize_whitespace(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn clean_html_fragment(input: &str) -> String {
    normalize_whitespace(&decode_html_entities(&strip_html_tags(input)))
}

fn truncate_bytes(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    let mut output = String::new();
    for ch in input.chars() {
        if output.len() + ch.len_utf8() > max_bytes.saturating_sub(3) {
            break;
        }
        output.push(ch);
    }
    output.push_str("...");
    output
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

fn render_diff(before: &str, after: &str) -> String {
    let before_lines: Vec<_> = before.lines().collect();
    let after_lines: Vec<_> = after.lines().collect();
    let mut output = String::from("--- before\n+++ after\n");
    let max = before_lines.len().max(after_lines.len());
    for index in 0..max {
        match (before_lines.get(index), after_lines.get(index)) {
            (Some(left), Some(right)) if left == right => {
                output.push(' ');
                output.push_str(left);
                output.push('\n');
            }
            (Some(left), Some(right)) => {
                output.push('-');
                output.push_str(left);
                output.push('\n');
                output.push('+');
                output.push_str(right);
                output.push('\n');
            }
            (Some(left), None) => {
                output.push('-');
                output.push_str(left);
                output.push('\n');
            }
            (None, Some(right)) => {
                output.push('+');
                output.push_str(right);
                output.push('\n');
            }
            (None, None) => {}
        }
    }
    output
}

#[cfg(test)]
mod tests;
