#!/usr/bin/env python3
"""Bind framework-gate test execution to the committed canonical D1 fixture."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: verify-framework-fixture.py <candidate.json> <canonical.json>", file=sys.stderr)
        return 2

    candidate_path, canonical_path = map(Path, sys.argv[1:])
    try:
        candidate_bytes = candidate_path.read_bytes()
        canonical_bytes = canonical_path.read_bytes()
        candidate = json.loads(candidate_bytes)
        canonical = json.loads(canonical_bytes)
    except (OSError, json.JSONDecodeError) as error:
        print(f"could not read framework fixture: {error}", file=sys.stderr)
        return 2

    identity_fields = ("fixture_id", "schema_version", "expected_view_sha256")
    identity_matches = all(candidate.get(field) == canonical.get(field) for field in identity_fields)
    if not identity_matches or candidate_bytes != canonical_bytes:
        print("candidate does not match the canonical fixture identity and digest", file=sys.stderr)
        return 1

    digest = hashlib.sha256(candidate_bytes).hexdigest()
    print(
        f"verified fixture_id={candidate['fixture_id']} sha256={digest} "
        f"projection_sha256={candidate['expected_view_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
