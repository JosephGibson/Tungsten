# Example 01: the aqueduct and the hollow (`e01hollow`) — in progress

- **status:** in progress
- **goal:** Example 01 (o) ([feedback](1.0/example-01-feedback.md) §3.11, V1–V16, V23, Q61–Q67): the aqueduct gets more scenery and a small pool, loses its three spouts for a second gargoyle (8 streams from 10), and gains a second pier top, more pier-base and floor variants, a coping tile along the channel's top, a broken pillar at its start, and fainter, sparser stains; the hollow's mouth is redrawn with its lintel a row lower, fewer props and rubble of its own; three fire geysers, a new hazard on a 3.9 s cycle that the ice spray chokes; a goblin, a bat and a fourth wisp; rubble stones in three sizes as bodies; a deep look pass on the cave, a darker ambient and a wider lavafall. Save schema 19 holds the geysers and the stones and moves the save IDs past the new map objects. The art and sound come from two redo [§6 asset sessions](roadmap-redo.md#6-asset-sessions) on §3's brief, after the code steps, which build on placeholders until the art is accepted.
- **non-goals:** A geyser lighting, throwing or chilling props and balls (Later); the troll lifting stones; stones in a run loaded from a save older than schema 19 (a fresh run sets them down, as a schema 11 save's run has no crates); any route, tome, support, lava, deck or elevator change; a light past the budget; the ravine's, the mine's or the clearing's look (beyond a floor family the aqueduct shares, §3.2); any kit, core, render or template change; a release, a changelog section or a decision entry (§5 Q16): a release is cut separately (`D-215`).
- **files to touch:** paths under `examples/01_platformer/` (01's) unless rooted; "the generated `assets/`" is the generator's output with `tools/outputs.json`.
  - Step 1, placeholders: `tools/{art,scenery,actors,generate,sfx}.py` (`art.py` for `SIZE_PREFIXES`), `tools/sounds.json`, `tools/test_generate.py`, `tools/README.md`; the generated `assets/` (sprites, particles, sounds, `manifest.json`, `tilemaps/level.tmj`).
  - Step 2, the geyser: `src/features/level/{geyser.rs (new),mod.rs}`, `src/features/toys/systems.rs`, `src/features/combat/systems.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/prefabs/geyser.json` (new), `tools/content/data/cue_rules/sounds.json`, `tests/game/hazards.rs`, `README.md`; the generated `assets/`.
  - Step 3, the stones: `src/features/props/{mod,components,systems,draw}.rs`, `src/features/elements/{systems,cascade}.rs`, `src/features/level/lava.rs`, `src/features/toys/systems.rs`, `src/save.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/prefabs/stone_{small,medium,large}.json` (new), `tools/content/data/cue_rules/sounds.json`, `tests/game/{props,saves}.rs`, `README.md`; the generated `assets/`.
  - Step 4, the hollow's map and schema 19: `tools/level.json`, `tools/{level,placement}.py`, `tools/test_generate.py` (only a role a moved object breaks), `src/features/level/{mod,geyser}.rs`, `src/features/run/mod.rs`, `src/save.rs`, `tests/game/{main,saves,enemies,dummy,hazards,props}.rs`, and only to re-derive a row the lintel moves (§6), `tests/game/{level,playthrough,areas}.rs`; `README.md`; the generated `assets/`.
  - Step 5, the aqueduct's art (asset session): `tools/{backdrops,terrain,scenery,placement,art,pixels}.py` (`placement.py` only to map a new piece's name, `pixels.py` a new helper only), `tools/level.json` (dressing only, §3.2), `tools/palette.json`, `tools/test_generate.py` (only a role a redrawn piece breaks), `tools/README.md`; the generated `assets/`; this plan's §3.6 lock and evidence row.
  - Step 6, the hollow's art and sound (asset session): as Step 5, with `tools/{actors,effects,sfx,generate}.py` and `tools/sounds.json`; this plan's §3.6 lock and evidence row.
  - Step 7, close: 01's `README.md`, `src/features/level/mod.rs` (only a one-constant `AMBIENT_INDOOR` the owner asks for in Step 6), `DESIGN.md`, `docs/plans/1.0/example-01-feedback.md` (§3.11's landed note), `docs/known-issues.md` (only for a finding), `ROADMAP.md`, this plan (deleted).
- **ordered steps:** (1) Placeholders for §3's contract. (2) The geyser. (3) The stones. (4) The hollow's map: the mouth, the foes, the geysers and stones placed, the ambient, schema 19. (5) The aqueduct's art, an asset session ending in the owner's acceptance and its lock. (6) The hollow's art and sound, the same. (7) Close. §4 holds each.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes with 01's whole suite, `just smoke` runs 01, §3.6's lock holds, and in-game captures show the aqueduct, the mouth, the geysers, the stones, the new foes and the lavafall on the accepted art; the plan is deleted with its citations and `e01hollow` is out of Now.

Item `e01hollow` in [ROADMAP.md](../../ROADMAP.md), from the [feedback](1.0/example-01-feedback.md)'s §3.11 with V1–V16, V23 and Q61–Q67. Written 2026-10-27 at `593bcdd` on the `0.110` branch, after `e01sfx`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs: Steps 1–4 are one code session at xhigh that builds on Step 1's placeholders and draws nothing; Steps 5 and 6 are asset sessions on Opus 5.5 at max on §3 alone, each ending when the owner accepts its art; Step 7 closes in a new code session at xhigh. A run session stops before Step 5 unless its evidence row is in, ending with §3.7's first prompt; Step 5's session ends with the second, Step 6's with the run prompt for Step 7.

## Context digest

- The owner's fourth playtest (2026-10-25) found the aqueduct busy with streams and stains and thin on scenery, and the hollow's mouth plain and wide, its cave flat, under-populated and too bright, with no hazard of its own and no rubble to throw; §3.11 sketched the stop, Q61–Q67 hold its defaults (Q61 and Q65 settled by the owner).
- It stands on M76's map and object layers (`D-202`), M85's aqueduct, floating check and stream checks (`D-214`), lava's touch, throw and crust (`D-208`), M81's props and impacts (`D-210`), the tomes' route tests (`e01tomes`), e01foes' foes and its asset loop, e01sfx's cue listener and schema 18.
- `D-215`: a commit per step, one critique round, the asset loop, the tiers (`D-151` as amended by `D-195`): art and sound on Opus 5.5 at max, code at xhigh. The brief's contract (§3.1) is what the code reads, so accepted art touches no code. A schema change breaks saves only with a migration ([operating rules](../operating-rules.md#freeze-process-removed)).
- Every new saved map object moves the save IDs after it (A14): three enemies and three geysers move the orbs, the player and every dynamic body, which schema 19's migration shifts as schema 12's did (`SHIFT_12`).
- The code comes first because the hollow's review sheet needs real loops of the geysers, the stones and the new foes (Q1). Later stops expect: (p) the toxic flask on this map and schema.

## 1. Audit

Read at `593bcdd`. Line numbers move as steps land; each step re-reads its rows.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `Hazard { fire, lava }` is the only hazard component; the map has no fire hazard (`hazards: []`), only the three lava basins in the `hazards` layer, fires first, then basins | `src/features/level/components.rs:24-30`; `tools/level.json` (`hazards` 0, `lava` 3); `tools/level.py:163-223` |
| A2 | Every map object a save names takes the next save ID in layer and file order: anything with `Hazard`, `PathFollower`, `ElevatorDef`, `Dummy`, `Enemy` or `Orb`. Layers spawn hazards, platforms, …, dummies, enemies, pickups, tomes; tomes take none. Today: basins 0–2, decks 3–7, dummy 8, enemies 9–34, orbs 35–42, player 43 | `src/features/level/mod.rs:195-203`, `:252-259`; `tools/level.py:300-317` |
| A3 | A cold spray ray finds fire hazards it crosses by their touch box (`fires_crossed`) and douses each for `DOUSE_SECS` 4; a ray that stops on lava's surface chills its basin | `src/features/toys/systems.rs:1085-1097`, `:1137-1162`; `src/features/elements/systems.rs:1467-1490`; `src/features/elements/mod.rs:86` |
| A4 | Lava's crust: `CRUST_COLD` 0.5 s of spray sets it for `CRUST_SECS` 6, its touch held aside; the chill drains as fast unsprayed; `lava::start` inserts each basin's state at a run's start; its look reads only saved state | `src/features/level/lava.rs:1-13`, `:41-47`, `:106-160` |
| A5 | Lava's bite is typed fire 16 with knockback 1,500 and `push_always`, its box reaching from just over the surface to the bed, so the push away from the box's centre is up and out | `tools/level.py:21-33`, `:206-220`; `crates/tungsten-kit/src/overlap/grid.rs:491-503` |
| A6 | Fire's ladder burns at 0.25 of the target's threshold within 3 s, blocked by `status.frozen` and `status.doused`; the player's threshold is 30 + 0.2 × 100 = 50, so 12 fire alone leaves him 0.5 short; goblins (15), bats (12) and crawlers (20) burn on one 12 hit | `tools/content/data/damage_types/game.json` (`damage.fire`); `tools/content/prefabs/{player,goblin,bat,crawler}.json` (`ailment_threshold`) |
| A7 | 01's `reactions` reads each step's `DamagedPart`s with their source; `EffectRequest::new(target, DataRef::by_id(..)).apply` applies an effect, as the douse's cascade does | `src/features/combat/systems.rs:80-100`; `src/features/elements/cascade.rs:788` |
| A8 | The light budget counts glowing props, tomes, fires, the lit lava and wisps per 92-column view, at most 10; the busiest view over the hollow holds 7 today (the ravine's tome and crystal, three wisps, the pact tome, the lit lava) | `tools/level.py:57-58`, `:676-688`; counted from `tools/level.json` this session |
| A9 | A bat must hang flush under a roof: a whole-row `y` with the cell above solid | `tools/level.py:689-695` |
| A10 | The mouth: `east_wall` (203–215, rows 30–56, `bottom` 57) over `pool_bed` and `cleft` (row 63), 6 rows open; `hollow_roof`'s bottom is 53 from 216, 10 open. Ravine bat c hangs at (209.5, 57) under it | `tools/level.json` (`platforms`, `enemies`) |
| A11 | A jump peaks 227.6 px and the head is 56 px over the feet, 283.6 px over the floor; with the lintel at row 58 the mouth opens 320 px, 36 px clear. The rows through the mouth: `LAUNCHES` pool_bed→cleft (205.85→206.75) and cleft→hollow_floor_1 (215.85→216.75), both jumping; the playthrough's same two; the areas' crossing walks 208→211 at row 63 | `tests/game/level.rs:121-122`; `tests/game/playthrough.rs:56-58`; `tests/game/areas.rs:30-36` |
| A12 | The hollow: `hollow_floor_1` 216–230 and `hollow_floor_2` 240–250 at row 63, the lava lake 230–240 round a stepping stone, `hollow_step` 250–254 (row 60), `hollow_shelf_a` 254–260 and `hollow_shelf_b` 263–268 (row 57) round the shelf's lava 260–263; three wisps at x 224.5, 237.5, 245.5; the pact tome at 232.5 over the lake. Every floor is walked by a `LAUNCHES` row or the playthrough | `tools/level.json`; `tests/game/level.rs:122-129`, `:190`; `tests/game/playthrough.rs:57-63` |
| A13 | The tests' `calm` despawns every enemy before a scripted walk; level, playthrough, areas, hazards and eight more test files call it | `tests/game/main.rs:204-213` |
| A14 | Schema 18; `map_ids` maps every saved ID through a function but walks neither `/snowballs` (`id`, `thrower`) nor `/planks` (`id`, `/fuel/lighter`), both added after the last shift; schema 12's `to_12` is the model shift (`S11_PICKUPS`, `SHIFT_12`); `MAP_ENEMIES` 26, `MAP_PICKUPS` 8 | `src/save.rs:142-149`, `:185`, `:270`, `:494-503`, `:866-933`, `:1319-1340` |
| A15 | `HazardSave` keeps a fire on its oscillator (the query needs `Oscillator`); `basins` and `elevators` keep their own lists; `SaveGame` refuses unknown fields | `src/save.rs:78-136`, `:1084-1090`, `:1381-1390` |
| A16 | Relabel tests push a current capture through older migrations: schema 7 (`:2003`), 14 (`:3249`, `:3274`, `:3330`), 15 (`:3543`, `:3672`), 16 (`:3627`) and 17 (`:3606`); a shifting migration shifts such a capture a second time. Fixture tests migrate schemas 8–13 and compare IDs through `schema_10_id`, `schema_12_id`, `without_new` and the `S10_*` spans | `tests/game/saves.rs:2003-2080`, `:2081-2141`, `:2400-2426`, `:2507-2775`, `:2986-3046` |
| A17 | Pinned counts: the map's bats 7, `MAP_ENEMIES` placed and saved, the player's ID as the dummy's + 1 + `MAP_ENEMIES` + `MAP_PICKUPS` | `tests/game/enemies.rs:1933`, `:2931-2948`; `tests/game/dummy.rs:44-48` |
| A18 | A full pit's save is 1,789,806 bytes, 307,346 under the 2 MiB bound (run this session); an enemy costs about 3.3 KB, a prop about 0.45 KB | `tests/game/saves.rs:2919-2946` (its printed size); M81's per-object costs |
| A19 | `PropKind` {Crate, Keg, Bomb, Rock, Plank} with `kind`, `all`, `half`, `centre`, `prefab`, `cap`, `rolls`; a goblin's rock is a circle of mass 6 capped at 24; planks are a `PropKind` saved apart in `planks`, since `bodies()` refuses a kind `S15_PROPS` lacks | `src/features/props/components.rs:8-16`; `src/features/props/mod.rs:150-243`, `:399-421`; `src/save.rs:591-640` |
| A20 | A fresh run sets the mine's iron, then the map's crates and kegs down at their markers after the player; `props::map_ids` counts them; `placement.prepare` stands each crate and keg box on its support from its prefab's half extents | `src/features/run/mod.rs:88-103`; `src/features/props/mod.rs:429-439`; `tools/placement.py:100-124`; `tools/level.py:288-298` |
| A21 | The player shoves bodies up to mass 30, slowed by mass over 60; walkers shove up to 8 | `src/features/player/mod.rs:72-78`; `src/features/enemies/mod.rs:90-93` |
| A22 | A rock meeting anything hard clacks (`Sound::RockHit`), a few a step; sight passes rocks, balls and bombs | `src/features/elements/systems.rs:1431-1434`; `src/features/enemies/systems.rs:450-461` |
| A23 | `Sound::ALL` holds 84; each variant needs `cue_of`'s arm and a cue rule | `src/features/audio/components.rs:220`; `src/features/audio/mod.rs:95-152`; `tools/content/data/cue_rules/sounds.json` |
| A24 | `AMBIENT_INDOOR` (0.30, 0.28, 0.32) is one constant; only the hollow is indoor; tests read it by name | `src/features/level/mod.rs:83`; `src/features/level/systems.rs:211-232`; `tests/game/areas.rs:10`, `:152` |
| A25 | The aqueduct (93–161): 10 pour stacks, 6 at the gaps' broken edges, 1 from the gargoyle (141.01), 3 from `spouts` at 101, 137, 153 (row 28), each spout a `channel_spout` variant; `arcade_pier_top` and `arcade_capital` once each over 11 piers; one `pier_base` ×17; `pier_wet_*` dark runs 10–17 px at dither 0.8; lime and wet streaks in the arches; the channel's coping drawn inside each `channel` tile; streams are scenery only (no `src/` reads them) | `tools/level.json` (`spouts`, `props`); `tools/level.py:420-421`; `tools/backdrops.py:311-372`, `:379-445`, `:447-469`; `tools/terrain.py:778-877`; `rg -i 'pour|gargoyle|spout' src` |
| A26 | The floating check's classes: streams by name, liquids and `pool*` exempt, sources `gargoyle`, `fissure`, `spill_`, the gap strip's pieces, `cave_mouth` a structure; a new piece needs a name it maps. `test_generate.py` finds the wall-anchored prop and a clean pour stack of 6+ by role, not by name | `tools/placement.py:174-185`; `tools/test_generate.py:165-192` |
| A27 | The mouth's look: `cave_mouth(8, 7)` cut into decorations over 203–210, rows 56–62; 14 props over 203–214; the hollow has no rubble scenery; `rubble()` and `rubble_pile` exist; the lavafall is 10 `lavafall_0` segments, a 7 px stream in 4 frames out of the fissure at x 238 | `tools/backdrops.py:731-774`; `tools/scenery.py:387`, `:1036`, `:2649-2660`; `tools/level.json` |
| A28 | Asset traps: any new sprite shifts every tile ID in `level.tmj`; linear-light blending makes faint overlays read heavy; emissive masks take only `y Y C c`; particles are one sprite from one point and draw at z 0 | `tools/art.py:104-107`; e01sfx's Step 8 (the hurt edge at 22 and 40 of 255) |

## 2. Design

### 2.1 The geyser (V11; Q65; §5 Q2–Q7)

- **`Geyser`** in `src/features/level/geyser.rs`, registered as `geyser` (its one prefab field `offset`, seconds into its cycle at a run's start), its state `#[serde(skip)]`: `phase` (`Idle`, `Sputter`, `Erupt`), `left` (seconds of the phase), `cold` and `sprayed` (as `Lava`'s), `choked` (seconds left) and `touch: Option<ContactDamage>` (held aside outside an eruption, as `Doused` holds a fire's). No `Hazard` on it, so nothing that queries `Hazard` (fires, lava, the playthrough's hazard watch) meets it; `level::saved` gains `Geyser` with the map's geysers in Step 4 (A2).
- **Cycle:** `IDLE_SECS` 2.5, `SPUTTER_SECS` 0.6, `ERUPT_SECS` 0.8; `geyser::start` (from `level::spawn`, after `lava::start`) takes each geyser's touch aside and starts it `offset` seconds into its cycle; `geyser_tick`, in the tick phase beside `lava_tick` and before the overlap phase's contact damage, does the same for a geyser spawned later (a test's) on its first step, then runs the phase down; at an eruption's start the touch goes back on, with `ex10_geyser_burst` and `Sound::GeyserErupt`, and at its end it comes off; a sputter's start plays `Sound::GeyserSputter`; its embers child emits only in a sputter or an eruption, drained else, as a doused fire's trail is.
- **Touch** (the prefab's `contact_damage`): `{"damage.fire": 12}`, `interval` 0.5, `knockback` 1,000 px/s, `push_always`, filter `any: ["team.enemy", "team.player"]` as lava's bite; its box half (20, 176) about (32, 32) from the geyser's point, the vent sprite's top-left, which is the floor's top under the vent's mouth, so it reaches 176 px up the plume and as far into the rock below, and the push from its centre throws what stands in it up and out (A5).
- **Burning:** in `reactions`, a `damage.fire` part over 0 whose source is a geyser, on a target not tagged `status.burning`, `status.frozen` or `status.doused`, applies `burning` from the geyser through `EffectRequest`, once a target a step (A6, A7). A fire-immune wisp takes no part, so no burn; fire on a frozen target stays thermal shock.
- **Choke** (Q65 "as it crusts lava"): `spray` with cold also collects the geysers its rays cross, by the vent box (half (24, 12) about (32, 20) from the point, so it stands over the floor, where a ray reaches it before the tile stops it), as `fires_crossed` does, and calls `geyser::chill`; `geyser_tick` runs `cold` as `lava::tick` does, and at `CRUST_COLD` 0.5 chokes it for `CRUST_SECS` 6: an eruption ends at once, its touch off, `ex10_steam_burst` at the vent and `Sound::Steamed`; when the choke ends it starts `Idle` with `IDLE_SECS`.
- **Look** (`geyser_look`, present phase, as `lava::look`): the vent sprite `ex10_geyser_vent`, `ex10_geyser_vent_choked` while choked; a child plume sprite shows `ex10_geyser_sputter_<n>` by the sputter's elapsed time (4 frames over 0.6 s) and `ex10_geyser_plume_<n>` by the eruption's (6 frames over 0.8 s), hidden else. It reads only the phase, its time and the choke, which the save keeps. No point light (Q6).
- **Save (Step 4):** `SaveGame::geysers` (`GeyserSave`: id, phase, left, cold, choked; `#[serde(default)]`), in save-ID order; a load sets each map geyser's state by ID, its touch on only in an eruption; touch timers start over, as a basin's do.

### 2.2 The stones (V15; Q66; §5 Q8, Q9)

- **`PropKind::Stone(StoneSize)`** (`Small`, `Medium`, `Large`; `Copy`, so the `const fn` tables match on it; a stone is never in `props`, so the variant's serde shape reaches no save) with `Stone { size, variant }` (variant by save-ID parity, as a rock's), three prefabs `stone_small`, `stone_medium`, `stone_large`: dynamic boxes of half (9, 7) about (16, 25), (13, 10) about (16, 22) and (20, 15) about (32, 49), mass 10, 15 and 20, restitution 0.1, on the sprites of §3.1. `kind`, `all`, `half`, `centre` and `prefab` take the size from the kind; `cap` none (`usize::MAX`: the map's own, never summoned); `rolls` false, so a stone slides by `WOOD_FRICTION` as a crate does.
- **Behaviour as a goblin's rock**, in every arm the compiler lists for `PropKind` and `What`: pushed, thrown and blasted; impacts hurt by mass and speed; unburnable and unfreezable; taken by lava in steam; passed by sight; struck by no lightning. A stone meeting anything hard knocks (`Sound::StoneKnock`) beside the rocks' clack, under the same per-step bound (A22).
- **Save:** `SaveGame::stones` (`StoneSave`: id, size, variant, position, velocity; `#[serde(default, skip_serializing_if = "Vec::is_empty")]`), not `props`, since `bodies()` refuses a kind `S15_PROPS` lacks (A19). Step 3 adds the list on schema 18: no stone stands on the map until Step 4, so no Step 3 save holds one; Step 4's schema 19 names it.
- **Placement (Step 4):** `level.json`'s `stones` (`{x, support, size}`), stood on their supports by `placement.prepare` from each size's prefab as crates are (A20), become markers `stone_small`, `stone_medium`, `stone_large`; a fresh run sets them down after the kegs, so no earlier dynamic ID moves.

### 2.3 The hollow's map (V8, V12, V13, V16; Q64, Q67; §5 Q10–Q12, Q15)

- **The mouth:** `east_wall`'s `bottom` 57 → 58 (rows 30–57), 5 rows open at the mouth, 10 inside; ravine bat c to (209.5, 58). The props hung under the wall follow its underside (`prepare`); the mouth's decorations are Step 6's (V7). `LAUNCHES` and the playthrough keep their rows (A11); a row that fails keeps the lintel at 57 (§6).
- **Foes**, appended to `enemies` in this order: `hollow goblin` on `hollow_shelf_b` (x 265.46875, y 56.046875, leash 1.5), `hollow bat` under `hollow_roof` (x 250.5, y 53), `hollow wisp d` (x 261.5, y 54.0) over the shelf's lava. The busiest view then holds 8 lights of 10 (A8). `MAP_ENEMIES` 29.
- **Geysers:** `level.json`'s `geysers` (`{x, support, offset}`): x 225.0 on `hollow_floor_1` (offset 0), 243.5 on `hollow_floor_2` (1.3), 257.0 on `hollow_shelf_a` (2.6), so no two erupt together. `level.py` writes them to a `geysers` object layer after `tomes`, class `geyser`, its point the vent sprite's top-left at (x × 64 − 32, row × 64 − 32), so the vent's bottom row stands on the support's top, `geyser.offset` a property; the layout check refuses a geyser off its support or with a wall within its plume (176 px). Their save IDs follow the orbs' (Q2). `MAP_GEYSERS` 3.
- **Stones:** nine in three heaps of small, medium and large: `cleft` x 211.0–212.5, `hollow_floor_1` x 219.0–220.5, `hollow_floor_2` x 246.5–248.0, each at least 2 tiles from a geyser and 1.5 from a launch's column (A12); the run session may move a heap up to 3 tiles to clear a check, recorded.
- **`calm`** (`tests/game/main.rs`) also despawns the geysers and the stones, so scripted walks and launches through the hollow meet nothing new (A13); the geysers' and stones' own tests run without it.
- **Ambient:** `AMBIENT_INDOOR` (0.22, 0.205, 0.235), today's hue at 0.22 / 0.30 (Q67); Step 6's sheet judges it; Step 7 takes a one-constant change the owner asks for.

### 2.4 Schema 19 (Q17)

- `S18_PICKUPS` 35 and `S18_PLAYER` 43, frozen; `to_19` maps every ID at or above `S18_PLAYER` up by `SHIFT_19` 6 (the three enemies and the three geysers) and every one from `S18_PICKUPS` below it up by `SHIFT_19_PICKUPS` 3 (the enemies), through `map_ids`, which now also walks `/snowballs` (`id`, `thrower`) and `/planks` (`id`, `/fuel/lighter`) (A14). `to_19` also moves what the lintel's new row holds (`S19_LINTEL`, x 203–216 and y 57–58 in tiles, frozen): the map's ravine bat c (save ID 14, `S18_BAT_C`) gets its saved `Enemy::home` set to the new roost, `S19_BAT_C_HOME` (13440, 3740), the frame's grip point under row 58, rather than moved by 64 px, since the schema 8–13 fixtures hold an older roost at y 3752.8; when its body meets the row its position goes to that roost too, since a load restores both over the map's (`src/save.rs:2116-2125`); every other saved body `bodies()` reads that meets the row is set at rest with its top 0.5 px under it, clear of the others, or dropped as `to_16` drops one, `bodies()` now reading the planks too. Arms 7–17 end with `to_19`; `18 => to_19`. A schema 18 save names no new enemy or geyser and holds no stone: the map's stand at home, the geysers start at their offsets, the hollow has no stones (Q9).
- Tests: `MAP_ENEMIES` 29, `MAP_GEYSERS` 3 and `dummy.rs`'s ID sum; a `schema_19_id` helper composed after `schema_10_id` and `schema_12_id` in the fixture tests, `without_new` setting aside the three enemies, `geysers` and `stones`; an `as_schema_18` helper that turns a current capture into the 0.110 code's (drops `geysers`, `stones` and the three enemies wherever a list names them, maps the IDs back, and puts bat c's home and position a row up, at its 0.110 roost); an `as_schema_19` helper that applies `to_19`'s bat and lintel moves to a fixture's old side before `moved_ids` compares, as `as_schema_15` and `as_schema_17` do, applied first in every relabel test (A16); a new test that `as_schema_18` of a capture, migrated, equals that capture without the new objects, and loads with the new enemies at home and the geysers idle.

### 2.5 Sounds and cues

`Sound::GeyserSputter` and `Sound::GeyserErupt` (Step 2) and `Sound::StoneKnock` (Step 3), each with its `cue_of` arm (`occurrence::<N>` next free) and its cue rule, played at their point through the cue listener's falloff; the choke reuses `Sound::Steamed`. `Sound::ALL` 84 → 87.

## 3. Asset brief

The redo's [§6](roadmap-redo.md#6-asset-sessions) brief for this stop's families. Step 1 makes every §3.1 file from today's art; Steps 5 and 6 draw in their places with the generator's own toolkit, within this contract, and touch no `src/` or `tests/` file.

### 3.1 Families and contract

Generator names; the manifest prefixes `ex10_`. The vents and stones are lit as the rocks are (normal and emissive maps); the plume is unlit, drawn over the lit world, its glow by emissive colours and bloom. Particles draw at z 0, under the sprites (A28).

| Family | Files | Size, frames, caps | Anchor and contract | The code's use |
| --- | --- | --- | --- | --- |
| Geyser vent | `geyser_vent`, `geyser_vent_choked` | 64 × 32 each | Its bottom row on the floor's top; the mouth's middle at (32, 20) | §2.1 |
| Geyser plume | `geyser_sputter_{0..3}`, `geyser_plume_{0..5}` | 64 × 64 and 64 × 176 | The frame's bottom row on the floor's top, over the vent, centred on its mouth, so the touch box covers it | §2.1 |
| Geyser particles | `geyser_embers` (continuous), `geyser_burst` (a key burst) | At most 16 and 24 alive | From the mouth | §2.1 |
| Stones | `stone_{small,medium}_{0,1}`, `stone_large_{0,1}` | 32 × 32 and 64 × 64 | The body box of §2.2 inside each, its lowest opaque row the box's bottom (31, 31, 63) | §2.2 |
| Sounds | `ex10_<id>_sfx` at `sounds/<id>.wav`: `geyser_sputter`, `geyser_erupt`, `stone_knock` | 0.4–0.7 s, 0.6–1.0 s, 0.08–0.4 s; volumes 0.3–0.7 | — | §2.5 |
| The aqueduct (Step 5) | §3.2's pieces | Free within the floating and stream checks | Dressing only: no platform, enemy, geyser, stone, support, tome, lava, route or deck moves | none |
| The hollow (Step 6) | §3.2's pieces | As the aqueduct | As the aqueduct | none |

### 3.2 Art direction

The house style and the owner's standing notes from earlier reviews (detailed and characterful first passes, tile variants, layered backs, props stood through the surface check, structures as joining tiles, a broken end over a pier or its haunch, carved figures on a mount in their own material).

- **The aqueduct (Step 5; V1–V6, Q61–Q63).** More scenery under and along the arcade, among it a small pool on the floor with reeds and wet stones at its edge (`pool_frame`'s calm kind reused or its own sprite, exempt as `pool*`); the three spouts gone, their tiles plain channel, and a second gargoyle with its own pour where the middle spout was (x 137), apart from the first in pose, 8 streams in all; a second pier top, alternating over the piers; more pier-base variants and floor variants under the arcade (a family of its own, or variants the clearing may share, judged in both); a coping tile drawn along the channel's top, apart from the trough; a broken pillar at the area's start (columns 93–99), stood on the floor as a structure; wet stains on 2 of the 6 piers at about half today's dither, lighter, and the arches' streaks thinned. `level.json` changes are dressing (`props`, `groups`, `decorations`, `backs`, `spouts`, `mists`); the arcane tome's bay (117–120, row 34) stays clear.
- **The hollow (Step 6; V7, V9, V10, V14, V23, Q64, Q67).** The mouth redrawn for the lintel at row 58: a cave mouth cut into the wet rock with its own lintel, framing stones and roots from the ravine side, its decorations re-cut to rows 57–62; of its 14 props keep about six (no stalactite row, no vine curtain), the rest replaced by rubble of the hollow's own: basalt and obsidian chunks, scree against the walls, a fallen slab, in two or three heaps per floor, apart from the stones' bodies but of their material. A deep pass on the cave: basalt tiles with more variants (cooled flow lines, glassy obsidian, ember cracks in `y Y C c`), layered backs with depth, haze unlit and subtle, ember pools and fire crystals that read as sources of the light. The lavafall wider, a bright core with crusted, darker edges and a splash and spatter at its foot in the basin. The geysers: a vent of cracked, heat-glazed basalt round a glowing throat, choked a crust of grey-white rime over it; the sputter licks of flame and smoke, the plume a roaring column of flame, white-yellow at its core, red and smoky at its edges, embers flung out. The stones: rubble of the hollow's basalt, angular, two looks a size. The cave at the darker ambient, judged against today's in the sheet.
- **Sounds** (redo Q5: a playlist with notes). The sputter a hissing gurgle with pops; the eruption a deep whoosh into a roar that drops off; the knock a dull stone thud, heavier than the rock's clack.

### 3.3 Values the code takes

None from the art: the contract fixes the sizes, frames, boxes, caps and lengths the code reads. Each asset step records in its evidence row each stone sprite's lowest opaque row against §3.1, the plume's frames against 64 × 176 and each sound's measured length. Step 6's sheet records the owner's word on `AMBIENT_INDOOR`.

### 3.4 Placeholders (Step 1)

Every §3.1 file from today's art at the contract's size, frames and cap, by copying, cropping, cycling or resizing, never drawn: the vents a crop of `ember_pool_0` and the same darkened; the sputter frames `lavafall_0`'s frames resized to 64 × 64; the plume frames `lavafall_0`'s stacked to 176 and cycled to six; `geyser_embers` a copy of `fire_trail`, `geyser_burst` of `blast_embers`; the stones `goblin_rock_0` and `_1` resized nearest-neighbour to each box; the sounds `fuse_hiss` (sputter), `flame_start` (eruption) and `iron_thud` (knock), measured. `tools/test_generate.py` gains `HollowContractTests` reading the generated `assets/`: every §3.1 sprite at its size and count, each stone's lowest opaque row at its box's bottom, each particle config's cap, each sound's length and volume in range. Today's files and the placeholders pass it, and so must the art.

### 3.5 The review sheets

One per asset step, in `perf-runs/<date>-e01hollow-aqueduct-review/` and `perf-runs/<date>-e01hollow-cave-review/` (machine-local), linked in the conversation at each round, the contact sheet's link first:

- A contact sheet at 4×: every new or redrawn piece beside today's.
- Before-and-after stills of the area at play zoom and at 1080p, from a scratch driver under `target/` (deleted after; the in-game shots recipe, one capture job at a time, the shell's pause off in the copy; `perf-runs/20261027-e01sfx-art-review/tools/` is the latest driver).
- Step 6 adds in-game loops: walking in through the mouth; each geyser's cycle, the wizard thrown and burning, a goblin thrown, the ice spray choking one; the force push sending a heap of stones into the new goblin; the new bat and wisp; the lavafall's foot; the cave at 0.22 beside 0.30. And a playlist with notes: the three sounds beside today's `fuse_hiss`, `flame_start` and `rock_hit`.

### 3.6 Lock

On acceptance, Step 5, then Step 6, appends one `sha256sum` line per file under `examples/01_platformer/assets/{sprites,particles,sounds}/` its diff adds or changes, paths from the repository root, between these fences. The check, `sed -n '/^```sha256$/,/^```$/p' docs/plans/e01-hollow.md | sed '1d;$d' | sha256sum -c --quiet`, exits 0 only when every file is unchanged, and 1 while the block is empty. Step 6 changing a file Step 5 locked is a stop (§6).

```sha256
e959f1625582336ea65f81826d0dd90dc60a9154dd4206eb66ff8193e1026fb2  examples/01_platformer/assets/sprites/amphora.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/amphora_e.png
38ee2f0472abaf4fa94a9d3e04b1eb275ac4c8e8b8c914a31567774566fb4b87  examples/01_platformer/assets/sprites/amphora_n.png
cdf92058efdd9be1adcd27a863ec47c5de2144a952b149700afca99a1da2c3d4  examples/01_platformer/assets/sprites/aq_paving_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/aq_paving_0_e.png
442b87999a96d4e60d1ae7abb1b43f3f7cc1734d65f79b71c79e5012e5f09a5c  examples/01_platformer/assets/sprites/aq_paving_0_n.png
a45fc33a583ad1d62fed7ad644a935f89f76b144fc7e0db3f0084dddd2a7034e  examples/01_platformer/assets/sprites/aq_paving_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/aq_paving_1_e.png
db1bd2c6c0b4f0d4bf873398e72655806bba177d80a8d1bf4c3a76e20b83e8cf  examples/01_platformer/assets/sprites/aq_paving_1_n.png
0e730899df8a97f1a93498735b915515cf5eb804e76c59f8f64afe436e6d1699  examples/01_platformer/assets/sprites/aq_paving_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/aq_paving_2_e.png
a39defb1cab5766bd98b0a16215b3e596df7f60c06d4a52627dbb86c098fb326  examples/01_platformer/assets/sprites/aq_paving_2_n.png
74b0c159820feafa4889ad6d0c8ac9a8014f494210551fe735308377f04c4c43  examples/01_platformer/assets/sprites/aq_paving_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/aq_paving_3_e.png
f679efc6e055c24531e0da1a21809cfd2f0b9e3706e248c568168c91cbeb88ac  examples/01_platformer/assets/sprites/aq_paving_3_n.png
d863ca46df2c95e9a77b394d313672b9868f1350f07455b6fef4fb83c9f056fe  examples/01_platformer/assets/sprites/arcade_arch_key.png
ba3a6c84a7887f0ce27b0c69cea1d4446d51ed9a6b76774c205b693e0033e82c  examples/01_platformer/assets/sprites/arcade_arch_key_n.png
eddaf7cd134611fb8f386915ca11ad6f4a186a5d0b21c0a2acb35037231c89dc  examples/01_platformer/assets/sprites/arcade_arch_left.png
a237d42c2a5536068206e577f6c982661d336b03e5d13633c8a540b8f797e9d9  examples/01_platformer/assets/sprites/arcade_arch_left_n.png
8f8c296b039c82f2c055cafc2e96aa9578a1b5792a11e629ad22bcb130391b0f  examples/01_platformer/assets/sprites/arcade_arch_right.png
713d13c063f8fa08bcbaed8c0c9cf4b23952237822fd8cdd2e38f2722bd19cdf  examples/01_platformer/assets/sprites/arcade_arch_right_n.png
09f96d8525559cffae0f2a6767cdf5eb08ef8b0d1270d3a6697578f0aa1ecb11  examples/01_platformer/assets/sprites/arcade_gap_0_0_0.png
0a1c0000d248d341d5cd552e6c228d8dd7396138031cff77e3be10a761466938  examples/01_platformer/assets/sprites/arcade_gap_0_0_0_n.png
ceb4e34bd7ec96b9a98ec0f4f0ad71ea2bb3f6273ebc9eadccb0fddab5f9830b  examples/01_platformer/assets/sprites/arcade_gap_0_0_1.png
28c502b78a027851f3cd5ac9b4502d10ebe9b6c58d36ea79c3e5d7762b746be4  examples/01_platformer/assets/sprites/arcade_gap_0_0_1_n.png
86caad5d83d984c4a73e34d2a25672aaf7fbd463b4018d0a75d461c135045b2d  examples/01_platformer/assets/sprites/arcade_gap_0_0_3.png
6ce6aaea59326552dc57d4ced3dfb47482a0c2cf4c7c0402c72a4bda3b2644d9  examples/01_platformer/assets/sprites/arcade_gap_0_0_3_n.png
3ced30092e2674a1dab24b727d6d006a4a01077ff48760d902b23497f4186bf6  examples/01_platformer/assets/sprites/arcade_gap_0_0_5.png
6a54c75b9d88c3bc4c540a2bf052e08e164a7a416ada400c50e2c78fb36c6b03  examples/01_platformer/assets/sprites/arcade_gap_0_0_5_n.png
8c83009689fb2ece00d54226fbe10e2d11952fb285d213b1435f7648a31e31fc  examples/01_platformer/assets/sprites/arcade_gap_1_0_0.png
1f796c82e22af93af56d90349b799ff13ed480e0f581789623c4c7dc5c79370d  examples/01_platformer/assets/sprites/arcade_gap_1_0_0_n.png
b237f8c8b39674fac1433e9d72af796ebefeeb6c3cb42793e9ed17634d757bf8  examples/01_platformer/assets/sprites/arcade_gap_1_0_1.png
8cf15f4c67c2c5d0db7b8e9472853b3462beeb8b85531484fc11e047d8092340  examples/01_platformer/assets/sprites/arcade_gap_1_0_1_n.png
bc9c25c1e095ce4f5f722d73391f4fe30cd7102d4e279fdcf210bae01b819026  examples/01_platformer/assets/sprites/arcade_gap_1_0_3.png
f5006cb962c44c1490494a0807e5f41ec20a5b78f75214e9b2588220eb593c94  examples/01_platformer/assets/sprites/arcade_gap_1_0_3_n.png
491b63c457d212c9fa481fefcb091bfc94f3f08fe2c302cc60582d3f8f0def21  examples/01_platformer/assets/sprites/arcade_gap_1_0_5.png
3aa6b27d56de9984f745c48fba258fb625bd92e0968e363192bbea38c4d3b688  examples/01_platformer/assets/sprites/arcade_gap_1_0_5_n.png
c68a2b32d3a5be5bc62106fac07c639225211dc56ebfdd83c47646e05248ab09  examples/01_platformer/assets/sprites/arcade_gap_2_0_0.png
8e7884320adbc0ccb782bf71279a4abf339e0fd4fff347f02aa5aafe1e21823b  examples/01_platformer/assets/sprites/arcade_gap_2_0_0_n.png
a7118374107d81c4c94fa5ee9426b6f22ed14194b1377afb0c703e6e092ac0af  examples/01_platformer/assets/sprites/arcade_gap_2_0_1.png
6fce5a65e2d315dd1c80ef055c7aaeacf9d8958eeecf992c55df4924b711209f  examples/01_platformer/assets/sprites/arcade_gap_2_0_1_n.png
b06edd02351f35de6b6110510c9f5e6416ec4fb5803ed0bac6c699faab6536a2  examples/01_platformer/assets/sprites/arcade_gap_2_0_3.png
d0c135c6b46b153cbf67036b631f06deb22bd1ee2c1ef13fdb4eca85882a9bea  examples/01_platformer/assets/sprites/arcade_gap_2_0_3_n.png
7a7eb8ffb04132508b0c34d9e37fd3ca4bb882920a5b8bbc88cdd840d1340507  examples/01_platformer/assets/sprites/arcade_gap_2_0_5.png
838964b77b8e9993ea67f54e1b606e830a021896d016582b4000ffc2ce5c52e4  examples/01_platformer/assets/sprites/arcade_gap_2_0_5_n.png
2a0210eeef920ba280d29b337169db10cac50948ca08356ea9cff66fb1bc9617  examples/01_platformer/assets/sprites/arcade_pier_top_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/arcade_pier_top_1_e.png
62e5b2be984548d0d86fe90b5485dbd62ff0af9b080e15d30cc4caf751d91262  examples/01_platformer/assets/sprites/arcade_pier_top_1_n.png
1c47b8d5623dd8fafc05fed586008da97349cea668798d352dbba9c959ea62ae  examples/01_platformer/assets/sprites/broken_pillar_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_0_0_e.png
70ed2487071fe6c99638c38f144abaf1b491918d3b08bbecc431b262062c8b3d  examples/01_platformer/assets/sprites/broken_pillar_big_0_0_n.png
1f43d6c82ce68c1caab2ce7e4592d60a4066ebb4cc1f8f889accaf61b1ea48f2  examples/01_platformer/assets/sprites/broken_pillar_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_0_1_e.png
166757ba07cbb939f89a002752b2a8be0230d6c3f84be03d22d789ff073d3cc5  examples/01_platformer/assets/sprites/broken_pillar_big_0_1_n.png
edf82649a15ce2fe68a11f2ab198e75d42bdc86f21b31bd4cd9d34f8df439dac  examples/01_platformer/assets/sprites/broken_pillar_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_1_0_e.png
969ac9b11a89db4373d40997ae90f2ee4904ba9e9db61347155791d20c38c45e  examples/01_platformer/assets/sprites/broken_pillar_big_1_0_n.png
b4314f68e1f7207568c211b870bc5627b98233e1efa457805b032c0bb824617d  examples/01_platformer/assets/sprites/broken_pillar_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_1_1_e.png
2ff914e02d63e3ab949541e583a6bc967ee7f8185cab94796e1243dce573540f  examples/01_platformer/assets/sprites/broken_pillar_big_1_1_n.png
f7091e5801f014f6f700b29f6383f1f2eb1171d997c312fa916d76acb7bd0aef  examples/01_platformer/assets/sprites/broken_pillar_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_2_0_e.png
24a297605fe926d9ef40be10296f42ff9d3dcb7b56fc01e8947d28dc0e1b2d64  examples/01_platformer/assets/sprites/broken_pillar_big_2_0_n.png
0e69047bfd7a5afe8dfbc1a156ff46411adb3e56c9fe544ba2a06ebbd5a8beaa  examples/01_platformer/assets/sprites/broken_pillar_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_2_1_e.png
27c19d2cd8244299fae9f788c9548d3efae0d1d325081ad71323b49dab7d559a  examples/01_platformer/assets/sprites/broken_pillar_big_2_1_n.png
9a4d5bd3906a7d6139863d5b1f2e65b0322817bb4746ffe72e6515ef0d17db4a  examples/01_platformer/assets/sprites/broken_pillar_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_3_0_e.png
c2f8913f69031471e946510a555e9d41019911761f9fb5a37ef9f7669b83d4ed  examples/01_platformer/assets/sprites/broken_pillar_big_3_0_n.png
fc4e6a41dbd49a62dfde7574b2c6d731aa115847e40524ad9df4c169179a1d1e  examples/01_platformer/assets/sprites/broken_pillar_big_3_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/broken_pillar_big_3_1_e.png
405a36e9e0812b85c7a925eb312dbf9f6206fcc755988584992989aa18ad7c20  examples/01_platformer/assets/sprites/broken_pillar_big_3_1_n.png
d71c6a5f6e1bc2383b4731054bcd7e4586f6e2cbc7d41b535dd541f452d1c2ee  examples/01_platformer/assets/sprites/channel_0.png
0b3feac32148ede77f5d065d813b590083601dc9194cdcb9b8bee5c9bb6c8421  examples/01_platformer/assets/sprites/channel_0_n.png
b31db04a2355663d43fa74ca4dcb71d862f6a3cb446d0a834a74ee256ba35fb6  examples/01_platformer/assets/sprites/channel_1.png
c10dd30f0795371785be6d7a33d0fe6648d7e323777f5bf7bdb5745081efc987  examples/01_platformer/assets/sprites/channel_1_n.png
74284c74b429e88fc9e55d8da4ec376454ac781e45de66f4c8fd21345ce00415  examples/01_platformer/assets/sprites/channel_2.png
e813df8030f23e0f822e982f0db452e48c6fde1a6f8702906370bb0c424e990e  examples/01_platformer/assets/sprites/channel_2_n.png
06ca0edafdb484cc63e4eefca664c88b8178c1775f60f090379c4b47b2d3f025  examples/01_platformer/assets/sprites/channel_3.png
15e0d0e089c0865cd20ba890f10f62472af5ad5350415506f78bd62a6ac41f0c  examples/01_platformer/assets/sprites/channel_3_n.png
b0d735215b5e8132594c9a36b080d88d7488e225e0dad009d6cff6dad067196d  examples/01_platformer/assets/sprites/channel_break_left_0.png
a73b685852017fcbc0d4450c760835348b6b25802ed0e272ac6462685573b87e  examples/01_platformer/assets/sprites/channel_break_left_0_n.png
8c6461aeaa1ddb80d93a6c8ba6f89b11df9db6d52e4e314cb6876c50b4f248c4  examples/01_platformer/assets/sprites/channel_break_left_1.png
00264546732851f91b7d31893443579a0ec8989f735b2435a40bb543e9ba2c5d  examples/01_platformer/assets/sprites/channel_break_left_1_n.png
9be54949a9747855e3adfc720f8981077ea49ab2df0b9578b952698de3d0278c  examples/01_platformer/assets/sprites/channel_break_left_2.png
ef2d1b7dc56a74c168ae783cac49dcf6027d25fed2220ce8989bf7c5d9b2d9b6  examples/01_platformer/assets/sprites/channel_break_left_2_n.png
6fe20c767c37b6d451d165d330210f843216ea2cb6672978640687cc6e75119a  examples/01_platformer/assets/sprites/channel_break_right_0.png
c0a5fc107b31a85097804724990ed8b94ec7771c546d52d8e185b6234dd3f41c  examples/01_platformer/assets/sprites/channel_break_right_0_n.png
5bb5c86d6e5c8d6d337c872b57b381a43a288397e97819028f74acd9b3d9f4fd  examples/01_platformer/assets/sprites/channel_break_right_1.png
286846c23bb094503ee704294850a2665586b677d3588891b6ef43f985d31e9b  examples/01_platformer/assets/sprites/channel_break_right_1_n.png
c190a83187e932a6113de543cacc1e1627bfea7ab3351318afdc64985d03d61b  examples/01_platformer/assets/sprites/channel_break_right_2.png
695b708dbac48ecb51194181e1e91f23bb593b329a29fe2fe2800dc99b233cd7  examples/01_platformer/assets/sprites/channel_break_right_2_n.png
133458a2e3828079e2ab897a0150b44d5b0bc056cbf4b21fc8a39cfffa9dba9a  examples/01_platformer/assets/sprites/channel_cracked.png
bc4709b5f1c5f06661b178a406ba1a53ee13daef6d127e9278416e8c7b96fcf0  examples/01_platformer/assets/sprites/channel_cracked_n.png
3421fc297de6c512816904dd95ce368231ec67cba0eb6a3964daddbeb8da421b  examples/01_platformer/assets/sprites/channel_mossy.png
e35d87c992e492173ab23aabdfb56c968434cde41d20c92fff345f7dff860558  examples/01_platformer/assets/sprites/channel_mossy_n.png
3cc83ec155ef7c444fc3e58cf1dc9ced9a85f29530ed1a9d4397c017bae8a174  examples/01_platformer/assets/sprites/channel_spout.png
a749780ebc2a2ea279a1bd36953679252b2949ed7e434e4b8e469838af6a16d3  examples/01_platformer/assets/sprites/channel_spout_n.png
af2472e58d1600d0755d902bc005827537708cb911b08720f1d7d0797608eb7a  examples/01_platformer/assets/sprites/fallen_capital.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/fallen_capital_e.png
6050c776fe8b5ec1d1ea8e4af318cdc2ff2dd36471a5d59419556cfb5946c54b  examples/01_platformer/assets/sprites/fallen_capital_n.png
6d73976f5930ec18344842f3e564604fa277e3c74eeb95f5b36bc773712584a3  examples/01_platformer/assets/sprites/gargoyle_drake_0.png
f566a692e073994da69a6ededf16e56c5deae7bea854365ac8a1be9da9e6f06c  examples/01_platformer/assets/sprites/gargoyle_drake_0_e.png
72d03c5698e75d6f97d7c7cb82459350e1fc8c96090b850f3d19759dceb589fe  examples/01_platformer/assets/sprites/gargoyle_drake_0_n.png
571e347e04f209bc87ce9a111499dd53a1e04f26b7633a5b98c8f612dbad697b  examples/01_platformer/assets/sprites/gargoyle_drake_1.png
f566a692e073994da69a6ededf16e56c5deae7bea854365ac8a1be9da9e6f06c  examples/01_platformer/assets/sprites/gargoyle_drake_1_e.png
72d03c5698e75d6f97d7c7cb82459350e1fc8c96090b850f3d19759dceb589fe  examples/01_platformer/assets/sprites/gargoyle_drake_1_n.png
6d73976f5930ec18344842f3e564604fa277e3c74eeb95f5b36bc773712584a3  examples/01_platformer/assets/sprites/gargoyle_drake_2.png
f566a692e073994da69a6ededf16e56c5deae7bea854365ac8a1be9da9e6f06c  examples/01_platformer/assets/sprites/gargoyle_drake_2_e.png
72d03c5698e75d6f97d7c7cb82459350e1fc8c96090b850f3d19759dceb589fe  examples/01_platformer/assets/sprites/gargoyle_drake_2_n.png
571e347e04f209bc87ce9a111499dd53a1e04f26b7633a5b98c8f612dbad697b  examples/01_platformer/assets/sprites/gargoyle_drake_3.png
f566a692e073994da69a6ededf16e56c5deae7bea854365ac8a1be9da9e6f06c  examples/01_platformer/assets/sprites/gargoyle_drake_3_e.png
72d03c5698e75d6f97d7c7cb82459350e1fc8c96090b850f3d19759dceb589fe  examples/01_platformer/assets/sprites/gargoyle_drake_3_n.png
0ca299f338b98888df3ad2d53c68a5b8f57a271c92800e771711efcacbdad31f  examples/01_platformer/assets/sprites/pier_base_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pier_base_1_e.png
6e4e284f72826f3f6e15ab387a7ce6e3a6acdfb712470e7b197aeb2ff0e90870  examples/01_platformer/assets/sprites/pier_base_1_n.png
f0a2a4671ed1f9ffedc2161ffd62e1ef38ded7e572104ec52af074d11211b4de  examples/01_platformer/assets/sprites/pier_base_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pier_base_2_e.png
1ae68136a8326efd59f88c42f86a7e3fab3991cf4da2d60e883b541519a0992d  examples/01_platformer/assets/sprites/pier_base_2_n.png
595e222b476c34ea1cda739bdd389bc06036fff970589dff0771dc6bbd7ab17c  examples/01_platformer/assets/sprites/pier_base_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pier_base_3_e.png
2b8993429a2dcae7a33ef2284825d876d7e05b719e1b18ac949e528770782bd8  examples/01_platformer/assets/sprites/pier_base_3_n.png
ac440fc293b8ac3ae3e53051c6ed5c822dafc4d596de38f2e17fd4b7acd59cbb  examples/01_platformer/assets/sprites/pier_wet_0.png
663e257a1c632382ce77bf8b25433da595b1a986baae27d8c0ad20ea572cab7b  examples/01_platformer/assets/sprites/pier_wet_0_n.png
0729442de56ac174cf4af9d692bccab14784bf49857027671c6b5185ec14db68  examples/01_platformer/assets/sprites/pier_wet_1.png
1a1e6545ad6b6876409ae846a2f05767acbb1f180bcdc0af6c68f7abb8ada5ba  examples/01_platformer/assets/sprites/pier_wet_1_n.png
b56ea5b62586023d9eea103c8d7c7edd993cdfe9ab90537561758920384f1794  examples/01_platformer/assets/sprites/pool_basin_big_0_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_0_0_e.png
36fe94639e66d9cc9e9b0546fd40d7ee7ad614932e4bfabaf9ba823ba0a56d40  examples/01_platformer/assets/sprites/pool_basin_big_0_0_0_n.png
27de4ab5ced3370ed116d4e2abf7a5468a782d5cae76f5d6f31b924c654ae380  examples/01_platformer/assets/sprites/pool_basin_big_0_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_0_1_e.png
c3a9b2ed01ddce0ab46e043dd4eebd292829f83afe90a2ff2427c0400fd68a90  examples/01_platformer/assets/sprites/pool_basin_big_0_0_1_n.png
ebaef55ae2075b598c1a97b358c9afeca056c0f3196656d5a4838e88ed07f610  examples/01_platformer/assets/sprites/pool_basin_big_0_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_0_2_e.png
1a72be7bd6c4fcd722add5d3bdb6d4b8299c8be9a9d0319b50a6c7133685fb0b  examples/01_platformer/assets/sprites/pool_basin_big_0_0_2_n.png
e49f8c1831a5c518652474d05dcfc3848b58eeb33dca0430bf843fff1d62f4d4  examples/01_platformer/assets/sprites/pool_basin_big_0_0_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_0_3_e.png
cb4ca4579b414b956831749ed6250c5d8d08b275d20079ddcc894e265fad013f  examples/01_platformer/assets/sprites/pool_basin_big_0_0_3_n.png
3ea05adf697ee9b24ef9dfb2872fc774e0e64aaa2f1e9e96252585ddeef05961  examples/01_platformer/assets/sprites/pool_basin_big_0_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_1_0_e.png
9823d503b69efe787bb526ad82a05f7fa127de7b04e65ffe08cfd93c056859b3  examples/01_platformer/assets/sprites/pool_basin_big_0_1_0_n.png
a776bb1620e67f0b0a810a654802751a80d7fe801dc1672947a7e92abcaac596  examples/01_platformer/assets/sprites/pool_basin_big_0_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_1_1_e.png
8cd5b54de4f38d84aada500e01dd988d7f71bf45a5ca84c5ee59645644bd29e4  examples/01_platformer/assets/sprites/pool_basin_big_0_1_1_n.png
6331f4043d48c1f4966f2b20803b27340d54a9b295b563e211ff4ff67cb903e9  examples/01_platformer/assets/sprites/pool_basin_big_0_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_1_2_e.png
772ba854c2b1eb6c3345346f80fb1deeadfe259b612505bcfa6354dc610a859c  examples/01_platformer/assets/sprites/pool_basin_big_0_1_2_n.png
8fe0f3a77a3438d462b1142f8beca1ffce0f3c158f14c0c694fb64bad984212e  examples/01_platformer/assets/sprites/pool_basin_big_0_1_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_1_3_e.png
0a70725eae2e40c5fb78105872bf913bd7370a6893042751f697f677aa53caa9  examples/01_platformer/assets/sprites/pool_basin_big_0_1_3_n.png
99e64b445fe2417b47027a8a74253837c133801ac05dc9cd6640fa809bc4e595  examples/01_platformer/assets/sprites/pool_basin_big_0_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_2_0_e.png
1452f1535a8ebeeaea6f126e2d6f8998704e476df01e7d5b564a82b2d2e8b5b7  examples/01_platformer/assets/sprites/pool_basin_big_0_2_0_n.png
c85de251f8e64ff34a88742b9137b985a2f3487e7891d5ec16424b045bf01146  examples/01_platformer/assets/sprites/pool_basin_big_0_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_2_1_e.png
61266e432b31294835e2bd8a15eab30108477127d68e28d22646f17543c663eb  examples/01_platformer/assets/sprites/pool_basin_big_0_2_1_n.png
13e45bab955aabd34164fcd8910b2bf17b1a0842f4c8f96ed4318badda88c29e  examples/01_platformer/assets/sprites/pool_basin_big_0_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_2_2_e.png
68b68eb2da642c80628433e8f04574e5ba629625c2c88468cf676fc40a9df05b  examples/01_platformer/assets/sprites/pool_basin_big_0_2_2_n.png
703d635759868ceebc39115d5b2690dcf424cf7ec433b3e8b0711be4aaedb862  examples/01_platformer/assets/sprites/pool_basin_big_0_2_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/pool_basin_big_0_2_3_e.png
9310f499dd6047f8d9545609793bc9b8b46ba3302dd4a9f3a24b1574e08c0ab8  examples/01_platformer/assets/sprites/pool_basin_big_0_2_3_n.png
ed9f0fdbfda4c4f7a9cce0dedb85f2c360222717cb4e0e6438b0cb521ad8d5ab  examples/01_platformer/assets/sprites/statue_big_0_0.png
930e2d2034d480be892ce4911395ec54fa5104b4e64ad93e5bc222cfef37f7e4  examples/01_platformer/assets/sprites/statue_big_0_0_n.png
858c12130da92799fe98fdc396ba729ca489b75f1d11a3d887ce24473b8aa469  examples/01_platformer/assets/sprites/statue_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/statue_big_0_1_e.png
678c5ef9275640e1292dbabe86c5643b4cdf8af9a115f9889f4ca2a18b465287  examples/01_platformer/assets/sprites/statue_big_0_1_n.png
e113e6248303657ab82c1b7d37b3f4d22b82989007ae83470e72c4842148ae3d  examples/01_platformer/assets/sprites/statue_big_1_0.png
ab2e707ecae27d392852ceecab5591565cb336ecdcd328a50113020068b8eca3  examples/01_platformer/assets/sprites/statue_big_1_0_n.png
d59f67f8633fff36507092ebbb38852d6d0c60e43cf8bdd02d80991f9a46514b  examples/01_platformer/assets/sprites/statue_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/statue_big_1_1_e.png
ed21cbca1489d09d1e17088b69d6890669e20f33ce847640a407ab1618191dd6  examples/01_platformer/assets/sprites/statue_big_1_1_n.png
0762cf4e85fa65eeed5fcbf0f43b80937288308725bb32f9c7ea55ac90252683  examples/01_platformer/assets/sprites/statue_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/statue_big_2_0_e.png
2d98507c65b4d5a11b741b35a3cde046e5228771f10d2df306c8a8e6687c787b  examples/01_platformer/assets/sprites/statue_big_2_0_n.png
5dd09857beffcdc2cb9585ede6c17011113ceb783263142a98c8f3ebdf494d33  examples/01_platformer/assets/sprites/statue_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/statue_big_2_1_e.png
6735beeeb81090ea021a26b53b467600f124e6cc1923f909a6c2ea7b60a92871  examples/01_platformer/assets/sprites/statue_big_2_1_n.png
88091a6cb2f73e346ff863635da44691e133714b812a474d4094f7a61bb9ad81  examples/01_platformer/assets/sprites/water_0.png
0e0d99857285bae7792c8939c8d423737d75d6ca86311c44b3c80f9045728319  examples/01_platformer/assets/sprites/water_0_n.png
947462a68dac06a760858b93b0861b5e24613a47ccb706653977a44323119851  examples/01_platformer/assets/sprites/water_1.png
2878725f850361e547a0f1bbbc78d1cd16c0d2426f9bb1d61a6c398c38522617  examples/01_platformer/assets/sprites/water_1_n.png
7500d419b0728357f4dc171e5018d5a72262171dcf9ceab675d7ce54d164abd5  examples/01_platformer/assets/sprites/water_2.png
1c9b849582a7cc3d11b6a80b2650048eb4ce938cd2089321e69a857f8ea7dfc8  examples/01_platformer/assets/sprites/water_2_n.png
da747534e0a99151f55bb2c9b864a72195097e75f10635462388c1e51c5fa95e  examples/01_platformer/assets/sprites/water_3.png
aceef0274587b653f53b63363a977f49105445a3acba1a70cc4ab76c28759658  examples/01_platformer/assets/sprites/water_3_n.png
5d25e1974cb213701ca8470eb91575d7ebace6f4ebb30ce604c9ec24f453ae98  examples/01_platformer/assets/sprites/water_4.png
0e0d99857285bae7792c8939c8d423737d75d6ca86311c44b3c80f9045728319  examples/01_platformer/assets/sprites/water_4_n.png
b00a706fe5e8d1f09f2bd5ec40e345e681d9bc65a3a5003db48b1503f59fd1b7  examples/01_platformer/assets/sprites/water_5.png
2878725f850361e547a0f1bbbc78d1cd16c0d2426f9bb1d61a6c398c38522617  examples/01_platformer/assets/sprites/water_5_n.png
96a0f20a1db512c6edc1a7c279ec9411759b5e1ad91cf8cbce18a903b414f397  examples/01_platformer/assets/sprites/water_6.png
1c9b849582a7cc3d11b6a80b2650048eb4ce938cd2089321e69a857f8ea7dfc8  examples/01_platformer/assets/sprites/water_6_n.png
e5e58505222347c4d3421b82508f2ec54f5b644b685e85b1ae4cd2ef75750c84  examples/01_platformer/assets/sprites/water_7.png
aceef0274587b653f53b63363a977f49105445a3acba1a70cc4ab76c28759658  examples/01_platformer/assets/sprites/water_7_n.png
```

### 3.7 The asset sessions' prompts

A run session that reaches Step 5 without its evidence row ends with the first, for a new session on Opus 5.5 at max; Step 5's session ends with the second; Step 6's with the run prompt for Step 7.

`Use the tungsten-milestone skill. Read docs/plans/e01-hollow.md and run Step 5 alone, the aqueduct's asset session: draw §3.2's aqueduct as it directs, within the contract, with the generator's own toolkit; regenerate; link me §3.5's review sheet and take my notes in rounds. On my acceptance write §3.6's lock lines and the evidence row, delete the scratch driver and the venv, commit and push under docs/operating-rules.md. Leave src/ and tests/ alone; end with Step 6's prompt.`

`Use the tungsten-milestone skill. Read docs/plans/e01-hollow.md and run Step 6 alone, the hollow's asset session: draw §3.1's geyser and stone families and §3.2's hollow in place of the placeholders as it directs, within the contract, with the generator's own toolkit, and the three sounds; regenerate; link me §3.5's review sheet with in-game loops and take my notes in rounds. On my acceptance write §3.6's lock lines and the evidence row, delete the scratch driver and the venv, commit and push under docs/operating-rules.md. Leave src/ and tests/ alone; end with the run prompt for Step 7.`

## 4. Steps

Every code step's gate also takes `cargo fmt --all -- --check` → exit 0, `cargo clippy -p example-01-platformer --all-targets --locked -- -D warnings` → exit 0, and `cargo test -p example-01-platformer --locked` → pass (01's whole suite, its count in the evidence; 400 game tests at `593bcdd`). A step that regenerates `assets/` makes a scratch Pillow venv once a session (`python3 -m venv "$VENV" && "$VENV/bin/pip" install -r examples/01_platformer/tools/requirements.txt`, `$VENV` in the scratchpad, removed with `shutil.rmtree` at the session's end) and passes `PATH="$VENV/bin:$PATH" just level-check` → pass, about five minutes, run in the background with the step's paths staged first; edit only the next step's `src/` and `tests/` meanwhile, never `tools/`. Filtered runs name the step's new tests in the evidence, since a filter that matches nothing still exits 0. Commit and push in separate calls.

### Step 1: Placeholders for §3's contract

- **Files:** Step 1's in the header.
- **Change:** §3.4: each new name's size in `art.py`'s `SIZE_PREFIXES`, which `validate_art` holds every image to (64 × 64 when undeclared); every §3.1 file from today's art (the vents and plume frames where the generator keeps the lavafall's clips, the stones beside `goblin_rock` in `actors.py`, the configs in `particle_configs`, the sounds in `build_sounds` and `sounds.json`); the contract test; a line in `tools/README.md` naming them placeholders until Step 6; regenerate.
- **Done-when:** `PATH="$VENV/bin:$PATH" just level-check` → pass, `HollowContractTests` among its tests; `cargo test -p example-01-platformer --locked` → pass (no code change); `just repo-check` → pass; `git status --porcelain` → only Step 1's paths.
- **Commit:** `feat(example-01): placeholders for the hollow's geysers, stones and sounds`
- **Moves:** `assets/tilemaps/level.tmj`'s tile IDs, which new sprites shift; the manifest.

### Step 2: The geyser (V11)

- **Files:** Step 2's in the header.
- **Change:** §2.1 without its save; `Sound::GeyserSputter` and `GeyserErupt` (§2.5); 01's README mechanics row for the geyser; regenerate for the prefab.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- hazards::` → pass, with new tests on a geyser spawned on the tests' ground: its touch is off through 2.5 s idle and 0.6 s sputter, on for the 0.8 s eruption and off after; the player standing on it when it erupts takes 12 fire, burns and leaves upward faster than 500 px/s; a wisp (fire resist 0) over it takes no damage and does not burn; a geyser with `offset` 2.6 erupts within 0.6 s of the start; the ice spray held on its vent chokes it within 0.6 s and it does not erupt for the next 6 s, then idles 2.5 s before its sputter; the level check (the prefab); the gate.
- **Commit:** `feat(example-01): fire geysers that erupt, burn and throw`

### Step 3: The stones (V15)

- **Files:** Step 3's in the header.
- **Change:** §2.2 without its placement: `PropKind::Stone`, `Stone`, the three prefabs, every arm as a rock's, `Sound::StoneKnock` and its rule, `SaveGame::stones`; 01's README mechanics and saves rows.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- props:: saves::` → pass, with new tests that each size spawns at its mass and box and rests on the ground; the force push throws a large stone into the dummy, which takes physical damage; the flamethrower held on a stone lights nothing; a stone landing from 300 px plays `StoneKnock`; three stones of three sizes save through JSON and load back at their places, sizes and variants; `every_component_on_a_saved_entity_is_saved_rebuilt_or_presentation` passing with `Stone` listed; the level check; the gate.
- **Commit:** `feat(example-01): rubble stones in three sizes`

### Step 4: The hollow's map and schema 19 (V8, V12, V13, V16)

- **Files:** Step 4's in the header.
- **Change:** §2.3 and §2.4: the lintel and bat c; the three foes; the geysers' layer, their layout check and §2.1's save; the stones' entries, markers and fresh-run spawn; `calm`; `AMBIENT_INDOOR`; schema 19 with `to_19`, the frozen constants and `map_ids`' two lists; the tests of A16 and A17; 01's README hollow, enemies and saves rows (twenty-nine enemies, the hollow's goblin, bat and four wisps, the geysers and stones, schema 19); regenerate.
- **Done-when:** `jq '([.enemies[] | select(.kind == "bat")] | length), (.enemies | length), (.geysers | length), (.stones | length)' examples/01_platformer/tools/level.json` → 8, 29, 3, 9; `rg -c 'const SCHEMA: u32 = 19;' examples/01_platformer/src/save.rs` → 1; `cargo test -p example-01-platformer --locked --test game -- saves:: enemies:: dummy:: hazards:: props:: level:: playthrough:: areas::` → pass, with new tests that the map's three geysers take the save IDs after the orbs and the player's is the dummy's + 1 + 29 + 8 + 3; a geyser saved mid-eruption and one saved choked load back to finish as they would have; a fresh run sets nine stones down and none in a run loaded from a schema 18 save; `as_schema_18` of a capture migrates back to that capture without the new objects and loads with the new enemies at home and bat c on its new roost; the schema 11 fixture's bat c migrates with its home at (13440, 3740); a schema 18 save with a bowling ball inside the lintel's new row loads it under the row; the ice spray aimed at `hollow_floor_1`'s map geyser chokes it; `the_maps_bats_hang_at_their_roosts_from_the_start` at 8 bats, every `LAUNCHES` row and the playthrough passing with their rows unchanged or re-derived (§6); `cargo test -p example-01-platformer --locked --test game -- saves::pit_full_saves_within_two_mib --nocapture` → pass, its bytes quoted, at most 2,097,152; the level check (the light budget, the bat, the geysers' supports); the gate.
- **Commit:** `feat(example-01): the hollow's geysers, stones, foes and mouth`
- **Moves:** 01's save schema, 18 to 19; the save IDs from the orbs on (+3) and from the player on (+6); `MAP_ENEMIES` 26 to 29; the map's bats 7 to 8; `east_wall`'s bottom and ravine bat c's row; `AMBIENT_INDOOR`.

### Step 5: The aqueduct's art (the first asset session)

- **Who:** a new session on Opus 5.5 at max, on §3 alone, after Step 4 (Q1); the owner reviews.
- **Files:** Step 5's in the header.
- **Change:** draw §3.2's aqueduct; `tools/README.md`'s rows; regenerate; §3.5's sheet; the owner's notes in rounds, a new small session when the context grows; on acceptance §3.6's lines, the evidence row, the scratch driver and the venv deleted.
- **Done-when:** the owner's acceptance quoted in the evidence row; `jq -e '((.spouts // []) | length) == 0 and ([.props[] | select(.sprite | test("gargoyle"))] | length) >= 2' examples/01_platformer/tools/level.json` → exit 0; `PATH="$VENV/bin:$PATH" just level-check` → pass; `cargo test -p example-01-platformer --locked` → pass; `just repo-check` → pass; §3.6's check → exit 0; `git status --porcelain` → only Step 5's paths.
- **Commit:** `feat(example-01): the aqueduct's scenery, gargoyle and piers`

### Step 6: The hollow's art and sound (the second asset session)

- **Who:** a new session on Opus 5.5 at max, on §3 alone, after Step 5; the owner reviews.
- **Files:** Step 6's in the header.
- **Change:** draw §3.1's geyser and stone families and §3.2's hollow in place of the placeholders; the three sounds in `sfx.py`, their volumes in `sounds.json`; `tools/README.md`'s rows; regenerate; §3.5's sheet with its loops and playlist; the owner's notes in rounds; on acceptance §3.3's measurements, the owner's word on the ambient, §3.6's lines, the evidence row, the scratch driver and the venv deleted.
- **Done-when:** the owner's acceptance quoted in the evidence row; `PATH="$VENV/bin:$PATH" just level-check` → pass, `HollowContractTests` among its tests; `cargo test -p example-01-platformer --locked` → pass; `just repo-check` → pass; §3.6's check → exit 0 with Step 5's lines unchanged; `git status --porcelain` → only Step 6's paths.
- **Commit:** `feat(example-01): the hollow's mouth, cave, geysers and stones art`

### Step 7: Close

- **Files:** Step 7's in the header.
- **Change:** run the checks and the demo; in-game captures of §3.5's scenes on the accepted art, by its driver, for the owner; a one-constant `AMBIENT_INDOOR` change if Step 6's row records one; `DESIGN.md`'s workspace line gains the geysers and the stones in 01's clause; §3.11 of the feedback gains its landed note, as §3.10's has; open follow-ups to Later or known issues; `e01hollow` out of Now and `e01flask` into it; §3.6's check, then this plan deleted with its citations; the venv removed.
- **Done-when:** §3.6's check → exit 0 before the deletion; `just check` → pass (formatting, lints and every crate's tests, 01's whole suite among them); `PATH="$VENV/bin:$PATH" just level-check` → pass; `just smoke` → pass; `just repo-check` → pass; the captures named in the last evidence row.
- **Commit:** `docs(example-01): records follow e01hollow`
- **Ends with:** the commits made, the evidence quoted, what the owner should play (in through the mouth at night, the three geysers with the ice spray, the force push into a heap of stones and the shelf's goblin, a walk along the aqueduct's floor past the pool and the second gargoyle) and the next prompt.

## 5. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The sessions' order (§3.11 says the plan session draws the sheet first) | The code session runs Steps 1–4, then asset sessions Step 5 (the aqueduct) and Step 6 (the hollow), then a new code session Step 7, as `e01foes` and `e01sfx` ran; two asset sessions, since the two areas' art outgrows one session's context. This session wrote the brief and drew nothing |
| Q2 | Where the geysers sit among the map's objects | Their own `geysers` layer after `tomes`, so their IDs follow the orbs'; in `hazards` they would move the decks, the dummy and every enemy too (A2) |
| Q3 | The geyser's numbers past Q65 | Touch 12 fire every 0.5 s, knockback 1,000 px/s with `push_always`, its box 176 px each way from the vent's floor point; offsets 0, 1.3 and 2.6 s |
| Q4 | "12 fire and burning" when 12 burns no player by the ladder (A6) | The touch keeps 12 and `reactions` applies `burning` from a geyser's fire part, unless the target is burning, frozen or doused, or took no fire |
| Q5 | The choke | As lava's crust: 0.5 s of cold spray on the vent chokes it for 6 s, cutting an eruption short with steam and `Steamed`; then 2.5 s idle |
| Q6 | The geysers' light | None: a point light each would pass the budget (A8); the plume glows by emissive colour and bloom |
| Q7 | Does an eruption light or throw props and balls | No: its touch meets live targets, as lava's bite; a Later line if play asks |
| Q8 | The stones' numbers (Q66's two or three sizes, mass 10–20) | Three sizes, boxes of mass 10, 15 and 20, two looks each, nine in three heaps, no cap, never summoned, as a goblin's rock in every rule but its shape and cap |
| Q9 | Stones in a run from an older save | None, as a schema 11 save's run has no crates (`to_12`) |
| Q10 | The new foes' places | §2.3's: the goblin on the far shelf, the bat over the step, the wisp over the shelf's lava; the run may move one up to 2 tiles to pass a check, recorded |
| Q11 | The lintel when a row fails | The lintel stays at 57 and the step records it, the rest of Step 4 landing |
| Q12 | `AMBIENT_INDOOR` | (0.22, 0.205, 0.235), today's hue scaled; the owner's word in Step 6 sets it at Step 7 |
| Q13 | New sounds | Three: `geyser_sputter`, `geyser_erupt`, `stone_knock`; the choke reuses `Steamed` |
| Q14 | Which spout the second gargoyle replaces (Q61) | The middle one at x 137, between the first gargoyle (141) and the west gaps |
| Q15 | Route and area tests through the hollow | `calm` takes the geysers and stones off with the enemies, so their rows stay as they are |
| Q16 | A decision entry | None: no dependency, policy or format beyond a migrated schema, as e01sfx's schema 18 took none |
| Q17 | The relabel tests under an ID shift | `as_schema_18` turns a capture into the 0.110 code's before each relabel, so each still tests the migration it names |
| Q18 | New sprites in the asset steps | Allowed: no code reads a tile ID; only §3.1's files are a contract |

Each question takes its stated default; the owner reviews the plan on their own time.

## 6. Invariants and stop conditions

- **May move:** `level.tmj`'s tile IDs and the manifest (Steps 1–6); the prefabs and object layers (Steps 2–4); 01's save schema, 18 to 19, the IDs from the orbs on, `MAP_ENEMIES`, the bats' count, `east_wall`'s bottom, ravine bat c's row and `AMBIENT_INDOOR` (Step 4, the ambient again at Step 7 by the owner's word); `Sound::ALL`, 84 to 87; `tools/level.json`'s dressing (Steps 5–6).
- **Must not move:** the kit hash and every kit, core, render and template byte; the determinism and containment hashes and the row digests; `api/`; `gpu-visual.png` and the post and transition regressions; 01's `src/shell/`, byte for byte the template's; the `S9_*`, `S10_*`, `S11_*`, `S15_*` and `S16_*` constants; every `LAUNCHES`, `TOME_LAUNCHES` and playthrough row but one the lintel re-derives; the tomes, their supports and the routes; the light budget's 10 and the 2 MiB save bound; §3.6's files after their asset step.
- A done-when check fails: restore the step's files from the last commit, then `git add` each restored path and `git rm --cached` each file the step added, so the index holds no part of it (`git diff --cached --quiet` → exit 0, since §4 stages a step before its level-check); mark the step skipped and carry on with the steps that don't depend on it (2 and 3 need 1; 4 needs 2 and 3; 5 needs 4's level; 6 needs 2–4 for its loops and runs on what landed, telling the owner which loops are missing; 7 needs 6's lock).
- A route, playthrough or area row fails after the lintel moves: re-derive that row's columns once; if it still fails, the lintel stays at 57 (Q11).
- A test outside the step's list fails because a geyser or a stone met its scripted walk: it calls `calm` or moves off them, recorded; a test that needs anything else skips the step.
- A full pit's save passes 2 MiB, the light budget's check fails, or the kit hash moves: the step restores and stops.
- §3.6's check fails after a regeneration: an accepted file changed, which only its asset step may change; restore the step's files.
- An asset step needs a contract change (a size, a box, a frame count, a cap, a length or a new ID the code reads): it records it in its evidence row and leaves `src/` alone; Step 7 takes a one-constant change, anything more goes to Later.
- A file outside the step's list needs a change: a test that pins a value the step moves is added and recorded; anything else skips the step.
- The Pillow venv will not install: stop, since the generator cannot run.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-27, on the plan at `593bcdd` with §7 and the log cut; `418.0 s, tokens in 1099619 (cached 1006592), out 9773, commands 31, output 324.3 KB, usage 5h 0% → 24%, week 22% → 26%`. Three findings, all checked true and accepted; the first two changed done-when checks, so round 2 ran. Round 2, the same settings with the same cut and a focus note of round 1's changes: `363.8 s, tokens in 799456 (cached 721408), out 6960, commands 15, output 159.6 KB, usage 5h 24% → 43%, week 26% → 29%`; one finding, checked true and accepted. There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Schema 19 restores the existing bat inside the lowered lintel | `src/save.rs:2116-2125` (`restore_enemy` places the saved position and inserts the saved `Enemy`, whose `home` is a field, `src/features/enemies/components.rs:60`); bat c is the sixth enemy, save ID 14; `to_16` (`src/save.rs:740-797`) is the model for bodies in a new solid | accepted | §2.4: `to_19` moves bat c's home and position and sets bodies in the row under it (`S19_LINTEL`, `S18_BAT_C`), `bodies()` reading planks; `as_schema_18` puts bat c back at its 0.110 roost; Step 4's done-when adds the bat and a ball in the row |
| 2 | [HIGH] The map placement puts the geyser's spray target underground | §2.3 said "the vent's top-left on the support's row" against §3.1's "bottom row on the floor's top"; cold rays stop at tiles (`src/features/toys/systems.rs:1137-1150`) | accepted | §2.1 and §2.3: one origin, the vent's top-left at (x × 64 − 32, row × 64 − 32), the touch about (32, 32) and the vent box about (32, 20) from it; Step 4's done-when adds a choke on a map geyser |
| 3 | [MEDIUM] Step 1 omits the size declarations required by its own gate | `tools/art.py:97-100` (`expected_size` from `SIZE_PREFIXES`), `:236` (`validate_art` refuses another size), `tools/generate.py:719` | accepted | Step 1's files and change take `art.py`'s `SIZE_PREFIXES` |
| 2.1 | [MEDIUM] Moving bat c's saved home down 64 pixels does not put every supported save at the new roost | `tests/game/fixtures/schema{8..13}.json` (bat c, ID 14 or 865, home y 3752.8); the new roost 58 × 64 + 28 = 3740 (`src/save.rs:218`, the bat's centre y 28); `restore_enemy` (`src/save.rs:2124`); `moved_ids` panics on a non-ID change (`tests/game/saves.rs:2103-2112`) | accepted | §2.4: bat c's home set to the frozen `S19_BAT_C_HOME`, its position moved only when it meets the row; an `as_schema_19` helper for the fixture comparisons; Step 4's done-when adds the schema 11 fixture's bat c at the new home |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-27 | pass | `just level-check`: `OK: 5088 owned outputs match; art, level, references and budgets validated`, `Ran 34 tests … OK` (`HollowContractTests`' 3 among them); `cargo test -p example-01-platformer --locked`: 399 passed, 1 ignored; `just repo-check` → exit 0; `git status --porcelain` → Step 1's paths only. Before it, two fixes the gates needed, each its own commit: HEAD's `just level-check` failed, since `593bcdd` changed only the generated `assets/prefabs/fire_missile.json`, so `491cdb8` set its source in `tools/content/` to the same (8, 8); and two tests failed at HEAD on `593bcdd` and `1664bec`, which `59bcef8` follows (`fixed_step`'s trail offset, `presentation`'s plough box at the beetle's drawn point). The stones are lit, as the contract's "normal and emissive maps" and the crates are, not unlit as the goblin's rock | the generated `assets/`; `tools/{art,scenery,actors,generate,sfx}.py`, `tools/sounds.json`, `tools/test_generate.py`, `tools/README.md` |
| 2 | 2026-10-27 | pass | `cargo test -p example-01-platformer --locked --test game -- hazards::` → 16 passed, the new `a_geysers_touch_is_off_through_its_idle_and_sputter_and_on_for_its_eruption` (48 steps on, the first within 185–187), `a_geysers_eruption_burns_the_player_standing_on_it_and_throws_them_up` (12 fire, burning, faster than 500 px/s up), `a_wisp_over_an_erupting_geyser_takes_no_fire_and_does_not_burn` and `the_ice_spray_on_a_geysers_vent_chokes_it_for_six_seconds_then_it_idles` among them; `just level-check`: `OK: 5089 owned outputs match`, `Ran 34 tests … OK`; fmt and clippy → exit 0; the suite: 403 passed, 1 ignored. The plume and embers are children `geyser::start` hangs, not prefab children, so the step adds one prefab; `geyser_look` runs after the physics sync and before the frame's particles, where the embers' drain must land, rather than in the present phase | `src/features/level/{geyser,mod}.rs`, `src/features/{toys,combat}/systems.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/prefabs/geyser.json`, `tools/content/data/cue_rules/sounds.json`, `tests/game/hazards.rs`, `README.md`, the generated `assets/` |
| 3 | 2026-10-27 | pass | `cargo test -p example-01-platformer --locked --test game -- props:: saves::` → 20 and 47 passed, the new `each_stone_size_spawns_at_its_mass_and_box_and_rests_on_the_ground`, `the_force_push_throws_a_large_stone_off_a_ledge_onto_the_dummy`, `the_flamethrower_held_on_a_stone_lights_nothing`, `a_stone_landing_from_three_hundred_pixels_knocks` and `three_stones_of_three_sizes_save_through_json_and_load_back` among them, `every_component_on_a_saved_entity_is_saved_rebuilt_or_presentation` with `stone` listed (`busy` sets a stone down); `just level-check`: `OK: 5092 owned outputs match`, `Ran 34 tests … OK`; fmt and clippy → exit 0; the suite: 408 passed, 1 ignored. Deviations: the force push gives a body 2,700 / mass px/s (`toys::wave`), 135 for a large stone, under the 450 a mass of 20 needs to hurt (`body_hit`), so the test pushes the stone off a ledge 300 px over the dummy, which takes its blow as it lands; a goblin's rock freezes (it is a prop, `elements::systems::is_body`), so the stones' "unfreezable" is a rule of their own (`elements::systems::freezes`), not the rock's | `src/features/props/{mod,components,systems}.rs`, `src/features/elements/{systems,cascade}.rs`, `src/features/level/lava.rs`, `src/features/toys/systems.rs`, `src/save.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/prefabs/stone_{small,medium,large}.json`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{props,saves}.rs`, `README.md`, the generated `assets/` |
| 4 | 2026-10-27 | pass | `jq … tools/level.json` → 8, 29, 3, 9; `rg -c 'const SCHEMA: u32 = 19;' src/save.rs` → 1; `cargo test … -- saves:: enemies:: dummy:: hazards:: props:: level:: playthrough:: areas::` → 204 passed, the new `the_maps_geysers_take_the_save_ids_after_the_orbs`, `a_geyser_saved_mid_eruption_and_one_saved_choked_load_back_to_finish_as_they_would_have`, `a_fresh_run_sets_nine_stones_down_and_a_run_from_a_schema_18_save_none`, `a_schema_18_capture_migrates_to_itself_without_the_new_objects` (the new enemies at home, bat c on its roost, the geysers at their offsets), `the_schema_11_fixtures_bat_c_migrates_to_its_new_roost`, `a_schema_18_save_with_a_ball_in_the_lintels_new_row_loads_it_under_the_row` and `the_ice_spray_aimed_at_the_first_floors_map_geyser_chokes_it` among them, `the_maps_bats_hang_at_their_roosts_from_the_start` at 8 bats; every `LAUNCHES`, playthrough and areas row passed unchanged with the lintel at 58, none re-derived; `saves::pit_full_saves_within_two_mib` → pass, `1801279 bytes (1.72 MiB)`; `just level-check`: `OK: 5092 owned outputs match; art, level, references and budgets validated`, `Ran 34 tests … OK`; `just repo-check` → exit 0; fmt and clippy → exit 0; the suite: 415 passed, 1 ignored. Deviations: four tests outside the list (`ball_pit::relative_body_impacts_work_on_b_side_and_dense_contacts_remain_bounded`, `presentation::jump_landing_bursts_fire_once_stay_at_event_and_cleanup`, `presentation::particle_caps_and_ambient_placements_remain_bounded`, `spells::fireball_blast_layers_emit_particles_and_drain_within_the_shared_budget`) count every live burst, which the map's geysers keep spawning, so each calls `calm` (§6); `tools/README.md` gains the `stones` and `geysers` keys and layer, which Step 4 makes its tables wrong without (operating rules, records); `save::map_ids` is public for `as_schema_18`, and also walks `/geysers` and `/stones`; a geyser loaded from a schema 18 save starts at its offset, so the third is in its sputter, not idle | `tools/{level.json,level.py,placement.py,README.md}`, `src/features/level/mod.rs`, `src/features/run/mod.rs`, `src/save.rs`, `tests/game/{main,saves,enemies,dummy,hazards,ball_pit,presentation,spells}.rs`, `README.md`, the generated `assets/` |
| 5 | 2026-10-27 | pass | The owner's acceptance, round 3: "Accepted"; round 1's notes "Do a pass at the satue, it looks bad. Fully redo it. The Amphora as well.", round 2's "Do one more pass at the satue, fix it's neck and anatomy". `jq -e '((.spouts // []) | length) == 0 and …' tools/level.json` → exit 0 (`ex10_gargoyle_0`, `ex10_gargoyle_drake_0`); `just level-check`: `OK: 5232 owned outputs match; art, level, references and budgets validated`, `Ran 34 tests … OK` (each round's passed); `cargo test -p example-01-platformer --locked`: 415 passed, 1 ignored; `just repo-check` → exit 0; §3.6's check → exit 0 over 194 lines, every sprite the step adds or changes (no particle or sound); `git status --porcelain` → Step 5's paths only; §3.3's measurements are Step 6's (no stone, plume or sound here). Landed: 8 streams, the spouts gone and a second gargoyle, a drake in profile, at x 137 with its own pour; a coping course along the channel's top; the oculus pier top on 5 piers from 96; 4 pier bases mixed; stains on 2 of the 6 gap piers (108, 148), half the dither, a step darker; the aqueduct's arch streaks thinned (the bridge's front arcade unchanged); a broken pillar on the step at 93; a pool under the tome's arch (116–119) with wet stones, reeds and lily pads; 13 paving pieces, a fallen capital, an amphora, undergrowth, rubble and hanging moss; the statue redrawn as a water-bearer, 2 x 3 tiles at 153.5. Deviations: the floor variants are a prop family laid in the turf (`aq_paving_<k>`, `y` written 6 px into the floor), since terrain variants need `level.py`, outside the step's files; the coping is drawn in the channel family's tiles, the `water` clip stopping behind it, not a tile of its own; `pixels.packed_stones` is the one new helper (scenery.py's `rubble` prop holds that name); `placement.py` maps `pier_top_<n>` and `pier_base_<k>`; the statue and the amphora were redrawn at the owner's notes; the rubble at 118.2 went for the pool; `level.tmj`'s tile IDs moved (Q18) | `tools/{backdrops,terrain,scenery,placement,pixels}.py`, `tools/level.json`, `tools/README.md`, the generated `assets/`, this plan's §3.6 and log; the review in `perf-runs/20261027-e01hollow-aqueduct-review/` (`contact.png`, `pairs/`, `after/`, `after_r1/`, `after_r2/`, `tools/`) |

## Follow-ups

None yet.
