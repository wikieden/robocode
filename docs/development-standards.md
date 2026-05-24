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
- Treat documentation and code comments as part of the implementation, not as
  optional cleanup.

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
