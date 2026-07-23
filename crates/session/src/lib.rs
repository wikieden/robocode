use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use viden_types::{
    RecentWorkQuery, Role, SessionSummary, TranscriptCursor, TranscriptEntry, TranscriptPage,
    TranscriptPageRequest, TranscriptRow, TranscriptRowKind, fresh_id, now_timestamp,
    truncate_for_preview,
};

mod recent;

pub use recent::RecentWorkInventory;

#[derive(Debug, Clone)]
pub struct SessionPaths {
    pub home_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub project_dir: PathBuf,
    pub transcript_path: PathBuf,
    pub index_db_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptBatchCommit {
    pub batch_id: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    cwd: PathBuf,
    session_id: String,
    paths: SessionPaths,
}

impl SessionStore {
    /// Reads the cross-project inventory rooted at one shared session home.
    pub fn query_recent_work(
        home_dir: impl AsRef<Path>,
        query: RecentWorkQuery,
    ) -> Result<RecentWorkInventory, String> {
        recent::query_recent_work(home_dir.as_ref(), query)
    }

    pub fn new(cwd: impl Into<PathBuf>, session_id: Option<String>) -> Result<Self, String> {
        let cwd = cwd.into();
        let local_home = cwd.join(".viden");
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
        let session_id = session_id.unwrap_or_else(|| fresh_id("session"));
        let paths = session_paths(home_dir, &cwd, &session_id);
        fs::create_dir_all(&paths.project_dir).map_err(|err| err.to_string())?;
        let store = Self {
            cwd,
            session_id,
            paths,
        };
        let _ = store.ensure_index();
        Ok(store)
    }

    /// Opens an existing default session store for lookup only.
    ///
    /// This path must not allocate a fresh session id, create project
    /// directories, create the SQLite index, or chmod files. Use `new*` for
    /// mutation-capable stores.
    pub fn open_default_existing_for_query(
        cwd: impl Into<PathBuf>,
    ) -> Result<Option<Self>, String> {
        let cwd = cwd.into();
        let local_home = cwd.join(".viden");
        if let Some(store) = Self::open_existing_for_query(&local_home, &cwd)? {
            return Ok(Some(store));
        }
        let home = default_home_dir()?;
        Self::open_existing_for_query(home, cwd)
    }

    /// Opens an existing session store rooted at `home_dir` for lookup only.
    ///
    /// Returns `Ok(None)` when neither the index nor the project transcript
    /// directory exists; this preserves pristine homes during failed resume
    /// lookup.
    pub fn open_existing_for_query(
        home_dir: impl Into<PathBuf>,
        cwd: impl Into<PathBuf>,
    ) -> Result<Option<Self>, String> {
        let cwd = cwd.into();
        let home_dir = home_dir.into();
        let paths = session_paths(home_dir, &cwd, "__query__");
        if !paths.index_db_path.exists() && !paths.project_dir.exists() {
            return Ok(None);
        }
        Ok(Some(Self {
            cwd,
            session_id: "__query__".to_string(),
            paths,
        }))
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
        self.append_payload(&payload)?;
        let _ = self.rebuild_index_for_current();
        Ok(())
    }

    pub fn append_entries_atomic(&self, entries: &[TranscriptEntry]) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }
        let batch = self.append_entries_precommit(entries)?;
        self.commit_entries_batch(&batch)
    }

    pub fn append_entries_precommit(
        &self,
        entries: &[TranscriptEntry],
    ) -> Result<TranscriptBatchCommit, String> {
        let batch_id = fresh_id("txbatch");
        self.append_entries_batch_payload(entries, &batch_id, false)?;
        Ok(TranscriptBatchCommit {
            batch_id,
            count: entries.len(),
        })
    }

    pub fn commit_entries_batch(&self, batch: &TranscriptBatchCommit) -> Result<(), String> {
        let payload = format!(
            "{{\"type\":\"runtime_event_batch_commit\",\"batch_id\":\"{}\",\"count\":{}}}\n",
            escape_json(&batch.batch_id),
            batch.count
        );
        self.append_payload(&payload)?;
        let _ = self.rebuild_index_for_current();
        Ok(())
    }

    pub fn append_entries_uncommitted_for_test(
        &self,
        entries: &[TranscriptEntry],
        committed_entries: usize,
    ) -> Result<(), String> {
        self.append_entries_atomic_with_uncommitted_for_test(entries, Some(committed_entries))
    }

    fn append_entries_atomic_with_uncommitted_for_test(
        &self,
        entries: &[TranscriptEntry],
        uncommitted_entries: Option<usize>,
    ) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }
        let batch_id = fresh_id("txbatch");
        let take_entries = uncommitted_entries
            .unwrap_or(entries.len())
            .min(entries.len());
        self.append_entries_batch_payload(&entries[..take_entries], &batch_id, true)?;
        if uncommitted_entries.is_none() {
            self.commit_entries_batch(&TranscriptBatchCommit {
                batch_id,
                count: entries.len(),
            })?;
        }
        if uncommitted_entries.is_some() {
            return Err("injected transcript append failure".to_string());
        }
        Ok(())
    }

    fn append_entries_batch_payload(
        &self,
        entries: &[TranscriptEntry],
        batch_id: &str,
        declared_full_count: bool,
    ) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }
        // Replay applies only batches with a commit marker, so a failed append can
        // leave auditable uncommitted lines without changing rebuilt session state.
        let mut payload = format!(
            "{{\"type\":\"runtime_event_batch_begin\",\"batch_id\":\"{}\",\"count\":{}}}\n",
            escape_json(batch_id),
            entries.len()
        );
        for entry in entries {
            payload.push_str(&entry.to_json_line());
            payload.push('\n');
        }
        if !declared_full_count {
            // The caller may append the commit marker only after related durable
            // stores succeed, leaving this batch ignored by replay if they fail.
        }
        self.append_payload(&payload)?;
        Ok(())
    }

    fn append_payload(&self, payload: &str) -> Result<(), String> {
        let _lock = AppendLock::acquire(&self.paths.transcript_path)?;
        if self.paths.transcript_path.exists() {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&self.paths.transcript_path)
                .map_err(|err| err.to_string())?;
            file.write_all(payload.as_bytes())
                .map_err(|err| err.to_string())?;
        } else {
            fs::write(&self.paths.transcript_path, payload).map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn load_entries(&self) -> Result<Vec<TranscriptEntry>, String> {
        Self::load_entries_from_path(&self.paths.transcript_path)
    }

    pub fn load_transcript_page(
        &self,
        request: &TranscriptPageRequest,
    ) -> Result<TranscriptPage, String> {
        if let Some(before) = &request.before
            && before.session_id != request.session_id
        {
            return Err(format!(
                "cursor session `{}` does not match request session `{}`",
                before.session_id, request.session_id
            ));
        }
        let entries = self.load_entries_for_exact_session(&request.session_id)?;
        let rows = transcript_rows_from_entries(&request.session_id, &entries);
        Ok(slice_transcript_rows(
            rows,
            request.before.as_ref(),
            request.limit,
        ))
    }

    fn load_entries_for_exact_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<TranscriptEntry>, String> {
        if session_id == self.session_id {
            return self.load_entries();
        }
        self.load_by_id_for_cwd(session_id)?
            .map(|(_, entries)| entries)
            .ok_or_else(|| format!("session `{session_id}` not found for current project"))
    }

    pub fn load_entries_from_path(path: &Path) -> Result<Vec<TranscriptEntry>, String> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
        let mut entries = Vec::new();
        let mut pending_batch: Option<PendingBatch> = None;
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            match transcript_batch_marker(line)? {
                Some(BatchMarker::Begin { batch_id, count }) => {
                    pending_batch = Some(PendingBatch {
                        batch_id,
                        count,
                        entries: Vec::new(),
                        invalid: pending_batch.is_some(),
                    });
                }
                Some(BatchMarker::Commit { batch_id, count }) => {
                    if let Some(batch) = pending_batch.take()
                        && !batch.invalid
                        && batch.batch_id == batch_id
                        && batch.count == count
                        && batch.entries.len() == count
                    {
                        entries.extend(batch.entries);
                    }
                }
                None => {
                    if let Some(batch) = &mut pending_batch {
                        match TranscriptEntry::from_json_line(line) {
                            Ok(entry) => batch.entries.push(entry),
                            Err(_) => batch.invalid = true,
                        }
                    } else {
                        let entry = TranscriptEntry::from_json_line(line)?;
                        entries.push(entry);
                    }
                }
            }
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
        // A non-empty SQLite index can be stale while a valid project JSONL
        // already exists. Exact lookup falls back to the safe project inventory
        // instead of constructing a path from an untrusted session id.
        for summary in self.list_sessions_from_project_dir()? {
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
                    title = Some(single_line_preview(&message.content, 48));
                }
                let preview = single_line_preview(&message.content, 80);
                last_preview = Some(preview.clone());
                last_activity_preview = Some(preview);
                last_activity_kind = Some(format!("message:{}", message.role.as_str()));
            }
            TranscriptEntry::ToolCall { .. } => {
                tool_call_count += 1;
                last_activity_kind = Some("tool_call".to_string());
            }
            TranscriptEntry::ToolResult { result } => {
                let preview = single_line_preview(&result.output, 80);
                last_preview = Some(preview.clone());
                last_activity_preview = Some(preview);
                last_activity_kind = Some("tool_result".to_string());
            }
            TranscriptEntry::Command { entry } => {
                command_count += 1;
                let preview = single_line_preview(&entry.output, 80);
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

fn transcript_rows_from_entries(
    session_id: &str,
    entries: &[TranscriptEntry],
) -> Vec<TranscriptRow> {
    let mut rows: Vec<TranscriptRow> = Vec::new();
    let mut message_rows: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for (ordinal, entry) in entries.iter().enumerate() {
        if let Some(message_id) = entry.coalescing_message_id()
            && let Some(row_index) = message_rows.get(message_id).copied()
        {
            // Streaming assistant updates reuse a message id. Keep the first
            // committed ordinal as the stable scroll anchor and replace only
            // the displayed row payload with the latest committed content.
            rows[row_index].timestamp = entry.timestamp();
            rows[row_index].kind = TranscriptRowKind::from(entry);
            continue;
        }
        let row = TranscriptRow::from_entry(session_id, ordinal as u64, entry);
        if let Some(message_id) = entry.coalescing_message_id() {
            message_rows.insert(message_id.to_string(), rows.len());
        }
        rows.push(row);
    }
    rows
}

fn slice_transcript_rows(
    rows: Vec<TranscriptRow>,
    before: Option<&TranscriptCursor>,
    limit: u16,
) -> TranscriptPage {
    let limit = usize::from(limit.clamp(1, 500));
    let end = before
        .and_then(|cursor| {
            rows.iter()
                .position(|row| row.cursor.ordinal >= cursor.ordinal)
        })
        .unwrap_or(rows.len());
    let start = end.saturating_sub(limit);
    let selected = rows[start..end].to_vec();
    let has_more = start > 0;
    // `older` is a stable next-page anchor; it must be the first returned row
    // only when rows exist before the selected page.
    let older = has_more
        .then(|| selected.first().map(|row| row.cursor.clone()))
        .flatten();
    let newer = (end < rows.len())
        .then(|| selected.last().map(|row| row.cursor.clone()))
        .flatten();
    TranscriptPage {
        rows: selected,
        older,
        newer,
        has_more,
    }
}

fn single_line_preview(input: &str, max_chars: usize) -> String {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_for_preview(&normalized, max_chars)
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
    default_session_home_dir()
}

/// Returns the shared, user-scoped home used for cross-project session data.
pub fn default_session_home_dir() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("VIDEN_HOME") {
        return Ok(PathBuf::from(path));
    }
    if cfg!(windows)
        && let Ok(path) = std::env::var("APPDATA")
    {
        return Ok(PathBuf::from(path).join("viden"));
    }
    std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".viden"))
        .map_err(|_| "Unable to determine Viden home directory".to_string())
}

fn project_key(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.display().to_string().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn session_paths(home_dir: PathBuf, cwd: &Path, session_id: &str) -> SessionPaths {
    let projects_dir = home_dir.join("projects");
    let project_dir = projects_dir.join(project_key(cwd));
    SessionPaths {
        index_db_path: home_dir.join("index.sqlite3"),
        transcript_path: project_dir.join(format!("{session_id}.jsonl")),
        home_dir,
        projects_dir,
        project_dir,
    }
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

struct PendingBatch {
    batch_id: String,
    count: usize,
    entries: Vec<TranscriptEntry>,
    invalid: bool,
}

enum BatchMarker {
    Begin { batch_id: String, count: usize },
    Commit { batch_id: String, count: usize },
}

fn transcript_batch_marker(line: &str) -> Result<Option<BatchMarker>, String> {
    if line.contains("\"type\":\"runtime_event_batch_begin\"") {
        let Some(batch_id) = extract_json_string_field(line, "batch_id") else {
            return Ok(None);
        };
        let Some(count) = extract_json_usize_field(line, "count") else {
            return Ok(None);
        };
        Ok(Some(BatchMarker::Begin { batch_id, count }))
    } else if line.contains("\"type\":\"runtime_event_batch_commit\"") {
        let Some(batch_id) = extract_json_string_field(line, "batch_id") else {
            return Ok(None);
        };
        let Some(count) = extract_json_usize_field(line, "count") else {
            return Ok(None);
        };
        Ok(Some(BatchMarker::Commit { batch_id, count }))
    } else {
        Ok(None)
    }
}

fn extract_json_string_field(line: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_json_usize_field(line: &str, field: &str) -> Option<usize> {
    let needle = format!("\"{field}\":");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn escape_json(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

struct AppendLock {
    path: PathBuf,
}

impl AppendLock {
    fn acquire(transcript_path: &Path) -> Result<Self, String> {
        let lock_path = transcript_path.with_extension("jsonl.lock");
        for _ in 0..200 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut file) => {
                    file.write_all(b"locked\n").map_err(|err| err.to_string())?;
                    return Ok(Self { path: lock_path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(err) => return Err(err.to_string()),
            }
        }
        Err(format!(
            "timed out waiting for transcript append lock {}",
            lock_path.display()
        ))
    }
}

impl Drop for AppendLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests;
