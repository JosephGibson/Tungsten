# Phase 6 milestone 78: 01 quick fixes (01 quick fixes)

- **status:** done
- **goal:** Example 01's ten quick fixes from the owner's notes of 2026-10-22 ([feedback](../../1.0/example-01-feedback.md) §3.1a): glowing orbs with a potion sound and a burst; a two-second death with a clip and a sound; ambience at half volume; a crawler lunge that always lands, with a clearer clip; two more bats each in the aqueduct and the ravine and a crawler at the falls' foot; bold serif area titles with no box; the staff light off at a run's start; eight placed health orbs (schema 9); no seam under the caves' ceilings; frozen wisps that fire and lava blow apart, through a kit event for parts an immunity drops. Planned and run in one session, with the owner's review of captures before the test pass and the release.
- **non-goals:** §3.1b's ground: the lava's touch on the player and its size, the mine's elevator, a fourth beetle and its charge effect, the hollow's cave mouth, the title's difficulty picker. The template and 01's `src/shell/` (byte copies). I9 (orbs appear and vanish silently), left to (f) (Q4). The wizard's redraw (h). Engine changes: the banner's shadow is built from copies of its label, and `K5` stays a gap. Tests beyond what a done-when names (examples test lightly).
- **files to touch:** per step in §3. In sum: `examples/01_platformer/` (`src/features/{combat,enemies,level,player,run,audio}/`, `src/save.rs`, `tests/game/{enemies,saves,player,hazards,presentation,spells,data,dummy}.rs`, `tests/game/fixtures/schema8.json`, `tools/{level.json,level.py,placement.py,actors.py,sfx.py,sounds.json,generate.py,README.md}`, `tools/content/{prefabs,data/cue_rules}/`, the generated `assets/` and `tools/outputs.json`, `README.md`); the kit's `crates/tungsten-kit/src/{damage/health.rs,damage/pipeline.rs,lib.rs,tests/pipeline.rs}`; `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`; `docs/plans/1.0/{w04-api-freeze.md,example-01-feedback.md,implementation-plan.md,README.md,roadmap.json}`; the release step's records.
- **ordered steps:** (0) Before shots and a schema 8 save. (1) The kit's immune parts. (2) Frozen wisps and fire. (3) The crawler's lunge. (4) Bats, the falls crawler and schema 9. (5) Placed health orbs. (6) The orbs' glow, potion sound and burst. (7) Death. (8) Ambience. (9) Area titles. (10) The staff light off at the start. (11) The caves' ceiling seam. (12) The owner's review. (13) The test pass. (14) Release 0.99.
- **done-when:** every step's evidence row quoted; the owner's review notes applied (step 12); workflow §5 closed; the release checks, with `just level-check` and `just api-check`, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate 01 quick fixes ([implementation plan](../../1.0/implementation-plan.md) §3 card, level B), milestone M78, release 0.99. Written 2026-10-22 at `be16369`, whose tree is `67f00c3`'s. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs: at the owner's request the plan and the run share one session, the run stops for the owner's review of in-game captures (step 12) before the test pass (step 13) and the release, and steps 1–11 run only the tests their own done-whens add.

## Context digest

- 01 is a sandbox on the template, kit first (`D-194`): a kit gap is fixed at the kit or becomes a backlog row, never a workaround in 01. 0.98 (M77, `D-205`) landed (e): the staff light that burns mana and scares the crawlers (lit at a run's start, Q15), schema 8, the banner of 01's own (B3).
- These are the owner's notes of 2026-10-22, split by size from §3.1b (0.100). Opus max by the stop's own tier (`D-195`): the run draws clips and glows and synthesizes sounds.
- One item reaches the kit: a part a target is immune to drops before any event (A33), so nothing in a game can react to the type that struck. Step 1 adds `ImmunePart`, an additive event, the stop's only public API; the roadmap's "No public API" note was written before the cause was found.
- Saves: the new map objects (five enemies, eight orbs) take save IDs before the player's, so schema 9 shifts every later ID; a genuine schema 8 save, captured from the 0.98 build before any change, is the migration's test.
- Captures run through a scratch crate outside the tree (`target/e01q-shots/`, the recipe of the in-game shots note): a run started at frame 5, the player pinned at a tile, `TUNGSTEN_CAPTURE_FRAME` in smoke mode, `TUNGSTEN_USER_DIR` in the scratchpad.

## 1. Audit

Read at `be16369`. Paths are under `examples/01_platformer/src/features/` unless rooted; `01/` is `examples/01_platformer/`, `kit/` is `crates/tungsten-kit/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Orbs: `health_orb`/`mana_orb` prefabs (unlit 24 px sprites with a baked halo, z 12), a `pickup` with magnet 360, `lifetime` 20 s, and the `picked_up` cue's chime `ex10_orb_pickup_sfx`; no glow, no burst; drops roll from `enemy_drops` (health 30, mana 30, none 40) | `01/tools/content/prefabs/health_orb.json:1-39`; `01/tools/effects.py:713-728`, `:772-773`; `01/tools/sounds.json:255-259`; `01/tools/sfx.py:814-819`; `01/tools/content/data/loot_tables/enemy_drops.json:1-16` |
| A2 | `orb_adopt` gives a dropped orb its save ID; `orb_collect` applies `orb_heal` (20) or `orb_mana` (25) on `PickedUp`; `drops::spawn` is the load path with its own `Lifetime` | `enemies/drops.rs:57-67`, `:69-82`, `:84-101`; `01/tools/content/data/effects/orb_heal.json` |
| A3 | `Glow` (01's) always spawns a point `Light`, and a halo and core sprite when set; halos hide with the Bloom option. 15 map lights today plus the lantern and the moon against a cap of 16 that keeps query order inside the view (P3) | `level/components.rs:86-124`; `level/systems.rs:77-133`; `crates/tungsten-render/src/lighting.rs:130-147`; `docs/known-issues.md:49` |
| A4 | Bursts: `shared::spawn_burst(world, name, at)`, at most 16 alive; burst configs in `particle_configs()` with `color_over_life` | `01/src/shared.rs:51-88`; `01/tools/generate.py:26-317` |
| A5 | `DEATH_SECS` 3.0, its one use counting game time to `shell::game_over`; a test hard-codes 3 s | `run/mod.rs:44-46`, `:131-143`; `01/tests/game/player.rs:128-166` |
| A6 | Death has no clip or sound: the corpse loops `idle` (breathing, blinking, glowing eyes) on the ground, the killing hit plays the hurt sound, and a toss or cast twin started before death plays on. One-shot clips go through `Shown`, which a test expects empty a step after death | `player/systems.rs:180-193`, `:204-223`, `:269-356`; `01/tools/actors.py:431`; `01/tests/game/player.rs:226-227`; `01/tests/game/hazards.rs:80-111` |
| A7 | Player frames: the lowest pixel on row 60 and the lantern glass (`y`/`Y`) wholly inside `(40, 30, 63, 60)` in every frame; a kneel or slump fits the rig (`hip`, `head`, `eyes`) | `01/tools/actors.py:18-21`, `:28-55`, `:552-627`; `01/tools/placement.py:32-41` |
| A8 | Sounds played from code: `Sound` (36 variants), `ALL`, `event()`, `cue_of` by index, a cue rule each | `audio/components.rs:40-198`; `audio/mod.rs:49-118`; `01/tools/content/data/cue_rules/sounds.json:536-553` |
| A9 | Ambience: the six area loops at 0.35, 0.35, 0.4, 0.4, 0.3, 0.35; the volume is data, which the game reads through the registry; the tools README quotes "0.3 to 0.4" | `01/tools/sounds.json:266-301`; `audio/systems.rs:150-191`; `01/tools/README.md:614-615` |
| A10 | The crawler hurts by `contact_damage` through its hurtbox (cold 12, interval 0.6), always on; there is no hitbox. Its hurtbox `[24, 9]` at `[32, 50]` is exactly as wide as its body `[24, 10]` | `01/tools/content/prefabs/crawler.json:14-65` |
| A11 | Both bodies are kinematic controllers that block each other; a move stops 0.01 px short (`SKIN`). A lunge that stops against the player leaves the centres 44.01 px apart against a combined reach of 44, and a touch that does not overlap is no hit: a lunge at a still player deals nothing | `kit/motion/platformer.rs:82-87`, `:504-507`; `kit/motion/slide.rs:10`, `:70-75`; `kit/overlap/grid.rs:283-286`, `:357-364` |
| A12 | The wind-up gate allows 440 px of height, the lunge reaches under 40 px up: crawlers lunge into steps at players on ledges 146–274 px above | `enemies/crawler.rs:170-174`; `:203-210`; `:217-225` |
| A13 | The lunge clip: four frames, three poses (0.35, 0.7, 1.0), 400 ms over the tell, its last frame held through the leap; `Recover` shows the walk | `01/tools/actors.py:1062-1073`, `:1654-1662`; `enemies/crawler.rs:246-254`; `enemies/systems.rs:599-635` |
| A14 | Map enemies: aqueduct crawlers a, b, c (104.5, 119.5, 137.5); ravine bats a, b under `rim_west` (163.5, 171.5) and c under `east_wall` (209.5); entries `{kind, name, x, y}` with x, y the sprite's top-left in tiles, in the map's last layer | `01/tools/level.json:9713-9791`; `01/tools/level.py:253-257` |
| A15 | A bat roosts by a cast straight up (480 px) and hangs 28 px under what it meets; the generator refuses a bat with no wall within 7.5 rows above; sight 440 px, dive within 300 px | `enemies/bat.rs:21-54`; `01/tools/level.py:570-575` |
| A16 | Roosts: the aqueduct's channel deck (row 28, cols 92–108, 113–128, 133–144, 149–155) over the climb's slab ledges (rows 36, 33, 31); the ravine's `rv_ledge` 1–7 (zig-zag, undersides rows 34–58) and the east ledges (cols 197–202, rows 48–60) | `01/assets/tilemaps/level.tmj` collision; `01/tools/level.json` platforms |
| A17 | The falls drop at cols 178–179 to the pool bed (row 63), which runs unbroken from col 177 to the lava at 232; a crawler patrol has no leash, so one placed at the foot would walk into the hollow | `01/tools/level.json` props and `pool_bed`; `enemies/crawler.rs:135-152` |
| A18 | Banner: one label on a root built by `add_root`, which is a `Panel`; the theme's panel paints a `#15171d` fill and a 1-unit `#646b7a` border: the box in the before shots. Regular weight, size 20 on 28, fade 0.35 s in and out over 2.5 s, a hard cut on a new area | `run/banner.rs:13-14`, `:28-34`, `:44-73`; `crates/tungsten-core/src/ui/tree.rs:241`; `01/assets/ui/themes/shell.json:5-6`, `:20-21` |
| A19 | `serif` holds only Source Serif 4 Regular; the generator copies the fonts and their manifest sections from the template byte for byte, and `--check` refuses any file under `assets/` it does not own. The repo's own `assets/fonts/SourceSerif4/static/SourceSerif4-Bold.ttf` (OS/2 weight 700, version 4.004) sits beside a Regular byte-identical to 01's, under the same licence | `01/assets/manifest.json:522-544`; `01/tools/generate.py:17-20`, `:380-381`, `:486-491`; `cmp` of the two Regulars and licences |
| A20 | A label's weight picks the family's nearest face and falls back to Regular in silence; no synthetic bold. `TextOverrides` has `weight` and `letter_spacing` (em); UI text has no shadow (`K5`); a child can sit out of flow (`Placement::Anchored`) and earlier children paint behind | `crates/tungsten-render/src/text/fonts.rs:99-107`; `crates/tungsten-core/src/ui/theme.rs:471-494`; `crates/tungsten-core/src/ui/style.rs:131-147`; `crates/tungsten-core/src/ui/paint.rs:810` |
| A21 | `Lantern::default()` is `on: true`, used only by `player::spawn`; a load restores `save.lantern` (a field since schema 7), so nothing loaded relies on the default; a Retry with no readable slot starts fresh | `player/components.rs:20-42`; `player/mod.rs:103-117`; `01/src/save.rs:68-72`, `:761-765` |
| A22 | Lit-start tests: the lantern toggle test, the bloom test, a spells test, the guttering save test, the crawler scare test | `01/tests/game/presentation.rs:428-518`, `:554-581`; `01/tests/game/spells.rs:207-260`; `01/tests/game/saves.rs:1634-1656`; `01/tests/game/enemies.rs:692-752` |
| A23 | Only the Ember hollow is indoor (ambient 0.30/0.28/0.32); the ravine and the mine are outdoor night areas. The hollow's lights: the lava lake's glow and three wisps; the mine's: a hanging lantern and a lantern | `01/tools/level.json:30-66`; `level/mod.rs:58-66`; `level/systems.rs:197-234`; `01/tools/level.py:48-56` |
| A24 | The crawler's fear reads the lit light (reach 175 px); the wind-up is gated on `!lit` | `enemies/crawler.rs:68-83`, `:169-176` |
| A25 | Map objects take save IDs in layer order: props 0–811, slabs 812–831, iron 832–838, hazards 839–840, platforms 841–845, areas 846–851, emitters 852–858, the dummy 859, enemies 860–872, the player 873, a fresh run's iron blocks 874–877 | `level/mod.rs:146-152`; `run/mod.rs:74-95`; `01/assets/tilemaps/level.tmj` |
| A26 | Schema 8: `migrate` takes 7 only (the staff light's ability entry). Saved IDs: `next_id`; `player.id`, `.sources[*][1]`; `platforms`, `hazards`, `blocks`, `black_hole`, `missiles` (`id`, `source`), `bullets` (`id`, `source`), `dummies` (`id`, `sources`), `enemies` (`id`, `sources`, `hit`, `touches[*][0]`, `held`), `throws` (`body`, `thrower`), `orbs.id`, `balls[*][0]`, `gone`, `cascades` entries | `01/src/save.rs:56-130`, `:139-236`; `elements/cascade.rs:66-88` |
| A27 | A killed map enemy stays gone through `gone`; placed enemies are found by ID on load; dropped orbs are saved with their `Lifetime` and spawned fresh | `01/src/save.rs:537-550`, `:736-744`, `:810-824`, `:862-870`; `enemies/mod.rs:340-408` |
| A28 | Tests pinning counts: `MAP_ENEMIES` 13 (`dummy + 1 + MAP_ENEMIES == player`), 13 placed, an orb count of 1 in a full run, both orb prefabs' lifetime; the coverage test's orb lists | `01/src/save.rs:97-100`; `01/tests/game/dummy.rs:44-48`; `01/tests/game/enemies.rs:1711-1712`, `:1858`; `01/tests/game/data.rs:75-81`; `01/tests/game/saves.rs:522-529` |
| A29 | "The stand helper": no function by that name; `placement.prepare` places a prop by its `support` and `anchor` from its sprite's box and refuses one off its support (`prop outside support`) | `01/tools/placement.py:19-57` |
| A30 | The seam: a ceiling's bottom tile (`basalt_under_*`, `wet_rock_4_*`) is cut away 0–12 px (basalt mean 9.2, wet 5.9), and no back tile is ever put behind a ground cell, so the unlit backdrop (sky, ridges, cliffs) shows through as a stripe under the rock, undimmed by the hollow's 0.30 ambient | `01/tools/level.py:296-315`, `:366-387`, `:396-401`; `01/tools/terrain.py:920-933`, `:1103`, `:1201-1215`; `level/draw.rs:71-80`; before shots |
| A31 | The ravine's mist band 2 is densest at its two tiles' boundary and loses its upper tile in ceiling cells, so its lower tile starts on a hard bright edge under `east_wall` (cols 203–210); band 4 loses its upper tile under the mine's `cut_top` (cols 281–289) and band 3 under `scaffold_3` | `01/tools/level.py:477-480`; `01/tools/level.json:9637-9639` |
| A32 | The hollow's back (`indoor`, no `backs` entry) darkens each tile's top rows and hangs stalactite back tiles from pixel row 0, which falls in the stripe; hanging props tuck only 6 px into the 9.2 px gap | `01/tools/backdrops.py:485-486`, `:533-540`; `01/tools/placement.py:7`, `:54` |
| A33 | Item 10's cause, (a): a resist factor of 0 drops the part before any event (`product_with` → `None`, `land` → `None`, `resolve` → `Hit::Ignored`), so the wisp's fire never reaches `combat_reactions` as a `DamagedPart`; the reaction itself is right | `kit/damage/defense.rs:258-262`; `kit/damage/pipeline.rs:662-677`, `:709-733`, `:780-788`; `combat/systems.rs:55-108`; `01/assets/prefabs/wisp.json:62-64` |
| A34 | (b) does not hold: missile, flamethrower and blasts are non-periodic. (c) holds besides (a): lava's touch is untyped 34 filtered to `team.player`. (d) does not hold: a frozen flyer drops its mover and falls, and wisp b roams over basin 0 | `toys/systems.rs:483-497`, `:905-910`; `elements/cascade.rs:525-546`; `01/assets/prefabs/ex10_lava_0.json:3-21`; `kit/overlap/contact.rs:751-756`; `enemies/systems.rs:535-551` |
| A35 | Thermal shock on a frozen target ends its frozen, hits everything in 128 px for 16 fire and 16 physical, and bursts; a full wisp (40) survives its own blast | `elements/cascade.rs:491-501`, `:552-594` |

**Where the prompt's facts differ from the tree**

- Item 4: there is no hitbox to arm; the crawler hurts by contact damage through its hurtbox, always on (A10).
- Item 6: the banner's background is its root, a panel (A18); the Bold face needs no fetch, the repo holds it (A19); `widgets.rs` and `theme.rs` are shell copies, so every change lives in `run/banner.rs` and the generator.
- Item 7: only the Ember hollow is a cave by its light; the Iron mine is an outdoor night area at the full midnight ambient (A23). The lit start is pinned by five tests, not one (A22).
- Item 8: "the stand helper" is `placement.prepare`'s support check (A29); the pickups need it extended.
- Item 9: the hollow has no `backs` strip, its back comes from `indoor` (A30, A32); the cause is a missing back behind the ceiling's cut-away underside, not a `top`, an edge row or a strip boundary; the mine's `cut_top` underside is opaque, so the mine shows no stripe, only the mist edge (A31).
- Item 10: the cause is (a), in the kit (A33), and (c) for lava besides (A34).
- Item 2: the player's prefab has no `cues` since 0.98; the hurt plays from code through a cue rule (A6, A8).

## 2. Design

### 2.1 The kit: `ImmunePart` (gap K15)

```rust
/// One part of a hit that the target's immunity dropped: a type it resists
/// at a factor of 0. Sent once [`HealthPlugin`] has registered its queue, in
/// part order, whether the rest of the hit lands or the hit is ignored; a
/// part of 0, a part a non-zero factor shrinks to nothing, and a target that
/// takes no hits send none. Build one with [`ImmunePart::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ImmunePart {
    pub entity: Entity,
    /// The part's amount before the resistances.
    pub amount: f32,
    pub kinds: DamageKinds,
    pub source: Option<Entity>,
    pub position: Option<Vec2>,
    pub periodic: bool,
    pub crit: bool,
}
// new(entity, amount), with_kinds, with_source, with_position, with_periodic, with_crit
```

`Defences::land` returns landed, immune or dropped; `land_parts` fills a second buffer with the immune parts' amounts and kinds; `resolve` sends them after `takes_hits` and before its `Hit::Ignored` check. No trace kind, no cue event, no `Damaged`; the landed path's arithmetic is unchanged. `HealthPlugin` registers the queue and its description adds the event. Re-exported beside `DamagedPart`.

### 2.2 Frozen wisps and fire

`combat_reactions` reads `ImmunePart` with a cursor of its own: a non-periodic part with the fire kind on a target frozen when the resolve began queues the explosion, as a landed one does. Cold on a burning immune target is left as it is (Q7). Each lava basin gets a child, `ex10_lava_frost_<i>`, through `spawn_children` (no map object, so no save ID): a typed `{"damage.fire": 34}` touch over the basin's touch box, filtered `{"all": ["status.frozen"]}`, no knockback. The player cannot be frozen, so its touch stays today's untyped 34.

### 2.3 The crawler

The touch gets its own shape, `aabb [28, 12]` at `[32, 51]`: 4 px past the body each side and 2 px above and below, so a stop flush against the player from beside, above or below still overlaps, as the bat's hitbox reaches past its body. The wind-up asks the target to be at most `LUNGE_RISE` 64 px above the crawler's centre (below stays `2 × LUNGE_RANGE`). Three clips replace `crawler_lunge`: `crawler_windup` (coil back, jaws parting, four frames over the 0.4 s tell), `crawler_lunge` (stretched long and low, forelegs reaching, three frames, the last held in the air) and `crawler_recover` (skid, settle, three frames over 0.36 s), shown in `WindUp`, `Attack` and `Recover`. A placement may set a patrol leash in tiles (`enemy.leash`, serde default none): a patrol turns back past it from its home.

### 2.4 Map additions

Appended to `enemies`, so the 13 keep their IDs: aqueduct bats at cols 98 and 106 under the channel deck (the arcade's keystones there), over the climb's ledges and crawler a's floor; ravine bats under `rv_ledge` 4 (col 189.5), over the zig-zag's lower ledges, and under the east ledge at row 57 (col 198.5), over the pool bed; the falls crawler on the pool bed at col 180.5 with a leash of 5 tiles. Each bat passes the generator's roof check and is placed 0.86 rows under its roost, as bats a and b are.

### 2.5 Placed orbs and schema 9

`level.json` gains `pickups`: `{name, support, x, float}`. `placement.prepare` checks the orb's centre over its support's columns (a covering stair taken, as props are) and sets `y` so the orb's centre floats `float` tiles over the surface; level.py writes them as `health_orb` objects in a `pickups` layer after `enemies`. At a run's start `drops::start` records them (`PlacedOrbs`) and removes their `Lifetime`; `gone` adds the taken ones. `SCHEMA` 9 and its `migrate` land with the first new map object (step 4), so no step leaves new map objects under schema 8: `migrate` from 7 runs 7 → 8 then 8 → 9, and 8 → 9 moves every save ID at or above schema 8's `player.id` up by the new map objects, `save::SHIFT_9 = (MAP_ENEMIES − 13) + MAP_PICKUPS`: 5 after step 4 (`MAP_PICKUPS` 0), 13 after step 5 (8). It covers every field of A26 plus `enemies[*].held`; the new placements are alive and every orb is there in a migrated save. `enemy.leash` reads as none from older saves. The migration's test loads a genuine schema 8 save captured from the 0.98 build in step 0 and expects the shift the constants give.

Default placements (eight; the review lists each with its reason): Lantern clearing, an off-path reward on the crystal ledge; aqueduct, past crawler c on the floor; aqueduct, off-path on the channel deck's east span; ravine, near the falls' foot on the pool bed; Ember hollow, past basin 0 on the far shelf; Ember hollow, off-path on the gallery; Iron mine, on the second bench; troll bridge, on `bridge_head` before the bridge. Each over a surface the support check confirms, reachable without the ice spray, none within 96 px of a test's fixed point (`hazards.rs` at the lake's first column).

### 2.6 Orbs, death and sounds

Orbs: `Glow` reads `light: 0` as no light part (no orb takes one of the 16), so a `glow` of radius 40 in the orb's colour gives each a pulsing halo under its sprite (red `[255, 70, 70, 200]`, blue `[90, 120, 255, 200]`). `orb_collect` bursts `ex10_orb_health` or `ex10_orb_mana` (sparks on the orb's colour ramp, 14, radial) at the orb; `orb_pickup()` becomes a potion: a cork pop, three rising glugs and a glassy shimmer (0.5 s), under the same ID. Death: `DEATH_SECS` 2.0; `player_death` (stagger, buckle, kneel, slump; five frames over 0.7 s, the lantern on the ground with a small flame, eyes closed) shown through `Shown` from the first dead step, facing fixed at death, held; toss, pose and cast glow cleared; the killing hit plays `Sound::PlayerDied` (`ex10_player_death_sfx`: a breathy falling groan, a knee and a body thump, the lantern's clink; 1.1 s) with the shake, in place of the hurt.

### 2.7 Area titles

All in `run/banner.rs` and the generator: the root's fill and border cleared (`widgets::fill(tree, root, [0; 4])`, as the HUD does); the generator copies the root's Bold beside the Regular and lists it as `serif_bold` in `serif`'s faces; the label at weight 700, size 34 on 42, letter spacing 0.12 em, capped to the viewport (`widgets::capped`); eight dark copies of the label anchored at the centre, a ring of 2-unit offsets at alpha 70 plus one drop at (1, 3) at alpha 120, added first so they paint behind; fade in 0.8 s, hold 2.0 s, fade out 1.2 s (`BANNER_SECS` 4.0), smoothstep eased, on the game clock; a new area mid-fade restarts from the opacity shown, not from 0.

### 2.8 The ceiling seam

level.py puts the family's back tile (the column's `backs` family, or the hollow's cave back) behind a ceiling cell whose underside is open: the cut-away gap then shows the back wall, not the backdrop, and the backdrop cull skips those cells as it skips opaque ones. A mist tile is skipped only where the ground sprite is wholly opaque. The hollow's first back row under the roof takes no stalactite tiles, and its top-row darkening moves down a row's worth. Every other cave and overhang (`rim_west`, the ravine's ledges, the mine) is checked in the shots.

## 3. Steps

Each generator step regenerates with a scratch venv (`$VENV`, `tools/requirements.txt`) and checks: `PATH=$VENV/bin:$PATH python3 examples/01_platformer/tools/generate.py` then `… generate.py --check` → exit 0 (written **gen** below). Every code step ends with `cargo fmt --all -- --check` → exit 0 and `cargo build -p example-01-platformer --locked` → exit 0 (written **build**). Test commands run from the repo root; a filter that matches nothing exits 0, so each check counts its passed tests.

### Step 0: Before shots and a schema 8 save

- **Files:** `examples/01_platformer/tests/game/fixtures/schema8.json` (new); a temporary `examples/01_platformer/tests/m78_fixture.rs`, deleted in the step.
- **Change:** the before shots for items 6 and 9 from the 0.98 build (done before this plan: the banner over the mine and the clearing at 0.17 s and 0.65 s after entry, the ravine's and the hollow's ceilings at zoom 1 and 1.636). Then, before any source change, a temporary test at `be16369` builds a run with every saved ID field in use at or above the player's ID (a killed map bat; a summoned dummy, wisp and two trolls, one holding iron and one with its iron thrown; a missile and a bullet in flight; balls and iron; the black hole; a dropped orb; a crawler touching the player; a bat whose hitbox has hit the player; a beetle and the summoned dummy burning by the player's hand; an explosion and a conduction jump with its chain waiting) and writes its capture in a schema 8 envelope to the fixture.
- **Done-when:** `git status --short examples/01_platformer/tests` → only `?? examples/01_platformer/tests/game/fixtures/`; `jq -c '.data as $d | ($d.player.id) as $t | [$t, $d.next_id, ([$d.player.sources[][1] | select(. >= $t)] | length), ([$d.dummies[].sources[][1] | select(. >= $t)] | length), ([$d.enemies[].sources[][1] | select(. >= $t)] | length), ([$d.enemies[].hit[] | select(. >= $t)] | length), ([$d.enemies[].touches[][0] | select(. >= $t)] | length), ([$d.enemies[].held | select(. != null and . >= $t)] | length), ([$d.throws[] | .body, .thrower | select(. >= $t)] | length), ([$d.cascades.entries[] | .target, .source, .chain[]? | select(. != null and . >= $t)] | length), ([$d.enemies[] | select(.map | not)] | length), ([$d.dummies[] | select(.map | not)] | length), ($d.gone | length)]' examples/01_platformer/tests/game/fixtures/schema8.json` → every count after the first two at least 1 (the player 873).

### Step 1: The kit's immune parts

- **Files:** `crates/tungsten-kit/src/damage/health.rs`, `crates/tungsten-kit/src/damage/pipeline.rs`, `crates/tungsten-kit/src/lib.rs`, `crates/tungsten-kit/src/tests/pipeline.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/example-01-feedback.md` (§6 K15).
- **Change:** first, `CARGO_TARGET_DIR=$PWD/target/criterion-native cargo bench -p tungsten-kit --bench kit --locked -- --save-baseline m78-before damage_resolve`. Then §2.1. `a_hit_whose_every_part_drops_is_ignored_with_no_side_effect` now expects its two immune parts and nothing else; a new test: a mixed hit sends its landed part as a `DamagedPart` and its immune part as an `ImmunePart` with its kinds, source and position, a periodic request flags it, a target in its invulnerability window sends none, a part of 0 none. A ledger row "as additions"; K15 in the feedback's §6, "fixed in M78".
- **Done-when:** `cargo test -p tungsten-kit --locked --lib -- damage::pipeline::tests` → every test passes, the new one among them; `cargo test -p tungsten-kit --locked --doc ImmunePart` → passes; `CARGO_TARGET_DIR=$PWD/target/criterion-native cargo bench -p tungsten-kit --bench kit --locked -- --baseline m78-before damage_resolve` → each of the four rows within ±5 % or "No change in performance detected" or "Change within noise threshold", and each row's median per entity at 10,000 at most twice its median per entity at 1,000; `just api` → a Python line diff of `api/tungsten-kit.txt` against HEAD removes 0 lines and every added line names `ImmunePart`, and `api/tungsten-kit-components.txt` changes only `health`'s `events:` line; `cargo fmt --all -- --check` → exit 0.
- **Moves:** the kit's public surface, an addition.

### Step 2: Frozen wisps and fire

- **Files:** `examples/01_platformer/src/features/combat/systems.rs`, `examples/01_platformer/src/features/combat/mod.rs`, `examples/01_platformer/src/features/level/components.rs` (the lava doc), `examples/01_platformer/tools/level.py`, the generated `assets/` and `tools/outputs.json`, `examples/01_platformer/tests/game/enemies.rs`.
- **Change:** §2.2. Two tests: a frozen wisp hit by a cast fire missile explodes (a `FrostBlasted` burst, its frozen ended, 16 physical taken, alive); a frozen wisp over basin 0 falls and explodes in the lava.
- **Done-when:** **gen**; **build**; `cargo test -p example-01-platformer --locked --test game -- frozen_wisp_` → 2 passed; `cargo test -p example-01-platformer --locked --test game -- the_wisp_is_immune_to_fire` → 1 passed (an unfrozen wisp still takes no fire).

### Step 3: The crawler's lunge

- **Files:** `examples/01_platformer/tools/content/prefabs/crawler.json`, `examples/01_platformer/src/features/enemies/crawler.rs`, `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/tools/actors.py`, the generated `assets/`, `examples/01_platformer/tests/game/enemies.rs`.
- **Change:** §2.3 without the leash. Three tests: from `stalker()`, the first lunge at a still player lands one cold part of 12 (`hits_over`; empty at `be16369`); a player 128 px up within `LUNGE_RANGE` sees no wind-up in 3 s; a crawler dropped onto a still player's head lands its touch (flush from above).
- **Done-when:** **gen**; **build**; `cargo test -p example-01-platformer --locked --test game -- crawler_lunge_` → 3 passed.

### Step 4: Bats, the falls crawler and schema 9

- **Files:** `examples/01_platformer/tools/level.json`, `examples/01_platformer/tools/level.py`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/enemies/crawler.rs`, `examples/01_platformer/src/save.rs` (`MAP_ENEMIES` 18, `SCHEMA` 9, `migrate`), `examples/01_platformer/tests/game/saves.rs`, the generated `assets/`, `examples/01_platformer/README.md`, `examples/01_platformer/tools/README.md` (the 13 enemies).
- **Change:** §2.4, the leash of §2.3 and §2.5's schema 9 with the shift of the five enemies. A test: the schema 8 fixture loads through `SaveSlots` (migrated), its player and every saved ID at or above 873 moved by the constants' shift, its gone map bat gone, the five new enemies alive, each effect source and touch naming the right entity, and a capture after the load reloads equal.
- **Done-when:** **gen** (its bat roof check passes for all seven bats); **build**; `jq '[.enemies[].kind] | group_by(.) | map({(.[0]): length}) | add' examples/01_platformer/tools/level.json` → bat 7, crawler 4, wisp 3, beetle 3, troll 1 (18, `MAP_ENEMIES`); `cargo test -p example-01-platformer --locked --test game -- schema_8_` → 1 passed.

### Step 5: Placed health orbs

- **Files:** `examples/01_platformer/tools/level.json`, `examples/01_platformer/tools/level.py`, `examples/01_platformer/tools/placement.py`, `examples/01_platformer/src/features/enemies/drops.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tests/game/saves.rs`, the generated `assets/`, `examples/01_platformer/README.md`.
- **Change:** §2.5's orbs; `MAP_PICKUPS` 8 joins the shift (13). A test: a fresh run has eight placed health orbs with no `Lifetime`, one taken stays gone through a save and a load and the other seven stay; step 4's fixture test now also finds the eight orbs.
- **Done-when:** **gen**; **build**; `cargo test -p example-01-platformer --locked --test game -- placed_health_orbs_ schema_8_` → 2 passed.

### Step 6: The orbs' glow, potion sound and burst

- **Files:** `examples/01_platformer/tools/content/prefabs/health_orb.json`, `examples/01_platformer/tools/content/prefabs/mana_orb.json`, `examples/01_platformer/src/features/level/components.rs`, `examples/01_platformer/src/features/level/systems.rs`, `examples/01_platformer/src/features/enemies/drops.rs`, `examples/01_platformer/tools/sfx.py`, `examples/01_platformer/tools/generate.py`, the generated `assets/`.
- **Change:** §2.6's orbs.
- **Done-when:** **gen**; **build**; `jq '.components.glow.light' examples/01_platformer/assets/prefabs/health_orb.json` → 0.

### Step 7: Death

- **Files:** `examples/01_platformer/src/features/run/mod.rs`, `examples/01_platformer/src/features/player/{mod.rs,components.rs,systems.rs}`, `examples/01_platformer/src/features/audio/{components.rs,mod.rs}`, `examples/01_platformer/tools/actors.py`, `examples/01_platformer/tools/sfx.py`, `examples/01_platformer/tools/sounds.json`, `examples/01_platformer/tools/content/data/cue_rules/sounds.json`, the generated `assets/`.
- **Change:** §2.6's death.
- **Done-when:** **gen** (`check_frame` passes for the five death frames); **build**; `rg -n 'DEATH_SECS: f32 = 2.0' examples/01_platformer/src/features/run/mod.rs` → one line.

### Step 8: Ambience

- **Files:** `examples/01_platformer/tools/sounds.json`, `examples/01_platformer/tools/README.md`, the generated manifest.
- **Change:** each area loop at half: 0.175, 0.175, 0.2, 0.2, 0.15, 0.175.
- **Done-when:** **gen**; `jq '[to_entries[] | select(.key | startswith("ambience_")) | .value.volume]' examples/01_platformer/tools/sounds.json` → `[0.175, 0.175, 0.2, 0.2, 0.15, 0.175]`.

### Step 9: Area titles

- **Files:** `examples/01_platformer/src/features/run/banner.rs`, `examples/01_platformer/tools/generate.py`, `examples/01_platformer/tools/README.md` (the fonts' paragraph), the generated `assets/fonts/SourceSerif4/SourceSerif4-Bold.ttf`, manifest and `tools/outputs.json`.
- **Change:** §2.7.
- **Done-when:** **gen**; **build**; `cmp assets/fonts/SourceSerif4/static/SourceSerif4-Bold.ttf examples/01_platformer/assets/fonts/SourceSerif4/SourceSerif4-Bold.ttf` → exit 0; `jq -c '.font_families.serif.faces' examples/01_platformer/assets/manifest.json` → `["serif","serif_bold"]`; a capture of the title at weight 700 against one at 400 (a scratch switch) differs in stroke width (the review shows both).

### Step 10: The staff light off at the start

- **Files:** `examples/01_platformer/src/features/player/components.rs`, `examples/01_platformer/src/features/player/mod.rs`, `examples/01_platformer/README.md`, `examples/01_platformer/tools/README.md`.
- **Change:** `on: false` and the docs that name the default (the components' and `spawn`'s, the tools README's "A fresh run lights the lantern", the README's light rows). Loads keep theirs (A21).
- **Done-when:** **build**; `rg -n 'lights the lantern' examples/01_platformer/tools/README.md; test $? -eq 1` → exit 0.

### Step 11: The caves' ceiling seam

- **Files:** `examples/01_platformer/tools/level.py`, `examples/01_platformer/tools/backdrops.py` (only if the hollow's top-row darkening moves), the generated `assets/`.
- **Change:** §2.8.
- **Done-when:** **gen**; the after shots of the ravine's passage and `rim_west` and the hollow's roof at zoom 1.636 show no stripe under the rock (cropped beside the before shots).

### Step 12: The owner's review

- **Files:** none in the tree; captures in the scratchpad.
- **Change:** the shots the owner listed, through the scratch crate: items 1 and 2 in game (orbs glowing, a pickup's burst, the death's frames); item 4's three clips as a frame strip; item 5 each new enemy where it stands; item 6 a title mid-fade and at full over the mine and the clearing beside the before shots; item 7 a new run's first frame and the mine with the light off and on; item 8 the list of the eight orbs (area, column, reason) and two in game; item 9 the ceilings before and after with the cause; item 10 the cause and frame strips of a frozen wisp blown apart by a fire missile and by lava. The report lists what changed per item and each item skipped or narrowed. **The run stops here** until the owner replies.
- **Done-when:** the owner's reply; their notes applied, each with its evidence row.

### Step 13: The test pass

- **Files:** the tests pinning changed values: `examples/01_platformer/tests/game/{player.rs,hazards.rs,presentation.rs,spells.rs,saves.rs,enemies.rs,data.rs,dummy.rs}`.
- **Change:** the death's timing to 2 s and its shown clip, the killing hit's sound, the lit-start tests turning the light on first, the orb counts (dropped orbs only), the orbs' coverage lists (`Glow` rebuilt, `GlowParts` presentation), `dummy + 1 + MAP_ENEMIES + MAP_PICKUPS`, the orb prefabs' lifetime.
- **Done-when:** `cargo test -p example-01-platformer --locked` → passes; `cargo test -p tungsten-kit --locked` → passes; `PATH=$VENV/bin:$PATH just level-check` → passes; `just check` → passes.

### Step 14: Release 0.99

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, the register row, `docs/plans/1.0/{README.md,implementation-plan.md,example-01-feedback.md,roadmap.json}`, `docs/plans/1.0/example-01-design.md` (the death clip beside "no hurt or death frames"), `docs/known-issues.md` if a finding goes there, this plan (moved to `docs/plans/archive/1.0/`).
- **Change:** close workflow §5 and this step's evidence row with `status: done`; the landing decision (amends `D-194`, and `D-205`'s lit start); then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.99.0` once, every release check once after it with `just level-check` and `just api-check`, then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report; `just release-preflight 0.99.0 --repo JosephGibson/Tungsten --message 'Update 0.99: M78 example 01 quick fixes'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: 01 level and title pass (§3.1b) in this stop's shape, plan and run in one session with a review stop before the test pass.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The orbs' glow: a light or a halo | A halo only (`light: 0`): 15 map lights, the lantern and the moon already meet the cap of 16, which drops the lantern first (P3) |
| Q2 | A placed orb's heal against a hazard's 34 | The drop's 20; the owner tunes it at the review |
| Q3 | One potion sound or one per kind | One, under the orb pickup's ID |
| Q4 | I9's fade and appearing sound | Left to (f): neither comes free with this stop's code |
| Q5 | A death in the air | The clip plays at once wherever the player dies |
| Q6 | Lava's touch on frozen things | 34 fire, no knockback, only `status.frozen`; unfrozen enemies untouched, as today |
| Q7 | Cold on a burning cold-immune target (the same gap) | Unchanged: only fire on frozen reads `ImmunePart`; recorded as a follow-up |
| Q8 | The lunge's fix: a wider touch or a windowed leap hitbox | The wider touch: the cause is the touch's width, and every other enemy's shape reaches past its body |
| Q9 | The aqueduct's crawlers with the light off at the start | Unchanged; the review reports a light-off crossing of the aqueduct |
| Q10 | Does a frozen wisp's own blast kill it | No: the blast thaws it and deals 16 of its 40, as for every frozen creature; the owner decides at the review |
| Q11 | The fixture's size | One run's capture without the ball pit, under 100 KB |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`), and the run's review stop (step 12) is the owner's.

## 5. Decisions expected

- 207: 01 quick fixes landed: the kit's `ImmunePart` (K15), the placed orbs and schema 9 shifting the new map objects' IDs, the staff light off at a run's start, a death clip on the courier; amends `D-194` and `D-205` (Q15's lit start).

## 6. Invariants

- **May move:** the kit's public API snapshot (step 1, an addition); 01's generated assets (steps 2–11); 01's save schema (8 → 9, step 4, its shift grown in step 5).
- **Must not move:** the determinism hash, the pinned containment hash, the row digests (no bench row uses a resistance), `gpu-visual.png`, the post and transition regressions, the template and 01's `src/shell/` and `shell.json` (`just repo-check`), the kit's other events and traces.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it (2 depends on 1; 5 on 4). Schema 9 and its `migrate` land in step 4 with the first new map objects, so a skipped step 5 leaves a consistent shift of 5, and a skipped step 4 skips 5.
- Step 1's criterion rows read outside ±5 % twice: the step stops and the review names it.
- The schema 8 fixture (step 0) fails to load once migrated: step 4 stops, since the migration is what it tests, and steps 4 and 5 are skipped together.
- A file outside §files to touch needs a change: recorded, and done only if the review takes it.
- Step 12 is a hard stop: nothing after it runs before the owner's reply.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 13, release | The kit's criterion pair in step 1; `just level-check`, `just check` in 13; the release checks once after the cut, with `just level-check`, `just api-check` and `just smoke` |
| 3 Invariants | 13, release | |
| 4 Decisions | release | 207 with its index row before the docs that cite it |
| 5 API | 1 | `just api`, the ledger row "as additions", rustdoc on every new item |
| 6 Template and guide | n/a | No template change (§3.1b owns the stops' only one) |
| 7 Routes | n/a | No new file a task opens; the fixture sits in `tests/game/` |
| 8 Design | n/a | No architecture change; the example design's death rule gains a line (release) |
| 9 Records | release | K15 in the feedback's §6; the register row; the README's four owned Now fields |
| 10 Close | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-22, reading `be16369` with this plan uncommitted (§9 and the evidence log cut); effort xhigh, context 872,000, output cap 4,000, 347.9 s, 1,456,113 tokens in (1,345,920 cached), 9,456 out, 22 commands, 342.8 KB of output (largest 54.1 KB), usage 5h 0% → 29%, week 0% → 5%. Three findings, all accepted; the first moved schema 9 into step 4 and the third fixed a done-when, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] A failed migration step leaves the changed map using schema 8: step 4's enemies before step 5's `migrate` give a 0.98 save's player ID to a new enemy | `examples/01_platformer/src/features/level/mod.rs:146-152` (IDs in layer order); `examples/01_platformer/src/save.rs:750` (the saved ID put on the player) | accepted | Schema 9 and its `migrate` move into step 4, with the fixture's test; step 5 adds the orbs to the shift (§2.5, steps 4–5, §7) |
| 2 | [MEDIUM] The wider crawler touch still misses landings from above: its bottom equals the body's | `examples/01_platformer/tools/content/prefabs/crawler.json:14-27` (body to y 61); `crates/tungsten-kit/src/motion/slide.rs:70-75`; `crates/tungsten-kit/src/overlap/grid.rs:283-286` | accepted | The touch reaches 2 px above and below the body; a third test drops a crawler on the player's head (§2.3, step 3) |
| 3 | [MEDIUM] Step 4's expected enemy counts cannot match its additions | `examples/01_platformer/tools/level.json:9713-9791` (three bats today) | accepted | Seven bats, eighteen enemies (step 4) |

Round 2: the same reviewer on the revised plan, 2026-10-22, with a focus note of round 1's changes (none rejected); effort xhigh, 169.0 s, 482,401 tokens in (414,336 cached), 4,826 out, 14 commands, 201.7 KB of output (largest 39 KB), usage 5h 29% → 43%, week 5% → 7%. Two findings, both accepted, both gaps round 1's fix left. There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] The migration fixture can pass without exercising several ID rewrites: `throws`, `cascades.entries`, enemy `hit` and `held` empty | The first fixture (`jq`: hits, held, throws and cascade entries empty); `examples/01_platformer/src/save.rs:921`, `:925` (holds and throws reconnected on load) | accepted | The fixture recaptured at `be16369` with every path populated at or above 873 (two summoned trolls holding and throwing iron, a bat's hit, a dummy's source, an explosion and a jump waiting); step 0's done-when counts each path |
| 2.2 | [MEDIUM] The missing-fixture fallback still belongs to step 5, and §6 still names step 5 for the schema | §7 and §6 as revised in round 1 | accepted | The fallback replaced: the fixture exists, and a migration that fails on it stops step 4 with step 5; §6 names step 4 |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 0 | 2026-10-22 | done | Before shots from the 0.98 build: the banner a `#15171d` box with a light border around Regular text, mid-fade at frame 111 and full at 140 (entry at 101), over the mine and the clearing; a lighter stripe 6–12 px under the ravine's `rim_west` and the hollow's roof at zoom 1.636. Fixture (recaptured after critique 2.1): 33,323 bytes; the done-when's `jq` → `[873,960,2,1,1,1,1,1,2,6,3,1,1]`; the temporary test deleted, `git status --short examples/01_platformer/tests` → `?? examples/01_platformer/tests/game/fixtures/` | `<scratchpad>/shots/before/`, `examples/01_platformer/tests/game/fixtures/schema8.json` |
| 1 | 2026-10-22 | done | `cargo test -p tungsten-kit --locked --lib -- damage::pipeline::tests` → 28 passed, the new `an_immune_part_goes_out_beside_the_landed_ones_and_only_when_struck` and the all-dropped test (renamed `…_ignored_with_its_immune_parts_alone`) among them; `--doc ImmunePart` → 1 passed. Criterion against `m78-before` (baseline 239.36 µs, 2.4778 ms, 588.63 µs, 6.0775 ms), the quiet rerun: `damage_resolve` 1,000 241.66 µs +0.91 % (noise), 10,000 2.4749 ms −0.12 % (no change); `_typed` 1,000 582.37 µs −1.19 % (noise), 10,000 6.0734 ms −0.07 % (no change); per entity at 10,000 over 1,000: 1.02 and 1.04. The first after-sitting, started at load 4.55 right after a build, read +2.25 % on `damage_resolve/1000`. `just api`: `api/tungsten-kit.txt` +60 −0, every line `ImmunePart`; `api/tungsten-kit-components.txt` only `health`'s `events:` line. Ledger row and K15 written; `cargo fmt --all -- --check` → 0 | `<scratchpad>/bench-before.log`, `bench-after2.log`, `kit-test.log` |
| 2 | 2026-10-22 | done | **gen** → `OK: 3999 owned outputs match` (new `ex10_lava_frost_0`, `_1`); **build** 0; fmt 0; `cargo test … --test game -- frozen_wisp_` → 2 passed (`a_frozen_wisp_blows_apart_under_a_fire_missile`, `…_in_the_lava`: the frost blast bursts, the ice ends, health 40 → 24, the blast's 16 physical alone); with `combat_reactions`' `ImmunePart` branch disabled both fail, restored and `cmp`-checked; `-- the_wisp_is_immune_to_fire` → 1 passed. The missile test first placed the wisp at x 900, past the missile's arc (it fell at x ≈ 544, `BLAST_RADIUS` 72); moved to x 520 | `<scratchpad>/t2.log`, `t2neg.log` |
| 3 | 2026-10-22 | done | The crawler's `contact_damage` `aabb [28, 12]` at `[32, 51]`; `LUNGE_RISE` 64; clips `crawler_windup` (coil 0.35/0.7/1.0/1.0 + tremble, 4 × 100 ms), `crawler_lunge` (stretch 0.6/1.0/1.0, 60/80/400 ms), `crawler_recover` (squash 1.0/0.55/0.2, 100/120/140 ms), feet on row 60 as `WALKER_FEET` requires (the leap's lifted feet failed it first); walk and scared frames byte-identical to HEAD (`cmp`); the tilemap's tile IDs renumbered by the twelve new sprites. **gen** → `OK: 4015 owned outputs match`; **build** 0; fmt 0; `-- crawler_lunge_` → 3 passed; reverting the touch shape fails the two touch tests, reverting the gate fails the third | `<scratchpad>/t3.log`, `t3neg.log`, `t3neg2.log` |
| 4 | 2026-10-22 | done | Map: aqueduct bats at x 98 and 141 under the channel (over the climb, by the east lift), ravine bats at 189 (under `rv_ledge` 4) and 198 (under the east ledge at row 57), the falls crawler at 181 on the pool bed with `leash` 5 (`enemy.leash` 320 px); `Enemy.leash` (serde default), the crawler's patrol turning past it. Schema 9: `SHIFT_9` = 18 − 13 + `MAP_PICKUPS` (0) = 5; `migrate` 7 → 8 → 9 and 8 → 9 through `shift_ids`. `jq` → `{"bat":7,"beetle":3,"crawler":4,"troll":1,"wisp":3}`; **gen** → `OK: 4015 owned outputs match` (the roof check passing for seven bats); **build** 0; fmt 0; `-- schema_8_` → 1 passed (`schema_8_saves_migrate_with_the_new_map_objects_ids_moved`: the 23 ID fields of the fixture moved by 5 and no other number; loaded, the player 878, the five new enemies live, bat 863 gone, the recapture equal to the migrated save within a hundredth on floats, a held block having moved 0.0002 px); dropping the throws' `body` rewrite fails it. The first run of the test failed on an unregenerated map (13 enemies), then on two comparison details of the test itself | `<scratchpad>/t4.log`, `t4neg.log` |
| 5 | 2026-10-22 | done | `pickups` (eight, each `float` 0.6 over its support: the pit's top shelf, the channel past the crawlers, the low route's rubble, the falls' foot, past the lava lake, before the lava shelf, the mine's second bench, the bridge's head), `placement.prepare`'s support check, the `pickups` layer last; `drops::start` (`PlacedOrbs`, `Lifetime` removed), `drops::gone` in the save's `gone`; `MAP_PICKUPS` 8, `SHIFT_9` 13. **gen** → `OK: 4015 owned outputs match`; **build** 0; fmt 0; repo-check 0; `-- placed_health_orbs_ schema_8_` → 2 passed (eight orbs, none with a `Lifetime`; one taken is in `gone` and stays gone through a load, the seven there; the fixture now finds the eight) | `<scratchpad>/t5.log` |
| 6 | 2026-10-22 | done | `Glow`'s `light` 0 spawns no light (`GlowParts.light` an `Option`); the orbs' `glow` a halo of radius 64, red `[255,70,70,255]` and blue `[90,120,255,255]`, `light` 0 (radius 34 at alpha 200 hid behind the sprite in the review's first shots); `orb_collect` bursts `ex10_orb_health` or `ex10_orb_mana` (26 sparks on the kind's ramp, scale 0.14–0.3) at the orb's centre; `orb_pickup()` a potion (a cork's pop, three gulps at 0.075/0.165/0.255 s, a shimmer from 0.35 s; 0.62 s, peak 26,028). `jq '.components.glow.light'` → 0; **gen** → `OK: 4017 owned outputs match`; **build** 0; fmt 0 | `examples/01_platformer/assets/sounds/orb_pickup.wav` |
| 7 | 2026-10-22 | done | `player_death` on today's courier: stagger, buckle, kneel, slump, rest (90/110/140/160/260 ms), the lantern set down and its flame dying to an ember (`Pose.flame`, `lantern(…, flame)`), every frame through `check_frame`; shown from the first dead step through `Shown`, held, facing as at death (`Moves.dead_left`), toss, pose and cast glow cleared; the killing hit plays `Sound::PlayerDied` (cue `player_died`: shake 0.8, `ex10_player_death_sfx`, a sinking groan, a knee's and a body's thump, the lantern's clink, its flame guttering; 1.1 s) in place of the hurt; `DEATH_SECS` 2.0. **gen** (with step 8) → `OK: 4080 owned outputs match`, 62 new death outputs and no other player sprite changed; **build** 0; `rg 'DEATH_SECS: f32 = 2.0'` → one line | `examples/01_platformer/assets/sprites/player_death_*.png`, `sounds/player_death.wav` |
| 8 | 2026-10-22 | done | `jq` → `[0.175,0.175,0.2,0.2,0.15,0.175]` (clearing, aqueduct, ravine, hollow, mine, bridge), the manifest's copy alike; the tools README's "0.15 to 0.2"; **gen** → `OK: 4080 owned outputs match` | `examples/01_platformer/tools/sounds.json` |
| 9 | 2026-10-22 | done | The banner's root cleared (`widgets::fill`), its title Source Serif 4 Bold at weight 700, 34 on 42, letter spacing 0.12 em, capped at 52; a ring of eight copies 2 units out at alpha 70 and a drop at (1, 3) at alpha 130 behind it; eased fades of 0.8 s in and 1.2 s out over `BANNER_SECS` 4.0, and a name arriving over a shown one waits for its 0.25 s fade out. The generator copies `assets/fonts/SourceSerif4/static/SourceSerif4-Bold.ttf` (`cmp` equal) as `serif_bold`; `jq` → `["serif","serif_bold"]`. **gen** → `OK: 4081 owned outputs match`; **build** 0; fmt 0; repo-check 0; `-- a_new_area_replaces_the_banner` → 1 passed. Captures: mid-fade (frame 325, 24 frames after entry) and full (391) over the mine and the clearing; a weight-400 twin draws thinner strokes, so the Bold face is the one drawn. A first take restarted the fade from the shown opacity, which skipped the fade in on a quick crossing; replaced by the swap | `<scratchpad>/shots/after/banner_*.png` |
| 10 | 2026-10-22 | done | `Lantern::default()` `on: false`; the docs naming the default (the component's, `spawn`'s, the tools README's, the README's light and schema lines). A load keeps its saved light (`save.rs` restore reads `save.lantern`, schema 7 on), so a migrated save does not rely on the default. **build** 0; `rg 'lights the lantern' tools/README.md` → no match (exit 1) | — |
| 11 | 2026-10-22 | done | Cause: a ceiling's lowest tile (`basalt_under_*`, `wet_rock_4_*`) is cut away 0–12 px at its bottom and no back tile sat behind a ground cell, so the unlit backdrop showed through, undimmed; in the ravine a mist band also started on its densest row there. Fix in `level.py`: a plain back tile (`cave_back_rock_<x % 4>`, `<family>_0_<x % 2>`) behind each roof's, ceiling's and ledge's lowest cell over a back, drawn outside the random stream, and a mist tile skipped only where the terrain's sprite is wholly opaque. Against the step's backup: background +143 cells (cols 158–289), decorations +72, none changed or removed, terrain and collision equal. **gen** → `OK: 4081 owned outputs match`; the after shots of `rim_west`, the east wall's passage, the hollow's roof and the mine's `cut_top` show no stripe | `<scratchpad>/shots/after/seam_*.png` |
| 12 | 2026-10-22 | done | Before the stop: **gen** → `OK: 4081 owned outputs match`; fmt 0; **build** 0; `cargo test -p example-01-platformer --locked --no-run` → 0; repo-check 0. Captures for every item through the scratch crate (`target/e01q-shots/`, its scenes: a death, a frozen wisp under a cast missile and over lava with the map's enemies cleared, the orbs' glow and pickup): the death in game, the lunge's three clips, the five new enemies marked where they stand, the titles before and after, a new run's start and the mine and the hollow with the light off and on, two placed orbs, the ceilings before and after, both wisp strips, on one private review page. The owner's reply, 2026-10-22: "Looks good go ahead and finish this session": no notes, so every default stands (Q2's heal of 20, Q9's crawlers, Q10's surviving wisp, the hollow's ambient) | `<scratchpad>/shots/review/` |
| 13 | 2026-10-22 | done | The first full run: 13 of 01's 231 failed. Twelve pinned changed values: the dummy's ID rule (`+ MAP_PICKUPS`), the dropped orbs' count (the eight placed left out through `PlacedOrbs`), the killing tick's death sound, the death's 2 s (`DEATH_SECS`, renamed `the_dead_fall_for_two_seconds_before_game_over`) and its clip at once, the five lit-start tests (lit first; the spells test now pins the dark start), the schema 7 rotation writing 9, and the coverage lists (the orbs' `Glow` and `GlowParts`, then the lava basins' `Children` from step 2's frost child). One was a regression: `a_thrown_frozen_enemy_flies_the_throws_arc_and_hurts_at_its_kinds_mass` read the first hit's source as the crawler, not the troll, since step 3's wider touch let the frozen crawler's own chill land first. Fixed with `HeldTouch` (a frozen enemy's touch set aside in `enemy_hold` until the thaw, removed at a death, its touches saved from either; `enemies/{components,systems,mod}.rs`, `save.rs`, 01's README, outside this step's list and inside the plan's); with the swap removed the throw test fails again (restored, `cmp`). `just check`'s first run stopped at clippy on steps 1's and 10's code (`needless_pass_by_value` on `Landing`, now `Copy`; `type_complexity` in the kit test's helper, now an alias; `derivable_impls` on `Lantern`'s `Default`, now derived). Then: `cargo test -p example-01-platformer --locked` → 230 passed, 1 ignored; `cargo test -p tungsten-kit --locked` → 644 passed, doc 107 passed; `PATH=$VENV/bin:$PATH just level-check` → 15 tests OK; `cargo clippy --workspace --all-targets --locked -- -D warnings` → 0; `just check` → exit 0 | `<scratchpad>/t13a.log`, `t13c.log`, `t13neg.log`, `t13kit.log`, `t13level.log`, `t13check2.log` |
| 14 | 2026-10-22 | done | Records before the cut: `D-207` with its index row, the marker lines and rows of `D-194` and `D-205`; the ledger row's decision; the feedback's §3.1a landing and K15's other half (cold on the burning, (f)'s); the example design's death clip and the quick fixes' notes; the known issues' dark light; `CHANGELOG.md`'s `[Unreleased]`; DESIGN's status line (an event for each part an immunity drops); the register row; the roadmap's note; this plan archived. The cut, the release checks and the preflight run after the archive, so their output is in the session's report | — |

## Follow-ups

- Cold on a burning cold-immune target sends no steam (Q7): the same gap's other half, for (f).
- A dark light still takes one of the 16 slots (`light_extract.rs:13-31`): with the light off by default the lantern's dark light competes for a slot every run (P3's neighbour).
- The ravine's and the hollow's backs meet at column 211 on a hard vertical edge (the mist band's end, unlit wet rock against lit cave back): the hollow's entrance, which 0.100's cave mouth rebuilds.
