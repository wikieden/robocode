use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use viden_core::{CoreClient, LocalCoreHost, WorkspaceOpenOverrides, WorkspaceOpenRequest};
use viden_session::SessionStore;
use viden_types::{Message, PermissionMode, Role, TranscriptEntry};

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("viden-core-host-{label}-{unique}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn missing_temp_path(label: &str) -> PathBuf {
    let base = temp_dir(label);
    base.join("missing-home")
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

#[test]
fn local_host_resumes_exact_session_without_returning_a_fresh_binding() {
    let home = temp_dir("resume-home");
    let project = temp_dir("resume-project");
    let session_id = seed_session(&home, &project, "session_exact_resume", "existing work");
    let host = LocalCoreHost::for_test(home);

    let binding = host
        .open_workspace(
            WorkspaceOpenRequest::new(project).with_resume_session_id(session_id.clone()),
        )
        .unwrap()
        .binding()
        .clone();

    assert_eq!(binding.session_id, session_id);
}

#[test]
fn local_host_rejects_missing_resume_without_returning_a_fresh_binding() {
    let home = temp_dir("missing-resume-home");
    let project = temp_dir("missing-resume-project");
    seed_session(&home, &project, "session_existing", "existing work");
    let host = LocalCoreHost::for_test(home);

    let error = match host.open_workspace(
        WorkspaceOpenRequest::new(project).with_resume_session_id("session_missing"),
    ) {
        Ok(_) => panic!("missing resume must not return a fresh binding"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("session_missing"));
    assert!(error.to_string().contains("not found"));
}

#[test]
fn local_host_rejects_ambiguous_resume_without_returning_a_fresh_binding() {
    let home = temp_dir("ambiguous-resume-home");
    let project = temp_dir("ambiguous-resume-project");
    seed_session(&home, &project, "session_ambiguous_a", "existing work a");
    seed_session(&home, &project, "session_ambiguous_b", "existing work b");
    let host = LocalCoreHost::for_test(home);

    let error = match host.open_workspace(
        WorkspaceOpenRequest::new(project).with_resume_session_id("session_ambiguous"),
    ) {
        Ok(_) => panic!("ambiguous resume must not return a fresh binding"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("session_ambiguous"));
    assert!(error.to_string().contains("ambiguous"));
}

#[test]
fn local_host_missing_resume_does_not_create_pristine_session_home() {
    let home = missing_temp_path("pristine-missing-resume-home");
    let project = temp_dir("pristine-missing-resume-project");
    let host = LocalCoreHost::for_test(home.clone());

    let error = match host.open_workspace(
        WorkspaceOpenRequest::new(project).with_resume_session_id("session_missing"),
    ) {
        Ok(_) => panic!("missing resume must not return a fresh binding"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("session_missing"));
    assert!(!home.exists(), "missing resume lookup must not create home");
}

#[test]
fn local_host_missing_resume_preserves_existing_empty_session_home() {
    let home = temp_dir("empty-missing-resume-home");
    let project = temp_dir("empty-missing-resume-project");
    let sentinel = home.join("sentinel.txt");
    std::fs::write(&sentinel, "unchanged").unwrap();
    let before = std::fs::metadata(&sentinel).unwrap();
    let host = LocalCoreHost::for_test(home.clone());

    let error = match host.open_workspace(
        WorkspaceOpenRequest::new(project).with_resume_session_id("session_missing"),
    ) {
        Ok(_) => panic!("missing resume must not return a fresh binding"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("session_missing"));
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "unchanged");
    assert_eq!(std::fs::metadata(&sentinel).unwrap().len(), before.len());
    #[cfg(unix)]
    assert_eq!(
        std::fs::metadata(&sentinel).unwrap().permissions().mode(),
        before.permissions().mode()
    );
    assert_eq!(
        std::fs::metadata(&sentinel)
            .unwrap()
            .permissions()
            .readonly(),
        before.permissions().readonly()
    );
    assert!(!home.join("projects").exists());
    assert!(!home.join("index.sqlite3").exists());
}

#[test]
fn workspace_open_request_debug_never_accepts_or_prints_raw_api_keys() {
    let request = WorkspaceOpenRequest::new(temp_dir("debug-project")).with_overrides(
        WorkspaceOpenOverrides {
            provider: Some("deepseek".to_string()),
            model: Some("deepseek-v4-flash".to_string()),
            permission_mode: Some(PermissionMode::Plan),
            ..WorkspaceOpenOverrides::default()
        },
    );

    let rendered = format!("{request:?}");
    assert!(!rendered.contains("sk-test-secret"));
    assert!(!rendered.contains("api_key"));

    let core_lib =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .unwrap();
    assert!(!core_lib.contains("pub use viden_config::CliOverrides"));
}

fn seed_session(home: &std::path::Path, project: &std::path::Path, id: &str, text: &str) -> String {
    let store =
        SessionStore::new_with_home(home, project.canonicalize().unwrap(), Some(id.to_string()))
            .unwrap();
    store
        .append_entry(&TranscriptEntry::Message {
            message: Message::new(Role::User, text.to_string()),
        })
        .unwrap();
    store.session_id().to_string()
}
