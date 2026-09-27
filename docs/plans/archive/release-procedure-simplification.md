# Release procedure simplification

- **status:** done
- **goal:** Make an ordinary milestone release a numbered, paste-ready command sequence in `docs/releases.md`, with one-line commit messages and thin skill entry points.
- **non-goals:** Run Git mutations or publish anything; alter the in-flight 0.29.0 release, version files, scripts, packaging, CI or repository settings; require PRs; automate plan-completion judgment.
- **files to touch:** `docs/releases.md`, `.claude/skills/tungsten-release/SKILL.md`, `.claude/skills/tungsten-finalize/SKILL.md`, this plan (archive on completion).
- **ordered steps:**
  1. Inspect release state and read the release guide, skills, tooling and D-071/D-072/D-074.
  2. Show the proposed happy-path command list for review before implementing it.
  3. Lead the guide with commands for cut/check/commit/push, local squash integration, SHA refresh, read-only preflight, exact-SHA tag push and matching-run verification.
  4. Consolidate branch preparation here; keep plan archival and link repair manual, and specify branch-free DESIGN status for future preparation. Preserve recovery, rehearsal and artifact details in reference sections.
  5. Reduce both skills to scope/routing and the human-pasted Git handoff.
  6. Validate, mark this plan done and move the known file to the archive without reading or listing that directory.
- **done-when:**
  - [x] Guide leads with one numbered command sequence and explicit placeholders.
  - [x] One-line commit convention and human-only Git mutations are explicit.
  - [x] Exact verified SHA, read-only preflight, fixed published tags and unique rehearsals remain required.
  - [x] Recovery and archive/asset verification knowledge remains available below the happy path.
  - [x] Both skills link to the guide without duplicating its procedure.
  - [x] `just ctx`, `just repo-check`, `just script-test` and applicable checks pass.

## Context and decisions

Initial tree: clean on `0.29`, tracking `origin/0.29`, at `f62b952` (Update 0.29: platformer art and gameplay revamp). `just release-check` passes at 0.29.0 with empty `[Unreleased]`. Live `main` and annotated `v0.29.0` resolve to `9d181f69be7df9299a671b0cf2faa34102c02065`; release run 36297491598 is in progress and no release is visible yet. Local `main` is stale. Do not touch release contents or Git refs.

No new decision: this consolidates presentation and documents the existing single-line history convention while preserving D-071/D-072/D-074. Branch names are not part of the workspace/changelog/DESIGN version invariant. Future branch preparation should omit that redundant label. Plan archival stays with documentation preparation because the cut cannot infer completion or choose meaningful replacement links.

## Result

The proposed local-squash sequence was shown for review before implementation. The guide now leads with seven numbered blocks, preserves the tested tree across squash integration, and visibly re-reads the published SHA before read-only preflight and explicit-SHA tagging. Both skills are ten-line entry points. Detailed preparation, resumption, recovery and artifact verification remain in the guide's reference section.

Validation passed: `just ctx`, `just repo-check`, `just script-test`, `just check`, both skill frontmatter validators, `git diff --check`, and Bash syntax validation of all eight documented command blocks without executing them. No source, version files, release scripts, Git refs or remote state changed. GPU smoke was unnecessary for this documentation-only change.
