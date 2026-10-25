---
name: tungsten-next
description: Read where Tungsten stands from the repo and ROADMAP.md, flag records that lag the tree, and give the owner the next prompt to paste and whether it runs in a new or the same session. Use when asked what's next, for the next prompt, or for status. Not for running the step.
---

# tungsten-next

Answers "what do I run next, and where?". The tree and [ROADMAP.md](../../../ROADMAP.md) are the sources, under the [operating rules](../../../docs/operating-rules.md). Read-only: no file edits, no git writes, no builds or checks. Never run the step you recommend, however small: hand it over.

## 1. Gather, in one round of parallel calls

- Run `bash .claude/skills/tungsten-next/state.sh` from the repo root; don't read the script. It prints the branch and upstream, the workspace version, the newest tag and whether it is on `origin/main`, the commits since the last release, `git status --short`, files edited in the last 3 minutes, other agent sessions in this tree, a running perf capture, frozen records edited since `D-215`, and ROADMAP.md's Now and Next with Now's count.
- Read [prompts.md](prompts.md).

Read nothing else unless the state leaves the next step unclear; then read one source the item cites (its feedback section, a plan's header). Never take status from the frozen records (the register, the catalog, the 1.0 README's owned fields): they stopped at `D-215`.

## 2. Check the records against the tree

The tree is evidence; ROADMAP.md is the only status record, and the commit that starts or finishes an item updates it.

| The state shows | Say (Heads-up) | Then |
| --- | --- | --- |
| A Now item the tree shows finished (its commits landed, the tag it waited on exists, its plan deleted) | `ROADMAP.md lags the tree: "<item>"; the tree has <evidence>.` | Records-lag line, prepended to the next prompt |
| Commits or uncommitted work that no Now item covers | Quote both | Records-lag: add the item to Now, or name the scope creep |
| Now with more than three items | The count | Records-lag: keep the one to three in work, the rest back to Next (`just repo-check` refuses more) |
| A frozen record edited since `D-215` (D-215 in HEAD) | File and commits | Records-lag: undo the edit, the status goes in ROADMAP.md. Before D-215 is in HEAD, the banner edits are the cutover's own |
| Records claim work the tree lacks | Both sides, quoted | Ask the owner: it may sit in another worktree, clone or stash. Don't recommend redoing it |
| Edits in the last 3 minutes, or another session in this tree | A session is still working | Give the step after it, marked "once it stops" |
| Uncommitted work, nothing edited in the last 3 minutes, no other session | A session stopped before its commit | Resume prompt |

## 3. Decide the next step

First row that applies:

| State | Next |
| --- | --- |
| A perf capture is running | Nothing that writes. Give the step marked "once the capture ends" |
| A session is still working (§2) | The step after it, marked "once it stops" |
| The version is past the newest tag (a cut waits) | You: the release session's command block (commit, tag, push, pull request), or the release prompt if it is lost |
| The newest tag is not on `origin/main` | You: merge the release pull request without squashing |
| Uncommitted work, no session working | The resume prompt, new session |
| A large plan in `docs/plans/` for a Now item | Its run or resume prompt, new session |
| An example stop just finished | You: play it before the next one is planned; findings go to Later as one line each, or re-cut the next stop. Skip when the owner says they have played it |
| A Now item not started | Its prompt by route (prompts.md), new session |
| Now is empty | Next's first item, its prompt opening with a records-lag line that moves it into Now |

## 4. Route, tier and where to run it

- **Route** by [the plan rules](../../../docs/operating-rules.md#plans): small (no plan, the commit is the record), medium (scope, non-goals, files and checks stated in the session), large (multi-session or risky: one plan file, the tungsten-milestone skill, one critique round at xhigh).
- **Tier:** a session that generates art or sound runs on Opus 5.5 at max; every other session at xhigh. An example stop that needs new art splits off an asset session ([redo §6](../../../docs/plans/roadmap-redo.md#6-asset-sessions)).
- **New session** for anything that starts work. **Same session** only for fixes to work an open session just showed, under about 60% context; otherwise new, its prompt starting `Read <plan or commit> first.`
- Never two editing sessions in one tree: a parallel one gets its own worktree, which the owner makes (`git worktree add`).
- A session that takes timing captures (`just perf run|suite|capacity`, `cargo bench`) needs the owner's NoMachine client disconnected until it ends; every other session stays connected.

## 5. Reply

Exactly this shape, no other prose. Leave out sections that would be empty.

````markdown
**<version> · <Now item>** · <state in a few words>

**Next: <step>** · <you | new session | same session>

<If the step is yours: a checklist of actions and what to look for, then its commands.>
- [ ] <action>

**Your shell** (repo root)
```bash
<commands>
```

<The session prompt: for Next when it is a session's, or for the first session after your step.>
**New session** (repo root) · Opus 5.5 · <max | xhigh> · <route> · <one-clause reason>
```text
<the prompt, filled in>
```
**NoMachine** · <stay connected | disconnect after pasting, until it ends>

**After that**
1. <step> · <new session | same session | you>

**Heads-up**
- <problem>: <action>
````

- Every prompt goes in its own `text` block under a label line naming where it runs: **New session** (repo root) or **Same session** (the open one whose work it fixes). Owner shell commands go in one `bash` block under **Your shell**. Never put a prompt or a command in a sentence, a list item or inline code.
- Give blocks for Next and, when Next is yours, for the first session after it. List later steps by name only: their prompts depend on what comes first.
- Heads-up: one line per item, problem then action, most urgent first. Their commands join the **Your shell** block.
