use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::ModelRequestControl;

pub(crate) struct HttpResponse {
    pub(crate) status_code: u16,
    pub(crate) body: String,
}

pub(crate) fn post_json_with_control(
    api_base: &str,
    path: &str,
    headers: &[String],
    body: &str,
    timeout_secs: u64,
    max_retries: u32,
    control: &ModelRequestControl,
) -> Result<HttpResponse, String> {
    let url = format!("{}{}", api_base.trim_end_matches('/'), path);
    let mut last_error = String::new();
    for attempt in 0..=max_retries {
        control.check_cancelled()?;
        let mut command = Command::new("curl");
        command
            .arg("--silent")
            .arg("--show-error")
            .arg("--max-time")
            .arg(timeout_secs.to_string())
            .arg("-X")
            .arg("POST")
            .arg("-w")
            .arg("\n%{http_code}")
            .arg(url.clone());
        for header in headers {
            command.arg("-H").arg(header);
        }
        command.arg("-d").arg(body);
        let output = run_cancellable(command, control)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            last_error = if !stderr.is_empty() { stderr } else { stdout };
            if attempt < max_retries {
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
            if status_code >= 500 && attempt < max_retries {
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
    control: &ModelRequestControl,
) -> Result<std::process::Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| err.to_string())?;
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

pub(crate) fn split_response_and_status(rendered: &str) -> Option<(String, u16)> {
    let trimmed = rendered.trim_end_matches('\n');
    let (body, status) = trimmed.rsplit_once('\n')?;
    let status_code = status.trim().parse::<u16>().ok()?;
    Some((body.to_string(), status_code))
}
