---
status: in progress
goal: "Phase 4 ships 9 milestones (M25–M33) covering render foundation, materials, stock post-effects, SMAA presentation AA, bloom, 2D lighting, parallax + game-feel, instanced mesh particles + transitions, MSDF text, and a collaborative showcase example."
non-goals:
  - "No 3D, scripting, networking, WASM, editor (DESIGN.md §Non-Commitments)."
  - "No new GIF/video capture pipeline; use existing screenshot hooks and manual acceptance artifacts."
  - "No deferred lighting, shadow casters, occluder polygons, volumetric lights, GI (Phase 5)."
  - "No engine asset-preprocessing pipeline (MSDF bakes at startup, not ahead of time). The approved archive/platformer-art-revamp.md exception permits an example-local offline authoring generator with checked-in outputs; it never runs at startup, during Cargo builds, or during asset loading."
  - "No cleanup-only milestone; each feature milestone slices the monoliths it touches."
files to touch:
  - "docs/plans/phase4.md"
  - "docs/plans/phase4-milestone-NN-short-topic.md for each Phase 4 milestone (`NN` = zero-padded milestone number, `short-topic` = concise kebab-case slug)"
ordered steps:
  - "Promote each remaining milestone below to its own `phase4-milestone-NN-short-topic.md` plan file."
  - "M25–M30 are shipped. Complete M31 and M32 before M33; M33 is last."
  - "For each milestone, write the plan, implement it, produce an acceptance artifact, and flip status to done."
done-when:
  - "All 9 milestones landed on the active integration branch, each with a completed, archived plan."
  - "DESIGN.md Status, CHANGELOG.md, and docs/DECISION_INDEX.md are updated where milestone decisions change canonical project guidance."
  - "This file is marked `status: done` and archived; any milestone that changes the shader/text/hot-reload rules updates AGENTS.md in the same change."
---

## Milestone Plan Filenames

Phase 4 milestone plans use `phase4-milestone-NN-short-topic.md`, where `NN` is the zero-padded milestone number and `short-topic` is a concise kebab-case slug. Example: `phase4-milestone-25-render-foundation.md`.

M25–M30 are shipped. M31–M33 below are future scope, with proposed API names; write a concrete milestone plan before implementation. Use [LLM_INDEX.md](../LLM_INDEX.md) and source for current APIs.

## Seam Constraints

- Manifest additions stay keyed JSON objects (`id -> entry`), matching the current `RawManifest` shape and merge behavior.
- `Tween` remains one component per entity ([D-055](../../DECISIONS.md)); Phase 4 extends entity-local `TweenChannel`/override data instead of introducing a cross-entity tween target model.
- Screen transitions stay umbrella-owned while `StateStack` queues concrete `GameState` values; do not move state-request abstractions into `tungsten-core` in Phase 4.
- `CameraState` stays render output only; shake/trauma lives in `CameraController` or a sibling camera-control resource.
- Screen-space text stays on the existing `ExtractTextFn` seam; sections already draw in the final overlay pass after post-processing and presentation AA so HUD/debug text stays crisp.
- M32 stays within the current thread policy: no general worker pool or async runtime.

---

## Shipped milestones

Implementation details live in [DESIGN.md](../../DESIGN.md), [the source index](../LLM_INDEX.md) and the listed decisions. Archived plans preserve execution history and are not current API specifications.

| Milestone | Release | Decisions | Archived plan |
| --- | --- | --- | --- |
| M25 — Render foundation | 0.22.0 | `D-057` | `docs/plans/archive/phase4-milestone-25-render-foundation.md` |
| M26 — Materials, post-stack, uniform tweens | 0.23.0 | `D-058` | `docs/plans/archive/phase4-milestone-26-materials-post-stack.md` |
| M27 — SMAA presentation AA | 0.24.0 | `D-059`, amended by `D-087` | `docs/plans/archive/phase4-milestone-27-smaa-presentation-aa.md` |
| M28 — Bloom | 0.25.0 | `D-060` | `docs/plans/archive/phase4-milestone-28-bloom.md` |
| M29 — Forward lighting | 0.26.0 | `D-061` | `docs/plans/archive/phase4-milestone-29-2d-lighting.md` |
| M30 — Parallax, shake, squash/stretch | 0.28.0 | `D-073` | `docs/plans/archive/phase4-milestone-30-parallax-shake-squash.md` |

---

## M31 — Instanced Mesh Particles + Screen Transitions

**Status:** planned; not implemented.

**Depends on:** M23 (particles), M26 (materials/post-stack).

**Adds (crates/tungsten-render/src/):**
- `mesh_particle.rs` — `MeshParticlePipeline`, `ParticleMesh { vertices: Vec<Vec2>, indices: Vec<u16> }` uploaded once, instanced per live particle.

**Adds (crates/tungsten-core/src/assets/particle.rs):**
- `ParticleConfig.render: ParticleRender` — `enum ParticleRender { Quad, Mesh { mesh_id: ParticleMeshAssetId } }`.
- `ParticleMeshAssetId(u32)` + `ParticleMeshRegistry` resource.

**Adds (crates/tungsten-core/src/assets/manifest.rs):**
- keyed `particle_meshes` section (`id -> ParticleMeshEntry { vertices, indices }`) — inline JSON or separate `.mesh.json`.

**Adds (crates/tungsten/src/):**
- `transition.rs` — `Transition { out_effect: PostPass, in_effect: PostPass, duration: f32 }`, `TransitionState { phase: Out | Swap | In, easing, elapsed }`.
- [state.rs](../../crates/tungsten/src/state.rs) — transition-aware queueing (`request_push_transition`, `request_pop_transition`, `request_replace_transition`, or equivalent) that carries the concrete umbrella-owned `StateCommand` already used by `StateStack`.

**State-system integration:**
- [state.rs](../../crates/tungsten/src/state.rs) — `state_dispatcher_system` advances the queued transition, runs `out_effect` via `PostStack` with driven progress uniform, executes the stored state command at the phase boundary, runs `in_effect` on the new state, and clears transition.

**Acceptance:**
- Bullet-trail spawn in `examples/04_shader_playground/` using a triangle mesh (instanced).
- [examples/03_scene_state/](../../examples/03_scene_state/) transitions between states using each of fade, radial wipe, dissolve, pixelate-out.

---

## M32 — MSDF Text

**Status:** planned; not implemented.

**Depends on:** M25 (shader assets), M26 (shared entity-local uniform override for outline/glow controls).

**Adds (new dep):**
- `msdfgen` crate + `ttf_parser` crate. Both satisfy [D-015](../../DECISIONS.md) rule 2 (well-specified format). Add `DECISIONS.md` entry narrowing [D-026](../../DECISIONS.md): `cosmic-text` retained for layout; rasterizer path split — `glyphon` remains default, MSDF is opt-in.

**Adds (crates/tungsten-render/src/):**
- `msdf_text.rs` — `MsdfTextPipeline`, `MsdfAtlas { texture, glyph_metrics: HashMap<GlyphId, GlyphMetrics> }`, per-frame vertex buffer.
- `shaders/msdf_text.wgsl` — median-of-RGB threshold, optional outline + glow uniforms.

**Adds (crates/tungsten-core/src/):**
- `MsdfText { text: String, font_id: AssetId, px: f32, color: [u8;4], outline: Option<OutlineParams>, glow: Option<GlowParams> }` component; when present, the M26 `UniformOverrideBlock` drives outline/glow thresholds without adding a second tween bridge.
- `MsdfFontRegistry` — parallel to `FontRegistry`, keyed by font ID.

**Adds (manifest — existing `fonts` section):**
- `fonts.<id>.msdf: bool | MsdfOptions` opt-in on the current keyed font entries; no parallel font-manifest format.

**Bake pipeline:**
- At startup, for each manifest font with `msdf: true`, `msdfgen` + `ttf_parser` generate a 512×512 atlas synchronously in the existing asset-load path (ASCII + Latin-1 Supplement + user-declared extra ranges). No general worker pool in M32.
- Atlas cached in memory only (no disk cache in M32).

**Extract:**
- `extract_msdf_text(&World)` — queries `(Transform, MsdfText, Visibility)` plus the optional M26 uniform override block, resolves via `MsdfFontRegistry`, uses `cosmic-text` `Buffer` for layout, emits per-glyph quad instances referencing atlas UVs.

**Touches:**
- [text.rs](../../crates/tungsten-render/src/text.rs) — unchanged (`glyphon` path stays); MSDF runs as a sibling pipeline.
- [asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) — proposed `asset_loader/msdf.rs` split (the loader is currently a flat module).

**Acceptance:** side-by-side gif — same string at 3 zoom levels rendered via `glyphon` vs `MsdfText`; outline + glow animated via the shared entity-local uniform override introduced in M26.

---

## M33 — Showcase Example (scope-locked at kickoff, not now)

**Status:** blocked on M31 and M32; scope lock remains at kickoff.

**Depends on:** M25–M32 all done.

**Hard rule:** no `examples/05_showcase/` directory, no asset production, no scope lock until M32 is flipped to `status: done`.

**Approved scoped exception:** [Platformer art revamp](archive/platformer-art-revamp.md) may author assets and gameplay presentation solely in `examples/01_platformer/` before M32. This neither completes M32 nor starts M33; the M32→M33 ordering, M33 acceptance requirements, and prohibition on `examples/05_showcase/` remain intact.

**Locked requirements (only these):**
- Lives in `examples/05_showcase/` with local `assets/manifest.json`.
- Must exercise: materials + ≥5 stock effects (M26), SMAA (M27), bloom (M28), ≥2 point lights + 1 directional + normal maps (M29), ≥3 parallax layers + shake + squash (M30), ≥1 mesh-instanced particle system + ≥1 screen transition (M31), MSDF title card (M32).
- Acceptance: ~30-second clip + 3 PNG stills committed under `docs/showcase/`.

Shape (game or non-game), assets, and systems are designed in the M33 plan file, not here.

---

## Final Ordering

| # | ID | Title | Hard Deps |
| --- | --- | --- | --- |
| 1 | M25 | Render foundation | — |
| 2 | M26 | Materials + post-stack + tween→material | M25 |
| 3 | M27 | SMAA presentation AA | M25, M26 |
| 4 | M28 | Bloom | M25, M26 |
| 5 | M29 | 2D lighting + emissive + rim | M25, M26 |
| 6 | M30 | Parallax + shake + squash | M25 |
| 7 | M31 | Mesh particles + transitions | M23, M26 |
| 8 | M32 | MSDF text | M25, M26 |
| 9 | M33 | Showcase | M25–M32 |

M25–M30 are complete. M31 and M32 have no dependency on each other; both must precede M33.

## Resolved Decisions

- 9 milestones. Locked.
- 17 stock effects in M26; SMAA ships in M27 as a fixed presentation tail, not a reorderable `PostPass`; bloom in M28; emissive + rim in M29 lit path.
- LYGIA WGSL snippets vendored under `crates/tungsten-render/src/shaders/stock/lygia/` with header attribution. No crate dependency.
- M32 must add a decision narrowing `D-026`; MSDF has not shipped. `cosmic-text` retained for layout.
- `D-057` narrowed `D-023` in M25. Shader hot reload for body edits only; signature changes require rebuild.
- New manifest sections follow the existing keyed object shape (`id -> entry`), not `Vec`-only formats.
- Material/MSDF animation stays entity-local by extending `TweenChannel` plus shared override data; no cross-entity `TweenTarget` in Phase 4.
- Screen transitions stay in `tungsten` alongside `StateStack`; no core `StateRequest` in Phase 4.
- SMAA is a renderer-owned presentation choice exposed through `render.post_aa`; it runs after the reorderable post stack and before screen-space text.
- M30 shake extends `CameraController`, not `CameraState`.
- M32 MSDF bake stays synchronous in the existing startup load path; parallel baking is Phase 5 work if startup cost justifies it.
- Every new `D-0xx` added in Phase 4 updates [docs/DECISION_INDEX.md](../DECISION_INDEX.md) in the same change; M25/M32 also sync [AGENTS.md](../../AGENTS.md) and [DESIGN.md](../../DESIGN.md) when canonical shader/text guidance changes.
- M26 `04_shader_playground` stays minimal. Heavy example authoring only in M33, except the approved [platformer art revamp](archive/platformer-art-revamp.md) scoped solely to `examples/01_platformer/` before M32.
- Depth-test sprite path ships in M25 as opt-in; `z_order` CPU-sort remains default.

## Sources

- [msdfgen — C++ reference implementation](https://github.com/Chlumsky/msdfgen)
- [msdfgen Rust crate](https://docs.rs/msdfgen)
- [awesome-msdf — shader collection](https://github.com/Blatko1/awesome-msdf)
- [SDF Fonts — redblobgames](https://www.redblobgames.com/blog/2024-03-21-sdf-fonts/)
- [SMAA official site](https://www.iryoku.com/smaa/)
- [SMAA paper (Eurographics 2012 PDF)](https://www.iryoku.com/smaa/downloads/SMAA-Enhanced-Subpixel-Morphological-Antialiasing.pdf)
- [SMAA reference implementation](https://github.com/iryoku/smaa)
- [LYGIA shader library](https://lygia.xyz/)
- [Godot CanvasItem shader reference](https://docs.godotengine.org/en/stable/tutorials/shaders/shader_reference/canvas_item_shader.html)
- [gdquest-demos/godot-shaders — 2D reference](https://github.com/gdquest-demos/godot-shaders)
- [LearnOpenGL — 2D post-processing](https://learnopengl.com/In-Practice/2D-Game/Postprocessing)
