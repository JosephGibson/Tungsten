# Repository review and tooling

- **status:** done
- **goal:** Review all crates and examples, correct documentation and clear defects, archive completed plans, and automate repeated repository checks.
- **non-goals:** Features, dependency upgrades, commits/pushes, archive reads, tracked-file deletion, speculative redesigns.
- **files to touch:** Defect-specific source/tests; active documentation; scripts, justfile, and agent configuration as supported by findings.
- **ordered steps:** (1) Baseline checks and inventory. (2) Review source and decisions. (3) Fix clear defects with regression tests. (4) Add CPU-only repository QA and a quick-check tier, with synthetic script tests. (5) Correct docs and archive completed plans. (6) Run all gates and write the report.
- **done-when:** `just check`, `just script-test`, `just ctx`, and new recipes pass; GPU smoke passes or has a recorded environmental limitation; report records fixes, deferred findings, cleanup, tooling, client checks, and unavailable checks.

Context: branch `0.27`, initially clean. Baseline CPU/script/context gates pass. Preserve `input.json` and `tungsten.json` (runtime inputs). Do not read `docs/plans/archive/` or delete performance captures. The agentic-restructure plan retains owner/platform checks. Tooling will reuse existing Rust validation for manifest semantics and decision IDs; new Python checks should cover missing filesystem coverage, active-plan lifecycle, and documentation links without third-party dependencies.

Completed 2026-09-25. Results and open follow-ups: `docs/repo-review-2026-09-25.md`. All requested local gates passed; Claude and other-platform/remote checks are explicitly deferred.
