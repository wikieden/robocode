use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneEffectError {
    UnsafePath { path: String },
    Io(String),
    Git(String),
    PatchConflict { path: PathBuf, message: String },
}

impl std::fmt::Display for LaneEffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsafePath { path } => write!(f, "unsafe worktree path `{path}`"),
            Self::Io(err) | Self::Git(err) => f.write_str(err),
            Self::PatchConflict { path, message } if path.as_os_str().is_empty() => {
                f.write_str(message)
            }
            Self::PatchConflict { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for LaneEffectError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCreateRequest {
    pub repo: PathBuf,
    pub path: String,
    pub branch: Option<String>,
    pub create_branch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRemoveRequest {
    pub repo: PathBuf,
    pub path: String,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEffectOutcome {
    pub path: PathBuf,
    pub output: String,
}

pub trait WorktreeBackend: Send + Sync {
    fn create_worktree(
        &self,
        request: &WorktreeCreateRequest,
    ) -> Result<WorktreeEffectOutcome, LaneEffectError>;

    fn remove_worktree(
        &self,
        request: &WorktreeRemoveRequest,
    ) -> Result<WorktreeEffectOutcome, LaneEffectError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalLaneEffects;

impl WorktreeBackend for LocalLaneEffects {
    fn create_worktree(
        &self,
        request: &WorktreeCreateRequest,
    ) -> Result<WorktreeEffectOutcome, LaneEffectError> {
        let target = resolve_worktree_path(&request.repo, &request.path)?;
        let mut args = vec!["worktree".to_string(), "add".to_string()];
        if request.create_branch {
            let branch = request.branch.clone().ok_or_else(|| {
                LaneEffectError::Git("worktree create requires `branch`".to_string())
            })?;
            args.push("-b".to_string());
            args.push(branch);
        }
        args.push(target.to_string_lossy().to_string());
        if let Some(branch) = request.branch.clone().filter(|_| !request.create_branch) {
            args.push(branch);
        }
        let output = run_git_capture_owned(&request.repo, &args)?;
        Ok(WorktreeEffectOutcome {
            path: target,
            output,
        })
    }

    fn remove_worktree(
        &self,
        request: &WorktreeRemoveRequest,
    ) -> Result<WorktreeEffectOutcome, LaneEffectError> {
        let target = resolve_worktree_path(&request.repo, &request.path)?;
        let mut args = vec!["worktree".to_string(), "remove".to_string()];
        if request.force {
            args.push("--force".to_string());
        }
        args.push(target.to_string_lossy().to_string());
        let output = run_git_capture_owned(&request.repo, &args)?;
        Ok(WorktreeEffectOutcome {
            path: target,
            output,
        })
    }
}

fn resolve_worktree_path(repo: &Path, raw: &str) -> Result<PathBuf, LaneEffectError> {
    let candidate = Path::new(raw);
    // Lane-owned worktrees may be absolute, but relative requests cannot climb
    // above the repository root through `..` components.
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) && !candidate.is_absolute()
    {
        return Err(LaneEffectError::UnsafePath {
            path: raw.to_string(),
        });
    }
    if candidate.is_absolute() {
        Ok(candidate.to_path_buf())
    } else {
        Ok(repo.join(candidate))
    }
}

#[allow(dead_code)]
fn _command_is_send_sync(_: &Command) {}

fn run_git_capture_owned(repo: &Path, args: &[String]) -> Result<String, LaneEffectError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|err| LaneEffectError::Git(err.to_string()))?;
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
        Err(LaneEffectError::Git(stderr))
    } else if !stdout.is_empty() {
        Err(LaneEffectError::Git(stdout))
    } else {
        Err(LaneEffectError::Git(format!(
            "git {} failed",
            args.join(" ")
        )))
    }
}
