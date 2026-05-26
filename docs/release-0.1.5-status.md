# RoboCode 0.1.5 Release Status

Last updated: 2026-05-26

## Goal

Version `0.1.5` is the programming-experience release. Its purpose is to make
the TUI cockpit feel usable for the everyday coding loop: understand the task,
approve mutations, inspect diff and test evidence, supervise lanes, and produce
installable release artifacts.

## Phase Mapping

1. TUI interaction stability: local smoke passed.
2. Coding loop evidence: local smoke passed.
3. Lane operator workflow: local smoke passed for shell and tmux lane gates.
4. Provider compatibility: DeepSeek V4 Flash live smoke passed.
5. Release artifact validation: pending final GitHub Actions artifact run.

## Candidate Evidence

- Workspace package version moved from `0.1.4` to `0.1.5`.
- `Cargo.lock` package entries now resolve to `0.1.5`.
- GitHub release workflow default tag moved to `v0.1.5`.
- README install examples now point at `v0.1.5`.
- The README system screenshot keeps its curated layout with the visible
  version updated to `0.1.5`.
- Local release smoke is scripted through `scripts/release-smoke.sh`; the
  script captures logs, generated TUI previews, fallback CLI smoke, lane smoke,
  and host package smoke in one evidence directory.
- Full local release smoke with DeepSeek live provider validation passed:
  `scripts/release-smoke.sh --version 0.1.5 --deepseek --out-dir /tmp/robocode-015-release-smoke-deepseek-local`.
- Evidence directory:
  `/tmp/robocode-015-release-smoke-deepseek-local`.
- DeepSeek V4 Flash live smoke passed; the transcript contains
  `robocode-deepseek-smoke-ok`.
- Host package smoke passed for `aarch64-apple-darwin`; the extracted binary
  prints `robocode-cli 0.1.5`.
- macOS arm64 archive SHA-256:
  `734fe4a266178946b871e10a847ec8ac1f50642e270f708d8446fe5a81315e78`.

## Validation Gates

Before publishing `v0.1.5`, run after pushing the version bump to `main`:

```bash
scripts/release-smoke.sh --version 0.1.5 --skip-package --deepseek --github-actions
```

The final status update should record:

- the GitHub Actions release artifact validation run URL;
- the final release workflow run URL after upload;
- the published release URL and artifact list.

## Open Findings

### P0

- Continue watching visual alignment regressions in the right rail and side
  screens; frame glyphs now use stable frame coloring independent of row
  semantic highlights.

### P1

- `/lane` remains a TUI/runtime command surface; the ordinary REPL still treats
  `/lane` as unknown. This is intentional for `0.1.5` and should remain explicit
  in release notes.

### P2

- Full cursor-addressed terminal replay remains deferred.
- Inline conflict editing remains deferred.
- More external coding-tool templates remain demand-driven follow-up work.

## Release Outcome

`v0.1.5` is not published yet. This page should be updated after the GitHub
Actions artifact validation and release workflow complete.
