# Example 01: a sandbox on the template — design

- **status:** draft
- **goal:** Rebuild example 01 from scratch as a sandbox on the template, in four stops before the feature gate (`D-194`): (a) rebuild, (b) elements and weapons, (c) bestiary, (d) new level. The kit's platformer controller, path follower and combat API meet a real game's hardest cases before W4b freezes them, and C1 gains 01 as a stable benchmark row.
- **non-goals:** An objective, a win or a boss: 01 stays a sandbox, distinct from the [acceptance game](acceptance-game.md)'s arena auto-shooter with XP, level-ups, waves and a boss. The example fixtures (`D-193`). Decisions for (b), (c) and (d), which each stop's milestone writes when it lands. A new engine snapshot API. Stereo panning, looping kit cues that keep their voice, kit line-of-sight and a same-step order that survives a load ([backlog](backlog-1.x.md) rows). Options of 01's own, since the shell's option set is fixed. Slopes and ramps, since the physics engine has only axis-aligned boxes and circles. Water physics.
- **files to touch:** `examples/01_platformer/` whole: `src/` with `src/shell/` copied from the template, `tests/`, `tools/` (the art and sound generators, `level.json`), `assets/`, `tungsten.json`, `input.json`, `README.md`. (a) also: `templates/basic/src/shell/`, `tests/game/` and `AGENTS.md` (the pending autosave and its seam name); the kit's `damage/effects.rs` (a stat-restoring payload, suppression), `motion/platformer.rs`, `motion/path.rs` and `foundation/rng.rs` (exact saves), with their tests, recipes and `api/` snapshots; `scripts/check-repo.py` and `scripts/test-check-repo.py` (the shell-copy check); `docs/LLM_INDEX.md` (01's row); `docs/known-issues.md` (the `gate_step` entry). (b): the kit's `damage/effects.rs` (an effect's source in saves). (c): the kit's `overlap/hit.rs` and `intent/state_machine.rs` (exact saves). Every stop: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `CHANGELOG.md` and [backlog](backlog-1.x.md) rows for the kit gaps it defers.
- **ordered steps:** (1) 01 (a) rebuild, level C, XL. (2) 01 (b) elements and weapons, level C, XL. (3) 01 (c) bestiary, level C, XL. (4) 01 (d) new level, level B, XL. Each is a register row with its own milestone plan, which starts from its section below, tunes the starting values and draws its art-review sheet.
- **done-when:** Each stop's done-when below passes in its release, and the owner has played each stop through. At the feature gate 01 copies the template's shell unchanged, moves its player on the kit's platformer controller and its platforms on the path follower, and has a frame-time capture recorded for C1, unless the gate has re-tiered (d) to 1.x (`D-194`).

## Context digest

- `D-194` places the overhaul in Phase 6, after the example fixtures (`D-193`) and before the Phase 6 QA pass, as four stops the owner tiers Must, so the feature gate waits for them. 01 (a) carries W12b's Must scope and W1 M6 ([w12](w12-template.md) step 3, [w01](w01-ui-text-suite.md) §11). Fallback: (d) is content with no public API, so if Phase 6 runs long the feature gate may re-tier it to 1.x without harming the freeze.
- Why before the freeze: `D-158` moves the platformer controller and the path follower to 1.x unless 01 uses them by the freeze, and the kit's combat API should meet a real game's hardest cases before W4b locks it. C1 then takes 01 as a stable benchmark row.
- 01 today: a platformer whose player is a dynamic body with its own input and ground detection (`D-158`), on a level of 184 × 50 tiles at 64 px in four named areas that `tools/generate.py` emits with its art; 106 Rust tests; screen text for its controls, status and death; no saves. The fixtures stop takes its lighting fixture first and (a) drops its two display-variable checks, so 01 keeps no `TUNGSTEN_*` hook.
- The template (`templates/basic`, `D-181` as `D-187` extends it): a shell every game copies unchanged and a sample it deletes, joined through a seam. The shell owns the title, pause, game over, eleven save slots, tabbed settings, keybindings, toasts, the difficulty picker and the gameplay options, and plays its own sounds by fixed ID.
- The kit covers 01 except for the gaps this file names; each becomes a level C kit step in the stop that first needs it, or a backlog row.
- Numbers marked "starting value" are where tuning starts; each stop's plan sets them. No decision exists yet for (b), (c) or (d): each stop's milestone writes its own, starting from this file, which may change while the stops are planned.

## Shared rules for all four stops

- **Same assets.** Sprites, sounds, animations, particles and shaders are kept. New art (spell and gun icons, enemy sprites, the lightning bolt), every new sound and every music track come from 01's generators in `tools/` (art and sound), in the same style, so no licence is involved.
- **The shell** is copied from `templates/basic/src/shell/` byte for byte and never changed in 01; a repo-check enforces it. A shell change 01 needs goes into the template. The shell plays its sounds by fixed manifest ID (`shell/theme.rs`): `ui_move`, `ui_select` and `ui_back`, the title's `music_title`, and game over's `game_over` sting and `music_over`, so 01's sound generator writes each under that ID.
- **Kit first.** Each stop's plan maps every 01 system to a kit module, or records why it stays game code. A kit gap becomes a level C step with an API critique, or a 1.x backlog row. It never becomes a workaround in 01.
- **The existing tests are the spec.** Each of 01's 106 tests is ported to the harness, or (a)'s plan records why it was retired.
- **Game layout.** 01 follows `templates/basic/AGENTS.md` and has no `TUNGSTEN_*` environment hooks. 01's README maps each mechanic to its kit module and data file, and (a) adds its `docs/LLM_INDEX.md` row. Today 01 also reads the root `input.json` and the root manifest (the fonts, `music_main`, `sfx_blip`, the `damage_flash` material); on its own folder it carries what it uses, as the template does.
- **Sound.** Each stop generates the sounds for what it adds.
  - One-shots are triggered from data through kit sound cues, with a voice limit per sound (M68, `D-179`).
  - Loops (ambience, the wisp's hum, burning) are played by 01's code through the engine's audio commands, which return the `Voice`, so 01 can set a loop's volume by distance. Kit cues play one-shots only and keep no voice (`react/cues.rs`); a looping cue that keeps its voice is a 1.x backlog row.
  - Stereo panning isn't available; it's a 1.x backlog row.
- **Saves** are a full snapshot of gameplay state, not presentation, including the run's random-number state.
  - Identity: every saved entity carries a stable save ID, assigned at spawn from a saved counter. Saves refer to other entities (an effect's source, a held object, a target) by save ID and remap it on load. Nothing orders by entity ID, which the ECS reuses from a free list (`core/ecs/entity.rs`).
  - Saves are written by hand in `save.rs` on the kit types' serde derives, with no new engine snapshot API.
  - `capture()` returns `None` only while the player is dead; the shell's curtains don't block it. In the template, a requested autosave (the run's first, an area's, the exit's) stays pending until a capture succeeds, instead of being dropped (`D-194`). Today `frame()` in `templates/basic/src/shell/mod.rs` clears `first_save` once, even when that capture fails. This is part of (a)'s template change.
  - A save round-trips: capture, then load, then capture again gives the same result.
  - Coverage: a test lists every component type on live gameplay entities and fails if one is neither saved nor on an explicit presentation-only list, so new state can't be left out silently. It works at the type level; missing fields are caught by the divergence test and the kit-gap tests below.
  - Known kit gaps for exact saves (checked 2026-10-20):
    - `Hitbox` saves no record of the targets it has already hit, so a loaded attack starts a fresh activation and can hit the same target again (`crates/tungsten-kit/src/overlap/hit.rs`, its scene-format doc). A piercing `Projectile` checks the same list, so a loaded bolt can too (`overlap/projectile.rs`).
    - A loaded running effect, an `EffectInstance`, loses its source (`damage/effects.rs`, whose deserialization sets `source: None`), so its payloads and `on_expire` then run with none.
    - `StateMachine` has no serde support and keeps its elapsed time and pending transition private (`intent/state_machine.rs`); one rebuilt with `new` runs its state's enter hook again on its first visit.
    - `PlatformerController` and `PathFollower` have no serde support and keep their runtime state private: the controller's velocity (where its run speed lands), coyote and jump-buffer timers and the startup step (`motion/platformer.rs`, which skips its first movement step after construction), and time along the path (`motion/path.rs`, whose scene `points` are offsets from the spawn position, so a follower loaded mid-path keeps its path's origin).

    Found while checking this file (2026-10-20): `KitRng` has no serde support either (`foundation/rng.rs`; `D-173` left it open), and saves hold the run's random-number state from (a) on.

    Each becomes a level C kit step in the first stop that saves that state: (a) for the controller, the path follower and `KitRng`, (b) for effect sources, (c) for hitboxes and state machines. Each comes with exact tests: saving mid-jump, mid-platform-cycle, after an attack has already hit, and while a transition is pending.
  - Divergence: a load continues the game equivalently, not identically. Entity IDs differ after a load, and the ECS's iteration order and the kit's same-step damage order (sorted by entity ID, `damage/pipeline.rs`; effects and hitboxes too) follow them, as does the physics step's unsaved state: warm-start impulses, sleep and the last step's sensor pairs (`core/physics/step/`). Spatial queries find nothing until the first step after a load (`core/physics/query.rs`).
    - So a run saved mid-play, loaded and driven by the same scripted inputs for a few seconds is compared with the uninterrupted run for: the same live entities by save ID; the same states (alive or dead, statuses, state-machine states, effects present, the cascade queue's contents); and health, mana, cooldowns and positions within tolerances the plan states.
    - The parts that must be exact are tested exactly on their own: the random-number state, the cascade queue, hitbox hit lists, effect sources, state-machine elapsed time and pending transitions, and controller and path-follower state.
    - Each scenario also runs from a fresh process and from a world that has spawned and despawned many entities first.
    - A same-step order that survives a load, so a load replays identically, is a 1.x backlog row.
  - Each stop that adds state bumps the schema and writes `migrate`.
  - (a)'s plan measures save size and time with the ball pit full and sets a cap.
- **Cascades** (freezes, shatters, explosions, steam bursts, conduction jumps) are capped per fixed step, not per rendered frame, so outcomes don't depend on frame rate. Today's per-frame `EXTINGUISH_BURSTS_PER_FRAME` becomes per step too; `SMALL_BALL_BURSTS_PER_FRAME` already counts per step, despite its name.
  - Work over the cap is deferred, never discarded. It waits in a queue in a stable order (distance from the cascade's origin, then save ID) and runs on the next steps, so a big chain rolls through a pile as a wave.
  - The queue is gameplay state, so saves include it.
  - The plan sets the queue's bound, large enough for the biggest chain a full ball pit can make. Work past the bound is dropped with a logged warning, and a test shows that chain stays under it.
- **Options.** 01 follows the shell's existing gameplay options, Damage numbers and Screen shake, and adds none of its own, since the shell's option set is fixed.

## Assets

Every new asset follows the house style in 01's `tools/README.md` and its generator:

- drawn by code (Pillow), using only `palette.json`'s colours;
- 64 × 64 tiles and frames (the troll's are 128 × 128, an exception beside the black hole's 96-pixel sprites), 1,024-pixel wrapping backdrops, and large scenery cut into 64-pixel tiles (`tools/scenery.py`);
- lit sprites get emissive (`_e`) and normal (`_n`) maps derived from their silhouettes; terrain uses relief normals; fire-like things (wisps, flames) are drawn unlit with no maps, as the fireball is (today's `fire_*` flames carry maps they don't use);
- cosmetic randomness seeded per asset name, `outputs.json` listing what the generator owns, and `just level-check` verifying every byte;
- review contact sheets kept outside `assets/`.

Each stop draws the assets its own features need.

**Element colours.** Each element's particles, status tints, damage numbers, icon border and spell-bar slot all use its colour:

| Element | Colour |
| --- | --- |
| Physical | Bone white |
| Fire | Ember orange |
| Cold | Ice cyan |
| Shock | Electric violet |
| Bleed | Dark maroon |
| Mana | Indigo; from (h) a deep bright blue (`И Й З Ю`) for the summons' tiles, the mana orb and the mana bar's liquid |
| Gravity (the force push and the black hole, from (f)) | Deep purple, from magenta toward indigo, apart from shock's violet; from (h) the push's and the hole's icon tiles |

Saturated yellow is reserved for crits. The palette gains violet, maroon and indigo shades, and in (f) a gravity ramp.

**Readability.** The sky stays dark (mean luminance below 25), so enemies get:

- a moonlit rim on their right side, like the cliffs;
- glowing detail in their emissive maps;
- distinct silhouettes: a round flame (wisp), low and wide (crawler), a dome (beetle), spiky wings (bat), tall and hunched (troll), small, big-eared and pot-bellied (goblin, from (g)).

**Status effects** are generic overlays and tints, never art drawn for each enemy. They come in three size classes: small (wisp, bat), medium (crawler, beetle, player, dummy) and large (troll).

| Status | Shown as |
| --- | --- |
| Frozen | An ice-block overlay and a cold tint over the target's held frame; from (f) the cube flickers to a cracked one over its last 0.75 s, and the thaw ends in glints and a crack |
| Burning | The existing `fire_*` flames, attached; from (f) larger, on three spots over a warm glow, with embers and a flickering ember tint |
| Shocked | A violet crackle overlay and a flickering tint |
| Chilled | A frost tint and the existing snowflakes (`ice_crystal`); from (f) the frost and a second slow deepen as the cold bucket fills toward the freeze |
| Bleeding | Drip particles; from (f) a spurt as it starts, and denser, larger, brighter drips |
| Stunned | A spark halo |
| Hurt, the courier (from (f)) | A red tint for 0.12 s after a blow, since its lit sprite shows no hit flash |

Getting hit uses the kit's hit flash. Deaths use their element's burst (a shatter, a burn-out, a steam pop or a plain poof), so no enemy needs hurt or death frames. The courier has one death clip since the quick fixes (`D-207`).

**Animation.** Each enemy is one rig posed per frame, like the courier, the player's rig (`tools/actors.py`), with its feet anchored to a fixed row and no raster rotated.

| Who | Frame | Clips |
| --- | --- | --- |
| Wisp | 64, self-lit | Float loop, alert flare |
| Crawler | 64 | Walk, lunge |
| Beetle | 64 | Walk, paw, charge |
| Bat | 64 | Roost, fly, dive wind-up, dive |
| Troll | 128 | Idle, walk, slam wind-up, slam, pick up, throw; from (g) a swipe's wind-up and swipe; the held object is its own sprite on the kit's hierarchy |
| Goblin, from (g) | 64 | Idle, walk, wind-up with a rock or a lit bomb, throw, the stab's wind-up and stab, hop |
| Player, in (a) | 64 | A toss clip; an arm and gun drawn at 8 aim directions instead of rotated; the lantern brightens on a cast through its emissive map |
| Player, from (h) | 96 | The wizard: idle, walk, jump, fall, land, air jump, hurt, a death that ends on the ground, the toss, bolt, push and conjure gestures, and the staff's and the pistol's poses at 5 aim directions; his staff's amber orb is the staff light, never emissive |

Today the player draws unlit through `damage_flash` and never reads its maps, since the fixtures stop removed the lighting fixture (`src/extract.rs`), so the lantern's brightening puts the player on the lit path in (a).

**Icons, items and effects:**

- icons for the 10 spells and the gun, shown at whole-number scale in the shell's theme, with the element's colour as the border; from (h) one family on 36 px tiles, the element's colour their bevelled frame;
- a bowling ball distinct from today's ball (darker, with finger holes and an iron sheen); health orbs (red) and mana orbs (indigo); the training dummy;
- 6–8 lightning segment sprites, combined differently on each strike;
- particles, mostly as new JSON settings over today's dust, spark, droplet and mote sprites: a muzzle flash and tracer, ricochet sparks, steam puffs, the force push (the shock ring re-tinted), the troll's slam, bleed drips and orb sparkle.

**Level tile families for (d)**, on the existing terrain rules (a 64-pixel repeat, shared edge columns, decks 23 px deep):

| Area | Reuses | New |
| --- | --- | --- |
| Lantern clearing | Turf, the woodland backdrop, oak, lamp posts | Ball-pit walls |
| Broken aqueduct | Masonry, arches | A channel and an animated water strip (decoration only, no water physics) |
| Waterfall ravine | Waterfall, cliffs | Ceiling underside tiles |
| Ember hollow | Fire hazards | Basalt with emissive ember cracks; a cave backdrop |
| Iron mine | Timber supports | Iron-plate walls in the iron brick's colours; rails and carts as decoration |
| Troll bridge | Bridge, rope, weathered bridge | A river strip; bones and rubble |

Also needed:

- an underside (ceiling) edge family, since today's terrain defines only the top lip;
- edge tiles where families meet, so area borders show no seam;
- backdrops that make outdoors and indoors obvious: outdoor areas keep the sky strips, and caves get dark rock backdrops with ember-lit and timber variants.

**Audio files.** Sound effects and music are 16-bit mono WAV, synthesized with the standard library, following the precedent of the template's `tools/sounds.py` (`D-187`).

- `--check` compares every byte, each loop's length and its seam.
- Music loops run at 22,050 Hz and last about 30 s, roughly 1.3 MB each.
- No encoder dependency.

**Budget.** Today 01's `assets/` holds 1.89 MB: 0.60 MB of sprites with their maps (528 files), 0.62 MB of sounds and a 0.58 MB tilemap. Expect about 3–4 MB more sprites with their maps, about 5 MB of music, and a tilemap two to three times larger.

**Art review.** Each stop's plan session draws its contact sheet in a scratch copy, as a spike does (nothing merged), and links it in the plan. The owner then reviews the look along with the plan before pasting the run prompt, and nothing stops for approval (`D-150`). The sheets:

- (a): the spell icons and the player's new clips;
- (b): the effects, status overlays and remaining icons;
- (c): an enemy silhouette sheet, before any animation is drawn;
- (d): one composite mood image per area, before full tiling.

## 01 (a): rebuild (level C, XL)

- **Screens.** All of today's non-menu screen text goes. The controls sheet becomes the shell's Keybindings screen, the status row becomes the engine's debug HUD, and the death title becomes the shell's game over (Retry, Load, Quit to Title).
- **Areas.** Entering each of today's four areas (Lantern clearing, Broken aqueduct, Waterfall ravine, Ruined tower) autosaves, and a toast names the area, so Retry resumes there. Today they are column ranges in `tools/level.json`'s `landmarks`, which nothing reads yet; the collection pit past the tower has no name.
- **Controls**, every action in `BINDABLE_ACTIONS`; pad bindings are declared, and work once the gamepad backend lands:

  | Action | Keyboard and mouse | Pad |
  | --- | --- | --- |
  | Move | A/D, arrows | Left stick |
  | Aim | Mouse | Right stick (kit aim) |
  | Jump, double jump | Space | South |
  | Shoot (from (b)) | LMB | RT |
  | Cast the selected spell (hold for sprays) | RMB | LT |
  | Cycle spells | Mouse wheel, wrapping | LB / RB |
  | Pick a spell directly | 1–9, 0 | — |
  | Cycle the summon type (from (b)) | Q / E | D-pad left / right |
  | Lantern | L | North |
  | Zoom | + / − | — |
  | Pause | Esc | Start |

  Removed: Mouse 4, the M music toggle, the S stop-all key and the 1, 2 and 3 volume keys; volume moves to the shell's settings. The engine's own keys (F4 HUD, F9 vsync, F11 fullscreen) stay as the engine binds them: F9 is the engine's `engine_toggle_vsync`, not 01's, and the shell keeps F9's and F11's changes in `Settings` (`D-192`). Today's tool buttons (LMB orbs, MMB small balls, RMB black hole, F ice spray, R iron brick, Enter to restart after death) give way to the spell bar and the shell's game over.

  Double jump is 01's own code today (`src/systems.rs`). The kit's controller has no air jump; it sits in the backlog's platformer row (air jump, wall jump and ledge grab). By the kit-first rule, (a)'s plan brings the air jump back as a level C kit step, or keeps the row and drops double jump.
- **Spells**, all available from the start and all costing mana, on one bar. A slot appears when its stop lands: 1 fire missile, 2 flamethrower, 3 ice spray, 4 lightning (b), 5 force push, 6 black hole, 7 iron block, 8 bowling ball, 9 small balls, 0 summon (b). Today's fireball, ice spray, black hole, iron brick (`IronBrick`, "Iron block" on screen) and small balls become slots 1, 3, 6, 7 and 9; the flamethrower, force push and bowling ball are new, and today's bronze orbs get no slot.
  - Every spell is a kit ability with its mana cost, cooldown and charges.
  - The flamethrower and the ice spray are one shared channelled spray with a short cone and a mana drain per second; only their data differs.
  - Force push is a wave that throws everything away from the player.
  - Bowling balls are heavy and tossed from the player toward the aim. They land low and roll. Small balls stay as today and fill the pit.
  - The iron block and summoned things are placed at the cursor.
- **Mana**: a kit stat that abilities spend (an ability's cost names its stat), capped by a max stat. The gun and the lantern cost nothing.
  - Two kit gaps (checked 2026-10-20). First, periodic effect payloads are damage and heal only, with nothing that restores another stat (the tick in `damage/effects.rs`). Second, an effect's blocking tags are checked only when it's applied, so a tag that appears later doesn't pause an effect already running (the apply path in `effects.rs`).
  - (a) closes both in one level C kit step: a periodic stat-restore payload capped at a max stat, and suppression, where an effect pauses while its target carries a listed tag. The payload raises the stat's base, since an ability's cost reads and spends the base (`intent/ability.rs`), and caps it at the max stat's value itself, since only `max_health` caps anything today: a stat's bounds are fixed numbers and `set_base` doesn't clamp (`foundation/stats.rs`).
  - Mana then regenerates through a never-ending effect that a casting tag suppresses for about a second after each cast (starting value). The troll's healing in (c) uses the same suppression with burning's tag.
- **HUD**, in the shell's theme:
  - an HP bar above a mana bar at the top left, each with "current / max", built the way the template sample's health bar is (a row whose fill is as wide as the value);
  - the spell bar at the bottom centre: icon, key, cost, cooldown sweep and charges, with the selected slot highlighted and unaffordable spells greyed out;
  - the gun's magazine beside it, from (b).
- **The template's audio.** The shell plays `ui_move`, `ui_select` and `ui_back` (`D-187`), loops `music_title` on the title (`D-191`) and plays game over's `game_over` sting and `music_over` (`D-189`), all by fixed ID; the sample plays a run loop and a pause loop; `templates/basic/tools/sounds.py` generates them all. The title loop the owner asked for is there already, so (a)'s template change is the pending autosave and its seam name, `shell::request_autosave` (`D-194`), which stay within the template's line caps (`D-187`). At 0.91 `src/shell/` holds 2,375 of its 2,400 lines of code and `src/` 2,872 of 2,900, and `AGENTS.md` 12,255 of its 12,288 bytes (`D-183`), so naming the call in its seam line takes an equal cut.
- **01's music**: generated title, exploration and pause loops, crossfaded with M68's fades. As in the template's sample, pause plays its loop over the game track, which keeps playing silently and resumes where it has got to, not where it stopped. A true voice pause stays the backlog's audio pause row.
- **Level**: today's level, with its placements moved from the generated `src/level_layout.rs` into `level.tmj`'s object layer and prefabs, emitted by `tools/generate.py` from `tools/level.json`. Today the object layer holds only the ten ambient emitters, and `level_layout.rs` the props, slab colliders, hazards and moving platforms. The only geometry change is widening the `gate_step` corner (`D-158`; [known issues](../../known-issues.md)). The player moves on the kit's platformer controller and the three moving platforms, on hand-written sine motion today, on its path follower.
- **Done-when:**
  - the route test passes with the player on the kit controller;
  - the shell-copy check passes, and so do the save round-trip, coverage and divergence tests;
  - the kit steps' tests pass: restoring a stat stops at its max; a suppressing tag added after regeneration has started pauses it, and removing the tag resumes it; and the controller and the path follower save and restore exactly mid-jump and mid-platform-cycle;
  - the template's pending-autosave test passes: the run's first autosave lands even when the first capture attempt returns `None`;
  - a scripted-input playthrough walks the level end to end, headless;
  - smoke is green;
  - the owner has played it through.

- **As built (M72, `D-197`).** Where the plan's answers departed from this section:
  - the fire hazards move on the kit's oscillator in (a), not (b) (Q2), since writing sine motion again would copy a kit feature;
  - the element rules (burning, chill and freezing, shattering, crushing, putting out fire) are game code ported with their tests (Q3); (b) moves burning and freezing onto damage types, effects and the ailment ladder;
  - the black hole opens at the cursor for 4 s, one at a time, and the drag while held went (Q4);
  - the iron block, the black hole and the small balls are placed by 01's code once a manual `use_ability` returns `Used::Used` (Q5), and the fire missile flies from the kit's `spawn` payload; a `place` payload waits in the backlog;
  - the courier draws lit, so a hit shows by its knockback, window and shake, with no flash (Q6);
  - a full pit's 12,000 balls save in 1.78 MiB, each ball one flat array (Q11), captured and written in 4.9 ms;
  - facing stays the movement's, and a cast's pose and the toss face the aim; the spell bar sits above the shell's toast lane at the bottom centre; the right stick's aim waits for the gamepad backend.

## 01 (b): elements and weapons (level C, XL)

**Damage model.** Kit damage types physical, fire, cold and shock, under one `damage` parent tag. A type's parent comes from its dotted name, so they are `damage.physical`, `damage.fire`, `damage.cold` and `damage.shock`, and a factor on `damage` reaches every part (`damage/kinds.rs`, `damage/defense.rs`). A hit resolves in this order:

1. the attack's base amount;
2. ×2 on a crit;
3. the target's `Resist` factors, meaning its type multipliers and then the shocked modifier;
4. shield, then health.

Enemy attacks also carry the difficulty multiplier, from (c).

- **Crits**: 5 % of the player's non-periodic hits crit for ×2 (starting value). They are rolled from the kit's seeded RNG, so a test can pin them, and set the kit's crit flag on the request. The kit carries the flag to `Damaged` and multiplies nothing, so 01 doubles the amount when it builds the request.
- **Physical** (the gun, impacts, dives, charges, the slam): hurts every target, never more than 1×, and armour resists it. It builds bleed on targets tagged `flesh`. Moving bodies (balls, blocks, thrown and falling things) deal physical damage by speed and mass above a threshold, credited to whoever launched them. The player takes it only from very hard hits.
- **Fire** (the missile, the flamethrower, burning, explosions): burns, and ignites flammable things. Burning grants a tag that suppresses regeneration effects, using the suppression (a) adds.
- **Cold** (the ice spray, the crawler's touch): chills, then freezes targets tagged `freezable`. The player only chills.
- **Shock** (lightning): it never ignites.
  - Its meter's first rung, shocked: for a few seconds the target takes 1.5× damage from every type. This is a `Mul(1.5)` modifier on its `damage` resist, so an immunity stays immune. An effect's resist modifier skips a target with no `Resist`, so everything that can be shocked carries one.
  - Its top rung: a brief stun that interrupts a wind-up.
  - It jumps between things tagged `conductive` (iron blocks, iron beetles), losing strength each jump, with jumps capped.
  - Ice doesn't conduct: a frozen target takes a direct strike's damage but passes nothing on and starts no ending.

**Frozen**: an effect that turns the target into an ice block, or a ball into an ice ball. It falls, can be pushed, and can be pulled and flung by the black hole. It thaws after a few seconds. A frozen target ends one of three ways:

- thermal shock: any fire hit causes an explosion (fire and physical parts, knockback). It ignites balls and shatters other frozen things in reach, so explosions chain. It hurts the player at reduced damage.
- shatter: physical damage past a threshold while frozen, done as an ailment rung on the physical meter gated by the `frozen` tag. It throws shards but doesn't explode.
- impact: a hard hit from a fast body or a fall shatters it.

**Burning hit by cold**: a steam burst. It's a wide push with modest fire and physical damage that puts out the target and anything burning nearby. Small things pop and are destroyed; bigger things take burst damage and stop burning. Bursts chain through burning piles. Things made of fire (wisps) freeze instead.

**Lightning:**

- It strikes from the sky at the cursor and stops at the first solid above, so in a cave it hits the ceiling.
- It jumps to the nearest iron within a short radius of the aim point (lightning rods).
- It shows a generated bolt sprite strip, a one-frame screen flash, a brief point light and screen shake (following the shell's Screen shake option), with generated thunder and a crackle when it jumps.

**Existing rules stay**: fire ignites balls, and cold frosts iron, which then ends the same three ways. The fire hazards move onto the kit's oscillator, and cold puts one out for a few seconds with steam. Of the level's 12 hazards, six are fire on hand-written sine paths today (`src/gameplay.rs`) and six are static spikes.

**The gun**: a fast kit projectile doing low physical damage. It is an ability whose charges are the magazine, with a reload. Shots push balls and bounce off iron.

**Damage numbers**: kit labels on enemies, the dummy and the player.

- Coloured by type; crits larger and yellow; burn and bleed ticks merged.
- Shown only while the shell's Damage numbers option is on.
- The kit's label cue can't style a crit differently (checked 2026-10-20: a label's style follows the damage type alone, and a cue occurrence drops `Damaged`'s crit flag, `react/cues.rs`), so 01 styles crits in its own code.

**The training dummy**: a target with settable `flesh`, `freezable` and `conductive` tags, placed in the Lantern clearing and summoned through slot 0, which arrives here with the dummy as its only entry. It makes (b) playable before any enemy exists. The summon slot's icon shows the selected type, and the Cycle summon type action, bindable like every other, changes it whether or not the slot is selected.

**Also in (b):**

- getting hit: the kit's hit flash, knockback, an invulnerability window and screen shake;
- first-time hints as shell toasts (first freeze, shatter, steam burst and shock), once per save.

**Done-when:**

- one harness test per rule above, including:
  - a chain of explosions through frozen balls;
  - a steam chain through a burning pile;
  - conduction stopping at ice;
  - a lightning rod;
  - shocked ×1.5 not breaking an immunity;
  - a crit at a pinned seed;
  - a chain larger than the per-step cap completing over several steps in its stable order;
- the save tests cover burning, frozen and shocked state, with each effect's source restored exactly, and a save taken mid-chain resumes with the same queue in the same save-ID order;
- smoke is green;
- the owner has played the elements through on the dummy.

- **As built (M74, `D-200`).** Where the plan's answers departed from this section:
  - balls, iron and scraps keep 01's own element components (burn, chill, a five-second `Frozen`, douse), and only what has health takes the kit's effects and ladders (Q1); each ending runs through a saved cascade queue, at most 48 entries a step, bounded at 16,384;
  - the dummy draws unlit through `damage_flash`, so the kit's flash shows (Q2); it takes every pulse, with a 0 s window (Q3), and stands in the Lantern clearing left of the spawn, placed by the map's last layer, `dummies`, its tags a Tiled property (Q4);
  - getting hit keeps the courier's set from (a), with no flash on its lit sprite (A20);
  - lightning's rod is the nearest conductive thing within 160 px of the aim (Q9), and conduction decides a jump a step, so ice that forms while a jump waits stops it (Q21); a rod under a ceiling is not struck, so that strike starts no chain;
  - the gun decides each bullet's step once, from what its path met first (Q22); the kit's `charges.all` refill starts at the first spent charge, so the ability counts the rounds and 01 times the 1.2 s reload, and the magazine reads "reloading" at none, with no sweep (Q20);
  - damage numbers float through the kit's `float_label` from 01's code: a type's parts merge while they arrive within 0.6 s, floated 0.6 s after the last or 1 s after the first, and a crit floats at once (Q13);
  - the summon slot places its dummy by 01's code once `use_ability` returns `Used::Used`, with no `place` payload (Q15), refused over a solid tile, four live at most, the oldest going (Q14);
  - the save is schema 5, which migrates 01 (a)'s schema 4 by moving every save ID from its player's up by the map's one dummy; a full pit saves in 1.94 MiB with each ball's one `ice` field.

## 01 (c): bestiary (level C, XL)

| Enemy | Moves | Physical | Fire | Cold | Shock | Bleeds | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Fire wisp (today's fireball, rewritten) | Flies near its spawn; chases on line of sight; searches the last-seen spot, then returns | 1× | Immune; burning piles attract and heal it | Freezes | 1× | No | Lights the scene; touch burns |
| Frost crawler | Walks ledges on the kit's platformer controller; avoids the lantern's light | 1× | 2× | Immune | 1× | Yes | Touch chills |
| Iron beetle | Heavy walker; paws the ground, then charges on sight | ¼×; its front deflects shots | ½× | Freezes brittle | 2×; conductive | No | Hit it from behind (kit facing) or freeze it; the iron brick crushes it |
| Dive bat | Roosts on ceilings; flashes, then swoops in an ability hitbox window, then climbs back | 1× | 1×, burns | Freezes and falls; shatters on landing | 1× | Yes | |
| Troll | Big, heavy and slow; high damage | 1× | 1×; burning stops its healing | Slowed in stages; freezes only at a very high threshold and thaws fast | 1×; stuns only at a high threshold | Yes | See below |

The troll:

- it regenerates steadily unless burning;
- it slams the ground after a long wind-up, sending a shockwave that damages, scatters balls and shatters frozen things;
- it picks up the nearest heavy object (an iron block, an ice block, a frozen enemy or a bowling ball) and throws it, so moving-body damage applies;
- knockback doesn't move it, the black hole only slows it, balls bounce off, and the iron brick can't crush it;
- it has a health bar over its head.

Behaviour:

- Line of sight is a `cast_ray` against terrain layers (M68 masks) in 01's own code, a few times a second, not a kit addition; a kit module is a 1.x backlog row.
- Each enemy runs patrol, alert, chase, search and return on the kit's state machine.
- An enemy that sees the player alerts enemies near it through a cue.
- Attacks are signalled before they land.
- Enemies' attacks and explosions hurt other enemies.
- Enemies far from the camera stop thinking.

Difficulty, from the shell's Easy, Normal and Hard (the run's difficulty in `RunMeta`), with starting values:

| | Enemy max HP | Enemy damage |
| --- | --- | --- |
| Easy | ×0.75 | ×0.75 |
| Normal | ×1 | ×1 |
| Hard | ×1.5 | ×1.5 |

These are kit stat modifiers from a `difficulty` source, applied at spawn, summoned enemies included. Ailment thresholds, the player and the dummy are not scaled.

Also in (c):

- Killed enemies stay dead; a Retry or Load restores them as saved.
- Summon (slot 0) adds all five enemies to its list after the dummy, picked with the Cycle summon type action. Live enemies are capped.
- Drops: health and mana orbs (loot tables and pickups).
- A generated combat-music layer crossfades in while an enemy sees the player.
- Each enemy has its sounds, and the troll has footsteps, a roar, the slam and the throw.

Done-when:

- one harness test per enemy rule, plus line of sight blocked by a wall, enemies hurting each other, the troll's healing blocked by burning, and each difficulty's HP and damage;
- the kit steps' exact tests pass: a save taken after an attack has already hit, and one taken while a state machine's transition is pending, restore exactly;
- the save divergence test passes with enemies mid-attack and mid-search;
- smoke is green;
- the owner has played it through.

- **As built (M75, `D-201`).** Where the plan's answers departed from this section:
  - the six fire hazards stay, and the wisp is a new enemy whose flame starts from the fireball's (Q2); nine enemies stand on today's map through its last layer, `enemies`, which (d) replaces (Q3): wisps over the tower's bridges, crawlers on the aqueduct's ledges, beetles and the troll on the gate's top, bats under the switch slabs;
  - the enemies draw unlit through `damage_flash`, as the dummy does, their rims and eyes painted in, and a lit sprite's flash waits in the backlog (Q1); the frozen look is a cube of ice fitted to each kind's hurtbox on a full frame, a small cube for a ball, in place of (b)'s flat block and round ice ball (Q17, Q18), with icy particles on a frozen target;
  - a spotting reaches the enemies within 320 px through 01's intent phase, the `spotted` cue playing its sting (Q5); an enemy past 1,200 px from the player thinks nothing (Q7); the flyers move on the kit's top-down mover and a frozen one falls on a borrowed controller (Q4); six summoned enemies live at most, the oldest going (Q12);
  - the crawler holds at the lantern's light and lunges only at an unlit player; the beetle's front is its saved heading, not the kit's facing (Q8): bullets pass it in the kit's pass and turn off its front in `gun_hits`, its rear takes every hit 01 builds whole, while an enemy's kit hitbox meets the shell (Q16), and its brittle freeze is a low shatter rung of its own; a frozen bat that lands takes a hit of all its health;
  - the troll lifts an iron block, a bowling ball, or a frozen enemy or dummy within 200 px, held as a sensor at its hands, not a child on the kit's hierarchy, since a body cannot be one (Q10); `Throws` credits a throw's first impact to its thrower at its damage factor, and the player takes impact damage from thrown bodies alone (Q9); its second slow is 01's, past twice its threshold, since a rung that fires empties its bucket; its freeze lasts 1.5 s; the black hole halves its speed; a left frame, its right one mirrored, draws 20 px over, so its overlays sit off its hurtbox facing left ([known issues](../../known-issues.md));
  - the orbs' pickup queues its heal or mana in 01's overlap phase, since a `picked_up` cue's effect would queue after the resolve (Q6); the combat layer fades in over 1.5 s and out over 3 s;
  - the save is schema 6 (`enemies`, `throws`, `orbs`), which migrates schema 5 by moving every save ID from its player's up by the map's nine enemies; a full pit with them alive saves in 1.97 MiB.

## 01 (d): new level (level C, XL)

Six areas, built in `tools/level.json` and emitted by the generator (tiles, art and the tmj object layer). The level is larger than today's 184 × 50 tiles at 64 px; the plan sets the size.

| Area | Built for |
| --- | --- |
| Lantern clearing (start) | No enemies: open ground, the big ball pit and the dummy, for summoning and chain reactions |
| Broken aqueduct | Frost crawlers: long ledges |
| Waterfall ravine | Bats: a tall shaft with ceilings to roost on |
| Ember hollow | Wisps and fire hazards: dark, lit by the wisps and the lantern |
| Iron mine | Beetles: an open cut under the sky, iron on its terraces' faces that deflects shots, and loose iron blocks |
| Troll bridge | The troll: a long, flat stretch with room to keep away |

Open sky and caves are mixed on purpose, since lightning only strikes outdoors. Long flat runs and stepped drops act as bowling lanes. There are no slopes or ramps, because the physics engine has only axis-aligned boxes and circles. The areas get ambience loops, autosaves and toasts.

Done-when:

- the route test and the scripted playthrough are rewritten for the new level and pass;
- a frame-time capture with the ball pit full and enemies active is recorded for C1;
- smoke is green;
- the owner has played it through.

Fallback: (d) is content with no public API. If Phase 6 runs long, the feature gate may re-tier it to 1.x without harming the freeze (`D-194`).

- **As built (M76, `D-202`).** Where the plan's answers departed from this section:
  - level C since the owner's reviews of 2026-10-22: projectiles meet the level, so the kit's `Projectile` turns at walls in its `bounce_layers`, passes those in its `pass_layers` and ends at the rest, with `ProjectileBounced`, `ProjectilePassed` and the wall on `ProjectileExpired` (Q4, Q16); the step 1 API critique raised no concerns, and the fallback is moot;
  - the level is 430 × 72 (Q1), today's start kept with the tests' sky empty (Q2), six areas joined by five transitions; the ball pit, dug into the clearing, sits under a rope bridge; the owner's last review halved it to 24 × 32 tiles with a free chute at each end (`pit_in_west`, `pit_in_east`), the bridge in two decks with a gap between them and ten shelves up its middle to that gap (`pit_climb`), took out the spikes and the roaming fireballs, sent the aqueduct's way over its channel and fallen spans and the ravine's down rocks across its falls, with two more lifts, and animated the falls' mist and the steam at the hollow's mouth (Q12);
  - the mine is an open cut under the sky, its iron only on its terraces' faces as static boxes in physics layer 2, its wooden scaffolds and the rope bridge in layer 3; bullets turn off iron and pass wood, the missile banks once off iron; four loose iron blocks are set down at markers on a fresh run, lightning rods near the beetles (Q5, Q15);
  - each area carries its light (a cave dims and the moon goes out, eased over 1 s; Q6), its ambience loop (Q7), its far strip and, for the ravine, the mine and the bridge, a true-black night sky, crossfaded by where the view stands (Q8, Q18); the hollow's lava lake and shelf pool hurt as the fires do and are never doused (Q17);
  - the art follows the review's sixth draw, with a pour clip for the gargoyle and the broken span, the river's depths, a fourth border (the mine into the bridge's masonry) and front copies of the arcade for the bridge; the far wall's tint in the sheet stays out, two darker steps carrying its depth in palette pixels;
  - the save is schema 7, refusing schemas 4 to 6 (Q3); a full pit with the map's 13 enemies saves in 2,079,145 bytes, under 2 MiB (Q13);
  - C1's reading, headless on the harness in a release build: a full pit with six enemies awake around the player on the rope bridge, `total` p50 6.306 ms on the 48 × 20 pit, owed again on the 24 × 32 one (Q9; [W10](w10-closing-phases.md) follow-ups).

## After (d): the feedback stops

The owner's notes after 0.97 became four more stops, (e) to (h), planned in the [feedback plan](example-01-feedback.md) (§3), each its own milestone and release; their notes of 2026-10-22 added two between (e) and (f), the quick fixes and a level and title pass.

- **As built, 01 (e) (M77, `D-205`).** Where it changed this file's rules and its stops' rows:
  - the player's lantern is the staff light's flame until (h)'s wizard: a kit ability burns 3 mana a second while it is lit, handed back at full mana, where the regeneration covers it; without mana it gutters out and relights at 10 (feedback Q15); 01 (c)'s crawler holds at the light's drawn edge, scared, with a clip, a hiss and a puff of frost breath, where it parked unexplained;
  - an area names itself on a banner of 01's own in the theme's heading face, one at a time (feedback Q14), not the shell's toast, which stays for the hints;
  - damage numbers float at once, burn and bleed ticks smaller; a dead player falls for three game seconds before game over; a hurt plays on a blow or the killing hit, never on a tick;
  - spells cost half and mana comes back at 30 a second; creatures freeze 7.5 s and the troll 3 s; the bolt falls at the cursor's x to the first thing below;
  - the save is schema 8, migrating schema 7 (the staff light's ability); the template's autosave writes over an older schema's slots (`D-204`);
  - the engine work the owner added past the feedback plan's non-goals: core physics keeps a tilemap's collision proxies across steps (`D-203`), the World keeps each query shape's matching archetypes (`D-206`, the feedback's X6, taken on its measurement), and the dev profile builds 01, the kit and the umbrella at opt-level 1 (P8).
- **As built, the quick fixes (M78, `D-207`).** Where they changed this file's rules and (e)'s:
  - the staff light is off at a new run's start, and a load keeps its saved light;
  - the courier has a death clip, shown from the first dead step and held, and the killing hit plays a death sound in place of the hurt; game over comes two game seconds after the death;
  - the area banner is Source Serif 4 Bold, 01's own copy beside the Regular, over no box, with a shadow of copies;
  - the crawler's touch reaches past its body, so its lunge lands; a frozen enemy touches nothing until it thaws; a placement may leash a patrol;
  - the map places eighteen enemies and eight health orbs that never fade; orbs glow without a light;
  - fire on a frozen fire-proof wisp blows it apart through the kit's `ImmunePart`, and lava blows apart what is frozen;
  - the save is schema 9, shifting the thirteen new map objects' save IDs.
- **As built, the level and title pass (M79, `D-208`).** Where it changed this file's rules and (d)'s:
  - lava is a surface the player stands on and never sinks below, in physics layer 4 that enemies' controllers leave out; its touch bites 16 fire, burns and throws up and toward the nearer bank, through the kit's `ContactDamage::with_push_always`; the hollow's lake is two pools round its stone and the shelf pool a column wider;
  - the ice spray sets a basin's crust after 0.5 s for 6 s, solid for every body, its touch held, cracking over its last 1.5 s before it melts and throws;
  - the mine's deck 4 is an elevator, the only way to an upper level with the map's nineteenth enemy, a fourth beetle; it starts when stood on through the kit's `PlatformerController::ground` and holds for a rider past its margin; a beetle's paw and charge are drawn;
  - the hollow's mouth is drawn as its own rock where the ravine's far wall showed, the ravine's pool ending against fallen stones, with no rising steam or heat at the mouth;
  - the New Game picker shows a card for each difficulty, in the template and 01;
  - the save is schema 10: only what a save names takes a save ID, so the map's art never moves one again, with each basin's crust and the elevator saved.
- **As built, 01 (f) (M80, `D-209`).** Where it changed this file's rules and (b)'s, (c)'s and (d)'s rows:
  - the force push and a missile's blast throw enemies by distance through the kit's `knock_back` at each kind's black-hole pull, and iron's blows push along their line; a kind whose prefab resists knockback whole, the troll, takes no push from 01, though the kit's own hitboxes and touches still stop it for 0.2 s ([known issues](../../known-issues.md)); an enemy past the level's edge is gone and stays gone;
  - up to three black holes stay open 6 s each, a fourth collapsing the oldest; each shrinks over its last 0.5 s and closes with a sound and a small ring, draws under the actors and props, hums on one voice by the nearest and sweeps up small balls from its core, twelve a hole a step;
  - iron weighs 400 and falls harder; its killing blow on flesh bursts in gore and its others knock back; a hard landing thuds; a block is refused before the spend where it would meet a tile, a body that is not dynamic or another block;
  - a jump under small balls shoves them off, and a jump they blocked launches again the next step without spending the air jump, since the kit's controller never pushes bodies (feedback K7);
  - the statuses look as the table above says; the troll's two chill stages become every freezable creature's graded slow of up to 35 % more; deaths, gore, summons, shatters and refusals take a cosmetic burst's slot at the cap; the sprays' particles draw over the enemies (feedback K2);
  - the gun holds nine rounds 0.2 s apart and reloads on R or pad West; an empty trigger clicks;
  - a refused cast says why (no mana, not ready, or no room with a fizzle) and spends nothing; the air jump, a spell switch, a kill, the dummy's death, a dropped orb, which blinks over its last 3 s, and the sprays' ends sound; small balls pour on a loop;
  - the save is schema 11: `black_holes` in a list, migrating schema 10's one hole and holding its magazine to nine.
- **As built, 01 (g) (M81, `D-210`).** Where it changed this file's rules and (b)'s, (c)'s and (d)'s rows:
  - pine crates and powder kegs are bodies like iron, set down at the map's markers by a fresh run and placed by the summon slot, whose types are an enum: the dummy, the six kinds, the crate and the keg; a crate's fire is 01's body state, as the balls' is, and a keg's fuse 01's saved field, since the kit's timers have no expiry event and no saved form (feedback K11);
  - an ending credits whoever began its chain, not only the player, and a keg's or a goblin's bomb's blast is a fourth ending, `Detonation`, since the kit has no radial blast payload (feedback K10);
  - the goblin is the sixth enemy, a skittish walker that keeps its distance, hops what it can and stabs a courier too close, throwing rocks that stay as rolling bodies and, one throw in five by `KitRng`, a lit bomb that rolls with its fuse; seven stand on the map, which places twenty-six;
  - the troll's slam tells for 0.8 s with a dust ring about its fist, a grunt and a shake, it recovers in 0.35 s, and under 90 px a quicker swipe, 01's box hit, takes the slam's place, since a kit hitbox cannot follow its owner's facing (feedback K3); the slam and the swipe kick each body by its mass; a troll lifts crates, never kegs;
  - a fresh alert shows "!" and sounds the sting once for the enemies it alerts, from 01's code rather than (c)'s `spotted` cue, and a search shows "?";
  - the save is schema 12 (`props`, the rocks among them), moving every save ID from schema 11's first placed orb's up by seven, past the map's goblins; a ball's array leaves its trailing nulls off, so a full pit with the map's 26 enemies and 14 props saves in 1,787,829 bytes.
- **As built, 01 (h) (M82, `D-211`).** Where it changed this file's rules and (a)'s rows:
  - the player is a wizard on a 96 px frame, the body as the courier's at (48, 65), so `BODY_OFFSET` moves and a save's player corner with it (schema 13); his staff's orb is the staff light, plain glass under the glow, its anchors from the rig; he casts each spell with a gesture, poses with the staff for the sprays and the small balls and with a pistol for the gun, plays a hurt clip on a blow and falls on his back when he dies; his status overlays draw in a 64-unit square on his body;
  - the icons are one family on 36 px tiles; the HUD's bars fill with a scrolling liquid in their own colours, each slot shows its live key with no cost or number, the gun stands apart with its rounds as cartridges, and no HUD widget takes the theme's accent;
  - the fire missile has its own clip, a rocket; the sprays reach 9.1 and 4.55 tiles, draw as animated sprites with a light, their particles accents, and burst on each live target every second damage pulse;
  - 01's theme is the template's structure in its own colours, which `just repo-check` holds by its masked text, amending `D-199`'s copy check; the title draws the night in OLED black with its own stars.
