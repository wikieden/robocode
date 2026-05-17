use super::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod file_tests;
mod git_tests;
mod lsp_tests;
mod shell_tests;
mod web_tests;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "robocode_tools_{name}_{}",
        robocode_types::fresh_id("tmp")
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git_init(cwd: &PathBuf) {
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(init.success());
}

fn git_config_identity(cwd: &PathBuf) {
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(name.success());
}

fn git_add(cwd: &PathBuf, path: &str) {
    let add = Command::new("git")
        .args(["add", path])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(add.success());
}

fn git_commit(cwd: &PathBuf, message: &str) {
    let commit = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(commit.success());
}

fn git_repo_with_tracked_file(name: &str) -> PathBuf {
    let cwd = temp_dir(name);
    git_init(&cwd);
    git_config_identity(&cwd);
    fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
    git_add(&cwd, "tracked.txt");
    git_commit(&cwd, "initial");
    cwd
}
