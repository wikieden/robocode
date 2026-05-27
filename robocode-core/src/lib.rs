use std::collections::BTreeMap;
use std::sync::Arc;
use std::{path::PathBuf, time::Duration};

mod agent_commands;
mod command_dispatch;
mod doctor;
mod extension_commands;
mod formatting;
mod git_commands;
mod lsp_tools;
mod presentation;
mod provider_commands;
mod runtime_loop;
mod runtime_tasks;
mod runtime_views;
mod session_lifecycle;
mod test_commands;
mod web_commands;
mod workflow_commands;

#[cfg(test)]
pub(crate) use doctor::DependencyStatus;
pub(crate) use doctor::{DoctorReport, system_dependency_status};
use formatting::{format_relative_age, render_resume_context, render_task_detail};
use robocode_lsp::{LspRuntime, LspServerRegistry};
use robocode_model::{ModelProvider, ProviderDescriptor, ProviderHost};
use robocode_permissions::PermissionEngine;
use robocode_session::SessionStore;
use robocode_tools::ToolRegistry;
use robocode_types::{
    AgentTaskRecord, MemoryEntry, Message, ModelUsage, PermissionMode, RuntimeSnapshot, TaskRecord,
};
use robocode_workflows::stores::WorkflowStore;

const PROVIDER_REASONING_CONTENT_KEY: &str = "__provider_reasoning_content";

#[derive(Debug, Clone)]
pub enum EngineEvent {
    System(String),
    Assistant(String),
    ToolCall(String),
    ToolResult(String),
    Command(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProviderTelemetry {
    pub request_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_latency_ms: Option<u128>,
    pub average_latency_ms: Option<u128>,
    pub last_event_count: usize,
    pub last_error: Option<String>,
    pub last_input_tokens: Option<u64>,
    pub last_output_tokens: Option<u64>,
    pub last_total_tokens: Option<u64>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub last_tokens_per_second: Option<u64>,
    pub last_cost_micro_usd: Option<u64>,
    pub total_cost_micro_usd: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestEvidence {
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub failure_summary: Vec<String>,
    pub failing_files: Vec<String>,
    pub output_tail: String,
}

impl ProviderTelemetry {
    fn record_success(&mut self, latency: Duration, event_count: usize, usage: Option<ModelUsage>) {
        self.record_latency(latency);
        self.success_count += 1;
        self.last_event_count = event_count;
        self.last_error = None;
        self.record_usage(latency, usage);
    }

    fn record_failure(&mut self, latency: Duration, error: &str) {
        self.record_latency(latency);
        self.failure_count += 1;
        self.last_event_count = 0;
        self.last_error = Some(error.to_string());
    }

    fn record_latency(&mut self, latency: Duration) {
        let latency_ms = latency.as_millis();
        let previous_requests = self.request_count;
        self.request_count += 1;
        self.last_latency_ms = Some(latency_ms);
        self.average_latency_ms = Some(match self.average_latency_ms {
            Some(previous_average) => {
                ((previous_average * u128::from(previous_requests)) + latency_ms)
                    / u128::from(self.request_count)
            }
            None => latency_ms,
        });
    }

    fn record_usage(&mut self, latency: Duration, usage: Option<ModelUsage>) {
        let Some(usage) = usage else {
            self.last_input_tokens = None;
            self.last_output_tokens = None;
            self.last_total_tokens = None;
            self.last_tokens_per_second = None;
            self.last_cost_micro_usd = None;
            return;
        };
        self.last_input_tokens = usage.input_tokens;
        self.last_output_tokens = usage.output_tokens;
        self.last_total_tokens = usage.total_tokens;
        if let Some(input_tokens) = usage.input_tokens {
            self.total_input_tokens = self.total_input_tokens.saturating_add(input_tokens);
        }
        if let Some(output_tokens) = usage.output_tokens {
            self.total_output_tokens = self.total_output_tokens.saturating_add(output_tokens);
        }
        if let Some(total_tokens) = usage.total_tokens {
            self.total_tokens = self.total_tokens.saturating_add(total_tokens);
            self.last_tokens_per_second = tokens_per_second(total_tokens, latency);
        } else {
            self.last_tokens_per_second = None;
        }
        self.last_cost_micro_usd = usage.cost_micro_usd;
        if let Some(cost) = usage.cost_micro_usd {
            self.total_cost_micro_usd =
                Some(self.total_cost_micro_usd.unwrap_or(0).saturating_add(cost));
        }
    }
}

fn tokens_per_second(tokens: u64, latency: Duration) -> Option<u64> {
    let millis = latency.as_millis();
    if tokens == 0 || millis == 0 {
        None
    } else {
        Some(((u128::from(tokens) * 1000) / millis) as u64)
    }
}

pub struct SessionEngine {
    cwd: PathBuf,
    provider: Box<dyn ModelProvider>,
    provider_host: Option<ProviderHost>,
    provider_api_base: Option<String>,
    provider_api_key: Option<String>,
    provider_plugin_dirs: Vec<PathBuf>,
    provider_request_timeout_secs: u64,
    provider_max_retries: u32,
    tools: ToolRegistry,
    permissions: PermissionEngine,
    store: SessionStore,
    workflows: WorkflowStore,
    lsp_runtime: Arc<LspRuntime>,
    messages: Vec<Message>,
    last_diff: Option<String>,
    last_test: Option<TestEvidence>,
    runtime_snapshot: RuntimeSnapshot,
    runtime_tasks: Vec<AgentTaskRecord>,
    provider_telemetry: ProviderTelemetry,
}

impl SessionEngine {
    pub fn new(cwd: impl Into<PathBuf>, provider: Box<dyn ModelProvider>) -> Result<Self, String> {
        Self::new_with_home(cwd, provider, Option::<PathBuf>::None)
    }

    pub fn new_with_home(
        cwd: impl Into<PathBuf>,
        provider: Box<dyn ModelProvider>,
        home_override: Option<PathBuf>,
    ) -> Result<Self, String> {
        let cwd = cwd.into();
        let default_snapshot = RuntimeSnapshot {
            cwd: cwd.clone(),
            provider_family: provider.provider_name().to_string(),
            model_label: provider.model().to_string(),
            permission_mode: PermissionMode::Default,
            config_summary: format!(
                "provider={} model={} permission_mode={} session_home=<default> timeout=<unknown> retries=<unknown>",
                provider.provider_name(),
                provider.model(),
                PermissionMode::Default.cli_name()
            ),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
        };
        Self::new_with_home_and_snapshot(cwd, provider, home_override, default_snapshot)
    }

    pub fn new_with_home_and_snapshot(
        cwd: impl Into<PathBuf>,
        provider: Box<dyn ModelProvider>,
        home_override: Option<PathBuf>,
        runtime_snapshot: RuntimeSnapshot,
    ) -> Result<Self, String> {
        let cwd = cwd.into();
        let store = match home_override {
            Some(home) => SessionStore::new_with_home(home, &cwd, None)?,
            None => SessionStore::new(&cwd, None)?,
        };
        let workflows = WorkflowStore::new(store.home_dir().to_path_buf(), &cwd)?;
        let engine = Self {
            cwd: cwd.clone(),
            provider,
            provider_host: None,
            provider_api_base: None,
            provider_api_key: None,
            provider_plugin_dirs: Vec::new(),
            provider_request_timeout_secs: 90,
            provider_max_retries: 1,
            tools: ToolRegistry::builtin(),
            permissions: PermissionEngine::new(&cwd),
            store,
            workflows,
            lsp_runtime: Arc::new(LspRuntime::new(LspServerRegistry::default())),
            messages: Vec::new(),
            last_diff: None,
            last_test: None,
            runtime_snapshot,
            runtime_tasks: Vec::new(),
            provider_telemetry: ProviderTelemetry::default(),
        };
        engine.persist_meta("permission_mode", engine.permissions.mode().cli_name())?;
        let model = engine.provider.model().to_string();
        engine.persist_meta("model", &model)?;
        Ok(engine)
    }

    pub fn session_id(&self) -> &str {
        self.store.session_id()
    }

    pub fn provider_name(&self) -> &str {
        self.provider.provider_name()
    }

    pub fn model_name(&self) -> &str {
        self.provider.model()
    }

    pub fn provider_telemetry(&self) -> ProviderTelemetry {
        self.provider_telemetry.clone()
    }

    pub fn provider_descriptors(&self) -> Vec<ProviderDescriptor> {
        self.provider_host
            .as_ref()
            .map(|host| host.registry().descriptors().to_vec())
            .unwrap_or_default()
    }

    pub fn active_task_snapshot(&self) -> Result<Vec<TaskRecord>, String> {
        Ok(self
            .workflows
            .load_task_state()?
            .active_tasks()
            .into_iter()
            .cloned()
            .collect())
    }

    /// Returns only memory entries that can drive live operator actions.
    pub fn memory_snapshot(&self) -> Result<Vec<MemoryEntry>, String> {
        let state = self.workflows.load_memory_state()?;
        let mut entries = BTreeMap::new();
        for entry in state
            .pending_suggestions()
            .into_iter()
            .chain(state.active_project_memory())
            .chain(state.active_session_memory(self.session_id()))
        {
            entries.insert(entry.memory_id.clone(), entry.clone());
        }
        Ok(entries.into_values().collect())
    }

    pub fn set_provider_runtime(
        &mut self,
        provider_host: ProviderHost,
        provider_plugin_dirs: Vec<PathBuf>,
        api_base: Option<String>,
        api_key: Option<String>,
        request_timeout_secs: u64,
        max_retries: u32,
    ) {
        self.provider_host = Some(provider_host);
        self.provider_plugin_dirs = provider_plugin_dirs;
        self.provider_api_base = api_base;
        self.provider_api_key = api_key;
        self.provider_request_timeout_secs = request_timeout_secs.max(1);
        self.provider_max_retries = max_retries;
    }

    pub fn mode(&self) -> PermissionMode {
        self.permissions.mode()
    }

    pub fn set_permission_mode(&mut self, mode: PermissionMode) -> Result<(), String> {
        self.permissions.set_mode(mode);
        self.persist_meta("permission_mode", mode.cli_name())
    }
}

#[cfg(test)]
mod tests;
