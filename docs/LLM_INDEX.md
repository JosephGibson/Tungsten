# LLM Navigation Index

Task → file map: read only the matching row's files. Prefixes: `core/` = `crates/tungsten-core/src/`, `render/` = `crates/tungsten-render/src/`, `tungsten/` = `crates/tungsten/src/`; other paths start at the repo root. `tungsten/app.rs` is the frame-loop hub. `D-NNN` IDs resolve through [DECISION_INDEX.md](DECISION_INDEX.md); [the documentation map](README.md) names canonical guides. Unit tests usually live in `crates/<crate>/src/tests/` (the module's `#[path]`).

## Runtime and ECS

| Task | Open |
| --- | --- |
| ECS storage/query bug | `core/ecs/world.rs`, `core/ecs/storage.rs`, `core/ecs/archetype.rs`, `core/lib.rs` |
| Deferred spawn/despawn (`D-039`) | `core/ecs/command_buffer.rs` |
| Event flush, holds, `EventCursor` (`D-040`, `D-139`, `D-170`) | `core/ecs/event_queue.rs`, `core/ecs/world.rs` |
| Frame order, smoke-frame exit, winit loop (`D-018`, `D-043`) | `tungsten/app.rs`, `tungsten/lib.rs` |
| Schedule, stages, plugins, `DefaultPlugins`, engine system names (`D-128`, `D-133`) | `core/schedule.rs`, `core/physics/plugin.rs`, `tungsten/plugins.rs`, `tungsten/app.rs`, `tungsten/tests/plugins.rs` |
| Kit by domain: phases, tags, stats, cues, motion, recipes (`D-155`, `D-172`–`D-177`) | `crates/tungsten-kit/src/`, `tungsten/tests/kit.rs`, `tungsten/tests/kit_recipes/`, `crates/tungsten/tests/fixtures/kit/` |
| Game clock, `Time`, `Timer`, `DeltaTime`, fixed step, interpolation (`D-088`, `D-129`, `D-134`, `D-137`, `D-139`, `D-146`) | `core/time.rs`, `core/tests/time.rs`, `core/physics/plugin.rs`, `tungsten/app.rs`, `tungsten/tests/fixed_step.rs`, `tungsten/tests/testing.rs`, `examples/01_platformer/src/tests/regime.rs` |
| Tuple queries, `With`/`Without`, bundles, `spawn_with`, `RigidBodyBundle` (`D-130`, `D-135`) | `core/ecs/query.rs`, `core/ecs/bundle.rs`, `core/ecs/world.rs`, `core/tests/ecs/query.rs` |
| Headless harness, `Harness`, `FrameDraw`, `UiHarness` (`D-110`, `D-125`) | `tungsten/testing.rs`, `tungsten/testing/ui.rs`, `tungsten/app.rs`, `tungsten/tests/testing.rs` |
| Config, env overrides, settings (`D-159`) | `core/config.rs`, `core/settings.rs`, `core/display.rs`, `tungsten/settings.rs`, `tungsten.json` |
| Logs, user folder, saves, crashes and target triple, `TUNGSTEN_USER_DIR`, `TUNGSTEN_TEST_PANIC` (`D-119`, `D-160`) | `tungsten/logging.rs`, `tungsten/user_dir.rs`, `core/save.rs`, `core/persist.rs`, `tungsten/crash.rs`, `tungsten/app.rs`, `crates/tungsten/build.rs`, `tools/crash-probe/` |
| Display apply, fullscreen, vsync, frame cap (`D-043`) | `tungsten/display.rs`, `core/display.rs` |
| Input actions, `input.json`, rebind reload (`D-045`, `D-159`) | `core/input/action_map.rs`, `core/input/key_serde.rs`, `tungsten/asset_loader/reload.rs`, `tungsten/debug_hud.rs`, `tungsten/display.rs`, `input.json` |
| Input state, raw stream, gamepads, focus loss, modifiers, pointer leave, `ScaleFactor`, winit bridge (`D-144`) | `core/input.rs`, `core/display.rs`, `tungsten/input_bridge.rs`, `tungsten/app.rs`, `core/tests/input.rs`, `tungsten/tests/input_bridge.rs` |
| Camera follow/zoom/bounds/shake (`D-073`) | `core/camera.rs`, `tungsten/camera.rs`, `examples/01_platformer/src/setup.rs`, `examples/01_platformer/src/systems.rs` |
| State stack, owned UI roots, scenes, components, `SceneEntity`, transitions (`D-046`, `D-093`, `D-153`) | `tungsten/state.rs`, `tungsten/transition.rs`, `core/assets/scene.rs`, `core/component_registry.rs`, `tungsten/asset_loader/scene.rs`, `examples/03_scene_state/src/states.rs` |
| Tweens (`D-054`–`D-056`) | `core/tween.rs`, `tungsten/tweens.rs`, `core/assets/scene.rs` |
| Parallax, camera shake trauma, squash/stretch (`D-073`) | `core/components.rs`, `core/camera.rs`, `tungsten/game_feel.rs`, `tungsten/sprite_extract.rs` |
| Particles, mesh particles (`D-049`–`D-051`, `D-093`) | `core/assets/particle.rs`, `core/rng.rs`, `core/components.rs`, `tungsten/particles.rs`, `render/mesh_particle.rs`, `examples/01_platformer/assets/particles/` |
| Audio mixer, decode (`D-027`, `D-028`, `D-034`) | `tungsten/audio.rs`, `core/assets/audio.rs`, `crates/tungsten-core/tests/audio_decode.rs` |
| Telemetry, perf lines (`D-038`, `D-041`) | `tungsten/telemetry.rs`, `tungsten/app.rs`, `docs/perf/profiling-workflow.md` |
| Perf runner, capacity search, compare reports (`D-078`) | `scripts/bench.py`, `scripts/bench_report.py`, `scripts/test-bench.py` |
| Criterion micro-benchmarks | `crates/*/benches/`, `docs/perf/profiling-workflow.md` |
| Debug views: HUD, overlay, inspectors, console; engine font (`D-044`, `D-123`, `D-165`) | `tungsten/debug_views.rs`, `tungsten/debug_hud.rs`, `tungsten/telemetry.rs`, `tungsten/engine_font.rs` |

## Assets

| Task | Open |
| --- | --- |
| Manifest load, data entries, load path, GPU upload, registry, IDs, composition (`D-009`, `D-017`, `D-035`, `D-052`, `D-113`, `D-171`) | `core/assets/manifest.rs`, `core/assets/data.rs`, `core/assets/registry.rs`, `core/assets/mod.rs`, `tungsten/asset_loader/` |
| Hot reload, file watch (`D-031`, `D-053`) | `tungsten/hot_reload.rs`, `tungsten/asset_loader/reload.rs` |
| Sprite atlases (`D-048`) | `core/assets/atlas.rs`, `tungsten/asset_loader/atlas.rs` |
| Tilemaps, tile collision (`D-032`, `D-033`) | `core/assets/tilemap.rs`, `tungsten/tilemap_extract.rs`, `core/physics/step/gather.rs` |
| Prefabs, spawn inheritance, Tiled objects, tilemap builder (`D-157`, `D-170`) | `core/assets/prefab.rs`, `core/assets/tilemap.rs`, `tungsten/asset_loader/prefab.rs` |
| UI theme asset, nine-slice insets (`D-148`) | `core/ui/theme_file.rs`, `core/assets/manifest.rs`, `tungsten/asset_loader/mod.rs`, `tungsten/asset_loader/reload.rs`, `tungsten/tests/asset_loader.rs` |

## Physics

| Task | Open |
| --- | --- |
| Contacts, sleep, kinematic bodies, sensors, queries (`D-033`, `D-062`–`D-067`, `D-075`, `D-076`, `D-080`–`D-082`, `D-092`, `D-094`, `D-154`, `D-156`) | `core/physics/step/`, `core/physics/collision.rs`, `core/physics/broadphase.rs`, `core/physics/query.rs` |
| Benchmarks, tunneling, determinism | `crates/tungsten-core/benches/physics_bench.rs`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten-core/tests/physics_containment.rs`, `crates/tungsten-core/tests/physics_determinism.rs`; [validation](validation.md#change-to-check-matrix) routes release-only physics checks |

## Rendering

Read `crates/tungsten-render/AGENTS.md` before editing the render crate.

| Task | Open |
| --- | --- |
| Renderer, pools, draw, GPU timings, surface acquire | `render/lib.rs`, `render/renderer.rs`, `render/timing.rs`, `render/surface_acquire.rs` |
| Pass order, direct present, late acquire (`D-087`, `D-145`) | `render/passes/order.rs`, `render/renderer.rs`, `render/surface_acquire.rs`, `render/screenshot.rs` |
| Text engine, layout cache, world labels, retained nodes, font families, fallback (`D-085`, `D-115`–`D-117`, `D-123`, `D-161`) | `render/text.rs`, `render/text/engine.rs`, `render/text/nodes.rs`, `render/text/fonts.rs`, `render/text/gpu.rs`, `tungsten/asset_loader/mod.rs`, `tungsten/engine_font.rs` |
| Neutral text types, world labels, `TextMeasure`, `TextNodeStore` (`D-117`, `D-125`) | `core/text.rs` |
| UI tree, layout, focus, hits, routing, tracks, prompts, capture, prewarm (`D-124`, `D-125`, `D-149`, `D-163`, `D-164`) | `core/ui/`, `tungsten/testing/ui.rs`, `core/tests/ui/`, `tungsten/tests/ui_route.rs` |
| UI paint list, overlay pipeline, text batches, layout stage, debug actions (`D-147`, `D-148`) | `core/ui/paint.rs`, `render/ui.rs`, `render/ui.wgsl`, `tungsten/app.rs`, `tungsten/ui_debug.rs`, `core/tests/ui/paint.rs`, `render/tests/ui.rs`, `render/tests/text_batches.rs` |
| Render components, default sprite extract, `Extracts` channels (`D-042`, `D-086`, `D-113`, `D-114`, `D-138`) | `core/components.rs`, `tungsten/sprite_extract.rs`, `tungsten/extract.rs` |
| Materials, post-stack (`D-058`) | `core/assets/material.rs`, `core/post.rs`, `render/material.rs`, `render/post/`, `render/shaders/stock/` |
| SMAA (`D-059`) | `tungsten/post_aa.rs`, `render/post/smaa.rs`, `render/post/smaa_luts.rs`, `render/targets.rs`, `render/passes/order.rs` |
| Bloom (`D-060`) | `core/post.rs`, `render/post/bloom.rs`, `render/targets.rs` |
| Lighting (`D-061`) | `core/components.rs`, `core/lighting.rs`, `render/lighting.rs`, `render/lit_sprite.rs`, `assets/shaders/lit_sprite.wgsl`, `tungsten/light_extract.rs` |
| Debug overlays, screenshots (`D-047`, `D-142`) | `core/debug_draw.rs`, `core/inspect.rs`, `render/debug_line.rs`, `render/screenshot.rs`, `render/image_diff.rs`, `tungsten/capture.rs`, `tungsten/physics_debug.rs`, `tungsten/systems_overlay.rs`, `tungsten/inspector.rs` |
| Pixel tests (`just visual`) | `examples/02_bench/tests/visual_regression.rs`, `examples/04_shader_playground/tests/post_regression.rs`, `examples/03_scene_state/tests/transition_regression.rs`, `examples/05_ui_lab/tests/ui_regression.rs` |

## Examples

Run with `cargo run -p example-NN-name`.

| Task | Open |
| --- | --- |
| Platformer | `examples/01_platformer/src/`: main, setup, systems, extract, state, gameplay, fireball, ice, burning, brick and death |
| Platformer art, level and layout regeneration | `examples/01_platformer/tools/README.md`, `examples/01_platformer/tools/generate.py`, `examples/01_platformer/tools/level.json`, `examples/01_platformer/src/level_layout.rs` |
| Benchmark harness, knobs, presets | `examples/02_bench/src/main.rs`, `examples/02_bench/src/knobs.rs`, `docs/perf/benchmarks.md` |
| One benchmark's workload | `examples/02_bench/src/<bench>.rs` |
| Scene/state lifecycle | `examples/03_scene_state/src/main.rs`, `examples/03_scene_state/src/states.rs` |
| Shader/material/post fixtures | `examples/04_shader_playground/src/main.rs` |
| UI lab: theme reload, screens | `examples/05_ui_lab/src/`, `examples/05_ui_lab/src/menus.rs`, `examples/05_ui_lab/tests/`, `examples/05_ui_lab/tools/README.md` |
| The game template, a game's own folder (`D-123`) | `templates/basic/src/game.rs`, `templates/basic/src/states.rs`, `templates/basic/tests/game.rs`, `templates/basic/AGENTS.md`, `scripts/smoke-examples.sh`, `scripts/template-check.sh`, `docs/getting-started.md` |
| GPU smoke run, fixture matrices (`just smoke`) | `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh` |
| Documentation maintenance | `docs/README.md`, `docs/plans/README.md` |
| Open findings, unchecked platforms and follow-ups | `docs/known-issues.md` |
| Validation selection (`just checks`), repository QA | `docs/validation.md`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `justfile`; client setup: `docs/agent-setup.md` |
| Agent instruction budgets and links (`just ctx`) | `scripts/check-agent-context.py` |
| Stale `target/` artifacts, a full disk (`just clean-target`) | `scripts/clean-target.py`, `scripts/test-clean-target.py` |
| Public-API snapshots (`just api`, `D-107`); breaks go in the W4 ledger | `scripts/public-api.sh`, `api/`, `docs/plans/1.0/w04-api-freeze.md` |
| Writing or running a 1.0 milestone plan | `.claude/skills/tungsten-milestone/SKILL.md`, `docs/plans/1.0/workflow.md` |
| Road to 1.0: status, next prompt, roadmap page, stop stages, session tiers (`D-109`, `D-118`, `D-150`–`D-152`) | `.claude/skills/tungsten-next/SKILL.md`, `.claude/skills/tungsten-next/prompts.md`, `docs/plans/1.0/roadmap.json`, `scripts/roadmap.py`, `scripts/test-roadmap.py`, `.claude/skills/tungsten-next/page.html` |
| Releases: checks, hand-off, recovery, debug archives, licenses, crash symbols (`D-071`, `D-074`, `D-079`, `D-120`, `D-122`) | `.github/workflows/release.yml`, `scripts/release.py`, `scripts/test-release.py`, `licenses/`, `scripts/crash-report.py`, `scripts/test-crash-report.py`, `tools/launcher/src/main.rs`, `.claude/skills/tungsten-finalize/SKILL.md`, `.claude/skills/tungsten-release/SKILL.md`, `docs/releases.md`, `scripts/release-preflight.py`, `scripts/test-release-preflight.py` |
| CI jobs, the Windows test job (`D-070`, `D-121`) | `.github/workflows/ci.yml` |
| New dependency or design change | `docs/DECISION_INDEX.md`, then `DECISIONS.md` by ID; to write an entry, `.claude/skills/tungsten-decision/SKILL.md` |

## Usually skip

`CHANGELOG.md` unless releasing; binary assets; large example tilemaps unless tilemap-specific. Never open `docs/plans/archive/`.
