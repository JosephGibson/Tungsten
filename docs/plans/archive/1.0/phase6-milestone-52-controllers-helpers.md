# Phase 6 milestone 52: controllers and gameplay helpers (W13c and W13d) — done

- **status:** done
- **goal:** The kit's character controllers and gameplay helpers ([criteria](../../1.0/criteria.md) §8.4): a platformer controller with a ground check, coyote time, a jump buffer and variable jump height, and a top-down mover, both on kinematic bodies in `fixed_update` before `physics_step`; health and damage with invulnerability time and a hit flash on `damage_flash`; trigger zones on sensors; a path follower for kinematic platforms; a per-entity state machine. Each item has rustdoc with an example and a harness test, and example 01's player moves onto the platformer controller, so its ground detection goes.
- **non-goals:** Slopes, one-way platforms and ladders (Q1, the backlog); a controller that pushes dynamic bodies, or a body-kind query filter (Q2; W6b's masks); kit scene components (Q10); the template's player (Q11); example 01's health, hit flash, hazards and moving platforms (Q12, W12b); sensor exit events (`D-154`'s open item); any change to `tungsten-core`, `tungsten-render` or the umbrella's runtime; perf captures (no benchmark row builds kit or example 01 code).
- **files to touch:** Step 1: `crates/tungsten-kit/src/lib.rs`; new `crates/tungsten-kit/src/{platformer,top_down,slide,health,trigger,path,state_machine}.rs` and `crates/tungsten-kit/src/tests/{platformer,top_down,health,trigger,path,state_machine}.rs`; `api/tungsten-kit.txt`. Step 2: `crates/tungsten/src/tests/kit.rs`; step 1's kit modules and their unit tests, only to fix a fault a harness test finds, the public surface unchanged. Step 3: `examples/01_platformer/src/{setup,systems,state,gameplay,fireball}.rs`; `examples/01_platformer/src/{death,extract,brick}.rs` only where a read of the player's body needs it; `examples/01_platformer/src/tests/{main,player,level,regime,presentation,fixed_step,hazards,death,ball_pit,ice,brick,camera,spells}.rs`, each only where it reads the old dynamic player; `examples/01_platformer/tools/README.md`. Step 4: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/{w13-kit,w04-api-freeze,backlog-1.x,criteria}.md`, `docs/plans/1.0/roadmap.json`. Step 5: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields, this plan (moved to `docs/plans/archive/1.0/`).
- **ordered steps:** (1) The kit's controllers and helpers with unit tests, doctests and `just api`, then step 1's API critique. (2) The harness tests and `just physics-release`. (3) Example 01's player onto the platformer controller. (4) The decision and the docs. (5) Release 0.72.
- **done-when:** Every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, every gate by change having run in a step (§8); the preflight prints the command block and the post-merge block.

Candidate W13c and W13d ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M52, release 0.72. Written 2026-10-14 at `2f46d1d` and revised the same day after its critique's first round (§9). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. W13 has not graduated, so criteria §8.4 is the scoping source and [w13-kit.md](../../1.0/w13-kit.md) holds placement and follow-ups. One candidate of two groups, not a `D-108` pair (implementation plan §4's cards).

## Context digest

W13c and W13d are the register's next stop after W16b (M51, 0.71). Three landed pieces carry them. The kit's rules (`D-155`): a crate on core alone, private modules with one public path per item, opt-in plugins added after the engine's, if-present constraints, value types with private fields behind constructors, `deny(missing_docs)`, harness tests in the umbrella. W6a's bodies and queries (`D-154`): a kinematic body moves by its `Velocity` each substep and has no contact with static or kinematic bodies or tiles, which reach it only through casts and queries; a sensor sends `SensorEntered` once per new pair and nothing on exit; queries read the last step's final positions. The fixed step (`D-129`, `D-137`): `fixed_update` with the history snapshot first, `Time::delta()` the step inside it, input edges and event queues seen through step views.

The tiers come from the acceptance game's mechanics rows (`D-132`): the top-down mover, health and damage with the hit flash, trigger zones and the state machine are Must for its player, enemies, gems and boss; the platformer controller and the path follower are Must only if example 01 moves onto the kit by the freeze, since it is their only user. The card moves example 01's player now; W12b moves the rest of example 01, examples 03 and 04, and the template after them. The kit is inside the 1.0 promise through `tungsten::kit` (`D-103`) and joins the freeze, so the plan is level C: an API sketch (§2), a max-effort plan critique, and a critique of step 1's API before the rest runs.

Q22 is due when W13c starts; the plan takes its default (Q1). Decision 158 records the landing and amends `D-155`'s rule that every kit system runs in `PostUpdate`. No core, render or umbrella runtime file changes, so no hash, digest or reference image moves and no capture is owed.

## 1. Audit

Read at `2f46d1d`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The kit: `deny(missing_docs)`, private modules, each item re-exported once at the root; plugins add `PostUpdate` systems with if-present constraints (the spawner after `physics_sync` and `transform_propagate_late`). | `crates/tungsten-kit/src/lib.rs:42-61`, `crates/tungsten-kit/src/spawner.rs:133-137` |
| A2 | The kit depends on `tungsten-core`, `glam`, `serde` and `log` only. | `crates/tungsten-kit/Cargo.toml` |
| A3 | `PhysicsPlugin` registers `physics_prev_snapshot`, then `physics_step` after it, in `FixedUpdate`, and `physics_sync` in `PostUpdate`; the snapshot heads its stage. | `crates/tungsten-core/src/physics/plugin.rs:39-53` |
| A4 | A kinematic body moves by its `Velocity` each substep, without gravity, has no contact or event with static or kinematic bodies or tiles, and a `Position` write teleports it. A sensor takes part in no contact and sends `SensorEntered` per new pair at the step's end; there is no exit event. | `crates/tungsten-core/src/physics/components.rs:94-108`, `crates/tungsten-core/src/physics/events.rs:33-55` |
| A5 | Queries read one index of the last step's final positions. `SpatialFilter` holds one exclusion, which `excluding` replaces, and one switch for every body kind; there is no kind filter. `cast_shape` treats shapes as open, so a mover slides along a face, and a cast that starts inside a shape hits it at distance 0 only when moving into it. | `crates/tungsten-core/src/physics/query.rs:1-12`, `:56-129`, `:345-357` |
| A6 | `physics_step` steps `Time::delta()` bounded by `PhysicsConfig::max_step_dt`. `PhysicsConfig::gravity` defaults to zero; example 01 sets (0, 3600). | `crates/tungsten-core/src/physics/step/mod.rs:427-445`, `crates/tungsten-core/src/physics/mod.rs:102`, `examples/01_platformer/src/setup.rs:334` |
| A7 | The default extract reads `Sprite.material_id` and an optional `UniformOverrideBlock` and keys its batches by both; a lit sprite draws without its material and warns once. | `crates/tungsten-core/src/components.rs:52-57`, `crates/tungsten/src/sprite_extract.rs:206-216`, `:385-436` |
| A8 | `damage_flash` exists only in the root manifest, as a shader and a material; its shader mixes the sprite toward `material.v0` by `material.f.x`. The template's manifest has no shaders or materials. `MaterialRegistry::get` resolves a material by name. | `assets/manifest.json:87`, `:173`, `assets/shaders/damage_flash.wgsl`, `templates/basic/assets/manifest.json`, `crates/tungsten-core/src/assets/material.rs:84` |
| A9 | `SoundOnEvent` keeps a handled-count cursor to read each event once outside the steps; `D-155` left an exactly-once reader in core for a second user. `select` takes a plain `fn`. | `crates/tungsten-kit/src/sound.rs:85`, `:145-158` |
| A10 | Example 01's player is a dynamic box of half extents 20 × 28 that runs at 560 px/s and jumps at 1280 px/s under the world's 3600 px/s². | `examples/01_platformer/src/setup.rs:432-434`, `examples/01_platformer/src/state.rs:29-36` |
| A11 | Its grounding reads the step's `CollisionEvent`s and the sleep state after `physics_step`; its input writes `Velocity`: the run, a ground jump that a held key repeats on landing, and one air jump on a fresh press. | `examples/01_platformer/src/systems.rs:32-127`, `:268-318`, `:491-499` |
| A12 | Its schedule table puts `player_input` and `move_obstacles` before `physics_step` and `ground_detection` after it; a test pins the resolved text. | `examples/01_platformer/src/setup.rs:67-75`, `:84`, `examples/01_platformer/src/tests/main.rs:324-330` |
| A13 | A hit knocks the player back by writing `Velocity` and a control lock; a fireball blast pushes dynamic bodies only, which included the player. | `examples/01_platformer/src/gameplay.rs:571-605`, `examples/01_platformer/src/fireball.rs:394-425` |
| A14 | Moving platforms are static bodies teleported each step on a `travel · sin` path; they carry riders (the player, balls, bricks, scrap) by `Position` writes while a rider's `Velocity.y >= -1`. | `examples/01_platformer/src/gameplay.rs:224-238`, `:300-318`, `:387-460` |
| A15 | The route test taps Space for one frame and expects a full jump; the regime test needs the player's floor contacts in the HUD's count. | `examples/01_platformer/src/tests/level.rs:60-132` (`:100`), `examples/01_platformer/src/tests/regime.rs:85-200` (`:153`) |
| A16 | Example 01 keeps its own health, a hit flash made of a `Tween` on the override block, and an immunity blink. | `examples/01_platformer/src/gameplay.rs:23-39`, `examples/01_platformer/src/systems.rs:501-532`, `examples/01_platformer/src/state.rs:213-219` |
| A17 | Example 01's test harness runs listed `fn` systems in `Update` without `PhysicsPlugin`; the route, pit and player tests use `PHYSICS_SYSTEMS` (input, step, ground detection). | `examples/01_platformer/src/tests/main.rs:78-106` |
| A18 | The umbrella's kit tests pin the `post_update` and `fixed_update` lines of `kit_app()`; there are 14 tests. | `crates/tungsten/src/tests/kit.rs:21-73` |
| A19 | The state stack's `GameState` has enter, exit and update for game states; nothing runs per entity. | `crates/tungsten/src/state.rs:33-40` |
| A20 | Core's `Easing::SineInOut` is the profile of example 01's `travel · sin` platforms. | `crates/tungsten-core/src/tween.rs:30` |
| A21 | Records: `q22` is open in the catalog and in criteria §10; the `w13cd` stop's title names spawners, which M51 landed; the known issues and `w13-kit.md` hand the spawned-entity state-ownership follow-up to "W13c and W13d". | `docs/plans/1.0/roadmap.json:588`, `docs/plans/1.0/criteria.md` §10, `docs/known-issues.md:54`, `docs/plans/1.0/w13-kit.md:47` |

## 2. Design

### 2.1 Rules carried, and where each item runs

Every item follows `D-155`: a private module, one public path at the crate root, rustdoc with a runnable example on a bare `World` and `Schedule`, and an opt-in plugin, a unit struct or, for a generic item, a value named by the game as `SoundOnEvent` is, which the game adds after the engine's plugins and `DefaultPlugins` never holds. Value types keep private fields behind constructors and `with_` builders, which panic with `#[track_caller]` on non-finite or out-of-range arguments. Constraints on the engine's names and between kit plugins are if-present. The kit's dependencies do not change (A2), and steps 1 and 2 change nothing outside the kit, its snapshot and its harness tests.

Decision 158 amends `D-155`'s `PostUpdate` rule: what drives a body or times a hit runs in `FixedUpdate`, before `physics_step`, so it acts in the same step and on the fixed-step clock, and the zones read that step's sensor events after it. One `fixed_update`:

| Order | System | Constraints, each if-present |
| --- | --- | --- |
| 1 | `physics_prev_snapshot` (engine) | Heads the stage (A3) |
| 2 | `health_tick` | After the snapshot; before `path_follow`, both controllers and `physics_step` |
| 3 | A `StateMachinePlugin`'s system, named by the game | After the snapshot and `health_tick`; before `path_follow`, both controllers and `physics_step` |
| 4 | `path_follow` | After the snapshot; before both controllers and `physics_step` |
| 5 | `platformer_controller`, `top_down_mover` | After the snapshot and `path_follow`; before `physics_step` |
| 6 | `physics_step` (engine) | — |
| 7 | A `TriggerZone`'s system, named by the game | After `physics_step` |

`hit_flash_tick` runs in `PostUpdate` after `tween_tick`. A game's input system for a controller runs in `fixed_update` before it (`.before(PLATFORMER_CONTROLLER)`), as each item's rustdoc example shows.

### 2.2 Character controllers (W13c)

```rust
// platformer.rs
pub const PLATFORMER_CONTROLLER: &str = "platformer_controller";

#[derive(Debug, Clone)]
pub struct PlatformerController { /* private */ }
impl PlatformerController {
    pub fn new(run_speed: f32, jump_speed: f32) -> Self;     // each finite, > 0
    pub fn with_gravity(self, gravity: f32) -> Self;         // finite, >= 0, px/s², +y down; default: the world's
    pub fn with_coyote_time(self, seconds: f32) -> Self;     // finite, >= 0; default 0.1
    pub fn with_jump_buffer(self, seconds: f32) -> Self;     // finite, >= 0; default 0.1
    pub fn with_jump_cut(self, factor: f32) -> Self;         // in [0, 1]; default 0.5; 1 keeps every jump full
    pub fn with_filter(self, filter: SpatialFilter) -> Self; // what blocks it; default SpatialFilter::new()
    pub fn set_run(&mut self, axis: f32);                    // clamped to [-1, 1], NaN as 0; held until changed
    pub fn press_jump(&mut self);                            // starts the jump buffer
    pub fn set_jump_held(&mut self, held: bool);             // held until changed
    pub fn launch(&mut self, velocity: Vec2);                // replaces its velocity at its next step
    pub fn push(&mut self, velocity: Vec2);                  // added to its velocity at its next step; pushes sum
    pub fn lock_input(&mut self, seconds: f32);              // the longer of this and the lock left
    pub fn grounded(&self) -> bool;                          // the getters read its last step
    pub fn can_jump(&self) -> bool;                          // grounded or in coyote time, and unlocked
    pub fn jumped(&self) -> bool;                            // its last step started a jump
}
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformerControllerPlugin;

// top_down.rs
pub const TOP_DOWN_MOVER: &str = "top_down_mover";

#[derive(Debug, Clone)]
pub struct TopDownMover { /* private */ }
impl TopDownMover {
    pub fn new(speed: f32) -> Self;                          // finite, > 0
    pub fn with_filter(self, filter: SpatialFilter) -> Self;
    pub fn set_direction(&mut self, direction: Vec2);        // longer than 1 is normalized, non-finite stops; held until changed
    pub fn direction(&self) -> Vec2;
}
#[derive(Debug, Default, Clone, Copy)]
pub struct TopDownMoverPlugin;
```

Each step, for each entity with the controller, a `Position`, a `Velocity`, a `Collider` and a `RigidBody` of kind `Kinematic` (any other is skipped, with one `log::warn` per entity, Q13):

1. **Step length.** `dt` is the step as `physics_step` takes it, `Time::delta()` bounded by `PhysicsConfig::max_step_dt` (A6); 0 does nothing.
2. **First step.** A controller's first step moves nothing: it writes a zero `Velocity` and keeps its input, so the physics step after it indexes the floor and walls it was spawned among, which its casts could not see yet (queries find nothing before a world's first step, and nothing spawned since the last one, A5). It moves from its second step.
3. **Ground check.** Unless it is rising, its collider casts 1 px down from `Position + offset` through its filter, its own body excluded (which replaces the filter's exclusion, A5). A hit whose normal points up (`n.y <= -0.7`, y down) grounds it and snaps it down to the surface; that body, when kinematic, is its ground.
4. **Launch.** A pending `launch` replaces its velocity, ungrounds it, ends coyote time and disarms the jump cut.
5. **Input**, unless locked: `vx = run · run_speed`, and a buffered press while grounded or with coyote time left jumps (`vy = -jump_speed`; buffer and coyote time cleared; `jumped`; the cut armed). While locked, presses are dropped and `vx` holds.
6. **Push.** The pushes made since its last step are added to its velocity, after the input, so a black hole's pull or a blast's kick acts as it does on a dynamic body.
7. **Variable jump.** An armed cut with the jump released while rising scales `vy` by the cut factor, once.
8. **Gravity**, `with_gravity`'s or `PhysicsConfig::gravity.y` read each step (Q3). On the ground and not moving up, `vy = 0`. Otherwise the step moves at `vy + g·dt/2` and ends at `vy + g·dt`, so positions lie on the exact parabola at step boundaries and a jump peaks within a quarter pixel of `jump_speed² / 2g`: 227.6 px in example 01, whose dynamic body, integrated per substep, peaked near 224.9.
9. **Move and slide.** The displacement casts against the last step's positions (A5) in up to four slides; each hit stops it 0.01 px short and projects the rest onto the surface. A floor hit while falling lands it (grounded, `vy = 0`), a ceiling hit while rising zeroes `vy`, and a wall zeroes the held `vx`.
10. **Carry.** On a kinematic ground with a `Velocity`, with no jump, launch or upward push this step, it adds the ground's `velocity · dt` uncast, so it rides a path follower.
11. **Write.** It sets `Velocity = displacement / dt`, which its kinematic body moves by in this step's substeps, and keeps its own end velocity.
12. **Timers** tick on the game clock last: the input lock, the jump buffer, and coyote time, refilled when the step ends grounded. Its getters and its next jump decision so read the same state.

The top-down mover takes steps 1, 2, 9 and 11 with `direction · speed`: it slides along walls, stops at bodies, and a dynamic body pressing into it sends a `CollisionEvent` whose `a` is that body (`D-154`), the acceptance game's contact damage. A private `slide` module holds the cast loop both controllers use.

The default filter sees bodies and tiles and no sensors, so dynamic bodies block a controller as walls do and hold it up as floors do; it never pushes one (Q2). Two controllers do not see each other's motion within a step, a body or tile spawned since the last step stays unseen until the next one, and nothing pushes a kinematic body out of an overlap, though a cast lets it leave one (A5).

### 2.3 Health, damage and the hit flash (W13d)

```rust
// health.rs
pub const HEALTH_TICK: &str = "health_tick";                // FixedUpdate: invulnerability windows
pub const HIT_FLASH_TICK: &str = "hit_flash_tick";          // PostUpdate: flashes
pub const HIT_FLASH_MATERIAL: &str = "damage_flash";

#[derive(Debug, Clone)]
pub struct Health { /* private */ }
impl Health {
    pub fn new(max: f32) -> Self;                            // finite, > 0; starts full
    pub fn with_invulnerability(self, seconds: f32) -> Self; // finite, >= 0; the window after each hit; default 0
    pub fn current(&self) -> f32;
    pub fn max(&self) -> f32;
    pub fn is_dead(&self) -> bool;
    pub fn is_invulnerable(&self) -> bool;
    pub fn heal(&mut self, amount: f32);                     // finite, >= 0; up to max; the dead stay dead
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hit { Ignored, Hurt, Killed }

pub fn damage(world: &mut World, target: Entity, amount: f32) -> Hit;

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Damaged { pub entity: Entity, pub amount: f32, pub killed: bool }
impl Damaged { pub fn new(entity: Entity, amount: f32, killed: bool) -> Self; }

#[derive(Debug, Clone)]
pub struct HitFlash { /* private */ }
impl HitFlash {
    pub fn new(color: [f32; 4], seconds: f32) -> Self;       // color finite; seconds finite, > 0
    pub fn is_active(&self) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HealthPlugin;                                     // EventQueue<Damaged>, HEALTH_TICK, HIT_FLASH_TICK
```

`damage` acts at once (Q7), so a caller knows the outcome in the same step, as the blades' one hit per enemy per window needs, and no reader needs `D-155`'s exactly-once cursor (A9). A negative or non-finite amount panics. An amount of 0, a target without `Health`, a dead one or one inside its window returns `Hit::Ignored`. Otherwise health falls by the amount to no less than 0, the window starts, a `HitFlash` on the target starts, a `Damaged` is sent when `HealthPlugin` has registered its queue, and the result is `Hurt` or `Killed`. `Damaged::amount` is the amount passed, for damage numbers.

`health_tick` counts every window down by the step in `fixed_update`, first after the snapshot (§2.1), so a hit sequence on the fixed-step clock applies the same hits however the frames group the steps, whereas a `PostUpdate` count would charge a window for the frame time before its hit and let two hits a step apart both land or not by the frame rate. A paused game runs no step, so windows hold.

The flash draws through the `damage_flash` material (A8, Q5). At a hit on an entity with a `HitFlash` and a `Sprite`, once per flash, it keeps the sprite's material and the entity's `UniformOverrideBlock`, or that it had none; then it sets `Sprite.material_id` to `damage_flash`'s and writes `vec4[0] = color` and `f32s[0] = 1.0` into the block, inserting one if absent. `hit_flash_tick` lowers `f32s[0]` linearly to 0 over `seconds` by `Time::delta()` in `PostUpdate`, leaving the first frame at full strength, then restores the kept material and block, removing a block the flash inserted: a block present overrides the material's authored defaults in the renderer, so leaving one behind would change the material (`crates/tungsten-render/src/sprite.rs:1024`). While it flashes, the flash owns the sprite's material and block, so a change made to either then is lost at the restore. Without the material in `MaterialRegistry` nothing flashes and `hit_flash_tick` warns once. A lit sprite draws without its material (A7), so it does not flash; a flashing sprite is a batch of its own until its flash ends; a game `Tween` on the same block loses to the flash, which runs after `tween_tick`. The kit writes only `Sprite.material_id` and the override block, which the default extract reads (`D-007`, `D-016`).

### 2.4 Trigger zones (W13d)

```rust
// trigger.rs
pub struct TriggerZone<Z, T> { /* private: the system's name */ }
impl<Z: 'static, T: 'static> TriggerZone<Z, T> {
    pub fn new(name: &'static str) -> Self;                  // a Plugin: EventQueue<TriggerEntered<Z, T>> and its system
}

pub struct TriggerEntered<Z, T> {
    pub zone: Entity,
    pub other: Entity,
    /* private marker, so it is built with new */
}
impl<Z, T> TriggerEntered<Z, T> { pub fn new(zone: Entity, other: Entity) -> Self; }
// Clone, Copy, Debug, PartialEq and Eq for every Z and T.
```

Zones and targets are the game's own component types, which its prefab files already name (`D-153`), so a spawned gem needs no kit component (Q8). Each step the system reads the step's `SensorEntered` in order (`iter_current`, A4) and sends `TriggerEntered::<Z, T>::new(sensor, other)` for each whose sensor carries a `Z` and whose `other` is an entity carrying a `T`; a tile (`other: None`) never triggers. Two zones over different types are two plugins with two names, as two `SoundOnEvent`s are. Entering only, as the physics reports it.

### 2.5 Path follower (W13d)

```rust
// path.rs
pub const PATH_FOLLOW: &str = "path_follow";

#[derive(Debug, Clone)]
pub struct PathFollower { /* private */ }
impl PathFollower {
    pub fn new(points: impl IntoIterator<Item = Vec2>, speed: f32) -> Self; // >= 2 finite points, no zero-length segment; speed finite, > 0
    pub fn looping(self) -> Self;                            // after the last point, on to the first (that segment checked too)
    pub fn ping_pong(self) -> Self;                          // back along the points, and again
    pub fn with_easing(self, easing: Easing) -> Self;        // each segment's progress; default Easing::Linear
    pub fn is_finished(&self) -> bool;                       // a path that neither loops nor ping-pongs, at its end
}
#[derive(Debug, Default, Clone, Copy)]
pub struct PathFollowerPlugin;
```

Each step it advances its time by `dt` along segments that each take their length over `speed`, eases each segment's progress (`SineInOut` gives example 01's `travel · sin` platforms, A20), carries leftover time into the next segment, and sets `Velocity = (point at the new time − Position) / dt`, so a kinematic body ends each step on the path and pushes the dynamic bodies it meets (`D-154`). The points are world positions, and the body starts at the first; of `looping` and `ping_pong`, the last call wins. Finished, it sets `Velocity` to zero.

### 2.6 Per-entity state machine (W13d)

```rust
// state_machine.rs
#[derive(Debug, Clone)]
pub struct StateMachine<S> { /* private */ }
impl<S: Copy + Eq + 'static> StateMachine<S> {
    pub fn new(initial: S) -> Self;
    pub fn state(&self) -> S;
    pub fn previous(&self) -> Option<S>;
    pub fn elapsed(&self) -> f32;                            // game time in the current state
    pub fn go(&mut self, next: S);                           // taken at its next visit; the last call wins
}

pub struct StateMachinePlugin<S> { /* private: the system's name and the hooks */ }
impl<S: Copy + Eq + 'static> StateMachinePlugin<S> {
    pub fn new(name: &'static str) -> Self;
    pub fn on_enter(self, state: S, hook: fn(&mut World, Entity)) -> Self;
    pub fn on_exit(self, state: S, hook: fn(&mut World, Entity)) -> Self;
    pub fn on_update(self, state: S, hook: fn(&mut World, Entity)) -> Self;
}
```

Hooks are plain `fn` pointers, as `SoundOnEvent::select` takes one (A9); a second hook of one kind for one state panics. Each step the system visits the machines in entity order. A first visit enters the state `go` asked for, else the initial one. A pending `go` runs the old state's exit hook, then the new state's enter hook, and resets `elapsed`; `go` to the current state re-enters it. Then the current state's update hook runs and `elapsed` grows by `dt`. A `go` made during a visit waits for the next step, so a step runs at most one transition per entity, and a hook that despawns its entity or removes its machine ends that entity's visit. It runs where the boss's movement needs it (Q9); a paused game runs no fixed step, so machines hold.

### 2.7 Example 01's player on the controller (step 3)

The player spawns as `RigidBodyBundle::kinematic(Position(PLAYER_SPAWN), Collider::aabb(PLAYER_HALF))` with `PlatformerController::new(PLAYER_MOVE_SPEED, PLAYER_JUMP_IMPULSE)`: the world's gravity (3600) and the kit's coyote time, jump buffer and jump cut (Q4). `configure_app` adds `PlatformerControllerPlugin`. `player_input` runs before `platformer_controller` and `move_obstacles` after it, so the controller reads a platform's deck before the platform teleports its riders (A14).

- **`player_input`** calls `set_run(dx)` and `set_jump_held(held)`. Outside the example's own control lock, a fresh press while `!can_jump()` with the air jump unused calls `launch(Vec2::new(0.0, -PLAYER_JUMP_IMPULSE))`, with the double-jump effect and sound as now; otherwise a fresh press, or the key held while `grounded()`, calls `press_jump()`, which keeps the ground jump's hold behaviour (A11) and adds the buffer.
- **`player_state`**, in `ground_detection`'s slot after `physics_step`, mirrors `grounded()` into `Player.grounded`, which the presentation and the HUD read as now; refills the air jump on the ground; sends the landing squash and effect on the rising edge; and on `jumped()` sets the jump effect at the feet of the step's start (`PrevPosition`) and plays the jump sound. `ground_detection` and `player_is_grounded_by_event` go.
- **Pushes.** The two systems that move only dynamic bodies, and so moved the player, reach it through the controller now. `damage_player` knocks the player back with `launch(Vec2::new(direction * 380.0, -420.0))` and `lock_input(0.18)` beside its own `control_lock`. `push_bodies` (the fireball blast) gives a body with a `PlatformerController` its kick through `push` and `lock_input` with the lock it computes, and `black_hole_force_system` its pull through `push` each step, with the response a unit-mass body has; dynamic bodies as now.
- **Behaviour that changes**, for the owner's playthrough: balls, bricks and scrap block the player and hold it up instead of yielding (Q2); the player has no contact with floors or walls, so the HUD's contact count drops; a jump peaks about 2.7 px higher; a tap gives a short hop and a held jump the full height (Q4); walking off an edge leaves a ground jump for 0.1 s before the air jump; a new run's player holds still for its first step (§2.2 step 2).
- **Tests**, each changed only where it reads the old body: the harness behind `PHYSICS_SYSTEMS` runs the fixed-step physics with the plugin and the example's input and state systems around it, since the kit's system is private; `spawn_test_player` matches the spawn; `RUNTIME_SCHEDULE` is pinned again; the route test holds Space while the player rises and replaces a `LAUNCHES` row only with the row it prints; the regime test takes its contacts from a dynamic box it drives along the row, and the player's walk keeps the seams check; `player_becomes_grounded_when_collision_event_lists_player_as_b` goes with the event path it tested. The example gains no test.
- **`tools/README.md`**: its grounding, hold and double-jump paragraphs describe the controller.

### 2.8 What does not change

No file under `crates/tungsten-core`, `crates/tungsten-render` or `examples/02_bench` changes, none under `crates/tungsten/src` but `tests/kit.rs`, and not `crates/tungsten-kit/Cargo.toml`. So `api/tungsten-core.txt`, `api/tungsten-render.txt` and `api/tungsten.txt`, which prints the kit as one re-export line, stay as they are, as do `DefaultPlugins`' resolved schedule, the template and its snapshot, the physics hashes, the benchmark digests and the pixel references (§6).

## 3. Steps

### Step 1: The kit's controllers and helpers

- **Files:** `crates/tungsten-kit/src/lib.rs`; new `crates/tungsten-kit/src/{platformer,top_down,slide,health,trigger,path,state_machine}.rs` and `crates/tungsten-kit/src/tests/{platformer,top_down,health,trigger,path,state_machine}.rs`; `api/tungsten-kit.txt`
- **Change:** §2.2–§2.6's items with their plugins and name constants, each module private, its items re-exported at the root and listed in the crate docs, `slide` private. Rustdoc on every public item (`deny(missing_docs)` holds it) and one runnable example per item, as the existing items' examples run. Unit tests in `src/tests/` for the rules that need no engine frame: argument panics and clamping; the controller's first step, timers, cut, parabola, `launch`, `push` and input lock against a tile floor and a static box under a hand-driven `physics_step`; the path follower's modes, easing and leftover time; the state machine's hook order and `go` rules; `damage`'s outcomes and the flash's material and override writes. Then `just api` and the snapshot diff read, then step 1's API critique as the skill prescribes, in the background: `critique --raw --repo --force --out <scratchpad>/api-critique.md --from 'git --no-pager diff HEAD -- api/ templates/; git ls-files -o --exclude-standard -z -- api templates | xargs -0 -r -n1 git --no-pager diff --no-index /dev/null'`. Each finding is checked against the repo, recorded in §9 as `C.<n>`, and changes step 1 where it holds. This step is the milestone's bulk; the run may compact after it with its focus on steps 2–5.
- **Done-when:**
  - `cargo test -p tungsten-kit --locked` → exit 0, its doctests naming each new module (quoted).
  - `cargo clippy -p tungsten-kit --all-targets --locked -- -D warnings` and `cargo fmt --all -- --check` → exit 0.
  - `for f in platformer top_down health trigger path state_machine; do rg -c '^/// ```$' "crates/tungsten-kit/src/$f.rs"; done` → at least 2 each.
  - `git diff --quiet HEAD -- crates/tungsten-kit/Cargo.toml` → exit 0.
  - After `just api`: `git diff --name-only HEAD -- api/` → `api/tungsten-kit.txt` alone; `comm -23 <(git cat-file -p HEAD:api/tungsten-kit.txt | sort) <(sort api/tungsten-kit.txt) | wc -l` → 0; `rg -c '^(#\[non_exhaustive\] )?pub (struct|enum) tungsten_kit::(PlatformerController|PlatformerControllerPlugin|TopDownMover|TopDownMoverPlugin|Health|HealthPlugin|HitFlash|Hit|Damaged|TriggerZone|TriggerEntered|PathFollower|PathFollowerPlugin|StateMachine|StateMachinePlugin)(<[A-Z, ]+>)?$' api/tungsten-kit.txt` → 15; `rg -c '^pub const tungsten_kit::(PLATFORMER_CONTROLLER|TOP_DOWN_MOVER|HEALTH_TICK|HIT_FLASH_TICK|PATH_FOLLOW|HIT_FLASH_MATERIAL): ' api/tungsten-kit.txt` → 6; `rg -c '^pub fn tungsten_kit::damage\(' api/tungsten-kit.txt` → 1.
  - The API critique's footer line and each `C.<n>` row in §9, or its not-run line with the error.
- **Moves:** none.

### Step 2: Harness tests and the physics release checks

- **Files:** `crates/tungsten/src/tests/kit.rs`; step 1's kit modules and their unit tests, only to fix a fault a harness test finds, with the public surface unchanged
- **Change:** `kit_app()` adds the six new plugins after the existing ones, one `TriggerZone` and one `StateMachinePlugin` over test types, and the resolve test pins the new `fixed_update` and `post_update` lines, which keep §2.1's order. New tests through the engine's frame with `DefaultPlugins`: (a) in a fresh world with no step run yet, a platformer spawned on a tile floor against a static box, its run input set before its first frame, never enters either, then lands, runs, stops at a tile wall and the box, and stops rising at a ceiling; (b) a held jump peaks within 0.25 px of `jump_speed² / 2g`, an early release lower, and with a cut of 1 a release changes nothing; (c) coyote time takes a press made just after walking off an edge, and the buffer a press made just before landing, each inside its window and not after it; (d) the platformer rides a ping-pong `PathFollower` platform both ways and leaves it on a jump; (e) `launch` with `lock_input` holds a knockback's horizontal speed for the lock, then the run input steers, and a `push` each step bends its path as a pull does; (f) the top-down mover, spawned against a wall in a fresh world, never enters it, slides along it on a diagonal and stops at a dynamic body, whose press into it sends a `CollisionEvent` naming it as `a`; (g) `damage` ignores hits inside the window, holds the window while paused, kills at zero and sends one `Damaged` per hit it applies, and one hit sequence on the fixed-step clock applies the same hits at frame dts of 1/30, 1/60 and 1/144 s; (h) a flashing sprite's batch in `Harness::draw` carries `damage_flash`'s material and a falling `f32s[0]`, and afterwards its own material and override block again, both for a material whose block carries values of its own and for a sprite that had no block; (i) a `TriggerZone` sends one `TriggerEntered` per entry of its target type and none for another body or a tile; (j) a looping and a finished `PathFollower` each end every step on the path; (k) a state machine runs exit, enter and update in that order in `fixed_update` before the step, so an update hook's `Velocity` write moves its body in the same step, and `elapsed` follows the game clock. Then `just physics-release`, the card's check.
- **Done-when:**
  - `cargo test -p tungsten --locked kit_tests` → exit 0; `rg -c '^#\[test\]' crates/tungsten/src/tests/kit.rs` → at least 25.
  - `cargo test -p tungsten-kit --locked` and `just api-check` → exit 0, so a fix to step 1's files kept the surface its critique read.
  - `just physics-release` → exit 0; its determinism and containment tests assert `0x088e_c07a_73c1_b168` and `0xaee2_72e0_1ffc_4e4c`.
  - `git diff --quiet HEAD -- crates/tungsten-core crates/tungsten-render examples/02_bench crates/tungsten/Cargo.toml` → exit 0; `git diff --name-only HEAD -- crates/tungsten/src` → `crates/tungsten/src/tests/kit.rs` alone; `git ls-files -o --exclude-standard -- crates/tungsten-core crates/tungsten-render crates/tungsten examples/02_bench` → empty.
- **Moves:** none.

### Step 3: Example 01's player onto the platformer controller

- **Files:** `examples/01_platformer/src/{setup,systems,state,gameplay,fireball}.rs`; `examples/01_platformer/src/{death,extract,brick}.rs` only where a read of the player's body needs it; `examples/01_platformer/src/tests/{main,player,level,regime,presentation,fixed_step,hazards,death,ball_pit,ice,brick,camera,spells}.rs`, each only where it reads the old dynamic player; `examples/01_platformer/tools/README.md`
- **Change:** §2.7.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked` → exit 0, with `authored_routes_and_recovery_shelves_traverse_with_real_physics`, `every_regime_walks_the_slab_joins_and_reads_each_collision_once` and `runtime_schedule_matches_expected_pipeline` named in its output, and every `LAUNCHES` row it replaced quoted.
  - `cargo clippy -p example-01-platformer --all-targets --locked -- -D warnings` → exit 0.
  - `rg -c 'fn ground_detection|fn player_is_grounded_by_event' examples/01_platformer/src/systems.rs` → no match (exit 1).
  - `rg -c 'RigidBodyBundle::kinematic' examples/01_platformer/src/setup.rs examples/01_platformer/src/tests/main.rs` → 1 each.
  - `just smoke` → exit 0.
  - Owner-only, at the ship sitting: a playthrough of example 01 on the controller (the card's check), with §2.7's changed behaviour in view.
- **Moves:** example 01's runtime schedule and its player's motion (§2.7), on purpose; none of §6's invariants.

### Step 4: The decision and the docs

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/{w13-kit,w04-api-freeze,backlog-1.x,criteria}.md`, `docs/plans/1.0/roadmap.json`
- **Change:** First decision 158 through the tungsten-decision skill, with its index row, the marker line on `D-155` and the amendment in `D-155`'s index row (§5). Then: `DESIGN.md`'s M52 section after M50's, and the kit clause of its opening status line. The LLM index's kit row names the new items and the decision. Known issues: a recorded-limits bullet for the kit's controllers and helpers (§2.2's last paragraph, the uncast carry, a lit sprite's missing flash, a flash over a game tween, a change to a flashing sprite's material or block lost at the restore, enter-only zones), and the spawned-entities bullet's next work without "W13c and W13d" (Q12). `w13-kit.md`: a revision note and the follow-ups below, its spawned-entities line updated the same way. The break ledger: one row of W13c and W13d additions, no breaks. The backlog: a row for slopes, one-way platforms and ladders (Q1). Criteria §10: Q22 moved to Answered with its default and the decision. `roadmap.json`, by textual edit: `q22` to state `default` with that answer, and the `w13cd` stop's title and summary without the spawner.
- **Done-when:**
  - `rg -c '^## D-15[8] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-15[8]:\*\*' DECISIONS.md` → 1; ``rg -c '^\| `D-15[8]` \|' docs/DECISION_INDEX.md`` → 1; ``rg -c '^\| `D-155` \|.*Amended by `D-15[8]`' docs/DECISION_INDEX.md`` → 1.
  - `rg -c '^### .* — M52$' DESIGN.md` → 1; `rg -c '^\| Kit: .*platformer controller' docs/LLM_INDEX.md` → 1.
  - ``rg -c '^- \*\*The kit.s controllers and helpers\*\* \(`D-15[8]`, M52\)' docs/known-issues.md`` → 1; `rg -c 'W13c and W13d' docs/known-issues.md` → no match.
  - `rg -c '\| W13c and W13d \(M52 ' docs/plans/1.0/w04-api-freeze.md` → 1; `rg -c -i '^\| .*slopes.*one-way' docs/plans/1.0/backlog-1.x.md` → 1.
  - `sed -n '/^## 10\./,/^### Answered/p' docs/plans/1.0/criteria.md | rg -c '^- \*\*22\.\*\*'` → no match; `sed -n '/^### Answered/,$p' docs/plans/1.0/criteria.md | rg -c '^- \*\*22\.\*\*'` → 1.
  - `rtk proxy jq -r '.questions[] | select(.id == "q22") | .state' docs/plans/1.0/roadmap.json` → `default`; `rtk proxy jq -r '.stops[] | select(.id == "w13cd") | .title' docs/plans/1.0/roadmap.json | rg -c -i spawner` → no match.
  - `just ctx` and `just repo-check` → exit 0.
- **Moves:** none.

### Step 5: Release 0.72

- **Files:** `CHANGELOG.md`, `DESIGN.md` (its status line; step 4 wrote the section), `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.72.0` once, every release check once after it, then the preflight with `--message`. No gate by change is left for the release: `just physics-release` ran in step 2 and `just smoke` in step 3, and the release chain runs smoke again.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.72.0 --repo JosephGibson/Tungsten --message 'Update 0.72: M52 controllers and gameplay helpers'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, W11b and W11c: `Use the tungsten-milestone skill. Write the milestone plan for W11b and W11c: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/w11-shipping-basics.md; audit only the code it touches; name it docs/plans/1.0/phase6-milestone-53-settings-save-slots.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Q22: controller features beyond the ground check, coyote time, jump buffer and variable jump height, such as slopes, one-way platforms or ladders? | No, criteria §10's default: those three go to the backlog (step 4). Example 01's air jump stays its own, made with `launch`. |
| Q2 | What blocks a controller, and does it push dynamic bodies? | Its `SpatialFilter`, by default bodies and tiles but no sensors, its own body excluded: dynamic bodies block it and hold it up as walls and floors do, and it never pushes one. Example 01's balls, bricks and scrap stop the player instead of yielding (§2.7), for the owner's playthrough. A push, or a body-kind filter, changes core and waits; W6b's layers and masks let a game pick what its controller sees. |
| Q3 | Where does the platformer's gravity come from? | The world's, `PhysicsConfig::gravity.y` read each step, unless `with_gravity`; its rustdoc says a zero-gravity world needs `with_gravity`. |
| Q4 | Example 01's jump: variable or fixed height? | Variable, at the kit's 0.5 cut: a tap hops and a held jump reaches the full height; the route test holds Space while the player rises; the hold that repeats a ground jump stays. |
| Q5 | Where does a game get `damage_flash`? | The root manifest keeps its shader and material for the examples. The kit names the material in `HIT_FLASH_MATERIAL`, and its rustdoc states the uniform contract and the two root files a game copies. The template ships neither (Q11); W12b and W9b's guide decide whether it should. |
| Q6 | Health as `f32` or an integer? | `f32`: fractional damage and healing, shown rounded; example 01's hearts become whole numbers when W12b moves it. |
| Q7 | Damage: an immediate call, or an event a system applies? | Immediate: `damage(world, target, amount) -> Hit`, and `Damaged` for observers. `D-155`'s open exactly-once reader stays open. |
| Q8 | Trigger zones: a kit component on the zone, or the game's types? | The game's zone and target types, `TriggerZone<Z, T>`, which prefab files already name; enter only until a zone needs `D-154`'s exit events. |
| Q9 | The state machine's stage and hooks? | `fixed_update`, before the path follower, the controllers and `physics_step`, as the boss's movement needs; another stage waits for a user. Hooks are `fn` pointers. |
| Q10 | Scene components for the new items? | None, as `D-155` left `lifetime` and `despawn_outside`: a game's own component builds the kit's in its `register_component_with` builder (`D-153`), which is how a spawner's enemies get `Health`. |
| Q11 | The template's player onto the top-down mover? | No: W12b's card has the template follow the examples, the template has no walls for a mover to slide along, and the acceptance game, the mover's user, adds it after starting from the template. |
| Q12 | Example 01's health, hit flash, hazards and moving platforms onto the kit now? | No: the card moves the player only, and W12b moves the rest, the path follower's user among them (`D-132`). The spawned-entity state-ownership follow-up stays with the acceptance game's first gap; step 4 drops "W13c and W13d" from it. |
| Q13 | A controller on a body that is not kinematic, or without a collider? | Skipped, with one warning per entity; a dynamic body would fight the solver. |
| Q14 | The controller's constants? | A 1 px ground probe, a 0.01 px skin, four slides, a floor where `n.y <= -0.7`; coyote time and jump buffer 0.1 s and a 0.5 cut, which the builders change. |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

158: W13c and W13d landed (M52): the kit's platformer controller and top-down mover on kinematic bodies in `fixed_update` before `physics_step` (Q22's four features, a still first step, a launch, a push and an input lock, move and slide over the last step's casts, the ground's carry), health with an invulnerability window counted on the fixed step, `damage` and its `Damaged` event, the hit flash on `damage_flash` that restores the sprite's material and override block, trigger zones over `SensorEntered` keyed by the game's types, a path follower and a per-entity state machine; example 01's player on the platformer controller. Amends `D-155`: the systems that drive bodies or count windows run in `fixed_update` before `physics_step`, and the zones after it. `D-103`, `D-132`, `D-153` and `D-154` stand. The next free ID when written (`D-157` is the last); step 4 checks it again.

## 6. Invariants

- **May move:** example 01's resolved schedule (`RUNTIME_SCHEDULE`) and its player's motion (step 3, §2.7), recorded in the evidence log and the `CHANGELOG.md` line; `api/tungsten-kit.txt`, which gains lines and loses none (step 1).
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168` and the pinned containment hash `0xaee2_72e0_1ffc_4e4c` (step 2's `just physics-release`); the eight benchmark row digests, `gpu-visual.png` and the post-stack, transition and UI overlay regressions (no file their builds read changes: step 2's diff check); `DefaultPlugins`' resolved schedule and the template's snapshot (their tests, in the release's `just check`); `api/tungsten-core.txt`, `api/tungsten-render.txt` and `api/tungsten.txt` (step 1's `git diff --name-only`); the existing kit items' behaviour (their unit tests unchanged; their harness tests changed only in `kit_app()`'s plugin list and the pinned stage lines).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 2 and 3 need step 1; step 4 records what landed; the release needs steps 1 and 2, since a kit item ships only with its harness test (`D-155`) and restoring step 2's test file would otherwise hide the fault it found. A harness test that finds a fault in step 1's code is fixed within step 2 (its files); a step 2 that still fails ends the run there, its report naming the failing test, and the release waits for a session that finishes the plan.
- A file outside §files to touch needs a change, `tungsten-core`, `tungsten-render` or the umbrella's runtime among them: restore the step's files, skip it and record the need as a follow-up. An API-critique finding whose fix needs such a file becomes an open question with a default, not a longer list.
- Step 3's route test reports a route not traversable on the controller, or a test passes only once what it checks is weakened: step 3 is skipped with its files restored, and the platformer ships without its example 01 user, which waits for W12b; step 4 records it.
- `just physics-release` fails or asserts another hash: stop the run, since no physics file changes here; the release waits for a release session.
- `pgrep -af scripts/bench.py` shows a capture: write nothing to the tree until it ends (workflow §6).
- Decision 158 is taken when step 4 runs: take the next free ID and record it in the evidence log.
- The API critique fails, as on a usage limit or a Codex error: record it as not run with its error line and continue (`D-150`).

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–3, release | The kit's tests, clippy and format (1); the umbrella's harness tests, `just api-check` and `just physics-release`, the card's check (2); example 01's tests and `just smoke` (3). No `just visual`, since no renderer or pixel-test input changes, and no perf row, since no row builds kit or example 01 code and no shared ECS or frame-loop file changes (step 2's diff check). The release step runs `just check`, `just smoke`, `just script-test` and the rest of the release checks once, after the cut |
| 3 Invariants | 1, 2 | §6 |
| 4 Decisions | 4 | 158, its index row and `D-155`'s marker, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 4 | Rustdoc on every item under `deny(missing_docs)` and `just api` (1); the ledger's row of additions, no breaks (4) |
| 6 Template and guide | n/a | No change to how a game is written: the items are opt-in plugins and the template's player stays (Q11); W9b has not started |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 4 | The kit row; no new `AGENTS.md` rule |
| 8 Design | 4 | `DESIGN.md`'s M52 section |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 4, release | Known issues gain the recorded limits; the backlog gains Q1's row; the gap log is empty, so nothing closes; `w13-kit.md`, criteria §10 and `roadmap.json` (4); the register row's release and status, and the README's owned fields from `python3 -B scripts/roadmap.py summary` (release) |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, on 2026-10-14, reading the tree at `2f46d1d` with this plan uncommitted; the plan went in through `--from` with this section and the evidence log cut (`sed '/^## 9\./,/^## Fol/{/^## Fol/!d}'`). Footer: `gpt-6-astra`, effort max (level C's first round), context 872000, repo mode (output cap 4000), 486.9 s, tokens in 1,461,254 (cached 1,356,672), out 13,744 (reasoning 9,617), 29 commands, output 340.1 KB (largest 49 KB), usage 5h 66% → 97%, week 43% → 48%. Five findings, each checked and accepted.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Controllers move before their collision index exists: their casts precede `physics_step`, queries find nothing before the first step (`query.rs:9`), so a controller starting on a floor or beside a wall moves unchecked, and nothing corrects a kinematic body's overlap | `crates/tungsten-core/src/physics/query.rs:1-12`, `:280-281` (no `PhysicsBuffers`, no index); `crates/tungsten-core/src/physics/components.rs:94-101` | Accepted | §2.2 step 2: a controller's first step writes a zero `Velocity`, so the physics step after it indexes what it stands among; §2.2's limits and step 4's known issue add that a body spawned since the last step is unseen for one step; step 2's tests (a) and (f) start in a fresh world against a floor and a wall |
| 2 | [HIGH] A failed harness test can disappear while the implementation stays releasable: the release needed step 1 only, and restoring step 2's file would drop the failing test before `just check` | §7 as written; `justfile:29-30` | Accepted | §7: the release needs steps 1 and 2; a fault a harness test finds in step 1's code is fixed within step 2, whose files now include the kit's modules with their surface unchanged (`just api-check` in its done-when); a step 2 that still fails ends the run |
| 3 | [MEDIUM] Invulnerability depends on how fixed steps group into frames: a window starts in a step but counts down by the frame's game delta in `PostUpdate` (`app.rs:1020`, `:1039`), so two hits a step apart land or not by the frame rate | `crates/tungsten/src/app.rs:1022-1041` (`enter_fixed_step`, `leave_fixed_steps`, then `game_delta`); `D-137`'s clause (1) | Accepted | §2.1, §2.3: `health_tick` counts windows by the step in `fixed_update`, first after the snapshot; flashes count down in `PostUpdate` under a new `HIT_FLASH_TICK`, at full strength in their first frame; step 1's snapshot count is 6 constants; step 2's test (g) replays one hit sequence at frame dts of 1/30, 1/60 and 1/144 s |
| 4 | [MEDIUM] Hit flashes overwrite the original material's uniforms for good: the block the flash writes or inserts stays, and the renderer prefers it to the restored material's authored defaults (`sprite.rs:1024`) | `crates/tungsten-render/src/sprite.rs:1024-1027` (the batch's override block, else the material's defaults) | Accepted | §2.3: the flash keeps and restores the sprite's material and override block, removing a block it inserted, and owns both while it runs; step 2's test (h) covers a material with a block of its own and a sprite without one; step 4's known issue |
| 5 | [MEDIUM] The migration silently removes the black hole's pull on the player: `black_hole_force_system` moves dynamic bodies only, which included the player | `examples/01_platformer/src/systems.rs:858-898`; the blast's `push_bodies` filters the same way (`examples/01_platformer/src/fireball.rs:397-425`) | Accepted | §2.2: `push(velocity)` joins the controller and `velocity()` leaves it, the blast its only user; §2.7: the blast and the black hole reach a controlled player through `push`; step 1's unit tests and step 2's test (e) cover it |

Round 2: not run. Round 1 changed done-when checks in steps 1 and 2 and the stop conditions, which calls for a second round at xhigh, but it left the 5-hour Codex window at 97%: `account/rateLimits/read` from `codex app-server` (no model call) read 97% used, resetting at 16:22 local, 2 h 20 min after round 1 ended. A `--repo` round costs 16–35 points of that window (M48–M51), so it could not finish, and the plan was not reviewed in its place. Round 1's changes thus get no second plan review; step 1's API critique reads the surface they changed (`push`, `HIT_FLASH_TICK`), and the owner reviews the rest on their own time (`D-150`).

API critique (step 1): `critique --raw --repo --force` on the `api/` and `templates/` diff (`git --no-pager diff HEAD -- api/ templates/` plus untracked files there; 23,616 characters, under the soft limit), on 2026-10-14 at 15:07 local. It was queued for the 5-hour window's reset at 16:22, since the window read 97% at 14:39 and an API critique takes about 20 points of it; the owner reported a weekly reset at 15:07, the window then read 0%, and it ran at once, after steps 2–4. Footer: `gpt-6-astra`, effort xhigh, context 872000, repo mode (output cap 4000), 194.8 s, tokens in 580,255 (cached 501,248), out 3,693 (reasoning 1,676), 30 commands, output 248.4 KB (largest 56.8 KB), usage 5h 0% → 15%, week 0% → 2%. Verdict: no concerns ("the additions match their implementations and reveal no substantive compatibility or API-design concern"; it checked exports, signatures, generic bounds, constructors, extensibility markers and plugin ordering against the source and `D-158`). No `C.<n>` rows, and step 1 does not change.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-14 | Passed; API critique below (§9) | `cargo test -p tungsten-kit --locked` exit 0: 154 unit tests, 14 doctests (one per new module: `health::Health`, `health::HitFlash`, `path::PathFollower`, `platformer::PlatformerController`, `state_machine::StateMachine`, `top_down::TopDownMover`, `trigger::TriggerZone`). Clippy `--all-targets -D warnings` and `cargo fmt --all -- --check` exit 0. Doc fences: platformer 2, top_down 2, health 4, trigger 2, path 2, state_machine 2. `git diff --quiet HEAD -- crates/tungsten-kit/Cargo.toml` exit 0. After `just api`: `api/tungsten-kit.txt` alone changed; removed lines 0; types 15; constants 6; `damage` 1. One addition beyond §2.2's sketch: `press_jump` also holds the jump until `set_jump_held(false)`, so a game that never releases jumps at full height | `crates/tungsten-kit/src/{lib,platformer,top_down,slide,health,trigger,path,state_machine}.rs`, `crates/tungsten-kit/src/tests/`, `api/tungsten-kit.txt` |
| 2 | 2026-10-14 | Passed | `cargo test -p tungsten --locked kit_tests` exit 0, 26 passed (12 new; the resolved lines `fixed_update: physics_prev_snapshot, health_tick, phases, path_follow, platformer_controller, top_down_mover, physics_step, zone_entered` and `post_update: …, spawner_tick, hit_flash_tick`); `rg -c '^#\[test\]'` 26. `cargo test -p tungsten-kit --locked` exit 0 (168 passed) and `just api-check` exit 0 ("Public API snapshots current: 4 crates"); no kit file changed in this step. `just physics-release` exit 0: containment 7 passed, determinism 1, tunneling 7, with `pinned_step_state_is_unchanged` and `physics_step_is_bit_identical_across_runs` asserting `0xaee2_72e0_1ffc_4e4c` and `0x088e_c07a_73c1_b168`. `git diff --quiet HEAD -- crates/tungsten-core crates/tungsten-render examples/02_bench crates/tungsten/Cargo.toml` exit 0; `crates/tungsten/src` changed: `tests/kit.rs` alone; no untracked file in those paths | `crates/tungsten/src/tests/kit.rs` |
| 3 | 2026-10-14 | Skipped: stop condition (§7, the route test) | With the player on the controller, `authored_routes_and_recovery_shelves_traverse_with_real_physics` (Space held while rising) printed two lines: `summit_steps: gate -> gate_step: replace its LAUNCHES row with: ("gate", "gate_step", 118.85, 119.75, true),` and `summit_steps: gate_step -> gate_crest is not traversable`. Cause, from a traced launch and the map: the slab over `gate_step` spans x 7424–7744 with its underside at y 983, 41 px above `gate_crest`'s top (y 1024) at their shared edge (x 7744), and the player is 56 px tall, so from every start clear of the crest the jump stops at the slab (head at 983) with its feet 15 px below the crest's top; the row's start, 121.35 tiles, puts the player inside the crest's tile column, where the dynamic solver pushed it out and the controller, finding the step tile's top face masked under the crest, sinks into it. Ten tests failed in the first run, each where it read the old body, before the route test was reworked. Files restored from their copies, each confirmed with `cmp`; `git diff --stat HEAD -- examples/01_platformer` empty. The work in progress was kept outside the tree | — |
| 4 | 2026-10-14 | Passed | `D-158` written (next free ID, checked); heading 1, `D-155`'s marker 1, index row 1, `D-155`'s row amended 1. `DESIGN.md` M52 section 1; LLM index kit row 1 (trimmed to its 12,288 B budget: 12,285 B); known issue 1, "W13c and W13d" no match; ledger row 1; backlog row 1; criteria Q22 open-section no match, answered 1; `roadmap.json` `q22` → `default`, the `w13cd` title without "spawner". `just ctx` and `just repo-check` exit 0. The records say step 3 stopped and why | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/{w13-kit,w04-api-freeze,backlog-1.x,criteria}.md`, `docs/plans/1.0/roadmap.json` |
| 5 | 2026-10-14 | Closed; release checks in the session report | Workflow §5: (1) evidence rows 1–4, step 3 skipped with its reason; (2) gates by change ran in steps 1 and 2, and step 3's (example 01's tests, `just smoke`) run in the release chain on the restored example; a dry `just check` before the cut exit 0; (3) invariants: both hashes in step 2, `api/` in step 1, no core, render, umbrella runtime, template or example file changed; (4) `D-158` in step 4; (5) the ledger's addition row, rustdoc under `deny(missing_docs)`, the snapshot; (6) n/a; (7) the LLM index's kit row, `just ctx`; (8) `DESIGN.md`'s M52 section; (9) known issues, backlog, `w13-kit.md`, criteria, `roadmap.json`, the register row (0.72, cut), the README's owned fields from `roadmap.py summary`; (10) one `CHANGELOG.md` `[Unreleased]` entry, `status: done`, archived before `just release-cut 0.72.0` | `CHANGELOG.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, this plan |

## Follow-ups

- Slopes, one-way platforms and ladders: the backlog row step 4 writes (Q1).
- A controller that pushes dynamic bodies, or a body-kind query filter: W6b's layers and masks first, a push option when a game asks (Q2).
- Scene components for the kit's controllers and helpers, when a level file needs one (Q10).
- `damage_flash` for a game outside this repository: the template or W9b's guide (Q5).
- Sensor exit events, when a zone needs leaving (`D-154`).
- Example 01's health, hit flash, hazards and moving platforms, and the template's player: W12b (Q11, Q12).
- State ownership of spawned entities: the acceptance game's first gap (Q12).
