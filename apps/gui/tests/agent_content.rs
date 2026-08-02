//! Reading the Agent content Core persisted for a message part.
//!
//! Core publishes a workspace-relative reference for bytes it wrote. A webview
//! cannot load that path directly, so the desktop shell reads it and hands the
//! frontend an inline data URL. The reference is a Core fact, but this boundary
//! still refuses to leave the directory Core owns: a projection defect must not
//! turn into an arbitrary file read.

use std::fs;
use std::path::PathBuf;

use viden_gui::{agent_content_data_url, resolve_agent_content_reference};

fn workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("viden_gui_agent_content_{name}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".viden").join("agents").join("parts")).expect("create workspace");
    root
}

#[test]
fn a_persisted_part_resolves_inside_the_workspace() {
    let root = workspace("resolve");
    let resolved = resolve_agent_content_reference(&root, ".viden/agents/parts/abc123.png")
        .expect("a reference Core published must resolve");
    assert_eq!(
        resolved,
        root.join(".viden")
            .join("agents")
            .join("parts")
            .join("abc123.png")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_reference_outside_the_parts_directory_is_refused() {
    let root = workspace("outside");
    for reference in [
        "/etc/passwd",
        "../../etc/passwd",
        ".viden/agents/parts/../../../etc/passwd",
        ".viden/agents/acp-session-1.jsonl",
        ".viden/agents/parts/nested/file.png",
        ".viden/agents/parts/",
    ] {
        assert!(
            resolve_agent_content_reference(&root, reference).is_err(),
            "{reference} must not resolve to a readable path"
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn persisted_bytes_are_returned_as_an_inline_data_url() {
    let root = workspace("read");
    let reference = ".viden/agents/parts/deadbeef.png";
    fs::write(root.join(reference), b"cat").expect("write persisted bytes");

    let url = agent_content_data_url(&root, reference).expect("persisted bytes must be readable");

    // The media type comes from the extension Core chose, never from the
    // caller, so a projection defect cannot relabel bytes as another type.
    assert_eq!(url, "data:image/png;base64,Y2F0");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_part_fails_instead_of_returning_empty_content() {
    let root = workspace("missing");
    assert!(agent_content_data_url(&root, ".viden/agents/parts/absent.png").is_err());
    let _ = fs::remove_dir_all(&root);
}
