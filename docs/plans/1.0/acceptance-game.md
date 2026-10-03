# Acceptance game — draft

- **status:** draft (skeleton)
- **goal:** Specify the small, complete game that tests definition A early enough for its feature map to cut the tiers of W5, W6 and W13, then log the gaps it finds while it is built.
- **non-goals:** The game's code, which lives in its own repository; features beyond what tests 1.0; engine patches made for it.
- **files to touch:** This file.
- **ordered steps:** (1) The owner picks the genre and writes the pitch (Q1). (2) List screens and mechanics. (3) Map each feature to an engine need and a workstream, and re-tier W5, W6 and W13 from the map at the definition gate. (4) Start the repository from `templates/basic` when W15a lands, where and on which dependency the definition gate says ([implementation plan](implementation-plan.md) §9, item 14); its sessions follow [workflow.md](workflow.md) §7. (5) Log every gap below until the feature gate.
- **done-when:** The feature map is agreed at the definition gate. At the feature gate, the game plays start to finish from its `tungsten package` archive, and every gap row is closed or moved to the [backlog](backlog-1.x.md).

## Constraints

From [criteria](criteria.md) §1 and the owner's answers of 2026-10-03:

- Its own repository, with the engine as a git dependency on a tag, started from `templates/basic`.
- The public API and `tungsten-kit` only, with no engine patches.
- Released through `tungsten package`.
- Small: a feature it needs is a 1.0 candidate, and a feature it does not need is 1.x.

Revised 2026-10-03 at `9cd5709`: step 4 points at agenda item 14; the settings row names W6c.

## Pitch

*Open (Q1): genre, length, and a paragraph on the game. The definition gate agenda proposes a small top-down action game, which gives the kit's top-down mover a user ([implementation plan](implementation-plan.md) §9, item 3).*

## Screens

| Screen | Contents | Engine needs |
| --- | --- | --- |
| Title | Start, settings, quit | W1 M4 state-owned roots; keyboard and, if Q9 says so, gamepad navigation |
| Settings | Volume, rebinding, display | W1 broader controls (Q8); W11b persistence; W6c buses for separate music and effects volume |
| Gameplay | *After the pitch* | — |
| Pause | Resume, settings, quit to title | W3a game clock paused; W1 M3 routed gameplay view |

## Mechanics

*After the pitch.* For each mechanic, say what it takes from the kit (W13) and what it needs from physics and audio (W6).

## Feature map

| Feature | Engine need | Workstream | Effect on tiers |
| --- | --- | --- | --- |
| Title menu, pause | UI screens, state-owned roots, routed input | W1 M3, M4 | Already Must |
| Settings for volume, rebinding, display | Sliders, checkboxes; persistence; buses if music and effects have their own volume | W1 broader controls, W11b, W6c | Pulls broader controls into 1.0 (Q8); separate volumes make W6c Must |
| Settings kept across a restart | User folder, settings layer | W11a, W11b | Already Must |
| Gamepad play | Gamepad backend; UI navigation | W5a | Makes W5 Must (Q9) |
| A save slot | Versioned envelope | W11c | Makes save slots Must (Q16) |
| Release archive | `tungsten package`, licence notices | W14b, W9a | Already Must |
| *One row per mechanic* | | W6, W13 | Decides W6b, W6c and kit items |

## Gap log

Rows come from the game repository's `GAPS.md`, copied here when an engine session next plans or releases ([workflow](workflow.md) §7).

| Date | Gap | Filed as | Status |
| --- | --- | --- | --- |
| — | — | — | — |
