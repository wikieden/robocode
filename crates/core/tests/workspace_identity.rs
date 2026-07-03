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

#[test]
fn release_install_docs_use_viden_artifact_identity() {
    let root = workspace_root();
    let checked_files = [
        "README.md",
        "README.zh-CN.md",
        "scripts/package-release.sh",
        "scripts/release-smoke.sh",
    ];

    for file in checked_files {
        let content = fs::read_to_string(root.join(file)).expect("read checked release file");
        assert!(
            !content.contains("robocode-v"),
            "{file} should not reference legacy robocode-v release archives"
        );
        assert!(
            !content.contains("RoboCode v"),
            "{file} should not label release archives with the legacy RoboCode product name"
        );
    }

    let package_script =
        fs::read_to_string(root.join("scripts/package-release.sh")).expect("read package script");
    let cli_manifest =
        fs::read_to_string(root.join("apps/cli/Cargo.toml")).expect("read cli manifest");
    assert!(
        cli_manifest.contains("[[bin]]") && cli_manifest.contains("name = \"viden\""),
        "apps/cli should expose the installed terminal binary as `viden`"
    );
    assert!(
        package_script.contains("ARCHIVE_NAME=\"viden-v${VERSION}-${TARGET}\""),
        "package-release.sh should create viden-v release archives"
    );
    assert!(
        package_script.contains("BIN_NAME=\"viden\""),
        "package-release.sh should ship the user-facing `viden` terminal binary"
    );

    for file in [
        "README.md",
        "README.zh-CN.md",
        "docs/user-guide.md",
        "docs/user-guide.zh-CN.md",
    ] {
        let content = fs::read_to_string(root.join(file)).expect("read user-facing docs");
        for bad_snippet in [
            "\nviden-cli",
            "\nviden-cli.exe",
            "viden-cli --help",
            "viden-cli.exe --help",
        ] {
            assert!(
                !content.contains(bad_snippet),
                "{file} should use the user-facing `viden` command in shell examples"
            );
        }
        assert!(
            !content.contains("/viden-cli"),
            "{file} should not install a `viden-cli` binary"
        );
        assert!(
            !content.contains("viden-v-/"),
            "{file} should preserve VERSION and TARGET variables in release archive paths"
        );
    }
}

#[test]
fn tui_source_lives_in_tui_app_not_cli_app() {
    let root = workspace_root();
    assert!(
        root.join("apps/tui/src/tui.rs").is_file(),
        "apps/tui should own the TUI module entrypoint"
    );
    assert!(
        root.join("apps/tui/src/tui/app.rs").is_file(),
        "apps/tui should own the TUI implementation files"
    );
    assert!(
        !root.join("apps/cli/src/tui.rs").exists(),
        "apps/cli should not keep the TUI module entrypoint"
    );
    assert!(
        !root.join("apps/cli/src/tui").exists(),
        "apps/cli should not keep the TUI implementation directory"
    );
}

#[test]
fn smoke_scripts_run_tui_tests_from_tui_package() {
    let root = workspace_root();
    for file in [
        "scripts/release-smoke.sh",
        "scripts/rc-tui-stability-smoke.sh",
        "scripts/tui-turn-controller-smoke.sh",
        "scripts/smoke-lane-operator-loop.sh",
    ] {
        let content = fs::read_to_string(root.join(file)).expect("read smoke script");
        assert!(
            !content.contains("cargo test -p viden-cli tui::"),
            "{file} should not run migrated TUI tests from viden-cli"
        );
        assert!(
            content.contains("cargo test -p viden-tui"),
            "{file} should run migrated TUI tests from viden-tui"
        );
    }
}
