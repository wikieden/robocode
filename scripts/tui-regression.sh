#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-"$ROOT/docs/previews/generated"}"
SCREENSHOT_DIR="$OUT_DIR/screenshots"
VERSION="${ROBOCODE_TUI_SCREENSHOT_VERSION:-0.1.12}"

mkdir -p "$SCREENSHOT_DIR"

"$ROOT/scripts/tui-previews.sh" "$OUT_DIR"

copy_artifact() {
  local source="$1"
  local target="$2"
  if [[ -f "$source" ]]; then
    cp "$source" "$target"
  else
    printf 'tui regression failed: missing artifact %s\n' "$source" >&2
    exit 1
  fi
}

copy_artifact "$OUT_DIR/main.svg" "$SCREENSHOT_DIR/${VERSION}-tui-main.svg"
copy_artifact "$OUT_DIR/main-idle.svg" "$SCREENSHOT_DIR/${VERSION}-tui-main-idle.svg"
copy_artifact "$OUT_DIR/main-live-turn.svg" "$SCREENSHOT_DIR/${VERSION}-tui-live-turn.svg"
copy_artifact "$OUT_DIR/main-resize.svg" "$SCREENSHOT_DIR/${VERSION}-tui-main-resize.svg"
copy_artifact "$OUT_DIR/main-cjk-input.svg" "$SCREENSHOT_DIR/${VERSION}-tui-cjk-input.svg"
copy_artifact "$OUT_DIR/main-command-palette.svg" "$SCREENSHOT_DIR/${VERSION}-tui-command-palette.svg"
copy_artifact "$OUT_DIR/main-provider-selector.svg" "$SCREENSHOT_DIR/${VERSION}-tui-provider-selector.svg"
copy_artifact "$OUT_DIR/main-model-selector.svg" "$SCREENSHOT_DIR/${VERSION}-tui-model-selector.svg"
copy_artifact "$OUT_DIR/main-lane.svg" "$SCREENSHOT_DIR/${VERSION}-tui-lane-detail.svg"
copy_artifact "$OUT_DIR/side-1.svg" "$SCREENSHOT_DIR/${VERSION}-tui-side-1.svg"
copy_artifact "$OUT_DIR/side-2.svg" "$SCREENSHOT_DIR/${VERSION}-tui-side-2.svg"

python3 - "$OUT_DIR" "$SCREENSHOT_DIR" "$VERSION" <<'PY'
from pathlib import Path
import json
import sys

out_dir = Path(sys.argv[1])
screenshot_dir = Path(sys.argv[2])
version = sys.argv[3]
required = {
    "main": out_dir / "main.txt",
    "main_idle": out_dir / "main-idle.txt",
    "main_live_turn": out_dir / "main-live-turn.txt",
    "main_resize": out_dir / "main-resize.txt",
    "main_cjk_input": out_dir / "main-cjk-input.txt",
    "command_palette": out_dir / "main-command-palette.txt",
    "provider_selector": out_dir / "main-provider-selector.txt",
    "model_selector": out_dir / "main-model-selector.txt",
    "lane_detail": out_dir / "main-lane.txt",
    "side_1": out_dir / "side-1.txt",
    "side_2": out_dir / "side-2.txt",
    "multiscreen": out_dir / "multiscreen.txt",
}

def fail(message: str) -> None:
    raise SystemExit(f"tui regression failed: {message}")

for name, path in required.items():
    if not path.exists():
        fail(f"missing {name} preview at {path}")

main_lines = required["main"].read_text(encoding="utf-8").splitlines()
if len(main_lines) != 40:
    fail(f"main preview has {len(main_lines)} lines, expected 40")
if not any("NOW WORKING" in line for line in main_lines):
    fail("main preview does not show Now Working")
if not any("RoboCode >" in line for line in main_lines):
    fail("main preview does not show composer title")
if not any("› " in line for line in main_lines):
    fail("main preview does not show composer prompt")

main_live_turn = required["main_live_turn"].read_text(encoding="utf-8")
if "is thinking" not in main_live_turn or "live provider request" not in main_live_turn:
    fail("main live-turn preview does not show provider activity evidence")

main_resize = required["main_resize"].read_text(encoding="utf-8")
if "Resize-safe redraw check" not in main_resize or "NOW WORKING" not in main_resize:
    fail("resize preview does not show redraw/Now Working evidence")

main_cjk_input = required["main_cjk_input"].read_text(encoding="utf-8")
if "你好，帮我检查当前变更" not in main_cjk_input:
    fail("CJK input preview does not show Chinese composer input")

provider_selector = required["provider_selector"].read_text(encoding="utf-8")
if "SELECT PROVIDER" not in provider_selector or "DEEPSEEK_API_KEY" not in provider_selector:
    fail("provider selector preview does not show provider config evidence")

model_selector = required["model_selector"].read_text(encoding="utf-8")
if "SELECT MODEL" not in model_selector or "deepseek deepseek-v4-flash" not in model_selector:
    fail("model selector preview does not show grouped model evidence")

side_1 = required["side_1"].read_text(encoding="utf-8")
side_2 = required["side_2"].read_text(encoding="utf-8")
if "AGENT LANES" not in side_1:
    fail("side-1 missing agent lanes")
if "TESTS / LSP" not in side_2:
    fail("side-2 missing tests/lsp evidence")

artifacts = sorted(str(path) for path in screenshot_dir.glob(f"{version}-tui-*.svg"))
if len(artifacts) < 6:
    fail("expected at least six screenshot artifacts")

(out_dir / "tui-regression-evidence.json").write_text(
    json.dumps(
        {
            "status": "passed",
            "preview_dir": str(out_dir),
            "screenshots": artifacts,
            "checks": [
                "line counts",
                "Now Working",
                "composer visibility",
                "live provider turn evidence",
                "resize redraw evidence",
                "CJK composer input",
                "provider selector config evidence",
                "model selector grouping evidence",
                "side-1 lanes",
                "side-2 tests/lsp",
                "screenshot artifact export",
            ],
        },
        indent=2,
        ensure_ascii=False,
    )
    + "\n",
    encoding="utf-8",
)
PY

cat >"$SCREENSHOT_DIR/README.md" <<EOF
# ${VERSION} TUI Regression Screenshots

Generated by \`scripts/tui-regression.sh\`.

Each SVG is a deterministic visual artifact for product review:

- \`${VERSION}-tui-main.svg\`: active main cockpit
- \`${VERSION}-tui-main-idle.svg\`: idle main cockpit
- \`${VERSION}-tui-live-turn.svg\`: live provider request evidence
- \`${VERSION}-tui-main-resize.svg\`: resized 100x30 redraw evidence
- \`${VERSION}-tui-cjk-input.svg\`: CJK input and cursor-placement evidence
- \`${VERSION}-tui-command-palette.svg\`: slash-command suggestion surface
- \`${VERSION}-tui-provider-selector.svg\`: provider config selector evidence
- \`${VERSION}-tui-model-selector.svg\`: provider-grouped model selector evidence
- \`${VERSION}-tui-lane-detail.svg\`: focused lane detail
- \`${VERSION}-tui-side-1.svg\`: lane side screen
- \`${VERSION}-tui-side-2.svg\`: ops/test side screen

Structured evidence: \`../tui-regression-evidence.json\`
EOF

printf '%s\n' "$OUT_DIR/tui-regression-evidence.json"
