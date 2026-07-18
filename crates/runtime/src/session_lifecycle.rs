use viden_permissions::PermissionEngine;
use viden_session::SessionStore;
use viden_types::{
    CostUsageRecord, Message, PermissionLevel, PermissionMode, Role, RuntimeEvent,
    RuntimeEventKind, SessionMetaEntry, SessionSummary, TranscriptEntry, WorkMode, fresh_id,
    now_timestamp,
};
use viden_workflows::stores::WorkflowAgentEvent;

use crate::SessionEngine;

impl SessionEngine {
    pub(super) fn handle_sessions(&self) -> Result<String, String> {
        let sessions = self.store.list_sessions_for_cwd()?;
        Ok(self.render_session_list(&sessions))
    }

    pub(super) fn handle_resume(&mut self, selector: Option<&str>) -> Result<String, String> {
        let Some(selector) = selector else {
            return self.handle_sessions();
        };
        if selector == "list" {
            return self.handle_sessions();
        }
        let loaded = match selector {
            "latest" => self.store.load_latest_for_cwd()?,
            other => self.resolve_resume_selector(other)?,
        };
        let Some((summary, entries)) = loaded else {
            return Ok("No resumable sessions found for the current project.".to_string());
        };
        let resumed_store = SessionStore::new_with_home(
            self.store.home_dir().to_path_buf(),
            self.cwd.clone(),
            Some(summary.session_id.clone()),
        )?;
        self.store = resumed_store;
        self.messages.clear();
        self.last_diff = None;
        self.last_test = None;
        self.provider_cost_usage.clear();
        self.permissions = PermissionEngine::new(&self.cwd);
        self.hydrate(entries)?;
        self.hydrate_workflow_agent_projection()?;
        Ok(format!(
            "Resumed session {} ({})",
            summary.session_id,
            summary.title.unwrap_or_else(|| "untitled".to_string())
        ))
    }

    fn resolve_resume_selector(
        &self,
        selector: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        let sessions = self.store.list_sessions_for_cwd()?;
        if sessions.is_empty() {
            return Ok(None);
        }

        if let Some(loaded) = self.store.load_by_id_for_cwd(selector)? {
            return Ok(Some(loaded));
        }

        let matches: Vec<_> = sessions
            .iter()
            .filter(|summary| {
                summary.session_id != self.session_id()
                    && (summary.session_id.starts_with(selector)
                        || summary
                            .session_id
                            .trim_start_matches("session_")
                            .starts_with(selector))
            })
            .cloned()
            .collect();
        match matches.as_slice() {
            [] => self.resolve_resume_index(&sessions, selector),
            [summary] => {
                let entries = SessionStore::load_entries_from_path(std::path::Path::new(
                    &summary.transcript_path,
                ))?;
                Ok(Some((summary.clone(), entries)))
            }
            _ => Err(format!(
                "Session selector `{selector}` is ambiguous.\n\n{}",
                self.render_session_list(matches.as_slice())
            )),
        }
    }

    fn resolve_resume_index(
        &self,
        sessions: &[SessionSummary],
        selector: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        let index_selector = selector.strip_prefix('#').unwrap_or(selector);
        let Ok(index) = index_selector.parse::<usize>() else {
            return Ok(None);
        };
        if index == 0 {
            return Err("Session indexes start at 1.".to_string());
        }
        if let Some(summary) = sessions.get(index - 1) {
            let entries = SessionStore::load_entries_from_path(std::path::Path::new(
                &summary.transcript_path,
            ))?;
            return Ok(Some((summary.clone(), entries)));
        }
        Err(format!("No session found at index {index}."))
    }

    fn hydrate(&mut self, entries: Vec<TranscriptEntry>) -> Result<(), String> {
        let mut provider_meta = None;
        let mut model_meta = None;
        for entry in entries {
            match entry {
                TranscriptEntry::Message { message } => self.messages.push(message),
                TranscriptEntry::ToolResult { result } => {
                    if let Some(diff) = result.diff.clone() {
                        self.last_diff = Some(diff);
                    }
                    self.messages.push(Message {
                        id: fresh_id("msg"),
                        role: Role::Tool,
                        content: result.output,
                        timestamp: now_timestamp(),
                        tool_name: Some(result.name),
                        tool_call_id: Some(result.tool_call_id),
                    });
                }
                TranscriptEntry::SessionMeta { entry } => match entry.key.as_str() {
                    "work_mode" => {
                        if let Some(mode) = WorkMode::parse_cli(&entry.value) {
                            self.runtime_snapshot.work_mode = mode;
                        }
                    }
                    "permission_mode" => {
                        if let Some(mode) = PermissionMode::parse_cli(&entry.value) {
                            self.permissions.set_mode(mode);
                            self.runtime_snapshot.permission_mode = mode;
                            self.runtime_snapshot.permission_level =
                                PermissionLevel::from_legacy_mode(mode);
                            if mode == PermissionMode::Plan {
                                self.runtime_snapshot.work_mode = WorkMode::Plan;
                            }
                        }
                    }
                    "model" => {
                        model_meta = Some(entry.value.clone());
                    }
                    "provider" => {
                        provider_meta = Some(entry.value.clone());
                    }
                    _ => {}
                },
                TranscriptEntry::CostUsage { cost } => self.remember_cost_usage(*cost),
                TranscriptEntry::RuntimeEvent { event } => {
                    self.remember_runtime_domain_event(*event)
                }
                _ => {}
            }
        }
        self.restore_provider_meta(provider_meta.as_deref(), model_meta.as_deref())?;
        Ok(())
    }

    fn restore_provider_meta(
        &mut self,
        provider_id: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), String> {
        if let Some(provider_id) = provider_id
            && self.provider_host.is_some()
        {
            let next_provider = self.create_provider_from_runtime(provider_id, model)?;
            self.provider = next_provider;
            self.runtime_snapshot.provider_family = provider_id.to_string();
            self.runtime_snapshot.model_label = self.provider.model().to_string();
            return Ok(());
        }
        if let Some(model) = model {
            self.provider.set_model(model.to_string());
            self.runtime_snapshot.model_label = self.provider.model().to_string();
        }
        self.runtime_snapshot.provider_family = self.provider.provider_name().to_string();
        Ok(())
    }

    pub(super) fn persist_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.store_entry(TranscriptEntry::SessionMeta {
            entry: SessionMetaEntry {
                timestamp: now_timestamp(),
                key: key.to_string(),
                value: value.to_string(),
            },
        })
    }

    pub(super) fn store_entry(&self, entry: TranscriptEntry) -> Result<(), String> {
        #[cfg(test)]
        if let Some(remaining) = self.fail_transcript_append_after.get() {
            if remaining == 0 {
                self.fail_transcript_append_after.set(None);
                return Err("injected transcript append failure".to_string());
            }
            self.fail_transcript_append_after.set(Some(remaining - 1));
        }

        self.store.append_entry(&entry)
    }

    pub(super) fn record_cost_usage(&mut self, cost: CostUsageRecord) -> Result<(), String> {
        self.store_entry(TranscriptEntry::CostUsage {
            cost: Box::new(cost.clone()),
        })?;
        self.remember_cost_usage(cost);
        Ok(())
    }

    pub(super) fn remember_cost_usage(&mut self, cost: CostUsageRecord) {
        if self
            .provider_cost_usage
            .iter()
            .any(|existing| existing.usage_id == cost.usage_id)
        {
            return;
        }
        self.provider_cost_usage.push(cost);
    }

    pub(crate) fn persist_runtime_domain_events(
        &mut self,
        events: &[RuntimeEvent],
    ) -> Result<(), String> {
        let domain_events = events
            .iter()
            .filter(|event| is_durable_runtime_domain_event(&event.kind))
            .cloned()
            .collect::<Vec<_>>();
        for event in &domain_events {
            self.persist_workflow_runtime_projection(event)?;
        }
        for event in domain_events {
            self.remember_runtime_domain_event(event);
        }
        Ok(())
    }

    fn persist_workflow_runtime_projection(&self, event: &RuntimeEvent) -> Result<(), String> {
        let Some((dag_id, task_id)) = self.workflow_projection_identity(event) else {
            return Ok(());
        };
        let mut payload = std::collections::BTreeMap::new();
        payload.insert(
            "runtime_event_kind".to_string(),
            runtime_projection_kind_name(&event.kind).to_string(),
        );
        payload.insert(
            "runtime_event_json".to_string(),
            serde_json::to_string(event).map_err(|err| err.to_string())?,
        );
        let projection = WorkflowAgentEvent {
            event_id: fresh_id("agent_projection"),
            dag_id,
            task_id,
            event_type: "runtime_projection".to_string(),
            timestamp: now_timestamp(),
            origin_session_id: Some(self.session_id().to_string()),
            payload,
        };
        #[cfg(test)]
        if self.fail_next_workflow_append.replace(false) {
            return Err("injected workflow append failure".to_string());
        }
        self.workflows.append_agent_event(&projection)
    }

    fn workflow_projection_identity(
        &self,
        event: &RuntimeEvent,
    ) -> Option<(String, Option<String>)> {
        match &event.kind {
            RuntimeEventKind::AgentDagUpdated { dag } => Some((dag.dag_id.clone(), None)),
            RuntimeEventKind::TaskUpdated { task } => Some((
                self.dag_id_for_task_projection(&task.id)
                    .unwrap_or_else(|| "project-agent-runtime".to_string()),
                Some(task.id.clone()),
            )),
            RuntimeEventKind::EvidenceRecorded { evidence } => {
                let task_id = evidence
                    .canonical
                    .as_ref()
                    .map(|canonical| canonical.producer.task_id.clone())
                    .or_else(|| self.task_id_for_evidence(&evidence.id));
                Some((
                    task_id
                        .as_deref()
                        .and_then(|id| self.dag_id_for_task_projection(id))
                        .unwrap_or_else(|| "project-agent-runtime".to_string()),
                    task_id,
                ))
            }
            RuntimeEventKind::EvidenceCanonicalized { evidence_id, .. } => {
                let task_id = self
                    .runtime_evidence
                    .iter()
                    .find(|evidence| evidence.id == *evidence_id)
                    .and_then(|evidence| {
                        evidence
                            .canonical
                            .as_ref()
                            .map(|canonical| canonical.producer.task_id.clone())
                    })
                    .or_else(|| self.task_id_for_evidence(evidence_id));
                Some((
                    task_id
                        .as_deref()
                        .and_then(|id| self.dag_id_for_task_projection(id))
                        .unwrap_or_else(|| "project-agent-runtime".to_string()),
                    task_id,
                ))
            }
            RuntimeEventKind::MergeGateUpdated { gate } => Some((
                self.dag_id_for_task_projection(&gate.task_id)
                    .unwrap_or_else(|| "project-agent-runtime".to_string()),
                Some(gate.task_id.clone()),
            )),
            RuntimeEventKind::ContextBundleBuilt { scope, .. } => {
                let task_id = task_id_for_context_scope(scope);
                Some((
                    task_id
                        .as_deref()
                        .and_then(|id| self.dag_id_for_task_projection(id))
                        .unwrap_or_else(|| "project-agent-runtime".to_string()),
                    task_id,
                ))
            }
            RuntimeEventKind::ContextItemStored { item } => {
                let task_id = task_id_for_context_scope(&item.scope);
                Some((
                    task_id
                        .as_deref()
                        .and_then(|id| self.dag_id_for_task_projection(id))
                        .unwrap_or_else(|| "project-agent-runtime".to_string()),
                    task_id,
                ))
            }
            RuntimeEventKind::ContextViewDerived { handle, .. } => {
                let task_id = task_id_for_context_scope(&handle.scope);
                Some((
                    task_id
                        .as_deref()
                        .and_then(|id| self.dag_id_for_task_projection(id))
                        .unwrap_or_else(|| "project-agent-runtime".to_string()),
                    task_id,
                ))
            }
            RuntimeEventKind::ContextBudgetExceeded { budget } => {
                let task_id = task_id_for_context_scope(&budget.scope);
                Some((
                    task_id
                        .as_deref()
                        .and_then(|id| self.dag_id_for_task_projection(id))
                        .unwrap_or_else(|| "project-agent-runtime".to_string()),
                    task_id,
                ))
            }
            RuntimeEventKind::ContextQualityFailed { .. }
            | RuntimeEventKind::ContextUpdated { .. } => {
                Some(("project-agent-runtime".to_string(), None))
            }
            _ => None,
        }
    }

    fn dag_id_for_task_projection(&self, task_id: &str) -> Option<String> {
        self.runtime_agent_dags
            .iter()
            .find(|dag| dag.tasks.iter().any(|task| task.task_id == task_id))
            .map(|dag| dag.dag_id.clone())
    }

    fn task_id_for_evidence(&self, evidence_id: &str) -> Option<String> {
        self.runtime_merge_gates
            .iter()
            .find(|gate| gate.evidence_ids.iter().any(|id| id == evidence_id))
            .map(|gate| gate.task_id.clone())
    }

    fn hydrate_workflow_agent_projection(&mut self) -> Result<(), String> {
        let events = self.workflows.load_agent_events()?;
        for event in events {
            if event.event_type != "runtime_projection" {
                continue;
            }
            let Some(json) = event.payload.get("runtime_event_json") else {
                continue;
            };
            let runtime_event =
                serde_json::from_str::<RuntimeEvent>(json).map_err(|err| err.to_string())?;
            self.remember_runtime_domain_event(runtime_event);
        }
        Ok(())
    }

    fn remember_runtime_domain_event(&mut self, event: RuntimeEvent) {
        match event.kind {
            RuntimeEventKind::AgentDagUpdated { dag } => {
                if let Some(existing) = self
                    .runtime_agent_dags
                    .iter_mut()
                    .find(|existing| existing.dag_id == dag.dag_id)
                {
                    *existing = dag;
                } else {
                    self.runtime_agent_dags.push(dag);
                }
            }
            RuntimeEventKind::TaskUpdated { task } => self.upsert_agent_task(task),
            RuntimeEventKind::ContextBundleBuilt { .. }
            | RuntimeEventKind::ContextItemStored { .. }
            | RuntimeEventKind::ContextViewDerived { .. }
            | RuntimeEventKind::ContextBudgetExceeded { .. }
            | RuntimeEventKind::ContextQualityFailed { .. } => {
                self.remember_context_runtime_event(event);
            }
            RuntimeEventKind::ContextUpdated { context } => {
                self.last_context_bundle = Some(context);
            }
            RuntimeEventKind::EvidenceRecorded { evidence } => {
                self.upsert_runtime_evidence(evidence)
            }
            RuntimeEventKind::MergeGateUpdated { gate } => {
                if let Some(existing) = self
                    .runtime_merge_gates
                    .iter_mut()
                    .find(|existing| existing.gate_id == gate.gate_id)
                {
                    *existing = gate;
                } else {
                    self.runtime_merge_gates.push(gate);
                }
            }
            RuntimeEventKind::EvidenceCanonicalized { .. } => {}
            _ => {}
        }
    }

    fn remember_context_runtime_event(&mut self, event: RuntimeEvent) {
        if !self
            .last_context_runtime_events
            .iter()
            .any(|existing| existing.kind == event.kind)
        {
            self.last_context_runtime_events.push(event);
        }
    }
}

fn task_id_for_context_scope(scope: &viden_types::ContextScope) -> Option<String> {
    match scope {
        viden_types::ContextScope::Task(task_id) => Some(task_id.clone()),
        _ => None,
    }
}

fn runtime_projection_kind_name(kind: &RuntimeEventKind) -> &'static str {
    match kind {
        RuntimeEventKind::AgentDagUpdated { .. } => "agent_dag_updated",
        RuntimeEventKind::TaskUpdated { .. } => "task_updated",
        RuntimeEventKind::ContextBundleBuilt { .. } => "context_bundle_built",
        RuntimeEventKind::ContextItemStored { .. } => "context_item_stored",
        RuntimeEventKind::ContextViewDerived { .. } => "context_view_derived",
        RuntimeEventKind::ContextBudgetExceeded { .. } => "context_budget_exceeded",
        RuntimeEventKind::ContextQualityFailed { .. } => "context_quality_failed",
        RuntimeEventKind::ContextUpdated { .. } => "context_updated",
        RuntimeEventKind::EvidenceRecorded { .. } => "evidence_recorded",
        RuntimeEventKind::EvidenceCanonicalized { .. } => "evidence_canonicalized",
        RuntimeEventKind::MergeGateUpdated { .. } => "merge_gate_updated",
        _ => "runtime_event",
    }
}

fn is_durable_runtime_domain_event(kind: &RuntimeEventKind) -> bool {
    // These facts rebuild runtime DAG/evidence/gate state. Transient command
    // acknowledgements stay out of JSONL so replay remains append-only domain state.
    matches!(
        kind,
        RuntimeEventKind::AgentDagUpdated { .. }
            | RuntimeEventKind::TaskUpdated { .. }
            | RuntimeEventKind::ContextBundleBuilt { .. }
            | RuntimeEventKind::ContextItemStored { .. }
            | RuntimeEventKind::ContextViewDerived { .. }
            | RuntimeEventKind::ContextBudgetExceeded { .. }
            | RuntimeEventKind::ContextQualityFailed { .. }
            | RuntimeEventKind::ContextUpdated { .. }
            | RuntimeEventKind::EvidenceRecorded { .. }
            | RuntimeEventKind::EvidenceCanonicalized { .. }
            | RuntimeEventKind::MergeGateUpdated { .. }
    )
}
