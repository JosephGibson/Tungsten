# Internal release procedure

Canonical procedure (`D-071`, `D-072`, `D-074`, `D-122`, `D-196`, `D-227`). **The agent writes the notes, runs `just release minor`, which cuts, checks and commits, and pushes the version branch; the owner pastes one block, whose tag push publishes.** Agents commit at each logical step and push the version branch after each commit ([operating rules](operating-rules.md#git), `D-215`, `D-216`); tags, every other push, merges, fetches and checkouts stay the owner's, and global `~/.claude/settings.json` denies them to agents (its push guard hook refuses every push but a version branch's), who supply the commands without bypassing those denies through another command or API. The same holds for `gh` commands that change GitHub (`gh pr create`, `gh workflow run`, reruns, deletions): agents run only the read-only ones.

**Every release task ends with the exact commands the owner still has to run**, values filled in, as one block: the tag, its push, the next branch and the pull request. The tag push publishes; the pull request is `main`'s merge and publishes nothing.

## Ordinary release

Start at the repo root on the version branch `0.NN`, named for the next minor, with [branch documentation prepared](#branch-preparation). A release is cut when something is worth shipping, not per change ([operating rules](operating-rules.md#releases)). `just release` takes `minor`, the next minor after the newest tag (0.x's breaking release), and `rehearsal`; any other kind refuses until the redo's 9.8 adds patch, major, release candidates and promote. Use Git, authenticated `gh`, network access and a GPU/display for smoke tests. Replace `X.Y.Z`, `0.NN` and the summary. If something is already cut, committed or tagged, [resume from that state](#resume-or-use-another-branch).

**Commit messages are one subject line, with no body**, in Conventional Commits, which the `commit-msg` hook checks once `just hooks` has run. The release commit is `chore(release): X.Y.Z short summary`, at most 72 characters, for example `chore(release): 0.109.0 example 01 quick fixes`; `just release` writes it and the pull request takes the same title.

**Rehearse first when `release.yml`, or `release.py`'s packaging, changed since the last final tag** ([rehearsals](#rehearsals-and-versioned-prereleases)): a final tag publishes on its push, with no merge left to stop a broken build.

1. **Agent: write the notes.** `[Unreleased]` from the commits since the newest tag (`git --no-pager log --oneline v0.<NN-1>.0..HEAD`), as [branch preparation](#branch-preparation) says, committed as `docs(changelog): 0.NN notes for …` and pushed.
2. **Agent: sit first** (`D-226`): `just perf sitting` against the previous tag, unattended, with the owner's NoMachine client disconnected (`--wait-quiet 10800` waits for quiet). A `regressed` owned metric needs a fix or a [known-issues](known-issues.md) entry naming the sitting's path before the cut.
3. **Agent: release.** Before writing anything, the command refuses a dirty tree, a branch not named `0.NN` and a subject over 72 characters. It runs the docs ratchet, the cut and the lock refresh, then the checks in order (`just check`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just smoke`, `just perf counters`, `just notices`, `just template-check`, `git diff --check`), commits `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock` and `DESIGN.md`, and runs the preflight, which prints the agent's push and the owner's block. A failed check leaves the cut uncommitted: fix it and run the same command again, which resumes at the checks. `--dry-run` prints the version and the chain and writes nothing.

   ```bash
   just release minor 'short summary'
   ```

4. **Agent: push the version branch**, the whole command, then end the task with the owner's block as preflight printed it ([hand-off](#handoff-and-next-branch)). The agent pushes no tag and runs no GitHub-changing `gh`.

   ```bash
   git push origin 0.NN
   ```

5. **Owner: paste the block** in the repo root. The tag push publishes: it starts [release.yml](../.github/workflows/release.yml), which checks the tag against the tagged tree, builds it and publishes the GitHub Release. The next branch starts from the version branch, so the next session waits on nothing.

   ```bash
   git tag -a vX.Y.Z -m 'Tungsten X.Y.Z' &&
   git push origin vX.Y.Z &&
   git switch -c 0.<NN+1> 0.NN &&
   git push -u origin 0.<NN+1> &&
   gh pr create --repo JosephGibson/Tungsten --base main --head 0.NN \
     --title 'chore(release): X.Y.Z short summary' \
     --body 'Release vX.Y.Z, published by its tag push. Merging it publishes nothing.'
   ```

6. **Owner: merge the pull request whenever convenient**, with a merge commit, never a squash, so the branch's commits and the tag stay on `main`'s history. `main` requires a pull request; the repository allows merge commits alone and titles them with the pull request's title (`merge_commit_title` `PR_TITLE`, `merge_commit_message` `BLANK`). The merge publishes nothing and gates no work. CI reports on the pull request (informational, `D-070`).
7. **Verify when asked.** Preflight reports the run and the release and prints the matching watch or view command. Finish with the [archive/asset checks](#archive-and-asset-verification); hosted builds do not test GPU/audio behavior.

   ```bash
   just release-preflight X.Y.Z --repo JosephGibson/Tungsten
   ```

The tag names the tested release commit. A tag is fixed once its GitHub Release exists: never move or force-push it. Through 0.109 squash merges left the tags off `main`'s history; from 0.110 they sit on it.

## Reference

### Branch preparation

[tungsten-release](../.claude/skills/tungsten-release/SKILL.md) handles this docs pass, the release, the command hand-off and release inspection; this guide owns the procedure. Preparation alone does not authorize publication or deletion; honor the requested scope and existing authorization without asking again.

- Establish the requested version; the owner fetches if needed. Inventory the commits since the newest tag and the active plans.
- Run `just release-check`. The workspace version must equal the newest versioned changelog heading and DESIGN's status version. It stays unchanged until the cut; version branch `0.NN` ships `0.NN.0`. Resolve drift without discarding unrelated work.
- Prepare `[Unreleased]` from the inventory: a `Summary:` line, applicable Added/Changed/Fixed/Removed groups, bold bullet leads and useful decision/path references. Do not create a release heading manually. If the requested version is already current with notes, inspect those notes, Cargo.lock and DESIGN and skip cutting again. After publication, new work goes in `[Unreleased]` for a later version. Resolve older or ambiguous version requests before editing released notes.
- During future preparation, use ``Workspace `vX.Y.Z`; release summary`` in DESIGN's status line; **omit `on branch …`**. A branch name goes stale and is not part of the version invariant. The cut updates the version token only; review the surrounding prose. README stays version-free; update its substance only when needed.
- Add any new decision's row to [DECISION_INDEX.md](DECISION_INDEX.md). Reversals add a decision, mark the old one `Superseded by D-NNN`, and update both rows. Update [LLM_INDEX.md](LLM_INDEX.md) for changed task areas/paths; keep it under 12 KiB (`D-106`).
- Plans close in their own sessions, deleted with their citations when done ([operating rules](operating-rules.md#plans)); a release moves none.

`just release minor` first runs `just docs-ratchet`, which refuses the cut when a crate's public items without rustdoc rose over [the baseline](../scripts/missing-docs.json) (`D-215`), then moves `[Unreleased]` into a section dated today, updates Cargo.toml and DESIGN, and refreshes Cargo.lock. It rejects inconsistent files, empty notes, non-increasing versions and dates before the last release. If the lock refresh fails, the cut files are already changed: fix Cargo.lock and run the command again, which resumes at the checks. The root README is never rewritten.

`just template-check` makes a game with `tungsten new` in a fresh folder outside the repository, after a 6 GiB free-space check on `target/`, points its tag pins at this checkout, then runs `tungsten check`, builds, tests and smoke-runs it there, packages it with `tungsten package` and smoke-runs the archive's binary, and seeds errors that `check` and `package` must report (`D-123`, `D-183`). The cut tree's pins read the new tag. The chain commits the tree it checked, and refuses to commit changes outside the cut. `main` may hold earlier releases' merges, which add nothing the branch lacks; preflight refuses any other commit there, which the owner merges into the branch (`git fetch origin`, `git merge origin/main`) before the chain runs again. CI remains informational (`D-070`); report its result for the tagged commit and outstanding hardware checks.

### Resume or use another branch

In any state, `just release-preflight X.Y.Z --repo OWNER/REPO` prints the commands that remain.

| State | Resume point |
| --- | --- |
| Notes written, nothing cut | Step 3. |
| Cut in place, not committed (a check or the lock refresh failed) | Fix it; `just release minor '…'` again resumes at the checks. |
| Cut committed | `just release minor '…'` again runs the preflight alone; step 4 if the branch isn't pushed. |
| Branch pushed, no tag | Step 5, the owner's block. |
| Tag pushed | Step 7. If no run started, see [recovery](#recover-according-to-state). |
| Release already exists | Inspect it at the tag's original commit (`--ref SHA`); do not cut or tag again. |
| Pull request open or merged | Information only: it publishes nothing. |
| Maintenance branch, no pull request | `--branch NAME --no-pr`: the owner's tag push publishes. |

Release runs start two ways (`D-227`):

| Trigger | Publishes |
| --- | --- |
| A `vX.Y.Z` or `vX.Y.Z-<pre>` tag is pushed | That release, prerelease or [rehearsal](#rehearsals-and-versioned-prereleases) |
| `gh workflow run release.yml --ref TAG` | That tag: recovery |

A pull request into `main` starts no release run, and its merge publishes nothing.

`just release-check [TAG]` checks files offline, including shallow CI; it does not inspect Git tag objects. `just release-preflight VERSION --repo OWNER/REPO [--branch B] [--base main] [--ref SHA] [--no-pr] [--rehearsal]` checks the committed files, a clean tree/index, unfinished Git operations, the live push destination and tracking ref, tag objects, that the base holds nothing the branch lacks but earlier releases' merges, GitHub repository state, the pull request and matching release/CI runs. `--branch` defaults to the checked-out branch. **Preflight stays read-only** and prints proposed commands only on success.

The tag push carries no branch: the agent pushes the version branch first, as a fast-forward. Existing tags can be verified at their original commit after the branch advances, including historical tags outside main. Fetch/inspect stale refs or divergent local branches; never reset automatically. Investigate conflicting tags without replacing them. Rerun preflight if the commit, branch, tag, pull request or release changes before acting. Pushing an unchanged tag again never starts a run.

### Archive and asset verification

Preflight reports the matching run ID, a tag run listed under the tag and the tagged commit. Watch that numeric ID rather than choosing the newest run interactively:

```bash
gh run watch RUN_ID --repo JosephGibson/Tungsten --exit-status &&
gh release view vX.Y.Z --repo JosephGibson/Tungsten &&
git ls-remote origin 'refs/tags/vX.Y.Z*'
```

For an annotated tag, the `^{}` line identifies its commit; the other line identifies the tag object. GitHub release `target_commitish` is not proof of the tagged commit. Confirm the remote tag, run SHA and intended SHA agree.

The workflow builds the examples and the template (`tungsten-template-basic`, `D-186`) for Linux (`x86_64-unknown-linux-gnu`, Ubuntu 24.04) and Windows (`x86_64-pc-windows-msvc`, static CRT), and publishes one zip per platform for players (`D-196`): `tungsten-examples-X.Y.Z-linux-x64.zip` and `tungsten-examples-X.Y.Z-windows-x64.zip`. Each one's top folder holds a launcher per build under its player-facing name from `TITLES` in `release.py` (`Platformer`, `Benchmark`, `Template`), README.txt, LICENSE.txt and the [license notices](#license-notices) `THIRD-PARTY-NOTICES.txt`; its `data/` folder holds the portable x86-64 and x86-64-v3 builds under `data/bin/<level>/`, the shared runtime files the examples read, and each folder game's own `tungsten.json`, `input.json` and `assets/` under its folder: the template's under `data/templates/basic/` and the platformer's under `data/examples/01_platformer/` (a folder game is a template or a shipped example whose folder holds its own `tungsten.json`). No debug file ships: packaging strips each Linux binary and the PDBs stay on the runner. Launchers select the fastest supported CPU level and set the working directory to `data/`, or for a folder game to its folder under it (`FOLDERS` in `tools/launcher/src/main.rs`). Hosted runners have no GPU, so nothing with a window runs there; each build job's `Crash-report probe` step runs `tools/crash-probe` from an archive-like copy and symbolizes its crash file, informational only (`continue-on-error`, `D-120`).

Expect the two zips and `SHA256SUMS`, which lists both; GitHub adds its own `Source code` archives, which `SHA256SUMS` does not list. Download them into a fresh directory and verify them (requires `sha256sum`; use an equivalent SHA-256 tool on Windows):

```bash
release_download=$(mktemp -d) &&
gh release download vX.Y.Z --repo JosephGibson/Tungsten --dir "$release_download" \
  --pattern 'tungsten-examples-*' --pattern SHA256SUMS &&
(cd "$release_download" && sha256sum -c SHA256SUMS)
```

Check that both zip names are present, notes match the tag's changelog section, and prerelease classification matches the tag. Read each zip's `THIRD-PARTY-NOTICES.txt`: its first line names the target, its crate count is the one `just notices` printed for that target, the Linux file lists Cantarell under `sctk-adwaita`, and the last section is the Rust standard library's notices. Extract the appropriate zip and launch `Platformer` and `Template` on a GPU machine, from the file manager and from a terminal outside the archive folder; on Linux check that the launchers kept their executable bit. On Windows the template's executables under `data/bin/<level>/` show the gem icon in the file manager, linked from `templates/basic/windows/icon.res` by its `build.rs` (`D-199`); the launchers carry none yet. On Linux the first console line reports the selected CPU level; Windows builds open no console, and each run's log and any crash report go to `%LOCALAPPDATA%\tungsten-examples\logs` (Linux: `~/.local/state/tungsten-examples/logs`, `D-119`), the template's to `%LOCALAPPDATA%\tungsten-template-basic\logs` (Linux: `~/.local/state/tungsten-template-basic/logs`) and the platformer's to `%LOCALAPPDATA%\tungsten-platformer\logs` (Linux: `~/.local/state/tungsten-platformer/logs`). `TUNGSTEN_CPU_LEVEL=x86-64` exercises the portable path; never force v3 on an unsupported CPU. Report untested platforms explicitly. Linux needs compatible Vulkan/windowing/audio libraries and may need glibc 2.39.

### License notices

Each zip's `THIRD-PARTY-NOTICES.txt` (`D-122`) comes from license data that each build job collects after the build. `release.py licenses <target>` takes the crates that `cargo tree -e normal` reaches from the examples, the template and the launcher, reads each crate's license expression and its root-level `LICENSE*`, `LICENCE*`, `COPYING*`, `COPYRIGHT*`, `UNLICENSE*` and `NOTICE*` files, lists its font files, and copies the toolchain's `COPYRIGHT-library.html`. The publish job's `package` reads that `licenses.json` from the artifact and writes the file, whose last section is the standard library's notices converted to plain text (`D-196`); `just notices` runs the same two commands for both targets into `target/notices/<target>/`.

A crate is used under the first alternative of its license expression, in the order MIT, Apache-2.0, Zlib, BSD-3-Clause, BSD-2-Clause, ISC, Unlicense, 0BSD, CC0-1.0, Unicode-3.0, MPL-2.0, Apache-2.0 WITH LLVM-exception, whose licenses each match one of its files. The other alternatives' texts stay out, `NOTICE` files always ship, and the file lists each distinct text once, numbered, under the crates that use it. When `just notices` fails, the error names every crate concerned:

- **No license text matches an alternative** (a new crate, or a version without license files): fetch the upstream license file at the crate's version tag, or at the commit in the package's `.cargo_vcs_info.json`, into `licenses/<crate>/`, and add a `texts` entry to [`licenses/clarifications.json`](../licenses/clarifications.json) with the crate's `version` and the file's `source` URL. Never write a license text from memory.
- **Checked at another version:** a clarified crate moved to another version. Check upstream again, refresh its files and update `version`.
- **Font files with neither a bundled entry nor a review:** read whether the crate's sources compile a font in (`include_bytes!`). If one does, add a `bundled` entry with the font's license text, its copyright as the font's name table or upstream gives it, the source and the version, as `sctk-adwaita`'s Cantarell has. If none does, add a `fonts_reviewed` note saying what the fonts serve.
- **An embedded work's file is missing:** `EMBEDDED_WORKS` in `scripts/release.py` lists the third-party works the engine compiles in, each with its license file (SMAA's lookup textures and shaders). Add a row when engine code embeds another one.

The font guard covers font files only. After a dependency change, check the new or updated crates' sources for other data they compile in under another license than their own (`include_bytes!` or `include_str!` outside tests and doc comments), and add each as a `bundled` entry.

### Rehearsals and versioned prereleases

A prerelease tag with its own changelog section is a versioned release and must match the workspace version. A prerelease tag without that section is a rehearsal: the workspace still passes consistency checks, notes come from `[Unreleased]`, and the tag version need not match. Both publish as GitHub prereleases as soon as the tag is pushed, without a pull request. A rehearsal needs no version cut.

`just release rehearsal` picks a unique tag on HEAD, `v0.0.0-test.YYYYMMDD.gSHA` with a suffix when one is taken, checks it and prints the owner's block: the tag and its push. After the push, record the tag's SHA and run ID and watch the run. Do not reuse a fixed test tag for a different commit. Cleanup is a separate requested action targeting only that rehearsal release/tag; inspect whether it is immutable before attempting deletion.

### Recover according to state

Always inspect the identified run and GitHub release first. Network/authentication errors are not evidence that a release is absent. Reruns use the original commit and ref; editing a workflow on a later commit cannot repair that run. [GitHub rerun behavior](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)

| Observed state | Next action |
| --- | --- |
| Tag pushed; no matching run | The owner starts one with `gh workflow run release.yml --repo JosephGibson/Tungsten --ref vX.Y.Z` (preflight prints it), then watches it (`gh run watch RUN_ID --exit-status`). Pushing the unchanged tag again is not a retry mechanism, and the tag never moves. |
| Run failed at the tag check, or a fix follows the tag push; no release exists | With no GitHub Release for the tag, the owner re-points it to the fixed, checked commit (`git tag -d vX.Y.Z`, `git push origin :refs/tags/vX.Y.Z`, `git tag -a vX.Y.Z SHA -m 'Tungsten X.Y.Z'`, `git push origin vX.Y.Z`); the new push starts the run. |
| Build failed; no release exists | Inspect `gh run view RUN_ID --log-failed`. For a transient failure, rerun that run's failed jobs with `gh run rerun RUN_ID --failed`. A source/workflow fix needs a new commit and a new version/tag, or a unique rehearsal tag. |
| Publish failed; no release exists | Inspect logs, then rerun the failed publish job if its build artifacts still exist. Artifacts have seven-day retention; expired artifacts require rebuilding via a full rerun if GitHub still allows it. |
| Draft release or partial upload exists | Verify the tag/commit and inspect assets. The workflow uses `gh release create`, so it cannot automatically resume an existing draft. Resume the draft with verified assets from the same run and publish it once complete, or delete only the draft (keep the tag) if that cleanup is authorized, then rerun. Do not blindly clobber uploaded assets. |
| Release is already published | Verify the tag, notes, expected assets and checksums. A lost response or failed rerun may follow successful publication. Stop if complete. An incomplete published release needs an explicit repair decision after checking immutability; do not delete/recreate it by default. |
| Local and remote tags disagree | Stop publication and identify which object was pushed. Never move a tag whose release exists. |

Current `gh release create` with assets creates a draft, uploads assets, then publishes. Drafts are mutable; published immutable releases lock their assets and tag. Account for that boundary during recovery. See [GitHub CLI release creation](https://cli.github.com/manual/gh_release_create). Rerun once for an identified transient problem, inspect the result, and stop repeating the same failing action without new evidence.

### Handoff and next branch

End the task with the agent's push done and the owner's block, after a short report: version/tag, the checks run and their results, anything not checked (platforms, GPU) and what the commands will do. A milestone's run session then gives the prompt for the next candidate's plan ([workflow](plans/1.0/workflow.md) §3), labelled as [tungsten-next's reply](../.claude/skills/tungsten-next/SKILL.md) labels it: `**New session** (repo root) · <mode> mode · <model> · <effort> · <reason>`, then the prompt, then its NoMachine line. Take the model and effort from the [operating rules' tiers](operating-rules.md#tiers) (`D-151`), never from memory. After the tag push, report the tagged commit SHA, run ID/result, release URL and archive/checksum verification. For preparation-only work, report requested/current versions, whether a cut occurred, checks and the remaining steps.

The owner's block starts the next branch from the version branch (`git switch -c 0.<NN+1> 0.NN`, `git push -u origin 0.<NN+1>`), before the merge, so the next session starts at once; `main` then gains only merges of branches it already holds, which preflight admits at the next release. A branch made from a local branch tracks nothing until `-u` gives it its own upstream, so a plain push never reaches `main`, which only the release pull request changes. Preflight prints a note in place of those commands when the next branch already exists: inspect its history before integrating; never reset it automatically. The release pull request is the only one the procedure needs; preserve existing authorization.

### Tool sources

| Concern | Source |
| --- | --- |
| Version, notes, the cut and `just release`, packaging and license notices | [release.py](../scripts/release.py), [tests](../scripts/test-release.py), [clarifications](../licenses/clarifications.json) |
| Crash-report symbolization and the release probe | [crash-report.py](../scripts/crash-report.py), [tests](../scripts/test-crash-report.py) |
| Read-only Git/GitHub preflight and command hand-off | [release-preflight.py](../scripts/release-preflight.py), [tests](../scripts/test-release-preflight.py) |
| The `missing_docs` ratchet the cut runs first | [missing-docs.py](../scripts/missing-docs.py), [baseline](../scripts/missing-docs.json) |
| Recipes | [justfile](../justfile) |
| Hosted builds and publication | [release.yml](../.github/workflows/release.yml) |
| Launcher CPU selection | [launcher](../tools/launcher/src/main.rs) |
