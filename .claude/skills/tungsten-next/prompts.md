# tungsten-next prompts

The only copy of the road to 1.0's prompts: workflow §3 and the roadmap page link here or receive them filled. Fill every angle bracket; a reply never shows one. Each session ends with the next one's prompt from this table (workflow §3): a plan session with `approve`, an approved plan with its `run` row, a run with the next stop's `plan` row. Templates are literal: what the table shows is what gets pasted, `Don't commit: leave the changes in the tree for the release commit.` (`D-105`) included. Keys are stable: the Flow table, the page's ticks (`<status>:<key>`) and `meta/now` use them, never the Step titles. **New** is a fresh session at the repo root; **same** is the session that stopped to ask. Make a "same" step new when that session is closed or over about 60% context, and start its prompt with `Read <plan or record> first.`

Fill from the stop's entry in `docs/plans/1.0/roadmap.json`: `<candidate>` is its `row` (a pair names both candidates), `<sources>` its `src` joined with "and", `<slug>` its `slug` or a short name for the card, `<N>` its phase, `<gate>` and `<reads>` a gate's `gate` and `reads`. `<NN>` is one past the highest milestone in the register (the first is M32).

## Session prompts

| Key | Step | Where | Prompt |
| --- | --- | --- | --- |
| `gate` | Gate | New | `Run the <gate>: read the gate's row in docs/plans/1.0/implementation-plan.md §2 and <reads>; ask me each open question with its default; write the gate record and the decisions it settles. Don't commit: leave the changes in the tree for the release commit.` |
| `answer` | Answer a gate question | Same | The answer, or `Default.` |
| `graduate` | Graduation | New | `Graduate <W>: read docs/plans/1.0/workflow.md §2 (Graduation) and the conventions in docs/plans/1.0/README.md; move its settled criteria.md section verbatim into its workstream file with links fixed, fold in the accepted amendments that touch it, leave a placement paragraph behind, and turn its steps into candidates in order, each with a done-when. Don't commit: leave the changes in the tree for the release commit.` |
| `review-fixes` | Review fixes | Same | `Change <what>; then stop for my review again. Don't commit: leave the changes in the tree for the release commit.` |
| `records-lag` | Records lag the tree | Prepend to the next prompt | `First bring <the README "Now" lines, the register row for <X>, …> in line with the tree: <what the tree shows>.` To the session that did the work instead, if it is still open: same session, with `Then stop for my review.` |
| `plan` | Plan | New | `Use the tungsten-milestone skill. Write the milestone plan for <candidate>: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and <sources>; audit only the code it touches; name it docs/plans/1.0/phase<N>-milestone-<NN>-<slug>.md; critique it last as the skill's step 7 says (--force is confirmed for the plan file); stop for my approval and end with the approval prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `qa-plan` | QA plan | New | `Write the Phase <N> QA plan as <path>: audit what Phase <N> changed, in the 0.40 QA shape (docs/plans/1.0/workflow.md §2, QA pass): findings with file:line evidence, numbered steps that each fit one session, the release as the last step, open questions with defaults; stop for my approval and end with the approval prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `approve` | Approve a plan | Same | `Approved <YYYY-MM-DD> with the stated defaults[, except: <changes>]. Add the approval line under the open questions in <plan>, then give me the run prompt for a new session. Don't commit: leave the changes in the tree for the release commit.` |
| `run` | Run, levels A and B | New | `Use the tungsten-milestone skill. Read <plan> and run every remaining step in order, the release last, stop conditions as written; end with the changed-file list, the command block, the post-merge block and the next prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `run-c1` | Run, level C, step 1 | New | `Use the tungsten-milestone skill. Read <plan> and run step 1; stay in scope. Then stop for my API review: show me the API sketch and the template diff, not the whole diff. Don't commit: leave the changes in the tree for the release commit.` |
| `run-c-rest` | Run, level C, the rest | Same | `API approved. Use the tungsten-milestone skill: read <plan> and run every remaining step from step 2, the release last, stop conditions as written; end with the changed-file list, the command block, the post-merge block and the next prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `resume` | Resume after a crash or a closed session | New | `Use the tungsten-milestone skill. Read <plan>; find the first step without an evidence-log row and run it; stay in scope. Don't commit: leave the changes in the tree for the release commit.` |
| `compact` | Compact a long session | Same | `/compact focus on the remaining steps of <plan>` |
| `release` | Release, to resume one or with no plan | New | `Prepare and release 0.NN.` |
| `verify` | Verify after the merge (optional) | New | `Verify the 0.NN release: run the read-only preflight and the archive checks.` |
| `game-spec` | Game spec, before the frame-loop gate | New | `Agree the acceptance game's spec with me before the frame-loop gate: read docs/plans/1.0/acceptance-game.md and the frame-loop row in docs/plans/1.0/implementation-plan.md §2; take the pitch, screens and mechanics one at a time, each with its current proposal as the default; then write the mechanics rows of the feature map and propose W6 and W13 tiers from them. Don't commit: leave the changes in the tree for the release commit.` |
| `experiment` | Experiment | New | `Run the <name> spike for the <gate> gate: read its item under Track B in docs/plans/1.0/implementation-plan.md §3, and docs/plans/1.0/workflow.md §2 (Spike) and §6; work in a scratch copy; keep the patch, scripts, captures and a README with the verdict in perf-runs/<YYYYMMDD>-<slug>/; merge nothing. Don't commit: leave the changes in the tree for the release commit.` |
| `rc-checklist` | Release candidate checklist | New | `Run C3 on the latest v1.0.0-rc tag: work through docs/plans/1.0/release-checklist.md and write a dated QA record; stop for my playthrough. Don't commit: leave the changes in the tree for the release commit.` |
| `rc-fix` | Release candidate failure | New | `Fix the C3 failure in the QA record, then prepare the next v1.0.0-rc prerelease. Don't commit: leave the changes in the tree for the release commit.` |
| `game-feature` | Acceptance game feature | New, in the game's repository | `Add <feature> using only the public API and tungsten-kit; log each engine gap in GAPS.md with a minimal repro.` |

## NoMachine

Every session prompt in a reply says whether the owner disconnects NoMachine until the session ends. A connected client runs the encoder, `nxcodec.bin`, and timing captures need it gone (workflow §6.3). Decide from the state's plan line, not from the step's name:

| Session | NoMachine line |
| --- | --- |
| `run`, `run-c-rest` or `resume` whose plan line lists captures without evidence; `run-c1` only when step 1 is listed | `disconnect after pasting, until it ends · step <N>[, <N>…] capture<s>` |
| `experiment` | `disconnect after pasting, until it ends · spikes capture`, unless its Track B item names no capture |
| Anything else: plans, approvals, answers, gates, graduations, the game spec, reviews, releases, release candidates | `stay connected · no captures` |

The plan line's list counts the runner's `run`, `suite` and `capacity` and criterion's `cargo bench`. It leaves out captures with `--allow-background`, which check only digests and tolerate the encoder, and `just smoke` and `just visual`, which need the display, not a quiet machine (workflow §6.4). When unsure, read the listed steps. A session waits for the encoder to exit before each sitting, so disconnecting late costs time; reconnecting during a capture costs that sitting. When the state shows the encoder running while a capture runs or something waits on the encoder, the first Heads-up item is `NoMachine is holding up <the capture or session>: disconnect now`.

## Flow

The keys each kind of stop runs, in order, with where each runs and the status the stop has while it is next. A step runs once unless its note says otherwise; `resume`, `compact`, `release`, `verify`, `records-lag`, `review-fixes` and `answer` can come at any point. `scripts/roadmap.py` reads this table into each stop's stages on the roadmap page, with a mode, model and effort for each session step (`D-118`), so keep each step as `` `key` (where, status) `` or `` `key` (where, status; note) ``, joined by `→`; a note "the <name> gate only" keeps the step on that gate alone.

| Stop | Steps: key (where, status) |
| --- | --- |
| Release, level A or B | `plan` (new, todo) → `approve` (same, plan) → `run` (new, run) → `ship` (you, ready or review) |
| Release, level C | `plan` (new, todo) → `approve` (same, plan) → `run-c1` (new, run) → `run-c-rest` (same, run) → `ship` (you, ready or review) |
| QA pass | `qa-plan` (new, todo) → `approve` (same, plan) → `run` (new, run) → `ship` (you, ready or review) |
| Gate | `game-spec` (new, todo; the frame-loop gate only, until the pitch is agreed) → `gate` (new, todo) → `graduate` (new, todo; one per workstream the record graduates) |
| Experiments (Track B) | `experiment` (new, todo; one per spike, in §3's order) |
| Release candidates (C3) | `rc-checklist` (new, todo) → `playthrough` (you, run) → `rc-fix` (new, run; after a failure, then `rc-checklist` again) |

## Owner steps

Each is a checklist in the reply; its commands go in one `bash` block, run from the repo root.

**Ship** (`ship`), one sitting after the run session ends (workflow §9). Review the diff: the areas the state's edit groups name, the `??` paths, and that nothing outside the work slipped in. Paste the command block the session ended with, approve and squash-merge the pull request on GitHub, then paste its post-merge block, which starts the next branch. Then `/tungsten-next` syncs the roadmap.

```bash
git status --short
git --no-pager diff --stat HEAD
git --no-pager diff HEAD -- <path>
```

The post-merge block, if the session's output is lost (preflight prints it only until the merge):

```bash
git fetch origin &&
git switch -c 0.NN --no-track origin/main &&
git push -u origin 0.NN
```

**Playthrough** (`playthrough`). Play the release candidate start to finish from its archive, and note anything that fails.

Why new or same: a plan, a record or the evidence log is the hand-off between sessions, so new work starts clean and loses nothing; an answer goes back to the session that asked, which already holds the context.
