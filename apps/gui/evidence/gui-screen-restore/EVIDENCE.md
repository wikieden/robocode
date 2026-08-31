# Restored GUI screens — visual evidence

Chinese version: [EVIDENCE.zh-CN.md](EVIDENCE.zh-CN.md)

Covers the five screens restored on `codex/v3-gui-screen-restore`: D2 decision
center, D10 lane monitor, D12 integration gate, D13 fleet and workflow, and D14
audit timeline.

## Why the projections are generated, not written

The capture page never hand-writes a projection. `tests/capture_projections.rs`
runs the real GUI projection over the canonical `frontend-contract-v1` fixtures
and serializes the result into `projections/*.json`, which the page then mounts.
The captured pixels are therefore what Core facts actually produce, and a
projection change that breaks a screen shows up in the capture instead of being
masked by a hand-tuned literal.

Regenerate after any projection change:

```bash
cargo test -p viden-gui --test capture_projections -- --ignored
```

## Capture procedure

Start the dev server from the worktree that owns `apps/gui/**`:

```bash
npm --prefix apps/gui run dev -- --port 4173 --strictPort
```

Then open each URL in the authorized Browser runtime at a 1440x900 viewport.
This mirrors `tools/capture-d1-visual.sh`: the procedure standardizes URLs and
dimensions and does not invoke browser automation outside that runtime.

| Screen | URL |
| --- | --- |
| D2 decision center | `http://localhost:4173/evidence/gui-screen-restore/screen-capture.html?screen=d2` |
| D10 lane monitor | `…?screen=d10` |
| D12 integration gate | `…?screen=d12` |
| D13 fleet and workflow | `…?screen=d13` |
| D14 audit trail (audit mode) | `…?screen=d14-audit` |
| D14 raw event replay (capability absent) | `…?screen=d14-raw` |
| Locale and skin proof | `…?screen=d12&locale=zh-CN&mode=light` |

`locale`, `mode`, and `density` are accepted on every screen and resolve through
the shared `resolveTheme` path, so the harness never ships a second palette.

## What each capture must show

- **D2**: three queue groups; the gate item selected with its Core risk bucket;
  scoped actions built from `allowed_scopes`; the audit id in the action bar;
  `GUI-CORE-012` on the context pane and `GUI-CORE-013` on the contract group.
- **D10**: one card per lane; gate strength from `AgentLaneRecord`; a bound lane
  showing its project and an unbound lane stating it has none; `GUI-CORE-014`
  in place of the event ticker.
- **D12**: the conflict banner with `strong gate · cannot be bypassed`; the
  missing required evidence named; `accept` visibly disabled while it is
  missing; the bounce timeline and the post-merge revert; `GUI-CORE-015`.
- **D13**: the DAG goal and status; a node's declared `depends on` edge; a
  blocked node naming its Core dependency reason; `Core recorded no handoff`.
- **D14 audit mode**: the mode toggle with `Audit trail` pressed; one row per
  `AuditRecord` newest-first with Core's raw dotted `action` key, the actor
  (including the agent id when the actor is an agent lane), the outcome, the
  linked object chips, the bounded argument chips, and a readable
  `YYYY-MM-DD HH:MM:SS UTC` time; the load-older control
  present while Core's page is incomplete.
- **D14 raw mode**: the same toggle with `Raw event replay (diagnostic)`
  pressed, the audit button visibly disabled, and the note naming the absent
  `runtime.audit` capability; below it, rows in Core cursor order labelled with
  Core's own event discriminants, the undecodable row kept and highlighted, and
  the paging control present while the batch is incomplete.

## Facts added on top of a fixture

Every addition below uses the same typed Core record the runtime publishes; no
field is invented and no display string is parsed into a fact.

| Screen | Fixture | Added |
| --- | --- | --- |
| D2 | `approval-allow-deny.json` | the approval replayed from the fixture's own event payload (the fixture's stream resolves it, leaving the pending queue empty), one `ContractRecord`, one pending `ReviewRequestRecord`, one `EvidenceView` |
| D10 | `multi-lane.json` | one `LaneRuntimeOwnerBinding`, so both the bound and unbound paths render |
| D12 | `merge-gate.json` | gate status `needs_changes` with one required evidence id, one `ConflictBounce`, one `RevertRecord` |
| D13 | `dag-blocker.json` | one blocked `DependencyRecord` |
| D14 raw | none | the replay contract has no fixture, so the batch is built from typed Core events and served through the same `CoreClient::replay` path |
| D14 audit | none | the audit store is not view state and has no fixture, so the page is built from `AuditRecord::sanitized` values — the only constructor Core's own emission sites use — and delivered as `CommandAccepted` then `AuditPageLoaded` through the production acceptance-first correlation machine |

## Known limitation

The authorized Browser runtime renders and verifies these pages but cannot
write PNG files, so this directory holds the reproducible harness rather than
committed images. Producing committed PNGs needs a capture script the project
explicitly sanctions; `tools/capture-d1-visual.sh` deliberately stops at the
same boundary.

The native Tauri window was not captured. It is a development binary rather
than a registered application bundle, so the desktop capture tooling cannot
target it. The app itself does start from this branch: `npm run tauri -- dev`
runs once port `1420` is free, or with an explicit override such as
`--config '{"build":{"beforeDevCommand":"npm run dev -- --port 4173 --strictPort","devUrl":"http://localhost:4173"}}'`.
