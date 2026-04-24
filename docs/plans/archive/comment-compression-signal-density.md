# status

Completed.

# goal

Compress comments across every Rust source file for signal density. Cut comments that restate code, historical narration, stale TODOs, decorative dividers, and commented-out code. Keep concise rustdoc and invariant notes where they protect behavior, ordering, asset rules, or known regressions.

# non-goals

- No M24 or feature work.
- No code logic, signature, import, or formatting changes outside comment lines.
- No dependency, manifest, shader, asset, or documentation cleanup beyond this plan and Rust comments.
- No broad architecture rewrite or rationale changes.

# files-to-touch

Every workspace `.rs` file outside `docs/plans/archive/`, sequenced as:

1. `crates/tungsten/src/app.rs`
2. `crates/tungsten/src/particles.rs`
3. ECS internals: `crates/tungsten-core/src/ecs/*.rs`
4. Render extraction paths: `crates/tungsten/src/sprite_extract.rs`, `crates/tungsten/src/tilemap_extract.rs`, `examples/01_platformer/src/extract.rs`
5. Remaining Rust files under `crates/`, `examples/`, benches, and integration tests.

# ordered-steps

1. Capture baseline comment-like line counts for every Rust file.
2. Edit `app.rs`; preserve frame-order, smoke-frame, event-flush, and D-0xx invariants.
3. Edit `particles.rs`; preserve lifecycle-order and deterministic-tick invariants.
4. Edit ECS internals; preserve storage, archetype, deferred command, event queue, borrow, and generation invariants.
5. Edit render extraction paths; preserve core/render seam and ID-to-handle extraction invariants.
6. Walk every remaining Rust file and apply the same cut/compress rules.
7. Record per-file before/after comment counts plus preserved invariant notes.
8. Run `cargo fmt --all` and `cargo test --workspace` unless comment-only changes make a narrower verification sufficient.
9. Confirm diff contains only comments plus this plan.

# done-when

- Every workspace `.rs` file has been reviewed.
- Comment-like line total is substantially reduced.
- Preserved comments are terse invariant notes or necessary public rustdoc.
- Per-file delta table and workspace totals are available.
- Source diff is restricted to comments.

# results

Reviewed Rust files: 135.
Full-line comment-like rows: 2,830 before, 746 after, -2,084 (-73.6%).

Verification:

- Comment-only stripped-content check: passed for 106 changed Rust files.
- `git diff --check`: passed.
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --workspace`: passed.

| path | before | after | delta | notes |
| --- | ---: | ---: | ---: | --- |
| `crates/tungsten-core/benches/action_map_bench.rs` | 3 | 1 | 2 | one-query-per-iteration invariant kept |
| `crates/tungsten-core/benches/ecs_bench.rs` | 59 | 6 | 53 | D-036/D-042 perf baselines kept |
| `crates/tungsten-core/benches/physics_bench.rs` | 5 | 1 | 4 | bench scenarios compressed |
| `crates/tungsten-core/src/assets/animation.rs` | 10 | 9 | 1 | D-010 animation data, zero-duration guard |
| `crates/tungsten-core/src/assets/atlas.rs` | 61 | 16 | 45 | deterministic pack order, non-mip UV padding |
| `crates/tungsten-core/src/assets/audio.rs` | 15 | 10 | 5 | eager PCM and manifest defaults |
| `crates/tungsten-core/src/assets/manifest.rs` | 16 | 8 | 8 | D-052 loaded graph, D-017 duplicate IDs |
| `crates/tungsten-core/src/assets/mod.rs` | 0 | 0 | 0 | asset comments reviewed |
| `crates/tungsten-core/src/assets/particle.rs` | 67 | 29 | 38 | Arc snapshot semantics, validation, deterministic RNG |
| `crates/tungsten-core/src/assets/registry.rs` | 22 | 11 | 11 | D-016 core/render handle seam, D-017 duplicate IDs |
| `crates/tungsten-core/src/assets/scene.rs` | 11 | 1 | 10 | D-046 scene sprite validation deferred |
| `crates/tungsten-core/src/assets/tilemap.rs` | 66 | 17 | 49 | D-032 tmj sprite IDs, D-007 atlas seam, collision/render layers |
| `crates/tungsten-core/src/audio.rs` | 21 | 15 | 6 | D-034 audio ring/callback ownership |
| `crates/tungsten-core/src/camera.rs` | 56 | 18 | 38 | camera matrix, no-compound zoom/shake |
| `crates/tungsten-core/src/components.rs` | 55 | 11 | 44 | D-042/D-033 render/physics separation, particle snapshots |
| `crates/tungsten-core/src/config.rs` | 3 | 2 | 1 | missing config fallback |
| `crates/tungsten-core/src/debug_draw.rs` | 13 | 5 | 8 | D-007/D-016 renderer-free seam |
| `crates/tungsten-core/src/display.rs` | 0 | 0 | 0 | ActionMap display hotkeys |
| `crates/tungsten-core/src/ecs/archetype.rs` | 70 | 18 | 52 | column equality, swap-remove, move_components_to caller duties |
| `crates/tungsten-core/src/ecs/command_buffer.rs` | 16 | 7 | 9 | pending handle lifetime, deferred flush boundary |
| `crates/tungsten-core/src/ecs/entity.rs` | 40 | 17 | 23 | generation stale-handle guard, D-036/D-022 |
| `crates/tungsten-core/src/ecs/event_queue.rs` | 22 | 10 | 12 | two-window flush lifetime |
| `crates/tungsten-core/src/ecs/mod.rs` | 0 | 0 | 0 | reviewed |
| `crates/tungsten-core/src/ecs/resource.rs` | 2 | 1 | 1 | type-indexed resources |
| `crates/tungsten-core/src/ecs/storage.rs` | 75 | 14 | 61 | archetype ID/empty archetype, edge caches, location updates |
| `crates/tungsten-core/src/ecs/world.rs` | 57 | 14 | 43 | D-036 storage, deferred despawn idempotence, flush order |
| `crates/tungsten-core/src/input.rs` | 12 | 5 | 7 | input edge-state lifecycle |
| `crates/tungsten-core/src/input/action_map.rs` | 54 | 20 | 34 | pure read-only action map, defaults/persist semantics |
| `crates/tungsten-core/src/input/key_serde.rs` | 4 | 1 | 3 | canonical input names |
| `crates/tungsten-core/src/inspect.rs` | 6 | 2 | 4 | inspector row trait |
| `crates/tungsten-core/src/lib.rs` | 3 | 1 | 2 | runtime/core comments reviewed |
| `crates/tungsten-core/src/physics/broadphase.rs` | 33 | 7 | 26 | scratch rebuild, generation-mark dedupe, cell-boundary rule |
| `crates/tungsten-core/src/physics/collision.rs` | 106 | 19 | 87 | contact normal convention, internal face filtering, degenerate normals, swept AABB |
| `crates/tungsten-core/src/physics/components.rs` | 25 | 12 | 13 | axis-aligned shape/body semantics |
| `crates/tungsten-core/src/physics/events.rs` | 10 | 2 | 8 | collision normal and tile None convention |
| `crates/tungsten-core/src/physics/mod.rs` | 36 | 6 | 30 | physics config semantics |
| `crates/tungsten-core/src/physics/step.rs` | 154 | 27 | 127 | substep order, no persistent broadphase, swept/slip guards, GS/event rules, tile masks |
| `crates/tungsten-core/src/rng.rs` | 27 | 10 | 17 | deterministic PCG/SplitMix, non-crypto |
| `crates/tungsten-core/src/tests/assets/animation.rs` | 2 | 0 | 2 | asset tests reviewed |
| `crates/tungsten-core/src/tests/assets/atlas.rs` | 5 | 1 | 4 | page-overflow pack invariant kept |
| `crates/tungsten-core/src/tests/assets/audio.rs` | 1 | 0 | 1 | asset tests reviewed |
| `crates/tungsten-core/src/tests/assets/manifest.rs` | 0 | 0 | 0 | asset tests reviewed |
| `crates/tungsten-core/src/tests/assets/particle.rs` | 0 | 0 | 0 | asset tests reviewed |
| `crates/tungsten-core/src/tests/assets/registry.rs` | 0 | 0 | 0 | asset tests reviewed |
| `crates/tungsten-core/src/tests/assets/scene.rs` | 0 | 0 | 0 | asset tests reviewed |
| `crates/tungsten-core/src/tests/assets/tilemap.rs` | 3 | 1 | 2 | D-032 tmj parsing kept |
| `crates/tungsten-core/src/tests/audio.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/camera.rs` | 5 | 1 | 4 | pre-M10 ortho compatibility kept |
| `crates/tungsten-core/src/tests/components.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/config.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/debug_draw.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/display.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/ecs/archetype.rs` | 13 | 1 | 12 | component-transfer invariant kept |
| `crates/tungsten-core/src/tests/ecs/command_buffer.rs` | 0 | 0 | 0 | ECS tests reviewed |
| `crates/tungsten-core/src/tests/ecs/entity.rs` | 2 | 0 | 2 | ECS tests reviewed |
| `crates/tungsten-core/src/tests/ecs/event_queue.rs` | 0 | 0 | 0 | ECS tests reviewed |
| `crates/tungsten-core/src/tests/ecs/storage.rs` | 40 | 1 | 39 | archetype transition/displacement invariant kept |
| `crates/tungsten-core/src/tests/ecs/world.rs` | 10 | 0 | 10 | ECS tests reviewed |
| `crates/tungsten-core/src/tests/input.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/input/action_map.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/input/key_serde.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/inspect.rs` | 0 | 0 | 0 | core tests reviewed |
| `crates/tungsten-core/src/tests/physics/broadphase.rs` | 1 | 0 | 1 | broadphase comments reviewed |
| `crates/tungsten-core/src/tests/physics/collision.rs` | 23 | 5 | 18 | normal-axis/sweep math kept |
| `crates/tungsten-core/src/tests/physics/step.rs` | 80 | 17 | 63 | tunneling, GS, seam, pile regressions kept |
| `crates/tungsten-core/src/tests/rng.rs` | 2 | 1 | 1 | SplitMix reference-vector guard kept |
| `crates/tungsten-core/src/time.rs` | 2 | 1 | 1 | delta-time resource |
| `crates/tungsten-core/tests/composition.rs` | 19 | 5 | 14 | D-052/D-017 composition contract kept |
| `crates/tungsten-core/tests/decision_index.rs` | 0 | 0 | 0 | integration tests reviewed |
| `crates/tungsten-core/tests/display.rs` | 0 | 0 | 0 | integration tests reviewed |
| `crates/tungsten-core/tests/manifests.rs` | 9 | 2 | 7 | D-017/D-035 manifest uniqueness kept |
| `crates/tungsten-core/tests/physics_timing.rs` | 12 | 1 | 11 | debug stdout timing-probe invariant kept |
| `crates/tungsten-render/benches/render_bench.rs` | 11 | 2 | 9 | CPU-only render bench and deterministic RNG kept |
| `crates/tungsten-render/src/debug_line.rs` | 19 | 2 | 17 | shared camera bind group, shader expansion |
| `crates/tungsten-render/src/image_diff.rs` | 9 | 1 | 8 | D-047 non-perceptual diff |
| `crates/tungsten-render/src/lib.rs` | 4 | 1 | 3 | render comments reviewed |
| `crates/tungsten-render/src/quad.rs` | 12 | 7 | 5 | shared camera uniform |
| `crates/tungsten-render/src/renderer.rs` | 64 | 29 | 35 | timestamp optionality, D-018 direct data, capture/timing stall |
| `crates/tungsten-render/src/screenshot.rs` | 16 | 6 | 10 | offscreen readback, poll-wait dev path |
| `crates/tungsten-render/src/sprite.rs` | 49 | 18 | 31 | POD layout, texture handle ownership, batch-slice alignment |
| `crates/tungsten-render/src/tests/debug_line.rs` | 0 | 0 | 0 | render tests reviewed |
| `crates/tungsten-render/src/tests/image_diff.rs` | 0 | 0 | 0 | render tests reviewed |
| `crates/tungsten-render/src/tests/renderer.rs` | 0 | 0 | 0 | render tests reviewed |
| `crates/tungsten-render/src/tests/screenshot.rs` | 0 | 0 | 0 | render tests reviewed |
| `crates/tungsten-render/src/tests/sprite.rs` | 0 | 0 | 0 | render tests reviewed |
| `crates/tungsten-render/src/text.rs` | 22 | 12 | 10 | font reload cache eviction, glyphon prepare/render |
| `crates/tungsten/benches/particle_tick.rs` | 10 | 2 | 8 | constant-population Criterion invariant kept |
| `crates/tungsten/src/app.rs` | 170 | 51 | 119 | frame order, event flush, startup dt, D-052/D-017/D-008, smoke/capture counters |
| `crates/tungsten/src/asset_loader.rs` | 141 | 28 | 113 | atlas UV/mip, D-052/D-017 composition, D-031 reload, D-046 scenes, D-053 audio |
| `crates/tungsten/src/audio.rs` | 31 | 10 | 21 | D-034 audio ring/callback ownership |
| `crates/tungsten/src/camera.rs` | 8 | 2 | 6 | camera matrix, no-compound zoom/shake |
| `crates/tungsten/src/debug_hud.rs` | 89 | 19 | 70 | HUD layout/borrow/refresh invariants |
| `crates/tungsten/src/display.rs` | 3 | 1 | 2 | ActionMap display hotkeys |
| `crates/tungsten/src/hot_reload.rs` | 39 | 11 | 28 | mpsc watcher, debounce, explicit-file filtering |
| `crates/tungsten/src/input_bridge.rs` | 0 | 0 | 0 | runtime/core comments reviewed |
| `crates/tungsten/src/inspector.rs` | 61 | 11 | 50 | pick order, stale entity clearing, cache key |
| `crates/tungsten/src/lib.rs` | 3 | 1 | 2 | runtime/core comments reviewed |
| `crates/tungsten/src/particles.rs` | 53 | 15 | 38 | lifecycle order, command-flush visibility, one-shot/pulse invariants |
| `crates/tungsten/src/physics_debug.rs` | 16 | 4 | 12 | D-042 authoritative physics debug state |
| `crates/tungsten/src/sprite_extract.rs` | 19 | 6 | 13 | D-042 visibility, stable z-order, z-run batching |
| `crates/tungsten/src/state.rs` | 41 | 15 | 26 | D-046 state lifecycle, D-039 scene despawns |
| `crates/tungsten/src/systems_overlay.rs` | 17 | 4 | 13 | D-044 independence, HUD stacking |
| `crates/tungsten/src/telemetry.rs` | 26 | 16 | 10 | frame timing resource semantics |
| `crates/tungsten/src/tests/app.rs` | 1 | 0 | 1 | tungsten tests reviewed |
| `crates/tungsten/src/tests/asset_loader.rs` | 16 | 3 | 13 | D-053/D-050 hot-reload invariants kept |
| `crates/tungsten/src/tests/audio.rs` | 1 | 0 | 1 | tungsten tests reviewed |
| `crates/tungsten/src/tests/debug_hud.rs` | 10 | 2 | 8 | HUD ownership/cache invariants kept |
| `crates/tungsten/src/tests/display.rs` | 0 | 0 | 0 | tungsten tests reviewed |
| `crates/tungsten/src/tests/hot_reload.rs` | 5 | 2 | 3 | watch path filtering invariants kept |
| `crates/tungsten/src/tests/inspector.rs` | 36 | 6 | 30 | picker/throttle regressions kept |
| `crates/tungsten/src/tests/physics_debug.rs` | 0 | 0 | 0 | tungsten tests reviewed |
| `crates/tungsten/src/tests/sprite_extract.rs` | 5 | 1 | 4 | z-order batch reset invariant kept |
| `crates/tungsten/src/tests/state.rs` | 0 | 0 | 0 | tungsten tests reviewed |
| `crates/tungsten/src/tests/systems_overlay.rs` | 2 | 1 | 1 | tungsten tests reviewed |
| `crates/tungsten/src/tests/telemetry.rs` | 0 | 0 | 0 | tungsten tests reviewed |
| `crates/tungsten/src/tilemap_extract.rs` | 22 | 4 | 18 | caller-owned ordering, render-only layers, layer order |
| `crates/tungsten/tests/asset_smoke.rs` | 17 | 1 | 16 | tungsten tests reviewed |
| `crates/tungsten/tests/atlas_integration.rs` | 11 | 2 | 9 | atlas batching seam kept |
| `crates/tungsten/tests/camera.rs` | 5 | 1 | 4 | camera zoom rewrite regression kept |
| `crates/tungsten/tests/particles.rs` | 19 | 5 | 14 | particle pipeline and hot-reload Arc invariant kept |
| `examples/01_platformer/src/extract.rs` | 29 | 6 | 23 | custom extract includes particles, draw-order notes, cursor mapping |
| `examples/01_platformer/src/main.rs` | 35 | 3 | 32 | example contract docs compressed |
| `examples/01_platformer/src/setup.rs` | 22 | 5 | 17 | D-052 startup and runtime order kept |
| `examples/01_platformer/src/state.rs` | 39 | 2 | 37 | spawn jitter and active-bounds invariants kept |
| `examples/01_platformer/src/systems.rs` | 53 | 9 | 44 | runtime ordering/physics invariants kept |
| `examples/01_platformer/src/tests/main.rs` | 20 | 8 | 12 | camera, spawn jitter, accumulator, active-hole invariants kept |
| `examples/02_sprite_stress/src/baseline.rs` | 6 | 1 | 5 | render-hot-path baseline kept |
| `examples/02_sprite_stress/src/ecs_high_load.rs` | 9 | 4 | 5 | self-contained perf synthetic sprite kept |
| `examples/02_sprite_stress/src/main.rs` | 30 | 5 | 25 | example comments reviewed |
| `examples/02_sprite_stress/src/shared.rs` | 3 | 1 | 2 | perf stdout invariant kept |
| `examples/02_sprite_stress/src/tests/ecs_high_load.rs` | 0 | 0 | 0 | self-contained perf synthetic sprite kept |
| `examples/02_sprite_stress/src/tests/main.rs` | 0 | 0 | 0 | example comments reviewed |
| `examples/02_sprite_stress/tests/visual_regression.rs` | 16 | 4 | 12 | D-002 opt-in GPU gate kept |
| `examples/03_scene_state/src/main.rs` | 13 | 3 | 10 | state flow docs compressed |
| `examples/03_scene_state/src/states.rs` | 8 | 3 | 5 | pause push/on_pause invariant kept |
