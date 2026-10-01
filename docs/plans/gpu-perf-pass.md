# GPU and render-path performance pass

status: in progress
goal: raise FPS and frame-time stability of the render path (GPU pass cost, the CPU render path of `extract`, `render_encode` and text shaping, and acquire/present pacing) from the baselines measured on 2026-10-01, in two sessions that each start from this file alone.
non-goals: physics, ECS and churn changes (those rows are the unchanged control); changes to a benchmark's work or `workload_version`; new shipped pacing defaults (`immediate / 1` stays, see "Acquire and present pacing"); new render features (render scale, half-resolution particles, GPU particles); the two uniform-sharing P2s, which `docs/plans/debug-cleanup-docs-pass.md` owns; interned sprite IDs; Wayland, Windows and macOS pacing; CI changes; Git commits (the owner handles Git).
files to touch: see "Files to touch" below.
ordered steps: Session A: A0 preflight and session baseline; A1 text pipeline; A2 default extract and tilemap extract; A3 per-frame GPU objects and the implicit surface clear; A4 close-out. Session B: B0 preflight; B1 frame-interval metric; B2 extract culling; B3 final pass to the swapchain; B4 compositor-bypass hint; B5 frame-cap interval check; B6 shape-run cache experiment; B7 close-out.
done-when: every step's own checks hold; `just check`, `just smoke`, `just visual` and `just repo-check` pass at the end of each session; each session's final suite compared with its own session baseline shows no `regressed` owned metric without a justification recorded here or in `DECISIONS.md`; `docs/perf/benchmarks.md`, `docs/perf/profiling-workflow.md`, `DECISIONS.md` with its `docs/DECISION_INDEX.md` rows and `CHANGELOG.md` describe what shipped; this plan is `done` and archived.

Session A complete: 2026-10-01 (A0 to A4; results and the filled targets table under "A4"). A1 (text pipeline) and A3 (per-frame GPU objects, without the present-pass clear) are in the tree; A2 (extract) was taken out for one `regressed` owned metric and is parked for the owner as a patch. Session B has not started and its gated steps still need approval.

## Context digest

- Planned 2026-10-01 on branch `0.34`, commit `1b869e5`, clean tree. Stack: `wgpu` 30.0.1, `winit` 0.30.13 (lock file), `glyphon` 0.12.0, `cosmic-text` 0.19.0, Vulkan on RADV (Mesa 26.2.3).
- Reference machine: Ryzen 5 6600H, Radeon 660M, X11 with KWin compositing, 3840×2160 at 60 Hz, bench window 1920×1080, `immediate / 1`.
- All measurements are on the unmodified build. Captures are in `perf-runs/` (machine-local); tables and scripts are in `perf-runs/20261001-gpu-pass-evidence/` (`EV` below; see its README).
- Largest findings: text shaping is 87% of `gpu`'s `render_encode` and its cache causes 32 ms hitches every 120 frames; the default extract allocates and frees two 25 MB vectors per frame at 400,000 sprites, which costs 9 ms of kernel time per frame; the compositor stalls one frame per refresh by 1.45 ms.
- Two tooling faults found here were fixed right after planning, on 2026-10-01 (`CHANGELOG.md`, `[Unreleased]`): the runner's pacing overrides (M1) and the frame cap (P5). Every baseline below predates them, so A0's compare against `pre-gpu-pass` is also their regression check.
- Session A needs no decision and keeps pixels identical. Session B holds the gated steps; do not start one without the owner's approval.
- Capture rules: `docs/perf/profiling-workflow.md` and the `tungsten-perf` skill. One capture sitting is one blocking foreground call through `EV/scripts/guard.sh`; recapture any run that overlapped `nxcodec.bin` (three did on 2026-10-01).
- GPU rows drift over hours and peak RSS shifted 62 MiB on the untouched tree, so each session captures and compares against its own baseline.
- Git mutations are human-only: deliver one patch per step.

## Measurement conditions and caveats

- Baseline provenance: commit `1b869e5`, `dirty=no`, governor `performance`, rustc 1.98.1, machine `a9df95cb6c32`, flags `-C force-frame-pointers=yes` (`perf-runs/20261001T142937Z-suite/gpu/capture.json`).
- A/A, same build: 0 `regressed` and 0 `improved` owned verdicts right after the baseline (`perf-runs/20261001T144106Z-compare-suite/`, `perf-runs/20261001T144107Z-compare-gpu-throughput/`) and two hours later (`perf-runs/20261001T162332Z-compare-suite/`).
- Noise seen in those A/A compares: `gpu` scene pass p50 7.41 → 7.79 ms (`noisy`), `gpu-throughput` `render_encode` p50 1.53 → 1.34 ms (`noisy`), peak RSS +62 MiB (`regressed`: `particles` 76.9 → 139.5 MiB, `integrated` 215.4 → 277.6 MiB), all in `perf-runs/20261001T162332Z-compare-suite/`. Transparent huge pages are `always`; 62 MiB is 31 huge pages. Read no RSS or scene-pass difference across sittings.
- The scene pass at the unchanged default read 6.89–7.92 ms across the sixteen sweeps (`EV/tables/sweeps-summary.txt`): two modes about 12% apart. Judge a scene-pass change only on captures taken back to back.
- FPS below is 1000 / mean `total`. `total` leaves out the event-loop turnaround and the telemetry logging: the acquire-to-acquire interval is longer by 0.07 ms (`gpu`, 0.6%), 0.05 ms (`gpu` at `min`, 1.2%), 0.03 ms (floor, 3.2%) and 0.21 ms (`integrated`, 2.0%) (`EV/tables/interval-check.txt`).
- "Spike" means a measured frame whose `total` exceeds 1.5 × the run's p50; "jitter" is p99 − p50 of `total`, as in the runner.
- Profilers: `perf` with frame-pointer and DWARF stacks, user mode only (`perf_event_paranoid` is 2). RenderDoc, samply and MangoHud are not installed, so pass and draw structure comes from exact call counts on the unmodified binary (`EV/scripts/frame_counts.sh`), GPU time from timestamp queries and DRM fdinfo.

## Baseline

Medians of 5 runs, milliseconds, from `perf-runs/20261001T142937Z-suite/<row>/` (saved as baseline `pre-gpu-pass`); table built by `EV/scripts/frame_stats.py` into `EV/tables/baseline-table.txt`. GPU columns come from the GPU diagnostic runs of the same captures.

| Row | `total` p50 / p95 / p99 | max | jitter | FPS | spikes per 300 | `extract` p50 / p95 | `render_encode` p50 / p95 | acquire p50 / p95 | submit + present p50 / p95 | `render_span` p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `gpu` | 11.03 / 12.56 / 14.54 | 22.65 | 3.49 | 87.8 | 2 | 1.88 / 2.19 | 2.65 / 2.91 | 6.03 / 7.74 | 0.37 / 0.45 | 11.30 / 11.86 |
| `gpu-throughput` | 26.02 / 26.69 / 26.87 | 27.68 | 1.01 | 38.5 | 0 | 24.15 / 24.69 | 1.53 / 1.89 | 0.05 / 0.05 | 0.20 / 0.21 | 8.95 / 9.67 |
| `integrated` | 10.54 / 11.12 / 11.49 | 12.47 | 0.95 | 95.5 | 0 | 2.78 / 3.03 | 0.93 / 1.01 | 0.03 / 0.04 | 0.43 / 0.50 | 6.25 / 6.61 |
| `particles` | 11.06 / 11.75 / 12.01 | 12.29 | 0.99 | 90.2 | 0 | 7.74 / 8.45 | 0.17 / 0.22 | 0.03 / 0.04 | 0.18 / 0.21 | n/a |
| `physics` (control) | 5.85 / 6.05 / 6.32 | 6.59 | 0.47 | 170.2 | 0 | 0.05 / 0.06 | 0.05 / 0.07 | 0.01 / 0.02 | 0.11 / 0.13 | n/a |
| `physics-sparse` (control) | 3.64 / 3.72 / 3.78 | 4.14 | 0.15 | 274.1 | 0 | 0.04 / 0.05 | 0.04 / 0.05 | 0.01 / 0.02 | 0.09 / 0.11 | n/a |
| `ecs` (control) | 10.92 / 11.28 / 11.55 | 11.90 | 0.59 | 91.3 | 0 | 0.08 / 0.09 | 0.11 / 0.13 | 0.04 / 0.04 | 0.18 / 0.20 | n/a |
| `churn` (control) | 3.58 / 3.95 / 4.27 | 5.01 | 0.66 | 275.1 | 0 | 0.00 / 0.00 | 0.05 / 0.07 | 0.02 / 0.03 | 0.13 / 0.16 | n/a |

Per-pass GPU time, p50 in ms (same captures; `EV/tables/baseline-suite-frame-stats.txt`):

| Pass | `gpu` | `integrated` | `gpu-throughput` |
| --- | --- | --- | --- |
| scene | 7.64 | 1.37 | 8.56 |
| bloom threshold | 0.17 | 0.08 | – |
| bloom downsample 1–5 (sum) | 0.22 | 0.19 | – |
| bloom upsample 4–0 (sum; level 0 alone is 0.33) | 0.44 | 0.44 | – |
| bloom composite | 0.35 | 0.34 | – |
| tonemap | – | 0.33 | – |
| vignette | 0.26 | 0.26 | – |
| `smaa_edges` | 0.47 | 0.60 | – |
| `smaa_blend_weights` | 0.44 | 1.34 | – |
| `smaa_neighborhood` | 0.61 | 0.62 | – |
| text | 0.04 | 0.01 | 0.00 |
| present blit | 0.32 | 0.32 | 0.35 |

GPU engine time per second of wall time, from DRM fdinfo (`EV/tables/gpu-usage.txt`; 1000 is one fully busy engine):

| Config | Bench process | Compositor | `gpu_busy_percent` |
| --- | --- | --- | --- |
| `gpu` | 922 | 41 | 95.8 |
| `gpu-throughput` | 437 | 70 | 61.3 |
| `integrated` | 647 | 105 | 88.6 |
| `particles` | 281 | 139 | 52.2 |
| `physics` | 239 | 118 | 61.2 |
| `gpu` at `min` | 806 | 94 | 99.6 |
| floor (empty frame) | 598 | 84 | 99.2 |

Frame structure, exact counts per frame (`EV/tables/frame-counts-{floor,gpu,integrated,text-static,text-dynamic}.txt`, `EV/tables/frame-counts-internal.txt`; n/m is not measured):

| Per frame | floor | `gpu` | `integrated` | 100 static sections | 100 changing sections |
| --- | --- | --- | --- | --- | --- |
| Render passes begun | 4 | 20 | 21 | 4 | 4 |
| Draws | 1 | 78 | 47.3 | 2 | 2 |
| Bind groups created (and destroyed) | 1 | 31 | 32 | 1 | 1 |
| Buffers created (and destroyed) | 3 | 38 | 42.3 | 4 | 4 |
| of which staging buffers | 3 | 26 | n/m | n/m | n/m |
| Implicit surface clear passes | 1 | 1 | n/m | n/m | n/m |
| Backend submits | 2 | 2 | 2 | 2 | 2 |
| `malloc` calls | 205 | 4,709 | 4,616 | 1,217 | 3,615 |

Steady kernel cost per frame (`EV/tables/rusage-slopes.txt`): `gpu-throughput` 1,606 minor faults and 8.98 ms of system time (18.02 ms user); `gpu` frames 100–300 387 faults and 0.96 ms, frames 600–900 3 faults and 0.10 ms; `integrated` 61 faults and 0.37 ms; `particles` and `physics` no faults and at most 0.15 ms.

Capacity (`perf-runs/20261001T154036Z-capacity-60hz/`, `perf-runs/20261001T154253Z-capacity-144hz/`): at 60 Hz `gpu` holds scale 1.54 (3,084 sprites, present-limited), `gpu-throughput` 0.57 (226,516 sprites, `extract`-limited at 14.82 ms), `integrated` 1.48 (`physics_step`-limited). At 144 Hz `gpu` cannot reach the budget at any scale (p95 8.76 ms with one sprite) and `integrated` holds scale 0.09, present-limited: the fixed 1080p post chain bounds both.

## Benchmark run order

Order used on 2026-10-01, and the order each execution step repeats in small:

1. `just perf describe gpu`: builds the capture binary once, so no capture includes a build.
2. `just perf suite --repeat 5`: the compare-grade baseline with the four control rows, taken first, before any profiler or diagnostic run has loaded the machine.
3. A/A of the render-path rows (`--only gpu,gpu-throughput,particles,integrated`) straight after, so the noise floor is known before any difference is read; once more at the end to bracket drift.
4. Knob sweeps on `gpu`, scene-pass drivers first (the row's declared bottleneck), then text, post chain, resolution and batch structure.
5. Isolation captures: an empty frame, then one feature at a time.
6. Pacing matrix, compositor pairs, frame-cap test.
7. Capacity search at 60 and 144 Hz.
8. CPU profiles, which leave the machine warm, after every timing capture.
9. Exact counts, GPU engine time and page-fault slopes: load-independent or diagnostic, so they go last.

## Key systems to test

| System | Code | Measured through |
| --- | --- | --- |
| Text layout cache, shaping, glyph vertex build | `crates/tungsten-render/src/text.rs` | `gpu` `render_encode`, spikes, peak RSS; text isolation captures; `--sweep glyphs`, `--sweep text_change`; `integrated --sweep tags` |
| Default sprite extract | `crates/tungsten/src/sprite_extract.rs` | `gpu-throughput` and `gpu` `extract`; `particles` and `integrated` `extract` (reported); `EV/scripts/rusage_slope.py` |
| Tilemap extract | `crates/tungsten/src/tilemap_extract.rs` | `gpu --sweep tile_layers`, `--sweep zoom`; tiles isolation capture |
| Sprite draw and instance upload | `crates/tungsten-render/src/sprite.rs` | `gpu-throughput` `render_encode`; `--sweep z_layers` |
| Post chain recording and per-frame GPU objects | `crates/tungsten-render/src/post/{mod,bloom,smaa}.rs`, `renderer.rs` | `EV/scripts/frame_counts.sh`; `integrated --sweep post`; post isolation captures |
| GPU passes | scene, bloom, SMAA, stock effects, present blit | GPU diagnostic rows; `--sweep post,aa,bloom_mips,resolution` |
| Acquire and present | `crates/tungsten-render/src/renderer.rs`, `surface.rs`; wgpu-hal swapchain | `render_acquire`, `render_submit_present`; pacing matrix; `EV/scripts/series.py` |
| Frame loop pacing | `crates/tungsten/src/app.rs` (`stage_pacing`, redraw scheduling) | frame-cap run; `EV/scripts/interval_check.sh` |
| Telemetry and runner | `crates/tungsten/src/app.rs`, `scripts/bench.py`, `scripts/bench_report.py` | `backend:` line against the request |

## Findings, ranked by measured cost

Overall order, by the cost each shows in the row where it is largest; hitches come first because the goal names microstutter. The tables below hold the evidence and capture directories.

1. C3, text-cache hitches: a 41–43 ms frame every 120 frames in `gpu` once the cache is full, and two 17–23 ms frames before.
2. C1, extract buffers allocated and freed per frame: 8.98 ms of kernel time per frame in `gpu-throughput`.
3. C5, extract iteration: 34.8% of main-thread user samples in `gpu-throughput`, about 6.3 ms of its 18.02 ms of user time per frame.
4. C4, extract sort and ID hashing: 29.0% + 10.7% of samples in `particles`, about 4.3 ms of its 10.91 ms of user time per frame (`EV/tables/rusage-slopes.txt`).
5. G1 and G3, the fixed post chain: 2.58 ms of GPU time on an empty 1080p frame, SMAA alone up to 4.04 ms.
6. C2, text shaping and rebuild: 2.30 ms of `render_encode` per frame in `gpu`, 0.73 ms even when nothing changed.
7. P1, compositor stall: 1.4–1.6 ms on one frame per refresh.
8. C6, no culling: about 1 ms of `integrated`'s 2.78 ms `extract` (estimate from the 7.8% in view).
9. C8, per-frame GPU objects: up to 0.42 ms of CPU per frame in `integrated`.
10. C7 and G2: tile lookups 0.40 ms of CPU; present blit 0.32 ms and an implicit clear of at most 0.17 ms of GPU time.

### CPU render path

| # | Weakness | Evidence |
| --- | --- | --- |
| C1 | **Default extract allocates and frees its buffers every frame.** `entries` and each batch's `instances` grow by doubling and are dropped at frame end; at 400,000 sprites each reaches 524,288 × 48 bytes, 25 MB (from the code, not a capture). glibc grows and trims the heap for them every frame, and the kernel hands back zeroed pages each time. | `gpu-throughput`: 8.98 ms of kernel time and 1,606 page faults per frame against 26.85 ms wall (`EV/tables/rusage-slopes.txt`); 7.0 `sbrk` calls and 5.0 `brk` system calls per frame, no `mmap` or `munmap` (`EV/tables/mmap-counts.txt`); `extract` p50 24.15 of `total` 26.02 ms (`perf-runs/20261001T142937Z-suite/gpu-throughput/`). User-mode profile: `realloc` 6.5% and `dealloc` 7.2% of main-thread samples (`EV/tables/profile-gpu-throughput-fp.txt`, from `perf-runs/20261001T154618Z-gpu-throughput/`). |
| C2 | **Text is reshaped, re-cached and rebuilt every frame.** `prepare` builds a `Buffer` before the cache lookup (`Buffer::new` shapes an empty line, source S14), keys the cache by content so changing text never hits, keeps every miss for 360 frames, and rebuilds glyph vertices for unchanged text. | `TextPipeline::prepare` is 40.8% of `gpu`'s main-thread samples (`shape_until_scroll` 32.0%, `Buffer::new` 6.7%, glyphon `prepare` 5.4%; `EV/tables/profile-gpu-fp.txt` from `perf-runs/20261001T154430Z-gpu/`; DWARF cross-check 38.5 / 30.0 / 4.5 / 5.1% in `EV/tables/profile-gpu-dwarf.txt` from `perf-runs/20261001T155517Z-gpu/`). `render_encode` p50 0.52 → 2.66 → 9.85 ms at 0 / 4,000 / 16,000 glyphs (`perf-runs/20261001T150031Z-gpu-sweep-glyphs/`): 21–23 µs per changed 40-character section. With text alone `render_encode` is 2.30 ms (`perf-runs/20261001T152010Z-gpu-set1cdab1/`), 87% of the row's 2.65 ms. 100 unchanged sections still cost 0.73 ms (`render_encode` 1.25 ms at `text_change=0` in `perf-runs/20261001T150245Z-gpu-sweep-text_change/` against 0.52 ms without text in the glyph sweep) and 1,012 `malloc` calls per frame over the floor (`EV/tables/frame-counts-text-static.txt`, `EV/tables/frame-counts-floor.txt`). In `integrated`, 64 tags cost 0.41 ms of `render_encode` (0.52 → 0.93; `perf-runs/20261001T160902Z-integrated-sweep-tags/`). |
| C3 | **Text cache growth causes deterministic hitches.** Two are consistent with the hash map growing at 14,336 and 28,672 entries (100 inserts per frame reach them at frames 143 and 286); from frame 480 the 120-frame prune drops 12,000 buffers at once. | Same frames in every run: measured frames 83 and 226 (`total` 17.4–23.3 ms, `render_encode` 9.0–14.1 ms) and every 120 frames from 419 (`total` 41.1–42.9 ms, `render_encode` 31.4–32.7 ms) in `perf-runs/20261001T160151Z-gpu/` (`EV/tables/gpu-frames900-spikes.txt`); frames 83 and 226 in all five baseline runs (`EV/tables/baseline-suite-frame-stats.txt`). Peak RSS 664.5 MiB at 300 frames (`perf-runs/20261001T142937Z-suite/gpu/`) and 910 MiB at 900 (`perf-runs/20261001T160151Z-gpu/`). |
| C4 | **Default extract sorts 48-byte tuples and hashes a string per sprite.** | `particles`: `extract_sprites_default` 68.5% of main-thread samples, stable sort 29.0%, `AssetRegistry::get_sprite` 10.7%, SipHash 7.6% (`EV/tables/profile-particles-fp.txt` from `perf-runs/20261001T154903Z-particles/`); `extract` p50 7.74 of `total` 11.06 ms (`perf-runs/20261001T142937Z-suite/particles/`). `gpu`: 0.13 µs per mixed-kind sprite (`extract` 1.90 → 5.80 ms from 2,000 to 32,000 sprites; `perf-runs/20261001T144335Z-gpu-sweep-sprites/`). `integrated`: sort 7.7%, extract 23.2% (`EV/tables/profile-integrated-fp.txt` from `perf-runs/20261001T154740Z-integrated/`). |
| C5 | **Default extract iterates through a `filter_map` over a flattened five-way zip.** | `gpu-throughput`: the collect chain is 58.4% of main-thread samples, 34.8% under the iterator's `try_fold` (`EV/tables/profile-gpu-throughput-fp.txt`). The sort is 7.2% there because the field is already in order. |
| C6 | **Default extract does not cull.** | `integrated` uploads 968,448–976,080 bytes of instances per frame, 20,200 instances (`EV/counts/staging-sizes-integrated.txt`), while the view is 1,920 px of a 24,576 px level (7.8%; `docs/perf/benchmarks.md`). |
| C7 | **Tilemap extract resolves a string ID per tile.** | 0.40 ms for about 12,650 tiles (the `visible_tiles` counter), 32 ns each (`perf-runs/20261001T152048Z-gpu-set356039/`); `extract` 1.94 → 2.99 ms when `zoom=0.5` shows four times the tiles (`perf-runs/20261001T151539Z-gpu-sweep-zoom/`); `extract_tilemaps` 9.4% of `gpu`'s main-thread samples (`EV/tables/profile-gpu-fp.txt`). |
| C8 | **GPU objects are created and destroyed every frame.** Bloom makes 12 uniform buffers and 25 bind groups; SMAA 4 bind groups; each stock pass and the present blit one; the heavy material's uniform block is written once per batch with the same bytes. | `gpu`: 31 bind groups and 38 buffers per frame, 26 of them staging buffers (`EV/tables/frame-counts-gpu.txt`, `EV/tables/frame-counts-internal.txt`). Cost of the chain's recording in a CPU-bound frame: `render_encode` 0.74 → 0.93 ms and submit 0.20 → 0.43 ms from `post=none` to `game` (`perf-runs/20261001T160705Z-integrated-sweep-post/`); submit grows 0.23 → 0.43 ms from 1 to 8 bloom mips (`perf-runs/20261001T150806Z-gpu-sweep-bloom_mips/`). |

### GPU pass cost

| # | Weakness | Evidence |
| --- | --- | --- |
| G1 | **A fixed post chain of full-screen passes.** Each point-wise 1080p pass costs 0.24–0.35 ms; bloom, vignette and SMAA add 2.58 ms to an empty frame. | Fog 0.24, vignette 0.26, tonemap 0.33, color adjust 0.35 ms (`perf-runs/20261001T150417Z-gpu-sweep-post/`). `render_span` 0.41 → 2.99 ms (`perf-runs/20261001T152006Z-gpu-setc52067/` against `perf-runs/20261001T152037Z-gpu-set3ba908/`). The chain scales with pixels: bloom 0.55 / 1.18 / 2.17 / 4.64 ms, SMAA 0.65 / 1.52 / 3.80 / 11.38 ms, blit 0.14 / 0.32 / 0.58 / 1.22 ms at 720p / 1080p / 1440p / 2160p (`perf-runs/20261001T151057Z-gpu-sweep-resolution/`). It sets the 144 Hz capacity limit of `gpu` and `integrated`. |
| G2 | **The present blit and an implicit clear run on every frame.** The blit is a full-screen copy into the swapchain; because that pass loads the fresh surface texture, wgpu clears the texture in a pass of its own first (S3). | Blit 0.32 ms in every row; it is 0.32 of the empty frame's 0.41 ms span (`perf-runs/20261001T152006Z-gpu-setc52067/`). One `clear_texture_via_render_passes` per frame and 4 passes for 3 recorded (`EV/tables/frame-counts-internal.txt`, `EV/tables/frame-counts-floor.txt`). GPU time outside `render_span` on the empty frame is at most 0.17 ms (0.58 ms of engine time per frame in `EV/tables/gpu-usage.txt` against the 0.41 ms span). |
| G3 | **SMAA cost follows edge count.** | SMAA total 1.52 ms in `gpu` and 2.56 ms in `integrated` (`smaa_blend_weights` 1.34; per-pass table above), 4.04 ms with the glow layer off (`perf-runs/20261001T144723Z-gpu-sweep-glow/`), 3.54 ms at `zoom=0.5` (`perf-runs/20261001T151539Z-gpu-sweep-zoom/`); 41% of `integrated`'s span. On an empty frame the blend-weight pass costs 0.18 ms (`perf-runs/20261001T152030Z-gpu-set331eb6/`), which is all a stencil mask (S15) could remove. |
| G4 | **The `gpu` scene pass is fill-bound.** | Glow: 3.95 ms alone (`perf-runs/20261001T152053Z-gpu-setc351c8/`), about 0.08 ns per blended pixel (scene 6.94 → 18.42 ms from 48 to 96 px; `perf-runs/20261001T145000Z-gpu-sweep-glow_px/`). Sprites: 2.50 ms alone (`perf-runs/20261001T152107Z-gpu-setde0034/`); lit shading 2.0 ns per pixel at 16 lights (scene 6.55 → 7.73 ms from 0 to 16 lights; `perf-runs/20261001T145338Z-gpu-sweep-lights/`). `msaa4` multiplies the scene pass by 2.2 (8.19 → 18.29 ms; `perf-runs/20261001T150610Z-gpu-sweep-aa/`, `perf-runs/20261001T151027Z-gpu-seta328e5/`). No engine-side change is proposed for this: the work is the benchmark's. |
| G5 | **Draw calls are not a cost at these counts.** | 64 z layers (about 400 batches) against 8 (56): `render_encode` +0.33 ms (`perf-runs/20261001T151400Z-gpu-sweep-z_layers/`). No batching work is proposed. |

### Acquire and present pacing

| # | Finding | Evidence |
| --- | --- | --- |
| P1 | **One frame per display refresh stalls 1.4–1.6 ms in surface acquire, whatever the load.** KWin composites the 4K desktop at 60 Hz and uses about 1.5 ms of GPU time per refresh; the bench waits behind it. | Empty frame: 17 stalls in 300 frames, 16.18 ms apart, +1.43 ms each; `gpu` at `min`: 68 in 300, +1.62 ms (`EV/tables/acquire-stall-series.txt`). p95 − p50 of `total` is 1.4–1.7 ms in the eight isolation captures that wait on the GPU (`EV/tables/isolation-frame-stats.txt`; the CPU-bound dynamic-text one shows none) and 1.53 ms in `gpu` (baseline table). KWin's GPU time is 148–156 ms per 1.6 s run (`perf-runs/20261001T153042Z-hand-gpu-min-immediate-lat1/capture.json`). With compositing blocked for the bench window the stalls vanish; see the table below. |
| P2 | **`max_frame_latency` 1, 2 and 3 are indistinguishable.** The X11 surface's minimum is 3 images, so wgpu clamps 1 to 2 (S2); its acquire waits for the submission three frames back. | Mean `total` differs by at most 0.12 ms across latencies 1–3 in every mode and row (`EV/tables/hand-pacing-frame-stats.txt`, from the 27 `perf-runs/20261001T153042Z-hand-gpu-min-immediate-lat1/` … `perf-runs/20261001T153649Z-hand-integrated-fifo-lat3/` captures); `minImageCount = 3` (S8). |
| P3 | **`mailbox` is not better than `immediate`** (mode definitions: S4). | `gpu`: same mean, p50 / p95 / p99 12.64 / 17.09 / 18.47 against 11.06 / 12.67 / 14.11 ms, 36 frames over 16.67 ms per 300 against 2 (`perf-runs/20261001T153253Z-hand-gpu-mailbox-lat1/` against `perf-runs/20261001T153210Z-hand-gpu-immediate-lat1/`). `gpu` at `min`: FPS 290.8 against 267.1 but 92 spikes per 300 against 31 (`perf-runs/20261001T153057Z-hand-gpu-min-mailbox-lat1/`, `perf-runs/20261001T153042Z-hand-gpu-min-immediate-lat1/`). |
| P4 | **`fifo` paces cleanly at 60 Hz.** | p50 / p95 / p99 16.61 / 17.12 / 17.39 (`gpu` at `min`), 16.62 / 17.45 / 17.93 (`gpu`), 16.45 / 16.97 / 17.28 ms (`integrated`); jitter 0.76–1.30 ms (`perf-runs/20261001T153112Z-hand-gpu-min-fifo-lat1/`, `perf-runs/20261001T153336Z-hand-gpu-fifo-lat1/`, `perf-runs/20261001T153604Z-hand-integrated-fifo-lat1/`). |
| P5 | **`display.frame_rate_cap` does not cap. Fixed 2026-10-01** (step 1 of `docs/plans/debug-cleanup-docs-pass.md`, its bug B1). `request_redraw` at frame end woke the loop before `ControlFlow::WaitUntil` could wait (S12); a capped frame now leaves the request to `about_to_wait`. | Before: 360 frames take 1.5–1.6 s with a cap of 60 or 144, as without one (`perf-runs/20261001T154001Z-hand-framecap60-gpumin/`, `perf-runs/20261001T154005Z-hand-framecap144-gpumin/`). After: 6.22 s at 60 and 2.81 s at 144 (`perf-runs/20261001T173028Z-hand-cap60-fixed/`, `perf-runs/20261001T173047Z-hand-cap144-fixed/`); frame interval p50 / p99 16.75 / 17.78 ms at 60 and 7.05 / 8.45 ms at 144, taken with a remote-desktop client connected (`EV/fault-fixes/interval-fixed.txt`). |
| P6 | **A GPU-bound frame waits in acquire, not in present.** | `gpu`: acquire p50 6.03 ms, submit + present 0.37 ms (`perf-runs/20261001T142937Z-suite/gpu/`). The benchmark doc's "the GPU wait shows as present time" is out of date. |

Compositor pairs, hand-run, `immediate / 1`, 3 runs each (`EV/tables/compositor-frame-stats.txt`; warm-ups of 900, 180 and 240 frames, so not comparable with the suite):

| Config | `total` p50 / p95 / p99 | jitter | FPS | spikes per 300 | KWin GPU ms per run | Capture |
| --- | --- | --- | --- | --- | --- | --- |
| `gpu` at `min`, compositing on | 3.36 / 5.09 / 5.14 | 1.78 | 268.4 | 27 | 446–450 | `perf-runs/20261001T153749Z-hand-comp-control-gpumin/` |
| `gpu` at `min`, blocked | 3.40 / 3.60 / 3.80 | 0.38 | 292.2 | 1 | 0–4 | `perf-runs/20261001T153805Z-hand-comp-blocked-gpumin/` |
| `gpu`, compositing on | 10.99 / 12.52 / 13.51 | 2.49 | 87.3 | 2 | 237–253 | `perf-runs/20261001T153819Z-hand-comp-control-gpu/` |
| `gpu`, blocked | 10.98 / 12.02 / 12.82 | 1.86 | 89.7 | 2 | 2–3 | `perf-runs/20261001T153837Z-hand-comp-blocked-gpu/` |
| `integrated`, compositing on | 10.87 / 11.56 / 12.02 | 1.15 | 92.4 | 1 | 595–639 | `perf-runs/20261001T153855Z-hand-comp-control-integrated/` |
| `integrated`, blocked | 10.71 / 11.23 / 11.75 | 1.04 | 94.4 | 1 | 0–3 | `perf-runs/20261001T153913Z-hand-comp-blocked-integrated/` |
| `gpu` at `min`, `_NET_WM_BYPASS_COMPOSITOR` alone (2 runs) | 3.38 / 3.60 / 3.96 | 0.58 | 293.3 | 1 | 3–4 | `perf-runs/20261001T165150Z-hand-comp-prop-net_wm_bypass_compositor/` |
| `gpu` at `min`, `_KDE_NET_WM_BLOCK_COMPOSITING` alone (2 runs) | 3.38 / 3.58 / 3.79 | 0.41 | 293.8 | 0–1 | 1–5 | `perf-runs/20261001T165159Z-hand-comp-prop-block_compositing/` |

"Blocked" rows set both window properties with `xprop` as soon as the window maps; either one alone is enough for KWin.

Shipped default: the matrix gives no reason to move from `immediate / 1` (P2–P4), so this plan proposes no change and no decision on it.

### Measurement and tooling faults

| # | Fault | Evidence |
| --- | --- | --- |
| M1 | **`just perf run --present-mode / --max-frame-latency` do not apply. Fixed 2026-10-01.** They set `TUNGSTEN_RENDER_*`, but the checked-in `tungsten.json` sets `display.present_mode` and `display.max_frame_latency`, and `DisplayConfig::resolve` lets those win (`crates/tungsten-core/src/display.rs`). The capture still read valid. Mesa's `MESA_VK_WSI_PRESENT_MODE` (S7) forces a mode in the driver, but the `backend:` line would not show it. The runner now sets the new `TUNGSTEN_DISPLAY_PRESENT_MODE` and `TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY`, and a run whose `backend:` line does not confirm the request makes the capture invalid. | Before: all 27 runner captures `perf-runs/20261001T152307Z-gpu-immediate-lat1/` … `perf-runs/20261001T152821Z-gpu-min-fifo-lat3/` log `present_mode: immediate max_frame_latency: 1` and equal numbers (`EV/tables/pacing-{gpu,integrated,gpumin}-frame-stats.txt`). After: `perf-runs/20261001T173002Z-gpu-min-fifo-lat2/` logs `fifo` / 2 and `perf-runs/20261001T173006Z-gpu-min-mailbox-lat3/` logs `mailbox` / 3 (one 120-frame run each with a remote-desktop client connected: proof that the mode applies, not timing data). |
| M2 | **No frame-interval metric.** FPS can only be derived from `total`, which is 0.6–3.2% short. | `EV/tables/interval-check.txt`. |
| M3 | **Spike counts and the per-run maximum are not in the runner's reports.** | They come from `EV/scripts/frame_stats.py` here. |
| M4 | **GPU pass times need the blocking diagnostic run,** which creates a query set and two buffers per frame and waits on the device (`crates/tungsten-render/src/timing.rs`). | Code reading; source S22 shows the non-blocking pattern. |
| M5 | **Provenance lacks GPU and desktop state:** GPU power level (`auto` here), compositor on or off, transparent-huge-page mode. | The scene-pass modes and the RSS shift under "Measurement conditions". |

## Targets

Set from the 2026-10-01 values in the "Baseline" column. Verdicts are judged against the session's own baseline from A0 or B0, because the GPU rows drift between sittings; the absolute targets are then checked on the final suite. A target that is missed while the judged verdict is `improved` is reported with its per-capture deltas, not chased with a third attempt.

| Row | Metric | Baseline | After Session A | After Session B |
| --- | --- | --- | --- | --- |
| `gpu` | FPS | 87.8 | not lower | ≥ 90 |
| `gpu` | `total` p50 / p95 / p99 | 11.03 / 12.56 / 14.54 | ≤ 11.03 / ≤ 12.56 / ≤ 13.8 | ≤ 10.8 / ≤ 12.3 / ≤ 13.4 |
| `gpu` | jitter | 3.49 | ≤ 2.8 | ≤ 2.6 |
| `gpu` | spikes per 300 frames; per 900 frames | 2; 7 | 0; 0 | 0; 0 |
| `gpu` | max `total`, 300 and 900 frames | 22.65; 42.66 | ≤ 16.5 | ≤ 16.5 |
| `gpu` | `render_encode` p50 / p95 | 2.65 / 2.91 | ≤ 2.30 / ≤ 2.60 | not regressed |
| `gpu` | `extract` p50 | 1.88 | ≤ 1.50 | not regressed |
| `gpu` | peak RSS, 300 and 900 frames | 664.5; 910 MiB | ≤ 300 MiB | not regressed |
| `gpu-throughput` | FPS | 38.5 | ≥ 55 | not lower |
| `gpu-throughput` | `total` p50 / p95 / p99 | 26.02 / 26.69 / 26.87 | ≤ 18.0 / ≤ 19.0 / ≤ 19.5 | not regressed |
| `gpu-throughput` | jitter; spikes | 1.01; 0 | ≤ 1.5; 0 | not regressed |
| `gpu-throughput` | `extract` p50 / p95 | 24.15 / 24.69 | ≤ 16.0 / ≤ 16.8 | not regressed |
| `gpu-throughput` | kernel time and faults per frame | 8.98 ms; 1,606 | ≤ 1.0 ms; ≤ 50 | not regressed |
| `particles` | FPS | 90.2 | ≥ 113 | not lower |
| `particles` | `total` p50 / p95 / p99 | 11.06 / 11.75 / 12.01 | ≤ 8.8 / ≤ 9.5 / ≤ 9.9 | not regressed |
| `particles` | jitter; `extract` p50 | 0.99; 7.74 | ≤ 1.1; ≤ 5.4 | not regressed |
| `integrated` | FPS | 95.5 | ≥ 99 | ≥ 106 |
| `integrated` | `total` p50 / p95 / p99 | 10.54 / 11.12 / 11.49 | ≤ 10.1 / ≤ 10.7 / ≤ 11.1 | ≤ 9.4 / ≤ 10.0 / ≤ 10.4 |
| `integrated` | jitter; spikes | 0.95; 0 | ≤ 1.0; 0 | ≤ 1.0; 0 |
| `integrated` | `extract` p50; `render_encode` p50 | 2.78; 0.93 | ≤ 2.45; ≤ 0.80 | ≤ 1.7; not regressed |
| `integrated` | `render_span` p50 | 6.25 | not regressed | ≤ 5.95 |
| floor | `render_span` p50; FPS | 0.41; 1,109 | not regressed | ≤ 0.15; ≥ 1,400 |
| `gpu` at `min`, hand-run | p95 / p99; spikes per 300; FPS | 5.15 / 5.26; 31; 267.1 | not regressed | with B4: ≤ 3.8 / ≤ 4.2; ≤ 2; ≥ 288 |
| controls | `physics`, `physics-sparse`, `ecs`, `churn` owned metrics | table above | not regressed | not regressed |

Floor baseline: `perf-runs/20261001T152006Z-gpu-setc52067/`; `gpu` at `min` baseline: `perf-runs/20261001T153042Z-hand-gpu-min-immediate-lat1/`; every other baseline: `perf-runs/20261001T142937Z-suite/` and, for the 900-frame values, `perf-runs/20261001T160151Z-gpu/`.

## Decisions for approval

Session A needs none. In Session B a gated step starts only when its row reads `yes` under "Approved". A new entry takes the next free ID (`D-085` is free on 2026-10-01) and adds its `docs/DECISION_INDEX.md` row in the same change.

B1 is not gated: it adds a reported telemetry field that no row owns, and `D-038` (inline timing) stands as written. The two environment overrides that were part of it shipped on 2026-10-01; they leave `D-043` (display fields win over legacy render fields) and `D-078` (override captures) as written.

| Step | Decision needed | Checked against | Recommendation | Approved |
| --- | --- | --- | --- | --- |
| B2 | New entry: the default extract no longer emits sprites outside the camera view. Amends `D-042`; the parallax remap of `D-073` runs before the test. | `D-018`, `D-042`, `D-073` | Do it | no |
| B3 | New entry: the last full-screen stage renders into the swapchain and text draws there; the present blit and `PresentSource` remain only on capture frames. Amends the tail description of `D-059` and the frame order in `crates/tungsten-render/AGENTS.md`. The `gpu` row stops owning `gpu.present`, which changes a row's owned metrics. | `D-047`, `D-057`, `D-058`, `D-059`, `D-078` | Do it | no |
| B4 | New entry: a direct `x11rb` dependency (`D-015` rule 1, platform API; already in `Cargo.lock` through winit) and a `display.bypass_compositor` setting with its default. Sets `_NET_WM_BYPASS_COMPOSITOR` (S10) as SDL does by default (S11). User-visible: desktop compositing is suspended while the window exists. | `D-015`, `D-043` | Do it, default on for borderless fullscreen and off for windowed | no |
| B6 | New entry if kept: a direct `cosmic-text` dependency to enable its `shape-run-cache` feature (`D-015` rule 2; already in the tree through glyphon, `D-026`). | `D-015`, `D-026` | Run the experiment; keep only on an `improved` verdict | no |

Nothing here reverses a decision. `D-078`'s pacing evidence is from the retired scene; the new matrix confirms its conclusion.

## Files to touch

- A1: `crates/tungsten-render/src/text.rs`, new `crates/tungsten-render/src/tests/text.rs`.
- A2: `crates/tungsten/src/sprite_extract.rs`, `crates/tungsten/src/tilemap_extract.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/sprite_extract.rs`, new tilemap-extract tests beside them.
- A3: `crates/tungsten-render/src/{renderer,targets,sprite}.rs`, `crates/tungsten-render/src/post/{mod,bloom,smaa,fullscreen}.rs`, `crates/tungsten-render/src/passes/order.rs`, `crates/tungsten-render/src/tests/{passes_order,bloom,post,smaa}.rs`, `crates/tungsten/src/app.rs`.
- A4 and B7: `docs/perf/benchmarks.md`, `docs/perf/profiling-workflow.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `CHANGELOG.md`, this plan.
- B1: `crates/tungsten/src/{app,telemetry}.rs`, `scripts/bench_report.py`, `scripts/test-bench.py`, `docs/perf/profiling-workflow.md`, `.claude/skills/tungsten-perf/SKILL.md`.
- B2: `crates/tungsten/src/sprite_extract.rs` and its tests.
- B3: `crates/tungsten-render/src/{renderer,targets,timing}.rs`, `crates/tungsten-render/src/passes/order.rs`, `crates/tungsten-render/src/post/{mod,bloom,smaa}.rs`, their tests, `crates/tungsten-render/AGENTS.md`, `examples/02_bench/src/gpu.rs` (owned list), `docs/perf/benchmarks.md`.
- B4: `Cargo.toml`, `crates/tungsten/Cargo.toml`, `Cargo.lock`, `crates/tungsten-core/src/{config,display}.rs`, `crates/tungsten/src/app.rs`, `tungsten.json`, `DESIGN.md`.
- B6: `Cargo.toml`, `crates/tungsten-render/Cargo.toml`, `crates/tungsten-render/src/text.rs`.

If `docs/plans/debug-cleanup-docs-pass.md` has run first, its render cleanups moved code in `renderer.rs` and `sprite.rs`: locate touch points again with `rg`, not by line number.

## Common setup for both sessions

```bash
git branch --show-current                      # expect 0.34, or the branch the owner names
pgrep -x nxcodec.bin && echo "disconnect the remote-desktop client first"
pgrep -a -x claude                             # one session only
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor   # performance
export WGPU_BACKEND=vulkan
E=perf-runs/20261001-gpu-pass-evidence
S=perf-runs/$(date -u +%Y%m%d)-gpu-pass-session-a   # session-b in Session B
mkdir -p $S && bash $E/scripts/loadlog.sh $S/load.log &     # stop it when the session ends
G="bash $E/scripts/guard.sh $S/guard.log"
```

- Every capture command below runs as `$G <command>`, alone, as one blocking foreground call. After it, check `guard: … sightings=0`; on a sighting, find the overlapped runs with `python3 $E/scripts/capture_load.py $S/load.log <capture>` and recapture them.
- A step's standard check is the full suite against the previous step: `$G just perf suite --repeat 5 --compare <previous baseline name>`, then `just perf baseline save perf-runs/<UTC>-suite <step name>`. Shared code moves rows a step does not target, so never narrow it with `--only`.
- Spikes, maxima and FPS: `python3 $E/scripts/frame_stats.py --spikes perf-runs/<UTC>-suite/gpu` (or any capture).
- If `EV` is missing, the machine is not the reference machine and none of these baselines compare; capture new ones per `docs/perf/profiling-workflow.md`.
- Each step ends with `cargo test -p <crates touched>` and strict clippy for them; render steps add `just smoke` and `just visual`.

## Session A: CPU render path, no decisions, pixels unchanged

### A0. Preflight and session baseline

1. Run the common setup. Read `crates/tungsten-render/AGENTS.md` and load the `tungsten-perf` and `tungsten-wgpu` skills. Set this plan to `status: in progress`.
2. `$G just perf suite --repeat 5`, then `just perf baseline save perf-runs/<UTC>-suite gpu-pass-a0`.
3. `just perf compare pre-gpu-pass gpu-pass-a0`.
4. `$G just perf run gpu --frames 900 --repeat 3 --gpu-timing off` (the 900-frame reference of this session).
5. `bash $E/scripts/frame_counts.sh a0-gpu gpu default ""` and `python3 $E/scripts/rusage_slope.py gpu-throughput gpu throughput "" 100 300`.

Done when: the suite is valid; step 3 shows no `regressed` or `improved` owned verdict (peak RSS may differ by about 62 MiB; if a `gpu` row verdict differs, recapture once, then record the drift and continue from `gpu-pass-a0`); the counts match the baseline table's `gpu` column.

`pre-gpu-pass` predates the 2026-10-01 fault fixes, which changed the frame loop's redraw scheduling (`stage_pacing`, `about_to_wait` in `crates/tungsten/src/app.rs`). The exact per-frame counts of `gpu` are the same on both trees (`EV/fault-fixes/frame-counts-fixed-gpu.txt` against `EV/tables/frame-counts-gpu.txt`), but no quiet-machine capture has compared them: a remote-desktop client was connected. If a `regressed` verdict in step 3 survives the recapture on a row that does not wait on the GPU (`physics`, `physics-sparse`, `ecs`, `churn`, `particles`), look there before calling it drift.

**A0 result (2026-10-01, 18:03Z–18:25Z): done.** The session folder is `perf-runs/20261001-gpu-pass-session-a/` (`SA` below): guard and load logs, `logs/`, `patches/`, and `snap/` with one full tree per step boundary (`st-head`, `st-a0`, …; a step is undone by copying its files back from the previous tree). Use that literal path for the rest of Session A.

- Baseline `gpu-pass-a0` is `perf-runs/20261001T180403Z-suite/`: commit `5631896`, `dirty=no`, valid, no encoder sighting. The A/A recapture `perf-runs/20261001T180951Z-suite/` reads 0 `regressed`, 0 `improved`, 40 `unchanged`, 15 `noisy` against it (`perf-runs/20261001T181459Z-compare-suite/`).
- Against `pre-gpu-pass` (`perf-runs/20261001T180908Z-compare-suite/`; recapture `perf-runs/20261001T181449Z-compare-suite/`): `gpu` `stage.extract` p50 reads `regressed` both times (1.89 → 2.00 and 2.01 ms, τ 0.057); `churn` `stage.flush` p50 reads `regressed` once (2.62 → 2.71 ms, τ 0.079) and `noisy` in the recapture (2.70 ms); peak RSS is 55–57 MiB higher in every row. No other owned verdict moved.
- Both are drift between sittings, not the fault fixes. The pre-fix commit `1b869e5`, built from an export with the runner's flags and captured in one guarded sitting with the working tree, reads the same as the working tree: `churn` `flush` p50 2.72 and 2.67 ms against 2.72 and 2.73 ms, `gpu` `extract` p50 2.03 against 2.02 ms, 0 `regressed` and 0 `improved` owned verdicts (`perf-runs/20261001T182040Z-compare-churn/`, `-compare-churn-2/`, `-compare-gpu/`; the old build's captures are in `SA/ab-captures-1b869e5/`). The fixes leave the uncapped frame's calls as they were (`request_redraw`, then `ControlFlow::Wait`). No fix patch was needed; every later verdict is against `gpu-pass-a0`.
- 900-frame reference `perf-runs/20261001T182110Z-gpu/`: `total` p50 / p95 / p99 11.07 / 12.56 / 13.42 ms, max 42.80 ms, jitter 2.35 ms, 87.2 FPS, 8 spikes (frames 83 and 226, then every 120 frames from 419; frame 11 in two runs), peak RSS 904.7 MiB. The suite's `gpu` row: 87.8 FPS, 2 spikes per 300 (frames 83 and 226), max 23.34 ms, jitter 3.22 ms, peak RSS 721.1 MiB (`SA/logs/a0-suite-frame-stats.txt`).
- Counts (`SA/logs/a0-frame-counts-gpu.txt`, raw files `EV/counts/a0-gpu-*`): 20 passes, 78 draws, 31 bind groups, 38 buffers and 4,709 `malloc` calls per frame, equal to the baseline table. `gpu-throughput`: 2,150 faults, 9.44 ms of kernel and 17.19 ms of user time per frame (`SA/logs/a0-rusage-gpu-throughput.txt`; 1,606 / 8.98 / 18.02 at planning).
- Patch: `SA/patches/00-a0-preflight.patch` (this file only).

### A1. Text pipeline: bounded cache, no wasted shaping, no work for unchanged text

Motivation: C2, C3. Sources: S13, S14.

Change, in `TextPipeline::prepare` and `post_frame`:

1. Look up before building. Build the key once per section and look it up before any `Buffer` exists. On a miss take a recycled `Buffer` or `Buffer::new_empty(metrics)`, then `set_metrics`, `set_size`, `set_text` and `shape_until_scroll`. `Buffer::new` must not be called: it shapes an empty line.
2. Bound the cache. Replace the 360-frame TTL and the 120-frame prune with per-frame aging: an entry unused for more than 2 frames leaves the map and its `Buffer` goes to a spare list capped at 256. The map then holds a few entries per live section, never rehashes upward in steady state and never drops thousands of buffers in one frame.
3. Skip unchanged frames. Keep last frame's per-section signature (key, position, color, clip bounds) and viewport. When this frame's sections equal it (compare before cloning anything), skip `text_renderer.prepare`: glyphon keeps its vertex buffer and atlas between prepares, and nothing else writes the atlas. `atlas.trim()` only clears glyphon's in-use set (S13), so skip it on those frames too.
4. `load_font` and `reload_font` clear the map, the spare list and the signature.

Keep the cache logic in a device-free type so tests need no GPU: load `assets/fonts` bytes into an empty font database.

Tests (new `tests/text.rs`): the map stays at or below 3 × sections over 1,000 frames of changing text; an unchanged frame reports a skipped prepare and a changed position or color does not; text evicted and shown again lays out to the same glyph count; a font reload empties every cache.

Expected gain (estimates, to verify): `gpu` `render_encode` p50 2.65 → about 2.2 ms, from `Buffer::new`'s 4.5–6.7% of main-thread samples and the allocation traffic of 100 inserts per frame; static text from 0.73 ms to near 0; the 9–14 ms and 32 ms frames gone; peak RSS below 300 MiB. Shaping itself (32.0% of samples) stays.

Risk: stale text. The `visual` preset covers static text only, so also run `TUNGSTEN_BENCH=gpu cargo run --release -p example-02-bench` and an example with the HUD on (`F4`) and check that changing text updates every frame.

Verify:

```bash
$G just perf suite --repeat 5 --compare gpu-pass-a0        # then save as gpu-pass-a1
$G just perf run gpu --frames 900 --repeat 3 --gpu-timing off
$G just perf run gpu --repeat 3 --set sprites=0 --set glow=0 --set lights=0 --set tile_layers=0 \
  --set post=none --set aa=off --compare perf-runs/20261001T152010Z-gpu-set1cdab1
$G just perf run gpu --repeat 3 --set sprites=0 --set glow=0 --set lights=0 --set tile_layers=0 \
  --set post=none --set aa=off --set text_change=0 --compare perf-runs/20261001T152019Z-gpu-set3fadd5
bash $E/scripts/frame_counts.sh a1-text-static gpu default "sprites=0,glow=0,lights=0,tile_layers=0,post=none,aa=off,text_change=0"
```

Done when: `gpu` `stage.render_encode` p50 and p95 read `improved`; no owned metric of any row reads `regressed`; `frame_stats.py --spikes` shows no spike in the suite's `gpu` runs or in the 900-frame runs and the 900-frame maximum is at most 16.5 ms; the 900-frame peak RSS is at most 300 MiB; the static-text capture's `render_encode` p50 is at most 0.10 ms (0.51 before) and its `malloc` count at most 300 per frame (1,217 before); `just visual` and `just smoke` pass.

**A1 result (2026-10-01, 18:25Z–18:55Z): kept; one check unmet (static-text `malloc` count 406 against ≤ 300).**

- Change (`crates/tungsten-render/src/text.rs`, tests in `src/tests/text.rs`): the fonts, the layout map, the spare list and the last frame's signature live in the device-free `TextLayoutCache`. A section is looked up before any buffer exists; a miss shapes into a recycled buffer or `Buffer::new_empty`. Aging runs at the start of each prepared frame, so the map peaks at three frames of sections; evicted keys and buffers go to a spare list of at most 256. A frame whose sections and viewport equal the last prepared one skips glyphon's prepare and the atlas trim and ages nothing; a failed prepare is retried the next frame. A zero font size takes a new buffer, because `Buffer::set_metrics` panics on one where `Buffer::new_empty` does not.
- Suite `perf-runs/20261001T183419Z-suite/`, saved as `gpu-pass-a1`. Against `gpu-pass-a0` (`perf-runs/20261001T183911Z-compare-suite/`): 0 `regressed`, 10 `improved`, 32 `unchanged`, 13 `noisy`; no encoder sighting.
  - `gpu`: `render_encode` p50 2.72 → 2.22 ms and p95 3.15 → 2.49 ms, both `improved`; `extract` p50 2.00 → 1.30 ms and p95 2.35 → 1.56 ms, both `improved`; medians of the five runs: `total` p50 / p95 / p99 11.04 / 12.61 / 14.22 → 11.02 / 12.46 / 13.18 ms; jitter 3.22 → 2.16 ms; 0 spikes (2 before); max 23.34 → 13.89 ms; 87.8 → 88.0 FPS (the row waits on the GPU: `render_acquire` p50 5.84 → 7.03 ms takes up what the CPU stages gave back); peak RSS 720.9 → 169.7 MiB.
  - `gpu` scene pass p50 7.49 → 6.90 ms and `render_span` p50 11.13 → 10.57 ms read `improved`: all five diagnostic runs sit in the faster of the two modes (6.90–6.91 ms), where `gpu-pass-a0` had one run of five there. Not claimed for A1 unless later suites repeat it.
  - `integrated`: `total` p50 10.59 → 9.77 ms and p95 11.25 → 10.36 ms, both `improved`; p99 11.56 → 10.64 ms (`noisy`, Δ −0.92 against τ 0.925); jitter 0.97 → 0.87 ms; 94.9 → 103.8 FPS; `extract` p50 2.85 → 2.14 ms; `render_encode` p50 0.94 → 0.95 ms (its tags move every frame, so glyphon still rebuilds their vertices); peak RSS 271.8 → 169.1 MiB.
  - The `extract` gains are the heap: the old cache took a little memory every frame, so the extract's vectors were allocated on fresh pages each frame. Faults per frame fell 387 → 0 in `gpu` (kernel time 0.96 → 0.15 ms) and 61 → 0 in `integrated` (`SA/logs/a1-rusage.txt`).
  - `churn` `stage.flush` p50 2.71 → 2.54 ms and p95 3.05 → 2.89 ms read `improved` although the row draws no text. It is code alignment: the A0 tree, built in `SA/refbuild/st-a0/` and captured in one guarded sitting with the A1 tree, reads 2.71 and 2.70 ms against 2.53 and 2.54 ms (`SA/logs/a1-ab-churn.log`), and `World::flush_reusing` has the same size in both binaries but starts 112 bytes earlier, on a 64-byte boundary, in the A1 one. A later step can move it back; an A/B of the two builds separates that from a real change.
- 900 frames, `perf-runs/20261001T184001Z-gpu/`: 0 spikes in three runs (8 before), `total` p50 / p95 / p99 11.02 / 12.45 / 12.86 ms, max 13.90–14.48 ms (42.80 before), jitter 1.85 ms, 88.1 FPS, peak RSS 170.4 MiB (904.7 before).
- Text alone, changing (`perf-runs/20261001T184036Z-gpu-setfb6144/`, compare `perf-runs/20261001T184041Z-compare-gpu/`): `render_encode` p50 2.32 → 1.25 ms and p95 2.47 → 1.34 ms, `improved`. Static (`perf-runs/20261001T184042Z-gpu-set0a402f/`, compare `perf-runs/20261001T184045Z-compare-gpu/`): `render_encode` p50 0.51 → 0.03 ms and p95 0.67 → 0.05 ms, `improved`.
- Unmet: the static-text frame makes 406 `malloc` calls (1,217 before; `SA/logs/a1-frame-counts-text-static.txt`). The empty frame makes 205 (`SA/a1-floor-alloc-*.txt`, unchanged) and the benchmark's own `extract_text` 201 (two strings per section and the vector), so the text pipeline's share is 0 (811 before) and no change within A1's files lowers the count; a second attempt had nothing to remove. Changing text: 3,615 → 1,613 `malloc` and 915 → 310 `realloc` calls per frame (`SA/a1-dyn-alloc-*.txt`). Buffers created on the static frame: 4 → 3.
- Pixels: 19 screenshots of fixed frames (`gpu` at frames 5, 150 and 400, where every section shows the frame number; static and half-changing text; `min`, `visual`, `post=full`, `aa=off`, `msaa4`, floor, `zoom=0.5`, 720p, `bloom_mips=1`, `throughput`, `particles`, `integrated`, `physics`) are byte-equal to the unmodified build's (`bash SA/shots.sh --cmp head-1 a1`; the unmodified build repeats itself byte for byte). The systems overlay, inspector, name tags and frame counter render and update (`SA/shots/a1-overlays/`). `cargo test -p tungsten-render` (98 + 6 tests), strict clippy, `just smoke` and `just visual` pass.
- Not verified: the debug HUD toggled with `F4`, which needs a key press.
- Patch: `SA/patches/01-a1-text-pipeline.patch`.

### A2. Default extract and tilemap extract, output unchanged

Motivation: C1, C4, C5, C7. Sources: S19.

First pin the output: add a golden test that builds a seeded world (three z layers, mixed lit, material, override and parallax sprites, entities spawned and despawned so IDs are out of order) and asserts the exact batches and instances `extract_sprites_default` returns today. Every sub-change keeps it green.

Change, in `extract_sprites_default`:

1. One pass, internal iteration. Replace `filter_map(…).collect()` with `for_each` over the query, writing plain-data records into scratch vectors reserved to last frame's count. No borrowed tuples, so the scratch can persist.
2. Compact sort. Sort 16-byte keys (`z_order`, order-preserving, and the entity ID packed into a `u64`, plus a `u32` record index), not 48-byte tuples. Entity IDs are unique among live entities, so `sort_unstable` on that key equals the present stable order; skip it when the keys are already ascending.
3. Asset lookup. Replace the last-ID memo with a small direct-mapped cache keyed by a cheap hash of the ID bytes and verified by string compare; fall back to `AssetRegistry::get_sprite`.
4. Override key. Hash the 256 bytes with one `write`, not 256 `write_u8` calls.
5. Reuse. Keep the scratch vectors in a world resource with interior mutability, inserted by `App::new`; `extract_sprites_default` falls back to local vectors when it is absent. After `stage_render`, `App` hands the frame's `Vec<SpriteBatch>` back so the next frame reuses the instance vectors' capacity. The public extract signature does not change.

In `extract_tilemaps`: resolve each tileset entry once per map per call, and replace the per-layer `HashMap` with a small first-seen-order list (today's `into_values()` order is arbitrary when a layer spans two atlas pages; first-seen order is deterministic). Add tests for both.

Expected gain (estimates): `gpu-throughput` loses the per-frame heap growth and trim, up to the 8.98 ms of kernel time, plus `realloc` and `dealloc` (13.7% of user samples); `particles` loses most of the sort (29.0%) and of the string hashing (10.7%); `gpu` `extract` loses most of the tile lookups' 0.40 ms (C7) and the mixed-kind lookups.

Risk: order equivalence and cache collisions, both covered by the golden test; a `RefCell` borrow held across a nested extract call (custom extracts call the default one once, so borrow only inside it).

Verify:

```bash
$G just perf suite --repeat 5 --compare gpu-pass-a1        # then save as gpu-pass-a2
python3 $E/scripts/rusage_slope.py gpu-throughput gpu throughput "" 100 300
for f in 100 200; do bash $E/scripts/count-addrs.sh $S/a2-brk-$f.txt $f gpu throughput "" sbrk=libc:sbrk brk=libc:brk; done
$G just perf run gpu --repeat 3 --sweep zoom=0.5,1,2       # tile extract at four times the tiles
just perf compare perf-runs/20261001T151539Z-gpu-sweep-zoom/zoom-0.5 perf-runs/<UTC>-gpu-sweep-zoom/zoom-0.5
```

Done when: `gpu-throughput` `stage.extract` p50 and p95 read `improved`; `gpu` `stage.extract` p50 reads `improved` or `noisy` with a negative delta; no owned metric of any row reads `regressed`; `gpu-throughput` shows at most 1.0 ms of kernel time and 50 faults per frame, and at most 0.1 `brk` calls per frame (5.0 before); the golden test and `cargo test -p tungsten` pass; `just visual` and `just smoke` pass. Report `particles` and `integrated` `extract` and `total` against the targets.

**A2 result (2026-10-01, 18:55Z–20:10Z): taken out after a second attempt and parked for the owner. One owned metric the change is responsible for reads `regressed`: `particles` `system.animate_sprites` p95, 0.37 → 0.40 ms.** The working tree is the A1 tree again; the step is in `SA/patches/parked/a2-extract-on-a1.patch` (applies after patch 01; tree in `SA/snap/st-a2-parked/`). Accepting it is the owner's call, as the same metric's last regression was (`D-084`).

- What was built, both attempts (`crates/tungsten/src/{sprite_extract,tilemap_extract,app}.rs`, tests beside them). One `for_each` pass writes an instance and a 16-byte key (`z_order`, entity ID, record index, batch class) per sprite into scratch vectors; the keys are sorted unless they arrive in painter order; a second pass gathers the instances batch by batch. Asset IDs resolve through the last-ID memo and a 64-slot direct-mapped cache. The scratch lives in a world resource (`ExtractScratch`, a `RefCell`) that `App::new` inserts; after `stage_render` the frame's batches go back to it, and their instance vectors are pooled by capacity class, so a batch gets a fitting vector whatever order a custom extract builds batches in. The tilemap extract resolves a tileset entry once per map and opens a layer's batches in first-seen order. The second attempt added: a frame that is one batch in query order hands its record vector over as the batch (no second pass); a radix sort above 1,024 keys; spans of equal z and class gathered at once.
- Output: a golden test keeps the old extract as the reference and compares every batch and instance on seeded worlds (three z layers, lit, material, override and parallax sprites, hidden ones, despawns and respawns), with a pinned digest (69 batches, 420 instances, `0x7cec8f233d8bfd57`, taken from the old extract before any edit). The 19 screenshots are byte-equal to the unmodified build's for both attempts; `cargo test -p tungsten` (160 unit tests), strict clippy, `just smoke` and `just visual` passed.
- Second attempt, suite `perf-runs/20261001T192138Z-suite/` against `gpu-pass-a1` (`perf-runs/20261001T192526Z-compare-suite/`): 3 `regressed`, 10 `improved`, 29 `unchanged`, 13 `noisy`; no encoder sighting.
  - `gpu-throughput`: `stage.extract` p50 24.30 → 13.08 ms and p95 24.85 → 13.45 ms, `improved`; `total` p50 / p95 / p99 26.07 / 26.61 / 27.07 → 14.53 / 15.00 / 15.38 ms; 38.4 → 68.6 FPS; jitter 0.84 ms; kernel time 8.59 → 0.20 ms and faults 1,596 → 0 per frame, `brk` 5.0 → 0.0 and `sbrk` 7.0 → 0.0 calls per frame (`SA/logs/a2b-rusage.txt`, `SA/a2-brk-*.txt`); peak RSS 242.4 → 227.0 MiB.
  - `gpu`: `stage.extract` p50 1.30 → 0.66 ms and p95 1.56 → 0.90 ms, `improved`; `total` unchanged (it waits on the GPU). At `zoom=0.5` `extract` p50 is 1.18 ms against 3.00 ms at planning, A1's share included (`perf-runs/20261001T195732Z-gpu-sweep-zoom/`, compare `perf-runs/20261001T195936Z-compare-gpu/`).
  - `particles`: `total` p50 / p95 / p99 11.02 / 11.64 / 12.06 → 6.14 / 7.07 / 7.61 ms, 90.4 → 159.0 FPS, `extract` p50 7.71 → 2.93 ms, jitter 1.19 → 1.46 ms (target ≤ 1.1: the slow frames are in the extract's first pass, 2.1 ms at p50 and 3.4 ms at p99); owned `unattributed` p50 2.11 → 2.01 ms and p95 2.63 → 2.48 ms, `improved`; owned `animate_sprites` p95 0.37 → 0.39 ms, `regressed`.
  - `integrated`: `total` p50 9.77 → 8.61 ms and p95 10.36 → 9.27 ms, `improved`; p99 10.64 → 9.95 ms (`noisy`); jitter 0.87 → 1.34 ms (`unchanged` by its threshold; target ≤ 1.0); 103.8 → 116.6 FPS; `extract` p50 2.14 → 1.00 ms.
  - First attempt (`perf-runs/20261001T190211Z-suite/`, compare `perf-runs/20261001T190613Z-compare-suite/`): `gpu-throughput` `extract` p50 16.02 ms, `particles` `total` p50 6.72 ms, `integrated` 8.73 ms; `regressed`: `ecs` `bounds_wrap` and `churn` `flush` p50 (2.54 → 2.65 ms; `noisy` with 15 runs a side).
- The three `regressed` readings, each checked with 15 runs a side against the A1 tree built in `SA/refbuild/st-a1/` (same sitting, script `SA/ab15.sh`):
  - `particles` `animate_sprites` p95: 0.38 → 0.40 ms (+0.02, interval +0.01 to +0.03, τ 0.020) and 0.37 → 0.40 ms in a repeat; 0.37 → 0.40 ms against the A0 build too (`perf-runs/20261001T193342Z-compare-particles/`, `perf-runs/20261001T195507Z-compare-particles/`, `-compare-particles-2/`). p50 reads `unchanged` or `noisy` (+0.01 to +0.02). It is the change: the old extract's second pass read every `Sprite` late in the frame and left the column in cache for the benchmark's first system of the next frame; the new extract reads it once, in its first pass. In a profile of both builds the system's extra samples sit on its first read of each `Sprite` (54 → 171 samples in 900 frames, the rest of the system equal). A no-op change that puts the system at the same code alignment as the A2 build leaves it `unchanged` (`SA/refbuild/st-a1-pad2/`, `perf-runs/20261001T194611Z-compare-particles/`), so it is not placement. The cost moves between stages: the row's `total` falls by 4.88 ms.
  - `ecs` `system.bounds_wrap` p50: 0.26 → 0.32 ms (`perf-runs/20261001T193342Z-compare-ecs/`). Code placement, not the change: the system's instructions are identical in both builds and it starts 32 bytes past a 64-byte boundary in the A2 builds (at 0 in the A0 build, at 48 in the A1 build). The A1 tree with a 76-byte function that no frame calls added to the benchmark crate, which puts the system at the same 32, reads 0.26 → 0.31 ms, `regressed`, with the same interval (`SA/refbuild/st-a1-pad/`, `perf-runs/20261001T194214Z-compare-ecs/`; A3 later rebuilt that tree with a 54-byte function, and the captures of this check moved to `perf-runs-pad32/` inside it). Any later step can move this system again; read a `regressed` on it against the alignment (`nm -C target/release/example-02-bench | rg 'systems::bounds_wrap$'`, address mod 64) before anything else.
  - `ecs` `system.stats_decay` p50: 0.19 → 0.29 ms in the suite, `noisy` with 15 runs (0.18 → 0.24, interval −0.00 to +0.11). The system has two per-run modes, 0.15 and 0.32 ms, in every build; the suite caught four slow runs of five against one of five.
- Not judged again: `churn` `stage.flush` reads 2.62 ms for the A1 build in the same sitting that read 2.67 ms for the A2 build (0 `regressed`, `perf-runs/20261001T193342Z-compare-churn/`); the 2.54 ms in `gpu-pass-a1` did not repeat.
- An encoder session of 4 s (19:36:49Z–19:36:53Z) overlapped runs 6 and 7 of one placebo `ecs` capture; it was recaptured and the overlapped capture renamed `…-OVERLAPPED-do-not-use`.
- After the restore, the tree was rebuilt (restored files need `touch`: `cp -p` keeps their old times and cargo then skips the build) and recaptured as `perf-runs/20261001T200403Z-suite/`, saved as `gpu-pass-a2`: it is the A1 tree, and reads 0 `regressed`, 0 `improved`, 40 `unchanged`, 15 `noisy` against `gpu-pass-a1` (`perf-runs/20261001T200854Z-compare-suite/`). A3 compares against it as written.
- Follow-up noted, not done: after this change the ID string compare is the largest cost left in `gpu-throughput`'s extract (about a quarter of the process's samples in a hand profile), which only interned sprite IDs remove (deferred candidates).
- Patch: `SA/patches/02-a2-extract-taken-out.patch` (this file only).

### A3. Per-frame GPU objects and the implicit surface clear

Motivation: C8, G2. Sources: S3, S5, S6, S9, S18.

Change:

1. Present pass. Give `tungsten_present_pass` a clear (`PassDesc::with_clear`), so the pass begins with `LoadOp::Clear`. The blit covers every pixel, so pixels do not change, and wgpu no longer inserts its own clear pass for the fresh surface texture. Update the pass-order tests and the doc comment that says the present pass never clears.
2. Target generation. Add a counter to `RenderTargetPool` that increases whenever `SceneTarget` is rebuilt; every cache below is keyed on it, which replaces "rebuild every frame to avoid stale views".
3. Bloom. Keep the 2 × mips uniform buffers and every bind group on `BloomPipeline`, per bloom slot, built when the generation, the slot's source and destination or the mip count change; write a uniform buffer only when `BloomParams` or sizes change.
4. Stock passes, SMAA, present blit. One cached source bind group per source target (`SceneColor`, `PostPing`, `PostPong`) shared by all stock effects; SMAA's four bind groups and the blit's one cached the same way.
5. Uniform writes. Skip a `queue.write_buffer` whose bytes equal the last bytes written to that buffer: the material block (written once per batch today), stock-pass parameters, camera and lights. This does not change what the two uniform-sharing P2s do: different payloads in one frame are still all written.
6. `stage_render`: stop cloning `PostStack` every frame.

Expected gain (estimates): bind groups created per frame 31 → at most 1 and buffers 38 → at most 8 in `gpu`; the cacheable share of the chain's 0.42 ms of CPU in `integrated` is unknown, expect at least 0.1 ms; one render pass and up to 0.17 ms of GPU time per frame removed by the clear.

Risk: stale views after a resize or a `post_aa` switch (generation test); two bloom slots in one stack (per-slot caches; add a test).

Verify:

```bash
$G just perf suite --repeat 5 --compare gpu-pass-a2        # then save as gpu-pass-a3
bash $E/scripts/frame_counts.sh a3-gpu gpu default ""
bash $E/scripts/frame_counts.sh a3-integrated integrated default ""
for f in 100 200; do bash $E/scripts/count-addrs.sh $S/a3-internal-$f.txt $f gpu default "" \
  'clear_via_passes=sym:wgpu_core::command::clear::clear_texture_via_render_passes$' \
  'staging_new=sym:wgpu_core::resource::StagingBuffer>::new$'; done   # per frame = (200-frame count − 100-frame count) / 100
python3 $E/scripts/gpu_usage.py floor gpu "" "sprites=0,glow=0,lights=0,tile_layers=0,glyphs=0,post=none,aa=off" --frames 6000
```

Done when: per frame in `gpu`, bind groups created ≤ 1, buffers created ≤ 8, render passes 19, implicit clears 0; no owned metric of any row reads `regressed`; `gpu` and `integrated` `render_encode` p50 read `improved` or `noisy` with a negative delta; a window resize and an SMAA toggle in `example-04-shader-playground` render correctly; `just visual` and `just smoke` pass. Record the floor's GPU time per frame against 0.58 ms.

**A3 result (2026-10-01, 20:10Z–21:00Z): kept after a second attempt, without the present-pass clear. Unmet: 20 render passes and 1 implicit clear per frame in `gpu` (19 and 0 asked).**

- Kept (changes 2 to 6): `RenderTargetPool` counts rebuilds (`generation()`), and `TargetCache` in `targets.rs` holds one value with the key it was built for. Keyed on the generation: bloom's uniform buffers and bind groups per post-stack slot (also on the slot's source, destination and mip count; a UBO is rewritten only when its bytes change), one source bind group per source target for all stock effects, SMAA's four bind groups, the blit's one. `Renderer::resize_targets` drops them all when the targets are reallocated, so the old textures are freed at once. Writes skipped when the buffer already holds the bytes: camera (one check for both camera buffers), lights, a material's block, a stock effect's parameters. `stage_render` borrows the `PostStack`. Signatures that changed: `PostStackRenderer::record_pass` and the `SmaaPipeline::record_*` functions take `&mut self` (the SMAA ones also the pool and the source target), `record_bloom_slot` takes the slot index, `Renderer::update_lights` takes `&mut self`.
- Taken out (change 1): the present-pass clear. With it (first attempt, `perf-runs/20261001T202109Z-suite/`, compare `perf-runs/20261001T202600Z-compare-suite/`, tree in `SA/snap/st-a3-attempt1/`) the counts read 19 passes and 0 implicit clears, but `gpu.present` p50, which `gpu` owns, read `regressed`: 0.32 → 0.46 ms (the same in `integrated`, where it is reported only), and `render_span` rose by the same 0.14 ms. The empty frame's GPU time did not move: 598.2 ms/s at 1,035.1 fps, 0.578 ms per frame, as before (`SA/logs/a3-gpu-usage-floor-with-clear.txt`). On this GPU `LoadOp::Clear` costs what wgpu's own clear pass cost; the change only moves that time into the timed pass. Skipping the clear takes `LoadOp::DontCare`, whose token is `unsafe` to obtain (`wgpu::LoadOpDontCare::enabled()`): it would be the engine's first `unsafe` outside a test, so a decision. B3 meets the same question for whichever pass ends up writing the swapchain.
- Second attempt, suite `perf-runs/20261001T202900Z-suite/`, saved as `gpu-pass-a3`. Against `gpu-pass-a2` (`perf-runs/20261001T203352Z-compare-suite/`): 3 `regressed`, 1 `improved`, 38 `unchanged`, 13 `noisy`; no encoder sighting.
  - `gpu`: `render_encode` p50 2.25 → 2.18 ms (`improved`), p95 2.57 → 2.53 ms (`noisy`); submit and present p50 0.36 → 0.33 ms; `total` `unchanged` (88.1 FPS); `gpu.present` and `render_span` `unchanged`.
  - `integrated`: `render_encode` p50 0.95 → 0.90 ms (`improved`; target ≤ 0.80), p95 1.05 → 1.00 ms; submit and present p50 0.44 → 0.40 ms; `total` p50 9.76 → 9.64 ms (`unchanged`); 104.7 FPS.
  - Counts per frame, `gpu` (`SA/logs/a3b-frame-counts-gpu.txt`, `SA/a3-internal-*.txt`): bind groups 31 → 0, buffers 38 → 5, staging buffers 26 → 5, render passes 20, implicit clears 1, `malloc` 4,709 → 1,928 (A1 and A3 together). `integrated` (`SA/logs/a3b-frame-counts-integrated.txt`): bind groups 32 → 0, buffers 42.3 → 16.3, passes 21, `malloc` 4,616 → 3,464. Empty frame: `malloc` 205 → 185; static text: 406 → 386 (A1's unmet check stays unmet).
  - Empty frame's GPU time: 596.0 ms/s at 1,035.1 fps, 0.576 ms per frame against 0.578 (`SA/logs/a3b-gpu-usage-floor.txt`).
- The three `regressed` readings are code placement, each reproduced by a change that does no work (the A1 tree plus one 54-byte function no frame calls, `SA/refbuild/st-a1-pad/`), 15 runs a side in one sitting:
  - `ecs` `system.brain` p50 2.17 → 2.26 ms. The system starts 16 bytes past a 64-byte boundary in the A3 build (48 in the A1 build). The A1 tree with the padding function, same alignment: 2.17 → 2.25 ms, `regressed` (`perf-runs/20261001T203900Z-compare-ecs/`); the A3 tree against the A1 tree: 2.17 → 2.25 ms (`-compare-ecs-2/`).
  - `churn` `stage.flush` p50 2.54 → 2.70 ms and p95 2.88 → 3.08 ms. The A1 build is the odd one out: in one sitting the A1 build reads 2.53 ms, the A1 tree with the padding function 2.63 ms (`regressed`, `perf-runs/20261001T204208Z-compare-churn/`), the A0 build 2.68 ms and the A3 tree 2.67 ms (`unchanged` against the A0 build, `perf-runs/20261001T204209Z-compare-churn/`). The `improved` this row read at A1 was the same effect in the other direction.
- Resize and SMAA toggle, scripted instead of by hand: copies of the tree before and after A3 (`SA/refbuild/st-a2-h/`, `st-a3-h/`) got the same edit to example 04 (`SA/harness-edit.py`): SMAA off at frame 100, on at 200, window to 1600 × 900 at 300, SMAA `ultra` at 700, window to 1024 × 600 at 800, through `request_post_aa` and `request_display_settings`. Screenshots at frames 50, 150, 250, 650, 750 and 1,150 for three post stacks (every stock effect; bloom only; two bloom slots with a vignette between them) are byte-equal before and after, 18 of 18, and each tree repeats itself (`SA/harness-run.sh`, `SA/shots/harness-*`).
- Pixels: the 19 screenshots are byte-equal to the unmodified build's (`bash SA/shots.sh --cmp head-1 a3b`). `cargo test -p tungsten-render -p tungsten`, strict clippy over the workspace, `just smoke` and `just visual` pass.
- Not verified: a resize by dragging the window and the playground's SMAA keys by hand; the harness drives the same two requests.
- Patch: `SA/patches/03-a3-per-frame-gpu-objects.patch`.

### A4. Close-out of Session A

1. `just perf compare gpu-pass-a0 gpu-pass-a3` and fill a results table here: every row of "Targets", baseline of this session, result, verdict.
2. `$G just perf run gpu --frames 900 --repeat 3 --gpu-timing off`, `$G just perf capacity gpu integrated --budget 60hz` and `--budget 144hz`.
3. `just check`, `just smoke`, `just visual`, `just repo-check`.
4. Docs: in `docs/perf/benchmarks.md` update the `gpu` and `gpu-throughput` calibrated numbers, the "Text-cache RSS" and "Default extract" engine findings and the `gpu` bottleneck note (P6); in `docs/perf/profiling-workflow.md` the memory note on the text cache. Add one `DECISIONS.md` entry recording the session's changes and results, with its index row, and a `CHANGELOG.md` entry under `[Unreleased]`.
5. Add a dated "Session A complete" line under the header; the status stays `in progress` until B7. Stop the load log.

Done when: the plan's done-when holds for Session A's part and the results table is filled.

**A4 result (2026-10-01, 20:50Z–21:35Z): done.** The tree holds A1 and A3; A2 is parked. One owned metric reads `regressed` against `gpu-pass-a0` and has its reason recorded: `ecs` `system.brain` p50, code placement (below and `D-085`). The targets that depended on A2 are missed and say so in the table.

Final suite `gpu-pass-a3` (`perf-runs/20261001T202900Z-suite/`) against `gpu-pass-a0` (`perf-runs/20261001T180403Z-suite/`), report `perf-runs/20261001T205125Z-compare-suite/`: 1 `regressed`, 8 `improved`, 36 `unchanged`, 10 `noisy` owned verdicts. No A4 capture overlapped an encoder session. Values are medians of the runs (`SA/logs/a4-a0-frame-stats.txt`, `SA/logs/a4-a3-frame-stats.txt`); verdicts are compare's, on per-run means. "With A2" is the parked step on this tree, from the suite described further down.

| Row | Metric | Session baseline (`gpu-pass-a0`) | Result (`gpu-pass-a3`) | Verdict | Session A target |
| --- | --- | --- | --- | --- | --- |
| `gpu` | FPS | 87.8 | 88.1 | `total` `unchanged` | not lower: met |
| `gpu` | `total` p50 / p95 / p99 | 11.04 / 12.61 / 14.22 | 11.04 / 12.43 / 13.22 | `unchanged` / `unchanged` / `improved` | ≤ 11.03 / ≤ 12.56 / ≤ 13.8: p50 0.01 over, equal to the session baseline (the row waits on the GPU); p95 and p99 met |
| `gpu` | jitter | 3.22 | 2.18 | not judged | ≤ 2.8: met |
| `gpu` | spikes per 300 frames; per 900 frames | 2; 8 | 0; 0 | not judged | 0; 0: met |
| `gpu` | max `total`, 300 and 900 frames | 23.34; 42.80 | 14.41 (largest of five runs 14.78); 13.64 (largest of three 15.09) | not judged | ≤ 16.5: met |
| `gpu` | `render_encode` p50 / p95 | 2.72 / 3.14 | 2.19 / 2.53 | `improved` / `improved` | ≤ 2.30 / ≤ 2.60: met |
| `gpu` | `extract` p50 | 1.99 | 1.28 | `improved` | ≤ 1.50: met |
| `gpu` | peak RSS, 300 and 900 frames | 720.9; 904.7 MiB | 168.9; 168.7 MiB | `improved` | ≤ 300 MiB: met |
| `gpu-throughput` | FPS | 38.3 | 38.6 | `total` `unchanged` | ≥ 55: missed, A2 parked (68.9 with A2) |
| `gpu-throughput` | `total` p50 / p95 / p99 | 26.14 / 27.00 / 27.27 | 25.90 / 26.51 / 26.84 | `unchanged` | ≤ 18.0 / ≤ 19.0 / ≤ 19.5: missed (14.48 / 14.91 / 15.30 with A2) |
| `gpu-throughput` | jitter; spikes | 1.13; 0 | 0.98; 0 | not judged | ≤ 1.5; 0: met |
| `gpu-throughput` | `extract` p50 / p95 | 23.99 / 24.42 | 24.30 / 24.83 | `unchanged` / `unchanged` | ≤ 16.0 / ≤ 16.8: missed (13.05 / 13.43 with A2) |
| `gpu-throughput` | kernel time and faults per frame | 9.44 ms; 2,150 | 8.81 ms; 1,610 | not judged | ≤ 1.0 ms; ≤ 50: missed (0.20 ms; 0 with A2 on the A1 tree) |
| `particles` | FPS | 90.4 | 89.7 | `total` `unchanged` | ≥ 113: missed, A2 parked (160.7 with A2) |
| `particles` | `total` p50 / p95 / p99 | 11.02 / 11.69 / 11.97 | 11.05 / 11.80 / 12.23 | `unchanged` | ≤ 8.8 / ≤ 9.5 / ≤ 9.9: missed (6.08 / 7.05 / 7.48 with A2) |
| `particles` | jitter; `extract` p50 | 0.94; 7.70 | 1.12; 7.76 | not judged; `unchanged` | ≤ 1.1; ≤ 5.4: missed (1.45; 2.90 with A2) |
| `integrated` | FPS | 94.9 | 104.7 | `total` `improved` | ≥ 99: met |
| `integrated` | `total` p50 / p95 / p99 | 10.61 / 11.16 / 11.49 | 9.62 / 10.14 / 10.61 | `improved` / `improved` / `noisy` (Δ −0.92, τ 0.925) | ≤ 10.1 / ≤ 10.7 / ≤ 11.1: met |
| `integrated` | jitter; spikes | 0.92; 0 | 0.99; 0 | `unchanged` | ≤ 1.0; 0: met |
| `integrated` | `extract` p50; `render_encode` p50 | 2.84; 0.94 | 2.11; 0.90 | `improved`; `noisy` (`improved` against `gpu-pass-a2`) | ≤ 2.45: met; ≤ 0.80: missed by 0.10 |
| `integrated` | `render_span` p50 | 6.24 | 5.91 | `improved` | not regressed: met |
| floor | `render_span` p50; FPS | 0.41; 1,108.6 (planning capture) | 0.41; 1,100.4 | `unchanged`; `total` `unchanged` | not regressed: met |
| `gpu` at `min`, hand-run | p95 / p99; spikes per 300; FPS | 5.15 / 5.26; 31; 267.1 (planning capture) | 5.07 / 5.23; 24; 269.1 | not judged (hand-run) | not regressed: met |
| controls | `physics`, `physics-sparse`, `ecs`, `churn` owned metrics | `gpu-pass-a0` | `gpu-pass-a3` | `physics` 3 `unchanged`; `physics-sparse` 1 `unchanged`, 1 `noisy`; `ecs` 11 `unchanged`, 4 `noisy`, 1 `regressed`; `churn` 4 `unchanged`, 2 `noisy` | not regressed: met except `ecs` `brain`, with its reason below |

- Captures of this step: 900 frames `perf-runs/20261001T205132Z-gpu/` (`SA/logs/a4-gpu-900-frame-stats.txt`; reference `perf-runs/20261001T182110Z-gpu/`); floor `perf-runs/20261001T205612Z-gpu-setc52067/`, compare `perf-runs/20261001T205615Z-compare-gpu/`; `gpu` at `min` `perf-runs/20261001T205616Z-hand-a4-gpu-min-immediate-lat1/`; kernel time `SA/logs/a4-rusage-gpu-throughput.txt`.
- Capacity (`perf-runs/20261001T205207Z-capacity-60hz/`, `perf-runs/20261001T205420Z-capacity-144hz/`). At 60 Hz: `gpu` scale 1.61 (3,221 sprites, present-limited; 1.54 at planning), `gpu-throughput` 0.57 (227,758 sprites, `extract`-limited at 14.88 ms), `integrated` 1.53 (3,835 actors, `physics_step`-limited; 1.48). At 144 Hz: `gpu` not reachable (p95 8.69 ms with one sprite; 8.76), `gpu-throughput` 0.31 (124,186 sprites), `integrated` 0.085, present-limited ✗ (0.09).
- **`ecs` `system.brain` p50, the one `regressed`:** 2.19 → 2.26 ms (+0.07, interval +0.05 to +0.08, τ 0.066). Code placement, as A3 found against `gpu-pass-a2`. The system starts 16 bytes past a 64-byte boundary in the final build and reads 2.25–2.26 ms there, as does the A1 tree plus a 54-byte function no frame calls (`SA/refbuild/st-a1-pad/`, `regressed` against the A1 tree, 15 runs a side). At offset 0 (A0 build), 48 (A1 build) and 48 again (this tree plus A2, `SA/refbuild/st-a3-a2/`) it reads 2.17–2.19 ms. Five builds, two values, set by the offset; the system's instructions are the same in all. `churn` `flush`, which read `regressed` against `gpu-pass-a2` for the same reason, is `unchanged` against `gpu-pass-a0` (2.71 → 2.70 ms).
- **The scene pass's two modes are gone, and they were the old text path's.** `gpu` `stage.gpu` p50 7.49 → 6.90 ms and `render_span` p50 11.13 → 10.56 ms read `improved`. Per diagnostic run the scene pass p50 was 6.88–6.94 or 7.54–7.88 ms before A1 (9 of 15 runs of three suites in the slower mode) and is 6.87–6.92 ms in all 35 runs of the seven suites since (the A2 attempts, the first A3 attempt and the parked check included). On the unmodified build the slower mode never showed where the text cache barely grew (`text_change` 0 and 0.1, 0 and 400 glyphs: 12 of 12 runs at 6.85–6.87 ms) and always at 16,000 glyphs (7.68–7.71 ms; `perf-runs/20261001T150245Z-gpu-sweep-text_change/`, `perf-runs/20261001T150031Z-gpu-sweep-glyphs/`). `integrated`'s scene pass moved with it, 1.35 → 1.02 ms. It is a property of the diagnostic run: the timing runs' mean `total` is 11.39 → 11.36 ms, so no frame rate is claimed from it. The mechanism was not established (a diagnostic frame runs CPU and GPU in turn, so the GPU idled longer per frame behind the old text path; the cache's page traffic is the other candidate). For Session B this means a scene-pass change can now be judged across sittings: the caveat under "Measurement conditions" describes the build before A1.
- **`particles` tail.** Jitter reads 1.12 ms against 0.94 because two of the five runs hold one 15.8 ms frame whose extract took 12.1 ms (7.76 at p50). The same frame shows on every tree of the session: 1 of 25 runs of the A0 tree, 4 of 70 of the A1 tree, 3 of 20 of this one, at no fixed frame number (`python3 SA/particles_tail.py`). It is the extract's per-frame allocation (C1), not A1 or A3; the row's owned metrics read `unchanged`. With A2 the largest frame of 20 runs is 11.5 ms and of the suite's five 8.5 ms.
- **A2 on this tree (still parked).** `SA/patches/parked/a2-extract-on-a1.patch` applies after patch 03 or 04 with no offset. Built in `SA/refbuild/st-a3-a2/`: format, strict clippy and `cargo test -p tungsten -p tungsten-render` pass (`SA/logs/a4-parked-verify.log`), and the 19 screenshots are byte-equal to the unmodified build's (`bash SA/shots.sh --cmp head-1 a3-a2`).
  - Suite `SA/refbuild/st-a3-a2/perf-runs/20261001T210802Z-suite/` against `gpu-pass-a3` (`perf-runs/20261001T211155Z-compare-suite/`): 0 `regressed`, 10 `improved`, 31 `unchanged`, 14 `noisy` owned verdicts. `gpu-throughput` `extract` p50 24.27 → 13.06 ms and p95 24.80 → 13.43 ms; `integrated` `total` p50 / p95 / p99 9.64 / 10.27 / 10.64 → 8.54 / 9.15 / 9.62 ms, all `improved`; `gpu` `extract` p50 1.27 → 0.65 ms; `particles` `unattributed` p50 2.10 → 1.99 ms `improved`, `animate_sprites` p50 0.30 → 0.32 and p95 0.38 → 0.39 ms, both `noisy`. Medians are in the "with A2" notes of the table (`SA/logs/a4-parked-suite-frame-stats.txt`).
  - `noisy` got more repeats: 15 runs a side in one sitting (`perf-runs/20261001T211317Z-particles/` against `SA/refbuild/st-a3-a2/perf-runs/20261001T211233Z-particles/`, compare `perf-runs/20261001T211437Z-compare-particles/`) read `animate_sprites` p95 0.37 → 0.41 ms, `regressed` (+0.04, interval +0.02 to +0.05, τ 0.020), and p50 0.30 → 0.32 ms, `noisy`. The system starts at the same offset (48) in that build as in the A1 build, which reads 0.37–0.38 ms, so it is again the change and not placement. A2 stays out.
  - Also for the owner: with A2 peak RSS reads `regressed` by its own threshold in `particles` (133.8 → 136.8 MiB) and `integrated` (167.4 → 172.6 MiB), where the extract keeps its buffers, and falls in `gpu-throughput` (240.2 → 227.6 MiB); `particles` (`total` p95 7.05 ms) and `integrated` (9.16 ms) would run below their calibration bands. `SA/patches/parked/README.md` lists what accepting it takes.
- Gates on the final tree: `just check`, `just smoke`, `just visual` and `just repo-check` pass (`SA/logs/a4-check.log`, `a4-smoke.log`, `a4-visual.log`, `a4-repo-check.log`); no encoder ran before or after the GPU ones.
- Docs: `docs/perf/benchmarks.md` (dated `gpu`, `gpu-throughput` and `integrated` numbers, the bottleneck note, the scene-pass note, the text-cache and default-extract findings, a surface-clear finding, code placement under `ecs` and `churn`, what is left open), `docs/perf/profiling-workflow.md` (memory note, where a GPU-bound frame waits, the A/A note, a placement line in the regression policy), `DECISIONS.md` `D-085` with its index row, `CHANGELOG.md` under `[Unreleased]`.
- Noted and left (outside Session A's files or scope):
  - `examples/02_bench/src/integrated.rs`: the row note still says text grows RSS through the 360-frame cache. `docs/perf/benchmarks.md` flags the sentence.
  - `gpu`'s static-text `malloc` count is bounded by the benchmark's own `extract_text` (201 of 386 calls), which a harness change could lower; A1's check on it stays unmet.
  - The surface clear needs `LoadOp::DontCare`, so `unsafe`, or B3's direct swapchain pass; B3 meets the same choice.
  - B2 (culling) edits `sprite_extract.rs`: if A2 is accepted, apply the parked patch before B2 starts.
- For B0: capture a new session baseline as written. `gpu-pass-a3` is Session A's last suite; `SA/refbuild/` holds a built copy of each tree for same-sitting A/Bs (`SA/ab15.sh`, `SA/ref-run.sh`, `SA/ref-suite.sh`), and a `regressed` on an `ecs` system row or on `churn` `flush` should be read against the offsets first (`docs/perf/benchmarks.md`, `ecs`).
- The load log was stopped at the end of the session.
- Patch: `SA/patches/04-a4-closeout.patch`; the commit script is `SA/patches/commit-series.sh`.

## Session B: pacing, culling and the present path

### B0. Preflight

As A0, with baseline name `gpu-pass-b0`, compared against `gpu-pass-a3` if that baseline exists. Read the "Decisions for approval" table: skip every step whose row is not approved and say so in the report.

### B1. A frame-interval metric, spike count and maximum

Motivation: M2, M3. Sources: S17.

The pacing-override half of this step (M1) shipped on 2026-10-01 and is not repeated: the two `TUNGSTEN_DISPLAY_*` overrides in `crates/tungsten-core/src/config.rs`, the runner flags that set them, the `backend:` check (`present_problems` in `scripts/bench_report.py`), their tests, and the corrected "Frame pacing" section of `docs/perf/profiling-workflow.md`. What remains:

1. `crates/tungsten/src/app.rs`, `telemetry.rs`: measure the time from the previous frame's start to this frame's start and log it as `interval=…ms` in the `frame:` line; `FrameTimings` gains the field.
2. `scripts/bench_report.py`: list `interval` after `total` in the stage order, and add per-run spike count (frames above 1.5 × p50 of `total`) and `max` to the capture README. `interval` is reported; no row owns it.
3. Docs: the "Telemetry lines" section of `docs/perf/profiling-workflow.md`, and the `tungsten-perf` skill where it lists the `frame:` fields.

Risk: a longer `frame:` line changes allocation traffic, which moved `particles` and `integrated` readings before (`docs/perf/benchmarks.md`, `particles` section). That is why this step has its own suite.

Verify:

```bash
just perf-test && cargo test -p tungsten
$G just perf suite --repeat 5 --compare gpu-pass-b0        # then save as gpu-pass-b1
$G just perf run gpu --preset min --repeat 3 --gpu-timing off --present-mode fifo --max-frame-latency 2
$G just perf run gpu --preset min --repeat 3 --gpu-timing off --present-mode mailbox --max-frame-latency 3
```

Done when: both override captures are valid and their `backend:` line and README show the requested mode and latency (already so on 2026-10-01, `perf-runs/20261001T173002Z-gpu-min-fifo-lat2/`); the `fifo` capture's `interval` p50 is 16.6–16.7 ms (reference: `total` p50 16.61 ms in `perf-runs/20261001T153112Z-hand-gpu-min-fifo-lat1/`); no owned metric of any row reads `regressed` in the suite.

### B2. Extract culling (gated)

Motivation: C6.

Change: in the extract's single pass, after the parallax remap, drop a sprite whose bounding circle (half the diagonal of its scaled size, so rotation is covered) lies outside `CameraState::visible_world_aabb`. `z_norm` is computed over the kept sprites; relative order is unchanged. `RenderCounts::sprite_instances` then counts visible instances.

Tests: an off-screen sprite is absent; a rotated sprite straddling the edge is kept; a `ParallaxLayer` factor-0 sprite stays while the camera moves; draw order of kept sprites equals the unculled order.

Expected gain (estimate from C6's 7.8% of the level in view, plus the visible tiles): `integrated` instances 20,200 → about 6,000 per frame and `extract` to at most 1.7 ms. The fields of `gpu`, `gpu-throughput` and `particles` are fully on screen, so they pay the test and gain nothing.

Risk: a material shader that moves vertices outside the sprite's quad would pop at the screen edge; state the rule in the decision entry. The on-screen rows must not regress.

Verify:

```bash
$G just perf suite --repeat 5 --compare gpu-pass-b1        # then save as gpu-pass-b2
STAGING_SKIP=0 TUNGSTEN_BENCH=integrated TUNGSTEN_SMOKE_FRAMES=130 WGPU_BACKEND=vulkan RUST_LOG=error \
  gdb -q -batch -x $E/scripts/staging_sizes.gdb target/release/example-02-bench | rg '^[0-9]+$' | tail -n 40 | sort -n | tail -n 1
```

The last command prints the largest staging buffer of the final frames, which is the instance upload (startup texture uploads are larger, hence the `tail -n 40` first); it read 968,448–976,080 bytes on 2026-10-01 (`EV/counts/staging-sizes-integrated.txt`).

Done when: `integrated` `stage.total` p50 reads `improved`; `gpu-throughput` and `gpu` `stage.extract` do not read `regressed` (if `gpu-throughput` does, stop and report: the owner decides between accepting it in the decision entry and dropping the step); the instance upload of an `integrated` frame is at most 336,000 bytes (7,000 instances); `just visual` and `just smoke` pass.

### B3. Final pass to the swapchain (gated)

Motivation: G1, G2. Sources: S9, S20.

Change, on frames without a pending screenshot:

- With SMAA: the neighborhood pass targets the swapchain view; the text draws follow in a pass that loads the swapchain (it is initialized by then, so no implicit clear returns).
- Without SMAA and with a post stack: the last post pass, or bloom's composite when bloom is last, targets the swapchain.
- With neither and `msaa == 1`: the scene pass targets the swapchain; with `msaa > 1` the swapchain is the resolve target.
- On a capture frame the present path is the current one, so the screenshot source and its pixels do not change.

Formats already match: `SceneColor`, `PresentSource` and every stock pipeline use the surface format. Timing labels: `present` disappears on normal frames; update `query_count` in `timing.rs`, the `gpu` row's owned list in `examples/02_bench/src/gpu.rs` (drop `gpu.present`) and check how `scripts/bench_report.py` judges a metric one side lacks before comparing across this step.

Tests: pass-order tests for the four cases and for a capture frame; a pixel test that forces the capture path and the direct path to produce the same image.

Expected gain: the 0.32 ms blit at 1080p (0.14 ms at 720p, 1.22 ms at 2160p; G1) and one render pass per frame; an empty frame's span falls from 0.41 to about 0.1 ms (G2: scene 0.06 ms plus text).

Risk: frame-order invariants, the screenshot path, timing-label consumers. `gpu`'s `render_span` moves by 0.32 ms against a threshold of 0.34 ms, so it may read `noisy`; the judged captures are the floor and `integrated`.

Verify:

```bash
$G just perf suite --repeat 5 --compare gpu-pass-b2        # then save as gpu-pass-b3
$G just perf run gpu --repeat 3 --set sprites=0 --set glow=0 --set lights=0 --set tile_layers=0 \
  --set glyphs=0 --set post=none --set aa=off --compare perf-runs/20261001T152006Z-gpu-setc52067
python3 $E/scripts/gpu_usage.py physics physics "" "" --frames 900
```

Done when: `integrated` `gpu.render_span` p50 reads `improved`; the floor capture's `render_span` p50 is at most 0.15 ms and its FPS at least 1,400; no owned metric that still exists reads `regressed`; `just visual`, `just smoke` and a manual screenshot (`TUNGSTEN_CAPTURE_FRAME=5`) pass; `crates/tungsten-render/AGENTS.md` states the new frame order. Record the `physics` row's GPU time per frame against 1.47 ms (239 ms/s at 162.3 FPS in `EV/tables/gpu-usage.txt`).

### B4. Compositor-bypass hint on X11 (gated)

Motivation: P1. Sources: S10, S11, S12.

Change: after window creation, on X11 only, set `_NET_WM_BYPASS_COMPOSITOR` to 1 (CARDINAL, 32-bit) on the window through `x11rb` when `display.bypass_compositor` resolves to on. winit 0.30.13 has no option for it (S12). The standard property alone is enough for KWin (compositor table above), so the KDE-specific one is not needed. Log the applied state at startup.

Risk: desktop effects stop while the window exists and the screen flickers once at start and exit; captures taken with the hint are not comparable with captures taken without it, and compare cannot see the difference (M5), so the benchmark keeps the hint off unless the owner decides otherwise.

Verify with the hand-run wrapper, which writes the setting into its scratch `tungsten.json` (the runner clears `TUNGSTEN_DISPLAY_*` and has no flag for it). The 900-frame warm-up gives KWin time to suspend, as in the 2026-10-01 pairs:

```bash
$G python3 $E/scripts/pacing_run.py --label b4-off --bench gpu --preset min --present-mode immediate \
  --max-frame-latency 1 --warmup 900 --display-set bypass_compositor=false
$G python3 $E/scripts/pacing_run.py --label b4-on --bench gpu --preset min --present-mode immediate \
  --max-frame-latency 1 --warmup 900 --display-set bypass_compositor=true
python3 $E/scripts/frame_stats.py perf-runs/<UTC>-hand-b4-off perf-runs/<UTC>-hand-b4-on
python3 $E/scripts/series.py perf-runs/<UTC>-hand-b4-on run-1
```

Use the value syntax the setting ends up with if it is not a boolean.

Done when: with the hint on, `gpu` at `min` shows at most 2 spikes per 300 frames, p95 ≤ 3.8 ms, p99 ≤ 4.2 ms and FPS ≥ 288, and each run's `kwin_gfx_ms` in `capture.json` is below 10 (measured with `xprop` setting the same property: 1 spike, 3.60, 3.96, 293.3 and 3–4 ms in `perf-runs/20261001T165150Z-hand-comp-prop-net_wm_bypass_compositor/`); with it off, the numbers match `perf-runs/20261001T153749Z-hand-comp-control-gpumin/` within its run-to-run spread; `qdbus6 org.kde.KWin /Compositor org.kde.kwin.Compositing.active` prints `true` after the window closes; `just deps` passes.

### B5. Frame-cap interval check

The fix shipped on 2026-10-01 (P5), and its wall-time check is done: 360 frames take 6.22 s at a cap of 60, where they took 1.5–1.6 s (`perf-runs/20261001T173028Z-hand-cap60-fixed/` against `perf-runs/20261001T154001Z-hand-framecap60-gpumin/`). The frame interval was only measured with a remote-desktop client connected: p50 / p99 16.75 / 17.78 ms at 60 (59.7 FPS) and 7.05 / 8.45 ms at 144 (142.1 FPS), in `EV/fault-fixes/interval-fixed.txt`. Take it again on the quiet machine, once B1 logs `interval`:

```bash
$G python3 $E/scripts/pacing_run.py --label cap60 --bench gpu --preset min --present-mode immediate \
  --max-frame-latency 1 --frame-cap 60 --repeat 3
```

Done when: each run's wall time is at least 5.9 s for 360 frames, and `interval` p50 is 16.5–16.8 ms with p99 at most 17.7 ms. If p99 is higher, record a follow-up and do not add a dependency here: each deadline counts from the frame's own start, so the loop's wake-up time adds about 0.08 ms per frame (the 0.5–1.3% shortfall above), which a deadline grid kept across frames would remove, and a sleep-then-spin tail (S23) would tighten the rest.

### B6. Shape-run cache experiment (gated)

Motivation: C2 (`Shaping::run` is 22.0% of `gpu`'s main-thread samples, `EV/tables/profile-gpu-fp.txt`). Source: S14.

Change: depend on `cosmic-text` 0.19 directly with `shape-run-cache`, and call `font_system.shape_run_cache.trim(2)` once per frame in `prepare`. No other code changes.

Verify: `$G just perf suite --repeat 5 --compare gpu-pass-b3` (or the latest saved step).

Done when: keep it only if `gpu` `stage.render_encode` p50 reads `improved` and no owned metric reads `regressed`; otherwise revert and record the measured delta here. `just deps` passes if kept.

### B7. Close-out of Session B

As A4: results table against `gpu-pass-b0`, the 900-frame run, both capacity searches, the runner's pacing matrix for `gpu`, `integrated` and `gpu` at `min` (`--present-mode immediate`, `mailbox` and `fifo` at `--max-frame-latency 1`) for the record, `just check`, `just smoke`, `just visual`, `just repo-check`, `just script-test`, `just ctx` (the skill and `AGENTS.md` changed), `just deps`. Docs: `docs/perf/benchmarks.md` (rows, ownership table if B3 ran, engine findings), `docs/perf/profiling-workflow.md`, one `DECISIONS.md` entry per approved gated step with index rows, `CHANGELOG.md`. Set `status: done` and move this file to `docs/plans/archive/`.

## Deferred candidates (not scheduled)

| Candidate | Evidence | Why not now |
| --- | --- | --- |
| Fuse consecutive point-wise post passes (tonemap, vignette, color adjust, grain, fog) into one pass | Each costs 0.24–0.35 ms at 1080p (`perf-runs/20261001T150417Z-gpu-sweep-post/`); `integrated` runs composite 0.34 + tonemap 0.33 + vignette 0.26 ms. S20. | Needs shader composition; `D-058` keeps one WGSL file per effect and no preprocessor. Its own decision. |
| SMAA stencil mask and edge-pass `discard` | S15; ceiling 0.18 ms (G3). | Small against SMAA's 1.52–4.04 ms; the growth above the empty frame's 1.22 ms (`perf-runs/20261001T152030Z-gpu-set331eb6/`) is edge pixels, which a mask does not skip. |
| `Shaping::Basic` as an opt-in on `TextSection` | S14; shaping is 32.0% of `gpu`'s main-thread samples. | Public API and visible change (no ligatures or fallback); the benchmark's sections would have to opt in, which changes its work. |
| Per-batch uniform slots (dynamic offsets) | One material write per heavy batch (from the code; the staging count in C8 includes them); fixes both uniform-sharing P2s. S5, S18. | Owned by the known-issues list of `debug-cleanup-docs-pass.md`; design-level. |
| `StagingBelt` for uniform and instance uploads | 26 staging buffers per frame in `gpu` (`EV/tables/frame-counts-internal.txt`); `StagingBelt::finish_and_recall_on_submit` is new in wgpu 30 (S1, S5). | A3 removes most of them; measure what is left first. |
| Reused instance buffers for quads and debug lines | `QuadPipeline::draw` and `DebugLinePipeline::draw` create a buffer per call (code reading; S6). | No tracked row draws quads or debug lines, so no capture would verify the change. |
| Interned sprite IDs | `get_sprite` 10.7% in `particles`. | Open proposal in `docs/perf/benchmarks.md`; needs its own decision. A2's lookup cache takes the cheap part. |
| Half-resolution or trimmed overdraw sprites | Glow fill 3.95 ms (G4). S21. | A feature with visible trade-offs; the cost is the benchmark's by design. |
| Render scale below native | Post chain 18.4 of 27.9 ms at 2160p (`perf-runs/20261001T151057Z-gpu-sweep-resolution/`). | A feature; `display.scale_mode` is parsed but not applied (known issue). |
| Block for the GPU before `update`, not in the middle of the frame | Acquire waits 6.03 ms after `update` and `extract` in `gpu` (P6). S16. | Latency is not a metric of this pass and the suite cannot measure it. |
| Display-timed `dt` | `total` varies by 1.5 ms per refresh (P1). S16. | No capture can judge it: smoke mode pins `dt`. |

## Measurement gaps

Closed on 2026-10-01: pacing overrides (M1). Covered by a step: frame interval, spike count and maximum (B1). Left open, each a possible follow-up step:

- A non-blocking GPU timing path (a ring of query sets and readback buffers), so pass times come from the timing run and the diagnostic run's distortion goes away (M4, S22).
- Provenance fields for the GPU power level, compositor state and huge-page mode (M5), read by compare as soft notes.
- A visible-instance and batch count on the `frame:` or `bench:` line, so culling and batching need no debugger.
- RenderDoc is not installed; a capture of one `gpu` and one `integrated` frame would confirm the pass list that the call counts imply.
- Kernel-side attribution: `perf_event_paranoid` is 2, so profiles are user-mode only; C1's kernel share comes from rusage.

## Sources

Local sources are under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

- S1. wgpu changelog, v30.0.0 (2026-07-01) and v30.0.1 (2026-08-21): `Queue::present`, `LoadOp::DontCare` token, `StagingBelt::finish_and_recall_on_submit`. <https://raw.githubusercontent.com/gfx-rs/wgpu/v30.0.1/CHANGELOG.md>, <https://github.com/gfx-rs/wgpu/releases/tag/v30.0.0>
- S2. wgpu-hal 30.0.1, `src/vulkan/swapchain/native.rs`: image count is `maximum_frame_latency + 1` (line 214), the latency range starts at `min_image_count − 1` (171), acquire waits for the submission that last used its semaphore (447–464). wgpu issue 2869 on frame latency: <https://github.com/gfx-rs/wgpu/issues/2869>
- S3. wgpu-core 30.0.1: `src/command/render.rs` 1155–1177 (a `LoadOp::Load` attachment registers an init action; clear and store do not), `src/device/queue.rs` 125–185 (present transitions the surface texture with a submit of its own), `src/present.rs` 333–336. wgpu-types 30.0.1 `src/render.rs` 711–744 (`LoadOp` documentation).
- S4. Vulkan present modes: <https://docs.vulkan.org/refpages/latest/refpages/source/VkPresentModeKHR.html>
- S5. wgpu documentation: queue writes run at the next submit and use short-lived staging allocations (<https://docs.rs/wgpu/30.0.1/wgpu/struct.Queue.html>); `StagingBelt` (<https://docs.rs/wgpu/30.0.1/wgpu/util/struct.StagingBelt.html>); `SurfaceConfiguration` and `PresentMode` (<https://docs.rs/wgpu/latest/wgpu/type.SurfaceConfiguration.html>, <https://docs.rs/wgpu/latest/wgpu/enum.PresentMode.html>, read through context7 `/websites/rs_wgpu`).
- S6. wgpu wiki, "Do's and Dont's": do not create many resources per frame; few submits per frame. <https://github.com/gfx-rs/wgpu/wiki/Do's-and-Dont's>
- S7. Mesa environment variables (`MESA_VK_WSI_PRESENT_MODE`): <https://docs.mesa3d.org/envvars.html>
- S8. `vulkaninfo` on the reference machine: `EV/tables/vulkan-surface-caps.txt`.
- S9. Khronos Vulkan Samples: load and store operations (<https://docs.vulkan.org/samples/latest/samples/performance/render_passes/README.html>); swapchain images (<https://docs.vulkan.org/samples/latest/samples/performance/swapchain_images/README.html>).
- S10. EWMH `_NET_WM_BYPASS_COMPOSITOR`: <http://specifications.freedesktop.org/wm/latest/ar01s05.html>
- S11. SDL sets the bypass hint by default: <https://wiki.libsdl.org/SDL3/SDL_HINT_VIDEO_X11_NET_WM_BYPASS_COMPOSITOR>
- S12. winit 0.30.13: `src/platform_impl/linux/x11/window.rs` 1844 (`request_redraw` wakes the loop), `src/platform/x11.rs` (no compositor option); `ControlFlow` and `request_redraw` documentation, read through context7 `/websites/rs_winit_winit` (<https://docs.rs/winit/latest/winit/event_loop/enum.ControlFlow.html>).
- S13. glyphon 0.12.0: `src/text_atlas.rs` 72–105, 219–221 and 334–337 (`trim` clears the in-use set; eviction happens on allocation), `src/text_render.rs` 329–342 (vertex upload on every prepare).
- S14. cosmic-text 0.19.0: `src/buffer.rs` 383–411 (`Buffer::new` shapes an empty line; `new_empty` does not), `src/shape.rs` 28–82 and 416–474 (`Shaping::Basic`; the `shape-run-cache` feature), `src/shape_run_cache.rs`.
- S15. SMAA reference shader, integration notes (stencil mask for the second pass) and presets: <https://github.com/iryoku/smaa/blob/master/SMAA.hlsl>, <https://www.iryoku.com/smaa/>
- S16. Frame pacing: A. Ladavac, "The Elusive Frame Timing", GDC 2018 (<https://www.gdcvault.com/play/1025407/Advanced-Graphics-Techniques-Tutorial-The>); Android frame pacing (<https://source.android.com/docs/core/graphics/frame-pacing>, <https://developer.android.com/games/sdk/frame-pacing>); R. Levien, "Swapchains and frame pacing" (<https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html>); Unity, "Fixing Time.deltaTime in Unity 2020.2" (<https://unity.com/blog/engine-platform/fixing-time-deltatime-in-unity-2020-2-for-smoother-gameplay>); Themaister on Vulkan WSI (<https://themaister.net/blog/2018/09/09/the-state-of-window-system-integration-wsi-in-vulkan-for-retro-emulators/>).
- S17. PresentMon metric definitions (time between presents and between display changes): <https://github.com/GameTechDev/PresentMon/blob/main/README-ConsoleApplication.md>
- S18. WebGPU best practices: bind groups (<https://toji.dev/webgpu-best-practices/bind-groups.html>), buffer uploads (<https://toji.dev/webgpu-best-practices/buffer-uploads.html>).
- S19. Sort keys and sorting: bgfx internals (<https://bkaradzic.github.io/bgfx/internals.html>); Rust slice sorting notes (<https://doc.rust-lang.org/std/primitive.slice.html>).
- S20. Single-pass post-processing: Unity post-processing stack's uber shader (<https://docs.unity3d.com/Packages/com.unity.postprocessing@2.3/api/UnityEngine.Rendering.PostProcessing.PostProcessResources.Shaders.html>); J. Austin, "Fast Post-Processing on the Oculus Quest" (<https://johnaustin.io/articles/2022/fast-post-processing-on-the-oculus-quest>).
- S21. GPU Gems 3, chapter 23, "High-Speed, Off-Screen Particles": <https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-23-high-speed-screen-particles>
- S22. wgpu-profiler, timer queries without stalling the device: <https://github.com/Wumpf/wgpu-profiler>
- S23. spin-sleep, sleep accuracy and spinning: <https://github.com/alexheretic/spin-sleep>
