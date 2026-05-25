# Development Standards

Chinese version: [development-standards.zh-CN.md](development-standards.zh-CN.md)

## Purpose

This document captures the coding standard RoboCode maintainers and coding
agents should follow when changing the project.

## Core Rules

- Keep changes small, reviewable, and reversible.
- Preserve existing behavior unless the task explicitly changes it.
- Reuse existing modules, helpers, and patterns before adding new abstractions.
- Prefer deletion and simplification over new layers.
- Do not introduce new dependencies without an explicit product or engineering
  reason.
- Treat documentation and code comments as part of the implementation. Future
  coding work is not complete until the code, the maintainer-facing comments,
  and the affected documentation are all aligned.

## Required Coding Standard: Docs and Comments

Every future code change must treat documentation and code comments as part of
the delivery, not as optional cleanup:

- before editing, identify whether the change can affect user-facing behavior,
  commands, configuration, architecture, workflows, packaging, or maintenance
  assumptions;
- while editing, add concise comments for non-obvious decisions, invariants,
  safety boundaries, protocol compatibility, terminal rendering rules, or
  concurrency behavior;
- when behavior changes, update the relevant durable docs in the same change
  set, including the matching `*.zh-CN.md` document when one exists;
- before handoff, review the final diff and state whether documentation and
  comments were updated, already sufficient, or intentionally unnecessary.

Code that changes behavior without updating stale docs or comments should be
treated as incomplete.

## Code Change Definition of Done

Every code change should be considered incomplete until the surrounding
maintainer context is also updated:

- decide during implementation whether the change affects docs, comments, or
  both; do not defer that review to a separate cleanup pass;
- update docs in the same change when behavior, commands, configuration,
  screenshots, workflows, architecture, packaging, or troubleshooting changes;
- add or refresh code comments for non-obvious invariants, safety boundaries,
  rendering rules, protocol compatibility, or concurrency behavior;
- remove stale comments and stale documentation when implementation changes make
  them misleading;
- keep comments concise and specific to the decision or invariant they protect;
- include documentation and comment review in the final diff check before
  testing and handoff.

## Documentation Standard

Update documentation in the same change set whenever a change affects:

- user-visible commands, flags, configuration, or installation steps;
- TUI behavior, keyboard controls, panes, screenshots, or workflows;
- architecture, module boundaries, runtime contracts, or persistence formats;
- provider behavior, permission behavior, tool semantics, or safety rules;
- release, packaging, Homebrew, or troubleshooting instructions.

Keep root README files focused on product value, installation, usage, feedback,
and license information. Put durable architecture, plans, and implementation
details under `docs/`.

When editing long-lived user-facing docs, update the matching Simplified
Chinese `*.zh-CN.md` companion whenever practical. If a translation cannot be
updated in the same change, call out the gap in the commit or PR notes.

Documentation should describe the current implemented behavior. Do not present
future plans, placeholders, mock panels, or unverified provider behavior as
finished features. If a feature is partial, label the current limitation and the
next expected step.

## Code Comment Standard

Use comments to explain intent that is not obvious from the code itself:

- invariants that later changes must preserve;
- safety or permission boundaries;
- protocol, transcript, persistence, or provider compatibility contracts;
- non-obvious control flow, terminal rendering behavior, or concurrency rules;
- why a tempting simpler implementation would be incorrect.

Avoid comments that only restate what the next line of code already says.
Prefer clear names and small functions first; add comments where names are not
enough to protect future maintainers from subtle mistakes.

Before completing a code change, review the diff for two questions:

- Would a maintainer understand why this behavior exists six months from now?
- Did the change modify a user-facing contract that should be reflected in
  docs?

If the answer is yes, add the smallest useful comment or documentation update
in the same change set.

## Verification Standard

Before calling work complete, run the smallest meaningful verification first,
then broaden when the change touches shared behavior:

- formatting for edited Rust code;
- focused tests for changed modules;
- workspace tests before release-facing or shared runtime changes;
- TUI preview/screenshot generation for visual changes;
- documentation review for changed commands, workflows, or public behavior.

If something was not tested, record that honestly in the final note or commit
trailers.
