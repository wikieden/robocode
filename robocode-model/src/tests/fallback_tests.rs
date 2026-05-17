use super::*;

#[test]
fn explicit_tool_syntax_still_creates_tool_calls() {
    let call = parse_explicit_tool_call("tool read_file path=Cargo.toml").unwrap();
    assert_eq!(call.name, "read_file");
    assert_eq!(
        call.input.get("path").map(String::as_str),
        Some("Cargo.toml")
    );
}
