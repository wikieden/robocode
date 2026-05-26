use robocode_types::{ToolInput, ToolSpec};

use crate::git::common::{resolve_git_base, run_git_capture, run_git_capture_owned};
use crate::{BuiltinTool, ToolExecutionContext, ToolExecutionOutput, resolve_path};

pub(crate) struct GitStatusTool;
pub(crate) struct GitDiffTool;
pub(crate) struct GitBranchTool;

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
            exit_code: None,
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
            exit_code: None,
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
            exit_code: None,
        })
    }
}
