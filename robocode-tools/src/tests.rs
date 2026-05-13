use super::*;
use std::fs;
use std::process::Command;
use std::sync::Arc;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "robocode_tools_{name}_{}",
        robocode_types::fresh_id("tmp")
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Debug)]
struct MockSemanticProvider;

impl SemanticToolProvider for MockSemanticProvider {
    fn diagnostics(&self, _cwd: &Path, path: &Path) -> Result<String, String> {
        Ok(format!("diagnostics for {}", path.display()))
    }

    fn symbols(&self, _cwd: &Path, path: &Path) -> Result<String, String> {
        Ok(format!("symbols for {}", path.display()))
    }

    fn references(
        &self,
        _cwd: &Path,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<String, String> {
        Ok(format!(
            "references for {}:{line}:{character}",
            path.display()
        ))
    }
}

#[test]
fn lsp_tool_specs_are_read_only() {
    let registry = ToolRegistry::builtin();
    for name in ["lsp_diagnostics", "lsp_symbols", "lsp_references"] {
        let spec = registry.spec(name).unwrap();
        assert!(!spec.is_mutating, "{name} must be read-only");
    }
}

#[test]
fn lsp_diagnostics_requires_semantic_provider() {
    let ctx = ToolExecutionContext {
        cwd: temp_dir("lsp_missing_provider"),
        semantic: None,
    };
    let mut input = ToolInput::new();
    input.insert("path".into(), "src/lib.rs".into());

    let error = ToolRegistry::builtin()
        .execute(
            &ToolCall {
                id: "tool_lsp_diagnostics".into(),
                name: "lsp_diagnostics".into(),
                input,
            },
            &ctx,
        )
        .unwrap_err();
    assert_eq!(error, "LSP semantic provider is not available");
}

#[test]
fn lsp_references_validates_line_and_character() {
    let ctx = ToolExecutionContext {
        cwd: temp_dir("lsp_bad_position"),
        semantic: Some(Arc::new(MockSemanticProvider)),
    };
    let mut input = ToolInput::new();
    input.insert("path".into(), "src/lib.rs".into());
    input.insert("line".into(), "abc".into());
    input.insert("character".into(), "0".into());

    let error = ToolRegistry::builtin()
        .execute(
            &ToolCall {
                id: "tool_lsp_references".into(),
                name: "lsp_references".into(),
                input,
            },
            &ctx,
        )
        .unwrap_err();
    assert_eq!(error, "lsp_references requires numeric `line`");
}

#[test]
fn lsp_symbols_returns_mock_semantic_output() {
    let ctx = ToolExecutionContext {
        cwd: temp_dir("lsp_symbols"),
        semantic: Some(Arc::new(MockSemanticProvider)),
    };
    let mut input = ToolInput::new();
    input.insert("path".into(), "src/lib.rs".into());

    let result = ToolRegistry::builtin()
        .execute(
            &ToolCall {
                id: "tool_lsp_symbols".into(),
                name: "lsp_symbols".into(),
                input,
            },
            &ctx,
        )
        .unwrap();
    assert!(result.success);
    assert_eq!(result.output, "symbols for src/lib.rs");
}

#[test]
fn read_write_edit_round_trip() {
    let cwd = temp_dir("files");
    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();

    let mut write_input = ToolInput::new();
    write_input.insert("path".into(), "notes.txt".into());
    write_input.insert("content".into(), "hello world".into());
    let write_result = registry
        .execute(
            &ToolCall {
                id: "tool_write".into(),
                name: "write_file".into(),
                input: write_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(write_result.success);

    let mut read_input = ToolInput::new();
    read_input.insert("path".into(), "notes.txt".into());
    let read_result = registry
        .execute(
            &ToolCall {
                id: "tool_read".into(),
                name: "read_file".into(),
                input: read_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(read_result.output.contains("hello world"));

    let mut edit_input = ToolInput::new();
    edit_input.insert("path".into(), "notes.txt".into());
    edit_input.insert("old".into(), "world".into());
    edit_input.insert("new".into(), "rust".into());
    let edit_result = registry
        .execute(
            &ToolCall {
                id: "tool_edit".into(),
                name: "edit_file".into(),
                input: edit_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(edit_result.diff.unwrap().contains("+hello rust"));
}

#[test]
fn shell_adapter_builds_cross_platform_invocations() {
    let (program_unix, args_unix) = build_shell_invocation("echo hi", false);
    assert_eq!(program_unix, "sh");
    assert_eq!(args_unix[0], "-lc");

    let (program_windows, args_windows) = build_shell_invocation("echo hi", true);
    assert_eq!(program_windows, "powershell");
    assert_eq!(args_windows[2], "-Command");
}

#[test]
fn parse_duckduckgo_results_extracts_links_and_titles() {
    let html = r#"
    <div class="results">
      <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F">Rust Programming Language</a>
      <a class="result__snippet">Fast and reliable systems programming language.</a>
    </div>
    "#;
    let results = parse_duckduckgo_results(html, 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Language");
    assert_eq!(results[0].url, "https://www.rust-lang.org/");
    assert!(results[0].snippet.contains("systems programming"));
}

#[test]
fn html_to_text_strips_tags_and_entities() {
    let html = r#"
    <html>
      <head><title>Test</title><style>.x { color: red; }</style></head>
      <body><h1>Hello &amp; Welcome</h1><p>Rust &quot;rocks&quot;.</p></body>
    </html>
    "#;
    let text = html_to_text(html, 10_000);
    assert!(text.contains("Hello & Welcome"));
    assert!(text.contains("Rust \"rocks\"."));
    assert!(!text.contains("<h1>"));
}

#[test]
fn url_encode_escapes_spaces_and_symbols() {
    assert_eq!(url_encode("rust cli"), "rust+cli");
    assert_eq!(url_encode("site:docs.rs tokio"), "site%3Adocs.rs+tokio");
}

#[test]
fn git_status_and_diff_work_in_repo() {
    let cwd = temp_dir("git_repo");
    let status = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(status.success());
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    fs::write(cwd.join("demo.txt"), "hello\n").unwrap();
    let add = Command::new("git")
        .args(["add", "demo.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());
    fs::write(cwd.join("demo.txt"), "hello again\n").unwrap();

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();

    let status_result = registry
        .execute(
            &ToolCall {
                id: "tool_git_status".into(),
                name: "git_status".into(),
                input: ToolInput::new(),
            },
            &ctx,
        )
        .unwrap();
    assert!(status_result.output.contains("demo.txt"));

    let diff_result = registry
        .execute(
            &ToolCall {
                id: "tool_git_diff".into(),
                name: "git_diff".into(),
                input: ToolInput::new(),
            },
            &ctx,
        )
        .unwrap();
    assert!(diff_result.output.contains("hello again"));
}

#[test]
fn git_branch_switch_and_commit_work() {
    let cwd = temp_dir("git_branch");
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
    let add = Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();

    let mut switch_input = ToolInput::new();
    switch_input.insert("branch".into(), "feature/demo".into());
    switch_input.insert("create".into(), "true".into());
    registry
        .execute(
            &ToolCall {
                id: "tool_git_switch".into(),
                name: "git_switch".into(),
                input: switch_input,
            },
            &ctx,
        )
        .unwrap();

    fs::write(cwd.join("tracked.txt"), "second\n").unwrap();
    let mut commit_input = ToolInput::new();
    commit_input.insert("message".into(), "update tracked file".into());
    commit_input.insert("all".into(), "true".into());
    let result = registry
        .execute(
            &ToolCall {
                id: "tool_git_commit".into(),
                name: "git_commit".into(),
                input: commit_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(
        result.output.contains("update tracked file") || result.output.contains("files changed")
    );
}

#[test]
fn git_add_stages_requested_paths() {
    let cwd = temp_dir("git_add");
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    fs::write(cwd.join("notes.txt"), "hello\n").unwrap();

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();

    let mut add_input = ToolInput::new();
    add_input.insert("path".into(), "notes.txt".into());
    registry
        .execute(
            &ToolCall {
                id: "tool_git_add".into(),
                name: "git_add".into(),
                input: add_input,
            },
            &ctx,
        )
        .unwrap();

    let status = Command::new("git")
        .args(["status", "--short"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("A  notes.txt"));
}

#[test]
fn git_push_pushes_current_branch_to_remote() {
    let remote = temp_dir("git_remote");
    let bare = Command::new("git")
        .arg("init")
        .arg("--bare")
        .current_dir(&remote)
        .status()
        .unwrap();
    assert!(bare.success());

    let cwd = temp_dir("git_push");
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    let origin = Command::new("git")
        .args(["remote", "add", "origin", remote.to_string_lossy().as_ref()])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(origin.success());
    fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
    let add = Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();
    let mut push_input = ToolInput::new();
    push_input.insert("set_upstream".into(), "true".into());
    let result = registry
        .execute(
            &ToolCall {
                id: "tool_git_push".into(),
                name: "git_push".into(),
                input: push_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(
        result.output.contains("main")
            || result.output.contains("branch")
            || result.output.contains("up to date")
    );

    let remote_refs = Command::new("git")
        .args(["show-ref"])
        .current_dir(&remote)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&remote_refs.stdout);
    assert!(stdout.contains("refs/heads/main"));
}

#[test]
fn git_restore_reverts_worktree_file() {
    let cwd = temp_dir("git_restore");
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
    let add = Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());
    fs::write(cwd.join("tracked.txt"), "second\n").unwrap();

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();
    let mut restore_input = ToolInput::new();
    restore_input.insert("path".into(), "tracked.txt".into());
    registry
        .execute(
            &ToolCall {
                id: "tool_git_restore".into(),
                name: "git_restore".into(),
                input: restore_input,
            },
            &ctx,
        )
        .unwrap();

    let contents = fs::read_to_string(cwd.join("tracked.txt")).unwrap();
    assert_eq!(contents, "first\n");
}

#[test]
fn git_stash_push_list_and_pop_work() {
    let cwd = temp_dir("git_stash");
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
    let add = Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());
    fs::write(cwd.join("tracked.txt"), "second\n").unwrap();

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();

    let mut push_input = ToolInput::new();
    push_input.insert("message".into(), "save work".into());
    let push_result = registry
        .execute(
            &ToolCall {
                id: "tool_git_stash_push".into(),
                name: "git_stash_push".into(),
                input: push_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(push_result.output.contains("save work") || push_result.output.contains("stash"));

    let list_result = registry
        .execute(
            &ToolCall {
                id: "tool_git_stash_list".into(),
                name: "git_stash_list".into(),
                input: ToolInput::new(),
            },
            &ctx,
        )
        .unwrap();
    assert!(list_result.output.contains("save work"));

    let pop_result = registry
        .execute(
            &ToolCall {
                id: "tool_git_stash_pop".into(),
                name: "git_stash_pop".into(),
                input: ToolInput::new(),
            },
            &ctx,
        )
        .unwrap();
    assert!(pop_result.output.contains("tracked.txt") || pop_result.output.contains("Dropped"));

    let contents = fs::read_to_string(cwd.join("tracked.txt")).unwrap();
    assert_eq!(contents, "second\n");
}

#[test]
fn git_worktree_add_list_and_remove_work() {
    let cwd = temp_dir("git_worktree_repo");
    let init = Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(init.success());
    let email = Command::new("git")
        .args(["config", "user.email", "robocode@example.com"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(email.success());
    let name = Command::new("git")
        .args(["config", "user.name", "RoboCode"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(name.success());
    fs::write(cwd.join("tracked.txt"), "first\n").unwrap();
    let add = Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&cwd)
        .status()
        .unwrap();
    assert!(commit.success());

    let worktree_path = cwd
        .parent()
        .unwrap()
        .join("robocode_tools_worktree_checkout");
    if worktree_path.exists() {
        fs::remove_dir_all(&worktree_path).unwrap();
    }

    let ctx = ToolExecutionContext {
        cwd: cwd.clone(),
        semantic: None,
    };
    let registry = ToolRegistry::builtin();

    let mut add_input = ToolInput::new();
    add_input.insert("path".into(), worktree_path.to_string_lossy().to_string());
    add_input.insert("branch".into(), "feature/worktree".into());
    add_input.insert("create".into(), "true".into());
    registry
        .execute(
            &ToolCall {
                id: "tool_git_worktree_add".into(),
                name: "git_worktree_add".into(),
                input: add_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(worktree_path.exists());

    let list_result = registry
        .execute(
            &ToolCall {
                id: "tool_git_worktree_list".into(),
                name: "git_worktree_list".into(),
                input: ToolInput::new(),
            },
            &ctx,
        )
        .unwrap();
    assert!(
        list_result
            .output
            .contains(worktree_path.to_string_lossy().as_ref())
    );

    let mut remove_input = ToolInput::new();
    remove_input.insert("path".into(), worktree_path.to_string_lossy().to_string());
    registry
        .execute(
            &ToolCall {
                id: "tool_git_worktree_remove".into(),
                name: "git_worktree_remove".into(),
                input: remove_input,
            },
            &ctx,
        )
        .unwrap();
    assert!(!worktree_path.exists());
}
