#!/usr/bin/env bash
set -euo pipefail

if (( $# == 0 )); then
  printf 'usage: %s <markdown-path> [...]\n' "$0" >&2
  exit 2
fi

python3 - "$@" <<'PY'
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

link_pattern = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
failures = []

for raw_document in sys.argv[1:]:
    document = Path(raw_document)
    if not document.is_file():
        failures.append(f"missing Markdown document: {document}")
        continue

    text = document.read_text(encoding="utf-8")
    for line_number, line in enumerate(text.splitlines(), start=1):
        for match in link_pattern.finditer(line):
            raw_target = match.group(1).strip()
            if raw_target.startswith("<") and ">" in raw_target:
                target = raw_target[1:raw_target.index(">")]
            else:
                target = raw_target.split(maxsplit=1)[0]

            parsed = urlsplit(target)
            if not parsed.path or parsed.scheme or parsed.netloc or parsed.path.startswith("/"):
                continue

            resolved = document.parent / unquote(parsed.path)
            if not resolved.exists():
                failures.append(
                    f"broken local link: {document}:{line_number}: {target} -> {resolved}"
                )

if failures:
    print("\n".join(failures), file=sys.stderr)
    raise SystemExit(1)
PY
