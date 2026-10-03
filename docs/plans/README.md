# Session Plans

Short-lived multi-step plans saved as `*.md`. A plan is the handoff artifact for a fresh assistant context, instead of long chat history; a typical restart prompt is `read docs/plans/… and implement; stay in scope`. For multi-step work, write the agreed plan here rather than leaving it only in chat.

## Naming

- Milestone implementation plans: `phaseN-milestone-NN-short-topic.md`. `N` is the phase number, `NN` the zero-padded milestone number, `short-topic` a concise kebab-case slug for the deliverable. Example: `phase4-milestone-26-materials-post-stack.md`.
- Other handoff plans: `descriptive-topic.md`.
- Program folders: plans that serve one goal may share `docs/plans/<program>/`, with a README that maps its files, as [`1.0/`](1.0/README.md) does. Header rules apply to every file there except the README. `just repo-check` reads only this folder's top level so far.

## Contents

- Required header fields: `status` (`draft` / `in progress` / `done` / `abandoned` / `superseded`), goal, non-goals, files to touch, ordered steps, done-when checks.
- Put a short context digest (under ~500 tokens) near the top instead of a separate context file.
- Settled rationale belongs in `DECISIONS.md`; plans are time-bounded execution documents.

## Executing a plan

- Read the whole plan first and treat it as the map; don't re-explore. Open only the files the step touches ([`docs/LLM_INDEX.md`](../LLM_INDEX.md) routes by task).
- Do exactly the requested step or steps. Record out-of-scope findings as follow-ups in the plan or in [`docs/known-issues.md`](../known-issues.md); don't fix them.
- Run the step's done-when checks and quote the results. Report a failed check with its output instead of working around it, and a check that did not run as not run.
- Where the plan defers a choice, use its stated default; otherwise check [`docs/DECISION_INDEX.md`](../DECISION_INDEX.md), then ask. The "Stuck" rule in [AGENTS.md](../../AGENTS.md) applies.
- Keep `status` current: set `in progress` when work starts and `done` when the last done-when check passes, then archive per Lifecycle (`just repo-check` flags a finished plan left here).
- When Git mutations are human-only, hand over one patch and commit message per step. Use the [tungsten-patch-handoff](../../.claude/skills/tungsten-patch-handoff/SKILL.md) skill.
- For perf plans, write the done-when checks per the profiling workflow's [Writing done-when checks](../perf/profiling-workflow.md#writing-done-when-checks).

## Lifecycle

- Update `status` when work finishes; don't leave a finished plan `in progress`.
- Keep one active plan per thread of work: archive or rename obsolete ones.
- Completed, abandoned or superseded plans move to `docs/plans/archive/` with the same basename; a program folder's files move to `docs/plans/archive/<program>/`. Agents never read that directory.

`just repo-check` validates active headers and flags completed plans left here. An old date alone does not make an in-progress plan stale: review its remaining checks against code and owner/platform dependencies before archiving. An audit can be complete with unresolved findings; retain still-relevant findings in a maintained follow-up document before moving it.

Move only known filenames, fix relative Markdown destinations for the new depth, and update incoming links in maintained docs. Never inspect the archive to discover files or confirm a move. Owner-directed retirement can preserve an unfinished plan as a historical snapshot; archived proposals do not authorize new work. Leave immutable decision text and dated release evidence intact, with historical-path guidance in the indexes.

Phase 4 is complete; its roadmap is archived as `docs/plans/archive/phase4.md`.
