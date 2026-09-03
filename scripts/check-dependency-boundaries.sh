#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if (( $# > 0 )); then
  manifests=("$@")
else
  manifests=("$ROOT/apps/tui/Cargo.toml")
  # The GUI crate is the Tauri sub-manifest. Probing apps/gui/Cargo.toml -- a
  # path that has never existed under the Tauri layout -- silently skipped the
  # frontend check for the whole GUI, so both locations are tried and a total
  # absence is a failure rather than a skip.
  gui_found=""
  for candidate in "$ROOT/apps/gui/src-tauri/Cargo.toml" "$ROOT/apps/gui/Cargo.toml"; do
    if [[ -f "$candidate" ]]; then
      manifests+=("$candidate")
      gui_found="yes"
      break
    fi
  done
  if [[ -z "$gui_found" ]]; then
    echo "no GUI manifest found under apps/gui; frontend boundary unchecked" >&2
    exit 1
  fi
fi

python3 - "${manifests[@]}" <<'PY'
import re
import sys
from pathlib import Path

FORBIDDEN = {
    # A frontend reaches external agents only through Core, never by driving
    # an ACP or Codex adapter itself.
    "viden-agents",
    "viden-context",
    "viden-provider",
    "viden-runtime",
    "viden-tools",
    "viden-workflows",
}
failures = []


def dependency_table(section):
    return any(
        section == name or section.endswith(f".{name}")
        for name in ("dependencies", "dev-dependencies", "build-dependencies")
    )


def inspect_manifest(manifest, text):
    section = ""
    lines = iter(enumerate(text.splitlines(), start=1))
    for line_number, raw_line in lines:
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1].strip()
            continue
        if not dependency_table(section):
            continue

        match = re.match(r'''(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))\s*=\s*(.*)''', line)
        if not match:
            continue
        dependency = next(group for group in match.groups()[:3] if group is not None)
        specification = match.group(4)
        while specification.count("{") > specification.count("}"):
            try:
                _, continuation = next(lines)
            except StopIteration:
                break
            specification += " " + continuation.split("#", 1)[0].strip()

        package_match = re.search(r'''\bpackage\s*=\s*["']([^"']+)["']''', specification)
        package = package_match.group(1) if package_match else None
        forbidden_name = dependency if dependency in FORBIDDEN else package
        if forbidden_name in FORBIDDEN:
            failures.append(
                f"forbidden frontend dependency in {manifest}:{line_number}: "
                f"{section}.{dependency} -> {forbidden_name}"
            )


for raw_manifest in sys.argv[1:]:
    manifest = Path(raw_manifest)
    if not manifest.is_file():
        failures.append(f"missing frontend manifest: {manifest}")
        continue
    try:
        inspect_manifest(manifest, manifest.read_text(encoding="utf-8"))
    except OSError as error:
        failures.append(f"cannot read frontend manifest {manifest}: {error}")

if failures:
    print("\n".join(failures), file=sys.stderr)
    raise SystemExit(1)
PY

# Each leaf extracted from below the runtime keeps an enumerated allow-list
# rather than a list of today's offenders, because the property worth holding
# is "nothing the runtime owns leaked back in", not "these five names stayed
# out". A leaf receives runtime policy -- sessions, providers, config, context,
# plugins, event sinks, permission contexts -- as injected parameters.
#
# Each leaf also names the modules it took with it. The extraction is only real
# while those modules stay extracted, so a reappearing path under
# crates/runtime/src fails the gate even if it compiles.
python3 - "$ROOT" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])

LEAVES = {
    # Lane lifecycle orchestration: reaches the OS only through viden-tools
    # backends (worktree, process, terminal, patch).
    "lanes": {
        "label": "lane",
        "allowed": {
            "viden-permissions",
            "viden-tools",
            "viden-types",
            "viden-workflows",
            # The crate may depend on itself to re-expose its `testing` helpers.
            "viden-lanes",
        },
        # Directories are rejected as well as files: the pre-extraction tree
        # kept these bands in crates/runtime/src/agents/.
        "stale": (
            "lane_runtime.rs",
            "lane_supervisor.rs",
            "lane_worker.rs",
        ),
    },
    # External agent adapters: one strategy per external CLI (ACP generic
    # client, Codex app-server), reaching the OS only through viden-tools
    # capabilities. It must not learn about sessions, lanes, providers,
    # frontends, or the runtime's trust loop.
    "agents": {
        "label": "agent",
        "allowed": {
            "viden-permissions",
            "viden-plugin-api",
            "viden-plugin-host",
            "viden-tools",
            "viden-types",
            "viden-agents",
        },
        "stale": (
            "agent_commands.rs",
            "agents",
        ),
    },
}

failures = []


def dependency_table(section):
    return any(
        section == name or section.endswith(f".{name}")
        for name in ("dependencies", "dev-dependencies", "build-dependencies")
    )


for crate, leaf in LEAVES.items():
    label = leaf["label"]
    allowed = leaf["allowed"]
    manifest = root / "crates" / crate / "Cargo.toml"
    if not manifest.is_file():
        failures.append(f"missing {label} manifest: {manifest}")
    else:
        section = ""
        lines = iter(enumerate(manifest.read_text(encoding="utf-8").splitlines(), start=1))
        for line_number, raw_line in lines:
            line = raw_line.split("#", 1)[0].strip()
            if not line:
                continue
            if line.startswith("[") and line.endswith("]"):
                section = line[1:-1].strip()
                continue
            if not dependency_table(section):
                continue
            match = re.match(r'''(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))\s*=\s*(.*)''', line)
            if not match:
                continue
            dependency = next(group for group in match.groups()[:3] if group is not None)
            specification = match.group(4)
            while specification.count("{") > specification.count("}"):
                try:
                    _, continuation = next(lines)
                except StopIteration:
                    break
                specification += " " + continuation.split("#", 1)[0].strip()
            package_match = re.search(r'''\bpackage\s*=\s*["']([^"']+)["']''', specification)
            name = package_match.group(1) if package_match else dependency
            if name.startswith("viden-") and name not in allowed:
                failures.append(
                    f"forbidden {label} dependency in {manifest}:{line_number}: "
                    f"{section}.{dependency} -> {name}"
                )

    for module in leaf["stale"]:
        stale = root / "crates" / "runtime" / "src" / module
        if stale.exists():
            failures.append(
                f"{label} module belongs to viden-{crate}, not viden-runtime: {stale}"
            )

if failures:
    print("\n".join(failures), file=sys.stderr)
    raise SystemExit(1)
PY
