# M12 Profiling Workflow

M12 establishes a reproducible CPU/GPU baseline before Phase 3 feature work. Use the same scene, build mode, backend, and frame window when comparing runs.

## Canonical Capture Rules

| Setting | Value |
| --- | --- |
| Build mode | `--release` |
| Primary scene | `example-02-sprite-stress` |
| Secondary scene | `example-01-platformer` |
| Linux backend | `WGPU_BACKEND=vulkan` |
| Resolution | `1920x1080` for sprite stress |
| Present mode | `render.present_mode = "auto"` |
| VSync selector | `window.vsync = false` for throughput measurement |
| Default max frame latency | `1` |
| Warm-up window | first 60 frames ignored |
| Capture window | 300 measured frames after warm-up |
| GPU timings | opt-in via `TUNGSTEN_GPU_TIMING=1` only |

## Quick Start

Use the capture script from the repo root:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300
```

Each run writes a timestamped directory under `perf-runs/` with telemetry logs, optional GPU timing logs, optional `perf` artifacts, and a per-run `README.md`.
The script automatically runs `60 + requested_frames` total frames, parses renderer metadata
into separate README rows, and computes post-warm-up averages plus `p50`/`p95`/`p99` for
`total` and `render_acquire`.

For the published Vulkan frame-pacing matrix, keep the default rows as full captures and use
telemetry-only override rows for the alternate configs:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode immediate --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode immediate --max-frame-latency 3 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode mailbox --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode mailbox --max-frame-latency 3 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300 --present-mode immediate --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300 --present-mode mailbox --max-frame-latency 2 --telemetry-only
```

`--present-mode` and `--max-frame-latency` inject child-only
`TUNGSTEN_RENDER_PRESENT_MODE` / `TUNGSTEN_RENDER_MAX_FRAME_LATENCY` overrides for the capture
processes, so the checked-in `tungsten.json` stays unchanged.

Parser-only verification for the reporting helpers:

```bash
bash scripts/test-perf-capture.sh
```

## Frame Pacing Policy

`render.present_mode` is the final authority when set to a concrete value. The checked-in
default keeps `render.present_mode = "auto"` and `render.max_frame_latency = 1`, so
`window.vsync` still selects the auto-vsync vs auto-no-vsync family on the default path.
`max_frame_latency` should be read as the requested `wgpu` hint, not a backend-confirmed
effective queue depth.

Reference Vulkan matrix captured on April 16, 2026 on an AMD Radeon 660M (`RADV REMBRANDT`)
and AMD Ryzen 5 6600H (Arch Linux, rustc 1.94.1, `lto = "thin"`, `codegen-units = 1`,
`panic = "abort"`, `target-cpu=native`):

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

- `Mailbox / 3` produced the lowest sprite-stress averages on this machine; `Mailbox / 2`
  was close behind and remains a useful explicit profiling knob when testing pacing sensitivity.
- None of the non-default rows displaced the checked-in default. `Immediate / 1` stays the
  shipped path because the engine's `auto` mode intentionally preserves the existing
  `Immediate`-first no-vsync selection, and platformer gains were too small to justify a
  blanket override.
- Keep `max_frame_latency = 1` as the checked-in default for now. Treat `2` and `3`
  as opt-in tuning values, not blanket upgrades.

## Engine Telemetry

Enable stage-level frame logging with:

```bash
TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 RUST_LOG=tungsten::app=debug \
  cargo run --release -p example-02-sprite-stress
```

Output format:

```text
backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: Immediate max_frame_latency: 1 timestamp_query: true
frame: total=3.21ms update=0.42ms flush=0.00ms extract=0.37ms render=2.11ms render_acquire=1.44ms render_encode=0.48ms render_submit_present=0.17ms gpu=n/a audio=0.01ms hot_reload=0.00ms
```

`frame:` values come from `tungsten::FrameTimings`, populated once per `RedrawRequested`. The `gpu=` field is populated only when `TUNGSTEN_GPU_TIMING=1` is enabled; otherwise it remains `n/a`.
The startup metadata line is the source of truth for the renderer backend, adapter, chosen
present mode, and requested max-frame-latency hint used in the run.

## GPU Diagnostics

Enable GPU pass timing with:

```bash
TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 TUNGSTEN_GPU_TIMING=1 \
  cargo run --release -p example-02-sprite-stress
```

Caveat: GPU timing forces a blocking `device.poll(wait_indefinitely())` readback every frame. This is for diagnosis only and will inflate CPU-side frame timings. Do not use it during flamegraph or `perf` captures.

Reference GPU spot-check from April 16, 2026 on the same Vulkan setup:

- `Immediate / 1` on `example-02-sprite-stress`: `avg_total = 3.70ms`, `avg_render_acquire = 1.31ms`, `avg_gpu = 0.61ms`.
- The GPU pass stayed far below total frame time, reinforcing that the dominant cost in these
  captures is presentation pacing rather than shader or draw throughput.

## Manual CPU Profiling

### Flamegraph

```bash
TUNGSTEN_SMOKE_FRAMES=360 RUSTFLAGS="-C force-frame-pointers=yes" cargo flamegraph \
  --package example-02-sprite-stress \
  --bin example-02-sprite-stress \
  --release
```

### `perf stat`

```bash
TUNGSTEN_SMOKE_FRAMES=360 perf stat -d -- cargo run --release -p example-02-sprite-stress
```

### `perf record`

```bash
TUNGSTEN_SMOKE_FRAMES=360 perf record --call-graph dwarf -- cargo run --release -p example-02-sprite-stress
perf report
```

## Backend Override Reference

| `WGPU_BACKEND` | Typical platform | `TIMESTAMP_QUERY` availability |
| --- | --- | --- |
| `vulkan` | Linux | best chance for `Some(frame_gpu_ms)` |
| `dx12` | Windows | often available on modern hardware |
| `metal` | macOS | backend-dependent; verify per machine |
| `gl` | fallback | may be unavailable or noisy |
| `auto` | any | convenient, but less reproducible |

`GpuFrameTimings::frame_gpu_ms` is expected to be `None` when the active backend or adapter does not expose timestamp queries. Backend, adapter, chosen present mode, and requested max-frame-latency hint are emitted once at renderer startup when `TUNGSTEN_PERF_LOG=1` is set.

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
| Update stage | keep well below 4ms in canonical scenes |
| Extract stage | keep well below 3ms in canonical scenes |
| Render stage | keep well below 8ms in canonical scenes |

These are guardrails, not hard engine limits. Record deviations in milestone notes when they are intentional.

## Hotspot Identification Guide

When reading flamegraphs, start by searching for:

- `App::window_event`
- `render_frame_full`
- `render_frame_full_timed`
- `extract_`
- `query2`
- `physics_step`
- `glyphon`
- `wgpu`

Cross-reference hot flamegraph regions with `TUNGSTEN_PERF_LOG` stage timings. A hot render stack with low `render_ms` often means sampling noise; a hot stage and a matching telemetry spike usually indicates a real regression.
When `render_ms` is high, use `render_acquire`, `render_encode`, and `render_submit_present` to decide whether the regression is swapchain pacing, CPU command generation, or present/readback wait.

## Regression Policy

Treat steady-state regressions above 10% in canonical captures as noteworthy. If a change intentionally trades performance for capability, add a short note in `DECISIONS.md` or the milestone plan explaining the regression and why it is acceptable.
