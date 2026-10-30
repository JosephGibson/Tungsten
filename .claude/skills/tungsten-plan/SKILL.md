---
name: tungsten-plan
description: Write or run a large Tungsten plan (multi-session or risky): the plan file from its skeleton, its critique and hand-off, its steps, commits and close. Use for a large plan's writing or run session, a phase plan's step included. Not for small or medium changes or releases.
---

# tungsten-plan

A large change gets one plan file, `docs/plans/<topic>.md`, from [skeleton.md](skeleton.md), deleted when done ([operating rules](../../../docs/operating-rules.md#plans), [plans README](../../../docs/plans/README.md)). A phase plan runs a step at a time: each step gets a plan of its own.

## Writing

- Each step names its files and ends in a commit; its done-when are commands with their expected results, a local command's as its exit code, perf checks as [tungsten-perf](../tungsten-perf/SKILL.md) writes them. A quick or functional sitting is marked functional only, never evidence (`D-226`).
- **Critique last, before the commit.** Read the 5-hour usage window first (`codex app-server`, no model call): a `--repo` round costs a large share of it. Then, from the repo root, in the background: `critique --raw --repo --out <scratchpad file> docs/plans/<topic>.md`, adding `--force` only when the size guard stops it. Never review the plan yourself in the critic's place: if Codex fails, record the critique as not run with its error line.
- Check each finding against the repo or the doc it cites before changing anything, and record it in §7: the finding, what it was checked against, the verdict, the change or why the plan stands.
- A second round, at xhigh, only when the first changed a mechanism (a done-when check, the step order, a step added or removed). It reads the plan without §7 and the log, `--from "sed '/^## 7\./,/^## Fol/{/^## Fol/!d}' <plan>"`, with a `--focus-file` naming round 1's changes and rejections. No third round: a finding still open becomes an open question with a default.
- Commit the plan alone (`docs(plans): …`) with its item moved into Now, push the version branch, and end with tungsten-next's `run` prompt ([prompts](../tungsten-next/prompts.md)) under its label line.

## Running

- Set `status: in progress` in the first step's commit, so `just status` reads the run. Run the steps in order, each a Conventional commit of its own paths, pushed (`git push origin 0.NNN`, the whole command).
- A failed check, a stop condition or a file outside the step's list stops the step. Restore each changed file with `git show HEAD:<path> > <path>`; `git restore`, `checkout`, `stash`, `reset` and `rm` are denied. Delete each file the step added (`python3 -c "import os; os.remove('<path>')"`, then `git add <path>` when it was tracked). Mark the step skipped with its reason in the evidence log, commit and push that alone, carry on with the steps that don't depend on it, name the ones that do, and report it at the end.
- Evidence: one row per step, each done-when check's output quoted and a check that did not run marked not run, so the close can be rechecked.

## Closing

Done means the tests pass and a demo works, quoted in the last evidence row. Move open follow-ups to known issues or Later, delete the plan with its citations in the last commit (`just repo-check` fails on a cited plan that is gone), take the item out of Now, and end with the commits, the evidence, what the owner should try and the next prompt.
