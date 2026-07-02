use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::ModelRequestControl;

pub(crate) struct HttpResponse {
    pub(crate) status_code: u16,
    pub(crate) body: String,
}

pub(crate) struct HttpRequestControl<'a> {
    pub(crate) timeout_secs: u64,
    pub(crate) max_retries: u32,
    pub(crate) control: &'a ModelRequestControl,
    pub(crate) stream_delta: Option<fn(&str) -> Option<String>>,
}

pub(crate) fn post_json_with_control(
    api_base: &str,
    path: &str,
    headers: &[String],
    body: &str,
    request_control: HttpRequestControl<'_>,
) -> Result<HttpResponse, String> {
    let url = format!("{}{}", api_base.trim_end_matches('/'), path);
    let mut last_error = String::new();
    for attempt in 0..=request_control.max_retries {
        request_control.control.check_cancelled()?;
        let mut command = Command::new("curl");
        command
            .arg("--silent")
            .arg("--show-error")
            .arg("--max-time")
            .arg(request_control.timeout_secs.to_string())
            .arg("-X")
            .arg("POST")
            .arg("-w")
            .arg("\n%{http_code}")
            .arg(url.clone());
        for header in headers {
            command.arg("-H").arg(header);
        }
        command.arg("--data-binary").arg("@-");
        let output = if request_control.control.has_stream_sink()
            && request_control.stream_delta.is_some()
        {
            run_streaming_cancellable(
                command,
                body,
                request_control.control,
                request_control.stream_delta,
            )?
        } else {
            run_cancellable(command, body, request_control.control)?
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            last_error = if !stderr.is_empty() { stderr } else { stdout };
            if attempt < request_control.max_retries {
                continue;
            }
            return Err(if last_error.is_empty() {
                format!("curl failed with status {}", output.status)
            } else {
                last_error
            });
        }
        let rendered = String::from_utf8_lossy(&output.stdout).to_string();
        if let Some((body, status_code)) = split_response_and_status(&rendered) {
            if status_code >= 500 && attempt < request_control.max_retries {
                last_error = format!("HTTP {}", status_code);
                continue;
            }
            return Ok(HttpResponse { status_code, body });
        }
        last_error = "Could not parse HTTP status from curl output".to_string();
    }
    Err(last_error)
}

fn run_cancellable(
    mut command: Command,
    body: &str,
    control: &ModelRequestControl,
) -> Result<std::process::Output, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| err.to_string())?;
    write_request_body(&mut child, body)?;
    loop {
        control.check_cancelled().inspect_err(|_err| {
            let _ = child.kill();
            let _ = child.wait();
        })?;
        if child.try_wait().map_err(|err| err.to_string())?.is_some() {
            return child.wait_with_output().map_err(|err| err.to_string());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn run_streaming_cancellable(
    mut command: Command,
    body: &str,
    control: &ModelRequestControl,
    stream_delta: Option<fn(&str) -> Option<String>>,
) -> Result<std::process::Output, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| err.to_string())?;
    write_request_body(&mut child, body)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture streaming stdout".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture streaming stderr".to_string())?;
    let stderr_handle = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        buffer
    });

    let mut rendered = Vec::<u8>::new();
    let mut reader = BufReader::new(&mut stdout);
    loop {
        control.check_cancelled().inspect_err(|_err| {
            let _ = child.kill();
            let _ = child.wait();
        })?;
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line).map_err(|err| {
            let _ = child.kill();
            err.to_string()
        })?;
        if read == 0 {
            break;
        }
        if let Ok(text) = std::str::from_utf8(&line)
            && let Some(parser) = stream_delta
            && let Some(delta) = parser(text)
        {
            control.emit_stream_delta(delta);
        }
        rendered.extend_from_slice(&line);
    }
    let status = child.wait().map_err(|err| err.to_string())?;
    let stderr = stderr_handle.join().unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout: rendered,
        stderr,
    })
}

fn write_request_body(child: &mut std::process::Child, body: &str) -> Result<(), String> {
    let Some(mut stdin) = child.stdin.take() else {
        return Err("failed to open curl stdin for request body".to_string());
    };
    stdin
        .write_all(body.as_bytes())
        .map_err(|err| format!("failed to write request body to curl stdin: {err}"))?;
    Ok(())
}

pub(crate) fn split_response_and_status(rendered: &str) -> Option<(String, u16)> {
    let trimmed = rendered.trim_end_matches('\n');
    let (body, status) = trimmed.rsplit_once('\n')?;
    let status_code = status.trim().parse::<u16>().ok()?;
    Some((body.to_string(), status_code))
}
