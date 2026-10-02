---
status: done
goal: "Ship M31: particles that draw an instanced triangle mesh instead of a sprite quad, and screen transitions (fade, radial wipe, dissolve, pixelate) driven by StateStack; then close out Phase 4."
non-goals:
  - "No particle pool: one ECS entity per live particle stays (D-051)."
  - "No textured, lit or material-driven meshes; a mesh is 2D positions plus a per-instance color."
  - "No z-order interleaving of mesh particles with sprites; no velocity-aligned rotation."
  - "No edits to the stock post shaders or their mirrors; no new PostPass variant."
  - "No transition coverage of screen-space text (phase4.md seam constraint); no core-side state requests."
  - "No separate mesh asset files, no new dependency, no hot reload of the mesh particle shader."
  - "No HUD or telemetry counters for mesh particles."
  - "No release cut: the plan ends at the hand-off to docs/releases.md."
files to touch:
  - "crates/tungsten-core/src/assets/{particle,manifest,mod}.rs, crates/tungsten-core/src/{lib,components}.rs, crates/tungsten-core/src/tests/assets/{particle,manifest}.rs"
  - "crates/tungsten-render/src/mesh_particle.rs (new), mesh_particle.wgsl (new), tests/mesh_particle.rs (new), lib.rs, renderer.rs"
  - "crates/tungsten/src/{particles,asset_loader,app,state,lib}.rs, transition.rs (new), tests/transition.rs (new), tests/{state,asset_loader,app}.rs, crates/tungsten/tests/particles.rs, crates/tungsten/benches/particle_tick.rs"
  - "examples/02_bench/src/particles.rs, examples/02_bench/src/integrated/assets.rs (new config field only)"
  - "examples/03_scene_state/src/{main,states}.rs, examples/03_scene_state/tests/transition_regression.rs (new)"
  - "examples/04_shader_playground/src/main.rs, assets/manifest.json, assets/particles/bullet_trail.json (new), tests/post_regression.rs"
  - "scripts/smoke-examples.sh, scripts/test-smoke-examples.sh, justfile"
  - "DECISIONS.md, docs/DECISION_INDEX.md, DESIGN.md, CHANGELOG.md, AGENTS.md, crates/tungsten-render/AGENTS.md, .claude/skills/tungsten-wgpu/SKILL.md"
  - "docs/LLM_INDEX.md, docs/README.md, README.md, docs/plans/README.md, docs/showcase/README.md, docs/known-issues.md"
  - "docs/plans/phase4.md and this file (status, archive)"
ordered steps:
  - "0. Preflight and performance baseline"
  - "1. Core: mesh asset, manifest section, config field, component"
  - "2. Render: MeshParticlePipeline"
  - "3. Umbrella: load, spawn, tick, extract, hot reload"
  - "4. Acceptance: example 04 bullet trail"
  - "5. Transition types"
  - "6. StateStack queueing, dispatcher, frame composition"
  - "7. Acceptance: example 03 transitions"
  - "8. Decision D-093 and documentation"
  - "9. Full verification"
  - "10. Close out Phase 4 and hand off"
done-when:
  - "A particle config with render kind mesh draws instanced triangles in example 04; quad configs draw as before."
  - "Example 03 changes state with each of fade, radial wipe, dissolve and pixelate."
  - "just check, just smoke, just visual, just script-test, just ctx and just repo-check pass."
  - "No owned metric of the benchmark suite reads regressed against the step 0 baseline."
  - "D-093 is in DECISIONS.md with its index row; D-051 carries the amendment marker."
  - "This plan and phase4.md are archived with status done, and no maintained doc links to their old paths."
---

# M31 — Instanced Mesh Particles + Screen Transitions

Execution plan for the last Phase 4 milestone, written 2026-10-02 against commit `10e3cd9` (branch `0.36`, workspace `0.35.0`, latest decision `D-092`). The roadmap entry is [phase4.md](phase4.md) §M31. Read `AGENTS.md` and `crates/tungsten-render/AGENTS.md` before editing; the `tungsten-wgpu` skill covers the seam and layout rules. Line numbers below are those of `10e3cd9`; locate by symbol once edits begin.

## Open questions

Resolved 2026-10-02: the owner accepted the default of each question. They stay here for the record, and the steps below already follow the defaults.

1. **Text is not covered by a transition.** Screen-space text draws after the post stack (seam constraint, `phase4.md`), so a fade to black leaves HUD and example text visible. Default: accept for M31, expose `StateStack::transition_cover()` so a game can fade its own text, have example 03 do so, and record the limit in `docs/known-issues.md`.
2. **Example 03 already fades by hand.** `examples/03_scene_state/src/states.rs` has a tweened black overlay sprite, `PendingTransition`, `TransitionTarget` and `handle_tween_complete_system`. Default: remove them in step 7, because they would stack with the engine transition and the example then no longer shows a tween-driven state change. Alternative: keep them on the gameplay → menu path and use engine transitions on the other three.
3. **Mesh particles draw above every sprite.** They use their own pipeline after the sprite batches, so `z_order` cannot place them between sprites. Default: accept and record the limit. Alternative: draw them before the sprites.
4. **Acceptance artifacts.** `docs/showcase/README.md` lists M27 and M28 as "regeneration recipe only". Default: the same for M31 (recipes and status rows, no checked-in images); the executing agent captures and inspects the frames and records commit, backend and result in the status table. Alternative: check in reviewed composites, which needs the owner's review.
5. **No successor roadmap exists.** After `phase4.md` is archived, the README and docs-map rows that point to it have no target an agent may read. Default: remove those rows (step 10) instead of pointing them at the archive.

## Context digest

- **Particles today.** One entity per live particle with `Particle + Transform + Sprite + Visibility` (`D-051`). `particle_emit_system`, `particle_tick_system` and `spawn_particle_via` are in `crates/tungsten/src/particles.rs`; the tick writes `Sprite.color`. Particles draw through whatever sprite extract the app sets. No file in `crates/tungsten-render/src/` knows about particles.
- **Configs.** `ParticleConfig` is a JSON file per config, listed in the manifest `particles` section, held as `Arc` (`D-050`). `sprite` is a string ID. Six Rust files build `ParticleConfig { .. }` literals (eight literals), so a new field touches all of them.
- **Manifest.** `RawManifest` sections are `HashMap<String, Entry>`. `materials` is the only inline section; its entries reload with the manifest (`reload_manifest` in `crates/tungsten/src/asset_loader.rs`).
- **Render.** `record_main_draws` in `renderer.rs` draws quads, sprites, debug quads, debug lines inside the scene pass. `DebugLinePipeline` is the closest template: instanced, untextured, shares the quad camera bind group, `passthrough_depth_stencil`. `tungsten.json` ships `depth_enabled: true`, so the scene pass has a depth attachment.
- **State.** `StateStack` queues `pub(crate) StateCommand` values; `state_dispatcher_system` drains them, then updates the top state. Hooks and scene-entity despawn follow `D-046`.
- **Post stack.** `PostStack` is a core resource the app reads in `App::stage_render`. `FadeParams`, `WipeRadialParams` and `DissolveParams` have a `progress`; `PostPass::Pixelate(f32)` is a block size in pixels. Parameters belong to the stack slot (`D-090`).
- **Budgets.** `AGENTS.md` is 6,096 of 6,144 bytes; `crates/tungsten-render/AGENTS.md` 4,016 of 4,096; `docs/LLM_INDEX.md` 7,061 of 8,192 (`just ctx`).
- **Git.** The owner's settings deny agent `git add`, `git commit` and `git rm`; move files with plain `mv` and leave staging and commits to the owner.

## Settled choices

| Choice | Decision | Reason |
| --- | --- | --- |
| Mesh storage | Inline in the manifest `particle_meshes` section; no `.mesh.json` | A triangle is three vertices. `materials` is already inline (`D-058`), so the manifest watcher, merge and duplicate-ID check cover it. A new file type would need a loader, a watcher route and a coverage rule in `scripts/check-repo.py`. |
| Mesh ID type | `AssetId<ParticleMesh>`, with `ParticleMeshAssetId` as its alias | `particle.rs` already defines the typed `AssetId<T>` and uses it for configs. |
| Config field | `render: ParticleRender`, `Mesh { mesh: String }` naming the manifest ID | Configs are JSON and already name their sprite by string. The ID is resolved once per emitting emitter per frame and stored on the particle. |
| Mesh particle components | `Particle + Transform + MeshParticle + Visibility`, no `Sprite` | The sprite extract queries `Sprite`, so it stays untouched and never draws a mesh particle as a quad. |
| Render entry | `Renderer::update_mesh_particles`, called beside `update_lights` | Keeps the seven-argument `render_frame_full` signature. |
| Shader | `mesh_particle.wgsl`, internal like `quad` and `debug_line` | No author-facing parameters; a manifest-tracked shader would need a seeded render-side ID (`D-091` fixed 0–27). |
| Transition effect type | Closed enum `TransitionEffect`, not a raw `PostPass` | The four effects do not share a progress convention (see step 5), and the other 14 `PostPass` variants have none. |
| Request API | `request_push_transition`, `request_pop_transition`, `request_replace_transition`, each returning `bool` | Mirrors the existing `request_*` trio. One transition at a time: a second full-screen effect over the first has no defined result, and `false` tells the caller the request was dropped. |
| Transition state | Inside `StateStack` | The dispatcher already owns that resource; the seam constraint keeps transitions in the umbrella crate. |
| Pass injection | `App::stage_render` appends the transition pass to a copy of the user's stack | A state's `on_enter` may clear or rebuild `PostStack`; the transition must survive that and needs no cleanup. |
| Phases | `Out` and `In`; the roadmap's `Swap` is the boundary frame | The command runs on the frame `Out` completes, and that frame draws fully covered. |
| Decision | One new entry, `D-093` | A new manifest section and asset ID, a second scene-pass draw path for particles, and an amended `D-051` component set. |

## How instancing fits `D-051`

`D-051` rejected a pool because it would be a second source of truth beside the entities. This plan keeps the entities: a mesh particle is spawned and despawned through `CommandBuffer`, counted by `particle_count_refresh_system` (which queries `Particle` alone) and bounded by `max_alive` and `ParticleBudget`. Instancing is draw-side only: the extract walks the entities and emits one batch per mesh, and the renderer issues one indexed draw per batch. The amendment is the component set: `MeshParticle` in place of `Sprite`.

## Steps

### 0. Preflight and performance baseline

- Set `status: in progress` in this file.
- `git status --short` is empty apart from the two plan files; `git log -1 --format=%h` is `10e3cd9` or a descendant that does not touch the files listed above.
- Capture two baseline suites on the untouched tree, following `docs/perf/profiling-workflow.md` (quiet machine, no remote-desktop encoder, nothing else building):

  ```bash
  just perf suite --repeat 5   # baseline A
  just perf suite --repeat 5   # baseline B
  just perf compare <A> <B>
  ```

  Record both directories here. A metric that reads `regressed` between A and B is not usable for judging this milestone; list it.

**Check:** `just check` passes; both suite directories exist under `perf-runs/` and the A/B compare report is written.

**Done 2026-10-02.**

- Tree: `git status --short` listed only the two plan files; `HEAD` is `10e3cd9`. `just check` exited 0 on the untouched tree.
- Machine: governor `performance`, one agent session, no build running. `nxcodec.bin` ran when the session started and had exited before the first capture; each suite ran as one blocking command under `perf-runs/20261002-m31-evidence/sit.sh` (encoder guard plus a per-second load log). Zero encoder sightings in both sittings; no run window shows a foreign load (the largest one-second peak is 1.66 busy CPUs at a run boundary).
- All captures use `WGPU_BACKEND=vulkan` (a hard compare field): use it for the step 3 and step 9 captures too.
- Baseline A: `perf-runs/20261002T134400Z-suite` (valid). Baseline B: `perf-runs/20261002T134753Z-suite` (valid).
- A/B compare: `perf-runs/20261002T135147Z-compare-suite/compare.md`. Owned verdicts: 0 regressed, 0 improved, 45 unchanged, 9 noisy. No metric is excluded from judging.
- Noisy in A/B, for reference when reading later compares: `ecs` `buffs`, `stats_decay`, `follow` p50; `churn` `churn_spawn` p50; `gpu` `extract` p95; `gpu-throughput` `render_encode` p50 and p95; `particles` `unattributed` p95 and `animate_sprites` p95; `integrated` peak RSS.
- Evidence: `perf-runs/20261002-m31-evidence/` (`guard.log`, `baseline-{a,b}.load.log`, `baseline-{a,b}.loadcheck.txt`, `baseline-{a,b}.out`, `compare-a-b.out`).

### 1. Core: mesh asset, manifest section, config field, component

Files: `crates/tungsten-core/src/assets/particle.rs`, `manifest.rs`, `mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/components.rs`, and every `ParticleConfig { .. }` literal: `crates/tungsten-core/src/tests/assets/particle.rs`, `crates/tungsten/tests/particles.rs`, `crates/tungsten/benches/particle_tick.rs`, `examples/02_bench/src/particles.rs`, `examples/02_bench/src/integrated/assets.rs`, `examples/04_shader_playground/src/main.rs`.

In `particle.rs`:

```rust
/// Triangle-list mesh in mesh-local pixels; the origin is the particle position.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ParticleMesh {
    pub vertices: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
}

impl ParticleMesh {
    pub fn validate(&self) -> Result<(), String>;
}

pub type ParticleMeshAssetId = AssetId<ParticleMesh>;

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ParticleRender {
    #[default]
    Quad,
    Mesh { mesh: String },
}

#[derive(Debug, Default)]
pub struct ParticleMeshRegistry { /* name <-> id, id -> mesh */ }

impl ParticleMeshRegistry {
    pub fn new() -> Self;
    /// Registers `name`, or replaces its mesh and returns the ID it already has.
    pub fn insert(&mut self, name: &str, mesh: ParticleMesh) -> ParticleMeshAssetId;
    pub fn id_for_name(&self, name: &str) -> Option<ParticleMeshAssetId>;
    pub fn name_for_id(&self, id: ParticleMeshAssetId) -> Option<&str>;
    pub fn get(&self, id: ParticleMeshAssetId) -> Option<&ParticleMesh>;
    pub fn names(&self) -> impl Iterator<Item = &str>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

- `ParticleMesh::validate`: at least 3 vertices and at most 65,536; indices non-empty, a multiple of 3, each below `vertices.len()`; every coordinate finite.
- `ParticleConfig`: add `#[serde(default)] pub render: ParticleRender`; change `sprite` to `#[serde(default)]`. `validate` requires a non-empty `sprite` for `Quad` and a non-empty `mesh` for `Mesh`. JSON form: `"render": { "kind": "mesh", "mesh": "ex04_bullet_tri" }`.
- Existing literals get `render: ParticleRender::Quad`. Change nothing else in them.

In `manifest.rs`:

```rust
pub type ParticleMeshEntry = ParticleMesh;   // RawManifest.particle_meshes: HashMap<String, ParticleMeshEntry>

#[derive(Debug, Clone)]
pub struct ResolvedParticleMesh {
    pub source_manifest: PathBuf,
    pub mesh: ParticleMesh,
}
// ResolvedManifest.particle_meshes: HashMap<String, ResolvedParticleMesh>
// ManifestError::InvalidParticleMesh { id: String, reason: String }
```

`load_unvalidated` validates each entry and fills `source_manifest` as the `materials` loop does; `merge_entries` rejects a duplicate ID with `DuplicateId`. The config → mesh reference is checked by the loader in step 3, as the config → sprite reference is today.

In `components.rs`:

```rust
/// Mesh-drawn particle (M31, `D-093`); takes the place of `Sprite` on a particle entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshParticle {
    pub mesh: ParticleMeshAssetId,
    pub color: [u8; 4],
}
```

Re-export `ParticleMesh`, `ParticleMeshAssetId`, `ParticleMeshRegistry`, `ParticleRender`, `ParticleMeshEntry`, `ResolvedParticleMesh` and `MeshParticle` through `assets/mod.rs` and `lib.rs`.

Tests, in `crates/tungsten-core/src/tests/assets/particle.rs`: `particle_mesh_validate_accepts_triangle`; `particle_mesh_validate_rejects_bad_index`, `_non_triangle_index_count`, `_non_finite_vertex`, `_too_few_vertices`; `render_defaults_to_quad`; `render_mesh_parses_from_json`; `quad_config_requires_sprite`; `mesh_config_needs_no_sprite`; `mesh_registry_insert_keeps_id_on_replace`. In `tests/assets/manifest.rs`: `particle_meshes_section_parses_inline_mesh`, `invalid_particle_mesh_is_rejected`, `duplicate_particle_mesh_id_across_manifests_is_fatal`.

**Check:** `cargo check --workspace --all-targets --locked && cargo test -p tungsten-core --locked -q` passes, and `cargo test -p tungsten --test particles --locked -q` passes with only the added field in `base_cfg`.

**Done 2026-10-02.** `cargo check --workspace --all-targets --locked` exit 0; `cargo test -p tungsten-core --locked -q` exit 0 (468 unit tests, the 13 new ones included); `cargo test -p tungsten --test particles --locked -q` exit 0 (6 passed, only `render: ParticleRender::Quad` and its import added). No deviation. Details the plan left open: `ParticleMeshRegistry` is `Vec`-backed, so `names()` yields registration order; `load_unvalidated` validates meshes in ID order, so the mesh an `InvalidParticleMesh` error names does not depend on map order; `ParticleMesh::MAX_VERTICES` holds the 65,536 limit.

### 2. Render: `MeshParticlePipeline`

Files: `crates/tungsten-render/src/mesh_particle.rs` (new), `mesh_particle.wgsl` (new), `tests/mesh_particle.rs` (new), `lib.rs`, `renderer.rs`.

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MeshParticleInstance {
    pub position: [f32; 2],   // location 1, Float32x2
    pub scale: [f32; 2],      // location 2, Float32x2
    pub rotation: f32,        // location 3, Float32
    pub color: [u8; 4],       // location 4, Unorm8x4
}                             // 24 bytes, align 4

pub struct MeshParticleBatch {
    pub mesh: ParticleMeshAssetId,
    pub instances: Vec<MeshParticleInstance>,
}

impl MeshParticlePipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        sample_count: u32,
        depth_attached: bool,
    ) -> Self;
    /// Uploads or replaces the vertex and index buffers of `id`.
    pub fn upload_mesh(&mut self, device: &wgpu::Device, id: ParticleMeshAssetId, vertices: &[[f32; 2]], indices: &[u16]);
    /// Writes this frame's instances into the kept instance buffer and records one draw range per batch.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, batches: &[MeshParticleBatch]);
    pub fn draw(&self, render_pass: &mut wgpu::RenderPass<'_>, camera_bind_group: &wgpu::BindGroup);
}

impl Renderer {
    pub fn upload_particle_mesh(&mut self, id: ParticleMeshAssetId, vertices: &[[f32; 2]], indices: &[u16]);
    /// Instances for the next frame; they hold until the next call, so the app calls it every frame.
    pub fn update_mesh_particles(&mut self, batches: &[MeshParticleBatch]);
}
```

- Copy the descriptor shapes from `DebugLinePipeline::new` in `debug_line.rs`: `TriangleList`, `cull_mode: None`, `BlendState::ALPHA_BLENDING`, `crate::quad::passthrough_depth_stencil(depth_attached)`, `sample_count`. Check the locked wgpu version with `cargo tree -i wgpu` before using an API not already in the crate.
- Vertex buffer 0 is `Float32x2` at location 0 (step `Vertex`); buffer 1 is `MeshParticleInstance` (step `Instance`). Draw with `draw_indexed` and `IndexFormat::Uint16`.
- The instance buffer is kept between frames and grows by doubling (`SpritePipeline::ensure_instance_capacity` is the model); one `queue.write_buffer` per frame; no write when there are no instances. A batch whose mesh was never uploaded is skipped with one `log::warn!` per ID.
- `mesh_particle.wgsl`: group 0 binding 0 is the camera struct of `quad.wgsl` (`projection: mat4x4<f32>`). The vertex stage scales the local position by `scale`, rotates it about the mesh origin and adds `position`; the fragment stage returns the instance color. Use the rotation sign and the instance-color handling of `sprite.wgsl`.
- `Renderer`: build the pipeline in `Renderer::new` beside `DebugLinePipeline::new` with `quad_pipeline.camera_bind_group_layout()`; in `record_main_draws` draw it after the `sprites` group and before `debug_quads`, in a `mesh_particles` debug group, with `quad_pipeline.camera_bind_group()`.
- Export `MeshParticleBatch`, `MeshParticleInstance` and `MeshParticlePipeline` from `lib.rs`.

Tests, in `crates/tungsten-render/src/tests/mesh_particle.rs`: `mesh_particle_instance_layout_is_stable` (size 24, align 4), `mesh_particle_instance_is_pod`, and a device-free test of the draw-range bookkeeping if `prepare` is split into a pure planning function (preferred). `tests/shader_coverage.rs` Naga-validates the new `.wgsl` without an edit; it needs no asset mirror.

**Check:** `cargo test -p tungsten-render --locked -q` and `cargo clippy -p tungsten-render --all-targets --locked -- -D warnings` pass.

**Done 2026-10-02.** `cargo test -p tungsten-render --locked -q` exit 0 (117 unit tests, 6 shader-coverage tests; the coverage test walks `src/` and validated `mesh_particle.wgsl`); `cargo clippy -p tungsten-render --all-targets --locked -- -D warnings` exit 0. Locked wgpu is 30.0.1; the pipeline uses only descriptor shapes `debug_line.rs` already uses, plus `set_index_buffer` / `draw_indexed`. No deviation. `prepare` is split as preferred: the device-free `plan_draws` fills a reused staging `Vec` (one `write_buffer` per frame) and the draw ranges, and has three tests (`plan_lays_batches_end_to_end`, `plan_skips_empty_batches_and_missing_meshes`, `plan_clears_the_previous_frame`). The layout test also pins the field offsets and attribute locations. GPU proof of the pipeline comes with steps 4 and 7.

### 3. Umbrella: load, spawn, tick, extract, hot reload

Files: `crates/tungsten/src/particles.rs`, `asset_loader.rs`, `app.rs`, `lib.rs`, `tests/asset_loader.rs`, `crates/tungsten/tests/particles.rs`.

Loading (`asset_loader.rs`):

```rust
pub fn load_particle_meshes(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()>;
```

- Takes the registry out of the world or defaults it (as `load_materials` does, so IDs survive a second load), inserts each mesh, uploads it, logs it. `App::new` inserts `ParticleMeshRegistry::new()` beside `ParticleConfigRegistry::new()`.
- `load_all` calls it before `load_particles`. The particle check at the end of `load_all` branches on `cfg.render`: `Quad` needs the sprite registered, `Mesh` needs the mesh registered. Apply the same branch in `reload_particle` and in the particle-additions block of `reload_manifest`. Put it in one helper.

Simulation (`particles.rs`):

- `particle_emit_system`: before the spawn loop, resolve `snapshot.render` once: `Quad`, or `Mesh` with the ID from `ParticleMeshRegistry::id_for_name`. An unknown name logs a warning and emits nothing for that emitter this frame. `build_particle` returns the render component: `Sprite` as today, or `MeshParticle { mesh, color }` with the same initial color.
- `particle_tick_system`: move the per-particle integration into one `#[inline(always)]` helper that returns the new color, or `None` when the particle aged out. Loop once over `query3_mut::<Particle, Transform, Sprite>()` and once over `query3_mut::<Particle, Transform, MeshParticle>()`. The sprite loop must produce the same values as before.
- Add:

  ```rust
  /// `spawn_particle_via` for a mesh config: `MeshParticle` in place of `Sprite`.
  #[allow(clippy::too_many_arguments)]
  pub fn spawn_mesh_particle_via(
      cmd: &mut CommandBuffer,
      emitter_ent: Option<Entity>,
      config: Arc<ParticleConfig>,
      mesh: ParticleMeshAssetId,
      position: Vec2,
      velocity: Vec2,
      lifetime: f32,
      start_scale: f32,
  );

  /// Visible mesh particles, one batch per mesh in first-seen order.
  pub fn extract_mesh_particles(world: &World) -> Vec<MeshParticleBatch>;
  ```

  `spawn_particle_via` keeps spawning a `Sprite` whatever the config's `render` says; document that. The extract reads `query3::<Transform, MeshParticle, Visibility>()`, skips invisible entities and copies `position`, `scale`, `rotation` and `color`. It allocates nothing when there are no mesh particles.

Frame (`app.rs`): `FrameExtract` gains `mesh_particles: Vec<MeshParticleBatch>`, filled in `stage_extract`; `stage_render` calls `renderer.update_mesh_particles(&extract.mesh_particles)` beside `renderer.update_lights`. The extract is engine-owned; there is no setter.

Hot reload:

| Edit | Result |
| --- | --- |
| Manifest adds a mesh | Registered and uploaded; configs loaded afterwards may name it |
| Manifest changes a mesh's vertices or indices | Re-uploaded under the same ID; live particles draw the new geometry on the next frame (geometry is not snapshotted, unlike the config `Arc` of `D-050`) |
| Manifest removes a mesh | Warning; the stale mesh stays |
| Manifest holds an invalid mesh | The reload fails in `load_and_merge_many`, is logged, and the last good graph stays |
| A config file changes `render` | `reload_particle` validates and swaps the `Arc`; live emitters keep their snapshot |
| `mesh_particle.wgsl` | Internal shader; rebuild |

In `reload_manifest`, add the mesh block before the particle block. Compute the changes in a renderer-free helper so it can be tested:

```rust
pub(crate) struct ParticleMeshDiff { pub added: Vec<String>, pub changed: Vec<String>, pub removed: Vec<String> }  // each sorted
pub(crate) fn diff_particle_meshes(registry: &ParticleMeshRegistry, manifest: &ResolvedManifest) -> ParticleMeshDiff;
```

Export `extract_mesh_particles` and `spawn_mesh_particle_via` from `lib.rs`.

Tests. `crates/tungsten/tests/particles.rs`: `mesh_config_spawns_mesh_particle_without_sprite`, `mesh_particles_count_against_budget_and_max_alive`, `mesh_particle_ages_out_and_tick_writes_color`, `unknown_mesh_name_emits_nothing`, `extract_groups_instances_by_mesh`, `extract_skips_invisible_mesh_particles`. `crates/tungsten/src/tests/asset_loader.rs`: `diff_particle_meshes_reports_added_changed_removed`, `reload_particle_accepts_registered_mesh`, `reload_particle_preserves_previous_on_unknown_mesh`.

**Check:** `cargo test -p tungsten --locked -q` and `cargo clippy --workspace --all-targets --locked -- -D warnings` pass. Then `just perf suite --only particles,integrated --repeat 5` and `just perf compare <baseline A> <new suite> --fail-on regressed` exits 0. A second caller has cost this codebase an inlined function before: if `particles` regresses, compare per-symbol samples (`just perf run particles --profile`) before changing the design.

**Done 2026-10-02, with an accepted cost.** The code is written and the tests pass; the perf compare reads one `regressed`. The owner accepted it on 2026-10-02 ("I'm okay with regressions. Continue"), so the plan's design stands and the tested option below was not applied. The record of the stop:

- Code: everything listed above is in the working tree as designed. `cargo test -p tungsten --locked -q` exit 0 (171 unit tests, `tests/particles.rs` 12 passed, the nine new tests included); `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0.
- Small additions, no deviation: `load_particle_meshes` registers meshes in ID order; one helper, `missing_particle_render_ref`, serves `load_all`, `reload_particle` and `reload_manifest`; `load_particles` logs the mesh name for a mesh config; `ParticleMeshDiff` derives `Default`, `PartialEq`, `Eq` for its test. An unknown mesh name is resolved after `plan_emission`, so a once-only burst is spent, and the warning repeats on every frame the emitter would emit (the loader rejects such a config, so only code-built configs reach it).
- Capture, widened to the full suite: `perf-runs/20261002T140200Z-suite` (valid, 0 encoder sightings). `just perf compare perf-runs/20261002T134400Z-suite perf-runs/20261002T140200Z-suite --fail-on regressed` exits 1; report `perf-runs/20261002T140551Z-compare-suite/compare.md`. Owned verdicts: 1 regressed, 1 improved, 42 unchanged, 10 noisy.
  - **Regressed:** `particles` `stage.unattributed` p50, 2.07 → 2.14 ms (+0.06 ms, +3.1%, interval [+0.01, +0.12], τ 0.062). It read `unchanged` between baselines A and B. Digests and counters are identical, so the sprite loop produces the same values.
  - Improved: `ecs` `system.bounds_wrap` p50, 0.31 → 0.25 ms. No ECS code changed; this is the known placement flip of that row, not a gain to claim.
  - `integrated`: 0 regressed, 4 unchanged.
- Investigation (per-symbol samples, as this step asks):
  - Nothing was outlined: neither binary has a symbol for `integrate_particle`, `build_particle` or `plan_emission`.
  - `particle_tick_system` holds 2,440 of 12,143 samples in a build of the untouched tree and 2,621 of 12,053 in the candidate (+7.4%); `particle_emit_system` 90 → 81, `particle_count_refresh_system` 765 → 782. The function grew from 2,922 to 5,325 bytes.
  - Cause: with the helper returning `Option<[u8; 4]>`, the sprite loop builds the color as a vector and stores it with shuffles (`pshufd`, `punpckldq`, `por`, `movd`) where the old loop packed it with scalar `movzbl`/`shl`/`or`, and the loop body is laid out across a wider address range.
  - Same-sitting A/B, baseline build against candidate, five runs each, three pairs: +0.05, +0.07, +0.06 ms (each reads `noisy`; the effect sits at τ).
- Tested option, in a scratch copy only (`perf-runs/20261002-m31-evidence/var`): the helper writes the color through `&mut [u8; 4]` and returns `bool`; still one `#[inline(always)]` helper and two loops. Two same-sitting pairs against the baseline build: +0.02 and −0.02 ms (means 2.036/2.054 and 2.062/2.038 ms), same digest. It was not run through the test suite.
- Evidence, all under `perf-runs/20261002-m31-evidence/`: `step3.out`, `step3.loadcheck.txt`, `compare-a-step3.out`; `base/` (export of `10e3cd9` without `docs/`, with its own `target/`) and `var/` (candidate tree plus the option); profiles `base/perf-runs/20261002T140824Z-particles/profile` and `perf-runs/20261002T140924Z-particles/profile`; `symbols-{base,cand}.txt`, `annotate-tick-{base,cand}.txt`; A/B captures listed in `ab1-*.out`, `ab2-*.out` and compared in `ab-compare-{1..4}.out`. The two scratch trees take 2.5 GB.
- Consequence for step 9: `particles` `stage.unattributed` p50 is expected to read `regressed` or `noisy` against baseline A (+0.05 to +0.07 ms). That verdict is accepted; `D-093` records it (regression policy: a justification in `DECISIONS.md` or the plan). Other `regressed` verdicts are reported to the owner without stopping.

### 4. Acceptance: example 04 bullet trail

Files: `examples/04_shader_playground/assets/manifest.json`, `assets/particles/bullet_trail.json` (new), `src/main.rs`, `tests/post_regression.rs`.

- Manifest: add `"particle_meshes": { "ex04_bullet_tri": { "vertices": [[0.0, -7.0], [6.0, 7.0], [-6.0, 7.0]], "indices": [0, 1, 2] } }` and `"particles": { "ex04_bullet_trail": { "path": "particles/bullet_trail.json" } }`.
- `bullet_trail.json`: `"render": { "kind": "mesh", "mesh": "ex04_bullet_tri" }`, no `sprite`, `"emission": { "kind": "continuous", "rate_hz": 180.0 }`, `max_alive` about 300, lifetime 0.5–0.9 s, a slow `radial` velocity, drag, non-zero `angular_velocity`, and scale, color and alpha curves that shrink and fade the triangle.
- `main.rs`: in `on_startup`, unless `TUNGSTEN_MESH_TRAIL_FIXTURE=off`, spawn one emitter (`ParticleEmitter::new(id)`, `ParticleEmitterState::default()`, `Transform`) with the ID from `ParticleConfigRegistry::id_for_name("ex04_bullet_trail")`. A `playground_bullet_trail` system copies the first bouncer's center into the emitter's `Transform` each frame. Document the env var in the file header.
- `tests/post_regression.rs`: let `capture` take extra env pairs, and add `mesh_trail_draws_instanced_triangles` behind `TUNGSTEN_VISUAL_REGRESSION`: capture the `empty` fixture with the trail and with `TUNGSTEN_MESH_TRAIL_FIXTURE=off`, and require more than 100 differing pixels. This is the only example test the step adds.

**Check:** `just repo-check` passes (the new JSON is manifest-listed); `TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-04-shader-playground --quiet --locked` exits 0; `just visual` passes on a GPU machine.

**Done 2026-10-02** (Vulkan, Radeon 660M, no encoder running). `just repo-check` exit 0; the three-frame run exit 0 and logged `Loaded particle mesh 'ex04_bullet_tri'` and `Loaded particle config 'ex04_bullet_trail' -> mesh 'ex04_bullet_tri'`; `just visual` exit 0; `cargo clippy -p example-04-shader-playground --all-targets --locked -- -D warnings` exit 0.

- `just visual` runs example 02's pixel test (2 passed) and example 04's `post_regression` (3 passed, `mesh_trail_draws_instanced_triangles` included). The two trail captures differ in 776 pixels, all inside x 357–394, y 232–268; the crop shows yellow triangles over the first bouncer with the trail and none without it.
- Deviation: the "config not registered" notice in `spawn_bullet_trail` is an `eprintln!`; the example has no `log` dependency and `Cargo.toml` is not in the file list.
- The emitter follows the center of the bouncer's drawn quad (`Transform.position` plus half its size, since a sprite spans from its position), computed by `bouncer_center`.

### 5. Transition types

Files: `crates/tungsten/src/transition.rs` (new), `tests/transition.rs` (new, declared with `#[path]` as the other modules do), `lib.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionEffect {
    Fade { color: [f32; 4] },
    WipeRadial { center: [f32; 2], softness: f32 },
    Dissolve { noise_scale: f32, edge_color: [f32; 4] },
    Pixelate { max_block_px: f32 },
}

impl TransitionEffect {
    /// The post pass for `cover` in [0, 1]: 0 leaves the frame untouched, 1 hides it as far as the effect can.
    pub fn pass_at(self, cover: f32) -> PostPass;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub out_effect: TransitionEffect,
    pub in_effect: TransitionEffect,
    pub out_secs: f32,
    pub in_secs: f32,
    pub easing: Easing,
}

impl Transition {
    /// One effect for both phases, `secs` each, linear.
    pub fn new(effect: TransitionEffect, secs: f32) -> Self;
    pub fn with_in_effect(self, effect: TransitionEffect) -> Self;
    pub fn with_easing(self, easing: Easing) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionPhase { Out, In }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionState {
    pub phase: TransitionPhase,
    pub elapsed: f32,
}

pub(crate) enum TransitionStep { Running, Boundary, Finished }
```

`pass_at` clamps `cover` and maps it per effect. The four stock shaders do not share a convention, which is why the roadmap's "driven progress uniform" is not enough:

| Effect | Mapping | Why |
| --- | --- | --- |
| `Fade` | `FadeParams { progress: cover, color }` | `fade.wgsl` mixes toward `color` as `progress` rises. |
| `WipeRadial` | `progress = (1 - cover) * min(1, (far + softness) / 1.5)`, `softness = softness * (1 - cover)`, with `far` the largest distance from `center` to a UV corner | `wipe_radial.wgsl` is inverted (`radius = progress * 1.5`; 1 shows the frame, 0 hides it). Scaling by `far` starts the wipe at the farthest corner instead of spending half the phase off screen. Shrinking `softness` closes the soft dot that `progress = 0` would leave at the center; the shader floors it at `1e-4`. |
| `Dissolve` | `DissolveParams { progress: cover, noise_scale, edge_color }` | At 1 every pixel is black or edge color, never the frame. |
| `Pixelate` | `PostPass::Pixelate(1 + (max(max_block_px, 1) - 1) * cover)` | The parameter is a block size, not a progress, and the effect never hides the frame: the state changes under the coarsest mosaic. |

Advance logic, pure and testable: a function that takes the `Transition`, the `TransitionState` and `dt` and returns a `TransitionStep`. In `Out`, `elapsed += dt`; when `elapsed >= out_secs` (or `out_secs <= 0`) it returns `Boundary`. The caller then runs the command and either finishes (`in_secs <= 0`) or sets `phase = In, elapsed = 0`: leftover time is dropped so the boundary frame always draws fully covered. In `In`, `elapsed += dt`; `elapsed >= in_secs` returns `Finished`. Cover is `easing.apply(elapsed / out_secs)` in `Out` and `1 - easing.apply(elapsed / in_secs)` in `In`. Non-finite or negative durations count as 0.

Export `Transition`, `TransitionEffect`, `TransitionPhase` and `TransitionState` from `lib.rs`.

Tests, in `crates/tungsten/src/tests/transition.rs`: `fade_progress_equals_cover`, `wipe_radial_is_inverted`, `wipe_radial_starts_at_the_far_corner`, `wipe_radial_softness_vanishes_at_full_cover`, `dissolve_progress_equals_cover`, `pixelate_block_runs_from_one_to_max`, `cover_is_clamped`, `out_reaches_boundary_at_out_secs`, `in_starts_fully_covered_and_drops_leftover_time`, `zero_durations_finish_on_the_first_step`, `easing_shapes_cover`.

**Check:** `cargo test -p tungsten --locked -q transition` passes.

**Done 2026-10-02.** `cargo test -p tungsten --locked -q transition` exit 0, 11 passed. The mapping table was checked against `wipe_radial.wgsl`, `fade.wgsl`, `dissolve.wgsl` and `pixelate.wgsl`. No deviation. Shape of the advance logic: `Transition::advance(&mut TransitionState, dt) -> TransitionStep` and `Transition::begin_in(&mut TransitionState) -> TransitionStep` (both `pub(crate)`), plus public `Transition::cover(state)` and `Transition::pass(state)`; `cover` clamps the eased value to [0, 1], so an overshooting easing cannot leave the range. A negative or non-finite `dt` does not move the clock. The crate warns about unused `advance`, `begin_in`, `START` and `TransitionStep` until step 6 uses them.

### 6. `StateStack` queueing, dispatcher, frame composition

Files: `crates/tungsten/src/state.rs`, `app.rs`, `tests/state.rs`, `tests/app.rs`.

`StateStack` gains two `pub(crate)` fields, a queued and an active transition, each holding the `Transition` and its `StateCommand` (`StateCommand` stays `pub(crate)`):

```rust
impl StateStack {
    /// Queue `push` behind `transition`. `false`, and the state is dropped, when a transition is queued or active.
    pub fn request_push_transition(&mut self, state: impl GameState, transition: Transition) -> bool;
    pub fn request_pop_transition(&mut self, transition: Transition) -> bool;
    pub fn request_replace_transition(&mut self, state: impl GameState, transition: Transition) -> bool;

    pub fn is_transitioning(&self) -> bool;                    // queued or active
    pub fn transition_state(&self) -> Option<TransitionState>; // active only
    pub fn transition_cover(&self) -> f32;                     // 0.0 when idle
    pub fn transition_pass(&self) -> Option<PostPass>;         // the pass to draw this frame
}
```

Dispatcher order per frame, in `state_dispatcher_system`:

1. Drain and apply the plain `pending` commands. Unchanged, so a plain request still applies at once, even during a transition.
2. If none is active, activate the queued transition (`Out`, `elapsed = 0`).
3. Advance the active transition by `DeltaTime` (0 when the resource is missing). On `Boundary`, apply its stored command with the same code as step 1, so hooks, scene despawn and order follow `D-046`. On `Finished`, clear it.
4. Update the top state and mirror `HudActiveState`, as today.

Move the body of the existing `match cmd` into one `fn apply_command(world: &mut World, cmd: StateCommand)` used by steps 1 and 3. States keep updating during both phases: the old state during `Out`, the new one during `In`. A game that wants input frozen checks `is_transitioning()`.

Frame composition (`app.rs`): `App` keeps a scratch `PostStack`. In `stage_render`, when `StateStack::transition_pass()` is `Some`, clear the scratch, copy the user's passes, push the transition pass last and render with the scratch; otherwise render with the user's stack as today. Put that in a small function over plain data so it can be tested without a renderer. The pass is the last post-stack slot, so it runs before the SMAA tail and the text overlay; `default_pass_order` and the timing labels already follow the stack's length.

Tests. `crates/tungsten/src/tests/state.rs` (insert a `DeltaTime` resource in the helper): `transition_defers_command_until_out_completes`, `transition_applies_command_once_at_boundary`, `replace_transition_fires_exit_then_enter_at_boundary`, `pop_transition_despawns_scene_entities_at_boundary`, `cover_rises_through_out_and_falls_through_in`, `transition_pass_is_none_when_idle`, `second_transition_request_is_rejected`, `plain_request_applies_during_transition`, `zero_duration_transition_applies_on_first_dispatch`, `top_state_updates_during_transition`. The eight existing tests pass unmodified. `crates/tungsten/src/tests/app.rs`: `transition_pass_is_appended_after_user_stack`, `no_transition_leaves_user_stack_untouched`.

**Check:** `cargo test -p tungsten --locked -q` and `cargo clippy -p tungsten --all-targets --locked -- -D warnings` pass.

**Done 2026-10-02.** `cargo test -p tungsten --locked -q` exit 0 (194 unit tests: the ten new state tests and two new app tests pass, the eight existing state tests pass unmodified apart from the `DeltaTime` resource in `make_world`); `cargo clippy -p tungsten --all-targets --locked -- -D warnings` exit 0. No deviation. Details the plan left open:

- The active transition holds its command as an `Option`, taken on the boundary frame; `QueuedTransition` and `ActiveTransition` are `pub(crate)` structs in `state.rs`.
- On the boundary frame the phase switches to `In` before the command runs, so the hooks see a fully covered frame. The transition is still active during those hooks, so a `request_*_transition` made from `on_enter` or `on_exit` is rejected.
- A queued transition has no cover and no pass until the dispatcher activates it.
- The frame composition function is `compose_post_stack(user, transition_pass, scratch)` in `app.rs`; `App` keeps the scratch as `transition_post_stack`.

### 7. Acceptance: example 03 transitions

Files: `examples/03_scene_state/src/main.rs`, `src/states.rs`, `tests/transition_regression.rs` (new), `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `justfile`.

| State change | Request | Effect |
| --- | --- | --- |
| menu → gameplay (`state_start`) | `request_replace_transition` | `Fade`, black |
| gameplay → pause (`state_pause`) | `request_push_transition` | `Pixelate` |
| pause → gameplay (`state_pause`) | `request_pop_transition` | `WipeRadial`, centered |
| gameplay → menu (`state_back`) | `request_replace_transition` | `Dissolve` |
| pause → menu (`state_back`) | `request_pop` then `request_replace`, as today | none (hard cut) |

- Under the default of open question 2, remove `spawn_fade_overlay`, `start_fade_out`, `PendingTransition`, `TransitionTarget`, `handle_tween_complete_system`, the `FADE_*` constants and the system registration in `main.rs`; update both module headers. The existing test `back_from_pause_removes_gameplay_before_entering_menu` stays and passes.
- Under the default of open question 1, `state_driven_text` multiplies each section's alpha by `1 - StateStack::transition_cover()`.
- `TUNGSTEN_TRANSITION_FIXTURE={none|fade|wipe_radial|dissolve|pixelate}`: any value turns the debug HUD off (its timing rows differ between runs); an effect name also requests menu → gameplay at startup with that effect, 0.1 s per phase, linear. With the smoke dt of 1/60 s and a first frame of dt 0, frame 5 is in `Out` at cover 0.67.
- `tests/transition_regression.rs`, modeled on example 04's `post_regression.rs` and gated by `TUNGSTEN_VISUAL_REGRESSION`: one test, `each_transition_effect_changes_the_frame`, captures frame 5 of 8 at 1280x720 for `none` and for each effect and requires more than 1,000 differing pixels per effect. `justfile`: add that test to the `visual` recipe.
- `scripts/smoke-examples.sh`: an "M31 fixture matrix" section after the M30 section, five rows: `example-04-shader-playground` with `TUNGSTEN_RENDER_MSAA=4 TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth` (the mesh pipeline under MSAA and a depth attachment), and `example-03-scene-state` with `TUNGSTEN_SMOKE_FRAMES=16` and each of the four effects, which crosses the boundary and finishes `In`. `scripts/test-smoke-examples.sh`: record the new env var in the stub's run line, add the five rows to the expected list and the section's "passed: 5/5" line to the expected output.

**Check:** `cargo test -p example-03-scene-state --locked -q` passes; `just script-test` passes; `just smoke` passes with the M31 section at 5/5; `just visual` passes.

**Done 2026-10-02** (Vulkan, Radeon 660M, no encoder running). `cargo test -p example-03-scene-state --locked -q` exit 0 (`back_from_pause_removes_gameplay_before_entering_menu` passes; the new test skips without the gate); `just script-test` exit 0; `just smoke` exit 0 with `Mesh/transition passed: 5/5` and every other section unchanged; `just visual` exit 0 (example 02: 2 passed, example 04: 3 passed, example 03: `each_transition_effect_changes_the_frame` passed); `cargo clippy -p example-03-scene-state --all-targets --locked -- -D warnings` exit 0.

- Pixels changed against `none` at frame 5, of 921,600: fade 921,600; radial wipe 878,575; dissolve 921,339; pixelate 44,893.
- Inspected by eye, frames 5, 8, 10 and 16 of a 16-frame run per effect: frame 5 shows the menu under the effect, frame 8 shows gameplay still mostly covered, frame 16 is identical for all four effects (gameplay, uncovered), so each run crosses the boundary and finishes `In`. The example's text dims with the cover and stays visible over the radial wipe's black, as open question 1 accepts. The radial wipe is an ellipse on a 16:9 window because the shader measures distance in UV.
- Smoke section name: `Mesh/transition passed` / `Mesh/transition failures`. The stub records the fixture as a trailing ` transition=<effect>` field, like ` cap=`, so the other expected rows are unchanged.
- Correction to this step's text: a smoke run steps 1/60 s on its first frame too, so frame 5 is in `Out` at cover 0.83 (measured from the fade's background pixel), not 0.67; frame 6 is the fully covered boundary frame and `In` ends at frame 12 of the 16-frame smoke rows. The test's doc comment says so.
- No deviation. Additions: an unknown fixture value is a startup error naming the accepted values (as `TUNGSTEN_POST_AA_FIXTURE` does in example 04); `GameplayState::default_scene()` lets `main.rs` build the startup request without exporting the scene path; the interactive transitions run 0.35 s per phase.

### 8. Decision `D-093` and documentation

`DECISIONS.md`: append `## D-093 — M31 instanced mesh particles and screen transitions`, in the house format (date, Decision, Why, Consequences). Content: the "Settled choices" table and "How instancing fits `D-051`" above, the hot-reload table of step 3, the per-effect mapping of step 5, the dispatcher order of step 6, the recorded limits below, tests by name, "no `unsafe`, no new dependency". Under the `D-051` heading add the marker line below, as `D-058` carries for `D-090`. Leave the rest of `D-051` and all of `D-046` as written: transitions extend the dispatcher without changing its hook matrix.

```markdown
**Amended by D-093:** a mesh particle carries `MeshParticle` in place of `Sprite`; entity-per-particle, the budget and the despawn path stand.
```

`docs/DECISION_INDEX.md`, in the same change (test-enforced): a `D-093` row after `D-091`, and "Amended by `D-093`" on the `D-051` row.

Other documents:

- `DESIGN.md`: a "Mesh Particles and Screen Transitions — M31" subsection after "Game Feel — M30"; a "Particle mesh (`particle_meshes` section, `D-093`)" row in the reload matrix; the transition API in "Scene / State System — M20"; `particle_meshes` wherever the manifest sections are listed (`rg -n 'materials' DESIGN.md`). Line 7 changes in step 10.
- `AGENTS.md`: a "Particle mesh" row in the Assets table (`manifest only`, `particle_meshes`, `vertices`, `indices`). The file has 48 bytes of headroom: shorten wording elsewhere in the same edit without dropping a rule.
- `crates/tungsten-render/AGENTS.md`: add `mesh_particle` to the internal, not manifest-tracked shaders (80 bytes of headroom).
- `.claude/skills/tungsten-wgpu/SKILL.md`: add `mesh_particle.rs` to the pipeline list.
- `docs/LLM_INDEX.md`: `render/mesh_particle.rs` on the Particles row with `D-093`; `tungsten/transition.rs` on the state-stack row. Stay under 8,192 bytes.
- `docs/known-issues.md`, "Recorded limits": transitions do not cover screen-space text; mesh particles draw above all sprites; at an odd window size the radial wipe leaves the center pixel at half brightness on the fully covered frame; pixelate never hides the frame.
- `docs/showcase/README.md`: status rows and capture recipes for the mesh trail (`TUNGSTEN_MESH_TRAIL_FIXTURE=off` against the default) and the four transitions (`TUNGSTEN_TRANSITION_FIXTURE`), per open question 4. Capture them, inspect the images and record commit, backend and result.
- `CHANGELOG.md`, `[Unreleased]`: a `Summary:` naming this plan and `D-093`; Added (mesh particles, `particle_meshes`, `ParticleConfig.render`, `Renderer::upload_particle_mesh` and `update_mesh_particles`, `Transition`, the three `request_*_transition` calls, the fixtures, the smoke and visual rows); Changed (`ParticleConfig.sprite` optional for mesh configs, example 03's fade). Do not add a version heading.

**Check:** `just ctx && just repo-check` passes (it runs the `decision_index` test), and `git diff --check` is clean.

**Done 2026-10-02.** `just ctx` exit 0 (`AGENTS.md` 6,127 of 6,144 bytes, `crates/tungsten-render/AGENTS.md` 4,033 of 4,096, `docs/LLM_INDEX.md` 7,168 of 8,192); `just repo-check` exit 0; `git diff --check` clean, and the new untracked files carry no trailing whitespace.

- `AGENTS.md`: the "Particle mesh" row cost 78 bytes; the sentence "Finish substantial work with `just check`" lost its parenthesis, which repeated the comment in the commands block. No rule was dropped.
- `D-093` also records the accepted `particles` regression of step 3 (regression policy) and what was not verified (a live manifest edit of a mesh). Its numbers are those of step 3; step 9 adds the final suite's reading.
- `DESIGN.md`: besides the listed edits, the Particle row of the reload matrix now says "sprite or mesh validation".
- Addition outside the step's list: `README.md` line 42 described example 03 as a "tween transition demo"; it now says "screen transition demo".
- Showcase, captured with the recipes into a scratch directory and inspected by eye: the mesh trail pair at frame 6 (943 pixels differ; yellow triangles over the first bouncer) and the five transition frames. The transition recipe overrides the helper's frame 6 with `TUNGSTEN_CAPTURE_FRAME=4` (cover 0.67), because frame 6 is the fully covered boundary frame. No image is checked in (open question 4).

### 9. Full verification

```bash
just check && just ctx && just repo-check && just script-test
just smoke && just visual                      # GPU and display
just perf suite --repeat 5
just perf compare <baseline A> <new suite> --fail-on regressed
```

A metric listed in step 0 as unstable between A and B does not count. For any other `regressed` verdict, follow `docs/perf/profiling-workflow.md` §Compare before accepting or reverting; do not accept a cost without the owner.

**Check:** every command above exits 0. Record the suite directory and the compare report path here.

**Done 2026-10-02, with an accepted compare failure** (Vulkan, Radeon 660M; no encoder sighting in any sitting).

- `just check && just ctx && just repo-check && just script-test` exit 0. `just smoke` exit 0 (every section full, `Mesh/transition passed: 5/5`). `just visual` exit 0 (example 02: 2 passed, example 04: 3, example 03: 1).
- Final suite: `perf-runs/20261002T143904Z-suite` (valid). `just perf compare perf-runs/20261002T134400Z-suite perf-runs/20261002T143904Z-suite --fail-on regressed` **exits 1**; report `perf-runs/20261002T144258Z-compare-suite/compare.md`. Owned verdicts: 1 regressed, 1 improved, 41 unchanged, 11 noisy.
  - **Regressed:** `churn` `stage.flush` p50, 2.41 → 2.54 ms (+5.5%, interval [+0.02, +0.24], τ 0.072). It read `unchanged` in the step 3 suite and between the baselines. Per run: 2.65, 2.47, 2.47, 2.50, 2.61 ms against 2.38–2.43 in baseline A. Two recaptures of the row on the same binary (`perf-runs/20261002T144337Z-churn`, `perf-runs/20261002T144345Z-churn`) read +0.08 and +0.10 ms, both `noisy`, each with one run at 2.62–2.63 ms.
  - What was checked: no M31 change touches the flush; `World::flush_reusing` is 3,665 bytes in the baseline and the final binary and sits at address mod 64 = 16 in the first and 32 in the second; the row's load windows read 1.22–1.28 busy CPUs, as in baseline A. Not done: the padded-baseline proof of the regression policy. The owner accepted regressions on 2026-10-02 ("I'm okay with regressions. Continue"), so this is recorded in `D-093` as an accepted reading and the plan goes on.
  - `particles` `stage.unattributed` p50, the step 3 regression: 2.07 → 2.10 ms, `noisy` here.
  - Improved: `ecs` `system.bounds_wrap` p50, 0.31 → 0.25 ms, the placement flip of step 3 again.
- A second agent session (another `claude` process) existed on the machine from about 14:21Z. It ran no build and used under 1% CPU; no run window of the final suite or the recaptures shows a load spike (largest one-second peak 1.49 busy CPUs).
- Evidence: `perf-runs/20261002-m31-evidence/` (`final.out`, `final.loadcheck.txt`, `compare-a-final.out`, `churn-re{1,2}.out`, `churn-re{1,2}-compare.out`).

### 10. Close out Phase 4 and hand off

Phase 4's done-when (`phase4.md` frontmatter): all seven milestones landed with archived plans; `DESIGN.md` status, `CHANGELOG.md` and `docs/DECISION_INDEX.md` updated; `phase4.md` marked done and archived.

1. In this file set `status: done`. In `phase4.md` set `status: done`, mark the M31 section shipped and add an M31 row to the shipped table (release `0.36.0`, `D-093`, this plan's archive path).
2. Fix relative links for one directory deeper in both files: `../LLM_INDEX.md` → `../../LLM_INDEX.md`, `../DECISION_INDEX.md` → `../../DECISION_INDEX.md`, `../../DECISIONS.md`, `../../DESIGN.md`, `../../AGENTS.md`, `../../crates/…` and `../../examples/…` → `../../../…`. The link between the two files stays `phase4.md`.
3. Move both with `mv` to `docs/plans/archive/` under the same basenames. Do not list or read that directory.
4. Repoint every inbound reference:

   | File | Now | Change to |
   | --- | --- | --- |
   | `DESIGN.md` line 7 | "Phase 4 M25–M30 shipped; [the roadmap](docs/plans/phase4.md) retains M31 …" | "Phases 3 and 4 are complete (M25–M31; roadmap archived at `docs/plans/archive/phase4.md`)." Keep the rest of the line. |
   | `README.md` line 22 | table row for `docs/plans/phase4.md` | remove the row |
   | `docs/README.md` line 12 | "Remaining Phase 4 scope" row | remove the row |
   | `docs/LLM_INDEX.md` line 71 | row lists `docs/plans/phase4.md` | drop that path; rename the task to "Documentation maintenance" |
   | `docs/plans/README.md` line 26 | "The [Phase 4 roadmap](phase4.md) remains active for M31. …" | "Phase 4 is complete; its roadmap is archived as `docs/plans/archive/phase4.md`." |
   | `CHANGELOG.md` lines 298, 318, 339 | links to `docs/plans/phase4.md` | keep the visible text, point the destination at `docs/plans/archive/phase4.md` (the M26 entry's archived-plan link is the precedent) |

   Leave `DECISIONS.md` and the plain-text mentions in older changelog entries as they are.
5. `DESIGN.md` line 5 (workspace version and release summary) and the version cut belong to branch preparation in `docs/releases.md`; do not edit them here.

**Check:** `rg -n 'plans/phase4\.md|\(phase4\.md\)' README.md DESIGN.md docs/README.md docs/LLM_INDEX.md docs/plans/README.md` prints nothing; `rg -n '\]\(docs/plans/phase4\.md\)' CHANGELOG.md` prints nothing; `fd . docs/plans --max-depth 1` lists neither file; `just ctx && just repo-check && just release-check` passes and `git diff --check` is clean.

Hand-off: report the changed-file list (`git status --short`) and the checks run, then continue with "Branch preparation" in [docs/releases.md](../../releases.md) when the owner asks for the release.

**Step 10 done 2026-10-02.** Appended after the move, without reading the archive.

- Both files carry `status: done`, their relative links are one directory deeper (`phase4.md`: 13 links from `../../` to `../../../` and 3 from `../` to `../../`; this file: `../releases.md` to `../../releases.md`), and both were moved with `mv -n` to `docs/plans/archive/` under the same basenames. `phase4.md` marks M31 shipped and lists it in the shipped table (release 0.36.0, `D-093`).
- Inbound references repointed as the table above says: `DESIGN.md` line 7, the `README.md` row and the `docs/README.md` row removed, the `docs/LLM_INDEX.md` row renamed to "Documentation maintenance", the `docs/plans/README.md` sentence, and three `CHANGELOG.md` link destinations (now lines 312, 332 and 353).
- Check: `rg -n 'plans/phase4\.md|\(phase4\.md\)' README.md DESIGN.md docs/README.md docs/LLM_INDEX.md docs/plans/README.md` prints nothing; `rg -n '\]\(docs/plans/phase4\.md\)' CHANGELOG.md` prints nothing; `fd . docs/plans --max-depth 1` lists `README.md` and `ui-text-suite-draft.md` only; `just ctx`, `just repo-check` and `just release-check` exit 0 ("workspace 0.35.0 = CHANGELOG.md [0.35.0]; [Unreleased] has entries"); `git diff --check` is clean.
- `DESIGN.md` line 5 and the version are untouched: the release cut belongs to "Branch preparation" in `docs/releases.md`.

**Done-when, as it stands:** mesh configs draw instanced triangles in example 04 and quad configs draw as before; example 03 changes state with each of the four effects; `just check`, `just smoke`, `just visual`, `just script-test`, `just ctx` and `just repo-check` pass; `D-093` and its index row exist and `D-051` carries the marker; both plans are archived with status done. **Not met as written:** "no owned metric reads regressed against the step 0 baseline". The final suite reads `churn` `stage.flush` p50 `regressed`, and the step 3 suite read `particles` `stage.unattributed` p50 `regressed`; the owner accepted both on 2026-10-02 and `D-093` records them.
