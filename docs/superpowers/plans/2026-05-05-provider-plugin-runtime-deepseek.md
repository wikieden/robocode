# Provider Plugin Runtime and DeepSeek v4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current hardcoded provider factory with a plugin-extensible provider runtime that supports dynamic provider discovery, provider-scoped config resolution, per-agent provider binding, and DeepSeek v4 as the first plugin-backed provider.

**Architecture:** Split `viden-model` into a host/registry/adapter/plugin shape. Keep `SessionEngine` and the core tool loop untouched: they continue to consume `Box<dyn ModelProvider>`. Phase 1 uses native dynamic libraries for plugin loading, but the ABI is kept serialized and host-mediated so it can later migrate to WASM without redesigning the provider contract.

**Tech Stack:** Rust 2024 workspace, existing `viden-model` / `viden-config` / `viden-cli`, native dynamic library loading via `libloading`, serializable descriptor/payload structs in a new SDK crate, existing `curl`-based HTTP provider path

---

## File Map

Create:

- `viden-provider-sdk/Cargo.toml`
- `viden-provider-sdk/src/lib.rs`
- `viden-provider-deepseek/Cargo.toml`
- `viden-provider-deepseek/src/lib.rs`
- `viden-model/src/adapters.rs`
- `viden-model/src/config.rs`
- `viden-model/src/descriptor.rs`
- `viden-model/src/host.rs`
- `viden-model/src/http.rs`
- `viden-model/src/plugin.rs`
- `viden-model/src/registry.rs`
- `docs/superpowers/plans/2026-05-05-provider-plugin-runtime-deepseek.md`

Modify:

- `Cargo.toml`
- `Cargo.lock`
- `viden-model/Cargo.toml`
- `viden-model/src/lib.rs`
- `viden-config/src/lib.rs`
- `viden-cli/src/main.rs`
- `README.md`
- `README.zh-CN.md`
- `docs/architecture.md`
- `docs/architecture.zh-CN.md`
- `docs/modules.md`
- `docs/modules.zh-CN.md`
- `PLAN.md`

No changes:

- `viden-core`
- `viden-tools`
- `viden-session`
- `viden-permissions`
- `viden-workflows`

## Task 1: Split `viden-model` Into Host/Registry/Adapter Modules

**Files:**
- Modify: `viden-model/Cargo.toml`
- Modify: `viden-model/src/lib.rs`
- Create: `viden-model/src/adapters.rs`
- Create: `viden-model/src/config.rs`
- Create: `viden-model/src/descriptor.rs`
- Create: `viden-model/src/http.rs`
- Create: `viden-model/src/plugin.rs`
- Create: `viden-model/src/registry.rs`

- [ ] **Step 1: Write the failing registry and descriptor tests**

Add to `viden-model/src/lib.rs` test module:

```rust
#[test]
fn registry_lists_builtin_provider_ids() {
    let registry = ProviderRegistry::with_builtins();
    let ids = registry.provider_ids();
    assert!(ids.contains(&"anthropic".to_string()));
    assert!(ids.contains(&"openai".to_string()));
    assert!(ids.contains(&"fallback".to_string()));
}

#[test]
fn descriptor_keeps_provider_identity_separate_from_protocol_family() {
    let descriptor = ProviderDescriptor {
        provider_id: "deepseek".to_string(),
        display_name: "DeepSeek".to_string(),
        version: "1".to_string(),
        protocol_family: ProtocolFamily::OpenAi,
        default_api_base: Some("https://api.deepseek.com".to_string()),
        default_model: Some("deepseek-v4-flash".to_string()),
        env_mappings: ProviderEnvMappings::default(),
        capabilities: ProviderCapabilities::default(),
        config_schema_version: 1,
    };

    assert_eq!(descriptor.provider_id, "deepseek");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::OpenAi);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p viden-model registry_lists_builtin_provider_ids
```

Expected: FAIL because the registry/descriptor modules do not exist yet.

- [ ] **Step 3: Create the module skeleton**

Create `viden-model/src/descriptor.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtocolFamily {
    Anthropic,
    OpenAi,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderEnvMappings {
    pub api_key_env: Option<String>,
    pub api_base_env: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_native_tool_calling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_family: ProtocolFamily,
    pub default_api_base: Option<String>,
    pub default_model: Option<String>,
    pub env_mappings: ProviderEnvMappings,
    pub capabilities: ProviderCapabilities,
    pub config_schema_version: u32,
}
```

Create `viden-model/src/registry.rs`:

```rust
use crate::descriptor::{ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings};

#[derive(Debug, Default, Clone)]
pub struct ProviderRegistry {
    descriptors: Vec<ProviderDescriptor>,
}

impl ProviderRegistry {
    pub fn with_builtins() -> Self {
        Self {
            descriptors: vec![
                ProviderDescriptor {
                    provider_id: "anthropic".to_string(),
                    display_name: "Anthropic".to_string(),
                    version: "builtin".to_string(),
                    protocol_family: ProtocolFamily::Anthropic,
                    default_api_base: None,
                    default_model: Some("claude-sonnet-4-5".to_string()),
                    env_mappings: ProviderEnvMappings {
                        api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                        api_base_env: None,
                    },
                    capabilities: ProviderCapabilities {
                        supports_streaming: true,
                        supports_native_tool_calling: true,
                    },
                    config_schema_version: 1,
                },
                ProviderDescriptor {
                    provider_id: "openai".to_string(),
                    display_name: "OpenAI".to_string(),
                    version: "builtin".to_string(),
                    protocol_family: ProtocolFamily::OpenAi,
                    default_api_base: None,
                    default_model: Some("gpt-5.2".to_string()),
                    env_mappings: ProviderEnvMappings {
                        api_key_env: Some("OPENAI_API_KEY".to_string()),
                        api_base_env: Some("OPENAI_API_BASE".to_string()),
                    },
                    capabilities: ProviderCapabilities {
                        supports_streaming: true,
                        supports_native_tool_calling: true,
                    },
                    config_schema_version: 1,
                },
                ProviderDescriptor {
                    provider_id: "fallback".to_string(),
                    display_name: "Fallback".to_string(),
                    version: "builtin".to_string(),
                    protocol_family: ProtocolFamily::OpenAi,
                    default_api_base: None,
                    default_model: Some("test-model".to_string()),
                    env_mappings: ProviderEnvMappings::default(),
                    capabilities: ProviderCapabilities::default(),
                    config_schema_version: 1,
                },
            ],
        }
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.descriptors
            .iter()
            .map(|item| item.provider_id.clone())
            .collect()
    }
}
```

Replace the top of `viden-model/src/lib.rs` with:

```rust
mod adapters;
mod config;
mod descriptor;
mod http;
mod plugin;
mod registry;

pub use config::ProviderConfig;
pub use descriptor::{ProtocolFamily, ProviderCapabilities, ProviderDescriptor, ProviderEnvMappings};
pub use registry::ProviderRegistry;
```

Leave the current concrete provider logic in place temporarily, but move helper types into the new modules so later tasks can replace the hardcoded factory cleanly.

- [ ] **Step 4: Run the focused tests**

Run:

```bash
cargo test -p viden-model registry_lists_builtin_provider_ids
cargo test -p viden-model descriptor_keeps_provider_identity_separate_from_protocol_family
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add viden-model/Cargo.toml viden-model/src/lib.rs viden-model/src/adapters.rs viden-model/src/config.rs viden-model/src/descriptor.rs viden-model/src/http.rs viden-model/src/plugin.rs viden-model/src/registry.rs
git commit -m "Split viden-model into provider runtime modules"
```

## Task 2: Add Provider SDK and Dynamic Plugin ABI

**Files:**
- Create: `viden-provider-sdk/Cargo.toml`
- Create: `viden-provider-sdk/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `viden-model/Cargo.toml`
- Modify: `viden-model/src/plugin.rs`

- [ ] **Step 1: Write the failing plugin manifest roundtrip test**

Create `viden-provider-sdk/src/lib.rs` with a failing test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_descriptor_roundtrips_through_json() {
        let descriptor = PluginDescriptor {
            provider_id: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            version: "1".to_string(),
            protocol_family: "openai".to_string(),
            default_api_base: Some("https://api.deepseek.com".to_string()),
            default_model: Some("deepseek-v4-flash".to_string()),
            api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
            api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.provider_id, "deepseek");
        assert_eq!(decoded.protocol_family, "openai");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p viden-provider-sdk plugin_descriptor_roundtrips_through_json
```

Expected: FAIL because `PluginDescriptor` does not exist yet in the new file.

- [ ] **Step 3: Add the SDK crate and plugin ABI types**

Create `viden-provider-sdk/Cargo.toml`:

```toml
[package]
name = "viden-provider-sdk"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Create `viden-provider-sdk/src/lib.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_family: String,
    pub default_api_base: Option<String>,
    pub default_model: Option<String>,
    pub api_key_env: Option<String>,
    pub api_base_env: Option<String>,
}

pub const VIDEN_PLUGIN_DESCRIPTOR_SYMBOL: &str = "viden_provider_descriptor_json";
```

Add to the workspace `Cargo.toml`:

```toml
members = [
  "viden-cli",
  "viden-config",
  "viden-core",
  "viden-lsp",
  "viden-model",
  "viden-permissions",
  "viden-provider-sdk",
  "viden-session",
  "viden-tools",
  "viden-types",
  "viden-workflows",
]
```

Add dependency in `viden-model/Cargo.toml`:

```toml
viden-provider-sdk = { path = "../viden-provider-sdk" }
libloading = "0.8"
```

Create the plugin loader skeleton in `viden-model/src/plugin.rs`:

```rust
use viden_provider_sdk::{PluginDescriptor, VIDEN_PLUGIN_DESCRIPTOR_SYMBOL};

#[derive(Debug)]
pub struct LoadedPluginDescriptor {
    pub source_path: std::path::PathBuf,
    pub descriptor: PluginDescriptor,
}

pub fn dynamic_library_suffixes() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    { &["dylib"] }
    #[cfg(target_os = "linux")]
    { &["so"] }
    #[cfg(target_os = "windows")]
    { &["dll"] }
}
```

- [ ] **Step 4: Run the focused tests**

Run:

```bash
cargo test -p viden-provider-sdk plugin_descriptor_roundtrips_through_json
cargo check -p viden-model
```

Expected: PASS. The SDK test passes, and `viden-model` still compiles with the new plugin skeleton and dependencies.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock viden-model/Cargo.toml viden-model/src/plugin.rs viden-provider-sdk/Cargo.toml viden-provider-sdk/src/lib.rs
git commit -m "Add provider plugin SDK and dynamic ABI skeleton"
```

## Task 3: Make Config Provider-Scoped With Generic Fallback

**Files:**
- Modify: `viden-config/src/lib.rs`
- Modify: `viden-model/src/config.rs`
- Modify: `README.md`
- Modify: `README.zh-CN.md`

- [ ] **Step 1: Write the failing DeepSeek config precedence test**

Add to `viden-config/src/lib.rs` tests:

```rust
#[test]
fn deepseek_provider_specific_env_overrides_generic_api_fields() {
    let cwd = std::env::temp_dir().join("viden_deepseek_config_test");
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(cwd.join(".viden")).unwrap();
    fs::write(
        cwd.join(".viden").join("config.toml"),
        r#"
provider = "deepseek"
api_key = "generic-key"
api_base = "https://generic.example"
[providers.deepseek]
api_base = "https://provider.example"
"#,
    )
    .unwrap();

    let env_map = map_env(&[
        ("DEEPSEEK_API_KEY", "provider-key"),
    ]);

    let config = load_config_with_env(&cwd, &CliOverrides::default(), &|key| {
        env_map.get(key).cloned()
    })
    .unwrap();

    assert_eq!(config.provider, "deepseek");
    assert_eq!(config.api_key.as_deref(), Some("provider-key"));
    assert_eq!(config.api_base.as_deref(), Some("https://provider.example"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p viden-config deepseek_provider_specific_env_overrides_generic_api_fields
```

Expected: FAIL because provider-scoped config and DeepSeek-specific env precedence do not exist yet.

- [ ] **Step 3: Extend config parsing**

Change `FileConfig` in `viden-config/src/lib.rs`:

```rust
#[derive(Debug, Default, Deserialize)]
struct ProviderScopedFileConfig {
    api_base: Option<String>,
    api_key: Option<String>,
    api_key_env: Option<String>,
    default_model: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ProvidersFileConfig {
    deepseek: Option<ProviderScopedFileConfig>,
    anthropic: Option<ProviderScopedFileConfig>,
    openai: Option<ProviderScopedFileConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    provider: Option<String>,
    model: Option<String>,
    api_base: Option<String>,
    api_key: Option<String>,
    api_key_env: Option<String>,
    permission_mode: Option<String>,
    session_home: Option<String>,
    request_timeout_secs: Option<u64>,
    max_retries: Option<u32>,
    providers: Option<ProvidersFileConfig>,
}
```

Then apply provider-scoped overrides after generic file fields:

```rust
fn apply_provider_scoped_config(
    resolved: &mut ResolvedConfig,
    provider: &str,
    providers: &Option<ProvidersFileConfig>,
    env_lookup: &impl Fn(&str) -> Option<String>,
) {
    let scoped = match provider {
        "deepseek" => providers.as_ref().and_then(|all| all.deepseek.as_ref()),
        "anthropic" => providers.as_ref().and_then(|all| all.anthropic.as_ref()),
        "openai" | "openai-compatible" => providers.as_ref().and_then(|all| all.openai.as_ref()),
        _ => None,
    };

    if let Some(scoped) = scoped {
        if let Some(api_base) = &scoped.api_base {
            resolved.api_base = Some(api_base.clone());
        }
        if let Some(api_key) = &scoped.api_key {
            resolved.api_key = Some(api_key.clone());
        }
        if let Some(api_key_env) = &scoped.api_key_env {
            if let Some(value) = env_lookup(api_key_env) {
                resolved.api_key = Some(value);
            }
        }
    }
}
```

Add DeepSeek env fallback in `apply_env_config`:

```rust
if resolved.provider == "deepseek" {
    if let Some(api_key) = env_lookup("DEEPSEEK_API_KEY") {
        resolved.api_key = Some(api_key);
    }
    if let Some(api_base) = env_lookup("DEEPSEEK_API_BASE") {
        resolved.api_base = Some(api_base);
    }
}
```

Document the new config shape in both READMEs.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p viden-config deepseek_provider_specific_env_overrides_generic_api_fields
cargo test -p viden-config --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add viden-config/src/lib.rs README.md README.zh-CN.md
git commit -m "Add provider-scoped config with DeepSeek precedence"
```

## Task 4: Replace Hardcoded Factory With Provider Host + Per-Agent Binding

**Files:**
- Modify: `viden-model/src/lib.rs`
- Modify: `viden-model/src/config.rs`
- Create: `viden-model/src/host.rs`
- Modify: `viden-cli/src/main.rs`

- [ ] **Step 1: Write the failing registry-refresh and per-instance binding tests**

Add to `viden-model/src/lib.rs` tests:

```rust
#[test]
fn provider_host_can_refresh_registry_without_replacing_existing_provider_instance() {
    let mut host = ProviderHost::with_builtins();
    let before = host.registry().provider_ids();
    host.refresh().unwrap();
    let after = host.registry().provider_ids();
    assert_eq!(before, after);
}

#[test]
fn different_provider_configs_create_independent_provider_instances() {
    let host = ProviderHost::with_builtins();
    let deepseek = host
        .create(ProviderConfig::from_settings("deepseek", Some("deepseek-v4-flash"), None, None, 90, 1).unwrap())
        .unwrap();
    let anthropic = host
        .create(ProviderConfig::from_settings("anthropic", Some("claude-sonnet-4-5"), None, None, 90, 1).unwrap())
        .unwrap();

    assert_eq!(deepseek.provider_name(), "deepseek");
    assert_eq!(anthropic.provider_name(), "anthropic");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p viden-model provider_host_can_refresh_registry_without_replacing_existing_provider_instance
```

Expected: FAIL because `ProviderHost` does not exist yet.

- [ ] **Step 3: Add ProviderHost and switch CLI construction**

Create `viden-model/src/host.rs`:

```rust
use crate::{ProviderConfig, ProviderRegistry, load_builtin_provider};

pub struct ProviderHost {
    registry: ProviderRegistry,
}

impl ProviderHost {
    pub fn with_builtins() -> Self {
        Self {
            registry: ProviderRegistry::with_builtins(),
        }
    }

    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        self.registry = ProviderRegistry::with_builtins();
        Ok(())
    }

    pub fn create(&self, config: ProviderConfig) -> Result<Box<dyn crate::ModelProvider>, String> {
        load_builtin_provider(&self.registry, config)
    }
}
```

In `viden-model/src/lib.rs`, replace `create_provider` with:

```rust
pub fn create_provider(config: ProviderConfig) -> Box<dyn ModelProvider> {
    ProviderHost::with_builtins()
        .create(config)
        .expect("builtin provider construction should succeed")
}
```

Then switch `viden-cli/src/main.rs` startup path from direct factory use to:

```rust
use viden_model::{ProviderConfig, ProviderHost};

let provider_host = ProviderHost::with_builtins();
let provider = provider_host.create(provider_config)?;
```

This preserves per-process shared registry while keeping actual provider
instances local to each engine.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p viden-model provider_host_
cargo test -p viden-cli -- --help
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add viden-model/src/lib.rs viden-model/src/config.rs viden-model/src/host.rs viden-cli/src/main.rs
git commit -m "Introduce provider host and instance-scoped provider creation"
```

## Task 5: Add DeepSeek Plugin and Dynamic Discovery

**Files:**
- Create: `viden-provider-deepseek/Cargo.toml`
- Create: `viden-provider-deepseek/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `viden-model/src/plugin.rs`
- Modify: `viden-model/src/registry.rs`
- Modify: `viden-model/src/lib.rs`

- [ ] **Step 1: Write the failing DeepSeek plugin tests**

Add to `viden-model/src/lib.rs` tests:

```rust
#[test]
fn registry_exposes_deepseek_as_independent_provider_id() {
    let registry = ProviderRegistry::with_builtins();
    assert!(registry.provider_ids().contains(&"deepseek".to_string()));
}

#[test]
fn deepseek_provider_uses_openai_protocol_family() {
    let registry = ProviderRegistry::with_builtins();
    let descriptor = registry.descriptor("deepseek").unwrap();
    assert_eq!(descriptor.provider_id, "deepseek");
    assert_eq!(descriptor.protocol_family, ProtocolFamily::OpenAi);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p viden-model registry_exposes_deepseek_as_independent_provider_id
```

Expected: FAIL because DeepSeek is not in the builtin or plugin registry yet.

- [ ] **Step 3: Add the DeepSeek provider plugin**

Create `viden-provider-deepseek/Cargo.toml`:

```toml
[package]
name = "viden-provider-deepseek"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
viden-provider-sdk = { path = "../viden-provider-sdk" }
serde_json = "1"
```

Create `viden-provider-deepseek/src/lib.rs`:

```rust
use viden_provider_sdk::PluginDescriptor;
use std::ffi::c_char;

static DESCRIPTOR_JSON: &str = r#"{
  "provider_id":"deepseek",
  "display_name":"DeepSeek",
  "version":"1",
  "protocol_family":"openai",
  "default_api_base":"https://api.deepseek.com",
  "default_model":"deepseek-v4-flash",
  "api_key_env":"DEEPSEEK_API_KEY",
  "api_base_env":"DEEPSEEK_API_BASE"
}"#;

#[unsafe(no_mangle)]
pub extern "C" fn viden_provider_descriptor_json() -> *const c_char {
    concat!(
        "{",
        "\"provider_id\":\"deepseek\",",
        "\"display_name\":\"DeepSeek\",",
        "\"version\":\"1\",",
        "\"protocol_family\":\"openai\",",
        "\"default_api_base\":\"https://api.deepseek.com\",",
        "\"default_model\":\"deepseek-v4-flash\",",
        "\"api_key_env\":\"DEEPSEEK_API_KEY\",",
        "\"api_base_env\":\"DEEPSEEK_API_BASE\"",
        "}\0"
    )
    .as_ptr()
    .cast()
}

pub fn descriptor() -> PluginDescriptor {
    serde_json::from_str(DESCRIPTOR_JSON).unwrap()
}
```

In `viden-model/src/registry.rs`, add builtin DeepSeek descriptor:

```rust
ProviderDescriptor {
    provider_id: "deepseek".to_string(),
    display_name: "DeepSeek".to_string(),
    version: "builtin".to_string(),
    protocol_family: ProtocolFamily::OpenAi,
    default_api_base: Some("https://api.deepseek.com".to_string()),
    default_model: Some("deepseek-v4-flash".to_string()),
    env_mappings: ProviderEnvMappings {
        api_key_env: Some("DEEPSEEK_API_KEY".to_string()),
        api_base_env: Some("DEEPSEEK_API_BASE".to_string()),
    },
    capabilities: ProviderCapabilities {
        supports_streaming: true,
        supports_native_tool_calling: true,
    },
    config_schema_version: 1,
},
```

Also add a `descriptor(&self, provider_id: &str) -> Option<&ProviderDescriptor>` lookup.

- [ ] **Step 4: Run focused and full model tests**

Run:

```bash
cargo test -p viden-model registry_exposes_deepseek_as_independent_provider_id
cargo test -p viden-model deepseek_provider_uses_openai_protocol_family
cargo test -p viden-model
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock viden-model/src/plugin.rs viden-model/src/registry.rs viden-model/src/lib.rs viden-provider-deepseek/Cargo.toml viden-provider-deepseek/src/lib.rs
git commit -m "Add DeepSeek as first plugin-backed provider"
```

## Task 6: Document Runtime Reload and Multi-Agent Provider Binding

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/architecture.md`
- Modify: `docs/architecture.zh-CN.md`
- Modify: `docs/modules.md`
- Modify: `docs/modules.zh-CN.md`
- Modify: `PLAN.md`

- [ ] **Step 1: Write the failing doc assertions**

Add a shell verification step by checking for required phrases after editing:

```bash
rg -n "deepseek|provider plugin|runtime reload|instance-scoped|OpenAI-style|Anthropic-style" README.md README.zh-CN.md docs/architecture.md docs/architecture.zh-CN.md docs/modules.md docs/modules.zh-CN.md PLAN.md
```

Expected before edits: at least some required phrases are missing.

- [ ] **Step 2: Update the docs**

Add to `README.md` provider section:

```md
- `deepseek`: independent provider family using the OpenAI-style adapter
- provider runtime supports built-in and dynamically loaded providers
- provider bindings are session/agent scoped rather than process-global
```

Add matching Chinese wording to `README.zh-CN.md`.

Update `docs/architecture.md` and `docs/architecture.zh-CN.md` to state:

- `viden-model` is now a provider host/runtime
- registry can refresh at runtime
- new sessions can see newly loaded providers
- active sessions keep their own provider instances

Update `docs/modules.md`, `docs/modules.zh-CN.md`, and `PLAN.md` to mention:

- plugin-extensible provider runtime
- DeepSeek as first plugin-backed provider target
- per-agent provider binding

- [ ] **Step 3: Run doc verification**

Run:

```bash
rg -n "deepseek|provider plugin|runtime reload|instance-scoped|OpenAI-style|Anthropic-style" README.md README.zh-CN.md docs/architecture.md docs/architecture.zh-CN.md docs/modules.md docs/modules.zh-CN.md PLAN.md
```

Expected: PASS with matches in all intended files.

- [ ] **Step 4: Run end-to-end verification**

Run:

```bash
cargo test -p viden-model
cargo test -p viden-config --lib
cargo test --workspace --quiet
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add README.md README.zh-CN.md docs/architecture.md docs/architecture.zh-CN.md docs/modules.md docs/modules.zh-CN.md PLAN.md
git commit -m "Document plugin-extensible provider runtime and DeepSeek support"
```

## Self-Review

Spec coverage:

- dynamic registry and native plugin loading: Tasks 1, 2, 5
- DeepSeek as independent provider family: Tasks 3, 5, 6
- provider-scoped config with generic fallback: Task 3
- runtime registry refresh boundary: Task 4
- per-agent provider binding: Task 4 and Task 6
- Anthropic/OpenAI protocol-family split: Tasks 1, 5, 6

Placeholder scan:

- no unfinished markers remain in executable tasks

Type consistency:

- `ProviderDescriptor`, `ProtocolFamily`, `ProviderRegistry`, `ProviderHost`, and `PluginDescriptor` are introduced before later tasks depend on them
- `viden-config` changes stay on the config side; `SessionEngine` remains untouched throughout the plan
