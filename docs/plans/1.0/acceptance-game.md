# Acceptance game — draft

- **status:** draft
- **goal:** Specify the small, complete game that tests definition A early enough for its feature map to cut the tiers of W5, W6 and W13, then log the gaps it finds while it is built.
- **non-goals:** The game's code, which lives in its own repository; features beyond what tests 1.0; engine patches made for it.
- **files to touch:** This file.
- **ordered steps:** (1) The owner picks the genre and writes the pitch (Q1). (2) List screens and mechanics. (3) Map each feature to an engine need and a workstream, and re-tier W5, W6 and W13 from the map at the definition gate. (4) Start the repository from `templates/basic` when W15a lands, where and on which dependency the definition gate says ([implementation plan](implementation-plan.md) §9, item 14); its sessions follow [workflow.md](workflow.md) §7. (5) Log every gap below until the feature gate.
- **done-when:** The feature map is agreed at the definition gate. At the feature gate, the game plays start to finish from its `tungsten package` archive, and every gap row is closed or moved to the [backlog](backlog-1.x.md).

## Constraints

From [criteria](criteria.md) §1 and the owner's answers of 2026-10-03:

- A GitHub repository of its own, cloned beside this checkout, with the engine as a git dependency on a `v0.NN.0` tag, started from `templates/basic` (definition gate, `D-102`).
- The public API and `tungsten-kit` only, with no engine patches.
- Released through `tungsten package`.
- Small: a feature it needs is a 1.0 candidate, and a feature it does not need is 1.x.

Revised 2026-10-03 at `9cd5709`: step 4 points at agenda item 14; the settings row names W6c. Revised at the definition gate the same day ([implementation plan](implementation-plan.md) §11): the genre, a proposed pitch, screens and mechanics, and the rows Q8, Q9 and Q16 decided.

## Pitch

*The genre was chosen at the definition gate (`D-102`). This pitch and the screens and mechanics below are proposals; the frame-loop gate agrees them with the owner and writes the feature map's mechanics rows.*

A top-down, survivors-like auto-shooter. The player only moves; weapons fire at the nearest enemies by themselves. A run lasts a set time on one map, with enemy waves from spawners growing denser as it goes. Defeated enemies drop XP, and each level-up pauses play for a choice of upgrades. A boss ends the run. A save slot keeps unlocks and best records between runs. It plays on keyboard or gamepad. Run length and the number of upgrades are open.

## Screens

| Screen | Contents | Engine needs |
| --- | --- | --- |
| Title | Start, settings, quit | W1 M4 state-owned roots; keyboard and gamepad navigation |
| Settings | Volume, rebinding, display | W1 BC sliders and checkboxes; W11b persistence; W6c buses for separate music and effects volume |
| Gameplay *(proposed)* | The map, the player, enemies, projectiles and pickups; a HUD with health, an XP bar and the run timer | Kit mover, spawner, health and damage (W13); W6 sensors and shape queries; W3a game clock |
| Level-up *(proposed)* | A choice of upgrades while play is paused | W3a game clock paused; W1 M4 screen with gamepad navigation |
| Pause | Resume, settings, quit to title | W3a game clock paused; W1 M3 routed gameplay view |
| Results *(proposed)* | Run time, kills, unlocks; back to title | W11c save slot written; W1 M4 |

## Mechanics

For each mechanic, what it takes from the kit (W13) and what it needs from physics and audio (W6). *Proposed at the definition gate; the frame-loop gate agrees them and turns them into feature-map rows.*

| Mechanic | From the kit (W13) | From physics and audio (W6) | Other |
| --- | --- | --- | --- |
| Movement | Top-down mover | Kinematic body (G1) | Gamepad stick (W5) |
| Weapons fire at the nearest enemy | Spawner of projectile prefabs; lifetime timers | Shape query for the nearest enemy; projectile sensors (G2) | Prefabs (W16) |
| Enemy waves | Spawner with interval and count; despawn outside a region | Bodies that push apart in a crowd | W3 timers |
| Contact damage | Health and damage, invulnerability time, hit flash | Sensors (G2); a sound on an event, on the effects bus | — |
| XP pickups | Trigger zones | Sensors (G2) | — |
| Level-up choice | — | Music keeps playing while play is paused | W1 screen; Q13's clock rules |
| Boss | Per-entity state machine | A music change on the music bus (W6c) | — |
| Unlocks and records | — | — | W11c save slot |

Hundreds of enemies and projectiles on screen at once make the game a load test as well: a candidate C1 budget row (Q12).

## Feature map

| Feature | Engine need | Workstream | Effect on tiers |
| --- | --- | --- | --- |
| Title menu, pause | UI screens, state-owned roots, routed input | W1 M3, M4 | Already Must |
| Settings for volume, rebinding, display | Sliders, checkboxes; persistence; buses if music and effects have their own volume | W1 BC, W11b, W6c | W1 BC placed (Q8, definition gate); separate volumes make W6c Must |
| Settings kept across a restart | User folder, settings layer | W11a, W11b | Already Must |
| Gamepad play | Gamepad backend; UI navigation | W5a | W5 Must (Q9, definition gate) |
| A save slot for unlocks and records | Versioned envelope | W11c | W11c Must (Q16, definition gate) |
| Release archive | `tungsten package`, licence notices | W14b, W9a | Already Must |
| *One row per mechanic, at the frame-loop gate* | | W6, W13 | Decides W6b, W6c and kit items |

## Gap log

Rows come from the game repository's `GAPS.md`, copied here when an engine session next plans or releases ([workflow](workflow.md) §7).

| Date | Gap | Filed as | Status |
| --- | --- | --- | --- |
| — | — | — | — |
