#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if (( $# > 0 )); then
  manifests=("$@")
else
  manifests=("$ROOT/apps/tui/Cargo.toml")
  if [[ -f "$ROOT/apps/gui/Cargo.toml" ]]; then
    manifests+=("$ROOT/apps/gui/Cargo.toml")
  fi
fi

python3 - "${manifests[@]}" <<'PY'
import re
import sys
from pathlib import Path

FORBIDDEN = {
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
