use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use viden_core::{CoreClient, LocalCoreHost, WorkspaceOpenRequest};

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("viden-core-host-{label}-{unique}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn local_host_opens_two_workspaces_without_state_bleed() {
    let home = temp_dir("home");
    let project_a = temp_dir("project-a");
    let project_b = temp_dir("project-b");
    let host = LocalCoreHost::for_test(home);

    let mut a = host
        .open_workspace(WorkspaceOpenRequest::new(project_a.clone()))
        .unwrap();
    let mut b = host
        .open_workspace(WorkspaceOpenRequest::new(project_b.clone()))
        .unwrap();

    assert_ne!(a.binding().canonical_root, b.binding().canonical_root);
    assert_ne!(a.binding().session_id, b.binding().session_id);
    assert_ne!(a.binding().stream_id, b.binding().stream_id);
    assert_eq!(
        a.client().snapshot().unwrap().view.snapshot.cwd,
        project_a.canonicalize().unwrap()
    );
    assert_eq!(
        b.client().snapshot().unwrap().view.snapshot.cwd,
        project_b.canonicalize().unwrap()
    );
}

#[test]
fn local_host_rejects_missing_roots_and_files_before_bootstrap() {
    let home = temp_dir("reject-home");
    let project = temp_dir("reject-project");
    let file = project.join("not-a-directory.txt");
    std::fs::write(&file, "not a workspace").unwrap();
    let host = LocalCoreHost::for_test(home);

    assert!(
        host.open_workspace(WorkspaceOpenRequest::new(project.join("missing")))
            .is_err()
    );
    assert!(
        host.open_workspace(WorkspaceOpenRequest::new(file))
            .is_err()
    );
}
