use std::cell::Cell;
use std::path::Path;

use crate::{EngineEvent, SessionEngine};
use robocode_types::ApprovalResponse;

use super::{SequenceProvider, temp_dir};

fn init_git_repo(cwd: &Path) {
    let init = std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(init.success());
}

fn configure_git_identity(cwd: &Path) {
    let email = std::process::Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = std::process::Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(name.success());
}

fn commit_demo_file(cwd: &Path) {
    configure_git_identity(cwd);
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();
    let add = std::process::Command::new("git")
        .args(["add", "demo.txt"])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(commit.success());
}

#[test]
fn git_status_command_uses_tool_runtime() {
    let home = temp_dir("git_status_home");
    let cwd = temp_dir("git_status_cwd");
    init_git_repo(&cwd);
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git status", &mut approver)
        .unwrap();
    assert_eq!(approvals, 0);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::Command(text) if text.contains("demo.txt")))
    );
}

#[test]
fn git_switch_requests_approval() {
    let home = temp_dir("git_switch_home");
    let cwd = temp_dir("git_switch_cwd");
    init_git_repo(&cwd);

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git switch feature/demo --create", &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("Switched") || text.contains("feature/demo"))
    ));
}

#[test]
fn git_add_requests_approval_and_stages_file() {
    let home = temp_dir("git_add_home");
    let cwd = temp_dir("git_add_cwd");
    init_git_repo(&cwd);
    std::fs::write(cwd.join("demo.txt"), "hello\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git add demo.txt", &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("git add") || text.contains("demo.txt"))
    ));

    let output = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("A  demo.txt"));
}

#[test]
fn git_restore_requests_approval_and_reverts_file() {
    let home = temp_dir("git_restore_home");
    let cwd = temp_dir("git_restore_cwd");
    init_git_repo(&cwd);
    commit_demo_file(&cwd);
    std::fs::write(cwd.join("demo.txt"), "changed\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let events = engine
        .process_input_with_approval("/git restore demo.txt", &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(
        |event| matches!(event, EngineEvent::Command(text) if text.contains("restore") || text.contains("demo.txt"))
    ));
    let contents = std::fs::read_to_string(cwd.join("demo.txt")).unwrap();
    assert_eq!(contents, "hello\n");
}

#[test]
fn git_stash_push_requests_approval_and_list_is_visible() {
    let home = temp_dir("git_stash_home");
    let cwd = temp_dir("git_stash_cwd");
    init_git_repo(&cwd);
    commit_demo_file(&cwd);
    std::fs::write(cwd.join("demo.txt"), "changed\n").unwrap();

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let approvals = Cell::new(0usize);
    let mut approver = |_prompt| {
        approvals.set(approvals.get() + 1);
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    engine
        .process_input_with_approval("/git stash push -m save-work", &mut approver)
        .unwrap();
    assert_eq!(approvals.get(), 1);
    let list_output = engine
        .process_input_with_approval("/git stash list", &mut approver)
        .unwrap();
    assert!(
        list_output
            .iter()
            .any(|event| matches!(event, EngineEvent::Command(text) if text.contains("save-work")))
    );
}

#[test]
fn git_worktree_add_requests_approval_and_creates_checkout() {
    let home = temp_dir("git_worktree_home");
    let cwd = temp_dir("git_worktree_cwd");
    init_git_repo(&cwd);
    commit_demo_file(&cwd);

    let worktree = cwd
        .parent()
        .unwrap()
        .join("robocode_core_worktree_checkout");
    if worktree.exists() {
        std::fs::remove_dir_all(&worktree).unwrap();
    }

    let provider = Box::new(SequenceProvider::new(vec![]));
    let mut engine = SessionEngine::new_with_home(&cwd, provider, Some(home)).unwrap();
    let mut approvals = 0usize;
    let mut approver = |_prompt| {
        approvals += 1;
        ApprovalResponse {
            approved: true,
            feedback: None,
        }
    };
    let command = format!(
        "/git worktree add {} feature/worktree --create",
        worktree.to_string_lossy()
    );
    let events = engine
        .process_input_with_approval(&command, &mut approver)
        .unwrap();
    assert_eq!(approvals, 1);
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Command(text)
            if text.contains("Preparing worktree")
                || text.contains("feature/worktree")
                || text.contains("HEAD is now at")
    )));
    assert!(worktree.exists());
}
