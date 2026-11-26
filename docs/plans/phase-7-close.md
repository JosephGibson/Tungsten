# Phase 7's close (phase 7, Step 17) — draft

- **status:** draft
- **goal:** Close Phase 7 as [the Phase 7 plan](phase-7.md)'s Step 17 says: carry its §2.3's 37 open rows into the [roadmap redo](roadmap-redo.md)'s §3 Phase 8 rows by ID (21 are not there yet), with P8-6's note; point every record that cites the plan, or names its tables as a disposition's home, at the redo or at the plan's last commit; close its follow-up F1; keep preflight's R7 reading rows once its source is gone; then, the tests passing and example 01 running, delete the plan and this one with their citations, `phase-7` out of Now and `phase-8` in.
- **non-goals:**
  - The [genre jam](genre-jam.md)'s triage: its 14 untriaged rows stay the owner's (`D-150`); the close records that the steps 13 of them were proposed for have closed, and moves none (Q3).
  - Phase 8's process: how a Phase 8 step records the rows it lands is 8.1's plan's to set (Q5).
  - The [paper designs](paper-designs.md)' step 5 and status; the redo's Phase 7 table beyond 7.13's citation; its 8.4–8.6 rows; `DECISIONS.md`; `CHANGELOG.md`, whose history keeps the plan's name; the frozen 1.0 records; known issues.
  - Code beyond `scripts/preflight.py`'s R7 and one doc comment in `examples/02_bench/src/piles.rs`; a release (Q4); a benchmark sitting (`D-226`: no row's workload changes).
  - A decision entry: the close sets no dependency, format or policy; where Phase 8's rows live is the Phase 7 plan's Q6 default, already taken.
- **files to touch:**
  - Step 1: `docs/plans/roadmap-redo.md`; this file (status, evidence).
  - Step 2: `docs/plans/arpg-design.md`, `docs/plans/seam-register.md`, `docs/plans/genre-jam.md`, `docs/plans/paper-designs.md`, `examples/02_bench/src/piles.rs`; this file (evidence).
  - Step 3: `scripts/preflight.py`, `scripts/test-preflight.py`; this file (evidence).
  - Step 4: this file (evidence, then deleted), `docs/plans/phase-7.md` (deleted), `ROADMAP.md`, `docs/plans/roadmap-redo.md` (its session log's deletion sentence).
- **ordered steps:** (1) recheck the Phase 7 plan's state and carry its open rows into the redo; (2) the registers and the bench comment off the plan; (3) preflight's R7 on the redo's rows; (4) the close: the checks and the demo, then `ROADMAP.md` and the deletions.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` exits 0 and example 01's demo prints `passed`; `docs/plans/phase-7.md` and this file deleted, `phase-7.md` named in no tracked file but `CHANGELOG.md`, `phase-7` and `phase-7-close` out of Now and `phase-8` in it; `just repo-check` and `just ctx` pass.

Item `phase-7`, Step 17, in [ROADMAP.md](../../ROADMAP.md), from the Phase 7 plan's Step 17, the redo's §3 Phase 8 rows (8.1, 8.2 and 8.7, and 8.3 for the S-rows 7.5 sent there) and the [ARPG design](arpg-design.md). Written 2026-11-25 at `baaecc20`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- Phase 7's Steps 1–16 and 18 (7.21) have closed, 7.16 last at `baaecc20` (`D-312`). Step 17 builds no system, so it adds no benchmark row or counter guard (the Phase 7 plan's §3), and it changes no row's workload, so it takes no sitting (`D-226`).
- The Phase 7 plan's done-when: every §2.2 row terminal (landed, closed, `Later` or `9.2`, its §2.1), each `Later` row a line in `ROADMAP.md`'s Later and each `9.2` row in the levers for 9.2; every open §2.3 row in the redo's §3 by ID; 7.14's and 7.15's items done, dropped or stopped; the tests pass and 01 runs; the plan deleted with its citations, `phase-7` out of Now.
- Its Q6 default: Phase 8's assignments live in the redo's §3 rows by ID, and the ARPG design stays until 8.7 closes. Its Q9: P8-6's schedules resolve by time, and walking between places waits for 8.2.
- Decisions: `D-215` (the redo the plan of record, `ROADMAP.md` the status), `D-218` (8.7), `D-150` (defaults; nothing stops for approval), `D-140` (routes), `D-226`.
- The plan's content stays as `baaecc20` holds it: this plan's commit and Steps 1–3 leave it alone, so every pointer that replaces its path names `baaecc20` (the genre jam's line cites, read at its framing, name `cad63fa5`), and `git show <sha>:docs/plans/phase-7.md` reads it, a path `CHANGELOG.md` keeps.
- After the close, 8.1's plan reads its rows from the redo's 8.1 row and 8.7's from its row and the ARPG design's §15–§16, and the seam register's and the genre jam's deletion rules name homes that exist.

## 1. Audit

Read at `baaecc20`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | §2.2 holds 300 rows, each terminal: 256 landed, 20 closed, 22 `Later`, 2 `9.2`; none open in a Phase 7 step | its table; §2.5's `open` exits 1 there |
| A2 | Each `Later` row's ID is in a line of `ROADMAP.md`'s Later (K25, S-1, S-15, S-17, S-24, S-35, S-47, S-103, S-132, S-182, S-184, EV-19–EV-24, EV-48, EV-50–EV-53), and both `9.2` rows are in the levers for 9.2 | `ROADMAP.md:11-143`; `docs/perf/benchmarks.md:513` (S-90), `:514` (EV-41) |
| A3 | §2.5's twelve items are terminal: C1–C5 done, C6 stopped; G1 and G6 done, G2 done in four areas and stopped in two, G3–G5 stopped | the Phase 7 plan's §2.5, its Status cells |
| A4 | §2.3 holds 39 rows: P8-14 and P8-16 closed, which the redo's 8.2 row says; 37 open, 12 for 8.1, 10 for 8.2, 6 for 8.3 and 9 for 8.7. 16 are in their step's redo row by ID; 21 are nowhere in the redo: PA-12, PA-13 and S-162 (8.1); PA-14, PA-15, PA-16 and S-6 (8.2); S-8, S-11, S-22, S-61 and S-69 (8.3); PA-3, PA-4, PA-9 and PA-17–PA-22 (8.7). Step 17's own check prints those 21 today | the Phase 7 plan's §2.3; `docs/plans/roadmap-redo.md:160-166` |
| A5 | P8-6's note, that walking between places waits for P8-8, is not in the redo's 8.1 row, which places an NPC by time while its area is unloaded | `roadmap-redo.md:160` |
| A6 | The plan is cited outside `CHANGELOG.md` (exempt, `scripts/check-repo.py:40`) at `ROADMAP.md:7` and `:141` (links); `roadmap-redo.md:136`, `:717`; `arpg-design.md:12`, `:19`, `:36` (links), `:624`, `:659`, `:694`; `seam-register.md:3`, `:220`; `genre-jam.md:66`, `:71` (two), `:86`, `:98`–`:104`; `scripts/preflight.py:178`, `:1351`, `:1734`; `examples/02_bench/src/piles.rs:3`. Once it is gone the citation check fails on the last two files (`check-repo.py:368-385`, every tracked file outside `docs/plans/`) and the link check on the five links (`:297-321`); the rest are dead routes no check sees | `rg -n 'phase-7\.md'` |
| A7 | Two registers name phase 7's tables as their dispositions' home: the seam register's files to touch and its deletion rule ("terminal (phase 7's §2.1)"), and the genre jam's header, files to touch and done-when | `seam-register.md:12`, `:14`; `genre-jam.md:3`, `:11`, `:13` |
| A8 | The genre jam's triage never ran: 14 rows wait, 13 proposed for 7.11, 7.12 and 7.16, which have closed (J-1–J-4, J-6, J-7, J-12, J-13, J-15, J-16, J-22, J-24, J-25), and J-19 for the kit backlog, with J-14's kit fix proposed for Later. No J- ID is in the Phase 7 plan, `ROADMAP.md`, known issues or the backlog, and 7.13g's evaluation found J-1 unbuilt | `genre-jam.md:131`; `rg -c 'J-[0-9]'` on those files; `37ec6dae` |
| A9 | Preflight's R7 reads its rows from the Phase 7 plan alone; with the file gone it finds nothing and says nothing. Its tests only list R7 in the rule table. On a probe plan naming P8-1 and S-6 (§2.5) it lists the plan's lines 375 and 395 today | `scripts/preflight.py:174-178`, `:1346-1355`; `scripts/test-preflight.py:208` |
| A10 | The plan's F1 is open: the paper designs' step 6 is unmarked, and it and their §2's task list say "8.5" where the redo's §8 says 8.6; F2–F4 are done | the Phase 7 plan's Follow-ups; `paper-designs.md:21`, `:143`; `roadmap-redo.md:548` |
| A11 | `ROADMAP.md`'s Now holds `phase-7` alone (one 21,343-byte line), Next is empty, and Later holds a Phase 7 line (linking the plan) and a Phase 8 line. `just repo-check` refuses an empty Now | `ROADMAP.md:7`, `:141`, `:142`; `check-repo.py`'s `check_roadmap_md` (one to three items) |
| A12 | The plan last changed at `baaecc20`. §2.6's assignments past Phase 7 are in the redo already (8.6 takes the paper designs' ten weekly tasks and 7.19's five, 9.1 the stress numbers), and the ARPG design's §1 cites §2.7's recheck | `git log -1 -- docs/plans/phase-7.md`; `roadmap-redo.md:165`, `:293`, `:548`; `arpg-design.md:36` |
| A13 | Every step but 17 has an evidence row (53, the cpu-stress pass's among them); Step 17's would land in a file its own last commit deletes, so this plan's log holds it | the Phase 7 plan's evidence log |
| A14 | Example 01's demo (§2.4) passes on this tree: a run, a reload through `reload_run`, and the player's health read before and after | `tungsten run: 420 frames, 0 captures, 2 asserts: passed`, exit 0; its report: no failed or unreached steps, `actual` 100.0 at frames 150 and 400 |

## 2. Design

### 2.1 The carry (Step 1)

Each open §2.3 row goes into its step's redo row by ID, at the clause naming the system it changes. `<date>` is the run's date throughout.

- **8.1:** after "with equipment staying statuses (P8-3, Q17)" add ", items with rolled lines, an instance holding its base, level, rarity, seed and lines, its local lines folded into its block and its global ones on its equipment status (PA-12), and ground items labelled in their rarity's style (PA-13), from the [ARPG design](arpg-design.md)'s §4.6 and §16"; after "7.20's screen files" add ", the inventory's and shops' cells in a grid layout, the first user `D-124` waits for (S-162)"; "(P8-6)" becomes "(P8-6; walking between its places waits for 8.2's navigation, P8-8)".
- **8.2:** after "no paper design covers them" add "; the seam register's S-6, a walker's hops, drops and leaps by casts"; after "and waves (P8-13)" add "; on that payload and P8-11's bursts, the ARPG design's projectile rows: count and pierce from the skill sheet (PA-14), pierce, then fork, then chain at each hit under a family's bound of 64 (PA-15), and repeats of the whole payload (PA-16), its §16's 8.2 row".
- **8.3:** after "beside our own format (Q16)" add "; and the seam register's asset rows: each file held to its asset contract by `tungsten check` (S-8), per-family generation in `tungsten` in place of 01's own pipeline (S-11), named points mirrored by facing and a body section in prefab data (S-22), a point per animation frame, as slices import (S-61), and normal maps and emissive masks the pipeline derives (S-69)".
- **8.7:** "less what 7.10, 8.1 and 8.2 took." becomes "less what 7.10, 8.1 and 8.2 took: the ARPG design's PA-3 and PA-4 (area, projectile speed and use time read from the sheet) and PA-9 (primary stats), moved from 7.10, and PA-17 (supports), PA-18 (the passive tree), PA-19 (item generation), PA-20 (uniques and handlers), PA-21 (monster level and rarity) and PA-22 (`explain` and build saves), its §15 and §16."
- **After the Phase 8 table** (`:168`), appended: "Since Phase 7's close (<date>) each row also names the register rows it takes by ID, the ARPG design's `PA-` rows (§15 there) and the seam register's `S-` rows among them, carried from the Phase 7 plan's §2.3 (at `baaecc20`), where P8-14 and P8-16 closed; S-6 is navigation on platforms' row."
- **Citations:** `:136`'s "(`phase-7.md` §2.2)" becomes "(the Phase 7 plan's §2.2, at `baaecc20`)"; `:717`'s "`phase-7.md` §2.8" becomes "the Phase 7 plan's §2.8, at `baaecc20`".
- **Session log,** a last entry: "- **<date>, Phase 7's close** (the Phase 7 plan's Step 17): §3's Phase 8 rows took the plan's 37 open rows by ID, 21 of them new there (PA-12, PA-13 and S-162 in 8.1; PA-14, PA-15, PA-16 and S-6 in 8.2; S-8, S-11, S-22, S-61 and S-69 in 8.3; PA-3, PA-4, PA-9 and PA-17 to PA-22 in 8.7), and P8-6's wait for 8.2's navigation." with the log's "Not critiqued." line under it. The deletion's sentence joins it in Step 4's last commit (§2.4), so no commit records a deletion before it happens.

### 2.2 The registers off the plan (Step 2)

The rule: a mention of the plan becomes "the Phase 7 plan" with its section and commit, with no path and no link; a claim that phase 7's tables hold a disposition becomes the redo's §3 Phase 8 rows for the rows still open. Lines that only record what was done at the time (the seam register's goal and digest, its §7, the jam's readers) stay.

- **ARPG design.** `:12`: "The [Phase 7 plan](phase-7.md) holds §15's rows' steps and status (its §2)" becomes "The [roadmap redo](roadmap-redo.md)'s §3 Phase 8 rows hold §15's open rows by ID since Phase 7's close, which found the rest landed (§15's triage note)". `:19`: "Done 2026-10-29 in the [Phase 7 plan](phase-7.md) (§2.2, §2.3, §2.6), with §1's assumptions rechecked there (§2.7)." becomes "Done 2026-10-29 in the Phase 7 plan (§2.2, §2.3, §2.6), with §1's assumptions rechecked there (§2.7); at its close (<date>, its last content at `baaecc20`) its open rows went into the redo's §3 Phase 8 rows by ID." `:36`: "([Phase 7 plan](phase-7.md) §2.7)" becomes "(the Phase 7 plan's §2.7, at `baaecc20`)". `:624`, `:659`, `:694`: "(`phase-7.md` §2.3)" becomes "(the Phase 7 plan's §2.3, at `baaecc20`)". §15's triage paragraph (`:624`) ends with: "At Phase 7's close (<date>) the eight rows 7.10 took had landed, PA-1, PA-2, PA-5, PA-6, PA-7, PA-10 and PA-11 in 7.10a (`D-238`) and PA-8 in 7.10b (`D-240`, `D-241`), and the other fourteen sit by ID in the redo's §3 rows: PA-12 and PA-13 in 8.1, PA-14, PA-15 and PA-16 in 8.2, and PA-3, PA-4, PA-9 and PA-17 to PA-22 in 8.7."
- **Seam register.** `:3`: "and the Phase 7 plan's tables (`docs/plans/phase-7.md` §2.2–§2.3) its disposition." becomes "and the Phase 7 plan's tables held its disposition (§2.2–§2.3, at `baaecc20`) until Phase 7's close carried the open ones into the [roadmap redo](roadmap-redo.md)'s §3 Phase 8 rows by ID." `:12`: "The dispositions live in phase 7's tables, `ROADMAP.md`'s Later and the levers for 9.2" becomes "The dispositions lived in phase 7's tables until its close; the open S-rows sit by ID in the redo's §3 Phase 8 rows, beside `ROADMAP.md`'s Later and the levers for 9.2". `:14`: "terminal (phase 7's §2.1)" becomes "terminal (landed, closed, `Later` or `9.2`, by the Phase 7 plan's §2.1 at `baaecc20`; the rows open at its close end in the Phase 8 steps whose §3 rows name them)". `:220`: "(`docs/plans/phase-7.md`)" becomes "(the Phase 7 plan, at `baaecc20`)".
- **Genre jam.** `:3`: "writes the other dispositions into phase 7's tables, a Phase 8 step, `ROADMAP.md`'s Later or the kit backlog" becomes "writes the other dispositions into a Phase 8 step's row in the redo's §3, `ROADMAP.md`'s Later or the kit backlog; phase 7's tables, a destination until Phase 7 closed (<date>), took none". `:11`: "the dispositions live in phase 7's tables" becomes "the dispositions live in the redo's §3 Phase 8 rows". `:13`: "triaged once into a phase 7 step's table, a Phase 8 step," becomes "triaged once into a Phase 8 step's row in the redo's §3,". Each "`phase-7.md:N`" (`:66`, `:71` twice, `:86`, `:98`–`:104`) becomes "the Phase 7 plan's line N at `cad63fa5`", the jam's framing commit, where lines 110, 153, 221, 222, 232, 234, 244, 245, 247, 250 and 325 hold S-63, P7-18, S-157, S-163, S-57, S-80, S-188, S-193, P7-17, S-140 and P8-13. §7 gains a bullet after **Proposed**: "- **Past steps** (<date>, Phase 7's close): 7.11, 7.12 and 7.16 closed without the 13 rows proposed for them (7.13g's evaluation found J-1 unbuilt), so they, J-19's kit row and J-14's kit fix wait for the owner's triage against Phase 8's steps (`D-150`); no row moved."
- **Paper designs (F1).** `:21`, step 6: "7.13 and 8.5 take the weekly tasks" becomes "7.13 and 8.6 take the weekly tasks", and the step ends with "Done 2026-10-29 in the Phase 7 plan (§2.2, §2.3, §2.6); at its close (<date>, its last content at `baaecc20`) the open Phase 8 rows went into the [roadmap redo](roadmap-redo.md)'s §3 rows by ID." `:143`: "for 7.13's and 8.5's evaluations" becomes "for 7.13's and 8.6's evaluations".
- **The bench comment.** `piles.rs:3`: "//! 7's piles, `docs/plans/phase-7.md` §2.6)." becomes "//! 7's piles, `docs/perf/benchmarks.md`'s `friction` and `slopes`).", the maintained doc that describes both rows (`docs/perf/benchmarks.md:274`).

### 2.3 R7 on the redo's rows (Step 3)

- `c_row_lines` (`scripts/preflight.py:1346`) reads `docs/plans/roadmap-redo.md`: a line opening with a Phase 8 row's cell (`^\|\s*8\.\d+\s*\|`) that names one of the plan's IDs, matched with `ROW_ID`'s guards (`(?<![\w-])` and `(?![\w-])`), yields the line and its first such ID; the loop is today's:

  ```python
  # Phase 8's rows name each register row by ID since phase 7's tables closed.
  regex = re.compile(r"^\|\s*8\.\d+\s*\|.*?(?<![\w-])(" + "|".join(map(re.escape, ids)) + r")(?![\w-])")
  for rel in ctx.corpus.match("docs/plans/roadmap-redo.md"):
  ```

  The redo's only other `| 8.N |` lines are §8's evaluation rows (`:547`, `:548`), which name no register ID.
- R7's `reason` becomes "the redo's §3 Phase 8 rows name each register row by ID; a step that moves one rewrites both rows", its `stop` "ui-pass step 16 (departure): phase 7's S-162 row, which the redo's 8.1 row holds since phase 7's close", and the parser's help example `docs/plans/<topic>.md`.
- `test-preflight.py` gains `test_register_rows_read_the_redos_phase_8_rows`: it writes a redo fixture of three lines (an 8.1 row naming P8-1 and PA-12, an 8.2 row naming P8-13, and a prose line naming P8-1) and a plan naming P8-1 (`QUIET` with that clause), and asserts that R7 fires and that the fixture's line 1 alone carries R7's source.

### 2.4 The close (Step 4)

- **Checks,** on the tree Step 3 leaves: `df -h target` first, and under 20 GB free `just clean-target --apply` before the build; `just check`; `git diff --check baaecc20 HEAD`; §2.5's recheck group again; and the demo.
- **The demo,** `target/phase-7-close/demo.json`, run from the game's folder as A14 ran it:

  ```json
  {
    "version": 1,
    "frames": 420,
    "resolution": [1280, 720],
    "steps": [
      { "frame": 2, "action": "start_run" },
      { "frame": 150, "assert": { "probe": "player", "path": "/health/0", "op": ">", "value": 0 } },
      { "frame": 180, "action": "reload_run", "args": {} },
      { "frame": 400, "assert": { "probe": "player", "path": "/health/0", "op": ">", "value": 0 } }
    ]
  }
  ```

  `cargo build -p tungsten-cli --locked`, then `cd examples/01_platformer && ../../target/debug/tungsten run --script ../../target/phase-7-close/demo.json --report ../../target/phase-7-close/demo-report.json`. The close changes nothing drawn, so it takes no capture or `render` sheet.
- **Two commits.** First this file's Step 4 evidence row, quoting the checks and the demo, alone. Then `ROADMAP.md`, the redo's session-log sentence and the deletions, its checks run on the staged tree before that commit and quoted in the session's report, since the log is gone with the file.
- **The redo's session log** in the second: Step 1's entry ends with "The plan was deleted the same day, its last content at `baaecc20`." (or the deletion's date, should it fall on another day).
- **`ROADMAP.md`** in the second: the `phase-7` and `phase-7-close` Now lines and Later's Phase 7 line deleted; Later's Phase 8 line becomes Now's one item, its link relative to the root:

  ```markdown
  - `phase-8`: Phase 8, Genres, a step at a time on one track in [the roadmap redo](docs/plans/roadmap-redo.md)'s §3 order: 8.1 story systems, 8.2 shooter and AI systems, 8.3 tooling for hand-made content, 8.7 ARPG systems (after 8.1 and 8.2, beside 8.3 and 8.4), 8.4 01 refreshed, 8.5 the template's final pass, 8.6 proof; each step's row names the register rows it takes by ID, carried there when Phase 7 closed (<date>) with every row of its own landed, closed or parked; 8.1's plan is next.
  ```

### 2.5 Checks

Named so the steps cite them; each is a local command and its expected result.

- `base`: `git log -1 --format=%h -- docs/plans/phase-7.md` → `baaecc20`.
- `open`: `rg -n '^\| [^|]+ \| [^|]+ \| 7\.[0-9]+ \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0.
- `terminal`: `awk '/^### 2\.2 /{on=1;next} /^### 2\.3 /{on=0} on && /^\| [A-Z]/ && !/^\| ID /' docs/plans/phase-7.md | rg -v '^\| [^|]+ \| [^|]+ \| [^|]+ \| (landed|closed|Later|9\.2)\b'; test $? -eq 1` → exit 0, and the same `awk` piped to `rg -c '^\|'` → 300.
- `later`: `rg -o -r '$1' '^\| ([A-Z0-9-]+) \| [^|]+ \| [^|]+ \| Later\b' docs/plans/phase-7.md | while read -r id; do awk '/^## Later/{on=1} on' ROADMAP.md | rg -q "\b$id\b" || echo "$id"; done` → prints nothing; the pattern's `rg -c` → 22.
- `levers`: the same loop on `\| 9\.2\b` against `awk '/^## Levers for 9\.2/{on=1;next} /^## /{on=0} on' docs/perf/benchmarks.md` → prints nothing; the pattern's `rg -c` → 2.
- `items`: `rg '^\| [CG][1-6] \|' docs/plans/phase-7.md | awk -F' [|] ' '{print $NF}' | rg -v '^(done|stopped|dropped)'; test $? -eq 1` → exit 0; `rg -c '^\| [CG][1-6] \|' docs/plans/phase-7.md` → 12.
- `carry`: `rg -o -r '$1 $2' '^\| ([A-Z0-9-]+) \| [^|]+ \| (8\.[0-9]+) \| open \|' docs/plans/phase-7.md | while read -r id step; do rg -q "^\| ${step} \|.*\b${id}\b" docs/plans/roadmap-redo.md || echo "$id $step"; done` → prints nothing (21 lines today); the pattern's `rg -c` → 37.
- `step17`: the Phase 7 plan's own, `rg -o -r '$1' '^\| ([A-Z0-9-]+) \| [^|]+ \| 8\.[0-9]+ \| open \|' docs/plans/phase-7.md | while read -r id; do rg -q "$id\b" docs/plans/roadmap-redo.md || echo "$id"; done` → prints nothing.
- `note`: `rg '^\| 8\.1 \|' docs/plans/roadmap-redo.md | rg -c 'P8-8'` → 1.
- `kept`: `git show HEAD:docs/plans/roadmap-redo.md | rg '^\| [78]\.[0-9]+ \|' | rg -v '^\| (7\.13|8\.[1237]) \|' > target/phase-7-close/kept-head.txt`, the same on the working file into `kept-now.txt`, `cmp` of the two → exit 0.
- `cited`: `rg -l 'phase-7\.md' --glob '!CHANGELOG.md' --glob '!docs/plans/phase-7.md' --glob '!docs/plans/phase-7-close.md' --glob '!ROADMAP.md' .; test $? -eq 1` → exit 0, once Steps 1–3 have all landed (Step 4).
- `cited-registers`: `rg -q 'phase-7\.md' docs/plans/arpg-design.md docs/plans/seam-register.md docs/plans/genre-jam.md docs/plans/paper-designs.md examples/02_bench/src/piles.rs; test $? -eq 1` → exit 0, Step 2's own files alone.
- `cited-tools`: `rg -q 'phase-7\.md' scripts/preflight.py scripts/test-preflight.py; test $? -eq 1` → exit 0, Step 3's own files alone. Each holds whichever of Steps 2 and 3 ran first.
- `homes`: `rg -n "dispositions live in phase 7's tables|terminal \(phase 7's §2\.1\)" docs/plans/seam-register.md; test $? -eq 1` → exit 0, and `rg -n "into phase 7's tables|dispositions live in phase 7's tables|a phase 7 step's table" docs/plans/genre-jam.md; test $? -eq 1` → exit 0 (each matches today).
- `f1`: `rg -q '8\.5' docs/plans/paper-designs.md; test $? -eq 1` → exit 0; `rg -c '^  6\. .*Done' docs/plans/paper-designs.md` → 1.
- `probe`: `target/phase-7-close/probe-plan.md`, a draft plan with this plan's headers, files to touch `docs/quiet-probe.md` and one step whose change names P8-1 and S-6; `python3 -B scripts/preflight.py target/phase-7-close/probe-plan.md --json` → R7 fired, its `files` `["docs/plans/roadmap-redo.md"]`, its rule hits on lines 160 and 161 (A9's form).
- `demo`: §2.4's run → exit 0, `tungsten run: 420 frames, 0 captures, 2 asserts: passed`, the report's `failed_steps` and `unreached` empty.

## 3. Steps

### Step 1: Recheck and carry

- **Files:** `docs/plans/roadmap-redo.md`; this file.
- **Change:** set this file's status to `in progress`; run `base`, `open`, `terminal`, `later`, `levers` and `items` before any edit; then §2.1.
- **Done-when:** before the edits, those six as §2.5 states them; after, `carry` and `step17` → print nothing, `note` → 1, `kept` → exit 0, `rg -q 'phase-7\.md' docs/plans/roadmap-redo.md; test $? -eq 1` → exit 0, `just repo-check` → passes.
- **Commit:** `docs(plans): carry phase 7's open rows into the redo's phase 8 rows`
- **Moves:** the redo's 8.1, 8.2, 8.3 and 8.7 rows, its paragraph after them, its 7.13 row's citation and its session log.

### Step 2: The registers and the bench comment

- **Files:** `docs/plans/arpg-design.md`, `docs/plans/seam-register.md`, `docs/plans/genre-jam.md`, `docs/plans/paper-designs.md`, `examples/02_bench/src/piles.rs`; this file.
- **Change:** §2.2.
- **Done-when:** `cited-registers` → exit 0; `homes` and `f1` as §2.5 states them; `cargo fmt --all -- --check` → exit 0; `cargo check -p example-02-bench --locked` → exit 0; `just repo-check` and `just ctx` → pass.
- **Commit:** `docs(plans): point phase 7's citations at the redo and its last commit`
- **Moves:** the lines §2.2 names, no others.

### Step 3: Preflight's R7

- **Files:** `scripts/preflight.py`, `scripts/test-preflight.py`; this file.
- **Change:** §2.3.
- **Done-when:** `python3 -B scripts/test-preflight.py` → `OK`, the new test among those run; `cited-tools` → exit 0; `probe` as §2.5 states it; `just script-test` → exit 0; `just repo-check` → passes.
- **Commit:** `fix(preflight): read register rows from the redo's phase 8 rows`
- **Moves:** R7's source, reason, stop and help text; one test added.

### Step 4: Close

- **Files:** this file (its evidence row, then deleted), `docs/plans/phase-7.md` (deleted), `ROADMAP.md`, `docs/plans/roadmap-redo.md` (the deletion's sentence in its session log).
- **Change:** §2.4, in its order: the checks and the demo, the evidence commit, then `ROADMAP.md` and the deletions. Preflight names 02's guard pins for `piles.rs` (Step 2); a doc comment moves none, and `just check` runs them.
- **Done-when:** first, `just check` → exit 0, `git diff --check baaecc20 HEAD` → exit 0, `base`, `open`, `terminal`, `later`, `levers`, `items`, `carry` and `step17` as §2.5 states them, `cited` → exit 0, and `demo` → passed. Then, on the second commit's staged tree: `just repo-check` and `just ctx` → pass; `rg -l 'phase-7\.md' --glob '!CHANGELOG.md'; test $? -eq 1` → exit 0; `rg -l 'phase-7-close'; test $? -eq 1` → exit 0; `awk '/^## Now/{on=1;next} /^## /{on=0} on' ROADMAP.md | rg -c '^- '` → 1, that item `phase-8`.
- **Commits:** `docs(plans): record phase 7's close checks and demo`, then `docs(plans): close phase 7 and move phase 8 into now`.
- **Ends with:** the commits, the evidence, what the owner should try (nothing new to play: 01 as 7.13 left it; the redo's Phase 8 rows to read), the genre jam and a release as heads-up lines (Q3, Q4), and the next prompt, 8.1's plan, under tungsten-next's label line: `Use the tungsten-plan skill. Write the plan for phase 8's step 8.1 (Story systems) from ROADMAP.md: read docs/plans/roadmap-redo.md's §3 8.1 row and the sections and registers it cites; audit only the code it touches; name it docs/plans/<topic>.md; critique it last as the skill says; commit it and end with the run prompt.`, its register list filled in.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | What enters Now when `phase-7` leaves? `just repo-check` refuses an empty Now | `phase-8`, from Later's Phase 8 line, a step at a time on one track as Phase 7 ran (§2.4); Later's Phase 7 line goes |
| Q2 | R7, once its tables are gone: retarget or retire? | Retarget to the redo's §3 Phase 8 rows (§2.3): a Phase 8 step that moves a row rewrites two of them, the failure R7's stop names; retired, that would be found by the step's own stop |
| Q3 | The genre jam's 13 rows proposed for closed steps | Untriaged, for the owner (`D-150`): the close records the closed steps in the jam's §7 and names it in the hand-off; no row moves |
| Q4 | A release at Phase 7's end (`0.120` holds 7.16's guidance) | None in the close; a release is the owner's request (the plans README); the hand-off names it as a heads-up |
| Q5 | How a Phase 8 step records the rows it lands | 8.1's plan sets it; the close only places the rows |

## 5. Invariants

- **May move:** the redo's 8.1, 8.2, 8.3 and 8.7 rows, its paragraph after them, its 7.13 row's citation, `:717` and its session log (Step 1); the lines §2.2 names in four registers and `piles.rs`'s comment (Step 2); R7's source, reason, stop and help text, with one test added (Step 3); `ROADMAP.md`'s Now and Later, and the redo's session-log entry's last sentence (Step 4).
- **Must not move:** the Phase 7 plan's content until its deletion (`base`); the redo's other rows and sections (`kept`); every test's outcome; every other preflight rule and pin; `DECISIONS.md`, `docs/DECISION_INDEX.md`, `CHANGELOG.md`, `docs/known-issues.md` and the frozen 1.0 records; any other code, asset, row digest or hash.

## 6. Stop conditions

- A done-when fails: the skill's rule (restore the step's files, mark it skipped, carry on with what does not depend on it).
- Step 1's recheck fails (a Phase 7 row open, a `Later` or `9.2` row without its home, an item open, or the plan changed since `baaecc20`): the run stops, since the Phase 7 plan's done-when is not met; nothing is edited.
- Step 4's `just check` or the demo fails: stop before the second commit, keeping both plans, and quote the failure; one the run's edits could not cause (docs, a comment and preflight changed) is rechecked on `baaecc20` and reported as found there.
- Steps 2, 3 and 4 depend on Step 1, whose carry the deletion needs; Steps 2 and 3 are independent of each other (each checks only its own files); Step 4 needs all three.
- When Step 4 stops, what Steps 1–3 committed stays true: it records the carry and the pointers, and the deletion's sentence comes only with the deletion.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-11-25, reading the uncommitted plan on `baaecc20`. A first attempt stopped with its session at 35% → 38% of the 5-hour window and wrote nothing; the rerun's footer: `242.7 s, tokens in 579926 (cached 509824), out 7231 (reasoning 4146), commands 15, output 385.3 KB (largest 84.6 KB), usage 5h 38% → 54%, week 34% → 36%`. Two findings, both accepted; they changed a done-when check and Step 4's file list, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Step 1 records a completed deletion before the close can succeed | §2.1's session-log entry ended "the plan was deleted", committed in Step 1, while §6 lets Step 4 stop with both plans kept | accepted | The entry records the carry alone; the deletion's sentence joins it in Step 4's last commit, with `docs/plans/roadmap-redo.md` in Step 4's files (§2.1, §2.4, §5); §6 says Steps 1–3's records stay true when Step 4 stops |
| 2 | [MEDIUM] Step 3's acceptance check contradicts its independence from Step 2 | Step 3's done-when ran `cited` → exit 0, which needs Step 2's register and comment edits, while §6 called Steps 2 and 3 independent | accepted | Step 3 checks `cited-tools`, its own two files (§2.5); the whole `cited` → exit 0 moves to Step 4's first group, which needs all three steps |

Round 2: the same reviewer at xhigh, 2026-11-25, reading the plan with this section and the log cut and a focus note on round 1's two changes. Footer: `182.5 s, tokens in 370348 (cached 312064), out 5739 (reasoning 3494), commands 21, output 291.6 KB (largest 70.9 KB), usage 5h 55% → 71%, week 37% → 39%`. One finding, a gap round 1's second fix left on the other side, accepted; no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Step 2 cannot pass unchanged when resumed after Step 3 | Step 2's done-when expected `cited` to list `scripts/preflight.py` alone, which Step 3 empties (`scripts/preflight.py:178`, `:1351`, `:1734`), so a Step 2 retried after Step 3 fails on correct edits | accepted | Step 2 checks `cited-registers`, its own five files (§2.5); `cited` reads exit 0 only in Step 4 |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups

- 2026-11-26, the QA pass (`D-314`): `docs/plans/1.0/` and `docs/plans/archive/` are deleted by its records commit, their last content at `2f0ffeb0`. This plan's files keep their citations of them as notes, for Step 1's recheck: `phase-7.md:25` (`example-01-feedback.md`); `roadmap-redo.md:17`, `:31`, `:33` and `:43` (the 1.0 records and the Phase 7 rewrite), its 9.7 row now reading done; `seam-register.md:19`, `:95`, `:133`, `:295`, `:454` and `:455`; `genre-jam.md:11`, `:84` and `:104`; `paper-designs.md:170`–`:355` (its `backlog-1.x.md` and `example-01-feedback.md` evidence); and `scripts/preflight.py:16`, `:33` and `:328`, which skip the deleted archive.
