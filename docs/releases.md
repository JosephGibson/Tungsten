# Internal release procedure

Canonical procedure (`D-071`, `D-072`, `D-074`, `D-079`). **The agent runs every check; the human pastes a short block of plain Git commands and approves the pull request in GitHub.** Global `~/.claude/settings.json` denies agent `git add/commit/push/fetch/tag/merge/checkout`; agents prepare files, run checks and supply commands, without bypassing those denies through another command or API. The same holds for `gh` commands that change GitHub (`gh pr create`, `gh workflow run`, reruns, deletions): agents run only the read-only ones.

**Every release task ends with the exact commands the human still has to run**, values filled in, as one block. Keep them basic: commit, tag, push, open the pull request. A second block follows for after the merge: it starts the next milestone branch. Nothing is published until the human merges that pull request.

## Ordinary milestone release

Start at the repo root on the finished milestone branch `0.NN`, with [branch documentation prepared](#branch-preparation). On the road to 1.0 the release is a milestone plan's last step, run in the session that ran the plan, so the cut happens once and the checks run once, after it ([workflow](plans/1.0/workflow.md) §1). A separate release session runs it to resume, or for a release with no milestone plan. Use Git, authenticated `gh`, network access and a GPU/display for smoke tests. Replace `X.Y.Z`, `0.NN` and the summary; adjust remote/repository if needed. If already cut, committed, tagged or merged, [resume from that state](#resume-or-use-another-branch).

**Commit messages are one subject line, with no body:** `Update 0.NN: short summary`. For example, `Update 0.28: M30 parallax, screen-shake, squash/stretch (#29)`. The pull request takes the same title, and GitHub's squash appends `(#PR)` once.

1. **Agent: cut once and run every check.** All of it passes before anything is handed over. Review the changed-file list: the commit takes everything `git status --short` shows.

   ```bash
   just release-cut X.Y.Z &&
   just check && just ctx && just repo-check && just api-check &&
   just script-test && just smoke &&
   git diff --check && git status --short && git diff --stat
   ```

2. **Agent: run the read-only preflight.** With `--message` it accepts the uncommitted cut as the release commit, checks the files, the tag, the remote branch and that `main` is contained in the branch, and prints step 3's commands, then step 4's post-merge block.

   ```bash
   just release-preflight X.Y.Z --repo JosephGibson/Tungsten \
     --message 'Update 0.NN: short summary'
   ```

3. **Human: paste the commands the agent ended with**, one block in the repo root.

   ```bash
   git add -A
   git commit -m 'Update 0.NN: short summary'
   git tag -a vX.Y.Z -m 'Tungsten X.Y.Z'
   git push origin 0.NN vX.Y.Z
   gh pr create --repo JosephGibson/Tungsten --base main --head 0.NN \
     --title 'Update 0.NN: short summary' \
     --body 'Release vX.Y.Z. Merging this pull request publishes it.'
   ```

4. **Human: approve and merge the pull request in GitHub, then paste the post-merge block.** Squash-merge, keep the title as the subject and clear the description. The merge starts [release.yml](../.github/workflows/release.yml), which builds the tagged commit and publishes the GitHub Release. CI reports on the pull request beforehand (informational, `D-070`). The post-merge block, printed under step 3's, starts the [next branch](#handoff-and-next-branch), where `0.<NN+1>` is the next minor:

   ```bash
   git fetch origin
   git switch -c 0.<NN+1> --no-track origin/main
   git push -u origin 0.<NN+1>
   ```

5. **Verify when asked.** Preflight reports the pull request, the run and the release, and prints the matching watch/view command. Finish with the [archive/asset checks](#archive-and-asset-verification); hosted builds do not test GPU/audio behavior.

   ```bash
   just release-preflight X.Y.Z --repo JosephGibson/Tungsten
   ```

The tag names the tested milestone commit. `main`'s squash commit has the same tree, which the workflow requires, but a different identity. A tag is fixed once its GitHub Release exists: never move or force-push it.

## Reference

### Branch preparation

[tungsten-finalize](../.claude/skills/tungsten-finalize/SKILL.md) handles this docs pass and the requested cut; [tungsten-release](../.claude/skills/tungsten-release/SKILL.md) handles the checks, the command hand-off and release inspection. This guide owns both procedures. Preparation alone does not authorize publication or deletion; honor the requested scope and existing authorization without asking again.

- Establish the requested version and refreshed branch base; have the human fetch if needed. Inventory commits/diffs and active plans. If already squash-merged, compare trees as well as history to avoid counting the same changes twice.
- Run `just release-check`. The workspace version must equal the newest versioned changelog heading and DESIGN's status version. It stays unchanged until the cut; milestone `0.NN` normally ships `0.NN.0`, maintenance increments the patch. Resolve drift without discarding unrelated work.
- Prepare `[Unreleased]` from the inventory: a `Summary:` naming the plan, applicable Added/Changed/Fixed/Removed groups, bold bullet leads and useful decision/path references. Do not create a release heading manually. If the requested version is already current with notes, inspect those notes, Cargo.lock and DESIGN and skip cutting again. After publication, new work goes in `[Unreleased]` for a later version. Resolve older or ambiguous version requests before editing released notes.
- During future preparation, use ``Workspace `vX.Y.Z`; release summary`` in DESIGN's status line; **omit `on branch …`**. A branch name goes stale and is not part of the version invariant. The cut updates the version token only; review the surrounding prose. README stays version-free; update its substance only when needed.
- Add any new decision's row to [DECISION_INDEX.md](DECISION_INDEX.md). Reversals add a decision, mark the old one `Superseded by D-NNN`, and update both rows. Update [LLM_INDEX.md](LLM_INDEX.md) for changed task areas/paths; keep it under 12 KiB (`D-106`).
- **Plan archival stays manual, before the cut.** Review acceptance checks, mark completed/abandoned/superseded plans, fix their relative links for the destination, move each known file to the archive, and repoint incoming links. Never read, search or list the archive. Keep unfinished plans active and update phase status appropriately. The cut cannot infer completion or choose replacement links; `just repo-check` checks active plans and non-archive links, so verify archive destinations by construction.

`just release-cut VERSION [--date YYYY-MM-DD]` moves `[Unreleased]` into a dated section, updates Cargo.toml and DESIGN, and refreshes Cargo.lock. It rejects inconsistent files, empty notes, non-increasing versions and dates before the last release. If lock refresh fails, the cut files are already changed: fix Cargo.lock and rerun checks, not the cut. README is never rewritten.

Step 1 covers ordinary milestone checks under [AGENTS.md](../AGENTS.md); narrow documentation-only work can omit script/GPU checks when those rules allow it. The checks run on the working tree that step 3 commits, so change nothing in between. The merge must keep the tagged tree: if `main` gained commits the branch lacks, the human merges `main` into the branch (`git fetch origin`, `git merge origin/main`) and the checks run again before the hand-off. CI remains informational (`D-070`); report its result for the tagged commit and outstanding hardware checks.

### Resume or use another branch

In any state, `just release-preflight X.Y.Z --repo OWNER/REPO` prints the commands that remain.

| State | Resume point |
| --- | --- |
| Version already cut, not committed | Skip the cut in step 1; run the checks on the existing cut, then step 2. |
| Cut committed, not tagged or pushed | Run the checks on that commit; preflight without `--message` prints the tag, push and pull-request commands. |
| Branch and tag pushed, no pull request | Preflight prints `gh pr create`. |
| Pull request open | Step 4: the human approves and merges it. |
| Pull request merged | Step 5. If no run started, see [recovery](#recover-according-to-state). |
| Release already exists | Inspect it at the tag's original commit (`--ref SHA`); do not cut or tag again. |
| Maintenance branch, no pull request | `--branch NAME --no-pr`: tag, push, then `gh workflow run release.yml --ref vX.Y.Z` starts the run. |

Release runs start three ways (`D-079`):

| Trigger | Publishes |
| --- | --- |
| A pull request merges into `main` and `v<workspace version>` names its head commit | That final release, after checking that the merge commit has the tagged tree |
| A `vX.Y.Z-<pre>` tag is pushed | That prerelease or [rehearsal](#rehearsals-and-versioned-prereleases) |
| `gh workflow run release.yml --ref TAG` | That tag: maintenance releases and recovery |

Pushing a final `vX.Y.Z` tag publishes nothing by itself. A merged pull request whose version is already released publishes nothing; one whose version is unreleased but whose head the tag does not name fails the run.

`just release-check [TAG]` checks files offline, including shallow CI; it does not inspect Git tag objects. `just release-preflight VERSION --repo OWNER/REPO [--branch B] [--base main] [--ref SHA] [--message TEXT] [--no-pr] [--rehearsal]` checks the files (committed ones, or the working tree with `--message`), a clean tree/index otherwise, unfinished Git operations, the live push destination and tracking ref, tag objects, that the base branch is contained, GitHub repository state, the pull request and matching release/CI runs. `--branch` defaults to the checked-out branch. **Preflight stays read-only** and prints proposed commands only on success.

A new tag is pushed together with its branch, as a fast-forward. Existing tags can be verified at their original commit after the branch advances, including historical tags outside main. Fetch/inspect stale refs or divergent local branches; never reset automatically. Investigate conflicting tags without replacing them. Rerun preflight if the commit, branch, tag, pull request or release changes before acting. Pushing an unchanged tag again never starts a run; push only the named branch and tag.

### Archive and asset verification

Preflight reports the matching run ID. A merged pull request's run is listed under the milestone branch and the tagged commit; a tag run under the tag. Watch that numeric ID rather than choosing the newest run interactively:

```bash
gh run watch RUN_ID --repo JosephGibson/Tungsten --exit-status &&
gh release view vX.Y.Z --repo JosephGibson/Tungsten &&
git ls-remote origin 'refs/tags/vX.Y.Z*'
```

For an annotated tag, the `^{}` line identifies its commit; the other line identifies the tag object. GitHub release `target_commitish` is not proof of the tagged commit. Confirm the remote tag, run SHA and intended SHA agree.

The workflow builds the four examples for Linux (`x86_64-unknown-linux-gnu`, Ubuntu 24.04) and Windows (`x86_64-pc-windows-msvc`, static CRT). Each platform's player archive contains portable x86-64 and x86-64-v3 builds, launchers, runtime assets and README.txt, and no debug files. Its debug archive, `tungsten-debug-vX.Y.Z-<target>`, holds each example build's debug file (Linux `bin/<level>/<example>.debug`, split off by `release.py package`; Windows `bin/<level>/<example>.pdb`) under the same top folder, so it extracts over the player archive (`D-120`). Launchers select the fastest supported CPU level and set the working directory to the archive root. Hosted runners have no GPU, so nothing with a window runs there; each build job's `Crash-report probe` step runs `tools/crash-probe` from an archive-like copy and symbolizes its crash file, informational only (`continue-on-error`, `D-120`).

Expect a Linux `.tar.gz` and a Windows `.zip` of each prefix, `tungsten-examples-*` and `tungsten-debug-*`, and `SHA256SUMS`, which lists all four. Download the player archives into a fresh directory and verify them; `--ignore-missing` skips the debug archives left out and still fails when no listed file is present (requires `sha256sum`; use an equivalent SHA-256 tool on Windows):

```bash
release_download=$(mktemp -d) &&
gh release download vX.Y.Z --repo JosephGibson/Tungsten --dir "$release_download" \
  --pattern 'tungsten-examples-*' --pattern SHA256SUMS &&
(cd "$release_download" && sha256sum -c --ignore-missing SHA256SUMS)
```

The debug archives are a separate download, for symbolizing a crash report; with them present every listed file is checked:

```bash
gh release download vX.Y.Z --repo JosephGibson/Tungsten --dir "$release_download" \
  --pattern 'tungsten-debug-*' &&
(cd "$release_download" && sha256sum -c SHA256SUMS)
```

Check that all four archive names are present, notes match the tag's changelog section, and prerelease classification matches the tag. Extract the appropriate archive and launch an example on a GPU machine, including from outside the archive directory. On Linux its first console line reports the selected CPU level; Windows builds open no console, and each run's log and any crash report go to `%LOCALAPPDATA%\tungsten-examples\logs` (Linux: `~/.local/state/tungsten-examples/logs`, `D-119`). `TUNGSTEN_CPU_LEVEL=x86-64` exercises the portable path; never force v3 on an unsupported CPU. Report untested platforms explicitly. Linux needs compatible Vulkan/windowing/audio libraries and may need glibc 2.39.

### Crash-report check

On a GPU machine, check that a shipped build's crash file symbolizes from its debug archive. Extract the Linux player and debug archives (downloaded above) into one folder, run an example with `TUNGSTEN_TEST_PANIC=1`, which panics in its first frame, then symbolize the crash file against that folder:

```bash
crash_check=$(mktemp -d) && target=x86_64-unknown-linux-gnu &&
tar -xzf "$release_download/tungsten-examples-vX.Y.Z-$target.tar.gz" -C "$crash_check" &&
tar -xzf "$release_download/tungsten-debug-vX.Y.Z-$target.tar.gz" -C "$crash_check" &&
root="$crash_check/tungsten-examples-vX.Y.Z-$target" &&
{ TUNGSTEN_USER_DIR="$crash_check/user" TUNGSTEN_TEST_PANIC=1 "$root/example-01-platformer" || true; } &&
python3 -B scripts/crash-report.py symbolize "$crash_check"/user/logs/*-crash.txt \
  --root "$root" --expect crates/tungsten/src/
```

It first checks the crash file's `build_id` against the debug file, then prints `N: function at file:line` per frame, and exits 1 on a mismatch or when no frame resolves in `crates/tungsten/src/`. The run's two `Crash-report probe` steps give the same verdict for the probe on each runner; read them on a rehearsal tag before merging.

### Rehearsals and versioned prereleases

A prerelease tag with its own changelog section is a versioned release and must match the workspace version. A prerelease tag without that section is a rehearsal: the workspace still passes consistency checks, notes come from `[Unreleased]`, and the tag version need not match. Both publish as GitHub prereleases as soon as the tag is pushed, without a pull request. A rehearsal may use the tip of a development branch by setting `--branch` explicitly; it does not require a version cut.

Use a unique rehearsal tag per attempt, for example `v0.0.0-test.20260926.gabcdef1`, adding a suffix if needed. Record its SHA and run ID. For such a tag, pass the version without `v` and `--rehearsal` to preflight. Do not reuse a fixed test tag for a different commit. Cleanup is a separate requested action targeting only that rehearsal release/tag; inspect whether it is immutable before attempting deletion.

### Recover according to state

Always inspect the identified run and GitHub release first. Network/authentication errors are not evidence that a release is absent. Reruns use the original commit and ref; editing a workflow on a later commit cannot repair that run. [GitHub rerun behavior](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)

| Observed state | Next action |
| --- | --- |
| Pull request merged or tag pushed; no matching run | `gh workflow run release.yml --ref vX.Y.Z` starts one (preflight prints it). Pushing the unchanged tag again is not a retry mechanism. |
| Run failed: the tag does not name the merged head | Commits reached the branch after tagging. Run the checks on the merged head. With no GitHub Release for the tag, the human re-points it (`git tag -d vX.Y.Z`, `git push origin :refs/tags/vX.Y.Z`, `git tag -a vX.Y.Z SHA -m 'Tungsten X.Y.Z'`, `git push origin vX.Y.Z`) and starts the run manually. Do the same before merging when a fix follows the hand-off. |
| Run failed: the merge commit differs from the tagged tree | `main` held commits the branch lacked. Test `main`'s merge commit locally, then re-point the unreleased tag to it as above and start the run manually, or ship the difference as the next patch version. |
| Build failed; no release exists | Inspect `gh run view RUN_ID --log-failed`. For a transient failure, rerun that run's failed jobs with `gh run rerun RUN_ID --failed`. A source/workflow fix needs a new commit and a new version/tag, or a unique rehearsal tag. |
| Publish failed; no release exists | Inspect logs, then rerun the failed publish job if its build artifacts still exist. Artifacts have seven-day retention; expired artifacts require rebuilding via a full rerun if GitHub still allows it. |
| Draft release or partial upload exists | Verify the tag/commit and inspect assets. The workflow uses `gh release create`, so it cannot automatically resume an existing draft. Resume the draft with verified assets from the same run and publish it once complete, or delete only the draft (keep the tag) if that cleanup is authorized, then rerun. Do not blindly clobber uploaded assets. |
| Release is already published | Verify the tag, notes, expected assets and checksums. A lost response or failed rerun may follow successful publication. Stop if complete. An incomplete published release needs an explicit repair decision after checking immutability; do not delete/recreate it by default. |
| Local and remote tags disagree | Stop publication and identify which object was pushed. Never move a tag whose release exists. |

Current `gh release create` with assets creates a draft, uploads assets, then publishes. Drafts are mutable; published immutable releases lock their assets and tag. Account for that boundary during recovery. See [GitHub CLI release creation](https://cli.github.com/manual/gh_release_create). Rerun once for an identified transient problem, inspect the result, and stop repeating the same failing action without new evidence.

### Handoff and next branch

End the task with the remaining command block and the post-merge block, after a short report: version/tag, the checks run and their results, anything not checked (platforms, GPU) and what the commands will do. A milestone's run session then gives the prompt for the next candidate's plan ([workflow](plans/1.0/workflow.md) §3). After the merge, report the tagged commit SHA, pull request and release URLs, run ID/result and archive/checksum verification. For preparation-only work, report requested/current versions, whether a cut occurred, plans moved, checks and the remaining steps.

The post-merge block starts the next milestone: it refreshes `origin/main`, creates the branch from it without tracking it and publishes it (`git fetch origin`, `git switch -c 0.<NN+1> --no-track origin/main`, `git push -u origin 0.<NN+1>`), rather than a stale local main or the pre-squash milestone branch: a branch that lacks `main`'s squash commit fails preflight's containment check at the next release. Without `--no-track` the branch tracks `main`, so a plain push or an editor sync sends milestone work to `main`, which only the release pull request may change. Preflight prints the block while the release pull request is unmerged, and a note in its place when the next branch already exists: inspect that branch's history and work before integrating main; never reset it automatically. The release pull request is the only one the procedure needs; preserve existing authorization.

### Tool sources

| Concern | Source |
| --- | --- |
| Version, notes and packaging | [release.py](../scripts/release.py), [tests](../scripts/test-release.py) |
| Crash-report symbolization and the release probe | [crash-report.py](../scripts/crash-report.py), [tests](../scripts/test-crash-report.py) |
| Read-only Git/GitHub preflight and command hand-off | [release-preflight.py](../scripts/release-preflight.py), [tests](../scripts/test-release-preflight.py) |
| Recipes | [justfile](../justfile) |
| Hosted builds and publication | [release.yml](../.github/workflows/release.yml) |
| Launcher CPU selection | [launcher](../tools/launcher/src/main.rs) |
