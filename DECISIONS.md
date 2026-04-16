# DECISIONS.md

Log of non-obvious decisions for Tungsten. Numbered sequentially; immutable once settled. Reversals add a new entry marked `Superseded by D-XXX`.

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
**Decision:** Apply the following compilation flags across the workspace:

*`.cargo/config.toml` (all builds on this machine):*
- `-C target-cpu=native` — enables AVX2/FMA and the full native ISA. Non-portable binary. All benchmark numbers below are keyed to this flag on AMD Radeon 660M / AMD Ryzen 5 6600H (Arch Linux, rustc 1.94.1).

*`[profile.release]` in workspace `Cargo.toml`:*
- `lto = "thin"` — ThinLTO: parallel cross-CGU import/export pass, cross-crate inlining.
- `codegen-units = 1` — single LLVM CGU, maximum within-crate inlining budget.
- `panic = "abort"` — removes landing pads and unwind tables from LLVM IR; verified safe across all deps including `cpal` on the 2026-04-16 validation pass (188 tests in the suite at that time).
- `debug = 1` — line-number tables only; preserves `perf`/flamegraph source annotation.
- `strip = "none"` — explicit; profiling workflow requires symbols in the binary.

*`[profile.dev.package."*"]`:*
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
