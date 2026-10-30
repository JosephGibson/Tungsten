# <Deliverable> (<ROADMAP.md item ID>) — draft

- **status:** draft
- **goal:** <what the item delivers, from its source>
- **non-goals:** <what it leaves to other items; scope creep named here>
- **files to touch:** <paths, grouped by step; nothing outside this list>
- **ordered steps:** <(1) … (2) …, matching §3>
- **done-when:** <every step's done-when passes, quoted in the evidence log; the tests pass and <the demo> works; the plan deleted with its citations and the item out of Now>

Item `<id>` in [ROADMAP.md](../../ROADMAP.md), from <source>. Written <date> at `<commit>`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

<Under about 500 tokens: why now, what it stands on, the decisions it cites, what later items expect.>

## 1. Audit

Read at `<commit>`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | <what the code does today that the steps change> | `<file>:<line>` |

## 2. Design

## 3. Steps

### Step 1: <title>

- **Files:** <paths>
- **Change:** <what, in what order>
- **Done-when:** `<command>` → <expected result>; …
- **Commit:** `<type>(<scope>): <summary>`
- **Moves:** <invariants this step moves on purpose, or none>

### Step <N>: Close

- **Files:** this plan (deleted), the files that cite it, `ROADMAP.md`, <follow-up homes>
- **Change:** the checks and the demo; follow-ups to known issues or Later; the plan and its citations deleted; the item out of Now
- **Done-when:** <the validation matrix's checks> → pass; <the demo> → works; `just repo-check` → passes
- **Ends with:** the commits, the evidence, what the owner should try, the next prompt

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | <question> | <stated default> |

## 5. Invariants

- **May move:** <what, by which step, its new value in the evidence log>
- **Must not move:** <hashes, digests, reference images, …>

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it.

## 7. Critique

<Reviewer, date, the commit it read, the round and the skill's footer line; or "Not run: <error line>". A second round's rows are numbered `2.<n>`.>

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups
