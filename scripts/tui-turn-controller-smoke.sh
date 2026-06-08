#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo test -p robocode-cli \
  tui::app::tests::runtime_provider_turn_starts_without_blocking_ui_thread \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::active_turn_enter_queues_next_prompt_and_keeps_composer_editable \
  --quiet
cargo test -p robocode-cli \
  tui::app::tests::active_approval_resolves_through_channel_without_nested_event_loop \
  --quiet
cargo test -p robocode-cli \
  tui::render::tests::transcript_status_marks_new_output_while_viewing_history \
  --quiet
cargo test -p robocode-cli \
  tui::state::tests::agent_tasks_surface_pending_turn_without_duplicate_provider_task \
  --quiet

printf 'TUI TurnController smoke passed\n'
