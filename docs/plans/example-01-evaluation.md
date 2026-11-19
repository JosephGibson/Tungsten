# 7.13g's evaluation, first pass and records (example-01-evaluation) — in progress

- **status:** in progress
- **goal:** Phase 7's 7.13g (§2.8 of [phase 7](phase-7.md)), its first plan: the redo's §8 tasks (an enemy, a room, a sound, a status with an interaction, a broken prefab's fix) in example 01 and in a `tungsten new` copy, and the paper designs' weekly tasks LW3 (a lit path), SF1 (a room template) and SF4 (a status band's decay) in the copy, each run once by a fresh Opus 5.5 agent at xhigh (`D-237`) and counted (turns, errors, files touched, wrong turns); every friction point a verified seam with a destination; the seams whose fix is documentation fixed; each of 7.13's parked seams given its disposition; 7.13g's second plan named.
- **non-goals:**
  - The code fixes, in 01, the template, the kit, the engine or the CLI, the rerun after them and the owner's acceptance of 01: **7.13g's fixes and rerun**, written when this plan closes (§2.1).
  - A second pass of any task here, or a task beyond the thirteen.
  - Agent guidance's shape (task guides, shipped skills, the instruction files' structure): 7.16, researched against this evaluation's records (the redo's 7.16 row).
  - The genre jam register's triage, which is the owner's; its rows are read for dedupe only.
  - A committed evaluation harness: it stays local in the run folder, as H2's and the jam's did (Q2).
  - A sitting, a benchmark row or a counter guard (`D-226`): the evaluation measures agents, not frames.
  - Any edit by an evaluation agent to this repository: the agents work in copies outside it (§2.2).
- **files to touch:** Steps 1–4: this plan only (the harness, the workspaces and the builds are local, §2.2). Step 5: the files §2.8's doc seams name, inside §2.8's list. Step 6: this plan, `docs/plans/phase-7.md`. Step 7: this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `docs/known-issues.md`, `DECISIONS.md` and `docs/DECISION_INDEX.md` (the protocol's decision).
- **ordered steps:** (1) the harness, the fixtures and the base's checks; (2) the copy's eight runs; (3) 01's five runs; (4) the seams, merged and verified; (5) the doc seams' fixes; (6) the parked seams' dispositions; (7) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; thirteen runs with a verdict and four counts each (§2.7); every friction row of the thirteen logs in one seam row or one entry of §2.12; the doc seams landed and reviewed; every parked seam in §2.9 fixed or left with its reason; `just check` passes and the demo (Step 7) works; the plan deleted with its citations, `example-01-evaluation` out of Now and 7.13g's second plan named in phase 7's §2.8.

Item `example-01-evaluation` in [ROADMAP.md](../../ROADMAP.md), phase 7's 7.13g, from the redo's 7.13 row and §8 ([roadmap redo](roadmap-redo.md)) and phase 7's Step 13, §2.6 and §2.8. Written 2026-11-18 at `a2da8184`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- 7.13a–7.13f have landed (`D-271`–`D-290`): every 7.13 row of phase 7's §2.2 is terminal (A15). 7.13g is what is left of Step 13: the evaluation, its seams' fixes and the rerun, the owner's acceptance of 01, and 7.13's parked seams (2026-11-13, the owner's choice).
- §8 (the redo): each run is a fresh agent on Opus 5.5 at xhigh, the sound task included (`D-237`), counting turns, errors, files touched and wrong turns; each friction point is a seam; the 7.13 checkpoint passes when every friction point is recorded and the rerun after the fixes completes the tasks. 8.5 and 8.6 repeat it, so the counts must compare: one frame, one harness, one counting rule (§2.3–§2.7).
- The precedents: H2's skill evaluations (`perf-runs/20261029-h2-skills/`: answers leak from a live tree, so copies; premises probed before launch; graders vary, so scripted checks beside them) and the genre jam (`docs/plans/genre-jam.md`: five fresh builders on `tungsten new`, a friction log per game, a merge and three verifiers). This plan keeps the jam's row shape so its rows and these dedupe.
- The split (§2.1): a step names its files, and a code fix's files come from its seam, so the code fixes, the rerun and the acceptance are the second plan's.
- Fresh means headless `claude -p` sessions started in workspaces outside this repository (A11, A12), each launch moving its workspace's memory folder aside first (A19): no memory, no engine `AGENTS.md` above a template copy.
- Later items expect: the second plan to start from this plan's seams table and §2.9 as its last commit holds them; 7.16 to read §2.11; 8.5 and 8.6 to reuse the harness, the frame and the briefs (Q2).

## 1. Audit

Read at `a2da8184`. `W` is `/home/joker/tungsten-eval/` (the agents' side), `R` is `perf-runs/713g-eval/` (the harness's side, ignored), `B` is `target/713g-eval/` (the builds), `E` is `examples/01_platformer/`, `T` is `templates/basic/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The CLI's verbs are `new`, `check`, `package`, `run`, `describe`, `render`, `schema` and `level` (`new`, `apply`, `query`, `map`, `describe`, `check`, `generate`, `render`); none of §8's scaffolds (`new enemy`, `new status`), `explain` or `save dump` exists, so the tasks meet the CLI as it is | `tools/cli/src/args.rs:208-210`, `:573-625`; `target/debug/tungsten --help` |
| A2 | `tungsten new` makes a game from the template the CLI carries, its engine lines pinned to the CLI's release tag; a copy before a tag needs its engine lines pointed at a path and the engine's cargo config, as the jam's pin script does | `tungsten --help` ("with the engine pinned to this command line's release tag"); `docs/getting-started.md:25`; `target/jam/pin-game.sh` |
| A3 | The template's `AGENTS.md` holds 12,285 of its 12,288 bytes (`D-183`): a doc fix there cuts what it adds. Its rules: the public API only, and `cargo test`, clippy and `tungsten check` before a commit | `wc -c T/AGENTS.md`; `T/AGENTS.md:45`, `:54` |
| A4 | The template sample is top-down (the player's `top_down_mover`) with no room, tileset, tilemap or light in its manifest, so the copy's room, LW3 and SF1 tasks build its first room and tileset (the levels guide); generators register with `App::register_generator` and `tungsten check --seeds` sweeps them | `T/assets/prefabs/player.json`; `jq keys T/assets/manifest.json`; `docs/getting-started.md:64`, `:215`; `docs/levels.md:135-137` |
| A5 | The light query exists: `light_at` sums every `Transform` and `Light`, no ambient; `light_level` is its luma | `crates/tungsten-core/src/lighting.rs:95`, `:111` |
| A6 | The kit's statuses: an effect's `intensity` with a `decay` a second, `bands` granting tags from an `at`, stacks with `expire` `clear` or `remove_one`; a decay that changes after a quiet time is not among them, which is SF4's friction to find, not to fix here | `api/tungsten-kit-components.txt:614`, `:624-625`, `:652`, `:664-666` |
| A7 | The copy's broken-prefab fixture: the sample's patrolling enemy (`T/assets/prefabs/enemy_patrol.json`, sprite `enemy`, an `oscillator` of amplitude `[280, 0]`), whose swing the sample's tests pin | `T/assets/prefabs/enemy_patrol.json`; `T/tests/game/sample.rs:1311-1316` |
| A8 | 01's assets are generator output: prefabs and data come from `E/tools/content/` (the crawler's from `content/prefabs/crawler.json`); `just level-check` runs `generate.py --check`, `tungsten check` and the tools' tests, and needs Pillow 12.3.0, which this machine's Python lacks | `E/tools/content/prefabs/crawler.json`; `justfile:99-103`; `E/tools/README.md:4`, `:12`; memory `level-check-pillow-venv` |
| A9 | 01's art and sound families are locked (`D-223`, 20 families, `sounds` and `music` among them); unlocking is the owner's call by `--unlock NAME`, so 01's sound brief grants it | `jq keys E/tools/locks.json`; `E/tools/README.md:74-80` |
| A10 | 01 has no `AGENTS.md`; the repository's sends a feature to plan first and commits with a push after each, so 01's frame names the medium route and a workspace without a remote | `AGENTS.md:49`, `:60`; `ls E` |
| A11 | Headless sessions: the npm `claude` refuses `--effort xhigh` and the VS Code extension's binary takes it; a child drops the parent's `CLAUDE*` variables; a `-p` session's background subagents stop ten minutes after its turn unless `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0`; the result JSON carries `num_turns` and `total_cost_usd` | `perf-runs/713-chain/spawn.sh:14-28`, its `README.md`; memory `headless-session-chain` |
| A12 | Claude Code reads `CLAUDE.md` from a session's folder up, so a copy under this repository would load the engine's `AGENTS.md`; neither `/home/joker` nor `/home` holds one | `ls /home/joker/CLAUDE.md /home/CLAUDE.md` (no such file) |
| A13 | The 01 workspace is a whole-repository export: 77,148,160 bytes, 7,365 files; its path-dependent build is warm only at one path, and an export's files carry the commit's time, so a reused build must be told which files the last run changed | `git archive HEAD \| wc -c`; `git ls-files \| wc -l`; `docs/known-issues.md:149` (a sitting's `git archive` base reusing a newer build) |
| A14 | Disk 91 GB free of 468 GB (80 %); 12 cores | `df -h /`, `nproc` |
| A15 | No phase 7 row is open at 7.13 | `rg -n '^\| [^\|]+ \| [^\|]+ \| 7\.13 \| open \|' docs/plans/phase-7.md` prints nothing |
| A16 | The parked seams: the known issues 7.13's plans recorded and the Later lines they wrote whose gap keeps 01's own code (§2.9) | `docs/known-issues.md:18`, `:31-33`, `:52`, `:67-68`, `:77`, `:125`, `:127`, `:134-135`; `ROADMAP.md:14-35`, `:48-59` |
| A17 | The genre jam's frame and row shape (an `<id>.<n>` per row; kinds stuck, guessed, kit-should-own, bug, doc-gap, rule-conflict, wrong-turn; a game's lines a fix would delete) and its 25 rows and 11 entries at `cad63fa5` | `docs/plans/genre-jam.md:29-44`, its §3–§4; `perf-runs/20261107-big-session/jam/briefs.md` |
| A18 | `tungsten new` names the game after its folder and refuses a name with anything but lowercase letters, digits, `-` and `_` | `tools/cli/src/new.rs:96-100`, `:136-145` |
| A19 | A headless session loads its project's memory (the chain's sessions did), kept per workspace path, so a path used again carries what an earlier session saved; a session's subagents write transcripts of their own under its folder, which the chain's spend script walks | `perf-runs/713-chain/README.md:29`; `perf-runs/713-chain/spend.py:3`, `:21` |

## 2. Design

### 2.1 Two plans

7.13g in one plan would carry a fixes step whose files no one can name before the seams exist, which the skill refuses (a step names its files). So 7.13g splits by its own order (phase 7's §6): this plan runs the thirteen tasks, records and verifies every seam, lands the doc seams and gives each parked seam its disposition; **7.13g's fixes and rerun**, written when this closes from §2.11, §2.12 and §2.9 as its last commit holds them, lands the code fixes (the parked seams' and the seams' with destination `7.13`), reruns all thirteen briefs under the same harness and frame, and ends with the owner's acceptance of 01. It may split again by the same rule.

### 2.2 Workspaces

| Path | What | Built by |
| --- | --- | --- |
| `W/engine/` | The copy's engine: `git archive` of the base minus this plan, a git repository of one commit; read-only for every agent | `eval.py setup` |
| `W/copy-<name>/` | A copy run's game, `copy-enemy` for `copy.enemy` (A18): `tungsten new` from the CLI built off `W/engine`, its engine lines pinned to `W/engine/crates/tungsten` and `W/engine/.cargo/config.toml` copied (`target/jam/pin-game.sh`'s two edits, without its `[workspace]` table, which only a game under this repository needed), the fixture applied, then `git init` and one commit | `eval.py workspace copy.<name>` |
| `W/e01/` | 01's workspace: `git archive` of the base minus this plan, the fixture applied, one commit; the same path for every 01 run, so its build stays warm | `eval.py workspace e01.<task>` |
| `W/runs/<pass>/<name>-a<attempt>/` | A finished workspace, moved there after its checks and scan; attempt 1, or 2 after a void (§2.7) | `eval.py launch` |
| `W/logs/<pass>/<task>-a<attempt>.md`, `W/start/<pass>/<task>-a<attempt>` | The agent's friction log; its start stamp (`date +%s`); a retry never sees the attempt before it | the agent; `eval.py launch` |
| `R/harness/` | `eval.py` (`setup`, `workspace`, `fixture`, `launch`, `check`, `scan`, `status`, `seams`, `probe`, `base`), `frame-copy.md`, `frame-e01.md`, `briefs.json` (§2.5), `fixtures/` | Step 1 |
| `R/runs/<pass>/<task>/a<attempt>/` | `result.json` and `stderr` (the session's), `checks.json`, `counts.json`, `diff.txt`, a copy of the log and of the transcript | `eval.py launch` |
| `R/seams/` | `merged.md` (the merge's rows and entries), `verify-1.md`, `verify-2.md` | Step 4 |
| `B/copy/`, `B/e01/` | One build folder for all copies (the engine export's path is one, so its crates build once) and one for 01's path | `eval.py setup` |
| `B/venv/` | The harness's Pillow venv (`E/tools/requirements.txt`) for its own `just level-check`; never on an agent's `PATH` | `eval.py setup` |

- **Warm-up, not measured:** `setup` builds the CLI from `W/engine` into `B/copy`, then in a scratch copy `cargo build`, `cargo test --no-run`, clippy and the template's `cargo doc -p tungsten -p tungsten-core -p tungsten-kit --no-deps`, and deletes the copy; for 01, `cargo build -p tungsten-cli -p example-01-platformer` and 01's `cargo test --no-run` and clippy in `W/e01/`. Both passes start from warm builds, so their counts compare.
- **01's reset between runs:** move the finished workspace to `W/runs/`, export a fresh one at `W/e01/`, and `touch -c` (never creating a file) every path the finished run changed or deleted that the fresh tree holds (`git diff --name-only` against its baseline), so cargo sees them newer than its outputs (A13's stale-build trap); then `git -C W/e01 status --porcelain` must print nothing. When `touch -c` touched a Rust source, `build.rs` or `Cargo.toml` (a restored input of a target that still exists), the next `cargo test -p example-01-platformer --no-run` (the library, the binary and the tests) must print that crate's `Compiling` line; a run that changed only runtime-read assets, or only added files (a test target among them) that the reset removed, needs none.
- **Memory, before every launch:** the workspace's project folder under `~/.claude/projects/` (its path with each `/` as `-`) has its `memory/` moved into the record of the run that last used that path (`R/runs/<pass>/<task>/a<attempt>/memory/`, what that agent saved), and `launch` refuses to start while it exists (A19); Step 1's probe proves it with a planted sentinel. Every copy and every 01 run, both passes, starts so.
- **What an agent's shell has:** `CARGO_TARGET_DIR` (`B/copy` or `B/e01`), `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=4`, `PATH` with that build's `debug/` first (its `tungsten`), and `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0`; every `CLAUDE*` variable of the run session dropped first (A11).

### 2.3 The driver

`eval.py launch <task>` starts one session and blocks until it ends: from the workspace (`W/copy-<name>/` for `copy.<name>`, or `W/e01/`, by the same task-to-folder map `workspace` uses), `timeout 7200` on the VS Code extension's binary with `-p <frame + brief>`, `--session-id <uuid>`, `--model claude-opus-5-5`, `--effort xhigh`, `--permission-mode bypassPermissions`, `--max-budget-usd 100`, `--output-format json`, stdout to `result.json`, stderr to `stderr`, stdin from `/dev/null`. No appended system prompt: the frame is the prompt. Then `check` and `scan` run, the transcripts (`~/.claude/projects/<folder>/<uuid>.jsonl` and every descendant under `<folder>/<uuid>/`, its subagents' and any workflow's, A19) and the log are copied to `R/runs/`, and the workspace moves to `W/runs/`.

- **Concurrency:** the copy's runs at most three at once, each a background Bash task of the run session; 01's runs one at a time. The second plan's rerun keeps both (Q5).
- **Caps:** 2 hours (the frame asks the agent to wrap up at 1 h 45 min) and USD 100 (the result's `total_cost_usd`, an API-price figure) a run; a capped run's verdict is `failed` with the cap named, never retried (it is the measurement).
- **Gates before each launch:** at least 30 GB free on `/` (`df`), and the week's spend under 90 % by `perf-runs/713-chain/spend.py`'s transcript sums; a closed gate stops the step (§6).

### 2.4 The frame

One text for every run of a workspace kind, kept in `R/harness/frame-copy.md` and `frame-e01.md`; `<…>` are filled per run. The copy's:

> You are working on `<game>`, a game made from the Tungsten engine's template. Do the task at the end for the game's owner, then stop.
>
> Setup, given by this session and not friction: every shell already has `CARGO_TARGET_DIR`, `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS` and a `PATH` whose `tungsten` is the command line of the engine at `W/engine`. The build folder is shared with other runs: never `cargo clean` it or delete from it. The game's folder is a git repository of one commit with no remote: commit as its rules say, never push. Nobody will answer a question: where you would ask, take the default you would propose, log it as `guessed`, and go on.
>
> Read: the game's folder, its `AGENTS.md`, `W/engine/docs/getting-started.md` and the docs it links (in `W/engine`), rustdoc (`cargo doc` as `AGENTS.md` says, or the doc comments of the public items the docs lead you to), the engine paths those docs name, and the build folder. Never: `W/engine/templates/`, any other folder under `W`, anything else under `/home/joker/Projects`, or `tungsten new`.
>
> Change files only in the game's folder. No engine patches: where the engine, kit or template falls short, work around it in the game and log the workaround. Cargo runs offline; nothing needs the network.
>
> Log every point of friction when it happens, never at the end, to `<log>`: a header with your start time, then one row each: `| ID | Min | Kind | Friction | Tried | Evidence | Lines | Fix |`. ID: `<task>.<n>` from 1. Min: minutes since the seconds in `<start>` (`date +%s`). Kind: stuck (could not go on until …), guessed (went on without the docs saying), kit-should-own (game code the engine, kit or template should own), bug (the engine, kit, template or command line behaves wrong), doc-gap (the docs miss or contradict something), rule-conflict (two rules or docs disagree), wrong-turn (a path taken and backed out of, with the minutes it cost), env (this machine or this session's setup, not the engine). Evidence: `path:line` of what you read, or a command with the output lines that show it. Lines: for kit-should-own, the game's lines a fix would delete (`src/x.rs:10-60, 51 lines`); else a dash. Fix: the smallest fix and where it belongs. End the file with a summary: done or not, minutes, what changed, and the commands that show the task done with their output.
>
> Stop when the task's done-when holds, or at 1 hour 45 minutes after your start: then finish the log's summary and stop.
>
> The task: `<task text>`
>
> Done when: `<done-when>`

01's frame differs in four places: the game is "example 01 in a copy of the Tungsten engine's repository at `W/e01`", whose build folder no other run shares; Read is "anything in `W/e01` (its `AGENTS.md` says where to start) and the build folder; never any other folder under `W` or anything else under `/home/joker/Projects`"; the change rule is "only under `examples/01_platformer/`; this is a medium change (the operating rules' Medium): state its scope, files and checks, then edit; no plan file and no critique"; and the network is "only for `pip install -r examples/01_platformer/tools/requirements.txt` into a virtual environment, if you need one".

### 2.5 The thirteen briefs

Every done-when below also holds the game's own suite, and the done-when an agent gets ends with it, spelt out: in a copy `cargo test`, `cargo clippy --all-targets -- -D warnings` and `tungsten check`; in 01 `cargo test` and `tungsten check` from `E`, `cargo clippy -p example-01-platformer --all-targets -- -D warnings` and `just level-check`. A test "whose name starts with `x_`" is checked by `cargo test x_` passing with at least one test run (a filter that matches nothing exits 0, so `check` sums the `N passed` lines).

| Task | Text | Done when | `check` adds |
| --- | --- | --- | --- |
| `copy.enemy` | Add a charger: an enemy that waits until the player is within 200 px, then dashes toward where the player was at 400 px/s for 0.6 s, rests for 1 s and repeats; it has 3 health and the player's shots hurt it. Put two in each run. | A test whose name starts with `charger_` shows a charger dashing at a player in range, resting, and dying to the player's shots. | — |
| `copy.room` | Give the run a room: a walled room about 40 × 24 tiles with two inner walls, its tiles from a tileset of your own, spawned when a run starts with the player inside it. | `tungsten level check` passes; a test whose name starts with `room_` starts a run, holds the player's move right for 3 s and finds the player stopped by the room's east wall. | `tungsten level check` → 0 |
| `copy.sound` | When an enemy dies, play a short descending pop: a new sound made with the game's own sound tools. | The sound is in the manifest; a test whose name starts with `pop_` sees its cue fire at an enemy's death. | the diff names a file under `tools/` |
| `copy.status` | Add `burning`: a new fire patch gives it on touch, 1 damage every 0.5 s for 3 s; a new puddle gives `wet`, which puts `burning` out at once and keeps it off for 2 s. | A test whose name starts with `burning_` shows the ticks, the douse and the 2 s block. | — |
| `copy.prefab` | Since the last edit to the patrolling enemy's prefab, `assets/prefabs/enemy_patrol.json`, the game stops at startup and the sample's tests fail. Find what is wrong and fix the prefab. | The suite passes with the prefab fixed, not its tests. | the fixture's two values back to the base's; the diff names nothing under `tests/` |
| `copy.lw3` | Light a new path at night: a room with a walkable path about 48 tiles long and no light but its lamps, six lamps on it as objects of a lamp prefab with a point light, placed in the room file, so that no stretch of more than six tiles along the path is darker than 0.05. | `tungsten level check` passes; a test whose name starts with `lit_path_` walks the path tile by tile, samples the light the engine computes at each tile's centre and finds no run of more than six tiles under 0.05. | `tungsten level check` → 0 |
| `copy.sf1` | Add a room template, a generator `hall`: a hall about 30 × 20 tiles with two rows of cover, a pair of explosive canisters (a prefab of your own), three squad places and doors north and east. It must generate cleanly in 500 seeds. | `tungsten check --seeds 500` passes and names `hall`; a test whose name starts with `hall_` generates it at ten seeds and finds in each its two rows of cover, two canisters, three squad places and its north and east doors, and places it in a run; `scripts/hall.json` generates and places it, and `tungsten run --script scripts/hall.json` exits 0. | both commands → 0, the check's output naming `hall` |
| `copy.sf4` | Add suppression: a `suppressed` status built by near misses (a `NearMiss` game event): each adds a stack, up to five; 1.5 s after the last near miss it starts to decay, a stack a second; it grants `suppressed.cover` at 3 stacks or more and `suppressed.pinned` at 5. Units stay pinned too long: once 1.5 s pass without a near miss, the top band must drain twice as fast. | A test whose name starts with `suppression_` sends near misses and shows the build to five, the bands and the hold while near misses keep coming; from five stacks left quiet, the fifth lost 2.0 s after the last near miss (the decay's 1.5 s, then half a second); from four left quiet, the fourth lost at 2.5 s; and a stack a second below the top band. | — |
| `e01.enemy` | Add the ash beetle: a beetle that walks at half the beetle's speed with twice its health, which fire does not hurt and which leaves a patch of fire where it dies. Place two on the map where beetles already walk. | A test whose name starts with `ash_beetle_` shows its speed and health, fire not hurting it and the fire at its death. | — |
| `e01.room` | Add a small chamber to the level: a pocket about 12 × 6 tiles under the clearing's east end, reached by a drop through a gap, holding a crate and a lantern, which the wizard can leave by jumping. | A test whose name starts with `chamber_` puts the wizard in the chamber and shows it leaving by jumps. | — |
| `e01.sound` | Add a sound for `<candidate>`, made with example 01's sound tools; its sounds family is locked, and you may unlock it for this (`--unlock sounds`, `tools/README.md`). | A test whose name starts with `sound_` shows its cue firing at `<the candidate's moment>`. | the diff names `E/tools/sfx.py` or `E/tools/sounds.json` |
| `e01.status` | Add `tarred`: a new tar pool (an existing pool's art, tinted dark) tars what touches it for 5 s; a tarred body moves at half speed, and fire on a tarred body burns twice as long. | A test whose name starts with `tarred_` shows the slow and the doubled burning. | — |
| `e01.prefab` | Since the last edit to the frost crawler's prefab, example 01 no longer starts and its tests fail. Find what is wrong and fix it. | The suite passes with the prefab's source fixed, not its tests. | the fixture's two values back; nothing under `E/tests/` changed |

- **`e01.sound`'s candidate,** chosen at Step 1: the first of (a) a bat's wings while it flies, (b) a crate scraping as it slides on the ground, (c) the lift starting, that has no sound in 01 at the base (`E/tools/sounds.json` and the cue rules searched by name), its moment written into the brief. Step 1 chose (a): the bat's sounds are its dive and its screech alone, and the brief's moment is "while a bat flies".
- **Premises,** probed at Step 1 in the base workspaces (H2's lesson): the sample's shots and patrol enemy, its `enemy` sprite and `tools/sounds.py`; 01's beetle as a walker that fire hurts, the clearing's east end with ground under it, a pool prop to tint, fire's burning on bodies, the crawler's prefab source. A failed premise rewrites its brief once, the change recorded in the evidence row before any launch.
- **A brief is the same text in the second plan's rerun,** with its candidate and its premises' fixes.

### 2.6 Fixtures

Each prefab task's workspace is broken twice before its baseline commit, one break `tungsten check` reports and one only a test catches, so the fix needs both tools:

- **`copy.prefab`:** in `assets/prefabs/enemy_patrol.json`, the sprite's `asset_id` `enemy` → `enmy`, and the oscillator's position amplitude `[280, 0]` → `[28, 0]`. Step 1 proves both on a scratch copy: `tungsten check` exits 1 naming the prefab's sprite, and with that break undone `cargo test` fails at the sample's swing test (`T/tests/game/sample.rs:1316`). A candidate that does not behave so is replaced from: a misspelt component name; `health.max` 0. Step 1 found that no test catches the amplitude (the swing test is an upper bound, `T/tests/game/sample.rs:1361`) and that `tungsten check` reports both fallbacks, so the test's break is `health.max` 3 → 30, which `sample::every_enemy_stands_at_the_run_start` alone catches (evidence row 1).
- **`e01.prefab`:** in `E/tools/content/prefabs/crawler.json`, then the generator run (with `B/venv`) so `generate.py --check` passes on the broken tree: its sprite or animation ID misspelt by one letter, and one value of its components that one of 01's tests catches, chosen at Step 1 by changing each candidate in turn (its health's `max`, its walk speed, its touch damage) on a scratch export until `cargo test` fails, that test's name recorded. Step 1's: the sprite `ex10_crawler_walk_0` → `ex10_crawer_walk_0`, and the walk speed (`run_speed`) 210 → 21, which `data::every_restated_constant_matches_its_data` catches (`health.max` 60 → 6 broke no test).
- The baseline is the workspace's only commit, so its history shows no break; the frame forbids the engine's `templates/` and `tungsten new`, which hold the copy's original, and the scan voids a run that reads them (§2.7).

### 2.7 Counting and verdicts

`eval.py scan <task>` writes `counts.json` from the result JSON, the run's transcripts (the session's and every descendant's, summed, the subagents' share beside) and the workspace:

| Count | Rule |
| --- | --- |
| Turns | One rule for every run, capped or not: the session transcript's assistant messages, deduplicated by message ID (as `spend.py` dedupes, A19), with each descendant's counted apart and summed beside; the result's `num_turns` recorded next to it when the run has a result |
| Tool calls | `tool_use` blocks in every transcript of the run, and those writing the log apart (work calls = all − log calls) |
| Errors | Tool results flagged `is_error` in every transcript of the run, split by pattern: compile (`error[E`, `could not compile`), test (`test result: FAILED`, `panicked at`), check (`tungsten check` or `level check` exiting non-zero), other |
| Files touched | Files changed, added or deleted against the baseline (`git diff --name-only <baseline>` and untracked files, without virtual environments, `__pycache__` and build output); the edited paths from the transcript beside it |
| Wrong turns | The log's `wrong-turn` rows the verifier confirms, plus the unlogged ones it finds; the scan's reverted files (edited back to the baseline, or added then deleted) shown beside |
| Also | Minutes (stamp to result), `total_cost_usd`, tokens in, out and cached, subagents started |

- **Verdict:** `complete` when every `check` passes, the diff stays in the game (copy: the copy, with `W/engine` unchanged by `git status`; 01: `examples/01_platformer/` alone) and the verifier (Step 4) finds the named test shows the brief's claim; `partial` when the suite builds and passes but a task check fails, the verifier finds the test does not show the claim, or the diff patched outside the game (that patch a seam); `failed` when the suite fails, the build fails, or a cap ended the run; `void` when any of the run's transcripts reads or runs what the frame forbids (another workspace or log, this repository outside `B`, which holds `R`, the engine's `templates/`, `tungsten new` in a copy, `cargo clean` on a shared build folder), rerun once as attempt 2, with a fresh workspace and its own log, record and archive paths (§2.2), a second void `failed`; the task's verdict names the attempt it rests on. Steps 2 and 3 write the scripted verdict; Step 4's verifiers may lower a `complete` to `partial`.
- `eval.py status [--pass N] [--set copy|e01]` prints one line per task, verdict and the four counts first, and exits 0 only when each task named has a verdict other than `void`.

### 2.8 Seams

- **Row:** the jam's (A17), one row per distinct friction: `| ID | Friction | Kind | Tasks | Evidence | Lines | Lacking | Fix | Destination |`. IDs `EV-<n>` in rank order (tasks hit, then Lines, then kind: stuck, bug, guessed, kit-should-own, doc-gap, rule-conflict, wrong-turn, env). Tasks: the count, then each run's log rows. Evidence: a workspace path under `W/runs/1/` or a base path, with `:line`, or a command with its output. Lacking ends with `none: <pattern>` (no match in `crates/*/src` or `api/*.txt`), the item that falls short, or `has: <item>` when the engine has it and the docs did not lead there.
- **Dedupe first:** a friction an existing ID covers (an S-, K-, P7-, P8-, PA- or J- row, a known issue, a Later line) becomes an entry in §2.12 with its new evidence, not a row; an entry on a parked seam flips it (§2.9).
- **Destination,** by phase 7's §2.1 rule, one of:
  - `doc`: the docs state something wrong or leave out what a task needed, and the fix is text alone in the files below; landed by Step 5.
  - `7.13`: a seam that kept a task from completing (the rerun must complete it), a bug in 01, the template, the kit, the engine or the CLI, or a gap in 01's or the template's own code or data: the second plan.
  - `7.14` or `7.15` for a cost; `7.16` for agent guidance (a task guide, a shipped skill, an instruction file's shape); a Phase 8 step or `Later` for a kit or engine item no Phase 7 step builds (`D-172`'s second genre); `known-issues` for a bug past one plan step; `env` for this machine or the harness, recorded with no engine change.
- **The doc seams' files:** `docs/*.md` but the frozen records; `T/AGENTS.md` (cut what it adds, A3), `T/README.md`, `T/tools/README.md`; `E/README.md`, `E/tools/README.md`; doc comments (`///`, `//!`) in `crates/*/src/`, `T/src/` and `E/src/`; `scripts/missing-docs.json` when new rustdoc lowers it. A kit description string is code, so its seam goes to `7.13`. The repository's own `AGENTS.md`, `docs/LLM_INDEX.md` (at its cap) and every skill are 7.16's. A doc seam needing any other file goes to `7.13`.

### 2.9 The parked seams

7.13's parked seams (phase 7's §2.8) with their default dispositions. **Flip rule:** a §2.12 entry on a seam left here, or the owner's play at the second plan's acceptance meeting one, flips it to fixed by the second plan; its phase 7 row, if `Later`, goes back to `open` at 7.13 with the seam's ID (§2.1 there).

| # | Seam (home) | Disposition | Reason |
| --- | --- | --- | --- |
| PK1 | The map's loose props named for saves, S-125's 01 part (`docs/known-issues.md:32`) | Fixed: second plan | Owner-parked; its next work first finds what a named entity costs the pit's window |
| PK2 | A set-aside top-down mover's first step saved (`docs/known-issues.md:33`) | Fixed: second plan | Owner-parked; a flyer saved frozen thaws 0.93 px off the uninterrupted run |
| PK3 | A troll's thrown crate breaking inside the area zone sensor (`docs/known-issues.md:125`) | Fixed: second plan | Owner-parked; a bug in play, 01's `touches` counting a sensor |
| PK4 | 01's unread `DamageApplied` cursor (`docs/known-issues.md:125`) | Fixed: second plan | Dead code in 01 |
| PK5 | 01's unread `Moves::aim` (`docs/known-issues.md:134`) | Fixed: second plan | Dead code in 01; a saved field takes its migration |
| PK6 | 01's run resources removed one by one at its exit, S-163's 01 site (`docs/known-issues.md:135`) | Fixed: second plan | 01's own code on an engine part it already has (`StateStack::own_resource`) |
| PK7 | The shell's module doc counting seven names in its seam (`docs/known-issues.md:135`) | Fixed: Step 5 | A word in `T/src/shell/mod.rs:4` and `E/src/shell/mod.rs:4` |
| PK8 | 01's `src/save.rs` module doc naming the tomes (`ROADMAP.md:54`) | Fixed: Step 5 | A doc line (`E/src/save.rs:10`) |
| PK9 | Kit items with one game's recipe (`ROADMAP.md`): the cone (`:14`), a place without a use (`:16`), held casts' events (`:17`), the throw record (`:20`), a shield's grant (`:21`'s first half), a cap per stimulus (`:22`), looks for piles (`:23`), looks with frames and timed tints (`:24`, S-1), looks' particle lists (`:25`), emission from the box (`:26`), 01's hits by side and armour (`:28`, S-47's rest), a projectile's saved emitter (`:32`), an effect request's duration (`:33`), a timed end's outputs (`:35`), shake, post and ambient data per area (`:51`, S-85's rest), a recursive despawn at once (`:53`, S-177's rest) | Left in Later | A kit item whose second genre's recipe (`D-172`) no Phase 7 step builds; 01's code stays its one user |
| PK10 | Art (`ROADMAP.md`): the unused `ex10_fire_missile_left` frames (`:18`), the slam's area strike with its ring (`:29`: no ring sprite in the locked effects family), the lantern's embers as its prefab's child (`:52`) | Left in Later | An asset session's, the owner's call (`D-223`) |
| PK11 | Costs: chemistry's clone a run (`ROADMAP.md:31`); the empty-archetype walk, `character_animator`'s per-frame work and `e01-stress` in a quick sitting (`docs/known-issues.md:52`, `:134`, `:127`) | Left in their homes | 7.14 measures the clone; the walk is 7.14's C4 (phase 7's §2.5); the animator is 9.2's lever list; a full sitting measures `e01-stress` |
| PK12 | Conditional ones nothing has shown: a tween channel's easing, one receiver's parts in turn, a stimulus's priority and the listener at the wizard's death (`ROADMAP.md:15`, `:27`, `:34`, `:48`); an impact on a tile floor and `burst_cleanup`'s order (`docs/known-issues.md:68`, `:125`, `D-277`) | Left in their homes | Each line's own condition (a prefab, a test or play that needs it) has not been met |
| PK13 | 01's refactors with no behaviour (`ROADMAP.md`): `PreviousVelocity` onto `step_velocity` (`:30`), anchors as data (`:58`), constants restating data (`:59`, which `E/tests/game/data.rs` holds to their files) | Left in Later | No behaviour moves; 01's refresh in Phase 8 takes them with the code it changes |
| PK14 | Engine and template items no game meets: exact sampling at every zoom, a modal screen's opener by key, a headless display request and the settings' game section accessor (`ROADMAP.md:19`, `:55`, `:56`, `:57`; the opener also `docs/known-issues.md:67`); two captures on one frame, a restore's zeroed velocity, and the kit's ends at the frame's flush, which 01 meets by ending its victims at once (`docs/known-issues.md:18`, `:31`, `:77`) | Left in their homes | No task or play has met them; each line names its fix for the game that does |
| PK15 | One genre's kit items (`ROADMAP.md`): a tome's air-jump cost as an effect (`:21`'s second half), the lift's hold while a rider's carry is blocked (`:49`, S-132), the controller set out of its own ground and the lift turned back over a body (`:50`) | Left in Later | Admissible on 01's use alone (`D-172` as `D-215` amended it: an item whose job is one genre's), but each only replaces 01 code that works and no task or play has needed it; 01's refresh in Phase 8 takes them, or a §2.12 entry flips them |

### 2.10 Budget

`spend.py` read about 19.2 % of the week used at writing (the window from 11-16 08:00Z, resetting 11-23 08:00Z). At its rate (8.8M weighted units per 1 %; memory `claude-weekly-meter-cache`), an hour-long xhigh session costs about 0.5–1.5 %, so the thirteen runs about 6–20 %, the merge and the verifiers about 2 %, and this run's own session about 3 %. Step 2's first wave measures it with `spend.py`; the 90 % gate stops launches past that (§6). The rerun costs the same again in the second plan.

### 2.11 Seams table

Filled at Step 4.

### 2.12 Registered elsewhere

Filled at Step 4.

## 3. Steps

### Step 1: The harness, the fixtures and the base's checks

- **Files:** this plan (`status: in progress`, the evidence row). Local: `R/harness/`, `W/`, `B/`.
- **Change:** write `eval.py` and the frames and `briefs.json` from §2.2–§2.7, reusing H2's scan (`perf-runs/20261029-h2-skills/scan.py`) and the jam's pin edits; `setup` the exports, the CLI, the warm builds and `B/venv`; probe the premises and pick `e01.sound`'s candidate (§2.5); build and prove the fixtures (§2.6); run the base's checks; run one probe session through the driver (model `claude-haiku-5-5`, effort low, a brief that lists the copy's files and stops) and delete its workspace.
- **Done-when:** `python3 -B R/harness/eval.py base` → exit 0, printing pass for the copy's `cargo test`, clippy and `tungsten check`, 01's `cargo test` (its pass count), `tungsten check`, clippy and `just level-check`, and both fixtures' expected failures (the check's message and the failing test's name); `python3 -B R/harness/eval.py probe` → exit 0 (its result's `subtype` `success`, its transcript found, `scan` counting at least one tool call, an `env` in its transcript showing the child's own `CLAUDE_CODE_SESSION_ID`, not the run session's, and a sentinel planted in its workspace's memory folder before the launch moved aside and absent from its transcripts); `git status --porcelain` → this plan alone.
- **Commit:** `docs(plans): start 7.13g's evaluation, its harness and base checks`
- **Moves:** none.

### Step 2: The copy's eight runs

- **Files:** this plan (evidence row).
- **Change:** for each of `copy.enemy`, `copy.room`, `copy.sound`, `copy.status`, `copy.prefab`, `copy.lw3`, `copy.sf1`, `copy.sf4`: `eval.py workspace`, then `eval.py launch` as a background task, at most three at once (§2.3), each gate read first; a void run once more.
- **Done-when:** `python3 -B R/harness/eval.py status --pass 1 --set copy` → exit 0, eight lines, each verdict `complete`, `partial` or `failed` with its four counts; `test -s W/logs/1/<task>-a<attempt>.md` → exit 0 for each of the eight, at the attempt its verdict names; `git -C W/engine status --porcelain` → prints nothing.
- **Commit:** `docs(plans): record the copy's eight evaluation runs`
- **Moves:** none.

### Step 3: 01's five runs

- **Files:** this plan (evidence row).
- **Change:** `e01.enemy`, `e01.room`, `e01.sound`, `e01.status`, `e01.prefab`, one at a time at `W/e01/`, each reset by §2.2's rule before the next, each gate read first.
- **Done-when:** `python3 -B R/harness/eval.py status --pass 1 --set e01` → exit 0, five lines as Step 2's; `test -s W/logs/1/<task>-a<attempt>.md` → exit 0 for each of the five, at the attempt its verdict names; after each reset `git -C W/e01 status --porcelain` → prints nothing, and where the reset touched a restored Rust source, `build.rs` or `Cargo.toml`, `cargo test -p example-01-platformer --no-run` printed that crate's `Compiling` line (quoted).
- **Commit:** `docs(plans): record 01's five evaluation runs`
- **Moves:** none.

### Step 4: The seams, merged and verified

- **Files:** this plan (§2.11, §2.12, the evidence row).
- **Change:** a fresh-context subagent merges the thirteen logs and the scans' error lists into §2.8's rows, deduped against the registers at the base (the seam register, the genre jam register, phase 7's §2.2–§2.7, the paper designs' and the ARPG design's rows, `docs/known-issues.md`, `ROADMAP.md`'s Later), with every log row mapped to one row or entry; two fresh-context verifiers, each half the rows, check every claim against the workspaces and the base (as the jam's did) and judge each `complete` run's named test against its brief; their corrections taken; the verdicts and wrong turns final in `R/runs/1/`; §2.11 and §2.12 written.
- **Done-when:** `python3 -B R/harness/eval.py seams --check docs/plans/example-01-evaluation.md` → exit 0: every log row ID appears once in §2.11's Tasks cells or §2.12, every row has one destination of §2.8's list, and the IDs run `EV-1` to `EV-<n>` with no gap; `python3 -B R/harness/eval.py status --pass 1` → exit 0, thirteen lines, every verdict final; the verifiers' files (`R/seams/verify-1.md`, `verify-2.md`) list every row as confirmed, corrected or dropped.
- **Commit:** `docs(plans): record 7.13g's evaluation seams`
- **Moves:** none.

### Step 5: The doc seams' fixes

- **Files:** the files §2.11's `doc` rows name, inside §2.8's list; `templates/basic/src/shell/mod.rs`, `examples/01_platformer/src/shell/mod.rs` and `examples/01_platformer/src/save.rs` (PK7, PK8).
- **Change:** each `doc` row's fix and PK7's and PK8's, every fact checked against the source; then a fresh-context subagent reviews the diff against the code; its blocking findings fixed, the rest answered in the evidence row.
- **Done-when:** `just checks <the step's paths>`'s recommendations → each passes; `just ctx` → passes (the template's `AGENTS.md` within 12,288 bytes); `just repo-check` → passes; before the commit, `rtk proxy git diff --unified=0 HEAD -- '*.rs' | rg '^[+-]' | rg -v '^(\+\+\+|---) ' | rg -v '^[+-]\s*(///|//!)'` → prints nothing (only doc comment lines change in Rust files; `rtk proxy` since rtk rewrites diffs).
- **Commit:** `docs: fix the docs 7.13g's evaluation agents tripped on`
- **Moves:** `scripts/missing-docs.json` may fall.

### Step 6: The parked seams' dispositions

- **Files:** this plan (§2.9 final), `docs/plans/phase-7.md` (a flipped seam's `Later` row back to `open`).
- **Change:** apply §2.9's flip rule from §2.12; each phase 7 row a flip takes back to `open` at 7.13 with its reason (`moved from Later (<EV-n>)`).
- **Done-when:** `sed -n '/^### 2.9/,/^### 2.10/p' docs/plans/example-01-evaluation.md | rg '^\| PK[0-9]+ ' | rg -v '^\| PK[0-9]+ \| [^|]+ \| (Fixed|Left)'` → prints nothing; `rg -n '^\| [^|]+ \| [^|]+ \| 7\.13 \| open \|' docs/plans/phase-7.md` → the flipped rows the evidence row lists, no other; `just repo-check` → passes.
- **Commit:** `docs(plans): settle 7.13's parked seams`
- **Moves:** none.

### Step 7: Close

- **Files:** this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `docs/known-issues.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
- **Change:** phase 7's §2.2 gains an `EV-` row per seam (step: `7.13` for the second plan's, the destination step otherwise, `landed <sha>` for Step 5's, `closed (env)` for `env`; Notes citing this plan's §2.11 at its last commit by name and sha); its §2.8 records the split, the run's numbers and §2.9's dispositions, and names 7.13g's second plan; its evidence log takes the row (`13 (7.13), part g's first`); `known-issues` rows land in `docs/known-issues.md`; the protocol (§2.2–§2.7: fresh headless sessions outside the repository, the frame, the caps, the counts) recorded with the tungsten-decision skill under the next free ID, so 8.5 and 8.6 compare; the plan deleted and `example-01-evaluation` out of Now.
- **Done-when:** `just check` → passes; the demo: `python3 -B R/harness/eval.py status --pass 1` → exit 0, thirteen final verdicts with their counts, and `just smoke` → passes (01 starts); `rg -c '^\| EV-[0-9]+ \|' docs/plans/phase-7.md` → the count of §2.11's rows, read before the deletion; `rg -l 'example-01-evaluation\.md' docs ROADMAP.md AGENTS.md; test $? -eq 1` → exit 0; `just repo-check` → passes.
- **Ends with:** the commits, the evidence, what the owner should try (01's play, for the acceptance the second plan asks), and the next prompt: tungsten-next's `step` prompt for 7.13g's fixes and rerun.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | One plan for 7.13g? | No: two (§2.1); the second may split again by phase 7's §6 |
| Q2 | Commit the harness? | No: it stays in `R/harness/`, as H2's and the jam's, and the second plan, 8.5 and 8.6 run it from there; the frame and the briefs are also §2.4–§2.5 here, which this plan's last commit keeps; the decision (Step 7) names both |
| Q3 | Memory for the agents? | None: each launch moves its workspace's memory folder aside first (§2.2), so a path used again (01's, the rerun's) carries nothing; the owner's global `CLAUDE.md`, skills and hooks load as in any session on this machine, and friction they cause is `env` |
| Q4 | The network? | Cargo offline everywhere; in 01, pip for `E/tools/requirements.txt` alone, as its README's own line does |
| Q5 | Concurrency? | Copies three at a time on one warm build, 01 one at a time at one path; the rerun the same |
| Q6 | A capped run? | `failed`, the cap named, no retry |
| Q7 | A run that patched the engine? | `partial`; its patch is a seam |
| Q8 | A doc fix past the template's `AGENTS.md` cap? | It cuts what it adds or goes to 7.16 |
| Q9 | A friction the agent did not log but the scan or a verifier sees? | A row like any other, its evidence the transcript's line |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** the docs of Step 5 and `scripts/missing-docs.json` (down only); phase 7's rows by Steps 6 and 7.
- **Must not move:** every pin, hash, digest, reference image and API snapshot line; any code outside doc comments; this repository's tree by any evaluation agent (§2.2, checked by `git status` at each step).

## 6. Stop conditions

- A base check fails at Step 1, or the probe session fails: stop the plan; the evaluation needs a green base and a working driver.
- A gate closes before a launch (under 30 GB free, or the week's spend at 90 %): launch nothing more, wait for the runs already started to end and be checked and scanned (their caps bound them), then pause the plan, still `in progress`: the ended runs keep their records, the step's evidence row reads `paused` with the gate's reading and the runs left, committed alone, and the session ends with tungsten-next's `resume` prompt for when the gate opens again. Steps 4 to 7 wait for all thirteen runs.
- A void run voids again: `failed` (§2.7).
- A doc fix's check fails: restore that file, its row to `7.13`, and go on.
- A failed check, a stop condition or a file outside a step's list stops the step (the skill).

## 7. Critique

Round 1: `critique --raw --repo` with a focus note (the harness's freshness, the briefs' feasibility, the done-when commands, the split), xhigh, 2026-11-18, the tree at `a2da8184` with this draft (44.8k characters, under the size guard). Its first attempt, on the CLI's default `gpt-6.2-astra`, failed before any model call: `status 400: The 'gpt-6.2-astra' model is not supported when using Codex with a ChatGPT account` (5h 0 % → 0 %); it ran again on `gpt-6-astra`, the model of 7.13f's rounds the same day. Footer: `model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 423.7 s, tokens in 1400940 (cached 1305728), out 10711 (reasoning 5708), commands 25, output 491.6 KB (largest 82 KB), usage 5h 0% → 28%, week 57% → 62%`. Nine findings, two high; every one checked against the lines it cites.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | 01's one path, and a rerun's, would carry memory an earlier session saved, unseen by any read | `perf-runs/713-chain/README.md:29` (headless sessions load memory); §2.2's one 01 path | Accepted | §2.2's memory bullet: each launch moves its workspace's memory folder aside and refuses to start while it exists; Step 1's probe plants a sentinel; A19, Q3 |
| 2 | The scan reads the session's transcript alone, so a subagent's work and reads escape the counts and the void rule | `perf-runs/713-chain/spend.py:3`, `:21` (its recursive walk over subagents' transcripts) | Accepted | §2.3 copies every descendant transcript; §2.7 counts and voids over all of them, the subagents' share beside |
| 3 | `tungsten new` refuses `copy.enemy` as a folder name | `tools/cli/src/new.rs:136-145` (lowercase letters, digits, `-` and `_` only) | Accepted | copy folders `W/copy-<name>/`; A18 |
| 4 | SF4's stack-a-second loss leaves the top band before the 1.5 s rule can apply | `docs/plans/paper-designs.md:271` ("decays 1 a second after 1.5 s without"), `:312` | Accepted | the loss starts 1.5 s after the last near miss; the test shows the hold, the first loss and the top band's doubled rate against the rate below |
| 5 | The reset's `touch` creates added paths, `cargo build` builds no tests, and Step 3's check contradicts the asset exemption | `touch` creates an absent file; `cargo build` builds no integration test target; §2.2 against Step 3 | Accepted | `touch -c` on the paths the fresh tree holds, a clean `git status` after it, `cargo test --no-run` for the `Compiling` line only after a source change; Step 3's check says the same |
| 6 | SF1 passes on exit codes without proving the hall's contents | `docs/levels.md:132` (the room rules: clearance, support, links, budget, routes) | Accepted | a `hall_` test checks the contents at ten seeds and the placement; Step 4's verifiers review it as they do the others |
| 7 | PK9's second-genre reason does not cover items `D-172`'s exception admits on one genre's use | `D-172`'s `D-215` amendment (`DECISIONS.md`); `docs/plans/roadmap-redo.md:257` | Accepted in part | the one-genre items (a tome's air-jump cost, the lift's hold, the controller's set-out and the lift's turn-back) move to PK15 with their own reason; the rest are generic, where the reason holds |
| 8 | Step 5's Rust gate refuses the kit description strings §2.8 allows | §2.8's list against Step 5's gate and §5 | Accepted | description strings leave Step 5, a code seam to `7.13`; the gate and §5 stay strict |
| 9 | A closed budget or disk gate cannot reach the close, which needs thirteen verdicts | §6 against §2.7, Steps 2, 3 and 7 | Accepted | a closed gate pauses the plan `in progress`, a `paused` evidence row and the `resume` prompt; Steps 4 to 7 wait for all thirteen |

Round 2: the same CLI and model, xhigh, 2026-11-18, the plan without §7 and the evidence log (`--from "sed '/^## 7\./,/^## Fol/{/^## Fol/!d}' …"`) and a focus note naming round 1's eight changes and its one partial rejection. Footer: `model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 492.8 s, tokens in 1095041 (cached 1017344), out 15755 (reasoning 10917), commands 23, output 345.2 KB (largest 84.8 KB), usage 5h 28% → 54%, week 62% → 66%`. Six findings, all medium, all taken; no third round (the skill).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | SF4's test fixed pinning's end at the decay's start, so the doubled top rate could not shorten it | §2.5's brief: pinned at five stacks, the first loss pinned at 1.5 s | Accepted | the test's timelines from five (the fifth lost at 2.0 s) and from four (the fourth at 2.5 s) tell the quiet time from the stack's time |
| 2.2 | A capped run counted turns by another rule than a finished one | §2.7's two turn rules; `perf-runs/713-chain/spend.py:3` (dedupe by message ID) | Accepted | one rule for every run: the transcript's assistant messages deduplicated by message ID, descendants apart; `num_turns` beside |
| 2.3 | A void's retry shared its log, record and archive paths with the attempt before | §2.2's paths keyed by pass and task alone | Accepted | per-attempt paths (`-a<attempt>`, `a<attempt>/`); the verdict names its attempt; Steps 2 and 3 check that attempt's log |
| 2.4 | A pause left runs in flight unsettled | §2.3's three concurrent runs against §6's pause | Accepted | a closed gate launches nothing more and waits for the started runs to end, be checked and scanned, then pauses |
| 2.5 | The reset's `Compiling` gate fails falsely when a run only added a test target | `examples/01_platformer/Cargo.toml` (no `autotests = false`, so integration targets are discovered); §2.2's rule on any test change | Accepted | the gate applies only when `touch -c` touched a restored Rust source, `build.rs` or `Cargo.toml`; added files the reset removed need none; Step 3 the same |
| 2.6 | §2.3 still launched from `W/copy.<task>/` | §2.3 against §2.2's `W/copy-<name>/` | Accepted | §2.3 resolves the folder by `workspace`'s task-to-folder map |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-19 | landed | `eval.py setup`: `W/engine` exported at `ca678445` without this plan, one commit; Claude Code 2.1.327 copied to `B/bin/claude`, every run's binary; warm builds (s): the CLI 41; the copy's build 146, `test --no-run` 18, clippy 34, rustdoc 3, `tungsten check` 0; 01's build 160, `test --no-run` 45, clippy 44, check 1; `B/venv` Pillow 12.3.0; `B/copy` 6.8 GB, `B/e01` 7.4 GB, 76 GB free. Premises held: the copy's `bolt.json` hitbox `any: [team.dummy, team.enemy]`, damage 1, the patrol's sprite `enemy`, `tools/sounds.py` writing its WAVs; 01's beetle `run_speed` 90, `health.max` 120, `resist` fire 0.5, five in the mine (columns 285.5–336); `east_rim`, columns 72–92, ground from row 36 to 72 inside the clearing (columns 0–92); the lava basins' art (`ex10_lava_*`, `ex10_ember_pool_*`); `data/effects/burning.json` (4 s, a tick each 0.5 s); `tools/content/prefabs/crawler.json`, whose sprite `generate.py` leaves to `tungsten check` (`validate_references`). `e01.sound`: (a), its moment "while a bat flies". Fixtures (§2.6, updated): 01's `health.max` 60 → 6 → `cargo test` exit 0, 500 passed; `run_speed` 210 → 21 → exit 101, `data::every_restated_constant_matches_its_data`, chosen. The copy's amplitude `[280, 0]` → `[28, 0]` failed its premise (`cargo test` exit 0, 186 passed; the first `base` exit 1, kept in `R/harness/base-first.out`), and `tungsten check` reports both fallbacks (``unknown component `oscilator` ``; ``component `health`, field `max`: must be finite and > 0 (got 0)``), so the value break is `health.max` 3 → 30 (`tungsten check: no errors`, one test failing). `python3 -B R/harness/eval.py base` → exit 0, every line pass: the copy's `cargo test` exit 0, 186 passed; clippy exit 0; `tungsten check` exit 0; its tree clean after the builds; fixture `tungsten check` exit 1, `assets/prefabs/enemy_patrol.json: prefab 'enemy_patrol' references unknown sprite ID 'enmy'`; both breaks `cargo test` exit 101; the sprite undone, check exit 0 and `cargo test` exit 101 failing `sample::every_enemy_stands_at_the_run_start`; 01's `cargo test` exit 0, 500 passed (55 s); `tungsten check` exit 0; clippy exit 0; `just level-check` exit 0 (547 s); fixture `generate.py --check` exit 0 on the broken tree; `tungsten check` exit 1, `assets/prefabs/crawler.json: prefab 'crawler' references unknown sprite ID 'ex10_crawer_walk_0'`; both breaks `cargo test` exit 101; the sprite undone, check exit 0 and `cargo test` exit 101 failing `data::every_restated_constant_matches_its_data`. `python3 -B R/harness/eval.py probe` → exit 0: subtype `success`, transcript found, 6 tool calls counted (Bash 5, Write 1), `env` showing the child's `CLAUDE_CODE_SESSION_ID` (`dbb27529…`) and not the run session's (`eaa85c40…`), the sentinel moved aside before the launch and absent from the transcripts; Haiku 5.5 at low, 0.3 min, USD 0.009; its workspace deleted. `git status --porcelain` → this plan alone. Week 19.6 % (`spend.py`). Beyond §2.2: the harness runs `git init` in the workspaces through Python (the shell's deny list is this session's); an agent's shell also has `DISABLE_AUTOUPDATER=1` (the binary is a copy) and `BROWSER=true` (rustdoc's `--open` opens nothing on the owner's display); 01's frame keeps "never `cargo clean`", since later runs reuse its folder | `docs/plans/example-01-evaluation.md`; local: `R/harness/` (`eval.py`, `frame-copy.md`, `frame-e01.md`, `briefs.json`, `fixtures/`), `W/engine`, `B/` |
| 2 | 2026-11-19 | landed | `python3 -B R/harness/eval.py status --pass 1 --set copy` → exit 0, eight lines (verdict, turns, errors, files, wrong turns; minutes, USD at API prices): `copy.enemy a1 complete 75 3 21 0` (11.6, 4.39); `copy.room a1 complete 72 2 15 0` (8.1, 3.44); `copy.sound a1 complete 33 2 6 0` (3.4, 1.29); `copy.status a1 complete 68 2 17 0` (9.0, 3.60); `copy.prefab a1 complete 20 0 1 0` (2.2, 0.85); `copy.lw3 a1 complete 97 1 14 0` (10.4, 4.50); `copy.sf1 a1 complete 96 2 25 1` (14.5, 5.95); `copy.sf4 a1 complete 86 2 17 0` (13.2, 4.95). `test -s W/logs/1/copy.<task>-a1.md` → exit 0 for all eight; `git -C W/engine status --porcelain` → nothing. Three at once from 01:22Z, the last ended 01:48:55Z; 72.4 agent-minutes, USD 28.97, 63 log rows; every run attempt 1, none capped, no subagents, every task check passing (the `x_` tests, `tungsten level check`, the pop in the manifest and `tools/`, the prefab's `enemy` and 3 back with `tests/` untouched, `check --seeds 500` naming `hall`, `run --script scripts/hall.json`). `copy.status`'s scripted void, `rg -l … /home/joker/tungsten-eval/*.toml` (a parent cargo config sought), matched no file (W holds folders alone) and its result showed only its own `Cargo.toml:35`: reviewed false, `complete`. The scan's other `check` items are each run's `ls` of its own log folder. Every `complete` waits on Step 4's verifiers. Week 20.9 %, 66 GB free | `docs/plans/example-01-evaluation.md`; local: `R/runs/1/copy.*`, `W/runs/1/copy-*-a1`, `W/logs/1/copy.*` |
| 3 | 2026-11-19 | landed | `python3 -B R/harness/eval.py status --pass 1 --set e01` → exit 0, five lines (verdict, turns, errors, files, wrong turns; minutes, USD): `e01.enemy a1 complete 141 0 17 1` (36.6, 8.09); `e01.room a1 complete 107 2 9 1` (35.0, 5.83); `e01.sound a1 complete 81 0 17 1` (22.6, 4.07); `e01.status a1 complete 159 2 23 2` (37.9, 9.42); `e01.prefab a1 complete 49 1 2 0` (25.5, 1.72). `test -s W/logs/1/e01.<task>-a1.md` → exit 0 for all five. Resets: `git -C W/e01 status --porcelain` printed nothing after each; before `e01.enemy` nothing to touch; before `e01.room` 13 paths touched, 6 Rust, before `e01.sound` 9 and 2, before `e01.status` 16 and 5, before `e01.prefab` 17 and 4, each followed by `Compiling example-01-platformer v0.117.0 (/home/joker/tungsten-eval/e01/examples/01_platformer)`. One at a time from 01:49:58Z to 05:22:08Z; 157.6 agent-minutes, USD 29.13, 38 log rows; every run attempt 1, none capped, no subagents, no void, every task check passing (the `x_` tests, `sfx.py` and `sounds.json` in `e01.sound`'s diff, the crawler's `ex10_crawler_walk_0` and 210 back with `E/tests/` untouched); each run's checks 10 min, `just level-check` 541–549 s of it. Week 22.3 %, 60 GB free | `docs/plans/example-01-evaluation.md`; local: `R/runs/1/e01.*`, `W/runs/1/e01-*-a1`, `W/logs/1/e01.*` |

## Follow-ups
