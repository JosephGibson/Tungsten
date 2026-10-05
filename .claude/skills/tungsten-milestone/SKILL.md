---
name: tungsten-milestone
description: Write or run a Tungsten 1.0 milestone plan: the plan skeleton, the plan session's order and the execution rules every milestone plan cites instead of copying. Use for a 1.0 candidate's plan or execution session. Not for gates, spikes or releases.
---

# tungsten-milestone

On the road to 1.0, each candidate, or a pair of small neighbours (`D-108`), gets one milestone plan, and the plan's last step is its `0.NN` release ([workflow](../../../docs/plans/1.0/workflow.md) §1). This skill holds what every plan would otherwise repeat. A plan cites it in its header and writes down only where it differs.

## Read first

- **Plan session:** the candidate's row and card in the [implementation plan](../../../docs/plans/1.0/implementation-plan.md) (§3–§5), its workstream file (or its criteria section if it has not graduated), workflow §2 and §4–§6, the [LLM index](../../../docs/LLM_INDEX.md) rows for the code it touches, and each cited decision's [index](../../../docs/DECISION_INDEX.md) row, then that one section.
- **Execution session:** the whole plan, then only the files the step names. The [plan rules](../../../docs/plans/README.md#executing-a-plan) apply.

## Writing the plan

1. **Branch.** `git status -sb` shows `0.NN...origin/0.NN`, which the previous release's post-merge block created ([releases](../../../docs/releases.md#handoff-and-next-branch): `--no-track`, then `push -u`). Otherwise give the owner that block and stop.
2. **Number.** The next milestone after the last one in the register (§10; the first is M32). Copy [skeleton.md](skeleton.md) to `phase<N>-milestone-<NN>-<slug>.md` in `docs/plans/1.0/`. A pair takes one number and one plan, its candidates' steps one after the other.
3. **Audit first.** Map the structure before any full read (`ast-grep`, `rg -l`, the index). Record each finding as `file:line` with the commit it was read at. Read a file whole only to operate on it.
4. **Steps.** Each step fits in one session and names its files; the last is the release, as the skeleton has it. Write its done-when as commands with their expected results. Write perf checks to the [done-when rules](../../../docs/perf/profiling-workflow.md#writing-done-when-checks) and pick rows with [tungsten-perf](../tungsten-perf/SKILL.md). A change to shared ECS or frame-loop code captures every CPU row.
5. **The rest of the skeleton.** Open questions, each with a default. Expected decision IDs, the next free ones from [tungsten-decision](../tungsten-decision/SKILL.md). The invariants the plan may move and those it must not. Stop conditions. An empty evidence log.
6. **What the milestone owes.** Map each of workflow §5's ten lines to the step that delivers it, or mark it n/a with a reason. A line with no home becomes a step.
7. **Critique, last.** Once the plan is complete, and before step 8 and the approval line, run the user-level `critique` skill on it with `--repo`, in the background; the plan prompt confirms `--force` for the plan file. Add `--effort max` when the stop's `plan` stage recommends Fable 5.1 (`D-118`'s top tier: the `rec` in the payload `python3 -B scripts/roadmap.py catalog <scratchpad>/catalog.json` writes, under the stop whose `row` is the candidate); otherwise keep the skill's default, xhigh. If the skill is missing, ask `codex:rescue` for the same review, read-only. If Codex fails, record the critique as not run with its error line and say so with the approval prompt; never review the plan yourself in its place. Check each finding against the repo or the docs it cites before changing anything, and record it in skeleton §9. One round is required; a second runs on the revised plan, at the first round's effort, only when the first changed a done-when check or the step order, or added or removed a step. There is no third: a finding still open becomes an open question with a default.
8. **Records and stop.** Fill the register row's plan column and the [1.0 README](../../../docs/plans/1.0/README.md) "Now" lines. Then stop, ending with the approval prompt for the owner to paste (the Approve row of [tungsten-next's prompts](../tungsten-next/prompts.md)). When it comes back, add "Approved <date> with the stated defaults", plus any changes, under the open questions, and end with the run prompt for the plan's level, for a new session.

## Running steps

- Start only after the approval line. Set `status: in progress` at the first step.
- Run one step per session by default. Levels A and B (workflow §4) may run several steps unattended under the plan's stop conditions. Level C stops after step 1 for the API review (the API sketch and the template diff) and runs the rest after it.
- Before each step, copy every file it names to the scratchpad. On a failed check or a stop condition, restore them with plain `cp` and confirm with `cmp`, mark the step skipped with its reason, and carry on with steps that don't depend on it. Use `git show HEAD:<path>` only for a file the milestone had not yet changed: `HEAD` is the previous release.
- No commits (`D-105`). Edit only the paths the plan names, never revert another session's changes, keep scratch files out of the tree, and end with the list of files changed.
- Captures follow the [profiling workflow](../../../docs/perf/profiling-workflow.md#comparison-rule-and-capture-rules) and workflow §6. Never write to the tree while `pgrep -af scripts/bench.py` shows a capture. Wait for the remote-desktop encoder to exit, and run each sitting as one blocking foreground command.
- Write decisions with tungsten-decision, with their index rows, before the docs that cite them.
- Public API: add a [break ledger](../../../docs/plans/1.0/w04-api-freeze.md#break-ledger) row for each break, give every new public item rustdoc, and run `just api` to regenerate the `api/` snapshots before the release, and review their diff (`D-107`; the release runs `just api-check`).
- Evidence: one row per step with its verdict, key numbers and paths. Quote each done-when check's output, and record a check that did not run as not run. Record out-of-scope findings as follow-ups in the plan or in [known issues](../../../docs/known-issues.md).

## Closing

The last step is the release, run in the same session after the other steps, so the cut happens once and the checks run once, after it (workflow §1).

1. **Close workflow §5:** invariants checked, `DESIGN.md` updated for architecture changes, records current (the register row's release and status, the README "Now" lines), one `CHANGELOG.md` line, this step's evidence row and `status: done`.
2. **Release:** [tungsten-finalize](../tungsten-finalize/SKILL.md), then [tungsten-release](../tungsten-release/SKILL.md) ([releases](../../../docs/releases.md)): the plan archived, the cut, every release check once after the cut with the gates the change needs that no step ran ([AGENTS.md](../../../AGENTS.md#tests) table), then the read-only preflight with `--message`. The plan is archived before the cut, so the checks' output goes in the report, not its evidence log.
3. **End with** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for the next register row (naming both candidates of a pair), or the gate, graduation or experiment prompt the register puts next ([tungsten-next's prompts](../tungsten-next/prompts.md)).

The block's commit takes the whole tree (`D-105`). A stop condition that ends the run before this step leaves the release to a release session, after the owner has seen why (workflow §3).
