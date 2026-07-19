use std::path::PathBuf;

use viden_config::CliOverrides;
use viden_runtime::{
    RuntimeBootstrapRequest, RuntimeResumeRequest, RuntimeSupervisor, bootstrap_runtime,
};
use viden_types::{PermissionMode, UiPreferences};

use crate::{CoreClient, LocalCoreTransport, StatefulCoreClient};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceOpenOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<PermissionMode>,
    pub session_home: Option<PathBuf>,
    pub request_timeout_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub config_path: Option<PathBuf>,
    pub ui: Option<UiPreferences>,
}

impl From<WorkspaceOpenOverrides> for CliOverrides {
    fn from(overrides: WorkspaceOpenOverrides) -> Self {
        Self {
            provider: overrides.provider,
            model: overrides.model,
            api_base: None,
            api_key: None,
            provider_plugin_dirs: Vec::new(),
            permission_mode: overrides.permission_mode,
            session_home: overrides.session_home,
            request_timeout_secs: overrides.request_timeout_secs,
            max_retries: overrides.max_retries,
            config_path: overrides.config_path,
            ui: overrides.ui,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceOpenRequest {
    pub root: PathBuf,
    pub resume_session_id: Option<String>,
    pub overrides: WorkspaceOpenOverrides,
}

impl WorkspaceOpenRequest {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            resume_session_id: None,
            overrides: WorkspaceOpenOverrides::default(),
        }
    }

    pub fn with_overrides(mut self, overrides: WorkspaceOpenOverrides) -> Self {
        self.overrides = overrides;
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

        let mut cli_overrides = CliOverrides::from(request.overrides);
        if cli_overrides.session_home.is_none() {
            cli_overrides.session_home = self.session_home.clone();
        }
        let mut bootstrap_request =
            RuntimeBootstrapRequest::new(canonical_root.clone(), cli_overrides);
        if let Some(session_id) = request.resume_session_id {
            bootstrap_request =
                bootstrap_request.with_resume(RuntimeResumeRequest::exact_session_id(session_id));
        }
        let bootstrap = bootstrap_runtime(bootstrap_request).map_err(CoreHostError::Bootstrap)?;
        let engine = bootstrap.engine;
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
