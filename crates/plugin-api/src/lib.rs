use std::os::raw::c_char;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginKind {
    Provider,
    Tool,
    Agent,
    Context,
    Workflow,
    Ui,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginCapability {
    Provider,
    ToolExecution,
    AgentRole,
    ContextSource,
    ContextReducer,
    WorkflowGate,
    UiSurface,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginPermission {
    Network,
    FileRead,
    FileWrite,
    Shell,
    Git,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    pub config_schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtocolFamily {
    Anthropic,
    OpenAi,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderAuthMode {
    ApiKey,
    WebLogin,
    Local,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderEnvMappings {
    pub api_key_env: Option<String>,
    pub api_base_env: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_native_tool_calling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCompatibility {
    #[serde(default = "default_true")]
    pub supports_tool_choice: bool,
    #[serde(default)]
    pub requires_reasoning_content_for_tool_calls: bool,
    #[serde(default)]
    pub requires_non_null_tool_call_content: bool,
    #[serde(default)]
    pub reasoning_effort_high: Option<String>,
    #[serde(default)]
    pub reasoning_effort_max: Option<String>,
}

impl Default for ProviderCompatibility {
    fn default() -> Self {
        Self {
            supports_tool_choice: true,
            requires_reasoning_content_for_tool_calls: false,
            requires_non_null_tool_call_content: false,
            reasoning_effort_high: None,
            reasoning_effort_max: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_family: ProtocolFamily,
    pub default_api_base: Option<String>,
    pub default_model: Option<String>,
    #[serde(default)]
    pub known_models: Vec<String>,
    pub env_mappings: ProviderEnvMappings,
    pub capabilities: ProviderCapabilities,
    #[serde(default)]
    pub compatibility: ProviderCompatibility,
    #[serde(default)]
    pub auth_modes: Vec<ProviderAuthMode>,
    pub config_schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentTransport {
    Acp,
    AppServer,
    Template,
    Pty,
    Tmux,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentSource {
    Registry,
    LocalCommand,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentAuthMode {
    AgentNative,
    ApiKey,
    WebLogin,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentProtocolVersion {
    AcpV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentPluginCapability {
    SessionPrompt,
    SessionLoad,
    SessionCancel,
    SessionSetMode,
    SessionSetModel,
    StreamingUpdates,
    ToolCalls,
    ImageInput,
    SlashCommands,
    McpEvents,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentPermissionProfile {
    ReadOnlyProbe,
    RuntimeGated,
    AgentNative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentEnvRef {
    pub name: String,
    pub required: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCommandSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<AgentEnvRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRegistryPackage {
    pub package: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPluginDescriptor {
    pub agent_id: String,
    pub display_name: String,
    pub version: String,
    pub transport: AgentTransport,
    pub source: AgentSource,
    pub command: AgentCommandSpec,
    pub registry_package: Option<AgentRegistryPackage>,
    #[serde(default)]
    pub protocol_versions: Vec<AgentProtocolVersion>,
    #[serde(default)]
    pub auth_modes: Vec<AgentAuthMode>,
    #[serde(default)]
    pub capabilities: Vec<AgentPluginCapability>,
    pub permission_profile: AgentPermissionProfile,
    #[serde(default)]
    pub experimental_methods: Vec<String>,
    pub config_schema_version: u32,
}

pub const CONTEXT_REDUCER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextReducerContentKind {
    Json,
    Code,
    Diff,
    Log,
    Diagnostic,
    Transcript,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerLimits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_output_items: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerDescriptor {
    pub reducer_id: String,
    pub display_name: String,
    pub version: String,
    pub supported_schema_versions: Vec<u32>,
    pub content_kinds: Vec<ContextReducerContentKind>,
    pub limits: ContextReducerLimits,
    /// Context reducers are optimization adapters. They must be explicitly
    /// enabled by configuration before the host calls them.
    #[serde(default)]
    pub default_enabled: bool,
    pub config_schema_version: u32,
    #[serde(default)]
    pub process: Option<ContextReducerProcessDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerCircuitBreakerConfig {
    pub failure_threshold: u32,
    pub backoff_ms: u64,
}

impl Default for ContextReducerCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            backoff_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerAdapterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub preferred_reducer_id: Option<String>,
    #[serde(default = "default_context_reducer_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_context_reducer_quality_threshold")]
    pub min_quality_score_microunits: u64,
    #[serde(default = "default_context_reducer_quality_threshold")]
    pub min_evidence_recall_microunits: u64,
    #[serde(default = "default_context_reducer_max_output_bytes")]
    pub max_output_bytes: usize,
    #[serde(default = "default_context_reducer_max_output_items")]
    pub max_output_items: usize,
    #[serde(default = "default_context_reducer_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub circuit_breaker: ContextReducerCircuitBreakerConfig,
}

impl Default for ContextReducerAdapterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            preferred_reducer_id: None,
            timeout_ms: default_context_reducer_timeout_ms(),
            min_quality_score_microunits: default_context_reducer_quality_threshold(),
            min_evidence_recall_microunits: default_context_reducer_quality_threshold(),
            max_output_bytes: default_context_reducer_max_output_bytes(),
            max_output_items: default_context_reducer_max_output_items(),
            max_depth: default_context_reducer_max_depth(),
            circuit_breaker: ContextReducerCircuitBreakerConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerProcessDescriptor {
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    pub trusted_root: String,
    #[serde(default)]
    pub env_allowlist: Vec<String>,
    #[serde(default = "default_context_reducer_stderr_bytes")]
    pub max_stderr_bytes: usize,
    #[serde(default)]
    pub authorization: Option<ContextReducerProcessAuthorization>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerProcessAuthorization {
    pub adapter_id: String,
    pub adapter_version: String,
    pub executable_identity: String,
    pub permission_snapshot_ref: String,
}

const fn default_context_reducer_stderr_bytes() -> usize {
    4 * 1024
}

const fn default_context_reducer_timeout_ms() -> u64 {
    250
}

const fn default_context_reducer_quality_threshold() -> u64 {
    900_000
}

const fn default_context_reducer_max_output_bytes() -> usize {
    16 * 1024
}

const fn default_context_reducer_max_output_items() -> usize {
    512
}

const fn default_context_reducer_max_depth() -> usize {
    64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerCanonicalRef {
    pub item_id: String,
    pub content_sha256: String,
    #[serde(default)]
    pub evidence_id: Option<String>,
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerPolicy {
    pub max_output_bytes: usize,
    pub max_output_tokens: u64,
    pub max_input_bytes: usize,
    pub max_depth: usize,
    pub max_output_items: usize,
    #[serde(default)]
    pub required_markers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerScope {
    pub role: String,
    pub task_id: String,
    #[serde(default)]
    pub dag_id: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerQualityFacts {
    pub passed: bool,
    pub score_microunits: u64,
    pub evidence_recall_microunits: u64,
    #[serde(default)]
    pub checks: Vec<String>,
    pub deterministic_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub content_kind: ContextReducerContentKind,
    pub canonical: ContextReducerCanonicalRef,
    pub policy: ContextReducerPolicy,
    pub scope: ContextReducerScope,
    pub permission_snapshot_ref: String,
    #[serde(default)]
    pub native_baseline_quality: Option<ContextReducerQualityFacts>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerOmission {
    pub reason: String,
    pub omitted_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextReducerHealthStatus {
    Ok,
    Disabled,
    CircuitOpen,
    PolicyRejected,
    AdapterAbsent,
    Timeout,
    Crash,
    Malformed,
    VersionMismatch,
    BindingMismatch,
    Oversize,
    QualityFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerHealthMetadata {
    pub latency_ms: u64,
    pub status: ContextReducerHealthStatus,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextReducerResponse {
    pub schema_version: u32,
    pub request_id: String,
    pub canonical_hash: String,
    pub permission_snapshot_ref: String,
    pub scope: ContextReducerScope,
    pub content_kind: ContextReducerContentKind,
    pub reduced_content: String,
    #[serde(default)]
    pub omissions: Vec<ContextReducerOmission>,
    pub reducer_id: String,
    pub reducer_version: String,
    pub quality: ContextReducerQualityFacts,
    pub health: ContextReducerHealthMetadata,
}

const fn default_true() -> bool {
    true
}

pub const VIDEN_PLUGIN_DESCRIPTOR_SYMBOL: &str = "viden_plugin_descriptor_json";
pub type PluginDescriptorFn = unsafe extern "C" fn() -> *const c_char;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_descriptor_roundtrips_through_json() {
        let descriptor = PluginDescriptor {
            provider_id: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            version: "1".to_string(),
            protocol_family: ProtocolFamily::OpenAi,
            default_api_base: Some("https://api.deepseek.com".to_string()),
            default_model: Some("deepseek-v4-flash".to_string()),
            known_models: vec![
                "deepseek-v4-flash".to_string(),
                "deepseek-v4-pro".to_string(),
            ],
            env_mappings: ProviderEnvMappings {
                api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
                api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
            },
            capabilities: ProviderCapabilities {
                supports_streaming: true,
                supports_native_tool_calling: true,
            },
            compatibility: ProviderCompatibility {
                supports_tool_choice: false,
                requires_reasoning_content_for_tool_calls: true,
                requires_non_null_tool_call_content: true,
                reasoning_effort_high: Some("high".to_string()),
                reasoning_effort_max: Some("max".to_string()),
            },
            auth_modes: vec![ProviderAuthMode::ApiKey],
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.provider_id, "deepseek");
        assert_eq!(decoded.protocol_family, ProtocolFamily::OpenAi);
        assert!(!decoded.compatibility.supports_tool_choice);
    }

    #[test]
    fn plugin_manifest_roundtrips_through_json() {
        let manifest = PluginManifest {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            version: "1".to_string(),
            kind: PluginKind::Provider,
            capabilities: vec![PluginCapability::Provider],
            permissions: vec![PluginPermission::Network],
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.kind, PluginKind::Provider);
        assert_eq!(decoded.capabilities, vec![PluginCapability::Provider]);
    }

    #[test]
    fn context_reducer_descriptor_roundtrips_with_disabled_default() {
        let descriptor = ContextReducerDescriptor {
            reducer_id: "neutral-context-adapter".to_string(),
            display_name: "Neutral Context Adapter".to_string(),
            version: "0.1.0".to_string(),
            supported_schema_versions: vec![CONTEXT_REDUCER_SCHEMA_VERSION],
            content_kinds: vec![ContextReducerContentKind::Log],
            limits: ContextReducerLimits {
                max_input_bytes: 4096,
                max_output_bytes: 1024,
                max_output_items: 32,
                max_depth: 8,
            },
            default_enabled: false,
            config_schema_version: 1,
            process: Some(ContextReducerProcessDescriptor {
                executable: "/trusted/root/bin/context-reducer".to_string(),
                args: vec!["--json".to_string()],
                cwd: Some("/trusted/root".to_string()),
                trusted_root: "/trusted/root".to_string(),
                env_allowlist: vec!["LANG".to_string()],
                max_stderr_bytes: 512,
                authorization: Some(ContextReducerProcessAuthorization {
                    adapter_id: "neutral-context-adapter".to_string(),
                    adapter_version: "0.1.0".to_string(),
                    executable_identity: "/trusted/root/bin/context-reducer".to_string(),
                    permission_snapshot_ref: "plugin-install:ctx-reducer:approved".to_string(),
                }),
            }),
        };

        let json = serde_json::to_value(&descriptor).unwrap();
        assert_eq!(json["reducer_id"], "neutral-context-adapter");
        assert_eq!(json["default_enabled"], false);
        let decoded: ContextReducerDescriptor = serde_json::from_value(json).unwrap();

        assert_eq!(decoded, descriptor);
        assert!(!ContextReducerAdapterConfig::default().enabled);
        assert_eq!(
            ContextReducerAdapterConfig::default().preferred_reducer_id,
            None
        );
    }

    #[test]
    fn context_reducer_envelope_exposes_hash_scope_and_budget_without_paths_or_secrets() {
        let request = ContextReducerRequest {
            schema_version: CONTEXT_REDUCER_SCHEMA_VERSION,
            request_id: "ctxred-1".to_string(),
            content_kind: ContextReducerContentKind::Log,
            canonical: ContextReducerCanonicalRef {
                item_id: "ctxi-1".to_string(),
                content_sha256: "ab".repeat(32),
                evidence_id: Some("ev-1".to_string()),
                reference: "context-item:ctxi-1".to_string(),
            },
            policy: ContextReducerPolicy {
                max_output_bytes: 1024,
                max_output_tokens: 256,
                max_input_bytes: 4096,
                max_depth: 8,
                max_output_items: 32,
                required_markers: vec!["first_failure".to_string()],
            },
            scope: ContextReducerScope {
                role: "executor".to_string(),
                task_id: "task-1".to_string(),
                dag_id: None,
                workflow_id: Some("wf-1".to_string()),
            },
            permission_snapshot_ref: "perm-snap-1".to_string(),
            native_baseline_quality: None,
        };

        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"content_sha256\""));
        assert!(!json.contains("storage_path"));
        assert!(!json.contains("/Users/"));
        assert!(!json.contains("sk-test"));
        assert_eq!(
            serde_json::from_str::<ContextReducerRequest>(&json).unwrap(),
            request
        );
    }

    #[test]
    fn context_reducer_response_binds_request_hash_scope_identity_quality_and_health() {
        let response = ContextReducerResponse {
            schema_version: CONTEXT_REDUCER_SCHEMA_VERSION,
            request_id: "ctxred-1".to_string(),
            canonical_hash: "ab".repeat(32),
            permission_snapshot_ref: "perm-snap-1".to_string(),
            scope: ContextReducerScope {
                role: "executor".to_string(),
                task_id: "task-1".to_string(),
                dag_id: None,
                workflow_id: Some("wf-1".to_string()),
            },
            content_kind: ContextReducerContentKind::Log,
            reduced_content: "ERROR src/a.rs:1 boom".to_string(),
            omissions: vec![ContextReducerOmission {
                reason: "deduplicated".to_string(),
                omitted_count: 2,
            }],
            reducer_id: "neutral-context-adapter".to_string(),
            reducer_version: "0.1.0".to_string(),
            quality: ContextReducerQualityFacts {
                passed: true,
                score_microunits: 980_000,
                evidence_recall_microunits: 990_000,
                checks: vec!["first_failure_retained".to_string()],
                deterministic_fingerprint: "fp-1".to_string(),
            },
            health: ContextReducerHealthMetadata {
                latency_ms: 12,
                status: ContextReducerHealthStatus::Ok,
                message: None,
            },
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["canonical_hash"], "ab".repeat(32));
        assert_eq!(json["quality"]["passed"], true);
        assert_eq!(json["health"]["status"], "ok");
        assert_eq!(
            serde_json::from_value::<ContextReducerResponse>(json).unwrap(),
            response
        );
    }

    #[test]
    fn agent_plugin_descriptor_roundtrips_through_json() {
        let descriptor = AgentPluginDescriptor {
            agent_id: "kiro-cli".to_string(),
            display_name: "Kiro CLI".to_string(),
            version: "1".to_string(),
            transport: AgentTransport::Acp,
            source: AgentSource::LocalCommand,
            command: AgentCommandSpec {
                command: "kiro-cli".to_string(),
                args: vec!["acp".to_string()],
                env: vec![],
            },
            registry_package: None,
            protocol_versions: vec![AgentProtocolVersion::AcpV1],
            auth_modes: vec![AgentAuthMode::AgentNative],
            capabilities: vec![
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::SessionCancel,
                AgentPluginCapability::SessionSetMode,
                AgentPluginCapability::SessionSetModel,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
            permission_profile: AgentPermissionProfile::RuntimeGated,
            experimental_methods: vec!["_kiro.dev/commands/execute".to_string()],
            config_schema_version: 1,
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: AgentPluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.agent_id, "kiro-cli");
        assert_eq!(decoded.transport, AgentTransport::Acp);
        assert_eq!(decoded.command.command, "kiro-cli");
        assert!(
            decoded
                .capabilities
                .contains(&AgentPluginCapability::ToolCalls)
        );
    }
}
