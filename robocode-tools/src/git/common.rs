use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use robocode_types::ToolInput;

use crate::{ToolExecutionContext, resolve_path};

pub(super) fn resolve_git_base(
    ctx: &ToolExecutionContext,
    input: &ToolInput,
) -> Result<PathBuf, String> {
    resolve_git_base_by_key(ctx, input, "path")
}

pub(super) fn resolve_git_base_by_key(
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

pub(super) fn collect_git_paths(input: &ToolInput) -> Vec<String> {
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

pub(super) fn path_relative_to_repo(repo: &Path, cwd: &Path, raw: &str) -> Result<String, String> {
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

pub(super) fn current_git_branch(repo: &Path) -> Result<String, String> {
    let branch = run_git_capture(repo, &["branch", "--show-current"])?;
    if branch.trim().is_empty() {
        Err("Could not determine the current branch".to_string())
    } else {
        Ok(branch)
    }
}

pub(super) fn run_git_capture(repo: &Path, args: &[&str]) -> Result<String, String> {
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

pub(super) fn run_git_capture_owned(repo: &Path, args: &[String]) -> Result<String, String> {
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git_capture(repo, &borrowed)
}
