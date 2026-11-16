# 01's and the template's restated data read off the loaded data, and 01's tumble as a tween (example-01-restated-data) — draft

- **status:** draft
- **goal:** Phase 7's 7.13d's restated data (§2.8), from 7.13d's toys and data's §2.1 S-13 row and its evidence log's Step 9 row as they stood at `3976ba81`. Example 01 reads each value it restates off the data it loads: its props' boxes, centres, fuels and the bomb's fuse point off their prefabs and the manifest; its balls', iron's and scraps' bodies and masses off their prefabs or their own colliders; the bullet's centre, radius and ricochets, the muzzle and the magazine off the bullet's prefab and the gun's ability; the orbs' life and box off their prefabs; the missile's speed and size off its ability, prefab and sprite; and its tests read the schools' and the crit's label colours off the damage types and the number cues (S-13). The template's tests read the player's speed and acceleration and the hazard's swing and period off their prefabs (S-13). And 01's scraps' and planks' tumble is a rotation tween, the twin-save test naming the tween a load drops (S-7's tumble). 01's tests' outcomes unchanged but §2.2's named moves.
- **non-goals:**
  - Constants outside S-13's register sites that also restate data, held by `tests/game/data.rs`'s lines outside the row (the courier's, troll's and crawler's numbers, the walkers' step and shove, the freeze and chill builds' bands, the foes' impact masses, the snowball's shape), elements' `FIRE_HALF` and `FIRE_CENTRE`, and the spikes' and fires' touch boxes in `level.rs` and `playthrough.rs`, which restate hazard prefabs no map places: one Later line (§2.11).
  - 01's own tuning that restates no loaded data: `BALL_SCALE`, `SMALL_BALL_SCALE`, `SCRAP_SCALE`, `RELOAD_SECS`, `MISSILE_RADIUS`, `SCORCH_ANCHOR`, `SPRAY_MUZZLE` (§2.11).
  - The props' boxes as 01's bodies data (S-13's sketch): the `bodies` family is locked (`D-223`), so its files are the owner's to unlock (Q2).
  - Engine changes: none in core, the kit or the umbrella; no public kit accessor for a damage type's label colour or a spawn payload (§2.3); no new art, no locked family's file (`D-223`); no schema change; no retuning; a sitting as evidence (`D-226`).
- **files to touch:** by step, §3's Files lines; nothing outside them but §6's admitted files. In short: Step 1 01's toys, props and elements with their tumble's tests and the save census; Steps 2–7 `examples/01_platformer/` (`src/`, `tests/game/`); Step 8 `templates/basic/tests/game/sample.rs`; Step 9 this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
- **ordered steps:** (1) 01's tumble as a tween; (2) the props' boxes and the bomb's fuse off their prefabs and the manifest; (3) the props' fuels off their prefabs; (4) the toys' bodies off their prefabs in 01's code; (5) the toys' bodies off their prefabs in 01's tests; (6) the gun's bullet, muzzle and magazine off their data; (7) the orbs, the missile and the label colours off their data; (8) the template's restated prefab data; (9) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes and the demo (§3's Step 9) works; S-7 and S-13 read `landed <sha>` in phase 7's §2.2, a skipped step's rows moved with their reason (§6); the plan deleted with its citations and `example-01-restated-data` out of Now.

Item `example-01-restated-data` in [ROADMAP.md](../../ROADMAP.md), phase 7's 7.13d's restated data (§2.8, [phase 7](phase-7.md)), from the redo's 7.13 row and phase 7's Step 13. Written 2026-11-16 at `2cbfaf2d`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- 7.13d's toys and data closed (`D-283`, `2cbfaf2d`) with its Step 9, the tumble as a tween, stopped at 01's twin-save test, and S-13's constants split to this plan (§2.1 at `3976ba81`: 37 constants at the register's sites and two in the template, about eight steps). This plan is their eight and the close: nine, within phase 7's §6 ten.
- Two read-only audits at `2cbfaf2d` (the props; the toys, gun, orbs, HUD and colours) and this session's reads of the tumble, the twin test, the template and the pins sized them. No save migration reads any of these constants (`to_30`…`to_39` hold their own frozen `S2x`/`S3x` names); every value is reachable from a `World` where code uses it; the registries' lookups borrow without allocating, but `Value::pointer` and a clone into `from_value` allocate (A6).
- The register's sketch reads the props' boxes from 01's bodies data, as the actors' are; that family is locked (`D-223`), and a prop has no hurtbox or points there, so the props read their prefabs, the one source the kit spawns them from (A8, Q2).
- It stands on `D-223` (locked families), `D-230` (01's records), `D-275` (engine records), `D-281` (the headless load interning sprite names) and `D-283` (noted sprite sizes, `with_channel_eased`, `with_despawn_at_end`).
- Rules: tests are the spec (§2.2); no art moves; `D-226`; code at xhigh; schema 39 kept (§2.10).
- Later items expect: 7.13g's parked seams to take §2.11's Later line; 7.13e to find 01's level facts (S-160) beside the dead hazard boxes named there.

## 1. Audit

Read at `2cbfaf2d`, with two read-only audits (the props' boxes, centres and fuels; the toys, gun, orbs, HUD and colours). Paths under `examples/01_platformer/` are written `src/…`, `tests/…`, `tools/…`, `assets/…`; `T/` is `templates/basic/`; `core/` is `crates/tungsten-core/src/`, `K/` `crates/tungsten-kit/src/`, `U/` `crates/tungsten/src/`. Each `tools/content/` file is copied byte-equal to `assets/` (`tools/generate.py:914-920`).

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Phase 7 sends this plan S-13 (01's and the template's constants restating loaded data) and S-7's tumble, stopped at 7.13d's toys and data's Step 9 | `docs/plans/phase-7.md:253`, `:254`, `:420`; `docs/plans/seam-register.md:248`; `git show 3976ba81:docs/plans/example-01-toys-data.md` (§2.1's S-13 row, the evidence log's Step 9 row) |
| A2 | The tumble today: `ScrapTumble { born_at, initial_angle, total_turn }` turns a scrap or plank by `initial + total · (1 − e^(−4·age))` on the drawn clock; `iron_look` writes it to the scrap and its frost children each frame, the props' look to a plank; `spawn_scrap` takes one; the shatter gives `variation · TAU` and `(4 + 3 · variation) · ±1`, the plank's crumble 0 and `side · (1 + row % 2) · π`; `toys::rest` gives a restored scrap or plank a zero-turn one at its saved angle; the census lists `ScrapTumble` as presentation on scraps and planks; three test literals spawn zero-turn scraps | `src/features/toys/components.rs:55-70`; `src/features/toys/mod.rs:497-553`; `src/features/toys/systems.rs:1630-1685`; `src/features/elements/systems.rs:1450-1469`; `src/features/props/mod.rs:349`, `:394-399`; `src/features/props/systems.rs:328-341`; `tests/game/saves.rs:572-584`, `:693-706`; `tests/game/brick.rs:333`; `tests/game/enemies.rs:457`, `:4721` |
| A3 | The twin test: after a capture and a load, one step on each run, each saved entity's component types must include its twin's, `EMITTER` aside for every kind; `lost(kind)` names what an engine record's load does not give back (`EMITTER` on the missile, bullet and flask) and the census counts it as a home for engine records; scraps and planks are engine records (`spawn_toy` prepares and saves a prefab) | `tests/game/saves.rs:356`, `:367-372`, `:888-912`, `:948-970`, `:1140-1184`; `src/features/toys/mod.rs:293-297`; `core/assets/prefab.rs:578-590` |
| A4 | Step 9's stop: built as its §2.11 said, the twin test failed with `{"plank": {"tungsten_core::tween::Tween"}}`: a load gives a mid-tumble plank back with no tween where its twin holds one; the new `brick::a_scrap_tumbles_by_its_tween` passed once its frost check waited a frame; every file restored | the toys and data plan's evidence log at `3976ba81`, Step 9 |
| A5 | Core's tween: `ExpoOut` is `1 − 2^(−10t)`, the tumble's curve over `10 · ln 2 / 4` s (1.733 s) but its last 2^−10 snapped; `with_despawn_at_end` and `with_channel_eased` exist; the tween tick (`TWEEN_TICK`) and 01's present systems both run in `PostUpdate` | `core/tween.rs:96-101`, `:459-482`; `U/plugins.rs:186-195`; `src/features/toys/mod.rs:222-230` |
| A6 | Reading loaded data: `PrefabRegistry::get` (a `BTreeMap<String, _>` by `&str`) and `DataRegistry::get` borrow without allocating; `PrefabData` holds `sprite`, `scale` and `components` (`serde_json::Value`s); `Value::get` and `as_f64` borrow, but `Value::pointer` allocates per token and `from_value(value.clone())` allocates (01's `prefab_rule` does); `enemies::rest_seconds` walks a prefab by `.get()` chains; no 01 prefab uses `extends` | `core/assets/prefab.rs:81-118`, `:324`; `core/assets/data.rs:219`; `src/features/toys/mod.rs:269-278`; `src/features/enemies/systems.rs:683-694`; `assets/prefabs/` |
| A7 | The props' boxes: `CRATE_HALF`/`CENTRE`, `KEG_HALF`/`CENTRE`, `BOMB_RADIUS`/`CENTRE`, `ROCK_RADIUS`/`CENTRE`, `PLANK_HALF`/`CENTRE`, `STONE_HALVES`/`CENTRES`, each its prefab's `rigid_body` shape and `offset`, read through the const fns `half` and `centre`; sites: `fits` (a summon's cast), `spawn` and `spawn_capped` (run start, a break), `spawn_bomb`, `lob_rock`, `break_crate`; `roll` reads both radii every fixed step and the props' draw the plank's centre every frame, both inside 01's pit window; the tests use the boxes on about 45 lines (props 16 of the crate's, enemies 16, saves 11, spells 6, player 3, audio 2, lightning 1), and `props::half` on three | `src/features/props/mod.rs:60-88`, `:208-232`, `:258-300`, `:402`, `:424`, `:612-613`; `src/features/props/systems.rs:45-58`; `src/features/props/draw.rs:75`; `tests/game/guards.rs:16-36` |
| A8 | Props as bodies data: `BodyFile` requires a hurtbox and denies unknown fields, `Body::collider` builds a box; the generator's `bodies()` and `check_bodies` take actors' boxes and hurtboxes from `actors.BODIES`; the `bodies` family is locked (eight files, 2026-10-28), a file gained failing every run until the owner unlocks it; the prefabs are `content`, source-driven and never locked | `src/shared.rs:82-151`; `tools/generate.py:751-800`, `:943-946`; `tools/locks.json` (`bodies`); `tools/README.md:72-80`; `D-223` |
| A9 | The props' fuels: `BURN_SECS` (8) and `PLANK_BURN_SECS` (4), the crate's and the plank's `stats.fuel`; read by `whole_burn` (a burning prop's step), `look` (every frame) and the props' draw (every frame), and at a crate's break; `elements::BURN_SECS` (10) is another constant | `src/features/props/mod.rs:90`, `:103`, `:402`; `src/features/props/systems.rs:107-114`, `:326`, `:338`; `src/features/props/draw.rs:64`, `:98`, `:110`; `src/features/elements/mod.rs:35` |
| A10 | The bomb's fuse: `BOMB_TIP` (10.5, 0.5) is the manifest's `fuse` point on `ex10_goblin_bomb_{0,1}`; `bomb_tip` turns it about `BOMB_CENTRE` and only `props.rs`'s roll test calls it | `src/features/props/systems.rs:39`, `:60-66`; `assets/manifest.json` (`/sprites/ex10_goblin_bomb_0/points/fuse`); `tests/game/props.rs:528` |
| A11 | The toys: `BALL_RADIUS` (15) and `SMALL_BALL_RADIUS` (7.5), `BALL_CENTRE` and `SMALL_BALL_CENTRE`, `IRON_HALF` (60, as `4 · BALL_RADIUS`), `IRON_MASS` (400), `SCRAP_HALF` (15, as `IRON_HALF / 4`), `SCRAP_MASS` (25, as `IRON_MASS / 16`), each its prefab's `rigid_body` shape, `offset` or `mass`; const contexts `CONTACT_DISTANCE` and `COVER_REACH`; sites in `ball_impacts`, `flame_exposure`, `crush`, `impact`, `body_impacts` and `shatter_iron` (each step, early or empty in the pit), `wisp::burning` (every step, in the pit), and the spawns (none in the window); `IRON_MASS` and `SCRAP_MASS` read nowhere but each other and the tests; the tests use them on about 125 lines in 13 files, `const IRON_Y` in `gun.rs` and `lightning.rs` among them | `src/features/toys/mod.rs:42-83`, `:375`, `:423-449`, `:465`, `:482`, `:510`; `src/features/elements/systems.rs:57`, `:60`, `:250-255`, `:588`, `:659`, `:745`, `:781-783`, `:1220`, `:1457`; `src/features/enemies/wisp.rs:48-50`; `assets/prefabs/{bowling_ball,small_ball,iron_block,iron_scrap}.json`; `tests/game/gun.rs:35`; `tests/game/lightning.rs:45` |
| A12 | The gun: `BULLET_CENTRE` (16, 1.5) and `BULLET_RADIUS` (2), the bullet's `hitbox` offset and circle; `MAX_RICOCHETS` (2), its `projectile.bounces` (`Projectile::bounces()` public); `MUZZLE_OFFSET` (24), the gun ability's spawn `offset`; `MAGAZINE` (9), its `charges.max`, as `ROUNDS`, the HUD's `[WidgetId; ROUNDS]` built at spawn, its update showing `most` of them every frame; the HUD record `Hud` is `Copy`, copied each frame, so a `Vec` in it would allocate a frame; `UiTree::children` borrows a slice; `Payload::spawn()` is private, its `Spawn` and `SpawnWith` variants public, `SpawnWith::offset()` and `Ability::charges()` public | `src/features/gun/mod.rs:37-68`, `:110-121`; `src/features/gun/systems.rs:81`, `:170`, `:360-390`, `:475-481`, `:697`; `src/features/spells/hud.rs:93`, `:255`, `:928`, `:1030-1040`, `:1176-1190`; `core/ui/tree.rs:564-568`; `K/intent/ability.rs:182-245`, `:364-372`, `:1014`; `assets/data/abilities/gun.json` |
| A13 | The orbs: `ORB_SECS` (20, the orbs' `lifetime.seconds`) is read by no code; `ORB_CENTRE` and `ORB_HALF` (12, 12) are their `pickup` offset and circle as a box, read at an orb's adoption and in `in_rock`. The missile: `MISSILE_SPEED` (900, its ability's spawn `speed`) feeds only `MISSILE_RANGE` (read by nothing; the prefab's `projectile.range` is 1,890); `MISSILE_SIZE` (40) is the prefab's `scale` (0.625) of its 64 px art, read at its spawn and by `bend`, `missile_contacts` and `missile_ends` (each step, over no missile in the pit) | `src/features/enemies/drops.rs:4`, `:37-41`, `:134-140`, `:170-180`; `src/features/toys/mod.rs:117-120`, `:586-596`; `src/features/toys/systems.rs:443`, `:489`, `:511`; `assets/prefabs/{health_orb,fire_missile}.json`; `assets/data/abilities/fire_missile.json` |
| A14 | The colours: `School` and its `color` table are read by no code but `combat.rs`'s two label tests; each colour is the sRGB byte encoding of its damage type's linear `label_style.color` (Mana has no damage type); `combat.rs`'s `YELLOW` is the encoding of the crit rule's `label.crit.color`; the kit's `srgb_byte` is crate-private | `src/features/spells/components.rs:88-114`; `tests/game/combat.rs:381-386`, `:416-417`, `:453`; `assets/data/damage_types/game.json`; `assets/data/cue_rules/numbers.json` (`/0/do/0/label/crit/color`); `K/present/labels.rs:487` |
| A15 | `DECK_DEPTH` (23) in `level.rs` is twice the platform prefabs' `rigid_body` half height (11.5); `SPIKES_HALF` there and `playthrough.rs`'s hazard boxes restate tools/level.py's spikes and fire prefabs, which no map places (`tools/level.json`'s `hazards` is empty), so their branches never run | `tests/game/level.rs:101-103`, `:325`, `:541-542`, `:596`, `:768`, `:987`; `tests/game/playthrough.rs:180-200`; `assets/prefabs/ex10_platform_*.json`; `tools/level.py:21-23` |
| A16 | The template's tests: `PLAYER_SPEED` (240, the player prefab's `top_down_mover.speed`) with `ONE_STEP` from it, `ramp`'s 30 px/s a step (its `acceleration` 1,800 over 60), `HAZARD_SWING` (240) and `HAZARD_PERIOD_FRAMES` (240), the hazard prefab's `oscillator.position` amplitude and period (4 s) at 60 Hz; the template's tests have no `serde_json`; the literals 240 and 120 at `:420`–`:451` are assertions | `T/tests/game/sample.rs:34-48`, `:176-240`, `:340-395`, `:420-451`, `:632-640`, `:799`; `T/assets/prefabs/{player,hazard}.json`; `T/Cargo.toml` |
| A17 | `data.rs`'s `every_restated_constant_matches_its_data` holds S-13's constants to their files beside lines outside the row (the actors' prefabs against their bodies, the walkers' step and shove, the courier's, troll's and crawler's numbers, the freeze and chill builds, the impact rules, the snowball); no migration reads those out-of-row constants either, though their comments say a migration restates them | `tests/game/data.rs:69-265`; `src/save.rs:100-176`, `:189-535`; `src/features/player/mod.rs:65-86` |
| A18 | Pins: 01's pit (61,869 allocations, 63,943,417 bytes, 298 moves, 295 spawned, 243 despawned, 16 events, no sprite drawn) over 60 frames after a New Game with 2,000 balls and six awake foes, nothing cast; 01's run tests 483 passed, 1 ignored (this session, 51.3 s); the template's 184 (this session); schema 39; the template's `AGENTS.md` at 12,277 and the LLM index at 12,278 of 12,288 bytes; `D-283` the last decision | `tests/game/guards/pit.json`; `tests/game/guards.rs:16-36`; `src/save.rs:98`; `scripts/check-agent-context.py:21-27`; `DECISIONS.md:4429` |
| A19 | ROADMAP's Later line on 01's props' and toys' boxes and anchors as data names `STONE_*`, `CRATE_*`, `KEG_*`, `MUZZLE_OFFSET`, `BOMB_TIP` (S-13 here), and `SPRAY_MUZZLE`, `SCORCH_ANCHOR` and the wisp prefab's glow offset (no loaded copy: the glow offset is data equal to its body's centre, unchecked) | `ROADMAP.md:46`; `src/features/toys/systems.rs:61`; `src/features/lightning/mod.rs:73`; `tools/content/prefabs/wisp.json:105-107`; `assets/data/bodies/wisp.json` |

## 2. Design

### 2.1 Scope: what lands, what goes on

| Row | Its gap at `2cbfaf2d` | The change |
| --- | --- | --- |
| S-7's tumble | A tween's load gives a mid-tumble plank or scrap back without its tween, which the twin test refuses (A3, A4) | Step 1: the tumble a tween, the tween in `lost` for scraps and planks (Q1) |
| S-13 | 37 constants restate the props', toys', gun's, orbs', missile's and colours' loaded data, and two in the template; the props' bodies data is a locked family (A7–A16) | Steps 2–8, read off the prefabs, abilities, damage types, cue rules and manifest; the out-of-row lines and 01's own tuning go on (§2.11) |

Nine steps, within §6's ten, so one plan.

### 2.2 Tests are the spec

- Each step keeps 01's 483 run tests passing (484 after Step 1's new test, 485 after Step 2's, 486 after Step 4's, 487 after Step 6's), and the template's 184, with their assertions as written. A test's body changes only where it reads, spawns or names what the step removes, ported to its equivalent read off the data, each port counted in the evidence row.
- A computed value moves only where the step's Moves names it, within the asserting test's own tolerance. Each read value equals its constant today (A7–A16), so S-13's steps move none; Step 1 moves the tumble as §2.9 says. Any other changed outcome stops the step (§6). No test is deleted but one whose subject the step deletes.
- New tests only where a done-when names them: one per data group that proves no Rust copy is left, by moving the value in the loaded data (as `data::a_body_moved_in_its_data_file_moves_the_spawned_body` does for the actors).

### 2.3 Reading 01's loaded data

- `shared::prefab_part(world, prefab, path)` walks a loaded prefab's `components` by `Value::get` along `path` (`&["rigid_body", "shape", "radius"]`), borrowing; `shared::prefab_box(world, prefab)` gives its `rigid_body`'s half size (a circle's radius as a square's) and its `offset`, and `shared::prefab_number` a number. Each panics naming the prefab and path when it is missing, as `shared::body` does for a body (a manifest without them is a broken build, which every harness test meets first). No reader calls `Value::pointer` or clones into `from_value`, so a read at a per-step or per-frame site allocates nothing (A6).
- At a site with the entity in reach, its own component is read instead (`Collider`, `Pickup`, `Projectile::bounces()`), by `world.get` inside the loop it already runs; no step adds a query item or widens a collected tuple, so no query shape moves the pit (A18).
- A const context becomes a value read once a call, after its early returns (`CONTACT_DISTANCE`, `COVER_REACH`).
- The tests read the files the game loads, as `actor_body` and `kind_info` do: `prefab_box(prefab)` and `prefab_number(prefab, pointer)` in `tests/game/main.rs`, moved from `data.rs`'s `body`, `json`, `number` and `pair` (a test may call `Value::pointer`).
- No public kit accessor is added: 01 matches `Payload::Spawn { offset, .. }` and `Payload::SpawnWith(spawn)` (`spawn.offset()`) itself with a wildcard arm, and the label tests encode linear to sRGB themselves, an independent check of the kit's own encoding (A12, A14).

### 2.4 The props' boxes and the bomb's fuse (Step 2)

- `props::half(world, kind)` and `props::centre(world, kind)` read their kind's prefab (`prefab_box`); the box constants go. `fits`, `spawn`, `spawn_capped`, `spawn_bomb` and `lob_rock` read them before they spawn; `break_crate` the plank's centre; `roll` both radii once a call, before its two loops; the props' draw the plank's centre once, before its query.
- `BOMB_TIP` and `bomb_tip` go: the roll test turns the manifest's `fuse` point (`assets/manifest.json`'s `/sprites/ex10_goblin_bomb_0/points/fuse`) about the bomb prefab's offset itself, an independent check of the kit's point (A10).
- The tests' boxes read `prefab_box`; `level.rs`'s `DECK_DEPTH` reads a platform prefab's (A15); `data.rs`'s props' and bomb's and rock's lines go.
- New: `data::a_props_box_moved_in_its_prefab_moves_its_spawn` (the crate's prefab inserted with a half of 20 × 24 and an offset of 30, 40: `props::half` and `props::centre` give them, and a crate spawned at a centre has its corner 30, 40 short of it and its collider those).

### 2.5 The props' fuels (Step 3)

- `props::whole_fuel(world, kind)` reads the kind's prefab's `stats.fuel`; `BURN_SECS` and `PLANK_BURN_SECS` go. `whole_burn` and `break_crate` call it; `look` and the props' draw read the crate's and the plank's once a call. The tests read `prefab_number(prefab, "/components/stats/fuel")`; `data.rs`'s fuel lines go.

### 2.6 The toys' bodies (Steps 4 and 5)

- Step 4, 01's code: the spawns subtract their prefab's `rigid_body.offset` (today's arithmetic: 60 + 4 = 64, 15 + 1 = 16, 16 and 8); `iron_fits` reads the iron's half; `ball_impacts`, `crush`, `impact` and `body_impacts` read each body's own `Collider` where they read the constants; `flame_exposure` and the contact pass read the two ball radii once a call for `CONTACT_DISTANCE` and `COVER_REACH`; `shatter_iron` the scrap's half; `wisp::burning` the balls' offsets once, before its query. The constants stay, read by the tests alone.
- Step 5, 01's tests: about 125 lines read the prefabs (`prefab_box`, `prefab_number`); `const IRON_Y` in `gun.rs` and `lightning.rs` becomes a function; the constants go, with `data.rs`'s toys' lines.
- New at Step 4: `data::a_toys_body_moved_in_its_prefab_moves_its_spawn` (the iron block's prefab inserted with a half of 50 and an offset of 54: `toys::place_iron` at a centre lands its corner 54 short of it with a collider of half 50; `toys::iron_fits` at a centre 55 px from a placed crate's face refuses with the prefab as loaded (half 60) and takes it with the moved one; the small ball's offset set to 9, 9: a ball spawned at a centre lands its corner 9, 9 short of it).

### 2.7 The gun (Step 6)

- `paths` and `hits` read the bullet prefab's hitbox offset and circle once a call, passing the radius into `contacts`; `ricochet` reads the bullet's `Projectile::bounces()`; `shoot` reads the gun ability's spawn offset (§2.3). The HUD builds the magazine's rounds at spawn from the gun's most charges (`gun::rounds`, which the update already reads off the gun's ability), as the row's children, and its update walks them by index (`tree.children(row).get(i)`), with no list in `Hud`; `ROUNDS`, `MAGAZINE` and the `rounds` field go. The constants go with `data.rs`'s gun lines; `gun.rs`'s tests read the ricochets off the prefab.
- New: `hud::a_magazine_shows_its_guns_charges` (the gun's ability inserted in the `DataRegistry` with a `charges.max` of 5 before the run's start: the magazine's row holds 5 rounds, all shown live, one spent after a shot).

### 2.8 The orbs, the missile and the label colours (Step 7)

- `ORB_SECS` goes, `enemies.rs` reading an orb's `Lifetime`; an orb's adoption reads its `Pickup` offset and `in_rock` its pickup's half extents; `ORB_CENTRE` and `ORB_HALF` go.
- `MISSILE_SPEED` and `MISSILE_RANGE` go, the tests reading the ability's spawn `speed`; `MISSILE_SIZE` goes: the missile's spawn and its systems take its prefab's `scale` times its sprite's size (`shared::sprite_size`, noted headless, `D-283`), computed inside their loops, so the pit's empty loops read nothing.
- `School` goes with its table; `combat.rs` reads each damage type's `label_style.color` and the crit rule's colour off their files and encodes them to sRGB bytes, `YELLOW` going with it.

### 2.9 01's tumble (Step 1)

- `ScrapTumble` goes, as the toys and data plan's §2.11 built it: a scrap's or plank's tumble is a rotation tween, `ExpoOut` over `TUMBLE_SECS` (`10 · ln 2 / 4`, 1.733 s) from its initial angle to that plus its turn, the old curve but its last 2^−10 of the turn (at most 0.007 rad) snapped at its end; a zero turn adds none. `spawn_scrap` takes the angle and the turn and sets the scrap's rotation to the angle at once (the shatter's `variation · TAU`, which a prepared toy's zero rotation does not hold); the plank's crumble passes its own. The props' plank look stops turning it. `iron_look`, ordered after `TWEEN_TICK`, stops writing a scrap's rotation and copies it onto its frost children, which land at the flush after the scrap, so a new or restored scrap's frost turns with it. `toys::rest` goes: a restored scrap or plank keeps its saved angle with no tween, as `rest`'s zero turn kept it.
- **The twin test.** The rest of a tumble is lost at a load, as it was under `rest`: `saves.rs`'s `lost(kind)` gains `"scrap" | "plank" => &["tungsten_core::tween::Tween"]` (a literal, as `EMITTER` is); the census moves `Tween` there from where `ScrapTumble` was, so it keeps one home; and the twin test skips a type in its kind's `lost` beside `EMITTER`, its doc naming the tumble. Every other kind's set is checked as today (Q1).
- What moves: the tumble on the game clock, so past the step budget it runs ahead of the drawn clock by the dropped time; its last 2^−10 snapped at 1.733 s; the twin test's exclusions by the tween on scraps and planks; 01's census.

### 2.10 Pins, guards, saves and records

- **Unchanged:** the physics hashes; the kit's determinism and schedule pins and the template's; 02's pins; the views guard; `src/shell/`; the describe census; `api/`; `DESIGN.md`; the template's `AGENTS.md` and the LLM index (no new source file).
- **01's pit:** no step is designed to move it (§2.3). Step 1 replaces two look queries' shapes (`iron_look`'s and the plank look's lose `ScrapTumble`) and Step 6 drops the `rounds` array from `Hud`: if either moves a line, the move is quoted with its cause and decomposed by probe (the guard's size histogram, per-size backtraces, a control that gives the old pin back); a move its cause does not account for, or one at any other step, stops the step (§6).
- **Saves:** no step migrates; schema 39 stays: tweens and the HUD unsaved, no saved component changed.
- **Records:** 01's README rows a step makes wrong; `D-28[4]` at the close with the tungsten-decision skill, the next ID rechecked then (Q3).

### 2.11 What goes on

- **Later line (one):** 01's constants outside S-13's register sites that restate its data, held by `data.rs`'s other lines (the courier's health, reach and jump; the troll's and crawler's speeds and touch, whose comments wrongly name a migration; the walkers' step and shove; the freeze and chill builds' bands; the foes' impact masses; the snowball's shape), and elements' `FIRE_HALF` and `FIRE_CENTRE`, read off their data as S-13's are.
- **The dead hazard boxes:** `level.rs`'s `SPIKES_HALF` and `playthrough.rs`'s spikes and fire boxes restate prefabs no map places; their branches go with 01's level and its facts (7.13e, S-160), named in the close's follow-ups.
- **01's own tuning, no loaded copy:** the balls' and scraps' art scales (the prefabs hold no `scale`), `RELOAD_SECS` (the gun's `refill` is a never), `MISSILE_RADIUS` (the missile's projectile has no shape), `SCORCH_ANCHOR` and `SPRAY_MUZZLE`; the wisp's glow offset stays data equal to its body's centre. ROADMAP's Later line on boxes and anchors (A19) is cut to `SPRAY_MUZZLE`, `SCORCH_ANCHOR` and the glow offset.

## 3. Steps

Every step's done-when adds, from `examples/01_platformer` (Step 8 from `templates/basic`): right after its tests are written, `cargo clippy --all-targets -- -D warnings` → exit 0, before the long checks; `cargo fmt --all -- --check` (repo root) → exit 0; `just lint` → exit 0; `just repo-check` → exit 0; `cargo test --test game` → exit 0, 483 passed before the plan's new tests, 1 ignored, the Moves named; `saves::every_component_on_a_saved_entity_is_saved_rebuilt_or_presentation` and `saves::every_loaded_entity_holds_its_uninterrupted_twins_components` ok; `TUNGSTEN_GUARDS_WRITE=1 cargo test --test game -- guards::` then `cargo test --test game -- guards::` → exit 0 and `git diff --quiet tests/game/guards/` → exit 0, or §2.10's move quoted and decomposed. Each absence check's pattern was run on the tree at `2cbfaf2d` and matches there, but Step 2's `pointer(` guard, which matches nothing there.

### Step 1: 01's tumble as a tween

- **Files:** `examples/01_platformer/src/features/toys/components.rs`, `examples/01_platformer/src/features/toys/mod.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/props/mod.rs`, `examples/01_platformer/src/features/props/systems.rs`, `examples/01_platformer/src/features/elements/systems.rs`, `examples/01_platformer/tests/game/brick.rs`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json` (§2.10's move alone).
- **Change:** §2.9.
- **Done-when:** the common checks; the three zero-turn literals ported, their tests ok as written; the new `brick::a_scrap_tumbles_by_its_tween` (a scrap spawned with angle 0.7 and turn 4: its rotation 0.7 at once and at 0.25 and 1 s within 0.001 rad of `0.7 + 4 · (1 − e^(−4t))`, at 2 s 4.7 and tween-free; a shattered block's sixteen scraps starting at fifteen nonzero angles, their frost turning with them from the first frame after their flush; a capture and a load mid-tumble: each scrap and its rebuilt frost at its saved angle, no tween) ok; the twin test ok as written but its `lost` exclusion, over the mid-tumble plank its capture holds today (A4); `rg -q 'ScrapTumble|pub fn rest\b' src/features/toys src/features/props src/features/elements tests; test $? -eq 1` → exit 0.
- **Commit:** `refactor(example-01): the scraps' and planks' tumble as tweens`
- **Moves:** §2.9's; 01's census; the pit's bytes only as §2.10 names.

### Step 2: The props' boxes and the bomb's fuse off their prefabs and the manifest

- **Files:** `examples/01_platformer/src/shared.rs`, `examples/01_platformer/src/features/props/mod.rs`, `examples/01_platformer/src/features/props/systems.rs`, `examples/01_platformer/src/features/props/draw.rs`, `examples/01_platformer/tests/game/main.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/props.rs`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/spells.rs`, `examples/01_platformer/tests/game/player.rs`, `examples/01_platformer/tests/game/audio.rs`, `examples/01_platformer/tests/game/lightning.rs`, `examples/01_platformer/tests/game/level.rs`.
- **Change:** §2.3, §2.4.
- **Done-when:** the common checks; `props::a_bomb_rolls_with_its_fuse_and_a_rock_stays_a_body_that_rolls` ok as written but its tip's source; the new `data::a_props_box_moved_in_its_prefab_moves_its_spawn` ok; `rg -q '\b(CRATE|KEG|PLANK)_(HALF|CENTRE)\b|\b(BOMB|ROCK)_(RADIUS|CENTRE)\b|STONE_(HALVES|CENTRES)|BOMB_TIP|bomb_tip|DECK_DEPTH|const fn (half|centre)\(' src tests; test $? -eq 1` → exit 0; `rg -q 'pointer\(' src; test $? -eq 1` → exit 0 (a guard: `src/` calls none today, A6).
- **Commit:** `refactor(example-01): the props' boxes read off their prefabs`
- **Moves:** none.

### Step 3: The props' fuels off their prefabs

- **Files:** `examples/01_platformer/src/features/props/mod.rs`, `examples/01_platformer/src/features/props/systems.rs`, `examples/01_platformer/src/features/props/draw.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/props.rs`.
- **Change:** §2.5.
- **Done-when:** the common checks; every `props::` and `burning::` test ok as written; `rg -q 'PLANK_BURN_SECS|\bBURN_SECS\b' src/features/props tests/game/props.rs tests/game/data.rs; test $? -eq 1` → exit 0 (`burning.rs`'s `BURN_SECS` is elements', A9).
- **Commit:** `refactor(example-01): the props' fuels read off their prefabs`
- **Moves:** none.

### Step 4: The toys' bodies off their prefabs in 01's code

- **Files:** `examples/01_platformer/src/features/toys/mod.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/elements/systems.rs`, `examples/01_platformer/src/features/enemies/wisp.rs`, `examples/01_platformer/tests/game/data.rs` (the new test).
- **Change:** §2.6's code.
- **Done-when:** the common checks; the new `data::a_toys_body_moved_in_its_prefab_moves_its_spawn` ok; `rg -n '\b(IRON_HALF|SCRAP_HALF|IRON_MASS|SCRAP_MASS|BALL_RADIUS|SMALL_BALL_RADIUS|BALL_CENTRE|SMALL_BALL_CENTRE|CONTACT_DISTANCE|COVER_REACH)\b' src | rg -v '^src/features/toys/mod\.rs:[0-9]+:(pub const |\s*//)'; test $? -eq 1` → exit 0: the constants left in `toys/mod.rs`'s definitions and docs alone.
- **Commit:** `refactor(example-01): the toys' bodies read off their prefabs`
- **Moves:** none.

### Step 5: The toys' bodies off their prefabs in 01's tests

- **Files:** `examples/01_platformer/src/features/toys/mod.rs` (the constants removed), `examples/01_platformer/tests/game/main.rs`, `examples/01_platformer/tests/game/data.rs`, and the tests naming the constants: `ice.rs`, `gun.rs`, `saves.rs`, `props.rs`, `player.rs`, `enemies.rs`, `tomes.rs`, `spells.rs`, `lightning.rs`, `cascades.rs`, `brick.rs`, `spawning.rs`, `ball_pit.rs` under `examples/01_platformer/tests/game/`.
- **Change:** §2.6's tests.
- **Done-when:** the common checks; `rg -q '\b(IRON_HALF|SCRAP_HALF|IRON_MASS|SCRAP_MASS|BALL_RADIUS|SMALL_BALL_RADIUS|BALL_CENTRE|SMALL_BALL_CENTRE|IRON_Y)\b' src tests; test $? -eq 1` → exit 0.
- **Commit:** `refactor(example-01): the toys' tests read their bodies off the prefabs`
- **Moves:** none.

### Step 6: The gun's bullet, muzzle and magazine off their data

- **Files:** `examples/01_platformer/src/features/gun/mod.rs`, `examples/01_platformer/src/features/gun/systems.rs`, `examples/01_platformer/src/features/spells/hud.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/gun.rs`, `examples/01_platformer/tests/game/hud.rs`, `examples/01_platformer/tests/game/guards/pit.json` (§2.10's move alone).
- **Change:** §2.7.
- **Done-when:** the common checks; every `gun::` and `hud::` test ok as written but its reads; the new `hud::a_magazine_shows_its_guns_charges` ok; `rg -q '\b(MAGAZINE|ROUNDS|BULLET_CENTRE|BULLET_RADIUS|MAX_RICOCHETS|MUZZLE_OFFSET)\b' src tests; test $? -eq 1` → exit 0.
- **Commit:** `refactor(example-01): the gun's bullet, muzzle and magazine off data`
- **Moves:** the pit's bytes only as §2.10 names.

### Step 7: The orbs, the missile and the label colours off their data

- **Files:** `examples/01_platformer/src/features/enemies/drops.rs`, `examples/01_platformer/src/features/toys/mod.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/spells/components.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/spells.rs`, `examples/01_platformer/tests/game/ice.rs`, `examples/01_platformer/tests/game/combat.rs`.
- **Change:** §2.8.
- **Done-when:** the common checks; `combat::numbers_float_in_their_types_colour_and_a_crit_larger_in_yellow`, `combat::two_hits_in_a_step_float_apart_and_only_the_crits_in_yellow`, `spells::mouse4_casts_a_fireball_toward_the_cursor` and every `spells::` missile test ok as written but their reads; `rg -q '\b(ORB_SECS|ORB_CENTRE|ORB_HALF|MISSILE_SPEED|MISSILE_RANGE|MISSILE_SIZE|YELLOW)\b|\bSchool\b' src tests; test $? -eq 1` → exit 0.
- **Commit:** `refactor(example-01): orbs, missile and label colours read off data`
- **Moves:** none.

### Step 8: The template's restated prefab data

- **Files:** `templates/basic/tests/game/sample.rs`.
- **Change:** the tests read the player prefab's `top_down_mover.speed` and `acceleration` and the hazard prefab's `oscillator.position` amplitude and period through a `prefab_number(harness, prefab, pointer)` helper over `PrefabRegistry::get`'s `components` (`Value::pointer`, an inherent method, so no `serde_json` dependency), the steps a period takes from `Time`'s fixed step; `PLAYER_SPEED`, `ONE_STEP`, `HAZARD_SWING` and `HAZARD_PERIOD_FRAMES` go, `ramp` taking the acceleration; the literal assertions at `:420`–`:451` stay (A16).
- **Done-when:** `cargo test -p tungsten-template-basic --locked` → exit 0, 184 passed, their assertions as written; `cargo clippy -p tungsten-template-basic --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, `just lint`, `just repo-check` → exit 0; `rg -q 'PLAYER_SPEED|ONE_STEP|HAZARD_SWING|HAZARD_PERIOD_FRAMES|30\.0 \* step' templates/basic/tests; test $? -eq 1` → exit 0.
- **Commit:** `test(template): the sample's tests read the prefabs' speeds and swing`
- **Moves:** none.

### Step 9: Close

- **Files:** this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
- **Change:** `D-28[4]` with the tungsten-decision skill (Q3; the next ID rechecked); phase 7's §2.2: S-7 and S-13 `landed`, S-7's cell naming the tumble's tween and the looks that stay 01's, S-13's the props off their prefabs and §2.11's remainder, a skipped step's rows `open` with their move; §2.8's entry closed with this plan's last commit; phase 7's evidence row "13 (7.13), part d's restated data", naming this plan's evidence log at its last commit; ROADMAP's Now: the phase-7 line naming the next plan by §2.8; Later: §2.11's line, the boxes-and-anchors line cut (A19); the follow-ups (the dead hazard boxes to 7.13e's plan as a note in phase 7's S-160 cell, or Later); the plan and its citations deleted; `example-01-restated-data` out of Now.
- **Done-when:** `just check` → exit 0; `just physics-release` → exit 0, the hashes unchanged; `just smoke` → passes; `just api-check`, `just docs-ratchet`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; before the deletion, `rg -c '^\| (S-7|S-13) \| [^|]+ \| 7\.13 \| landed ' docs/plans/phase-7.md` → `2`, or the evidence log naming a skipped row; after it, `rg -l 'plans/example-01-restated-data\.md' --glob '!CHANGELOG.md' .; test $? -eq 1` → exit 0; `WGPU_BACKEND=vulkan just perf sitting --quick` → `e01-pit` quoted as information, not evidence; the demo: from `examples/01_platformer`, `tungsten run --script --report <file>` on a scratch script (`start_run`; the iron block cast over the clearing's crate stack (`tools/level.json`'s first three crates, x 12.2–13.1 tiles), falling onto it: `capture` every 4 frames for 24 frames from the break, the planks' tumble; the ice spray held at the iron until it is frozen and the fire missile cast at it: `capture` every 4 frames for 24 frames from the shatter, the scraps' tumble; the gun fired three times at a wall, `capture` the HUD's magazine; `assert` `count` of `iron_scrap` 16 after the shatter) → exit 0, its asserts passed, its captures the sheet under `perf-runs/<date>-restated-data/`.
- **Ends with:** the commits, the evidence, what the owner should try (01 plays and looks as before: the scraps and planks tumbling as they fly, the crates and kegs, bombs and rocks, the balls, iron and scraps where they were, the gun's nine rounds and its ricochets, the orbs, the missile and the damage numbers' colours), the next prompt (tungsten-next's, the next 7.13 plan by §2.8).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The tumble's way out of the twin test: a resting zero-turn tween at a restore, or the test naming the tween a load drops | The test names it in `lost` for scraps and planks: the rest of a tumble was already lost at a load (`rest`'s zero turn), and a resting tween would be a component that draws nothing, there to pass the test; the census keeps one home for it (A3, §2.9) |
| Q2 | The props' boxes as 01's bodies data (S-13's sketch) or off their prefabs | Off their prefabs: the `bodies` family is locked (`D-223`) and the owner unlocks it; a prop has no hurtbox or points for a body file, and the prefab stays the kit's spawn source, so bodies data would be a second copy the generator holds equal (A8) |
| Q3 | A decision at the close | One, `D-28[4]`: where 01's restated values live constrains later stops (the props' boxes off their prefabs, not bodies data; a presentation tween a load drops named in the census's `lost`) |
| Q4 | `School` kept for the HUD's school borders, or gone | Gone: no code reads it (A14); a border that wants a school's colour reads its damage type then |
| Q5 | The magazine's rounds: a list in `Hud`, or the row's children | The row's children: `Hud` is `Copy` and copied each frame, and a list in it would allocate a frame in the pit (A12) |
| Q6 | `MISSILE_SIZE` from the prefab's `scale` and sprite, or its glow's offset | The `scale` and sprite (`D-283`'s noted sizes): the glow's offset is itself a copy of the centre |
| Q7 | The out-of-row constants (§2.11) here, or Later | Later, one line: they sit outside S-13's register sites, and taking them would pass ten steps (§6) |
| Q8 | The balls' and scraps' art scales into their prefabs' `scale` | No: 01's own tuning, read by no data; moving them is a data change for no restated copy |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** 01's census (Step 1); the twin test's exclusions (Step 1); 01's pit bytes at Steps 1 and 6 as §2.10 names, quoted and decomposed; 01's tumble as §2.9 names.
- **Must not move:** the physics hashes (`just physics-release`); the kit's determinism and schedule pins and the template's; 02's pins; the views guard; the template's tests' outcomes; `src/shell/`, byte-equal to the template's; the perf rows' scripts; every locked family's files and `tools/`; 01's tests' assertions; 01's schema; the describe census; `api/`; the template's `AGENTS.md` and the LLM index.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it: Step 3 depends on 2 (its reader), Step 5 on 4, Steps 4, 6 and 7 on 2's reader; 1 and 8 on none. A skipped step's constants stay, its rows `open` at the close with the stop's reason; the close's checks count the steps that landed.
- A test whose assertion must change outside §2.2's Moves: stop the step; never widen a tolerance.
- A change to core, the kit, the umbrella, `tools/`, a locked family's file or `src/shell/`: stop the step.
- A pin moves in a line §2.10 holds, or past what its named cause accounts for: stop, decompose the move by probes, record it.
- A read value differs from the constant it replaces: stop the step and record both.

## 7. Critique

Not run: `critique: codex exec failed (exit 1, usage limit reached)` — "You've hit your usage limit … try again at Nov 17th, 2026 7:52 PM." Round 1 at xhigh, 2026-11-16, `--raw --repo` on the tree at `2cbfaf2d` with this plan untracked (about 44k characters): the critic stopped after 28 commands and 346.6 KB of output with no findings written; 5h 0 → 1 %, week 100 → 100 % (the week read 100 % before it, as it had for the toys and data plan's two rounds, which ran). The weekly window resets after this plan's run would start, so the round is recorded as not run, not queued, and no second round follows.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups
