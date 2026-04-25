# Decision Index

Use this as the cheap rationale lookup before opening [`DECISIONS.md`](../DECISIONS.md).

This file intentionally references every current `D-xxx` heading in [`DECISIONS.md`](../DECISIONS.md). The workspace test suite checks that coverage, so any new decision should add a matching short takeaway here in the same change.

If you need deeper context, grep the specific `D-0xx` entry in the full log instead of reading it serially.

## Foundations

| Decision | Takeaway |
| --- | --- |
| `D-001` | Project name is Tungsten; crate prefix stays `tungsten-`. |
| `D-002` | Local judgment over formal process; no CI-first workflow. |
| `D-003` | Native only. No WASM target or WASM-driven design compromises. |
| `D-004` | `wgpu` is the renderer. |
| `D-005` | No external ECS crate. ECS work stays in-project by design. |
| `D-006` | Three-crate workspace split is intentional: `tungsten-core`, `tungsten-render`, `tungsten`. |
| `D-007` | `tungsten-render` may depend on `tungsten-core`; strict isolation is not the goal. |
| `D-008` | One workspace-root `tungsten.json`, loaded at startup. Missing file falls back to defaults; invalid JSON is fatal. |

## Assets / Rendering

| Decision | Takeaway |
| --- | --- |
| `D-009` | Assets are manifest-driven and referenced by stable IDs, never file paths in game code. |
| `D-010` | Animations use Tungsten’s own small JSON format. |
| `D-011` | Sprite filter mode is per-sprite in the manifest. |
| `D-012` | Hot reload was deferred from Phase 1, but the ID-based asset model was kept compatible. |
| `D-013` | Shared assets live under workspace `assets/`; examples can have local `examples/NN_name/assets/`. |
| `D-014` | Asset registry is a `World` resource, not a global singleton. |
| `D-016` | Core owns opaque asset handles only; no `wgpu` types in `tungsten-core`. |
| `D-017` | Multiple manifests compose by extension only; duplicate IDs are fatal. |
| `D-018` | Extract plain render data before drawing; renderer should not need long-lived mutable `World` access. |
| `D-023` | WGSL shaders are embedded with `include_str!`; shader edits require rebuilds. |
| `D-026` | Text rendering uses `glyphon` / `cosmic-text`. |
| `D-032` | Tilemaps use Tiled-compatible `.tmj` data and reuse the sprite render path instead of a separate tile pipeline. |
| `D-042` | `Transform`, `Sprite`, `Visibility`, and `Tag` are engine-level components; default sprite extraction is explicit and opt-in through those components. |
| `D-048` | M22 sprite atlases: shelf-next-fit packer in `tungsten-core` with a mandatory deterministic tie-break, per-filter page lists, 1 px transparent padding + half-texel UV inset, renderer mints `TextureHandle`s, rebuild-on-growth with in-place shrink, and manifest hot-reload additions routed through `rebuild_atlas_for_filter`. |
| `D-054` | M24 tween easings are a closed `enum` with a pure `fn apply(t) -> f32`; no trait object, no dependency (curve math is ~60 lines of closed-form). |
| `D-055` | M24 single `Tween` component per entity, multi-property via `Vec<TweenChannel>` sharing the easing + duration; more than one tween per scene entry logs `ERROR` and keeps the first. |
| `D-052` | Asset composition is owned by the umbrella: `App::set_manifest_roots` + `asset_loader::load_all_merged` merge manifests via `ResolvedManifest::load_and_merge_many` and run `load_all` once on the result, with the merged graph stored as a `LoadedManifest` world resource; per-type loaders stay public but must not be used to compose. |
| `D-053` | Hot-reload support matrix is one published table in `DESIGN.md §Hot Reload — M9`: sprites/animations/fonts/tilemaps/particles support single-file and manifest-add reloads with warn-only removal; sounds are session-static (mixer owns cloned PCM). Particle manifest-add mirrors the tilemap-add validation path; `LoadedManifest` is refreshed on every successful manifest reload. |
| `D-057` | M25 shaders are manifest-tracked `.wgsl` assets bridged through `ShaderRegistry` + `ShaderModuleCache`; body edits hot-reload through the existing umbrella watcher after `wgpu::naga` validation, signature changes still need a rebuild, `SceneColor` format equals the swapchain sRGB format, and MSAA sample-count swaps require a relaunch. Narrows `D-023`, extends `D-053`. |
| `D-058` | M26 materials + post-stack + tween→material bridge: manifest-tracked `materials` section (shader id + 256-byte `MaterialUniformDefaults`), closed-enum `PostPass` (17 stock effects) reorderable via `PostStack` resource, entity-local `UniformOverrideBlock` as the shared animation surface, new `TweenChannel::Uniform*` variants. `PostStack::default()` is empty → byte-identical to the M25 baseline. Narrows `D-023` (material body hot reload) and `D-055` (uniform-slot tween variants without a second `Tween` per entity). SMAA stays out of `PostPass` and ships later in M27. |
| `D-059` | M27 SMAA 1x presentation AA: `RenderConfig.post_aa` (`Off / SmaaLow / SmaaMedium / SmaaHigh / SmaaUltra`, `#[non_exhaustive]`) + `TUNGSTEN_RENDER_POST_AA`; renderer-owned three-pass tail (edge → blend → neighborhood) between `PostStack` and the text overlay; `area` / `search` LUTs ship as `include_bytes!` engine content (not manifest-tracked) with MIT attribution; the three SMAA stage shaders are manifest-tracked and follow `D-057`'s body-edit reload path; preset knobs ride a 256-byte UBO (no recompile on switch); `SceneColor` + post ping/pong carry non-sRGB `view_formats` twin while SMAA is active so edge detection sees gamma-encoded pixels; `post_aa = Off` is byte-identical to the M26 frame; runtime changes go through `tungsten::request_post_aa` and apply at a frame boundary — no relaunch. Narrows `D-058`, extends `D-053`, narrows `D-023` like `D-057`. No new runtime dependency. |
| `D-060` | M28 bloom: `BloomParams { threshold, knee, intensity, radius }` ships as the 18th `PostPass` variant on the reorderable `PostStack`; an `Rgba16Float` `BloomPyramid` lives on `SceneTarget` sized by `bloom_mip_count_for_size(width, height, render.bloom_max_mips)` (default `6`, range `1..=8`, env `TUNGSTEN_RENDER_BLOOM_MAX_MIPS`); the bloom slot is the first `PostPass` recorded at encoder level — threshold + N-1 13-tap Karis-weighted downsamples + N-1 9-tap tent additive upsamples + replace-blend composite, each opening its own `RenderPass` through `BloomPipeline::record_pass`; four manifest-tracked stage shaders follow `D-057`'s body-edit reload path; the 256-byte UBO contract from `D-058`/`D-059` is reused. `SceneColor` stays sRGB — only the pyramid is HDR. With `PostStack` empty the M27 baseline frame stays byte-identical. `bloom_max_mips` is startup-only like `msaa`. No new runtime dependency. Narrows neither `D-058` nor `D-059`; extends `D-053`. |

## Dependencies / Tooling

| Decision | Takeaway |
| --- | --- |
| `D-015` | New dependencies must satisfy one of three acceptance rules: platform API, well-specified format, or solved primitive. |
| `D-019` | `pollster` blocks on `wgpu` async init. |
| `D-020` | `bytemuck` handles GPU POD layout. |
| `D-025` | Project license is MIT. |
| `D-027` | `cpal` handles audio device access. |
| `D-028` | `symphonia` handles audio decoding. |
| `D-031` | Hot reload uses `notify` and a simple watcher/event flow, not an async runtime. |
| `D-034` | Audio command channel uses `rtrb` SPSC ring buffer. |
| `D-037` | `criterion` is used for render-side micro-benchmarks. |
| `D-038` | Frame timing uses inline `Instant` instrumentation in `app.rs`; no extra profiling crate for core telemetry. |
| `D-041` | Release/profile tuning is part of the current baseline; perf comparisons should assume those settings. |

## ECS / Runtime Flow

| Decision | Takeaway |
| --- | --- |
| `D-021` | Entity IDs started as `u32`; generational IDs shipped in M12. |
| `D-022` | Panic on programmer errors, return `Option`/`Result` on runtime conditions. |
| `D-024` | Phase 1 close-out observations were recorded to guide Phase 2. |
| `D-029` | Audio mixer stays hand-rolled; no `kira`. |
| `D-030` | The M12 ECS rewrite required an explicit go/no-go decision. |
| `D-033` | Physics is hand-rolled in `tungsten-core`; `Position` stays separate from gameplay render components. |
| `D-035` | Manifest merge order is call-site order, usually shared manifest first then example-local. |
| `D-036` | Archetypal ECS rewrite is intentional and benchmark-validated. |
| `D-039` | `CommandBuffer` is a world resource with post-system flush; deferred structural changes are visible to extract/render in the same frame and to systems on the next frame. |
| `D-040` | `EventQueue<T>` keeps two windows (`previous`, `current`) and flushes once per frame after systems. |
| `D-043` | Display settings live in `tungsten.json`, runtime changes go through `request_display_settings`, and actual window/surface mutation happens only at a frame boundary. |
| `D-044` | Runtime HUD lives in the umbrella crate, reads existing telemetry resources, and ships off-by-default with EWMA-smoothed frame timing; the original M18 `F4` toggle later moved under `D-045`'s action map. |
| `D-045` | Input actions map string names to `Vec<Binding>` in `tungsten-core`; load and persist through workspace-root `input.json`, hot-reload through the existing `notify` watcher, and cover keyboard, mouse buttons, wheel directions, and engine-owned controls. |
| `D-046` | Scene/state system: single engine-owned dispatcher drives a `StateStack` + `GameState` trait; `SceneEntity { state_id }` marker auto-despawns through `CommandBuffer` on exit; `scene.json` reuses M15 components; `state_start` / `state_pause` / `state_back` action defaults ship with `ActionMap`. |
| `D-047` | Debug tooling: `DebugDraw` is core POD drained into `QuadInstance` (AABB edges) + `DebugLineInstance` (lines/circles); overlays are independent action-toggled resources (`F1`/`F2`/`F3`), not HUD rows; screenshots render to an offscreen `RENDER_ATTACHMENT \| COPY_SRC` texture and read back via row-padded `MAP_READ` buffer; GPU debug groups + explicit wgpu labels are always-on. |
| `D-049` | M23 ships a hand-rolled PCG32 + SplitMix64 PRNG in `tungsten-core`; no `rand` / `getrandom` dependency. |
| `D-050` | M23 particle configs live behind `Arc<ParticleConfig>`; emitters snapshot on first tick and live particles keep their original `Arc` across hot-reload, so in-flight curves never reinterpret mid-life. |
| `D-051` | M23 uses one ECS entity per live particle (no pool); despawns route through the standard `CommandBuffer` flush, and `max_alive` + global `ParticleBudget` bound the archetype. |
| `D-056` | M24 `TweenComplete` routes through `EventQueue<TweenComplete>` and terminal `Tween` removal routes through `CommandBuffer::remove_component`; a `pending_remove` latch prevents re-fire between tick and frame-end flush. |

## When To Open Full `DECISIONS.md`

- You are considering a new dependency.
- A change would alter the core/render seam, asset-ID model, or frame-order invariants.
- You think a current behavior looks wrong but it may be intentional.
- You need the detailed rationale or consequences behind a specific `D-0xx`.
