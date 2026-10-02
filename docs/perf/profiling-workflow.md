# Profiling Workflow

The capture contract for Tungsten performance work: how to run the benchmark suite, what a valid capture is, and how two captures are compared (`D-078`). [`benchmarks.md`](benchmarks.md) describes the six benchmarks, their knobs, guards, owned metrics and calibrated defaults. `just perf <subcommand>` runs `scripts/bench.py`, a standard-library Python 3.12 runner; `scripts/bench_report.py` holds the parsing, statistics and reports, and `just perf-test` runs `scripts/test-bench.py`.

## Comparison rule and capture rules

Compare two captures only when they measure the same row with the same `workload_version`, resolved knobs, frames, warm-up, build flags, backend, adapter, present mode, frame latency and machine. Compare checks every one of these and suppresses verdicts on a mismatch. Judge on the runs, not on single frames: compare-grade captures use `--repeat 5`, and a verdict needs at least 3 runs per side.

| Setting | Value |
| --- | --- |
| Build | `--release` with the tuned profile (`D-041`) and `RUSTFLAGS="-C force-frame-pointers=yes"`, the runner's default; `TUNGSTEN_PERF_RUSTFLAGS` overrides it. It replaces `.cargo/config.toml`'s `target-cpu=native`, so captures are generic x86-64 |
| Backend | `WGPU_BACKEND=vulkan` on Linux |
| Resolution | 1920×1080 (the `gpu` benchmark's `resolution` knob aside) |
| Present | `display.present_mode = "auto"` with `display.vsync = false`, `display.max_frame_latency = 1` and no `display.frame_rate_cap`, the checked-in `tungsten.json`; `immediate` / 1 on the reference machine |
| Frames | the benchmark's warm-up (60–180 frames), then 300 measured frames |
| Repeats | `--repeat 5` for compare-grade captures and suites |
| GPU timing | only in the separate GPU diagnostic run (`gpu` and `integrated`), never in a timing run |
| Machine | governor `performance`; nothing else using the GPU or the screen |

**No remote-desktop encoder may run during a capture.** A connected NoMachine client runs `nxcodec.bin`, which encodes the screen and competes for memory bandwidth with the integrated GPU. On the reference machine it slowed every third frame of `ecs`, moving `total` p95 from 11.5 to 18.2 ms, while every guard passed and the digests matched. The runner records no background-load provenance, so check before and after every capture, probe, smoke timing or visual run, and poll during long ones:

```bash
pgrep -x nxcodec.bin && echo "disconnect the remote-desktop client first"
```

A capture that overlapped such a session is invalid: exclude it and capture again.

## Quick start

From the repository root:

```bash
export WGPU_BACKEND=vulkan
just perf describe                                 # benchmarks, knobs, presets and tracked rows
just perf run physics --repeat 5                   # one capture: perf-runs/<UTC>-physics/
just perf suite --repeat 5                         # every tracked row: perf-runs/<UTC>-suite/
just perf baseline save perf-runs/<UTC>-suite main-2026-10-01
just perf suite --repeat 5 --compare main-2026-10-01
just perf compare <baseline> <candidate>           # compare.md, compare.html, compare.json
just perf capacity --all --budget 60hz             # largest scale per row; also 144hz or milliseconds
just perf-test                                     # runner regression tests, no GPU
```

A new machine starts with an A/A check (see "Compare") before any verdict is trusted.

## Configuration

The binary owns the knob schema and reads environment variables:

| Variable | Meaning |
| --- | --- |
| `TUNGSTEN_BENCH` | Benchmark: `physics` (default), `ecs`, `churn`, `gpu`, `particles` or `integrated` |
| `TUNGSTEN_BENCH_PRESET` | Named knob set; default `default`. Every benchmark has `min` and `default` |
| `TUNGSTEN_BENCH_SCALE` | Float multiplier, default 1.0, on the preset's scalable knobs |
| `TUNGSTEN_BENCH_SET` | `knob=value,knob=value` overrides, applied last |
| `TUNGSTEN_BENCH_DESCRIBE` | `1` prints every benchmark's schema as JSON; `config` prints the resolved configuration. Both exit before a window opens |

- **Resolution order:** preset, then scale (integer knobs round, every knob clamps to its range), then overrides, then validation. An unknown benchmark, preset or knob, or an out-of-range value, is a fatal error that names the knob and its range.
- **Runner flags** map one to one onto the variables: `--preset`, `--scale`, a repeatable `--set knob=value`, and `--sweep knob=v1,v2,…`, which writes one capture per value plus `sweep.md` and `sweep.html` plotting the owned metrics against the knob. Before anything runs, the runner resolves the request through `TUNGSTEN_BENCH_DESCRIBE=config`, so a bad request fails without a window, and each run's `bench-config:` line must then match that object.
- **`run` flags:** `--frames` (default 300), `--warmup` (default: the benchmark's), `--repeat`, `--gpu-timing on|off` (default: the benchmark's), `--present-mode` and `--max-frame-latency` (see "Frame pacing"), `--profile` (see "Profiling") and `--compare BASELINE`. `--sweep` combines with neither `--compare` nor `--profile`.
- **Child environment.** Every run clears inherited `TUNGSTEN_BENCH*`, `TUNGSTEN_RENDER_*`, `TUNGSTEN_DISPLAY_*`, `TUNGSTEN_CAPTURE_*`, `TUNGSTEN_GPU_TIMING`, `TUNGSTEN_OVERLAYS_ON`, `TUNGSTEN_SMOKE_FRAMES`, `TUNGSTEN_PERF_LOG` and `RUST_LOG`, then sets `TUNGSTEN_SMOKE_FRAMES` to warm-up plus frames, `TUNGSTEN_PERF_LOG=1` and `RUST_LOG=tungsten::app=debug,bench=debug`. `TUNGSTEN_SMOKE_FRAMES` pins `dt` to 1/60 s, so one build replays an identical workload. The binary runs from the repository root and writes nothing there.

```bash
TUNGSTEN_BENCH=physics TUNGSTEN_BENCH_PRESET=sparse TUNGSTEN_BENCH_SCALE=2 \
  TUNGSTEN_BENCH_SET=speed_max=2000,cell=64 cargo run --release -p example-02-bench
just perf run physics --preset sparse --scale 2 --set speed_max=2000 --set cell=64 --repeat 5
just perf run gpu --sweep lights=0,1,2,4,8,16 --repeat 3
```

## Capture layout and provenance

```text
perf-runs/<UTC>-<row>[-<preset>][-s<scale>][-set<hash6>][-<present>][-lat<N>]/
  capture.json        schema 1: row, bench, workload_version, resolved config, request, frames,
                      warm-up, build flags, provenance, renderer, owned metrics, per-run stats,
                      rusage and RSS growth, guards, digests, valid flag and reasons
  README.md           owned metrics, stages, frame time per run (maximum, spikes), systems,
                      GPU passes, counters, memory, validity
  run-N/telemetry.log run-N/gpu.log (GPU diagnostic run) run-N/rss.tsv
  profile/            perf-stat.txt, perf-record.data, perf-record.log, flamegraph.svg (--profile)
perf-runs/<UTC>-<row>…-sweep-<knob>/<knob>-<value>/…   plus sweep.json, sweep.md, sweep.html
perf-runs/<UTC>-suite[-<preset>][-s<scale>]/<row>/…    plus suite.json and README.md
perf-runs/<UTC>-compare-<row>/  or  -compare-suite/    compare.json, compare.md, compare.html
perf-runs/<UTC>-capacity-<budget>/                     capacity.json, capacity.md, capacity.html, probes/
perf-runs/baselines/<name>/                            a copied capture or suite plus baseline.json
```

The directory name carries the preset only when it differs from the row's own preset, and `set<hash6>` is a hash of the overrides. `perf-runs/` is gitignored and machine-local.

- **Per-run statistics.** `runs[].stats.<group>.<name>` holds `n`, `mean`, `min`, `p50`, `p95`, `p99` and `max` for the groups `stages`, `systems`, `gpu_passes` (GPU diagnostic runs only, under `gpu_run`), `physics` and `counters`. Percentiles use the nearest rank. `runs[].spikes` counts the measured frames whose `total` exceeds 1.5 × the run's p50; the README lists it per run beside the run's largest `total`.
- **Provenance** is read before the build, so it describes the built sources: the commit; the dirty-tree state (`no`, or `yes (diff <12 hex>)`, a SHA-256 over the tracked diff plus the untracked files, so two dirty captures of one tree match); the cpu0 governor, the ACPI platform profile and the AC state (`n/a` where the host lacks them); the kernel, CPU model, memory, hostname and the machine fingerprint built from them; `rustc --version`; and `WGPU_BACKEND`. Attribute a drift between captures only when commit, fingerprint, governor and profile are known.
- **Suites.** `suite.json` (schema 1, `kind: suite`) records the request (`preset`, `scale`, `only`, `repeat`, frames), the shared build and provenance, and one row per capture: preset, directory, validity and reasons, owned-metric medians, `total` p50/p95/p99 and jitter medians, and the median peak RSS. The suite is valid when every row is. Its README tabulates the rows and states the `--preset` rule when one applied.
- **Baselines.** `just perf baseline save <capture-or-suite> <name>` copies the directory (without `*.data` profiler recordings) to `perf-runs/baselines/<name>/` and writes `baseline.json`: name, kind (`capture` or `suite`), source, date, machine fingerprint, validity, and the row or rows. `baseline list` and `baseline rm <name>` manage them. A baseline name works wherever a capture or suite directory does. Captures without `capture.json`, such as the retired bash script's, can't be baselines.

## Telemetry lines

`TUNGSTEN_PERF_LOG=1` with `RUST_LOG=tungsten::app=debug,bench=debug` logs one group per frame. The parser strips the `env_logger` prefix and attaches each companion line to the preceding `frame:` line; missing companions are tolerated. System and pass names are literal keys.

```text
backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: immediate max_frame_latency: 1 timestamp_query: true
bench-config: {"bench":"integrated","derived":{…},"knobs":{…},"preset":"default","row":"integrated","scale":1.0,"seed":1,"workload_version":1}
frame: total=11.81ms interval=11.94ms update=7.24ms flush=0.16ms extract=2.60ms render=1.44ms render_acquire=0.03ms render_encode=0.96ms render_submit_present=0.43ms gpu=n/a audio=0.00ms hot_reload=0.00ms
systems: __hud_toggle=0.00ms bench_counters=0.02ms actor_ai=0.08ms physics_step=6.57ms collision_events=0.15ms …
gpu_passes:
physics: proxies=13045 dynamic=4517 sleeping=1016 pairs=6461 contacts=6080
bench: actors=2500 projectiles=16 hits=1 particles=6249 lights=28 camera_x=1216 view_out=0 flashing=20 landings=20 shots=0 turns=86 events=22373
```

- **`backend:`** once at renderer startup: backend, adapter, chosen present mode, the requested frame-latency hint and timestamp-query support. Compare reads them as hard fields.
- **`frame:`** from `tungsten::FrameTimings`, once per redraw. `render` contains acquire, encode and submit/present. `unattributed` is derived: `total` minus `update`, `flush`, `hot_reload`, `extract`, `render` and `audio`. It holds the untimed particle and tween stages and the event flush. `interval` is the time from the previous frame's start to this frame's start, `n/a` on the first frame: the previous frame's `total` plus what follows it, which is the telemetry logging, a frame cap's wait and the event loop's turnaround. It is the period of the frame rate, so FPS is 1000 / mean `interval`; on the reference machine its mean is 0.05–0.13 ms above the mean `total` in the suite's rows, so an FPS taken from `total` reads 0.5% (`gpu`) to 1.5% (`integrated`) too high. It is not work: it is no part of `total`, of `unattributed`, of compare's stacked stage bars or of capacity's limiting stage, and no row owns it.
- **`systems:`** every registered system's wall time in registration order (`FrameTimings::system_timings`); whitespace and `=` in names become `_`.
- **`gpu_passes:`** empty unless `TUNGSTEN_GPU_TIMING=1` and the adapter has timestamp queries, so frame counts stay aligned. It lists every render pass in execution order (scene; indexed post slots such as `post0_bloom_threshold`; each bloom mip and the composite; `smaa_edges`, `smaa_blend_weights`, `smaa_neighborhood`; `text`) plus `render_span`, from the first scene timestamp to the end of the text overlay, gaps included. Since `D-087` the last full-screen stage writes the swapchain, so there is no `present` pass. Only a screenshot frame (`TUNGSTEN_CAPTURE_FRAME`) still blits, and the runner's captures have none. `render_span` excludes uploads, query readback and presentation waits and isn't a sum of passes. `frame:`'s `gpu=` keeps its scene-pass-only meaning. A stack that exceeds the adapter's query-set limit renders without timings.
- **`physics:`** when physics runs: proxies and dynamic bodies gathered by the last step, sleeping bodies, and the final substep's pairs and contacts.
- **`bench-config:`** once: the resolved configuration as JSON, with derived values such as the world size or the expected batch count.
- **`bench:`** the benchmark's counters. Frame N + 1's first system logs frame N's line, so it follows frame N's group; the last frame has none. A hex token such as `ecs`'s `digest=0x…` stays out of the numeric statistics.

To read the lines without the runner:

```bash
TUNGSTEN_BENCH=ecs TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 RUST_LOG=tungsten::app=debug,bench=debug \
  WGPU_BACKEND=vulkan cargo run --release -p example-02-bench
```

## Validity and determinism

A capture is valid when every run, GPU diagnostic runs included:

- exits 0;
- reports the requested number of measured frames after the warm-up;
- logs a `bench-config:` line equal to the resolved request;
- reports the present mode and frame latency in its `backend:` line that `--present-mode` and `--max-frame-latency` asked for, when either was given (an `auto`, `auto_vsync` or `auto_no_vsync` request accepts any mode of its family);
- passes the row's guards (checked on every measured frame; `bench:` guards skip the last frame, which has no line);
- has the same determinism digest as every other run.

The digest is a SHA-256, shortened to 16 hex digits, over the exact `physics:` and `bench:` text of every measured frame. One build under the pinned `dt` replays the same workload, so repeats must match; GPU diagnostic runs join the match. Digests aren't compared across captures: an engine change legitimately moves them. [`benchmarks.md`](benchmarks.md) lists each row's guards.

An invalid capture stays on disk with `valid: false` and its reasons in `capture.json` and the README. `run`, `suite` and `--sweep` then exit 3, and compare suppresses verdicts unless `--force` is given; `--force` never overrides a hard mismatch.

## Compare

`just perf compare <baseline> <candidate>` takes two captures or two suites, each a directory or a baseline name; mixing a capture and a suite is an error. `run --compare BASELINE` and `suite --compare BASELINE` compare the new capture or suite as soon as it is written.

1. **Comparability.** Hard fields suppress every verdict on a mismatch: row, `workload_version`, every resolved knob, frames, warm-up, build flags, backend, adapter, present mode, frame latency and the machine fingerprint. Soft notes flag a different `rustc`, kernel, governor, platform profile, AC state or GPU-diagnostic setting. Workload counters (`physics:` fields and `bench:` counters) whose means move by more than 5% raise a workload-drift warning: engine changes legitimately move trajectory-dependent counts, so verdicts still appear, marked.
2. **Statistics.** The run is the statistical unit, because frames within a run are autocorrelated and share clocks, thermals and memory placement. Each run contributes its mean, p50, p95, p99 and max per metric. Jitter is a derived per-run statistic, p99 − p50 of `total`.
3. **Verdict per metric and statistic.**
   - Δ is the mean of the candidate's per-run values minus the mean of the baseline's.
   - The interval is a 95% Welch interval over the per-run values. Each side's variance is floored at (0.5% of its mean)², and the t quantile comes from a built-in table, rounding the degrees of freedom down to the next tabulated entry.
   - The practical threshold is τ = max(τ_rel × the baseline mean, τ_abs), with τ_rel 3% for mean and p50, 5% for p95 and 8% for p99, and τ_abs 0.05 ms for stages and 0.02 ms for systems, GPU passes and the scene pass.
   - **Jitter takes p99's threshold:** τ = max(8% × the baseline's per-run p99 mean, 0.05 ms), about 1.0 ms for `integrated`. Jitter carries p99's noise, and a τ relative to the small jitter itself would read `noisy` in every A/A. Jitter still catches tail growth that p99 alone misses when p50 moves the other way.
   - Peak RSS is judged per run with τ = max(2% × the baseline mean, 2 MiB).
   - `regressed`: the interval lies entirely above 0 and Δ > τ. `improved`: the interval lies entirely below 0 and Δ < −τ. `unchanged`: the interval lies within ±τ. `noisy`: anything else; rerun with more repeats or on a quieter machine.
   - Fewer than 3 runs on either side shows the deltas without a verdict.
4. **Owned metrics.** Only a row's owned metrics produce verdicts that count against it (see [`benchmarks.md`](benchmarks.md), "Ownership"); every other stage, system and pass is judged and reported for information.
5. **`compare.md`**, in order: a header with both sides, their commits, the machine and the comparability status; the owned-metric table (baseline, candidate, Δ, Δ%, interval, τ, verdict); the stages; the systems whose |Δ| > τ; the GPU passes from the diagnostic runs; memory and CPU (peak RSS with its verdict, RSS growth, CPU seconds); the workload counters; guards and validity. `compare.json` holds the same report as data.
6. **`compare.html`** is self-contained: inline CSS and SVG, no scripts, fonts or network requests, light and dark schemes. It shows an ECDF of the pooled `total` frames per side with p50/p95/p99 markers and 16.7 and 6.9 ms budget lines, the run-1 frame-time series, stacked stage bars, per-system bars (p50 with p95 whiskers), GPU-pass bars, peak-RSS bars and the counter table. Verdict badges carry text labels, not color alone.
7. **Suite compare.** Each row both suites hold gets its own report in `<out>/<row>/`; the suite-level `compare.json`, `compare.md` and `compare.html` add the verdict counts, `total` and peak-RSS bars per row and each row's owned table. Rows held by only one side are listed, not compared.
8. **A/A check.** Two captures, or two suites, of one build on one quiet machine must yield no `regressed` or `improved` verdict on any owned metric. Run one per machine before trusting verdicts. `noisy` verdicts are expected on sub-millisecond system rows that sit on the 0.02 ms floor and on the CPU render stages of the two `gpu` rows. Until `D-085` the scene pass also moved about 12% between captures; see the scene-pass note in [`benchmarks.md`](benchmarks.md#gpu). Two readings can break the rule with nothing changed: `gpu-throughput` `render_encode`, whose p95 takes one of two values per run (an A/A against a suite two hours older read its p50 `regressed`, 1.28 → 1.44 ms, on 2026-10-01), and the `ecs` rows of the per-run mode (`stats_decay`, `regen`, `follow`, `buffs`, `team_bags`, `accelerate`, `integrate`), where `buffs` read `regressed` in a five-run suite and `unchanged` with 15 runs a side in one sitting (`D-086`). An A/A of 2026-10-02, 73 minutes apart, read two of those rows `regressed` and two `improved` and nothing else: one suite held four mode runs of five and the other none.
9. **Exit codes.** 0 on success; `--fail-on regressed` exits 1 when any owned metric regressed, in a capture or any suite row (local scripting only, `D-070`); 2 on a bad request or environment problem; 3 from `run`, `suite` and `--sweep` when a capture is invalid.

## Capacity search

`just perf capacity (<bench>… | --all) [--budget 60hz|144hz|MS] [--stat p95] [--axis scale|KNOB] [--tolerance 0.05] [--frames 180]` finds, per tracked row, the largest scale whose statistic of `total` stays within a frame-time budget: 16.7 ms at 60hz, 6.9 ms at 144hz, or a millisecond value.

1. **Probes.** Each probe runs the row's preset at one axis value for the warm-up plus `--frames` measured frames, once, telemetry only, with no GPU diagnostic run. It passes when the statistic (p95 by default) is within the budget. A child that fails, misses frames or mismatches its config fails the probe with the reason recorded; guard failures are recorded per probe and listed, but don't fail it.
2. **Axis.** `scale` multiplies the row's scaled key knobs; its bounds are the tightest `min/value` and `max/value` over them, and a key knob whose range starts at 0 floors the axis at its first nonzero value. `--axis KNOB` searches one numeric knob instead.
3. **Bracketing.** Start at scale 1. Double on a pass until the first fail or the upper bound; halve on a fail until the first pass or the lower bound.
4. **Bisection.** Bisect geometrically between the last pass and the first fail until their ratio is at most 1 + tolerance, or until the resolved key counts stop changing.
5. **Confirmation.** Rerun the best pass 3 times; it holds when their median is within the budget. Otherwise step down one tolerance step and confirm again, at most 3 times.
6. **Report.** Per row: the budget and the maximum scale, or "≥ max (bound reached)" or "< min (budget not reachable)"; the key counts at that scale; p95 and p99; the limiting stage, meaning the largest mean among the top system, `flush`, `particles`, `extract`, `render_encode`, the present wait (`render_acquire` + `render_submit_present`) and `unattributed`, flagged ✓ or ✗ against the row's declared bottleneck; peak RSS; the probe count and the wall time. Medians come from the confirmation probes, or from the last probe for an unreachable budget. It is written to `capacity.md`, `capacity.json` and `capacity.html` (the statistic against scale on a log axis, with the budget line).

Results are machine-specific and informational: a lower capacity is a finding to explain, not a verdict. A ✗ says the row hit a different wall than it was built to load, as `particles` (limited by the default extract) and `integrated` at 144hz (limited by its fixed post chain) do on the reference machine. A full `--all` search takes about 6 minutes at 60hz and 3 at 144hz there.

## Memory

- **Peak RSS** comes from `ru_maxrss` through `os.wait4`, for every run, with no engine change. The timing run's value is the one reported, because the diagnostic run allocates query buffers. It includes driver-mapped memory, so compare it only on one machine and driver. rusage also gives user and system CPU seconds, faults and context switches.
- **RSS growth** is the least-squares slope, in KiB/s, of `/proc/<pid>/statm` samples taken every 100 ms, fitted over the second half of the run after dropping samples within 0.2 s of the last one (the child frees memory while it shuts down). It is reported for every row and judged for none: the leak threshold for `churn` is an open proposal ([`benchmarks.md`](benchmarks.md), "Open proposals"). One 4 KiB page over a short run reads as a few KiB/s. Until `D-085` the rows that rewrite text every frame (`gpu`, `integrated`) grew by MiB/s while the text cache filled, so their peak RSS depended on capture length. The layout cache is bounded now: `gpu` reads no growth and the same peak at 300 and 900 frames.
- **Allocation counting** isn't measured (gap M1).

## Tracked rows, suites and regression policy

| Row | Command | Judge on (owned) | Guards |
| --- | --- | --- | --- |
| `physics` | `just perf run physics --repeat 5` | `physics_step` p50/p95, `update` p95 | `physics.sleeping <= 0`; `bench.teleports >= 1` |
| `physics-sparse` | `just perf run physics --preset sparse --repeat 5` | `physics_step` p50/p95 | `physics.sleeping <= 0` |
| `ecs` | `just perf run ecs --repeat 5` | `update` p50/p95; the 14 system rows at p50 | `bench.structural <= 0`; `bench.entities constant` |
| `churn` | `just perf run churn --repeat 5` | `flush` p50/p95; `churn_scan`, `churn_despawn`, `churn_toggle`, `churn_spawn` at p50. RSS growth is reported only | `bench.population constant`; `bench.spawned == bench.despawned` |
| `gpu` | `just perf run gpu --repeat 5` | Scene pass (`gpu`) and `render_span` p50/p95; the bloom threshold and composite, vignette, SMAA and text passes at p50; `extract`, `render_encode` p50/p95 | None; the GPU rows are n/a without timestamp queries and the capture stays valid |
| `gpu-throughput` | `just perf run gpu --preset throughput --repeat 5` | `extract`, `render_encode` p50/p95 | None |
| `particles` | `just perf run particles --repeat 5` | `unattributed` p50/p95 (the untimed particle stage), `animate_sprites` p50/p95 | `bench.live within ±10% of its median` |
| `integrated` | `just perf run integrated --repeat 5` | `total` p50/p95/p99, jitter (p99 − p50) | `bench.view_out <= 0` |
| capacity | `just perf capacity --all --budget 60hz` and `--budget 144hz` | Maximum scale per row (informational) | — |

Every row also reports peak RSS, with a verdict, and RSS growth. `interval` and, per run, the largest `total` and the spike count (frames above 1.5 × the run's p50) are reported too; no row owns them.

**Suite runs.** `just perf suite [--preset P] [--scale S] [--only ROWS] [--repeat N] [--compare BASELINE]` captures every tracked row `describe` lists, in its order, with the same flags and 300 measured frames each. It resolves every row's configuration before the first run, writes each capture to `perf-runs/<UTC>-suite…/<row>/`, then `suite.json` and the suite README, and exits 3 when any row is invalid. `--only physics,ecs` narrows it. `--repeat 5` takes about 6 minutes on the reference machine.

**The `--preset` rule.** `--preset P` replaces the preset only of the rows whose own preset is `default`. `physics-sparse` and `gpu-throughput` keep `sparse` and `throughput`, because replacing their preset would measure another row. Before anything runs, the suite checks that P exists for every affected benchmark and that each resolved configuration still measures its row. `just perf suite --preset min` is a quick functional pass; `suite.json` records the request, and the README states the rule. Its `physics` row fails the per-frame teleport guard at `min` (about 3 teleports per frame, see [`benchmarks.md`](benchmarks.md#physics)), so that suite reads invalid and exits 3 even when every other row is valid.

**Regression policy.**

- A `regressed` verdict on an owned metric needs a fix, or a justification in `DECISIONS.md` or the plan that accepts it.
- A `regressed` on a row whose code the change did not touch can be code placement (the `ecs` system rows and `churn`'s `flush`; see [`benchmarks.md`](benchmarks.md#ecs)). It still needs its justification: the same verdict from the baseline tree plus a function that no frame calls, which puts the row's code at the same alignment, with both compares recorded.
- `improved` on an owned metric is the evidence an optimization claims; quote the compare report.
- `noisy` needs more repeats or a quieter machine, not a threshold change. The thresholds, the variance floor and the owned metrics change only by decision.
- Non-owned metrics are context: a regression there belongs to the row that owns the cost.
- A change to a benchmark's work bumps its `workload_version`; captures across versions aren't comparable, so the baseline restarts.

**Budgets.** The capacity budgets, 60 Hz (16.7 ms) and 144 Hz (6.9 ms) p95 of `total`, are the only budgets. Benchmarks load their owned stage on purpose, so no stage has a fixed limit.

## Profiling

- **`--profile`.** `just perf run <bench> --profile [--call-graph dwarf|fp] [--sample-frequency HZ]` runs the binary twice more after the timing runs, with the same configuration and only error logging: once under `perf stat -d` and once under `perf record` (DWARF stacks by default), then folds a flamegraph from that recording (`flamegraph --perfdata`), all into `profile/`. A missing `perf` or `flamegraph` is noted in `capture.json`. Profiler runs never feed the statistics.
- **Stacks.** `--call-graph fp` gives smaller records but needs `-C force-frame-pointers=yes` in the effective flags (the runner refuses it otherwise); system libraries may still lack frame pointers, so use DWARF when stacks truncate or attribution is ambiguous. `--sample-frequency 499` lowers the sample count; both are recorded. Profiles locate costs; the timing runs judge them.
- **By hand.** Build once, then profile the binary directly so Cargo isn't sampled. `perf record` and `cargo flamegraph` write `perf.data` into the current directory, which must be the repository root, so pass `-o`. `cargo flamegraph` with different `RUSTFLAGS` rebuilds `target/release`.

  ```bash
  RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p example-02-bench
  mkdir -p perf-runs/manual
  TUNGSTEN_BENCH=churn TUNGSTEN_SMOKE_FRAMES=360 WGPU_BACKEND=vulkan \
    perf stat -d -- target/release/example-02-bench
  TUNGSTEN_BENCH=churn TUNGSTEN_SMOKE_FRAMES=360 WGPU_BACKEND=vulkan \
    perf record -o perf-runs/manual/perf.data --call-graph dwarf -- target/release/example-02-bench
  perf report -i perf-runs/manual/perf.data
  ```

  `samply record` is an interactive alternative to `perf record` with the same environment; keep the window at the benchmark's warm-up plus 300 frames.
- **GPU timing is diagnosis only.** `TUNGSTEN_GPU_TIMING=1` forces a blocking `device.poll(wait_indefinitely())` readback every frame, which inflates CPU-side timings. That is why GPU metrics come from the separate diagnostic run. Never enable it for a timing run or a profile.

## Hotspots

Search these first in a flamegraph:

- `App::window_event`, `render_frame_full`, `render_frame_full_timed`
- `extract_`, `extract_sprites_default` (no culling; one pass, a key sort that is skipped when the query is already in painter order, and a string compare per sprite, `D-086`)
- `query2`, `World::flush` (archetype moves; the app's flush shows as `World::flush_reusing`, with `insert_run` and `move_components_to` under it)
- `physics_step`, `build_pairs`, `gather_tilemap_proxies` (tile proxies rebuilt from a full-map scan every frame)
- `particle_tick_system`
- `glyphon` and the text `prepare` (`TextPipeline::prepare`: `TextLayoutCache::update` reshapes every changed section, and a frame of unchanged text skips glyphon's prepare)
- `wgpu`

Interpretation:

- Cross-reference a hot flamegraph region with the stage and system rows of the same configuration.
- A hot render stack with a low `render` stage is sampling noise; a hot stage with a matching telemetry change is real.
- When `render` is high, classify it: `render_acquire` is swapchain pacing, `render_encode` is CPU command generation, `render_submit_present` is the present or readback wait. A GPU-bound frame waits in the acquire, not in the present: `gpu` spends 7.5 of its 10.7 ms in `render_acquire` and 0.3 ms in `render_submit_present` (after `D-087`). Capacity search's "present" is the sum of the two.
- Near-zero baselines make percentages meaningless; compare absolute values, as τ_abs does.

## Frame pacing

A concrete `display.present_mode` (`immediate`, `mailbox` or `fifo`) is final; `"auto"` lets `display.vsync` choose the family. The checked-in defaults are `display.present_mode = "auto"`, `display.vsync = false` and `display.max_frame_latency = 1`, which resolve to the engine's auto no-vsync family (`immediate` first). `max_frame_latency` is the requested `wgpu` hint, not a backend-confirmed queue depth.

Legacy `window.vsync`, `render.present_mode` and `render.max_frame_latency` only fill in what `display.*` leaves unset: a display field wins whenever it is set, `"auto"` included (`D-043`). The checked-in `tungsten.json` sets both display pacing fields, so `TUNGSTEN_RENDER_PRESENT_MODE` and `TUNGSTEN_RENDER_MAX_FRAME_LATENCY` change nothing there. `TUNGSTEN_DISPLAY_PRESENT_MODE` and `TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY` override the display fields themselves.

For pacing studies, keep the default captures and add override captures:

```bash
just perf run gpu --repeat 5
just perf run gpu --repeat 5 --present-mode mailbox --max-frame-latency 2
```

`--present-mode` and `--max-frame-latency` set child-only `TUNGSTEN_DISPLAY_PRESENT_MODE` and `TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY`, so `tungsten.json` stays unchanged, and they suffix the directory with `-<mode>-lat<N>`. Each run's `backend:` line must confirm the request, or the capture is invalid (see "Validity and determinism"). Present mode and frame latency are hard compare fields. Acquire and present pacing belong to the environment: they are reported and owned by no row. Read the frame period from `interval`, not from `total`. A `fifo` frame blocks inside `total`, so both read the refresh period (`gpu` at `min` under `fifo` / 2: 16.63 and 16.67 ms at p50); a wait between two frames shows in `interval` alone. A frame-rate cap (`display.frame_rate_cap`) is such a wait: at a cap of 60, `gpu` at `min` reads 0.79 ms of `total` and 16.71 ms of `interval` at p50 (p99 17.06 ms, 59.8 FPS), where `total` alone would give 1,191 FPS. The April 2026 Vulkan matrix that keeps `Immediate / 1` as the shipped default is recorded in `D-078`; shipped pacing defaults change only by decision.

The same matrix on the current suite, for the record (2026-10-02, the reference machine at 60 Hz, frame latency 1, three timing runs a cell, medians in ms):

| Row | Mode | `total` p50 / p95 / p99 | `interval` p50 | FPS (1000 / mean `interval`) |
| --- | --- | --- | --- | --- |
| `gpu` | `immediate` | 10.80 / 12.23 / 12.70 | 10.86 | 89.4 |
| `gpu` | `mailbox` | 11.76 / 17.02 / 17.32 | 11.84 | 89.7 |
| `gpu` | `fifo` | 16.63 / 17.36 / 17.72 | 16.68 | 60.0 |
| `integrated` | `immediate` | 8.48 / 8.99 / 9.35 | 8.62 | 116.9 |
| `integrated` | `mailbox` | 8.43 / 9.06 / 9.28 | 8.54 | 117.9 |
| `integrated` | `fifo` | 16.51 / 17.06 / 17.27 | 16.69 | 60.0 |
| `gpu` at `min` | `immediate` | 3.04 / 4.69 / 4.77 | 3.08 | 294.2 |
| `gpu` at `min` | `mailbox` | 2.90 / 4.74 / 5.03 | 2.94 | 321.2 |
| `gpu` at `min` | `fifo` | 16.62 / 17.13 / 17.46 | 16.66 | 60.0 |

`mailbox` gives the GPU-bound `gpu` row the frame rate of `immediate` with a wider spread (jitter 5.51 ms against 1.89) and gains 9% on `gpu` at `min`; the CPU-bound `integrated` row moves by under 1%.

## Backends and RenderDoc

| `WGPU_BACKEND` | Typical platform | `TIMESTAMP_QUERY` |
| --- | --- | --- |
| `vulkan` | Linux | Best chance of GPU pass timings |
| `dx12` | Windows | Often available on modern hardware |
| `metal` | macOS | Backend-dependent; verify per machine |
| `gl` | Fallback | May be unavailable or noisy |
| `auto` | Any | Convenient, but less reproducible |

The `backend:` line is the source of truth for the backend, adapter, chosen present mode and requested frame latency. Without timestamp queries the GPU rows are n/a and captures stay valid.

RenderDoc on Linux with Vulkan:

1. Launch RenderDoc and set the executable to `target/release/example-02-bench`, with the working directory at the repository root.
2. Set the environment: `WGPU_BACKEND=vulkan`, plus `TUNGSTEN_BENCH` and `TUNGSTEN_BENCH_PRESET` for the benchmark to inspect.
3. Start the capture and trigger a frame after the warm-up.
4. Inspect the scene pass for draw calls, texture bindings and pass duration, and compare the batch count with `bench-config`'s derived value where the benchmark logs one.

## Criterion micro-benchmarks

Criterion (`D-037`) covers isolated primitives: `ecs_bench`, `physics_bench`, `action_map_bench` and `tween_tick` in `tungsten-core`, `render_bench` in `tungsten-render` and `particle_tick` in `tungsten`. They are regression detectors, not throughput claims. `just bench-build` compiles every bench without running it; run one with `cargo bench -p tungsten-core --bench physics_bench`. Benches build with `.cargo/config.toml`'s `target-cpu=native` flags, so their numbers never compare with capture numbers.
