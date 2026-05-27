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
BIN_FILE="$BIN_NAME"
if [[ "$TARGET" == *"windows"* ]]; then
  BIN_FILE="$BIN_NAME.exe"
fi
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
cp "$ROOT/target/$TARGET/release/$BIN_FILE" "$DIST_DIR/$BIN_FILE"
cp "$ROOT/README.md" "$DIST_DIR/README.md"
cp "$ROOT/README.zh-CN.md" "$DIST_DIR/README.zh-CN.md"

if [[ "$TARGET" == *"windows"* ]]; then
  cat >"$DIST_DIR/INSTALL.md" <<EOF
# RoboCode v${VERSION}

Install on Windows PowerShell:

\`\`\`powershell
New-Item -ItemType Directory -Force "\$env:USERPROFILE\\bin"
Copy-Item .\\${BIN_FILE} "\$env:USERPROFILE\\bin\\${BIN_FILE}"
\$env:PATH += ";\$env:USERPROFILE\\bin"
${BIN_FILE} --help
\`\`\`

Run fallback smoke test:

\`\`\`powershell
${BIN_FILE} --no-tui --provider fallback --model test-local
\`\`\`

Run TUI:

\`\`\`powershell
${BIN_FILE} --tui --provider fallback --model test-local
\`\`\`
EOF
else
  cat >"$DIST_DIR/INSTALL.md" <<EOF
# RoboCode v${VERSION}

Install on macOS/Linux:

\`\`\`bash
chmod +x ${BIN_FILE}
sudo mv ${BIN_FILE} /usr/local/bin/${BIN_NAME}
${BIN_NAME} --help
\`\`\`

Run fallback smoke test:

\`\`\`bash
${BIN_NAME} --no-tui --provider fallback --model test-local
\`\`\`

Run TUI:

\`\`\`bash
${BIN_NAME} --tui --provider fallback --model test-local
\`\`\`
EOF
fi

(
  cd "$ROOT/dist"
  tar -czf "$ARCHIVE_NAME.tar.gz" "$ARCHIVE_NAME"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$ARCHIVE_NAME.tar.gz" >"$ARCHIVE_NAME.tar.gz.sha256"
  else
    sha256sum "$ARCHIVE_NAME.tar.gz" >"$ARCHIVE_NAME.tar.gz.sha256"
  fi
)

printf '%s\n' "$ROOT/dist/$ARCHIVE_NAME.tar.gz"
