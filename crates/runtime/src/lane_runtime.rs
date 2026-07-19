use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
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
pub(crate) enum LaneEffectRequest {
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
pub(crate) struct LaneEffectResult {
    pub(crate) output: String,
    pub(crate) conflict_paths: Vec<String>,
}

impl LaneEffectResult {
    pub(crate) fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            conflict_paths: Vec::new(),
        }
    }
}

pub(crate) trait LaneEffectExecutor: Send + Sync {
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
pub(crate) struct LocalLaneEffectExecutor {
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
                let Some(path) = lane.worktree else {
                    return Ok(LaneEffectResult::success(
                        "lane registered without a worktree",
                    ));
                };
                let outcome = self
                    .worktrees
                    .create_worktree(&WorktreeCreateRequest {
                        repo,
                        path,
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
                let Some(path) = lane.worktree else {
                    return Ok(LaneEffectResult::success("lane cleanup completed"));
                };
                let outcome = self
                    .worktrees
                    .remove_worktree(&WorktreeRemoveRequest { repo, path, force })
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
        let Some(path) = lane.worktree.clone() else {
            return Ok(());
        };
        self.worktrees
            .remove_worktree(&WorktreeRemoveRequest {
                repo: repo.to_path_buf(),
                path,
                force: true,
            })
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn lane_working_directory(repo: &Path, lane: &AgentLaneRecord) -> Result<PathBuf, String> {
    let configured = lane
        .worktree
        .as_deref()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repo.join(path)
            }
        })
        .unwrap_or_else(|| repo.to_path_buf());
    configured
        .canonicalize()
        .map_err(|error| format!("invalid lane root `{}`: {error}", configured.display()))
}

/// Resolve one output path for every local lane route. The nearest existing
/// ancestor is canonicalized so a symlink cannot redirect a not-yet-created
/// log outside the lane root.
pub(crate) fn resolve_lane_output_log(
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
