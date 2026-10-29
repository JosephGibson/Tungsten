---
name: tungsten-perf
description: Tungsten performance work — benchmark rows (just perf), sittings and verdicts, counter guards and pins, GPU counters, baselines, telemetry, flamegraph/perf/samply, GPU timings, frame time/p95, pacing, or perf-runs/. Encodes the repo's capture rules.
---

# tungsten-perf

[Profiling workflow](../../../docs/perf/profiling-workflow.md) owns the capture contract: read its capture rules first, then the section for the operation. [Benchmarks](../../../docs/perf/benchmarks.md) owns the rows' workloads, knobs, owned metrics and guards, the counter guards and the levers for 9.2 (`D-078` as redo 7.3 amends it). `scripts/perf-rows.json` is the one registry of rows. Do not keep a second copy of those tables here.

## Entry points

`just perf` runs `scripts/perf.py`; `scripts/perf_stats.py` holds parsing, statistics and verdicts. Rows: the five benchmarks of `example-02-bench` as seven rows, `e01-pit`, `template` and the `e01-areas` group.

```bash
just perf list
WGPU_BACKEND=vulkan just perf run ecs                       # one capture
WGPU_BACKEND=vulkan just perf sitting --quick               # the tree against the last tag, CPU rows, informational
WGPU_BACKEND=vulkan just perf sitting --rows physics,ecs    # a full sitting: A1 B1 A2 B2, one rerun
just perf compare <capture> <capture>
just perf baseline save <capture-or-sitting> <name>
just perf counters                                          # GPU draw and upload pins, local
just perf-test
```

## Policy

- **No per-plan sitting.** A plan's perf checks are its counter guards, which `just check` runs. A change to a hot path (ECS, frame loop, extract, physics, render) may quote a quick sitting as information.
- **Full sittings** are the evidence for a plan that claims a speed-up, measures a 7.14, 7.15 or 9.2 list item, or changes a row's workload; and for every minor or major release, against the previous tag, before its cut, unattended with the owner's NoMachine client disconnected. A `regressed` owned metric needs a fix or a known-issues entry with the sitting's path before the cut.
- **Render or extract changes** run `just perf counters` beside `just smoke` and `just visual`.

## Rules easy to miss

- Captures are release builds with frame pointers and every function and non-fallthrough block aligned to 64 bytes, on both sides of a sitting, so code placement moves no row; `--plain` gives shipping flags. Compare requires the same row, workload (version, knobs, script hash, resolution, warm-up, frames), build flags, renderer and machine, read from the captures.
- **A sitting judges a change.** An owned metric reads `regressed` only when both direct pairs say so and the base's A/A reads it `unchanged` or `noisy`; an unstable A/A or a known-drift metric earns one rerun, whose verdicts stand alone. Peak RSS is judged only within one sitting.
- **No remote-desktop encoder, no other work.** `run` refuses the encoder and foreign builds; a sitting's start check also refuses a process above 20 % of a core and a tree that changes, and `--wait-quiet S` waits for 60 s of quiet. Run each capture or sitting as one blocking command, or one background job with nothing beside it. Never write to the tree while `pgrep -af scripts/perf.py` shows a capture.
- **Counter guards.** A failing guard names each counter's pinned and new value; a rise in a cost counter needs a fix or its cause in the plan. Rewrite a pin only for a meant change: `TUNGSTEN_GUARDS_WRITE=1` on that test, then run it twice. Pins are Linux's.
- **GPU timings arrive late.** `gpu_passes: from=N` lines come up to four frames after frame N, from a ring that never stalls; a GPU run takes five tail frames and needs 95 % of its window sampled. `frame:`'s `gpu=` is for the HUD only.
- **After restoring files, confirm the rebuild.** `cp -p` keeps old mtimes, so cargo skips the rebuild; the build must print `Compiling`.
- Measure FPS from `interval`; `total` excludes pacing waits. Pacing studies run the binary by hand under `TUNGSTEN_DISPLAY_PRESENT_MODE` / `TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY`; do not change shipped defaults to chase a row.
- A row's work change bumps its `workload_version`; engine changes never do.

## Route by task

| Task | Read |
| --- | --- |
| Capture failures or digests | [Validity and determinism](../../../docs/perf/profiling-workflow.md#validity-and-determinism) |
| Judging a change | [Sittings](../../../docs/perf/profiling-workflow.md#sittings), [Compare](../../../docs/perf/profiling-workflow.md#compare) |
| Plan done-when checks | [Writing done-when checks](../../../docs/perf/profiling-workflow.md#writing-done-when-checks) |
| Counter guards, GPU counters | [Counter guards](../../../docs/perf/benchmarks.md#counter-guards) |
| What 9.2 can work on | [Levers for 9.2](../../../docs/perf/benchmarks.md#levers-for-92) |
| perf, flamegraph or samply | [Profiling](../../../docs/perf/profiling-workflow.md#profiling), then [Hotspots](../../../docs/perf/profiling-workflow.md#hotspots) |
| Add a row | [Adding a row](../../../docs/perf/benchmarks.md#adding-a-row) |

Use native profilers on the built binary, with outputs under `perf-runs/`. Criterion's native-flag micro-benchmarks are separate from captures. New runtime profiling dependencies need a `D-015` decision.
