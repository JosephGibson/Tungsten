# W13 `tungsten-kit` — draft

- **status:** draft
- **goal:** A broad, generic `tungsten-kit` that a game of any 2D genre composes its entities from in data, with Rust only for what is the game's own: the base (foundations, tags, stats, cues, motion, overlap, damage, effects and abilities) before the 1.0 freeze, the leaf items in 1.x.
- **non-goals:** Physics features (friction, layers and masks, sensor exit and stay, approach speed: W6b); per-tile gameplay properties; reworking the examples (W12b); render types in the kit (`D-007`); cataloguing a project's own components; a scripting language; UI widgets, 3D, multiplayer, physics joints; a live agent bridge (a follow-up).
- **files to touch:** Per candidate, the catalogue's header below: core's `ecs/event_queue.rs`, a spawn-inheritance module and `assets/prefab.rs` (W13e); `assets/manifest.rs`, a data-registry module, the umbrella's `asset_loader/`, `docs/assets.md`, `scripts/check-repo.py` (W13f); `crates/tungsten-kit/src/` in domain subfolders, `crates/tungsten-kit/benches/`, the umbrella's `tests/` and recipe fixtures, `testing.rs`, `scripts/public-api.sh` (W13g onward); `templates/basic/`, `docs/getting-started.md`, `docs/LLM_INDEX.md`, `DESIGN.md`; every milestone's `api/`, `DECISIONS.md`, `docs/DECISION_INDEX.md` and `CHANGELOG.md`.
- **ordered steps:** W13e (F1) to W13k (E), §9 below: each a level C milestone with its own plan session, execution session and release, in the register's order; W12b follows W13k.
- **done-when:** §11 below.

The scoping text of the landed candidates, W13a to W13d, stays in [criteria](criteria.md) §8.4; this file holds the kit catalogue, the scoping source of W13e to W13k ([conventions](README.md#conventions)). Revised 2026-10-03 on `db1177c`: the definition gate's tier timing and Q24's answer. Revised 2026-10-05 on `01f9c55`, uncommitted: tiers proposed from the acceptance game's mechanics rows. Revised 2026-10-06, uncommitted: the frame-loop gate confirmed them and answered Q21 (`D-132`). Revised 2026-10-14, uncommitted: W13a and W13b landed in 0.69 (M50, `D-155`): `tungsten-kit` on core alone, re-exported as `tungsten::kit` behind a default feature, with opt-in plugins per item and harness tests in the umbrella; its value types keep private fields behind constructors in place of `#[non_exhaustive]`. Revised again the same day, uncommitted: the spawner landed in 0.71 (M51, `D-157`), under the same rules: `Spawner` and `SpawnerPlugin`, whose `spawner_tick` spawns a prefab at its entity's position once per interval of game time through the command buffer. Revised a third time the same day, uncommitted: W13c and W13d landed in 0.72 (M52, `D-158`): the platformer controller and the top-down mover on kinematic bodies, health and `damage` with a hit flash, trigger zones, a path follower and a per-entity state machine, the body drivers and window counts in `FixedUpdate` before `physics_step`. Example 01's player did not move onto the platformer controller (a route corner only overlap resolution passed), so the platformer controller and the path follower wait for W12b's move for their user. Revised 2026-10-16 on `0732305`, uncommitted: the kit catalogue, drafted on 2026-10-15 and 2026-10-16 and finalized by its review session, folded in as the scoping source of seven level C candidates, W13e to W13k; the plan itself is archived as `docs/plans/archive/kit-catalogue.md` (superseded), with its two critique rounds and evidence log. Revised again the same day, uncommitted: W13e landed in 0.80 (M60, `D-170`): `EventCursor` and `EventRead` in core, spawn inheritance applied at the flush to `CommandBuffer::spawn_prefab_from`, `StatePlugin` registering `SceneEntity`, the kit's spawner spawning from its spawner, and `SoundOnEvent` and `TriggerZone` on cursors.

## Placement

- **Tier:** Must per item, if a user ships it by the freeze. From the acceptance game's mechanics rows, agreed with the owner on 2026-10-05 ([acceptance game](acceptance-game.md#proposed-tiers)): every item has the game as a user except `cursor_to_world`, the platformer controller and the path follower, whose only user is example 01, so those three are Must only if it moves onto the kit by the freeze (W12b). Confirmed at the frame-loop gate on 2026-10-06 (`D-132`; [implementation plan](implementation-plan.md) §11). Physics bodies live on root entities only (Q21): the hierarchy propagates after `physics_sync`, and the game places its orbiting blades each fixed step. From W13e on, recipes in two genres replace a user as the admission rule (§4 below), and the catalogue's base is Must before the freeze.
- **Candidates:** W13a basics and W13b transform hierarchy as one candidate; W13c character controllers and W13d gameplay helpers as another; the spawner after W16b (all Phase 6; implementation plan amendment 3 and §4's cards). Then the kit catalogue's seven, each level C and unpaired: W13e (F1), W13f (F2), W13g (F3), W13h (A), W13i (M), W13j (C) and W13k (E), in the register's order after M59 and before W6b and W6c; W12b follows W13k (§7 and §10 below).
- **Needs first:** W15a (`Schedule` and `Plugin` in core); W3a, for timers; W16a, so that items register as they land; W6a, for W13c and W13d; M59 (0.79) for W13e, then each catalogue candidate the one before it (§7).
- **Feeds:** The template; examples 01, 03 and 04; the acceptance game; the genre fixtures (§4).
- **Owner questions:** Q22. Q21 answered at the frame-loop gate: bodies on roots only (`D-132`). Q24 answered at the definition gate: inside the 1.0 promise (`D-103`).
- **Decisions:** `tungsten-kit`: dependency direction, admission rule, and a row in `AGENTS.md`'s "Where code goes". Its stability is `D-103`'s: inside the promise, with `#[non_exhaustive]` on kit settings structs. The catalogue's expected decisions: §13 below.

## Context digest

- **The kit today** (`D-155`, `D-158`): 13 items in 15 flat modules, 3.4k lines, on core alone. Only `animation` is a scene component, no kit item has inspector rows, and no example or template code uses the kit yet.
- **Owner decisions, 2026-10-15 and 2026-10-16:**
  - Recipes replace "has a user".
  - Bodiless overlap is a kit-owned grid.
  - Features nest as modules; the template stays flat.
  - Scene names are bare and reserved.
  - The base ships before the freeze.
  - A generic `data` manifest section holds shared definitions.
  - `Cues` are the one way to react to events in data.
  - Work starts after M59's release (0.79): each milestone gets a plan session, then an execution session.
- **The 2026-10-16 review** (§1) found that the base needs core work first:
  - events have no exactly-once reader;
  - assets cannot reference each other at load;
  - damage has no source or hook;
  - kit spawns cannot be owned by a state.

  So the program opens with three foundation milestones: F1 (core events and ownership), F2 (core data assets) and F3 (kit foundations).
- **The finalizing review, 2026-10-16 on `0732305` (0.80 tree):** §1 re-verified; seven milestones in place of eight. G dissolved: pickups to C, the animator and depth sort to M, damage numbers as a cue action in A, waves and presets out. Composition replaces three items (`Aura`, `Breakable`, `DropTable`) and two options (homing, return). Cost: seven releases.
- **The survey** (§6) found:
  - one design recurring across engines: tags, ordered stat modifiers, stacking effects and abilities, with cosmetic reactions decoupled (Unreal's Gameplay Ability System);
  - shared definitions by reference or inline in Godot and Unity, per-reader event cursors in Bevy;
  - agent tooling built on running, inspecting and driving a game, and on machine-readable component descriptions.
- **Example 01** reimplements kit-shaped mechanics. Its perf shows what to avoid: per-entity scans, per-step hash grids, 12,000 physics bodies (a 10.3 ms step at 12,000 balls, `D-139`).

## 1. Review findings, 2026-10-16

Each finding names its evidence, re-verified at `0732305` (the 0.79 tree, 2026-10-16), and where the plan answers it.

| # | Weak point | Evidence | Answer |
| --- | --- | --- | --- |
| R1 | **No exactly-once event reader.** A reader must choose `iter` or `iter_current` by its place against the senders and the stage. `SoundOnEvent` works around it with a handled count that holds in `PostUpdate` only. The catalogue adds about ten reactive readers across both stages. | `event_queue.rs:1-5, 78-108`; `sound.rs:135-162`; `w13-kit.md:42`; `D-139` left the cursor open | F1: `EventCursor` in core; `SoundOnEvent` moves onto it |
| R2 | **Assets cannot reference each other.** Prefabs load in sorted ID order, each is checked against the World, and the registry is inserted after the loop. An ability's projectile, a drop or an effect named by ID is unchecked until used. | `asset_loader/prefab.rs:19-31`; `prefab.rs:95-124` | F2: a two-phase load and the data registry (§3) |
| R3 | **Damage has no source, kind or hook.** Resistances, kill credit, friendly fire and damage types have nowhere to go. | `damage(world, target, amount)`, `Damaged { entity, amount, killed }`, `health.rs:172-229` | C: the damage pipeline; `damage` stays the immediate path; `Damaged.killed` makes a `Killed` event unnecessary |
| R4 | **A kit hit shape must not be a `Collider`.** The gather makes every entity with a `Collider` and a `Position` a physics proxy, static without a `RigidBody`. | `gather.rs:59-106` | §2 rule 6; C's `HitShape` |
| R5 | **Motion had no write target.** The sync interpolates any entity with `Transform` and `Position`, and the history copies any `Position` with a `PrevPosition`; only a `Collider` makes a proxy. So bodiless motion by `Position` in `FixedUpdate` is deterministic and smooth at no cost, and motion by `Transform` stutters or is overwritten. | `physics/plugin.rs:60-95`; `gather.rs:59` | §2 rule 6 |
| R6 | **Ordering does not scale.** The schedule has names with before, after and if-present, and no sets; a second add of a name fails `resolve`. | `schedule.rs:143-215, :283, :327, :344-351` | F3: phase anchors (§2 rule 7) |
| R7 | **Kit spawns outlive their state.** `SceneEntity` is the umbrella's, so a kit on core alone cannot copy it. | `state.rs:35`; `known-issues.md:52` | F1: spawn inheritance |
| R8 | **Existing items are not data-first.** Only `animation` is a scene component; `TriggerZone<Z, T>`, `StateMachine<S>` and `SoundOnEvent<E>` cannot be named in data. | `animation.rs:72`; no other kit module calls `register_component` | F3: scene components for the eight data-shaped items; `Zone` |
| R9 | **Recipes need the real frame loop.** Event rotation, step views and held frames are the App's; the kit's doctests run hand-written loops. | `event_queue.rs:1-5`; `D-139`; `testing.rs:55-241` | §4: recipes on the umbrella's `Harness` |
| R10 | **The kit is invisible when debugging.** No kit component has inspector rows. | `inspect.rs:9-40`; `debug_draw.rs:46-85`; no kit module names `InspectRegistry` | F3: inspector rows; `KitDebugPlugin` |
| R11 | **No change detection.** The ECS has `With` and `Without` only. | `query.rs:363-366` | §2 rule 8 |
| R12 | **Stats would not drive existing items.** Speeds, maxima and cooldowns read their own fields. | kit sources | A: well-known stats kit items read when present |

**Second pass.** What a data-first prefab still could not express, and what would let a later change break games silently.

| # | Gap | Answer | Milestone |
| --- | --- | --- | --- |
| P1 | Reactions need Rust: each `SoundOnEvent<E>` pairing is a plugin a game writes. | `Cues` (§3) | A |
| P2 | Nothing says what death does. | `Health`'s death policy: despawn after a delay, cues first, or keep | C |
| P3 | A player needs Rust for input and camera. | `ActionInput`, `CameraTarget` over `CameraMode::Follow` | F3 |
| P4 | Aiming is per game. | `Aim` | M |
| P5 | Randomness would scatter across items. | `KitRng`: one stream seeded from the run seed, drawn in sorted order | A |
| P6 | Healing, crits and regeneration had no path. | Heal requests and a `crit` flag through the pipeline; regeneration as a periodic effect | C, E |
| P7 | **A DTO change breaks every game's data silently**: `just api` snapshots Rust items, not JSON shapes, and the formats sit inside the 1.0 promise (`D-103`). | Field descriptions written to `api/tungsten-kit-components.txt`, compared by `just api-check` | F3 |
| P8 | A reorder of kit systems could change gameplay with no failing test. | A kit determinism hash over the genre recipes, pinned as the physics hashes are | F3 |
| P9 | The acceptance game's load has no row. | C1's kit row, which the implementation plan §5 already lists; the kit adds criterion rows only | C1 |
| P10 | Kit state could not be saved. | Data state derives serde (§2 rule 14) | every milestone |
| P11 | Hit sounds flood the mixer until W6c's voice limit. | `min_interval` on sound cues and `SoundOnEvent` | A |
| P12 | HUD values need code. | 1.x: `BindText` | after the freeze |
| P13 | Damage numbers would be a component per prefab. | The `label` cue action, timed labels pushed into core's `WorldLabels` each frame: one cue rule shows every enemy's damage | A |

**Seams the catalogue stands on**, confirmed at `0732305`: `CameraMode::Follow(Entity)` on core's `CameraController` (`camera.rs:40-44, :82`); `WorldLabels::push` in core (`text.rs:564-583`); `ShakeEvent` and `SquashEvent` in core (`event_queue.rs:170-178`); `register_component_with_assets` checks IDs against the World at load (`component_registry.rs:598-602`) and `MissingAsset` names field, kind and ID (`:464-472`); `cast_ray`, `cast_shape`, `overlap` and `nearest` over the last step (`physics/query.rs:314-430`); `rigid_body` is a scene component (`physics/plugin.rs:43`).

## 2. The contract for every item

1. **Generic.** Names no game type. Filters on `Tags`, or takes the game's types with a tag-filtered data form beside it. Assumes no gravity, no up direction and no player. Units are world pixels, Y down.
2. **Cross-genre.** Names at least two of the four reference genres (platformer, top-down action, shmup, grid puzzle or roguelike) and passes a recipe in each (§4).
3. **Data first.** A plain component with a builder, and a scene registration under a reserved bare name with a serde DTO. Shared definitions are data kinds (§3), named by ID or written inline. Every ID is checked at load. A game composes the item with no Rust.
4. **Events out, reactions as cues.** Each event names its entity and, where it has one, its position, so cues can react to it (§3). Every reader uses `EventCursor`.
5. **Owned.** Whatever it spawns carries what F1's spawn inheritance derives from its source: the umbrella's `SceneEntity`, so a state's exit takes it, and the kit's `Source` (F3), so credit and "on source" reactions follow the chain.
6. **Writes stated.**
   - Motion writes `Position` (bodiless or static) or `Velocity` (kinematic or dynamic), never the `Transform` of an entity with a `Position`.
   - A bodiless entity carries `Position`, `PrevPosition` and `Transform`, and no `Collider`.
   - Hit shapes are the kit's own component.
7. **Phased.** Each system joins one phase anchor. An anchor is a no-op system that a plugin adds when `Schedule::contains` says it is absent, ordered against the engine's names with if-present constraints. Inside a phase, systems run in registration order (`schedule.rs:420`), so an item names another item's system, with an if-present constraint, only where it reads that item's output in the same step: today the path follower runs before the controllers so a rider reads its platform's velocity (`path.rs:220`). Each plan lists its within-phase edges on the item card, and F3's moving-platform recipe registers the plugins in reverse order to prove them. `D-158`'s chain maps onto the anchors and these edges unchanged.

   | Phase | Stage | What runs there |
   | --- | --- | --- |
   | `kit_tick` | `FixedUpdate`, after the history snapshot | timers, cooldowns, effect durations, health windows |
   | `kit_intent` | `FixedUpdate` | input, aim, abilities, AI, state machines |
   | `kit_move` | `FixedUpdate`, before `physics_step` | motion items, controllers, the path follower, magnets |
   | `kit_overlap` | `FixedUpdate`, after `physics_step` | the hit grid, zones, pickups |
   | `kit_resolve` | `FixedUpdate` | damage, heals, effects, stats |
   | `kit_react` | `FixedUpdate`, last | cues, spawns, counters, levels |
   | `kit_present` | `PostUpdate`, after the sync and the late propagation | animators, facing, labels, depth sort |
8. **No change detection.** One-shot work on spawn rides a marker its system removes. A derived value carries a dirty flag.
9. **Cheap.**
   - No per-entity world scan.
   - `query_mut`, never collect plus `get_mut`.
   - Region work goes through the kit grid or the broadphase.
   - Each per-entity per-step system has a criterion row.
10. **Deterministic and pause-safe.** Entity-id order; `KitRng`; the game clock; queued requests sorted before they resolve.
11. **Visible.** Inspector rows; a debug drawing where it has a shape; its events in the event trace.
12. **Named by convention.**

    | Thing | Form |
    | --- | --- |
    | Component | `Projectile` |
    | Scene name | `projectile` |
    | DTO | `ProjectileDef` |
    | Plugin | `ProjectilePlugin` |
    | System constant | `PROJECTILE_TICK` |
    | Data kind | plural snake case, such as `effects` |
    | Event | past tense, such as `ProjectileExpired` |
    | Builder | `with_*` |
13. **Documented and tested.** Rustdoc with a doctest, a unit test, a harness test, the recipes, a field description (§5.3) and a reviewed `just api` diff (`D-107`).
14. **Saveable.** State that is data derives `Serialize` and `Deserialize`, so a game's `SaveData` can hold it.
15. **Data is API.** DTO fields and defaults are in the components snapshot (P7). Renaming one is a break-ledger row.
16. **Composed, not optioned.** A behaviour another item already gives is reached by putting both on the prefab, never by an option that copies it: a homing bolt is `projectile` with `steering`, an aura is `contact_damage` with a shape, a crate is `health` with `killed` cues. A recipe shows each composition the catalogue once named as an item.

**Item card.** Each milestone plan fills one card per item: component and scene name; DTO fields with defaults; data kinds it reads; events in and out; phase; writes; stats read; genres and recipes; inspector rows and debug drawing; perf row.

## 3. The data model

Three layers. Each is checked at load, and none needs game Rust.

1. **Components in prefabs.** What one entity is: `"health": { "max": 5 }`.
2. **Definitions in data kinds.** Shared, named definitions in the manifest's `data` section (F2).
   - Core's `DataRegistry` takes kinds that plugins register by name with a DTO and a load check, as `ComponentRegistry` takes components. Core never learns the kit's types.
   - Entries load before prefabs in the two-phase load, hot-reload for later reads, and fail the load on a bad ID with the file and the field.
   - A field that names a definition is a `DataRef<T>`: an ID (`"burning"`) or the definition written in place. Prototype inline; promote to an ID when a second prefab needs it. Its deserializer keeps `T`'s own field path in an error, so an inline definition's mistake is named as a file's would be, never as "no variant matched".
   - Kit kinds: `tags`, `stats`, `effects`, `abilities`, `loot_tables`, `curves`, `offers`, `cue_rules`. A game registers its own kinds the same way (enemy tables, item lists), which makes this the data tables a game tunes from.

   ```jsonc
   "data": {
     "tags":      { "game": "data/tags.json" },
     "effects":   { "burning": "data/effects/burning.json", "swift": "data/effects/swift.json" },
     "cue_rules": { "combat": "data/cues/combat.json" }
   }
   ```
3. **Cues.** Reactions to events, in three forms read through `EventCursor` in `kit_react`:
   - **Entity cues** (`cues` component): reactions to events about that entity, in its prefab.
   - **Cue rules** (`cue_rules` kind): reactions that belong to no single prefab, chosen by event and tag filter. For example, every enemy kill adds to `kills`:

     ```jsonc
     { "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1 } }] }
     ```
   - **`CuesOnEvent<E>`**: a game's own Rust event mapped to an entity and a position, reusing the same actions. `SoundOnEvent` stays, moved onto `EventCursor`; no `SpawnOnEvent<E>` is added.

   Cue vocabulary, closed at each release; a milestone adds the events and actions of its own items and no others:
   - **Events:** spawned, damaged, healed, killed, expired, hit, picked_up, ability_used, effect_applied, effect_expired, zone_entered, cooldown_elapsed, level_up.
   - **Actions:**
     - gameplay: `prefab` (inheriting), `counter`, `flag`, `despawn` (A); `roll` (C: a `loot_tables` entry, its prefabs spawned at the event's position); `effect` (E: on self or source);
     - presentation: `sound` (with `min_interval`), `shake`, `squash`, `flash`, `label` (A: the event's amount as a timed world label in a named style; `WorldLabels` lasts one frame, `text.rs:558`, so the kit keeps the live labels with their age on the game clock and pushes them each presentation frame, as example 05's damage numbers do).

   **The boundary.**
   - Each action takes at most one tag filter.
   - There are no conditions, variables or sequences. Anything more is Rust in a game's feature module. `label` shows the event's own amount and nothing else.
   - Gameplay actions run in sorted order; presentation actions change no state.
   - Cues run before the death policy's despawn, so `killed` cues always fire.

## 4. Recipes replace "has a user"

A recipe is a set of prefabs and data plus a harness test that spawns it and asserts the behaviour, with no game Rust: no game-authored component, system or plugin. The test itself may drive the kit's Rust API (press an action, call `damage`), as a game's tests would.
- Recipes run on the umbrella's `Harness`, so event rotation, step views and held frames are the App's (R9).
- Each genre has a fixture folder with its own manifest: `platformer/`, `top_down/`, `shmup/`, `grid/`.
- A coverage test builds every kit plugin into a World and fails when a registered scene name or data kind appears in fewer than two genres' fixtures.

Example, `Projectile`:
- top-down: a bolt that pierces two enemies and expires;
- platformer: a fireball that stops at a wall;
- shmup: a bullet despawned outside the view.

A recipe is the evidence a first game would give, before the freeze. A real game can still force a change, which takes a break-ledger row before 1.0 (`w04-api-freeze.md`). The fixtures are the cookbook the template and agents copy from, and the genre starters Q9 names.

Fallback, not adopted: if the freeze review objects to recipe-proven items, a `preview` cargo feature holds them outside the 1.0 promise.

## 5. Developer and agent ergonomics

What makes the engine faster to build with, for a developer reading rustdoc and for an agent writing data and tests.

1. **One way in (F3).**
   - `tungsten::kit::prelude`.
   - `KitPlugins`, the set of every kit plugin, with `without` as `PluginSet` has it (M50's follow-up, `w13-kit.md:40`). No genre-named presets: the genre fixtures (§4) are the starters, and an item or a set names no game type (§2 rule 1).
   - The naming convention (§2 rule 12).
   - The kit's root docs as the catalogue, grouped by domain, each item linked to its recipe.
2. **Errors that say what to do (every milestone).** A data error names the file, the field path and the expected values, and suggests the nearest name for an unknown tag, stat, prefab, data ID or component, by an edit distance written in core (no dependency). `serde_path_to_error` gives the path (`D-153`).
3. **Field descriptions (F3, then each milestone).** Each scene component and data kind registers a description: summary, fields with types, defaults and enum variants, events, genres. A test holds the two together: it deserializes the minimal sample and compares every defaulted field's value with the described default, deserializes each described variant, and feeds each numeric field the extremes of its described type, so a changed serde default, a removed variant or a narrowed type fails the test and a changed description changes the snapshot. The DTOs are private (`animation.rs:80-84`), so the Rust snapshot sees none of this; the components snapshot is their only guard.
   - `tungsten check` and a `describe` command can print them (W14b).
   - The inspector shows them.
   - The snapshot puts them under `just api-check` (P7).
   - A JSON Schema export comes later (Q8).
4. **Tuning without a rebuild (F2).** Data kinds hot-reload, so balance (effects, stats, drops) changes while the game runs.
5. **Seeing the kit run (F3, A).**
   - Inspector rows and `KitDebugPlugin` (F3).
   - An event trace (A, with the cues it explains): a bounded ring of kit events with frame and step in a `KitTrace` resource, shown in a debug view and read by a harness test, so a failing recipe can print why an entity died or never spawned.
6. **Driving a game headless (follow-up, W14b).** The proposal is `tungsten run --script <file>`: inputs at frames, assertions over components and the trace, screenshots, and a JSON report. An agent drives a game through the CLI with no new thread. A live bridge stays 1.x and must poll on the main thread.
7. **Templates that teach (F3 to E).** Each milestone lands its template change in the same step. `templates/basic`'s player becomes a prefab of kit components; later milestones add one hazard and one pickup, within the template's budget (§8).

## 6. Reference survey, 2026-10-15 and 2026-10-16

| Reference | What it teaches | Where it lands |
| --- | --- | --- |
| Unreal GAS: attributes, effects, tags, cues, abilities | One model for buffs, passives, damage over time, weapons and cosmetics. GAS's cues are cosmetic only; the kit's word covers gameplay reactions too, under §3's boundary | A `Stats` and cues, F3 `Tags`, E |
| GAS modifier order `((base + add) × Σmul ÷ Σdiv × Πcompound) + add_final`, override wins; multipliers sum | A fixed, documented order | A `Stats` |
| GAS stacking: limit; refresh, keep or extend; expiry clear all or remove one | Named policies in place of ad-hoc timers | E `Effect` |
| GDevelop's behaviour list (about 40 generic 2D behaviours) | The leaf catalogue | M, 1.x rows |
| Godot hit and hurt areas; `VisibleOnScreenEnabler2D`; TileSet custom data | Hit pairs; dormancy; per-tile data | C; 1.x `Dormant`; a core candidate |
| `bevy_spatial`: marker-tracked spatial index | A kit-owned index for bodiless overlap | C's grid |
| Bevy 0.16 relationships | Owner and source links kept by the engine | F1; F3's `Source` |
| Bevy `Messages<M>` and `MessageCursor` (0.20): a double buffer swapped once per update, consumption tracked per reader, `missed_messages` for a reader that fell behind, "readers that read at least once per update will never drop messages" | The per-reader cursor over a retained window is a settled design; its miss count is part of the API | F1 `EventCursor` |
| Bevy 0.17 (2025-09-30) splits `Event` (triggered and observed, entity-targeted as `EntityEvent`) from `Message` (buffered) as "fundamentally very different things" | Two mechanisms cost two models; the kit keeps buffered events and one reaction path | §3 cues, no observers |
| Godot resources: external `.tres` files or built-in resources inside the scene file, loaded once and shared; custom `Resource` classes as data containers | Shared definitions by reference or inline, one copy in memory | F2 `DataRef<T>` |
| Unity `ScriptableObject`: "a container for shared data used by multiple objects at runtime", which prefabs "access by reference" | The same split between a prefab and its shared definitions | F2's `data` section |
| Flecs prefabs: `IsA` inheritance with shared values, overrides, prefab variants, copied child subtrees | Variants and inherited values; the engine's prefabs copy, so variants are a later W16 row | 1.x row |
| Bevy `RecursiveDependencyLoadState`: an asset graph's load tracked per asset and per tree | A reference checked when the whole graph is present; here once, synchronously, at the manifest load | F2's two-phase load |
| Bevy remote protocol, `bevy_brp_mcp`, Godot MCP servers; MCP for Unity (third party, about 50 editor tools: scenes, scripts, tests, builds) | Agents run, inspect and drive a game; the surface they need is machine-readable descriptions and a scripted run | §5.3, §5.5, §5.6 |
| Godot's CLI: `--headless`, `--script`, `--quit-after`, `--write-movie` | A headless scripted run with frames and output is a CLI matter | Q10 |
| Godot Gameplay Systems: a GAS-shaped addon (attributes, abilities, effects), under rewrite | The GAS model travels between engines | A, E |
| `big-brain`, `seldom_state`, behaviour trees | AI beyond a flat state machine | 1.x rows |

Construct 3's official behaviour page returned 403; no row relies on it.

Sources:
- [GDevelop behaviours](https://wiki.gdevelop.io/gdevelop5/behaviors/all-behaviors/)
- [GAS gameplay effects](https://dev.epicgames.com/documentation/unreal-engine/gameplay-effects-for-the-gameplay-ability-system-in-unreal-engine)
- [`EGameplayModOp`](https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/GameplayAbilities/EGameplayModOp__Type)
- [GAS stacking](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/GameplayEffectStackingDurationPolicy)
- [Godot `VisibleOnScreenEnabler2D`](https://docs.godotengine.org/en/latest/classes/class_visibleonscreenenabler2d.html)
- [Godot TileSet layers](https://godotengine.org/article/tiles-editor-progress-3/)
- [`bevy_spatial`](https://docs.rs/bevy_spatial)
- [Bevy 0.15 to 0.16](https://bevy.org/learn/migration-guides/0-15-to-0-16/)
- [Bevy `Messages`](https://docs.rs/bevy_ecs/latest/bevy_ecs/message/struct.Messages.html) and [`MessageCursor`](https://docs.rs/bevy_ecs/latest/bevy_ecs/message/struct.MessageCursor.html)
- [Bevy 0.17 release notes](https://bevy.org/news/bevy-0-17/)
- [Godot resources](https://docs.godotengine.org/en/stable/tutorials/scripting/resources.html)
- [Unity `ScriptableObject`](https://docs.unity3d.com/Manual/class-ScriptableObject.html)
- [Flecs prefabs](https://www.flecs.dev/flecs/PrefabsManual.html)
- [Bevy `RecursiveDependencyLoadState`](https://docs.rs/bevy_asset/latest/bevy_asset/enum.RecursiveDependencyLoadState.html)
- [`bevy_remote`](https://docs.rs/bevy_remote)
- [`bevy_brp_mcp`](https://docs.rs/crate/bevy_brp_mcp/0.22.3)
- [godot-mcp-runtime](https://mcpservers.org/en/servers/erodenn/godot-mcp-runtime)
- [MCP for Unity](https://github.com/CoplayDev/unity-mcp)
- [Godot command line](https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html)
- [Godot Gameplay Systems](https://github.com/OctoD/godot-gameplay-systems)
- [`big-brain`](https://docs.rs/big-brain)

## 7. Catalogue by milestone

Each milestone is level C: new public API, so step 1's API sketch gets the critic (`D-150`). Each lands its template change, and each plan fixes the API from its row. Before the freeze, in order: F1, F2, F3, A, M, C, E. Shapes named here are the catalogue's; each plan's API sketch fixes them.

| Milestone | Contents | Waits on |
| --- | --- | --- |
| **F1, core events and ownership** | **`EventCursor`:** a per-reader sequence over `EventQueue`, reading each event once at any stage, with a missed count when windows dropped events. `SoundOnEvent` moves onto it, behaviour unchanged. **Spawn inheritance:** a core registry of what a spawn derives from its source entity, copied components the common case, applied to what a `PreparedPrefab` spawns from that source. A spawn queued through the command buffer derives at the flush, from the source as it is then, and a source despawned by an earlier command drops the spawn, so a spawner due in the frame its state exits leaves nothing behind (`state.rs:248-262` snapshots the state's entities in `PreUpdate`; the flush runs after `PostUpdate`, `app.rs:1645`). The umbrella registers `SceneEntity`, closing `known-issues.md:52` for the spawner. Changes ECS events, so every CPU row is captured | — |
| **F2, core data assets** | **The two-phase load:** at the manifest load, parse every prefab and data entry, insert the registries, then check all, and one failure fails the load. On a reload, stage every entry the reload adds or changes, one file or a whole manifest edit (`reload_new_prefabs` loads new prefabs one at a time today, `asset_loader/prefab.rs:56-82`), check them all against the registries with the staging in place of the old entries, then swap only when every check passes; a failure keeps the previous entries and logs why, as `reload_prefab` does (`asset_loader/prefab.rs:37-47`, `D-157`). **`DataRegistry`** and the manifest's `data` section (§3): kinds by name, entries by ID, `DataRef<T>`, hot reload, `MissingAsset` kinds `prefab` and `data`. Asset docs and the coverage check | F1 |
| **F3, kit foundations** | Domain subfolders, root re-exports unchanged; `prelude`; `KitPlugins`; the phase anchors, existing items moved onto them. **`Tags`** and `TagFilter` (all, any, none): hierarchical by dots, declared in a `tags` kind or at plugin build, checked at load. **`Source`:** the spawning entity, derived through F1's registry, so a spawn chain keeps its origin. **`Zone`:** tag-filtered sensor entries, the data form of `TriggerZone`. Scene components for `lifetime`, `despawn_outside`, `spawner`, `health`, `hit_flash`, `path_follower`, `top_down_mover`, `platformer_controller`. **`ActionInput`** writes intents (a move direction, an aim, slot presses) that the movers read when present, so input and AI share them; the top-down mover gains a bodiless mode by `Position` (Q14). **`CameraTarget`** over `CameraMode::Follow`. **Tooling:** inspector rows, `KitDebugPlugin`, field descriptions and their snapshot. **Tests:** the recipe harness and fixtures, the coverage test, the kit determinism hash, the criterion scaffold. **Template:** the growth rule, a player prefab with no game Rust, getting-started's "Composing with the kit" | F2 |
| **A, data and reactions** | **`Stats`:** base plus modifiers keyed by source, in the order `((base + Σadd) × (1 + Σpct) × Πmul)`, then override and clamp, with a dirty flag. Well-known stats kit items read when present (`move_speed`, `max_health`, `damage`, `cooldown_rate`, `area`, `pickup_radius`, `projectile_speed`); a game's own in the `stats` kind. **Reactions:** `Cues`, the `cue_rules` kind and `CuesOnEvent<E>` (§3) with A's actions, `min_interval` on sounds, `label` over `WorldLabels`. **The event trace** (§5.5). **Also:** `Cooldown` (repeating; `CooldownElapsed`), `Counters` and `Flags` (serde), `KitRng`, `SpawnChildren` | F3 |
| **M, motion and facing** | `LinearMotion`, `Follow`, `Orbit`, `Oscillator` (position, rotation or scale), `Facing`, `SpeedLimit`, `Wrap`, `StayInView`, `Knockback`, `Steering` (seek, flee, arrive), `Aim`; in `kit_present`, `CharacterAnimator` (the animation from motion state; example 01 hand-writes about 90 lines of it) and `DepthSort` (`z_order` from `y`) | A |
| **C, overlap and damage** | **The grid:** `HitShape`, `Hitbox` and `Hurtbox` on the kit's grid, rebuilt each step from hurtboxes and sorted; a hit is tested over the step's relative motion, the hitbox's and the hurtbox's sweeps both (each hurtbox enters the grid with its bounds grown by its step motion), so a fast bolt skips no hurtbox and two shapes that cross between their endpoints meet; hits come in sweep order, and a wall met along the sweep by a cast (`physics/query.rs:357`) ends it before the targets behind; one hit per target per activation; tag-filtered; `Source` carried from spawn to hit. **The damage pipeline:** `DamageRequest` (amount, source, tags, position, knockback, crit) resolved in `kit_resolve` in sorted order through immunity tags, `Resist`, `Shield` and `Health`. `Damaged` gains source, tags, position and crit, additively (`#[non_exhaustive]`). `damage` runs the same resolution at once. Heal requests; `Health`'s death policy. **Items:** `ContactDamage` (interval re-hits, so an aura is its shape), `Projectile` (pierce, range, bounce by casts; homing by `steering`), `Pickup` (magnet from `pickup_radius`, collect actions as cues), `WeightedTable`, the `roll` action and the `loot_tables` kind | A, M |
| **E, effects and abilities** | **`Effect`** in the `effects` kind: instant, timed or infinite; periodic; stack limit; refresh, keep or extend; expiry clear or remove one; modifiers on `Stats`; granted and blocking tags; `EffectApplied` and `EffectExpired`; the `effect` action. **`Ability`** inline or from the `abilities` kind: trigger by action, interval or event; cost in a stat; cooldown scaled by `cooldown_rate`; charges with a refill policy (by time, by pickup), which is what ammo and reload are; targeting self, aim, nearest by filter or random in range; payloads of a prefab, an effect, an impulse or a hitbox window. **`Level`:** an xp curve over a counter, `Curve` and the `curves` kind; offers from the `offers` kind; `LevelUp` | C |
| **After the freeze, 1.x rows** | `WaveSchedule` and a `waves` kind (until then `spawner`, or `cooldown` with a `prefab` cue, places waves, as the acceptance game's generator places its spawners); `Interactable`, `Checkpoint`, `Switch`, `Gate`, `PressurePlate`, `Portal`, `ForceField`, `Draggable`, `ViewEvents`, `Dormant`, `HitStop`, `Flicker`, `Blink`, `Flash`, `Shake`, `AfterImage`, `CameraRoom`, `BindText`; returning projectiles; `Grid<T>`, `Dir4`, `Dir8`, `GridMover`, `Perception`, `Patrol`, A\* and flow field, separation and boids; platformer extras (air jump, wall jump, ledge grab, one-way, slopes); state charts; utility AI; prefab variants (W16) | — |

**What left the eighth milestone.** G held `Pickup`, `WaveSchedule`, `DamageNumbers`, `YSort`, `CharacterAnimator`, the presets and the `kit_crowd` row. Pickups are overlap (C); the animator and depth sort are presentation of motion (M); damage numbers are a reaction, so one cue rule gives them to every enemy (A, P13); waves have a data-only stand-in and wait for a game; presets named genres (§5.1); the crowd row is C1's (P9). `Aura` and `Breakable` are compositions (§2 rule 16); `DropTable` is the `roll` action, so drops are one more reaction and not a component.

## 8. Template growth rule (F3's text)

"When the game passes about three components or two features, move each feature into `src/features/<name>/`: `mod.rs` holds its `Plugin`, `components.rs` and `systems.rs` the rest. `game.rs` only adds the feature plugins. A component two features share goes in `src/shared.rs`. A feature plugin registers its own prefab components, data kinds and systems, as a kit item does. The kit's scene names and data kinds are reserved: the registries fail startup on a duplicate."

The nested layout is shown by the reworked examples and the acceptance game; `templates/basic` stays basic (`w12-template.md`).

## 9. Ordered steps

1. **Integration** (§10): this review's session, after M59's release.
2. **F1.** Plan session (`tungsten-milestone`), then execution session and release; every CPU row captured (`tungsten-perf`).
3. **F2.**
4. **F3.**
5. **A.**
6. **M.**
7. **C.** Its plan opens with the grid and damage pipeline sketch.
8. **E.** Its plan opens with the `Effect` and `Ability` sketch, the largest API of the program.

W12b follows E, so the reworked examples are the kit's first real users before the freeze. W6b stays where the register has it: tags filter events, and masks later save broadphase work. The acceptance game's gap log holds no row yet (`acceptance-game.md:140`), so the recipes are the only user until W12b; a gate session may still drop or re-cut a registered stop (`D-109`).

## 10. Integration

The base is new public API before the freeze, so the route is the 1.0 program's (`D-140`, workflow §2). The finalizing session (2026-10-16, this one) integrates after M59's release (0.79) and before the next plan session; the release commit takes the whole tree (`D-105`).

- **Register:** seven Phase 6 candidates, each level C, none paired (`D-108` pairs only small level A or B candidates):

  | Candidate | Milestone |
  | --- | --- |
  | W13e | F1 |
  | W13f | F2 |
  | W13g | F3 |
  | W13h | A |
  | W13i | M |
  | W13j | C |
  | W13k | E |

  W12b's needs gain W13k.
- **Records:**
  - `implementation-plan.md` §4 (rows and cards) and §10, `roadmap.json` (stops after `uilab`, before `w6bc`) and `just repo-check`;
  - `w13-kit.md` holds this catalogue as the candidates' scoping source;
  - `backlog-1.x.md` rows for the 1.x list;
  - the catalogue plan, `docs/plans/kit-catalogue.md`, moved to `docs/plans/archive/kit-catalogue.md` as superseded; this file holds its text.
- **Cost:** seven release cycles, each with a plan critique and a step-1 API critique. The smaller first cut would be F1 to F3, then re-decide; the foundations pay off whatever follows.

## 11. Done-when

- **Releases:**
  - Each milestone's release is cut with its plan's checks green.
  - `just api-check` is clean at each release, comparing `api/tungsten-kit.txt` (each item once, at one public path) and `api/tungsten-kit-components.txt`.
- **Coverage:** the coverage test passes, with every scene name and data kind in at least two genres' fixtures. Every kit component and data kind has inspector rows or a description, each checked by a test.
- **Foundations:**
  - Every kit reader uses `EventCursor`; `sound.rs` keeps no handled count.
  - A prefab or data entry naming a missing prefab or data ID fails the manifest load with the file and the field.
  - Editing a data file hot-reloads it for later reads, and a manifest edit that adds two entries referencing each other loads both (harness tests).
  - A kit-spawned entity leaves with its spawner's state (harness test).
- **No game Rust:**
  - `templates/basic`'s player has no game Rust for movement, health or camera (F3).
  - A top-down recipe counts kills, bursts the enemy, shows the damage and rolls a drop through cues and cue rules alone (C).
- **Stability:**
  - The kit determinism hash is unchanged at each release, or its change is recorded in that release's decision.
  - The physics hashes (`0x088e_c07a_73c1_b168` and the containment hash `0xaee2_72e0_1ffc_4e4c`) and `DefaultPlugins`' resolved schedule are unchanged at every release.
  - Each per-entity item has a criterion row whose per-entity time does not grow between 1,000 and 10,000 entities (each plan fixes the bound).
- **Tests:** `cargo test -p tungsten-kit`, the umbrella's `kit_tests` and the recipes pass, and every check `just checks` recommends for the milestone's change has run with its result in the evidence log (the selector recommends and never certifies, `docs/validation.md:51`).
- **Ownership and hits:** a harness test exits a state in the frame its spawner is due and finds nothing left (F1); swept-hit tests send a bolt faster than a hurtbox's width per step through it, and cross a bolt and a moving hurtbox between their endpoint positions, and count one hit each (C).

## 12. Open questions, each with a default

- **Q1. Bodiless overlap.** Settled (2026-10-15): a kit-owned grid; C fixes the details within §2 rule 6.
- **Q2. Scope before the freeze.** Settled: F1 to E, seven milestones (this review); the rest are 1.x rows.
- **Q3. Stat keys.** Default: interned `StatId`s from the `stats` kind or plugin build; the kit predeclares its well-known stats.
- **Q4. Tag declaration.** Default: the `tags` kind or plugin build, both checked at load.
- **Q5. Data tables.** Settled (2026-10-16): the `data` section and `DataRegistry` (F2).
- **Q6. Damage pipeline.** Default: queued requests resolved once a step in sorted order, `damage` as the immediate path over the same resolution. No break.
- **Q7. `EventCursor` placement.** Default: in core beside `EventQueue`, since the umbrella's readers share the problem; it amends `D-040` and takes the cursor `D-139` left open.
- **Q8. Descriptions and schema.** Default: hand-written descriptions checked by the defaults-and-variants test (§5.3). A JSON Schema export, and any crate for it, waits for a `D-015` entry.
- **Q9. Genre starters.** Default: the genre fixtures (§4), then W12b's reworked examples, become starters `tungsten new` can copy (W14b).
- **Q10. Agent bridge.** Default: `tungsten run --script` with a JSON report, proposed to W14b; a live bridge stays 1.x.
- **Q11. This draft in M59's commit.** Moot: M59 was cut as 0.79 on 2026-10-16 and this file sits in the 0.80 tree.
- **Q12. Effect and ability definitions.** Settled (2026-10-16): data kinds, by ID or inline (§3).
- **Q13. Cue rule order.** Default: entity cues before cue rules, both in event order, each event's actions in written order.
- **Q14. Bodiless movers.** Default: the top-down mover moves an entity without a body by `Position` (§2 rule 6), so the template's player and a shmup's ship need no `rigid_body`; a kinematic body keeps today's cast-and-slide path (`D-158`). F3 fixes the shape.
- **Q15. Drops.** Default: the `roll` cue action over the `loot_tables` kind, not a `DropTable` component, since a drop is a reaction to `killed`. C fixes the shape.
- **Q16. Damage numbers.** Default: the `label` cue action over core's `WorldLabels` (`text.rs:564-583`), the amount of the event in a named style whose seconds, rise and scale curve the kit ages on the game clock, pushing each live label every frame; the per-label scale and offset are W1 WL's (`D-161`). A fixes the shape and whether styles are a data kind.

## 13. Decisions expected

- F1: the event cursor (amending `D-040`, taking up `D-139`'s open cursor); spawn inheritance.
- F2: the two-phase load (amending `D-157`); the data registry and the `data` section (extending the manifest's `D-017` and `D-052`).
- F3: the amended admission rule (amending `D-155`); the kit phases (amending `D-158`); tags and `Source`; `KitPlugins`; the components snapshot in `just api-check` (amending `D-107`).
- A: the stats model; the cue vocabulary and its boundary.
- M: the motion write rule as the kit's (§2 rule 6).
- C: the overlap grid and the damage pipeline (amending `D-158`'s health).
- E: the effect and ability model.
- One landed decision per milestone, as `D-155` and `D-158` were.

IDs are the next free ones when each is written (`tungsten-decision`).

## 14. Invariants and stop conditions

- **Must not move:**
  - The kit depends on core alone, and core names no kit type.
  - No new third-party dependency (`D-015`).
  - `DefaultPlugins`, the determinism hash `0x088e_c07a_73c1_b168` and the containment hash `0xaee2_72e0_1ffc_4e4c`.
  - Opt-in plugins.
  - No render types in the kit.
  - Two background threads.
  - Prefab and scene files keep their fields (`D-153`, `D-157`): new behaviour arrives as components and data kinds.
- **Stop and record, do not work around:**
  - An item needs a physics change: mark it waiting.
  - An item cannot pass recipes in two genres: it is not generic, so leave it out.
  - The API critique finds a shape tied to one genre: redesign or drop.
  - A cue needs a condition beyond a tag filter: the reaction belongs in game Rust.
  - A per-entity row grows super-linearly: fix it before the release.
  - F1 moves a CPU row past its noise: stop at the release.

## Appendix: baseline sketch (non-binding)

How the base composes once E lands. Field names are placeholders for each milestone's API sketch; the shapes are what the recipes test.

```jsonc
// top_down/prefabs/hero.json: a player with no game Rust
{ "sprite": { "asset_id": "hero" }, "components": {
  "tags": ["team.player"],
  "action_input": { "move": ["move_left", "move_right", "move_up", "move_down"], "slots": { "primary": "attack" } },
  "top_down_mover": { "speed": 160 },
  "stats": { "move_speed": 160, "pickup_radius": 48 },
  "health": { "max": 5, "invulnerability": 0.8, "on_death": "keep" },
  "hurtbox": { "shape": { "circle": 10 } },
  "hit_flash": { "color": [1, 1, 1, 1], "seconds": 0.12 },
  "camera_target": null,
  "abilities": ["wand"],
  "cues": { "damaged": [{ "sound": "hurt", "min_interval": 0.1 }, { "shake": 0.3 }] }
} }

// top_down/data/abilities/wand.json: shared by every prefab that names "wand"
{ "trigger": { "interval": 0.8 },
  "target": { "nearest": { "all": ["team.enemy"] }, "range": 320 },
  "payload": [{ "spawn": { "prefab": "bolt", "speed": 420 } }] }

// top_down/prefabs/bolt.json: spawned by the ability, inheriting Source and SceneEntity
{ "sprite": { "asset_id": "bolt" }, "components": {
  "tags": ["team.player"],
  "projectile": { "pierce": 1, "range": 600 },
  "hitbox": { "shape": { "circle": 4 }, "damage": 3, "filter": { "all": ["team.enemy"] } },
  "cues": { "expired": [{ "prefab": "spark" }] }
} }

// top_down/prefabs/slime.json
{ "sprite": { "asset_id": "slime" }, "components": {
  "tags": ["team.enemy"],
  "steering": { "seek": { "nearest": { "all": ["team.player"] } }, "max_speed": 90 },
  "health": { "max": 6, "on_death": { "despawn": 0.1 } },
  "hurtbox": { "shape": { "circle": 9 } },
  "contact_damage": { "damage": 1, "interval": 0.5, "filter": { "all": ["team.player"] } },
  "cues": { "killed": [{ "roll": "slime_drops" }, { "prefab": "slime_burst" }, { "sound": "pop" }] }
} }

// top_down/data/cue_rules/combat.json: every enemy kill counts and every enemy hit shows its damage,
// with no field on each prefab
[{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1 } }] },
 { "on": "damaged", "filter": { "all": ["team.enemy"] }, "do": [{ "label": "damage" }] }]

// top_down/prefabs/xp_gem.json
{ "sprite": { "asset_id": "gem" }, "components": {
  "pickup": { "magnet": true, "filter": { "all": ["team.player"] }, "collect": [{ "counter": { "name": "xp", "add": 1 } }] },
  "oscillator": { "channel": "position", "axis": [0, 1], "amplitude": 3, "period": 1.2 }
} }

// platformer/data/effects/burning.json
{ "duration": 3, "period": 0.5, "periodic": { "damage": 1, "tags": ["damage.fire"] },
  "stack": { "limit": 3, "refresh": "extend", "expire": "remove_one" },
  "modifiers": [{ "stat": "move_speed", "pct": -0.2 }],
  "grant_tags": ["status.burning"], "blocked_by": ["status.wet"] }

// platformer/prefabs/fire_hazard.json: example 01's fireball, as data
{ "sprite": { "asset_id": "fireball" }, "components": {
  "animation": { "id": "fireball" },
  "path_follower": { "points": [[0, 0], [0, -96]], "speed": 80, "mode": "ping_pong", "easing": "sine_in_out" },
  "hitbox": { "shape": { "circle": 18 }, "damage": 1, "tags": ["damage.fire"], "filter": { "all": ["team.player"] }, "apply": ["burning"] },
  "spawn_children": [{ "prefab": "fire_glow", "offset": [0, 0] }]
} }
```

## Critique and evidence

The catalogue's two critique rounds, each finding with its disposition, and its evidence log are in the archived plan, `docs/plans/archive/kit-catalogue.md`. Each candidate's plan records its own critique.

## Follow-ups

- **Links into the archived catalogue.** `docs/plans/phase7-rewrite.md`, another session's draft open while the catalogue was archived on 2026-10-16, links `kit-catalogue.md` twice (its non-goals and its §10 "Why B2"); both should point at this file, `1.0/w13-kit.md`, which holds the catalogue and its digest. Left to that draft's session; `just repo-check` reports the three broken links until then.
Each waits for a user (M50's plan, archived at `docs/plans/archive/1.0/phase6-milestone-50-kit-basics-hierarchy.md`):

- Reparenting and detaching through the kit; today a game removes `Parent`, which detaches the child where it stands.
- Spawning children through the command buffer; `spawn_child` is immediate, and a child spawned at the flush would draw one frame before its first propagation.
- Rotation and scale inheritance, with a pivot design for the default extract, which turns a sprite about its centre (M50 Q7).
- A `KitPlugins` set, if games add most of the kit's plugins together.
- Scene components for `lifetime` and `despawn_outside`, when W16b's prefabs need them (M50 Q8).
- An exactly-once event reader in core, if a second reader needs `SoundOnEvent`'s handled-count cursor. Landed in M60 (`D-170`): `EventCursor`, through which `SoundOnEvent` and `TriggerZone` read.

From M51 (`docs/plans/archive/1.0/phase6-milestone-51-prefabs-tiled-objects.md`), each waiting for a user:

- A `spawner` scene component, so a level file places a `Spawner`; the acceptance game's generator places its spawners in code (M51 Q16).
- State ownership of spawned entities: a `Spawner`'s entities carry no `SceneEntity`, so a state's exit leaves them ([known issues](../../known-issues.md)); the acceptance game's first gap. Landed in M60 (`D-170`): the spawner spawns from its entity, whose `SceneEntity` the spawn inherits under `StatePlugin`.

From M52 (`docs/plans/archive/1.0/phase6-milestone-52-controllers-helpers.md`):

- Example 01's player onto the platformer controller, with W12b: the `gate_step` corner first ([known issues](../../known-issues.md)); the path follower's user is example 01's moving platforms, also W12b's.
- Slopes, one-way platforms and ladders: the [backlog](backlog-1.x.md) (Q22).
- A controller that pushes dynamic bodies, or a body-kind query filter: W6b's layers and masks first, a push option when a game asks.
- Scene components for the controllers and helpers, when a level file needs one; a game's own component builds the kit's meanwhile (`D-153`).
- `damage_flash` for a game outside this repository: the template or W9b's guide.
- Sensor exit events, when a zone needs leaving (`D-154`).

## Follow-ups/{/^## Follow-ups/!d}'`) and a focus file naming round 1's changes, 2026-10-16: gpt-6-astra, 352 s, 674,422 tokens in (592,512 cached), 29 commands, 282 KB output, 5h window 22% → 41%. Three findings, all checked at `0732305` and all accepted. There is no third round.

| # | Finding | Checked against | Disposition |
| --- | --- | --- | --- |
| 1 | MEDIUM. Staging one changed entry misses a manifest edit that adds entries referencing each other; today's manifest reload inserts new prefabs one at a time in sorted order. | `asset_loader/prefab.rs:56-82` (`reload_new_prefabs`) | Accepted. F2 stages every entry a reload adds or changes before the checks; §11 gains the two-new-entries test. |
| 2 | MEDIUM. Sweeping the hitbox alone misses a hurtbox that moves across its path within the step. | the C row as written after round 1; `physics/query.rs:347` (casts see the last step's positions) | Accepted. C tests hits over the step's relative motion, both sweeps; §11 gains the crossing test. |
| 3 | MEDIUM. The defaults-and-variants test misses a narrowed numeric type, and the DTOs are private, so the Rust API snapshot misses it too. | `animation.rs:80-84` (`AnimationDef` is private); `scripts/public-api.sh:62` | Accepted. §5.3's test feeds each numeric field the extremes of its described type; the components snapshot is named as the DTOs' only guard. |

## Evidence log

| Date | Step | Verdict |
| --- | --- | --- |
| 2026-10-16 | Finalizing review on `0732305` | §1 re-verified: R1's ranges moved (`event_queue.rs` is 185 lines; `sound.rs:135-162`), R3 to `health.rs:172-229`, R5 to `physics/plugin.rs:60-95`, R6's duplicate is caught at `resolve` (`schedule.rs:344-351`), R7's known-issues row is line 52; the rest hold. Seams confirmed (§1). Two critique rounds, ten findings, all accepted (Critique). Verdict: go, seven milestones; integrated per §10. |

- **Core candidates the catalogue points at, none started:**
  - Per-tile gameplay properties.
  - Physics features: friction, sensor exit and stay, approach speed.
  - A JSON Schema export (Q8).
  - Input record and replay (`D-129` left it open).
  - `tungsten run --script` (Q10).
  - A `spawn <prefab>` debug console command.
- **Outside `docs/plans/`, left to the milestones:** `docs/known-issues.md:52` closed with F1 in M60 (`D-170`), its exit-frame limit kept; `scripts/check-repo.py`'s asset coverage learns the `data` section with F2; `scripts/public-api.sh` writes the components snapshot with F3.
- **Review records:** this plan's findings (§1) go into F1's audit with the commit they were read at.
