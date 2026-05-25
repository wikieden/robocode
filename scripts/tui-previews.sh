#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-"$ROOT/docs/previews/generated"}"
THEME="${ROBOCODE_TUI_PREVIEW_THEME:-aurora-cyan}"
THEMES="${ROBOCODE_TUI_PREVIEW_THEMES:-aurora-cyan ember-gold plasma-violet monochrome-ice}"
PROVIDER="${ROBOCODE_TUI_PREVIEW_PROVIDER:-openai}"
MODEL="${ROBOCODE_TUI_PREVIEW_MODEL:-gpt-4o}"

mkdir -p "$OUT_DIR"

run_preview() {
  local name="$1"
  shift
  cargo run -p robocode-cli -- "$@" --provider "$PROVIDER" --model "$MODEL" >"$OUT_DIR/$name.txt"
}

run_ansi_preview() {
  local name="$1"
  shift
  cargo run -p robocode-cli -- "$@" --tui-theme "$THEME" --provider "$PROVIDER" --model "$MODEL" >"$OUT_DIR/$name.ansi"
}

render_svg_preview() {
  local source="$1"
  local target="$2"
  python3 - "$source" "$target" <<'PY'
from html import escape
from pathlib import Path
import re
import sys

source = Path(sys.argv[1])
target = Path(sys.argv[2])
raw_lines = source.read_text(encoding="utf-8").splitlines()
ansi_re = re.compile(r"\x1b\[([0-9;]*)m")
default_fg = "#c4e0ff"
default_bg = None

def rgb(parts, offset):
    return f"#{int(parts[offset]):02x}{int(parts[offset + 1]):02x}{int(parts[offset + 2]):02x}"

def apply_sgr(state, params):
    if params == [""] or "0" in params:
        state["fg"] = default_fg
        state["bg"] = default_bg
    index = 0
    while index < len(params):
        code = params[index]
        if code == "":
            index += 1
            continue
        value = int(code)
        if value == 39:
            state["fg"] = default_fg
        elif value == 49:
            state["bg"] = default_bg
        elif value == 38 and index + 4 < len(params) and params[index + 1] == "2":
            state["fg"] = rgb(params, index + 2)
            index += 4
        elif value == 48 and index + 4 < len(params) and params[index + 1] == "2":
            state["bg"] = rgb(params, index + 2)
            index += 4
        index += 1

def parse_ansi_line(line):
    state = {"fg": default_fg, "bg": default_bg}
    spans = []
    cursor = 0
    for match in ansi_re.finditer(line):
        if match.start() > cursor:
            spans.append((line[cursor:match.start()], state["fg"], state["bg"]))
        apply_sgr(state, match.group(1).split(";"))
        cursor = match.end()
    if cursor < len(line):
        spans.append((line[cursor:], state["fg"], state["bg"]))
    return spans

lines = [parse_ansi_line(line) for line in raw_lines]
plain_lines = ["".join(text for text, _fg, _bg in line) for line in lines]
cell_w = 10
cell_h = 19
pad_x = 24
pad_y = 30
width = pad_x * 2 + max(len(line) for line in plain_lines) * cell_w
height = pad_y * 2 + len(lines) * cell_h
rows = []
for index, line in enumerate(lines):
    y = pad_y + index * cell_h
    col = 0
    for text, fg, bg in line:
        if not text:
            continue
        x = pad_x + col * cell_w
        if bg:
            rows.append(
                f'<rect x="{x}" y="{y - 15}" width="{len(text) * cell_w}" height="{cell_h}" fill="{bg}" opacity="0.82"/>'
            )
        rows.append(f'<text x="{x}" y="{y}" class="term" fill="{fg}">{escape(text)}</text>')
        col += len(text)
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <defs>
    <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="2.5" result="blur"/>
      <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
  </defs>
  <rect width="100%" height="100%" fill="#030c16"/>
  <rect x="8" y="8" width="{width - 16}" height="{height - 16}" fill="none" stroke="#1c5f84" stroke-width="2" filter="url(#glow)"/>
  <rect x="15" y="15" width="{width - 30}" height="{height - 30}" fill="none" stroke="#08233a" stroke-width="1"/>
  <style>
    .term {{ font-family: Menlo, Monaco, Consolas, monospace; font-size: 16px; white-space: pre; dominant-baseline: alphabetic; }}
  </style>
  {''.join(rows)}
</svg>
'''
target.write_text(svg, encoding="utf-8")
PY
}

cd "$ROOT"

run_preview main --tui-preview
run_preview main-idle --tui-preview-idle
run_preview main-command-palette --tui-preview-command-palette
run_preview main-lane --tui-preview-lane
run_preview side-1 --tui-preview-side
run_preview side-2 --tui-preview-side-2

run_ansi_preview main --tui-preview-ansi
run_ansi_preview main-idle --tui-preview-idle-ansi
run_ansi_preview main-command-palette --tui-preview-command-palette-ansi
run_ansi_preview main-lane --tui-preview-lane-ansi
run_ansi_preview side-1 --tui-preview-side-ansi
run_ansi_preview side-2 --tui-preview-side-2-ansi

for theme_name in $THEMES; do
  cargo run -p robocode-cli -- --tui-preview-ansi --tui-theme "$theme_name" --provider "$PROVIDER" --model "$MODEL" >"$OUT_DIR/main.$theme_name.ansi"
done

render_svg_preview "$OUT_DIR/main.ansi" "$OUT_DIR/main.svg"
render_svg_preview "$OUT_DIR/main-idle.ansi" "$OUT_DIR/main-idle.svg"
render_svg_preview "$OUT_DIR/main-command-palette.ansi" "$OUT_DIR/main-command-palette.svg"
render_svg_preview "$OUT_DIR/main-lane.ansi" "$OUT_DIR/main-lane.svg"

assert_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -Fq "$needle" "$file"; then
    printf 'preview check failed: %s missing %s\n' "$file" "$needle" >&2
    exit 1
  fi
}

assert_ansi_truecolor() {
  local file="$1"
  if ! grep -q $'\033\\[38;2;' "$file"; then
    printf 'preview check failed: %s missing truecolor foreground sequences\n' "$file" >&2
    exit 1
  fi
  if ! grep -q $'\033\\[48;2;' "$file"; then
    printf 'preview check failed: %s missing truecolor background sequences\n' "$file" >&2
    exit 1
  fi
}

assert_ansi_contains() {
  local file="$1"
  local needle="$2"
  NEEDLE="$needle" perl -0ne '
    s/\e\[[0-9;]*m//g;
    exit(index($_, $ENV{"NEEDLE"}) >= 0 ? 0 : 1);
  ' "$file" || {
    printf 'preview check failed: %s missing ANSI text %s\n' "$file" "$needle" >&2
    exit 1
  }
}

assert_line_count() {
  local file="$1"
  local expected="$2"
  local actual
  actual="$(wc -l <"$file" | tr -d ' ')"
  if [[ "$actual" != "$expected" ]]; then
    printf 'preview check failed: %s has %s lines, expected %s\n' "$file" "$actual" "$expected" >&2
    exit 1
  fi
}

assert_balanced_chips() {
  local file="$1"
  awk '
    {
      left = gsub(/\[/, "[")
      right = gsub(/\]/, "]")
      if (left != right) {
        printf "preview check failed: %s:%d has unbalanced chips: %s\n", FILENAME, NR, $0 > "/dev/stderr"
        exit 1
      }
    }
  ' "$file"
}

assert_char_width() {
  local file="$1"
  local expected="$2"
  local start_line="${3:-1}"
  EXPECTED_WIDTH="$expected" START_LINE="$start_line" perl -CSDA -Mutf8 -ne '
    chomp;
    next if $. < $ENV{"START_LINE"};
    my $length = length($_);
    if ($length != $ENV{"EXPECTED_WIDTH"}) {
      print STDERR "preview check failed: $ARGV:$.: width $length, expected $ENV{EXPECTED_WIDTH}: $_\n";
      exit 1;
    }
  ' "$file"
}

{
  printf 'MAIN 140x40'
  printf '%130s' ''
  printf '    SIDE-1 80x40'
  printf '%67s' ''
  printf '    SIDE-2 80x40\n'
  paste "$OUT_DIR/main.txt" "$OUT_DIR/side-1.txt" "$OUT_DIR/side-2.txt" | sed $'s/\t/    /g'
} >"$OUT_DIR/multiscreen.txt"

assert_line_count "$OUT_DIR/main.txt" 40
assert_line_count "$OUT_DIR/main-idle.txt" 40
assert_line_count "$OUT_DIR/main-command-palette.txt" 40
assert_line_count "$OUT_DIR/main-lane.txt" 40
assert_line_count "$OUT_DIR/side-1.txt" 40
assert_line_count "$OUT_DIR/side-2.txt" 40
assert_line_count "$OUT_DIR/multiscreen.txt" 41
assert_char_width "$OUT_DIR/main.txt" 140
assert_char_width "$OUT_DIR/main-idle.txt" 140
assert_char_width "$OUT_DIR/main-command-palette.txt" 140
assert_char_width "$OUT_DIR/main-lane.txt" 140
assert_char_width "$OUT_DIR/side-1.txt" 80
assert_char_width "$OUT_DIR/side-2.txt" 80
assert_char_width "$OUT_DIR/multiscreen.txt" 308 2
for preview_file in \
  "$OUT_DIR/main.txt" \
  "$OUT_DIR/main-idle.txt" \
  "$OUT_DIR/main-command-palette.txt" \
  "$OUT_DIR/main-lane.txt" \
  "$OUT_DIR/side-1.txt" \
  "$OUT_DIR/side-2.txt" \
  "$OUT_DIR/multiscreen.txt"; do
  assert_balanced_chips "$preview_file"
done
assert_contains "$OUT_DIR/multiscreen.txt" "MAIN 140x40"
assert_contains "$OUT_DIR/multiscreen.txt" "SIDE-1 80x40"
assert_contains "$OUT_DIR/multiscreen.txt" "SIDE-2 80x40"
assert_contains "$OUT_DIR/multiscreen.txt" "APPROVAL REQUIRED"
assert_contains "$OUT_DIR/main-idle.txt" "No approval is blocking right now"
assert_contains "$OUT_DIR/main-command-palette.txt" "COMMANDS"
assert_contains "$OUT_DIR/main-command-palette.txt" "› /git push origin release/v0.1.3"
assert_contains "$OUT_DIR/main-command-palette.txt" "Remote branch origin/release/v0.1.3"
assert_contains "$OUT_DIR/main.txt" "diagnostics unavailable"
assert_contains "$OUT_DIR/main.txt" "TELEMETRY"
assert_contains "$OUT_DIR/main.txt" "EVENTS"
assert_contains "$OUT_DIR/main.txt" "LANES"
if grep -Fq "APPROVAL REQUIRED" "$OUT_DIR/main-idle.txt"; then
  printf 'preview check failed: %s should not contain approval modal\n' "$OUT_DIR/main-idle.txt" >&2
  exit 1
fi
assert_contains "$OUT_DIR/multiscreen.txt" "AGENT LANES"
assert_contains "$OUT_DIR/multiscreen.txt" "LSP / BUILD"
assert_contains "$OUT_DIR/multiscreen.txt" "pty/01"
assert_contains "$OUT_DIR/multiscreen.txt" "diagnostics unavailable"
assert_contains "$OUT_DIR/main-lane.txt" "LANE DETAIL"
assert_contains "$OUT_DIR/main-lane.txt" "ROUTE main→side-1"
assert_contains "$OUT_DIR/main-lane.txt" "CMD    codex exec test fixes"
assert_contains "$OUT_DIR/main-lane.txt" "CONTROL [stop] [view] [route] [side-2]"
for ansi_file in \
  "$OUT_DIR/main.ansi" \
  "$OUT_DIR/main-idle.ansi" \
  "$OUT_DIR/main-command-palette.ansi" \
  "$OUT_DIR/main-lane.ansi" \
  "$OUT_DIR/side-1.ansi" \
  "$OUT_DIR/side-2.ansi"; do
  assert_ansi_truecolor "$ansi_file"
done
assert_ansi_contains "$OUT_DIR/main.ansi" "APPROVAL REQUIRED"
assert_ansi_contains "$OUT_DIR/main-idle.ansi" "No approval is blocking right now"
assert_ansi_contains "$OUT_DIR/main-command-palette.ansi" "COMMANDS"
assert_ansi_contains "$OUT_DIR/main-lane.ansi" "LANE DETAIL"
assert_ansi_contains "$OUT_DIR/side-1.ansi" "AGENT LANES"
assert_ansi_contains "$OUT_DIR/side-2.ansi" "LSP / BUILD"

cat >"$OUT_DIR/README.md" <<EOF
# Generated TUI Previews

Generated by \`scripts/tui-previews.sh\`.

- Theme: \`$THEME\`
- Theme variants: \`$THEMES\`
- Provider: \`$PROVIDER\`
- Model: \`$MODEL\`

Files:

- \`main.txt\` / \`main.ansi\`
- \`main-idle.txt\` / \`main-idle.ansi\`
- \`main-command-palette.txt\` / \`main-command-palette.ansi\`
- \`main-lane.txt\` / \`main-lane.ansi\`
- \`main.svg\` / \`main-idle.svg\` / \`main-command-palette.svg\` / \`main-lane.svg\` quick visual screenshots
- \`side-1.txt\` / \`side-1.ansi\`
- \`side-2.txt\` / \`side-2.ansi\`
- \`multiscreen.txt\` combined plain-text workstation preview
- \`main.<theme>.ansi\` for each generated theme variant
EOF

wc -l "$OUT_DIR"/main.txt "$OUT_DIR"/main-idle.txt "$OUT_DIR"/main-command-palette.txt "$OUT_DIR"/main-lane.txt "$OUT_DIR"/side-1.txt "$OUT_DIR"/side-2.txt
