# <Deliverable> (<ROADMAP.md item ID>) — draft

- **status:** draft
- **goal:** <what the item delivers, in one or two sentences, from its source>
- **non-goals:** <what it leaves to other items; name scope creep here instead of absorbing it>
- **files to touch:** <paths, grouped by step; nothing outside this list>
- **ordered steps:** <(1) … (2) …, one line each, matching §3>
- **done-when:** <every step's done-when passes, quoted in the evidence log; the tests pass and <the demo> works; the plan deleted with its citations and the item out of Now>

Item `<id>` in [ROADMAP.md](../../ROADMAP.md), from <source>. Written <date> at `<commit>`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

<Under about 500 tokens: why this item now, what it stands on, the decisions it cites, what later items expect from it.>

## 1. Audit

Read at `<commit>`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | <what the code does today that the steps change> | `<file>:<line>` |

## 2. Design

<Types and names, the seam (`D-007`, `D-018`), data formats and their migration, the template change if any.>

## 3. Steps

### Step 1: <title>

- **Files:** <paths>
- **Change:** <what, and in what order>
- **Done-when:** `<command>` → <expected result>; …
- **Commit:** `<type>(<scope>): <summary>`
- **Moves:** <invariants this step moves on purpose, or none>

### Step <N>: Close

- **Files:** this plan (deleted), the files that cite it, `ROADMAP.md`, <follow-up homes>
- **Change:** run the checks the change needs and the demo; move open follow-ups to known issues or Later; delete this plan and its citations; take the item out of Now
- **Done-when:** <the validation matrix's checks for the change> → pass; <the demo> → works; `just repo-check` → passes
- **Ends with:** the commits made, the evidence quoted, what the owner should play or try, and the next prompt

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | <question> | <stated default> |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** <a hash, digest or reference image and the step that moves it, with the new value recorded in the evidence log>
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions, <others>

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it.
- <The plan's own: a perf row reads `regressed`, an open question turns out to need the owner, a file outside §files to touch needs a change, …>

## 7. Critique

<Reviewer (the `critique` skill with `--repo`, `gpt-6-astra` by default, or `codex:rescue` when the skill was missing), the date, the commit it read, the round, and the skill's footer line; or "Not run: <error line>". Each finding is checked against the repo or the docs it cites before it changes anything; none is applied unchecked. A second round's rows are numbered `2.<n>`.>

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | <[severity] claim, as the critic wrote it> | <`path:line`, doc section or command run> | <accepted or rejected> | <the plan section it changed, or why the plan stands> |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups

<Out-of-scope findings, each with its home: a known issue or a line in ROADMAP.md's Later.>
