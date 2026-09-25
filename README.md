# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Status

Workspace `v0.26.0`; current development branch `0.27`. Phase 3 is complete; all milestones `M12`–`M24` shipped. The rollout plan is archived at [`docs/plans/archive/phase3.md`](docs/plans/archive/phase3.md). Phase 4 is underway: M25 (render foundation), M26 (materials + post-stack + tween→material bridge), M27 (SMAA 1x presentation AA), M28 (bloom), and M29 (2D forward normal-mapped lighting) are live; remaining milestones are tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md).

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a uniform-grid broad-phase; frame telemetry, Criterion benches, and a perf capture workflow.

## Documents

| File | Use |
| --- | --- |
| [`DESIGN.md`](DESIGN.md) | Architecture, stack, subsystem detail |
| [`AGENTS.md`](AGENTS.md) | Repo rules, commands, test layers, task workflow |
| [`DECISIONS.md`](DECISIONS.md) | Non-obvious decisions and rationale (`D-NNN`) |
| [`CLAUDE.md`](CLAUDE.md) | Imports `AGENTS.md` for Claude Code |
| [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) | On-demand task → source-path map for coding agents |
| [`docs/agent-setup.md`](docs/agent-setup.md) | How Claude Code and Codex load instructions, skills and search filters |
| [`docs/plans/README.md`](docs/plans/README.md) | Session-plan storage rules and milestone plan naming convention |
| [`docs/plans/phase4.md`](docs/plans/phase4.md) | Active Phase 4 plan and milestone index |
| [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md) | Canonical profiling workflow, capture rules, perf budgets |
| [`CHANGELOG.md`](CHANGELOG.md) | Versioned change history |

## Quick Start

Rust 1.98.1 is pinned in `rust-toolchain.toml`; rustup installs it on first use. The shared checks use [`just`](https://just.systems), `cargo-deny`, Python 3.9+, Bash and ShellCheck (perf capture also needs `jq` and GNU `timeout`):

```bash
cargo install --locked just@1.58.0 cargo-deny@0.20.2   # ShellCheck 0.11.0: see .github/workflows/ci.yml
just quick                              # fmt, context, repository QA, cargo check
just check                              # fmt check, clippy -D warnings, tests
just script-test                        # script regressions and ShellCheck
cargo test --workspace                  # raw equivalent of the test step
cargo build --workspace
cargo run -p example-01-platformer      # comprehensive engine demo
cargo run -p example-02-sprite-stress   # baseline scene; use perf command below for ecs-high-load
cargo run -p example-03-scene-state     # scene/state + tween transition demo
cargo run -p example-04-shader-playground  # materials + 18-effect post-stack demo (incl. bloom)
```

Reproducible Linux perf capture:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300   # primary scene (default)
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300   # render-hot-path baseline
bash scripts/test-perf-capture.sh
```

## Read Order

- Human: `README.md` → `DESIGN.md` → `DECISIONS.md` → `AGENTS.md`
- AI agent: `AGENTS.md` (plus the scoped `AGENTS.md` of any directory being edited) → touched files only; `docs/LLM_INDEX.md` on demand, `DESIGN.md` for architecture and `DECISIONS.md` for rationale when needed

## License

MIT. See [LICENSE](LICENSE).
