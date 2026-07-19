use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
#[cfg(unix)]
use std::{
    ffi::CString,
    os::unix::{
        ffi::OsStrExt,
        io::{AsRawFd, FromRawFd},
    },
};

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
                    write_private_content_addressed(&prefix_dir.join(hash), bytes)?;
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
        require_plain_directory(&recovery_root)?;
        let recovery_dir = open_existing_private_directory(&recovery_root)?;
        let lock = open_existing_private_file_at(
            &recovery_dir,
            Path::new("recovery.lock"),
            &recovery_root.join("recovery.lock"),
        )?
        .file;
        lock.lock_shared().map_err(|error| error.to_string())?;
        let snapshot_dir_path = recovery_root.join(&reference.snapshot_id);
        let snapshot_dir = open_existing_private_directory_at(
            &recovery_dir,
            Path::new(&reference.snapshot_id),
            &snapshot_dir_path,
        )?;
        let manifest_path = snapshot_dir_path.join("manifest.json");
        let manifest_bytes = read_existing_private_file_at(
            &snapshot_dir,
            Path::new("manifest.json"),
            &manifest_path,
        )?
        .bytes;
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
                    let blob_dir_path = snapshot_dir_path.join("blobs").join(&hash[..2]);
                    let blob_dir = open_existing_private_directory_at(
                        &snapshot_dir,
                        Path::new("blobs").join(&hash[..2]).as_path(),
                        &blob_dir_path,
                    )?;
                    let blob_path = blob_dir_path.join(&hash);
                    let bytes =
                        read_existing_private_file_at(&blob_dir, Path::new(&hash), &blob_path)?
                            .bytes;
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
    require_plain_path(path, true)?;
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
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
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

struct OpenPrivateFile {
    file: File,
    bytes: Vec<u8>,
}

fn open_existing_private_directory(path: &Path) -> Result<File, String> {
    #[cfg(unix)]
    {
        open_existing_private_directory_unix(path)
    }
    #[cfg(not(unix))]
    {
        require_plain_directory(path)?;
        File::open(path).map_err(|error| error.to_string())
    }
}

#[cfg(unix)]
fn open_existing_private_directory_unix(path: &Path) -> Result<File, String> {
    reject_symlink_ancestors(path)?;
    let c_path = cstring_for_open(path.as_os_str(), path)?;
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(format!(
            "{}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "recovery path `{}` is not a plain directory",
            path.display()
        ));
    }
    Ok(file)
}

fn open_existing_private_directory_at(
    root: &File,
    relative: &Path,
    display_path: &Path,
) -> Result<File, String> {
    #[cfg(unix)]
    {
        open_private_path_at_unix(root, relative, display_path, true)
    }
    #[cfg(not(unix))]
    {
        require_plain_directory(display_path)?;
        File::open(display_path).map_err(|error| error.to_string())
    }
}

fn read_existing_private_file_at(
    root: &File,
    relative: &Path,
    display_path: &Path,
) -> Result<OpenPrivateFile, String> {
    #[cfg(unix)]
    {
        let mut file = open_private_path_at_unix(root, relative, display_path, false)?;
        let bytes = read_open_private_file(display_path, &mut file)?;
        Ok(OpenPrivateFile { file, bytes })
    }
    #[cfg(not(unix))]
    {
        let mut file = File::open(display_path).map_err(|error| error.to_string())?;
        validate_open_private_file(display_path, &file)?;
        let bytes = read_open_private_file(display_path, &mut file)?;
        Ok(OpenPrivateFile { file, bytes })
    }
}

fn open_existing_private_file_at(
    root: &File,
    relative: &Path,
    display_path: &Path,
) -> Result<OpenPrivateFile, String> {
    read_existing_private_file_at(root, relative, display_path)
}

#[cfg(unix)]
fn open_private_path_at_unix(
    root: &File,
    relative: &Path,
    display_path: &Path,
    directory: bool,
) -> Result<File, String> {
    let mut components = relative.components().peekable();
    let mut current = root.try_clone().map_err(|error| error.to_string())?;
    while let Some(component) = components.next() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "recovery path `{}` contains unsafe traversal",
                display_path.display()
            ));
        };
        let name = cstring_for_open(segment, display_path)?;
        let final_component = components.peek().is_none();
        let flags = if final_component && !directory {
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW
        } else {
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW
        };
        let fd = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(format!(
                "{}: {}",
                display_path.display(),
                std::io::Error::last_os_error()
            ));
        }
        let opened = unsafe { File::from_raw_fd(fd) };
        let metadata = opened.metadata().map_err(|error| error.to_string())?;
        if final_component {
            if directory && !metadata.file_type().is_dir() {
                return Err(format!(
                    "recovery path `{}` is not a plain directory",
                    display_path.display()
                ));
            }
            if !directory && !metadata.file_type().is_file() {
                return Err(format!(
                    "recovery path `{}` is not a plain file",
                    display_path.display()
                ));
            }
            return Ok(opened);
        }
        if !metadata.file_type().is_dir() {
            return Err(format!(
                "recovery path `{}` crosses non-directory ancestor",
                display_path.display()
            ));
        }
        current = opened;
    }
    Err("recovery relative path cannot be empty".to_string())
}

fn validate_open_private_file(path: &Path, file: &File) -> Result<(), String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "recovery path `{}` is not a plain file",
            path.display()
        ));
    }
    Ok(())
}

fn read_open_private_file(path: &Path, file: &mut File) -> Result<Vec<u8>, String> {
    validate_open_private_file(path, file)?;
    let len = file.metadata().map_err(|error| error.to_string())?.len();
    let capacity = usize::try_from(len)
        .map_err(|_| format!("recovery file `{}` is too large", path.display()))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

#[cfg(unix)]
fn cstring_for_open(value: &std::ffi::OsStr, display_path: &Path) -> Result<CString, String> {
    CString::new(value.as_bytes()).map_err(|_| {
        format!(
            "recovery path `{}` contains a NUL byte",
            display_path.display()
        )
    })
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = open_private_file(path, true)?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn write_private_content_addressed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    match write_private_new(path, bytes) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            let parent = path
                .parent()
                .ok_or_else(|| "recovery blob path has no parent".to_string())?;
            let file_name = path
                .file_name()
                .ok_or_else(|| "recovery blob path has no file name".to_string())?;
            let parent_dir = open_existing_private_directory(parent)?;
            let existing =
                read_existing_private_file_at(&parent_dir, Path::new(file_name), path)?.bytes;
            if existing == bytes {
                Ok(())
            } else {
                Err("recovery content-addressed blob collision".to_string())
            }
        }
        Err(error) => Err(error),
    }
}

fn require_plain_directory(path: &Path) -> Result<(), String> {
    require_plain_path(path, true)
}

fn require_plain_path(path: &Path, directory: bool) -> Result<(), String> {
    reject_symlink_ancestors(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let wrong_kind = if directory {
        !metadata.is_dir()
    } else {
        !metadata.is_file()
    };
    if metadata.file_type().is_symlink() || wrong_kind {
        return Err(format!(
            "recovery path `{}` is not a plain {}",
            path.display(),
            if directory { "directory" } else { "file" }
        ));
    }
    Ok(())
}

fn reject_symlink_ancestors(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    let mut components = path.components().peekable();
    let mut inside_recovery_tree = false;
    while let Some(component) = components.next() {
        current.push(component.as_os_str());
        if components.peek().is_none() {
            break;
        }
        if !inside_recovery_tree {
            inside_recovery_tree =
                matches!(component, Component::Normal(segment) if segment == "recovery");
            if !inside_recovery_tree {
                continue;
            }
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "recovery path `{}` crosses symlink ancestor `{}`",
                    path.display(),
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
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

    use viden_types::{RecoverySnapshotReference, fresh_id};

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

    #[test]
    fn recovery_snapshot_reuses_identical_preimage_blobs_for_multiple_paths() {
        let home = temp_dir("dedupe_home");
        let cwd = temp_dir("dedupe_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let shared_preimage = b"same private bytes".to_vec();

        let reference = store
            .write_recovery_snapshot(
                "recovery-dedupe",
                &[
                    RecoverySnapshotEntry {
                        relative_path: PathBuf::from("src/lib.rs"),
                        preimage: Some(shared_preimage.clone()),
                        unix_mode: Some(0o644),
                        expected_postimage_sha256: None,
                        created_parent_dirs: vec![],
                    },
                    RecoverySnapshotEntry {
                        relative_path: PathBuf::from("src/main.rs"),
                        preimage: Some(shared_preimage.clone()),
                        unix_mode: Some(0o644),
                        expected_postimage_sha256: None,
                        created_parent_dirs: vec![],
                    },
                ],
            )
            .unwrap();

        let loaded = store.load_recovery_snapshot(&reference).unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert!(
            loaded
                .entries
                .iter()
                .all(|entry| entry.preimage.as_ref() == Some(&shared_preimage))
        );
        let blob_root = store
            .paths()
            .project_dir
            .join("recovery/recovery-dedupe/blobs");
        let blob_count = fs::read_dir(blob_root)
            .unwrap()
            .map(|prefix| fs::read_dir(prefix.unwrap().path()).unwrap().count())
            .sum::<usize>();
        assert_eq!(blob_count, 1);
    }

    #[test]
    fn recovery_snapshot_loader_uses_single_handle_source_guard() {
        let source = include_str!("recovery.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production recovery module");
        assert!(production.contains("lock.lock_shared()"));
        assert!(production.contains("read_existing_private_file_at"));
        assert!(production.contains("read_open_private_file"));
        assert!(production.contains(".metadata()"));
        assert!(production.contains(".read_to_end("));
        assert!(
            !production.contains("fs::read("),
            "production recovery loading must not reopen by path after metadata checks"
        );
        assert!(
            !production.contains("require_plain_file("),
            "production recovery file loading must validate metadata from the opened handle"
        );
        assert!(
            !production.contains("open_existing_private_file(display_path)"),
            "non-Unix recovery branch must not reference the removed path-check helper"
        );
        assert!(production.contains("File::open(display_path)"));
        assert!(production.contains("validate_open_private_file(display_path, &file)"));
        #[cfg(unix)]
        {
            assert!(production.contains("libc::openat"));
            assert!(production.contains("libc::O_NOFOLLOW"));
            assert!(production.contains("libc::O_DIRECTORY"));
        }
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

    #[cfg(unix)]
    #[test]
    fn recovery_snapshot_rejects_symlinked_lock_without_chmoding_target() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let home = temp_dir("lock_symlink_home");
        let cwd = temp_dir("lock_symlink_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let recovery_root = store.paths().project_dir.join("recovery");
        fs::create_dir_all(&recovery_root).unwrap();
        let redirected_lock = temp_dir("lock_symlink_redirect").join("external.lock");
        fs::write(&redirected_lock, b"external").unwrap();
        fs::set_permissions(&redirected_lock, fs::Permissions::from_mode(0o644)).unwrap();
        symlink(&redirected_lock, recovery_root.join("recovery.lock")).unwrap();

        let result = store.write_recovery_snapshot(
            "recovery-lock-symlink",
            &[RecoverySnapshotEntry {
                relative_path: PathBuf::from("src/lib.rs"),
                preimage: Some(b"secret".to_vec()),
                unix_mode: None,
                expected_postimage_sha256: None,
                created_parent_dirs: vec![],
            }],
        );

        assert!(result.is_err());
        assert_eq!(
            fs::metadata(&redirected_lock).unwrap().permissions().mode() & 0o777,
            0o644,
            "opening recovery.lock must not follow symlinks or chmod the target"
        );
    }

    #[test]
    fn recovery_snapshot_load_missing_store_is_read_only() {
        let home = temp_dir("load_missing_store_home");
        let cwd = temp_dir("load_missing_store_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let recovery_root = store.paths().project_dir.join("recovery");
        assert!(!recovery_root.exists());

        let result = store.load_recovery_snapshot(&RecoverySnapshotReference {
            snapshot_id: "missing-snapshot".to_string(),
            manifest_sha256: "0".repeat(64),
        });

        assert!(result.is_err());
        assert!(
            !recovery_root.exists(),
            "read-only load must not create recovery root, lock, or private dirs"
        );
    }

    #[cfg(unix)]
    #[test]
    fn recovery_snapshot_load_rejects_symlinked_blob_ancestor() {
        use std::os::unix::fs::symlink;

        let home = temp_dir("load_blob_ancestor_home");
        let cwd = temp_dir("load_blob_ancestor_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let reference = store
            .write_recovery_snapshot(
                "recovery-blob-ancestor-symlink",
                &[RecoverySnapshotEntry {
                    relative_path: PathBuf::from("src/lib.rs"),
                    preimage: Some(b"secret".to_vec()),
                    unix_mode: None,
                    expected_postimage_sha256: None,
                    created_parent_dirs: vec![],
                }],
            )
            .unwrap();
        let snapshot_dir = store
            .paths()
            .project_dir
            .join("recovery")
            .join(&reference.snapshot_id);
        let blob_root = snapshot_dir.join("blobs");
        let first_prefix = fs::read_dir(&blob_root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let redirected = temp_dir("load_blob_ancestor_redirect");
        fs::remove_dir_all(&first_prefix).unwrap();
        symlink(&redirected, &first_prefix).unwrap();

        let result = store.load_recovery_snapshot(&reference);

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_snapshot_load_rejects_symlinked_manifest_file() {
        use std::os::unix::fs::symlink;

        let home = temp_dir("load_manifest_symlink_home");
        let cwd = temp_dir("load_manifest_symlink_cwd");
        let store = WorkflowStore::new(&home, &cwd).unwrap();
        let reference = store
            .write_recovery_snapshot(
                "recovery-manifest-symlink",
                &[RecoverySnapshotEntry {
                    relative_path: PathBuf::from("src/lib.rs"),
                    preimage: Some(b"secret".to_vec()),
                    unix_mode: None,
                    expected_postimage_sha256: None,
                    created_parent_dirs: vec![],
                }],
            )
            .unwrap();
        let snapshot_dir = store
            .paths()
            .project_dir
            .join("recovery")
            .join(&reference.snapshot_id);
        let manifest = snapshot_dir.join("manifest.json");
        let redirected_manifest = temp_dir("load_manifest_symlink_redirect").join("manifest.json");
        fs::write(&redirected_manifest, b"{}").unwrap();
        fs::remove_file(&manifest).unwrap();
        symlink(&redirected_manifest, &manifest).unwrap();

        let result = store.load_recovery_snapshot(&reference);

        assert!(result.is_err());
    }
}
