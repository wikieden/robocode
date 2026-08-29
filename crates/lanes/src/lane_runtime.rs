use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use viden_tools::lane::{
    LocalLaneEffects, WorktreeBackend, WorktreeCreateRequest, WorktreeRemoveRequest,
};
use viden_tools::patch::{LocalPatchBackend, PatchBackend, PatchRequest};
use viden_tools::process::{
    LaneProcessHandle, LaneTerminalHandle, LocalProcessBackend, LocalTerminalBackend,
    ProcessBackend, SpawnProcess, SpawnTerminal, TerminalBackend, TerminalKind,
};
use viden_types::{AgentLaneRecord, AgentRoute};

#[derive(Debug, Clone)]
pub enum LaneEffectRequest {
    Create {
        repo: PathBuf,
        lane: AgentLaneRecord,
    },
    Start {
        repo: PathBuf,
        lane: AgentLaneRecord,
        command: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
        output_log: Option<String>,
    },
    Stop {
        lane_id: String,
    },
    SendInput {
        lane_id: String,
        input: String,
    },
    Apply {
        cwd: PathBuf,
        unified_diff: String,
    },
    Cleanup {
        repo: PathBuf,
        lane: AgentLaneRecord,
        force: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneEffectResult {
    pub output: String,
    pub conflict_paths: Vec<String>,
}

impl LaneEffectResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            conflict_paths: Vec::new(),
        }
    }
}

pub trait LaneEffectExecutor: Send + Sync {
    fn execute(&self, request: LaneEffectRequest) -> Result<LaneEffectResult, String>;

    fn apply_transactionally(
        &self,
        request: LaneEffectRequest,
        persist: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<LaneEffectResult, String> {
        let result = self.execute(request)?;
        if result.conflict_paths.is_empty() {
            persist()?;
        }
        Ok(result)
    }

    fn shutdown_lane(&self, lane_id: &str) -> Result<(), String> {
        self.execute(LaneEffectRequest::Stop {
            lane_id: lane_id.to_string(),
        })
        .map(|_| ())
    }

    fn compensate_create(&self, _repo: &Path, _lane: &AgentLaneRecord) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ActiveLaneHandle {
    Process(LaneProcessHandle),
    Terminal(LaneTerminalHandle),
}

/// Owns every local process, terminal, worktree, and patch side effect used by
/// lane commands. Frontends only submit typed commands and never receive raw
/// backend handles.
#[derive(Debug, Default)]
pub struct LocalLaneEffectExecutor {
    worktrees: LocalLaneEffects,
    processes: LocalProcessBackend,
    terminals: LocalTerminalBackend,
    patches: LocalPatchBackend,
    handles: Mutex<BTreeMap<String, ActiveLaneHandle>>,
}

impl LaneEffectExecutor for LocalLaneEffectExecutor {
    fn execute(&self, request: LaneEffectRequest) -> Result<LaneEffectResult, String> {
        match request {
            LaneEffectRequest::Create { repo, lane } => {
                if lane.worktree.is_none() {
                    return Ok(LaneEffectResult::success(
                        "lane registered without a worktree",
                    ));
                }
                let mut lane = lane;
                lane.worktree = Some(
                    resolve_lane_target(&repo, &lane, true)?
                        .to_string_lossy()
                        .to_string(),
                );
                let repo = canonical_repo_root(&repo)?;
                let outcome = self
                    .worktrees
                    .create_worktree(&WorktreeCreateRequest {
                        repo,
                        path: lane.worktree.expect("resolved lane worktree"),
                        create_branch: lane.branch.is_some(),
                        branch: lane.branch,
                    })
                    .map_err(|error| error.to_string())?;
                Ok(LaneEffectResult::success(outcome.output))
            }
            LaneEffectRequest::Start {
                repo,
                lane,
                command,
                args,
                env,
                output_log,
            } => {
                if self
                    .handles
                    .lock()
                    .map_err(|_| "lane effect handle registry poisoned".to_string())?
                    .contains_key(&lane.id)
                {
                    return Err(format!("lane `{}` already has an active runtime", lane.id));
                }
                let cwd = lane_working_directory(&repo, &lane)?;
                let output_log = resolve_lane_output_log(&cwd, output_log.as_deref(), &lane.id)?;
                let handle = match lane.route {
                    AgentRoute::BuiltIn | AgentRoute::Acp => ActiveLaneHandle::Process(
                        self.processes
                            .spawn(&SpawnProcess {
                                command,
                                args,
                                cwd,
                                env,
                                output_log: Some(output_log),
                            })
                            .map_err(|error| error.to_string())?,
                    ),
                    AgentRoute::Terminal | AgentRoute::Tmux => ActiveLaneHandle::Terminal(
                        self.terminals
                            .spawn(&SpawnTerminal {
                                kind: if lane.route == AgentRoute::Tmux {
                                    TerminalKind::Tmux
                                } else {
                                    TerminalKind::Pty
                                },
                                session_name: Some(format!("viden-{}", lane.id)),
                                command,
                                args,
                                cwd,
                                env,
                                output_log,
                            })
                            .map_err(|error| error.to_string())?,
                    ),
                };
                let id = match &handle {
                    ActiveLaneHandle::Process(handle) => handle.id.clone(),
                    ActiveLaneHandle::Terminal(handle) => handle.id.clone(),
                };
                self.handles
                    .lock()
                    .map_err(|_| "lane effect handle registry poisoned".to_string())?
                    .insert(lane.id, handle);
                Ok(LaneEffectResult::success(format!(
                    "lane runtime started: {id}"
                )))
            }
            LaneEffectRequest::Stop { lane_id } => {
                let handle = self
                    .handles
                    .lock()
                    .map_err(|_| "lane effect handle registry poisoned".to_string())?
                    .remove(&lane_id)
                    .ok_or_else(|| format!("lane `{lane_id}` has no active runtime"))?;
                match handle {
                    ActiveLaneHandle::Process(handle) => self.processes.stop(&handle),
                    ActiveLaneHandle::Terminal(handle) => self.terminals.stop(&handle),
                }
                .map_err(|error| error.to_string())?;
                Ok(LaneEffectResult::success("lane runtime stopped"))
            }
            LaneEffectRequest::SendInput { lane_id, input } => {
                let handles = self
                    .handles
                    .lock()
                    .map_err(|_| "lane effect handle registry poisoned".to_string())?;
                let handle = handles
                    .get(&lane_id)
                    .ok_or_else(|| format!("lane `{lane_id}` has no active runtime"))?;
                match handle {
                    ActiveLaneHandle::Process(handle) => {
                        self.processes.send(handle, input.as_bytes())
                    }
                    ActiveLaneHandle::Terminal(handle) => {
                        self.terminals.send(handle, input.as_bytes())
                    }
                }
                .map_err(|error| error.to_string())?;
                Ok(LaneEffectResult::success("lane input delivered"))
            }
            LaneEffectRequest::Apply { cwd, unified_diff } => {
                let cwd = canonical_repo_root(&cwd)?;
                let outcome = self
                    .patches
                    .apply(&PatchRequest { cwd, unified_diff })
                    .map_err(|error| error.to_string())?;
                if outcome.applied {
                    Ok(LaneEffectResult::success(format!(
                        "applied {} lane path(s)",
                        outcome.writes.len()
                    )))
                } else {
                    Ok(LaneEffectResult {
                        output: outcome
                            .conflicts
                            .iter()
                            .map(|conflict| conflict.message.clone())
                            .collect::<Vec<_>>()
                            .join("; "),
                        conflict_paths: outcome
                            .conflicts
                            .iter()
                            .map(|conflict| conflict.path.to_string_lossy().to_string())
                            .collect(),
                    })
                }
            }
            LaneEffectRequest::Cleanup { repo, lane, force } => {
                if lane.worktree.is_none() {
                    return Ok(LaneEffectResult::success("lane cleanup completed"));
                }
                let mut lane = lane;
                let resolved = resolve_lane_target(&repo, &lane, true)?;
                if !resolved.exists() {
                    return Ok(LaneEffectResult::success("lane cleanup already reconciled"));
                }
                lane.worktree = Some(resolved.to_string_lossy().to_string());
                let outcome = self
                    .worktrees
                    .remove_worktree(&WorktreeRemoveRequest {
                        repo: canonical_repo_root(&repo)?,
                        path: lane.worktree.expect("resolved lane worktree"),
                        force,
                    })
                    .map_err(|error| error.to_string())?;
                Ok(LaneEffectResult::success(outcome.output))
            }
        }
    }

    fn shutdown_lane(&self, lane_id: &str) -> Result<(), String> {
        let handle = self
            .handles
            .lock()
            .map_err(|_| "lane effect handle registry poisoned".to_string())?
            .remove(lane_id);
        let Some(handle) = handle else {
            return Ok(());
        };
        match handle {
            ActiveLaneHandle::Process(handle) => self.processes.stop(&handle),
            ActiveLaneHandle::Terminal(handle) => self.terminals.stop(&handle),
        }
        .map_err(|error| error.to_string())
    }

    fn apply_transactionally(
        &self,
        request: LaneEffectRequest,
        persist: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<LaneEffectResult, String> {
        let LaneEffectRequest::Apply { cwd, unified_diff } = request else {
            return Err("transactional lane apply requires an apply request".to_string());
        };
        let cwd = canonical_repo_root(&cwd)?;
        let outcome = self
            .patches
            .apply_transactionally(&PatchRequest { cwd, unified_diff }, persist)
            .map_err(|error| error.to_string())?;
        if outcome.applied {
            Ok(LaneEffectResult::success(format!(
                "applied {} lane path(s)",
                outcome.writes.len()
            )))
        } else {
            Ok(LaneEffectResult {
                output: outcome
                    .conflicts
                    .iter()
                    .map(|conflict| conflict.message.clone())
                    .collect::<Vec<_>>()
                    .join("; "),
                conflict_paths: outcome
                    .conflicts
                    .iter()
                    .map(|conflict| conflict.path.to_string_lossy().to_string())
                    .collect(),
            })
        }
    }

    fn compensate_create(&self, repo: &Path, lane: &AgentLaneRecord) -> Result<(), String> {
        if lane.worktree.is_none() {
            return Ok(());
        }
        let mut lane = lane.clone();
        lane.worktree = Some(
            resolve_lane_target(repo, &lane, true)?
                .to_string_lossy()
                .to_string(),
        );
        self.worktrees
            .remove_worktree(&WorktreeRemoveRequest {
                repo: canonical_repo_root(repo)?,
                path: lane.worktree.expect("resolved lane worktree"),
                force: true,
            })
            .map_err(|error| error.to_string())?;
        // A successful Create request with `branch=Some` used `git worktree add -b`,
        // so the branch did not predate this failed transaction. Remove it only
        // after the validated worktree target has been compensated successfully.
        if let Some(branch) = lane.branch {
            let repo = canonical_repo_root(repo)?;
            let output = Command::new("git")
                .args(["branch", "--delete", "--force", "--", &branch])
                .current_dir(&repo)
                .output()
                .map_err(|error| format!("cannot compensate lane branch `{branch}`: {error}"))?;
            if !output.status.success() {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return Err(if detail.is_empty() {
                    format!("cannot compensate lane branch `{branch}`")
                } else {
                    format!("cannot compensate lane branch `{branch}`: {detail}")
                });
            }
        }
        Ok(())
    }
}

fn lane_working_directory(repo: &Path, lane: &AgentLaneRecord) -> Result<PathBuf, String> {
    resolve_lane_target(repo, lane, false)
}

pub(crate) fn canonical_repo_root(repo: &Path) -> Result<PathBuf, String> {
    repo.canonicalize()
        .map_err(|error| format!("invalid repository root `{}`: {error}", repo.display()))
}

/// Resolve the filesystem object that both permission checks and local effects use.
/// Missing create/cleanup targets are anchored through their nearest real parent so
/// symlinks and `..` cannot turn an in-repo spelling into an out-of-repo effect.
pub(crate) fn resolve_lane_target(
    repo: &Path,
    lane: &AgentLaneRecord,
    allow_missing: bool,
) -> Result<PathBuf, String> {
    let root = canonical_repo_root(repo)?;
    let Some(raw) = lane.worktree.as_deref() else {
        return Ok(root);
    };
    let configured = Path::new(raw);
    if configured
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("lane target `{raw}` escapes repository root"));
    }
    let candidate = if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        root.join(configured)
    };
    let resolved = if std::fs::symlink_metadata(&candidate).is_ok() {
        candidate
            .canonicalize()
            .map_err(|error| format!("invalid lane target `{}`: {error}", candidate.display()))?
    } else if allow_missing {
        if !candidate.starts_with(&root) {
            return Err(format!(
                "lane target `{}` escapes repository root `{}`",
                candidate.display(),
                root.display()
            ));
        }
        let mut parent = candidate.parent();
        while let Some(path) = parent {
            if path == root {
                break;
            }
            if std::fs::symlink_metadata(path)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                return Err(format!(
                    "lane target `{}` has a symlink parent `{}`",
                    candidate.display(),
                    path.display()
                ));
            }
            parent = path.parent();
        }
        let mut ancestor = candidate.as_path();
        while std::fs::symlink_metadata(ancestor).is_err() {
            ancestor = ancestor.parent().ok_or_else(|| {
                format!("lane target `{}` has no scoped parent", candidate.display())
            })?;
        }
        let suffix = candidate
            .strip_prefix(ancestor)
            .map_err(|_| format!("lane target `{}` has no scoped parent", candidate.display()))?;
        ancestor
            .canonicalize()
            .map_err(|error| format!("invalid lane target `{}`: {error}", candidate.display()))?
            .join(suffix)
    } else {
        return Err(format!(
            "invalid lane target `{}`: path does not exist",
            candidate.display()
        ));
    };
    if !resolved.starts_with(&root) {
        return Err(format!(
            "lane target `{}` escapes repository root `{}`",
            resolved.display(),
            root.display()
        ));
    }
    Ok(resolved)
}

/// Resolve one output path for every local lane route. The nearest existing
/// ancestor is canonicalized so a symlink cannot redirect a not-yet-created
/// log outside the lane root.
pub fn resolve_lane_output_log(
    lane_root: &Path,
    requested: Option<&str>,
    lane_id: &str,
) -> Result<PathBuf, String> {
    let root = lane_root
        .canonicalize()
        .map_err(|error| format!("invalid lane root `{}`: {error}", lane_root.display()))?;
    let default_log = format!(".viden/lanes/{lane_id}.log");
    let raw = requested.unwrap_or(&default_log);
    let relative = Path::new(raw);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("lane output log `{raw}` escapes the lane root"));
    }
    let candidate = root.join(relative);
    let mut ancestor = candidate.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| format!("lane output log `{raw}` has no scoped parent"))?;
    }
    let canonical_ancestor = ancestor
        .canonicalize()
        .map_err(|error| format!("invalid lane output log `{raw}`: {error}"))?;
    if !canonical_ancestor.starts_with(&root) {
        return Err(format!("lane output log `{raw}` escapes the lane root"));
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_lane_apply_rolls_back_bytes_when_persistence_fails() {
        let root = std::env::temp_dir().join(format!(
            "viden-lane-transaction-{}",
            viden_types::fresh_id("test")
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("demo.txt");
        std::fs::write(&path, "old\n").unwrap();
        let executor = LocalLaneEffectExecutor::default();
        let mut persist = || Err("injected lane persistence failure".to_string());
        let result = executor.apply_transactionally(
            LaneEffectRequest::Apply {
                cwd: root.clone(),
                unified_diff: "diff --git a/demo.txt b/demo.txt\n--- a/demo.txt\n+++ b/demo.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
            },
            &mut persist,
        );
        assert!(
            result
                .unwrap_err()
                .contains("injected lane persistence failure")
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "old\n");
    }
}
