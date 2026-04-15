# Tungsten

A from-scratch Rust 2D game engine. `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Native only (Linux / macOS / Windows) — no WASM.

**Status:** `v0.9.0` — Phase 3 M12 complete. CPU/GPU telemetry, benchmark harnesses, and baseline capture tooling are in place. Next: M13 command buffers.

## Stack

Hand-rolled ECS (archetypal storage), wgpu rendering, manifest-driven assets, text (glyphon), audio (cpal + symphonia + hand-rolled mixer), hot reload (notify), tilemaps (.tmj / Tiled), 2D physics (AABB + circle, uniform-grid broad-phase), and Phase 3 baseline tooling (frame telemetry, Criterion benches, perf capture workflow).

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
```

## Read order

| Audience | Order |
|---|---|
| Human | `DESIGN.md` → `DECISIONS.md` → `AGENTS.md` |
| AI agent | `AGENTS.md` → `DESIGN.md` → `DECISIONS.md` |

## License

MIT — see [LICENSE](LICENSE).
