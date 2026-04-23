---
status: done
goal: Audit the current Tungsten workspace for major issues, pain points, scope creep, and testing imbalance.
non-goals: implementing fixes, rewriting architecture during the audit, GPU/display smoke validation, reviewing archived plans
files-to-touch: docs/plans/project-audit-2026-04-22.md
ordered-steps:
  1. Read repo guidance, subsystem index, crate/example surfaces, and relevant decision entries.
  2. Inspect major runtime, asset, render, example, script, and test hotspots.
  3. Run headless validation (`cargo test --workspace`, `cargo clippy --workspace --all-targets`) and static code-health scans.
  4. Summarize severity-ranked findings with concrete references and recommended next steps.
done-when:
  - Findings are ordered by severity.
  - Validation results and audit limits are recorded.
  - The report distinguishes correctness gaps from maintainability and scope issues.
---

## Implementation Status — 2026-04-22

Phases 1–4 from the Implementation Strategy below have landed on branch `0.21`. The audit itself is still `done`; the per-phase status is:

- **Phase 1 — asset-composition-contract: shipped.** `ResolvedManifest::load_and_merge_many` + `asset_loader::load_all_merged` + `App::set_manifest_roots` + `LoadedManifest` resource; examples 01 and 03 migrated; the workaround comment is gone. Headless coverage: `crates/tungsten-core/tests/composition.rs`. Rationale: `D-052`.
- **Phase 2 — hot-reload-matrix: shipped.** `DESIGN.md §Hot Reload — M9` publishes the one authoritative matrix; `reload_manifest` gained the particle manifest-add branch (mirroring tilemap validation) and a debug-log for session-static sound changes; `LoadedManifest` is refreshed on every successful manifest reload. Headless coverage: 7 new tests across `crates/tungsten/src/asset_loader.rs` (animation/tilemap/particle replace + last-known-good) and `crates/tungsten/src/hot_reload.rs` (path-filter accept/reject). Sprite/font reload still needs GPU and remains Layer 2. Rationale: `D-053`.
- **Phase 3 — app-frame-stage-decomposition: shipped inline.** The `RedrawRequested` arm is now a short list of `self.stage_*` calls (~60 lines incl. timing struct construction, down from ~340). Stage bodies live as methods on `App` with `FrameExtract` / `FrameRenderOut` / `FrameStageTimings` helper structs. Frame order is preserved byte-for-byte. `app.rs` total line count stayed close to the original — splitting into separate `frame/*.rs` files was deliberately deferred because the in-file method form avoided field-visibility and borrow-checker friction without changing behavior. Smoke (`scripts/smoke-examples.sh`) is the gate; not run here (requires GPU/display).
- **Phase 4 — perf-harness-split: shipped.** `examples/02_sprite_stress/src/main.rs` is 173 lines (CLI dispatch only); `baseline.rs` (161 lines), `ecs_high_load.rs` (792 lines, tests co-located), and `shared.rs` (77 lines) split by responsibility. Scene tests moved next to their scene. The `cargo run -p example-02-sprite-stress -- --release` + `STRESS_SCENE=ecs-high-load` invocation is unchanged, so `docs/perf/profiling-workflow.md` and `scripts/perf-capture.sh ecs-high-load 300` continue to resolve to the same binary.
- **Phase 5 — test-locality-cleanup: not pursued here.** Remains opportunistic per the plan ("do this opportunistically when the file is touched for another reason"); action_map.rs, physics/step.rs, renderer.rs tests are still embedded.

Validation at close-out: `cargo fmt --all` clean, `cargo test --workspace` = **418 passed, 1 ignored** (up from 402 pre-implementation), `cargo clippy --workspace --all-targets` = clean. Layer 2 (`scripts/smoke-examples.sh`) and the capture workflow need GPU/display and were not run from this session.

New `DECISIONS.md` entries: `D-052` (asset composition contract), `D-053` (hot-reload matrix + audio session-static).


# Tungsten Project Audit — 2026-04-22

## Scope

This audit covered the current workspace state across:

- `tungsten-core`
- `tungsten-render`
- `tungsten`
- in-tree examples
- perf/smoke scripts and key docs

The review was performed against the current working tree, not a clean checkout. There were already local modifications present in:

- `crates/tungsten-core/src/input/action_map.rs`
- `crates/tungsten/src/app.rs`
- `crates/tungsten/src/debug_hud.rs`
- `crates/tungsten/src/inspector.rs`
- `crates/tungsten/src/state.rs`
- `crates/tungsten/src/systems_overlay.rs`
- `examples/01_platformer/src/extract.rs`

I did not modify those files during the audit.

## Validation

Headless validation run during this audit:

- `cargo test --workspace` — passed
- `cargo clippy --workspace --all-targets` — passed

Additional audit signals gathered:

- file-size scan across `crates/` and `examples/`
- test-count and test-locality scan
- targeted review of asset loading, hot reload, runtime orchestration, perf harnesses, and example boundaries

Not run during this audit:

- `./scripts/smoke-examples.sh`

Reason: it requires a real GPU/display and is explicitly a separate validation layer.

## Executive Summary

The project is in better shape than many codebases at the same feature count: headless tests are green, `clippy` is clean, decisions are documented, and the repo still has a clear core/render/app split.

The biggest issues are not “the code is broken.” They are:

1. the asset-loading/runtime boundary no longer gives the repo’s multi-manifest model a single clear architectural home,
2. hot reload support is incomplete and lightly tested in exactly the areas where the code is most stateful,
3. the umbrella runtime is accumulating too many frame-stage responsibilities in one place.

The result is a codebase that is still functional, but is starting to rely on contributor memory and call-site discipline in places where the architecture claims a stronger invariant.

## Architecture Lens

This final pass narrows the audit around architectural pressure rather than code-size symptoms.

The major guardrails that still look sound and should stay fixed are:

- the three-crate split (`tungsten-core`, `tungsten-render`, `tungsten`) remains the right top-level boundary ([`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:34)),
- the frame boundary remains the engine’s main correctness story: systems, then flush, then hot reload, then extract, then render ([`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:66), [`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:95)),
- the extract-before-render seam and opaque asset handles are still the right core/render contract ([`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:101), [`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:81), [`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:91)),
- runtime display mutation belongs at the umbrella frame boundary, not inside systems or core ([`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:239)),
- scene/state runtime policy still belongs in `tungsten`, not `tungsten-core` ([`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:223), [`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:279)).

The strongest architectural pressure is concentrated in two places:

1. the asset-composition boundary, where the documented multi-manifest model lacks one first-class loading abstraction,
2. the umbrella runtime boundary, where `App` is absorbing too many ordered responsibilities.

The remaining findings are real, but they should be treated as secondary to those two seams.

## Findings

### High — Asset composition is under-specified at the loader boundary

The repo’s documented model is “multiple manifests compose by extension, never override” ([`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:145), [`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:161)). The architectural gap is that there is no single first-class abstraction that owns that composition. Instead, composition is encoded implicitly in per-type loader behavior, and only sprites happen to be additive today.

Evidence:

- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:228) `load_animations()` creates a fresh `AnimationRegistry` and replaces the world resource.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:250) `load_fonts()` creates a fresh `FontRegistry` and replaces the world resource.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:282) `load_sounds()` creates a fresh `SoundRegistry` and replaces the world resource.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:312) `load_tilemaps()` creates a fresh `TilemapRegistry` and replaces the world resource.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:337) `load_particles()` creates a fresh `ParticleConfigRegistry` and replaces the world resource.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:157) `load_sprites()` is the outlier: it extends existing state instead of replacing it.
- [`examples/01_platformer/src/setup.rs`](/home/joker/Projects/Tungsten/examples/01_platformer/src/setup.rs:153) contains an explicit workaround comment explaining that the local manifest cannot use `load_all()` because it would overwrite previously loaded registries.

Why this matters:

- The repo-level asset model says shared and example-local manifests compose, but the runtime does not enforce that model through one explicit boundary.
- The loader layer currently makes composition fragile and call-site dependent.
- Contributors now have to remember which loaders are additive and which are destructive.
- That is a missing architectural abstraction, not just a rough edge in a few functions.

Recommendation:

- Choose one explicit composition architecture and make it impossible to misuse at call sites.
- Either merge manifests into one resolved asset graph before decode/upload, or define additive registry semantics for every asset class.
- Back that contract with a headless `root manifest + local manifest` composition test for every asset type, not just sprites.

### High — Hot reload support is incomplete, and the riskiest reload paths are barely tested

Hot reload is part of the engine story, but the architecture’s support contract is no longer fully aligned across docs, code comments, and implementation. The frame-boundary mutation rule still looks right; the unclear part is which asset classes are truly supported and to what extent.

Evidence:

- [`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:184) says audio assets are not hot-reloadable for the session.
- [`DESIGN.md`](/home/joker/Projects/Tungsten/DESIGN.md:188) lists sprites, animations, fonts, and manifest as the covered hot-reload classes.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:880) documents `reload_manifest()` as if it generally registers new assets and warns on removals.
- The implementation handles sprites, animations, tilemaps, and fonts, then exits at EOF:
  - sprites: [asset_loader.rs](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:899)
  - animations: [asset_loader.rs](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:961)
  - tilemaps: [asset_loader.rs](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:996)
  - fonts: [asset_loader.rs](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:1040)
- There is no equivalent manifest-add/remove path for sounds or particles in that function.
- [`crates/tungsten/src/audio.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/audio.rs:33) initializes audio by cloning all decoded sound data into callback-owned `captured_sounds`, which means later sound-registry changes would not automatically reach the live mixer anyway.
- [`crates/tungsten/src/asset_loader.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs:1) has no unit tests at all.
- [`crates/tungsten/src/hot_reload.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/hot_reload.rs:180) has one trivial test, and it only checks the debounce constant.

Why this matters:

- The implementation surface implies more hot-reload support than is actually guaranteed.
- The supported matrix is partly documented, partly inferred, and partly contradicted by the code surface.
- Asset additions through manifest edits are currently asymmetric across asset classes.
- The most stateful code in the workspace (`reload_*`, watcher filtering, manifest reload merging, live runtime mutation) has some of the weakest direct coverage.

Recommendation:

- Publish one authoritative hot-reload matrix in both code and docs.
- Preserve the current frame-boundary mutation invariant while making the per-type support contract explicit.
- If sounds remain session-static because the mixer owns cloned PCM buffers, say so plainly and do not imply manifest-add reload for them.
- Add headless tests around the asset types the engine explicitly claims to hot reload, plus watcher path filtering.

### Medium — `App` has become the project’s de facto god object and implicit scheduler

The concern is not that `App` exists or that the umbrella crate owns runtime policy. That part is intentional ([`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:34), [`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:239), [`DECISIONS.md`](/home/joker/Projects/Tungsten/DECISIONS.md:279)). The problem is that those responsibilities are no longer decomposed internally, so one file is now carrying too much ordered behavior.

Evidence:

- [`crates/tungsten/src/app.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/app.rs:69) shows `App` owning config, windowing, renderer lifecycle, world resources, input maps, smoke frames, GPU timing flags, hot reload, screenshots, event queues, pacing, and engine-owned systems.
- [`crates/tungsten/src/app.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/app.rs:152) `App::new()` inserts a long list of engine resources and registers engine-owned systems/events up front.
- [`crates/tungsten/src/app.rs`](/home/joker/Projects/Tungsten/crates/tungsten/src/app.rs:954) `WindowEvent::RedrawRequested` runs the entire frame pipeline inline:
  - delta time
  - user systems
  - engine exit handling
  - particle refresh/emit/tick
  - command flush
  - event flush
  - hot reload
  - extract
  - debug draw expansion
  - HUD / systems overlay / inspector composition
  - render
  - audio command drain
  - telemetry update
  - control-flow pacing
  - smoke-frame exit
- The file is also the largest production file in the workspace at about `1300` lines.

Why this matters:

- This centralizes too much hidden order-dependence in one place.
- It raises the cost of adding or reviewing new runtime features.
- It increases the chance that future milestone work becomes “touch `app.rs` and hope frame order still works.”
- It makes code ownership fuzzier: runtime policy, dev tooling, perf tooling, and feature scheduling are all mixed together.
- It tempts future fixes to leak runtime concerns into `tungsten-core` or `tungsten-render`, which would be the wrong response.

Recommendation:

- Decompose the runtime inside `crates/tungsten`, not by smearing policy across crate boundaries.
- Extract explicit frame-phase coordinators or internal stage modules while preserving the current fixed order.
- Keep `App` as the boundary object, but move stage bodies out of the event-loop branch and away from the giant `RedrawRequested` arm.

### Medium — `example-02-sprite-stress` has become a scope magnet instead of a clean example

This example is now simultaneously:

- a demo example,
- the canonical perf baseline,
- the canonical render baseline,
- an ECS high-load scene,
- a telemetry/logging harness,
- a camera/overlay showcase,
- and a visual regression target.

Evidence:

- [`examples/02_sprite_stress/src/main.rs`](/home/joker/Projects/Tungsten/examples/02_sprite_stress/src/main.rs:1) documents two modes in one binary: `baseline` and `ecs-high-load`.
- [`docs/perf/profiling-workflow.md`](/home/joker/Projects/Tungsten/docs/perf/profiling-workflow.md:10) uses the same package as both the primary and secondary canonical perf scene.
- [`docs/perf/profiling-workflow.md`](/home/joker/Projects/Tungsten/docs/perf/profiling-workflow.md:33) makes that package the required child process for canonical captures.
- The file is also one of the largest in the repo at about `1150` lines, with production logic, harness logic, and tests all co-located.

Why this matters:

- Any change to this example now risks affecting perf baselines, regression images, demo readability, and stress-scene behavior at the same time.
- It is harder to reason about what counts as a “behavioral change” versus a “benchmark fixture change.”
- This is classic scope creep: a useful tool became the default home for every adjacent concern.

Recommendation:

- Split baseline render stress and ECS high-load stress into separate binaries or separate internal modules with clearer ownership.
- Decide whether this package is primarily a teaching example or a benchmark fixture, then give the other role a different home.
- Keep one canonical perf harness if needed, but stop making a single example do every diagnostic job.

### Low — Test-locality bloat is real, but it should not drive the architecture roadmap

This is not a “too many tests slow the repo down” problem. The suite is fast. The bloat is in review surface and file shape.

Evidence:

- `cargo test --workspace` stayed fast and green, so execution cost is healthy.
- Total test count is roughly `402`, which is large but still manageable for this repo size.
- Several already-large files are inflated further by embedded test modules:
  - [`crates/tungsten-core/src/physics/step.rs`](/home/joker/Projects/Tungsten/crates/tungsten-core/src/physics/step.rs:683) starts tests around line `683` in a `1200`-line file.
  - [`crates/tungsten-core/src/input/action_map.rs`](/home/joker/Projects/Tungsten/crates/tungsten-core/src/input/action_map.rs:611) starts tests around line `611` in a `910`-line file.
  - [`crates/tungsten-render/src/renderer.rs`](/home/joker/Projects/Tungsten/crates/tungsten-render/src/renderer.rs:939) starts tests around line `939` in a `1048`-line file.
  - [`examples/02_sprite_stress/src/main.rs`](/home/joker/Projects/Tungsten/examples/02_sprite_stress/src/main.rs:915) embeds tests directly in the already-large example binary.
  - [`examples/01_platformer/src/tests.rs`](/home/joker/Projects/Tungsten/examples/01_platformer/src/tests.rs:1) is itself a `643`-line companion test file.

Why this matters:

- These files are exactly where churn is likely to continue.
- Large embedded tests increase review fatigue and merge-conflict probability.
- The test budget is already generous; moving some of that budget toward under-tested asset/hot-reload seams would have better marginal value.
- In architecture terms, this is secondary to the asset-composition and runtime-boundary issues above.

Recommendation:

- Keep the test count, but rebalance the organization.
- Move the biggest embedded test suites into focused neighboring modules where that reduces file bulk.
- Spend the next chunk of test effort on loader/reload composition rather than on already well-covered ECS and math paths.

## What I Did Not Find

- No headless red flags in the workspace today: tests pass and `clippy` is clean.
- No obvious dependency-policy violations against the current `D-015` rules.
- No sign of runaway execution-time test bloat right now; the current problem is mostly coverage shape and code-shape, not raw test duration.

## Suggested Order Of Attack

1. Write down the asset-composition contract first.
   Decide whether the engine owns a merged/resolved manifest graph or additive per-type registries, then make the loader match it everywhere.

2. Stabilize the hot-reload contract second.
   Publish one supported reload matrix, keep the frame-boundary rule, and align docs/comments/implementation around the same promise.

3. Split `App` frame orchestration into smaller runtime-phase modules before M24.
   Do that decomposition inside `tungsten` so the current crate seam and frame order remain intact.

4. Give the perf harness a clearer architectural home.
   Treat benchmark fixtures and showcase examples as related but not identical concerns, even if they continue sharing code.

5. Clean up test locality opportunistically while touching affected files.
   This is worth doing, but it should follow the architecture fixes above rather than compete with them.

## Audit Bottom Line

Tungsten is not in “major defect” territory. It is in “successful phase transition” territory: the engine has accumulated enough capability that several once-reasonable shortcuts are now turning into architectural drag.

The three-crate split still looks right. The main architectural pressure is at the asset-composition boundary and inside the umbrella runtime. If the next cycle addresses those two seams first, the project will be in a much stronger position for M24 and beyond.

---

# Expansion — 2026-04-22 (Technical Detail + Implementation Strategy)

The original audit ends above. The following sections expand each finding with concrete code references and a sequenced implementation strategy. The audit itself stays `done`; each implementation phase below is a candidate for its own `docs/plans/<topic>.md` execution plan once the direction is approved. Nothing in this expansion modifies code — it tightens the spec for the work that should follow.

## Verified Against Current Tree

The audit's findings hold against the working tree at `0.20`. Specific re-checks (referenced again throughout the strategy below):

- `ResolvedManifest::merge` already exists at [crates/tungsten-core/src/assets/manifest.rs:263-301](../../crates/tungsten-core/src/assets/manifest.rs#L263-L301) and is the natural primitive for "merge before load." It enforces `D-017` duplicate-ID rules and returns `ManifestError::DuplicateId`.
- The destructive loaders (`load_animations`, `load_fonts`, `load_sounds`, `load_tilemaps`, `load_particles`) all end with `world.insert_resource(...)`, replacing the prior registry. `load_sprites` is the only additive variant — and even there the additivity is implicit (`mem::take` → mutate → re-insert).
- `reload_manifest` ([crates/tungsten/src/asset_loader.rs:883-1068](../../crates/tungsten/src/asset_loader.rs#L883-L1068)) handles sprite / animation / tilemap / font additions. Sounds and particles have no manifest-add path.
- `AudioSystem::init` clones decoded PCM into `captured_sounds: HashMap<AudioHandle, Vec<f32>>` ([crates/tungsten/src/audio.rs:59-62](../../crates/tungsten/src/audio.rs#L59-L62)). The mixer thread reads from this map directly, so any post-init `SoundRegistry` mutation is invisible to the live mixer without a new command-channel message.
- `App::new` inserts ~25 resources in one block ([crates/tungsten/src/app.rs:152-220](../../crates/tungsten/src/app.rs#L152-L220)) and the `RedrawRequested` arm is a single ~340-line inline pipeline ([crates/tungsten/src/app.rs:954-1293](../../crates/tungsten/src/app.rs#L954-L1293)).
- Top-N file sizes (current): app.rs 1301, sprite_stress/main.rs 1153, physics/step.rs 1200, asset_loader.rs 1068, renderer.rs 1048, action_map.rs 910.

## Technical Deep-Dive Per Finding

### F1 deep-dive — Asset composition boundary

**Status today.** The "compose, never override" model from `D-017` is enforced inside `ResolvedManifest::merge`, but never used by the runtime. Every example individually loads a root manifest, then a local manifest, and per-type loaders decide their own composition policy:

- `load_sprites` is additive against `AtlasRegistry` (via `mem::take` + rebuild).
- `load_animations` / `load_fonts` / `load_sounds` / `load_tilemaps` / `load_particles` each construct a fresh registry and `world.insert_resource(...)` it, dropping anything from the prior call.

Result: the only safe call shape is "load every manifest's per-type subset by hand, in the right order, skipping `load_all` after the first." That is exactly what the workaround comment in [examples/01_platformer/src/setup.rs:153-156](../../examples/01_platformer/src/setup.rs#L153-L156) documents.

**Two viable architectures** (the implementation strategy picks A; B is here for completeness):

- **A. Merge-first.** The umbrella owns one `ResolvedManifest` per app session, built from a `&[PathBuf]` list of manifest roots via `ResolvedManifest::merge`. The per-type loaders run exactly once on the merged graph. This collapses N call sites into one, makes duplicate-ID conflicts hard errors at boot, and matches the existing `merge` primitive without inventing new abstractions.
- **B. Additive registries.** Every registry grows an `extend_from(&ResolvedManifest, ...)` method and the per-type loaders mutate-in-place. Closer to the current `load_sprites` shape, but multiplies the number of methods that need symmetric removal/replace semantics for hot reload, and pushes more bookkeeping into each registry type.

Strategy below adopts **A**. Reasons: `ResolvedManifest::merge` already enforces the D-017 invariant; merging once means there is exactly one source of truth for "what's loaded" at startup; and the additive case (hot-reload manifest add) becomes "merge into the existing `ResolvedManifest`, then run a delta loader" rather than two parallel APIs.

**The key thing to design.** Whether the merged `ResolvedManifest` becomes a long-lived `World` resource or stays scoped to the loader. Resource-shaped is more useful (hot-reload can diff against it), but it duplicates state with the per-type registries; loader-scoped is simpler but forces recomputation on each manifest reload. Recommendation: long-lived as a resource, named `LoadedManifest`, with `reload_manifest` mutating it after a successful merge.

### F2 deep-dive — Hot reload contract

**Today's matrix (verified):**

| Asset class | Single-file edit reload | Manifest-add reload | Manifest-remove reload |
| --- | --- | --- | --- |
| Sprite (PNG) | yes — `reload_sprite` ([asset_loader.rs:496](../../crates/tungsten/src/asset_loader.rs#L496)) | yes — `reload_manifest` (`rebuild_atlas_for_filter`) | warn-only ("keeping stale") |
| Animation (JSON) | yes — `reload_animation` | yes | warn-only |
| Tilemap (TMJ) | yes — `reload_tilemap` | yes (with sprite-id revalidation) | warn-only |
| Font (TTF/OTF) | yes — `reload_font` | yes | warn-only |
| Particle (JSON) | yes — `reload_particle` | **no** — not handled in `reload_manifest` | n/a |
| Sound (decoded PCM) | **no reload path** | **no** | n/a — mixer owns clones |
| `input.json` | yes — `reload_action_map` | n/a | n/a |
| `manifest.json` | yes — `reload_manifest` | n/a | n/a |

**The mixer constraint.** `AudioSystem::init` reads every `SoundData::samples` clone into `captured_sounds: HashMap<AudioHandle, Vec<f32>>` and the mixer closure captures it ([audio.rs:59-87](../../crates/tungsten/src/audio.rs#L59-L87)). The mixer's only inbound channel today is `rtrb` (`PlaySound { handle }` per `D-034`). Hot-reloading sound data requires either:

- a new mixer command (`UpdatePcm { handle, samples: Arc<[f32]> }`) on the existing or a parallel control ring, with the mixer swapping its `Arc` between callbacks, or
- a documented "session-static" rule and zero attempt at sound hot reload.

The cheaper architectural move is the second: state plainly that audio is session-static (already in [DESIGN.md:184](../../DESIGN.md#L184)) and align doc + code. The mixer command path is real work and should be its own milestone if it ships.

**Coverage gap.** [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) has no `#[cfg(test)] mod tests`. [crates/tungsten/src/hot_reload.rs](../../crates/tungsten/src/hot_reload.rs) has one assertion (`debounce_constant_is_50ms`). The riskiest mutation surface in the workspace has near-zero direct unit coverage. `cargo test --workspace` doesn't exercise reload paths anywhere — they only run in interactive sessions.

### F3 deep-dive — `App` decomposition

The `RedrawRequested` arm at [crates/tungsten/src/app.rs:954-1293](../../crates/tungsten/src/app.rs#L954-L1293) runs, in order:

1. `apply_pending_display_request` (display delta apply at frame boundary, `D-043`).
2. Snapshot `prev_total_ms` for HUD smoothing.
3. Delta-time stamp.
4. Update stage: iterate `self.systems` with per-system `Instant::now()`.
5. Engine exit guard (Escape → `event_loop.exit()`).
6. Particle stage: `count_refresh → emit → tick`.
7. Command-buffer flush (`D-039`).
8. Event-queue flush loop.
9. `process_hot_reload`.
10. Extract: quads, sprites, text closures.
11. `RenderCounts` populate.
12. `physics_debug_emit_system` then `DebugDraw` drain → expand AABB / circle / line.
13. HUD compose (remove → fill → re-insert dance).
14. Systems-overlay compose (same dance).
15. Inspector compose (same dance).
16. Render: `render_frame_full[_timed]` + screenshot capture arming.
17. Audio command drain to `cpal` ring.
18. `InputState::begin_frame` (clear edge state).
19. `FrameTimings` populate.
20. Optional `TUNGSTEN_PERF_LOG` emit.
21. `request_redraw`.
22. Frame-budget pacing (`ControlFlow::WaitUntil` or `Wait`).
23. `smoke_frames_remaining` decrement → exit.

That ordering is the engine's correctness story. The decomposition target is **not** to change the order or smear it across crates. It's to give each phase a name and a function so the `RedrawRequested` arm reads as a 20-line list of stage calls.

A natural seam: introduce `crate::frame::Stages` (umbrella-only, `pub(crate)`), each method taking `&mut self.world`, `&mut self.renderer`, plus the `&ActiveEventLoop` it needs. Stages stay borrow-compatible with the existing closure types because the `Box<dyn Fn(&World) -> ...>` extract slots live on `App`, not inside the stage struct.

### F4 deep-dive — `example-02-sprite-stress` scope

The package is wearing five hats simultaneously:

1. The `STRESS_SCENE=baseline` mode is the M12 sine-wave demo.
2. The `STRESS_SCENE=ecs-high-load` mode is a 50k-entity flow-field simulation.
3. The package is the *primary* and *secondary* canonical perf scene per [docs/perf/profiling-workflow.md:10-33](../perf/profiling-workflow.md#L10-L33).
4. Embedded `#[cfg(test)] mod tests` (around line 915) verifies extract behavior.
5. Generates a synthetic sprite (`HIGH_LOAD_SPRITE_ID = "ex02_high_load_agent"`, `HIGH_LOAD_SPRITE_PATH = "__generated__/ex02_high_load_agent.png"`) bypassing the manifest path.

The cleanest split: keep one *binary* but extract the two scenes into `mod baseline;` and `mod ecs_high_load;` siblings under `examples/02_sprite_stress/src/`, with `main.rs` containing only `StressScene` parsing + dispatch. Tests move next to the scene they exercise. The perf workflow does not need to change — `cargo run -p example-02-sprite-stress -- --release` with `STRESS_SCENE=ecs-high-load` is already the canonical incantation.

If `baseline` is genuinely a teaching example and `ecs-high-load` is the perf scene, splitting into two binaries (`example-02a-sprite-baseline`, `example-02b-ecs-high-load`) makes the scope-creep impossible to recreate. That is a heavier change but a clearer end state.

### F5 deep-dive — Test locality

This is intentionally lower priority. Concrete moves available when each file is touched:

- [crates/tungsten-core/src/physics/step.rs:683+](../../crates/tungsten-core/src/physics/step.rs#L683) → move to `crates/tungsten-core/src/physics/step_tests.rs` and re-export with `#[path = "step_tests.rs"] mod tests;`.
- Same pattern for [crates/tungsten-core/src/input/action_map.rs:611+](../../crates/tungsten-core/src/input/action_map.rs#L611) and [crates/tungsten-render/src/renderer.rs:939+](../../crates/tungsten-render/src/renderer.rs#L939). The umbrella already uses this pattern (`#[path = "app_tests.rs"] mod tests;` at [app.rs:1300](../../crates/tungsten/src/app.rs#L1300)).
- The platformer's [tests.rs](../../examples/01_platformer/src/tests.rs) is already extracted; it is large but in the right shape.

## Implementation Strategy

Five phases, ordered by architectural leverage. Each phase is a candidate for a dedicated execution plan; the heading shows the suggested filename.

### Phase 1 — `asset-composition-contract.md` (highest leverage)

**Goal.** The umbrella loads N manifests through one explicit primitive: merge, then load. No call site decides composition policy.

**Files to touch.**
- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) — add `load_all_merged(roots: &[PathBuf], world, renderer)`; mark `load_animations` / `load_fonts` / `load_sounds` / `load_tilemaps` / `load_particles` as accepting `&ResolvedManifest` already (no signature change), but document that they replace registries and should not be called for composition.
- [crates/tungsten/src/app.rs](../../crates/tungsten/src/app.rs) — add `App::set_manifest_roots(&mut self, roots: Vec<PathBuf>)`; have `App` call `load_all_merged` from the existing startup path *before* the user's `on_startup` closure. Insert the merged graph as a `LoadedManifest(ResolvedManifest)` resource.
- [examples/01_platformer/src/setup.rs](../../examples/01_platformer/src/setup.rs), [examples/03_scene_state/src/main.rs](../../examples/03_scene_state/src/main.rs) — replace the manual root+local sequence with `app.set_manifest_roots(vec![ROOT, LOCAL])`. Delete the workaround comments.
- [crates/tungsten-core/src/assets/manifest.rs](../../crates/tungsten-core/src/assets/manifest.rs) — `merge` stays as is. Add a focused test `merge_compose_two_manifests_disjoint_ids` if not already present.
- [DECISIONS.md](../../DECISIONS.md) — add `D-048 Composition contract: umbrella merges, loaders never compose.` Cite as supersedes-clarification of `D-017`.

**Steps.**
1. Add `LoadedManifest(ResolvedManifest)` resource type in `tungsten-core::assets`.
2. Implement `load_all_merged`: fold the `&[PathBuf]` through `ResolvedManifest::merge`, store it as the `LoadedManifest` resource, then call `load_all` once on the merged graph.
3. Wire `App::set_manifest_roots` + automatic invocation in the startup hook (kept *before* the user's `on_startup` closure so user code can spawn against loaded assets).
4. Migrate examples 01 and 03 to the new entry point. Delete the workaround comments.
5. Add a headless integration test: `crates/tungsten-core/tests/composition.rs` builds a temp dir with two manifests covering every asset type, merges, asserts no panics and full coverage.
6. Add a duplicate-ID conflict test confirming `D-017` still bites.
7. Update [docs/LLM_INDEX.md](../LLM_INDEX.md) "Asset manifest, registry, IDs" row to include `LoadedManifest`.

**Done-when.**
- `cargo test --workspace` includes the composition test and passes.
- Examples 01 and 03 no longer reference the per-type loaders directly.
- `DECISIONS.md` has the new entry.
- The workaround comment is gone from the working tree.

**Risks.**
- Test asset directories under `examples/*/assets/` may have unintended duplicate IDs that boot only because today's loaders silently overwrite. The composition test will surface them as failures — fixing the IDs is in scope; widening composition semantics to "last wins" is not.
- `App::set_manifest_roots` plus an `on_startup` closure that *also* calls a loader directly should stay supported (back-compat for example 02 which currently builds a synthetic sprite). Keep the per-type loaders public but document them as advanced.

### Phase 2 — `hot-reload-matrix.md`

**Goal.** One published reload matrix that code, tests, and docs all agree on.

**Files to touch.**
- [DESIGN.md](../../DESIGN.md) §hot-reload — replace prose with a table identical in shape to F2 above.
- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) — add `reload_manifest` paths for particles (manifest-add → registry insert + sprite-id revalidate, mirroring tilemap) and an explicit "sounds: not supported" branch that logs at debug level.
- Either drop the `reload_sound`-shaped TODO from comments or implement it (out of scope here unless audio is in scope for the cycle).
- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) — add `#[cfg(test)] mod tests` covering: `reload_manifest_adds_animation`, `reload_manifest_adds_particle`, `reload_manifest_warns_on_remove`, `reload_sprite_in_place_when_fits`, `reload_sprite_rebuilds_on_growth`. These run headless if they avoid GPU upload — refactor `build_atlas_for_filter` to accept an injectable `dyn TextureSink` trait or split the pure-CPU packing path from the upload path so tests can hit only the packing logic.
- [crates/tungsten/src/hot_reload.rs](../../crates/tungsten/src/hot_reload.rs) — add tests for `accept()` path filtering: extra-file match, recursive-root match, parent-directory non-match.

**Steps.**
1. Document the matrix in `DESIGN.md`.
2. Implement the missing particle manifest-add branch in `reload_manifest`.
3. Add the explicit sounds branch (warn-once or debug-log).
4. Refactor `build_atlas_for_filter` so the CPU-side packing is testable without `Renderer`.
5. Land headless tests for each row of the matrix.
6. Add `D-049 Hot-reload matrix and audio session-static invariant`.

**Done-when.**
- The published table matches the implementation row-for-row.
- `cargo test -p tungsten` adds at least 6 reload-path tests.
- `cargo test --workspace` stays green.

**Risks.**
- Splitting CPU-pack from GPU-upload is a non-trivial internal refactor; if it grows, scope it as a sub-step before the test additions.
- Particle manifest-add validation must reject unknown sprite IDs (mirrors tilemap path) — make sure the rejection log message is consistent with the tilemap one.

### Phase 3 — `app-frame-stage-decomposition.md`

**Goal.** `App::window_event`'s `RedrawRequested` arm reads as a sequence of named stage calls. Frame order does not change. Crate boundaries do not change.

**Files to touch.**
- New: `crates/tungsten/src/frame/mod.rs` — declares submodules and a `pub(crate) struct FrameContext<'a>` carrying `&mut World`, `&mut Option<Renderer>`, `&ActiveEventLoop`, `&mut Option<AudioSystem>`, plus extract closures and frame-counter fields.
- New: `crates/tungsten/src/frame/{update.rs, particles.rs, flush.rs, hot_reload.rs, extract.rs, debug_compose.rs, render.rs, audio.rs, telemetry.rs, pacing.rs, smoke.rs}` — one file per stage. Each exposes a single `pub(crate) fn run(ctx: &mut FrameContext) -> StageResult` that owns its existing inline body.
- [crates/tungsten/src/app.rs](../../crates/tungsten/src/app.rs) — `RedrawRequested` arm collapses to ~25 lines of `frame::<stage>::run(&mut ctx)?;` calls.

**Steps.**
1. Cut the existing inline blocks into stage modules verbatim, preserving telemetry timing harness around each. Do not change ordering, do not refactor any branch.
2. Land the move as one PR with no behavior change. `cargo test --workspace` and `scripts/smoke-examples.sh` are the gates.
3. Once mechanical: collapse the HUD/systems-overlay/inspector "remove resource → mutate → re-insert" dance into a `World::with_resource_mut::<R>(|r, world|)` helper if it shows up identically three times. (Probably out of scope — flag for follow-up.)
4. Optional: extract `App::new` resource insertion into `crate::bootstrap::insert_engine_resources(&mut world)`.

**Done-when.**
- `RedrawRequested` arm shrinks to ≤50 lines.
- `app.rs` total lines drop below ~700.
- `cargo test --workspace` passes; `scripts/smoke-examples.sh` passes on Linux.

**Risks.**
- Borrow-checker friction around `&mut self.world` plus `&mut self.renderer` simultaneously. Mitigation: `FrameContext` carries `&mut Option<Renderer>` and methods early-return if `None`, mirroring today's check.
- The `event_loop.exit()` calls inside particle/render branches need to bubble through `StageResult`. Use a small `enum StageOutcome { Continue, Exit }`.
- This phase has no unit-test surface change. Layer 2 (`smoke-examples.sh`) is the gate. Document this explicitly in the plan so reviewers don't expect new tests.

### Phase 4 — `perf-harness-split.md`

**Goal.** The benchmark fixture and the demo example stop sharing one binary file.

**Files to touch.**
- [examples/02_sprite_stress/src/main.rs](../../examples/02_sprite_stress/src/main.rs) → split into `main.rs` (CLI dispatch only), `baseline.rs`, `ecs_high_load.rs`. Move embedded tests next to their scene.
- [docs/perf/profiling-workflow.md](../perf/profiling-workflow.md) — confirm the canonical command line still works (it should — the package name is unchanged).

**Steps.**
1. Mechanical split into modules. No behavior change.
2. If the user wants the harder split into two binaries, do it as a follow-up with new package names; defer until F4 has been touched once and the boundaries are obvious.
3. Verify `./scripts/perf-capture.sh ecs-high-load 300` still resolves to the same binary path.

**Done-when.**
- `examples/02_sprite_stress/src/main.rs` is ≤200 lines.
- Tests run via `cargo test -p example-02-sprite-stress`.
- `scripts/perf-capture.sh ecs-high-load 300` produces output identical-in-shape to a pre-split run.

**Risks.**
- `STRESS_SCENE` defaulting to `baseline` must be preserved for backward compatibility with any unrecorded muscle memory.

### Phase 5 — `test-locality-cleanup.md` (opportunistic)

**Goal.** Reduce file bulk in the largest source files without changing test count or behavior.

**Files to touch.**
- [crates/tungsten-core/src/physics/step.rs](../../crates/tungsten-core/src/physics/step.rs) — extract tests to `step_tests.rs` via `#[path = "step_tests.rs"] mod tests;`.
- [crates/tungsten-core/src/input/action_map.rs](../../crates/tungsten-core/src/input/action_map.rs) — same pattern.
- [crates/tungsten-render/src/renderer.rs](../../crates/tungsten-render/src/renderer.rs) — same pattern.

**Strategy.** Do this opportunistically when the file is touched for another reason. Do not schedule it as standalone work; the architectural payoff is too small.

## Sequencing Summary

| Order | Phase | Why this slot |
| --- | --- | --- |
| 1 | `asset-composition-contract` | Highest leverage; unblocks 2; deletes the workaround comment that documents real architectural debt. |
| 2 | `hot-reload-matrix` | Builds on the merged-manifest resource; closes the largest under-tested seam. |
| 3 | `app-frame-stage-decomposition` | Pure restructuring; do it before M24 so new tween code lands in named stage modules from day one. |
| 4 | `perf-harness-split` | Mechanical; can run in parallel with 1–3; defer if M24 is in flight. |
| 5 | `test-locality-cleanup` | Opportunistic; never standalone. |

## What This Plan Deliberately Does Not Do

- It does not propose moving runtime policy out of `tungsten` into `tungsten-core` or `tungsten-render`. `D-007` and `D-016` keep that boundary, and the audit confirmed it is sound.
- It does not propose hot-reloading audio. That is its own milestone with its own decision cost (lock-free PCM swap on the mixer thread). The fix here is to align the docs with the existing session-static reality.
- It does not propose changing the frame order. `D-018`/`D-039`/`D-040` lock that down; the stage decomposition preserves it byte-for-byte.
- It does not propose a CI pipeline, even though phase 2 would benefit from one. `AGENTS.md` "What This Project Is Not Doing" still applies.

## Validation Path Per Phase

Both gates must stay green at every phase boundary:

- Phase 1, 2, 4, 5: `cargo test --workspace` (layer 1) covers the new tests and existing unit suites.
- Phase 2 additionally needs the watcher-path tests under `cargo test -p tungsten`.
- Phase 3: `cargo test --workspace` plus `./scripts/smoke-examples.sh` (layer 2). Phase 3 has no new unit tests — smoke is the only behavioral gate, by design.
- All phases: `cargo clippy --workspace --all-targets` stays clean.
