# M30 — Parallax + Screen-Shake + Squash/Stretch

- **status:** done
- **goal:** Ship three game-feel primitives — per-layer parallax scroll, trauma-enveloped camera shake driven by `ShakeEvent`, and one-shot squash/stretch on `Transform.scale` — and demo all three in `examples/01_platformer/` and `examples/04_shader_playground/`.
- **non-goals:** see [Non-goals](#non-goals)
- **files to touch:** see [Files to touch](#files-to-touch)
- **ordered steps:** see [Ordered steps](#ordered-steps)
- **done-when:** see [Done-when checks](#done-when-checks)

## Context digest

Scope comes from the `## M30` section of [phase4.md](phase4.md). Several names there are marked TBD and three of its
assumptions do not survive contact with shipped M24–M29 code; this plan resolves them against source.

What already exists:

- `CameraController` ([core/camera.rs:73](../../crates/tungsten-core/src/camera.rs#L73)) already carries a **sine shake**
  (`shake_amplitude`, `shake_frequency_hz`, `shake_phase`) plus private `last_output_position` / `last_shake_offset`
  bookkeeping, consumed by `camera_update_system` ([tungsten/camera.rs:38](../../crates/tungsten/src/camera.rs#L38)).
  M30 adds a trauma **envelope** over that carrier rather than a second shake model.
- `Tween` (M24) has `ScaleX`/`ScaleY` channels, and `D-055` allows **one `Tween` per entity**.
- The platformer already ships the M26 damage flash: `damage_flash_on_ball_hit`
  ([systems.rs:231](../../examples/01_platformer/src/systems.rs#L231)) drives the `damage_flash` material through
  `UniformOverrideBlock` and **already consumes the player's single `Tween` slot**. The `damage_flash` shader and
  material live in the root [assets/manifest.json](../../assets/manifest.json); nothing new is needed for the flash.
- `App::new` registers built-in events inline (`CollisionEvent`, `TweenComplete`, …) via `register_event_inner`
  ([app.rs:117](../../crates/tungsten/src/app.rs#L117)); `App::register_event` is the public path.
- Frame order ([app.rs:1365](../../crates/tungsten/src/app.rs#L1365)): user systems → particles → tweens → flush commands
  → flush events → hot reload → extract → render. `camera_update_system` is **example-registered**, last in the
  platformer's `RUNTIME_SYSTEM_ORDER`.
- Example 01 uses a **custom** `extract_sprites`; example 04 uses `extract_sprites_default`. Its parallax art is already
  in the manifest (`ex10_sky`, `ex10_mountain_*`, `ex10_vines_*`) but currently baked into the `level.tmj` tile layers.

Governing decisions: `D-007`/`D-016`/`D-018` (core holds no `wgpu`, extract hands plain data to render), `D-042`
(explicit render components), `D-039`/`D-040` (command + event flush), `D-054`/`D-055`/`D-056` (closed easing enum, one
tween per entity, `TweenComplete` routing), `D-058` (materials, `UniformOverrideBlock`).

## Gaps in the phase-doc assumptions

Three assumptions in [phase4.md](phase4.md) are wrong against current code. Resolutions are load-bearing for the steps
below and are what the new decision (`D-073`) must record.

### 1. Per-bucket camera matrices do not fit the existing seam

The phase doc calls for "one camera matrix per `depth_bucket` through the existing `ExtractSpritesFn` seam". That seam is
`Box<dyn Fn(&World) -> Vec<SpriteBatch>>` ([app.rs:58](../../crates/tungsten/src/app.rs#L58)); the view-projection is built
**separately** in `stage_render` from `CameraState` ([app.rs:837](../../crates/tungsten/src/app.rs#L837)) and uploaded once
per frame into a single camera UBO + bind group (`SpritePipeline::update_camera`,
[sprite.rs:802](../../crates/tungsten-render/src/sprite.rs#L802)). Carrying a matrix per bucket means a per-batch VP on
`SpriteBatch`, dynamic-offset camera binding in the render crate, and a changed `render_frame_full` signature.

**Resolution: CPU-side parallax, no render-crate change.** Parallax is a pure position remap applied at extract time:

```
parallax_world_position(authored, scroll_factor, camera_position)
    = authored + camera_position * (Vec2::ONE - scroll_factor)
```

`scroll_factor == 1` is world-locked (identity), `0` is screen-locked. One VP still draws every layer correctly. This
keeps `D-018` intact and leaves `crates/tungsten-render/` untouched.

**Consequence: `depth_bucket` is dropped.** It only existed to group draws per matrix. Draw order stays
`Sprite.z_order`, so `ParallaxLayer` carries `scroll_factor` alone; keeping both would create two sources of ordering
truth.

### 2. `Tween` cannot express a one-shot squash, and the slot is already taken

- `TweenRepeat::Once` runs `from → to` and stops — no return leg. `PingPong` returns but **never completes** (it flips
  direction forever and emits no `TweenComplete`). `Times(n)` resets `elapsed` to `0.0`, restarting from `from` rather
  than reversing ([tweens.rs:64](../../crates/tungsten/src/tweens.rs#L64)). No `Easing` variant is symmetric; `BackOut`
  and `BounceOut` overshoot but still land on `to`.
- `D-055` allows one `Tween` per entity, and the platformer's player already spends it on the damage flash. The
  acceptance line needs **squash on land and flash on hit on the same entity**, so a squash `Tween` would clobber the
  flash (its own comment says "overwrite any active tween").

**Resolution: squash/stretch does not use `Tween`.** `squash_stretch_tick_system` owns a symmetric envelope and writes
`Transform.scale` directly from a runtime component. This sidesteps `D-055` entirely and leaves the flash working.

### 3. `shake_noise_seed` is redundant

`shake_phase` + `shake_frequency_hz` already give a deterministic oscillator. Trauma is an envelope over it, so no noise
source is needed and `core/rng.rs` stays out of the camera path. `shake_noise_seed` is dropped.

Confirmed sound as assumed: `EventQueue` two-window semantics (`iter()` covers previous + current, so a system reads an
event regardless of whether it ran before or after the sender in the same frame), `App::register_event`, and the M26
material uniform path for the flash — no changes needed to any of them.

## Verified API shape

### `crates/tungsten-core/src/components.rs`

```rust
/// Parallax scroll factor per axis. `1.0` = world-locked, `0.0` = screen-locked.
/// Applied at extract time; never mutates `Transform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParallaxLayer {
    pub scroll_factor: Vec2,
}

impl ParallaxLayer {
    #[must_use] pub fn new(scroll_factor: Vec2) -> Self;
    /// Uniform factor on both axes.
    #[must_use] pub fn uniform(factor: f32) -> Self;
}

/// Which gameplay trigger arms this entity's squash. Closed enum per `D-054` style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquashTrigger { OnLand, OnHit, OnPickup, Manual }

/// Squash/stretch config. `amount` is the peak scale multiplier
/// (e.g. `(1.3, 0.7)` = widen and flatten).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteSquashStretch {
    pub on: SquashTrigger,
    pub amount: Vec2,
    pub duration: f32,
    pub easing: Easing,
}

/// Runtime envelope state; inserted/updated by `squash_stretch_trigger_system`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquashStretchState {
    pub elapsed: f32,
    pub base_scale: Vec2,
}
```

`base_scale` is captured at trigger time so the envelope restores the authored scale and a re-trigger mid-flight cannot
compound. Re-export all four from `crates/tungsten-core/src/lib.rs` (the existing `pub use components::{…}` block at
line 33).

### `crates/tungsten-core/src/camera.rs`

`CameraController` gains three public fields; `CameraState` stays output-only.

```rust
pub shake_trauma: f32,      // 0.0..=1.0, clamped on write
pub shake_decay: f32,       // trauma units per second, default 1.0
pub shake_max_offset: Vec2, // pixels at trauma == 1.0
```

`Default` sets `shake_trauma: 0.0`, `shake_decay: 1.0`, `shake_max_offset: Vec2::ZERO`, so existing callers are
unaffected. Plus:

```rust
impl CameraController {
    /// Clamped trauma add; saturates at 1.0 rather than wrapping.
    pub fn add_trauma(&mut self, amount: f32);
    /// Shake offset for the current phase: sine carrier, trauma-squared envelope.
    #[must_use] pub fn shake_offset(&self) -> Vec2;
}
```

`shake_offset` is where the existing sine block in `camera_update_system` moves to, extended to:

```
effective = shake_amplitude + shake_max_offset * shake_trauma * shake_trauma
offset    = (effective.x * sin(phase), effective.y * sin(phase + FRAC_PI_2))
```

Trauma-squared is the standard trauma curve and makes the tail fall off fast. With `shake_trauma == 0.0` the expression
reduces to today's output exactly, which keeps the M29 render fixtures byte-identical.

### `crates/tungsten-core/src/ecs/event_queue.rs` usage (new events, defined in core)

```rust
/// Additive camera trauma request. Handled by `shake_tick_system`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShakeEvent { pub trauma_add: f32 }

/// Arms an entity's squash envelope. Sent by gameplay, not by core.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquashEvent { pub entity: Entity, pub trigger: SquashTrigger }
```

`SquashEvent` is the addition the phase doc omits: `SquashTrigger::OnLand` cannot be detected in core without core
knowing about gameplay, so gameplay sends the event and the component's `on` field filters it. Both are registered in
`App::new` alongside `CollisionEvent` via `register_event_inner`, and re-exported from `tungsten_core`.

### New systems (`crates/tungsten/src/game_feel.rs`, new module)

```rust
/// Drains `EventQueue<ShakeEvent>` into controller trauma, then decays it by dt.
/// Must run before `camera_update_system` in the same frame.
pub fn shake_tick_system(world: &mut World);

/// Matches `EventQueue<SquashEvent>` against `SpriteSquashStretch.on`;
/// inserts `SquashStretchState` with the entity's current `Transform.scale`
/// as `base_scale`. Uses `CommandBuffer` for the insert (`D-039`).
pub fn squash_stretch_trigger_system(world: &mut World);

/// Advances every `SquashStretchState`, writes `Transform.scale`, and removes
/// finished states through `CommandBuffer` (`D-039`), restoring `base_scale`.
pub fn squash_stretch_tick_system(world: &mut World);
```

Envelope: `t = elapsed / duration`, `k = easing.apply(t)`, `env = sin(k * PI)` (0 → 1 → 0), and
`scale = base_scale * (Vec2::ONE + (amount - Vec2::ONE) * env)`. Exact at both ends, so the authored scale is restored
without a separate cleanup write.

Both squash systems follow the `tween_tick_system` pattern: buffer structural work, never mutate the archetype
mid-iteration. All three are `pub use`-exported from `crates/tungsten/src/lib.rs` and registered by examples, matching
how `camera_update_system` is wired today (the engine cannot know where in a game's order they belong).

### `crates/tungsten/src/sprite_extract.rs`

`extract_sprites_default` reads `CameraState` once, and for each entity carrying `ParallaxLayer` remaps the instance
position. The `BatchKey` is **unchanged** — parallax alters positions, not batch identity. Put the remap in a small
`#[must_use] pub fn parallax_world_position(...)` in `core/components.rs` so the platformer's custom extract calls the
same function instead of duplicating the formula.

### `crates/tungsten/src/camera.rs`

`camera_update_system` replaces its inline sine block with `controller.shake_offset()`. `record_output_position` /
`resolve_base_position` bookkeeping is untouched, so trauma shake cannot accumulate into the base position.

## Files to touch

| File | Change |
| --- | --- |
| `crates/tungsten-core/src/components.rs` | `ParallaxLayer`, `SquashTrigger`, `SpriteSquashStretch`, `SquashStretchState`, `parallax_world_position` |
| `crates/tungsten-core/src/camera.rs` | trauma fields, `add_trauma`, `shake_offset` |
| `crates/tungsten-core/src/ecs/event_queue.rs` | `ShakeEvent`, `SquashEvent` |
| `crates/tungsten-core/src/lib.rs` | re-exports |
| `crates/tungsten-core/src/tests/components.rs` | parallax + envelope unit tests |
| `crates/tungsten-core/src/tests/camera.rs` | trauma/offset unit tests |
| `crates/tungsten/src/game_feel.rs` | **new** — the three systems |
| `crates/tungsten/src/tests/game_feel.rs` | **new** — system unit tests |
| `crates/tungsten/src/camera.rs` | call `shake_offset()` |
| `crates/tungsten/src/tests/camera.rs` | **new** — `camera_update_system` has no unit tests today |
| `crates/tungsten/src/sprite_extract.rs` | parallax remap in default extract |
| `crates/tungsten/src/tests/sprite_extract.rs` | parallax cases |
| `crates/tungsten/src/app.rs` | register `ShakeEvent` + `SquashEvent` |
| `crates/tungsten/src/lib.rs` | `mod game_feel;` + re-exports |
| `examples/01_platformer/src/{setup,state,systems,extract}.rs` | backdrop spawn, components, triggers, parallax in custom extract |
| `examples/01_platformer/assets/tilemaps/level.tmj` | clear the `background` layer (see below) |
| `examples/04_shader_playground/src/main.rs` | parallax quads, camera pan, shake + squash on impact |
| `scripts/smoke-examples.sh` | M30 fixture rows |
| `scripts/test-smoke-examples.sh` | stub env echo + `expected-runs` |
| `docs/LLM_INDEX.md`, `docs/DECISION_INDEX.md`, `DECISIONS.md` | `D-073` row + entry (same change, test-enforced) |
| `CHANGELOG.md` | `[Unreleased]` entry |

Deliberately **not** touched: `crates/tungsten-render/` (the CPU-parallax resolution removes any need) and
`crates/tungsten/src/tilemap_extract.rs`.

### The platformer backdrop problem

`level.tmj`'s `background` layer is **100% filled** (2688/2688) with opaque sky tiles (`sky`, `sky_1`, `sky_2`), while
`decorations` holds the mountains/clouds/vines and `foreground` the ground. `extract_tilemaps` emits all render layers in
one call, so parallax sprite entities appended after it draw over the ground, and entities emitted before it are hidden
by the opaque sky.

**Recommended:** zero the `background` layer's `data` array and let the parallax sky layer supply the sky. The
platformer's custom extract then emits parallax batches **first**, giving sky → mid-hills → near-trees → tilemap
(decorations, foreground) → particles → player. `sky*.png` stay referenced through their existing manifest entries, so
`just repo-check` asset coverage is unaffected.

**Alternative if the result reads badly:** split `extract_tilemaps` into per-layer groups so parallax can be interleaved
between `background` and `decorations`. That adds `tilemap_extract.rs` to the touch list; take it only if step 8 looks
wrong on screen.

## Ordered steps

1. **Core components.** Add the four types plus `parallax_world_position` to `core/components.rs`; re-export from
   `core/lib.rs`. Unit tests for the remap identity (`scroll_factor == 1`), screen-lock (`== 0`), per-axis asymmetry, and
   envelope endpoints.
2. **Core camera trauma.** Add the three fields (`Default` = inert), `add_trauma` (clamped to `0.0..=1.0`), and
   `shake_offset`. Move the sine expression out of `camera_update_system` into `shake_offset`. Unit tests: zero trauma
   reproduces the pre-M30 offset byte-for-byte, trauma-squared falloff, saturating add, `shake_max_offset` respected.
3. **Core events.** Define `ShakeEvent` and `SquashEvent`; re-export them.
4. **Umbrella systems.** Add `crates/tungsten/src/game_feel.rs` with the three systems, `mod` + `pub use` in
   `tungsten/src/lib.rs`.
5. **Event registration.** Register both events in `App::new` next to `CollisionEvent`; assert in the `app.rs` tests that
   `EventQueue<ShakeEvent>` and `EventQueue<SquashEvent>` exist after `App::new`.
6. **Camera wiring.** `camera_update_system` calls `controller.shake_offset()`. Add
   `crates/tungsten/src/tests/camera.rs` (new): follow/dead-zone behaviour unchanged, shake offset applied, base position
   never accumulates shake across frames.
7. **Default extract parallax.** Apply the remap in `extract_sprites_default`; assert `BatchKey` grouping is unchanged
   and that an entity without `ParallaxLayer` produces byte-identical instances to pre-M30.
8. **Platformer parallax.** Zero `level.tmj`'s `background` layer. Spawn three backdrop layers in `setup.rs` from
   existing sprite IDs — sky (`ex10_sky`, `scroll_factor 0.05`, `z_order -300`), mid-hills (`ex10_mountain_big_*`, `0.35`,
   `-200`), near-trees (`ex10_vines_big_*`, `0.6`, `-100`) — tiled horizontally across the map width, and emit them first
   in the custom `extract_sprites` using `parallax_world_position`.
9. **Platformer squash + shake.** Give the player `SpriteSquashStretch { on: OnLand, amount: (1.25, 0.75), duration:
   0.18, easing: QuadOut }`. Extend `ground_detection` to send `SquashEvent` on the rising edge of `grounded` (add a
   `was_grounded` field to `Player` so a resting player does not re-fire every frame). Extend
   `damage_flash_on_ball_hit` to also send `ShakeEvent { trauma_add: 0.5 }` — it keeps its `Tween` for the flash, and the
   squash path no longer competes for it. Register `shake_tick_system`, `squash_stretch_trigger_system` and
   `squash_stretch_tick_system` in `RUNTIME_SYSTEM_ORDER`, with `shake_tick_system` immediately before
   `camera_update_system`.
10. **Acceptance demo.** Run `cargo run -p example-01-platformer` (leave `TUNGSTEN_LIGHTING_FIXTURE` unset — the flash
    material is only bound on the unlit path). Walk horizontally and confirm three layers scroll at visibly different
    rates; land and confirm squash; take a ball hit and confirm shake **and** red flash fire together. Capture the gif for
    the phase-doc acceptance line.
11. **Shader playground demo.** Uses `extract_sprites_default`, so parallax needs no example-side extract work. Add three
    `ex04_quad` layers at distinct tints, `z_order` and `scroll_factor`; register `camera_update_system` with
    `CameraMode::Follow` on one bouncer so the camera pans and the layers separate. Feed the existing `contacts` vector in
    `pair_collision_system` and the wall bounces in `bounce_system` into `ShakeEvent` + `SquashEvent`, giving each bouncer
    `SpriteSquashStretch { on: OnHit, .. }`. Add a `TUNGSTEN_GAME_FEEL_FIXTURE=on` env gate in the same style as
    `TUNGSTEN_BLOOM_FIXTURE`. **No new assets:** `ex04_quad` plus tint, `Transform.scale` and `z_order` cover all three
    layers, so `examples/04_shader_playground/assets/manifest.json` is unchanged.
12. **Smoke rows.** Add an M30 section to `scripts/smoke-examples.sh` (one `example-04-shader-playground` row with
    `TUNGSTEN_GAME_FEEL_FIXTURE=on`, one `example-01-platformer` row). Update `scripts/test-smoke-examples.sh`: add
    `feel=${TUNGSTEN_GAME_FEEL_FIXTURE:-}` to the stub's `run` echo, add the two `expected-runs` lines, and add the
    `"Game-feel passed: 2/2"` expectation.
13. **Docs.** Write `D-073` in `DECISIONS.md` **and** its `docs/DECISION_INDEX.md` row in the same change (the
    `decision_index` test enforces the pair). Add an M30 row to `docs/LLM_INDEX.md` under Runtime and ECS. Add the
    `CHANGELOG.md` `[Unreleased]` entry. Set this plan's `status` to `done` and move it to `docs/plans/archive/`.
14. **`just check`** — fmt-check, `clippy -D warnings`, workspace tests.

## Tests

### Unit tests

| Target | Cases |
| --- | --- |
| `core/tests/components.rs` | `parallax_world_position`: identity at `1.0`, screen-lock at `0.0`, per-axis factors, negative camera positions; squash envelope: `0.0` and `1.0` endpoints return `base_scale`, midpoint hits `amount`, `duration <= 0.0` does not divide by zero |
| `core/tests/camera.rs` | `add_trauma` clamps and saturates; `shake_offset` is zero with zero trauma and zero amplitude; trauma-squared falloff; `shake_max_offset` scaling per axis; pre-M30 sine output reproduced when `shake_trauma == 0.0` |
| `tungsten/tests/game_feel.rs` | `shake_tick_system` accumulates several `ShakeEvent`s in one frame, decays by `shake_decay * dt`, floors at `0.0`, and no-ops with no controller; `squash_stretch_trigger_system` matches only the component's `on` trigger, ignores a `SquashEvent` for a missing entity, and captures `base_scale` from the live `Transform`; `squash_stretch_tick_system` restores `base_scale` exactly on completion, removes state via `CommandBuffer`, and a re-trigger mid-flight does not compound scale |
| `tungsten/tests/camera.rs` (new) | shake offset reaches `CameraState.position`; `resolve_base_position` still strips it so shake does not accumulate across frames; follow + bounds behaviour unchanged |
| `tungsten/tests/sprite_extract.rs` | a `ParallaxLayer` entity's instance position is remapped; an entity without one is byte-identical to pre-M30; batch grouping and `z_norm` unchanged by parallax |
| `tungsten/tests/app.rs` | both new `EventQueue`s exist after `App::new` |

### Layered checks

- **Layer 1** (`crates/tungsten-core/tests/manifests.rs`, part of `cargo test`) — applies: `level.tmj` changes in step 8.
  No manifest schema change, so this is a regression guard rather than new coverage. The `decision_index` test also gates
  step 13.
- **Layer 2** (`just smoke`) — applies: both examples change wiring, and step 12 adds the fixture rows. This is the only
  check that runs the new systems against a real GPU frame.
- **`just script-test`** — applies: step 12 edits `scripts/smoke-examples.sh`, and `scripts/test-check-repo.py` /
  `test-smoke-examples.sh` gate it.
- **Visual** — not applicable. The pixel test is `example-02-sprite-stress`, which M30 does not touch.

## New decision needed

`D-073` (next free ID — `DECISIONS.md` ends at `D-072`). Not written now; step 13 writes it. It must record all four
deviations from [phase4.md](phase4.md) and why:

- Parallax is a **CPU position remap at extract time**, not a per-`depth_bucket` camera matrix — the single-VP upload in
  `stage_render` and the single camera UBO in `SpritePipeline` make per-bucket matrices a render-crate change that buys
  nothing. Narrows `D-018`, `D-042`.
- **`depth_bucket` dropped**; `Sprite.z_order` remains the sole ordering authority.
- **Squash/stretch does not use `Tween`.** `TweenRepeat` has no out-and-back one-shot, and `D-055`'s one-tween-per-entity
  rule is already spent on the M26 damage flash for the very entity the acceptance line needs to both squash and flash.
  Narrows `D-055`.
- Shake is a **trauma envelope over the existing sine carrier**, additive with `shake_amplitude` and inert at
  `shake_trauma == 0.0`; `shake_noise_seed` dropped as redundant.

## Non-goals

- Per-`depth_bucket` camera matrices, per-batch view-projections, or any change under `crates/tungsten-render/`.
- Relaxing `D-055` to allow multiple tweens per entity.
- Manifest-authored parallax or squash (both are code-side components this milestone; no schema change).
- Parallax for tilemap layers, and any change to `tilemap_extract.rs` unless step 8's alternative is taken.
- New example art. Both demos reuse existing sprites.
- M31 (mesh particles, screen transitions) and every other Phase 4 milestone.
- Editing `docs/plans/phase4.md`.
- Writing `D-073` now.

## Done-when checks

All verified; the on-screen acceptance in step 10 was confirmed by the owner. A QA pass after it fixed seven
findings: the trigger system restarts an in-flight envelope in place rather than buffering an insert the tick's
removal would undo, the tick system sweeps states orphaned by config removal and is inert on a frozen frame, the
playground maps snapshot indices correctly and screen-locks its bloom source, the platformer shakes horizontally
only, and both systems document their ordering constraints.

- `crates/tungsten-core` exports `ParallaxLayer`, `SquashTrigger`, `SpriteSquashStretch`, `SquashStretchState`,
  `ShakeEvent`, `SquashEvent`; `CameraController` carries `shake_trauma` / `shake_decay` / `shake_max_offset` with an
  inert `Default`.
- `crates/tungsten` exports `shake_tick_system`, `squash_stretch_trigger_system`, `squash_stretch_tick_system`, and
  `App::new` registers both new event queues.
- A sprite with no `ParallaxLayer` extracts byte-identically to pre-M30, and `CameraController::default()` produces the
  pre-M30 shake offset.
- `cargo run -p example-01-platformer` shows three parallax layers at different rates, squash on landing, and
  simultaneous shake + damage flash on a ball hit.
- `cargo run -p example-04-shader-playground` shows parallax under a panning camera plus shake and squash on impact.
- `just check` passes (fmt-check, `clippy -D warnings`, workspace tests).
- `just smoke` passes, including the new M30 rows; `just script-test` passes.
- `just repo-check` and `just ctx` pass.
- `D-073` exists in `DECISIONS.md` with its `docs/DECISION_INDEX.md` row; `docs/LLM_INDEX.md` has an M30 row;
  `CHANGELOG.md` has an `[Unreleased]` entry.
- This plan is `status: done` and moved to `docs/plans/archive/`.
