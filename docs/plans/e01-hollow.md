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
5b8869e46075bf39106116c2df53205f2f6c6d533edc8e3900e301355c6c9f43  examples/01_platformer/assets/particles/geyser_burst.json
9edb06709c9dd4b5e73baffc7e14ca40c32a60bf840bf62140720b647f52ad5b  examples/01_platformer/assets/particles/geyser_embers.json
e01b9c6a7b82d33123884c79251b9084e0c92ee7277d35cdca24584fb7f7e118  examples/01_platformer/assets/sounds/geyser_erupt.wav
b2b618153a5e19342815ae24c2d1ec8af770398b692b2e4d1e4ed5c978852024  examples/01_platformer/assets/sounds/geyser_sputter.wav
c406b76f61c858487037db0ecc0bbee6aff304ed5dcc8d5838b37d94357b0b73  examples/01_platformer/assets/sounds/stone_knock.wav
9afa386dd1ffea698779d733d6f565db2f6df1d8184dad926186afa71b333789  examples/01_platformer/assets/sprites/basalt_0.png
d78285d745f15bfd4bc782c95d8cb55e4ada6c1df64069e889468804de3ca110  examples/01_platformer/assets/sprites/basalt_0_n.png
98126af281178eb87920c78e275804224e5af36e46132b9572125a4f641c24ed  examples/01_platformer/assets/sprites/basalt_1.png
2ddc84dbf51504ae5fe1900f412b3c1c684c975d92653069f776864ae452dd17  examples/01_platformer/assets/sprites/basalt_10.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_10_e.png
56b25436028026c3124346ca6cd3ca07ce9e2abe92af6dc1955ce0a86c79c19f  examples/01_platformer/assets/sprites/basalt_10_n.png
feee6bf9b372fc4f4a30b041c0b55155267ad29a3c76ab81e7063c406e30a9a7  examples/01_platformer/assets/sprites/basalt_11.png
dbbda2a505edad2b88ea7ac685832d5bfa0be605ba8e7da802e69edf10c968fd  examples/01_platformer/assets/sprites/basalt_11_n.png
94d008babf99554b3a9ccb5a601c4521b0d4872c98baa171fb8eb37eb33b3ab4  examples/01_platformer/assets/sprites/basalt_12.png
105cc13cb7a7e5a1a67bc15bd4b27452228bd67e9900fd74a8001aeed45f74ed  examples/01_platformer/assets/sprites/basalt_12_e.png
8b14913047615d768badf9d2de3f4eea643fdc8ed1a42adb1f213694089af30d  examples/01_platformer/assets/sprites/basalt_12_n.png
a12aa0d2b8952e75597baab583767f0e09e249f3d69bcf341597b44ededbe662  examples/01_platformer/assets/sprites/basalt_13.png
4b387f4873a06bd510249a1ab2ee68895b96e0c9afc8cf706b85df192771559b  examples/01_platformer/assets/sprites/basalt_13_e.png
c63ed600546b0949e95a56dd3d15e5c6ade7ed2fa1a1f915e8ffbd09e7429eb8  examples/01_platformer/assets/sprites/basalt_13_n.png
930af37bcb272906bd36b0c7bab221487dd85dce211e8bdc88bb5c5189e728f6  examples/01_platformer/assets/sprites/basalt_14.png
7535017e649ea2befd607610bf248f4652da98e89af943f6180d3abf532976ed  examples/01_platformer/assets/sprites/basalt_14_n.png
8af2d9a2c8b1c8f4ccf7ce214d782e9b6396f5f516af1866ef2fce2402a4c227  examples/01_platformer/assets/sprites/basalt_15.png
c28a5727656f4db544490e501f4a08404ce33aac4a09dfff5a4487a06b9cd36f  examples/01_platformer/assets/sprites/basalt_15_n.png
d294c8bc239e9b48344054ccb1aa0d682283a23390c69bf1f7b454dd46015c76  examples/01_platformer/assets/sprites/basalt_1_n.png
b9ff1b3683a0df1a455ba7c40a4104a230033468d4c5548f79c50abdc8a1d214  examples/01_platformer/assets/sprites/basalt_2.png
61fcbc16844b45a273e7f3f7e43b4166f10c4ff869cd4a9925215c476a7f21a7  examples/01_platformer/assets/sprites/basalt_2_n.png
29ea62d871ae799799fe0565f804f7b2f615a8b5fa825d4646b83c438af2a210  examples/01_platformer/assets/sprites/basalt_3.png
e110596a4531f4fc9da02e6ba8dd1a4e268392be10c7cb6d67984b8e08241591  examples/01_platformer/assets/sprites/basalt_3_n.png
d0b8157f5327e1534686898aa137dd4afe612b7db878e06e8fd4f206dbe44053  examples/01_platformer/assets/sprites/basalt_4.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_4_e.png
d00b30da0eb5db51351e42e4eda65c731340795ab7a409a373971148beea995b  examples/01_platformer/assets/sprites/basalt_4_n.png
ac8aa629b6ecb7c4f5be6a079aa17bc647ca8462ad00f5fd0c000c9bd4d3b156  examples/01_platformer/assets/sprites/basalt_5.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_5_e.png
3ff1c4107d025177ecbc4bdbc1a5be644f8166cf4e30b447db383a8aa1d40c1b  examples/01_platformer/assets/sprites/basalt_5_n.png
1c229b531c69b0db2641ba98c84859b04557824820f50681c672e699aa764f45  examples/01_platformer/assets/sprites/basalt_6.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_6_e.png
bc66b2a21276e22bcde5003bc592190147e514efdb42178bb058d7d197c2327d  examples/01_platformer/assets/sprites/basalt_6_n.png
2558030a63160f64871c26fdf2f08305301956a05173200de49d1f33516cb57a  examples/01_platformer/assets/sprites/basalt_7.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_7_e.png
d0babc937551769cd6b2f39f2b02252b596c500a4e967220ce9275a7805e41a2  examples/01_platformer/assets/sprites/basalt_7_n.png
50b9206fe786c65260b404b16fe3df5ff5c3cb3c0b0b17f730ea13142bf943df  examples/01_platformer/assets/sprites/basalt_8.png
96e0130c08bada843bb474f814b284d3e4650a62f0ea3ecc7b10e88188b47826  examples/01_platformer/assets/sprites/basalt_8_n.png
4d3b4ac8d98f086237e87fb5210f93a3c44dbce71efb91a35e84b5372c23abb8  examples/01_platformer/assets/sprites/basalt_9.png
7967a74259b907251db859a37e8fb064fae7305181d8a737a2aecb86705320e7  examples/01_platformer/assets/sprites/basalt_9_n.png
d12721caab906c7412dc39c771de5996ea8e58594da0208a77d1285e6371636e  examples/01_platformer/assets/sprites/basalt_cliff_left.png
606cb0e1abce521c9f0426c877406f69a34a7a919bfa5673709d838ffe506538  examples/01_platformer/assets/sprites/basalt_cliff_left_n.png
7973f5e3f86c48256d08a7f41289e01535588c15b6a7714a97b8b9ba23b4d847  examples/01_platformer/assets/sprites/basalt_cliff_right.png
14b18c4f2ef0639d7e566cf191da6cfbc50c65d59458270e6b83b15fef042a24  examples/01_platformer/assets/sprites/basalt_cliff_right_n.png
8cc5611cdb56c13400ad9f8c888d3822bc0b2ecc0fe08b710693f97a26f9459b  examples/01_platformer/assets/sprites/basalt_rubble_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_rubble_0_e.png
ba83c5ae346029f338086f9d2fe0bc39363bfa0d042353f932e619f6a8f6e8d0  examples/01_platformer/assets/sprites/basalt_rubble_0_n.png
22d28ba3a1c36e83f8a23f85b19b83f0d1ae499f896e66b4a9576912bc13267d  examples/01_platformer/assets/sprites/basalt_rubble_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_rubble_1_e.png
efaaf088067869854c5a2e4e26783d81a1195afff11fc780af0f5339be5f3964  examples/01_platformer/assets/sprites/basalt_rubble_1_n.png
e230f8102ef221e72dc980d33802aa8bfd892dc09a412e17578a5ef901d12b07  examples/01_platformer/assets/sprites/basalt_rubble_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/basalt_rubble_2_e.png
e7541c71d7ab25ca7b55f50027b92e11c0f655b9561e4a2ac14e0e6cedc17b06  examples/01_platformer/assets/sprites/basalt_rubble_2_n.png
aacd161cbc19d0fd4fa86cb64c32d4e35c2b14b2941745a1e44907e8d41dd6be  examples/01_platformer/assets/sprites/basalt_top_0.png
11c739a45dee2a7d14bb1ca4f6e4428ab86dbfb4671f2bc22bb2e900b93eab38  examples/01_platformer/assets/sprites/basalt_top_0_e.png
9c3e0049328e963e51d39c78a0b3c66537f7715dd895fbf183ef0cdff24e2cd5  examples/01_platformer/assets/sprites/basalt_top_0_n.png
8ece554fadf8f626e97d54c55d81d5f35710263d866d712c446472b5fd3d58c5  examples/01_platformer/assets/sprites/basalt_top_1.png
9156ff8a5d65869fd2f344dae2dbc3b00fdcda07eb8f6104c47fb6e4f8ddd9a0  examples/01_platformer/assets/sprites/basalt_top_1_e.png
635a89f239e0e801869d34ae9516c0e36c1688dbeca32605ce590b2c00915bfd  examples/01_platformer/assets/sprites/basalt_top_1_n.png
4725eca051530f9a4e209bc5eee44fbf8db9426cbbadf2a200a7433bfbc7babc  examples/01_platformer/assets/sprites/basalt_top_2.png
783156a6736ca7b8b8c775a55648fef70a99a3664b8701889f8b413d3a5cd9e1  examples/01_platformer/assets/sprites/basalt_top_2_e.png
ba415eaa2ec43d0ab6baa34ac98a4d2ba35240d396b88efaacf797f4b60bbf47  examples/01_platformer/assets/sprites/basalt_top_2_n.png
89f2db01611ed290cefff6a2b62ceb4fd90fa4883e67958aff3e34d2e5d08936  examples/01_platformer/assets/sprites/basalt_top_3.png
783156a6736ca7b8b8c775a55648fef70a99a3664b8701889f8b413d3a5cd9e1  examples/01_platformer/assets/sprites/basalt_top_3_e.png
5e50f4252e3008206ca9991aecc3a7c3c0bcfc808efef2dfb7f11fdd4fb83995  examples/01_platformer/assets/sprites/basalt_top_3_n.png
d91acf3df3c57e264d6bbf99a22b657595dc677ac0a985ad1996ef6abb373a0b  examples/01_platformer/assets/sprites/basalt_top_left.png
11c739a45dee2a7d14bb1ca4f6e4428ab86dbfb4671f2bc22bb2e900b93eab38  examples/01_platformer/assets/sprites/basalt_top_left_e.png
9e9076a42d058a1a66d1bc6a812ce037f4314b7c6c0427d2a203579ea3fd0160  examples/01_platformer/assets/sprites/basalt_top_left_n.png
1ae51d78577be308abe37b2cd9c142a8bc9e39fa3bab621bdc91f5719cfdaacc  examples/01_platformer/assets/sprites/basalt_top_right.png
9156ff8a5d65869fd2f344dae2dbc3b00fdcda07eb8f6104c47fb6e4f8ddd9a0  examples/01_platformer/assets/sprites/basalt_top_right_e.png
639138d12216c68080951f09224e4cac65f4920393d7abfe150f6264eba8e298  examples/01_platformer/assets/sprites/basalt_top_right_n.png
c06cf26d547437291ec2bc7d77206f3a9fbb4af933f193dbba1f1a8bcd7b4aaf  examples/01_platformer/assets/sprites/basalt_under_0.png
e6e905020dfdcce949be60b833a7e501909d22447a16d23211704b1c0af17e56  examples/01_platformer/assets/sprites/basalt_under_0_e.png
d3838c0a3d40f4aea885dd81b1dc0523c20542f827f43af89996cb6ae454ac37  examples/01_platformer/assets/sprites/basalt_under_0_n.png
adb13f44a4c22722c4119deeabf44f29b6d23c698df72a36ce440ae461b66b79  examples/01_platformer/assets/sprites/basalt_under_1.png
fc721f79a2325a390f17ef3d8e0cde5ba8a2f6720c177631741738f270a5a206  examples/01_platformer/assets/sprites/basalt_under_1_e.png
6bad8c339049c83e9c5afc498170e7057e43e2d12872fa2f2cc260ac679b0a38  examples/01_platformer/assets/sprites/basalt_under_1_n.png
8d239e5a910e0fc590128c28ea5e719ee44146e505e6a547a240b160ecb277ad  examples/01_platformer/assets/sprites/basalt_under_2.png
bcfe048282621a4d58490caff987c36183cce385df74b1bc19faea06628cc2ab  examples/01_platformer/assets/sprites/basalt_under_2_e.png
f468dc30944254c3ce3b33e6077a8389db55119d114438373678f791c7965027  examples/01_platformer/assets/sprites/basalt_under_2_n.png
e6ba57178b4bd5ea8e5391cf4132476cff618e2911a83743e57c49d2ef33afcb  examples/01_platformer/assets/sprites/basalt_under_3.png
ed0257d916e0ab6cba796582a43c1712fd88e8a12d538062c704a526b8e81070  examples/01_platformer/assets/sprites/basalt_under_3_e.png
cdaa90f3db219c64d3b40adec797a4b93b00ba8d9b06448adf2cc00ba0658419  examples/01_platformer/assets/sprites/basalt_under_3_n.png
7a9992d65fcbd82870f6ad4584eccdb1164f2d974dc471c71ee024168ef5287e  examples/01_platformer/assets/sprites/border_basalt_rock.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/border_basalt_rock_e.png
76f8564d4045ffce78a4398a7c6ab77dfd46e33fee700aedc60ab61d913ea596  examples/01_platformer/assets/sprites/border_basalt_rock_n.png
6ee98bdca665dfc5925cda04e71e89e190621580d7b0930ba767d001752389c3  examples/01_platformer/assets/sprites/border_basalt_rock_top.png
8a8202b76f6a68572d4ab8a31273f921aed393859a0ade7607959ee6222a5687  examples/01_platformer/assets/sprites/border_basalt_rock_top_n.png
f9678492b7f4c6320e486a04013a816936b6c76201e9a3fb9ada0f0087f0f0a6  examples/01_platformer/assets/sprites/border_rock_basalt.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/border_rock_basalt_e.png
509dfb8c62caa888ccfb11957b310ff0e84379a1e7f2100956131773f8fe8938  examples/01_platformer/assets/sprites/border_rock_basalt_n.png
5ebb9214e40b57efadc66275eafe8e158bb28b71bacef541da33d277aaf136c3  examples/01_platformer/assets/sprites/border_rock_basalt_top.png
ac5db06bc9b76ceccf998063707e54393949153508954ee19f4f21183b088812  examples/01_platformer/assets/sprites/border_rock_basalt_top_e.png
f5b8ba3128d82ab0ac1f3972248bc6187cf61a9e5f109e6f3a6b0e78d27fedbd  examples/01_platformer/assets/sprites/border_rock_basalt_top_n.png
203890d3a541f04f04c6d798cecb856e8cea055953b1b4441acf864f258d4836  examples/01_platformer/assets/sprites/cave_back_ember_0.png
6da7d6a2401e13ebb09f3a7bc482ba907476cbe7c1678e4e1a6cfb6f916536ea  examples/01_platformer/assets/sprites/cave_back_ember_0_e.png
3035b33cff997a25d2e73f8e3e4f16af8d9996a15a6a220fbb6619742388ce3a  examples/01_platformer/assets/sprites/cave_back_ember_0_n.png
7ece1f0aa36a22b977ed2df68204336d0f37ced17e4e9ef5435ece3a762cf6d8  examples/01_platformer/assets/sprites/cave_back_ember_1.png
40d79a54f38f67539263692bce6d6cd2df5cba378719e575412ad588ee350b09  examples/01_platformer/assets/sprites/cave_back_ember_1_n.png
2913e159db62e10b4b8c090085094ae5e358d223ce7abea28d7e25a48b9d407e  examples/01_platformer/assets/sprites/cave_back_ember_2.png
0d0bf883e7f7fcdc36daed6d8c5f5cd262f37bbbcec8d7ba32f698855c979f22  examples/01_platformer/assets/sprites/cave_back_ember_2_n.png
e0cb12249cad570a19c84458c06dbdab4e6e43e538f814cdd855ec1c01908159  examples/01_platformer/assets/sprites/cave_back_ember_3.png
b700c9c1ee59514f54834f50645370ed051693aaaa26562f5c884ff87d23a531  examples/01_platformer/assets/sprites/cave_back_ember_3_n.png
b0e25c1eec70af7e436a4dfca78f0981d1d24281509537b6221915f74a5e1763  examples/01_platformer/assets/sprites/cave_back_ladder.png
1b0b603306587e70811dc93f1bf51fc75f7f309ec343fdca4d715a4eee00cbe0  examples/01_platformer/assets/sprites/cave_back_ladder_n.png
d1a096e7e52fc5a2a5bdab8c6cca2a97e79680ffe8befc921db8cb7b2e83060a  examples/01_platformer/assets/sprites/cave_back_ore_0.png
f684fb72e12ff3d772d27ea932ff370483efe50f886c163cf6cd10c2fbafcd53  examples/01_platformer/assets/sprites/cave_back_ore_0_n.png
9edca60c2c6d589a91d30df6afecd20f7956d85d7b0ae52f250e87eab8d1d5ee  examples/01_platformer/assets/sprites/cave_back_ore_1.png
94ad507d136b2c6983c7c69cca4f0de6908e04bae9e03e31f102637b58248b49  examples/01_platformer/assets/sprites/cave_back_ore_1_n.png
b452e5c6756c8ff047669c5beb31d7abf2d364f3d24748e2631384c497035977  examples/01_platformer/assets/sprites/cave_back_pillar_0.png
14c930892e46a96cba89ceb373d4ec5caff974f5983cad9d80faf43c2b86765d  examples/01_platformer/assets/sprites/cave_back_pillar_0_n.png
cb987aa8f7bb7491f730aca291d75b47a56091609ac8af55b391addea12670ac  examples/01_platformer/assets/sprites/cave_back_pillar_1.png
f09124c4ebc83256f8400e922c13ab6b2339fe2385d5b6841beb11d706b8700b  examples/01_platformer/assets/sprites/cave_back_pillar_1_n.png
e8d5fea5e659549a377af330c7b575b178def14d6901c0eb743168a285c34376  examples/01_platformer/assets/sprites/cave_back_rock_0.png
b7c0d0ca16ec0b669f4ddf9baef663bdb1664b327083e91449c535fef9dffc40  examples/01_platformer/assets/sprites/cave_back_rock_0_n.png
13d65f98425f411ea2422bd373e65e70ea263a910f6f06dc85adfe6c7856b813  examples/01_platformer/assets/sprites/cave_back_rock_1.png
ae6e7e8d22eee355e450defb7a0aae66f937e667399bd76e7de769435ca94599  examples/01_platformer/assets/sprites/cave_back_rock_1_n.png
c9fcaa1600a9d700314e239a6ef8c15472dd1794fc1165e1b2fc86b67450f353  examples/01_platformer/assets/sprites/cave_back_rock_2.png
4749468f7493551f0ca690119e48ee08a2d59ea9e4de8f77fcd3ffc6f76ed1ad  examples/01_platformer/assets/sprites/cave_back_rock_2_n.png
c7852958805de9f379eb9d80636cb416a6b7f366a73d5660fe0c04af960d8ffd  examples/01_platformer/assets/sprites/cave_back_rock_3.png
2cb086748e88a5181de52c0ab3b96cec9d123a4b4b4a3fc8bde4458cac991634  examples/01_platformer/assets/sprites/cave_back_rock_3_n.png
b52aa2fab84bb2f6803bd963967dbede2222fc0ecdbc3d87f0e2334c4ca552d9  examples/01_platformer/assets/sprites/cave_back_stalactites_0.png
a86daf975412b286c0f74c3deb9927771703381f529e3b0413d66d41aa762199  examples/01_platformer/assets/sprites/cave_back_stalactites_0_n.png
16076f93a840b10bbbd68b603ffede263176df3d9e8cde2ca45fea24d7520df7  examples/01_platformer/assets/sprites/cave_back_stalactites_1.png
d10405b41e675bf23a9f7412dbe69220fcc512a196c8bcc774612035e1478363  examples/01_platformer/assets/sprites/cave_back_stalactites_1_n.png
4bec5578724048fa99095bf2ea7705e84558b4fa62f41a01e4f54538f5937f78  examples/01_platformer/assets/sprites/cave_back_timber_0.png
c5e56ef7f3b5e80302770e0a8b151d6c024e1e6c5aff09995155adde4fdfe9b1  examples/01_platformer/assets/sprites/cave_back_timber_0_n.png
ee6808a9941cf8092e3a1f4b0fd22b7164b2cae372bae58cd6f058ee24d437c1  examples/01_platformer/assets/sprites/cave_back_timber_1.png
d59059f1b8731825847e23d9f3f8a713b5b9377d272d13f2b22f32e66fabf34e  examples/01_platformer/assets/sprites/cave_back_timber_1_n.png
70398a33303bee2d58b55f6c03ae7089943cb52b19ea7fb109d09193a214ed89  examples/01_platformer/assets/sprites/cave_back_xbrace_0.png
4b6ea375a47b3f390757a2f44a71e5e961da37b8135da7650dc760feac32f759  examples/01_platformer/assets/sprites/cave_back_xbrace_0_n.png
024ad37aed093c95602918628c0cd01b749f27d7f7fa8d764477196a6716304f  examples/01_platformer/assets/sprites/cave_deep_0_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_0_big_0_0_e.png
2abac45f12feb47e661d3334aec9fcf1e6e3f1b0571afe665a3325aabbcc5134  examples/01_platformer/assets/sprites/cave_deep_0_big_0_0_n.png
7ab056dd05d0dcd131bad1d0c5f694c41cd40956b48860195459c585484a837a  examples/01_platformer/assets/sprites/cave_deep_0_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_0_big_0_1_e.png
8f67b544f77e827cf070750ba118ad597d0c8a0d30a3eff492a24b79ae52df21  examples/01_platformer/assets/sprites/cave_deep_0_big_0_1_n.png
e3d6753698f89e91fa4c199e939bece0c5063b390e1889a3a51ffb8a37d60bd1  examples/01_platformer/assets/sprites/cave_deep_0_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_0_big_0_2_e.png
949ff99184de66e0371a2946fc4e9d0f5163e2166289e918ece857a16f112b0d  examples/01_platformer/assets/sprites/cave_deep_0_big_0_2_n.png
b574354856f470fda5d407f510efd2dfb64d39de5baab43ae1bec081e6494b65  examples/01_platformer/assets/sprites/cave_deep_0_big_0_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_0_big_0_3_e.png
fa635ac888b6e76999b80991f835d6084a445d61e42e32a2365a2c2aebe9bb4b  examples/01_platformer/assets/sprites/cave_deep_0_big_0_3_n.png
baaeaafd74a589f6c7f27eb9d26919605a467e60c0518c58df8876c80b63d24b  examples/01_platformer/assets/sprites/cave_deep_0_big_1_0.png
3f873faa753564780b9b5adac236bb33a3d775b3f6b0c039271b4b88addaf4d6  examples/01_platformer/assets/sprites/cave_deep_0_big_1_0_e.png
69c266a8c3c4c4ced394221945799078df023cde5f3e7533754c90f10848329e  examples/01_platformer/assets/sprites/cave_deep_0_big_1_0_n.png
11fc79fa008575d29de74a1541dc401fa25b0b2d271e5b47ae157ebaffbc84d4  examples/01_platformer/assets/sprites/cave_deep_0_big_1_1.png
8bb7b68fc523d8af22f572f3c4eff99333bac5e62085822d913f88b4e33d013c  examples/01_platformer/assets/sprites/cave_deep_0_big_1_1_e.png
c11896f0ceb0cbf1f3a46275ce06f25b7b0e82e33a7d77f56800247beed1dcc2  examples/01_platformer/assets/sprites/cave_deep_0_big_1_1_n.png
76aa20835d2b182ca9f65c7153289cf08b29fa6e9f344992db5c784dbc9c2d9d  examples/01_platformer/assets/sprites/cave_deep_0_big_1_2.png
1530deb08b9e3833351fa5ae6417940c07df4c35a44ae4a7856e16821404a2da  examples/01_platformer/assets/sprites/cave_deep_0_big_1_2_e.png
de2058fb1aaa5bea4764e0ce3e2dd28594ea9aed99e7a06ce15cc0a3f6ea881b  examples/01_platformer/assets/sprites/cave_deep_0_big_1_2_n.png
5247b4e9fbc85ce56c8df0dade91e35416f17dd3060d74f8eb484abfd5861af4  examples/01_platformer/assets/sprites/cave_deep_0_big_1_3.png
7f9d3cf42fdc4e1e78b45b909a66810608189cbc356cf49f4884e409af396741  examples/01_platformer/assets/sprites/cave_deep_0_big_1_3_e.png
85db63e650c9160bf4899761816d75972d49f72f056e21bb6469313c9374f1c9  examples/01_platformer/assets/sprites/cave_deep_0_big_1_3_n.png
8a43d7690d8115b4d594fcd48aa83b5713f1e14dab17e018e777288bb517b0ba  examples/01_platformer/assets/sprites/cave_deep_0_big_2_0.png
165bc4d8a6221fcdb0c9dec0a1aa68a474ee59a56819bbb8f5d5007379c1891f  examples/01_platformer/assets/sprites/cave_deep_0_big_2_0_e.png
cadd17376d307a926db67d4f7e6325a7b861b7dd8062e6a855ed43f8142a6e5b  examples/01_platformer/assets/sprites/cave_deep_0_big_2_0_n.png
082a57a4dc19caaf361463af719f3e0f4488ec8464c662f2253e44fa4df47e12  examples/01_platformer/assets/sprites/cave_deep_0_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_0_big_2_1_e.png
b5395ef5bb3a9887e31a13d76f7e7cc88125e5e1e6d2db714d070e3e240a9140  examples/01_platformer/assets/sprites/cave_deep_0_big_2_1_n.png
fc1c6af896831b220899ad09a1ec29865bf4710900234013c8eced4a4ca283d5  examples/01_platformer/assets/sprites/cave_deep_0_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_0_big_2_2_e.png
2b8f9a88a06e1267fc88e08fb1e70552b1c639cb6a67c1f5e8bb2f86f6d12423  examples/01_platformer/assets/sprites/cave_deep_0_big_2_2_n.png
96075b56a60065bcf5d3733877f9fe570a7f0d6436e6292a38a5de3b361772ba  examples/01_platformer/assets/sprites/cave_deep_0_big_2_3.png
87baee0c6edb322ae53fcb268acb300fd00b828ae50e1e43742577f74808270b  examples/01_platformer/assets/sprites/cave_deep_0_big_2_3_e.png
665b1515301a98d81836551b5744c5f8b4288434d0dacdd0b686979d06ed04a7  examples/01_platformer/assets/sprites/cave_deep_0_big_2_3_n.png
bb6ed9e96cd4ff39f4d8304517ec82c626bf7bdcb108966c687380a539dffd8a  examples/01_platformer/assets/sprites/cave_deep_1_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_1_big_0_0_e.png
870b5ce0dfa5ef4cb1877733eb822fd7f86da4c4bac863773034f457997a300b  examples/01_platformer/assets/sprites/cave_deep_1_big_0_0_n.png
45ffbf3c9fd2669c5386d5a5f599a521d927196915c3a7127bddfe757ce05841  examples/01_platformer/assets/sprites/cave_deep_1_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_1_big_0_1_e.png
dd78091b4772676bb31689e766d949723354945966a992d5fbe6b83e8ad06c68  examples/01_platformer/assets/sprites/cave_deep_1_big_0_1_n.png
afb10cf7012b9474cf73aa603eec5574daf5a16e2b517ecbae52eb9ee5a0b21a  examples/01_platformer/assets/sprites/cave_deep_1_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_1_big_0_2_e.png
30cf7717010b9e3516f08b1ab0e5a543041a99b82235ed73fa55b86a4d1ba895  examples/01_platformer/assets/sprites/cave_deep_1_big_0_2_n.png
328b943ae124650ebed471f1cf8fb8f906b8bb7e9e9574a946d620be1c889ff0  examples/01_platformer/assets/sprites/cave_deep_1_big_1_0.png
7dc445d369e4a59b1ad8aa8ea749a559c3acbd75e272184a923e32e63b55df15  examples/01_platformer/assets/sprites/cave_deep_1_big_1_0_e.png
7110c4ea9faf9273859ac58afac434341995d17882ccbdeb76aba28809bdf128  examples/01_platformer/assets/sprites/cave_deep_1_big_1_0_n.png
800f4a5a1e12dcf3ff02ff0d3c52333d2a985e1b13f9424a775081c8c5e6cffd  examples/01_platformer/assets/sprites/cave_deep_1_big_1_1.png
2c49858bc807fdc0601003b917c6494ed3804988f842d114ea1fe64eb53cd3db  examples/01_platformer/assets/sprites/cave_deep_1_big_1_1_e.png
fa8265de44d3ede9e6150d6818380feea6361039efcfb697a4ec7977b7fc430b  examples/01_platformer/assets/sprites/cave_deep_1_big_1_1_n.png
24c77575f1e41fdaf811e64c792996695301e39aee2c13b726a0db9fcd760c46  examples/01_platformer/assets/sprites/cave_deep_1_big_1_2.png
870e240e95267a15037811af90e9f2f168e78ef9d0113f441f87dea0af06d5ab  examples/01_platformer/assets/sprites/cave_deep_1_big_1_2_e.png
e07785f1b36926b6de6b5c25069b1dedd6cf71cef31d30417c683cb3c66e147c  examples/01_platformer/assets/sprites/cave_deep_1_big_1_2_n.png
9414288389182fa12fa934feff0bf231e7d0fcb7190ecdd957e843dc024b3113  examples/01_platformer/assets/sprites/cave_deep_1_big_2_0.png
1567baace1d47c383e97ebe60f306227f2f0db1683bce5ea641ba09308ba8531  examples/01_platformer/assets/sprites/cave_deep_1_big_2_0_e.png
59b3441affeabb51e0f82425186b5c50d5a42431f4ff2c555474cdd16fc62954  examples/01_platformer/assets/sprites/cave_deep_1_big_2_0_n.png
7f9c8719c98ec4a086a86ffbdabbb117ed2fde9328b0c9f9e0dc5cd9e85619fb  examples/01_platformer/assets/sprites/cave_deep_1_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_1_big_2_1_e.png
dd308bc30f14b93867d77334eb152bd70230ecf16d6efa8052ed48a6b1041611  examples/01_platformer/assets/sprites/cave_deep_1_big_2_1_n.png
387769c0ebe6ed81813c246fb88f796bf01d212a078d3fc9ad0101547cc4868b  examples/01_platformer/assets/sprites/cave_deep_1_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_deep_1_big_2_2_e.png
f398cd3be3d275e788f83640b49d935a389b8e0bc2cb0321d96f879ae0f8827e  examples/01_platformer/assets/sprites/cave_deep_1_big_2_2_n.png
b514890f9d3483ad446635d1f0a3260de2bd21024c511d43416ccf9dcf6f0808  examples/01_platformer/assets/sprites/cave_mouth_big_0_0.png
e881b8bc0de031ce2900d391d4a662e576fb041601da78be3d96008d92739879  examples/01_platformer/assets/sprites/cave_mouth_big_0_0_n.png
3b1d31b0cd1bea4204032b671d63006a61d159f402b2a061a58c5a3ee219f441  examples/01_platformer/assets/sprites/cave_mouth_big_0_1.png
d7a5c84b5adb757190bed1716d7691fc2a4595039902ba1a84163b8df80ff176  examples/01_platformer/assets/sprites/cave_mouth_big_0_1_n.png
8d99d1ad3947b24b85d806ef6b95f291a95ad0a2d6ba20d37dfbd6f486fa7040  examples/01_platformer/assets/sprites/cave_mouth_big_0_2.png
9041e519b8dceded82360110a0846dc326aa6cc986d0429bebcea755d3bd6071  examples/01_platformer/assets/sprites/cave_mouth_big_0_2_n.png
7b4e40e7c6a756e758072f5f578ee6540c5c964a864bba60f52df80416ac594c  examples/01_platformer/assets/sprites/cave_mouth_big_0_3.png
177ec0f5baa3e24a1d274b4b5d37ef8f1f6f276295e588396cd14e3b32307896  examples/01_platformer/assets/sprites/cave_mouth_big_0_3_n.png
b52bc87e0f627abc264e509cf00716dfc29b90d7c199fa551f6ee339563dcc03  examples/01_platformer/assets/sprites/cave_mouth_big_0_4.png
a8cae68266fdb1818098c05ff21bee1df44ed73bf37b9ef444c8034559d8bb16  examples/01_platformer/assets/sprites/cave_mouth_big_0_4_n.png
7c2c464a80a9d15f88e900f92201f5b16bec084d28a3e158a70307d682466c4f  examples/01_platformer/assets/sprites/cave_mouth_big_0_5.png
911e6765c2fc1fdd2fa93ee3b0e2cbb555bca1cb897b96657209278e22fedbf5  examples/01_platformer/assets/sprites/cave_mouth_big_0_5_n.png
dbb7229a18b04042f82ce6cad5f1e9ea21efb79bc8b657fc9334a58313ac64d1  examples/01_platformer/assets/sprites/cave_mouth_big_0_6.png
a1ccc91a26cc4ef10266467f0707d2ea3e19b3282e314799b4d6f40ed821d353  examples/01_platformer/assets/sprites/cave_mouth_big_0_6_n.png
7e59b392fb74dc61494f8683843e77b0240808ed6d5f11704e1a60c5a4cabbdc  examples/01_platformer/assets/sprites/cave_mouth_big_0_7.png
cc7e66f71d75c5e95761fb52a1b9c5d8aeeb7ec71e0dde75c33b61ada252c6ba  examples/01_platformer/assets/sprites/cave_mouth_big_0_7_n.png
1133dabb7be8bbd7b19001da03feaace9ecab070c53e9b956f4434853bc7faf5  examples/01_platformer/assets/sprites/cave_mouth_big_1_0.png
952017582f0d02c27b772cd5d5556e23bd88d34ba1392ecb6a04593bad246f91  examples/01_platformer/assets/sprites/cave_mouth_big_1_0_n.png
80915aae5f1968633294d2adcccf9d6d1fb66a7efeba94bcfe62aaa6eafcd3f4  examples/01_platformer/assets/sprites/cave_mouth_big_1_1.png
f0b2d3b71752641b8b4c6ef10c7348ff3e795d1cf12fa9f2efd4a0eeb986dc0d  examples/01_platformer/assets/sprites/cave_mouth_big_1_1_n.png
c9c59e202077e00dadc9ee6fde8e3badbe2d10ec474f34f1002d4a7f60004c51  examples/01_platformer/assets/sprites/cave_mouth_big_1_2.png
4dc4b7282cbbf49b4ba8209fbb8915c8793ba45507c4b69ea385f7ec8ae06d2a  examples/01_platformer/assets/sprites/cave_mouth_big_1_2_n.png
f731c3a321ab9de791e864ced9a86971972c5fbd1c95166e453f44e3722f967b  examples/01_platformer/assets/sprites/cave_mouth_big_1_3.png
1e5bc0285ecf0fce35b4b64111a17f84e5cd953fb5e30194ea2b0ccef1cc5064  examples/01_platformer/assets/sprites/cave_mouth_big_1_3_n.png
5957ea6a9296bbd68028137e81084d7a16051bc5ecfb89edefab31bfa86e417b  examples/01_platformer/assets/sprites/cave_mouth_big_1_4.png
8e5f9609bde1d8fad48431a0237c3a98041ac7ec44ede77589f0fffbfd4d5d3d  examples/01_platformer/assets/sprites/cave_mouth_big_1_4_n.png
91841fa91ddfa30e068593f88b3a8a06120694eb9dff87c36e61a57444b261f0  examples/01_platformer/assets/sprites/cave_mouth_big_1_5.png
15c7f7d98edf763d71b6df83810d1ed77a79bf27971de994ec7b251e91001e9e  examples/01_platformer/assets/sprites/cave_mouth_big_1_5_n.png
b4d1459d114c1ff02d88b2c6efdc2ba1e19c2231047a7bdf5693b4c6d753001e  examples/01_platformer/assets/sprites/cave_mouth_big_1_6.png
93ac75001ffa067d7db28c8f9104cc43882e8afb9b003d4d0bf9db7e66c3a998  examples/01_platformer/assets/sprites/cave_mouth_big_1_6_n.png
0d1cf3ceba8c3917fbd7a465140cac49a0911ff9ece19a33b51ac864ece1a284  examples/01_platformer/assets/sprites/cave_mouth_big_1_7.png
97572da940c3d38b762cd2ac7163a73677026b068f9688f4f950a5b729002d36  examples/01_platformer/assets/sprites/cave_mouth_big_1_7_n.png
3f2213a44bbb67f9b3757de9e82a09bb6152cc458cfcc81e79176746faee34fe  examples/01_platformer/assets/sprites/cave_mouth_big_2_0.png
a3224b1f4332f1fd3d2806793e50c3cd3ff2a0d7959a8e6ca1b78a30b9f1e013  examples/01_platformer/assets/sprites/cave_mouth_big_2_0_n.png
a13f96ae8fb8dde66c5d0f980e0af4d82aa6f3965334720f53b186cddb5237ca  examples/01_platformer/assets/sprites/cave_mouth_big_2_1.png
a4cdc492edd281ede01c5966087f16818c1be7b637ee122c621d3cc218c71d38  examples/01_platformer/assets/sprites/cave_mouth_big_2_1_n.png
03c13dc6875a57a3174b04a4b1bdd79fc343b043e6b09d32ecc775da33d29957  examples/01_platformer/assets/sprites/cave_mouth_big_2_2.png
b128b9b5dd533735aaac079083acfd4fd368835136aea32f8efb9bcb24b1093f  examples/01_platformer/assets/sprites/cave_mouth_big_2_2_n.png
6bb8eae59faed5452431b62036c047d3229fd1e70deed22a27d8cb3abe1e498b  examples/01_platformer/assets/sprites/cave_mouth_big_2_3.png
1bd11e467442c84828c5e39394c9b53dec4c44fe754ef6f15d4a63911e2d9640  examples/01_platformer/assets/sprites/cave_mouth_big_2_3_n.png
d82b6e972850267df75f2adcd381bd461435477ae777d2a378e297fbaf8758e1  examples/01_platformer/assets/sprites/cave_mouth_big_2_4.png
700e1d97e64e7f18ac4d8a09f96eb80d2f6891becc900f35b81ab1988539d3b2  examples/01_platformer/assets/sprites/cave_mouth_big_2_4_n.png
23c6bf73d137f2d6124ff4830f46e6e3fe4442c6a23bdd096ecf7354df4d63a9  examples/01_platformer/assets/sprites/cave_mouth_big_2_5.png
2d5b876c85a98f7eeffe7313927332ecb7fe93c7ceeef44eb738a2266164cd8e  examples/01_platformer/assets/sprites/cave_mouth_big_2_5_n.png
abaf9cb3d33e694c56cd94f4ec6b883257ee1e103d7e8e6ef51367d80200002f  examples/01_platformer/assets/sprites/cave_mouth_big_2_6.png
ef08a59f9bad0bcfc67f68c551f3cc06b3ff203e7771a34c1fdf359cc88df1ac  examples/01_platformer/assets/sprites/cave_mouth_big_2_6_n.png
41c9f609706eb7e3618f69f55b19316e7a935a095539162f00f033f59c6f1c0b  examples/01_platformer/assets/sprites/cave_mouth_big_2_7.png
a3428c25564fd5274c462161994ac986189005a9528cbc8bef83218c7b92c17c  examples/01_platformer/assets/sprites/cave_mouth_big_2_7_n.png
15361f257c64e1c35d1de005d36a800de6c3c7fa681238f2eb4467b598dc73b5  examples/01_platformer/assets/sprites/cave_mouth_big_3_0.png
adb2373096e68f5db86f31afc3a3e95722487c66e5468bb2015caa254308167b  examples/01_platformer/assets/sprites/cave_mouth_big_3_0_n.png
7e89bab990846486f8fc4f76ce51e2abd85bb2f6e2928ef846c373fd5b3d3d1d  examples/01_platformer/assets/sprites/cave_mouth_big_3_1.png
eab87a2ac3589197f2490848da5ea1c216977b51afb53a9aa728ef0fc9dfadd7  examples/01_platformer/assets/sprites/cave_mouth_big_3_1_n.png
def42d0c0231ce3821a74439f73fb4518a71958a794b68ee32c6d24fa6b0aa87  examples/01_platformer/assets/sprites/cave_mouth_big_3_2.png
e4435001b70e4fdbfc4aa7a927bbe9b7a92ac71f7f3bb09674732c76d3e6cfea  examples/01_platformer/assets/sprites/cave_mouth_big_3_2_n.png
0b42722c68bdadf397c468fc34c41b5f6734d6f533b6a13d991153a4ecfda0e3  examples/01_platformer/assets/sprites/cave_mouth_big_3_3.png
b3d8f7212938bfc8d1369885686616a4c652b2e83721af949af8e158e0112ffc  examples/01_platformer/assets/sprites/cave_mouth_big_3_3_n.png
e882b427066347fbe54d2becf11b148fa4a24b86aa4723240240538d45df7725  examples/01_platformer/assets/sprites/cave_mouth_big_3_4.png
f274217827acef8aa07293c9fcdf8005d586ff7e3499de2ed8e912131a009105  examples/01_platformer/assets/sprites/cave_mouth_big_3_4_n.png
2e901c3b90a70bdb8e6f4283c2cce3b16fc531e259d34ec131aa768131dc57fe  examples/01_platformer/assets/sprites/cave_mouth_big_3_5.png
37b5abdd02312762e155bd801189be187987889e65f1b4b1943264207dd41fcd  examples/01_platformer/assets/sprites/cave_mouth_big_3_5_n.png
0e63eff73b0a315c3f3aaece1cf8e2d3db6a98b10b08e453543166123427d9b0  examples/01_platformer/assets/sprites/cave_mouth_big_3_6.png
e7bceda973df2e73977301db04df91851b00272397bc5e09acc212ab1402e2bb  examples/01_platformer/assets/sprites/cave_mouth_big_3_6_n.png
ea01d1f9003c8697b575347d9ca9b4d75f39f88d4d1582bbf3880b57b036f8f3  examples/01_platformer/assets/sprites/cave_mouth_big_3_7.png
edfcf0199d570704b1e6a1bf51accdd324b2ae9b17df85a3a1a371eddb3b49e1  examples/01_platformer/assets/sprites/cave_mouth_big_3_7_n.png
3231c68c59cd3004f61d6325be3263bc9399f1d35ee52793285d5e1efaa818d3  examples/01_platformer/assets/sprites/cave_mouth_big_4_0.png
fa348bb73d1b205a16d5dad6f75e0b63d52dbc617f5d1a3dae8fc58c2571892c  examples/01_platformer/assets/sprites/cave_mouth_big_4_0_n.png
62a9ba6a0fc501e631b911137860af8932968887d7e9a37950c0ecfc0a4a888f  examples/01_platformer/assets/sprites/cave_mouth_big_4_1.png
247a08b74acd28ae295d39c44b4870377019790c821bddda915df9b541a6a861  examples/01_platformer/assets/sprites/cave_mouth_big_4_1_n.png
1ed123f50d364bfb805743d729c52865af888864a56f69e07ec603f76d5f53f2  examples/01_platformer/assets/sprites/cave_mouth_big_4_2.png
656e8cbf7cd1b2a27d65ff14859f1915f2ee9d90ac256259a705438c14187606  examples/01_platformer/assets/sprites/cave_mouth_big_4_2_n.png
ca793b5f9190fc8ed33bd053e4a22a560e2fc4a5352ceb92f94e57504f1b164d  examples/01_platformer/assets/sprites/cave_mouth_big_4_3.png
df8e9fc73c5351465dc543aa770b45d5386977aca4adc2b43961c81597812403  examples/01_platformer/assets/sprites/cave_mouth_big_4_3_n.png
382fb8ac0ebbf0836ee6868e567e74d5876dd38c91ce577226a87ce551f6c9d8  examples/01_platformer/assets/sprites/cave_mouth_big_4_4.png
5f41ad22d656c421377159407c72de4bf746e4e820c7ea5e6d57594f9a8de39b  examples/01_platformer/assets/sprites/cave_mouth_big_4_4_n.png
7b69de8edc0bcc67a26fe2a28fa9766e35ca4601803e96d51a6150c6ac1a8cda  examples/01_platformer/assets/sprites/cave_mouth_big_4_5.png
373bc67568e5a10d74ff04aeccd9aa710861231624162fd72e4172209e81489c  examples/01_platformer/assets/sprites/cave_mouth_big_4_5_n.png
e8a54f94b0768d40b2276d7a82ae30ad578a72c77d09936365f9eefcca09df8c  examples/01_platformer/assets/sprites/cave_mouth_big_4_6.png
1cdd554a86be0933744102bfbb11f885e548107000d51b0336cd7a29d730cbd7  examples/01_platformer/assets/sprites/cave_mouth_big_4_6_n.png
83a4443fb2c7289915b590e46ea19f9f63c3a373599444ec809e2447a60c97aa  examples/01_platformer/assets/sprites/cave_mouth_big_4_7.png
f4d657f3f540925a91f0841e57c90614287279eca3835fcd066d5fa0744ee5b7  examples/01_platformer/assets/sprites/cave_mouth_big_4_7_n.png
f7f9adea8633e3de5f1323b0ee8929dc7d170b6a8b4507ba10f1c633baf85d6d  examples/01_platformer/assets/sprites/cave_mouth_big_5_0.png
3a50d9bd96142554c7fe497d883fec3e04739b746e5544df5aebd9eec0582185  examples/01_platformer/assets/sprites/cave_mouth_big_5_0_n.png
1e182c295097cf1d30231cf2c9e8b2de2cac0a55a5c1a2e690e86c534d3dd4f0  examples/01_platformer/assets/sprites/cave_mouth_big_5_1.png
b7fc5a2248abd9af906b26dfbcc0ab8df8c11fad8843bba03ec434e3e56f7acf  examples/01_platformer/assets/sprites/cave_mouth_big_5_1_n.png
ec418e4dd73e65f7b8cbe9f9ddb1886518a99bc3a81d0d87e31797ab530c7ec2  examples/01_platformer/assets/sprites/cave_mouth_big_5_2.png
79949c4f2f7075d91aff456aa0bbcc8d9bfd73c25ed56edd498b09b90745516c  examples/01_platformer/assets/sprites/cave_mouth_big_5_2_n.png
bb0f50cb23749e9d99025b1940b73b807d7f4fa3b9237053008421b1655ef256  examples/01_platformer/assets/sprites/cave_mouth_big_5_3.png
8017c25d0c88d34abe277c58ecd3e44ca01eedc741abd69238a8ca811537b8c1  examples/01_platformer/assets/sprites/cave_mouth_big_5_3_n.png
a861033b83bb650c76e115fff0898a416bcf9397ac3d8ff5c9b4c126a9d4bc92  examples/01_platformer/assets/sprites/cave_mouth_big_5_4.png
da11a3d1bbaf5960412b479f8827877ca04dd142f79fd8f8a8565eb031145eee  examples/01_platformer/assets/sprites/cave_mouth_big_5_4_n.png
c2af26a1f1672ae4eef8ff3b9c811cafcfc59ea988f2c29ba32a25e98c0f644c  examples/01_platformer/assets/sprites/cave_mouth_big_5_5.png
fdd227494643658edff00c0b416c7d4c3af2ebf87930d352418823b9eb3b50a5  examples/01_platformer/assets/sprites/cave_mouth_big_5_5_n.png
64623cc0457598480ef3318a46e043c478253aec289e6fd98f7f6882488920b4  examples/01_platformer/assets/sprites/cave_mouth_big_5_6.png
0d425b6ce3a44c565f454a9c549502f54ccf534d3d8e45962031ce412ec0af52  examples/01_platformer/assets/sprites/cave_mouth_big_5_6_n.png
9867d102b3a916941e7450ea6f10352b1cb8bc4c7039690f7a48220f6ce83837  examples/01_platformer/assets/sprites/cave_mouth_big_5_7.png
be022f5ded69a61be11152e1330cb221cf02135e23b4699b42b4e835a0fdface  examples/01_platformer/assets/sprites/cave_mouth_big_5_7_n.png
1fc568c6738192850b7f15d813b36449947e4ade0b2049a835ff24782edc3751  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_0_e.png
31cc37294b319aef1b1c2824093cb400fbca8433f628e4fa5df3b7ffa8753e1f  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_0_n.png
668005fc99dd3ba08dd8bafce8de887b25057b5c048bc0914f3468923b731c5f  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_1_e.png
91afa817ba2e11d6717bcaf4716da8680666d50fd944fd6e29b2546da9d00677  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_1_n.png
5f799c305a64cef1f19325db9a423f19a3d7b87f449d25c363f7af663a6d9e36  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_2_e.png
e0a8184b97c5ce4f3dff2db4b0944d8ba86ff93b5171cdb8c1c68b8989cc3cb1  examples/01_platformer/assets/sprites/cave_pillar_0_big_0_2_n.png
47a0ac9024d82539d71c88ec5a7ca274eb10954534ae5801da77e61387290286  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_0_e.png
730a5b892bb2f7b67e889d981a0b1211aab9b7a25cda8141b453d5affa9b2243  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_0_n.png
d4ff88a26cd334a63ac3a8a5dcd6d80c26830409a18f0890161bb047ee82adc0  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_1_e.png
40b6f89b463d2fc1a72b4d87b521dafb0506b33cfc9f22c13ebc5eb349fc557f  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_1_n.png
c997954d80b8d97b460cccbf79f9dc615762b9c3f394e1fa091cf65aadc01027  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_2_e.png
cb76a1f49b1888530da987d3b8f83c87b32eea79f8b3562f2451796484aeadbe  examples/01_platformer/assets/sprites/cave_pillar_0_big_1_2_n.png
b2db58c1647b9f19a9268e3c06f2998b547cbfed63be770429992b2d0010979a  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_0_e.png
2d37bef0af4e8f5c65aefe6d8278bb34bf9e2a4417da62ab3c33fc708dd9957d  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_0_n.png
1b91b0d4fddfcd648cc139fd9c8863287ddfba3cc526aebf2fe4cbe92c355477  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_1.png
9c9957d7d1c3ee1b07a5bb1cf7cdc1f137184b800d1779aa47829f50b3e998e1  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_1_e.png
44e80c49bf10241f3bb7d5e860127f9be67124aa833d9d48087898eb37bed8ab  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_1_n.png
82bfb1ae3817716c68c9362af6740965ccf1c740bce858e5b72261d2a0f7e234  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_2_e.png
d6fa99ce5020280d1016131179562dacc99257ad650f3fcebc151e93ee6a65fc  examples/01_platformer/assets/sprites/cave_pillar_0_big_2_2_n.png
b32672457b00138d2bc27f1eef8c3c738d2b59d2a6422e278a82ce1ac4e078fa  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_0_e.png
c213262f6bb76d12723312fc5d9b06220ad311a1905bb048bc64dc726642eafc  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_0_n.png
63bea34ba67ddc0c564308a70dc6592f5976ac9856f99c24fb68b7fabff6c2b6  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_1.png
04ec664039ef74b1609261d7ea441502705be1a549adbce1ba35ccaa6bfcca33  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_1_e.png
091ca2879d2b901b3c533334a2faa66deb423228c95a42357285a5901d7fe455  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_1_n.png
86a589798c9f63ca36c2f3035a41d33bb38c9f11453ff17ddf1b664f5546dfae  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_2_e.png
f8c68bf33be2d3232c8a5814a093a1f5787e3536d51819b5d8404e8844fcee24  examples/01_platformer/assets/sprites/cave_pillar_0_big_3_2_n.png
b3ede8d7c2ea5a41691eb108f57990a8856ad669370d1b9e41e3affbd437326c  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_0_e.png
7185a52e876ef09abf6ed459127be7c0a9b84eaef975e039185b5b11a4fad318  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_0_n.png
1e1b53ec73d1338616a635018411e466e6b783a60cb9ffd8271f1e583cc62cab  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_1.png
7b31cad9f69d976a13abdbe50bc1c566eb703375933bbd233d09c824e99b5d9a  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_1_e.png
462ef8741f1728d70b93af619c321d5425a0d76a9a6f2d4c002d8f4b886d888a  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_1_n.png
139a63f90224038fcaeffa72e680a16b264f3dba679a6cd25aa6483773cd3211  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_2_e.png
61dcb832dffd08b7b72e247296fa930a2dc10e6b85f1996ae9f2f5312e97de0c  examples/01_platformer/assets/sprites/cave_pillar_0_big_4_2_n.png
a840042b3711aeec1ebf815566cce1dd8c3968a69949f5edbe7bf9e5801e96cd  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_0_e.png
a95894b823b6738399f5c0eb0ff03b3d08d9e069034a88457483dc02b5cb0d35  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_0_n.png
935978d25baac42b8d68f9bfc0f68897586e0f8fb100e0af4ab1fc136b60c128  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_1_e.png
676ebc19cd26115077855c32b64a48acf14318508552df77b9b94529397669d7  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_1_n.png
9114aaadc6f3e299cf6dd651764f8752a41fbe7b8ae1dd872247ae6cf551e571  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_2_e.png
96e67de97aeca5bc69dd3742ecd514c293592a69384353692ffc80c3023c8856  examples/01_platformer/assets/sprites/cave_pillar_0_big_5_2_n.png
8767d60668478b1be22e718427c17c6fa758f310d984ac3adb0623c07b800081  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_0_e.png
d565cf0bb97300636495283e14b17b3a2f2b023ff9a4854c2248a2ba613ef44a  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_0_n.png
ebb6d540ac352da266589c04c911e695cbbc32e52c7cba8f98d5fe2acd66bdf6  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_1_e.png
64fcbc21a7648fdd41f05cd4e3d6c26933e37f906e62b8b8ddee80ed3dcfe5ee  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_1_n.png
b45b6cea6b977c077f210e4715d48af55762c060012e32d8b88623b519f88a2d  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_2_e.png
cadee31db526205fc989f61a9a15b336f02ddf98ce69f293d8a0d0b1a4e2822b  examples/01_platformer/assets/sprites/cave_pillar_1_big_0_2_n.png
1e60d4a692bb9a2cbf450dea957e99725083f3bb71f76c092a078eec39d115ac  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_0_e.png
eb2cda3616b72a72b3c65e46263b34894d3baa5709ef5a9b6e30d04db12b2f69  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_0_n.png
380cda019cd92c65bf3d307eb84f6d8e43cf395f007a4fe1e70f82730e104862  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_1_e.png
df94008515a3727ad2508c42d98175790e5ed51657d6c6d3baf242b34b3318db  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_1_n.png
f27951cc7cd85251febbae596c793484997ad79198c5154d9b2ea807b2257826  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_2_e.png
35f98ed58a41686ab3702badfe5880005a29ddaaf209781fab7f6264c3889a1e  examples/01_platformer/assets/sprites/cave_pillar_1_big_10_2_n.png
97cf056a0d4b3acc44fdaa3f5104a9187b13481079546ba988191f263c0a2ad2  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_0_e.png
bb454028071809510ba80059e904d5ed95404b90c32c90b18d70617ecae7f13e  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_0_n.png
6c1531ed5f0b9554a9f23edfbd5b488cbe5aac3aaccc8234bce209e4d685dcd3  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_1_e.png
3c73b8fa173a82551832dab871a304539b1b7a5ef2509d11af3b617887247932  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_1_n.png
08b2178b3e9ef47228a81b1a6249b4713abef94a630c846ec30f7ab305d42193  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_2_e.png
38acc033e0907261d940606527554f327f4adf0e68cb5d10d2f7f6aa5d313ff4  examples/01_platformer/assets/sprites/cave_pillar_1_big_1_2_n.png
9f6d4b1cf23fe2032761f1c55d8cc04572785aa9959feee8bb01b8095c03a4e8  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_0_e.png
effacb9b0a7dfd85433578644121dd4fe111fbd2ca6cd06bd9ea1d09f4e45b7e  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_0_n.png
b8a67e8b33bdc57b1a0fcc4c5b68794bbef6e51c236a6ece39e71be681330677  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_1_e.png
b1b8bab9728961b1906df2be91ebdce118dcafa17900e13be44eac2ae7371c57  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_1_n.png
2496aca0ab18deeea6c69f96d9f172aeb084310ec980edc743c40f7326b7954f  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_2_e.png
ca90f4d817ae2e9542e41ac74fc392219434e504fc180266135901cfa7ff07e0  examples/01_platformer/assets/sprites/cave_pillar_1_big_2_2_n.png
5ab2f9fb627954102eb197e1f9ba695ed0e94245be01f6bf10db52adb4dd532c  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_0_e.png
627309f3c9c68fccf9f6b737bf769832eb7e5e049db4506cf5d83cfa6575d0f6  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_0_n.png
5b05f8a00ead6f66295c54a170fb86d8c15af7ef7b0f8922246046e5c19350f1  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_1_e.png
dbd25aa443d6fdbf4a83831251d548653e7c6790e360c63ee03cb1acfbaa10c1  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_1_n.png
a0c1c037e624c0454afc74570e8bcea690cafcfd9c08686638c5139b08d9e532  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_2_e.png
cb870cc6282a66fce6207c8162130f6b88df2a993a6076a0968ddcc2c2ee2822  examples/01_platformer/assets/sprites/cave_pillar_1_big_3_2_n.png
1c7c0054d71ba7640f496e96df987c3f6276a9906f8a131a550e743dcccb0483  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_0_e.png
9c88d489bdce67b6921bbd12100031a56ff30a8bfa7f80b71bc41d7a9c63baa7  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_0_n.png
7beab622569f08dfd987990a9e7a282796c491f207b1e023a3407539254794b5  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_1_e.png
5bfc8cc9a5ed76bf96fbcd605ebe3937dd01a64e8e507870c4d7de1798594b7f  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_1_n.png
ed3b42c18f129a5150b4e3cc25349f7dcd9b7431c797203b243382dde7cd2c4e  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_2_e.png
093178f86d0973eef9cc7d77dfb738d1ce678599895fd313f49f32ec992341d3  examples/01_platformer/assets/sprites/cave_pillar_1_big_4_2_n.png
83e2f0c0f24d172fb1354931b255c28e484121ed38ac40d16c1da67d6c81e7c3  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_0_e.png
93a2f5ba564073fe9a2e6864f46fce969d62da04e63ce0725e17f2fcb79c3c1a  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_0_n.png
7946ab8b768fc4b679cac120bc100b85fb5c0e5a851b3ac12d43bebd735c8038  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_1.png
1a61efb8314095576b11ce7ea94fdb66d2d8c6b406681e2e6581331e2cc39ae1  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_1_e.png
75c05349a8a73a9f67db1a7d328c647780cf1e79ca3f9a115dd11232d775068f  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_1_n.png
d76fced66310f1c79347d6a8839ae8f18745b88c644e1e55db5ea92c4de3ad5a  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_2_e.png
91890a33be1dccb5c5524e43588b3ae8da188b5ac6c95b450a2119e1a2982ee7  examples/01_platformer/assets/sprites/cave_pillar_1_big_5_2_n.png
aaf0283c9e990a427bba4534be34913f58879beb5593eb7ebc9a6a0616cd4863  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_0_e.png
e0c91a41d58e6536e617628fbb462463918ac5a9796300ad875a05c322cae2bb  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_0_n.png
7ecbf8267c56d5907b045cbd5dd0aecb412ab653d62419866907564c6ecc4edc  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_1.png
3cf85cb91b14a90fe4552ec0e63359cd4197fd2ebbc69c981461d38a283e893a  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_1_e.png
0d399201a7a6d94989c161783887e86290788696ca3259c753338a6a8ba8b7a9  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_1_n.png
5712bf246984f91290fce71270c7622efbc01338359a46b1e2714fbbb0681a49  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_2_e.png
433d7db18f3d1db094dd9ef30279638ea618e586a4ac4b39a37b6912406a04df  examples/01_platformer/assets/sprites/cave_pillar_1_big_6_2_n.png
0718e9d015eedea8d9b135bf26fd3f4cc193d1a2e773257dcaa17ca96e453569  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_0_e.png
1182003f69cceb484a90abb5aa729c11d67153319ce49bd18a038442eaa7bcdd  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_0_n.png
482fe1c0a200abfca4fba18cf0ced4741c9e7057e3012f04ab524a1fc1b39c24  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_1.png
8746150e823bf86c0deb5624981499088b20ca01a7eaf53d1f7a5c7a43e6e1b5  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_1_e.png
bb9f1bf7fe5cd83ee64a2629e1bc1c0146af76ce3df03e4bd01ab10a80837f6e  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_1_n.png
2cb024910f84e9eed6287a16387a369357b2175a7dc8185df90f95c5ca80004f  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_2_e.png
765ee55a56814a5a19f9e9e0da500f258b1f7132e3be3d7f986564acdb60cacc  examples/01_platformer/assets/sprites/cave_pillar_1_big_7_2_n.png
8dfdb67a5d989a7ae92eaf2b12438ff1dcfa452d1a89aa59fa8456fb895de4aa  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_0_e.png
1cd4e06520131971317ab9008033d5af1d3140e23ae584fd44dcaad1e38ff640  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_0_n.png
3da9c97f6f7f4b0917fbb17c3fe1e0cb65c53ceae70333cb079f84797e8b4875  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_1_e.png
cfc3c61046e5d89b5275e7ca8998555dc6eeb3257a91cefcfcee78d811b8480f  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_1_n.png
c55f2ea2001e6d59c023fc6933d87ffb34ecbe08554180324dfe7ac303438bff  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_2_e.png
40762a0fb8724eaf6e4045a7a8d16674931c3439adbdee065fa465c415309e49  examples/01_platformer/assets/sprites/cave_pillar_1_big_8_2_n.png
c019e643c70cd3b4bdabd98229aa8df17e2bc624f9dda12c5b8a901a8d40b1d9  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_0_e.png
d8d1fcd07f11474e0fcbea86828f8c6675205f0de9039fa6d53cc850745504a2  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_0_n.png
5ee32d749423ff06afb902e4d06b8dce89bd6f46ce1191185cc4b91fedc22492  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_1_e.png
b66c325b194ab22662cca3660e613de35e709bd639bd60df69fe415ba07ebc40  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_1_n.png
481e321f8e2ab1ca84af4e962207c251998c7506ed2414555296b31fe1d5e524  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_2_e.png
be8541e55dcc021cc682df22e36454239b28fcf2ad1febf2a0df7de792063d1d  examples/01_platformer/assets/sprites/cave_pillar_1_big_9_2_n.png
c8306ae5d7f39cde76c1fdcdf6214dd4377e055894abd659ae5d877d5617b6a8  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_0_e.png
4c057282cac156b97ae7f87342f8a043f0322d9fcc489a8885b811f91677876c  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_0_n.png
4d908d1dcc038d25ba83c5366d0724a2ad9f4e6f25625fa62a7ca6c06de145b2  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_1_e.png
da785dc155e7b52ddc100c5fa6218d8e71301f8d02997a8409e18995c5bdfd95  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_1_n.png
875b75548bd71e9d6e7b30ba146759535915e07c1cf044b86349a9881a89979b  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_2_e.png
cd0213e94475eb0e6e3595f112e7ae40f6c90af730826af4f49a889264850e9f  examples/01_platformer/assets/sprites/cave_pillar_2_big_0_2_n.png
2cc18f2256d81c5326d0d343ca4e7fc876234805d8671e37cfc678f4d49a5fb4  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_0_e.png
995b1709eac8a7087f22f68ef1037e87f45f36f3763283b8b6027e5cc9890d33  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_0_n.png
7589e4aa1b9609e479dc74188309a586648322137e3d6c398802b9a151a31a6e  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_1_e.png
e21ba25013d3bb7edaee616535c5fc1572a63ae8903669cad23bb6185657550f  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_1_n.png
a05715a1d916372be36a4aefa3e965fbfe26837f93391373a4159cee31224fa9  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_2_e.png
26a89c081fbcbfeff30f5b8da3b322327cfcaa3fda9d490c79db8a6317019b03  examples/01_platformer/assets/sprites/cave_pillar_2_big_10_2_n.png
b7e0100369a45364c8b5382fd2f02822810d5b992b7a3bcf8a7368f68d0947cd  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_0_e.png
52a186ce1087d61cbdab50631db228cfdd073e223dd096025dd6496f14f2e134  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_0_n.png
8161a9f766e72cd5b5f375167239847b6456b3b7204832edef806bc8d05c8184  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_1_e.png
86411c738fe8b3cc9b5a08221be9ab064edd303aa3903a1a5f3cc1b51de76884  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_1_n.png
8e5966451651bc2f36c40c43e43a1b5f57492af67bb502f7c54021a9b04abbb5  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_2_e.png
aed6a68b5972656ed322460fdad9c290f908d0a3c80b8e337aca478868974e52  examples/01_platformer/assets/sprites/cave_pillar_2_big_11_2_n.png
89eece16248eed65d33df5ceb114829118ec36ac562c1a635ffd61df8066708a  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_0_e.png
eb69de9868b27b08a1f83b8bacffebc423af784bfe5be34b5bea137983a0f3ba  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_0_n.png
432ae2110202152b892ec7aca8b86e6dd5d61a8e14d26e640a749c7a3b285287  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_1_e.png
c105713c403979cb26972837e855502834495d9b2ebe610827d0c3f765f28671  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_1_n.png
ec1ed3dcea953bcd2f36a1584b4c91daf8c7749af280da0b605afa476c3ceab5  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_2_e.png
8c63c33ddec01abd17e274818af6d3b0b0a2eeb7069f27824cef3044a6090489  examples/01_platformer/assets/sprites/cave_pillar_2_big_12_2_n.png
23b73461e0a0a63795bcabbb67f73b67022d37a102dcfa996dbe916f7bfcf708  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_0_e.png
8757d43dc61b5fa8ca8ea253f72c2e9941cede7f86f52e9538dd5020e7ff30d2  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_0_n.png
627b41efea756cf8e9fef3c89124e930246ed6f57a78d744d987f446df76a268  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_1.png
cddac20702a8eeacd5a38b1b5ac1876cc6850c0d6ee2d9023bb366e159a0dee7  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_1_e.png
b53cd23eba2b77fb441e77c0308dfed7f99de6f5920fb44409ab62bf753f4c75  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_1_n.png
2151afb96b2c5ae0ab702f6e8e85e386736802918c7809dd55cb8e4353c106c4  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_2.png
5eb969859298182aa1ef34de0a19086ead429d52161bc896f6b86fc0ee2b1612  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_2_e.png
d13857bfcbcc0b220ad321497f711b2fe1b6ff99efa45c76fbf83324feb50401  examples/01_platformer/assets/sprites/cave_pillar_2_big_13_2_n.png
b1fd5d763aa640f58b0c2b97409512a1dd9111b5c645356b71ce3fcbabaad1ae  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_0.png
4c894f7ce78d26cf0bc1d652c640555072644152af0137347b6a1338c0897cd9  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_0_e.png
0a627b244c30d7c228443b19ae6bd80e0a2622904b638e0049be63cc05c03b52  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_0_n.png
50a019c15af3cdde5646894120d98dba8c099cdbaf0a9dc2564e97d1511a6c5f  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_1.png
4c83df6a3736c72c0639189755cf9e3f3215db8b622f2f4c7ab151524c2f2786  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_1_e.png
e35dfbe15b4addcea2a48c5b078521ab2641f701830cd1ec78c8eb09d39b8ee0  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_1_n.png
8839cfa5cbd8110c2adeae637ccc7e12b03191cb752ca93445e5188960ebf955  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_2.png
b3c4b9056bf417f5ce24b06b5ad8385f7ea604be73e08ba2afba34306cd3e124  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_2_e.png
efbd877877a59ddd342a2a32b238c5f0e9caf296e3abab4d6981d9728c037c19  examples/01_platformer/assets/sprites/cave_pillar_2_big_14_2_n.png
8a742ac08551d28a9ac28e84d9ff5b3845a7fede1634722fe2755798928d664b  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_0_e.png
6ac9fc7586c023f378455371514090d64248974574947cfb13f8ea8d9d7d3e8a  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_0_n.png
d6738f1e3c9fa0b2489ca2617da712f8048de1a8de7e213d3717093de7d72850  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_1_e.png
f928dd35b6599a58d14a04d8effc5c3088204b67d605bb89659a6988626e54ae  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_1_n.png
4e801b48ea2e87769cab749389703c17b095b146cdeeb5b3b60739a0bda840f3  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_2_e.png
e68e08aa7fd6a5c804383e851638ec99b19e5799d8974d4915713055ce0fb961  examples/01_platformer/assets/sprites/cave_pillar_2_big_1_2_n.png
2971a95721fc568bdee958b2479d1edc12317bb0a583813c2a68e9bf04bb5188  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_0_e.png
455dcd4b8112aa8444646bc7f4a24102dedfe74ed9db34c4b9fadd64cb3e9b86  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_0_n.png
8155d622750da12dc41a6fa4d11786f06e3db721a2440e12f9062d46cb012b8c  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_1_e.png
cf27ec76cafa4bb5858d122f78d2063d6377ba25681e6dbb481f2fc10e2d4447  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_1_n.png
a3240ee097007414becf7e54349395fa9e78ab074b3fb068079564e277942d44  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_2_e.png
b575de810ffe3d1ac4701b6fe425d74f18967415cf632d67ec0303ca2b352f00  examples/01_platformer/assets/sprites/cave_pillar_2_big_2_2_n.png
685bfef6ec4d44bf8fe84e25a50bb0b744c83a8fb0d06493e9b5760f28b5e522  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_0_e.png
6e2b95d88faa34c87d57d38ef65fe7135f683c751d7bb477ea42a91aad193aba  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_0_n.png
6d42a51641079a65ee32a6dcd595eb25e495c2b74c480350d2962d607de34e70  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_1_e.png
308e96677af4b19a2cdc284b6923703f6377911bcba2ae070d1b1026030331e2  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_1_n.png
2a820dd6d367970d6683236fa58701b69cb593a3468ef69d540d104f11172cad  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_2_e.png
400d256b54fad07e4c451c9c198db839974e8b0d89a254d88c0a1f3514b57cfd  examples/01_platformer/assets/sprites/cave_pillar_2_big_3_2_n.png
de206c34ed258417484b9d8db0dae7c5b8d44042588b305379057942db97d605  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_0_e.png
c9b19753ce55204823f9b6ea47a9a610b4bd759b21268b512686ae9f9a329b4d  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_0_n.png
7c3f6b0f71e44fd1cbd4fb80c83431bac1dc03c87516befeac6a0afe9f9a119b  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_1_e.png
056ea7a280c86fc099dd8558a54474775a19717ab94b5ece7e0d89e995ad2f77  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_1_n.png
b13a701fa92f31941d866779092029fc492959764ff3d94e27fc2e0dd4bf91bb  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_2_e.png
827c3b2fdac71d090fb75ef58c99ef590509073cb79599f790a99af81b75267c  examples/01_platformer/assets/sprites/cave_pillar_2_big_4_2_n.png
74120de27c34eae553e6ed19cfe29faf2e14d6f936dfe8917b79a42a69b414a9  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_0_e.png
bf4dbe2579e98b7d5375239251b5e40f2b49d286fd54e7614913140edadf7477  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_0_n.png
bec9ad9a1c2ea32160dcb7c90bba7aca2f8ef06fee89788d3a894afd0aabf9e7  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_1_e.png
c8242f14789fd958d973e6d17d6186ecd8e685824dc40db86c0459c2a1989e34  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_1_n.png
0399393afd7cccea1906af907010a7ad283e3ad1b9db59696266f03c4eb972ac  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_2_e.png
44276b45da6e9af6599185e36cc8d20a3ea6d4748c4c234d37d94a8abd5bc781  examples/01_platformer/assets/sprites/cave_pillar_2_big_5_2_n.png
fc91cf8927bb7facf8761dd1848541d061e78af93921841dda06c34c0faeaf55  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_0_e.png
3d6d2c3cf6b6cdcb02f79ccbb5a96f2dc7eee83754b9d87febfcf20109b1d8a2  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_0_n.png
8fb4291fcda87120b03c097b7f0a535d96333e6f501560abe5374f8825858687  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_1.png
0b79b4b4c94651046ebfdcfc92dfca9da1d6e409f743807eefb3d23d34163391  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_1_e.png
2fafb432d66b2ff2b31a1235e3e90d01aa53d164ae4f74d63864b1e156e7922e  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_1_n.png
4da46c242a8747d01b9266c4faa14f1a6ec8aef769fd5aaf1849ca36aa7e97df  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_2_e.png
3304bcf6aef61ddee95809c44ce878484c3db8031933339df7ad1ccc35d176b0  examples/01_platformer/assets/sprites/cave_pillar_2_big_6_2_n.png
e83900d779f53fc8703d5e073edc8705a8afeaf1562a4c369a577f9bc2645bab  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_0_e.png
dd602eb71d259e7c735273316e1a3a58a33bdac24a29b85a8de33dbc86fe43ba  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_0_n.png
2a5a52f11ab5b1ec61d762390e38f52b753ef50749cf2a73d98751eca794b794  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_1.png
cbb94616322e1f15dd4004fdb999a2c263031c1da51c8a2f5eb86882e4424ea9  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_1_e.png
5d41a9a37e15ed6a8f557691097f9490ddf1fbffa58112e00c23663a503ab01b  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_1_n.png
5df241a9b52456a0a8d654097eba6fbf3c47f5fe3e957280f71514ca6be4ab2d  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_2_e.png
dc929c53fc01c7fd2f6bfe4bb32b9f9c3a8051e0a303675fff2b31d46709134d  examples/01_platformer/assets/sprites/cave_pillar_2_big_7_2_n.png
456b921db58501e7c216bd1ad7808818c348c8e2b3a0d6e624c923edbd4ea106  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_0_e.png
ab38b6ebfb3a40172f13f99c627f1bb6ecad7988c4a363d01bea8073cfa66cf9  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_0_n.png
8e8a1f2feea22684de63ce803b35c45a93e16ab5637ca404ff2413159e37ce3a  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_1.png
9ec8a8ab1a7cd4de213825469495075b96e81c4adfe5fb78c6fa61bfd731c6ab  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_1_e.png
17019dd497436fa21a8712bac827fbac573f695dd45d9f61d284e65d6c64d636  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_1_n.png
b85eae5c58a07f9794893cd26605327ddacf935efbba68edf6a9e6d3d20ff68a  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_2_e.png
94bbb2cdb607d0fdec99aecbcf58754ce4a3a52b1b970d2c5106030609de0df4  examples/01_platformer/assets/sprites/cave_pillar_2_big_8_2_n.png
84d97b33390b0129e76a503ca7913fe3eecbe2c3546c1770673c6d933986a673  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_0_e.png
50c58ebb8d5988b7018290647376df4b5f0e55de6fca8f2636af50ad584b68df  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_0_n.png
7bb9087f83a021fb6fd14e06fc1b67f1cdc88ef2353cecb5a75b1e6b88d6102a  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_1.png
092e50ae6974575b425732711d7d2bfa2af6c87baa48810003b740ec7ac8cc75  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_1_e.png
b88c49745ab9597ada9af2d2f8ac20c8aa3f376ec2db5f19f3ad07f418235681  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_1_n.png
2d4f911810d6ba4884183fc2bdefd0798606eb19a65f22c77ce5364ed48ab180  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_2_e.png
85cdc9d2a081c8bde033abc312218c56c005e05b548d857a95a18dc7ebb75f62  examples/01_platformer/assets/sprites/cave_pillar_2_big_9_2_n.png
88faf60be0f92886f6e22850d8f68a898d2f81e6b99ee06f1aee4e0ab2b44c59  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_0_e.png
0049dd9d7a8ff6a4432cef8ee5ab77f4821cfd6142a2ca7f82aa88948a02689f  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_0_n.png
cda0e47fccc837fb75a8716595bf803b65846c1c8cb1a8d90c709338e3a3cdfb  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_1_e.png
b48ba6560c542b9263966e07dfa3306350233da2023948ecb5e886e130b26fd1  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_1_n.png
ac4865e1867c9f395731c2567247f4c81197aa85eca72ebdbb13b7302f97b9dd  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_2_e.png
17ae14d6c11696471a1c84118b4e267dd84010e1d98cae0fd853d86f3bcbe716  examples/01_platformer/assets/sprites/cave_pillar_3_big_0_2_n.png
82cff1cea18856ec7e5464b7b5eac2e1cb3215fd7328e1c2805a9e6152c3ca53  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_0_e.png
549aeb1d52ea17c8f4386d98ac9f3d6a6f2eecd46d3c21884cd4c1e4a7f217d6  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_0_n.png
f065b859b9b1330e6df244891d2e6f5f55b8ff65ae107659e2a5b02ca22ded61  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_1_e.png
42741fdd68093647bc067e72cc9cdd53298946d3b2b33648602d0f612377a1d7  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_1_n.png
c5e1da62db6c463e36f1a47eac0dc748a90386c0c75f1008397b32e318bb5367  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_2_e.png
95871bb2438fbad640998d0345114bc569e86ba7578f9c8068da9ac261062791  examples/01_platformer/assets/sprites/cave_pillar_3_big_1_2_n.png
b6795bea07ba79713f49a0bcc67d40d9b55e8a6b0b160e7d598a87aab83ebcc3  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_0_e.png
4ddcc786cd991d1fc794d751384f46d0f4efc2f11d6467534524caf25a7ccbe5  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_0_n.png
8da038a4f3330cfbee2e867b167c7ce9fb0ebd36730cb833573c51db4123a943  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_1.png
f5062eb5617722013bbbbf5302768e44c51703eb8709912f0094f451a9c09eea  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_1_e.png
ee4843bf8f808a636eb93f30d98d31da69d32a8d3bf822a5e4cacb26da03225f  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_1_n.png
9e9d894a1c82a2f4e3b2758da22a8baae3932ec95eb92af78af4c9e5efb604f6  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_2_e.png
fb7ad44ae1605e5f766e803444cbda67dcf70ec909a4a8fae3f5ae1bc3a94612  examples/01_platformer/assets/sprites/cave_pillar_3_big_2_2_n.png
1a4f3fe15698aa37c9ce9043d867f7f8bc73f45d0333bdbf5e35e3044a46e6fe  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_0_e.png
00c6b9ec0dc7b837752b258db14dd9be0b3cd321839f80a8a7e5497693879dac  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_0_n.png
11a324ea1f057733ca7e2b80b960b7bdba242b8584534ad73f90b35853bb4eb3  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_1.png
c738c342998f73b70b0bcb043da64cf5a0f6a977bf59706bc4f09eeb33f8fa52  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_1_e.png
f3b7fc6d02aa937f54c7a68158765380b6086c3e8d7b467ce74d3dd545c6afc7  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_1_n.png
bf3d18b408a5673f49b02bba4fa974886d779bef96a221b3a179e4a7751980b8  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_2_e.png
c5ace5509bdc0ca0708b5ad34d65218083510f3b6ec542580f1e7ad1f145dec6  examples/01_platformer/assets/sprites/cave_pillar_3_big_3_2_n.png
4ec759f4979aa544b1169ecbea5dc0f8cf3c84578b1e0a071731cd6860d23509  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_0_e.png
4cc733d5ca007db64b48ef2df665fed416d17f7763f76b52480f7509706ac5ee  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_0_n.png
5af422994321ee4cdcc54d2573e21bcf6d7eba032df6a1332cc75ecdb3430863  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_1_e.png
eaa6b60bd14e75026eefe9b1cace0475f96fbf2fc40d8c0c44cbeeaa7b66bae1  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_1_n.png
bec0155a88d084bd8113e10c9ceb499cbd8b3f3a94facad561012359a07a2b47  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_2_e.png
a338ffbfddf42e632563b3d6c395f63ef7e9f81f1cd567f9425735d18a110102  examples/01_platformer/assets/sprites/cave_pillar_3_big_4_2_n.png
802c3c095ecffaf665068b12a1c9eb5731d0741642a8c29e3e6ed42278d14f32  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_0_e.png
722bc6230c8f9158ac2acb11cb3ac39833dcc36262dcccb84844911c3de238c5  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_0_n.png
0e02bcbcc29021cb38b01524dd012ae53cb38c61bab696454d4c90d4e4c4288e  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_1_e.png
df0ace3e64c30599474e9bd38bfe1933a7eb1099611c6ebc5ad26ae6e4952d50  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_1_n.png
af3ee300dba6eceb5549179da2ffca1d4f961c7c660ca1cbf857616b4445751f  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_2_e.png
8fbb837ee900842c92b3087d8d2cf20a08fd1cce446c2f98d9c0f697b6efa9ec  examples/01_platformer/assets/sprites/cave_pillar_4_big_0_2_n.png
e4454ec703ae6feff723a6a4d1b2eccfd11a3ad692611a086ef1bfb55ace1a59  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_0_e.png
cd32c1c43de644d0ad58096794c9edb43c796013f672f2a9bc518e110c9b9dcd  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_0_n.png
74e33f4ab80b90cf138ffb8754ac74d1635146c804bf85f2bcfa8bd26261a072  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_1.png
725ea5132c8ce48fbefdcd216a39c4676dfff0855f73beded15f76d9e9c40861  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_1_e.png
0868c7ab1dbfecdeaa055ebe2ffe237db3bf12b49fc605264976077eee6d38e6  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_1_n.png
07486692240f1733f79a1230dc721b884508ae2c1ca9fcc6a4cff6940559ac88  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_2_e.png
726aab0337558cee3c7575d1399c45052e7607a12f4f947a8ba1dc8b0f3e2c92  examples/01_platformer/assets/sprites/cave_pillar_4_big_1_2_n.png
21cc1a045a746dfd3d117819c9d4a83cacaca85bdefab13d5129f178f4e1ca23  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_0_e.png
8133e76ef56f987c56efdd77b0a7798e408409143cc8202dd00837dd60bba0ad  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_0_n.png
75ad223079328ef48b9a9cfa9e6d3515ab57d2339d8fbe04bee230a567bcf165  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_1.png
d3d72b42b888a8610778b8df3e0b7d081f0798753981a017e8039e3561a9198d  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_1_e.png
dbcceeb4a033970ac1c34658813a1da27e7d1717056d1967aec77962134f5c72  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_1_n.png
1dd844986588c00572e90bf9d5fa8e47612fb59373e0ff87501d83832b140f55  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_2_e.png
2f6410563152f6a53f0c68345c018a6b300034aaadcde895b828c0744d110739  examples/01_platformer/assets/sprites/cave_pillar_4_big_2_2_n.png
cc76ea89269bbb5462c67773ada168780d65f9c2f5b4417e0704463c80b8014e  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_0_e.png
268e1ae62107cbb1ac66b328fa25312a2afa0d46b751cf6c90ae40e60f19256e  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_0_n.png
68418c3f2d645ef45873581563e7e8141c35fc35c5801b21e56377defbd9a780  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_1.png
476c2d2fdf37688650d98502b5bf7271ddf357d19fa789ee81d8685cf211757a  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_1_e.png
907f43ed04bc6dd9ecfa8173f8b450547168e2c4ed80df5bd21d06dbde173c4a  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_1_n.png
0942ee91342aa2d60adb69953e70c71b026324998cfc28182ecd829fb925bfb0  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_2_e.png
02a8d33185c55161b60a9ebcb22a57c76d7cecc26bfaa7c2a4e684499f9ff684  examples/01_platformer/assets/sprites/cave_pillar_4_big_3_2_n.png
430e6ba30a00af875c2f9b651f657bff17e01c339becbb4b32701411b43a92ed  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_0_e.png
8ff8e9beaaed40414603e814f7579b4a4d80d2c493e5c17508204e92dee74eba  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_0_n.png
a492657109ba6de8578e22e39d870c9622c3434a4ac37061dcf00e3b18522e50  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_1_e.png
ea244474888fcbb9a8a72e58d549e2b437d43fa621a3df99f239a86cb57887de  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_1_n.png
1e64d8a3ffdec0dbcb07a8707a12b584d27620bba62230b7fb217af21c400bc2  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_2_e.png
d0df50028926b1ef545e99494e1fa1c1b54e030d819942370d40e71b760c51e9  examples/01_platformer/assets/sprites/cave_pillar_4_big_4_2_n.png
8a0eb353bc0b70547017dfe31ef0a5e8309b2533d871d6208424922c7ef95f2c  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_0_0.png
c2b208bed2039de749a1561f3353583ce247b9b5560cdcd3c4e32fa07320be9c  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_0_0_n.png
6ff1a7d648f24a9ad9322b6aa4e161630e87a9641af17e30443c6cb8b8ac92f0  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_0_1.png
7c6b59da3562a14eb6335a44dce374bbf19bce029ef91d1bbb8e97998718ef6a  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_0_1_n.png
be687e410cf87670b1a52c8ca675b2eb2439458691af83977c174236c2a686cd  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_1_0.png
6624d1d65a70df32b6d4c712c6c59f4c23babfc61c47653adf29bdc63d3a9611  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_1_0_n.png
013be039747512e21418ae4cc8f85b56d6aabea90285db7d5d9c0c654a484c96  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_1_1.png
958fcc8254d6aeed1d0e262e2a1bd98585deb35c48901ea7c47463644e7c577e  examples/01_platformer/assets/sprites/cave_tunnel_ember_big_1_1_n.png
ee7c27a861a01237f54b99677b72987e53e7023e656cb9c21205d29243a96393  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_0_0.png
a5af240b9199a85f97391be289a3fe441b92f9b48d89fcad857bb4b64456ab8d  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_0_0_n.png
e07720611cf11fd18f7f7340eab9024b7ee1b5664ff3d4229e0b72ce5addbaec  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_0_1.png
4a0ba2ef1304f73b96a30764da752c29e94c68c8414e0be293b75413fa82804f  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_0_1_n.png
fe059fa53455de9195a772afea59b952b25f627661bf5fdcbb30680ab1cce1ec  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_1_0.png
40ed989aef21054f2f8937f23ee38342165c1bae095fe69c03af5e45056bcbad  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_1_0_n.png
23773e73c0aa432cefce857c618c7ff6c1f4805a092ecbf764bc89f870189407  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_1_1.png
431cbebd85c3997f3626a0e948750d7ea76d331530b2c2bfe82aa7c83f363718  examples/01_platformer/assets/sprites/cave_tunnel_mine_big_1_1_n.png
1c7c0054d71ba7640f496e96df987c3f6276a9906f8a131a550e743dcccb0483  examples/01_platformer/assets/sprites/cave_wall_0_big_0_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_0_0_e.png
9c88d489bdce67b6921bbd12100031a56ff30a8bfa7f80b71bc41d7a9c63baa7  examples/01_platformer/assets/sprites/cave_wall_0_big_0_0_n.png
336f2860880692cc4b75c22b7b84961cb46a01ab7611248aa49ed966e281869b  examples/01_platformer/assets/sprites/cave_wall_0_big_0_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_0_1_e.png
6f0667d387d34a0cbbeac7e1b8f368d855c93d84b6e49164ffb8f0970e2ece49  examples/01_platformer/assets/sprites/cave_wall_0_big_0_1_n.png
ed3b42c18f129a5150b4e3cc25349f7dcd9b7431c797203b243382dde7cd2c4e  examples/01_platformer/assets/sprites/cave_wall_0_big_0_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_0_2_e.png
093178f86d0973eef9cc7d77dfb738d1ce678599895fd313f49f32ec992341d3  examples/01_platformer/assets/sprites/cave_wall_0_big_0_2_n.png
83e2f0c0f24d172fb1354931b255c28e484121ed38ac40d16c1da67d6c81e7c3  examples/01_platformer/assets/sprites/cave_wall_0_big_1_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_1_0_e.png
93a2f5ba564073fe9a2e6864f46fce969d62da04e63ce0725e17f2fcb79c3c1a  examples/01_platformer/assets/sprites/cave_wall_0_big_1_0_n.png
783f774af74748e98d0c5e89e5cad2860db4319de639ca5f6c43a6110ce4c483  examples/01_platformer/assets/sprites/cave_wall_0_big_1_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_1_1_e.png
1da3e3d8643fa21066470ee92d93e5af5751717ae1ea5b1c2fce1ffd0d58b122  examples/01_platformer/assets/sprites/cave_wall_0_big_1_1_n.png
d76fced66310f1c79347d6a8839ae8f18745b88c644e1e55db5ea92c4de3ad5a  examples/01_platformer/assets/sprites/cave_wall_0_big_1_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_1_2_e.png
91890a33be1dccb5c5524e43588b3ae8da188b5ac6c95b450a2119e1a2982ee7  examples/01_platformer/assets/sprites/cave_wall_0_big_1_2_n.png
aaf0283c9e990a427bba4534be34913f58879beb5593eb7ebc9a6a0616cd4863  examples/01_platformer/assets/sprites/cave_wall_0_big_2_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_2_0_e.png
e0c91a41d58e6536e617628fbb462463918ac5a9796300ad875a05c322cae2bb  examples/01_platformer/assets/sprites/cave_wall_0_big_2_0_n.png
480e085c815ea2fbd38caf6685f40ff660c15f05835c0dd150f440982b6492eb  examples/01_platformer/assets/sprites/cave_wall_0_big_2_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_2_1_e.png
f66c620020be7fd24c1cfcfb21c838b9131381a81f5c43bdf2a10d5de08b363e  examples/01_platformer/assets/sprites/cave_wall_0_big_2_1_n.png
5712bf246984f91290fce71270c7622efbc01338359a46b1e2714fbbb0681a49  examples/01_platformer/assets/sprites/cave_wall_0_big_2_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_2_2_e.png
433d7db18f3d1db094dd9ef30279638ea618e586a4ac4b39a37b6912406a04df  examples/01_platformer/assets/sprites/cave_wall_0_big_2_2_n.png
0718e9d015eedea8d9b135bf26fd3f4cc193d1a2e773257dcaa17ca96e453569  examples/01_platformer/assets/sprites/cave_wall_0_big_3_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_3_0_e.png
1182003f69cceb484a90abb5aa729c11d67153319ce49bd18a038442eaa7bcdd  examples/01_platformer/assets/sprites/cave_wall_0_big_3_0_n.png
0a95818e47c1aeaca98590531bb81cb513ec5bb236eef18fb84a44e1de374425  examples/01_platformer/assets/sprites/cave_wall_0_big_3_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_3_1_e.png
d15739eef0bd565edabfa917bd17baec95bc606e8d620a2819e8ba003e56f7ce  examples/01_platformer/assets/sprites/cave_wall_0_big_3_1_n.png
2cb024910f84e9eed6287a16387a369357b2175a7dc8185df90f95c5ca80004f  examples/01_platformer/assets/sprites/cave_wall_0_big_3_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_3_2_e.png
765ee55a56814a5a19f9e9e0da500f258b1f7132e3be3d7f986564acdb60cacc  examples/01_platformer/assets/sprites/cave_wall_0_big_3_2_n.png
8dfdb67a5d989a7ae92eaf2b12438ff1dcfa452d1a89aa59fa8456fb895de4aa  examples/01_platformer/assets/sprites/cave_wall_0_big_4_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_4_0_e.png
1cd4e06520131971317ab9008033d5af1d3140e23ae584fd44dcaad1e38ff640  examples/01_platformer/assets/sprites/cave_wall_0_big_4_0_n.png
1de616c814a2c5fa830de14b69c02e8fe3a0d07028999e5072fbc51f64013dd5  examples/01_platformer/assets/sprites/cave_wall_0_big_4_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_4_1_e.png
80fff90edcb5bc8ab1d30e2e0769e5ca8e52e01aba3206a723dd36ea8d9a2215  examples/01_platformer/assets/sprites/cave_wall_0_big_4_1_n.png
c55f2ea2001e6d59c023fc6933d87ffb34ecbe08554180324dfe7ac303438bff  examples/01_platformer/assets/sprites/cave_wall_0_big_4_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_4_2_e.png
40762a0fb8724eaf6e4045a7a8d16674931c3439adbdee065fa465c415309e49  examples/01_platformer/assets/sprites/cave_wall_0_big_4_2_n.png
c019e643c70cd3b4bdabd98229aa8df17e2bc624f9dda12c5b8a901a8d40b1d9  examples/01_platformer/assets/sprites/cave_wall_0_big_5_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_5_0_e.png
d8d1fcd07f11474e0fcbea86828f8c6675205f0de9039fa6d53cc850745504a2  examples/01_platformer/assets/sprites/cave_wall_0_big_5_0_n.png
ad04f366c29d363cad1f6e3de71bd6291eb4781b68140448d6d98b9aba812599  examples/01_platformer/assets/sprites/cave_wall_0_big_5_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_5_1_e.png
3fb0673941905e3d342e29580c668dbc1bf6836aec299b62a1e73441a49d38dc  examples/01_platformer/assets/sprites/cave_wall_0_big_5_1_n.png
481e321f8e2ab1ca84af4e962207c251998c7506ed2414555296b31fe1d5e524  examples/01_platformer/assets/sprites/cave_wall_0_big_5_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_5_2_e.png
be8541e55dcc021cc682df22e36454239b28fcf2ad1febf2a0df7de792063d1d  examples/01_platformer/assets/sprites/cave_wall_0_big_5_2_n.png
d5ed321883561e8e0b527be0b15abcc88528c03d35e426a576fb314ccdfd61f4  examples/01_platformer/assets/sprites/cave_wall_0_big_6_0.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_6_0_e.png
cc3ad61f0af70369182aa8ef0e4c5103069c4a88b887dd8ec104f5beb08df253  examples/01_platformer/assets/sprites/cave_wall_0_big_6_0_n.png
e8adcf8ffa1cb5b16a4b4fb14d2f258fd821976c2ea22e0e7f2b3dd0309eaa53  examples/01_platformer/assets/sprites/cave_wall_0_big_6_1.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_6_1_e.png
237f4fcf879621a70b58dd73593ca58f5f88f4c378807763626e968880604a44  examples/01_platformer/assets/sprites/cave_wall_0_big_6_1_n.png
5cb26c28304834d27658fbe854e265b1f94e9edfdae1d2e82fda426ec256a19e  examples/01_platformer/assets/sprites/cave_wall_0_big_6_2.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/cave_wall_0_big_6_2_e.png
c0c0953f7867677fcc8b1c6fae393f67d091cd0a7dbd3cf44a445d977c3dce34  examples/01_platformer/assets/sprites/cave_wall_0_big_6_2_n.png
2c16f5d6d7a60b81881c3b50be2c9ed42023fe567e7f5b5c8818b8c92bf4e3ad  examples/01_platformer/assets/sprites/ember_pool_0_0.png
d3cc7bc693a9345cb7b718b9cde16e774f85773bae52344314c3b477a5ad7d91  examples/01_platformer/assets/sprites/ember_pool_0_0_e.png
9903540f78e2051a2607e5e72d0b10f2ca93d535b6bf86f271fed926fd4962cf  examples/01_platformer/assets/sprites/ember_pool_0_0_n.png
92d3d4304f827ec143e6a9100bc17a12a5eda74a124bec683ec416b99ada4e3c  examples/01_platformer/assets/sprites/ember_pool_0_1.png
2e73a9770b2bb6f00ce704ca8d54032a28fca86c294540f37f45b7255ee9fbf5  examples/01_platformer/assets/sprites/ember_pool_0_1_e.png
ea9ae091c6f37278a3f9ac9830aa4ef7dac51c1d51b5254e0d30093e259cf5bf  examples/01_platformer/assets/sprites/ember_pool_0_1_n.png
3858f96ff47479f54f9430f57c4b96189f8879f610c170b46ba7e22b3dc395be  examples/01_platformer/assets/sprites/ember_pool_0_2.png
96ba9ce5d5a5d7b7c0b9f3736f654cb5468d63a3d181acd8a043a411b0f8ed9a  examples/01_platformer/assets/sprites/ember_pool_0_2_e.png
0ec5a73fb35ebbcc312e2a54be4ba758bad640840b49f19ef70e158916a5e6d8  examples/01_platformer/assets/sprites/ember_pool_0_2_n.png
02365b50b5e76839be37e9e10db1ad34e8afb5e81cab632520ab8e41d162993a  examples/01_platformer/assets/sprites/ember_pool_0_3.png
fa791280ac514d4e0df6cc276ca037e9316888464a87ecf2b58e8f8b765f1cba  examples/01_platformer/assets/sprites/ember_pool_0_3_e.png
b834e347332cab3b8e61be5234e6635ad7644fffad175b444c4060091592c71b  examples/01_platformer/assets/sprites/ember_pool_0_3_n.png
08bd49e80ee8715e573b2a008ff98f34aa27e872a98ade854703d54071dba5b6  examples/01_platformer/assets/sprites/ember_pool_1_0.png
23f9ea8d73977d7bc13de43f17b88a3a199c90c3895cfacafea059d82dca10f8  examples/01_platformer/assets/sprites/ember_pool_1_0_e.png
9a862858b7a4d4956d8bb7f8051b9b7d78c202c8612c09d574597899c80bbb7e  examples/01_platformer/assets/sprites/ember_pool_1_0_n.png
2ca8d381baf87a432a90b0e1b32f37a71b96cdc9c52ff72da0639d1ca896e55b  examples/01_platformer/assets/sprites/ember_pool_1_1.png
dc6e85e25f687fe2027d95b2469d112c4697a4780fa3a3eea589569e431d356d  examples/01_platformer/assets/sprites/ember_pool_1_1_e.png
a337a743d2cee4ef8bce4d4c74f3b1ca03ce58793e1270e5ba5604facca4b516  examples/01_platformer/assets/sprites/ember_pool_1_1_n.png
b8beae47ff6006f66e5a82617888034996edaf4fea0cb4ddd08b5b13f7832104  examples/01_platformer/assets/sprites/ember_pool_1_2.png
1c1f1cb1308afc7bba0d7d0dee68cca8505552ea61d41a01c290b3922a74ee45  examples/01_platformer/assets/sprites/ember_pool_1_2_e.png
cefcc73f9243a3183e16a8dab4141db7b1badb76b460d4d0bbe7f9ee316fde01  examples/01_platformer/assets/sprites/ember_pool_1_2_n.png
1e550379fa9bb20f1b17853fa2dcbefe124847ea9c7db12178417b5748eb7a03  examples/01_platformer/assets/sprites/ember_pool_1_3.png
6f80ff672660bfe2be01bf02378e75bb3bb61759ee7587409ef4cfb13e5f3151  examples/01_platformer/assets/sprites/ember_pool_1_3_e.png
ceda614a43643d8410ba1827d13ee0219123f7647f4b81848b6cb1b1c35b55e6  examples/01_platformer/assets/sprites/ember_pool_1_3_n.png
ce83ef72e5174c8ee5807bf99b7a6b8c7fb44788d0a2784e1541ed43655694e1  examples/01_platformer/assets/sprites/fallen_slab.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/fallen_slab_e.png
8998b54312e9c1e20a66de233d2880fb02d2dfa36408457b3b9ee33cddb08a23  examples/01_platformer/assets/sprites/fallen_slab_n.png
eaa99be79d98783e04a6a03005e55e4e9d4d75f1a5a43486816624649c0861f4  examples/01_platformer/assets/sprites/fire_crystal_0.png
d85c544ffc2c946b4bf7b1e87e0e9bdbb423a7664543e0e5799362c6b2e1c6f4  examples/01_platformer/assets/sprites/fire_crystal_0_e.png
b4a330a24446a8c7922057df8db0592bfcbe1bd64d998240ff881de14e959460  examples/01_platformer/assets/sprites/fire_crystal_0_n.png
c2c9433553a5666fa09bd5183088b367c89d4fe9e0962f08555a463ce6f93b24  examples/01_platformer/assets/sprites/fire_crystal_1.png
402a6ff39c338d0c07fca35bc05c52a193e5fd9c7686bf618f0dfe47d59cab9d  examples/01_platformer/assets/sprites/fire_crystal_1_e.png
85279b7d25d2fee51d2e9c8a6d4245e6759dd8e13ae5a2d0914722c636006af1  examples/01_platformer/assets/sprites/fire_crystal_1_n.png
d0cc2695e7d467188ca22c88c7167aa0bf8358133715ec1d037143dcc1a5301c  examples/01_platformer/assets/sprites/fire_crystal_2.png
5688d4ecbc1d93d5f84e3fbdc76bbde61e734fac00784a9813820f46f55d5a44  examples/01_platformer/assets/sprites/fire_crystal_2_e.png
b7dc3eff1c306460e485c10933e513de2f976d14cd9cda4a52cc7c1456c2b142  examples/01_platformer/assets/sprites/fire_crystal_2_n.png
06a860e563080a3406f5eedcdd19ff174b534539e543a5a3eef0dc95e4b1e4a5  examples/01_platformer/assets/sprites/fire_crystal_hanging_0.png
e27e75fd0cf3361676029125fd06daf9d2e05ccc7fa2918f306e9baf2082f758  examples/01_platformer/assets/sprites/fire_crystal_hanging_0_e.png
b814525f160a98c2015266580bdb2e45f7829be9afc6965a8a1a4fa889e46479  examples/01_platformer/assets/sprites/fire_crystal_hanging_0_n.png
d2a1f62b6a8e00fc888b0d82f6ca54f8c06ef155285c47c142de2e4e36e030e6  examples/01_platformer/assets/sprites/fire_crystal_hanging_1.png
b2b29a1c7c8e3a9eefc355ca5df10dac48bef9baec0b8a32d8f01985dd902490  examples/01_platformer/assets/sprites/fire_crystal_hanging_1_e.png
f922b8059af401d4ea7a3117bff7c10e325f891e2dd67407f04337ce93286401  examples/01_platformer/assets/sprites/fire_crystal_hanging_1_n.png
9a3231b605a3d959df3fb64ba9cc5aaf0e73cc0cfce14dd7482fe5ce9a5b1301  examples/01_platformer/assets/sprites/fissure.png
4449e247e446705d550b6448bb3df3ba10af1895e399bd8b8a2537d2bde8f160  examples/01_platformer/assets/sprites/fissure_e.png
8d87b654f510258290d469ce2e9679e151dca79a704f68c1dbd2294b2482f78c  examples/01_platformer/assets/sprites/fissure_n.png
8d25a7d8d6c9c15a15497278b0ae023aab2531da559109d80fe2b97ebb2dafe4  examples/01_platformer/assets/sprites/geyser_plume_0.png
6c8eb76c0cf9473335fe2041c779f2b2fe39ddaff474e8fecd71338b3281e9ee  examples/01_platformer/assets/sprites/geyser_plume_1.png
050c5801e50ae46013c9f03cb0133a1850307bcf0190f20a23b9a2dd88994c5a  examples/01_platformer/assets/sprites/geyser_plume_2.png
83a3089172d209841277cbf6c60e8c01ccf398a95db8b8b93e0282223b42574a  examples/01_platformer/assets/sprites/geyser_plume_3.png
3d0b2b286a45af5fe67c47735bdb2219d55a2022c775d09cd3934f263a59c65a  examples/01_platformer/assets/sprites/geyser_plume_4.png
947cbd2c91874854229800f7d7b66fd9d4c844b13c9c4aac96b81cafe754fe4b  examples/01_platformer/assets/sprites/geyser_plume_5.png
8fc3cfeb7fa19f7965cc7bba52a0363efd0a5909d445a4988f9458c80265c14c  examples/01_platformer/assets/sprites/geyser_sputter_0.png
38bb8a8b9ff7a2dc636d58f0a3100a5e3f8cff4080cd4ec4ef4744923c41813c  examples/01_platformer/assets/sprites/geyser_sputter_1.png
8bef98ed0277167412eb926737467d0d5702e24936e70be1d47f77eb67b08aa7  examples/01_platformer/assets/sprites/geyser_sputter_2.png
89b3973dfbecf43245bc2c5c04891a24c947a4de75a399eb20a0065c3afae8ef  examples/01_platformer/assets/sprites/geyser_sputter_3.png
c813ec831038593c21b126215c8e9960db1d050093b1c146f614fe71dc8cdd51  examples/01_platformer/assets/sprites/geyser_vent.png
4cdefb8e9c022a210be9e48dd6a8a4cbd84ee475e0eb097d4004906d724a2583  examples/01_platformer/assets/sprites/geyser_vent_choked.png
825045860f4ddfbba6940075541583bdc40d2396972b3d941e0309abe808e863  examples/01_platformer/assets/sprites/geyser_vent_choked_n.png
232a1e4299db4be8674f9c367c55f580ccb7730b17d4178c9b3be16b0b8f9ee5  examples/01_platformer/assets/sprites/geyser_vent_e.png
23ad4c38514871b1e6f707553d463a659377085c30d815605eeb07c3b036efad  examples/01_platformer/assets/sprites/geyser_vent_n.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_0_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_0_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_0_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_0_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_0_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_0_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_1_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_1_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_1_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_1_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_1_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_1_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_2_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_2_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_2_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_2_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_2_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_2_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_3_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_3_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_3_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_3_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_3_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_3_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_4_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_4_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_4_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_4_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_4_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_4_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_5_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_5_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_5_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_5_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_5_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_5_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_6_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_6_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_6_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_6_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_6_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_6_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_7_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_7_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_7_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_7_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_7_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_7_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_8_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_8_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_8_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_8_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_8_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_8_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_9_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_9_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_9_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_9_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_9_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_big_0_9_5.png
a0f4607d0ced9fd68f5f230ac1c5fcf2a84dedf9574cb517512d40e243e71fef  examples/01_platformer/assets/sprites/haze_big_1_0_0.png
3ef3ff2c50289f559b1cc9b7fde963e6d5b0bac8830c19a8175f5a3c5135e1b1  examples/01_platformer/assets/sprites/haze_big_1_0_1.png
adee874dcf6dfd28e5df330919b026c1546cfc550b39c2826da79339371a7ee4  examples/01_platformer/assets/sprites/haze_big_1_0_2.png
8d300ec970de34c22c0d4b9d02135978125f54f8c69da6fe0737dbbf79dab375  examples/01_platformer/assets/sprites/haze_big_1_0_3.png
785bdd607f5941d2e3e860ea86dd4223a98eb87462f23393c46313e44dface96  examples/01_platformer/assets/sprites/haze_big_1_0_4.png
7b16c4e214f4dc6b94cc311dec567fa37b82a4dfe32f60ac3aab7ff77a9678ac  examples/01_platformer/assets/sprites/haze_big_1_0_5.png
bef47e4f9d408b41af41e0b3d7a0b87c324fda8cb77573a4bdb0e161c5994b03  examples/01_platformer/assets/sprites/haze_big_1_1_0.png
ca974eadce5d4baa1e62697c84983678e5c7cade8c63f87f621b37160753a7a1  examples/01_platformer/assets/sprites/haze_big_1_1_1.png
389a060cbd7a3f4963694867d8ec29a4d9dee3d2129e586f43c0d09b109de558  examples/01_platformer/assets/sprites/haze_big_1_1_2.png
27eee67224964f322b3a3bd56b093edce0604d9b53cf75b6e3fc9bcf9f8d4924  examples/01_platformer/assets/sprites/haze_big_1_1_3.png
dc7af0938782db4bf619010995b71fc59e40a237fee16deed1099fe02f74b891  examples/01_platformer/assets/sprites/haze_big_1_1_4.png
b80d137383b11cbc29307bd833c4853c1efe8f2c713b6019a0d134715d44f8cf  examples/01_platformer/assets/sprites/haze_big_1_1_5.png
13b5880b617063e9c5fb89b80a7b752f7a235a485e121f8be30b9015e6885920  examples/01_platformer/assets/sprites/haze_big_1_2_0.png
a2e21229e2c305cc77d0128eb0443997803498456b83b1eae4a4a20360b1f836  examples/01_platformer/assets/sprites/haze_big_1_2_1.png
e98bc7693d2537c10bfbe708b3e240080073b64fba96c40ecaf1bb02a7b2289a  examples/01_platformer/assets/sprites/haze_big_1_2_2.png
bfacd58573627aa54353049fc2ecac195d4605866ea806773f7390544a4fca76  examples/01_platformer/assets/sprites/haze_big_1_2_3.png
88eff95eef77b3a41e44c1bf50e2f5dfb78c6c1e77f536333eb454852d18cc39  examples/01_platformer/assets/sprites/haze_big_1_2_4.png
4960076e33d855e70fa1a7b3ebf0efeea7379b52170ee2a384a1ef26535905ac  examples/01_platformer/assets/sprites/haze_big_1_2_5.png
d61022922d4a3efa8b7b2cf0f0bec10b8bd8f8185a5d5290bfc4cd4178e3c76c  examples/01_platformer/assets/sprites/haze_big_1_3_0.png
34264dc1c5dc1b9d0112797de21aa29b19a5b57669215f29e741bfd55ffd9995  examples/01_platformer/assets/sprites/haze_big_1_3_1.png
59d6e69b8368ca78da3bf559de79c743fa01deaa1dddb1a0aae0b86fdadb7311  examples/01_platformer/assets/sprites/haze_big_1_3_2.png
17d3c6c8297671bee165891151933b0720b3122e71e8c7808b8676d7dcab29f9  examples/01_platformer/assets/sprites/haze_big_1_3_3.png
731c5723de0bb012aa787e7e5eb8f8d4c9f899e8dbb73e68f91441a24aae91cd  examples/01_platformer/assets/sprites/haze_big_1_3_4.png
f9edee61226a20e367d7683bbf0174d94f113e4ff2e9feedd1ffa1b6a54c7dc9  examples/01_platformer/assets/sprites/haze_big_1_3_5.png
f82b7ba7ea974a9607cefc6d014548fa4382e7209dff97b53db69404deddf4f2  examples/01_platformer/assets/sprites/haze_big_1_4_0.png
38769aa5363f99a25b1233d46d4364b3f5d885dc9a49ba1d95b5b60f7f45b280  examples/01_platformer/assets/sprites/haze_big_1_4_1.png
0d3d0702807932d7d62b03e4741a6b42494c6986b3e1022ae589a3439de95258  examples/01_platformer/assets/sprites/haze_big_1_4_2.png
5fbe7adb07d11fd1c5b04b3096b79eaf36ea6a8d68c9f7a683159d87f9c0fafc  examples/01_platformer/assets/sprites/haze_big_1_4_3.png
f368b4d84969371d1568e11876636e5a62b9a86856d3ad9be735e21af7a9d64a  examples/01_platformer/assets/sprites/haze_big_1_4_4.png
eee6a109e7609ed3d25f95064bf2c61ae1f8308ee7682a18006c0bce8d4b0f7e  examples/01_platformer/assets/sprites/haze_big_1_4_5.png
68c4a2ac25a3eb474927d2d0ada4531ca47b66c31abecfd55feb277b2ce9058d  examples/01_platformer/assets/sprites/haze_big_1_5_0.png
6e6d334996c7c2472559add513469f227fcb3b5562e79d12f33906a5ec4cbfc9  examples/01_platformer/assets/sprites/haze_big_1_5_1.png
235575e45659ed4f4d3d5d2b28e8a0091e9949c4dd7430e8cf25404ce723a9b8  examples/01_platformer/assets/sprites/haze_big_1_5_2.png
18db0ffa1792b86624882622c5f56de4f220a23b3a40b905e8d23cbf0297b78a  examples/01_platformer/assets/sprites/haze_big_1_5_3.png
7421bd91682fa6174627a6409cf3226ca4a95be636cc1b35f785d7339cb3123e  examples/01_platformer/assets/sprites/haze_big_1_5_4.png
617e369d9ebc9002ddff33614caf114815dba0c5780c74de126d14dcc6423e8e  examples/01_platformer/assets/sprites/haze_big_1_5_5.png
4dfb2cfad50fc14915d44a49d664d9af8dae1cac4c083ff95b693b51004c1f71  examples/01_platformer/assets/sprites/haze_big_1_6_0.png
6e29ffd7bb51a84093599a24a4db3a7a730d6b1a2372c64267c820d83838f346  examples/01_platformer/assets/sprites/haze_big_1_6_1.png
ca6dc90d7cfe0f31dc7d877c65c5fe80ed41b0b3c2d3e8102ee98ebb1249c1bf  examples/01_platformer/assets/sprites/haze_big_1_6_2.png
ee59deab8acd4778521e68ee4ee37ded35193b51eb92f7804cdc73a7c98b3ea5  examples/01_platformer/assets/sprites/haze_big_1_6_3.png
1e243d4315aaf709baf0bc4727dce240e779f1fc8201a124ae6baa1522e42a42  examples/01_platformer/assets/sprites/haze_big_1_6_4.png
c901b137f1c5bb5c4cb8df6f99472a2d04b7c68d6d0fd4a0f5fe24adcc0100fe  examples/01_platformer/assets/sprites/haze_big_1_6_5.png
605406ab9960ccb47e05a56ad76b7415976dd19f72d2d58a4b8bfb655092a93e  examples/01_platformer/assets/sprites/haze_big_1_7_0.png
bb5a4408786db1412750ad1335e434561bf59646dd4dfc47c829215bb7671f38  examples/01_platformer/assets/sprites/haze_big_1_7_1.png
173dbf5b7d629262cd5273c77a845949af96d6033b2d56c20196c8e74af2c767  examples/01_platformer/assets/sprites/haze_big_1_7_2.png
40e4dc9443e4154d4d94317c75fa8f21b35c1a1e9f44c735ffff93a7971cacb6  examples/01_platformer/assets/sprites/haze_big_1_7_3.png
20152431409edaf128dce03c4e79f7b3beee6ec1909176a707ffe30ebfe4a272  examples/01_platformer/assets/sprites/haze_big_1_7_4.png
f92bb882fce45b1907c6cfce87f398ebd25cf559c82d9c5f9912313e67ab4e95  examples/01_platformer/assets/sprites/haze_big_1_7_5.png
58465430d51136a2af686dd725745f998b70286a9de43abc65febc490ccf58a1  examples/01_platformer/assets/sprites/haze_big_1_8_0.png
dcf01a738cc9701ecb878f0160c13c7d6f07b0457a6f61eeba98572561d24e3c  examples/01_platformer/assets/sprites/haze_big_1_8_1.png
eb88248d3bd9d4fe85a496af61722a8b253b2aedfb3b931f899581e1e5545c97  examples/01_platformer/assets/sprites/haze_big_1_8_2.png
c81bacd609831259218934327b9db95f8d4d6dec0b3e64cce4b81f44a19579c3  examples/01_platformer/assets/sprites/haze_big_1_8_3.png
c179cabeb1ca753ddfc4faa5a28ea64c1214930db856a15b8c5fbe5e88befde6  examples/01_platformer/assets/sprites/haze_big_1_8_4.png
c8b73d65d35fdd316e2bd7e4a479b3b4bc36dd9c44e88be7bc978899c5a0c3e6  examples/01_platformer/assets/sprites/haze_big_1_8_5.png
e7bb423276d930a820227b94792d55c2f2043b4a3697fc0fbf4f0c1a15dac6c4  examples/01_platformer/assets/sprites/haze_big_1_9_0.png
60943bc7108c626f4536d190ece378d9982e94f778cc0028a2b967092296bc80  examples/01_platformer/assets/sprites/haze_big_1_9_1.png
fca7c408c93aaaa6dc6317f9559400d85f38109153094b56086d051cd5b4c7e6  examples/01_platformer/assets/sprites/haze_big_1_9_2.png
a60e89ef0b7e9bf41d70738621769cb7602bb638b10d30f7fcfd6575669229bb  examples/01_platformer/assets/sprites/haze_big_1_9_3.png
a156b973025af3319ad5bee87c4976d593b79d3c4b02def5c7e812d90b6d2fa5  examples/01_platformer/assets/sprites/haze_big_1_9_4.png
961c97fa4d789985c390b59d71f568dde574ccc6c3597a92ac657468d56c82ba  examples/01_platformer/assets/sprites/haze_big_1_9_5.png
22bf004c1950dd35101bd0d424fba3fe359f30989681fb60869ebc58a7317aef  examples/01_platformer/assets/sprites/haze_big_2_0_0.png
a05ca6dc0738c5911b3b00d58cd059d7f437716a704cc73581e41d1d62001c85  examples/01_platformer/assets/sprites/haze_big_2_0_1.png
eb814af9c5174718a61b01869e86a4e6c420f4c827e9d1eda7a05321d7ad6f0d  examples/01_platformer/assets/sprites/haze_big_2_0_2.png
040f7b086fdf400dcd7c6e8b913aec00adfefccd9dccb130264cf0b02ea293bc  examples/01_platformer/assets/sprites/haze_big_2_0_3.png
1815fe05062aeb302f4db0880ac6d2c9b739323cd04fa4562119291014be5c7d  examples/01_platformer/assets/sprites/haze_big_2_0_4.png
d3dfccf99d7b86b58175d2be9b45822158651e23c9833fec9a081e121cf2d653  examples/01_platformer/assets/sprites/haze_big_2_0_5.png
0453e172c7191f7dbfd44656005b99cc3fb3260e81329a3814dc28200ff7ce9b  examples/01_platformer/assets/sprites/haze_big_2_1_0.png
c075d1e6381f2c2f8d721facd2e274664993fa668c28a55426ee8e2af596e9a3  examples/01_platformer/assets/sprites/haze_big_2_1_1.png
c971eba2c13fa010ea0e0805a061626ebdf68a3b67741cfe6b9ebf4143ab85ea  examples/01_platformer/assets/sprites/haze_big_2_1_2.png
426f9a7bb64df60965a8340ad9a1943d71fd37c7bef7808ce99c0348428e5da4  examples/01_platformer/assets/sprites/haze_big_2_1_3.png
1110b55b1b6da7ce9400d9f859683da523e165806d54a2c4ef3313d478869688  examples/01_platformer/assets/sprites/haze_big_2_1_4.png
55c1e399f26e4e905445d55c98060f397aca427c1d3481d05bb89d5ceb5a7c55  examples/01_platformer/assets/sprites/haze_big_2_1_5.png
a1b221bf8a32435ceee5b803aece9a7f81347211d7a2244abd4b506df79603cc  examples/01_platformer/assets/sprites/haze_big_2_2_0.png
a3a439787e38514f4ed4fbcc9cc28a71af56e3ca5f85e58f679eb642774f8497  examples/01_platformer/assets/sprites/haze_big_2_2_1.png
6ccd39b804c5e615f5398bc67b5f6900be3e9d9d09d398f29dce7bca89a48f2e  examples/01_platformer/assets/sprites/haze_big_2_2_2.png
e3cdef07045797c126ba461e27d79b5b10c922ccb0335df52b11cea2c0440f80  examples/01_platformer/assets/sprites/haze_big_2_2_3.png
8a4339a35d40eabae6d040b6b4f79cf4c65a67a17ea26bc84e6c1ec98bd40b80  examples/01_platformer/assets/sprites/haze_big_2_2_4.png
2565f37db577d0f52ea250fa28a4510b8151b08427f922838f4a2411153b15bf  examples/01_platformer/assets/sprites/haze_big_2_2_5.png
9359cedda612499d538e6158a559eaa2e53b8f79ea29afcd059d1db7c2109c29  examples/01_platformer/assets/sprites/haze_big_2_3_0.png
47a96a07214c941eb79afae87f1b04800f484d8492f5ba6b08d1435be1975f06  examples/01_platformer/assets/sprites/haze_big_2_3_1.png
a63d7aff20054657269fb7b853c3363acf4d95283871c13deba3a9e2e985b787  examples/01_platformer/assets/sprites/haze_big_2_3_2.png
489161d565128b44fd17b0d67baad82b480a7a9861bb2338dead6b1950a0078d  examples/01_platformer/assets/sprites/haze_big_2_3_3.png
fc74a1d468abc8cdb0ea86701bbe1e31beba05c01971c771b6ed5bae290b7e16  examples/01_platformer/assets/sprites/haze_big_2_3_4.png
e83df9e7cc58e5a38889dfd77efd7d1717e9c0ec3dbf82dc1d45f2406cddb9c3  examples/01_platformer/assets/sprites/haze_big_2_3_5.png
169a5f09ab3a3eed715adc06aa6de28e5fd2755ea8bb5c5a90e2cf85e5b05757  examples/01_platformer/assets/sprites/haze_big_2_4_0.png
5df09373a0120b00479e6f851e8abcdab4273ddc639cb7a3d10cb0186c3d3355  examples/01_platformer/assets/sprites/haze_big_2_4_1.png
fd5ec869a61ae120b9ce6586c5d620e603b9f7e0f794d767390c132db11b2d9c  examples/01_platformer/assets/sprites/haze_big_2_4_2.png
a846311f38fec02c7e7b1c45721ed0d17a17ed4635ed3bba6e16a80e35a3aad1  examples/01_platformer/assets/sprites/haze_big_2_4_3.png
9eb75fc000d04d14bc3db161aac6c2be773618d3ad3bd8b31b3285e0e26608fd  examples/01_platformer/assets/sprites/haze_big_2_4_4.png
1d501f5a3e51a3e5428e541617a5ad61b27a009123e6f4e6fae6c15722fa3365  examples/01_platformer/assets/sprites/haze_big_2_4_5.png
cbcff9d316a01d1d81400b422979db2159a545e6d6c23cad16c18eaaf5acd2dd  examples/01_platformer/assets/sprites/haze_big_2_5_0.png
2ed68711b57897c95b5d6d4a6f300a7b58b5e7448a34aa26fa7211c81ddb7d7e  examples/01_platformer/assets/sprites/haze_big_2_5_1.png
7be86a27b956b85267302d8456888cb3b83730069f972e4a9fedb5f7ec9f9c00  examples/01_platformer/assets/sprites/haze_big_2_5_2.png
990205afe0e681c017c03470593cba61e6e9a7e3d5cc546e45774edc4894eaf9  examples/01_platformer/assets/sprites/haze_big_2_5_3.png
1c9d9eda8cbc63be890e42e591c0e45b45832bc6a032b11c67a3c4c655ecaad1  examples/01_platformer/assets/sprites/haze_big_2_5_4.png
4382736be57fa1771d4cd7b36e42d78be9276c14e70cbc933f6dc9f84f798af7  examples/01_platformer/assets/sprites/haze_big_2_5_5.png
ad65d6f3fae9d1a17f071cbe7d93feb90f5e2d50b2e97661fe4342e44c9a2968  examples/01_platformer/assets/sprites/haze_big_2_6_0.png
503a04e666f595cdfc9c1eb731cfdef6033c5a390e16f039f4e2e6eddc4ff7e8  examples/01_platformer/assets/sprites/haze_big_2_6_1.png
ed39c2cfbd976f1727751115a09972e19694a7986164d43df1470d905ce6c3e4  examples/01_platformer/assets/sprites/haze_big_2_6_2.png
a01576ac176b9b95997ed5060581b850955c03c42d39c3a8282d81dcac17a775  examples/01_platformer/assets/sprites/haze_big_2_6_3.png
a915d89e59695fd360535b2fe5499cd508f6872ee991873f0e90686928f39f0e  examples/01_platformer/assets/sprites/haze_big_2_6_4.png
844c1c8ad4f32daa3c76bd6148504bd0f2a67fb6d1da4109d90bb235bf58d5d0  examples/01_platformer/assets/sprites/haze_big_2_6_5.png
207c1d6f3567f7d8ff402706d2b4150530cc568069740969732d04091f7b4814  examples/01_platformer/assets/sprites/haze_big_2_7_0.png
18b3384017b049f4ffce77193d115ef20087174ad4a3f1cb75166b2bd3dc37bb  examples/01_platformer/assets/sprites/haze_big_2_7_1.png
1f722f5d727f329ad61b6be93c60b869e1a88b6ebde63d37537b0b484b3d5bfc  examples/01_platformer/assets/sprites/haze_big_2_7_2.png
23b5627c0f03dba0f05240f3fd7606b2ea328c49870e050563cd81b1441bdb78  examples/01_platformer/assets/sprites/haze_big_2_7_3.png
2e04fac3e68ec5884fd1f6158afbf0730ed96ce2c8afa59cf4574704b9ee1657  examples/01_platformer/assets/sprites/haze_big_2_7_4.png
1d0ff9435aac5183a9e1b6905f78a19ff8bd90ca9ff2c5e934212bfdc16cc2b3  examples/01_platformer/assets/sprites/haze_big_2_7_5.png
e2dda31ab1b496a09c0e70c63c283084ea279af53019f1bb85003e6519fc4298  examples/01_platformer/assets/sprites/haze_big_2_8_0.png
0d622ee08684a6d578df630496ad06630e2d39efc01cbf732c2815c1c6e2d1b2  examples/01_platformer/assets/sprites/haze_big_2_8_1.png
f567dff85fe6dd4f1d2a26416b43d142fca1280758036c9f43eef86fa4920b4f  examples/01_platformer/assets/sprites/haze_big_2_8_2.png
2f4dc4ed779a0fed0d95f77ab0d189d6cdaf4ca8303eeda0fdc05203680ac9a5  examples/01_platformer/assets/sprites/haze_big_2_8_3.png
eaa7dc9989515538e62adb2bbe5acd28520b30f885b8e3879ed59dff0f35f733  examples/01_platformer/assets/sprites/haze_big_2_8_4.png
a29096d199919a56f816178c252e28cc8aa3e08223ea3a88e09f5198dacf0298  examples/01_platformer/assets/sprites/haze_big_2_8_5.png
273c238ab9a7af7a3150c1459e4b037083e48b2c99db4b203be3f5c154a4ae80  examples/01_platformer/assets/sprites/haze_big_2_9_0.png
baba0961e6b026283bc94214516c066a89df2acc068ae50878150f547d8328e3  examples/01_platformer/assets/sprites/haze_big_2_9_1.png
e91df0f1ed54f7c0d6f686b09443ece0ed618be62080b23b6de4cbcdb2c0a217  examples/01_platformer/assets/sprites/haze_big_2_9_2.png
23f88a9cb0042b71a1f69ab11e1a866278d1a9c6448e62e8c5e665503264958f  examples/01_platformer/assets/sprites/haze_big_2_9_3.png
43df93265f80f809f14a3e4bb595e8d13e15b7ef9e20a07d8cf176e72c927a83  examples/01_platformer/assets/sprites/haze_big_2_9_4.png
6b28f63b6cdef2d3b1b8678a9331736c57647df15819d48fbc42adde799ec508  examples/01_platformer/assets/sprites/haze_big_2_9_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_0_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_0_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_0_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_0_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_0_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_0_5.png
d8a8a0581e6a38fe1a042427883b66d1b7f17b4a780b9a39f2d43c52ba23b62a  examples/01_platformer/assets/sprites/haze_small_big_0_1_0.png
89885332a3be9f60270e6af760ada4781884a56a259d5d0e5d53ab1d4b5ae31a  examples/01_platformer/assets/sprites/haze_small_big_0_1_1.png
5b9d137e407753bb249ce71d3fccec4697e1e1ab888edf903f26bd8a68b9d249  examples/01_platformer/assets/sprites/haze_small_big_0_1_2.png
0f618e0db337fcaac07d0dbbc6ff0bb38dcc53a94af03c8363c40d1e4b8e04d1  examples/01_platformer/assets/sprites/haze_small_big_0_1_3.png
89885332a3be9f60270e6af760ada4781884a56a259d5d0e5d53ab1d4b5ae31a  examples/01_platformer/assets/sprites/haze_small_big_0_1_4.png
89885332a3be9f60270e6af760ada4781884a56a259d5d0e5d53ab1d4b5ae31a  examples/01_platformer/assets/sprites/haze_small_big_0_1_5.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_2_0.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_2_1.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_2_2.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_2_3.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_2_4.png
ee5f4ae0aae66a3d9ad8166933132bf2fad25da6220d8f02127f69546b5a4795  examples/01_platformer/assets/sprites/haze_small_big_0_2_5.png
6c61eaffd50f8d44d4aaf98ac15a5bcc617e96d15ea2b9477512c82e519d3f92  examples/01_platformer/assets/sprites/haze_small_big_1_0_0.png
c1113071b314b0c8c13a0949681e09ef2861c17dab7adba1c3bc7a9a70a19535  examples/01_platformer/assets/sprites/haze_small_big_1_0_1.png
6e396953cfc526632523bb65f46ae860ff82d5dc3a26b8efa2ae7ba6bae7a470  examples/01_platformer/assets/sprites/haze_small_big_1_0_2.png
a8942700593f8fb1e64e5c43debab9680d3c5dea3dd68f08de7f1214e68ef831  examples/01_platformer/assets/sprites/haze_small_big_1_0_3.png
7face14827e2f1b222fc7b69da192a3cdeae3a3209c74ccd9711b88cf491cad4  examples/01_platformer/assets/sprites/haze_small_big_1_0_4.png
7535fb407e927ca63fd4abae85bfa2ff50a9cded73ced8d71af8825a864bca0c  examples/01_platformer/assets/sprites/haze_small_big_1_0_5.png
94e608f0f2d1b512819639a8445a3ec98af3f279018455b04e6e5f738869c906  examples/01_platformer/assets/sprites/haze_small_big_1_1_0.png
0ce0564ec45276b4a3421ac659a84ab35319907da392350082e29354d1c17894  examples/01_platformer/assets/sprites/haze_small_big_1_1_1.png
b55ae68c965bdd4f0f94d8670f84c99b7018000b8d3cb045e2e15b52007ca72e  examples/01_platformer/assets/sprites/haze_small_big_1_1_2.png
96c2a98f4611fbfe6ee05e37daca801633d8471031916228bf0fc6c800565d99  examples/01_platformer/assets/sprites/haze_small_big_1_1_3.png
b34fef4a2e5b4247ee63a0c5042dc3f34d0d3d45b6f348c2c99c7952b39107e5  examples/01_platformer/assets/sprites/haze_small_big_1_1_4.png
d2f75604607f0478c0cc069411d4958354f432b130c0e20930b07d9775c37fa0  examples/01_platformer/assets/sprites/haze_small_big_1_1_5.png
9e785d305ad6dfe8d4d4d8216131a8d2a641a9b2496671534209b5ae841c4e69  examples/01_platformer/assets/sprites/haze_small_big_1_2_0.png
54e63fa94256994e43d17a066e243b20595f8cbd417d09dd7aec2f5c29afbaa2  examples/01_platformer/assets/sprites/haze_small_big_1_2_1.png
8846a1aabdafc0170930e95e568bf5680624b7759955e92b9f1216ee32dd9417  examples/01_platformer/assets/sprites/haze_small_big_1_2_2.png
5963b83d5fdc7f18661358e003bbee7c06a1b4bc88485101ea448df2498ed708  examples/01_platformer/assets/sprites/haze_small_big_1_2_3.png
48c4b20a1c1ff29f89ee35551e667e3f052e1a8685e061de84aea634a4b87454  examples/01_platformer/assets/sprites/haze_small_big_1_2_4.png
955d7f404b30802bae5441e664d006d5436e3e831e98ed2d007cc85c5c00598f  examples/01_platformer/assets/sprites/haze_small_big_1_2_5.png
2cd9adb3c6a529ffee50d73488519dd176f5b2e3e5c23f3d94070560aa504c0f  examples/01_platformer/assets/sprites/lava_splash_0.png
22e59e2ff66271561a88f8190d469ae8910852ad7860c5c31a143efe152288e8  examples/01_platformer/assets/sprites/lava_splash_0_e.png
ccfd13bcb72b359d0d63c14728eb34379384ddc451c8af67bdbd9fe9ffe36565  examples/01_platformer/assets/sprites/lava_splash_0_n.png
5184b486d8a82e757e09d385fcc3388c05b04e47124658f4bc9749c9198047a6  examples/01_platformer/assets/sprites/lava_splash_1.png
0a6e8c04649e875208ebd02033487e3141a4a37ae257446ce64322bb5c6fc680  examples/01_platformer/assets/sprites/lava_splash_1_e.png
b17d3c1c85fed4c74578040e9d5d12364e7257645f002084776aa28e4fad50b2  examples/01_platformer/assets/sprites/lava_splash_1_n.png
cad9939ec97f4ed41937bcb247a6aa9eb6177788a1f7fb2403d21563d0cdc2f5  examples/01_platformer/assets/sprites/lava_splash_2.png
fdb70d6513a9d26a1c20caa8b94361fb8874d417131441f810ea4276841138bd  examples/01_platformer/assets/sprites/lava_splash_2_e.png
9512319101eaffc559722744815f46c3ca462af8de2fd1f9875862d29374092d  examples/01_platformer/assets/sprites/lava_splash_2_n.png
4371432f12631a3d968d3c89b81881fee6adac4088f47c3b9895dcdde3050796  examples/01_platformer/assets/sprites/lava_splash_3.png
2915411dcc6fe8bf8a7b5c8d9a4a8387755d3c1701827ebfcd39bb53a78387bc  examples/01_platformer/assets/sprites/lava_splash_3_e.png
995baf426168abc484893b75529697dec11e69a2c7513d52294543dcbdf7a603  examples/01_platformer/assets/sprites/lava_splash_3_n.png
f69208ea1e43f34af9ea8990a61cb0bf4c6bc08a566834a1e02bde4bf0e77e8b  examples/01_platformer/assets/sprites/lavafall_0.png
d7e3e30274d9d5acfbe7e7b48c0f6013aa640ba8dd9e8933e09f7f4a31797b49  examples/01_platformer/assets/sprites/lavafall_0_e.png
746e7ed2654713452e17f5a267a166af9c4dc13f39d6ca968165b027cd7f85db  examples/01_platformer/assets/sprites/lavafall_0_n.png
a923963b2ab6d6e569eaebfedfe9aff4744b2de8ea72da9e2b32b96dbbb60371  examples/01_platformer/assets/sprites/lavafall_1.png
6da3bdf6d08f1b0bf2c65a9bc77a5af9811bdd31cf669e2fad23c6f76b72ddbd  examples/01_platformer/assets/sprites/lavafall_1_e.png
746e7ed2654713452e17f5a267a166af9c4dc13f39d6ca968165b027cd7f85db  examples/01_platformer/assets/sprites/lavafall_1_n.png
2812db1fdf872debe96cb119a084b92ca9ec492470c6b69572675c5b90ca483a  examples/01_platformer/assets/sprites/lavafall_2.png
0a8d060f010916e9193d54a991426cc796209049c82c7f5a2c4d4c7b64612f22  examples/01_platformer/assets/sprites/lavafall_2_e.png
746e7ed2654713452e17f5a267a166af9c4dc13f39d6ca968165b027cd7f85db  examples/01_platformer/assets/sprites/lavafall_2_n.png
d9163c03b4d49e2a54bb81719ca60e3bf3323a98fe789bfe5f5b4de55e0f9b23  examples/01_platformer/assets/sprites/lavafall_3.png
b89e2e1f3d836718e647c145d888ecb966225e189de029c9d3d998fcfde954eb  examples/01_platformer/assets/sprites/lavafall_3_e.png
746e7ed2654713452e17f5a267a166af9c4dc13f39d6ca968165b027cd7f85db  examples/01_platformer/assets/sprites/lavafall_3_n.png
4f61273900ee888f251e5ae2da42377e272ef619a55dc63daef24676b2efce49  examples/01_platformer/assets/sprites/obsidian_rubble.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/obsidian_rubble_e.png
46e699e689253369368e783dcc2b9a47cf50192ba325ad915b50d0ee94db7a81  examples/01_platformer/assets/sprites/obsidian_rubble_n.png
c6aca1f6b8f7f2892aadf0f154d4a6dc1cb6515aac6bd12e2fd2cfa95539f794  examples/01_platformer/assets/sprites/scree_left.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/scree_left_e.png
7158f6e50699f206b988fca7a358c377be44cff743e211045083c2de51844382  examples/01_platformer/assets/sprites/scree_left_n.png
3e27cca771e231ccd245a9a1028b653f004b04cbeea3b5cd5163e1661a3ded9f  examples/01_platformer/assets/sprites/scree_right.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/scree_right_e.png
6e079beca40e1c9840b36f5c166bd6612b508d67a725c7cea8c10866c96fab94  examples/01_platformer/assets/sprites/scree_right_n.png
acc093b1ad1c3aecc80efb23d95d113df1c1f25684a40e3767d9871d2bf27658  examples/01_platformer/assets/sprites/stalactite_0.png
b347d4d2dc1b7bb18eef7891d8be65b5c046ee7a771036041efdbbbc88c4a7d7  examples/01_platformer/assets/sprites/stalactite_0_e.png
c8cdcbb8076fc23589d123a1ae41ef1fee122ae1cf89abd060a6b4a022ed2e75  examples/01_platformer/assets/sprites/stalactite_0_n.png
2e016b60597be633ec101b05985b2d2cab09bcfcb07a6e7e3c990884b75a53f0  examples/01_platformer/assets/sprites/stalactite_1.png
a3f0e16ba675fb317d06f0abaa56d9b2bcc68be2faa083404877cc15e0b74a03  examples/01_platformer/assets/sprites/stalactite_1_e.png
84f4b53946c0afdd298a708ac28754d5c2fb9c4d1a89351188dde6e430d41632  examples/01_platformer/assets/sprites/stalactite_1_n.png
0bd56940f34c5b04b8671d0e2c1ac0ed038dc6149c64716ce399d96459234728  examples/01_platformer/assets/sprites/stalactite_2.png
3275fc1aac10ad56179a6f2333a51756a6d2a6f547c5ff3d9292de92158122ad  examples/01_platformer/assets/sprites/stalactite_2_e.png
00c2ee8c34aadf77f934839a2523060497ff8ded039203a20d6d2e82a119b8fe  examples/01_platformer/assets/sprites/stalactite_2_n.png
cf5b3b0a8a759582d4c51cc34c28b10bb743888b4674ccfece18c3745971cff9  examples/01_platformer/assets/sprites/stalactite_3.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/stalactite_3_e.png
5c2db254d0a308778ec92b811d8ce0f3fe8f8e0c8f969f8f067361ad68d1408e  examples/01_platformer/assets/sprites/stalactite_3_n.png
f775989d85f230cdf1daaaa626212328defcdabc7723fcf1860d7627a9a0bddb  examples/01_platformer/assets/sprites/stalactite_4.png
602227a0a828c11a4a30379083829756c6a3c8254e5302e95e581ca0971358f1  examples/01_platformer/assets/sprites/stalactite_4_e.png
6857e2f98cd8931998e0e8c17a1c0049deeae800055b2c76c8580a15b1bb3cb6  examples/01_platformer/assets/sprites/stalactite_4_n.png
2c869c47537188ffabe8ba1d78bc6b8e41941c5b27a940d251be336653f351cb  examples/01_platformer/assets/sprites/stalactite_5.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/stalactite_5_e.png
7dfe8a0f143b8f17670916d9e6e68689c0d1541b408e761a539a267c620e8aa4  examples/01_platformer/assets/sprites/stalactite_5_n.png
b906065bb9fe3215ba8dfe7cbf78ae0673b88ffd5ab8eab8dbf2905f343df1f2  examples/01_platformer/assets/sprites/stalactite_6.png
334a66740ff2739817852a78eb03e1cff35da608515457567c739af831d0379d  examples/01_platformer/assets/sprites/stalactite_6_e.png
00f749d0cbb0dc4187c67fab0dc84ab5bec6141c1b6f1d487b28eb259b996209  examples/01_platformer/assets/sprites/stalactite_6_n.png
ceaf314e19ad91a14ff6f50612dcaf6cf805a18815caf8619c0e68f5f579b07f  examples/01_platformer/assets/sprites/stalactite_7.png
4c782dae8effc023d85788cdf0918f1f4ef5b36901b8d4406069140c23dd013a  examples/01_platformer/assets/sprites/stalactite_7_e.png
c29987c60173f4ace69586ce1890a353bdb1945d12cdb16cad3986bfbd74dce7  examples/01_platformer/assets/sprites/stalactite_7_n.png
dcafcd09df5e08594c51f8eada7dc5aa0718977967bba6ec0a84718cd2d47648  examples/01_platformer/assets/sprites/stone_large_0.png
ee5e7260b80ed331467b94ef5c919e6fc0762a7aa86f4bde2c24fa841334a6fb  examples/01_platformer/assets/sprites/stone_large_0_n.png
1586fb35f3e0973196c43cfaa6c8f72e9f0dce5247be90cd21910ad96daf21f5  examples/01_platformer/assets/sprites/stone_large_1.png
ca5b7db88ad2a2e5edc6e0d5c956f6af98913897933e370c6f69909722c7e4e8  examples/01_platformer/assets/sprites/stone_large_1_n.png
1ff337fee03e13350789892fc42a079612a1500ba16ac3c31ec0dec6fda854a2  examples/01_platformer/assets/sprites/stone_medium_0.png
827f452d11397e12b4446ffe695ad3d7f31c2017ce518266ed2c37c77dceb3ba  examples/01_platformer/assets/sprites/stone_medium_0_n.png
c15fb934335b2533ba341f12dc186c4b0eeecfa75938d6cd5b86ec26191229be  examples/01_platformer/assets/sprites/stone_medium_1.png
e70b1980fb9192d0b43734a156258ea38ce7878b6639682c6cd04001ae83f432  examples/01_platformer/assets/sprites/stone_medium_1_n.png
bc90e7bd65ea728e98eeb91aa6e91abee32a7a4f7aeb9df4cbfcbc940c789716  examples/01_platformer/assets/sprites/stone_small_0.png
ff4b2b3331728f7046b0ed531a9318093a06b2fef80ee40775c57eaaa57a8efa  examples/01_platformer/assets/sprites/stone_small_0_n.png
5c0bcd079c324d19556f0ca0c4f80368ea4caf30a0fa3b753b2d5b159f04cfa4  examples/01_platformer/assets/sprites/stone_small_1.png
356af6138ae5ce5aa3719f1b9fa115aea585058521c09969e948187d0e55a631  examples/01_platformer/assets/sprites/stone_small_1_n.png
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
| 6 | 2026-10-27 | pass | The owner's acceptance, round 3: "Looks good accepted"; round 1's notes "The Cave pillars don't look good at all, redo them completely. Add in more varriation for the Stalactites, different shapes and sizes.", round 2's "Remove the barbbed wire sceanary and improve the look of the tiny lava pools, do a final improvement pass on the lava mist effects." `just level-check`: `OK: 6033 owned outputs match; art, level, references and budgets validated`, `Ran 34 tests … OK` (`HollowContractTests` among them; each round's passed); `cargo test -p example-01-platformer --locked`: 415 passed, 1 ignored; `just repo-check` → exit 0; §3.6's check → exit 0 with Step 5's 194 lines unchanged (exit 0 before the step's went in) and 1,096 of the step's: 1,091 sprites with their maps, 2 particle configs and 3 sounds (the 54 files it deletes take none); `git status --porcelain` → Step 6's paths only. §3.3: the stones' lowest opaque rows 31, 31 and 63, both looks; the plume's six frames 64 × 176, the sputter's four 64 × 64, both vents 64 × 32; `geyser_sputter` 0.600 s, `geyser_erupt` 0.950 s, `stone_knock` 0.260 s, their volumes Step 1's (0.45, 0.45, 0.5); the caps 16 and 24. The ambient: each round's sheet showed the cave at 0.30 beside 0.22; the owner accepted with no change asked, so `AMBIENT_INDOOR` stays (0.22, 0.205, 0.235) and Step 7 takes no constant. Landed: the mouth, a jamb and lintel of rough-dressed wet stone at the wall's face over the hollow's back in shadow, roots and moss from the ravine's side, re-cut to rows 57–62 (8 × 6); five of its 14 props kept, scree, obsidian and a fallen slab of the hollow's own; the basalt's sixteen variants (flow lines, black glass, an ember crack, a nodule, gas holes, ash), the back's receding columns with an ember seam in one variant; two deep recesses, ridges against a far glow of embers; the pillars drawn whole; stalactites in eight shapes (19 on the roof); fire crystals and small lava pools that glow, the pools animated; the lavafall 23 px wide with crust plates and a splash at its foot; the heat over the lava animated wisps; the geyser's vent, choked vent, sputter and eruption; the stones, a column piece and a slab of flow crust; rubble heaps on each floor; the geyser's embers and burst; the three sounds. Deviations: new pieces (Q18), `level.tmj`'s tile IDs moved: `lava_splash_<f>`, `basalt_rubble_<k>`, `obsidian_rubble`, `scree_left`, `scree_right`, `fallen_slab`, `stalactite_3`–`_7`, `cave_deep_<k>`, `cave_pillar_<k>`, `cave_wall_0`, and the pools and the heat as clips (`ember_pool_<i>_<f>`, `haze*_big_<r>_<c>_<f>` in `scenery.RISING`); `placement.py` maps the new structures; `pixels.fbm` the one new helper; `level.py`'s pillar columns are covered by whole pillars placed as decorations (its file is outside the step), so `cave_back_pillar_<n>`, redrawn as the shaft, shows nowhere; the mouth keeps five props, not about six, after the ribcage that read as barbed wire (`bones_1`) left the hollow at the owner's note; props moved out of the geysers' plumes and off the stones' heaps; the placeholders' `scenery.darkened` and `sfx.recut` removed with them. Findings: a goblin on an erupting vent hops about 13 px where the player is thrown about two tiles (the geyser's knockback, Step 2's code); the force push slides the stones about a fifth of a tile a cast (Step 3's finding), so the push loop casts three times | `tools/{actors,backdrops,generate,pixels,placement,scenery,sfx,terrain}.py`, `tools/level.json`, `tools/README.md`, the generated `assets/`, this plan's §3.6, log and follow-ups; the review in `perf-runs/20261027-e01hollow-cave-review/` (`sheets/`, `stills/`, `pairs/`, `seq/`, `loops/`, `sounds/`, `review.html`, `tools/`) |

## Follow-ups

- A goblin on an erupting geyser hops about 13 px while the player is thrown about two tiles (the touch's knockback against its mass, Step 2's code); a Later line or a known issue if play asks for the goblin thrown (Step 6's review, `goblin` loop).
- `bones_1`, the ribcage the owner read as barbed wire, still stands twice on the troll bridge (385.6, 401); Step 6 took it out of the hollow only.
