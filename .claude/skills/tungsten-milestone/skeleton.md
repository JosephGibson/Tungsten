# Phase <N> milestone <NN>: <deliverable> (<candidate ID>) — draft

- **status:** draft
- **goal:** <what the candidate delivers, in one or two sentences, from its card>
- **non-goals:** <what its card or workstream leaves to other candidates; no scope creep>
- **files to touch:** <paths, grouped by step; nothing outside this list>
- **ordered steps:** <(1) … (2) …, one line each, matching §3; the last is "Release 0.<NN>">
- **done-when:** <every step's evidence row quoted; workflow §5 closed; the release checks, plus the gates by change no step ran, pass after the cut; the preflight prints the command block and the post-merge block>

Candidate <ID> ([implementation plan](implementation-plan.md) §3 card, level <A|B|C>), milestone M<NN>, release 0.<NN>. Written <date> at `<commit>`. Runs under the [tungsten-milestone skill](../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](workflow.md) §4–§6; this plan records only where it differs.

## Context digest

<Under about 500 tokens: why this candidate now, what it stands on, the decisions it cites, what later candidates expect from it.>

## 1. Audit

Read at `<commit>`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | <what the code does today that the steps change> | `<file>:<line>` |

## 2. Design

<The API sketch (level C reviews it after step 1), types and names, the seam (`D-007`, `D-018`), the template change once W12a has landed.>

## 3. Steps

### Step 1: <title>

- **Files:** <paths>
- **Change:** <what, and in what order>
- **Done-when:** `<command>` → <expected result>; …
- **Moves:** <invariants this step moves on purpose, or none>

### Step <N>: Release 0.<NN>

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README "Now" lines, this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../releases.md)): the plan archived, `just release-cut 0.<NN>.0` once, every release check once after it with <the gates by change no step ran>, then the preflight with `--message`
- **Done-when:** [releases](../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.<NN>.0 --repo JosephGibson/Tungsten --message 'Update 0.<NN>: <summary>'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: <the plan prompt for the next register row, or the gate, graduation or experiment prompt>

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | <question> | <stated default> |

<Approval line goes here: "Approved <date> with the stated defaults", plus any change.>

## 5. Decisions expected

<`D-NNN`: title; amends or supersedes `D-0NN`. Next free IDs when written; checked again when the step runs.>

## 6. Invariants

- **May move:** <a hash, digest or reference image and the step that moves it, with the new value recorded in the evidence log and the `CHANGELOG.md` line>
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions, <others>

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- <The plan's own: a perf row reads `regressed`, an open question turns out to need the owner, a file outside §files to touch needs a change, …>

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | <N>, release | <perf rows and visual in their steps; the release step runs `just check`, `just smoke`, `just script-test` and the rest of the release checks once, after the cut> |
| 3 Invariants | <N> | |
| 4 Decisions | <N> | |
| 5 API ([break ledger](w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | <N> | |
| 6 Template and guide | <N> | <n/a before W12a> |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | <N> | |
| 8 Design | <N> | |
| 9 Records (known issues, backlog, gap log, register, workstream file, README "Now") | <N> | |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

<Reviewer (the `critique` skill with `--repo`, or `codex:rescue` when the skill was missing) and its model family, the date, the commit it read and the round; or "Not run: <error line>". Each finding is checked against the repo or the docs it cites before it changes anything; none is applied unchecked. A second round's rows are numbered `2.<n>`.>

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | <[severity] claim, as the critic wrote it> | <`path:line`, doc section or command run> | <accepted or rejected> | <the plan section it changed, or why the plan stands> |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups

<Out-of-scope findings, each with its home: a known issue, a backlog row or a later candidate.>
