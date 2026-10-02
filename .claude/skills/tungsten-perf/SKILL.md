---
name: tungsten-perf
description: Tungsten performance work — the benchmark suite (just perf), baselines, compare verdicts, capacity search, telemetry, flamegraph/perf/samply, GPU timings, frame time/p95, present mode and swapchain pacing, or the perf-runs/ directory. Encodes the repo's canonical capture rules.
---

# tungsten-perf

[Profiling workflow](../../../docs/perf/profiling-workflow.md) owns the capture contract. Read its capture rules first, then the section for the requested operation. [Benchmarks](../../../docs/perf/benchmarks.md) owns workloads, knobs, guards, owned metrics, dated results and open proposals (`D-078`). Do not maintain a second copy of those tables here.

## Entry points

`example-02-bench` runs six benchmarks across eight tracked rows. `just perf` wraps `scripts/bench.py`; `scripts/bench_report.py` owns statistics and reports. Use `just perf describe` for the binary's current knobs and rows.

```bash
WGPU_BACKEND=vulkan just perf run physics --repeat 5
WGPU_BACKEND=vulkan just perf suite --repeat 5
just perf compare <baseline> <candidate>
just perf baseline save <capture-or-suite> <name>
WGPU_BACKEND=vulkan just perf capacity --all --budget 60hz
just perf-test
```

## Rules easy to miss

- Canonical captures use release builds with the runner's `-C force-frame-pointers=yes`, Vulkan, 1920×1080, auto no-vsync and latency 1, each row's warm-up plus 300 measured frames, and `--repeat 5`. Setting `RUSTFLAGS` replaces `.cargo/config.toml`'s native flags; compare only identical effective flags.
- Compare requires matching row, workload version, knobs, frame counts, build flags, backend, adapter, pacing and machine. Run a same-build A/A on a quiet machine before trusting verdicts; known placement/mode effects are in the [Compare section](../../../docs/perf/profiling-workflow.md#compare).
- **No remote-desktop encoder during captures.** Check `pgrep -x nxcodec.bin` before and after, poll during long runs, and recapture any run that overlaps it. Validity guards and digests do not catch that load.
- **Nothing else runs beside a capture.** No other agent session or `cargo` build in the tree, and none of your own commands: run each sitting as one blocking foreground command. Guards and digests miss this load too; see [capture rules](../../../docs/perf/profiling-workflow.md#comparison-rule-and-capture-rules).
- **After restoring files, confirm the rebuild.** `cp -p` keeps old mtimes, so cargo skips the rebuild and the binary keeps the removed change; the build must print `Compiling` before a capture. See [Profiling](../../../docs/perf/profiling-workflow.md#profiling).
- Timing runs are telemetry-only. `TUNGSTEN_GPU_TIMING=1` blocks on readback and belongs only in a separate diagnostic run, never a CPU timing run or profile.
- Judge only the row's owned metrics. A `regressed` owned metric needs a fix or a recorded justification in a decision or plan. `noisy` needs more repeats or a quieter machine; thresholds and owned metrics change only by decision. RSS growth and capacity are informational.
- Measure FPS from `interval`, the frame period; `total` measures work and excludes pacing waits. GPU `frame:` timing is scene-only; `gpu_passes:` and `render_span` come from diagnostic runs. See [Telemetry lines](../../../docs/perf/profiling-workflow.md#telemetry-lines).
- Pacing studies use `--present-mode` / `--max-frame-latency` child overrides. Do not change shipped defaults to chase a benchmark. [Frame pacing](../../../docs/perf/profiling-workflow.md#frame-pacing) owns precedence and historical matrices.
- A benchmark work change bumps its `workload_version` and starts a new baseline. Engine optimizations do not change workload versions. Archiving a plan does not approve its leftover experiments.

## Route by task

| Task | Read |
| --- | --- |
| Capture failures or digests | [Validity and determinism](../../../docs/perf/profiling-workflow.md#validity-and-determinism) |
| Compare verdicts and drift | [Compare](../../../docs/perf/profiling-workflow.md#compare) |
| Plan done-when checks | [Writing done-when checks](../../../docs/perf/profiling-workflow.md#writing-done-when-checks) |
| Scale or frame budgets | [Capacity search](../../../docs/perf/profiling-workflow.md#capacity-search) |
| RSS or allocation questions | [Memory](../../../docs/perf/profiling-workflow.md#memory) |
| perf, flamegraph or samply | [Profiling](../../../docs/perf/profiling-workflow.md#profiling), then [Hotspots](../../../docs/perf/profiling-workflow.md#hotspots) |
| Add or recalibrate a workload | [Adding a benchmark](../../../docs/perf/benchmarks.md#adding-a-benchmark) |

Use native profilers and the built binary, with outputs under `perf-runs/`. Criterion's native-flag microbenchmarks are separate from runner captures. New runtime profiling dependencies need a `D-015` decision; web/Node profilers do not apply to this engine.
