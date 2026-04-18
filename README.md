# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.14.0` on branch `0.14`. Phase 3 M17 is shipped. The engine now pairs typed two-window event queues and deferred ECS command buffers with a shared camera module plus a core-owned display state/config model, frame-boundary runtime display requests, and display telemetry wired up for the upcoming HUD work. Next milestone: `M18` runtime telemetry HUD.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## 0.14 Highlights

- `tungsten_core` now ships `DisplayState`, `DisplayConfig`, `DisplayMode`, `ScaleMode`, `Resolution`, and `DisplayValidationError`
- `tungsten.json` now carries a canonical `display` block while legacy `window.*` and `render.*` display fields remain valid for M17 compatibility
- `tungsten::request_display_settings` is the single public runtime mutation path, and `tungsten::DisplayTelemetry` mirrors the effective display state back into the `World`
- `example-01-platformer` now exercises the runtime path directly: `F11` toggles borderless fullscreen and `F9` toggles `vsync` while re-running auto present-mode selection

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
