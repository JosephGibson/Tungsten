# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.17.0` on branch `0.17`. Phase 3 M20 is shipped. The engine now pairs typed two-window event queues and deferred ECS command buffers with a shared camera module, a core-owned display state/config model with frame-boundary runtime apply, a runtime telemetry HUD rendered through the existing text pipeline, a workspace-root `input.json` action map (keyboard / mouse / wheel + hot reload + engine-owned HUD/display/exit controls), and a scene/state dispatcher that drives `MainMenu → Gameplay → Pause → Gameplay` flow with scene-owned entity auto-cleanup and data-driven `scene.json` spawning. Next milestone: `M21` debug tooling.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## 0.17 Highlights

- `StateStack` + `GameState` trait drive deferred `push` / `pop` / `replace` transitions through a single engine-owned dispatcher system; `on_pause` / `on_resume` default to no-op so a Pause state overlays Gameplay without tearing its scene down
- `SceneEntity { state_id }` marker auto-despawns scene-owned entities through the `CommandBuffer` when a state exits, inheriting the M13 frame-boundary visibility rules
- `scene.json` reuses the M15 `Transform` / `Sprite` / `Visibility` / `Tag` components; `asset_loader::spawn_scene` spawns every entry through the command buffer
- `ActionMap::default_map()` now ships `state_start` (`Enter`), `state_pause` (`KeyP`), and `state_back` (`Backspace`) so the new example flow works out-of-the-box without editing `input.json`
- New `example-04-scene-state` demonstrates the `MainMenu → Gameplay → Pause → Gameplay` loop and the data-driven `scene.json` spawn path end-to-end

## Documents

| File | Use |
| --- | --- |
| [`DESIGN.md`](DESIGN.md) | Architecture, stack, subsystem detail |
| [`AGENTS.md`](AGENTS.md) | Repo rules, commands, test layers, task workflow |
| [`DECISIONS.md`](DECISIONS.md) | Non-obvious decisions and rationale (`D-NNN`) |
| [`CLAUDE.md`](CLAUDE.md) | Claude Code pointer file |
| [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) | Subsystem → source-path map for coding agents |
| [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md) | Canonical profiling workflow, capture rules, perf budgets |
| [`CHANGELOG.md`](CHANGELOG.md) | Versioned change history |

## Quick Start

```bash
cargo build --workspace
cargo test --workspace
cargo run -p example-01-platformer      # comprehensive engine demo
cargo run -p example-02-sprite-stress   # canonical perf stress scene
cargo run -p example-03-component-sprites
cargo run -p example-04-scene-state
```

Reproducible Linux perf capture:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300   # primary scene (default)
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300   # render-hot-path baseline
bash scripts/test-perf-capture.sh
```

## Read Order

- Human: `README.md` → `DESIGN.md` → `DECISIONS.md` → `AGENTS.md`
- AI agent: `AGENTS.md` → `docs/LLM_INDEX.md` → touched files only; use `DESIGN.md` for architecture and `DECISIONS.md` for rationale when needed

## License

MIT. See [LICENSE](LICENSE).
