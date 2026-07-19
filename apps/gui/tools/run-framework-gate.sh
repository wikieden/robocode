#!/usr/bin/env bash
set -uo pipefail

if (( $# != 2 )); then
  printf 'usage: %s <tauri|gpui> <fixture.json>\n' "$0" >&2
  exit 2
fi

framework="$1"
if [[ "$framework" != "tauri" && "$framework" != "gpui" ]]; then
  printf 'framework must be tauri or gpui: %s\n' "$framework" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "$0")" && pwd -P)"
repo_root="$(cd "$script_dir/../../.." && pwd -P)"
fixture_path="$(python3 - "$2" <<'PY'
import sys
from pathlib import Path
print(Path(sys.argv[1]).resolve())
PY
)"
if [[ ! -f "$fixture_path" ]]; then
  printf 'fixture does not exist: %s\n' "$fixture_path" >&2
  exit 2
fi

evidence_dir="$repo_root/apps/gui/evidence/framework-gate"
mkdir -p "$evidence_dir"
gate_tmp_dir="$(mktemp -d)"
trap 'rm -r -- "$gate_tmp_dir"' EXIT
commands_jsonl="$gate_tmp_dir/commands.jsonl"

record_command() {
  local name="$1"
  local command="$2"
  local exit_code="$3"
  local log_path="$4"
  python3 - "$name" "$command" "$exit_code" "$log_path" >> "$commands_jsonl" <<'PY'
import json
import sys
from pathlib import Path

name, command, raw_exit_code, raw_log_path = sys.argv[1:]
exit_code = int(raw_exit_code)
lines = Path(raw_log_path).read_text(encoding="utf-8", errors="replace").splitlines()
print(json.dumps({
    "name": name,
    "command": command,
    "status": "pass" if exit_code == 0 else "fail",
    "exit_code": exit_code,
    "output_tail": lines[-40:],
}, ensure_ascii=False))
PY
}

run_check() {
  local name="$1"
  local command="$2"
  local log_path="$gate_tmp_dir/$name.log"
  printf 'framework gate [%s] %s\n' "$framework" "$name"
  (cd "$repo_root" && bash -lc "$command") >"$log_path" 2>&1
  LAST_EXIT_CODE=$?
  tail -40 "$log_path"
  record_command "$name" "$command" "$LAST_EXIT_CODE" "$log_path"
  if (( LAST_EXIT_CODE == 0 )); then
    LAST_STATUS="pass"
  else
    LAST_STATUS="fail"
  fi
}

run_launch() {
  local name="$1"
  local binary="$2"
  local command
  printf -v command 'python3 %q 5 %q' "$repo_root/apps/gui/tools/check-process-survival.py" "$binary"
  run_check "$name" "$command"
}

canonical_fixture="$repo_root/crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
fixture_guard=""
printf -v fixture_guard 'python3 %q %q %q' \
  "$repo_root/apps/gui/tools/verify-framework-fixture.py" \
  "$fixture_path" \
  "$canonical_fixture"

run_check \
  "shared-ordered-events" \
  "$fixture_guard && cargo test --manifest-path apps/gui/spikes/Cargo.toml -p viden-gui-spike-common --test replay ten_thousand_fixture_events_preserve_cursor_and_content_identity -- --exact"
ordered_status="$LAST_STATUS"

run_check \
  "shared-transcript-rows" \
  "$fixture_guard && cargo test --manifest-path apps/gui/spikes/Cargo.toml -p viden-gui-spike-common --test replay fifty_thousand_rows_round_trip_every_page_without_caching_row_history -- --exact"
transcript_status="$LAST_STATUS"

if [[ "$framework" == "tauri" ]]; then
  run_check "candidate-tests" "$fixture_guard && cd apps/gui/spikes/tauri && npm test -- --run"
  candidate_test_status="$LAST_STATUS"
  run_check "frontend-build" "cd apps/gui/spikes/tauri && npm run build"
  frontend_build_status="$LAST_STATUS"
  run_check \
    "bridge-tests" \
    "$fixture_guard && cargo test --manifest-path apps/gui/spikes/tauri/src-tauri/Cargo.toml"
  bridge_status="$LAST_STATUS"
  run_check "native-build" "cd apps/gui/spikes/tauri && npm run tauri -- build --debug"
  native_build_status="$LAST_STATUS"
  binary_path="$repo_root/apps/gui/spikes/tauri/src-tauri/target/debug/viden-gui-spike-tauri"
else
  run_check \
    "candidate-tests" \
    "$fixture_guard && cargo test --manifest-path apps/gui/spikes/Cargo.toml -p viden-gui-spike-gpui"
  candidate_test_status="$LAST_STATUS"
  frontend_build_status="pass"
  bridge_status="pass"
  run_check \
    "native-build" \
    "cargo build --manifest-path apps/gui/spikes/Cargo.toml -p viden-gui-spike-gpui"
  native_build_status="$LAST_STATUS"
  binary_path="$repo_root/apps/gui/spikes/target/debug/viden-gui-spike-gpui"
fi

host_os="$(uname -s)"
if [[ "$host_os" == "Darwin" && "$native_build_status" == "pass" && -x "$binary_path" ]]; then
  run_launch "native-launch" "$binary_path"
  native_launch_status="$LAST_STATUS"
else
  native_launch_status="unavailable"
fi

export VIDEN_GATE_FRAMEWORK="$framework"
export VIDEN_GATE_FIXTURE_PATH="$(python3 - "$repo_root" "$fixture_path" <<'PY'
import sys
from pathlib import Path
root, fixture = map(Path, sys.argv[1:])
try:
    print(fixture.relative_to(root))
except ValueError:
    print(fixture)
PY
)"
export VIDEN_GATE_FIXTURE_SHA256="$(shasum -a 256 "$fixture_path" | awk '{print $1}')"
export VIDEN_GATE_PROJECTION_SHA256="$(python3 - "$fixture_path" <<'PY'
import json
import sys
from pathlib import Path
print(json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))["expected_view_sha256"])
PY
)"
export VIDEN_GATE_HOST_OS="$host_os"
export VIDEN_GATE_HOST_ARCH="$(uname -m)"
export VIDEN_GATE_RUSTC_VERSION="$(rustc --version)"
export VIDEN_GATE_NODE_VERSION="$(node --version)"
export VIDEN_GATE_PYTHON_VERSION="$(python3 --version)"
export VIDEN_GATE_COMMANDS_JSONL="$commands_jsonl"
export VIDEN_GATE_ORDERED_STATUS="$ordered_status"
export VIDEN_GATE_TRANSCRIPT_STATUS="$transcript_status"
export VIDEN_GATE_CANDIDATE_TEST_STATUS="$candidate_test_status"
export VIDEN_GATE_FRONTEND_BUILD_STATUS="$frontend_build_status"
export VIDEN_GATE_BRIDGE_STATUS="$bridge_status"
export VIDEN_GATE_NATIVE_BUILD_STATUS="$native_build_status"
export VIDEN_GATE_NATIVE_LAUNCH_STATUS="$native_launch_status"
export VIDEN_GATE_OUTPUT="$evidence_dir/$framework.json"

python3 <<'PY'
import json
import os
from datetime import datetime, timezone
from pathlib import Path


def command_evidence(name):
    return [f"command:{name}"]


def measurement(status, value, unit, operator, threshold, evidence):
    return {
        "status": status,
        "value": value,
        "unit": unit,
        "threshold": {"operator": operator, "value": threshold, "unit": unit},
        "evidence": evidence,
    }


def hard_gate(status, summary, evidence=None):
    return {"status": status, "summary": summary, "evidence": evidence or []}


framework = os.environ["VIDEN_GATE_FRAMEWORK"]
commands = [
    json.loads(line)
    for line in Path(os.environ["VIDEN_GATE_COMMANDS_JSONL"]).read_text(encoding="utf-8").splitlines()
]
ordered_status = os.environ["VIDEN_GATE_ORDERED_STATUS"]
transcript_status = os.environ["VIDEN_GATE_TRANSCRIPT_STATUS"]
candidate_status = os.environ["VIDEN_GATE_CANDIDATE_TEST_STATUS"]
bridge_status = os.environ["VIDEN_GATE_BRIDGE_STATUS"]
native_build_status = os.environ["VIDEN_GATE_NATIVE_BUILD_STATUS"]
native_launch_status = os.environ["VIDEN_GATE_NATIVE_LAUNCH_STATUS"]
host_os = os.environ["VIDEN_GATE_HOST_OS"]

if candidate_status == "pass":
    cjk = hard_gate(
        "partial",
        "Framework composition tests pass, but no native operating-system IME injection was captured.",
        command_evidence("candidate-tests"),
    )
    keyboard = hard_gate(
        "partial",
        "Framework focus traversal tests pass, but no full native-window keyboard run was captured.",
        command_evidence("candidate-tests"),
    )
else:
    cjk = hard_gate("fail", "Candidate composition tests failed.", command_evidence("candidate-tests"))
    keyboard = hard_gate("fail", "Candidate focus tests failed.", command_evidence("candidate-tests"))

if framework == "tauri":
    parity_evidence = command_evidence("candidate-tests") + command_evidence("bridge-tests")
    parity_status = "pass" if candidate_status == "pass" and bridge_status == "pass" else "fail"
else:
    parity_evidence = command_evidence("candidate-tests")
    parity_status = candidate_status

d1_slice_parity = hard_gate(
    parity_status,
    (
        "The candidate reproduces the shared D1 projection and interaction contract."
        if parity_status == "pass"
        else "The candidate failed the shared D1 projection or interaction contract."
    ),
    parity_evidence,
)

if host_os == "Darwin" and native_build_status == "pass" and native_launch_status == "pass":
    macos = hard_gate(
        "pass",
        "The debug desktop binary built and remained alive for a five-second macOS launch smoke.",
        command_evidence("native-build") + command_evidence("native-launch"),
    )
elif host_os == "Darwin":
    macos = hard_gate(
        "fail",
        "The macOS debug build or five-second launch smoke failed.",
        command_evidence("native-build") + command_evidence("native-launch"),
    )
else:
    macos = hard_gate("unavailable", "This evidence was not collected on macOS.")

manifest = (
    "apps/gui/spikes/tauri/src-tauri/Cargo.toml"
    if framework == "tauri"
    else "apps/gui/spikes/gpui/Cargo.toml"
)
record = {
    "schema_version": 1,
    "framework": framework,
    "component_version": "0.1.0-alpha.1",
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "fixture": {
        "path": os.environ["VIDEN_GATE_FIXTURE_PATH"],
        "sha256": os.environ["VIDEN_GATE_FIXTURE_SHA256"],
        "projection_sha256": os.environ["VIDEN_GATE_PROJECTION_SHA256"],
    },
    "host": {
        "os": host_os,
        "arch": os.environ["VIDEN_GATE_HOST_ARCH"],
        "rustc": os.environ["VIDEN_GATE_RUSTC_VERSION"],
        "node": os.environ["VIDEN_GATE_NODE_VERSION"],
        "python": os.environ["VIDEN_GATE_PYTHON_VERSION"],
    },
    "commands": commands,
    "measurements": {
        "composer_input_p95_ms": measurement("unavailable", None, "ms", "lt", 50, []),
        "event_to_visible_p95_ms": measurement("unavailable", None, "ms", "lt", 100, []),
        "frame_work_p95_ms": measurement("unavailable", None, "ms", "lt", 16, []),
        "ordered_event_count": measurement(
            ordered_status,
            10000 if ordered_status == "pass" else None,
            "events",
            "gte",
            10000,
            command_evidence("shared-ordered-events"),
        ),
        "transcript_row_count": measurement(
            transcript_status,
            50000 if transcript_status == "pass" else None,
            "rows",
            "gte",
            50000,
            command_evidence("shared-transcript-rows"),
        ),
    },
    "hard_gates": {
        "d1_slice_parity": d1_slice_parity,
        "cjk_ime": cjk,
        "keyboard_only": keyboard,
        "screen_reader": hard_gate("unavailable", "No screen-reader semantics run was captured."),
        "macos_build_launch": macos,
        "linux_build_launch": hard_gate("unavailable", "No Linux build and launch run was captured."),
        "windows_build_launch": hard_gate("unavailable", "No Windows build and launch run was captured."),
        "bounded_transcript_rendering": hard_gate(
            "partial" if transcript_status == "pass" else "fail",
            "Shared Core paging stays bounded at 50,000 rows; framework rendering was not instrumented.",
            command_evidence("shared-transcript-rows"),
        ),
        "bounded_soak": hard_gate("unavailable", "No bounded-duration native soak was captured."),
        "idle_cpu_near_zero": hard_gate("unavailable", "No native idle CPU sample was captured."),
        "no_long_lived_framework_fork": hard_gate(
            "pass",
            "The spike uses released framework packages and carries no repository patch or fork.",
            [f"file:{manifest}"],
        ),
        "visual_parity": hard_gate("unavailable", "No repeatable live D1 screenshot comparison was captured."),
        "signing": hard_gate("unavailable", "No signed artifact was produced."),
        "updater": hard_gate("unavailable", "No updater path was exercised."),
        "credential_storage": hard_gate("unavailable", "No OS credential-store path was exercised."),
        "crash_recovery": hard_gate("unavailable", "No native crash/restart recovery run was captured."),
    },
}

output = Path(os.environ["VIDEN_GATE_OUTPUT"])
output.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(f"wrote {output}")
PY
