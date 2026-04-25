# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.24.0` on branch `0.24`. Phase 3 is complete; all milestones `M12`–`M24` shipped. The rollout plan is archived at [`docs/plans/archive/phase3.md`](docs/plans/archive/phase3.md). Phase 4 is underway: M25 (render foundation), M26 (materials + post-stack + tween→material bridge), and M27 (SMAA 1x presentation AA) are live; remaining milestones are tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md).

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## Documents

| File | Use |
| --- | --- |
| [`DESIGN.md`](DESIGN.md) | Architecture, stack, subsystem detail |
| [`AGENTS.md`](AGENTS.md) | Repo rules, commands, test layers, task workflow |
| [`DECISIONS.md`](DECISIONS.md) | Non-obvious decisions and rationale (`D-NNN`) |
| [`CLAUDE.md`](CLAUDE.md) | Claude Code pointer file |
| [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) | Subsystem → source-path map for coding agents |
| [`docs/plans/README.md`](docs/plans/README.md) | Session-plan storage rules and milestone plan naming convention |
| [`docs/plans/phase4.md`](docs/plans/phase4.md) | Active Phase 4 plan and milestone index |
| [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md) | Canonical profiling workflow, capture rules, perf budgets |
| [`CHANGELOG.md`](CHANGELOG.md) | Versioned change history |

## Quick Start

```bash
cargo build --workspace
cargo test --workspace
cargo run -p example-01-platformer      # comprehensive engine demo
cargo run -p example-02-sprite-stress   # canonical perf stress scene
cargo run -p example-03-scene-state     # scene/state + tween transition demo
cargo run -p example-04-shader-playground  # materials + 17-effect post-stack demo
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
