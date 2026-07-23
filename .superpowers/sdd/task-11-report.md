# Task 11 Project Onboarding Runtime Contract Report

Date: 2026-07-20
Branch: `codex/v3-core-runtime`
Worktree: `/Users/wiki/Documents/GitHub/viden/.worktrees/v3-core-runtime`
Starting HEAD: `31bf6c5fb07907bf28c32e84d80f0120598112ec`

## Outcome

Task 11 now provides a typed, frontend-safe project onboarding boundary for
Core 0.3.1:

- read-only Git/non-Git and root `viden.toml` project probes;
- a repository-policy parser kept separate from machine-local
  `.viden/config.toml` provider settings;
- read-only preview with exact reviewable UTF-8 contents, byte length, SHA-256,
  and destination base hash;
- Build-mode confirmation that writes only the retained preview bytes after
  permission approval and rejects stale or mismatched previews before effects;
- an injected credential backend whose serialized command carries only an
  opaque one-use ingress id and whose facts contain only safe handle metadata;
- provider/model health projected together with the active credential handle;
- schema-1 known events, additive capability negotiation, Core facade exports,
  replayable view state, and durable workflow audit facts.

The task brief listed no credential-specific event. `CredentialHandleStored`
was added because otherwise `CredentialHandle` could not become a replayable
Core fact without frontend-private state. The additive
`runtime.project_onboarding` and `runtime.credential_handles` capabilities gate
the new commands/events while the frozen Core 0.3.0 fixture corpus remains
unchanged.

## TDD evidence

The implementation followed RED-GREEN checkpoints:

1. The first config test failed because `ProjectFileConfig`,
   `parse_project_config`, and the project module did not exist. The D11 policy
   parser then made valid, malformed, empty, missing-runner, and secret-bearing
   cases pass.
2. The first runtime test compile failed because `CredentialBackend` and the
   four onboarding commands/facts did not exist. Typed facts, engine state,
   exact preview retention, command handling, and the backend boundary made the
   focused runtime suite pass.
3. The credential audit assertion then failed because the safe handle was not
   in the workflow projection. Making `CredentialHandleStored` durable produced
   an auditable handle without exposing the seeded secret.
4. The transaction failure test exposed a macOS `/var` versus `/private/var`
   canonical-path mismatch during rollback. Capturing the rollback target under
   the canonical project root restored the old exact bytes and kept the preview
   retryable.
5. A supervisor integration test exercises the real
   preview -> approval request -> response -> exact-byte confirmation path,
   preventing the generic supervisor's deny-only approver from blocking
   CoreClient usage.

The eight focused runtime tests cover Git/non-Git, valid/missing/invalid config,
preview read-only behavior, exact confirmed bytes/hash, stale destination
rejection, Plan denial before approval/write, audit rollback/retry, supervisor
approval resume, provider/model health, and credential redaction across command,
event, transcript-shaped JSON, and workflow audit JSON.

### Independent review hardening follow-up

An independent review of implementation commit `1fd3e59c` found that the first
parser revision still allowed unknown root tables and relied on a small key
denylist. It also found that opaque credential identifiers accepted path-like
and secret-like labels. The follow-up used two new RED checkpoints:

- the config regression failed because `[provider] api_token = "sk-..."`
  parsed successfully;
- the runtime regressions failed because that candidate produced valid exact
  preview contents and `sk-*` credential identifiers reached the backend.

The hardened parser now implements the strict D11 root/nested allowlist,
validates runner/target records, rejects expanded secret field names, and scans
string values for credential-shaped prefixes before exposing exact contents.
Provider, backend, and credential-request identifiers now use a bounded ASCII
opaque-id grammar that excludes path syntax, traversal, secret markers, and
unsafe long labels. The regressions cover `api_token`, `token`, `sk-*`, unknown
`provider`/`local` tables, plus field-specific `sk-`, `token`, `api_key`, and
path-like values for all three identifiers.

## Security and transaction invariants

- Root `viden.toml` policy is never parsed through layered
  `.viden/config.toml`, which may contain machine-local provider secrets.
- The parser rejects known secret-bearing field names and does not echo source
  snippets in syntax diagnostics. It accepts only the documented D11
  `project`, `gates`, `runner`, `budget`, and `targets` schema and rejects
  credential-shaped string values before preview publication.
- Invalid candidates omit exact contents and are never inserted into the
  confirmable preview map.
- `ConfirmProjectConfig` carries only preview id and SHA-256. Core rehashes the
  retained bytes, reparses them, checks the destination base hash, resolves
  permission, then writes through a same-directory temporary file and rename.
- A failed workflow audit append restores both the previous file bytes and the
  in-memory preview/state snapshot.
- `StoreCredentialHandle` carries provider id, backend id, and an opaque ingress
  id only. All three ids must satisfy the bounded, non-path, non-secret-like
  opaque ASCII grammar. The injected backend owns secret bytes and returns a
  `CredentialHandle { provider_id, backend_id, status }`.
- Command-accepted payloads redact config contents and bound credential
  identifiers. Durable audit stores only valid project policy and safe handle
  metadata.
- Project mutation approvals participate in supervisor permission generations;
  queued Plan/ReadOnly changes stale an outstanding allow before any effect.

## Documentation and comments

English and Chinese compatibility and frontend contract documents were updated
together. They describe capability negotiation, preview/confirm semantics, and
the credential secret boundary. Concise comments were added only at the policy
file boundary, credential backend boundary, exact preview field, and secret-safe
handle type; ordinary control flow remains self-explanatory.

No migrations or frozen fixture refresh were required. Existing Core 0.3.0
fixture replay remained byte-for-byte valid because optional extension fields
are omitted when empty.

## Verification

Passed:

- `cargo test -p viden-config`: 24 passed;
- `cargo test -p viden-types`: 49 passed;
- `cargo test -p viden-runtime project_runtime_`: 8 passed;
- `cargo test -p viden-runtime`: 357 passed, 1 ignored live-provider test;
- `cargo test -p viden-core`: unit, 12 CoreClient, 3 frontend-contract, and 5
  workspace-identity tests passed; 1 manual fixture refresh ignored;
- `cargo clippy -p viden-config -p viden-types -p viden-runtime -p viden-core --all-targets -- -D warnings`;
- `scripts/check-dependency-boundaries.sh`;
- paired-document and link checks for both changed English/Chinese document
  pairs;
- `cargo fmt --all -- --check`;
- `git diff --check`.

Attempted but blocked outside Task 11 scope:

- `cargo test --workspace --quiet` reaches the existing TUI/Core migration
  mismatch. `apps/tui` still imports removed root facade items such as
  `viden_core::SessionEngine`, uses the removed `ApprovalResponse.approved`
  field, and constructs historical string/field forms of typed task/lane
  records. No TUI or GUI files were changed.

## Handoff

Owned changes are limited to Core types/config/runtime/supervisor/session
projection, the Core facade extension manifest/tests, paired Core/frontend
contract documentation, and this report. No Task 12+ implementation, live
provider, push, merge, tag, release, or frontend mutation was performed.
