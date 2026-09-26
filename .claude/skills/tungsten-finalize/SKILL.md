---
name: tungsten-finalize
description: Prepare Tungsten milestone or patch branch documentation and cut the requested version when needed. Updates changelog, DESIGN status, indexes and active plans, then hands off to tungsten-release for Git publication or verification.
---

# tungsten-finalize

Use after the branch's last code change. This skill owns documentation and the version cut; [docs/releases.md](../../../docs/releases.md) owns Git handoff, publishing, verification and recovery. A preparation request ends at that handoff. Honor any broader authorization already given.

Follow [AGENTS.md](../../../AGENTS.md): read a whole doc before editing, except CHANGELOG, DECISIONS and DESIGN (find headings and read only the relevant section). Decisions are immutable; reversals add an entry and update the index. Never read or list the plan archive.

## Establish state

Identify the requested version and branch base before editing. Refresh the intended remote base as described in the release guide; do not assume local main is current. Inventory commits/diffs against that base and inspect active plans only. If the branch was already squash-merged, compare file trees as well as history so identical changes are not counted twice.

Run `just release-check`. Resolve actual version/status drift without discarding unrelated work. The workspace version changes only at the cut (`D-071`, `D-074`).

- Requested version above the workspace version: prepare `[Unreleased]`, then cut it once.
- Requested version already current with its changelog section: inspect its notes, Cargo.lock and DESIGN status; skip cutting again. If this is already published, new work belongs in `[Unreleased]` for a later version.
- Requested version older, unclear, or inconsistent with existing publication: inspect and resolve the intended release before changing versioned notes or cutting.

## Complete branch documentation

- Complete the relevant changelog section from the inventory. Use a `Summary:` naming the plan, applicable Added/Changed/Fixed/Removed groups, bold bullet leads and useful decision/path references. Cite new decisions only when there are any. Do not create a version heading manually.
- Add every new decision's row to [DECISION_INDEX.md](../../../docs/DECISION_INDEX.md). A reversal marks the old decision `Superseded by D-NNN` and updates both rows.
- Update [LLM_INDEX.md](../../../docs/LLM_INDEX.md) for new task areas and moved paths; keep it under 8 KiB.
- For plans completed by this branch, tick acceptance checks and set done/abandoned/superseded status, then move the known file to the archive without inspecting that directory. Repoint links. Keep unfinished plans active; update phase milestone status only when appropriate to the release state.

## Cut when needed

Confirm the intended version from the task: milestone branch `0.NN` normally ships `0.NN.0`; maintenance fixes increment the patch. Then run:

```bash
just release-cut X.Y.Z
```

The command rejects inconsistent files, empty `[Unreleased]`, non-increasing versions and dates before the latest release. `--date YYYY-MM-DD` selects an intentional release date. It updates Cargo.toml, changelog and DESIGN and refreshes Cargo.lock. If lock refresh fails, the cut files are already changed: fix the lockfile and rerun checks, not the cut.

Review the diff. Update DESIGN's surrounding status prose as needed. README remains version-free; edit its overview, commands or document links only if their substance changed. Preserve historical release prose and do not scan the archive for stale versions.

## Validate and hand off

Run the checks appropriate to the branch under AGENTS.md (`just check` for code, `just script-test` for scripts, and local hardware checks where required), then finish with:

```bash
just ctx
just repo-check
```

Report the requested/current version, whether a cut occurred, plans moved, validation and remaining work. Hand off to [tungsten-release](../tungsten-release/SKILL.md) and the release guide. Publication selects the final integrated commit after any merge; do not hand out a bare tag command that implicitly tags the development HEAD.
