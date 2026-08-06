//! Reads the Agent content Core persisted for a typed message part.
//!
//! Core writes inline Agent bytes into the workspace and publishes the path it
//! wrote as the part's reference. A webview cannot load that path, so the shell
//! reads the file and returns an inline data URL.
//!
//! The reference is a Core fact, but this boundary still refuses to leave the
//! directory Core owns. A projection defect, or a future Core that publishes a
//! reference from somewhere else, must fail here rather than become an
//! arbitrary file read performed on the operator's behalf.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;

/// Directory Core persists Agent content into, relative to the workspace root.
const PARTS_PREFIX: &str = ".viden/agents/parts/";

/// Resolves a Core-published content reference to a readable path.
///
/// Only a direct child of the parts directory resolves: no nesting, no parent
/// traversal, no absolute path.
pub fn resolve_agent_content_reference(root: &Path, reference: &str) -> Result<PathBuf, String> {
    let Some(name) = reference.strip_prefix(PARTS_PREFIX) else {
        return Err(format!("gui.agentContent.outsideParts:{reference}"));
    };
    let rejected = name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.');
    if rejected {
        return Err(format!("gui.agentContent.invalidReference:{reference}"));
    }
    Ok(root.join(".viden").join("agents").join("parts").join(name))
}

/// Reads persisted Agent content as an inline data URL.
///
/// The media type is derived from the extension Core chose when it wrote the
/// file, never from the caller, so a caller cannot relabel bytes as another
/// type on the way into the page.
pub fn agent_content_data_url(root: &Path, reference: &str) -> Result<String, String> {
    let path = resolve_agent_content_reference(root, reference)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("gui.agentContent.unreadable:{reference}:{error}"))?;
    let media_type = media_type_for(&path);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{media_type};base64,{encoded}"))
}

/// Maps the extension Core wrote onto the media type the page renders with.
///
/// An extension this build does not recognise stays a generic byte stream: the
/// page shows that content exists without the shell asserting a type it cannot
/// vouch for.
fn media_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
