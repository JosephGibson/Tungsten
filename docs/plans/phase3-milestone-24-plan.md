---
status: draft
goal: Ship a core-owned tween system that animates `Transform`/`Sprite` channels with built-in easings, fires `TweenComplete` through `EventQueue<T>`, self-removes via `CommandBuffer::remove_component`, is authorable from `scene.json`, and demonstrates a state-transition fade in `examples/03_scene_state/`.
non-goals: runtime tween builders exposed as a fluent DSL, custom easing curves outside the built-in enum, dependency tree animation, skeletal animation, spline/path tweens, GPU-side interpolation, easing author UI, tween scripting, physics-coupled tweens, serialized tween state across runs, Phase-4 timeline/sequencer.
files-to-touch:
  - crates/tungsten-core/src/components.rs
  - crates/tungsten-core/src/tween.rs (new)
  - crates/tungsten-core/src/lib.rs
  - crates/tungsten-core/src/assets/scene.rs
  - crates/tungsten-core/src/tests/tween.rs (new)
  - crates/tungsten-core/src/tests/assets/scene.rs
  - crates/tungsten-core/benches/tween_tick.rs (new)
  - crates/tungsten/src/tweens.rs (new)
  - crates/tungsten/src/app.rs
  - crates/tungsten/src/asset_loader.rs
  - crates/tungsten/src/lib.rs
  - examples/03_scene_state/src/states.rs
  - examples/03_scene_state/src/main.rs
  - examples/03_scene_state/assets/scene.json
  - docs/LLM_INDEX.md
  - docs/plans/Phase3.md
  - DECISIONS.md
  - docs/DECISION_INDEX.md
  - CHANGELOG.md
  - Cargo.toml
---

# Phase 3 M24 — Tween System

## Context Digest

- Scope source: [docs/plans/Phase3.md](Phase3.md) M24 row — `Tween { target, easing, duration, elapsed }`, built-in easings only, no dependency, emit `TweenComplete` on completion, remove via `CommandBuffer`, support `scene.json` authoring; done-when: example animates UI/state transitions and reacts to `TweenComplete`.
- Dependencies satisfied: M13 `CommandBuffer` (`tungsten_core::ecs::command_buffer::CommandBuffer` — exposes `remove_component::<T>(Entity)`), M14 `EventQueue<T>` (`App::register_event::<T>()` at [crates/tungsten/src/app.rs:316](../../crates/tungsten/src/app.rs#L316)), M15 `Transform`/`Sprite`/`Visibility` at [crates/tungsten-core/src/components.rs](../../crates/tungsten-core/src/components.rs), M20 `StateStack`+`SceneEntity` + `scene.json` at [crates/tungsten/src/state.rs](../../crates/tungsten/src/state.rs) and [crates/tungsten-core/src/assets/scene.rs](../../crates/tungsten-core/src/assets/scene.rs).
- Seam: `Tween`, `TweenChannel`, `Easing`, `TweenRepeat` components + the easing/curve helpers live in `tungsten-core`; the tick system, event registration, and scene-tween spawn helper live in `tungsten`. No `wgpu` touched.
- Render path: unchanged — tweens only mutate `Transform` and `Sprite.color`; the default M15 sprite extract at [crates/tungsten/src/sprite_extract.rs](../../crates/tungsten/src/sprite_extract.rs) picks up the mutated values on the same frame per the `run systems → particles → tweens → flush commands → flush events → hot reload → extract → render` order already enforced in [crates/tungsten/src/app.rs:712](../../crates/tungsten/src/app.rs#L712).
- Determinism: tween drives off `DeltaTime`; no RNG. Easing is a pure `fn(f32) -> f32`. Scene-authored tweens spawn identically across runs given identical `DeltaTime` sequences.
- Removal: on completion, a `Once` tween enqueues `CommandBuffer::remove_component::<Tween>(entity)` so the component drops at the frame-end flush (D-039); `TweenComplete` is enqueued into `EventQueue<TweenComplete>` so readers see it the next frame (D-040 two-window lifetime). No scope drift into generic `remove_component`: that exists per M13 (D-039) already.
- Hot reload: scene hot reload re-spawns scene entities fresh — no in-flight Arc semantics needed; tween definitions are copy-by-value.
- Example: [examples/03_scene_state/src/states.rs](../../examples/03_scene_state/src/states.rs) is the chosen surface — `on_enter` spawns scene-authored fade-in alpha tweens; a user-initiated transition flips a `FadeOutRequested` flag that spawns a fade-out alpha tween tagged `state_exit`; a system reads `EventQueue<TweenComplete>` for `tag == "state_exit"` and calls `StateStack::request_replace`/`request_pop`.

## Design

### Primitive: `Easing`

- File: `crates/tungsten-core/src/tween.rs`.
- Closed set, no trait object, no boxed fn. Matches on `self`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    Linear,
    QuadIn, QuadOut, QuadInOut,
    CubicIn, CubicOut, CubicInOut,
    QuartIn, QuartOut, QuartInOut,
    SineIn, SineOut, SineInOut,
    ExpoIn, ExpoOut, ExpoInOut,
    BackIn, BackOut, BackInOut,
    BounceIn, BounceOut, BounceInOut,
}

impl Easing {
    #[inline] pub fn apply(self, t: f32) -> f32 { /* closed-form */ }
}
```

- `apply` contract: input `t` is pre-clamped to `[0, 1]` by the caller; output is unconstrained for `Back`/`Elastic`-class curves (intentional overshoot). `Linear` returns `t`. Back constant `s = 1.70158`; Bounce uses the Robert Penner reference constants.
- No dependency (`D-015`: not a platform API, not a well-specified format, not a solved primitive that warrants a crate for ~60 lines of arithmetic).

### Component: `Tween`

- File: `crates/tungsten-core/src/tween.rs`; re-exported from `crates/tungsten-core/src/components.rs` and `crates/tungsten-core/src/lib.rs` alongside `Transform`/`Sprite`.

```rust
pub struct Tween {
    pub channels: Vec<TweenChannel>,
    pub easing: Easing,
    pub duration: f32,   // seconds; must be > 0
    pub elapsed: f32,    // seconds; starts at 0
    pub repeat: TweenRepeat,
    pub direction: TweenDirection, // forward/backward for PingPong state
    pub completed_cycles: u32,
    pub on_complete_tag: Option<String>,
}

pub enum TweenChannel {
    PositionX { from: f32, to: f32 },
    PositionY { from: f32, to: f32 },
    Rotation  { from: f32, to: f32 },
    ScaleX    { from: f32, to: f32 },
    ScaleY    { from: f32, to: f32 },
    ColorR    { from: u8,  to: u8  },
    ColorG    { from: u8,  to: u8  },
    ColorB    { from: u8,  to: u8  },
    ColorA    { from: u8,  to: u8  },
}

pub enum TweenRepeat { Once, Loop, PingPong, Times(u32) }

pub enum TweenDirection { Forward, Backward } // PingPong uses both; others stay Forward.
```

- One `Tween` per entity (archetypal ECS single-component-per-type rule); multi-property animation is expressed by pushing multiple `TweenChannel`s into the same tween. Concurrent tweens on the same entity needing different easings/durations are out of scope — open a follow-up if needed.
- `Tween::new(duration, easing) -> Self` builder + `with_channel(Self, TweenChannel) -> Self` + `with_repeat` + `with_tag` keep example code terse.
- Builders validate `duration.is_finite() && duration > 0.0`; panic in debug, clamp to `f32::EPSILON` in release (matches M23 config posture: `unwrap`/clamp acceptable in a young module).

### Event: `TweenComplete`

```rust
#[derive(Debug, Clone)]
pub struct TweenComplete { pub entity: Entity, pub tag: Option<String> }
```

- Fires once per terminal completion:
  - `Once`: when `elapsed >= duration` for the first time.
  - `Times(n)`: on the `n`-th forward completion.
  - `Loop`: never (by contract — loop is explicitly infinite; document loudly).
  - `PingPong`: never on its own — pair with `Times(n)` via a new `TweenRepeat::PingPongTimes(n)` only if the M20 integration test needs it; otherwise defer.
- Registered with `App::register_event::<TweenComplete>()` from `App::new` (same slot as `ParticleBurstEmitted` at [crates/tungsten/src/app.rs:200-209](../../crates/tungsten/src/app.rs#L200-L209)).

### Frame Order Slot

Insert between `stage_particles` and `stage_flush_commands` in `App::render_frame_*` paths at [crates/tungsten/src/app.rs:712-720](../../crates/tungsten/src/app.rs#L712-L720):

```
user systems
  → stage_particles
  → stage_tweens          // NEW: tween_tick_system
  → stage_flush_commands  // applies Tween removal + despawn
  → stage_flush_events    // TweenComplete visible to readers on the NEXT frame's previous-window
  → stage_hot_reload
  → stage_extract         // sees tween-mutated Transform/Sprite this frame
  → render
```

- Emission order (particles before tweens) means a particle's first-frame `Transform` is authored before the tween has a chance to override it — that's the intended direction for tween-over-particle effects. Document the invariant in the module docstring.

### System: `tween_tick_system(&mut World)`

- File: `crates/tungsten/src/tweens.rs`.
- Query `(Entity, &mut Tween, Option<&mut Transform>, Option<&mut Sprite>)`. For each:
  - `dt = world.get_resource::<DeltaTime>().map(|d| d.dt).unwrap_or(0.0)`.
  - Advance: `signed_dt = if direction == Backward { -dt } else { dt }; tween.elapsed = (tween.elapsed + signed_dt).clamp(0.0, tween.duration)`.
  - `u = (tween.elapsed / tween.duration).clamp(0.0, 1.0)`.
  - `k = tween.easing.apply(u)`.
  - For each `TweenChannel`: compute interpolated value (f32: `lerp(from, to, k)`; u8: `lerp_u8(from, to, k) = (from as f32 + (to as f32 - from as f32) * k).round().clamp(0.0, 255.0) as u8`). Write into the optional `Transform`/`Sprite` guard; channel whose target component is absent is silently skipped (log once at `WARN` per entity via a resource-side set; not gated on first release).
  - Boundary transitions:
    - Forward and `elapsed >= duration`: handle `repeat`.
    - Backward and `elapsed <= 0.0`: handle `repeat`.
  - `Once`: send `TweenComplete`; push `(entity, tween_component_removed)` into a local `Vec<Entity>` for post-loop `cmd.remove_component::<Tween>(entity)`.
  - `Times(n)`: `completed_cycles += 1`; if `== n`, treat as `Once`; else reset `elapsed = 0.0`.
  - `Loop`: reset `elapsed = 0.0`; no event.
  - `PingPong`: flip `direction`; clamp `elapsed` to the appropriate endpoint; no event (unless `PingPongTimes` is added later).
- Event send uses `world.get_resource_mut::<EventQueue<TweenComplete>>().unwrap().send(...)`. Removal + event emission are buffered through `CommandBuffer`/`EventQueue` so neither mutates the active query's archetype.

### Scene JSON Authoring

- Extend `SceneEntry` in [crates/tungsten-core/src/assets/scene.rs](../../crates/tungsten-core/src/assets/scene.rs) with `#[serde(default)] pub tweens: Vec<SceneTween>`.

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SceneTween {
    pub duration: f32,
    #[serde(default)] pub easing: Easing, // Easing::Linear
    #[serde(default)] pub repeat: SceneTweenRepeat, // Once
    #[serde(default)] pub tag: Option<String>,
    pub channels: Vec<SceneTweenChannel>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneTweenChannel {
    PositionX { from: f32, to: f32 },
    // …one variant per TweenChannel
    ColorA    { from: u8,  to: u8  },
}
```

- `SceneTweenRepeat` mirrors `TweenRepeat` but serializes as `"once" | "loop" | "ping_pong" | { "times": n }`. Default `Once`.
- Parse-time validation (in `SceneData::load` path, pushed down to `serde` where possible): reject `duration <= 0.0` or non-finite; reject empty `channels`; unknown enum variants fail the whole scene load (matches M20 posture).
- `spawn_scene` in [crates/tungsten/src/asset_loader.rs:467-502](../../crates/tungsten/src/asset_loader.rs#L467) extended: after `SceneEntity` marker insert, if `entry.tweens.is_empty()` skip; else for each `SceneTween`, convert into a runtime `Tween` via `SceneTween::into_tween()` and `buf.insert_pending(pending, tween)`. Only one tween per entry is honored (archetypal constraint). If `entry.tweens.len() > 1`, log `ERROR` and take the first (do not silently drop; do not fail the entire scene).

### Example Integration — `03_scene_state` Fade Transitions

- [examples/03_scene_state/assets/scene.json](../../examples/03_scene_state/assets/scene.json) already spawns ~24 sprite entries; extend 3–5 representative entities with a fade-in tween:

```json
"tweens": [{
  "duration": 0.4,
  "easing": "cubic_out",
  "repeat": "once",
  "tag": "scene_fade_in",
  "channels": [{ "kind": "color_a", "from": 0, "to": 255 }]
}]
```

- Add a second small entity (tagged `fade_overlay`) that occupies the full viewport, color `[0, 0, 0, 0]`, z_order above all scene content. It never tweens on spawn; it is the transition overlay.
- In [examples/03_scene_state/src/states.rs](../../examples/03_scene_state/src/states.rs):
  - `Hub::update` checks a `PendingTransition` resource; when set, it inserts a `Tween` onto the `fade_overlay` entity with `ColorA { from: 0, to: 255 }`, `duration: 0.35`, `easing: CubicIn`, `on_complete_tag: Some("state_exit")`.
  - A new user system `handle_tween_complete_system` reads `EventQueue<TweenComplete>::iter()` (both windows) and, for `tag == "state_exit"`, calls `world.get_resource_mut::<StateStack>().request_replace(Hub::new())` (or pop/push per the stored `PendingTransition::target`), then clears the flag.
  - Reverse fade-in on `on_enter`: `spawn_scene` already runs; the scene-authored per-entity fade-in tweens cover the incoming animation. The `fade_overlay` gets a fade-out `ColorA { from: 255, to: 0 }, duration: 0.35, easing: CubicOut` tween inserted in `on_enter`.
- No new action bindings required — reuse the existing `state_start`/`state_back` actions from M20's `ActionMap` defaults.

### Budgets and Gates

- No per-tween cap; there is no analogue of `ParticleBudget`. One tween component per entity bounds the count at `<= entity_count`.
- New bench `crates/tungsten-core/benches/tween_tick.rs` — 5000 entities carrying `Tween + Transform + Sprite`; one `criterion` iteration advances one tick by a fixed `dt`. Target: `<= 10%` regression vs the baseline captured on the same reference machine as M23's `particle_tick`.
- Steady-state bench list (per [docs/plans/Phase3.md](Phase3.md) `Benchmark And Quality Gates`) gains a `tween_tick` entry; the entry is additive and does not displace existing benches.

### Decision Records

Three `DECISIONS.md` entries (next free ids after `D-053`):

- `D-054`: Tween easings are a closed `enum` with `fn apply(f32) -> f32`; no dependency, no trait object. Cites `D-015`: closed-form math on ~60 lines of code does not meet the "platform API / well-specified format / solved primitive" bar.
- `D-055`: Single `Tween` component per entity, multi-property via `Vec<TweenChannel>`. Cites the archetypal-ECS one-component-per-type constraint and M23 prior art of keeping per-emitter state in one mutable component. Alternatives (boxed component marker trick, per-property components) explicitly rejected.
- `D-056`: `TweenComplete` is routed through `EventQueue<T>` (D-040 two-window lifetime) and the tween component is removed through `CommandBuffer::remove_component` (D-039 deferred mutation). No direct `World` mutation from inside the tick system.

Add rows for each to [docs/DECISION_INDEX.md](DECISION_INDEX.md) under the appropriate bucket (first two under "Architecture choices", third under "Frame order / mutation").

## Ordered Steps

1. Add `crates/tungsten-core/src/tween.rs`: `Easing`, `Easing::apply`, `TweenChannel`, `TweenRepeat`, `TweenDirection`, `Tween` (with builder API), `lerp_u8` helper. No `World` references.
2. Unit-test `Easing::apply` endpoints (`apply(0.0) == 0.0` for all variants where the curve starts at 0; `apply(1.0) == 1.0`; `Linear` identity; `QuadIn(0.5) == 0.25`; Bounce closed-form reference values) and `lerp_u8` boundary behavior in `crates/tungsten-core/src/tests/tween.rs`.
3. Re-export `Tween`, `TweenChannel`, `TweenRepeat`, `Easing`, `TweenDirection`, `TweenComplete` from `crates/tungsten-core/src/lib.rs` alongside existing components.
4. Extend `crates/tungsten-core/src/assets/scene.rs`: add `SceneTween`, `SceneTweenChannel`, `SceneTweenRepeat`, `SceneEntry.tweens`. Implement `SceneTween::into_tween(&self) -> Tween` (pure conversion). Update existing `tests/assets/scene.rs` to cover a scene with tweens parsing round-trip and a rejected malformed tween (non-finite duration).
5. Add `crates/tungsten/src/tweens.rs` with `tween_tick_system`. Local buffer collects `(Entity, TweenComplete)` and `Entity` for `remove_component` so query borrow is released before `CommandBuffer` mutation.
6. Register `EventQueue<TweenComplete>` in `App::new` — extend the `register_event_inner::<…>` block at [crates/tungsten/src/app.rs:195-209](../../crates/tungsten/src/app.rs#L195-L209) with a fourth entry for `TweenComplete`.
7. Add `stage_tweens` method on `App` mirroring `stage_particles` ([crates/tungsten/src/app.rs:711-721](../../crates/tungsten/src/app.rs#L711-L721)); call `crate::tweens::tween_tick_system(&mut self.world)`. Wire the call in every frame path that currently invokes `stage_particles` (grep for both `render_frame_full_timed` and `render_frame_full`).
8. Extend `crates/tungsten/src/asset_loader.rs::spawn_scene` to insert `Tween` components from `entry.tweens[0]` and log a single `ERROR` + continue when `entry.tweens.len() > 1`.
9. Integration tests in `crates/tungsten/src/tests/` (new `tweens.rs`):
   - `tween_once_completes`: entity with `Tween{ Once, duration=0.1, ColorA 0→255 }` + `Transform` + `Sprite`; advance 5 ticks of `dt=0.03`; assert `Sprite.color[3] == 255`, `TweenComplete` in event queue, `Tween` component removed after flush.
   - `tween_times_completes_on_nth`: `Times(3)`; advance past 3 durations; expect exactly one `TweenComplete` and removal.
   - `tween_loop_never_completes`: `Loop`; advance past 5 durations; expect 0 `TweenComplete` events, `Tween` component still present.
   - `tween_pingpong_reverses`: `PingPong`; sample direction at `elapsed == duration` — direction flipped; `elapsed` clamped to boundary.
   - `tween_position_and_color_together`: single tween with `PositionX` + `ColorA` channels; assert both write at `u=0.5` with `Easing::Linear`.
   - `scene_tween_spawn_spawns_component`: build a `SceneData` with one entry carrying one tween; run `spawn_scene`; flush `CommandBuffer`; assert the spawned entity has a `Tween` component matching.
10. Add `crates/tungsten-core/benches/tween_tick.rs` using the `criterion` pattern of [crates/tungsten-core/benches/physics_bench.rs](../../crates/tungsten-core/benches/physics_bench.rs): 5 000 entities, each with a `Tween` carrying two channels; measure a single `tween_tick_system`-equivalent loop (inline the tick math to avoid depending on the umbrella crate from a core bench).
11. Wire the fade overlay + scene-authored fade-in tweens in [examples/03_scene_state/assets/scene.json](../../examples/03_scene_state/assets/scene.json) and the fade-out transition + `handle_tween_complete_system` in [examples/03_scene_state/src/states.rs](../../examples/03_scene_state/src/states.rs) and [examples/03_scene_state/src/main.rs](../../examples/03_scene_state/src/main.rs). Confirm `SceneEntity { state_id }` is still attached to every tween-carrying entity so state exit still despawns them.
12. Validation:
    - `cargo fmt && cargo test --workspace` — layer-1 manifest test still passes (tween addition doesn't touch manifest schema, but scene.json now carries `tweens` — confirm existing scene parses with or without the field per `#[serde(default)]`).
    - `./scripts/smoke-examples.sh` — every example renders `TUNGSTEN_SMOKE_FRAMES=3` cleanly, including `03_scene_state` with the new fade logic.
    - `cargo bench -p tungsten-core --bench tween_tick` — record baseline next to existing bench artifacts; note machine + profile in the bench commit.
    - Manual: `cargo run -p example-03-scene-state` — observe fade-in on state enter, fade-out on state exit, state replace fires after the overlay reaches opaque.
13. Update [docs/LLM_INDEX.md](LLM_INDEX.md) Subsystem Map with a row `Tweens (M24) | crates/tungsten-core/src/tween.rs, crates/tungsten/src/tweens.rs, crates/tungsten-core/src/assets/scene.rs`. Update the Task Map with a row for "Change tween easing/channel behavior or scene-tween authoring".
14. Update [docs/plans/Phase3.md](Phase3.md) M24 row to `complete`, record version `0.21.0` + date, link this archived plan, then bump workspace `Cargo.toml:2` to `0.21.0`. Update the "Current Status" paragraph to list M24 in completed milestones and replace the "Next recommended milestone" line.
15. Append `CHANGELOG.md` `[0.21.0]` section: "M24 Tween System — `Tween` + built-in easings in `tungsten-core`, `TweenComplete` via `EventQueue`, scene-authored tweens, `03_scene_state` fade-on-transition demo". Update [CLAUDE.md](../../CLAUDE.md) Status line similarly.
16. Add `DECISIONS.md` entries `D-054`/`D-055`/`D-056` per the Decision Records section; register in [docs/DECISION_INDEX.md](DECISION_INDEX.md).
17. Flip this plan's frontmatter `status: done` and move to `docs/plans/archive/phase3-milestone-24-plan.md`.

## Done When

- [ ] `crates/tungsten-core/src/tween.rs` defines `Easing`, `Tween`, `TweenChannel`, `TweenRepeat`, `TweenDirection`, `TweenComplete`, re-exported from `tungsten_core::*`.
- [ ] `Easing::apply` covers Linear/Quad/Cubic/Quart/Sine/Expo/Back/Bounce with In/Out/InOut variants; endpoint + known-sample unit tests pass.
- [ ] `SceneEntry.tweens` parses, rejects non-finite/zero duration, and round-trips through `serde_json`.
- [ ] `tween_tick_system` runs in the documented `particles → tweens → flush commands` slot in all frame paths in [crates/tungsten/src/app.rs](../../crates/tungsten/src/app.rs).
- [ ] `EventQueue<TweenComplete>` is registered in `App::new` and flushed by the existing event-flusher loop.
- [ ] `Once` and `Times(n)` send exactly one `TweenComplete` per tween and enqueue `CommandBuffer::remove_component::<Tween>(entity)`; `Loop` and `PingPong` do not send completion events.
- [ ] `spawn_scene` inserts scene-authored tweens on the spawned entities; duplicate-tween-per-entry logs `ERROR` and keeps the first.
- [ ] `cargo run -p example-03-scene-state` shows fade-in on enter and fade-out on transition, and a user-triggered state change pops/replaces only after `TweenComplete { tag: "state_exit" }` arrives.
- [ ] `cargo test --workspace` passes; `./scripts/smoke-examples.sh` passes; `bash scripts/test-perf-capture.sh` still passes (no perf parser change expected).
- [ ] `crates/tungsten-core/benches/tween_tick.rs` recorded a baseline on the reference machine; `<= 10%` regression documented.
- [ ] `DECISIONS.md` entries `D-054`/`D-055`/`D-056` land with matching rows in [docs/DECISION_INDEX.md](DECISION_INDEX.md).
- [ ] [docs/LLM_INDEX.md](LLM_INDEX.md) gains a Tweens subsystem row and a matching task-map row.
- [ ] [docs/plans/Phase3.md](Phase3.md) M24 is `complete` with version `0.21.0` + date; workspace version is `0.21.0`; `CHANGELOG.md` carries the `[0.21.0]` entry.
- [ ] This file is archived under `docs/plans/archive/phase3-milestone-24-plan.md` with `status: done`.
