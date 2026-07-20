use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use viden_types::{
    RecentProjectSummary, RecentSessionSummary, RecentWorkQuery, TranscriptEntry,
    transcript_entry_timestamp, transcript_entry_type,
};

use super::{BatchMarker, project_key, sqlite_available, transcript_batch_marker};

const MAX_RECENT_WORK: usize = 100;
const CANONICAL_ROOT_KEY: &str = "canonical_root";
const CREATED_AT_KEY: &str = "session_created_at";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentWorkInventory {
    pub projects: Vec<RecentProjectSummary>,
    pub sessions: Vec<RecentSessionSummary>,
    pub diagnostics: Vec<String>,
}

#[derive(Default)]
struct TranscriptFacts {
    canonical_root: Option<String>,
    created_at: Option<u64>,
    last_updated_at: u64,
    message_count: u64,
    tool_call_count: u64,
    command_count: u64,
}

struct PendingFacts {
    batch_id: String,
    count: usize,
    seen: usize,
    invalid: bool,
    facts: TranscriptFacts,
}

pub(super) fn query_recent_work(
    home_dir: &Path,
    query: RecentWorkQuery,
) -> Result<RecentWorkInventory, String> {
    let limit = usize::from(query.limit.clamp(1, MAX_RECENT_WORK as u16));
    let projects_dir = home_dir.join("projects");
    let mut diagnostics = BTreeSet::new();
    let mut sessions = Vec::new();

    for project_dir in sorted_directories(&projects_dir)? {
        let project_key_from_dir = project_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        for transcript_path in sorted_transcripts(&project_dir)? {
            let session_id = transcript_path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            let facts = match read_transcript_facts(&transcript_path) {
                Ok(facts) => facts,
                Err(_) => {
                    diagnostics.insert("recent.invalid_transcript".to_string());
                    continue;
                }
            };
            let Some(canonical_root) = facts.canonical_root else {
                diagnostics.insert("recent.missing_canonical_root".to_string());
                continue;
            };
            let Some(created_at) = facts.created_at else {
                diagnostics.insert("recent.missing_stable_timestamp".to_string());
                continue;
            };
            if !Path::new(&canonical_root).is_absolute()
                || project_key(Path::new(&canonical_root)) != project_key_from_dir
            {
                diagnostics.insert("recent.project_identity_mismatch".to_string());
                continue;
            }
            sessions.push(RecentSessionSummary {
                canonical_root,
                session_id,
                created_at,
                last_updated_at: facts.last_updated_at.max(created_at),
                message_count: facts.message_count,
                tool_call_count: facts.tool_call_count,
                command_count: facts.command_count,
            });
        }
    }

    sessions.sort_by(|left, right| {
        right
            .last_updated_at
            .cmp(&left.last_updated_at)
            .then_with(|| left.canonical_root.cmp(&right.canonical_root))
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    let canonical_identities: BTreeSet<_> = sessions
        .iter()
        .map(|session| (session.canonical_root.clone(), session.session_id.clone()))
        .collect();
    match indexed_identities(home_dir) {
        Ok(Some(indexed)) if indexed != canonical_identities => {
            diagnostics.insert("recent.index_stale".to_string());
        }
        Err(_) => {
            diagnostics.insert("recent.index_stale".to_string());
        }
        _ => {}
    }
    sessions.truncate(limit);

    let mut projects_by_root = BTreeMap::new();
    for session in &sessions {
        projects_by_root
            .entry(session.canonical_root.clone())
            .or_insert_with(|| RecentProjectSummary {
                canonical_root: session.canonical_root.clone(),
                display_name: display_name(&session.canonical_root),
                last_updated_at: session.last_updated_at,
                latest_session_id: Some(session.session_id.clone()),
            });
    }
    let mut projects: Vec<_> = projects_by_root.into_values().collect();
    projects.sort_by(|left, right| {
        right
            .last_updated_at
            .cmp(&left.last_updated_at)
            .then_with(|| left.canonical_root.cmp(&right.canonical_root))
    });
    projects.truncate(limit);

    Ok(RecentWorkInventory {
        projects,
        sessions,
        diagnostics: diagnostics.into_iter().collect(),
    })
}

fn indexed_identities(home_dir: &Path) -> Result<Option<BTreeSet<(String, String)>>, String> {
    let index_path = home_dir.join("index.sqlite3");
    let index_is_regular_file = fs::symlink_metadata(&index_path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false);
    if !sqlite_available() || !index_is_regular_file {
        return Ok(None);
    }
    let output = run_readonly_sql(
        &index_path,
        "SELECT hex(cwd), hex(session_id) FROM sessions ORDER BY cwd, session_id;",
    )?;
    if output.trim().is_empty() {
        return Ok(None);
    }
    let mut identities = BTreeSet::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let (cwd, session_id) = line
            .split_once('|')
            .ok_or_else(|| "invalid recent-work index row".to_string())?;
        identities.insert((decode_hex(cwd)?, decode_hex(session_id)?));
    }
    Ok(Some(identities))
}

fn run_readonly_sql(database: &Path, sql: &str) -> Result<String, String> {
    let output = Command::new("sqlite3")
        .arg("-readonly")
        .arg(database)
        .arg(sql)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn decode_hex(input: &str) -> Result<String, String> {
    if !input.len().is_multiple_of(2) {
        return Err("invalid recent-work index text".to_string());
    }
    let mut bytes = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
        bytes.push(u8::from_str_radix(pair, 16).map_err(|error| error.to_string())?);
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn sorted_directories(projects_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let metadata = match fs::symlink_metadata(projects_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(projects_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn sorted_transcripts(project_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(project_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let path = entry.path();
        if file_type.is_file() && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn read_transcript_facts(path: &Path) -> Result<TranscriptFacts, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut facts = TranscriptFacts::default();
    let mut pending: Option<PendingFacts> = None;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        match transcript_batch_marker(&line)? {
            Some(BatchMarker::Begin { batch_id, count }) => {
                let already_pending = pending.is_some();
                pending = Some(PendingFacts {
                    batch_id,
                    count,
                    seen: 0,
                    invalid: already_pending,
                    facts: TranscriptFacts::default(),
                });
            }
            Some(BatchMarker::Commit { batch_id, count }) => {
                if let Some(batch) = pending.take()
                    && !batch.invalid
                    && batch.batch_id == batch_id
                    && batch.count == count
                    && batch.seen == count
                {
                    merge_facts(&mut facts, batch.facts);
                }
            }
            None => {
                if let Some(batch) = &mut pending {
                    batch.seen += 1;
                    if apply_line(&mut batch.facts, &line).is_err() {
                        batch.invalid = true;
                    }
                } else {
                    apply_line(&mut facts, &line)?;
                }
            }
        }
    }
    Ok(facts)
}

fn apply_line(facts: &mut TranscriptFacts, line: &str) -> Result<(), String> {
    let kind = transcript_entry_type(line)?;
    match kind.as_str() {
        "message" => {
            facts.message_count = facts.message_count.saturating_add(1);
            observe_timestamp(facts, transcript_entry_timestamp(line)?);
        }
        "tool_call" => facts.tool_call_count = facts.tool_call_count.saturating_add(1),
        "command" => {
            facts.command_count = facts.command_count.saturating_add(1);
            observe_timestamp(facts, transcript_entry_timestamp(line)?);
        }
        "permission" | "session_meta" | "runtime_event" => {
            observe_timestamp(facts, transcript_entry_timestamp(line)?);
            if kind == "session_meta"
                && let TranscriptEntry::SessionMeta { entry } =
                    TranscriptEntry::from_json_line(line)?
            {
                match entry.key.as_str() {
                    CANONICAL_ROOT_KEY => facts.canonical_root = Some(entry.value),
                    CREATED_AT_KEY => {
                        facts.created_at = entry.value.parse().ok();
                    }
                    _ => {}
                }
            }
        }
        "cost_usage" => observe_timestamp(facts, transcript_entry_timestamp(line)?),
        "tool_result" => {}
        _ => return Err("unknown transcript type".to_string()),
    }
    Ok(())
}

fn observe_timestamp(facts: &mut TranscriptFacts, timestamp: Option<u64>) {
    if let Some(timestamp) = timestamp {
        facts.last_updated_at = facts.last_updated_at.max(timestamp);
    }
}

fn merge_facts(target: &mut TranscriptFacts, source: TranscriptFacts) {
    if source.canonical_root.is_some() {
        target.canonical_root = source.canonical_root;
    }
    if source.created_at.is_some() {
        target.created_at = source.created_at;
    }
    target.last_updated_at = target.last_updated_at.max(source.last_updated_at);
    target.message_count = target.message_count.saturating_add(source.message_count);
    target.tool_call_count = target
        .tool_call_count
        .saturating_add(source.tool_call_count);
    target.command_count = target.command_count.saturating_add(source.command_count);
}

fn display_name(canonical_root: &str) -> String {
    Path::new(canonical_root)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(canonical_root)
        .to_string()
}
