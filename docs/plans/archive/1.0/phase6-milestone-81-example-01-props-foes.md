# Phase 6 milestone 81: example 01 props and foes (01 (g)) — draft

- **status:** done
- **goal:** Example 01's (g) stop ([feedback](../../1.0/example-01-feedback.md) §3.3): pine crates that catch fire, burn down and fall in, and red powder kegs on a fuse that blow up and set each other off, through one sourced detonation the goblin's bomb shares; a goblin sapper that throws rocks on an arc and, one throw in five, a lit bomb; the troll's quicker slam with a dust ring, a grunt and a shake, a fast swipe in front of it and a kick by mass; an alert mark, a search mark and one sting per alert; crates, kegs and goblins on the map and in the summon slot; save schema 12.
- **non-goals:** Kit, engine, template or shell changes: K3, K10 and K11 stay 01 code (§2.1), and a gap found becomes a stop (§7). The oil barrel and troll keg throws (feedback Q16, Q18). (h)'s look: the wizard, the icon family, the HUD and its bars, 01's theme, the title. New spells, slopes, water. Older saves gaining the newly placed crates and kegs (Q12). P2's tint table (the Phase 6 QA pass).
- **files to touch:** under `examples/01_platformer/` unless rooted.
  - Step 1: `tests/game/fixtures/schema11.json` (new); `tests/game/saves.rs` (a capture test added and taken out again, restored by `cp`).
  - Step 2: `tools/palette.json`, `tools/art.py`, `tools/actors.py`, `tools/effects.py`, `tools/props.py` (new), `tools/generate.py`, `tools/sfx.py`, `tools/sounds.json`, `tools/content/data/cue_rules/sounds.json`, `tools/outputs.json`, the generated `assets/` (sprites and their maps, animations, particles, sounds, `manifest.json`); `src/features/audio/components.rs`, `src/features/audio/mod.rs`.
  - Step 3: `src/features/props/{mod.rs,components.rs,systems.rs,draw.rs}` (new), `src/features/mod.rs`, `src/game.rs`; `src/features/toys/{mod.rs,systems.rs}`; `src/features/elements/{mod.rs,systems.rs,cascade.rs}`; `src/features/combat/systems.rs` (the reactions' credit); `src/features/gun/systems.rs`; `src/features/lightning/mod.rs`; `src/features/spells/{mod.rs,systems.rs}`; `src/features/enemies/troll.rs` (crates liftable); `src/features/level/lava.rs` (a basin's crust read by the props, if it is not public); `src/features/audio/{mod.rs,systems.rs,components.rs}`; `tools/content/prefabs/{crate,keg}.json` (new) and their generated `assets/`; `tests/game/props.rs` (new), `tests/game/main.rs`, `tests/game/enemies.rs`, `tests/game/spells.rs`, `tests/game/dummy.rs`.
  - Step 4: `src/features/enemies/{components.rs,mod.rs,systems.rs,goblin.rs (new)}`; `src/features/props/{mod.rs,components.rs,systems.rs}` (the bomb); `src/features/elements/cascade.rs`; `src/features/spells/mod.rs`; `tools/content/prefabs/{goblin,goblin_rock,goblin_bomb}.json` (new) and their generated `assets/`; `tests/game/enemies.rs`, `tests/game/data.rs`, `tests/game/saves.rs` (the `kind()` arm only).
  - Step 5: `src/features/enemies/{troll.rs,systems.rs,draw.rs,mod.rs,components.rs}` (`Brain.alerted`, `Strike`'s troll variants); `src/features/toys/systems.rs` (the shake's shape shared); `tools/content/data/cue_rules/sounds.json` and its generated copy in `assets/`; `tests/game/enemies.rs`, `tests/game/presentation.rs`.
  - Step 6: `tools/level.json`, `tools/level.py`, `tools/placement.py`, the generated `assets/` (tilemap, `manifest.json`); `src/features/level/mod.rs`, `src/features/run/mod.rs`, `src/save.rs`, `src/features/props/mod.rs`, `src/features/enemies/{mod.rs,goblin.rs}`; `tests/game/{saves,dummy,enemies,level,playthrough,areas}.rs`.
  - Step 7: `README.md`, `tools/README.md`; root `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/example-01-design.md`, `docs/plans/1.0/example-01-feedback.md`.
  - Step 8: the skeleton's release files.
  - Machine-local, never in the tree: `perf-runs/20261023-e01g-art-review/` (this session's sheet), `perf-runs/20261023-e01g-frame/` (steps 1 and 7), the scratchpad's venv and scripts.
- **ordered steps:** (1) Baselines: the schema 11 fixture and a frame reading, before any edit. (2) Art and sound. (3) Crates and kegs. (4) The goblin. (5) The troll and the marks. (6) Placement and save schema 12. (7) Records and the second frame reading. (8) Release 0.102.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, `just level-check` having run last in step 6 and `just check` running the layer 1 manifests test; the preflight prints the command block and the post-merge block.

Candidate 01 (g) props and foes ([implementation plan](../../1.0/implementation-plan.md) §4 card, level B), milestone M81, release 0.102. Written 2026-10-23 at `6773e75` (0.101.0, clean tree). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Sources: the feedback plan's rows C1–C4, T5, I7, I10 and I17's last note, its §3.3 and Q16–Q18; the [design](../../1.0/example-01-design.md)'s shared rules, Assets and Art review. Model tier: Opus 5.5 max by the stop's own `tier` (`D-195`).

## Context digest

Example 01 is a sandbox on the template, rebuilt in stops (`D-194`); this one follows (f) feel (M80, `D-209`, save schema 11). Kit first: a kit gap becomes a backlog row or a level C step, never a workaround; this plan needs none, and maps each new system to its kit module or its reason for staying 01 code (§2.1). The shell stays the template's byte copy.

The keg and the goblin's bomb are one mechanism, built once in step 3: a body with a fuse whose end queues a `Detonation` on 01's saved cascade queue, credited to whoever lit it. Today that queue credits only the run's player (A20). Crates and kegs are dynamic box toys like the iron block, so they join every place iron's code names iron (A4–A15). Fire on them is 01's body state, as the balls' is (`D-200`): the kit's `burning` needs `Health` (A17).

The goblin is a sixth `EnemyKind` on the kit's state machine. Its rock is a bodiless kit `Projectile` carried by `LinearMotion`'s acceleration (A35–A37), its bomb a thrown keg body. The troll keeps its slam for 90–140 px and gains a swipe under 90 px as 01 code, since a kit `Hitbox` cannot follow its owner's facing (K3, A31).

Placing goblins in the map's `enemies` layer moves the save IDs of the placed orbs, the player and everything spawned after them (A45), so schema 12 shifts them as schema 9 did (`SHIFT_9`). The schema 11 fixture that tests the shift must be captured from this tree before any edit (step 1).

The art is drawn on this session's review sheet, `perf-runs/20261023-e01g-art-review/sheet.png` (machine-local), whose README holds every proposal step 2 takes; the owner reviews it with this plan, on their own time (`D-150`).

## 1. Audit

Read at `6773e75`. Paths are under `examples/01_platformer/src/features/` unless rooted; `src/`, `tests/`, `tools/` and `assets/` are under `examples/01_platformer/`, `kit/` is `crates/tungsten-kit/src/`. Three read-only audits covered props, foes and saves; the rows marked † were read again in this session.

### 1.1 Toys, fire and the cascade queue

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Iron: half 60, mass 400, cap 24; `spawn_toy` (private) gives a toy its next `SaveId`, `PreviousVelocity` and the gameplay scene owner, at once † | `toys/mod.rs:54-59`, `:211-228` |
| A2 | `iron_fits` overlaps an `IRON_HALF` box at the last step's places, refusing a tile (a `None` hit), a body that is not dynamic or another block; the spell checks it before the spend † | `toys/mod.rs:353-373`; `spells/systems.rs:225`, `:235-239` |
| A3 | `place_iron` sorts blocks by save ID and despawns the surplus over the cap with a cosmetic `ex10_poof` (dropped at the burst cap); `restore_iron` is a quiet `spawn_toy` † | `toys/mod.rs:375-398`, `:434-437`; `shared.rs:73-127` |
| A4 | `toys_bounds` culls balls, iron, scraps, dummies and enemies past the world's edge; anything else falls forever | `toys/systems.rs:682-702` |
| A5 | Friction marks iron, scraps and dummies in a slot bitmap; frozen friction 120 | `toys/systems.rs:704-772` |
| A6 | The hole's pull response: iron 0.2, scrap 1.0, a frozen dummy 0.2, else the body's inverse mass (a mass-12 crate 0.083); `hole_broom` takes small balls only | `toys/systems.rs:240-257`, `:115-164` |
| A7 | `radial_push` moves dynamic bodies by `away × inv_mass` (the missile's 2,500 px/s gives a mass-40 body 62.5) and live enemies by their kind's pull, skipping the troll | `toys/systems.rs:620-660` |
| A8 | `is_body` is iron, scrap or ball; it gates the chill, the freeze, thermal shatter, `target_hits` and the frozen-impact shatter | `elements/systems.rs:796-800`, `:809`, `:844`, `:895`, `:1015`, `:1108` |
| A9 | `body_impacts`: a frozen body at 600 px/s or more against a still or heavy body shatters; a hit is 0.002 × mass × (v − 400), at most 120, credited to the player or a thrower; only iron pushes and lands hard | `elements/systems.rs:1056-1219` |
| A10 | The gun's bodies are balls, iron and beetles from `Option` columns; kit projectile walls are tiles and static bodies only, so a bullet meets no dynamic body but these | `gun/systems.rs:179-280`; `kit/overlap/projectile.rs:762-776` |
| A11 | Lightning's conductors are iron, scraps and live targets tagged `conductive`; a strike takes what has health or conducts within 40 px; a jump shocks only targets with health and stops at a frozen one | `lightning/mod.rs:78-118`, `:176-237`, `:278-330` |
| A12 | The troll's `liftable`: iron, a bowling ball, or a live frozen enemy (not a troll) or dummy, not held or thrown, the nearest within 200 px; a held body is a sensor | `enemies/troll.rs:92-152` |
| A13 | A ball burns 10 s (`BallBurn`); `ignite` lights a small ball never burnt and not frozen; `fire_spread`'s sources are burning balls and its targets the step's contacts and unburnt small balls within 15.5 px, centre to centre | `elements/components.rs:6-19`; `elements/systems.rs:69-170`; `elements/mod.rs:32` |
| A14 | What lights balls: the flamethrower's cone, missile heat (72 px), fire hazards, thermal shock and burning neighbours; lava lights nothing and the map places no fire hazard (`"hazards": []`) | `toys/systems.rs:1053-1062`, `:568-576`; `elements/systems.rs:398-468`; `tools/level.py:203-221`; `tools/level.json:886` |
| A15 | Cold on a burning small ball queues Steam; the hole spends every burning ball within 384 px each step with at most four puffs | `toys/systems.rs:1029-1044`; `elements/systems.rs:1393-1426`; `elements/mod.rs:66`, `:79` |
| A16 | Pooled fire emitters: at most 128 (64 plume and ember pairs) over the exposed burning balls, centred 6 px up | `elements/systems.rs:305-362`; `elements/mod.rs:68-70` |
| A17 | The kit's `burning` damages only what has `Health`, and its ladder feeds only on landed parts | `kit/damage/pipeline.rs:119-122`, `:657-666`; `kit/damage/ailments.rs:509-512` |
| A18 | `CascadeKind` is four unit variants serialized by snake-case name, so an appended `Detonation` loads every older queue; `execute`'s match is exhaustive † | `elements/cascade.rs:49-67`, `:449-466` |
| A19 | `Entry` (denying unknown fields) already carries `source: Option<u64>` and `strength`; the queue is saved whole and loads with its save IDs † | `elements/cascade.rs:69-90`; `src/save.rs:1162-1165` |
| A20 | `queue` and `queue_jump` set every entry's source to the run's player, and `credited` resolves only the player † | `elements/cascade.rs:150-200` (`:159-163`), `:219-227`, `:443-446` |
| A21 | `run` counts the step, then runs at most `CASCADE_CAP` (48) due entries ordered by due, distance from origin and target ID; an entry queued before `cascade_run` in a step is due in it, one queued during or after it waits a step † | `elements/cascade.rs:288-332`; `elements/mod.rs:53` |
| A22 | Thermal shock's `explode`: reach 160 for iron, 64 a scrap, 48 a ball, else 128; parts of reach/8 fire and physical through `blast_hit` (`.blast()`, `.from(source)`), push 900; it lights small balls, queues frozen bodies and shoves bodies; `end_frozen` despawns a frozen body that is not iron, quietly † | `elements/cascade.rs:259-285`, `:474-577`; `elements/mod.rs:41-45` |
| A23 | `Hit::request` rolls a crit only for the run's player and halves a blast on the player whatever its source; its push is dropped on a kind that stands firm † | `combat/mod.rs:163-199` |
| A24 | The missile's `explode` hurts targets but the player within 72 px (20 falling to 10 fire, credited to the player), bursts four configs (`fireball_blast`, `blast_embers`, `blast_dust`, `blast_smoke`), rings, shatters, pushes (288 px, 2,500), shakes by view distance (0.68) and lights balls; its flare (384 px) comes from its callers, uncapped; `blast_trauma` is private | `toys/systems.rs:459-602`, `:662-680` |
| A25 | `Sound` runs 0–53 (`ALL: [Sound; 54]`), `cue_of` maps by index, and `tests/game/data.rs` wants a rule and a manifest ID per sound; `blasted` has no `min_interval` | `audio/components.rs:45-274`; `audio/mod.rs:54-143`; `tests/game/data.rs:127-149`; `tools/content/data/cue_rules/sounds.json:2-16` |
| A26 | `Music` keeps the wisps' hum and the holes' voices, each following the nearest source (600 px, 900 px), ducked; there is no fire loop | `audio/components.rs:6-25`; `audio/mod.rs:33-41`, `:148-201`; `audio/systems.rs:25-149` |
| A27 | The mine's decorative `ex10_crate_0/1` and `ex10_barrel_0/1` (dark bark, a diagonal brace, silver hoops) stand on nine mine platforms; no prefab, sprite or icon is named `crate`, `keg` or `goblin` | `tools/scenery.py:1590-1620`; `tools/level.json:4899-6229` |
| A28 | Toys sit in physics layer 0 with every mask; lava's surface is layer 4 with mask `[]`, so a toy falls into a basin and sits on its bed | `enemies/mod.rs:68-75`; `level/mod.rs:42-46`; `tools/level.py:213-215` |
| A29 | Islands sleep under 20 px/s for 0.5 s and send no collision events; the global particle cap is 2,048, the burst cap 16, the light cap 16 (in-view lights tie at 0 and keep their order) | `crates/tungsten-core/src/physics/mod.rs:108-109`; `level/mod.rs:83`; `shared.rs:75`; `crates/tungsten-render/src/lighting.rs:109-147` |

### 1.2 Enemies and the troll

| # | Finding | Evidence |
| --- | --- | --- |
| A30 | `EnemyKind` has five variants (snake case); `ALL: [EnemyKind; 5]` is read only by the data test; `KindInfo` has 13 fields filled by one exhaustive `info()`; exhaustive kind arms in `enemies/systems.rs` (patrol, chase, wind-up, attack, search, home, clip); no `EnemyState` match is exhaustive | `enemies/components.rs:8-40`; `enemies/mod.rs:95-200`; `enemies/systems.rs:171-180`, `:217-266`, `:295-340`, `:616-631` |
| A31 | The kit's `Hitbox` has `with_offset` only and a crate-private `set_damage`; a hitbox with damage builds its own request, skipping `stands_firm`, the beetle's rear and the difficulty's factor; the troll's prefab has none | `kit/overlap/hit.rs:774-777`, `:920-926`; `kit/overlap/grid.rs:471-488` |
| A32 | The troll's cycle: tell 1.2 s, attack 0.4 s, the shared recovery 0.5 s, 130 steps slam to slam; T5's 0.8 and 0.35 give 97; `troll_windup` is 200 + 300 + 700 ms; recovery sums have no slack (72 steps sum to 1.1999996) | `troll.rs:51`, `:70`; `enemies/mod.rs:63`; `enemies/systems.rs:268-284`, `:276`; `tools/actors.py:1688-1696` |
| A33 | The troll's chase: holding → the throw's tell; grounded, within 140 across and 120 up → the slam's tell; something liftable within 200 → lift; else walk. The slam hits every saved body's centre within 160 of its fist (56, 50, mirrored) but its own: a live target takes physical with push 420, a frozen one a Shatter, a ball `away·650 + (0, −390)` whatever its mass | `troll.rs:265-317`, `:366-424` |
| A34 | The one slam test kicks a bowling ball (mass 8) 60 px behind the troll and asserts `x > 100` and `y < 0`; 650/8 is 81 | `tests/game/enemies.rs:1354-1403` |
| A35 | `ballistic(from, to, time, gravity)` serves the troll's throw alone, its flight 0.45–1.2 s under world gravity 3,600; `Throws` keeps a thrown body's thrower by save ID with its damage factor | `troll.rs:202-260`; `level/mod.rs:49`; `enemies/components.rs:135-156`; `src/save.rs:483-491` |
| A36 | `LinearMotion` has a saved acceleration, stepped `(v + a·dt/2)·dt`; a bodiless `Projectile` is carried by its movers, ends at tiles and static colliders by layer, sweeps hurtboxes by tag filter and sends `Struck`; the ability payload's spawn is a straight line, so a lob is spawned by 01, as `toys::fire_missile` is | `kit/motion/linear.rs:75-111`, `:255-296`; `kit/overlap/projectile.rs:38-59`, `:835-899`; `kit/intent/ability.rs:1426-1433`; `toys/mod.rs:481-495` |
| A37 | The bullet's way keeps 01's rules: hitbox damage 0, then `gun_hits` lands 01's `Hit` on `Struck`, after `PROJECTILE_TICK`; a command-buffer spawn lands at the flush and needs an adopt pass before a save | `tools/content/prefabs/bullet.json`; `gun/mod.rs:84-88`, `:150-189`; `gun/systems.rs:415-560` |
| A38 | Facing is `Enemy::heading` (±1); the kit's `Facing` serves the player alone | `enemies/components.rs:46-62`; `troll.rs:265-271` |
| A39 | `Brain` (saved, denying unknown fields) holds `last_seen`, `search` and `alert`; `Shown`, which picks the troll's throw or slam clip, is presentation and never saved † | `enemies/components.rs:72-79`, `:181-188`; `troll.rs:446-449` |
| A40 | The alert: `Spotted` fires when sight turns on; every awake, non-chasing enemy within 320 px gets `go(Alert)`; the sting is the `spotted` cue on every `Spotted` (0.5 s apart); a chase that misses one check goes to Search and a re-spot goes Search → Alert; a bat re-spotting from patrol skips Alert | `enemies/systems.rs:432-502`, `:182-215`, `:286-340`; `enemies/mod.rs:223-226`; `tools/content/data/cue_rules/sounds.json:508-518`; `bat.rs:89-93` |
| A41 | The troll's bar is an `Extracts` sprite function over `&World`, flat quads of `ex10_mote`'s texel 6 px over the frame; the beetle's glint reads `machine.state()` and `elapsed()`, which are saved | `enemies/mod.rs:220-221`; `enemies/draw.rs:16-147` |
| A42 | `KitRng` is one saved stream for crits and loot rolls; tests seed it and count its draws | `combat/mod.rs:202-207`; `tests/game/combat.rs:89-103`; `tests/game/enemies.rs:1855-1856` |
| A43 | Per area: aqueduct three crawlers and two bats, ravine five bats and a crawler, hollow three wisps, mine four beetles, bridge the troll, the clearing none; no test pins an area's count, and the area walks and the playthrough despawn every enemy first (`calm`) | `tools/level.json` `enemies`; `tests/game/areas.rs:63`, `:89`; `tests/game/playthrough.rs:308`; `tests/game/main.rs:197-208` |

### 1.3 Summon, placement and saves

| # | Finding | Evidence |
| --- | --- | --- |
| A44 | `SummonType { name, prefab, icon, enemy: Option<EnemyKind> }`; `SUMMONS: [SummonType; 6]` (the dummy, then the five kinds); `None` routes to the dummy's tile-only fit and spawn; the selection is saved as an index modulo the table's length; one `summon` ability (20 mana, 1 s) serves every type † | `spells/mod.rs:50-94`; `spells/systems.rs:216-287`; `src/save.rs:102-103`, `:1278-1280`; `tools/content/data/abilities/summon.json` |
| A45 | Since `D-208` a map object takes a save ID only when a save names it, in layer order: basins 0–2, decks 3–7, the dummy 8, enemies 9–27, orbs 28–35, the player 36, then the fresh run's iron (37–40) and every later spawn; a goblin in the `enemies` layer moves every ID from 28 on † | `level/mod.rs:165-183`, `:219-230`; `src/save.rs:113-144`; `run/mod.rs:88-99` |
| A46 | A fresh run places the mine's iron at its `iron` markers before `Run` exists, so its sounds stay silent; a load skips the markers and restores `blocks` † | `run/mod.rs:88-101`; `audio/mod.rs:136-139` |
| A47 | Schema 11: `migrate` has arms 7–10, each ending in `to_11`; `to_11` replaces `black_hole` by `black_holes`; `map_ids` walks `/black_hole/id` but not `/black_holes` † | `src/save.rs:146-204`, `:306-323`, `:327-386` (`:330`) |
| A48 | `SaveGame` and its parts deny unknown fields; `basins` and `elevators` show the `#[serde(default)]` pattern for a list older saves lack † | `src/save.rs:61-110` |
| A49 | The coverage test maps every saved entity by `kind()` (a precedence chain ending in an exhaustive `EnemyKind` match), needs each component type in one list and the kinds seen to equal a fixed list of 17; an unknown kind panics; `busy()` makes the toys and the map supplies the enemy kinds | `tests/game/saves.rs:132-180`, `:220-671` |
| A50 | Fixtures are whole slot envelopes from the release before (`schema8.json`, `schema9.json`, `schema10.json`); there is no `schema11.json`; `migrate()` writes one to slot 1 of its own user folder and loads it through `SaveSlots::load`; the schema 7 test relabels a current capture as schema 7 | `tests/game/fixtures/`; `tests/game/saves.rs:1685-1715`, `:1825-1842`, `:2406-2415` |
| A51 | `MAP_ENEMIES = 19` is pinned by tests and by the migrations' expectations (`added.count() == 6`, "the new beetle", the frozen layout) | `src/save.rs:116`; `tests/game/saves.rs:2031-2032`, `:2129-2148`, `:2198-2206`; `tests/game/dummy.rs:43-48`; `tests/game/enemies.rs:2006-2023` |
| A52 | `level.py` writes `spawn`, `iron` and `lift_sheave` as class-less markers, then the dummies, the enemies (before `pickups` so earlier objects keep their IDs) and the pickups; `placement.py` stands a pickup on its support and refuses it off it | `tools/level.py:278-308`; `tools/placement.py:58-71` |
| A53 | The walking line is `routes` (`main` has 41 platforms); the route test tolerates partial blocking, the playthrough walks `main` with only enemies removed and does not; no route uses `channel_west` (cols 90–109, row 28), `mine_upper` (318–332, row 37) or `shaft_bed`; the map's east end is `east_bank` (424–428); the spawn is col 6.5 on `clearing` (0–16) | `tools/level.json` `platforms`, `routes`, `spawn`; `tests/game/level.rs:219-276`; `tests/game/playthrough.rs:288-359` |
| A54 | The tests' sky `TEST_SKY = (38, 20)` keeps columns 0–37, rows 0–19 empty | `tools/level.py:56-57`, `:637-643`; `tests/game/main.rs:351-359` |
| A55 | Tests: 268 `#[test]`s in `tests/game/`, one ignored (the frame capture); the integration target is `game`; `cargo test -p example-01-platformer` runs from the example's folder | `tests/game/main.rs:15-40`; `tests/game/frame_time.rs:81` |
| A56 | The frame capture `frame_time::a_full_pit_with_enemies_awake` (ignored): a New Game at Normal, 12,000 balls in the pit, six enemies awake near `bridge_b`, the median of five 300-frame windows; C1's reading is owed again on the 24 × 32 pit | `tests/game/frame_time.rs:1-100`; `docs/plans/1.0/w10-closing-phases.md:35` |
| A57 | Records that name today's counts: `tools/README.md:259` ("The 18 enemies"; 19 since `D-208`) and `:246` (every object a save ID, before `D-208`); `DESIGN.md:5`'s "a bestiary of five enemies"; 01's `README.md` controls and mechanics rows | `tools/README.md:244-262`; `DESIGN.md:5`; `README.md:32`, `:67`, `:83`, `:99`, `:127`, `:145`, `:149` |
| A58 | K3 and K10 have no backlog row; K11's evidence is the backlog's `Lifetime` rows | `docs/plans/1.0/example-01-feedback.md:288`, `:295`, `:296`; `docs/plans/1.0/backlog-1.x.md:62`, `:88`, `:90` |

### 1.4 Art and sound

| # | Finding | Evidence |
| --- | --- | --- |
| A59 | A clip is `ENEMY_CLIPS`' frames and durations → `ex10_<clip>_<i>`, given a `_left` twin by `mirror_facing`; walkers keep their feet on a fixed row (`WALKER_FEET`); a kind's icon (`ICONS`, `ICON_ART`, `icon_of`) and fitted cube (`HURTBOXES`, held to the prefab by `check_ice_fits` once the prefab exists) are tables that raise on a mismatch, and missing ice art draws nothing at run time | `tools/actors.py:1734-1765`; `tools/art.py:15-44`; `tools/effects.py:404-470`, `:515-540`; `tools/generate.py:410-456`; `combat/draw.rs:51-75` |
| A60 | The generator owns every byte of `assets/` (`outputs.json`) and needs Pillow (`tools/requirements.txt`), absent from the system Python; `just level-check` compares and runs its unit tests | `tools/generate.py:464-596`; docs/validation.md matrix row "Platformer generator" |
| A61 | A sound needs an `sfx.py` builder (`LOOPS` for a loop), a `sounds.json` ID, a cue rule and a `Sound` variant appended to `ALL`, `event()` and `cue_of`; a `splinter` sound already exists (a shot passing wood) | `tools/sfx.py:1353-1972`; `audio/components.rs:46-273`; `audio/mod.rs:75-130` |

## 2. Design

### 2.1 Kit first

| 01 system | Kit module or reason it stays 01 code |
| --- | --- |
| The goblin's mind | The kit's `StateMachine<EnemyState>` with 01's hooks and holds, as every enemy (`D-201`) |
| The rock | A bodiless kit `Projectile` with a damage-0 `Hitbox`, carried by `LinearMotion`'s acceleration, its `Source` the goblin; 01 spawns it (the payload's spawn is a straight line, A36) and lands 01's `Hit` on `Struck`, as the gun does (A37) |
| The 1-in-5 bomb | `KitRng`, one draw at the tell (A42) |
| The bomb's and the keg's fuse | 01's own saved field: kit timers have no expiry event and no saved form (K11) |
| The detonation | 01's saved cascade queue, a new kind: the kit has no radial blast payload (K10) |
| Fire on crates and kegs | 01's body state beside the balls' (`D-200`): the kit's `burning` needs `Health` (A17) |
| The troll's swipe | 01's box hit, built as the slam is: a kit `Hitbox` cannot follow its owner's facing and one with damage skips 01's hit rules (K3, A31) |
| Marks over an enemy | 01's `Extracts` sprites, as the troll's bar (A41): presentation, read from the saved machine |
| Loops (the fire's crackle) | 01's voice through the engine's audio commands, as the hum (A26); kit cues play one-shots only |

### 2.2 Art and sound (the review sheet)

`perf-runs/20261023-e01g-art-review/README.md` lists every proposal; in short:

- **Palette:** a pine ramp `Л` (236, 204, 146), `П` (208, 164, 104), `У` (164, 118, 70).
- **Props (lit, 64 px):** `pine_crate_0` (stiles, rails and slats with straw in the gaps, stencilled arrows) and `pine_crate_1` (planks under battens, a burnt-in brand, a split end, a chipped corner), apart from the mine's dark crates; `powder_keg` in red-painted staves with riveted iron hoops, a skull and crossbones and a fuse from its bung, and `powder_keg_lit_0`–`_3` (the cord shorter, the tip sparking); unlit overlays `pine_crate_char_0`–`_2` (soot climbing, ember cracks, holes at the last); fitted cubes `status_ice_crate` and `status_ice_keg` over their boxes.
- **The goblin (unlit, 64 px, feet on row 60):** a sapper on a joint skeleton (skull, hooked nose, slit-pupilled yellow eye, fangs, notched leaf ears with a ring, a goggled leather cap, a laced jerkin over a pot belly, a pouch of rocks and a bomb at the belt, clawed feet); clips `goblin_idle` (240 200 160 120 ms, looping), `goblin_walk` (6 × 85), `goblin_windup` and `goblin_windup_bomb` (110 140 200), `goblin_throw` (60 70 170); `icon_goblin`; `HURTBOXES['goblin'] = (25, 21, 43, 59)` and its cube and cracked twin; `goblin_rock_0`, `goblin_rock_1`, `goblin_bomb_0`, `goblin_bomb_1` (16 px).
- **The troll:** `troll_swipe_windup` (120 180 ms), `troll_swipe` (60 70 200, streaks along the hand's arc), `troll_windup` retimed to 133 200 467 (0.8 s).
- **Marks (16 px, unlit):** `mark_alert` (!) and `mark_search` (?). **Icons:** `icon_crate`, `icon_keg` on the summon ring.
- **Particles:** sprites `char_splinter`, `keg_stave`, `rock_chip`; configs `fuse_sparks`, `fuse_smoke`, `crate_ignite`, `crate_collapse`, `collapse_embers`, `keg_staves`, `rock_chips`, as `sheet.py`'s `proposed_configs`.
- **Sounds (16-bit mono, 44,100 Hz):** `crate_ignite` 0.6 s, `fire_crackle` 2.00 s loop, `crate_collapse` 0.8 s, `fuse_hiss` 1.5 s, `keg_boom` 1.1 s, `rock_throw` 0.25 s, `rock_hit` 0.3 s, `goblin_cackle` 0.6 s, `troll_swipe` 0.35 s, `troll_grunt` 0.5 s. `Sound` gains `CrateIgnited`, `CrateCollapsed`, `FuseLit`, `KegBlasted`, `RockThrown`, `RockHit`, `GoblinCackled`, `TrollSwiped`, `TrollGrunted` (54–62) and `Alerted` (63, on the alert sting's file, §2.8), appended; the crackle is a voice, not a `Sound`.

The prop art is a new generator module, `tools/props.py` (`build_props`, called from `art.build_art`), as `scenery.py` is the level's: the sheet's `crate_slatted`, `crate_planked`, `keg_canvas`, `crate_char` and particle sprites move there, the goblin's rig and clips into `actors.py`, the icons and cubes into `effects.py`.

### 2.3 Crates and kegs

A new feature folder, `src/features/props/` (`crate` is a keyword, so the module is `props`): `PropsPlugin`, `Crate { variant }` (by save ID parity), `Keg`, and from step 4 `Bomb`; prefabs `crate.json` (dynamic box, half 28 at (32, 32), mass 12, restitution 0.1) and `keg.json` (half (19, 25) at (32, 35), mass 16), each naming its marker as `iron_block.json` does, so each prefab lands in the step that registers its component. `props::fits(world, kind, at)` is `iron_fits` with the prop's half, refusing another prop too; `props::place` caps crates at 16 and kegs at 8, the map's counted, the oldest going in a key `ex10_poof` (A3's cosmetic one is dropped at the burst cap); `props::restore` (step 6, with its caller) spawns quietly. An item lands in the step of its first caller, since `-D warnings` refuses dead code between steps.

Crates and kegs join each place iron's code names iron, kept to `Option` columns (P6's style):

| Seam | Crates and kegs |
| --- | --- |
| `toys_bounds` (A4) | Culled past the world's edge |
| Friction (A5) | Wood 900 (iron 1,500), frozen 120 |
| The hole (A6) | Pull response 0.5 (crate), 0.4 (keg); never swept |
| `radial_push` (A7) | By inverse mass: the missile's push throws a crate at 208 px/s at its centre |
| `is_body` (A8) | Both: the ice spray chills and freezes them, fire on a frozen one is thermal shock, a fast one hits targets, a frozen one shatters on a hard hit |
| `blast_reach` (A22) | A frozen crate or keg explodes at 96 |
| Cascade bins (A22) | `What::Crate` and `What::Keg`: an explosion or detonation lights a crate and sets a keg off; steam puts a crate out and snuffs a keg; both are shoved |
| The gun (A10) | A bullet passes a crate as it passes the mine's wood, kicking it with `BALL_KICK` over its mass and splintering (`Sound::Splintered`); a bullet ends at a keg and sets it off |
| Lightning (A11) | A keg's hoops conduct: a strike or a jump that reaches a keg sets it off, and a frozen keg stops the chain |
| `liftable` (A12) | Crates, burning ones too; never kegs (feedback Q18) |
| The slam (step 5) | Kicked by mass |

### 2.4 Fire on crates

`Fuel { left, lit, lighter, doused }`: 8 s of fire, whether it burns, the save ID it credits, and the seconds a douse keeps it from lighting (`DOUSE_SECS`, 4).

- **What lights a crate** (none while frozen or doused): the flamethrower's cone (the spray's traced bodies), missile heat within 72 px, a fire hazard's touch (tests; the map has none), an uncrusted lava basin its box overlaps (Q6), a burning ball or crate within 2 px of its box, an explosion or a detonation in reach.
- **Burning:** `left` falls by the step; a crate burning 2 s or more lights crates and kegs within 4 px by surface distance (`shared::surface_distance`), and at once the small balls within 2 px of its box (`elements::systems::ignite`). Proximity runs per crate through `physics::overlap` of its box grown by the margin, never over the balls, since resting bodies send no contacts (A29) and the pit holds 12,000. A burning crate's impact on a live target adds 12 fire (a thrown one is a firebomb, Q20). At 0 it falls in: despawned with `burn_out`, `collapse_embers` and `crate_collapse` as key bursts and `CrateCollapsed`.
- **Out:** cold on a burning crate queues Steam there and douses it; the hole douses every burning crate in its reach each step (so a crate in a hole's reach stays out, A15's ping-pong closed by `doused`); the fuel left is kept.
- **Look:** the crate tinted from burning's flicker (M80's two ember tints) toward `(112, 86, 72)` by `(1 − left/8)^1.2`; the char overlay's stage by thirds of the burn; three flames, four past a third, five past two thirds; a warm glow; the burning crates join the pooled emitters (A16) first; one `fire_crackle` voice follows the nearest burning crate, silent past 600 px and ducked as the hum.

### 2.5 Fuses and the sourced detonation

- **A keg** carries `fuse: Option<f32>` and `lighter: Option<u64>`. Whatever lights a crate lights its fuse (1.5 s, `FuseLit`, `fuse_sparks` and `fuse_smoke` at the tip, a red pulse over the last half); a hard impact (closing at 600 px/s or more in `body_impacts`), a bullet, lightning or a blast sets it off at once; cold or a hole's douse snuffs a lit fuse (steam, the douse's 4 s); a frozen keg never lights, and fire on it is thermal shock, which shatters it.
- **`CascadeKind::Detonation`**, appended (A18). `cascade::queue_from(world, kind, target, at, origin, strength, source)` takes the credit; `queue` keeps the player's. `run` adds the due entries' sources to the bin's wanted IDs, so `credited` resolves the run's player or any live binned target by save ID, and a gone lighter credits none (A20).
- **Credit through a chain:** `explode` queues each frozen neighbour's shock through `thermal_shock` (`elements/cascade.rs:566`), which credits the player (`:275`), so a goblin's bomb that shocks a frozen body would turn into a player-credited explosion with a crit roll (`combat/mod.rs:163-170`). `thermal_shock_from(world, thing, at, origin, source)` takes the source, and `explode`, `steam` and the detonation pass their entry's own source ID to every ending they queue; `thermal_shock`'s three other callers (`toys/systems.rs:1057`, `elements/systems.rs:454`, `:910`) keep the player's. `combat::systems::reactions` queues an explosion for fire on a frozen target and steam for cold on a burning one, from `DamagedPart` and `ImmunePart` alike, through the player's `queue` (`combat/systems.rs:95-121`, `:164-169`); both events carry the hit's `source` (`kit/damage/health.rs:446`, `:549`), so it queues through `queue_from` with that source's save ID: none when the source has gone, the player's when the part has no source, as today.
- **Executing one:** the keg (or bomb) despawns; a frozen one shatters instead. Then `explode`'s body at its reach (keg 192, bomb 128): reach/8 fire and physical to the targets, `.blast()`, push 900, credited; small balls lit; crates lit and kegs set off, each with this lighter; frozen bodies queued; bodies shoved. Its look: the missile's four bursts plus `keg_staves`, key bursts for the step's first detonation and cosmetic after; the missile's flare for the step's first two; a shake by the courier's distance through `blast_trauma` (made `pub(crate)`); `KegBlasted`, whose rule takes `min_interval` 0.06.
- **Chains** roll in waves of at most 48 a step (A21), deduplicated per kind and target, and save with the queue. Deduplication alone cannot stop a second blast: `run` drops every due entry's key before it executes any (`elements/cascade.rs:321-325`) and `execute` falls back to a gone target's stored point (`:449-456`), so two kegs blowing in one run could queue each other again. A detonation therefore runs only while its target is a live keg or bomb not yet spent in this run, and is skipped otherwise.
- **Credit** (feedback Q18): the player for every spell, the gun and a hard impact of the player's; a goblin for its bomb; a fire's lighter passes to what it lights; a fire nobody lit (a burning ball, lava) credits the player, as today.

### 2.6 The goblin

`EnemyKind::Goblin`, appended (`ALL` to 6, `name()` "goblin"), `enemies/goblin.rs` on the beetle's pattern, prefab `goblin.json`: a kinematic box half (9, 20) at (34, 41), the beetle's controller at 110 px/s, 40 health, `team.enemy`, `flesh`, `freezable`, knockback resist 0, the hurtbox of §2.2, the `killed` cue rolling `enemy_drops`. `KindInfo`: frame 64, eye (41, 24), mass 50, sight 520, alert 0.4, pull 0.12, resist 0, `ex10_icon_goblin`, its cube, not a flyer; and a new field, `recover`, in place of the shared `RECOVER_SECS` (0.5 for the five, 0.9 for the goblin; the troll's moves in step 5).

- **Patrol** at 0.6 of its pace within its leash; **Alert** 0.4 s (`GoblinCackled`); **Chase**: closer than 140 px across it backs away while the ground behind holds (the walk's ledge rule), farther than 420 it closes, between them, seen and within 200 px of rise, it winds up.
- **WindUp** 0.45 s: at its entry one `KitRng` draw picks a bomb under 0.2, saved as the brain's strike (below); the clip is `goblin_windup` or `goblin_windup_bomb`.
- **Attack** 0.25 s: at its entry the throw, from its hand (50, 30), mirrored, toward the courier's centre; `RockThrown`. A rock flies on `ballistic` under its own 1,800 px/s², its `LinearMotion`'s acceleration, with a flight of `distance / 650` held to 0.35–0.9 s. A bomb is a dynamic body the solver pulls at the world's 3,600 (`level/mod.rs:49`; `crates/tungsten-core/src/physics/step/mod.rs:916`), so it flies on `ballistic` under that, with the troll's `flight()` (0.45–1.2 s, A35): the higher lob the sheet draws. Then **Recover** 0.9 s, the goblin's own.
- **The rock** (`goblin_rock.json`): bodiless, `Projectile` range 900, no bounce (it ends at a wall with `rock_chips` and `RockHit`), a circle-5 hitbox of damage 0 filtered to the courier and the dummy, `LinearMotion` set at the spawn, spinning by its velocity. A `Rock { thrower, factor }` keeps its goblin's save ID and damage factor from the throw (A35's rule). `rock_hits`, after `PROJECTILE_TICK` and before the resolve, lands 8 physical × factor with a push of 260 along its flight, from the goblin while it lives.
- **The bomb** (`goblin_bomb.json`): a dynamic circle of radius 6, mass 3, a lit fuse of 1.5 s from the throw, lighter the goblin; a `Bomb` prop on §2.5's code at reach 128. Snuffed, it fizzles out at once (`ex10_fizzle`, `Fizzled`), so a bomb keeps no state past its fuse.
- `Brain` gains `#[serde(default, skip_serializing_if = "Option::is_none")] strike: Option<Strike>`: the attack a tell chose, so a load swings the same one and the clip is picked from saved state (A39). Step 4 adds `Strike::{Rock, Bomb}` and step 5 `Slam`, `Swipe` and `Throw`, each with its first use. Every arm of A30 takes the goblin; the hold, the bounds and the hole treat it as any walker.

### 2.7 The troll

- **T5:** `SLAM_TELL` 0.8 s with `troll_windup` retimed; the troll's `KindInfo::recover` 0.35 s. Each tell and recovery compares with a −1e-4 slack (A32).
- **The swipe (C2):** chasing, grounded, within 90 px across and 80 up, the tell is a swipe (`strike` `Swipe`): `troll_swipe_windup` 0.3 s, then at the attack's first step a box half (40, 36) at (52 × heading, −10) from the body's centre (12–92 px ahead, chest to knee): every live saved target but itself takes 18 physical × its factor and a push of `(560 × heading, −200)` (dropped on one that stands firm), a frozen one shatters, dynamic bodies are kicked by mass; `TrollSwiped` at the attack; recovery 0.35 s. From 90 to 140 the slam stays.
- **I7:** at the slam's tell a ground ring grows from 0 to `SLAM_REACH` (160) in a dust colour, drawn as the troll's bar is, centred where the hit is measured: the slam's point, its fist 56 px ahead (`troll.rs:56`, `:289`), at the troll's feet; `TrollGrunted` at the tell's entry; at the slam a shake through `blast_trauma`'s shape, 0.5 within 120 px of the courier falling to 0 at 640.
- **The kick by mass (I17's note):** the slam and the swipe add `min(650, 2,400 × inv_mass)` along `away` and 0.6 of it up to every dynamic body in reach: a small ball 650 as today, a bowling ball 300, a crate 200, a keg 150, iron 6. The slam test asserts the bowling ball's kick to 2 px/s.
- **Crates:** `liftable` takes crates; a held burning crate keeps burning, and one that falls in while held leaves the troll's hands empty.

### 2.8 Marks and the sting (I10)

- "!" (`mark_alert`) over an enemy for 0.8 s from a fresh alert (entering Alert from Patrol or Return), bobbing 2 px. The machine cannot hold it: Alert lasts 0.4–0.8 s and each transition overwrites `previous` and zeroes `elapsed` (`kit/intent/state_machine.rs:389-391`). So `Brain` gains `#[serde(default)] alerted: f32`, the mark's seconds left, set at a fresh alert and counted down by the step; it rides in the save. "?" (`mark_search`) through a Search once it has lasted 0.4 s, read from the machine's state and elapsed, so a chase that misses one sight check (A40) blinks nothing. Both 6 px over the frame, centred on the mirrored body.
- The sting moves from the `spotted` cue to `enter_alert`: a new `Sound::Alerted` on the sting's file, sounded by each enemy entering Alert from Patrol or Return, never from Search (a re-spot mid-fight). The rule's 0.5 s `min_interval` makes the spotter's and its alerted neighbours' one sting. The `spotted` cue rule goes; `Spotted` still passes the alert. A bat that re-spots from its patrol skips Alert and shows no "!" (a known issue).

### 2.9 Placement

- `tools/level.json` gains `crates` and `kegs`, each `{support, x}` in tiles; `level.py` writes class-less `crate` and `keg` markers at the box's centre, stood on the support by `placement.py`'s stand (the pickups' rule: refused off its support, a stair over it taking its place).
- Defaults (Q11), all off every route's landings: nine crates, three side by side at the clearing's west end (cols 0.5–3, behind the spawn at 6.5) and two each on `channel_west`, `mine_upper` and `east_bank`'s east end; five kegs, on the clearing's west end, `channel_west`, `mine_upper` (two) and `east_bank`. None under the elevator's shaft, where a descending deck would press one.
- A fresh run places them at their markers after the iron and before `Run`, silently, their save IDs after the iron's; a load restores `props`.
- Three goblins appended to `enemies` (`MAP_ENEMIES` 22): one on `channel_west` over the aqueduct's ledges, one on `mine_upper` by its kegs, one on `cut_floor_3` (main, among the beetles), each clear of the tests' sky (A54).

### 2.10 Save schema 12

- `SCHEMA` 12. New lists with `#[serde(default)]`: `props` (`PropSave { id, kind, variant, at, velocity, chill, frozen, fuel, fuse, lighter }`, crates, kegs and bombs) and `rocks` (`RockSave { id, thrower, factor, at, motion, travelled }`, on the bullets' form; a spent rock is skipped). `Brain.strike` and `Brain.alerted` ride in `EnemySave`. Props and bombs restore at once, before the `by_id` rebuild, so a held crate and a bomb's throw resolve (`src/save.rs:1228-1273`).
- `to_12`: every save ID at or above schema 11's first orb (`S11_PICKUPS` 28, frozen beside `S11_PLAYER` 36) moves up by `SHIFT_12` 3, through `map_ids`, which now walks `/black_holes` too (A47). Arms 7–10 end in `to_11` then `to_12`; arm 11 runs `to_12`.
- A migrated schema 11 save names no goblin, in its lists or in `gone`, so the map's three stand at their homes after the load (A45), and it has none of the new crates and kegs (Q12).

## 3. Steps

Every code step's done-when includes these, each → exit 0: `cargo fmt --all -- --check`; `cargo clippy -p example-01-platformer --all-targets --locked -- -D warnings`; `cargo test -p example-01-platformer --locked`. A step that touches `tools/` or `assets/` adds `PATH="<scratchpad>/art-venv/bin:$PATH" just level-check` → exit 0. A step's named tests run as `cargo test -p example-01-platformer --locked --test game -- --exact <names>` → exit 0, and `python3 -B <scratchpad>/listed.py <names>` → exit 0, where `listed.py` (step 1 writes it) runs `cargo test -p example-01-platformer --locked --test game -- --list` through `subprocess` and exits 1 naming any listed test missing, since a filter that matches nothing passes. Gate on exit codes: the shell's output filter rewrites cargo's output.

### Step 1: Baselines

- **Files:** `tests/game/fixtures/schema11.json` (new); `tests/game/saves.rs` (copied to the scratchpad first, edited, restored).
- **Change:** before any other edit. (a) Write `<scratchpad>/listed.py`. (b) Add a temporary ignored test to `tests/game/saves.rs` that builds `grounded()` with `busy()`, opens a second black hole (two in the list), leaves at least one cascade entry waiting with the player's source (`cascade::queue` on a frozen ball, run before the step's resolve), captures, and writes the envelope through `SaveSlots::new(Some(<scratch dir>)).save("slot_1", &saved)`; run it with `cargo test -p example-01-platformer --locked --test game -- --ignored --exact saves::capture_schema_11`; copy `slot_1.json` to `tests/game/fixtures/schema11.json`; restore `saves.rs` with plain `cp` and check it with `cmp`. (c) The frame reading: `df -h .` first (§7); `RUSTFLAGS="-C force-frame-pointers=yes" CARGO_TARGET_DIR=target/e01g-frame cargo test --release -p example-01-platformer --locked --test game --no-run`, then from `examples/01_platformer` the binary it names with `--ignored --exact frame_time::a_full_pit_with_enemies_awake --nocapture`, its output saved in `perf-runs/20261023-e01g-frame/before.txt`. Indicative only: not a capture under the profiling workflow, and no perf verdict (§6).
- **Done-when:** `cmp examples/01_platformer/tests/game/saves.rs <scratchpad>/step1/saves.rs` → exit 0; `python3 -B -c "import json,sys; e=json.load(open('examples/01_platformer/tests/game/fixtures/schema11.json')); d=e['data']; sys.exit(not (e['schema']==11 and len(d['black_holes'])==2 and any(x['source'] is not None for x in d['cascades']['entries'])))"` → exit 0; `rg -q "median of the windows' total p50" perf-runs/20261023-e01g-frame/before.txt` → exit 0.
- **Moves:** none.

### Step 2: Art and sound

- **Files:** step 2's list in the header.
- **Change:** `python3 -m venv <scratchpad>/art-venv` and `pip install -r examples/01_platformer/tools/requirements.txt` (a failed install is a stop, §7). Port the review sheet's art into the generator with its own toolkit (`Part`, `shade`, `compose`, `outline`, `two_bone`, `strand`), the owner's notes on the sheet applied if any: §2.2's palette, prop art (`tools/props.py`), goblin rig and clips (`actors.py`, `ENEMY_CLIPS`, `WALKER_FEET`), troll clips, marks, icons (`ICONS`, `ICON_ART`), cubes (`HURTBOXES['goblin']`; a `PROP_BOXES` table for the crate's and the keg's), 16 px sprites (`ELEMENT_SIZES`), particle sprites and configs (`generate.particle_configs`); the ten sounds (`sfx.py`, `fire_crackle` in `LOOPS`), their IDs and cue rules; the ten `Sound` variants of §2.2 appended with their events, rules and `cue_of` entries (`Alerted`'s rule on the sting's existing file, with the `spotted` rule's 0.5 s `min_interval`). Regenerate `assets/` with the generator.
- **Done-when:** the common checks with `just level-check` (the data test holds a rule and an ID to every `Sound`); `python3 -B <scratchpad>/has_ids.py` → exit 0, a script this step writes that loads `assets/manifest.json` and exits 1 naming any of these IDs missing from its section, each prefixed `ex10_`:
  - `sprites`: `pine_crate_0`, `pine_crate_1`, `pine_crate_char_2`, `powder_keg`, `powder_keg_lit_3`, `status_ice_crate`, `status_ice_keg`, `goblin_idle_3`, `goblin_throw_2`, `goblin_windup_bomb_2`, `status_ice_goblin_cracked`, `goblin_rock_1`, `goblin_bomb_1`, `troll_swipe_2`, `troll_swipe_windup_1`, `mark_alert`, `mark_search`, `icon_goblin`, `icon_crate`, `icon_keg`, `char_splinter`, `keg_stave`, `rock_chip`;
  - `animations`: `goblin_idle`, `goblin_walk_left`, `goblin_windup`, `goblin_windup_bomb`, `goblin_throw`, `troll_swipe_windup`, `troll_swipe`;
  - `particles`: `fuse_sparks`, `fuse_smoke`, `crate_ignite`, `crate_collapse`, `collapse_embers`, `keg_staves`, `rock_chips`;
  - `sounds`: `crate_ignite_sfx`, `fire_crackle_sfx`, `crate_collapse_sfx`, `fuse_hiss_sfx`, `keg_boom_sfx`, `rock_throw_sfx`, `rock_hit_sfx`, `goblin_cackle_sfx`, `troll_swipe_sfx`, `troll_grunt_sfx`.
- **Moves:** none; no code reads the new art yet.

### Step 3: Crates and kegs

- **Files:** step 3's list in the header.
- **Change:** §2.3–§2.5, the summon types as an enum (`Summoned::{Dummy, Enemy(EnemyKind), Crate, Keg}` in place of `enemy`, `SUMMONS` appending crate and keg; `press` fits and places props through `props::fits` and `props::place`), crates liftable. The tests that read `.enemy` take the enum (`tests/game/enemies.rs:2059-2061`, `tests/game/dummy.rs:140-184`). New tests in `tests/game/props.rs`:
  - `props::a_crate_set_alight_burns_down_and_falls_in`: lit by the flamethrower, burning 8 s ± a step, lighting a crate 3 px away at 2 s and a small ball resting 1 px off its box at once, then gone with the collapse.
  - `props::cold_and_a_hole_put_a_crate_out_with_its_fuel_kept`: the ice spray douses it with steam; it cannot light for 4 s; it then burns out its fuel left; in a hole's reach it never relights.
  - `props::a_lit_keg_blows_after_its_fuse_and_its_neighbours_follow`: a fuse of 1.5 s ± a step; three kegs 150 px apart go in three waves; a dummy in reach is hit with a fire and a physical part of 24 each before its resistances, its source the lighter; a target past 192 px is not hit.
  - `props::a_bullet_a_hard_landing_and_lightning_each_set_a_keg_off`.
  - `props::a_frozen_keg_shatters_in_fire_and_never_blows`.
  - `props::a_detonations_credit_passes_through_the_endings_it_sets_off`: a keg lit with a dummy's save ID as its lighter, beside a frozen ball (the bin's path), a frozen second dummy (the reactions' `DamagedPart` path) and a frozen wisp (fire-immune, their `ImmunePart` path), whose endings set off a second keg; every hit of the chain has the first dummy as its source and none rolls a crit.
  - `props::two_kegs_blowing_in_one_step_blow_once_each`: two touching kegs lit together blow in the same step, two blasts and none in the next step.
  - `spells::the_summon_slot_places_crates_and_kegs_where_they_fit`: a crate and a keg placed at the cursor; refused in a wall and on iron with the fizzle, spending nothing; the cap's oldest poofs.
- **Done-when:** the common checks with `just level-check`; the eight tests above listed and passing.
- **Moves:** none.

### Step 4: The goblin

- **Files:** step 4's list in the header.
- **Change:** §2.6, the bomb on step 3's code; `enemy(EnemyKind::Goblin)` in `SUMMONS` after the troll (dummy, the six kinds, crate, keg); `tests/game/saves.rs`'s `kind()` takes the goblin's arm so it compiles; `tests/game/data.rs` checks the goblin's body and resist against its prefab through `ALL`. New tests in `tests/game/enemies.rs`:
  - `enemies::a_goblin_lobs_rocks_on_their_arc_and_one_throw_in_five_is_a_bomb`: seeded `KitRng`; a rock within 2 px of its `ballistic` arc under 1,800 for 12 steps (the throws' arc test's shape, `tests/game/enemies.rs:1454-1495`), landing 8 physical on the courier from the goblin; a bomb within 2 px of its arc under the world's 3,600 for 12 steps before it touches anything, and its detonation credited to the goblin; over 25 throws the bombs match the stream's draws under 0.2.
  - `enemies::a_goblin_keeps_its_distance`: the courier at 100 px, it backs off to 140 or more within 2 s; at 600 it closes.
  - `enemies::a_rock_from_a_goblin_killed_in_flight_still_lands_at_its_factor`.
- **Done-when:** the common checks with `just level-check` (`check_ice_fits` now holds the goblin's cube to its prefab); the three tests listed and passing.
- **Moves:** none.

### Step 5: The troll and the marks

- **Files:** step 5's list in the header.
- **Change:** §2.7 and §2.8. The slam test (`tests/game/enemies.rs:1354-1403`) takes the 0.8 s tell through `SLAM_TELL` and asserts the bowling ball's kick to 2 px/s. New tests:
  - `enemies::the_troll_swipes_what_is_close_and_slams_what_is_farther`: the courier 60 px ahead takes the swipe's 18 after 0.3 s ± a step and one 120 px ahead the slam's 40 after 0.8 s ± a step; slam to slam in 97 steps; a crate in the swipe's box kicked by its mass.
  - `presentation::an_alert_shows_its_mark_and_stings_once`: a fresh alert shows "!" for 0.8 s, past Alert into the chase, and a save taken 0.5 s in, loaded through JSON, shows it 0.3 s more; it sounds `Alerted` once for the spotter and a neighbour together; a re-spot from Search sounds none and shows no "!"; a search past its first 0.4 s shows "?".
- **Done-when:** the common checks with `just level-check` (the cue rules changed); the two new tests and the slam test listed and passing.
- **Moves:** none.

### Step 6: Placement and save schema 12

- **Files:** step 6's list in the header.
- **Change:** §2.9 and §2.10, regenerated. Tests:
  - The coverage test's kinds list takes `crate`, `keg`, `bomb`, `rock` and `goblin` (17 → 22) with their `lists()` arms; `busy()` adds a burning crate, a lit keg, a bomb in flight and a rock in flight; the map's goblins supply their kind.
  - `saves::a_keg_chain_saved_mid_chain_finishes_the_same_after_a_load`: six kegs 150 px apart, saved after the second wave, loaded through JSON (`serde_json::from_value`), the queue's entries equal at the capture, the same kegs gone and the queue empty in both runs 30 steps later.
  - `saves::schema_11_saves_move_their_ids_past_the_map_goblins`: `fixtures/schema11.json` migrates; every ID from 28 up moved by 3 (`next_id`, the player, the orbs, the blocks, both holes, the cascade entries' sources and targets), every ID under 28 kept; none of the goblins' IDs 28–30 in `gone`; the loaded run has the three goblins at their homes.
  - The schema 7 test keeps its current capture relabelled (A50): it asserts the staff light, the lantern and the autosaves' rotation, which `to_12` leaves alone, and already loads a capture that `shift_ids` and `to_11` reshape. The schema 8–10 tests, the frozen layout and the `MAP_ENEMIES` pins (A51) take `SHIFT_12` and 22; tests that pin a fresh run's IDs take the props after the iron.
- **Done-when:** the common checks with `just level-check`; the two new tests listed and passing; `python3 -B -c "import json,sys; m=json.load(open('examples/01_platformer/assets/tilemaps/level.tmj')); n=[o['name'] for l in m['layers'] if l['name']=='markers' for o in l['objects']]; e=[o for l in m['layers'] if l['name']=='enemies' for o in l['objects'] if o['type']=='goblin']; sys.exit(not (n.count('crate')==9 and n.count('keg')==5 and len(e)==3))"` → exit 0 (the markers by name and the goblins by class: a layer is named `iron`, so a text count misreads).
- **Moves:** save schema 11 → 12; save IDs from 28 up by 3.

### Step 7: Records and the second frame reading

- **Files:** step 7's list in the header.
- **Change:** 01's `README.md` (controls: E/Q's nine types; the mechanics rows for crates, kegs, the goblin, the swipe; 22 enemies; schema 12); `tools/README.md` (the `crates` and `kegs` fields, their markers, "The 22 enemies", A57's stale line 246); the decision (§5) with its index row and the old entries' marker lines; `DESIGN.md:5`'s clause ("a bestiary of six enemies", crates that burn and kegs that blow), leaving its `Workspace` text for the cut; the design's "As built, 01 (g)" bullets; the feedback plan's §3.3 "Landed" paragraph and §6's K3, K10 and K11 rows; backlog rows for K3 and K10 (A58) with 01's code as their evidence; known issues for what this plan found and left (the bat's missing "!", older saves without the new props, the indicative frame reading, any found in the run). The frame reading again as in step 1, into `perf-runs/20261023-e01g-frame/after.txt`, then `target/e01g-frame` removed with `shutil.rmtree`; the venv removed the same way.
- **Done-when:** `just repo-check` → exit 0; `just ctx` → exit 0; `rg -q 'D-21[0]' docs/DECISION_INDEX.md` → exit 0; `python3 -B -c "import re,sys; r=lambda p: float(re.search(r'total p50: ([0-9.]+) ms', open(p).read()).group(1)); a,b=r('perf-runs/20261023-e01g-frame/after.txt'),r('perf-runs/20261023-e01g-frame/before.txt'); print(b,a); sys.exit(not a<=b+1.0)"` → exit 0, both numbers quoted (a larger rise is a stop, §7); `test ! -e target/e01g-frame` → exit 0.
- **Moves:** none.

### Step 8: Release 0.102

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.102.0` once, every release check once after it, then the preflight with `--message`. No gate beyond them: `just level-check` ran last in step 6 and nothing after it touches `tools/` or `assets/`, and `just check` runs the layer 1 manifests test. `git diff --quiet HEAD -- crates templates api` → exit 0 (no engine, kit, template or API change).
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.102.0 --repo JosephGibson/Tungsten --message 'Update 0.102: M81 example 01 props and foes'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for 01 (h) the look.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The owner's notes on the review sheet | Step 2 applies any written in this plan or the sheet's README before the run; none, the sheet's proposals as drawn |
| Q2 | Masses | Crate 12, keg 16, bomb 3 (the feedback's 40 left every push near still, A7) |
| Q3 | The hole's pull on props | Crate 0.5, keg 0.4, bomb by its inverse mass |
| Q4 | Caps | Crates 16, kegs 8, the map's counted, the oldest in a key poof; bombs uncapped, bounded by the goblins' pace |
| Q5 | Burn | 8 s; lights the small balls within 2 px at once, and crates and kegs within 4 px once it has burned 2 s; a douse holds 4 s |
| Q6 | Lava lights props | Yes, an uncrusted basin a prop's box overlaps; the map has no fire hazard to stand in for it |
| Q7 | Bullets and crates | A bullet passes a crate as it passes wood, kicking and splintering; it ends at a keg and sets it off |
| Q8 | Detonation | Keg reach 192 (24 + 24), bomb 128 (16 + 16), push 900; flares for a step's first two; the boom 0.06 s apart |
| Q9 | Credit | §2.5's list; a gone lighter credits none |
| Q10 | The goblin's numbers | §2.6's: 40 health, 110 px/s, range 140–420, tell 0.45 s, throw 0.25 s, recovery 0.9 s, rock 8 and push 260, its gravity 1,800, flight 0.35–0.9 s, range 900; a bomb 0.2, under the world's gravity on the troll's flight; its fuse 1.5 s from the throw, fizzling out at once when snuffed |
| Q11 | The map | Three goblins and §2.9's crates and kegs, off every route's landings |
| Q12 | Older saves | A schema 11 save keeps its world: the map's goblins stand at their homes after the load, and the new crates and kegs come with a new run (a known issue) |
| Q13 | The swipe | Within 90 px across and 80 up; tell 0.3 s; 18 physical, push 560 out and 200 up; the box 12–92 px ahead, chest to knee; recovery 0.35 s |
| Q14 | The slam | Tell 0.8 s, the troll's recovery 0.35 s; the ring to 160 px about its fist's point; the shake 0.5 within 120 px to 0 at 640 |
| Q15 | The kick | `min(650, 2,400 × inv_mass)` out and 0.6 of it up, every dynamic body in reach |
| Q16 | Marks | "!" 0.8 s at a fresh alert, "?" through a Search past its first 0.4 s; one sting a fresh alert |
| Q17 | What trolls lift | Crates, burning ones too; no kegs (feedback Q18) |
| Q18 | Summon order | Dummy, wisp, crawler, beetle, bat, troll, goblin, crate, keg; the one `summon` ability for all |
| Q19 | The frame bar | Indicative: the after-reading's `total` p50 at most 1.0 ms over the before-reading's |
| Q20 | A burning crate's touch | Its impact on a live target adds 12 fire; otherwise its fire lights only what burns |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

210: example 01 (g) landed (props, the sourced detonation, the goblin, the troll's swipe and timings, marks, save schema 12); amends 194 (the stops after (d)), 200 (the cascade queue's credit and a fourth kind of ending), 201 (a sixth enemy, the troll's tell and recovery, the alert's sting), 202 (the map's goblins and placed props) and 209 (the slam's kick by mass, M80's note), as their marker lines will say. Checked again when step 7 runs.

## 6. Invariants

- **May move:** example 01's save schema (11 → 12) and its save IDs from 28 up (step 6), recorded in the evidence log and the `CHANGELOG.md` line.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions; the kit hash `0x3334_9665_de0c_501a` (`crates/tungsten/src/tests/kit_recipes/determinism.rs:36`); the `api/` snapshots; the shell's byte copy and the template. No step touches `crates/`, `templates/` or `api/`, which step 8's `git diff --quiet HEAD -- crates templates api` holds.
- **Perf:** no row this plan touches is in the benchmark suite; the frame readings of steps 1 and 7 are indicative, with no verdict, and C1's owed reading stays owed (A56).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies (plain `cp`, then `cmp`), mark the step skipped, and continue with steps that don't depend on it. Steps 3–6 each depend on the steps before them, so a skipped step 3 skips 4–6, and the release waits for a release session after the owner has seen why.
- The fixture of step 1 cannot be captured from the unmodified tree: stop before step 2's edits; step 6's migration test needs it.
- The venv's install fails: steps 2–6 cannot regenerate `assets/`; stop.
- A behaviour needs a kit or engine change: record it as a gap row in the feedback plan's §6 and a follow-up, leave that part out, and continue only where the rest stands without it; never a workaround in 01.
- The after-reading's `total` p50 rises more than 1.0 ms: stop before the release, find the per-ball path with a probe, and leave the release to a release session.
- Disk under 15 GB free before a release build: clear `target/debug/incremental` (a disposable cache, workflow §6.5); under 10 GB after it, stop.
- `pgrep -af scripts/bench.py` shows another session's capture: no writes to the tree until it ends (workflow §6.2).

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 2–6, release | `just level-check` and 01's tests in the steps; `just smoke` (01's runtime wiring), layer 1 and `just check` at the release; the frame readings indicative (§6) |
| 3 Invariants | 8 | `git diff --quiet HEAD -- crates templates api`; the kit hash inside `just check` |
| 4 Decisions | 7 | 210 and its index row before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | n/a | No public API: 01 is an example; `just api-check` at the release passes unchanged |
| 6 Template and guide | n/a | No template change; the shell stays a byte copy (`just repo-check`) |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 7 | 01's row names `src/features/` and `tests/game/`, so `props/`, `goblin.rs` and `props.rs` need no row; `just ctx` |
| 8 Design | 7 | No architecture change; `DESIGN.md:5`'s 01 clause |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 7, 8 | K3 and K10 backlog rows; the design's and the feedback plan's records; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | 8 | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo` on `gpt-6-astra`, 2026-10-23, reading the tree at `6773e75` with this plan uncommitted. Model gpt-6-astra, effort xhigh, context 872,000, repo mode (output cap 4,000), 426.9 s, tokens in 985,201 (cached 895,872), out 10,502 (reasoning 5,535), 34 commands, output 261.7 KB (largest 24 KB), usage 5h 0% → 23%, week 0% → 4%; six findings. Each was checked against the code it cites before any change.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] The bomb's launch calculation uses half its actual gravity: a dynamic body falls at the world's 3,600, not the rock's 1,800, landing about 324 px low over a 0.6 s flight | `examples/01_platformer/src/features/level/mod.rs:49`; `crates/tungsten-core/src/physics/step/mod.rs:913-917` (a dynamic proxy takes `config.gravity`) | Accepted | §2.6: the bomb flies on `ballistic` under the world's gravity with the troll's `flight()`, the rock keeps its own 1,800; step 4's test checks each arc on its own gravity; Q10 |
| 2 | [MEDIUM] The machine cannot hold the alert mark's history: Alert ends before 0.8 s and a transition overwrites `previous` and zeroes `elapsed` | `crates/tungsten-kit/src/intent/state_machine.rs:389-391` | Accepted | §2.8: `Brain.alerted`, the mark's seconds left, saved; §2.10; step 5's test carries the mark into the chase and through a load |
| 3 | [MEDIUM] The saved bomb omits its dud countdown | §2.6 and §2.10 as written | Accepted | A snuffed bomb fizzles out at once, so a bomb keeps no state past its fuse (§2.6, Q10) |
| 4 | [MEDIUM] Reusing thermal shock's chain path loses the goblin's credit: `explode` queues frozen neighbours through the player-crediting `thermal_shock`, with a crit roll | `examples/01_platformer/src/features/elements/cascade.rs:275`, `:566`; `combat/mod.rs:163-170` | Accepted | §2.5: `thermal_shock_from`, every ending a sourced entry queues keeping its source; step 3 adds `props::a_detonations_credit_passes_through_the_endings_it_sets_off` |
| 5 | [MEDIUM] Relabelling schema 11 as schema 7 still runs `shift_ids` (+13) and `to_11`, which empties an existing hole list | `examples/01_platformer/src/save.rs:172`, `:227-234`, `:306-313`; `tests/game/saves.rs:1681-1754` | Accepted in part | The relabelling is dropped: the schema 7 test keeps its current capture, which it already sends through `shift_ids` and `to_11`, since it asserts only the staff light, the lantern and the rotation (step 6). Rebuilding schema 7 data in its old layout is left out: the schema 8–10 fixtures and the new schema 11 one carry the ID chain through `to_12` |
| 6 | [MEDIUM] The slam's warning ring sits at the troll's feet while its hit is measured about the fist, 56 px ahead | `examples/01_platformer/src/features/enemies/troll.rs:56`, `:265-289` | Accepted | §2.7 and Q14: the ring is centred on the slam's point |

Round 1 changed done-when checks (step 3's tests, step 4's and step 5's), so round 2 ran on the revised plan, with this section and the evidence log cut and a focus note naming the changed sections and finding 5's rejected part: 2026-10-23, the same tree, gpt-6-astra, effort xhigh, context 872,000, repo mode (output cap 4,000), 216.5 s, tokens in 973,981 (cached 885,248), out 6,015 (reasoning 2,833), 18 commands, output 229.2 KB (largest 25.5 KB), usage 5h 23% → 44%, week 4% → 7%; two findings. There is no third round, and neither is left open.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] Reactions on frozen enemies still reset a chain's credit to the player: `reactions` queues explosions from fire on a frozen target, by `DamagedPart` and `ImmunePart`, through the player-crediting `queue`; step 3's test uses a frozen ball, which bypasses it | `examples/01_platformer/src/features/combat/systems.rs:95-121`, `:164-169`; `crates/tungsten-kit/src/damage/health.rs:446`, `:549` (both events carry `source`) | Accepted | §2.5: `reactions` queues through `queue_from` with the part's source; step 3's credit test adds a frozen dummy and a frozen wisp; `combat/systems.rs` joins step 3's files |
| 2.2 | [MEDIUM] Deduplication does not stop repeated detonations: `run` drops every due key before executing any, and `execute` falls back to a gone target's point, so two kegs blowing together can queue each other again | `examples/01_platformer/src/features/elements/cascade.rs:321-325`, `:449-456` | Accepted | §2.5: a detonation runs only on a live keg or bomb not yet spent in the run; step 3 adds `props::two_kegs_blowing_in_one_step_blow_once_each` |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-23 | Done | The temporary `saves::capture_schema_11` passed (exit 0); the fixture, 167,427 bytes: schema 11, 502 balls, 21 blocks, holes 561 and 565, one explosion waiting on 566 credited to the player (36), `next_id` 567; `cmp` of `saves.rs` → 0; the fixture check → 0; the before-reading's median of the windows' total p50 3.743 ms (windows 3.669–3.988), `rg -q` → 0 | `tests/game/fixtures/schema11.json`; `perf-runs/20261023-e01g-frame/before.txt` |
| 2 | 2026-10-23 | Done | Venv with Pillow 12.3.0; `tools/props.py` new; the generator wrote 4,293 outputs; the sheet's 43 frames equal the generated sprites pixel for pixel, the goblin's one row higher (the sheet drew its outline's lowest row on 61, the walkers' is 60, where its collider ends); the ten sounds at their lengths, `fire_crackle` 2.00 s passing the seam check; fmt → 0; clippy → 0; tests → 0 (267 passed, 1 ignored); `level-check` → 0 (4,293 owned outputs match, 15 generator tests OK); `has_ids.py` → 0 (47 of 47) | `tools/props.py`; `assets/` |
| 3 | 2026-10-23 | Done | `props/` new (plugin, `Crate`, `Fuel`, `Keg`, heat, looks, fuse emitters, draw); `crate.json` and `keg.json`, the generator rerun (4,295 outputs); `CascadeKind::Detonation`, `queue_from`, `thermal_shock_from`, sources binned and credited, every ending passing its entry's source; reactions credit the part's source; crates and kegs in the bounds, friction (wood 900), the hole (pull 0.5 and 0.4, put out in reach), the missile's heat, the sprays, fire hazards, lava, impacts (a keg off at 600 px/s, so one falling more than about 50 px blows on landing; a burning crate's hit +12 fire), the gun (passes crates, ends at kegs), lightning (kegs conduct and go off), the troll's lift and the summon slot (`Summoned::{Dummy, Enemy, Crate, Keg}`, eight types); fmt → 0; clippy → 0; tests → 0 (275 passed, 1 ignored); the eight named → 0; `listed.py` → 0 (8 of 8); `level-check` → 0 | `src/features/props/`; `tests/game/props.rs` |
| 4 | 2026-10-23 | Done | `EnemyKind::Goblin` (six kinds) and `goblin.rs` (keeps off at 140, closes past 420, tell 0.45 s, throw 0.25 s, recovery 0.9 s through the new `KindInfo::recover`); `Brain.strike` with `Strike::{Rock, Bomb}`; `Rock` and `rock_hits` (8 × factor, push 260), the rock's spin; the bomb a lit prop (`Bomb`, `PropKind::Bomb`, reach 128, fizzling when snuffed, the props' friction, restitution 0: at 0.3 a bomb bounced off the courier's body and rolled out of its own reach); `goblin.json`, `goblin_rock.json`, `goblin_bomb.json` (4,298 outputs); nine summon types; the data test holds the props', the rock's and the bomb's restated constants; fmt → 0; clippy → 0; tests → 0 (278 passed, 1 ignored); the three named → 0; `listed.py` → 0 (3 of 3); `level-check` → 0, `check_ice_fits` holding the goblin's cube to its prefab | `src/features/enemies/goblin.rs`; `tests/game/enemies.rs` |
| 5 | 2026-10-23 | Done | The slam's tell 0.8 s and the troll's recovery 0.35 s (every recovery now compares with the step's −1e-4 slack); the swipe under 90 px across and 80 up (tell 0.3 s, box half (40, 36) at (52, −10), 18 × factor, push (560, −200)); the kick by mass for both (`min(650, 2,400 / mass)` out, 0.6 of it up); the slam's grunt, its ring about the fist and its shake (0.5 within 120 px of the courier, none past 640, the shape shared as `toys::systems::falloff`); `Strike::{Slam, Swipe, Throw}` in the brain, the troll's clips read from it; `Brain.alerted` and the marks ("!" 0.8 s from a fresh alert, a machine's first visit counting as fresh; "?" past a search's first 0.4 s); `Sound::Alerted` and the `spotted` rule removed (64 rules); the slam test renamed `the_troll_slams_after_its_wind_up_hurting_kicking_and_shattering`, its ball's kick held to 2 px/s; slam to slam 97 steps; fmt → 0; clippy → 0; tests → 0 (280 passed, 1 ignored); the three named → 0; `listed.py` → 0 (3 of 3); `level-check` → 0 | `src/features/enemies/{troll,draw}.rs`; `tests/game/presentation.rs` |
| 6 | 2026-10-23 | Done | `level.json`'s `crates` (9) and `kegs` (5), `{support, x}` with an optional `tier`, stood by `placement.py` half a pixel over their support (refused off it, a stair taking its place, a tier's corners over the boxes under it) and written as `crate` and `keg` markers; three goblins appended (22 enemies, save IDs 28–30: the aqueduct's leashed 3 tiles over the ledges, the upper one between its two kegs, the cut's leashed 3 tiles); deviation: the map's dummy stands on the clearing's cols 2.28–2.72, so its third crate stands on the first two and its keg east of the dummy at col 3.08; a fresh run sets them down after the iron, quietly (iron 40–43, crates 44–52, kegs 53–57); schema 12: `props` (`PropSave`, its point named `position` as the other bodies' are, and a keg's `doused`) and `rocks` (`RockSave`, the bullets' form), restored before the `by_id` rebuild, `to_12` (`SHIFT_12` 3 from `S11_PICKUPS` 28), `map_ids` walking `/black_holes`, `/props` and `/rocks`; deviation: a ball's record leaves its trailing `null`s off (a load reads 6–10 values), since the goblins and props took the full pit to 2,111,844 bytes, over the 2 MiB bound, and it now saves in 1,775,844; the coverage test's 22 kinds, `busy()` adding a crate alight, a lit keg, a bomb and a rock in flight (east of its hole's 384 px douse); the schema 8–10 tests on `SHIFT_12`, 22 enemies and the trimmed balls; the summon test takes the map's props off first (`tests/game/spells.rs`, step 3's); generator rerun (4,298 outputs); fmt → 0; clippy → 0; tests → 0 (282 passed, 1 ignored); the two named → 0; `listed.py` → 0 (2 of 2); the tilemap check → 0; `level-check` → 0, last (15 generator tests OK); five in-game frames of the placements read clean (scratchpad, the capture crate deleted) | `tools/level.json`; `tools/{level,placement}.py`; `src/save.rs`; `tests/game/saves.rs` |
| 7 | 2026-10-23 | Done | 01's `README.md` (nine summon types, the crates' and kegs' rules, the goblin and the troll's new moves, 22 enemies, schema 12, the `props` tests); `tools/README.md` (the `crates` and `kegs` fields and their markers, "The 22 enemies", A57's save-ID line on `D-208`'s rule, `props.py`, the goblin's and the troll's clips, the ten sounds); `D-210` with its index row and marker lines on `D-194`, `D-200`, `D-201`, `D-202` and `D-209`; `DESIGN.md:5`'s clause; the design's "As built, 01 (g)" bullets with its silhouette and clip rows; the feedback plan's §3.3 "Landed" paragraph and its K3, K10 and K11 rows; backlog rows for K3 and K10 and a keg chain in K11's (`Lifetime`, the chain queue); known issues for the bat's "!", older saves without the new props, short keg falls and the indicative readings, and a follow-up for `sfx.py`'s cackle docstring; the after reading, the median of the windows' total p50 3.709 ms (windows 3.690–3.818) against 3.743 before, the check printing `3.743 3.709` → 0, indicative (one run, a remote-desktop encoder at ~19 % on the machine, otherwise 94 % idle); `target/e01g-frame` and the venv removed with `shutil.rmtree`; `just repo-check` → 0; `just ctx` → 0; `rg -q 'D-21[0]' docs/DECISION_INDEX.md` → 0; `test ! -e target/e01g-frame` → 0 | `DECISIONS.md`; `docs/known-issues.md`; `perf-runs/20261023-e01g-frame/after.txt` |
| 8 | 2026-10-23 | Done | Workflow §5 closed: `git diff --quiet HEAD -- crates templates api` → 0 (no engine, kit, template or API change), the kit hash inside `just check`; `DESIGN.md:5`'s clause (step 7); the register row (`Cut 2026-10-23 as v0.102.0`, `D-210`), `CHANGELOG.md`'s `[Unreleased]` notes and this row, `status: done`; the plan archived before the cut, so `just release-cut 0.102.0`, the release checks and the preflight run after it and their output is in the session's report | `CHANGELOG.md`; `docs/plans/1.0/implementation-plan.md` |

## Follow-ups

- C1's frame reading on the 24 × 32 pit stays owed (A56); this plan's readings are indicative and do not discharge it ([known issues](../../../known-issues.md)).
- A bat that re-spots from its patrol shows no "!" (A40): a known issue for the Phase 6 QA pass.
- Older saves without the newly placed props (Q12): a known issue.
- A keg dropped more than about 50 px blows on landing, by C4's 600 px/s rule: a known issue for the owner's play.
- `tools/sfx.py`'s `goblin_cackle` docstring names the throw's tell where the game cackles at the alert: a known-issues follow-up, a docstring only.
