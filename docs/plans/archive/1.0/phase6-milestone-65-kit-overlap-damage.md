# Phase 6 milestone 65: kit overlap and damage (W13j) — done

- **status:** done
- **goal:** The kit catalogue's C ([w13-kit](../../1.0/w13-kit.md) §7): hit shapes on a kit-owned grid, each hit tested over the step's relative motion so that a fast bolt skips nothing; one damage pipeline that hitboxes, contact damage, projectiles and a game's Rust share, whose requests carry a source, tags, a position, a knockback and a crit flag and resolve once a step in sorted order through resistances, a shield and health, with heals and a death policy; contact damage, projectiles and pickups with a magnet; drops rolled from the `loot_tables` kind by the `roll` cue action. Each item composes in data and passes recipes in two genres.
- **non-goals:** W13k's effects and abilities (the `effect` action, a hitbox's `apply`, hitbox windows, periodic damage and regeneration, the `area` and `projectile_speed` stats); a `heal` cue action and a `spawned` cue event; layers, masks or any other core or physics change (W6b); returning projectiles, drop scatter, crit chances and HUD text (1.x); zones and the nearest search on the grid; the examples (W12b); the acceptance game.
- **files to touch:**
  - Step 1: `crates/tungsten-kit/src/damage/{health.rs,mod.rs}` and new `damage/{pipeline.rs,defense.rs}`; new `crates/tungsten-kit/src/lifecycle/ended.rs` and `lifecycle/{mod.rs,lifetime.rs,despawn.rs}`; new `crates/tungsten-kit/src/overlap/{hit.rs,contact.rs,projectile.rs,pickup.rs}` and `overlap/mod.rs`; new `crates/tungsten-kit/src/react/loot.rs` and `react/{cues.rs,mod.rs}`; `crates/tungsten-kit/src/foundation/{describe.rs,stats.rs,plugins.rs,phases.rs,rng.rs}`; `crates/tungsten-kit/src/lib.rs`; `crates/tungsten-kit/src/tests/{health.rs,describe.rs,cues.rs,stats.rs,lifetime.rs,despawn.rs}` and new `tests/{pipeline.rs,defense.rs,ended.rs,hit.rs,contact.rs,projectile.rs,pickup.rs,loot.rs}`; `crates/tungsten/src/tests/kit.rs`; `crates/tungsten/src/tests/kit_recipes/coverage.rs`; the four fixture folders under `crates/tungsten/tests/fixtures/kit/` (new prefabs, new `data/loot_tables/` entries, `data/tags.json`, `manifest.json`); `templates/basic/tests/game.rs`; `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`
  - Step 2: new `crates/tungsten-kit/src/overlap/grid.rs`; `overlap/{hit.rs,contact.rs,mod.rs}`; `crates/tungsten-kit/src/foundation/debug.rs`; `crates/tungsten-kit/src/tests/{hit.rs,contact.rs,debug.rs}` and new `tests/grid.rs`; `crates/tungsten/src/tests/kit.rs`
  - Step 3: `crates/tungsten-kit/src/overlap/{projectile.rs,grid.rs}`; `crates/tungsten-kit/src/tests/{projectile.rs,grid.rs}`; `crates/tungsten/src/tests/kit.rs`
  - Step 4: `crates/tungsten-kit/src/overlap/pickup.rs`; `crates/tungsten-kit/src/tests/pickup.rs`; `crates/tungsten/src/tests/kit.rs`
  - Step 5: `crates/tungsten/src/tests/kit_recipes/{mod.rs,top_down.rs,platformer.rs,shmup.rs,grid.rs,determinism.rs}`; the four fixture folders; `crates/tungsten-kit/src/tests/phases.rs`
  - Step 6: `crates/tungsten-kit/benches/kit.rs`; new `perf-runs/<YYYYMMDD>-m65-kit-criterion/summary.txt`; `docs/perf/profiling-workflow.md`
  - Step 7: `templates/basic/assets/{manifest.json,prefabs/hazard.json,prefabs/player.json}` and new `prefabs/gem.json`; `templates/basic/src/{game.rs,components.rs,states.rs}`; `templates/basic/tests/game.rs`; `templates/basic/AGENTS.md`; `docs/getting-started.md`
  - Step 8: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/LLM_INDEX.md`, `CHANGELOG.md`; `api/` when a later step changed surface
  - Step 9: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row in `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`'s four owned Now fields, this plan (moved to `docs/plans/archive/1.0/`)
- **ordered steps:** (1) The API, the damage pipeline and the `roll` action, then the step 1 API critique; (2) the hit grid, hitboxes and contact damage; (3) projectiles; (4) pickups; (5) recipes in the genres and the kit hash; (6) the kit bench's rows and their sitting; (7) the template's hazard on contact damage and its pickup; (8) records; (9) Release 0.85.
- **done-when:** every step's evidence row quoted; workflow §5 closed; the release checks, with `just physics-release`, which no step runs, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W13j ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M65, release 0.85. Written 2026-10-17 at `73043fe`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- **Why now.** C follows M in the catalogue's order (F1, F2, F3, A, M, C, E; w13-kit §7, §9). It stands on A's cues, counters, stats and `KitRng` (`D-173`) and on M's movers, `Steering` for homing and `knock_back` (`D-174`). W13k's effects queue damage and heals through this pipeline, and its abilities spawn these projectiles and hitboxes.
- **What it stands on.** `D-158`'s `Health`, `damage` and `Damaged`, which carry no source, kind or hook (w13-kit R3); `D-170`'s spawn inheritance, so a drop and a bolt carry their spawner's `SceneEntity` and `Source`; `D-171`'s data kinds and `DataRef`; `D-172`'s phases, tags, `Source`, descriptions, recipes and kit hash; `D-154`'s casts and overlaps over the last step.
- **What the catalogue fixed.** Bodiless overlap is a kit grid (Q1). A hit shape is the kit's own component, never a `Collider` (R4). `damage` stays the immediate path over the queued pipeline, with no break (Q6). Drops are the `roll` action, not a component (Q15). The death policy despawns after the `killed` cues (P2, §3). Heals and a crit flag go through the pipeline (P6). Composition beats options (§2 rule 16): an aura is `contact_damage` with a shape, a homing bolt is `projectile` with `steering`.
- **Where this plan departs from the card**, each with its question: `Damaged` stays `Copy` and gains no tags (Q1); walls are tiles and static colliders found without a core change (Q2); walls end projectiles only (Q3).

## 1. Audit

Read at `73043fe`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `damage(world, target, amount)` acts at once. An amount of 0, no `Health`, a dead target or an open window is `Hit::Ignored`; otherwise health falls, the window and flash start, and `Damaged { entity, amount, killed }` goes out when the queue is registered. Nothing carries a source, kind, position or knockback (R3). | `crates/tungsten-kit/src/damage/health.rs:263-312` |
| A2 | `Damaged` derives `Copy` and `PartialEq`, is `#[non_exhaustive]` and is built by `new(entity, amount, killed)`. Tests and the template compare sent events with `Damaged::new(..)` and read them with `.copied()`. `Tags` holds a `Vec<String>`, so a `Tags` field would end `Copy`, a compile break. | `health.rs:236-261`, `:65`; `crates/tungsten-kit/src/foundation/tags.rs:84-85`; `crates/tungsten/src/tests/kit.rs:1290`; `crates/tungsten/src/tests/kit_recipes/top_down.rs:58-59`; `templates/basic/tests/game.rs:293-294` |
| A3 | `Hit` is `#[non_exhaustive]`: `Ignored`, `Hurt`, `Killed`. | `health.rs:222-234` |
| A4 | `Health` saves through `HealthFields` (`current`, `max`, `invulnerability`, `window_left`, unknown fields refused); the scene DTO takes `max` and `invulnerability`. A dead entity stays until something despawns it. | `health.rs:77-129`, `:181-185`, `:569-577` |
| A5 | `HealthPlugin` registers `Damaged`, `health`, `hit_flash`, `health_tick` in `Tick` and `hit_flash_tick` in `Present`; its unit test counts nine systems. `health_tick` only counts windows down. | `health.rs:474-567`, `:597-605`; `crates/tungsten-kit/src/tests/health.rs:67` |
| A6 | The cue system's kit events are four, each recording whether it carries an amount and another entity: `damaged` and `killed` (an amount, no other), `zone_entered` (an other), `cooldown_elapsed`. A `damaged` or `killed` occurrence takes its subject's point when the cue system reads it. | `crates/tungsten-kit/src/react/cues.rs:42-47`, `:792-810`, `:1452-1473`, `:1503-1512` |
| A7 | Nine actions, no `roll`. `prefab` prepares at the occurrence's point and queues `spawn_prefab_from(subject)`; the load checks prefab and sound IDs, and the amount or other entity an action needs. | `cues.rs:229-250`, `:474-484`, `:1024-1069`, `:1305-1331` |
| A8 | A cue `despawn` marks its target `Despawning` and queues it after the step's spawns; an occurrence whose subject is gone or marked is skipped. | `cues.rs:1170-1175`, `:1214-1228`, `:1252-1257`, `:1379-1390` |
| A9 | A spawn queued from a source that an earlier command of the frame despawned is dropped at the flush, so an item that despawns its own entity must queue that despawn after the entity's cue spawns. A second recursive despawn of a gone entity does nothing. | `D-170`; `docs/known-issues.md:52-53`; `crates/tungsten-kit/src/transform/hierarchy.rs:281-284` |
| A10 | `cues`' description has one field per kit event, which its round trip fills. The description test counts 32 components, 3 kinds and 14 shapes; the coverage test counts 32 names and 3 kinds. | `cues.rs:1514-1557`; `crates/tungsten-kit/src/tests/describe.rs:122-124`; `crates/tungsten/src/tests/kit_recipes/coverage.rs:81-82` |
| A11 | `FieldType` is `#[non_exhaustive]`, with `Prefab`, `Tag`, `Filter`, `Object` and `Shape` and no data-ID form. No kit item holds a `DataRef` yet; its `check` is the load check and its `resolve` the read. | `crates/tungsten-kit/src/foundation/describe.rs:252-300`; `crates/tungsten-core/src/assets/data.rs:377-459` |
| A12 | `kit_overlap` runs after `physics_step`; `kit_resolve` holds nothing yet ("damage and effects (later milestones)"). `Phase::join` orders a member after its anchor and before the next. | `crates/tungsten-kit/src/foundation/phases.rs:43-51`, `:148-167` |
| A13 | The physics history copies each `Position` into its `PrevPosition` first in `fixed_update`, so in `kit_overlap` a root's step motion is `PrevPosition → Position`. `wrap` runs in `kit_overlap` and writes `PrevPosition` with `Position`, which erases that step's motion. | `crates/tungsten-core/src/physics/plugin.rs:55-68`; `crates/tungsten-kit/src/motion/wrap.rs:45` |
| A14 | The gather makes every `Collider` on a `Position` a physics proxy, static without a `RigidBody` (R4). | `crates/tungsten-core/src/physics/step/gather.rs:59-65` |
| A15 | Queries read the last step's index. `with_bodies` covers static, kinematic and dynamic bodies and bare colliders together, and an index entry records no body kind, so no filter picks static geometry alone. `cast_shape` returns the first body or tile met, and a shape that starts overlapping one while moving into it hits at distance 0. `overlap` lists every body and tile over a shape, a tile as `None`. | `crates/tungsten-core/src/physics/query.rs:60-131`, `:150-161`, `:347-389`, `:391-425` |
| A16 | The fixtures' enemies, heroes and bolts are kinematic bodies with colliders, and their walls static bodies; no fixture loads a tilemap. A cast over bodies would stop a bolt at the first enemy's collider. | `crates/tungsten/tests/fixtures/kit/top_down/prefabs/{slime,hero,bolt,wall}.json`; the four fixture `manifest.json` files |
| A17 | The movers' helpers: `body_of` (child, bodiless, static, kinematic, dynamic, unplaced), and `step_position` and `write_velocity` by `Write::Set` or `Write::Add`. `LinearMotion::set_velocity` is public; `Steering::velocity_mut` is crate-private. | `crates/tungsten-kit/src/motion/drive.rs:28-61`, `:106-133`; `motion/linear.rs:161`; `motion/steering.rs:305` |
| A18 | `knock_back(world, entity, velocity) -> bool` refuses a dead target; the template calls it on `Hit::Hurt`. | `crates/tungsten-kit/src/motion/knockback.rs:223-253`; `templates/basic/src/game.rs:74-98` |
| A19 | `stats_apply` writes `move_speed`, `max_health` and `cooldown_rate` through a six-column query. `DAMAGE`, `PICKUP_RADIUS` and `PROJECTILE_SPEED` say they are read "from the damage pipeline on", "from the pickups on" and "from the projectiles on". | `crates/tungsten-kit/src/foundation/stats.rs:243-258`, `:726-756` |
| A20 | `Source` is the spawn chain's origin, derived at the flush. Only `CooldownPlugin` inserts `KitRng`. | `crates/tungsten-kit/src/foundation/source.rs`; `crates/tungsten-kit/src/lifecycle/cooldown.rs:359` |
| A21 | `Lifetime` and `despawn_outside` queue `despawn_recursive` at once. `TraceKind` is `#[non_exhaustive]`: `Event`, `Cue`, `Skipped`, `Spawn`, `SpawnFailed`, `Despawn`. | `crates/tungsten-kit/src/lifecycle/lifetime.rs:150-164`; `crates/tungsten-kit/src/foundation/trace.rs:155-210` |
| A22 | `KitPlugins::set()` builds `HealthPlugin` eighth and ends with M's plugins, then `KitDebugPlugin`. | `crates/tungsten-kit/src/foundation/plugins.rs:79-113` |
| A23 | The umbrella pins the kit's resolved `fixed_update` and `post_update`; the template pins its schedule, with `hazard_touch` in `kit_overlap`. | `crates/tungsten/src/tests/kit.rs:37-41`; `templates/basic/tests/game.rs:121-131` |
| A24 | The kit hash is `0xd651_23fe_e583_03ee`: each genre's fixed script for 300 frames at 1/60 s, with `damage` at fixed frames, hashing each entity's point, health, tags, counters, flags, stats, rotation, depth and clip. No fixture's `damaged` or `killed` cue spawns a prefab or targets another entity. | `crates/tungsten/src/tests/kit_recipes/determinism.rs:23`, `:56-115`, `:117-256`; `top_down/prefabs/brute.json`; `grid/prefabs/slug.json`; the two `cue_rules` files |
| A25 | Recipes run at dt 1/60 and 1/30; the hit-sequence test ran at 1/60, 1/30 and 1/144; the harness runs at most two fixed steps a frame. | `crates/tungsten/src/tests/kit_recipes/mod.rs:29-30`; `crates/tungsten/src/tests/kit.rs:1297-1320` |
| A26 | The kit bench has 23 rows at 1,000 and 10,000 entities, each per-entity median at 10,000 at most twice its per-entity median at 1,000; M64's run read ratios of 0.56–1.09. | `crates/tungsten-kit/benches/kit.rs:1-18`; `perf-runs/20261017-m64-kit-criterion/summary.txt` |
| A27 | The template's hazard hurts through `hazard_touch` in `Phase::Overlap` over its `Hazard { radius, damage, push }`; the template's tests put the player on the hazard and walk it into the patrol. | `templates/basic/src/game.rs:37-60`, `:74-98`; `templates/basic/src/components.rs:11-54`; `templates/basic/tests/game.rs:329-546` |
| A28 | Known issues wait on this milestone: a bodiless entity enters no zone "until the kit grid of W13j", the nearest search waits "for C's grid", and the template's hazard hurts through Rust "until C's contact damage". | `docs/known-issues.md:54-55` |
| A29 | `docs/LLM_INDEX.md` holds 12,282 of its 12,288 bytes; its kit row routes the whole kit folder, citing `D-172`–`D-174`. | `docs/LLM_INDEX.md:14`; `wc -c` |
| A30 | An event sent while the clock is paused would be lost: a flush that does not hold drops what was held. | `crates/tungsten-core/src/ecs/event_queue.rs:32-43` |

## 2. Design

### 2.1 What lands

| Item | Scene name or kind | Plugin | Systems, by phase |
| --- | --- | --- | --- |
| Hit shapes | `hitbox`, `hurtbox` | `HitPlugin` | `Overlap`: `hit_grid`, `hitbox_tick` |
| Contact damage | `contact_damage` | `ContactDamagePlugin` | `Overlap`: `contact_damage` |
| Projectile | `projectile` | `ProjectilePlugin` | `Overlap`: `projectile_tick` |
| Pickup | `pickup` | `PickupPlugin` | `Move`: `pickup_magnet`; `Overlap`: `pickup_tick` |
| Resistances, shield | `resist`, `shield` | `HealthPlugin` | `Tick`: `shield_tick` |
| Requests, heals, the death policy | `health`'s `despawn_on_death` | `HealthPlugin` | `Resolve`: `damage_resolve`; `Tick`: `health_tick` counts death delays; `React`: `kit_despawn` |
| Drops | the `loot_tables` kind, the `roll` action | `CuesPlugin` | `React`: `cues` |

### 2.2 API sketch

Every type keeps private fields behind constructors that panic outside the data's bounds (`#[track_caller]`), every new public item has rustdoc with an example, and each is data where its state is data. A hitbox's and a contact damage's per-target lists hold entities, so they are not saved: a loaded one starts a fresh activation.

```rust
// overlap/hit.rs
pub const HIT_GRID: &str = "hit_grid";
pub const HITBOX_TICK: &str = "hitbox_tick";

/// A circle or a box; each item places it at its entity's point plus the item's offset.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HitShape { /* core `Shape` */ }
impl HitShape {
    pub fn circle(radius: f32) -> Self;             // finite, in (0, 16384], as `rigid_body`'s
    pub fn aabb(half_extents: Vec2) -> Self;        // each axis finite, in (0, 16384]
    pub fn shape(&self) -> Shape;
}

/// Hits the hurtboxes its shape meets over each step, each target once per activation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hitbox { /* shape, offset, damage, tags, filter, knockback; the targets hit, not saved */ }
impl Hitbox {
    pub fn new(shape: HitShape, damage: f32) -> Self; // damage finite, >= 0
    pub fn with_offset(self, offset: Vec2) -> Self;   // finite, each axis within ±16384
    pub fn with_tags(self, tags: Tags) -> Self;       // the damage's kinds: `damage.fire`
    pub fn with_filter(self, filter: TagFilter) -> Self;
    pub fn with_knockback(self, speed: f32) -> Self;  // finite, >= 0, world px per second
    pub fn shape(&self) -> HitShape;
    pub fn offset(&self) -> Vec2;
    pub fn damage(&self) -> f32;
    pub fn tags(&self) -> &Tags;
    pub fn filter(&self) -> &TagFilter;
    pub fn knockback(&self) -> f32;
    pub fn has_hit(&self, target: Entity) -> bool;
    pub fn rearm(&mut self);                          // a new activation: every target again
}

/// What hitboxes, contact damage and pickups meet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hurtbox { /* shape, offset */ }
impl Hurtbox {
    pub fn new(shape: HitShape) -> Self;
    pub fn with_offset(self, offset: Vec2) -> Self;
    pub fn shape(&self) -> HitShape;
    pub fn offset(&self) -> Vec2;
}

/// A hitbox, a projectile or a contact damage met a target: the cue event `hit`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Struck { pub entity: Entity, pub target: Entity, pub position: Vec2, pub amount: f32 }
impl Struck { pub fn new(entity: Entity, target: Entity, position: Vec2, amount: f32) -> Self; }

#[derive(Debug, Default, Clone, Copy)]
pub struct HitPlugin;

// overlap/contact.rs
pub const CONTACT_DAMAGE: &str = "contact_damage";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContactDamage { /* damage, interval, shape and offset, tags, filter, knockback; per-target times, not saved */ }
impl ContactDamage {
    pub fn new(damage: f32) -> Self;                  // every 0.5 s, the entity's hurtbox and its offset
    pub fn with_interval(self, seconds: f32) -> Self; // finite, > 0
    pub fn with_shape(self, shape: HitShape, offset: Vec2) -> Self; // an aura: its own shape
    pub fn with_tags(self, tags: Tags) -> Self;
    pub fn with_filter(self, filter: TagFilter) -> Self;
    pub fn with_knockback(self, speed: f32) -> Self;
    // an accessor per field
}
#[derive(Debug, Default, Clone, Copy)]
pub struct ContactDamagePlugin;

// overlap/projectile.rs
pub const PROJECTILE_TICK: &str = "projectile_tick";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Projectile { /* pierce, range, bounces; hits, travelled, bounced, spent */ }
impl Projectile {
    pub fn new() -> Self;                             // ends at its first hit or wall, no range
    pub fn with_pierce(self, targets: u32) -> Self;   // passes this many, ends at the next
    pub fn with_range(self, distance: f32) -> Self;   // finite, > 0, world px
    pub fn with_bounces(self, walls: u32) -> Self;
    pub fn pierce(&self) -> u32;
    pub fn range(&self) -> Option<f32>;
    pub fn bounces(&self) -> u32;
    pub fn hits(&self) -> u32;
    pub fn travelled(&self) -> f32;
    pub fn is_spent(&self) -> bool;
}
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ProjectileExpired { pub entity: Entity, pub position: Vec2 }
impl ProjectileExpired { pub fn new(entity: Entity, position: Vec2) -> Self; }
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectilePlugin;

// overlap/pickup.rs
pub const PICKUP_TICK: &str = "pickup_tick";
pub const PICKUP_MAGNET: &str = "pickup_magnet";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pickup { /* shape, offset, filter, magnet; collected */ }
impl Pickup {
    pub fn new(shape: HitShape) -> Self;              // any collector, no magnet
    pub fn with_offset(self, offset: Vec2) -> Self;
    pub fn with_filter(self, filter: TagFilter) -> Self;
    pub fn with_magnet(self, speed: f32) -> Self;     // finite, >= 0, world px per second; 0 none
    pub fn shape(&self) -> HitShape;
    pub fn offset(&self) -> Vec2;
    pub fn filter(&self) -> &TagFilter;
    pub fn magnet(&self) -> f32;
    pub fn is_collected(&self) -> bool;
}
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PickedUp { pub entity: Entity, pub by: Entity, pub position: Vec2 }
impl PickedUp { pub fn new(entity: Entity, by: Entity, position: Vec2) -> Self; }
#[derive(Debug, Default, Clone, Copy)]
pub struct PickupPlugin;

// damage/pipeline.rs
pub const DAMAGE_RESOLVE: &str = "damage_resolve";
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRequest { /* target, amount, source, tags, position, knockback, crit */ }
impl DamageRequest {
    pub fn new(target: Entity, amount: f32) -> Self;      // amount finite, >= 0
    pub fn with_source(self, source: Entity) -> Self;
    pub fn with_tags(self, tags: Tags) -> Self;
    pub fn with_position(self, position: Vec2) -> Self;   // finite
    pub fn with_knockback(self, velocity: Vec2) -> Self;  // finite
    pub fn with_crit(self, crit: bool) -> Self;
    pub fn target(&self) -> Entity;
    pub fn amount(&self) -> f32;
    pub fn source(&self) -> Option<Entity>;
    pub fn tags(&self) -> &Tags;
    pub fn position(&self) -> Option<Vec2>;
    pub fn knockback(&self) -> Option<Vec2>;
    pub fn is_crit(&self) -> bool;
    pub fn apply(self, world: &mut World) -> Hit;          // resolved now
    pub fn queue(self, world: &mut World) -> bool;         // for this or the next step's `damage_resolve`;
                                                           // false without `HealthPlugin`'s queue
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealRequest { /* target, amount, source */ }
impl HealRequest {
    pub fn new(target: Entity, amount: f32) -> Self;      // amount finite, >= 0
    pub fn with_source(self, source: Entity) -> Self;
    pub fn target(&self) -> Entity;
    pub fn amount(&self) -> f32;
    pub fn source(&self) -> Option<Entity>;
    pub fn apply(self, world: &mut World) -> f32;          // the health restored
    pub fn queue(self, world: &mut World) -> bool;
}
pub fn heal(world: &mut World, target: Entity, amount: f32) -> f32; // `HealRequest::new(..).apply(world)`
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Healed { pub entity: Entity, pub amount: f32, pub source: Option<Entity> }
impl Healed {
    pub fn new(entity: Entity, amount: f32) -> Self;
    pub fn with_source(self, source: Entity) -> Self;
}

// damage/defense.rs
pub const SHIELD_TICK: &str = "shield_tick";
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resist { /* (damage tag, factor), sorted by tag */ }
impl Resist {
    pub fn new() -> Self;
    pub fn with_factor(self, tag: &str, factor: f32) -> Self; // a tag name; finite, in [0, 1000]; 0 is immunity
    pub fn factor(&self, tags: &Tags) -> f32;                 // the product over the listed tags `tags` has
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shield { /* current, max, recharge, delay, time since its last hit */ }
impl Shield {
    pub fn new(max: f32) -> Self;                                   // full; finite, > 0
    pub fn with_recharge(self, per_second: f32, delay: f32) -> Self; // each finite, >= 0
    pub fn current(&self) -> f32;
    pub fn max(&self) -> f32;
    pub fn restore(&mut self, amount: f32);                         // up to its max
}

// damage/health.rs: additions
pub enum Hit { Ignored, Hurt, Killed, Absorbed }  // `Absorbed`: the shield took it all
pub struct Damaged {
    pub entity: Entity, pub amount: f32, pub killed: bool,
    pub source: Option<Entity>, pub position: Option<Vec2>, pub crit: bool, // new; `new` gives None, None, false
}
impl Damaged {
    pub fn with_source(self, source: Entity) -> Self;
    pub fn with_position(self, position: Vec2) -> Self;
    pub fn with_crit(self, crit: bool) -> Self;
}
impl Health {
    pub fn with_despawn_on_death(self, seconds: f32) -> Self;  // finite, >= 0; kept when not set
    pub fn despawn_on_death(&self) -> Option<f32>;
}
pub fn damage(world: &mut World, target: Entity, amount: f32) -> Hit; // `DamageRequest::new(..).apply(world)`

// lifecycle/ended.rs
pub const KIT_DESPAWN: &str = "kit_despawn";

// react/loot.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightedTable { /* rolls; entries of a prefab or nothing, with a weight */ }
impl WeightedTable {
    pub fn new() -> Self;                                                   // one roll, no entries
    pub fn with_entry(self, prefab: impl Into<String>, weight: f32) -> Self; // weight finite, > 0
    pub fn with_nothing(self, weight: f32) -> Self;
    pub fn with_rolls(self, rolls: u32) -> Self;                            // 1..=64
    pub fn rolls(&self) -> u32;
    pub fn draw(&self, rng: &mut KitRng) -> Option<&str>;                   // one roll's prefab, or none
}

// react/cues.rs
impl CueAction { pub fn roll(table: DataRef<WeightedTable>) -> Self; }  // name "roll"

// foundation/describe.rs
pub enum FieldType { /* … */ Data(&'static str) }  // an entry ID of this data kind, or the entry in place
```

The data forms:

```jsonc
"hurtbox": { "shape": { "circle": 8 } }
"hitbox": { "shape": { "aabb": [6, 3] }, "offset": [8, 0], "damage": 2,
            "tags": ["damage.fire"], "filter": { "all": ["team.enemy"] }, "knockback": 300 }
"contact_damage": { "damage": 1, "interval": 0.5, "filter": { "all": ["team.player"] }, "knockback": 420,
                    "shape": { "circle": 28 } }  // an aura; the hurtbox's shape and offset when left out
"projectile": { "pierce": 1, "range": 600, "bounces": 0 }  // each optional
"pickup": { "shape": { "circle": 6 }, "filter": { "all": ["team.player"] }, "magnet": 240 }
"resist": { "damage.fire": 0.5, "damage.poison": 0 }
"shield": { "max": 3, "recharge": 1, "delay": 2 }
"health": { "max": 6, "invulnerability": 0.2, "despawn_on_death": 0.1 }
// data/loot_tables/slime_drops.json: an entry without `prefab` draws nothing
{ "rolls": 1, "entries": [{ "prefab": "gem", "weight": 3 }, { "weight": 1 }] }
"cues": { "killed": [{ "roll": "slime_drops" }, { "prefab": "slime_burst" }],
          "hit": [{ "prefab": "spark" }], "expired": [{ "prefab": "puff" }],
          "picked_up": [{ "counter": { "name": "xp", "add": 1, "target": "other" } }] }
```

A hit shape is written `{ "circle": r }` or `{ "aabb": [x, y] }`, the catalogue's form: a one-of shape, so the descriptions describe it and their test probes each form alone (`crates/tungsten-kit/src/tests/describe.rs:198-231`), where `rigid_body`'s `kind`-tagged form would fail the variant probe, which sets each `kind` in the sample and keeps the rest (`:57-67`, `:234-261`). The offset sits beside the shape, as `rigid_body`'s does, defaulting to `[0, 0]`; `contact_damage` refuses an `offset` without its own `shape`. Every DTO is private, refuses unknown fields and checks its numbers at deserialization; the load checks every tag, filter, prefab and loot table ID and names its field as written (`hitbox.filter.all.0`, `killed.0.roll.entries.1.prefab`).

### 2.3 The grid and the hit test

- **Points and sweeps.** A shape's centre is its entity's point plus its item's offset. A root with a `Position` sweeps from its `PrevPosition` to its `Position`, the step's motion, gameplay's and the solver's (A13). One without a `PrevPosition` stands at its `Position`. An entity without a `Position`, or a child, stands at its `Transform.position`, for a child the last frame's propagation (Q15). Only `pickup`'s scene component spawns `Position` and `PrevPosition` at the spawn point, as M's movers do, since its magnet moves it; a hit shape takes the point its entity has, so a sword on a child stays the hierarchy's.
- **The grid.** `hit_grid`, first of C's overlap systems, gathers each hurtbox whose entity is alive, not dead by its `Health` and not marked `Despawning`, with its swept bounds (its shape's box at both ends), into 64-px cells (Q12): a list of (cell, entry) pairs sorted in kept storage and rebuilt each step. A query walks the cells that a sweep's bounds cover by binary search and takes its candidates in entry order, each once. Cell work is bounded, as core's index bounds its own (`query.rs:253-274`): an entry whose swept bounds span more than 64 cells goes on an oversized list that every query also tests, and a query whose bounds, clamped to the occupied ones, span more cells than `max(4 × entries, 64)` tests every entry instead, so a huge shape or a long sweep costs a scan, never a cell walk, and is never missed.
- **The test.** For a moving shape A from a0 to a1 and a hurtbox B from b0 to b1, let p = a0 − b0 and d = (a1 − a0) − (b1 − b0). They meet at the least t in [0, 1] at which A at p + t·d overlaps B at the origin with penetration above 0, as `overlap` counts it (A15): a ray against their Minkowski sum (a circle of r_a + r_b; a box of h_a + h_b; for a circle and a box, the box grown by r with round corners), and t = 0 when they start overlapping. Two shapes that cross between their endpoints meet, and a bolt faster than a hurtbox's width per step meets it, exactly and with no substeps.
- **Order and position.** A shape's candidates sort by (t, target index): sweep order. At a hit, `Struck.position` is the hitting shape's centre at t and the request's position the target's centre at t, so a spark lands at the blow and a label over the target. Hitting entities run in entity order.
- **Who is hit.** A hurtbox whose entity is not the hitting entity, not the entity it was spawned from and not its `Source`, passes its filter by its `Tags` and has not been hit in this activation. `Source` is the spawn chain's origin (`crates/tungsten-kit/src/foundation/source.rs:77-87`), so for a nest's turret's bolt it names the nest; the shooter is the bolt's emitter, a crate-private `Emitter(Entity)` that `HitPlugin`, `ContactDamagePlugin` and `ProjectilePlugin` derive at the flush as the spawn's own source (`World::derive_from_source`, `world.rs:174-188`), so a shot spawned on its shooter's hurtbox spares it and the chain's origin. An activation is a hitbox's life until `rearm`, a projectile's life, and for contact damage each target's interval.
- **What a hit does.** A hitting entity that is gone or marked `Despawning` does nothing (§2.7). A hit sends `Struck` and, when its damage is above 0, queues a `DamageRequest` for `damage_resolve`, so a 0-damage hitbox is a trigger whose `hit` cues run: the shape's damage and tags; as source the hitting entity's `Source`, else the entity itself; the target's centre; a knockback of `speed × unit(target centre − hitter centre)`, else along d, else none.

### 2.4 Walls (projectiles only)

A projectile's sweep from a0 to a1 ends at its first wall (Q2, Q3):

- tile layers, by `cast_shape(world, shape, a0, a1 − a0, |a1 − a0|, &SpatialFilter::new().with_bodies(false))` (`query.rs:357`), at t = distance / |a1 − a0|;
- static colliders, a `Collider` on a `Position` with a static `RigidBody` or none (A14), found by `overlap` over the sweep's bounds with tiles off and the projectile itself excluded (`query.rs:396`), each met by §2.3's test against the collider at its `Position` plus its offset, keeping the face normal at contact.

Kinematic and dynamic bodies are never walls: their hurtboxes are what a projectile hits (A16). Hits after the wall's t are dropped. A zero-length sweep casts nothing.

### 2.5 Projectiles

Each step, `projectile_tick` takes the live projectiles in entity order; a dynamic body or a child is skipped with one warning, as M's movers skip them (`D-174`):

1. t_end is the least of 1, the wall's t and the range's t, (range − travelled) / |a1 − a0|.
2. Its `Hitbox`'s candidates up to t_end are hit in sweep order as §2.3 says; the hit after `pierce` of them ends it at that hit's t.
3. Otherwise, at a wall with bounces left it is placed at the contact point and each velocity its movers keep is reflected about the wall's normal (`LinearMotion`'s, `Steering`'s, its body's `Velocity`), dropping the rest of the step's travel (Q11); with none left it ends there, and at its range it ends there.
4. `travelled` grows by the distance covered. An end sends `ProjectileExpired` at its point, places it there, zeroes its `LinearMotion`'s velocity and acceleration, its `Steering`'s velocity and its body's `Velocity`, marks it spent and lists it for `kit_despawn`. A plain bolt then stands where it ended until the flush; a homer's steering moves it from rest for the rest of its frame's steps, a recorded limit.

`hitbox_tick` skips an entity with a `Projectile`, and a spent projectile does nothing more.

### 2.6 Contact damage and pickups

- **Contact damage.** `contact_damage` tests each entity's shape (its own, else its `Hurtbox`'s; with neither it warns once) by §2.3 against the grid, its own entity excluded. A target met is hit at once, then again each `interval` of the game seconds the item has stepped while it is still met; its last-hit times are kept sorted by entity, and a target met by none of the item's steps for a whole interval is dropped from the list.
- **Pickups.** `pickup_tick` tests each uncollected pickup's shape, swept by its own motion, against the hurtboxes whose `Tags` pass its filter. The first in sweep order collects it: it is marked collected, `PickedUp { entity, by, position }` goes out at the pickup's centre at t, and it is listed for `kit_despawn`.
- **The magnet.** `pickup_magnet` (`Move`) moves each uncollected pickup with a magnet speed toward the nearest collector: an entity that `pickup_tick` could collect it for (a hurtbox entity alive, not dead by its `Health`, not marked `Despawning`, its `Tags` passing the filter) whose `Stats` hold a `pickup_radius` reaching the pickup's centre, ties to the lower index, so a corpse kept by the default death policy draws nothing. It moves by `min(speed·dt, distance)`, as an increment to a bodiless or static pickup's `Position` after the oscillator's (`Write::Add`, `D-174`); a body or a child is skipped with one warning. Each run bins its eligible collectors, for each distinct filter, into cells as wide as the largest `pickup_radius` among them, and each pickup scans the 3×3 cells about its point, so the cost follows the collectors near a pickup, not all of them (unlike `Target`'s scan, seekers times matches, `crates/tungsten-kit/src/motion/target.rs:171-177`).

### 2.7 The damage pipeline

Resolution, by `DamageRequest::apply` and by `damage_resolve` for each queued request:

1. `Hit::Ignored`, with no side effect, when the amount is 0 (as `damage` is today, `health.rs:281-283`) or the target is gone, marked `Despawning`, without `Health`, dead or inside its window.
2. Its `Resist`: a listed tag that the request's `Tags` have, ancestors included, with a factor of 0 is immunity, `Ignored` (Q4), checked before any product; otherwise the matching factors multiply the amount in `f64`, and the result is clamped to `f32::MAX`, so every amount stays finite (as `D-173`'s stats do); a result of 0 is `Ignored` as in 1. A factor is finite in [0, 1000].
3. Its `Shield` takes `min(current, amount)` and waits its delay again before it recharges.
4. Health falls by the rest, to no less than 0; the window and the flash start; `Damaged` goes out when the queue is registered, with the amount after resistances, whether it killed, and the request's source, position and crit; the trace records `damaged`, and `killed` on a kill.
5. The outcome is `Killed` (health reached 0), `Hurt` (it fell and some is left) or `Absorbed` (the shield took it all).
6. On `Hurt` or `Absorbed`, the request's knockback goes through `knock_back` (A18).
7. A kill of a `Health` with `despawn_on_death` starts its delay. `health_tick` counts it down from the step after the kill and lists the entity for `kit_despawn` once it has passed, the step after the kill for 0. A kill can land anywhere, after `cues` in a step or outside the fixed step through `damage`, and the cue system reads it at the latest in the next step's `cues`, which run before that step's `kit_despawn`, so `killed` cues always run first.

`damage(world, target, amount)` is `DamageRequest::new(target, amount).apply(world)`. For an entity without `Resist`, `Shield` or a despawn it gives today's outcome and today's `Damaged` (`source` and `position` `None`, `crit` false), so `Damaged::new(entity, amount, killed)` still compares equal (A2).

- **Heals.** `HealRequest::apply` restores `min(amount, max − current)` to a live, undead target with `Health`, sends `Healed` when it restored any and traces `healed`; `heal` is its immediate form. A heal never fills a shield; `Shield::restore` does.
- **The queue** (Q14). `queue` pushes onto a kit resource that `HealthPlugin` inserts. `damage_resolve` (`Resolve`) takes the queued damage requests stable-sorted by (target index, source index with none first) and applies each, then the heal requests sorted the same way. A request queued outside the fixed step waits for the next step, a paused clock included (A30).
- **`kit_despawn`** (`React`, after `cues`), the kit's one end-of-step removal. Each entity the step listed, unless gone or marked `Despawning`, is marked `Despawning` with every descendant under it (through `Children`, as `despawn_recursive` walks them, `hierarchy.rs:294-306`), queued for `despawn_recursive` and traced as `Despawn { by }`: a death past its delay (`health`), an ended projectile (`projectile`), a collected pickup (`pickup`), and now a `Lifetime`'s end (`lifetime`) and a `despawn_outside` (`despawn_outside`), which list their entities here instead of queueing at once (`lifetime.rs:150-164`, `despawn.rs:270-287`). Every kit despawn therefore queues after the step's cue spawns from its entity, which an earlier despawn command would drop at the flush (A9, `world.rs:820-826`), so a bolt whose lifetime ends as it hits still spawns its `hit` cue's spark; and a frame's later step, before the flush (`crates/tungsten/src/app.rs:1600-1609`), skips an entity the kit ended and its descendants: a hitting entity, a hurtbox, a pickup, a collector or a cue subject marked `Despawning` does nothing, so a dying parent's child hitbox strikes no more. A cue's `despawn` keeps its own queue, after the step's spawns (A8), and now marks its target's descendants too. A game's own despawn, or the state stack's, earlier in the frame still drops the spawns from its entity, a recorded limit.

### 2.8 Cues

- **Events.** Read after A's, in this order: `hit` (`Struck`: the hitting entity the subject, the target the other, its amount), `healed` (its amount, the source the other), `picked_up` (the pickup the subject, the collector the other) and `expired` (the projectile; no amount, no other). `damaged` and `killed` gain the request's source as their other entity, and take the event's position when it has one, else the subject's point as today.
- **The action `roll`.** `{ "roll": "<loot_tables ID>" }`, or the table in place. It draws the table's rolls from `KitRng`, which `CuesPlugin` now inserts when absent, in the cue system's order, and spawns each drawn prefab at the occurrence's position from the subject, as `prefab` does. The load checks the ID, or an inline table's prefab IDs.
- **The kind.** `loot_tables` (a `WeightedTable` per entry) is `CuesPlugin`'s; its load check covers every entry's prefab.
- **The trace.** Each new event is traced on its subject as `TraceKind::Event`, each drop as `Spawn { by: "cues" }`, and `kit_despawn`'s despawns as `Despawn`.
- **The boundary** stands (w13-kit §3): no conditions, and `roll` draws only from the table it names.

### 2.9 Stats

`stats_apply` writes `damage` into the entity's `Hitbox` and `ContactDamage` when present, under `D-173`'s rule that items read no stats; it becomes the query's eighth column. The magnet reads its collector's `pickup_radius`, the one stat an item reads from another entity, since the stat is the collector's and the item the pickup's (Q13). `projectile_speed` and `area` wait for W13k (Q9); their rustdoc says so.

### 2.10 Phases and edges

| Phase | New members |
| --- | --- |
| `Tick` | `shield_tick`; `health_tick` also counts death delays |
| `Move` | `pickup_magnet`, after `oscillator` and before `speed_limit`, each `_if_present` |
| `Overlap` | `hit_grid`; then `hitbox_tick`, `contact_damage`, `projectile_tick` and `pickup_tick`, each after `hit_grid`; all five before `despawn_outside`, `wrap` and `stay_in_view` (`_if_present`), so they read the step's motion before a wrap writes `PrevPosition` (A13), and `despawn_outside` judges a projectile where its wall left it, not past the wall it bounced off |
| `Resolve` | `damage_resolve` |
| `React` | `kit_despawn`, after `cues` (`_if_present`) |

Each of C's overlap plugins adds `hit_grid` when the schedule lacks it (`Schedule::contains`, as `ensure_phases` does), so any one of them works alone; `HealthPlugin`, `ProjectilePlugin`, `PickupPlugin`, `LifetimePlugin` and `DespawnOutsidePlugin` add `kit_despawn` the same way. `KitPlugins::set()` builds `HitPlugin`, `ContactDamagePlugin`, `ProjectilePlugin` and `PickupPlugin` after `DepthSortPlugin` and before `KitDebugPlugin`. With `DefaultPlugins` then `KitPlugins::set()`, `fixed_update` is expected to resolve to:

`fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, shield_tick, stats_apply, cooldown_tick, kit_intent, action_input, aim, kit_move, path_follow, linear_motion, oscillator, platformer_controller, top_down_mover, steering, follow, orbit, knockback, pickup_magnet, speed_limit, physics_step, kit_overlap, zone_tick, speed_limit_settle, hit_grid, hitbox_tick, contact_damage, projectile_tick, pickup_tick, despawn_outside, wrap, stay_in_view, kit_resolve, damage_resolve, kit_react, spawner_tick, spawn_children, cues, kit_despawn, kit_trace_step`

`despawn_outside` moves after C's systems, behind `zone_tick` and `speed_limit_settle`, which read and write no point it judges.

`post_update` is unchanged. A resolved order that differs but holds every edge above is pinned as the resolver gives it and recorded as a departure; one that breaks an edge fails its step.

### 2.11 Item cards

| Item | DTO fields (defaults) | Events | Phase | Writes | Stats read | Genres | Inspector, debug | Bench row |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `hitbox`, `hurtbox` | hitbox: `shape`, `offset` (`[0, 0]`), `damage`, `tags` (`[]`), `filter` (`{}`), `knockback` (0); hurtbox: `shape`, `offset` (`[0, 0]`) | out `Struck`; requests | `Overlap` | its hit list; requests | — (`stats_apply` writes `damage`) | top-down, shmup, platformer, grid | shape, damage, targets hit; shapes drawn | `hit_grid`, `hitbox_tick` |
| `contact_damage` | `damage`, `interval` (0.5), `shape` (the hurtbox's), `offset` (with `shape`; `[0, 0]`), `tags` (`[]`), `filter` (`{}`), `knockback` (0) | out `Struck`; requests | `Overlap` | its target times; requests | — (`damage` written) | top-down, platformer, grid | damage, interval, targets timed; shape drawn | `contact_damage` |
| `projectile` | `pierce` (0), `range` (none), `bounces` (0) | in its hitbox's hits; out `ProjectileExpired` | `Overlap` | `Position` and its movers' velocities at a wall or its end | — | top-down, platformer, shmup | pierce, range and bounces left, spent | `projectile_tick` |
| `pickup` | `shape`, `offset` (`[0, 0]`), `filter` (`{}`), `magnet` (0) | out `PickedUp` | `Move`, `Overlap` | `Position` (the magnet) | the collector's `pickup_radius` | top-down, platformer, grid | shape, magnet, collected; shape drawn | `pickup_magnet`, `pickup_tick` |
| `resist` | a damage tag → a factor | — | (`Resolve`) | — | — | top-down, shmup | factors | in `damage_resolve` |
| `shield` | `max`, `recharge` (0), `delay` (0) | — | `Tick`, (`Resolve`) | its current | — | shmup, top-down | current, max, recharge | `shield_tick` |
| `health`'s `despawn_on_death` | seconds (none: kept) | out `Damaged`, `Healed` | `Resolve`, `Tick` | — | `max_health`, as today | top-down, grid | the death row | `damage_resolve` |
| `roll`, `loot_tables` | `rolls` (1), `entries` of `prefab` (none: nothing) and `weight` | in any cue event | `React` | spawns | — | top-down, grid | — | in `cues` |

### 2.12 Recipes, the hash and the bench

- **Fixtures.** Each new scene name appears in at least two genres' fixtures, and `loot_tables` in two (the coverage test, `D-172`): top-down `ranger` (the hero, with a `hurtbox` and `pickup_radius`), `ember` (a bodiless bolt with `hitbox` and `projectile`), `ghoul` (`hurtbox`, `contact_damage`, `health` with `despawn_on_death`), `ghoul_burst`, `orb` (a `pickup` with a magnet), `knight` (`shield`, `resist`) and `data/loot_tables/ghoul_drops.json`; platformer `climber`, `fireball`, `brick` (a static wall), `spiker`, `coin`; shmup `interceptor`, `laser`, `carrier` (a recharging `shield`), `bulwark` (immune to lasers); grid `adventurer`, `swing` (a short-lived `hitbox`), `chest`, `gold`, `spikes` and `data/loot_tables/chest_loot.json`. Each genre's `data/tags.json` declares the damage tags its prefabs name. The names are the run's to change. Step 1 lands them as data, so the coverage test holds from step 1; step 5 drives them.
- **The card's recipe**, top-down, cues and cue rules alone: the `ranger`'s `cooldown` fires an `ember` through its `cooldown_elapsed` cue, so the ember's `Source` is the ranger; the ember's hit kills a `ghoul`. The `combat` rule counts the kill on the game's `kills` and floats the hit's damage (A24); the ghoul's `killed` cues spawn `ghoul_burst` and roll `ghoul_drops`, one `orb` at the hit; a `counter` with `target: other` counts the kill on the ranger, the kill's other entity; the ghoul is gone the step after its kill, after its cues (`despawn_on_death: 0`).
- **The hash.** The script spawns C's prefabs after the existing ones in each genre and lets their bolts, contacts and pickups land; the rows add each entity's shield. The pin moves in step 5 to the value the run prints, equal across two in-process runs, recorded in the evidence log, the `CHANGELOG.md` line and the decision. Steps 1–4 hold `0xd651_23fe_e583_03ee`, since no existing script entity carries C's components (A24).
- **The bench** (step 6). Eight rows at 1,000 and 10,000 entities at one density, the area growing with the count, so that no grid row is judged on a denser crowd: `hit_grid` (moving hurtboxes, no hitbox), `hitbox_tick` (each hitbox over one hurtbox it has already hit, the steady state), `contact_damage` (each over one target, re-hit each interval), `projectile_tick` (bodiless projectiles with hitboxes in open space beside one static body, so the wall query runs), `pickup_tick` (no collector reached), `pickup_magnet` (each pickup inside a collector's radius, with one collector per ten pickups, so a scan over every collector would fail the bound), `damage_resolve` (one request a step per target, through `Resist` and `Shield`) and `shield_tick` (recharging). The 31 rows keep M64's bound (A26).

### 2.13 The template (step 7)

The hazard keeps its patrol and becomes data: `contact_damage { damage 1, interval 0.5, knockback 420, filter team.player, shape circle 20 }`, and the player gains `hurtbox { circle 8 }`, so a touch lands below 28 px between their points as `hazard_touch` hurt at 28. `hazard_touch`, `Hazard` and their registration go. Gameplay spawns three gems (`prefabs/gem.json`: the player sprite tinted gold, `pickup { circle 6, filter team.player, magnet 240 }`, an `oscillator` bob, a `picked_up` cue counting `gems` on the game's `Counters`), and the player's `stats` hold `pickup_radius` 96. The template keeps no game Rust for the hazard or the gems, and its text is unchanged (Q16).

### 2.14 The seam

The kit stays on core alone. The grid, the test and the walls use core's `Shape`, `overlap`, `cast_shape` and `SpatialFilter` as they are: no core, render or umbrella change, no render type, no dependency and no thread.

## 3. Steps

Every code step ends with `cargo fmt --all` before its evidence row. A private item whose first non-test reader lands in a later step takes `#[expect(dead_code, reason = "M65 step N")]`, removed in that step and recorded as a departure.

### Step 1: The API, the damage pipeline and the `roll` action

- **Files:** step 1's in the header.
- **Change:** in this order:
  1. The pipeline (§2.7) whole: `DamageRequest`, `HealRequest`, `Healed`, `heal`, `Resist`, `Shield`, `Hit::Absorbed`, `Damaged`'s fields and builders, `Health`'s `despawn_on_death` (its scene field and its saved fields `despawn_on_death` and `death_left`, both defaulted), `damage` onto `apply`, `damage_resolve`, `shield_tick`, the death delay in `health_tick`, and `kit_despawn` with the ended list, onto which `Lifetime` and `despawn_outside` now list their ends (§2.7), each of those plugins adding `kit_despawn` as the others do; `kit_despawn` and a cue's `despawn` mark every descendant `Despawning` with the entity. `HealthPlugin` registers `Healed`, the two queues, `resist` and `shield` with their descriptions and inspector rows.
  2. The overlap items' public surface (§2.2): `HitShape`, `Hitbox`, `Hurtbox`, `Struck`, `ContactDamage`, `Projectile`, `ProjectileExpired`, `Pickup`, `PickedUp` and their four plugins, each registering its scene component, description, inspector rows and event, with its systems in their phases and edges (§2.10) as empty bodies until steps 2–4.
  3. `WeightedTable` and `draw`, the `loot_tables` kind, `CueAction::roll` and its run, the four cue events and their readers, `damaged` and `killed` with the source as other, `KitRng` inserted by `CuesPlugin`, and `FieldType::Data`.
  4. `stats_apply`'s `damage` writes; the three stats' rustdoc (§2.9); the `KitPlugins::set()` order; the phase table's rustdoc; `KitRng`'s rustdoc on who draws; the root re-exports, `prelude` and the crate docs' catalogue.
  5. The fixtures (§2.12) as data, the tags declared.
  6. Unit tests: each type's constructors, builders and refusals; serde through the checked fields; each new description held to its DTO; the pipeline (an amount of 0 ignored with no event, window, flash or shield change, immediate or queued; a `Resist` product over a tag and its ancestor, immunity at 0 ahead of factors whose product passes `f32::MAX`, a product past it clamped and finite; a kill landed by a system after `cues` keeping its `killed` cues and despawned the step after; a shield's absorb and its recharge after its delay, `Absorbed`, a despawn at 0 and at 0.1 s after the `killed` cues, a heal capped at max and refused on the dead, requests sorted by target then source, a request queued while paused resolving on the next step); a lifetime's end and a `despawn_outside` queued after the step's cue spawns from their entity, each traced once with its own `by`; the descendants of an entity `kit_despawn` or a cue's `despawn` ends marked with it; a weighted draw's frequencies under a seed; `roll` spawning from the subject at its point. The pins move: the lifetime and despawn-outside tests' system counts from 8 to 9 (`crates/tungsten-kit/src/tests/lifetime.rs:30`, `despawn.rs:30`); the description test to 39 components, 4 kinds and 17 shapes (`hit_shape`, `weighted_table`, `weighted_entry`), its one-of probe from 12 to 15 (`hit_shape`'s two forms and `cue_action`'s tenth, `roll`), `weighted_table` beside `cue_rule` and `stat_bounds` as a kind's entry that no field names (`describe.rs:128-153`), and its probe values taught `FieldType::Data`; the health test's system count; the coverage test to 39 and 4; the umbrella's `KIT_FIXED_UPDATE`; and the template's `SCHEDULE`, with `hazard_touch` after `stay_in_view`.
  7. `just api`.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0
  - `cargo test -p tungsten-kit --locked` → exit 0, its doctests included; `for m in damage::pipeline damage::defense lifecycle::ended overlap::hit overlap::contact overlap::projectile overlap::pickup react::loot; do rtk proxy cargo test -p tungsten-kit --lib --locked "$m::tests::" -- --list 2>/dev/null | rg -c ': test$'; done` → eight counts, each at least 1
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0
  - `cargo test -p tungsten --lib --locked kit` → exit 0, `kit_tests` and `kit_recipes` included
  - `rg -c 'KIT_RECIPES_HASH: u64 = 0xd651_23fe_e583_03ee;' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 1
  - `rg -c 'knockback, pickup_magnet, speed_limit, physics_step, kit_overlap, zone_tick, speed_limit_settle, hit_grid, hitbox_tick, contact_damage, projectile_tick, pickup_tick, despawn_outside, wrap, stay_in_view, zone_entered, kit_resolve, damage_resolve, kit_react, spawner_tick, spawn_children, cues, kit_despawn, kit_trace_step' crates/tungsten/src/tests/kit.rs` → 1, or the departure's line (§2.10)
  - `cargo test -p tungsten --lib --locked plugins::tests::default_plugins_resolve_to_the_snapshot` → 1 passed; `git diff --quiet -- crates/tungsten/src/tests/plugins.rs` → exit 0
  - `cargo test -p tungsten-template-basic --locked` → exit 0
  - `just api` → exit 0, then:
    - `rg -c 'pub struct tungsten_kit::(Hitbox|Hurtbox|HitShape|ContactDamage|Projectile|ProjectileExpired|Pickup|PickedUp|Struck|Resist|Shield|DamageRequest|HealRequest|Healed|WeightedTable)\b' api/tungsten-kit.txt` → 15
    - `rg -c 'pub struct tungsten_kit::(HitPlugin|ContactDamagePlugin|ProjectilePlugin|PickupPlugin)\b' api/tungsten-kit.txt` → 4
    - `rg -c 'pub const tungsten_kit::(HIT_GRID|HITBOX_TICK|CONTACT_DAMAGE|PROJECTILE_TICK|PICKUP_TICK|PICKUP_MAGNET|DAMAGE_RESOLVE|SHIELD_TICK|KIT_DESPAWN):' api/tungsten-kit.txt` → 9
    - `rg -c '^pub tungsten_kit::(prelude::)?Hit::Absorbed$' api/tungsten-kit.txt` → 2; `rg -c '^pub tungsten_kit::Damaged::(source|position|crit):' api/tungsten-kit.txt` → 3; `rg -c '^pub fn tungsten_kit::(prelude::)?heal\(' api/tungsten-kit.txt` → 2
    - `comm -23 <(git cat-file -p HEAD:api/tungsten-kit.txt | sort) <(sort api/tungsten-kit.txt) | wc -l` → 0
    - `rg -c '^component ' api/tungsten-kit-components.txt` → 39; `rg -c '^shape ' api/tungsten-kit-components.txt` → 17; `rg -c '^kind ' api/tungsten-kit-components.txt` → 4
    - `git diff --quiet -- api/tungsten-core.txt api/tungsten-render.txt api/tungsten.txt` → exit 0
  - `just repo-check` → exit 0
- **Moves:** the umbrella's and the template's resolved-schedule pins (nine systems, empty in part); the description and coverage counts; `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt`, by additions.
- **Then:** the step 1 API critique as the skill's level C rule says, each finding recorded in §9 as `C.<n>`, and step 1 changed where one holds.

### Step 2: The hit grid, hitboxes and contact damage

- **Files:** step 2's in the header.
- **Change:** the grid with its cell bounds and the swept test (§2.3) in `overlap/grid.rs`, the grid's `ensure` also registering the emitter's derivation, then `hit_grid`'s, `hitbox_tick`'s and `contact_damage`'s bodies (§2.3, §2.6); `kit_debug_draw` draws hurtboxes, hitboxes, contact shapes and pickups. Unit tests in `tests/grid.rs`: each shape pair's first t, a start overlap at 0, a graze that never overlaps, two paths that cross inside a step, a fast pass, candidate order, and a hurtbox past 64 cells and a sweep past the occupied bounds each still met through its overflow path. Harness tests in `kit_tests`:
  - `a_hitbox_hits_each_target_once_per_activation_at_three_frame_rates`: a hitbox kept over two hurtboxes for 60 steps at dt 1/60, 1/30 and 1/144 hits each once, with the same `Damaged` events at each dt; after `rearm`, once more.
  - `a_bolt_faster_than_a_hurtbox_per_step_hits_it_once`: a bodiless bolt at 1,800 px/s (30 px a step) through a hurtbox 8 px across, never overlapping it at a step's end (asserted), sends one `Struck` and one `Damaged`.
  - `a_bolt_and_a_moving_hurtbox_crossing_between_their_endpoints_meet_once`: a bolt moving along x and a hurtbox moving along y that cross inside one step, apart at every step's end (asserted), meet once.
  - `contact_damage_rehits_a_target_each_interval_while_it_touches`: a target kept in a contact for 1.5 s at the three dts is hit on steps 1, 31 and 61 at each; a target that leaves and returns inside the interval waits it out.
  - `a_hitbox_spares_its_entity_its_emitter_and_its_source_and_follows_its_filter`: with the default filter, a bolt a nest's turret spawns on the turret's own hurtbox hits neither the turret nor the nest, and hits a third entity.
  - `a_dead_or_despawning_hurtbox_takes_no_hit`
  - `a_dying_parents_child_hitbox_strikes_no_more_that_frame`: at dt 1/30, a parent whose lifetime ends in the frame's first step keeps its child hitbox's first-step hit and that hit's cue spawn, and the child strikes nothing in the second step.
  - `stats_damage_sets_the_damage_a_hitbox_and_a_contact_deal`
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0
  - `cargo test -p tungsten-kit --locked` → exit 0; `rtk proxy cargo test -p tungsten-kit --lib --locked overlap::grid::tests:: -- --list 2>/dev/null | rg -c ': test$'` → at least 8
  - `cargo test -p tungsten --lib --locked kit` → exit 0; `rtk proxy cargo test -p tungsten --lib --locked kit_tests:: -- --list 2>/dev/null | rg -c '(hits_each_target_once_per_activation_at_three_frame_rates|faster_than_a_hurtbox_per_step_hits_it_once|crossing_between_their_endpoints_meet_once|rehits_a_target_each_interval_while_it_touches|spares_its_entity_its_emitter_and_its_source_and_follows_its_filter|dead_or_despawning_hurtbox_takes_no_hit|dying_parents_child_hitbox_strikes_no_more_that_frame|stats_damage_sets_the_damage_a_hitbox_and_a_contact_deal): test$'` → 8
  - `rg -c 'KIT_RECIPES_HASH: u64 = 0xd651_23fe_e583_03ee;' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 1
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0; `cargo test -p tungsten-template-basic --locked` → exit 0
- **Moves:** none.

### Step 3: Projectiles

- **Files:** step 3's in the header.
- **Change:** the wall test (§2.4) in `overlap/grid.rs`, with the contact normal, then `projectile_tick`'s body (§2.5). Unit tests in `tests/grid.rs` for the normal at each shape pair; harness tests in `kit_tests`:
  - `a_projectile_ends_at_a_static_wall_and_at_a_tile_and_spares_what_is_behind`: one wall a static body, one a tile layer (the tests' `tile_map` helper).
  - `a_projectile_passes_kinematic_bodies_and_hits_their_hurtboxes`
  - `a_bouncing_projectile_reflects_off_a_wall_then_ends_at_the_next`: a bodiless `linear_motion`, a kinematic body and a `steering` homer, each placed at the contact point with its velocity reflected.
  - `a_piercing_projectile_passes_its_targets_in_sweep_order_and_ends_at_the_next`: two of them inside one step.
  - `a_projectile_ends_at_its_range_on_its_path`
  - `an_ended_projectile_stops_sends_expired_and_despawns_after_its_cues`: its `expired` cue's prefab spawns at the end point at dt 1/60 and 1/30, and at 1/30 it does not move in the frame's second step.
  - `a_projectile_whose_lifetime_ends_as_it_hits_keeps_its_hit_cue`: the spark its `hit` cue spawns survives the flush that takes the bolt.
  - `a_projectile_bouncing_back_inside_its_region_stays`: a bolt whose sweep crosses its `despawn_outside` region's edge at a wall and bounces back inside is not despawned.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0
  - `cargo test -p tungsten-kit --locked` → exit 0
  - `cargo test -p tungsten --lib --locked kit` → exit 0; `rtk proxy cargo test -p tungsten --lib --locked kit_tests:: -- --list 2>/dev/null | rg -c '(ends_at_a_static_wall_and_at_a_tile_and_spares_what_is_behind|passes_kinematic_bodies_and_hits_their_hurtboxes|reflects_off_a_wall_then_ends_at_the_next|passes_its_targets_in_sweep_order_and_ends_at_the_next|ends_at_its_range_on_its_path|stops_sends_expired_and_despawns_after_its_cues|lifetime_ends_as_it_hits_keeps_its_hit_cue|bouncing_back_inside_its_region_stays): test$'` → 8
  - `rg -c 'KIT_RECIPES_HASH: u64 = 0xd651_23fe_e583_03ee;' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 1
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0
- **Moves:** none.

### Step 4: Pickups

- **Files:** step 4's in the header.
- **Change:** `pickup_tick`'s and `pickup_magnet`'s bodies (§2.6). Harness tests in `kit_tests`:
  - `a_pickup_goes_once_to_the_first_collector_its_filter_passes`: two collectors reach it in one step; its `picked_up` cue with `target: other` counts on the first; it is despawned after its cues.
  - `a_magnet_pulls_a_pickup_to_the_nearest_collector_within_its_pickup_radius`: the same path at dt 1/60, 1/30 and 1/144; never past the collector; no pull from a collector without the stat or out of range.
  - `a_magnet_leaves_a_body_or_a_child_with_one_warning`
  - `a_magnet_passes_a_nearer_corpse_for_a_live_collector`: a dead collector kept by the default policy, nearer than a live one, draws nothing.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0
  - `cargo test -p tungsten-kit --locked` → exit 0
  - `cargo test -p tungsten --lib --locked kit` → exit 0; `rtk proxy cargo test -p tungsten --lib --locked kit_tests:: -- --list 2>/dev/null | rg -c '(goes_once_to_the_first_collector_its_filter_passes|pulls_a_pickup_to_the_nearest_collector_within_its_pickup_radius|leaves_a_body_or_a_child_with_one_warning|passes_a_nearer_corpse_for_a_live_collector): test$'` → 4
  - `rg -c 'KIT_RECIPES_HASH: u64 = 0xd651_23fe_e583_03ee;' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 1
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0
- **Moves:** none.

### Step 5: Recipes in the genres and the kit hash

- **Files:** step 5's in the header.
- **Change:** sixteen recipes, each at both of the recipes' dts (A25), each item in two genres (§2.11):
  - top-down: `a_cue_driven_kill_counts_bursts_labels_and_rolls_a_drop` (the card's, §2.12); `an_ember_pierces_two_ghouls_and_expires_at_its_range`; `a_knights_shield_takes_a_hit_then_its_resist_halves_fire`; `an_orb_drifts_to_the_ranger_and_counts_its_xp`; `a_ghouls_touch_hurts_the_ranger_each_interval`; `the_card_recipe_holds_with_the_overlap_plugins_reversed`, which builds `HealthPlugin`, `CuesPlugin` and C's four plugins last in reverse order through a new `overlap_reversed()` in `kit_recipes/mod.rs`.
  - platformer: `a_fireball_stops_at_a_brick`; `a_bouncing_fireball_turns_at_a_brick_then_stops`; `a_spikers_touch_hurts_the_climber_and_throws_it_back`; `a_coin_counts_once`.
  - shmup: `a_laser_hits_once_and_ends_at_its_range`; `a_carriers_shield_recharges_after_its_delay`; `a_bulwark_ignores_lasers`.
  - grid: `a_chest_rolls_its_loot_when_broken` (the adventurer's `swing`); `gold_counts_once`; `spikes_hurt_the_adventurer_each_interval`.
  - order: `crates/tungsten-kit/src/tests/phases.rs` builds each §2.10 edge's two plugins alone, in both orders.
  - the hash (§2.12): C's prefabs spawned after the existing ones, the shield in the rows, the new pin from the run's print.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0
  - `cargo test -p tungsten --lib --locked kit_recipes` → exit 0, the coverage test at 39 names and 4 kinds and the hash test equal across two in-process runs
  - `rtk proxy cargo test -p tungsten --lib --locked kit_recipes:: -- --list 2>/dev/null | rg -c '(cue_driven_kill_counts|pierces_two_ghouls|shield_takes_a_hit_then|orb_drifts_to_the_ranger|ghouls_touch_hurts|overlap_plugins_reversed|fireball_stops_at_a_brick|fireball_turns_at_a_brick|spikers_touch_hurts|coin_counts_once|laser_hits_once|shield_recharges_after|bulwark_ignores_lasers|chest_rolls_its_loot|gold_counts_once|spikes_hurt_the_adventurer)[a-z_]*: test$'` → 16
  - `rg -c 'KIT_RECIPES_HASH: u64 = 0xd651_23fe_e583_03ee;' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 0
  - `cargo test -p tungsten-kit --locked` → exit 0, the edge tests included
- **Moves:** the kit hash, its new value in the evidence log.

### Step 6: The kit bench's rows and their sitting

- **Files:** step 6's in the header.
- **Change:** the eight rows (§2.12) and the bench's header (31 rows, the bound); a test-mode run; then one quiet sitting with the owner disconnected (workflow §6): no `nxcodec.bin`, `cargo` or `rustc` beside it and nothing written to the tree during it. It runs as at most three blocking foreground calls of `CARGO_TARGET_DIR=$PWD/target/criterion-native cargo bench -p tungsten-kit --bench kit --locked -- '<row filter>'`, each under 600 s, with `pgrep -x 'nxcodec.bin|cargo|rustc'` before and after each. A scratchpad script reads each row's `new/estimates.json` median at 1,000 and 10,000 and writes the per-entity medians and ratios, in M64's format, to the summary. The profiling workflow's criterion paragraph then names 31 rows and M65's run.
- **Done-when:**
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0
  - `rg -c ' ratio ' perf-runs/<YYYYMMDD>-m65-kit-criterion/summary.txt` → 31; `awk '/ ratio / && $NF + 0 > 2.0' perf-runs/<YYYYMMDD>-m65-kit-criterion/summary.txt` → prints nothing
  - the quiet checks before and after each part print nothing (quoted)
  - `health_tick`, `stats_apply` and `cues`, which steps 1–2 touch, within 1.5× their M64 per-entity medians at both sizes (`perf-runs/20261017-m64-kit-criterion/summary.txt`; across sittings, so a guard against gross cost rather than a verdict)
- **Moves:** none.

### Step 7: The template's hazard on contact damage and its pickup

- **Files:** step 7's in the header.
- **Change:** §2.13. The template's tests find the hazard by its `ContactDamage` and keep their expectations (one hit per window, pushed away, the label and the shake); a test that put the player exactly on the hazard's point, where `hazard_touch` pushed straight up and the kit pushes along the hazard's motion (§2.3), places it a pixel off that point instead; `SCHEDULE` loses `hazard_touch`; `a_gem_drifts_to_the_player_and_counts_once` joins them. `templates/basic/AGENTS.md` lists the new scene names, the four cue events, `roll`, `DamageRequest` and `heal`, and drops the `hazard_touch` sentence; the guide's "Systems and stages" example drops `hazard_touch`, and its "Composing with the kit" gains the hit shapes, contact damage, projectiles, pickups, resistances, shields, the death policy and `roll`.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0
  - `cargo test -p tungsten-template-basic --locked` → exit 0; `rtk proxy cargo test -p tungsten-template-basic --locked -- --list 2>/dev/null | rg -c 'a_gem_drifts_to_the_player_and_counts_once: test$'` → 1
  - `rg -c 'hazard_touch|Hazard\b' templates/basic/src templates/basic/tests templates/basic/AGENTS.md docs/getting-started.md` → no match (exit 1)
  - `rg -c '"contact_damage"' templates/basic/assets/prefabs/hazard.json` → 1; `rg -c '"hurtbox"' templates/basic/assets/prefabs/player.json` → 1; `rg -c '"pickup"' templates/basic/assets/prefabs/gem.json` → 1
  - `just template-check` → exit 0; `just repo-check` → exit 0
- **Moves:** the template's schedule pin, without `hazard_touch`.

### Step 8: Records

- **Files:** step 8's in the header.
- **Change:** in this order: decision 175 with the [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md) skill (§5), its index row and the marker lines on `D-158`, `D-172` and `D-173`; then `DESIGN.md`'s "Overlap and Damage — M65" after "Motion and Facing — M64"; the known issues' kit rows (`docs/known-issues.md:52-55`) lose the waits C ends (the template's hazard; a bodiless entity is found by a hitbox or a pickup now, though zones still see bodies only), narrow M63's dropped-`prefab`-cue limit to spawns from an entity that a game's system or the state stack despawned earlier in the frame, since the kit's own despawns now queue after the cues and mark the entity's descendants, and gain C's limits (§Follow-ups and Q1, Q3, Q11, Q15, a homer's creep after its end); the W4 ledger's two rows, additions and behaviour changes; `w13-kit.md`'s revision sentence; backlog rows for what C left; the LLM index's kit row, whose decision range ends at 175 instead of 174, the same byte count; the `CHANGELOG.md` line; a final `just api` when steps 2–7 changed surface.
- **Done-when:**
  - `rg -c '^## D-17[5] — W13j landed' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-17[5]:\*\*' DECISIONS.md` → 3; `` rg -c '^\| `D-17[5]` \|' docs/DECISION_INDEX.md `` → 1
  - `cargo test -p tungsten-core --test decision_index --locked` → exit 0
  - `rg -c '^### Overlap and Damage — M65' DESIGN.md` → 1
  - `` rg -c 'D-172`–`D-17[5]`' docs/LLM_INDEX.md `` → 1; `just ctx` → exit 0
  - `rg -c "until C's contact damage|until the kit grid of W13j" docs/known-issues.md` → no match (exit 1)
  - `rg -c 'W13j \(M65' docs/plans/1.0/w04-api-freeze.md` → 2
  - `just api-check` → exit 0; `just repo-check` → exit 0; `git diff --check` → exit 0
- **Moves:** none.

### Step 9: Release 0.85

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.85.0` once, every release check once after it with `just physics-release` (the physics hashes, which no step runs), then the preflight with `--message`
- **Done-when:** [releases](../../../releases.md) step 1's command chain and `just physics-release` → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.85.0 --repo JosephGibson/Tungsten --message 'Update 0.85: M65 kit overlap and damage'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for W13 damage types, the next register row since the 2026-10-18 roadmap amendment ([tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md), `plan`), naming `docs/plans/1.0/phase6-milestone-66-kit-damage-types.md`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | `Damaged` gains tags additively, as the card says? `Tags` is not `Copy` and `Damaged` is (A2), so a tags field is a compile break. | No tags. `Damaged` stays `Copy` and gains `source`, `position` (an `Option`, so `damage`'s event is unchanged) and `crit`; the request's tags stay on the request, where the resolution reads them. A reader that needs the kinds reads the hitting entity's `Hitbox`; a `Copy` handle for them is a follow-up. |
| Q2 | Which geometry ends a projectile's sweep? No query filter picks static bodies (A15), and a cast over bodies stops at the fixtures' kinematic enemies (A16). | Tile layers by a tiles-only `cast_shape`, and static colliders found by `overlap` and met by the kit's own test (§2.4): no core change. A static-only filter in core is W6b's, with its layers. |
| Q3 | Do walls end a plain hitbox's or a contact damage's sweep, as the catalogue's grid line reads? | Projectiles only. An aura or a body pressed against a wall would meet it at t = 0 (A15) and hit nothing; a thrown blade that should stop composes `projectile` (§2 rule 16). |
| Q4 | What are the catalogue's "immunity tags"? | A `Resist` factor of 0 for a damage tag; an immunity a status grants, such as an effect's tag, is W13k's to add as a field. |
| Q5 | Does a shield recharge? | Optionally: `recharge` per second after `delay` seconds without a hit, both 0 by default. |
| Q6 | Can data heal in C? | No: the vocabulary gives C `roll` alone (w13-kit §3), and W13k's `effect` heals. Heals come from Rust (`heal`, `HealRequest`) and from W13k. |
| Q7 | Does C add the `spawned` event (`D-173`'s follow-up, "with C's projectile")? | No: no C item needs it; it waits for a user, likely W13k's abilities. `expired` lands. |
| Q8 | Does `Lifetime` send `expired`? | No: `Lifetime` is not C's item; a projectile's range ends it with the event. A follow-up. |
| Q9 | Does C read `projectile_speed` (A19)? | No: W13k's abilities set a spawned projectile's speed from it; its rustdoc says so. |
| Q10 | What does `pierce: n` mean? | It passes n targets and ends at the next; 0 by default, ending at its first hit. |
| Q11 | Where does a bounce leave a projectile? | At the contact point with its movers' velocities reflected; the rest of the step's travel is dropped, a recorded limit. |
| Q12 | The grid's cell size? | 64 world px, fixed; a setting waits for a game that needs one. |
| Q13 | Whose radius is the magnet's? | The collector's `pickup_radius` stat, read across entities (§2.9); the speed is the pickup's. |
| Q14 | Are requests events? | No: a kit resource, drained each step, so a request queued while paused waits for the next step instead of being dropped (A30). |
| Q15 | Hit shapes on a child? | They stand at the child's `Transform.position`, the last frame's propagation, with no sweep: a recorded limit. Bodies stay on roots (`D-132`). |
| Q16 | Does the template show its gem count? | No: HUD text needs code until 1.x's `BindText` (w13-kit P12). |
| Q17 | A `knockback` cue action (`D-174`'s follow-up)? | No: the request carries the knockback, so contact damage and hitboxes push in data. |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

175: W13j landed (M65): the kit's overlap and damage. Hit shapes on a kit grid, tested over the step's relative motion; the damage pipeline (requests resolved once a step in sorted order through resistances, a shield and health, with heals and a death policy); contact damage, projectiles with walls from tiles and static colliders, and pickups with a magnet; the `loot_tables` kind and the `roll` action; the kit bench's 31 rows and the kit hash moved. It amends `D-158` (`damage` over the pipeline, `Damaged`'s fields, `Hit::Absorbed`, the death policy), `D-172` (the phase table's members, the set, and the lifecycle items' despawns, which queue through `kit_despawn` after the step's cues) and `D-173` (the cue vocabulary's events and the `roll` action; `stats_apply` writing `damage`); `D-154`, `D-170`, `D-171` and `D-174` carry it. It is the next free ID now (`DECISIONS.md` and its index end at 174); step 8 checks it again.

## 6. Invariants

- **May move:** the kit hash, in step 5 only, its new value in the evidence log, the `CHANGELOG.md` line and the decision; the umbrella's and the template's resolved-schedule pins (steps 1 and 7); the description and coverage counts (step 1); `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt`, by additions (steps 1 and 8).
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168`, the pinned containment hash `0xaee2_72e0_1ffc_4e4c`, the row digests, `gpu-visual.png`, the post and transition regressions; `DefaultPlugins`' resolved schedule; `api/tungsten-core.txt`, `api/tungsten-render.txt` and `api/tungsten.txt`; the kit on core alone with no new dependency (`git diff --quiet HEAD -- Cargo.lock` before the cut); two background threads; opt-in plugins; existing scene components' fields and defaults (`health` gains an optional field); the kit hash through steps 1–4.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 2 needs step 1; steps 3 and 4 need step 2; steps 5 and 6 need steps 2–4; step 7 needs steps 2 and 4; step 8 records what landed.
- The release needs steps 1–6, which make the card's candidate (the pipeline, the grid, projectiles, pickups, their recipes and bench rows). A skip among them ends the run before the release, which a release session takes after the owner has seen why (workflow §3). Step 7 alone may be skipped: the template keeps `hazard_touch`, a known-issues row says so, and its gates run in the release chain (`just template-check`).
- The step 1 API critique finds a shape tied to one genre: redesign it in step 1, or drop the item (w13-kit §14).
- An item needs a core or physics change, as walls would if §2.4's two queries could not find them: stop and record; the item waits (w13-kit §14).
- An item cannot pass recipes in two genres: leave it out with a backlog row, unless it is the card's own check (one hit per activation, the two bolts, the card's recipe), which stops the run.
- A reaction needs a condition beyond a tag filter: it is a game's Rust, and the recipe changes, not the vocabulary.
- A bench row's ratio passes 2.0: fix it before the release, else stop (w13-kit §14). A touched row past 1.5× its M64 median: rerun its part once; if it holds, stop and record.
- The kit hash moves in steps 1–4, or the physics hashes, `DefaultPlugins`' schedule or a core, render or umbrella snapshot moves at all: stop and record.
- `just ctx` fails on the LLM index's budget: trim inside step 8, else stop.
- The machine is not quiet for step 6: wait for the encoder to exit (`until ! pgrep -x nxcodec.bin`) and never capture beside it.
- Another session took 175 before step 8: take the next free ID and record it.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–7, 6, release | Lint, kit, umbrella and template tests and the bench's test mode in each code step; criterion's bound in step 6. No perf-suite row: nothing `just perf` runs links the kit (`D-174`), and no shared ECS or frame-loop code changes. No visual check: no render change, and the debug drawing is off by default. The release runs `just check`, `just smoke`, `just script-test`, `just template-check` and the rest of its chain once, after the cut, with `just physics-release` |
| 3 Invariants | 1–5, release | Steps 1–4 hold the kit hash; step 5 moves it on purpose; the release's `just physics-release` holds the physics hashes |
| 4 Decisions | 8 | 175 with its index row and marker lines, before `DESIGN.md`, the known issues and the ledger cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 8, release | Rustdoc on every new item under the kit's `deny(missing_docs)`; `just api` in step 1 and again in step 8 when surface changed; two ledger rows; the release runs `just api-check` |
| 6 Template and guide | 7 | The template moves onto the finished items in one step after them, with the guide |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 8 | The new files sit in the kit folder the index routes; the kit row's decision range only |
| 8 Design | 8 | `DESIGN.md`'s "Overlap and Damage — M65" |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 8, release | The gap log has no rows (`acceptance-game.md`); `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-17, reading `73043fe` with this plan uncommitted; the artifact was the plan less this section and the evidence log, by the skill's `sed` (75,376 characters, so `--force`). Effort max, context 872,000, output cap 4,000; 471.1 s; tokens in 920,362 (cached 815,232), out 16,656 (reasoning 12,168); 40 commands, output 325.8 KB (largest 60.6 KB); usage 5h 0% → 27%, week 82% → 86%. Six findings, all accepted; they changed done-when checks and the expected schedule, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Excluding `Source` does not prevent a projectile from hitting its shooter: for nest → enemy → bolt, `Source` names the nest, so the bolt can hit the enemy that spawned it on its own hurtbox. | `crates/tungsten-kit/src/foundation/source.rs:77-87` (the chain's origin); `crates/tungsten-core/src/ecs/world.rs:174-188` (a derivation per spawn from its source); `crates/tungsten-core/src/component_registry.rs:428-446` (a scene component sees only the World and the transform) | accepted | §2.3: a crate-private `Emitter`, derived as each spawn's own source, is excluded with `Source`; step 2 registers it and its test takes a two-generation chain |
| 2 | [HIGH] Earlier despawn commands defeat the ordering after cues: `Lifetime` and `despawn_outside` queue at once, so a bolt whose lifetime ends as it hits loses its cue spawns at the flush, and `despawn_outside` judges a bounced projectile's unclipped endpoint. | `crates/tungsten-kit/src/lifecycle/lifetime.rs:150-164`, `lifecycle/despawn.rs:270-287` (queued at once); `world.rs:820-826` (a spawn whose source an earlier command despawned is dropped); no fixture entity both expires and spawns, so the kit hash holds | accepted | §2.7: `Lifetime` and `despawn_outside` list their ends for `kit_despawn`; §2.10: C's overlap systems before `despawn_outside` too, the expected order and step 1's pin changed; step 1 gains the two files and their tests' pins; step 3 gains two tests; step 8 narrows M63's limit |
| 3 | [MEDIUM] The magnet can pick collectors that collection rejects, such as a corpse kept under the default death policy. | §2.3 against §2.6 as written | accepted | §2.6: the magnet's collectors take `pickup_tick`'s eligibility; step 4 gains `a_magnet_passes_a_nearer_corpse_for_a_live_collector` |
| 4 | [MEDIUM] Gathering collectors once per filter leaves a scan per pickup, pickups × collectors, which a bench with one collector cannot catch. | `crates/tungsten-kit/src/motion/target.rs:171-177` (seekers times matches); §2.12's row as written | accepted | §2.6: collectors binned per run into cells of the largest radius, each pickup scanning 3×3 cells; §2.12 and step 6: the row keeps one collector per ten pickups |
| 5 | [MEDIUM] Valid shapes and sweeps can overwhelm the grid's cell enumeration: a 16,384-px half extent spans 512 cells a side, and sweeps have no length bound. | `crates/tungsten-core/src/physics/query.rs:253-274` (core clamps a query to its occupied bounds and scans every entry past `max(4 × entries, 64)` cells); `crates/tungsten-kit/src/motion/linear.rs:97` (velocities only finite) | accepted | §2.3: an oversized list past 64 cells for insertion and core's fallback for queries; step 2's grid tests cover both paths |
| 6 | [MEDIUM] Finite resistance factors do not keep zero-factor immunity: `1e30 × 1e30 × 0` is NaN in `f32`. | `f32::MAX` is about 3.4e38, so `1e30 × 1e30` is infinite and `inf × 0` NaN (IEEE 754) | accepted | §2.7: a matching 0 is immunity before any product, the product in `f64` clamped to `f32::MAX`, factors in [0, 1000]; step 1's tests cover both |

Round 2: the same reviewer, 2026-10-17, on the revised plan cut the same way (79,204 characters, so `--force`), with a focus note naming round 1's changed sections and no rejected finding. Effort xhigh, context 872,000, output cap 4,000; 216.4 s; tokens in 593,019 (cached 514,176), out 7,717 (reasoning 4,946); 13 commands, output 230.6 KB (largest 64.8 KB); usage 5h 27% → 44%, week 86% → 89%. Three findings, all accepted and applied; there is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] Recursive despawns leave descendants active in later fixed steps: a parent's lifetime ends in a frame's first step, its unmarked child hitbox strikes in the second, and the parent's earlier despawn drops the child's cue spawn, so step 8's narrowing of the known issue would be wrong. | `crates/tungsten-kit/src/transform/hierarchy.rs:294-306` (children go at the flush); `crates/tungsten/src/app.rs:1600-1609` (one flush after `post_update`); `world.rs:820-826` | accepted | §2.7: `kit_despawn` and a cue's `despawn` mark every descendant `Despawning`, and every item skips a marked hitting entity, hurtbox, pickup, collector or subject; §2.3 likewise; step 2 gains `a_dying_parents_child_hitbox_strikes_no_more_that_frame`; step 8's narrowing reworded |
| 2.2 | [MEDIUM] Immediate damage after `cues` can remove a victim before its `killed` cues run: a zero-delay death listed in the killing step is despawned before the next cue read. | `crates/tungsten-kit/src/react/cues.rs:1214-1228` (a despawning or gone subject is skipped), `:1452-1473` (the read) | accepted | §2.7 step 7: `health_tick` lists a death from the step after the kill, so the cue system has read it first wherever it landed; step 1's tests cover a kill after `cues`; §2.12's recipe says the ghoul goes the step after |
| 2.3 | [MEDIUM] The resolution omits the zero-damage guard: an amount of 0 would start the window, reset a shield's recharge and send `Damaged`, against `damage`'s behaviour and its test. | `crates/tungsten-kit/src/damage/health.rs:281-283`; `crates/tungsten-kit/src/tests/health.rs:71-76` | accepted | §2.7 step 1: an amount of 0, before or after the resistances, is `Ignored` with no side effect; §2.3: a 0-damage hit sends `Struck` and queues nothing; step 1's tests cover both paths |

Step 1 API critique: the same reviewer, 2026-10-17, `--raw --repo --force` on the `api/` and template diff with the untracked snapshots (`git --no-pager diff HEAD -- api/ templates/` plus each new file against `/dev/null`), xhigh, context 872,000, output cap 4,000; 154.5 s; tokens in 1,145,850 (cached 1,034,496), out 4,939 (reasoning 2,237); 26 commands, output 263.8 KB (largest 48 KB); usage 5h 0% → 24%, week 0% → 4%. No concerns: it read the empty overlap bodies as the plan's staging for steps 2–4. No `C.<n>` rows; step 1 stands.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-17 | done | `cargo fmt --all -- --check` exit 0; `just lint` exit 0; `cargo test -p tungsten-kit --locked` exit 0 (499 unit, 73 doctests); module counts pipeline 17, defense 12, ended 7, hit 11, contact 8, projectile 5, pickup 5, loot 7; `cargo test -p tungsten-kit --bench kit --locked` exit 0; `cargo test -p tungsten --lib --locked kit` exit 0 (99, `kit_recipes` 46 with coverage at 39 names and 4 kinds and the hash test); hash pin rg → 1; `KIT_FIXED_UPDATE` rg → 1, the expected order exactly; `default_plugins_resolve_to_the_snapshot` 1 passed, `plugins.rs` unchanged (exit 0); template tests exit 0 (18); `just api` exit 0, then 15, 4, 9, 2, 3, 2, `comm -23` → 0, components 39, shapes 17, kinds 4, core/render/umbrella snapshots unchanged (exit 0); `just repo-check` exit 0. Departures: the inspector's and the state machine's pins moved too (`tests/inspect.rs` 32 → 39 described components with rows, `tests/state_machine.rs`'s resolved order), files the header missed; the components snapshot changes one line besides its 72 additions, `health`'s events `Damaged` → `Damaged, Healed`; `contact_damage`'s `offset` is described `= null` (it comes only with `shape`, `[0, 0]` then); `kit_despawn` also follows `spawner_tick` and `spawn_children` when registered, so a spawner's spawn from an ending entity survives without cues too; the cue test of a despawning subject shows its skip with a zone event, since the pipeline now ignores a second hit on a despawning target (§2.7 step 1); the umbrella test's game event `picked_up` became `coin_taken` and `CuesOnEvent`'s doc example `opened`, `picked_up` being the kit's now; the platformer fixture adds `ricochet`, a fireball with one bounce, for step 5's bounce recipe; the numeric-field probe counts 69. | `crates/tungsten-kit/src/{damage,lifecycle,overlap,react,foundation}/`, `crates/tungsten/tests/fixtures/kit/`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt` |
| 2 | 2026-10-17 | done | `cargo fmt --all -- --check` exit 0; `just lint` exit 0 (after renaming `disk`'s one-letter bindings and an `if let` in `due`, pedantic lints); `cargo test -p tungsten-kit --locked` exit 0 (513 unit, 73 doctests); `overlap::grid::tests::` 11; `cargo test -p tungsten --lib --locked kit` exit 0 (107); the eight named `kit_tests` listed → 8; hash pin rg → 1; bench test mode exit 0; template tests exit 0. Departures: each hitting plugin registers the emitter's derivation (`ensure_emitter`: hit and contact here, projectile in step 3) rather than the grid's ensure, which takes no World, and pickups spare no one; contact damage spares its emitter and source too, as §2.3's "who is hit" reads for every hitting item; `kit_trace_step` also follows `kit_despawn` when registered, so a despawn's trace keeps its step whatever the build order (a debug test builds both orders). | `crates/tungsten-kit/src/overlap/{grid.rs,hit.rs,contact.rs}`, `crates/tungsten-kit/src/foundation/debug.rs`, `crates/tungsten-kit/src/tests/{grid.rs,contact.rs,debug.rs}`, `crates/tungsten/src/tests/kit.rs` |
| 3 | 2026-10-17 | done | `cargo fmt --all -- --check` exit 0; `just lint` exit 0 (after a `?` and a named fn-pointer type, pedantic lints); `cargo test -p tungsten-kit --locked` exit 0 (516 unit, 73 doctests; `overlap::grid::tests::` 14 with the normals); `cargo test -p tungsten --lib --locked kit` exit 0 (115); the eight named `kit_tests` listed → 8; hash pin rg → 1; bench test mode exit 0. Departures: a static collider a projectile starts inside meets it at once only when it moves in, with the push-out normal, and passes when it moves out, as `cast_shape` treats a start overlap, so a bolt fired from inside a static body leaves it and one left at a contact leaves its wall; hits at the wall's or the range's `t` itself still land; a projectile without a hitbox sweeps as a point and hits nothing; the steering homer seeks back to the wall it bounced off, so its next wall is the same one, which the test accepts. | `crates/tungsten-kit/src/overlap/{projectile.rs,grid.rs}`, `crates/tungsten-kit/src/tests/grid.rs`, `crates/tungsten/src/tests/kit.rs` |
| 4 | 2026-10-17 | done | `cargo fmt --all -- --check` exit 0; `just lint` exit 0 (after an `unwrap_or_else` in place of a single-arm `match`); `cargo test -p tungsten-kit --locked` exit 0 (517 unit, with a magnet test against a full scan of 40 collectors and 30 pickups in two teams, 73 doctests); `cargo test -p tungsten --lib --locked kit` exit 0 (119); the four named `kit_tests` listed → 4; hash pin rg → 1; bench test mode exit 0. Departures: the magnet keeps its own once-per-entity warning, since `drive.rs`'s skip reasons name no kinematic body for an adder that moves only bodiless and static roots and `drive.rs` is not a step 4 file; its test asserts that the body and the child hold, as the movers' skip tests do, not the log line; a collector's reach is measured from its hurtbox's centre; a pickup is never collected by its own entity. | `crates/tungsten-kit/src/overlap/pickup.rs`, `crates/tungsten-kit/src/tests/pickup.rs`, `crates/tungsten/src/tests/kit.rs` |
| 5 | 2026-10-17 | done | `cargo fmt --all -- --check` exit 0; `just lint` exit 0; `cargo test -p tungsten --lib --locked kit_recipes` exit 0 (62: coverage at 39 names and 4 kinds, the hash equal across two in-process runs); the sixteen recipes listed → 16, each at both dts; old pin rg → no match (exit 1); `cargo test -p tungsten-kit --locked` exit 0 (518 unit with the edge table, 23 M65 edges each built from its two plugins alone in both orders, and C's members in both whole-set orders; 73 doctests). The kit hash moved `0xd651_23fe_e583_03ee` → `0x5a73_d4a5_51f4_0bfa`: the script spawns C's prefabs after the rest (the ranger's embers through a ghoul and a knight, a ghoul at its side, an orb in reach; a fireball into a brick, a ricochet between two, a spiker beside the climber, a coin on it; two interceptors' lasers on a carrier and a bulwark; the adventurer's swings on a chest, spikes beside it, gold on it) and each row adds its shield. Departures: the script's check now also asserts that C's items acted (the climber's coin, the ranger's kill and xp, the adventurer's gold), and the shmup's expected score moves from 2 to 3, the carrier the lasers kill; the chest recipe asserts the roll's two draws, its gold at the chest and one count at both dts (one or more under the default seed) rather than a fixed count; the interval recipes run 62 steps, since `frames_for(61/60)` rounds 30.4999 frames down at 1/30. | `crates/tungsten/src/tests/kit_recipes/{mod.rs,top_down.rs,platformer.rs,shmup.rs,grid.rs,determinism.rs}`, `crates/tungsten-kit/src/tests/phases.rs` |
| 6 | 2026-10-17 | done: `health_tick` past the 1.5× guard, accepted as the death policy's cost | Test mode with the eight rows: `cargo test -p tungsten-kit --bench kit --locked` exit 0 (62 `Success`, 31 rows at both sizes). One quiet sitting in three blocking parts, owner disconnected, each `bench exit 0` under 600 s: part 1 21:59:09Z–22:02:30Z (201 s), part 2 22:02:34Z–22:06:23Z (229 s), part 3 22:06:29Z–22:10:01Z (212 s), and part 1 rerun once for the guard, 22:10:29Z–22:13:50Z; `pgrep -a -x 'nxcodec.bin|cargo|rustc'` before and after each printed nothing (`quiet`), the 1 Hz encoder poll saw none (`encoder seen during partN: 0`), newest tree mtime 1792272870 throughout. `perf-runs/20261017-m65-kit-criterion/summary.txt` is the rerun's summary (part 1's ten rows from the rerun, parts 2 and 3 from the sitting): `rg -c ' ratio '` → 31; the `awk` past 2.0 → prints nothing; ratios from 0.65 (`health_tick`) to 1.33 (`linear_motion`). C's rows, ns per entity at 1,000 / 10,000 and ratio: `hit_grid` 57.85 / 71.90, 1.24; `hitbox_tick` 372.32 / 412.90, 1.11; `contact_damage` 388.59 / 422.04, 1.09; `projectile_tick` 122.15 / 121.77, 1.00; `pickup_tick` 78.05 / 85.61, 1.10; `pickup_magnet` 258.67 / 282.09, 1.09; `damage_resolve` 153.56 / 152.26, 0.99; `shield_tick` 2.31 / 1.83, 0.79. Guard against `perf-runs/20261017-m64-kit-criterion/summary.txt`: `stats_apply` 52.97 / 50.87 against 38.50 / 37.42 (1.38× / 1.36×) and `cues` 223.48 / 219.65 against 223.10 / 223.37 (1.00× / 0.98×) within 1.5×; `health_tick` 1.47 / 0.96 against 0.87 / 0.49 (1.69× / 1.96×) past it, as in the first part 1 (1.46 / 0.95, 1.68× / 1.94×). Departure: §7's stop on the guard is waived on the owner's word, `health_tick`'s cost accepted as the death policy's: `Health` grew from 16 to 32 bytes (`despawn_on_death` and `death_left`, step 1) and `health_tick` now reads the entity column and checks each death delay, about 0.46 ns per entity and 140 ns per step from the two sizes; 10,000 entities still tick in under 10 µs a step. Landed: `crates/tungsten-kit/benches/kit.rs` from `step6-kept/kit.rs` (`cmp` exit 0); the profiling workflow's criterion paragraph at 31 rows and M65's 0.65–1.33; the bench rows in `D-175` (its bench bullet and a sentence of its `Why`), `DESIGN.md`'s M65 section, `w13-kit.md`'s revision sentence and the `CHANGELOG.md` line, which step 8 left without them. `perf-runs/` is gitignored, so the summary stays local beside M64's. | `crates/tungsten-kit/benches/kit.rs`, `perf-runs/20261017-m65-kit-criterion/summary.txt`, `docs/perf/profiling-workflow.md`, `DECISIONS.md`, `DESIGN.md`, `docs/plans/1.0/w13-kit.md`, `CHANGELOG.md`; the sitting's bench, both summaries and the four part logs: `/tmp/claude-1000/-home-joker-Projects-Tungsten/0c28b646-0983-47e3-aec0-3d7f87f0b896/scratchpad/step6-kept/` |
| 7 | 2026-10-17 | done | `cargo fmt --all -- --check` exit 0 (after `cargo fmt --all` on the template's files); `just lint` exit 0; `cargo test -p tungsten-template-basic --locked` exit 0 (19 passed); the `--list` count for `a_gem_drifts_to_the_player_and_counts_once: test$` → 1; `rg -c 'hazard_touch|Hazard\b' templates/basic/src templates/basic/tests templates/basic/AGENTS.md docs/getting-started.md` → no match (exit 1); `"contact_damage"` in `hazard.json` → 1, `"hurtbox"` in `player.json` → 1, `"pickup"` in `gem.json` → 1; `just template-check` exit 0 (19 passed outside the repository, smoke at 3 frames OK); `just repo-check` exit 0. The hazard is `contact_damage { damage 1, interval 0.5, knockback 420, filter team.player, shape circle 20 }` on its oscillator; the player gains `hurtbox { circle 8 }` and `stats { pickup_radius 96 }`; gameplay spawns three gems at `GEM_STARTS`, out of reach of the player's start; `components.rs` keeps only `Player`, `game.rs` keeps `setup` and `toggle_mute`, and the schedule pin drops `hazard_touch`. `place_on_hazard` puts the player a pixel right of the hazard's point. Beyond the change as written: the guide's layout list (`docs/getting-started.md:108`) no longer gives `game.rs` the hazard's touch. Follow-up, outside the plan's paths: `templates/basic/README.md:3` still calls `hazard_touch` the template's one piece of game Rust. | `templates/basic/`, `docs/getting-started.md` |
| 8 | 2026-10-17 | done | 175 still free (`rg` of `DECISIONS.md` gave `D-175` next). `rg -c '^## D-17[5] — W13j landed' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-17[5]:\*\*' DECISIONS.md` → 3 (`D-158`, `D-172`, `D-173`); the index row count → 1, and the three amended rows name `D-175`; `cargo test -p tungsten-core --test decision_index --locked` exit 0 (2 passed); `rg -c '^### Overlap and Damage — M65' DESIGN.md` → 1; the LLM index's range count → 1 at 12,282 bytes, unchanged; `just ctx` exit 0; the known issues' wait lines → no match (exit 1); `rg -c 'W13j \(M65' docs/plans/1.0/w04-api-freeze.md` → 2; `just api-check` exit 0 (no surface change after step 1, so no second `just api`); `just repo-check` exit 0; `git diff --check` exit 0. With step 6 skipped, `D-175`, `DESIGN.md`, the `w13-kit.md` revision sentence and the `CHANGELOG.md` line describe what landed and name no bench rows; a session that lands step 6 adds them there. The known issues gain C's limits as one row, the narrowed `prefab`-cue limit and the README follow-up; the backlog gains six rows (drops and crits, damage kinds on `Damaged`, walls, children's sweeps and the cell size, zones and `nearest` on the grid, `spawned` and `Lifetime`'s `expired`); `w13-kit.md` also marks the M63 and M64 follow-ups C settles. | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/{w04-api-freeze.md,w13-kit.md,backlog-1.x.md}`, `docs/LLM_INDEX.md`, `CHANGELOG.md` |
| 9 | 2026-10-18 | done | Workflow §5 closed: evidence rows 1–8 quoted; before the cut `git diff --quiet HEAD -- Cargo.lock` exit 0, the core, render and umbrella snapshots unchanged (exit 0), `plugins.rs`' `DefaultPlugins` pin unchanged (exit 0), no core or render file changed (exit 0), no umbrella source outside its tests; `just release-check` consistent at 0.84.0 with `[Unreleased]` entries; `DESIGN.md`'s status line gains C; `CHANGELOG.md`'s `[Unreleased]` takes a `Summary:`, Added and Changed from the plan's line; the register row records 0.85 and `D-175`; this plan `status: done` and archived. Before this step, step 9's next prompt moved from W13k's plan to W13 damage types' (`docs/plans/1.0/phase6-milestone-66-kit-damage-types.md`), the register's next row since the 2026-10-18 roadmap amendment. The cut, every release check after it with `just physics-release` and the preflight are quoted in the session's report, the plan being archived before the cut. | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, this plan |

## Follow-ups

- Damage kinds on `Damaged` through a `Copy` handle (Q1).
- A static-only or layered wall query in core, with W6b's layers and masks (Q2), and walls for plain hitboxes (Q3).
- A `spawned` cue event (Q7); `expired` from `Lifetime` (Q8).
- Drop scatter; crit chances on hitboxes; a grid cell size per game (Q12).
- A child's hit shape swept from its root's motion (Q15).
- Zones and the nearest search on the grid (`docs/known-issues.md:54-55`).
- HUD text for counters (w13-kit P12, 1.x).
