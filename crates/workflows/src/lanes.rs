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
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};

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
    fn cross_process_legacy_import_publishes_one_valid_event() {
        let home = temp_dir("process_migration_home");
        let cwd = temp_dir("process_migration_cwd");
        let legacy_path = cwd.join(".viden").join("lanes.tsv");
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, LEGACY_LANES).unwrap();
        let start_gate = cwd.join("import.start");
        let results = (0..4)
            .map(|index| cwd.join(format!("import-{index}.result")))
            .collect::<Vec<_>>();
        let mut children = results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                spawn_lane_store_helper(
                    "import",
                    &home,
                    &cwd,
                    Some(&legacy_path),
                    None,
                    result,
                    &start_gate,
                    index,
                )
            })
            .collect::<Vec<_>>();

        release_lane_helpers_when_ready(&mut children, &results, &start_gate);
        wait_for_lane_helpers(children);

        let outcomes = results
            .iter()
            .map(|path| fs::read_to_string(path).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes.iter().filter(|value| *value == "imported").count(),
            1
        );
        assert_eq!(
            outcomes.iter().filter(|value| *value == "existing").count(),
            3
        );
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        assert_eq!(store.load_lane_events().unwrap().len(), 1);
        assert_eq!(store.load_lane_state().unwrap().lanes().len(), 4);
    }

    #[test]
    fn cross_process_checked_create_rejects_duplicate_lane_atomically() {
        let home = temp_dir("process_duplicate_home");
        let cwd = temp_dir("process_duplicate_cwd");
        let results = [
            cwd.join("duplicate-a.result"),
            cwd.join("duplicate-b.result"),
        ];
        let start_gate = cwd.join("duplicate.start");
        let mut children = results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                spawn_lane_store_helper(
                    "create",
                    &home,
                    &cwd,
                    None,
                    Some("lane_duplicate"),
                    result,
                    &start_gate,
                    index,
                )
            })
            .collect::<Vec<_>>();

        release_lane_helpers_when_ready(&mut children, &results, &start_gate);
        wait_for_lane_helpers(children);

        let outcomes = results
            .iter()
            .map(|path| fs::read_to_string(path).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            outcomes.iter().filter(|value| *value == "created").count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|value| value.starts_with("rejected:"))
                .count(),
            1
        );
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        assert_eq!(store.load_lane_events().unwrap().len(), 1);
        assert_eq!(store.load_lane_state().unwrap().lanes().len(), 1);
    }

    #[test]
    fn cross_process_first_checked_appends_preserve_both_events() {
        let home = temp_dir("process_first_append_home");
        let cwd = temp_dir("process_first_append_cwd");
        let results = [cwd.join("first-a.result"), cwd.join("first-b.result")];
        let lane_ids = ["lane_first_a", "lane_first_b"];
        let start_gate = cwd.join("first-append.start");
        let mut children = results
            .iter()
            .zip(lane_ids)
            .enumerate()
            .map(|(index, (result, lane_id))| {
                spawn_lane_store_helper(
                    "create",
                    &home,
                    &cwd,
                    None,
                    Some(lane_id),
                    result,
                    &start_gate,
                    index,
                )
            })
            .collect::<Vec<_>>();

        release_lane_helpers_when_ready(&mut children, &results, &start_gate);
        wait_for_lane_helpers(children);

        assert!(
            results
                .iter()
                .all(|path| fs::read_to_string(path).unwrap() == "created")
        );
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        assert_eq!(store.load_lane_events().unwrap().len(), 2);
        assert_eq!(store.load_lane_state().unwrap().lanes().len(), 2);
    }

    #[test]
    fn lane_store_process_helper() {
        let Ok(case) = env::var("VIDEN_LANE_STORE_HELPER") else {
            return;
        };
        let home = PathBuf::from(env::var_os("VIDEN_LANE_HOME").unwrap());
        let cwd = PathBuf::from(env::var_os("VIDEN_LANE_CWD").unwrap());
        let result_path = PathBuf::from(env::var_os("VIDEN_LANE_RESULT").unwrap());
        let ready_path = PathBuf::from(env::var_os("VIDEN_LANE_READY").unwrap());
        let start_gate = PathBuf::from(env::var_os("VIDEN_LANE_START_GATE").unwrap());
        let index = env::var("VIDEN_LANE_INDEX")
            .unwrap()
            .parse::<u64>()
            .unwrap();

        // Publish readiness before opening the store so every contender reaches the
        // same pre-lock boundary and the parent can release them together.
        fs::write(&ready_path, b"ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !start_gate.exists() {
            assert!(
                Instant::now() < deadline,
                "lane store helper timed out waiting for start gate"
            );
            thread::sleep(Duration::from_millis(5));
        }

        let store = WorkflowStore::new(home, &cwd).unwrap();
        let result = match case.as_str() {
            "import" => {
                let legacy_path = PathBuf::from(env::var_os("VIDEN_LANE_LEGACY").unwrap());
                match store.import_legacy_lanes_tsv_once(
                    legacy_path,
                    100 + index,
                    Some(format!("session_process_{index}")),
                ) {
                    Ok(outcome) if outcome.imported => "imported".to_string(),
                    Ok(_) => "existing".to_string(),
                    Err(error) => format!("error:{error}"),
                }
            }
            "create" => {
                let lane_id = env::var("VIDEN_LANE_ID").unwrap();
                let mut lane = parse_legacy_lanes_tsv(LEGACY_LANES).unwrap()[0].clone();
                lane.id = lane_id.clone();
                lane.task_id = Some(format!("task_{lane_id}"));
                let event = LaneEvent::created(
                    format!("event_{lane_id}_{index}"),
                    lane,
                    100 + index,
                    Some(format!("session_process_{index}")),
                );
                match store.append_lane_event_checked(&event) {
                    Ok(()) => "created".to_string(),
                    Err(error) => format!("rejected:{error}"),
                }
            }
            other => panic!("unknown lane store helper case {other}"),
        };
        fs::write(result_path, result).unwrap();
    }

    fn spawn_lane_store_helper(
        case: &str,
        home: &PathBuf,
        cwd: &PathBuf,
        legacy_path: Option<&PathBuf>,
        lane_id: Option<&str>,
        result_path: &PathBuf,
        start_gate: &PathBuf,
        index: usize,
    ) -> Child {
        let ready_path = result_path.with_extension("ready");
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("lanes::tests::lane_store_process_helper")
            .arg("--nocapture")
            .env("VIDEN_LANE_STORE_HELPER", case)
            .env("VIDEN_LANE_HOME", home)
            .env("VIDEN_LANE_CWD", cwd)
            .env("VIDEN_LANE_RESULT", result_path)
            .env("VIDEN_LANE_READY", ready_path)
            .env("VIDEN_LANE_START_GATE", start_gate)
            .env("VIDEN_LANE_INDEX", index.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(path) = legacy_path {
            command.env("VIDEN_LANE_LEGACY", path);
        }
        if let Some(lane_id) = lane_id {
            command.env("VIDEN_LANE_ID", lane_id);
        }
        command.spawn().unwrap()
    }

    fn release_lane_helpers_when_ready(
        children: &mut [Child],
        result_paths: &[PathBuf],
        start_gate: &PathBuf,
    ) {
        let ready_paths = result_paths
            .iter()
            .map(|path| path.with_extension("ready"))
            .collect::<Vec<_>>();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready_paths.iter().all(|path| path.exists()) {
            for child in children.iter_mut() {
                if let Some(status) = child.try_wait().unwrap() {
                    panic!("lane store helper exited before ready: {status}");
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for lane store helpers to become ready"
            );
            thread::sleep(Duration::from_millis(5));
        }

        assert!(
            result_paths.iter().all(|path| !path.exists()),
            "lane store helper ran before the shared start gate opened"
        );
        fs::write(start_gate, b"start").unwrap();
    }

    fn wait_for_lane_helpers(children: Vec<Child>) {
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
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
