# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.13.0` on branch `0.13`. Phase 3 M16 is shipped. The engine now pairs typed two-window event queues and deferred ECS command buffers with a shared camera module built around authoritative `CameraState` and `CameraController` resources, plus M12 telemetry, benchmark coverage, profiling tooling, the swapchain frame-pacing follow-up, and canonical gameplay-side render components with an opt-in default sprite-extract path. Next milestone: M17 display state + config.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## 0.13 Highlights

- Shared `CameraState`, `CameraController`, `CameraMode`, and `CameraBounds` now ship from `tungsten_core`
- `camera_update_system` writes one authoritative camera state per frame with follow, dead-zone, smoothing, bounds clamp, zoom multiplier, and deterministic shake support
- `example-01-platformer` now uses the shared camera path for player follow, map-edge clamp, and window-height-derived zoom
- Camera tests cover follow/clamp behavior, zero-rotation matrix compatibility, zoom-multiplier updates, and deterministic shake

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
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
bash scripts/test-perf-capture.sh
```

## Read Order

- Human: `README.md` → `DESIGN.md` → `DECISIONS.md` → `AGENTS.md`
- AI agent: `AGENTS.md` → `docs/LLM_INDEX.md` → touched files only; use `DESIGN.md` for architecture and `DECISIONS.md` for rationale when needed

## License

MIT. See [LICENSE](LICENSE).
