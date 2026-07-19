#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

mod agent_commands;
mod brief_commands;
mod command_dispatch;
mod context_bundle;
mod doctor;
mod event_journal;
mod extension_commands;
mod formatting;
mod git_commands;
mod lane_runtime;
mod lane_supervisor;
mod lane_worker;
mod lsp_tools;
mod presentation;
mod provider_commands;
mod runtime_contract;
mod runtime_loop;
mod runtime_supervisor;
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
pub use runtime_supervisor::RuntimeSupervisor;
use viden_lsp::{LspRuntime, LspServerRegistry};
use viden_permissions::PermissionEngine;
use viden_plugin_api::{ContextReducerAdapterConfig, ContextReducerDescriptor};
use viden_plugin_host::ContextReducerCircuitBreaker;
use viden_provider::{ModelProvider, ProviderDescriptor, ProviderHost};
use viden_session::SessionStore;
use viden_tools::ToolRegistry;
#[cfg(test)]
use viden_types::PermissionRule;
use viden_types::{
    AgentDagRecord, AgentTaskRecord, ContextBundleRecord, CostScope, CostUsageRecord, EvidenceView,
    MemoryEntry, MergeGateRecord, Message, ModelUsage, PermissionLevel, PermissionMode,
    ResolvedUiPreferences, RuntimeEvent, RuntimeSnapshot, TaskRecord, WorkMode, now_timestamp,
};
use viden_workflows::stores::WorkflowStore;

const PROVIDER_REASONING_CONTENT_KEY: &str = "__provider_reasoning_content";
const LANE_STATE_UNAVAILABLE_MESSAGE: &str = "invalid or unreadable lane event log";

pub(crate) type RuntimeEventSink = Arc<dyn Fn(Vec<RuntimeEvent>) + Send + Sync + 'static>;

const COST_ATTRIBUTION_ID_MAX_CHARS: usize = 96;

#[derive(Debug, Clone, Default)]
pub(crate) struct CostAttribution {
    pub(crate) request_id: Option<String>,
    pub(crate) agent_task_id: Option<String>,
    pub(crate) dag_id: Option<String>,
    pub(crate) workflow_id: Option<String>,
    pub(crate) smoke_run_id: Option<String>,
}

impl CostAttribution {
    pub(crate) fn with_request_id(&self, request_id: impl AsRef<str>) -> Self {
        let mut next = self.clone();
        next.request_id = bounded_cost_id(request_id.as_ref());
        next
    }

    pub(crate) fn scopes(&self) -> Vec<CostScope> {
        let mut scopes = Vec::new();
        push_scope(
            &mut scopes,
            self.request_id
                .as_ref()
                .map(|id| CostScope::Request(id.clone())),
        );
        push_scope(
            &mut scopes,
            self.agent_task_id
                .as_ref()
                .map(|id| CostScope::AgentTask(id.clone())),
        );
        push_scope(
            &mut scopes,
            self.dag_id.as_ref().map(|id| CostScope::Dag(id.clone())),
        );
        push_scope(
            &mut scopes,
            self.workflow_id
                .as_ref()
                .map(|id| CostScope::Workflow(id.clone())),
        );
        push_scope(
            &mut scopes,
            self.smoke_run_id
                .as_ref()
                .map(|id| CostScope::SmokeRun(id.clone())),
        );
        scopes
    }
}

fn push_scope(scopes: &mut Vec<CostScope>, scope: Option<CostScope>) {
    if let Some(scope) = scope
        && !scopes.contains(&scope)
    {
        scopes.push(scope);
    }
}

fn bounded_cost_id(input: &str) -> Option<String> {
    let bounded = input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .take(COST_ATTRIBUTION_ID_MAX_CHARS)
        .collect::<String>();
    (!bounded.is_empty()).then_some(bounded)
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    System(String),
    Assistant(String),
    ToolCall(String),
    ToolResult {
        output: String,
        success: bool,
        exit_code: Option<i32>,
    },
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
    pub last_cached_input_tokens: Option<u64>,
    pub last_total_tokens: Option<u64>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cached_input_tokens: u64,
    pub total_tokens: u64,
    pub last_tokens_per_second: Option<u64>,
    pub last_cost_micro_usd: Option<u64>,
    pub total_cost_micro_usd: Option<u64>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextBenchmarkProjectionMode {
    Off,
    On,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContextBenchmarkMetrics {
    pub(crate) request_input_chars: usize,
    pub(crate) projection_chars: usize,
    pub(crate) raw_baseline_chars: usize,
    pub(crate) context_event_count: usize,
    pub(crate) retrieval_count: usize,
    pub(crate) retry_count: u64,
    pub(crate) compression_ratio: f64,
    pub(crate) bundle_build_ms: u128,
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
            self.last_cached_input_tokens = None;
            self.last_total_tokens = None;
            self.last_tokens_per_second = None;
            self.last_cost_micro_usd = None;
            return;
        };
        self.last_input_tokens = usage.input_tokens;
        self.last_output_tokens = usage.output_tokens;
        self.last_cached_input_tokens = usage.cached_input_tokens;
        self.last_total_tokens = usage.total_tokens;
        if let Some(input_tokens) = usage.input_tokens {
            self.total_input_tokens = self.total_input_tokens.saturating_add(input_tokens);
        }
        if let Some(output_tokens) = usage.output_tokens {
            self.total_output_tokens = self.total_output_tokens.saturating_add(output_tokens);
        }
        if let Some(cached_input_tokens) = usage.cached_input_tokens {
            self.total_cached_input_tokens = self
                .total_cached_input_tokens
                .saturating_add(cached_input_tokens);
        }
        if let Some(total_tokens) = usage.total_tokens {
            self.total_tokens = self.total_tokens.saturating_add(total_tokens);
            self.last_tokens_per_second = tokens_per_second(total_tokens, latency);
        } else {
            self.last_tokens_per_second = None;
        }
        self.last_cost_micro_usd = usage.actual_cost_micro_usd.or(usage.cost_micro_usd);
        if let Some(cost) = usage.actual_cost_micro_usd.or(usage.cost_micro_usd) {
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
    user_config_path_override: Option<PathBuf>,
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
    runtime_agent_dags: Vec<AgentDagRecord>,
    runtime_merge_gates: Vec<MergeGateRecord>,
    runtime_evidence: Vec<EvidenceView>,
    queued_runtime_inputs: Vec<runtime_contract::QueuedRuntimeInput>,
    runtime_event_sink: Option<RuntimeEventSink>,
    provider_telemetry: ProviderTelemetry,
    provider_cost_usage: Vec<CostUsageRecord>,
    transaction_file_rollback: RefCell<Vec<FileRollback>>,
    cost_workflow_id: Option<String>,
    cost_smoke_run_id: Option<String>,
    active_cost_attribution: Option<CostAttribution>,
    last_context_bundle: Option<ContextBundleRecord>,
    last_context_runtime_events: Vec<RuntimeEvent>,
    context_engine_root: PathBuf,
    context_budget_override: Option<(u64, u64)>,
    context_reducer_config: ContextReducerAdapterConfig,
    context_reducer_descriptor: Option<ContextReducerDescriptor>,
    context_reducer_breaker: RefCell<ContextReducerCircuitBreaker>,
    #[cfg(test)]
    context_benchmark_projection_mode: Option<ContextBenchmarkProjectionMode>,
    #[cfg(test)]
    last_context_benchmark_metrics: Option<ContextBenchmarkMetrics>,
    #[cfg(test)]
    context_reducer_test_behavior: Option<ContextReducerTestBehavior>,
    #[cfg(test)]
    fail_next_workflow_append: Cell<bool>,
    #[cfg(test)]
    fail_workflow_append_after: Cell<Option<usize>>,
    #[cfg(test)]
    fail_transcript_append_after: Cell<Option<usize>>,
}

#[derive(Debug, Clone)]
pub(crate) struct FileRollback {
    pub(crate) root: PathBuf,
    pub(crate) path: PathBuf,
    pub(crate) contents: Option<Vec<u8>>,
    pub(crate) permissions: Option<std::fs::Permissions>,
    pub(crate) created_parent_dirs: Vec<PathBuf>,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) enum ContextReducerTestBehavior {
    Output(String),
    SleepThenOutput { sleep_ms: u64, content: String },
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
            work_mode: WorkMode::Build,
            permission_mode: PermissionMode::Default,
            permission_level: PermissionLevel::Ask,
            config_summary: format!(
                "provider={} model={} work_mode={} permission_mode={} session_home=<default> timeout=<unknown> retries=<unknown>",
                provider.provider_name(),
                provider.model(),
                WorkMode::Build.cli_name(),
                PermissionMode::Default.cli_name()
            ),
            loaded_config_files: Vec::new(),
            startup_overrides: Vec::new(),
            ui_preferences: ResolvedUiPreferences::default(),
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
        let legacy_lanes_path = cwd.join(".viden").join("lanes.tsv");
        if legacy_lanes_path.is_file() {
            workflows.import_legacy_lanes_tsv_once(
                &legacy_lanes_path,
                now_timestamp(),
                Some(store.session_id().to_string()),
            )?;
        }
        let context_engine_root = cwd.join(".viden").join("context-engine");
        let cost_workflow_id =
            Some(bounded_cost_id(store.session_id()).unwrap_or_else(|| "session".to_string()));
        let engine = Self {
            cwd: cwd.clone(),
            provider,
            provider_host: None,
            provider_api_base: None,
            provider_api_key: None,
            provider_plugin_dirs: Vec::new(),
            provider_request_timeout_secs: 90,
            provider_max_retries: 1,
            user_config_path_override: None,
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
            runtime_agent_dags: Vec::new(),
            runtime_merge_gates: Vec::new(),
            runtime_evidence: Vec::new(),
            queued_runtime_inputs: Vec::new(),
            runtime_event_sink: None,
            provider_telemetry: ProviderTelemetry::default(),
            provider_cost_usage: Vec::new(),
            transaction_file_rollback: RefCell::new(Vec::new()),
            cost_workflow_id,
            cost_smoke_run_id: None,
            active_cost_attribution: None,
            last_context_bundle: None,
            last_context_runtime_events: Vec::new(),
            context_engine_root,
            context_budget_override: None,
            context_reducer_config: ContextReducerAdapterConfig::default(),
            context_reducer_descriptor: None,
            context_reducer_breaker: RefCell::new(ContextReducerCircuitBreaker::default()),
            #[cfg(test)]
            context_benchmark_projection_mode: None,
            #[cfg(test)]
            last_context_benchmark_metrics: None,
            #[cfg(test)]
            context_reducer_test_behavior: None,
            #[cfg(test)]
            fail_next_workflow_append: Cell::new(false),
            #[cfg(test)]
            fail_workflow_append_after: Cell::new(None),
            #[cfg(test)]
            fail_transcript_append_after: Cell::new(None),
        };
        engine.persist_meta("work_mode", engine.runtime_snapshot.work_mode.cli_name())?;
        engine.persist_meta("permission_mode", engine.permissions.mode().cli_name())?;
        let model = engine.provider.model().to_string();
        engine.persist_meta("model", &model)?;
        Ok(engine)
    }

    pub(crate) fn set_runtime_event_sink(&mut self, sink: Option<RuntimeEventSink>) {
        self.runtime_event_sink = sink;
    }

    pub(crate) fn runtime_event_sink(&self) -> Option<RuntimeEventSink> {
        self.runtime_event_sink.clone()
    }

    pub(crate) fn cost_attribution_for_request(
        &self,
        request_id: &str,
        default_agent_task_id: Option<&str>,
    ) -> CostAttribution {
        let mut attribution = self.active_cost_attribution.clone().unwrap_or_default();
        attribution.request_id = bounded_cost_id(request_id);
        if attribution.agent_task_id.is_none() {
            attribution.agent_task_id = default_agent_task_id.and_then(bounded_cost_id);
        }
        if attribution.dag_id.is_none()
            && let Some(agent_task_id) = attribution.agent_task_id.as_deref()
        {
            attribution.dag_id = self.dag_id_for_task_optional(agent_task_id);
        }
        if attribution.workflow_id.is_none() {
            attribution.workflow_id = self.cost_workflow_id.clone();
        }
        if attribution.smoke_run_id.is_none() {
            attribution.smoke_run_id = self.cost_smoke_run_id.clone();
        }
        attribution
    }

    pub(crate) fn cost_attribution_for_context_scope(
        &self,
        request_id: &str,
        scope: &viden_types::ContextScope,
    ) -> CostAttribution {
        let mut attribution = self.cost_attribution_for_request(request_id, None);
        match scope {
            viden_types::ContextScope::Task(id) => {
                attribution.agent_task_id = bounded_cost_id(id);
                attribution.dag_id = self.dag_id_for_task_optional(id);
            }
            viden_types::ContextScope::Dag(id) => {
                attribution.dag_id = bounded_cost_id(id);
            }
            viden_types::ContextScope::Workflow(id) => {
                attribution.workflow_id = bounded_cost_id(id);
            }
        }
        attribution
    }

    pub(crate) fn dag_id_for_task_optional(&self, task_id: &str) -> Option<String> {
        self.runtime_agent_dags
            .iter()
            .find(|dag| dag.tasks.iter().any(|task| task.task_id == task_id))
            .and_then(|dag| bounded_cost_id(&dag.dag_id))
    }

    #[cfg(test)]
    pub(crate) fn add_permission_rule_for_test(&mut self, rule: PermissionRule) {
        self.permissions.add_rule(rule);
    }

    #[cfg(test)]
    pub(crate) fn set_cost_workflow_id_for_test(&mut self, workflow_id: Option<&str>) {
        self.cost_workflow_id = workflow_id.and_then(bounded_cost_id);
    }

    #[cfg(test)]
    pub(crate) fn set_cost_smoke_run_id_for_test(&mut self, smoke_run_id: Option<&str>) {
        self.cost_smoke_run_id = smoke_run_id.and_then(bounded_cost_id);
    }

    #[cfg(test)]
    pub(crate) fn set_context_benchmark_projection_mode_for_test(
        &mut self,
        mode: ContextBenchmarkProjectionMode,
    ) {
        self.context_benchmark_projection_mode = Some(mode);
    }

    #[cfg(test)]
    pub(crate) fn context_benchmark_metrics_for_test(&self) -> Option<ContextBenchmarkMetrics> {
        self.last_context_benchmark_metrics.clone()
    }

    #[cfg(test)]
    pub(crate) fn seed_context_benchmark_history_for_test(
        &mut self,
        marker: &str,
    ) -> Result<(), String> {
        for index in 0..12 {
            let user = Message::new(
                viden_types::Role::User,
                format!(
                    "benchmark history user {index} {marker} {}",
                    "raw-context ".repeat(240)
                ),
            );
            self.messages.push(user.clone());
            self.store_entry(viden_types::TranscriptEntry::Message { message: user })?;
            let assistant = Message::new(
                viden_types::Role::Assistant,
                format!(
                    "benchmark history assistant {index} {marker} {}",
                    "projected-answer ".repeat(180)
                ),
            );
            self.messages.push(assistant.clone());
            self.store_entry(viden_types::TranscriptEntry::Message { message: assistant })?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_runtime_dag_id_for_test(&mut self, task_id: &str, dag_id: &str) {
        if let Some(dag) = self
            .runtime_agent_dags
            .iter_mut()
            .find(|dag| dag.tasks.iter().any(|task| task.task_id == task_id))
            && let Some(bounded) = bounded_cost_id(dag_id)
        {
            dag.dag_id = bounded;
        }
    }

    #[cfg(test)]
    pub(crate) fn fail_next_workflow_append_for_test(&self) {
        self.fail_next_workflow_append.set(true);
    }

    #[cfg(test)]
    pub(crate) fn fail_after_workflow_appends_for_test(&self, successful_appends: usize) {
        self.fail_workflow_append_after
            .set(Some(successful_appends));
    }

    #[cfg(test)]
    pub(crate) fn fail_after_transcript_appends_for_test(&self, successful_appends: usize) {
        self.fail_transcript_append_after
            .set(Some(successful_appends));
    }

    #[cfg(test)]
    pub(crate) fn clear_permission_rules_for_test(&mut self) {
        let mode = self.permissions.mode();
        self.permissions
            .restore_context(viden_permissions::PermissionContext {
                mode,
                ..viden_permissions::PermissionContext::default()
            });
    }

    pub fn session_id(&self) -> &str {
        self.store.session_id()
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub(crate) fn workflow_store(&self) -> WorkflowStore {
        self.workflows.clone()
    }

    pub(crate) fn lane_permission_engine(&self) -> PermissionEngine {
        self.permissions.clone()
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

    #[cfg(test)]
    pub(crate) fn set_user_config_path_override(&mut self, path: PathBuf) {
        self.user_config_path_override = Some(path);
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
        self.runtime_snapshot.permission_mode = mode;
        self.runtime_snapshot.permission_level = PermissionLevel::from_legacy_mode(mode);
        if mode == PermissionMode::Plan {
            self.runtime_snapshot.work_mode = WorkMode::Plan;
            self.persist_meta("work_mode", WorkMode::Plan.cli_name())?;
        } else if self.runtime_snapshot.work_mode == WorkMode::Plan {
            self.runtime_snapshot.work_mode = WorkMode::Build;
            self.persist_meta("work_mode", WorkMode::Build.cli_name())?;
        }
        self.persist_meta("permission_mode", mode.cli_name())
    }

    pub fn work_mode(&self) -> WorkMode {
        self.runtime_snapshot.work_mode
    }

    pub fn permission_level(&self) -> PermissionLevel {
        self.runtime_snapshot.permission_level
    }

    pub fn set_work_mode(&mut self, mode: WorkMode) -> Result<(), String> {
        self.runtime_snapshot.work_mode = mode;
        self.persist_meta("work_mode", mode.cli_name())?;
        match mode {
            WorkMode::Plan | WorkMode::Review | WorkMode::Explore => {
                self.permissions.set_mode(PermissionMode::Plan);
                self.runtime_snapshot.permission_mode = PermissionMode::Plan;
                self.runtime_snapshot.permission_level = PermissionLevel::ReadOnly;
                self.persist_meta("permission_mode", PermissionMode::Plan.cli_name())?;
            }
            WorkMode::Build => {
                if self.permissions.mode() == PermissionMode::Plan {
                    self.permissions.set_mode(PermissionMode::Default);
                    self.runtime_snapshot.permission_mode = PermissionMode::Default;
                    self.runtime_snapshot.permission_level = PermissionLevel::Ask;
                    self.persist_meta("permission_mode", PermissionMode::Default.cli_name())?;
                } else {
                    self.runtime_snapshot.permission_level =
                        PermissionLevel::from_legacy_mode(self.permissions.mode());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
