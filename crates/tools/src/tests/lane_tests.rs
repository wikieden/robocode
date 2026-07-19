use std::fs;
use std::thread;
use std::time::Duration;

use crate::lane::{
    LaneEffectError, LocalLaneEffects, WorktreeBackend, WorktreeCreateRequest,
    WorktreeRemoveRequest,
};
use crate::patch::{LocalPatchBackend, PatchBackend, PatchRequest};
use crate::process::{
    FakeTerminalBackend, LocalProcessBackend, ProcessBackend, SpawnProcess, SpawnTerminal,
    TerminalBackend, TerminalKind,
};

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
        .spawn(&SpawnTerminal {
            kind: TerminalKind::Tmux,
            session_name: Some("viden-lane-a".into()),
            command: "worker".into(),
            args: vec!["--lane".into(), "lane-a".into()],
            cwd: temp_dir("lane_fake_terminal"),
            env: vec![("VIDEN_LANE".into(), "lane-a".into())],
            output_log: temp_dir("lane_fake_terminal_log").join("lane.log"),
        })
        .unwrap();

    terminal.send(&handle, b"hello\n").unwrap();
    terminal.stop(&handle).unwrap();

    let sessions = terminal.sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].handle, handle);
    assert_eq!(sessions[0].request.kind, TerminalKind::Tmux);
    assert_eq!(
        sessions[0].request.session_name.as_deref(),
        Some("viden-lane-a")
    );
    assert_eq!(sessions[0].inputs, vec![b"hello\n".to_vec()]);
    assert!(sessions[0].stopped);
}

#[test]
fn lane_fake_terminal_preserves_typed_pty_route() {
    let terminal = FakeTerminalBackend::default();
    let handle = terminal
        .spawn(&SpawnTerminal {
            kind: TerminalKind::Pty,
            session_name: None,
            command: "worker".into(),
            args: Vec::new(),
            cwd: temp_dir("lane_fake_pty"),
            env: Vec::new(),
            output_log: temp_dir("lane_fake_pty_log").join("lane.log"),
        })
        .unwrap();

    assert_eq!(handle.kind, TerminalKind::Pty);
    assert_eq!(terminal.sessions()[0].request.kind, TerminalKind::Pty);
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
            output_log: None,
        })
        .unwrap();

    backend.stop(&handle).unwrap();
    let err = backend.stop(&handle).unwrap_err();
    assert!(err.to_string().contains("unknown process handle"));
}

#[test]
fn lane_local_process_forwards_stdin_before_stop() {
    let cwd = temp_dir("lane_local_process_send");
    let backend = LocalProcessBackend::default();
    let handle = backend
        .spawn(&SpawnProcess {
            command: "sh".into(),
            args: vec![
                "-c".into(),
                "read line; printf '%s' \"$line\" > received.txt; sleep 30".into(),
            ],
            cwd: cwd.clone(),
            env: Vec::new(),
            output_log: None,
        })
        .unwrap();

    backend.send(&handle, b"hello lane\n").unwrap();
    for _ in 0..100 {
        if cwd.join("received.txt").exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        fs::read_to_string(cwd.join("received.txt")).unwrap(),
        "hello lane"
    );
    backend.stop(&handle).unwrap();
}

#[test]
fn lane_local_process_drains_large_stdout_and_stderr_to_log() {
    let cwd = temp_dir("lane_local_process_output_log");
    let output_log = cwd.join("process.log");
    let backend = LocalProcessBackend::default();
    let handle = backend
        .spawn(&SpawnProcess {
            command: "sh".into(),
            args: vec![
                "-c".into(),
                "dd if=/dev/zero bs=65536 count=4 2>/dev/null; dd if=/dev/zero bs=65536 count=4 2>&1 >/dev/stderr; printf done > completed.txt".into(),
            ],
            cwd: cwd.clone(),
            env: Vec::new(),
            output_log: Some(output_log.clone()),
        })
        .unwrap();

    for _ in 0..500 {
        if matches!(
            fs::read_to_string(cwd.join("completed.txt")),
            Ok(contents) if contents == "done"
        ) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        fs::read_to_string(cwd.join("completed.txt")).unwrap(),
        "done"
    );
    assert!(fs::metadata(output_log).unwrap().len() >= 512 * 1024);
    backend.stop(&handle).unwrap();
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
fn lane_patch_applies_standard_new_file_diff() {
    let cwd = temp_dir("lane_patch_create_file");
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+first\n+second\n"
            .into(),
    };

    let outcome = patch.apply(&request).unwrap();

    assert!(outcome.applied);
    assert_eq!(
        fs::read_to_string(cwd.join("new.txt")).unwrap(),
        "first\nsecond\n"
    );
}

#[test]
fn lane_patch_applies_standard_deleted_file_diff() {
    let cwd = temp_dir("lane_patch_delete_file");
    fs::write(cwd.join("old.txt"), "first\nsecond\n").unwrap();
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-first\n-second\n"
            .into(),
    };

    let outcome = patch.apply(&request).unwrap();

    assert!(outcome.applied);
    assert!(!cwd.join("old.txt").exists());
}

#[test]
fn lane_patch_new_file_conflicts_when_target_exists() {
    let cwd = temp_dir("lane_patch_create_conflict");
    fs::write(cwd.join("new.txt"), "keep\n").unwrap();
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1 @@\n+created\n"
            .into(),
    };

    let outcome = patch.apply(&request).unwrap();

    assert!(!outcome.applied);
    assert!(outcome.conflicts[0].message.contains("already exists"));
    assert_eq!(fs::read_to_string(cwd.join("new.txt")).unwrap(), "keep\n");
}

#[test]
fn lane_patch_deleted_file_conflicts_when_content_changed() {
    let cwd = temp_dir("lane_patch_delete_conflict");
    fs::write(cwd.join("old.txt"), "changed\n").unwrap();
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-original\n"
            .into(),
    };

    let outcome = patch.apply(&request).unwrap();

    assert!(!outcome.applied);
    assert_eq!(
        fs::read_to_string(cwd.join("old.txt")).unwrap(),
        "changed\n"
    );
}

#[test]
fn lane_patch_new_file_rolls_back_after_persistence_failure() {
    let cwd = temp_dir("lane_patch_create_rollback");
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1 @@\n+created\n"
            .into(),
    };

    let err = patch
        .apply_transactionally(&request, || Err("injected create failure".into()))
        .unwrap_err();

    assert!(err.to_string().contains("injected create failure"));
    assert!(!cwd.join("new.txt").exists());
}

#[test]
fn lane_patch_deleted_file_rolls_back_after_persistence_failure() {
    let cwd = temp_dir("lane_patch_delete_rollback");
    fs::write(cwd.join("old.txt"), "restore me\n").unwrap();
    let patch = LocalPatchBackend;
    let request = PatchRequest {
        cwd: cwd.clone(),
        unified_diff: "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-restore me\n"
            .into(),
    };

    let err = patch
        .apply_transactionally(&request, || Err("injected delete failure".into()))
        .unwrap_err();

    assert!(err.to_string().contains("injected delete failure"));
    assert_eq!(
        fs::read_to_string(cwd.join("old.txt")).unwrap(),
        "restore me\n"
    );
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
