# Profiling Workflow

The capture contract for Tungsten performance work: how to run the rows, what a valid capture is, how two captures compare and how a sitting judges a change (`D-078` as `D-226` amends it). [`benchmarks.md`](benchmarks.md) describes the rows, their knobs, owned metrics and guards, the counter guards and the levers for 9.2. `just perf <command>` runs `scripts/perf.py`, a standard-library Python 3.12 runner; `scripts/perf_stats.py` holds the parsing, statistics and verdicts, `scripts/perf-rows.json` the rows, and `just perf-test` runs `scripts/test-perf.py`.

Read the [capture rules](#comparison-rule-and-capture-rules) before measuring, then only the procedure needed: [quick start](#quick-start), [sittings](#sittings), [compare](#compare), [done-when checks](#writing-done-when-checks), [profiling](#profiling), [pacing](#frame-pacing) or the [telemetry format](#telemetry-lines).

## Comparison rule and capture rules

Compare two captures only when they measure the same row with the same workload (version, every bench knob, the script's hash, resolution, warm-up and frames), the same build flags, the same backend, adapter, present mode and frame latency, and the same machine. Compare reads every one of these from the captures and suppresses every verdict on a mismatch. Judge on runs, not frames: a verdict needs at least 3 runs a side. A change is judged in a [sitting](#sittings), which interleaves the base and the candidate with an A/A of the base.

| Setting | Value |
| --- | --- |
| Build | `--release --locked` with `RUSTFLAGS="-C force-frame-pointers=yes -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6"`: every function and non-fallthrough block starts on a 64-byte boundary, on both sides, so code placement moves no row. `--plain` drops the alignment for a comparison at shipping flags. `RUSTFLAGS` replaces `.cargo/config.toml`'s `target-cpu=native`, so captures are generic x86-64. Each tree builds into its own folder under `target/perf/` |
| Backend | `WGPU_BACKEND=vulkan` on Linux |
| Resolution | the row's: 1920×1080, the area rows 3840×2160 |
| Present | the checked-in `tungsten.json`'s `display` pacing; `immediate` / 1 on the reference machine |
| Frames | the row's warm-up, setup frames included, then 300 measured frames |
| Repeats | the row's (5, the area rows 3) |
| GPU timing | only in a GPU run, beside each timing run or alone, never in a timing run |
| Machine | governor `performance`; nothing else using the CPU, the GPU or the screen |

**No remote-desktop encoder during a capture.** A connected NoMachine client runs `nxcodec.bin`, which encodes the screen and competes for memory bandwidth with the integrated GPU: it slowed every third frame of `ecs`, `total` p95 11.5 → 18.2 ms, while every guard passed and the digests matched. `run` refuses it, and `cargo` or `rustc` outside the runner's own process tree, before building (`D-141`); a sitting's start check refuses more (below). Every measured run is scanned before, once a second during and after, and a run it overlaps is invalid.

**Nothing else may load the machine.** Guards and digests pass under this load too:

- **No other agent session at work, no other build.** Another session's `cargo` run landed inside a suite and read `particles` p95 41.9 ms against 12.5. An idle session passes a sitting's start check; a working one shows as a busy process. A commit or dirty-tree change during a run invalidates it.
- **None of your own commands.** Searches and edits beside a capture fattened `churn` `flush` p95 and jitter with p50 unmoved. Run each sitting as one blocking foreground command, or as one background job with nothing beside it.
- **No busy editor.** After a large `just clean-target --apply`, VS Code's file watcher sat at a full core for hours, which the runner's name scan cannot see; the start check's busy-process sample does.
- **The compositor.** KWin adds about 1.45 ms to one frame per refresh in the acquire, which sets the p95 and jitter of GPU-waiting rows (`gpu` at `min`). It is the environment's, and no row owns it.

## Quick start

From the repository root:

```bash
export WGPU_BACKEND=vulkan
just perf list                                     # the rows
just perf run ecs                                  # one capture: perf-runs/<UTC>-ecs/
just perf run e01-areas --repeat 1                 # a group: one capture per row
just perf sitting --quick                          # the tree against the last tag, CPU rows, informational
just perf sitting --rows physics,ecs               # a full sitting of two rows
just perf compare <capture> <capture>              # verdicts, printed
just perf baseline save <sitting-or-capture> <name>
just perf counters                                 # the GPU counter pins
just perf-test                                     # runner tests, no GPU or cargo
```

## Rows and launch

A row lives in `scripts/perf-rows.json`. A bench row runs `example-02-bench` from the repository root with `TUNGSTEN_BENCH`, `TUNGSTEN_BENCH_PRESET` (and `TUNGSTEN_BENCH_SCALE` when the row sets `scale`) and `TUNGSTEN_SMOKE_FRAMES` at warm-up plus frames, which pins dt; its `bench-config:` line must name the row and its preset. A scripted row runs its game's build from the game's folder in script mode (`D-225`) on a script the runner composes: the row's steps, any `until: "end"` held through the last frame, and the row's asserts on the window's last frame; `TUNGSTEN_SCRIPT`, `TUNGSTEN_REPORT` and a fresh `TUNGSTEN_USER_DIR` per run, which is deleted after it. Script mode pins dt at 60 Hz. `run` flags: `--repeat N`, `--frames N`, `--plain` and `--allow-background`.

**Child environment.** Every run clears the inherited `TUNGSTEN_BENCH*`, `TUNGSTEN_RENDER_*`, `TUNGSTEN_DISPLAY_*`, `TUNGSTEN_CAPTURE_*`, `TUNGSTEN_GPU_TIMING`, `TUNGSTEN_OVERLAYS_ON`, `TUNGSTEN_SMOKE_FRAMES`, `TUNGSTEN_PERF_LOG`, `RUST_LOG` and the script handshake variables, then sets `TUNGSTEN_PERF_LOG=1` and `RUST_LOG=tungsten::app=debug,bench=debug`; a GPU run adds `TUNGSTEN_GPU_TIMING=1` and five frames past the window, so the window's last timestamps arrive.

The bench binary's own variables, for reading a row by hand:

| Variable | Meaning |
| --- | --- |
| `TUNGSTEN_BENCH` | `physics` (default), `ecs`, `churn`, `gpu` or `particles` |
| `TUNGSTEN_BENCH_PRESET` | Named knob set; default `default` |
| `TUNGSTEN_BENCH_SCALE` | Multiplier on the preset's scalable knobs |
| `TUNGSTEN_BENCH_SET` | `knob=value,…` overrides, applied last |
| `TUNGSTEN_BENCH_DESCRIBE` | `1` prints the knobs and presets as JSON; `config` the resolved configuration |

## Captures

```text
perf-runs/<UTC>-<row>/
  capture.json         schema 2 (below)
  README.md            validity, provenance and each owned metric per run
  run-N/timing.log     run-N/gpu.log, and a scripted row's script and report per run
perf-runs/<UTC>-sitting[-quick]/
  sitting.json, README.md, captures/<row>-<A|B><n>/
perf-runs/<UTC>-counters/<row>/timing.log
perf-runs/baselines/<name>/   a copied capture or sitting
```

```text
{schema:2, kind:"capture", row, sitting, tree{commit,dirty,ref}, build{flags,package},
 workload{version, config, script_sha256, resolution, warmup, frames, tail},
 renderer{backend, adapter, present_mode, max_frame_latency}, machine{fingerprint, …},
 owned, valid, reasons, notes,
 runs[{valid, reasons, digest, rss_peak_kib, cpu{stages,systems,counters}, gpu{stages,passes}|null}]}
```

- Each metric is `{p50, p95, p99, mean}` over the window, nearest-rank percentiles. `cpu.counters` holds the `counters:` fields, and the `bench:` and `physics:` fields as `bench.<name>` and `physics.<name>`. `gpu.passes` comes from the `gpu_passes:` lines by the frame each names; `gpu.stages` is the GPU run's own `frame:` stages.
- **Metric names** in the registry: `stage.<frame field>`, `system.<name>`, `counter.<name>` (the timing run), `gpu.<pass>` (the GPU run's passes, `gpu.scene` the scene pass) and `gpustage.<frame field>`.
- **Provenance.** `tree` is the measured tree's own commit and dirty state (`no`, or `yes (diff <12 hex>)` over the tracked diff and the untracked files), or for a base export the ref's commit. `machine` holds a fingerprint of the hostname, CPU and memory, which compare holds equal, and the governor, platform profile, AC state, kernel, `rustc` and `WGPU_BACKEND`.
- **`workload.config`** is the bench's resolved knobs from `bench-config`, null for a scripted row, which its script's hash pins instead. A registry edit after a capture therefore cannot make two captures look alike.
- **Baselines.** `baseline save <capture-or-sitting> <name>` copies it without logs; `baseline list` and `baseline rm <name>` manage them. A baseline name works wherever a capture path does. Schema-1 captures from the old runner are refused.

## Telemetry lines

`TUNGSTEN_PERF_LOG=1` with `RUST_LOG=tungsten::app=debug,bench=debug` logs one group per frame. The parser strips the `env_logger` prefix and attaches each companion line to the preceding `frame:` line, except a `gpu_passes: from=N` line, which attaches to frame N, the N-th `frame:` line; missing companions are tolerated. System and pass names are literal keys.

```text
backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: immediate max_frame_latency: 1 timestamp_query: true
bench-config: {"bench":"gpu","derived":{…},"knobs":{…},"preset":"default","row":"gpu","scale":1.0,"seed":1,"workload_version":1}
frame: total=10.75ms interval=10.81ms update=0.03ms flush=0.00ms extract=0.65ms render=9.92ms render_acquire=7.53ms render_encode=2.12ms render_encode_pre_acquire=2.07ms render_submit_present=0.27ms gpu=6.88ms audio=0.00ms hot_reload=0.00ms
counters: draw_calls=76 upload_bytes=1664304 text_renders=1 sprite_batches=60
systems: physics_debug_toggle=0.00ms … bench_counters=0.00ms gpu_camera=0.00ms gpu_lights=0.00ms gpu_text=0.02ms
gpu_passes: from=57 scene=6.88ms post0_bloom_threshold=0.16ms … text=0.07ms render_span=10.39ms
bench: sprites=2000 glow=20000 visible_tiles=12769 glyphs_changed=4000 lights=16
```

- **`backend:`** once at renderer startup: backend, adapter, chosen present mode, the requested frame-latency hint and timestamp-query support. Compare reads them as hard fields.
- **`frame:`** from `tungsten::FrameTimings`, once per frame. `render` contains acquire, encode and submit/present; `render_encode_pre_acquire` is the part recorded before the acquire (`D-145`). `unattributed` is derived: `total` minus `update`, `flush`, `hot_reload`, `extract`, `render` and `audio`. `interval` is the time from the previous frame's start to this one's, `n/a` on the first frame: the frame's real period, so FPS is 1000 / mean `interval`; no row owns it. `gpu=` is the newest scene pass read back, for the HUD; the runner takes no GPU metric from it.
- **`counters:`** right after `frame:`, the renderer's own counts of the frame: `draw_calls` and `upload_bytes` (buffer and texture writes, buffers created with contents) as the engine records them, apart from glyphon's draws and uploads inside its `prepare` and `render`; `text_renders`, glyphon's `render` calls; `sprite_batches`, the extract's sprite batches. They are `RenderCounts`' fields, zero headless, and stay out of the digest.
- **`systems:`** every system's wall time in run order across the stages (`FrameTimings::system_timings`, `D-128`); a `fixed_update` system's runs are summed over the frame's steps (`D-137`). Whitespace and `=` in names become `_`.
- **`gpu_passes: from=N`** only under `TUNGSTEN_GPU_TIMING=1` on an adapter with timestamp queries, one line per timed frame read back, naming frame N it measured. The renderer never waits for timestamps: a frame resolves them into one of a ring of four readbacks and a later frame's non-blocking poll reads them, so a line arrives up to four frames after its frame, two can arrive in one frame and a frame that reads none prints none. A frame that finds every slot in flight runs untimed. The line lists every render pass in execution order (scene; indexed post slots such as `post0_bloom_threshold`; each bloom mip and the composite; `smaa_edges`, `smaa_blend_weights`, `smaa_neighborhood`; `text`, the overlay pass) plus `render_span`, from the first scene timestamp to the end of the overlay, gaps included. Only a capture frame (`TUNGSTEN_CAPTURE_FRAME`) has a `present` blit. A stack past the adapter's query-set limit renders without timings.
- **`physics:`** when physics runs: proxies and dynamic bodies gathered by the last step, sleeping bodies, and the final substep's pairs and contacts.
- **`bench-config:`** once: the bench's resolved configuration as JSON, with derived values such as the expected batch count.
- **`bench:`** the bench's counters. Frame N + 1's first system logs frame N's line, so it follows frame N's group; the last frame of a run without tail frames has none. A hex token such as `ecs`'s `digest=0x…` stays out of the statistics.

To read the lines without the runner:

```bash
TUNGSTEN_BENCH=ecs TUNGSTEN_SMOKE_FRAMES=360 TUNGSTEN_PERF_LOG=1 RUST_LOG=tungsten::app=debug,bench=debug \
  WGPU_BACKEND=vulkan cargo run --release -p example-02-bench
```

## Validity and determinism

A capture is valid when every run, GPU runs included:

- exits 0 within its deadline, 30 s plus 50 ms a frame (warm-up, window and tail), and logs every measured frame after the warm-up; a run past its deadline is killed, so a base that predates a row's variables (0.112 has no script mode) fails its first run and the row reads `new`;
- passes the row's guards (checked on every window frame; `bench:` guards skip the last, whose line prints in the next frame);
- for a bench row, logs a `bench-config:` line naming the row and its preset; for a scripted row, writes a passing script report;
- in a GPU run on an adapter with timestamp queries, has `gpu_passes:` samples for at least 95 % of its window frames, ring-full skips counted;
- saw no background load: no encoder and no foreign build in its scans, and the working tree's commit and dirty state unchanged (`--allow-background` makes these notes, `D-095`);
- shares its digest with every other run of the capture.

The digest is a SHA-256, shortened to 16 hex digits, over the window's `physics:` lines and the `bench:` lines of every window frame but the last. A frame's `bench:` line prints in the next frame, so the last one exists only when tail frames follow; leaving it out, and the tail frames' own lines, makes a GPU run with tails hash as a timing run without them. One build under the pinned dt replays one workload, so runs must match. Compare reports whether two captures' first-run digests match, for information only: an engine change moves them legitimately; a change that must keep behaviour shows there that it did.

An invalid capture stays on disk with `valid: false` and its reasons; `run` exits 3, and compare suppresses its verdicts.

## Compare

`just perf compare <baseline> <candidate>` takes two captures, each a directory, a `capture.json` or a baseline name, and judges the candidate's owned metrics from the registry (or the capture's own list for a row the registry no longer has).

1. **Comparability.** Every hard field above suppresses every verdict on a mismatch, and so does an invalid side.
2. **The run is the unit.** Frames within a run are autocorrelated and share clocks, thermals and memory placement, so each run contributes its p50, p95, p99 and mean per metric.
3. **Verdict per metric and statistic** (`D-078`):
   - Δ is the mean of the candidate's per-run values minus the baseline's, and its 95 % Welch interval floors each side's variance at (0.5 % of its mean)², the t quantile from a built-in table.
   - τ = max(τ_rel × the baseline mean, τ_abs), with τ_rel 3 % for mean and p50, 5 % for p95 and 8 % for p99, and τ_abs 0.05 ms for stages and 0.02 ms for systems and GPU passes.
   - `regressed`: the interval lies above 0 and Δ > τ. `improved`: below 0 and Δ < −τ. `unchanged`: within ±τ. `noisy`: anything else. Fewer than 3 runs a side shows the deltas without a verdict.
4. **Peak RSS** (`ru_maxrss` per run) is judged, at τ = max(2 %, 2 MiB), only between captures of one sitting. Across sittings both values print without a verdict: with transparent huge pages one build read 62 MiB more in every row an hour later.
5. **Budgets.** A row's budget (`{metric, stat, ms}`) reads `within` when the candidate's 95 % interval lies under it, `over` when above, else `near`. No row has a budget yet; 9.1 sets them.

## Sittings

`just perf sitting [--base REF] [--rows LIST] [--quick] [--cpu-only] [--wait-quiet S] [--allow-background] [--plain] [--keep]` judges the working tree (B, the candidate) against `REF` (A, the base), by default the last tag reachable from `HEAD`. The candidate's runner and registry drive both sides.

1. **Start check.** The sitting refuses, exit 2, on the encoder or a build outside the runner; on a process other than the runner and its ancestors above 20 % of a core over a 5 s sample; or on the tree's dirty state changing meanwhile. It records the 1-minute load, the governor, AC power and the five busiest processes. `--wait-quiet S` polls every 10 s until 60 s run quiet, giving up at S. An idle agent session passes; a working one shows as busy. `--allow-background` starts despite a refusal and records it.
2. **Base export.** `git archive REF` into `target/perf/base-tree/`, deleted at the end unless `--keep`; both trees build before anything is measured, the base into `target/perf/base-aligned/`. A package the base cannot build makes its rows `new`.
3. **Interleaving.** Per row, captures run A1 B1 A2 B2, each at the row's repeats and sharing the sitting's id. A1↔A2 is the A/A; A1→B1 and A2→B2 are the direct pairs. A capture that only background load invalidated is taken again once, after a wait for quiet.
4. **Verdict.** An owned metric is `regressed` when both direct pairs read it `regressed` and the A/A reads it `unchanged` or `noisy`.
5. **One rerun.** A row runs A3 B3 A4 B4, replacing the first set, when its A/A reads an owned metric `regressed` or `improved`, or when one of its known-drift metrics reads `regressed` in a direct pair. The rerun stands alone: with a stable A/A, a metric `regressed` in both of its pairs reads `regressed`, a known-drift one included; a rerun whose own A/A fails reads `unresolved`.
6. **Row verdicts:** `ok`, `regressed` (with the metrics), `unresolved`, `invalid`, or `new` (a row the base cannot build or run while the candidate can, measured on the candidate alone).
7. **Output.** `perf-runs/<UTC>-sitting[-quick]/sitting.json` (`{schema:2, kind:"sitting", id, base{ref,commit}, cand{commit,dirty}, quick, wall_s, wait_s, start, rows[{name, verdict, regressed[], reran, aa, pairs, budgets}]}`; `wall_s` runs from the end of the start check, `wait_s` is the wait for quiet before it), a README table and the captures under `captures/`. Exit 0, 1 on a `regressed` or `unresolved` row, 3 on an `invalid` one.
8. **Quick** (`--quick`): CPU rows only, A1 B1 A2 at 3 repeats and 150 frames, no rerun, always exit 0. Informational: never a release's evidence.
9. **`--cpu-only`** skips GPU runs and GPU-only rows, for a base whose GPU timing differs in kind.

## Counter guards and GPU counters

The CPU counter guards are ordinary tests in `just check` (`D-070`'s informational CI included): see [`benchmarks.md`](benchmarks.md#counter-guards). `just perf counters [--write]` checks or writes the GPU counter pins locally, beside `just smoke` and `just visual`, for render and extract changes.

## Memory

- **Peak RSS** comes from `ru_maxrss` through `os.wait4`, for every run, and includes driver-mapped memory: compare it on one machine and driver, within one sitting.
- **Allocations** are counted by the counter guards, headless, on the test's thread.

## Regression policy

- A `regressed` owned metric needs a fix, or a justification in `DECISIONS.md` or the change's plan that accepts it.
- `improved` on an owned metric is the evidence an optimization claims: quote the sitting.
- `noisy` needs more repeats or a quieter machine, not a threshold change. The thresholds, the variance floor and the owned metrics change only by decision.
- Non-owned metrics are context: a regression there belongs to the row that owns the cost.
- A change to a row's work bumps its `workload_version`; captures across versions are not comparable.
- Aligned builds keep code placement out of a verdict. When a row whose frames run the changed code still reads `regressed`, the control is a placebo: the candidate built with its change switched off, its addresses checked with `nm` ([profiling](#profiling)); base → placebo is placement, placebo → candidate is the change.

## Writing done-when checks

- **The counter guards first** (`D-226`). A plan's perf checks are its counter guards: a step that adds a system or changes a guarded one names the guard and states whether its pin may move and why. No plan takes a sitting of its own, except one that claims a speed-up, measures a 7.14, 7.15 or 9.2 item or changes a row's workload, which quotes a full sitting's `sitting.json`; a hot-path change may quote a quick sitting as information. A minor or major release sits against the previous tag before its cut.
- **Ask for "not `regressed`".** An effect just under τ can't read `improved` and seldom `unchanged`: `ecs` `update` p50 read −0.26, −0.30 and −0.28 ms against τ 0.32, `noisy` each time (`D-083`). Ask for `improved` only for moves well past τ; in the dead zone, report the deltas and intervals.
- **Sit every row a shared change can move.** An ECS step captured on `ecs` alone once hid `churn` `flush` 2.68 → 3.1 ms until the final suite; a sitting of a shared ECS or frame-loop change takes all the CPU rows, its default.
- **Estimate cost on the judged rows.** The arrival pass was noise on a settling pile and +8.3 % `physics_step` in `physics`, which lists about 195 bodies per substep (`D-092`). Count the new work's triggers per frame in those rows and price them against τ before writing the check.
- **Judge relocated work on frame time.** Work moved between passes moves the pass verdicts and `render_span`: `smaa_neighborhood` read `regressed` while `gpu` `total` fell (`D-087`). Name the pass that absorbs the cost, and judge `total` and GPU engine time per frame (`drm-engine-*` in `/proc/<pid>/fdinfo/`).

## Profiling

- **By hand.** Build once with the runner's flags, then profile the binary directly so Cargo isn't sampled. `perf record` and `cargo flamegraph` write `perf.data` into the current directory, so pass `-o`.

  ```bash
  RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p example-02-bench
  mkdir -p perf-runs/manual
  TUNGSTEN_BENCH=churn TUNGSTEN_SMOKE_FRAMES=360 WGPU_BACKEND=vulkan \
    perf stat -d -- target/release/example-02-bench
  TUNGSTEN_BENCH=churn TUNGSTEN_SMOKE_FRAMES=360 WGPU_BACKEND=vulkan \
    perf record -o perf-runs/manual/perf.data --call-graph dwarf -- target/release/example-02-bench
  perf report -i perf-runs/manual/perf.data
  ```

  `--call-graph fp` gives smaller records and needs `-C force-frame-pointers=yes`; system libraries may lack frame pointers, so use DWARF when stacks truncate. `samply record` is an interactive alternative with the same environment; `flamegraph --perfdata` folds a recording. Profiles locate costs; captures judge them. A game row profiles the same way from its folder with the script's variables.
- **GPU timing is diagnosis.** `TUNGSTEN_GPU_TIMING=1` no longer stalls a frame, but it resolves and copies queries every frame, so the runner keeps it out of timing runs.
- **Confirm the rebuild after restoring files.** Cargo judges freshness by mtime, so files restored with `cp -p` (or `tar -x`, `rsync -t`) look older than the last build. Restore with plain `cp` or `touch` them; the build must print `Compiling` for each restored crate.
- **Code placement.** Before aligned builds, rows the change never touched moved with their functions' addresses modulo 64 (`D-085`, `D-087`). To check a layout, diff `nm -C --defined-only` of the release builds.
- **A second caller can de-inline a hot function.** A second call site made rustc outline `narrow_phase` from the pair loop, about 5.6 % of `physics_step` until `#[inline(always)]` (`D-092`). Compare per-symbol sample counts (`perf report --no-children -g none --sort symbol -n`), not shares.
- **A helper's return type can change a hot loop.** An inlined helper returning `Option<[u8; 4]>` made the particle sprite loop store through vector shuffles, +7.4 % of `particle_tick_system` (`D-093`); writing through `&mut [u8; 4]` read within noise. Compare `perf annotate` of the hot symbol on both builds.

## Hotspots

Search these first in a flamegraph:

- `App::run_frame` (shared with the headless harness, `D-110`), `render_frame_full`, `render_frame_full_timed`
- `extract_sprites_default` (column-slice queries, interned `SpriteAssetId` indexing, reused buffers, a key sort skipped in painter order, a cull for stock pipelines; `D-086`, `D-113`, `D-114`)
- `World::query`, `World::query_slices` (`D-135`), `World::flush_reusing` with `insert_run` and `move_components_to` under it
- `physics_step`, `build_pairs`, `gather_tilemap_proxies`
- `particle_tick_system`
- `glyphon`, `TextEngine` and `TextPipeline::prepare` (`D-085`, `D-117`)
- `wgpu`

When `render` is high, classify it: `render_acquire` is swapchain pacing, `render_encode` CPU command recording, `render_submit_present` the present wait. A GPU-bound frame waits in the acquire: `gpu` spends about 7.5 of its 10.7 ms there. Near-zero baselines make percentages meaningless; compare absolute values, as τ_abs does.

## Frame pacing

A concrete `display.present_mode` (`immediate`, `mailbox` or `fifo`) is final; `"auto"` lets `display.vsync` choose the family. The checked-in defaults are `display.present_mode = "auto"`, `display.vsync = false` and `display.max_frame_latency = 1`, which resolve to the auto no-vsync family (`immediate` first). `max_frame_latency` is the requested `wgpu` hint, not a confirmed queue depth.

Legacy `window.vsync`, `render.present_mode` and `render.max_frame_latency` only fill in what `display.*` leaves unset: a display field wins whenever it is set, `"auto"` included (`D-043`). `TUNGSTEN_DISPLAY_PRESENT_MODE` and `TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY` override the display fields themselves; the runner clears them. Present mode and frame latency are hard compare fields, read from the `backend:` line; pacing belongs to the environment and no row owns it. Read the frame period from `interval`, not `total`: a `fifo` frame blocks inside `total`, and a frame-rate cap's wait shows in `interval` alone. The Vulkan matrix that keeps `immediate` / 1 as the shipped default is in `D-078`; `mailbox` gives the GPU-bound `gpu` row `immediate`'s frame rate with a wider spread.

## Backends and RenderDoc

| `WGPU_BACKEND` | Typical platform | `TIMESTAMP_QUERY` |
| --- | --- | --- |
| `vulkan` | Linux | Best chance of GPU pass timings |
| `dx12` | Windows | Often available on modern hardware |
| `metal` | macOS | Backend-dependent; verify per machine |
| `gl` | Fallback | May be unavailable or noisy |
| `auto` | Any | Convenient, less reproducible |

The `backend:` line is the source of truth for the backend, adapter, present mode and frame latency. Without timestamp queries a GPU run has no samples, and its capture stays valid.

RenderDoc on Linux with Vulkan: set the executable to the built binary with the working directory at the repository root (a game row: its folder), set `WGPU_BACKEND=vulkan` and the row's variables, capture a frame after the warm-up, and check the scene pass's draws against the `counters:` line.

## Criterion micro-benchmarks

Criterion (`D-037`) covers isolated primitives: `ecs_bench`, `physics_bench`, `action_map_bench` and `tween_tick` in `tungsten-core`, `render_bench` in `tungsten-render`, `particle_tick` in `tungsten` and `kit` in `tungsten-kit` (the kit's per-entity systems; its bound is each row's median per entity at 10,000 at most twice its median per entity at 1,000, `D-174`). They are regression detectors, not throughput claims. A relative `CARGO_TARGET_DIR` puts criterion's reports under the bench's package folder, so `CARGO_TARGET_DIR=target/criterion-native cargo bench -p tungsten-kit` writes them to `crates/tungsten-kit/target/criterion-native/criterion/`; an absolute path keeps both under one folder. `just bench-build` compiles every bench without running it; run one with `cargo bench -p tungsten-core --bench physics_bench`. Benches build with `.cargo/config.toml`'s `target-cpu=native` flags, so their numbers never compare with captures. They need a machine as quiet as a capture does; compare with `-- --save-baseline before`, then `-- --baseline before`.
