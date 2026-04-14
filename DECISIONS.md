# DECISIONS.md

Log of non-obvious decisions for Tungsten. Numbered sequentially; entries are compact by design. When a decision is reversed, add a new entry and mark the old one `Superseded by D-XXX` — old numbers are never reused. Prior prose was trimmed in an editing pass; the *settled answer* for each D-NNN is preserved.

---

## D-001 — Project name: Tungsten
**Date:** 2026-04-07
**Decision:** Project name is Tungsten; crate prefix `tungsten-`; umbrella crate is `tungsten`.
**Why:** Dense element, not already taken by a prominent Rust project.

## D-002 — Framing: hobby project, fun first
**Date:** 2026-04-07
**Decision:** Optimize every process and architectural decision for "will I want to come back to this on a Saturday." Most rules are soft; no CI gate, no mandatory docs, no self-review checklist. Judgment over compliance.
**Why:** The biggest risk isn't technical debt, it's quiet abandonment. Heavy process kills hobby projects.
**Consequences:** Judgment over compliance. Reassess milestones explicitly rather than abandon quietly.

## D-003 — Native only, no WASM
**Date:** 2026-04-07
**Decision:** Native targets only (Linux, macOS, Windows).
**Why:** Supporting WASM constrains dependency choices and doubles the test matrix.
**Consequences:** Free use of any native-only crate; revisiting would be nontrivial.

## D-004 — wgpu as renderer
**Date:** 2026-04-07
**Decision:** Use `wgpu`.
**Why:** Cross-platform GPU API at a manageable level. `ash` is too much yak-shaving for triangles; `glow`/OpenGL is dated.
**Consequences:** Renderer is a wgpu wrapper. If wgpu proves too painful, fallback is `pixels` or `macroquad`, not `ash`.

## D-005 — Hand-rolled ECS, no external crate
**Date:** 2026-04-07
**Decision:** Build the ECS by hand. Naive `HashMap<TypeId, ...>` first; evolve on real pain. No external ECS crate, ever.
**Why:** ECS is one of the main things this project is here to teach; using `bevy_ecs` or `hecs` defeats the purpose.
**Consequences:** Early performance will be poor. If the naive version stays good enough forever, that's success, not failure.

## D-006 — Cargo workspace, three crates
**Date:** 2026-04-07
**Decision:** `tungsten-core` (ECS, math, config, time, resources, asset registry types), `tungsten-render` (wgpu wrapper, sprite drawing, samplers), `tungsten` (umbrella + winit app loop). Split further only when a crate becomes genuinely unwieldy.
**Why:** An earlier draft proposed six crates; over-splitting for this size.

## D-007 — `tungsten-render` may know `tungsten-core` types
**Date:** 2026-04-07
**Decision:** `tungsten-render` may depend on `tungsten-core` and use its types where convenient.
**Why:** Strict separation was a contrarian bet for a solo hobby project; Bevy couples them for good reasons. Simpler glue code in M3+.

## D-008 — Config is JSON, loaded once at startup
**Date:** 2026-04-07
**Decision:** Single `tungsten.json` at workspace root, loaded once via `serde_json`, validated, passed by value. No global, no hot reload. Missing → defaults with warning. Invalid → fatal naming the bad field.
**Why:** Engine-level parameters shouldn't require recompilation. TOML/RON add no decisive value here.
**Consequences:** `serde` and `serde_json` are Phase 1 deps.

## D-009 — Manifest-driven assets, ID-referenced
**Date:** 2026-04-07
**Decision:** `assets/manifest.json` registers every asset by string ID. Game code references assets by ID, never by path. Manifest paths are relative to the manifest file. Validation at load time catches missing files and unresolved references.
**Why:** Decouples code from file layout. Indirection is the architectural prerequisite for hot reload.
**Consequences:** Slight ceremony to add assets; renaming files is a manifest edit, not a code change.

## D-010 — Custom JSON animation format
**Date:** 2026-04-07
**Decision:** Roll a small JSON schema: `{ looping: bool, frames: [{sprite: id, duration_ms: u32}] }`. Each animation lives in its own file under `assets/animations/`, registered in the manifest.
**Why:** Aseprite's export schema would lock the project into a third-party format for no learning payoff. Per-frame durations keep emphasis frames possible.

## D-011 — Per-sprite filter mode in the manifest
**Date:** 2026-04-07
**Decision:** Filter mode is a per-sprite manifest property, `nearest` (default) or `linear`. The renderer creates one sampler per filter mode and binds the right one per sprite.
**Why:** A global setting can't mix pixel art and high-res UI in the same scene.
**Consequences:** Two samplers live in the renderer. Future blend/wrap/mipmap fields can be added without breaking changes.

## D-012 — Hot reload deferred to Phase 2
**Date:** 2026-04-07
**Decision:** M5 ships without hot reload. Hot reload is a Phase 2 milestone (now M9, shipped). M5 must preserve the registry-by-ID invariant.
**Why:** Scope risk on an already-large M5; the cost/value ratio is much better once the indirection is in place.

## D-013 — Asset directory layout: by-type at workspace root
**Date:** 2026-04-07
**Decision:** Shared `assets/` at workspace root, organized `sprites/`, `animations/`, `sounds/`, `fonts/`, with `manifest.json` at its root. Examples that need throwaway assets ship `examples/NN_name/assets/` with a local manifest. The loader takes a manifest path.
**Why:** Manifest sections match folder structure; adding a new asset type later is a new directory plus a new manifest section.

## D-014 — Asset registry is a Resource in the World
**Date:** 2026-04-08
**Decision:** The asset registry is a `Resource` in the World, accessed by the same mechanism as `DeltaTime` and `InputState`.
**Why:** Avoids a second "global-ish" pathway in a design that's trying to have exactly one. Static/singleton is ruled out by the no-global-mutable-state rule.
**Consequences:** If the World is dropped and recreated, the registry and its opaque handles die with it, while the renderer remains responsible for the actual `wgpu` resource lifetime.
**Known limitation (teardown order):** Drop order between World and the renderer's wgpu resource pools is not explicitly guaranteed. For the current hobby scope the process exits via OS kill; no explicit teardown is required. A multi-scene or long-lived-window design would need either documented drop order (World before renderer) or explicit pool invalidation on World drop to avoid GPU resource leaks.

## D-015 — Dependency philosophy: three acceptance rules
**Date:** 2026-04-08
**Decision:** A dependency is acceptable if at least one of these applies:
1. **Platform API abstraction** — it wraps OS-specific code I'd otherwise write per platform (`winit`, `wgpu`, `notify`, `cpal`).
2. **Well-specified data format** — it parses a format that isn't the interesting part of what I'm building (`serde_json`, `image`, `symphonia`).
3. **Math/primitive** — solved problem, not architecture (`glam`, `bytemuck`).

A crate that hands me something the project is supposed to teach me to build is not acceptable (any ECS crate, any higher-level engine, any rendering helper). Gray-zone crates get their own decision log entry.
**Consequences:** Every dep entry below references these rules by number.

## D-016 — `tungsten-core` stores opaque asset handles, not `wgpu` types
**Date:** 2026-04-08
**Decision:** `tungsten-core` stores opaque runtime asset handles (newtypes/IDs). `tungsten-render` owns GPU textures, samplers, and other `wgpu` resources in internal pools keyed by those handles.
**Why:** Leaving "handle" underspecified risked leaking `wgpu` types into core or scattering ad hoc ownership decisions.
**Consequences:** Core stays free of `wgpu` types; the registry remains the one game-facing lookup path.

## D-017 — Multiple manifests compose by extension, never override
**Date:** 2026-04-08
**Decision:** Multiple manifests compose by extension only. Asset IDs must be globally unique across the merged set; duplicates are fatal at load time. Each path resolves relative to its declaring manifest. Later manifests may reference earlier IDs but not replace them.
**Why:** Avoids implicit last-wins semantics and keeps example-local manifests from silently shadowing shared content.

## D-018 — Phase 1 rendering extracts plain data before drawing
**Date:** 2026-04-08
**Decision:** Systems mutate the `World` during `tick`; the app then extracts POD render data (`QuadInstance`, `SpriteInstance`, `TextSection`) into temporary buffers and passes slices into `tungsten-render`. The renderer may read the asset registry for ID resolution but doesn't require long-lived mutable World access at draw time.
**Why:** Keeps borrow-checker pressure contained and preserves a direct-data render API for testing.

## D-019 — `pollster` for blocking on wgpu async init
**Date:** 2026-04-12
**Decision:** Use `pollster` v0.4 to block on wgpu's `request_adapter`/`request_device` during init.
**Why:** wgpu v29 exposes these as async; the frame loop is synchronous. `pollster` is ~50 lines, zero deps, single purpose. Satisfies D-015 rule 3.
**Rejected:** `futures::executor::block_on` (heavier); hand-rolled executor (not worth it); `tokio`/`async-std` (ruled out).

## D-020 — `bytemuck` for GPU data layout
**Date:** 2026-04-12
**Decision:** Use `bytemuck` v1 with the `derive` feature. All GPU-uploaded structs derive `Pod` and `Zeroable`.
**Why:** Vertex/instance buffers need safe `&[T]` → `&[u8]` casting. Satisfies D-015 rule 3 (solved primitive).

## D-021 — Entity ID is `u32`
**Date:** 2026-04-12
**Decision:** Entity ID is `u32`. No generational index in Phase 1.
**Why:** ~4B entities is plenty for 2D; generational indices add complexity that isn't paying off yet.
**Consequences:** Upgrade to generational index only if despawn/respawn aliasing bugs appear.

## D-022 — ECS error strategy: panic vs Result
**Date:** 2026-04-12
**Decision:** Panic on programmer errors (insert on a dead entity, wrong type downcast). Return `Option`/`Result` on runtime conditions (entity not found, component not present).
**Why:** All-Result is too noisy for game code; all-panic is too fragile for runtime lookups.
**Consequences:** `World::insert` asserts the entity is alive; `World::get` returns `Option`.

## D-023 — WGSL shaders embedded via `include_str!`
**Date:** 2026-04-12
**Decision:** Shaders are separate `.wgsl` files in `tungsten-render/src/`, pulled in at compile time with `include_str!`.
**Why:** Standalone files for editing/highlighting; no runtime file loading for Phase 1.
**Consequences:** Shader changes require recompilation. A Phase 2 hot-reload fallback could add runtime loading without losing the baked-in default.

## D-024 — Phase 1 exit observations for Phase 2 planning
**Date:** 2026-04-12
**Decision:** Phase 1 (M0–M6) exit observations, used as inputs to Phase 2 planning:
1. **Text:** `glyphon` (built on `cosmic-text`) is the pick. Fonts already staged in `assets/fonts/`.
2. **ECS performance:** Naive storage works fine at Phase 1 scale. Archetypal rewrite is learning-motivated, not a prerequisite.
3. **Audio:** Deferred to M8. `symphonia` likely fine; mixer vs `kira` needs its own decision.
4. **Hot reload:** The M5 registry-by-ID invariant holds; no game code holds direct GPU handles. `notify` is the planned file-watcher.
**Why:** `DESIGN.md` required a "stop and reassess" at the Phase 1 boundary.

## D-025 — License: MIT
**Date:** 2026-04-12
**Decision:** MIT. `LICENSE` at repo root; `license = "MIT"` in workspace `Cargo.toml`.
**Why:** Simple, permissive, ecosystem-standard. Apache-2.0's patent clause adds no benefit for a solo hobby project; dual MIT/Apache-2.0 is overhead for a repo not published to crates.io.

## D-026 — `glyphon` + `cosmic-text` for text rendering
**Date:** 2026-04-12
**Decision:** Use `glyphon` (pulls in `cosmic-text`, `swash`, `fontdb`) for M7 text rendering. Satisfies D-015 rule 2 (TrueType/OpenType is a well-specified format).
**Why:** Font parsing, shaping, layout, and GPU rasterization are a multi-month side quest that teaches font internals, not engine architecture. Purpose-built for wgpu.
**Consequences:** Currently a git dep pinned to a specific commit on `main` because glyphon 0.10.0 on crates.io requires `wgpu ^28.0.0` and this project is on wgpu 29. Pin to a crates.io version once a wgpu-29 release ships.
**Known fragility:** A branch pin (`branch = "main"`) is more fragile than a commit hash — upstream can advance and silently break builds for old commits. This plan pins to a specific rev; the pin should be updated to a crates.io release once one ships.

## D-027 — `cpal` for audio device access
**Date:** 2026-04-13
**Decision:** Use `cpal` v0.15 for audio output. Satisfies D-015 rule 1 (platform API abstraction: WASAPI/CoreAudio/ALSA).
**Why:** Writing three OS audio codepaths has no learning payoff for engine architecture.
**Consequences:** The `cpal` callback thread is the only background thread in the engine; game code communicates with it via `mpsc`. `cpal` is a dep of `tungsten` only.

## D-028 — `symphonia` for audio decoding
**Date:** 2026-04-13
**Decision:** Use `symphonia` v0.5 with features `ogg`, `wav`, `mp3` for eager load-time decode into `Vec<f32>` PCM. Satisfies D-015 rule 2.
**Why:** Vorbis/WAV/MP3 are well-specified formats; decoding them is a side quest.
**Consequences:** Supported formats: OGG Vorbis, WAV, MP3. No `symphonia` types appear at runtime in the audio callback. Dep of `tungsten-core` (decoding during asset load) and transitively `tungsten`.

## D-029 — Hand-rolled audio mixer, no `kira`
**Date:** 2026-04-13
**Decision:** Hand-roll the mixer as a closure owned by the `cpal` callback in `tungsten/src/audio.rs`. Feature set: play/stop/loop, master volume, per-sound volume. No DSP effects, envelope curves, or spatial audio.
**Why:** `kira` fails all D-015 rules — the mixer (cpal callback contract, sample-level PCM, loop/one-shot state machine, `mpsc` command passing) is exactly what M8 is here to teach. ~150 lines.
**Rejected:** `kira` (hands me the mixer); `rodio` (bundles decoder and device — even more opinionated).
**Known limitation (device format):** The mixer assumes the cpal output device operates at the same sample rate and channel count as the decoded PCM (44.1 kHz stereo f32). Format mismatches are not detected or converted at runtime. This is acceptable for the hobby scope: all target audio files are 44.1 kHz stereo. If cpal cannot open a stream with the requested format, it returns an error at stream-open time; the engine will panic with a clear message rather than silently producing wrong-pitch or clipping audio.
**Note (command channel):** The original implementation used `std::sync::mpsc::try_recv` in the callback thread. This was replaced with a wait-free SPSC ring buffer (`rtrb`) in D-034 to eliminate the theoretical RT-unsafe allocation.

## D-030 — M12 ECS rewrite is conditional
**Date:** 2026-04-13
**Decision:** M12 (archetypal ECS rewrite) is conditional. After M11, assess whether the naive ECS has caused measurable friction; if not, skip M12 and go directly to M13. A new `DECISIONS.md` entry is required before M12 begins (either confirming need or explicitly descoping).
**Why:** Phase 1 and M7 exercised the naive ECS with zero performance pain. D-005 already said "naive forever is success, not failure." Committing M12 up front is premature optimization.
**Consequences:** `v0.7.0-alpha` may be skipped; `v1.0.0` (M13) is unblocked by this decision.

## D-031 — `notify` for file watching (hot reload)
**Date:** 2026-04-13
**Decision:** Use `notify` v6 with `default-features = false`. `RecommendedWatcher` auto-selects the backend per platform (inotify / FSEvents / ReadDirectoryChanges). Events cross threads via `std::sync::mpsc`. A 50ms debounce in main-thread polling collapses editor double-writes. Satisfies D-015 rule 1.
**Why:** Avoids three OS file-watching codepaths.
**Consequences:** `notify` is a dep of `tungsten` only. The watcher thread is a second background thread alongside the `cpal` audio callback; game logic stays single-threaded.

## D-032 — M10 tilemap shape (format, pipeline reuse, camera default)
**Date:** 2026-04-13
**Decision:** Three coupled choices that define M10's shape:
1. **`.tmj` extension for tilemap JSON, Tiled-compatible schema.** Distinct from animation `.json` so the hot-reload dispatcher in `App::process_hot_reload` can route on extension alone. `notify` events only carry paths; content-sniffing to distinguish animation JSON from tilemap JSON would be a strict loss. **Supersedes the original custom schema:** the loader was updated to parse Tiled's standard `.tmj` JSON format verbatim (width/height/tilewidth/tileheight/layers[]/tilesets[]). Collision-layer metadata is carried via Tiled's custom `properties` array on layers. This unblocks standard off-the-shelf Tiled editor use for M13 content creation.
2. **Tilemaps reuse the sprite pipeline.** `extract_tilemaps(&World)` resolves visible tiles into `SpriteBatch`es keyed by texture handle and returns them in layer order. The sprite pipeline draws them with zero changes. No new wgpu pipeline, no new shader, no new bind-group layout. Preserves D-007 (core/render seam): `tungsten-core` holds `TilemapData`/`TilemapRegistry`/`TilemapInstance` (plain data, no wgpu types); the umbrella crate's free function is where the AABB→tile-grid culling happens. Game code uses the D-018 direct-data API through `set_extract_sprites`, giving the caller control over ordering vs. entity sprites.
3. **`Camera2D` default preserves pre-M10 behavior.** The new `Camera2D` resource (position top-left, zoom) produces its view-projection via `Mat4::orthographic_rh(pos.x, pos.x+w/zoom, pos.y+h/zoom, pos.y, -1, 1)`. At the default (position zero, zoom 1.0) this is the exact matrix the sprite pipeline built internally in M7–M9, so examples 01–08 are pixel-identical without being touched. A unit test in `camera.rs` asserts the equivalence.
**Why:** The three decisions together keep M10 to pure additive work. No existing example or downstream code needed to change; all three crates compile with the new signatures behind defaults that match the old behavior. The `.tmj` split specifically buys clean hot-reload dispatch with zero parsing overhead.
**Consequences:**
- Text (glyphon) deliberately does **not** consume `Camera2D` — HUD/UI stays screen-space while the world scrolls. Documented on the `Camera2D` type.
- `SpritePipeline::update_camera` and `QuadPipeline::update_camera` now take `&Mat4` instead of `(width, height)`; the umbrella crate computes the matrix each frame from `Camera2D` + `WindowSize`.
- No new runtime dep (D-015 entry not required — `glam::Mat4` is already in use).
- A non-rendering `LayerKind::Collision` is accepted by the loader and round-trips through the registry but is skipped by `extract_tilemaps`. This is the M11 seam — M11 will read it directly without format changes.

## D-033 — M11 physics shape (hand-rolled, uniform grid, AABB+circle only)
**Date:** 2026-04-14
**Decision:** Ship M11 as a hand-rolled 2D collision subsystem inside `tungsten-core::physics`. Four coupled choices:
1. **No external physics crate.** `rapier2d`, `box2d`, `parry2d` are all rejected — they fail D-005 ("build the ECS / game loop yourself") and D-015 (the physics is the point, not a solved problem we want to abstract over). Target is game-jam grade, not Box2D parity: no joints, no continuous collision detection, no polygon SAT, no rotation.
2. **Uniform spatial grid broad-phase, rebuilt each substep.** `HashMap<IVec2, Vec<ProxyId>>` keyed on `floor(pos / cell_size)`, cell size tunable via `PhysicsConfig::broadphase_cell_size` (default 32.0). No persistent state between frames — rebuild-from-scratch is cheap at Phase 2 entity counts and sidesteps incremental-update bugs. The SAT generalization is documented in `collision.rs` as a learning note rather than implemented, because M11 has no rotating shapes.
3. **`Position` and `Velocity` live at library level in `tungsten-core::physics`, not migrated into existing examples.** Examples 02–04 keep their local `Position { x, y }` structs untouched; only physics-aware code (example-10 onward) uses the library types. Avoids scope creep into an existing-examples migration while still giving the library a canonical component shape.
4. **Tilemap colliders are transient.** The physics step walks every `TilemapInstance` and emits one static AABB per non-negative tile on any `LayerKind::Collision` layer, fresh each substep. No parallel registry of "baked" collision geometry — this means hot-reloaded collision layers take effect on the next frame with zero extra machinery, at the cost of re-scanning the tile grid each step. `CollisionEvent.b = None` marks tile contacts (tiles have no entity id).
**Why:** The hand-rolled direction is forced by D-005 and D-015; the other three choices are the simplest thing that exercises the acceptance criteria without building engine machinery we can't justify yet. Uniform-grid broad-phase is one file, the narrow-phase is three pairwise tests, and MTV resolution is a handful of lines — all of this is the learning goal of M11.
**Consequences:**
- `CollisionEvents` is a resource populated each step; game code reads it for ground detection / triggers (example-10 demonstrates the pattern). `normal` points from `a` into `b`'s free space consistently across all three shape helpers.
- Substep count is chosen per frame as `ceil(max_dynamic_speed * dt / min_half_extent)` capped at `PhysicsConfig::max_substeps` (default 8). Guards against tunneling without needing swept CCD.
- `PhysicsConfig::gravity` defaults to `Vec2::ZERO` so top-down games pay no physics tax; example-10-platformer overrides it at startup. No crate-level "physics feature flag" — unused if the user never registers `physics_step`.
- D-015 not triggered: no new runtime dep (glam is already in; `HashMap` is std).
- `physics_step` is a plain system registered by the user via `app.add_system(physics_step)`. The frame loop doesn't hard-wire it, keeping ordering under game-code control (input systems before, gameplay reads events after).
**Known limitation (tilemap collider cost):** Tilemap collision proxies are regenerated per substep (transient by design, see point 4). With 8 substeps and a 256×256 map this is ~512 K AABB checks per frame. **Tile-count budget for current implementation: ≤128×128 maps.** Larger maps should pre-bake a static spatial index rebuilt only on hot reload. This is a M13 concern; no current example exceeds the budget.

## D-034 — Lock-free SPSC ring for the audio command channel
**Date:** 2026-04-14
**Decision:** Replace `std::sync::mpsc` in the `cpal` audio callback with `rtrb` v0.3, a wait-free single-producer single-consumer ring buffer. Ring capacity: 64 commands. Drain capped at ring capacity per callback invocation (bounded by construction).
**Why:** `std::sync::mpsc::try_recv` is non-blocking but can allocate on internal state transitions; it provides no hard real-time guarantee. `rtrb::Consumer::pop` is allocation-free and wait-free on the fast path. Satisfies D-015 rule 3 (well-specified primitive; lock-free ring buffers are a solved problem). Also a good learning exercise before M13.
**Rejected:** Hand-rolling a ring buffer (not worth the surface area); `ringbuf` (functionally equivalent, `rtrb` is more ergonomic).
**Consequences:** `rtrb` is added as a dep of `tungsten` only. `D-027` consequence updated: "game code communicates with the callback via rtrb, not mpsc."

## D-035 — Manifest merge order: call-site order, forward references are runtime None
**Date:** 2026-04-14
**Decision:** When multiple manifests are loaded together, merge order is call-site order (the order `Manifest::load` is called by the app — typically root manifest first, then example-local manifests). Later manifests may reference IDs introduced by earlier ones; the registry returns `None` at runtime for unresolved forward references (no forward-reference detection at load time).
**Why:** DFS-include or lexical ordering would require a manifest-level include directive that doesn't exist yet and adds parsing complexity. Call-site order is explicit and predictable for the current use case (one root manifest + one example-local manifest).
**Global uniqueness:** Enforced by the Layer 1 integration test (`crates/tungsten-core/tests/manifests.rs` — `all_manifest_ids_are_globally_unique`), which aggregates all discovered manifests and asserts no ID collision. Duplicate IDs remain fatal at load time per D-017.
**Consequences:** A future DFS-include directive or explicit load-order manifest field is the preferred upgrade if manifest composition grows complex.

## D-036 — M12: Proceed with archetypal ECS rewrite
**Date:** 2026-04-14
**Decision:** Proceed with M12. After M11 the full M7–M11 workload is in place — text, audio, hot reload, tilemaps, physics — giving a realistic benchmark target. The naive ECS has caused no correctness issues, but M12 is learning-motivated (archetypal storage, cache-friendly iteration, component move semantics), not a fix for a crisis. D-030 satisfied.

### Storage design
Replace `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` with an archetype graph:
- **`AnyColumn` trait** — type-erased interface over `Vec<T>`. Implemented by `TypedVec<T>(Vec<T>)`. Downcast cost is one `downcast_ref::<TypedVec<T>>()` per archetype per query, not per element; elements are then accessed as `col.0[i]` over a contiguous `Vec<T>`.
- **`Archetype`** — a table whose columns are `HashMap<TypeId, Box<dyn AnyColumn>>` plus a parallel `Vec<Entity>` row index. `component_types: Box<[TypeId]>` (sorted) uniquely identifies each archetype.
- **`Archetypes` registry** — `Vec<Archetype>` (index = ArchetypeId) + `HashMap<Box<[TypeId]>, ArchetypeId>` for O(1) archetype lookup by component set. Archetype 0 is the empty archetype; all freshly spawned entities start there.
- **Archetype graph edges** — lazy: on the first `insert<T>` / `remove<T>` from a given archetype, the target archetype id is computed (sorted union/difference) and cached in `add_edges[TypeId]` / `remove_edges[TypeId]`. Repeat transitions are O(1) edge lookup.
- **Column initialization** — new archetype columns are created lazily on first use: `AnyColumn::new_empty()` clones the column type so `move_components_to` can create a matching empty `TypedVec<T>` in a destination archetype without requiring the concrete type `T` at the call site.
- **Modelling after hecs** — `Box<dyn AnyColumn>` (typed columns, no unsafe raw bytes) rather than Bevy's `BlobVec`. Teaches the concepts without unsafe noise. `BlobVec` is the documented deferred upgrade path for maximum cache performance (M13+ concern).

### Generational entity IDs (Expansion 2)
`Entity` changes from `Entity(u32)` to `Entity { index: u32, generation: u32 }`. Rationale: M12 rewrites the entity table anyway; generational indices catch stale-handle bugs that parent/child relationships in M13 will expose (D-021 deferred "upgrade only if bugs appear"; M13 creates exactly that risk). `entity.id()` continues to return `index` as `u32` — source-compatible. Generation wraps at `u32::MAX` (theoretical, not a practical concern at hobby scale; noted but not guarded).

### Multi-component tuple queries (Expansion 1)
`query2<A, B>()`, `query2_entities<A, B>()`, `query3<A, B, C>()`, `query3_entities<A, B, C>()` — immutable; one downcast per archetype per type, then sequential row access over contiguous `Vec<T>` slices. Mutable multi-component queries require unsafe split-borrow and are deferred.

### New dependency
`criterion` added to `[dev-dependencies]` in `tungsten-core/Cargo.toml` for the M12 benchmark suite. Dev-only; satisfies D-015 rule 3 (solved primitive for microbenchmarking).

### Benchmark results
Measured on the development machine (release profile, Criterion 0.5, 10k entities, 3+ component types). Naive baseline is an inline `HashMap<TypeId, HashMap<u32, Box<dyn Any>>>` simulation in the same bench file.

| Benchmark | Archetypal (new) | Naive (old) | Ratio |
|-----------|-----------------|-------------|-------|
| `query::<Position>` — single type, 10k entities | **6.8 µs** | 43.4 µs | ~6× faster |
| `query2::<Position, Velocity>` — 10k, one archetype | **7.1 µs** | 1 424 µs (query_entities+get) | ~200× faster |
| `query2::<Position, Velocity>` — 10k, 5 archetypes | **7.5 µs** | same naive | ~190× faster |
| spawn + 3 inserts × 10k | 4.4 ms | — | — |

The single-type query win (~6×) comes from linear `Vec<T>` access vs. per-element `HashMap::get` + `downcast_ref`. The multi-component query win (~200×) is larger: the naive pattern requires N separate `HashMap::get` calls for the second component type; the archetypal `query2` does one downcast per archetype then zips two `Vec<T>` slices directly.

### Deferred / out of scope
Parallel system scheduling, change detection, command buffers, reactive queries, `BlobVec` raw-byte columns, SparseSet storage, dynamic components.
