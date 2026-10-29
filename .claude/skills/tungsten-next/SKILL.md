---
name: tungsten-next
description: Read where Tungsten stands from `just status` and ROADMAP.md, and give the owner the next prompt to paste and where it runs. Use when asked what's next, for the next prompt, or for status. Not for running the step.
---

# tungsten-next

Run `just status` from the repo root and read [prompts.md](prompts.md). ROADMAP.md is the only status record; the tree is the evidence. Read-only: never run the step you recommend, hand it over.

- Another session working in this tree, or a perf capture running: nothing that writes now; the step after it is still the next step, its prompt included, marked "once it ends".
- A phase plan in Now, which runs a step at a time, whose next step has no plan of its own: that step's `step` prompt, never the phase plan's `run` prompt.
- The newest tag not on `origin/main` is a heads-up, never the next step: the owner merges the release pull request by merge commit whenever convenient, and the merge publishes nothing and gates no work (`D-227`).

## Reply

One prompt, the next step's; later steps by name only. Leave out empty sections.

````markdown
**<version> · <Now item>** · <state in a few words>

**Next: <step>** · <you | new session | same session>

**New session** (repo root) · Opus 5.5 · <max for art or sound, else xhigh> · <route> · <one-clause reason>
```text
<the prompt, filled in>
```
**NoMachine** · <disconnect after pasting, until it ends, for timing captures; else stay connected>

**Heads-up**
- <problem>: <action>
````
