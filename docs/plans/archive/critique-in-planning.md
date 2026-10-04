# Critique in milestone planning

- **status:** done
- **goal:** Make the antagonistic critique the explicit last planning step of a 1.0 milestone plan, after the plan is complete and before its approval line: one required round with the user-level `critique` skill by default, a recorded verdict per finding, and a fallback when the skill is missing. The first real use is W14a's plan session.
- **non-goals:** Editing `~/.claude/skills/critique` or naming its model in repo docs. A `just repo-check` rule, an `AGENTS.md` line or a decision entry. Critiquing the w01–w16 workstream files. Rewriting past critique records (criteria.md, the roadmap rework §12, w11). A `CHANGELOG.md` line, or listing rejected findings in step 8's closing message (both declined 2026-10-04). QA plans (the `qa-plan` prompt, workflow §2's QA pass row): see Follow-ups.
- **files to touch:** `.claude/skills/tungsten-milestone/SKILL.md` (step 7; `.agents/skills/tungsten-milestone` is a symlink to its folder); `.claude/skills/tungsten-milestone/skeleton.md` (§9); `docs/plans/1.0/workflow.md` (§2: the Plan row's Produces cell and one sentence of "Plan sessions"); `.claude/skills/tungsten-next/prompts.md` (the `plan` row); this plan, moved to `docs/plans/archive/` at the close.
- **ordered steps:** (1) scratch copies; (2) SKILL.md step 7; (3) skeleton §9; (4) workflow.md; (5) prompts.md; (6) checks; (7) close. Details in §3.
- **done-when:** every check in §4 gives its expected result.

Drafted 2026-10-04 on branch `0.42` (`HEAD` `db1177c`, 0.42 cut, not released). The baseline is the dirty tree; nothing outside the files above changes.

## Context digest

- Skill step 7 now reads "Before approval, get an antagonistic review from another model family (for example the user-level `critique` skill), then record what it changed"; workflow.md:41 and :48 say the same. Skeleton §9 is one placeholder line. No decision backs the plan-session shape; `D-002` prefers local judgment over formal process; tungsten-decision applies to a design choice reversed or a dependency added, and this is neither. So there is no D-NNN.
- Three critique records exist (criteria.md:361 and :363, roadmap rework §12, w11:41). The w11 line shows a finding checked against the std docs and rejected, so the record of each rejection is the safeguard. The owner sees that record at approval.
- The `critique` skill (outside the repo) stops above 10,000 characters until `--force` is confirmed. Milestone plans run 15–50k. It reads only the artifact unless `--repo` is given, and on any Codex failure, a reached usage limit included, it reports and stops. `codex:rescue` uses the same Codex runtime and plan limits, so it only stands in when the skill itself is missing.
- `just ctx` caps a SKILL.md body at 8,192 B; tungsten-milestone's file is 6,650 B, so step 7 stays short and the record format lives in skeleton §9.
- W14a's plan needs branch `0.43` (skill step 1), which the 0.42 release creates. No `/critique` runs in this session.
- The roadmap rework plan (in progress) still has workflow.md §2 edits pending in its step 8, in the Gate and Graduation rows; this plan touches only the Plan row and one paragraph. Its step 3 check needs `audit only the code it touches` to stay in `prompts.md`; step 5 keeps it.

## 1. Answers (2026-10-04)

| # | Question | Answer |
| --- | --- | --- |
| A1 | First real use | W14a's plan session on `0.43`; nothing runs here |
| A2 | Fallback | `codex:rescue`, read-only, only when the skill is missing; a Codex failure is recorded as not run and named with the approval prompt; never a self-review |
| A3 | Critic mode | `--repo` in the background; the `plan` prompt confirms `--force` for the plan file |
| A4 | Rounds | One required; a second only when the first changed a done-when check, the step order, or added or removed a step; no third, and a finding still open becomes an open question with a default |

## 2. Wording

**SKILL.md step 7** replaces the current line:

> 7. **Critique, last.** Once the plan is complete, and before step 8 and the approval line, run the user-level `critique` skill on it with `--repo`, in the background; the plan prompt confirms `--force` for the plan file. If the skill is missing, ask `codex:rescue` for the same review, read-only. If Codex fails, record the critique as not run with its error line and say so with the approval prompt; never review the plan yourself in its place. Check each finding against the repo or the docs it cites before changing anything, and record it in skeleton §9. One round is required; a second runs on the revised plan only when the first changed a done-when check or the step order, or added or removed a step. There is no third: a finding still open becomes an open question with a default.

**Skeleton §9** replaces its placeholder:

> <Reviewer (the `critique` skill with `--repo`, or `codex:rescue` when the skill was missing) and its model family, the date, the commit it read and the round; or "Not run: <error line>". Each finding is checked against the repo or the docs it cites before it changes anything; none is applied unchecked. A second round's rows are numbered `2.<n>`.>
>
> | # | Finding | Checked against | Verdict | Change or reason |
> | --- | --- | --- | --- | --- |
> | 1 | <[severity] claim, as the critic wrote it> | <`path:line`, doc section or command run> | <accepted or rejected> | <the plan section it changed, or why the plan stands> |

**workflow.md §2**, Plan row, Produces cell: "One milestone plan, critiqued last and revised, the critique recorded in it; then the approval prompt, and once approved, the run prompt".

**workflow.md "Plan sessions"**, replacing the sentence that starts "Before approval, the plan goes to a reviewer":

> The last planning step, once the plan is complete and before approval, is an antagonistic critique from another model family, by default the user-level `critique` skill, as criteria.md had twice ([tungsten-milestone](../../../.claude/skills/tungsten-milestone/SKILL.md) step 7): one round, a second only when the first changed the steps or a done-when check. The plan records each finding with what it was checked against, its verdict, and the change or the reason it was rejected.

**prompts.md `plan` row**: insert `critique it last as the skill's step 7 says (--force is confirmed for the plan file); ` before `stop for my approval`. The rest of the template, its `Don't commit` line included, stays as it is.

## 3. Steps

1. **Copies.** Copy the four files to the scratchpad, plus `git status --porcelain` as `status-before.txt`. Done when `cmp` confirms each copy.
2. **Skill.** Replace step 7 with §2's text. Done when `just ctx` passes.
3. **Skeleton.** Replace §9's placeholder with §2's header line and table.
4. **Workflow.** The Plan row's Produces cell and the "Plan sessions" sentence, per §2. No other line of the file changes.
5. **Prompt.** The `plan` row's insertion, per §2. Done when `just ctx` passes.
6. **Checks.** Every §4 check not marked "after step 7", quoted with its output.
7. **Close.** `status: done`, then move this file to `docs/plans/archive/critique-in-planning.md`: the one write into the archive, with nothing there read. Then `just repo-check`. End with the changed-file list. No commit (`D-105`).

## 4. Done-when checks

| Check | Expected |
| --- | --- |
| `readlink .agents/skills/tungsten-milestone` | `../../.claude/skills/tungsten-milestone` |
| `rg -n -F 'for example the user-level' .claude docs/plans/1.0/workflow.md` | no match (exit 1) |
| `rg -n -F 'codex:rescue' .claude/skills/tungsten-milestone/SKILL.md` | one line, step 7 |
| `rg -n 'Checked against.+Verdict.+Change or reason' .claude/skills/tungsten-milestone/skeleton.md` | one line, under `## 9. Critique` (no match before step 3) |
| `rg -n -F 'step 7' docs/plans/1.0/workflow.md` | one line, :48 (no match before step 4) |
| `rg -n -F -- '--force is confirmed' .claude/skills/tungsten-next/prompts.md` | one line, the `plan` row |
| `rg -c 'audit only the code it touches' .claude/skills/tungsten-next/prompts.md` | 1, as before, so the rework plan's step 3 check still holds |
| `diff -u <copy> <file>` for each of the four files | only the hunks §2 names |
| `just ctx` | exit 0 |
| After step 7: `git status --porcelain` against `status-before.txt` | the same paths, this plan's only under the archive |
| After step 7: `just repo-check` | exit 0, "0 error(s)" |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-04 | pass | `cmp` silent for all four copies; `status-before.txt` taken with `rtk proxy git status --porcelain` (plain `git status` through the rtk hook wrote an empty file) | scratchpad `critique-plan/` |
| 2–5 | 2026-10-04 | pass | `diff -U0` against the copies: SKILL.md `@@ -23 +23`, skeleton.md `@@ -83 +83,5`, workflow.md `@@ -41` and `@@ -48`, prompts.md `@@ -16`, nothing else; SKILL.md body 6,985 B of 8,192; `just ctx` "Agent context checks: OK", self-test OK, exit 0 after steps 2 and 5 | the four files |
| 6 | 2026-10-04 | pass | `readlink` `../../.claude/skills/tungsten-milestone`; old wording exit 1; `codex:rescue` SKILL.md:23 only; table skeleton.md:85 under `## 9. Critique` (:81); `step 7` workflow.md:48 only; `--force is confirmed` prompts.md:16 only; `audit only the code it touches` count 1 in prompts.md | — |

## Follow-ups

- QA plans (`qa-plan` in prompts.md, workflow §2's QA pass row) go through approval without a critique step. Decide when the Phase 5 QA pass is planned.
