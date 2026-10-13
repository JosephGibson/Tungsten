---
name: tungsten-next
description: Read where the road to 1.0 stands from the repo, flag records that lag the tree, and give the owner the next prompt to paste and whether it runs in a new or the same session; sync the roadmap artifact. Use when asked what's next, for the next prompt, or for 1.0 status. Not for running the step.
---

# tungsten-next

Answers "what do I run next, and where?" on the road to 1.0. The repo is the source of truth; the [roadmap artifact](https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9) mirrors it. Read-only: no file edits, no git writes, no builds or checks. The one write is the artifact's database, from Claude Code. Never run the step you recommend, however small: hand it over. Sessions end with the next one's prompt (workflow §3), so the owner runs this once per release, after the merge, to sync the roadmap, or to resume after a crash or a lost prompt.

## 1. Gather, in one round of parallel calls

- Run `bash .claude/skills/tungsten-next/state.sh` from the repo root; don't read the script. It prints git and release state, other agent sessions open in this tree, a running perf capture, the NoMachine encoder and what waits on it, uncommitted edits in time-ordered groups, what the release commit's `git add -A` would take or miss, the four derived [Now fields](../../../docs/validation.md#read-only-selector), decisions not yet in `HEAD`, each plan's status, whether its run started and its critique is recorded, its steps, evidence and capture steps, and gate sign-offs.
- Read [prompts.md](prompts.md).
- Run `python3 -B scripts/roadmap.py status --json <scratch>/status.json` and `python3 -B scripts/roadmap.py catalog <scratch>/catalog.json`, `<scratch>` being the session's scratchpad, never the tree. `status` prints each stop's status with its evidence, the current stop, the next release and milestone.
- Claude Code: read [roadmap.md](roadmap.md) and list the artifact's `meta` and `stops` collections with `ArtifactData` (load it with ToolSearch `select:ArtifactData` when deferred). Any other client, such as Codex, has no `ArtifactData`: skip both, and §5 says what to print.

Then read the named stop's entry in the payload, which fills its prompts (prompts.md says which field fills which bracket) and holds its stages, each session's with its recommendation: `jq '.catalog.stops[] | select(.id == "<id>")' <scratch>/catalog.json`. Read nothing else unless the state leaves the next step unclear; then read one section (a plan's open questions, a gate's agenda).

## 2. Check the records against the tree

The tree is evidence. `roadmap.py` derives stop status and the first open stop from register/plan evidence and local refs; the README's four owned fields mirror it. Do not reparse free-form register prose or use timestamps to decide completion. Check other prose and plan headers against the step's actual evidence.

| The state shows | Say (Heads-up) | Then |
| --- | --- | --- |
| Work the records don't mention (a stop's work in the tree while its register row says it hasn't started) | `<file> lags the tree: "<record>"; the tree has <evidence>.` | Records-lag line from prompts.md, prepended to the next prompt |
| Records claim work the tree lacks | Both sides, quoted | Ask the owner: the work may sit in another clone or a stash. Don't recommend redoing it |
| A step's items partly present, nothing edited in the last 3 minutes | The session stopped partway | Resume prompt |
| Edits in the last 3 minutes | A session is still working | Wait for it; give the step after it, marked "once it stops" |
| Another agent session open in this tree | Which step it ran (its start time against the edit groups); open is not editing | Close it or confirm it's idle before a new editing session. Two sessions whose uptimes both cover one edit group may have edited at once: the owner checks `git diff` there for crossed edits |
| The README's owned fields disagree with derived status | Both, quoted | Records-lag prompt: `python3 -B scripts/roadmap.py summary --write`; other roadmap prose is not another status source. A cut, tagged or merged stop already counts as closed (`D-152`), so a merge alone never makes them lag |

## 3. Decide the next step

First row that applies:

| State | Next |
| --- | --- |
| A perf capture is running | Nothing that writes. Give the step marked "once the capture ends" |
| A session is still working (§2) | The step after it, marked "once it stops" |
| A release is cut with no tag (the workspace version is the branch's `0.NN.0` and the state shows `tag v0.NN.0: none`; on the road its register row says cut) | You: ship it in one sitting (prompts.md, Ship). It comes before the next stop's plan |
| Work landed for a release with no cut (a plan's run stopped before its release step, or off-roadmap work in the tree) | You: review the uncommitted diff; then the release prompt, new session |
| Tag exists, release not on `origin/main` | You: finish shipping (approve and squash-merge the pull request, the post-merge block) |
| Release on `origin/main` | Sync the roadmap. Then the plan prompt the run session ended with, or the next stop's; first, you: the post-merge block if the next branch is missing |
| The first open stop is a gate | That prompt |
| The first open stop is a release or a QA pass with no plan | The plan or QA-plan prompt |
| A plan with no critique record, no session working | The plan session stopped partway: the resume prompt, new session |
| A plan not started, its critique recorded | The run prompt, new session |
| A plan whose run started, steps without evidence rows, no session working | The resume prompt, new session |

## 4. Where to run it

- **New session** for anything that starts work: a gate, a plan, a run, a release, an experiment, a resume.
- **Same session** only for fixes the owner asks of work an open session just showed (`review-fixes`); no session stops to ask (`D-150`).
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
**New session** (repo root) · <mode> mode · <model> · <effort> · <one-clause reason>
```text
<the prompt, filled in>
```
**NoMachine** · <the line from prompts.md's NoMachine table>

**After that**
1. <step> · <new session | same session | you>

**Heads-up**
- <problem>: <action>

**Roadmap** · <what changed | already in step | not synced (why)> · https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9
````

- Every prompt goes in its own `text` block under a label line naming where it runs: **New session** (repo root), with the step's `rec` as [roadmap.md](roadmap.md) copies it, or **Same session** (the open one whose work it fixes). Owner shell commands go in one `bash` block under **Your shell**. Never put a prompt or a command in a sentence, a list item or inline code. Under each prompt's block goes its **NoMachine** line ([prompts.md](prompts.md#nomachine)).
- Give blocks for Next and, when Next is yours, for the first session after it. List later steps by name only: their prompts depend on what comes first.
- Heads-up: one line per item, problem then action, most urgent first. Their commands join the **Your shell** block, or get their own after the list when Next has none.
- When state.sh warns (an upstream, an ignored skill, junk the release would sweep in, `origin/main` not in `HEAD` before tagging), that's a Heads-up item.
