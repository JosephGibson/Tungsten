---
name: tungsten-next
description: Read where the road to 1.0 stands from the repo, flag records that lag the tree, and give the owner the next prompt to paste and whether it runs in a new or the same session; sync the roadmap artifact. Use when asked what's next, for the next prompt, or for 1.0 status. Not for running the step.
---

# tungsten-next

Answers "what do I run next, and where?" on the road to 1.0. The repo is the source of truth; the [roadmap artifact](https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9) mirrors it. Read-only: no file edits, no git writes, no builds or checks. The one write is the artifact's database, from Claude Code. Never run the step you recommend, however small: hand it over. Sessions end with the next one's prompt (workflow §3), so the owner runs this once per release, after the merge, to sync the roadmap, or to resume after a crash or a lost prompt.

## 1. Gather, in one round of parallel calls

- Run `bash .claude/skills/tungsten-next/state.sh` from the repo root; don't read the script. It prints git and release state, other agent sessions open in this tree, a running perf capture, uncommitted edits in time-ordered groups, what the release commit's `git add -A` would take or miss, the [1.0 README](../../../docs/plans/1.0/README.md) "Now" lines, the open register rows, decisions not yet in `HEAD`, Step 0's items while its row is unreleased, each plan's status, approval, steps and evidence, and gate sign-offs.
- Read [prompts.md](prompts.md).
- Run `python3 -B scripts/roadmap.py status --json <scratch>/status.json` and `python3 -B scripts/roadmap.py catalog <scratch>/catalog.json`, `<scratch>` being the session's scratchpad, never the tree. `status` prints each stop's status with its evidence, the current stop, the next release and milestone.
- Claude Code: read [roadmap.md](roadmap.md) and list the artifact's `meta` and `stops` collections with `ArtifactData` (load it with ToolSearch `select:ArtifactData` when deferred). Any other client, such as Codex, has no `ArtifactData`: skip both, and §5 says what to print.

Then read the named stop's catalog entry, which fills its prompts (prompts.md says which field fills which bracket): `jq '.stops[] | select(.id == "<id>")' docs/plans/1.0/roadmap.json`. Read nothing else unless the state leaves the next step unclear; then read one section (a plan's open questions, a gate's agenda).

## 2. Check the records against the tree

The tree is evidence. The README "Now" lines, the register and plan headers are records that sessions write, and they lag when a session stops before its records step or two sessions share the tree. Trust the tree, then the register, then the "Now" lines; decide from the tree.

| The state shows | Say (Heads-up) | Then |
| --- | --- | --- |
| Work the records don't mention (Step 0 items present while "Next" says Step 0 hasn't started) | `<file> lags the tree: "<record>"; the tree has <evidence>.` | Records-lag line from prompts.md: prepended to the next prompt, or same session if the one that did the work is still open |
| Records claim work the tree lacks | Both sides, quoted | Ask the owner: the work may sit in another clone or a stash. Don't recommend redoing it |
| A step's items partly present, nothing edited in the last 3 minutes | The session stopped partway | Resume prompt |
| Edits in the last 3 minutes | A session is still working | Wait for it; give the step after it, marked "once it stops" |
| Another agent session open in this tree | Which step it ran (its start time against the edit groups); open is not editing | Close it or confirm it's idle before a new editing session. Two sessions whose uptimes both cover one edit group may have edited at once: the owner checks `git diff` there for crossed edits |
| "Next" and the register disagree | Both, quoted | Follow the one the tree supports; if neither, the register |

## 3. Decide the next step

First row that applies:

| State | Next |
| --- | --- |
| A perf capture is running | Nothing that writes. Give the step marked "once the capture ends" |
| A session is still working (§2) | The step after it, marked "once it stops" |
| A release is cut with no tag (the register row says cut; its plan archived) | You: ship it in one sitting (prompts.md, Ship). It comes before the next candidate's plan even when "Next" skips it |
| Work landed for a release with no plan or cut (register row landed, or a plan's run stopped before its release step) | You: review the uncommitted diff; then the release prompt, new session |
| Tag exists, release not on `origin/main` | You: finish shipping (approve, squash-merge, the post-merge block) |
| Release on `origin/main` | Sync the roadmap. Then the plan prompt the run session ended with, or the next candidate's; first, you: the post-merge block if the next branch is missing |
| "Next" says the owner reviews something | You: that review; fixes go to the same session |
| "Next" names a gate or a graduation | That prompt |
| First open register row has no plan | The plan prompt |
| Plan without an "Approved" line | Read its open questions; the approve prompt, same session |
| Approved, steps without evidence rows | The run prompt for its level, which ends in the release, or the resume prompt if its session closed |
| Frame-loop gate next, acceptance-game pitch not agreed | The game spec session first |

## 4. Where to run it

- **New session** for anything that starts work: a gate, a plan, a run, a graduation, a release, an experiment, a resume.
- **Same session** for what that session stopped to ask: plan approval, an API review, a gate question, fixes to work it just showed.
- Make "same" new when that session is closed or over about 60% context; the prompt then starts with `Read <plan or record> first.`
- Never two editing sessions in one tree at a time (workflow §6).

## 5. Sync the roadmap

Claude Code: follow [roadmap.md](roadmap.md): one `ArtifactData` batch with the catalog when its hash changed, the stops whose derived fields changed and `meta/now` from this reply; never ticks, never a republish. Then report on the Roadmap line. Any other client: write nothing and print `not synced (no ArtifactData in this client)`, followed by the status changes `scripts/roadmap.py status` shows, so the owner can rerun in Claude Code.

## 6. Reply

Exactly this shape, no other prose. Leave out sections that would be empty.

````markdown
**<release> · <candidate or stop>** · <phase> · <state in a few words>

**Next: <step>** · <you | new session | same session>

<If the step is yours: a checklist of actions and what to look for, then its commands.>
- [ ] <action>

**Your shell** (repo root)
```bash
<commands>
```

<The session prompt: for Next when it is a session's, or for the first session after your step.>
**New session** (repo root) · <one-clause reason>
```text
<the prompt, filled in>
```

**After that**
1. <step> · <new session | same session | you>

**Heads-up**
- <problem>: <action>

**Roadmap** · <what changed | already in step | not synced (why)> · https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9
````

- Every prompt goes in its own `text` block under a label line naming where it runs: **New session** (repo root) or **Same session** (the one that stopped for <what>). Owner shell commands go in one `bash` block under **Your shell**. Never put a prompt or a command in a sentence, a list item or inline code.
- Give blocks for Next and, when Next is yours, for the first session after it. List later steps by name only: their prompts depend on what comes first.
- Heads-up: one line per item, problem then action, most urgent first. Their commands join the **Your shell** block, or get their own after the list when Next has none.
- When state.sh warns (an upstream, an ignored skill, junk the release would sweep in, `origin/main` not in `HEAD` before tagging), that's a Heads-up item.
