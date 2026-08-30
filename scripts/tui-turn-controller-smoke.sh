#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

tests=(
  tui::app::tests::runtime_view_projects_authoritative_frontend_facts_without_workspace_fixture
  tui::app::tests::submit_queue_cancel_and_approval_use_runtime_commands
  tui::app::tests::active_turn_enter_queues_follow_up_instead_of_submitting_second_turn
  tui::app::tests::approval_shortcut_builds_response_for_core_request_id
  tui::app::tests::pinned_approval_never_owns_composer_y_n_d_or_enter
  tui::app::tests::explicitly_focused_approval_owns_shortcuts_and_enter
  tui::app::tests::exact_setup_enter_opens_setup_while_nonexact_prefix_only_completes
  tui::app::tests::setup_previews_exact_draft_before_core_confirmation
  tui::app::tests::runtime_replacement_atomically_clears_stale_lane_and_session_identity
  tui::app::tests::lane_without_core_session_stays_on_board_with_lane_detail
  tui::app::tests::composer_stays_editable_while_events_stream
  tui::app::tests::runtime_provider_turn_starts_without_blocking_ui_thread
  tui::app::tests::active_approval_does_not_swallow_composer_typing
  tui::app::tests::paste_normalizes_crlf_preserves_leading_slash_and_never_submits
  tui::composer::tests::cursor_sits_on_middle_input_row_for_ime_candidate_placement
  tui::keymap::tests::escape_unwinds_overlay_then_selection_then_insert
  tui::statusbar::tests::status_bar_tracks_insert_and_overlay_ownership
  tui::input::tests::approval_keyboard_focus_reaches_deny_diff_and_approve
  tui::app::tests::owner_scoped_cancel_uses_the_exact_live_lane_owner_without_denying_approval
  tui::client::tests::failed_replay_does_not_publish_partial_view_or_cursor
  tui::client::tests::complete_replay_without_incoming_rolls_back_all_staged_events
  tui::client::tests::shared_frontend_fixtures_reduce_to_core_expected_facts
  tui::client::tests::lane_runtime_owner_extension_fixture_replays_the_exact_owner
  tui::app::tests::release_manifest_declares_requested_and_effective_presentation_inputs
  tui::statusbar::tests::status_bar_always_displays_the_explicit_input_mode
  tui::render::structured_runtime_tests::render_frame_keeps_live_activity_visible_for_lanes_and_tool_calls
  tui::app::tests::streaming_delta_does_not_steal_scrollback_when_user_scrolled_up
  tui::render::structured_runtime_tests::agent_tasks_do_not_keep_failed_provider_turn_active
  tui::pending::tests::command_acceptance_never_confirms_a_supervision_decision
  tui::pending::tests::only_the_exact_merge_gate_fact_confirms_the_decision
  tui::pending::tests::review_conflict_and_revert_expectations_each_confirm_on_their_own_fact
  tui::pending::tests::a_fact_for_another_command_never_confirms_an_unaccepted_decision
  tui::pending::tests::rejection_carries_the_core_reason_and_clears_the_pending_decision
  tui::pending::tests::a_second_supervision_command_is_refused_while_one_is_pending
  tui::pending::tests::events_are_ignored_entirely_when_no_supervision_command_is_pending
  tui::app::tests::supervision_decision_confirms_only_on_the_core_business_fact
)

for test_name in "${tests[@]}"; do
  output="$(cargo test -p viden-tui "$test_name" -- --exact --nocapture 2>&1)"
  if ! grep -Eq 'test result: ok\. 1 passed' <<<"$output"; then
    printf 'TUI TurnController smoke matched no passing test: %s\n' "$test_name" >&2
    printf '%s\n' "$output" >&2
    exit 1
  fi
done

printf 'TUI TurnController smoke passed\n'
