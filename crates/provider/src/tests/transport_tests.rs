use super::*;
use crate::transport::{HttpRequestControl, post_json_with_control};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

#[test]
fn split_response_and_status_parses_curl_suffix() {
    let response = split_response_and_status("{\"ok\":true}\n200").unwrap();
    assert_eq!(response.0, "{\"ok\":true}");
    assert_eq!(response.1, 200);
}

#[test]
fn post_json_sends_large_body_through_stdin() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server");
    let address = listener.local_addr().expect("local addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("connection");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request
                .windows(b"\r\n\r\n".len())
                .any(|window| window == b"\r\n\r\n")
            {
                let rendered = String::from_utf8_lossy(&request);
                let content_length = rendered
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .expect("content length");
                let header_end = request
                    .windows(b"\r\n\r\n".len())
                    .position(|window| window == b"\r\n\r\n")
                    .expect("headers")
                    + 4;
                while request.len().saturating_sub(header_end) < content_length {
                    let read = stream.read(&mut buffer).expect("read body");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                break;
            }
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}")
            .expect("write response");
    });
    let body = format!("{{\"prompt\":\"{}\"}}", "x".repeat(128 * 1024));
    let control = ModelRequestControl::new();

    let response = post_json_with_control(
        &format!("http://{address}"),
        "/v1/chat/completions",
        &["Content-Type: application/json".to_string()],
        &body,
        HttpRequestControl {
            timeout_secs: 5,
            max_retries: 0,
            control: &control,
            stream_delta: None,
        },
    )
    .expect("large request body should not be passed as argv");

    server.join().expect("server joined");
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, "{\"ok\":true}");
}
