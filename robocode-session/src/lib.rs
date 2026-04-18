use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use robocode_types::{
    Role, SessionSummary, TranscriptEntry, fresh_id, now_timestamp, truncate_for_preview,
};

#[derive(Debug, Clone)]
pub struct SessionPaths {
    pub home_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub project_dir: PathBuf,
    pub transcript_path: PathBuf,
    pub index_db_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    cwd: PathBuf,
    session_id: String,
    paths: SessionPaths,
}

impl SessionStore {
    pub fn new(cwd: impl Into<PathBuf>, session_id: Option<String>) -> Result<Self, String> {
        let cwd = cwd.into();
        let local_home = cwd.join(".robocode");
        match Self::new_with_home(&local_home, &cwd, session_id.clone()) {
            Ok(store) => Ok(store),
            Err(_) => {
                let home = default_home_dir()?;
                Self::new_with_home(home, cwd, session_id)
            }
        }
    }

    pub fn new_with_home(
        home_dir: impl Into<PathBuf>,
        cwd: impl Into<PathBuf>,
        session_id: Option<String>,
    ) -> Result<Self, String> {
        let cwd = cwd.into();
        let home_dir = home_dir.into();
        let projects_dir = home_dir.join("projects");
        let project_dir = projects_dir.join(project_key(&cwd));
        let session_id = session_id.unwrap_or_else(|| fresh_id("session"));
        let paths = SessionPaths {
            index_db_path: home_dir.join("index.sqlite3"),
            transcript_path: project_dir.join(format!("{session_id}.jsonl")),
            home_dir,
            projects_dir,
            project_dir,
        };
        fs::create_dir_all(&paths.project_dir).map_err(|err| err.to_string())?;
        let store = Self {
            cwd,
            session_id,
            paths,
        };
        let _ = store.ensure_index();
        Ok(store)
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn transcript_path(&self) -> &Path {
        &self.paths.transcript_path
    }

    pub fn home_dir(&self) -> &Path {
        &self.paths.home_dir
    }

    pub fn index_db_path(&self) -> &Path {
        &self.paths.index_db_path
    }

    pub fn append_entry(&self, entry: &TranscriptEntry) -> Result<(), String> {
        let mut payload = entry.to_json_line();
        payload.push('\n');
        if self.paths.transcript_path.exists() {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&self.paths.transcript_path)
                .map_err(|err| err.to_string())?;
            use std::io::Write;
            file.write_all(payload.as_bytes())
                .map_err(|err| err.to_string())?;
        } else {
            fs::write(&self.paths.transcript_path, payload).map_err(|err| err.to_string())?;
        }
        let _ = self.rebuild_index_for_current();
        Ok(())
    }

    pub fn load_entries(&self) -> Result<Vec<TranscriptEntry>, String> {
        Self::load_entries_from_path(&self.paths.transcript_path)
    }

    pub fn load_entries_from_path(path: &Path) -> Result<Vec<TranscriptEntry>, String> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
        let mut entries = Vec::new();
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            entries.push(TranscriptEntry::from_json_line(line)?);
        }
        Ok(entries)
    }

    pub fn load_latest_for_cwd(
        &self,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        if let Some(summary) = self
            .list_sessions_for_cwd()?
            .into_iter()
            .max_by_key(|item| item.last_updated_at)
        {
            let entries = Self::load_entries_from_path(Path::new(&summary.transcript_path))?;
            return Ok(Some((summary, entries)));
        }
        Ok(None)
    }

    pub fn load_by_id_for_cwd(
        &self,
        session_id: &str,
    ) -> Result<Option<(SessionSummary, Vec<TranscriptEntry>)>, String> {
        for summary in self.list_sessions_for_cwd()? {
            if summary.session_id == session_id {
                let entries = Self::load_entries_from_path(Path::new(&summary.transcript_path))?;
                return Ok(Some((summary, entries)));
            }
        }
        Ok(None)
    }

    pub fn list_sessions_for_cwd(&self) -> Result<Vec<SessionSummary>, String> {
        let from_sqlite = self.list_sessions_from_sqlite()?;
        if !from_sqlite.is_empty() {
            return Ok(from_sqlite);
        }
        self.list_sessions_from_project_dir()
    }

    pub fn ensure_index(&self) -> Result<(), String> {
        if !sqlite_available() {
            return Ok(());
        }
        fs::create_dir_all(&self.paths.home_dir).map_err(|err| err.to_string())?;
        let sql = "CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            cwd TEXT NOT NULL,
            project_key TEXT NOT NULL,
            title TEXT,
            last_preview TEXT,
            message_count INTEGER NOT NULL DEFAULT 0,
            tool_call_count INTEGER NOT NULL DEFAULT 0,
            command_count INTEGER NOT NULL DEFAULT 0,
            last_activity_kind TEXT,
            last_activity_preview TEXT,
            transcript_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_updated_at INTEGER NOT NULL
        );";
        run_sql(&self.paths.index_db_path, sql).map(|_| ())
    }

    pub fn rebuild_index_for_current(&self) -> Result<(), String> {
        let entries = self.load_entries()?;
        let summary = summary_from_entries(
            &self.session_id,
            &self.cwd,
            &self.paths.transcript_path,
            &entries,
        )?;
        self.upsert_summary(&summary)
    }

    fn upsert_summary(&self, summary: &SessionSummary) -> Result<(), String> {
        if !sqlite_available() {
            return Ok(());
        }
        self.ensure_index()?;
        let sql = format!(
            "INSERT INTO sessions (
                session_id, cwd, project_key, title, last_preview, message_count, tool_call_count, command_count, last_activity_kind, last_activity_preview, transcript_path, created_at, last_updated_at
            ) VALUES (
                '{session_id}', '{cwd}', '{project_key}', {title}, {last_preview}, {message_count}, {tool_call_count}, {command_count}, {last_activity_kind}, {last_activity_preview}, '{transcript_path}', {created_at}, {last_updated_at}
            )
            ON CONFLICT(session_id) DO UPDATE SET
                cwd=excluded.cwd,
                project_key=excluded.project_key,
                title=excluded.title,
                last_preview=excluded.last_preview,
                message_count=excluded.message_count,
                tool_call_count=excluded.tool_call_count,
                command_count=excluded.command_count,
                last_activity_kind=excluded.last_activity_kind,
                last_activity_preview=excluded.last_activity_preview,
                transcript_path=excluded.transcript_path,
                last_updated_at=excluded.last_updated_at;",
            session_id = sql_quote(&summary.session_id),
            cwd = sql_quote(&summary.cwd),
            project_key = sql_quote(&project_key(Path::new(&summary.cwd))),
            title = sql_value(summary.title.as_deref()),
            last_preview = sql_value(summary.last_preview.as_deref()),
            message_count = summary.message_count,
            tool_call_count = summary.tool_call_count,
            command_count = summary.command_count,
            last_activity_kind = sql_value(summary.last_activity_kind.as_deref()),
            last_activity_preview = sql_value(summary.last_activity_preview.as_deref()),
            transcript_path = sql_quote(&summary.transcript_path),
            created_at = summary.created_at,
            last_updated_at = summary.last_updated_at,
        );
        run_sql(&self.paths.index_db_path, &sql).map(|_| ())
    }

    fn list_sessions_from_sqlite(&self) -> Result<Vec<SessionSummary>, String> {
        if !sqlite_available() || !self.paths.index_db_path.exists() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT session_id, cwd, transcript_path, title, last_preview, message_count, tool_call_count, command_count, last_activity_kind, last_activity_preview, created_at, last_updated_at
             FROM sessions
             WHERE project_key = '{project_key}'
             ORDER BY last_updated_at DESC;",
            project_key = sql_quote(&project_key(&self.cwd))
        );
        let output = match run_sql(&self.paths.index_db_path, &sql) {
            Ok(output) => output,
            Err(_) => return Ok(Vec::new()),
        };
        let mut summaries = Vec::new();
        for line in output.lines().filter(|line| !line.trim().is_empty()) {
            let parts: Vec<_> = line.split('|').collect();
            if parts.len() != 12 {
                continue;
            }
            summaries.push(SessionSummary {
                session_id: parts[0].to_string(),
                cwd: parts[1].to_string(),
                transcript_path: parts[2].to_string(),
                title: empty_to_none(parts[3]),
                last_preview: empty_to_none(parts[4]),
                message_count: parts[5].parse().unwrap_or_default(),
                tool_call_count: parts[6].parse().unwrap_or_default(),
                command_count: parts[7].parse().unwrap_or_default(),
                last_activity_kind: empty_to_none(parts[8]),
                last_activity_preview: empty_to_none(parts[9]),
                created_at: parts[10].parse().unwrap_or_default(),
                last_updated_at: parts[11].parse().unwrap_or_default(),
            });
        }
        Ok(summaries)
    }

    fn list_sessions_from_project_dir(&self) -> Result<Vec<SessionSummary>, String> {
        let mut summaries = Vec::new();
        if !self.paths.project_dir.exists() {
            return Ok(summaries);
        }
        for entry in fs::read_dir(&self.paths.project_dir).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let session_id = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("session")
                .to_string();
            let entries = Self::load_entries_from_path(&path)?;
            summaries.push(summary_from_entries(
                &session_id,
                &self.cwd,
                &path,
                &entries,
            )?);
        }
        summaries.sort_by_key(|item| std::cmp::Reverse(item.last_updated_at));
        Ok(summaries)
    }
}

fn summary_from_entries(
    session_id: &str,
    cwd: &Path,
    transcript_path: &Path,
    entries: &[TranscriptEntry],
) -> Result<SessionSummary, String> {
    let metadata = fs::metadata(transcript_path).ok();
    let created_at = metadata
        .as_ref()
        .and_then(|meta| meta.created().ok())
        .and_then(|created| created.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_else(now_timestamp);
    let last_updated_at = metadata
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(created_at);

    let mut title = None;
    let mut last_preview = None;
    let mut message_count = 0usize;
    let mut tool_call_count = 0usize;
    let mut command_count = 0usize;
    let mut last_activity_kind = None;
    let mut last_activity_preview = None;
    for entry in entries {
        match entry {
            TranscriptEntry::Message { message } => {
                message_count += 1;
                if title.is_none() && message.role == Role::User {
                    title = Some(truncate_for_preview(&message.content, 48));
                }
                let preview = truncate_for_preview(&message.content, 80);
                last_preview = Some(preview.clone());
                last_activity_preview = Some(preview);
                last_activity_kind = Some(format!("message:{}", message.role.as_str()));
            }
            TranscriptEntry::ToolCall { .. } => {
                tool_call_count += 1;
                last_activity_kind = Some("tool_call".to_string());
            }
            TranscriptEntry::ToolResult { result } => {
                let preview = truncate_for_preview(&result.output, 80);
                last_preview = Some(preview.clone());
                last_activity_preview = Some(preview);
                last_activity_kind = Some("tool_result".to_string());
            }
            TranscriptEntry::Command { entry } => {
                command_count += 1;
                let preview = truncate_for_preview(&entry.output, 80);
                last_preview = Some(preview.clone());
                last_activity_preview = Some(preview);
                last_activity_kind = Some("command".to_string());
            }
            _ => {}
        }
    }

    Ok(SessionSummary {
        session_id: session_id.to_string(),
        cwd: cwd.display().to_string(),
        transcript_path: transcript_path.display().to_string(),
        title,
        last_preview,
        message_count,
        tool_call_count,
        command_count,
        last_activity_kind,
        last_activity_preview,
        created_at,
        last_updated_at,
    })
}

fn sqlite_available() -> bool {
    Command::new("sqlite3")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_sql(database: &Path, sql: &str) -> Result<String, String> {
    let output = Command::new("sqlite3")
        .arg(database)
        .arg(sql)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn default_home_dir() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("ROBOCODE_HOME") {
        return Ok(PathBuf::from(path));
    }
    if cfg!(windows) {
        if let Ok(path) = std::env::var("APPDATA") {
            return Ok(PathBuf::from(path).join("robocode"));
        }
    }
    std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".robocode"))
        .map_err(|_| "Unable to determine RoboCode home directory".to_string())
}

fn project_key(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.display().to_string().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn project_key_for_path(path: &Path) -> String {
    project_key(path)
}

fn sql_quote(input: &str) -> String {
    input.replace('\'', "''")
}

fn sql_value(input: Option<&str>) -> String {
    input
        .map(|value| format!("'{}'", sql_quote(value)))
        .unwrap_or_else(|| "NULL".to_string())
}

fn empty_to_none(input: &str) -> Option<String> {
    if input.is_empty() {
        None
    } else {
        Some(input.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robocode_types::{CommandLogEntry, Message, ToolCall, TranscriptEntry};

    fn temp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("robocode_test_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn jsonl_round_trip_works() {
        let home = temp_home("jsonl");
        let cwd = home.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let store =
            SessionStore::new_with_home(&home, &cwd, Some("session_roundtrip".into())).unwrap();
        store
            .append_entry(&TranscriptEntry::Message {
                message: Message::new(Role::User, "hello"),
            })
            .unwrap();
        let entries = store.load_entries().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn sqlite_index_is_updated() {
        let home = temp_home("sqlite");
        let cwd = home.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let store = SessionStore::new_with_home(&home, &cwd, Some("session_index".into())).unwrap();
        store
            .append_entry(&TranscriptEntry::Message {
                message: Message::new(Role::User, "hello"),
            })
            .unwrap();
        let sessions = store.list_sessions_for_cwd().unwrap();
        assert!(!sessions.is_empty());
    }

    #[test]
    fn summary_metadata_counts_messages_commands_and_tool_calls() {
        let home = temp_home("summary_meta");
        let cwd = home.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let store = SessionStore::new_with_home(&home, &cwd, Some("session_summary".into())).unwrap();
        store
            .append_entry(&TranscriptEntry::Message {
                message: Message::new(Role::User, "inspect summary"),
            })
            .unwrap();
        store
            .append_entry(&TranscriptEntry::ToolCall {
                call: ToolCall {
                    id: "tool_1".into(),
                    name: "read_file".into(),
                    input: Default::default(),
                },
            })
            .unwrap();
        store
            .append_entry(&TranscriptEntry::Command {
                entry: CommandLogEntry {
                    timestamp: now_timestamp(),
                    name: "status".into(),
                    args: vec![],
                    output: "status output".into(),
                },
            })
            .unwrap();
        let summary = store
            .list_sessions_for_cwd()
            .unwrap()
            .into_iter()
            .find(|item| item.session_id == "session_summary")
            .unwrap();
        assert_eq!(summary.message_count, 1);
        assert_eq!(summary.tool_call_count, 1);
        assert_eq!(summary.command_count, 1);
        assert_eq!(summary.last_activity_kind.as_deref(), Some("command"));
        assert_eq!(summary.last_activity_preview.as_deref(), Some("status output"));
    }

    #[test]
    fn falls_back_to_project_scan_when_sqlite_index_has_old_schema() {
        let home = temp_home("sqlite_fallback");
        let cwd = home.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let store = SessionStore::new_with_home(&home, &cwd, Some("session_fallback".into())).unwrap();
        store
            .append_entry(&TranscriptEntry::Message {
                message: Message::new(Role::User, "fallback session"),
            })
            .unwrap();

        if sqlite_available() {
            let legacy_sql = "DROP TABLE IF EXISTS sessions;
                CREATE TABLE sessions (
                    session_id TEXT PRIMARY KEY,
                    cwd TEXT NOT NULL,
                    project_key TEXT NOT NULL,
                    title TEXT,
                    last_preview TEXT,
                    transcript_path TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    last_updated_at INTEGER NOT NULL
                );";
            run_sql(&home.join("index.sqlite3"), legacy_sql).unwrap();
        }

        let sessions = store.list_sessions_for_cwd().unwrap();
        assert!(sessions.iter().any(|item| item.session_id == "session_fallback"));
    }
}
