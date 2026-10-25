# Phase 6 milestone 84: example 01's pushes and spells (01 (j)) — done

- **status:** done
- **goal:** The owner's notes N4, N6, N9, N15, N19 and N20 of 2026-10-24 ([feedback](../../1.0/example-01-feedback.md) §3.6). The kit's `PlatformerController` gains an opt-in shove of light bodies and an opt-in step height (K7, K18), and 01 turns them on, so the wizard walks small bodies aside and walkers get over the balls in their way. The force push becomes a wave of bent air from the wizard toward the cursor, throwing what its front crosses along the aim. The black hole lifts bowling balls, opens with a grow-in, a closing ring, an inward rush of sparks and a generated sound, and holds bats and wisps about its core rather than flinging them through it. A chilled creature's frost climbs in four stages from the first point of cold.
- **non-goals:** 01 (k)'s notes N1, N2, N5 and N21 (scenery, the aqueduct, the lava's freeze and thaw, the start). K7's other half (a controller pushed by bodies), K20 (outside velocity on the top-down mover), K21 (an emitter's rate at run time) and K28, found here (a hit that does not flash): backlog rows (§3 step 7). Any engine, core, template or shell change. A save schema change (§2.7). New spells, slopes, and any tuning outside the six notes.
- **files to touch:**
  - Step 1: `crates/tungsten-kit/src/motion/platformer.rs`, `crates/tungsten-kit/src/motion/slide.rs`, `crates/tungsten-kit/src/tests/platformer.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `docs/plans/1.0/w04-api-freeze.md` (the ledger row), `DESIGN.md` (the kit's controller paragraph), `docs/known-issues.md` (the controllers' bullet).
  - Step 2: `examples/01_platformer/src/features/enemies/{mod.rs,systems.rs,crawler.rs}`, `examples/01_platformer/src/features/player/mod.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tests/game/{player.rs,enemies.rs,saves.rs}`, `examples/01_platformer/README.md`.
  - Step 3: `examples/01_platformer/src/features/toys/{wave.rs (new),mod.rs,components.rs,draw.rs}`, `examples/01_platformer/src/features/spells/systems.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/tools/{effects.py,art.py,generate.py,sfx.py,README.md}`, the generated `examples/01_platformer/assets/` (sprites, animations, particles, `sounds/force_push.wav`, `manifest.json`) and `tools/outputs.json`, `examples/01_platformer/tests/game/{enemies.rs,spells.rs}`, `examples/01_platformer/README.md`.
  - Step 4: `examples/01_platformer/src/features/toys/{mod.rs,systems.rs,components.rs,draw.rs}`, `examples/01_platformer/tools/{generate.py,sfx.py,sounds.json,README.md}`, the generated `assets/` (`particles/hole_open.json`, `sounds/black_hole_open.wav`, `manifest.json`; `sounds/black_hole.ogg` deleted) and `tools/outputs.json`, `examples/01_platformer/tests/game/{enemies.rs,spells.rs,spawning.rs,presentation.rs}`, `examples/01_platformer/README.md`.
  - Step 5: `examples/01_platformer/src/features/combat/{mod.rs,systems.rs,components.rs,draw.rs}`, `examples/01_platformer/tools/{effects.py,art.py,generate.py,README.md}`, the generated `assets/` (frost sprites, `particles/chill_flakes_{1..4}.json`, `manifest.json`) and `tools/outputs.json`, `examples/01_platformer/tests/game/{ice.rs,data.rs,saves.rs}`, `examples/01_platformer/README.md`.
  - Step 6: the files of steps 1–5 only, for a failing test's fix.
  - Step 7: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/example-01-feedback.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/known-issues.md`, `docs/plans/1.0/implementation-plan.md` (the register row), `docs/plans/1.0/README.md` (the four owned Now fields), this plan.
  - Step 8: as the skeleton's release step.
- **ordered steps:** (1) The kit's shove and step height. (2) 01's shoves and steps. (3) The force push as a wave. (4) The black hole. (5) The chill's build-up. (6) The test pass. (7) Records. (8) Release 0.105.
- **done-when:** Every step's evidence row is quoted. Workflow §5 is closed. The release checks, with `just level-check` (step 6 ran it), pass after the cut. The preflight prints the command block and the post-merge block.

Candidate 01 (j) pushes and spells ([implementation plan](../../1.0/implementation-plan.md) §4 card, level B), milestone M84, release 0.105. Written 2026-10-24 at `7d83d48`, on branch `0.105`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. The art-review sheet the feedback plan asks the plan session to draw first is machine-local in `perf-runs/20261024-e01j-art-review/`. It holds real in-game loops of a prototype in a scratch copy of 01 under `target/` (nothing merged), the art and sound proposals, and their scripts. §2.8 holds its proposals as this plan's defaults.

## Context digest

- 01 (j) is the eighth of example 01's stops after (d) (`D-194`; [feedback](../../1.0/example-01-feedback.md) §3), from the owner's third playtest (2026-10-24, N1–N21). 01 (i) (M83, 0.104, `D-212`) took the code-and-data notes; 01 (k) takes scenery, the aqueduct, the lava and the start. Every stop runs on Opus 5.5 at max by its catalog tier (`D-195`), since each draws art and makes sound.
- Kit first: a kit gap becomes a kit step or a backlog row, never a workaround in 01 (§6 of the feedback plan). Two gaps land here as additive builders on the existing `PlatformerController`: K7's shove and K18's step. The card prices them as additions with ledger rows and `just api`. The feedback plan's Q28 leaves the level at B unless the change needs an API sketch. §2.1 sketches it anyway, and Q1 keeps B.
- The decisions cited: `D-158` (the controller: casts against the last step, dynamic bodies block it), `D-172` (data is the API: a builder's setting gets a scene field, as `D-185` gave the top-down mover's rates), `D-174` (one driver per kinematic body), `D-175` and `D-177` (the pipeline's flash and the ailment ladder), `D-209` (01 (f)'s pushes and holes, which this changes), `D-212` (the cold ladder at 0.15 and 0.5 with 6 s windows) and `D-194` (the stops, which the landing decision amends).
- Later candidates expect nothing new from it. 01 (k) works on other files, apart from `tools/` and the README. The Phase 6 QA pass then reads the whole tree.

## 1. Audit

Read at `7d83d48` by four read-only audits (the pushes and steps; the force push; the black hole; the chill and the sounds), with every anchor below checked by hand and the kit's controller read whole. Paths under `examples/01_platformer/` are written from there unless rooted.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The controller steps on `&World`. Its move is one `move_and_slide` cast loop, and a wall zeroes its stored horizontal speed. Dynamic bodies block it and hold it up as walls and floors do, and "it never pushes one". The doc says so, and so do `D-158` and DESIGN. | `crates/tungsten-kit/src/motion/platformer.rs:478-612` (`:573-592`), `:82-84`; `DESIGN.md:491` |
| A2 | `PlatformerController::push(velocity)` already exists: a velocity added at the next step. The hole's pull on walkers uses it, it is saved, and two kit tests cover it. So the new builder cannot be named "push". | `crates/tungsten-kit/src/motion/platformer.rs:398-411`, `:774`; `src/features/toys/systems.rs:348-349`; `crates/tungsten-kit/src/tests/platformer.rs:333`, `:525` |
| A3 | `move_and_slide` and `probe_ground` are crate-private. The top-down mover and the knockback share them, so changing their behaviour would move other items. Its `Slide` reports a floor's body but not a wall's. | `crates/tungsten-kit/src/motion/slide.rs:37-89`, `:93-101`; `crates/tungsten-kit/src/motion/top_down.rs:454`; `crates/tungsten-kit/src/motion/knockback.rs:422` |
| A4 | The controller's saved form and its scene definition refuse unknown fields. Its settings are saved, its filter is not, and 01's save fixtures for schemas 8–13 hold controllers. | `crates/tungsten-kit/src/motion/platformer.rs:736-785`, `:89-93`; `tests/game/fixtures/schema8.json`–`schema13.json` (`"push":[0.0,0.0]`) |
| A5 | A kinematic body has inverse mass 0, so no contact moves it. A body's island wakes on an external `Velocity` write, and islands sleep under 20 px/s for 0.5 s. | `crates/tungsten-core/src/physics/step/mod.rs:58-80` |
| A6 | The kit hash is pinned at `0x3334_9665_de0c_501a`. It hashes positions, health, stats, buckets and more in the four genres' fixtures, so a builder that is off by default must leave it alone. The kit bench has no controller row, and no perf-suite row runs the kit or 01. | `crates/tungsten/src/tests/kit_recipes/determinism.rs:36`, `:71-140`, `:540-550`; `crates/tungsten-kit/benches/kit.rs:1-44`; `examples/02_bench/` (no kit) |
| A7 | `api/tungsten-kit.txt` lists each controller method twice: the root path and the prelude's. The components snapshot holds `platformer_controller`'s seven fields. The ledger's last kit row is M79's. | `api/tungsten-kit.txt:2583`, `:6370`; `api/tungsten-kit-components.txt:204-213`; `docs/plans/1.0/w04-api-freeze.md:133` |
| A8 | The wizard's and the walkers' controllers come from prefabs. 01 sets walkers' filters in code at a spawn, at a run's start and after a load (`walk_on_rock`). A frozen flyer borrows a fresh controller with the filter. The player's saved controller is inserted as it was saved. | `src/features/enemies/mod.rs:86-100`, `:356`, `:446`; `src/features/enemies/systems.rs:589-606` (`:598`); `src/save.rs:1303-1328` (`:1320`), `:1608-1612` |
| A9 | Nothing gives a kinematic mover a mass. `KindInfo.mass`, read only by a thrown enemy's impact, gives the crawler 40, the beetle 120, the troll 600 and the goblin 50. | `src/features/enemies/mod.rs:109-125`, `:154`, `:170`, `:202`, `:218`; `src/features/elements/systems.rs:1005` |
| A10 | The dynamic bodies' masses: small ball 1, bowling ball 8, rock 6, bomb 3, crate 12, keg 16, scrap 25, dummy 120, iron 400. A frozen enemy stays kinematic. | `tools/content/prefabs/{bowling_ball,goblin_rock,goblin_bomb,crate,keg,iron_scrap,dummy,iron_block}.json` (`"mass"`), core's default 1; `src/features/combat/draw.rs:50` |
| A11 | Walkers' AI (`blocked`, and the crawler's copy) counts a wall or the ground under the toes only as a tile or a static body, the first ray hit winning. So a dynamic body hides a wall behind it. A drop of more than 14 px under the toes reads as a ledge, and a walker standing on a ball would rest there. In the sheet's run, a beetle walked up to a small ball and stood pressing into it: its toes' ray began inside the ball and found the floor. A taller walker's ray meets the ball first and reads a ledge. | `src/features/enemies/systems.rs:111-136`; `src/features/enemies/crawler.rs:50-66` |
| A12 | The goblin hops a crate, a rock, a ball or a wall up to `HOP_CLEAR`, and turns at a keg or a bomb. The feedback plan's "keeps its hop for crates and kegs" is today's crate hop, and its keg turn stays. | `src/features/enemies/goblin.rs:201-243` (`:221`), `:93-95` |
| A13 | The jump's shove (`unstick`) kicks the bodies over and beside the wizard on a jump press and relaunches a blocked jump once. It stays. | `src/features/player/systems.rs:92-200`; `tests/game/player.rs:378` |
| A14 | A keg, like a frozen body, blows at a contact closing at `SHATTER_SPEED` 600 px/s. A 50 px fall reaches that, so a walker that pushed a keg off a ledge would set it off. A moving body hurts at 400 px/s past `BODY_HIT_SPEED`, nothing under the floor of 2. | `src/features/elements/mod.rs:58`, `:68`, `:90`; `src/features/elements/systems.rs:1163-1166`, `:1197-1204` |
| A15 | The playthrough's last leg walks into east_bank's two crates and keg, 5 px from the east wall, and passes at x > 424 tiles. A `LAUNCHES` route lands in the slot past the mine's four iron blocks (400, never shoved). | `tests/game/playthrough.rs:327-337`; `tests/game/level.rs:144` |
| A16 | The force push is `force_push(world, centre)`: `radial_push` over 384 px at 2,500 px/s, a 768 px `Ring::Push` and `ex10_warp_burst`. It ignores the aim computed beside it. `radial_push` is also the missile's blast (`explode`), whose falloff a test pins. | `src/features/spells/systems.rs:261-269`; `src/features/toys/mod.rs:114-118`, `:527-537`; `src/features/toys/systems.rs:579`, `:678-710`; `tests/game/spells.rs:115` |
| A17 | `KindInfo::pull` feeds both the hole's pull and `radial_push`, so the wave needs a weight of its own. The troll stands by `stands_firm` (resist 1). | `src/features/enemies/mod.rs:141`–`:221`, `:497-501`; `src/features/toys/systems.rs:338-346`, `:702` |
| A18 | A walker's knockback launches its controller and locks its input for its 0.2 s, holding the horizontal speed with no friction. So a throw's length is its speed times its lock, and the troll's throws already lock longer by hand. A flyer's knockback is a push fading over 0.2 s. | `crates/tungsten-kit/src/motion/knockback.rs:231-253`; `crates/tungsten-kit/src/motion/platformer.rs:518-521`; `src/features/enemies/troll.rs:212-231`; the prefabs' `knockback.seconds` |
| A19 | The push's test pins the instant, all-round push. It calls `force_push` with no aim, and its troll stands behind the cast point. | `tests/game/enemies.rs:2386` |
| A20 | The sprays draw as animated sprites, one entity a part, turned to the aim. A left aim mirrors by a negative `scale.y`, and each frame is a PNG with no sheets. | `src/features/toys/spray_look.rs:140-165`; `tools/effects.py:1050-1058`, `:1359-1374`; `tools/art.py:44-48`; `tools/generate.py:528-533` |
| A21 | The hole's pull is `HOLE_FORCE` × (1 − d/384) toward the core. A bowling ball takes it by its inverse mass, 0.125, so 1,500 px/s² against gravity's 3,600, and never lifts. Iron takes 0.2. A response of 0.625 lifts a bowling ball within 199.7 px. | `src/features/toys/systems.rs:237-272`; `src/features/toys/mod.rs:86-91`; `tools/content/prefabs/bowling_ball.json:22` |
| A22 | A flyer in a hole takes a fresh `knock_back` each step: about 2,100 px/s for a bat and 1,800 for a wisp at the core, against flying speeds of 260 and 140. The pull flips at the core with no damping, so they shoot through. The top-down mover takes a direction of at most length 1, and no outside velocity (K20). | `src/features/toys/systems.rs:338-352`; `crates/tungsten-kit/src/motion/top_down.rs:214-229`; `tools/content/prefabs/{bat,wisp}.json` |
| A23 | A hole opens with no look of its own beyond its prefab's emitters. Its `BlackHole { left }` is saved, so its age is `HOLE_SECS − left` with no schema change. Its drawing scales by the shrink alone. A test wants a fresh hole's quads non-empty at age 0. Two tests pull and two douse in the hole's first step, so a grow-in must stay visual. | `src/features/toys/mod.rs:465-478`; `src/features/toys/components.rs:87-94`; `src/save.rs:856-860`; `src/features/toys/draw.rs:96-191` (`:137`); `tests/game/presentation.rs:250`; `tests/game/spawning.rs:46`; `tests/game/brick.rs:290`; `tests/game/spells.rs:91`; `tests/game/props.rs:191` |
| A24 | The hole's opening sound is the one hand-made file, `black_hole.ogg`. It plays (a test pins one play at 0.65) but peaks at −32 dBFS against every other sound's −2. Once it leaves `HAND_AUTHORED_SOUNDS`, `--check` flags the leftover file, and generation will not delete it. | `tools/sfx.py:14`; `tools/sounds.json:2-6`; `tests/game/spawning.rs:178-216`; `tools/generate.py:630-639`, `:687-694` |
| A25 | The cold ladder's chill rung needs 0.15 of the threshold (6 s window), and the freeze 0.5. The first rung that passes empties the bucket, so the fill climbs to 0.15 with no chill shown, resets, then climbs to 0.5. Today nothing shows before the chill. The tint grades only while chilled, and the flakes are one 6 Hz emitter from the chill on. | `tools/content/data/damage_types/game.json:91-114`; `crates/tungsten-kit/src/damage/ailments.rs:555-559`; `src/features/combat/systems.rs:180-181`, `:206-272`, `:361-416`; `src/features/combat/mod.rs:49` |
| A26 | Overlays draw over every sprite, unlit, on the sprite's square, or the wizard's 64 px square on his body. The ice cube is fitted to each kind's hurtbox on its frame, with size classes 32, 64 and 128 for other bodies. | `src/features/combat/draw.rs:40-68`, `:91-175`; `tools/effects.py:656-705`; `tools/art.py:23-56` |
| A27 | Under the ice beam, each 0.1 s pulse is a non-periodic hit, which restarts the target's white hit flash (0.1 s on the enemies), so the target stays white and its tint cannot show. A periodic hit would not flash, but periodic parts do not feed the ladder. Found by the sheet's loops. | `crates/tungsten-kit/src/damage/pipeline.rs:899-901`; `crates/tungsten-kit/src/damage/ailments.rs:510`; `src/features/toys/systems.rs:1097-1101`; `tools/content/prefabs/*.json` (`hit_flash`) |
| A28 | A sound maps to its cue by its index in `Sound::ALL`, so a new variant appends. Every sound event needs a cue rule and a manifest entry. Keeping the IDs `ex10_force_push_sfx` and `ex10_black_hole_sfx` needs no new variant or rule. | `src/features/audio/components.rs:50-253`; `src/features/audio/mod.rs:82-151`; `tests/game/data.rs:184`; `tools/content/data/cue_rules/sounds.json:82-95`, `:187-200` |
| A29 | Content edits go under `tools/content/`. `generate.py --check` regenerates into a temporary folder and fails on any differing, missing, stale or unowned file, and `test_generate.py` wants two runs byte-identical. The generator needs Pillow 12.3.0. | `tools/generate.py:612-618`, `:687-694`, `:571`; `tools/requirements.txt` |
| A30 | The feedback plan's §3.6 test lines are stale: the push's test is at `tests/game/enemies.rs:2386` and the troll in a hole at `:1745`. | `tests/game/enemies.rs:2386`, `:1745` |
| A31 | A left-facing troll draws 20 px off its body (`mirror`), and the overlays draw at `Transform.position`, so its cube and the prototype's frost sit off its body. The sheet's frames show it (`perf-runs/20261024-e01j-art-review/shots/troll_left_*.png`). The cube test covers kinds with no shift. | `src/features/enemies/systems.rs:750-764`; `tests/game/enemies.rs:2319` |

## 2. Design

### 2.1 The kit's shove and step height (K7, K18)

Two builders on `PlatformerController`, off by default, so every other game, the fixtures and the kit hash stay as they are:

```rust
impl PlatformerController {
    /// The controller with a shove: of the dynamic bodies its horizontal
    /// move meets as walls, one of mass `m` up to `share` of `mass` (its
    /// own mass for shoves) takes the controller's horizontal velocity
    /// less `m / mass` of it, unless it already moves that way faster, and
    /// is woken. The controller stops at the body that step, as at a wall,
    /// then follows at the body's own pace, so it slows by `m / mass`; a
    /// body that cannot move holds it as before. A heavier body, a
    /// kinematic or static one and a tile block it as before.
    ///
    /// # Panics
    ///
    /// When `mass` is not finite and above 0, or `share` is not in (0, 1].
    #[must_use]
    #[track_caller]
    pub fn with_shove(self, mass: f32, share: f32) -> Self;

    /// The controller with a step height: on the ground, a horizontal move
    /// that meets a wall goes up to `height` px, across and back down in
    /// the same step when that lands it on a floor farther on, so it
    /// climbs a ledge or a body no taller than `height` with room above. A
    /// climb goes at least 0.15 of `height` across, and 0.29 of its radius
    /// more for a circle collider, so it reaches a round top's walkable
    /// part, and moves the body by its position rather than
    /// its velocity, so it shoves nothing it climbs. A kinematic body is
    /// never a step, and neither is a floor on one. 0, the default, climbs
    /// nothing.
    ///
    /// # Panics
    ///
    /// When `height` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_step_height(self, height: f32) -> Self;
}
```

- **Names.** `push` is taken (A2), so the builder is `with_shove`, and `with_step_height` keeps clear of the private `step` (Q2).
- **The step's order.** After today's move, a step that met a wall tries the climb when the controller is grounded and has a step height, and the wall is not a kinematic body. The climb casts up by the height (room above). It then moves across from there by the step's horizontal move, but by at least 0.15 of the step height, plus 0.29 of the controller's radius when its collider is a circle, in the move's direction. A cast's contact turns floor-like (`FLOOR_NORMAL_Y`, `crates/tungsten-kit/src/motion/slide.rs:22`) once the two shapes' rounded parts overlap past 0.286 of their summed radii from side contact: an obstacle circle no taller than the height needs 0.143 of the height, and a circle controller adds 0.286 of its own radius, round obstacle or box corner (`crates/tungsten-core/src/physics/query.rs:686-696`). The bare move (0.9 px a step for a patrolling beetle) would meet the ball's flank and turn back (§9, 1.1 and 2.2). Last, it casts down by what it rose plus the ground probe. It takes the result only when that lands on a floor that is not a kinematic body, farther on than the plain move. Otherwise a shove applies when set: the first wall met that is a dynamic body light enough, the controller moving into it. A step it can climb comes first, so a walker gets over a ball rather than herding it.
- **The climb's move.** The physics step moves a kinematic body along one straight velocity (`crates/tungsten-core/src/physics/step/mod.rs:953-956`), and a straight line from the start to the landing cuts the climbed body's corner, which would shove a dynamic one. So an accepted climb sets the body's `Position` to the landing the casts cleared and its `Velocity` to zero for that step, and the runner writes both (§9, 1.2). `PrevPosition`, snapshotted at the step's start (`crates/tungsten-core/src/physics/plugin.rs:56-68`), draws it rising over the frame. The controller stays its body's one driver (`D-174`), and its own velocity keeps the step's run for the animator.
- **The shove's write.** `step` runs on `&World` (A1), so it returns the body and the velocity, and the runner writes `Velocity.x` and wakes the body after the step (A5). A body already moving that way faster keeps its speed, and two shovers on one body keep the faster. Only the horizontal component changes. The controller still stops at the body, so it never overlaps one, and a body pinned by a wall or a pile holds it as today. The shove reads the step's horizontal velocity, a launch's or a lock's too, so a charge pushes at the charge's speed.
- **`Slide`** gains the first wall's body and normal, crate-private. `move_and_slide` is otherwise unchanged, so the top-down mover and the knockback do not move (A3). The climb is a new crate-private function in `slide.rs`.
- **Data** (`D-172`, as `D-185`): `platformer_controller` takes `shove_mass` (> 0, none by default), `shove_share` (in (0, 1], 0.5 by default, read only with `shove_mass`) and `step_height` (≥ 0, 0 by default), checked at deserialization. These are three field lines in `api/tungsten-kit-components.txt` and two `pub fn` lines × two paths in `api/tungsten-kit.txt` (A7). The component's description sentence stays as it is, and there is no new type.
- **Saves:** `ControllerSave` gains `shove` (mass and share) and `step_height`. A field is left out when off and defaults when absent, so an older save, 01's fixtures among them, loads with both off (A4). The saved form of a controller with both off is unchanged byte for byte. `try_from` checks the same bounds as the builders. A game that sets these in code after a load, as 01 does its filter, sees no difference.
- **Docs:** the type's rustdoc steps (5, 8 and 10 of its list: a climb sets `Position`) and its "never pushes one" sentence, DESIGN.md:491, and known issues' controllers bullet (a shoved body moves by its own physics, a step behind the shover; a pinned body holds it; a climb moves the body by its position, not swept).

### 2.2 01's shoves and steps (N6, N19)

- **The wizard:** `with_shove(60, 0.5)`, so he walks aside a small ball (1), a bowling ball (8), a rock (6), a bomb (3), a crate (12), a keg (16) and a scrap (25), slowed by their share (a crate 20 %, a keg 27 %). The dummy (120) and iron (400) stop him. He has no step. A shove at his 560 px/s run lands no body hit (a crate at 448 px/s closing deals 1.15, under the floor of 2) and sets off no keg (448 < 600; A14). It is set at `player::spawn` and on a load (`save::restore` after the saved controller goes in, A8), by `player::shove_bodies`, so a 0.104 save shoves too.
- **Walkers** (crawler, goblin, beetle, troll): `walk_on_rock` adds `with_shove(mass, WALK_SHOVE_MOST / mass)`, the kind's `KindInfo.mass` with `WALK_SHOVE_MOST` 8: a walker pushes small balls, bowling balls, rocks and bombs, never a crate or a keg, so no walker pushes a keg off a ledge (A14). It also adds `with_step_height(WALK_STEP)`, half a tile, 32 px. A frozen flyer's borrowed controller keeps the filter alone, at its swap and at a load, since `walk_on_rock` skips a flyer.
- **The walkers' checks:** `blocked` and the crawler's copy look past dynamic bodies, to the first tile or static body (a ray restarted inside the body it met, which it then ignores). A ball in front no longer hides a wall behind it. A drop to solid ground within `WALK_STEP` more than today's is no ledge, so a walker on a ball walks on rather than resting. A tile ledge, at least 64 px, still turns it.
- **The goblin** keeps its hop over crates, rocks and balls and its turn at kegs and bombs (A12, Q7). `unstick` stays for the jump (A13).

### 2.3 The force push as a wave (N4, N20)

`toys::wave` (`toys/wave.rs`) replaces `force_push` for the spell. `radial_push` stays for the blast (A16).

- **The front:** from the wizard's centre plus 24 px along the aim (the push gesture's free hand, Q8), at 1,200 px/s for 0.3 s, so its reach is 360 px. Its sector's half angle widens from 15° to 30°. In a step whose front moves from radius `r0` to `r1` at speed `S`, it crosses a body or an enemy that meets it inside the sector during the step, taken at the meeting:
  - The target, at distance `a` along the aim and `c` across at the step's start, with velocity `v`, is past the front (`a > r0`) and meets it within the step: `t* = (a − r0) / (S − v·aim)` with `0 ≤ t* ≤ dt`.
  - At `t*` its distance across, `|c + (v·across) × t*|`, lies within the sector's half width at the front's radius then, `(r0 + S × t*) × tan(half)`, plus half its own smallest extent.

  So neither a target running into the front nor one crossing it on a slant slips between two steps (§9, 1.3 and 2.1). The extent is there because the sheet's prototype found a low enemy beside the wizard missed by the bare sector (§2.8).
- **The push:** once a target (`Wave.crossed`), along the aim, by the falloff `1 − 0.5 × along / 360`.
  - An enemy takes `knock_back` (its kind's resist applying) of `(aim × WAVE_THROW + (0, −WAVE_HOP)) × falloff × weight`, with `WAVE_THROW` 500 and `WAVE_HOP` 300 (a flyer takes no hop). A walker's input then locks for `WAVE_LOCK` 0.35 s, so its throw is the speed held through the lock (A18).
  - The weights are the wave's own (`KindInfo::wave`, Q9): crawler 1.0, goblin 0.86, beetle 0.53, bat 4.0, wisp 3.5, troll 0.14.
  - The troll is pushed back a little (the owner's review of the sheet). A kind that stands firm (`stands_firm`, its knockback's resist 1) would take nothing through `knock_back`, which scales the push by `1 − resist` and still locks its input (`crates/tungsten-kit/src/motion/knockback.rs:238-244`). So the wave launches its controller with the push and the same lock itself, as the troll's own throws launch what it holds (`src/features/enemies/troll.rs:212-231`). The blast and the hole leave it standing as today.
  - A dynamic body takes `(aim × 1,800 + (0, −500)) × falloff × inv_mass` and is woken.
  - Measured in the prototype at 1.5 tiles: crawler 158 px, goblin 135, beetle 42, bat 163, wisp 158, troll 22; at 4.5 tiles 111, 95, 29, 123, 111 and 16.
- **The look:** `ex10_push_wave` (6 frames of 72 × 128 at 40 ms, looping, sampled linearly). It is a crescent of bent air bowed forward: a bright seam with a gravity-purple fringe, two ripples behind it and trailing speed streaks. It is tinted `[236, 214, 255]` and placed with its seam at the front, turned to the aim (a negative `scale.y` for a left aim, as the sprays). It is `along` 1 → 1.35 long and as wide as the sector there plus 22 px, fading in over its first 12 % and out over its last 30 %, at z 11.
- **Dust where it scrapes:** 9 samples across the front, each a ray 40 px down to a tile or a static body. Each raises an `ex10_push_dust` puff (7 frames of 48 × 32 at 55 ms, once, its foot on the floor, mirrored for a left aim) every 44 px it moves, at most 12 a wave, each with an `ex10_push_grit` burst (6 grains).
- **Gone:** `Ring::Push`, `FORCE_PUSH_RADIUS`, `force_push`, `push` (whose only caller it was) and `ex10_warp_burst`, each used only by the push. Not saved: a wave lives 0.3 s, and a save loses only what its front had not reached, since an enemy's saved `Knockback` and controller carry a throw already running (`src/save.rs:589`, `:604`).
- **Sound:** `force_push()` re-synthesized as the wave's whoosh under the same ID and `Sound::Pushed` (§2.6).

### 2.4 The black hole (N9)

- **Bowling balls lift:** `BOWLING_PULL` 0.625 in `pull_response`, so a bowling ball lifts within 199.7 px of the core (A21). Iron keeps 0.2 and stays down.
- **The opening** (`HOLE_OPEN_SECS` 0.3), all presentation, so the pull and the douse act from the first step (A23):
  - The vortex's layers and streaks are drawn at `0.08 + 0.92 × (1 − (1 − age / 0.3)³)` of their size, from `age = HOLE_SECS − left`.
  - A closing ring, `Ring::Open`, goes from 2.4 × `HOLE_DIAMETER` (432 px) to 24 px as `(1 − p)^1.6` over 0.3 s, fading in over its first quarter. It is drawn with the hole's own `ex10_photon_ring` art tinted `[236, 180, 255]`, which read better than the shock ring scaled up.
  - An inward rush, `ex10_hole_open`: a burst of 72 sparks that `hole_particles` sets on a ring 230 px out (plus a share of each spark's speed) and draws in at 700 px/s, the burst's emitter carrying `OpeningOf(hole)`.
- **Flyers held** (Q10): the per-step `knock_back` on a top-down mover goes (K20 stays a backlog row). A new `toys_hold`, in the move phase before `TOP_DOWN_MOVER`, after the enemies' states have set their directions, steers a live flyer in a hole's reach on its own mover.
  - It steers toward a ring 40 px about the nearest core at the hole's pull (`HOLE_FORCE × (1 − d/384) × pull × ENEMY_PULL_SECS`), capped at the flyer's own `move_speed`.
  - The pull eases within 48 px of the ring and circles there at 0.55 of the flyer's speed.
  - It is blended with the flyer's own direction by `min(1, pull / speed)`, so near the core the hole holds it and at its edge the flyer's own steering wins.
  - It never keeps its correction. It notes each flyer's own direction before writing the blend, and a `toys_release` after `TOP_DOWN_MOVER` writes that back once the mover has moved. A direction a state sets once and keeps (the bat's dive, set in `enter_attack`, `src/features/enemies/bat.rs:138-153`, then only timed) is then blended from the same intent each step and is whole again when the hole closes or the flyer leaves its reach (§9, 1.4). The list lives one step in a resource, so nothing new is saved.
  - A frozen flyer, on its borrowed controller, takes the walkers' pull as today.
- **Sound:** a generated `black_hole_open()` under `ex10_black_hole_sfx` at 0.65, so the cue rule and the test stay (A24). `black_hole.ogg` is deleted and `HAND_AUTHORED_SOUNDS` empties.

### 2.5 The chill's build-up (N15)

- **The frost's level** (`combat::frost_level`) is for a creature that freezes, unfrozen. Before the chill it is `0.25 × fill / (CHILL_NEEDS × threshold)`. From the chill it is `0.25 + 0.75 × fill / (FREEZE_NEEDS × threshold)`. The threshold is read as `chill_grade` reads it, and `CHILL_NEEDS` 0.15 is restated and held to the data by `every_restated_constant_matches_its_data`, as `FREEZE_NEEDS` is. It climbs with the fill's two phases (A25).
- **Stages** (`frost_stage`): 1 below 0.25 (rime at the feet, before the chill), 2 from the chill (the legs), 3 from 0.5 (the body), 4 from 0.75 (a crust). The freeze's cube then takes over.
- **The overlay:** `ex10_frost_<kind>_<stage>` for the wisp, beetle, bat, troll, goblin and dummy, fitted to each one's hurtbox on its frame as the cube is (Q12), and `ex10_frost_<class>_<stage>` over the status box for a body with no fitted one. It is drawn before the cube's check, white and subtle, at the owner's review of the sheet's first draw (which drew it in cold blues with a dark rim). It is rime in the palette's whites (`Ψ`, `φ`, `x`, `ϱ`, a sparse `u`): a band at the foot that thickens each stage, patches, sides that frost up from the foot and fade toward the frost's top so no frame shows, thin white feathers, and a few crystals on its ragged edge. The crust adds patches and crystals standing out of its outline. Over the white flash a beam keeps a target in, it reads faintly, and the flakes carry the build-up there (A27, Q13). The crawler (immune) and the wizard (who only chills, flat) show none (Q14).
- **The tint**, whiter and gentler at the owner's review: before the chill it goes from white toward frost's tint by `level / 0.25`. From the chill it grades toward a pale `CHILL_DEEP` `[194, 229, 255]`, where today it grades toward frozen's `COLD_TINT`, which the freeze keeps. `FROST_TINT` goes from `[204, 237, 255]` to `[222, 241, 255]`.
- **The flakes:** `ex10_chill_flakes_{1..4}`, white and fine: pulse emitters of 1, 1, 2 and 3 crystals every 0.45 s (caps 3, 4, 7 and 10), from white to a pale blue as they fade. Each is its target's own `StatusEmitter`, which gains its stage and is spawned again when the stage changes, from stage 1 on (K21's way round, the burst cap not shared). The wizard's chill keeps `ex10_chill_flakes`, which takes the same white ramp in place of today's blue `ice_ramp`.
- **The left-facing troll** (A31): the overlays' origin takes the kind's mirror shift back off `Transform.position` when it faces left, so its frost and its cube sit on its hurtbox as they do facing right (Q16). The goblin's 4 px gets the same.

### 2.6 Sounds

Both 16-bit mono at 44,100 Hz, normalized to −2 dBFS, drawn in `sfx.py`'s toolkit. The proposals are in `perf-runs/20261024-e01j-art-review/sounds.py` and `sounds/`, beside today's two.

| ID | File | Length | What |
| --- | --- | --- | --- |
| `ex10_force_push_sfx` (re-made) | `force_push.wav` | 0.55 s | The wave: a soft push-off and a whump of bent air, a dense rush flying away and falling in pitch, a low warped tone bowing under it, a scuff of grit |
| `ex10_black_hole_sfx` (now generated) | `black_hole_open.wav` | 0.9 s | A hole tearing open: an inward suck rising over its 0.3 s opening, a deep whomp falling into its drone, a rumble and a faint glassy shimmer |

### 2.7 Saves

No schema change; 01's schema stays 14 (`src/save.rs:194`):

- The wave is not saved (§2.3).
- The hole's opening comes from its saved `left` (A23).
- The frost, the tint and the flakes are presentation from the saved buckets and tags.
- The shove and the step ride in the kit's saved controller, defaulting off. 01 sets its own at every spawn and load, so a schema 14 save from 0.104 walks and shoves at once.
- The component-coverage test (`tests/game/saves.rs:720`) sees no new component on a saved entity: `Wave`, `OpeningOf` and the stage on `StatusEmitter` sit on unsaved entities. Step 2 adds a test that a 0.104-shaped save (the fields left out) shoves after its load. The pit's 2 MiB bound gains about 70 bytes a controller, checked in step 2.

### 2.8 The art-review sheet and its proposals

Drawn in this session at `7d83d48` in `perf-runs/20261024-e01j-art-review/` (machine-local). Its tools:

- `proposals.py`: the wave, the dust, the frost and the configs, in the generator's toolkit, all on palette.
- `sounds.py`.
- `tools/`: `proto_frost.py` and `proto_toys.py`, which patch a scratch copy of 01; `wave.rs`, the prototype's wave; `shots.rs`, the driver; `build_run.py`, `shot.sh`, `loop.sh`, `export_loops.py` and `frost_preview.py`.

Its loops in `loops/` are real frames of that copy (`target/e01j-shots/`): the wave at a crawler, a beetle and the troll; at bodies; up at a bat and a wisp; the hole's opening and hold; the frost on a beetle, the troll, a goblin and the dummy under the beam's cold; and the beam itself on the troll. `frost_stills.png` shows each kind at stages 0–4 and frozen, over its frame and over its white flash. The owner's view of the loops, the stills, the throws and the sounds is a private review page, https://claude.ai/artifact/7zrhDsGmeU4CqZYp251JkW. The owner reviewed the first draw on 2026-10-24. The second draw, the page's current version, answers both notes: the troll pushed back a little by the wave, and the chill's frost, tint and flakes whiter, frostier and more subtle. The run takes these as defaults; an owner's note before the run amends this section, and one after it is a follow-up stop.

| What | Proposal |
| --- | --- |
| Wave | 1,200 px/s, 0.3 s, half angle 15° → 30°, from 24 px ahead of the centre; a target taken at its meeting with the front, with half its smallest extent in the front's test |
| Wave's push | enemies `(aim × 500 + (0, −300)) × falloff × weight`, walkers locked 0.35 s, flyers no hop; bodies `(aim × 1,800 + (0, −500)) × falloff × inv_mass`; falloff 1 → 0.5 at 360 px; weights crawler 1.0, goblin 0.86, beetle 0.53, bat 4.0, wisp 3.5, troll 0.14, the troll launched straight past its resist |
| Wave's art | `push_wave` 72 × 128, 6 × 40 ms, looping, linear filter, tint `[236, 214, 255]`, z 11; `push_dust` 48 × 32, 7 × 55 ms, once; `push_grit` burst of 6 `ex10_dust` grains thrown up in a 110° cone, 140–360 px/s, gravity 900, so it needs no turning for a left aim |
| Hole | `BOWLING_PULL` 0.625; `HOLE_OPEN_SECS` 0.3, grow-in from 0.08 (cubic ease-out); `Ring::Open` 432 → 24 px over 0.3 s on `ex10_photon_ring` `[236, 180, 255]`; `hole_open` 72 sparks from 230 px in at 700 px/s, lifetime 0.32–0.4 s |
| Hold | ring 40 px, ease 48 px, circling 0.55 of the flyer's speed, capped at its speed, blended by `min(1, pull / speed)` |
| Frost | 4 stages at level 0, 0.25 (the chill), 0.5, 0.75; reach 0.2, 0.45, 0.72 and 1.0 of the fitted box; white rime `Ψ` `φ` `x` `ϱ`, a sparse `u`, no dark rim (the owner's review); per kind on its frame (64, the troll's 128) and per class (32, 64, 128) |
| Tint | `FROST_TINT` `[222, 241, 255]`; the chill's grade toward `CHILL_DEEP` `[194, 229, 255]`; frozen keeps `COLD_TINT` |
| Flakes | pulses of 1, 1, 2, 3 every 0.45 s, caps 3, 4, 7, 10, speeds 10 to 28–36, scale 0.05 to 0.09–0.12, white fading to pale blue; the wizard's `chill_flakes` on the same white ramp |
| Shove and step (no art) | wizard 60 × 0.5; walkers up to 8; step 32 px, a climb at least 0.15 of it across, plus 0.29 of a circle controller's radius |

## 3. Steps

Every code step ends with `cargo fmt --all -- --check` → exit 0 and its crate's `cargo clippy -p <crate> --all-targets --locked -- -D warnings` → exit 0. Every step that touches `tools/` or the generated assets runs the generator from a scratch Pillow venv: `python3 -m venv <scratchpad>/level-venv && <scratchpad>/level-venv/bin/pip install -r examples/01_platformer/tools/requirements.txt`, then `<scratchpad>/level-venv/bin/python -B examples/01_platformer/tools/generate.py`, and checks with `PATH="<scratchpad>/level-venv/bin:$PATH" just level-check` → exit 0. The run deletes the venv at its end with `shutil.rmtree` (stop condition 3). Test filters go after `--`. 01's tests are the `game` target, the kit's are `motion::platformer::tests` in its library.

### Step 1: The kit's shove and step height

- **Files:** `crates/tungsten-kit/src/motion/platformer.rs`, `crates/tungsten-kit/src/motion/slide.rs`, `crates/tungsten-kit/src/tests/platformer.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/known-issues.md`.
- **Change:** §2.1, in this order:
  1. `Slide`'s wall body and normal.
  2. The climb in `slide.rs`.
  3. The builders, the fields, the step's climb and shove, and the runner's write.
  4. The scene fields and their descriptions.
  5. `ControllerSave` and `try_from`.
  6. Rustdoc.
  7. The tests.
  8. `just api`.
  9. The ledger row ("M84's kit API, for the freeze": the two builders, the three data fields, the saved form's two optional fields; behaviour a game may notice: none until a builder is set), DESIGN.md:491's sentence, and the known issues' bullet.

  The new tests in `tests/platformer.rs`:
  - `a_shove_moves_a_light_body_at_the_run_less_its_share`: mass 60 × 0.5 into a dynamic box of mass 12 on a floor. Within 30 steps the box moves at 0.8 of the run ± 5 %, the controller follows at the box's pace and never overlaps it, and the box is awake.
  - `a_body_past_the_share_blocks_as_before`: mass 40.
  - `a_pinned_body_holds_the_shover`: a light box against a static wall.
  - `a_shove_moves_no_kinematic_or_static_body`.
  - `a_shove_keeps_a_faster_body_s_speed`.
  - `the_step_climbs_a_low_box_and_not_a_tall_one`: step 16, boxes 12 and 20 px.
  - `the_step_climbs_a_ball_at_a_slow_walk`: run 50 px/s and step 32 against a dynamic circle of radius 15 on the floor. The controller ends on and past it, and the circle has not moved more than 0.5 px, so neither the climb nor its move shoves it.
  - `a_round_controller_climbs_a_ball_too`: the same with the controller's collider a circle of radius 15.
  - `the_step_needs_the_ground_and_room_above`: none in the air, none under a low ceiling.
  - `a_kinematic_body_is_never_a_step`.
  - `a_step_comes_before_a_shove`: a low light box is climbed, not pushed.
  - `the_scene_fields_build_the_same_controller`, with the refused values.
  - `a_save_keeps_the_shove_and_the_step_and_an_old_one_loads_them_off`.
  - `a_zero_shove_mass_panics`, `a_shove_share_above_one_panics`, `a_negative_step_height_panics`.
- **Done-when:**
  - `cargo test -p tungsten-kit --locked --lib -- motion::platformer::tests` → exit 0.
  - `python3 -B -c "import subprocess as s;o=s.run(['cargo','test','-p','tungsten-kit','--locked','--lib','--','motion::platformer::tests','--list'],capture_output=True,text=True).stdout;n=['a_shove_moves_a_light_body_at_the_run_less_its_share','a_body_past_the_share_blocks_as_before','a_pinned_body_holds_the_shover','the_step_climbs_a_low_box_and_not_a_tall_one','the_step_climbs_a_ball_at_a_slow_walk','a_round_controller_climbs_a_ball_too','a_kinematic_body_is_never_a_step','a_step_comes_before_a_shove','a_save_keeps_the_shove_and_the_step_and_an_old_one_loads_them_off'];m=[x for x in n if x not in o];assert not m,m"` → exit 0.
  - `cargo test -p tungsten --locked --lib -- kit_recipes kit_tests` → exit 0, `the_recipes_hash_the_same_twice_and_match_the_pin` among them, with `KIT_RECIPES_HASH` unchanged: `rg -c '0x3334_9665_de0c_501a' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 1.
  - `cargo test -p tungsten-kit --locked --doc` → exit 0.
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0.
  - After `just api`: `python3 -B -c "import subprocess as s;d=s.run(['git','diff','--unified=0','--','api/tungsten-kit.txt'],capture_output=True,text=True).stdout.splitlines();a=[l for l in d if l.startswith('+') and not l.startswith('+++')];r=[l for l in d if l.startswith('-') and not l.startswith('---')];assert len(a)==4 and not r and sum('with_shove(self, f32, f32) -> Self' in l for l in a)==2 and sum('with_step_height(self, f32) -> Self' in l for l in a)==2,(a,r)"` → exit 0.
  - The same check on `api/tungsten-kit-components.txt` with three `+  field` lines, `shove_mass`, `shove_share` and `step_height`, and no `-` line → exit 0.
  - `just api-check` → exit 0.
  - `rg -c "M84's kit API" docs/plans/1.0/w04-api-freeze.md` → 1.
  - `rg -q 'it never pushes one' DESIGN.md; test $? -eq 1` → exit 0.
  - fmt → exit 0; `cargo clippy -p tungsten-kit --all-targets --locked -- -D warnings` → exit 0.
- **Moves:** the `api/` snapshots (additions only); no hash.

### Step 2: 01's shoves and steps

- **Files:** `examples/01_platformer/src/features/enemies/{mod.rs,systems.rs,crawler.rs}`, `examples/01_platformer/src/features/player/mod.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tests/game/{player.rs,enemies.rs,saves.rs}`, `examples/01_platformer/README.md`.
- **Change:** §2.2:
  - `player::shove_bodies` with `PLAYER_SHOVE_MASS` 60 and `PLAYER_SHOVE_SHARE` 0.5, called in `player::spawn` and in `save::restore` after the saved controller goes in.
  - `WALK_SHOVE_MOST` 8 and `WALK_STEP` 32 in `enemies`. `walk_on_rock` adds both for a walker and skips a flyer.
  - `solid` and `blocked` (and the crawler's pair) look past dynamic bodies, with the ledge's depth grown by `WALK_STEP`.
  - The README's controller and pushes rows.

  Tests:
  - `player::the_wizard_walks_small_bodies_aside`: a small ball and a crate ahead move with him, his pace past the crate is 448 ± 10 % px/s, and iron stops him.
  - `saves::a_wizard_loaded_from_an_older_save_still_shoves`: a capture through JSON with the controller's shove and step fields removed, loaded, then the ball test's push.
  - `enemies::a_walker_steps_over_balls_on_its_way`: a beetle on patrol, the wizard past its sight, walks over a small ball and a bowling ball in its path; its centre passes both within 300 steps, grounded at the end. A beetle, since a crawler's lunge leaps balls today, as the sheet's run showed.
  - `enemies::a_walker_leaves_crates_and_kegs_where_they_stand`: a beetle on patrol into a crate, then a keg, moves neither by more than 1 px.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- player:: enemies:: saves:: level:: playthrough::` → exit 0, the four new tests listed by `--list`, and `a_goblin_hops_a_crate_in_its_way`, `a_jump_under_small_balls_shoves_them_off_and_rises_clear`, `pit_full_saves_within_two_mib` and every `level::` route among the passes.
  - `rg -l 'with_shove|with_step_height' examples/01_platformer/src` → prints `examples/01_platformer/src/features/enemies/mod.rs` and `examples/01_platformer/src/features/player/mod.rs` and nothing else.
  - fmt → exit 0; `cargo clippy -p example-01-platformer --all-targets --locked -- -D warnings` → exit 0.
- **Moves:** none.

### Step 3: The force push as a wave

- **Files:** `examples/01_platformer/src/features/toys/{wave.rs,mod.rs,components.rs,draw.rs}`, `examples/01_platformer/src/features/spells/systems.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/tools/{effects.py,art.py,generate.py,sfx.py,README.md}`, the generated `examples/01_platformer/assets/` and `tools/outputs.json`, `examples/01_platformer/tests/game/{enemies.rs,spells.rs}`, `examples/01_platformer/README.md`.
- **Change:** §2.3 and §2.8. It starts from the prototype's `perf-runs/20261024-e01j-art-review/tools/wave.rs`, tightened to 01's rules: no hand-picked asset paths, the `ex10_` IDs, its systems registered in `ToysPlugin` (the front in the move phase, the look in the present phase).
  - `KindInfo::wave` holds the weights. A kind that stands firm (the troll) takes the push through its controller's `launch` and the lock, past its resist.
  - `Spell::ForcePush` casts `toys::wave::cast(world, centre, aim)`.
  - `Ring::Push`, `FORCE_PUSH_RADIUS`, `force_push`, `push` and `warp_burst` go.
  - The art: `push_wave` and `push_dust` in `effects.py` from `proposals.py`, sized in `art.py`, their clips and the `push_grit` config in `generate.py`, `push_wave` sampled linearly.
  - `force_push()` in `sfx.py` from `sounds.py`. The READMEs.

  `enemies::a_force_push_throws_enemies_by_distance_and_the_troll_stands` (`tests/game/enemies.rs:2386`) becomes `enemies::the_wave_throws_each_kind_along_the_aim_the_troll_a_little`: at 1.5 tiles along a level aim, each alone, a crawler 140–175 px, a goblin 118–150, a beetle 32–50, a bat and a wisp over 100, the troll 12–35 (pushed back a little, past its resist). Two more tests:
  - `enemies::the_wave_pushes_once_and_never_behind`: a ball's velocity changes on one step only, and an enemy behind the wizard is untouched.
  - `spells::the_wave_meets_a_body_running_into_it`: a ball flying at the wizard at 900 px/s is pushed back, placed where the stationary span test of the prototype skips it (just past the first step's span); and one on a slant, `(along, across)` = (130, 65) px from the front's start at 100 px and moving at (−900, −900) px/s, the round-2 case, is pushed too.
  - `spells::a_wave_lives_three_tenths_of_a_second`: no `Wave` and no look left after 19 steps.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- enemies:: spells:: data::` → exit 0, the four tests listed, and `a_blast_pushes_dynamic_bodies_away_by_distance_and_mass` passing unchanged.
  - `rg -q 'Ring::Push|warp_burst|FORCE_PUSH_RADIUS|fn force_push' examples/01_platformer/src examples/01_platformer/tools/generate.py; test $? -eq 1` → exit 0.
  - `PATH=… just level-check` → exit 0.
  - fmt and clippy (`example-01-platformer`) → exit 0.
- **Moves:** none.

### Step 4: The black hole

- **Files:** `examples/01_platformer/src/features/toys/{mod.rs,systems.rs,components.rs,draw.rs}`, `examples/01_platformer/tools/{generate.py,sfx.py,sounds.json,README.md}`, the generated `assets/` (`sounds/black_hole.ogg` deleted by hand) and `tools/outputs.json`, `examples/01_platformer/tests/game/{enemies.rs,spells.rs,spawning.rs,presentation.rs}`, `examples/01_platformer/README.md`.
- **Change:** §2.4 and §2.8. It starts from the prototype's patch (`tools/proto_toys.py`):
  - `BOWLING_PULL`.
  - `HOLE_OPEN_SECS`, the drawn grow-in, `Ring::Open` on the photon ring, and `OpeningOf` with `hole_open`'s placement in `hole_particles`.
  - `toys_hold` before `TOP_DOWN_MOVER` and `toys_release` after it, with `pull` leaving top-down movers to them.
  - `black_hole_open()` in `sfx.py`, `sounds.json`'s path, `HAND_AUTHORED_SOUNDS = ()`, `black_hole.ogg` deleted, and the READMEs.

  Tests:
  - `spells::a_hole_lifts_bowling_balls_within_200_px_and_leaves_iron`: a bowling ball 180 px under the core rises, one 230 px out stays on the floor, and iron 120 px out stays.
  - `enemies::a_hole_holds_a_bat_and_a_wisp_about_its_core`: each alone, placed 250 px out with nothing else near. Once within 60 px of the core it stays within 100 px for the rest of the hole's life, where today's renewed knockback flings it out and back through the core.
  - `enemies::a_bat_diving_through_a_hole_dives_on_its_own_way_after`: a bat diving past an open hole that collapses mid-dive has its dive's own direction back the step after.
  - `presentation::a_hole_opens_over_three_tenths_of_a_second`: the vortex's quads at age 0 are 0.08 of their size at 0.3 s ± 0.01, `Ring::Open` spawns at the press with size 432, and the rush's emitter carries `OpeningOf`.

  `knockback_the_black_hole_balls_and_iron_barely_move_the_troll` (`tests/game/enemies.rs:1745`) passes, its setup moved only if a lifted bowling ball now strikes the troll (recorded in the evidence row).
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- spells:: enemies:: spawning:: presentation:: brick:: ball_pit:: props:: dummy:: saves::` → exit 0, the four tests listed, and `spawn_black_hole_system_creates_attractor_at_cursor_on_right_click` (one play of `ex10_black_hole_sfx` at 0.65) passing unchanged.
  - `test ! -e examples/01_platformer/assets/sounds/black_hole.ogg` → exit 0.
  - `rg -q 'black_hole\.ogg' examples/01_platformer/tools/sfx.py examples/01_platformer/tools/sounds.json; test $? -eq 1` → exit 0.
  - `PATH=… just level-check` → exit 0.
  - fmt and clippy → exit 0.
- **Moves:** none.

### Step 5: The chill's build-up

- **Files:** `examples/01_platformer/src/features/combat/{mod.rs,systems.rs,components.rs,draw.rs}`, `examples/01_platformer/tools/{effects.py,art.py,generate.py,README.md}`, the generated `assets/` and `tools/outputs.json`, `examples/01_platformer/tests/game/{ice.rs,data.rs,saves.rs}`, `examples/01_platformer/README.md`.
- **Change:** §2.5 and §2.8. It starts from the prototype's patch (`tools/proto_frost.py`):
  - `CHILL_NEEDS`, `frost_level`, `frost_stage`, the tint before the chill, `FROST_TINT` whiter and `CHILL_DEEP` for the chill's grade, `StatusEmitter.stage` and the flakes by stage, and the overlay in `draw.rs`.
  - The frost art in `effects.py` from `proposals.py` (each kind's frame and the classes, sized in `art.py`, with a `check_ice_fits`-like hold of each frost sprite's pixels inside its kind's fitted box grown by 6 px, room for its crust's spikes; the troll's and the classes' sprites listed in `ELEMENT_SIZES`, 128 and 32 or 128 px, since no prefix sizes them), and the four flake configs in `generate.py`, whose `chill_flakes` takes the same white ramp.
  - A left-facing troll's boxes (§2.5, Q16).

  Tests in `ice.rs`:
  - `the_frost_climbs_in_four_stages_from_the_first_point_of_cold`: a beetle under 1.5 cold every 0.1 s reads stage 1 before its chill, 2 at it, then 3 and 4 before its freeze, and none frozen.
  - `the_frost_draws_each_kind_s_own_overlay`: the overlay batch holds `ex10_frost_beetle_<n>`, the dummy's, and a class's for a body with no fitted box.
  - `the_tint_turns_toward_frost_before_the_chill`: whiter, toward `FROST_TINT` `[222, 241, 255]`, and from the chill toward `CHILL_DEEP`.
  - `the_flakes_thicken_by_stage`: the emitter's config is `ex10_chill_flakes_<stage>`, and the wizard's chill keeps `ex10_chill_flakes`.
  - `a_left_facing_troll_s_frost_and_cube_fit_its_hurtbox`.

  In `data.rs`, `CHILL_NEEDS` is held to the chill rung's `needs`.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- ice:: combat:: data:: enemies:: saves:: presentation::` → exit 0, the five tests listed, and `a_chill_slows_and_frosts_more_as_its_bucket_fills` passing unchanged.
  - `PATH=… just level-check` → exit 0.
  - fmt and clippy → exit 0.
- **Moves:** none.

### Step 6: The test pass

- **Files:** none beyond steps 1–5, for a failing test's fix in the step that owns it.
- **Change:** run 01's whole suite, the kit's and the umbrella's, the generator check and smoke. A failure goes back to its step's files, or skips that step per stop condition 1.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked` → exit 0.
  - `cargo test -p tungsten-kit --locked` → exit 0.
  - `cargo test -p tungsten --locked --lib -- kit_recipes kit_tests` → exit 0.
  - `PATH=… just level-check` → exit 0.
  - `just smoke` → exit 0.
  - `cargo fmt --all -- --check` → exit 0.
  - `just lint` → exit 0.
- **Moves:** none.

### Step 7: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/example-01-feedback.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/known-issues.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, this plan.
- **Change:**
  - The landing decision through tungsten-decision (§5): what landed, with the measured throws, the lift and the hold's numbers. It amends `D-158` (the shove and the step), `D-194` (the stops) and `D-209` (the radial force push replaced, the flyers' pull), with their marker lines and index rows.
  - The feedback plan's §3.6 gains a "Landed (M84, 0.105, `D-21[3]`)" paragraph. Its §6 changes: K7 and K18 fixed in M84, K20 and K21 homed in the backlog, a K28 row for the beam's flash (A27), beside K27, which the 01 (l) scoping wrote for a shield's hits flashing from the same `start_flash` call.
  - The backlog: K7's row keeps "is pushed by them" only, and K20, K21 and K28 gain rows.
  - Known issues: anything step 6 left.
  - The register row's plan column, filled at the plan session, keeps.
- **Done-when:**
  - `rg -c '^## D-21[3] — ' DECISIONS.md` → 1.
  - `rg -c '^\| `D-21[3]` \|' docs/DECISION_INDEX.md` → 1.
  - `rg -c 'Amended by `D-21[3]`' docs/DECISION_INDEX.md` → 3.
  - `rg -c 'Amended by D-21[3]' DECISIONS.md` → 3 (the marker lines on `D-158`, `D-194` and `D-209`).
  - `rg -c 'Landed \(M84' docs/plans/1.0/example-01-feedback.md` → 1.
  - `rg -c 'K28' docs/plans/1.0/example-01-feedback.md` → 1 or more.
  - `just repo-check` → exit 0.
  - `just ctx` → exit 0.
- **Moves:** none.

### Step 8: Release 0.105

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`).
- **Change:** close workflow §5 and this step's evidence row with `status: done`. Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)):
  1. Archive the plan.
  2. Run `just release-cut 0.105.0` once.
  3. Run every release check once after it, with `PATH=… just level-check` as the gate by change no step re-runs after the cut.
  4. Run the preflight with `--message`.

  The register row records the cut and `D-21[3]`; the CHANGELOG line names the six notes.
- **Done-when:**
  - [releases](../../../releases.md) step 1's command chain, plus `PATH=… just level-check` → every check passes, quoted in the report (the plan is archived before the cut).
  - `just release-preflight 0.105.0 --repo JosephGibson/Tungsten --message 'Update 0.105: M84 example 01 pushes and spells'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for 01 (k) scenery, aqueduct and start.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Level B or C: the card's two builders, or an API sketch's level | B: two additive builders and three optional data fields on an existing item, no new type, the semantics the feedback plan settles; §2.1 sketches it for the critique anyway |
| Q2 | The builders' names, `push` being taken (A2) | `with_shove(mass, share)` and `with_step_height(height)`; data `shove_mass`, `shove_share`, `step_height` |
| Q3 | Data fields for the builders | Yes, as `D-185` gave the top-down mover's rates (`D-172`: data is the API); 01 sets its own in code, beside its filter |
| Q4 | Saved | Yes, left out when off, defaulting off; 01 re-applies its own at every spawn and load, so no schema bump |
| Q5 | Whether a kinematic body is a step | Never: a moving deck is reached by a jump, and the elevator's riders and margin (`src/features/level/elevator.rs:42-44`, `:128-152`) stay as they are |
| Q6 | Who shoves what | The wizard up to 30 (60 × 0.5); walkers up to 8, so balls, bowling balls, rocks and bombs, never a crate or a keg, which a fall sets off (A14); the step 32 px, half a tile |
| Q7 | The goblin "keeps its hop for crates and kegs" | Today's hop: crates, rocks, balls and walls up to a tile; kegs and bombs still turn it (A12) |
| Q8 | "The wave leaves the staff" while the push gesture is the free hand's shove with the staff raised behind | From 24 px ahead of the centre along the aim, as the sprays start, where the hand shoves |
| Q9 | The wave's numbers the notes leave open | §2.8's, measured in the prototype: 158, 135 and 42 px at 1.5 tiles for the three walkers named. The troll is pushed back 22 px, settled by the owner's review of the sheet (2026-10-24): "the troll does get pushed back but just a little" |
| Q10 | The flyers' hold | §2.4's ring of 40 px, eased within 48 px, circling at 0.55 of the flyer's speed, capped at it; K20 stays a backlog row |
| Q11 | Bowling balls' lift | `BOWLING_PULL` 0.625, so within 199.7 px; iron stays at 0.2 |
| Q12 | The frost per size class (the feedback plan) or per kind | Per kind, fitted to each hurtbox as the cube is since the owner's 01 (c) review, with the three classes for a body with no fitted box |
| Q13 | The beam's white flash hiding the tint (A27) | Left: a hit that does not flash is kit API (K28, a backlog row). The owner's review chose white, subtle frost over the first draw's cold blues, which stood out over the flash; under a beam the frost reads faintly and the flakes carry it, and between beams it shows whole |
| Q14 | Frost on the wizard | None: he only chills, flat; his tint and flakes stay today's |
| Q15 | `Ring::Push` and `ex10_warp_burst` | Gone with the radial force push; the blast keeps `radial_push` |
| Q16 | A left-facing troll's overlays sit 20 px off its body (A31, seen in the sheet) | Fixed in step 5 for the frost and the cube alike, since the frost draws from the same origin: the overlays' origin takes the mirror shift back off |
| Q17 | The owner's review of the sheet | Given 2026-10-24, before the run, and folded into §2.3, §2.5 and §2.8: the troll pushed back a little, the chill whiter, frostier and more subtle. A later note before the run amends §2.8 the same way; one after it becomes a follow-up stop |

Each question takes its stated default; the owner reviews the plan and the sheet on their own time (`D-150`).

## 5. Decisions expected

213: Example 01 (j) pushes and spells landed: the kit's `PlatformerController::with_shove` and `with_step_height` (K7, K18), 01's wizard shoving, walkers stepping and shoving the light, the force push as a wave that shoves even the troll a little, the hole lifting bowling balls, opening and holding flyers, the frost in four stages, white and subtle, with a paler chill tint. Amends `D-158`, `D-194` and `D-209`. Next free when written; checked again when step 7 runs (another session may take it).

## 6. Invariants

- **May move:** the `api/` snapshots (step 1, additions only: two `pub fn` lines × two paths, three field lines), recorded in the evidence log and the ledger row.
- **Must not move:**
  - the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, and the post and transition regressions (no core, render or engine file changes);
  - the kit hash `0x3334_9665_de0c_501a` (both builders off in every fixture);
  - 01's save schema 14;
  - the template and 01's `src/shell/` copy (`just repo-check`);
  - the missile's blast (`a_blast_pushes_dynamic_bodies_away_by_distance_and_mass`).

## 7. Stop conditions

1. A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with the steps that do not depend on it. Step 2 needs step 1. Steps 3–5 stand alone. Step 6's fixes stay inside the failing step's files.
2. The kit hash moves, or an existing kit test fails with both builders off: step 1 stops and is restored, and step 2 skips with it.
3. The Pillow venv cannot install: steps 3–5 are skipped whole, since their code needs their art. Step 6's `level-check` is recorded not run, and the release waits for a session that can run it.
4. A walker can only get over a ball by stepping onto a kinematic body, or the elevator's tests fail on a walker riding it: step 2 keeps the shove, drops the step, and records it.
5. `pit_full_saves_within_two_mib` fails on the controllers' new fields: step 2 stops and records the size; a schema-trimmed form is a follow-up.
6. A file outside §files to touch needs a change: a follow-up, not an edit.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–6, release | No perf row runs the kit's controller or 01 (A6), so no capture; `level-check` in steps 3–6; the release runs `just check`, `just smoke`, `just script-test` and the rest once after the cut, with `level-check` |
| 3 Invariants | 1, 6, release | The kit hash in step 1; the others unchanged by construction, the release checks confirming |
| 4 Decisions | 7 | One landing decision, its index row and three marker lines |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1 | Additions only, a ledger row, rustdoc on both builders |
| 6 Template and guide | n/a | The template's player is top-down; no template or guide change |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 7 | `toys/wave.rs` sits under the indexed `src/features/` row; `just ctx` passes |
| 8 Design | 1 | DESIGN.md:491's controller sentence |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 7, release | `python3 -B scripts/roadmap.py summary` supplies the owned fields; no gap-log row; `w13-kit.md` names no controller limit |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1, run 2026-10-25 (UTC):
- **Reviewer:** the `critique` skill with `--repo`, `gpt-6-astra`, on this plan at `7d83d48` with this section and the evidence log cut by the skill's `sed`.
- **Run:** effort xhigh, context 872,000, repo mode (output cap 4,000), 295.1 s, tokens in 1,104,465 (cached 1,010,432), out 9,095 (reasoning 4,830), 22 commands, output 302.1 KB (largest 29.2 KB).
- **Usage:** 5h 6% → 30%, week 31% → 35%.
- **Result:** four findings, all checked true and accepted. Round 1's rows are numbered 1.1–1.4.

Round 2, the same day, on the revised plan with this section and the evidence log cut, with a `--focus-file` naming the sections round 1 changed (no finding was rejected):
- **Run:** `--force`, since the cut plan and the note passed 60,000 characters (step 7 confirms it). Effort xhigh, 279.3 s, tokens in 756,993 (cached 672,640), out 8,837 (reasoning 5,145), 20 commands, output 324.3 KB (largest 43.6 KB).
- **Usage:** 5h 30% → 50%, week 35% → 38%.
- **Result:** two findings, both gaps the round-1 fixes left, both checked true and accepted (rows 2.1–2.2). There is no third round, and no finding stays open.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1.1 | [HIGH] One tick's horizontal travel is insufficient to step onto the balls: a patrolling beetle moves 0.9 px a step, and a bowling ball's top turns floor-like only about 4.3 px past its side | `crates/tungsten-kit/src/motion/slide.rs:22` (`FLOOR_NORMAL_Y` −0.7); `tools/content/prefabs/bowling_ball.json:16` (radius 15); `src/features/enemies/beetle.rs:24` (`PATROL_PACE` 0.6) and the beetle's run 90: 15 × (1 − √0.51) = 4.29 px | Accepted | §2.1: the climb goes at least 0.15 of the step height across, enough for any circle no taller than the step; step 1 adds `the_step_climbs_a_ball_at_a_slow_walk` |
| 1.2 | [MEDIUM] The physics step will not follow the climb's cast path: the body moves along one straight velocity, cutting the climbed body's corner and shoving a dynamic one | `crates/tungsten-core/src/physics/step/mod.rs:953-956` (kinematic proxies move by velocity each substep); `crates/tungsten-kit/src/motion/platformer.rs:611` (`(at − origin) / dt`) | Accepted | §2.1: an accepted climb sets `Position` to the landing and `Velocity` to zero for its step; the slow-walk test asserts the circle moves no more than 0.5 px |
| 1.3 | [MEDIUM] The wave can cross a moving target without hitting it: a body at 21 px moving to 9 px between two steps misses both spans | The prototype's `within` (`perf-runs/20261024-e01j-art-review/tools/wave.rs`), testing the current distance alone | Accepted | §2.3: crossed when `a > r0` and `a + (velocity · aim) × dt ≤ r1`; step 3 adds `the_wave_meets_a_body_running_into_it` |
| 1.4 | [MEDIUM] Hole steering overwrites the bat's retained dive direction, so later steps blend against the hole's last correction, which stays after the hole goes | `src/features/enemies/bat.rs:138-160` (the dive's direction set once, then only timed); the prototype's `hold` writing the blend back (`tools/proto_toys.py`) | Accepted | §2.4: `toys_release` after the movers writes each flyer's own direction back; step 4 adds `a_bat_diving_through_a_hole_dives_on_its_own_way_after` |
| 2.1 | [MEDIUM] The wave can still miss targets moving diagonally into its front: the revised test advances the distance along the aim but checks the distance across at the step's start, so a ball at (130, 65) moving at (−900, −900) px/s meets the front 14.3 ms later inside the sector yet fails the check | Recomputed: with the front from 100 to 120 px, at 0.083 s (half angle 19.2°, tan 0.348) the ball meets the front at (117.1, 52.1), within 117.1 × 0.348 + 15 = 55.8 px; at the start 65 > 130 × 0.348 + 15 = 60.2 | Accepted | §2.3: each target taken at its meeting time `t*`, both coordinates and the sector's width there; step 3's test adds the slanted ball |
| 2.2 | [MEDIUM] The minimum climb advance does not cover circular controllers: a circle-on-circle cast uses the summed radii, so a radius-15 controller beside a radius-15 ball needs more than 4.8 px | `crates/tungsten-kit/src/motion/platformer.rs:954-962` (`kinematic_body` takes any collider); `crates/tungsten-core/src/physics/query.rs:686-696` (circle against circle at `other + radius`); `crates/tungsten-kit/src/tests/platformer.rs:52-60` (the test hero is a box). At 4.8 px the contact normal's y is about −0.54 | Accepted | §2.1: the advance adds 0.29 of a circle controller's radius; step 1 adds `a_round_controller_climbs_a_ball_too` |


## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 The kit's shove and step height | 2026-10-25 | Pass | `motion::platformer::tests` 48 passed (16 new), the nine named in `--list` → exit 0; the kit 663 + 108 doc tests; `kit_recipes kit_tests` 169 passed, `the_recipes_hash_the_same_twice_and_match_the_pin ... ok`, `0x3334_9665_de0c_501a` → 1; `--doc` and `--bench kit` (78 `Success`) → exit 0. Shove 60 × 0.5 into a 12 crate: 448 px/s ± 5 % and the hero at its pace; a 40 blocks; a pinned crate held within 1 px (0.66 px into the wall's soft contact). Slow walk (50 px/s, step 32) over a radius-15 ball: feet to −29.74 (box) and −29.75 (circle), past it to x 367.4 and 372.3, the ball 0.256 px straight down (its settle; x 0). `api/tungsten-kit.txt` +4 (two `with_shove`, two `with_step_height`), `api/tungsten-kit-components.txt` +3 field lines, no `-`; `just api-check` → "Public API snapshots current"; ledger `M84's kit API` → 1; `it never pushes one` absent → exit 0; fmt and clippy → exit 0. Deviation (owner-approved 2026-10-25, against stop conditions 2 and 6): `crates/tungsten-kit/src/tests/describe.rs`'s numeric-field census 100 → 103 for the three data fields, a file outside the step's list | `crates/tungsten-kit/src/motion/{platformer,slide}.rs`, `crates/tungsten-kit/src/tests/{platformer,describe}.rs`, `api/tungsten-kit{,-components}.txt`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/known-issues.md` |
| 2 01's shoves and steps | 2026-10-25 | Pass | `player:: enemies:: saves:: level:: playthrough::` 129 passed, 0 failed; the four new tests in `--list`; `a_goblin_hops_a_crate_in_its_way`, `a_jump_under_small_balls_shoves_them_off_and_rises_clear`, `pit_full_saves_within_two_mib` and the nine `level::` tests among the passes. The wizard behind a crate: 448.0 px/s (a probe, removed); a small ball ahead rolls off over 100 px; iron holds within 1 px. A patrolling beetle rose 29.7 px over a bowling ball and passed both balls (centre at x 1179.4) within 300 frames, then stood on the ground; a crate and a keg it walks into move at most 1 px. `rg -l 'with_shove\|with_step_height' examples/01_platformer/src` → `enemies/mod.rs`, `player/mod.rs`; fmt and clippy → exit 0. Found in the run: `schema_8_saves_migrate_with_the_new_map_objects_ids_moved` and `schema_9_saves_migrate_to_the_new_save_ids` compare a reloaded old save with its recapture, which now holds the shoves and steps 01 sets at a load; `saves.rs`'s `without_new` sets them aside | `examples/01_platformer/src/features/enemies/{mod,systems,crawler}.rs`, `src/features/player/mod.rs`, `src/save.rs`, `tests/game/{player,enemies,saves}.rs`, `README.md` |
| 3 The force push as a wave | 2026-10-25 | Pass | `enemies:: spells:: data::` 87 passed, 0 failed, the four new tests among them; `a_blast_pushes_dynamic_bodies_away_by_distance_and_mass` passes with its body byte-identical to `HEAD`'s. Throws at 1.5 tiles (a probe, removed): crawler 157.5 px, goblin 135.5, beetle 41.7, bat 180, wisp 157.5, troll 22.0. The absence `rg` → exit 0; `just level-check` → "OK: 4248 owned outputs match", 15 tests OK; fmt and clippy → exit 0. The 13 `push_wave`/`push_dust` frames equal `proposals.py`'s pixel for pixel, and `force_push.wav` equals the reviewed `sounds/force_push_wave_new.wav` (`cmp`). Generation removed `particles/warp_burst.json` and shifted `level.tmj`'s tile IDs by 13, the new frames sorting into its one tileset. Changed from the prototype: each target is taken at its meeting with the front (§2.3), and a left-aimed dust puff keeps its foot on the hit point (the prototype set it 48 px off, since a negative scale already spans the quad leftward) | `examples/01_platformer/src/features/toys/{wave,mod,components,draw}.rs`, `src/features/spells/systems.rs`, `src/features/enemies/mod.rs`, `tools/{effects,art,generate,sfx}.py`, `tools/README.md`, `tools/outputs.json`, `assets/` (13 sprites, two clips, `push_grit`, `force_push.wav`, `manifest.json`, `level.tmj`; `warp_burst.json` gone), `tests/game/{enemies,spells}.rs`, `README.md` |
| 4 The black hole | 2026-10-25 | Pass | `spells:: enemies:: spawning:: presentation:: brick:: ball_pit:: props:: dummy:: saves::` 187 passed, 0 failed, the four new tests among them; `spawn_black_hole_system_creates_attractor_at_cursor_on_right_click` passes with `spawning.rs` unchanged, and `knockback_the_black_hole_balls_and_iron_barely_move_the_troll` with its body unchanged (no setup moved). Probes (removed): a bowling ball 180 px under a core rose 219.7 px within 60 frames, one 230 px under stirred 0.26 px and iron 120 px under 0.005; a bat held within 58.0 px and a wisp within 58.4 for the hole's life once within 60; the hold bent a dive by 0.18 rad while its mover kept the dive's own direction, and its flight was the dive's own the step after the collapse. `black_hole.ogg` deleted by hand (`test ! -e` → exit 0) and absent from `sfx.py` and `sounds.json` (exit 0); `black_hole_open.wav` equals the reviewed `sounds/black_hole_open_new.wav` (`cmp`). `just level-check` → "OK: 4250 owned outputs match", 15 tests OK; fmt and clippy → exit 0 | `examples/01_platformer/src/features/toys/{mod,systems,components,draw}.rs`, `tools/{generate,sfx}.py`, `tools/{sounds,outputs}.json`, `tools/README.md`, `assets/` (`particles/hole_open.json`, `sounds/black_hole_open.wav`, `manifest.json`; `sounds/black_hole.ogg` gone), `tests/game/{enemies,spells,presentation}.rs`, `README.md` |
| 5 The chill's build-up | 2026-10-25 | Pass | `ice:: combat:: data:: enemies:: saves:: presentation::` 171 passed, 0 failed, the five new `ice::` tests and the `data::` hold of `CHILL_NEEDS` among them; `a_chill_slows_and_frosts_more_as_its_bucket_fills` passes with its body byte-identical to `HEAD`'s. A beetle under 1.5 cold every 0.1 s read `[(Some(1), unchilled), (Some(2), chilled), (Some(3)), (Some(4)), (None, frozen)]`; its flakes were one emitter a stage, `ex10_chill_flakes_1` to `_4`, and the wizard's chill kept `ex10_chill_flakes`. A left-facing troll draws 20 px on, and its frost and cube sit on its point as facing right. The 36 frost sprites equal `proposals.frost_art()` pixel for pixel, and `chill_flakes_1`–`_4` (with `push_grit` and `hole_open`) equal `proposals.configs()`. `just level-check` → "OK: 4290 owned outputs match", 15 tests OK; fmt and clippy → exit 0; `saves.rs` needed no change. Changed from the prototype: the threshold read is one `threshold_at` beside `chill_grade`, and the overlays take a left-facing enemy's shift back only when it has its mind, since `clips`, which shifts its drawing, shows a clip only then. Per §2.5 the whiter `FROST_TINT` and the white ramp of `chill_flakes` reach the wizard's flat chill too | `examples/01_platformer/src/features/combat/{mod,systems,components,draw}.rs`, `tools/{effects,art,generate}.py`, `tools/README.md`, `tools/outputs.json`, `assets/` (36 `frost_*` sprites, `chill_flakes_{1..4}.json`, `chill_flakes.json`, `manifest.json`, `level.tmj`), `tests/game/{ice,data}.rs`, `README.md` |
| 6 The test pass | 2026-10-25 | Pass | `cargo test -p example-01-platformer --locked` → exit 0, 323 passed, 1 ignored (`frame_time`); `cargo test -p tungsten-kit --locked` → 663 + 108 doc tests; `cargo test -p tungsten --locked --lib -- kit_recipes kit_tests` → 169 passed, `the_recipes_hash_the_same_twice_and_match_the_pin ... ok`; `just level-check` → "OK: 4290 owned outputs match", 15 tests OK; `just smoke` → exit 0 (`example-01-platformer ... OK`, every row passed); `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0. No failure, so no step's files changed | — |
| 7 Records | 2026-10-25 | Pass | `D-213` written (still the next free ID), amending `D-158`, `D-194` and `D-209`: `rg -c '^## D-21[3] — '` → 1, the index row → 1, `Amended by \`D-21[3]\`` in the index → 3, `Amended by D-21[3]` in `DECISIONS.md` → 3. The feedback plan's `Landed (M84` → 1 and `K28` → 2 (K7 and K18 fixed, K20 and K21 homed in the backlog, K28 beside K27, still the next free K); the backlog's K7 row narrowed to a controller pushed by bodies, with K20, K21 and K28 rows; known issues cite `D-213` and note walkers pressing at crates and kegs. `just repo-check` → 0 errors (97 notes), release consistency and the index tests pass; `just ctx` → exit 0 | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/{example-01-feedback,backlog-1.x}.md`, `docs/known-issues.md`, this plan |
| 8 Release 0.105 | 2026-10-25 | Pass, to the hand-off | Workflow §5 closed: every step's row quoted; the gates by change in steps 1–6 and the release chain after the cut with `just level-check`; the kit hash held (steps 1 and 6) and no core, render, engine, template or shell file changed; `D-213` with its index row and three markers; the ledger row, rustdoc on both builders and `just api`; template n/a; `just ctx` passes; DESIGN.md's controller paragraph and status line; known issues, backlog rows K7, K20, K21 and K28, the register row cut; one `CHANGELOG.md` section; `status: done`, archived before the cut. `target/debug/incremental` (33 GB, workflow §6.5's disposable cache) cleared first, the disk at 6.5 GB free under template-check's 6 GiB floor once the cut rebuilt the workspace. The cut, the release checks and the preflight are quoted in the session's report | `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/{implementation-plan,README,example-01-feedback}.md`, this plan (archived) |

## Follow-ups

- K20, K21 and K28 as backlog rows (step 7); K7's other half (a controller pushed by bodies) stays in its row.
- The stale comment `src/features/enemies/troll.rs:21-22` (its stun "four times" the others', 3 against 1 in the data): the Phase 6 QA pass.
