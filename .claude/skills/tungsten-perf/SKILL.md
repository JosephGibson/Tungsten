---
name: tungsten-perf
description: Tungsten performance work — baselines, telemetry, flamegraph/perf/samply, GPU timings, regression judgment, frame time/FPS/p95, present mode and swapchain pacing, or the perf-runs/ directory. Encodes the repo's canonical capture rules.
---

# tungsten-perf

Tungsten has a disciplined, in-repo profiling workflow. Use it; don't invent a new one.

## Single source of truth

**Read [docs/perf/profiling-workflow.md](../../../docs/perf/profiling-workflow.md) first.** It is authoritative for capture rules, the present-mode matrix, telemetry format, GPU diagnostics, budgets and regression policy. This skill only highlights what's easy to miss.

## Canonical capture

Never compare runs across different scenes, build modes, backends, flags or frame windows. The comparable setup:

- `--release` build, `WGPU_BACKEND=vulkan` on Linux
- 60 warm-up frames discarded, 300 measured
- Primary scene: `ecs-high-load` (the script default; `example-02-sprite-stress` with `STRESS_SCENE=ecs-high-load`)
- Secondary: `sprite-stress` (`STRESS_SCENE=baseline`); physics: `physics-stress`

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --telemetry-only   # telemetry only
```

Outputs land in `perf-runs/<timestamp>-<scene>/` with a per-run `README.md` listing `p50/p95/p99` for `total` and `render_acquire`. The parser regression check is `bash scripts/test-perf-capture.sh`. Compare checkpoints with at least three sequential captures and the median of their p95 values.

## Telemetry format

Stage logging (`TUNGSTEN_PERF_LOG=1`) emits one line per redraw:

```
frame: total=... update=... flush=... extract=... render=... render_acquire=... render_encode=... render_submit_present=... gpu=n/a audio=... hot_reload=...
```

Values come from `tungsten::FrameTimings`. `gpu=` is `n/a` unless `TUNGSTEN_GPU_TIMING=1`. See [telemetry.rs](../../../crates/tungsten/src/telemetry.rs) and [app.rs](../../../crates/tungsten/src/app.rs).

## GPU timing is for diagnosis only

`TUNGSTEN_GPU_TIMING=1` forces a blocking `device.poll(wait_indefinitely())` every frame and inflates CPU-side timings. **Never enable it during a flamegraph, perf or samply capture**; you'd be profiling the blocking poll.

## CPU profiling tools

Native tooling only:

- **Flamegraph** (preferred for hotspots): prefer the capture script without `--telemetry-only`, which records flags and writes into the capture directory. By hand:
  ```bash
  TUNGSTEN_SMOKE_FRAMES=360 RUSTFLAGS="-C force-frame-pointers=yes" cargo flamegraph \
    --package example-02-sprite-stress --bin example-02-sprite-stress --release
  ```
  Setting `RUSTFLAGS` replaces the target flags from `.cargo/config.toml`, so compare only against captures built with the same effective flags.
- **`perf stat`** / **`perf record --call-graph dwarf`**: exact invocations are in the workflow doc.
- **samply** works as an interactive alternative to `perf record`; keep capture windows aligned with the 360-frame smoke window.
- **criterion** for microbenchmarks; pair with `cargo flamegraph --bench` for a specific bench.

Release debug symbols come from the profile config; don't weaken optimization for a capture unless the workflow doc says so.

## Hotspot shortlist

Search these first in a flamegraph: `App::window_event`, `render_frame_full`, `render_frame_full_timed`, `extract_`, `query2`, `physics_step`, `glyphon`, `wgpu`.

## Interpretation

- Hot render stack with low `render_ms` is sampling noise, not a regression.
- A hot stage with a matching telemetry spike is a real regression.
- When `render_ms` is high, classify via `render_acquire` (swapchain pacing), `render_encode` (CPU command generation) or `render_submit_present` (present/readback wait).
- Near-zero baselines (e.g. `render_acquire` ≈ 0.1 ms) make percentage deltas meaningless; compare absolute values.

## Budgets (guardrails, not hard limits)

- Sustained ≥ 60 FPS; p95 frame time ≤ 16.7 ms
- Update < 4 ms, Extract < 3 ms, Render < 8 ms on canonical scenes

## Regression policy

Steady-state regressions > 10% on canonical captures are noteworthy. Intentional trade-offs go in a `DECISIONS.md` entry or the milestone plan with a short justification.

## Do not

- Suggest web/Node profilers (Lighthouse, Chrome DevTools, Clinic.js, bundle analyzers). Wrong stack.
- Add a profiling crate as a runtime dependency without a D-015 entry in `DECISIONS.md`.
- Change `tungsten.json` pacing defaults to chase a benchmark; use `--telemetry-only` overrides. Shipped defaults change only by explicit decision.
