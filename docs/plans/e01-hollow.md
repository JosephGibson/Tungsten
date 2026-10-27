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

## Follow-ups

None yet.
