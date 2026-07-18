use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_types::{
    ContextContentKind, ContextHandleRecord, ContextItemRecord, ContextScope, fresh_id,
    now_timestamp,
};

#[derive(Debug)]
pub enum ContextError {
    Io(std::io::Error),
    MetadataEncode(serde_json::Error),
    MalformedMetadata { line: usize, message: String },
    MissingHandle { handle_id: String },
    MissingBlob { content_sha256: String },
    HashMismatch { expected: String, actual: String },
    ScopeDenied { handle_id: String },
}

impl Display for ContextError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(formatter, "context store I/O failed: {err}"),
            Self::MetadataEncode(err) => write!(formatter, "context metadata encode failed: {err}"),
            Self::MalformedMetadata { line, message } => {
                write!(
                    formatter,
                    "context metadata line {line} is malformed: {message}"
                )
            }
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
        }
    }
}

impl Error for ContextError {}

impl From<std::io::Error> for ContextError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let handles = load_metadata(&metadata_path)?;
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
            item_id: fresh_id("ctxi"),
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
            handle_id: fresh_id("ctxh"),
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
        if &handle.scope != scope {
            return Err(ContextError::ScopeDenied {
                handle_id: handle.handle_id.clone(),
            });
        }
        let record =
            self.handles
                .get(&handle.handle_id)
                .ok_or_else(|| ContextError::MissingHandle {
                    handle_id: handle.handle_id.clone(),
                })?;
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
                if blob?.file_type()?.is_file() {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    fn blob_path(&self, content_sha256: &str) -> PathBuf {
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
        let blob_path = self.blob_path(content_sha256);
        if blob_path.exists() {
            return Ok(());
        }
        let parent = blob_path.parent().expect("blob path has parent");
        fs::create_dir_all(parent)?;
        let tmp_path = parent.join(format!(".{content_sha256}.{}.tmp", fresh_id("write")));
        fs::write(&tmp_path, content)?;
        match fs::rename(&tmp_path, &blob_path) {
            Ok(()) => Ok(()),
            Err(err) if blob_path.exists() => {
                let _ = fs::remove_file(&tmp_path);
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    return Ok(());
                }
                Ok(())
            }
            Err(err) => {
                let _ = fs::remove_file(&tmp_path);
                Err(ContextError::Io(err))
            }
        }
    }
}

fn append_metadata(path: &Path, record: &MetadataRecord) -> Result<(), ContextError> {
    let mut payload = serde_json::to_string(record).map_err(ContextError::MetadataEncode)?;
    payload.push('\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(payload.as_bytes())?;
    Ok(())
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

    let complete = String::from_utf8_lossy(&bytes[..complete_len]);
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
        handles.insert(record.handle.handle_id.clone(), record);
    }
    Ok(handles)
}

fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use viden_types::{ContextContentKind, ContextScope, fresh_id};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("viden_context_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
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
}
