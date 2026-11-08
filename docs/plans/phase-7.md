# Phase 7 plan from the registers (phase-7)

- **status:** in progress
- **goal:** Assign every register row that exists to the Phase 7 or Phase 8 step that builds the system it changes, set Phase 7's remaining order, recheck 7.19's provisional assumptions against the tree, and name 7.14's and 7.15's finite lists with a target per item (the [roadmap redo](roadmap-redo.md)'s step 7.7, §5.8, Q20). Then serve as Phase 7's map: each step takes its rows from §2 and closes them here.
- **non-goals:**
  - Each step's mechanisms: its own plan specifies them from the tree of its day (the redo's non-goal).
  - Code, captures and the analysis pass itself (7.5). The lists' numbers are taken at each pass's start on 7.3's rows (§2.5).
  - Edits to the registers' sources beyond [the ARPG design](arpg-design.md)'s step 6; the [paper designs](paper-designs.md)' step 6 is a follow-up.
  - Phase 8's order and Phase 9 (the redo's §3 holds them); budgets (9.1).
- **files to touch:**
  - This session: this file; `ROADMAP.md` (Now, Later's Phase 7 line); `docs/plans/arpg-design.md` (step 6, files to touch).
  - Each later step: its own plan file (`docs/plans/<topic>.md`), this file's §2 status cells and evidence row, `ROADMAP.md`, and what its plan lists.
  - The close: this file (deleted), `ROADMAP.md`, `docs/plans/roadmap-redo.md` (§3's Phase 8 rows take §2.3's IDs), `docs/plans/arpg-design.md` (its pointer to this file).
- **ordered steps:** (1) 7.2 agent tooling. (2) 7.3 benchmarks, part 1. (3) 7.4 H2. (4) 7.5 the analysis pass, its rows folded into §2. (5) 7.8 terrain and physics. (7) 7.9 saves and engine seams. (6) 7.17 the UI pass, after step 7. (8) 7.20 screens as data, after step 6. (9) 7.10 the kit pass. (10) 7.18 camera rigs. (11) 7.11 template updates. (18) 7.21 core hardening, after step 11. (12) 7.12 level format, CLI, editor. (13) 7.13 01 on the new systems. (14) 7.14 the CPU pass. (15) 7.15 the GPU pass. (16) 7.16 agent guidance. (17) Close. One track, a step at a time in this order (§2.8): Step 7 runs before Step 6 and Step 18 before Step 12, and each keeps its number, which plans cite.
- **done-when:** every row in §2.2 reads landed, closed, Later or 9.2 (§2.1), none open in a Phase 7 step, and every open row in §2.3 is carried into the redo's §3 by ID; every item on 7.14's and 7.15's lists reads done, dropped or stopped (§2.5); every step's evidence row written by §3's rule; the tests pass and example 01 runs on Phase 7's systems (7.13); the plan deleted with its citations, §2.3 carried into the redo, and `phase-7` out of Now.

Item `phase-7` in [ROADMAP.md](../../ROADMAP.md), from the redo's step 7.7 (§3, §5.8, §7.3, Q20). Written 2026-10-29 at `08111d5a`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md), and differs from the skill in one way: each step is a redo step of a session or more with its own plan, so a session takes one step, never "every remaining step".

## Context digest

At writing (`08111d5a`, 2026-10-29); the evidence log holds what has run since.

- 0.112 is out with `qa6`, the quick wins (7.1) and the ARPG design (7.19). 7.2–7.5 have not run: no agent tooling, the old benchmark suite, no H2, no seam register (A15). 7.6 ran on 2026-10-23; the owner triaged 7.19 on 2026-10-29 with every row and question at its default.
- 7.7 waits only for 7.19 (Q20), so it runs now, ahead of 7.2–7.5 in time but not in order. 7.5's register joins §2 at its triage by §2.4's rule, and 7.3's rows hold the lists' numbers (Q1).
- The registers: the paper designs' §4 (P7-1–P7-20, P8-1–P8-17), the ARPG design's §15 (PA-1–PA-22), and the feedback plan's kit and engine gaps ([`example-01-feedback.md`](1.0/example-01-feedback.md) §6, K1–K31), which the redo's §3 cites and 7.5 seeds from.
- The rule is §7.3's triage: a row goes to the step that builds the system it changes, so each system is built once (`D-218`). A row split across systems is split here (P7-2, P7-3).
- Phase 7 is one track since 2026-10-31: 7.17 and 7.20 run after 7.9 in this tree, not beside 7.8–7.10 in a worktree (§2.8).
- Decisions: `D-215` (the redo), `D-218` (the fifth genre; 7.7 waits for 7.19), `D-172` (admission, amended by §5.3), `D-140` (routes), `D-070` (CI is CPU only; GPU and time rows stay local).
- Later steps read this file: 7.10 its rows and its four plans' order, with PA-1–PA-11 but PA-3, PA-4 and PA-9, which moved to 8.7 (§2.3, §2.8); 7.14 and 7.15 their lists (§2.5); 7.13 and 8.6 their tasks, 9.1 its stress rows (§2.6).

## 1. Audit

Read at `08111d5a`: only what an assignment, an assumption or a list item needs.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The kit's modifier formula is unchanged: `((base + Σadd) × (1 + Σpct) × Πmul)`, the last override, then the clamp | `crates/tungsten-kit/src/foundation/stats.rs:96` |
| A2 | No status type in any crate; today's effects and ailment ladders stand | `rg -n 'pub struct (Status\|Statuses)\b' crates/` → none; `crates/tungsten-kit/src/damage/effects.rs:1355` (`Effects`), `damage/ailments.rs:321` (`AilmentThreshold`) |
| A3 | No item, inventory or equipment type in the engine or kit | `rg -l 'Inventory\|Equipment\|ItemInstance' crates/` → only a private struct in `crates/tungsten-render/tests/shader_coverage.rs:28` |
| A4 | Abilities and projectiles have no burst or pattern payload | `rg -n -i 'burst\|pattern' crates/tungsten-kit/src/intent/ability.rs crates/tungsten-kit/src/overlap/projectile.rs` → none |
| A5 | No saved-component registry; the template captures and restores by hand; 01's saves are schema 23 with names from 7.1 | `rg -l 'register_saved\|SaveRegistry\|SavedComponent' crates/` → none; `templates/basic/src/save.rs`; `examples/01_platformer/src/save.rs:332` (2,873 lines) |
| A6 | The asset registry has no `remove` (P7-16's `+`) | `rg -n 'pub fn remove' crates/tungsten-core/src/assets/` → none |
| A7 | Kit domains: `damage`, `foundation`, `intent`, `lifecycle`, `motion`, `overlap`, `present`, `react`, `transform`; `react` holds cues, counters, level and loot; no `chemistry` or `build` | `crates/tungsten-kit/src/` |
| A8 | Crit is a flag; `area` and `projectile_speed` are declared and read by nothing | `crates/tungsten-kit/src/damage/pipeline.rs:218`; `foundation/stats.rs:279`, `:287` (the only matches in `crates/`) |
| A9 | The kit's genre tags are `platformer`, `top_down`, `shmup`, `grid` | `crates/tungsten-kit/src/foundation/describe.rs:551-554` |
| A10 | 7.1 landed P7-9 (map save IDs from authored names, `D-222`) and P7-12 (gamepads, `D-220`, `D-221`) | commits `4f40f34e`, `5e516e60`; `d3724465`, `2fda1ff3`; `crates/tungsten/Cargo.toml:19` |
| A11 | K25 landed in part (an air jump's cost and refusal; the run-time count is in Later), K26 and K29 landed | `crates/tungsten-kit/src/motion/platformer.rs:432`, `:636`; `intent/ability.rs:305` (`with_cost_scale`); `react/cues.rs` (`CueListener`); ROADMAP.md Later |
| A12 | K12, K13 and K14 stand: `has` scans the type list; `animation_tick` resolves each state's clip by name twice a sprite; the query index rebuilds whole when not fresh | `crates/tungsten-core/src/ecs/archetype.rs:196`; `crates/tungsten-kit/src/present/animation.rs:125-136`; `crates/tungsten-core/src/physics/query.rs:305` |
| A13 | Lights cap at 16, culled by `cull_to_cap` | `crates/tungsten-core/src/lighting.rs:10`; `crates/tungsten-render/src/lighting.rs:129` |
| A14 | 01's full-pit row exists as an ignored release test | `examples/01_platformer/tests/game/frame_time.rs:83` |
| A15 | 7.2–7.5 have not run: no seam register in the tree, and Later lists them | `rg -l -i 'seam register'` → `docs/plans/roadmap-redo.md` only; ROADMAP.md Later |

## 2. Design

### 2.1 Rule and status cells

- **Rule:** a row goes to the step that builds the system it changes (§7.3's triage, §5.3). A row whose system no Phase 7 step builds goes to Phase 8 or Later. A row depending on a later step's system moves to the first step after it.
- **Status cell:** `open`; `landed <sha>`; `closed (<reason>)` for a row no step builds; `Later`, for a row that is one line in ROADMAP.md's Later; `9.2`, for a cost 9.2's lever list takes (§2.4). All but `open` are terminal. The commit that closes a step updates its rows here.
- **A move** rewrites the row's step cell, keeps it `open` and notes `moved from <step> (<reason>)`; a row moved into Phase 8 goes to §2.3. Nothing else changes a row's step.
- **Pattern** is the register's, shortened; the register holds the evidence.
- **7.5's rows:** [the seam register](seam-register.md) holds the S-rows' evidence, and its §4 01's sites for the other IDs.

### 2.2 Phase 7 rows

| ID | Pattern | Step | Status | Note |
| --- | --- | --- | --- | --- |
| P7-9 | Map objects by name | 7.1 | landed `5e516e60` | `D-222`, schema 23 |
| P7-12 | Reading a gamepad | 7.1 | landed `2fda1ff3` | `D-220`, `D-221` |
| K26 | An ability's cost scaled by a stat | e01tomes | landed (0.109) | `Ability::with_cost_scale` |
| K29 | A cue sound's position and falloff | e01sfx | landed (0.110) | `CueListener`; P7-13's rest is 7.9's |
| K25 | Air jumps priced, refused, counted at run time | Later | Later | Cost and refusal landed in `e01tomes`; the run-time and unlimited counts wait in Later |
| S-103 | The hurt flash's vignette generated as a full-view sprite | Later | Later | no step builds the post passes |
| S-182 | A test's press held over frames until a fixed step has read it | Later | Later | no step builds the harness's fixed-step stepping |
| P7-1 | Tiles changed at runtime | 7.8 | landed `c31c8bfc` | `D-228`, the runtime terrain (§5.1) |
| P7-2 | Pushes and impacts: physics part | 7.10 | landed `2acbebad` | 7.10m (§2.8), `D-239`: impulses, radial impulses and a kinematic body's push record by begins; moved from 7.8 (physics-on-terrain's step 7 stopped: its push record counted the soft solver's push-out); contact speeds, begins and ends landed `992f75d2` (§5.6, `D-229`) |
| P7-3 | Something in range: sensor exit | 7.8 | landed `e6c9bbe0` | `SensorExited`, `D-229`; `Interactable` is 8.1's (§2.3) |
| K7 | A controller pushed by bodies | 7.10 | landed `bd6145e7` | 7.10m, `D-239`: a controller whose body has a mass joins its push, `pushed()`; M84's shove on impulses to Later; moved from 7.8 (on P7-2's impulses, which did not land there) |
| K14 | The query index rebuilt from every body | 7.14 | open | §5.6's cheaper index; its remaining cost is C1 (§2.5); moved from 7.8 (a resting grid kept across refreshes moved `terrain`'s pin as the cave's bodies shift between asleep and awake: beside C1) |
| K23 | Kinematic pairs never touch | 7.10 | landed `8db34c19` | 7.10m, `D-239`: a moving platform pushes a controller out, `crushed()` when cut short; other kinematic pairs to Later; moved from 7.8 (with the controller's rules, behind step 12) |
| K30 | A locked controller stops dead at a body | 7.10 | landed `bd6145e7` | 7.10m, `D-239`: the plough under locked input; named in §5.6; moved from 7.8 (on P7-2's impulses) |
| S-60 | Rays cast again from inside each body they meet (K31) | 7.8 | landed `5656e347` | `cast_ray_where`, `cast_shape_where`; 8.2's sight rays use them |
| S-16 | A fire hazard's touch swept by hand | 7.8 | landed `e6c9bbe0` | `RigidBody::swept`, a start overlap counting |
| S-26 | Friction and rolling by hand | 7.8 | landed `52202d87` | Friction `aae37231`, rolling circles `52202d87` |
| S-65 | A body's world box worked out by hand | 7.8 | landed `79c0f268` | `Collider::world_box` |
| S-77 | An entity teleported by writing its positions by hand | 7.8 | landed `79c0f268` | `physics::teleport` |
| S-99 | A controller's filter, shove and step set in code at each spawn and load | 7.13 | open | + 7.10m: a layer-mask filter in the controller's data and saved form; moved from 7.10 (its kit part landed `09ef56a5`, `D-239`, with the controller on slopes and the drop through a one-way edge after the slope casts' fix, `7dbe7ee9`; 01's walkers onto it) |
| S-112 | Iron's extra fall added to its velocity each step by hand | 7.8 | landed `79c0f268` | `RigidBody::gravity_scale` |
| S-132 | A lift's trip held until every rider stands wholly on it | 7.13 | open | 7.10m; §5.6's riding: the carry cast against the world; moved from 7.10 (its kit part landed `bd6145e7`, `D-239`; 01's elevator onto it) |
| S-172 | A point's distance to a shape's surface (`surface_distance`) | 7.8 | landed `79c0f268` | `Shape::distance` |
| S-175 | Physics layer bits restated in Rust and in the generator | 7.8 | landed `ddc3e3e4` | `physics_layers` in the manifests, names in `rigid_body`; a tile layer's names wait for 7.12 (Later) |
| P7-8 | Saves of many areas | 7.9 | landed `1ec87ae6` | `D-230`: save identities, the registry, captures per area, kept areas, the kit's saves, the template on one-call saves (§5.5, G4); 01's captures and records move onto it at 7.13 |
| P7-10 | A tile layer's depth among sprites | 7.9 | landed `b686d9fd` | a tile layer's `z` among sprites (`D-231`); 01's layers move at 7.13 |
| P7-13 | Audio: looping cue voices, pan, bus ducking | 7.10 | landed `5bddc01e` | `D-242`: the kit's loops on entities, the listener's pan and `SoundOnEvent` at a point (`e3d0131f`), a status look's sound loop (`5bddc01e`); the engine's part landed `01826112`; music per state stays S-63 at 7.11 |
| P7-14 | Text outline (K5) | 7.17 | landed `f7f3224f` | 7.9 lands first (§2.8), so 7.17's named text styles take the outline as a field; moved from 7.9 (`TextOverrides`' and `TextSection`'s new fields needed literals in files boot-and-seams' step 12 did not list) |
| P7-15 | A file per frame | 7.9 | landed `a6c3d80a` | Sprite sheets, with §5.4's boot loading; sheets in `ce71db3c`, boot loading in `a6c3d80a` (`D-231`); 01's per-frame files move at 7.13 |
| K2 | A particle config's draw depth | 7.9 | landed `e48bc6c1` | `z_order` on a particle config (`D-231`); 01's sites at 7.13 |
| K4 | An animation's playback rate | 7.9 | landed `48000b85` | `AnimationState` speed (`D-231`); 01's sites at 7.13 |
| K9 | Lit sprites without the hit-flash material | 7.9 | landed `0bdb8e33` | the lit shader takes the flash's lanes (`D-231`); 01's lit courier and foes at 7.13 |
| S-38 | Emitters changed at run time by hand (K21) | 7.9 | landed `e48bc6c1` | `rate_scale`, `direction`, `paused` (`D-231`); 01's sites at 7.13 |
| S-2 | Migrations setting saved bodies out of solids a map change added | 7.9 | landed `8278ab06` | `physics::free_spot`, the restore's last pass |
| S-3 | Retunes carried into saves by hand | 7.9 | landed `b3b3b985` | a named entity's changes as patches over its fresh spawn |
| S-25 | Looks and a muzzle flash aged by hand each frame | 7.9 | landed `0bdb8e33` | + a light-intensity tween channel; the looks on `Lifetime`; `light_intensity` (`D-231`); 01's looks at 7.13 |
| S-27 | A spray's stream as hand-made particles | 7.9 | landed `e48bc6c1` | + particle clips, a stretch along the velocity and a stop at a surface; clips, `stretch`, `stop` (`D-231`); 01's spray at 7.13 |
| S-29 | A hole's particles and streaks steered into a spiral by hand | 7.9 | landed `e48bc6c1` | + a particle attractor: a pull and a swirl toward a point; `attractor` (`D-231`); 01's hole at 7.13 |
| S-40 | Parallax strips as Rust tables, repeated and stretched by hand | 7.9 | landed `b686d9fd` | + a repeating parallax layer under the tiles; `Backdrop` (`D-231`); 01's strips at 7.13 |
| S-43 | Each source schema's `migrate` arm restating every later step's call | 7.9 | landed `e271a5ee` | `SaveData::migrate_step`; 01's arms at `93540b4a` |
| S-46 | Left-facing art as whole-frame mirrored twins | 7.9 | landed `0bdb8e33` | + a sprite flip about a pivot that mirrors lit normals; `SpriteFlip`, `facing`'s `flip`, a clip set without `left` (`D-231`); 01's twins at 7.13 |
| S-58 | Art variants by hand | 7.12 | open | + a prefab's sprite variants picked per instance, kept through a save; moved from 7.9 (`SceneSprite`'s `variants` needed a literal in a file boot-and-seams' step 13 did not list; 7.12 builds prefab data) |
| S-59 | A same-step order kept by hand across a load | 7.9 | landed `93197b6c` | the kit's orders by save ID while saving; the physics step's order and a saved solver cache are one line in `ROADMAP.md`'s Later |
| S-63 | Music per state run by hand | 7.11 | landed `67d483e8` | + music per state on the stack, with P7-13's ducking; moved from 7.9 (music per state and the template's tracks, with step 6's loops, needed files its list did not hold); `GameState::music` (`57575da2`), the shell's and the sample's songs on it, `D-250`; 01's site at 7.13 |
| S-70 | Quad helpers for 01's own sprite extracts, copying the default extract | 7.9 | landed `6a9e7881` | + public render helpers: an instance from a sprite, joined batches, the view box; `sprite_instance`, `Batches`, `view_box` (`D-231`); 01's extracts at 7.13 |
| S-71 | A material restating the sprite contract to change its fragment | 7.9 | landed `6a9e7881` | + fragment-only materials; fragment-only materials (`D-231`); 01's materials at 7.13 |
| S-76 | The ice beam's core laid as tiles along the cone each frame | 7.9 | landed `6a9e7881` | + a beam sprite tiled along a segment; `beam` (`D-231`); 01's ice beam at 7.13 |
| S-87 | One-shot clips spawned by hand, each with a lifetime restating its clip's length | 7.9 | landed `48000b85` | + a one-shot clip that despawns as it ends; `despawn` at a one-shot's end (`D-231`); 01's clips at 7.13 |
| S-89 | Newborn particles patched after spawn | 7.9 | landed `e48bc6c1` | + a particle's start colour by its direction or a range; `start_color` (`D-231`); 01's newborns at 7.13 |
| S-95 | Flat-colour rectangles and thick lines faked as sprite quads | 7.9 | landed `6a9e7881` | + a flat-colour sprite instance and a segment helper; `engine_white`, `SpriteInstance::flat` and `segment` (`D-231`); 01's quads at 7.13 |
| S-97 | The far walls baked as tinted copies of the play layer's rock | 7.9 | landed `b686d9fd` | + a tile layer's tint and an unlit flag; `Unlit` in `0bdb8e33`, a layer's tint and `unlit` in `b686d9fd` (`D-231`); 01's far walls at 7.13 |
| S-101 | Save envelopes faked as text in the tests | 7.9 | landed `e271a5ee` | `SaveSlots::save_stamped`; the template's tests on it at `ef935f73`, 01's fixtures at 7.13 |
| S-113 | A sound played on a walk's footfall frames by a watch of 01's | 7.9 | landed `48000b85` | + frame cues on a clip; a frame's `sound` (`D-231`); 01's footfalls at 7.13 |
| S-117 | Soft glows drawn by a material of 01's own | 7.9 | landed `6a9e7881` | + a stock glow material; `engine_glow` (`D-231`); 01's glows at 7.13 |
| S-125 | The map's props set down at its markers by the run's code | 7.9 | landed `ce42a2ae` | `World::spawn_area` names a map's named objects for saves; 01's props as classed objects at 7.13 |
| S-137 | Sprite anchors restated as constants | 7.9 | landed `ce71db3c` | + a sprite anchor in the manifest (Later's props' anchors as data); a sprite's `anchor` (`D-231`); 01's constants at 7.13 |
| S-138 | A state's particles cleared by hand when it leaves | 7.9 | landed `e48bc6c1` | + particles owned by their emitter's state; a state's exit takes its emitters' particles (`D-231`); 01's clears at 7.13 |
| S-143 | The sprite base replaced by a copy of the engine's default | 7.9 | landed `b686d9fd` | + extract contributions ordered under the default base, with P7-10; tile slots and raised layers in the default base (`D-231`); 01's copied base at 7.13 |
| S-148 | Sprites mocked by hand in tests | 7.9 | landed `ce71db3c` | + the headless load registers sprite metadata; `Harness::register_sprites` (`D-231`); 01's mocks at 7.13, the template's at 7.11 |
| S-173 | A manifest sound played at its manifest volume by hand | 7.9 | landed `01826112` | + a play at the manifest's volume and loop; `audio::play` by ID (`D-231`); the template's shell took it here, 01's copy with it |
| S-179 | The manifest root and debug hot reload wired by hand in `main` | 7.9 | landed `aa7c6689` | + the manifest root and hot reload from `Config`; `assets.manifest` and `assets.hot_reload` (`D-231`); the template took it here, 01 at 7.13 |
| PA-1 | Lines scoped by tags | 7.10 | landed `502b1be1` | 7.10a (§2.8), `D-238` |
| PA-2 | Skill sheets and the snapshot | 7.10 | landed `2809e88d` | 7.10a, `D-238` |
| PA-5 | Crit | 7.10 | landed `a8e54afa` | 7.10a; every genre (PA-Q3); 01's on it at `a869a70c`, `D-238` |
| PA-6 | Conversion and extra | 7.10 | landed `5b34dd38` | 7.10a, §9.1 at 32.07, `D-238` |
| PA-7 | Defender's side | 7.10 | landed `921cae44` | 7.10a, `D-238` |
| P7-7 | A hit's side | 7.10 | landed `921cae44` | 7.10a, with PA-2's hit context, `D-238`; 01's armour onto it with S-47 at 7.13 |
| PA-10 | World sources | 7.10 | landed `502b1be1` | 7.10a; 01's difficulty on it (PA-Q5) at `a869a70c`, `D-238` |
| PA-11 | `explain` records | 7.10 | landed `e3b465d7` | 7.10a; shared with chemistry's `explain`, `D-238` |
| P7-4 | Statuses on tiles and things | 7.13 | open | moved from 7.10 (its kit part landed: the statuses, `25e56d79`, `D-240`; interactions `4e90d73e`, spread, impacts and light `7682cdf4`, tiles `2bbf36f4` and `become` `d4adf9fd`, `D-241`): 01's elements onto interactions, with the ladders, buckets and `ailment_threshold` removed; §5.2, Q17's merge, Q11's names |
| P7-5 | Light as a stimulus | 7.10 | landed `95eeb8e1` | the core light-at-a-point query (`D-240`); chemistry's light stimulus is P7-4's, with 7.10b's chemistry |
| PA-8 | Statuses from their user | 7.10 | landed `4e90d73e` | 7.10b: the status effect and instance lines (`25e56d79`, `D-240`) and the damage-over-time rate from the hit (`4e90d73e`, `D-241`) |
| S-122 | Hits told apart in code (K1) | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: a rule's `periodic` (`59fdaff0`); 01's hurt and blood onto it) |
| S-36 | Saved countdowns hand-ticked on 01's own components (K11) | 7.13 | open | + moved from 7.10b (has: the kit's `Cooldown` saves and sends `CooldownElapsed`; `Lifetime`'s saved form and expiry stay a kit gap no 01 code needs) |
| K27 | `Damaged` hides a shield's absorption | 7.10 | landed `f53aa2ba` | `D-242`: `Damaged::absorbed`, a rule's `absorbed` and the flash's `shield` colour |
| K28 | Every non-periodic hit flashes over a status tint | 7.10 | landed `9a63b31f` | `D-242`: the flash's `min_interval` (`f53aa2ba`) and a status look's tint under it (`9a63b31f`) |
| P7-2 | Pushes and impacts: kit part | 7.13 | open | impact damage and bodies as walls by layer, on the physics part (`2acbebad`); its blast is K10, in 7.10b; moved from 7.10 (7.10m's step 8 stopped: its system changes the kit's phase snapshot in `crates/tungsten/src/tests/kit.rs`, outside its list; 01's impacts onto it, the projectile's layer names with it) |
| S-4 | Radial blasts hand-rolled (K10) | 7.13 | open | moved from 7.10 (the kit's `blast` output landed, `8f202a8c`, `D-241`): 01's blasts onto it, replacing its detonation (`elements/cascade.rs:634`); 7.10c's S-73 fires it |
| P7-6 | Ability fixes | 7.10 | landed `c56d2ebb` | 7.10c, with K8, `D-244`: a held trigger, a movement direction and the refill countdown (`Abilities::refill_left`); 01's reload onto them at 7.13 (the seam register's §4) |
| K8 | `charges { all: true }` refills from the first charge | 7.10 | landed `4d51a071` | qa6, in 0.112; 01's own reload waits on P7-6's held trigger and refill countdown (the seam register's §4) |
| P7-19 | One-shot clips | 7.10 | landed `2357a1f4` | `D-246`: `CharacterAnimator::play` and the `clip` cue action; 01's land, air-jump and toss clips onto it at 7.13 |
| P7-20 | A hitbox by facing (K3) | 7.10 | landed `ec9dbcb5` | `D-246`: `hitbox`'s `facing`, `mirror` or `turn`; 01's troll and goblin swings onto it at 7.13 |
| K3 | A hitbox that follows its owner's facing | 7.10 | landed `ec9dbcb5` | in P7-20 |
| K19 | The magnet and a falling bodiless pickup | 7.13 | open | moved from 7.10 (7.10m's step 7 stopped: `a_magnet_leaves_a_body_or_a_child_with_one_warning` in `crates/tungsten/src/tests/kit.rs`, outside its list, pins a moving pickup held; its work in `perf-runs/20261103-pushes-and-riders/step7-pickups.patch`; 01's orbs onto it) |
| K20 | A top-down mover's outside velocity; additive knockback | 7.10 | landed `065e1958` | 7.10m, `D-239`: `TopDownMover::push` and a stacking knockback |
| K22 | A hotkey for `KitDebug`; hit shapes without the trace | 7.10 | landed `ec9dbcb5` | `D-246`: `KitDebug::set_key` and its `shapes` and `trace` switches; 01's F1 mirror onto it at 7.13 |
| P7-18 | The kit's genre claims | 7.10 | landed `a17375bd` | 7.10c, `D-244`: `platformer`, `top_down`, `rpg`, `shooter`, `arpg`; `grid` and `shmup` moved whole to `rpg` and `shooter` (G1–G3), `arpg` with its first recipes; each component claims exactly the genres whose fixtures name it |
| S-1 | Status looks drawn by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: status looks, a tint, an overlay and a glow (`9a63b31f`), particles, a light and a sound loop (`5bddc01e`); 01's looks onto them) |
| S-5 | Chain lightning decided and queued in 01's code | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: the `conduct` payload over its receivers' tags (`82b79d79`); 01's lightning onto it) |
| S-10 | Presentation shown by an enemy's AI state, drawn in code | 7.13 | open | moved from 7.10 (its kit part landed, `D-247`: `looks` shown while an entity's tags pass a filter, growing over their time, mirrored as it is drawn or by its facing's side, and an overlay's scale by its share (`3352bda1`); 01's marks, streak, ring and embers onto it) |
| S-12 | Damage numbers shaped in 01's own pass over `DamagedPart` | 7.13 | open | + 7.10a: damage numbers merged over a window, ticks smaller, crits styled with PA-5; moved from 7.10 (its kit part landed `04506c39`, `D-238`; 01's numbers onto it with S-47) |
| S-15 | Cones by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: the `cone` payload, at once or as a widening front (`ee9b32ec`); 01's spray and wave onto it) |
| S-17 | Summons, toys and props placed at the aim point by 01's own code | 7.13 | open | moved from 7.10 (its kit part landed, `D-245`: the `place` payload, set down by its centre at the aim's point and refused before the spend with no point, no room or at its cap (`a1606731`); 01's placements onto it) |
| S-18 | Live counts capped by hand per kind | 7.13 | open | moved from 7.10 (its kit part landed, `D-245`: a `cap` on a prefab group's live instances, the oldest culled with a `culled` cue or the spawn refused, map-placed ones aside (`2b4b3eaf`); 01's hand caps onto it) |
| S-19 | A rider-started lift written in Rust | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `path_follower`'s `wait` and `rider`, a lift that waits for its rider and comes back, with `arrived` and `departed` (`148d2688`); 01's lift onto it) |
| S-20 | Each kind's clip picked from its AI state in a match | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `character_animator`'s `rules`, clips picked by tags and motion over a state tag the game sets (`2357a1f4`); 01's clip matches onto it) |
| S-31 | A kill row and the level's box by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `WorldBounds` and `despawn_outside`'s `world` margin with `kill` (`332c909c`); 01's kill row and box onto it) |
| S-37 | A body carried over a carrier's head in 01's code | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `carry`, `Hold` and `release`, a body held as a sensor at its carrier's point plus an offset and let go at a death or its drop filter (`f7ae8384`); 01's troll carry onto it) |
| S-39 | Code-sent cue events registered one `CuesOnEvent` per name | 7.13 | open | moved from 7.10 (its kit part landed, `D-243`: `CuesOnEvent::with_names` and `CueEvent::named` (`92a18364`); 01's 91 registrations onto one) |
| S-41 | Impact cues by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: a rule's `amount`, `min_interval` and `cap` and `CueEvent::of` (`59fdaff0`); 01's impact cues onto them) |
| S-48 | The controller's jumps and landing answered in code | 7.13 | open | moved from 7.10 (its kit part landed, `D-243`: the controller's `jumped`, `air_jumped`, `jump_refused` and `landed` cue events at its feet and `PlatformerController::landed` (`92a18364`), after 01's own `air_jumped` became `double_jumped` (`ee6b5224`); 01's jump, bound, refusal and landing code onto them, `double_jumped` onto `air_jumped`) |
| S-51 | A lit fuse's emitters kept by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: named sprite points (`42118b13`) and a look's particles at one (`5bddc01e`); 01's fuse emitters onto them, its generator writing the points) |
| S-52 | A health bar over an enemy drawn as flat quads | 7.13 | open | moved from 7.10 (its kit part landed, `D-247`: `health_bar` over its holder at its health's share, hidden while it is dead (`3352bda1`); 01's troll bar onto it) |
| S-53 | Lobs aimed in 01's code | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: a spawn's `lob` onto `Aim::point` by the body that carries it, and `lob_velocity` (`89a35f6b`); 01's lobs onto them) |
| S-56 | Reactions to an effect applied or run out written in code | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: a rule's `effect` and `ended` (`59fdaff0`); 01's reactions onto them) |
| S-66 | A cast's pose picked by the aim's angle by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `Clips`' `aim` set, the pose nearest the aim's elevation (`2357a1f4`); 01's cast pose onto it) |
| S-73 | A projectile's end read from its events in Rust to fire its blast | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: a projectile's `end` blast through chemistry's run (`afe4365d`); 01's flask onto it, its look in the `expired` cue) |
| S-78 | Missiles and tracers turned to their flight by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `facing`'s `mirror` past the vertical (`ec9dbcb5`); 01's missiles and tracers onto it) |
| S-79 | A look at a point played statement by statement from code | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: the `world_cues` kind and `WorldCue` (`4e999198`); 01's looks at a point onto them) |
| S-84 | A periodic hazard's phase cycle by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `StateMachinePlugin::after` and `StateMachine::starting_at` (`148d2688`); 01's geyser phases onto it) |
| S-86 | Fallen enemies brought back at their places by a hand-ticked timer each | 7.11 | landed `52abefe0` | moved from 7.10 (its kit part landed, `D-246`: `spawner`'s `alive` and `first`, a spawner that spawns again an interval after its spawn dies (`332c909c`); the template's bring-back onto it); the sample's enemies from spawners, `D-250` |
| S-88 | Spawned prefabs moved by hand onto their source's body centre | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: centred spawns and cue actions and `centre_of` (`89a35f6b`); 01's spawns onto them) |
| S-92 | A flyer's arrive at a point on a top-down mover among walls | 7.13 | open | + 7.10m: `steering` driving a `TopDownMover`, with K20; moved from 7.10 (its kit part landed `065e1958`, `D-239`; 01's flyers onto it) |
| S-96 | A look held after its last trigger by a game-time stamp | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: a rule's `fresh` (`59fdaff0`); 01's look stamps onto it) |
| S-105 | A spell's icon and cast gesture as per-spell arms | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: an ability's `icon` and `clip` in its data (`2357a1f4`); 01's spell icons and gestures onto it) |
| S-110 | HUD flashes and a fade ticked by hand; a cooldown's end found by comparison | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: `ability_ready` as an entry's cooldown ends (`a0e60bd0`); 01's flashes and fade onto it and `UiTree::animate` tracks) |
| S-121 | A shake's share worked out by hand from its distance | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: a shake's falloff (`e3d0131f`); 01's blast shares onto it) |
| S-130 | A spell's cast, start and end sounds played from code | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: a rule's `ability` filter on `ability_used`, and `ability_held` and `ability_released` (`a0e60bd0`); 01's spell sounds onto them) |
| S-134 | Each enemy kind's knockback resist restated in its info table | 7.13 | open | + 7.10m: `Knockback::resist()`, with K20's knockback; moved from 7.10 (its kit part landed `065e1958`, `D-239`; 01's table onto it) |
| S-135 | A refused use's reason mapped from `Used` and played from code | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: `ability_refused` with its `reason` (`a0e60bd0`); 01's refusal sounds onto it) |
| S-142 | A dead entity disarmed by hand | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `health`'s `disarm`, the touch, the hitbox and the controller switched off at a kill (`8aa77f63`); 01's disarmed dead onto it) |
| S-150 | A heading of 01's own on each enemy | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `Facing::face` and `face_toward` (`ec9dbcb5`); 01's enemy headings onto it) |
| S-154 | The cue listener moved to the player each step by 01's system | 7.13 | open | moved from 7.10 (its kit part landed, `D-242`: `CueListener::following` (`e3d0131f`); 01's `audio_listen` removed) |
| S-156 | A contact damage switched off by taking the component off | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `ContactDamage`'s and `Hitbox`'s `on` (`8aa77f63`); 01's geyser touch onto it) |
| S-159 | A timed pickup blinks over its last seconds at a steady 5 Hz | 7.13 | open | moved from 7.10 (its kit part landed, `D-246`: `lifetime`'s `blink` (`8aa77f63`); 01's timed drops onto it) |
| S-165 | The damage numbers option copied each frame into a tag | 7.11 | landed `52abefe0` | moved from 7.10 (its kit part landed, `D-242`: a rule's `flags` (`59fdaff0`); the template's option sets the flag once); the sample's `labels` rule on the `show.damage_numbers` flag, `D-250` |
| S-186 | A spawned missile's fall put back after the spawn payload drops it | 7.13 | open | moved from 7.10 (its kit part landed, `D-244`: a spawn keeps its prefab's acceleration (`89a35f6b`); 01's `missile_adopt` removed) |
| S-9 | The shell's UI builders hand-built over the tree | 7.17 | landed `f1998611` | the tree-only builders |
| S-28 | A confirm dialog built by the shell | 7.13 | open | the confirm; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-33 | A modal screen's plumbing by hand | 7.13 | open | the modal screen helper; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-42 | Fit rules written over the tree in the tests | 7.17 | landed `7b3000c6` | `assert_fits` |
| S-45 | Tree queries hand-written in tests beside the harness's `find` | 7.17 | landed `7b3000c6` | `UiHarness`'s queries |
| S-62 | Toasts built and ticked by the shell | 7.13 | open | the toast; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-64 | UI art sized in whole screen pixels per art pixel by hand | 7.17 | landed `067b1bf6` | + a pixel-art UI mode; 01's site at 7.13 |
| S-67 | The area banner's fades stepped by hand on the game clock | 7.17 | landed `067b1bf6` | + UI tracks on the game clock; 01's site at 7.13 |
| S-81 | A change of run behind a hand-made UI curtain | 7.13 | open | + a reset transition for the whole stack; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-82 | Styles changed by read-modify-write | 7.17 | landed `f1998611` | styles edited in place |
| S-91 | The shell's spacing, widths and text sizes as 22 Rust constants | 7.17 | landed `f1998611` | named text styles and spacing in the theme file |
| S-93 | Display labels sized by hand under the text scale | 7.17 | landed `f1998611` | shrink-to-fit text |
| S-94 | A slot's key label picked from the bindings by hand each frame | 7.17 | landed `7defa02d` | + a short binding label and a compact prompt placeholder; 01's site at 7.13 |
| S-102 | The gameplay cursor drawn as a sprite by 01 | 7.17 | landed `7f11aef8` | + a cursor sprite from config; 01's site at 7.13 |
| S-104 | A cooldown's radial sweep built as mesh triangles by hand | 7.17 | landed `4efcf875` | + a radial fill; 01's site at 7.13 |
| S-108 | WCAG contrast computed by hand in tests | 7.17 | landed `7b3000c6` | + a contrast ratio in the theme, or in `check` |
| S-109 | Menu sounds played from the UI's events by hand | 7.13 | open | + menu sounds in the theme and a back role; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-114 | 01's theme made by substituting the template's literal colours | 7.17 | landed `f1998611` | + kind styles naming colour tokens |
| S-116 | An action read through the `ActionMap` on the routed input by hand | 7.17 | landed `7defa02d` | + world-level `pressed` on the routed input; 01's site at 7.13 |
| S-123 | The UI's reference height and default scale set in Rust | 7.13 | open | + the UI's reference height and scale in `tungsten.json`; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-127 | A health meter hand-built from a track and a fill panel | 7.17 | landed `f1998611` | the meter |
| S-133 | A scale slider applied at its release by hand | 7.13 | open | + a slider's value committed on release; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-139 | The shell's theme applied by hand from the registry at the title | 7.13 | open | + the manifest's starting theme; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-149 | The template's shell assets copied into 01 by its generator each run | 7.11 | landed `1c544efc` | + the shell's assets in the engine; the hit-flash material with the kit; moved from 7.17 (they reach a game by `tungsten new`'s copy and 01's generator, the template's packaging); the hit flash's shader and material in the engine, `D-250` |
| S-152 | A label kept on one line by turning its wrap off or widening its box | 7.17 | landed `fce27866` | shrink-to-fit text, or the wrap fix (known issue 32) |
| S-153 | A UI panel placed over a world point by hand | 7.17 | landed `067b1bf6` | + a UI root anchored to a world point; 01's site at 7.13 |
| S-155 | A pressed widget identified by its position in a list | 7.17 | landed `f1998611` | a typed key per widget |
| S-157 | The game clock paused and resumed by hand under a covering screen | 7.13 | open | + a state that holds the game clock while covered; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped, as S-28's did; the template's site landed, `52abefe0`, 01's left, `D-250`) |
| S-163 | A run's resources removed one by one as gameplay exits | 7.11 | landed `52abefe0` | + state-owned resources; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); the sample's run resources owned by its state, `D-250`; 01's site at 7.13 |
| S-171 | A label's text read back by joining its spans by hand | 7.17 | landed `f1998611` | + `UiTree::plain_text` |
| S-183 | Panels used as layout groups, painted clear by hand | 7.17 | landed `f1998611` | the layout-only group |
| S-187 | A rebind capture's 5 s timeout ticked by hand on the real clock | 7.13 | open | + a timeout on `capture_next_input`; moved from 7.17 (ui-pass's step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin; the engine part landed, `D-235`); moved from 7.11 (template-updates' Step 6 stopped: on `tungsten::screens` six of 01's tests failed outside its list, `D-250`) |
| S-72 | The settings screen's rows as a Rust table | 7.20 | landed a49a7a48 | the settings screen as a file |
| S-75 | The camera's view as a zoom recomputed each frame for a set world height | 7.18 | landed `51d3e9a0` | 01's and the template's cameras as rigs, `D-249` |
| S-146 | The Screen shake option applied by scaling the shake offset by hand | 7.18 | landed `793963fd` | + a shake scale on the camera, `D-249` |
| S-44 | Audio commands gathered and filtered by hand in test helpers | 7.11 | landed `52abefe0` | the harness's audio queries; the log and its queries (`c5ed7c39`), the template's tests on them, `D-250`; 01's site at 7.13 |
| S-49 | The template's synthesis primitives copied into 01's sound script | 7.11 | landed `8c753dd2` | the template's synthesis primitives as a module 01 imports; `tools/synth.py`, `D-250` |
| S-50 | The Bloom option kept by taking the game's Bloom passes out and back | 7.13 | open | a post stack the shell owns and a Bloom switch in `Settings`; moved from 7.11 (its engine part landed, `2320b8f0`; the shell's part stopped with template-updates' Step 7, which runs on Step 6's shell, `D-250`) |
| S-57 | A `place_player` action and a `player` probe registered by each game | 7.11 | landed `52abefe0` | core script steps that place a tagged entity and probe it; the kit's `place` and `tagged` (`c5ed7c39`), the template's `place_player` onto `place`, `D-250`; 01's site at 7.13 |
| S-68 | The rebindable actions and their labels restated as a Rust table | 7.11 | landed `a097f11b` | a label per action in the action map's file; `input.json`'s `labels` (`67dd4a00`), the seam's six names, `D-250` |
| S-80 | The `start_run` hook registered by the game, though the shell owns it | 7.11 | landed `a097f11b` | the shell registers `start_run`; 01's own went, `D-250` |
| S-118 | Gameplay options kept in a second `SaveSlots` store | 7.13 | open | game options kept in `Settings`; moved from 7.11 (its engine part landed, `2320b8f0`; the shell's part stopped with template-updates' Step 7, which runs on Step 6's shell, `D-250`) |
| S-119 | A key or an action tapped for one frame by a test helper | 7.11 | landed `52abefe0` | `tap_action` and `tap_key`; `tap_action` and `tap_key` (`c5ed7c39`), the template's tests on them, `D-250`; 01's site at 7.13 |
| S-120 | The engine's F11 and F9 toggles mirrored into `Settings` by the shell | 7.13 | open | display keys that write `Settings`; moved from 7.11 (its engine part landed, `2320b8f0`; the shell's part stopped with template-updates' Step 7, which runs on Step 6's shell, `D-250`) |
| S-124 | An action's binding columns picked out of its flat list by hand | 7.11 | landed `a097f11b` | an action's binding by device class; `binding_at` and `set_binding_at` (`67dd4a00`), `D-250` |
| S-145 | Mute on focus loss by applying a zero-volume copy of `Settings` | 7.13 | open | a mute-when-unfocused switch; moved from 7.11 (its engine part landed, `2320b8f0`; the shell's part stopped with template-updates' Step 7, which runs on Step 6's shell, `D-250`) |
| S-147 | The display as set, else as it runs, merged by hand | 7.13 | open | the display `Settings` ask for, exposed; moved from 7.11 (its engine part landed, `2320b8f0`; the shell's part stopped with template-updates' Step 7, which runs on Step 6's shell, `D-250`) |
| S-167 | A lead line's notes parsed by the same loop twice | 7.11 | landed `8c753dd2` | a `melody()` helper, `D-250` |
| S-168 | Reset reloads `input.json` to learn the shipped bindings | 7.11 | landed `a097f11b` | the action map's shipped bindings and reset; `shipped` and `reset` (`67dd4a00`), `D-250` |
| S-176 | Script hook arguments parsed by hand | 7.11 | landed `52abefe0` | a typed `add_script_action`; `add_script_action_typed` (`c5ed7c39`), the shell's `start_run` and the sample's `reload_run` on it, `D-250`; 01's site at 7.13 |
| S-188 | `Counters` inserted when missing | 7.11 | landed `52abefe0` | has: the kit's `Counters`, `D-250` |
| S-193 | The patrol enemy's place restated in a test | 7.11 | landed `52abefe0` | has: the test reads `ENEMIES`, `D-250` |
| P7-16 | Seeded generation | 7.12 | open | Depends on P7-1 (7.8); the registry `remove` (A6) |
| P7-17 | Places, routes and rooms in our own format | 7.12 | open | Depends on P7-9 (landed) |
| S-85 | Engine settings as Rust literals in the level plugin | 7.12 | open | + scene sections for gravity, the broadphase cell, the particle budget and the post stack |
| S-129 | A prefab's art scale passed in code at every spawn | 7.12 | open | + a scale in prefab data |
| S-140 | Each actor's prefabs restate its body file's box and hurtbox | 7.12 | open | + prefab variants by `extends` |
| S-144 | Map object names re-read from the object layers | 7.12 | open | + map objects spawned with their names |
| S-178 | Structures drawn whole, then cut into cell-sized tiles | 7.12 | open | + tiles larger than their cell |
| S-7 | Looks animated by hand from an age each frame | 7.13 | open | + has: 01's looks as tweened sprites ended by a `Lifetime` |
| S-13 | Constants restating prefab, ability, damage-type and art data | 7.13 | open | + has: values read off the loaded prefabs, abilities and damage types |
| S-21 | A thrown dynamic body as a projectile in 01's own code | 7.13 | open | + has: the snowball as a kit projectile on P7-2's bodies as walls |
| S-30 | Per-kind data set in Rust | 7.13 | open | + has: 01's components registered and named in its prefabs |
| S-32 | A thick ray traced by hand against bodies and tile points | 7.13 | open | + has: each trace a `cast_shape` |
| S-34 | Per-kind tables in Rust | 7.13 | open | + has: 01's per-kind tables as data kinds |
| S-47 | 01's `Hit` builder restating the kit's `DamageRequest` builder | 7.13 | open | + has: 01's hits as `DamageRequest`s once 7.10a lands; with them S-12's numbers on the kit's type label and P7-7's armour on the kit's sides |
| S-55 | Clip records built by hand in each art builder | 7.13 | open | + 01's tools: one clip helper, in 7.13's asset session |
| S-74 | Props larger than a tile cut into 64 px pieces | 7.13 | open | + has: big props placed whole, in 7.13's asset session |
| S-83 | Boxes tested against the collision layer by indexing its tiles | 7.13 | open | + has: the tests call `overlap` on tiles |
| S-98 | Equality guards before UI setters that already skip an equal value | 7.13 | open | + has: 01's direct setter calls; the setters already skip an equal value, so no §2.5 item has a cost to take |
| S-100 | A pickup's effect and burst run by a system of 01's | 7.13 | open | + has: the orbs' `picked_up` cues |
| S-106 | Places in the map by coordinates in code and tests | 7.13 | open | + has: the pit's spots as named markers |
| S-107 | Integer hashes hand-rolled to a unit float | 7.13 | open | + has: `splitmix64` for each unit draw |
| S-111 | Entities classed by a marker component or a list of component kinds | 7.13 | open | + has: tags for 01's and the template sample's markers |
| S-115 | Fire shaded by copied code | 7.13 | open | + 01's tools: one flame field, in 7.13's asset session |
| S-126 | A projectile's wall meetings answered in code | 7.13 | open | + has: wall meetings brought to cues by `CuesOnEvent` |
| S-128 | Parts tied to an owner by hand | 7.13 | open | + has: parts spawned as children |
| S-131 | The generator and its tests re-check what `tungsten check` reports | 7.13 | open | + has: `tungsten check` after the generator, in 7.13's asset session |
| S-136 | The level's own game clock beside the engine's | 7.13 | open | + has: `Time::elapsed` |
| S-141 | Loop crossfades written inline beside 01's own `crossfaded` | 7.13 | open | + has: `crossfaded`, in 7.13's asset session |
| S-151 | The summoning ring drawn three times | 7.13 | open | + has: `icon_of`, in 7.13's asset session |
| S-158 | A dive's speed as a stats override added and removed by hand | 7.13 | open | + has: the dive's speed as an effect |
| S-160 | Map facts restated as constants | 7.13 | open | + has: the map's facts read off the loaded map |
| S-161 | The dithered alpha quantiser copied | 7.13 | open | + 01's tools: `white_alpha` takes its steps, in 7.13's asset session |
| S-164 | A test that every cue rule's sound is in the manifest | 7.13 | open | + has: the kit's `cue_rules` check |
| S-166 | Palette ramps picked by hand | 7.13 | open | + has: `pixels.ramp`, in 7.13's asset session |
| S-169 | A shooting star moved and despawned by hand | 7.13 | open | + has: `LinearMotion` and `Lifetime` |
| S-170 | A tell's or a throw's seconds restating its clip's length | 7.13 | open | + has: a state ended by its clip |
| S-174 | Art sizes restated as constants | 7.13 | open | + has: sizes read off the registry's sprites |
| S-177 | An immediate recursive despawn beside the kit's deferred one | 7.13 | open | + has: `despawn_recursive` flushed at once |
| S-180 | A child's offset and its transform both written | 7.13 | open | + has: the look ordered before `TRANSFORM_PROPAGATE` |
| S-181 | An animated sprite's first frame named by hand | 7.13 | open | + has: `current_sprite` |
| S-185 | A once-a-frame guard on a fixed-step press | 7.13 | open | + has: the take as a plain fixed-step system |
| S-189 | An attack's seconds restating its ability's hitbox window | 7.13 | open | + has: a state ended by its hitbox's window |
| S-190 | A push skipped in code for a kind that resists knockback whole | 7.13 | open | + has: the kit's whole resist |
| S-192 | A slot's charge pips by spell kind | 7.13 | open | + has: the ability's `charges` |
| K12 | `World::has` scans the type list | 7.14 | open | Item C4 (§2.5) |
| K13 | `animation_tick` resolves clips by name | 7.14 | open | Item C3 (§2.5) |
| S-24 | A bounded pool of fire emitters hopped along the burning crest | 7.14 | open | C2: burning looks at 12,000 bodies, one emitter over many points |
| S-35 | A ball's save hand-packed into a flat array | 7.14 | open | C5: a prefab's runtime spawns saved as a table |
| S-54 | C1's frame-time capture kept as an ignored test beside the `e01-pit` row | 7.14 | open | C1: the `e01-pit` row measures it; the ignored test goes (known issues 39, 40) |
| S-90 | Each small ball's rainbow hue turned by the CPU every frame | 7.14 | open | C3: `ball_look`'s `powf` a ball (known issue 41) |
| S-184 | Enemies far from the player held still by 01's distance check | 7.14 | open | C3: an activity radius that holds far machines and movers |
| S-191 | Three autosaves' times taken from `SaveSlots::list` | 7.14 | open | C5: a per-slot header read (known issue 36) |
| P7-11 | More lights than the cap | 7.15 | open | Item G6 (§2.5) |
| S-14 | Backdrop quads culled on the CPU under opaque tiles | 7.15 | open | G2: the backdrops under opaque tiles, culled by the renderer |
| S-23 | One-off particle bursts capped and despawned by hand | 7.15 | open | G4: a capped burst budget that keeps key bursts, drained emitters despawned by the engine |

7.16 holds no register row: its sketch is the redo's 7.16 row. 7.5's triage (§2.4) sent S-rows to every other open step, the template's to 7.11 and those only 01 needs to 7.13.

### 2.3 Phase 8 rows

| ID | Pattern | Step | Status | Note |
| --- | --- | --- | --- | --- |
| P7-3 | Something in range: `Interactable` | 8.1 | open | With P8-1 |
| P8-1 | Dialogue | 8.1 | open | |
| P8-2 | Quests and the journal | 8.1 | open | On P8-1 |
| P8-3 | Items, inventory and shops | 8.1 | open | Equipment as statuses (Q17; A3) |
| P8-4 | Cutscenes | 8.1 | open | On 7.18's rigs |
| P8-5 | Typed text | 8.1 | open | |
| P8-6 | Clock and schedules | 8.1 | open | Depends on P8-8 (8.2): 8.1 places NPCs by time without paths (Q9) |
| P8-7 | Loading between areas | 8.1 | open | On P7-8 (7.9) |
| P8-17 | A cone light kind | 8.1 | open | The redo's 8.1 row holds it |
| PA-12 | Items with rolled lines | 8.1 | open | After PA-8 (7.10) |
| PA-13 | Ground items | 8.1 | open | Styled by rarity name; 8.7's `rarities` rolls it |
| S-162 | Columns lined up across rows by hand, with no grid layout | 8.1 | open | + a grid layout; moved from 7.17 (`D-124` keeps grid layout out until a user needs it; 8.1's inventory and shops are the first) |
| P8-8 | Navigation | 8.2 | open | On P7-1 (7.8); platform links for 8.4 |
| P8-9 | Perception | 8.2 | open | On P7-4, P7-5 (7.10) and K31 (7.8) |
| P8-10 | Squads and cover | 8.2 | open | |
| P8-11 | Weapons as data | 8.2 | open | On P7-6 (7.10) |
| P8-12 | Hit-stop | 8.2 | open | |
| P8-13 | Bullet patterns and waves | 8.2 | open | |
| PA-14 | Count and pierce from the sheet | 8.2 | open | On P8-13's pattern payload |
| PA-15 | Chain and fork | 8.2 | open | |
| PA-16 | Repeats | 8.2 | open | On P8-11's bursts |
| S-6 | A walker's way over what lies ahead, by casts | 8.2 | open | navigation on platforms: hops, drops and leaps |
| P8-15 | Drawn art | 8.3 | open | `asset add` and Aseprite import |
| S-8 | Each stop's asset contract restated as test tables | 8.3 | open | + asset contracts `tungsten check` holds each file to |
| S-11 | 01's own asset pipeline | 8.3 | open | + per-family generation in `tungsten` (the redo's §8 Assets row) |
| S-22 | Actors' bodies as 01's own data kind | 8.3 | open | + named points mirrored by facing and a body section in prefab data |
| S-61 | A point per animation frame | 8.3 | open | per-frame anchors, as slices import |
| S-69 | Lit sprites' normal maps and emissive masks derived by 01's tools | 8.3 | open | + normal maps and emissive masks derived by the pipeline |
| PA-3 | Area and projectile speed read | 8.7 | open | Moved from 7.10 (PA-Q2, taken early on 2026-10-31: ARPG only, and the ARPG design's §16 moves it with no rework); §9.1's sheet reads 690 px/s |
| PA-4 | Use time | 8.7 | open | Moved from 7.10 (PA-Q2, as PA-3); on P7-6's held trigger (7.10c); §9.1's use time reads 0.721 s |
| PA-9 | Primary stats | 8.7 | open | Moved from 7.10 (PA-Q2, as PA-3); `per` lines on 7.10a's scoped lines |
| PA-17 | Supports | 8.7 | open | `build` domain (PA-Q1) |
| PA-18 | Passive tree | 8.7 | open | |
| PA-19 | Item generation | 8.7 | open | |
| PA-20 | Uniques and handlers | 8.7 | open | |
| PA-21 | Monster level and rarity | 8.7 | open | |
| PA-22 | `explain` and build saves | 8.7 | open | On PA-11 (7.10) and 7.9's saves |
| P8-14 | Rumble | — | closed (G5: none in 1.0) | The two-thread rule stands |
| P8-16 | Aim assist | — | closed (stays the game's) | Until a second game asks |

### 2.4 7.5's register

- 7.5 numbers its rows `S-1`, `S-2`, … (Q8); a row it re-reports from the K-rows keeps its K ID beside its S ID.
- Its triage sends each row by §2.1's rule: a system a Phase 7 step builds → that step; a template row → 7.11; a fix only 01 needs → 7.13; a Phase 8 system → its 8.x step; anything else → Later as one line.
- A performance workaround it finds joins the evidence of a §2.5 item, never the list; a cost no item covers waits for 9.2.
- A row for a step already started goes to the first unstarted step that touches that system.
- The triage's commit writes every row into §2.2 (Phase 7 steps, with `Later` or `9.2` in the step cell and status for the rest) or §2.3 (Phase 8 steps), so each S-ID has one disposition in the tables.

### 2.5 7.14's and 7.15's finite lists

The candidates are the redo's 7.14 and 7.15 rows; the workload is 01 on the new systems (7.13). The evidence below orders them; it is dated and from older trees, so it ranks and does not set targets.

- **Gate at each pass's start:** the pass measures each item on its row with 7.3's runner on 7.13's tree and records the item's share of the row's time. An item under 5 % of its row leaves the list (`dropped`, with the number). Nothing joins (Q4). G6 is a register row (P7-11), so the floor never drops it: it closes on its behaviour.
- **Target:** remove at least half of the item's measured share, as an absolute time on its row, recorded at the gate (Q4). G6's target is behavioural.
- **End:** every item `done` (its target met, a not-`regressed` whole-frame row and the other resource's rows beside it), `dropped` at the gate, or `stopped` (a profile shows the remaining cost is the work itself; not for G6). Everything else waits for 9.2 (§5.8).

**7.14, CPU:**

| # | Item | Row (7.3's set or the step that adds it) | Evidence today | Status |
| --- | --- | --- | --- | --- |
| C1 | The spatial query index after 7.8 (K14) | 01's full pit, `physics_step`; `physics-sparse` | `physics_step` 2.07 of 3.71 ms p50 (56 %) in 01's full pit, release, 2026-10-23 at M81's tree (`perf-runs/20261023-e01g-frame/after.txt`); A12; S-54: the ignored frame-time test beside the `e01-pit` row; the physics review of 2026-11-01 (`perf-runs/20261101-physics-review/round1/a4/`, its notes checked against the code): `refresh` rebuilds the bodies' part on the first query after every step, a sleeping pit's proxies included, and inside a step with sensors or sensor pairs (`sensor_overlaps`); a static at the loader's 16,384 px half extent covers 1,048,576 to 1,050,625 of the index's cells, the first ray after a step 4.16 ms against 0.004; a query inside a `_where` cast's `keep` builds a temporary index; and the index, keyed by `tiles_version`, keeps the runs of a `PhysicsBuffers` replaced or removed under it, so the next cast panics in `run_cells_in` (`gather.rs`), which no workspace code does; its second round (`round2/a4/`): with a sensor or a sensor pair the index rebuilds inside every step, the sensor pass tests each sensor against every other's box (sensors squared a step), and a configured cell size under 1 clears and refits the grid at every rebuild | open |
| C2 | Interaction spread (7.10's chemistry) | 02's `spread` row (12,000 receivers, `71fe6b3a`, §5.8) | Every receiver lit: `system.chemistry` p50 0.38 ms, p95 12.3 ms, its peaks the steps where all 12,000 emit at once, every 15 frames; `system.effect_tick` p50 2.5 ms, p95 4.1 ms; a run clones its rules (a diagnostic capture beside a remote desktop, 2026-11-03, `perf-runs/20261103T221523Z-spread/`); 01's `fire_spread` rebuilt a cell map of every unburnt ball per step (P5, unmeasured); S-24: 01's pool of fire emitters hopped along the burning crest | open |
| C3 | Per-entity kit systems: `animation_tick` resolving clips by name (K13) | 01's full pit, `update`; `particles` `animate_sprites` | 0.90 ms for 12,000 balls on one clip (K13, at 0.97); interned animation IDs, an open proposal in `docs/perf/benchmarks.md`; S-90: `ball_look`'s `powf` a ball, 0.26 ms for 12,000 (known issue 41); S-184: far enemies held by 01's distance check | open |
| C4 | `World::has` and the query paths (K12) | `ecs` `update`; 01's full pit | No isolated time; P1–P7 moved 01 off per-entity `has` (feedback §1); a query shape's kept matches regrow, 2 allocations and 128 bytes a shape at the pit's seven words, each time 01's pit adds an archetype, matching or not, which stopped 7.10g's Step 3 (`464cbf73`) until `World::has_any` gated its discovery (`D-247`) | open |
| C5 | The save snapshot | 7.9's save row (§5.5's autosave cost) | Not built; 01's `save.rs` is 2,873 lines (A5); S-35: a ball's save hand-packed so 12,000 fit; S-191: the autosave rotation parses every slot whole (known issue 36) | open |
| C6 | The extract | 01's full pit, `extract`; `gpu-throughput` | 0.13 of 3.71 ms p50 (3.5 %) in 01's full pit (as C1); 13.3 ms at 400,000 sprites in `gpu-throughput` (2026-10-02) | open; likely dropped at the gate in 01 |

**7.15, GPU** (local, `D-070`):

| # | Item | Row | Evidence today | Status |
| --- | --- | --- | --- | --- |
| G1 | The lit-sprite path in the scene pass | 01's area row, scene pass (Q5); `gpu` scene pass | The scene pass is 13.3–23.9 of 19.4–30.1 ms `render_span` p50 (69–79 %) in six areas at 2160p, the mine's spot under game over's passes (M77's survey, 2026-10-22 at 0.97, `perf-runs/20261022-m77-example-01-fixes/gpu-areas/`); `gpu` 6.88 of 10.39 ms (2026-10-02) | open |
| G2 | Tile layers and the layered backdrops (overdraw) | 01's area row, scene pass | The backdrops take 3.2–4.9 ms of the scene pass at 2160p (`gpu-areas-nobackdrop/` against `gpu-areas/`); S-14: 01's CPU cull of backdrop quads under opaque tiles | open |
| G3 | Bloom and the post stack | 01's area row, the post passes; `gpu` pass rows | The bloom chain about 3.8 ms and the vignette pass 1.8 ms at 2160p: 19–29 % of `render_span` (same survey) | open |
| G4 | Particle quads | `particles`; 01's spray frames | Inside the scene pass, not separated today; S-23: 01's burst cap and its despawn of drained emitters | open |
| G5 | Sprite batching and upload bytes | `gpu-throughput`; 7.3's local upload-byte counter | `gpu` `render_encode` 2.12 ms p50 (2026-10-02) | open |
| G6 | The light cull past 16 by importance (P7-11) | 01's lantern scene; `gpu` light sweep | Cap 16; over it, zero-intensity lights drop and the rest rank by distance to the view, with nothing pinning the player's light (A13, `crates/tungsten-render/src/lighting.rs:133-150`) | open; target: the player's light never culled and a dark light holding no slot, each shown by a test past the cap; the scene pass not `regressed` |

M77's survey predates M82's look and M85's scenery, so the order may change at the gate.

### 2.6 Evaluations and stress rows

- **§8's five tasks:** 7.13 (in 01 and a `tungsten new` copy) and 8.5.
- **The paper designs' weekly tasks** (§2, §3 there): 7.13 takes LW3 (a lit path: 7.10's light query, 7.12's authoring), SF1 (a room template: 7.12's rooms and seeds) and SF4 (a status band's decay: 7.10's statuses). The other seven need Phase 8 systems (dialogue, items, cutscenes, weapons, AI, perception); 8.6 takes all ten.
- **The ARPG design's five tasks** (§12 there): 8.6.
- **Stress rows:** the paper designs' stress numbers and the ARPG design's seven rows (§13 there): 9.1.
- **System rows** (§5.8): 7.8 friction and slope piles; 7.9 the save snapshot and boot to first frame; 7.10 interaction spread at 12,000 bodies; 7.17 and 7.20 their layout counter guards; 7.18 none; 7.21 none.

### 2.7 7.19's assumptions, rechecked

The redo's §7.3 and the ARPG design's §1 list them; every one holds at `08111d5a`.

| Assumption | The tree | Verdict |
| --- | --- | --- |
| §5.2's statuses and Q17's merge | No status type; effects and ladders stand (A2); §5.2 and Q17 unchanged since 2026-10-28 | Holds: 7.10b builds statuses with PA-1's lines as their stat changes and PA-8's user scaling |
| P8-3's equipment as statuses | No item type (A3); the redo's 8.1 row keeps equipment as statuses. P8-3 says "kit effects", Q17's earlier name for them | Holds |
| The modifier formula, unchanged | A1 | Holds; no step changes it |
| P8-11's and P8-13's payloads | None today (A4); 8.2's row holds both | Holds: PA-14 and PA-16 cannot land before 8.2 |
| §5.5's saves and P7-16's seeds | Names and save IDs in 01 only; no registry; no `remove` (A5, A6) | Holds: PA-22 waits for 7.9. PA-19's item seeds are its own (T5 there), so P7-16 is not on the ARPG's path |
| Q11's names | `react` holds cues; no `chemistry` or `build` (A7) | Holds: 7.10b adds `chemistry`, 8.7 adds `build` (PA-Q1) |

The brief's kit survey still holds where a row rests on it: crit is a flag and `area` and `projectile_speed` are unread (A8), so PA-3 and PA-5 stand.

### 2.8 Order, tracks and routes

- **One track,** a step at a time in this tree (2026-10-31, the owner's choice): 7.2 → 7.3 → 7.4 → 7.5 → 7.8 → 7.9 → 7.17 → 7.20 → 7.10 → 7.18 → 7.11 → 7.21 → 7.12 → 7.13 → 7.14 → 7.15 → 7.16. The UI steps no longer run beside 7.8–7.10 in a worktree: a session beside the track would share the CPU with each close's timed rows and leave the owner a merge across version branches, and 7.9 changes the template's saves, the one file set they share. Steps 6 and 7 keep their numbers, which plans cite, so Step 7 (7.9) runs before Step 6 (7.17). 7.21, core hardening, joined between 7.11 and 7.12 on 2026-11-07 (the owner's choice), from track C of that day's big session; its plan was written that day too, so 7.11's hand-off is 7.21's run prompt, not a plan prompt; it keeps the number Step 18, after the close's 17, and runs between Steps 11 and 12 as Steps 6 and 7 run out of number order.
- **Fixed orders** (the redo's §3): 7.9 → 7.17 → 7.20 → 7.11; 7.18 after 7.10 and before 7.13; 7.5 before 7.8, so its rows reach the steps that build them.
- **Routes:** every step is large under `D-140` and writes its own plan with the tungsten-plan skill, except 7.16, which is medium unless its research spans sessions. Where a step needs several plans:
  - 7.8: runtime terrain, then physics with slopes and the controller last (the redo's 7.8 row).
  - 7.9: saves of many areas, then boot loading with sprite sheets and the small seams.
  - 7.10 (Q3): **7.10a** stats and hits (PA-1, PA-2, PA-5–PA-7, PA-10, PA-11, P7-7; 7.5's S-12), the shapes the rest stand on; **7.10m**, the motion plan, after 7.10a, whose hits its impact damage and knockback carry: the impulses and the controller's rules 7.8 moved on, the known issues' slope-cast fix ahead of the slope rules, and the motion 7.10b's blast stands on (P7-2's physics and kit parts, K7, K19, K20, K23, K30; 7.5's S-92, S-99, S-132, S-134); **7.10b** statuses and chemistry on them, with the blast its outputs need (P7-4, P7-5, PA-8, S-4 (K10)); **7.10b's chemistry**, its interactions, rates, spread, tiles, `become` and blast, re-planned after 7.10b's step 3 stopped (P7-4's rest, PA-8's rates, S-4); **7.10p**, the looks and cues on them, after 7.10b, whose statuses its looks and status cues read (K27, K28, P7-13's kit part, S-122 (K1); 7.5's S-1, S-39, S-41, S-48, S-51, S-56, S-79, S-96, S-121, S-154, S-165); **7.10c** abilities, their payloads and the genre tags (P7-6, K8, P7-18; 7.5's S-5, S-15, S-17, S-18, S-53, S-73, S-88, S-110, S-130, S-135, S-186), the genre tags first, since every later 7.10 item takes its recipes in them; **7.10g**, the rest of 01's generic code, after 7.10c, whose genre tags its recipes use and whose lob its carry throws with: one-shot clips and clip rules, a hitbox and attachments by facing, the lifecycle and motion items, a health bar and the debug view's hotkey (P7-19, P7-20, K3, K22; 7.5's S-10, S-19, S-20, S-31, S-37, S-52, S-66, S-78, S-84, S-86, S-105, S-142, S-150, S-156, S-159). K11 moved to 7.13 as S-36, the kit's `Cooldown` already being its timer (§2.2). Each plan updates the kit's determinism pin where it changes what the hash serializes (§5). The order is 7.10a → 7.10m → 7.10b → 7.10b's chemistry → 7.10p → 7.10p's Step 2 → 7.10c → 7.10g (2026-10-31; 7.10p on 2026-11-03, when 7.10b's plan passed ten steps with every row, §6; 7.10b's chemistry the same day, when that plan's step 3 stopped and its steps 4 to 8 with it; 7.10p's Step 2, the cue-events plan, re-planned on 2026-11-04, when it stopped at a name 01 holds; 7.10g the same day, when 7.10c's plan passed ten steps with every row): 7.10m took the six rows 7.8 moved on, which no plan held, and 7.10c's motion; PA-3, PA-4 and PA-9 moved to 8.7 then, PA-Q2's move taken early (§2.3).
  - 7.12: the format and CLI, then the editor (the redo's 7.12 row).

## 3. Steps

Each step is a redo step; its own plan holds its mechanisms, files, checks and commits. **Every step's done-when** adds these to the specifics below:

- No row of the step left open: `rg -n '^\| [^|]+ \| [^|]+ \| <step> \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0.
- Its plan closed: deleted with its citations, `just repo-check` → passes.
- This file's evidence row written in the step's closing commit, terse as the [plans README](README.md) asks, in about 800 characters: the verdict; the plan and its commit range; its own evidence log, cited by the last commit that held it; at most five key numbers from its done-when, the demo's among them; how many rows landed and moved (the tables name which); the capture paths.
- From 7.8 on, a system step's benchmark row and counter guard in 7.3's runner (§5.8), except 7.18 (§2.6) and 7.21, which adds no system.

**Hand-off:** each step ends with the plan prompt of the next step in §2.8's order, from tungsten-next's `step` row with this file and the redo's row as its sources.

### Step 1 (7.2): Agent tooling foundations

- **Rows:** none. **Sketch:** the redo's 7.2 row and §8's foundations row.
- **Done-when:** the sketch's three commands (`run --script`, `render`, `describe`) work as its plan's checks show; an asset review sheet comes from `render` without a scratch crate.
- **Moves:** none here.

### Step 2 (7.3): Benchmarks, part 1

- **Rows:** none. **Sketch:** the redo's 7.3 row and §5.8's part 1.
- **This plan asks of it:** a row set (Q12) that measures every §2.5 item: 01's full pit, the template frame, `ecs`, `particles`, `physics-sparse`, `gpu`, `gpu-throughput`, a local GPU row of 01's areas (Q5), and the upload-byte and draw-call counters locally.
- **Done-when:** the sketch's runner, guards, ported rows and purge, as its plan's checks show; each row named here is listed by the runner.
- **Moves:** retires per-milestone sittings (the rewrite's R10).

### Step 3 (7.4): H2

- **Rows:** none. **Sketch:** the redo's 7.4 row and §4.
- **This plan asks of it:** tungsten-next's prompts gain a phase plan's hand-off, a step's plan prompt in place of the `run` prompt (follow-up F2).
- **Done-when:** the sketch, as its plan's checks show.

### Step 4 (7.5): Analysis pass

- **Rows:** none of §2; it writes the seam register. **Brief:** the redo's §7.1. Its seeds include the K-rows; those §2.2 already assigns are re-reported only with new evidence.
- **Done-when:** the register written, verified and triaged; every `S-` row in §2.2 or §2.3 with one disposition by §2.4 (a step, `Later` or `9.2`): the register's row count equals `rg -c '^\| S-[0-9]+ ' docs/plans/phase-7.md`.

### Step 5 (7.8): Terrain and physics

- **Rows:** P7-1, P7-2 (physics), P7-3 (sensor exit), K7, K14, K23, K30, S-60 (K31); 7.5's S-16, S-26, S-65, S-77, S-99, S-112, S-132, S-172, S-175. **Design:** §5.1, §5.6.
- **Done-when:** the redo's 7.8 sketch; friction and slope piles as rows (§2.6).
- **Moves:** the physics hashes, under 7.3's guards.

### Step 6 (7.17): UI pass

- **Rows:** 7.5's S-9, S-28, S-33, S-42, S-45, S-62, S-64, S-67, S-81, S-82, S-91, S-93, S-94, S-102, S-104, S-108, S-109, S-114, S-116, S-123, S-127, S-133, S-139, S-149, S-152, S-153, S-155, S-157, S-162, S-163, S-171, S-183, S-187. **Design:** §5.9. After 7.9 (§2.8).
- **Done-when:** the redo's 7.17 sketch, the shell's `tokei` count against `D-199`'s 2,900 measured, and the idle-screen counter guard.

### Step 7 (7.9): Saves and engine seams

- **Rows:** P7-8, P7-10, P7-13, P7-14, P7-15, K2, K4, K9, S-38 (K21); 7.5's S-2, S-3, S-25, S-27, S-29, S-40, S-43, S-46, S-58, S-59, S-63, S-70, S-71, S-76, S-87, S-89, S-95, S-97, S-101, S-113, S-117, S-125, S-137, S-138, S-143, S-148, S-173, S-179. **Design:** §5.4, §5.5.
- **Done-when:** the redo's 7.9 sketch, with §5.5's acceptance test (a restore into a world with other entity allocations, 01's save tests unchanged); the save snapshot and boot rows.

### Step 8 (7.20): Screens as data

- **Rows:** 7.5's S-72. **Design:** §5.11. After 7.17.
- **Done-when:** the redo's 7.20 sketch and the reloaded-screen counter guard.

### Step 9 (7.10): Kit pass

- **Rows:** every row §2.2 sends to 7.10, as 7.10a, 7.10m, 7.10b, 7.10b's chemistry, 7.10p, 7.10c and 7.10g, in that order (§2.8). **Design:** §5.2, §5.3, §5.6 for 7.10m, the ARPG design's §3–§8 and §16.
- **Done-when:** the redo's 7.10 sketch; the ARPG design's §16 row for 7.10 (§9.1's hit resolves to 32.07 in a kit test, its sheet's speed and use time being 8.7's with PA-3 and PA-4; 01's crit, blast share and difficulty are data with its tests' outcomes unchanged); the spread row at 12,000 bodies.
- **Moves:** the kit's determinism pin, at most once per 7.10 plan, and the physics hashes in 7.10m, each under 7.3's guards (§5).

### Step 10 (7.18): Camera rigs

- **Rows:** 7.5's S-75, S-146. **Design:** §5.10. After 7.10, before 7.13.
- **Done-when:** the redo's 7.18 sketch; a switch never jumps at 30, 60 or 144 Hz.

### Step 11 (7.11): Template updates

- **Rows:** 7.5's template rows (§2.4). After 7.9, 7.17 and 7.20.
- **Done-when:** the redo's 7.11 sketch within `D-181`'s budget.

### Step 18 (7.21): Core hardening

- **Rows:** none of §2; its list is 23 findings, `docs/known-issues.md`'s seventeen physics rows that had no step (the row on colliders built past the `rigid_body` caps stays with 7.14's C1) and the six rows its oracles found. **Plan:** `core-hardening`, written 2026-11-07. After 7.11, before 7.12 (§2.8).
- **Done-when:** its plan's checks: each finding's repro test committed before its fix, the two oracles' default runs and long modes passing, the physics hashes held (§5); no benchmark row, since it adds no system. No sitting of its own (`D-226`): the minor release after 7.21 sits `e01-pit`, `friction`, `physics`, `physics-sparse`, `slopes` and `terrain` as its perf check, since counter guards miss per-pair and per-sleeper cost (the gap `D-233` met).
- **Moves:** the counter guards and tests each of its steps names, and 02's row digests where a step changes contacts; no physics hash.
- **Landed in part** (2026-11-08): steps 8, 9, 11, 12, 13, 15, 22 and 24 stopped at their checks; each finding's row stays in `docs/known-issues.md` with its next work and its evidence under `perf-runs/20261107-core-hardening/`.

### Step 12 (7.12): Level format, agent CLI, editor

- **Rows:** P7-16, P7-17; 7.5's S-85, S-129, S-140, S-144, S-178. **Design:** §5.1.
- **Done-when:** the redo's 7.12 sketch; `check`'s props supported generalizing `D-214`.

### Step 13 (7.13): 01 on the new systems

- **Rows:** 7.5's 01-only rows (§2.4), and the template shell's that 7.11 moved (§2.2). **Tasks:** §8's five, LW3, SF1, SF4 (§2.6).
- **Done-when:** the redo's 7.13 sketch; each task's friction recorded as a seam and the rerun complete; the owner accepts 01.

### Step 14 (7.14): Performance pass, CPU

- **Rows:** K12, K13; 7.5's S-24, S-35, S-54, S-90, S-184, S-191, the evidence of C2, C5, C1, C3, C3 and C5; **list:** C1–C6 (§2.5).
- **Done-when:** the gate's shares recorded; each item `done`, `dropped` or `stopped` in §2.5; the whole-frame row and the GPU rows not `regressed`.

### Step 15 (7.15): Performance pass, GPU

- **Rows:** P7-11; 7.5's S-14, S-23, the evidence of G2 and G4; **list:** G1–G6 (§2.5).
- **Done-when:** as step 14, with the CPU frame, the extract included, not `regressed`.

### Step 16 (7.16): Agent guidance

- **Rows:** none. **Sketch:** the redo's 7.16 row, against 7.13's evaluation.
- **Done-when:** what the template ships decided and recorded.

### Step 17: Close

- **Files:** this plan (deleted), `ROADMAP.md`, `docs/plans/roadmap-redo.md`, `docs/plans/arpg-design.md`, the follow-ups' homes.
- **Change:** carry §2.3's open rows into the redo's §3 Phase 8 rows by ID (PA-12 and PA-13 into 8.1, PA-14–PA-16 into 8.2, P8-6's note into 8.1, PA-3, PA-4, PA-9 and PA-17–PA-22 named in 8.7); point the ARPG design at the redo; move open follow-ups to known issues or Later; delete this plan and its citations; take `phase-7` out of Now.
- **Done-when:** no Phase 7 step's row open (`rg -n '^\| [^|]+ \| [^|]+ \| 7\.[0-9]+ \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0, run before the deletion); every §2.2 row's status terminal (§2.1), each `Later` row a line in ROADMAP.md's Later, each `9.2` row in the redo's 9.2 lever list, and each list item done, dropped or stopped, as the last evidence row quotes; every open §2.3 row in the redo, moved and S-rows included (`rg -o -r '$1' '^\| ([A-Z0-9-]+) \| [^|]+ \| 8\.[0-9]+ \| open \|' docs/plans/phase-7.md | while read -r id; do rg -q "$id\b" docs/plans/roadmap-redo.md || echo "$id"; done` → prints nothing, run before the deletion); `just repo-check` → passes; example 01 runs.
- **Ends with:** the commits made, the evidence quoted, what the owner should play, and the next prompt (8.1's plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | 7.7 ran before 7.2–7.5. Re-cut the order? | No: the redo's order stands; 7.5's rows join by §2.4, and the lists' numbers come from 7.3's rows at each pass's start |
| Q2 | The K-rows the redo's §3 doesn't cite (K1, K11, K19–K23, K27, K28, K31) | The steps in §2.2; 7.5's triage may re-cut them with new evidence |
| Q3 | 7.10 as three plans in dependency order | Yes, as §2.8; four since 2026-10-31, 7.10m holding the motion 7.8 moved on, with PA-3, PA-4 and PA-9 moved to 8.7 (PA-Q2); five since 2026-11-03, 7.10p holding 7.10b's looks and cues (§6's ten steps); six the same day, 7.10b's chemistry re-planned after its step 3 stopped; seven on 2026-11-04, 7.10g holding 7.10c's rows past its abilities, payloads and genre tags (§6's ten steps) |
| Q4 | The lists' floor and target | A floor of 5 % of the row's time at the gate; a target of half the item's measured share; G6 behavioural |
| Q5 | Where 7.15 measures 01 | A local GPU row of 01's areas in 7.3's set, M77's area survey made a row |
| Q6 | Where Phase 8's assignments live after this plan | The redo's §3 rows by ID (step 17); the ARPG design stays until 8.7 closes |
| Q7 | `phase-7` in Now for the whole phase | Yes, beside the step in work; one track since 2026-10-31, so Now holds two |
| Q8 | 7.5's row IDs | `S-1`, `S-2`, … |
| Q9 | P8-6's schedules depend on P8-8's navigation, a step later | 8.1 resolves schedules by time and places NPCs without paths; walking between places waits for 8.2 |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** a row's step, only by §2.1's move with its reason; a list item, only out, at its gate. The kit's determinism pin (`crates/tungsten/src/tests/kit_recipes/determinism.rs`) at most once per 7.10 plan, since it serializes effects and buckets (7.10b) and each genre's world (7.10m, 7.10c), each move with its cause in the evidence log; this splits §5.2's and the ARPG design's §16's "moves once" across 7.10's plans (Q3).
- **Must not move:** the redo's fixed orders (§2.8); the lists' membership beyond the gate; the determinism test's two-run assertion, which holds at every commit. Engine invariants are each step's plan's: the physics hashes move only in 7.8 and with the rows 7.8 moved on (7.10m's, and K14 in 7.14's C1), each under 7.3's guards with its cause in the evidence log; the pinned containment hash, the row digests, `gpu-visual.png` and the post and transition regressions move only where a step's plan says so.

## 6. Stop conditions

- A step's done-when fails: its rows stay open, steps that depend on it wait, and a later step that doesn't may run first, within §2.8's fixed orders. A step's own plan handles its failed checks as the skill says.
- A row needs a system no Phase 7 step builds: it moves to Phase 8 or Later, with its reason.
- A row's dependency lands after its step: it moves to the first step after the dependency.
- A 7.10 plan passes ten steps or its critique finds it too large: it splits by §2.8's dependency order (PA-Q2's three rows have moved).
- A list item can't reach its target: `stopped`, with the profile.
- A new performance finding: 9.2, never the list.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-29, reading `1b0c5236`. Footer: `463.2 s, tokens in 1378106 (cached 1272960), out 10056, commands 26, output 429.4 KB (largest 99.4 KB), usage 5h 0% → 28%, week 12% → 17%`. Five findings, all accepted; they changed done-when checks and 7.10's split, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Chemistry needs the blast implementation assigned to the following plan | The redo's §5.2 lists `blast` among chemistry's outputs (`roadmap-redo.md:246`) and replaces 01's detonations (`:252`); 01's blast is game code (`examples/01_platformer/src/features/elements/cascade.rs:634`) | accepted | K10 moves to 7.10b (§2.2, §2.8); P7-2's kit part keeps impact damage and bodies as walls in 7.10c |
| 2 | [MEDIUM] The single hash update conflicts with independently landing the kit subplans | The pin serializes effects and buckets (`crates/tungsten/src/tests/kit_recipes/determinism.rs:120`) and runs per genre (`:526`), so 7.10b and 7.10c each move it | accepted | §5 and step 9: the pin moves at most once per 7.10 plan, with its cause; the two-run assertion never moves |
| 3 | [MEDIUM] The performance gate can discard G6's unmet behavioral requirements | `cull_to_cap` ranks by direction and distance, with no player light (`crates/tungsten-render/src/lighting.rs:133-150`); it already drops zero-intensity lights over the cap, so G6's evidence was stale too | accepted | §2.5: the floor never drops G6, `stopped` is not for it, and its evidence and target name tests past the cap |
| 4 | [MEDIUM] A valid Later assignment makes the analysis pass fail its completion check | §2.4 sent rows to Later and 9.2 outside the tables that step 4 counts | accepted | §2.4 writes every S-row into the tables, `Later` and `9.2` included; §2.1 adds `9.2`; step 4's check counts dispositions |
| 5 | [MEDIUM] The phase's completion contract rejects terminal states its steps allow | The header's done-when omitted `Later` (K25) and `stopped` | accepted | §2.1 names the terminal states; the header and step 17 accept them, with each `Later` row in ROADMAP.md's Later and each `9.2` row in 9.2's list |

Round 2: the same reviewer at xhigh, 2026-10-29, reading `a577dfc1` with the Critique section and the log cut and a focus note on round 1's changes. Footer: `210.1 s, tokens in 511722 (cached 448256), out 4929, commands 22, output 299.2 KB (largest 50.1 KB), usage 5h 28% → 42%, week 17% → 19%`. Two findings, both repairs of round-1 fixes, both accepted; no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Moved rows can escape the Phase 8 handoff | §2.1 made `moved` terminal, and step 17 carried only §2.3's open rows and checked PA-12–PA-22 alone, so PA-3, PA-4 or PA-9 moved to 8.7 would be lost | accepted | §2.1: a move rewrites the step cell and keeps the row open, Phase 8 rows in §2.3; step 17 checks every open §2.3 ID against the redo |
| 2.2 | [MEDIUM] A valid `9.2` disposition still violates the overall completion condition | The header's done-when listed landed, moved, closed and Later | accepted | The header accepts §2.1's terminal states, `9.2` included |

Round 3, on the re-cut of 2026-10-31 (one track, 7.10m, PA-3, PA-4 and PA-9 to 8.7, the evidence rule): the same reviewer at xhigh, artifact-only, on the diff `35e1849c..0866a8c6`. Footer: `77.9 s, tokens in 31361 (cached 0), out 2070, usage 5h 59% → 64%, week 75% → 76%`. No concerns.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 (7.2) | 2026-10-29 | passed | `agent-tooling`, commits `2ecd7d41` (core script format and hooks), `bc1718ab` (the runner on the frame body and the harness), `1dbd0c71` (script mode in the window), `b4815fc9` (`run --script`, `describe`), `831811b0` (the template's and 01's hooks), `6e642b4d` (`render`, `D-225`) and the close; its done-when: every step's checks passed (its evidence log at `6e642b4d`); `just check` → passes (fmt, workspace clippy, every test and doctest); 01 through its own `cargo run` with a script (`start_run`, frame 180) → passed, `area` "Lantern clearing", the stack `["gameplay"]`; a review sheet of 01 from `tungsten render` with no scratch crate: the six foes' contact sheet (1280 × 720), a 20-frame campfire loop (5 distinct SHA-256 sums) and the first area's still with the physics overlay; step 2's perf gate, after a rerun and cold call points, 0 owned metrics `regressed` in both direct pairs; the plan deleted with its citations, its follow-ups in known issues, out of Now | `perf-runs/20261029-agent-tooling/` (sittings, step fixtures, `close/`), `perf-runs/20261029-agent-tooling-sheet/` |
| 2 (7.3) | 2026-10-29 | passed | `bench-runner`, commits `6664ed3b` (the plan) through `eea02d45` (world counters, `CounterGuard`), `a920cc49` (guards on the seven bench rows and 01's pit), `56b49dfa` (`scripts/perf.py`, `perf_stats.py`, `perf-rows.json`), `bb05f883` (a four-slot timestamp ring, `gpu_passes: from=N`, draw, upload and text counters), `38fd3929` (sittings: A/A, one rerun, quick, start check), `6c95b983` (the switch; `bench.py`, `bench_report.py` and `integrated` purged), `c0fc596d` (`D-226`), `ea98e798`, `cdaa4a3d` (a run's deadline), `3b2422b6`, `47bac794`, `4602f631` and the close; its done-when: `just perf list` lists 15 rows, every row this step names (01's full pit, the template frame, `ecs`, `particles`, `physics-sparse`, `gpu`, `gpu-throughput`, `e01-areas`' six areas); `just check` → passes, the guards included; `just perf counters` → gpu, gpu-throughput, template, e01-area-clearing ok; the first full sitting, base HEAD, every row `ok` in 2,320 s, provisional baselines saved as `provisional-0.113`; the GPU run's `total` against the timing run's fell from 1.28 to 1.01; the old runner gone, its findings in benchmarks.md's Levers for 9.2; per-milestone sittings retired (`D-226`); `just script-test`, `ctx`, `repo-check`, `api-check`, `docs-ratchet`, `smoke` (13/13 rows), `visual` → pass; `just perf run e01-pit --repeat 1` → valid, digest `7daaf2f10196ccad`; 01 through its own `cargo run` with a script → the player walks from x 368 to 1,212 in the Lantern clearing, health 100, passed; the plan deleted with its citations, its follow-up in known issues, out of Now | `perf-runs/20261029T163620Z-sitting` (the baseline), `perf-runs/20261029T171238Z-sitting`, `perf-runs/20261029T181822Z-sitting-quick`, `perf-runs/20261029T182915Z-counters`, `perf-runs/20261029T182922Z-e01-pit` |
| 3 (7.4) | 2026-10-29 | passed | `health-h2`, commits `c8105ad6` (the plan) with its critique rounds `fe83ae90` and `ccd6195a`; `62ab8efe` (H1's overhead retaken: the example group at 20.7 % of tool calls and 16.5 % of output tokens against 22.1 % and 20.0 %, a median of 10 record files a release against 12); `a49de384` (the carried proposals: deny-list bans, exact allows, `.editorconfig` and `.gitattributes`, the demo rule, client checks on Claude Code 2.1.311 and Codex 0.160.0); `ef9486cf` (`just status`); `ceff7e09` (releases on a tag push, `just release minor` and `rehearsal`, `D-227`); `0a383dbb`, `b0868291`, `9d6e41e5`, `376218d2`, `b1ed7a26` (the decision, release, next and plan skills rewritten against no-skill baselines on 7.1–7.3's tasks, every lean skill passing its blind-graded checklists, three of them after one revision; 11,838 bytes against 23,239; 6.74M tokens); `b3204331` (the memory hubs pruned: 61, 42 and 13 links to 50, 14 and 8, files 168 to 117) and the close. Its done-when: every step's checks passed (its evidence log at `b3204331`); `just check` → 3,703 tests passed, 0 failed; `just script-test`, `just ctx`, `just repo-check`, `just deps` and `git diff --check` → exit 0; citations of the deleted plan → none; phase 7's status set in progress; the demo: `just status` → six sections, `phase-7.md: in progress · steps: 17 · evidence rows: 3 · last: step 3 (7.4), passed`; `just release rehearsal` → exit 0 and the owner's block, a rehearsal tag and its push; a fresh agent with the lean `tungsten-next` on the tree after the deletion → 7.5's plan as the `step` prompt (phase 7's Step 4, the redo's 7.5 row and §7.1) | `perf-runs/20261029-overhead-h1/`, `perf-runs/20261029-h2-skills/`, `perf-runs/20261029-h2-memory/` |
| 4 (7.5) | 2026-10-30 | passed | `analysis-pass`, commits `3b01712d` (the plan), `203fad2d` (the register framed), `12ea9e55` (the reading and merge), `6c264e33` (the verified register), `088c903a` (the triage) and the close; its done-when: step 1 `shares.py --check` → 157 files, 86,471 lines, `unassigned 0, doubled 0`, and `check.py` on its fixture → exit 1 on the overlap and the Lines mismatch; step 2, nine readers (5.00M subagent tokens) → 253 rows and 86 entries, `shares.py --ledger` → `unread 0`, `check.py --sources` → 195 merged rows (15,622 lines) and 32 entries, `unaccounted 0, doubled 0`; step 3, thirteen verifiers (4.58M tokens) → rows 84 confirmed, 109 corrected, 2 dropped, entries 18 confirmed and 14 corrected, seeds 2 confirmed (K8, known issue 31) and 1 dropped (K25), bugs 8 confirmed and 1 dropped, `missing 0`, and `check.py --register` → 193 S-rows (15,810 lines) and 32 §4 entries (9,919 lines), `errors 0`; step 4, the S-IDs here and in the register `cmp`-equal, 193 each; `triage_check.py` → `errors 0`; the Later and 9.2 loops print nothing; K8 `landed 4d51a071`; the S-rows per disposition 7.8 10, 7.9 29, 7.10 42 (a 1, b 13, c 28), 7.17 33, 7.20 1, 7.18 2, 7.11 17, 7.12 5, 7.13 38, 7.14 6, 7.15 2, 8.2 1, 8.3 5, `Later` 2, `9.2` 0; every step's no-code check → exit 0. The close: seven bugs filed in known issues (the eighth, b6, was already Later's route-test line), known issue 31 removed (fixed by `1803bb7d`), F3 retaken; no 7.5 row open; `just check` → exit 0, 3,703 tests passed, 0 failed; `just ctx`, `just repo-check` and `git diff --check` → exit 0; citations of the plan → none; the demo: `just status` → `docs/plans/phase-7.md: in progress · steps: 17 · evidence rows: 4 · last: step 4 (7.5), passed`, Now without `analysis-pass` | `perf-runs/20261030-analysis-pass/` (`prompts/`, `readers/`, `merged.md`, `verify/`, the scripts, `overhead/`, `close/`) |
| 5 (7.8), part 1 | 2026-10-30 | passed | `runtime-terrain`, commits `9180036e` (the plan), `2f325e87` (tile and collision-layer properties), `df0a1141` (tiles in their layer's physics layers), `630cb828` (edits in place, the journal, cell health), `beb08bd2` (the auto-tiler), `a9ca7b39` (region queries and sensors on the grids), `2361fa67` (the tile cache's cold state boxed, so `PhysicsBuffers` keeps its size in the guard pins), `efa93284` and `f32495aa` (step 6 stopped and revised: one contact a run softened 01's pit walls, centres past a wall face in 56 of 202 pit cases against 8 with cells, and no stiffened run contact restored it), `d2295cdb` (the outline traced per 16 × 16 chunk, casts on its runs, 01's index 10,093 cells to 298 runs), `920e2944` (one-way edges), `f1bf04e9` (the outline in the physics overlay), `beb7b081` (the `terrain` row and its guard), `c31c8bfc` (`D-228`) and the close; its done-when: every step's checks passed (its evidence log at `c31c8bfc`); `just check` → exit 0, 3,744 tests passed in 30 suites, 0 failed; `just physics-release` (determinism and containment hashes unchanged), `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx` and `just repo-check` and `git diff --check` → exit 0; `just smoke` → every section passed (benchmarks 13/13); `just perf counters` → gpu, gpu-throughput, template, e01-area-clearing ok; the demo: from `examples/01_platformer`, `tungsten run --script` (`start_run`, the player walking right from x 377, hopping the clearing's crates to x 949 and walking back to x 430 by frame 300) → exit 0, 4 asserts passed: `area` "Lantern clearing", the player standing on the ground (y 2210.99), x below 500; `tungsten render --after start_run --at 640,2240 --view 288 --overlay physics --frame 120` → exit 0, the floor's top face one outline line and none inside, a wider capture outlining the raised ledge and the floating platforms on their exposed faces; `just perf run terrain --repeat 1` → valid on the clean tree, digest `8675de67eeab6c93` as at step 9, `bench.edits constant` (32..32) and `bench.traced <= 256` (51..77) ok, `physics_step` p50 5.33 ms and p95 5.84 ms, `update` p95 6.16 ms; the plan's citation check run with `check-repo.py`'s exemption of `CHANGELOG.md`, whose released 0.113 notes name the plan; citations of the plan → none; P7-1 landed, the rest of Step 5's rows open for the physics plan | `perf-runs/20261030-runtime-terrain/` (the stop, the options measured, `close/`) |
| 5 (7.8), part 2 | 2026-10-31 | passed | `physics-on-terrain`, commits `79c0f268` (body seams), `ddc3e3e4` (physics layer names and materials), `aae37231` (friction), `52202d87` (circles that roll), `992f75d2` (contact events), `e6c9bbe0` (sensor exits and swept sensors), `3b9b2762` (step 7 stopped: its push record counted the soft solver's push-out), `5656e347` (casts that pass refused hits), `a6c4251a` (step 9 stopped: the resting index moved `terrain`'s pin), `e9bba6e8` (slope tiles for bodies), `5ae9ab86` (slope tiles in queries and the overlay), `2e8108cb` (step 12 stopped: the slope casts let a box at an exact contact pass a masked vertex side), `be8ef6c6` (step 13 behind steps 7 and 12), `8d36de70` (the `friction` and `slopes` rows), `d0b0d477` (`D-229`) and the close; its done-when: every landed step's checks passed (its evidence log at `d0b0d477`); `just check` → exit 0, 3,793 tests passed in 30 suites, 0 failed; `just physics-release` (16 passed, the determinism and containment hashes unchanged), `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → every section passed (benchmarks 17/17, `piles` at its four presets); `just perf counters` → gpu, gpu-throughput, template, e01-area-clearing ok; the demo: from `examples/01_platformer`, `tungsten run --script` on part 1's script → exit 0, 300 frames, 4 asserts passed: `area` "Lantern clearing", the player standing on the ground (y 2210.99), x 430 below 500, its path as part 1's (x 377 at frame 30, 949 over the crates at 120, 430 by 270); `just perf run friction --repeat 1` → valid, digest `d95faee6785e4893` as at step 14, `bench.sleeping <= 0` and `bench.escaped <= 0` ok, `physics_step` p50 7.08 ms and p95 7.88 ms, `update` p95 7.94 ms; `just perf run slopes --repeat 1` → valid, digest `ee588679485534d1`, `bench.recycled >= 60` (299..483) and `bench.inside <= 0` ok, 2.30, 2.40 and 2.46 ms; Step 5's rows: P7-3, S-16, S-26, S-60, S-65, S-77, S-112, S-172 and S-175 landed, P7-2's impulses, K7, K23, K30, S-99 and S-132 moved to 7.10 and K14 to 7.14 (§6), none open; its follow-ups in known issues and Later; the plan deleted with its citations, out of Now | `perf-runs/20261031-physics-on-terrain/` (the stopped steps' patches and probes, `close/`), `perf-runs/20261031T082623Z-friction`, `perf-runs/20261031T082626Z-slopes` |
| 7 (7.9), part 1 | 2026-10-31 | passed | `saves-and-seams`, commits `a9cb10c3` (save identities and the registry), `b3b3b985` (capture and restore; a save spawn's prefab ID rides the command stream, a `CommandBuffer` field having moved 02's `ecs` pin), `ce42a2ae` (areas, tiles, kept areas, pending references), `8278ab06` (the free spot), `e271a5ee` (`migrate_step`, `save_stamped`), `93540b4a` (01's arms a schema at a time), `8db2c754` (the kit's 28 saved components, codecs and request queues), `93197b6c` (orders by save ID), `ef935f73` (the template on one-call saves), `c8232257` (the `saves` row), `1ec87ae6` (`D-230`) and the close; its done-when: every step's checks passed; `just check`, `just physics-release` (hashes unchanged), `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; the kit's pin, 01's `pit` and 02's ten pins unchanged, 01's tests untouched; `just smoke` → every section passed (benchmarks 19/19); `just perf counters` → gpu, gpu-throughput, template, e01-area-clearing ok; the demos: from `templates/basic`, `tungsten run --script` (`start_run`, x 624 at frame 9, `place_player` to x 900 at 30, `reload_run` at 90, x 900 at 89 and 150) → exit 0, 4 asserts passed, where a held `move_right` moves the player headless but not in the window, before this plan too (known issues), so the demo places it; from `examples/01_platformer`, `tungsten run --script` on 7.8's script → exit 0, 4 asserts passed, its path as 7.8's (x 377 at 30, 949 at 120, 430 by 270); `just perf run saves --repeat 1` → valid (diagnostic: `--allow-background` beside the owner's NoMachine session), digest `0ac30092c6b48f1c`, 12,101 records, `bytes` ≤ 3,080,192 ok, `save_snapshot` p50 26.2 ms, p95 29.2 ms; P7-8, S-2, S-3, S-43, S-59 (the kit's part; its rest in Later), S-101 and S-125 (the engine's part) landed, none open; the plan deleted with its citations, out of Now; Step 7's other rows and its part 2 are the second plan's | `perf-runs/20261031T215321Z-saves`, `perf-runs/20261031T220639Z-saves`, `perf-runs/20261031T220627Z-counters` |
| 7 (7.9), part 2 | 2026-11-01 | passed | `boot-and-seams`, commits `aa7c6689` (the startup line, the manifest from `Config`), `ce71db3c` (sprite sheets and anchors, headless sprites), `a6c3d80a` (boot loading), `e1af0b5b` (sound reload), `01826112` (pan, ducks, plays by ID), `9c144ab9` (step 6 stopped: the kit's loops and music per state needed files outside its list), `48000b85` (animation speed, one-shot ends, frame sounds), `e48bc6c1` (particles), `0bdb8e33` (flips, lit flashes, unlit sprites, light tweens, the `seams` fixture), `b686d9fd` (tile layers among sprites, slots, backdrops), `6a9e7881` (fragment-only materials, `engine_glow`, `engine_white`, draw helpers), `81f4df88` and `c986cb2f` (steps 12 and 13 stopped: text outlines and sprite variants needed literals in files outside their lists), `0aa735d2` (the `boot` row and `guards_boot`), `6cc1055f` (`D-231`) and the close; its done-when: `just check` → exit 0; `just physics-release` 16 passed; `just api-check`, `just docs-ratchet` (no rise), `just perf-test` (45 OK), `just ctx`, `just repo-check` and `git diff --check` (the tree and `aa7c6689~1..HEAD`) → exit 0; `just smoke` passed every section (benchmarks 21/21, seams 1/1); `just perf counters` ok on its four rows; `just visual` passed its six suites, the goldens included; 01's demo from its folder (`tungsten run --script`, 7.8's script with the `startup` probe) → 300 frames, 5 asserts passed, `first_frame` 110.71 ms, `loaded` 728.36 ms, 9 boot frames (debug); the template's → 150 frames, 5 asserts passed, `first_frame` 68.83 ms, `loaded` 116.07 ms, 1 boot frame; `just perf run boot --repeat 1` → valid, digest `0d32539a6b42bedd` as at step 14; the drawing sheet copied; the 30 rows: 26 landed, P7-13's kit part to 7.10, S-63 to 7.11, P7-14 to 7.17, S-58 to 7.12; no pin moved but the default extract's digest, by design (its lit batches keep their blocks), the save snapshot and boot rows both landed | `perf-runs/20261101-boot-and-seams/` (`seams-sheet.png`, `demos/`, `startup/`, `step3/`, `step6-stopped/`), `perf-runs/20261101T045340Z-boot` |
| 6 (7.17) | 2026-11-02 | passed | `ui-pass`, commits `8853849f` (the plan), `24d85c9b` (keys, groups, styles in place), `800c5864` (named styles, sizes, sounds and token colours in theme files), `f7f3224f` (outlines and shadows), `fce27866` (shrinking labels, the commit width), `4efcf875` (`ui::widgets`), `7defa02d` (commits, timed captures, back keys, menu sounds, routed reads, short labels), `067b1bf6` (game-clock tracks, snapping, the pointer sprite, world anchors), `840c1ac9` (the reset transition, held clocks, owned resources), `a2bf7baf` (`tungsten::screens`), `7f11aef8` (`tungsten.json`'s `ui`), `1f1b9b26` (`UiHarness`'s queries, the idle guard), `f1998611` (the shell on core's widgets and keys, 2,786 → 2,647 lines), `0c454a7f` (step 13 stopped: dropping the shell's per-frame toast tick moved 01's `pit` pin, 72,637 → 72,577 allocations), `7b3000c6` (both games' and the fixtures' helpers), `72526117` (step 15 skipped with 13), `a60a813e` (`D-235`) and the close; its done-when: every landed step's checks passed (its evidence log at `a60a813e`); `just check` → exit 0, 4,098 tests passed in 30 suites, 0 failed; `just physics-release` (16 passed, the hashes unchanged), `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → every section passed (benchmarks 21/21); `just perf counters` → gpu, gpu-throughput, template, e01-area-clearing ok; `just visual` → exit 0, 28 passed, `title.png`, `ui-visual.png` and `ui-labels.png` unchanged; the demos, the `ui` probe and `render --screen` not landing with step 15: from `templates/basic`, `tungsten run --script` (the title, then Settings by `ui_down` twice and `ui_accept`, the `states` probe `["title", "settings"]`, `ui_cancel` back to `["title"]`) → exit 0, 150 frames, 2 captures, 3 asserts passed; from `examples/01_platformer`, `start_run`, `state_pause` to `["gameplay", "pause"]` and back → exit 0, 240 frames, 1 capture, 3 asserts passed; their captures the sheet; the shell's `tokei` 2,647 against `D-199`'s 2,900; Step 6's rows: P7-14, S-9, S-42, S-45, S-64, S-67, S-82, S-91, S-93, S-94, S-102, S-104, S-108, S-114, S-116, S-127, S-152, S-153, S-155, S-171 and S-183 landed, S-28, S-33, S-62, S-81, S-109, S-123, S-133, S-139, S-157, S-163 and S-187 moved to 7.11 with the shell's port, S-162 to 8.1 and S-149 to 7.11, none open; its follow-ups in known issues and Later; the plan deleted with its citations, out of Now | `perf-runs/20261102-ui-pass/` (the demos' scripts, reports and captures), `perf-runs/20261102T085517Z-counters` |
| 8 (7.20) | 2026-11-02 | passed | `screens-as-data`, commits `ce4e8c69` (the plan), `fd01ef9a` (screen files from `ui_screens`), `a1fdb0eb` (building with bindings and mounts), `52c00f9a` (data screens and their actions), `7c06e7b0` (the check, the schemas, `describe` at version 2), `d0c33eeb` (hot reload and the reload guard), `f8a6cbd6` (`render --screen`, the `screen` step), `d67cd8c7` (pause and game over as files), `a49a7a48` (settings as a file, S-72), `6c0fc0b1` (the title as a file), `37a8c928` (`D-236`) and the close; its done-when: every step's checks passed (its evidence log at `37a8c928`), the converted screens' captures `cmp`-equal at each step in both games, 16 shots a game; `just check` → exit 0, 4,168 passed; `just physics-release` (16), `just api-check`, `just docs-ratchet` (no rise), `just perf-test` (45), `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → passed; `just perf counters` → ok; `just visual` → exit 0 (28, `title.png` unchanged); `just perf run boot --repeat 1` → valid, digest `0d32539a6b42bedd`, step 1's (functional only); `just template-check` → passed; the demos: the template's script (Settings from the title, `states` ["title", "settings"], back, a run, pause, ["gameplay", "pause"]) → 3 asserts passed, 01's run and pause → passed; `tungsten render --screen pause` and `--screen settings` from `templates/basic` → written, settings on its Display tab alone; the shell 2,647 → 2,462 lines, `src/` 3,198 → 3,013; S-72 landed; no 7.20 row open; the plan's citations removed | `perf-runs/20261102-screens-as-data/` (`base7` to `after9`, `close/`), `perf-runs/20261102T162930Z-boot`, `perf-runs/20261102T181929Z-boot` |
| 9 (7.10), part a | 2026-11-03 | passed | `stats-and-hits`, commits `481fb635` (the plan), `502b1be1` (scoped lines, context reads, world sources), `921cae44` (a hit's context and side, `damage_taken`, resist floors), `a8e54afa` (crit from the source's stats), `a869a70c` (01's crit, blast share and difficulty on the kit, schema 24), `2809e88d` (skill sheets, the `Delivery` snapshot), `5b34dd38` (conversion and extra with history), `e3b465d7` (why-records), `04506c39` (the type label's merge, ticks and crit), `ed5ceca5` (`D-238`) and the close; its done-when: every step's checks passed (its evidence log at `db812927`); §9.1's hit 32.07 in a kit test (60.94 on a crit, 27.26 chained, 2.89 more with the ring); `just check` → exit 0, 4,293 passed; `just physics-release` (16, the hashes unchanged), `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → passed; `just perf counters` → ok; `just perf run saves --repeat 1 --allow-background` (NoMachine's encoder a note) → valid, digest `0ac30092c6b48f1c`, the run start's (functional only); the demos: the template's 7.9 round trip (4 asserts) and 01's walk at Hard with max health 100 (5 asserts) → exit 0. Rows: 8 landed (PA-1, PA-2, PA-5, PA-6, PA-7, PA-10, PA-11, P7-7), S-12 moved to 7.13 | `perf-runs/20261103-stats-and-hits/demos/` |
| 9 (7.10), part m | 2026-11-03 | passed, steps 7 and 8 stopped | `pushes-and-riders`, commits `9674cb4b` (the plan), `7dbe7ee9` (slope casts at a masked vertex), `2acbebad` (impulses, the push record), `09ef56a5` (the controller on slopes, its drop and filter), `bd6145e7` (pushed and riding), `8db34c19` (push-out), `065e1958` (movers, knockback, steering), `1056a4eb` and `fd798fc9` (steps 7 and 8 stopped at umbrella tests outside their lists), `c0b69e14` (`D-239`) and the close; its evidence log at `01f97948`; a chain's push 140 and 110 px/s against 400; `just check` → exit 0, 4,330 passed; `just physics-release` (16, the hashes unchanged) and the other checks → exit 0, the kit's pin and every guard pin unchanged; the demos: the template's 7.9 round trip (4 asserts) and 01's 7.8 walk (x 377, 949, 430) → exit 0. Rows: 5 landed (P7-2's physics part, K7, K20, K23, K30), 6 moved to 7.13 (S-92, S-99, S-132 and S-134 with their kit parts landed; K19 and P7-2's kit part) | `perf-runs/20261103-pushes-and-riders/` |
| 9 (7.10), part b | 2026-11-03 | landed in part, steps 3 to 8 stopped | `statuses-and-chemistry`, commits `474e536d` (the plan), `95eeb8e1` (the light at a point), `25e56d79` (statuses: intensity, decay, bands, fuel, a status effect from the user, instance lines), `fa9f24cd` (step 3 stopped: the `interactions` kind moves `kit_recipes/coverage.rs`'s kinds count, a file outside its list, and steps 4 to 8 stood on it), `0bd05844` (`D-240`) and the close; its evidence log at `7c2bc549`; `just check` → exit 0; `just physics-release` (16, the hashes unchanged) and the other checks → exit 0, the kit's pin and every guard pin unchanged; the demos: the template's 7.9 round trip (4 asserts) and 01's 7.8 walk (x 377, 949, 430) → exit 0. Rows: 1 landed (P7-5); P7-4's statuses and PA-8's status effect and lines landed, those rows and S-4 open at 7.10 for 7.10b's chemistry | `perf-runs/20261103-statuses-and-chemistry/` (the demos, step 3's patch) |
| 9 (7.10), part b, chemistry | 2026-11-03 | passed | `chemistry`, commits `acf3bca7` (the plan), `4e90d73e` (interactions, the queue and outputs, rates from the hit), `7682cdf4` (spread, impacts and light), `2bbf36f4` (statuses on tiles, `set_tile`), `d4adf9fd` (`become` under an identity), `8f202a8c` (the blast), `71fe6b3a` (the `spread` row), `a8208a36` (`D-241`) and the close; its evidence log at `2ef4c320`; the flood at 12,000 every receiver burnt by run 1,890, nothing dropped; `just check` → exit 0, 4,396 passed; `just physics-release` (16, the hashes unchanged) and the other checks → exit 0, the kit's pin and every guard pin unchanged; the demos: the template's 7.9 round trip (4 asserts) and 01's 7.8 walk (x 377, 949, 430) → exit 0. Rows: 1 landed (PA-8), 2 moved to 7.13 with their kit parts landed (P7-4, S-4) | `perf-runs/20261103T221523Z-spread/`, `perf-runs/20261103-chemistry/demos/` |
| 9 (7.10), part p | 2026-11-04 | landed in part, step 2 stopped | `looks-and-cues`, commits `47d83743` (the plan), `59fdaff0` (cue rule filters and limits), `42eb0a6f` (step 2 stopped: 01 registers its own `air_jumped` game cue event, which a kit event cannot take while 01 stays unchanged; its work in `perf-runs/20261104-looks-and-cues/`), `e3d0131f` (a following, panning listener, loops, a shake's falloff), `4e999198` (named world cues), `f53aa2ba` (the flash's interval, a shield's take), `42118b13` (named sprite points), `9a63b31f` (looks: tint, overlay, glow), `5bddc01e` (looks: particles, a light, a sound loop), `25931a20` (`D-242`) and the close; its evidence log at `f0f05877`; `just check` → exit 0, 4,450 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just perf counters` and the other checks → exit 0, the kit's pin, every counter guard pin, `DamageRequest` 152 and `Occurrence` 64 held; 01 unchanged; the template's walk and 01's 7.8 walk passed (x 377, 949, 430) | `ROADMAP.md`, this file's §2.2, `docs/known-issues.md`, `D-242` |
| 9 (7.10), part p, cue events | 2026-11-04 | passed | `cue-events`, 7.10p's Step 2 re-planned, commits `f34e0133` (the plan), `ee6b5224` (01's own event renamed `double_jumped`), `92a18364` (named game events; the controller's `jumped`, `air_jumped`, `jump_refused` and `landed`, sent only when named, a landing by the speed the floor stopped), `f4d0f8b6` (`D-243`) and the close; its evidence log at `4616f05a`; a 1.5 px drop lands at dt 1/30 (120 px/s) and 1/60 (60 px/s), a spawn standing none; `just check` → exit 0, 4,462 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just perf counters` and the other checks → exit 0, the kit's pin, every counter guard pin and `PlatformerController` 224 held; the template's walk and 01's 7.8 walk passed (x 377, 949, 430); S-39 and S-48 moved to 7.13 | `ROADMAP.md`, this file's §2.2, `perf-runs/20261104-cue-events/demos/`, `D-243` |
| 9 (7.10), part c | 2026-11-04 | landed in part, step 5 stopped | `abilities-and-genres`, commits `490bc67e` (the plan), `a17375bd` (the five genre tags, true claims and `arpg`'s first recipes; the kit's pin moved to `0xa2de_8a8e_2368_07d3`), `c56d2ebb` (a held trigger, a movement direction and the refill countdown), `a0e60bd0` (cue events for a refused, ready, held and released ability, sent only when named), `89a35f6b` (spawns by their centre, their motion kept and a lob), `d6fb6d03` (step 5 stopped: `crates/tungsten-kit/src/tests/inspect.rs`, outside its list, pins 45 described components, which a described `cap` makes 46; its work in `perf-runs/20261104-abilities-and-genres/`), `ee9b32ec` (the cone), `82b79d79` (conduction), `afe4365d` (a projectile's end blast), `7945edd1` (`D-244`) and the close; its evidence log at `f193c6d0`; `just check` → exit 0, 4,507 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just perf counters` and the other checks → exit 0, every counter guard pin, `DamageRequest` 152, `Occurrence` 64 and `Payload` 32 held, the kit's pin moved once; 01 unchanged and the template but its `AGENTS.md`; the template's walk and 01's 7.8 walk passed (x 377, 949, 430); P7-6 and P7-18 landed; S-5, S-15, S-53, S-73, S-88, S-110, S-130, S-135 and S-186 moved to 7.13; S-17 and S-18 open at 7.10 for a re-plan, next | `ROADMAP.md`, this file's §2.2, `perf-runs/20261104-abilities-and-genres/demos/`, `D-244` |
| 9 (7.10), part c, place and caps | 2026-11-04 | passed | `place-and-caps`, 7.10c's Step 5 re-planned, commits `2c7bedd7` (the plan), `2b4b3eaf` (caps on a group's live instances: every kit spawn route admitted, `culled` and the cue late pass), `a1606731` (the `place` payload), `dadf48b8` (`D-245`) and the close; its evidence log at `96efb3eb`; `just check` → exit 0, 4,528 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just perf counters` and the other checks → exit 0, the kit's pin, every counter guard pin, `Payload` 32, `Occurrence` 64 and `DamageRequest` 152 held; 01 unchanged and the template but its `AGENTS.md`; the template's walk and 01's 7.8 walk passed (x 377, 949, 430); S-17 and S-18 moved to 7.13 | `ROADMAP.md`, this file's §2.2, `perf-runs/20261104-place-and-caps/demos/`, `D-245` |
| 9 (7.10), part g | 2026-11-05 | landed in part, step 3 stopped | `clips-facing-and-lifecycle`, commits `78b039a0` (the plan), `2357a1f4` (one-shot clips, clip rules, aim poses, an ability's look), `ec9dbcb5` (facing a point, a turned mirror, hit shapes by facing, `KitDebug`'s key), `464cbf73` (step 3 stopped: the looks' and bars' discovery queries moved 01's pit pin, allocations 59,977 to 59,981; its work in `perf-runs/20261105-clips-facing-and-lifecycle/`), `8aa77f63` (a death policy, on switches, a lethal kill, a blink), `332c909c` (spawners that wait on their spawns, world bounds), `148d2688` (timed machines, waiting paths), `f7ae8384` (the carry), `1ba767dd` (`D-246`) and the close; its evidence log at `d0d1a79a`; `just check` → exit 0, 4,597 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just perf counters` and the other checks → exit 0, the kit's pin, every counter guard pin, `Payload` 32, `Occurrence` 64 and `DamageRequest` 152 held; 01 unchanged and the template but its `AGENTS.md`; the template's walk and 01's 7.8 walk passed (x 377, 949, 430); P7-19, P7-20, K3 and K22 landed; twelve S-rows moved to 7.13 and S-86 to 7.11; S-10 and S-52 open at 7.10 for a re-plan, next | `ROADMAP.md`, this file's §2.2, `perf-runs/20261105-clips-facing-and-lifecycle/demos/`, `D-246` |
| 9 (7.10), part g, looks and bars | 2026-11-06 | passed; Step 9 closed | `looks-and-bars`, 7.10g's Step 3 re-planned, commits `249b8b41` (the plan), `be5a0bf2` (core's presence read, `World::has_any`), `3352bda1` (looks by tag and the health bar, their discovery gated by it), `5c2cb403` (`D-247`) and the close; its evidence log at `e558c9fc`; `just check` → exit 0, 4,615 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just perf counters` and the other checks → exit 0, the kit's pin, every counter guard pin (01's pit at 59,977 allocations), `Payload` 32, `Occurrence` 64 and `DamageRequest` 152 held; 01 unchanged and the template but its `AGENTS.md`; the template's walk and 01's 7.8 walk passed (x 377, 949, 430); S-10 and S-52 moved to 7.13. Step 9's done-when: no 7.10 row open; §9.1's volley at 32.07 (60.94 on a crit) and the spread flood at 12,000 receivers pass, `1 passed` each; 01's crit, blast share and difficulty as data in `just check` | `ROADMAP.md`, this file's §2.2 and §2.5, `perf-runs/20261106-looks-and-bars/demos/`, `D-247` |
| 10 (7.18) | 2026-11-07 | passed | `camera-rigs`, commits `213a5d16` (the plan), `84092017` (rigs, blends and the camera's output alone), `d43e3fe3` (rigs as data), `4bc9e372` (camera zones and the `camera` cue action), `4727981d` (the debug shapes and the `rigs` probe), `793963fd` (Screen shake as the camera's scale), `99a514a6` (the template's camera on its player's prefab), `51d3e9a0` (01's level and title cameras as data), `cca9838f` (`D-249`) and the close; its evidence log at `8e8f991f`; a switch never jumps at dt 1/30, 1/60 and 1/144 (`a_switch_never_jumps_at_30_60_or_144_hz`, and the interior's recipe going in and out, its cut twin past the bound); `just check` → exit 0, 4,660 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just visual`, `just perf counters`, `just template-check` and `just level-check` → exit 0, the kit's pin, every counter guard pin (01's pit at 59,977 allocations and 68,096,156 bytes), `Payload` 32, `Occurrence` 64 and `DamageRequest` 152 held, the schedule snapshots unchanged; both games' demo `camera` probes byte for byte the baseline's, and the `rigs` probe names the player's rig the winner in a run and 01's title rig on its title; S-75 and S-146 landed | `ROADMAP.md`, this file's §2.2, `perf-runs/20261107-camera-rigs/`, `D-249` |
| 11 (7.11) | 2026-11-08 | landed in part, steps 6 and 7 stopped | `template-updates`, commits `295461eb` (the plan), `2320b8f0` (`Settings`' game section, Bloom, the focus mute, display keys), `67dd4a00` (action labels, shipped bindings, device cells), `c5ed7c39` (taps, the audio log, typed actions, the kit's `place` and `tagged`), `57575da2` (music per state), `1c544efc` (the hit flash's material), `b015c270` (steps 6 and 7 stopped: on `tungsten::screens` six of 01's tests failed outside the list; the stopped tree's patch in `perf-runs/20261107-template-updates/`), `a097f11b` (keybindings on the map, `start_run` the shell's), `67d483e8` (the shell's songs), `52abefe0` (the sample on spawners, a flag and the kit's hooks), `8c753dd2` (`tools/synth.py`), `8c13bec1` (the boot splash's hold), `9fe2a3a3` (`D-250`), `2665445d` (the logo) and the close; its evidence log at `25b0c4c4`; `just check` → exit 0, 4,715 passed; `just physics-release` (16, the hashes unchanged), `just smoke`, `just visual`, `just perf counters` and `just template-check` → exit 0, the kit's pin, every counter guard pin (01's pit at 59,977 allocations and 68,096,156 bytes) and the schedule snapshots held; the template's demo (4 asserts, the player at [900, 400] across `reload_run`) and 01's 7.8 walk (x 377, 949, 430) passed; the splash held its 1.5 s (`boot_frames=44`, loaded at 1.64 s); 17 rows landed, 15 moved to 7.13 | `ROADMAP.md`, this file's §2.2, `perf-runs/20261108-template-logo/` (the logo's sheet, the owner's review awaited), `perf-runs/20261108-template-updates-close/`, `D-250` |
| 18 (7.21) | 2026-11-08 | landed in part, steps 8, 9, 11, 12, 13, 15, 22 and 24 stopped | `core-hardening`, commits `dba6f46e` (the plan), `e5756114` and `78b5f6bb` (the physics oracle, the ECS and save oracles, their default runs sharded), then each finding's repro test before its fix: `1427b84f` (oversize pair boxes past 64 cells, `D-251`), `4628e812` (the safety net leaves a hit within the slop to the contacts, `D-252`), `bcc71605` (the free spot's resting band, `D-253`), `e1874a68` (a restore that names nothing keeps no save state), `bafd09ac` (a named entity's references loaded onto its saved form), `faacb2d4` (kinematic supports by kind, layers and shape, `D-254`), `17b588f0` (an open slope corner rounds a circle's contact, `D-255`), `e86ccfd1` (a mass at most 1e12, `D-256`), `f7d4c75e` (a split key loads in two calls, a contract, `D-257`), `ec0fe3c0` (a static body's velocity moves nothing, `D-258`), `0cb9425e` (static joins by a sorted sweep, queries keeping every entity face, `D-259`), `78a6655f` (maps edge to edge, `D-260`), `513e78ff` (per-proxy gravity budgets, `D-261`), `67cc4a8a` (a body gravity pulls with nothing under it stays awake, `D-262`), `6a58b406` (the spin wrap, `D-263`); stopped: `bedc689c` (step 8: the write wake woke a box the step had left deep in a solid cell beside a slope, seed 1844), `b44f4171` (step 9: 24 of the island-tag cases, a woken holder moving off its sleeper), `2bc3d479` (step 11: two seeds, a flat box's centre under the cell's top), `beea7965` (step 12: the default seeds just past the threshold), `b23f71f7` (step 13: the crush's sink is the contacts' bias under the arrival threshold), `68aee17f` (step 15: `float_roundtrip` moved three loaders' error contracts), `76998854` (step 22: the case gone on step 21's tree), `0aa3a5fe` (step 24: the thin box never stands on the cell's top) and the close; its evidence log at `CLOSE_LOG`; `just check` → exit 0, 4,770 passed; `just physics-release` (16, the hashes unchanged); the long modes (physics seeds 0..20000, saves, ECS) → exit 0; `just smoke` → exit 0 at every landed step; 01's 7.8 walk (x 377, 949, 430, every probe as at 7.11's close) passed; moved, as listed: 01's pit to 59,980 allocations and 69,236,700 bytes, 02's `physics`, `physics-sparse`, `slopes` and `terrain` guards; 15 findings landed, 8 stopped with their rows and next work in known issues | `ROADMAP.md`, `docs/known-issues.md`, `perf-runs/20261107-core-hardening/` (local: each finding's patches and each stop's evidence), `perf-runs/20261108-core-hardening-close/`, `D-251`–`D-263` |

## Follow-ups

- **F1.** The paper designs' step 6 and its done-when ("7.7 has assigned every row of §4") are met by §2; its step 6 also says "7.13 and 8.5 take the weekly tasks", where the redo's §8 now says 7.13 and 8.6. Mark it at the next edit of that file, or at step 17.
- **F2.** Done at 7.4: tungsten-next's `step` prompt hands a phase plan's next step its own plan, and its `run` prompt says "never a phase plan".
- **F3.** Done at 7.5 (`perf-runs/20261030-analysis-pass/overhead/`, H1's classifier with 7.5's two sessions and its plan and register as its own files): the main sessions at 64.5 % of tool calls and 66.3 % of output tokens (45.4 % and 49.2 % with the plan and the register counted as 7.5's work), 11.5 % and 13.8 % with the subagents' 3,986 calls. A documents pass whose product is records, so read beside H2's retake of H1 with that caveat: the example group at 20.7 % of tool calls and 16.5 % of output tokens (baseline 22.1 % and 20.0 %), Phase 7's sessions at 30.5 % and 29.9 %, plan sessions 33–72 %.
- **F4.** Done at 7.8's close (`perf-runs/20261031-overhead-7.8/`, F3's classifier on 7.8's seven sessions: two plans, four runs, one an 8-call false start, and a revision; 0.113's release left out): 28.8 % of tool calls and 18.5 % of output tokens touched records, against the example group's 20.7 % and 16.5 % at H2's retake. The runs took 13.0–20.6 % and 8.0–11.9 %; the plan sessions 54–55 % and 38–43 %, mostly their own plan file, 19–22 % of calls without it. With each session's own plan counted as its work, 7.8 took 15.3 % and 10.3 %.
