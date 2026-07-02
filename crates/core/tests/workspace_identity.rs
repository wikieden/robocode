use std::fs;
use std::path::Path;

fn workspace_root() -> &'static Path {
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    loop {
        let manifest = dir.join("Cargo.toml");
        if fs::read_to_string(&manifest)
            .map(|content| content.contains("[workspace]"))
            .unwrap_or(false)
        {
            return dir;
        }
        dir = dir
            .parent()
            .expect("viden-core must live below the workspace root");
    }
}

fn package_name(manifest: &Path) -> Option<String> {
    fs::read_to_string(manifest)
        .ok()?
        .lines()
        .find_map(|line| line.trim().strip_prefix("name = "))
        .and_then(|value| value.trim().trim_matches('"').split('"').next())
        .map(str::to_string)
}

fn workspace_members(root: &Path) -> Vec<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read workspace manifest");

    let mut inside_members = false;
    let mut members = Vec::new();
    for raw in manifest.lines() {
        let line = raw.trim();
        if line == "members = [" {
            inside_members = true;
            continue;
        }
        if inside_members && line == "]" {
            break;
        }
        if inside_members {
            let member = line.trim_end_matches(',').trim_matches('"');
            if !member.is_empty() {
                members.push(member.to_string());
            }
        }
    }
    members
}

#[test]
fn workspace_crate_names_use_viden_identity() {
    let root = workspace_root();
    let members = workspace_members(root);

    let legacy_members: Vec<_> = members
        .iter()
        .filter(|member| member.starts_with("robocode"))
        .cloned()
        .collect();
    assert!(
        legacy_members.is_empty(),
        "workspace member paths still use legacy RoboCode names: {legacy_members:?}"
    );

    let legacy_packages: Vec<_> = members
        .iter()
        .filter_map(|member| package_name(&root.join(member).join("Cargo.toml")))
        .filter(|name| name.starts_with("robocode"))
        .collect();
    assert!(
        legacy_packages.is_empty(),
        "workspace package names still use legacy RoboCode names: {legacy_packages:?}"
    );
}

#[test]
fn workspace_uses_apps_crates_plugins_layout() {
    let root = workspace_root();
    for directory in ["apps", "crates", "plugins"] {
        assert!(
            root.join(directory).is_dir(),
            "workspace should expose a top-level `{directory}/` directory"
        );
    }

    let members = workspace_members(root);
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read workspace manifest");
    for member in [
        "apps/cli",
        "apps/tui",
        "crates/core",
        "crates/runtime",
        "crates/plugin-api",
        "crates/plugin-host",
        "plugins/providers/deepseek",
    ] {
        assert!(
            manifest.contains(&format!("\"{member}\"")),
            "workspace manifest should include `{member}`"
        );
    }

    let product_named_dirs: Vec<_> = members
        .iter()
        .filter(|member| {
            member
                .split('/')
                .any(|part| part.starts_with("viden") || part.starts_with("robocode"))
        })
        .cloned()
        .collect();
    assert!(
        product_named_dirs.is_empty(),
        "workspace member directories should use product-neutral names: {product_named_dirs:?}"
    );
}
