use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::lane::LaneEffectError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchRequest {
    pub cwd: PathBuf,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchConflictReport {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchApplyOutcome {
    pub applied: bool,
    pub writes: Vec<PathBuf>,
    pub conflicts: Vec<PatchConflictReport>,
}

pub trait PatchBackend: Send + Sync {
    fn check(&self, request: &PatchRequest) -> Result<PatchApplyOutcome, LaneEffectError>;
    fn apply(&self, request: &PatchRequest) -> Result<PatchApplyOutcome, LaneEffectError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalPatchBackend;

#[derive(Debug, Clone)]
pub struct PatchApplication {
    writes: Vec<(PathBuf, String)>,
}

impl PatchApplication {
    pub fn write_paths(&self) -> impl Iterator<Item = &Path> {
        self.writes.iter().map(|(path, _)| path.as_path())
    }
}

impl LocalPatchBackend {
    pub fn prepare(&self, request: &PatchRequest) -> Result<PatchApplication, LaneEffectError> {
        let patch_files = parse_unified_diff(&request.unified_diff)?;
        if patch_files.is_empty() {
            return Err(LaneEffectError::PatchConflict(
                "no unified diff patch found".to_string(),
            ));
        }

        // Prepare every target byte buffer before writing any file. Runtime
        // transactions can then stage rollback once and keep conflict handling
        // free of partial writes.
        let mut writes = Vec::new();
        for patch_file in patch_files {
            let relative_path = validate_patch_path(&patch_file.path)?;
            let full_path = request.cwd.join(&relative_path);
            let current = fs::read_to_string(&full_path).map_err(|err| {
                LaneEffectError::Io(format!("{}: {err}", relative_path.display()))
            })?;
            let updated = apply_patch_file(&current, &patch_file).map_err(|err| {
                LaneEffectError::PatchConflict(format!("{}: {err}", relative_path.display()))
            })?;
            writes.push((full_path, updated));
        }

        Ok(PatchApplication { writes })
    }

    pub fn write_application(
        &self,
        application: &PatchApplication,
    ) -> Result<PatchApplyOutcome, LaneEffectError> {
        let rollback = stage_rollback(application)?;
        if let Err(err) = write_patch_application(application) {
            restore_rollback(&rollback)?;
            return Err(err);
        }
        Ok(PatchApplyOutcome {
            applied: true,
            writes: application.write_paths().map(Path::to_path_buf).collect(),
            conflicts: Vec::new(),
        })
    }

    pub fn apply_transactionally<F>(
        &self,
        request: &PatchRequest,
        persist: F,
    ) -> Result<PatchApplyOutcome, LaneEffectError>
    where
        F: FnOnce() -> Result<(), String>,
    {
        let application = match self.prepare(request) {
            Ok(application) => application,
            Err(LaneEffectError::PatchConflict(err)) => {
                return Ok(conflict_outcome(err));
            }
            Err(err) => return Err(err),
        };
        let rollback = stage_rollback(&application)?;
        if let Err(err) = write_patch_application(&application) {
            restore_rollback(&rollback)?;
            return Err(err);
        }
        if let Err(err) = persist() {
            restore_rollback(&rollback)?;
            return Err(LaneEffectError::Io(err));
        }
        Ok(PatchApplyOutcome {
            applied: true,
            writes: application.write_paths().map(Path::to_path_buf).collect(),
            conflicts: Vec::new(),
        })
    }
}

impl PatchBackend for LocalPatchBackend {
    fn check(&self, request: &PatchRequest) -> Result<PatchApplyOutcome, LaneEffectError> {
        match self.prepare(request) {
            Ok(application) => Ok(PatchApplyOutcome {
                applied: false,
                writes: application.write_paths().map(Path::to_path_buf).collect(),
                conflicts: Vec::new(),
            }),
            Err(LaneEffectError::PatchConflict(err)) => Ok(conflict_outcome(err)),
            Err(err) => Err(err),
        }
    }

    fn apply(&self, request: &PatchRequest) -> Result<PatchApplyOutcome, LaneEffectError> {
        match self.prepare(request) {
            Ok(application) => self.write_application(&application),
            Err(LaneEffectError::PatchConflict(err)) => Ok(conflict_outcome(err)),
            Err(err) => Err(err),
        }
    }
}

fn conflict_outcome(message: String) -> PatchApplyOutcome {
    PatchApplyOutcome {
        applied: false,
        writes: Vec::new(),
        conflicts: vec![PatchConflictReport {
            path: PathBuf::new(),
            message,
        }],
    }
}

#[derive(Debug)]
struct PatchFile {
    path: String,
    hunks: Vec<PatchHunk>,
}

#[derive(Debug)]
struct PatchHunk {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

fn write_patch_application(application: &PatchApplication) -> Result<(), LaneEffectError> {
    for (path, contents) in &application.writes {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| LaneEffectError::Io(format!("{}: {err}", parent.display())))?;
        }
        fs::write(path, contents)
            .map_err(|err| LaneEffectError::Io(format!("{}: {err}", path.display())))?;
    }
    Ok(())
}

#[derive(Debug)]
struct FileRollback {
    path: PathBuf,
    contents: Option<Vec<u8>>,
    permissions: Option<fs::Permissions>,
}

fn stage_rollback(application: &PatchApplication) -> Result<Vec<FileRollback>, LaneEffectError> {
    let mut rollback = Vec::new();
    for path in application.write_paths() {
        let metadata = fs::metadata(path).ok();
        rollback.push(FileRollback {
            path: path.to_path_buf(),
            contents: fs::read(path).ok(),
            permissions: metadata.map(|metadata| metadata.permissions()),
        });
    }
    Ok(rollback)
}

fn restore_rollback(files: &[FileRollback]) -> Result<(), LaneEffectError> {
    for file in files.iter().rev() {
        match &file.contents {
            Some(contents) => {
                if let Some(parent) = file.path.parent() {
                    fs::create_dir_all(parent).map_err(|err| {
                        LaneEffectError::Io(format!("{}: {err}", parent.display()))
                    })?;
                }
                fs::write(&file.path, contents).map_err(|err| {
                    LaneEffectError::Io(format!("{}: {err}", file.path.display()))
                })?;
                if let Some(permissions) = &file.permissions {
                    fs::set_permissions(&file.path, permissions.clone()).map_err(|err| {
                        LaneEffectError::Io(format!("{}: {err}", file.path.display()))
                    })?;
                }
            }
            None => {
                if file.path.exists() {
                    fs::remove_file(&file.path).map_err(|err| {
                        LaneEffectError::Io(format!("{}: {err}", file.path.display()))
                    })?;
                }
            }
        }
    }
    Ok(())
}

fn parse_unified_diff(diff: &str) -> Result<Vec<PatchFile>, LaneEffectError> {
    let mut files = Vec::new();
    let mut current_file: Option<PatchFile> = None;
    let mut current_hunk: Option<PatchHunk> = None;

    for raw_line in diff.split_inclusive('\n') {
        let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("diff --git ") {
            finish_patch_hunk(&mut current_file, &mut current_hunk);
            if let Some(file) = current_file.take() {
                files.push(file);
            }
            let path = rest
                .split_whitespace()
                .find_map(|part| part.strip_prefix("b/"))
                .or_else(|| rest.split_whitespace().nth(1))
                .ok_or_else(|| {
                    LaneEffectError::PatchConflict(format!("invalid diff header `{line}`"))
                })?;
            current_file = Some(PatchFile {
                path: path.to_string(),
                hunks: Vec::new(),
            });
            continue;
        }

        if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(file) = current_file.as_mut()
                && let Some(path) = path.trim().strip_prefix("b/")
            {
                file.path = path.to_string();
            }
            continue;
        }

        if line.starts_with("@@") {
            let Some(file) = current_file.as_mut() else {
                return Err(LaneEffectError::PatchConflict(
                    "hunk appeared before file header".to_string(),
                ));
            };
            if let Some(hunk) = current_hunk.take() {
                file.hunks.push(hunk);
            }
            current_hunk = Some(PatchHunk {
                old_lines: Vec::new(),
                new_lines: Vec::new(),
            });
            continue;
        }

        let Some(hunk) = current_hunk.as_mut() else {
            continue;
        };
        if line.starts_with('\\') {
            continue;
        }
        let Some((prefix, content)) = split_patch_line(raw_line) else {
            continue;
        };
        match prefix {
            ' ' => {
                hunk.old_lines.push(content.clone());
                hunk.new_lines.push(content);
            }
            '-' => hunk.old_lines.push(content),
            '+' => hunk.new_lines.push(content),
            _ => {}
        }
    }

    finish_patch_hunk(&mut current_file, &mut current_hunk);
    if let Some(file) = current_file {
        files.push(file);
    }
    files.retain(|file| !file.hunks.is_empty());
    Ok(files)
}

fn finish_patch_hunk(file: &mut Option<PatchFile>, hunk: &mut Option<PatchHunk>) {
    if let (Some(file), Some(hunk)) = (file.as_mut(), hunk.take()) {
        file.hunks.push(hunk);
    }
}

fn split_patch_line(raw_line: &str) -> Option<(char, String)> {
    let prefix = raw_line.chars().next()?;
    if !matches!(prefix, ' ' | '-' | '+') {
        return None;
    }
    Some((prefix, raw_line[prefix.len_utf8()..].to_string()))
}

fn apply_patch_file(current: &str, patch_file: &PatchFile) -> Result<String, String> {
    let mut lines = split_preserving_newlines(current);
    let mut cursor = 0usize;
    for hunk in &patch_file.hunks {
        let Some(index) = find_line_sequence(&lines, &hunk.old_lines, cursor) else {
            return Err("patch conflict: expected hunk context was not found".to_string());
        };
        lines.splice(index..index + hunk.old_lines.len(), hunk.new_lines.clone());
        cursor = index + hunk.new_lines.len();
    }
    Ok(lines.concat())
}

fn split_preserving_newlines(input: &str) -> Vec<String> {
    if input.is_empty() {
        Vec::new()
    } else {
        input
            .split_inclusive('\n')
            .map(ToString::to_string)
            .collect()
    }
}

fn find_line_sequence(lines: &[String], needle: &[String], start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(lines.len()));
    }
    if needle.len() > lines.len() {
        return None;
    }
    (start..=lines.len() - needle.len())
        .find(|&index| lines[index..index + needle.len()] == *needle)
}

fn validate_patch_path(path: &str) -> Result<PathBuf, LaneEffectError> {
    let normalized = path
        .trim()
        .trim_start_matches("a/")
        .trim_start_matches("b/");
    if normalized.is_empty() || normalized == "/dev/null" {
        return Err(LaneEffectError::PatchConflict(
            "patch path is empty or unsupported".to_string(),
        ));
    }
    let candidate = Path::new(normalized);
    if candidate.is_absolute() {
        return Err(LaneEffectError::UnsafePath {
            path: normalized.to_string(),
        });
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(LaneEffectError::UnsafePath {
            path: normalized.to_string(),
        });
    }
    Ok(candidate.to_path_buf())
}
