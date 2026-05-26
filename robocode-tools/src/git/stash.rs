use robocode_types::{ToolInput, ToolSpec};

use crate::git::common::{
    collect_git_paths, path_relative_to_repo, resolve_git_base, run_git_capture,
    run_git_capture_owned,
};
use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput};

pub(crate) struct GitStashListTool;
pub(crate) struct GitStashPushTool;
pub(crate) struct GitStashPopTool;
pub(crate) struct GitStashDropTool;

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
            exit_code: None,
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
            exit_code: None,
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
            exit_code: None,
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
            exit_code: None,
        })
    }
}
