# Core 0.3 SDD Progress

Plan: `docs/superpowers/plans/2026-07-19-core-0.3-runtime-contract.md`
Branch: `codex/v3-core-runtime`
Baseline: `aba8b05c5d334cf1a8424c8dc899819b4ecae0bb`

- Task 1: complete (commits aba8b05c..fbabc9ae, review clean)
- Task 2: complete (commits fbabc9ae..5b934592, review approved; legacy TUI workspace compile deferred to the TUI client migration gate)
- Task 3: complete (commits 5b934592..e596e5c2, security re-review clean; legacy TUI compile remains the explicit integration gate)
- Task 4: complete (commits e596e5c2..a3c5bc17, review clean; en/zh-CN and eight effective skin/mode pairs frozen)
- Task 5: complete (commits a3c5bc17..4b1c58ab, stale-index fix re-reviewed clean; transcript paging/session/type/runtime gates passed)
- Task 6: complete (commits 4b1c58ab..52df5925, two Important review findings fixed and re-review passed; replayable journal/CoreClient gates passed)
- Task 7: complete (payload 52df5925..5bd2b80b, evidence checkpoint afd6fcc9; full review and facade/tag-gate re-review passed; nine schema-1 fixtures, typed UI preferences, and bilingual compatibility docs verified)
- Task 8: complete (commits 1e30bc73..e81718f1; structured lane/process/patch effect adapters, git worktree delegation, runtime patch parser removal, fail-closed permission epochs, Lane completion ordering, and session-rule owner isolation; independent re-review PASS/APPROVED with no findings)
- Task 9: complete (storage `0af55fcc`, runtime integration `7873dc4e`; append-only typed Lane store, idempotent legacy import, resume activation, and typed-only runtime projection independently re-reviewed PASS/APPROVED with no findings)
- Task 10: complete (historical Lane implementation through `e81718f1`, evidence checkpoint `31bf6c5f`; exact owner-routing/non-blocking/Plan-before-effect acceptance independently re-reviewed PASS/APPROVED with no findings)
- Task 11: complete (implementation `1fd3e59c`; strict D11 schema/value and opaque credential-id hardening follow-up complete)
- Task 12: complete (typed cross-lane trust loop, canonical-evidence MergeGate recovery, conflict revalidation, and audited revert complete; Core gates green, legacy TUI integration gate remains)
- Task 13: pending
- Task 14: pending

Minor findings for final review:

- Task 2: confirm/document in the lane lifecycle task that `Detached` remains active while its background process/session may still be running.
