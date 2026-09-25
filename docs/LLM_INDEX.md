# LLM Navigation Index

Task → file map. Open it on demand before a broad search, not by default. Prefixes: `core/` = `crates/tungsten-core/src/`, `render/` = `crates/tungsten-render/src/`, `tungsten/` = `crates/tungsten/src/`; other paths start at the repo root. `tungsten/app.rs` is the frame-loop hub and touches most runtime rows. `D-0NN` IDs resolve through [`DECISION_INDEX.md`](DECISION_INDEX.md).

## Runtime and ECS

| Task | Open |
| --- | --- |
| ECS storage/query bug | `core/ecs/world.rs`, `core/ecs/storage.rs`, `core/ecs/archetype.rs`, `core/lib.rs` |
| Deferred spawn/despawn (`D-039`) | `core/ecs/command_buffer.rs` |
| Event lifetime/flush (`D-040`) | `core/ecs/event_queue.rs` |
| Frame order, smoke-frame exit, winit loop (`D-018`, `D-043`) | `tungsten/app.rs`, `tungsten/lib.rs` |
| Config parsing, env overrides | `core/config.rs`, `core/display.rs`, `tungsten.json` |
| Display apply, fullscreen, vsync, frame cap (`D-043`) | `tungsten/display.rs`, `core/display.rs` |
| Input actions, `input.json`, rebind reload (`D-045`) | `core/input/action_map.rs`, `core/input/key_serde.rs`, `tungsten/asset_loader.rs`, `tungsten/debug_hud.rs`, `tungsten/display.rs`, `input.json` |
| Camera follow/zoom/bounds | `core/camera.rs`, `tungsten/camera.rs`, `examples/01_platformer/src/setup.rs`, `examples/01_platformer/src/systems.rs` |
| State stack, scene spawn, `SceneEntity` cleanup (`D-046`) | `tungsten/state.rs`, `core/assets/scene.rs`, `tungsten/asset_loader.rs`, `examples/03_scene_state/src/states.rs` |
| Tweens (`D-054`–`D-056`) | `core/tween.rs`, `tungsten/tweens.rs`, `core/assets/scene.rs` |
| Particles (`D-049`–`D-051`) | `core/assets/particle.rs`, `core/rng.rs`, `core/components.rs`, `tungsten/particles.rs`, `examples/01_platformer/assets/particles/` |
| Audio mixer, decode (`D-027`, `D-028`, `D-034`) | `tungsten/audio.rs`, `core/assets/audio.rs`, `crates/tungsten-core/tests/audio_decode.rs` |
| Telemetry, perf logging (`D-038`, `D-041`) | `tungsten/telemetry.rs`, `docs/perf/profiling-workflow.md`, `scripts/perf-capture.sh`, `scripts/test-perf-capture.sh` |
| HUD rows/toggle (`D-044`) | `tungsten/debug_hud.rs`, `tungsten/telemetry.rs` |

## Assets

| Task | Open |
| --- | --- |
| Manifest load, registry, IDs, composition (`D-009`, `D-017`, `D-035`, `D-052`) | `core/assets/manifest.rs`, `core/assets/registry.rs`, `core/assets/mod.rs`, `tungsten/asset_loader.rs` |
| Load path, GPU upload bridge | `tungsten/asset_loader.rs` |
| Hot reload, file watch (`D-031`, `D-053`) | `tungsten/hot_reload.rs`, `tungsten/asset_loader.rs` |
| Sprite atlases (`D-048`) | `core/assets/atlas.rs`, `tungsten/asset_loader.rs` |
| Tilemaps, tile collision (`D-032`, `D-033`) | `core/assets/tilemap.rs`, `tungsten/tilemap_extract.rs`, `core/physics/step.rs` |

## Physics

| Task | Open |
| --- | --- |
| Contacts, resolution, broadphase, sleeping (`D-033`, `D-062`–`D-067`) | `core/physics/step.rs`, `core/physics/collision.rs`, `core/physics/broadphase.rs` |
| Benchmarks, tunneling, determinism | `crates/tungsten-core/benches/physics_bench.rs`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten-core/tests/physics_containment.rs`, `crates/tungsten-core/tests/physics_determinism.rs` |

## Rendering

Read `crates/tungsten-render/AGENTS.md` before editing the render crate.

| Task | Open |
| --- | --- |
| Renderer, pools, draw, GPU timings, surface acquire | `render/lib.rs`, `render/renderer.rs`, `render/surface_acquire.rs` |
| Render components, default sprite extract (`D-042`) | `core/components.rs`, `tungsten/sprite_extract.rs` |
| Materials, post-stack (`D-058`) | `core/assets/material.rs`, `core/post.rs`, `render/material.rs`, `render/post/`, `render/shaders/stock/` |
| SMAA (`D-059`) | `tungsten/post_aa.rs`, `render/post/smaa.rs`, `render/post/smaa_luts.rs`, `render/targets.rs`, `render/passes/order.rs` |
| Bloom (`D-060`) | `core/post.rs`, `render/post/bloom.rs`, `render/targets.rs` |
| Lighting (`D-061`) | `core/components.rs`, `core/lighting.rs`, `render/lighting.rs`, `render/lit_sprite.rs`, `assets/shaders/lit_sprite.wgsl`, `tungsten/light_extract.rs` |
| Debug overlays, screenshots (`D-047`) | `core/debug_draw.rs`, `core/inspect.rs`, `render/debug_line.rs`, `render/screenshot.rs`, `render/image_diff.rs`, `tungsten/physics_debug.rs`, `tungsten/systems_overlay.rs`, `tungsten/inspector.rs` |

## Examples

Run with `cargo run -p example-NN-name`.

| Task | Open |
| --- | --- |
| Platformer | `examples/01_platformer/src/main.rs`, `examples/01_platformer/src/setup.rs`, `examples/01_platformer/src/systems.rs`, `examples/01_platformer/src/extract.rs`, `examples/01_platformer/src/state.rs` |
| Sprite stress, perf scenes | `examples/02_sprite_stress/src/main.rs`, `examples/02_sprite_stress/src/ecs_high_load.rs`, `examples/02_sprite_stress/src/physics_stress.rs` |
| Scene/state lifecycle | `examples/03_scene_state/src/main.rs`, `examples/03_scene_state/src/states.rs` |
| Shader/material/post fixtures | `examples/04_shader_playground/src/main.rs` |
| Review findings and follow-ups | `docs/repo-review-2026-09-25.md` |
| Repository QA and quick checks | `scripts/check-repo.py`, `scripts/test-check-repo.py`, `justfile`, `docs/agent-setup.md` |
| New dependency or design change | `docs/DECISION_INDEX.md`, then `DECISIONS.md` by ID |

## Usually skip

`CHANGELOG.md` unless releasing; binary assets; large example tilemaps unless the bug is tilemap-specific. Never open `docs/plans/archive/`.
