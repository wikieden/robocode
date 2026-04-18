//! Workflow event-log persistence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use robocode_session::project_key_for_path;
use serde::{Deserialize, Serialize};

use crate::memory::{MemoryEvent, MemoryState, reduce_memory_events};
use crate::tasks::{TaskEvent, TaskState, reduce_task_events};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPaths {
    pub home_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub project_dir: PathBuf,
    pub tasks_log: PathBuf,
    pub memory_log: PathBuf,
    pub index_db_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorkflowStore {
    paths: WorkflowPaths,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTaskEvent {
    pub event_id: String,
    pub task_id: String,
    pub event_type: String,
    pub timestamp: u64,
    pub origin_session_id: Option<String>,
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowMemoryEvent {
    pub event_id: String,
    pub memory_id: String,
    pub event_type: String,
    pub timestamp: u64,
    pub origin_session_id: Option<String>,
    pub payload: BTreeMap<String, String>,
}

impl WorkflowStore {
    pub fn new(home_dir: impl Into<PathBuf>, cwd: impl AsRef<Path>) -> Result<Self, String> {
        let home_dir = home_dir.into();
        let projects_dir = home_dir.join("workflows").join("projects");
        let project_dir = projects_dir.join(project_key_for_path(cwd.as_ref()));
        let paths = WorkflowPaths {
            tasks_log: project_dir.join("tasks.jsonl"),
            memory_log: project_dir.join("memory.jsonl"),
            index_db_path: project_dir.join("workflow.sqlite3"),
            home_dir,
            projects_dir,
            project_dir,
        };
        fs::create_dir_all(&paths.project_dir).map_err(|err| err.to_string())?;
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &WorkflowPaths {
        &self.paths
    }

    pub fn append_task_event(&self, event: &WorkflowTaskEvent) -> Result<(), String> {
        append_json_line(&self.paths.tasks_log, event)
    }

    pub fn append_memory_event(&self, event: &WorkflowMemoryEvent) -> Result<(), String> {
        append_json_line(&self.paths.memory_log, event)
    }

    pub fn load_task_events(&self) -> Result<Vec<WorkflowTaskEvent>, String> {
        load_json_lines(&self.paths.tasks_log)
    }

    pub fn load_memory_events(&self) -> Result<Vec<WorkflowMemoryEvent>, String> {
        load_json_lines(&self.paths.memory_log)
    }

    pub fn append_task_domain_event(&self, event: &TaskEvent) -> Result<(), String> {
        append_json_line(&self.paths.tasks_log, event)
    }

    pub fn load_task_domain_events(&self) -> Result<Vec<TaskEvent>, String> {
        load_json_lines(&self.paths.tasks_log)
    }

    pub fn load_task_state(&self) -> Result<TaskState, String> {
        reduce_task_events(&self.load_task_domain_events()?)
    }

    pub fn append_memory_domain_event(&self, event: &MemoryEvent) -> Result<(), String> {
        append_json_line(&self.paths.memory_log, event)
    }

    pub fn load_memory_domain_events(&self) -> Result<Vec<MemoryEvent>, String> {
        load_json_lines(&self.paths.memory_log)
    }

    pub fn load_memory_state(&self) -> Result<MemoryState, String> {
        reduce_memory_events(&self.load_memory_domain_events()?)
    }

    pub fn rebuild_index(&self) -> Result<(), String> {
        if sqlite_available() {
            let sql = "CREATE TABLE IF NOT EXISTS workflow_events (
                event_id TEXT PRIMARY KEY,
                entity_kind TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );";
            run_sql(&self.paths.index_db_path, sql)?;
        } else if !self.paths.index_db_path.exists() {
            fs::write(&self.paths.index_db_path, []).map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

fn append_json_line<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let mut payload = serde_json::to_string(value).map_err(|err| err.to_string())?;
    payload.push('\n');
    if path.exists() {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|err| err.to_string())?;
        file.write_all(payload.as_bytes())
            .map_err(|err| err.to_string())
    } else {
        fs::write(path, payload).map_err(|err| err.to_string())
    }
}

fn load_json_lines<T>(path: &Path) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|err| err.to_string()))
        .collect()
}

fn sqlite_available() -> bool {
    Command::new("sqlite3")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_sql(db_path: &Path, sql: &str) -> Result<String, String> {
    let output = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use robocode_types::fresh_id;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("robocode_workflows_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn workflow_store_paths_are_project_scoped() {
        let home = temp_dir("paths_home");
        let cwd = temp_dir("paths_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();

        assert_eq!(store.paths().home_dir, home);
        assert!(
            store
                .paths()
                .project_dir
                .starts_with(store.paths().projects_dir.clone())
        );
        assert_eq!(store.paths().tasks_log.file_name().unwrap(), "tasks.jsonl");
        assert_eq!(
            store.paths().memory_log.file_name().unwrap(),
            "memory.jsonl"
        );
        assert_eq!(
            store.paths().index_db_path.file_name().unwrap(),
            "workflow.sqlite3"
        );
    }

    #[test]
    fn task_and_memory_events_roundtrip_through_jsonl() {
        let home = temp_dir("roundtrip_home");
        let cwd = temp_dir("roundtrip_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();

        let mut task_payload = BTreeMap::new();
        task_payload.insert("title".to_string(), "Plan workflow store".to_string());
        let task_event = WorkflowTaskEvent {
            event_id: "evt_task_1".to_string(),
            task_id: "task_1".to_string(),
            event_type: "task_created".to_string(),
            timestamp: 10,
            origin_session_id: Some("session_1".to_string()),
            payload: task_payload,
        };

        let mut memory_payload = BTreeMap::new();
        memory_payload.insert("content".to_string(), "Use append-only logs".to_string());
        let memory_event = WorkflowMemoryEvent {
            event_id: "evt_memory_1".to_string(),
            memory_id: "mem_1".to_string(),
            event_type: "memory_added".to_string(),
            timestamp: 20,
            origin_session_id: Some("session_1".to_string()),
            payload: memory_payload,
        };

        store.append_task_event(&task_event).unwrap();
        store.append_memory_event(&memory_event).unwrap();

        assert_eq!(store.load_task_events().unwrap(), vec![task_event]);
        assert_eq!(store.load_memory_events().unwrap(), vec![memory_event]);
    }

    #[test]
    fn workflow_index_rebuilds_from_event_logs() {
        let home = temp_dir("index_home");
        let cwd = temp_dir("index_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        store
            .append_task_event(&WorkflowTaskEvent {
                event_id: "evt_task_index".to_string(),
                task_id: "task_index".to_string(),
                event_type: "task_created".to_string(),
                timestamp: 30,
                origin_session_id: None,
                payload: BTreeMap::new(),
            })
            .unwrap();

        store.rebuild_index().unwrap();

        assert!(store.paths().index_db_path.exists());
    }
}
