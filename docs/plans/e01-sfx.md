# Example 01: sounds and effects (`e01sfx`) — draft

- **status:** in progress
- **goal:** Example 01 (n) ([feedback](1.0/example-01-feedback.md) §3.10, V27, V30, V31, V33, V37, V38): the kit's cue sounds gain an opt-in listener with a falloff by distance (K29), and 01 hears its one-shots and held loops from the player; lightning says what it met, rolls deeper and strikes the ground with a crackle, sparks and a scorch; shock crackles with arcs over the body, brighter sparks, a stronger flicker and longer chain arcs; one crackle voice follows the nearest burning thing, and every damage tick on the player pulses the screen's edge red with a soft tick; a crate hit hard enough breaks into five burnable planks, capped and saved (schema 18); both sprays burst on whatever they meet. The art and sound come from a redo [§6 asset session](roadmap-redo.md#6-asset-sessions) on §3's brief, after the code steps, which build on placeholders until the art is accepted.
- **non-goals:** Stereo pan, looping cue voices, bus ducking and sound hot reload (redo 7.9, P7-13). A listener on `SoundOnEvent`, whose events carry no position. K1 in the kit (the tick's sound is 01's code). Placed area ambience, music or combat layer. Planks from blasts, burning or blows on kinematic bodies; snowballs that douse fire (Later). Any map, route, light or level change. Any core, render or template change. A release, a changelog section or a decision entry (§4 Q19): a release is cut separately (`D-215`).
- **files to touch:** paths under `examples/01_platformer/` (01's) unless rooted; "the generated `assets/`" is the generator's output with `tools/outputs.json`.
  - Step 1, placeholders: `tools/{art,effects,generate,props,sfx}.py`, `tools/sounds.json`, `tools/test_generate.py`, `tools/README.md`; the generated `assets/` (sprites, particles, sounds, `manifest.json`, `tilemaps/level.tmj`).
  - Step 2, the kit: `crates/tungsten-kit/src/react/cues.rs`, `crates/tungsten-kit/src/tests/cues.rs`, `crates/tungsten-kit/src/lib.rs`, `api/tungsten-kit.txt`, `DESIGN.md`.
  - Step 3, hearing by distance: `src/features/audio/{mod,systems,components}.rs`, `tests/game/audio.rs` (new), `tests/game/main.rs`, `README.md`; and only to move a source within hearing (§6), `tests/game/{ice,presentation,enemies,hazards,spawning,brick,spells}.rs`.
  - Step 4, lightning and shock: `src/features/lightning/{mod,draw,systems,components}.rs`, `src/features/combat/{systems,draw}.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{lightning,presentation}.rs`, `README.md`; the generated `assets/`.
  - Step 5, the damage edge and tick: `src/features/player/{systems,components}.rs`, `src/features/combat/{draw,mod}.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/data/cue_rules/sounds.json`, `tests/game/player.rs`, `README.md`; the generated `assets/`.
  - Step 6, planks: `src/features/props/{mod,components,systems,draw}.rs`, `src/features/elements/{systems,mod}.rs`, `src/save.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/prefabs/plank.json` (new), `tools/content/data/cue_rules/sounds.json`, `tests/game/{props,saves,data}.rs`, `README.md`; the generated `assets/`.
  - Step 7, spray impacts: `src/features/toys/systems.rs`, `tests/game/spells.rs`, `README.md`.
  - Step 8, the asset session: `tools/{art,effects,generate,props,sfx,pixels}.py` (`pixels.py` a new helper only), `tools/{sounds,palette}.json`, `tools/README.md`; the generated `assets/`; this plan's §3.6 lock and evidence row.
  - Step 9, close: 01's `README.md`, `DESIGN.md`, `docs/plans/1.0/example-01-feedback.md` (§3.10's landed note), `docs/known-issues.md` (only for a finding), `ROADMAP.md`, this plan (deleted).
- **ordered steps:** (1) Placeholders for §3's contract. (2) The kit's listener (K29). (3) 01 hears by distance, the crackle on every burning thing. (4) Lightning and shock. (5) The damage edge and tick. (6) Planks, schema 18. (7) Spray impacts on every surface. (8) Art and sound, the asset session, ending in the owner's acceptance and the lock. (9) Close. §4 holds each.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes with 01's whole suite, `just smoke` runs 01, §3.6's lock holds, and in-game captures show each effect on the accepted art; the plan is deleted with its citations and `e01sfx` is out of Now.

Item `e01sfx` in [ROADMAP.md](../../ROADMAP.md), from the [feedback](1.0/example-01-feedback.md)'s §3.10 with V27, V30, V31, V33, V37, V38, Q57–Q60 and K29. Written 2026-10-27 at `a5e5e18` on the `0.110` branch, after `e01foes`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs: Steps 1–7 are one code session at xhigh that builds on Step 1's placeholders and draws nothing; Step 8 is an asset session on Opus 5.5 at max on §3 alone, which ends when the owner accepts its art and sound; Step 9 closes in a new code session. A run session stops before Step 8 unless its evidence row is in, ending with §3.7's prompt.

## Context digest

- The owner's fourth playtest (2026-10-25) heard sounds from across the map at full volume, a thin lightning, a flat shock, silent burning and damage ticks that pass unseen, crates that only burn and sprays that splash walls with nothing; §3.10 sketched the stop, Q57–Q60 hold its defaults, K29 its one kit gap.
- It stands on the kit's cues (`D-172`'s set, the `cue_rules` kind), core's voices and their volumes (`D-179`), 01's `Sounded` events and loops, M81's crates and impacts (`D-210`), M82's spray bursts, e01foes' schema 17 and its asset loop.
- `D-215`: a commit per step, one critique round, the asset loop, the tiers (`D-151` as amended): art and sound on Opus 5.5 at max, code at xhigh. The brief's contract is what the code reads, so the accepted art touches no code. Level C and its API sketch are retired ([operating rules](../operating-rules.md#freeze-process-removed)), so §3.10's re-rating has nothing to do; §2.2 is the listener's API.
- The code comes first because §3.5's review sheet needs real loops of the planks, the bursts and the edge (Q1).
- Later stops expect: (o) and (p) place their sounds through the same listener; 7.9 adds pan and looping cue voices on top of it.

## 1. Audit

Read at `a5e5e18`. 01's paths are under `examples/01_platformer/`; `features/` is its `src/features/`; the kit's are under `crates/tungsten-kit/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The kit's `sound` action checks its `min_interval` on the cue clock, looks the sound and its manifest volume up and plays `play_with(handle, volume, false)`, then stamps `played`; the occurrence's `position` is never read (K29). An occurrence carries the event's position or its entity's point | `react/cues.rs:1944-1963`, `:1135-1143`, `:2118-2207` |
| A2 | Skips trace a reason string (`NO_SOUND`, `TOO_SOON`); the reactor keeps `clock` and `played` | `react/cues.rs:1700-1701`, `:1708-1741` |
| A3 | Exports: the cue items at the root and in the prelude; the cue tests run as `react::cues::tests`, and the interval test reads `AudioCommands::drain` for a play's volume | `lib.rs:296`, `:313-314`; `react/cues.rs:2613-2615`; `tests/cues.rs:725-745` |
| A4 | `SoundOnEvent<E>` picks a sound per event, with no position | `present/sound.rs:59-62` |
| A5 | The kit hash is pinned at `0x3334_9665_de0c_501a`; no recipe could insert a listener | `crates/tungsten/src/tests/kit_recipes/determinism.rs:36` |
| A6 | Core has a volume per play and per voice, no listener or pan | `crates/tungsten-core/src/audio.rs:144-146`, `:198-205` |
| A7 | 01's one-shots: `audio::play` sends `Sounded { sound, at, subject }` while the run's player lives, each a cue event by `occurrence::<N>`, played by `cue_rules/sounds.json` (thunder's interval 0.1). All 85 `audio::play` calls pass a world point, none a placeholder | `features/audio/mod.rs:77-80`, `:82-165`, `:170-178`; `tools/content/data/cue_rules/sounds.json:234-248` |
| A8 | Loops: a run's `audio::start`, after `save::restore`, plays the hums, the crackle, the combat layer and the ambience silent; `hum`, `holes` and `crackle` set their voice to `level × (1 − d / reach)` by the nearest source to the player, reaches 600, 900 and 600, over `HUM_FADE`; the crackle's sources are lit `Fuel` alone, the crates. `stop` ends them; the run's exit calls it | `features/audio/systems.rs:81-118`, `:126-157`, `:163-196`; `features/audio/mod.rs:37`, `:42`, `:46`, `:182-238`; `features/run/mod.rs:107-128` |
| A9 | The sprays' and the small balls' loops are the caster's, always the player | `features/toys/systems.rs:1294-1303`; `features/spells/systems.rs:210`, `:347`; `features/toys/mod.rs:324` |
| A10 | Tests on audio: the holes' hum on one voice (above 0 near, 0 once closed); tests reading `AudioCommand::Play` | `tests/game/spells.rs:393-438`; `ice.rs:302`, `:390`; `presentation.rs:969`; `enemies.rs:3234`; `hazards.rs:66`; `spawning.rs:206`, `:245`; `brick.rs:413` |
| A11 | A view at zoom 1 is 18 rows of 64 px, so 2,048 × 1,152 at 16:9; the Zoom option multiplies it | `features/level/mod.rs:53-54`; `features/level/systems.rs:32-39` |
| A12 | `bolt_end` falls from the level's top to the first tile or body its ray meets, a static collider's top, or a live enemy's top within its kind's strike width, else the level's foot; it returns a point. Callers: `strike` and one test | `features/lightning/mod.rs:160-203`, `:212`; `tests/game/lightning.rs:161` |
| A13 | `show` spawns the bolt, its light and shake, bursts `ex10_strike_sparks` and plays `Snapped` then `Thundered` at the landing, whatever it met; the thunder is 1.8 s at 0.75, the snap 0.3 s | `features/lightning/mod.rs:283-313`; `tools/sfx.py:463`, `:1700`; `tools/sounds.json:150-153`, `:367-370` |
| A14 | A jump's `conduct` draws an `Arc` and plays `Crackled`; `lightning_age` ends bolts and arcs alike at `BOLT_SECS` 0.15; an arc's zigzag is fixed by its seed | `features/lightning/mod.rs:53`, `:321-340`; `features/lightning/systems.rs:12-45`; `features/lightning/components.rs:18-25` |
| A15 | The strike test strikes iron (a body): one `Thundered`, the jump's arc and one `Crackled`, then bolt and arc gone 15 frames on | `tests/game/lightning.rs:328-405` (`:387`, `:400`) |
| A16 | Shock's look: `SHOCK_TINT` on `flicker`'s frame 0 at `FLICKER_HZ` 12, shared with burning's embers; the overlay `ex10_status_crackle_{class}_{0,1}` by `flicker`; status particles `ex10_shock_sparks`, 14 a second, 12 alive. The generator draws two crackle frames per class (32, 64, 128) and `art.py` sizes them | `features/combat/systems.rs:184-226`; `features/combat/draw.rs:190-193`; `features/combat/components.rs:56`; `tools/generate.py:323-327`; `tools/effects.py:1043-1049`; `tools/art.py:24-30` |
| A17 | `player_hurt`: a non-periodic hit sets `Moves::hurt` (0.12 s), the hurt clip and `PlayerHurt`, a killing one `PlayerDied`; ticks show the flash alone (K1, X1). `Moves` is presentation, its timers decayed together; nothing saves it | `features/player/systems.rs:57`, `:317-349`, `:467`; `features/player/components.rs:55-70`; `tests/game/player.rs:442` |
| A18 | Overlays over the view: `level::draw::view` and the veil's quad from the tracer's texel; the post stack's passes carry no identity | `features/level/draw.rs:125-140`; `features/lightning/draw.rs:37-60`; `features/level/mod.rs:275-291` |
| A19 | Burning: crates' `Fuel`, small balls' `BallBurn`, targets' `status.burning` tag | `features/props/components.rs:26-35`; `features/elements/components.rs:10`; `features/combat/mod.rs:59` |
| A20 | Props: four `PropKind`s, their tables, `spawn`, `restore` and the rock's cap pattern; `light`, `put_out`, `burning` and `tick` read `Fuel` on any prop; `heat`'s `bodies` takes crates and kegs, `look` and `draw::effects` crates. A crate weighs 12, its half 28 | `features/props/components.rs:7-24`; `features/props/mod.rs:47-48`, `:124-298`, `:349-477`; `features/props/systems.rs:56-98`, `:136-160`, `:200`, `:314-336`; `features/props/draw.rs:43`; `tools/content/prefabs/crate.json:21` |
| A21 | A crate breaks only by burning out. Frozen iron's sixteen scraps, thrown from a grid with a `ScrapTumble`, are the model | `features/props/systems.rs:89-91`; `features/elements/systems.rs:1441-1471`; `features/toys/components.rs:70-85` |
| A22 | `body_impacts` reads the step's collision events at a closing speed of `SHATTER_SPEED` 600 or more; a keg goes off by `blows_keg(weight, closing)`, `weight` the other's mass when it is dynamic, else infinite; `moves` is "dynamic", `before_step` the speed before the step; thrown bodies are `Throws` entries. Masses: a bowling ball 8 (tossed at 700 px/s), a keg 16, scrap 25, iron 400 | `features/elements/systems.rs:980-1001`, `:1148-1343` (`:1165-1169`, `:1190-1206`); `features/elements/mod.rs:66-93`, `:209-212`; `features/toys/mod.rs:54`; `tools/content/prefabs/*.json` |
| A23 | Saves: schema 17, arms 7–16 ending in `to_17`; `snowballs` a list of its own, skipped when empty; `bodies()` reads `props` by `S15_PROPS`' kinds and errors on another, and the relabel tests feed current captures through it; `PropSave`, its capture and `restore_prop`'s match on the kind | `src/save.rs:79-133`, `:221-226`, `:264-366`, `:584-640`, `:1294-1306`, `:1460-1464`, `:1562-1590`, `:2013-2049` |
| A24 | Save tests: the coverage test over `busy()` and `lists`; a full pit within 2 MiB; the 0.109 save test's pattern (a current capture relabelled) | `tests/game/saves.rs:21`, `:156`, `:307`, `:752`, `:3519-3550` |
| A25 | Sprays: `spray`'s rays keep each body met once (`met`), a tile stop being a ray's point with no body; every second `SPRAY_PULSE` each live target met bursts `ex10_frost_hit` or `ex10_flame_hit` at its contact; other bodies chill or catch. `contact_point` gives the splash emitter the nearest stop short of the reach, iron first | `features/toys/systems.rs:74-75`, `:1106-1226` (`:1150-1182`), `:1384-1446` |
| A26 | The spray test: the dummy's hit burst within 18 frames at its face; "a wall bursts nothing" | `tests/game/spells.rs:677-728` (`:706-727`) |
| A27 | `Sound::ALL` holds 81, `cue_of` to `occurrence::<80>`; `data.rs` checks every event has a rule whose sound is in the manifest | `features/audio/components.rs:212`; `features/audio/mod.rs:82-165`; `tests/game/data.rs:206-222` |
| A28 | The generator owns `assets/` byte for byte, new sprites shift `level.tmj`'s tile IDs, and it runs here only in a scratch Pillow venv; `just level-check` runs the check and `tools/test_*.py`, the foes' contract test among them. Particle configs in `generate.py`, sounds in `sfx.py` (the crackle's loop 2 s) and `sounds.json`, the crate's art in `props.py`. Caps today: `strike_sparks` 32, `frost_hit` 16, `flame_hit` 18, `crate_collapse` 20 | `justfile:96-98`; `tools/test_generate.py:269-330`; `tools/generate.py:248-495`; `tools/sfx.py:2502`, `:2554-2606`; `tools/props.py:51-329`; `assets/particles/*.json` |
| A29 | Sound lengths today: `fizzle` 0.35 s, `gun_click` 0.12, `crate_collapse` 0.80, `fire_crackle` 2.00, `thunder` 1.80 | `assets/sounds/*.wav`, measured |
| A30 | Bursts: at most `BURST_CAP` 16 cosmetic ones; a key burst takes a cosmetic one's slot | `src/shared.rs:73-75`, `:100`, `:112` |
| A31 | 01's README rows (sounds, crates and kegs, lightning, saves); `DESIGN.md`'s workspace line and its cues paragraph | 01's `README.md:135`, `:144`, `:148`, `:169-171`; `DESIGN.md:5`, `:563` |
| A32 | Speculative contacts stop a fast body at touching, and a collision event needs positive penetration, so the step a body arrives in can send none; `target_hits` tests touch at the step's end for that reason. Iron falling faster than 30 px/s falls by another 1,800 px/s² | `crates/tungsten-core/src/physics/step/mod.rs:9-17`, `:48-58`; `features/elements/systems.rs:1042-1049`; `features/toys/mod.rs:64-68`; `features/toys/systems.rs:195-205` |

## 2. Design

### 2.1 Scope

The kit gains one opt-in resource (§2.2), off by default, so no recipe and no other game hears a change and the kit hash stands (A5). The rest is 01's game Rust and data: no core, render or template change, so the physics hashes and the visual references stand. Per-step costs are a distance per cue sound, one nearest-burning scan a frame over the tagged bodies and the props, and at most 40 more props in `props_heat`, so no capture is planned. New `Sound` variants land with their first use.

### 2.2 The kit's listener (K29, Q57)

In `react/cues.rs`, exported at the root and in the prelude:

- `CueListener`, a World resource: `CueListener::new(full, silent)`, which panics unless both are finite and `0 ≤ full < silent`; `position()`, `set_position(Vec2)` and `at(Vec2) -> Self`; `full()`, `silent()`; and `gain(at: Vec2) -> f32`: 1 within `full` of the position, 0 from `silent`, `(1 − t)²` between, `t` the distance's share of the way from `full` to `silent`.
- The `sound` action, with a `CueListener` in the World: after the interval and the lookup, the gain at the occurrence's position; at 0 it skips with `UNHEARD` ("the cue's position is past the listener's hearing") and leaves `played` unstamped, so a nearer cue may still play within the interval; else it plays at volume × gain. Without one, as today.
- No component, scene field, description or inspector row, so `api/tungsten-kit-components.txt` and the description counts stand; `SoundOnEvent` unchanged (A4).
- Tests (`react::cues::tests`): a listener with full 100 and silent 300 at the origin: a cue sound at 50 px plays at its volume, at 200 at a quarter of it, at 300 and at 500 nothing, each with the `UNHEARD` skip; a far cue then a near one inside the interval: the near one plays; `set_position` moves the hearing; `new` panics on `full ≥ silent`, a negative and a NaN; `gain` is 1 at `full`, falls monotonically and is 0 at `silent`. The existing interval test stands for the listener-less path. `DESIGN.md`'s cues paragraph (`:563`) gains a sentence, and its workspace line (`:5`) a clause.

### 2.3 01 hears by distance (V38, V31; Q57, Q58)

- `audio::LISTEN_FULL` 1,024 px (half a view's width at zoom 1) and `LISTEN_SILENT` 3,072 (one and a half views), world distances that the Zoom option leaves alone (A11, Q4).
- `audio::start` inserts `CueListener::new(LISTEN_FULL, LISTEN_SILENT)` at the player's centre; `stop` removes it. `audio_listen`, in the react phase before the kit's `CUES`, sets its position to the live player's centre each step (`enemies::systems::player`).
- `hum`, `holes` and `crackle` set their voice to `level × gain(nearest)` from the listener, their reaches (`HUM_REACH`, `HOLE_HUM_REACH`, `CRACKLE_REACH`) gone; without a listener, silent. The sprays' and the small balls' loops sit at the player, so they stay at their level; the ambience, the music and the combat layer stay unplaced (Q5).
- The crackle's sources: lit `Fuel` (crates, then planks), small balls with a `BallBurn` left, and every live entity tagged `status.burning`, the player, the dummy and the enemies (A19, Q12).
- Tests (`tests/game/audio.rs`, `audio::`): a `Sounded` at 500 px plays at its rule's volume, one at 2,000 at volume × gain, one at 3,500 plays nothing, and the player's own at full; a lit crate 300 px off raises the crackle to its level, and moving the player 2,000 px off during the loop sets it to level × gain, past 3,072 to 0; a burning dummy alone, a burning small ball alone and a burning player each raise it; a load re-inserts the listener at the loaded player. The holes' test (`spells.rs:393-438`) stands.

### 2.4 Lightning (V27)

- `bolt_end` returns a `Landing { at, met }`, `met` a `Met`: `Ground` for a tile, a static collider's top or the level's foot; `Deck(entity)` for a body neither dynamic nor an enemy, which moves (a deck, the elevator); `Body(entity)` for a live enemy within its strike width or a dynamic body (Q7). `strike` takes `at`; the test caller reads `.at`.
- `show` on a body: today's look and sounds. On the ground or a deck: `ex10_ground_sparks` in place of `ex10_strike_sparks`, `Sound::GroundStruck` in place of `Snapped`, then `Thundered`. On the ground alone, a scorch: `Scorch { age }` on an `ex10_scorch` sprite whose bottom row's middle sits on the landing, under the bodies, fading over `SCORCH_SECS` 5 in `lightning_age`; at most `SCORCH_CAP` 6, the oldest going; presentation, never saved. A deck takes none, since a mark left at a point would hang in the air once it moves.
- The thunder's depth and its later rumble are the sound's redraw (§3), its ID and rule kept, with no timer (Q8).

### 2.5 Shock (V30; Q9)

- The overlay takes four frames, `ex10_status_crackle_{class}_{0..3}`, by `shock_frame(time)` at `SHOCK_HZ` 16; burning keeps `flicker` at 12.
- The flicker: `SHOCK_TINT` deepens to [196, 168, 255] and shows on the overlay's even frames.
- `ex10_shock_sparks`' brightness is its config's redraw (§3).
- Arcs show for `ARC_SECS` 0.35, their zigzag's seed stepping every `ARC_JITTER` 0.05 s so they crackle, their glow fading over the last third; bolts keep `BOLT_SECS` 0.15.

### 2.6 The damage edge and tick (V31; Q10, Q11)

- `Moves::edge`, a share from 0 to 1 that falls to 0 over `EDGE_SECS` 0.4 with the other timers. `player_hurt`: a blow sets it to 1, as now with its tint, clip and hurt sound; a periodic hit that does not kill sets it to at least `TICK_EDGE` 0.5 and plays `Sound::PlayerTicked` at the player (its rule's interval 0.15 s), once a step however many ticks land (K1 stays).
- `combat::draw::edge` draws `ex10_hurt_edge` stretched over `view(world)`, as the veil is (A18), at alpha `edge` × 255, registered beside the overlays; not a post pass, since passes have no identity.

### 2.7 Planks (V33; Q13, Q14)

- **The break.** Not from collision events, which a speculative contact can withhold for the step a body arrives in (A32), but by a touch test as `target_hits` makes for live targets: `crate_hits`, in `body_impacts`, takes each crate not frozen, in save-ID order, and the dynamic bodies whose box meets its own grown by `TOUCH_MARGIN` 2 px at the step's end, each with its own speed toward the crate along the line between their centres and the pair's closing speed on that line, both from the speeds before the step. A crate breaks when the hitter's speed and the closing speed are both at least `BREAK_FLOOR` 300 px/s, so resting contacts and bodies moving together break nothing, and the hitter's mass times its speed reaches `CRATE_BREAK` 5,600 (a bowling ball at its 700 px/s toss, a crate at 467, a keg at 350, scrap and iron at the floor); or when the crate is a throw (a troll's toss) moving at `SHATTER_SPEED` or more before the step and touching a tile or a body after it, its thrower aside, as `target_hits` spares a thrower. A crate dropped on the ground stays whole; a frozen crate shatters as today. Each crate breaks once a step, in save-ID order, last in `body_impacts`, after its hits and landings, so a moving crate's own blow lands first.
- **`props::break_crate`**: the crate goes with `ex10_plank_splinters` (a key burst) and `Sound::CrateBroke`; five planks take its place, one per row of its box from the top, `PLANK_ROWS` y −20, −10, 0, 10, 20 from its centre, each at the crate's velocity plus `PLANK_KICK` 260–420 px/s away from its centre (by row, no stream draw), less 180 up, plus a quarter of the hitter's velocity, with a tumble of whole half-turns over its flight (`ScrapTumble`'s shape, turned in `props_look`), so it lies flat. A burning crate's planks burn with its fire left, at most `PLANK_BURN_SECS` 4, crediting its lighter; a doused one's keep the douse.
- **`PropKind::Plank`** with `Plank { variant }` (`plank` prefab: sprite `ex10_plank_0`, a dynamic box of half (14, 4) about (16, 8), mass 2), its variant its row; `kind`, `all`, `half`, `centre`, `prefab`, `cap` and `rolls` gain the arm, so sprays, blasts and fire treat planks as props. `heat`'s `bodies` takes planks; `look` chars them as crates over `PLANK_BURN_SECS`; `draw::effects` gives a burning plank one flame and a small glow; a plank burnt out falls in quietly with `ex10_collapse_embers`. `PLANK_CAP` 40: past it the oldest, the lowest save ID, crumbles quietly with `ex10_plank_splinters`, as rocks do.
- **Schema 18.** `SaveGame::planks` (`PlankSave`: id, variant, position, velocity, chill, frozen, fuel; `#[serde(default, skip_serializing_if = "Vec::is_empty")]`), not `props`, since `bodies()` refuses a kind `S15_PROPS` lacks (A23). `capture` leaves planks out of `props`; the load restores them with the props. `SCHEMA` 18; arms 7–16 unchanged, `17 => Ok(data)`.

### 2.8 Spray impacts (V37; Q15)

Every second pulse (0.2 s), as M82's bursts: each live target met bursts the element's hit as today; every other body met (props and planks, balls, iron and scrap, frozen bodies, a lava basin) bursts the element's surface hit, `ex10_frost_wall_hit` or `ex10_flame_wall_hit`, at its contact; and the nearest ray stop on a tile short of the reach bursts it once a pulse. Cosmetic bursts, so the cap may drop them (A30). M82's "a wall bursts nothing" is reversed.

### 2.9 Sounds and cues

Each variant appended to `Sound::ALL` at its step with its event, the next `occurrence::<N>` in `cue_of` and a rule naming §3.1's ID (A27); Step 1's placeholders put each sound in the manifest first.

| Variant | Event | Sound | Interval | Step |
| --- | --- | --- | --- | --- |
| `GroundStruck` | `ground_struck` | `ex10_ground_strike_sfx` | 0.1 | 4 |
| `PlayerTicked` | `player_ticked` | `ex10_player_tick_sfx` | 0.15 | 5 |
| `CrateBroke` | `crate_broke` | `ex10_crate_break_sfx` | 0.08 | 6 |

`ex10_thunder_sfx` and `ex10_fire_crackle_sfx` keep their IDs, rules and lengths' roles, redrawn (§3.1).

## 3. Asset brief

The redo's [§6](roadmap-redo.md#6-asset-sessions) brief for this stop's families. Step 1 makes every file below from today's art; Step 8 draws each in its place with the generator's own toolkit, within this contract, and touches no `src/` or `tests/` file.

### 3.1 Families and contract

Generator names; the manifest prefixes `ex10_`. The planks and the scorch are lit as the crates are (the generator's normal and emissive maps); the overlays and the edge are unlit, drawn over the lit world. Particles draw at z 0, under the sprites, so bursts launch out past the body.

| Family | Files | Size, frames, caps | Anchor and contract | The code's use |
| --- | --- | --- | --- | --- |
| Shock overlay | `status_crackle_{small,medium,large}_{0..3}` (from 2) | 32, 64, 128 square; 4 frames | Over the body's frame as today's two | §2.5 |
| Shock sparks | particles `shock_sparks`, redrawn | Continuous, at most 24 alive | — | §2.5 |
| Ground strike | particles `ground_sparks`; sprite `scorch` | A burst of at most 32; 48 × 16 | The scorch's bottom row's middle (24, 15) on the landing | §2.4 |
| Planks | `plank_0`–`plank_4`; particles `plank_splinters` | 32 × 16 each; a burst of at most 20 | The board within the body's box, 28 × 8 about (16, 8) | §2.7 |
| Surface hits | particles `frost_wall_hit`, `flame_wall_hit` | Bursts of at most 18 | From the contact | §2.8 |
| Hurt edge | `hurt_edge` | 320 × 180 | Clear from a quarter in; the code sets its alpha | §2.6 |
| Sounds | `ex10_<id>_sfx` at `sounds/<id>.wav`: `ground_strike`, `player_tick`, `crate_break` new; `thunder`, `fire_crackle` redrawn | The ground strike 0.3–1.0 s; the tick at most 0.2 s; the break 0.25–0.9 s; the thunder 1.6–3.6 s; the crackle a 2.0 s loop; volumes: the tick 0.2–0.4, the others 0.35–0.8 | — | §2.9 |

### 3.2 Art direction

From the owner's reviews since M75: detailed, characterful passes in the house style (palette pixels, outlines, moonlit top-left edges), effects that read as their names and mark hits, motion shown as real in-game loops; drawn with the generator's toolkit, never pixel by pixel.

- **Shock (V30).** Crackling arcs over the body: jagged white-cored, violet-glowing lines leaping between points on its edge, a new path each frame, a few bright nodes where they land; the sparks white-hot at birth, fading to violet, quicker and more of them.
- **The ground strike (V27).** Sparks bursting low and wide along the ground, a few rising, white to violet; the scorch a charred starburst on the ground's top with cracks radiating and a faint violet ember at its heart, which the code fades.
- **Planks (V33).** Five boards from the pine crate's own planks (its grain, edges and nail holes, both variants' woods), each its own: a broken end, a split, a bent nail; the splinters pale fresh wood with a few dark chips.
- **Surface hits (V37).** Frost: steam puffs and frost chips, white and pale blue, subtle as M84's frost; fire: sparks with a puff of soot that marks the stone.
- **The hurt edge (V31).** A soft red glow at the screen's edges, deeper at the corners, clear from about a quarter in, with a little grain so it reads as pixel art.
- **Sounds** (redo Q5: a playlist with notes). The thunder deeper: a sharp crack into a long low roll, a distant rumble about 1.5 s in. The ground strike an electric crackle and sizzle, a buzzing snap with a hissing tail. The crackle a fire's pops and hiss for anything alight, a seamless loop. The tick a soft muted thud, apart from the hurt. The break a wooden crack into a splintering clatter.

### 3.3 Values the code takes

None from the art: the contract fixes the sizes, counts, caps and lengths the code reads. Step 8 records in its evidence row the scorch's bottom row against (24, 15) and each sound's measured length.

### 3.4 Placeholders (Step 1)

Every §3.1 file comes from today's art at the contract's size, count and cap, by copying, cycling or resizing, never drawn: the crackle's frames 2 and 3 copies of 0 and 1; `shock_sparks` as today (12 alive); `ground_sparks` a copy of `strike_sparks`; `scorch` today's `pine_crate_char_2` resized; the planks the pine crate resized, its two variants in turn; `plank_splinters` a copy of `crate_collapse`; the surface hits copies of `frost_hit` and `flame_hit`; `hurt_edge` `shock_ring` resized; the sounds today's, measured (A29): the ground strike `fizzle` (0.35 s), the tick `gun_click` (0.12), the break `crate_collapse` (0.80), thunder and crackle themselves. `tools/test_generate.py` gains `EffectsContractTests` reading the generated `assets/`: every §3.1 sprite at its size, four crackle frames per class, each particle config's cap, each sound's length and volume in range. Today's files and the placeholders pass it, and so must the art.

### 3.5 The review sheet

In `perf-runs/<date>-e01sfx-art-review/` (machine-local), linked in the conversation at each round, the contact sheet's link first:

- A contact sheet at 4×: every §3.1 sprite, today's crackle frames beside the new four, the planks with the crate they came from.
- Animated previews of the crackle per class and each particle config.
- In-game loops from a scratch driver under `target/` (deleted after; the in-game shots recipe, one capture job at a time, the shell's pause off in the copy): a shocked goblin and the dummy, and a chain's arcs; a strike on the ground and on an enemy; both sprays on a wall, a crate and the floor; a bowling ball breaking a crate and its planks burning down; the wizard burning, the edge pulsing.
- A playlist with notes: the thunder before and after, the ground strike, the crackle before and after, the tick and the break, beside `thunder_snap.wav` and `crackle.wav`.

### 3.6 Lock

On acceptance, Step 8 writes one `sha256sum` line per file §3.1's families own (sprites, particle configs, sounds), paths from the repository root, between these fences. The check, `sed -n '/^```sha256$/,/^```$/p' docs/plans/e01-sfx.md | sed '1d;$d' | sha256sum -c --quiet`, exits 0 only when every file is unchanged, and 1 while the block is empty.

```sha256
```

### 3.7 The asset session's prompt

A run session that reaches Step 8 without its evidence row ends with this, for a new session on Opus 5.5 at max:

`Use the tungsten-milestone skill. Read docs/plans/e01-sfx.md and run Step 8 alone, the asset session: draw §3.1's families in place of Step 1's placeholders as §3.2 directs, within the contract, with the generator's own toolkit; regenerate; link me §3.5's review sheet with in-game loops and take my notes in rounds. On my acceptance write §3.6's lock and the evidence row, delete the scratch driver and the venv, commit and push under docs/operating-rules.md. Leave src/ and tests/ alone; end with the run prompt for Step 9.`

## 4. Steps

Every code step's gate also takes `cargo fmt --all -- --check` → exit 0, `cargo clippy -p <crate> --all-targets --locked -- -D warnings` → exit 0 for each crate it changes, and `cargo test -p example-01-platformer --locked` → pass (01's whole suite, its count in the evidence). A step that regenerates `assets/` makes a scratch Pillow venv once a session (`python3 -m venv "$VENV" && "$VENV/bin/pip" install -r examples/01_platformer/tools/requirements.txt`, `$VENV` in the scratchpad, removed with `shutil.rmtree` at the session's end) and passes `PATH="$VENV/bin:$PATH" just level-check` → pass, about five minutes, run in the background with the step's paths staged first. Filtered runs name the step's new tests in the evidence, since a filter that matches nothing still exits 0. Commit and push in separate calls.

### Step 1: Placeholders for §3's contract

- **Files:** Step 1's in the header.
- **Change:** §3.4: every §3.1 file from today's art (sprites where the generator keeps its effects and props, the configs in `particle_configs`, the sounds in `build_sounds` and `sounds.json`, the crackle's frames in `art.py`'s sizes); the contract test; a line in `tools/README.md` naming them placeholders until Step 8; regenerate.
- **Done-when:** `PATH="$VENV/bin:$PATH" just level-check` → pass, `EffectsContractTests` among its tests; `cargo test -p example-01-platformer --locked` → pass (no code change); `just repo-check` → pass; `git status --porcelain` → only Step 1's paths.
- **Commit:** `feat(example-01): placeholders for the sounds' and effects' art`
- **Moves:** `assets/tilemaps/level.tmj`'s tile IDs, which new sprites shift; the manifest.

### Step 2: The kit's listener (K29)

- **Files:** Step 2's in the header.
- **Change:** §2.2, rustdoc on every new public item; `just api`.
- **Done-when:** `cargo test -p tungsten-kit --locked` → pass, with §2.2's tests among `react::cues::tests`; `cargo test -p tungsten --lib --locked kit_recipes::determinism` → 1 passed (the pin unmoved); after `just api`, `git diff --name-only -- api/` → `api/tungsten-kit.txt` alone, and `just api-check` → pass; `just docs-ratchet` → pass; `just repo-check` → pass; the gate's clippy for `tungsten-kit`.
- **Commit:** `feat(kit): a cue listener that quiets cue sounds by distance`
- **Moves:** `api/tungsten-kit.txt`.

### Step 3: 01 hears by distance (V38, V31's crackle)

- **Files:** Step 3's in the header.
- **Change:** §2.3; 01's README sounds row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- audio:: spells::` → pass, with §2.3's tests among `audio::`; `the_holes_hum_on_one_voice_that_falls_silent_when_the_last_closes` unchanged and passing; `rg -q 'HUM_REACH|CRACKLE_REACH' examples/01_platformer/src; test $? -eq 1` → exit 0; the gate.
- **Commit:** `feat(example-01): sounds fall off with their distance from the player`

### Step 4: Lightning and shock (V27, V30)

- **Files:** Step 4's in the header.
- **Change:** §2.4, §2.5, `GroundStruck` (§2.9); the strike test's end moves to the arcs' 0.35 s (bolt gone after 0.15 s, arc after 0.35); 01's README lightning row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- lightning:: presentation:: data::` → pass, with new tests that a strike on bare ground meets `Ground`, plays `GroundStruck` and `Thundered` and no `Snapped`, bursts `ex10_ground_sparks` and leaves one scorch faded out by 5 s, at most six left after eight strikes; that one on a live enemy and one on iron meet `Body` and keep `Snapped`; that one on a deck meets `Deck`, sparks and sounds as the ground does and leaves no scorch; that a shocked dummy's overlay steps through `ex10_status_crackle_medium_0`–`_3` within 0.25 s with `SHOCK_TINT` on half the frames; and that a jump's arc shows at 0.3 s and is gone after 0.35 s with its zigzag changed between; `a_strike_shows_its_bolt_flash_light_and_shake` passing with its moved end; the level-check gate.
- **Commit:** `feat(example-01): lightning strikes the ground and shock crackles`
- **Moves:** the strike test's arc end.

### Step 5: The damage edge and tick (V31)

- **Files:** Step 5's in the header.
- **Change:** §2.6, `PlayerTicked`; 01's README row for the player's feedback.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- player:: data::` → pass, with new tests that a burning player's tick sets the edge to at least 0.5, draws `ex10_hurt_edge` over the view and sends one `PlayerTicked` with no `PlayerHurt`, that a blow sets it to 1 with `PlayerHurt` and no `PlayerTicked`, and that the edge is gone 0.4 s after; `a_blow_tints_the_courier_red_for_a_moment` unchanged and passing; the level-check gate.
- **Commit:** `feat(example-01): damage ticks pulse the screen's edge`

### Step 6: Planks (V33, schema 18)

- **Files:** Step 6's in the header.
- **Change:** §2.7, `CrateBroke`; `data.rs` pins the plank's body; the coverage test lists `Plank` as saved; `busy()` breaks a crate; 01's README crates and saves rows.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- props:: saves:: data::` → pass, with new tests that a tossed bowling ball breaks a crate into five planks of mass 2 with one `CrateBroke`, from either side and from three starting gaps; that iron dropped 16 px onto a crate (its speed the step before the touch asserted between 300 and 600 px/s, with iron's weight, A32) breaks it and a keg rolled into one at 400 px/s does too, while a bowling ball at 500 px/s, iron resting against one and iron falling at 400 px/s with a crate it touches break nothing; that a crate the test throws (a `Throws` entry) breaks on landing at 700 px/s while one dropped 150 px stays whole; that a frozen crate shatters into no planks; that a burning crate's planks burn and fall in by `PLANK_BURN_SECS`; that nine crates broken leave 40 planks, the oldest gone; that a run with burning planks saves through JSON and loads them back; and that a schema 17 save from the 0.110 code (a current capture without `planks`, numbered 17) loads; the full pit's save within 2 MiB; the level-check gate.
- **Commit:** `feat(example-01): crates break into planks under a heavy hit`
- **Moves:** 01's save schema, 17 to 18.

### Step 7: Spray impacts on every surface (V37)

- **Files:** Step 7's in the header.
- **Change:** §2.8; the wall half of `the_sprays_reach_further_and_burst_on_what_they_hit` reversed; 01's README row.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- spells::` → pass, with the wall half asserting a surface hit (`ex10_flame_wall_hit`, `ex10_frost_wall_hit`) within 24 px of the wall's face within 18 frames and no target hit, and new tests that a crate met bursts the surface hit and the dummy keeps its hit; the gate.
- **Commit:** `feat(example-01): the sprays burst on whatever they meet`
- **Moves:** M82's "a wall bursts nothing".

### Step 8: Art and sound (the asset session)

- **Who:** a new session on Opus 5.5 at max, on §3 alone, after Steps 1–7 (Q1); the owner reviews.
- **Files:** Step 8's in the header.
- **Change:** draw §3.1's families as §3.2 directs, in place of the placeholders; the five sounds in `sfx.py`, their volumes in `sounds.json`; `tools/README.md`'s rows; regenerate; §3.5's sheet; the owner's notes in rounds, a new small session when the context grows; on acceptance §3.3's measurements and §3.6's block, the evidence row, the scratch driver and the venv deleted.
- **Done-when:** the owner's acceptance quoted in the evidence row; `PATH="$VENV/bin:$PATH" just level-check` → pass, the contract test among its tests; `cargo test -p example-01-platformer --locked` → pass; `just repo-check` → pass; §3.6's check → exit 0; `git status --porcelain` → only Step 8's paths.
- **Commit:** `feat(example-01): the sounds' and effects' art`

### Step 9: Close

- **Files:** Step 9's in the header.
- **Change:** run the checks and the demo; in-game captures of §3.5's scenes on the accepted art, by its driver, for the owner; finish 01's README (the rows and the kit-items table's `CueListener`); `DESIGN.md`'s workspace line gains the sounds and effects in 01's clause; §3.10 of the feedback gains its landed note, as §3.9's has; open follow-ups to Later or known issues; `e01sfx` out of Now and `e01hollow` into it; §3.6's check, then this plan deleted with its citations; the venv removed.
- **Done-when:** §3.6's check → exit 0 before the deletion; `just check` → pass (formatting, lints and every crate's tests, 01's whole suite among them); `PATH="$VENV/bin:$PATH" just level-check` → pass; `just smoke` → pass; `just repo-check` → pass; the captures named in the last evidence row.
- **Commit:** `docs(example-01): records follow e01sfx`
- **Ends with:** the commits made, the evidence quoted, what the owner should play (a storm of strikes on the mine's floor and its goblins, a bowling ball into the crates, the flamethrower on a wall, a walk away from a burning crate, standing in a fire) and the next prompt.

## 5. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The sessions' order (§3.10 says the plan session draws the sheet first) | The code session runs Steps 1–7, then the asset session Step 8, since §3.5's loops need the planks, the bursts and the edge, then a new code session Step 9, as `e01foes` ran. This session wrote the brief and drew nothing |
| Q2 | The listener's shape | A World resource whose position the game writes each step: no component, scene field or description, and `SoundOnEvent` untouched |
| Q3 | The falloff's curve | `(1 − t)²` from `full` to `silent`; at 0 the play is skipped and its interval left free |
| Q4 | "Half a screen", "1.5 screens" | 1,024 and 3,072 px, from a view at zoom 1, fixed whatever the Zoom option |
| Q5 | Which loops follow the curve | The wisps' hum, the holes' hum and the crackle; the sprays' and the small balls' sit at the player; ambience, music and the combat layer stay unplaced |
| Q6 | A listener when no run plays | None: `stop` removes it, so nothing outside a run is quieted |
| Q7 | What lightning met | `Ground`: a tile, a static collider, the level's foot; `Deck`: a moving kinematic body, a deck or the elevator, the ground's look without its scorch; `Body`: a live enemy or a dynamic body |
| Q8 | The later rumble | Baked into the redrawn thunder, one clip, no timer |
| Q9 | Shock's numbers | Four frames at 16 Hz, `SHOCK_TINT` [196, 168, 255] on the even ones, arcs 0.35 s re-seeded every 0.05 s, sparks up to 24 alive |
| Q10 | The edge | A sprite over the view under the HUD: a blow full, a tick half, gone over 0.4 s |
| Q11 | The tick's sound | On any periodic hit on the player that does not kill, once a step, its rule's interval 0.15 s |
| Q12 | "The nearest burning thing" | Lit fuel (crates, planks), burning small balls, and live targets tagged burning (the player, the dummy, enemies) |
| Q13 | The break's bar (Q59) | A dynamic hitter's mass × its own speed toward the crate ≥ 5,600, that speed and the pair's closing speed each at least 300 px/s, by a touch test at the step's end rather than collision events; or a thrown crate moving at 600 px/s or more that ends the step touching anything; still ground and kinematic bodies break nothing; a frozen crate shatters instead |
| Q14 | The planks' numbers | Five, mass 2, a 4 s burn, a cap of 40 crumbling quietly, saved in their own list |
| Q15 | Surface bursts | One per non-live body met and one at the nearest tile stop each burst pulse, with surface configs apart from the hit configs |
| Q16 | The scorch | The ground strike's alone, 5 s, at most six, never saved; the fire spray's scorch is its burst's soot |
| Q17 | §3.10's re-rating | None: level C is retired (`D-215`); §2.2 is the API |
| Q18 | Crate on crate | Only the struck crate breaks, by the hitter's own speed, so a stack set down whole stays whole |
| Q19 | A decision entry for the listener | None: the operating rules give one only to a choice that constrains later work, as `e01tomes`' kit additions took none; DESIGN.md and 01's README carry it |
| Q20 | Placeholders | Copies of today's art at the contract's sizes, counts and caps, never drawn (§3.4) |

Each question takes its stated default; the owner reviews the plan on their own time.

## 6. Invariants and stop conditions

- **May move:** `level.tmj`'s tile IDs and the manifest (Step 1; Steps 4–6 their rules and prefab; Step 8 bytes only); `api/tungsten-kit.txt` (Step 2); 01's save schema, 17 to 18 (Step 6); `Sound::ALL`, 81 to 84; the strike test's arc end (Step 4); the spray test's wall half (Step 7).
- **Must not move:** the kit hash `0x3334_9665_de0c_501a`; the determinism and containment hashes and the row digests (no core change); `api/tungsten-kit-components.txt` and the description counts; `gpu-visual.png` and the post and transition regressions; 01's `src/shell/`, byte for byte the template's; the `S15_*` and `S16_*` tables; the bolt's 0.15 s, the strike's shock and reach; every `LAUNCHES` row; §3.6's files after Step 8.
- A done-when check fails: restore the step's files from the last commit, then `git add` each restored path and `git rm --cached` each file the step added, so the index holds no part of it (`git diff --cached --quiet` → exit 0, since §4 stages a step before its level-check); mark the step skipped and carry on with the steps that don't depend on it (2 needs nothing; 3 needs 2; 4–7 need 1; 8 runs on what landed, telling the owner which loops are missing; 9 needs 8's lock). A skipped step's sound leaves its index to the next.
- A test reading `AudioCommand::Play` fails in Step 3 because its source lies past the hearing: it moves its source or its player within `LISTEN_FULL`, recorded; a test that needs anything else skips the step.
- §3.6's check fails after a regeneration: an art file changed, which only an asset session may change; restore the step's files.
- Step 8 needs a contract change (a size, a cap, a length or a new ID the code reads): it records it in its evidence row and leaves `src/` alone; Step 9 takes a one-constant change, anything more goes to Later.
- A full pit's save passes 2 MiB, or the kit hash moves: the step restores and stops.
- A file outside the step's list needs a change: a test that pins a value the step moves is added and recorded; anything else skips the step.
- The Pillow venv will not install: stop, since the generator cannot run.

## 7. Critique

The `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-27, reading `a5e5e18` with this plan uncommitted, §7 and the log cut; round 1. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 397.5 s, tokens in 1036148 (cached 904448), out 9108 (reasoning 4951), commands 30, output 243 KB (largest 22.2 KB), usage 5h 0% → 22%, week 16% → 19%`.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] A failed step can remain staged after its rollback: §4 stages before the level-check, and `git show HEAD:<path> > <path>` restores the working tree alone | `.claude/skills/tungsten-milestone/SKILL.md:28`; §4's gate | Accepted | §6: the restored paths re-added and the step's new files `git rm --cached`, `git diff --cached --quiet` → exit 0 before the next step |
| 2 | [MEDIUM] The 600 px/s floor defeats the listed thresholds: `body_impacts` skips an event under `SHATTER_SPEED` before any per-pair rule | `features/elements/systems.rs:1165-1169`; `features/elements/mod.rs:93` | Accepted | §2.7, Q13: the break read before the floor with its own `BREAK_FLOOR` 300 px/s of the hitter's speed, so resting contacts break nothing; Step 6 tests iron at about 540 px/s and a keg at 400 breaking a crate, a ball at 500 and resting iron not |
| 3 | [MEDIUM] Scorches on moving platforms stay behind in mid-air: decks and the elevator move, and the scorch has no support | `tools/level.py:255-262` (kinematic decks on a ping-pong follower); `features/level/elevator.rs:243-247` | Accepted | §2.4, Q7: `Met::Deck` for a moving kinematic body, the ground's sparks and sound with no scorch; Step 4 tests it |

Round 1 changed mechanisms (the break's floor, the rollback, two done-when checks), so round 2 ran at xhigh with §7 and the log cut and a focus note of these changes. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 361.4 s, tokens in 679805 (cached 606080), out 8242 (reasoning 3988), commands 27, output 266.5 KB (largest 28.9 KB), usage 5h 22% → 40%, week 19% → 22%`. It found the deck and rollback fixes sound; each of its three was a gap the break's fix left, and none is open, so no question carries one. Sweeping the fixes against each other then found a thrown crate touching the troll that released it: §2.7's touch test passes over its thrower.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] Collision events can miss the impact that should break a crate: speculative contacts stop a body at touching with no event | `crates/tungsten-core/src/physics/step/mod.rs:9-17`, `:48-58`; `features/elements/systems.rs:1042-1049` (`target_hits` for the same reason) | Accepted | §2.7, Q13: `crate_hits`, a touch test at the step's end on the speeds before it; A32; Step 6 tosses the ball from three starting gaps |
| 2.2 | [MEDIUM] The hitter's own speed breaks a crate with no relative impact, as touching bodies moving together | `crates/tungsten-core/src/physics/step/mod.rs:876-892` (events on penetration alone) | Accepted | §2.7, Q13: the pair's closing speed at least `BREAK_FLOOR` too; Step 6 tests iron falling with a crate it touches |
| 2.3 | [MEDIUM] A 40 px iron drop lands near 650 px/s, not 540: falling iron gains `IRON_WEIGHT` | `features/toys/mod.rs:64-68`; `features/toys/systems.rs:195-205` (√(2 × 5,400 × 40) ≈ 657) | Accepted | Step 6: a 16 px drop (about 416 px/s), its speed before the touch asserted between 300 and 600 |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-27 | Passed | `level-check`: `OK: 5048 owned outputs match`, `Ran 31 tests … OK`, the three `EffectsContractTests` among them; 01's suite 379 passed, 1 ignored; `repo-check` exit 0; the index held Step 1's paths alone. 25 sprites (six crackle frames; five planks and the scorch, each with its two maps; the edge), 4 particle configs and 3 sounds added. The planks are each crate shrunk to the 28 × 8 board inside its 32 × 16 frame, so the placeholders keep §3.1's anchor too; `art.build_art` runs `effects.build_sfx_art` after `enrich_art`, which draws the shock ring | `examples/01_platformer/tools/`, its `assets/` |
| 2 | 2026-10-27 | Passed | `cargo test -p tungsten-kit --locked`: 783 passed, the six new among `react::cues::tests` (`a_listener_hears_in_full_within_full_then_falls_to_silence_at_silent`, the three `a_listener_…_panics`, `running::a_listener_quiets_cue_sounds_by_distance_and_skips_those_past_its_hearing`: 1.0, 0.25, then two `UNHEARD` skips; `running::an_unheard_cue_leaves_its_interval_to_a_nearer_one_and_a_moved_listener_hears_anew`); `kit_recipes::determinism` 1 passed, the pin unmoved; `just api` moved `api/tungsten-kit.txt` alone, `api-check` exit 0; `docs-ratchet` no rise; `repo-check` exit 0; fmt and kit clippy exit 0; 01's suite 379 passed, 1 ignored. A gain at a point that is not finite is 0, so a NaN position is unheard | `crates/tungsten-kit/src/{react,tests}/cues.rs`, `lib.rs`, `api/tungsten-kit.txt`, `DESIGN.md` |
| 3 | 2026-10-27 | Passed | `--test game -- audio:: spells::`: 21 passed, among them the new `audio::a_cue_sound_plays_by_its_distance_from_the_player` (full at 0 and 500 px, level × gain at 2,000, nothing at 3,500), `audio::the_crackle_follows_the_player_away_from_a_burning_crate` (its level at 300 px, level × gain at about 2,000, 0 past 3,072), `audio::a_burning_dummy_ball_or_player_each_raise_the_crackle` and `audio::a_load_puts_the_listener_at_the_loaded_player`; `the_holes_hum_on_one_voice_that_falls_silent_when_the_last_closes` unchanged and passing; the `HUM_REACH\|CRACKLE_REACH` check exit 0; fmt and clippy exit 0; 01's suite 383 passed, 1 ignored. §6's rule applied once: `spawning::spawn_black_hole_system_creates_attractor_at_cursor_on_right_click` read the hole's 0.65 at about 1,790 px from the spawn, so it teleports the player 200 px under the hole | `src/features/audio/{mod,systems,components}.rs`, `tests/game/{audio,main,spawning}.rs`, 01's `README.md` |
| 4 | 2026-10-27 | Passed | `--test game -- lightning:: presentation:: data::`: 40 passed, among them the new `lightning::a_strike_on_the_ground_crackles_sparks_and_leaves_a_scorch_that_fades` (`Ground`, `GroundStruck` and `Thundered` with no `Snapped`, one `ex10_ground_sparks`, the scorch's anchor on the landing, alpha 100–160 at 2.5 s, gone by 5 s, six after eight strikes), `lightning::a_strike_on_an_enemy_or_iron_meets_a_body_and_snaps`, `lightning::a_strike_on_a_deck_sparks_as_the_ground_does_and_leaves_no_scorch`, `lightning::a_jumps_arc_crackles_on_new_paths_and_goes_after_its_life` and `presentation::a_shocked_dummy_crackles_through_four_frames_and_flickers_violet` (frames 0–3 within 15 frames, the tint on the even ones); `a_strike_shows_its_bolt_flash_light_and_shake` passing with its arc still shown 14 frames on and gone 9 after; `level-check` 5,048 outputs, 31 tests OK; fmt and clippy exit 0 (clippy asked for `is_multiple_of`); 01's suite 388 passed, 1 ignored. `Sound::ALL` 81 → 82. A ray's kinematic enemy is a `Body`, any other kinematic body a `Deck`, a static one the ground | `src/features/{lightning,combat,audio}/`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{lightning,presentation}.rs`, 01's `README.md`, the generated `assets/` |
| 5 | 2026-10-27 | Passed | `--test game -- player:: data::`: 20 passed, among them the new `player::a_burning_tick_pulses_the_hurt_edge_half_with_a_soft_tick` (one `PlayerTicked`, no `PlayerHurt`, `ex10_hurt_edge` over the view at the edge's alpha) and `player::a_blow_fills_the_hurt_edge_with_its_hurt_and_it_is_gone_after_0_4_s` (one `PlayerHurt`, no `PlayerTicked`, 0 and undrawn 25 frames on); `a_blow_tints_the_courier_red_for_a_moment` unchanged and passing; `level-check` 5,048 outputs, 31 tests OK; fmt and clippy exit 0 (clippy asked for a range's `contains`); 01's suite 390 passed, 1 ignored. `Sound::ALL` 82 → 83. `player_clips` falls the edge in the present phase after the fixed step sets it, so a frame's read is a frame's fall under what was set: the tests allow that one frame | `src/features/{player,combat,audio}/`, `tools/content/data/cue_rules/sounds.json`, `tests/game/player.rs`, 01's `README.md`, the generated `assets/` |

## Follow-ups

- A listener for `SoundOnEvent`, pan and looping cue voices: redo 7.9 (P7-13).
- K1 in the kit, a cue rule that tells a tick from a hit: the feedback's backlog row.
- Planks from blasts and burning crates: a Later line if play asks for it.
