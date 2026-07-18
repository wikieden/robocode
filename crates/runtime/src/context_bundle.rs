use std::process::Command;

use sha2::{Digest, Sha256};
use viden_context::{
    ContextEngine, ContextPutRequest, ReductionEstimate, ReductionOmission, ReductionPolicy,
    ReductionResult, reduce,
};
use viden_plugin_api::{
    CONTEXT_REDUCER_SCHEMA_VERSION, ContextReducerCanonicalRef, ContextReducerContentKind,
    ContextReducerDescriptor, ContextReducerHealthMetadata, ContextReducerHealthStatus,
    ContextReducerOmission, ContextReducerPolicy, ContextReducerQualityFacts,
    ContextReducerRequest, ContextReducerResponse, ContextReducerScope,
};
use viden_plugin_host::{ContextReducerExecutor, execute_context_reducer_with_breaker};
use viden_types::{
    ContextBudgetRecord, ContextBundleRecord, ContextContentKind, ContextHandleRecord,
    ContextOmittedSourceRecord, ContextScope, ContextSourceRecord, ContextViewRecord, RuntimeEvent,
    RuntimeEventKind, fresh_id, now_timestamp, truncate_for_preview,
};

use crate::{SessionEngine, TestEvidence};

const MAIN_SOFT_BUDGET: u64 = 48_000;
const MAIN_HARD_LIMIT: u64 = 128_000;
const STRICT_RETRY_SOFT_BUDGET: u64 = 12_000;
const STRICT_RETRY_HARD_LIMIT: u64 = 32_000;
const PROVIDER_SOURCE_SNIPPET_CHARS: usize = 320;
const PROVIDER_MANIFEST_SNIPPET_CHARS: usize = 4_000;

pub(crate) struct BuiltContextBundle {
    pub(crate) bundle: ContextBundleRecord,
    pub(crate) events: Vec<RuntimeEvent>,
    pub(crate) hard_exceeded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextBuildMode {
    Normal,
    RequestTooLargeRetry,
}

#[derive(Debug, Clone)]
struct ContextSourceDraft {
    record: ContextSourceRecord,
    content: String,
    content_kind: ContextContentKind,
}

impl SessionEngine {
    pub fn provider_context_bundle(&self) -> Option<ContextBundleRecord> {
        self.last_context_bundle.clone()
    }

    pub(crate) fn build_main_context_bundle(&self, input: &str) -> ContextBundleRecord {
        self.build_main_context_bundle_with_mode(input, ContextBuildMode::Normal)
            .bundle
    }

    pub(crate) fn build_main_context_bundle_with_mode(
        &self,
        input: &str,
        mode: ContextBuildMode,
    ) -> BuiltContextBundle {
        let mut sources = vec![
            context_source("user-task", "task", input, 100, 240),
            context_source(
                "workspace",
                "workspace",
                &format!(
                    "project workspace on {} with {} dirty files",
                    git_branch_label(self),
                    dirty_file_count(self).unwrap_or(0)
                ),
                90,
                320,
            ),
        ];
        if let Some(brief) = self.active_brief_snapshot() {
            sources.push(context_source(
                "active-brief",
                "brief",
                &format!("{}: {}", brief.id, brief.goal),
                96,
                420,
            ));
        }
        let steering = self
            .steering_summaries()
            .into_iter()
            .take(3)
            .map(|(file, summary)| format!("{file}: {}", compact_text(&summary, 4)))
            .collect::<Vec<_>>()
            .join("\n");
        if !steering.trim().is_empty() {
            sources.push(context_source(
                "project-steering",
                "steering-summary",
                &steering,
                82,
                520,
            ));
        }
        if let Some(diff) = self
            .last_diff
            .as_deref()
            .filter(|diff| !diff.trim().is_empty())
        {
            sources.push(context_source(
                "latest-diff",
                "diff",
                &compact_text(diff, 18),
                80,
                720,
            ));
        }
        if let Some(test) = self.last_test.as_ref() {
            sources.push(context_source(
                "latest-test",
                "test",
                &test_summary(test),
                85,
                520,
            ));
        }
        let transcript = self
            .messages
            .iter()
            .rev()
            .take(6)
            .map(|message| {
                format!(
                    "{:?}: {}",
                    message.role,
                    compact_text(&message.content, 2).replace('\n', " ")
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        if !transcript.trim().is_empty() {
            sources.push(context_source(
                "recent-transcript",
                "transcript-summary",
                &transcript,
                70,
                480,
            ));
        }
        let tasks = self
            .workflows
            .load_task_state()
            .ok()
            .map(|state| {
                state
                    .active_tasks()
                    .into_iter()
                    .take(4)
                    .map(|task| format!("{} {:?} {}", task.task_id, task.status, task.title))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if !tasks.trim().is_empty() {
            sources.push(context_source(
                "active-tasks",
                "task-summary",
                &tasks,
                75,
                360,
            ));
        }
        let memory = self
            .workflows
            .load_memory_state()
            .ok()
            .map(|state| {
                state
                    .active_project_memory()
                    .into_iter()
                    .chain(state.active_session_memory(self.session_id()))
                    .take(4)
                    .map(|entry| format!("{:?} {}", entry.kind, entry.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if !memory.trim().is_empty() {
            sources.push(context_source("memory", "memory", &memory, 65, 360));
        }
        let runtime = self
            .runtime_tasks
            .iter()
            .rev()
            .take(4)
            .map(|task| format!("{} {} {}", task.kind, task.status, task.activity))
            .collect::<Vec<_>>()
            .join("\n");
        if !runtime.trim().is_empty() {
            sources.push(context_source(
                "runtime-tasks",
                "runtime-summary",
                &runtime,
                72,
                360,
            ));
        }
        let (soft_budget, hard_limit) = self.context_budget_for_mode(mode);
        let bundle_id = match mode {
            ContextBuildMode::Normal => format!("ctx-main-{}", self.session_id()),
            ContextBuildMode::RequestTooLargeRetry => {
                format!("ctx-main-{}-retry", self.session_id())
            }
        };
        let task_id = format!("turn-{}", self.session_id());
        let scope = ContextScope::Task(task_id.clone());
        let mut context_events = Vec::new();
        let mut handle_ids = Vec::new();
        let mut materialized_sources = Vec::new();
        let mut quality_events = Vec::new();
        let mut context_engine = ContextEngine::open(&self.context_engine_root)
            .map_err(|err| format!("context engine open failed: {err}"));
        for source in sources {
            let source_result = match context_engine.as_mut() {
                Ok(engine) => self.materialize_context_source(engine, &scope, &source, mode),
                Err(err) => Err(err.clone()),
            };
            match source_result {
                Ok((record, source_events, source_handle_ids)) => {
                    handle_ids.extend(source_handle_ids);
                    context_events.extend(source_events);
                    materialized_sources.push(record);
                }
                Err(err) => {
                    quality_events.push(RuntimeEvent::new(
                        1,
                        RuntimeEventKind::ContextQualityFailed {
                            quality: viden_types::ContextQualityRecord {
                                quality_id: fresh_id("ctxq"),
                                target_id: source.record.name.clone(),
                                passed: false,
                                score_microunits: None,
                                checks: vec!["context_source_materialized".to_string()],
                                failure_reason: Some(err),
                                checked_at: Some(now_timestamp()),
                            },
                        },
                    ));
                }
            }
        }
        context_events.extend(quality_events);
        let mut sources = materialized_sources;
        let mut omitted_sources = omit_sources_over_soft_budget(&mut sources, soft_budget);
        let hard_omitted = omit_sources_over_hard_limit(&mut sources, hard_limit);
        omitted_sources.extend(hard_omitted);
        let estimated_tokens = sources.iter().fold(0_u64, |sum, source| {
            sum.saturating_add(source.estimated_tokens)
        });
        let largest_sources = largest_sources(&sources);
        let mut compaction_notes = vec![
            "context sources are stored canonically and reduced into provider views".to_string(),
            "raw transcript and tool audit remain in session storage".to_string(),
            "role and evidence sources are preserved before lower-priority summaries".to_string(),
        ];
        if mode == ContextBuildMode::RequestTooLargeRetry {
            compaction_notes.push(
                "request-too-large retry uses stricter deterministic context policy".to_string(),
            );
        }
        if !omitted_sources.is_empty() {
            compaction_notes.push(format!(
                "{} source(s) omitted by budget policy",
                omitted_sources.len()
            ));
        }
        if estimated_tokens > soft_budget {
            compaction_notes.push(
                "soft budget exceeded; low-priority sources should be trimmed first".to_string(),
            );
        }
        if estimated_tokens > hard_limit {
            compaction_notes
                .push("hard limit exceeded; provider input rejected before transport".to_string());
        }
        let bundle = ContextBundleRecord {
            bundle_id: bundle_id.clone(),
            task_id,
            policy: match mode {
                ContextBuildMode::Normal => "v1-priority-budget".to_string(),
                ContextBuildMode::RequestTooLargeRetry => {
                    "v1-priority-budget-strict-retry".to_string()
                }
            },
            sources,
            omitted_sources,
            estimated_tokens,
            largest_sources,
            compaction_notes,
            soft_token_budget: soft_budget,
            hard_token_limit: hard_limit,
        };
        context_events.push(RuntimeEvent::new(
            1,
            RuntimeEventKind::ContextBundleBuilt {
                bundle_id,
                scope: scope.clone(),
                handle_ids,
                estimated_tokens,
            },
        ));
        let budget = context_budget_record(&bundle, scope);
        let hard_exceeded = budget.exceeded;
        if hard_exceeded || estimated_tokens > soft_budget {
            context_events.push(RuntimeEvent::new(
                1,
                RuntimeEventKind::ContextBudgetExceeded { budget },
            ));
        }
        BuiltContextBundle {
            bundle,
            events: context_events,
            hard_exceeded,
        }
    }

    pub(crate) fn render_context_command(&self) -> String {
        let Some(bundle) = self.provider_context_bundle() else {
            return "No provider ContextBundle yet. Send a provider turn first, then run `/context`."
                .to_string();
        };
        render_context_bundle_detail(&bundle)
    }
}

pub(crate) fn render_context_bundle_detail(bundle: &ContextBundleRecord) -> String {
    let mut sources = bundle.sources.clone();
    sources.sort_by_key(|source| {
        (
            std::cmp::Reverse(source.priority),
            std::cmp::Reverse(source.estimated_tokens),
        )
    });
    let source_rows = sources
        .iter()
        .map(|source| {
            format!(
                "  p{:<3} {:<18} {:<18} {:>6} tok  {}",
                source.priority,
                source.name,
                source.kind,
                source.estimated_tokens,
                source.include_reason
            )
        })
        .collect::<Vec<_>>();
    let omitted_rows = bundle
        .omitted_sources
        .iter()
        .map(|source| {
            format!(
                "  {:<18} {:<18} {:>6} tok  {}",
                source.name, source.kind, source.estimated_tokens, source.reason
            )
        })
        .collect::<Vec<_>>();
    [
        "ContextBundle:".to_string(),
        format!("  Bundle: {}", bundle.bundle_id),
        format!("  Policy: {}", bundle.policy),
        format!(
            "  Pressure: {}% ({}/{})",
            bundle.pressure_percent(),
            bundle.estimated_tokens,
            bundle.hard_token_limit
        ),
        format!("  Soft budget: {}", bundle.soft_token_budget),
        format!("  Sources: {}", bundle.sources.len()),
        "Sources by priority:".to_string(),
        if source_rows.is_empty() {
            "  <none>".to_string()
        } else {
            source_rows.join("\n")
        },
        "Omitted sources:".to_string(),
        if omitted_rows.is_empty() {
            "  <none>".to_string()
        } else {
            omitted_rows.join("\n")
        },
        "Compaction notes:".to_string(),
        list_or_none(&bundle.compaction_notes),
    ]
    .join("\n")
}

pub(crate) fn render_provider_context_message(bundle: &ContextBundleRecord) -> String {
    let mut remaining_snippet_chars = PROVIDER_MANIFEST_SNIPPET_CHARS;
    let sources = bundle
        .sources
        .iter()
        .map(|source| {
            let snippet = provider_source_snippet(source, &mut remaining_snippet_chars);
            format!(
                "- {} [{}] ~{} tok handle={} item={} view={} raw_hash={} view_hash={} quality={}\n  Snippet: {}",
                source.name,
                source.kind,
                source.estimated_tokens,
                source.handle_id.as_deref().unwrap_or("<pending>"),
                source.item_id.as_deref().unwrap_or("<pending>"),
                source.view_id.as_deref().unwrap_or("<pending>"),
                source.content_sha256.as_deref().unwrap_or("<pending>"),
                source.view_sha256.as_deref().unwrap_or("<pending>"),
                source.quality_id.as_deref().unwrap_or("<pending>"),
                snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Viden ContextBundle\nBundle: {}\nScope: task:{}\nPolicy: {}\nEstimated tokens: {}\nContext pressure: {}%\nSoft budget: {}\nHard limit: {}\nSources:\n{}\nOmitted sources:\n{}\nCompaction notes:\n{}",
        bundle.bundle_id,
        stable_scope_id(&bundle.task_id),
        bundle.policy,
        bundle.estimated_tokens,
        bundle.pressure_percent(),
        bundle.soft_token_budget,
        bundle.hard_token_limit,
        if sources.is_empty() {
            "- <none>".to_string()
        } else {
            sources
        },
        list_omitted_or_none(&bundle.omitted_sources),
        list_or_none(&bundle.compaction_notes)
    )
}

fn provider_source_snippet(source: &ContextSourceRecord, remaining: &mut usize) -> String {
    if *remaining == 0 {
        return "<omitted by provider manifest bound>".to_string();
    }
    let max_chars = PROVIDER_SOURCE_SNIPPET_CHARS.min(*remaining);
    let safe = redact_for_event(&source.summary.replace('\n', " "));
    let snippet = truncate_for_preview(&safe, max_chars);
    *remaining = remaining.saturating_sub(snippet.chars().count());
    if snippet.is_empty() {
        "<empty>".to_string()
    } else {
        snippet
    }
}

pub(crate) fn context_evidence_rows(bundle: &ContextBundleRecord) -> Vec<String> {
    let mut rows = vec![
        format!("context_bundle {}", bundle.bundle_id),
        format!(
            "context_pressure {}% ({}/{})",
            bundle.pressure_percent(),
            bundle.estimated_tokens,
            bundle.hard_token_limit
        ),
        format!("context_sources {}", bundle.sources.len()),
        format!("context_policy {}", bundle.policy),
    ];
    if !bundle.omitted_sources.is_empty() {
        rows.push(format!("context_omitted {}", bundle.omitted_sources.len()));
    }
    if let Some(source) = bundle.largest_sources.first() {
        rows.push(format!("largest_context_source {source}"));
    }
    if let Some(source) = bundle
        .sources
        .iter()
        .find(|source| source.name == "active-brief")
    {
        rows.push(format!("active_brief {}", compact_text(&source.summary, 1)));
    }
    rows
}

fn context_source(
    name: &str,
    kind: &str,
    summary: &str,
    priority: u8,
    minimum_tokens: u64,
) -> ContextSourceDraft {
    let record = ContextSourceRecord {
        name: name.to_string(),
        kind: kind.to_string(),
        priority,
        estimated_tokens: estimate_tokens(summary).max(minimum_tokens),
        summary: redact_for_event(summary),
        include_reason: format!("priority {priority}; selected by v1-priority-budget policy"),
        handle_id: None,
        item_id: None,
        view_id: None,
        content_sha256: None,
        view_sha256: None,
        quality_id: None,
    };
    ContextSourceDraft {
        record,
        content: summary.to_string(),
        content_kind: content_kind_for_source(kind),
    }
}

impl SessionEngine {
    pub(crate) fn context_budget_for_mode(&self, mode: ContextBuildMode) -> (u64, u64) {
        if let Some(override_budget) = self.context_budget_override {
            return override_budget;
        }
        match mode {
            ContextBuildMode::Normal => (MAIN_SOFT_BUDGET, MAIN_HARD_LIMIT),
            ContextBuildMode::RequestTooLargeRetry => {
                (STRICT_RETRY_SOFT_BUDGET, STRICT_RETRY_HARD_LIMIT)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn set_context_budget_for_test(
        &mut self,
        soft_token_limit: u64,
        hard_token_limit: u64,
    ) {
        self.context_budget_override = Some((soft_token_limit, hard_token_limit));
    }

    #[cfg(test)]
    pub(crate) fn set_context_engine_root_for_test(&mut self, root: std::path::PathBuf) {
        self.context_engine_root = root;
    }

    #[cfg(test)]
    pub(crate) fn set_context_reducer_adapter_for_test(
        &mut self,
        reducer_id: &str,
        version: &str,
        reduced_content: &str,
    ) {
        self.context_reducer_config = viden_plugin_api::ContextReducerAdapterConfig {
            enabled: true,
            preferred_reducer_id: Some(reducer_id.to_string()),
            ..viden_plugin_api::ContextReducerAdapterConfig::default()
        };
        self.context_reducer_descriptor = Some(ContextReducerDescriptor {
            reducer_id: reducer_id.to_string(),
            display_name: "Test Context Reducer".to_string(),
            version: version.to_string(),
            supported_schema_versions: vec![CONTEXT_REDUCER_SCHEMA_VERSION],
            content_kinds: vec![
                ContextReducerContentKind::Json,
                ContextReducerContentKind::Code,
                ContextReducerContentKind::Diff,
                ContextReducerContentKind::Log,
                ContextReducerContentKind::Diagnostic,
                ContextReducerContentKind::Transcript,
                ContextReducerContentKind::Text,
            ],
            limits: viden_plugin_api::ContextReducerLimits {
                max_input_bytes: 2 * 1024 * 1024,
                max_output_bytes: 16 * 1024,
                max_output_items: 512,
                max_depth: 64,
            },
            default_enabled: false,
            config_schema_version: 1,
        });
        self.context_reducer_test_behavior = Some(crate::ContextReducerTestBehavior::Output(
            reduced_content.to_string(),
        ));
    }

    #[cfg(test)]
    pub(crate) fn set_absent_context_reducer_adapter_for_test(
        &mut self,
        reducer_id: &str,
        version: &str,
    ) {
        self.set_context_reducer_adapter_for_test(reducer_id, version, "");
        self.context_reducer_test_behavior = None;
    }

    #[cfg(test)]
    pub(crate) fn set_disabled_context_reducer_adapter_for_test(
        &mut self,
        reducer_id: &str,
        version: &str,
    ) {
        self.set_context_reducer_adapter_for_test(reducer_id, version, "");
        self.context_reducer_config.enabled = false;
        self.context_reducer_test_behavior = None;
    }

    #[cfg(test)]
    pub(crate) fn set_sleeping_context_reducer_adapter_for_test(
        &mut self,
        reducer_id: &str,
        version: &str,
        sleep_ms: u64,
        timeout_ms: u64,
    ) {
        self.set_context_reducer_adapter_for_test(reducer_id, version, "adapter reduced");
        self.context_reducer_config.timeout_ms = timeout_ms;
        self.context_reducer_config
            .circuit_breaker
            .failure_threshold = 1;
        self.context_reducer_config.circuit_breaker.backoff_ms = 30_000;
        self.context_reducer_test_behavior =
            Some(crate::ContextReducerTestBehavior::SleepThenOutput {
                sleep_ms,
                content: "adapter reduced".to_string(),
            });
    }

    fn materialize_context_source(
        &self,
        engine: &mut ContextEngine,
        scope: &ContextScope,
        source: &ContextSourceDraft,
        mode: ContextBuildMode,
    ) -> Result<(ContextSourceRecord, Vec<RuntimeEvent>, Vec<String>), String> {
        let existing_handle = context_handle_from_source(&source.record, scope);
        let (content, item_event, mut handle) = if let Some(handle) = existing_handle {
            let content = engine
                .retrieve(&handle, scope)
                .map_err(|err| format!("context retrieve failed: {err}"))?;
            (content, None, handle)
        } else {
            let stored = engine
                .store(ContextPutRequest {
                    scope: scope.clone(),
                    kind: source.content_kind,
                    content: source.content.as_bytes(),
                    evidence_id: None,
                })
                .map_err(|err| format!("context store failed: {err}"))?;
            let mut item = stored.item;
            item.title = source.record.name.clone();
            item.summary = format!("{} {}", source.record.name, source.record.kind);
            item.token_count = source.content.len() as u64;
            (
                source.content.as_bytes().to_vec(),
                Some(RuntimeEvent::new(
                    1,
                    RuntimeEventKind::ContextItemStored { item },
                )),
                stored.handle,
            )
        };
        let policy = reduction_policy_for_source(source, mode);
        let reduced = self.reduce_context_source(source, scope, &handle, &content, &policy)?;
        let view_id = fresh_id("ctxv");
        let view = ContextViewRecord {
            view_id: view_id.clone(),
            item_id: handle.item_id.clone(),
            kind: source.content_kind,
            derivation: format!(
                "{}:{}:{}",
                reduced.reducer_id, reduced.reducer_version, source.record.name
            ),
            content_sha256: sha256_hex(reduced.content.as_bytes()),
            token_count: reduced.reduced.token_count,
            quality_id: Some(reduced.quality.quality_id.clone()),
            created_at: Some(now_timestamp()),
        };
        handle.preferred_view_id = Some(view_id);
        let mut record = source.record.clone();
        record.estimated_tokens = reduced.reduced.token_count;
        record.summary = self.redact_context_summary(&reduced.content);
        record.handle_id = Some(handle.handle_id.clone());
        record.item_id = Some(handle.item_id.clone());
        record.view_id = handle.preferred_view_id.clone();
        record.content_sha256 = Some(handle.content_sha256.clone());
        record.view_sha256 = Some(view.content_sha256.clone());
        record.quality_id = view.quality_id.clone();
        if reduced.reducer_id != "viden-context-native" {
            record.include_reason = format!(
                "{}; reducer {}:{} quality {:?}",
                record.include_reason,
                reduced.reducer_id,
                reduced.reducer_version,
                reduced.quality.score_microunits
            );
        }
        if !reduced.omissions.is_empty() {
            let reasons = reduced
                .omissions
                .iter()
                .map(|omission| format!("{}:{}", omission.reason, omission.omitted_count))
                .collect::<Vec<_>>()
                .join(",");
            record.include_reason =
                format!("{}; reduced omissions {reasons}", record.include_reason);
        }
        let mut events = Vec::new();
        if let Some(item_event) = item_event {
            events.push(item_event);
        }
        events.push(RuntimeEvent::new(
            1,
            RuntimeEventKind::ContextViewDerived {
                view,
                handle: handle.clone(),
            },
        ));
        Ok((record, events, vec![handle.handle_id]))
    }

    fn reduce_context_source(
        &self,
        source: &ContextSourceDraft,
        scope: &ContextScope,
        handle: &ContextHandleRecord,
        content: &[u8],
        policy: &ReductionPolicy,
    ) -> Result<ReductionResult, String> {
        let native = reduce(source.content_kind, content, policy)
            .map_err(|err| format!("context reduction failed: {err}"))?;
        let Some(descriptor) = self.context_reducer_descriptor.as_ref() else {
            return Ok(native);
        };
        if !self.context_reducer_config.enabled {
            return Ok(native);
        }
        let request =
            context_reducer_request(source, scope, handle, policy, native_quality_facts(&native));
        let native_response = context_reducer_response_from_native(&request, &native);
        let executor = self.context_reducer_executor_for_runtime(descriptor, &request);
        let outcome = execute_context_reducer_with_breaker(
            &self.context_reducer_config,
            descriptor,
            request,
            executor,
            |_| native_response,
            &mut self.context_reducer_breaker.borrow_mut(),
        );
        if outcome.used_native_fallback {
            return Ok(native);
        }
        Ok(reduction_result_from_adapter_response(
            source.content_kind,
            content,
            &outcome.response,
        ))
    }

    fn context_reducer_executor_for_runtime(
        &self,
        descriptor: &ContextReducerDescriptor,
        _request: &ContextReducerRequest,
    ) -> Option<ContextReducerExecutor> {
        #[cfg(not(test))]
        let _ = descriptor;
        #[cfg(test)]
        {
            let reducer_id = descriptor.reducer_id.clone();
            let reducer_version = descriptor.version.clone();
            self.context_reducer_test_behavior.clone().map(|behavior| {
                Box::new(move |request: ContextReducerRequest| {
                    let content = match behavior {
                        crate::ContextReducerTestBehavior::Output(content) => content,
                        crate::ContextReducerTestBehavior::SleepThenOutput {
                            sleep_ms,
                            content,
                        } => {
                            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
                            content
                        }
                    };
                    Ok(ContextReducerResponse {
                        schema_version: request.schema_version,
                        request_id: request.request_id.clone(),
                        canonical_hash: request.canonical.content_sha256.clone(),
                        permission_snapshot_ref: request.permission_snapshot_ref.clone(),
                        scope: request.scope.clone(),
                        content_kind: request.content_kind,
                        reduced_content: content.clone(),
                        omissions: Vec::new(),
                        reducer_id: reducer_id.clone(),
                        reducer_version: reducer_version.clone(),
                        quality: ContextReducerQualityFacts {
                            passed: true,
                            score_microunits: 990_000,
                            evidence_recall_microunits: 990_000,
                            checks: vec!["test_adapter_retained_required_context".to_string()],
                            deterministic_fingerprint: "test-adapter".to_string(),
                        },
                        health: ContextReducerHealthMetadata {
                            latency_ms: 1,
                            status: ContextReducerHealthStatus::Ok,
                            message: None,
                        },
                    })
                }) as ContextReducerExecutor
            })
        }
        #[cfg(not(test))]
        {
            None
        }
    }

    pub(crate) fn materialize_existing_context_bundle(
        &self,
        bundle: &ContextBundleRecord,
        mode: ContextBuildMode,
    ) -> BuiltContextBundle {
        let scope = ContextScope::Task(bundle.task_id.clone());
        let mut events = Vec::new();
        let mut handle_ids = Vec::new();
        let mut materialized_sources = Vec::new();
        let mut context_engine = ContextEngine::open(&self.context_engine_root)
            .map_err(|err| format!("context engine open failed: {err}"));
        for source in &bundle.sources {
            let draft = ContextSourceDraft {
                record: source.clone(),
                content: source.summary.clone(),
                content_kind: content_kind_for_source(&source.kind),
            };
            let source_result = match context_engine.as_mut() {
                Ok(engine) => self.materialize_context_source(engine, &scope, &draft, mode),
                Err(err) => Err(err.clone()),
            };
            match source_result {
                Ok((record, source_events, source_handle_ids)) => {
                    materialized_sources.push(record);
                    events.extend(source_events);
                    handle_ids.extend(source_handle_ids);
                }
                Err(err) => events.push(RuntimeEvent::new(
                    1,
                    RuntimeEventKind::ContextQualityFailed {
                        quality: viden_types::ContextQualityRecord {
                            quality_id: fresh_id("ctxq"),
                            target_id: source.name.clone(),
                            passed: false,
                            score_microunits: None,
                            checks: vec!["context_source_materialized".to_string()],
                            failure_reason: Some(err),
                            checked_at: Some(now_timestamp()),
                        },
                    },
                )),
            }
        }
        let mut final_bundle = bundle.clone();
        let (soft_budget, hard_limit) = self.context_budget_for_mode(mode);
        final_bundle.soft_token_budget = soft_budget;
        final_bundle.hard_token_limit = hard_limit;
        if mode == ContextBuildMode::RequestTooLargeRetry
            && !final_bundle.policy.ends_with("-strict-retry")
        {
            final_bundle.policy = format!("{}-strict-retry", final_bundle.policy);
        }
        final_bundle.sources = materialized_sources;
        let mut omitted_sources = omit_sources_over_soft_budget(
            &mut final_bundle.sources,
            final_bundle.soft_token_budget,
        );
        omitted_sources.extend(omit_sources_over_hard_limit(
            &mut final_bundle.sources,
            final_bundle.hard_token_limit,
        ));
        final_bundle.omitted_sources.extend(omitted_sources);
        final_bundle.estimated_tokens = final_bundle.sources.iter().fold(0_u64, |sum, source| {
            sum.saturating_add(source.estimated_tokens)
        });
        final_bundle.largest_sources = largest_sources(&final_bundle.sources);
        if final_bundle.estimated_tokens > final_bundle.hard_token_limit {
            final_bundle
                .compaction_notes
                .push("hard limit exceeded; provider input rejected before transport".to_string());
        }
        events.push(RuntimeEvent::new(
            1,
            RuntimeEventKind::ContextBundleBuilt {
                bundle_id: final_bundle.bundle_id.clone(),
                scope: scope.clone(),
                handle_ids,
                estimated_tokens: final_bundle.estimated_tokens,
            },
        ));
        let budget = context_budget_record(&final_bundle, scope);
        let hard_exceeded = budget.exceeded;
        if final_bundle.estimated_tokens > final_bundle.soft_token_budget || hard_exceeded {
            events.push(RuntimeEvent::new(
                1,
                RuntimeEventKind::ContextBudgetExceeded { budget },
            ));
        }
        BuiltContextBundle {
            bundle: final_bundle,
            events,
            hard_exceeded,
        }
    }

    fn redact_context_summary(&self, input: &str) -> String {
        redact_context_summary_for_event(&self.cwd, input)
    }
}

pub(crate) fn redact_context_summary_for_event(cwd: &std::path::Path, input: &str) -> String {
    let project_relative = preserve_project_relative_paths(input);
    let cwd = cwd.to_string_lossy();
    let cwd_with_separator = format!("{cwd}/");
    let project_relative = project_relative.replace(cwd_with_separator.as_str(), "");
    redact_for_event(&project_relative)
}

fn content_kind_for_source(kind: &str) -> ContextContentKind {
    match kind {
        "diff" => ContextContentKind::Diff,
        "test" | "runtime-summary" | "lsp-diagnostics" => ContextContentKind::Log,
        "transcript-summary" => ContextContentKind::Transcript,
        _ => ContextContentKind::Text,
    }
}

fn plugin_content_kind(kind: ContextContentKind) -> ContextReducerContentKind {
    match kind {
        ContextContentKind::Json => ContextReducerContentKind::Json,
        ContextContentKind::Code => ContextReducerContentKind::Code,
        ContextContentKind::Diff => ContextReducerContentKind::Diff,
        ContextContentKind::Log => ContextReducerContentKind::Log,
        ContextContentKind::Diagnostic => ContextReducerContentKind::Diagnostic,
        ContextContentKind::Transcript => ContextReducerContentKind::Transcript,
        ContextContentKind::Text => ContextReducerContentKind::Text,
    }
}

fn context_reducer_request(
    source: &ContextSourceDraft,
    scope: &ContextScope,
    handle: &ContextHandleRecord,
    policy: &ReductionPolicy,
    native_baseline_quality: ContextReducerQualityFacts,
) -> ContextReducerRequest {
    ContextReducerRequest {
        schema_version: CONTEXT_REDUCER_SCHEMA_VERSION,
        request_id: fresh_id("ctxred"),
        content_kind: plugin_content_kind(source.content_kind),
        canonical: ContextReducerCanonicalRef {
            item_id: handle.item_id.clone(),
            content_sha256: handle.content_sha256.clone(),
            evidence_id: None,
            reference: format!("context-item:{}", handle.item_id),
        },
        policy: ContextReducerPolicy {
            max_output_bytes: policy.max_output_bytes,
            max_output_tokens: policy.max_output_tokens,
            max_input_bytes: policy.max_input_bytes,
            max_depth: policy.max_json_depth,
            max_output_items: policy.max_json_values,
            required_markers: policy.required_markers.clone(),
        },
        scope: ContextReducerScope {
            role: "provider_context".to_string(),
            task_id: match scope {
                ContextScope::Task(task_id) => task_id.clone(),
                ContextScope::Dag(dag_id) => dag_id.clone(),
                ContextScope::Workflow(workflow_id) => workflow_id.clone(),
            },
            dag_id: match scope {
                ContextScope::Dag(dag_id) => Some(dag_id.clone()),
                _ => None,
            },
            workflow_id: match scope {
                ContextScope::Workflow(workflow_id) => Some(workflow_id.clone()),
                _ => None,
            },
        },
        permission_snapshot_ref: format!("permission-scope:{}", scope_label(scope)),
        native_baseline_quality: Some(native_baseline_quality),
    }
}

fn context_reducer_response_from_native(
    request: &ContextReducerRequest,
    native: &ReductionResult,
) -> ContextReducerResponse {
    ContextReducerResponse {
        schema_version: request.schema_version,
        request_id: request.request_id.clone(),
        canonical_hash: request.canonical.content_sha256.clone(),
        permission_snapshot_ref: request.permission_snapshot_ref.clone(),
        scope: request.scope.clone(),
        content_kind: request.content_kind,
        reduced_content: native.content.clone(),
        omissions: native
            .omissions
            .iter()
            .map(|omission| ContextReducerOmission {
                reason: omission.reason.clone(),
                omitted_count: omission.omitted_count,
            })
            .collect(),
        reducer_id: native.reducer_id.clone(),
        reducer_version: native.reducer_version.clone(),
        quality: native_quality_facts(native),
        health: ContextReducerHealthMetadata {
            latency_ms: 0,
            status: ContextReducerHealthStatus::Ok,
            message: None,
        },
    }
}

fn native_quality_facts(native: &ReductionResult) -> ContextReducerQualityFacts {
    ContextReducerQualityFacts {
        passed: native.quality.passed,
        score_microunits: native.quality.score_microunits.unwrap_or(0),
        evidence_recall_microunits: native.quality.score_microunits.unwrap_or(0),
        checks: native.quality.checks.clone(),
        deterministic_fingerprint: native.quality.quality_id.clone(),
    }
}

fn reduction_result_from_adapter_response(
    kind: ContextContentKind,
    original: &[u8],
    response: &ContextReducerResponse,
) -> ReductionResult {
    let original_estimate = reduction_estimate(original.len());
    let reduced_estimate = reduction_estimate(response.reduced_content.len());
    ReductionResult {
        content: response.reduced_content.clone(),
        original: original_estimate,
        reduced: reduced_estimate,
        omissions: response
            .omissions
            .iter()
            .map(|omission| ReductionOmission {
                reason: omission.reason.clone(),
                omitted_count: omission.omitted_count,
            })
            .collect(),
        retained_markers: response.quality.checks.clone(),
        reducer_id: response.reducer_id.clone(),
        reducer_version: response.reducer_version.clone(),
        quality: viden_types::ContextQualityRecord {
            quality_id: fresh_id("ctxq"),
            target_id: format!(
                "{}:{}:{:?}",
                response.reducer_id, response.reducer_version, kind
            ),
            passed: response.quality.passed,
            score_microunits: Some(response.quality.score_microunits),
            checks: response.quality.checks.clone(),
            failure_reason: None,
            checked_at: Some(now_timestamp()),
        },
        fallback_raw: false,
    }
}

fn reduction_estimate(byte_count: usize) -> ReductionEstimate {
    ReductionEstimate {
        byte_count,
        token_count: u64::try_from(byte_count.div_ceil(4)).unwrap_or(u64::MAX),
    }
}

fn scope_label(scope: &ContextScope) -> String {
    match scope {
        ContextScope::Task(id) => format!("task:{id}"),
        ContextScope::Dag(id) => format!("dag:{id}"),
        ContextScope::Workflow(id) => format!("workflow:{id}"),
    }
}

fn context_handle_from_source(
    source: &ContextSourceRecord,
    scope: &ContextScope,
) -> Option<ContextHandleRecord> {
    Some(ContextHandleRecord {
        handle_id: source.handle_id.clone()?,
        item_id: source.item_id.clone()?,
        preferred_view_id: source.view_id.clone(),
        content_sha256: source.content_sha256.clone()?,
        scope: scope.clone(),
        expires_at: None,
    })
}

fn reduction_policy_for_source(
    source: &ContextSourceDraft,
    mode: ContextBuildMode,
) -> ReductionPolicy {
    let mut policy = ReductionPolicy::default();
    let divisor = match mode {
        ContextBuildMode::Normal => 1,
        ContextBuildMode::RequestTooLargeRetry => 3,
    };
    let base_tokens = source.record.estimated_tokens.max(80);
    policy.max_output_tokens = base_tokens.saturating_div(divisor).clamp(16, 2_000);
    policy.max_output_bytes = usize::try_from(policy.max_output_tokens.saturating_mul(4))
        .unwrap_or(usize::MAX)
        .clamp(256, 8 * 1024);
    policy.max_input_bytes = 2 * 1024 * 1024;
    policy
}

fn omit_sources_over_soft_budget(
    sources: &mut Vec<ContextSourceRecord>,
    soft_budget: u64,
) -> Vec<ContextOmittedSourceRecord> {
    let mut total = sources.iter().fold(0_u64, |sum, source| {
        sum.saturating_add(source.estimated_tokens)
    });
    if total <= soft_budget {
        return Vec::new();
    }
    sources.sort_by_key(|source| {
        (
            std::cmp::Reverse(source.priority),
            std::cmp::Reverse(source.estimated_tokens),
            source.name.clone(),
        )
    });
    let mut omitted = Vec::new();
    let mut index = sources.len();
    while total > soft_budget && index > 0 {
        index -= 1;
        if sources[index].priority >= 90 {
            continue;
        }
        let source = sources.remove(index);
        total = total.saturating_sub(source.estimated_tokens);
        omitted.push(ContextOmittedSourceRecord {
            name: source.name,
            kind: source.kind,
            estimated_tokens: source.estimated_tokens,
            reason: format!(
                "soft budget {} exceeded; evicted by explicit priority {}",
                soft_budget, source.priority
            ),
        });
    }
    omitted
}

fn context_budget_record(bundle: &ContextBundleRecord, scope: ContextScope) -> ContextBudgetRecord {
    let remaining_tokens = bundle
        .hard_token_limit
        .saturating_sub(bundle.estimated_tokens);
    ContextBudgetRecord {
        budget_id: format!("ctxbudget-{}", bundle.bundle_id),
        scope,
        soft_token_limit: bundle.soft_token_budget,
        hard_token_limit: bundle.hard_token_limit,
        used_tokens: bundle.estimated_tokens,
        remaining_tokens,
        exceeded: bundle.estimated_tokens > bundle.hard_token_limit,
        updated_at: Some(now_timestamp()),
    }
}

fn stable_scope_id(input: &str) -> String {
    let bounded = input.chars().take(96).collect::<String>();
    redact_for_event(&bounded)
}

fn redact_for_event(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if lower.starts_with("sk-")
                || lower.contains("secret")
                || lower.contains("token=")
                || lower.contains("api_key")
                || word.starts_with('/')
                || word.contains("/Users/")
                || word.contains("/tmp/")
                || word.contains("/var/")
            {
                "[REDACTED]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn preserve_project_relative_paths(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            ["/crates/", "/apps/", "/plugins/", "/docs/"]
                .iter()
                .find_map(|marker| {
                    word.find(marker)
                        .map(|index| word[index.saturating_add(1)..].to_string())
                })
                .unwrap_or_else(|| word.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn omit_sources_over_hard_limit(
    sources: &mut Vec<ContextSourceRecord>,
    hard_limit: u64,
) -> Vec<ContextOmittedSourceRecord> {
    let mut total = sources
        .iter()
        .map(|source| source.estimated_tokens)
        .sum::<u64>();
    if total <= hard_limit {
        return Vec::new();
    }
    sources.sort_by_key(|source| std::cmp::Reverse(source.priority));
    let mut omitted = Vec::new();
    let mut index = sources.len();
    while total > hard_limit && index > 0 {
        index -= 1;
        if sources[index].priority >= 90 {
            continue;
        }
        let source = sources.remove(index);
        total = total.saturating_sub(source.estimated_tokens);
        omitted.push(ContextOmittedSourceRecord {
            name: source.name,
            kind: source.kind,
            estimated_tokens: source.estimated_tokens,
            reason: format!(
                "priority {} omitted to stay under hard token budget {}",
                source.priority, hard_limit
            ),
        });
    }
    omitted
}

fn estimate_tokens(text: &str) -> u64 {
    text.chars().count().saturating_div(4).max(1) as u64
}

fn compact_text(text: &str, max_lines: usize) -> String {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() <= max_lines {
        return lines.join("\n");
    }
    let head = max_lines / 2;
    let tail = max_lines.saturating_sub(head);
    let mut compacted = lines.iter().take(head).copied().collect::<Vec<_>>();
    compacted.push("<... summarized middle ...>");
    compacted.extend(lines.iter().skip(lines.len().saturating_sub(tail)).copied());
    compacted.join("\n")
}

fn test_summary(test: &TestEvidence) -> String {
    let exit = test
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    format!(
        "{} exit={} duration={}ms command={}\n{}",
        test.status,
        exit,
        test.duration_ms,
        test.command,
        compact_text(&test.output_tail, 8)
    )
}

fn largest_sources(sources: &[ContextSourceRecord]) -> Vec<String> {
    let mut rows = sources
        .iter()
        .map(|source| {
            (
                source.estimated_tokens,
                format!("{} {} tok", source.name, source.estimated_tokens),
            )
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));
    rows.into_iter().take(3).map(|(_, row)| row).collect()
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "- <none>".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("- {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn list_omitted_or_none(values: &[ContextOmittedSourceRecord]) -> String {
    if values.is_empty() {
        return "- <none>".to_string();
    }
    values
        .iter()
        .map(|value| {
            format!(
                "- {} [{}] ~{} tok: {}",
                value.name, value.kind, value.estimated_tokens, value.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn dirty_file_count(engine: &SessionEngine) -> Option<usize> {
    let output = Command::new("git")
        .args(["status", "--short", "--untracked-files=all"])
        .current_dir(&engine.cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
    )
}

fn git_branch_label(engine: &SessionEngine) -> String {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&engine.cwd)
        .output();
    output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
