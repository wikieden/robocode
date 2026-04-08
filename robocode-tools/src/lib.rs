use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use robocode_types::{ToolCall, ToolInput, ToolResult, ToolSpec};

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub cwd: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionOutput {
    pub output: String,
    pub diff: Option<String>,
    pub success: bool,
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
        registry.register(GitStatusTool);
        registry.register(GitDiffTool);
        registry.register(GitBranchTool);
        registry.register(GitSwitchTool);
        registry.register(GitCommitTool);
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

pub fn build_shell_invocation(command: &str, windows: bool) -> (String, Vec<String>) {
    if windows {
        (
            "powershell".to_string(),
            vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                command.to_string(),
            ],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-lc".to_string(), command.to_string()],
        )
    }
}

struct ReadFileTool;
struct WriteFileTool;
struct EditFileTool;
struct GlobTool;
struct GrepTool;
struct ShellTool;
struct GitStatusTool;
struct GitDiffTool;
struct GitBranchTool;
struct GitSwitchTool;
struct GitCommitTool;

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

impl BuiltinTool for ShellTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".to_string(),
            description: "Run a shell command".to_string(),
            is_mutating: true,
            input_schema_hint: "command='cargo test'".to_string(),
        }
    }

    fn run(
        &self,
        ctx: &ToolExecutionContext,
        input: &ToolInput,
    ) -> Result<ToolExecutionOutput, String> {
        let command = input
            .get("command")
            .ok_or_else(|| "shell requires `command`".to_string())?;
        let windows = cfg!(windows);
        let (program, args) = build_shell_invocation(command, windows);
        let output = Command::new(program)
            .args(args)
            .current_dir(&ctx.cwd)
            .output()
            .map_err(|err| err.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut rendered = String::new();
        if !stdout.trim().is_empty() {
            rendered.push_str(stdout.trim_end());
        }
        if !stderr.trim().is_empty() {
            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(stderr.trim_end());
        }
        if rendered.is_empty() {
            rendered = format!("Command exited with {}", output.status);
        }
        Ok(ToolExecutionOutput {
            output: rendered,
            diff: None,
            success: output.status.success(),
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
    let candidate = input
        .get("path")
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

fn resolve_path(cwd: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
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
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "robocode_tools_{name}_{}",
            robocode_types::fresh_id("tmp")
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn read_write_edit_round_trip() {
        let cwd = temp_dir("files");
        let ctx = ToolExecutionContext { cwd: cwd.clone() };
        let registry = ToolRegistry::builtin();

        let mut write_input = ToolInput::new();
        write_input.insert("path".into(), "notes.txt".into());
        write_input.insert("content".into(), "hello world".into());
        let write_result = registry
            .execute(
                &ToolCall {
                    id: "tool_write".into(),
                    name: "write_file".into(),
                    input: write_input,
                },
                &ctx,
            )
            .unwrap();
        assert!(write_result.success);

        let mut read_input = ToolInput::new();
        read_input.insert("path".into(), "notes.txt".into());
        let read_result = registry
            .execute(
                &ToolCall {
                    id: "tool_read".into(),
                    name: "read_file".into(),
                    input: read_input,
                },
                &ctx,
            )
            .unwrap();
        assert!(read_result.output.contains("hello world"));

        let mut edit_input = ToolInput::new();
        edit_input.insert("path".into(), "notes.txt".into());
        edit_input.insert("old".into(), "world".into());
        edit_input.insert("new".into(), "rust".into());
        let edit_result = registry
            .execute(
                &ToolCall {
                    id: "tool_edit".into(),
                    name: "edit_file".into(),
                    input: edit_input,
                },
                &ctx,
            )
            .unwrap();
        assert!(edit_result.diff.unwrap().contains("+hello rust"));
    }

    #[test]
    fn shell_adapter_builds_cross_platform_invocations() {
        let (program_unix, args_unix) = build_shell_invocation("echo hi", false);
        assert_eq!(program_unix, "sh");
        assert_eq!(args_unix[0], "-lc");

        let (program_windows, args_windows) = build_shell_invocation("echo hi", true);
        assert_eq!(program_windows, "powershell");
        assert_eq!(args_windows[2], "-Command");
    }

    #[test]
    fn git_status_and_diff_work_in_repo() {
        let cwd = temp_dir("git_repo");
        let status = Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(status.success());
        let email = Command::new("git")
            .args(["config", "user.email", "robocode@example.com"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(email.success());
        let name = Command::new("git")
            .args(["config", "user.name", "RoboCode"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(name.success());
        fs::write(cwd.join("demo.txt"), "hello\n").unwrap();
        let add = Command::new("git")
            .args(["add", "demo.txt"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(add.success());
        let commit = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(commit.success());
        fs::write(cwd.join("demo.txt"), "hello again\n").unwrap();

        let ctx = ToolExecutionContext { cwd: cwd.clone() };
        let registry = ToolRegistry::builtin();

        let status_result = registry
            .execute(
                &ToolCall {
                    id: "tool_git_status".into(),
                    name: "git_status".into(),
                    input: ToolInput::new(),
                },
                &ctx,
            )
            .unwrap();
        assert!(status_result.output.contains("demo.txt"));

        let diff_result = registry
            .execute(
                &ToolCall {
                    id: "tool_git_diff".into(),
                    name: "git_diff".into(),
                    input: ToolInput::new(),
                },
                &ctx,
            )
            .unwrap();
        assert!(diff_result.output.contains("hello again"));
    }

    #[test]
    fn git_branch_switch_and_commit_work() {
        let cwd = temp_dir("git_branch");
        let init = Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(init.success());
        let email = Command::new("git")
            .args(["config", "user.email", "robocode@example.com"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(email.success());
        let name = Command::new("git")
            .args(["config", "user.name", "RoboCode"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(name.success());
        fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
        let add = Command::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(add.success());
        let commit = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&cwd)
            .status()
            .unwrap();
        assert!(commit.success());

        let ctx = ToolExecutionContext { cwd: cwd.clone() };
        let registry = ToolRegistry::builtin();

        let mut switch_input = ToolInput::new();
        switch_input.insert("branch".into(), "feature/demo".into());
        switch_input.insert("create".into(), "true".into());
        registry
            .execute(
                &ToolCall {
                    id: "tool_git_switch".into(),
                    name: "git_switch".into(),
                    input: switch_input,
                },
                &ctx,
            )
            .unwrap();

        fs::write(cwd.join("tracked.txt"), "second\n").unwrap();
        let mut commit_input = ToolInput::new();
        commit_input.insert("message".into(), "update tracked file".into());
        commit_input.insert("all".into(), "true".into());
        let result = registry
            .execute(
                &ToolCall {
                    id: "tool_git_commit".into(),
                    name: "git_commit".into(),
                    input: commit_input,
                },
                &ctx,
            )
            .unwrap();
        assert!(
            result.output.contains("update tracked file")
                || result.output.contains("files changed")
        );
    }
}
