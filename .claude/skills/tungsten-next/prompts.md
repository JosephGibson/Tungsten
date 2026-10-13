# tungsten-next prompts

The only copy of the road to 1.0's prompts: workflow §3 and the roadmap page link here or receive them filled. Fill every angle bracket; a reply never shows one. Each session ends with the next one's prompt from this table (workflow §3): a plan session with its stop's `run` row, a run with the next stop's `plan` row. No session stops for the owner's approval (`D-150`): open questions take their stated defaults, and the owner reviews plans and records on their own time. Templates are literal: what the table shows is what gets pasted, `Don't commit: leave the changes in the tree for the release commit.` (`D-105`) included. Keys are stable: the Flow table, the page's ticks (`<status>:<key>`) and `meta/now` use them, never the Step titles. **New** is a fresh session at the repo root. **Same** is the open session whose work a `review-fixes` prompt changes, under about 60% context; otherwise a new one, whose prompt starts with `Read <plan or record> first.`

Fill from the stop's entry in `docs/plans/1.0/roadmap.json`: `<candidate>` is its `row` (a pair names both candidates), `<sources>` its `src` joined with "and", `<slug>` its `slug` or a short name for the card, `<N>` its phase, `<gate>` and `<reads>` a gate's `gate` and `reads`. `<NN>` is one past the highest milestone in the register (the first is M32).

## Session prompts

| Key | Step | Where | Prompt |
| --- | --- | --- | --- |
| `gate` | Gate | New | `Run the <gate>: read the gate's row in docs/plans/1.0/implementation-plan.md §2 and <reads>; settle each open question at its stated default (D-150); write the gate record with its sign-off, the decisions it settles and the roadmap.json changes beside the register. Don't commit: leave the changes in the tree for the release commit.` |
| `review-fixes` | Review fixes | Same | `Change <what>; stay in scope. Don't commit: leave the changes in the tree for the release commit.` |
| `records-lag` | Records lag the tree | Prepend to the next prompt | `First bring <the register row for <X>, …> in line with the tree: <what the tree shows>.` |
| `plan` | Plan | New | `Use the tungsten-milestone skill. Write the milestone plan for <candidate>: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and <sources>; audit only the code it touches; name it docs/plans/1.0/phase<N>-milestone-<NN>-<slug>.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `qa-plan` | QA plan | New | `Write the Phase <N> QA plan as <path>: audit what Phase <N> changed, in the QA shape (docs/plans/1.0/workflow.md §2, QA pass): findings with file:line evidence, numbered steps, done-when checks as commands, an evidence log, the release as the last step, open questions with defaults; critique it last as the tungsten-milestone skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `run` | Run | New | `Use the tungsten-milestone skill. Read <plan> and run every remaining step in order, the release last, stop conditions as written; end with the changed-file list, the command block, the post-merge block and the next prompt. Don't commit: leave the changes in the tree for the release commit.` |
| `resume` | Resume after a crash or a closed session | New | `Use the tungsten-milestone skill. Read <plan>. If its critique is not recorded, finish the plan from the skill's step 7 and end with the run prompt; otherwise run every step without an evidence-log row in order, the release last, stop conditions as written. Don't commit: leave the changes in the tree for the release commit.` |
| `compact` | Compact a long session | Same | `/compact focus on the remaining steps of <plan>` |
| `release` | Release, to resume one or with no plan | New | `Prepare and release 0.NN.` |
| `verify` | Verify after the merge (optional) | New | `Verify the 0.NN release: run the read-only preflight and the archive checks.` |
| `experiment` | Experiment | New | `Run the <name> spike for the <gate> gate: read its item under Track B in docs/plans/1.0/implementation-plan.md §3, and docs/plans/1.0/workflow.md §2 (Spike) and §6; work in a scratch copy; keep the patch, scripts, captures and a README with the verdict in perf-runs/<YYYYMMDD>-<slug>/; merge nothing. Don't commit: leave the changes in the tree for the release commit.` |
| `rc-checklist` | Release candidate checklist | New | `Run C3 on the latest v1.0.0-rc tag: work through docs/plans/1.0/release-checklist.md, write a dated QA record and end with the owner's playthrough checklist. Don't commit: leave the changes in the tree for the release commit.` |
| `rc-fix` | Release candidate failure | New | `Fix the C3 failure in the QA record, then prepare the next v1.0.0-rc prerelease. Don't commit: leave the changes in the tree for the release commit.` |
| `game-feature` | Acceptance game feature | New, in the game's repository | `Add <feature> using only the public API and tungsten-kit; log each engine gap in GAPS.md with a minimal repro.` |

## NoMachine

Every session prompt in a reply says whether the owner disconnects NoMachine until the session ends. A connected client runs the encoder, `nxcodec.bin`, and timing captures need it gone (workflow §6.3). Decide from the state's plan line, not from the step's name:

| Session | NoMachine line |
| --- | --- |
| `run` or `resume` whose plan line lists captures without evidence | `disconnect after pasting, until it ends · step <N>[, <N>…] capture<s>` |
| `experiment` | `disconnect after pasting, until it ends · spikes capture`, unless its Track B item names no capture |
| Anything else: plans, gates, reviews, releases, release candidates | `stay connected · no captures` |

The plan line's list counts the runner's `run`, `suite` and `capacity` and criterion's `cargo bench`. It leaves out captures with `--allow-background`, which check only digests and tolerate the encoder, and `just smoke` and `just visual`, which need the display, not a quiet machine (workflow §6.4). When unsure, read the listed steps. A session waits for the encoder to exit before each sitting, so disconnecting late costs time; reconnecting during a capture costs that sitting. When the state shows the encoder running while a capture runs or something waits on the encoder, the first Heads-up item is `NoMachine is holding up <the capture or session>: disconnect now`.

## Flow

The keys each kind of stop runs, in order, with where each runs and the status the stop has while it is next. A step runs once unless its note says otherwise; `resume`, `compact`, `release`, `verify`, `records-lag` and `review-fixes` can come at any point. `scripts/roadmap.py` reads this table into each stop's stages on the roadmap page, with a mode, model and effort for each session step (`D-118`, `D-151`), so keep each step as `` `key` (where, status) `` or `` `key` (where, status; note) ``, joined by `→`.

| Stop | Steps: key (where, status) |
| --- | --- |
| Release | `plan` (new, todo) → `run` (new, plan or run) → `ship` (you, ready or review) |
| QA pass | `qa-plan` (new, todo) → `run` (new, plan or run) → `ship` (you, ready or review) |
| Gate | `gate` (new, todo) |
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

Why new: a plan, a record or the evidence log is the hand-off between sessions, so new work starts clean and loses nothing.
