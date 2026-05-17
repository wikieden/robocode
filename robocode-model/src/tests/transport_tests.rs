use super::*;

#[test]
fn split_response_and_status_parses_curl_suffix() {
    let response = split_response_and_status("{\"ok\":true}\n200").unwrap();
    assert_eq!(response.0, "{\"ok\":true}");
    assert_eq!(response.1, 200);
}
