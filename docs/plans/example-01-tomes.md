# Example 01: the tomes (`e01tomes`) — draft

- **status:** in progress
- **goal:** Example 01 (l) ([feedback](1.0/example-01-feedback.md) §3.8, R1–R8): three books hidden off the routes, each taken with an `interact` press after a panel states its gift and its cost. The demonic pact halves the maximum health for a 50-point shield that recharges 3 s after a hit, drawn on the health bar; the tome of bounding lets every air jump past the double jump cost 25 mana, with no cap; the tome of arcane power doubles every spell's cost and every hit a spell makes. Two opt-in kit additions carry them (K25 a paid air jump, K26 an ability cost scaled by a stat), a HUD strip shows the tomes held and save schema 16 keeps them. The art and sound come first, from the redo's first [§6 asset session](roadmap-redo.md#6-asset-sessions).
- **non-goals:** K27 (a shield's share on `Damaged`, no flash or window on a hit absorbed whole): shield hits read as hits (§5 Q6). A kit interact or prompt item: game Rust, as the [paper designs](paper-designs.md)' LW3 rates it. K25's free count set at run time and an unlimited free count, and a scene field for the paid jump: none has a consumer (§2.1). The hollow's and the aqueduct's look round the spots, which (o) redraws and keeps the books in. Any change to the falls, the lava, the stock player's jump or an existing route's jump. New spells, damage types or enemies. A release, a changelog section or a decision entry: a release is cut separately (`D-215`), and nothing here constrains later work beyond its code. The template.
- **files to touch:** paths under `examples/01_platformer/` (01's) unless rooted.
  - Step 1, the asset session: 01's `tools/art.py`, `tools/effects.py`, `tools/generate.py` (particle configs and clips), `tools/sfx.py`, `tools/sounds.json`, `tools/palette.json` (only for a new glow colour, §3.2), `tools/README.md`; the generated `assets/` (sprites, animations, particles, sounds, `manifest.json`, `tilemaps/level.tmj`) and `tools/outputs.json`; this plan's §3.3 final column, §3.5 lock and evidence row.
  - Step 2, the kit: `crates/tungsten-kit/src/motion/platformer.rs`, `crates/tungsten-kit/src/tests/platformer.rs`, `crates/tungsten-kit/src/intent/ability.rs`, `crates/tungsten-kit/src/tests/ability.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `DESIGN.md`.
  - Step 3, the books: 01's `tools/level.json`, `tools/level.py`, `tools/placement.py`, `tools/content/prefabs/tome_{pact,bounding,arcane}.json`, `tools/content/prefabs/tome_motes_{pact,bounding,arcane}.json`, `tools/content/data/cue_rules/sounds.json`, `input.json`, `src/game.rs`, `src/features/mod.rs`, `src/features/tomes/{mod,components,systems}.rs` (new), `src/features/audio/{components,mod}.rs`, `src/features/spells/hud.rs`, `src/save.rs`, `tests/game/{main,tomes,level,saves}.rs` (`tomes.rs` new), `README.md`; the generated `assets/`.
  - Step 4, the pact: 01's `src/features/tomes/`, `src/features/player/systems.rs`, `src/features/spells/hud.rs`, `src/save.rs`, `tests/game/{tomes,saves,hud}.rs`, `README.md`.
  - Step 5, bounding: 01's `src/features/tomes/`, `src/features/player/systems.rs`, `src/features/audio/{components,mod}.rs`, `tools/content/data/cue_rules/sounds.json`, `tests/game/{tomes,player}.rs`, `README.md`; the generated `assets/`.
  - Step 6, arcane power: 01's `tools/content/data/abilities/{fire_missile,flamethrower,ice_spray,lightning,force_push,black_hole,iron_block,bowling_ball,small_balls,summon}.json`, `tools/content/data/stats/game.json`, `src/features/tomes/`, `src/features/combat/mod.rs`, `src/features/toys/systems.rs`, `src/features/lightning/mod.rs`, `src/features/spells/{mod,systems,hud}.rs`, `tests/game/tomes.rs`, `README.md`; the generated `assets/`.
  - Step 7, close: 01's `README.md`, `DESIGN.md`, `docs/plans/1.0/example-01-feedback.md` (§3.8's landed note), `docs/known-issues.md` (only for a finding), `ROADMAP.md`, this plan (deleted).
- **ordered steps:** (1) Art and sound, the asset session, ending in the owner's acceptance and the lock. (2) The kit: a paid air jump and a cost scale. (3) The books on the map, the prompt, the take, the strip and schema 16. (4) The pact and the shield on the bar. (5) Bounding. (6) Arcane power. (7) Close. §4 holds each.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes, 01's whole suite among its tests, `just smoke` runs 01, and in-game captures show the books in place and taken, the prompt, the shield on the bar, the strip and the bound; the plan is deleted with its citations and `e01tomes` is out of Now.

Item `e01tomes` in [ROADMAP.md](../../ROADMAP.md), from the [feedback](1.0/example-01-feedback.md)'s §3.8 with R1–R8, Q34–Q41 and K25–K27. Written 2026-10-26 at `2e4ecb2` on the `0.109` branch, after `e01quick2`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs: Step 1 is an asset session on Opus 5.5 at max on §3 alone, which ends when the owner accepts its art and sound, and Steps 2–7 are one code session at xhigh that builds against §3's contract and leaves `tools/` art alone.

## Context digest

- The owner asked after 0.104 for three pickups hidden on the level, something for players to find and a demo of the kit's shield, stat modifiers and abilities (feedback §1, "The tomes"). §3.8 sketched the stop; Q34–Q41 hold its defaults, K25 and K26 its two kit gaps, K27 a third left in the backlog.
- It stands on the kit's `Shield`, `Stats` modifiers by source and ability costs, the controller's air jumps and its shove (`D-213`), the kit's oscillator, core's binding-aware prompts and the shell's `prompt_row`, 01's `combat::Hit` (the crit's ×2 lives in its factor), its HUD meters and icon family, and save schema 15. E and the pad's East are free since `e01quick2` moved the summons to the arrows (Q44).
- `D-215`: H1's rules (a commit per step, one critique round, the asset loop, the tiers); kit additions are admitted by 01's use, its amendment of `D-172`; level C and its API sketch are retired, so Q41's re-rating falls away and the critique reads §2.2 instead. `D-151` as amended puts art and sound on Opus 5.5 at max and code at xhigh; under `D-150` nothing stops for approval, but §6's review is the owner's by design.
- It is the first example stop under H1 that draws new art, so §6's first trial on today's generator: §3 is the brief, Step 1 generates the files and the review sheet, the owner reviews in rounds, and the accepted files' hashes go in §3.5, which the run checks after every regeneration (the generator's own locks are redo step 7.1's).
- Later stops expect: (o) keeps the books where they lie, which Step 3's double-jump test holds; (p)'s flask reaches arcane power through the cost scale and `Hit::spell`; (m) migrates schema 16.

## 1. Audit

Read at `2e4ecb2`. 01's paths are under `examples/01_platformer/`; `features/` is its `src/features/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The controller's air jumps are a free count its builder fixes; a press past coyote time jumps while one is left, and grounding refills them; `air_jumped()` reports the step's. Nothing in it reads a stat (K25). Its step reads `&World`; the runner writes the controller, its velocity, a climb's position and a shove back afterwards. | `crates/tungsten-kit/src/motion/platformer.rs:368-372`, `:556`, `:592`, `:606-627`, `:668`, `:1199-1248` |
| A2 | `ControllerSave` (deny unknown fields) keeps the settings and the runtime state, the shove and the step height left out when off, so a 0.104 save still loads; `air_jumps_left` may not pass `air_jumps`. | `platformer.rs:1006-1035`, `:1037-1136`; 01's `tests/game/saves.rs:3226-3259` |
| A3 | An ability's cost is `(stat, amount)`: a use refuses below the amount and spends it from the stat's base; a use reads no stat but `cooldown_rate`, in the tick (K26). `cost()` returns the data's amount; the data form denies unknown fields. | `crates/tungsten-kit/src/intent/ability.rs:217`, `:279-290`, `:392-396`, `:569-600`, `:1379-1386`, `:1396-1401`, `:1519-1522` |
| A4 | `ability_cost` is a described shape (`stat`, `amount`); the description counts are 45 components, 9 kinds and 40 shapes, and `numeric_fields_take_their_extremes_and_refuse_beyond` counts numeric fields only, so a text field moves neither. | `ability.rs:1738-1752`; `api/tungsten-kit-components.txt:310-313`; `crates/tungsten-kit/src/tests/describe.rs:125-127` |
| A5 | The kit hash is pinned at `0x3334_9665_de0c_501a`; no recipe sets an air-jump cost or a cost scale. | `crates/tungsten/src/tests/kit_recipes/determinism.rs:36`, `:541-550` |
| A6 | `Shield` takes a hit after resistances and before health, recharges by `with_recharge(per_second, delay)` from `delay` after its last hit, which `take` restarts, is never filled by a heal and saves through `ShieldFields`. 01 uses none. | `crates/tungsten-kit/src/damage/defense.rs:417-464`, `:467-526`, `:558-567`, `:598-619` |
| A7 | The pipeline takes the landed total off the shield, then health; every non-periodic hit flashes; each `DamagedPart` carries its whole landed part, so ailment buckets fill through a shield, and `Damaged.amount` the whole total; `Health::hurt` opens the window on any non-periodic hit, one of 0 included (K27). 01's `player::hurt` sounds and tints on every non-periodic `Damaged`. | `crates/tungsten-kit/src/damage/pipeline.rs:891-929`; `crates/tungsten-kit/src/damage/health.rs:278-288`; `features/player/systems.rs:304-330` |
| A8 | `stats_apply` writes `max_health` into `Health`, which keeps its share, in the Tick phase, which runs before Intent, so a modifier added in Intent reaches `Health` a step later. `Stats::add_modifier` recomputes the stat at once, goes by a source string as `remove_modifiers` does, and gives a stat the entity lacks nothing. | `crates/tungsten-kit/src/foundation/stats.rs:386-413`, `:749-751`, `:782-784`; `crates/tungsten-kit/src/foundation/phases.rs:47-58`; `health.rs:305-311` |
| A9 | The player: `air_jumps` 1, `jump_speed` 1,280, health 100 at every difficulty, stats `max_health`, `mana` and `max_mana` 100, twelve abilities, an `action_input` slot for `jump` only. Gravity 3,600: one jump peaks at 227.6 px, two at about 455. Mana regenerates 3 a 0.1 s, held while `casting`. | `tools/content/prefabs/player.json`; `features/level/mod.rs:52`; `tools/content/data/effects/mana_regen.json` |
| A10 | The kill row hits for `health.current()`, periodic, and so does leaving the world box, whose top is 8 tiles over the map. | `features/player/systems.rs:205-240`; `features/level/mod.rs:58-64` |
| A11 | A jump puffs, or in the air bursts `ex10_double_jump` and plays `Sound::AirJumped`; a test pins one air-jump sound and a silent ground jump. | `features/player/systems.rs:403-409`; `tests/game/player.rs:529-566` |
| A12 | The ten slots' costs in mana: missile 4, flamethrower 0.1667 a step, ice spray 0.15 a step, lightning 15, force push 12.5, black hole 15, iron 7.5, bowling ball 6, small balls 0.125 a step, summon 10; the staff light 0.05 a step through `use_ability`; the gun none. 01 reads a cost in `spells::cost`, which one caller uses, and in the HUD's payable check. | `tools/content/data/abilities/*.json`; `features/spells/mod.rs:23-34`, `:174-181`; `features/spells/systems.rs:341`; `features/spells/hud.rs:589-605`; `features/player/systems.rs:249-296` |
| A13 | `Hit::request` builds the kit's request, the crit's ×2 in its factor. A spell's own hits: the missile's blast (`burn_targets`, from `explode` at its contact or its end), the sprays' pulses, the strike and the chain's jumps, whose strengths are `STRIKE_SHOCK` times `JUMP_KEEP` a jump, never the damage dealt. The other `Hit::new`s are props, cascades, enemies and the gun. | `features/combat/mod.rs:101-205`; `features/toys/systems.rs:551-592`, `:641-642`, `:706-725`, `:1161-1176`; `features/lightning/mod.rs:39-46`, `:225-230`, `:266`, `:344-348`, `:371` |
| A14 | The HUD: `Meter` (a track, a well, a fill clipping the liquid's strip, a pulse, an edge, a readout), `Art`, `Hud`; liquids are 128 × 10 strips scrolled in the fill; the meters root is a column of the health and mana rows; `size` sets every art-pixel size from the UI scale; `fill` writes "current / max". | `features/spells/hud.rs:44-55`, `:91-127`, `:131-161`, `:211-232`, `:312-431`, `:664-720`, `:821-856` |
| A15 | The icon family: 36 px tiles of 32 px subjects in an element's frame and backing, named and sized in `art.py`, beside the liquids. Emissive pixels come only from the palette's `y Y C c`, matched by colour value. | `tools/effects.py:186-275`; `tools/art.py:16-19`, `:57-58`, `:136-173` |
| A16 | Placed orbs: the `pickups` layer, the map's last, each over a named support by `x` and `float`, sized by `images['health_orb']`. A map object takes a save ID only when a save names it, and `gone` takes those off at a load. | `tools/level.py:308-311`; `tools/placement.py:85-95`; `features/level/mod.rs:194-201`, `:249-262`; `src/save.rs:107-110`, `:1571-1575` |
| A17 | Any 92 × 52 window holds at most 10 lights (glowing props, fires, lit lava, wisps): the worst holds 5 today and 6 with a light at each of §2.3's three spots (measured from `tools/level.json`). The tests' sky, columns 0–37 by rows 0–19, holds nothing. | `tools/level.py:52-57`, `:646-682` |
| A18 | The route test lands each route pair's `LAUNCHES` row and fails on a row no route takes. Its launch presses jump once and holds it, so it never makes an air jump, and no route needs one. `reach()` and the elevator test model the stock jump. | `tests/game/level.rs:93-172`, `:222-281`, `:286-308`, `:310-364`, `:508-533` |
| A19 | The hollow (211–280): its roof rows 40–52, its floor row 63, lava at row 64 in 230–234 and in 236–240 (lit) round the stepping stone 234–236. Both main-route hops over it peak with the head 0.57 rows under row 58. | `tools/level.json` (`hollow_roof`, `hollow_floor_1`, `stepping_stone`, `lava`); `tests/game/level.rs:112-113` |
| A20 | The ravine: the falls' two columns at x 177.61 and 178.61 over rows 28–61; the zigzag's ledges between 181 and 192, `rv_ledge_2` 187–191 at rows 36–37 and `rv_ledge_4` 188–192 at row 44 among them; open shaft from 192 to the east climb at 197–203, its lift below row 55. | `tools/level.json` (`falls_*`, `rv_ledge_*`, `ravine_east_*`, `moving_platforms`); `tests/game/level.rs:157-164` |
| A21 | The aqueduct: its floor at row 40 (95–158) under `channel_east_a` (133–145, row 28), piers every four columns from 88, a pier tile and a three-tile opening, a bat under the channel at x 141 and a crawler on the floor at 138. | `tools/level.json`; `tools/backdrops.py:302-312` |
| A22 | No `interact` in `input.json`; E and the pad's East bind nothing. `BINDABLE_ACTIONS` must list `input.json`'s actions exactly. | `input.json`; `src/game.rs:33-58`; `tests/game/seam.rs:55-74` |
| A23 | A prompt is core's `UiTree::set_prompt`, which the shell's `widgets::prompt_row` wraps (the shell copies the template byte for byte); a world point maps to the screen through `CameraState::world_to_screen`; a gameplay root covered by a pause hides with `RootCover::Hide`. | `crates/tungsten-core/src/ui/prompt.rs:150-170`; 01's `src/shell/widgets.rs:302-308`; `crates/tungsten-core/src/camera.rs:258-262`; `crates/tungsten/src/state.rs:79-86` |
| A24 | A sound takes a `Sound` variant, its place in `Sound::ALL` (69 now) and its event name, an `occurrence::<N>` in `cue_of`, a rule in `cue_rules/sounds.json` and its file through `tools/sounds.json` and `sfx.py`; `data.rs` checks a rule and a manifest sound for every variant. | `features/audio/components.rs:50`, `:191`, `:265`; `features/audio/mod.rs:82-155`; `tests/game/data.rs:190-213` |
| A25 | Saves: schema 15; `migrate`'s arms 7–14 end in `to_15`; `PlayerSave` denies unknown fields; `capture_player`; `restore`, which puts the shove back with the rest of the controller kept; every component on a saved entity sits in the coverage test's lists. | `src/save.rs:237-313`, `:801-822`, `:1515-1537`, `:1566-1591`; `features/player/mod.rs:183-189`; `tests/game/saves.rs:296` |
| A26 | The kit's oscillator bobs a bodiless pickup in data, giving it a `Position`; a glow whose `light` is above 0 casts a point light, which no crawler fears (they read the staff's alone). | `crates/tungsten-kit/src/motion/oscillator.rs:36-80`; `features/level/components.rs:85-133`; `features/enemies/crawler.rs:62-72` |
| A27 | Bursts: `shared::spawn_burst`, and `spawn_key_burst`, never dropped at the cap; a clip plays at a point as `toys::wave`'s parts do. Particle configs come from `generate.py`'s `particle_configs`, sounds from `sfx.py`'s `build_sounds`; the generator owns `assets/` byte for byte and runs here only in a scratch Pillow venv. | `src/shared.rs:100-128`; `features/toys/wave.rs:98-111`; `tools/generate.py:60-536`, `:743-770`; `tools/sfx.py:2251` |
| A28 | `restore` puts a saved body's point back as it was, and nothing pushes a kinematic body out of an overlap, so a body saved where a new solid now stands loads inside it. `to_15` moved the bodies in the start's new wall out of it from frozen boxes through `bodies()`, dropping one with no room. | `src/save.rs:181-215`, `:510-532`, `:615-716`, `:1577`, `:1942-1953`; `crates/tungsten-kit/src/motion/platformer.rs:110-113` |
| A29 | The generator refuses a stream whose segments run through drawn terrain, so no support may stand in the falls' columns or a pour's. Spouts pour at x 101, 137 and 153 under the channel and a statue stands at 154; moving platforms sweep the aqueduct's bays (x 132.6–141.4 at row 35.5, 145.7–148.3 over rows 34–39) and the ravine (193.2–195.8 from row 55.5 down); a level test checks every sweep's terrain and headroom. | `tools/placement.py:352-373`; `tools/level.json` (`spouts`, `moving_platforms`, props); `tests/game/level.rs:407` |

## 2. Design

### 2.1 Scope

What R1–R8 ask, what §3.8 added, and what this plan takes:

| Item | R1–R8 | §3.8 added | Smallest version | Taken |
| --- | --- | --- | --- | --- |
| K25 | A paid jump past the double jump (R7) | A free count set at run time and an unlimited form | A paid tier past the free count with no cap, set at run time, saved, reported | The smallest: the paid tier is R7's "unlimited"; a run-time free count and an unlimited free count have no consumer, so they wait (Follow-ups) |
| K26 | Spells twice as dear (R8) | — | A cost scaled by a stat its data names | As is |
| Books | A glowing sprite and a pickup sound each (R2) | A slow bob, motes, a halo and a take burst | One lit frame each, the prefab's glow, the sound | §3.8's list: the bob is the kit's oscillator in data, the halo the prefab's glow, and the clips and motes are the asset session's, so they cost the code nothing. If Step 1 runs long, its cut line is one shared take clip and no motes |
| Prompt | A message and the key (R3) | A panel with gift and cost | One label | The panel: the cost is what makes taking a choice (Q36) |
| Pact | Shield and half health (R5), a light-blue layer (R6) | The kill row through it, a "+50" readout | The layer alone | All: the kill row is a bug otherwise, the readout one label |
| Bounding | Paid jumps, a sound, blue particles (R7) | Their burst and trail | A tinted burst | §3's ring and bursts; plus the world box's top left out for the player, since unlimited jumps could carry it past (§2.5) |
| Arcane | Twice cost, damage, build-up (R8) | — | — | As is; the build-up follows the damage (Q39) |
| Spots | Three hidden places (R1) | A route test for each | — | Three ledges and a double-jump launch in the route test, which today never makes an air jump (A18) |

Named and left out: K27 (§5 Q6), a kit interact item (LW3), a scene field for the paid jump, the spots' surroundings ((o)'s), a decision entry.

### 2.2 The kit (K25, K26)

Both off by default, so other games and the kit hash stay as they are (A5).

**A paid air jump** on `PlatformerController`, past its free ones:

- `with_air_jump_cost(stat, amount)` and `set_air_jump_cost(Option<(&str, f32)>)`, which a pickup calls at run time; both panic on a stat that is not `[a-z][a-z0-9_]*` or an amount not finite and at least 0, as the other builders do. `air_jump_cost()` reads it.
- In the step's input branch, after the free air jump: a press in the air, fresh or buffered, with no free jump left, the input unlocked and a cost set jumps at `jump_speed` when the entity's `Stats` base of the stat holds the amount, with no cap. The runner takes the amount off the base after the step, as it writes the shove (A1). Below it the press is refused: no jump, the jump buffer keeping it for a landing, or for a paid jump once the stat holds the cost.
- Reports: `paid_air_jumped()` on every step whose air jump was paid, a buffered press's included (`air_jumped()` stays true for it), and `air_jump_refused()` only on the step of a fresh press the cost refuses, so a held refusal reports once. A press refused at its fresh step that pays inside its buffer reports the paid jump on that step.
- Grounding refills the free count alone; the paid tier has none.
- The saved form keeps the cost and both flags, each left out when none or false, so a 0.108 save loads with none (A2).
- No scene field, no inspector row.

**A cost scale** on `Ability`:

- The data's cost gains `scale`, a stat name: `{ "stat": "mana", "amount": 4, "scale": "spell_cost" }`, written only when set; `with_cost_scale(stat)`, which panics on a bad name or an ability without a cost, and `cost_scale()`.
- `cost_for(&self, stats: Option<&Stats>) -> Option<(&str, f32)>`: the amount times the user's value of the scale stat, at least 0, or the amount when the user lacks it or there is no scale. `use_ability` checks and spends `cost_for`. `cost()` keeps returning the data's amount.
- The `ability_cost` shape gains the `scale` field (text, default null), which moves `api/tungsten-kit-components.txt` by a line and no count (A4).

Tests: a controller with one free air jump and a 25-mana cost, from 100 mana, makes its free air jump and then four paid ones of 25 each, is refused at 24 with one report a press, and reports a paid jump apart from the free one; a press refused at 24 whose stat reaches 25 inside the jump buffer jumps then and reports it paid; the cost and the flags round-trip through a save, and a save without them reads with none. An ability scaled by `spell_cost` 2 spends twice and is refused at 1.9 times its amount; a user without the stat pays the amount; the data form round-trips with and without `scale` and refuses a bad name. `DESIGN.md`'s controller paragraph (`:491`) and the abilities section (`:591-599`) gain a sentence each, and the workspace line (`:5`) a clause.

### 2.3 The books, the prompt and the take (R1–R3)

**Types** (`features/tomes/`): `TomeKind { Pact, Bounding, Arcane }` (serde `pact`, `bounding`, `arcane`); `Tome { kind }` on a map book, the scene component `tome` its plugin registers; `Tomes(Vec<TomeKind>)` on the player from its first take, in the order taken. A book takes no save ID (A16): the map spawns all three and a load takes off those the player holds, so no save ID moves and `gone` is untouched. Modifier sources `tome.pact` and `tome.arcane`.

**Prefabs** `tome_<kind>`: the idle clip's first frame at z 12, the `tome`, the idle `animation`, an `oscillator` position channel for the bob, the `glow` and a `spawn_children` emitter `tome_motes_<kind>`, with §3.3's values.

**The map**: `tools/level.json` gains a `tomes` list, `{ name, kind, support, x, float, from }`, `from` the route platform its launch starts on; `level.py` writes a `tomes` object layer after `pickups`, each object of class `tome_<kind>`, and counts each book's glow in the light budget; `placement.py` places a book as it places an orb, refusing one off its support, sized by its idle frame. Each support stays out of every stream's columns and every moving platform's sweep (A29), which the generator's check and the level tests hold.

**The spots** (Q34; the run may move one up to 2 tiles within its area when a check fails, recording the cells):

| Tome | Area | New support | Book | Reached | Why hidden |
| --- | --- | --- | --- | --- | --- |
| Pact | The ember hollow, over the dark pool | `pact_shelf`, basalt, 231–234, row 57, one row deep | x 232.5, float 0.6 | From `hollow_floor_1` by the double jump after a run, 6 rows up | High in the cave's unlit half under the roof, lit by its own glow; its underside clears both hops' heads by 36 px (A19) |
| Bounding | The waterfall ravine, east of the zigzag | `bound_shelf`, wet, 194–196, row 40, one row deep | x 195.0, float 0.6 | From `rv_ledge_4` by the double jump, 4 rows up and east | In the shaft's dark between the ledges and the east climb, off every way, clear of the falls, the lift and every route's arc (A20, A29) |
| Arcane | The broken aqueduct, in a bay of the arcade | `arcane_ledge`, `slab_carved`, 117–120, row 34 | x 118.5, float 0.6 | From `aqueduct_floor` by the double jump, 6 rows up, from either side past the rubble | In the bay between the piers at 116 and 120 under `channel_mid`, off both ways, with no pour, prop or mover in it (A21, A29) |

Q34 put bounding behind the falls: a shelf there runs the falls through drawn terrain, which the generator refuses (A29), so it stands east of the zigzag. A support must also leave room to rise beside it, since the test's launch steers toward its goal from the first step: each spot's `from` gives the double jump the run-up its rise needs, about 4.5 tiles for 6 rows.

**The route test**: `launch_lands` gains an air flag that releases jump at the rise's top and presses it again the next step, so the stock controller makes its air jump; a test lands each book's `TOME_LAUNCHES` row (`name, start, goal`) from its `from` onto its support, fails on a row no book takes, and proposes a row through the search with the flag. The existing rows keep their single jump, and every one must still land with the shelves in place.

**The prompt**: each frame, after the camera's update, the nearest book whose centre lies within 64 px of the live player's body centre, while gameplay is on top, shows a panel over it, placed by `world_to_screen` and the UI scale and kept on screen: the book's name, what it gives, what it costs and `{action:interact} Take` through `widgets::prompt_row`; otherwise the panel hides. Its root is owned by gameplay with `RootCover::Hide`. A loop over the three books replaces LW3's `physics::nearest`, which would need sensor bodies.

| Tome | Name | Gives | Costs |
| --- | --- | --- | --- |
| Pact | The Demonic Pact | A shield of 50 over your health, back 3 s after a hit | Half your maximum health, for good |
| Bounding | The Tome of Bounding | Jump again and again in the air | 25 mana for each jump past the second |
| Arcane | The Tome of Arcane Power | Spells deal twice the damage and build twice the ailments | Spells cost twice the mana |

**`interact`**: E and the pad's East in `input.json`, and `("interact", "Interact")` in `BINDABLE_ACTIONS` after `reload` (Q35).

**The take**, in the tick phase before the kit's `STATS_APPLY` when `interact` was just pressed with a book in reach, so the pact's health lands in the step its shield does (A8): its kind joins the player's `Tomes`, its change applies (Steps 4–6; Step 3's take changes nothing else), its take clip and key burst play at its centre with its pickup sound, and the book goes with its children. A dead player takes nothing.

### 2.4 The pact (R5, R6)

- The take adds `Mul(0.5)` to `max_health` from `tome.pact`, so 100 / 100 becomes 50 / 50 and 80 / 100 becomes 40 / 50 (A8), and inserts `Shield::new(50.0).with_recharge(20.0, 3.0)`, full: after a hit it waits 3 s and is full 2.5 s later (Q37). Both land in the take's step, since the take runs before `stats_apply` (§2.3). Orbs heal health only (A6).
- The kill row and the world box hit for health plus the shield (A10).
- Shield hits read as hits: flash, window, hurt sound and tint (A7, K27); ailments build through the shield, which takes damage, not status (§5 Q6).
- The bar: a shield layer in the health well over its fill, from the left, as wide as the shield's share of the maximum capped at the bar, clipping three tiles of `ex10_hud_liquid_shield` scrolled at its own pace, with its own edge; a second readout after "50 / 50", "+50" in the shield's blue, the shield rounded up, hidden while it holds none (Q38). Low health still pulses by health alone.

### 2.5 Bounding (R7)

- The take calls `set_air_jump_cost(Some(("mana", 25.0)))`: the ground jump and the first air jump stay free, and every later one costs 25, refused below it, with no cap; regeneration runs on (Q40: it stays 25 under arcane).
- A paid jump plays §3's ring and its two bursts at the feet and `Sound::Bounded`, in place of the double jump's burst and `AirJumped`; a refused press plays `Sound::NoMana` once.
- The player's world-box check leaves out the box's top. A full bar buys six paid jumps after the two free ones, regeneration included, about 28 rows of rise: from the start's wall (row 21) that brings the feet within a tile of the box's top, 8 tiles over the map, and from the east wall's end (row 10) well past it, where the check would kill a wizard that gravity always brings back. The view stays clamped to the map, so such a bound leaves it until it falls (§5 Q9).
- Bounding reaches what the elevator gates, the mine's upper level among it; the reach tests pin the stock player (§5 Q10).

### 2.6 Arcane power (R8)

- The take sets the bases of `spell_cost` and `spell_power` to 1 where the player lacks them, then adds `Mul(2.0)` to each from `tome.arcane`. `data/stats/game.json` declares both with `min: 0`.
- The ten slots' abilities (A12) take `"scale": "spell_cost"`; the staff light's and the gun's do not.
- `combat::Hit::spell()` marks a spell's own hit; `request` multiplies a marked hit's factor by its source's `spell_power` value, 1 without it, beside the crit's ×2. Marked: `burn_targets`, the sprays' pulses, the strike and the chain's jumps (A13); the chain's strengths come from constants, so nothing compounds. The build-up follows the damage (Q39). Bounding's jumps, thrown balls, bowling balls, iron, summons, kegs, cascades and the gun stay as they are.
- `spells::cost` takes the player and reads `cost_for`, as the HUD's payable check does.

### 2.7 The strip (R4)

A third row in the meters column, under mana: three 36 px image slots in the icon family (`ex10_icon_tome_<kind>`), filled in the order taken, sized by `size` like the spell tiles, the row hidden until the first take.

### 2.8 Saves: schema 16

`PlayerSave` gains `tomes` and `shield`, each left out when empty or none. `to_16` sets each saved body whose box overlaps one of the three new supports (their cells, frozen as `S16_SUPPORTS` as `to_15` froze its wall) on that support's top, its x kept on it and its velocity zeroed, through `bodies()`, and drops one with no room there as `to_15` does (A28). `15 => to_16`, and every older arm ends in `to_16` after `to_15`. The paid tier rides in the controller's saved form and the modifiers in `Stats`, both saved whole (A2, A25). `restore` puts back `Tomes` and the shield and takes off the books the player holds. Tests: a run holding two tomes with the shield half full saves through JSON and loads back the same, with those two books gone and the third on the map; a schema 15 save from the 0.108 code (a current capture without the new fields, numbered 15) loads with no tome, no shield and every book on the map; and a schema 15 save with the player inside `pact_shelf`, a ball inside `bound_shelf` and an orb inside `arcane_ledge` loads with each on its support's top, clear of it. The coverage test lists `Tomes` and `Shield` as saved.

### 2.9 Sounds and cues

`Sound::TomePactTaken`, `TomeBoundingTaken`, `TomeArcaneTaken` (Step 3) and `Bounded` (Step 5), appended to `Sound::ALL` with their events (`tome_pact_taken`, `tome_bounding_taken`, `tome_arcane_taken`, `bounded`), `occurrence::<69>` to `::<72>` in `cue_of`, and a rule each in `cue_rules/sounds.json` naming §3's sound IDs (A24).

## 3. Asset brief

The redo's [§6](roadmap-redo.md#6-asset-sessions) brief for this stop's families. Step 1 generates every file below through the generator, with its own toolkit; the code builds against this contract (IDs, sizes, anchors, frames, clips) and never edits `tools/` art.

### 3.1 Families and contract

Generator names; the manifest prefixes `ex10_`. `<kind>` is `pact`, `bounding` or `arcane`.

| Family | Files | Size, frames, timing | Anchor | The code's use |
| --- | --- | --- | --- | --- |
| Books | `tome_<kind>_idle_0` to `_3`, clip `tome_<kind>_idle` | 32 × 32 px, lit (normal and emissive maps), 4 frames at 0.18 s, looping | The book's centre at (16, 16); the sprite's top-left at the map object's point | Prefab `tome_<kind>`; floats 0.6 tiles over its support |
| Motes | Particle config `tome_<kind>_motes` | Continuous, at most 8 alive | Emitted at the book's centre | Prefab `tome_motes_<kind>`, the book's child |
| Take | `tome_<kind>_take_0` to `_5`, clip `tome_<kind>_take`; burst `tome_<kind>_take_sparks` | 64 × 64 px, unlit, 6 frames at 0.08 s, once; the burst at most 24 alive | The clip's centre (32, 32) on the book's centre | At the take, the book gone that step |
| Bound | `bound_ring_0` to `_5`, clip `bound_ring`; bursts `bound_sparks`, `bound_trail` | 64 × 32 px, unlit, 6 frames at 0.05 s, once; bursts at most 20 and 12 alive | The ring's centre (32, 16) on the feet | On a paid jump, in place of `double_jump` |
| Icons | `icon_tome_<kind>` | 36 × 36 px in the icon family | The HUD tile | The strip, in the order taken |
| Shield liquid | `hud_liquid_shield` | 128 × 10 px, tiling left to right as `hud_liquid_health` does | The health well | The shield layer, scrolled |
| Sounds | `ex10_tome_<kind>_sfx` (`sounds/tome_<kind>.wav`), `ex10_bound_sfx` (`sounds/bound.wav`) | Each pickup 0.6–1.2 s, the bound at most 0.35 s, at volumes of 0.4–0.7 | — | `Sound::Tome<Kind>Taken`, `Sound::Bounded` |

### 3.2 Art direction

From the owner's reviews since M75: detailed, characterful first passes in the house style (palette pixels, outlines, moonlit top-left edges), material texture and a few telling details that read at 2×; effects drawn as animated sprites with particles as accents; motion shown as real in-game loops; UI on near-black.

- **Books.** Each its own cover, colour and glow. The pact: a heavy grimoire in dark, cracked hide with iron corners and an ember sigil, its page edges glowing ember. Bounding: sky-blue tooled leather with a silver wing clasp, its pages lifting as if in a draught, glowing pale cyan. Arcane: a violet cover with gold-leaf runes and a gem, the runes glowing violet over white. The idle loop is a slow shimmer of the glowing parts, never a flicker. Motes in each book's colour: embers and ash for the pact, small blue sparks rising for bounding, violet sparks for arcane. A book must read as a faint glint from a few tiles off in its area's light and as itself up close: that is what hides it (§5 Q2).
- **Emissive colours.** Only `y Y C c` glow, matched by colour value (A15). A sky-blue or violet glow needs a new palette symbol whose RGBA no other symbol has, added to the glow set, so no existing sprite's emissive map changes: `--check` then shows only this brief's files, the manifest, `level.tmj` and `outputs.json` as new or changed.
- **Take.** The book flares in its colour and comes apart into light in about half a second; the sparks follow it out.
- **Bound.** A ring of sky-blue light, flattened under the feet, opening outward, sparks ringing out from it and a short trail of motes left behind as the wizard rises; apart from the double jump's warm sparks.
- **Icons.** Each book as a 32 px subject in a frame and backing of its own that reads apart from the spell icons beside it.
- **Shield liquid.** A light, cool blue, lighter than mana's deep blue (Q7) and clearly over the red health liquid, moving slower than it.
- **Sounds** (redo Q5: the same loop, a playlist with notes). Each pickup apart from `orb_pickup` and from the others: the pact low and ominous (a deep chord, a heartbeat thump, an ember crackle), bounding bright and airy (a rising chime, a soft whoosh), arcane shimmering (an arpeggio over a swelling hum). The bound a bright, airy spring (a quick upward whoosh with a crystalline ping), apart from `air_jump`'s push-off.

### 3.3 Values the code takes

Step 1 tunes these in its captures and writes the final column; the run copies it into the prefabs and `hud.rs`.

| Value | Default | Final |
| --- | --- | --- |
| Glow colours (RGBA) | Pact [255, 110, 60, 220]; bounding [130, 205, 255, 220]; arcane [185, 120, 255, 220] | Pact [255, 110, 60, 220]; bounding [130, 205, 255, 220]; arcane [185, 120, 255, 220] |
| Glow shape | `offset` [16, 16], `radius` 56, `light` 0.5, `reach` 96 | `offset` [16, 16], `radius` 56, `light` 0.5, `reach` 96 |
| Bob | `oscillator` position, `amplitude` [0, 3] px, `period` 2.4 s | `oscillator` position, `amplitude` [0, 3] px, `period` 2.4 s |
| Shield edge, readout (RGBA) | [200, 240, 255, 255], [150, 215, 255, 255] | [200, 240, 255, 255], [150, 215, 255, 255] |
| Shield flow | 6 texels a second | 6 texels a second |

### 3.4 The review sheet

In `perf-runs/20261026-e01tomes-art-review/` (machine-local), shown in the conversation:

- A contact sheet at 4×: the books' frames lit under the hollow's ambient (0.30, 0.28, 0.32), the ravine's night and the aqueduct's moonlight; the take and ring clips; the three icons beside the spell bar's; the shield strip.
- Animated previews: each book's loop with its bob and motes, each take, the bound on a jumping wizard frame.
- A HUD mock at the owner's UI scale of 1.75: the shield layer full and half over the health fill with "+50", and the strip holding one, two and three tomes.
- In-game captures from a scratch driver under `target/` (deleted after): each book with its glow at its spot in §2.3, from the nearest main-route platform and from beside it (the shelves arrive in Step 3, so the books hang where they will stand).
- A playlist of the four sounds with notes, beside `orb_pickup.wav` and `air_jump.wav`.

### 3.5 Lock

On acceptance, Step 1 writes one `sha256sum` line per file §3.1's families own, paths from the repository root, between these fences. The run's check, `sed -n '/^```sha256$/,/^```$/p' docs/plans/example-01-tomes.md | sed '1d;$d' | sha256sum -c --quiet`, exits 0 only when every file is unchanged, and 1 while the block is empty.

```sha256
786a68502a146f3531146a67392f8fd8b1283c3d4d2751cb4773774c623943bb  examples/01_platformer/assets/animations/bound_ring.json
3c0712efc3409681d315eb006265f19e2dd627d2cf6259319b91978ebd331541  examples/01_platformer/assets/animations/tome_arcane_idle.json
ceb8c083182a104032c26994ce1a650b9a48ab9daead56538e6e8f52db7d66e7  examples/01_platformer/assets/animations/tome_arcane_take.json
eca4e671bc8b7d352df97bd1fcab5f71e078041540a0fd559c5403c8517594e1  examples/01_platformer/assets/animations/tome_bounding_idle.json
2b56842ded5ee9c002477aaad528ef08010afb15490b0e2a7d634aa13b20888d  examples/01_platformer/assets/animations/tome_bounding_take.json
bb7ef1f6451e154bdd1f49e40c91a33df3709400fa8e3c38c71b35ba55e3b878  examples/01_platformer/assets/animations/tome_pact_idle.json
e2b035da63fa2b5e32a37c03a003c61853501499ab5d715664924bcbd10fd0d1  examples/01_platformer/assets/animations/tome_pact_take.json
9ab9576cac8c9da7e0739cf3a2ca12748256b5d66bb8aa8d58d39f1a8399e122  examples/01_platformer/assets/particles/bound_sparks.json
6b8f3b54dbb0605920c8ae0b5f587fc230c34f72e332fbf66d64d1671f5fb4e9  examples/01_platformer/assets/particles/bound_trail.json
c055d32d4b666bab2384329d0b8df30ce40f6b5ab468ef6d514ad15b9a25c931  examples/01_platformer/assets/particles/tome_arcane_motes.json
25ba9043a57ad444d79383e114febf3ac9d61db3fc0ef25a2f8e8cec333ea674  examples/01_platformer/assets/particles/tome_arcane_take_sparks.json
50dff321e010ff764815f1c2d305f5fc0fc99f149456802b61cf608f55960605  examples/01_platformer/assets/particles/tome_bounding_motes.json
aca127ca4acc664933cde43b38ad34b3dc06d9b073270a96b49aabb15da4cc97  examples/01_platformer/assets/particles/tome_bounding_take_sparks.json
42afa30a28cac49976f24d5bf617ae44f8aabbef53e5caa9f3f3b47c2b0f0ac4  examples/01_platformer/assets/particles/tome_pact_motes.json
16ab2e3f72e3bf756196196512f947af73af06f9eaacccdd08ff41e921bb245f  examples/01_platformer/assets/particles/tome_pact_take_sparks.json
7da2fd8d449bb2920beca5d7f3106f2b7c65d266bfd3b85ff1dae401a56df45a  examples/01_platformer/assets/sounds/bound.wav
7910ec968da4d5b4741fce6bdc1e79f408c906776f072f8ad7c8234c2fe33aa8  examples/01_platformer/assets/sounds/tome_arcane.wav
02b3da223bf379ce666eae6e52bc25488916955e8125b99c04ea46c5bcb84901  examples/01_platformer/assets/sounds/tome_bounding.wav
279cbc9b2af729d4082fc77ada969e7045b3d2aa7ffc093bb7ee55a9ef62f57a  examples/01_platformer/assets/sounds/tome_pact.wav
f4496e3466d63bd54544f26ca159d6791bf59391378a9d8c29e7f07412f0b03f  examples/01_platformer/assets/sprites/bound_ring_0.png
ba6e1473be3222e8d4656717a4e1f7050ae4254dbe869f923fffda62c7e403a9  examples/01_platformer/assets/sprites/bound_ring_1.png
99838e7b0d6ade2a99e2419c097a62760f0c8e8ea582772b86f84a533c10b52d  examples/01_platformer/assets/sprites/bound_ring_2.png
bb41ed232be430949292ba4ef4a6fbc81e410f587bdf044694c9522427ca6d9e  examples/01_platformer/assets/sprites/bound_ring_3.png
c9aa844658be41b524b38d35bff25fa55c0b502bb2aa4af7c76a9a4f4f713176  examples/01_platformer/assets/sprites/bound_ring_4.png
b54fd468c656c1c5841917c716b7537e9ee67140708083b740328a03b14505e8  examples/01_platformer/assets/sprites/bound_ring_5.png
5ac6bafea3fee34432f0c338b0511db1ec3dece2c5c4f73b584c19314cace015  examples/01_platformer/assets/sprites/hud_liquid_shield.png
56335a6f3be295c01af67b79ecf5f9df751f4c48f1df7f5864a8af1a38fd7820  examples/01_platformer/assets/sprites/icon_tome_arcane.png
fb734c77c99dfdaad6d4dea935fbf8458e108dcf401a59a3eccabac529ceb1cc  examples/01_platformer/assets/sprites/icon_tome_bounding.png
badfc0f9649c03c4355bb53d5f0f8a97e811aa2c95d1f5b255bce441fa8d4552  examples/01_platformer/assets/sprites/icon_tome_pact.png
3b26c126acdd3ab353adae9daa3e845140e35bf67390e8a2709bd5e9bcfbfcd0  examples/01_platformer/assets/sprites/tome_arcane_idle_0.png
b2d40583c19f10987deda256b37222f113934d245a8eb4369b5b5cf70601f376  examples/01_platformer/assets/sprites/tome_arcane_idle_0_e.png
ed2be34548282415f558a03ad8d8b173ace2307b13f1ce88784e839f30e5dc4b  examples/01_platformer/assets/sprites/tome_arcane_idle_0_n.png
c2a29a694a0e7ed2b3bb5fb2e50a5ddbd54c879ddd05acd8faa6f61a302a4ca3  examples/01_platformer/assets/sprites/tome_arcane_idle_1.png
b4cbd0cb3485b6d6a001ed7c192dc5966d4bd81e9cc919f8285d61dfc7c62697  examples/01_platformer/assets/sprites/tome_arcane_idle_1_e.png
9817a0e6ebd3ca32ecf6906bacf423cf5d3419d5cc23b5ec61618b06b778f961  examples/01_platformer/assets/sprites/tome_arcane_idle_1_n.png
02d8fa1e6ac6a15e04970ea11673060e12bdbc524fa2dc69b51e3d0795af5785  examples/01_platformer/assets/sprites/tome_arcane_idle_2.png
b8d4c1021a6ed60e102fe90b87fdcc60f9b9abd0a1e0cad86b4075f1691e4728  examples/01_platformer/assets/sprites/tome_arcane_idle_2_e.png
9817a0e6ebd3ca32ecf6906bacf423cf5d3419d5cc23b5ec61618b06b778f961  examples/01_platformer/assets/sprites/tome_arcane_idle_2_n.png
8ed9b723af0e8f59afee466e34d0698257dee9990db3a09621ca4c81edae19e7  examples/01_platformer/assets/sprites/tome_arcane_idle_3.png
5b6ef9ed2ebefcaf49d4e6c0e7ff5308099acbb738f645837c4e56f18f06bf82  examples/01_platformer/assets/sprites/tome_arcane_idle_3_e.png
ed2be34548282415f558a03ad8d8b173ace2307b13f1ce88784e839f30e5dc4b  examples/01_platformer/assets/sprites/tome_arcane_idle_3_n.png
22f2b1b288eade3a4f918b120f83b336a3a452ecac3f0bb96a2a45a0e4a45249  examples/01_platformer/assets/sprites/tome_arcane_take_0.png
7f8279522cd4a9eb24bac6227b7c6ead295c451af252913a0aff07587285c035  examples/01_platformer/assets/sprites/tome_arcane_take_1.png
de782fd6a7d08526cfd297bba39104354b97e6636f30761facfaa468fbeafb6e  examples/01_platformer/assets/sprites/tome_arcane_take_2.png
d11ca5776dcb7f5671150389f4b97aca7f2f9ca49f6b05d0f65cbae3b98c5aa8  examples/01_platformer/assets/sprites/tome_arcane_take_3.png
bae6172828fa863e14af62ecdd7956373f715d652a86e95e63a61a8c51c46843  examples/01_platformer/assets/sprites/tome_arcane_take_4.png
cbc2e81ab7bc8c1d492398aace8ebac5aebce68e6bb1648ac8ccd8eaaadff05f  examples/01_platformer/assets/sprites/tome_arcane_take_5.png
65aef6f3aa94fd2e26763f9865af1458005694c953aa487f9620096219437191  examples/01_platformer/assets/sprites/tome_bounding_idle_0.png
ad488b2556e69bab59342fcbed239d4490d99d7915e93d0da5101b4163c803d6  examples/01_platformer/assets/sprites/tome_bounding_idle_0_e.png
a13cfb3b8c9c7a3dc51d8cd0388994b9b0691c23da171647ddcde7c6cd0f1cd0  examples/01_platformer/assets/sprites/tome_bounding_idle_0_n.png
a631c610cb55c711c08d24c7b042a817180f42ec74a221290283a32a1f4e2755  examples/01_platformer/assets/sprites/tome_bounding_idle_1.png
ed8ef3af1551055a6c20becc6ff06b48bfcc9902d8d95eae948523a1830045b9  examples/01_platformer/assets/sprites/tome_bounding_idle_1_e.png
11ad513fe2b0ded65ef0586b1498113d54a31d1fd3bb6628703f0760237fe7fd  examples/01_platformer/assets/sprites/tome_bounding_idle_1_n.png
935036e4941b513c9654a9fb9e7875d3c2ceb90c8c9bacede2731104bcb1720b  examples/01_platformer/assets/sprites/tome_bounding_idle_2.png
f4f434ffed8addd4151e404e1d4fa48441cfbc543bc3b98cedf18ed842d24dd2  examples/01_platformer/assets/sprites/tome_bounding_idle_2_e.png
53584989f5d9d2d420359dfb87eb2e94e6a7692a52ba2a5da3fbe5d42e96e2bd  examples/01_platformer/assets/sprites/tome_bounding_idle_2_n.png
98202cbd06a658cc4698d5d198d5398a8d0ece61e4d06538829550c3c225ccf8  examples/01_platformer/assets/sprites/tome_bounding_idle_3.png
632deeeeabe0ff3a8ea23f74c3d0e11102c56bae0d100ca78198285dbc683c39  examples/01_platformer/assets/sprites/tome_bounding_idle_3_e.png
11ad513fe2b0ded65ef0586b1498113d54a31d1fd3bb6628703f0760237fe7fd  examples/01_platformer/assets/sprites/tome_bounding_idle_3_n.png
7921cbe1856eed196cd137cf97127596073a808d6996c4a2a1b32e14079d2487  examples/01_platformer/assets/sprites/tome_bounding_take_0.png
1575015016ed80a79698e30093b4e87510d92d1073139ccb431d920987cf929f  examples/01_platformer/assets/sprites/tome_bounding_take_1.png
6772859e5d4fb80e0bca514a3e4c68dc097f9a1d8b611a7e9c5146360a8fd3c4  examples/01_platformer/assets/sprites/tome_bounding_take_2.png
32303863197fd73921b0a5dde5bbb99da44060499b1c906c098ca36949e60ce2  examples/01_platformer/assets/sprites/tome_bounding_take_3.png
f1690b689d4638a9d5844abc88960d83142870b0aaf6ffc5633c2f5ea6cbe591  examples/01_platformer/assets/sprites/tome_bounding_take_4.png
19f7f880a0990ef90fe5ec73acd2dbcee90c5ee3c23ee3b602f36c63e57eed50  examples/01_platformer/assets/sprites/tome_bounding_take_5.png
0cf18b79e18a031854b1ac8940920157fdab83afefe362c506b98b0c3a53b9ad  examples/01_platformer/assets/sprites/tome_pact_idle_0.png
b515d5334dc5c9f83fc905ac9085199abca763247c4df6bfced5d8a260a66209  examples/01_platformer/assets/sprites/tome_pact_idle_0_e.png
be812860e85aea7d28cc6286f58d4e06415c8d523ae5c000cdff546006f6ce13  examples/01_platformer/assets/sprites/tome_pact_idle_0_n.png
9f6347521e613e9440d7321fdfd2361206ffc41e373f0b173ebbd089c607ca94  examples/01_platformer/assets/sprites/tome_pact_idle_1.png
2a012c6c5b2104d8ed0efc6807cf3c49cfd64402955f04a2acc1e9ccffc51ccf  examples/01_platformer/assets/sprites/tome_pact_idle_1_e.png
be812860e85aea7d28cc6286f58d4e06415c8d523ae5c000cdff546006f6ce13  examples/01_platformer/assets/sprites/tome_pact_idle_1_n.png
02d602d52ad77d68ef852b135c9b1601f00baf99402d2c128bfe3430f9af0185  examples/01_platformer/assets/sprites/tome_pact_idle_2.png
54550dce652fd2b7cc3ab6e29bbe27bb878157649f3c13681b4f33bb9e5cb310  examples/01_platformer/assets/sprites/tome_pact_idle_2_e.png
be812860e85aea7d28cc6286f58d4e06415c8d523ae5c000cdff546006f6ce13  examples/01_platformer/assets/sprites/tome_pact_idle_2_n.png
0246f8e3dc724e948e96068c368ead5e992e7e18bef452b019713426b63fce96  examples/01_platformer/assets/sprites/tome_pact_idle_3.png
dd260eadc3095099979e2f6950740e14b922bb9aac8b56d1f5cc21adec697391  examples/01_platformer/assets/sprites/tome_pact_idle_3_e.png
be812860e85aea7d28cc6286f58d4e06415c8d523ae5c000cdff546006f6ce13  examples/01_platformer/assets/sprites/tome_pact_idle_3_n.png
4513c0f8d08ecbab5b5f87d4ffb41dbc62ee7fa98c5336f27f94c82d2f20f1dd  examples/01_platformer/assets/sprites/tome_pact_take_0.png
4086cb53ea17f1736714fd3c50f5dddc853268fb97eae9e4b3e014f4ffa44011  examples/01_platformer/assets/sprites/tome_pact_take_1.png
fcea4ba9bdb7ce097fed1a7de84aea9475fcff09297c2cba58fb7a1c5c902246  examples/01_platformer/assets/sprites/tome_pact_take_2.png
7ca56d25d36617b96b26540cffd38ebb8ff1002dfa06fe312deb689273cb290c  examples/01_platformer/assets/sprites/tome_pact_take_3.png
17de1f190d2577eda1fe9a0b93b3d8bc1626b99580b49b1361223e7a23dcad23  examples/01_platformer/assets/sprites/tome_pact_take_4.png
5d6434ae1813dc150016c3875b9f8791728a9f0fa5286d227dfcce0190162df8  examples/01_platformer/assets/sprites/tome_pact_take_5.png
```

## 4. Steps

Every code step's gate also takes `cargo fmt --all -- --check` → exit 0 and `cargo clippy -p <crate> --all-targets --locked -- -D warnings` → exit 0 for each crate it changes. A step that regenerates `assets/` makes a scratch Pillow venv once a session (`python3 -m venv "$VENV" && "$VENV/bin/pip" install -r examples/01_platformer/tools/requirements.txt`, `$VENV` in the scratchpad, removed with `shutil.rmtree` at the close) and passes `PATH="$VENV/bin:$PATH" just level-check` → pass and §3.5's check → exit 0. 01's filtered runs name the step's new tests in the evidence, since a filter that matches nothing still exits 0.

### Step 1: Art and sound (the asset session)

- **Who:** a new session on Opus 5.5 at max, on §3 alone; the owner reviews.
- **Files:** Step 1's in the header.
- **Change:** draw §3.1's families in the art modules with the generator's toolkit, the clips and particle configs where the generator keeps its others, and the four sounds in `sfx.py` and `sounds.json`; regenerate; write §3.4's sheet and show it; take the owner's notes in rounds, a new small session when the context grows; on acceptance fill §3.3's final column and §3.5's block, add the evidence row, delete the scratch driver and the venv, and commit.
- **Done-when:** the owner's acceptance quoted in the evidence row; `PATH="$VENV/bin:$PATH" just level-check` → pass; `cargo test -p example-01-platformer --locked` → pass (art only); `just repo-check` → pass; §3.5's check → exit 0; `git status --porcelain` → only Step 1's paths.
- **Commit:** `feat(example-01): tome, bound and shield art and sounds`
- **Moves:** `assets/tilemaps/level.tmj`'s tile IDs, which new sprites shift; the manifest.

### Step 2: The kit, a paid air jump and a cost scale (K25, K26)

- **Files:** Step 2's in the header.
- **Change:** §2.2, rustdoc on every new public item; `just api`.
- **Done-when:** `cargo test -p tungsten-kit --locked` → pass, with §2.2's tests among `motion::platformer::tests` and `intent::ability::tests`; `cargo test -p tungsten --lib --locked kit_recipes::determinism` → 1 passed (the pin unmoved); after `just api`, `git diff --name-only -- api/` → `api/tungsten-kit-components.txt` and `api/tungsten-kit.txt` alone, and `just api-check` → pass; `just docs-ratchet` → pass; `just repo-check` → pass.
- **Commit:** `feat(kit): paid air jumps and ability costs scaled by a stat`
- **Moves:** the two `api/` snapshots.

### Step 3: The books on the map, the prompt, the take and the strip (R1–R4)

- **Before:** Step 1's row says accepted and §3.5's check exits 0; otherwise the run stops here.
- **Files:** Step 3's in the header.
- **Change:** §2.3, §2.7, §2.8 for `tomes`, §2.9's three pickup sounds; 01's README: the controls row, the mechanics row, the saves line, the tests module.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- tomes:: level:: saves:: seam:: data:: hud::` → pass, with new tests that a book's panel shows within 64 px and hides beyond, an `interact` press takes the nearest book once with its burst and its sound, the strip follows the order taken, Step 3's save and migration tests (§2.8), and the double-jump test (§2.3), every existing `LAUNCHES` row still landing and `every_platform_sweep_clears_terrain_and_player_headroom` passing with the new supports; the level-check and lock gates above.
- **Commit:** `feat(example-01): three hidden tomes, their prompt and the strip`
- **Moves:** 01's save schema to 16; the manifest and `level.tmj` (prefabs, the layer, three supports).

### Step 4: The pact and the shield on the bar (R5, R6)

- **Files:** Step 4's in the header.
- **Change:** §2.4, the shield in §2.8; 01's README.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- tomes:: saves:: hud:: player::` → pass, with new tests that the pact turns 100 / 100 into 50 / 50 and 80 / 100 into 40 / 50 with a full shield of 50, that a 75-point hit landing in the take's own step leaves 25 / 50 behind an empty shield, that a 30-point hit takes the shield to 20 and leaves health whole, that the shield holds for 3 s after a hit and then fills at 20 a second, that the kill row kills a player behind a full shield, that the layer reads full with "+50" at 50 and half with "+25" at 25 and hides the readout at 0, and that a half shield saves and loads back.
- **Commit:** `feat(example-01): the demonic pact and its shield on the bar`

### Step 5: Bounding (R7)

- **Files:** Step 5's in the header.
- **Change:** §2.5, `Bounded` (§2.9); 01's README.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- tomes:: player:: data::` → pass, with new tests that a wizard holding bounding spends nothing on the ground jump or the first air jump and 25 mana on each later one, makes one with exactly 25 and is refused with 24 and one `NoMana`; that a paid jump sounds `Bounded` and bursts `bound_sparks`, never `AirJumped`; and that a wizard set 10 tiles over the map's top falls back alive; `an_air_jump_and_a_spell_switch_each_sound_once` unchanged and passing; the gates above.
- **Commit:** `feat(example-01): the tome of bounding`

### Step 6: Arcane power (R8)

- **Files:** Step 6's in the header.
- **Change:** §2.6; 01's README.
- **Done-when:** `cargo test -p example-01-platformer --locked --test game -- tomes:: spells:: hud:: lightning:: combat::` → pass, with new tests that arcane power makes the missile cost 8 and the summon 20, leaves the staff light's step at 0.05, and flips the HUD's payable at 8 mana; that a spray pulse, the missile's blast, the strike and the chain's first jump each deal a dummy twice what the same hit deals without it, crits aside, while a gun shot and a bowling ball's impact deal the same; and that all three tomes together leave 50 health, a 50 shield, a missile at 8 and a paid jump at 25; the gates above.
- **Commit:** `feat(example-01): the tome of arcane power`

### Step 7: Close

- **Files:** Step 7's in the header.
- **Change:** run the checks and the demo; take in-game captures of the books in place and taken, the prompt, the shield on the bar, the strip and the bound, by §3.4's driver, for the owner; finish 01's README; `DESIGN.md`'s workspace line gains the tomes in 01's clause; §3.8 of the feedback gains its landed note, as §3.7a's has; open follow-ups to Later as one line each, or to known issues; `e01tomes` out of Now and `e01foes` into it; delete this plan and its citations; remove the venv.
- **Done-when:** `just check` → pass (formatting, lints and every crate's tests, 01's whole suite among them); `PATH="$VENV/bin:$PATH" just level-check` → pass; `just smoke` → pass; `just repo-check` → pass; the captures saved and named in the last evidence row.
- **Commit:** `docs(example-01): records follow e01tomes`
- **Ends with:** the commits made, the evidence quoted, what the owner should play (each book's spot and the three tomes together) and the next prompt.

## 5. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The order of the sessions | Step 1 first; the run starts once its lock is in (Step 3's check), since the prefabs name §3's IDs and the manifest must hold them. Step 2 needs no art, so it may run beside Step 1 in a worktree the owner makes |
| Q2 | "Seen only from close by" (Q34) | Judged in Step 1's captures and the owner's play, not by a test: dark spots off the ways and a small glow |
| Q3 | Where the books lie (Q34) | §2.3's three spots; the run may move one up to 2 tiles in its area, recorded |
| Q4 | Taken books in a save (§3.8 step 8 said `gone`) | By kind in the player's `Tomes`: books take no save ID, so no ID moves and no migration shifts one |
| Q5 | The prompt's reach | 64 px from the body's centre to the book's centre, the nearest book only |
| Q6 | K27 | Not fixed: shield hits read as hits, ailments build through the shield, and the bar shows the loss; it stays in the backlog's row beside K28 |
| Q7 | The shield's readout at 0 | Hidden |
| Q8 | Bounding behind the falls (Q34) | Not there: a shelf in the falls' columns fails the generator's stream check (A29), and the falls stay as they are, so it stands east of the zigzag (§2.3) |
| Q9 | A bound past the map's top | The player's box check leaves the top out; the view stays clamped, a Later line if play shows it matters |
| Q10 | Bounding past the elevator's gate | Accepted: a power-up; the reach tests pin the stock player |
| Q11 | A refused bound's feedback | The no-mana sound, once a press |
| Q12 | The arcane stats on old saves | The take sets the bases it needs, so no migration adds them |

Each question takes its stated default; the owner reviews the plan on their own time.

## 6. Invariants and stop conditions

- **May move:** `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt` (Step 2); the manifest and `level.tmj` (Steps 1, 3, 5, 6); 01's save schema, 15 to 16 (Step 3).
- **Must not move:** the kit hash `0x3334_9665_de0c_501a`; the determinism and containment hashes and the row digests (no core change); `gpu-visual.png` and the post and transition regressions; 01's `src/shell/`, byte for byte the template's; the description counts and the numeric census (A4); §3.5's files after Step 1; the stock player's jump and every existing `LAUNCHES` row.
- A done-when check fails: restore the step's files from the last commit, mark the step skipped and carry on with the steps that don't depend on it (3 needs 1 and 2; 4–6 need 3; 5 and 6 need 2).
- §3.5's check fails after a regeneration: an art file changed, which only an asset session may change; restore the step's files.
- The kit hash moves: Step 2 restores and stops.
- A spot cannot be reached within 2 tiles of §2.3's without breaking an existing row, meeting a stream's columns or a mover's sweep: take the nearest working spot in the same area and record it, or with none there the nearest in a neighbouring area, which Step 7 names to the owner.
- A file outside the step's list needs a change: a test that pins a value the step moves is added and recorded; anything else skips the step.
- The Pillow venv will not install: stop, since the generator cannot run.

## 7. Critique

The `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-26, reading `2e4ecb2` with this plan uncommitted; round 1, the only one the owner's prompt asked for. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 660.4 s, tokens in 1112062 (cached 1020032), out 11878 (reasoning 6205), commands 34, output 350.9 KB (largest 33.4 KB), usage 5h 2% → 27%, week 0% → 4%`.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] The pact's shield activates before its health cost: the take runs in Intent after `stats_apply`, so a hit in the take's step meets the shield over unhalved health | `crates/tungsten-kit/src/foundation/phases.rs:47-58` (Tick before Intent); `stats.rs:749-751` | Accepted | §2.3: the take runs in Tick before `STATS_APPLY`; §2.4; Step 4 tests a 75-point hit in the take's step; A8 |
| 2 | [MEDIUM] Schema 15 passthrough can restore players inside the new shelves | `src/save.rs:1577`, `:1942-1953` (`place`); `:615-716` (`to_15`'s precedent); `platformer.rs:110-113` | Accepted | §2.8: `to_16` sets a body inside a new support on its top, dropping one with no room; Step 3 tests it; A28 |
| 3 | [MEDIUM] Buffered paid jumps have contradictory reporting rules | `platformer.rs:610` (a buffered press jumps); `tools/content/data/effects/mana_regen.json` (3 a 0.1 s) | Accepted | §2.2: a paid jump takes a buffered press and reports on its step; a refusal reports on the fresh press only; a kit test of a refused press that pays inside its buffer |

Checking finding 2 against the tree also showed two spots the generator or the level tests would refuse: a shelf in the falls' columns fails the stream check (`tools/placement.py:371-373`), and the bay at 137–140 lies in a moving platform's sweep, while the bay at 153–156 holds a pour and a statue. Bounding moved east of the zigzag and arcane to the bay at 117–120 (A29, §2.3, Q8). Round 1 changed mechanisms (the take's phase, a migration, two spots), for which the skill allows a second round; it was not run, at the prompt's word.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-26 | Pass: the owner accepted round 1's sheet, "Accept the changes and prepare for next session" | 83 files under §3.1, hashed in §3.5: 64 PNGs (the 12 idle frames with their normal and emissive maps, 18 take frames, 6 ring frames, 3 tome tiles, the shield strip), 7 clips, 8 particle configs, 4 WAVs (pickups 1.10, 0.95 and 1.15 s, the bound 0.32 s, peaks −2 dBFS). `PATH="$VENV/bin:$PATH" just level-check` → pass (4,898 owned outputs match, 24 tests); `cargo test -p example-01-platformer --locked` → pass (333 passed, 1 ignored); `just repo-check` → pass (0 errors); §3.5's check → exit 0 (83 lines); `git status --porcelain` → only Step 1's paths (95). The palette gains five glow symbols in `art.GLOW` (`Ъ`, an ember orange beside the brief's cyan `Ч Я` and violet `Э Ы`); `assets/data/opaque_tiles/level.json` gains 4 lines (the shield strip and the three square tome tiles are opaque); the motes launch fast because particles draw at z 0 under the book; §3.3's finals are its defaults | 01's `tools/{art,effects,generate,sfx}.py`, `tools/{sounds,palette,outputs}.json`, `tools/README.md` and `assets/` (83 new, `manifest.json`, `tilemaps/level.tmj`, `data/opaque_tiles/level.json`); the sheet in `perf-runs/20261026-e01tomes-art-review/` (machine-local), published at https://claude.ai/artifact/EDdQScct3BdqSydJ31fUnv; the shot driver and the venv deleted |
| 2 | 2026-10-26 | Pass | `cargo test -p tungsten-kit --locked` → 776 passed (lib and doc tests), the new `past_the_free_air_jumps_each_press_pays_with_no_cap_and_below_the_cost_is_refused`, `the_air_jump_cost_and_its_flags_round_trip_and_an_old_save_has_none`, `an_air_jump_cost_on_a_bad_stat_panics`, `a_negative_air_jump_cost_panics` and `a_scaled_cost_spends_the_amount_times_the_users_stat` among them (`-- motion::platformer::tests intent::ability::tests` → 63 passed); `cargo test -p tungsten --lib --locked kit_recipes::determinism` → 1 passed (the pin unmoved); after `just api`, `git diff --name-only -- api/` → `api/tungsten-kit-components.txt` (one line, `scale`) and `api/tungsten-kit.txt` (8 functions); `just api-check` → current; `just docs-ratchet` → no rise; `just repo-check` → 0 errors; fmt and clippy → exit 0. The runner moves the cost across its controller copy rather than cloning it, so a step still allocates nothing | `crates/tungsten-kit/src/motion/platformer.rs`, `crates/tungsten-kit/src/intent/ability.rs`, their tests, `api/`, `DESIGN.md` |

## Follow-ups

- K25's free count set at run time, and an unlimited free count: no consumer (§2.1); a Later line if a game wants them.
- The generator's own lock for accepted art: redo step 7.1.
