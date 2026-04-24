# Changelog

Records all notable project changes.

Format reference: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.23.0] - 2026-04-24

Summary: Phase 4 Milestone 26 — materials + post-stack + tween→material bridge (manifest-tracked materials, a reorderable 17-effect post stack, entity-local uniform overrides shared with tween channels, and a new shader-playground example). Phase 4 scope is tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md).

### Added

- M26 materials + post-stack + tween→material bridge (`D-058`). New `materials` section in the manifest graph maps a stable material id to a WGSL shader id + 256-byte `MaterialUniformDefaults`; render-side `MaterialPipeline` reuses the built-in sprite layout and adds a per-material UBO at group 2. New `PostStack` world resource (default empty, byte-identical to the M25 baseline) carries a reorderable `Vec<PostPass>` — 17 stock effects (tonemap, vignette, lut, chromatic_aberration, color_adjust, tone_mono, crt, film_grain, dither, pixel_outline, fade, wipe_radial, dissolve, glitch, pixelate, fog, god_rays) ping-pong between `PostPing` / `PostPong` offscreen targets before the present blit. New `UniformOverrideBlock` component + `TweenChannel::UniformVec4Lane` / `UniformScalar` / `UniformInt` drive per-entity animation into the same 256-byte payload shared with the M32 MSDF outline/glow slot. Stock shaders live under `crates/tungsten-render/src/shaders/stock/` with MIT LYGIA-derived helpers; `assets/shaders/stock/` mirrors them for manifest-driven hot reload. New workspace `damage_flash` material + platformer ball-hit tween fires through the new `Sprite.material_id` path. New `example-04-shader-playground` crate exercises the 17-effect fixture under `TUNGSTEN_POST_STACK_FIXTURE`.
- `scripts/smoke-examples.sh` appends a `TUNGSTEN_POST_STACK_FIXTURE ∈ empty, all` matrix over `example-04-shader-playground`.

### Changed

- Workspace version bumped to `0.23.0`.
- `README.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/phase4.md` now reflect branch `0.23` with both M25 and M26 shipped; the detailed M26 plan moved to [`docs/plans/archive/phase4-milestone-26-materials-post-stack.md`](docs/plans/archive/phase4-milestone-26-materials-post-stack.md).
- `AGENTS.md` §Asset Rules lists the new `materials` manifest section and the vendored `assets/shaders/stock/` mirror rule.
- `DESIGN.md` §Status and §Hot Reload matrix: M26 row added; `shader` row widened to include material-pipeline rebuilds on shader reload.

### Fixed

- **M26 release-polish QA pass:** `MaterialUniformDefaults::to_override_block()` now builds its `UniformOverrideBlock` in one initializer, `PostStack::{as_slice, as_slice_mut}` are marked `#[must_use]`, and `UniformOverrideBlock` no longer exposes its reserved padding tail as a public field. This keeps `cargo clippy --workspace --all-targets -- -D warnings` green without weakening the lint surface.
- Release QA pass completed locally: `cargo fmt --all --check`, `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `bash scripts/test-perf-capture.sh`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --telemetry-only`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` all passed.

## [0.22.0] - 2026-04-24

Summary: Phase 4 Milestone 25 — render foundation (offscreen `SceneTarget` with optional depth + MSAA, named/ordered pass list with an engine-internal present blit, manifest-tracked WGSL with body-edit hot reload, and opt-in GPU depth-test sprite path). Phase 4 scope is tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md).

### Added

- M25 render foundation (`D-057`): offscreen `SceneTarget` (color + optional depth + optional MSAA) driven by a named, ordered pass list (`scene` → `present`). The present pass blits `SceneColor` into the swapchain via an engine-internal `shaders/present_blit.wgsl` with exact-texel `textureLoad` so the default `msaa=1`, `depth_sort=cpu_stable` config stays byte-identical to the 0.21 baseline.
- M25 config knobs in `RenderConfig`: `msaa` (1 | 2 | 4 | 8, default 1), `depth_enabled` (default `true`), `depth_sort` (`cpu_stable` default, `gpu_depth` opt-in) with matching `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_DEPTH_ENABLED`, `TUNGSTEN_RENDER_DEPTH_SORT` env overrides.
- M25 WGSL hot reload: shaders move into `assets/shaders/` under a manifest `shaders` section with a core-side `ShaderRegistry` + render-side `ShaderModuleCache`. Body edits hot-reload through the existing umbrella `notify` watcher after `wgpu::naga` parse + validation; the previous `ShaderModule` + live pipeline stay intact on any validation or rebuild failure.
- M25 GPU depth-test sprite path: `SpriteInstance` gains a `z_norm` field derived from deterministic `(z_order, Entity::id)` painter order; under `depth_sort = "gpu_depth"` the sprite pipeline attaches `Depth32Float` with `depth_compare = LessEqual` so the depth buffer reproduces the same visible order as the CPU-stable path.
- `scripts/smoke-examples.sh` appends a `{msaa ∈ 1, 4} × {depth_sort ∈ cpu_stable, gpu_depth}` matrix over `example-02-sprite-stress` via the new env overrides.

### Changed

- Workspace version bumped to `0.22.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, `docs/plans/phase4.md`, and `docs/plans/phase4-milestone-25-render-foundation.md` now reflect branch `0.22` as the active integration line.
- `AGENTS.md` §Asset Rules: shaders are now manifest-tracked with body-edit hot reload (`D-057`).
- `DESIGN.md` §Status + §Hot Reload matrix: `shader` row added, `SceneColor` format noted.
- `tungsten.json` `render` block documents the new `msaa` / `depth_enabled` / `depth_sort` defaults.
- `renderer.rs` is split: surface/present-mode helpers moved to `surface.rs`, frame timing types moved to `timing.rs`, and the main frame now loops over a `PassOrder` instead of a single inline `begin_render_pass`.
- `SpriteInstance` grew from 40 B to 48 B (+20%) to carry the new `z_norm: f32` and an explicit 4-byte `_pad` for 16-byte GPU alignment. The default-data path still writes `z_norm = 0.0` for callers that build instances by hand (`SpriteInstance::whole`, tilemap extract, custom example extracts).
- Screenshot path simplified: captures now read directly from `SceneColor` after the scene pass (single draw, no duplicate capture-only pass). Under `msaa > 1` the read picks up the resolved target. Baseline image-diff is still byte-stable for the default config.

### Fixed

- **Smoke-mode dt is now deterministic.** Under `TUNGSTEN_SMOKE_FRAMES`, `App::stage_delta_time` pins the per-frame `DeltaTime.dt` to `1/60 s` instead of reading wall-clock. Previously the visual-regression fixture run and a subsequent test re-run would integrate different `dt` values at frame N, so sprite positions (and therefore pixels) diverged across otherwise-identical runs. With the pin in place, smoke-mode captures are reproducible across build profiles and host load, which is what the `visual_regression` fixture requires. Outside smoke mode, dt continues to come from `Instant::now()` as before.
- **M25 QA pass:** four `GpuDepth` / MSAA bugs that would have reached 0.22 release without this sweep.
  - `depth_sort = gpu_depth` now forces the quad / debug-line / text pipelines to carry a matching read-only `DepthStencilState` (`Always` + no write). Previously they declared no depth state and wgpu rejected them the moment the pass attached `SceneDepth`.
  - `Renderer::new` now builds the sprite pipeline with the correct `depth_write` up front; the first frame under `gpu_depth` no longer boots a `depth: None` pipeline against a depth-attached pass.
  - `SceneColorMsaa` drops `COPY_SRC` + `TEXTURE_BINDING` from its usage flags — multisampled textures reject `COPY_SRC` in wgpu, and nothing reads from the MSAA color target directly (the present blit reads the resolved `SceneColor`).
  - `depth_sort = gpu_depth` + `depth_enabled = false` used to panic in the recorder (requested a depth target the pool never allocated). `Renderer::new` now logs and falls back to `cpu_stable` for that combination; `default_pass_order` takes `depth_enabled` so the depth attachment can never disagree with the pool.
- **Painter-order depth orientation:** `z_norm` now decreases along painter order (`(total-1-i)/total`) so `LessEqual` accepts later-drawn fragments as they overwrite earlier overlaps. The previous ascending formula silently culled every later-drawn sprite in overlapping stacks under `gpu_depth`.

## [0.21.0] - 2026-04-23

Summary: Phase 3 Milestone 24 — tween system (closed-enum easings, multi-channel tweens on one component, `TweenComplete` via `EventQueue`, scene-authored tweens, and a fade-on-state-transition demo in `03_scene_state`).

### Added

- **Tween primitives (`tungsten_core::tween`):** `Easing` (Linear/Quad/Cubic/Quart/Sine/Expo/Back/Bounce × In/Out/InOut; `Easing::apply(t)` pre-clamped `[0,1]` — Back/Bounce overshoot intentionally), `TweenChannel` (per-property track for `PositionX/Y`, `Rotation`, `ScaleX/Y`, `ColorR/G/B/A`), `TweenRepeat { Once, Loop, PingPong, Times(u32) }`, `TweenDirection`, `Tween { channels, easing, duration, elapsed, repeat, direction, completed_cycles, on_complete_tag, pending_remove }` with `Tween::new(duration, easing).with_channel().with_repeat().with_tag()` builders, `lerp_f32` / `lerp_u8` helpers, and `TweenComplete { entity, tag }`. Closed `enum` avoids a trait-object dependency per `D-054` / `D-015` rule 3.
- **Scene-authored tweens (`tungsten_core::assets::scene`):** `SceneEntry.tweens: Vec<SceneTween>` plus `SceneTween { duration, easing, repeat, tag, channels }`, `SceneTweenChannel` (tagged-union mirror of `TweenChannel`), and `SceneTweenRepeat` (`once | loop | ping_pong | { "times": n }`). `SceneData::load` runs `SceneTween::validate()` on every tween — non-finite / non-positive durations and empty channel lists are fatal via a new `SceneError::Validation` variant.
- **`tween_tick_system` (`tungsten::tweens`):** advances every `Tween` using `DeltaTime.dt`, writes interpolated `Transform` / `Sprite` fields in-place, and defers terminal completion through `EventQueue<TweenComplete>` + `CommandBuffer::remove_component::<Tween>`; `Once` / `Times(n)` emit exactly one `TweenComplete` and latch `pending_remove` so subsequent ticks cannot re-fire before the next frame-end flush; `Loop` rewinds silently, `PingPong` flips direction at each boundary.
- **Frame-order slot:** `App::render_frame_*` now runs `stage_tweens` between `stage_particles` and `stage_flush_commands`, so tween writes override particle writes on the same frame and `TweenComplete` enqueues before the event flush window rotates (D-039 / D-040).
- **`App::new` event registration:** `EventQueue<TweenComplete>` is pre-registered alongside `CollisionEvent`, `ParticleBurstEmitted`, and `ParticleSystemDrained`.
- **Scene spawn (`tungsten::asset_loader`):** `spawn_scene` inserts one `Tween` per entry; entries carrying more than one tween log `ERROR` and keep the first (D-055 archetypal one-component-per-type).
- **Example 03 fade transitions (`examples/03_scene_state`):** `scene.json` now ships `color_a 0 → 255 / cubic_out / 0.45s` fade-in tweens on five representative hub/ring sprites. `GameplayState::on_enter` spawns a full-viewport black `fade_overlay` that tweens `color_a 255 → 0`. Pressing `state_back` inserts a reverse `color_a → 255 / 0.35s / cubic_in` tween tagged `state_exit`; the new `handle_tween_complete_system` reads `EventQueue<TweenComplete>` and calls `StateStack::request_replace(MainMenuState)` only after the opaque frame arrives. A `PendingTransition` resource gates re-presses while the fade-out is in flight.
- **`tween_tick` bench (`crates/tungsten-core/benches/tween_tick.rs`):** 5 000 entities each carrying `Tween + Transform + Sprite` with two channels and `cubic_in_out` easing; one `criterion` iteration advances the inline equivalent of `tween_tick_system`. Baseline additive to the existing `particle_tick_5k` / `position_integration_50k` / `broadphase_rebuild_5k` / `action_map_dispatch` benches.
- **Integration tests (`crates/tungsten/src/tests/tweens.rs`):** `tween_once_completes_and_removes_component`, `tween_times_fires_once_after_n_cycles`, `tween_loop_never_completes`, `tween_pingpong_reverses_at_boundary`, `tween_position_and_color_together_at_u_half`, `tween_complete_carries_tag`, `tween_without_target_components_is_noop`, `scene_tween_spawns_component_through_command_buffer`. Core unit tests in `crates/tungsten-core/src/tests/tween.rs` cover every easing endpoint + known-sample values and `lerp_u8` clamp behavior. Scene tween parsing + validation covered in `crates/tungsten-core/src/tests/assets/scene.rs`.
- **Decision records:** `DECISIONS.md` adds `D-054` (closed-enum easings, no trait object, no dependency), `D-055` (single `Tween` component per entity with `Vec<TweenChannel>`), and `D-056` (`TweenComplete` routes through `EventQueue`, component removal through `CommandBuffer`). `docs/DECISION_INDEX.md` carries matching takeaways.

### Changed

- Workspace version bumped to `0.21.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.21.0` / branch `0.21` release line, `M24` complete, and Phase 3 closeout.
- `docs/LLM_INDEX.md` gains a Tweens subsystem row and a "Change tween easing/channel behavior or scene-tween authoring" task row.
- `docs/plans/Phase3.md` marks `M24` as `complete` at `v0.21.0` / `2026-04-23`; the implementation plan is archived at `docs/plans/archive/phase3-milestone-24-plan.md`.
- `CLAUDE.md` Status line now reflects `0.21.0` on branch `0.21` with M24 shipped.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, and `./scripts/smoke-examples.sh` all passed.

## [0.20.0] - 2026-04-20

Summary: Phase 3 Milestone 23 — particle system (ECS-native emitters with Arc-snapshot hot reload, per-emitter/global caps, burst/continuous/pulse modes, lifecycle events, and a platformer black-hole demo).

### Added

- **In-tree PRNG (`tungsten_core::rng`):** `Pcg32` (PCG32 XSH-RR, seeded via `Pcg32::seeded(u64)`, `next_u32`, `next_f32`, `range_f32`) plus a `splitmix64(u64) -> u64` helper and a `WorldRngSeed` resource that mints per-emitter seeds through SplitMix64 so emitters are decoupled from spawn order. No new dependency — `rand` / `getrandom` were both rejected under the three-rule acceptance test (`D-049`). Unit tests cover statistical sanity, deterministic replay, and SplitMix64 distribution.
- **Particle asset + registry (`tungsten_core::assets::particle`):** `AssetId<ParticleConfig>`, `Range`, `BlendMode { Alpha, Premultiplied }`, `EmissionKind { Burst { count, once }, Continuous { rate_hz }, Pulse { count_per_pulse, interval_sec, total_pulses } }`, `InitialVelocity { Cone, Radial, Vector }`, `Curve<V: Copy + Lerp>` with a piecewise-linear sampler, and `ParticleConfig` carrying `sprite / max_alive / seed / blend / emission / lifetime / initial_velocity / gravity / drag_per_sec / angular_velocity / start_scale / scale_over_life / color_over_life / alpha_over_life / tint`. `ParticleConfig::load(path)` parses JSON and runs `validate()` (checks ranges are min ≤ max, lifetimes positive, emission parameters non-negative, curves sorted and non-empty). `ParticleConfigRegistry` holds `Arc<ParticleConfig>` per id and exposes `register / replace / get / id_for_name / name_for_id / id_for_path / path_for_id`. `ParticleBudget { global_cap }` and `ParticleActive { count }` are engine-owned resources.
- **Manifest `particles` section (`tungsten_core::assets::manifest`):** `RawManifest` / `ResolvedManifest` gain `particles: HashMap<String, ParticleEntry { path }>`; `ResolvedManifest::load` resolves every `particles.*.path` relative to the manifest directory and reports `MissingParticleFile` when the sibling does not exist; `merge` treats duplicate particle IDs as fatal through the existing `DuplicateId` error. Three new unit tests cover the resolve + merge paths.
- **Particle components (`tungsten_core::components`):** `ParticleEmitter { config: AssetId<ParticleConfig>, seed_override: Option<u64> }`, `ParticleEmitterState { config_snapshot: Option<Arc<ParticleConfig>>, rng: Pcg32, elapsed, continuous_accum, pulse_timer, pulses_fired, active_count, drained, first_tick_done, drain_reported }`, and `Particle { config: Arc<ParticleConfig>, emitter: Option<Entity>, age, lifetime, velocity, angular_velocity, start_scale, base_rgba }`. `ParticleEmitter::new(id)` is the default constructor; `ParticleEmitterState::default()` constructs without a snapshot (resolved on first tick).
- **Particle systems (`tungsten::particles`):** three frame-order-hardened systems — `particle_count_refresh_system` walks live particles and rewrites each emitter's `active_count` + drain latch; `particle_emit_system` resolves the `Arc` snapshot on first tick (from `ParticleConfigRegistry` + `WorldRngSeed`), plans one frame's emission through `plan_emission` (Burst uses a one-shot latch, Continuous uses a rate-hz accumulator, Pulse fires at most one pulse per tick from a `pulse_timer`), clips against per-emitter `max_alive` and global `ParticleBudget.global_cap`, and spawns each particle via `CommandBuffer` (`Particle + Transform + Sprite + Visibility`); `particle_tick_system` ages every particle, integrates `(velocity + gravity * dt) * exp(-drag_per_sec * dt)`, samples `scale_over_life` / `color_over_life` / `alpha_over_life`, applies CPU-side `Premultiplied` RGB premultiply when requested, and despawns age-outs through the command buffer. Events: `ParticleBurstEmitted { emitter, count }` fires on every Burst/Pulse discrete spawn; `ParticleSystemDrained { emitter }` fires exactly once when a drained emitter's `active_count` reaches zero (latched via `drain_reported`). A `spawn_particle_via(world, buf, cfg, origin, rng)` helper is exposed for external callers.
- **Asset loader + hot reload (`tungsten::asset_loader`):** new `load_particles(manifest, world)` parses every `ResolvedParticle`, validates each config's `sprite` against the live `AssetRegistry`, and registers into `ParticleConfigRegistry`. `reload_particle(id, path, world)` parses, validates, and calls `ParticleConfigRegistry::replace`; parse errors and unknown-sprite references warn and retain the previous config (last-known-good per `D-031`). `load_all` runs particle loading after sprite loading so cross-references are verifiable. `HotReloadWatcher` `.json` dispatch tries `AnimationRegistry` first, then `ParticleConfigRegistry`.
- **App wiring (`tungsten::app`):** `App::new` inserts `ParticleConfigRegistry`, `ParticleActive`, `ParticleBudget`, and `WorldRngSeed` resources plus the `ParticleBurstEmitted` / `ParticleSystemDrained` event queues; the frame loop runs `particle_count_refresh_system → particle_emit_system → particle_tick_system` immediately after user systems and before the `CommandBuffer` flush, so spawned particles are visible to the extract path in the same frame.
- **Integration tests (`crates/tungsten/tests/particles.rs`):** six headless tests — `burst_once_emits_exactly_count_then_drains`, `continuous_rate_matches_expected_count` (60 ticks @ 100 Hz → 99–101 particles), `pulse_emits_fixed_pulses_then_drains` (exactly 3 pulses), `per_emitter_max_alive_clips_emissions` (1000 requested, 16 `max_alive` → 16), `global_budget_cap_clips_across_emitters` (cap 10, two emitters → ≤ 10), `hot_reload_snapshot_preserves_live_particles` (`Arc::as_ptr` unchanged after `replace`).
- **Particle tick bench (`crates/tungsten/benches/particle_tick.rs`):** `particle_tick_5k` measures ~657 µs per frame for 5000 live particles with gravity, drag, `scale_over_life`, `color_over_life`, and `alpha_over_life` all active (Ryzen 7 + RADV, release profile).
- **Platformer black-hole emitter:** `examples/01_platformer/assets/particles/black_hole.json` ships a `continuous { rate_hz: 160 }` emitter with premultiplied blend, `0.4–1.1 s` lifetime, `40–140` radial speed, `drag_per_sec = 1.2`, `±6 rad/s` angular velocity, `0.6–1.2` start scale, `scale_over_life` `[0→0, 0.15→1, 1→0]`, purple-to-magenta `color_over_life`, and matching `alpha_over_life`. A new `ex10_spark` 8×8 radial-falloff sprite backs the emitter. `spawn_black_hole_system` attaches `ParticleEmitter + ParticleEmitterState + Transform` to each black hole entity and updates the `Transform` position on every drag frame.
- **Decision records + archived plan:** `DECISIONS.md` gains `D-049` (in-tree PCG32 + SplitMix64), `D-050` (Arc snapshot semantics on hot reload), and `D-051` (entity-per-particle, no pool); `docs/DECISION_INDEX.md` carries the new takeaways; `docs/LLM_INDEX.md` adds a Particles subsystem row and a "Tune or add particle effects" task row; the implementation plan is archived at `docs/plans/archive/phase3-milestone-23-particle-system.md`.

### Changed

- Workspace version bumped to `0.20.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.20.0` / M23 release line and the next-step `M24` planning state; `DESIGN.md` Non-Commitments drops the stale "Texture atlases / sprite sheet packing" bullet now covered by M22.

## [0.19.0] - 2026-04-20

Summary: Phase 3 Milestone 22 — sprite atlases (shelf-next-fit packer, per-filter pages, half-texel UV inset, renderer-minted texture handles, rebuild-on-growth hot reload), and release-line alignment.

### Added

- **CPU-side atlas packer (`tungsten_core::assets::atlas`):** new module `crates/tungsten-core/src/assets/atlas.rs` ships `UvRect { min, max }` (plus the `UvRect::FULL` constant), `PackInput { id, width, height }`, `PackedSprite { id, page, x, y, width, height }`, `AtlasPage { width, height }`, and `PackResult { pages, sprites }`. `pack_shelf(inputs, max_dim, padding)` sorts a stable copy by `(height desc, width desc, id asc)`, fills shelves inside the current page until either axis overflows `max_dim`, then opens a new power-of-two-sized page. Panics when a single sprite exceeds `max_dim - 2 * padding` on either axis. Unit tests cover empty input, single-sprite origin placement, shared-page packing, two-page overflow, oversize panic, and determinism.
- **Per-filter atlas registry (`tungsten::asset_loader`):** new `AtlasRegistry { nearest_pages, linear_pages, packed: HashMap<String, PackedSprite> }` resource partitions sprites by `FilterMode` and records each sprite's packed rect. `build_atlas_for_filter` packs one filter class, uploads every page through `Renderer::upload_texture`, and registers each sprite with a half-texel-inset `UvRect` so bilinear sampling cannot reach the transparent padding column. `load_sprites` logs `Packed N sprites → M atlas pages (X nearest + Y linear)` for every manifest load.
- **Renderer-minted texture handles (`tungsten_render::sprite`):** `SpritePipeline::allocate_texture_handle()` returns a monotonically increasing `TextureHandle`; `drop_texture(handle)` removes the `GpuTexture` pool entry on rebuild shrink; `upload_texture(handle, bytes, w, h, filter)` now takes the filter up front so the bind group bakes in the sampler and `SpritePipeline::draw` no longer switches samplers per batch. `write_subtexture(queue, handle, rgba, x, y, w, h)` supports in-place sub-region uploads. `Renderer::max_2d_texture_dimension()` exposes the backend's max page dimension clamped to 8192. A `SpriteInstance::whole()` constructor builds an instance with `uv_min = [0.0, 0.0]` / `uv_size = [1.0, 1.0]` for callers that want full-texture behaviour.
- **Per-instance UV slice on the GPU:** `SpriteInstance` gains `uv_min: [f32; 2]` at `@location(6)` and `uv_size: [f32; 2]` at `@location(7)`. `sprite.wgsl` computes `out.tex_coord = instance.inst_uv_min + vertex.uv * instance.inst_uv_size`, so one atlas texture serves many sprites within a single bind group.
- **Hot-reload rebuild-on-growth (`tungsten::asset_loader`):** `reload_sprite` takes the in-place fast path via `write_subtexture` when the new decode is `≤` the packed rect on both axes (leaving `SpriteAsset.uv` untouched); otherwise `rebuild_atlas_for_filter` re-reads every sprite in the affected filter class from disk, repacks, reuses old `TextureHandle`s 1:1, drops excess, and writes the new atlas bindings through `AssetRegistry::update_sprite_entry`. Decode errors anywhere in the rebuild partition abandon the rebuild and keep the previous atlas (last-known-good per `D-031`). Manifest additions run through the same rebuild path with placeholder (`atlas = TextureHandle(0)`, `uv = UvRect::FULL`) entries so the orphan case is bounded.
- **Atlas integration test:** `crates/tungsten/tests/atlas_integration.rs` asserts that two sprites sharing an atlas collapse to a single `SpriteBatch` with distinct `uv_min` slices, and that three sprites across two atlases produce two batches sized `2` and `1` through the default extract path.
- **Atlas pack bench baseline:** `atlas_pack_startup_200` ≈ `7.45 µs` on AMD Radeon 660M / RADV Vulkan — first recorded number on this machine; future runs guard the `≤20%` regression rule from `docs/plans/Phase3.md`.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-048` covering the six coupled M22 choices (shelf packer, per-filter pages, half-texel inset, rebuild-on-growth hot reload, renderer handle authority, manifest-addition path); the implementation plan is archived at `docs/plans/archive/phase3-milestone-22-sprite-atlases.md`; `docs/DECISION_INDEX.md` and `docs/LLM_INDEX.md` carry the new subsystem/task rows.

### Changed

- Workspace version bumped to `0.19.0`.
- **`AssetRegistry::register_sprite` signature (`tungsten_core::assets::registry`):** now takes `(id, filter, width, height, path, atlas: TextureHandle, uv: UvRect)` — the registry no longer mints handles, and every sprite carries its packed UV slice. `SpriteAsset` gains `atlas: TextureHandle` and `uv: UvRect` alongside the existing `filter / width / height / path`. `update_sprite_entry(id, atlas, uv, width, height)` replaces the M9-era `update_sprite_dimensions` path used by hot reload.
- **Batch key (`tungsten::sprite_extract`):** the default extract now groups by `(asset.atlas.0, asset.filter)` and emits one `SpriteInstance` per entity with `uv_min = asset.uv.min` and `uv_size = asset.uv.max - asset.uv.min`. Sprites that share an atlas page collapse into a single batch; `SpritePipeline::draw` warns and skips when the pool entry's filter disagrees with the batch's filter.
- **`SpriteInstance` size grew from 24 B to 40 B (+66%)** to carry the per-instance UV slice. `sprite_extract_batch_build_2k` measures pre-M22 ≈ `6.32 µs` vs. post-M22 ≈ `7.72 µs` (+22%). The bench pre-allocates 10 fixed batches and does not exercise batch collapse, so the synthetic regression is stride-dominated; the engineered-in wins (fewer bind-group switches, fewer live textures) land in the real-scene draw path.
- Image diff (Pillow per-pixel RGBA, tolerance `0`): `01_platformer`, `02_sprite_stress`, and `03_scene_state` are pixel-identical against the pre-M22 HEAD capture.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.19.0` / M22 release line and the next-step `M23` planning state.

## [0.18.0] - 2026-04-20

Summary: Phase 3 Milestone 21 — debug tooling (geometric overlays, text inspector, screenshot + image-diff), and release-line alignment.

### Added

- **Core debug primitives (`tungsten_core`):** `DebugDraw`, `DebugShape::{Aabb, Circle, Line}`, `DebugCommand`, and `DEFAULT_CIRCLE_SEGMENTS` ship as pure POD in `crates/tungsten-core/src/debug_draw.rs`. `Inspectable` (`crates/tungsten-core/src/inspect.rs`) is a trait with blanket impls for `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, and `Sprite`. `KeyCode::{F1, F2, F3}` are new variants and round-trip through the input bridge and `key_serde` tables.
- **Engine overlays (`tungsten`):** `PhysicsDebugOverlay` (`F1`), `SystemTimingOverlay` (`F2`, EWMA-smoothed per-system timings sourced from `FrameTimings`), and `InspectorState` (`F3`, LMB pick + registered `Inspectable` row renderers) ship as independent action-toggled resources; `App::register_inspectable::<T: Inspectable>(label)` wires new component types into the inspector.
- **Action-map defaults + `input.json` entries:** `engine_toggle_physics_debug` (`F1`), `engine_toggle_systems_overlay` (`F2`), `engine_toggle_inspector` (`F3`) merge into user input maps via `ActionMap::merged_with_defaults`.
- **Render seam (`tungsten_render`):** new `DebugLinePipeline` + `DebugLineInstance` draws oriented lines and circle polylines and borrows `QuadPipeline`'s camera bind group layout so only one `view_proj` uniform ships on the GPU. `Renderer::render_frame_full[_timed]` gain `debug_quads: &[QuadInstance]` and `debug_lines: &[DebugLineInstance]` parameters; AABB edges expand into four thin `QuadInstance`s drawn through the existing pipeline. `QuadPipeline::camera_bind_group_layout()` / `camera_bind_group()` are now public accessors.
- **Screenshot + visual-regression helpers (`tungsten_render`):** `Renderer::capture_frame(path)` renders into an offscreen `RENDER_ATTACHMENT | COPY_SRC` texture and encodes the readback via `image::save_buffer`; `image_diff::compare_png(lhs, rhs, tolerance)` returns a `DiffReport { width, height, max_delta, mean_delta, pixels_above_tolerance }`. Capture is armed via `TUNGSTEN_CAPTURE_FRAME=<n>` plus optional `TUNGSTEN_CAPTURE_PATH` / `TUNGSTEN_CAPTURE_RESOLUTION=<WxH>` and is off by default. An opt-in integration test (`examples/02_sprite_stress/tests/visual_regression.rs`) gated on `TUNGSTEN_VISUAL_REGRESSION=1` shells out to `example-02-sprite-stress` and diffs against the committed baseline.
- **GPU debug groups + explicit wgpu labels:** the encoder wraps each frame in `push_debug_group("tungsten_frame")`; the main pass opens named groups for `quads`, `sprites`, `debug_quads`, `debug_lines`, and `text`. Always-on, no feature gate; RenderDoc captures are self-describing.
- **Perf-capture scaffolding:** `examples/02_sprite_stress` parses `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` to flip overlay `.enabled` flags before `App::run`, so the overlays-on vs. overlays-off capture pair is driven purely from the command line. A perf-run skeleton lives at `perf-runs/M21-debug-tooling/README.md`.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-047`; the implementation plan is archived at `docs/plans/archive/phase3-milestone-21-debug-tooling.md`; `docs/DECISION_INDEX.md` and `docs/LLM_INDEX.md` carry the new subsystem/task rows.

### Changed

- Workspace version bumped to `0.18.0`.
- `App::new` now inserts `DebugDraw`, `PhysicsDebugOverlay`, `SystemTimingOverlay`, and `InspectorState` world resources; engine toggle systems (`__physics_debug_toggle`, `__systems_overlay_toggle`, `__inspector_toggle`, `__inspector_pick`) register at the head of the engine chain so they observe `just_pressed` before user systems. `physics_debug_emit_system` runs at the start of the extract stage before `DebugDraw::drain`, then commands are split into `Vec<QuadInstance>` (AABB edges) + `Vec<DebugLineInstance>` (lines / circle polylines) and passed through to `Renderer::render_frame_full[_timed]` alongside the existing quad / sprite / text channels.
- `example-01-platformer`'s header `Controls:` block documents the new `F1` / `F2` / `F3` overlays.
- `tungsten-render` gains `image = { workspace = true }` as a direct dependency (the workspace dep already existed for `asset_loader`); no new workspace dependency.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.18.0` / M21 release line and the next-step `M22` planning state.

## [0.17.0] - 2026-04-20

Summary: Phase 3 Milestone 20 — scene / state dispatcher, `scene.json` data-driven spawn path, and release-line alignment.

### Added

- **Scene / state system (`tungsten::state`):** `StateStack`, the `GameState` trait, `StateContext`, `StateId`, and a `SceneEntity { state_id }` marker now ship in the umbrella crate. A single engine-owned `state_dispatcher_system` drains deferred `request_push` / `request_pop` / `request_replace` requests each frame, fires the `on_pause` / `on_enter` / `on_exit` / `on_resume` matrix, auto-despawns scene-owned entities through `CommandBuffer` on exit, and mirrors the active state id into `HudActiveState` so the M18 `state` HUD row keeps rendering.
- **Scene data model (`tungsten_core::assets::scene`):** `SceneData`, `SceneEntry`, `SceneTransform`, `SceneSprite`, and `SceneError` define a minimal JSON schema that reuses the M15 `Transform` / `Sprite` / `Visibility` / `Tag` components. `SceneData::load` parses a `scene.json` file; `asset_loader::load_scene` and `asset_loader::spawn_scene` wrap the load + `CommandBuffer` spawn path so scenes land at the canonical frame boundary.
- **State-transition action defaults:** `ActionMap::default_map()` now ships `state_start` (`Enter`), `state_pause` (`KeyP`), and `state_back` (`Backspace`) so examples drive transitions without an edited `input.json`. `KeyCode::Backspace` and `KeyCode::KeyP` are new variants on the core-owned keyboard enum (and route through the input bridge + serde tables).
- **New example — `example-03-scene-state`:** end-to-end demo of the `MainMenu → Gameplay → Pause → Gameplay` flow. Gameplay entities come from `scene.json` via `spawn_scene` (25-entity constellation: pulsing hub + three counter-rotating orbital rings); Pause overlays Gameplay without tearing the scene down; the HUD `state` row mirrors the active state id.
- **Decision record + detailed plan:** `DECISIONS.md` now includes `D-046`; the implementation plan is archived at `docs/plans/archive/phase3-milestone-20-scene-state-system.md`; `docs/DECISION_INDEX.md` and `docs/LLM_INDEX.md` reflect the new subsystem.

### Changed

- Workspace version bumped to `0.17.0`.
- `App::new` now inserts `StateStack` and `HudActiveState` as world resources and registers `__state_dispatcher` immediately after `__display_input` so state transitions fire before user systems observe this frame's input.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, `docs/LLM_INDEX.md`, `docs/DECISION_INDEX.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.17.0` / M20 release line and the next-step `M21` planning state.
- Release QA pass completed locally: `cargo fmt --all --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `bash scripts/test-perf-capture.sh`, and `WGPU_BACKEND=vulkan bash scripts/smoke-examples.sh` all passed (4/4 examples).

## [0.16.0] - 2026-04-19

Summary: Phase 3 Milestone 19 — input mapping, mouse support, runtime rebind persistence, and release-line alignment.

### Added

- **Core action map (`tungsten_core::input`):** `ActionMap`, `Binding`, and `ActionMapError` now ship as the core-owned boolean input binding surface. Actions resolve through keys, mouse buttons, or discrete wheel directions and are re-exported from both `tungsten_core` and `tungsten`.
- **Workspace-root `input.json`:** default bindings now live in a checked-in action-map file with hot reload, missing-file fallback, startup-fatal invalid JSON handling, and a runtime persist path that writes atomically back to disk.
- **Mouse input surface:** `InputState` now exposes current cursor position, per-frame cursor delta, wheel line delta, and wheel pixel delta; extra mouse buttons serialize as `button4`, `button5`, etc.
- **Engine-owned actions:** HUD toggle, vsync toggle, fullscreen toggle, and exit now route through action names (`engine_toggle_hud`, `engine_toggle_vsync`, `engine_toggle_fullscreen`, `engine_exit`) instead of hardcoded key branches.
- **Action-map micro-bench:** `crates/tungsten-core/benches/action_map_bench.rs` now records per-call keyboard and mouse dispatch costs. Current local medians: `action_map_is_pressed_key` ~`51.051 ns`, `action_map_just_pressed_key` ~`34.912 ns`, `action_map_is_pressed_mouse_button` ~`32.267 ns`, `action_map_just_pressed_scroll` ~`35.365 ns`.

### Changed

- Workspace version bumped to `0.16.0`.
- `example-01-platformer` now consumes gameplay input exclusively through action lookups, demonstrates mouse-button bindings (`LMB` jump, `RMB` music toggle, `MMB` stop-all) plus scroll zoom, and renders live cursor / wheel telemetry in the on-screen text.
- `docs/plans/Phase3.md`, `AGENTS.md`, `CLAUDE.md`, `README.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, and `docs/DECISION_INDEX.md` now reflect the shipped M19 release line; the detailed plan moved to `docs/plans/archive/phase3-milestone-19-input-mapping.md`.
- Release QA pass completed locally: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo fmt --all --check`, `./scripts/smoke-examples.sh`, and `cargo bench -p tungsten-core --bench action_map_bench` all passed.

### Fixed

- **Reserved-key drift:** `F4`, `F9`, `F11`, and `Escape` now share the same action-map pipeline as gameplay bindings, removing the last hardcoded key checks from the shipped engine flow.
- **Mouse extra-button coverage:** the input bridge now preserves `winit` back/forward mouse buttons as rebindable extra-button IDs instead of collapsing them into an unusable fallback.
- **Action-map persistence coverage:** runtime rebinds can now round-trip back to `input.json` without discarding unrelated top-level fields when the existing file layout can be safely patched.

## [0.15.0] - 2026-04-18

Summary: Phase 3 Milestone 18 — runtime telemetry HUD, diagnostic counters, and release-line alignment.

### Added

- **Runtime telemetry HUD (`tungsten::debug_hud`):** `DebugHud`, `HudCorner`, `HudRow`, `HudActiveState`, `hud_toggle_system`, and built-in/custom row providers now ship in the umbrella crate. Built-in rows cover FPS/frame ms, camera state, display state, tagged player position/speed, live entity + sprite counts, and top-N slowest systems.
- **Diagnostic counters:** `tungsten::RenderCounts` mirrors per-frame entity and sprite counts into the `World`, while `tungsten_core::World::entity_count()` exposes the live ECS entity count in O(1).
- **HUD toggle + example wiring:** `KeyCode::F4` is now plumbed through the input bridge, `example-01-platformer` tags the player entity for HUD lookup, and the controls text documents the new developer HUD toggle.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-044`, the detailed M18 rollout plan now lives at `docs/plans/archive/phase3-milestone-18-runtime-telemetry-hud.md`, and the capture summary lives at `perf-runs/M18-hud/README.md`.

### Changed

- Workspace version bumped to `0.15.0`.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.15.0` / M18 release line and the next-step `M19` planning state.
- The shipped HUD defaults now favor readability in busy scenes: larger text, taller line spacing, and a throttled text refresh interval while the EWMA timing row keeps updating from frame telemetry.
- Release QA pass completed locally: `cargo fmt --all`, `cargo build --workspace`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `bash scripts/test-perf-capture.sh`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --telemetry-only`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` all passed.

### Fixed

- **Perf-capture README quoting:** `scripts/perf-capture.sh` now escapes the literal `` `STRESS_SCENE` `` / `` `STRESS_COUNT` `` notes in its generated README so shell command substitution cannot corrupt the notes section.
- **Sprite-stress lint noise:** `example-02-sprite-stress` now uses `usize::div_ceil` for row count calculation and gates the `leader` field's dead-code allowance to non-test builds.

## [0.14.0] - 2026-04-17

Summary: Phase 3 Milestone 17 — display state/config, frame-boundary runtime display changes, and release-line alignment.

### Added

- **Display model (`tungsten_core::display`):** `DisplayState`, `DisplayConfig`, `DisplayMode`, `ScaleMode`, `Resolution`, and `DisplayValidationError` now ship as the core-owned display data/validation surface. The checked-in `tungsten.json` now includes a canonical `display` block while legacy `window.*` / `render.*` display inputs remain valid for M17 compatibility.
- **Single runtime display request path:** `tungsten::request_display_settings(&mut World, DisplayState)` validates requests up front, queues one pending change, and lets `App` apply fullscreen, resize, surface-pacing, and frame-cap deltas only at the top of `RedrawRequested`.
- **Display telemetry:** `tungsten::DisplayTelemetry` mirrors authoritative resolution, display mode, vsync intent, lower-case applied present-mode label, max-frame-latency hint, scale mode, and frame-rate cap back into the `World`.
- **Runtime display demo wiring:** `example-01-platformer` now exercises the runtime path directly: `F11` toggles borderless fullscreen and `F9` toggles `vsync` while re-running auto present-mode selection.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-043` for the single-file display config shape and frame-boundary apply rule, and the detailed M17 rollout plan now lives at `docs/plans/archive/phase3-milestone-17-display-state-config.md`.

### Changed

- Workspace version bumped to `0.14.0`.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, `docs/plans/Phase3.md`, and `docs/perf/profiling-workflow.md` now reflect the shipped `0.14.0` / M17 release line and the `display.*` config surface.
- `example-02-sprite-stress` and `example-03-component-sprites` now express startup sizing through `config.display.resolution` instead of post-load legacy `config.window.*` mutations that are shadowed by the checked-in `display` block.
- `scripts/perf-capture.sh` help text now describes pacing overrides without pointing at superseded pre-M17 config wording.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, and `./scripts/smoke-examples.sh` all passed.

### Fixed

- **Release metadata drift:** top-level docs, planning docs, and changelog entries now agree on branch `0.14`, workspace `0.14.0`, and M17 shipped state.
- **Example display override drift:** sprite-stress and component-sprites no longer rely on legacy startup window overrides that do not win over the resolved `display` block after `Config::load()`.
- **Config error masking:** `example-03-component-sprites` now propagates `Config::load` failures instead of silently falling back to defaults.

## [0.13.0] - 2026-04-17

Summary: Phase 3 Milestone 16 — shared camera module and authoritative camera flow.

### Added

- **Shared camera data model (`tungsten_core::camera`):** `CameraState { position, zoom, rotation }`, `CameraController`, `CameraMode`, and `CameraBounds` centralize camera ownership and follow behavior. The default camera still matches the pre-M10 top-left pixel-ortho matrix at `(0, 0)` / `zoom = 1.0`.
- **Shared camera update system:** `tungsten::camera_update_system` reads `CameraController`, `DeltaTime`, `WindowSize`, and a followed entity `Transform`, then writes the authoritative `CameraState` for the frame.
- **Controller features:** follow/free/scripted modes, dead-zone sizing, smoothing, bounds clamp, zoom multiplier, and deterministic shake fields (`shake_amplitude`, `shake_frequency_hz`, `shake_phase`).
- **Camera test coverage:** `crates/tungsten/tests/camera.rs` covers follow, bounds clamp, scripted zoom scaling, pre-M10 zero-rotation matrix parity, zoom-multiplier changes, and deterministic shake; `tungsten-core::camera` unit tests cover bounds math plus rotated visible-AABB over-coverage.

### Changed

- Workspace version bumped to `0.13.0`.
- `App::new` now inserts `CameraState` and `CameraController` resources by default alongside the existing runtime resources.
- `example-01-platformer` now configures player follow and map-bounds clamp through `CameraController`, recomputes base zoom from window height each frame, and runs `camera_update_system` after `sync_position_to_transform`.
- `extract_tilemaps` now culls through `CameraState::visible_world_aabb(...)`, so tile visibility follows the shared camera state and still over-covers safely when camera rotation is non-zero.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.13.0` / M16 release line.

### Fixed

- **Base-camera stability:** shared camera bookkeeping now avoids compounding `zoom_multiplier` or shake offsets when gameplay rewrites the base camera pose/zoom each frame before `camera_update_system` runs.

## [0.12.0] - 2026-04-16

Summary: Phase 3 Milestone 15 — canonical render components (`Transform`, `Sprite`, `Visibility`, `Tag`) and a default sprite-extract path that removes the need for per-example extract closures in the common case.

### Added

- **Render components (`tungsten_core::components`):** `Transform { position, rotation, scale }`, `Sprite { asset_id, color, z_order }`, `Visibility { visible }`, and `Tag { name }` ship as the baseline gameplay/render component types. Re-exported from `tungsten_core` for convenience.
- **One-way physics sync:** `tungsten_core::sync_position_to_transform` copies physics `Position.0` into `Transform.position` for every entity that carries both. Explicit, opt-in registration; there is no reverse sync (`D-033`).
- **Default sprite extract:** `tungsten::extract_sprites_default` iterates `Transform + Sprite + Visibility`, resolves each sprite against `AssetRegistry`, and builds per-`(texture, filter)` `SpriteBatch`es stably sorted by `z_order`. Installed automatically by `App::run` when no custom sprite extract is set. `Visibility` is required — no implicit fallback (`D-042`).
- **Per-instance rotation + tint on the GPU:** `SpriteInstance` now carries `rotation: f32` (radians, CCW, around the quad centre) and `color: [u8; 4]` (RGBA `Unorm8x4`). The WGSL pipeline rotates around centre and multiplies the sampled texel by the tint.
- **`KeyCode::KeyV`:** added for the new example's `Visibility` toggle demo.
- **Example `examples/03_component_sprites`:** renders rotating, pulsing, tint-cycling, and z-stacked sprites through the default extract path with no `set_extract_sprites` call. `V` toggles visibility on a tagged entity.
- **Bench `sprite_components_query3_2k`:** new ecs_bench entry that regression-tests `query3::<Transform, Sprite, Visibility>` over 2 000 matching entities spread across five archetypes.
- **DECISIONS.md D-042:** records the four coupled M15 choices — component ownership in `tungsten-core`, the one-way physics sync, the `SpriteInstance` layout change, and the `Visibility`-required default extract.

### Changed

- Workspace version bumped to `0.12.0`.
- `SpriteInstance` size grew from 16 bytes to 24 bytes; all in-tree call sites (`tilemap_extract`, `01_platformer`, `02_sprite_stress`, render bench) migrated in the same commit with no backwards-compat shim.
- `sprite.wgsl` now applies centre-origin rotation. When `rotation == 0.0`, `world_pos` reduces algebraically to the pre-M15 top-left-anchored expression so existing sprites render unchanged.
- `FilterMode` derives `Hash` so `(TextureHandle, FilterMode)` can key batch maps.
- `DESIGN.md`, `docs/LLM_INDEX.md`, and `docs/plans/Phase3.md` updated to reference the new component surface and default extract path.
- Release QA pass completed locally: `cargo fmt --all -- --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `bash scripts/test-perf-capture.sh`, `./scripts/smoke-examples.sh`, `cargo bench -p tungsten-core --bench ecs_bench -- sprite_components_query3_2k`, and `cargo bench -p tungsten-render --bench render_bench -- sprite_extract_batch_build_2k` all passed. Current local bench medians: `sprite_components_query3_2k` ~`711 ns`, `sprite_extract_batch_build_2k` ~`5.79 us`.

### Fixed

- `SpritePipeline::draw` now advances its packed instance-buffer cursor even when a batch is skipped for a missing GPU texture, so later batches keep the correct instance slice instead of rendering misaligned sprite data.

## [0.11.0] - 2026-04-16

Summary: Phase 3 Milestone 14 — typed event queues and fixed-frame event flush.

### Added

- **Typed event buffering:** `tungsten_core::EventQueue<T>` adds a reusable two-window event resource with `send`, `iter`, `iter_current`, `flush`, `len`, `is_empty`, and `Default`.
- **App-level event registration:** `App::register_event::<T>()` inserts an `EventQueue<T>` resource and schedules its per-frame flush alongside the existing command-buffer lifecycle.
- **Event-queue benchmark:** `event_queue_flush_10_types` added to the `tungsten-core` ECS Criterion suite; current local result is ~2.44 us for 10 queue types with 100 events each.
- **DECISIONS.md D-040:** Records the two-window event design, frame-boundary flush order, startup-only registration contract, and initial benchmark result.

### Changed

- Workspace version bumped to `0.11.0`.
- `App` frame order is now explicit: run systems, flush command buffers, flush event queues, then hot reload, extract, and render.
- Physics collision signaling migrated from the bespoke `CollisionEvents` resource to `EventQueue<CollisionEvent>`.
- `example-01-platformer` now consumes collision contacts through `EventQueue<CollisionEvent>` for grounded detection and HUD contact counts.
- `README.md`, `DESIGN.md`, `CLAUDE.md`, `AGENTS.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.11.0` release line and Phase 3 M14 completion.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, `./scripts/smoke-examples.sh`, and `cargo bench -p tungsten-core --bench ecs_bench -- event_queue_flush_10_types` all passed.

### Fixed

- **Release metadata drift:** top-level status docs and workspace version metadata now agree on the active `0.11.0` release line instead of mixing `0.10.0` and M14-complete language.

## [0.10.0] - 2026-04-15

Summary: Phase 3 Milestone 13 — command buffers and fixed-frame structural mutation flush.

### Added

- **Deferred ECS mutation path:** `tungsten_core::CommandBuffer` and `PendingEntity` provide queued `spawn`, `despawn`, `insert`, `insert_pending`, and `remove_component` operations without requiring structural mutation during system iteration.
- **`World::flush`:** New two-pass flush API resolves pending spawns first, then replays queued mutations in registration order with dead-entity guards for late inserts/despawns.
- **Flush telemetry:** `tungsten::FrameTimings` now records `flush_ms`, and `App` logs flush timing in `TUNGSTEN_PERF_LOG` output.
- **M13 ECS coverage:** New unit/integration tests cover command buffer queueing, pending-entity resolution, command ordering, dead-entity guards, and empty-buffer no-op behavior.
- **Command-buffer benchmark:** `command_buffer_flush_1k_spawns` added to `tungsten-core` Criterion benches; current local result is ~252 us for 1k spawns plus 2k deferred inserts.
- **Frame-pacing config knobs:** `render.present_mode` and `render.max_frame_latency` are now typed `tungsten.json` fields backed by `PresentModeConfig`.
- **Perf-capture parser regression test:** `scripts/test-perf-capture.sh` exercises metadata parsing plus nearest-rank `p50`/`p95`/`p99` calculations against a synthetic telemetry log.
- **DECISIONS.md D-039:** Records the resource-based command-buffer delivery model, two-pass flush design, and initial benchmark numbers.

### Changed

- Workspace version bumped to `0.10.0`.
- `App` now inserts a fresh `CommandBuffer` resource on startup and drains/replaces it once per frame between system execution and hot reload/extract.
- `tungsten-render` now resolves present mode through explicit precedence rules: concrete `render.present_mode` overrides `window.vsync`, unsupported concrete modes fail fast, and `render.max_frame_latency = 0` is rejected at renderer init.
- `scripts/perf-capture.sh` now records renderer backend/adapter/present-mode metadata as separate README rows and reports post-warm-up `p50`/`p95`/`p99` for total and acquire timing.
- `docs/perf/profiling-workflow.md`, `README.md`, `DESIGN.md`, `CLAUDE.md`, `AGENTS.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.10.0` release line instead of a pre-release state.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, `./scripts/smoke-examples.sh`, `cargo clippy --workspace --all-targets`, `bash scripts/test-perf-capture.sh`, the new `command_buffer_flush_1k_spawns` bench, and steady-state ECS regression benches all passed.

### Fixed

- **Perf metadata wording:** release docs now describe `max_frame_latency` as the requested `wgpu` hint rather than a backend-confirmed effective queue depth.
- **Sprite-stress capture note:** example docs now describe the checked-in default auto no-vsync path without implying that the example hard-overrides `render.present_mode` from `tungsten.json`.

## [0.9.0] - 2026-04-15

Summary: Phase 3 Milestone 12 — performance baseline, telemetry, and profiling harness.

### Added

- **CPU frame telemetry:** `tungsten::FrameTimings` resource now records per-frame stage timings (`update`, `extract`, `render`, `audio`, `hot_reload`, `total`) plus a per-system timing breakdown. The render stage is also split into `render_acquire`, `render_encode`, and `render_submit_present` for finer profiling. `App::add_system_named()` allows stable system labels for diagnostics while preserving existing unnamed-system registration.
- **GPU timing diagnostics:** `tungsten_render::GpuFrameTimings` and `Renderer::render_frame_full_timed()` add an opt-in timestamp-query path for render-pass GPU timing. Backend, adapter, chosen present mode, and max-frame-latency metadata are exposed for downstream tooling and HUD work.
- **Benchmark suite expansion:** `tungsten-core` now ships `physics_bench` alongside the existing ECS benchmarks, and `tungsten-render` now has a Criterion-backed `render_bench` target for CPU-side render-data construction costs.
- **`example-02-sprite-stress`:** Canonical 2k-sprite stress scene for repeatable perf captures. Uses a startup-uploaded placeholder texture, named systems, and periodic telemetry logging.
- **Profiling workflow docs:** `docs/perf/profiling-workflow.md` documents canonical capture rules, backend overrides, manual profiling commands, RenderDoc workflow, and perf budgets.
- **Automated capture script:** `scripts/perf-capture.sh` builds a release binary with frame pointers, captures engine telemetry and GPU timing logs, and integrates optional `cargo flamegraph`, `perf stat`, and `perf record` runs into one timestamped output directory.
- **`perf-runs/.gitkeep`:** Placeholder directory for local machine-specific baseline captures.
- **DECISIONS.md D-037 / D-038:** Render-side Criterion rationale and the inline `Instant`-based telemetry decision are now recorded.

### Changed

- Workspace version bumped to `0.9.0`.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/LLM_INDEX.md` now reflect that Phase 3 M12 is complete and point to the new perf tooling/docs.
- `scripts/perf-capture.sh` bounds flamegraph capture with `TUNGSTEN_SMOKE_FRAMES`, matching the rest of the scripted capture flow.
- Engine defaults now ship with `vsync = false`, and the renderer prefers lower-latency no-vsync present modes plus a 1-frame latency hint when the backend supports them.
- Release QA pass completed locally: `cargo test --workspace`, `cargo clippy --workspace --all-targets`, all three benchmark targets, `./scripts/smoke-examples.sh`, and short release perf sanity runs all passed.

## [0.8.0-alpha] - 2026-04-15

Summary: Phase 2 integration — comprehensive platformer demo, example consolidation, and Phase 3 planning.

### Added

- **`example-01-platformer` (comprehensive demo):** Single example that exercises every Phase 2 engine feature in one scene: ECS, physics (AABB player + bouncing circles + tilemap collision), sprites, walk-cycle animation, audio (one-shot SFX, looping music, volume levels), HUD text, camera follow with zoom (= / −), keyboard input, and hot reload. Supersedes and retires the ten separate milestone examples.
- **`KeyCode::Equal` / `KeyCode::Minus`:** New key code variants to support zoom-in / zoom-out input.
- **`docs/plans/Phase3.md`:** Execution plan for M13–M21: command buffers, event queues, transform/render components, input mapping, scene/state system, sprite atlases, debug tooling, particle system, and tween system.

### Changed

- Workspace version bumped to `0.8.0-alpha`.
- Previous milestone examples (`01_window` through `10_platformer`) removed; their feature coverage is consolidated into `01_platformer`.
- `PHASE2.md` archived to `docs/plans/archive/phase2.md`.

### Fixed

- **First-frame dt spike:** `App` now stamps `last_frame` after the startup callback completes rather than before. Asset-load time no longer registers as game time, preventing fast-moving physics bodies from tunneling through thin geometry on the very first frame.
- **Walk animation frame timing:** `walk_2` frame duration corrected from 1500 ms to 150 ms (copy-paste typo in the original JSON).

## [0.7.0-alpha] - 2026-04-14

Summary: Phase 2 Milestone 12 — Archetypal ECS rewrite.

### Added

- **Archetypal storage engine:** Replaced naive `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` with a proper archetype table. Components of the same type within an archetype are stored in a contiguous `TypedVec<T>` column. Query iteration is now cache-friendly across homogeneous entity sets.
- **Archetype graph:** Lazy-cached add/remove edges between archetypes. First transition builds the edge; subsequent transitions follow the cached pointer in O(1).
- **Generational entity IDs:** Entity handles now carry a generation counter. Stale handles to recycled slots are detected and rejected.
- **Multi-component queries:** `query2` / `query2_entities` / `query3` / `query3_entities` iterate over all archetypes that contain the requested component set, yielding contiguous slices per archetype.
- **Criterion benchmark suite:** Benchmarks on ≥10 000 entities with 3+ component types. Results: ~6× improvement on single-type queries; ~200× on multi-component queries vs. the M2 baseline.
- **DECISIONS.md D-036:** Decision to proceed with the rewrite (cites D-030 "skip if naive suffices"), storage design rationale, and benchmark results.

### Changed

- Workspace version bumped to `0.7.0-alpha`.
- All 10 existing examples compile and smoke-test clean without API changes — the `World` public surface is unchanged.
- PHASE2.md: M12 marked complete.

### Fixed

- **Sound path canonicalization:** `ResolvedManifest::load` now canonicalizes resolved sound asset paths, consistent with sprites, animations, fonts, and tilemaps.
- **Window creation error handling:** `App::resumed` now logs and calls `event_loop.exit()` on window creation failure instead of panicking — consistent with the existing renderer initialization failure path.
- **ECS clippy polish:** `Archetype::move_components_to` uses `entry().or_insert_with()` (avoids double lookup); `split_two_mut` parameter narrowed from `&mut Vec<Archetype>` to `&mut [Archetype]`.
- **Stale doc comment in tilemap extract:** comment updated to reflect that M11 ships as `physics_step` reading collision layers directly.

## [0.6.0-alpha] - 2026-04-14

Summary: Phase 2 Milestone 11 — 2D Physics.

### Added

- **`tungsten-core::physics` module:** Hand-rolled 2D collision subsystem. Exports `Position`, `Velocity`, `Collider`, `RigidBody`, `Shape { Aabb, Circle }`, `BodyKind { Static, Dynamic }`, plus `PhysicsConfig` and `CollisionEvents` resources. No external physics crate — `rapier2d`/`box2d`/`parry2d` all rejected (see D-033).
- **Narrow-phase shape tests:** `aabb_vs_aabb`, `circle_vs_circle`, `aabb_vs_circle` in `physics::collision`. Each returns `Option<Contact { normal, penetration }>` with a consistent convention: `normal` points from `a` into `b`'s free space (the direction `a` should move to escape). MTV on the axis of minimum overlap for AABB, closest-point test for AABB/circle, distance check for circle/circle. No SAT — AABB axes are world-aligned and circles need no SAT; the generalization is documented as a learning note.
- **Uniform-grid broad-phase:** `SpatialGrid` (`HashMap<IVec2, Vec<ProxyId>>`) keyed on `floor(pos / cell_size)`. Cell size is tunable via `PhysicsConfig::broadphase_cell_size` (default 32.0 px). Rebuilt from scratch each physics substep — no incremental state.
- **`physics_step` system:** Registered by the user via `app.add_system(physics_step)`. Per substep: integrate (`position += velocity * dt`, `velocity += gravity * dt`), gather entity proxies + transient tilemap-tile proxies, broad-phase, narrow-phase with MTV resolution split along inverse-mass ratio, velocity impulse `j = -(1+e)·(v·n)/Σ(1/m)`, collision events pushed into `CollisionEvents`. Substep count = `ceil(max_dynamic_speed * dt / min_half_extent)` capped at `PhysicsConfig::max_substeps` (default 8) — guards against tunneling without swept CCD.
- **Tilemap collision layers:** The step walks every `TilemapInstance` and emits one static AABB per non-negative tile on any `LayerKind::Collision` layer, fresh each substep. Hot-reloaded collision layers take effect on the next frame with zero extra machinery. `CollisionEvent.b = None` marks tile contacts.
- **`PhysicsConfig` resource:** `broadphase_cell_size`, `max_substeps`, `gravity` (default `Vec2::ZERO` so top-down games cost nothing). Auto-inserted by `App::new`; games override before `app.run()`.
- **`CollisionEvents` resource:** Per-frame event stream populated each step. Game code reads `events` for ground detection, triggers, damage, etc. `CollisionEvent { a: Entity, b: Option<Entity>, normal, penetration }`.
- **`example-10-platformer`:** Side-scrolling platformer with a player AABB driven by A/D + Space, three bouncing circles at restitution 0.85, gravity override (`Vec2::new(0.0, 900.0)`), a 48×18 tilemap with ground/platforms/walls on a `LayerKind::Collision` layer, grounded detection via `CollisionEvents` scan (`normal.y < -0.5`), and a camera that follows the player horizontally clamped to level bounds. Exercises AABB↔AABB, circle↔circle, AABB↔circle, dynamic↔tilemap-static, event consumption by game code, and non-zero gravity in one scene.
- **DECISIONS.md D-033:** Hand-rolled physics, uniform spatial grid broad-phase, AABB+circle only, library-level `Position`/`Velocity` placement, transient tilemap colliders.

### Changed

- Workspace version bumped to `0.6.0-alpha`.
- `App::new` inserts `PhysicsConfig` and `CollisionEvents` resources alongside the existing resource set.
- `aabb_vs_circle` normal convention fixed to match `aabb_vs_aabb` and `circle_vs_circle` — normal now consistently points from `a` into `b`'s free space across all three helpers.
- PHASE2.md: M11 marked complete.

## [0.5.0-alpha] - 2026-04-13

Summary: Phase 2 Milestone 10 — Tilemaps.

### Added

- **Tilemap data types:** `TilemapData`, `TilemapLayer`, `LayerKind { Render, Collision }`, `TileIndex` (alias for `i32`), and `EMPTY_TILE = -1` sentinel in `tungsten-core`. Custom `.tmj` JSON format (tilemap JSON) with `tile_width`, `tile_height`, `width`, `height`, `tileset: Vec<String>`, and `layers: [{name, kind, tiles}]`. Flat row-major `tiles` array with `-1` as the empty-tile marker; non-empty indices look up into `tileset` (D-010 precedent).
- **`TilemapRegistry` resource:** String-ID → `TilemapData` lookup mirroring `AnimationRegistry`, with path-indexed hot-reload lookup (`insert_with_path`, `id_for_path`, `ids`).
- **`TilemapInstance` component:** Plain-data ECS component (`id: String`, `origin: Vec2`) placed on an entity to draw a tilemap at a world position. Multiple instances are supported.
- **`Camera2D` resource:** World-space `position` (top-left) and `zoom`, with a `view_projection(viewport_w, viewport_h) -> Mat4` method. The default (position zero, zoom 1.0) produces the exact same matrix the sprite pipeline built before M10, so examples 01–08 are pixel-identical.
- **Camera-aware pipelines:** `SpritePipeline::update_camera` and `QuadPipeline::update_camera` now take a view-projection `&Mat4` directly; the ortho is computed by the umbrella crate from the `Camera2D` resource each frame. Text is deliberately *not* transformed by the camera — HUD/UI remains screen-space (glyphon owns its own viewport).
- **Manifest tilemaps section:** `assets/manifest.json` gains a `tilemaps` section, with the same fatal missing-file and duplicate-ID checks as sprites/fonts/sounds/animations. `ManifestError::MissingTilemapFile` added.
- **`extract_tilemaps(&World) -> Vec<SpriteBatch>`:** Free function in the umbrella crate that walks every `TilemapInstance`, computes the visible world-AABB from `Camera2D` + `WindowSize`, clips to the tile grid (this is the culling), and batches tiles per texture handle per layer. Returned in layer order so draw order is preserved. Callers concatenate it with their own sprite extract inside `set_extract_sprites` — flat API, caller controls ordering (behind or in front of entity sprites).
- **Tilemap hot reload:** Editing a `.tmj` file re-parses it and replaces the entry in `TilemapRegistry` live. Tileset sprite IDs are revalidated on every reload; a bad reference logs an error and keeps the stale data rather than crashing. Manifest hot reload handles added/removed tilemap entries the same way it already handles sprites/animations/fonts.
- **`example-09-tilemap`:** 48×30 two-render-layer tilemap (ground + decorations) with a non-rendering `collision` layer (M11 seam, accepted by the loader but skipped by extract). WASD/arrows pan a `Camera2D` at 280 px/sec clamped to map bounds. HUD text stays screen-space while the world scrolls. Edit `assets/tilemaps/demo.tmj` live and changes apply within a frame.
- **DECISIONS.md D-032:** `.tmj` extension picked for hot-reload watcher dispatch, tilemaps reuse sprite pipeline, Camera2D default preserves pre-M10 behavior.

### Changed

- Workspace version bumped to `0.5.0-alpha`.
- `Renderer::render_frame_full` now takes `&Mat4` view-projection as its first parameter.
- `SpritePipeline::update_camera` / `QuadPipeline::update_camera` take `&Mat4` instead of `(width, height)`.
- `App::new` inserts `Camera2D` and `TilemapRegistry` resources alongside the existing asset/animation/font/sound registries.
- PHASE2.md: M10 marked complete.
- CLAUDE.md: status line updated to Phase 2 through M10 complete, branch `0.5`.

## [0.4.0-alpha] - 2026-04-13

Summary: Phase 2 Milestone 9 — Hot Reload.

### Added

- **Hot reload watcher:** `HotReloadWatcher` uses `notify` v6 (`RecommendedWatcher`) to watch the `assets/` directory on a background thread. Events cross to the main thread via `std::sync::mpsc` only — no `Arc<Mutex>`, no async (D-031).
- **50ms debounce:** Events are coalesced per path; a path is only dispatched to the reload handler after no new events have arrived for 50ms. Collapses editor double-writes into a single reload per save.
- **Sprite hot reload:** Editing a PNG re-uploads the decoded RGBA bitmap behind the same `TextureHandle`. If dimensions change the old `wgpu::Texture` is replaced in-place (deferred GPU destruction). No restart needed.
- **Animation hot reload:** Editing an animation JSON file reparses the data and replaces the entry in `AnimationRegistry` live. Running `AnimationState` components pick up the new frame timings on the next advance.
- **Font hot reload:** Editing a TTF/OTF removes the old `fontdb` face IDs, trims the glyph atlas, and re-registers the new bytes — text using that font updates within a few frames.
- **Manifest hot reload:** Adding entries to `assets/manifest.json` while running loads new sprites, animations, and fonts immediately. Removed entries log a warning and stay stale (no crash). Duplicate IDs log an error and are skipped.
- **`App::enable_hot_reload(assets_dir, manifest_path)`:** Opt-in per example. Has no effect if the watcher fails to start (the error is logged and the engine continues without hot reload).
- **`FontRegistry` resource:** New resource in `tungsten-core` tracking path→font ID for hot-reload reverse lookup. Inserted by `load_fonts`.
- **`AnimationRegistry` path index:** Added `insert_with_path`, `id_for_path`, `ids()` to `AnimationRegistry`.
- **`AssetRegistry` path index:** Added `path` field to `SpriteAsset`, `path_to_sprite_id` reverse map, `sprite_id_for_path`, `update_sprite_dimensions`.
- **`example-08-hot-reload`:** Demonstrates all three live asset types — a static sprite, a walk-cycle animation, and an instruction text label. Edit any of the watched files while the example is running; no restart needed.
- **DECISIONS.md D-031:** `notify` v6 rationale under D-015 rule 1.

### Changed

- Workspace version bumped to `0.4.0-alpha`.
- `load_fonts` now takes `world: &mut World` to insert the `FontRegistry` resource.
- `register_sprite` now takes a `path: PathBuf` parameter (stored for hot-reload reverse lookup).
- AGENTS.md, CLAUDE.md, DESIGN.md: status updated to M9 complete, M10 tilemaps next.
- PHASE2.md: M7/M8 condensed; M9 marked complete with all acceptance criteria checked.

## [0.3.0-alpha] - 2026-04-13

Summary: Phase 2 Milestone 8 — Audio.

### Added

- **Audio subsystem:** `cpal` output device init with a hand-rolled mixer running on a dedicated callback thread. Game code writes to `AudioCommands` resource; the audio thread drains it each callback. No async runtime (D-027, D-029).
- **Sound decoding:** `symphonia` decodes OGG/WAV/MP3/AAC files eagerly at startup into `SoundData` (f32 PCM). Linear interpolation resampling and mono→stereo upmix happen at decode time, so the mixer callback stays simple (D-028).
- **Sound manifest section:** `assets/manifest.json` extended with a `sounds` section (`looping`, `volume` fields). Sounds are loaded by string ID — consistent with the sprite/animation/font registry pattern.
- **Audio registry:** `SoundRegistry` resource maps string IDs → `AudioHandle(u32)` and stores manifest-declared default volume and looping per handle (`get_volume()`, `get_looping()`). `AudioHandle` is opaque and cheap to copy.
- **`AudioCommands` resource:** `play()`, `play_looping()`, `play_with()`, `stop()`, `stop_all()`, `set_master_volume()` — synchronous API from any system.
- **`AudioSystem` integration in `App`:** Initialized after the startup callback (so sounds are decoded first). Non-fatal if no audio device is available (logs a warning and continues).
- **`KeyCode` variants:** Added `KeyM`, `Digit1`, `Digit2`, `Digit3` to the engine key enum and input bridge.
- **`exit_on_escape` on `App`:** `set_exit_on_escape(false)` lets game code claim the Escape key for pause menus.
- **`assets/sounds/`:** `sfx_blip.ogg` (short one-shot blip) and `music_main.ogg` (30-second looping tone).
- **`example-07-audio`:** Demonstrates one-shot SFX (Space), looping music toggle (M), master volume levels (1/2/3), and stop-all (S), with live status text using M7 fonts.
- **Asset smoke test** (`crates/tungsten/tests/asset_smoke.rs`): headless integration test that loads the workspace manifest, decodes all animations and sounds, and runs as part of `cargo test --workspace` — catches codec/format bugs before example runtime.
- **DECISIONS.md D-027–D-030:** `cpal`, `symphonia`, hand-rolled mixer, and M12 conditional framing.

### Changed

- Workspace version bumped to `0.3.0-alpha`.
- AGENTS.md: structured AI session workflow (startup checklist, session types, principles checklist); font family directory exception documented.
- DESIGN.md: audio architecture section, resolved Phase 2 gating questions table.
- PHASE2.md: M8 complete, M12 conditional on ECS pain.
- CLAUDE.md: current status updated to M8 complete; font family exception documented.

### Fixed

- **OGG Vorbis playback:** Added `vorbis` feature to the `symphonia` workspace dependency. The `ogg` feature only enables the container demuxer; `vorbis` is the required codec. Without it, any OGG file panicked at runtime with "unsupported codec".
- **Manifest sound defaults ignored:** `SoundRegistry::register()` now accepts `volume` and `looping` and stores them per handle. `load_sounds()` passes the manifest-declared values. Previously the `volume` and `looping` fields in the manifest `sounds` section were parsed but silently dropped, so all sounds played at volume 1.0 regardless of their manifest entries.
- **`example-07-audio` volume mixing:** The example now issues `play_with(handle, manifest_volume, looping)` and relies on `set_master_volume` for global scaling, rather than incorrectly passing the master volume as the per-sound volume.

## [0.2.0-alpha.0] - 2026-04-12

Summary: Phase 2 Milestone 7 — Text rendering.

### Added

- **Text rendering pipeline:** GPU text rendering via `glyphon` (built on `cosmic-text` + `swash`), integrated alongside the existing quad and sprite pipelines in `tungsten-render` (D-026).
- **Font manifest section:** `assets/manifest.json` extended with a `fonts` section. Fonts are loaded by string ID, never by file path — consistent with the sprite/animation registry pattern.
- **Font loading:** TTF/OTF files decoded and registered at startup. Three font families staged in `assets/fonts/`: Inter (sans), Source Serif 4 (serif), JetBrains Mono (mono).
- **Text extraction API:** `ExtractTextFn` added to `App`; `TextSection` type in `tungsten-render` for specifying text content, position, font ID, size, and color. The renderer resolves font IDs at draw time via an internal atlas.
- **`example-06-text`:** Demonstrates multi-font text rendering, labels at fixed positions, and a live FPS overlay using the debug text path.
- **DECISIONS.md D-026:** Rationale for `glyphon`/`cosmic-text` under D-015 rule 2.

## [0.1.0-alpha] - 2026-04-12

Summary: Phase 1 complete (milestones M0 through M6).

### Added

- **Workspace scaffold:** Three-crate Cargo workspace (`tungsten-core`, `tungsten-render`, `tungsten`) with pinned dependencies and `rust-toolchain.toml`.
- **Hand-rolled ECS:** `World` with entity lifecycle, type-erased component storage, singleton resources, and typed queries (`query`, `query_entities`). Panic on programmer error, `Option` on runtime lookups (D-022).
- **wgpu renderer:** GPU initialization, surface management, window resizing, and a clear-color render pass. Shaders embedded via `include_str!` from `.wgsl` files (D-023).
- **Colored-quad pipeline:** Instanced rendering of axis-aligned colored rectangles with an orthographic camera.
- **Textured-sprite pipeline:** Instanced sprite rendering with per-sprite nearest/linear filter modes (D-011), a GPU texture pool keyed by opaque `TextureHandle`s (D-016), and alpha blending.
- **Data-driven config:** `tungsten.json` loaded at startup via `serde_json`, with sensible defaults when the file is missing (D-008).
- **Manifest-driven asset loading:** `assets/manifest.json` registers sprites and animations by string ID. Paths resolve relative to the manifest. Multiple manifests compose by extension with fatal duplicate-ID checks (D-017). Validation catches missing files and unresolved sprite references at load time (D-009).
- **Frame-based animation:** Custom JSON animation format with per-frame sprite IDs and durations (D-010). `AnimationState` component advances frames, supports looping and one-shot playback, and guards against zero-duration infinite loops.
- **Edge-triggered input:** Keyboard and mouse input with `is_pressed`, `just_pressed`, and `just_released` semantics. Engine-specific key/button enums decoupled from `winit` via an input bridge.
- **Frame timing:** `DeltaTime` resource updated each frame.
- **Five examples:** `01_window` (clear screen), `02_ecs` (stdout ECS demo), `03_dots` (bouncing quads with keyboard/mouse input), `04_sprites` (textured sprites from manifest), `05_animation` (looping walk cycle).
- **MIT license.**
