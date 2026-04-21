---
status: done
goal: Ship ECS-native particle emitters/particles that reuse the M15 sprite path, load config from a manifest `particles` section via asset ID, hot-reload with in-flight snapshot semantics, enforce per-emitter + global caps, emit typed lifecycle events, and demonstrate in the platformer black hole.
non-goals: new render pipeline, additive GPU blend pass, compute/GPU particles, mesh or ribbon trails, sub-emitters, particle-collision response, authoring UI, serialized emitter state across runs, Phase-4 physics integration.
files-to-touch:
  - crates/tungsten-core/src/components.rs
  - crates/tungsten-core/src/assets/manifest.rs
  - crates/tungsten-core/src/assets/registry.rs
  - crates/tungsten-core/src/assets/mod.rs
  - crates/tungsten-core/src/assets/particle.rs (new)
  - crates/tungsten-core/src/rng.rs (new)
  - crates/tungsten-core/src/lib.rs
  - crates/tungsten-core/benches/particle_tick.rs (new)
  - crates/tungsten/src/particles.rs (new)
  - crates/tungsten/src/asset_loader.rs
  - crates/tungsten/src/hot_reload.rs
  - crates/tungsten/src/app.rs
  - crates/tungsten/src/lib.rs
  - examples/01_platformer/assets/manifest.json
  - examples/01_platformer/assets/particles/black_hole.json (new)
  - examples/01_platformer/src/setup.rs
  - examples/01_platformer/src/systems.rs
  - docs/LLM_INDEX.md
  - docs/plans/Phase3.md
  - DECISIONS.md
  - CHANGELOG.md
---

# Phase 3 M23 — Particle System

## Context Digest

- Scope source: [docs/plans/Phase3.md](Phase3.md) `M23` row: burst/continuous + hot-reload config + command-buffer despawn + reuse `Sprite` path from M15.
- Dependencies satisfied: M13 `CommandBuffer`, M14 `EventQueue<T>`, M15 `Transform`/`Sprite`/`Visibility`.
- Seam: emitters + particles live as components in `tungsten-core`; tick/emit systems + asset load wiring live in `tungsten`; no `wgpu` touched.
- Render path: particles are normal entities with `Transform + Sprite + Visibility + Particle`; the default M15 extract picks them up. No new WGSL, no new pipeline, no bind-group changes.
- Blend: "adaptive" via existing sprite alpha pipeline — particle config selects one of two runtime color pre-processing modes (`alpha` / `premultiplied`) applied CPU-side before writing `Sprite.color`. No shader change.
- Determinism: each emitter owns a `Pcg32` seeded either by config `seed: u64` or by a monotonic `WorldRngSeed` resource (incremented per spawn) so screenshot tests reproduce with a fixed world seed.
- Hot-reload semantics: registry holds `Arc<ParticleConfig>`; emitter state caches one `Arc` snapshot at spawn; particles carry that same `Arc`. Reload swaps the registry Arc only; existing emitters and live particles keep their snapshot.

## Entity-vs-Pool Tradeoff (resolved)

Chosen: **entity-per-particle**.

- Scope text mandates command-buffer despawn and reuse of the M15 sprite path; both only compose cleanly if particles are entities.
- Cost bound: global cap resource + per-emitter `max_alive` keep archetype churn and query size bounded. Bench gate at 5k particles.
- Pool alternative rejected: would require a second extract path and a custom instance writer, duplicating M22 atlas + M15 `Visibility` rules.

## Data Model

### Asset: `ParticleConfig`

- Location: `crates/tungsten-core/src/assets/particle.rs`.
- Registered under new manifest section `particles` (sibling of `sprites` / `animations` / `fonts` / `sounds`).
- JSON schema:

```json
{
  "sprite": "sprite_asset_id",
  "max_alive": 256,
  "seed": 0,
  "blend": "alpha",
  "emission": {
    "kind": "continuous",
    "rate_hz": 120.0
  },
  "lifetime": { "min": 0.6, "max": 1.2 },
  "initial_velocity": {
    "kind": "cone",
    "direction": [0.0, 1.0],
    "spread_deg": 45.0,
    "speed": { "min": 10.0, "max": 80.0 }
  },
  "gravity": [0.0, -300.0],
  "drag_per_sec": 0.5,
  "angular_velocity": { "min": -2.0, "max": 2.0 },
  "start_scale": { "min": 1.0, "max": 1.0 },
  "scale_over_life": [[0.0, 1.0], [1.0, 0.0]],
  "color_over_life": [[0.0, [0.8, 0.2, 1.0, 1.0]], [1.0, [0.2, 0.0, 0.4, 1.0]]],
  "alpha_over_life": [[0.0, 1.0], [0.8, 1.0], [1.0, 0.0]]
}
```

- `emission.kind ∈ {"burst","continuous","pulse"}`.
  - `burst`: `{ count: u32, once: bool }`.
  - `continuous`: `{ rate_hz: f32 }`.
  - `pulse`: `{ count_per_pulse: u32, interval_sec: f32, total_pulses: Option<u32> }`.
- `initial_velocity.kind ∈ {"cone","radial","vector"}`.
- Curves: piecewise-linear, sorted on `t ∈ [0,1]`; parser enforces monotone `t`, clamps sample to endpoints.
- Manifest entry form:

```json
"particles": [
  { "id": "black_hole_swirl", "path": "particles/black_hole.json" }
]
```

- Registry type: `ParticleConfigRegistry` keyed by `AssetId<ParticleConfig>` → `Arc<ParticleConfig>`. Mirrors `SpriteRegistry`.

### Components (in `tungsten-core/src/components.rs`)

```rust
pub struct ParticleEmitter {
    pub config: AssetId<ParticleConfig>,
    pub seed_override: Option<u64>,
}

pub struct ParticleEmitterState {
    pub config_snapshot: Arc<ParticleConfig>,
    pub rng: Pcg32,
    pub elapsed: f32,
    pub continuous_accum: f32,
    pub pulse_timer: f32,
    pub pulses_fired: u32,
    pub active_count: u32,
    pub drained: bool,
    pub first_tick_done: bool,
}

pub struct Particle {
    pub config: Arc<ParticleConfig>,
    pub emitter: Option<EntityId>,
    pub age: f32,
    pub lifetime: f32,
    pub velocity: Vec2,
    pub angular_velocity: f32,
    pub start_scale: f32,
    pub base_rgba: [f32; 4],
}
```

- `ParticleEmitter` is immutable post-spawn (config id + seed override only).
- `ParticleEmitterState` is the only mutable emitter data; always paired with `ParticleEmitter` via a scene helper `App::spawn_emitter(cmd, config_id, transform, seed_override) -> PendingEntity`.
- `Particle` stores sampled values at spawn so hot-reload cannot alter in-flight motion.

### Resources

- `ParticleBudget { global_cap: u32 }` — default `10_000`. Lives in `tungsten-core`, inserted by `App::default`.
- `WorldRngSeed { next: u64 }` — monotonic u64 counter; `WorldRngSeed::derive_seed(&mut self) -> u64` uses SplitMix64 over `next` and post-increments.
- `ParticleEvents`: register `EventQueue<ParticleBurstEmitted>` + `EventQueue<ParticleSystemDrained>` via `App::register_event::<T>()`.

### Events

```rust
pub struct ParticleBurstEmitted { pub emitter: EntityId, pub count: u32 }
pub struct ParticleSystemDrained { pub emitter: EntityId }
```

- `ParticleBurstEmitted` fires on every `burst` and `pulse` discrete emission; not for continuous.
- `ParticleSystemDrained` fires once when `state.drained && state.active_count == 0` transitions true.

### RNG (`tungsten-core/src/rng.rs`)

- `Pcg32 { state: u64, inc: u64 }` — LCG with XSH-RR output. Inlined implementation, no dep.
- `SplitMix64(seed: u64) -> u64` — used to derive `inc` from `seed` and to mix `WorldRngSeed`.
- API:

```rust
impl Pcg32 {
    pub fn seeded(seed: u64) -> Self;
    pub fn next_u32(&mut self) -> u32;
    pub fn next_f32_unit(&mut self) -> f32;       // [0, 1)
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32;
    pub fn next_unit_vec2(&mut self) -> Vec2;     // radial sampler
}
```

- Unit tests: SplitMix64 matches reference vectors; `next_f32_unit` bounded; `next_range` distribution mean within tolerance over 10k samples.
- DECISIONS.md entry: PRNG rolled in-tree (cites `D-015` "no new runtime dep without entry"); `rand`/`fastrand` rejected for this milestone.

## Systems

File: `crates/tungsten/src/particles.rs`.

- `particle_count_refresh_system(world)` — scans `&Particle`, resets then accumulates per-emitter `active_count` into `ParticleEmitterState` by `Particle::emitter`.
- `particle_emit_system(world, dt, cmd, particle_registry, budget, world_rng_seed, burst_events)`
  - Query: `(&ParticleEmitter, &mut ParticleEmitterState, &Transform)`.
  - On first tick for an emitter: resolve `Arc<ParticleConfig>` via registry, seed `rng` from `seed_override ?? config.seed ?? world_rng_seed.derive_seed()`, mark `first_tick_done`.
  - Branch on `config_snapshot.emission.kind`:
    - `burst`: emit `count` once; if `once == true`, set `drained = true`.
    - `continuous`: `accum += rate_hz * dt`; emit `floor(accum)`; subtract.
    - `pulse`: step `pulse_timer`; each overrun emits `count_per_pulse`, increments `pulses_fired`; drain when `total_pulses` reached.
  - Emission helper `emit_n(cmd, emitter_ent, state, transform, n, budget, burst_events)`:
    - `n_eff = min(n, config.max_alive - state.active_count, budget.global_cap - global_active)`.
    - For each particle: sample lifetime, velocity, angular velocity, start scale, base color; build `Particle + Transform + Sprite + Visibility{visible:true}`; `cmd.spawn(...)`.
    - If `config.emission.kind ∈ {burst, pulse}` and `n_eff > 0`: `burst_events.send(ParticleBurstEmitted { emitter: emitter_ent, count: n_eff })`.
  - End: if `state.drained && state.active_count == 0 && !previously_reported`: `drained_events.send(ParticleSystemDrained { emitter })`.
- `particle_tick_system(world, dt, cmd)`
  - Query: `(&mut Particle, &mut Transform, &mut Sprite)`.
  - `p.age += dt`; if `p.age >= p.lifetime`: `cmd.despawn(entity)`; continue.
  - `u = p.age / p.lifetime`.
  - Integrate: `p.velocity += gravity * dt`; apply drag `p.velocity *= exp(-drag_per_sec * dt)`.
  - `t.position += p.velocity * dt`; `t.rotation += p.angular_velocity * dt`.
  - `t.scale = vec2_splat(p.start_scale * sample_curve(scale_over_life, u))`.
  - Compute `rgba = sample_rgba_curve(color_over_life, u) * sample_curve(alpha_over_life, u)`.
  - If `blend == premultiplied`: `rgba.rgb *= rgba.a` before writing.
  - `s.color = rgba * p.base_rgba`.
- Global active count for the budget check: track via a `ParticleActive { count: u32 }` resource updated by `particle_count_refresh_system` (sum of all emitter `active_count` + orphaned particles whose `emitter == None`).

### Frame Order Integration

- App-phase insertion, before existing `flush command buffers → flush event queues → hot reload → extract → render`:
  1. user systems
  2. `particle_count_refresh_system`
  3. `particle_emit_system`
  4. `particle_tick_system`
  5. existing `CommandBuffer::flush`
  6. existing event flush
  7. hot reload
  8. extract
  9. render
- Emit-before-tick guarantees newly spawned particles don't skip a frame; despawn is still deferred via `cmd`.

## Hot Reload

- Extend `tungsten/src/asset_loader.rs` to parse `particles` manifest section, resolve referenced sprite id exists, parse each `.json`, insert `Arc<ParticleConfig>` into `ParticleConfigRegistry`.
- Extend `tungsten/src/hot_reload.rs` watcher match arms for `assets/particles/*.json` and example-local `assets/particles/*.json`: re-parse and `Arc::swap` the registry entry on change.
- In-flight guarantee: emitters and particles hold `Arc` clones taken at spawn; registry swap does not visit them.
- Re-arm: emitters that have not yet run `first_tick_done` pick up the new Arc. Emitters already running keep the old snapshot until despawned and respawned.
- Invalid reload: log a warning, retain previous Arc. Same pattern as scene.json reload.

## Example Integration — Platformer Black Hole

- Add `examples/01_platformer/assets/particles/black_hole.json`:
  - `sprite: "spark"` (new 4x4 `linear` sprite added to example manifest if none suitable exists; prefer an existing small sprite if available).
  - `max_alive: 384`.
  - `emission: { kind: "continuous", rate_hz: 160.0 }`.
  - `lifetime: { min: 0.4, max: 1.1 }`.
  - `initial_velocity: { kind: "radial", speed: { min: 40.0, max: 140.0 } }`.
  - `gravity: [0.0, 0.0]`, `drag_per_sec: 1.2`.
  - `angular_velocity: { min: -6.0, max: 6.0 }`.
  - `start_scale: { min: 0.6, max: 1.2 }`.
  - `scale_over_life: [[0.0, 1.0], [1.0, 0.0]]`.
  - `color_over_life`: purple ramp `[0.55, 0.2, 0.95, 1.0] → [0.25, 0.05, 0.55, 1.0]`.
  - `alpha_over_life: [[0.0, 0.0], [0.15, 1.0], [1.0, 0.0]]`.
  - `blend: "premultiplied"`.
- Register in `examples/01_platformer/assets/manifest.json` under `particles`.
- `examples/01_platformer/src/setup.rs`: in the black-hole spawn site, also spawn an emitter entity with `Transform { position: black_hole_pos, ... }` and `ParticleEmitter { config: ids.black_hole_swirl, seed_override: None }` + default `ParticleEmitterState`.
- `examples/01_platformer/src/systems.rs`: add a follow system that keeps emitter `Transform` pinned to the black hole entity position each frame (mirrors any other child-follow pattern already used; otherwise use a small inline system).
- No change to example extract (`extract.rs`): default M15 sprite extract covers particles.

## Budgets and Gates

- Per-emitter cap: `config.max_alive`.
- Global cap: `ParticleBudget::global_cap` default `10_000`, overridable via `tungsten.json` `particles.global_cap` (add optional field, not required).
- Perf gate: new bench `particle_tick` at `5000` active particles advancing one frame; `<= 10%` regression vs newly captured baseline.
- No startup bench needed (config parse cost is negligible at this asset count).

## Ordered Steps

1. Add `crates/tungsten-core/src/rng.rs` with `Pcg32` + `SplitMix64`; unit tests; re-export from `lib.rs`.
2. Add `crates/tungsten-core/src/assets/particle.rs` with `ParticleConfig`, curve types, `EmissionKind`, velocity shape enums, serde derive, validation (monotone `t`, non-empty curves, non-negative `max_alive`, finite floats).
3. Extend `crates/tungsten-core/src/assets/manifest.rs` with `particles: Vec<ManifestAssetRef>`; extend `ResolvedManifest::load` to resolve particle `.json` files; fail hard on duplicate ids or missing sprite refs.
4. Extend `crates/tungsten-core/src/assets/registry.rs` with `ParticleConfigRegistry`; expose `get(AssetId<ParticleConfig>) -> Option<Arc<ParticleConfig>>`.
5. Add `Particle`, `ParticleEmitter`, `ParticleEmitterState` to `crates/tungsten-core/src/components.rs` behind the existing component registration pattern.
6. Add `ParticleBudget`, `WorldRngSeed`, `ParticleActive` resources in `tungsten-core`; default-insert from `App::default` in `tungsten`.
7. Add `crates/tungsten/src/particles.rs` with the three systems + emission helper; register in `tungsten/src/app.rs` with the frame-order slot above; register the two events via `App::register_event::<...>()`.
8. Extend `crates/tungsten/src/asset_loader.rs` to parse and upload `ParticleConfig` into `ParticleConfigRegistry`; reuse existing JSON loader + error path; no GPU upload.
9. Extend `crates/tungsten/src/hot_reload.rs` watcher with a `particles/*.json` arm that `Arc::swap`s the registry entry; warn-and-retain on parse failure.
10. Unit tests in `tungsten-core`:
    - Curve sampling: endpoints, midpoints, unsorted-rejection.
    - `ParticleConfig` serde round-trip.
    - Manifest: duplicate id fatal; unknown sprite id fatal.
    - RNG: `next_f32_unit` bounds; distribution sanity.
11. Integration tests in `tungsten`:
    - Burst once: after one tick, emits `N` particles, sends `ParticleBurstEmitted`, next tick sets `drained`, eventually sends `ParticleSystemDrained`.
    - Continuous: after `1.0 s` at `100 Hz`, expect `~100` particles within ±2.
    - Pulse with `total_pulses = 3`: exactly 3 `ParticleBurstEmitted` events, then `ParticleSystemDrained`.
    - Budget: per-emitter `max_alive` and global cap both reject excess emissions without panicking.
    - Hot-reload: swap `color_over_life`, new emissions use new colors, pre-existing particles keep old Arc.
12. Add `crates/tungsten-core/benches/particle_tick.rs` mirroring the existing bench harness layout (alongside `broad_phase` / `sprite_extract`); 5k particles; record baseline on the reference Linux machine.
13. Wire black-hole emitter in `examples/01_platformer/` (assets + setup + follow system). Confirm `Visibility { visible: true }` is present.
14. Smoke: `cargo fmt && cargo test --workspace`; `./scripts/smoke-examples.sh`; `bash scripts/test-perf-capture.sh` if perf parser touched (it will not be).
15. Manual visual check: run `cargo run -p example-01-platformer`; verify purple swirl around the black hole; edit `black_hole.json` while running; confirm new particles pick up changes within ~1 s while in-flight particles keep prior appearance.
16. Update [docs/LLM_INDEX.md](LLM_INDEX.md) with a `Particles (M23)` row pointing to the new core + tungsten files.
17. Update [Phase3.md](Phase3.md) M23 row to `complete` with version bump and date; link archived detailed plan; bump workspace version to `0.20.0` in `Cargo.toml` workspace root and any member crates that track it.
18. Add `DECISIONS.md` entries:
    - `D-0xx`: in-tree PCG32 instead of `rand`/`fastrand` dep, cites `D-015`.
    - `D-0xx`: `Arc<ParticleConfig>` snapshot semantics for hot-reload in-flight immutability.
    - `D-0xx`: entity-per-particle over pooled storage, cites reuse of M15 extract and M13 despawn.
    - Register entries in [docs/DECISION_INDEX.md](DECISION_INDEX.md).
19. Append `CHANGELOG.md` under next version: "M23 Particle System — manifest-registered emitters, hot-reloadable, adaptive blending, platformer black-hole demo".
20. Move this plan to `docs/plans/archive/phase3-milestone-23-particle-system.md` on close; flip frontmatter `status: done`.

## Done When

- [ ] `ParticleEmitter`, `ParticleEmitterState`, `Particle` exist in `tungsten-core` with the signatures above.
- [ ] `particles` manifest section parses, resolves, and rejects duplicates + unknown sprite refs.
- [ ] `ParticleConfigRegistry` stores `Arc<ParticleConfig>` and hot-swaps on file change without mutating live Arcs.
- [ ] `particle_count_refresh_system`, `particle_emit_system`, `particle_tick_system` registered in the documented frame-order slot.
- [ ] `Burst`, `Continuous`, `Pulse` modes behave per tests in step 11.
- [ ] `ParticleBurstEmitted` and `ParticleSystemDrained` round-trip through `EventQueue<T>` and pass integration tests.
- [ ] Per-emitter `max_alive` and `ParticleBudget::global_cap` both clip emissions; integration test verifies both.
- [ ] Platformer demo: `cargo run -p example-01-platformer` renders purple particles centered on the black hole; live editing `black_hole.json` affects new particles only.
- [ ] `cargo test --workspace` passes; `./scripts/smoke-examples.sh` passes.
- [ ] `particle_tick` bench added; baseline recorded; regression envelope `<= 10%` documented in bench notes.
- [ ] `DECISIONS.md` entries for PRNG choice, Arc snapshot reload semantics, and entity-per-particle tradeoff land with `D-0xx` ids; `DECISION_INDEX.md` updated.
- [ ] `docs/LLM_INDEX.md` lists the new Particles subsystem row.
- [ ] [Phase3.md](Phase3.md) M23 row is `complete` with version + date; this file is archived and set to `status: done`.
