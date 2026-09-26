# Decision Index

One-line takeaways for every decision heading in [`DECISIONS.md`](../DECISIONS.md). `crates/tungsten-core/tests/decision_index.rs` fails if a heading is missing here or an unknown ID appears, so a new decision adds its row in the same change. For detail, grep the one entry: `rg -n -A 12 '^## D-0NN' DECISIONS.md`.

## Foundations

| Decision | Takeaway |
| --- | --- |
| `D-001` | Project name is Tungsten; crate prefix stays `tungsten-`. |
| `D-002` | Local judgment over formal process; no CI-first workflow. Narrowed by `D-070`: CPU-only CI reports but doesn't block. |
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
| `D-023` | WGSL shaders are embedded with `include_str!`; shader edits require rebuilds. Narrowed by `D-057`: shader bodies now hot-reload; signature changes still rebuild. |
| `D-026` | Text rendering uses `glyphon` / `cosmic-text`. |
| `D-032` | Tilemaps use Tiled-compatible `.tmj` data and reuse the sprite render path instead of a separate tile pipeline. |
| `D-042` | `Transform`, `Sprite`, `Visibility`, and `Tag` are engine-level components; default sprite extraction is explicit and opt-in through those components. |
| `D-048` | M22 atlases: deterministic shelf packer in core, per-filter pages, 1 px padding + half-texel inset, renderer-minted handles, rebuild on growth (hot-reload adds via `rebuild_atlas_for_filter`). |
| `D-054` | M24 easings are a closed `enum` with pure `apply(t)`; no trait object or dependency. |
| `D-055` | M24: one `Tween` per entity with `Vec<TweenChannel>` sharing easing/duration; extra tweens in a scene entry log `ERROR`, first wins. |
| `D-052` | The umbrella owns asset composition: `App::set_manifest_roots` + `load_all_merged` merge manifests once into a `LoadedManifest` resource; don't compose with per-type loaders. |
| `D-053` | Hot-reload support matrix lives in `DESIGN.md §Hot Reload — M9`: sprites/animations/fonts/tilemaps/particles reload (removal warns); sounds are session-static. |
| `D-057` | M25 shaders are manifest-tracked `.wgsl` (`ShaderRegistry` + `ShaderModuleCache`); body edits hot-reload after Naga validation, signature changes rebuild; `SceneColor` matches swapchain sRGB; MSAA changes relaunch. Narrows `D-023`. |
| `D-058` | M26 manifest `materials` (shader ID + 256-byte defaults), closed-enum `PostPass` on a reorderable `PostStack` (empty = byte-identical), `UniformOverrideBlock` + `TweenChannel::Uniform*`. Narrows `D-023`, `D-055`. |
| `D-059` | M27 SMAA 1x tail between `PostStack` and text: `render.post_aa` presets, `include_bytes!` LUTs (not manifest-tracked), manifest-tracked stage shaders, runtime switch at a frame boundary; `Off` is byte-identical. |
| `D-060` | M28 bloom is the 18th `PostPass`: `Rgba16Float` pyramid on `SceneTarget` (`bloom_max_mips` 1..=8, startup-only), encoder-level passes, manifest-tracked stages; `SceneColor` stays sRGB. |
| `D-061` | M29 forward lighting: core `Light`/`LightKind`/`AmbientLight`, `LitSpritePipeline` with a 544-byte `LightUbo` (`LIGHT_CAP = 16`), normal/emissive sibling atlases, per-frame culled `extract_lights`; lit wins over material. |

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
| `D-068` | `AGENTS.md` is the one instruction body (`CLAUDE.md` imports it); scoped render rules, on-demand index, skills shared via `.agents/skills` symlinks, budgets checked by `just ctx`. |
| `D-069` | Rust 1.98.1 pinned and declared as `rust-version`; edition 2024 / resolver 3; `just check` (strict clippy) and `cargo-deny` are the shared gates. |
| `D-070` | CPU-only CI reports on PRs/pushes without blocking; GPU/audio/perf stay local. Narrows `D-002`; release builds in `D-071`. |
| `D-071` | `v*` tags build Linux/Windows x86-64 example archives plus `SHA256SUMS` into a GitHub Release with CHANGELOG notes; build only, write token in the publish job only; pre-release tags without a CHANGELOG section rehearse. Workspace version = newest CHANGELOG release, bumped only by `just release-cut`; `repo-check` and CI enforce it. CPU levels: `D-072`. |
| `D-072` | Release archives ship `x86-64-v3` and portable builds; a per-example std-only launcher runs the fastest one the CPU supports from the archive root (`TUNGSTEN_CPU_LEVEL` overrides). Measured: physics frames ~5% faster, ECS frames within 1%; native and fat LTO no better. Supersedes `D-071`'s single generic build. |

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
| `D-062` | Physics broadphase: flat prefix-sum spatial hash reused across substeps under a drift budget, with AABB prefilter; supersedes `D-033`'s per-substep rebuild. |
| `D-063` | Physics solver: warm-started soft step (Box2D v3 style) with clamped accumulated impulses, soft bias and one relax pass; bodies rest at ~slop. |
| `D-064` | Physics CCD: speculative contacts close tunneling up to 15,360 px/s; fixed 4 substeps; seam normals clamped by `face_mask`; events only at positive penetration. Supersedes that clause of `D-033`. |
| `D-065` | Physics island sleeping via deterministic union-find; sleepers are bit-frozen, wake on contact/external write/despawn/`physics::wake`, and emit no collision events. |
| `D-066` | Physics SoA staging: gather and write back once per frame through `query2_opt2`; substeps touch only dense arrays; events drain once per frame. |
| `D-067` | Physics step stays serial: a colour-parallel solver was deterministic but gained ~2.5% and cost the serial path ~18%, so it was dropped. Threading policy unchanged. |

## When To Open a Decision

Find the relevant heading with `rg -n 'D-0NN' DECISIONS.md`, then read that section only.

- You are considering a new dependency.
- A change would alter the core/render seam, asset-ID model, or frame-order invariants.
- You think a current behavior looks wrong but it may be intentional.
- You need the detailed rationale or consequences behind a specific `D-0xx`.
