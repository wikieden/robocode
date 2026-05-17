use robocode_types::{ToolInput, ToolSpec};

use crate::git::common::{resolve_git_base_by_key, run_git_capture, run_git_capture_owned};
use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput, resolve_path};

pub(crate) struct GitWorktreeListTool;
pub(crate) struct GitWorktreeAddTool;
pub(crate) struct GitWorktreeRemoveTool;

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
