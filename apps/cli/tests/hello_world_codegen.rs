use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("viden_cli_{name}_{nanos}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn run_viden(cwd: &Path, session_home: &Path, args: &[&str], input: &str) -> String {
    run_viden_args(
        cwd,
        session_home,
        args.iter().map(|arg| arg.to_string()).collect(),
        input,
    )
}

fn run_viden_args(cwd: &Path, session_home: &Path, args: Vec<String>, input: &str) -> String {
    let mut cli_args = vec!["--no-tui".to_string()];
    cli_args.extend(args);
    let mut child = Command::new(env!("CARGO_BIN_EXE_viden"))
        .args(cli_args)
        .current_dir(cwd)
        .env("VIDEN_SESSION_HOME", session_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "viden failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn startup_errors_use_viden_prefix() {
    let cwd = temp_dir("invalid_startup_flag");
    let session_home = temp_dir("invalid_startup_flag_sessions");
    let output = Command::new(env!("CARGO_BIN_EXE_viden"))
        .arg("--definitely-invalid")
        .current_dir(&cwd)
        .env("VIDEN_SESSION_HOME", &session_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("viden: Unknown startup flag `--definitely-invalid`"),
        "stderr:\n{stderr}"
    );
}

fn assert_python_hello_world(path: &Path) {
    let contents = fs::read_to_string(path).unwrap();
    assert!(
        contents.contains("print(") && contents.to_ascii_lowercase().contains("hello"),
        "unexpected hello world contents:\n{contents}"
    );

    let output = Command::new("python3").arg(path).output().unwrap();
    assert!(
        output.status.success(),
        "python failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Hello, world!"
    );
}

#[test]
fn fallback_tool_codegen_creates_python_hello_world() {
    let cwd = temp_dir("fallback_hello_world");
    let session_home = temp_dir("fallback_hello_world_sessions");

    let stdout = run_viden(
        &cwd,
        &session_home,
        &["--provider", "fallback", "--model", "test-local"],
        "tool write_file path=hello_world.py content=print('Hello,'+chr(32)+'world!')\ny\nquit\n",
    );

    assert!(stdout.contains("write_file"));
    assert_python_hello_world(&cwd.join("hello_world.py"));
}

#[test]
fn fallback_tool_workflow_reads_and_runs_generated_python() {
    let cwd = temp_dir("fallback_tool_workflow");
    let session_home = temp_dir("fallback_tool_workflow_sessions");

    let stdout = run_viden(
        &cwd,
        &session_home,
        &["--provider", "fallback", "--model", "test-local"],
        "tool write_file path=hello_world.py content=print('Hello,'+chr(32)+'world!')\ny\ntool read_file path=hello_world.py\ntool shell command=python3<hello_world.py\ny\nquit\n",
    );

    assert!(stdout.contains("write_file"), "stdout:\n{stdout}");
    assert!(stdout.contains("read_file"), "stdout:\n{stdout}");
    assert!(stdout.contains("shell"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("print('Hello,'+chr(32)+'world!')"),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains("Hello, world!"), "stdout:\n{stdout}");
    assert_python_hello_world(&cwd.join("hello_world.py"));
}

#[test]
#[ignore = "requires DEEPSEEK_API_KEY and live network access"]
fn deepseek_generates_python_hello_world_from_natural_language() {
    if std::env::var("DEEPSEEK_API_KEY").is_err() {
        panic!("DEEPSEEK_API_KEY is required for this ignored smoke test");
    }

    let cwd = temp_dir("deepseek_hello_world");
    let session_home = temp_dir("deepseek_hello_world_sessions");
    let prompt = "Create a file named hello_world.py in the current directory. Use the write_file tool. The file must contain exactly this Python source: print(\"Hello, world!\"). Do not describe the code.";

    let stdout = run_viden(
        &cwd,
        &session_home,
        &[
            "--provider",
            "deepseek",
            "--model",
            "deepseek-v4-flash",
            "--request-timeout",
            "90",
            "--max-retries",
            "1",
        ],
        &format!("{prompt}\ny\nquit\n"),
    );

    assert!(stdout.contains("write_file"), "stdout:\n{stdout}");
    assert_python_hello_world(&cwd.join("hello_world.py"));
}

#[test]
#[ignore = "requires VIDEN_LIVE_PROVIDER, VIDEN_LIVE_MODEL, credentials, and live network access"]
fn selected_live_provider_generates_python_hello_world_from_natural_language() {
    let provider = std::env::var("VIDEN_LIVE_PROVIDER")
        .expect("VIDEN_LIVE_PROVIDER is required for this ignored smoke test");
    let model = std::env::var("VIDEN_LIVE_MODEL")
        .expect("VIDEN_LIVE_MODEL is required for this ignored smoke test");

    let cwd = temp_dir("live_provider_hello_world");
    let session_home = temp_dir("live_provider_hello_world_sessions");
    let prompt = "Create a file named hello_world.py in the current directory. Use the write_file tool. The file must contain exactly this Python source: print(\"Hello, world!\"). Do not describe the code.";

    let mut args = vec![
        "--provider".to_string(),
        provider,
        "--model".to_string(),
        model,
        "--request-timeout".to_string(),
        "90".to_string(),
        "--max-retries".to_string(),
        "1".to_string(),
    ];
    if let Ok(api_base) = std::env::var("VIDEN_LIVE_API_BASE") {
        args.push("--api-base".to_string());
        args.push(api_base);
    }
    if let Ok(api_key) = std::env::var("VIDEN_LIVE_API_KEY") {
        args.push("--api-key".to_string());
        args.push(api_key);
    }

    let stdout = run_viden_args(&cwd, &session_home, args, &format!("{prompt}\ny\nquit\n"));

    assert!(stdout.contains("write_file"), "stdout:\n{stdout}");
    assert_python_hello_world(&cwd.join("hello_world.py"));
}
