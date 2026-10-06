# Acceptance game — draft

- **status:** in progress
- **goal:** Specify the small, complete game that tests definition A early enough for its feature map to cut the tiers of W5, W6 and W13, then log the gaps it finds while it is built.
- **non-goals:** The game's code, which lives in its own repository; features beyond what tests 1.0; engine patches made for it.
- **files to touch:** This file.
- **ordered steps:** (1) The owner picks the genre and writes the pitch (Q1). (2) List screens and mechanics. (3) Map each feature to an engine need and a workstream, and re-tier from the map: W5 at the definition gate, W6 and W13 at the frame-loop gate. (4) Start the repository from `templates/basic` when W15a lands, where and on which dependency the definition gate says ([implementation plan](implementation-plan.md) §9, item 14); its sessions follow [workflow.md](workflow.md) §7. (5) Log every gap below until the feature gate.
- **done-when:** The feature map is agreed: its first rows at the definition gate, its mechanics rows and the W6 and W13 tiers at the frame-loop gate. At the feature gate, the game plays start to finish from its `tungsten package` archive, and every gap row is closed or moved to the [backlog](backlog-1.x.md).

## Constraints

From [criteria](criteria.md) §1 and the owner's answers of 2026-10-03:

- A GitHub repository of its own, cloned beside this checkout, with the engine as a git dependency on a `v0.NN.0` tag, started from `templates/basic` (definition gate, `D-102`).
- The public API and `tungsten-kit` only, with no engine patches.
- Released through `tungsten package`.
- Small: a feature it needs is a 1.0 candidate, and a feature it does not need is 1.x.

Revised 2026-10-03 at `9cd5709`: step 4 points at agenda item 14; the settings row names W6c. Revised at the definition gate the same day ([implementation plan](implementation-plan.md) §11): the genre, a proposed pitch, screens and mechanics, and the rows Q8, Q9 and Q16 decided. Revised 2026-10-05 on `01f9c55`, uncommitted: the pitch, screens and mechanics agreed with the owner ahead of the frame-loop gate, the feature map's mechanics rows written and W6 and W13 tiers proposed from them. Revised 2026-10-06 on `01f9c55`, uncommitted: the frame-loop gate confirmed the tiers as proposed and placed the five items (`D-132`), and the glyph gate read the damage numbers as per-label pop and scale (`D-126`).

## Pitch

*Agreed with the owner on 2026-10-05, ahead of the frame-loop gate: the definition gate's pitch (`D-102`) with a ten-minute run, three weapons and five passives, and an arena generated each run.*

A top-down, survivors-like auto-shooter. The player only moves; weapons attack by themselves, the wand firing at the nearest enemy. A run lasts ten minutes in an arena generated from a seed, with enemy waves from spawners growing denser as it goes, in crowds that can trap the player. Defeated enemies drop XP, and each level-up pauses play for a choice of three upgrades from three weapons (a wand, an aura and orbiting blades) and five passives (speed, max health, damage, cooldown and pickup radius), each up to level 5. At ten minutes a boss arrives: beating it wins the run, and the player's death ends it at any point. A save slot keeps unlocks and best records between runs. It plays on keyboard or gamepad.

The arena is a bounded map of about 64×64 tiles built in code each run (floor, edge walls, obstacle clusters) with scattered prop prefabs. The camera follows the player within its bounds, and the generator picks the spawn points. A debug override shortens the run for tests.

## Screens

| Screen | Contents | Engine needs |
| --- | --- | --- |
| Title | Start, settings, quit | W1 M4 state-owned roots; keyboard and gamepad navigation (W1 M3, W5a) |
| Settings | Music and effects volume, rebinding, display | W1 BC sliders and checkboxes; W1 M4's capture-next-input mode, with gamepad bindings (W5a); W11b persistence; W6c buses for separate music and effects volume |
| Gameplay | The arena, the player, enemies, projectiles, pickups and damage numbers; a HUD with health, the XP bar, the level, the kill count and the run timer, and the boss's health bar while it lives | Kit mover, spawner, health and damage (W13); W6 kinematic bodies, sensors, queries and masks; W3a game clock; W1 M4 HUD; W1 world labels for the damage numbers |
| Level-up | Three upgrade cards with icons while play is paused | W3a game clock paused; W1 M4 screen with M2 images and gamepad navigation |
| Pause | Resume, settings, quit to title | W3a game clock paused; W1 M3 routed gameplay view |
| Results | Win or loss, run time, kills, level, unlocks and new records; back to title | W11c save slot written; W1 M4 |

Agreed on 2026-10-05: the six screens as proposed, with the HUD's level, kill count and boss bar and the damage numbers added. Declined: a records screen, and retry and the run's seed on Results.

## Mechanics

For each mechanic, what it takes from the kit (W13) and what it needs from physics and audio (W6). *Agreed with the owner on 2026-10-05, ahead of the frame-loop gate: the definition gate's eight rows with the pitch's weapons and arena, enemies that block the player, four projectile behaviours, damage numbers that pop and scale (per label since the glyph gate, `D-126`) with bold crits, and game feel.*

| Mechanic | From the kit (W13) | From physics and audio (W6) | Other |
| --- | --- | --- | --- |
| Movement | Top-down mover | Kinematic body (G1) that slides along walls and stops at enemies: shape casts against tiles and bodies | Gamepad stick (W5a); W3b fixed step |
| Enemies block the player; contact damage | Health and damage, invulnerability time, hit flash | Contact events between the kinematic player and dynamic enemies (G1); a sound on the effects bus | Camera shake on a hit; the player's death ends the run |
| Wand: bolts at the nearest enemy | Lifetime timers | Shape query for the nearest enemy; bolt sensors masked to enemies (G2, W6b) | Bolt prefabs on a W3a `Timer` (W16b) |
| Aura: damage around the player | Health and damage; the ring sprite a child of the player (W13b) | Circle overlap query on a timer | W3a `Timer` |
| Orbiting blades | Children of the player (W13b), or roots placed each step if Q21 keeps bodies on roots | Kinematic sensors masked to enemies (G1, G2, W6b); one hit per enemy per invulnerability window | — |
| Projectile behaviours: stop at walls, pass over them, bounce, pierce | Despawn outside the arena, for bolts that pass over its walls | Ray and shape casts against tile layers with the hit point and normal (stop, bounce); a mask per prefab (pass over); sensor enter events per pair (pierce) | Behaviour set per prefab (W16b) |
| Enemy waves | Spawners with interval and count, placed by the generator | Dynamic bodies that steer at the player and push apart in a crowd | W3a timers and the run clock; enemy prefabs (W16b) |
| XP pickups | Trigger zones | Gem sensors masked to the player (G2, W6b); a circle query for the magnet radius | Gem prefabs (W16b) |
| Level-up choice | — | Music keeps playing while play is paused | W1 M4 screen with icon cards (M2 images); Q13's clock rules; offers from the seeded `Pcg32` |
| Boss | Per-entity state machine | A music change on the music bus (W6c); a crossfade needs fades | HUD boss bar (W1 M4) |
| Unlocks and records | — | — | W11c save slot |
| Generated arena | — | Tile collision for kinematic, dynamic and sensor bodies and for queries (W6a) | A tilemap built in code; the seeded `Pcg32`; prop prefabs (W16b); camera follow and bounds (`D-073`) |
| Damage numbers | — | — | W1 world labels (W1 WL, `D-126`, `D-132`); pop and scale per label, since the glyph gate kept glyphon (T1b) and deferred per-glyph effects to 1.x; bold crits from a family's bold face (`D-115`) |
| Game feel | Animated sprite | — | Death particles and camera shake, as plugins (W15a) |
| Music and effects | A sound played on an event | Music and effects buses (W6c); a voice limit per sound, so a burst of hits neither clips nor overflows the command ring, which drops commands when full (known P3) | Volume sliders (W1 BC), kept by W11b |

Hundreds of enemies, bolts and gems on screen at once make the game a load test as well: a candidate C1 budget row (Q12). A crowd pressing on the player from every side is the bounded-load pile of [known issues](../../known-issues.md) (`D-063`, `D-094`) without gravity, against a kinematic body; the arena's tile proxies (tile collision gathers every solid tile each step) and the damage numbers' churn add to it.

## Feature map

The first six rows came from the definition gate; the rest are one row per mechanic, agreed on 2026-10-05.

| Feature | Engine need | Workstream | Effect on tiers |
| --- | --- | --- | --- |
| Title menu, pause | UI screens, state-owned roots, routed input | W1 M3, M4 | Already Must |
| Settings for volume, rebinding, display | Sliders, checkboxes; persistence; buses if music and effects have their own volume | W1 BC, W11b, W6c | W1 BC placed (Q8, definition gate); separate volumes make W6c Must |
| Settings kept across a restart | User folder, settings layer | W11a, W11b | Already Must |
| Gamepad play | Gamepad backend; UI navigation | W5a | W5 Must (Q9, definition gate) |
| A save slot for unlocks and records | Versioned envelope | W11c | W11c Must (Q16, definition gate) |
| Release archive | `tungsten package`, licence notices | W14b, W9a | Already Must |
| Movement, blocked by walls and crowds | Kinematic body; shape casts against tiles and bodies; contact events | W6a, W13c, W5a, W3b | W6a kinematic bodies and casts Must; W13c top-down mover Must |
| Contact damage | Health, invulnerability time, hit flash; contact events | W13d, W6a | W13d health and damage Must |
| Wand | Nearest-enemy query; bolt sensors; prefabs on a timer; lifetime | W6a, W6b, W13a, W16b, W3a | W6a shape queries and sensors Must; W13a lifetime Must |
| Aura | Circle overlap query; a ring sprite parented to the player | W6a, W13b, W3a | W13b hierarchy Must, with a game user |
| Orbiting blades | Kinematic sensors moved each step | W6a, W13b | Input to Q21 |
| Projectile behaviours | Casts with the hit normal against tile layers; masks per prefab; sensor enter events; despawn outside the arena | W6a, W6b, W13a | W6a ray casts Must (were Should); W6b layers and masks Must (were Should); W13a despawn outside a region Must |
| Enemy waves | Spawners placed by the generator; crowds of dynamic bodies; the run clock | W13 spawner, W16b, W3a | Kit spawner Must; the crowd is a W6a probe and a C1 row candidate |
| XP pickups | Trigger zones; masked gem sensors; magnet query | W13d, W6a, W6b | W13d trigger zones Must |
| Level-up choice | Game clock paused with music on; card screen; navigation | W3a, W1 M2–M4, W5a | Already Must; a case for Q13 |
| Boss | Per-entity state machine; music change; HUD bar | W13d, W6c, W1 M4 | W13d state machine Must; W6c fades Should |
| Unlocks and records | Save slot | W11c | Already Must (Q16) |
| Generated arena | A tilemap built in code; tile collision for every body kind and query; prop prefabs | W6a, W16b; the builder unplaced | New need: a supported tilemap builder, W16b's (`D-132`); W16b's Tiled object classes keep example 01 as their only user |
| Damage numbers | World labels with per-label pop and scale; bold crits | W1 WL | World-anchored text from the 1.x backlog into 1.0 as W1 WL, after M2 (`D-126`, `D-132`); the per-glyph version is the T2 backlog row's first criterion |
| Game feel | Animated sprites; particles and shake as plugins | W13a, W15a | W13a animated sprite Must |
| Music and effects | Buses; a voice limit per sound; a sound on an event | W6c, W13a | W6c buses and voice limits Must; audio pause to 1.x |

## Proposed tiers

From the mechanics rows; confirmed as proposed at the frame-loop gate on 2026-10-06 (`D-132`; [implementation plan](implementation-plan.md) §11). W6's rows are Q10's answer. The kit's admission rule ([criteria](criteria.md) §8.4) still applies at the freeze: an item whose only user is example 01 is Must only if example 01 moves onto the kit by then (W12b).

| W6 item | Proposed | Was | Users in the game |
| --- | --- | --- | --- |
| Kinematic bodies (G1), with contact events against dynamic bodies | Must | Must | The mover; the blades |
| Sensors (G2), with enter events per pair | Must | Must | Bolts, blades and gems; pierce |
| Shape queries: nearest and overlap | Must | Must | Wand targeting, the aura, the magnet |
| Shape and ray casts against bodies and tile layers, with the hit point and normal | Must | Must for shapes, Should for rays | The mover against walls and crowds; bolts that stop or bounce |
| Layers and masks (W6b) | Must | Should | Every sensor and the mover's casts; bolts that pass over walls |
| Audio buses (W6c) | Must | Should; the settings row made it Must | Music and effects volume; the boss's music change |
| A voice limit per sound (W6c) | Must | Should | Bursts of hits and kills |
| Voice handles and fades (W6c) | Should | Should | A crossfade into the boss music; a cut works |
| Audio pause (W6c) | 1.x | Should | None: music plays through pause and level-up, and effects are short |

| W13 item | Proposed | Users |
| --- | --- | --- |
| Animated sprite (W13a) | Must | The game's walk cycles; example 01 |
| Lifetime and despawn-after timers (W13a) | Must | Bolts |
| Despawn outside a region or the view (W13a) | Must | Bolts that pass over the arena's walls; example 01's out-of-bounds bodies |
| A sound played on an event (W13a) | Must | Hits, kills, pickups, level-ups |
| `cursor_to_world` (W13a) | Must if example 01 migrates by the freeze | Example 01's click-spawn only |
| Transform hierarchy (W13b) | Must | The aura's ring; the blades, if Q21 allows sensors on children; example 01's light anchors |
| Spawner, after W16b | Must | Enemy waves |
| Top-down mover (W13c) | Must | The player |
| Platformer controller (W13c) | Must if example 01 migrates by the freeze | Example 01 |
| Health and damage, invulnerability time, hit flash (W13d) | Must | The player, enemies and the boss |
| Trigger zones (W13d) | Must | Gems |
| Path follower (W13d) | Must if example 01 migrates by the freeze | Example 01's moving platforms |
| Per-entity state machine (W13d) | Must | The boss |

Beyond W6 and W13, the agreed spec left five items for the frame-loop and glyph gates, settled on 2026-10-06:

- **World-anchored text.** The damage numbers bring it from the 1.x backlog into 1.0 as W1's world-label path ([w01](w01-ui-text-suite.md) §14, with its scenario in §10). Placed: W1 WL, a level B step after M2 in Phase 6 (`D-126`, `D-132`); the backlog row is gone.
- **T2.** Per-glyph pop and scale is a per-glyph effect, which T1 and T1b cannot draw (w01 §6), so it was a T2 criterion for the glyph gate. Settled: the gate kept glyphon as T1b and the damage numbers pop and scale per label (`D-126`); the per-glyph version heads the T2 backlog row. Bold crits need only a family's bold face (`D-115`).
- **A tilemap built in code.** The arena needs a supported constructor and tile setters. Today only `TilemapData`'s public fields and `TilemapRegistry::insert` allow it, and W4's audit could close them. Settled: W16b, which already touches the tilemap loader, owns a public builder with tile setters (`D-132`).
- **The crowd.** W6a's plan probes enemies pressing on a kinematic player from every side, with several spawn orders and frame dt as a parameter, before the kit's mover lands (`D-132`).
- **Q13 and Q21.** While play is paused, audio and UI run on real time, and gameplay, particles, shake and damage numbers on the game clock: the clock table of `D-129`. The blades were the game's case for a sensor on a child entity; Q21 keeps bodies on roots (`D-132`), so the game places them each fixed step from the player's position.

## Gap log

Rows come from the game repository's `GAPS.md`, copied here when an engine session next plans or releases ([workflow](workflow.md) §7).

| Date | Gap | Filed as | Status |
| --- | --- | --- | --- |
| — | — | — | — |
