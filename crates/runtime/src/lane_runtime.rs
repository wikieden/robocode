use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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
                let cwd = lane_working_directory(&repo, &lane);
                let handle = match lane.route {
                    AgentRoute::BuiltIn | AgentRoute::Acp => ActiveLaneHandle::Process(
                        self.processes
                            .spawn(&SpawnProcess {
                                command,
                                args,
                                cwd,
                                env,
                                output_log: output_log.map(PathBuf::from),
                            })
                            .map_err(|error| error.to_string())?,
                    ),
                    AgentRoute::Terminal | AgentRoute::Tmux => {
                        let log = output_log.map(PathBuf::from).unwrap_or_else(|| {
                            repo.join(".viden/lanes").join(format!("{}.log", lane.id))
                        });
                        ActiveLaneHandle::Terminal(
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
                                    output_log: log,
                                })
                                .map_err(|error| error.to_string())?,
                        )
                    }
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
}

fn lane_working_directory(repo: &Path, lane: &AgentLaneRecord) -> PathBuf {
    lane.worktree
        .as_deref()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repo.join(path)
            }
        })
        .unwrap_or_else(|| repo.to_path_buf())
}
