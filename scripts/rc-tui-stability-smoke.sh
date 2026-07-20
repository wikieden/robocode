#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-}"

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$(mktemp -d "/tmp/viden-rc-tui-stability.XXXXXX")"
fi

mkdir -p "$OUT_DIR"
SUMMARY="$OUT_DIR/summary.md"
STEPS="$OUT_DIR/step-results.tsv"
: >"$SUMMARY"
: >"$STEPS"

cd "$ROOT"

record() {
  printf '%s\n' "$*" >>"$SUMMARY"
}

record_step() {
  local name="$1"
  local status="$2"
  local log_file="$3"
  printf '%s\t%s\t%s\n' "$name" "$status" "$log_file" >>"$STEPS"
}

run_step() {
  local name="$1"
  shift
  local log_file="$OUT_DIR/${name}.log"
  printf '[rc-tui] START %s\n' "$name"
  if "$@" >"$log_file" 2>&1; then
    printf '[rc-tui] PASS  %s\n' "$name"
    record "- PASS \`$name\`"
    record_step "$name" "pass" "$log_file"
  else
    local rc=$?
    printf '[rc-tui] FAIL  %s (exit %s)\n' "$name" "$rc" >&2
    record "- FAIL \`$name\` (exit $rc, log: \`$log_file\`)"
    record_step "$name" "fail" "$log_file"
    tail -80 "$log_file" >&2 || true
    exit "$rc"
  fi
}

run_cargo_test() {
  local name="$1"
  shift
  run_step "$name" cargo_test_with_match "$@"
}

cargo_test_with_match() {
  local output
  local rc
  set +e
  output="$(cargo test -p viden-tui "$@" -- --nocapture 2>&1)"
  rc=$?
  set -e
  printf '%s\n' "$output"
  if [[ "$rc" -ne 0 ]]; then
    return "$rc"
  fi
  if ! grep -Eq 'test result: ok\. [1-9][0-9]* passed' <<<"$output"; then
    printf 'test filter matched no passing tests: %s\n' "$*" >&2
    return 1
  fi
}

mouse_capture_default_is_disabled() {
  if rg -q 'EnableMouseCapture|DisableMouseCapture' apps/tui/src/tui/terminal.rs; then
    printf 'terminal default path must not enable mouse capture\n' >&2
    return 1
  fi
  rg -q '^mouse_capture = false$' apps/tui/release-manifest.toml
}

record "# Viden RC TUI Stability Smoke"
record ""
record "- Evidence directory: \`$OUT_DIR\`"
record ""
record "## Guardrail Results"

run_step "terminal-redraw-and-residue-tests" cargo test -p viden-tui tui::terminal::tests -- --nocapture
run_cargo_test "fake-slow-provider-nonblocking" runtime_provider_turn_starts_without_blocking_ui_thread
run_cargo_test "approval-nonblocking" active_approval_does_not_swallow_composer_typing
run_cargo_test "typed-lane-projection-render" typed_done_review_and_blocked_lanes_project_into_rendered_statuses
run_cargo_test "typed-side-lane-state" typed_core_lane_states_drive_side_counts_and_state_rows
run_cargo_test "shortcut-hint-consistency" rendered_shortcut_hints_match_command_and_agent_handlers
run_step "mouse-capture-default-off" mouse_capture_default_is_disabled
run_cargo_test "streaming-scrollback" streaming_delta_does_not_steal_scrollback_when_user_scrolled_up
run_cargo_test "focus-paste-repaint-policy" focus_and_paste_events_force_repaint_without_becoming_input
run_cargo_test "composer-residue-filter" composer_discards_terminal_escape_residue_instead_of_rendering_it
run_cargo_test "provider-model-picker-setup" provider_and_model_selector_paths_are_reachable_from_core_client_loop
run_cargo_test "configured-model-picker-scope" models_selector_filters_unconfigured_providers
run_cargo_test "welcome-missing-key-clean-start" render_frame_uses_welcome_layout_for_first_empty_session
run_cargo_test "live-work-preview-contract" render_frame_keeps_live_activity_visible_for_lanes_and_tool_calls
run_cargo_test "core-client-startup" startup_check_connects_core_client_without_entering_terminal
run_cargo_test "runtime-view-projection" runtime_view_projects_authoritative_frontend_facts_without_workspace_fixture
run_cargo_test "approval-core-command" approval_shortcut_builds_response_for_core_request_id
run_cargo_test "streaming-composer" composer_stays_editable_while_events_stream
run_cargo_test "queue-core-command" active_turn_enter_queues_follow_up_instead_of_submitting_second_turn
run_cargo_test "shared-contract-fixture" shared_frontend_fixtures_reduce_to_core_expected_facts
run_cargo_test "gap-replay" sequence_gap_requests_replay_before_success_is_visible
run_cargo_test "atomic-replay-rollback" failed_replay_does_not_publish_partial_view_or_cursor
run_cargo_test "core-preference-skin-mode" core_color_mode_changes_the_effective_palette
run_cargo_test "core-preference-color-depth" core_color_depth_selects_a_non_rgb_terminal_palette
run_cargo_test "all-palettes-all-depths" all_eight_palettes_map_across_truecolor_ansi256_and_ansi16
run_cargo_test "settings-apply-reset-receipts" apply_and_reset_wait_for_matching_core_receipts
run_cargo_test "synthetic-planning-clears-after-result" agent_tasks_do_not_keep_failed_provider_turn_active
run_step "tui-regression-preview" scripts/tui-regression.sh "$OUT_DIR/tui-regression"

record ""
record "## Automated Gate Scope"
record ""
record "- Every named gate above matched at least one passing test."
record "- This smoke does not claim that the complete P0/P1 backlog is empty."
record ""
record "## Manual Terminal Acceptance"
record ""
record "Manual macOS Terminal and iTerm2 screenshots remain human evidence. This"
record "smoke records whether a release run supplied screenshot evidence through"
record "\`VIDEN_TUI_MANUAL_EVIDENCE_DIR\`."

manual_dir="${VIDEN_TUI_MANUAL_EVIDENCE_DIR:-}"
if [[ -n "$manual_dir" ]]; then
  terminal_count="$(find "$manual_dir" -maxdepth 1 -type f -iname '*terminal*' | wc -l | tr -d ' ')"
  iterm_count="$(find "$manual_dir" -maxdepth 1 -type f \( -iname '*iterm*' -o -iname '*iterm2*' \) | wc -l | tr -d ' ')"
  if [[ "$terminal_count" -lt 1 || "$iterm_count" -lt 1 ]]; then
    printf 'manual evidence dir must contain Terminal and iTerm2 screenshots: %s\n' "$manual_dir" >&2
    exit 1
  fi
  record "- Manual evidence: \`$manual_dir\`"
  record "- Terminal screenshots: \`$terminal_count\`"
  record "- iTerm2 screenshots: \`$iterm_count\`"
else
  record "- Manual evidence: not supplied to this automated run."
  record "- Release status must either link the real screenshots or record this as"
  record "  a remaining manual evidence risk for the final 0.1.x zero-bug gate."
fi

record ""
record "Generated at: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
printf '%s\n' "$OUT_DIR"
