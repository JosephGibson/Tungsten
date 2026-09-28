# Internal release procedure

Canonical procedure (`D-071`, `D-072`, `D-074`). **The human pastes the Git commands.** Global `~/.claude/settings.json` denies agent `git add/commit/push/fetch/tag/merge/checkout`; agents prepare files, run checks and supply commands, without bypassing those denies through another command or API.

## Ordinary milestone release

Start at the repo root on the finished milestone branch, with [branch documentation prepared](#branch-preparation) and the intended changes reviewed. Use Git, authenticated `gh`, network access and a GPU/display for smoke tests. Replace `X.Y.Z`, `0.NN`, `MNN short summary` and the run ID; adjust remote/repository if needed. Paste **one numbered block at a time in the same Bash terminal**, stopping on any failure. If already cut, merged or tagged, [resume from that state](#resume-or-use-another-branch).

**Commit messages are one subject line, with no body:** `Update 0.NN: short summary`. For example, `Update 0.28: M30 parallax, screen-shake, squash/stretch (#29)`. Include `(#PR)` once only when there is a PR; PRs are optional. The commands below use a local squash merge.

1. Set the release details.

   ```bash
   release_version='X.Y.Z'
   milestone_branch='0.NN'
   release_message='Update 0.NN: MNN short summary'
   release_remote=origin
   release_repo=JosephGibson/Tungsten
   release_tag="v$release_version"
   ```

2. Cut once, validate and review the changed-file list.

   ```bash
   test "$(git branch --show-current)" = "$milestone_branch" &&
   just release-cut "$release_version" &&
   just check && just ctx && just repo-check &&
   just script-test && just smoke &&
   git diff --check && git status --short && git diff --stat
   ```

3. Commit the reviewed changes with one line and push the milestone branch.

   ```bash
   git add -A &&
   git commit -m "$release_message" &&
   git -c push.followTags=false push "$release_remote" \
     "HEAD:refs/heads/$milestone_branch" &&
   release_tested_sha=$(git rev-parse HEAD)
   ```

4. Squash into refreshed `main`; require the tested tree before committing.

   ```bash
   git fetch --no-tags "$release_remote" \
     "refs/heads/main:refs/remotes/$release_remote/main" &&
   git switch main &&
   git merge --ff-only "refs/remotes/$release_remote/main" &&
   git merge --squash "$release_tested_sha" &&
   git diff --cached --check &&
   test "$(git write-tree)" = "$(git rev-parse "$release_tested_sha^{tree}")" &&
   git commit -m "$release_message" &&
   git -c push.followTags=false push "$release_remote" HEAD:refs/heads/main
   ```

5. **RE-READ THE SHA HERE.** The squash commit has a new identity. Fetch it after the push, run the read-only preflight, then tag exactly that SHA.

   ```bash
   git fetch --no-tags "$release_remote" \
     "refs/heads/main:refs/remotes/$release_remote/main" &&
   release_sha=$(git rev-parse "refs/remotes/$release_remote/main^{commit}") &&
   test "$release_sha" = "$(git rev-parse HEAD)" &&
   just release-preflight "$release_version" --ref "$release_sha" \
     --remote "$release_remote" --branch main --repo "$release_repo" &&
   git tag -a "$release_tag" "$release_sha" -m "Tungsten $release_version" &&
   git -c push.followTags=false push "$release_remote" \
     "refs/tags/$release_tag:refs/tags/$release_tag"
   ```

6. Find the row whose `headBranch` is the tag and `headSha` is `release_sha`. If delivery is pending, repeat this lookup after a short wait.

   ```bash
   gh run list --repo "$release_repo" --workflow release.yml \
     --commit "$release_sha" --event push \
     --json databaseId,headSha,headBranch,status,conclusion,url
   ```

7. Replace the run ID with that row's `databaseId`, wait, and inspect the release and remote tag.

   ```bash
   release_run_id='RUN_ID_FROM_STEP_6'
   gh run watch "$release_run_id" --repo "$release_repo" --exit-status &&
   gh release view "$release_tag" --repo "$release_repo" &&
   git ls-remote "$release_remote" \
     "refs/tags/$release_tag" "refs/tags/$release_tag^{}"
   ```

The remote tag's `^{}` line must equal `release_sha`. Finish with the [archive/asset checks](#archive-and-asset-verification); hosted builds do not test GPU/audio behavior. Keep published tags fixed: never move or force-push them. Always tag the exact preflight-verified SHA, never bare `HEAD`.

## Reference

### Branch preparation

[tungsten-finalize](../.claude/skills/tungsten-finalize/SKILL.md) handles this docs pass and the requested cut; [tungsten-release](../.claude/skills/tungsten-release/SKILL.md) handles the Git handoff and release inspection. This guide owns both procedures. Preparation alone does not authorize publication or deletion; honor the requested scope and existing authorization without asking again.

- Establish the requested version and refreshed branch base; have the human fetch if needed. Inventory commits/diffs and active plans. If already squash-merged, compare trees as well as history to avoid counting the same changes twice.
- Run `just release-check`. The workspace version must equal the newest versioned changelog heading and DESIGN's status version. It stays unchanged until the cut; milestone `0.NN` normally ships `0.NN.0`, maintenance increments the patch. Resolve drift without discarding unrelated work.
- Prepare `[Unreleased]` from the inventory: a `Summary:` naming the plan, applicable Added/Changed/Fixed/Removed groups, bold bullet leads and useful decision/path references. Do not create a release heading manually. If the requested version is already current with notes, inspect those notes, Cargo.lock and DESIGN and skip cutting again. After publication, new work goes in `[Unreleased]` for a later version. Resolve older or ambiguous version requests before editing released notes.
- During future preparation, use ``Workspace `vX.Y.Z`; release summary`` in DESIGN's status line; **omit `on branch …`**. A branch name goes stale and is not part of the version invariant. The cut updates the version token only; review the surrounding prose. README stays version-free; update its substance only when needed.
- Add any new decision's row to [DECISION_INDEX.md](DECISION_INDEX.md). Reversals add a decision, mark the old one `Superseded by D-NNN`, and update both rows. Update [LLM_INDEX.md](LLM_INDEX.md) for changed task areas/paths; keep it under 8 KiB.
- **Plan archival stays manual, before the cut.** Review acceptance checks, mark completed/abandoned/superseded plans, fix their relative links for the destination, move each known file to the archive, and repoint incoming links. Never read, search or list the archive. Keep unfinished plans active and update phase status appropriately. The cut cannot infer completion or choose replacement links; `just repo-check` checks active plans and non-archive links, so verify archive destinations by construction.

`just release-cut VERSION [--date YYYY-MM-DD]` moves `[Unreleased]` into a dated section, updates Cargo.toml and DESIGN, and refreshes Cargo.lock. It rejects inconsistent files, empty notes, non-increasing versions and dates before the last release. If lock refresh fails, the cut files are already changed: fix Cargo.lock and rerun checks, not the cut. README is never rewritten.

Step 2 covers ordinary milestone checks under [AGENTS.md](../AGENTS.md); narrow documentation-only work can omit script/GPU checks when those rules allow it. Report `release_tested_sha` and the final SHA separately: the squash must preserve the tested tree. If integration changes the tree, resolve it and test the resulting contents before publication. CI remains informational (`D-070`); report results for the selected SHA and outstanding hardware checks.

### Resume or use another branch

| State | Resume point |
| --- | --- |
| Version already cut, not committed | Skip the cut in step 2; review and validate the existing cut. |
| Cut committed and milestone branch pushed | Validate it, set `release_tested_sha` to its commit SHA, then step 4. |
| Already squash-merged through GitHub | Skip the local squash. Fetch main, select its final integrated SHA and compare its tree with the tested milestone commit; run step 5's preflight/tag/push using that SHA. No requirement to switch a stale local main. |
| Tag already exists | Inspect at its original commit using preflight; use only the proposed missing push/watch/view commands. Do not cut or create the tag again. |
| Maintenance or rehearsal branch | Explicitly select that pushed branch with `--branch`; fetch its tracking ref and select its SHA. Milestone tags use final integrated main. |

`just release-check [TAG]` checks files offline, including shallow CI; it does not inspect Git tag objects. `just release-preflight VERSION --ref SHA --remote REMOTE --branch BRANCH --repo OWNER/REPO` checks committed files, a clean tree/index, unfinished Git operations, the live push destination and tracking ref, tag objects, GitHub repository state and matching release/CI runs. **Preflight stays read-only** and prints proposed commands only on success. It needs the branch already pushed and the final SHA, so it runs near the end.

New tags must name the live branch tip. Existing tags can be verified at their original commit after the branch advances, including historical tags outside main. Fetch/inspect stale refs or divergent local main; never reset automatically. Investigate conflicting tags without replacing them. Rerun preflight if the commit, branch, tag or release changes before acting. An unchanged tag push does not start another workflow; push only the named tag.

An explicitly chosen atomic branch-and-tag push updates both named refs or neither if rejected/unsupported. It needs separate review of both refs; ordinary preflight requires an already-published branch, so this is not a workaround for a failed check. See [git push](https://git-scm.com/docs/git-push).

### Archive and asset verification

Preflight after the tag push can also report the matching run ID. Match both tag and SHA, and watch that numeric ID rather than choosing the newest run interactively.

For an annotated tag, the `^{}` line identifies its commit; the other line identifies the tag object. GitHub release `target_commitish` is not proof of the tagged commit. Confirm the remote tag, run SHA and intended SHA agree.

The workflow builds the four examples for Linux (`x86_64-unknown-linux-gnu`, Ubuntu 24.04) and Windows (`x86_64-pc-windows-msvc`, static CRT). Each platform archive contains portable x86-64 and x86-64-v3 builds, launchers, runtime assets and README.txt. Launchers select the fastest supported CPU level and set the working directory to the archive root. Hosted runners only build.

Expect a Linux `.tar.gz`, a Windows `.zip`, and `SHA256SUMS`. Download into a fresh directory and verify both archives (requires `sha256sum`; use an equivalent SHA-256 tool on Windows):

```bash
release_download=$(mktemp -d) &&
gh release download "$release_tag" --repo "$release_repo" --dir "$release_download" \
  --pattern 'tungsten-examples-*' --pattern SHA256SUMS &&
(cd "$release_download" && sha256sum -c SHA256SUMS)
```

Check that both expected platform filenames are present, notes match the tag's changelog section, and prerelease classification matches the tag. Extract the appropriate archive and launch an example on a GPU machine, including from outside the archive directory. Its first console line reports the selected CPU level. `TUNGSTEN_CPU_LEVEL=x86-64` exercises the portable path; never force v3 on an unsupported CPU. Report untested platforms explicitly. Linux needs compatible Vulkan/windowing/audio libraries and may need glibc 2.39.

### Rehearsals and versioned prereleases

A prerelease tag with its own changelog section is a versioned release and must match the workspace version. A prerelease tag without that section is a rehearsal: the workspace still passes consistency checks, notes come from `[Unreleased]`, and the tag version need not match. Both publish as GitHub prereleases. A rehearsal may use the tip of a pushed development branch by setting `--branch` explicitly; it does not require a version cut.

Use a unique rehearsal tag per attempt, for example `v0.0.0-test.20260926.gabcdef1`, adding a suffix if needed. Record its SHA and run ID. For such a tag, pass the version without `v` and `--rehearsal` to preflight. Do not reuse a fixed test tag for a different commit. Cleanup is a separate requested action targeting only that rehearsal release/tag; inspect whether it is immutable before attempting deletion.

### Recover according to state

Always inspect the identified run and GitHub release first. Network/authentication errors are not evidence that a release is absent. Reruns use the original commit and ref; editing a workflow on a later commit cannot repair that run. [GitHub rerun behavior](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)

| Observed state | Next action |
| --- | --- |
| Build failed; no release exists | Inspect `gh run view RUN_ID --log-failed`. For a transient failure, rerun that run's failed jobs with `gh run rerun RUN_ID --failed`. A source/workflow fix needs a new commit and a new version/tag, or a unique rehearsal tag. |
| Tag exists; no matching run | Verify the push event and workflow in the tagged commit. Wait briefly for delivery; an unchanged tag push is not a retry mechanism. |
| Publish failed; no release exists | Inspect logs, then rerun the failed publish job if its build artifacts still exist. Artifacts have seven-day retention; expired artifacts require rebuilding via a full rerun if GitHub still allows it. |
| Draft release or partial upload exists | Verify the tag/commit and inspect assets. The workflow uses `gh release create`, so it cannot automatically resume an existing draft. Resume the draft with verified assets from the same run and publish it once complete, or delete only the draft (keep the tag) if that cleanup is authorized, then rerun. Do not blindly clobber uploaded assets. |
| Release is already published | Verify the tag, notes, expected assets and checksums. A lost response or failed rerun may follow successful publication. Stop if complete. An incomplete published release needs an explicit repair decision after checking immutability; do not delete/recreate it by default. |
| Local and remote tags disagree | Stop publication and identify which object was published. Do not force-push or move a published tag. |

Current `gh release create` with assets creates a draft, uploads assets, then publishes. Drafts are mutable; published immutable releases lock their assets and tag. Account for that boundary during recovery. See [GitHub CLI release creation](https://cli.github.com/manual/gh_release_create). Rerun once for an identified transient problem, inspect the result, and stop repeating the same failing action without new evidence.

### Handoff and next branch

Report version/tag, commit SHA, release URL, run ID/result, checks performed, archive/checksum verification and outstanding platform/GPU checks. For preparation-only work, report requested/current versions, whether a cut occurred, plans moved, checks and the remaining numbered Git steps.

When starting the next milestone, refresh `origin/main` and create the branch from it, rather than a stale local main or the pre-squash milestone branch. If the next branch already exists, inspect its history and work before integrating main; never reset it automatically. Preserve existing authorization and the repository's optional PR process.

### Tool sources

| Concern | Source |
| --- | --- |
| Version, notes and packaging | [release.py](../scripts/release.py), [tests](../scripts/test-release.py) |
| Read-only Git/GitHub preflight | [release-preflight.py](../scripts/release-preflight.py), [tests](../scripts/test-release-preflight.py) |
| Recipes | [justfile](../justfile) |
| Hosted builds and publication | [release.yml](../.github/workflows/release.yml) |
| Launcher CPU selection | [launcher](../tools/launcher/src/main.rs) |
