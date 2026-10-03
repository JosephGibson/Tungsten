---
name: tungsten-decision
description: Add, amend or supersede a Tungsten decision in DECISIONS.md — next D-NNN ID, entry shape, the marker line on the old entry and the docs/DECISION_INDEX.md row the same change needs. Use when a change records or reverses a design choice or adds a dependency. Not for looking a decision up.
---

# tungsten-decision

Writes a decision; [AGENTS.md](../../../AGENTS.md) owns the rules. Lookups: [the index](../../../docs/DECISION_INDEX.md), then one section of [DECISIONS.md](../../../DECISIONS.md) by ID; never read it whole.

```bash
rg -oN '^## D-(\d+)' -r '$1' DECISIONS.md | sort -n | tail -1 | awk '{printf "D-%03d\n", $1 + 1}'  # next ID
rg -n '^## D-' DECISIONS.md | rg -A1 ':## D-0NN '  # one entry's line range
```

IDs are never reused. Mirror the newest entry and append yours at the end of the file:

```markdown
## D-NNN — Title stating the choice
**Date:** YYYY-MM-DD
**Decision:** The choice, naming what a reader will search for.

**Why:** The rationale, with the measurement behind it.

**Consequences:**
- What it amends and what stands; tests that pin it; accepted regressions; open limits.
```

All four fields are standard from `D-049` on. A new third-party runtime dependency gets an entry whose Decision says "Satisfies D-015 rule N".

## Amend or supersede

Settled entries are immutable: the old body stays. In the same change the old entry takes a marker line directly under its heading, above `**Date:**`, several in ID order:

```markdown
**Amended by D-NNN:** the <clause> only; <what holds now>; <the rest> stand.
**Superseded by D-NNN:** the <clause> only; <the rest> stand.
```

`Amended` narrows or extends a clause, `Superseded` reverses one. The new entry's Consequences says "Amends `D-0XX` as its marker line says". `**Status:**` on `D-071` and `D-074` is an older form; don't copy it.

## Index row

Add `` | `D-NNN` | Takeaway. | `` at the end of its index section (physics sits under ECS / Runtime Flow). An amendment touches both rows: the new one ends ``Amends `D-0XX`.``, the old one gains ``Amended by `D-NNN`: …``.

`just repo-check` fails when a heading ID is missing from the index or a doc it checks cites an ID with no heading. Any mention in the index satisfies it, so list heading, markers and rows yourself:

```bash
rg -no '.{0,60}D-NNN.{0,60}' DECISIONS.md docs/DECISION_INDEX.md
```
