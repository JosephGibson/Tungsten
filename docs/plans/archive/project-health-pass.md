# 0.66 project-health pass: no approval stops, leaner tiers, a stronger critic

- **status:** done
- **goal:** Make the rest of the road to 1.0 faster and smoother: take every owner-approval stop out of the roadmap workflow, move Opus 5.5 max sessions down to xhigh where max does not pay for itself, fix agent-facing text that is stale, duplicated or paid for by every session, pull roadmap work forward where its dependencies allow, trim safe bloat, and make `gpt-6-astra` the critic with a larger budget. Release it as 0.66.
- **non-goals:** Roadmap stop work, which 0.67 resumes. Engine code, public API changes and perf captures. `~/.claude/CLAUDE.md`, `~/.claude/RTK.md` and user-level skills other than `critique`. Public API removals, which become follow-up stops (§9). Git publication, which stays with the owner (`D-105`, `D-079`).
- **files to touch:** Step 1: `~/.local/bin/critique`, `~/.config/ai/prompts/critic.md`, `~/.config/ai/prompts/critic-repo.md`, `~/.claude/skills/critique/SKILL.md`, `.claude/skills/tungsten-milestone/SKILL.md` (step 7), `.claude/skills/tungsten-milestone/skeleton.md` (§9). Step 2: `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 3: `scripts/roadmap.py`, `scripts/test-roadmap.py`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `.claude/skills/tungsten-next/state.sh`, `justfile`, `docs/releases.md` (the cut's paragraph), and `prompts.md`'s Flow table, which the new `roadmap.py` reads. Step 4: `.claude/skills/tungsten-next/{SKILL.md,prompts.md,roadmap.md,page.html}`, `.claude/skills/tungsten-milestone/{SKILL.md,skeleton.md}`. Step 5: `docs/plans/README.md`, `docs/plans/1.0/{README.md,workflow.md,implementation-plan.md,roadmap.json,criteria.md,w04-api-freeze.md,w05-input.md,w11-shipping-basics.md,w14-tooling.md,w16-prefabs.md}`. Step 6: `AGENTS.md`, `crates/tungsten-render/AGENTS.md`, `.claude/skills/tungsten-wgpu/SKILL.md`, `docs/agent-setup.md`, `docs/validation.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/ai-development-workflow.md` (archived), and the link in `CHANGELOG.md`'s 0.61 entry that pointed at it. Step 7: the project memory folder. Step 8: the roadmap artifact. Step 9: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the 1.0 README's owned block, this plan (archived).
- **ordered steps:** (1) The critic. (2) Decisions 150–152. (3) Roadmap tooling. (4) Prompts and skills. (5) Program records and the pull-forward. (6) Agent docs. (7) Memory. (8) The roadmap page. (9) Release 0.66.
- **done-when:** `just check`, `just script-test` and `just repo-check` pass, and `python3 -B scripts/roadmap.py status` runs clean; a plain `critique --repo` run on this plan ends with a footer naming `model gpt-6-astra`; `/tungsten-next` gives the 0.67 prompt with no approval stage, and the roadmap artifact shows the new tiers; the 0.66 release runs through tungsten-release and ends with the owner's commit, tag, push and pull-request block.

Written 2026-10-13 on branch `0.66` at `12ccd36` (0.65 merged), tree clean. Bounded maintenance under `D-140`, kept as a saved plan in the QA-pass shape (workflow §2): findings with `file:line` evidence read at `12ccd36`, numbered steps, done-when commands, an evidence log, and the release last. The owner reviews this file on their own time, and open questions take their stated defaults (§4). Finding labels follow the request's items: A approval stops, T tiers, W workflow audit, R roadmap pull-forward, B bloat, C critique. **Mine** marks the author's own recommendations (§2).

## Context digest

- 0.66 is a short break from the road to 1.0; 0.67 resumes it at W16a (`roadmap.py status`: first open stop `w16a`, next milestone M48). Everything here is workflow, tooling, records and the critic: no engine code moves, so the hashes, digests and pixel fixtures cannot.
- Today a candidate costs the owner a plan approval, a level C candidate an API review, and a gate an owner sitting. Gates also run in plan mode, which waits for the user's approval before it edits. After every merge the README's owned fields go stale, so `just repo-check` fails on this branch now; before the block arrived in 0.61, 14 sessions opened with a records-lag prompt for the hand-written lines.
- `tier()` puts 43 of the 46 remaining session stages at Opus 5.5 max. Transcripts show Opus max plan sessions writing about 1.5× the output tokens of xhigh ones (on different plans, so a bound on cost, not quality), with no outcome evidence for max.
- The critic is gpt-6.1-sol at xhigh, capped at five findings, told to read as little as possible, at the model's default context window. 11 of 25 repo-mode plan critiques hit the cap.
- Decisions involved: `D-079` and `D-105` (publication stays the owner's), `D-108` (pairs), `D-109` (catalog and status), `D-118` (stages and tiers), `D-136` (Opus max for A/B releases), `D-140` (routes; approval and API review kept).

## 1. Findings

Read at `12ccd36`. Line numbers are that commit's.

### A. Owner-approval stops (item 1)

| # | Stop | Evidence |
| --- | --- | --- |
| A1 | Plan → approve → run. The plan and QA-plan prompts end with "stop for my approval and end with the approval prompt"; `approve` is a same-session row; the milestone skill holds the run until the approval line | `.claude/skills/tungsten-next/prompts.md:16-18`, Flow `:50-52`; `.claude/skills/tungsten-milestone/SKILL.md:23-24`, `:28`; `skeleton.md:50` |
| A2 | Plan → run status comes from an `^Approved` line | `scripts/roadmap.py:292-298` (`plan_progress`), `:365-370`; `.claude/skills/tungsten-next/state.sh:190`, `:210-211`; `roadmap.md:13-14` (`plan` "written, not approved", ticks `plan:approve`); `page.html:271` ("plan in review") |
| A3 | Same-session stages keep their opener's settings: `KEEPS`, `keeps` and `if_new` exist only for `approve` and `run-c-rest` | `scripts/roadmap.py:45`, `:172-187`; `roadmap.md:16`, `:18`; `page.html:313-315`, `:411`, `:568`, `:638` |
| A4 | Level C API review after step 1 | `prompts.md:20-21`, Flow `:51`; milestone `SKILL.md:29`; `skeleton.md:26`; `docs/plans/1.0/workflow.md:72`, `:133`; `implementation-plan.md` Owner-only cells `:163`, `:165-167`, `:170`, `:176` (Phase 6); `roadmap.json` `"roles": ["api"]` on `w16a`, `w13ab`, `w16b`, `w13cd`; `w16-prefabs.md:19` ("at W16b's API review") |
| A5 | Gate questions. The gate prompt asks the owner each question, `answer` returns it, gates are "a session with the owner present", the record ends with "the owner's sign-off", and gates run in plan mode, whose exit asks the user to approve | `prompts.md:11-12`, Flow `:53`; `workflow.md:41`, `:131`; `implementation-plan.md:35`, `:329`; `roadmap.py:44` (`PLAN_MODE`); `roadmap.json` `decide` roles on `gfeat`, `c1`, `gfreeze`, `grc`, `g10` |
| A6 | Open questions with no default a session can take: q22's default reads "Asked in the W13c plan", and q2's leaves publishing "open"; W11b's Q15 and W14b's Q20 are "answered before the plan is approved" | `roadmap.json` `q22`, `q2`; `criteria.md:223`, `:228`; `w11-shipping-basics.md:54`; `w14-tooling.md:50` |
| A7 | Policy text that keeps them | `workflow.md:15`, `:19`, `:43`, `:47`, `:50`, `:56`, `:60`, `:66`, `:70-73`, `:98`, `:127-133`; `docs/plans/README.md:11`; `docs/validation.md:27`; `AGENTS.md:57` ("Milestone/API review requirements stand"); `tungsten-next/SKILL.md:47-48`, `:54`; `prompts.md:3`, `:14`, `:15`, `:40`, `:79`; `implementation-plan.md:178` ("Plan approval"; Phase 5's cards, such as `:128`, keep theirs as the record of what those candidates cost) |
| A8 | Decisions that require them | `D-140` (`DECISIONS.md:1719`: "retains the program's plan, critique, approval and applicable level C API review"); `D-118` (`DECISIONS.md:1442`: `approve` and `run-c-rest` keep their session) |

What stays the owner's: the ship sitting (diff review, the command block, the merge and the post-merge block, `D-105`, `D-079`), hands-on checks that need a person or hardware (playthroughs, a gamepad, an alt-tab, Windows), and any decision they choose to revisit.

### T. Model tiers (item 2)

| # | Finding | Evidence |
| --- | --- | --- |
| T1 | `tier()` gives Opus max to complexity 2–4 on every scaled stage; `FIXED` holds `game-spec` at max | `scripts/roadmap.py:158-169`, `:42`; `D-118`, `D-136` |
| T2 | 43 of the 46 remaining session stages run at Opus max (8 of 42 after this pass and its two merges); only `grc` and `g10` (c 1 S, high) and `c3`'s `rc-checklist` (xhigh) do not; no remaining stop qualifies for Fable | `scripts/roadmap.py catalog` at `12ccd36`; the moves table in step 3 |
| T3 | Plan sessions on Opus (Claude Code transcripts, deduplicated by message): max (9 sessions) medians 199,282 output tokens, 45.1 model-minutes, 4.0 s per call, 165 tool calls; xhigh (4) 128,974 tokens, 35.0 min, 3.15 s, 139 calls. Max costs about 1.5× the tokens and 1.3× the model time; the groups did different plans (xhigh M32–M34 and M42, max M35–M47), so this bounds cost, not quality | `scratchpad/effort.py` over `~/.claude/projects/-home-joker-Projects-Tungsten/*.jsonl`; register rows `implementation-plan.md:299-308` |
| T4 | Run sessions on Opus: max (10) medians 270,066 output tokens, 6.0 s per call; xhigh (4) 251,466 tokens, 5.15 s. Every run stop in both groups was a perf verdict or an owner question (M39, M40, M41, M43, M44 rows), never a reasoning failure that more effort would have caught | same; `implementation-plan.md:299-304` |

### W. Agent workflow audit (item 3)

| # | Finding | Evidence |
| --- | --- | --- |
| W1 | The README's owned fields go stale at every merge (First open stop and Stop state change when `origin/main` gains the release), so `just repo-check` fails on a freshly merged branch: it exits 1 now on `m3`/`ready` against `w16a`/`todo`. Before the block arrived in 0.61, 14 sessions (2026-10-04 to 10-06) opened with "First bring the README … in line with the tree" for the hand-written lines | `docs/plans/1.0/README.md:8-11`; `scripts/check-repo.py:372-396`; transcripts |
| W2 | The release chain runs `just repo-check` right after `just release-cut`, which moves the Workspace field, and nothing refreshes the block in between; `roadmap.py summary` only prints | `docs/releases.md:16-19`; `scripts/release.py:303-306`; `roadmap.py:398-401` |
| W3 | Register rows promise post-merge edits nobody makes ("the squash commit and PR filled in after the merge", 0.56 and 0.63, both merged) | `implementation-plan.md:305-306` |
| W4 | `plan_file` drops the archive folder, so `status` prints "… not in the tree" for every released stop | `scripts/roadmap.py:280-289`; `roadmap.py status` output |
| W5 | Dead Step 0 machinery (Step 0 released in 0.42) | `state.sh:172-182`; `tungsten-next/SKILL.md:12`, `:25` |
| W6 | Dead `game-spec` machinery (the frame-loop gate was signed on 2026-10-06) | `prompts.md:26`, Flow `:53`; `roadmap.py:42`, `:44`, `:48`, `:199-201`; `SKILL.md:49`; `implementation-plan.md:436` |
| W7 | §3's "cut with no tag" row keys on a register row, which off-roadmap releases (0.57–0.60, this one) lack | `tungsten-next/SKILL.md:40` |
| W8 | The render rules cite "the visual test from the root `AGENTS.md`", which has none since `D-140` moved the matrix | `crates/tungsten-render/AGENTS.md:31` |
| W9 | tungsten-wgpu's pipeline list lacks `ui.rs` (M46's `UiPipeline`) and names `text.rs` for a GPU half that lives in `text/gpu.rs` | `.claude/skills/tungsten-wgpu/SKILL.md:13` |
| W10 | Agent setup calls cargo-shear's `#[path]` test-module warnings false positives; M45 listed them under `ignored-paths` and `just udeps` passes | `docs/agent-setup.md:58`; `implementation-plan.md:306` |
| W11 | Validation lists `just visual` as benchmark, post-stack and transition comparisons; it also runs example 05's UI regression | `docs/validation.md:57`; `justfile:49` |
| W12 | The LLM index has 7 of 12,288 bytes left; its "Committing plan work" row repeats `AGENTS.md`'s hard rule | `just ctx`; `docs/LLM_INDEX.md:97` |
| W13 | `just repo-check` prints 46 notes in its 49 lines (5,652 B) on every run | `python3 -B scripts/check-repo.py` output |
| W14 | The memory index is 15,893 B and loads every session; 10 memories are stale or repeat the repo, and `plan_size_critique_limit.md` is wrong (`--force` passes 60,000 characters, tested with a fake `codex`) | `memory/MEMORY.md`; the memory files |
| W15 | Workflow's header is a 1.4 KB revision chain, and §8 still proposes Step 0's tooling, which landed in 0.42 | `workflow.md:10`, `:112-123` |
| W16 | The AI-workflow research plan has stayed a draft since 2026-10-11 (proposals 4–9 unapproved), at 33 KB; the 1.0 README links its release-metadata table | `docs/plans/ai-development-workflow.md:3`; `docs/plans/1.0/README.md:16` |
| W17 | The implementation plan is 87 KB, and the Read tool stops at line 287 of 482. Its Revisions list (42 entries) repeats git history, and register cells repeat CHANGELOG (M44's row is 1.7 KB) | `implementation-plan.md:299`, `:438-481` |

### R. Roadmap pull-forward (item 4)

Checked against `roadmap.json`, `implementation-plan.md` §3–§6 and §10, w01–w16, `criteria.md` and `backlog-1.x.md`.

| # | Finding | Evidence |
| --- | --- | --- |
| R1 | W5a (gamepad) sits after M4 and M5, yet M4's owner check is "a playthrough of the fixture's menus on keyboard and gamepad", with binding-aware prompts. W5a needs only W1 M0b, which landed in 0.62 | `implementation-plan.md:171`, `:150`, `:319`; `w01-ui-text-suite.md:418` |
| R2 | W11c needs only W11b's atomic write. Both are level B, c 2 M, in one workstream and one user-folder module, so they can run as one candidate | `implementation-plan.md:145`, `:152`, `:168`, `:175`; `w11-shipping-basics.md` steps 2–3 |
| R3 | W1 WL and W5a are both level B, c 2 M, touch different files (the overlay's world-label batch; the input bridge and action map), and are adjacent once R1 moves W5a: a `D-108` pair | `implementation-plan.md:169`, `:173` |
| R4 | W14b's `new` and `package` need only landed work, and only `check` needs W16b. Splitting was considered and not taken: one more stop, and RC-A7 needs all three | `w14-tooling.md:50` |
| R5 | `D-103`'s hiding rows have no candidate and default to W4b in Phase 7 | `w04-api-freeze.md:103` |
| R6 | Deprecated surface waits for W4b: eleven arity-named queries, `DeltaTime` and three `InspectorState` row functions, with 31 `#[allow(deprecated)]` sites in engine code, tests and a bench. No example or template uses them | `crates/tungsten-core/src/ecs/world.rs:313-609`; `crates/tungsten-core/src/time.rs:37`; `crates/tungsten/src/inspector.rs:75-102` |
| R7 | W4b's `missing_docs` is 3,213 undocumented public items: core 893, render 1,113, umbrella 1,207 (`rtk proxy cargo clippy -p <crate> --lib -- -W missing_docs`) | clippy at `12ccd36` |
| R8 | The acceptance game's repository was due when W15a landed (0.52); none exists beside the checkout and the gap log is empty, so no Phase 6 plan can cite a real gap | `acceptance-game.md` step 4 and Gap log; `/home/joker/Projects` |
| R9 | The definition gate left the other graduations for "as their first milestone nears" (W5, W6, W7, W8, W10, W13, W16), but no flow schedules them. Plans may already cite `criteria.md` before graduation, so a graduation only moves text | `implementation-plan.md:281`; `docs/plans/1.0/README.md:62`; `prompts.md` Flow `:53` |

### B. Bloat (item 5)

| # | Item | Evidence | This pass |
| --- | --- | --- | --- |
| B1 | Approval and same-session machinery | A1–A5 | Removed |
| B2 | `game-spec`, `GATE_ONLY` and plan mode | W6, A5 | Removed |
| B3 | Step 0 block | W5 | Removed |
| B4 | The AI-workflow research plan | W16 | Archived; proposals 4–9 carried in §9 |
| B5 | Implementation plan revisions and register narratives | W17 | Trimmed to facts; CHANGELOG and archived plans keep the detail |
| B6 | Workflow header chain and §8 | W15 | Trimmed |
| B7 | Memory | W14 | Index rebuilt around hubs; stale files deleted |
| B8 | `graduate` stage and prompt | R9 | Removed (**mine**, R-c) |
| B9 | Index committing row; check notes | W12, W13 | Removed; notes behind `--notes` |
| B10 | Deprecated API, `LightUbo::byte_size`, the arity half of `tuple_query_bench.rs` | R6; `docs/known-issues.md` | Follow-up stop (public API) |
| B11 | `just bench-build` beside `ci`'s dev-profile bench build; perf docs' "Left open by" proposal lists; implementation plan §8–§9 (cited history) | `justfile`; `docs/perf/benchmarks.md`; `implementation-plan.md:240-283` | Follow-ups |

### C. Critique (item 6)

| # | Finding | Evidence |
| --- | --- | --- |
| C1 | The critic model is `gpt-6.1-sol`; the repo never names it, and step 7 and §9 only say "another model family" and "its model family" | `~/.local/bin/critique:50`; `~/.claude/skills/critique/SKILL.md:3`, `:10`; milestone `SKILL.md:23`; `skeleton.md:83` |
| C2 | Effort: xhigh by default, and `--effort max` only when a plan stage recommends Fable, which no remaining stop does | `critique:51`; milestone `SKILL.md:23`; T2 |
| C3 | At most five findings, and 11 of 25 repo-mode Tungsten critiques returned exactly five | `critic.md:43`, `:57`; transcripts (`scratchpad/critiques.py`) |
| C4 | Repo mode is told to "read only what a finding needs … do not survey the repository" | `critic-repo.md:16` |
| C5 | Codex runs Astra at its 272,000-token default; the cache lists `max_context_window` 872,000, and Codex 0.160 reads `model_context_window` | `~/.codex/models_cache.json` (`gpt-6-astra`); Codex binary strings |
| C6 | Size guard: 10,000 characters soft, 50,000 hard. Every milestone plan passes the soft limit, so every plan prompt carries "(--force is confirmed for the plan file)" | `critique:52-53`; `prompts.md:16-17` |
| C7 | Repo-mode Codex loads the repository's `AGENTS.md`. OpenAI's Astra guidance says the model "can be more sensitive to instructions contained in skills and other files, such as `AGENTS.md`" and to "make the priority of user instructions and skills explicit"; `critic-repo.md` addresses only builds and tests | `critic-repo.md:17`; [Using GPT-6](https://developers.openai.com/api/docs/guides/latest-model) |

## 2. Recommendations of my own (mine)

- **R-a. Records written at the cut stay true through the merge.** The README's owned fields count a cut or tagged stop as closed, so the merge changes nothing they show, and `just release-cut` rewrites the block. Register rows record the cut; `origin/main` proves the merge, and nobody edits a row afterwards. This removes W1–W3's records-lag session after each release. Decision 152.
- **R-b. Level C runs critique their own API.** Level C keeps the Opus max plan tier and its §2 API sketch. In place of the owner's review after step 1, the run critiques step 1's `api/` snapshot and template diff with `critique --repo --from`, then records and checks each finding as the plan's are.
- **R-c. No more graduation sessions.** Plans cite their criteria section until a workstream graduates; gates graduate nothing. That removes up to seven sessions (R9).
- **R-d. The critic gets the plan's sources.** In repo mode the critic checks every claim against the files, sections, decisions and commands the artifact names, reading whole where a claim needs it. The repository's `AGENTS.md` and skills become context, outranked by the critic prompt.
- **R-e. Quiet checks.** `check-repo.py` prints errors and a one-line note count; `--notes` lists them (W13, research proposal 8).
- **R-f (owner).** Start the acceptance game's repository now from `templates/basic` on the `v0.66.0` tag, so the Phase 6 plans meet real gaps (R8). It needs a GitHub repository, which is publication.
- **R-g (follow-up stops).** Pull W4b's cost forward: a `missing_docs` ratchet in the release checks, so the count of 3,213 cannot grow; a level A rustdoc stop before C1, splittable per crate; and the deprecated-surface removal and `D-103`'s hiding rows in the Phase 6 QA pass (R5–R7). Each is listed in §9.

## 3. Steps

Each step copies the repository files it names to the scratchpad first; a failed check restores them with plain `cp` and `cmp` (milestone skill, Running steps). User-level files are backed up to `~/.claude/skills/critique/backup-20261013/` first.

### Step 1: The critic

- **Files:** `~/.local/bin/critique`, `~/.config/ai/prompts/critic.md`, `~/.config/ai/prompts/critic-repo.md`, `~/.claude/skills/critique/SKILL.md`; `.claude/skills/tungsten-milestone/SKILL.md` (step 7), `skeleton.md` (§9).
- **Change:** Default model `gpt-6-astra`, with the header, help and footer to match. Pass `-c model_context_window=<max>` read from `~/.codex/models_cache.json` for the chosen model, and skip it when the cache has none. Raise the size guard to 60,000 characters soft and 250,000 hard. Raise the cap to ten findings. Rewrite repo-mode reading per R-d, with the critic prompt outranking the repository's `AGENTS.md` and skills (C7). In `SKILL.md`, name the model, update the timings, and accept a calling skill's own `--force` confirmation. In the repo, step 7 names the critic and its effort, confirms `--force`, and drops the Fable-only `--effort max` rule; §9's reviewer line names the model. Default effort `max` (Q5); `ultra` is refused, since its sub-agents need persisted sessions and the run is ephemeral. Then run this plan's critique (§8) as a plain `critique --repo`.
- **Done-when:** `critique --help | rg 'gpt-6-astra'` → a match; a fake-`codex` run (`PATH=<scratch>/fakebin:$PATH`) records `-m gpt-6-astra` and `-c model_context_window=872000` in its argv, exits 0 on a 60,000-character input without `--force`, and exits 2 on a 260,000-character one; `bash -n ~/.local/bin/critique` → exit 0; a plain `critique --raw --repo --out <scratch>/c1.md docs/plans/project-health-pass.md` → exit 0, footer `critique: model gpt-6-astra, …`.
- **Moves:** nothing in the engine.

### Step 2: Decisions 150, 151 and 152

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`.
- **Change:** With tungsten-decision, invoked without arguments. **150**: no owner-approval stops on the road to 1.0. Plan and QA-plan sessions end with the run prompt; level C runs in one session and critiques its step 1 API diff (R-b); gates run in auto mode, take each open question's stated default and sign their own records; plan → run status comes from the plan's header and evidence rows; graduation sessions end (R-c); publication stays the owner's. Amends `D-109`, `D-118` and `D-140`. **151**: Opus 5.5 max only where a stage's judgment is final and no command checks it: level C plans and gate sessions at complexity 2–4. Every other stage that max covered takes xhigh, and the Fable rule stands. Supersedes `D-136`'s Opus max clause and `D-118`'s complexity 2–3 max clause. **152**: records written at the cut stay true through the merge (R-a); it amends no decision and extends `D-109`. Each old entry gets its marker line, and both index rows change.
- **Done-when:** `rg -c '^## D-15[012] ' DECISIONS.md` → 3; `rg -c '^\| \`D-15[012]\` ' docs/DECISION_INDEX.md` → 3; `rg -n '^\*\*(Amended|Superseded) by D-15' DECISIONS.md` → markers on `D-109`, `D-118` (two), `D-136` and `D-140`; `cargo test -p tungsten-core --test decision_index --locked -q` → ok.

### Step 3: Roadmap tooling

- **Files:** `scripts/roadmap.py`, `scripts/test-roadmap.py`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `.claude/skills/tungsten-next/state.sh`, `justfile`.
- **Change:** `roadmap.py` loses `KEEPS`, `PLAN_MODE`, `GATE_ONLY` and the `game-spec` and `graduate` entries. `recommend` takes the stage key into `tier(stop, key)` (decision 151) and returns auto mode everywhere. `plan_progress` returns `(started, steps, evidence)`, where started means the header status is `in progress` or `done`, or an evidence row exists, so a plan reads `plan` until its run starts. `plan_file` keeps archived links as archive paths, and their "why" reads "plan archived" (W4). `summary_fields` skips stops whose status is `ready`, `review`, `done` or `skip` when it picks the first open stop, and `summary --write` replaces the README block (R-a). Flow labels: one `Release` row for every level. Tests follow, and one test checks the rule over the repository's whole catalog. `check-repo.py` prints notes only with `--notes`, otherwise their count (R-e). `state.sh` drops the Step 0 block, prints `started`/`critique` in place of `approved`, and names gate records only. `just release-cut` runs `python3 -B scripts/roadmap.py summary --write` after the cut.
- **Done-when:** `python3 -B scripts/test-roadmap.py` → OK; `python3 -B scripts/test-check-repo.py` → OK; `python3 -B scripts/roadmap.py status` → no "not in the tree" for released stops, current stop `w16a`; `python3 -B scripts/roadmap.py catalog <scratch>/catalog.json` → every remaining stop's recommendation matches the table; `bash -n .claude/skills/tungsten-next/state.sh` and `shellcheck scripts/*.sh` → clean; `python3 -B scripts/check-repo.py | wc -l` → under 10 lines when it passes.

Tier moves for the remaining stops (decision 151; xhigh unless noted, all Opus 5.5):

| Stop | Level, ratings | `plan` or `qa-plan` | `run` (`rc-fix` for `c3`) | Stays at max |
| --- | --- | --- | --- | --- |
| `w16a`, `w13ab`, `w16b`, `w13cd`, `bc`, `w14b` | C, c 2–3 | stays max | max → xhigh | plan |
| `w6a`, `w11b` (with W11c), `wl` (with W5a), `m4`, `m5`, `w6bc`, `w12b` | B, c 2–3 | max → xhigh | max → xhigh | — |
| `qa6` | A, c 2 L | max → xhigh | max → xhigh | — |
| `c1`, `w4b`, `c2`, `w79` | unset until the feature gate cards them | max → xhigh | max → xhigh | — (a level C card moves the plan back) |
| `c3` | c 2 M | `rc-checklist` stays xhigh | max → xhigh | — |
| `gfeat`, `gfreeze` | gates, c 2 S | — | — | gate |
| `grc`, `g10` | gates, c 1 S | — | — | high, unchanged |

### Step 4: Prompts and skills

- **Files:** `.claude/skills/tungsten-next/{SKILL.md,prompts.md,roadmap.md,page.html}`, `.claude/skills/tungsten-milestone/{SKILL.md,skeleton.md}`.
- **Change:** `prompts.md`: the plan and QA-plan prompts end with the run prompt and drop the `--force` clause. `approve`, `answer`, `run-c1`, `run-c-rest`, `game-spec` and `graduate` go. The gate prompt takes defaults and signs the record. `review-fixes` and `records-lag` no longer ask for a stop. The Flow rows become Release, QA pass, Gate, Experiments and Release candidates, and the NoMachine and "why same" text follows. tungsten-next `SKILL.md`: §1–§4 lose the approval, Step 0, game-spec and same-session rows; "cut with no tag" keys on the workspace version and the tag (W7); a plan with no critique record and no session working is a plan session to resume. `roadmap.md`: statuses, ticks and `rec` without `keeps`/`if_new`. `page.html`: status label `plan` "planned", level labels, no `keeps`/`if_new`, roles, footer. Milestone skill: step 7 and 8 per steps 1 and 3, Running steps without the approval line, level C with R-b. `skeleton.md`: no approval line; §2's level C note; §4 states that open questions take their defaults.
- **Done-when:** `rg -n -i 'approv|run-c1|run-c-rest|game-spec|if_new|keeps' .claude/skills/tungsten-next .claude/skills/tungsten-milestone` → no workflow hit (owner approval of the GitHub pull request excepted); `python3 -B scripts/roadmap.py catalog <scratch>/catalog.json` → builds; `node --check` on `page.html`'s script → exit 0.

### Step 5: Program records and the pull-forward

- **Files:** `docs/plans/README.md`, `docs/plans/1.0/{README.md,workflow.md,implementation-plan.md,roadmap.json,criteria.md,w04-api-freeze.md,w11-shipping-basics.md,w14-tooling.md,w16-prefabs.md}`.
- **Change:** Workflow §1–§4, §6, §8 and §9 per decisions 150–152: levels keep their meaning and drop the owner touchpoints; the header and §8 are trimmed (W15). Plan routes (`docs/plans/README.md:11`). The implementation plan: the Phase 6 cards' Owner-only cells drop "API review" and "Plan approval" (Phase 5's keep them as history); §2's owner-present line and §11's sign-off line follow decision 150. The register moves W5a ahead of M4 as the pair "W1 WL with W5a" (R1, R3) and merges "W11b and W11c" (R2); §4's Needs table, §6's pairs paragraph and the card notes follow. Released rows shrink to their release facts (W3, W17), and the Revisions list becomes one line pointing to git history (W17). `roadmap.json`, edited textually with its rows matching the register in order: `w11c` folds into `w11b`, and `w5a` into `wl`; the `api` and `decide` roles go; q2 and q22 get defaults a session can take (A6). Criteria §10's Q2 and Q22 match. W11 and W14 lose "answered before the plan is approved", W16 loses its API review, W04 follow-up 1 points to §9. The 1.0 README's owned block comes from `summary --write`, and its link to the archived research plan goes.
- **Done-when:** `python3 -B scripts/check-repo.py` → 0 errors (the register and catalog agree in order, and the owned fields match); `python3 -B scripts/roadmap.py status` → `w5a` and `w11c` gone, `wl` and `w11b` in their new places; `rg -n -i 'approv|API review' docs/plans/1.0/workflow.md docs/plans/README.md` → only the pull-request approval and the historical record; `wc -c docs/plans/1.0/implementation-plan.md` → under 70,000.

### Step 6: Agent docs

- **Files:** `AGENTS.md`, `crates/tungsten-render/AGENTS.md`, `.claude/skills/tungsten-wgpu/SKILL.md`, `docs/agent-setup.md`, `docs/validation.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/ai-development-workflow.md`.
- **Change:** Fix W8–W12 and A7's lines in `AGENTS.md`, the render rules, the wgpu skill, agent setup, validation and the index; the index row for the roadmap names decisions 150–152. Archive the research plan to `docs/plans/archive/ai-development-workflow.md` with its status set to `superseded`: its relative links gain one `../` for the new depth before the move, since agents never open the archive afterwards. Proposals 4–9 go to §9. Nothing active links to it after step 5.
- **Done-when:** `just ctx` → OK, with the index under its 12 KiB budget; `python3 -B scripts/check-repo.py` → 0 errors; `rg -n 'root .AGENTS.md.' crates/tungsten-render/AGENTS.md` → no "visual test" claim; `test -e docs/plans/ai-development-workflow.md` → exit 1.

### Step 7: Memory

- **Files:** `/home/joker/.claude/projects/-home-joker-Projects-Tungsten/memory/` (outside the repository; backed up to the scratchpad first).
- **Change:** Delete stale or duplicate memories: `project_model`, `feedback_git_denied_patch_handoff`, `milestone_branch_upstream_trap`, `feedback_release_handoff_commands`, `p2_pass_arrival_pass_held`, `dense_pile_step_bound_handoff`, `ball_pit_extract_handoff`, `glyph_path_prototype_handoff`, `roadmap_artifact`, `build_link_io_bound` and the wrong `plan_size_critique_limit`. Merge the two commit-message memories. Add three hub memories (perf captures and probes, plan writing, releases) that link their files, and rebuild `MEMORY.md` as one short line per top-level memory and hub. Fix stale lines in kept files.
- **Done-when:** `wc -c MEMORY.md` → under 4,000 B; every `[[link]]` in the folder names an existing memory's `name:` (a scratch script prints none missing); every file in the folder is reachable from `MEMORY.md` directly or through a hub.

### Step 8: The roadmap page

- **Files:** `.claude/skills/tungsten-next/page.html` (edited in step 4); the artifact `EyM2iTgnnActfbMBZKcav9`.
- **Change:** Run the page's stub render test (catalog and status JSON, headless Chromium at 1000 and 760 px), read the live artifact, then republish `page.html` to the same URL without `capabilities`.
- **Done-when:** both screenshots render the `w16a` card with its stages and no `approve` stage; the publish result names the same URL.

### Step 9: Release 0.66

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the README's owned block, this plan (moved to `docs/plans/archive/`).
- **Change:** Fill this step's evidence row, set `status: done` and archive the plan. Then tungsten-finalize and tungsten-release: one `[Unreleased]` entry, DESIGN's status line, `just release-cut 0.66.0` once, the release checks once after it, `just release-preflight 0.66.0 --repo JosephGibson/Tungsten --message 'Update 0.66: project-health pass'`. Finally `/tungsten-next`, which syncs the roadmap artifact and gives the 0.67 prompt.
- **Done-when:** `docs/releases.md` step 1's chain → every check passes (quoted in the report, since the plan is archived before the cut); `python3 -B scripts/roadmap.py status` → clean, current stop `w16a`; the preflight prints the command block and the post-merge block; `/tungsten-next` shows "Next: ship" with W16a's plan prompt after it, no `approve` stage, and the Roadmap line reports the catalog rewritten.
- **Ends with:** the changed-file list, the command block, the post-merge block and the next prompt (W16a's plan, 0.67).

## 4. Open questions

The owner reviews these on their own time. Each takes its default.

| # | Question | Default |
| --- | --- | --- |
| Q1 | Who signs a gate record when no owner is present? | The gate session, once every item has passed or taken its default: `Sign-off: Signed <date> by the gate session; open questions at their defaults`. `roadmap.py`'s parser is unchanged |
| Q2 | Does level C still mean anything? | Yes: new public API. It gets the Opus max plan, an API sketch in §2 and the step 1 diff critique (R-b), and it still rules out `D-108` pairs; it no longer stops |
| Q3 | Graduation? | Ends (R-c): plans cite the criteria section; the skeleton workstream files keep their placement |
| Q4 | The register moves (R1–R3)? | Taken: W5a ahead of M4 paired with W1 WL, W11b and W11c as one candidate. The 25 stops left after M3 become 23 |
| Q5 | The critic's default effort | `max`. The planned Astra comparison hit the plan's usage limit, so the evidence is the 25 repo-mode Sol critiques in the transcripts: the 7 at max averaged 1.9 HIGH findings and 4.4 findings a run, the 18 at xhigh 0.6 and 3.8, at 398 s against 363 s on average. Severity is the critic's own label, and the max runs were on larger plans |
| Q6 | The findings cap | Ten |
| Q7 | q22 and q2 defaults | q22: nothing beyond ground check, coyote time, jump buffer and variable jump; slopes, one-way platforms and ladders go to the backlog. q2: no crates.io publication for 1.0 (W9c and RC-D5 struck at the freeze gate) |
| Q8 | The research plan's proposals 4–9 | Carried to §9: 8 is done here (R-e); 4, 5, 6, 7 and 9 stay proposals |

## 5. Decisions expected

150: no owner-approval stops on the road to 1.0; amends `D-109`, `D-118` and `D-140`. 151: Opus 5.5 max only for level C plans and gates; supersedes the max clauses of `D-118` and `D-136`. 152: records written at the cut stay true through the merge; extends `D-109`, amends no decision. These are the next free IDs at `12ccd36`; step 2 checks again.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post, transition and UI regressions, and the `api/` snapshots. No step touches engine code, assets or shaders; `git diff --stat -- crates examples templates assets api` → empty at the release.

## 7. Stop conditions

- A done-when check fails: fix it within the step's files, or restore them and mark the step skipped, and carry on with steps that do not depend on it. The release needs steps 2–6.
- The critique changes a done-when check or the step order: a second round on the revised plan (milestone step 7). There is no third.
- A file outside the list needs a change: make it if the step cannot pass without it, and list it in the evidence row.
- `just smoke` or `just template-check` cannot reach the display or the disk floor: the release report records it as not run, and the hand-off says so. The pass changes no runtime path.

## 8. Critique

Round 1, first attempt, 2026-10-13 11:05–11:14 ADT, at `12ccd36` plus this plan: three parallel `--repo` runs (plain at the then-default xhigh, `--effort max`, `--effort ultra`) all exited 1 on the ChatGPT plan's usage limit ("try again at 4:11 PM"); the ultra run had already failed to spawn a sub-agent ("collab spawn failed … no rollout found for thread id").

Not run before the release: `critique: codex exec failed (exit 1, usage limit reached)`, "You’ve hit your usage limit … try again at 4:11 PM". The owner asked for the release at 13:15 ADT, before the reset, so the done-when's plain `critique --repo` run is not met at the cut. A copy of this plan, saved to the session's scratchpad before archiving, takes that run after the reset; its findings go to 0.67. A fresh-context review of the diff ran in its place as a check, not as the critique (Evidence log, Review).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-13 | Partly done: the plan's critique is owed before step 9 (§8) | `critique --help` names gpt-6-astra, default effort max; fake `codex`: argv `-m gpt-6-astra` and `-c model_context_window=872000`, a 60,000-character input exit 0 without `--force`, 260,000 exit 2, no cache → `context default`; real Astra run on `w05-input.md`: exit 0, 47.0 s, context 872000; `--effort ultra` → exit 1 with its reason; `bash -n` and shellcheck clean | `~/.claude/skills/critique/backup-20261013/`; scratchpad `guard/`, `crit-r1/` |
| 2 | 2026-10-13 | Done | `rg -c '^## D-15[012] ' DECISIONS.md` → 3; index rows → 3; markers: `D-109` (150), `D-118` (150, 151), `D-136` (151), `D-140` (150); `cargo test -p tungsten-core --test decision_index` → 2 passed | `DECISIONS.md`, `docs/DECISION_INDEX.md` |
| 3 | 2026-10-13 | Done, with `prompts.md`'s Flow table written here (the new `roadmap.py` reads it) and `docs/releases.md`'s cut sentence, both outside the step's list | `test-roadmap.py` 17 OK; `test-check-repo.py` 47 OK; `roadmap.py status`: 0 "not in the tree", released stops "plan archived", current `w16a`; catalog 44,340 B (hash `79d1d736…`); `check-repo.py` 3 lines (2 errors, the stale README fields step 5 owns; 47 notes counted); `state.sh` and `shellcheck scripts/*.sh` clean; critique-record detection right on four fixtures | `scripts/{roadmap,test-roadmap,check-repo,test-check-repo}.py`, `state.sh`, `justfile` |
| 4 | 2026-10-13 | Done (`prompts.md` was written in step 3) | `rg -n -i 'approv\|run-c1\|run-c-rest\|game-spec\|if_new\|keeps'` over both skills → only the pull-request approval, "no session stops for the owner's approval" and "no step keeps another's session"; catalog builds (44,340 B); `node --check` on the page's script → ok; `just ctx` → OK | `.claude/skills/tungsten-next/{SKILL,prompts,roadmap}.md`, `page.html`, `.claude/skills/tungsten-milestone/{SKILL,skeleton}.md` |
| 5 | 2026-10-13 | Done | `check-repo.py` → 0 errors (register and catalog agree in order; owned fields `w16a`/`todo`); `roadmap.py status`: `w5a` and `w11c` gone, `w11b` ("W11b and W11c") and `wl` ("W1 WL with W5a") in place, 23 stops left before 1.0, from 25; `rg -i 'approv\|API review'` over workflow and plan routes → only "no stop" statements; implementation plan 87,148 → 64,350 B, workflow 22,428 → 19,542 B | `docs/plans/README.md`, `docs/plans/1.0/{README,workflow,implementation-plan,criteria,w04-api-freeze,w11-shipping-basics,w14-tooling,w16-prefabs}.md`, `roadmap.json` |
| 6 | 2026-10-13 | Done, with one link outside the list: `CHANGELOG.md`'s 0.61 entry now points at the archived research plan, as earlier entries do for theirs | `check-agent-context.py` → OK (root 5,197 B, index 12,240 B, from 12,281); `check-repo.py` → 0 errors; the render rules point at `just smoke`/`just visual`; `docs/plans/ai-development-workflow.md` gone (14 links deepened, status superseded); six follow-ups in known issues | `AGENTS.md`, `crates/tungsten-render/AGENTS.md`, `.claude/skills/tungsten-wgpu/SKILL.md`, `docs/{agent-setup,validation,LLM_INDEX,known-issues}.md`, `docs/plans/archive/ai-development-workflow.md` |
| 7 | 2026-10-13 | Done | `MEMORY.md` 15,893 → 1,421 B (10 lines: 7 memories, 3 hubs of 36, 10 and 6 notes); 12 files deleted (10 stale or repo-duplicated, the wrong `plan_size_critique_limit`, and `feedback_commit_one_line` merged into `feedback_no_commit_attribution`); every `name:` is its filename in kebab case; 0 unresolved `[[links]]` (15 before); every file reachable from the index; rtk's upper-cased first character recorded | the memory folder; backup in scratchpad `memory-backup/` |
| 8 | 2026-10-13 | Done | Stub render (Helium 147 headless, catalog and status from the tree): W16a shows Plan (auto, Opus 5.5, max) → Run (auto, Opus 5.5, xhigh) → Ship, no `approve`, Phase 6 at 15 stops; 1000 and 760 px screenshots clean; the live page (734 lines, read whole) equalled the repo copy before this pass's edits; republished as version 23 with its `db` capability carried forward | scratchpad `page-test/`; https://claude.ai/artifact/EyM2iTgnnActfbMBZKcav9 |
| Review | 2026-10-13 | 20 findings from a fresh-context reviewer (the owner's practice for unattended audit passes); all checked against the tree, each fixed or corrected | Fixed: `state.sh`'s critique record misread the skeleton's placeholder row and a no-concerns record (six fixtures now right, the awk taken from the script); a started run left the owned Stop state stale (the run now writes the block); level C's API critique missed untracked snapshots, `just api` and `--force`; q3 had no takeable default; D-151's "every plan needed a second round" and its 35-of-46 count; D-152's evidence and its marker on `D-109` (it amends nothing); the index's blank line, which had split its table since 0.61; the D-140 row's takeaway; a per-stop tier test that a gate's carding would break (now a rule over the catalog); the cut's failure path and hints; two gate criteria that needed the owner; same-session leftovers; three stale memory lines; four stops' hands-on roles; this plan's counts (46 notes, 42 revisions, 33 deprecated sites, 25 → 23 stops); `scripts/__pycache__` removed. `check-repo.py` → 0 errors; `test-roadmap.py` 17 OK; `test-check-repo.py` OK; decision-index test 2 passed | `scratchpad/statetest/`, `api-probe.diff` |
| 9 | 2026-10-13 | Release 0.66, done-when not fully met: the plan's critique did not run (usage limit, §8) | Pre-cut: `just check` (1,474 passed), `just script-test`, `just ctx`, `just repo-check`, `just api-check` (3 crates current) and `just notices` (205 and 178 crates) passed before the review fixes; `script-test`, `check-repo.py` (0 errors) and the roadmap tests passed after them. The plan is archived before the cut, so the cut, the release checks, the preflight and `/tungsten-next` are reported in the hand-off | `docs/plans/archive/project-health-pass.md` |

## 9. Follow-ups

- **Follow-up stop (public API): retire deprecated surface early.** Remove the eleven arity-named queries, `DeltaTime`, the three `InspectorState` row functions and `LightUbo::byte_size`, with their tests and `tuple_query_bench.rs`'s arity half, each with a ledger row. Proposed home: the Phase 6 QA pass, ahead of W4b (R6, B10).
- **Follow-up stop (public API): `D-103`'s hiding rows.** A curated render re-export, a game-facing handle or `doc(hidden)` tier for the `Renderer` methods that use `wgpu` types, and engine-owned types in place of `winit` ones. Proposed home: the Phase 6 QA pass (R5).
- **Follow-up stop: rustdoc before the freeze.** A `missing_docs` ratchet (per-crate counts of 893, 1,113 and 1,207 that may only fall) in the release checks, then a level A rustdoc stop before C1, splittable per crate (R7).
- **Owner: the acceptance game's repository** (R-f, R8).
- **Feature gate:** C1's new benchmark rows (UI, text-heavy screens, hierarchy, kit systems) could land with the stops that add those features rather than all at C1.
- **Research proposals carried from the archived AI-workflow plan:** 4 (deny-list bans and a decision-row check), 5 (a released-tag consumer check and an early RC-A9 pilot), 6 (refreshed client discovery and exact-command permissions for `just api`, `api-check`, `notices`, `template-check` and `level-check`), 7 (behaviour and visual evidence for feature reviews), 9 (`.editorconfig` and `.gitattributes`).
- **Bloat not trimmed:** `just bench-build` against `ci`'s dev-profile bench build; the perf docs' "Left open by" proposal lists; implementation plan §8–§9, which many documents cite by number.
