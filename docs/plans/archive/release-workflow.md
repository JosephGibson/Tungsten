# Internal release workflow

- **status:** done
- **goal:** Move release operations out of the README and make agent-driven releases identify and verify the exact Git commit.
- **non-goals:** Cut a version, publish a release, rewrite existing tags, change runtime code, or impose required CI/PR rules.
- **files to touch:** README.md, AGENTS.md, DECISIONS.md, CHANGELOG.md, docs/releases.md, docs/LLM_INDEX.md, docs/DECISION_INDEX.md, docs/agent-setup.md, .claude/skills/tungsten-finalize/SKILL.md, .claude/skills/tungsten-release/SKILL.md, .agents/skills/tungsten-release, .gitignore, scripts/release.py, scripts/test-release.py, scripts/release-preflight.py, scripts/test-release-preflight.py, scripts/check-repo.py, justfile, .github/workflows/ci.yml, .github/workflows/release.yml.
- **ordered steps:** Add canonical guide and focused skill; decouple README from release cuts; implement read-only Git/GitHub preflight; document publication/recovery by state; update decisions and navigation; validate and archive this plan.
- **done-when:** Release-free README, shared discoverable skill, committed-tree and live-ref preflight with temporary-repo regressions, and passing skill/context/repository/script/CPU/GPU checks.

## Context

The existing finalize skill cuts the version, but publishing instructions live in the README and release.py requires its version line. D-074 replaces that clause of D-071. The v0.28.0 tag and squash-merged main have identical trees but different commits; future milestone tags should name the final merged commit. The clean 0.29 branch was fast-forwarded from v0.27.0 to current origin/main before implementation. Keep release-check usable offline and in shallow CI; Git/network checks belong in a separate read-only preflight.

## Checks

- [x] Guide, skills and README migration complete.
- [x] Preflight validates commit contents, branch/tag refs and GitHub state without mutation.
- [x] Regression tests cover stale refs, squash divergence, dirty trees, conflicts, rehearsal and resume paths.
- [x] Skill validation, just script-test, just ctx, just repo-check, just check and just smoke pass.

Validation: 19 Git preflight regressions, 23 release-script tests, full script/context/repository/CPU checks and all GPU smoke fixture matrices passed. Live read-only preflight against v0.28.0 in a temporary checkout identified the published release and exact successful run. No release was cut, tagged, pushed or published.
