# Seam register (redo 7.5)

Every place example 01 and the template compensate for the engine, kit or template, with evidence, ranked by the code a generic fix would delete and how often the pattern recurs: the roadmap redo's analysis pass (`docs/plans/roadmap-redo.md`, step 7.5, its brief in §7.1). It keeps its register as the paper designs and the ARPG design keep theirs: this file holds each row's evidence, and the Phase 7 plan's tables (`docs/plans/phase-7.md` §2.2–§2.3) its disposition. Framed at `3b01712d`.

- **status:** draft
- **goal:** Every seam in 01's `src/`, `tests/` and `tools/` and in the template as a verified row: the code a generic fix would delete, how often the pattern recurs, what the engine, kit or template lacks, the fix and its prerequisites; ranked, and each row triaged once into phase 7's tables, so the step that builds each system takes its seams.
- **non-goals:**
  - Fixes, and edits to code, tests, tools, assets, data or `api/`: the steps that build each system make them.
  - The paper designs' and the ARPG design's rows, and a K-row without new evidence: their 01 sites go to §4, not into new rows.
  - Bugs beyond one verified line each (§5); known issues takes them.
  - A second reading or verification round.
- **files to touch:** This file, by redo 7.5's run. The dispositions live in phase 7's tables, `ROADMAP.md`'s Later and the levers for 9.2 in `docs/perf/benchmarks.md` (§7). After 7.5, only the commit that deletes this file.
- **ordered steps:** (1) the method and seeds (§1–§2); (2) nine readings and their merge, kept in the run's folder (§1.5); (3) verification (§3–§6); (4) triage (§3's Step column, §7); (5) the close (this status, §5's filed marks).
- **done-when:** §3's rows read, merged, verified and triaged, each S-ID once in phase 7's tables (its Step 4's count). After 7.5 this file stays `in progress`. Once every S-row and every ID its §4 names is terminal (phase 7's §2.1), one commit ends each `Later` and `9.2` line naming one of them with "the seam register at `<sha>`", this file's last commit, and deletes this file with its citations; that pointer names no `docs/plans/` path, which the citation check would fail once the file is gone.

## Context digest

- Phase 7 builds each system once, in the step its rows name (phase 7's §2.1). 7.5 runs before 7.8 so its rows reach the steps that build them (phase 7's §2.8); 7.2–7.4 have landed and no UI-track step has started.
- Its seeds (§2): the feedback plan's K-rows (`docs/plans/1.0/example-01-feedback.md` §6), the backlog rows citing 01, known issues and Later's example 01 lines, the redo's §1 figures retaken, and what 7.1–7.4 and `qa6` already fixed. The paper designs' (P7-, P8-) and the ARPG design's (PA-) rows are registered in phase 7's tables and are not re-reported.
- Decisions: `D-150` (no session stops for approval, so the triage takes its defaults), `D-197` (01's shell is the template's, read once), `D-215` (the redo), `D-226` (documents take no benchmark sitting).
- Readers: 7.8–7.12 and 7.17–7.20 take their systems' rows, 7.13 the rows only 01 needs, 7.14 and 7.15 the performance evidence, Phase 8's steps their rows; each reads this file's evidence for its rows.

## 1. Method

### 1.1 Scope and shares

At `3b01712d`, 157 files and 86,471 lines (`wc -l`): 01's `src/` without its shell (78 files, 28,389 lines), `tests/` (31, 24,658) and `tools/` (14 Python files and one shader, 22,078); the template's `src/` and `build.rs` (21, 4,448), `tests/` (9, 5,940) and `tools/` (3 Python files, 958). 01's `src/shell/` (12 files, 3,627 lines) is the template's byte for byte (`D-197`; the repository check's `check_shell_copies`), so it is read once, in the template's share. 01's generated `assets/`, its `tools/` data and the template's assets are consulted, never read whole.

Nine readers, one share each. `f/` is 01's `src/features/`; 01's paths are under `examples/01_platformer/`; tests are `tests/game/<name>.rs`.

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

A reader reads every line of its share once, in ranges of at most 400 lines, and keeps a ledger line per file (`- <path>: 1-<n> of <n>`). The engine and kit are read only to check a row's Lacking claim: `api/*.txt` first, then a targeted search in `crates/`.

### 1.2 Row shape

One Markdown table row per seam, the same in the readers' files, the merge and §3:

`| ID | Pattern | Where | Sites | Lines | Lacking | Generic fix | Depends on | Lens | Step |`

- **ID:** a reader's `<share>.<n>`, the merge's `m<n>`, this register's `S-<n>`.
- **Pattern:** the compensation, short. A row that re-reports a K-row ends with `(K<n>)`.
- **Where:** the code the generic fix would delete, as `path:a-b` or `path:a`, comma-separated. 01's paths are from `examples/01_platformer/` (`src/…`, `tests/…`, `tools/…`); a path starting `templates/` is from the repository root.
- **Sites:** how many places the pattern recurs, an integer, then the search that reproduces it when one does (`6: rg -n 'tick -= dt' src/features`).
- **Lines:** the sum of Where's ranges.
- **Lacking:** what the engine, kit or template lacks, ending with its evidence: `none: '<pattern>'` (no match in `api/*.txt` or `crates/*/src`), the item that falls short (`crates/…:line`), or `has: <item> (crates/…:line)` when the engine or kit already has it and only 01 does not use it.
- **Generic fix:** the smallest engine, kit or template change that deletes Where's code; for a `has:` row, 01's use of that item.
- **Depends on:** prerequisites only, as S-, K-, P7-, P8- or PA- IDs or redo steps; `—` when none. A split ID names its part as phase 7's Pattern does: `P7-2 (physics)` or `P7-2 (kit)`, `P7-3 (sensor exit)` or `P7-3 (Interactable)`. A seed a row matches is provenance, kept in §2's Match cells, not here.
- **Lens:** one of §1.3's ten.
- **Step:** `—` until the triage writes the disposition (§7); a `+` after it when the step's sketch in the redo's §3 does not list the fix.

A finding that a registered row's fix would delete (§2.7) is an entry in §4, with the registered ID, its ranges, sites and lines and the reader's row cells, never an S-row; a K-row with new evidence (more sites or lines than its feedback row, or a home that changed) is the one exception, an S-row ending `(K<n>)`. A bug a reader meets is one line with its `file:line`, never chased (§5).

### 1.3 Lenses

| Lens | Compensation |
| --- | --- |
| `copy` | Helpers copying engine or kit logic |
| `data` | Rust where data belongs |
| `tags` | Markers where tags belong |
| `timer` | Hand-ticked timers |
| `arms` | Per-kind arms |
| `const` | Constants restating data |
| `boiler` | Save and ordering boilerplate |
| `perf` | Performance workarounds |
| `pos` | Positions copied into tests |
| `tools` | Duplication in the tools |

### 1.4 Counting and ranking

- A line sits in one row only (or one §4 entry). Lines count what the fix deletes, tests included when the fix deletes them, and do not subtract the call that replaces it.
- Rank: Lines, then Sites, both descending, then share order (the share holding most of the row's lines).
- IDs: `S-1` … in rank order, assigned once after verification and never renumbered.

### 1.5 The run

The run's folder, `perf-runs/20261030-analysis-pass/`, is local (`perf-runs/` is gitignored): the prompts sent, the nine readers' files with their ledgers, the merge (`merged.md`), the verifiers' files, and the scripts. `shares.py --check` proves every in-scope file sits in one share, and `--ledger` that each reader's ledger covers its share at its whole `wc -l`. `check.py` refuses a row with a cell missing, a repeated ID, a range outside its file, a line in two rows, or Lines unequal to its ranges' sum; `--sources` adds that every reader row is accounted for once in the merge, `--verdicts` that every merged row, §4 entry, seed fixed upstream and bug has one verifier line, and `--register` that §3's S-IDs run from `S-1` in rank order, each mapped in §6.

## 2. Seeds

Each reader's prompt carries the seeds; a row that matches one says so in that seed's line (`<seed> → <row ID>`), which the merge carries and step 3 writes into the Match cells below by S-ID.

### 2.1 K-rows

The feedback plan's kit and engine gaps (`docs/plans/1.0/example-01-feedback.md` §6), with phase 7's dispositions. For each K-row whose workaround sits in its share, a reader reports whether 01's workaround stands, grew, is gone, or the gap was fixed upstream, with `file:line`. State: written at step 3.

| K | Gap | Phase 7 | 01's workaround (feedback §6) | Shares | State |
| --- | --- | --- | --- | --- | --- |
| K1 | Cue rules can't tell a periodic tick from a hit | 7.10 (7.10b), open | Hurt played from code on non-periodic hits | 4 | |
| K2 | A particle config's draw depth | 7.9, open | Each spray's newborn particles set to z 11 the frame after they spawn | 3, 4 | |
| K3 | A hitbox that follows its owner's facing | 7.10 (7.10c, in P7-20), open | The troll's swipe as 01's box hit ahead of it, mirrored by its heading | 1, 4 | |
| K4 | An animation's playback rate | 7.9, open | The chill as a graded frost tint, clips at their own rate | 1, 2, 4 | |
| K5 | UI text outline or shadow | 7.9 as P7-14, open | Keys on a dark backing | 3, 4, 5 | |
| K6 | `World::query` matched every archetype per call | Fixed in M77 (`D-206`) | Queries hoisted | all | |
| K7 | A controller pushed by bodies | 7.8, open | The wizard's and walkers' shoves (`with_shove`) | 1, 4 | |
| K8 | `charges { all: true }` refills from the first charge | 7.10 (7.10c, in P7-6), open; fixed by `4d51a071` (`qa6`, in 0.112) | 01 times its reload | 3, 4 | |
| K9 | Lit sprites without the hit-flash material | 7.9, open | A tint; enemies drawn unlit through `damage_flash` | 1, 4 | |
| K10 | A radial blast payload | 7.10 (7.10b), open | `CascadeKind::Detonation` in 01's saved queue; hand-rolled blasts | 1, 2, 3, 4 | |
| K11 | Timers with expiry and a saved form | 7.10 (7.10b), open | 01's own saved fuse and burn fields (`Keg`, `Bomb`, `Fuel`) | 2, 3 | |
| K12 | `World::has` scans the type list | 7.14 (C4), open | 01 moved off per-entity `has` | all | |
| K13 | `animation_tick` resolves clips by name | 7.14 (C3), open | — | — | |
| K14 | The query index rebuilt from every body | 7.8 (C1), open | — | — | |
| K15 | An immune part sent no event | Fixed in M78 (`D-207`) | 01's reactions read `ImmunePart` | 2 | |
| K16 | A contact's push stopped with its damage | Fixed in M79 (`D-208`) | Lava throws on `with_push_always` | 5 | |
| K17 | The controller reported only `grounded` | Fixed in M79 (`D-208`) | The elevator boards on `ground` | 5 | |
| K18 | No step height | Fixed in M84 (`D-213`) | Walkers step over balls | 1, 4 | |
| K19 | The magnet and a falling bodiless pickup | 7.10 (7.10c), open | 01's fall for a drop | 1, 4 | |
| K20 | A top-down mover's outside velocity; additive knockback | 7.10 (7.10c), open | `toys_hold` and `toys_release` steer a flyer in a hole | 3 | |
| K21 | An emitter's rate and direction at run time | 7.9, open | A flakes emitter per frost stage; the wave's dust and grit as bursts | 2, 3, 4 | |
| K22 | A hotkey for `KitDebug`; hit shapes without the trace | 7.10 (7.10c), open | 01 mirrors F1 into `KitDebug` | 1, 4 | |
| K23 | Kinematic pairs never touch | 7.8, open | The elevator sets a player out of its deck and turns back on a swept box | 5 | |
| K24 | The animator read the ground's carry | Fixed in M83 (`D-212`) | — | — | |
| K25 | Air jumps priced, refused, counted at run time | Later (cost and refusal landed in `e01tomes`) | — | 3, 4 | |
| K26 | An ability's cost scaled by a stat | Landed (0.109, `e01tomes`) | — | 3 | |
| K27 | `Damaged` hides a shield's absorption | 7.10 (7.10b), open | Shield hits read as hits | 3, 4 | |
| K28 | Every non-periodic hit flashes over a status tint | 7.10 (7.10b), open | The frost white and subtle under the flash | 2, 4 | |
| K29 | A cue sound's position and falloff | Landed (0.110, `e01sfx`) | 01's listener at the player | 4 | |
| K30 | A locked controller stops dead at a body | 7.8, open | The beetle's charge relaunches each step | 1 | |
| K31 | A spatial filter by body kind or mass | 7.8, open | Sight passes small bodies in a loop | 1 | |

### 2.2 Backlog rows citing 01

`docs/plans/1.0/backlog-1.x.md`, 29 rows (`rg -c '^\|.*(examples/01_platformer|[Ee]xample 01|01 \()'`). Match: written at step 3.

| Line | Row | Match |
| --- | --- | --- |
| 36 | Tile layers ordered among sprites by z | |
| 82 | Line-of-sight sensing in the kit | |
| 83 | Stereo panning per voice | |
| 84 | Looping kit sound cues that keep their voice | |
| 85 | A same-step order that survives a load | |
| 86 | A `place` payload for abilities | |
| 87 | One-shot clips for the character animator | |
| 88 | `Lifetime` in saves | |
| 89 | The pad's aim on the kit's `Aim` | |
| 90 | A chain-reaction queue in the kit | |
| 91 | Impact damage by speed and mass | |
| 92 | A conduction payload | |
| 93 | A cue filter for a hit's kind (K1) | |
| 94 | A cue filter for a killing blow | |
| 95 | A draining effect | |
| 97 | A lit sprite that flashes (K9) | |
| 98 | A kit hit that carries its angle | |
| 99 | A cue event for a projectile's wall meetings | |
| 100 | Materials on tiles | |
| 101 | A particle config's draw depth (K2) | |
| 102 | A playback rate on the animation state (K4) | |
| 103 | A controller pushed by the bodies it meets (K7) | |
| 104 | A hitbox that follows its owner's facing (K3) | |
| 105 | A radial blast payload (K10) | |
| 106 | A pickup that falls; a magnet for a pickup with a body (K19) | |
| 107 | A key for `KitDebug`; hit shapes without the trace (K22) | |
| 108 | Outside velocity on a top-down mover (K20) | |
| 109 | An emitter whose rate and direction change (K21) | |
| 110 | A hit that does not flash (K28) | |

### 2.3 Known issues citing 01 or the template

`docs/known-issues.md`, 11 lines (`rg -c 'examples/01_platformer|templates/basic'`). Match: written at step 3.

| Line | Entry | Shares | Match |
| --- | --- | --- | --- |
| 29 | A fire hazard's touch times left out of 01's save | 5, 8 | |
| 30 | `gun_hits` runs after the kit's projectile pass | 4 | |
| 31 | A troll facing left draws 20 px right of its transform | 1, 4 | |
| 36 | The autosave rotation parses every slot whole | 9 | |
| 37 | A bat sighting on its way home skips Alert | 1 | |
| 38 | A schema 11 save loads without the map's crates and kegs | 4, 8 | |
| 39 | M81's frame readings are indicative | 4 | |
| 40 | `frame_time.rs` fails deterministically at 0.113 | 4 | |
| 41 | `ball_look`'s `powf` a frame per ball | 3 | |
| 102 | The root sprites and sounds have no recorded origin | 9 | |
| 104 | `spread_fire` rebuilds a cell map each step | 2 | |

### 2.4 Later lines naming example 01

`ROADMAP.md`'s Later, 7 lines (`rg -c '^- Example 01'`). Match: written at step 3.

| Line | Line's subject | Match |
| --- | --- | --- |
| 15 | The route test's `LAUNCHES` rows over the next step's face | |
| 16 | Snowballs dousing props and chilling bodies | |
| 17 | Crates breaking into planks | |
| 18 | Geysers lighting, throwing or chilling props and balls | |
| 19 | Props' and toys' boxes and anchors as data beside their sprites | |
| 20 | Tile variants' per-cell overrides | |
| 21 | The ribcage prop off the troll bridge | |

### 2.5 The redo's §1 figures, retaken

At `3b01712d`; no code changed since `e76c32f0`.

| Figure | The redo's §1 (`bb1e4c0`) | `3b01712d` | Command |
| --- | --- | --- | --- |
| `has::<` checks in 01's `src/` | 134 | 161 | `rg -o 'has::<' examples/01_platformer/src \| wc -l` |
| `TagFilter` in 01's Rust | absent | absent | `rg -c TagFilter examples/01_platformer/src` → exit 1 |
| 01's `save.rs` | 1,989 | 2,873 | `wc -l` |
| 01's `src/` (shell included, 90 files) | 27,051 | 32,016 | `wc -l` |
| 01's `tests/` | 18,933 | 24,658 | `wc -l` |

### 2.6 Done at 7.1–7.4 and `qa6`

A seam these fixed is marked done in the reader's file, not reported.

- **0.112** (`qa6` and 7.1; `CHANGELOG.md`'s 0.112.0 section): the deprecated surface (W4b); 01's authored map IDs (P7-9, `D-222`), asset families and locks (`D-223`), bodies as data (`assets/data/bodies/`) and a tileset of the tiles its map uses; one integration-test binary per crate; gamepads (P7-12, `D-220`, `D-221`); state commands (`D-217`); the kit's all-charges refill counted from the last charge spent (K8, `4d51a071`) and a whole knockback resist; and the release's other fixes.
- **7.2–7.4,** the commits since `671cae00` (the 0.112 cut): `tungsten run --script`, `render` and `describe` (`2ecd7d41` to `6e642b4d`), with 01's and the template's script hooks (`831811b0`); world counters and counter guards, 01's pit among them (`eea02d45`, `a920cc49`); the benchmark runner (`56b49dfa` to `47bac794`); H2's status script, release script and skills (`a49de384` to `b1ed7a26`). In 01 and the template they touched `src/features/run/` (`mod.rs`, `pit.rs`, `script.rs`), `src/features/level/mod.rs`, `tests/game/frame_time.rs`, `guards.rs` and `main.rs`, `tools/README.md`, and the template's `src/sample/` (`mod.rs`, `script.rs`) and `tests/game/` (`main.rs`, `script.rs`).

### 2.7 Registered rows

Phase 7's §2.2 and §2.3 (`docs/plans/phase-7.md`): the P7-, P8- and PA- rows, from the paper designs' and the ARPG design's registers, and the K-rows of §2.1 without new evidence. A finding one of their fixes would delete is a §4 entry, never an S-row; a K-row with new evidence is an S-row ending `(K<n>)` (§1.2).

## 3. Register

Rows `S-1` … in rank order (§1.4), written at step 3; the Step column at step 4.

| ID | Pattern | Where | Sites | Lines | Lacking | Generic fix | Depends on | Lens | Step |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |

## 4. Registered elsewhere

Written at step 3.

## 5. Bugs noted

Written at step 3.

## 6. Verification

Written at step 3.

## 7. Triage

Written at step 4.
