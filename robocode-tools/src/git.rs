use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use robocode_types::{ToolInput, ToolSpec};

use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput, resolve_path};

pub(crate) struct GitStatusTool;
pub(crate) struct GitDiffTool;
pub(crate) struct GitBranchTool;
pub(crate) struct GitSwitchTool;
pub(crate) struct GitAddTool;
pub(crate) struct GitRestoreTool;
pub(crate) struct GitCommitTool;
pub(crate) struct GitPushTool;
pub(crate) struct GitStashListTool;
pub(crate) struct GitStashPushTool;
pub(crate) struct GitStashPopTool;
pub(crate) struct GitStashDropTool;
pub(crate) struct GitWorktreeListTool;
pub(crate) struct GitWorktreeAddTool;
pub(crate) struct GitWorktreeRemoveTool;

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
