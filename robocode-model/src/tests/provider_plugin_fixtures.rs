use crate::plugin::dynamic_library_suffixes;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("robocode_provider_host_{name}_{nanos}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

pub(crate) fn compile_runtime_provider_plugin(plugin_dir: &Path) -> PathBuf {
    compile_provider_plugin(
        plugin_dir,
        "runtime_provider_plugin",
        "runtime-provider",
        "Runtime Provider",
        "https://runtime.example.com",
        "runtime-default",
    )
}

pub(crate) fn compile_invalid_provider_plugin(plugin_dir: &Path) -> PathBuf {
    compile_provider_plugin(
        plugin_dir,
        "invalid_provider_plugin",
        "invalid-provider",
        "Invalid Provider",
        "ftp://invalid.example.com",
        "invalid-default",
    )
}

pub(crate) fn compile_provider_plugin(
    plugin_dir: &Path,
    crate_name: &str,
    provider_id: &str,
    display_name: &str,
    api_base: &str,
    default_model: &str,
) -> PathBuf {
    let source_text = r#"
use std::ffi::c_char;

#[no_mangle]
pub extern "C" fn robocode_provider_descriptor_json() -> *const c_char {
    static DESCRIPTOR_JSON: &str = concat!(
        "{\"provider_id\":\"__PROVIDER_ID__\",",
        "\"display_name\":\"__DISPLAY_NAME__\",",
        "\"version\":\"1\",",
        "\"protocol_family\":\"OpenAi\",",
        "\"default_api_base\":\"__API_BASE__\",",
        "\"default_model\":\"__DEFAULT_MODEL__\",",
        "\"env_mappings\":{\"api_key_env\":\"RUNTIME_PROVIDER_API_KEY\",\"api_base_env\":null},",
        "\"capabilities\":{\"supports_streaming\":false,\"supports_native_tool_calling\":true},",
        "\"config_schema_version\":1}\0"
    );
    DESCRIPTOR_JSON.as_ptr().cast()
}
"#
    .replace("__PROVIDER_ID__", provider_id)
    .replace("__DISPLAY_NAME__", display_name)
    .replace("__API_BASE__", api_base)
    .replace("__DEFAULT_MODEL__", default_model);
    compile_raw_provider_plugin(plugin_dir, crate_name, &source_text)
}

pub(crate) fn compile_raw_provider_plugin(
    plugin_dir: &Path,
    crate_name: &str,
    source_text: &str,
) -> PathBuf {
    let source = plugin_dir.join(format!("{crate_name}.rs"));
    let library = plugin_dir.join(format!("lib{crate_name}.{}", dynamic_library_suffixes()[0]));
    std::fs::write(&source, source_text).unwrap();
    let output = Command::new("rustc")
        .args(["--edition=2021", "--crate-type", "cdylib"])
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap_or_else(|err| panic!("failed to launch rustc for provider plugin test: {err}"));
    assert!(
        output.status.success(),
        "failed to compile provider plugin test dylib\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    library
}
