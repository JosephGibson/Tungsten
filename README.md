# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.15.0` on branch `0.15`. Phase 3 M18 is shipped. The engine now pairs typed two-window event queues and deferred ECS command buffers with a shared camera module, a core-owned display state/config model with frame-boundary runtime apply, and a runtime telemetry HUD rendered through the existing text pipeline. Next milestone: `M19` input mapping.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## 0.15 Highlights

- `tungsten::DebugHud` now ships as a world resource with built-in FPS, camera, display, player, counts, and top-N system timing rows plus a custom-row extension hook
- `F4` toggles the developer HUD at runtime, and `example-01-platformer` now tags the player entity so the HUD can report position and speed directly
- `tungsten::RenderCounts` and `World::entity_count()` expose frame-local render counts and O(1) live-entity counts for diagnostics
- The shipped HUD defaults now favor readability on busy scenes: larger text, a taller line height, and a throttled text refresh interval while EWMA timing still updates every frame

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
