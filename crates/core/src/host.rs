use std::path::PathBuf;

use viden_config::CliOverrides;
use viden_runtime::{RuntimeBootstrapRequest, RuntimeSupervisor, bootstrap_runtime};

use crate::{CoreClient, LocalCoreTransport, StatefulCoreClient};

#[derive(Clone, Default)]
pub struct WorkspaceOpenRequest {
    pub root: PathBuf,
    pub resume_session_id: Option<String>,
    pub cli_overrides: CliOverrides,
}

impl WorkspaceOpenRequest {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            resume_session_id: None,
            cli_overrides: CliOverrides::default(),
        }
    }

    pub fn with_cli_overrides(mut self, cli_overrides: CliOverrides) -> Self {
        self.cli_overrides = cli_overrides;
        self
    }

    pub fn with_resume_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.resume_session_id = Some(session_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBinding {
    pub canonical_root: PathBuf,
    pub session_id: String,
    pub stream_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreHostError {
    InvalidWorkspace { root: PathBuf, reason: String },
    Bootstrap(String),
    Snapshot(String),
}

impl std::fmt::Display for CoreHostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidWorkspace { root, reason } => {
                write!(
                    formatter,
                    "invalid workspace `{}`: {reason}",
                    root.display()
                )
            }
            Self::Bootstrap(message) => write!(formatter, "core bootstrap failed: {message}"),
            Self::Snapshot(message) => write!(formatter, "core snapshot failed: {message}"),
        }
    }
}

impl std::error::Error for CoreHostError {}

pub struct LocalCoreHost {
    session_home: Option<PathBuf>,
}

impl LocalCoreHost {
    pub fn new() -> Self {
        Self { session_home: None }
    }

    pub fn with_session_home(session_home: impl Into<PathBuf>) -> Self {
        Self {
            session_home: Some(session_home.into()),
        }
    }

    pub fn for_test(session_home: impl Into<PathBuf>) -> Self {
        Self::with_session_home(session_home)
    }

    pub fn open_workspace(
        &self,
        request: WorkspaceOpenRequest,
    ) -> Result<BoundCoreClient, CoreHostError> {
        let canonical_root =
            request
                .root
                .canonicalize()
                .map_err(|err| CoreHostError::InvalidWorkspace {
                    root: request.root.clone(),
                    reason: err.to_string(),
                })?;
        if !canonical_root.is_dir() {
            return Err(CoreHostError::InvalidWorkspace {
                root: canonical_root,
                reason: "workspace root must be an existing directory".to_string(),
            });
        }

        let mut cli_overrides = request.cli_overrides;
        if cli_overrides.session_home.is_none() {
            cli_overrides.session_home = self.session_home.clone();
        }
        let resume_session_id = request.resume_session_id;
        let bootstrap = bootstrap_runtime(RuntimeBootstrapRequest::new(
            canonical_root.clone(),
            cli_overrides,
        ))
        .map_err(CoreHostError::Bootstrap)?;
        let mut engine = bootstrap.engine;
        if let Some(session_id) = resume_session_id {
            let mut deny = |_prompt| viden_types::ApprovalResponse::deny(None);
            engine
                .process_input_with_approval(&format!("/resume {session_id}"), &mut deny)
                .map_err(CoreHostError::Bootstrap)?;
        }
        let session_id = engine.session_id().to_string();
        let supervisor = RuntimeSupervisor::start(engine);
        let snapshot = supervisor
            .snapshot_envelope()
            .map_err(CoreHostError::Snapshot)?;
        let binding = WorkspaceBinding {
            canonical_root,
            session_id,
            stream_id: snapshot.cursor.stream_id,
        };
        let mut client = StatefulCoreClient::new(LocalCoreTransport::new(supervisor));
        client.discover().map_err(|err| {
            CoreHostError::Bootstrap(format!("core client handshake failed: {err}"))
        })?;
        Ok(BoundCoreClient { binding, client })
    }
}

impl Default for LocalCoreHost {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BoundCoreClient {
    binding: WorkspaceBinding,
    client: StatefulCoreClient<LocalCoreTransport>,
}

impl BoundCoreClient {
    pub fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }

    pub fn client(&mut self) -> &mut StatefulCoreClient<LocalCoreTransport> {
        &mut self.client
    }
}
