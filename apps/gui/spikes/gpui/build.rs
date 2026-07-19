use std::{env, fs, path::PathBuf};

const TOKEN_ROLES: [&str; 4] = ["bg-base", "fg-primary", "accent", "gold"];

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let source = manifest_dir.join("../../../../docs/viden-design/Viden/tokens.css");
    println!("cargo:rerun-if-changed={}", source.display());

    let css = fs::read_to_string(&source).expect("read canonical Viden tokens.css");
    let aurora = extract_palette(&css, "[data-skin=\"aurora\"][data-mode=\"dark\"]{");
    let ice = extract_palette(&css, "[data-skin=\"ice\"][data-mode=\"light\"]{");
    let generated = format!(
        "pub const AURORA_DARK_TOKENS: GeneratedTokens = {};\n\
         pub const ICE_LIGHT_TOKENS: GeneratedTokens = {};\n",
        render_tokens(&aurora),
        render_tokens(&ice)
    );

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("gpui_tokens.rs");
    fs::write(out, generated).expect("write generated GPUI token adapter");
}

fn extract_palette(css: &str, selector: &str) -> [u32; 4] {
    let start = css
        .find(selector)
        .unwrap_or_else(|| panic!("missing selector {selector}"));
    let block = &css[start + selector.len()..];
    let block = &block[..block.find('}').expect("close token block")];
    TOKEN_ROLES.map(|role| extract_hex(block, role))
}

fn extract_hex(block: &str, role: &str) -> u32 {
    let marker = format!("--{role}:");
    let value = block
        .split(&marker)
        .nth(1)
        .unwrap_or_else(|| panic!("missing token {marker}"))
        .split(';')
        .next()
        .expect("token terminator")
        .trim();
    u32::from_str_radix(value.trim_start_matches('#'), 16)
        .unwrap_or_else(|_| panic!("token {marker} is not a hex color: {value}"))
}

fn render_tokens(values: &[u32; 4]) -> String {
    format!(
        "GeneratedTokens {{ bg_base: 0x{:06x}, fg_primary: 0x{:06x}, accent: 0x{:06x}, gold: 0x{:06x} }}",
        values[0], values[1], values[2], values[3]
    )
}
