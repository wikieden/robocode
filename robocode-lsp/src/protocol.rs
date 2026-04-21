pub fn initialize_request(id: u64, root_uri: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "rootUri": root_uri,
            "capabilities": {}
        }
    })
}

pub fn did_open_text_document(path_uri: &str, language_id: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": path_uri,
                "languageId": language_id,
                "version": 1,
                "text": text
            }
        }
    })
}

pub fn document_symbol_request(id: u64, path_uri: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": {
                "uri": path_uri
            }
        }
    })
}

pub fn references_request(
    id: u64,
    path_uri: &str,
    line: u32,
    character: u32,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": {
                "uri": path_uri
            },
            "position": {
                "line": line,
                "character": character
            },
            "context": {
                "includeDeclaration": true
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_initialize_request() {
        let request = initialize_request(7, "file:///tmp/project");
        assert_eq!(request["jsonrpc"], "2.0");
        assert_eq!(request["id"], 7);
        assert_eq!(request["method"], "initialize");
        assert_eq!(request["params"]["rootUri"], "file:///tmp/project");
    }

    #[test]
    fn builds_references_request_with_zero_based_position() {
        let request = references_request(9, "file:///tmp/project/src/lib.rs", 3, 4);
        assert_eq!(request["method"], "textDocument/references");
        assert_eq!(request["params"]["position"]["line"], 3);
        assert_eq!(request["params"]["position"]["character"], 4);
        assert_eq!(request["params"]["context"]["includeDeclaration"], true);
    }
}
