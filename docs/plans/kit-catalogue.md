# Kit catalogue: generic, cross-genre components — draft

- **status:** draft
- **goal:** Make Tungsten a better engine to build 2D games with, for developers and for AI agents.
  - A game of any 2D genre composes its entities from data over a broad, generic `tungsten-kit`, writes Rust only for what is its own, and can see, test and debug what the kit does.
  - The base ships before the 1.0 freeze: the foundations, tags, stats, cues, motion, overlap, damage, effects and abilities.
  - The leaf items follow in 1.x, since adding them breaks nothing.
- **non-goals:**
  - Physics features: friction, layers and masks (W6b), sensor exit and stay, approach speed on contacts.
  - Per-tile gameplay properties.
  - Reworking the examples (W12b).
  - Render types in the kit (`D-007`).
  - Marking or cataloguing a project's own components: the owner skipped it.
  - A scripting language (§3).
  - UI widgets, 3D, multiplayer, physics joints.
  - A live agent bridge into a running game (§5.6 records it as a follow-up).
- **files to touch:**
  - F1: `crates/tungsten-core/src/ecs/event_queue.rs`, a spawn-inheritance module in core, `crates/tungsten-core/src/assets/prefab.rs` (spawn from a source), `crates/tungsten/src/state.rs`, `crates/tungsten-kit/src/sound.rs`.
  - F2: `crates/tungsten-core/src/assets/manifest.rs`, `prefab.rs` and a data-registry module, `crates/tungsten/src/asset_loader/` (`mod.rs`, `prefab.rs`, `reload.rs`), `docs/assets.md`, `scripts/check-repo.py`'s asset coverage.
  - F3 onward: `crates/tungsten-kit/src/` (domain subfolders, new modules), `crates/tungsten-kit/benches/`, `crates/tungsten/src/tests/` and recipe fixtures, `crates/tungsten/src/testing.rs`.
  - Template and docs: `templates/basic/`, `docs/getting-started.md`, `docs/LLM_INDEX.md`, `DESIGN.md`.
  - Every milestone: `api/` (by `just api`), `DECISIONS.md` and `docs/DECISION_INDEX.md`, `CHANGELOG.md`.
  - Graduation: `docs/plans/1.0/` records (§10).
- **ordered steps:** §9.
- **done-when:** §11.

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
- **The survey** (§6) found:
  - one design recurring across engines: tags, ordered stat modifiers, stacking effects and abilities, with cosmetic reactions decoupled (Unreal's Gameplay Ability System);
  - agent tooling built on running, inspecting and driving a game, and on machine-readable component descriptions.
- **Example 01** reimplements kit-shaped mechanics. Its perf shows what to avoid: per-entity scans, per-step hash grids, 12,000 physics bodies.

## 1. Review findings, 2026-10-16

Each finding names its evidence at the 0.79 tree and where the plan answers it.

| # | Weak point | Evidence | Answer |
| --- | --- | --- | --- |
| R1 | **No exactly-once event reader.** A reader must choose `iter` or `iter_current` by its place against the senders and the stage. `SoundOnEvent` works around it with a handled count that holds in `PostUpdate` only. The catalogue adds about ten reactive readers across both stages. | `event_queue.rs:1-160`; `sound.rs:131-157`; `w13-kit.md`'s open item | F1: `EventCursor` in core; `SoundOnEvent` moves onto it |
| R2 | **Assets cannot reference each other.** Prefabs load in sorted ID order, each is checked against the World, and the registry is inserted after the loop. An ability's projectile, a drop or an effect named by ID is unchecked until used. | `asset_loader/prefab.rs:19-31`; `prefab.rs:95-124` | F2: a two-phase load and the data registry (§3) |
| R3 | **Damage has no source, kind or hook.** Resistances, kill credit, friendly fire and damage types have nowhere to go. | `damage(world, target, amount)`, `Damaged { entity, amount, killed }`, `health.rs:172-240` | C: the damage pipeline; `damage` stays the immediate path; `Damaged.killed` makes a `Killed` event unnecessary |
| R4 | **A kit hit shape must not be a `Collider`.** The gather makes every entity with a `Collider` and a `Position` a physics proxy, static without a `RigidBody`. | `gather.rs:59-106` | §2 rule 6; C's `HitShape` |
| R5 | **Motion had no write target.** The sync interpolates any entity with `Transform` and `Position`, and the history copies any `Position` with a `PrevPosition`; only a `Collider` makes a proxy. So bodiless motion by `Position` in `FixedUpdate` is deterministic and smooth at no cost, and motion by `Transform` stutters or is overwritten. | `physics/plugin.rs:56-90`; `gather.rs:59` | §2 rule 6 |
| R6 | **Ordering does not scale.** The schedule has names with before, after and if-present, and no sets; a second add of a name is an error. | `schedule.rs:143-300`, `:327` | F3: phase anchors (§2 rule 7) |
| R7 | **Kit spawns outlive their state.** `SceneEntity` is the umbrella's, so a kit on core alone cannot copy it. | `state.rs:35`; `known-issues.md:51` | F1: spawn inheritance |
| R8 | **Existing items are not data-first.** Only `animation` is a scene component; `TriggerZone<Z, T>`, `StateMachine<S>` and `SoundOnEvent<E>` cannot be named in data. | `animation.rs:72` | F3: scene components for the eight data-shaped items; `Zone` |
| R9 | **Recipes need the real frame loop.** Event rotation, step views and held frames are the App's; the kit's doctests run hand-written loops. | `event_queue.rs:1-38`; `D-139`; `testing.rs:55-241` | §4: recipes on the umbrella's `Harness` |
| R10 | **The kit is invisible when debugging.** No kit component has inspector rows. | `inspect.rs:9-40`; `debug_draw.rs:46-85` | F3: inspector rows; `KitDebugPlugin` |
| R11 | **No change detection.** The ECS has `With` and `Without` only. | `query.rs:363-366` | §2 rule 8 |
| R12 | **Stats would not drive existing items.** Speeds, maxima and cooldowns read their own fields. | kit sources | A: well-known stats kit items read when present |

**Second pass.** What a data-first prefab still could not express, and what would let a later change break games silently.

| # | Gap | Answer | Milestone |
| --- | --- | --- | --- |
| P1 | Reactions need Rust: each `SoundOnEvent<E>` pairing is a plugin a game writes. | `Cues` (§3) | A |
| P2 | Nothing says what death does. | `Health`'s death policy: despawn after a delay, cues first, or keep | C |
| P3 | A player needs Rust for input and camera. | `ActionInput`, `CameraTarget` | F3 |
| P4 | Aiming is per game. | `Aim` | M |
| P5 | Randomness would scatter across items. | `KitRng`: one stream seeded from the run seed, drawn in sorted order | A |
| P6 | Healing, crits and regeneration had no path. | Heal requests and a `crit` flag through the pipeline; regeneration as a periodic effect | C, E |
| P7 | **A DTO change breaks every game's data silently**: `just api` snapshots Rust items, not JSON shapes, and the formats sit inside the 1.0 promise (`D-103`). | Field descriptions written to `api/tungsten-kit-components.txt`, compared by `just api-check` | F3 |
| P8 | A reorder of kit systems could change gameplay with no failing test. | A kit determinism hash over the genre recipes, pinned as the physics hashes are | F3 |
| P9 | The acceptance game's load has no row. | `kit_crowd`, the kit row C1 planned | G |
| P10 | Kit state could not be saved. | Data state derives serde (§2 rule 14) | every milestone |
| P11 | Hit sounds flood the mixer until W6c's voice limit. | `min_interval` on sound cues and `SoundOnEvent` | A |
| P12 | HUD values need code. | 1.x: `BindText` | after the freeze |

## 2. The contract for every item

1. **Generic.** Names no game type. Filters on `Tags`, or takes the game's types with a tag-filtered data form beside it. Assumes no gravity, no up direction and no player. Units are world pixels, Y down.
2. **Cross-genre.** Names at least two of the four reference genres (platformer, top-down action, shmup, grid puzzle or roguelike) and passes a recipe in each (§4).
3. **Data first.** A plain component with a builder, and a scene registration under a reserved bare name with a serde DTO. Shared definitions are data kinds (§3), named by ID or written inline. Every ID is checked at load. A game composes the item with no Rust.
4. **Events out, reactions as cues.** Each event names its entity and, where it has one, its position, so cues can react to it (§3). Every reader uses `EventCursor`.
5. **Owned.** Whatever it spawns inherits the spawner's inherited components (F1), so a state's exit takes it.
6. **Writes stated.**
   - Motion writes `Position` (bodiless or static) or `Velocity` (kinematic or dynamic), never the `Transform` of an entity with a `Position`.
   - A bodiless entity carries `Position`, `PrevPosition` and `Transform`, and no `Collider`.
   - Hit shapes are the kit's own component.
7. **Phased.** Each system joins one phase anchor and never names another kit item's system. An anchor is a no-op system that a plugin adds when `Schedule::contains` says it is absent, ordered against the engine's names with if-present constraints. `D-158`'s chain maps onto the anchors unchanged.

   | Phase | Stage | What runs there |
   | --- | --- | --- |
   | `kit_tick` | `FixedUpdate`, after the history snapshot | timers, cooldowns, effect durations, health windows |
   | `kit_intent` | `FixedUpdate` | input, aim, abilities, AI, state machines |
   | `kit_move` | `FixedUpdate`, before `physics_step` | motion items, controllers, the path follower |
   | `kit_overlap` | `FixedUpdate`, after `physics_step` | the hit grid, zones, pickups |
   | `kit_resolve` | `FixedUpdate` | damage, heals, effects, stats |
   | `kit_react` | `FixedUpdate`, last | cues, drops, spawns, counters, levels |
   | `kit_present` | `PostUpdate`, after the sync and the late propagation | animators, facing, damage numbers, YSort |
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

**Item card.** Each milestone plan fills one card per item: component and scene name; DTO fields with defaults; data kinds it reads; events in and out; phase; writes; stats read; genres and recipes; inspector rows and debug drawing; perf row.

## 3. The data model

Three layers. Each is checked at load, and none needs game Rust.

1. **Components in prefabs.** What one entity is: `"health": { "max": 5 }`.
2. **Definitions in data kinds.** Shared, named definitions in the manifest's `data` section (F2).
   - Core's `DataRegistry` takes kinds that plugins register by name with a DTO and a load check, as `ComponentRegistry` takes components. Core never learns the kit's types.
   - Entries load before prefabs in the two-phase load, hot-reload for later reads, and fail the load on a bad ID with the file and the field.
   - A field that names a definition is a `DataRef<T>`: an ID (`"burning"`) or the definition written in place. Prototype inline; promote to an ID when a second prefab needs it.
   - Kit kinds: `tags`, `stats`, `effects`, `abilities`, `loot_tables`, `waves`, `curves`, `offers`, `cue_rules`. A game registers its own kinds the same way (enemy tables, item lists), which makes this the data tables a game tunes from.

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

   Cue vocabulary:
   - **Events** (a closed set): spawned, damaged, healed, killed, expired, hit, picked_up, ability_used, effect_applied, effect_expired, zone_entered, cooldown_elapsed.
   - **Actions:**
     - gameplay: `prefab` (inheriting), `counter`, `flag`, `effect` (on self or source), `despawn`;
     - presentation: `sound` (with `min_interval`), `shake`, `squash`, `flash`.

   **The boundary.**
   - Each action takes at most one tag filter.
   - There are no conditions, variables or sequences. Anything more is Rust in a game's feature module.
   - Gameplay actions run in sorted order; presentation actions change no state.
   - Cues run before the death policy's despawn, so `killed` cues always fire.

## 4. Recipes replace "has a user"

A recipe is a set of prefabs and data plus a harness test that spawns it and asserts the behaviour, with no game Rust.
- Recipes run on the umbrella's `Harness`, so event rotation, step views and held frames are the App's (R9).
- Each genre has a fixture folder with its own manifest: `platformer/`, `top_down/`, `shmup/`, `grid/`.
- A coverage test builds every kit plugin into a World and fails when a registered scene name or data kind appears in fewer than two genres' fixtures.

Example, `Projectile`:
- top-down: a bolt that pierces two enemies and expires;
- platformer: a fireball that stops at a wall;
- shmup: a bullet despawned outside the view.

A recipe is the evidence a first game would give, before the freeze. A real game can still force a change, which takes a break-ledger row before 1.0 (`w04-api-freeze.md`). The fixtures are the cookbook the template and agents copy from.

Fallback, not adopted: if the freeze review objects to recipe-proven items, a `preview` cargo feature holds them outside the 1.0 promise.

## 5. Developer and agent ergonomics

What makes the engine faster to build with, for a developer reading rustdoc and for an agent writing data and tests.

1. **One way in (F3).**
   - `tungsten::kit::prelude`.
   - `kit::presets::{Platformer, TopDown, Shmup, Grid}` (G).
   - The naming convention (§2 rule 12).
   - The kit's root docs as the catalogue, grouped by domain, each item linked to its recipe.
2. **Errors that say what to do (every milestone).** A data error names the file, the field path and the expected values, and suggests the nearest name for an unknown tag, stat, prefab, data ID or component. `serde_path_to_error` gives the path (`D-153`).
3. **Field descriptions (F3, then each milestone).** Each scene component and data kind registers a description: summary, fields with types and defaults, events, genres. A test deserializes a sample built from it, so the two cannot drift.
   - `tungsten check` and a `describe` command can print them (W14b).
   - The inspector shows them.
   - The snapshot puts them under `just api-check` (P7).
   - A JSON Schema export comes later (Q8).
4. **Tuning without a rebuild (F2).** Data kinds hot-reload, so balance (effects, stats, waves, drops) changes while the game runs.
5. **Seeing the kit run (F3).**
   - Inspector rows and `KitDebugPlugin`.
   - An event trace: a bounded ring of kit events with frame and step, shown in a debug view and returned by `Harness::kit_trace()`, so a failing test can print why an entity died or never spawned.
6. **Driving a game headless (follow-up, W14b).** The proposal is `tungsten run --script <file>`: inputs at frames, assertions over components and the trace, screenshots, and a JSON report. An agent drives a game through the CLI with no new thread. A live bridge stays 1.x and must poll on the main thread.
7. **Templates that teach (F3 to G).** Each milestone lands its template change in the same step. `templates/basic`'s player becomes a prefab of kit components; later milestones add one hazard and one pickup, within the template's budget (§8).

## 6. Reference survey, 2026-10-15 and 2026-10-16

| Reference | What it teaches | Where it lands |
| --- | --- | --- |
| Unreal GAS: attributes, effects, tags, cues, abilities | One model for buffs, passives, damage over time, weapons and cosmetics | A `Stats` and cues, F3 `Tags`, E |
| GAS modifier order `((base + add) × Σmul ÷ Σdiv × Πcompound) + add_final`, override wins; multipliers sum | A fixed, documented order | A `Stats` |
| GAS stacking: limit; refresh, keep or extend; expiry clear all or remove one | Named policies in place of ad-hoc timers | E `Effect` |
| GDevelop's behaviour list (about 40 generic 2D behaviours) | The leaf catalogue | M, G, 1.x rows |
| Godot hit and hurt areas; `VisibleOnScreenEnabler2D`; TileSet custom data | Hit pairs; dormancy; per-tile data | C; 1.x `Dormant`; a core candidate |
| `bevy_spatial`: marker-tracked spatial index | A kit-owned index for bodiless overlap | C's grid |
| Bevy 0.16 relationships | Owner and source links kept by the engine | F1; C's `Source` |
| Bevy remote protocol, `bevy_brp_mcp`, Godot MCP servers | Agents run, inspect and drive a game | §5.5, §5.6 |
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
- [`bevy_remote`](https://docs.rs/bevy_remote)
- [`bevy_brp_mcp`](https://docs.rs/crate/bevy_brp_mcp/0.22.3)
- [godot-mcp-runtime](https://mcpservers.org/en/servers/erodenn/godot-mcp-runtime)
- [`big-brain`](https://docs.rs/big-brain)

## 7. Catalogue by milestone

Each milestone is level C: new public API, so step 1's API sketch gets the critic (`D-150`). Each lands its template change, and each plan fixes the API from its row. Before the freeze, in order: F1, F2, F3, A, M, C, E, G.

| Milestone | Contents | Waits on |
| --- | --- | --- |
| **F1, core events and ownership** | **`EventCursor`:** a per-reader sequence over `EventQueue`, reading each event once at any stage, with a missed count when windows dropped events. `SoundOnEvent` moves onto it, behaviour unchanged. **Spawn inheritance:** a core registry of component types copied from a source entity to what a `PreparedPrefab` spawns from it. The umbrella registers `SceneEntity`, closing `known-issues.md:51` for the spawner. Changes ECS events, so every CPU row is captured | — |
| **F2, core data assets** | **The two-phase load:** parse every prefab and data entry, insert the registries, then check all, on load and on reload. **`DataRegistry`** and the manifest's `data` section (§3): kinds by name, entries by ID, `DataRef<T>`, hot reload, `MissingAsset` kinds `prefab` and `data`. Asset docs and the coverage check | F1 |
| **F3, kit foundations** | Domain subfolders, root re-exports unchanged; `prelude`; the phase anchors, existing items moved onto them. **`Tags`** and `TagFilter` (all, any, none): hierarchical by dots, declared in a `tags` kind or at plugin build, checked at load. **`Zone`:** tag-filtered sensor entries, the data form of `TriggerZone`. Scene components for `lifetime`, `despawn_outside`, `spawner`, `health`, `hit_flash`, `path_follower`, `top_down_mover`, `platformer_controller`. `ActionInput` and `CameraTarget`. **Tooling:** inspector rows, `KitDebugPlugin`, the event trace, field descriptions and their snapshot. **Tests:** the recipe harness, the coverage test, the kit determinism hash, the criterion scaffold. **Template:** the growth rule, a player prefab with no game Rust, getting-started's "Composing with the kit" | F2 |
| **A, data and reactions** | **`Stats`:** base plus modifiers keyed by source, in the order `((base + Σadd) × (1 + Σpct) × Πmul)`, then override and clamp, with a dirty flag. Well-known stats kit items read when present (`move_speed`, `max_health`, `damage`, `cooldown_rate`, `area`, `pickup_radius`, `projectile_speed`); a game's own in the `stats` kind. **Reactions:** `Cues`, the `cue_rules` kind and `CuesOnEvent<E>` (§3), with `min_interval` on sounds. **Also:** `Cooldown` (repeating; `CooldownElapsed`), `Counters` and `Flags` (serde), `WeightedTable`, `Curve` and the `curves` kind, `KitRng`, `SpawnChildren` | F3 |
| **M, motion** | `LinearMotion`, `Follow`, `Orbit`, `Oscillator` (position, rotation or scale), `Facing`, `SpeedLimit`, `Wrap`, `StayInView`, `Knockback`, `Steering` (seek, flee, arrive, separate), `Aim` | A |
| **C, overlap and damage** | **The grid:** `HitShape`, `Hitbox` and `Hurtbox` on the kit's grid, rebuilt each step from hurtboxes and sorted; one hit per target per activation; tag-filtered. **`Source`**, carried from ability to spawn to hit. **The damage pipeline:** `DamageRequest` (amount, source, tags, position, knockback, crit) resolved in `kit_resolve` in sorted order through immunity tags, `Resist`, `Shield` and `Health`. `Damaged` gains source, tags, position and crit, additively (`#[non_exhaustive]`). `damage` runs the same resolution at once. Heal requests; `Health`'s death policy. **Items:** `ContactDamage`, `Projectile` (pierce, bounce, return, homing, range), `Breakable`, `DropTable` with the `loot_tables` kind, `Aura` | A, M |
| **E, effects and abilities** | **`Effect`** in the `effects` kind: instant, timed or infinite; periodic; stack limit; refresh, keep or extend; expiry clear or remove one; modifiers on `Stats`; granted and blocking tags; `EffectApplied` and `EffectExpired`. **`Ability`** inline or from the `abilities` kind: trigger by action, interval or event; cost in a stat; cooldown scaled by `cooldown_rate`; charges, ammo and reload; targeting self, aim, nearest by filter or random in range; payloads of a prefab, an effect, an impulse or a hitbox window. **`Level`:** an xp curve over a counter; offers from the `offers` kind | C |
| **G, game-facing** | `Pickup` (magnet from `pickup_radius`; collect actions as cues), `WaveSchedule` with the `waves` kind, `DamageNumbers`, `YSort`, `CharacterAnimator` (example 01 hand-writes about 90 lines of it), the presets, the `kit_crowd` row | E |
| **After the freeze, 1.x rows** | `Interactable`, `Checkpoint`, `Switch`, `Gate`, `PressurePlate`, `Portal`, `ForceField`, `Draggable`, `ViewEvents`, `Dormant`, `HitStop`, `Flicker`, `Blink`, `Flash`, `Shake`, `AfterImage`, `CameraRoom`, `BindText`; `Grid<T>`, `Dir4`, `Dir8`, `GridMover`, `Perception`, `Patrol`, A\* and flow field, boids; platformer extras (air jump, wall jump, ledge grab, one-way, slopes); state charts; utility AI | — |

## 8. Template growth rule (F3's text)

"When the game passes about three components or two features, move each feature into `src/features/<name>/`: `mod.rs` holds its `Plugin`, `components.rs` and `systems.rs` the rest. `game.rs` only adds the feature plugins. A component two features share goes in `src/shared.rs`. A feature plugin registers its own prefab components, data kinds and systems, as a kit item does. The kit's scene names and data kinds are reserved: the registries fail startup on a duplicate."

The nested layout is shown by the reworked examples and the acceptance game; `templates/basic` stays basic (`w12-template.md`).

## 9. Ordered steps

1. **Graduation session** (§10), after M59's release.
2. **F1.** Plan session (`tungsten-milestone`), then execution session and release; every CPU row captured (`tungsten-perf`).
3. **F2.**
4. **F3.**
5. **A.**
6. **M.**
7. **C.** Its plan opens with the grid and damage pipeline sketch.
8. **E.** Its plan opens with the `Effect` and `Ability` sketch, the largest API of the program.
9. **G.**
10. **Close.** Fold the catalogue into `w13-kit.md` and the 1.x rows into `backlog-1.x.md`; archive this plan.

W12b follows G, so the reworked examples are the kit's first real users before the freeze. W6b stays where the register has it: tags filter events, and masks later save broadphase work.

## 10. Graduation

The base is new public API before the freeze, so the route is the 1.0 program's (`D-140`, workflow §2). Run graduation as its own session after M59's release (0.79) and before the next plan session. The release commit takes the whole tree (`D-105`), so graduating earlier would put this work in M59's commit.

- **Register:** eight Phase 6 candidates, each level C, none paired (`D-108` pairs only small level A or B candidates):

  | Candidate | Milestone |
  | --- | --- |
  | W13e | F1 |
  | W13f | F2 |
  | W13g | F3 |
  | W13h | A |
  | W13i | M |
  | W13j | C |
  | W13k | E |
  | W13l | G |

  W12b's needs gain W13l.
- **Records:**
  - `implementation-plan.md` §4 and §10, `roadmap.json` and `just repo-check`;
  - `w13-kit.md`'s candidate list;
  - `backlog-1.x.md` rows for the 1.x list;
  - this file moves to `docs/plans/1.0/w13-kit-catalogue.md`.
- **Cost:** eight release cycles, each with a plan critique and a step-1 API critique. A smaller first cut is F1 to F3, then re-decide; the foundations pay off whatever follows.

## 11. Done-when

- **Releases:**
  - Each milestone's release is cut with its plan's checks green.
  - `just api-check` is clean at each release, comparing `api/tungsten-kit.txt` (each item once, at one public path) and `api/tungsten-kit-components.txt`.
- **Coverage:** the coverage test passes, with every scene name and data kind in at least two genres' fixtures. Every kit component and data kind has inspector rows or a description, each checked by a test.
- **Foundations:**
  - Every kit reader uses `EventCursor`; `sound.rs` keeps no handled count.
  - A prefab or data entry naming a missing prefab or data ID fails the manifest load with the file and the field.
  - Editing a data file hot-reloads it for later reads (harness test).
  - A kit-spawned entity leaves with its spawner's state (harness test).
- **No game Rust:**
  - `templates/basic`'s player has no game Rust for movement, health or camera.
  - A top-down recipe counts kills, bursts the enemy and rolls a drop through cues and cue rules alone.
- **Stability:**
  - The kit determinism hash is unchanged at each release, or its change is recorded in that release's decision.
  - The physics hashes (`0x088e_c07a_73c1_b168` and the containment hash) and `DefaultPlugins`' resolved schedule are unchanged at every release.
  - Each per-entity item has a criterion row whose per-entity time does not grow between 1,000 and 10,000 entities (each plan fixes the bound).
- **Tests:** `cargo test -p tungsten-kit`, the umbrella's `kit_tests` and the recipes pass, and `just checks` recommends nothing further.

## 12. Open questions, each with a default

- **Q1. Bodiless overlap.** Settled (2026-10-15): a kit-owned grid; C fixes the details within §2 rule 6.
- **Q2. Scope before the freeze.** Settled: F1 to G; the rest are 1.x rows.
- **Q3. Stat keys.** Default: interned `StatId`s from the `stats` kind or plugin build; the kit predeclares its well-known stats.
- **Q4. Tag declaration.** Default: the `tags` kind or plugin build, both checked at load.
- **Q5. Data tables.** Settled (2026-10-16): the `data` section and `DataRegistry` (F2).
- **Q6. Damage pipeline.** Default: queued requests resolved once a step in sorted order, `damage` as the immediate path over the same resolution. No break.
- **Q7. `EventCursor` placement.** Default: in core beside `EventQueue`, since the umbrella's readers share the problem; it amends `D-040`.
- **Q8. Descriptions and schema.** Default: hand-written descriptions checked by a sample-deserialization test. A JSON Schema export, and any crate for it, waits for a `D-015` entry.
- **Q9. Genre starters.** Default: W12b's reworked examples become starters `tungsten new` can copy (W14b).
- **Q10. Agent bridge.** Default: `tungsten run --script` with a JSON report, proposed to W14b; a live bridge stays 1.x.
- **Q11. This draft in M59's commit.** Default: it stays in the tree uncommitted; the owner may move it aside until M59 is cut.
- **Q12. Effect and ability definitions.** Settled (2026-10-16): data kinds, by ID or inline (§3).
- **Q13. Cue rule order.** Default: entity cues before cue rules, both in event order, each event's actions in written order.

## 13. Decisions expected

- F1: the event cursor (amending `D-040`); spawn inheritance.
- F2: the two-phase load (amending `D-157`); the data registry and the `data` section (extending the manifest's `D-017` and `D-052`).
- F3: the amended admission rule (amending `D-155`); the kit phases (amending `D-158`); tags; the components snapshot in `just api-check` (amending `D-107`).
- A: the stats model; cues.
- C: the overlap grid and the damage pipeline (amending `D-158`'s health).
- E: the effect and ability model.
- One landed decision per milestone, as `D-155` and `D-158` were.

IDs are the next free ones when each is written (`tungsten-decision`).

## 14. Invariants and stop conditions

- **Must not move:**
  - The kit depends on core alone, and core names no kit type.
  - No new third-party dependency (`D-015`).
  - `DefaultPlugins` and the physics hashes.
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

How the base composes once G lands. Field names are placeholders for each milestone's API sketch; the shapes are what the recipes test.

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
  "steering": { "seek": { "nearest": { "all": ["team.player"] } }, "separate": 16, "max_speed": 90 },
  "health": { "max": 6, "on_death": { "despawn": 0.1 } },
  "hurtbox": { "shape": { "circle": 9 } },
  "contact_damage": { "damage": 1, "interval": 0.5, "filter": { "all": ["team.player"] } },
  "drop_table": "slime_drops",
  "cues": { "killed": [{ "prefab": "slime_burst" }, { "sound": "pop" }] }
} }

// top_down/data/cue_rules/combat.json: every enemy kill counts, with no field on each prefab
[{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1 } }] }]

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

## Evidence log

Empty.

## Follow-ups

- **Core candidates the catalogue points at, none started:**
  - Per-tile gameplay properties.
  - Physics features: friction, sensor exit and stay, approach speed.
  - A JSON Schema export (Q8).
  - Input record and replay (`D-129` left it open).
  - `tungsten run --script` (Q10).
  - A `spawn <prefab>` debug console command.
- **Review records:** this plan's findings (§1) go into F1's audit with the commit they were read at.
