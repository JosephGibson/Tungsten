# Tungsten

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Targets: native only (`Linux`, `macOS`, `Windows`). No WASM.

## Stack

Hand-rolled ECS with archetypal storage, deferred command buffers, and typed event queues; `wgpu` rendering; manifest-driven assets; `glyphon` text; `cpal` + `symphonia` + hand-rolled audio mixer; `notify` hot reload; `.tmj` / Tiled-compatible tilemaps; 2D AABB + circle physics with a spatial-grid broadphase, speculative CCD, warm-started soft contacts and island sleeping; frame telemetry, Criterion benches, and a perf capture workflow.

## Documents

The [documentation map](docs/README.md) names the document that owns each kind of fact, from the architecture and the repository's rules to the release procedure, and how to read a large one.

## Quick Start

Rust 1.98.1 is pinned in `rust-toolchain.toml`; rustup installs it on first use. The shared checks use [`just`](https://just.systems), `cargo-deny`, Python 3.12+, Bash, ShellCheck and actionlint (the smoke scripts also need `jq` and GNU `timeout`):

```bash
cargo install --locked just@1.58.0 cargo-deny@0.20.2   # ShellCheck 0.11.0, actionlint 1.7.12: see .github/workflows/ci.yml
just quick                              # fmt, context, repository QA, cargo check
just check                              # fmt check, clippy -D warnings, tests
just script-test                        # script regressions, ShellCheck and actionlint
cargo test --workspace                  # raw equivalent of the test step
cargo build --workspace
cd examples/01_platformer && cargo run   # a platformer sandbox on the template
cargo run -p example-02-bench           # benchmark suite; TUNGSTEN_BENCH selects one
```

To make a game of your own, start from the template: [Getting started](docs/getting-started.md).

Reproducible Linux perf capture (`docs/perf/profiling-workflow.md`):

```bash
just perf list                                               # the benchmark rows
WGPU_BACKEND=vulkan just perf run ecs                        # one capture of one row
WGPU_BACKEND=vulkan just perf sitting --quick                # the tree against the last tag, informational
WGPU_BACKEND=vulkan just perf sitting                        # the full sitting a minor or major release runs
just perf counters                                           # GPU draw and upload counter pins
just perf-test                                               # runner regression tests, no GPU
```

## Read Order

- Human: `README.md` → `DESIGN.md` → `DECISIONS.md` → `AGENTS.md`
- AI agent: `AGENTS.md` (plus the scoped `AGENTS.md` of any directory being edited) → touched files only; `docs/LLM_INDEX.md` on demand, `DESIGN.md` for architecture and `DECISIONS.md` for rationale when needed

## License

MIT. See [LICENSE](LICENSE).
