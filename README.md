# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.16.0` on branch `0.16`. Phase 3 M19 is shipped. The engine now pairs typed two-window event queues and deferred ECS command buffers with a shared camera module, a core-owned display state/config model with frame-boundary runtime apply, a runtime telemetry HUD rendered through the existing text pipeline, and a workspace-root `input.json` action map that covers keyboard, mouse buttons, wheel directions, hot reload, and engine-owned HUD/display/exit controls. Next milestone: `M20` scene/state system.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## 0.16 Highlights

- `input.json` at the workspace root maps named actions to keyboard, mouse-button, and scroll bindings; it loads at startup, merges with built-in defaults, and survives layout-preserving rewrites through `ActionMap::persist`
- `ActionMap` is a world resource exposing `is_pressed`, `just_pressed`, and `just_released` per action; `tungsten::InputState` now also tracks cursor position/delta plus line and pixel scroll deltas
- Engine-owned actions cover HUD toggle (`engine_toggle_hud`), vsync toggle (`engine_toggle_vsync`), fullscreen toggle (`engine_toggle_fullscreen`), and exit (`engine_exit`); gameplay binds its own actions in `input.json` instead of hardcoded keycodes
- The hot-reload watcher now observes `input.json` alongside the asset manifests, so binding edits take effect at the next frame boundary without a restart

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
