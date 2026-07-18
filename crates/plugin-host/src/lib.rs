//! Static plugin registry boundary for Viden runtime integrations.
//!
//! Dynamic loading stays in provider-specific code for now. This host crate is
//! the shared place for plugin discovery, validation, and lifecycle contracts as
//! tools, agents, workflows, and providers move behind the plugin API.

use std::collections::HashMap;

use viden_plugin_api::{
    AgentAuthMode, AgentCommandSpec, AgentEnvRef, AgentPermissionProfile, AgentPluginCapability,
    AgentPluginDescriptor, AgentProtocolVersion, AgentRegistryPackage, AgentSource, AgentTransport,
    CONTEXT_REDUCER_SCHEMA_VERSION, ContextReducerAdapterConfig, ContextReducerContentKind,
    ContextReducerDescriptor, ContextReducerHealthMetadata, ContextReducerHealthStatus,
    ContextReducerRequest, ContextReducerResponse, PluginKind, PluginManifest,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticPluginRegistry {
    manifests: Vec<PluginManifest>,
    agent_descriptors: Vec<AgentPluginDescriptor>,
    context_reducers: Vec<ContextReducerDescriptor>,
}

impl StaticPluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, manifest: PluginManifest) {
        self.manifests.push(manifest);
    }

    pub fn register_agent(&mut self, descriptor: AgentPluginDescriptor) {
        self.agent_descriptors.push(descriptor);
    }

    pub fn register_context_reducer(
        &mut self,
        descriptor: ContextReducerDescriptor,
    ) -> Result<(), PluginHostError> {
        if self
            .context_reducers
            .iter()
            .any(|registered| registered.reducer_id == descriptor.reducer_id)
        {
            return Err(PluginHostError::DuplicateContextReducer {
                reducer_id: descriptor.reducer_id,
            });
        }
        self.context_reducers.push(descriptor);
        Ok(())
    }

    pub fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }

    pub fn agent_descriptors(&self) -> &[AgentPluginDescriptor] {
        &self.agent_descriptors
    }

    pub fn context_reducers(&self) -> &[ContextReducerDescriptor] {
        &self.context_reducers
    }

    pub fn agent_by_id(&self, id: &str) -> Option<&AgentPluginDescriptor> {
        self.agent_descriptors
            .iter()
            .find(|descriptor| descriptor.agent_id == id)
    }

    pub fn by_kind(&self, kind: PluginKind) -> impl Iterator<Item = &PluginManifest> {
        self.manifests
            .iter()
            .filter(move |manifest| manifest.kind == kind)
    }

    pub fn negotiate_context_reducer(
        &self,
        config: &ContextReducerAdapterConfig,
        kind: ContextReducerContentKind,
        schema_version: u32,
    ) -> Option<&ContextReducerDescriptor> {
        if !config.enabled {
            return None;
        }
        self.context_reducers.iter().find(|descriptor| {
            config
                .preferred_reducer_id
                .as_deref()
                .is_none_or(|preferred| preferred == descriptor.reducer_id)
                && descriptor
                    .supported_schema_versions
                    .contains(&schema_version)
                && descriptor.content_kinds.contains(&kind)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginHostError {
    DuplicateContextReducer { reducer_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextReducerHostError {
    AdapterCrash,
    MalformedResponse,
}

pub type ContextReducerExecutor = Box<
    dyn Fn(&ContextReducerRequest) -> Result<ContextReducerResponse, ContextReducerHostError>
        + Send,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextReducerHostOutcome {
    pub response: ContextReducerResponse,
    pub health: ContextReducerHealthMetadata,
    pub used_native_fallback: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ContextReducerCircuitBreaker {
    failures: HashMap<String, u32>,
}

impl ContextReducerCircuitBreaker {
    pub fn failure_count(&self, reducer_id: &str) -> u32 {
        self.failures.get(reducer_id).copied().unwrap_or(0)
    }

    fn record_success(&mut self, reducer_id: &str) {
        self.failures.remove(reducer_id);
    }

    fn record_failure(&mut self, reducer_id: &str) {
        let failure_count = self.failures.entry(reducer_id.to_string()).or_default();
        *failure_count = failure_count.saturating_add(1);
    }

    fn is_open(&self, reducer_id: &str, config: &ContextReducerAdapterConfig) -> bool {
        let threshold = config.circuit_breaker.failure_threshold.max(1);
        self.failure_count(reducer_id) >= threshold
    }
}

pub fn execute_context_reducer<N>(
    config: &ContextReducerAdapterConfig,
    descriptor: &ContextReducerDescriptor,
    request: ContextReducerRequest,
    executor: Option<ContextReducerExecutor>,
    native_fallback: N,
) -> ContextReducerHostOutcome
where
    N: FnOnce(&ContextReducerRequest) -> ContextReducerResponse,
{
    let mut breaker = ContextReducerCircuitBreaker::default();
    execute_context_reducer_with_breaker(
        config,
        descriptor,
        request,
        executor,
        native_fallback,
        &mut breaker,
    )
}

pub fn execute_context_reducer_with_breaker<N>(
    config: &ContextReducerAdapterConfig,
    descriptor: &ContextReducerDescriptor,
    request: ContextReducerRequest,
    executor: Option<ContextReducerExecutor>,
    native_fallback: N,
    breaker: &mut ContextReducerCircuitBreaker,
) -> ContextReducerHostOutcome
where
    N: FnOnce(&ContextReducerRequest) -> ContextReducerResponse,
{
    if !config.enabled {
        return fallback_outcome(
            &request,
            native_fallback,
            health(
                ContextReducerHealthStatus::Disabled,
                0,
                "context reducer disabled",
            ),
        );
    }
    if let Err(message) = validate_request(&request) {
        return fallback_outcome(
            &request,
            native_fallback,
            health(ContextReducerHealthStatus::PolicyRejected, 0, message),
        );
    }
    if !descriptor
        .supported_schema_versions
        .contains(&request.schema_version)
        || request.schema_version != CONTEXT_REDUCER_SCHEMA_VERSION
    {
        return fallback_outcome(
            &request,
            native_fallback,
            health(
                ContextReducerHealthStatus::VersionMismatch,
                0,
                "unsupported context reducer schema version",
            ),
        );
    }
    if !descriptor.content_kinds.contains(&request.content_kind) {
        return fallback_outcome(
            &request,
            native_fallback,
            health(
                ContextReducerHealthStatus::PolicyRejected,
                0,
                "content kind not supported by reducer",
            ),
        );
    }
    if breaker.is_open(&descriptor.reducer_id, config) {
        return fallback_outcome(
            &request,
            native_fallback,
            health(
                ContextReducerHealthStatus::CircuitOpen,
                0,
                "context reducer circuit breaker open",
            ),
        );
    }
    let Some(executor) = executor else {
        breaker.record_failure(&descriptor.reducer_id);
        return fallback_outcome(
            &request,
            native_fallback,
            health(
                ContextReducerHealthStatus::AdapterAbsent,
                0,
                "context reducer executor absent",
            ),
        );
    };

    let response = match executor(&request) {
        Ok(response) => response,
        Err(ContextReducerHostError::AdapterCrash) => {
            breaker.record_failure(&descriptor.reducer_id);
            return fallback_outcome(
                &request,
                native_fallback,
                health(
                    ContextReducerHealthStatus::Crash,
                    0,
                    "context reducer crashed",
                ),
            );
        }
        Err(ContextReducerHostError::MalformedResponse) => {
            breaker.record_failure(&descriptor.reducer_id);
            return fallback_outcome(
                &request,
                native_fallback,
                health(
                    ContextReducerHealthStatus::Malformed,
                    0,
                    "context reducer returned malformed JSON",
                ),
            );
        }
    };

    match validate_response(config, descriptor, &request, &response) {
        Ok(()) => {
            breaker.record_success(&descriptor.reducer_id);
            ContextReducerHostOutcome {
                health: response.health.clone(),
                response,
                used_native_fallback: false,
            }
        }
        Err(health) => {
            breaker.record_failure(&descriptor.reducer_id);
            fallback_outcome(&request, native_fallback, health)
        }
    }
}

fn validate_request(request: &ContextReducerRequest) -> Result<(), &'static str> {
    if request.schema_version != CONTEXT_REDUCER_SCHEMA_VERSION {
        return Err("unsupported context reducer request version");
    }
    for value in [
        request.canonical.reference.as_str(),
        request.permission_snapshot_ref.as_str(),
        request.canonical.item_id.as_str(),
    ] {
        if contains_path_or_secret(value) {
            return Err("context reducer request contains path-like or secret-like data");
        }
    }
    Ok(())
}

fn validate_response(
    config: &ContextReducerAdapterConfig,
    descriptor: &ContextReducerDescriptor,
    request: &ContextReducerRequest,
    response: &ContextReducerResponse,
) -> Result<(), ContextReducerHealthMetadata> {
    if response.health.latency_ms > config.timeout_ms {
        return Err(health(
            ContextReducerHealthStatus::Timeout,
            response.health.latency_ms,
            "context reducer timeout",
        ));
    }
    if response.schema_version != request.schema_version {
        return Err(health(
            ContextReducerHealthStatus::VersionMismatch,
            response.health.latency_ms,
            "context reducer response schema mismatch",
        ));
    }
    if response.request_id != request.request_id
        || response.canonical_hash != request.canonical.content_sha256
        || response.scope != request.scope
    {
        return Err(health(
            ContextReducerHealthStatus::BindingMismatch,
            response.health.latency_ms,
            "context reducer response does not bind request/hash/scope",
        ));
    }
    let max_output_bytes = config
        .max_output_bytes
        .min(descriptor.limits.max_output_bytes)
        .min(request.policy.max_output_bytes);
    let max_output_items = config
        .max_output_items
        .min(descriptor.limits.max_output_items);
    if response.reduced_content.len() > max_output_bytes
        || response.omissions.len() > max_output_items
        || response.reduced_content.lines().count() > max_output_items
        || max_structural_depth(&response.reduced_content)
            > config.max_depth.min(descriptor.limits.max_depth)
    {
        return Err(health(
            ContextReducerHealthStatus::Oversize,
            response.health.latency_ms,
            "context reducer response exceeds bounded output limits",
        ));
    }
    if !response.quality.passed
        || response.quality.score_microunits < config.min_quality_score_microunits
        || response.quality.evidence_recall_microunits < config.min_evidence_recall_microunits
    {
        return Err(health(
            ContextReducerHealthStatus::QualityFailed,
            response.health.latency_ms,
            "context reducer quality threshold failed",
        ));
    }
    if contains_path_or_secret(&response.reduced_content) {
        return Err(health(
            ContextReducerHealthStatus::PolicyRejected,
            response.health.latency_ms,
            "context reducer response contains path-like or secret-like data",
        ));
    }
    Ok(())
}

fn fallback_outcome<N>(
    request: &ContextReducerRequest,
    native_fallback: N,
    fallback_health: ContextReducerHealthMetadata,
) -> ContextReducerHostOutcome
where
    N: FnOnce(&ContextReducerRequest) -> ContextReducerResponse,
{
    ContextReducerHostOutcome {
        response: native_fallback(request),
        health: fallback_health,
        used_native_fallback: true,
    }
}

fn health(
    status: ContextReducerHealthStatus,
    latency_ms: u64,
    message: &'static str,
) -> ContextReducerHealthMetadata {
    ContextReducerHealthMetadata {
        latency_ms,
        status,
        message: Some(message.to_string()),
    }
}

fn contains_path_or_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.contains("/Users/")
        || value.contains("\\Users\\")
        || value.contains("://")
        || lower.contains("storage_path")
        || lower.contains("api_key")
        || lower.contains("credential")
        || lower.contains("password")
        || value.contains("sk-")
}

fn max_structural_depth(value: &str) -> usize {
    let mut depth = 0_usize;
    let mut max_depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '[' | '{' | '(' => {
                depth = depth.saturating_add(1);
                max_depth = max_depth.max(depth);
            }
            ']' | '}' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max_depth
}

pub fn builtin_agent_descriptors() -> Vec<AgentPluginDescriptor> {
    vec![
        registry_acp_agent(
            "claude-acp",
            "Claude Agent",
            "0.56.0",
            "@agentclientprotocol/claude-agent-acp",
            &[
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::SessionLoad,
                AgentPluginCapability::SessionCancel,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
        ),
        registry_acp_agent(
            "codex-acp",
            "Codex",
            "1.1.0",
            "@agentclientprotocol/codex-acp",
            &[
                AgentPluginCapability::SessionPrompt,
                AgentPluginCapability::SessionCancel,
                AgentPluginCapability::StreamingUpdates,
                AgentPluginCapability::ToolCalls,
            ],
        ),
        local_kiro_agent(),
    ]
}

fn registry_acp_agent(
    agent_id: &str,
    display_name: &str,
    version: &str,
    package: &str,
    capabilities: &[AgentPluginCapability],
) -> AgentPluginDescriptor {
    AgentPluginDescriptor {
        agent_id: agent_id.to_string(),
        display_name: display_name.to_string(),
        version: version.to_string(),
        transport: AgentTransport::Acp,
        source: AgentSource::Registry,
        command: AgentCommandSpec {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), format!("{package}@{version}")],
            env: Vec::new(),
        },
        registry_package: Some(AgentRegistryPackage {
            package: package.to_string(),
            version: version.to_string(),
        }),
        protocol_versions: vec![AgentProtocolVersion::AcpV1],
        auth_modes: vec![AgentAuthMode::AgentNative],
        capabilities: capabilities.to_vec(),
        permission_profile: AgentPermissionProfile::RuntimeGated,
        experimental_methods: Vec::new(),
        config_schema_version: 1,
    }
}

fn local_kiro_agent() -> AgentPluginDescriptor {
    AgentPluginDescriptor {
        agent_id: "kiro-cli".to_string(),
        display_name: "Kiro CLI".to_string(),
        version: "local".to_string(),
        transport: AgentTransport::Acp,
        source: AgentSource::LocalCommand,
        command: AgentCommandSpec {
            command: "kiro-cli".to_string(),
            args: vec!["acp".to_string()],
            env: vec![
                AgentEnvRef {
                    name: "VIDEN_KIRO_AGENT".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_MODEL".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_EFFORT".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_TRUST_ALL_TOOLS".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_TRUST_TOOLS".to_string(),
                    required: false,
                },
                AgentEnvRef {
                    name: "VIDEN_KIRO_AGENT_ENGINE".to_string(),
                    required: false,
                },
            ],
        },
        registry_package: None,
        protocol_versions: vec![AgentProtocolVersion::AcpV1],
        auth_modes: vec![AgentAuthMode::AgentNative],
        capabilities: vec![
            AgentPluginCapability::SessionPrompt,
            AgentPluginCapability::SessionLoad,
            AgentPluginCapability::SessionCancel,
            AgentPluginCapability::SessionSetMode,
            AgentPluginCapability::SessionSetModel,
            AgentPluginCapability::StreamingUpdates,
            AgentPluginCapability::ToolCalls,
            AgentPluginCapability::ImageInput,
            AgentPluginCapability::SlashCommands,
            AgentPluginCapability::McpEvents,
        ],
        permission_profile: AgentPermissionProfile::RuntimeGated,
        experimental_methods: vec![
            "_kiro.dev/commands/available".to_string(),
            "_kiro.dev/commands/options".to_string(),
            "_kiro.dev/commands/execute".to_string(),
            "_kiro.dev/mcp/oauth_request".to_string(),
            "_kiro.dev/mcp/server_initialized".to_string(),
            "_kiro.dev/compaction/status".to_string(),
            "_kiro.dev/clear/status".to_string(),
            "_session/terminate".to_string(),
        ],
        config_schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viden_plugin_api::{
        AgentPluginCapability, AgentSource, AgentTransport, ContextReducerCanonicalRef,
        ContextReducerCircuitBreakerConfig, ContextReducerLimits, ContextReducerOmission,
        ContextReducerPolicy, ContextReducerQualityFacts, ContextReducerScope, PluginCapability,
        PluginPermission,
    };

    #[test]
    fn registry_filters_static_plugins_by_kind() {
        let mut registry = StaticPluginRegistry::new();
        registry.register(PluginManifest {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            version: "1".to_string(),
            kind: PluginKind::Provider,
            capabilities: vec![PluginCapability::Provider],
            permissions: vec![PluginPermission::Network],
            config_schema_version: 1,
        });

        assert_eq!(registry.by_kind(PluginKind::Provider).count(), 1);
        assert_eq!(registry.by_kind(PluginKind::Tool).count(), 0);
    }

    #[test]
    fn registry_resolves_static_agents_by_id() {
        let mut registry = StaticPluginRegistry::new();
        let kiro = builtin_agent_descriptors()
            .into_iter()
            .find(|agent| agent.agent_id == "kiro-cli")
            .expect("kiro descriptor exists");

        registry.register_agent(kiro);

        let descriptor = registry
            .agent_by_id("kiro-cli")
            .expect("registered descriptor resolves by id");
        assert_eq!(descriptor.transport, AgentTransport::Acp);
        assert_eq!(descriptor.source, AgentSource::LocalCommand);
        assert_eq!(descriptor.command.command, "kiro-cli");
        assert_eq!(descriptor.command.args, vec!["acp"]);
        let env_names = descriptor
            .command
            .env
            .iter()
            .map(|env| env.name.as_str())
            .collect::<Vec<_>>();
        assert!(env_names.contains(&"VIDEN_KIRO_AGENT"));
        assert!(env_names.contains(&"VIDEN_KIRO_MODEL"));
        assert!(env_names.contains(&"VIDEN_KIRO_EFFORT"));
        assert!(env_names.contains(&"VIDEN_KIRO_TRUST_ALL_TOOLS"));
        assert!(env_names.contains(&"VIDEN_KIRO_TRUST_TOOLS"));
        assert!(env_names.contains(&"VIDEN_KIRO_AGENT_ENGINE"));
        assert!(
            descriptor
                .capabilities
                .contains(&AgentPluginCapability::SessionSetModel)
        );
    }

    #[test]
    fn builtin_acp_agents_cover_claude_codex_and_kiro() {
        let agents = builtin_agent_descriptors();
        let ids = agents
            .iter()
            .map(|agent| agent.agent_id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"claude-acp"));
        assert!(ids.contains(&"codex-acp"));
        assert!(ids.contains(&"kiro-cli"));
        assert!(
            agents
                .iter()
                .all(|agent| agent.transport == AgentTransport::Acp)
        );
        let claude = agents
            .iter()
            .find(|agent| agent.agent_id == "claude-acp")
            .expect("claude descriptor");
        assert_eq!(claude.version, "0.56.0");
        assert_eq!(
            claude.command.args,
            vec!["-y", "@agentclientprotocol/claude-agent-acp@0.56.0"]
        );
    }

    fn context_reducer_descriptor(id: &str) -> ContextReducerDescriptor {
        ContextReducerDescriptor {
            reducer_id: id.to_string(),
            display_name: "Neutral Reducer".to_string(),
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
        }
    }

    fn context_reducer_request() -> ContextReducerRequest {
        ContextReducerRequest {
            schema_version: CONTEXT_REDUCER_SCHEMA_VERSION,
            request_id: "ctxred-1".to_string(),
            content_kind: ContextReducerContentKind::Log,
            canonical: ContextReducerCanonicalRef {
                item_id: "ctxi-1".to_string(),
                content_sha256: "ab".repeat(32),
                evidence_id: None,
                reference: "context-item:ctxi-1".to_string(),
            },
            policy: ContextReducerPolicy {
                max_output_bytes: 128,
                max_output_tokens: 64,
                max_input_bytes: 4096,
                max_depth: 8,
                max_output_items: 16,
                required_markers: vec!["first_failure".to_string()],
            },
            scope: ContextReducerScope {
                role: "executor".to_string(),
                task_id: "task-1".to_string(),
                dag_id: None,
                workflow_id: Some("wf-1".to_string()),
            },
            permission_snapshot_ref: "perm-snap-1".to_string(),
            native_baseline_quality: Some(ContextReducerQualityFacts {
                passed: true,
                score_microunits: 950_000,
                evidence_recall_microunits: 950_000,
                checks: vec!["native".to_string()],
                deterministic_fingerprint: "native-fp".to_string(),
            }),
        }
    }

    fn context_reducer_response(request: &ContextReducerRequest) -> ContextReducerResponse {
        ContextReducerResponse {
            schema_version: request.schema_version,
            request_id: request.request_id.clone(),
            canonical_hash: request.canonical.content_sha256.clone(),
            scope: request.scope.clone(),
            reduced_content: "ERROR src/a.rs:1 boom".to_string(),
            omissions: vec![ContextReducerOmission {
                reason: "deduplicated".to_string(),
                omitted_count: 1,
            }],
            reducer_id: "adapter".to_string(),
            reducer_version: "0.1.0".to_string(),
            quality: ContextReducerQualityFacts {
                passed: true,
                score_microunits: 990_000,
                evidence_recall_microunits: 990_000,
                checks: vec!["first_failure_retained".to_string()],
                deterministic_fingerprint: "adapter-fp".to_string(),
            },
            health: ContextReducerHealthMetadata {
                latency_ms: 4,
                status: ContextReducerHealthStatus::Ok,
                message: None,
            },
        }
    }

    fn native_response(request: &ContextReducerRequest) -> ContextReducerResponse {
        let mut response = context_reducer_response(request);
        response.reducer_id = "viden-context-native".to_string();
        response.reducer_version = "native-v1".to_string();
        response.reduced_content = "native fallback".to_string();
        response
    }

    #[test]
    fn context_reducer_registration_rejects_duplicates_and_negotiates_only_when_enabled() {
        let mut registry = StaticPluginRegistry::new();
        registry
            .register_context_reducer(context_reducer_descriptor("adapter"))
            .unwrap();

        assert!(
            registry
                .register_context_reducer(context_reducer_descriptor("adapter"))
                .is_err()
        );
        assert!(
            registry
                .negotiate_context_reducer(
                    &ContextReducerAdapterConfig::default(),
                    ContextReducerContentKind::Log,
                    CONTEXT_REDUCER_SCHEMA_VERSION,
                )
                .is_none()
        );

        let config = ContextReducerAdapterConfig {
            enabled: true,
            preferred_reducer_id: Some("adapter".to_string()),
            ..ContextReducerAdapterConfig::default()
        };
        let selected = registry
            .negotiate_context_reducer(
                &config,
                ContextReducerContentKind::Log,
                CONTEXT_REDUCER_SCHEMA_VERSION,
            )
            .unwrap();

        assert_eq!(selected.reducer_id, "adapter");
    }

    #[test]
    fn context_reducer_accepts_valid_external_response() {
        let request = context_reducer_request();
        let config = ContextReducerAdapterConfig {
            enabled: true,
            ..ContextReducerAdapterConfig::default()
        };
        let outcome = execute_context_reducer(
            &config,
            &context_reducer_descriptor("adapter"),
            request.clone(),
            Some(Box::new(|request| Ok(context_reducer_response(request)))),
            native_response,
        );

        assert_eq!(outcome.response.reducer_id, "adapter");
        assert!(!outcome.used_native_fallback);
        assert_eq!(outcome.health.status, ContextReducerHealthStatus::Ok);
    }

    #[test]
    fn context_reducer_falls_back_on_absent_timeout_crash_malformed_and_binding_failures() {
        let config = ContextReducerAdapterConfig {
            enabled: true,
            timeout_ms: 1,
            ..ContextReducerAdapterConfig::default()
        };
        let descriptor = context_reducer_descriptor("adapter");

        let cases: Vec<(&str, Option<ContextReducerExecutor>)> = vec![
            ("absent", None),
            (
                "timeout",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.health.latency_ms = 20;
                    Ok(response)
                })),
            ),
            (
                "crash",
                Some(Box::new(|_| Err(ContextReducerHostError::AdapterCrash))),
            ),
            (
                "malformed",
                Some(Box::new(|_| {
                    Err(ContextReducerHostError::MalformedResponse)
                })),
            ),
            (
                "wrong_version",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.schema_version = 999;
                    Ok(response)
                })),
            ),
            (
                "wrong_hash",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.canonical_hash = "cd".repeat(32);
                    Ok(response)
                })),
            ),
            (
                "wrong_scope",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.scope.task_id = "task-2".to_string();
                    Ok(response)
                })),
            ),
            (
                "oversize",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.reduced_content = "x".repeat(1024);
                    Ok(response)
                })),
            ),
            (
                "too_many_items",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.reduced_content = (0..64)
                        .map(|index| format!("line {index}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(response)
                })),
            ),
            (
                "too_deep",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.reduced_content = "[[[[[[[[[too deep]]]]]]]]]".to_string();
                    Ok(response)
                })),
            ),
            (
                "quality",
                Some(Box::new(|request| {
                    let mut response = context_reducer_response(request);
                    response.quality.passed = false;
                    response.quality.evidence_recall_microunits = 1;
                    Ok(response)
                })),
            ),
        ];

        for (name, executor) in cases {
            let outcome = execute_context_reducer(
                &config,
                &descriptor,
                context_reducer_request(),
                executor,
                native_response,
            );
            assert!(outcome.used_native_fallback, "{name}");
            assert_eq!(
                outcome.response.reducer_id, "viden-context-native",
                "{name}"
            );
            assert_ne!(
                outcome.health.status,
                ContextReducerHealthStatus::Ok,
                "{name}"
            );
        }
    }

    #[test]
    fn context_reducer_circuit_breaker_skips_repeated_failures() {
        let config = ContextReducerAdapterConfig {
            enabled: true,
            circuit_breaker: ContextReducerCircuitBreakerConfig {
                failure_threshold: 2,
                backoff_ms: 30_000,
            },
            ..ContextReducerAdapterConfig::default()
        };
        let descriptor = context_reducer_descriptor("adapter");
        let mut breaker = ContextReducerCircuitBreaker::default();

        for _ in 0..2 {
            let outcome = execute_context_reducer_with_breaker(
                &config,
                &descriptor,
                context_reducer_request(),
                Some(Box::new(|_| Err(ContextReducerHostError::AdapterCrash))),
                native_response,
                &mut breaker,
            );
            assert!(outcome.used_native_fallback);
        }

        let outcome = execute_context_reducer_with_breaker(
            &config,
            &descriptor,
            context_reducer_request(),
            Some(Box::new(|request| Ok(context_reducer_response(request)))),
            native_response,
            &mut breaker,
        );

        assert!(outcome.used_native_fallback);
        assert_eq!(
            outcome.health.status,
            ContextReducerHealthStatus::CircuitOpen
        );
        assert_eq!(breaker.failure_count("adapter"), 2);
    }

    #[test]
    fn context_reducer_request_validation_rejects_paths_and_secrets_before_adapter_call() {
        let config = ContextReducerAdapterConfig {
            enabled: true,
            ..ContextReducerAdapterConfig::default()
        };
        let descriptor = context_reducer_descriptor("adapter");
        let mut request = context_reducer_request();
        request.canonical.reference = "/Users/wiki/private/context".to_string();
        request.permission_snapshot_ref = "sk-test-secret".to_string();

        let outcome = execute_context_reducer(
            &config,
            &descriptor,
            request,
            Some(Box::new(|request| Ok(context_reducer_response(request)))),
            native_response,
        );

        assert!(outcome.used_native_fallback);
        assert_eq!(
            outcome.health.status,
            ContextReducerHealthStatus::PolicyRejected
        );
    }
}
