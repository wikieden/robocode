# TUI Supervision Decision Checkpoints

Chinese version: [checkpoints.zh-CN.md](checkpoints.zh-CN.md)

Date: 2026-08-30

This evidence describes a local candidate only. Nothing here is published,
signed, notarized, pushed, merged, tagged, or certified against a live
provider. The work exists as two commits on a feature branch in an isolated
worktree.

## Candidate Line

| Item | SHA / path |
| --- | --- |
| Base `origin/main` | `126a5321` |
| Stage 1 — supervision foundation | `2aafa99b` on `claude/tui-supervision-foundation` |
| Stage 2 — decision workflows | tip of `claude/tui-supervision-decisions` |
| Worktree | `.worktrees/tui-supervision-decisions` |
| Component | TUI `0.3.3`, `min_core_version` `0.3.4` |

Stage 2 branches off stage 1, not off `main`. Neither branch has been merged or
pushed.

## Surfaces Delivered

| Surface | Behavior |
| --- | --- |
| Decision Center (`OverlayKind::Decisions`) | Lists approvals, merge gates, pending review requests, and pending conflict bounces as one ordered pick list, with the registered `⏸` / `◌` / `⚠` glyphs and a selection marker. Approval rows still route to the pinned Approval overlay. |
| Supervision decision overlay (`OverlayKind::SupervisionDecision`) | New overlay parameterized by a `SupervisionTarget` (gate, review, or bounce). Keyboard-first action bar; arrows and number keys select, `Enter` confirms, `Esc` unwinds the reason line then the overlay. |
| Action availability | Derived from the full Core record re-read from `RuntimeViewState`, never from the compact projection row. Actions that can never apply to the record's current status are not listed. |
| Reason / feedback input | Single-line input inside the overlay. Reject, revert, and bounce require text; review feedback is optional for both verdicts. Empty required text and text over Core's 500-character trust-text limit are refused locally with nothing sent. |
| Dispatch and outcome | Every action sends one `RuntimeCommand` through the Core client and registers a confirm-on-fact expectation. Outcome renders in the overlay and echoes in the status bar: pending `◌`, confirmed `✓`, rejected `✗` with Core's own reason verbatim. |
| Reset and escape | A settled outcome resets to idle when the next supervision action is initiated or the overlay is closed. A pending outcome is never auto-reset. A `dismiss` action in the overlay and in the Decision Center footer clears a stranded pending correlation without settling it and without cancelling the Core command. |
| Wall-time display | Blind-lane run stats render wall time as milliseconds, seconds with one decimal, or minutes plus seconds, replacing the raw millisecond figure. |

## Payload Derivation Mirrored From The GUI

The TUI sends the same shapes the GUI integration-gate adapter sends, because
Core compares acceptance against those exact values
(`apps/gui/src-tauri/src/projection.rs`, `apps/gui/src-tauri/src/adapter.rs`):

| Command | Derivation |
| --- | --- |
| `AcceptMergeGate.actor` | `d12_accept_actor`: with a validator, the validator owner when it carries a Lane id, otherwise no actor; without a validator, the gate owner. |
| `AcceptMergeGate.reviewed_evidence` | `d12_reviewed_evidence`: with a validator, the review request's recorded `evidence_bindings` verbatim; a default-owner gate sends an empty list; otherwise the canonical bindings rebuilt from `latest_evidence`, sorted and deduped (`d12_canonical_bindings`). Any listed evidence without a canonical reference means no valid payload, so the action is refused locally. |
| `RejectMergeGate.actor` | `d12_reject_actor`: the gate owner when it is not the default owner, otherwise the validator owner when it has a Lane id and is not default. |
| `RevertAppliedChange.owner` | The gate owner, refused when it is the default owner. |
| `BounceMergeConflict` | Owner and `original_lane_id` both replayed from the gate owner, as `validate_conflict_bounce` requires. |
| `RevalidateMergeConflict` | `bounce_id` and `actor` from the pending conflict record; `evidence` is the one canonical binding whose `source_hash` differs from every baseline hash, as `validate_conflict_revalidation` requires. |
| `DecideReview.actor` | The gate validator's owner when it names this review, otherwise Core's own `reviewer_owner_from_requester` derivation: the requester owner re-pointed at the reviewer Lane with session and turn identity unclaimed. |

## Confirm-On-Fact Expectations

| Action | Confirming Core fact |
| --- | --- |
| Accept gate | `MergeGateUpdated` with this gate id and `Accepted` |
| Reject gate | `MergeGateUpdated` with this gate id and `NeedsChanges` |
| Revalidate conflict | `MergeGateUpdated` with this gate id and `CollectingEvidence` (`trust_loop::revalidate_merge_conflict` publishes `MergeConflictBounced` then returns the gate to evidence collection) |
| Revert | `RevertRecorded` for this gate |
| Bounce | `MergeConflictBounced` for this gate |
| Decide review | `ReviewRequestUpdated` with this review id and the status the verdict maps to |

`CommandAccepted` never confirms. `CommandRejected` for the same command id is
the only local failure path and carries Core's reason unchanged.

## Pinned Tests

Added to `scripts/tui-turn-controller-smoke.sh`:

```text
tui::pending::tests::abandon_clears_a_stranded_pending_decision_without_settling_it
tui::pending::tests::only_a_settled_outcome_resets_to_idle
tui::decision::tests::action_availability_follows_the_records_current_status
tui::decision::tests::accept_payload_mirrors_the_gui_actor_and_reviewed_evidence_derivation
tui::decision::tests::reject_revert_and_bounce_replay_core_owned_parties_and_require_a_reason
tui::decision::tests::revalidation_carries_the_bounce_identity_and_a_changed_canonical_receipt
tui::decision::tests::review_verdicts_carry_the_reviewer_lane_actor_and_optional_feedback
tui::decision::tests::decision_picks_list_approvals_gates_pending_reviews_then_pending_bounces
tui::app::tests::decision_center_lists_supervision_rows_and_routes_every_pick
tui::app::tests::supervision_overlay_unwinds_escape_in_order_and_yields_to_a_pinned_approval
tui::app::tests::supervision_overlay_only_lists_actions_the_gate_status_can_accept
tui::app::tests::a_required_reason_is_enforced_locally_and_nothing_is_sent
tui::app::tests::every_supervision_decision_round_trips_through_its_exact_core_fact
tui::app::tests::core_rejection_renders_its_own_reason_and_frees_the_decision_slot
tui::app::tests::a_second_supervision_action_while_one_is_pending_sends_nothing
tui::app::tests::dismiss_releases_a_stranded_pending_decision_without_sending_anything
tui::app::tests::a_settled_outcome_resets_on_the_next_action_and_on_overlay_close
tui::app::tests::composer_stays_editable_while_the_supervision_overlay_is_open_during_a_stream
tui::app::tests::blind_lane_wall_time_is_rendered_at_the_scale_an_operator_reads
tui::modal::tests::decisions_overlay_projects_typed_gates_recovery_and_pending_core_command
tui::modal::tests::blind_lane_inspector_shows_bounded_run_facts_and_never_fabricates_zeros
```

`every_supervision_decision_round_trips_through_its_exact_core_fact` drives all
seven commands through the fake Core client and asserts, for each one, the exact
`RuntimeCommandEnvelope` sent, that the receipt leaves the decision pending, and
that only the matching business fact confirms it.

## Deterministic Evidence

| Command | Result |
| --- | --- |
| `cargo test -p viden-tui` | PASS, 306 lib + 1 API test |
| `bash scripts/tui-turn-controller-smoke.sh` | PASS |
| `bash scripts/rc-tui-stability-smoke.sh` | PASS |
| `bash scripts/tui-regression.sh` | PASS |
| `bash scripts/tui-previews.sh` | PASS, all preview assertions hold |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets` | PASS, no new warning in `viden-tui` |
| `cargo test --workspace --quiet` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `git diff --check` | PASS |
| `scripts/check-doc-pairs.sh` / `scripts/check-doc-links.sh` on changed Markdown | PASS |

The i18n catalog digests in `apps/tui/release-manifest.toml` were recomputed for
the added supervision keys and both locales keep exact key and parameter parity.

## Not Delivered Here (T1b)

- The audit/history panel over `AuditRecord` timelines.
- Creation flows for handoffs, contracts, and dependencies. Their intent
  builders exist in `apps/tui/src/tui/supervision.rs` and remain undispatched
  behind a local `#[allow(dead_code)]`.
- A dedicated evidence inspector. The decision overlay shows evidence counts and
  identifiers, not evidence contents.
- Multi-select or batch supervision decisions. Exactly one supervision command
  may be in flight, by design.
