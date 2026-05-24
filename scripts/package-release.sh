#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"
TARGET="${2:-}"

if [[ -z "$VERSION" ]]; then
  VERSION="$(cargo pkgid -p robocode-cli | sed 's/.*#//')"
fi

if [[ -z "$TARGET" ]]; then
  TARGET="$(rustc -Vv | awk '/host:/ { print $2 }')"
fi

BIN_NAME="robocode-cli"
ARCHIVE_NAME="robocode-v${VERSION}-${TARGET}"
DIST_DIR="$ROOT/dist/$ARCHIVE_NAME"
TARGET_ARGS=()
if [[ -n "$TARGET" ]]; then
  TARGET_ARGS=(--target "$TARGET")
fi

cd "$ROOT"
cargo build -p robocode-cli --release "${TARGET_ARGS[@]}"

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
cp "$ROOT/target/$TARGET/release/$BIN_NAME" "$DIST_DIR/$BIN_NAME"
cp "$ROOT/README.md" "$DIST_DIR/README.md"
cp "$ROOT/README.zh-CN.md" "$DIST_DIR/README.zh-CN.md"

cat >"$DIST_DIR/INSTALL.md" <<EOF
# RoboCode v${VERSION}

Install:

\`\`\`bash
chmod +x ${BIN_NAME}
sudo mv ${BIN_NAME} /usr/local/bin/${BIN_NAME}
${BIN_NAME} --help
\`\`\`

Run fallback smoke test:

\`\`\`bash
${BIN_NAME} --provider fallback --model test-local
\`\`\`

Run TUI:

\`\`\`bash
${BIN_NAME} --tui --provider fallback --model test-local
\`\`\`
EOF

(
  cd "$ROOT/dist"
  tar -czf "$ARCHIVE_NAME.tar.gz" "$ARCHIVE_NAME"
  shasum -a 256 "$ARCHIVE_NAME.tar.gz" >"$ARCHIVE_NAME.tar.gz.sha256"
)

printf '%s\n' "$ROOT/dist/$ARCHIVE_NAME.tar.gz"
