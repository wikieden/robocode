use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_types::{
    ContextContentKind, ContextHandleRecord, ContextItemRecord, ContextScope, fresh_id,
    now_timestamp,
};

static CONTEXT_ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum ContextError {
    Io(std::io::Error),
    MetadataEncode(serde_json::Error),
    InvalidContentSha256 {
        byte_len: usize,
        category: Sha256ValidationCategory,
    },
    MetadataInvariantViolation {
        invariant: MetadataInvariant,
    },
    InvalidMetadataUtf8 {
        line: usize,
    },
    MalformedMetadata {
        line: usize,
        message: String,
    },
    DivergentDuplicateHandle {
        handle_id: String,
        line: usize,
    },
    MissingHandle {
        handle_id: String,
    },
    MissingBlob {
        content_sha256: String,
    },
    HashMismatch {
        expected: String,
        actual: String,
    },
    ScopeDenied {
        handle_id: String,
    },
    #[cfg(test)]
    InjectedBlobWriteFailure,
}

impl Display for ContextError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(formatter, "context store I/O failed: {err}"),
            Self::MetadataEncode(err) => write!(formatter, "context metadata encode failed: {err}"),
            Self::InvalidContentSha256 { byte_len, category } => {
                write!(
                    formatter,
                    "invalid context content sha256: byte_len={byte_len}, category={category}"
                )
            }
            Self::MetadataInvariantViolation { invariant } => {
                write!(
                    formatter,
                    "context metadata invariant violation: {invariant}"
                )
            }
            Self::InvalidMetadataUtf8 { line } => {
                write!(formatter, "context metadata line {line} is not valid UTF-8")
            }
            Self::MalformedMetadata { line, message } => {
                write!(
                    formatter,
                    "context metadata line {line} is malformed: {message}"
                )
            }
            Self::DivergentDuplicateHandle { handle_id, line } => write!(
                formatter,
                "context metadata line {line} diverges for duplicate handle: {handle_id}"
            ),
            Self::MissingHandle { handle_id } => {
                write!(formatter, "context handle not found: {handle_id}")
            }
            Self::MissingBlob { content_sha256 } => {
                write!(
                    formatter,
                    "context blob not found for sha256: {content_sha256}"
                )
            }
            Self::HashMismatch { expected, actual } => {
                write!(
                    formatter,
                    "context blob hash mismatch: expected {expected}, got {actual}"
                )
            }
            Self::ScopeDenied { handle_id } => {
                write!(formatter, "context scope denied for handle: {handle_id}")
            }
            #[cfg(test)]
            Self::InjectedBlobWriteFailure => write!(formatter, "injected blob write failure"),
        }
    }
}

impl Error for ContextError {}

impl From<std::io::Error> for ContextError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sha256ValidationCategory {
    Empty,
    WrongLength,
    NonAsciiHex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataInvariant {
    HandleItemIdMatchesItem,
    HandleScopeMatchesItem,
    HandleHashMatchesItem,
}

impl Display for MetadataInvariant {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HandleItemIdMatchesItem => write!(formatter, "handle_item_id_matches_item"),
            Self::HandleScopeMatchesItem => write!(formatter, "handle_scope_matches_item"),
            Self::HandleHashMatchesItem => write!(formatter, "handle_hash_matches_item"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum BlobWriteFailurePoint {
    TempAlreadyExists,
    AfterTempCreate,
    AfterWrite,
    AfterSync,
}

impl Display for BlobWriteFailurePoint {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TempAlreadyExists => write!(formatter, "temp_already_exists"),
            Self::AfterTempCreate => write!(formatter, "after_temp_create"),
            Self::AfterWrite => write!(formatter, "after_write"),
            Self::AfterSync => write!(formatter, "after_sync"),
        }
    }
}

impl Display for Sha256ValidationCategory {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(formatter, "empty"),
            Self::WrongLength => write!(formatter, "wrong_length"),
            Self::NonAsciiHex => write!(formatter, "non_ascii_hex"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextPutRequest<'a> {
    pub scope: ContextScope,
    pub kind: ContextContentKind,
    pub content: &'a [u8],
    pub evidence_id: Option<String>,
}

impl<'a> ContextPutRequest<'a> {
    pub fn task(task_id: impl Into<String>, kind: ContextContentKind, content: &'a [u8]) -> Self {
        Self {
            scope: ContextScope::Task(task_id.into()),
            kind,
            content,
            evidence_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredContext {
    pub item: ContextItemRecord,
    pub handle: ContextHandleRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MetadataRecord {
    item: ContextItemRecord,
    handle: ContextHandleRecord,
}

#[derive(Debug, Clone)]
pub struct ContextStore {
    root: PathBuf,
    metadata_path: PathBuf,
    handles: HashMap<String, MetadataRecord>,
}

impl ContextStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ContextError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("blobs"))?;
        let metadata_path = root.join("context-items.jsonl");
        let handles = load_metadata_locked(&metadata_path)?;
        Ok(Self {
            root,
            metadata_path,
            handles,
        })
    }

    pub fn put(&mut self, request: ContextPutRequest<'_>) -> Result<StoredContext, ContextError> {
        let content_sha256 = sha256_hex(request.content);
        self.write_blob_if_absent(&content_sha256, request.content)?;

        let now = now_timestamp();
        let item = ContextItemRecord {
            item_id: unique_context_id("ctxi"),
            scope: request.scope.clone(),
            kind: request.kind,
            content_sha256: content_sha256.clone(),
            title: format!("{:?}", request.kind).to_ascii_lowercase(),
            summary: String::new(),
            token_count: request.content.len() as u64,
            evidence_id: request.evidence_id,
            created_at: Some(now),
        };
        let handle = ContextHandleRecord {
            handle_id: unique_context_id("ctxh"),
            item_id: item.item_id.clone(),
            preferred_view_id: None,
            content_sha256,
            scope: request.scope,
            expires_at: None,
        };
        let record = MetadataRecord {
            item: item.clone(),
            handle: handle.clone(),
        };
        append_metadata(&self.metadata_path, &record)?;
        self.handles.insert(handle.handle_id.clone(), record);
        Ok(StoredContext { item, handle })
    }

    pub fn retrieve(
        &self,
        handle: &ContextHandleRecord,
        scope: &ContextScope,
    ) -> Result<Vec<u8>, ContextError> {
        validate_content_sha256(&handle.content_sha256)?;
        if &handle.scope != scope {
            return Err(ContextError::ScopeDenied {
                handle_id: handle.handle_id.clone(),
            });
        }
        let record = self.lookup_record(handle)?;
        if record.handle.scope != *scope {
            return Err(ContextError::ScopeDenied {
                handle_id: handle.handle_id.clone(),
            });
        }
        if record.handle.item_id != handle.item_id
            || record.handle.content_sha256 != handle.content_sha256
            || record.item.content_sha256 != handle.content_sha256
        {
            return Err(ContextError::HashMismatch {
                expected: record.handle.content_sha256.clone(),
                actual: handle.content_sha256.clone(),
            });
        }

        let blob_path = self.blob_path(&handle.content_sha256);
        if !blob_path.exists() {
            return Err(ContextError::MissingBlob {
                content_sha256: handle.content_sha256.clone(),
            });
        }
        let bytes = fs::read(blob_path)?;
        let actual = sha256_hex(&bytes);
        if actual != handle.content_sha256 {
            return Err(ContextError::HashMismatch {
                expected: handle.content_sha256.clone(),
                actual,
            });
        }
        Ok(bytes)
    }

    fn lookup_record(&self, handle: &ContextHandleRecord) -> Result<MetadataRecord, ContextError> {
        if let Some(record) = self.handles.get(&handle.handle_id) {
            return Ok(record.clone());
        }
        let mut refreshed = load_metadata_locked(&self.metadata_path)?;
        refreshed
            .remove(&handle.handle_id)
            .ok_or_else(|| ContextError::MissingHandle {
                handle_id: handle.handle_id.clone(),
            })
    }

    pub fn blob_count(&self) -> Result<usize, ContextError> {
        let blobs = self.root.join("blobs");
        if !blobs.exists() {
            return Ok(0);
        }
        let mut count = 0;
        for prefix in fs::read_dir(blobs)? {
            let prefix = prefix?;
            if !prefix.file_type()?.is_dir() {
                continue;
            }
            for blob in fs::read_dir(prefix.path())? {
                let blob = blob?;
                if blob.file_type()?.is_file()
                    && blob
                        .file_name()
                        .to_str()
                        .map(is_valid_content_sha256)
                        .unwrap_or(false)
                {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn handle_count(&self) -> usize {
        self.handles.len()
    }

    fn blob_path(&self, content_sha256: &str) -> PathBuf {
        debug_assert!(is_valid_content_sha256(content_sha256));
        self.root
            .join("blobs")
            .join(&content_sha256[..2])
            .join(content_sha256)
    }

    fn write_blob_if_absent(
        &self,
        content_sha256: &str,
        content: &[u8],
    ) -> Result<(), ContextError> {
        self.write_blob_if_absent_with_failure(content_sha256, content, None)
    }

    fn write_blob_if_absent_with_failure(
        &self,
        content_sha256: &str,
        content: &[u8],
        #[cfg_attr(not(test), allow(unused_variables))] failure: Option<BlobWriteFailurePoint>,
    ) -> Result<(), ContextError> {
        validate_content_sha256(content_sha256)?;
        let blob_path = self.blob_path(content_sha256);
        let parent = blob_path.parent().expect("blob path has parent");
        fs::create_dir_all(parent)?;
        let _blob_lock = lock_file_exclusive(&parent.join("blob-write.lock"))?;
        if blob_path.exists() {
            return verify_blob_hash(&blob_path, content_sha256);
        }
        let tmp_path = parent.join(format!(
            ".{content_sha256}.{}.tmp",
            unique_context_id("write")
        ));
        let mut tmp_guard = TempFileCleanup::new(tmp_path.clone());
        #[cfg(test)]
        if failure == Some(BlobWriteFailurePoint::TempAlreadyExists) {
            fs::write(&tmp_path, b"preexisting temp placeholder")?;
        }
        {
            let mut tmp = match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
            {
                Ok(tmp) => tmp,
                Err(err) => {
                    #[cfg(test)]
                    if failure == Some(BlobWriteFailurePoint::TempAlreadyExists) {
                        return Err(ContextError::InjectedBlobWriteFailure);
                    }
                    return Err(ContextError::Io(err));
                }
            };
            #[cfg(test)]
            if failure == Some(BlobWriteFailurePoint::AfterTempCreate) {
                return Err(ContextError::InjectedBlobWriteFailure);
            }
            tmp.write_all(content)?;
            #[cfg(test)]
            if failure == Some(BlobWriteFailurePoint::AfterWrite) {
                return Err(ContextError::InjectedBlobWriteFailure);
            }
            tmp.sync_all()?;
            #[cfg(test)]
            if failure == Some(BlobWriteFailurePoint::AfterSync) {
                return Err(ContextError::InjectedBlobWriteFailure);
            }
        }
        match fs::rename(&tmp_path, &blob_path) {
            Ok(()) => {
                tmp_guard.disarm();
                sync_directory(parent)?;
                Ok(())
            }
            Err(_) if blob_path.exists() => {
                let _ = fs::remove_file(&tmp_path);
                tmp_guard.disarm();
                verify_blob_hash(&blob_path, content_sha256)
            }
            Err(err) => {
                let _ = fs::remove_file(&tmp_path);
                Err(ContextError::Io(err))
            }
        }
    }

    #[cfg(test)]
    fn write_blob_with_injected_failure(
        &self,
        content: &[u8],
        failure: BlobWriteFailurePoint,
    ) -> Result<(), ContextError> {
        let content_sha256 = sha256_hex(content);
        self.write_blob_if_absent_with_failure(&content_sha256, content, Some(failure))
    }
}

struct TempFileCleanup {
    path: PathBuf,
    armed: bool,
}

impl TempFileCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempFileCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn append_metadata(path: &Path, record: &MetadataRecord) -> Result<(), ContextError> {
    let mut payload = serde_json::to_string(record).map_err(ContextError::MetadataEncode)?;
    payload.push('\n');
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let _lock = lock_file_exclusive(&parent.join("context-items.lock"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(payload.as_bytes())?;
    file.flush()?;
    file.sync_all()?;
    sync_directory(parent)?;
    Ok(())
}

fn lock_file_exclusive(path: &Path) -> Result<fs::File, ContextError> {
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    lock.lock_exclusive()?;
    Ok(lock)
}

fn lock_file_shared(path: &Path) -> Result<fs::File, ContextError> {
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    lock.lock_shared()?;
    Ok(lock)
}

fn load_metadata_locked(path: &Path) -> Result<HashMap<String, MetadataRecord>, ContextError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let _lock = lock_file_shared(&parent.join("context-items.lock"))?;
    load_metadata(path)
}

fn load_metadata(path: &Path) -> Result<HashMap<String, MetadataRecord>, ContextError> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let bytes = fs::read(path)?;
    let complete_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    if complete_len == 0 {
        return Ok(HashMap::new());
    }

    let complete = std::str::from_utf8(&bytes[..complete_len]).map_err(|err| {
        ContextError::InvalidMetadataUtf8 {
            line: byte_offset_to_line(&bytes[..complete_len], err.valid_up_to()),
        }
    })?;
    let mut handles = HashMap::new();
    for (index, line) in complete.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: MetadataRecord =
            serde_json::from_str(line).map_err(|err| ContextError::MalformedMetadata {
                line: index + 1,
                message: err.to_string(),
            })?;
        validate_metadata_record(&record)?;
        if let Some(existing) = handles.get(&record.handle.handle_id) {
            if existing != &record {
                return Err(ContextError::DivergentDuplicateHandle {
                    handle_id: record.handle.handle_id,
                    line: index + 1,
                });
            }
            continue;
        }
        handles.insert(record.handle.handle_id.clone(), record);
    }
    Ok(handles)
}

fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    format!("{digest:x}")
}

fn unique_context_id(prefix: &str) -> String {
    let sequence = CONTEXT_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}_{sequence}", fresh_id(prefix))
}

fn validate_metadata_record(record: &MetadataRecord) -> Result<(), ContextError> {
    validate_content_sha256(&record.item.content_sha256)?;
    validate_content_sha256(&record.handle.content_sha256)?;
    if record.item.item_id != record.handle.item_id {
        return Err(ContextError::MetadataInvariantViolation {
            invariant: MetadataInvariant::HandleItemIdMatchesItem,
        });
    }
    if record.item.scope != record.handle.scope {
        return Err(ContextError::MetadataInvariantViolation {
            invariant: MetadataInvariant::HandleScopeMatchesItem,
        });
    }
    if record.item.content_sha256 != record.handle.content_sha256 {
        return Err(ContextError::MetadataInvariantViolation {
            invariant: MetadataInvariant::HandleHashMatchesItem,
        });
    }
    Ok(())
}

fn validate_content_sha256(value: &str) -> Result<(), ContextError> {
    match content_sha256_validation_category(value) {
        None => Ok(()),
        Some(category) => Err(ContextError::InvalidContentSha256 {
            byte_len: value.len(),
            category,
        }),
    }
}

fn is_valid_content_sha256(value: &str) -> bool {
    content_sha256_validation_category(value).is_none()
}

fn content_sha256_validation_category(value: &str) -> Option<Sha256ValidationCategory> {
    if value.is_empty() {
        Some(Sha256ValidationCategory::Empty)
    } else if value.len() != 64 {
        Some(Sha256ValidationCategory::WrongLength)
    } else if !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(Sha256ValidationCategory::NonAsciiHex)
    } else {
        None
    }
}

fn verify_blob_hash(path: &Path, expected: &str) -> Result<(), ContextError> {
    let bytes = fs::read(path)?;
    let actual = sha256_hex(&bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(ContextError::HashMismatch {
            expected: expected.to_string(),
            actual,
        })
    }
}

fn sync_directory(path: &Path) -> Result<(), ContextError> {
    match fs::File::open(path).and_then(|dir| dir.sync_all()) {
        Ok(()) => Ok(()),
        Err(err) if directory_sync_error_is_unsupported(&err) => Ok(()),
        Err(err) => Err(ContextError::Io(err)),
    }
}

fn directory_sync_error_is_unsupported(err: &std::io::Error) -> bool {
    match err.kind() {
        std::io::ErrorKind::InvalidInput => true,
        // Some non-Unix platforms do not expose syncable directory handles.
        // Unix PermissionDenied is durability-relevant and must propagate.
        #[cfg(windows)]
        std::io::ErrorKind::PermissionDenied => true,
        _ => false,
    }
}

fn byte_offset_to_line(bytes: &[u8], offset: usize) -> usize {
    bytes[..offset.min(bytes.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::thread;

    use viden_types::{ContextContentKind, ContextScope, fresh_id};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("viden_context_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn metadata_line_count(root: &Path) -> usize {
        fs::read_to_string(root.join("context-items.jsonl"))
            .unwrap()
            .lines()
            .count()
    }

    fn temp_blob_paths(root: &Path) -> Vec<PathBuf> {
        let blobs = root.join("blobs");
        if !blobs.exists() {
            return Vec::new();
        }
        let mut paths = Vec::new();
        for prefix in fs::read_dir(blobs).unwrap() {
            let prefix = prefix.unwrap();
            if !prefix.file_type().unwrap().is_dir() {
                continue;
            }
            for entry in fs::read_dir(prefix.path()).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') && name.ends_with(".tmp") {
                    paths.push(entry.path());
                }
            }
        }
        paths
    }

    #[test]
    fn repeated_content_reuses_one_canonical_blob() {
        let root = temp_dir("dedup");
        let mut store = ContextStore::open(&root).unwrap();

        let first = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Log,
                b"same",
            ))
            .unwrap();
        let second = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Log,
                b"same",
            ))
            .unwrap();

        assert_eq!(first.item.content_sha256, second.item.content_sha256);
        assert_eq!(store.blob_count().unwrap(), 1);
        assert_eq!(
            store
                .retrieve(&first.handle, &ContextScope::Task("task-1".into()))
                .unwrap(),
            b"same"
        );
    }

    #[test]
    fn retrieve_returns_byte_identical_content_after_reopen() {
        let root = temp_dir("reopen");
        let bytes = b"\0binary\ncontent\xff";
        let handle = {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Diagnostic,
                    bytes,
                ))
                .unwrap()
                .handle
        };

        let store = ContextStore::open(&root).unwrap();

        assert_eq!(
            store
                .retrieve(&handle, &ContextScope::Task("task-1".into()))
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn stale_reader_refreshes_metadata_on_retrieve_miss() {
        let root = temp_dir("stale-reader-refresh");
        let stale_reader = ContextStore::open(&root).unwrap();
        let mut writer = ContextStore::open(&root).unwrap();
        let stored = writer
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"from another store",
            ))
            .unwrap();

        assert_eq!(
            stale_reader
                .retrieve(&stored.handle, &ContextScope::Task("task-1".into()))
                .unwrap(),
            b"from another store"
        );
        assert!(matches!(
            stale_reader.retrieve(&stored.handle, &ContextScope::Task("task-2".into())),
            Err(ContextError::ScopeDenied { .. })
        ));
    }

    #[test]
    fn modified_blob_returns_hash_mismatch() {
        let root = temp_dir("hash-mismatch");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"original",
            ))
            .unwrap();
        fs::write(store.blob_path(&stored.item.content_sha256), b"modified").unwrap();

        assert!(matches!(
            store.retrieve(&stored.handle, &ContextScope::Task("task-1".into())),
            Err(ContextError::HashMismatch { .. })
        ));
    }

    #[test]
    fn mismatched_scope_returns_scope_denied() {
        let root = temp_dir("scope-denied");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"scoped",
            ))
            .unwrap();

        assert!(matches!(
            store.retrieve(&stored.handle, &ContextScope::Task("task-2".into())),
            Err(ContextError::ScopeDenied { .. })
        ));
    }

    #[test]
    fn reopen_ignores_only_trailing_partial_metadata_append() {
        let root = temp_dir("trailing-partial");
        let handle = {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Text,
                    b"valid",
                ))
                .unwrap()
                .handle
        };
        use std::io::Write;
        let mut log = fs::OpenOptions::new()
            .append(true)
            .open(root.join("context-items.jsonl"))
            .unwrap();
        write!(log, "{{\"partial\":").unwrap();

        let store = ContextStore::open(&root).unwrap();

        assert_eq!(
            store
                .retrieve(&handle, &ContextScope::Task("task-1".into()))
                .unwrap(),
            b"valid"
        );
    }

    #[test]
    fn reopen_rejects_malformed_metadata_before_trailing_append() {
        let root = temp_dir("malformed-middle");
        {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Text,
                    b"valid",
                ))
                .unwrap();
        }
        fs::write(
            root.join("context-items.jsonl"),
            "{\"bad\":\n{\"also\":\"unreachable\"}\n",
        )
        .unwrap();

        assert!(matches!(
            ContextStore::open(&root),
            Err(ContextError::MalformedMetadata { .. })
        ));
    }

    #[test]
    fn retrieve_rejects_invalid_caller_supplied_hashes_without_panicking() {
        let root = temp_dir("retrieve-invalid-hashes");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"valid",
            ))
            .unwrap();

        for invalid_hash in ["", "abc", "../outside", &format!("{}z", "a".repeat(63))] {
            let mut handle = stored.handle.clone();
            handle.content_sha256 = invalid_hash.to_string();

            assert!(matches!(
                store.retrieve(&handle, &ContextScope::Task("task-1".into())),
                Err(ContextError::InvalidContentSha256 { .. })
            ));
        }
    }

    #[test]
    fn invalid_hash_error_formats_do_not_expose_raw_caller_input() {
        let root = temp_dir("redacted-invalid-hashes");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"valid",
            ))
            .unwrap();

        for invalid_hash in ["../outside/secret.txt", "sk-proj-test-secret-value"] {
            let mut handle = stored.handle.clone();
            handle.content_sha256 = invalid_hash.to_string();
            let error = store
                .retrieve(&handle, &ContextScope::Task("task-1".into()))
                .unwrap_err();

            assert!(matches!(
                error,
                ContextError::InvalidContentSha256 {
                    byte_len: _,
                    category: _
                }
            ));
            for formatted in error_formats(&error) {
                assert!(
                    !formatted.contains(invalid_hash),
                    "leaked invalid hash in error formatting: {formatted}"
                );
            }
        }
    }

    #[test]
    fn replay_rejects_invalid_hashes_without_constructing_blob_paths() {
        let root = temp_dir("replay-invalid-hashes");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"valid",
            ))
            .unwrap();

        for (index, invalid_hash) in ["", "abc", "../outside", &format!("{}z", "a".repeat(63))]
            .into_iter()
            .enumerate()
        {
            for field in ["item", "handle"] {
                let case_root = root.join(format!("case-{index}-{field}"));
                fs::create_dir_all(&case_root).unwrap();
                let mut record = MetadataRecord {
                    item: stored.item.clone(),
                    handle: stored.handle.clone(),
                };
                if field == "item" {
                    record.item.content_sha256 = invalid_hash.to_string();
                } else {
                    record.handle.content_sha256 = invalid_hash.to_string();
                }
                let line = serde_json::to_string(&record).unwrap();
                fs::write(case_root.join("context-items.jsonl"), format!("{line}\n")).unwrap();

                assert!(matches!(
                    ContextStore::open(&case_root),
                    Err(ContextError::InvalidContentSha256 { .. })
                ));
            }
        }
    }

    #[test]
    fn existing_canonical_blob_is_verified_before_metadata_append() {
        let root = temp_dir("existing-corrupt-blob");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"same",
            ))
            .unwrap();
        fs::write(store.blob_path(&stored.item.content_sha256), b"corrupt").unwrap();

        assert!(matches!(
            store.put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"same",
            )),
            Err(ContextError::HashMismatch { .. })
        ));
        assert_eq!(metadata_line_count(&root), 1);
    }

    #[test]
    fn replay_rejects_invalid_utf8_in_complete_metadata_line() {
        let root = temp_dir("invalid-utf8");
        fs::write(root.join("context-items.jsonl"), [0xff, b'\n']).unwrap();

        assert!(matches!(
            ContextStore::open(&root),
            Err(ContextError::InvalidMetadataUtf8 { .. })
        ));
    }

    #[test]
    fn replay_allows_exact_duplicate_handle_records() {
        let root = temp_dir("exact-duplicate");
        {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Text,
                    b"valid",
                ))
                .unwrap();
        }
        let line = fs::read_to_string(root.join("context-items.jsonl")).unwrap();
        fs::write(root.join("context-items.jsonl"), format!("{line}{line}")).unwrap();

        assert!(ContextStore::open(&root).is_ok());
    }

    #[test]
    fn replay_rejects_divergent_duplicate_handle_records() {
        let root = temp_dir("divergent-duplicate");
        let stored = {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Text,
                    b"valid",
                ))
                .unwrap()
        };
        let mut divergent = MetadataRecord {
            item: stored.item,
            handle: stored.handle,
        };
        divergent.item.title = "different".into();
        let line = serde_json::to_string(&divergent).unwrap();
        let mut metadata = fs::read_to_string(root.join("context-items.jsonl")).unwrap();
        metadata.push_str(&line);
        metadata.push('\n');
        fs::write(root.join("context-items.jsonl"), metadata).unwrap();

        assert!(matches!(
            ContextStore::open(&root),
            Err(ContextError::DivergentDuplicateHandle { .. })
        ));
    }

    #[test]
    fn replay_rejects_handle_item_id_mismatch() {
        let root = temp_dir("item-id-mismatch");
        let stored = {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Text,
                    b"valid",
                ))
                .unwrap()
        };
        let mut record = MetadataRecord {
            item: stored.item,
            handle: stored.handle,
        };
        record.handle.item_id = "ctxi-other".into();
        let line = serde_json::to_string(&record).unwrap();
        fs::write(root.join("context-items.jsonl"), format!("{line}\n")).unwrap();

        assert!(matches!(
            ContextStore::open(&root),
            Err(ContextError::MetadataInvariantViolation { .. })
        ));
    }

    #[test]
    fn replay_rejects_handle_scope_mismatch() {
        let root = temp_dir("scope-mismatch");
        let stored = {
            let mut store = ContextStore::open(&root).unwrap();
            store
                .put(ContextPutRequest::task(
                    "task-1",
                    ContextContentKind::Text,
                    b"valid",
                ))
                .unwrap()
        };
        let mut record = MetadataRecord {
            item: stored.item,
            handle: stored.handle,
        };
        record.handle.scope = ContextScope::Task("task-2".into());
        let line = serde_json::to_string(&record).unwrap();
        fs::write(root.join("context-items.jsonl"), format!("{line}\n")).unwrap();

        assert!(matches!(
            ContextStore::open(&root),
            Err(ContextError::MetadataInvariantViolation { .. })
        ));
    }

    #[test]
    fn injected_prerename_blob_write_failures_remove_temp_bytes() {
        let root = temp_dir("injected-temp-cleanup");
        let store = ContextStore::open(&root).unwrap();
        let content = b"raw context bytes that must not remain in temp files";

        for failure in [
            BlobWriteFailurePoint::TempAlreadyExists,
            BlobWriteFailurePoint::AfterTempCreate,
            BlobWriteFailurePoint::AfterWrite,
            BlobWriteFailurePoint::AfterSync,
        ] {
            assert!(matches!(
                store.write_blob_with_injected_failure(content, failure),
                Err(ContextError::InjectedBlobWriteFailure)
            ));
            assert!(temp_blob_paths(&root).is_empty());
            assert!(!find_file_containing(&root, content));
        }
    }

    #[test]
    fn failed_existing_corrupt_blob_path_leaves_no_temp_files() {
        let root = temp_dir("corrupt-existing-no-temp");
        let mut store = ContextStore::open(&root).unwrap();
        let stored = store
            .put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"same",
            ))
            .unwrap();
        fs::write(store.blob_path(&stored.item.content_sha256), b"corrupt").unwrap();

        assert!(matches!(
            store.put(ContextPutRequest::task(
                "task-1",
                ContextContentKind::Text,
                b"same",
            )),
            Err(ContextError::HashMismatch { .. })
        ));
        assert!(temp_blob_paths(&root).is_empty());
    }

    #[test]
    fn directory_sync_error_classification_does_not_swallow_permission_denied() {
        let permission_denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let invalid_input = std::io::Error::from(std::io::ErrorKind::InvalidInput);

        assert!(!directory_sync_error_is_unsupported(&permission_denied));
        assert!(directory_sync_error_is_unsupported(&invalid_input));
    }

    #[test]
    fn concurrent_independent_writers_append_replayable_metadata() {
        let root = temp_dir("concurrent-writers");
        let writers = 16;
        let writes_per_thread = 20;
        let threads: Vec<_> = (0..writers)
            .map(|writer| {
                let root = root.clone();
                thread::spawn(move || {
                    let mut store = ContextStore::open(&root).unwrap();
                    for index in 0..writes_per_thread {
                        let content = format!("writer-{writer}-content-{index}");
                        store
                            .put(ContextPutRequest::task(
                                format!("task-{writer}"),
                                ContextContentKind::Log,
                                content.as_bytes(),
                            ))
                            .unwrap();
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().unwrap();
        }

        let store = ContextStore::open(&root).unwrap();

        assert_eq!(store.handle_count(), writers * writes_per_thread);
        assert_eq!(metadata_line_count(&root), writers * writes_per_thread);
    }

    fn error_formats(error: &ContextError) -> Vec<String> {
        let mut formats = vec![error.to_string(), format!("{error:?}")];
        let mut source = std::error::Error::source(error);
        while let Some(next) = source {
            formats.push(next.to_string());
            formats.push(format!("{next:?}"));
            source = next.source();
        }
        formats
    }

    fn find_file_containing(root: &Path, needle: &[u8]) -> bool {
        let mut stack = vec![root.to_path_buf()];
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    stack.push(entry.unwrap().path());
                }
                continue;
            }
            if fs::read(&path)
                .map(|bytes| contains_bytes(&bytes, needle))
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty()
            && haystack
                .windows(needle.len())
                .any(|window| window == needle)
    }
}
