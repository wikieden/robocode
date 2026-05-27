#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=""
TARGET=""
OUT_DIR=""
QUICK=0
RUN_DEEPSEEK=0
RUN_GITHUB_ACTIONS=0
SKIP_PACKAGE=0

usage() {
  cat <<'EOF'
Usage: scripts/release-smoke.sh [options]

Runs the RoboCode release smoke matrix and stores logs in an evidence directory.

Options:
  --version <version>     Release version to package; defaults to Cargo package version.
  --target <triple>       Release target triple; defaults to the local rustc host.
  --out-dir <dir>         Evidence directory; defaults to /tmp/robocode-release-smoke-...
  --quick                 Run a faster local check set without full workspace tests or package smoke.
  --skip-package          Skip host package archive smoke.
  --deepseek              Run the opt-in DeepSeek provider smoke. Requires DEEPSEEK_API_KEY.
  --github-actions        Dispatch release.yml with upload_to_release=false. Requires gh auth.
  -h, --help              Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --target)
      TARGET="${2:-}"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="${2:-}"
      shift 2
      ;;
    --quick)
      QUICK=1
      SKIP_PACKAGE=1
      shift
      ;;
    --skip-package)
      SKIP_PACKAGE=1
      shift
      ;;
    --deepseek)
      RUN_DEEPSEEK=1
      shift
      ;;
    --github-actions)
      RUN_GITHUB_ACTIONS=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

cd "$ROOT"

if [[ -z "$VERSION" ]]; then
  VERSION="$(cargo pkgid -p robocode-cli | sed 's/.*#//')"
fi

if [[ -z "$TARGET" ]]; then
  TARGET="$(rustc -Vv | awk '/host:/ { print $2 }')"
fi

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$(mktemp -d "/tmp/robocode-release-smoke-v${VERSION}.XXXXXX")"
fi

mkdir -p "$OUT_DIR"

SUMMARY="$OUT_DIR/summary.md"
: >"$SUMMARY"

log() {
  printf '[smoke] %s\n' "$*"
}

record() {
  printf '%s\n' "$*" >>"$SUMMARY"
}

run_step() {
  local name="$1"
  shift
  local log_file="$OUT_DIR/${name}.log"
  log "START $name"
  if "$@" >"$log_file" 2>&1; then
    log "PASS  $name"
    record "- PASS \`$name\`"
  else
    local rc=$?
    log "FAIL  $name (exit $rc)"
    record "- FAIL \`$name\` (exit $rc, log: \`$log_file\`)"
    tail -80 "$log_file" >&2 || true
    exit "$rc"
  fi
}

run_bash_step() {
  local name="$1"
  local script="$2"
  run_step "$name" bash -lc "$script"
}

fallback_cli_smoke() {
  local work_dir="$OUT_DIR/fallback-workspace"
  local transcript="$OUT_DIR/fallback-cli-transcript.log"
  rm -rf "$work_dir"
  mkdir -p "$work_dir"
  (
    cd "$work_dir"
    git init >/dev/null
    git config user.email smoke@example.com
    git config user.name "RoboCode Smoke"
    printf 'smoke\n' >README.md
    git add README.md
    git commit -m initial >/dev/null
    printf '/test printf smoke-ok\ny\n/status\n/exit\n' |
      cargo run -p robocode-cli --manifest-path "$ROOT/Cargo.toml" --quiet -- \
        --provider fallback \
        --model test-local
  ) >"$transcript" 2>&1

  grep -Fq "status: passed" "$transcript"
  grep -Fq "smoke-ok" "$transcript"
  grep -Fq "Last test: passed" "$transcript"
  cat "$transcript"
}

deepseek_cli_smoke() {
  local transcript="$OUT_DIR/deepseek-cli-transcript.log"
  if [[ -z "${DEEPSEEK_API_KEY:-}" ]]; then
    printf 'DEEPSEEK_API_KEY is required for --deepseek\n' >&2
    return 2
  fi

  local work_dir="$OUT_DIR/deepseek-workspace"
  rm -rf "$work_dir"
  mkdir -p "$work_dir"
  (
    cd "$work_dir"
    git init >/dev/null
    git config user.email smoke@example.com
    git config user.name "RoboCode Smoke"
    printf 'smoke\n' >README.md
    git add README.md
    git commit -m initial >/dev/null
    printf 'Reply with exactly: robocode-deepseek-smoke-ok\n/exit\n' |
      cargo run -p robocode-cli --manifest-path "$ROOT/Cargo.toml" --quiet -- \
        --provider deepseek \
        --model deepseek-v4-flash
  ) >"$transcript" 2>&1

  grep -Fq "robocode-deepseek-smoke-ok" "$transcript"
  cat "$transcript"
}

lane_operator_smoke() {
  run_step "lane-operator-loop-smoke" scripts/smoke-lane-operator-loop.sh
}

package_smoke() {
  local package_log="$OUT_DIR/package-archive.log"
  local archive
  archive="$(scripts/package-release.sh "$VERSION" "$TARGET" | tee "$package_log" | tail -1)"
  [[ -f "$archive" ]]
  [[ -f "$archive.sha256" ]]

  (
    cd "$(dirname "$archive")"
    if command -v shasum >/dev/null 2>&1; then
      shasum -a 256 -c "$(basename "$archive").sha256"
    else
      sha256sum -c "$(basename "$archive").sha256"
    fi
  ) >>"$package_log" 2>&1

  local extract_dir="$OUT_DIR/package-extract"
  rm -rf "$extract_dir"
  mkdir -p "$extract_dir"
  tar -xzf "$archive" -C "$extract_dir"

  local package_dir="$extract_dir/robocode-v${VERSION}-${TARGET}"
  local binary="$package_dir/robocode-cli"
  if [[ "$TARGET" == *"windows"* ]]; then
    binary="$package_dir/robocode-cli.exe"
  fi

  [[ -x "$binary" || -f "$binary" ]]
  "$binary" --version >>"$package_log" 2>&1
  "$binary" --help >>"$package_log" 2>&1
  grep -Fq "robocode-cli $VERSION" "$package_log"
  cat "$package_log"
}

github_actions_validation() {
  command -v gh >/dev/null
  gh workflow run release.yml \
    --repo wikieden/robocode \
    -f "tag=v${VERSION}" \
    -f "upload_to_release=false"
}

record "# RoboCode Release Smoke"
record ""
record "- Version: \`$VERSION\`"
record "- Target: \`$TARGET\`"
record "- Quick: \`$QUICK\`"
record "- Evidence directory: \`$OUT_DIR\`"
record ""
record "## Results"

run_step "cargo-fmt" cargo fmt --check

if [[ "$QUICK" == "1" ]]; then
  run_bash_step "robocode-cli-terminal-tests" "cargo test -p robocode-cli tui::terminal::tests -- --nocapture"
else
  run_bash_step "robocode-cli-tests" "cargo test -p robocode-cli --quiet -- --test-threads=1"
  run_bash_step "workspace-tests" "cargo test --workspace --quiet -- --test-threads=1"
fi

run_step "tui-previews" scripts/tui-previews.sh "$OUT_DIR/tui-previews"
run_step "fallback-cli-smoke" fallback_cli_smoke
run_step "codex-app-server-protocol-fixture" scripts/smoke-codex-app-server-protocol-fixture.sh
run_step "codex-app-server-write-guard" scripts/smoke-codex-app-server-write-guard.sh
lane_operator_smoke

if [[ "$SKIP_PACKAGE" == "0" ]]; then
  run_step "package-smoke" package_smoke
else
  log "SKIP  package-smoke"
  record "- SKIP \`package-smoke\`"
fi

if [[ "$RUN_DEEPSEEK" == "1" ]]; then
  run_step "deepseek-cli-smoke" deepseek_cli_smoke
else
  log "SKIP  deepseek-cli-smoke (use --deepseek)"
  record "- SKIP \`deepseek-cli-smoke\` (use \`--deepseek\`)"
fi

if [[ "$RUN_GITHUB_ACTIONS" == "1" ]]; then
  run_step "github-actions-release-validation" github_actions_validation
else
  log "SKIP  github-actions-release-validation (use --github-actions)"
  record "- SKIP \`github-actions-release-validation\` (use \`--github-actions\`)"
fi

record ""
record "Generated at: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
log "DONE  evidence: $OUT_DIR"
printf '%s\n' "$OUT_DIR"
