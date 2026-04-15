# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.10.0] - 2026-04-15

Phase 3 Milestone 13 — command buffers and fixed-frame structural mutation flush.

### Added

- **Deferred ECS mutation path:** `tungsten_core::CommandBuffer` and `PendingEntity` provide queued `spawn`, `despawn`, `insert`, `insert_pending`, and `remove_component` operations without requiring structural mutation during system iteration.
- **`World::flush`:** New two-pass flush API resolves pending spawns first, then replays queued mutations in registration order with dead-entity guards for late inserts/despawns.
- **Flush telemetry:** `tungsten::FrameTimings` now records `flush_ms`, and `App` logs flush timing in `TUNGSTEN_PERF_LOG` output.
- **M13 ECS coverage:** New unit/integration tests cover command buffer queueing, pending-entity resolution, command ordering, dead-entity guards, and empty-buffer no-op behavior.
- **Command-buffer benchmark:** `command_buffer_flush_1k_spawns` added to `tungsten-core` Criterion benches; current local result is ~252 us for 1k spawns plus 2k deferred inserts.
- **DECISIONS.md D-039:** Records the resource-based command-buffer delivery model, two-pass flush design, and initial benchmark numbers.

### Changed

- Workspace version bumped to `0.10.0`.
- `App` now inserts a fresh `CommandBuffer` resource on startup and drains/replaces it once per frame between system execution and hot reload/extract.
- `README.md`, `DESIGN.md`, `CLAUDE.md`, `AGENTS.md`, and `docs/plans/Phase3.md` now reflect that Phase 3 M13 is complete and that the repo is preparing for the `0.10` line.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, `./scripts/smoke-examples.sh`, `cargo clippy --workspace --all-targets`, the new `command_buffer_flush_1k_spawns` bench, and steady-state ECS regression benches all passed.

## [0.9.0] - 2026-04-15

Phase 3 Milestone 12 — performance baseline, telemetry, and profiling harness.

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

Phase 2 integration — comprehensive platformer demo, example consolidation, and Phase 3 planning.

### Added

- **`example-01-platformer` (comprehensive demo):** Single example that exercises every Phase 2 engine feature in one scene: ECS, physics (AABB player + bouncing circles + tilemap collision), sprites, walk-cycle animation, audio (one-shot SFX, looping music, volume levels), HUD text, camera follow with zoom (= / −), keyboard input, and hot reload. Supersedes and retires the ten separate milestone examples.
- **`KeyCode::Equal` / `KeyCode::Minus`:** New key code variants to support zoom-in / zoom-out input.
- **`docs/plans/Phase3.md`:** Execution plan for M13–M21: command buffers, event queues, transform/render components, input mapping, scene/state system, sprite atlases, debug tooling, particle system, and tween system.

### Changed

- Workspace version bumped to `0.8.0-alpha`.
- Previous milestone examples (`01_window` through `10_platformer`) removed; their feature coverage is consolidated into `01_platformer`.
- `PHASE2.md` archived to `docs/plans/archive/phase2-m7-m12.md`.

### Fixed

- **First-frame dt spike:** `App` now stamps `last_frame` after the startup callback completes rather than before. Asset-load time no longer registers as game time, preventing fast-moving physics bodies from tunneling through thin geometry on the very first frame.
- **Walk animation frame timing:** `walk_2` frame duration corrected from 1500 ms to 150 ms (copy-paste typo in the original JSON).

## [0.7.0-alpha] - 2026-04-14

Phase 2 Milestone 12 — Archetypal ECS rewrite.

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

Phase 2 Milestone 11 — 2D Physics.

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

Phase 2 Milestone 10 — Tilemaps.

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

Phase 2 Milestone 9 — Hot Reload.

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

Phase 2 Milestone 8 — Audio.

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

Phase 2 Milestone 7 — Text rendering.

### Added

- **Text rendering pipeline:** GPU text rendering via `glyphon` (built on `cosmic-text` + `swash`), integrated alongside the existing quad and sprite pipelines in `tungsten-render` (D-026).
- **Font manifest section:** `assets/manifest.json` extended with a `fonts` section. Fonts are loaded by string ID, never by file path — consistent with the sprite/animation registry pattern.
- **Font loading:** TTF/OTF files decoded and registered at startup. Three font families staged in `assets/fonts/`: Inter (sans), Source Serif 4 (serif), JetBrains Mono (mono).
- **Text extraction API:** `ExtractTextFn` added to `App`; `TextSection` type in `tungsten-render` for specifying text content, position, font ID, size, and color. The renderer resolves font IDs at draw time via an internal atlas.
- **`example-06-text`:** Demonstrates multi-font text rendering, labels at fixed positions, and a live FPS overlay using the debug text path.
- **DECISIONS.md D-026:** Rationale for `glyphon`/`cosmic-text` under D-015 rule 2.

## [0.1.0-alpha] - 2026-04-12

Phase 1 complete (milestones M0 through M6).

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
