#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo test -p viden-tui \
  tui::app::tests::runtime_view_projects_authoritative_frontend_facts_without_workspace_fixture \
  --quiet
cargo test -p viden-tui \
  tui::app::tests::submit_queue_cancel_and_approval_use_runtime_commands \
  --quiet
cargo test -p viden-tui \
  tui::app::tests::active_turn_enter_queues_follow_up_instead_of_submitting_second_turn \
  --quiet
cargo test -p viden-tui \
  tui::app::tests::approval_shortcut_builds_response_for_core_request_id \
  --quiet
cargo test -p viden-tui \
  tui::app::tests::composer_stays_editable_while_events_stream \
  --quiet
cargo test -p viden-tui \
  tui::client::tests::failed_replay_does_not_publish_partial_view_or_cursor \
  --quiet
cargo test -p viden-tui \
  tui::client::tests::complete_replay_without_incoming_rolls_back_all_staged_events \
  --quiet
cargo test -p viden-tui \
  tui::client::tests::shared_frontend_fixtures_reduce_to_core_expected_facts \
  --quiet
cargo test -p viden-tui \
  tui::statusbar::tests::bottom_bar_reflects_runtime_mode_and_permission_level \
  --quiet
cargo test -p viden-tui \
  tui::topbar::tests::top_bar_status_reflects_active_turn_instead_of_static_auto_text \
  --quiet
cargo test -p viden-tui \
  tui::render::tests::transcript_status_marks_new_output_while_viewing_history \
  --quiet
cargo test -p viden-tui \
  tui::state::tests::agent_tasks_surface_pending_turn_without_duplicate_provider_task \
  --quiet

printf 'TUI TurnController smoke passed\n'
