---
name: tungsten-milestone
description: Write or run a large Tungsten plan (multi-session or risky) under the operating rules: the skeleton, the session order and the execution rules a plan cites. Use for a large plan's writing or run session. Not for small or medium changes or releases.
---

# tungsten-milestone

A large change (multi-session, or risky: concurrency, formats, the render seam) gets one plan file, `docs/plans/<topic>.md`, with the [plans README](../../../docs/plans/README.md#contents)'s headers (status, goal, non-goals, files to touch, ordered steps, done-when), deleted when done ([operating rules](../../../docs/operating-rules.md#plans)). Small and medium changes have no plan file. This skill holds what every plan would otherwise repeat; a plan cites it and writes down only where it differs.

## Read first

- **Plan session:** the item's line in [ROADMAP.md](../../../ROADMAP.md) and the sources it cites, the [LLM index](../../../docs/LLM_INDEX.md) rows for the code it touches, and each cited decision's [index](../../../docs/DECISION_INDEX.md) row, then that one section.
- **Execution session:** the whole plan, then only the files the step names. The [plan rules](../../../docs/plans/README.md#executing-a-plan) apply.

## Writing the plan

1. **Audit first.** Map the structure before any full read (`ast-grep`, `rg -l`, the index). Record each finding as `file:line` with the commit it was read at. Read a file whole only to operate on it.
2. **File.** Copy [skeleton.md](skeleton.md) to `docs/plans/<topic>.md`, a kebab-case slug for the deliverable.
3. **Steps.** Each step fits in one session, names its files and ends in a commit. Write its done-when as commands with their expected results. Write perf checks to the [done-when rules](../../../docs/perf/profiling-workflow.md#writing-done-when-checks) and pick rows with [tungsten-perf](../tungsten-perf/SKILL.md). A change to shared ECS or frame-loop code captures every CPU row.
4. **The rest of the skeleton.** Open questions, each with a default. Invariants the plan may move and those it must not. Stop conditions. An empty evidence log. A decision only for a choice that constrains later work (a dependency, a format, a policy), from [tungsten-decision](../tungsten-decision/SKILL.md).
5. **Critique, last.** Once the plan is complete, run the user-level `critique` skill on it with `--repo`, in the background: `gpt-6-astra` at the skill's defaults (effort xhigh, up to ten findings). Confirm `--force` when the plan passes the skill's soft size limit. If the skill is missing, ask `codex:rescue` for the same review, read-only. If Codex fails, record the critique as not run with its error line and say so with the run prompt; never review the plan yourself in its place. Check each finding against the repo or the docs it cites before changing anything, and record it in the skeleton's Critique section. One round; a second, at xhigh, only when the first changed a mechanism (a done-when check, the step order, a step added or removed). Round 2 drops the Critique section and the log (`--from "sed '/^## 7\./,/^## Fol/{/^## Fol/!d}' <plan>"`) and passes a `--focus-file` of round 1's changes and rejections. There is no third: a finding still open becomes an open question with a default.
6. **Commit and hand over.** Commit the plan alone (`docs(plans): …`), with the item moved into Now in ROADMAP.md if it isn't there. End with the run prompt for a new session (the `run` row of [tungsten-next's prompts](../tungsten-next/prompts.md)). Nothing waits for the owner: open questions take their stated defaults.

## Running steps

- Set `status: in progress` in the first step's commit. Run every step in order, under the plan's stop conditions.
- Commit at each step's end in Conventional Commits, staging only the paths the step changed (`git add <paths>`, never `-A` or `.`); each commit builds and passes the checks its change needs ([validation](../../../docs/validation.md)). Never revert another session's changes; keep scratch files out of the tree. Push the version branch after each commit (`git push origin <branch>`, the whole command); a refused push is reported, never forced. The owner tags and merges.
- On a failed check or a stop condition, restore the step's files from the last commit (`git show HEAD:<path> > <path>` per file, since `git restore` stays denied to agents, and delete files the step added), mark the step skipped with its reason, and carry on with steps that don't depend on it.
- Captures follow the [profiling workflow](../../../docs/perf/profiling-workflow.md#comparison-rule-and-capture-rules). Never write to the tree while `pgrep -af scripts/bench.py` shows a capture; run each sitting as one blocking foreground command.
- Correct maintained docs in the same commit as the change that makes them wrong (`DESIGN.md`, the LLM index, `README.md`, assets, validation, known issues, the template's docs). Give every new public item rustdoc; when the public surface moves, run `just api` and commit the snapshot diff.
- Evidence: one row per step with its verdict, key numbers and paths. Quote each done-when check's output, and record a check that did not run as not run. Record out-of-scope findings as follow-ups.

## Closing

1. **Done** means the tests pass and a demo works: run the checks the change needs ([validation matrix](../../../docs/validation.md#change-to-check-matrix)) and the demo, and quote both in the last evidence row.
2. **Delete the plan** in the last commit, with its citations removed (`just repo-check` fails on a cited plan that is gone) and the item out of Now in ROADMAP.md. Move open follow-ups first: to [known issues](../../../docs/known-issues.md), or to Later as one line each.
3. **No release, changelog line or decision entry per plan:** a release is cut separately, when something is worth shipping.
4. **End with** the commits made, the evidence quoted, then what the owner should play or try and the next prompt ([tungsten-next's prompts](../tungsten-next/prompts.md)).
