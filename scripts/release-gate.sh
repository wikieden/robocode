#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=""
TARGET=""
OUT_DIR=""
PHASE="prepublish"
RUN_GITHUB_ACTIONS=0
DRY_RUN=0

final_zero_bug_required() {
  [[ "${VIDEN_REQUIRE_FINAL_ZERO_BUG:-0}" == "1" || "$VERSION" == "0.1.30" ]]
}

usage() {
  cat <<'EOF'
Usage: scripts/release-gate.sh --version <version> [options]

Canonical release testing entrypoint.

Phases:
  prepublish   Full local release smoke plus live DeepSeek development smoke.
  postpublish  Published GitHub assets plus Homebrew tap validation.
  all          Run prepublish and postpublish in order.

Options:
  --version <version>     Release version, for example 0.1.24.
  --target <triple>       Optional release target triple for package smoke.
  --out-dir <dir>         Evidence root; defaults to /tmp/viden-release-gate-...
  --phase <phase>         prepublish, postpublish, or all. Default: prepublish.
  --github-actions        Also dispatch the release workflow in prepublish.
  --dry-run               Print commands without running them.
  -h, --help              Show this help.

Prepublish requires DEEPSEEK_API_KEY because every release must include a real
development scenario smoke with token and cost evidence.
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
    --phase)
      PHASE="${2:-}"
      shift 2
      ;;
    --github-actions)
      RUN_GITHUB_ACTIONS=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
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

if [[ -z "$VERSION" ]]; then
  printf '--version is required for the release gate\n\n' >&2
  usage >&2
  exit 2
fi

case "$PHASE" in
  prepublish | postpublish | all) ;;
  *)
    printf 'unknown phase: %s\n\n' "$PHASE" >&2
    usage >&2
    exit 2
    ;;
esac

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$(mktemp -d "/tmp/viden-release-gate-v${VERSION}.XXXXXX")"
fi
mkdir -p "$OUT_DIR"

SUMMARY="$OUT_DIR/release-gate-summary.md"
: >"$SUMMARY"

record() {
  printf '%s\n' "$*" >>"$SUMMARY"
}

print_command() {
  printf '[release-gate] %q' "$1"
  shift
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf '\n'
}

run_or_print() {
  print_command "$@"
  if [[ "$DRY_RUN" == "0" ]]; then
    "$@"
  fi
}

run_prepublish() {
  if [[ "$DRY_RUN" == "0" && -z "${DEEPSEEK_API_KEY:-}" ]]; then
    printf 'DEEPSEEK_API_KEY is required for release prepublish gate\n' >&2
    exit 2
  fi

  local phase_dir="$OUT_DIR/prepublish"
  local -a args=(scripts/release-smoke.sh --version "$VERSION" --out-dir "$phase_dir")
  if [[ -n "$TARGET" ]]; then
    args+=(--target "$TARGET")
  fi
  args+=(--deepseek)
  if [[ "$RUN_GITHUB_ACTIONS" == "1" ]]; then
    args+=(--github-actions)
  fi
  run_or_print "${args[@]}"
  if final_zero_bug_required; then
    run_or_print scripts/final-zero-bug-smoke.sh "$phase_dir/final-zero-bug"
  fi
  record "- prepublish: \`$phase_dir\`"
}

run_postpublish() {
  local phase_dir="$OUT_DIR/postpublish"
  local -a args=(scripts/release-smoke.sh --version "$VERSION" --out-dir "$phase_dir")
  if [[ -n "$TARGET" ]]; then
    args+=(--target "$TARGET")
  fi
  args+=(--quick --skip-package --github-release-assets --homebrew)
  run_or_print "${args[@]}"
  record "- postpublish: \`$phase_dir\`"
}

record "# Viden Release Gate"
record ""
record "- Version: \`$VERSION\`"
record "- Phase: \`$PHASE\`"
record "- Evidence root: \`$OUT_DIR\`"
record "- Dry run: \`$DRY_RUN\`"
record ""
record "## Evidence"

cd "$ROOT"

case "$PHASE" in
  prepublish)
    run_prepublish
    ;;
  postpublish)
    run_postpublish
    ;;
  all)
    run_prepublish
    run_postpublish
    ;;
esac

record ""
record "Generated at: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
printf '%s\n' "$OUT_DIR"
