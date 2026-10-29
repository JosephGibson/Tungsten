# H2: releases, status, skills and memory (health-h2)

- **status:** in progress
- **goal:** Redo step 7.4, H2 (the redo's §4): retake H1's overhead against the M76–M85 baseline; decide the five carried workflow proposals; `just status`, derived from the tree; one `just release <kind>` with its rehearsal, a final tag publishing on its push (R-Q15); the planning, next, release and decision skills rewritten lean against a no-skill baseline on tasks 7.1–7.3 produced, with a phase plan's step prompt (phase 7's F2); the plan, perf and release memory hubs pruned to what still applies.
- **non-goals:**
  - Retiring the per-version branches (H1's Git row, "then the per-version branches retire"): agents would push `main`, which main's pull-request rule and the owner's push guard (`D-216`) refuse, and both are the owner's settings (Q1). A Later line.
  - R3's other release kinds (patch and minor refusing breaks, the RC series, `promote`): 9.8 switches them on and rehearses them (the redo's §4).
  - `tungsten-perf` (rewritten at 7.3) and `tungsten-wgpu` (a domain skill the redo doesn't name).
  - A release: H2 cuts none. The next release session runs the new flow, after the owner's rehearsal (§2.4).
  - Rust code: no crate, example, template or tool source changes.
  - History: `CHANGELOG.md`, the frozen records and existing `DECISIONS.md` bodies stay as written; frozen records' links to removed or renamed skills rot by design (Q5).
  - Agent memory outside the three hubs (feedback notes, art direction, run traps the hubs don't link).
  - The template's `AGENTS.md` line of 7.2's follow-up (waits for 7.16).
- **files to touch:**
  - Step 1: none in the tree but this plan's evidence log; the retake in `perf-runs/<UTC>-overhead-h1/` (untracked).
  - Step 2: `deny.toml`, `.claude/settings.json`, `.editorconfig` (new), `.gitattributes` (new), `docs/agent-setup.md`, `docs/operating-rules.md` (Plans), `docs/known-issues.md` (`:108`, `:110`, `:113`), `ROADMAP.md` (Later).
  - Step 3: `scripts/status.py` (new), `scripts/test-status.py` (new), `justfile`, `.claude/skills/tungsten-next/state.sh` (deleted), `.claude/skills/tungsten-next/SKILL.md` (§1's command only), `.claude/settings.json`, `docs/operating-rules.md` (Status), `docs/LLM_INDEX.md` (row 91).
  - Step 4: `.github/workflows/release.yml`, `scripts/release.py`, `scripts/test-release.py`, `scripts/release-preflight.py`, `scripts/test-release-preflight.py`, `justfile`, `docs/releases.md`, `docs/operating-rules.md` (intro, Git, Releases, Docs), `docs/validation.md` (`:3`, `:31`, `:54`), `docs/agent-setup.md` (`:33`, `:58`), `docs/known-issues.md` (`:107`), `docs/LLM_INDEX.md` (row 92), `DECISIONS.md`, `docs/DECISION_INDEX.md`, `.claude/skills/{tungsten-release,tungsten-finalize,tungsten-next}/SKILL.md` (their hand-off, publishing and merge lines only, so a skipped rewrite at step 6 or 7 still leaves them on the new flow).
  - Step 5: `.claude/skills/tungsten-decision/SKILL.md`; evaluations in the run session's scratchpad and `perf-runs/<UTC>-h2-skills/` (untracked).
  - Step 6: `scripts/check-repo.py`, `scripts/test-check-repo.py`; `.claude/skills/tungsten-release/SKILL.md`, `.claude/skills/tungsten-finalize/` (deleted), `.agents/skills/tungsten-finalize` (deleted), `.gitignore`, `docs/releases.md` (`:63`), `docs/agent-setup.md` (`:18`), `docs/LLM_INDEX.md` (row 92).
  - Step 7: `.claude/skills/tungsten-next/SKILL.md`, `.claude/skills/tungsten-next/prompts.md`, `docs/agent-setup.md` (`:18`), `docs/releases.md` (`:167`, only if the reply's label shape changes).
  - Step 8: `.claude/skills/tungsten-milestone/` renamed to `.claude/skills/tungsten-plan/` (`SKILL.md`, `skeleton.md`), `.agents/skills/tungsten-milestone` replaced by `.agents/skills/tungsten-plan`, `.gitignore`, `.claude/skills/tungsten-next/{SKILL.md,prompts.md}` (the name), `docs/plans/README.md` (Executing, Lifecycle), `docs/plans/phase-7.md` (`:17`'s link), this plan (its header link), `docs/LLM_INDEX.md` (row 90), `docs/agent-setup.md` (`:18`), `scripts/check-repo.py` (`:46`'s comment), `scripts/test-check-repo.py` (`:296`'s fixture path).
  - Step 9: `docs/known-issues.md` (`:86`'s memory clause); the memory folder, outside the tree, with its backup in `perf-runs/<UTC>-h2-memory/` (untracked).
  - Step 10: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md` (Step 3's evidence row, F2, F3), the follow-ups' homes (`ROADMAP.md` Later, `docs/known-issues.md`).
- **ordered steps:** (1) Retake H1's overhead. (2) The carried proposals decided. (3) `just status`. (4) Releases on a tag push, `just release`, the decision. (5) The evaluations' setup and `tungsten-decision`. (6) One release wrapper. (7) `tungsten-next` and the step prompt. (8) `tungsten-plan` in place of `tungsten-milestone`. (9) The memory hubs pruned. (10) Close.
- **done-when:** every step's done-when passes, quoted in the evidence log; the retake recorded beside the baseline, lower or its miss carried to 7.5's triage (§2.1); a final tag publishes on its push and no pull request does (`release.yml`), and `just release rehearsal` prints the owner's block; `just status` prints the derived state; the four skills pass their gates (§2.5) and weigh less in all than at this plan's commit; the three hubs pruned (§2.7); `just check`, `just script-test`, `just ctx`, `just repo-check` and `just deps` pass; the demo (step 10) works; the plan deleted with its citations, phase 7's evidence row written, and `health-h2` out of Now.

Item `health-h2` in [ROADMAP.md](../../ROADMAP.md), from the [roadmap redo](roadmap-redo.md)'s 7.4 row and §4, and step 3 of the [phase 7 plan](phase-7.md) with its F2. Written 2026-10-29 at `4066c6ad`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); it differs in two ways: steps 5–8 evaluate each skill with subagents before and after its rewrite (§2.5), and step 8 renames the skill this plan runs under (a running session keeps the text it loaded).

## Context digest

- **Why now.** H1 (2026-10-25, `D-215`) froze the records and wrote the operating rules; H2 is the half that needed real tasks (the redo's §4). Since then 7.1–7.3 produced three plans, nine run sessions, two decisions and a release, and the six example stops ran under H1.
- **What it stands on.** The operating rules; `D-079` (a release publishes when its pull request merges), whose merge-publishes clause the redo's §9 hands to H2 (R-Q15); `D-216` (agents push the version branch alone, held by the owner's push guard); `D-226` (a sitting before each minor release); `D-140` and `D-150` (routes, no approval stops); the frozen-record rule (operating rules, Records).
- **The redo's H2 rows:** releases (`just release <kind>`, its rehearsal, a final tag publishing on push, the 0.x kind first), status (`just status`, R5), skills (each against a no-skill baseline, R4), the five proposals decided, the memory hubs pruned, the overhead remeasured and lower.
- **Phase 7 asks** for a phase plan's hand-off: a step's plan prompt in place of the `run` prompt (F2).
- **Decisions cited:** `D-215`, `D-216`, `D-079`, `D-071`, `D-074`, `D-226`, `D-140`, `D-150`, `D-005` (no external ECS), `D-070` (CI informational).
- **Later steps expect:** 7.5 onward planned and run on the lean skills; 7.13's evaluations reuse §2.5's method (RC-A9); 9.8 adds R3's kinds to `just release`.

## 1. Audit

Read at `4066c6ad`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | A final tag publishes only when the pull request whose head it names merges: the push trigger takes prerelease tags alone, and a `pull_request: closed` run checks the tag, the head and the merge tree | `.github/workflows/release.yml:19-26`, `:57-94` |
| A2 | Preflight's hand-off is `git add -A`, a commit, the tag, `git push origin <branch> <tag>` and `gh pr create`, then a post-merge block that fetches and starts the next branch from `origin/main`; it requires `main` inside the branch, and matches a final release's run to the merged pull request | `scripts/release-preflight.py:147`, `:176-188`, `:192-197`, `:211-244` |
| A3 | The guide's happy path: the agent cuts and runs ten checks, the owner commits with `git add -A`, tags, pushes, opens and merges the pull request and pastes the post-merge block; 25,773 bytes | `docs/releases.md:7-57`, `:165-169` |
| A4 | `just release-cut` runs the docs ratchet, `release.py cut` and the lock refresh; `release-check` and `release-preflight` wrap the rest; `cut` refuses an inconsistent tree, a non-increasing version and an empty `[Unreleased]` | `justfile:127-137`; `scripts/release.py:237` (`check_tree`), `:305` (`cut`) |
| A5 | Live files naming `release-cut` beside the justfile | `docs/operating-rules.md:58`, `:71`; `docs/known-issues.md:107`; `docs/validation.md:54`; `docs/releases.md:18`, `:72`; `scripts/release.py:15`, `:277`, `:1141` |
| A6 | `main` requires a pull request (no approvals, admins not enforced) and the repository merges by merge commit alone | `gh api repos/JosephGibson/Tungsten/branches/main/protection` (`required_pull_request_reviews`, `enforce_admins` false) and `gh api repos/JosephGibson/Tungsten` (`allow_merge_commit` alone), read 2026-10-29 |
| A7 | The owner's push guard lets an agent run `git push (-u )?origin 0.NNN` alone | `~/.claude/hooks/git-push-guard.sh:22`, outside the tree (`D-216`) |
| A8 | The merge is the publish step "until H2" in `D-079`, `D-215`'s Git bullet and the operating rules | `DECISIONS.md:664-669`, `:3245`; `docs/operating-rules.md:3`, `:28`, `:58` |
| A9 | `tungsten-next`'s `state.sh` prints git, the tree, other sessions, frozen-record edits and ROADMAP's Now and Next; it counts from a squash commit for releases through 0.108; nothing reports a plan's progress | `.claude/skills/tungsten-next/state.sh:23-31`, `:70-89`; `SKILL.md:13` |
| A10 | The skills H2 rewrites, in bytes: `tungsten-milestone` 6,437 + `skeleton.md` 3,725; `tungsten-next` 6,273 + `prompts.md` 2,486; `tungsten-release` 1,144 + `tungsten-finalize` 788, both sending the agent to the 25,773-byte guide; `tungsten-decision` 2,386 | `wc -c .claude/skills/*/*` |
| A11 | The `run` prompt says "run every remaining step", which a phase plan forbids | `.claude/skills/tungsten-next/prompts.md:10`; `docs/plans/phase-7.md`'s F2 |
| A12 | The frozen workflow links five skill files and `prompts.md`; `check_docs` checks every link in the 1.0 folder, frozen or not, so removing or renaming a skill fails `just repo-check` | `docs/plans/1.0/workflow.md:17`, `:37`, `:53`, `:63`, `:65`, `:117`; `scripts/check-repo.py:211-220`, `:278-296`, `:298-312` |
| A13 | Every `.claude/skills/<name>` needs an `.agents/skills/<name>` symlink and a `.gitignore` allow line; a description holds 300 characters, a body 8,192 bytes | `scripts/check-agent-context.py:44-47`, `:105-138`; `.gitignore:56-62` |
| A14 | The LLM index holds 12,273 of its 12,288 bytes; rows 90–92 name the skills and release files | `docs/LLM_INDEX.md:90-92`; `scripts/check-agent-context.py:25` |
| A15 | The plans README still moves finished plans to the archive, where the operating rules delete them | `docs/plans/README.md:43`, and its Executing list's status line |
| A16 | The five carried proposals, and the consumer check's note | `docs/known-issues.md:110`, `:108` |
| A17 | P4: `[bans]` has no deny list; none of AGENTS.md's ten banned crates is in the graph; the decision-row check exists | `deny.toml:47-50`; `cargo tree --workspace --locked --offline -e all -i <crate>` → no match, all ten; `crates/tungsten-core/tests/integration/decision_index.rs:32` |
| A18 | P6: the allow list keeps two merged packages' launches and lacks five release and asset recipes; agent-setup describes the old skills, a human push, four launch commands and repo-check's removed roadmap checks, and was last verified on Claude Code 2.1.110 (2.1.311 runs here) | `.claude/settings.json:34-35`; `docs/agent-setup.md:18`, `:31`, `:33`, `:46`, `:54`; `docs/known-issues.md:113` |
| A19 | P9: no `.editorconfig` or `.gitattributes`; the index holds 2,127 LF text files and no CRLF; Windows CI turns `autocrlf` off | `git ls-files --eol`; `.github/workflows/ci.yml:116` |
| A20 | The overhead baseline is machine-local, with its scripts: sessions hard-coded for M76–M85, a git side reading `Update 0.NN:` squash commits; H1's retake, due after the first stops under H1, was never recorded | `perf-runs/20261025-overhead-baseline/classify.py` (`SESSIONS`, `RECORD`), `git_baseline.py` (`MAP`); the redo's §4, H1's Measured row |
| A21 | Agent memory: 168 files; the plan hub links 61 notes, the perf hub 42, the release hub 13; `MEMORY.md` 12,871 bytes; 7.2's follow-up names the in-game shot recipes for H2 | `~/.claude/projects/<this project>/memory/`; `docs/known-issues.md:86` |

## 2. Design

### 2.1 The overhead retake (step 1)

The baseline's scripts, copied to `perf-runs/<UTC>-overhead-h1/` with four changes: `SESSIONS` takes the table below; `RECORD` adds `ROADMAP.md` and `docs/operating-rules.md`, the successors of `roadmap.json` and `workflow.md`, which it counts; `own_plan_re` takes each item's plan (`docs/plans/<topic>.md`); and each run prints which pattern made each bookkeeping call, as top-five counts. The git side counts, per release 0.109–0.112, the record files (`is_record` plus `ROADMAP.md`) touched by its own non-merge commits, `git log --no-merges --name-only <from>..<tag>`. `<from>` is the previous release's point on this history: 0.108's squash `d31ee9bb` for 0.109 and 0.109's squash `433b4998` for 0.110, since both were squash-merged and their tags sit off it (`merge-base` would reach back to 0.107 and count a squash as the next release's), then `v0.110.0` and `v0.111.0`, which are ancestors of the next tags. Each H1-era plan's lines are counted at its last commit before deletion. Main transcripts give the headline, as the baseline's did; `--subagents` is printed beside.

| Group | Sessions (transcript ID prefixes) |
| --- | --- |
| Example (headline) | `e01quick2` e37e49b7; `e01tomes` 524ac6e5, fc80dc82, cab02c03; 0.109 1cd7756c; `e01foes` 6d4e4b95, 10ab362e, 19eafce2, 514b3f21; `e01sfx` 9c187913, ddcf39f1, 871e8f03, 1b66d873; `e01hollow` 2b8c2c41, 25baf081, c1c63f7e, 79fb68eb, 5960292f; 0.110 5bce7a4a; `e01flask` de069216, 5135cf2f, d635f704; 0.111 0167c9fc |
| Phase 7 (apart) | `qa6` b50c24a0; 7.1 0c6f4c08, 43718f9d, c625bd77, c464fda9, baf65e55, 81257724; 0.112 66a95df2; 7.7 af0d23bc; 7.2 a805011d, 67d79c4a; 7.3 af6410ef, 25f01f81 |

Left out, as the baseline left them: intake, prompt and quick-fix sessions, and starts under 20 tool calls. The baseline's milestones released inside their run sessions, so the example group carries its release sessions.

**Gate:** the example group below the baseline: bookkeeping under 22.1 % of tool calls and 20.0 % of output tokens, and a median under 12 record files a release. Plan lines are recorded beside the baseline's 312, not gated: H1 kept a plan file for large items.

### 2.2 The carried proposals (step 2)

| # | Proposal | Disposition |
| --- | --- | --- |
| P4a | Deny-list bans | Adopted: `deny.toml`'s `[bans] deny` lists AGENTS.md's ten crates (`bevy_ecs`, `hecs`, `specs`, `legion`, `amethyst`, `fyrox`, `ggez`, `macroquad`, `tokio`, `async-std`), each with a `reason`, so `just deps` and `just ci` refuse them |
| P4b | A decision-row check | Landed already: `decision_index_mentions_every_decision_heading` and `check_docs`' unknown-ID rule (A17) |
| P5a | A released-tag consumer check (01 built outside the tree on a tag) | To 9.8, whose candidates run the getting-started guide's commands from a tag (known issues already hold that gap); 01's path dependencies make an outside build a port, not a check. Known issues `:108` says so |
| P5b | An early RC-A9 pilot | Adopted as §2.5: fresh agents on real tasks, counting tool calls, tokens and misses, the method the redo's §8 runs at 7.13 |
| P6 | Refreshed client discovery, exact-command permissions | Adopted: allow `just api`, `just api-check`, `just notices`, `just template-check`, `just level-check`; drop the two merged packages' launches; agent-setup corrected (its skills line again at steps 6–8); its two client checks rerun from the root and from `crates/tungsten-render/` on the installed Claude Code and Codex, with the versions and what loaded |
| P7 | Behaviour and visual evidence for feature reviews | Adopted as one rule in the operating rules' Plans: a game feature's demo is a `tungsten run --script` report, with a `tungsten render` sheet when it changes what is drawn, since a demo told from memory can't be rechecked |
| P9 | `.editorconfig` and `.gitattributes` | Adopted, minimal: `* text=auto eol=lf` with `binary` for the asset types (the index is all LF, so nothing renormalizes); `.editorconfig` with UTF-8, LF, a final newline and 4-space Rust and Python, and nothing for JSON or Markdown, which generators write |

No decision entry: P4a enforces `D-005` and AGENTS.md's rule, P9's files state their rule where tools apply it, and P7 is an operating rule.

### 2.3 `just status` (step 3)

`scripts/status.py`, standard library only, read-only and offline (origin refs as of the last fetch). `just status` runs it, and `tungsten-next` reads it in place of `state.sh`, which goes. Sections, in order:

- **git:** branch and upstream; Cargo.toml's version; the newest final `v*` tag on HEAD's history, and whether a cut waits (the version is past it); whether the tag is on `origin/main`, with the last fetch's age; the commits since it, 30 shown.
- **tree:** `git status --short`; files written in the last 3 minutes.
- **sessions:** other `claude` and `codex` processes working in this tree; a running `scripts/perf.py`.
- **frozen records:** files carrying the `D-215` banner that changed since `D-215`'s commit.
- **ROADMAP.md:** Now and Next, and Now's count.
- **plans** (new, derived): each active plan outside the frozen 1.0 folder, with its status, its ordered steps' count, its evidence rows' count and the last row's step and verdict.

`state.sh`'s squash-era counting goes: tags are on HEAD's history since 0.110. `/proc` and file ages print `unavailable` off Linux. Tests in `scripts/test-status.py`, on temporary repositories as `test-release-preflight.py` builds them: commits since the tag, a waiting cut, Now's count, a plan's rows, a frozen-record edit after the `D-215` commit, a dirty file, a missing `/proc`.

### 2.4 Releases on a tag push (step 4)

**The flow.**
1. The agent writes `[Unreleased]` from the commits since the newest tag, committed as `docs(changelog): …`.
2. A minor release sits first (`D-226`), unattended.
3. `just release minor '<summary>'` refuses a dirty tree, a branch not named for the next minor, and a subject over 72 characters, before writing anything. It runs the docs ratchet, the cut and the lock refresh, then the checks in order: `just check`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just smoke`, `just perf counters`, `just notices`, `just template-check`, `git diff --check`. It commits `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock` and `DESIGN.md` as `chore(release): X.Y.Z <summary>`, runs the preflight and prints the agent's push and the owner's block. A failed check leaves the cut uncommitted; a rerun finds it and resumes at the checks. `--dry-run` prints the version and the chain and writes nothing.
4. The agent pushes the branch (`git push origin 0.NN`).
5. The owner pastes one block: `git tag -a vX.Y.Z -m 'Tungsten X.Y.Z'`, `git push origin vX.Y.Z` (this publishes), `git switch -c 0.<NN+1> 0.<NN>`, `git push -u origin 0.<NN+1>`, `gh pr create` into `main`.
6. The owner merges the pull request with a merge commit whenever convenient; it publishes nothing.

**Kinds.** `minor`: the next minor after the newest release, today's breaking 0.x release (the redo's §4). `rehearsal`: no cut and no commit; a unique `v0.0.0-test.<YYYYMMDD>.g<sha>` on HEAD, `just release-check` and the preflight's rehearsal block. Any other kind refuses, naming 9.8, which switches on R3's.

**`release.yml`.** `on.push.tags` takes `v[0-9]+.[0-9]+.[0-9]+` beside the prerelease pattern. The `pull_request` trigger, its fork guard and the merge branch of the select step go. `workflow_dispatch` on a tag stays, for recovery. A tag run checks the tag against the tagged tree as it does today (`release.py check "$tag"`); the build and publish jobs don't change.

**Preflight.** A final tag publishes on its push. The pull request is printed for a final release on a version branch as `main`'s merge, and its state is information only. The merged-tree and "merging publishes nothing" refusals go. Tag-push runs match final tags as they match prerelease ones. Containment becomes: every commit the base holds that the release commit lacks (`git rev-list <sha>..<base>`, the base's tip fetched as today) is a merge of a branch the release contains that adds nothing of its own: its second parent is an ancestor of `<sha>`, and its tree equals that parent's. The next branch starts before the earlier release's merge, which is then all `main` holds that it lacks; a merge with its own resolution, or any other commit, is refused as content the release would miss. The rule holds release over release, since each release merge's tree equals its branch's. The post-merge block goes; the next branch joins the hand-off after the tag push.

**Why from the version branch.** The next branch starts from the version branch's tip, the tagged commit unless commits followed it, so the merge stops gating anything: the owner pastes one block and the next session starts at once. A later branch still merges cleanly, since `main` gains only merges of branches it already holds.

**The transition.** A `pull_request` run reads `release.yml` from the pull request's merge ref, which carries this step's file without the trigger, so a release pull request merged after this step starts no release run. The old path stays reachable only through a pull request from a branch without this step, and none is open. Were it to run on a matching head, it would skip its "already released" exit (`.github/workflows/release.yml:75`), build, and then fail at `gh release create` (`:244`) on the existing release, leaving the published one untouched. The first release after H2 checks that its merge started no release run (a known-issues line from step 10).

**The rehearsal rule.** A change to `release.yml`, or to `release.py`'s packaging, is rehearsed (`just release rehearsal`, then the owner's tag push) before the next final tag. H2's close hands the owner that block. A final tag pushed with no run starting is recovered by `gh workflow run release.yml --ref <tag>`, the owner's.

**Recipes.** `release-cut` goes; `release-check`, `release-preflight` and `docs-ratchet` stay.

**Docs.** `docs/releases.md`: the happy path above, under "Ordinary release" (validation's and agent-setup's links follow the anchor); the headings the frozen records link (`#handoff-and-next-branch`, `#rehearsals-and-versioned-prereleases`), `#license-notices` and `#branch-preparation` (which `tungsten-finalize` links until step 6) stay; the triggers, resume and recovery tables rewritten for the tag push. The operating rules' intro, Git and Releases lines, and Docs' `release-cut`.

### 2.5 Skill evaluations (steps 5–8)

Each skill is rewritten against a no-skill baseline on tasks 7.1–7.3 produced (§2.6: two or three a skill), under three arms. Every scratch file lives in the run session's scratchpad (`<scratch>` below), outside any repository: under `target/`, git would find the live repository, whose history and `DECISIONS.md` hold the edit tasks' answers.

- **N, no skill:** told not to call the Skill tool or open any skill folder.
- **O, the old skill:** told to read and follow the skill's files as they stood at this plan's commit: `git archive -o <scratch>/old.tar <commit> .claude/skills`, then `tar -xf` into `<scratch>/old/`, where `<commit>` is `git log --diff-filter=A --format=%h -1 -- docs/plans/health-h2.md`. Archives go to a file, never through a pipe, since the rtk hook rewrites piped git output.
- **L, the lean skill:** told to read and follow the rewritten skill's files, copied to `<scratch>/new/` before its runs.

O and L read the docs their skill points to in the task's tree (a copy's for an edit task, the live tree's otherwise), so they follow one procedure and differ only in the skill. The old guide's cost is its bytes (A3), not a run.

**Runs.** One fresh `general-purpose` subagent per arm and task, with the same prompt but the arm's line, a budget of 40 tool calls, and its answer written to `<scratch>/h2-skills/<task>/out-<arm>.md`; it returns one line. An edit task works in its own copy of the tree at the task's commit (`git archive -o`, then `tar -xf`, 67 MB), treated as the repository root. The copy has no `.git`, so git finds no history, and no `.claude/skills/` or `.agents/skills/`, which the run session removes so each arm brings its own skill or none. A run reads and writes nothing outside its copy, its skill copy and its output file. A judgment task reads the live tree, writing nothing; a fixture task answers from its fixture and runs no git or `just` command. A script then scans each run's subagent transcript (`<session>/subagents/*.jsonl`) and voids the run on any of these:
- in N, a Skill call or a path into a skill folder;
- in O or L, a read of the live `.claude/skills/`;
- in an edit task, any tool input naming the live repository's path;
- in a fixture task, a git or `just` command;
- in R1, a write.

A voided run reruns once with the rule restated; a second void scores a miss. Each task's copies are deleted once it is graded. Its prompts, outputs, diffs, grades and keep list go to `perf-runs/<UTC>-h2-skills/` (untracked).

**Measures.** Each run's tool uses, total tokens and duration, from its Agent result. For an edit task the run session, not the subagent, takes the evidence. It keeps a pristine extraction of the task's commit beside the arms' copies. A Python script (`filecmp`, `difflib`; the rtk hook rewrites `diff` and `git diff` output) writes each copy's changed-file list and unified diff against it. The run session then runs the task's check command in each copy and keeps the output. A fresh grader subagent per task scores the three arms against the task's checklist and reference: the diffs, file lists and command outputs for an edit task, the answers for a judgment task. The arms are labelled A–C in a shuffled order the grader isn't told; the run session maps them back.

**Order per skill.** N and O first; then L, written to keep what N missed; then L's runs and the grade.

**Gate, per skill:**
- L passes every checklist item of its tasks: the items are the floor. N and O are references, and an item they both miss still binds L (O misses new-flow items by design).
- L's files weigh no more than O's.
- Every instruction L keeps is an item N missed, a new-flow item, or a denied-command trap (`git restore`, `rm`, `tag`, `stash`, a push past the version branch). The one exception is a required mechanism no task exercises: it stays only when it names the failure it prevents, and the evaluation file lists it as untested. Its keep list goes in the evaluation file.

A missed item earns one revision of L and one rerun of that task. A second miss fails the step's done-when (§6): the skill keeps its old text, with steps 3 and 4's fixes, and the step is marked skipped with the item.

L's tokens over the skill's tasks are read against O's: lower, unchanged within 15 %, or higher. Higher earns one trim round on that skill, then stands as recorded.

**Budget:** 30 task runs and 10 grader runs. A run past 60 tool calls without an answer is stopped and scored as a miss.

### 2.6 The tasks and the four skills

| Task | Skill | Kind and source | Reference | Checklist (each item pass or fail) |
| --- | --- | --- | --- | --- |
| D1 | decision | Edit, in a copy at `c0fc596d^`: record 7.3's benchmark policy from `docs/plans/bench-runner.md`'s design | `c0fc596d`'s `DECISIONS.md` and index diff | a `## D-226 — ` heading; Date, Decision, Why and Consequences; one `**Amended by D-226:**` line under each of `D-070`, `D-078`, `D-141`, above Date and after older markers; a new row at its index section's end, and the three rows' `Amended by`; `python3 -B scripts/check-repo.py` in the copy reports no decision or link error; no other file changed |
| D2 | decision | Edit, in a copy at `d3724465^`: record 7.1's gamepad dependency from `docs/plans/quick-wins.md` §4 | `d3724465`'s `DECISIONS.md` and index diff | a `## D-220 — ` heading; the four fields, its Decision saying "Satisfies `D-015` rule 1" for `gilrs`; one `**Amended by D-220:**` line under each of `D-045`, `D-131`, `D-144`, `D-149`; the new row and the four rows' `Amended by`; the copy's check clean; no other file changed |
| R1 | release | Read-only, on the tree after step 4: "Prepare and release 0.113": every command in order, the owner's block, nothing run that writes (0.112's release task, session 66a95df2, restated) | the guide after step 4 | notes from `git log v0.112.0..HEAD` as a `docs(changelog)` commit; the sitting against `v0.112.0` before the cut, NoMachine disconnected; `just release minor` with a subject of 72 characters at most; the agent pushes `git push origin 0.113` alone, no tag and no `gh` write; the owner's block: tag, tag push, `git switch -c 0.114`, `git push -u origin 0.114`, `gh pr create`; the merge publishes nothing |
| R2 | release | Judgment: the owner pushed `v0.113.0` an hour ago and preflight reports no release run | the guide's recovery table | hands the owner `gh workflow run release.yml --ref v0.113.0`; never re-pushes or moves the tag; verifies with preflight and `gh run watch <id>`; runs no GitHub-changing `gh` |
| X1 | next | Judgment, on session 928a9d47's status output (7.3's run mid-sitting) | that session's reply | nothing that writes while the capture runs, the step marked to wait for it; the step is 7.4's plan; the prompt alone in a `text` block under a label line with Opus 5.5 and xhigh; one prompt; the NoMachine line |
| X2 | next | Judgment, on `perf-runs/20261029-h2-plan/next-state-7.3-close.txt` (7.3 closed, `phase-7` in Now, no 7.4 plan; its sessions line set to none, since the session it showed was the idle 7.3 run) | this plan's own prompt | the next step is 7.4's plan, in a new session; the prompt reads phase 7's Step 3 and the redo's 7.4 row and §4, not phase 7's `run` prompt; it names a `docs/plans/<topic>.md`, the critique and the run prompt; the label line; one prompt |
| X3 | next | Judgment, on X2's fixture with its git lines rewritten to the new flow's unmerged release: branch `0.114` tracking `origin/0.114`, version 0.113.0, newest tag `v0.113.0`, not on `origin/main`, no commits since | the guide after step 4: the merge publishes nothing and gates no work | the next step is the Now item's (7.4's plan), not the merge; the release pull request's merge appears as a heads-up for the owner, with a merge commit and at any time; the label line; one prompt |
| P1 | plan | Edit, in a copy at `6664ed3b`: write `docs/plans/bench-runner.md`'s step 5 back, its section removed first | `6664ed3b`'s step 5 | Files, Change, Done-when, Commit and Moves; done-when as commands with expected results; `scripts/test-perf.py` cases for at least 9 of the reference's 13 behaviours; local commands with exit codes, the quick sitting marked functional only; `just script-test`; a Conventional subject of 72 characters at most; no file outside step 5's list |
| P2 | plan | Judgment: running `bench-runner` as of `6664ed3b`, step 4 needs `crates/tungsten-render/src/text/atlas.rs`, outside its list, and a file outside the list stops the step; steps 1–3 are committed, step 4 has six changed files and one new | the skill's run rules | each changed file restored with `git show HEAD:<path> > <path>` and the new file deleted; no `git restore`, `checkout`, `stash`, `reset` or `rm`; step 4 marked skipped with its reason in the evidence log, committed alone and pushed; carries on with the steps that don't depend on it and names the ones that do; reports it at the end |
| P3 | plan | Judgment: `bench-runner`'s plan at `6664ed3b`, extracted to `<scratch>`, is complete and uncommitted; finish the plan session as the rules require, as commands, with how each finding is recorded (7.3's plan session, af6410ef, did this) | the skill's critique and hand-off rules | the window read before a `--repo` round; `critique --raw --repo --out <scratch file> <plan>` in the background, `--force` only past the soft limit; each finding checked against the repo or a doc before any change, and recorded with what it was checked against, its verdict and its change; a second round only when the first changed a mechanism, with the §7 and log cut and a `--focus-file`; never a self-review in the critic's place; the plan committed alone, the item into Now, the branch pushed; the run prompt under a label line |

The status outputs of X1 come from the transcript (`~/.claude/projects/<this project>/928a9d47*.jsonl`, the tool result holding `== git`).

**What each skill becomes** (L keeps only what its gate allows):

- **`tungsten-decision`:** expected to keep marker placement and order, Amended against Superseded, the old row's marker, the `D-015` phrase (D2) and the ID command.
- **`tungsten-release`**, one wrapper: when a release runs, what is the owner's (the tag and its push, `gh` writes, merges), the rehearsal rule, and the guide's happy path as `just release`. `tungsten-finalize` folds in. `check_docs` first reports a frozen record's dead link or fragment as a note, as it reports archive links (Q5).
- **`tungsten-next`:** `just status` in place of `state.sh`. A `step` prompt: `Use the tungsten-milestone skill. Write the plan for <phase>'s step <N> (<name>) from ROADMAP.md: read docs/plans/<phase>.md's Step <k> and docs/plans/roadmap-redo.md's <N> row and the sections it cites; audit only the code it touches; name it docs/plans/<topic>.md; critique it last as the skill says; commit it and end with the run prompt.` Its skill name becomes `tungsten-plan` at step 8, with the other prompts'. A table row sends a phase plan in Now whose next step has no plan to that prompt, never to the `run` prompt. The "newest tag not on `origin/main`" row becomes a heads-up, since the merge publishes nothing and gates no work. Route, tier and reply rules only where N misses them.
- **`tungsten-plan`**, renamed from `tungsten-milestone` (Q6): the critique mechanics (P3), the restore commands that stand in for denied ones (P2), the hand-off; `skeleton.md` trimmed to the sections plans use. The plans README's Executing and Lifecycle say "deleted when done", as the operating rules do.

### 2.7 Memory pruning (step 9)

- **Scope:** the three hubs, the notes they link and their `MEMORY.md` lines, and the in-game shot recipes 7.2's follow-up names.
- **First:** a backup copy of the memory folder to `perf-runs/<UTC>-h2-memory/before/`.
- **A note goes** when the flow, tool or file it guards is gone (checked with `rg` or `fd` on the tree: the register, `roadmap.json` edits, level C, per-milestone releases and sittings, `bench.py`, squash merges, `release-cut`, `state.sh`, scratch shot drivers), when a check now catches it (named), or when a lean skill or doc now says it (named).
- **Otherwise it stays.** Notes on the same trap merge into one.
- **Output:** `perf-runs/<UTC>-h2-memory/pruned.md` lists each deleted or merged note with its reason; the evidence row gives each hub's links before and after, the folder's file count and `MEMORY.md`'s bytes. Every hub link left resolves to a note's `name:`.

### 2.8 Decision

One entry at step 4, with the next free ID (`D-22[7]` at this writing; recheck before committing): releases on a tag push (redo H2). It records `just release <kind>`, with `minor` and `rehearsal`; a final tag publishing on its push; the release pull request as `main`'s merge, publishing nothing; the next branch started in the owner's block, before the merge; the agent committing the cut; the rehearsal rule. It supersedes `D-079`'s merge-publishes clause (with its "pushing a final tag publishes nothing") and amends `D-215`'s Git bullet's "until H2" clause, each with its marker line and index row. No other entry: §2.2 enforces existing rules, and `just status` and the skills are tools.

## 3. Steps

### Step 1: Retake H1's overhead

- **Files:** this plan's evidence log; `perf-runs/<UTC>-overhead-h1/` (untracked).
- **Change:** §2.1, before anything else changes.
- **Done-when:**
  - `python3 -I -B perf-runs/<UTC>-overhead-h1/classify.py` → both groups' tables and the top-five patterns. The example group: under 22.1 % of tool calls and 20.0 % of output tokens.
  - `python3 -I -B perf-runs/<UTC>-overhead-h1/git_baseline.py .` → a median under 12 record files over 0.109–0.112.
  - The evidence row quotes both, the Phase 7 group and the plans' lines.
- **Commit:** `docs(plans): record H2's retake of H1's overhead`
- **Moves:** none.

### Step 2: The carried proposals decided

- **Files:** as listed for step 2.
- **Change:** §2.2's dispositions. Known issues `:110` and `:113` go and `:108` names 9.8; ROADMAP's Later takes Q1's line.
- **Done-when:**
  - `just deps` → passes.
  - A copy of `deny.toml` at `target/h2-deny.toml` with `serde` added to the deny list → `cargo deny --locked check --config target/h2-deny.toml bans` exits non-zero naming `serde`; the copy deleted.
  - `rg -c 'Bash\(just (api|api-check|notices|template-check|level-check)\)' .claude/settings.json` → 5; `rg -q 'example-03-scene-state|example-04-shader-playground' .claude/settings.json; test $? -eq 1` → exit 0.
  - `git ls-files --eol | rg -q 'i/crlf'; test $? -eq 1` → exit 0; with `.gitattributes` in place, `git status --short` lists only this step's files (nothing renormalized).
  - `rg -q 'roadmap catalog/register|four example launch|synced with the roadmap artifact' docs/agent-setup.md; test $? -eq 1` → exit 0; the client checks' outputs and versions in the evidence row.
  - `rg -c 'tungsten run --script' docs/operating-rules.md` → 1.
  - `rg -q 'Workflow proposals 4, 5, 6, 7 and 9|still allows .cargo run -p example-03' docs/known-issues.md; test $? -eq 1` → exit 0.
  - `just ctx`, `just repo-check` → pass.
- **Commit:** `chore(repo): decide the five carried workflow proposals`
- **Moves:** none.

### Step 3: `just status`

- **Files:** as listed for step 3.
- **Change:** §2.3. `just script-test` runs the new test. The allow list takes `Bash(just status)`. The operating rules' Status gains `just status` and the failure it prevents (status read by hand drifted from the tree). The LLM index's row 91 names `scripts/status.py`, within its 12,288 bytes.
- **Done-when:**
  - `python3 -B scripts/test-status.py` → passes, with §2.3's seven cases.
  - `just status` → exit 0 and six sections; `plans` lists `phase-7` and `health-h2` with their rows.
  - `test ! -e .claude/skills/tungsten-next/state.sh` → exit 0; `rg -q 'state\.sh' .claude docs/LLM_INDEX.md docs/agent-setup.md docs/operating-rules.md; test $? -eq 1` → exit 0.
  - `just script-test`, `just ctx`, `just repo-check` → pass.
- **Commit:** `feat(scripts): add just status, derived from the tree`
- **Moves:** none.

### Step 4: Releases on a tag push

- **Files:** as listed for step 4.
- **Change:** §2.4 and §2.8: the workflow, `release.py`'s kinds and `run` (behind `just release`), the preflight, the recipes, the guide, the operating rules, the docs naming `release-cut`, the decision and its rows. In the three skills, their hand-off, publishing and merge lines only: the agent commits the cut and pushes its branch; the owner's block tags, pushes the tag (which publishes), starts the next branch and opens the pull request; `tungsten-next`'s "newest tag not on `origin/main`" row becomes a heads-up. Steps 6 and 7 rewrite the rest.
- **Done-when:**
  - `python3 -B scripts/test-release.py` → passes, with cases for:
    - `minor` from 0.112.0 → 0.113.0, and a branch named otherwise refused;
    - `rehearsal`'s tag; `patch` refused naming 9.8;
    - a 73-character subject refused before any write;
    - a rerun resuming at the checks when the cut is in place;
    - a failed check leaving the cut uncommitted;
    - the commit's four paths;
    - `--dry-run` writing nothing.
  - `python3 -B scripts/test-release-preflight.py` → passes, with cases for:
    - a final tag on a version branch: tag, tag push, next branch, `gh pr create`, and no `git add -A` and no post-merge block;
    - a pushed final tag with no run → `gh workflow run`;
    - `main` ahead by an earlier release's merge → passes; by a non-merge commit, or by a merge whose parents the branch holds but whose resolution changes a file → refused;
    - the rehearsal block.
  - `rg -q 'pull_request' .github/workflows/release.yml; test $? -eq 1` → exit 0; `rg -c '^\s+- "v\[0-9\]\+\.\[0-9\]\+\.\[0-9\]\+' .github/workflows/release.yml` → 2.
  - `rg -l 'release-cut' justfile scripts docs/releases.md docs/operating-rules.md docs/validation.md docs/known-issues.md .claude; test $? -eq 1` → exit 0.
  - `just release minor --dry-run` → prints `0.113.0` and the chain; `just release rehearsal` → exit 0 and the owner's rehearsal block; `git status --short` → empty after both.
  - `rg -q 'human merging the pull request|merge the release pull request without squashing|Git mutations are human-pasted' .claude/skills; test $? -eq 1` → exit 0.
  - `rg -c '^## D-22[7] — ' DECISIONS.md` → 1; `rg -c '^\*\*Superseded by D-22[7]:' DECISIONS.md` → 1, under `D-079`; `rg -c '^\*\*Amended by D-22[7]:' DECISIONS.md` → 1, under `D-215`; `rg -c 'D-22[7]' docs/DECISION_INDEX.md` → 3.
  - `just script-test` (actionlint included), `just ctx`, `just repo-check` → pass.
- **Commit:** `feat(release)!: publish on a tag push and cut with just release`
- **Moves:** the release flow (`D-079`'s clause, by this step's entry).

### Step 5: The evaluations' setup and `tungsten-decision`

- **Files:** as listed for step 5.
- **Change:**
  - `<scratch>/old/` extracted (§2.5).
  - The X1 status text taken from its transcript.
  - D1 and D2 under N and O; the lean skill written; D1 and D2 under L; graded.
- **Done-when:**
  - `perf-runs/<UTC>-h2-skills/` holds D1's and D2's prompts, each arm's diff, changed-file list and check output, the grades, each run's tool uses, tokens and duration, and the keep list.
  - §2.5's gate holds for D1 and D2, with L at most 2,386 bytes.
  - `just ctx`, `just repo-check` → pass.
- **Commit:** `docs(skills): rewrite tungsten-decision against a no-skill baseline`
- **Moves:** none.

### Step 6: One release wrapper

- **Files:** as listed for step 6.
- **Change:** first `check_docs`' frozen-record rule (Q5), its own commit. Then R1 and R2 under N and O, the merged `tungsten-release`, R1 and R2 under L, graded. Then `tungsten-finalize` and its symlink deleted (`python3 -c` to remove, `git add <path>` to stage, as `git rm` is denied), with its `.gitignore` line, the guide's links, agent-setup's skills line and the LLM index's row 92.
- **Done-when:**
  - `python3 -B scripts/test-check-repo.py` → passes, with a frozen record's missing link and missing fragment reported as notes, and an unfrozen doc's as errors.
  - §2.5's gate for R1 and R2, with L at most 1,932 bytes; the guide's bytes recorded against 25,773.
  - `test ! -e .claude/skills/tungsten-finalize && test ! -L .agents/skills/tungsten-finalize` → exit 0; `rg -q 'tungsten-finalize' .gitignore docs/releases.md docs/agent-setup.md docs/LLM_INDEX.md .claude; test $? -eq 1` → exit 0.
  - `just script-test`, `just ctx`, `just repo-check` → pass.
- **Commits:** `fix(scripts): report frozen records' dead links as notes`; `docs(skills): fold tungsten-finalize into a lean tungsten-release`
- **Moves:** none.

### Step 7: `tungsten-next` and the step prompt

- **Files:** as listed for step 7.
- **Change:** X1–X3 under N and O; the lean skill with §2.6's `step` prompt, its table row and the merge row as a heads-up; X1–X3 under L; graded. `prompts.md` stays a file unless the eval shows the split costs reads and the body stays under 8,192 bytes (Q11).
- **Done-when:**
  - §2.5's gate for X1–X3, with L at most 8,759 bytes.
  - `rg -c '^\| .step. \|' .claude/skills/tungsten-next/prompts.md` → 1 (or the row in `SKILL.md`, by Q11).
  - `just ctx`, `just repo-check` → pass.
- **Commit:** `docs(skills): rewrite tungsten-next lean, with a phase step's prompt`
- **Moves:** none.

### Step 8: `tungsten-plan` in place of `tungsten-milestone`

- **Files:** as listed for step 8.
- **Change:**
  - P1–P3 under N and O; the lean skill, under its new name; P1–P3 under L; graded.
  - The rename: folder, symlink, `.gitignore` line, the links in `phase-7.md` and this plan, the LLM index, agent-setup, `tungsten-next`'s name in its prompts and route line, the checker's comment and the test fixture's path.
  - The plans README's Executing and Lifecycle.
- **Done-when:**
  - §2.5's gate for P1–P3, with L's files at most 10,162 bytes.
  - `test -f .claude/skills/tungsten-plan/SKILL.md && test -L .agents/skills/tungsten-plan && test ! -e .claude/skills/tungsten-milestone && test ! -L .agents/skills/tungsten-milestone` → exit 0.
  - `rg -l 'tungsten-milestone' AGENTS.md docs/LLM_INDEX.md docs/agent-setup.md docs/plans/README.md docs/operating-rules.md docs/releases.md .claude scripts .gitignore; test $? -eq 1` → exit 0; no link names it: `rg -q '\]\([^)]*tungsten-milestone' docs/plans/phase-7.md docs/plans/health-h2.md; test $? -eq 1` → exit 0.
  - `rg -q 'superseded plans move to' docs/plans/README.md; test $? -eq 1` → exit 0.
  - The four skills' Markdown files weigh less in all than the old five's 23,239 bytes (A10, `tungsten-finalize` included; `state.sh` left out as a script), by `wc -c` over `.claude/skills/tungsten-{plan,next,release,decision}/*.md`.
  - `python3 -B scripts/test-check-repo.py`, `just ctx`, `just repo-check` → pass.
- **Commit:** `docs(skills): replace tungsten-milestone with a lean tungsten-plan`
- **Moves:** the planning skill's name (Q6).

### Step 9: The memory hubs pruned

- **Files:** as listed for step 9.
- **Change:** §2.7.
- **Done-when:**
  - `perf-runs/<UTC>-h2-memory/before/` holds the folder as it was, and `pruned.md` gives each removal's reason.
  - Every `[[name]]` in the three hubs resolves to a note's `name:`, by a scratch script → prints nothing.
  - The evidence row gives each hub's links before (61, 42, 13) and after, the folder's files (168 before) and `MEMORY.md`'s bytes (12,871 before).
  - `rg -q 'scratch-driver recipes' docs/known-issues.md; test $? -eq 1` → exit 0.
  - `just repo-check` → passes.
- **Commit:** `docs(known-issues): drop the shot recipes 7.2 replaced`
- **Moves:** memory notes, outside the tree.

### Step 10: Close

- **Files:** as listed for step 10.
- **Change:**
  - The checks and the demo.
  - The follow-ups to their homes: Q1's Later line exists from step 2. F3 in phase 7: the overhead retaken on 7.5's sessions, the first under H2's skills, with step 1's scripts. A known-issues line: the first release pull request merged after H2 starts no `release.yml` run (`gh run list --workflow release.yml --event pull_request`), then the line goes (§2.4, the transition).
  - Phase 7's Step 3 evidence row; F2 marked done.
  - This plan and its citations deleted; the scratch copies gone (§2.5 deletes each after its grade).
  - `health-h2` out of Now; `phase-7`'s line says 7.4 landed and 7.5 is next.
- **Done-when:**
  - `just check`, `just script-test`, `just ctx`, `just repo-check`, `just deps`, `git diff --check` → pass.
  - The demo, quoted:
    - `just status` → its six sections;
    - `just release rehearsal` → the owner's block;
    - a fresh subagent with the lean `tungsten-next` on the tree after the deletion → 7.5's plan as the `step` prompt.
  - `rg -n '^\| [^|]+ \| [^|]+ \| 7\.4 \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0.
  - `rg -l 'plans/health-h2\.md' --glob '!CHANGELOG.md' --glob '!DECISIONS.md' .; test $? -eq 1` → exit 0; `just repo-check` → passes.
- **Ends with:** the commits made, the evidence quoted, what the owner should try (the rehearsal block, `just status`), and the next prompt (7.5's plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Retire the per-version branches now (H1's Git row)? | No. Agents would push `main`, which its pull-request rule (A6) and the push guard (A7) refuse, and both are the owner's settings. H2 removes what the branches cost instead: no post-merge block, the next branch started in the release block. A Later line names both settings |
| Q2 | The kinds' names | `minor`, which semver already treats as breaking in 0.x, and `rehearsal`; the rest refuse until 9.8 |
| Q3 | The next branch before the merge, or after it? | Before, from the version branch's tip in the same block (§2.4); containment admits the merge commits `main` holds |
| Q4 | Who commits the cut? | The agent, through `release.py run`, naming its four paths (`D-215`); the owner's block holds no `git add` or commit |
| Q5 | Frozen records' links to removed skills | Reported as notes, as archive links are; frozen records are never edited, and 9.7 deletes them |
| Q6 | Rename `tungsten-milestone`? | Yes, to `tungsten-plan`: milestones ended at M85, and the redo asks for a planning skill "in place of" it |
| Q7 | One run per arm, or repeats? | One; behaviour gates (every checklist item, for L), tokens are read with a 15 % band, and a missed item earns one revision and rerun |
| Q8 | H2's own "lower" before any session runs under it | The skills' bytes against the old (gated) and the evaluations' tokens (read); the session-level retake under H2's skills at 7.5's close (F3) |
| Q9 | A decision for `.gitattributes`? | No: the file states its rule where git applies it |
| Q10 | The check chain in the script or the guide? | The script, so one command runs it; `--dry-run` prints it |
| Q11 | Fold `prompts.md` into `tungsten-next`'s `SKILL.md`? | Only if the evaluation shows the split costs reads and the body stays under 8,192 bytes; else the file stays |
| Q12 | The owner hasn't rehearsed when the next release runs | The release skill's rule: rehearse first, then cut |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** the release flow (`D-079`'s clause, step 4); `state.sh` (gone, step 3); the skills' names and files (steps 6 and 8); memory notes (step 9).
- **Must not move:**
  - Rust code and what it produces. No crate, example, template or tool source changes, so every hash, digest, counter pin, golden image and guard stands, and `just check` passes unchanged.
  - `release.yml`'s build and publish jobs: archives, notices and checksums.
  - The frozen records: `just status`'s frozen section reads no edits since `D-215`.
  - `CHANGELOG.md` and existing decision bodies.
  - The `api/` snapshots.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it. Step 6's R1 and R2 need step 4; step 7's L needs step 3; steps 6 and 8 need step 6's checker rule.
- A step needs Rust code changed: stop that step and record it as a follow-up.
- The retake reads not lower: record the top patterns and carry on, since steps 5–8 target them; the close carries the miss to 7.5's triage.
- An evaluation run passes 60 tool calls or returns no answer: score it as a miss. A lean skill missing a checklist item twice fails its step (§2.5): the old skill stays, and the step is marked skipped with the item. Across steps 5–8, past 8M tokens: start no new runs, finish the current skill, and skip the remaining skills' steps with their reason.
- `release.yml` can't be checked locally past actionlint: never push a tag to test it (tags are the owner's); the rehearsal is handed over.
- A push is refused: report it, never force (`D-216`).
- Another session edits a file a step lists: stop and report.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-29, reading `c8105ad6`. Footer: `352.6 s, tokens in 814845 (cached 728192), out 8271 (reasoning 4786), commands 22, output 284.6 KB (largest 35.8 KB), usage 5h 2% → 23%, week 34% → 37%`. Six findings, all accepted; they changed done-when checks (step 4's preflight cases, steps 5–8's gate), the evaluation procedure and step 1's method, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Merge-only ancestry does not establish that the release branch contains main's changes | A merge can carry its own resolution, which `--no-merges` admits; the old preflight compared trees only for a merged pull request (`scripts/release-preflight.py:176-180`) | accepted | §2.4: each commit the base holds beyond the release is a merge whose second parent the release holds and whose tree equals it; step 4 tests a merge with its own resolution |
| 2 | [MEDIUM] The skill gate allows both implementations to miss required behavior | O read the old guide (`docs/releases.md:18`, `:94`), and the old routing sends a Now item's large plan to its run prompt (`.claude/skills/tungsten-next/SKILL.md:42`), so new-flow items could fail in O and L alike | accepted | §2.5: every checklist item binds L; O follows the old skill with the tree's current docs; a second miss fails the step (§6) |
| 3 | [MEDIUM] The overhead retake counts previous releases across squash boundaries | `git merge-base v0.108.0 v0.109.0` → `bb1e4c0e` (0.107); 0.108's squash `d31ee9bb` and 0.109's `433b4998` are single-parent commits on this history; `v0.109.0` is no ancestor of `v0.110.0` | accepted | §2.1: `<from>` is `d31ee9bb` for 0.109 and `433b4998` for 0.110, the previous tag after |
| 4 | [MEDIUM] Grading answer files cannot verify the edit-task checklists | §2.5 gave the grader answers only, while D1's, D2's and P1's checklists need each copy's state | accepted | §2.5: the run session diffs each copy against a pristine extraction in Python and runs the check itself; the grader scores diffs, file lists and outputs; step 5's evidence names them |
| 5 | [MEDIUM] The next-step evaluations omit the new flow's unmerged-release state | `.claude/skills/tungsten-next/SKILL.md:40` makes the release pull request's merge the next step while the newest tag is off `origin/main`; X2's fixture has its tag on main (`next-state-7.3-close.txt:4`) | accepted | §2.6: X3, a published tag with its pull request unmerged; the merge row becomes a heads-up; step 7 runs X1–X3 |
| 6 | [MEDIUM] The claimed first-merge no-op contradicts the old workflow | `.github/workflows/release.yml:75` takes the "already released" exit only when the tag names another commit; a matching head builds and reaches `gh release create` (`:244`) | accepted | §2.4's transition: a pull request's run reads the workflow from its merge ref, which drops the trigger; the old path's worst case is a failed publish; step 10 adds the first release's check to known issues |

Round 2: the same reviewer at xhigh, 2026-10-29, reading `fe83ae90` with the Critique section and the log cut and a focus note on round 1's changes. Footer: `417.1 s, tokens in 738864 (cached 652800), out 11692 (reasoning 8072), commands 36, output 237 KB (largest 23.3 KB), usage 5h 23% → 44%, week 37% → 40%`. Three findings, all accepted; no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] A skipped skill rewrite restores instructions incompatible with the new release flow | A skipped step 7 keeps `tungsten-next`'s merge-first row (`.claude/skills/tungsten-next/SKILL.md:40`); a skipped step 6 keeps "Git mutations are human-pasted" (`.claude/skills/tungsten-release/SKILL.md:10`, and `tungsten-finalize`'s), while `just release` commits the cut | accepted | Step 4 rewrites the three skills' hand-off, publishing and merge lines, with a check, so a failed gate leaves them on the new flow |
| 2.2 | [MEDIUM] The historical evaluation copies can read the live repository's answers | `git -C target rev-parse --show-toplevel` resolves to the live root, whose `DECISIONS.md` holds `D-225` and `D-226`; each copy also carries its era's skills | accepted | §2.5: copies and skill copies in the run session's scratchpad, outside any repository; copies stripped of their skill folders; a transcript scan voids contaminated runs |
| 2.3 | [MEDIUM] The retention gate excludes mechanisms the plan explicitly expects to retain | No task exercised the critique mechanics (`.claude/skills/tungsten-milestone/SKILL.md:21`) or a dependency decision's `D-015` phrase (`.claude/skills/tungsten-decision/SKILL.md:28`) | accepted | §2.6: D2 becomes 7.1's `D-220` (`gilrs`, `D-015` rule 1, four amendments), and P3 adds the critique and hand-off; §2.5 keeps an unexercised required mechanism only with its failure named, listed as untested |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-29 | done, lower | `classify.py`, main transcripts: the example group's bookkeeping 1,055 of 5,098 calls, 20.7 % (baseline 22.1 %), and 16.5 % of output tokens (20.0 %), 15.1 % and 10.6 % with each item's own plan as work; with `--subagents` 20.2 % and 16.3 %; gate passes. Its top five: memory 139, `e01-foes.md` 92, `docs/plans/1.0/` 87, `README.md` 84, `e01-flask.md` 74. Release sessions ran 51–65 %, plan sessions 41–50 %. Phase 7, apart: 784 of 2,572 calls, 30.5 %, and 29.9 % of output tokens (24.1 % and 22.3 % own plan as work); top five `DECISIONS.md` 79, `bench-runner.md` 71, `DESIGN.md` 64, `agent-tooling.md` 57, `known-issues.md` 54; its plan sessions 57–72 %, 0.112's release 61 %. `git_baseline.py .`: record files 11, 9, 7 and 11 over 0.109–0.112, median 10 (baseline 12); gate passes. Plan lines at deletion, eight plans: 425, 516, 380, 1,567, 245, 213, 290, 692, median 402.5 (baseline 312, not gated) | `perf-runs/20261029-overhead-h1/` (scripts, `classify-main.txt`, `classify-sub.txt`, `git.json`) |
| 2 | 2026-10-29 | done | `just deps` → advisories, bans, licenses, sources ok. The plan's probe order exits 2 on the argument (cargo-deny 0.20.2 takes `--config` before `check`); `cargo deny --locked --config target/h2-deny.toml check bans` with `serde` added → `error[banned]: crate 'serde = 1.0.229' is explicitly banned`, `bans FAILED`, exit 2; the copy deleted. Allow list → 5, merged launches → none; `i/crlf` → none, and `git status --short` lists only this step's eight files; agent-setup's three stale phrases → none; `tungsten run --script` → 1; known issues' two lines → none. Client checks: Claude Code 2.1.311 (the VS Code extension's; `PATH`'s `claude` is still 2.1.110) at the root loads the root `CLAUDE.md` and its `AGENTS.md`; in `crates/tungsten-render/`, the crate's `CLAUDE.md` and `AGENTS.md` and the root `CLAUDE.md` without its import; the headless root run ignored the allow list (untrusted workspace). Codex CLI 0.160.0: the root `AGENTS.md`, then the root and render files. `just ctx`, `just repo-check` → exit 0 | `docs/agent-setup.md` |
| 3 | 2026-10-29 | done | `python3 -B scripts/test-status.py` → 10 tests OK: the seven cases (commits since the newest final tag on HEAD, a prerelease and an off-history tag ignored; a waiting cut; Now's count; a plan's rows, the 1.0 folder left out; a frozen-record edit after the `D-215` commit; a dirty file and its age; a missing `/proc`), the section order, a clean tree, the CLI. `just status` → exit 0, 6 sections; plans: `health-h2.md: in progress · steps: 10 · evidence rows: 2 · last: step 2, done`, `phase-7.md: draft · steps: 17 · evidence rows: 2 · last: step 2 (7.3), passed`. `state.sh` gone, no reference left. `just script-test` (8 suites OK), `just ctx`, `just repo-check` → exit 0. LLM index 12,279 of 12,288 bytes | `scripts/status.py`, `scripts/test-status.py` |
| 4 | 2026-10-29 | done | `test-release.py` → 53 OK, `Run`'s 8 cases among them (minor 0.26.0 → 0.27.0 committing its four paths, then a rerun repeating only the preflight; a branch named otherwise refused; the rehearsal tag and its `.2` suffix; `patch`, `major`, `rc` and `promote` refused naming 9.8; 73 characters refused before any write, 72 passing; a failed `just smoke` leaving the cut uncommitted and a rerun running the checks alone; a dirty file refused; `--dry-run` writing nothing; the CLI). `test-release-preflight.py` → 22 OK (a version branch's agent push and the owner's chained block: tag, tag push, `git switch -c 1.2 1.1`, `git push -u origin 1.2`, `gh pr create`, no `git add`, no post-merge block; a pushed tag with no run → `gh workflow run`; `main` ahead by an earlier release's merge passes, by a merge with its own resolution or by a commit refused; the rehearsal block). `pull_request` in `release.yml` → none; the two tag patterns → 2; `release-cut` in the listed paths → none; the skills' old hand-off phrases → none; `D-227`'s heading and markers → 1, 1, 1, its index lines → 3. `just script-test` (actionlint included), `just ctx`, `just repo-check` → exit 0. After the commit, on a clean tree: `just release minor --dry-run` → exit 0, `Release 0.113.0 (minor) on 0.113, after v0.112.0: cutting.` and the chain (ratchet, cut, lock refresh, the ten checks, the commit, the preflight); `just release rehearsal` → exit 0, `v0.0.0-test.20261029.gb0b4e52` checked, the live preflight passed with the agent's branch push and the owner's tag and tag push; `git status --short` → empty after both | `scripts/release.py`, `scripts/release-preflight.py`, `.github/workflows/release.yml`, `docs/releases.md` |

## Follow-ups
