#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
GUI_ROOT="$ROOT/apps/gui"
EVIDENCE_ROOT="$GUI_ROOT/evidence/0.1.0-rc.3"
URL="http://127.0.0.1:4173/evidence/0.1.0-rc.3/d1-canonical-qa.html"
HARNESS_URL="http://127.0.0.1:4173/evidence/0.1.0-rc.3/d1-target-viewport-capture.html"

cat <<EOF
Viden D1 rc.3 browser-control capture helper

Prerequisites:
- run from the repository worktree that owns apps/gui/**
- start the local preview:
  npm --prefix apps/gui run dev -- --host 127.0.0.1 --port 4173 --strictPort
- capture through the authorized Browser runtime, not Playwright CLI

Canonical URL:
  $URL

Exact viewport harness:
  $HARNESS_URL

Required same-state captures:
- $EVIDENCE_ROOT/d1-design-reference-canonical.png
  viewport: 5140x2650 nested px, URL: $HARNESS_URL?source=design&width=5140&height=2650
- $EVIDENCE_ROOT/d1-main-dark.png
  viewport: 5140x2650 nested px, URL: $HARNESS_URL?width=5140&height=2650
- $EVIDENCE_ROOT/d1-responsive-1280x800-dark.png
  viewport: 1280x800 clipped nested px, URL: $HARNESS_URL?width=1280&height=800
- $EVIDENCE_ROOT/d1-responsive-960x640-dark-drawer.png
  viewport: 960x640 clipped nested px, URL: $HARNESS_URL?width=960&height=640&drawer=open
- $EVIDENCE_ROOT/d1-context-dock-bottom-1280x800.png
  viewport: 1280x800 clipped nested px, URL: $HARNESS_URL?width=1280&height=800&contextScroll=bottom
- $EVIDENCE_ROOT/d1-main-light.png
  viewport: 5140x2650 nested px, URL: $HARNESS_URL?mode=light&width=5140&height=2650
- $EVIDENCE_ROOT/d1-main-zh-CN.png
  viewport: 5140x2650 nested px, URL: $HARNESS_URL?locale=zh-CN&width=5140&height=2650
- $EVIDENCE_ROOT/d1-compact-readable.png
  viewport: 1280x800 clipped nested px, URL: $HARNESS_URL?density=compact&width=1280&height=800
- $EVIDENCE_ROOT/d1-design-reference-vs-actual.png
  side-by-side: design reference left, production actual right

Limitations:
- this helper standardizes URLs, dimensions, and output paths only
- it intentionally does not invoke browser automation outside the authorized Browser controller
EOF
