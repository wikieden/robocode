# Documentation Localization Policy

## Purpose

This document defines how RoboCode documentation should be maintained in both
English and Simplified Chinese.

## Current Policy

- English remains the canonical base version for technical structure and file naming.
- Simplified Chinese is maintained as a first-class companion translation, not
  as an optional afterthought.
- Every long-lived product, architecture, roadmap, requirements, and plan
  document should have a matching `*.zh-CN.md` file.

## File Naming

- English source file: `name.md`
- Simplified Chinese companion: `name.zh-CN.md`

Examples:

- `README.md`
- `README.zh-CN.md`
- `docs/architecture.md`
- `docs/architecture.zh-CN.md`

## Linking Rules

- Each English document should link to its Chinese companion near the top.
- Each Chinese document should link back to its English companion near the top.
- High-level entrypoints such as `README.md` should list both language variants.

## Update Rules

- When a canonical English document changes materially, update the corresponding
  Chinese document in the same change set whenever practical.
- If a companion translation cannot be updated immediately, mark that gap in the
  same pull request or commit description rather than silently letting the two
  diverge.
- Avoid combining English and Chinese bodies in the same file unless the file is
  intentionally short and index-like.

## Scope

This policy applies to:

- repository overview docs
- architecture docs
- product requirements
- staged roadmaps
- gap matrices
- implementation plans intended for long-term reuse

It does not require bilingual duplication for:

- generated artifacts
- temporary scratch notes
- one-off local debugging notes

## Style Guidance

- Keep section structure aligned between language versions.
- Preserve filenames, command names, enum names, and code identifiers in their
  original form.
- Translate prose, not code.
- Prefer clear engineering language over overly literal translation.

