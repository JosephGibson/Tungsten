# Documentation cleanup

status: done
goal: archive the owner-designated plans, correct stale documentation and improve targeted agent navigation.
non-goals: runtime changes, release/version cuts, historical decision rewrites, archive inspection, or closing the review report being handled in another session.
files to touch: maintained Markdown documentation and local skills; the three named plans move to the archive.
ordered steps: inventory and read maintained docs; check claims against source and decisions; archive designated plans and update incoming references; correct guidance and navigation; run documentation and workspace checks.
done-when: the named plans are archived, maintained links and source references resolve, current guidance reflects implemented behavior, and just ctx, just repo-check, just check and git diff --check pass.

## Context

The owner requested a full documentation cleanup. Archive `gpu-perf-pass.md`, `platformer-polish-pass.md` and `debug-cleanup-docs-pass.md`. Treat `docs/repo-review-2026-09-25.md` as closing out in another session: remove it from live navigation, but let that session own its contents and move. Keep unresolved roadmap work active; leave dated release history and immutable decisions intact.

## Completed

Archived the three owner-designated plans unchanged. Added a canonical documentation map; corrected implemented architecture, reload support, font registration and fixture/capture guidance; reduced duplicated release detail in DESIGN and the Phase 4 roadmap; shortened local skills around canonical sources. The review report remains owned by the other session.

Validation passed: `just ctx`, `just repo-check`, `just check` (850 tests passed, four ignored), `git diff --check`, both edited skill validators, maintained Markdown links/anchors, and shell syntax/capture-recipe checks. Archive contents and external URLs were not inspected; visual artifacts were not regenerated.
