#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo test -p robocode-cli \
  tui::app::tests::runtime_provider_turn_starts_without_blocking_ui_thread \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::provider_turn_streams_approves_tools_runs_queued_followup_and_releases_composer \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::active_turn_enter_queues_next_prompt_and_keeps_composer_editable \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::active_approval_resolves_through_channel_without_nested_event_loop \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::active_approval_does_not_swallow_composer_typing \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::mode_and_permission_commands_immediately_sync_tui_runtime_status \
  --quiet
cargo test -p robocode-cli \
  tui::statusbar::tests::bottom_bar_reflects_runtime_mode_and_permission_level \
  --quiet
cargo test -p robocode-cli \
  tui::topbar::tests::top_bar_status_reflects_active_turn_instead_of_static_auto_text \
  --quiet
cargo test -p robocode-cli \
  tui::render::tests::transcript_status_marks_new_output_while_viewing_history \
  --quiet
cargo test -p robocode-cli \
  tui::state::tests::agent_tasks_surface_pending_turn_without_duplicate_provider_task \
  --quiet

printf 'TUI TurnController smoke passed\n'
