---
name: tungsten-perf
description: Tungsten performance work — the benchmark suite (just perf), baselines, compare verdicts, capacity search, telemetry, flamegraph/perf/samply, GPU timings, frame time/p95, present mode and swapchain pacing, or the perf-runs/ directory. Encodes the repo's canonical capture rules.
---

# tungsten-perf

Tungsten has a disciplined, in-repo benchmark workflow. Use it; don't invent a new one.

## Single source of truth

**Read [docs/perf/profiling-workflow.md](../../../docs/perf/profiling-workflow.md) first.** It is authoritative for capture rules, configuration, telemetry, validity, compare verdicts, capacity search, memory, tracked rows and profiling. [docs/perf/benchmarks.md](../../../docs/perf/benchmarks.md) describes each benchmark: bottleneck, knobs, presets, guards, owned metrics and calibrated defaults. `D-078` records the design. This skill only highlights what's easy to miss.

## The suite

`example-02-bench` runs one of six benchmarks per launch: `physics` (rows `physics`, `physics-sparse`), `ecs`, `churn`, `gpu` (rows `gpu`, `gpu-throughput`), `particles` and `integrated`. `TUNGSTEN_BENCH`, `TUNGSTEN_BENCH_PRESET`, `TUNGSTEN_BENCH_SCALE` and `TUNGSTEN_BENCH_SET` configure it; `just perf describe` prints knobs, presets and tracked rows from the binary. `just perf` runs `scripts/bench.py` (Python 3.12, standard library); `just perf-test` runs its tests.

```bash
WGPU_BACKEND=vulkan just perf run physics --repeat 5        # one capture
WGPU_BACKEND=vulkan just perf suite --repeat 5              # all eight tracked rows, ~6 min
just perf compare <baseline> <candidate>                    # captures or suites, dirs or baseline names
just perf baseline save <capture-or-suite> <name>
WGPU_BACKEND=vulkan just perf capacity --all --budget 60hz  # and --budget 144hz
```

## Canonical capture

Never compare across rows, `workload_version`, knobs, frames, warm-up, build flags, backend, adapter, present mode, frame latency or machine; compare suppresses verdicts on any of these.

- Release build with `-C force-frame-pointers=yes` (the runner's default; it replaces `target-cpu=native`), `WGPU_BACKEND=vulkan`, 1920×1080, `auto` present mode with vsync off and latency 1.
- Each benchmark's warm-up (60–180 frames) plus 300 measured frames; `--repeat 5` for compare-grade captures; a verdict needs 3 runs per side.
- Captures are telemetry-only by default. GPU timing runs only in the separate diagnostic run (`gpu`, `integrated`).
- **Nothing else on the GPU or screen.** A remote-desktop encoder (`nxcodec.bin`, NoMachine) moved `ecs` p95 by 50% while guards passed and digests matched. Check `pgrep -x nxcodec.bin` before and after every capture, poll during long ones, and redo any capture that overlapped a session.
- Run a same-build A/A compare once per machine before trusting verdicts.

## Validity

A capture is valid when every run exits 0, logs every measured frame, logs a `bench-config` equal to the request, confirms a requested present mode and latency in its `backend:` line, passes the row's guards and matches the other runs' determinism digest. Invalid captures stay on disk with `valid: false`; the runner exits 3 and compare refuses verdicts unless `--force`. Guards: `physics.sleeping <= 0` (both physics rows), `bench.teleports >= 1`, `bench.structural <= 0` and `bench.entities constant` (`ecs`), `bench.population constant` and `bench.spawned == bench.despawned` (`churn`), `bench.live within ±10% of its median` (`particles`), `bench.view_out <= 0` (`integrated`).

## Owned metrics and verdicts

Each row is judged only on what it owns; everything else is context.

- `physics`: `physics_step` p50/p95, `update` p95. `physics-sparse`: `physics_step` p50/p95.
- `ecs`: `update` p50/p95 and the 14 system rows at p50. `churn`: `flush` p50/p95 and the four churn systems at p50.
- `gpu`: scene pass and `render_span` p50/p95, the post, SMAA, text and present passes at p50, `extract` and `render_encode` p50/p95 (GPU values from the diagnostic run; n/a without timestamp queries). `gpu-throughput`: `extract`, `render_encode` p50/p95.
- `particles`: `unattributed` p50/p95 (the particle stage has no timing of its own) and `animate_sprites` p50/p95.
- `integrated`: `total` p50/p95/p99 and jitter (p99 − p50).

Verdicts use per-run values and a 95% Welch interval against τ = max(τ_rel × baseline, τ_abs): τ_rel 3% (mean, p50), 5% (p95), 8% (p99); τ_abs 0.05 ms for stages, 0.02 ms for systems and GPU passes. Jitter takes p99's threshold. Peak RSS: 2% and 2 MiB. Labels: `regressed`, `improved`, `unchanged`, `noisy` (rerun with more repeats; never loosen τ). A `regressed` owned metric needs a fix or a justification in `DECISIONS.md` or the plan. RSS growth is reported only; its threshold is an open proposal.

## Telemetry format

`TUNGSTEN_PERF_LOG=1` with `RUST_LOG=tungsten::app=debug,bench=debug` emits per frame `frame:` (stages from `tungsten::FrameTimings`; `gpu=` is the scene pass, `n/a` unless `TUNGSTEN_GPU_TIMING=1`), `systems:`, `gpu_passes:` (with `render_span`), `physics:` when physics runs, and the benchmark's `bench:` counters, plus `backend:` and `bench-config:` once. `unattributed` = `total` − the timed stages. See [telemetry.rs](../../../crates/tungsten/src/telemetry.rs) and [app.rs](../../../crates/tungsten/src/app.rs).

## GPU timing is for diagnosis only

`TUNGSTEN_GPU_TIMING=1` forces a blocking `device.poll(wait_indefinitely())` every frame and inflates CPU-side timings. **Never enable it during a timing run, flamegraph, perf or samply capture**; you'd be profiling the blocking poll.

## CPU profiling tools

Native tooling only:

- **`just perf run <bench> --profile`** (preferred): perf stat, perf record and a flamegraph in the capture's `profile/`, with recorded flags. `--call-graph fp` (needs frame pointers) and `--sample-frequency 499` shrink recordings; DWARF stays the default for attribution.
- **By hand:** build once with `RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p example-02-bench`, then `perf record -o perf-runs/manual/perf.data --call-graph dwarf` or `samply record` on `target/release/example-02-bench` with `TUNGSTEN_BENCH` and `TUNGSTEN_SMOKE_FRAMES` set. Setting `RUSTFLAGS` replaces `.cargo/config.toml`'s flags, so compare only builds with identical flags.
- **criterion** for primitives (`just bench-build`); its native-flag numbers never compare with captures.

## Hotspot shortlist

`App::window_event`, `render_frame_full`, `render_frame_full_timed`, `extract_`, `extract_sprites_default`, `query2`, `World::flush`, `physics_step`, `build_pairs`, `gather_tilemap_proxies`, `particle_tick_system`, `glyphon` and the text `prepare`, `wgpu`.

## Interpretation

- Hot render stack with low `render` is sampling noise; a hot stage with a matching telemetry change is real.
- When `render` is high, classify via `render_acquire` (swapchain pacing), `render_encode` (CPU command generation) or `render_submit_present` (present/readback wait).
- Capacity search is informational: a ✗ limiting stage means the row hit a different wall than its declared bottleneck.
- Near-zero baselines make percentages meaningless; compare absolute values.

## Do not

- Suggest web/Node profilers (Lighthouse, Chrome DevTools, Clinic.js, bundle analyzers). Wrong stack.
- Add a profiling crate as a runtime dependency without a D-015 entry in `DECISIONS.md`.
- Change `tungsten.json` pacing defaults to chase a benchmark; use `--present-mode` / `--max-frame-latency` override captures. Shipped defaults change only by explicit decision.
- Loosen verdict thresholds, the variance floor or a row's owned metrics to make a compare pass, or change a benchmark's work without bumping its `workload_version`.
