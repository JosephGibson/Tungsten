# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a spatial-grid broadphase, speculative CCD, warm-started soft contacts and island sleeping; frame telemetry, Criterion benches, and a perf capture workflow.

## Documents

| File | Use |
| --- | --- |
| [`docs/README.md`](docs/README.md) | Documentation map, canonical sources and targeted reading |
| [`DESIGN.md`](DESIGN.md) | Architecture, stack, subsystem detail |
| [`AGENTS.md`](AGENTS.md) | Repo rules, commands, test layers, task workflow |
| [`DECISIONS.md`](DECISIONS.md) | Non-obvious decisions and rationale (`D-NNN`) |
| [`docs/DECISION_INDEX.md`](docs/DECISION_INDEX.md) | One-line decision summaries; find the relevant rationale without loading the full log |
| [`CLAUDE.md`](CLAUDE.md) | Imports `AGENTS.md` for Claude Code |
| [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) | On-demand task → source-path map for coding agents |
| [`docs/agent-setup.md`](docs/agent-setup.md) | How Claude Code and Codex load instructions, skills and search filters |
| [`docs/plans/README.md`](docs/plans/README.md) | Session-plan storage rules and milestone plan naming convention |
| [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md) | Profiling workflow: capture rules, compare verdicts, capacity search |
| [`docs/perf/benchmarks.md`](docs/perf/benchmarks.md) | The six benchmarks, their knobs, owned metrics and calibrated defaults |
| [`docs/releases.md`](docs/releases.md) | Internal release preparation, checks, Git handoff and recovery |
| [`docs/showcase/README.md`](docs/showcase/README.md) | Visual acceptance artifacts, availability and regeneration commands |
| [`CHANGELOG.md`](CHANGELOG.md) | Versioned change history |

## Quick Start

Rust 1.98.1 is pinned in `rust-toolchain.toml`; rustup installs it on first use. The shared checks use [`just`](https://just.systems), `cargo-deny`, Python 3.12+, Bash and ShellCheck (the smoke scripts also need `jq` and GNU `timeout`):

```bash
cargo install --locked just@1.58.0 cargo-deny@0.20.2   # ShellCheck 0.11.0: see .github/workflows/ci.yml
just quick                              # fmt, context, repository QA, cargo check
just check                              # fmt check, clippy -D warnings, tests
just script-test                        # script regressions and ShellCheck
cargo test --workspace                  # raw equivalent of the test step
cargo build --workspace
cargo run -p example-01-platformer      # comprehensive engine demo
cargo run -p example-02-bench           # benchmark suite; TUNGSTEN_BENCH selects one
cargo run -p example-03-scene-state     # scene/state + screen transition demo
cargo run -p example-04-shader-playground  # materials + 18-effect post-stack demo (incl. bloom)
```

Reproducible Linux perf capture (`docs/perf/profiling-workflow.md`):

```bash
WGPU_BACKEND=vulkan just perf suite --repeat 5               # every tracked benchmark row
just perf compare <baseline> <candidate>                     # verdicts, compare.md and compare.html
WGPU_BACKEND=vulkan just perf capacity --all --budget 60hz   # largest scale within a budget
just perf-test                                               # runner regression tests, no GPU
```

## Read Order

- Human: `README.md` → `DESIGN.md` → `DECISIONS.md` → `AGENTS.md`
- AI agent: `AGENTS.md` (plus the scoped `AGENTS.md` of any directory being edited) → touched files only; `docs/LLM_INDEX.md` on demand, `DESIGN.md` for architecture and `DECISIONS.md` for rationale when needed

## License

MIT. See [LICENSE](LICENSE).
