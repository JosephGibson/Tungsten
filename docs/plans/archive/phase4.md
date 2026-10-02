---
status: done
goal: "Phase 4 ships 7 milestones (M25–M31) covering render foundation, materials, stock post-effects, SMAA presentation AA, bloom, 2D lighting, parallax + game-feel, and instanced mesh particles + transitions."
non-goals:
  - "No 3D, scripting, networking, WASM, editor (DESIGN.md §Non-Commitments)."
  - "No new GIF/video capture pipeline; use existing screenshot hooks and manual acceptance artifacts."
  - "No deferred lighting, shadow casters, occluder polygons, volumetric lights, GI (Phase 5)."
  - "No engine asset-preprocessing pipeline. The approved archive/platformer-art-revamp.md exception permits an example-local offline authoring generator with checked-in outputs; it never runs at startup, during Cargo builds, or during asset loading."
  - "No cleanup-only milestone; each feature milestone slices the monoliths it touches."
files to touch:
  - "docs/plans/phase4.md"
  - "docs/plans/phase4-milestone-NN-short-topic.md for each Phase 4 milestone (`NN` = zero-padded milestone number, `short-topic` = concise kebab-case slug)"
ordered steps:
  - "M25–M31 are shipped. M31 followed `docs/plans/archive/phase4-milestone-31-mesh-particles-transitions.md`; the owner accepted the defaults of its open questions on 2026-10-02."
  - "Phase 4 closed with that plan's final step on 2026-10-02: it and this file are archived with status done and the inbound links are repointed."
done-when:
  - "All 7 milestones landed on the active integration branch, each with a completed, archived plan."
  - "DESIGN.md Status, CHANGELOG.md, and docs/DECISION_INDEX.md are updated where milestone decisions change canonical project guidance."
  - "This file is marked `status: done` and archived; any milestone that changes the shader/text/hot-reload rules updates AGENTS.md in the same change."
---

## Milestone Plan Filenames

Phase 4 milestone plans use `phase4-milestone-NN-short-topic.md`, where `NN` is the zero-padded milestone number and `short-topic` is a concise kebab-case slug. Example: `phase4-milestone-25-render-foundation.md`.

M25–M31 are shipped. The M31 section below keeps the scope as it was planned; use [LLM_INDEX.md](../../LLM_INDEX.md) and source for current APIs.

## Seam Constraints

- Manifest additions stay keyed JSON objects (`id -> entry`), matching the current `RawManifest` shape and merge behavior.
- `Tween` remains one component per entity ([D-055](../../../DECISIONS.md)); Phase 4 extends entity-local `TweenChannel`/override data instead of introducing a cross-entity tween target model.
- Screen transitions stay umbrella-owned while `StateStack` queues concrete `GameState` values; do not move state-request abstractions into `tungsten-core` in Phase 4.
- `CameraState` stays render output only; shake/trauma lives in `CameraController` or a sibling camera-control resource.
- Screen-space text stays on the existing `ExtractTextFn` seam; sections already draw in the final overlay pass after post-processing and presentation AA so HUD/debug text stays crisp.

---

## Shipped milestones

Implementation details live in [DESIGN.md](../../../DESIGN.md), [the source index](../../LLM_INDEX.md) and the listed decisions. Archived plans preserve execution history and are not current API specifications.

| Milestone | Release | Decisions | Archived plan |
| --- | --- | --- | --- |
| M25 — Render foundation | 0.22.0 | `D-057` | `docs/plans/archive/phase4-milestone-25-render-foundation.md` |
| M26 — Materials, post-stack, uniform tweens | 0.23.0 | `D-058` | `docs/plans/archive/phase4-milestone-26-materials-post-stack.md` |
| M27 — SMAA presentation AA | 0.24.0 | `D-059`, amended by `D-087` | `docs/plans/archive/phase4-milestone-27-smaa-presentation-aa.md` |
| M28 — Bloom | 0.25.0 | `D-060` | `docs/plans/archive/phase4-milestone-28-bloom.md` |
| M29 — Forward lighting | 0.26.0 | `D-061` | `docs/plans/archive/phase4-milestone-29-2d-lighting.md` |
| M30 — Parallax, shake, squash/stretch | 0.28.0 | `D-073` | `docs/plans/archive/phase4-milestone-30-parallax-shake-squash.md` |
| M31 — Mesh particles, screen transitions | 0.36.0 | `D-093`, amending `D-051` | `docs/plans/archive/phase4-milestone-31-mesh-particles-transitions.md` |

---

## M31 — Instanced Mesh Particles + Screen Transitions

**Status:** shipped in 0.36.0 (`D-093`), implemented on 2026-10-02. Execution plan: [phase4-milestone-31-mesh-particles-transitions.md](phase4-milestone-31-mesh-particles-transitions.md), archived beside this file with its per-step results. The names below are the planned scope, checked against source on 2026-10-02 (commit `10e3cd9`); `D-093` and the source hold what shipped.

**Depends on:** M23 (particles), M26 (materials/post-stack).

**Adds (crates/tungsten-render/src/):**
- `mesh_particle.rs` — `MeshParticlePipeline`, `MeshParticleInstance`, `MeshParticleBatch`: a mesh's vertex and index buffers uploaded once per ID, one instanced indexed draw per mesh. Fed through `Renderer::upload_particle_mesh` and `Renderer::update_mesh_particles`; `render_frame_full` keeps its signature.
- `mesh_particle.wgsl` — internal shader, like `quad` and `debug_line`.

**Adds ([particle.rs](../../../crates/tungsten-core/src/assets/particle.rs), [components.rs](../../../crates/tungsten-core/src/components.rs)):**
- `ParticleMesh { vertices: Vec<[f32; 2]>, indices: Vec<u16> }` in core, since core has no render types.
- `ParticleConfig.render: ParticleRender` — `enum ParticleRender { Quad, Mesh { mesh: String } }`. The config names the mesh by manifest ID, as it names its sprite; `sprite` becomes optional for mesh configs.
- `ParticleMeshAssetId = AssetId<ParticleMesh>` (the typed handle `particle.rs` already has) + `ParticleMeshRegistry` resource.
- `MeshParticle { mesh, color }` component, in place of `Sprite` on a mesh particle. One entity per live particle stays (`D-051`).

**Adds ([manifest.rs](../../../crates/tungsten-core/src/assets/manifest.rs)):**
- keyed `particle_meshes` section (`id -> { vertices, indices }`), inline JSON only, like `materials`. No `.mesh.json`.

**Adds (crates/tungsten/src/):**
- `transition.rs` — `TransitionEffect { Fade, WipeRadial, Dissolve, Pixelate }`, `Transition { out_effect, in_effect, out_secs, in_secs, easing }`, `TransitionState { phase: Out | In, elapsed }`. The effect is a closed enum, not a `PostPass`: the four stock shaders share no progress convention (`wipe_radial` is inverted, `PostPass::Pixelate` is a block size). The roadmap's `Swap` phase is the boundary frame.
- [particles.rs](../../../crates/tungsten/src/particles.rs) — mesh spawn and tick paths, `extract_mesh_particles`; [asset_loader.rs](../../../crates/tungsten/src/asset_loader.rs) — `load_particle_meshes` and the manifest-reload diff.
- [state.rs](../../../crates/tungsten/src/state.rs) — `StateStack::request_push_transition`, `request_pop_transition`, `request_replace_transition` (each returns `false` when a transition is already queued or active), carrying the `pub(crate)` `StateCommand` that `StateStack` already queues.

**State-system integration:**
- [state.rs](../../../crates/tungsten/src/state.rs) — `state_dispatcher_system` drains plain commands as today, advances the transition by `DeltaTime`, applies the stored command on the frame `Out` completes, then runs `In` on the new state and clears the transition.
- [app.rs](../../../crates/tungsten/src/app.rs) — `stage_render` appends the transition's pass after the user's `PostStack` for that frame; the resource itself is not modified, so a state that rebuilds its stack cannot drop the transition.

**Acceptance:**
- Bullet-trail emitter in `examples/04_shader_playground/` drawing an instanced triangle mesh.
- [examples/03_scene_state/](../../../examples/03_scene_state/) changes state with each of fade, radial wipe, dissolve and pixelate. It replaces the tweened overlay fade the example has today (accepted by the owner, 2026-10-02).

**Decision:** `D-093` (new), amending `D-051`.

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

M25–M31 are complete.

## Resolved Decisions

- 7 milestones, M25–M31.
- 17 stock effects in M26; SMAA ships in M27 as a fixed presentation tail, not a reorderable `PostPass`; bloom in M28; emissive + rim in M29 lit path.
- LYGIA WGSL snippets vendored under `crates/tungsten-render/src/shaders/stock/lygia/` with header attribution. No crate dependency.
- `D-057` narrowed `D-023` in M25. Shader hot reload for body edits only; signature changes require rebuild.
- New manifest sections follow the existing keyed object shape (`id -> entry`), not `Vec`-only formats.
- Material animation stays entity-local through `TweenChannel` plus shared override data; no cross-entity `TweenTarget` in Phase 4.
- Screen transitions stay in `tungsten` alongside `StateStack`; no core `StateRequest` in Phase 4.
- SMAA is a renderer-owned presentation choice exposed through `render.post_aa`; it runs after the reorderable post stack and before screen-space text.
- M30 shake extends `CameraController`, not `CameraState`.
- Every new `D-0xx` added in Phase 4 updates [docs/DECISION_INDEX.md](../../DECISION_INDEX.md) in the same change; also sync [AGENTS.md](../../../AGENTS.md) and [DESIGN.md](../../../DESIGN.md) when canonical guidance changes.
- Depth-test sprite path ships in M25 as opt-in; `z_order` CPU-sort remains default.

## Sources

- [SMAA official site](https://www.iryoku.com/smaa/)
- [SMAA paper (Eurographics 2012 PDF)](https://www.iryoku.com/smaa/downloads/SMAA-Enhanced-Subpixel-Morphological-Antialiasing.pdf)
- [SMAA reference implementation](https://github.com/iryoku/smaa)
- [LYGIA shader library](https://lygia.xyz/)
- [Godot CanvasItem shader reference](https://docs.godotengine.org/en/stable/tutorials/shaders/shader_reference/canvas_item_shader.html)
- [gdquest-demos/godot-shaders — 2D reference](https://github.com/gdquest-demos/godot-shaders)
- [LearnOpenGL — 2D post-processing](https://learnopengl.com/In-Practice/2D-Game/Postprocessing)
