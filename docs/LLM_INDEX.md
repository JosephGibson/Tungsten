# LLM Navigation Index

Use this as the first repo map. Keep scope tight and prefer opening one task row over broad repo search.

## Cheap Reads

- Rules and commands: [`AGENTS.md`](../AGENTS.md)
- Quick rationale lookup: [`docs/DECISION_INDEX.md`](DECISION_INDEX.md)
- Architecture only when needed: [`DESIGN.md`](../DESIGN.md)
- Full rationale only when needed: [`DECISIONS.md`](../DECISIONS.md)

## Subsystem Map

| Area | Start Here |
| --- | --- |
| ECS (`World`, entities, components, resources) | [`crates/tungsten-core/src/ecs/`](../crates/tungsten-core/src/ecs/), [`crates/tungsten-core/src/lib.rs`](../crates/tungsten-core/src/lib.rs) |
| Event queue (`EventQueue<T>`, frame flush) | [`crates/tungsten-core/src/ecs/event_queue.rs`](../crates/tungsten-core/src/ecs/event_queue.rs) |
| Render components (`Transform`, `Sprite`, `Visibility`, `Tag`) + default sprite extract | [`crates/tungsten-core/src/components.rs`](../crates/tungsten-core/src/components.rs), [`crates/tungsten/src/sprite_extract.rs`](../crates/tungsten/src/sprite_extract.rs) |
| Camera module | [`crates/tungsten-core/src/camera.rs`](../crates/tungsten-core/src/camera.rs), [`crates/tungsten/src/camera.rs`](../crates/tungsten/src/camera.rs) |
| Display state/config + runtime apply boundary | [`crates/tungsten-core/src/display.rs`](../crates/tungsten-core/src/display.rs), [`crates/tungsten-core/src/config.rs`](../crates/tungsten-core/src/config.rs), [`crates/tungsten/src/display.rs`](../crates/tungsten/src/display.rs), [`tungsten.json`](../tungsten.json) |
| Asset manifest, registry, IDs | [`crates/tungsten-core/src/assets/manifest.rs`](../crates/tungsten-core/src/assets/manifest.rs), [`crates/tungsten-core/src/assets/registry.rs`](../crates/tungsten-core/src/assets/registry.rs), [`crates/tungsten-core/src/assets/mod.rs`](../crates/tungsten-core/src/assets/mod.rs) |
| App / `winit` loop, smoke frames | [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`crates/tungsten/src/lib.rs`](../crates/tungsten/src/lib.rs) |
| Runtime telemetry | [`crates/tungsten/src/telemetry.rs`](../crates/tungsten/src/telemetry.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs) |
| Runtime HUD (M18) | [`crates/tungsten/src/debug_hud.rs`](../crates/tungsten/src/debug_hud.rs), [`crates/tungsten/src/telemetry.rs`](../crates/tungsten/src/telemetry.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs) |
| Load path, GPU upload bridge | [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs) |
| Hot reload | [`crates/tungsten/src/hot_reload.rs`](../crates/tungsten/src/hot_reload.rs) |
| Input action map (M19) | [`crates/tungsten-core/src/input/`](../crates/tungsten-core/src/input/), [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs), [`input.json`](../input.json) |
| Scene/state stack (M20) | [`crates/tungsten/src/state.rs`](../crates/tungsten/src/state.rs), [`crates/tungsten-core/src/assets/scene.rs`](../crates/tungsten-core/src/assets/scene.rs), [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs), [`examples/03_scene_state/`](../examples/03_scene_state/) |
| Debug tooling (M21) | [`crates/tungsten-core/src/debug_draw.rs`](../crates/tungsten-core/src/debug_draw.rs), [`crates/tungsten-core/src/inspect.rs`](../crates/tungsten-core/src/inspect.rs), [`crates/tungsten-render/src/debug_line.rs`](../crates/tungsten-render/src/debug_line.rs), [`crates/tungsten-render/src/screenshot.rs`](../crates/tungsten-render/src/screenshot.rs), [`crates/tungsten-render/src/image_diff.rs`](../crates/tungsten-render/src/image_diff.rs), [`crates/tungsten/src/physics_debug.rs`](../crates/tungsten/src/physics_debug.rs), [`crates/tungsten/src/systems_overlay.rs`](../crates/tungsten/src/systems_overlay.rs), [`crates/tungsten/src/inspector.rs`](../crates/tungsten/src/inspector.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs) |
| `wgpu` renderer, pools, draw, GPU timings | [`crates/tungsten-render/src/lib.rs`](../crates/tungsten-render/src/lib.rs), [`crates/tungsten-render/src/renderer.rs`](../crates/tungsten-render/src/renderer.rs) |
| Tilemaps (core data + umbrella extract) | [`crates/tungsten-core/src/assets/tilemap.rs`](../crates/tungsten-core/src/assets/tilemap.rs), [`crates/tungsten/src/tilemap_extract.rs`](../crates/tungsten/src/tilemap_extract.rs) |
| 2D physics (M11) | [`crates/tungsten-core/src/physics/`](../crates/tungsten-core/src/physics/) |
| Examples (by feature) | [`examples/`](../examples/) — `cargo run -p example-NN-name` |
| Perf workflow | [`docs/perf/profiling-workflow.md`](../docs/perf/profiling-workflow.md), [`scripts/perf-capture.sh`](../scripts/perf-capture.sh), [`scripts/test-perf-capture.sh`](../scripts/test-perf-capture.sh) |

## Task Map

| If you need to… | Open these first |
| --- | --- |
| Fix an ECS storage/query bug | [`crates/tungsten-core/src/ecs/world.rs`](../crates/tungsten-core/src/ecs/world.rs), [`crates/tungsten-core/src/ecs/storage.rs`](../crates/tungsten-core/src/ecs/storage.rs), [`crates/tungsten-core/src/ecs/archetype.rs`](../crates/tungsten-core/src/ecs/archetype.rs) |
| Change deferred spawn/despawn behavior | [`crates/tungsten-core/src/ecs/command_buffer.rs`](../crates/tungsten-core/src/ecs/command_buffer.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-039` |
| Change event lifetime or flush behavior | [`crates/tungsten-core/src/ecs/event_queue.rs`](../crates/tungsten-core/src/ecs/event_queue.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-040` |
| Fix frame order, smoke-frame exit, or `winit` loop behavior | [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`crates/tungsten/src/lib.rs`](../crates/tungsten/src/lib.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-018`, `D-040`, `D-043` |
| Fix config parsing or env overrides | [`crates/tungsten-core/src/config.rs`](../crates/tungsten-core/src/config.rs), [`crates/tungsten-core/src/display.rs`](../crates/tungsten-core/src/display.rs), [`tungsten.json`](../tungsten.json) |
| Fix runtime display apply, fullscreen, vsync, or frame cap | [`crates/tungsten/src/display.rs`](../crates/tungsten/src/display.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`crates/tungsten-core/src/display.rs`](../crates/tungsten-core/src/display.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-043` |
| Fix manifest loading, registry lookup, or asset ID rules | [`crates/tungsten-core/src/assets/manifest.rs`](../crates/tungsten-core/src/assets/manifest.rs), [`crates/tungsten-core/src/assets/registry.rs`](../crates/tungsten-core/src/assets/registry.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-009`, `D-017`, `D-035` |
| Fix hot reload or file-watch behavior | [`crates/tungsten/src/hot_reload.rs`](../crates/tungsten/src/hot_reload.rs), [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-031` |
| Fix action lookups, mouse/scroll dispatch, input.json persistence, or rebind hot reload | [`crates/tungsten-core/src/input/action_map.rs`](../crates/tungsten-core/src/input/action_map.rs), [`crates/tungsten-core/src/input/key_serde.rs`](../crates/tungsten-core/src/input/key_serde.rs), [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`crates/tungsten/src/debug_hud.rs`](../crates/tungsten/src/debug_hud.rs), [`crates/tungsten/src/display.rs`](../crates/tungsten/src/display.rs), [`input.json`](../input.json), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-045` |
| Fix camera follow, zoom, or bounds behavior | [`crates/tungsten-core/src/camera.rs`](../crates/tungsten-core/src/camera.rs), [`crates/tungsten/src/camera.rs`](../crates/tungsten/src/camera.rs), [`examples/01_platformer/src/setup.rs`](../examples/01_platformer/src/setup.rs), [`examples/01_platformer/src/systems.rs`](../examples/01_platformer/src/systems.rs) |
| Fix default sprite extraction or gameplay-side sprite components | [`crates/tungsten-core/src/components.rs`](../crates/tungsten-core/src/components.rs), [`crates/tungsten/src/sprite_extract.rs`](../crates/tungsten/src/sprite_extract.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-042` |
| Fix tilemap extraction or tilemap collision | [`crates/tungsten-core/src/assets/tilemap.rs`](../crates/tungsten-core/src/assets/tilemap.rs), [`crates/tungsten/src/tilemap_extract.rs`](../crates/tungsten/src/tilemap_extract.rs), [`crates/tungsten-core/src/physics/step.rs`](../crates/tungsten-core/src/physics/step.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-032`, `D-033` |
| Fix physics contacts, collision resolution, or broad-phase behavior | [`crates/tungsten-core/src/physics/step.rs`](../crates/tungsten-core/src/physics/step.rs), [`crates/tungsten-core/src/physics/collision.rs`](../crates/tungsten-core/src/physics/collision.rs), [`crates/tungsten-core/src/physics/broadphase.rs`](../crates/tungsten-core/src/physics/broadphase.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-033` |
| Fix telemetry or perf logging output | [`crates/tungsten/src/telemetry.rs`](../crates/tungsten/src/telemetry.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`docs/perf/profiling-workflow.md`](../docs/perf/profiling-workflow.md), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-038`, `D-041` |
| Fix HUD rows, toggle, or composition | [`crates/tungsten/src/debug_hud.rs`](../crates/tungsten/src/debug_hud.rs), [`crates/tungsten/src/telemetry.rs`](../crates/tungsten/src/telemetry.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-044` |
| Fix a debug overlay or screenshot check | [`crates/tungsten-core/src/debug_draw.rs`](../crates/tungsten-core/src/debug_draw.rs), [`crates/tungsten-core/src/inspect.rs`](../crates/tungsten-core/src/inspect.rs), [`crates/tungsten-render/src/debug_line.rs`](../crates/tungsten-render/src/debug_line.rs), [`crates/tungsten-render/src/screenshot.rs`](../crates/tungsten-render/src/screenshot.rs), [`crates/tungsten-render/src/image_diff.rs`](../crates/tungsten-render/src/image_diff.rs), [`crates/tungsten/src/physics_debug.rs`](../crates/tungsten/src/physics_debug.rs), [`crates/tungsten/src/systems_overlay.rs`](../crates/tungsten/src/systems_overlay.rs), [`crates/tungsten/src/inspector.rs`](../crates/tungsten/src/inspector.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-047` |
| Work inside the platformer example | [`examples/01_platformer/src/main.rs`](../examples/01_platformer/src/main.rs), [`examples/01_platformer/src/setup.rs`](../examples/01_platformer/src/setup.rs), [`examples/01_platformer/src/systems.rs`](../examples/01_platformer/src/systems.rs), [`examples/01_platformer/src/extract.rs`](../examples/01_platformer/src/extract.rs), [`examples/01_platformer/src/state.rs`](../examples/01_platformer/src/state.rs) |
| Work inside the sprite stress example | [`examples/02_sprite_stress/src/main.rs`](../examples/02_sprite_stress/src/main.rs), [`crates/tungsten/src/telemetry.rs`](../crates/tungsten/src/telemetry.rs) |
| Change state transitions, scene spawn, or SceneEntity cleanup | [`crates/tungsten/src/state.rs`](../crates/tungsten/src/state.rs), [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs), [`crates/tungsten-core/src/assets/scene.rs`](../crates/tungsten-core/src/assets/scene.rs), [`examples/03_scene_state/src/states.rs`](../examples/03_scene_state/src/states.rs), [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) for `D-046` |
| Decide whether a new dependency or design change fits project rules | [`docs/DECISION_INDEX.md`](DECISION_INDEX.md), then [`DECISIONS.md`](../DECISIONS.md) via `D-0xx` lookup |

## Usually Skip Unless Needed

- `docs/plans/archive/` — completed or abandoned plans
- `CHANGELOG.md` — release history, not day-to-day behavior
- binary assets under `assets/` and `examples/*/assets/`
- large example-local tilemaps unless the bug is tilemap-specific

Core/render seam and frame-order invariants: see [`docs/DECISION_INDEX.md`](DECISION_INDEX.md) and then `DECISIONS.md` (`D-007`, `D-016`, `D-018`, `D-039`, `D-040`, `D-043`).
