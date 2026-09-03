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

# Both boundary checks -- the frontend forbidden-list and the per-leaf
# allow-list -- read manifests through one parser, so a Cargo syntax the parser
# does not understand cannot be closed in one check and left open in the other.
python3 - "$ROOT" "${manifests[@]}" <<'PY'
import re
import sys
from pathlib import Path

DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def split_section(header):
    """Split a section header on dots that are outside quotes.

    `target.'cfg(windows)'.dependencies` must split into three segments, not
    into pieces of the cfg expression, so quoted spans are copied verbatim.
    """
    parts = []
    current = ""
    quote = None
    for character in header:
        if quote is not None:
            current += character
            if character == quote:
                quote = None
        elif character in "\"'":
            quote = character
            current += character
        elif character == ".":
            parts.append(current)
            current = ""
        else:
            current += character
    parts.append(current)
    return [part.strip() for part in parts]


def unquote(value):
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


def classify_section(header):
    """Describe what a section header declares.

    Returns (kind, table, dependency):

    - ("inline", <table>, None) for `[dependencies]` and
      `[target.'cfg(..)'.dev-dependencies]`, whose entries are `name = ...`
      lines inside the table;
    - ("dotted", <table>, <name>) for `[dependencies.viden-provider]`, which is
      equally valid Cargo and declares that dependency by its section name
      alone. Matching only the table names skipped these sections entirely, so
      a dotted table was an unchecked way to declare any dependency.
    """
    segments = split_section(header)
    if not segments:
        return (None, None, None)
    if unquote(segments[-1]) in DEPENDENCY_TABLES:
        return ("inline", header, None)
    if len(segments) >= 2 and unquote(segments[-2]) in DEPENDENCY_TABLES:
        return ("dotted", ".".join(segments[:-1]), unquote(segments[-1]))
    return (None, None, None)


def read_sections(text):
    """Group a manifest into (header, header_line, body) sections."""
    sections = []
    header = ""
    header_line = 0
    body = []
    for line_number, raw_line in enumerate(text.splitlines(), start=1):
        line = raw_line.split("#", 1)[0].strip()
        if line.startswith("[") and line.endswith("]"):
            sections.append((header, header_line, body))
            header = line[1:-1].strip()
            header_line = line_number
            body = []
            continue
        body.append((line_number, line))
    sections.append((header, header_line, body))
    return sections


ENTRY = re.compile(r'''(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))\s*=\s*(.*)''')
PACKAGE = re.compile(r'''\bpackage\s*=\s*["']([^"']+)["']''')


def manifest_dependencies(manifest):
    """Yield (line_number, table, key, package) for every declared dependency.

    `package` is the crate actually depended on, honoring a `package = "..."`
    rename in either the inline or the dotted form.
    """
    found = []
    for header, header_line, body in read_sections(
        manifest.read_text(encoding="utf-8")
    ):
        kind, table, dotted_name = classify_section(header)
        if kind is None:
            continue
        if kind == "dotted":
            package = dotted_name
            for _, line in body:
                match = PACKAGE.search(line)
                if match:
                    package = match.group(1)
                    break
            found.append((header_line, table, dotted_name, package))
            continue
        lines = iter(body)
        for line_number, line in lines:
            if not line:
                continue
            match = ENTRY.match(line)
            if not match:
                continue
            key = next(group for group in match.groups()[:3] if group is not None)
            specification = match.group(4)
            while specification.count("{") > specification.count("}"):
                try:
                    _, continuation = next(lines)
                except StopIteration:
                    break
                specification += " " + continuation
            package_match = PACKAGE.search(specification)
            package = package_match.group(1) if package_match else key
            found.append((line_number, table, key, package))
    return found


failures = []

# --- Frontend boundary -------------------------------------------------------
# A frontend owns presentation state and reaches business state only through
# Core. viden-agents is on the list because a client must not drive an ACP or
# Codex adapter itself.
FORBIDDEN = {
    "viden-agents",
    "viden-context",
    "viden-provider",
    "viden-runtime",
    "viden-tools",
    "viden-workflows",
}

for raw_manifest in sys.argv[2:]:
    manifest = Path(raw_manifest)
    if not manifest.is_file():
        failures.append(f"missing frontend manifest: {manifest}")
        continue
    try:
        declared = manifest_dependencies(manifest)
    except OSError as error:
        failures.append(f"cannot read frontend manifest {manifest}: {error}")
        continue
    for line_number, table, key, package in declared:
        if key in FORBIDDEN or package in FORBIDDEN:
            name = key if key in FORBIDDEN else package
            failures.append(
                f"forbidden frontend dependency in {manifest}:{line_number}: "
                f"{table}.{key} -> {name}"
            )

# --- Leaf boundaries ---------------------------------------------------------
# Each leaf extracted from below the runtime keeps an enumerated allow-list
# rather than a list of today's offenders, because the property worth holding
# is "nothing the runtime owns leaked back in", not "these five names stayed
# out". A leaf receives runtime policy -- sessions, providers, config, context,
# plugins, event sinks, permission contexts -- as injected parameters.
#
# Each leaf also names the modules it took with it. The extraction is only real
# while those modules stay extracted, so a reappearing path under
# crates/runtime/src fails the gate even if it compiles.
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
        # Directories are rejected as well as files: the pre-extraction tree
        # kept these bands in crates/runtime/src/agents/.
        "stale": (
            "agent_commands.rs",
            "agents",
        ),
    },
}

for crate, leaf in LEAVES.items():
    label = leaf["label"]
    allowed = leaf["allowed"]
    manifest = root / "crates" / crate / "Cargo.toml"
    if not manifest.is_file():
        failures.append(f"missing {label} manifest: {manifest}")
    else:
        for line_number, table, key, package in manifest_dependencies(manifest):
            # A rename is checked by the crate it resolves to, not by the local
            # alias, so `[dependencies.anything] package = "viden-runtime"` is
            # caught the same as a plain entry.
            if package.startswith("viden-") and package not in allowed:
                failures.append(
                    f"forbidden {label} dependency in {manifest}:{line_number}: "
                    f"{table}.{key} -> {package}"
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
