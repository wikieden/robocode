use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use viden_core::{
    CoreClient, LocalCoreHost, SecretBytes, WorkspaceOpenOverrides, WorkspaceOpenRequest,
};
use viden_session::SessionStore;
use viden_types::{
    FRONTEND_SCHEMA_V1, Message, PermissionMode, Role, RuntimeCommand, RuntimeCommandEnvelope,
    RuntimeOwner, TranscriptEntry,
};

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

#[test]
fn staged_secret_is_one_use_bound_to_workspace_and_absent_from_serialized_surfaces() {
    let home = temp_dir("credential-home");
    let project_a = temp_dir("credential-project-a");
    let project_b = temp_dir("credential-project-b");
    let host = LocalCoreHost::for_test(home.clone());
    let overrides = WorkspaceOpenOverrides {
        permission_mode: Some(PermissionMode::DontAsk),
        ..WorkspaceOpenOverrides::default()
    };
    let mut a = host
        .open_workspace(WorkspaceOpenRequest::new(project_a).with_overrides(overrides.clone()))
        .unwrap();
    let mut b = host
        .open_workspace(WorkspaceOpenRequest::new(project_b).with_overrides(overrides))
        .unwrap();
    let request = a
        .stage_credential(
            "sequence",
            "test-keychain:item-1",
            SecretBytes::new(b"sk-test".to_vec()),
        )
        .unwrap();

    assert!(!format!("{request:?}").contains("sk-test"));
    assert!(!serde_json::to_string(&request).unwrap().contains("sk-test"));

    let cross_workspace = store_handle_command(
        "cross-workspace",
        "sequence",
        "test-keychain:item-1",
        request.id().to_string(),
    );
    b.client().send(cross_workspace).unwrap();
    let rejected = snapshot_until_rejected(b.client(), "cross-workspace");
    assert!(rejected.contains("credential request"));
    assert!(!rejected.contains("sk-test"));

    let command = store_handle_command(
        "store-once",
        "sequence",
        "test-keychain:item-1",
        request.id().to_string(),
    );
    assert!(!serde_json::to_string(&command).unwrap().contains("sk-test"));
    a.client().send(command).unwrap();
    snapshot_until_credential_stored(a.client(), "sequence", "test-keychain:item-1");

    let replay = store_handle_command(
        "store-replay",
        "sequence",
        "test-keychain:item-1",
        request.id().to_string(),
    );
    a.client().send(replay).unwrap();
    let replay_error = snapshot_until_rejected(a.client(), "store-replay");
    assert!(replay_error.contains("credential request"));
    assert!(!replay_error.contains("sk-test"));
    assert!(!read_all_jsonl(&home).contains("sk-test"));
}

#[test]
fn staged_secret_expiry_capacity_sink_failure_and_concurrency_are_fail_closed() {
    let home = temp_dir("credential-policy-home");
    let project = temp_dir("credential-policy-project");
    let host = LocalCoreHost::for_test(home.clone())
        .with_credential_capacity_for_test(2)
        .with_credential_clock_for_test(100);
    let mut client = host
        .open_workspace(
            WorkspaceOpenRequest::new(project).with_overrides(WorkspaceOpenOverrides {
                permission_mode: Some(PermissionMode::DontAsk),
                ..WorkspaceOpenOverrides::default()
            }),
        )
        .unwrap();

    let expired = client
        .stage_credential(
            "sequence",
            "test-keychain:item-1",
            SecretBytes::new(b"expired".to_vec()),
        )
        .unwrap();
    host.set_credential_clock_for_test(401);
    client
        .client()
        .send(store_handle_command(
            "expired",
            "sequence",
            "test-keychain:item-1",
            expired.id().to_string(),
        ))
        .unwrap();
    assert!(snapshot_until_rejected(client.client(), "expired").contains("expired"));

    let first = client
        .stage_credential(
            "sequence",
            "test-keychain:item-1",
            SecretBytes::new(b"one".to_vec()),
        )
        .unwrap();
    let second = client
        .stage_credential(
            "sequence",
            "test-keychain:item-2",
            SecretBytes::new(b"two".to_vec()),
        )
        .unwrap();
    assert!(
        client
            .stage_credential(
                "sequence",
                "test-keychain:item-3",
                SecretBytes::new(b"three".to_vec())
            )
            .is_err()
    );

    client
        .client()
        .send(store_handle_command(
            "mismatch",
            "sequence",
            "test-keychain:item-2",
            first.id().to_string(),
        ))
        .unwrap();
    assert!(snapshot_until_rejected(client.client(), "mismatch").contains("credential request"));

    host.fail_next_credential_sink_for_test("platform sink unavailable");
    client
        .client()
        .send(store_handle_command(
            "sink-failure",
            "sequence",
            "test-keychain:item-2",
            second.id().to_string(),
        ))
        .unwrap();
    assert!(
        snapshot_until_rejected(client.client(), "sink-failure")
            .contains("platform sink unavailable")
    );
    client
        .client()
        .send(store_handle_command(
            "sink-replay",
            "sequence",
            "test-keychain:item-2",
            second.id().to_string(),
        ))
        .unwrap();
    assert!(snapshot_until_rejected(client.client(), "sink-replay").contains("credential request"));

    let concurrent = client
        .stage_credential(
            "sequence",
            "test-keychain:item-4",
            SecretBytes::new(b"concurrent".to_vec()),
        )
        .unwrap();
    let id = concurrent.id().to_string();
    let binding = client.binding().clone();
    let success_count = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            host.consume_staged_credential_for_test(
                &binding,
                "sequence",
                "test-keychain:item-4",
                &id,
            )
            .is_ok()
        });
        let second = scope.spawn(|| {
            host.consume_staged_credential_for_test(
                &binding,
                "sequence",
                "test-keychain:item-4",
                &id,
            )
            .is_ok()
        });
        usize::from(first.join().unwrap()) + usize::from(second.join().unwrap())
    });
    assert_eq!(success_count, 1);
    assert!(!read_all_jsonl(&home).contains("concurrent"));
}

#[test]
fn secret_bytes_zeroizes_on_drop_and_has_no_serialized_trait_derives() {
    let observed = SecretBytes::drop_probe_for_test(b"sk-zeroize".to_vec());
    assert!(observed.iter().all(|byte| *byte == 0));

    let core_host =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/host.rs"))
            .unwrap();
    for trait_name in ["Clone", "Debug", "serde::Serialize", "serde::Deserialize"] {
        assert!(!core_host.contains(&format!("SecretBytes, {trait_name}")));
    }
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

fn store_handle_command(
    command_id: &str,
    provider_id: &str,
    backend_id: &str,
    credential_request_id: String,
) -> RuntimeCommandEnvelope {
    RuntimeCommandEnvelope {
        schema_version: FRONTEND_SCHEMA_V1,
        client_id: "frontend-host-test".to_string(),
        command_id: command_id.to_string(),
        owner: RuntimeOwner::default(),
        command: RuntimeCommand::StoreCredentialHandle {
            provider_id: provider_id.to_string(),
            backend_id: backend_id.to_string(),
            credential_request_id,
        },
    }
}

fn snapshot_until_rejected(client: &mut impl CoreClient, command_id: &str) -> String {
    let mut seen = Vec::new();
    for _ in 0..16 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let snapshot = client.snapshot().expect("snapshot");
        seen.extend(
            snapshot
                .view
                .errors
                .iter()
                .map(|error| error.message.clone()),
        );
        if let Some(error) = snapshot.view.errors.iter().find(|error| {
            error
                .message
                .contains(&format!("command {command_id} rejected:"))
        }) {
            return error.message.clone();
        }
    }
    panic!("missing rejection for {command_id}; seen {seen:?}");
}

fn snapshot_until_credential_stored(
    client: &mut impl CoreClient,
    provider_id: &str,
    backend_id: &str,
) {
    for _ in 0..16 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let snapshot = client.snapshot().expect("snapshot");
        if snapshot
            .view
            .credential_handles
            .iter()
            .any(|handle| handle.provider_id == provider_id && handle.backend_id == backend_id)
        {
            return;
        }
    }
    panic!("missing stored credential handle for {provider_id}/{backend_id}");
}

fn read_all_jsonl(root: &std::path::Path) -> String {
    fn visit(path: &std::path::Path, out: &mut String) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, out);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
                && let Ok(contents) = std::fs::read_to_string(&path)
            {
                out.push_str(&contents);
            }
        }
    }
    let mut out = String::new();
    visit(root, &mut out);
    out
}
