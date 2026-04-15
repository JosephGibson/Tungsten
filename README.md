# Tungsten

A from-scratch Rust 2D game engine. `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Native only (Linux / macOS / Windows) — no WASM.

**Status:** `v0.10.0` — release line prepared. Deferred ECS command buffers now land at a fixed frame boundary alongside the existing telemetry, benchmark, profiling tooling, and swapchain frame-pacing follow-up. Next: M14 event queues.

## Stack

Hand-rolled ECS (archetypal storage + deferred command buffers), wgpu rendering, manifest-driven assets, text (glyphon), audio (cpal + symphonia + hand-rolled mixer), hot reload (notify), tilemaps (.tmj / Tiled), 2D physics (AABB + circle, uniform-grid broad-phase), and Phase 3 tooling (frame telemetry, Criterion benches, perf capture workflow).

## 0.10 Highlights

- Deferred ECS command buffers with a fixed post-system flush boundary.
- Swapchain pacing controls in `tungsten.json`: `render.present_mode` and `render.max_frame_latency`.
- Perf capture reporting now includes renderer metadata plus `p50`/`p95`/`p99` for total and acquire timing.
- Canonical Linux profiling workflow documented in [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md).

## Documents

| File | Purpose |
|---|---|
| [`DESIGN.md`](DESIGN.md) | Architecture, stack, subsystem detail. **Context.** |
| [`AGENTS.md`](AGENTS.md) | Operational rules for working in the repo. **Tasks.** |
| [`DECISIONS.md`](DECISIONS.md) | Log of non-obvious decisions with rationale (D-NNN). |
| [`CLAUDE.md`](CLAUDE.md) | Pointer file for Claude Code. |
| [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) | Subsystem → source paths for coding agents. |
| [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md) | Canonical profiling workflow, capture rules, and perf budgets. |
| [`CHANGELOG.md`](CHANGELOG.md) | Per-version change log. |

## Quick start

```bash
cargo build --workspace
cargo test --workspace
cargo run -p example-01-platformer      # comprehensive engine demo
cargo run -p example-02-sprite-stress   # canonical perf stress scene
```

For reproducible profiling captures on Linux:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
bash scripts/test-perf-capture.sh
```

## Read order

| Audience | Order |
|---|---|
| Human | `DESIGN.md` → `DECISIONS.md` → `AGENTS.md` |
| AI agent | `AGENTS.md` → `DESIGN.md` → `DECISIONS.md` |

## License

MIT — see [LICENSE](LICENSE).
