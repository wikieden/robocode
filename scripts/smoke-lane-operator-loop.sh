#!/usr/bin/env sh
set -eu

run() {
  name="$1"
  pattern="$2"
  printf '==> %s\n' "$name"
  cargo test -p viden-tui "$pattern" --quiet -- --test-threads=1
}

run "lane shell runtime" "lane_run_"
run "lane inspect and decisions" "lane_decision_records_changed_files_and_inspect_evidence"
run "lane embedded PTY send" "lane_send_writes_to_embedded_pty_input_fifo"
run "lane tmux attach evidence" "lane_tmux_"
run "lane accept and apply" "lane_apply_requires_accept_and_applies_worktree_patch"
run "lane conflict review" "lane_apply_conflict_writes_review_artifact_without_mutating_workspace"
run "lane conflict resolve" "lane_resolve_retries_apply_conflict_after_manual_workspace_fix"
run "lane discard and cleanup" "lane_cleanup_requires_force_for_dirty_worktree_and_preserves_artifacts"
run "lane archive" "lane_archive_preserves_evidence_without_deleting_worktree"

printf 'Lane operator loop smoke passed.\n'
