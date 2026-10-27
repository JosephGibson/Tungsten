# Example 01: foes (`e01foes`) — draft

- **status:** in progress
- **goal:** Example 01 (m) ([feedback](1.0/example-01-feedback.md) §3.9, V17–V22, V25, V26): a beetle whose charge ploughs through crates and kegs and throws the player hard, with a tell that reads (a rearing pose, scraping dust, a rising sound) and stronger streaks; a frost crawler redrawn, afraid at 1.3 × the staff light's reach and backing out of it, quicker, hopping ledges and gaps, with a chilling snowball on a cooldown; a troll that lifts and tosses kegs and clears its way in Search and Return; goblins with a throw cooldown, a dash-in stab one approach in three and a panic while burning; their sounds; save schema 17. The art and sound come from a redo [§6 asset session](roadmap-redo.md#6-asset-sessions) on §3's brief, after the code steps, which build on placeholders until the art is accepted.
- **non-goals:** K30 in the kit (the beetle relaunches in 01; the kit row stays in the backlog). The bat's dive tell (V18 names it; Q51 picked the beetle's). Burning sounds and the player's damage tell (V31, `e01sfx`). Snowballs that douse fire or chill props. New kinds, the armoured troll (V43, `e01flask`). Any map, route, light or level change. Any kit, core, render or template change. A release, a changelog section or a decision entry (§5 Q14): a release is cut separately (`D-215`).
- **files to touch:** paths under `examples/01_platformer/` (01's) unless rooted; "the generated `assets/`" is the generator's output with `tools/outputs.json`.
  - Step 1, placeholders: `tools/actors.py`, `tools/effects.py`, `tools/generate.py`, `tools/sfx.py`, `tools/sounds.json`, `tools/test_generate.py`, `tools/README.md`; the generated `assets/` (sprites, animations, particles, sounds, `manifest.json`, `tilemaps/level.tmj`).
  - Step 2, the beetle: `src/features/enemies/{beetle,draw}.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/prefabs/beetle.json`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{enemies,presentation}.rs`, `README.md`; the generated `assets/`.
  - Step 3, the crawler's fear, retreat, speed and hops: `src/features/enemies/{crawler,goblin,systems,mod,hop}.rs` (`hop.rs` new), `src/features/audio/{components,mod}.rs`, `src/save.rs`, `tools/content/prefabs/crawler.json`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{enemies,saves,data}.rs`, `README.md`; the generated `assets/`.
  - Step 4, the snowball: `src/features/enemies/{crawler,components,systems,mod,troll,snowball}.rs` (`snowball.rs` new), `src/features/audio/{components,mod}.rs`, `src/save.rs`, `tools/content/prefabs/snowball.json` (new), `tools/content/data/cue_rules/sounds.json`, `tests/game/{enemies,saves,data}.rs`, `README.md`; the generated `assets/`.
  - Step 5, the troll: `src/features/enemies/troll.rs`, `src/features/elements/systems.rs`, `tests/game/enemies.rs`, `README.md`.
  - Step 6, the goblins: `src/features/enemies/{goblin,components,systems,mod}.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{enemies,saves}.rs`, `README.md`; the generated `assets/`.
  - Step 7, the asset session: `tools/{actors,effects,generate,sfx}.py`, `tools/pixels.py` (a new helper only, §3.2), `tools/{sounds,palette}.json`, `tools/README.md`; the generated `assets/`; this plan's §3.6 lock and evidence row.
  - Step 8, close: 01's `README.md`, `DESIGN.md`, `docs/plans/1.0/example-01-feedback.md` (§3.9's landed note), `docs/known-issues.md` (only for a finding), `ROADMAP.md`, this plan (deleted).
- **ordered steps:** (1) Placeholders for §3's contract. (2) The beetle's charge and tell. (3) The crawler's fear, retreat, speed and hops, schema 17. (4) The crawler's snowball. (5) The troll. (6) The goblins. (7) Art and sound, the asset session, ending in the owner's acceptance and the lock. (8) Close. §4 holds each.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes with 01's whole suite, `just smoke` runs 01, §3.6's lock holds, and in-game captures show each foe's new behaviour on the accepted art; the plan is deleted with its citations and `e01foes` is out of Now.

Item `e01foes` in [ROADMAP.md](../../ROADMAP.md), from the [feedback](1.0/example-01-feedback.md)'s §3.9 with V17–V22, V25, V26, Q51–Q56, Q73, Q74 and K30. Written 2026-10-26 at `433b499` on the `0.110` branch, after `e01tomes`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs: Steps 1–6 are one code session at xhigh that builds on Step 1's placeholders and draws nothing; Step 7 is an asset session on Opus 5.5 at max on §3 alone, which ends when the owner accepts its art and sound; Step 8 closes in a new code session. A run session stops before Step 7 unless its evidence row is in, ending with §3.7's prompt.

## Context digest

- The owner's fourth playtest (2026-10-25) asked for harder, more readable foes; §3.9 sketched the stop, Q51–Q56, Q73 and Q74 hold its defaults, K30 its one kit gap, routed in 01 (§6 of the feedback).
- It stands on 01's enemies on the kit's state machine (`D-201`), M81's props, goblin and troll throws (`D-210`), M84's walker shove and step (`D-213`), e01quick2's keg threshold (Q46) and sight past bodies (Q47), e01tomes' schema 16 and its first asset session.
- `D-215`: a commit per step, one critique round, the asset loop, the tiers (`D-151` as amended): art and sound on Opus 5.5 at max, code at xhigh. The brief's contract (IDs, sizes, anchors, frame counts, clips, timings) is what the code reads, so the accepted art touches no code; Step 1 makes every contract file from today's art so the code steps never wait for the owner's review.
- The code comes first because §3.5's review sheet needs real in-game loops of the new behaviour (Q1).
- Later stops expect: (n) adds burning sounds and the cue falloff on these cues; (o) places a goblin in the hollow; (p) adds the armoured troll beside today's.

## 1. Audit

Read at `433b499`. 01's paths are under `examples/01_platformer/`; `features/` is its `src/features/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The charge: `launch(±600)`, input locked 0.8 s, the `beetle_charge` ability arming its kit hitbox; the attack ends after 0.8 s | `features/enemies/beetle.rs:120-141` |
| A2 | Locked input keeps the controller's own `velocity.x`, and a wall it can't climb zeroes it, so a charge stops dead at a crate (12) or a keg (16) and stays stopped (K30). Walkers shove only up to mass 8, with a 32 px step | `crates/tungsten-kit/src/motion/platformer.rs:688-692`, `:774-794`; `features/enemies/mod.rs:86-120`; `tools/content/prefabs/{crate,keg}.json` |
| A3 | The troll's `blow` kicks a dynamic body `min(650, 2,400 / mass)` away and 0.6 of it up; `meets` tests a body against a box | `features/enemies/troll.rs:311-353`, `:393-397` |
| A4 | The charge's hitbox: 30 physical, window 0.8, knockback 520. The kit pushes a target away from the hitter's centre at the knockback's speed; a standing player's centre is about 15 px over a beetle's, so the push runs out and a little up | `tools/content/prefabs/beetle.json`; `crates/tungsten-kit/src/overlap/grid.rs:493-504`; `tests/game/data.rs:136-137` |
| A5 | The tell's look: a glint swelling at the horn and three flat streaks behind a charge; dust at the front foot every 0.15 s and in the trail every 0.06 s, both `ex10_charge_dust`; the paw clip 4 frames of 100 ms looping, the charge 4 of 60; `BeetlePaw` once at the tell's start | `features/enemies/draw.rs:99-165`; `beetle.rs:102-110`, `:169-211`; `tools/actors.py:1710-1717` |
| A6 | F1 outlines 01's own blows (a goblin's stab box, a troll's swipe box, its slam's reach) while `Brain::strike` lands in Attack; the beetle's shows as the kit's armed hitbox | `draw.rs:296-353`; `tests/game/presentation.rs:1007-1095` |
| A7 | The crawler's fear edge is the staff light's reach: `light()` gives the glow's centre and `reach()`, 1.4 × the halo's 125 = 175 px; `into_light` refuses a step that ends inside it and nearer | `features/enemies/crawler.rs:62-81`; `features/level/components.rs:126-130`; `tools/content/prefabs/player.json` (`glow`) |
| A8 | A refused step stands it facing the player and starts a scare: one `CrawlerScared` hiss, one `ex10_frost_breath` puff, its scared clip held 0.4 s (`Scared`, presentation). It never backs off, and nothing moves it out once inside. Its chase never lunges while the light burns | `crawler.rs:83-130`, `:160-190`; `features/enemies/components.rs:204-210` |
| A9 | Crawler numbers: run and `move_speed` 160, `jump_speed` 600 unused; its only jump is the lunge (640 across, 620 up, lock 0.45, range 300, rise 64); a 15 cold touch. Patrol and seek walk with no hop; its `blocked` copies `systems::blocked` | `tools/content/prefabs/crawler.json`; `crawler.rs:19-60`, `:132-158`, `:236-250`; `features/enemies/systems.rs:149-162` |
| A10 | The goblin's hop: `Ahead`, `ahead` (a body at its height that is no keg or bomb, or a wall no higher than 64 px, with room: hop; a drop under 96 px: step; else stop), `room` (its half hard-coded; head room, the way over, ground beyond), `hop` (720 up at its full run) and `scurry`. A deep ledge stops it: no gap leap | `features/enemies/goblin.rs:90-95`, `:187-308` |
| A11 | A load restores `Stats` bases whole; `to_14` moved the troll's `move_speed` base and the crawler's `damage` base; `stats_apply` writes `move_speed` into the controller's run; `data.rs` pins both restated values | `src/save.rs:497-538`; `tests/game/data.rs:103-120` |
| A12 | `Brain` keeps `strike`, `alerted` and `slam_cooldown`, each skipped when unset, unknown fields denied; `troll::cooldowns` (`troll_cooldowns`) counts the slam's down before the machine | `features/enemies/components.rs:75-111`; `troll.rs:626-635`; `features/enemies/mod.rs:303-310` |
| A13 | The goblin's throw: one `KitRng` draw at its tell picks a bomb under 0.2, else a rock lobbed by `props::lob_rock` on `troll::ballistic` (flight distance / 650 held to 0.35–0.9 s, from its hand at (50, 30)), credited through `Throws`. No cooldown: Recover (0.9 s) goes back to Chase, which winds up again at once between 140 and 420 px | `goblin.rs:324-392`, `:508-550`; `features/props/mod.rs:280-298`; `systems.rs:320-340` |
| A14 | `body_impacts` hurts a live target a moving body touches (`target_hits`, sparing the player from un-thrown bodies), a thrown rock landing its own blow, credited to the thrower while it lives; a keg that blows credits the thrower of what met it, else the player, never its own thrower (Q18 deferred the troll's keg throws); `blows_keg` needs 1,050 px/s against the heavy | `features/elements/systems.rs:1050-1101`, `:1121-1343` (`:1197-1208`, `:1255-1286`); `features/elements/mod.rs:204-212` |
| A15 | Saves: schema 16, arms 7–15 ending in `to_16`; `SaveGame`'s lists, `props` `#[serde(default)]`; `EnemySave` keeps the controller, machine, `Brain` and `Stats` whole. `bodies()` errors on a prop kind with no box in `S15_PROPS`, and the relabel tests feed current captures through it | `src/save.rs:79-128`, `:204-221`, `:241-347`, `:563-636` (`:616-618`), `:954-984` |
| A16 | The player's cold: threshold 30 + 0.2 × max health = 50, `chilled` at 0.15 of it (7.5 cold) in a 6 s window; only `freezable` targets freeze, and the player has `team.player` alone | `tools/content/prefabs/player.json`; `tools/content/data/damage_types/game.json` |
| A17 | The troll lifts only in Chase: the nearest liftable within 200 px (iron, a bowling ball, a crate burning or not, a live frozen enemy or dummy; never a keg), then throws it. Search and Return `stride`, which looks past bodies, so a crate or keg holds it pressed there. A keg's `fuse` is `Some` once lit. Scrap weighs 25, a bowling ball 8 | `troll.rs:127-165`, `:484-520`, `:581-594`; `features/props/components.rs:36-47`; `features/toys/mod.rs:62`, `:82`; `tools/content/prefabs/bowling_ball.json` |
| A18 | Burning: a fire rung at 0.25 of the threshold (a goblin's 15) grants `status.burning` for 4 s; the goblin reads it nowhere | `tools/content/data/damage_types/game.json`; `tools/content/data/effects/burning.json`; `features/combat/mod.rs:59`, `:243-245` |
| A19 | The kit's machine: a `go` to the current state enters it again; a transition sets `previous` and zeroes `elapsed`; a held machine is not visited | `crates/tungsten-kit/src/intent/state_machine.rs:176-197`, `:366-410` |
| A20 | `EnemyState`'s eight states and `Strike`'s six, saved by name; hooks dispatch by kind; `chasing` keeps alerts off a busy enemy; each kind's clip by state | `components.rs:100-143`; `systems.rs:31-48`, `:552-562`, `:728-746` |
| A21 | `Sound::ALL` holds 73 and `cue_of` maps them through `occurrence::<72>`; `play` sends `Sounded`; `data.rs` checks every variant has a rule and every rule's sound is in the manifest, not the reverse | `features/audio/components.rs:198`; `features/audio/mod.rs:82-158`, `:162-168`; `tools/content/data/cue_rules/sounds.json`; `tests/game/data.rs:190-212` |
| A22 | The generator: enemy clips are `actors.ENEMY_CLIPS` (frames, durations, looping), each given a `_left` twin by `mirror_facing`; particle configs in `particle_configs`; sounds by file in `sfx.build_sounds`, by ID in `tools/sounds.json`; icons drawn from the creatures' canvases; enemies unlit | `tools/actors.py:2594-2641`; `tools/generate.py:60-571`, `:579-594`; `tools/sfx.py:2338`; `tools/effects.py:568-591` |
| A23 | The crawler's art: `crawler_canvas` (five plate ellipses, eight legs, a wedge head, crystals) and five clips (walk 6 × 90 ms, wind-up 4 × 100, lunge 60/80/400, recover 100/120/140, scared 90/70/90/70) on a 64 px frame, its lowest drawn row `SOLE` 59 and the outline round it, so its twenty frames' lowest opaque row is 60, as every beetle frame's and the goblin's but its stab's first (62) and its hop's first (59), measured; the walk's columns run 0–63; `shade()` rims 1 px and fills flat (V19's flatness) | `tools/actors.py:26`, `:1438-1569`; `tools/pixels.py:73`, `:112` |
| A24 | Lightning meets an enemy within `KindInfo::strike` of its body's centre, 32 for the crawler (its walk's opaque columns); a test strikes 2 px inside it on each side | `features/enemies/mod.rs:122-193`; `tests/game/lightning.rs:421-454` |
| A25 | The goblin's clips: idle 4, walk 6, wind-ups 3 (rock, bomb), throw 3, stab wind-up 1, stab 2, hop 2 | `tools/actors.py:2467-2550`; `goblin.rs:570-593` |
| A26 | A clip plays at a point as a hidden sprite with its `AnimationState` (`toys::wave`'s parts); `spawn_burst` starts a particle burst | `features/toys/wave.rs:95-111`; `src/shared.rs:100` |
| A27 | Tests pinning what moves: the scare at the reach (`tests/game/enemies.rs:840-902`); lunges from 260 px, the light out (`stalker`, `:919-1069`), the freeze mid-tell (`:1070-1113`); the charge (`:1178-1216`); the goblin's seeded bombs, 25 throws in 3,200 frames (`:2657-2764`), its seeded rock (`:2863`); the troll's chase (`:2933-3025`); walkers on patrol leaving crates and kegs (`:3056-3083`) | `tests/game/enemies.rs` |
| A28 | Save tests: the coverage test's kinds and lists over `busy()`; a full pit under 2 MiB; a schema 15 save from the 0.108 code, a current capture without the new fields numbered 15 | `tests/game/saves.rs:728-783`, `:2714-2743`, `:3326-3343` |
| A29 | 01's README rows for the crawler, beetle, goblin and troll; `DESIGN.md`'s workspace line lists 01's features | 01's `README.md:105-109`; `DESIGN.md:5` |
| A30 | The generator owns `assets/` byte for byte (`generate.py --check`), new sprites shift `level.tmj`'s tile IDs, and it runs here only in a scratch Pillow venv; `just level-check` runs the check and `tools/test_*.py` | `justfile:96-98`; `tools/test_generate.py` |
| A31 | `iron_friction` slows iron, props and dummies on something in the overlap phase, after the physics step | `features/toys/systems.rs:838-915` |
| A32 | The solver's speculative contacts stop a fast body at touching, and a collision event needs positive penetration, so a body can meet a wall and send none | `crates/tungsten-core/src/physics/step/mod.rs:9-17`, `:876-892` |
| A33 | A launch sets the controller's velocity even under a lock, which keeps its `x`; `lock_input` only lengthens a lock, so nothing ends one early | `crates/tungsten-kit/src/motion/platformer.rs:568-574`, `:679-692` |
| A34 | The dummy is `freezable` with a 20 + 0.05 × max health threshold, so cold freezes it once chilled past half of it | `tools/content/prefabs/dummy.json`; `tools/content/data/damage_types/game.json` |
| A35 | A `SpatialFilter` leaves sensors out by default and excludes one entity, a second `excluding` replacing the first (K31) | `crates/tungsten-core/src/physics/query.rs:60-95` |

## 2. Design

### 2.1 Scope

All of it is 01's game Rust and data on today's kit: no kit, core or render change, so the kit hash and the physics hashes stand. Each per-step cost is a spatial `overlap` over a small box (one per charging beetle, per walking troll's way and per snowball, eight at most), so no capture is planned. New `Sound`, `Strike` and `EnemyState` variants land with their first use, so no step leaves a dead variant for `-D warnings`.

### 2.2 The beetle's charge (V17, V18; Q51, Q74)

- **Relaunch (K30).** `beetle::attack` relaunches the charge each step before `CHARGE_SECS`: `launch(heading × CHARGE_SPEED, v.y)`, `v` the body's `Velocity`, under the lock its start set, so a wall's zeroing (A2) lasts one step.
- **The plough.** Each step of the charge, `beetle::plough` takes the dynamic bodies, not held, that meet `PLOUGH_HALF` (12, 14) about `PLOUGH_AT` (36, −2) from its centre, mirrored (the 24 px before its face): each lighter than the beetle (under its mass, 120) whose speed along the heading is under its kick `min(PLOUGH_MOST, PLOUGH_KICK / mass)` takes that speed along it and `PLOUGH_RISE` of it up, its vertical speed no lower, and wakes, in save-ID order (`overlap`, then `troll::meets`). `PLOUGH_KICK` 12,000, `PLOUGH_MOST` 1,000, `PLOUGH_RISE` 0.3: a crate 1,000, a keg 750, scrap 480, a bowling ball 1,000. Iron (400) it never kicks, since friction after the step would let each step's kick top it up again (A31): a wall it presses until the charge ends. Under `KEG_SPEED` 1,050 the kick never sets a keg off itself; its landing does only past Q46's threshold (A14). Kinematic bodies (the player, enemies, the frozen) are the hitbox's.
- **The player.** `beetle.json`'s hitbox knockback 520 → 800; the push away from the hitter's centre gives about (0.94, −0.34) × 800 to a standing player (A4, Q8).
- **The tell and the charge's look.** The rearing pose, its glint and the stronger charge are the art's (§3.1): `draw::charges` drops its glint quad and draws `ex10_charge_streak_<f>` (`_<f>_left` facing left) behind the rear in place of the flat streaks, the frame by drawn time at 50 ms; `beetle::dust` puffs `ex10_paw_dust` at the front foot through the paw, `ex10_charge_dust` in the trail as now; the rising paw is `beetle_paw.wav`'s redraw.
- **Sound.** `Sound::BeetlePloughed` at the first body a step sets moving (its speed along the heading under half its kick), one a step.
- **F1.** `draw::debug_boxes` outlines each charging beetle's plough box in `HIT_COLOR`.

### 2.3 The crawler's fear and retreat (V20; Q52)

- `FEAR_SHARE` 1.3; `crawler::fear(world)` gives the lit light's centre and 1.3 × its reach (227.5 px for the staff's 175), the drawn light unchanged; `afraid(world, crawler)` holds while its centre lies inside it. `into_light` reads the fear edge.
- `crawler::retreat`: it faces the player and runs straight away from the light's centre at `RETREAT_PACE` 0.8 of its run; stopped behind by a wall or a ledge (`systems::blocked`), it stands, cornered, its touch hurting as ever. A retreat's first step plays `Sound::CrawlerRetreated` and a frost puff in place of the scare's hiss (`scare` gains a `retreating` flag, so no new component).
- Patrol, Chase, Search and Return retreat first while afraid; a wind-up while afraid spends its strike and goes back to Chase; a lunge or a throw in flight runs on. Afraid, it neither lunges nor throws (Q2).
- `crawler::clip(world, crawler, state)`: `crawler_retreat` while afraid and moving, `crawler_scared` while it holds at the edge or stands cornered.

### 2.4 The crawler's speed, hops and leaps (V21; Q53)

- `crawler.json`'s run and `move_speed` 160 → 210, restated as `crawler::MOVE_SPEED` for the migration and `data.rs`.
- `enemies/hop.rs` takes the goblin's `Ahead`, `ahead`, `room`, `hop` and `scurry` (A10), each now for any walker by its kind's half and a `Hops` table (`rise`, `clear`, `step_down`, `leap: Option<Leap>`), with `Ahead::Leap` and `body_ahead` (the first dynamic body in `ahead`'s probe). `goblin::HOPS` is today's (720, 64, 96, none), so every goblin test stands.
- `crawler::HOPS`: 720, 64, 96 and a leap of 420 across and 600 up, its input locked 0.35 s, which rises 50 px on the world's 3,600 px/s² (`features/level/mod.rs:52`). It leaps at a ledge whose far ground tops out between `LEAP_UP` 24 px above and 96 px below its feet and begins within the arc's reach at that height, less 16 px, with head room: 420 across while the lock lasts, then its run's speed, which the controller restores at the lock's end (A2; 210 at full run), over the flight to that height (about 140 px level, 120 at 24 up, 168 at 96 down); higher ground across a gap it stops at, since a leap could not land on it. It hops and leaps in Chase, Search and Return; its patrol keeps its ledge (Q4). `Sound::CrawlerHopped` at a hop or leap; `crawler_hop` while airborne outside its lunge. Its `blocked` copy goes.
- **Schema 17** (`to_17`): each saved crawler's `move_speed` base becomes `MOVE_SPEED`, its modifiers kept; arms 7–15 end in `to_16` then `to_17`, and `16 => to_17`. Steps 4 and 6 add only fields an older save lacks and reads as none (§2.5, §2.7), so no further migration.

### 2.5 The snowball (V22; Q54)

- **The cooldown.** `Brain::cooldown` (saved, skipped at 0, an older save's none): the seconds before a crawler's snowball or a goblin's throw or dash-in may come again. `systems::cooldowns` (`enemy_cooldowns`) replaces `troll::cooldowns` in its place, counting `slam_cooldown` and `cooldown` for every kind.
- **The chase**, not afraid: grounded, its cooldown spent, the player 140–360 px across and no more than 200 above or below: it faces the player, keeps `Strike::Snowball` and winds up; else the lunge as today (light out, within 300 across, its rises); else it closes in, hopping, the fear edge holding it. At the edge it throws, never lunges while the light burns (Q2).
- **The throw.** Its tell `SNOWBALL_TELL` 0.36 s (`crawler_snowball_windup`, silent); its attack's start throws from `SNOWBALL_FROM` (66, 40) off its sprite's top-left, mirrored, just ahead of its mouth and clear of its body, at the player's centre on the world's gravity, the flight distance / 650 held to 0.3–0.7 s (`troll::ballistic`), sets `cooldown` 4.0 and plays `SnowballThrown`; the attack runs `SNOWBALL_SECS` 0.25 (`crawler_snowball_throw`), then Recover. The lunge keeps no strike, as now (Q20).
- **The snowball** (`enemies/snowball.rs`): the `snowball` prefab (sprite `ex10_snowball_0`, clip `ex10_snowball`, a dynamic circle of radius 5 at (8, 8), mass 1, restitution 0), with `Snowball { thrower: Option<u64>, factor, left }` inserted at the throw, so the prefab names no 01 component; `SNOWBALL_CAP` 8 live, the oldest bursting. `snowballs`, in the overlap phase after the physics step, each in save-ID order: the first live target off `team.enemy` its circle touches, within the impacts' contact margin as `target_hits` tests (A14), takes `SNOWBALL_COLD` 12 cold × its factor, from its thrower while it lives, and it bursts there; else its circle grown by `SNOWBALL_MARGIN` 2 px meeting a solid tile or a body bursts it, since contact events miss a body stopped at touching (A32): an `overlap` whose filter excludes the snowball itself (sensors are left out by default), its thrower passed over in the results, as a filter excludes one entity (A35); else it bursts when `left` (3 s) runs out. A burst: the `ex10_snowball_burst` clip at its centre (A26), its particles, `SnowballBurst`, its despawn. Its cold is any cold: 12 chills the player at every difficulty (9 at Easy, against 7.5; A16), who never freezes, not being `freezable`, and builds a chilled dummy toward its freeze as other cold does (A34).
- **Saved** in `snowballs` (`SnowballSave`: id, position, velocity, thrower, factor, left; `#[serde(default, skip_serializing_if = "Vec::is_empty")]`), not `props`, which `bodies()` reads (A15, Q6).

### 2.6 The troll (V25; Q55)

- `liftable` adds a keg whose fuse is unlit (A17); a lit one it kicks.
- `troll::clear_way` runs before its stride in Chase and in `seek` (Search, Return): holding nothing, it lifts a liftable body ahead (`hop::body_ahead`); a body it may not lift, heavier than `WALK_SHOVE_MOST`, it kicks along its heading by `blow`'s kick, with the swipe's sound. Holding, it throws in Chase as today and walks on in Search and Return; `patrol` sets what it holds down first (`drop_held`), a fall under Q46's.
- `body_impacts` credits a keg that blows to its own throw's thrower, then to the thrower of what met it, then the player (A14).

### 2.7 The goblins (V26; Q56)

- **The cooldown.** `goblin::COOLDOWN` 2.5 into `Brain::cooldown` at a throw's release and at a dash-in's end. The chase keeps today's order (the stab within 56 across and 40 up or down, flight, backing off, closing in); in its band it rests facing the player while cooling, and with the cooldown spent and the player within 200 up or down an approach starts.
- **The approach (Q9).** One `KitRng` draw: under `DASH_CHANCE` 1/3, `Strike::Dash`; else a throw, whose tell draws its bomb as today.
- **The dash-in (Q10).** Its tell `DASH_TELL` 0.3 s, a crouch (`goblin_dash_windup`, `GoblinCrouched`); its attack launches `heading × DASH_SPEED` 560 under a `DASH_MOST` 0.6 s lock (`GoblinDashed`, `ex10_dash_dust` at its feet); each step, a courier within `STAB_REACH` across and `STAB_RISE` up or down makes it keep `Strike::Stab` and `go(Attack)`, which enters the attack again (A19): today's stab, clip and leap back. A dash that reaches no one ends at `DASH_MOST` in Recover. A body stops it dead (K30; no relaunch). Clip `goblin_dash`.
- **Panic (Q11).** `EnemyState::Panic`. `goblin_panic`, in the intent phase after `enemy_hold` and before the machine: a goblin whose machine is not held, burning, neither in nor bound for Panic, spends its strike and goes to Panic. Its enter yelps (`GoblinYelped`) and launches it away from the player at its run speed with a startled skip (`PANIC_HOP` 300 up), which a dash's lock cannot hold off and its `x` then keeps for the lock's rest (A33); its update runs from the player (from its heading with none) at its full run by `hop::scurry`, the other way once that stops, else stands flailing; once not burning it chases on sight, else searches. `chasing` counts Panic; its clip is `goblin_panic`, in the air too.
- **Saved:** `Brain::cooldown`, `Strike::Dash` and `EnemyState::Panic`, by name; an older save holds none.

### 2.8 Sounds and cues

Each variant appended to `Sound::ALL` at its step with its event, the next `occurrence::<N>` in `cue_of` and a rule naming §3.1's ID (A21); Step 1's placeholders put each sound in the manifest first.

| Variant | Event | Sound | Step |
| --- | --- | --- | --- |
| `BeetlePloughed` | `beetle_ploughed` | `ex10_beetle_plough_sfx` | 2 |
| `CrawlerRetreated`, `CrawlerHopped` | `crawler_retreated`, `crawler_hopped` | `ex10_crawler_retreat_sfx`, `ex10_crawler_hop_sfx` | 3 |
| `SnowballThrown`, `SnowballBurst` | `snowball_thrown`, `snowball_burst` | `ex10_snowball_throw_sfx`, `ex10_snowball_burst_sfx` | 4 |
| `GoblinCrouched`, `GoblinDashed`, `GoblinYelped` | `goblin_crouched`, `goblin_dashed`, `goblin_yelped` | `ex10_goblin_crouch_sfx`, `ex10_goblin_dash_sfx`, `ex10_goblin_yelp_sfx` | 6 |

`ex10_beetle_paw_sfx` keeps its ID and its cue, redrawn (§3.1). The troll's kick reuses `TrollSwiped`.

## 3. Asset brief

The redo's [§6](roadmap-redo.md#6-asset-sessions) brief for this stop's families. Step 1 makes every file below from today's art; Step 7 draws each in its place with the generator's own toolkit, within this contract, and touches no `src/` or `tests/` file.

### 3.1 Families and contract

Generator names; the manifest prefixes `ex10_`; "twins" are `mirror_facing`'s `_left` copies (A22). Enemy frames are 64 × 64 and unlit, on today's anchors: the crawler's body centre (32, 51) and half (24, 10), the beetle's (32, 48) and (24, 13), the goblin's (34, 41) and (9, 20), the lowest drawn row `SOLE` 59 with the outline on row 60, the lowest opaque row of every grounded frame (the hop's may rise; A23). Particles draw at z 0, under the sprites, so bursts launch out past the body.

| Family | Files | Frames, timing | Anchor and contract | The code's use |
| --- | --- | --- | --- | --- |
| Crawler, redrawn | `crawler_walk_0`–`_5`, `crawler_windup_0`–`_3`, `crawler_lunge_0`–`_2`, `crawler_recover_0`–`_2`, `crawler_scared_0`–`_3`, twins; `icon_crawler` follows | Today's: 6 × 90 ms looping; 4 × 100; 60, 80, 400; 100, 120, 140; 90, 70, 90, 70 looping | The walk's opaque columns within 30–32 px of x 32 on each side (`KindInfo::strike` 32, A24) | Today's clips by name |
| Crawler, new | `crawler_retreat_0`–`_5`; `crawler_hop_0`–`_2`; `crawler_snowball_windup_0`–`_3`; `crawler_snowball_throw_0`–`_2`; twins | 6 × 80 ms looping; 70, 90, 240 once; 4 × 90 (0.36 s) once; 50, 80, 120 (0.25 s) once | The snowball leaves just ahead of its mouth at (66, 40) on the throw's frame 0, facing right | §2.3–§2.5's clips |
| Snowball | `snowball_0`–`_3`, clip `snowball`; `snowball_burst_0`–`_5`, clip `snowball_burst`; particles `snowball_burst` | 16 × 16, 4 × 60 ms looping; 48 × 48, 6 × 50 ms once; a burst of at most 24 alive | The ball's centre (8, 8), drawn within radius 6 (its body's 5); the burst's centre (24, 24) on the contact | §2.5 |
| Beetle | `beetle_paw_0`–`_7` (from 4), `beetle_charge_0`–`_3`, twins; `charge_streak_0`–`_3`, clip `charge_streak`, twins; particles `paw_dust` | 8 × 100 ms (0.8 s, `PAW_SECS`) once; 4 × 60 looping; 64 × 24, 4 × 50 ms looping; bursts of at most 12 | The paw's glint at its horn's tip, swelling over the eight frames; the streak's right edge's middle (64, 12) on the rear point, centre − heading × (half.x + 4); the paw dust from the front foot, centre + (heading × 14, 9) | §2.2 |
| Goblin | `goblin_dash_windup_0`–`_1`, `goblin_dash_0`–`_3`, `goblin_panic_0`–`_5`, twins; particles `dash_dust` | 120, 180 ms (0.3 s) once; 4 × 70 looping; 6 × 70 looping; a burst of at most 10 | The dash's knife inside the stab box, (24, −4) ± (20, 16) from its body's centre facing right | §2.7 |
| Sounds | `ex10_<id>_sfx` at `sounds/<id>.wav`, IDs `beetle_paw` (redrawn), `beetle_plough`, `crawler_retreat`, `crawler_hop`, `snowball_throw`, `snowball_burst`, `goblin_crouch`, `goblin_dash`, `goblin_yelp` | The paw 0.75–0.85 s, its tell's 0.8; the yelp 0.4–0.8 s; the retreat 0.4–0.7 s; the burst at most 0.45 s; the others at most 0.35 s; volumes 0.4–0.75 | — | §2.8 |

### 3.2 Art direction

From the owner's reviews since M75: detailed, characterful passes in the house style (palette pixels, outlines, moonlit top-left edges), anatomy and material texture that read at 2×, effects as animated sprites with particles as accents, motion shown as real in-game loops; drawn with the generator's toolkit (`Part`, `shade`, `compose`, `outline`), never pixel by pixel.

- **The crawler, redrawn (V19, Q73).** Today's flatness is `shade()`'s 1 px rim over a flat fill (A23). Graded shading across each plate (lit top-left to shadowed underside, a few steps, dithered), a bright highlight line along each plate's ridge, darker seams between plates with the cold glow showing through, faceted crystals that catch a glint (one or two bright pixels a frame, shifting through the walk), jointed legs with lit upper segments; alien and many-legged as now, unlit. A graded-shading helper may go in `pixels.py` if nothing else calls it, so no other sprite changes (`--check`).
- **Its retreat.** Backing away from what it faces: legs stepping back in sets, body recoiled low at the front, plates raised and feelers laid flat; it reads as fear, not a walk played backwards.
- **Its hop.** A spring from bunched legs, aloft with legs tucked and feelers swept back, a tucked hold for the landing.
- **Its snowball.** A clump of packed snow and ice packed between its mandibles through the tell, flicked out with a jerk of the head; the ball a lumpy white-blue sphere with ice grains, spinning; the burst a puff of powder and chips that settles, flakes as accents. Frosty and white, as M84's frost (subtle, no dark rims).
- **The beetle's wind-up (V18, Q51).** Rearing: its front and horn rising, a foreleg raking the ground, the shell tilting, a glint swelling at the horn's tip; scraping dust kicked back from the foot in clumps. Its charge stronger: head down, horn levelled, legs a blur; the streaks bold speed lines tapering behind it, two tones, flickering.
- **The goblin's dash-in and panic (V26).** The crouch low and coiled, the knife drawn back, a sly look; the dash a low sprint, the knife thrust ahead; the dash dust a scuff from its heels. The panic: running flat out, arms flailing over its head, mouth wide, eyes wild (the fire overlay is combat's, drawn over it).
- **Sounds** (redo Q5: the same loop, a playlist with notes). The paw a raking scrape that rises in pitch and level into the charge. The plough a heavy wooden thud. The crawler's retreat a long backing hiss over chittering legs, apart from its scare's hiss; its hop a chitter and a click. The snowball's throw a packed crunch and a flick; its burst a soft crump with ice tinkles. The goblin's crouch a sly snicker; its dash a scuffle and a whoosh; its yelp a high squeal into panicked gabble, apart from its cackle.

### 3.3 Values the code takes

None from the art: the contract fixes the anchors, sizes, counts and timings the code reads. Step 7 measures and records in its evidence row the walk's opaque reach about x 32 (each side 30–32), the throw's release pixel against (66, 40) and each clip's total against its state's seconds.

### 3.4 Placeholders (Step 1)

Every §3.1 file comes from today's art at the contract's size, count and timing, by copying, cycling or resizing frames, configs and synthesized sounds, never drawn: the new crawler clips from its walk, lunge, wind-up and recover frames; the snowball from the goblin's rock frames, its burst from `frost_puff`'s first six; the paw's eight from today's four cycled, the streak from a resized trail frame, the goblin's dash wind-up from its stab wind-up and its dash and panic from its walk (its stab's first frame ends on row 62, so it is no source; A23); `paw_dust` and `dash_dust` copies of `charge_dust`, `snowball_burst` a copy of `ice_shatter` capped at 24; each sound one of today's within its range, measured (the retreat the crawler's tell hiss, 0.5 s; the hop, the throw and the plough the rock's throw, 0.25; the burst the lunge's, 0.36; the crouch and the dash the stab's, 0.22; the yelp the cackle, 0.6; the paw the charge's, 0.8, in place of today's 0.42). `tools/test_generate.py` gains a contract test that reads the generated `assets/`: every §3.1 sprite at its size, every clip at its frames, durations and looping, the twins, each particle config's cap, each sound's length in range, row 60 as the lowest opaque row of every grounded enemy frame §3.1 names (the hops exempt) and the crawler's walk reaching 30–32 px each side of x 32. Today's crawler and beetle frames pass it (A23), the placeholders from these sources too, and so must the art.

### 3.5 The review sheet

In `perf-runs/<date>-e01foes-art-review/` (machine-local), linked in the conversation at each round, the contact sheet's link first, as the owner asked on 2026-10-26:

- A contact sheet at 4×: every §3.1 frame, both facings, today's crawler beside the redrawn one, the snowball and its burst, the streaks.
- Animated previews of each clip.
- In-game loops from a scratch driver under `target/` (deleted after; the in-game shots recipe, one capture job at a time, the shell's pause off in the copy): the crawler backing out of the lit wizard and holding at the edge, hopping up a step and leaping a one-tile gap, throwing a snowball that bursts on the wizard and on the ground; the beetle rearing with its glint and dust, then charging through a crate and a keg with its streaks; the goblin's crouch, dash-in and stab, and its panic while it burns.
- A playlist of the nine sounds with notes, beside `crawler_scared.wav`, `beetle_charge.wav`, `goblin_stab.wav` and `goblin_cackle.wav`.

### 3.6 Lock

On acceptance, Step 7 writes one `sha256sum` line per file §3.1's families own (sprites with their twins, clips, particle configs, sounds, `icon_crawler`), paths from the repository root, between these fences. The check, `sed -n '/^```sha256$/,/^```$/p' docs/plans/e01-foes.md | sed '1d;$d' | sha256sum -c --quiet`, exits 0 only when every file is unchanged, and 1 while the block is empty.

```sha256
dfedede8fd35e7c5a8531d5092f7e603a926e0d9374be47a1a9ee9ca89732950  examples/01_platformer/assets/animations/beetle_charge.json
279fbf55c40b07c00f1ccc7a38c7defec5c899c32807abef8132920d2163daf6  examples/01_platformer/assets/animations/beetle_charge_left.json
f049a98d2c1f9bf226adf5b4ec8c3b670e0aed8c22a7514d573703988cf256b5  examples/01_platformer/assets/animations/beetle_paw.json
1226904f742c5a44984cc2fcb027e42788be21d971848e6b697d522b2a51d7b0  examples/01_platformer/assets/animations/beetle_paw_left.json
ad6b9f8c6616c907b6f14c445de80c848ba4d57d26931ca01576be9cd5416689  examples/01_platformer/assets/animations/charge_streak.json
cdc7755b1878d83de78260b6bccd26b4f9802f02ed69fa52030b1c2f6a10b34d  examples/01_platformer/assets/animations/charge_streak_left.json
fd9f6eb83fe20e9ba4e27365601c66fef957f6fa7398800a83b9659d2c6ed40a  examples/01_platformer/assets/animations/crawler_hop.json
95ce40d14c540a4a0906633c7ffcf1866d6be67d4b5d37e9f2031dbe4228c096  examples/01_platformer/assets/animations/crawler_hop_left.json
b7fb24ee439b10e4a0901ffbfb5910561d08e1fc435f480df05d350ea22fcbf0  examples/01_platformer/assets/animations/crawler_lunge.json
baa35552141546f8eeffd03243bc5a7137d7f724a6b6857eaa1b1a8cc7d2b0bf  examples/01_platformer/assets/animations/crawler_lunge_left.json
9a6509deec361561a17807c3d0300c38e089ed16fd069056b2b71040b2f73886  examples/01_platformer/assets/animations/crawler_recover.json
c695b74798885cfa7d178db5234924656cee6175bbe6f4921a0710350ff39deb  examples/01_platformer/assets/animations/crawler_recover_left.json
be8d66fece2c2e578c62c518c2adb5c228d95e102fe587a4de4251bfd1ddef94  examples/01_platformer/assets/animations/crawler_retreat.json
60f781f78d6c8ee4a40bb2de1b1db3420a883aeca64ece740d5c7ccb45b0eaf4  examples/01_platformer/assets/animations/crawler_retreat_left.json
bf93c5540cb29f7fa6ab4ee2340c05f2c511b2e5dcd82ea12529d705cec763dd  examples/01_platformer/assets/animations/crawler_scared.json
2f739de2a6afb473f19ec262e102cefa784a9ece56f02cd494e7f825cd4b4eb7  examples/01_platformer/assets/animations/crawler_scared_left.json
9dc7213516ca2e04d6c0522049942fab095045a1e2026fc48ed8aa1af37ac3e3  examples/01_platformer/assets/animations/crawler_snowball_throw.json
4d80ea8058a4eaee9c9523d9b01f457564b8ff8b666e6abbb312f1a1cd4f3bc2  examples/01_platformer/assets/animations/crawler_snowball_throw_left.json
9c97c1fb75b769f16db90db84afc3892910d72a38a1ae2233ba0f8cf308ba9cb  examples/01_platformer/assets/animations/crawler_snowball_windup.json
74962b35f94d67eaf0fba93c9312f9d7419027cfd1b6da4f4f28d9ecbb2a1eab  examples/01_platformer/assets/animations/crawler_snowball_windup_left.json
60d4df19314003a66fc71942382d1f5b5747b5e450ab3a0fdaf9d00f961ccc7d  examples/01_platformer/assets/animations/crawler_walk.json
897e790997a644369bff77b1f5a62bd5bab590291a1523bcfaa978944f21aa03  examples/01_platformer/assets/animations/crawler_walk_left.json
44595e75cceb2b8bb043dda8c6f0a89805e57a310c734e19e4779619d1abc8e5  examples/01_platformer/assets/animations/crawler_windup.json
fb5f263d340bc365775a68284d73bd53b622130018e9789409165096714c256e  examples/01_platformer/assets/animations/crawler_windup_left.json
202efb660637c765ccd8ad8cb4badb025e77b130ad3de2a2dfdc824aca9b3a9d  examples/01_platformer/assets/animations/goblin_dash.json
dfae8bf872398abb9600847a0658a6d618d788f96bb745dbd90829846917f29a  examples/01_platformer/assets/animations/goblin_dash_left.json
d58a8a2b9bd0edb32513b9fe2e95b253a38ccb2c28b117cb5e7a296f0320fa30  examples/01_platformer/assets/animations/goblin_dash_windup.json
d65eb77bc2cf4a5b4f3c3ccc300846949e32609c1112581cc1f00d8ce4f0f5cb  examples/01_platformer/assets/animations/goblin_dash_windup_left.json
2c50c2156712d8cc1f162dac775ed324c5111ab42964b8a32b18acc912881bfc  examples/01_platformer/assets/animations/goblin_panic.json
149984bcb4f73ca20d902ad089262b0c9197403312f4254fb5f73fdada3c73c9  examples/01_platformer/assets/animations/goblin_panic_left.json
c8c33035eb6bff98f534fe2ea3862d5468f32246e1c535b1adba8d2b4fbe749f  examples/01_platformer/assets/animations/snowball.json
0ae7f9f492b9eb24b16225df3f71bb9513bb0bb7fa4ca0ab1bcff474c7234d17  examples/01_platformer/assets/animations/snowball_burst.json
95326484fa272a8c83afd975dbc32db0bcf605b4175aa4933c0b6fa960563fbd  examples/01_platformer/assets/particles/dash_dust.json
a282cc3f394ccb867f73a851474ee34fbb62ca43f242022eda652ae03759c313  examples/01_platformer/assets/particles/paw_dust.json
f35fb91b88d9b51886a8de906006ad73052bab5dd408f532e40ca3281d3efcc8  examples/01_platformer/assets/particles/snowball_burst.json
f83ff33626b4893f5678fd64ed6d2607bcd025f39fdf6e9cac40b605a03c0bfc  examples/01_platformer/assets/sounds/beetle_paw.wav
8e830e9cd46cf8f81f4ec05e3057e949c6c0c1289c8cf0b1a457af2a22a91100  examples/01_platformer/assets/sounds/beetle_plough.wav
7bf323cdceeaf50fc8540408a76cc4c7c6b16f8c4ff596c9b6a2b1d021e9b628  examples/01_platformer/assets/sounds/crawler_hop.wav
10f83ec9fc642cbede15b522d140c3c78d58b917fd28c387d315d5cfd1c83810  examples/01_platformer/assets/sounds/crawler_retreat.wav
aad632253faa4f58273ac654d3c54b10604c5f04be8395385859cf163c209e1e  examples/01_platformer/assets/sounds/goblin_crouch.wav
8916bc50271ff1bdba0fab359cb8bf8505f9ae9967965700f069f1dfccb820a2  examples/01_platformer/assets/sounds/goblin_dash.wav
98ec9651e9e8af5a501af07d7d533342e31c19852b5dd4a8955868423ca53247  examples/01_platformer/assets/sounds/goblin_yelp.wav
3662f3e54f7f10c18e9075eeef9c21c9747ef1086d8757a8013b259ff870183d  examples/01_platformer/assets/sounds/snowball_burst.wav
ec4009d847a28784c84ba776e4bd37e42d5e1149816bb108bbfbbdb3317fef48  examples/01_platformer/assets/sounds/snowball_throw.wav
9fa7eefa6fb14438ac1c6c2c6f4bdf66776521d0beb81dca9ef53b612218f415  examples/01_platformer/assets/sprites/beetle_charge_0.png
5eaa5430cc0fdf2d9bfeb1975195dea2a5c2b60c775d3556596f6626f00d7f49  examples/01_platformer/assets/sprites/beetle_charge_0_left.png
8db48e400ab0e20e868728ffbb6f3fe54bb50b422b5c87a2527e81d863f560d0  examples/01_platformer/assets/sprites/beetle_charge_1.png
204de79aca45b4589410b25316c400cf8134c86f2b1ed761755dfab15e01d487  examples/01_platformer/assets/sprites/beetle_charge_1_left.png
5144d383db57351215546a1e2e9e8d84d33e8f2472668a8e0b58eb5e8867f421  examples/01_platformer/assets/sprites/beetle_charge_2.png
ca0cfffc819d03ffa7ec0fd90666ccd0ef7132497c5dc41b126185c96a117941  examples/01_platformer/assets/sprites/beetle_charge_2_left.png
99b87a9a53eab45d876f96814fc42079b63c395f0d32b77e65f82bb891d79d1a  examples/01_platformer/assets/sprites/beetle_charge_3.png
e70c6506efae8b34bad30f148606b7dede212cda819d41c890dbfb67a288ec91  examples/01_platformer/assets/sprites/beetle_charge_3_left.png
9d932abfec31f3f6d2e5b3a310e88b17edf0cb84cf1089f27278517834569c5a  examples/01_platformer/assets/sprites/beetle_paw_0.png
92b2cbbd5a489c2d73f93d185032764e98307d613e32104563903a4ad0694d11  examples/01_platformer/assets/sprites/beetle_paw_0_left.png
ee86d91fbaa7d426f6f40eb9f06e9cf29d527958ee77d77f6f4ddf21bfd97b4a  examples/01_platformer/assets/sprites/beetle_paw_1.png
012057564e649c5e40690c76d8ad71c033ae91dc2076bfa9d8c60d77391eb061  examples/01_platformer/assets/sprites/beetle_paw_1_left.png
c35cb553fe3cb1d0e74fab5304ab553383a2e75698174a829074d4d3522bd75f  examples/01_platformer/assets/sprites/beetle_paw_2.png
f316c7587b695b0f330eace4523ad91b0b283748223c86f8997e6c8d74f9deb3  examples/01_platformer/assets/sprites/beetle_paw_2_left.png
5953c0049209d1edc179f55863dd413e6aee03b6ba940a4ceafdf24f7c663e4a  examples/01_platformer/assets/sprites/beetle_paw_3.png
5efa97ab13bc359be76d1b3608383cbd0464ce51849a96bb062b4136c8f50882  examples/01_platformer/assets/sprites/beetle_paw_3_left.png
9c0ab1f8e283b9eb9abed5203bc192db04efd507c65839d294cff0740e77c669  examples/01_platformer/assets/sprites/beetle_paw_4.png
fe8910c3e4ff3c77440ad2ac56ff33f309a5ca7ae670f9e4b621db8f8e1d6364  examples/01_platformer/assets/sprites/beetle_paw_4_left.png
19ef052a54ae01bed2419da5e96c9a55ea22e4febde7bab4ca2f13b77297613b  examples/01_platformer/assets/sprites/beetle_paw_5.png
824d298305cca7b8792ffd470454f03c7eaf53af2c9956fe049c6105fa071996  examples/01_platformer/assets/sprites/beetle_paw_5_left.png
bf07b9449c32f34f0a81655c0cc4d3939e4d6f1cba4bf4fd340d40cc71238c76  examples/01_platformer/assets/sprites/beetle_paw_6.png
48c8f0e5c42c1b69940c3e10b152a7880f5f5ed5dbf6960ef74d566f7818c448  examples/01_platformer/assets/sprites/beetle_paw_6_left.png
112908fb6c4432718cf51b402b971007430836b769a8895c68aff771689759f1  examples/01_platformer/assets/sprites/beetle_paw_7.png
3bd4e9ec47d394e60a389f32167fb1fe6b60e16f2595ff2a3980268aff2ce004  examples/01_platformer/assets/sprites/beetle_paw_7_left.png
873874dae08ba941f27beb362fe3f10ffe7d45fa3f052db608e9eabc5feae2cf  examples/01_platformer/assets/sprites/charge_streak_0.png
cacac60005376a882d6b16221f35dc743b48dfdda9a7ff156f3c0e265a37f5cb  examples/01_platformer/assets/sprites/charge_streak_0_left.png
3cacb400987d8afcf9f37b5d87448ec5dc45c80241fff4c7c69523b864b3ba75  examples/01_platformer/assets/sprites/charge_streak_1.png
0502d75ce42e148867c3e426ff8d2bfbfd9ccc44b318a89a1cfc09b602c19b41  examples/01_platformer/assets/sprites/charge_streak_1_left.png
05d0430d7453521039838da2a72e5e25daec88526c892eca17c530d5ab57d4a4  examples/01_platformer/assets/sprites/charge_streak_2.png
f1b25e16e44d44cb70d6bda326738cab3d57d6a51fb5f4275ab0016016cbd4c5  examples/01_platformer/assets/sprites/charge_streak_2_left.png
38b0fd8499d01b6494f29079dfc17e85ee8f217b614f0e87b776dd7248d3235f  examples/01_platformer/assets/sprites/charge_streak_3.png
9057391352a86144bd88fcff31ad1498b71136852a4345d42c46be2fe414ee80  examples/01_platformer/assets/sprites/charge_streak_3_left.png
9f6f53adf93f4a53f965fdcb96178f1321ee73d6963a326f0153480734a5ae61  examples/01_platformer/assets/sprites/crawler_hop_0.png
09a32c1d854df3d8edb95ab1988e474c5b53a0ab7e1a18ac1b0f89160e9478d1  examples/01_platformer/assets/sprites/crawler_hop_0_left.png
488b60068562aff5cf06fbdcff7558ae1cf9021b29f402e847610798ad774bea  examples/01_platformer/assets/sprites/crawler_hop_1.png
b9a0c96a81dba346cf45044ec007f7d7df91628614f65d3dac7139b431f4a4d7  examples/01_platformer/assets/sprites/crawler_hop_1_left.png
cd5e7a2152b091e98868d90bd633a932535e9aa2a70d1290157c3008e387a4b0  examples/01_platformer/assets/sprites/crawler_hop_2.png
9535444e5e53acb21e2bf1a8e0fa90a2ff71de72bdda9c94e0709aac1c5453e6  examples/01_platformer/assets/sprites/crawler_hop_2_left.png
483f5e4a6c8545492332ceedfd1a25b9b4ccaf94b43c949612431605b434a3bd  examples/01_platformer/assets/sprites/crawler_lunge_0.png
6a9f7ead2e031e4ee6749ee25be46d66fdc0dba369099648fb4813420cb5f103  examples/01_platformer/assets/sprites/crawler_lunge_0_left.png
a00905f221b55de2fd961737046aca5ad946414900852ed503b69945791a5e61  examples/01_platformer/assets/sprites/crawler_lunge_1.png
96fa837d30cb0a4e8ea0f6769033a59fbe9d9962f7ed957bcf2729066a4bb0b3  examples/01_platformer/assets/sprites/crawler_lunge_1_left.png
a00905f221b55de2fd961737046aca5ad946414900852ed503b69945791a5e61  examples/01_platformer/assets/sprites/crawler_lunge_2.png
96fa837d30cb0a4e8ea0f6769033a59fbe9d9962f7ed957bcf2729066a4bb0b3  examples/01_platformer/assets/sprites/crawler_lunge_2_left.png
806540bce184921e2fde71cd4d65211198b3cc7f84ac51ed213bb205891623b1  examples/01_platformer/assets/sprites/crawler_recover_0.png
e89376d18614d4f38c00a696570a32b3a8d55841608996bf91dd0b32e5313b8a  examples/01_platformer/assets/sprites/crawler_recover_0_left.png
59f924584792dac01d9b64fee719b8e586d5ec788c505f561ad9e29aa3fde835  examples/01_platformer/assets/sprites/crawler_recover_1.png
b41f0ed2412d2a62055c22769fcc5be78d873cf3850b110da7584e54edea3319  examples/01_platformer/assets/sprites/crawler_recover_1_left.png
063f3cf282698d8b4b1c388f0f5b3e987520069aed1acde121d574e3923bbfe9  examples/01_platformer/assets/sprites/crawler_recover_2.png
cad6c5943fcb40f77108ef61ae5e31fbc74498ce21d710f882906ea2d984e4ed  examples/01_platformer/assets/sprites/crawler_recover_2_left.png
9758b254bfb8f468485bbd3dea5af9855ada1835271eac03492a8e786104c499  examples/01_platformer/assets/sprites/crawler_retreat_0.png
5d5ac53bcede88749a586c2d1f4d26da1795a135b07757b1c025cf6f0e4e2a33  examples/01_platformer/assets/sprites/crawler_retreat_0_left.png
e8974892672c766d6ed459b97283a98d3f3b3e56406dfa044a415e508e77ad2b  examples/01_platformer/assets/sprites/crawler_retreat_1.png
95948dc5ccc9837b9ceb9e9eab67c9ef12a069c7c0f14f5618d4055a305c6a16  examples/01_platformer/assets/sprites/crawler_retreat_1_left.png
5025d9b71238c818d59a99be00633e0335f16d647045946785b47fc284ad151c  examples/01_platformer/assets/sprites/crawler_retreat_2.png
10ae7124a7b902e40740c3ccb8ba5c87a85bbe2f1bd886548422bfbc3462a11f  examples/01_platformer/assets/sprites/crawler_retreat_2_left.png
35869b3ad740f236017ce01b7308a781353d59ce5ff2f3f4b6f3b959c6a9a190  examples/01_platformer/assets/sprites/crawler_retreat_3.png
b69358e5df7a6dbc10d5473e6036ad50ad9d2f3cf68cd3384f0cae71a35cd27e  examples/01_platformer/assets/sprites/crawler_retreat_3_left.png
256026fc2761eb3f469c24301b612ad0915fe8e350cd5867b3ad15aa51740e25  examples/01_platformer/assets/sprites/crawler_retreat_4.png
919d96200a06832a944c5db0f89a2986a910672e5c84e08a79a73db78d893017  examples/01_platformer/assets/sprites/crawler_retreat_4_left.png
a9b92263b849209f8ea923a8514fa78d6eb652831805c5bd16977c9592f774dc  examples/01_platformer/assets/sprites/crawler_retreat_5.png
5664456c2029066391b36ba73caad823a22765a91a34cffcd0c2408c21bf8615  examples/01_platformer/assets/sprites/crawler_retreat_5_left.png
2b486ae1e685517c15366c1209dd5b8467649b67b136cf77eeb6f958d5ecf06d  examples/01_platformer/assets/sprites/crawler_scared_0.png
1e8876c8ed6290cf9c6d69bdb787e59042267875156ac16c330dd4dced1f8d59  examples/01_platformer/assets/sprites/crawler_scared_0_left.png
0c6d8074bc612cc7404eafeb4c11db1096a5ea9cb961ac7763b846c6cbc97c73  examples/01_platformer/assets/sprites/crawler_scared_1.png
43c284b2e0f7410efd9c6b20eddaacdcff2e90456e795bbd10e7c309abd73b1d  examples/01_platformer/assets/sprites/crawler_scared_1_left.png
d3f7df5eed3f2e19fac9849d28f2c59499f95dab060335f89da20f89888edad2  examples/01_platformer/assets/sprites/crawler_scared_2.png
47b89bdd4c9ab12f83fa464f84dc7cba87f73bfa350e72d9215decd4936c23c6  examples/01_platformer/assets/sprites/crawler_scared_2_left.png
a6a91e9fd3647cc18f94671474ef30233f2fb27889b63778c318b8486bf6ea99  examples/01_platformer/assets/sprites/crawler_scared_3.png
f4d998cca358244e6f44f1fc452c686ee09065197cfd6febb89295922e05a5c8  examples/01_platformer/assets/sprites/crawler_scared_3_left.png
1ed71c9642f06c8c1fa2db34025c35a3f801a3dd2a3c1b76a79ac25392cf7e2f  examples/01_platformer/assets/sprites/crawler_snowball_throw_0.png
37b7dfaa7d9b5b8023e1628b1ef998bf2a940292ccf1124b85fa35057413bb4c  examples/01_platformer/assets/sprites/crawler_snowball_throw_0_left.png
ec999a9e6c8b77a138ed0fda18e5354b553d885f43cef8ab09c6a5efb90cfa39  examples/01_platformer/assets/sprites/crawler_snowball_throw_1.png
30579a9864b62a14af31f0512857c32c9a4d373d6bfd19869f5e6b6d37bf81e1  examples/01_platformer/assets/sprites/crawler_snowball_throw_1_left.png
059067312b40cbb93f10940facf610d48d8885c6b8879443c8c6e6141da0335a  examples/01_platformer/assets/sprites/crawler_snowball_throw_2.png
0637a84cf1cf9d98c92e37cdbe332d00f590bfd800b9e5fb59cee80ba8ec9cc1  examples/01_platformer/assets/sprites/crawler_snowball_throw_2_left.png
63acb8e733b9c6152cf2b3a632daea789e2cb16edb4bccd8f602f24433a38a2d  examples/01_platformer/assets/sprites/crawler_snowball_windup_0.png
7f91ac0d05d48913fe2237399b6e37fe515cdb025a42f92d2aa35963c1d32c25  examples/01_platformer/assets/sprites/crawler_snowball_windup_0_left.png
e81fcb6702c6e22bb80c7658bb1001dc69e6fdf1f990b9761dec152819599d65  examples/01_platformer/assets/sprites/crawler_snowball_windup_1.png
ccc68a3a48c710de3a0b61a663e8eedd6a4e58c7e05dcffb350ef3d0975e4430  examples/01_platformer/assets/sprites/crawler_snowball_windup_1_left.png
204ca2df3fae45ce9a59d02d8758c10f600d9ac23428292e2c080596656cf800  examples/01_platformer/assets/sprites/crawler_snowball_windup_2.png
d0a582753bd88b3a474a16b16eef549b5eb758689ba241d6a3c5da121e1bb9cf  examples/01_platformer/assets/sprites/crawler_snowball_windup_2_left.png
da9d6d74ddf72412933ca6679d8b0eb3a7bab1fafd9287165bfd6017e02b30c5  examples/01_platformer/assets/sprites/crawler_snowball_windup_3.png
c7e7c7d8b233b7c56fe5177e09bc9476ee6b0eaec7caa8371f02ed8e30b58afa  examples/01_platformer/assets/sprites/crawler_snowball_windup_3_left.png
c2f037820b6fdb0427644b4d4e63ce609bc7fe29bf488d2eff503cad4ea188a5  examples/01_platformer/assets/sprites/crawler_walk_0.png
263da83091e7054d591fba6c7548abec9a9978820748aeceeab32b01b143550b  examples/01_platformer/assets/sprites/crawler_walk_0_left.png
da443952c6eb0481856b5e27f3bc4f5d823343d9e9f1c9eeb9098a2002fd0e2c  examples/01_platformer/assets/sprites/crawler_walk_1.png
6b494b5fd403f0180aebd0c09f6bdb647ce189e52dda6dc4b23f3ee2bb0e16f2  examples/01_platformer/assets/sprites/crawler_walk_1_left.png
6206844251b91d745fedac8a18b9a3d6f64d4aa63191729afdb9e2fefe3602d0  examples/01_platformer/assets/sprites/crawler_walk_2.png
b65e903264c45e3898d48a3c790498f41e28f5dbffa3936e0588ab5faee8d1a7  examples/01_platformer/assets/sprites/crawler_walk_2_left.png
7fff946cc1d3cb1e1a301ae425314435ae6915d02c4ab8312e057027551ee28c  examples/01_platformer/assets/sprites/crawler_walk_3.png
01e310069f68f6c3e993da6f96eb2e19430c233a5a91b1021346f97b802cb664  examples/01_platformer/assets/sprites/crawler_walk_3_left.png
1a80a0d5fbf2086f582e8bb6342b37c49d250409eb845b51b33fda5e94294d90  examples/01_platformer/assets/sprites/crawler_walk_4.png
73194d7ce313aa9783f053ea546036b8a66e9e9d318d124863d0b9b85b8e1ffe  examples/01_platformer/assets/sprites/crawler_walk_4_left.png
92a195b49d3a1a18be7be741afc951c6d9eb32fa0c597027b34047418489a822  examples/01_platformer/assets/sprites/crawler_walk_5.png
87fadd07b5d2496f77c92c2dd547345e228139ccff56131f10f8e8ebead5b2f4  examples/01_platformer/assets/sprites/crawler_walk_5_left.png
25620ed304565c5fc0611508580d6696e9fd78172abf59cf6b364bb71f6f2875  examples/01_platformer/assets/sprites/crawler_windup_0.png
817f4ddd6c6f22d7438e088aaf68a60d8e5b797d34347fac4b113baa694f7438  examples/01_platformer/assets/sprites/crawler_windup_0_left.png
87a06032d9751b75e681d2bd9b42957316a404f112296a5623944f79b0d08d03  examples/01_platformer/assets/sprites/crawler_windup_1.png
a14ef3ec88cc7bdb57b7c3d597a136fdfac9642bb0d489660cd0d8ae021a6b8d  examples/01_platformer/assets/sprites/crawler_windup_1_left.png
e84402f1495ee83d13104efb338246442f5e1b1a65ccd7de31fdc30c7ae7fbe2  examples/01_platformer/assets/sprites/crawler_windup_2.png
966a71d1953085c114bde874218ea53aec75f1c52a7504eb573bd1ce5138d65b  examples/01_platformer/assets/sprites/crawler_windup_2_left.png
2e5ae307593efc9fa56913d8a8cf7e9d6c0ccadfdea2867f745b725a1d264e70  examples/01_platformer/assets/sprites/crawler_windup_3.png
43752b248a8e07966434805eeac97e9afe265569bf3fa3ead566b34c2eb7825d  examples/01_platformer/assets/sprites/crawler_windup_3_left.png
d01f998524eca7e9e35c123bbc525a256864a0d32efb11102c697059fb74cf5a  examples/01_platformer/assets/sprites/goblin_dash_0.png
deb120146b1820da8c050278b68e5c8314ea9095aa6b07c317970ab1c5249ded  examples/01_platformer/assets/sprites/goblin_dash_0_left.png
c3985606fc4b2b5bd0567967969c6057ff754ba367830b1eb6d48c800fb3744f  examples/01_platformer/assets/sprites/goblin_dash_1.png
2a3d1946a71005a0eae05bbe4c2590ef4f3b2d0cc34d4fe62034465132ab44cd  examples/01_platformer/assets/sprites/goblin_dash_1_left.png
4d18766cd48edbefa7d723d1bffdd3636af01c6031b3f12d50b10461daf24fa8  examples/01_platformer/assets/sprites/goblin_dash_2.png
312529076e969c6244487b909d752a7a4d77585aa11c3a43073d3bce47092d0d  examples/01_platformer/assets/sprites/goblin_dash_2_left.png
628436dcb296c4579f7e343b33160a298cae693f7cba5a0820174283c08fb2d3  examples/01_platformer/assets/sprites/goblin_dash_3.png
73003873eb78d5ac0a795aa1f60f627d5adc902774e8b46062f153620ea57842  examples/01_platformer/assets/sprites/goblin_dash_3_left.png
68afb92db44fdd4cec40209e8ebea148a7fa2fbf212017fa1d6022e10ed7d5e6  examples/01_platformer/assets/sprites/goblin_dash_windup_0.png
74593c5cae0dbfc8cc07a820abc6983c505cb595c4087f286ac7242222b82350  examples/01_platformer/assets/sprites/goblin_dash_windup_0_left.png
5481cc612ce9336768b09efe1a6b6c7b6f4c33d89df04a6536ba41ad76c2d25c  examples/01_platformer/assets/sprites/goblin_dash_windup_1.png
1fabeb954a0b5b480f95da36dd7efb401de6fe92b12db231be02ba508c120972  examples/01_platformer/assets/sprites/goblin_dash_windup_1_left.png
370562d9bb230bda8c7ef8a7e0069552fa2e830a8b8c653c510748f46d020ae0  examples/01_platformer/assets/sprites/goblin_panic_0.png
a1f387249e04d02b4b2e809af25926b7023338b0d71592f38984879f97495a75  examples/01_platformer/assets/sprites/goblin_panic_0_left.png
f08a8061575fa72f0abaa986b215227ffc0c6abe9b7ea173b7ae269673b47731  examples/01_platformer/assets/sprites/goblin_panic_1.png
2ae5e4b7c19a349668f4baa06fb4d10aac640a9f54a94ec51bfa6e68b9e35975  examples/01_platformer/assets/sprites/goblin_panic_1_left.png
532d6c9fdc546b51555d74a0fa583622cf6237407bf7ca7c67c16947655bcaee  examples/01_platformer/assets/sprites/goblin_panic_2.png
4fb15f6b15b14149c215de301633c326db54beacb4c3ef0d2bc97a708c08a770  examples/01_platformer/assets/sprites/goblin_panic_2_left.png
4ac9185d9b247ac9daad52bb01a48a48341798582fa2a14e5ac7e9089ef23dc3  examples/01_platformer/assets/sprites/goblin_panic_3.png
bb660537458efc832406a8acef924509c1163c34bcd65ea6f04cb0ef2b27252a  examples/01_platformer/assets/sprites/goblin_panic_3_left.png
5023a2b29773885605dbb2855bb52709dcce39448b9e9bc8c2f84d71d14785fa  examples/01_platformer/assets/sprites/goblin_panic_4.png
51a128090adc4dd289c2d80a84ee453d6b4c1e91a4c80c3abda39d9a0847dc8f  examples/01_platformer/assets/sprites/goblin_panic_4_left.png
8899927f6d8b8e2338860f5089d3434eb6923a73f2733840e38b654287acfc08  examples/01_platformer/assets/sprites/goblin_panic_5.png
9384114e15216839b95e498b9ad7a4318a1429e400fb2fb92b8f57b25e0a3930  examples/01_platformer/assets/sprites/goblin_panic_5_left.png
d13b25445925d08bec526bca80c035f50e1df1a0d02bfed6a609cbe297e2e519  examples/01_platformer/assets/sprites/icon_crawler.png
191e47c6bb11fb387190ddd7c109db20bb6b91027f425aaf6106b56c473c79f2  examples/01_platformer/assets/sprites/snowball_0.png
85417a7995b6132fc829d02c211c6a1c2ed8911cbfc0b17318762ac547055ff7  examples/01_platformer/assets/sprites/snowball_1.png
07c9442f57386a0690e9d55129275fe27e5ab916e735c7306791adb74dd3b416  examples/01_platformer/assets/sprites/snowball_2.png
9f2a748877fdc9e54ed1888302b01fea57e0ea19f01794bf9a6a851fefbffe53  examples/01_platformer/assets/sprites/snowball_3.png
8c175519e8459ea9904f52e21df377eb3ae31a384abc53028191f9db532b30b0  examples/01_platformer/assets/sprites/snowball_burst_0.png
48d75897fbd5d2dab8caa0bd8d1f0ccea55716db5b9ee1178fcf67d80a249c21  examples/01_platformer/assets/sprites/snowball_burst_1.png
a32c5a8095315df342d6c086f5aacb6206618105009c9a71934be0956b07c8ef  examples/01_platformer/assets/sprites/snowball_burst_2.png
38f29d9da08b7e5df3ce88aba024e5e30e8cdfeb5ad66471cd7f33fd33e957d6  examples/01_platformer/assets/sprites/snowball_burst_3.png
f8267ecf78f9624e42e41c21863f152c32b65c33e60966b63112b112ecec19ca  examples/01_platformer/assets/sprites/snowball_burst_4.png
3d17b143cd314531cc6cd5d6513a8284c439550a27faf692ec1c4b6db3869d72  examples/01_platformer/assets/sprites/snowball_burst_5.png
```

### 3.7 The asset session's prompt

A run session that reaches Step 7 without its evidence row ends with this, for a new session on Opus 5.5 at max:

`Use the tungsten-milestone skill. Read docs/plans/e01-foes.md and run Step 7 alone, the asset session: draw §3.1's families in place of Step 1's placeholders as §3.2 directs, within the contract, with the generator's own toolkit; regenerate; link me §3.5's review sheet with in-game loops and take my notes in rounds. On my acceptance write §3.6's lock and the evidence row, delete the scratch driver and the venv, commit and push under docs/operating-rules.md. Leave src/ and tests/ alone; end with the run prompt for Step 8.`

## 4. Steps

Every code step's gate also takes `cargo fmt --all -- --check` → exit 0, `cargo clippy -p example-01-platformer --all-targets --locked -- -D warnings` → exit 0 and `cargo test -p example-01-platformer --locked` → pass (01's whole suite, its count in the evidence). A step that regenerates `assets/` makes a scratch Pillow venv once a session (`python3 -m venv "$VENV" && "$VENV/bin/pip" install -r examples/01_platformer/tools/requirements.txt`, `$VENV` in the scratchpad, removed with `shutil.rmtree` at the session's end) and passes `PATH="$VENV/bin:$PATH" just level-check` → pass. Filtered runs name the step's new tests in the evidence, since a filter that matches nothing still exits 0.

### Step 1: Placeholders for §3's contract

- **Files:** Step 1's in the header.
- **Change:** §3.4: every §3.1 file from today's art (the clips in `ENEMY_CLIPS` and where the generator keeps its effects' clips, the particle configs in `particle_configs`, the sounds in `build_sounds` and `sounds.json`); the contract test; a line in `tools/README.md` naming them placeholders until Step 7; regenerate.
- **Done-when:** `PATH="$VENV/bin:$PATH" just level-check` → pass, the contract test among its tests; `cargo test -p example-01-platformer --locked` → pass (no code change); `just repo-check` → pass; `git status --porcelain` → only Step 1's paths.
- **Commit:** `feat(example-01): placeholders for the foes' art and sounds`
- **Moves:** `assets/tilemaps/level.tmj`'s tile IDs, which new sprites shift; the manifest.

### Step 2: The beetle's charge and tell (V17, V18)

- **Files:** Step 2's in the header.
- **Change:** §2.2, `BeetlePloughed` (§2.8); 01's README row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- enemies:: presentation:: data::` → pass, with new tests that a charge ploughs a crate and a keg out of its way from either side (each moved at least 120 px along the heading, the beetle's centre past its start, the keg not blown, one `BeetlePloughed` a step at most), that it throws a standing player at 700–850 px/s, more than 150 of it up, that it stops at iron (moved under 4 px, the beetle behind it, Recover at `CHARGE_SECS`), that its paw puffs `ex10_paw_dust` and its charge draws `ex10_charge_streak` frames, and F1's test outlining a charging beetle's plough box; `the_beetle_paws_then_charges_and_its_charge_hurts_enemies_too` and `a_walker_leaves_crates_and_kegs_where_they_stand` unchanged and passing; the level-check gate.
- **Commit:** `feat(example-01): the beetle ploughs through props and reads its tell`

### Step 3: The crawler's fear, retreat, speed and hops (V20, V21)

- **Files:** Step 3's in the header.
- **Change:** §2.3, §2.4 with schema 17, `CrawlerRetreated` and `CrawlerHopped`; the goblin onto `hop.rs`; 01's README rows.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- enemies:: saves:: data::` → pass, with `a_crawler_is_scared_at_the_drawn_light` moved to the fear edge (outside it, within 24 px; one hiss and one puff; its crawler's snowball cooling once Step 4 lands), and new tests that a crawler 120 px from a lit light backs out past the fear edge within 1.5 s facing the player, showing `crawler_retreat`, with one `CrawlerRetreated` and no wind-up, that a cornered one stands and still lands its 15 cold touch, that it runs at 210 from a fresh spawn, that a chasing crawler hops a 48 px step and leaps a level 64 px gap, a 64 px gap onto ground 16 px higher and a 140 px gap onto ground 90 px lower to the player but stops at a gap onto ground 48 px higher and at a 256 px gap, from either side, with `CrawlerHopped`, and that a schema 16 save from the 0.109 code (a current capture without the new fields, numbered 16, its crawlers' base at 160) loads them at 210; `the_crawler_walks_its_ledge_and_turns_at_the_ends` and every goblin test unchanged and passing; the level-check gate.
- **Commit:** `feat(example-01): crawlers back out of the light, run and hop`
- **Moves:** 01's save schema, 16 to 17; the scare test's edge.

### Step 4: The crawler's snowball (V22)

- **Files:** Step 4's in the header.
- **Change:** §2.5, `SnowballThrown` and `SnowballBurst`; `stalker()` and the scare test set their crawler's `cooldown` to 60 s, so the lunge tests stay lunges and the scare its edge; `data.rs` pins the snowball's body; the coverage test lists `Snowball` as saved; 01's README row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- enemies:: saves:: data::` → pass, with new tests that a dark crawler 260 px off throws a snowball on its arc (within 2 px for its first twelve steps) that lands 12 cold from the crawler and chills the player, and throws again no sooner than 4 s after, that on its cooldown it closes in and lunges, that a crawler held at the fear edge snowballs a lit player and never lunges, that an afraid one throws nothing, that a snowball bursts at its first touch of the ground, a vertical wall and a crate, hurting nothing, that one on a dummy chilled near its freeze rung freezes it, and that a run with a snowball in flight and a cooling crawler saves through JSON and loads them back, the snowball landing as it would have; every lunge test passing; the level-check gate.
- **Commit:** `feat(example-01): the crawler's snowball`

### Step 5: The troll (V25)

- **Files:** Step 5's in the header.
- **Change:** §2.6; 01's README row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- enemies:: props::` → pass, with new tests that a chasing troll lifts an unlit keg and throws it at the player, never lighting it, and leaves a lit one, that a searching troll lifts the crate in its way and passes the crate's start, that a returning one kicks scrap from its way and passes it, that it sets what it holds down on reaching its patrol, and that a keg it threw that blows credits it; the troll's chase tests (`enemies.rs:2933-3025`) unchanged and passing.
- **Commit:** `feat(example-01): the troll lifts kegs and clears its way`

### Step 6: The goblins (V26)

- **Files:** Step 6's in the header.
- **Change:** §2.7, `GoblinCrouched`, `GoblinDashed` and `GoblinYelped`; `goblin_seed` and the bombs test follow the new draws (a dash draw each approach, a bomb draw each throw's tell), their count 12 throws within 8,000 frames; the rock test's seed likewise; 01's README row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- enemies:: saves:: data::` → pass, with new tests that throws in its band come at least 2.5 s apart with no tell between, that a seed whose first approach draws a dash gives a crouch (`Strike::Dash`, one `GoblinCrouched`), a dash at about 560 px/s and a stab of 6 that reaches, then the leap back, that over twelve approaches the dashes are the stream's draws under 1/3, that a goblin set burning mid-wind-up spends its strike, panics with one yelp and `goblin_panic`, runs from the courier on either side and chases or searches once the fire ends, that one set burning mid-dash moves away from the courier within two steps and lands no stab, and that a cooling goblin, a dashing one and a panicking one save through JSON and load back the same; the moved seeded tests passing; the level-check gate.
- **Commit:** `feat(example-01): goblins cool their throws, dash in and panic`
- **Moves:** the seeded goblin tests' seeds and counts.

### Step 7: Art and sound (the asset session)

- **Who:** a new session on Opus 5.5 at max, on §3 alone, after Steps 1–6 (Q1); the owner reviews.
- **Files:** Step 7's in the header.
- **Change:** draw §3.1's families as §3.2 directs, in place of the placeholders; the nine sounds in `sfx.py`, their volumes in `sounds.json`; `tools/README.md`'s rows; regenerate; §3.5's sheet; the owner's notes in rounds, a new small session when the context grows; on acceptance §3.3's measurements and §3.6's block, the evidence row, the scratch driver and the venv deleted.
- **Done-when:** the owner's acceptance quoted in the evidence row; `PATH="$VENV/bin:$PATH" just level-check` → pass, the contract test among its tests; `cargo test -p example-01-platformer --locked` → pass; `just repo-check` → pass; §3.6's check → exit 0; `git status --porcelain` → only Step 7's paths.
- **Commit:** `feat(example-01): the foes' art and sounds`

### Step 8: Close

- **Files:** Step 8's in the header.
- **Change:** run the checks and the demo; in-game captures of §3.5's scenes on the accepted art, by its driver, for the owner; finish 01's README (each foe's row and the kit-items table); `DESIGN.md`'s workspace line gains the foes in 01's clause; §3.9 of the feedback gains its landed note, as §3.8's has; open follow-ups to Later or known issues; `e01foes` out of Now and `e01sfx` into it; §3.6's check, then this plan deleted with its citations; the venv removed.
- **Done-when:** §3.6's check → exit 0 before the deletion; `just check` → pass (formatting, lints and every crate's tests, 01's whole suite among them); `PATH="$VENV/bin:$PATH" just level-check` → pass; `just smoke` → pass; `just repo-check` → pass; the captures named in the last evidence row.
- **Commit:** `docs(example-01): records follow e01foes`
- **Ends with:** the commits made, the evidence quoted, what the owner should play (a lit and a dark crawler in the aqueduct, the mine's beetles among the crates, a goblin set alight, the troll at the bridge's kegs) and the next prompt.

## 5. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The sessions' order | The code session runs Steps 1–6, then the asset session Step 7, since §3.5's loops need the behaviour, then a new code session Step 8. Overlap only in a worktree the owner makes, its branch merged by the owner with `assets/` regenerated at the merge, Step 7 then starting from Step 1's commit |
| Q2 | "None while scared" (Q52, Q54) | Afraid, inside the fear edge, it backs out and neither lunges nor throws; held at the edge it may throw and never lunges while the light burns |
| Q3 | The fear edge | 1.3 × the glow's reach, 227.5 px, the drawn light unchanged (Q52) |
| Q4 | Where the crawler hops | Chase, Search and Return; its patrol keeps its ledge, as the walk test pins; the goblin keeps its table, no leap |
| Q5 | The snowball's numbers | 12 cold at the crawler's factor, any cold's build-up (a chilled dummy may freeze, the player never); a 4 s cooldown from its release; 140–360 px across, 200 up or down; a flight of distance / 650 held to 0.3–0.7 s; 8 live at most; it bursts at its first touch or after 3 s, hitting only targets off `team.enemy` |
| Q6 | Snowballs in a save | Their own list, not `props`, since `bodies()` errors on a prop kind with no box (A15) |
| Q7 | The plough's numbers | §2.2's, for bodies under the beetle's mass: a crate at 1,000, a keg at 750; iron a wall; under `KEG_SPEED`, so a ploughed keg blows only past Q46's threshold |
| Q8 | The player's throw (Q74) | Knockback 800 on the kit hitbox, its lift from the hitter-to-target line; 01's own box hit, as the swipe's, if the owner finds the lift weak |
| Q9 | "One approach in three" | One draw each time the goblin's band and cooldown allow an attack: a dash under 1/3, else a throw; both cool 2.5 s; the stab at 56 px stays uncooled |
| Q10 | The dash's numbers | A 0.3 s crouch, 560 px/s for at most 0.6 s, the stab once within reach; a body stops it (K30) |
| Q11 | The panic's shape | A new `EnemyState::Panic`, for the goblin alone, ending with the burning tag |
| Q12 | The troll in Search and Return | It lifts what blocks it, kicks what it can't lift, carries what it holds until it sees the player, and sets it down at its patrol |
| Q13 | Which kegs it lifts | Unlit ones; a lit keg is kicked |
| Q14 | `D-210`'s "never a keg" (Q55 says amended) | No entry: the operating rules give none to a change that constrains no later work, as `e01quick2` changed `D-210`'s 600 px/s keg rule with none; 01's README carries the rule |
| Q15 | Sounds | §2.8's eight and the paw's redraw; the troll's kick takes its swipe's; burning stays `e01sfx`'s |
| Q16 | The crawler's icon and ice cube | The icon follows the redraw (it draws from the canvas); the cube stays fitted to the unchanged hurtbox |
| Q17 | The strike width | The redraw keeps the walk's reach at 30–32 px a side, so `KindInfo::strike` stays 32; one outside it is a contract change (§6) |
| Q18 | The bat's dive | Unchanged (Q51 picked the beetle's tell) |
| Q19 | Placeholders | Copies of today's art at the contract's sizes and counts, never drawn (§3.4) |
| Q20 | The lunge's strike | None, as now, so an older save's crawler mid-lunge loads as before; `Strike::Snowball` names the throw alone |

Each question takes its stated default; the owner reviews the plan on their own time.

## 6. Invariants and stop conditions

- **May move:** `level.tmj`'s tile IDs and the manifest (Step 1; Step 7 changes bytes only); 01's save schema, 16 to 17 (Step 3); `Sound::ALL`, 73 to 81; the scare test's edge (Step 3); the seeded goblin tests (Step 6).
- **Must not move:** the kit hash and the determinism and containment hashes and row digests (no kit or core change); `gpu-visual.png` and the post and transition regressions; 01's `src/shell/`, byte for byte the template's; the `S15_*` and `S16_*` tables; the crawler's frame and body (Q73); every `LAUNCHES` row and the tomes' spots; every goblin hop test; the walker tests on patrol; §3.6's files after Step 7.
- A done-when check fails: restore the step's files from the last commit, mark the step skipped and carry on with the steps that don't depend on it (2 and 3 need 1; 4, 5 and 6 need 3; 6 adds `Brain::cooldown` and `cooldowns` itself if 4 was skipped; 7 runs on what landed, telling the owner which loops are missing; 8 needs 7's lock).
- §3.6's check fails after a regeneration: an art file changed, which only an asset session may change; restore the step's files.
- Step 7 needs a contract change (a size, anchor, count or timing the code reads, a new ID): it records it in its evidence row and leaves `src/` alone; Step 8 takes a one-constant change, anything more goes to Later.
- A full pit's save passes 2 MiB, or the kit hash moves: the step restores and stops.
- A file outside the step's list needs a change: a test that pins a value the step moves is added and recorded; anything else skips the step.
- The Pillow venv will not install: stop, since the generator cannot run.

## 7. Critique

The `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-26, reading `433b499` with this plan uncommitted, §7 and the log cut; round 1. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 581.6 s, tokens in 1753517 (cached 1625984), out 14049 (reasoning 8419), commands 31, output 394.5 KB (largest 41.7 KB), usage 5h 18% → 54%, week 7% → 13%`.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Reapplying the iron kick defeats "iron stays put": friction after the step lets each step's kick top iron up again, about 24 px over a charge | `features/toys/systems.rs:838-915` (friction in the overlap phase, after the step) | Accepted | §2.2: only bodies under the beetle's mass are kicked, iron a wall; A31; Q7 |
| 2 | [MEDIUM] Panic does not override a dash's lock: `scurry`'s run is ignored while locked, and a lock only lengthens | `crates/tungsten-kit/src/motion/platformer.rs:568-574`, `:679-692` | Accepted | §2.7: Panic's enter launches it away, which a lock does not refuse; Step 6 tests a goblin set burning mid-dash; A33 |
| 3 | [MEDIUM] The leap accepts ground 64 px up, but 600 px/s rises 50 px | `features/level/mod.rs:52` (3,600; 600² / 7,200 = 50) | Accepted | §2.4: far ground from 24 px up to 96 down, within the arc's reach at its height; Step 3 tests ground 16 px and 48 px higher across a gap |
| 4 | [MEDIUM] Contact events don't guarantee a burst: speculative contacts stop a body at touching with no event | `crates/tungsten-core/src/physics/step/mod.rs:9-17`, `:876-892` | Accepted | §2.5: a touch test (its circle grown 2 px against tiles and bodies, its thrower and sensors left out) in place of events, the release moved clear of its body to (66, 40); Step 4 tests a vertical wall; A32 |
| 5 | [MEDIUM] "Freezes nothing" against the snowball's filter: the dummy is `freezable` | `tools/content/prefabs/dummy.json` (`freezable`, 20 + 0.05 × max); the cold ladder's freeze rung at 0.5 | Accepted | §2.5, Q5: its cold builds as any cold, a chilled dummy may freeze, the player never; Step 4 tests a chilled dummy; A34 |
| 6 | [MEDIUM] The contract's lowest opaque row rejects today's frames: the outline lies on row 60 | `tools/actors.py:26`, `tools/pixels.py:112`; every crawler, beetle and goblin frame measured, its lowest opaque row 60 | Accepted | §3.1, §3.4, A23: row 60 for grounded frames, the hops exempt |

Round 1 changed mechanisms (the plough's gate, the panic's launch, the leap's window, the burst's test, the contract test's row), so round 2 ran at xhigh with §7 and the log cut and a focus note of these changes. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 262.3 s, tokens in 838373 (cached 765312), out 7145 (reasoning 3609), commands 19, output 220 KB (largest 33.9 KB), usage 5h 55% → 73%, week 13% → 16%`. Each of its three was a gap a round-1 fix left; none is open, so no question carries one. Sweeping the fixes against each other then found the placeholder sounds outside §3.1's lengths (the scare's hiss 0.38 s, the lunge's 0.36, `ice_shatter` 0.62, today's paw 0.42): §3.4 names sources inside them, measured.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] The snowball's touch test meets the snowball itself: a filter excludes nothing by default, and a second `excluding` replaces the first | `crates/tungsten-core/src/physics/query.rs:60-95` | Accepted | §2.5: the filter excludes the snowball, its thrower passed over in the results; A35 |
| 2.2 | [MEDIUM] The leap's reach assumes 420 for the whole flight, but the 0.35 s lock ends before a landing 96 px down, and the run's 210 takes over | `crates/tungsten-kit/src/motion/platformer.rs:688-692` | Accepted | §2.4: the reach at 420 while locked, then the run's speed (168 px at 96 down, from 190); Step 3 tests a 140 px gap onto ground 90 px lower |
| 2.3 | [MEDIUM] A prescribed goblin placeholder breaks the row-60 contract: `goblin_stab_0` ends on row 62 | Every goblin frame measured: the stab's first 62, the hop's first 59, the rest 60 | Accepted | §3.4: the dash and the panic from its walk, the crouch from its stab wind-up; A23 corrected |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-26 | Passed | `level-check`: `OK: 5015 owned outputs match`, `Ran 28 tests … OK` (the four `FoeContractTests` among them); 01's suite 351 passed, 1 ignored; `repo-check` exit 0. 82 sprites, 19 clips, 3 particle configs and 8 sounds added; `beetle_paw` 4 frames looping → 8 once, its sound 0.42 → 0.8 s. The streak's "trail frame" is the gun's `tracer`, doubled. Deviation: `tools/art.py`, outside the list, takes the new off-grid sizes (`ELEMENT_SIZES`, which `validate_art` holds every sprite to) and `AIRBORNE`, the crawler's hop let off `WALKER_FEET`'s row 60 as §3.1 lets it rise, so Step 7 needs no `art.py` change | `examples/01_platformer/tools/`, its `assets/` |
| 2 | 2026-10-26 | Passed | `--test game -- enemies:: presentation:: data::`: 101 passed, among them the new `a_charge_ploughs_crates_and_kegs_out_of_its_way_from_either_side`, `a_charge_throws_a_standing_player_back_and_up`, `a_charge_stops_at_iron_and_presses_it_until_it_ends`, `the_beetles_paw_puffs_its_dust_and_its_charge_draws_its_streaks`, `presentation::f1_outlines_a_charging_beetles_plough_box`, and `the_beetle_paws_then_charges_and_its_charge_hurts_enemies_too`, `a_walker_leaves_crates_and_kegs_where_they_stand` unchanged; 01's suite 356 passed, 1 ignored; fmt and clippy exit 0; `level-check` 5,015 outputs, 28 tests OK. `Sound::ALL` 73 → 74 | `src/features/enemies/{beetle,draw}.rs`, `tests/game/{enemies,presentation}.rs` |
| 3 | 2026-10-26 | Passed | `--test game -- enemies:: saves:: data::`: 124 passed, among them the new `a_crawler_in_the_light_backs_out_past_its_fear_facing_the_player`, `a_cornered_crawler_stands_and_still_chills_on_touch`, `the_crawler_runs_at_210_from_a_fresh_spawn`, `a_chasing_crawler_hops_a_step_and_leaps_the_gaps_it_can_land_across` (both sides: over a 48 px step, a level 64 px gap, 64 px onto +16, 140 px onto −90; stopped at 64 px onto +48 and at 256 px, at the edge) and `saves::a_schema_16_save_loads_its_crawlers_at_their_new_run`; `a_crawler_is_scared_at_the_drawn_light` held at the fear edge, `the_crawler_walks_its_ledge_and_turns_at_the_ends` and every goblin test unchanged and passing; 01's suite 361 passed, 1 ignored; fmt and clippy exit 0; `level-check` 5,015 outputs, 28 tests OK. Schema 16 → 17, `Sound::ALL` 74 → 76. The six migration tests that compare a fixture's fields gained `as_schema_17`, the step's own value move | `src/features/enemies/{crawler,goblin,hop}.rs`, `src/save.rs`, `tests/game/{enemies,saves,data}.rs` |
| 4 | 2026-10-26 | Passed | `--test game -- enemies:: saves:: data::`: 130 passed, among them the new `a_dark_crawler_snowballs_the_player_on_its_arc_then_cools_four_seconds` (12 steps on its arc, 12 cold from the crawler, chilled, the next throw ≥ 4 s on), `a_cooling_crawler_closes_in_and_lunges`, `a_crawler_held_at_the_fear_edge_snowballs_a_lit_player_and_never_lunges`, `an_afraid_crawler_throws_nothing`, `a_snowball_bursts_at_its_first_touch_of_the_ground_a_wall_and_a_crate_hurting_nothing` (each within 7 px of the surface), `a_snowball_on_a_dummy_chilled_near_its_freeze_rung_freezes_it` and `saves::a_snowball_in_flight_and_a_cooling_crawler_save_and_load_and_it_lands_as_it_would_have` (the same step, within 0.5 px); every lunge test passing; 01's suite 368 passed, 1 ignored; fmt and clippy exit 0; `level-check` 5,016 outputs, 28 tests OK. `Sound::ALL` 76 → 78. Also cooled 60 s, as the plan cools `stalker()`'s and the scare test's: Step 3's retreat test and `saves::a_save_with_a_pending_transition_enters_it_once_after_the_load`, whose crawlers' first attack is now the snowball. The edge test allows the staff light's sway with the wizard's idle frames (up to 12 px), which the crawler backs out of; a dummy freezes once chilled and 10 more cold in, since the chill rung empties the bucket | `src/features/enemies/{snowball,crawler,components,systems}.rs`, `src/save.rs`, `tests/game/{enemies,saves,data}.rs` |
| 5 | 2026-10-26 | Passed | `--test game -- enemies:: props::`: 101 passed, among them the new `a_chasing_troll_lifts_an_unlit_keg_and_throws_it_but_leaves_a_lit_one`, `a_searching_troll_lifts_the_crate_in_its_way_and_carries_it_past`, `a_returning_troll_kicks_scrap_from_its_way_and_passes_it`, `a_troll_sets_down_what_it_holds_back_on_patrol` (no blast, on the ground) and `a_keg_a_troll_threw_that_blows_credits_it` (its blast's hits on a dummy from the troll); `the_troll_walks_at_a_courier_on_either_side_and_slams_it` and `the_troll_swipes_what_is_close_and_slams_what_is_farther` unchanged and passing; 01's suite 373 passed, 1 ignored; fmt and clippy exit 0. Deviation: the patrol's set-down places what it carried behind its back at its hands' height before `drop_held`, since its hands are over its own head and a plain drop landed the keg on it. `a_save_while_the_troll_holds_iron_loads_with_the_hold` and `killing_or_evicting_the_troll_drops_what_it_holds` set their troll searching (`carrying`), as on patrol it now sets down what it holds | `src/features/enemies/troll.rs`, `src/features/elements/systems.rs`, `tests/game/enemies.rs` |
| 6 | 2026-10-26 | Passed | `--test game -- enemies:: saves:: data::`: 142 passed, among them the new `a_goblins_throws_come_at_least_its_cooldown_apart_with_no_tell_between`, `a_goblins_dash_in_crouches_sprints_and_stabs_then_leaps_back` (one crouch, 560 px/s within 10, a stab of 6, the leap back), `a_goblins_dash_ins_are_the_streams_draws_under_a_third` (twelve approaches, both kinds), `a_goblin_set_burning_mid_tell_panics_from_the_courier_on_either_side` (strike spent, one yelp, `goblin_panic`, 60 px farther off, then a chase or a search), `a_goblin_set_burning_mid_dash_turns_away_at_once_and_stabs_no_one` and `saves::a_cooling_a_dashing_and_a_panicking_goblin_save_and_load_back_the_same`; `goblin_seed` and the bombs test on the new draws (12 throws within 8,000 frames, the bombs the stream's), the rock test on its seed; 01's suite 379 passed, 1 ignored; fmt and clippy exit 0; `level-check` 5,016 outputs, 28 tests OK; `just check` exit 0 (3,538 passed, 9 ignored). `Sound::ALL` 78 → 81. The panic's skip is from the ground (in the air it keeps its flight, so a lava throw stands), and a dash that reaches its courier stops for the stab | `src/features/enemies/{goblin,components,systems,mod}.rs`, `tests/game/{enemies,saves}.rs` |
| 7 | 2026-10-26 | Passed | The owner accepted round 1, the first draw: "Looks good approved". `level-check`: `OK: 5016 owned outputs match`, `Ran 28 tests … OK` (`FoeContractTests` among them); 01's suite 379 passed, 1 ignored; `repo-check` exit 0; §3.6's check exit 0 over 183 files (139 sprites with their twins, 32 clips, 3 particle configs, 9 sounds), 152 of them changed, beside the manifest's nine volumes; every other output as at HEAD, the beetle's walk and the goblin's other clips among them. §3.3: the walk's opaque columns reach 31 px left and 32 right of x 32; on the throw's first frame the open jaws' tips sit at x 62, 4 px behind (66, 40), so the snowball (within 5.5 px of its centre) leaves from inside the mouth; the tell 360 ms (0.36 s), the throw 250 (0.25), the paw 800 (0.8), the dash's crouch 300 (0.3), the burst 300 (0.3). New: `pixels.grade`, eight palette symbols (`ѳ ѵ` the carapace's, `Ѣ ѣ Ѳ Ѵ` the snow's, `ѐ ё` the charge's leg blur). No contract change; no `src/` or `tests/` file. The review's loops are staged by a scratch driver (`perf-runs/20261026-e01foes-art-review/tools/`, deleted from `target/`): the hop course is carved in its run folder's map only, the level having no one-tile gap, and the beetle's crate and keg land as its paw begins, since they would hide the wizard from it (V24) | `examples/01_platformer/tools/{actors,effects,generate,pixels,sfx}.py`, `tools/{palette,sounds}.json`, `tools/README.md`, its `assets/`; `perf-runs/20261026-e01foes-art-review/` |

## Follow-ups

- K30 in the kit, a locked controller's speed kept at a shoved body: the feedback's backlog row.
- Snowballs that douse burning props or chill bodies: a Later line if play asks for it.
- The generator's own lock for accepted art: redo step 7.1.
