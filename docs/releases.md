# Internal release procedure

Canonical release operations for people and coding agents (`D-071`, `D-072`, `D-074`). Start with the requested outcome: prepare a branch, publish a version, verify an existing release, rehearse, or recover a failure. Preparation does not imply publication or deletion. Follow authorization already given in the session; do not repeatedly ask for it.

## Sources and tools

| Concern | Source |
| --- | --- |
| Branch documentation and version cut | [tungsten-finalize](../.claude/skills/tungsten-finalize/SKILL.md) |
| Release orchestration and handoff | [tungsten-release](../.claude/skills/tungsten-release/SKILL.md) |
| Version, notes and packaging | [release.py](../scripts/release.py), [tests](../scripts/test-release.py) |
| Git/GitHub preflight | [release-preflight.py](../scripts/release-preflight.py), [tests](../scripts/test-release-preflight.py) |
| Hosted builds and publication | [release.yml](../.github/workflows/release.yml) |
| Launcher CPU selection | [launcher](../tools/launcher/src/main.rs) |

The workspace version equals the newest versioned changelog heading and the DESIGN status version. `just release-check [TAG]` validates those files, not Git tag objects; it works offline and in shallow CI. `just release-cut VERSION` moves `[Unreleased]` to a dated version section, updates Cargo.toml and DESIGN, and refreshes Cargo.lock. README has no release status and is never rewritten by the cut.

Git operations require Git; GitHub inspection requires authenticated `gh` and network access. Run from the repository root. Example commands below use Bash and these placeholders; replace the version with the intended release, and use the repository and remote actually being released:

```bash
release_version=X.Y.Z
release_tag="v$release_version"
release_remote=origin
release_branch=main
release_repo=JosephGibson/Tungsten
```

## Prepare and select the commit

1. Finalize the branch using `tungsten-finalize`. Inspect the requested version first: if it is already the current version with notes, verify that cut and skip another cut. New work after publication belongs in `[Unreleased]` for a later version. Do not bump versions when opening branches.
2. Review and commit the final changes, then integrate them into the intended release branch within the user's authorized scope. Normal milestone releases use the final merged commit on `main`; patch releases can explicitly select a maintenance branch. A squash merge creates a different commit, even when the files are identical. Tag the integrated commit after merging.
3. Refresh the selected branch without silently merging or rewriting local work:

   ```bash
   git fetch --no-tags "$release_remote" "refs/heads/$release_branch:refs/remotes/$release_remote/$release_branch"
   release_sha=$(git rev-parse "refs/remotes/$release_remote/$release_branch^{commit}")
   just release-preflight "$release_version" --ref "$release_sha" \
     --remote "$release_remote" --branch "$release_branch" --repo "$release_repo"
   ```

The preflight is read-only: it checks a clean working tree/index, unfinished Git operations, files at the selected commit, the live push destination, the remote-tracking branch, local/remote tag objects, GitHub repository state and runs for that commit. A new tag must name the live branch tip; an existing tag can be inspected at its original commit after the branch advances. Historical published tags outside main remain valid for verification. A prerelease without its own changelog section requires explicit `--rehearsal`.

A missing or stale ref calls for a fetch and inspection, not an automatic reset or force push. An existing conflicting tag calls for investigation, not replacement. The preflight prints proposed commands only after its checks pass. State can change after the check: rerun it if the commit, branch, tag or release changes before acting.

## Validate, tag and push

Run the checks required by [AGENTS.md](../AGENTS.md) on the contents being released: `just check`, `just ctx`, `just repo-check`, plus `just script-test` for scripts and `just smoke` for GPU wiring/non-trivial changes. Record the tested SHA; if testing a branch before a squash merge, compare its tree with the final commit and report that distinction. CI remains informational (`D-070`); inspect and report results for the selected SHA, not merely the latest green run. A release build does not perform GPU/audio validation.

Use the preflight's exact SHA and tag commands. For a new tag the shape is:

```bash
git tag -a "$release_tag" "$release_sha" -m "Tungsten $release_version"
git -c push.followTags=false push "$release_remote" "refs/tags/$release_tag:refs/tags/$release_tag"
```

Keep existing published tags fixed. If the same annotated tag already exists locally and remotely, resume verification/recovery; pushing it again does not start another workflow. Tagging alone does not publish; pushing the tag starts the release workflow. Use one explicit tag instead of pushing every local tag.

For an explicitly chosen workflow that publishes a local branch update and its tag together, Git supports an atomic push of those two named refs. It fails without updating either if the server rejects one or lacks atomic support. The ordinary preflight above requires the branch to be published first; do not bypass a failure by switching casually to this variant. It needs separate review of both proposed refs. See [git push](https://git-scm.com/docs/git-push).

## Watch and verify the exact run

Rerun preflight after pushing to obtain the matching run ID. If event delivery is still pending, query again after a short wait. For manual inspection:

```bash
gh run list --repo "$release_repo" --workflow release.yml --commit "$release_sha" \
  --event push --json databaseId,headSha,headBranch,status,conclusion,url
```

Select the row whose `headBranch` equals the tag and whose `headSha` equals the selected SHA. Use that numeric ID, not an interactive choice of the newest run:

```bash
release_run_id=RUN_ID
gh run watch "$release_run_id" --repo "$release_repo" --exit-status
gh release view "$release_tag" --repo "$release_repo"
git ls-remote "$release_remote" "refs/tags/$release_tag" "refs/tags/$release_tag^{}"
```

For an annotated tag, the `^{}` line identifies its commit; the other line identifies the tag object. GitHub release `target_commitish` is not proof of the tagged commit. Confirm the remote tag, run SHA and intended SHA agree.

The workflow builds the four examples for Linux (`x86_64-unknown-linux-gnu`, Ubuntu 24.04) and Windows (`x86_64-pc-windows-msvc`, static CRT). Each platform archive contains portable x86-64 and x86-64-v3 builds, launchers, runtime assets and README.txt. Launchers select the fastest supported CPU level and set the working directory to the archive root. Hosted runners only build.

Expect a Linux `.tar.gz`, a Windows `.zip`, and `SHA256SUMS`. Download into a fresh directory and verify both archives (requires `sha256sum`; use an equivalent SHA-256 tool on Windows):

```bash
release_download=$(mktemp -d)
gh release download "$release_tag" --repo "$release_repo" --dir "$release_download" \
  --pattern 'tungsten-examples-*' --pattern SHA256SUMS
(cd "$release_download" && sha256sum -c SHA256SUMS)
```

Check that both expected platform filenames are present, notes match the tag's changelog section, and prerelease classification matches the tag. Extract the appropriate archive and launch an example on a GPU machine, including from outside the archive directory. Its first console line reports the selected CPU level. `TUNGSTEN_CPU_LEVEL=x86-64` exercises the portable path; never force v3 on an unsupported CPU. Report untested platforms explicitly. Linux needs compatible Vulkan/windowing/audio libraries and may need glibc 2.39.

## Rehearsals and versioned prereleases

A prerelease tag with its own changelog section is a versioned release and must match the workspace version. A prerelease tag without that section is a rehearsal: the workspace still passes consistency checks, notes come from `[Unreleased]`, and the tag version need not match. Both publish as GitHub prereleases. A rehearsal may use the tip of a pushed development branch by setting `--branch` explicitly; it does not require a version cut.

Use a unique rehearsal tag per attempt, for example `v0.0.0-test.20260926.gabcdef1`, adding a suffix if needed. Record its SHA and run ID. For such a tag, pass the version without `v` and `--rehearsal` to preflight. Do not reuse a fixed test tag for a different commit. Cleanup is a separate requested action targeting only that rehearsal release/tag; inspect whether it is immutable before attempting deletion.

## Recover according to state

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

## Handoff and next branch

Report version/tag, commit SHA, release URL, run ID/result, checks performed, archive/checksum verification and outstanding platform/GPU checks. If only preparation was requested, report the cut/version and remaining merge/tag/publication steps instead.

When starting the next milestone, refresh `origin/main` and create the branch from it, rather than a stale local main or the pre-squash milestone branch. If the next branch already exists, inspect its history and work before integrating main; never reset it automatically. Preserve existing authorization and the repository's optional PR process.
