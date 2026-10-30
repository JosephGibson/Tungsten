---
name: tungsten-decision
description: Add, amend or supersede a Tungsten decision — a DECISIONS.md entry, the old entry's marker line and the docs/DECISION_INDEX.md rows, in one change. Use when a change records or reverses a design choice or adds a dependency. Not for looking a decision up.
---

# tungsten-decision

[AGENTS.md](../../../AGENTS.md) owns the rules, and the newest entries, their marker lines and their index rows are the pattern. One thing they don't show:

- A new third-party runtime dependency's entry says "Satisfies `D-015` rule N" in its **Decision** field, not only under Why: the rule is what admits the dependency, so it is part of what was decided.
