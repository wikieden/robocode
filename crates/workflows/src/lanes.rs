//! Append-only typed lane lifecycle state.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use viden_types::{AgentLaneRecord, LaneStatus};

pub const LEGACY_LANES_MIGRATION_ID: &str = "legacy_lanes_tsv_v0";
pub const LEGACY_LANES_SCHEMA: &str = "viden.lanes.tsv.v0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneEvent {
    pub event_id: String,
    pub lane_id: String,
    pub timestamp: u64,
    pub origin_session_id: Option<String>,
    pub kind: LaneEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LaneEventKind {
    Created {
        lane: AgentLaneRecord,
    },
    Replaced {
        lane: AgentLaneRecord,
    },
    StatusChanged {
        status: LaneStatus,
        summary: String,
    },
    Archived {
        summary: String,
    },
    LegacyImported {
        source: String,
        schema: String,
        lanes: Vec<AgentLaneRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneMigrationAudit {
    pub migration_id: String,
    pub source: String,
    pub schema: String,
    pub imported_lane_ids: Vec<String>,
    pub event_id: String,
    pub timestamp: u64,
    pub origin_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaneState {
    lanes: BTreeMap<String, AgentLaneRecord>,
    migrations: BTreeMap<String, LaneMigrationAudit>,
    seen_event_ids: BTreeSet<String>,
}

impl LaneState {
    pub fn lane(&self, lane_id: &str) -> Option<&AgentLaneRecord> {
        self.lanes.get(lane_id)
    }

    pub fn lanes(&self) -> &BTreeMap<String, AgentLaneRecord> {
        &self.lanes
    }

    pub fn migrations(&self) -> &BTreeMap<String, LaneMigrationAudit> {
        &self.migrations
    }

    pub fn migration(&self, migration_id: &str) -> Option<&LaneMigrationAudit> {
        self.migrations.get(migration_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyLaneImportOutcome {
    pub imported: bool,
    pub lane_count: usize,
}

impl LaneEvent {
    pub fn created(
        event_id: impl Into<String>,
        lane: AgentLaneRecord,
        timestamp: u64,
        origin_session_id: Option<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            lane_id: lane.id.clone(),
            timestamp,
            origin_session_id,
            kind: LaneEventKind::Created { lane },
        }
    }

    pub fn status_changed(
        event_id: impl Into<String>,
        lane_id: impl Into<String>,
        status: LaneStatus,
        summary: impl Into<String>,
        timestamp: u64,
        origin_session_id: Option<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            lane_id: lane_id.into(),
            timestamp,
            origin_session_id,
            kind: LaneEventKind::StatusChanged {
                status,
                summary: summary.into(),
            },
        }
    }

    pub fn legacy_imported(
        event_id: impl Into<String>,
        source: impl Into<String>,
        lanes: Vec<AgentLaneRecord>,
        timestamp: u64,
        origin_session_id: Option<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            lane_id: LEGACY_LANES_MIGRATION_ID.to_string(),
            timestamp,
            origin_session_id,
            kind: LaneEventKind::LegacyImported {
                source: source.into(),
                schema: LEGACY_LANES_SCHEMA.to_string(),
                lanes,
            },
        }
    }
}

pub fn reduce_lane_events(events: &[LaneEvent]) -> Result<LaneState, String> {
    let mut state = LaneState::default();
    for event in events {
        if !state.seen_event_ids.insert(event.event_id.clone()) {
            return Err(format!("duplicate lane event id `{}`", event.event_id));
        }
        apply_event(&mut state, event)?;
    }
    Ok(state)
}

fn apply_event(state: &mut LaneState, event: &LaneEvent) -> Result<(), String> {
    match &event.kind {
        LaneEventKind::Created { lane } => {
            require_matching_lane_id(event, lane)?;
            if state.lanes.insert(lane.id.clone(), lane.clone()).is_some() {
                return Err(format!("lane `{}` already exists", lane.id));
            }
        }
        LaneEventKind::Replaced { lane } => {
            require_matching_lane_id(event, lane)?;
            if !state.lanes.contains_key(&lane.id) {
                return Err(format!("lane `{}` does not exist", lane.id));
            }
            state.lanes.insert(lane.id.clone(), lane.clone());
        }
        LaneEventKind::StatusChanged { status, summary } => {
            let lane = state
                .lanes
                .get_mut(&event.lane_id)
                .ok_or_else(|| format!("lane `{}` does not exist", event.lane_id))?;
            lane.status = *status;
            lane.summary = summary.clone();
        }
        LaneEventKind::Archived { summary } => {
            let lane = state
                .lanes
                .get_mut(&event.lane_id)
                .ok_or_else(|| format!("lane `{}` does not exist", event.lane_id))?;
            lane.status = LaneStatus::Archived;
            lane.summary = summary.clone();
        }
        LaneEventKind::LegacyImported {
            source,
            schema,
            lanes,
        } => {
            if event.lane_id != LEGACY_LANES_MIGRATION_ID {
                return Err("legacy lane import uses the wrong migration identity".to_string());
            }
            if schema != LEGACY_LANES_SCHEMA {
                return Err(format!("unsupported legacy lane schema `{schema}`"));
            }
            if state.migrations.contains_key(LEGACY_LANES_MIGRATION_ID) {
                return Err("legacy lane migration already exists".to_string());
            }
            let mut imported_lane_ids = Vec::with_capacity(lanes.len());
            for lane in lanes {
                if state.lanes.contains_key(&lane.id) {
                    return Err(format!("legacy lane `{}` already exists", lane.id));
                }
                if imported_lane_ids.contains(&lane.id) {
                    return Err(format!("legacy lane `{}` is duplicated", lane.id));
                }
                imported_lane_ids.push(lane.id.clone());
            }
            // Validate the complete batch before publishing any lane so a bad
            // migration event cannot leave a partially reduced state.
            for lane in lanes {
                state.lanes.insert(lane.id.clone(), lane.clone());
            }
            state.migrations.insert(
                LEGACY_LANES_MIGRATION_ID.to_string(),
                LaneMigrationAudit {
                    migration_id: LEGACY_LANES_MIGRATION_ID.to_string(),
                    source: source.clone(),
                    schema: schema.clone(),
                    imported_lane_ids,
                    event_id: event.event_id.clone(),
                    timestamp: event.timestamp,
                    origin_session_id: event.origin_session_id.clone(),
                },
            );
        }
    }
    Ok(())
}

fn require_matching_lane_id(event: &LaneEvent, lane: &AgentLaneRecord) -> Result<(), String> {
    if event.lane_id == lane.id {
        Ok(())
    } else {
        Err(format!(
            "lane event id `{}` does not match payload `{}`",
            event.lane_id, lane.id
        ))
    }
}

pub fn parse_legacy_lanes_tsv(raw: &str) -> Result<Vec<AgentLaneRecord>, String> {
    raw.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let parts = line.split('\t').collect::<Vec<_>>();
            if !matches!(parts.len(), 5 | 7 | 8) {
                return Err(format!(
                    "legacy lane line {} must contain five, seven, or eight tab-separated columns",
                    index + 1
                ));
            }
            if parts[0].trim().is_empty() {
                return Err(format!("legacy lane line {} has an empty id", index + 1));
            }
            if let Some(raw_progress) = parts.get(5) {
                let progress = raw_progress
                    .parse::<u8>()
                    .map_err(|_| format!("legacy lane line {} has invalid progress", index + 1))?;
                if progress > 100 {
                    return Err(format!(
                        "legacy lane line {} has progress above 100",
                        index + 1
                    ));
                }
            }
            let stable_id = parts[0].trim_start_matches("L-").replace('-', "_");
            let summary = parts
                .get(6)
                .filter(|summary| !summary.trim().is_empty())
                .copied()
                .unwrap_or(parts[2]);
            serde_json::from_value(serde_json::json!({
                "id": parts[0],
                "task_id": format!("task_{stable_id}"),
                "agent": parts[1],
                "screen": parts[2],
                "transport": parts[4],
                "status": parts[3],
                "summary": summary,
                "evidence": [format!("evidence_{stable_id}")],
            }))
            .map_err(|error| format!("legacy lane line {}: {error}", index + 1))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use viden_types::{LaneStatus, fresh_id};

    use super::*;
    use crate::stores::WorkflowStore;

    const LEGACY_LANES: &str =
        include_str!("../../types/tests/fixtures/frontend-contract-v1/legacy-lanes.tsv");

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("viden_workflows_lane_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn legacy_lane_import_is_atomic_and_idempotent() {
        let home = temp_dir("migration_home");
        let cwd = temp_dir("migration_cwd");
        let legacy_path = cwd.join(".viden").join("lanes.tsv");
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, LEGACY_LANES).unwrap();
        let store = WorkflowStore::new(&home, &cwd).unwrap();

        let first = store
            .import_legacy_lanes_tsv_once(&legacy_path, 10, Some("session_import".into()))
            .unwrap();
        let second = store
            .import_legacy_lanes_tsv_once(&legacy_path, 20, Some("session_repeat".into()))
            .unwrap();

        assert!(first.imported);
        assert_eq!(first.lane_count, 4);
        assert!(!second.imported);
        let events = store.load_lane_events().unwrap();
        assert_eq!(events.len(), 1, "the import is one append-only transaction");
        let state = store.load_lane_state().unwrap();
        assert_eq!(state.lanes().len(), 4);
        assert_eq!(state.migrations().len(), 1);
        assert_eq!(
            state.lane("L-conflict").unwrap().status,
            LaneStatus::Blocked
        );
        assert_eq!(
            state.lane("L-detached").unwrap().status,
            LaneStatus::Detached
        );
    }

    #[test]
    fn concurrent_legacy_lane_import_publishes_one_valid_event() {
        let home = temp_dir("concurrent_migration_home");
        let cwd = temp_dir("concurrent_migration_cwd");
        let legacy_path = cwd.join(".viden").join("lanes.tsv");
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, LEGACY_LANES).unwrap();
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let barrier = Arc::new(Barrier::new(8));

        let handles = (0..8)
            .map(|index| {
                let store = store.clone();
                let barrier = Arc::clone(&barrier);
                let legacy_path = legacy_path.clone();
                thread::spawn(move || {
                    barrier.wait();
                    store.import_legacy_lanes_tsv_once(
                        &legacy_path,
                        10 + index,
                        Some(format!("session_{index}")),
                    )
                })
            })
            .collect::<Vec<_>>();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.imported).count(),
            1
        );
        assert_eq!(store.load_lane_events().unwrap().len(), 1);
        assert_eq!(store.load_lane_state().unwrap().lanes().len(), 4);
    }

    #[test]
    fn legacy_lane_parser_matches_the_frozen_typed_fixture() {
        let parsed = parse_legacy_lanes_tsv(LEGACY_LANES).unwrap();
        let frozen: Vec<AgentLaneRecord> = serde_json::from_str(include_str!(
            "../../types/tests/fixtures/frontend-contract-v1/typed-lanes.json"
        ))
        .unwrap();

        assert_eq!(parsed, frozen);
    }

    #[test]
    fn lane_lifecycle_replay_matches_live_reduction() {
        let imported = parse_legacy_lanes_tsv(LEGACY_LANES).unwrap();
        let mut lane = imported[0].clone();
        lane.status = LaneStatus::Running;
        lane.summary = "lane running".into();
        let events = vec![
            LaneEvent::created("evt_create", lane.clone(), 10, Some("session_1".into())),
            LaneEvent::status_changed(
                "evt_block",
                lane.id.clone(),
                LaneStatus::Blocked,
                "dependency failed",
                20,
                Some("session_1".into()),
            ),
        ];

        let live = reduce_lane_events(&events).unwrap();
        assert_eq!(live.lane(&lane.id).unwrap().status, LaneStatus::Blocked);
        assert_eq!(live.lane(&lane.id).unwrap().summary, "dependency failed");

        let home = temp_dir("replay_home");
        let cwd = temp_dir("replay_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        for event in &events {
            store.append_lane_event_checked(event).unwrap();
        }
        assert_eq!(store.load_lane_state().unwrap(), live);
    }

    #[test]
    fn malformed_legacy_import_writes_no_partial_event() {
        let home = temp_dir("invalid_home");
        let cwd = temp_dir("invalid_cwd");
        let legacy_path = cwd.join("lanes.tsv");
        fs::write(&legacy_path, "L-bad\tcodex\tmissing-columns\n").unwrap();
        let store = WorkflowStore::new(&home, &cwd).unwrap();

        let error = store
            .import_legacy_lanes_tsv_once(&legacy_path, 10, None)
            .unwrap_err();

        assert!(error.contains("five, seven, or eight tab-separated columns"));
        assert!(store.load_lane_events().unwrap().is_empty());
        assert!(!store.paths().lanes_log.exists());
    }

    #[test]
    fn lifecycle_rejects_unknown_lane_without_appending() {
        let home = temp_dir("unknown_home");
        let cwd = temp_dir("unknown_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let event = LaneEvent::status_changed(
            "evt_unknown",
            "lane_missing",
            LaneStatus::Running,
            "must not exist",
            10,
            None,
        );

        assert!(store.append_lane_event_checked(&event).is_err());
        assert!(store.load_lane_events().unwrap().is_empty());
    }
}
