# M12 Profiling Workflow

Establishes the reproducible CPU/GPU baseline that anchored Phase 3 perf gates and remains the canonical capture contract for ongoing work. Compare runs only when scene, build mode, backend, and frame window match.

## Canonical Capture Rules

| Setting | Value |
| --- | --- |
| Build mode | `--release` |
| Primary scene | `example-02-sprite-stress` with `STRESS_SCENE=ecs-high-load` (full-system stress: ECS, physics, steering, camera, render) |
| Secondary scene | `example-02-sprite-stress` with `STRESS_SCENE=baseline` (render-hot-path baseline, preserves M17/M18 history) |
| Physics scene | `example-02-sprite-stress` with `STRESS_SCENE=physics-stress` (narrow phase + solver: 3,000 dynamic circle colliders piling under gravity in a static box, engine-default extract) |
| Linux backend | `WGPU_BACKEND=vulkan` |
| Resolution | `1920x1080` for sprite stress |
| Present mode | `display.present_mode = "auto"` |
| VSync selector | `display.vsync = false` for throughput measurement |
| Default max frame latency | `display.max_frame_latency = 1` |
| Warm-up window | first `60` frames ignored |
| Capture window | `300` measured frames after warm-up |
| GPU timings | opt-in via `TUNGSTEN_GPU_TIMING=1` only |

## Quick Start

Run the capture script from the repo root:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh                     # defaults to ecs-high-load 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300   # explicit primary scene
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300   # render-hot-path baseline
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300  # contacts + solver scene
```

Each run writes a timestamped directory under `perf-runs/` with telemetry logs, optional GPU timing logs, optional `perf` artifacts, and a per-run `README.md`. The script runs `60 + requested_frames` total frames, parses renderer metadata into separate README rows, and computes post-warm-up averages plus `p50` / `p95` / `p99` for `total` and `render_acquire`.

`just perf <args>` wraps the script. Full runs (without `--telemetry-only`) also write `perf-stat.txt`, `perf-record.data` and a `flamegraph.svg` folded from that recording (`flamegraph --perfdata`) into the same directory. The game still runs from the repo root so config and manifests resolve, but nothing is written there. The capture binary is built once with `RUSTFLAGS="-C force-frame-pointers=yes"` (override with `TUNGSTEN_PERF_RUSTFLAGS`). That setting replaces `.cargo/config.toml`'s `target-cpu=native`, so default script captures are generic x86-64 builds. Historical captures used different flags; use their recorded provenance. Each README records the compiler and build flags; compare only captures whose flags match.

Each README also records provenance read before the build: the short git commit, whether the tree was dirty (with a 12-hex fingerprint of the tracked diff plus untracked files, so two dirty captures of the same tree still match), the cpu0 `scaling_governor` and the ACPI `platform_profile` (`n/a` when the host lacks them). Attribute a drift between captures only when commit, fingerprint, governor and profile are known; a governor or profile change alone can move CPU-bound scenes.

All four scenes launch `example-02-sprite-stress`; the capture script injects `STRESS_SCENE=ecs-high-load`, `STRESS_SCENE=baseline`, `STRESS_SCENE=physics-stress`, or `STRESS_SCENE=render-features` for the child process and resets any inherited `STRESS_SCENE` / `STRESS_COUNT` so canonical runs stay reproducible. `physics-stress` was added by the 2026-07 performance audit ([`docs/plans/archive/perf-overhead-audit.md`](../plans/archive/perf-overhead-audit.md)) because no prior scene exercised the narrow phase and solver — `ecs-high-load` spawns dynamic bodies without colliders.

For Vulkan frame-pacing sweeps, keep the default rows as full captures and use telemetry-only override rows for alternate configs:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --present-mode immediate --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --present-mode immediate --max-frame-latency 3 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --present-mode mailbox --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --present-mode mailbox --max-frame-latency 3 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode mailbox --max-frame-latency 3 --telemetry-only
```

`--present-mode` and `--max-frame-latency` inject child-only `TUNGSTEN_RENDER_PRESENT_MODE` / `TUNGSTEN_RENDER_MAX_FRAME_LATENCY` compatibility overrides, so the checked-in `tungsten.json` stays unchanged while the runtime display resolver still lands on the requested pacing values.

`--stress-count <n>` injects a child-only `STRESS_COUNT` override and suffixes the output directory with `count<n>`. Canonical runs stay at each scene's default count (no flag); use override rows for scale sweeps, e.g. the physics 20k-body target:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300                              # canonical 3k baseline
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300 --stress-count 10000 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300 --stress-count 20000 --telemetry-only
```

`--repeat <n>` runs `n` sequential captures of one build into `run-1/` … `run-n/` (each with its own telemetry, README and `metrics.tsv`) and writes `summary.md` at the capture root: the median across runs of each run's average and p95 for `total`, `update`, `extract`, `render`, `render_acquire`, `render_encode` and every system, with the per-run values alongside. All measured runs go back to back; in a full capture the profilers then run once, into `run-1/`. This is the comparison rule below in one command:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --repeat 3 --telemetry-only
```

Compare checkpoints with at least three sequential captures and the median of their p95 values; quote `summary.md` medians rather than a single run.

The per-run README also reports `update`, `extract` and `render` stage averages/percentiles alongside `total` and `render_acquire`, plus a per-system table from the `systems:` telemetry lines; for physics scenes `update` (and its `physics_step` row) is the primary signal because `physics_step` runs inside the update stage.

Parser-only verification:

```bash
bash scripts/test-perf-capture.sh
```

### Physics awake phase

With smoke mode's fixed dt, the canonical `physics-stress` pile falls asleep at the same frame in every run (the first all-asleep `physics:` line is frame 286 of 360 on 2026-09-27, leaving 225 awake measured frames), after which `physics_step` drops to a fraction of a millisecond. The whole-window average therefore blends two regimes, and any change that shifts settling time moves it independently of solver cost. Judge physics changes on the **Physics Awake Phase** README section instead: `physics_step`, `update`, `total`, pairs and contacts over the measured frames whose `physics:` line shows `sleeping < dynamic`. `summary.md` carries the same rows as `awake:*` medians plus the awake frame count, which should stay stable across runs of one build.

For solver-throughput rows with no sleep onset at all, `--physics-sleep off` (physics-stress only) injects a child-only `STRESS_PHYSICS_SLEEP=0`, which sets `PhysicsConfig::sleep_threshold = 0`, and suffixes the directory with `sleepoff`:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300 --physics-sleep off --repeat 3 --telemetry-only
```

The matching criterion bench is `physics_step/dense_pile_awake/3000`: the `dense_pile` world settled with sleeping off, so every iteration runs the awake contact solve. `physics_step/dense_pile/3000` settles with sleeping on and mostly measures the slept state.

### ECS workload and density sweeps

ECS scene **v2** (2026-09-28) caches constant drift/color phases and simplifies radial repulsion. Compare engine changes only within the same scene version; its savings are workload changes, not engine gains. The canonical scene remains 50,000 entities in a 3200×1800 world.

`--ecs-density fixed|preserve` applies only to `ecs-high-load`. `fixed` is the default and keeps the same world at every count, so larger counts increase both iteration cost and neighbor density. `preserve` scales both world dimensions by `sqrt(count / 50000)`: world area is proportional to count, while sprite size, grid cell size and neighbor radius stay fixed. This isolates density better, but changing bounds, initial layout, flow and camera coverage still changes the simulation and rendered population. It is not a pure ECS benchmark. At 50k the two modes have identical dimensions and behavior.

```bash
for count in 12500 25000 50000; do
  WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --stress-count "$count" --ecs-density fixed --repeat 3 --telemetry-only
  WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --stress-count "$count" --ecs-density preserve --repeat 3 --telemetry-only
done
cargo bench -p tungsten-core --bench ecs_bench -- high_load_
```

The density flag sets child-only `STRESS_ECS_DENSITY`, appears in capture provenance and directory suffixes, and clears inherited values. The two `high_load_*_50k` Criterion benches use the scene v2 seven-component archetype and isolate read/mutable column iteration from neighbor search and allocation. They are new baselines, not speedup comparisons, and use the bench build flags (native by default), separately from canonical frame-pointer captures.

## Tracked Rows

Checkpoints and optimization steps compare these rows, each as `--repeat 3 --telemetry-only` medians plus one full capture for profiles:

| Row | Command args | Judge on |
| --- | --- | --- |
| ECS + steering | `ecs-high-load 300` | `update`, per-system rows |
| Physics | `physics-stress 300` | awake-phase `physics_step` (see above), whole-window `total` p95 |
| Present path | `sprite-stress 300` | `total`, `render_acquire` (present-bound; its p95 is acquire noise) |
| Render throughput | `sprite-stress 300 --stress-count 100000` | `extract`, `render_encode` avg/p95, `gpu` avg/p95 |
| Render features | `render-features 300` | per-pass GPU rows, `render_span`, extract/encode; separate 4k mixed-batch workload |

The canonical 2k `sprite-stress` row is present-bound: acquire dominates and the GPU pass is a fraction of a millisecond, so render-path costs only show at scale. Judge render-path changes (sprite upload, default extract) on the 100k row's `extract`, `render_encode` and `gpu` values, not on any row's total p95. Each README and `summary.md` reports avg and p95 for `render_encode`, `render_submit_present` and `gpu` (the last from the run's GPU-timing log).

## Frame Pacing Policy

`display.present_mode` is the final authority when set to a concrete value. The checked-in defaults are `display.present_mode = "auto"`, `display.vsync = false`, and `display.max_frame_latency = 1`, so the default path still resolves to the engine's auto no-vsync family. Legacy `window.vsync` / `render.present_mode` / `render.max_frame_latency` fields and env overrides remain valid compatibility inputs in M17. `max_frame_latency` is the requested `wgpu` hint, not a backend-confirmed effective queue depth.

Reference Vulkan matrix captured on April 16, 2026 on AMD Radeon 660M (`RADV REMBRANDT`) + AMD Ryzen 5 6600H, Arch Linux, `rustc 1.94.1`, with `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`, and `target-cpu=native`:

| Config | Scene | Avg total | p95 total | p99 total | Avg acquire | p95 acquire | p99 acquire |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Immediate / 1` | sprite-stress | `3.74ms` | `13.79ms` | `15.54ms` | `3.39ms` | `13.36ms` | `14.95ms` |
| `Immediate / 2` | sprite-stress | `3.78ms` | `13.95ms` | `16.49ms` | `3.44ms` | `13.35ms` | `15.80ms` |
| `Immediate / 3` | sprite-stress | `3.03ms` | `11.57ms` | `15.31ms` | `2.70ms` | `10.73ms` | `14.93ms` |
| `Mailbox / 2` | sprite-stress | `2.75ms` | `12.05ms` | `15.46ms` | `2.36ms` | `11.20ms` | `15.13ms` |
| `Mailbox / 3` | sprite-stress | `2.46ms` | `11.68ms` | `14.51ms` | `2.07ms` | `10.53ms` | `13.50ms` |
| `Immediate / 1` | platformer | `4.11ms` | `15.00ms` | `16.98ms` | `3.40ms` | `13.73ms` | `15.90ms` |
| `Immediate / 2` | platformer | `4.21ms` | `15.29ms` | `16.77ms` | `3.51ms` | `13.93ms` | `16.13ms` |
| `Mailbox / 2` | platformer | `4.00ms` | `15.50ms` | `16.66ms` | `3.31ms` | `14.66ms` | `15.87ms` |

Takeaways:

- `Mailbox / 3` produced the lowest sprite-stress averages on this machine; `Mailbox / 2` was close behind and remains a useful explicit pacing-sensitivity knob.
- None of the non-default rows displaced the checked-in default. `Immediate / 1` remains the shipped path because the engine’s `auto` mode intentionally preserves the existing `Immediate`-first no-vsync selection, and platformer gains were too small to justify a blanket override.
- Keep `display.max_frame_latency = 1` as the checked-in default. Treat `2` and `3` as opt-in tuning values, not blanket upgrades.

## Engine Telemetry

Enable stage-level frame logging:

```bash
TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 RUST_LOG=tungsten::app=debug \
  cargo run --release -p example-02-sprite-stress

TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 RUST_LOG=tungsten::app=debug \
  STRESS_SCENE=ecs-high-load \
  cargo run --release -p example-02-sprite-stress
```

Output format:

```text
backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: immediate max_frame_latency: 1 timestamp_query: true
frame: total=3.21ms update=0.42ms flush=0.00ms extract=0.37ms render=2.11ms render_acquire=1.44ms render_encode=0.48ms render_submit_present=0.17ms gpu=n/a audio=0.01ms hot_reload=0.00ms
```

`frame:` values come from `tungsten::FrameTimings` and are populated once per `RedrawRequested`. `gpu=` is populated only when `TUNGSTEN_GPU_TIMING=1` is enabled; otherwise it remains `n/a`. Startup metadata is the source of truth for renderer backend, adapter, chosen present mode, and requested max-frame-latency hint.

Each `frame:` line is followed by a `systems:` line with every registered system's wall time in registration order (`FrameTimings::system_timings`; whitespace and `=` in names become `_`):

```text
systems: steer_agents_system=69.53ms physics_step=2.02ms confine_agents_system=0.05ms ...
```

The capture script turns these into the per-run README's per-system table (avg/p50/p95/p99), next to `update`, `extract` and `render` stage percentiles. Captures from binaries without the `systems:` line report the table as n/a.

When a `PhysicsBuffers` resource exists (any scene that runs `physics_step`), a `physics:` line follows: proxies and dynamic entity bodies gathered by the last step, sleeping bodies, and the final substep's broadphase pairs and contact constraints:

```text
physics: proxies=3003 dynamic=3000 sleeping=0 pairs=30862 contacts=8670
```

## GPU Diagnostics

Enable GPU pass timing:

```bash
TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 TUNGSTEN_GPU_TIMING=1 \
  cargo run --release -p example-02-sprite-stress

TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 TUNGSTEN_GPU_TIMING=1 \
  STRESS_SCENE=ecs-high-load \
  cargo run --release -p example-02-sprite-stress
```

GPU timing forces a blocking `device.poll(wait_indefinitely())` readback every frame. Use it for diagnosis only. It inflates CPU-side frame timings. Do not use it during flamegraph or `perf` captures.

`GpuFrameTimings::frame_gpu_ms` and `frame: gpu=` retain their historical **scene-only** meaning. `pass_gpu_ms` reports every actual render pass in execution order: scene; indexed post slots; bloom threshold, each downsample/upsample mip and composite; SMAA edges/blend/neighborhood; text; present blit. Repeated post effects have separate slot labels. `render_gpu_ms` / `render_span` spans the first scene timestamp through the end of the present blit, including gaps. It excludes queued uploads, query resolve/readback and presentation waits, and is not a sum of pass durations.

A `gpu_passes:` companion line follows each logged frame; it is empty when disabled, unsupported, skipped or readback fails, so warm-up counting remains aligned. Capture READMEs and repeat summaries report per-pass and span avg/p95 from the **separate GPU diagnostic run**. Passes absent in a frame have no sample. There is one query resolve and blocking readback per timed frame. Query count follows the actual stack and bloom mip count; stacks exceeding the adapter API's query-set limit render normally without timings. Untimed rendering allocates no query buffers or pass labels.

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh render-features 300 --repeat 3 --telemetry-only
```

`render-features` defaults to 4,000 rotating sprites across three z layers, nearest/linear atlas textures, stock and damage-flash material batches (two uniform variants), and normal/emissive lit sprites. It uses the root manifest, two point lights plus one directional light, bloom followed by vignette, SMAA High and overlay text. `--stress-count` changes sprite count; the lights and post stack stay fixed. This is a new representative attribution baseline, not an optimization or a replacement for the 100k sprite row. SMAA is this scene's explicit setting; existing scenes and shipped config defaults remain as before. Inspect actual pass rows when environment render overrides are present.

Reference GPU spot-check from April 16, 2026 on the same Vulkan setup:

- `Immediate / 1` on `example-02-sprite-stress`: `avg_total = 3.70ms`, `avg_render_acquire = 1.31ms`, `avg_gpu = 0.61ms`
- the GPU pass stayed far below total frame time
- conclusion: these captures are dominated by presentation pacing, not shader or draw throughput

## Manual CPU Profiling

Prefer the capture script above. Manual `cargo flamegraph` and bare `perf record` write `perf.data` into the current directory, which must be the repo root; pass `perf record -o <dir>/perf.data` to keep it out. `cargo flamegraph` with different `RUSTFLAGS` also rebuilds `target/release`.

### Flamegraph

```bash
TUNGSTEN_SMOKE_FRAMES=360 RUSTFLAGS="-C force-frame-pointers=yes" cargo flamegraph \
  --package example-02-sprite-stress \
  --bin example-02-sprite-stress \
  --release

TUNGSTEN_SMOKE_FRAMES=360 STRESS_SCENE=ecs-high-load \
  RUSTFLAGS="-C force-frame-pointers=yes" cargo flamegraph \
  --package example-02-sprite-stress \
  --bin example-02-sprite-stress \
  --release
```

### Smaller recordings

Full captures retain DWARF stacks by default for inline attribution. Use `--call-graph fp` for smaller stack records or `--sample-frequency 499` to reduce sampling volume (both may be combined):

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --call-graph fp --sample-frequency 499
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300 --sample-frequency 499
```

Frame-pointer mode requires `force-frame-pointers=yes` in the effective capture build flags. These are already the default flags; system libraries may still lack frame pointers, so use DWARF when stacks truncate or attribution is ambiguous. A lower frequency gives fewer samples, especially in short captures. Keep frame windows fixed and use repeat telemetry to judge timings; profiler samples locate costs rather than replacing that comparison. READMEs and repeat summaries record the requested call graph and sampling frequency; `perf-record.log` retains perf's diagnostics (including throttling/errors). An omitted frequency preserves perf's own default.

### Manual `perf stat` / `perf record`

Build once, then profile the binary directly so compilation and Cargo are not counted. Prefer the capture script for canonical flags, metadata and output naming.

```bash
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p example-02-sprite-stress
mkdir -p perf-runs/manual
TUNGSTEN_SMOKE_FRAMES=360 STRESS_SCENE=baseline WGPU_BACKEND=vulkan \
  perf stat -d -- target/release/example-02-sprite-stress
TUNGSTEN_SMOKE_FRAMES=360 STRESS_SCENE=ecs-high-load WGPU_BACKEND=vulkan \
  perf record -o perf-runs/manual/perf.data --call-graph dwarf -- target/release/example-02-sprite-stress
perf report -i perf-runs/manual/perf.data
```

## Backend Override Reference

| `WGPU_BACKEND` | Typical platform | `TIMESTAMP_QUERY` availability |
| --- | --- | --- |
| `vulkan` | Linux | best chance for `Some(frame_gpu_ms)` |
| `dx12` | Windows | often available on modern hardware |
| `metal` | macOS | backend-dependent; verify per machine |
| `gl` | fallback | may be unavailable or noisy |
| `auto` | any | convenient, but less reproducible |

`GpuFrameTimings::frame_gpu_ms` measures the scene pass only, excluding post-processing, SMAA, overlay text and presentation. It is expected to be `None` when the active backend or adapter does not expose timestamp queries. Backend, adapter, chosen present mode, and requested max-frame-latency hint are emitted once at renderer startup when `TUNGSTEN_PERF_LOG=1` is set.

## RenderDoc Workflow

Linux Vulkan capture flow:

1. Launch RenderDoc.
2. Set the executable to the built example binary under `target/release/`.
3. Set environment `WGPU_BACKEND=vulkan`.
4. Start capture and trigger a representative frame after warm-up.
5. Inspect the main render pass for draw-call count, texture bindings, and pass duration.

## Perf Budgets

| Metric | Target |
| --- | --- |
| Sustained FPS | `>= 60` |
| p95 frame time | `<= 16.7ms` |
| Update stage | keep well below `4ms` in canonical scenes |
| Extract stage | keep well below `3ms` in canonical scenes |
| Render stage | keep well below `8ms` in canonical scenes |

These are guardrails, not hard engine limits. Record intentional deviations in milestone notes.

## Hotspot Identification Guide

Search for these first in flamegraphs:

- `App::window_event`
- `render_frame_full`
- `render_frame_full_timed`
- `extract_`
- `query2`
- `physics_step`
- `glyphon`
- `wgpu`

Interpretation:

- Cross-reference hot flamegraph regions with `TUNGSTEN_PERF_LOG` stage timings.
- A hot render stack with low `render_ms` often means sampling noise.
- A hot stage plus a matching telemetry spike usually indicates a real regression.
- When `render_ms` is high, use `render_acquire`, `render_encode`, and `render_submit_present` to classify the regression as swapchain pacing, CPU command generation, or present/readback wait.

## Regression Policy

- Treat steady-state regressions above `10%` in canonical captures as noteworthy
- If a change intentionally trades performance for capability, add a short note in `DECISIONS.md` or the milestone plan explaining the regression and why it is acceptable
