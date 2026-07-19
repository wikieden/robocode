use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use viden_types::RecoverySnapshotReference;

use crate::stores::WorkflowStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySnapshotEntry {
    pub relative_path: PathBuf,
    pub preimage: Option<Vec<u8>>,
    pub unix_mode: Option<u32>,
    pub expected_postimage_sha256: Option<String>,
    pub created_parent_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedRecoverySnapshot {
    pub reference: RecoverySnapshotReference,
    pub entries: Vec<RecoverySnapshotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecoveryManifest {
    schema_version: u32,
    snapshot_id: String,
    entries: Vec<RecoveryManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct RecoveryManifestEntry {
    relative_path: String,
    preimage_sha256: Option<String>,
    unix_mode: Option<u32>,
    expected_postimage_sha256: Option<String>,
    created_parent_dirs: Vec<String>,
}

impl WorkflowStore {
    pub fn write_recovery_snapshot(
        &self,
        snapshot_id: &str,
        entries: &[RecoverySnapshotEntry],
    ) -> Result<RecoverySnapshotReference, String> {
        validate_snapshot_id(snapshot_id)?;
        let recovery_root = self.paths().project_dir.join("recovery");
        create_private_dir(&recovery_root)?;
        let lock = open_private_file(&recovery_root.join("recovery.lock"), false)?;
        lock.lock_exclusive().map_err(|error| error.to_string())?;
        let final_dir = recovery_root.join(snapshot_id);
        if final_dir.exists() {
            return Err(format!("recovery snapshot `{snapshot_id}` already exists"));
        }
        let temp_dir = recovery_root.join(format!(".{snapshot_id}.tmp-{}", std::process::id()));
        if temp_dir.exists() {
            return Err("recovery snapshot temporary path already exists".to_string());
        }
        create_private_dir(&temp_dir)?;
        create_private_dir(&temp_dir.join("blobs"))?;

        let result = (|| {
            let mut manifest_entries = Vec::with_capacity(entries.len());
            let mut seen = BTreeSet::new();
            for entry in entries {
                let relative_path = normalize_relative_path(&entry.relative_path)?;
                if !seen.insert(relative_path.clone()) {
                    return Err(format!("duplicate recovery path `{relative_path}`"));
                }
                let created_parent_dirs = entry
                    .created_parent_dirs
                    .iter()
                    .map(|path| normalize_relative_path(path))
                    .collect::<Result<Vec<_>, _>>()?;
                for parent in &created_parent_dirs {
                    let parent_path = Path::new(parent);
                    if !Path::new(&relative_path).starts_with(parent_path)
                        || parent_path == Path::new(&relative_path)
                    {
                        return Err(format!(
                            "recovery created parent `{parent}` is not an ancestor of `{relative_path}`"
                        ));
                    }
                }
                if let Some(hash) = &entry.expected_postimage_sha256 {
                    validate_sha256(hash)?;
                }
                let preimage_sha256 = entry.preimage.as_ref().map(|bytes| sha256_hex(bytes));
                if let (Some(bytes), Some(hash)) = (&entry.preimage, &preimage_sha256) {
                    let prefix_dir = temp_dir.join("blobs").join(&hash[..2]);
                    create_private_dir(&prefix_dir)?;
                    write_private_new(&prefix_dir.join(hash), bytes)?;
                }
                manifest_entries.push(RecoveryManifestEntry {
                    relative_path,
                    preimage_sha256,
                    unix_mode: entry.unix_mode,
                    expected_postimage_sha256: entry.expected_postimage_sha256.clone(),
                    created_parent_dirs,
                });
            }
            manifest_entries.sort();
            let manifest = RecoveryManifest {
                schema_version: 1,
                snapshot_id: snapshot_id.to_string(),
                entries: manifest_entries,
            };
            let manifest_bytes =
                serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
            let manifest_sha256 = sha256_hex(&manifest_bytes);
            write_private_new(&temp_dir.join("manifest.json"), &manifest_bytes)?;
            sync_dir(&temp_dir)?;
            fs::rename(&temp_dir, &final_dir).map_err(|error| error.to_string())?;
            sync_dir(&recovery_root)?;
            Ok(RecoverySnapshotReference {
                snapshot_id: snapshot_id.to_string(),
                manifest_sha256,
            })
        })();
        if result.is_err() && temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        result
    }

    pub fn load_recovery_snapshot(
        &self,
        reference: &RecoverySnapshotReference,
    ) -> Result<LoadedRecoverySnapshot, String> {
        validate_snapshot_id(&reference.snapshot_id)?;
        validate_sha256(&reference.manifest_sha256)?;
        let recovery_root = self.paths().project_dir.join("recovery");
        let lock = open_private_file(&recovery_root.join("recovery.lock"), false)?;
        lock.lock_shared().map_err(|error| error.to_string())?;
        let snapshot_dir = recovery_root.join(&reference.snapshot_id);
        require_plain_directory(&snapshot_dir)?;
        let manifest_path = snapshot_dir.join("manifest.json");
        require_plain_file(&manifest_path)?;
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| error.to_string())?;
        if sha256_hex(&manifest_bytes) != reference.manifest_sha256 {
            return Err("recovery manifest hash mismatch".to_string());
        }
        let manifest: RecoveryManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
        if manifest.schema_version != 1 || manifest.snapshot_id != reference.snapshot_id {
            return Err("recovery manifest identity mismatch".to_string());
        }
        let mut entries = Vec::with_capacity(manifest.entries.len());
        let mut seen = BTreeSet::new();
        for entry in manifest.entries {
            let relative_path = normalize_relative_path(Path::new(&entry.relative_path))?;
            if !seen.insert(relative_path.clone()) {
                return Err(format!("duplicate recovery path `{relative_path}`"));
            }
            if let Some(hash) = &entry.expected_postimage_sha256 {
                validate_sha256(hash)?;
            }
            let preimage = match entry.preimage_sha256 {
                Some(hash) => {
                    validate_sha256(&hash)?;
                    let blob_path = snapshot_dir.join("blobs").join(&hash[..2]).join(&hash);
                    require_plain_file(&blob_path)?;
                    let bytes = fs::read(blob_path).map_err(|error| error.to_string())?;
                    if sha256_hex(&bytes) != hash {
                        return Err("recovery blob hash mismatch".to_string());
                    }
                    Some(bytes)
                }
                None => None,
            };
            let created_parent_dirs = entry
                .created_parent_dirs
                .iter()
                .map(|path| normalize_relative_path(Path::new(path)).map(PathBuf::from))
                .collect::<Result<Vec<_>, _>>()?;
            entries.push(RecoverySnapshotEntry {
                relative_path: PathBuf::from(relative_path),
                preimage,
                unix_mode: entry.unix_mode,
                expected_postimage_sha256: entry.expected_postimage_sha256,
                created_parent_dirs,
            });
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(LoadedRecoverySnapshot {
            reference: reference.clone(),
            entries,
        })
    }
}

fn validate_snapshot_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 120
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid recovery snapshot id".to_string());
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid recovery sha256".to_string());
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<String, String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("recovery path must be non-empty and relative".to_string());
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => return Err("recovery path cannot contain traversal components".to_string()),
        }
    }
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err("recovery path must contain normal components".to_string());
    }
    Ok(parts.join("/"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn create_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    require_plain_directory(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn open_private_file(path: &Path, create_new: bool) -> Result<fs::File, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(file)
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = open_private_file(path, true)?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn require_plain_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "recovery path `{}` is not a plain directory",
            path.display()
        ));
    }
    Ok(())
}

fn require_plain_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "recovery path `{}` is not a plain file",
            path.display()
        ));
    }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use viden_types::fresh_id;

    use super::*;
    use crate::stores::WorkflowStore;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("viden_recovery_{name}_{}", fresh_id("tmp")));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn recovery_snapshot_is_restricted_content_addressed_and_tamper_evident() {
        let home = temp_dir("home");
        let cwd = temp_dir("cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let secret = b"private preimage bytes".to_vec();
        let entries = vec![RecoverySnapshotEntry {
            relative_path: PathBuf::from("src/lib.rs"),
            preimage: Some(secret.clone()),
            unix_mode: Some(0o640),
            expected_postimage_sha256: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            created_parent_dirs: vec![],
        }];

        let reference = store
            .write_recovery_snapshot("recovery-test-1", &entries)
            .unwrap();
        let loaded = store.load_recovery_snapshot(&reference).unwrap();
        assert_eq!(loaded.entries, entries);
        let snapshot_dir = store.paths().project_dir.join("recovery/recovery-test-1");
        let manifest = fs::read(snapshot_dir.join("manifest.json")).unwrap();
        assert!(
            !manifest
                .windows(secret.len())
                .any(|window| window == secret)
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&snapshot_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(snapshot_dir.join("manifest.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let blob = fs::read_dir(snapshot_dir.join("blobs"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let blob = fs::read_dir(blob).unwrap().next().unwrap().unwrap().path();
        fs::write(blob, b"tampered").unwrap();
        assert!(store.load_recovery_snapshot(&reference).is_err());
    }

    #[test]
    fn recovery_snapshot_rejects_unsafe_or_duplicate_paths() {
        let home = temp_dir("unsafe_home");
        let cwd = temp_dir("unsafe_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        for relative_path in [PathBuf::from("../escape"), PathBuf::from("/absolute")] {
            let result = store.write_recovery_snapshot(
                "recovery-unsafe",
                &[RecoverySnapshotEntry {
                    relative_path,
                    preimage: None,
                    unix_mode: None,
                    expected_postimage_sha256: None,
                    created_parent_dirs: vec![],
                }],
            );
            assert!(result.is_err());
        }
        let duplicate = RecoverySnapshotEntry {
            relative_path: PathBuf::from("src/lib.rs"),
            preimage: None,
            unix_mode: None,
            expected_postimage_sha256: None,
            created_parent_dirs: vec![],
        };
        assert!(
            store
                .write_recovery_snapshot("recovery-duplicate", &[duplicate.clone(), duplicate],)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn recovery_snapshot_rejects_symlinked_private_store_paths() {
        use std::os::unix::fs::symlink;

        let home = temp_dir("symlink_home");
        let cwd = temp_dir("symlink_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let redirected = temp_dir("symlink_redirect");
        symlink(&redirected, store.paths().project_dir.join("recovery")).unwrap();

        let result = store.write_recovery_snapshot(
            "recovery-symlink",
            &[RecoverySnapshotEntry {
                relative_path: PathBuf::from("src/lib.rs"),
                preimage: Some(b"secret".to_vec()),
                unix_mode: None,
                expected_postimage_sha256: None,
                created_parent_dirs: vec![],
            }],
        );

        assert!(result.is_err());
        assert!(!redirected.join("recovery-symlink").exists());
    }
}
