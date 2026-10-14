# Phase 6 milestone 49: kinematic bodies, sensors and spatial queries (W6a) — done

- **status:** done
- **goal:** Kinematic bodies and sensors in core physics, with contact events between kinematic and dynamic bodies and sensor enter events per pair; ray and shape casts and overlap and nearest queries against bodies and tile layers, with the hit point and normal; tile collision for every body kind and query; the `rigid_body` scene component and the physics debug overlay know the new kinds; the crowd probe measured before the kit's mover builds on it (`D-132`).
- **non-goals:** Layers and masks (W6b; the filter takes them later without a break); audio (W6c); controllers, trigger zones and the path follower (W13c, W13d); bodies on child entities (roots only, `D-132`); swept sensors and sensor exit events (no user yet); friction, rotation or new shapes; a benchmark workload with kinematic bodies or sensors (G1 and G2 in the bench are a workload change); the touching-static-box seam (known issue); the `contact_hertz` lever (1.x backlog).
- **files to touch:** Step 1: `crates/tungsten-core/src/physics/{components,events,mod}.rs`, `crates/tungsten-core/src/physics/step/{mod,gather,pairs,sweep,arrival,sleep}.rs` (`sleep.rs` added at the run with the owner's approval), `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten/src/tests/fixed_step.rs`, `examples/01_platformer/src/tests/{ice,ball_pit,player,hazards,burning,brick}.rs`. Step 2: `crates/tungsten-core/src/physics/query.rs` (new), `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/physics/query.rs` (new). Step 3: `crates/tungsten-core/src/physics/{components,events,mod,plugin,query}.rs`, `crates/tungsten-core/src/physics/step/{mod,gather}.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/physics/{step,query,plugin}.rs`. Step 4: `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/component_registry.rs`, `crates/tungsten-core/src/tests/physics/plugin.rs`, `crates/tungsten/src/physics_debug.rs`, `crates/tungsten/src/tests/physics_debug.rs`. Step 5: `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/step/{mod,gather,sleep}.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/tests/physics_containment.rs` (the probe swapped in for one check, then restored from `HEAD`). Step 6: `crates/tungsten-core/tests/physics_containment.rs`. Step 7: none in the tree. Step 8: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/perf/benchmarks.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w06-gameplay-systems.md`, `api/tungsten-core.txt`, `api/tungsten.txt`. Step 9: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (register row), `docs/plans/1.0/README.md` (owned Now fields), this plan (archived).
- **ordered steps:** (1) Kinematic bodies. (2) The spatial index, queries and casts. (3) Sensors and their enter events. (4) The scene component's kinds and the debug overlay. (5) Kinematic contacts under a crowd. (6) The crowd probe. (7) The perf sitting. (8) Decision, docs, ledger and API snapshot. (9) Release 0.68.
- **done-when:** Every step's evidence row quoted; workflow §5 closed; the release checks, with `just visual` (no step runs it), pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W6a ([implementation plan](../../1.0/implementation-plan.md) §4 card, level B), milestone M49, release 0.68. Written 2026-10-13 at `026f9e4` on branch `0.68`, with another session's uncommitted edits to the tungsten-milestone skill, its skeleton and `CHANGELOG.md` in the tree. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Scoping source: [criteria](../../1.0/criteria.md) §8.1, since W6 has not graduated ([w06](../../1.0/w06-gameplay-systems.md) holds its placement), with the tiers of the [acceptance game](../../1.0/acceptance-game.md#proposed-tiers) (`D-132`). Amended 2026-10-13, after the crowd probe's first run stopped the run: step 5 fixes the two kinematic contact failures it found (§2.8), and the old steps 5–8 are now 6–9.

## Context digest

W6a is the register row after W16a (M48, 0.67). The acceptance game's mover, blades, bolts, gems and wand need what physics lacks: `BodyKind` is `Static` or `Dynamic`, and there is no sensor and no world query (criteria §2.1). `D-132` made these Must: kinematic bodies with contact events against dynamic bodies, sensors with enter events per pair, nearest and overlap queries, and shape and ray casts against bodies and tile layers with the hit point and normal. It also asked this plan to probe a crowd pressing on a kinematic player from every side, over several spawn orders and frame dts, before W13c's mover lands.

It stands on the hand-rolled step (`D-033`): one gather per frame into dense proxies and a positional writeback (`D-066`), speculative contacts at fixed substeps (`D-064`), persistent pairs under travel budgets with repairs (`D-075`, `D-081`), the arrival pass (`D-092`), island sleeping and its wake paths (`D-065`), the step bound and the fixed step (`D-094`, `D-137`). The design keeps every existing body on the path it takes today, so the determinism and containment hashes and the eight row digests stay put; new work runs only when a kinematic body, a sensor or a query exists.

Later candidates build on it. W6b adds layers and masks to bodies, sensors and `SpatialFilter`, whose fields are private, so the addition is not a break. W13c moves a kinematic body by its velocity in `fixed_update` before `physics_step`, casting its shape against tiles and bodies. W13d's trigger zones read `SensorEntered`. The game's bolts are sensors that cast against tile layers to stop or bounce.

## 1. Audit

Read at `026f9e4`. `step/…` is `crates/tungsten-core/src/physics/step/…`; `physics/…` and `ecs/…` are under `crates/tungsten-core/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `BodyKind` has two variants and no `#[non_exhaustive]`; `RigidBody` and `RigidBodyBundle` have `dynamic` and `r#static` constructors only | `physics/components.rs:83-88`, `:98-130`, `:152-188` |
| A2 | `CollisionEvent` has four public fields and no constructor; ten struct literals outside core build it | `physics/events.rs:6-13`; `examples/01_platformer/src/tests/ice.rs:437`, `:486`, `ball_pit.rs:222`, `:275`, `player.rs:64`, `hazards.rs:55`, `burning.rs:26`, `brick.rs:138`, `:343`; `crates/tungsten/src/tests/fixed_step.rs:174` |
| A3 | An integration test matches `BodyKind` exhaustively twice; it compiles as its own crate, so a `#[non_exhaustive]` enum needs a wildcard arm there | `crates/tungsten-core/tests/physics_tunneling.rs:78-82`, `:518-524` |
| A4 | The gather maps a kind to `(is_dynamic, inv_mass, restitution)`: every kind but `Dynamic` is immovable with inverse mass 0. Tile proxies follow the entity proxies: `entity: None`, a face mask from solid neighbours, a full-map scan each frame | `step/gather.rs:44-55`, `:77-150` |
| A5 | Collider-less bodies move in `integrate_loose_bodies`, `Dynamic` only | `step/mod.rs:427-441` |
| A6 | Gravity, position integration and the travel-budget sum run for awake dynamic proxies only | `step/mod.rs:597-604`, `:630-640`, `:684-688` |
| A7 | `write_back` skips every non-dynamic proxy and zips proxies positionally with the query's rows, debug-asserted | `step/mod.rs:400-421`, `:412-415` |
| A8 | The narrow-phase loop takes `a` to be an awake dynamic body: the wake test asserts it, the island union checks only `b`, and the event names `a` as the pushed body. A pair whose inverse masses sum to 0 is dropped before its event | `step/mod.rs:515`, `:573-580`, `:581-590`, `:537-540` |
| A9 | Pair building lets awake dynamic proxies (`PAIR_AWAKE`) initiate queries, inflates them by their travel and every other proxy by `2·linear_slop`, and counts every non-dynamic proxy as a static; repairs keep that ownership through `PAIR_AWAKE`; `collect_tripped` scans awake dynamic proxies | `step/pairs.rs:183-220`, `:184`, `:355-365`, `:249-257` |
| A10 | The statics-only sweep grid stages every non-dynamic proxy once a frame ("Statics never move within a frame"), and the safety net skips only dynamic targets, so a moving kinematic proxy would be a wall at its current point | `step/sweep.rs:51-70`, `:120-125` |
| A11 | `arrival_inv` gives a non-dynamic target inverse mass 0, right for a kinematic one; `arrival_center` gives it `center`, which for a proxy that moved this substep is where it ended, not where it began | `step/arrival.rs:122-141` |
| A12 | Sleep entries and islands cover dynamic proxies only | `step/sleep.rs:65`, `:281`, `:300`, `:323` |
| A13 | `SpatialGrid::query` and `for_each_in` take `&mut self` (lazy build, dedupe marks). The pair grid's inflated boxes cover a proxy's travel only up to the budget check before each substep's narrow phase, and the last substep's solve and arrival pass can still move it, so they do not index final positions exactly | `physics/broadphase.rs:156-227`; `step/pairs.rs:125-144`, `:254` |
| A14 | `sweep_aabb_vs_aabb` is the only cast primitive (box against box, start overlaps ignored) and serves the safety net; no ray-against-circle or rounded-box cast exists | `physics/collision.rs:217-252`; `step/sweep.rs:133` |
| A15 | A World resource holding `RefCell` buffers behind `&World` is the extract's precedent (`D-086`) | `crates/tungsten/src/sprite_extract.rs:252-272` |
| A16 | `World::is_alive` tells a despawned entity | `ecs/world.rs:147` |
| A17 | Core already exports an ECS `QueryFilter` trait, so the physics filter needs another name | `ecs/query.rs:357` |
| A18 | `PhysicsPlugin` registers `EventQueue<CollisionEvent>` and `rigid_body`; the scene kind is `dynamic` or `static`, `mass` is dynamic-only, and `velocity` defaults to zero on a dynamic body and to none on a static one; the registry's module doc holds the JSON shape | `physics/plugin.rs:37-50`, `:104-110`, `:189-208`; `crates/tungsten-core/src/component_registry.rs:27` |
| A19 | `PhysicsDebugOverlay` has four public fields and colours outlines by shape; its query reads no `RigidBody`; its tests build it with `..Default::default()` | `crates/tungsten/src/physics_debug.rs:9-24`, `:70-96`; `crates/tungsten/src/tests/physics_debug.rs:24`, `:48` |
| A20 | `Proxy`'s fields take 76 bytes and it is 80 with padding (the pair flags' comment relies on 80), so two `bool`s fit without growing it | `step/mod.rs:100-124`, `:193-194` |
| A21 | Pinned invariants: the determinism hash, the containment hash and the bench rows' digests | `crates/tungsten-core/tests/physics_determinism.rs:75`; `crates/tungsten-core/tests/physics_containment.rs:246`; `docs/perf/benchmarks.md:117` |
| A22 | `just physics-release` names its three test files, so a new release test goes in one of them | `justfile:33-34` |
| A23 | The known issue on touching static boxes (entity colliders keep every face) applies to casts too; the bounded-load pile is the crowd's nearest analogue | `docs/known-issues.md:15`, `:25` |
| A24 | The template's schedule snapshot lists `fixed_update: physics_prev_snapshot, physics_step`; its player is not a body | `templates/basic/tests/game.rs:47-50`; `templates/basic/src/states.rs:77` |
| A25 | The LLM index is at 12,282 of its 12,288 bytes; its physics row is 252 bytes | `docs/LLM_INDEX.md:48`; `scripts/check-agent-context.py:25` |
| A26 | G1 and G2 are open proposals in the benchmark doc; `integrated` wires `physics_step` itself in `update` | `docs/perf/benchmarks.md:480-481`; `examples/02_bench/src/integrated.rs:353` |
| A27 | `physics_step` fixes the substep count from `PhysicsConfig::substeps` after the step bound and before the gather; the gather matches each body's kind | `step/mod.rs:424-434`, `:443-448`; `step/gather.rs:77` |
| A28 | `soft_params` caps a contact's stiffness at a quarter of the substep rate. Contacts against immovables ask for twice `contact_hertz`, 60 Hz by default, so a 1/120 s substep (1/30 s over four) holds them to 30 Hz, the stiffness of the crowd's own contacts; a 1/240 s substep keeps 60 Hz | `step/solver.rs:21-23`; `step/mod.rs:760-766`; `physics/mod.rs:43-54`, `:88-92` |
| A29 | The wake test wakes a sleeper at any speed only when `a` is a moving kinematic body; a dynamic `a` needs the 20 px/s threshold, and a sleeper below it is solved at inverse mass 0 | `step/mod.rs:644-673`, `:674-684` |
| A30 | `kinematic_touch` marks the pushed body `kept_awake`, and the frame-end pass holds that body's island awake; kinematic contacts join no island, and touching awake dynamic pairs union only with sleeping on | `step/mod.rs:715-729`, `:875-889`; `step/sleep.rs:195-216`, `:286-297` |
| A31 | The arrival pass's backward sweep skips a pair whose mover has inverse mass 0 there, a kinematic mover among them; the pass lists only bodies whose solved velocity moved by more than `ARRIVAL_SLOPS · linear_slop` of travel per substep (60 px/s at 1/120 s) | `step/arrival.rs:356-367`, `:12-16`, `:123-131` |
| A32 | Smoke and capture runs, the bench rows among them, step at a pinned 1/60 s, and the fixed loop defaults to 1/60 s | `crates/tungsten/src/app.rs:60`; `crates/tungsten-core/src/time.rs:115` |

A27–A32 were read for step 5 on 2026-10-13, at `026f9e4` with steps 1–4 in the tree.

## 2. Design

### 2.1 Body kinds

| Kind | Moves | Solver | Contacts and events | Sleep |
| --- | --- | --- | --- | --- |
| `Static` | Never | The immovable side | `CollisionEvent` with dynamic bodies | — |
| `Dynamic` | Gravity, velocity, contacts | Yes | With every kind but sensors, and with tiles | Islands (`D-065`) |
| `Kinematic` | By its `Velocity` each substep; no gravity; never by a contact | The immovable side: inverse mass 0, static softness | `CollisionEvent` with dynamic bodies only, `a` the dynamic body; none with static or kinematic bodies or tiles (Q1) | Never; moving at any speed, it wakes a sleeper it touches or closes on and keeps awake a body touching it (2.3), whose island wakes the sleepers it touches or closes on (2.8) |
| `Sensor` | As a kinematic body | Never | No `CollisionEvent`; `SensorEntered` at the step's end for each static, dynamic or kinematic body or solid tile it begins to overlap; sensors do not detect sensors (Q2–Q4) | Never |

- Tile collision for every kind: dynamic bodies through contacts, as today; sensors through `SensorEntered` (`other: None`); kinematic bodies, and games, through casts and queries, which see tile layers. A mover is blocked by walls that way (Q1).
- A `Position` write moves a kinematic body or a sensor as a teleport: no contact sees the jump and it wakes nothing. Moving by `Velocity` is what pushes and wakes. A collider-less kinematic body or sensor moves by its velocity in `integrate_loose_bodies`.
- Bodies stay on root entities (`D-132`). Every bundle body keeps `PrevPosition`, so kinematic bodies and sensors are drawn interpolated.

### 2.2 Public surface

The umbrella re-exports `tungsten_core::physics` (`crates/tungsten/src/lib.rs:66`), so games reach all of this as `tungsten::physics::…`. Every new public item gets rustdoc.

```rust
// physics/components.rs
#[non_exhaustive]
pub enum BodyKind { Static, Dynamic, Kinematic, Sensor }
impl RigidBody {
    pub fn kinematic() -> Self; // inverse mass 0, restitution 0
    pub fn sensor() -> Self;    // inverse mass 0, restitution 0
}
impl RigidBodyBundle {
    pub fn kinematic(position: Position, collider: Collider) -> Self; // with Velocity(ZERO)
    pub fn sensor(position: Position, collider: Collider) -> Self;    // with Velocity(ZERO)
}

// physics/events.rs
#[non_exhaustive] // fields stay public to read; `a` is the body the contact pushed
pub struct CollisionEvent { pub a: Entity, pub b: Option<Entity>, pub normal: Vec2, pub penetration: f32 }
impl CollisionEvent { pub fn new(a: Entity, b: Option<Entity>, normal: Vec2, penetration: f32) -> Self; }

/// A sensor began to overlap a body (`other`) or a solid tile (`other: None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SensorEntered { pub sensor: Entity, pub other: Option<Entity> }
impl SensorEntered { pub fn new(sensor: Entity, other: Option<Entity>) -> Self; }

// physics/query.rs: a private module; `physics` re-exports all of it, the crate root the two types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialFilter { /* private: exclude, bodies, sensors, tiles */ }
impl Default for SpatialFilter { /* bodies and tiles, no sensors, nothing excluded */ }
impl SpatialFilter {
    pub fn new() -> Self;
    pub fn excluding(self, entity: Entity) -> Self;
    pub fn with_bodies(self, on: bool) -> Self; // static, dynamic and kinematic bodies
    pub fn with_sensors(self, on: bool) -> Self;
    pub fn with_tiles(self, on: bool) -> Self;  // solid tiles of collision layers
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SpatialHit { pub entity: Option<Entity>, pub point: Vec2, pub normal: Vec2, pub distance: f32 }

pub fn cast_ray(world: &World, origin: Vec2, direction: Vec2, max_distance: f32, filter: &SpatialFilter) -> Option<SpatialHit>;
pub fn cast_shape(world: &World, shape: Shape, origin: Vec2, direction: Vec2, max_distance: f32, filter: &SpatialFilter) -> Option<SpatialHit>;
pub fn overlap(world: &World, shape: Shape, center: Vec2, filter: &SpatialFilter, out: &mut Vec<Option<Entity>>);
pub fn nearest(world: &World, point: Vec2, max_distance: f32, filter: &SpatialFilter) -> Option<SpatialHit>;

// crates/tungsten/src/physics_debug.rs: two more public fields
pub struct PhysicsDebugOverlay { /* enabled, color_aabb, color_circle, */ pub color_kinematic: [f32; 4], pub color_sensor: [f32; 4] /* , thickness */ }
```

`SpatialFilter` and `SpatialHit` sit beside `SpatialGrid`, since the ECS already has a `QueryFilter` (A17). `SpatialFilter`'s fields are private, so W6b's layers and masks add builder methods without a break; `SpatialHit` and both events are `#[non_exhaustive]` with public fields. `CollisionEvent` stays `Copy` with public fields, and `new` replaces the ten literals (A2).

### 2.3 Inside the step

- `Proxy` gains `kinematic: bool` and stays 80 bytes (A20; a unit test pins `size_of`). A proxy *moves* in a substep when it is an awake dynamic one or a kinematic one.
- Gather (A4): a `Kinematic` row becomes a proxy with `is_dynamic: false`, inverse mass 0 and `kinematic: true`. A `Sensor` row becomes no proxy: it joins a `sensors` list in `PhysicsBuffers` (entity, key, centre, shape, velocity), so no substep loop, pair, solve or sweep sees it. In a frame with sensors, the gather also records each tile proxy's tilemap entity, layer and cell beside it, the identity the sensor history gives a tile (2.5); a frame without sensors records nothing more.
- Substeps: gravity stays dynamic-only (A6); position integration and the budget sum take every moving proxy, and only dynamic ones are listed for the arrival pass. Pair building makes a kinematic proxy a `PAIR_AWAKE` initiator inflated by its travel, which the existing ownership rules deduplicate (A9). It pairs with everything; the narrow phase drops its immovable pairs before any event, as it drops static ones today (A8). It counts as awake in `collect_tripped` and never trips, since its velocity cannot change within a frame. The statics exclude it: the statics-only grid does not stage it, the static count skips it and the safety net never clamps against it (A9, A10). `arrival_center` gives it `prev_center` (A11).
- Narrow phase (A8): the island union also requires `a` to be dynamic. When `a` is kinematic, the event swaps `a` and `b` and negates the normal, so `a` is always the pushed body. The wake test gains a kinematic rule: a sleeper paired with a kinematic body whose velocity is not zero wakes when the pair touches or its gap closes, at any speed. The test's speed threshold (`step/mod.rs:517`, 20 px/s by default) would otherwise leave the sleeper at inverse mass 0 against an inverse mass of 0, so the pair is dropped (`:537-540`) and a slow kinematic body passes through it. A dynamic body touching a moving kinematic body marks its island kept awake for the step, and the frame-end pass sleeps no island so marked, whatever `time_to_sleep` is. Resetting the body's timer alone would not hold once `time_to_sleep` is shorter than the step, since the pass adds the step's dt before it compares (`step/sleep.rs:286`, `:328`). Box2D keeps such an island awake the same way. A kinematic body with zero velocity wakes nothing, like a static one.
- Writeback (A7): a kinematic proxy writes its `Position`, not its `Velocity`. A sensor row, found by its `RigidBody` kind, pulls no proxy, moves by `velocity · dt` and stores its new centre in `sensors`.
- After the writeback: the spatial index is marked stale; when `sensors` is not empty, the index is built and the sensor pass (2.5) runs, and when it is empty the sensor history is emptied. Collision events drain first, then sensor events.
- No existing function in `collision.rs` or `broadphase.rs` changes (A14): casts bring their own geometry, so nothing they add can reach the hashes.

### 2.4 The spatial index and queries

- One grid over the exact world boxes (no inflation) of every proxy and sensor at the step's final positions (A13), held in a private `World` resource with `RefCell` state (A15). `PhysicsBuffers` keeps its auto traits, and queries take `&World`. `physics_step` inserts the resource, marks it stale each step and builds it when sensors exist; otherwise the first query after a step builds it. A borrow already taken, which no callback can cause, builds a temporary index instead.
- Queries see the last step: a body moved since is found where the step left it, one spawned since is not found, and one despawned since is skipped (`World::is_alive`, A16). Before the first step they find nothing.
- `cast_ray` and `cast_shape` take a finite nonzero direction (normalized inside) and a finite `max_distance >= 0`, or return `None`. A hit's `distance` is how far the ray or the shape's centre travels, `point` is where the shapes first touch, and `normal` is the hit surface's outward unit normal. A ray ignores a shape that contains its origin. A shape cast that starts overlapping a body hits it at distance 0 when the direction points into it (against the narrow phase's push-out normal) and otherwise passes it, so a mover can always leave an overlap and never sink deeper (Q7).
- Tiles: a face the tile's mask marks internal is never hit, nor a corner one of whose faces is internal, as `D-064`'s contacts treat seams, so a cast along a tile floor does not catch. Entity colliders keep every face (A23).
- `overlap` lists, in index order, every body and tile whose shape overlaps the given shape at `center` with penetration above 0, a tile as `None` (Q9). `nearest` finds the body or tile whose surface is closest to `point` within `max_distance`; `point` is the closest surface point, and a `point` inside a shape gives distance 0 and a zero normal.
- Ties go to the lower index (proxies in gather order, then sensors), so every result is deterministic.

### 2.5 Sensor events

At the end of a step with sensors, each sensor, in gather order, tests the index for the bodies and tiles it overlaps at final positions (penetration above 0), skipping sensors. An overlapping pair's key is ordered, the sensor's warm-start key in the high half and the other's identity in the low half, not the order-independent `pair_key` (`step/pairs.rs:71-76`), so two overlapping entities that swap which one is the sensor (`RigidBody.kind` is public) make a new pair. A body's identity is its warm-start key; a tile's is a hash of its tilemap entity, layer and cell, not its warm-start key, which hashes the tile's centre (`step/gather.rs:17-24`): a map whose public `origin` moves keeps its pairs, and a tile of another layer at the same cell is a new one. A key is new when the previous step's sorted key list lacks it. Each new pair sends `SensorEntered`, in sensor order and then index order, and the list is replaced. A step that runs with no sensor empties the list, so a sensor that comes back reports what it overlaps again. A pair that stays overlapped sends nothing more; one that separates and overlaps again sends again; a step that does not run (game dt 0) changes nothing. A sensor that moves farther in one step than its size plus a target's can miss it (Q3); `cast_shape` covers that case.

### 2.6 Scene component and debug overlay

- `rigid_body.kind` takes `"kinematic"` and `"sensor"`. `mass` stays dynamic-only, with the error it has; `velocity` defaults to `[0, 0]` on dynamic, kinematic and sensor bodies and to none on a static one; `restitution` is accepted on every kind and does nothing on a sensor (Q14). The registry's module doc row says so. Scene files are file format under `D-103`; this is an addition.
- The overlay outlines kinematic bodies in `color_kinematic` and sensors in `color_sensor`, whatever their shape; static and dynamic bodies and colliders without a body keep the shape colours. Its query adds `Option<&RigidBody>`.

### 2.7 Seam

Core holds everything but the overlay, which is the umbrella's. No render or extract change, no new system or stage (A24's snapshots stay), no new dependency. `PhysicsPlugin` registers `EventQueue<SensorEntered>` beside the collision queue.

### 2.8 Kinematic contacts under a crowd (step 5)

The crowd probe's first run, on 2026-10-13 as step 5 before this step was added, stopped the run (§7). Its logs, the probe and a diagnostic copy with substep and sleep switches are in `perf-runs/20261013-m49-kinematic/step5/` (machine-local). It failed twice:

- **Press.** With four substeps, at each dt of 1/30 s and above (0.05 and 0.1 s step as 1/30 s under the bound), four of the six press configurations put a crowd centre inside the walking body: Box BodyFirst for 17 steps from step 314, deepest 12.44 px; Box BodyLast 19 from 304, 11.33 px; Box Shuffled 15 from 306, 13.75 px; Circle BodyFirst 23 from 306, 16.76 px. Circle BodyLast reached 10.00 px and Circle Shuffled 8.46 px without crossing. At 1/60 s every case held, deepest 2.69 to 5.81 px (Circle BodyLast 5.81, Box BodyLast 5.76 and Box Shuffled 5.01 over Q12's 5 px). With eight substeps no case put a centre inside at any dt: deepest 1.59 to 2.98 px, and 1.62 to 2.74 px at 1/30 s and above.
- **Slow walk.** No centre went inside, but the walk at 10 px/s sank 7.70 px at 1/60 s and 9.48 px at 1/30 s and above, against 0.37 and 1.03 px at 150 px/s; with sleeping off it sank 0.26 to 0.27 px. A body the kinematic body pushes slower than the wake threshold leaves the sleeper ahead asleep, and the sleeper holds it at inverse mass 0 against the kinematic body's (A29).

In the pin, a recorded limit (Q12), the deepest penetration was 20.55 to 21.77 px, and the pinned body's centre crossed the static box's face at every dt and the tile wall's at none. The first failing assertion read `press Circle BodyFirst dt 0.033333335: a crowd centre -0.37952232 px inside the body at step 306`.

The press fits A28: at 1/30 s over four substeps the kinematic contacts lose the doubled stiffness they have at 1/60 s, and eight substeps give it back. Of the levers the first run named, step 5 takes two:

- **More substeps in a world with a kinematic body (taken, Q15).** A step whose gather staged a kinematic proxy, moving or still, runs `clamp(⌈substeps · dt · 60 − 0.001⌉, substeps, 2 · substeps)` substeps, counted after the gather: `substeps` at 1/60 s and shorter, six at 1/45 s and eight at 1/30 s with the default four. No substep is then longer than a 1/60 s step's up to the default bound, and no step runs more than twice `substeps` past it (a larger or unbounded `max_step_dt`); the 0.001 keeps rounding at 1/60 and 1/30 s from adding a substep. The gather counts kinematic proxies in its kinematic arm alone. A world without one keeps `substeps` at every dt, so the pinned hashes, the row digests and every test without a kinematic body take today's path (A32). In the probe at 1/30 s and above, the press runs the diagnostic's eight-substep arithmetic from its first step. A still kinematic body counts, so one that starts or stops moving leaves the world's substep count as it was. Cost: such a world's 1/30 s step costs about two 1/60 s steps (the press: about 330 µs a step with four substeps, 540 with eight); the default fixed step never takes the rule. `D-064` fixed the count to end a heuristic that one fast body could drive to eight times a step's cost; this count depends on the world and the dt alone and at most doubles, an amendment to `D-064` for these worlds (§5).
- **A pressed chain moved with its kinematic pusher in the arrival pass (not taken).** `D-092`'s backward sweep skips a mover it cannot slow, a kinematic one among them (A31). Moving that mover's pressed chain with it would be kinematic-only and cheap, but no run has tested it, and the pass sees only bodies whose solved velocity moved by more than its threshold, which a body held between the pusher and the crowd may never do. It stays a follow-up.
- **The kinematic wake carried through the island it touches (taken, the walk's only lever, Q16).** A sleeper wakes at any speed when an awake dynamic body touches it or closes on it while that body's island so far this frame (`D-065`'s union of touching contacts, accumulated across substeps) holds a body touching a moving kinematic body (`kept_awake`, A30). This is §2.3's rule for the kinematic body itself, reaching what it presses, so a row of sleepers wakes body by body as the press reaches it; elsewhere the threshold stands. Touching counts, as in the kinematic rule, since the solve can leave a pressed body at rest against the sleeper ahead. So a sleeper resting against such an island wakes and joins it, and a sleeping pile the island touches wakes and stays awake while the kinematic body moves: the whole-island wake `D-065` declined for contacts in general, here only through a moving kinematic body. A frame in which no moving kinematic body touches a body adds at most one test per sleeper pair; the bench rows hold no kinematic body.

Neither rule changes the solver, `soft_params` or `contact_hertz` (the 1.x lever), the arrival pass, the pin case (a body pressed between a kinematic body and a static one still crosses its face, Q12) or a signature; the rustdoc of `PhysicsConfig::substeps` and `max_step_dt` states the substep rule.

## 3. Steps

### Step 1: Kinematic bodies

- **Files:** `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/events.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/sleep.rs` (added at the run: §2.3's frame-end pass), `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten/src/tests/fixed_step.rs`, `examples/01_platformer/src/tests/ice.rs`, `examples/01_platformer/src/tests/ball_pit.rs`, `examples/01_platformer/src/tests/player.rs`, `examples/01_platformer/src/tests/hazards.rs`, `examples/01_platformer/src/tests/burning.rs`, `examples/01_platformer/src/tests/brick.rs`
- **Change:** (a) `BodyKind` takes `#[non_exhaustive]` and `Kinematic`; `RigidBody::kinematic` and `RigidBodyBundle::kinematic`, with rustdoc stating 2.1's rules. (b) `CollisionEvent` takes `#[non_exhaustive]` and `new`, its rustdoc saying `a` is the pushed body; the ten literals (A2) call `new`; the two matches (A3) gain a wildcard arm. (c) 2.3's step changes for kinematic proxies, in the order gather, pairs, substep, sweep, arrival, writeback; the module docs of `physics` and `physics::step` name the kind. (d) Tests appended to `tests/physics/step.rs`, each name containing `kinematic`: one moves by its velocity without gravity and ends, bit for bit, where it ends in a world with nothing else; it pushes a dynamic body and is never pushed; its contact event names the dynamic body as `a`, with the kinematic body spawned first and last; it passes a static body, a tile and another kinematic body with no contact and no event; moving into a sleeping body, and away from one resting on it, wakes it; moving into a sleeping body at 10 px/s (half the sleep threshold) and at exactly 20 px/s wakes it and never passes it, with the sleeper's centre ahead of the kinematic body's leading face throughout; a body resting on a kinematic platform that rises at 10 px/s under gravity stays awake for 2 s, and so it does with `time_to_sleep` at 0.01 s (shorter than the step) and at 0; a body resting on a kinematic body with zero velocity falls asleep; `persistent_pairs_match_fresh_contacts_with_kinematic_bodies`, where the oracle `assert_pair_contacts` lets a kinematic proxy initiate as an awake one does; a collider-less one moves; `Proxy` stays 80 bytes with the flag.
- **Done-when:** `cargo check --workspace --all-targets --locked` → finishes without an error; `cargo test -p tungsten-core --locked --lib kinematic` → at least 13 passed, 0 failed; `cargo test -p tungsten-core --locked --lib physics` → 0 failed; `cargo test -p tungsten-core --locked --test physics_tunneling` → 0 failed; `cargo test -p example-01-platformer --locked` → 0 failed; `cargo test -p tungsten --locked --lib fixed_step` → 0 failed; `rg -c 'CollisionEvent \{' examples crates/tungsten/src` → no match (exit 1); `just fmt-check lint` → passes; `just physics-release` → 0 failed, 0 ignored, with `physics_step_is_bit_identical_across_runs` and `pinned_step_state_is_unchanged` among the passes.
- **Moves:** none.

### Step 2: The spatial index, queries and casts

- **Files:** `crates/tungsten-core/src/physics/query.rs` (new), `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/physics/query.rs` (new)
- **Change:** 2.4's index resource, `SpatialFilter`, `SpatialHit` and the four functions in `query.rs`, with their cast and distance geometry (a ray, a box or a circle against a box, a circle, and the rounded box a circle sweeps past a box), rustdoc on each item and one example in the module doc; `physics_step` marks the index stale. `query.rs` mounts its tests from `tests/physics/query.rs` (`#[cfg(test)]` and `#[path]`, as `step/mod.rs` does). Tests: a ray's point, normal and distance against a box, a circle and a tile; `max_distance` respected; a ray from inside a shape passing it; a zero or non-finite direction returning `None`; box and circle casts against boxes, circles and tiles; a cast that starts overlapping stopping at 0 into the body and passing out of it; a cast along a row of tiles catching on no seam; `overlap` strict and in index order; `excluding`, `with_bodies` and `with_tiles`; a despawned entity skipped; a body spawned after the step not found, one moved by a step found at its new place, nothing before the first step; `nearest` by surface distance, 0 inside, ties to the lower index; two identical worlds answering alike; the index built at most once per step (a `cfg(test)` build counter).
- **Done-when:** `cargo test -p tungsten-core --locked --lib physics::query` → at least 14 passed, 0 failed; `cargo test -p tungsten-core --locked --doc physics` → 0 failed; `cargo check --workspace --all-targets --locked` → finishes without an error; `just fmt-check lint` → passes; `just physics-release` → 0 failed, 0 ignored, both pinned-hash tests among the passes.
- **Moves:** none.

### Step 3: Sensors and their enter events

- **Files:** `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/events.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/physics/query.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `crates/tungsten-core/src/tests/physics/plugin.rs`
- **Needs:** step 2 (the index).
- **Change:** `BodyKind::Sensor`, `RigidBody::sensor`, `RigidBodyBundle::sensor` and `SensorEntered`, re-exported from `physics` and the crate root; the `sensors` list, the writeback's sensor rows, sensors in the index behind `with_sensors`, and 2.5's pass; `PhysicsPlugin` registers `EventQueue<SensorEntered>`. Tests, each name containing `sensor`: one moves by its velocity while a dynamic body passes through it with no `CollisionEvent`; one `SensorEntered` per overlap against a dynamic, a kinematic, a static and a sleeping body and a tile (`other: None`), and one more after it separates and overlaps again; none against another sensor; a sensor spawned overlapping a body reports it on the first step; two overlapping entities that swap which one is the sensor send one event for the new sensor; the only sensor turned into a body for one step and back, still overlapping, sends one more event; a sensor resting on a tile of a map whose `origin` moves each step sends one event; a tile replaced by another layer's tile at the same cell sends one more; a paused clock sends nothing; the events follow sensor order, then index order; queries see a sensor only `with_sensors(true)`; the plugin registers the queue; two identical runs send identical events.
- **Done-when:** `cargo test -p tungsten-core --locked --lib sensor` → at least 14 passed, 0 failed; `cargo test -p tungsten-core --locked --lib physics` → 0 failed; `cargo check --workspace --all-targets --locked` → finishes without an error; `just fmt-check lint` → passes; `just physics-release` → 0 failed, 0 ignored, both pinned-hash tests among the passes.
- **Moves:** none.

### Step 4: The scene component's kinds and the debug overlay

- **Files:** `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/component_registry.rs`, `crates/tungsten-core/src/tests/physics/plugin.rs`, `crates/tungsten/src/physics_debug.rs`, `crates/tungsten/src/tests/physics_debug.rs`
- **Needs:** step 1; its sensor half, step 3. Without step 3, only `"kinematic"` and `color_kinematic` land.
- **Change:** 2.6. Tests: `"kinematic"` and `"sensor"` entries spawn their bundles with a zero `Velocity`; `mass` on either fails, naming `mass`; an unknown kind still fails; the overlay outlines a kinematic body and a sensor in their colours.
- **Done-when:** `cargo test -p tungsten-core --locked --lib physics::plugin` → 0 failed, with at least 3 new tests; `cargo test -p tungsten --locked --lib physics_debug` → 0 failed, with at least 1 new test; `rg -c '"kinematic"' crates/tungsten-core/src/component_registry.rs` → 1; `just fmt-check lint` → passes.
- **Moves:** none.

### Step 5: Kinematic contacts under a crowd

- **Files:** `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/tests/physics_containment.rs` (the probe swapped in for one check, then restored from `HEAD`, which the milestone has not changed)
- **Needs:** step 1.
- **Change:** §2.8's two rules. (a) The gather counts kinematic proxies; `physics_step` sets the substep count after the gather, by §2.8's clamp when it counted one and `substeps` otherwise; the rustdoc of `PhysicsConfig::substeps` and `max_step_dt` and the module doc of `physics::step` state the rule. (b) The wake test's kinematic rule reaches a sleeper that a dynamic body touches or closes on while that body's island so far this frame holds a body touching a moving kinematic body; the module doc names it. Tests appended to `tests/physics/step.rs`, each name containing `kinematic`: with a kinematic proxy, a step runs `substeps` at 1/60 s, six at 1/45 s, eight at 1/30 s and in a 0.1 s frame, eight at 0.05 s with `max_step_dt` 0.05 and two at 1/30 s with `substeps` 1, each dt fed through `Time::advance_frame` and counted by a `cfg(test)` substep counter; a world with static, dynamic and sensor bodies and a collider-less kinematic body runs `substeps` at 1/30 s and in a 0.1 s frame; a body pressed by a kinematic body moving at 10 px/s wakes the sleeper 1 px ahead and never sinks more than 1 px into the kinematic body over 4 s, at 1/60 s and at 1/30 s; a row of three sleepers 1 px apart ahead of it wakes in order within the same bound; without a kinematic body, a dynamic body moving at 10 px/s into a sleeper leaves it asleep; a kinematic body at rest, touched by a body moving at 10 px/s into a sleeper, wakes nothing. Removing either rule fails at least one of its tests (recorded as mutations). The probe only measures the step; step 6 lands it.
- **Done-when:** `cargo check --workspace --all-targets --locked` → finishes without an error; `cargo test -p tungsten-core --locked --lib kinematic` → at least 23 passed (17 before the step), 0 failed; `cargo test -p tungsten-core --locked --lib physics` → 0 failed; `just fmt-check lint` → passes; `just physics-release` → 0 failed, 0 ignored, with `physics_step_is_bit_identical_across_runs` and `pinned_step_state_is_unchanged` among the passes. Then the probe: `cp perf-runs/20261013-m49-kinematic/step5/physics_containment.probe.rs crates/tungsten-core/tests/physics_containment.rs`, then `just physics-release -- --nocapture` → 0 failed, 0 ignored, with `crowd_presses_on_a_kinematic_body_from_every_side`, `kinematic_body_walks_into_a_sleeping_crowd` and `kinematic_body_pins_a_body_against_a_wall` among the passes, and each of the eight `crowd walk … 10 px/s` lines printing `deepest` under 1.00 px (Q17); then, whatever the probe printed, `git show HEAD:crates/tungsten-core/tests/physics_containment.rs > crates/tungsten-core/tests/physics_containment.rs` and `cmp <(git show HEAD:crates/tungsten-core/tests/physics_containment.rs) crates/tungsten-core/tests/physics_containment.rs` → exit 0. The press and walk lines' deepest values go in the evidence row; after a failure, the diagnostic copy beside the probe records every case.
- **Moves:** none: no pinned test has a kinematic body.

### Step 6: The crowd probe

- **Files:** `crates/tungsten-core/tests/physics_containment.rs`
- **Needs:** steps 1 and 5.
- **Change:** Three release-only tests (`#[cfg_attr(debug_assertions, ignore = …)]`, as the file's others), named in the file's header:
  1. `crowd_presses_on_a_kinematic_body_from_every_side`: 500 dynamic circles of radius 10 on rings 21 px apart around a kinematic body at the origin, with zero gravity. Before each step every crowd body's velocity is set to 90 px/s toward the body's centre: 300 steps with the body still, then 120 with it moving at 150 px/s along +x. Cases: the body a circle of radius 12 and a box of half-extent 12; spawn orders body first with rings inside out, body last with rings outside in, and a seeded shuffle with the body in the middle; frame dt 1/60, 1/30, 0.05 and 0.1 s through `Time::advance_frame` (the step bound holds the last two to 1/30 s, `D-094`): 24 cases.
  2. `kinematic_body_walks_into_a_sleeping_crowd`: the same 500 circles on a grid 21 px apart, starting 2 px from the body, with no drive, stepped at 1/60 s until all sleep (at most 120 steps); then the body moves into them for 240 steps at 150 px/s and, in a second case, at 10 px/s (below the sleep threshold), with the same shapes and dts: 16 cases.
  3. `kinematic_body_pins_a_body_against_a_wall`: a kinematic box driven at 150 px/s into a dynamic circle resting against a static box and, in a second case, against a tile wall, for 120 steps at each dt: 8 cases.

  Every case asserts that no crowd centre is inside the kinematic body's shape after any step (tests 1 and 2), that the kinematic body's position after every step equals, bit for bit, its position in the same script with nothing else in the world, that every position and velocity stays finite, and that two runs end with the same FNV-1a hash of every dynamic body's position and velocity (the determinism test's `state_hash`, copied). Each case prints one line: the case, the deepest penetration into the kinematic body over the run, the 99th-percentile crowd–crowd penetration at the end, contacts and sleepers at the end and microseconds per step; test 2 adds how many bodies woke, test 3 whether the pinned body's centre crossed the wall's face. The probe kept at `perf-runs/20261013-m49-kinematic/step5/physics_containment.probe.rs` (machine-local) is this file as the first run wrote it to this spec, and step 5 runs it; the rerun copies it in, or writes the tests from this spec where it is missing.
- **Done-when:** `just physics-release -- --nocapture` → 0 failed, 0 ignored, the three new tests among the passes and both pinned-hash tests passing; the 48 printed lines quoted in the evidence log. A deepest penetration over 5 px (half a crowd radius), or a pinned body crossing a wall, is a recorded limit for step 8 (Q12), not a failure.
- **Moves:** none.

### Step 7: The perf sitting

- **Files:** none in the tree. Evidence goes to `perf-runs/<YYYYMMDD>-m49-kinematic/` (machine-local); the baseline tree, and the control tree when one is built, go under `target/m49-*` and are deleted after the step.
- **Needs:** steps 1–6 (§7).
- **Change:** The milestone capture gate ([profiling workflow](../../../perf/profiling-workflow.md#comparison-rule-and-capture-rules), workflow §6). `df -h .` shows at least 10 GB free first. The baseline is `git archive HEAD` (the 0.67 release) extracted to `target/m49-base`; the candidate is this tree after steps 1–6. Prebuild both release binaries with the runner's command and flags (`-C force-frame-pointers=yes`); a second build of each prints only `Finished`, and `sha256sum` tells them apart. Then predict placement ([profiling workflow](../../../perf/profiling-workflow.md#profiling), "Predict code placement"): diff `nm -C --defined-only` of the two binaries and list the functions behind the owned metrics of the rows the change does not run (`ecs`, `churn`, `particles`, `gpu`, `gpu-throughput`) whose address moves modulo 64. If any moved, build the regression policy's control before the sitting: the baseline tree plus a function that no frame calls, sized so that every listed function sits at the candidate's offset modulo 64 (checked with `nm`). Then one background job that writes nothing to the tree: wait for quiet, then `WGPU_BACKEND=vulkan just perf suite --repeat 5` from base, candidate, base and candidate, then the control if one was built; a suite that exits 3 is set aside and run once more. `just perf compare` the two direct pairs (base → candidate), the two A/A pairs and, with a control, base → control and control → candidate.
- **Done-when:** every suite valid; in both direct pairs, `physics` (`physics_step` p50/p95, `update` p95), `physics-sparse` (`physics_step` p50/p95) and `integrated` (`total` p50/p95/p99, jitter) read not `regressed`, and the first-run digests match on all eight rows; every other row's owned metric reads not `regressed` in both direct pairs or, for a row whose functions the placement list names, base → control reads it `regressed` and control → candidate reads it not `regressed` (the [regression policy](../../../perf/profiling-workflow.md#tracked-rows-suites-and-regression-policy)'s justification, both compares recorded); the A/A verdicts quoted beside them; the verdict table, the placement list and the paths in the evidence log.
- **Moves:** none.

### Step 8: Decision, docs, ledger and API snapshot

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/perf/benchmarks.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w06-gameplay-systems.md`, `api/tungsten-core.txt`, `api/tungsten.txt`
- **Needs:** step 7 (§7).
- **Change:**
  - Decision 154 with the [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md) skill (§5), its index row and the amended rows' notes, before any doc cites it; its Why quotes step 6's probe lines beside the first run's (§2.8) and step 7's verdicts.
  - `DESIGN.md`'s Physics section: the four kinds, kinematic contacts with §2.8's substep and wake rules, sensors and their event, the spatial index and its last-step rule, with the decision's ID; the component and resource sentence names `SensorEntered`.
  - [Known issues](../../../known-issues.md#recorded-limits): sensors test overlaps at the step's end (Q3); queries see the last step (Q6); a kinematic body's position write is a teleport that wakes nothing; whatever step 6 recorded as a limit (Q12). The touching-static-box row (A23) adds that casts catch on the join too.
  - [Benchmarks](../../../perf/benchmarks.md#open-proposals): G1 and G2 say they landed in 0.68 and that a benchmark using them is a workload change, with a decision of its own.
  - `LLM_INDEX.md`: the physics row gains the decision's ID, kinematic bodies, sensors and queries, and `core/physics/query.rs`, trimming its own wording so the file stays within 12,288 bytes (A25).
  - The [break ledger](../../1.0/w04-api-freeze.md#break-ledger): `BodyKind` and `CollisionEvent` (`#[non_exhaustive]`, two variants, `new`; the workspace's literals and matches changed); `PhysicsDebugOverlay`'s two fields; an addition row listing the new surface for the freeze.
  - `w06`'s placement: W6a landed in 0.68 (M49).
  - `just api`, then review `git diff --unified=1 -- api/`.
- **Done-when:** `rg -c '^## D-15[4] ' DECISIONS.md` → 1; ``rg -c '^\| `D-15[4]` ' docs/DECISION_INDEX.md`` → 1; ``rg -c '^\| `D-0(33|64|65)` .*D-15[4]' docs/DECISION_INDEX.md`` → 3; `rg -c '^\*\*Amended by D-15[4]:' DECISIONS.md` → at least 3; `rg -c -i 'kinematic' DESIGN.md` → at least 1; `rg -c -i 'sensor' docs/known-issues.md` → at least 1; `sed -n '/^## Open proposals/,/^\*\*Below/p' docs/perf/benchmarks.md | rg -c '0\.68'` → 2; `rg -c 'core/physics/query.rs' docs/LLM_INDEX.md` → 1; `sed -n '/^## Break ledger/,/^## Context digest/p' docs/plans/1.0/w04-api-freeze.md | rg -c 'M49'` → at least 3; `rg -c 'M49' docs/plans/1.0/w06-gameplay-systems.md` → 1. After `just api`: `rg -c '^#\[non_exhaustive\] pub enum tungsten_core::physics::components::BodyKind$' api/tungsten-core.txt` → 1; `rg -c 'BodyKind::(Kinematic|Sensor)$' api/tungsten-core.txt` → 6; `rg -c '^#\[non_exhaustive\] pub struct .*::CollisionEvent$' api/tungsten-core.txt` → 3; `rg -c '(CollisionEvent|SensorEntered)::new\(' api/tungsten-core.txt` → 6; `rg -c '(SpatialFilter|SpatialHit)::clone\(' api/tungsten-core.txt` → 4; `rg -c '^pub fn tungsten_core::physics::(cast_ray|cast_shape|overlap|nearest)\(' api/tungsten-core.txt` → 4; `rg -c 'PhysicsDebugOverlay::color_(kinematic|sensor):' api/tungsten.txt` → 4; `just api-check` → passes; `just ctx` → `Agent context checks: OK`; `just repo-check` → passes.
- **Moves:** none.

### Step 9: Release 0.68

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.68.0` once, every release check once after it with `just visual` (the must-not-move pixel invariants; no step runs it), then the preflight with `--message`. The `CHANGELOG.md` line sits under `[Unreleased]` beside the other session's `Changed` entry already there.
- **Done-when:** [releases](../../../releases.md) step 1's command chain, with `just visual` → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.68.0 --repo JosephGibson/Tungsten --message 'Update 0.68: M49 kinematic bodies, sensors and spatial queries'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for the next register row, W13a and W13b

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Do kinematic bodies get contacts or events against static bodies, tiles or other kinematic bodies? | No (Box2D's rule): both sides are immovable, so the narrow phase drops the pair as it drops static pairs. Walls reach a kinematic body through casts and queries; a game that wants a touch report adds a sensor |
| Q2 | Is a sensor a body kind or a component on any body? | A kind: the card names new kinds for `rigid_body`, W6b's layer fields then sit on one body component, and a sensor moves as a kinematic body. There is no dynamic sensor |
| Q3 | Do sensors detect a body they pass within one step? | No: overlaps at the step's final positions, as Box2D v3 does; `cast_shape` covers a fast sensor |
| Q4 | Sensor exit events? | Not in W6a, which has no user for them; a `SensorExited` would be an addition when W13d's zones need one |
| Q5 | Do queries take `&World` or `&mut World`? | `&World`, over an index in a private `RefCell` resource (the extract's `D-086` precedent), so a system can query inside its own query loop and `PhysicsBuffers` keeps `Sync` |
| Q6 | Do queries see the live World or the last step? | The last step: an index of its final positions, built at most once per step; despawned entities skipped, bodies spawned since unseen |
| Q7 | A shape cast that starts overlapping a body | It hits at distance 0 only when moving deeper and otherwise passes; a ray ignores a shape containing its origin |
| Q8 | The default filter | Bodies and tiles, no sensors, nothing excluded |
| Q9 | How does `overlap` report tiles? | As `None`, one per tile, as `CollisionEvent.b` does; `with_tiles(false)` leaves them out |
| Q10 | The overlay's colours: two public fields (a break) or derived from the shape colours? | Two fields and a ledger row; nothing in the workspace builds the overlay without `..Default::default()` (A19) |
| Q11 | The crowd's parameters | 500 bodies of radius 10 driven at 90 px/s toward a body of radius or half-extent 12 that walks at 150 px/s, and into the sleeping crowd also at 10 px/s, below the sleep threshold: the acceptance game's hundreds of enemies |
| Q12 | The crowd sinks deep into the kinematic body without crossing its centre, or a pinned body crosses a wall | A recorded limit with its numbers in known issues and in decision 154's consequences; no tuning in W6a beyond step 5's two rules (§2.8; `contact_hertz` stays the backlog's lever), and W13c's mover stops before it pins |
| Q13 | A row the change runs (`physics`, `physics-sparse`, `integrated`) reads `regressed` | Its pair is captured once more; a second `regressed` ends the run before step 8 (§7), with the steps' work left in the tree. A fix, or a decision accepting the cost, precedes the records and the cut in a release session |
| Q14 | `restitution` on a sensor in a scene | Accepted, with no effect |
| Q15 | Which lever fixes the press at 1/30 s and above, and does a still kinematic body count? | More substeps in a world with a kinematic proxy, up to twice `substeps` (§2.8): the only lever with evidence. A still one counts, so the probe at 1/30 s and above runs the diagnostic's arithmetic and a body that starts or stops moving changes no other body's step. The arrival-pass chain move stays a follow-up |
| Q16 | How far down a pressed chain does the kinematic wake reach? | The pressing body's island so far this frame, touching or closing as the kinematic rule does: a rule that stopped at the bodies touching the kinematic body, or that needed closing motion, can leave a sleeper holding the chain. A sleeping pile that island touches wakes while the kinematic body moves (§2.8) |
| Q17 | The slow walk's bar | Under 1 px in each 10 px/s walk case: the first run sank 7.70 and 9.48 px, the walk with sleeping off 0.26 to 0.27 px, and Q12's limit is 5 px |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

154: W6a landed (M49): kinematic bodies, sensors with enter events, and spatial queries and casts over an index of the last step. Amends `D-033` (the body model; tile layers reach every kind), `D-064` (a step longer than 1/60 s in a world with a kinematic proxy runs up to twice `substeps`, so no substep is longer than a 1/60 s step's up to the default bound) and `D-065` (wake path 1 counts a kinematic body moving at any speed, which also keeps awake the island of each body it touches, whatever `time_to_sleep` is, and that island wakes the sleepers it touches or closes on at any speed; kinematic bodies and sensors never sleep and join no island). Step 8 checks `D-066` (sensor rows pull no proxy in the positional writeback), `D-081` (the budgets of awake proxies), `D-092` (immovable targets) and `D-094` (the bound's substep arithmetic) against what steps 1 and 5 landed and gives a marker to any clause it changed. The next free ID when written (`D-153` is the last); checked again when step 8 runs, and the next free one taken if another session has used it.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168`, the pinned containment hash `0xaee2_72e0_1ffc_4e4c`, the eight row digests (`physics` `c10885dddd6115a4` and `physics-sparse` `9df5ade5305646dc` among them), `gpu-visual.png`, the post and transition regressions, the template's schedule snapshot (A24), `Proxy`'s 80 bytes.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 3 needs step 2; step 4's kinematic half needs step 1 and its sensor half step 3; step 5 needs step 1; step 6 needs steps 1 and 5; step 7 needs steps 1–6; step 8 needs step 7.
- Steps 1–6 deliver W6a's card, `D-132`'s Must items among them, so a skipped step there ends the run before step 7. Its work and evidence stay in the tree, and the sitting, the records and the cut wait for a release session after the fix, so no release records W6a without them.
- A step moves a pinned hash or a row digest: its done-when has failed (above).
- The crowd probe fails an assertion (a crowd centre inside the kinematic body, the body off its path, a non-finite value, two runs apart), in step 5's run or step 6's: that step is skipped (above), with the failing lines in the evidence log, since W13c must not build on wrong kinematic contacts. Step 5 restores `physics_containment.rs` from `HEAD` whatever its probe printed.
- A `physics`, `physics-sparse` or `integrated` owned metric reads `regressed` in a direct pair: that row's pair is captured once more (`just perf suite --only <row> --repeat 5` from base and candidate), and a second `regressed` ends the run before step 8 (Q13). Another row's owned metric that reads `regressed` in a direct pair without the control's justification (step 7), including when no single pad places every listed function, ends it there too.
- A suite exits 3 twice: the run ends before step 8.
- A file outside §files to touch needs a change: that step stops, unless the compiler names the file as a caller of an item the step changes; then it joins the step and its evidence row names it.
- Another session's edits share the tree (the skill, skeleton and `CHANGELOG.md` edits were there when this plan was written): never revert them; the report lists them.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–7, release | `just physics-release` and targeted tests in 1–6; the full suite in 7, judged on `physics`, `physics-sparse` and `integrated`; the release step runs the release checks once after the cut, with `just visual` |
| 3 Invariants | 1–7, release | Hashes by `just physics-release` in 1–6, digests in 7, pixels by `just visual` at the release |
| 4 Decisions | 8 | 154, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1–5, 8 | Rustdoc with each item in 1–4, and step 5's rule in `PhysicsConfig`'s; the ledger rows and the snapshot in 8; `just api-check` at the release |
| 6 Template and guide | n/a | The template's game moves no body and its schedule is unchanged (A24); W9b has not started, so the guide waits; W13c moves games onto kinematic movers |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 8 | `core/physics/query.rs` joins the physics row within the byte budget |
| 8 Design | 8 | `DESIGN.md`'s Physics section |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 8, release | Known issues, the benchmarks' G1 and G2, `w06` and the ledger in 8; nothing cut, so no backlog row; no gap row names W6a; the register row and the owned fields at the release |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, 2026-10-13, at `026f9e4` with this plan untracked; the artifact left out this section and the evidence log (`--from` with step 7's `sed`). Footer: `gpt-6-astra`, effort xhigh, context 872,000, output cap 4,000, 283.4 s, tokens in 711,295 (cached 630,528), out 8,050 (reasoning 5,044), 18 commands, output 267 KB (largest 37.4 KB), 5-hour window 0% → 18%, week 16% → 19%. A first attempt at 17:1x ADT had stopped at `critique: codex exec failed (exit 1, usage limit reached)` after 24 commands, with no finding (5-hour window 84% → 100%); it was not retried until the window reset, and the plan was not reviewed in the critic's place.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Slow kinematic bodies can pass through sleeping bodies: the wake test needs a relative speed above the sleep threshold, and the immovable pair is then dropped | `step/mod.rs:514-540`; `physics/mod.rs:89` (20 px/s) | Accepted | §2.1 and §2.3: a kinematic body moving at any speed wakes a sleeper it touches or closes on, and a body touching one stays awake; step 1 gains the threshold and platform tests (at least 12); step 5's sleeping crowd also walks at 10 px/s (48 lines); §5's `D-065` wording; Q11 |
| 2 | [HIGH] Failed Must features can be skipped while W6a is recorded as complete: §7 restores and skips a failed step, and step 7 lowered its counts and still released | This plan's §7 and step 7; `DECISIONS.md:1623` (`D-132`'s Must list) | Accepted | §7: a skipped step among 1–5 ends the run before step 6, so no release records W6a without its card; steps 6 and 7 name what they need; step 7's done-when loses its skipped-step clause |
| 3 | [MEDIUM] Sensor history loses changes in which entity is the sensor (`pair_key` is order-independent), and a step without sensors leaves stale history | `step/pairs.rs:71-76`; `physics/components.rs:93` (`kind` is public) | Accepted | §2.3 and §2.5: ordered keys, sensor first; a step that runs with no sensor empties the history; step 3 gains both cases (at least 12) |
| 4 | [MEDIUM] The aligned comparison does not establish the claimed placement explanation | `docs/perf/profiling-workflow.md:212` (the policy's padded baseline), `:246` (predicting placement with `nm`) | Accepted | Step 6: the `nm` prediction and, when a listed function moves, the policy's control replace the aligned builds; its done-when, §7's perf condition and Q13 follow |

Round 1 changed done-when checks (steps 1, 3, 5, 6 and 7), so round 2 ran on the revised plan, at xhigh, without this section and the evidence log, with a focus note naming the revised sections (no finding was rejected). Footer: `gpt-6-astra`, effort xhigh, context 872,000, output cap 4,000, 196.7 s, tokens in 654,221 (cached 577,664), out 5,677 (reasoning 3,070), 17 commands, output 237.5 KB (largest 37.8 KB), 5-hour window 37% → 54%, week 22% → 24%.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Resetting the timer during substeps does not always prevent sleep: the frame-end pass adds the step's dt and sleeps the island once that reaches a public `time_to_sleep` shorter than the step | `step/sleep.rs:286`, `:328`; `physics/mod.rs:73` | Accepted | §2.3: touching a moving kinematic body marks the island kept awake, and the frame-end pass sleeps no marked island; step 1's platform test also runs at `time_to_sleep` 0.01 s and 0 (at least 13); §5's wording |
| 2.2 | [MEDIUM] Tile warm-start keys cannot reliably identify persistent sensor pairs: `tile_key` hashes the centre, which a moved `origin` changes and two layers share | `step/gather.rs:17-24`; `assets/tilemap.rs:335` | Accepted | §2.3 and §2.5: in a frame with sensors the gather records each tile's map entity, layer and cell, and sensor history keys a tile by them; step 3 gains a moving-map and a replaced-tile test (at least 14) |

There is no third round; no finding is left open.

Both rounds name steps as they were numbered before step 5 was added: their steps 5–8 are now 6–9. The amendment that added step 5 (2026-10-13, after the probe's first run) had no round, since the skill allows two; its choices are Q15–Q17 at their defaults.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-13 | Done. `step/sleep.rs` joined the step with the owner's approval: §2.3's kept-awake rule is a change to its frame-end pass, and the step's file list had left it out. A marked member restarts its timer and holds its island's minimum at −∞. `Proxy` takes `kinematic` and `kept_awake` and an `#[allow(clippy::struct_excessive_bools)]` | `cargo check --workspace --all-targets --locked` exit 0; `--lib kinematic` 15 passed, 0 failed; `--lib physics` 108 passed, 0 failed; `physics_tunneling` 7 passed; `example-01-platformer` 106 passed; `--lib fixed_step` 13 passed; `rg -c 'CollisionEvent \{' examples crates/tungsten/src` exit 1 (the `fixed_step.rs` helper whose signature matched was inlined at its two calls); `just fmt-check lint` exit 0; `just physics-release` 12 passed, 0 failed, 0 ignored, `pinned_step_state_is_unchanged` and `physics_step_is_bit_identical_across_runs` among them. Mutations: dropping the kinematic wake rule fails 3 tests, the kept-awake mark 3, the event swap 4, the island hold (timer reset only) 1 (`time_to_sleep` 0) | `perf-runs/20261013-m49-kinematic/step1/` (machine-local) |
| 2 | 2026-10-13 | Done. Rays treat shapes as closed (a ray down a tile seam cannot slip between two tiles; one starting on an edge ignores that shape, as Box2D's does) and shape casts as open (a touching shape slides). A query box spanning more cells than four per entry tests every entry instead of the grid, so a long ray over a sparse hashed grid cannot walk billions of cells. A zero-extent `cast_shape` casts as a ray | `--lib physics::query` 17 passed, 0 failed; `--doc physics` 1 passed (the module example); `cargo check --workspace --all-targets --locked` exit 0; `just fmt-check lint` exit 0 (after `f32::midpoint` for clippy's `manual_midpoint`); `just physics-release` 12 passed, 0 ignored, both pinned-hash tests among them. Mutation: ignoring tile face masks fails 3 tests | `perf-runs/20261013-m49-kinematic/step2/` |
| 3 | 2026-10-13 | Done. Sensors join a list beside the proxies; the writeback moves each by `velocity · dt`; with sensors, the step builds the index and runs the pass, so its queries reuse that build (one per step, tested) | `--lib sensor` 15 passed, 0 failed; `--lib physics` 140 passed, 0 failed; `cargo check --workspace --all-targets --locked` exit 0; `just fmt-check lint` exit 0; `just physics-release` 12 passed, 0 ignored, both pinned-hash tests among them. Mutations: an order-independent pair key fails the swap test; keeping the pairs through a step without sensors fails the round-trip test; keying tiles by their warm-start key fails the moving-map and other-layer tests | `perf-runs/20261013-m49-kinematic/step3/` |
| 4 | 2026-10-13 | Done. `mass` on any kind but `dynamic` fails before the kind's branch; the old `"kinematic"` unknown-variant case became `"ghost"` and two `mass` cases | `--lib physics::plugin` 13 passed, 0 failed (3 new tests); `-p tungsten --lib physics_debug` 6 passed, 0 failed (1 new); `rg -c '"kinematic"' crates/tungsten-core/src/component_registry.rs` 1; `just fmt-check lint` exit 0 (after naming `DebugShape::Line` for clippy's `match_wildcard_for_single_variants`) | `perf-runs/20261013-m49-kinematic/step4/` |
| 5 | 2026-10-13 | Done. The gather counts kinematic proxies in its kinematic arm and `physics_step` sets the count after it (`substep_count`); `kinematic_touch` lists each body it marks in `kept`, and the island rule looks for a listed body in `a`'s island (`island_holds_any`, `sleep.rs`) only while the list is not empty. The probe ran swapped in and the file was restored from `HEAD` | `cargo check --workspace --all-targets --locked` exit 0; `--lib kinematic` 23 passed (17 before), 0 failed; `--lib physics` 149 passed, 0 failed; `just fmt-check lint` exit 0 (after `rustfmt` on the tests file); `just physics-release` 12 passed, 0 failed, 0 ignored, both pinned-hash tests among them. Probe, `just physics-release -- --nocapture`: 15 passed, 0 failed, 0 ignored, the three crowd tests among them. Walk at 10 px/s: `deepest 0.26 px` in all eight lines (first run 7.70 and 9.48); at 150 px/s 0.37 px in all eight (0.37 and 1.03). Press, BodyFirst / BodyLast / Shuffled: at 1/60 s Circle 3.03 / 5.81 / 3.19 px, Box 2.69 / 5.76 / 5.01 px (four substeps, as the first run); at 1/30 s and above Circle 1.62 / 2.74 / 1.65 px, Box 1.92 / 2.70 / 1.84 px, no centre inside (first run: four of six crossed); 301–419 µs a step at 1/60 s, 529–553 µs above. Pin: static box 21.53 px at 1/60 s and 19.97 px above, crossed at every dt; tiles 21.51 px, crossed at none. Restore `cmp` exit 0. Mutations: without rule (a) the substep-count test fails; without rule (b) both press tests fail (sink 1.04 px at step 22; measured deepest 7.70 px against 0.25–0.27 px with the rule, the row woken at step 126 against 7, 15 and 22); rule (b) without the `kept` condition fails both no-wake tests; a still kinematic body listing its touches fails 3 tests | `perf-runs/20261013-m49-kinematic/step5/rules/` (machine-local); the first run's logs, the probe and the diagnostic copy in `step5/` |
| 6 | 2026-10-13 | Done. The probe kept in `step5/` landed unchanged (`cmp` exit 0 against it); `cargo fmt --all -- --check` and `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` exit 0 | `just physics-release -- --nocapture` exit 0: 15 passed, 0 failed, 0 ignored, the three crowd tests, `pinned_step_state_is_unchanged` and `physics_step_is_bit_identical_across_runs` among the passes; the 48 lines below the table, identical to step 5's probe run but for µs per step. Recorded limits for step 8 (Q12): at 1/60 s the press reaches 5.81 px (Circle BodyLast), 5.76 (Box BodyLast) and 5.01 (Box Shuffled), over half a crowd radius; the pinned body's centre crossed the static box's face at every dt, deepest 19.97 to 21.53 px, and the tile wall's at none | `perf-runs/20261013-m49-kinematic/step6/` |
| 7 | 2026-10-13 | Done, with no control: 28 of the 29 listed functions move modulo 64, and in address order their shifts run 16 (the bench crate and `extract_sprites_default::{closure#0}`, +1552 bytes), 32 (`tungsten`'s particle systems and `extract_sprites_default`, +1568), 0 (`World::flush_reusing`, unmoved) and 16 (the renderer, +19152), so no single pad places every one (§7). The sitting ran the four suites | `df` 194 GB free; base `git archive HEAD` (026f9e4) in `target/m49-base`; second builds `Finished` in 0.19 and 0.11 s; sha256 base `6b80a9218ae12b4b`, candidate `b8fdc2cabdeefcd1`. Sitting 00:21–00:36 UTC, encoder absent, every suite valid at its first attempt, the tree's diff hash `2db8d86e2432` before and after. Direct pairs (`direct` / `direct-b`), all not `regressed`: `physics` `physics_step` p50 6.146 → 6.166 / 6.160 → 6.150, p95 6.300 → 6.310 / 6.304 → 6.282, `update` p95 6.354 → 6.366 / 6.366 → 6.340 ms, `unchanged`; `physics-sparse` `physics_step` p50 3.704 → 3.670 in both, p95 3.762 → 3.736 / 3.764 → 3.728 ms, `unchanged`; `integrated` `total` p50 7.954 → 7.986 / 7.914 → 8.002 and p95 8.352 → 8.420 / 8.414 → 8.546 ms `unchanged`, p99 8.802 → 8.936 `unchanged` / 8.968 → 9.300 `noisy`, jitter 0.848 → 0.950 `unchanged` / 1.054 → 1.298 `noisy`. Every other row: no owned `regressed` in either pair; `noisy` above τ: `ecs` `follow` p50 +0.112 (τ 0.051), `gpu` `render_encode` p95 +0.104 (τ 0.092) and `gpu-throughput` `render_encode` p50 +0.154 (τ 0.050) in `direct`, `ecs` `stats_decay` p50 +0.022 (τ 0.020) in `direct-b`. First-run digests match in 8 of 8 rows of all four compares; `physics` `c10885dddd6115a4` and `physics-sparse` `9df5ade5305646dc` in every run. A/A: `base-aa` 0 `regressed`, 0 `improved`; `cand-aa` `particles` `particle_count_refresh` p50 `improved` 0.648 → 0.616 ms (per run 0.66, 0.65, 0.67, 0.64, 0.62 against 0.60, 0.62, 0.60, 0.61, 0.65), one build against itself, and nothing else. Verdict table below; the scratch tree and its build deleted | `perf-runs/20261013-m49-kinematic/step7/` (`README.md`, `summary.md`, `placement.md`, `captures/`, `scripts/`) |
| 8 | 2026-10-13 | Done. `D-154` with the tungsten-decision skill (`D-153` was still the last) and seven markers: `D-033`, `D-064` and `D-065` as §5 planned, and `D-066` (sensor rows in the writeback zip), `D-081` (kinematic budgets), `D-092` (kinematic bodies immovable in the arrival pass) and `D-094` (a kinematic world's 1/30 s step in eight substeps), whose clauses steps 1 and 5 changed; each index row carries its note. The LLM index's physics row now reads "Contacts, sleep, kinematic bodies, sensors, queries" to fit (12,286 B). The ledger gains three break rows and an addition row. The API diff holds only the planned surface: six lines changed for the two types turned `#[non_exhaustive]`, the rest added | `rg` counts: `D-154` heading 1, its index row 1, the `D-033`/`D-064`/`D-065` rows 3, markers 7, `DESIGN.md` kinematic 3, known-issues sensor 2, the proposals' `0.68` 2, the LLM index's `core/physics/query.rs` 1, the ledger's `M49` 4, `w06`'s `M49` 1. After `just api`: `BodyKind` `#[non_exhaustive]` 1, its variants 6, `CollisionEvent` `#[non_exhaustive]` 3, `::new(` 6, `::clone(` 4, the query functions 4, the overlay colours 4. `just api-check` passes ("Public API snapshots current: 3 crates"); `just ctx` → `Agent context checks: OK`; `just repo-check` exit 0 (0 errors, 51 notes) | `perf-runs/20261013-m49-kinematic/step8-checks.log`, `step8-api.log` |
| 9 | 2026-10-13 | Workflow §5 closed before the cut. (1) Rows 1–8 quote every done-when. (2) and (3) The hashes held in steps 5 and 6 and the digests in step 7; the release checks, `just visual` among them, run once after the cut. (4) `D-154` and its index rows landed before the docs that cite it. (5) The ledger's three break rows and its addition row; `cargo doc -p tungsten-core -p tungsten` with `-W missing-docs` flags no new item, its 975 warnings all older. (6) n/a: the template uses none of the changed types (`rg` finds no `CollisionEvent`, `BodyKind` or `PhysicsDebugOverlay` in `templates/`), and `just template-check` builds it. (7) `core/physics/query.rs` routed, `just ctx` OK. (8) `DESIGN.md`'s Physics section and status line. (9) Known issues, the benchmarks' G1 and G2, `w06` and the register row; no backlog row, since nothing was cut; the gap log is empty. (10) One `CHANGELOG.md` entry under `[Unreleased]`, `status: done`, the plan archived | The release checks and the preflight run after the cut, so their output is in the session's report, not here | — |

Step 6's 48 lines, as printed:

```text
crowd pin StaticBox dt 0.0167: deepest 21.53 px, crowd p99 0.00 px, contacts 0, sleepers 1, 1 us/step, crossed the wall true
crowd pin StaticBox dt 0.0333: deepest 19.97 px, crowd p99 0.00 px, contacts 0, sleepers 1, 1 us/step, crossed the wall true
crowd pin StaticBox dt 0.0500: deepest 19.97 px, crowd p99 0.00 px, contacts 0, sleepers 1, 1 us/step, crossed the wall true
crowd pin StaticBox dt 0.1000: deepest 19.97 px, crowd p99 0.00 px, contacts 0, sleepers 1, 1 us/step, crossed the wall true
crowd pin Tiles dt 0.0167: deepest 21.51 px, crowd p99 0.00 px, contacts 0, sleepers 1, 2 us/step, crossed the wall false
crowd pin Tiles dt 0.0333: deepest 21.51 px, crowd p99 0.00 px, contacts 0, sleepers 1, 3 us/step, crossed the wall false
crowd pin Tiles dt 0.0500: deepest 21.51 px, crowd p99 0.00 px, contacts 0, sleepers 1, 3 us/step, crossed the wall false
crowd pin Tiles dt 0.1000: deepest 21.51 px, crowd p99 0.00 px, contacts 0, sleepers 1, 3 us/step, crossed the wall false
crowd walk Circle 150 px/s dt 0.0167: deepest 0.37 px, crowd p99 0.25 px, contacts 2, sleepers 478, 46 us/step, woke 174
crowd walk Circle 150 px/s dt 0.0333: deepest 0.37 px, crowd p99 0.26 px, contacts 1, sleepers 476, 55 us/step, woke 185
crowd walk Circle 150 px/s dt 0.0500: deepest 0.37 px, crowd p99 0.26 px, contacts 1, sleepers 476, 57 us/step, woke 185
crowd walk Circle 150 px/s dt 0.1000: deepest 0.37 px, crowd p99 0.26 px, contacts 1, sleepers 476, 55 us/step, woke 185
crowd walk Circle 10 px/s dt 0.0167: deepest 0.26 px, crowd p99 0.25 px, contacts 32, sleepers 466, 33 us/step, woke 56
crowd walk Circle 10 px/s dt 0.0333: deepest 0.26 px, crowd p99 0.25 px, contacts 30, sleepers 468, 47 us/step, woke 66
crowd walk Circle 10 px/s dt 0.0500: deepest 0.26 px, crowd p99 0.25 px, contacts 30, sleepers 468, 47 us/step, woke 66
crowd walk Circle 10 px/s dt 0.1000: deepest 0.26 px, crowd p99 0.25 px, contacts 30, sleepers 468, 47 us/step, woke 66
crowd press Circle BodyFirst dt 0.0167: deepest 3.03 px, crowd p99 2.76 px, contacts 1389, sleepers 0, 307 us/step
crowd walk Box 150 px/s dt 0.0167: deepest 0.37 px, crowd p99 0.25 px, contacts 2, sleepers 475, 49 us/step, woke 176
crowd walk Box 150 px/s dt 0.0333: deepest 0.37 px, crowd p99 0.25 px, contacts 1, sleepers 476, 55 us/step, woke 176
crowd walk Box 150 px/s dt 0.0500: deepest 0.37 px, crowd p99 0.25 px, contacts 1, sleepers 476, 55 us/step, woke 176
crowd walk Box 150 px/s dt 0.1000: deepest 0.37 px, crowd p99 0.25 px, contacts 1, sleepers 476, 55 us/step, woke 176
crowd walk Box 10 px/s dt 0.0167: deepest 0.26 px, crowd p99 0.25 px, contacts 24, sleepers 472, 33 us/step, woke 56
crowd walk Box 10 px/s dt 0.0333: deepest 0.26 px, crowd p99 0.25 px, contacts 26, sleepers 470, 48 us/step, woke 68
crowd walk Box 10 px/s dt 0.0500: deepest 0.26 px, crowd p99 0.25 px, contacts 26, sleepers 470, 48 us/step, woke 68
crowd walk Box 10 px/s dt 0.1000: deepest 0.26 px, crowd p99 0.25 px, contacts 26, sleepers 470, 48 us/step, woke 68
crowd press Circle BodyLast dt 0.0167: deepest 5.81 px, crowd p99 2.79 px, contacts 1391, sleepers 0, 303 us/step
crowd press Circle Shuffled dt 0.0167: deepest 3.19 px, crowd p99 2.71 px, contacts 1390, sleepers 0, 312 us/step
crowd press Circle BodyFirst dt 0.0333: deepest 1.62 px, crowd p99 0.90 px, contacts 1345, sleepers 0, 546 us/step
crowd press Circle BodyLast dt 0.0333: deepest 2.74 px, crowd p99 0.97 px, contacts 1327, sleepers 0, 556 us/step
crowd press Circle Shuffled dt 0.0333: deepest 1.65 px, crowd p99 0.90 px, contacts 1338, sleepers 0, 538 us/step
crowd press Circle BodyFirst dt 0.0500: deepest 1.62 px, crowd p99 0.90 px, contacts 1345, sleepers 0, 543 us/step
crowd press Circle BodyLast dt 0.0500: deepest 2.74 px, crowd p99 0.97 px, contacts 1327, sleepers 0, 554 us/step
crowd press Circle Shuffled dt 0.0500: deepest 1.65 px, crowd p99 0.90 px, contacts 1338, sleepers 0, 540 us/step
crowd press Circle BodyFirst dt 0.1000: deepest 1.62 px, crowd p99 0.90 px, contacts 1345, sleepers 0, 543 us/step
crowd press Circle BodyLast dt 0.1000: deepest 2.74 px, crowd p99 0.97 px, contacts 1327, sleepers 0, 554 us/step
crowd press Circle Shuffled dt 0.1000: deepest 1.65 px, crowd p99 0.90 px, contacts 1338, sleepers 0, 540 us/step
crowd press Box BodyFirst dt 0.0167: deepest 2.69 px, crowd p99 2.89 px, contacts 1378, sleepers 0, 301 us/step
crowd press Box BodyLast dt 0.0167: deepest 5.76 px, crowd p99 2.79 px, contacts 1387, sleepers 0, 307 us/step
crowd press Box Shuffled dt 0.0167: deepest 5.01 px, crowd p99 2.60 px, contacts 1389, sleepers 0, 307 us/step
crowd press Box BodyFirst dt 0.0333: deepest 1.92 px, crowd p99 0.90 px, contacts 1342, sleepers 0, 541 us/step
crowd press Box BodyLast dt 0.0333: deepest 2.70 px, crowd p99 0.83 px, contacts 1339, sleepers 0, 543 us/step
crowd press Box Shuffled dt 0.0333: deepest 1.84 px, crowd p99 0.81 px, contacts 1363, sleepers 0, 530 us/step
crowd press Box BodyFirst dt 0.0500: deepest 1.92 px, crowd p99 0.90 px, contacts 1342, sleepers 0, 542 us/step
crowd press Box BodyLast dt 0.0500: deepest 2.70 px, crowd p99 0.83 px, contacts 1339, sleepers 0, 544 us/step
crowd press Box Shuffled dt 0.0500: deepest 1.84 px, crowd p99 0.81 px, contacts 1363, sleepers 0, 530 us/step
crowd press Box BodyFirst dt 0.1000: deepest 1.92 px, crowd p99 0.90 px, contacts 1342, sleepers 0, 542 us/step
crowd press Box BodyLast dt 0.1000: deepest 2.70 px, crowd p99 0.83 px, contacts 1339, sleepers 0, 544 us/step
crowd press Box Shuffled dt 0.1000: deepest 1.84 px, crowd p99 0.81 px, contacts 1363, sleepers 0, 530 us/step
```

Step 7's owned verdicts per row and pair:

| Row | `direct` | `direct-b` | `base-aa` | `cand-aa` |
| --- | --- | --- | --- | --- |
| `physics` | 3 unchanged | 3 unchanged | 3 unchanged | 3 unchanged |
| `physics-sparse` | 2 unchanged | 2 unchanged | 2 unchanged | 2 unchanged |
| `ecs` | 7 unchanged, 9 noisy | 13 unchanged, 3 noisy | 11 unchanged, 5 noisy | 9 unchanged, 7 noisy |
| `churn` | 5 unchanged, 1 noisy | 5 unchanged, 1 noisy | 6 unchanged | 3 unchanged, 3 noisy |
| `gpu` | 14 unchanged, 1 noisy | 13 unchanged, 2 noisy | 13 unchanged, 2 noisy | 14 unchanged, 1 noisy |
| `gpu-throughput` | 2 unchanged, 2 noisy | 2 unchanged, 2 noisy | 2 unchanged, 2 noisy | 2 unchanged, 2 noisy |
| `particles` | 5 unchanged, 3 noisy | 5 unchanged, 3 noisy | 5 unchanged, 3 noisy | 1 improved, 6 unchanged, 1 noisy |
| `integrated` | 4 unchanged | 2 unchanged, 2 noisy | 4 unchanged | 2 unchanged, 2 noisy |

## Follow-ups

- The arrival-pass lever step 5 did not take (§2.8): moving a kinematic mover's pressed chain with it in `D-092`'s backward sweep, which skips movers it cannot slow, if the press depths over 5 px at 1/60 s (Q12) need more than a recorded limit.
- Sensor exit events, when W13d's trigger zones need them (Q4).
- Swept sensors, if the game's bolts outrun a step (Q3).
- A `physics` variant with kinematic paddles and real bin sensors (G1, G2): a workload change with its decision.
- A criterion bench for casts and the sensor pass once the game's query load is known: W13c or the Phase 6 QA pass.
- The touching-static-box seam for contacts and casts ([known issues](../../../known-issues.md), A23).
