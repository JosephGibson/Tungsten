# Session Plans

Short-lived multi-step plans saved as `*.md`. A plan is the handoff artifact for a fresh assistant context, instead of long chat history; a typical restart prompt is `read docs/plans/… and implement; stay in scope`. For work spanning sessions, write the agreed plan here rather than leaving it only in chat.

## Choosing a route

`D-140` scales planning to the change:

- Bounded maintenance (a local bug fix, documentation correction or tooling repair) records scope/non-goals, affected files and acceptance commands in the session before editing. Use the [validation matrix](../validation.md) to select checks. A saved milestone plan or another approval round is unnecessary within the owner's authorized scope.
- Multi-session or risky work keeps one saved plan with the headers below, ordered steps, stop conditions and evidence, so a fresh context can resume it, and one critique round at xhigh (a second only when the first changed a mechanism). Done means the tests pass and a demo works; the plan is deleted when done, with its citations. Promote maintenance to this route when its scope grows or a handoff is needed.
- The 1.0 milestone route (the milestone skill's plan, its critique rounds and level C) is retired (`D-215`); the [operating rules](../operating-rules.md#plans) own the plan sizes, and nothing stops for the owner's approval (`D-150`).

Audit scope is separate: a crate correctness audit reads the full named crate surface; a scoped workflow/documentation audit reads its named documents/tools and reports within that scope. Both report findings unless fixes were requested.

Capture provenance and hardware-check rules stand (`D-070`, `D-095`). Every route commits at each logical step and pushes the version branch ([operating rules](../operating-rules.md#git), `D-215`, `D-216`); no route cuts a release unless the owner requested one.

## Naming

- Plans: `descriptive-topic.md`. Milestone numbers ended at M85 (`D-215`); the archive keeps the `phaseN-milestone-NN-short-topic.md` names.
- Program folders: plans that serve one goal may share `docs/plans/<program>/`, with a README that maps its files, as [`1.0/`](1.0/README.md) does. Header rules apply to every file there except the README, and `just repo-check` reads them one folder deep.

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
- Commit at each logical step in Conventional Commits, staging only the paths you changed (never `git add -A`), and push the version branch after each commit; tags, merges and every other push stay with the human ([operating rules](../operating-rules.md#git)). Keep scratch files out of the tree.
- Keep the evidence log terse: one row per step with its verdict, key numbers and capture or log paths. The release's changelog section is written from the commits at the cut.
- A plan's perf checks are its counter guards, which `just check` runs; it takes a full sitting only when it claims a speed-up, measures a 7.14, 7.15 or 9.2 item or changes a row's workload (`D-226`). Write those checks per the profiling workflow's [Writing done-when checks](../perf/profiling-workflow.md#writing-done-when-checks).

## Lifecycle

- Update `status` when work finishes; don't leave a finished plan `in progress`.
- Keep one active plan per thread of work: archive or rename obsolete ones.
- Completed, abandoned or superseded plans move to `docs/plans/archive/` with the same basename; a program folder's files move to `docs/plans/archive/<program>/`. Agents never read that directory.

`just repo-check` validates active headers and flags completed plans left here. An old date alone does not make an in-progress plan stale: review its remaining checks against code and owner/platform dependencies before archiving. An audit can be complete with unresolved findings; retain still-relevant findings in a maintained follow-up document before moving it.

Move only known filenames, fix relative Markdown destinations for the new depth, and update incoming links in maintained docs. Never inspect the archive to discover files or confirm a move. Owner-directed retirement can preserve an unfinished plan as a historical snapshot; archived proposals do not authorize new work. Leave immutable decision text and dated release evidence intact, with historical-path guidance in the indexes.

Phase 4 is complete; its roadmap is archived as `docs/plans/archive/phase4.md`.
