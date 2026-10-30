# Analysis pass and seam register (analysis-pass)

- **status:** in progress
- **goal:** Redo 7.5 ([roadmap redo](roadmap-redo.md) §7.1): find every place example 01 and the template compensate for the engine, kit or template, with evidence, ranked by the code a generic fix would delete and how often the pattern recurs; write it as a verified seam register, `docs/plans/seam-register.md`; and triage each row into the [Phase 7 plan](phase-7.md)'s tables by its §2.4, so the step that builds each system takes its seams.
- **non-goals:**
  - Fixes, and edits to code, tests, tools, assets, data or `api/`: the steps that build each system make them.
  - Chasing bugs: a bug a reader meets is noted, verified and filed in known issues (§2.7).
  - Re-reporting the paper designs' or the ARPG design's rows, or a K-row without new evidence: their 01 sites go to the register's §4, not into new rows (§2.4).
  - A second reading or verification round, and reading the engine or kit beyond the claim a row makes (the brief's stop).
  - Reading 01's generated `assets/`, its `tools/content/` data, the template's assets or its `AGENTS.md` whole: they are consulted. 01's `src/shell/` is the template's bytes (`D-197`), read once in the template's share.
  - Phase 7's finite lists beyond an item's evidence cell (§2.5 there); the redo's §3, where a `+` marks what a sketch lacks; the feedback plan, the backlog and the designs, which are read only.
- **files to touch:**
  - Step 1: `docs/plans/seam-register.md` (new); this plan.
  - Step 2: this plan's evidence row; the run's folder `perf-runs/<date>-analysis-pass/` (local, gitignored).
  - Step 3: `docs/plans/seam-register.md`; this plan.
  - Step 4: `docs/plans/seam-register.md`; `docs/plans/phase-7.md` (§2.1, §2.2, §2.3, §2.5's evidence cells, §2.8's 7.10 bullet, §3's Rows lines); `ROADMAP.md` (Later); `docs/perf/benchmarks.md` (the levers for 9.2, only for a `9.2` row); this plan.
  - Step 5: this plan (deleted); `ROADMAP.md` (Now); `docs/plans/phase-7.md` (its evidence row, F3); `docs/plans/seam-register.md` (status, §5's filed marks); `docs/known-issues.md`.
- **ordered steps:** (1) frame the register; (2) read and merge; (3) verify; (4) triage; (5) close. §3 holds each.
- **done-when:** every step's done-when passes, quoted in the evidence log; phase 7's Step 4: the register and phase-7.md hold the same S-rows, each with one disposition (a step, `Later` or `9.2`); no code, test, tool, asset or data file changed since this plan's commit; `just check` passes and the demo (`just status` reading phase 7's step 4 passed) works; this plan deleted with its citations and `analysis-pass` out of Now.

Item `analysis-pass` in [ROADMAP.md](../../ROADMAP.md), from [phase-7.md](phase-7.md)'s Step 4 and the redo's 7.5 row and §7.1. Written 2026-10-30 at `e76c32f0`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs: its product is documents, so its tests are `just check` on a tree whose code it leaves alone, and its demo is the register read back through `just status` and the triage's counts (step 5).

## Context digest

- 7.2–7.4 have landed (agent tooling, the bench runner, H2), so 7.5 is phase 7's next step; it runs before 7.8 so its rows reach the steps that build them (phase-7.md §2.8). No 7.17 work exists yet (A11).
- The brief (the redo's §7.1): scope 01's `src/`, `tests/` and `tools/`, and the template; the engine and kit read only to verify; about eight readers, each reading a share whole into one row shape (pattern, `file:line`, lines, what is lacking, the generic fix, dependencies) through ten lenses; seeds; then a merge, a fresh-context check of each row and the owner's triage. Stop: every folder read once and every row verified, no second rounds; the 2026-10-14 whole-repo review is the cost lesson.
- Phase 7's §2.4 sets the IDs (`S-1`, …; Q8 there), keeps a re-reported K-row's K ID, routes each row (a Phase 7 step, 7.11, 7.13, an 8.x step, `Later`, or for performance a §2.5 item's evidence or `9.2`), and writes every row into §2.2 or §2.3; its Step 4 checks the count.
- The paper designs (§4) and the ARPG design (§15) keep registers "in the analysis pass's shape" in their own documents, and phase 7's rows cite them: "the register holds the evidence" (phase-7.md §2.1). The seam register follows them (A9).
- Decisions: `D-150` (no approval stops, so the triage takes its defaults: Q1), `D-197` (01's shell is the template's), `D-215` (the redo), `D-226` (no sitting for documents).
- Later steps read the S-rows: 7.8–7.12 and 7.17–7.20 their systems' rows, 7.13 the 01-only rows, 7.14 and 7.15 the performance evidence, Phase 8 §2.3's rows, each with the register's evidence.

## 1. Audit

Read at `e76c32f0`: the scope's sizes and boundaries, the seeds, and the phase 7 tables the triage writes. The analysis itself is the run's.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The scope is 157 files and 86,471 lines: 01's `src/` without its shell (78 files, 28,389 lines), `tests/` (31, 24,658) and `tools/` (14 Python files and one shader, 22,078); the template's `src/` and `build.rs` (21, 4,448), `tests/` (9, 5,940) and `tools/` (3 Python files, 958) | `wc -l` over §2.2's lists, which cover the globs once each |
| A2 | 01's `src/shell/` (12 files, 3,627 lines) is the template's byte for byte, and the repository check holds it so | `scripts/check-repo.py:163` (`check_shell_copies`); `cmp` over the 12 files → equal |
| A3 | The K-rows are 31: K6, K15–K18 and K24 are fixed; phase 7 assigns the rest (K5 as P7-14) | `docs/plans/1.0/example-01-feedback.md:608`; `docs/plans/phase-7.md:62` |
| A4 | K8 is fixed, yet phase 7 lists it open for 7.10c: `4d51a071` (qa6, 2026-10-28, in 0.112) counts an all-charges refill from the last charge spent, and it precedes phase 7's audit commit `08111d5a` | `git merge-base --is-ancestor 4d51a071 08111d5a` → exit 0; `docs/plans/phase-7.md:105` |
| A5 | The other seeds: 29 backlog rows cite 01, 11 known-issue lines cite 01's or the template's paths, and 7 Later lines name example 01 | Commands (a)–(c) below |
| A6 | The redo's §1 figures grew since `bb1e4c0`: 161 `has::<` checks in 01's `src/` (134 there), `save.rs` 2,873 lines (1,989), `src/` 32,016 (27,051), `tests/` 24,658 (18,933); `TagFilter` is still absent from 01's Rust | Commands (d)–(e) below; `wc -l` |
| A7 | Phase 7's two tables have the cells ID, Pattern, Step, Status and Note; its checks read the ID cell as one token (`[A-Z0-9-]+`, step 17) and the step cell whole; Step 9 counts "the 29 rows §2.2 sends to 7.10", which the table matches; §2.8 lists 7.10a–c's rows by ID | `docs/plans/phase-7.md:62`, `:125`, `:331`, `:289`, `:228`; command (f) below → 29 |
| A8 | The homes outside a step: ROADMAP.md's Later for a `Later` row (phase 7's §2.1), and the levers for 9.2 in `docs/perf/benchmarks.md`, which the redo's 9.2 row cites, for a `9.2` row | `docs/plans/phase-7.md:56`; `docs/perf/benchmarks.md:187`; `docs/plans/roadmap-redo.md:178` |
| A9 | The registers before this one live in their own plan documents, which stay while their rows are open | `docs/plans/paper-designs.md:387`; `docs/plans/arpg-design.md:5`, `:593` |
| A10 | `perf-runs/` is local and gitignored; H2 kept its evaluations there | `.gitignore:31`; `perf-runs/20261029-h2-skills/` |
| A11 | No UI-track work: the local branches are the version branches and `main`, and no commit names a 7.10–7.20 plan; `git worktree` is denied to agents | `git branch --list`; command (g) below → nothing; `~/.claude/settings.json` (`Bash(git worktree *)`) |
| A12 | Every `docs/plans/*.md` needs the six headers and `draft` or `in progress`; links and decision IDs in active plans are checked; a plan cited outside `docs/plans/`, the changelog and the decisions must exist | `scripts/check-repo.py:230`, `:308`, `:352`, `:40` |
| A13 | F3 asks for the bookkeeping overhead retaken on 7.5's sessions; the script takes a table of sessions and one own-plan topic, escaped | `docs/plans/phase-7.md:397`; `perf-runs/20261029-overhead-h1/classify.py:23`, `:63` |

Commands, run at `e76c32f0`:

- (a) `rg -c '^\|.*(examples/01_platformer|[Ee]xample 01|01 \()' docs/plans/1.0/backlog-1.x.md` → 29, the redo §1's figure.
- (b) `rg -c 'examples/01_platformer|templates/basic' docs/known-issues.md` → 11.
- (c) `rg -c '^- Example 01' ROADMAP.md` → 7.
- (d) `rg -o 'has::<' examples/01_platformer/src | wc -l` → 161.
- (e) `rg -c TagFilter examples/01_platformer/src` → nothing, exit 1.
- (f) `rg -c '^\| [^|]+ \| [^|]+ \| 7\.10 \|' docs/plans/phase-7.md` → 29.
- (g) `git log --all --format='%h %s' -- docs/plans/ | rg 'redo 7\.(1[0-9]|20)\b'` → nothing, exit 1.

## 2. Design

### 2.1 Outputs

- **The register,** `docs/plans/seam-register.md`, a plan document like the designs' (A9), written by steps 1, 3 and 4: its six headers; a digest; §1 method (the shares, the row shape, the lenses, counting and ranking, from §2.2–§2.3 here); §2 seeds (§2.4); §3 the register, rows `S-1` … in rank order and a totals line (rows and lines by lens, then by disposition); §4 registered elsewhere (an existing ID's 01 sites and lines); §5 bugs noted; §6 verification; §7 triage. Only §3's rows begin `| S-`; every other mention of an S-ID sits inside a cell or a list line. It never links or names this plan's file (A12, step 5's citation check), and it names phase-7.md and the redo in backticks, not links, so deleting either breaks no link in it. After 7.5 it stays `in progress`; once every S-row and every ID its §4 names is terminal, a commit points each `Later` and `9.2` line naming one of them at the register's last commit and deletes it with its citations (Q3).
- **Phase 7's tables** (step 4): every S-row in §2.2 or §2.3 (§2.6).
- **ROADMAP.md's Later and the levers for 9.2** (step 4): one line per `Later` or `9.2` row, carrying its S-ID.
- **Known issues** (step 5): each verified bug as a row.
- **The run's folder,** `perf-runs/<date>-analysis-pass/` (`$RUN` below; `<date>` is the run's start, `YYYYMMDD`), local: `sessions.txt` (§3), `shares.py`, `check.py`, `triage_check.py`, the prompts sent (`prompts/`), the readers' files (`readers/`), `merged.md`, the verifiers' files (`verify/`) and F3's retake (`overhead/`). Every count in the evidence log names its file there.

### 2.2 Shares

Nine readers (Q2). The brief's eight groups on today's tree: props, tomes and the dummy join their kin, the gun goes with combat to even the shares, the tools take two readers because they are 22,078 lines, and 01's shell is read once, as the template's. `f/` is 01's `src/features/`; 01's paths are under `examples/01_platformer/`; tests are `tests/game/<name>.rs`.

| # | Share | Files | Lines |
| --- | --- | --- | --- |
| 1 | Enemies | `f/enemies/`, `f/dummy/`; tests `enemies`, `dummy` | 10,432 |
| 2 | Elements and props | `f/elements/`, `f/props/`; tests `burning`, `cascades`, `ice`, `brick`, `props` | 8,124 |
| 3 | Spells, toys, lightning, tomes | `f/spells/`, `f/toys/`, `f/lightning/`, `f/tomes/`; tests `spells`, `ball_pit`, `spawning`, `lightning`, `tomes` | 9,978 |
| 4 | Combat, player, gun, audio, run | `f/combat/`, `f/player/`, `f/gun/`, `f/audio/`, `f/run/`; tests `combat`, `player`, `gun`, `audio`, `hud`, `presentation`, `regime`, `fixed_step`, `frame_time`, `guards` | 9,642 |
| 5 | Level | `f/level/`; tests `level`, `areas`, `hazards`, `camera`, `playthrough`; `tools/level.py`, `tools/placement.py` | 7,196 |
| 6 | Tools: terrain, scenery, the driver | `tools/`: `terrain.py`, `backdrops.py`, `scenery.py`, `props.py`, `art.py`, `pixels.py`, `polish_art.py`, `generate.py`, `test_generate.py` | 10,870 |
| 7 | Tools: actors, effects, sounds | `tools/`: `actors.py`, `effects.py`, `sfx.py`, `shaders/soft_glow.wgsl` | 9,894 |
| 8 | Saves and the root | `src/`: `save.rs`, `game.rs`, `lib.rs`, `main.rs`, `shared.rs`, `features/mod.rs`; tests `saves`, `main`, `seam`, `data` | 8,989 |
| 9 | The template | `templates/basic/`: `src/` (its shell is 01's), `build.rs`, `tests/`, `tools/*.py` | 11,346 |

Step 1's `shares.py` holds these lists. `--check` proves every in-scope file (01's `src/**/*.rs` without `src/shell/`, `tests/**/*.rs`, `tools/*.py` and `tools/shaders/*`; the template's `src/**/*.rs`, `build.rs`, `tests/**/*.rs` and `tools/*.py`) sits in exactly one share. `--ledger DIR` proves each reader's file has a ledger line covering every file of its share at its whole `wc -l`.

### 2.3 Row shape

One Markdown table row per seam, the same in the readers' files, `merged.md` and the register:

`| ID | Pattern | Where | Sites | Lines | Lacking | Generic fix | Depends on | Lens | Step |`

- **ID:** a reader's `<share>.<n>`, the merge's `m<n>`, the register's `S-<n>`.
- **Pattern:** the compensation, short. A row that re-reports a K-row ends its pattern with `(K<n>)` (Q4).
- **Where:** the code the generic fix would delete, as `path:a-b` or `path:a`, comma-separated. 01's paths are from `examples/01_platformer/` (`src/…`, `tests/…`, `tools/…`); a path starting `templates/` is from the repository root.
- **Sites:** how many places the pattern recurs, an integer, then the search that reproduces it when one does (`6: rg -n 'tick -= dt' src/features`).
- **Lines:** the sum of Where's ranges. A line sits in one row only. It counts what the fix deletes, tests included when the fix deletes them, and does not subtract the call that replaces it.
- **Lacking:** what the engine, kit or template lacks, ending with its evidence: `none: '<pattern>'` (no match in `api/*.txt` or `crates/*/src`), the item that falls short (`crates/…:line`), or `has: <item> (crates/…:line)` when the engine or kit already has it and only 01 does not use it.
- **Generic fix:** the smallest engine, kit or template change that deletes Where's code; for a `has:` row, 01's use of that item.
- **Depends on:** prerequisites only, as S-, K-, P7-, P8- or PA- IDs or redo steps; `—` when none. A split ID names its part as phase 7's Pattern does: `P7-2 (physics)` or `P7-2 (kit)`, `P7-3 (sensor exit)` or `P7-3 (Interactable)`. A seed a row matches is provenance, kept in the seed lines (§2.4), not here.
- **Lens:** `copy` helpers copying engine logic; `data` Rust where data belongs; `tags` markers where tags belong; `timer` hand-ticked timers; `arms` per-kind arms; `const` constants restating data; `boiler` save and ordering boilerplate; `perf` performance workarounds; `pos` positions copied into tests; `tools` duplication in the tools.
- **Step:** `—` until the triage writes the disposition (§2.6).

**Ranking:** Lines, then Sites, both descending, then share order. **IDs:** `S-1` … in rank order, assigned once after verification and never renumbered.

`check.py FILE` reads these rows and refuses (exit 1, naming the row) a cell missing, a repeated ID, a range outside its file, a line in two rows, or Lines unequal to its ranges' sum. `--sources DIR` adds: every reader row's ID is accounted for once in `merged.md`, in a merged row's sources line, in §4's entries or in the dropped list. `--verdicts DIR` adds: every merged row, §4 entry, seed fixed upstream and bug has one verifier line. `--register` adds: S-IDs contiguous from `S-1`, in rank order, each mapped in the register's §6 from one kept merged row or one §4 entry verified as a row.

### 2.4 Seeds and what is not re-reported

Step 1 writes the register's §2, and each reader's prompt carries the part for its share.

- **K-rows** (A3), each with phase 7's disposition. A reader reports, for each K-row whose "Meanwhile in 01" names its share, whether 01's workaround stands, grew, is gone, or the gap was fixed upstream (as A4's K8), with `file:line`. A K-row phase 7 assigns is re-reported, as an S-row ending `(K<n>)`, only with new evidence: more sites or lines than its feedback row, or a home that changed. Otherwise its sites go to the register's §4.
- **Backlog rows, known issues and Later lines** (A5), listed by line. A row that matches one says so in that seed's line (`<seed> → <row ID>`), which the merge and the register's §2 carry; a `Later` row that matches a Later line takes that line instead of a new one (§2.6).
- **The redo's §1 figures,** retaken at the run's HEAD with A6's commands.
- **Done at 7.1–7.4 and qa6:** the 0.112 notes (qa6 and 7.1) and 7.2–7.4's commits since `671cae00`. A seam they fixed is marked done in the reader's file, not reported.
- **Registered rows:** phase 7's §2.2 and §2.3: the P7-, P8- and PA- rows, and the K-rows without new evidence. A finding one of their fixes would delete goes to §4 with the ID, its ranges, sites and lines and the reader's row cells, never an S-row (Q5); a K-row with new evidence is the one exception, an S-row ending `(K<n>)` (above).

### 2.5 Readers, the merge and the verifiers

Subagents through the Agent tool (`general-purpose`, since each writes its file in `$RUN`), on the session's model (Q6). Each prompt is saved in `$RUN/prompts/` before it is sent; each names the register's §1–§2 as its method and seeds.

**Reader brief,** one per share; the nine go in one message so they run together:

- Read every line of every file in your share, in order, with the Read tool or `sed -n` in ranges of at most 400 lines; a search is not a read. After each file, append its rows and a ledger line `- <path>: 1-<n> of <n>` to your file, so nothing is lost to compaction.
- For each place the code compensates for the engine, kit or template, write a row in the register's §1 shape and lenses; for each seed of your share, a seed line (§2.4); for each bug, one line with `file:line`, never chased.
- Read the engine and kit only to check a row's Lacking claim: `api/*.txt` first, then a targeted `rg` and `sed -n` in `crates/`; never a whole engine or kit file.
- Don't report what §2.4 marks done. List a finding a registered row covers under §4 in your file, with its row cells, except a K-row with new evidence (§2.4), which is a row ending `(K<n>)`.
- Don't edit, stage or delete anything in the repository; write only your file, `$RUN/readers/<n>-<slug>.md`.
- End with counts: rows, lines, seeds, bugs, and whether the ledger covers the share.

**Merge brief,** one fresh subagent after the nine: read the nine files and the register's §1–§2, and write `$RUN/merged.md`. Rows from several shares with the same generic fix join into one (ranges and sites joined, the larger part's lens); a row a registered ID covers moves to §4; a K-row with new evidence keeps `(K<n>)`; a row with no generic fix is dropped, or listed as a bug when it is one. Rank and number the rows `m1` …; under the table, a sources line per merged row (`m3 ← 1.4, 3.2`); then §4's entries `e1` … (the registered ID, the ranges, sites and lines, and the reader's row cells), the seed lines (a seed fixed upstream with the commit that fixed it; K8 from A4 whether or not a reader names it), the bugs `b1` …, and the dropped rows with their reasons. The sources lines, §4's entries and the dropped list name every reader row once.

**Verifier brief,** fresh subagents, one per slice of at most 20 items: the merged rows grouped by the share of their largest part, then §4's entries with the seeds fixed upstream, then the bugs, each kind in slices of its own; none has read a share, and each opens nothing in `$RUN` but its own file. One line per item to `$RUN/verify/<k>.md`, one round:

- **A row:** open every range and confirm the code there is the pattern and is what the fix would delete; rerun Sites' search; run Lacking's evidence again and try at least two other names a real item might have in `api/*.txt` and `crates/`; judge whether the generic fix would delete the code and whether the engine or kit already has it (then it is a `has:` row); check that Depends on's IDs exist. `m<n>: confirmed`, `m<n>: corrected — <cell>: <new value>; …` or `m<n>: dropped — <reason>`.
- **A §4 entry:** confirm its ranges are the pattern and that the registered row's fix would delete them, `e<n>: confirmed`; if that fix would not, check it as a row and give its cells, `e<n>: row — <cells>`; if it compensates for nothing, `e<n>: dropped — <reason>`.
- **A seed fixed upstream:** confirm from the commit's diff and today's code that the gap is closed, `K<n>: confirmed`, else `K<n>: dropped — <reason>`; a seed without an ID goes by its `file:line`.
- **A bug:** `b<n>: confirmed` or `b<n>: dropped — <reason>`.

### 2.6 Triage

By phase 7's §2.4, the first rule that fits, at the defaults (Q1):

1. A `perf` row whose cost a §2.5 item covers goes to that item's pass (7.14 for a C-item, 7.15 for a G-item), its note naming the item, and the item's evidence cell names the S-ID. A cost no item covers goes to `9.2`.
2. A system a Phase 7 step builds goes to that step (the redo's §3 sketches and phase 7's §2.2 say which); a 7.10 row's note names 7.10a, b or c by §2.8's split. A started step sends its row to the first unstarted step that touches the system, the note saying `moved from <step> (started)`. Started means its evidence row is in phase-7.md, ROADMAP.md's Now names it, or a commit on any local branch names its plan (A11's command (g)).
3. A template row goes to 7.11; a fix only 01 needs, a `has:` row included, to 7.13.
4. A Phase 8 system goes to its 8.x step, in §2.3.
5. Anything else goes to `Later`, with its reason in the note.

Then the dependency pass (phase 7's §2.1: a row depending on a later step's system moves to the first step after it). A row whose Depends on names a row or a step that does not come at or before the row's own step moves to the first step that comes after it, the note saying `moved from <step> (depends on <ID>)`; a row depending on a `Later` row is `Later`. The pass repeats until no row moves. The order:

- the main track, 7.8 → 7.9 → 7.10a → 7.10b → 7.10c → 7.18 → 7.11 → 7.12 → 7.13 → 7.14 → 7.15 → 7.16;
- the UI track, 7.17 → 7.20, both before 7.11 and unordered against 7.8–7.18, so a row on one track that depends on the other moves to 7.11 or later;
- Phase 8 after all of Phase 7: 8.1 → 8.2 → 8.3 → 8.4 → 8.5 → 8.6, with 8.7 after 8.2 and before 8.5;
- then `9.2`, then `Later`.

A row may share its dependency's step or sub-plan, and a landed dependency binds nothing.

A `+` follows the register's Step, and opens phase 7's note, when the step's sketch in the redo's §3 does not list the fix.

Written into phase-7.md:

- **Rows:** a new S-row joins §2.2 after the last row of its step (Phase 7 steps, `Later` and `9.2`), or §2.3 (Phase 8): `| S-<n> | <pattern, shortened> | <step> | <status> | <note> |`, its status `open` for a step and `Later` or `9.2` otherwise. A re-reported K-row's row becomes its S-row in place: the ID cell `S-<n>`, the pattern ending `(K<m>)`, its step kept unless a §2.1 move says why (Q4).
- **Seeds fixed upstream:** each phase 7 row whose gap the verifiers confirmed fixed upstream becomes `landed <sha>` with its commit: K8 `landed 4d51a071`, noted "qa6, in 0.112" (A4), and any other the readers found. A fixed seed with no phase 7 row is noted in the register's §2; a known-issue entry among them leaves known issues in step 5.
- **§2.1:** one sentence: the seam register (`docs/plans/seam-register.md`) holds the S-rows' evidence, and its §4 01's sites for other IDs.
- **§2.5:** each `perf` S-row's ID in its item's evidence cell.
- **§3:** each Phase 7 step's Rows line gains its S-IDs, replacing "none" or "none yet" where it says so; 7.11's and 7.13's lines already point at §2.4 and stay. Step 9's count becomes the 7.10 rows' count, and §2.8's 7.10 bullet gains the S-IDs in their sub-plan's list.

Each `Later` row gets one line in ROADMAP.md's Later ending with its S-ID, or puts its ID on the Later line it matches. Each `9.2` row gets one line after the last of the levers for 9.2, `open`, under a lead-in naming redo 7.5. `Later` and `9.2` are terminal at once (phase 7's §2.1), so each line stands alone: the pattern, the generic fix and the main `file:line`, then the S-ID. The register's §7 records the date, the defaults, the counts by disposition, the started-steps check and the seeds fixed upstream.

`triage_check.py` checks that each S-row's phase 7 step cell equals the register's Step without `+` and the 7.10 letter, that its status fits it, that its step comes at or after the step of each ID its Depends on names, by the order above (a K-, P7-, P8- or PA- ID's step from phase 7's tables, a split ID's by its part, and a bare split ID refused), that each Phase 7 step's S-rows are on its Rows line (7.10's in §2.8's named list; 7.11's and 7.13's by their pointers), that Step 9's count matches, and prints the S-rows per disposition.

### 2.7 Bugs

A bug a reader meets is one line in its file. The merge carries it, a verifier confirms or drops it, the register's §5 lists the confirmed ones, and step 5 files each as a row in known issues' Open findings (P2 or P3, its trigger naming "found by redo 7.5"), marking it filed in §5.

### 2.8 Cost and stop

- Each of the 86,471 lines is read once, by one of nine readers; the engine and kit only for a claim; one merge agent; verifiers at up to 20 items a slice: two to five for 40–100 rows, one or two for §4's entries and the seeds fixed upstream, one for the bugs; no second round of either (the brief's stop). The run starts no other subagent.
- A share is 7,196–11,346 lines, so a reader's context holds one share. The main session reads `merged.md` and the verifiers' files, never the readers' files whole, so its context holds the register, not the shares.
- A resumed run keeps every reader's file whose ledger is complete and resends only the shares without one.

## 3. Steps

`$RUN` is `perf-runs/<date>-analysis-pass`. `<plan>` is this plan's commit, `git log --diff-filter=A --format=%h -- docs/plans/analysis-pass.md`. The no-code check is `git diff --quiet <plan> -- crates examples templates tools scripts api Cargo.toml Cargo.lock justfile` → exit 0. Each session that works on this plan appends its transcript prefix (the session ID's first eight characters, as in the scratchpad's path) to `$RUN/sessions.txt` before its first change, creating `$RUN` if need be, so F3 counts every session a resumed run used, an interrupted one included. Each step's commit pushes the version branch.

### Step 1: Frame the register

- **Files:** `docs/plans/seam-register.md` (new); this plan; `$RUN/shares.py`, `$RUN/check.py`, `$RUN/fixture.md` (local).
- **Change:** set this plan's status to `in progress`. Write the register: six headers (`status: draft`); a digest; §1 method from §2.2–§2.3 here; §2 seeds from §2.4 (the K-rows with phase 7's dispositions and an empty state column, the backlog, known-issue and Later lines by number, the redo's §1 figures retaken, the commits done at 7.1–7.4 and qa6, and the registered IDs by pointer to phase 7's §2.2–§2.3); §3 with its table header and no rows; §4–§7 empty. Write `shares.py` and `check.py`, and run `check.py` on a fixture of three hand-made rows: one good, one overlapping it, one whose Lines disagree with its range.
- **Done-when:** `python3 -I -B $RUN/shares.py --check` → `unassigned 0, doubled 0`, exit 0, with A1's 157 files and 86,471 lines or the moved tree's, quoted; `python3 -I -B $RUN/check.py $RUN/fixture.md` → exit 1, naming the overlap and the Lines mismatch; `python3 -B scripts/check-repo.py` → exit 0; the no-code check → exit 0.
- **Commit:** `docs(plans): frame the seam register for redo 7.5`
- **Moves:** none.

### Step 2: Read and merge

- **Files:** this plan's evidence row; `$RUN/prompts/`, `$RUN/readers/`, `$RUN/merged.md` (local).
- **Change:** send the nine readers (§2.5) in one message. Check each ledger; a reader whose ledger misses files is sent once more for those files alone. Then the merge agent, then `check.py` on `merged.md`. Read `merged.md` whole.
- **Done-when:** `git status --porcelain` → empty after the agents, before the evidence row is written; `python3 -I -B $RUN/shares.py --ledger $RUN/readers` → `unread 0`, exit 0; `python3 -I -B $RUN/check.py --sources $RUN/readers $RUN/merged.md` → exit 0, its merged rows and lines quoted; `merged.md` holds the ranked rows, their sources lines, §4's entries, the seed lines, the bugs and the dropped rows.
- **Commit:** `docs(plans): record redo 7.5's reading and merge`
- **Moves:** none.

### Step 3: Verify

- **Files:** `docs/plans/seam-register.md` (§2's states, §3, §4, §5, §6); this plan; `$RUN/verify/` (local).
- **Change:** send the verifiers (§2.5) in one message. Apply each correction to its row; an `e<n>: row` entry joins the rows with its cells; move each dropped row or entry to §6 with its reason; rerun `check.py`; rank; number `S-1` …. Write §3 with its totals line, §4 (the confirmed entries), §5 (the confirmed bugs) and §6 (the slices, the verdict counts, what changed, the dropped items and the `S-<n> ← m<k>` or `← e<k>` map); fill §2's states from the merge's seed lines, each upstream fix marked confirmed or dropped and each match named by its S-ID.
- **Done-when:** `python3 -I -B $RUN/check.py --verdicts $RUN/verify $RUN/merged.md` → `missing 0`, exit 0, its confirmed, corrected and dropped counts quoted; `python3 -I -B $RUN/check.py --register docs/plans/seam-register.md` → exit 0; `rg -c '^\| S-[0-9]+ ' docs/plans/seam-register.md` → the confirmed and corrected rows plus the §4 entries verified as rows; `python3 -B scripts/check-repo.py` → exit 0; the no-code check → exit 0.
- **Commit:** `docs(plans): write the verified seam register for redo 7.5`
- **Moves:** none.

### Step 4: Triage

- **Files:** `docs/plans/seam-register.md` (its Step column, §3's totals, §7); `docs/plans/phase-7.md` (§2.1, §2.2, §2.3, §2.5, §2.8, §3); `ROADMAP.md` (Later); `docs/perf/benchmarks.md` (only for a `9.2` row); this plan; `$RUN/triage_check.py` (local).
- **Change:** check which steps have started (§2.6, rule 2); dispose of each row by §2.6 and write it into the register and phase-7.md; the seeds fixed upstream; the Later and `9.2` lines; the register's §7.
- **Done-when:**
  - `cmp <(rg -o '^\| S-[0-9]+' docs/plans/seam-register.md | sort) <(rg -o '^\| S-[0-9]+' docs/plans/phase-7.md | sort)` → exit 0: the same S-IDs, each once in phase 7's tables. Phase 7's Step 4 count, `rg -c '^\| S-[0-9]+ ' docs/plans/phase-7.md`, quoted beside the register's.
  - `python3 -I -B $RUN/triage_check.py` → exit 0, the S-rows per disposition quoted.
  - `for id in $(rg -o -r '$1' '^\| (S-[0-9]+) \| [^|]+ \| Later \|' docs/plans/phase-7.md); do rg -q "\b$id\b" ROADMAP.md || echo "$id"; done` → prints nothing; the same loop with `9\.2` in place of `Later`, over `docs/perf/benchmarks.md` → prints nothing.
  - ``rg -c '^\| K8 \| [^|]+ \| 7\.10 \| landed `4d51a071`' docs/plans/phase-7.md`` → 1.
  - `python3 -B scripts/check-repo.py` → exit 0; the no-code check → exit 0.
- **Commit:** `docs(plans): triage the seam register into phase 7`
- **Moves:** phase-7.md's tables and the lines §2.6 names; ROADMAP.md's Later; the levers list.

### Step 5: Close

- **Files:** this plan (deleted); `ROADMAP.md` (Now); `docs/plans/phase-7.md` (its evidence row, F3); `docs/plans/seam-register.md` (status, §5's filed marks); `docs/known-issues.md`; `$RUN/overhead/` (local).
- **Change:**
  1. File §5's bugs in known issues (§2.7), and remove each known-issue entry the verifiers confirmed fixed upstream, its commit named in the register's §2.
  2. F3 (Q7): copy `perf-runs/20261029-overhead-h1/classify.py` to `$RUN/overhead/`, with one group for 7.5 (this plan's session, transcript prefix `bc379e5c`, and every prefix in `$RUN/sessions.txt`, the close's own included) and `own_plan_re` matching `analysis-pass` and `seam-register`; run it with and without `--subagents`; quote both in phase 7's evidence row and mark F3 done there.
  3. The register's status: `in progress (written, verified and triaged by redo 7.5; the evidence its rows' steps build from, deleted with its citations by Q3's rule)`, with Q3's rule in its done-when.
  4. Phase 7's evidence row `4 (7.5)`: the commits from this plan's to the close, each step's done-when quoted from the evidence log below, the S-rows per disposition, `$RUN`.
  5. Take `analysis-pass` out of Now; phase 7's line says 7.5 landed with the register's row count and that 7.8 is next. Delete this plan.
  6. Run the checks and the demo; put their results in phase 7's evidence row before the push.
- **Done-when:**
  - `rg -n '^\| [^|]+ \| [^|]+ \| 7\.5 \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0 (phase 7's every-step check); phase 7's Step 4 count → the register's, quoted.
  - `just check` → passes; `just ctx`, `just repo-check` and `git diff --check` → exit 0.
  - `rg -l 'analysis-pass\.md' --glob '!CHANGELOG.md' --glob '!DECISIONS.md'; test $? -eq 1` → exit 0.
  - `rg -c 'redo 7\.5' docs/known-issues.md` → §5's filed count (none filed: no output, exit 1).
  - The demo: `just status` → `docs/plans/phase-7.md: in progress · steps: 17 · evidence rows: 4 · last: step 4 (7.5), passed`, and Now without `analysis-pass`.
  - After the commit, `git diff --quiet <plan> HEAD -- crates examples templates tools scripts api Cargo.toml Cargo.lock justfile` → exit 0.
- **Commit:** `docs(plans): close the analysis pass, its rows in phase 7`
- **Ends with:** the commits, the evidence, what the owner should read (the register's top rows and its §7; a re-cut is a phase 7 §2.1 move), and the next prompt: tungsten-next's `step` prompt for 7.8's first plan, runtime terrain (phase 7's Step 5 and §2.8, the redo's 7.8 row, §5.1 and §5.6).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The redo's 7.5 row ends at "the owner's triage done", and `D-150` stops no session for approval | The run triages every row at §2.6's defaults and says so in the register's §7; the owner reads §7 on their own time, and a re-cut is a phase 7 §2.1 move |
| Q2 | Nine readers where the brief says about eight | Yes: the tools are 22,078 lines and take two; the rest follow the brief's groups (§2.2) |
| Q3 | Where the register lives and for how long | `docs/plans/seam-register.md`, as the designs' registers do. After 7.5 it stays `in progress`, outliving phase-7.md while a Phase 8 S-row or a Phase 8 row its §4 names is open. Once every S-row and every ID in its §4 is terminal, the commit that deletes it with its citations first ends each `Later` and `9.2` line naming one of them with "the seam register at `<sha>`", its last commit, as the levers for 9.2 cite their history; that form names no `docs/plans/` path, which the citation check would fail once the file is gone (`scripts/check-repo.py:41`). Its header's done-when holds this rule |
| Q4 | Where a re-reported K-row's "K ID beside its S ID" goes | At the pattern's end, `(K<n>)`: phase 7's step 17 reads the ID cell as one token, so `S-<n> (K<n>)` there would escape its carry check. The K-row's row becomes the S-row in place, so no seam has two rows |
| Q5 | A finding that a P7-, P8- or PA- row's fix would delete | The register's §4, with the ID, sites and lines; no S-row (the brief) |
| Q6 | The subagents' model | The session's, Opus 5.5: a row's worth rests on the reader's judgment, and the verifiers check it |
| Q7 | F3's retake on a documents pass | At the close, with the register counted as 7.5's own work beside the plan, both variants quoted and read with that caveat beside the code sessions' figures |
| Q8 | A floor for small rows | None: every seam is a row, ranked by lines and sites; most small ones are 01-only and go to 7.13 |

## 5. Invariants

- **May move:** in step 4, phase-7.md's §2.1 sentence, §2.2 and §2.3 (S-rows added, re-reported K-rows rewritten in place, the statuses of seeds fixed upstream), §2.5's evidence cells, §2.8's 7.10 bullet, §3's Rows lines and Step 9's count; ROADMAP.md's Later; the levers for 9.2. In step 5, phase-7.md's evidence row and F3; ROADMAP.md's Now; known issues.
- **Must not move:** code, tests, tools, assets, data, scripts, `api/`, `Cargo.lock` and the justfile (the no-code check at every step); every other phase 7 row's step and status, except by a §2.1 move with its reason; §2.5's lists' membership; the redo, the designs, the feedback plan and the backlog.

## 6. Stop conditions

- A done-when check fails: the skill's rule (restore the step's files, mark it skipped, carry on with the steps that don't depend on it). Steps 3–5 depend on step 2, and 4–5 on step 3.
- A reader's ledger misses files after its one resend: step 2 stops, since no register is built from a partial reading.
- The merge's sources miss a reader row, or a verifier's file misses a row of its slice: that agent is resent once; a second gap stops the step.
- An agent changes the tree: restore each changed file from `HEAD` (`git show HEAD:<path> > <path>`) and remove each file it added (`python3 -c "import os; os.remove('<path>')"`), keep its output in `$RUN` only if it reads whole, and rerun step 2's tree check.
- Another session works in this tree at a step's start (`just status`): wait for it to end.
- A row no triage rule fits: `Later`, with its reason in the note.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-30, reading the draft over `e76c32f0`. Footer: `447.4 s, tokens in 509585 (cached 442880), out 7437 (reasoning 4864), commands 14, output 272.9 KB (largest 46.8 KB), usage 5h 1% → 16%, week 41% → 43%`. Five findings, all accepted; they changed the verification's scope, the triage's rules and their checks, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Evidence moved under existing IDs escapes verification | The brief verifies every row (`docs/plans/roadmap-redo.md:425`, `:426`); §2.5 sent the verifiers merged rows and bugs only, and step 4 changed statuses on upstream-fix claims | accepted | §2.5: §4's entries and the seeds fixed upstream get verifier slices and verdicts (`e<n>`, `K<n>`), and an entry its registered row does not cover is verified as a row; `check.py --verdicts` covers them; §2.6 and step 4 act only on confirmed upstream fixes |
| 2 | [MEDIUM] The reader instructions contradict the exception for new K-row evidence | `docs/plans/phase-7.md:263` re-reports a K-row with new evidence; §2.4's registered rows called every K-row "never an S-row" | accepted | §2.4 and the reader brief: registered means the P7-, P8- and PA- rows and the K-rows without new evidence; a K-row with new evidence is a row ending `(K<n>)` |
| 3 | [MEDIUM] The retirement rule can delete evidence for deferred work | `docs/plans/phase-7.md:56`: `Later` and `9.2` are terminal at once | accepted | §2.6: each `Later` and `9.2` line stands alone (pattern, fix, main `file:line`, S-ID); §2.1, Q3 and step 5: the deleting commit first points those lines' S-IDs at the register's last commit, as the levers for 9.2 cite their history (refined by 2.3 and 2.4) |
| 4 | [MEDIUM] Triage does not enforce prerequisite ordering | `docs/plans/phase-7.md:55`: a row depending on a later step's system moves to the first step after it | accepted | §2.6: a dependency pass over the main track, the UI track, 7.10's sub-plans and Phase 8, repeated until no row moves; `triage_check.py` checks each row's step against its dependencies' |
| 5 | [MEDIUM] F3 omits earlier execution sessions after a resumed run | `perf-runs/20261029-overhead-h1/classify.py:157`, `:160` scan only the listed prefixes | accepted | §3's preamble: each step's evidence row named its session's transcript prefix, and step 5's F3 classified every prefix the log named (replaced by 2.5) |

Round 2: the same reviewer at xhigh, 2026-10-30, reading the draft with this section and the log cut, and a focus note on round 1's changes. Footer: `394.9 s, tokens in 587102 (cached 516736), out 8051 (reasoning 4404), commands 26, output 284 KB (largest 40.5 KB), usage 5h 16% → 32%, week 43% → 46%`. Five findings, each a gap a round-1 fix left, all accepted; none stays open, and no third round runs. A sweep of each round's fixes against the clauses that read the same state also changed §2.8's cost line and steps 2–3's checks (round 1), and step 3's seed matches, §2.1's folder list and the register's references to phase-7.md and the redo (round 2).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Dependency IDs do not uniquely identify prerequisites | P7-2 sits at 7.8 and 7.10 (`docs/plans/phase-7.md:70`, `:102`), P7-3 at 7.8 and 8.1 (`:71`, `:127`), the tables' only repeated IDs | accepted | §2.3: a split ID names its part as phase 7's Pattern does; `triage_check.py` resolves it by its part and refuses a bare one |
| 2.2 | [MEDIUM] Seed citations are being treated as dependency edges | Backlog rows carry no ID (`docs/plans/1.0/backlog-1.x.md:90`), and §2.3 allowed only IDs and steps in Depends on | accepted | §2.3: Depends on holds prerequisites only; §2.4: a match is `<seed> → <row ID>` in the seed's line, which the merge carries and step 3 names by S-ID |
| 2.3 | [MEDIUM] The proposed historical citation prevents register deletion | The citation check matches any `docs/plans/….md` text and fails a missing file (`scripts/check-repo.py:41`, `:362`) | accepted | Q3: the pointer reads "the seam register at `<sha>`", with no `docs/plans/` path |
| 2.4 | [MEDIUM] Retirement still ignores evidence stored under existing IDs | §4 holds 01's evidence for registered rows that stay open, P8-9's at 8.2 likely among them (`docs/plans/paper-designs.md:432`, `docs/plans/phase-7.md:139`) | accepted | §2.1, Q3 and step 5: deleted once every S-row and every ID in §4 is terminal, the pointer on each `Later` and `9.2` line naming either |
| 2.5 | [MEDIUM] F3 can still omit a session interrupted within a step | A session can write reader files and end before step 2's evidence row; `classify.py:157`, `:160` read only the listed prefixes | accepted | §3's preamble: each session appends its prefix to `$RUN/sessions.txt` before its first change, and F3 reads that file |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-30 | passed | `shares.py --check` → the nine shares at §2.2's figures, `in scope: 157 files, 86,471 lines`, `unassigned 0, doubled 0`, exit 0; `check.py fixture.md` → `refused: fixture.md:8 m2: src/game.rs:15 is also in m1 (overlap)`, `refused: fixture.md:9 m3: Lines 12 but its ranges sum to 10`, exit 1; `check-repo.py` → 0 errors, exit 0; the no-code check against `3b01712d` → exit 0. Session `f5a4f152` in `sessions.txt`; the other session in the tree (`bc379e5c`) answered a status question read-only, idle from 02:40:58, before the first change | `perf-runs/20261030-analysis-pass/` (`shares.py`, `check.py`, `fixture.md`, `sessions.txt`) |
| 2 | 2026-10-30 | passed | Nine readers in one message (Opus 5.5, `general-purpose`): 5.00M subagent tokens, 1,747 tool calls, 20–42 min each; 253 rows (15,085 lines), 86 §4 entries (10,653 lines), 132 seed lines, 9 bugs, none resent. The merge agent: 0.39M tokens, 40 calls. `git status --porcelain` after the agents → empty; `shares.py --ledger` → `unread 0`, exit 0 (157 ledger lines); `check.py --sources readers merged.md` → `e: 32 rows, 10,116 lines; m: 195 rows, 15,622 lines`, `reader rows 339, reader bugs 9, unaccounted 0, doubled 0`, `errors 0`, exit 0; `merged.md` (584 lines) holds the ranked rows m1–m195, a sources line each, the entries e1–e32 with theirs, 81 seed lines (fixed upstream: K8 `4d51a071`, K25 `433b4998`, known issue 31 `1803bb7d`), bugs b1–b9, the dropped list (none) and 8 done lines | `perf-runs/20261030-analysis-pass/` (`prompts/`, `readers/`, `merged.md`) |
| 3 | 2026-10-30 | passed | Thirteen verifiers in one message (Opus 5.5), each sent a pointer to a byte-identical copy of its saved prompt outside `$RUN`, so it opened nothing there but its own file: ten slices of the rows (20 each, the last 15), two of the 32 entries with K8, K25 and known issue 31, one of the 9 bugs; 4.58M tokens, 2,026 calls. `check.py --verdicts verify merged.md` → rows 84 confirmed, 109 corrected, 2 dropped; entries 18 confirmed, 14 corrected; seeds 2 confirmed (K8, known issue 31), 1 dropped (K25, closed in part); bugs 8 confirmed, 1 dropped; `items 239, missing 0`, `errors 0`, exit 0. Corrected ranges reached 322 lines other rows held in the merge (51 spans), each left with that row (§6). `check.py --register seam-register.md` → `S: 193 rows, 15,810 lines; e: 32 rows, 9,919 lines`, `errors 0`, exit 0; `rg -c '^\| S-[0-9]+ '` → 193 (84 confirmed, 109 corrected, no entry verified as a row); `check-repo.py` → 0 errors, exit 0; the no-code check → exit 0 | `perf-runs/20261030-analysis-pass/` (`verify/`, `gen_verify.py`, `build_register.py`) |

## Follow-ups

None yet.
