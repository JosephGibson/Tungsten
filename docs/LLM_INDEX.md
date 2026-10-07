# LLM Navigation Index

Task → file map. Open it before a broad search; read only the matching row's files. Prefixes: `core/` = `crates/tungsten-core/src/`, `render/` = `crates/tungsten-render/src/`, `tungsten/` = `crates/tungsten/src/`; other paths start at the repo root. `tungsten/app.rs` is the frame-loop hub. `D-NNN` IDs resolve through [DECISION_INDEX.md](DECISION_INDEX.md); [the documentation map](README.md) identifies canonical guides. Unit tests usually live in `crates/<crate>/src/tests/`; check the module's `#[path]` declaration.

## Runtime and ECS

| Task | Open |
| --- | --- |
| ECS storage/query bug | `core/ecs/world.rs`, `core/ecs/storage.rs`, `core/ecs/archetype.rs`, `core/lib.rs` |
| Deferred spawn/despawn (`D-039`) | `core/ecs/command_buffer.rs` |
| Event lifetime/flush (`D-040`) | `core/ecs/event_queue.rs`, `core/ecs/world.rs` |
| Frame order, smoke-frame exit, winit loop (`D-018`, `D-043`) | `tungsten/app.rs`, `tungsten/lib.rs` |
| Schedule, stages, plugins, `DefaultPlugins`, engine system names (`D-128`, `D-133`) | `core/schedule.rs`, `core/physics/plugin.rs`, `tungsten/plugins.rs`, `tungsten/app.rs`, `tungsten/tests/plugins.rs` |
| Game clock, `Time`, `Timer`, `DeltaTime`, the fixed step, interpolation (`D-088`, `D-129`, `D-134`, `D-137`) | `core/time.rs`, `core/tests/time.rs`, `core/physics/plugin.rs`, `core/ecs/event_queue.rs`, `tungsten/app.rs`, `tungsten/tests/fixed_step.rs`, `tungsten/tests/testing.rs` |
| Tuple queries, `With`/`Without`, bundles, `spawn_with`, `RigidBodyBundle` (`D-130`, `D-135`) | `core/ecs/query.rs`, `core/ecs/bundle.rs`, `core/ecs/world.rs`, `core/tests/ecs/query.rs` |
| Headless test harness, `Harness`, `FrameDraw`, `UiHarness` (`D-110`, `D-125`) | `tungsten/testing.rs`, `tungsten/testing/ui.rs`, `tungsten/app.rs`, `tungsten/tests/testing.rs` |
| Config parsing, env overrides | `core/config.rs`, `core/display.rs`, `tungsten.json` |
| Logs, user folder, crash reports, `TUNGSTEN_USER_DIR`, `TUNGSTEN_TEST_PANIC` (`D-119`) | `tungsten/logging.rs`, `tungsten/user_dir.rs`, `tungsten/crash.rs`, `tungsten/app.rs`, `tools/crash-probe/` |
| Display apply, fullscreen, vsync, frame cap (`D-043`) | `tungsten/display.rs`, `core/display.rs` |
| Input actions, `input.json`, rebind reload (`D-045`) | `core/input/action_map.rs`, `core/input/key_serde.rs`, `tungsten/asset_loader/reload.rs`, `tungsten/debug_hud.rs`, `tungsten/display.rs`, `input.json` |
| Camera follow/zoom/bounds/shake (`D-073`) | `core/camera.rs`, `tungsten/camera.rs`, `examples/01_platformer/src/setup.rs`, `examples/01_platformer/src/systems.rs` |
| State stack, scene spawn, `SceneEntity` cleanup, screen transitions (`D-046`, `D-093`) | `tungsten/state.rs`, `tungsten/transition.rs`, `core/assets/scene.rs`, `tungsten/asset_loader/scene.rs`, `examples/03_scene_state/src/states.rs` |
| Tweens (`D-054`–`D-056`) | `core/tween.rs`, `tungsten/tweens.rs`, `core/assets/scene.rs` |
| Parallax, camera shake trauma, squash/stretch (`D-073`) | `core/components.rs`, `core/camera.rs`, `tungsten/game_feel.rs`, `tungsten/sprite_extract.rs` |
| Particles, mesh particles (`D-049`–`D-051`, `D-093`) | `core/assets/particle.rs`, `core/rng.rs`, `core/components.rs`, `tungsten/particles.rs`, `render/mesh_particle.rs`, `examples/01_platformer/assets/particles/` |
| Audio mixer, decode (`D-027`, `D-028`, `D-034`) | `tungsten/audio.rs`, `core/assets/audio.rs`, `crates/tungsten-core/tests/audio_decode.rs` |
| Telemetry, perf lines (`D-038`, `D-041`) | `tungsten/telemetry.rs`, `tungsten/app.rs`, `docs/perf/profiling-workflow.md` |
| Perf runner, capacity search, compare reports (`D-078`) | `scripts/bench.py`, `scripts/bench_report.py`, `scripts/test-bench.py` |
| Criterion micro-benchmarks | `crates/*/benches/`, `docs/perf/profiling-workflow.md` |
| HUD rows/toggle, the engine font (`D-044`, `D-123`) | `tungsten/debug_hud.rs`, `tungsten/telemetry.rs`, `tungsten/engine_font.rs` |

## Assets

| Task | Open |
| --- | --- |
| Manifest load, registry, IDs, composition (`D-009`, `D-017`, `D-035`, `D-052`, `D-113`) | `core/assets/manifest.rs`, `core/assets/registry.rs`, `core/assets/mod.rs`, `tungsten/asset_loader/mod.rs` |
| Load path, GPU upload bridge | `tungsten/asset_loader/` |
| Hot reload, file watch (`D-031`, `D-053`) | `tungsten/hot_reload.rs`, `tungsten/asset_loader/reload.rs` |
| Sprite atlases (`D-048`) | `core/assets/atlas.rs`, `tungsten/asset_loader/atlas.rs` |
| Tilemaps, tile collision (`D-032`, `D-033`) | `core/assets/tilemap.rs`, `tungsten/tilemap_extract.rs`, `core/physics/step/gather.rs` |

## Physics

| Task | Open |
| --- | --- |
| Contacts, resolution, broadphase, pair repair, sleeping, arrival pass, step bound (`D-033`, `D-062`–`D-067`, `D-075`, `D-076`, `D-080`–`D-082`, `D-092`, `D-094`) | `core/physics/step/`, `core/physics/collision.rs`, `core/physics/broadphase.rs` |
| Benchmarks, tunneling, determinism | `crates/tungsten-core/benches/physics_bench.rs`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten-core/tests/physics_containment.rs`, `crates/tungsten-core/tests/physics_determinism.rs`; after `core/physics/step/`, `core/physics/collision.rs` or `core/physics/broadphase.rs` changes run `just physics-release` (determinism and containment are ignored in debug, so `just check` skips them) |

## Rendering

Read `crates/tungsten-render/AGENTS.md` before editing the render crate.

| Task | Open |
| --- | --- |
| Renderer, pools, draw, GPU timings, surface acquire | `render/lib.rs`, `render/renderer.rs`, `render/timing.rs`, `render/surface_acquire.rs` |
| Pass order, direct present path (`D-087`) | `render/passes/order.rs`, `render/renderer.rs`, `render/screenshot.rs` |
| Text engine and GPU halves, layout cache, retained nodes, font families and fallback (`D-085`, `D-115`–`D-117`, `D-123`) | `render/text.rs`, `render/text/engine.rs`, `render/text/nodes.rs`, `render/text/fonts.rs`, `render/text/gpu.rs`, `tungsten/asset_loader/mod.rs`, `tungsten/engine_font.rs` |
| Neutral text types, `TextMeasure`, `TextNodeStore` (`D-117`, `D-125`) | `core/text.rs` |
| UI tree, style, layout, focus, hit testing (`D-124`, `D-125`) | `core/ui/`, `core/text.rs`, `tungsten/testing/ui.rs`, `core/tests/ui/` |
| Render components, default sprite extract (`D-042`, `D-086`, `D-113`, `D-114`) | `core/components.rs`, `tungsten/sprite_extract.rs` |
| Materials, post-stack (`D-058`) | `core/assets/material.rs`, `core/post.rs`, `render/material.rs`, `render/post/`, `render/shaders/stock/` |
| SMAA (`D-059`) | `tungsten/post_aa.rs`, `render/post/smaa.rs`, `render/post/smaa_luts.rs`, `render/targets.rs`, `render/passes/order.rs` |
| Bloom (`D-060`) | `core/post.rs`, `render/post/bloom.rs`, `render/targets.rs` |
| Lighting (`D-061`) | `core/components.rs`, `core/lighting.rs`, `render/lighting.rs`, `render/lit_sprite.rs`, `assets/shaders/lit_sprite.wgsl`, `tungsten/light_extract.rs` |
| Debug overlays, screenshots (`D-047`) | `core/debug_draw.rs`, `core/inspect.rs`, `render/debug_line.rs`, `render/screenshot.rs`, `render/image_diff.rs`, `tungsten/physics_debug.rs`, `tungsten/systems_overlay.rs`, `tungsten/inspector.rs` |
| Pixel tests (`just visual`) | `examples/02_bench/tests/visual_regression.rs`, `examples/04_shader_playground/tests/post_regression.rs`, `examples/03_scene_state/tests/transition_regression.rs` |

## Examples

Run with `cargo run -p example-NN-name`.

| Task | Open |
| --- | --- |
| Platformer | `examples/01_platformer/src/main.rs`, `examples/01_platformer/src/setup.rs`, `examples/01_platformer/src/systems.rs`, `examples/01_platformer/src/extract.rs`, `examples/01_platformer/src/state.rs`, `examples/01_platformer/src/gameplay.rs`, `examples/01_platformer/src/fireball.rs`, `examples/01_platformer/src/burning.rs` |
| Platformer art, level and layout regeneration | `examples/01_platformer/tools/README.md`, `examples/01_platformer/tools/generate.py`, `examples/01_platformer/tools/level.json`, `examples/01_platformer/src/level_layout.rs` |
| Benchmark harness, knobs, presets | `examples/02_bench/src/main.rs`, `examples/02_bench/src/knobs.rs`, `docs/perf/benchmarks.md` |
| One benchmark's workload | `examples/02_bench/src/<bench>.rs` |
| Scene/state lifecycle | `examples/03_scene_state/src/main.rs`, `examples/03_scene_state/src/states.rs` |
| Shader/material/post fixtures | `examples/04_shader_playground/src/main.rs` |
| The game template, a game's layout from its own folder (`D-123`) | `templates/basic/src/game.rs`, `templates/basic/src/states.rs`, `templates/basic/tests/game.rs`, `templates/basic/AGENTS.md`, `scripts/smoke-examples.sh`, `scripts/template-check.sh`, `docs/getting-started.md` |
| GPU smoke run, fixture matrices (`just smoke`) | `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh` |
| Documentation maintenance | `docs/README.md`, `docs/plans/README.md` |
| Open findings, unchecked platforms and follow-ups | `docs/known-issues.md` |
| Repository QA and quick checks | `scripts/check-repo.py`, `scripts/test-check-repo.py`, `justfile`, `docs/agent-setup.md` |
| Agent instruction budgets and links (`just ctx`) | `scripts/check-agent-context.py` |
| Public-API snapshots (`just api`, `D-107`); breaks go in the W4 ledger | `scripts/public-api.sh`, `api/`, `docs/plans/1.0/w04-api-freeze.md` |
| Writing or running a 1.0 milestone plan | `.claude/skills/tungsten-milestone/SKILL.md`, `docs/plans/1.0/workflow.md` |
| Road to 1.0 status, next prompt, roadmap page, stop stages and session recommendations (`D-109`, `D-118`) | `.claude/skills/tungsten-next/SKILL.md`, `.claude/skills/tungsten-next/prompts.md`, `docs/plans/1.0/roadmap.json`, `scripts/roadmap.py`, `scripts/test-roadmap.py`, `.claude/skills/tungsten-next/page.html` |
| Release preparation, checks and command hand-off, publication/recovery, debug archives, license notices and crash symbolization (`D-071`, `D-074`, `D-079`, `D-120`, `D-122`) | `.github/workflows/release.yml`, `scripts/release.py`, `scripts/test-release.py`, `licenses/`, `scripts/crash-report.py`, `scripts/test-crash-report.py`, `tools/launcher/src/main.rs`, `.claude/skills/tungsten-finalize/SKILL.md`, `.claude/skills/tungsten-release/SKILL.md`, `docs/releases.md`, `scripts/release-preflight.py`, `scripts/test-release-preflight.py` |
| CI jobs, the Windows test job (`D-070`, `D-121`) | `.github/workflows/ci.yml` |
| New dependency or design change | `docs/DECISION_INDEX.md`, then `DECISIONS.md` by ID; to write an entry, `.claude/skills/tungsten-decision/SKILL.md` |
| Committing plan work (`D-097`, `D-098`) | `docs/plans/README.md` |

## Usually skip

`CHANGELOG.md` unless releasing; binary assets; large example tilemaps unless the bug is tilemap-specific. Never open `docs/plans/archive/`.
