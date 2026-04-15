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
| VSync | disabled for throughput measurement |
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
The script automatically runs `60 + requested_frames` total frames and computes averages from the post-warm-up portion only.

## Engine Telemetry

Enable stage-level frame logging with:

```bash
TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 RUST_LOG=tungsten::app=debug \
  cargo run --release -p example-02-sprite-stress
```

Output format:

```text
backend: Vulkan adapter: AMD RADV NAVI10 present_mode: Immediate max_frame_latency: 1 timestamp_query: true
frame: total=3.21ms update=0.42ms extract=0.37ms render=2.11ms render_acquire=1.44ms render_encode=0.48ms render_submit_present=0.17ms gpu=n/a audio=0.01ms hot_reload=0.00ms
```

`frame:` values come from `tungsten::FrameTimings`, populated once per `RedrawRequested`. The `gpu=` field is populated only when `TUNGSTEN_GPU_TIMING=1` is enabled; otherwise it remains `n/a`.

## GPU Diagnostics

Enable GPU pass timing with:

```bash
TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 TUNGSTEN_GPU_TIMING=1 \
  cargo run --release -p example-02-sprite-stress
```

Caveat: GPU timing forces a blocking `device.poll(wait_indefinitely())` readback every frame. This is for diagnosis only and will inflate CPU-side frame timings. Do not use it during flamegraph or `perf` captures.

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

`GpuFrameTimings::frame_gpu_ms` is expected to be `None` when the active backend or adapter does not expose timestamp queries. Backend, adapter, chosen present mode, and max-frame-latency hint are emitted once at renderer startup when `TUNGSTEN_PERF_LOG=1` is set.

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
