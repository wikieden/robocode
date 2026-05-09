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
    let path = std::env::temp_dir().join(format!("robocode_cli_{name}_{nanos}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn run_robocode(cwd: &Path, session_home: &Path, args: &[&str], input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_robocode-cli"))
        .args(args)
        .current_dir(cwd)
        .env("ROBOCODE_SESSION_HOME", session_home)
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
        "robocode failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
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

    let stdout = run_robocode(
        &cwd,
        &session_home,
        &["--provider", "fallback", "--model", "test-local"],
        "tool write_file path=hello_world.py content=print('Hello,'+chr(32)+'world!')\ny\nquit\n",
    );

    assert!(stdout.contains("write_file"));
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

    let stdout = run_robocode(
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
