# tungsten-next prompts

The prompt shapes [SKILL.md](SKILL.md) fills under the [operating rules](../../../docs/operating-rules.md). Fill every angle bracket; a reply never shows one. `<id>` is the item's ID in [ROADMAP.md](../../../ROADMAP.md) and `<source>` what its line cites (a feedback section, a redo step). A session commits at each logical step in Conventional Commits, staging only its own paths, and pushes the version branch after each commit; the prompts say so once, by naming the rules. No session stops for the owner's approval: open questions take their stated defaults.

| Key | Step | Where | Prompt |
| --- | --- | --- | --- |
| `example` | Example stop (medium; large when it spans sessions) | New | `Do <id> from ROADMAP.md: read <source> and docs/operating-rules.md. State its scope, non-goals, files and checks before editing; name any scope creep and propose the smallest version. Move <id> into Now in your first commit and out in your last. Done when the tests pass and example 01 runs; end with what I should play and the next prompt.` |
| `quick-fix` | Quick fix (small) | New | `Fix <what>: <where it shows>. Small route under docs/operating-rules.md: no plan, one fix(<scope>) commit staging only the paths you changed.` |
| `plan` | Large plan | New | `Use the tungsten-milestone skill. Write the plan for <id> from ROADMAP.md: read <source>; audit only the code it touches; name it docs/plans/<topic>.md; critique it last as the skill says; commit it and end with the run prompt.` |
| `run` | Run a large plan | New | `Use the tungsten-milestone skill. Read docs/plans/<topic>.md and run every remaining step in order, a commit per step, stop conditions as written; close it as the skill says and end with the next prompt.` |
| `resume` | Resume after a crash or a closed session | New | `Read <plan, or the last commits and git status>. Finish <id> under docs/operating-rules.md: review the uncommitted changes, commit them by logical step, then carry on from the first unfinished step.` |
| `review-fixes` | Review fixes | Same | `Change <what>; stay in scope, and commit it as its own step.` |
| `records-lag` | Records lag the tree | Prepend to the next prompt | `First bring ROADMAP.md in line with the tree in its own docs(roadmap) commit: <what the tree shows>.` |
| `release` | Release | New | `Prepare and release 0.<NN>.` |

Why new: a plan, a commit or ROADMAP.md is the hand-off between sessions, so new work starts clean and loses nothing.
