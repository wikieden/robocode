use std::fs;

use crate::lane::{
    LaneEffectError, LocalLaneEffects, WorktreeBackend, WorktreeCreateRequest,
    WorktreeRemoveRequest,
};
use crate::patch::{LocalPatchBackend, PatchBackend, PatchRequest};
use crate::process::{FakeTerminalBackend, LocalProcessBackend, ProcessBackend, SpawnProcess};

use super::temp_dir;

#[test]
fn lane_worktree_rejects_path_traversal_before_effects() {
    let cwd = temp_dir("lane_worktree_path_traversal");
    let effects = LocalLaneEffects;
    let escape = format!("../{}_escape", cwd.file_name().unwrap().to_string_lossy());

    let err = effects
        .create_worktree(&WorktreeCreateRequest {
            repo: cwd.clone(),
            path: escape.clone(),
            branch: Some("feature/escape".into()),
            create_branch: true,
        })
        .unwrap_err();

    assert!(matches!(err, LaneEffectError::UnsafePath { .. }));
    assert!(
        !cwd.parent()
            .unwrap()
            .join(escape.trim_start_matches("../"))
            .exists()
    );
}

#[test]
fn lane_fake_terminal_records_send_and_stop_without_process_side_effects() {
    let terminal = FakeTerminalBackend::default();
    let handle = terminal
        .spawn(&SpawnProcess {
            command: "worker".into(),
            args: vec!["--lane".into(), "lane-a".into()],
            cwd: temp_dir("lane_fake_terminal"),
            env: vec![("VIDEN_LANE".into(), "lane-a".into())],
        })
        .unwrap();

    terminal.send(&handle, b"hello\n").unwrap();
    terminal.stop(&handle).unwrap();

    let sessions = terminal.sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].handle, handle);
    assert_eq!(sessions[0].inputs, vec![b"hello\n".to_vec()]);
    assert!(sessions[0].stopped);
}

#[test]
fn lane_local_process_stop_cancels_spawned_child() {
    let backend = LocalProcessBackend::default();
    let handle = backend
        .spawn(&SpawnProcess {
            command: "sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            cwd: temp_dir("lane_local_process_stop"),
            env: Vec::new(),
        })
        .unwrap();

    backend.stop(&handle).unwrap();
    let err = backend.stop(&handle).unwrap_err();
    assert!(err.to_string().contains("unknown process handle"));
}

#[test]
fn lane_patch_check_reports_conflict_without_partial_write() {
    let cwd = temp_dir("lane_patch_conflict_no_partial_write");
    fs::write(cwd.join("tracked.txt"), "old\n").unwrap();
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/tracked.txt b/tracked.txt\n--- a/tracked.txt\n+++ b/tracked.txt\n@@ -1 +1 @@\n-missing\n+new\n"
            .into(),
    };

    let outcome = patch.check(&request).unwrap();
    assert!(outcome.conflicts.iter().any(|conflict| {
        conflict
            .message
            .contains("expected hunk context was not found")
    }));
    assert_eq!(
        fs::read_to_string(cwd.join("tracked.txt")).unwrap(),
        "old\n"
    );

    let apply = patch.apply(&request).unwrap();
    assert!(!apply.applied);
    assert_eq!(
        fs::read_to_string(cwd.join("tracked.txt")).unwrap(),
        "old\n"
    );
}

#[test]
fn lane_patch_apply_rolls_back_after_injected_persistence_failure() {
    let cwd = temp_dir("lane_patch_transaction_rollback");
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/lib.rs"), "old\n").unwrap();
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n"
            .into(),
    };

    let err = patch
        .apply_transactionally(&request, || Err("injected persistence failure".into()))
        .unwrap_err();

    assert!(err.to_string().contains("injected persistence failure"));
    assert_eq!(fs::read_to_string(cwd.join("src/lib.rs")).unwrap(), "old\n");
}

#[test]
fn lane_worktree_remove_rejects_path_traversal_before_effects() {
    let cwd = temp_dir("lane_worktree_remove_path_traversal");
    let effects = LocalLaneEffects;

    let err = effects
        .remove_worktree(&WorktreeRemoveRequest {
            repo: cwd,
            path: "../escape".into(),
            force: false,
        })
        .unwrap_err();

    assert!(matches!(err, LaneEffectError::UnsafePath { .. }));
}
