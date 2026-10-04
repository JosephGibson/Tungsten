# Tungsten 1.0 workflow — draft

- **status:** in progress
- **goal:** Say how the road to 1.0 runs as Claude Code sessions in this repository's existing style: the kinds of session, how much each candidate can run without the owner, what every milestone owes before it releases, how sessions share the tree and the reference machine, where the owner is needed, and the tooling that has to exist first.
- **non-goals:** The order of work and the gates ([implementation plan](implementation-plan.md)); scoping ([criteria](criteria.md) and the workstream files); rules that already have a home ([plan rules](../README.md), [profiling workflow](../../perf/profiling-workflow.md), [release procedure](../../releases.md)), which this file links instead of copying; editing `AGENTS.md`, skills or scripts, which §8 proposes for Step 0.
- **files to touch:** This file.
- **ordered steps:** (1) The owner agrees §1–§7 at the definition gate. (2) Step 0 lands or rejects each item in §8. (3) The first milestone plan cites §4 and §5 instead of copying them, and later plans do the same.
- **done-when:** The owner has agreed this file; every §8 item has landed or been rejected; the first milestone plan cites it.

Drafted 2026-10-03 on branch `0.40` (`941b63e`), from the plans and evidence logs of 0.34–0.40, the release procedure, and a planning session that shared the tree with the unattended 0.40 QA session (§6). Revised the same day for `D-097` and `D-098`: steps end in local commits, not a patch series. Revised on `9cd5709` (0.40 merged): the index budget re-read, step 9's outcome, the release count (§1) and scratch-copy capture provenance (§2). Agreed at the definition gate on 2026-10-03, with one release per candidate (implementation plan §11). Revised on `db1177c` for `D-105`: from 0.42 sessions leave their work uncommitted, and the release commit is a milestone's only commit. Revised on 2026-10-04 (`db1177c`, uncommitted) to cut the owner's prompts per candidate: a milestone's release is its plan's last step, run in the execution session (§1–§3, §5); each session ends with the next one's prompt (§3); the owner's diff review, command block, merge and post-merge block make one sitting (§9); two small adjacent candidates may share a plan and a release (`D-108`, §1). Revised on 2026-10-04 for `D-109`: gate and graduation sessions update `roadmap.json` with the register (§2), and `/tungsten-next` syncs the roadmap page from the tree (§9).

## Context digest

- The road to 1.0 is about 45 candidates in three phases with six gates ([implementation plan](implementation-plan.md)). By default each candidate becomes one milestone plan and one `0.NN` release, as each Phase 4 milestone and each pass since did.
- Practice this file keeps, from 0.34–0.40: a plan, not the chat, is the hand-off between sessions; the owner approves a plan's open questions, each with a stated default; steps run one per session, or in an unattended multi-step session with stop conditions (0.40 QA steps 3–14 ran that way; step 9 was skipped on a `regressed` verdict with its files restored and landed in the close-out, steps 15–29); the owner pushes, tags and merges (`D-097`); every check is quoted in an evidence log; perf verdicts follow the profiling workflow; decisions go through the [tungsten-decision](../../../.claude/skills/tungsten-decision/SKILL.md) skill; one release pull request per milestone branch.
- Limits that shape it, on 2026-10-03: the root `AGENTS.md` has 17 B of its 6,144 B budget left and `docs/LLM_INDEX.md` 195 B of 8,192 B (`just ctx` at `9cd5709`; the index had 599 B before the 0.40 close-out added rows). The reference machine is the only GPU, display and perf host. Captures wait for the remote-desktop encoder to exit, and `D-095` invalidates a capture during which any tracked file changes. The disk filled during 0.40 (step 3 freed 34 GB of stale incremental caches).
- From 0.42 sessions don't commit (`D-105`): a milestone's work stays uncommitted on its branch until the release session's command block makes the release's only commit, so the tree holds every session's changes until then (§6).

## 1. Units of work

| Unit | Lives in | Written by | Ends when |
| --- | --- | --- | --- |
| Candidate | [Implementation plan](implementation-plan.md) §3–§5 | Gate sessions | Its milestone plan exists |
| Milestone plan | `docs/plans/1.0/phaseN-milestone-NN-slug.md` | A plan session | Its release step sets `status: done` and archives it before the cut |
| Step | A section of the milestone plan | An execution session | Its done-when is quoted in the evidence log |
| Change | The milestone branch's working tree, uncommitted (`D-105`) | Gate, graduation, plan and execution sessions | The release commit takes it |
| Release | Branch `0.NN`, a tag, the release pull request | The milestone plan's last step, in its execution session; a release session for a resume or a release with no plan | The owner merges the pull request and pastes the post-merge block |
| Spike | `perf-runs/<date>-<slug>/`: patch, scripts, captures and a README with the verdict | A spike session | The verdict is in a gate record |
| Gate | A dated record in the implementation plan (§11 there) | A gate session with the owner | The owner signs the record |

Defaults: one candidate, one milestone, one release, which makes the road about 45 releases from 0.41, each with the owner's ship sitting (§9). Two small candidates may share one plan and one release when both are level A or B (§4), touch different files and sit next to each other in the queue, as W11a with W7a does (`D-108`, which revises the definition gate's one release per candidate, implementation plan §11); the register holds each pair as one row. A large candidate splits into milestones of its own (W1's ladder). A milestone takes its number when its plan is written. Spikes and gates take no number and release nothing.

The release is the milestone plan's last step. The session that runs the steps follows the [tungsten-finalize](../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../.claude/skills/tungsten-release/SKILL.md) skills after them, so the cut happens once and the checks run once, after the cut, and it ends with the changed-file list, the command block, the post-merge block ([releases](../../releases.md#handoff-and-next-branch)) and the next prompt (§3). A separate release session stays for resumes and for releases with no milestone plan, such as 0.42.

## 2. Session types

| Session | Reads | Produces | Owner |
| --- | --- | --- | --- |
| Gate | The gate's row, its agenda, the questions it needs and the spikes' verdicts | Answers, decision entries, graduated workstream files, revised tiers, checklist and backlog, `roadmap.json` updates in the same change as the register (new stops, levels, pairs, groups, question states; `D-109`), and a dated gate record; docs only | Present: answers the questions |
| Graduation | One settled criteria section and its workstream file | The section moved verbatim into the file with its links fixed, a placement paragraph left behind ([conventions](README.md#conventions)); the file's steps are its candidates in order, each with a done-when; `roadmap.json` follows any register change (`D-109`) | Reviews the diff |
| Plan | The candidate's row and card, its workstream file or criteria section, this file, the `docs/LLM_INDEX.md` rows for the code it touches, the decisions it cites | One milestone plan, critiqued last and revised, the critique recorded in it; then the approval prompt, and once approved, the run prompt | Approves the open questions |
| Execution | The plan, then only the files the step names | An evidence-log row per step and one `CHANGELOG.md` line, left uncommitted; the last step releases (§1) and ends with both blocks and the next candidate's plan prompt | Ships it in one sitting (§9) |
| Spike | The spike's question in its gate row | Evidence and a verdict in `perf-runs/`; nothing merged | Reads the verdict at the gate |
| Release (resume, or no plan) | The [release procedure](../../releases.md) | The cut, every check, the command block, which makes the release's only commit, and the post-merge block; the plan archived and the register filled in | Pastes the blocks and merges |
| QA pass | The tree, as the 0.40 pass read it | An audit plan with findings and numbered steps, then its execution | Approves the plan |
| Acceptance game | The game's own repository and the engine's public documentation | Game code and gap rows (§7) | Plays it at the gates |

**Plan sessions** follow the shape of the 0.40 QA plan. A focused audit of the code the candidate touches comes first, each finding with `file:line` evidence and the commit it was read at. Then come steps that each fit one session, each naming its files; done-when checks written as commands with their expected results; open questions with a default each; the decision IDs it expects (the next free ones when it runs); the invariants it may move and those it must not; its stop conditions; and an empty evidence log. Structure comes before full reads (`ast-grep`, `rg -l`, the index), and a file is read whole only for surgery in it. The last planning step, once the plan is complete and before approval, is an antagonistic critique from another model family, by default the user-level `critique` skill, as criteria.md had twice ([tungsten-milestone](../../../.claude/skills/tungsten-milestone/SKILL.md) step 7): one round, a second only when the first changed the steps or a done-when check. The plan records each finding with what it was checked against, its verdict, and the change or the reason it was rejected. Approval is a line under the questions, as in 0.40: "Approved <date> with the stated defaults", plus any change. The plan session ends with that approval as a prompt for the owner to paste; once pasted, it adds the line and ends with the run prompt. Execution starts only after it.

**Execution sessions** follow the [plan rules](../README.md#executing-a-plan), one step per session by default. A plan whose candidate is level A or B (§4) may run as an unattended multi-step session: a failed check or a stop condition restores that step's files (plain `cp` from copies taken before the step, then `cmp`; `git show HEAD:<path>` only for a file the milestone had not yet changed, since `HEAD` is the previous release) and records the step as skipped; later steps that do not depend on it continue. State lives in the plan and the working tree, so a session can compact with its focus on the remaining steps, or restart, without losing anything. The last step is the release (§1); a stop condition that ends the run before it leaves the release to a release session, after the owner has seen why.

**Spike sessions** work in a scratch copy of the tree, so the gate can still reverse the idea, and keep the patch, scripts, raw captures and a README with the verdict in `perf-runs/<date>-<slug>/`, as the 2026-10 GPU-pass and physics probes did. A capture taken from a copy under `target/` records the enclosing repository's commit and dirty hash as its provenance (known issues), so the README names each capture's tree by hand.

**QA passes** close Phase 5 and Phase 6 (implementation plan amendment 18), in the 0.40 shape: audit, findings, approval, steps, close-out compare.

## 3. Restart prompts

A plan is the hand-off, so a prompt names the plan and the scope ([plan rules](../README.md)). Each session ends with the next one's prompt: a plan session with the approval, the approval with the run, a run with the next candidate's plan. `/tungsten-next` then runs once per release, after the merge, to sync the roadmap, or to resume after a crash.

The prompts, their keys and the order each kind of stop runs them in live in one place, [tungsten-next's prompts](../../../.claude/skills/tungsten-next/prompts.md). A release session runs the [tungsten-finalize](../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../.claude/skills/tungsten-release/SKILL.md) skills.

## 4. Autonomy per candidate

Each candidate's card in the implementation plan sets its level, and its plan's approval may change it.

| Level | Fits | Owner touchpoints | Examples |
| --- | --- | --- | --- |
| **A** Unattended | Mechanical work whose done-when checks are commands: bug fixes with a failing test first, CI jobs, packaging | Plan approval, the release | W7a; the fix steps of a QA pass |
| **B** Approved design | The plan's open questions settle the design, and the steps are then mechanical | Plan approval, including any API sketch; the release | W8a, W9a, W11a, W12a, W14a, W3a, W3b, W1 M0a and M0b, W2 R1 and R2 |
| **C** Owner in the loop | New public API whose shape is the deliverable and which the freeze fixes | Plan approval; a review of the API sketch and the template diff after the first step, before the rest run; the release | W2 R0's ID API, W15a–c, W1 M1–M3, W16a–b, the kit |
| **S** Spike | Evidence for a gate | The verdict, at the gate | The R1 spike, the tuple-query spike, the frame-loop design, the glyph-path prototype |

Level C exists because the freeze makes API taste expensive to fix later. Every public name ends up in the break ledger or the freeze snapshot, and the template shows it to every game.

## 5. What every milestone owes before its release

The release step checks each line, or the release session for a release with no plan. A missing line becomes a step, not a footnote.

1. **Evidence.** Each step's done-when is quoted in the evidence log, or the step is marked skipped with its reason.
2. **Gates by change.** The [AGENTS.md](../../../AGENTS.md#tests) table applies: `just check` always; layer 1, `just smoke`, `just visual`, `just physics-release` and `just script-test` as the change requires. The release step runs the release checks (`just check`, `just smoke` and `just script-test` among them) once, after the cut, with any gate here that no step ran. Perf rows that its code touches read "not `regressed`" ([done-when rules](../../perf/profiling-workflow.md#writing-done-when-checks)); a change to shared ECS or frame-loop code captures every CPU row.
3. **Invariants.** The determinism hash, the pinned containment hash, the row digests, `gpu-visual.png` and the post and transition regressions are unchanged, or a step moves them on purpose and records the new values in the evidence log and in its `CHANGELOG.md` line.
4. **Decisions.** Written with the tungsten-decision skill, their index rows in the same change, and landed before the docs that cite them.
5. **API.** Every public-API break has a row in the [break ledger](w04-api-freeze.md#break-ledger), new public surface is listed for the freeze, and every new public item has rustdoc, since `missing_docs` gates the freeze.
6. **Template.** Once W12a lands, a milestone that changes how a game is written updates `templates/basic` in the same step, and once W9b starts, the getting-started guide too.
7. **Routes.** Each new file that a task would open has a `docs/LLM_INDEX.md` row; a new `AGENTS.md` rule comes only with its decision; `just ctx` passes.
8. **Design.** An architecture change has its `DESIGN.md` section, as each Phase 4 milestone had.
9. **Records.** Known issues lose the findings it fixed and gain the ones it found out of scope; the backlog gains a row for anything it cut; the acceptance game's gap log marks the rows it closed; the implementation plan's register, the workstream file and this folder's README "Now" lines are current.
10. **Close.** One `CHANGELOG.md` line per plan (`D-097`); the release step sets `status: done` and archives the plan per the [lifecycle](../README.md#lifecycle) before the cut.

## 6. Sharing the tree and the machine

On 2026-10-03 a planning session and the unattended 0.40 QA session worked in the same tree. The QA session was waiting for the remote-desktop encoder to exit before a capture, and its step 0 had already lost an A/A pair to a documentation session's edits. The [capture rules](../../perf/profiling-workflow.md#comparison-rule-and-capture-rules) cover the machine; these rules cover sessions:

1. **The release commit takes the tree.** Sessions don't commit (`D-105`), and the release block stages with `git add -A`, so everything in the tree ships with the next release. A session edits only the paths its plan names, never reverts another session's changes, keeps scratch files out of the tree and lists the files it changed in its report. Two sessions that edit the same files run one after another, or in separate clones.
2. **No writes during a capture.** A second session (planning, review or Codex) drafts in its scratchpad, then copies its files in only while `pgrep -af scripts/bench.py` shows no capture, or it works in a separate clone that the owner creates. A session that only reads is safe.
3. **Captures need the owner away.** Capture steps run in unattended sittings after the owner disconnects, and owner-facing work (gates, approvals, API reviews) happens while the owner is connected. A session that must capture waits for the encoder to exit (`until ! pgrep -x nxcodec.bin …`), as the QA session did, rather than capturing on a busy machine. Each sitting is one blocking foreground command.
4. **GPU checks build.** `just smoke` and `just visual` need the display, not a quiet machine, but they build, so they never run inside another session's capture.
5. **Disk.** Check free space before an outside-copy check, a game-repository build or a perf sitting. `target/debug/incremental` is a disposable cache. The acceptance game's repository builds into a target folder of its own, so budget for it.
6. **Parallel tracks are serial by default.** Track A's candidates touch different crates, but they share one tree and one capture machine, every change in the tree goes into the next release commit, and the owner reviews each. A second track runs in parallel only in a separate clone, with its own branch and with captures scheduled around the first.

## 7. The acceptance game's repository

The game starts from `templates/basic` when W15a lands (implementation plan §4), in its own repository, on a git dependency at a tag.

- **Sessions there** read the template's rules (amendment 16), the getting-started guide and rustdoc. Reaching into the engine's source to finish a feature is a documentation gap and gets a gap row.
- **A gap** is a row in the game's `GAPS.md`: date, what the game needed, a minimal repro, the workaround if any. The game never patches the engine.
- **Triage.** The next engine plan or release session copies new rows into the [gap log](acceptance-game.md#gap-log) and files each as a candidate step, a backlog row or "not a gap".
- **Engine releases.** The game moves to each release by bumping its tag. A breakage that the break ledger did not announce is a gap row too.

## 8. Tooling for Step 0

Step 0 of the implementation plan teaches the repository check to read this folder. Proposed beside it, each in its own session, all before the first milestone plan:

| Item | Why | Proposal |
| --- | --- | --- |
| 0a Plan check | `just repo-check` reads only `docs/plans/*.md` | As the implementation plan describes in §3 |
| 0b Context headroom | 1.0 decisions add `AGENTS.md` rules: the kit's row in "Where code goes" (W13), a prefabs row in the assets table (W16), the threading rule in place of a hard rule (W2) and the game layout (W12). The index needs about a dozen rows: the clock, the schedule, logs and user files, the harness, UI, the kit, the CLI, the template and prefabs | A decision before W11a: raise the budget of the on-demand index, and keep `AGENTS.md` within its budget by moving task-specific detail behind links, or raise both |
| 0c Milestone skill | About 45 plans would repeat the same execution rules; 0.40's ran to about 3 KB | `tungsten-milestone`: routes to the plan rules, this file and the decision and perf skills, and holds the milestone plan skeleton, so each plan cites it instead |
| 0d API snapshot | The freeze otherwise finds new public surface all at once | A recipe that writes the public surface to a tracked file each milestone updates, so the diff shows in review; the tool is the definition gate's choice (amendment 17) |

All four landed on 2026-10-03 and 2026-10-04, uncommitted for the 0.42 release commit (`D-105`): 0a in `scripts/check-repo.py` with a case in `scripts/test-check-repo.py`; 0b as `D-106` (the index budget is 12 KiB, and `AGENTS.md`'s asset table moved to [docs/assets.md](../../assets.md)); 0c as the [tungsten-milestone](../../../.claude/skills/tungsten-milestone/SKILL.md) skill with its plan skeleton; 0d as `D-107` (`just api` writes `api/<crate>.txt` from the pinned toolchain, and the release checks run `just api-check`).

## 9. Owner touchpoints

A level B candidate takes five: `/tungsten-next` once, after the previous merge; the plan prompt; the approval; the run prompt, whose session also releases; and the ship sitting. Level C adds the API review. A pair (§1) takes the same five for two candidates.

| Touchpoint | How often | Kept short by |
| --- | --- | --- |
| Gate | Six times | An agenda with a default per question, written before the session (implementation plan §9 for the first) |
| Plan approval | Each milestone | Open questions with defaults; approval in one line, handed over as a prompt |
| API review | Level C candidates, after the first step | The API sketch and the template diff, not the whole diff |
| Ship | Each milestone | One sitting after the run: the uncommitted diff, read before the release commit (`D-105`); the command block; the merge; the post-merge block |
| `/tungsten-next` | Once per release, after the merge | It syncs the roadmap page's database from the tree and `roadmap.json` with no hand edits (`D-109`); each session's last prompt already names the next one |
| Owner-only checks | As cards list them | Collected per phase: Windows and macOS hosts, a crash file from a shipped archive, playthroughs |
| Capture windows | Perf steps | Disconnecting; sessions wait for the encoder |
