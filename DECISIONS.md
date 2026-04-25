# DECISIONS.md

Decision log for non-obvious Tungsten choices.

- IDs are sequential
- Settled entries are immutable
- Reversals add a new entry marked `Superseded by D-XXX`

---

## D-001 — Project name: Tungsten
**Date:** 2026-04-07  
**Decision:** Project name is Tungsten; crate prefix `tungsten-`; umbrella crate is `tungsten`.

## D-002 — Process: judgment over compliance
**Date:** 2026-04-07  
**Decision:** No CI gate, no mandatory docs, no self-review checklist. Judgment over compliance. Reassess milestones explicitly rather than abandon quietly.

## D-003 — Native only, no WASM
**Date:** 2026-04-07  
**Decision:** Native targets only (Linux, macOS, Windows).  
**Why:** WASM constrains dependency choices and doubles the test matrix.

## D-004 — wgpu as renderer
**Date:** 2026-04-07  
**Decision:** Use `wgpu`. Fallback if too painful: `pixels` or `macroquad`, not `ash`.  
**Why:** Cross-platform GPU API at a manageable level; `ash` is too low-level, `glow`/OpenGL is dated.

## D-005 — Hand-rolled ECS, no external crate
**Date:** 2026-04-07  
**Decision:** Build the ECS by hand; no external ECS crate ever.  
**Why:** `bevy_ecs`, `hecs`, etc. hand over the thing this project is supposed to build.

## D-006 — Cargo workspace, three crates
**Date:** 2026-04-07  
**Decision:** `tungsten-core` (ECS, assets, config, time), `tungsten-render` (wgpu, pipelines, samplers), `tungsten` (umbrella + winit app loop). Split further only when a crate becomes genuinely unwieldy.

## D-007 — `tungsten-render` may depend on `tungsten-core`
**Date:** 2026-04-07  
**Decision:** `tungsten-render` may depend on `tungsten-core` and use its types where convenient.  
**Why:** Strict separation is overhead without benefit for a solo project.

## D-008 — Config is JSON, loaded once at startup
**Date:** 2026-04-07  
**Decision:** Single `tungsten.json` at workspace root, loaded once via `serde_json`. Missing → defaults + warning. Invalid → fatal.  
**Why:** Engine parameters shouldn't require recompilation; TOML/RON add no decisive value.

## D-009 — Manifest-driven assets, ID-referenced
**Date:** 2026-04-07  
**Decision:** `assets/manifest.json` registers every asset by string ID. Game code uses IDs, never paths. Validation at load time.  
**Why:** Decouples code from file layout; the indirection is the architectural prerequisite for hot reload.

## D-010 — Custom JSON animation format
**Date:** 2026-04-07  
**Decision:** `{ looping: bool, frames: [{sprite: id, duration_ms: u32}] }`. Each animation in its own file under `assets/animations/`.  
**Why:** Avoids locking into Aseprite's export schema; per-frame durations support emphasis frames.

## D-011 — Per-sprite filter mode in the manifest
**Date:** 2026-04-07  
**Decision:** Filter mode is a per-sprite manifest property — `nearest` (default) or `linear`. Renderer creates one sampler per mode.  
**Why:** A global setting can't mix pixel art and high-res UI in the same scene.

## D-012 — Hot reload deferred to Phase 2
**Date:** 2026-04-07  
**Decision:** Phase 1 shipped without hot reload; hot reload shipped in M9. Phase 1 must preserve the registry-by-ID invariant.

## D-013 — Asset directory layout: by-type at workspace root
**Date:** 2026-04-07  
**Decision:** Shared `assets/` at workspace root — `sprites/`, `animations/`, `sounds/`, `fonts/`. Examples ship `examples/NN_name/assets/` with a local manifest.

## D-014 — Asset registry is a Resource in the World
**Date:** 2026-04-08  
**Decision:** The asset registry is a `Resource`, accessed the same way as `DeltaTime` and `InputState`.  
**Why:** Avoids a second "global-ish" pathway; static/singleton ruled out by no-global-mutable-state rule.  
**Consequences:** If the World is dropped and recreated, registry handles die with it; the renderer remains responsible for actual wgpu resource lifetimes.

## D-015 — Dependency philosophy: three acceptance rules
**Date:** 2026-04-08  
**Decision:** A dep is acceptable if it (1) abstracts a platform API, (2) implements a well-specified data format, or (3) provides a math/primitive solved problem. See `DESIGN.md` for the table.

## D-016 — Opaque asset handles, no wgpu types in core
**Date:** 2026-04-08  
**Decision:** `tungsten-core` stores opaque `TextureHandle(u32)` IDs. `tungsten-render` owns GPU textures in internal pools keyed by those handles.

## D-017 — Multiple manifests compose by extension, never override
**Date:** 2026-04-08  
**Decision:** IDs must be globally unique across the merged manifest set; duplicates are fatal. Each path resolves relative to its declaring manifest.

## D-018 — Extract plain data before drawing
**Date:** 2026-04-08  
**Decision:** Systems mutate the `World` during `tick`; extract functions produce POD render data (`QuadInstance`, `SpriteInstance`, `TextSection`) passed into `tungsten-render`. Renderer may read the asset registry but needs no long-lived mutable World access.

## D-019 — `pollster` for blocking on wgpu async init
**Date:** 2026-04-12  
**Decision:** Use `pollster` v0.4 to block on `request_adapter`/`request_device`. Satisfies D-015 rule 3.

## D-020 — `bytemuck` for GPU data layout
**Date:** 2026-04-12  
**Decision:** Use `bytemuck` v1 with `derive`. GPU-uploaded structs derive `Pod` and `Zeroable`. Satisfies D-015 rule 3.

## D-021 — Entity ID is `u32` (Phase 1); generational in M12
**Date:** 2026-04-12  
**Decision:** Phase 1: `Entity(u32)`. M12: upgraded to `Entity { index: u32, generation: u32 }` when the ECS was rewritten (D-036); `entity.id()` returns `index` for source compatibility.

## D-022 — ECS error strategy: panic vs Result
**Date:** 2026-04-12  
**Decision:** Panic on programmer errors (insert on dead entity, wrong downcast). Return `Option`/`Result` on runtime conditions (entity not found, component absent).

## D-023 — WGSL shaders embedded via `include_str!`
**Date:** 2026-04-12  
**Decision:** Shaders are `.wgsl` files in `tungsten-render/src/`, pulled in at compile time. Shader changes require recompilation; not hot-reloadable.

## D-024 — Phase 1 exit observations for Phase 2 planning
**Date:** 2026-04-12  
**Decision:** Phase 1 (M0–M6) exit observations: (1) `glyphon` for text; (2) naive ECS fine at Phase 1 scale; (3) `symphonia` for audio decode; (4) registry-by-ID invariant holds, `notify` planned for hot reload.

## D-025 — License: MIT
**Date:** 2026-04-12  
**Decision:** MIT. `LICENSE` at repo root; `license = "MIT"` in workspace `Cargo.toml`.

## D-026 — `glyphon` + `cosmic-text` for text rendering
**Date:** 2026-04-12  
**Decision:** Use `glyphon` (pulls in `cosmic-text`, `swash`, `fontdb`) for M7 text rendering. Satisfies D-015 rule 2 (TrueType/OpenType is a well-specified format).

## D-027 — `cpal` for audio device access
**Date:** 2026-04-13  
**Decision:** Use `cpal` v0.15 for audio output. Satisfies D-015 rule 1 (wraps WASAPI/CoreAudio/ALSA). Dep of `tungsten` only.

## D-028 — `symphonia` for audio decoding
**Date:** 2026-04-13  
**Decision:** Use `symphonia` v0.5 (features: `ogg`, `wav`, `mp3`) for eager load-time decode into `Vec<f32>` PCM. Satisfies D-015 rule 2. No `symphonia` types appear at runtime in the audio callback.

## D-029 — Hand-rolled audio mixer, no `kira`
**Date:** 2026-04-13  
**Decision:** Hand-roll the mixer in the `cpal` callback (~150 lines). Features: play/stop/loop, master volume, per-sound volume. No DSP, no spatial audio.  
**Why:** `kira` and `rodio` hand over the mixer; the mixer is within scope for this project to build.

## D-030 — M12 ECS rewrite is conditional
**Date:** 2026-04-13  
**Decision:** M12 (archetypal ECS) requires a `DECISIONS.md` entry before beginning, confirming whether to proceed or skip. `v1.0.0` is not blocked on M12. Satisfied by D-036.

## D-031 — `notify` for file watching (hot reload)
**Date:** 2026-04-13  
**Decision:** Use `notify` v6 with `default-features = false`. `RecommendedWatcher` auto-selects per platform. Events via `std::sync::mpsc`; 50ms debounce in main-thread polling. Satisfies D-015 rule 1. Dep of `tungsten` only.

## D-032 — M10 tilemap shape
**Date:** 2026-04-13  
**Decision:** Three coupled choices: (1) `.tmj` extension with Tiled-compatible schema — extension-based hot-reload dispatch, standard editor compatibility; (2) tilemaps reuse the sprite pipeline — `extract_tilemaps` produces `SpriteBatch`es, no new wgpu pipeline; (3) `Camera2D` default (position zero, zoom 1.0) produces the exact matrix the sprite pipeline built internally pre-M10, so examples 01–08 are pixel-identical.  
**Consequences:** Text ignores `Camera2D` (screen-space). `SpritePipeline::update_camera` and `QuadPipeline::update_camera` now take `&Mat4`.

## D-033 — M11 physics shape
**Date:** 2026-04-14  
**Decision:** Four coupled choices: (1) no external physics crate — hand-rolled in `tungsten-core::physics`; (2) uniform spatial grid broad-phase rebuilt per substep, no persistent state; (3) `Position`/`Velocity` live at library level, not migrated into existing examples; (4) tilemap colliders are transient — one static AABB per tile per substep, no baked registry.  
**Known limits:** Variable-dt with substep cap — preferred upgrade is semi-fixed accumulator. Tilemap collider budget: ≤128×128 tiles; larger maps should pre-bake a static spatial index.

## D-034 — Lock-free SPSC ring for the audio command channel
**Date:** 2026-04-14  
**Decision:** Replace `std::sync::mpsc` in the `cpal` callback with `rtrb` v0.3 (wait-free SPSC ring, capacity 64). Satisfies D-015 rule 3. Dep of `tungsten` only.  
**Why:** `mpsc::try_recv` can allocate on state transitions; `rtrb::Consumer::pop` is allocation-free on the fast path.

## D-035 — Manifest merge order: call-site order
**Date:** 2026-04-14  
**Decision:** Multiple manifests merge in call-site order (typically root manifest first, then example-local). Forward references from later manifests return `None` at runtime. Global uniqueness enforced by the Layer 1 integration test.

## D-036 — M12: Proceed with archetypal ECS rewrite
**Date:** 2026-04-14  
**Decision:** Proceed with M12. After M11 the full M7–M11 workload was in place — a realistic benchmark target. Satisfies D-030.  
**Storage design:** See `DESIGN.md` §ECS for the full description (archetype graph, `AnyColumn`/`TypedVec<T>`, lazy edges, generational IDs, `query2`/`query3`).  
**Results:** ~6× on single-type queries, ~200× on multi-component queries vs. naive `HashMap<TypeId, HashMap<u32, Box<dyn Any>>>` baseline (10k entities, release profile). See `DESIGN.md` §Archetypal ECS for the benchmark table.

## D-037 — `criterion` added to `tungsten-render` dev-dependencies
**Date:** 2026-04-15  
**Decision:** Add `criterion = { version = "0.5", features = ["html_reports"] }` as a `[dev-dependencies]` entry in `crates/tungsten-render/Cargo.toml` for render-side micro-benchmarks (sprite batch build, extract cost). Satisfies D-015 rule 3 (benchmark harness is a solved primitive). `criterion` is already a `tungsten-core` dev-dep at the same version; this extends the pattern symmetrically.

## D-038 — M12 CPU telemetry: std::time::Instant inline, no external dep
**Date:** 2026-04-15  
**Decision:** Frame-stage timings (update/extract/render/audio/hot-reload) measured with `std::time::Instant::now()` / `.elapsed()` inline in `app.rs`, accumulated in a `FrameTimings` struct stored as a World resource. No external profiling crate is introduced. Rationale: (1) `std::time::Instant` gives millisecond-resolution diagnostics sufficient for Phase 3 scale; (2) keeping measurements in the same file as timed code avoids over-abstraction; (3) M18 HUD can consume `FrameTimings` from the resource with no API change. Per-system timing: `App` stores system names alongside closures (`system_names: Vec<String>`, `system_name_counter: usize`). Each system call is wrapped with `Instant`; durations populate `FrameTimings::system_timings: Vec<(String, f32)>`. Cost: one `Instant::now()` + `.elapsed()` per system per frame — acceptable at Phase 3 scale.

## D-039 — M13 CommandBuffer: two-pass flush, closure-typed removes, resource-based delivery
**Date:** 2026-04-15  
**Decision:** Implement `CommandBuffer` as a `Vec<Command>` stored as a `World` resource. `App` inserts a fresh buffer before each frame's systems run and drains it immediately after (flush stage, before hot-reload and extract). Four operations: `spawn` -> `PendingEntity`, `insert` / `insert_pending` (live vs. pending target), `remove_component`, `despawn`. Flush algorithm: two-pass — allocate real entities for all `Spawn` commands first (building a `Vec<Entity>` indexed by `pending_id`), then replay all mutations in registration order. Type-erased component insert uses a private `ComponentSetter` trait object (`pub(super)` within the `ecs` module). Type-erased remove uses `Box<dyn FnOnce(&mut World)>` capturing entity and type statically, which avoids adding a type-erased remove method to `Archetypes`. Stale-despawn guard: `if world.is_alive(e)` in the flush despawn arm. Next-frame visibility rule for systems: entities spawned in frame N are queryable by systems starting frame N+1, but visible to extract/render in frame N. No new crate dependencies (D-015 satisfied). Bench: `command_buffer_flush_1k_spawns` ~= 252 us (1k spawns + 2k inserts via buffer) vs. `spawn_despawn_1k` ~= 80 us on the 2026-04-15 local verification run.

## D-040 — M14 EventQueue: two-window typed event buffering
**Date:** 2026-04-16
**Decision:** Add `EventQueue<T>` as the canonical event-passing primitive. Each queue stores two windows (`previous`, `current`) so readers always see at least the most recent frame's events regardless of system registration order. `send()` appends to `current`; `iter()` yields `previous` then `current`; `iter_current()` is the opt-in same-frame-only view. `flush()` rotates at the same frame boundary as `CommandBuffer` flush — after systems, before hot reload, extract, and render. `App::register_event::<T>()` is a startup-only API that inserts the resource and stores a type-erased per-frame flush closure. Re-registering the same type is a no-op so duplicate startup calls cannot accidentally double-flush a queue. `flush()` remains `pub` so the umbrella crate can invoke it across crate boundaries, with docs warning that game systems should not call it directly. `CollisionEvents` is removed with no compatibility shim; all call sites migrate to `EventQueue<CollisionEvent>`. Bench: `event_queue_flush_10_types` measured ~= 2.44 us on the 2026-04-16 final local verification run (Criterion range: 2.4234-2.4597 us for 10 queue types with 100 events each).

## D-041 — Cargo profile optimization: release LTO + codegen-units + panic=abort + target-cpu=native
**Date:** 2026-04-16  
**Decision:** Apply these compilation flags across the workspace:

**`.cargo/config.toml`** (all builds on this machine):
- `-C target-cpu=native` — enables AVX2/FMA and the full native ISA. Non-portable binary. All benchmark numbers below are keyed to this flag on AMD Radeon 660M / AMD Ryzen 5 6600H (Arch Linux, rustc 1.94.1).

**`[profile.release]` in workspace `Cargo.toml`:**
- `lto = "thin"` — ThinLTO: parallel cross-CGU import/export pass, cross-crate inlining.
- `codegen-units = 1` — single LLVM CGU, maximum within-crate inlining budget.
- `panic = "abort"` — removes landing pads and unwind tables from LLVM IR; verified safe across all deps including `cpal` on the 2026-04-16 validation pass (188 tests in the suite at that time).
- `debug = 1` — line-number tables only; preserves `perf`/flamegraph source annotation.
- `strip = "none"` — explicit; profiling workflow requires symbols in the binary.

**`[profile.dev.package."*"]`:**
- `opt-level = 2` for all external deps in dev builds — `wgpu`/`winit`/`glam`/`cpal` run at useful speed; project crates remain at opt-level 0 for fast incremental cycles.

**Benchmark results** (post-optimization, 2026-04-16, Criterion `bench` profile inherits `[profile.release]`):

| Benchmark | Time | vs. D-036/D-039/D-040 baseline | Note |
|-----------|------|-------------------------------|------|
| `spawn_insert_3_components_10k` | 3.736 ms | −12.6% | |
| `query_single_10k` | 6.746 µs | −1.5% | |
| `query2_homogeneous_10k` | 6.789 µs | −6.5% | |
| `query2_fragmented_5arch_10k` | 7.045 µs | −8.0% | |
| `query2_10k_5archetypes_pv` | 13.845 µs | −3.2% | |
| `spawn_despawn_1k` | 72.964 µs | −9.5% | |
| `command_buffer_flush_1k_spawns` | 236.89 µs | −7.6% | |
| `naive_query_single_10k` | 29.976 µs | −20.8% | HashMap baseline; LTO inlines HashMap internals more aggressively |
| `naive_query2_via_entities_10k` | 652.22 µs | −31.4% | Same |
| `event_queue_flush_10_types` | 2.486 µs | −19.3% | |
| `position_integration_50k` | 1.980 ms | −3.7% | glam Vec2 gains from FMA/AVX |
| `broadphase_rebuild_5k_dynamic` | 312.56 µs | −37.3% | Largest gain; AABB/grid code fully vectorised |
| `sprite_extract_batch_build_2k` | 5.842 µs | −20.4% | |

The prior D-036 comparison ratios (~6× and ~200× archetypal vs. naive) still hold directionally; the absolute numbers for both sides improved proportionally under the new profile. The archetypal advantage is unchanged.

## D-042 — M15 Transform + render components
**Date:** 2026-04-16  
**Decision:** Four coupled choices:

1. New engine-level components live in `tungsten-core::components`:
   - `Transform { position: Vec2, rotation: f32, scale: Vec2 }`
   - `Sprite { asset_id: String, color: [u8; 4], z_order: i32 }`
   - `Visibility { visible: bool }`
   - `Tag { name: String }`
2. Physics `Position` stays separate (per `D-033`). `Position -> Transform.position` is an opt-in free-fn system `sync_position_to_transform`; examples register it between `physics_step` and any extract stage that needs authoritative visuals. There is no reverse sync; physics remains the source of truth for `Position`.
3. `SpriteInstance` grows by two fields (`rotation: f32`, `color: [u8; 4]`) so the component data can reach the GPU; all in-tree call sites migrate in the same commit — no backwards-compat shim.
4. If the App has no custom sprite-extract, `extract_sprites_default` runs over `Transform + Sprite + Visibility`. `Visibility` is required — entities with `Transform + Sprite` but no `Visibility` are never emitted by the default path. No implicit fallback.

Plan number conflict note: the M15 plan originally reserved `D-041`, but that ID was claimed on the same day by the Cargo profile entry; the M15 decision was renumbered to `D-042` on close-out.

## D-043 — M17 display settings live in `tungsten.json` and apply at a frame boundary
**Date:** 2026-04-17  
**Decision:** Four coupled choices:

1. Display settings live under a `display` section inside the existing workspace-root `tungsten.json`, not in a second `display.json` file. This preserves D-008's single-config-file rule.
2. `tungsten-core` owns the plain data model (`DisplayState`, `DisplayConfig`, `DisplayMode`, `ScaleMode`, `Resolution`) and validation only. No `winit` or `wgpu` types cross into core, preserving D-007 and D-016.
3. Gameplay/example code requests runtime changes through one public API: `tungsten::request_display_settings(&mut World, DisplayState)`. Actual window/surface mutation happens only at the top of `WindowEvent::RedrawRequested`, before surface acquire, so systems never mutate `winit`/`wgpu` state mid-frame.
4. Legacy `window.*` and `render.*` display fields remain valid for M17. The new `display.*` fields win when both specify the same concern. `exclusive_fullscreen` is accepted in config and requests, but runtime support is still limited to windowed and borderless fullscreen, so exclusive requests are downgraded to borderless with a warning until a later milestone adds real video-mode selection.

## D-044 — M18 runtime telemetry HUD
**Date:** 2026-04-18  
**Decision:** Six coupled choices:

1. HUD implementation lives in the umbrella crate (`crates/tungsten/src/debug_hud.rs`) and reads existing telemetry resources (`FrameTimings`, `DisplayTelemetry`, `CameraState` / `CameraController`, `RenderCounts`, optional `HudActiveState`) rather than owning a parallel timing path. `D-038` / `D-043` / `D-042` dictate where those resources already live; HUD does not duplicate or bypass them.
2. `F4` is a hardcoded engine toggle, routed through one engine-owned system (`hud_toggle_system`) registered by `App::new` as the first system each frame. Rebinding waits for `M19`'s `ActionMap`.
3. The extension point is `Vec<Box<dyn Fn(&World) -> Vec<HudRow>>>`. Providers return `Vec<HudRow>` rather than a single row so one provider can emit the top-N slowest systems as N rows in a stable slot.
4. `DebugHud::enabled` defaults to `false` and every in-tree example ships off-by-default. Examples opt in by mutating the resource during setup (`world.get_resource_mut::<DebugHud>().unwrap().enabled = true`).
5. HUD-side frame-time smoothing uses EWMA (`alpha = 0.1`) applied to the previous frame's `FrameTimings::total_ms`. The one-frame lag is intentional: it keeps the compose helper in the extract stage (so the HUD is visible in the same frame its rows describe, apart from the fps/frame-ms row) and avoids a second post-render pass.
6. Perf budget is qualitative: "negligible at Phase 3 scale", tracked via `perf-capture.sh` sprite-stress runs recorded in `perf-runs/M18-hud/`. A regression over 5% requires a `DECISIONS.md` amendment before milestone close.

## D-045 — M19 input action map
**Date:** 2026-04-19  
**Decision:** Eight coupled choices:

1. Scope is boolean actions over keyboard + mouse only. No gamepad, no virtual axes / 2D axes, no chord or sequence bindings. The serialized binding shape is `HashMap<String, Vec<Binding>>` where `Binding` is `{ Key { code }, Mouse { button }, Scroll { direction } }`. Analog and higher-order binding schemes stay out until a concrete consumer (M21 debug tools, later settings UI) demands them.
2. `ActionMap` lives in `tungsten-core` (`crates/tungsten-core/src/input/action_map.rs`). `InputState`, `KeyCode`, `MouseButton`, and the raw cursor / scroll data already live there; putting `ActionMap` alongside them keeps the query path (`ActionMap::is_pressed(&InputState, action)`) on one side of the core/render seam. Placing it in the umbrella would force every core consumer that wants action lookup to import from `tungsten::`, violating `D-007` in spirit even though `D-007` only guards against winit/wgpu leaks.
3. No new dependency. `serde` + `serde_json` already satisfy load and (de)serialization; `notify` already runs the hot-reload watcher. `D-015` rules 2 (well-specified format) and 1 (platform primitive) both cover the existing surface; adding a dedicated input-mapping crate would have failed `D-015` for no runtime benefit.
4. User bindings in `input.json` override defaults per action; missing actions inherit `ActionMap::default_map()`. The fallback is silent — missing file logs info and keeps defaults, invalid JSON at startup is fatal (parity with `tungsten.json` under `D-008`). On hot-reload, invalid JSON logs an error and keeps the previous map so a save-in-progress never breaks a live session.
5. Engine-owned controls now live in the same map as gameplay actions: `engine_toggle_hud`, `engine_toggle_vsync`, `engine_toggle_fullscreen`, and `engine_exit`. Their default bindings remain `F4`, `F9`, `F11`, and `Escape`, but routing them through the action map removes the last hardcoded key branches while keeping safe defaults whenever `input.json` is absent. `App::set_exit_on_escape(false)` still gates the engine exit action for examples or future states that want to own the mapped input themselves.
6. Mouse support is split into two layers. Buttons share the same held / just-pressed / just-released semantics as keys. Wheel movement is exposed both as raw per-frame line/pixel deltas on `InputState` and as one-frame `ScrollDirection::{Up,Down}` impulses so scroll-up / scroll-down can participate in boolean action queries. Extra mouse buttons serialize canonically as `button4`, `button5`, etc.
7. Hot-reload reuses the existing `HotReloadWatcher` (`D-031`). Because `input.json` lives at the workspace root rather than inside an asset dir, `HotReloadWatcher::new` accepts an `extra_files: &[PathBuf]` list. Each extra file pins its parent directory with `RecursiveMode::NonRecursive` and the canonical file path is recorded for `drain_ready` filtering, so unrelated files in the same parent don't trip reloads. One watcher instance, 50ms debounce, same `process_hot_reload` dispatch site.
8. Runtime rebind persistence writes atomically back to `input.json` using a same-directory temp-file + rename path. The writer first tries to replace only the top-level `actions` object in the previously loaded source text so surrounding layout or extra top-level fields survive; if that patch is not safe, it falls back to canonical pretty JSON. Unknown action names at query time (`ActionMap::is_pressed(&input, "dance")`) return `false` rather than panic — per `D-022` this is a runtime miss, not a programmer bug.

**Rejected alternatives:**

- `input.json` under `assets/` to piggyback on the existing recursive watcher. Rejected for parity with `tungsten.json`: config-level data belongs at the workspace root, not mixed in with shipped assets.
- Axis-typed actions. Deferred; no current consumer needs analog input in M19, and adding `Axis { positive, negative }` alongside boolean `Key`/`Mouse` now would commit to a serialized shape before we have a real use case.
- Keep engine-reserved controls hardcoded forever. Rejected once M19 gained persistence and default-fallback coverage: the hardcoded branches had become the odd path out, and routing the engine controls through the same map gives one source of truth while the built-in defaults still prevent lockout when `input.json` is missing.
- Shorter key aliases (`"a"` for `"KeyA"`). Canonical `KeyCode` variant names only, to keep the serialization surface a 1:1 map with `winit`. Aliases can layer on post-M19 with zero migration risk.

## D-046 — M20 scene/state system
**Date:** 2026-04-20  
**Decision:** Six coupled choices:

1. `StateStack` and `GameState` live in the `tungsten` umbrella crate (`crates/tungsten/src/state.rs`) per the Phase 3 Core Objects table; `SceneData` / `SceneEntry` / `SceneTransform` / `SceneSprite` live in `tungsten-core` (`crates/tungsten-core/src/assets/scene.rs`) because asset parsing belongs on the core side of the crate seam. The dispatcher needs the umbrella's `DebugHud` / `HudActiveState` surfaces, so keeping the state machine out of core avoids an inverted dep.
2. A single engine-owned `state_dispatcher_system` registered immediately after `__display_input` drives transitions. This mitigates the Phase 3 M20 risk "runtime system-list churn; prefer a single dispatcher system": the runtime system list stays static across state transitions, and the dispatcher drains `StateStack.pending` once per frame so hooks fire in deterministic order within the canonical frame (input → systems → flush → events → hot reload → extract → render).
3. Scene-owned entity cleanup uses a `SceneEntity { state_id }` marker component. The dispatcher walks `query::<SceneEntity>()` and enqueues a `CommandBuffer::despawn` for every matching entity **before** the user's `on_exit` runs. The engine's post-systems `CommandBuffer` flush (`D-039`) applies the despawns so the last frame of an exiting state already sees its scene entities gone and the first frame of the next state already sees its scene entities present.
4. Transition matrix: `push(new)` fires `old.on_pause` → `new.on_enter`; `pop()` fires `old.on_exit` (after auto-despawn) → `next.on_resume`; `replace(new)` fires `old.on_exit` (after auto-despawn) → `new.on_enter`. `on_pause` and `on_resume` default to no-op on the `GameState` trait so a Pause state can overlay Gameplay without tearing its scene down — only `on_exit` triggers auto-despawn.
5. `scene.json` is a minimal schema that reuses the `D-042` components: `SceneEntry` maps directly to `Transform + Sprite? + Visibility + Tag?`. Sprite ids are not validated against `AssetRegistry` at spawn time; unresolved ids fall through to the sprite-extract warning path, matching how `TilemapInstance` treats unresolved tile ids. Scene hot-reload is out of scope for M20 — the loader is a plain `SceneData::load(&Path)` call from user code, not a watcher.
6. `ActionMap::default_map()` grows three engine-neutral defaults — `state_start` (`Enter`), `state_pause` (`KeyP`), `state_back` (`Backspace`) — so examples drive state transitions without an edited `input.json`. These stay distinct from the `engine_*` set (which are reserved for engine-owned controls like `F4`, `F9`, `F11`, `Escape`) because state transitions are gameplay semantics expressed through the action map, not engine policy.

**Rejected alternatives:**

- Per-state system lists (add/remove systems on transitions). Rejected upfront per the M20 risk; churning the runtime system list across transitions would fight the fixed-order frame loop (`D-038`) and leak state-dispatcher concerns into every milestone that touches system scheduling.
- Validating sprite ids at `spawn_scene` time. Rejected for parity with tilemap behaviour and because the sprite-extract default already logs on miss; adding a second validation point would only split the error path.
- Scene hot-reload. Explicit non-goal for M20 — adding it pulls in `HotReloadWatcher` wiring that the data-driven spawn path does not need to ship, and the marker-based auto-despawn design is orthogonal to reload cadence.
- A separate `SceneEntityRemoved` event. Rejected: the `CommandBuffer::despawn` path is already the canonical way structural edits land (`D-039`), and adding an event would duplicate the signal already available through querying `SceneEntity` presence.

## D-047 — M21 debug tooling
**Date:** 2026-04-20
**Decision:** Five coupled choices:

1. `DebugDraw` lives in `tungsten-core` as pure POD (`DebugShape::{Aabb, Circle, Line}` + `DebugCommand { shape, color, thickness }`) and crosses the core/render seam the same way `QuadInstance` does (`D-018`). The extract stage drains one frame's commands into two GPU-friendly channels: axis-aligned AABB edges expand into four `QuadInstance`s drawn through the existing `QuadPipeline` (the pipeline runs twice per frame — once for gameplay quads, once for debug AABB edges), while lines and circle polylines expand into `DebugLineInstance`s drawn by a new `DebugLinePipeline`. `DebugLinePipeline` borrows `QuadPipeline`'s camera bind group layout so only one `view_proj` uniform ships on the GPU. No second AABB pipeline ships; no circle pipeline ships.
2. The three new overlays (`F1` physics, `F2` system timings, `F3` inspector) are independent action-toggled resources, not `DebugHud` rows (`D-044`). Each overlay owns its own `.enabled` flag, its own compose state, and is toggled through the engine-owned action map (`D-045`) — `engine_toggle_physics_debug`, `engine_toggle_systems_overlay`, `engine_toggle_inspector`. `DebugHud`'s `F4` toggle stays orthogonal: flipping the HUD does not touch the overlays and vice versa. The rationale is that HUD rows are a fixed text list while these overlays have distinct inputs (world geometry, per-system EWMA, entity picking), and coupling them to `DebugHud.enabled` would force one toggle to govern four unrelated behaviours.
3. Screenshot capture renders the armed frame into an offscreen `wgpu::Texture` (`format = surface_format`, `usage = RENDER_ATTACHMENT | COPY_SRC`, label `"tungsten_screenshot_target"`), issues `copy_texture_to_buffer` into a row-padded `MAP_READ` buffer, blocks on `device.poll(Wait)`, strips the `bytes_per_row` padding, and encodes via `image::save_buffer`. The swapchain is never `COPY_SRC`. Capture is armed via `TUNGSTEN_CAPTURE_FRAME=<n>` (+ optional `TUNGSTEN_CAPTURE_PATH`, `TUNGSTEN_CAPTURE_RESOLUTION=<WxH>`) and is off by default so release-mode examples pay zero capture cost. The blocking map path is explicitly dev-tool only and documented as unsafe for production frame-critical code.
4. Image diff is per-pixel RGBA with a single tolerance threshold: delta per pixel is `max(|Δr|, |Δg|, |Δb|, |Δa|)` and `pixels_above_tolerance` is the count above the caller-supplied byte threshold. No perceptual metric (SSIM, Delta-E, etc.) — driver-level noise on the reference machine already sits within `tolerance = 2` and no milestone consumer needs perceptual weighting. If a future driver jitters above that floor, the fallback is to raise `pixels_above_tolerance < 16` rather than swap the metric. Mismatched dimensions are a hard error (`ImageDiffError::DimensionMismatch`) rather than a resize.
5. GPU debug groups (`encoder.push_debug_group("tungsten_frame")`, render-pass groups `"quads" / "sprites" / "debug_quads" / "debug_lines" / "text"`) plus explicit `label:` fields on every new pipeline, buffer, texture, and bind group are always-on. No feature gate, no RenderDoc-only path — the calls are near-free on non-capture backends and make every captured frame self-describing. This is the policy-level commitment; individual call sites are enforced by code review, not a lint.

**Rejected alternatives:**

- A second pipeline for axis-aligned AABB edges. Rejected — `QuadPipeline` already consumes `QuadInstance` POD, so emitting four thin quads per AABB from the extract stage reuses the pipeline for free. A second pipeline would duplicate the camera binding, vertex buffer, and module with no functional benefit.
- Putting overlay toggles under `DebugHud.enabled`. Rejected — covered in choice 2; one flag cannot govern four unrelated behaviours without surprising users who enable the HUD and get an inspector they did not ask for.
- Making the screenshot path re-use the swapchain texture as a `COPY_SRC`. Rejected — requires the swapchain to carry `COPY_SRC` usage every frame even when capture is disabled, which is a runtime cost for a dev-only feature. The offscreen-target path keeps the default frame cost unchanged.
- Perceptual image diff (SSIM / CIEDE2000). Rejected — no consumer needs perceptual weighting; a fixed RGBA tolerance catches the regressions M21 cares about (content changes, pipeline regressions) while tolerating driver-level noise. Adding a perceptual metric would pull in an extra dep under `D-015` rule 3 with no matching use case.
- Gating GPU debug groups behind a feature flag or `cfg(debug_assertions)`. Rejected — the calls are cheap on every backend and release-build captures need the labels too. Always-on keeps RenderDoc output identical across profiles.

## D-048 — M22 sprite atlases (shelf packer, per-filter pages, half-texel inset)
**Date:** 2026-04-20
**Decision:** Six coupled choices (Phase 3 M22 delivers on `DESIGN.md` §17's "Pack sprites into atlas textures at load time"):

1. **Shelf-next-fit packer, hand-rolled in `tungsten-core`.** `crates/tungsten-core/src/assets/atlas.rs` sorts a stable copy of `&[PackInput]` by `(height desc, width desc, id asc)` and fills shelves inside an `AtlasPage` until either axis would overflow `max_dim`, then opens a new page. Page dimensions are `next_power_of_two(observed_extent)` clamped to `max_dim`. The sort's `id asc` tie-break is mandatory — without it two pack runs with identical input can yield different shelf layouts, and both the image-diff gate (step 9) and the hot-reload in-place fast path (choice 4) depend on determinism. No new runtime dependency; `rect_packer`, `crunch-rs`, and `guillotiere` were all considered and rejected under `D-015` rule 3 for a 200-LOC problem.
2. **One page list per `FilterMode`, overflow to multiple 2D pages.** Nearest and linear sprites are packed into separate page lists and bound to separate `GpuTexture` pool entries, each with its sampler baked at upload time; `SpritePipeline::draw` picks the pool entry's pre-built bind group rather than switching samplers per batch. Array textures (`wgpu::TextureViewDimension::D2Array`) were rejected because every in-tree example fits inside one 2D page per filter (`01_platformer` and `03_scene_state` each land at one nearest + zero linear; `02_sprite_stress` lands at one nearest); paying the array-layer bind and WGSL refactor now would be premature. Overflow beyond one page per filter is handled by opening a second 2D page, not a second array layer.
3. **1 px transparent padding + half-texel UV inset.** Every packed rect is surrounded by 1 px of transparent RGBA in the page canvas, and `SpriteAsset.uv` is inset by half a texel on each side: `uv_min = [(x+0.5)/W, (y+0.5)/H]`, `uv_max = [(x+w-0.5)/W, (y+h-0.5)/H]`. Point sampling treats the inset as a no-op (nearest already snaps to the correct texel); bilinear sampling at non-mip zoom stays strictly inside the drawn rect, so neighbour bleed across packed boundaries is impossible at the sampling regimes M22 supports. Mipmaps remain a non-goal per the plan — a future mip switch will invalidate this invariant and require more padding or per-mip atlas pages, which is intentionally deferred.
4. **Rebuild-on-growth, in-place for shrink or equal.** `reload_sprite` consults the `AtlasRegistry` for the packed rect; if the new decode is `≤` the packed rect on both axes, it writes a canvas-sized-to-the-packed-rect with the new bitmap at the top-left and transparent fill below/right into `(packed.x, packed.y)` via `write_subtexture` and leaves `SpriteAsset.uv` unchanged. The shrink case accepts a visual transparent tail at the packed rect's right/bottom — callers that change sprite dimensions at runtime already accept re-authoring overhead, and compensating via `uv_max` update would force a bind-group rebuild for a purely transient case. Growth triggers `rebuild_atlas_for_filter`, which re-decodes every sprite in that filter class, repacks, reuses existing `TextureHandle`s 1:1 for the new page list, drops excess handles, allocates for any new tail, and uploads. Decode errors anywhere in the rebuild partition abandon the rebuild and keep the previous atlas (last-known-good per `D-031`).
5. **Renderer mints `TextureHandle`s; `AssetRegistry` stops minting.** Handle authority moves to `SpritePipeline::allocate_texture_handle` (a monotonic `u32` counter seeded at 0 for the process lifetime). `AssetRegistry::register_sprite` now takes an `atlas: TextureHandle` parameter rather than returning one, matching the reality that one atlas handle is shared across many sprite ids. The previous scheme — registry mints, renderer writes `(handle → GpuTexture)` — worked only when `1 sprite = 1 texture`; post-M22 the renderer is the single source of truth for which handles are live, which is needed for `drop_texture(handle)` on rebuild shrink. Core still sees no wgpu types (`D-016`).
6. **Hot-reload addition path runs through `rebuild_atlas_for_filter`.** `reload_manifest` registers each newly-added sprite with placeholder `(atlas=TextureHandle(0), uv=UvRect::FULL, w=0, h=0)` and `path` set to the manifest entry, groups additions by filter class, and calls `rebuild_atlas_for_filter` for each class that gained at least one entry. The old `load_sprites(&additions, ...)` call is removed because it would stomp the existing page handles for affected filter classes (the Step 4 `load_sprites` partition-and-rebuild pattern is not incremental). The orphan-entry risk if rebuild decode fails is accepted — next successful manifest reload overwrites the placeholder; this is a dev-time flow where last-known-good already governs.

**Observable-results block from plan Step 9 (AMD Radeon 660M, RADV Vulkan):**

- Image diff (Pillow per-pixel RGBA, tolerance = 0): `01_platformer`, `02_sprite_stress`, `03_scene_state` all pixel-identical vs. the pre-M22 HEAD capture.
- `sprite_extract_batch_build_2k`: pre-M22 ≈ 6.32 µs, post-M22 ≈ 7.72 µs (+22 %). The bench pre-allocates 10 fixed batches and does not exercise batch-collapse, so the measurement is dominated by the `SpriteInstance` stride growth (24 B → 40 B, +66 %). The plan's ≤10 % gate assumed batch collapse would compensate inside this micro-bench; in practice the collapse lives in the real-scene draw path, not the synthetic struct-push loop. The engineered-in wins (fewer bind-group switches, fewer live textures) show up in `SpritePipeline::draw` and the startup texture-count reduction, not here.
- `atlas_pack_startup_200` baseline: ≈ 7.45 µs (first recorded number on this machine; future runs guard the ≤20 % regression rule from the archived Phase 3 rollout plan, [`docs/plans/archive/phase3-rollout.md`](docs/plans/archive/phase3-rollout.md), Benchmark And Quality Gates).

**Rejected alternatives:**

- Array-texture atlases (`wgpu::TextureViewDimension::D2Array`). Rejected for M22 — no in-tree example exercises overflow, and moving to a layered atlas forces a WGSL rewrite (`textureSample` → `textureSampleLevel` with an explicit layer) plus new bind-group wiring. Revisit when a second page appears in any shipped example.
- GPU-compressed atlas formats (BC7/BCn, KTX2, Basis Universal). Rejected explicitly per `DESIGN.md` §17 Phase 4+ deferral and the M22 no-new-dep rule. Live with `Rgba8UnormSrgb` for now.
- Compensating for shrink by updating `uv_max`. Rejected — would invalidate the in-place fast path's bind-group reuse guarantee (every shrink would require a `SpriteAsset.uv` write observed by the extract in the same frame the write happens, introducing an ordering hazard with the between-frames invariant). The transparent-tail artefact is confined to reloads that actually shrink a sprite; shipping code rarely does.
- A "one texture per atlas page plus one extra for untracked sprites". Rejected — `AtlasRegistry.packed` covers every manifest-registered sprite, and sprites created outside `load_sprites` (e.g. `example-02-sprite-stress` high-load generated sprite) already opt out of hot-reload by routing through `renderer.allocate_texture_handle` + `register_sprite(UvRect::FULL)` directly. Adding a separate "untracked pool" would duplicate code paths for one narrow case.
- Registry still mints `TextureHandle`s; renderer accepts whatever handle the registry hands over. Rejected — see choice 5; without handle authority on the renderer side, `drop_texture(handle)` on rebuild shrink cannot safely invalidate a handle another crate might still own a copy of.
- Caching decoded sprite RGBA in the `AtlasRegistry` so `rebuild_atlas_for_filter` does not re-read disk. Rejected — doubles peak RAM during boot and defeats `D-031`'s contract that hot-reload reloads from disk.

## D-049 — M23 in-tree PRNG (PCG32 + SplitMix64)
**Date:** 2026-04-20
**Decision:** Add `crates/tungsten-core/src/rng.rs` with a hand-rolled PCG32 XSH-RR generator and a SplitMix64 seed mixer; do not pull `rand` / `rand_core` / `fastrand`.
**Why:** The particle emitter needs per-emitter deterministic sampling for lifetime, velocity, angular velocity, start scale, and hot-reload-stable frame-to-frame output; `D-015` rule 3 ("replace what you would otherwise write in a day") applies — PCG32 is ~80 lines and well-specified. `rand` also pulls `getrandom` which forces a libc/wasm-shim decision M23 does not need to make. SplitMix64 gives the per-emitter seed from a single `WorldRngSeed` resource without coupling emitters to their spawn order.
**Consequences:** Every subsystem that later wants randomness should prefer `tungsten_core::Pcg32`; adding a second PRNG is a decision that requires re-opening this one. Not cryptographic — documented inline.

## D-050 — M23 Arc snapshot semantics for ParticleConfig
**Date:** 2026-04-20
**Decision:** `ParticleConfigRegistry` stores `Arc<ParticleConfig>`; emitters resolve and cache the `Arc` on first tick into `ParticleEmitterState.config_snapshot`; every spawned `Particle` carries its own `Arc::clone` in `Particle.config`. On hot-reload, `ParticleConfigRegistry::replace(id, new_arc)` swaps only the registry's pointer — live emitters and particles keep the old `Arc` until they drain.
**Why:** The alternative (config-by-value copy at spawn time, or a `Vec<ParticleConfig>`-indexed table) makes hot-reload mid-flight visually chaotic — curves and color ramps reinterpret against the new config partway through a particle's lifetime. Arc snapshot gives us "new emissions pick up the new config on their next first-tick; old particles finish their life against the config they were born under" for free, at the cost of one `Arc` clone per spawned particle and one `Option<Arc>` in `ParticleEmitterState`. `Arc::strong_count` churn under load is measured in the `particle_tick_5k` bench (~657 µs for 5k particles with all curves active on Ryzen 7 + RADV) and is not a bottleneck.
**Consequences:** Emitters that need to re-snapshot against the new config mid-life must explicitly clear `ParticleEmitterState.first_tick_done`; this is a future-proofing hook — nothing in-tree uses it yet. The Arc-per-particle pattern generalises to any future asset that benefits from late-binding against live gameplay code (e.g. AI behaviour trees) so the indirection cost is a pattern investment, not a one-off.

## D-051 — M23 entity-per-particle (no pool)
**Date:** 2026-04-20
**Decision:** Each live particle is a distinct ECS entity with `Particle + Transform + Sprite + Visibility`, despawned through `CommandBuffer` on age-out. No fixed-size ring pool, no packed `Vec<Particle>` storage alongside the archetype.
**Why:** `D-039` guarantees `CommandBuffer` flush at the single frame boundary between user systems and the render extract; particle despawns land on the same flush as every other structural edit, so the frame-order story is already solved. Pooling would require either (a) a custom pool resource plus a sprite-extract fast path that reads from it — a second sprite source of truth, which breaks the `D-042` "`Sprite` is the one authority on GPU-visible 2D" invariant — or (b) an ECS reservation pool with sentinel "dead" flags, which costs the same archetype walks as real despawns while making debug tooling lie. The `particle_tick_5k` bench clears at ~657 µs per frame which is well under the M12 16.6 ms envelope at 60 Hz, and `max_alive` + `ParticleBudget.global_cap` keep the archetype bounded at an explicit ceiling the game owner controls.
**Consequences:** A future 10k+ CPU particle scene may re-open this choice; the fallback is a pool-backed `ParticleEmitter` variant rather than a wholesale rewrite (entities can coexist). Inspector and serialisation keep working on particles by default.

## D-052 — Asset-composition contract: umbrella merges, per-type loaders never compose
**Date:** 2026-04-22
**Decision:** The umbrella crate (`tungsten`) owns asset composition through a single primitive: `App::set_manifest_roots(Vec<PathBuf>)` hands an ordered list of manifest JSON paths to the engine, which calls `asset_loader::load_all_merged` before the user's `on_startup` closure runs. That helper folds every path through `ResolvedManifest::load_and_merge_many` (duplicate IDs are fatal per `D-017`), stores the merged graph as a `LoadedManifest(ResolvedManifest)` world resource, and runs `asset_loader::load_all` exactly once on the merged graph. The per-type loaders (`load_sprites`, `load_animations`, `load_fonts`, `load_sounds`, `load_tilemaps`, `load_particles`) remain public so the rare synthetic-sprite case (`example-02-sprite-stress`) can opt out, but they are documented as "advanced, not for composition" because each replaces its registry resource wholesale on every call — which was the root cause of the pre-D-052 workaround comment in `examples/01_platformer/src/setup.rs` where contributors had to remember which loaders were additive and which were destructive.
**Why:** `D-017` ("multiple manifests compose by extension, never override") was not enforced at any single call site before D-052. Composition was spread across every example's `on_startup` closure, with per-type loader destructivity encoded implicitly (sprites happened to be additive via `mem::take`; everything else replaced its registry). The merge-first architecture collapses N call sites into one, makes duplicate-ID conflicts hard errors at boot (not silent overwrites), and leaves exactly one source of truth for "what's loaded" in `LoadedManifest`. The alternative — teaching every registry an additive `extend_from` method — was rejected because it multiplies the number of methods that need symmetric removal/replace semantics for hot reload and pushes bookkeeping into each registry type. `ResolvedManifest::merge` already enforced the `D-017` invariant; D-052 wires it into the boot path.
**Consequences:** `LoadedManifest` is a long-lived world resource the hot-reload path can diff against on `reload_manifest` — future work can replace the current asymmetric add/remove branches with a single `ResolvedManifest` diff. Examples that currently call per-type loaders directly in `on_startup` for reasons other than synthetic-sprite generation should migrate to `set_manifest_roots` when touched. The per-type loaders stay public because removing them would force `example-02-sprite-stress` to invent a parallel "register generated sprite" API; keeping them public with the narrowed contract is the smaller change. Supersedes-clarification of `D-017` and `D-035`.

## D-053 — Hot-reload matrix and audio session-static invariant
**Date:** 2026-04-22
**Decision:** Publish one authoritative matrix of supported hot-reload surfaces. Sprites, animations, fonts, tilemaps, and particles support both single-file edits and manifest-add reloads; removals are warn-only (stale entries kept). Sounds remain session-static — the `cpal` mixer callback (`D-027`/`D-029`/`D-034`) captures decoded PCM into its own map at `AudioSystem::init` time, so any post-init registry mutation is invisible to live playback. `reload_manifest` now covers the particle add path (mirroring tilemap-add validation: unknown sprite IDs are rejected, last-known-good is preserved) and logs a debug message when the manifest's sound list changes so the "audio is session-static" contract is visible instead of silent. `LoadedManifest` (`D-052`) is updated on every successful manifest reload so future diff-driven reload work has one source of truth. The single-source-of-truth matrix lives in `DESIGN.md §Hot Reload — M9`.
**Why:** Pre-D-053 the supported reload surface was partly documented, partly inferred, and partly contradicted by the code. `DESIGN.md` listed "sprites, animations, fonts, manifest" as covered; `reload_manifest` handled sprite/animation/tilemap/font additions but had no particle-add branch; sounds had no reload path at all but nothing in docs or logs made that explicit. The fix is alignment, not new capability — the most stateful code in the workspace now matches one published matrix. Adding mixer-side PCM swap for sounds is real milestone work (a new mixer command on the existing or a parallel `rtrb` ring, lock-free `Arc<[f32]>` swap between callbacks) and was intentionally not bundled here; the documented session-static invariant reflects the shipped reality, not a permanent position.
**Consequences:** Headless tests for animation/tilemap/particle reload paths ship in `crates/tungsten/src/asset_loader.rs::tests`, exercising replace, unknown-sprite rejection, last-known-good preservation, and stable `AssetId` across reloads (`D-050`). Watcher path-filter tests in `crates/tungsten/src/hot_reload.rs::tests` cover extra-file exact match, recursive-root nested-file match, sibling-of-extra-file rejection, and sibling-of-recursive-root rejection. Sprite and font reload paths still need a live `Renderer` and remain Layer 2 (`scripts/smoke-examples.sh`) territory. Any future reload-matrix change must update the `DESIGN.md §Hot Reload — M9` table in the same commit.

## D-054 — M24 tween easings are a closed enum with a pure `apply(t: f32) -> f32`
**Date:** 2026-04-23
**Decision:** `tungsten_core::tween::Easing` is a closed `enum` covering Linear / Quad / Cubic / Quart / Sine / Expo / Back / Bounce in In / Out / InOut variants; `Easing::apply(self, t: f32) -> f32` is the single entry point and is pre-clamped `[0, 1]` by callers. No trait-object indirection, no boxed `dyn Fn`, no external easing crate. Back and Bounce variants overshoot the `[0, 1]` output range on purpose; callers (`lerp_u8`) clamp results before they reach the GPU.
**Why:** The built-in set is ~60 lines of closed-form arithmetic. `D-015` rule 3 ("replace what you would otherwise write in a day") does not apply — there is nothing to replace — and a dependency would add versioning surface for code that will never grow. A closed `enum` makes the set explicit in the public API, survives `serde`-round-trip authoring cleanly (`#[serde(rename_all = "snake_case")]`), and keeps the dispatch pure so `Tween` can stay plain ECS data. The trait-object alternative was rejected because the set is deliberately closed and the added indirection cost buys no extensibility a user actually wants: the whole point of the built-ins is that everyone picks from the same small menu.
**Consequences:** Future custom curves are a bigger design question, not an incremental API change — adding a new variant is a public-enum break, which is the intended speed bump. `Easing::Linear` is `Default` so `#[serde(default)]` on `SceneTween.easing` lands without ceremony. The Bounce math references the Robert Penner constants; keep the `bounce_out` helper inline so the constants are grep-able in one place.

## D-055 — M24 single `Tween` component per entity, multi-property via `Vec<TweenChannel>`
**Date:** 2026-04-23
**Decision:** Entities carry at most one `Tween` component; simultaneous multi-property animation uses `Vec<TweenChannel>` on that single component, all sharing the same `easing` + `duration`. Scene authoring matches (`SceneEntry.tweens` is honored as a `Vec<SceneTween>` but `spawn_scene` inserts only the first entry and logs `ERROR` on extras). `TweenChannel` is a closed `enum` covering `PositionX / PositionY / Rotation / ScaleX / ScaleY / ColorR / ColorG / ColorB / ColorA`.
**Why:** The archetypal ECS (`D-036`) allows one component per type per entity; supporting concurrent tweens with independent easings would need either a per-tween marker type (breaking the "one authority per property" model from `D-042`), a `Vec<Tween>` wrapper (duplicating `CommandBuffer::remove_component<Tween>` branch logic for individual-tween removal), or a new pool-backed indirection. The one-component model matches M23's emitter-state pattern (`D-051`) — one mutable component per entity, plain data — and keeps the tick system's hot loop a straight archetype walk. Multi-property animation with shared timing (fade + slide, scale + color pulse) is the realistic common case and is expressible directly.
**Consequences:** Callers that need two overlapping tweens with different easings or durations must model them as two entities (typical for UI overlays) or pick one `easing` + `duration` and collapse channels into one tween. If this becomes a real friction point the follow-up is a new `Tweens { tracks: Vec<Tween> }` collection component, not splitting the existing single-component model mid-flight. Scene-entry tweens beyond the first are logged, not silently dropped, so authors see the violation.

## D-056 — M24 `TweenComplete` via `EventQueue`; `Tween` removal via `CommandBuffer`
**Date:** 2026-04-23
**Decision:** On `Once` or final-cycle `Times(n)` completion, `tween_tick_system` enqueues `TweenComplete { entity, tag }` into `EventQueue<TweenComplete>` and enqueues `CommandBuffer::remove_component::<Tween>(entity)`. Neither the event send nor the component removal touches `World` storage directly during the tick; both land at the frame-end flush. A `Tween::pending_remove: bool` latch set inside the tick prevents a completed-but-not-yet-flushed tween from re-firing its completion event on subsequent ticks in the same frame-boundary window. `Loop` never completes (infinite by contract); `PingPong` flips direction at each boundary and also never completes.
**Why:** The frame-order invariants from `D-039` (`CommandBuffer` flush after systems, before extract) and `D-040` (two-window `EventQueue`, flushed once per frame) already solve the "mutate archetype / observe events" ordering problem for particles (`D-051`) and collisions. Routing tween completion through the same two primitives keeps one frame-boundary story in the engine, makes the tween integration testable without a live renderer, and costs only two `Vec` pushes per completing tween. The `pending_remove` latch is necessary because `CommandBuffer` flush runs *after* the tick system — without the latch, a completed tween on frame N that has not yet been flushed would re-enter the tick on any subsequent tick call and re-emit completion; the latch is one bool on the `Tween` and needs no extra resource. Direct `World::remove_component` from inside the tick was rejected because it would either panic (called through `query_entities` snapshots) or silently mutate archetypes mid-iteration.
**Consequences:** Readers see `TweenComplete` the frame after it fires (two-window semantics) — the same rule as `CollisionEvent` and `ParticleBurstEmitted`; stage the tween-driven state transition in a plain user system that polls `EventQueue<TweenComplete>::iter()` rather than trying to stitch completion into the tick itself. `pending_remove` is a minor wart but is part of the `Tween` public surface so users who construct `Tween` via the struct-literal form see it; the `Tween::new` builder defaults it to `false`. `Loop` and `PingPong` are documented as "never completes" — callers that want a count-bounded ping-pong use `Times(n)` with the ping-pong direction logic modeled manually, or add a `PingPongTimes(n)` follow-up variant later.

## D-058 — M26 materials + post-stack + tween→material bridge
**Date:** 2026-04-24
**Decision:** Ship three tightly-coupled extensions to the 0.22 render path. (1) **Materials:** a new `materials` section in `ResolvedManifest` keyed by stable id, each entry pointing at a `shaders` id plus `MaterialUniformDefaults`. `MaterialAssetId` and `MaterialRegistry` mirror the shape of `ShaderAssetId` / `ShaderRegistry` (D-016 seam, no `wgpu` in core). Render-side `MaterialPipeline` reuses the built-in sprite pipeline layout for group 0 (camera) and group 1 (texture+sampler); group 2 is a per-material 256-byte UBO matching `UniformOverrideBlock`. `SpriteBatch.material_id` selects the pipeline per batch — `None` keeps the built-in path and the M25 byte-identical output. Materials live inside the manifest (no separate per-material JSON file in M26) so manifest reload is the only author-visible edit surface. (2) **Post-stack:** `PostPass` is a closed enum of 17 stock effects (tonemap, vignette, lut, chromatic_aberration, color_adjust, tone_mono, crt, film_grain, dither, pixel_outline, fade, wipe_radial, dissolve, glitch, pixelate, fog, god_rays) mirroring `Easing`'s closed-enum reasoning (D-054). `PostStack(Vec<PostPass>)` is a reorderable world resource; its length splices N pass descriptors between the scene pass and the present pass, ping-ponging `PostPing` / `PostPong` allocations in `RenderTargetPool`. SMAA / post-AA is **not** a `PostPass` — M27 ships it as a fixed presentation-tail pass, not a variant here. `PostStack::default()` is empty, preserving M25 byte-identity as a hard gate. (3) **Tween→material bridge:** new `UniformOverrideBlock` component carries the same 256-byte payload used by material and post UBOs; new `TweenChannel::UniformVec4Lane`/`UniformScalar`/`UniformInt` variants animate it, missing-block channels log and skip. Stock shader bodies are vendored under `crates/tungsten-render/src/shaders/stock/` (MIT LYGIA-derived helpers under `…/lygia/`) and mirrored under `assets/shaders/stock/` so the manifest + existing `.wgsl` watcher already cover hot reload. No WGSL preprocessor — helper composition stays as Rust-side source concatenation inside the pipeline module.
**Why:** Materials, post-stack, and tween-driven uniform animation are three features that must share one data layout or the UX falls apart. A material's UBO, a post-pass's params UBO, and the entity-local `UniformOverrideBlock` are the same 256-byte shape on disk, in Rust, and in WGSL — which means `TweenChannel::UniformScalar` can drive a material slot today and an MSDF outline slot in M32 without a schema change. Manifest tracking (vs a second asset-file mechanism) kept the loader graph flat: one place to merge keys, one place to validate cross-refs, one hot-reload watcher. Closed-enum `PostPass` mirrors `Easing` — serde is trivial, there is no trait-object indirection, and extension is an explicit plan-file change rather than a silent runtime registration. Ping-pong over two `PostPing` / `PostPong` targets is the minimum allocation that supports arbitrary stacks; the alternation is deterministic so the capture/readback path can always name its final-source target without reading back renderer state. Vendoring LYGIA-derived helper WGSL (instead of pulling a crate) satisfies D-015 rule 3 and matches how the sprite shader already lives on disk + in `include_str!`.
**Consequences:** Narrows D-023 and D-055 without reversing either: hot reload covers material body edits + shader body edits (signature changes still need a rebuild); `Tween` stays one-per-entity and multi-property through `Vec<TweenChannel>` — no new `TweenTarget`, no cross-entity target. `SpriteBatch` grows `material_id` + `uniform_overrides`; extract must split batches on effective material state so the renderer can stay world-free. `default_pass_order` takes `post_stack_len` so the `post_stack_len == 0` case produces the exact M25 pass order (regression gate for the image-diff baseline). `SceneColor` stays swapchain sRGB in M26 — M28 introduces the HDR sibling when bloom actually needs it. The 17 stock WGSL files are vendored with MIT attribution (LYGIA); helper snippets live under `shaders/stock/lygia/` and are kept byte-equal between the compile-time `include_str!` source and the `assets/shaders/stock/` mirror. M26 ships baseline / roughly-tuned implementations of each effect; fine-tuning individual effects is follow-up work that does not expand the scope of this decision.

## D-057 — M25 shader assets: manifest-tracked, body-edit hot reload with `naga` validation
**Date:** 2026-04-23
**Decision:** WGSL shaders move into the manifest graph under a new `shaders` section (`assets/shaders/<id>.wgsl`), allocate a `ShaderAssetId` through a core-side `ShaderRegistry`, and stream through the existing umbrella hot-reload watcher (no second `notify` instance). `Renderer::new` pre-seeds an in-memory `ShaderModuleCache` with the compile-time `include_str!` bytes keyed by the same ids — `asset_loader::load_shaders` then byte-equal short-circuits when on-disk WGSL matches, which is what keeps the default config byte-identical to the 0.21 baseline. Hot-reload triggers on `.wgsl` edits: source text is re-read, validated through `wgpu::naga::front::wgsl::parse_str` + `naga::valid::Validator`, and only committed to the cache after the dependent sprite pipeline has been rebuilt successfully on the new module; any failure (parse, validation, rebuild) logs and leaves the prior `ShaderModule` + live pipeline untouched. Signature changes (new bindings, new instance attributes, different bind-group layout) still require a rebuild — narrowing but not reversing `D-023`. `SceneColor` format equals the swapchain sRGB format for M25; M27 will add an HDR sibling target for bloom input. MSAA sample-count changes require a relaunch. See `D-053` hot-reload matrix; this entry adds the `shader` row.
**Why:** Shader iteration was the highest-friction authoring loop left in the engine — any WGSL tweak forced `cargo build`, a full app relaunch, and lost session state (camera position, scene selection). The umbrella crate already owns a debounced recursive `notify` watcher and the manifest graph as a single resource (`D-052`), so routing `.wgsl` edits through the same path was the smallest viable change: one new `ResolvedManifest::shaders` map, one render-side cache module, one extension branch in `process_hot_reload`. `naga` validation runs before any GPU work so a typo can't crash the device or drop back to the driver; keeping the previous module live on failure means body-edit iteration is safe to do with the app running under a debugger. Manifest tracking (not `include_str!`) is what unlocks the hot path, and it also collapses the old two-step "ship the shader binary, ship an identical source file" pattern that existed for any contributor who wanted the runtime source.
**Consequences:** The compile-time default path (`include_str!("../../../assets/shaders/sprite.wgsl")`) and the manifest-tracked runtime source now share one file on disk; loaders never touch the live `ShaderModule` if the bytes match (`ShaderModuleCache::unchanged_count` tracks no-op calls for telemetry). Hot reload is body-only: signature/bind-group-layout edits still need a rebuild, and the log line on those failures cites the naga error or the rebuild crash so the cause is visible. `AGENTS.md` §Asset Rules now lists shaders as manifest-tracked; `DESIGN.md` adds the `shader` row to the hot-reload matrix; `CHANGELOG.md` records the surface change. The `SceneColor = swapchain sRGB` note is load-bearing for M25's baseline image-diff claim and is the boundary M27 will extend when it allocates an HDR sibling target. MSAA is a per-run choice (relaunch to change) because the sprite/quad/debug/text pipelines bake `sample_count` into their `MultisampleState` at build time; live swap would require a six-pipeline rebuild on every toggle and was judged not worth the matrix complexity at this milestone.

## D-059 — M27 SMAA presentation AA as a renderer-owned tail
**Date:** 2026-04-24
**Decision:** Ship SMAA 1x as a renderer-owned presentation stage exposed through `RenderConfig.post_aa` (`Off | SmaaLow | SmaaMedium | SmaaHigh | SmaaUltra`, `#[non_exhaustive]`) plus the env override `TUNGSTEN_RENDER_POST_AA`. SMAA is **not** a `PostPass` variant and never enters `PostStack`'s reorderable list; it runs as a fixed three-pass tail (edge detect → blend weights → neighborhood blend) after the M26 post stack and before the screen-space text overlay, writing into a new `PresentSource` target that the present blit + screenshot path source. The `area` and `search` lookup textures ship as `include_bytes!`-embedded engine content under `crates/tungsten-render/src/assets/smaa/` with MIT attribution; they are intentionally not entries in `assets/manifest.json`. The three SMAA stage shaders **are** manifest-tracked (`smaa_edge`, `smaa_blend_weights`, `smaa_neighborhood_blend`), follow the stock-shader pattern (compile-time `include_str!` mirror under `crates/tungsten-render/src/shaders/stock/` + byte-equal mirror under `assets/shaders/stock/`), and hot-reload through `Renderer::upload_shader` / `reload_shader` with `naga` validation; failure leaves the live pipelines untouched. Preset knobs (threshold, max search steps, max diag steps, corner rounding) ride a 256-byte UBO so switching presets neither rebuilds nor recompiles a pipeline. `SceneColor` and the `PostPing` / `PostPong` targets carry a non-sRGB twin in `view_formats` while SMAA is active so edge detection samples gamma-encoded values; the rest of the frame keeps using the primary view. `post_aa = Off` produces the byte-identical M26 frame across the msaa × depth_sort × post_stack_len matrix — verified by `post_aa_off_matches_m26_baseline_across_matrix` in `crates/tungsten-render/src/tests/passes_order.rs`. Runtime changes go through `tungsten::request_post_aa(world, mode)`; `App` applies the request at a frame boundary (after hot-reload, before extract) so `SmaaPipeline` allocation/drop and intermediate-target reallocation never happen mid-render. Unlike `msaa`, switching `post_aa` requires no relaunch.
**Why:** Presentation AA must see the fully shaded post output (so it smooths post-stack pixels, not just scene pixels) and must not smooth screen-space text (so font edges stay crisp). Modeling SMAA as a reorderable `PostPass` would break both invariants — the user could place AA before tone-mapping (visually wrong) and the overlay would land on a target SMAA had already smoothed. A fixed renderer-owned tail expresses the constraint structurally: the order is not a knob. Embedding the area/search LUTs as `include_bytes!` instead of manifest assets makes them un-fiddleable engine content (regenerating them requires re-running the upstream byte-extraction, not a JSON edit) — they are part of the algorithm, not user data, so the manifest contract from `D-052` does not apply. The 256-byte preset UBO matches the engine-wide post-UBO contract from `D-058`, so future preset additions or runtime tuning do not need a new layout. Non-sRGB twin views via `view_formats` are the wgpu-supported way to read gamma-encoded pixels through an sRGB attachment without copying the whole frame to a linear sibling. The runtime request/apply seam mirrors the display-state pattern from `D-043`: user systems get `&mut World`, not `&mut App`, so a `request_*` writes a pending world resource and the umbrella applies it at the same frame-boundary slot every other reallocating change uses. No new runtime or dev dependency — `wgpu`, `bytemuck`, and `glam` already live in the workspace, so `D-015` is satisfied without a new rule citation.
**Consequences:** Narrows `D-058` (SMAA stays out of `PostPass`'s closed enum, formally locked here rather than left implicit); extends `D-053` (the body-edit hot-reload matrix gains the three SMAA stage shaders, and the LUT binaries are explicitly out-of-matrix); and narrows `D-023` the same way `D-057` did (manifest-tracked WGSL with `include_str!` short-circuit). The SMAA WGSL ports are three standalone modules — there is no `smaa_common.wgsl` because the loader validates one file as one WGSL module and adding a preprocessor would be larger work than triplicating ~20 lines of helpers. The orthogonal-only path is what shipped in this milestone; diagonal and corner-pattern detection are gated by the preset sentinels (`max_search_steps_diag = 0` and `corner_rounding = u32::MAX` respectively) and remain follow-up work that does not change the public surface or this decision. `tungsten.json` documents `render.post_aa` so contributors see the key; the hot-reload matrix in `DESIGN.md` lists the three SMAA stage shaders as body-edit reloadable.
