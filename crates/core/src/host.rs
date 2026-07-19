use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use viden_config::CliOverrides;
use viden_runtime::{
    CredentialBackend, RuntimeBootstrapRequest, RuntimeResumeRequest, RuntimeSupervisor,
    bootstrap_runtime,
};
use viden_types::{
    CredentialHandle, CredentialRequestId, CredentialStatus, PermissionMode, UiPreferences,
    fresh_id, now_timestamp,
};

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
    Credential(String),
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
            Self::Credential(message) => write!(formatter, "credential staging failed: {message}"),
        }
    }
}

impl std::error::Error for CoreHostError {}

/// Secret bytes accepted only at the trusted host boundary.
///
/// This type intentionally does not implement `Clone`, `Debug`, `Serialize`,
/// or `Deserialize`; staged runtime commands receive only a
/// [`CredentialRequestId`].
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    fn zeroize(&mut self) {
        for byte in &mut self.0 {
            // Prevent the compiler from optimizing away the wipe before the
            // allocation is released.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }

    #[doc(hidden)]
    pub fn drop_probe_for_test(bytes: Vec<u8>) -> Vec<u8> {
        let mut secret = Self::new(bytes);
        secret.zeroize();
        secret.0.clone()
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.zeroize();
    }
}

pub struct LocalCoreHost {
    session_home: Option<PathBuf>,
    credential_staging: Arc<CredentialStagingStore>,
    credential_clock: Arc<AtomicU64>,
    credential_capacity: usize,
    credential_sink: Arc<dyn TrustedCredentialSink>,
}

impl LocalCoreHost {
    pub fn new() -> Self {
        Self {
            session_home: None,
            credential_staging: Arc::new(CredentialStagingStore::default()),
            credential_clock: Arc::new(AtomicU64::new(0)),
            credential_capacity: DEFAULT_CREDENTIAL_STAGING_CAPACITY,
            credential_sink: Arc::new(UnavailableTrustedCredentialSink),
        }
    }

    pub fn with_session_home(session_home: impl Into<PathBuf>) -> Self {
        Self {
            session_home: Some(session_home.into()),
            credential_staging: Arc::new(CredentialStagingStore::default()),
            credential_clock: Arc::new(AtomicU64::new(0)),
            credential_capacity: DEFAULT_CREDENTIAL_STAGING_CAPACITY,
            credential_sink: Arc::new(UnavailableTrustedCredentialSink),
        }
    }

    pub fn for_test(session_home: impl Into<PathBuf>) -> Self {
        Self::with_session_home(session_home).with_test_credential_sink()
    }

    #[doc(hidden)]
    pub fn with_credential_capacity_for_test(mut self, capacity: usize) -> Self {
        self.credential_capacity = capacity.max(1);
        self
    }

    #[doc(hidden)]
    pub fn with_credential_clock_for_test(self, now: u64) -> Self {
        self.credential_clock.store(now, Ordering::Release);
        self
    }

    #[doc(hidden)]
    pub fn set_credential_clock_for_test(&self, now: u64) {
        self.credential_clock.store(now, Ordering::Release);
    }

    #[doc(hidden)]
    pub fn fail_next_credential_sink_for_test(&self, message: &str) {
        if let Some(sink) = self
            .credential_sink
            .as_any()
            .downcast_ref::<TestTrustedCredentialSink>()
        {
            *sink.fail_next.lock().expect("test sink lock") = Some(message.to_string());
        }
    }

    #[doc(hidden)]
    pub fn consume_staged_credential_for_test(
        &self,
        binding: &WorkspaceBinding,
        provider_id: &str,
        backend_id: &str,
        credential_request_id: &str,
    ) -> Result<CredentialHandle, String> {
        let backend = BoundCredentialBackend {
            staging: Arc::clone(&self.credential_staging),
            sink: Arc::clone(&self.credential_sink),
            binding: WorkspaceCredentialBinding::from(binding),
            clock: Arc::clone(&self.credential_clock),
        };
        backend.store(provider_id, backend_id, credential_request_id)
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
        let mut engine = bootstrap.engine;
        let session_id = engine.session_id().to_string();
        let placeholder_binding = WorkspaceBinding {
            canonical_root: canonical_root.clone(),
            session_id: session_id.clone(),
            stream_id: String::new(),
        };
        engine = engine.with_credential_backend(Arc::new(BoundCredentialBackend {
            staging: Arc::clone(&self.credential_staging),
            sink: Arc::clone(&self.credential_sink),
            binding: WorkspaceCredentialBinding::from(&placeholder_binding),
            clock: Arc::clone(&self.credential_clock),
        }));
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
        Ok(BoundCoreClient {
            binding,
            client,
            credential_staging: Arc::clone(&self.credential_staging),
            credential_clock: Arc::clone(&self.credential_clock),
            credential_capacity: self.credential_capacity,
        })
    }

    pub fn stage_credential_for_binding(
        &self,
        binding: &WorkspaceBinding,
        provider_id: &str,
        backend_id: &str,
        secret: SecretBytes,
    ) -> Result<CredentialRequestId, CoreHostError> {
        stage_credential(
            &self.credential_staging,
            &self.credential_clock,
            self.credential_capacity,
            binding,
            provider_id,
            backend_id,
            secret,
        )
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
    credential_staging: Arc<CredentialStagingStore>,
    credential_clock: Arc<AtomicU64>,
    credential_capacity: usize,
}

impl BoundCoreClient {
    pub fn binding(&self) -> &WorkspaceBinding {
        &self.binding
    }

    pub fn client(&mut self) -> &mut StatefulCoreClient<LocalCoreTransport> {
        &mut self.client
    }

    pub fn stage_credential(
        &self,
        provider_id: &str,
        backend_id: &str,
        secret: SecretBytes,
    ) -> Result<CredentialRequestId, CoreHostError> {
        stage_credential(
            &self.credential_staging,
            &self.credential_clock,
            self.credential_capacity,
            &self.binding,
            provider_id,
            backend_id,
            secret,
        )
    }
}

const DEFAULT_CREDENTIAL_STAGING_CAPACITY: usize = 64;
const CREDENTIAL_TTL_SECONDS: u64 = 300;

trait TrustedCredentialSink: Send + Sync {
    fn store(
        &self,
        provider_id: &str,
        backend_id: &str,
        secret: SecretBytes,
    ) -> Result<CredentialHandle, String>;

    fn as_any(&self) -> &dyn std::any::Any;
}

struct UnavailableTrustedCredentialSink;

impl TrustedCredentialSink for UnavailableTrustedCredentialSink {
    fn store(
        &self,
        _provider_id: &str,
        _backend_id: &str,
        _secret: SecretBytes,
    ) -> Result<CredentialHandle, String> {
        Err("credential platform sink unavailable".to_string())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct TestTrustedCredentialSink {
    fail_next: Mutex<Option<String>>,
}

impl TrustedCredentialSink for TestTrustedCredentialSink {
    fn store(
        &self,
        provider_id: &str,
        backend_id: &str,
        _secret: SecretBytes,
    ) -> Result<CredentialHandle, String> {
        if let Some(message) = self.fail_next.lock().expect("test sink lock").take() {
            return Err(message);
        }
        Ok(CredentialHandle {
            provider_id: provider_id.to_string(),
            backend_id: backend_id.to_string(),
            status: CredentialStatus::Available,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LocalCoreHost {
    fn with_test_credential_sink(mut self) -> Self {
        self.credential_sink = Arc::new(TestTrustedCredentialSink {
            fail_next: Mutex::new(None),
        });
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WorkspaceCredentialBinding {
    canonical_root: PathBuf,
    session_id: String,
}

impl From<&WorkspaceBinding> for WorkspaceCredentialBinding {
    fn from(binding: &WorkspaceBinding) -> Self {
        Self {
            canonical_root: binding.canonical_root.clone(),
            session_id: binding.session_id.clone(),
        }
    }
}

struct StagedCredential {
    binding: WorkspaceCredentialBinding,
    provider_id: String,
    backend_id: String,
    expires_at: u64,
    secret: SecretBytes,
}

#[derive(Default)]
struct CredentialStagingStore {
    entries: Mutex<BTreeMap<String, StagedCredential>>,
}

struct BoundCredentialBackend {
    staging: Arc<CredentialStagingStore>,
    sink: Arc<dyn TrustedCredentialSink>,
    binding: WorkspaceCredentialBinding,
    clock: Arc<AtomicU64>,
}

impl CredentialBackend for BoundCredentialBackend {
    fn store(
        &self,
        provider_id: &str,
        backend_id: &str,
        credential_request_id: &str,
    ) -> Result<CredentialHandle, String> {
        let now = credential_now(&self.clock);
        let staged = {
            let mut entries = self
                .staging
                .entries
                .lock()
                .map_err(|_| "credential staging lock poisoned".to_string())?;
            let Some(staged) = entries.get(credential_request_id) else {
                return Err("credential request is missing or already consumed".to_string());
            };
            if staged.binding != self.binding
                || staged.provider_id != provider_id
                || staged.backend_id != backend_id
            {
                return Err(
                    "credential request does not match workspace, provider, or backend".to_string(),
                );
            }
            if now > staged.expires_at {
                entries.remove(credential_request_id);
                return Err("credential request expired".to_string());
            }
            entries
                .remove(credential_request_id)
                .expect("validated staged request")
        };
        self.sink.store(provider_id, backend_id, staged.secret)
    }
}

fn stage_credential(
    staging: &Arc<CredentialStagingStore>,
    clock: &Arc<AtomicU64>,
    capacity: usize,
    binding: &WorkspaceBinding,
    provider_id: &str,
    backend_id: &str,
    secret: SecretBytes,
) -> Result<CredentialRequestId, CoreHostError> {
    validate_host_identifier("provider_id", provider_id).map_err(CoreHostError::Credential)?;
    validate_host_identifier("backend_id", backend_id).map_err(CoreHostError::Credential)?;
    let now = credential_now(clock);
    let mut entries = staging
        .entries
        .lock()
        .map_err(|_| CoreHostError::Credential("credential staging lock poisoned".to_string()))?;
    entries.retain(|_, staged| staged.expires_at >= now);
    if entries.len() >= capacity {
        return Err(CoreHostError::Credential(
            "credential staging capacity reached".to_string(),
        ));
    }
    let mut id = fresh_id("crq");
    while entries.contains_key(&id) {
        id = fresh_id("crq");
    }
    entries.insert(
        id.clone(),
        StagedCredential {
            binding: WorkspaceCredentialBinding::from(binding),
            provider_id: provider_id.to_string(),
            backend_id: backend_id.to_string(),
            expires_at: now.saturating_add(CREDENTIAL_TTL_SECONDS),
            secret,
        },
    );
    Ok(CredentialRequestId::new(id))
}

fn credential_now(clock: &AtomicU64) -> u64 {
    let fixed = clock.load(Ordering::Acquire);
    if fixed == 0 { now_timestamp() } else { fixed }
}

fn validate_host_identifier(name: &str, value: &str) -> Result<(), String> {
    let grammar_is_safe = !value.is_empty()
        && value.len() <= 96
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && !value.contains("..")
        && !value.contains("::");
    if grammar_is_safe {
        Ok(())
    } else {
        Err(format!("invalid credential {name}"))
    }
}
