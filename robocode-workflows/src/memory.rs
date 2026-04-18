//! Project and session memory domain.

use std::collections::BTreeMap;

use robocode_types::{
    MemoryEntry, MemoryId, MemoryKind, MemoryScope, MemorySource, MemoryStatus, TaskId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryEvent {
    Added {
        memory_id: MemoryId,
        scope: MemoryScope,
        session_id: Option<String>,
        kind: MemoryKind,
        content: String,
        source: MemorySource,
        related_task_ids: Vec<TaskId>,
        confidence_hint: Option<String>,
        timestamp: u64,
    },
    Suggested {
        memory_id: MemoryId,
        kind: MemoryKind,
        content: String,
        source: MemorySource,
        related_task_ids: Vec<TaskId>,
        confidence_hint: Option<String>,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Confirmed {
        memory_id: MemoryId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Rejected {
        memory_id: MemoryId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Pruned {
        memory_id: MemoryId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
    Superseded {
        memory_id: MemoryId,
        timestamp: u64,
        origin_session_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryState {
    entries: BTreeMap<MemoryId, MemoryEntry>,
}

impl MemoryState {
    pub fn memory(&self, memory_id: &str) -> Option<&MemoryEntry> {
        self.entries.get(memory_id)
    }

    pub fn active_project_memory(&self) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|entry| {
                entry.scope == MemoryScope::Project && entry.status == MemoryStatus::Active
            })
            .collect()
    }

    pub fn active_session_memory(&self, session_id: &str) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|entry| {
                entry.scope == MemoryScope::Session
                    && entry.session_id.as_deref() == Some(session_id)
                    && entry.status == MemoryStatus::Active
            })
            .collect()
    }

    pub fn pending_suggestions(&self) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|entry| entry.status == MemoryStatus::Suggested)
            .collect()
    }
}

pub fn reduce_memory_events(events: &[MemoryEvent]) -> Result<MemoryState, String> {
    let mut state = MemoryState::default();
    for event in events {
        apply_event(&mut state, event)?;
    }
    Ok(state)
}

fn apply_event(state: &mut MemoryState, event: &MemoryEvent) -> Result<(), String> {
    match event {
        MemoryEvent::Added {
            memory_id,
            scope,
            session_id,
            kind,
            content,
            source,
            related_task_ids,
            confidence_hint,
            timestamp,
        } => {
            ensure_new_memory(state, memory_id)?;
            if *scope == MemoryScope::Session && session_id.is_none() {
                return Err("session memory requires a session id".to_string());
            }
            state.entries.insert(
                memory_id.clone(),
                MemoryEntry {
                    memory_id: memory_id.clone(),
                    scope: *scope,
                    session_id: session_id.clone(),
                    kind: *kind,
                    content: content.clone(),
                    source: *source,
                    status: MemoryStatus::Active,
                    created_at: *timestamp,
                    updated_at: *timestamp,
                    related_task_ids: related_task_ids.clone(),
                    confidence_hint: confidence_hint.clone(),
                },
            );
        }
        MemoryEvent::Suggested {
            memory_id,
            kind,
            content,
            source,
            related_task_ids,
            confidence_hint,
            timestamp,
            origin_session_id,
        } => {
            ensure_new_memory(state, memory_id)?;
            state.entries.insert(
                memory_id.clone(),
                MemoryEntry {
                    memory_id: memory_id.clone(),
                    scope: MemoryScope::Project,
                    session_id: origin_session_id.clone(),
                    kind: *kind,
                    content: content.clone(),
                    source: *source,
                    status: MemoryStatus::Suggested,
                    created_at: *timestamp,
                    updated_at: *timestamp,
                    related_task_ids: related_task_ids.clone(),
                    confidence_hint: confidence_hint.clone(),
                },
            );
        }
        MemoryEvent::Confirmed {
            memory_id,
            timestamp,
            origin_session_id: _,
        } => {
            let entry = require_memory_mut(state, memory_id)?;
            if entry.status != MemoryStatus::Suggested {
                return Err(format!("memory `{memory_id}` is not pending confirmation"));
            }
            entry.status = MemoryStatus::Active;
            entry.updated_at = *timestamp;
        }
        MemoryEvent::Rejected {
            memory_id,
            timestamp,
            origin_session_id: _,
        } => {
            let entry = require_memory_mut(state, memory_id)?;
            if entry.status != MemoryStatus::Suggested {
                return Err(format!("memory `{memory_id}` is not pending rejection"));
            }
            entry.status = MemoryStatus::Rejected;
            entry.updated_at = *timestamp;
        }
        MemoryEvent::Pruned {
            memory_id,
            timestamp,
            origin_session_id: _,
        } => {
            let entry = require_memory_mut(state, memory_id)?;
            entry.status = MemoryStatus::Pruned;
            entry.updated_at = *timestamp;
        }
        MemoryEvent::Superseded {
            memory_id,
            timestamp,
            origin_session_id: _,
        } => {
            let entry = require_memory_mut(state, memory_id)?;
            entry.status = MemoryStatus::Superseded;
            entry.updated_at = *timestamp;
        }
    }
    Ok(())
}

fn ensure_new_memory(state: &MemoryState, memory_id: &str) -> Result<(), String> {
    if state.entries.contains_key(memory_id) {
        Err(format!("memory `{memory_id}` already exists"))
    } else {
        Ok(())
    }
}

fn require_memory_mut<'a>(
    state: &'a mut MemoryState,
    memory_id: &str,
) -> Result<&'a mut MemoryEntry, String> {
    state
        .entries
        .get_mut(memory_id)
        .ok_or_else(|| format!("missing memory: `{memory_id}`"))
}

#[cfg(test)]
mod tests {
    use robocode_types::{MemoryKind, MemoryScope, MemorySource, MemoryStatus};

    use super::*;

    #[test]
    fn memory_reducer_adds_session_memory_directly() {
        let state = reduce_memory_events(&[MemoryEvent::Added {
            memory_id: "mem_session".to_string(),
            scope: MemoryScope::Session,
            session_id: Some("session_1".to_string()),
            kind: MemoryKind::Decision,
            content: "Use workflow event logs".to_string(),
            source: MemorySource::User,
            related_task_ids: vec!["task_1".to_string()],
            confidence_hint: Some("high".to_string()),
            timestamp: 10,
        }])
        .unwrap();

        let session_memory = state.active_session_memory("session_1");
        assert_eq!(session_memory.len(), 1);
        assert_eq!(session_memory[0].status, MemoryStatus::Active);
        assert_eq!(session_memory[0].content, "Use workflow event logs");
    }

    #[test]
    fn memory_reducer_confirms_project_suggestions() {
        let state = reduce_memory_events(&[
            MemoryEvent::Suggested {
                memory_id: "mem_project".to_string(),
                kind: MemoryKind::Convention,
                content: "Project memory requires confirmation".to_string(),
                source: MemorySource::AssistantSuggestion,
                related_task_ids: vec!["task_1".to_string()],
                confidence_hint: Some("medium".to_string()),
                timestamp: 10,
                origin_session_id: Some("session_1".to_string()),
            },
            MemoryEvent::Confirmed {
                memory_id: "mem_project".to_string(),
                timestamp: 11,
                origin_session_id: Some("session_1".to_string()),
            },
        ])
        .unwrap();

        assert!(state.pending_suggestions().is_empty());
        let project_memory = state.active_project_memory();
        assert_eq!(project_memory.len(), 1);
        assert_eq!(project_memory[0].status, MemoryStatus::Active);
    }

    #[test]
    fn memory_reducer_rejects_and_prunes_entries() {
        let state = reduce_memory_events(&[
            suggested_event("mem_reject", "Reject this", 10),
            MemoryEvent::Rejected {
                memory_id: "mem_reject".to_string(),
                timestamp: 11,
                origin_session_id: None,
            },
            suggested_event("mem_prune", "Prune this", 12),
            MemoryEvent::Confirmed {
                memory_id: "mem_prune".to_string(),
                timestamp: 13,
                origin_session_id: None,
            },
            MemoryEvent::Pruned {
                memory_id: "mem_prune".to_string(),
                timestamp: 14,
                origin_session_id: None,
            },
        ])
        .unwrap();

        assert_eq!(state.memory("mem_reject").unwrap().status, MemoryStatus::Rejected);
        assert_eq!(state.memory("mem_prune").unwrap().status, MemoryStatus::Pruned);
        assert!(state.active_project_memory().is_empty());
    }

    #[test]
    fn memory_reducer_supersedes_entries_and_isolates_session_scope() {
        let state = reduce_memory_events(&[
            MemoryEvent::Added {
                memory_id: "mem_s1".to_string(),
                scope: MemoryScope::Session,
                session_id: Some("session_1".to_string()),
                kind: MemoryKind::Fact,
                content: "Session one fact".to_string(),
                source: MemorySource::Command,
                related_task_ids: Vec::new(),
                confidence_hint: None,
                timestamp: 10,
            },
            MemoryEvent::Added {
                memory_id: "mem_s2".to_string(),
                scope: MemoryScope::Session,
                session_id: Some("session_2".to_string()),
                kind: MemoryKind::Fact,
                content: "Session two fact".to_string(),
                source: MemorySource::Command,
                related_task_ids: Vec::new(),
                confidence_hint: None,
                timestamp: 11,
            },
            MemoryEvent::Superseded {
                memory_id: "mem_s1".to_string(),
                timestamp: 12,
                origin_session_id: Some("session_1".to_string()),
            },
        ])
        .unwrap();

        assert!(state.active_session_memory("session_1").is_empty());
        assert_eq!(state.active_session_memory("session_2").len(), 1);
        assert_eq!(state.memory("mem_s1").unwrap().status, MemoryStatus::Superseded);
    }

    fn suggested_event(memory_id: &str, content: &str, timestamp: u64) -> MemoryEvent {
        MemoryEvent::Suggested {
            memory_id: memory_id.to_string(),
            kind: MemoryKind::Fact,
            content: content.to_string(),
            source: MemorySource::AssistantSuggestion,
            related_task_ids: Vec::new(),
            confidence_hint: None,
            timestamp,
            origin_session_id: None,
        }
    }
}
