pub fn encode_message(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    let mut output = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    output.extend(body);
    Ok(output)
}

pub fn decode_message(buffer: &[u8]) -> Result<Option<serde_json::Value>, String> {
    let header_end = match buffer.windows(4).position(|window| window == b"\r\n\r\n") {
        Some(position) => position,
        None => return Ok(None),
    };
    let header = std::str::from_utf8(&buffer[..header_end]).map_err(|err| err.to_string())?;
    let content_length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .ok_or_else(|| "Missing Content-Length header".to_string())?
        .trim()
        .parse::<usize>()
        .map_err(|_| "Invalid Content-Length header".to_string())?;
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Ok(None);
    }
    let body = &buffer[body_start..body_end];
    let value = serde_json::from_slice(body).map_err(|err| err.to_string())?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_content_length_header() {
        let payload = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"});
        let encoded = encode_message(&payload).unwrap();
        assert!(encoded.starts_with(b"Content-Length: "));
        assert!(encoded.windows(4).any(|window| window == b"\r\n\r\n"));
    }

    #[test]
    fn decodes_single_message_from_buffer() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let mut raw = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        raw.extend(body);
        let decoded = decode_message(&raw).unwrap().unwrap();
        assert_eq!(decoded["id"], 1);
    }

    #[test]
    fn returns_none_for_partial_message() {
        let raw = b"Content-Length: 100\r\n\r\n{\"jsonrpc\":\"2.0\"";
        assert!(decode_message(raw).unwrap().is_none());
    }
}
