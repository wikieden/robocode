use viden_types::{ToolInput, ToolSpec};

use crate::git::common::{resolve_git_base_by_key, run_git_capture};
use crate::lane::{
    LocalLaneEffects, WorktreeBackend, WorktreeCreateRequest, WorktreeRemoveRequest,
};
use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput};

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
            exit_code: None,
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
        let branch = input.get("branch").cloned();
        let create = input
            .get("create")
            .map(|value| value == "true")
            .unwrap_or(false);
        let outcome = LocalLaneEffects
            .create_worktree(&WorktreeCreateRequest {
                repo,
                path: target.clone(),
                branch,
                create_branch: create,
            })
            .map_err(|err| err.to_string())?;
        Ok(ToolExecutionOutput {
            output: outcome.output,
            diff: None,
            success: true,
            exit_code: None,
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
        let force = input
            .get("force")
            .map(|value| value == "true")
            .unwrap_or(false);
        let outcome = LocalLaneEffects
            .remove_worktree(&WorktreeRemoveRequest {
                repo,
                path: target.clone(),
                force,
            })
            .map_err(|err| err.to_string())?;
        Ok(ToolExecutionOutput {
            output: outcome.output,
            diff: None,
            success: true,
            exit_code: None,
        })
    }
}
