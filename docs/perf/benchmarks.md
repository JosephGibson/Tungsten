# Benchmarks

The rows `just perf` measures (`D-078` as redo 7.3 amends it), each owning one cost, and the counter guards that `just check` runs. `scripts/perf-rows.json` is the one registry: each row's package and folder, its launch, resolution, warm-up, frames, repeats, GPU runs, owned metrics, guards, known-drift metrics, budgets and `workload_version`. [`profiling-workflow.md`](profiling-workflow.md) covers running them, captures, sittings and verdicts. Dated results live in the captures under `perf-runs/` and in the history of this file; the levers below point at them.

```bash
just perf list                                   # every row, its kind and repeats
just perf run ecs                                # one capture
just perf sitting --quick                        # the working tree against the last tag, CPU rows, informational
cargo run -p example-02-bench                    # physics at `default`, interactive; Tab cycles the thirteen benchmarks
TUNGSTEN_BENCH=gpu TUNGSTEN_BENCH_PRESET=min cargo run -p example-02-bench
```

**Reference machine.** Ryzen 5 6600H, Radeon 660M (Mesa RADV `REMBRANDT`), Vulkan, `immediate` present mode with latency 1, governor `performance`, generic x86-64 release builds with frame pointers, every function and non-fallthrough block aligned to 64 bytes (`just perf`'s default flags). Its numbers are informational (`D-070`).

## Harness

- **Two kinds of row.** The thirteen benchmarks of `examples/02_bench/` (package `example-02-bench`) run one per launch, selected by `TUNGSTEN_BENCH` and `TUNGSTEN_BENCH_PRESET` and captured under `TUNGSTEN_SMOKE_FRAMES`, which pins dt to 1/60 s. The game rows run a game's own build in script mode (`D-225`) on an inline script the registry holds: setup steps, then the warm-up and the window, ending on the row's asserts; script mode pins dt too.
- **The bench binary.** `main.rs` resolves the configuration (`knobs.rs`: preset, then scale, then overrides, then validation) and builds the app through `bench_app`, which the counter guards build alike. `TUNGSTEN_BENCH_DESCRIBE=1` prints every benchmark's knobs and presets as JSON and `=config` the resolved configuration, before a window opens. `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` enables overlays. In an interactive run Tab relaunches the binary on the next benchmark, keeping the preset when it has it.
- **Determinism.** Every random choice comes from `Pcg32::seeded(seed)` and dt is pinned, so one build replays one workload. No rendered text depends on wall-clock time. A capture's runs share a digest over the window's `physics:` and `bench:` lines.
- **Assets.** `gen.rs` generates every bench texture under `bench_` IDs through a `TextureSink`: the renderer uploads them, and the counter guards' headless sink hands out the harness's handles and uploads nothing, so both register the same sprites, pages and batch keys. The bench's only asset files are `examples/02_bench/assets/manifest.json` and the `bench_heavy` shader and material it lists, which `gpu` loads after the root manifest; `boot` writes its own, seeded, under `target/bench-boot/`.
- **Lean view.** `view.rs` is a culled, unsorted extract over `Position` plus `ViewSprite`. `physics`, `ecs` and `churn` draw through it, so their render cost stays flat as they scale; `gpu` and `particles` use the engine's extracts.
- **Telemetry.** `counters.rs` logs `bench-config:` once and `bench:` counters every frame, on log target `bench`; a frame's `bench:` line prints in the next frame. The app logs one `startup:` line once loaded and presented (redo 7.9, `StartupTimings`): its phases and milestones in ms, `n/a` for one not measured, which `perf_stats.py` reads as one sample a run of `startup.<field>`.
- **Engine hook.** `Bench::engine_config` runs before the window opens; `gpu` applies `aa` and `bloom_mips` there unless `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_POST_AA` or `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` is set. The runner clears those variables.
- **Workload version.** A change to a row's work bumps its `workload_version` in the registry; engine changes never do. Every row is at version 1.
- **Peak RSS** is reported for every row and judged only between captures of one sitting.

## `physics` and `physics-sparse`

`physics` (pachinko) owns the narrow phase, contact build, solve, collision events and sleep bookkeeping; `physics-sparse` owns proxy gather, broadphase build and pair query. Warm-up 120 frames. Module: `physics.rs`.

- **Pachinko.** Balls fall through a staggered peg field into bins and recirculate: a bin floor teleports a ball to the spawn band on its first contact, `bumpers` of the pegs kick a touching ball to at least `kick`, and a ball below 30 px/s for 0.25 s gets a seeded kick and respawns at 0.4 s, so nothing sleeps. The board width follows `balls · mean area / (fill · 1,824)` in whole bins; the culled view draws only visible balls.
- **Sparse.** `bodies` circles of `radius` move at 600–1,400 px/s with no gravity and restitution 1 in an arena sized by `density`, their speeds renormalized every frame; about 6,660 are visible at zoom 0.5.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `mode` | `pachinko` | `pachinko`, `sparse` | | |
| `balls` | 8,000 | 200–400,000 | yes | Board width follows |
| `fill` | 0.36 | 0.05–0.40 | | Ball area ÷ board area |
| `radius_mix` | `mixed` | `uniform`, `mixed`, `wide` | | |
| `peg_radius` | 5 | 2–12 px | | |
| `bumpers` | 0.2 | 0–1 | | Share of pegs that kick |
| `kick` | 260 | 0–1,000 px/s | | |
| `gravity` | 900 | 100–3,000 px/s² | | |
| `restitution` | 0.3 | 0–1 | | |
| `bodies` | 8,000 | 1,000–2,000,000 | yes | Sparse only |
| `radius` | 2 | 1–8 px | | Sparse only |
| `speed_min`, `speed_max` | 600, 1,400 | 50–8,000 px/s | | Sparse only |
| `density` | 0.01 | 0.001–0.05 | | Sparse only |
| `substeps`, `iterations`, `cell` | 4, 1, 32 | | | `PhysicsConfig` fields |
| `sleep` | `on` | `on`, `off` | | |
| `render` | `culled` | `culled`, `none` | | |
| `seed` | 1 | u64 | | Every benchmark has it |

- **Presets:** `min` (1,000 balls), `default`, `sparse-min` (2,000 bodies) and `sparse` (the `physics-sparse` row).
- **Owned:** `physics`: `physics_step` p50/p95 and `update` p95. `physics-sparse`: `physics_step` p50/p95.
- **Guards:** `physics.sleeping <= 0` on both; `bench.teleports >= 1` on `physics`.

## `ecs`

Owns steady-state query iteration. Warm-up 60 frames. Modules: `ecs.rs`, `ecs/systems.rs`.

- **Components.** Up to 16 per entity (`Position`, `Vel`, `Acc`, `Heading`, `Health`, `Regen`, `Cooldowns`, `Brain`, `Team`, `Faction`, `Stats`, `Tint`, `Phase`, `Age`, `Follow`, `Bag`) and zero-sized tags that fragment the archetypes.
- **Systems,** each a different query shape: `brain`, `cooldowns`, `buffs`, `regen`, `stats_decay`, `accelerate`, `follow` (one random-access lookup per follower), `integrate`, `bounds_wrap`, `heading`, `tint`, `age_phase`, `team_bags` and `faction_histogram`, then `ecs_audit`. Nothing structural happens after startup.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `entities` | 250,000 | 1,000–4,000,000 | yes | |
| `fragmentation` | 8 | 1–256 | | Tag combinations |
| `optional` | `on` | `on`, `off` | | |
| `followers` | 0.1 | 0–0.5 | | |
| `systems` | `all` | `all`, `dense`, `sparse` | | |
| `view` | 4,096 | 0–65,536 | | Rendered sample |

- **Presets:** `min` (1,000 entities) and `default`.
- **Owned:** `update` p50/p95 and each of the 14 systems at p50.
- **Guards:** `bench.structural <= 0` and `bench.entities constant`.
- **Reference machine**, 2026-11-21: `update` p50 10.12 → 9.74 and 10.15 → 9.74 ms once a query's collection walks its shape's set bits and gives no entry for an archetype with no rows (7.14's C4, `ebc81a73`, `perf-runs/20261121T102443Z-sitting/`); the guard's window fell from 7,920 allocations and 35.6 MB to 1,860 and 11.7 MB.

## `churn`

Owns structural change: command recording, `World::flush`, archetype moves, and entity allocation and free. Warm-up 60 frames. Module: `churn.rs`.

- **Population.** A pre-aged population is replaced FIFO, each entity living `1/turnover` frames; each spawn inserts `components` components, applied as one move by the flush in `deferred` mode and one move each in `immediate` mode. Survivors gain and lose one of `statuses` status components on a one-frame cycle.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `population` | 125,000 | 1,000–2,000,000 | yes | |
| `turnover` | 0.05 | 0–0.5 | | |
| `toggles` | 0.05 | 0–0.5 | | |
| `components` | 6 | 2–12 | | |
| `statuses` | 3 | 1–8 | | |
| `mode` | `deferred` | `deferred`, `immediate` | | |
| `view` | 0 | 0–16,384 | | Rendered sample |

- **Presets:** `min` (population 1,000) and `default`.
- **Owned:** `flush` p50/p95 and `churn_scan`, `churn_despawn`, `churn_toggle`, `churn_spawn` at p50.
- **Guards:** `bench.population constant` and `bench.spawned == bench.despawned`.

## `gpu` and `gpu-throughput`

Own the render path: the default extract, encode, text shaping and every GPU pass. Warm-up 60 frames; each run has a GPU run beside its timing run. Modules: `gpu.rs`, `gpu/scene.rs`.

- **Scene.** `sprites` screen-locked sprites over `z_layers` interleaved layers (lit orbs and crates, `shader_share` drawn with `bench_heavy`, unlit textures), `glow` alpha-blended overdraw sprites, `lights` moving point lights plus a directional one, a render-only tilemap `tile_layers` deep under a ping-ponging camera, 100 text sections of which `text_change` change each frame, and the `post` chain with `aa` and `bloom_mips`. `bench-config` derives the expected batch count, which the startup checks.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `sprites` | 2,000 | 0–2,000,000 | yes | |
| `sprite_px` | 24 | 4–128 | | |
| `textures` | 4 | 1–8 | | |
| `z_layers` | 8 | 1–64 | | Drives the batch count |
| `lit` | 0.5 | 0–1 | | |
| `lights` | 16 | 0–16 | | |
| `glow`, `glow_px` | 20,000, 48 | | `glow` | |
| `shader_share`, `shader_iters` | 0.1, 32 | | | `lit + shader_share <= 1` |
| `tile_layers`, `tile_px` | 3, 16 | | | |
| `scroll`, `zoom` | 600, 1.0 | | | |
| `glyphs`, `text_change` | 4,000, 1.0 | | `glyphs` | |
| `post` | `light` | `none`, `light`, `full` | | |
| `aa` | `smaa_high` | `off`, `smaa_high`, `msaa4` | | |
| `bloom_mips` | 6 | 1–8 | | |
| `resolution` | 1920x1080 | up to 3840x2160 | | |

- **Presets:** `min`; `default` (the `gpu` row); `throughput` (the `gpu-throughput` row: 400,000 unlit 8 px sprites in one batch, nothing else); `visual`, which `just visual` compares with `examples/02_bench/tests/fixtures/gpu-visual.png`.
- **Owned:** `gpu`: the scene pass (`gpu.scene`) and `render_span` p50/p95; `post0_bloom_threshold`, `post0_bloom_composite`, `post1_vignette`, `smaa_edges`, `smaa_blend_weights`, `smaa_neighborhood` and `text` at p50; `extract` and `render_encode` p50/p95. `gpu-throughput`: `extract` and `render_encode` p50/p95.
- **Guards:** none. A GPU run needs 95 % of its window sampled on an adapter with timestamp queries.
- **GPU counters** (`just perf counters`): both rows pin `draw_calls`, `upload_bytes`, `text_renders` and `sprite_batches`.
- **`gpu-throughput`'s `extract` is half stall** (7.14's gate, 2026-11-21). The stage reads about 12 ms a frame (mean 11.890, p50 11.984 of a 13.662 ms `total` over the 0.118 release sitting's ten runs, `perf-runs/20261121T011258Z-sitting/`), but a run's first frame, with no GPU frame in flight, reads 5.5–6.7 ms and the second 12.2–12.9, the GPU's scene pass being 9.45 ms p50. In 9 of those 3,000 window frames the stall sat on the encode's copy of the same 19,200,128 instance bytes instead (6.17–7.11 ms against its 1.33 p50) with the frame's `total` unchanged: it follows the integrated GPU's pass onto whichever stage writes memory under it. With the fill cut to a quarter (`sprite_px=4`, the same 400,000 sprites) the extract's p50 reads 6.67–6.71 ms against 11.86–11.96, 0.56–0.57 of the row's, in three pairs on a quiet machine (`fill_cut.py` in `perf-runs/20261121-cpu-pass-plan/tools/`, `perf-runs/714-cpu-pass/step-1/fill-cut.log`). The ratio shows the reading follows the fill, not where the extract's own floor is, so the stage's reading is not a quantity a CPU fix can halve: 7.14's C6 is judged on a reading taken with the GPU idle.
- **The headless extract control** (7.14's second plan, 2026-11-21) is that reading. `the_extract_headless`, an ignored release test in `examples/02_bench/src/tests/guards.rs`, steps `gpu-throughput`'s scene (400,000 sprites in one batch) through the shared frame loop with no renderer (`D-110`), so no GPU pass shares memory with the extract: 60 warm-up frames, then 300 whose `FrameTimings::extract_ms` it prints as p50, p95 and lowest, with the frame before the window and the one after it checked to draw every sprite.
  - **Run:** five times in turn with the runner's flags, alone on a quiet machine with no capture running: `RUSTFLAGS="-C force-frame-pointers=yes -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6" CARGO_TARGET_DIR=target/perf/tree-aligned cargo test --release -p example-02-bench --locked the_extract_headless -- --ignored --nocapture`.
  - **Read:** a reading is the median of the five p50s, which outvotes the one run in five that lands in a slower mode. A candidate reads lower than a base when its median is and at least four of its five runs sit under the base's lowest.
  - **Why no row:** no windowed variant has the GPU idle. At one pixel a sprite (`sprite_px=4,zoom=0.25`) a GPU timing run still reads the scene pass 2–3 ms a frame (2.07–3.00, against 9.4 on the row), and the extract's p50 4.9 ms there against 12.0 on the row and 5.8 headless.
  - **Its milliseconds are its own:** the test binary compiles the extract's row loop to 250 instructions a sprite where the bench binary's is 217, so a base and a candidate are both read on the control and neither against a row.
  - **The first reading** (`d1c1ea22`'s tree with the control added): 5.833, 5.832, 5.791, 5.809 and 5.791 ms, median 5.809; a set on the same source before `rustfmt` moved one import read 5.789, 5.792, 5.788, 5.793 and 5.801, so two builds of one loop sit 0.3 % apart. `gpu-throughput` `stage.extract` read p50 11.94, 12.03 and 12.04 ms in a capture between them (`perf-runs/20261121T185742Z-gpu-throughput/`).
  - **After 7.14's C6** (2026-11-21, the next reading's base): the plain row loop read 3.568, 3.533, 3.547, 3.547 and 3.552 ms (`e2f0487c`, median 3.547) and the keyless frame 3.335, 3.337, 3.317, 3.314 and 3.308 (`a32172fe`, median 3.317), each beside five runs of its parent's build (`perf-runs/714-cpu-pass-2/step-3/`, `step-4/`).
- **`gpu`'s `render_encode` reads the scene pass inversely** (7.15, 2026-11-22). The stage's own work on this row is mostly text (1.74 ms mean of an 11.04 ms `total` at 7.15's gate, 0.23 with nothing drawn), but its reading rises as the GPU's scene pass falls. On one build the gate's light sweep read it 1.74 / 2.02 / 2.10 / 2.22 ms at `lights=` 16 / 8 / 4 / 0, the scene pass 6.93 / 6.22 / 5.79 / 5.36 (`perf-runs/715-gpu-pass/step-1/bench/`). The fragment's range test, a change of the lit shader's text alone, read it p50 1.664 → 1.950 and 1.686 → 1.938 ms, `regressed`, beside `gpu.scene` 6.942 → 6.330, `render_span` 10.432 → 9.822 and `total` 10.708 → 10.106, with the same 76 draw calls and the same upload bytes, p50 1,664,304 (`perf-runs/20261122T035647Z-sitting/`, and again in `perf-runs/20261122T040840Z-sitting/`): the acquire's wait fell 7.660 → 6.616 ms while `render_encode_pre_acquire` rose 1.600 → 1.886 and `render_submit_present` 0.504 → 0.652. So a change that shortens this row's GPU frame is judged on `total`, with the stage named as the one that reads the contention ([profiling workflow](profiling-workflow.md), "Judge relocated work on frame time"); judged on the stage, the range test stopped in 7.15's first plan; the second landed it, and the light skip after it, judged on `total` (known issues).
- **After 7.15's first plan** (2026-11-22, `02de1761`): `gpu` reads as before it, the scene pass p50 6.940 → 6.938 and 6.942 → 6.940 ms and `render_span` 10.428 → 10.418 and 10.420 → 10.424 against 0.118 at the plan's close (`perf-runs/20261122T045713Z-sitting/`). Each of its sixteen point lights reaches about half the view, so the light list by reach drops none, and its lit path, the scene pass less the same frame with lit batches drawn plain, reads 1.742 ms against 1.738 at the gate (`perf-runs/715-gpu-pass/step-2/gpu-g1.log`): G1's target on this row, at most 0.869, was open with the range test.
- **After 7.15's second plan** (2026-11-23, `dc89562f`): the lit path reads 0.832 ms, under G1's target. A lit fragment computes a point light's attenuation first and skips the rest of its term where that is 0 (`4fa9ea27`: the lit path 1.133 ms, the scene pass p50 6.938 → 6.334), and a quad's vertices mark the lights that cannot reach it, which its fragments pass over (`a0f4574c`: 0.832, the scene pass 6.326 → 6.014; off under MSAA). The split that led to the mark, the lit shader swapped by its asset file on one build (`perf-runs/20261122-gpu-pass-2-plan/bench/`): sixteen terms a fragment cost 1.573 ms and sixteen rejections 0.647, 41 % of a term each, with 35 % of the light and fragment pairs in reach, so after the range test alone the lit path was about 0.16 ms with no light, 0.55 of terms in reach and 0.42 of rejections. On the landed shader the light sweep reads the scene pass 6.022 / 5.694 / 5.521 / 5.366 ms at `lights=` 16 / 8 / 4 / 0 and 5.190 with lit batches plain, 0.175 of the lit path with no light (`perf-runs/715-gpu-pass-2/step-2/g1.log`). Against 0.118 at the plan's close the scene pass reads p50 6.938 → 6.016 ms, `render_span` 10.428 → 9.498 and `total` 10.710 → 9.796, with `render_encode` 1.660 → 2.020, the inverse reading above, accepted (`perf-runs/20261123T024118Z-sitting/`; known issues). The row draws under `cpu_stable` and its sprites are generated and unmeasured, so the depth sort and the clip move nothing on it.
- **`gpu-throughput`'s `render_encode` p95 follows the base's run mix** (2026-11-22). In the four sittings from 7.14's close on that hold the row, the tree's ten captures read it 2.75–3.46 ms, where `v0.118.0`'s read 1.72–2.44 in five captures and 5.55 in a sixth: `noisy` at 7.14's close (2.438 → 2.752 and 2.214 → 3.166), unmoved across the light list's commit (2.848 → 2.866 and 3.264 → 2.808, `perf-runs/20261122T030739Z-sitting/`), `regressed` at 7.15's first plan's close (1.720 → 2.910 and 2.200 → 3.460, `perf-runs/20261122T045713Z-sitting/`) and `ok` in that row's rerun (5.548 → 3.166 `noisy`, 1.980 → 2.842 `regressed`, `perf-runs/20261122T054937Z-sitting/`). Its p50 read `noisy` in all eight pairs, and the row's `extract` `improved` in every pair against `v0.118.0`.

## `particles`

Owns particle emit, tick and count refresh, and animation playback. Warm-up 180 frames. Module: `particles.rs`.

- **Emitters** move on Lissajous paths and draw on `configs` generated configs spanning every emission kind and velocity model, each targeting about 150 × `rate` live particles; bursts re-arm on their periods (`D-143`). **Animated sprites** play `clips` generated 8-frame clips on one atlas page. Both draw through the default extract, whose cost `gpu` owns.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `emitters` | 280 | 0–20,000 | yes | |
| `configs` | 12 | 1–64 | | |
| `rate` | 1.0 | 0.1–10 | | |
| `budget` | 100,000 | 1,000–5,000,000 | yes | `ParticleBudget::global_cap` |
| `animated` | 8,000 | 0–1,000,000 | yes | |
| `clips`, `anim_fps` | 8, 12 | | | |
| `motion` | `on` | `on`, `off` | | |

- **Presets:** `min` (8 emitters, 500 animated sprites) and `default`.
- **Owned:** `particle_count_refresh`, `particle_emit`, `particle_tick` and `animate_sprites`, each p50/p95 (`D-133`).
- **Guard:** `bench.live within ±10% of its median`.
- **Reference machine**, 2026-11-21, after 7.14's C3 (a clip's slot kept in `AnimationState`, `63c4fa78`): `animate_sprites` p50 0.232 → 0.102 and 0.236 → 0.104 ms and p95 0.278 → 0.172 and 0.280 → 0.166 in its sitting's two pairs (`perf-runs/20261121T192806Z-sitting/`), 0.240 → 0.104 and 0.242 → 0.100 ms against 0.118 at the pass's close (`perf-runs/20261121T224015Z-sitting/`), the row's digest unchanged.

## `terrain`

Owns the runtime terrain's share of `physics_step` (redo 7.8): the registry's writes and journal, the tile cache's chunk re-traces and the edited map's proxy rebuild, the pair grid's tile restage and the sensors' re-test of the cells written under them. Warm-up 120 frames. Module: `terrain.rs`.

- **Cave.** A seeded cave of 16 px cells under a sky band an eighth of the map's height: noise smoothed four times, cut by a two-column drain every 64 columns, in a collision layer and a styled art layer of 16 generated variants and a brush, auto-tiled by open sides. Each frame refills the cells it broke 60 frames before, then breaks `edits` floors (solid cells under air), a seeded one in each column of a seeded walk, in both layers, so from the 60th frame on the solid count holds. Circles of 3, 4.5 and 6 px fall on it and through the floors broken under them; one below the map respawns in the sky. Each of the `sensors` sensors is a column the map's height. The art layer draws through the engine's tilemap extract and the bodies through the lean view, the camera fitting the map.
- **Settling.** Bodies collect in the cave's pits slowly, the awake count falling from 1,200 to about 960 over a `default` capture, while `physics_step` holds: the edited map's proxy rebuild and the tile restage lead it (p50 5.35 ms, p95 5.77 ms on the reference machine, 2026-10-30; 4.26 and 4.49 ms on 2026-11-11, after the CPU stress pass).

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `width`, `height` | 512, 192 | 32–4,096, 32–1,024 | | Cells of 16 px, the sky's band included |
| `bodies` | 1,200 | 0–20,000 | yes | |
| `edits` | 32 | 1–256 | yes | Floors broken a frame in both layers; as many refilled |
| `sensors` | 4 | 0–16 | | |

- **Presets:** `min` (64 × 32 cells, 60 bodies, 4 edits, one sensor) and `default`.
- **Counters:** `bench:` `bodies`, `edits`, `refills`, `traced` (chunks traced that frame), `awake`, `respawns` and `visible`.
- **Owned:** `physics_step` p50/p95 and `update` p95.
- **Guards:** `bench.edits constant` and `bench.traced <= 256`, 8 × `edits`: each of a frame's 2 × `edits` collision writes re-traces at most the chunks within one cell of it.

## `saves`

Owns a world save's capture (redo 7.9): `capture_world` over named objects and a named spawner's runtime spawns, then `serde_json::to_vec`, each frame. Warm-up 60 frames. Module: `saves.rs`.

- **World.** One area of named objects with `Health`, `Stats` and `Tags`, and a named spawner's runtime spawns from an empty prefab, each a kinematic circle with a velocity; everything drifts across the viewport, bouncing off its edges, so each save holds changed positions. The kit's health, stats and tags plugins and the physics plugin build into a schedule the app never runs: their saved components register, their systems stay out of the frame. `save_snapshot` runs in `pre_update`, after the counters, where the command buffer is empty; the lean view draws everything.
- **Cost.** At `default`, 12,101 saved entities and about 1.61 MB a save, the 12,000 spawns one table (format 2, `D-295`): `save_snapshot` p50 4.8 ms, p95 4.9 ms (reference machine, 2026-11-21, ten runs: p50 12.99 → 4.85 and 12.94 → 4.82 ms in the two pairs of the table's sitting, `perf-runs/20261121T222526Z-sitting/`, the save 3,072,770 → 1,608,917 bytes and the run's peak RSS 95.6 → 74.1 MiB; 18.33 → 4.84 and 18.27 → 4.81 ms against 0.118 at the pass's close, `perf-runs/20261121T224015Z-sitting/`). Before the table a record a spawn was a `Value` tree, its text and its drop: 12.9 ms once the capture's own costs went (18.23 → 12.93 and 18.17 → 12.89 ms, `perf-runs/20261121T132058Z-sitting/`: the spawns read an archetype at a time, each record only the components its archetype holds, its error's label built on an error alone, its area found without cloning the name, no waiting reference looked up while none waits), 17.87 ms p50 at 7.14's gate, whose target for C5 was half that, 8.934 ms, and 26.5 and 29.2 ms on 2026-10-31 (a diagnostic sitting beside a remote desktop). What remains is a row's own work: each column's keyed read, its leaves' pushes and their text.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `objects` | 100 | 1–10,000 | yes | Named objects in the area |
| `spawns` | 12,000 | 0–100,000 | yes | The named spawner's runtime spawns; 12,000 is example 01's full pit |

- **Presets:** `min` (20 objects, 200 spawns) and `default`.
- **Counters:** `bench:` `saved` (records), `bytes` (the serialized save) and `visible`.
- **Owned:** `system.save_snapshot` p50/p95 and `stage.update` p95.
- **Guards:** `bench.saved constant` and `bench.bytes <= 3,080,192`, the first valid reading's 3,074,117 rounded up to 64 KiB. The bound stays format 1's though a format-2 save is 1,608,190 bytes, so a sitting against a base from before the table holds its guard on both sides.

## `boot`

Owns a boot on a game's assets (redo 7.9): the load plan's units in slices behind boot frames, from the window's opening to the first frame on the loaded game. Module: `boot.rs`.

- **Workload.** Before its run the bench writes seeded assets under `target/bench-boot/<preset>/` and their manifest: PNG sprites of 16 to 96 px, eight-frame sheets of 16 to 64 px frames and 16-bit mono WAV sounds, five in 117 a minute long and the rest a quarter second, with a boot group of the root manifest's `sans`, `green_circle` and `blue_square`; a stamp of the configuration skips the writing when the files stand. The app then boots on the root manifest and that one. The frames after it draw nothing of the bench's.
- **What it measures.** The `startup:` line (`StartupTimings`, one sample a run): `first_frame` and `loaded` from the window's opening, and `slice_max`, the longest boot slice; `boot_frames` and `slice_max` hang on the clock, so they stay on that line, outside the workload digest.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `sprites` | 2,700 | 1–20,000 | yes | PNG sprites, 16 to 96 px; 2,700 is example 01's scale |
| `sheets` | 40 | 0–1,000 | yes | Eight-frame sheets of 16 to 64 px frames |
| `sounds` | 117 | 0–2,000 | yes | 16-bit mono WAVs, five in 117 a minute long, the rest 0.25 s |

- **Presets:** `min` (64 sprites, 4 sheets, 4 sounds, none a minute long) and `default`.
- **Counters:** `bench:` `sprites`, `frames` (sheet frames), `sounds` and `units` (the load plan's), fixed by the seed.
- **Owned:** `startup.first_frame`, `startup.loaded` and `startup.slice_max`, p50.
- **Guards:** `bench.units constant`. No budget (9.1).
- **`slice_max` is the pack and the decode's remainder** (7.15's second plan, 2026-11-22). A boot slice runs units until 12 ms have passed, so the slice that holds the pack unit, about 60 ms, also holds what the decode units left of their last slice, between nothing and 12 ms: 2.4 ms at `a0f4574c`. A change that adds a millisecond to the decode moves the metric by as much, and one that adds it to the pack too: a byte-wise pass over every sprite's pixels inside the pack read `slice_max` p50 63.3 → 70.3 ms and `pack` 60.7 → 66.4, and the same pass as word arithmetic at the decode 63.7 → 63.9 (`perf-runs/20261122T201205Z-sitting/`, `perf-runs/20261122T210419Z-sitting/`). The pack also read 3.3 ms longer with the scene depth target unallocated at start-up, its atlas pages' uploads paying for it; no profile was taken (known issues' render F11).
- **The candidate's side reads apart from the base's** (the same plan's sittings). In all twelve judged pairs of the six sittings that hold the row the candidate read `startup.first_frame` p50 8 to 10 ms under the base's (93–94 against 102–104 ms), and in eleven of them its peak RSS 2 to 16 MiB over it, one commit's code among them read as a candidate and then as the next sitting's base. The reading follows the side, not the change; its cause was not looked for.

## `spread`

Owns chemistry's spread over a saturated field (redo 7.10b): `chemistry`'s run (the emissions through its grid, the queue's merge, its take of 256 and the builds it applies), `chemistry_tick` and `effect_tick`, whose instances count the emission clocks. Warm-up 60 frames. Module: `spread.rs`.

- **Field.** `entities` bodiless receivers tagged `flammable` on a square grid 24 px apart, every one lit at the start with `burning` (an intensity of max 4 and decay 0.5, sending `heat` 1 within 50 px every 0.25 s), and the rule `heat` on `flammable` building `burning` by 1, all inserted in code. Each receiver emits every 0.25 s, so the queue holds one `heat` entry per receiver and drains its cap of 256 a run, and no receiver goes out. The kit's tags, health, effects and chemistry plugins run in the app's schedule with their phases (no `KitPlugins::set()`); the lean view draws the dots.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `entities` | 12,000 | 1–100,000 | yes | Receivers on a square grid 24 px apart; 12,000 is the redo's spread row |

- **Presets:** `min` (200 receivers) and `default`.
- **Counters:** `bench:` `entities`, `burning` (receivers carrying it), `pending`, `dropped`, `cut` and `ran` (chemistry's queue and counts), read from the frame's final state.
- **Owned:** `system.chemistry`, `system.chemistry_tick` and `system.effect_tick` p50/p95, and `stage.update` p95.
- **Guards:** `bench.pending <= 16,384` (the queue's bound), `bench.dropped <= 0` and `bench.burning >= 12,000`, its first valid reading rounded down to a thousand: the whole field stays lit.
- **Reference machine**, 2026-11-21, after 7.14's C2 (its close sitting's ten runs against 0.118, `perf-runs/20261121T143639Z-sitting/`): `system.chemistry` p95 5.11 → 2.42 and 5.10 → 2.44 ms, p50 0.20 → 0.14, and `stage.update` p95 6.62 → 3.87 ms, the digest unchanged. An emission step's frame, one in fifteen, read 4.0 ms of `chemistry` before the last step: the grid's scan 1.6, the spread's merges 1.0, the queue's index probes 0.9, the cells' sort 0.19 (`perf-runs/714-cpu-pass/step-9/spread-hot-before.txt`). The grid's cell directory, packed centres and squared test took 1.1 ms of the p95 (`f2e4597b`), and a receiver's kept slot as a hint across runs, compact entity buckets, the cells' order kept and the take keyed to its cut-off age 1.5 (`c33f83f7`).

## `rooms`

Owns a live room's patch (redo 7.12), the path a script's `level` step and a room file's reload share: `World::apply_room_batch` applies a batch to a copy of the room's document, compiles it for the batch's check and again for the patch, checks its objects against the prefabs, compares the cells and writes the changed ones in place, and despawns the moved objects and spawns them again under their names; and, every 120 frames, the room's whole spawn. Warm-up 120 frames. Module: `rooms.rs`.

- **Room.** Built in code (`World::insert_room`): a seeded cave of 16 px cells, a floor four rows deep and two walls, a ramp every 20 columns rising right four rows, 45° and 2:1 in turn, seeded ledges and a one-way bridge line a third of the way down, in an art layer and a collision layer on a styled rock brush with its slope tiles; `bodies` circles of a prefab registered in code, dynamic bodies the step never moves, since the physics plugin builds into a schedule the app never runs, as `saves`'s does. Each frame's batch moves the first `moves` bodies a cell right and paints `paints` seeded art cells; the next frame's moves them back and erases the cells, so from the second frame on every frame patches the same work. Every 120 frames the room's instance and objects despawn at once, freeing their names, and the room spawns again. A moved body's new spawn takes its lean-view sprite from the instance through the spawn inheritance; the art layer draws through the engine's tilemap extract, the camera fitting the room.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `width`, `height` | 120, 68 | 40–512, 24–256 | | Cells of 16 px |
| `bodies` | 400 | 1–10,000 | yes | |
| `moves` | 16 | 1–1,000 | yes | Bodies a batch moves a cell, at most `bodies` |
| `paints` | 32 | 1–1,000 | yes | Art cells a batch paints, or erases on the next |

- **Presets:** `min` (40 × 24 cells, 40 bodies, 4 moves, 8 paints) and `default`.
- **Counters:** `bench:` `moved` (bodies the frame's patch spawned again), `painted` (cells it wrote in place), `spawned` (entities the frame's whole spawn made, else 0) and `visible`.
- **Owned:** `stage.update` p50/p95; its first capture is functional only (`D-226`).
- **Guards:** `bench.moved constant` and `bench.painted constant`.


## `generate`

Owns a generated run's churn (redo 7.12, P7-16): `World::generate_rooms` runs a generator registered in code over a template room, each room it adds compiled and registered with its generation, then spawned at its origin with its objects named; the frame after, every room's instance and objects despawn and `World::remove_room` takes each room out with its map and its area before the next seed's run. Warm-up 60 frames. Module: `generate.rs`.

- **Rooms.** The template is built in code (`World::insert_room`): a 24 × 14 hall of 16 px cells, a floor two rows deep and a wall at its left, in an art layer and a collision layer, holding one circle from a prefab registered in code, a dynamic body the step never moves, since the physics plugin builds into a schedule the app never runs, as `rooms`'s does. The generator adds `rooms` copies side by side, SF22's chain, each with `objects` circles at cells the generation's seeded stream picks. Each frame despawns the last run, removes its rooms, generates the next seed's and spawns them, so a frame pays a run's whole churn; runs alternate the prefixes `run_a` and `run_b`, so a removal that left a run's rooms or areas behind would show. The art layer draws through the engine's tilemap extract and the circles through the lean view, the camera fitting the chain.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `rooms` | 8 | 1–64 | | Rooms a generation adds side by side |
| `objects` | 24 | 1–1,000 | yes | Circles each room holds |

- **Presets:** `min` (2 rooms of 4 circles) and `default`.
- **Counters:** `bench:` `rooms` (rooms the frame generated and spawned), `spawned` (entities their spawns made), `removed` (rooms it removed) and `visible`.
- **Owned:** `stage.update` p50/p95.
- **Guards:** `bench.rooms`, `bench.spawned` and `bench.removed` constant over the window.
## `friction` and `slopes`

`friction` owns `physics_step`'s friction and rolling rows; `slopes` its slope tiles' contacts (phase 7's piles). One benchmark, `piles`, whose `mode` knob names its row as `physics`' names `physics-sparse`. Warm-up 120 frames. Module: `piles.rs`.

- **Friction.** A bin of 16 px tiles, walls down both sides and a floor, their physics material built in code: boxes and circles of 3, 4 and 5 px, every other one a circle that rolls, under the material's `friction` and `rolling` and their own, fall into it from its upper three fifths and are stirred by kinematic paddles of 16 × 80 px, one to each 24 cells of floor, sweeping it at `paddle_speed`. Each turns back two cells short of its section's ends, so none pins a body against a wall: a kinematic push has no bound, and a body crushed into a corner cell leaves through the map's edge.
- **Slopes.** The same bodies and tiles in a field: below a spawn band a quarter of its height, rows of slope-cell diamonds four rows apart, staggered, a 45° one (`45 br` and `45 bl` over `45 tr` and `45 tl`) and a 2:1 one (`2:1 low br`, `high br`, `high bl` and `low bl` over the four ceilings) by turns, two clear cells at each wall: every slope kind. The bodies start spread over the field's empty cells, so the stream is in flight from the first frame; each whose collision event names a floor cell returns to the band's upper half at rest.
- **Sleep** is off by default, so every body stays in the solve: one paddle per 24 cells still left a straggler asleep in its wake for a second at a time.
- **Reference machine**, 2026-10-31, one run each: `friction` `physics_step` p50 7.21 ms, p95 8.05 ms, `update` p95 8.10 ms; `slopes` 2.33, 2.44 and 2.50 ms.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `mode` | `friction` | `friction`, `slopes` | | Names the row |
| `width`, `height` | 96, 40 | 16–512, 16–256 | | Cells of 16 px |
| `bodies` | 3,000 | 0–50,000 | yes | |
| `friction`, `rolling` | 0.5, 0.02 | 0–2, 0–0.5 | | The bodies' and the tiles' alike |
| `paddle_speed` | 240 | 0–2,000 | | `friction` only; px/s |
| `sleep` | `off` | `off`, `on` | | `on` lets resting bodies sleep |

- **Presets:** `default` and `friction` (the defaults), `friction-min` (24 × 24 cells, 400 bodies), `slopes-min` (32 × 32 cells, 300 bodies, friction 0.3) and `slopes` (96 × 48 cells, 2,000 bodies, friction 0.3).
- **Counters:** `bench:` `bodies`, `awake`, `sleeping`, `escaped` (bodies past the walls' or the floor's cells), `spinning` (circles turning faster than 0.5 rad/s), `recycled` (returns to the band over the last 60 frames), `inside` (bodies whose centre is inside a slope cell's polygon) and `visible`.
- **Owned:** `physics_step` p50/p95 and `update` p95, each row.
- **Guards:** `friction`: `bench.sleeping <= 0` and `bench.escaped <= 0`. `slopes`: `bench.recycled >= 60`, at least one a frame over each second, and `bench.inside <= 0`.

## `blast`

Owns `physics_step` over a dense pile's sleep, wake and blasts, with no game system beside it (the CPU stress pass): example 01's full pit as a bench, so a physics change reads apart from 01's own systems. Warm-up 480 frames. Module: `blast.rs`.

- **Pile.** `bodies` circles of `radius` start at rest on a hexagonal lattice in a closed bin of 16 px tiles, `width` cells across with a floor, two walls and a ceiling, under 01's gravity, restitution and broadphase cell. They settle and sleep, one island, inside the warm-up: by frame 400 at `default`.
- **Blasts.** From window frame 20 a blast lands every `period` frames: `apply_radial_impulse` of `impulse` over `reach` px, centred on the highest body within a body's width of a seeded column, as a projectile meeting the pile there would land it. The first wakes the island whole, and at `period` 20 the pile never sleeps again, so a window holds 20 frames asleep, the wake, and 14 blasts on an awake pile.
- **Reference machine**, 2026-11-11, after the CPU stress pass (its close sitting's ten runs, `perf-runs/20261111T115945Z-sitting/`): `physics_step` 0.65 ms asleep, 20.8 ms in the wake's frame and 36.6 ms six frames on, about 17.8 ms before the next blast; p50 19.8 to 20.0 ms, p95 31.3 to 31.6 ms, p99 35.5 to 35.8 ms. Before the pass, 2026-11-10: 0.68, 25.2, 48.8 and about 24.3 ms; p50 26.9 to 27.3 ms, p95 40.8 to 41.2 ms, p99 46.3 to 46.7 ms. The step sends about 50,000 collision events a frame while the pile is awake, one a penetrating pair (`D-270`; about 200,000 before it, one a pair a substep).

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `width`, `height` | 98, 144 | 18–512, 24–512 | | Cells of 16 px, the walls', the ceiling's and the floor's included; 96 cells inside is 01's pit |
| `bodies` | 12,000 | 100–50,000 | yes | 12,000 is 01's full pit |
| `radius` | 7.5 | 2–16 px | | 01's small ball |
| `period` | 20 | 1–600 | | Frames between blasts |
| `reach`, `impulse` | 288, 2,500 | 16–1,024 px, 0–10,000 | | A blast's radius and its impulse at the centre, falling linearly to none at the reach; 01's fire missile |
| `gravity`, `restitution`, `cell` | 3,600, 0.85, 64 | | | 01's gravity, its small ball's restitution and its broadphase cell |

- **Presets:** `min` (34 × 48 cells, 600 bodies, reach 96, impulse 1,200) and `default`.
- **Counters:** `bench:` `bodies`, `awake`, `sleeping`, `escaped` (bodies outside the bin), `blasts` (1 on a blast's frame), `pushed` (bodies its impulse reached), `events` (the step's collision events) and `visible`.
- **Owned:** `physics_step` p50/p95/p99 and `update` p95.
- **Guards:** `bench.bodies constant` and `bench.escaped <= 0`.

## `e01-pit`

Example 01's heaviest frame: the full ball pit under the rope bridge with six enemies awake about the player (`features/run/pit.rs`). The script: `start_run` at frame 2, `fill_pit` to 12,000 balls at 30, `wake_foes` at 630, then 180 warm-up frames (warm-up 810 in all) and 300 measured; it ends asserting the `pit` probe (`balls` = 12,000, `foes_alive` = 6, `foes_awake` ≥ 5). 1920×1080, CPU, five repeats.

- **Owned:** `total` p50/p95; `update`, `physics_step` and `extract` p50.
- **Guard:** the script's report.
- **Reference machine**, 2026-11-21, after 7.14's first plan (its close sitting's ten runs against 0.118, `perf-runs/20261121T143639Z-sitting/`): `total` p50 23.93 → 22.55 and 23.88 → 22.57 ms, p95 37.08 → 35.89 and 37.07 → 35.76; `physics_step` p50 19.00 → 18.21; `extract` p50 0.43 → 0.37. In a gated profile of the window the ECS query and keyed-access paths read 1.10 → 0.43 ms a frame (C4: the collection's walk 0.52 → 0.04) and the sensor pass with the query index 1.40 → 0.58 (C1): the area zone over the pit pairs with each of its 12,000 balls every step, and the pass now compares those pairs with the last step's in visit order and keeps an unchanged pass whole. The window holds 75 frames with the pile asleep, 190 awake and 35 between.

## `e01-stress`

Example 01's full pit under a storm of spells (the CPU stress pass): the frames where blasts, a pile's wake, fire, impacts and enemies load the step together. `e01-pit`'s setup and warm-up (`start_run` at frame 2, `fill_pit` to 12,000 at 30, `wake_foes` at 630, warm-up 810), then `cast_at`, which lands a spell's toy at a point free of mana and cooldown, through the 300 measured frames, numbered from 0:

- **Blasts:** a fire missile's on the pile's surface every 20 frames from frame 20, at five points in turn, and one on the deck among the enemies every 100 from frame 30.
- **Other spells:** lightning onto the deck every 96 frames from 25 and onto the pile every 96 from 73; a force push along the deck, east every 120 from 35 and west every 120 from 95; an iron block dropped onto the pile at 44, 104, 164 and 224; a black hole over the pile at 149.

The window holds the pile asleep for 20 frames, its one island's wake at the first blast and the storm on an awake pile; the fire reaches every ball by about frame 90. It ends asserting the `pit` probe (`foes_alive` = 6, `foes_awake` ≥ 5, `balls` ≥ 11,000, the hole sweeping some up) and the `pit_fire` probe (`burning` ≥ 10,000). 1920×1080, CPU, five repeats.

- **Reference machine**, 2026-11-11, after the CPU stress pass (its close sitting's ten runs, `perf-runs/20261111T115945Z-sitting/`): `total` 5.4 ms asleep and 32.3 to 32.6 ms in the wake's frame; p50 38.7 ms, p95 45.3 to 45.4 ms, p99 47.9 ms. Of the 20 highest frames' 46.6 ms, `physics_step` holds 73.1 %, 01's `fire_spread` 8.8 %, `fire_exposure` 5.6 %, `body_impacts` 1.8 % and `iron_crush` 1.1 %. Before the pass, 2026-11-10: 5.8 and 42.7 ms; p50 55.1 to 55.2 ms, p95 63.2 to 64.1 ms, p99 67.2 to 67.3 ms; of the 20 highest frames' 64.8 ms, `physics_step` 58.9 %, `fire_spread` 18.4 %, `iron_crush` 5.3 % and `fire_exposure` 4.6 %.
- **Reference machine**, 2026-11-21, after 7.14's first plan (the same close sitting): `total` p50 37.90 → 36.26 and 37.89 → 36.37 ms, p95 45.02 → 43.22, p99 47.51 → 46.16; `iron_crush` p95 1.36 → 0.46 ms, which fell when `has` came to answer through the archetype's slot (K12, `581b6ac7`): the sittings either side of that commit read it 1.33 and 0.46 ms.
- **Owned:** `total` p50/p95/p99; `update` p50/p95; `extract` p50; `physics_step` p50/p95; `fire_spread`, `fire_exposure`, `body_impacts`, `iron_friction` and `animation_tick` p50; `iron_crush` p95.
- **Guard:** the script's report. It has no counter guard: `pit`'s pins the full pit's frame and `blast`'s the engine's pile under blasts.

## `template`

The template's sample game, the frame a new game starts from. The script: `start_run` at frame 2, then 120 warm-up frames (warm-up 122) and 300 measured; it ends asserting the `player` probe's health above 0, since the template has no state probe. 1920×1080, CPU, five repeats.

- **Owned:** `total` p50/p95; `update` and `extract` p50.
- **GPU counters:** pinned in `just perf counters`.

## `e01-areas`

Six rows of example 01 at 3840×2160, GPU runs only, three repeats: `e01-area-clearing`, `-aqueduct`, `-ravine`, `-hollow`, `-mine` and `-bridge`, the areas of M77's survey. The script: `start_run` at frame 2, then `place_player` from frame 30 through the last frame at a point standing in the area (`clearing`, `aqueduct_floor`, `rv_ledge_2`, `hollow_floor_1`, `cut_floor_1`, `troll_bridge` of `tools/level.json`), 60 warm-up frames (warm-up 90) and 300 measured, asserting the `area` probe names the area.

- **Owned:** the scene pass and `render_span` p50/p95; `post0_bloom_threshold`, `post0_bloom_composite` and `post1_vignette` p50.
- **GPU counters:** `e01-area-clearing` is pinned in `just perf counters`.
- **The scene pass, split at 7.15's gate** (2026-11-21, `65d82c00`, `perf-runs/715-gpu-pass/step-1/`). A GPU pass is timed whole, so the gate read the scene pass's parts as differences of captures under knock-out switches, applied to the tree for the measurement and restored (`perf-runs/20261121-gpu-pass-plan/tools/`, its `README.md`), three GPU runs a label through the runner on a quiet machine. Clearing / aqueduct / ravine / hollow / mine / bridge:
  - **The scene pass** 16.105 / 20.207 / 7.980 / 28.068 / 19.446 / 12.431 ms of a 22.160 / 26.278 / 13.936 / 34.213 / 25.487 / 18.398 ms `render_span`.
  - **The lit path,** the pass less itself with lit batches drawn through the plain pipeline: 12.858 / 16.899 / 5.269 / 24.691 / 16.080 / 9.378 ms. 01's World holds 23 lights and every area packs 16, one of them dark (two in the hollow); a lit fragment loops over all sixteen.
  - **The backdrops and tile layers,** under the plain draw: 2.664 / 2.873 / 2.358 / 2.629 / 2.777 / 2.507 ms; with both out the pass reads 0.582 / 0.435 / 0.353 / 0.748 / 0.589 / 0.545, the `Sprite` entities and an empty pass.
  - **Particles:** knocking emission out moves no area by 0.01 ms.
  - **The post passes** sum to 5.43–5.56 ms in every area, bloom's chain 3.73–3.83 and the vignette 1.69–1.73, each pass's time following its target's pixels and taps (`step-1/areas/passes.txt`); the stack reads 5.34 ms on `gpu` at 3840x2160 with nothing drawn.
- **After 7.15's first plan** (2026-11-22). Only the lights that can light the view are packed (`928e5dae`): a dropped light's term is exactly zero at every fragment of the view, and the six areas' stills are byte for byte the same. Against 0.118 at the plan's close (`perf-runs/20261122T045713Z-sitting/`), clearing / aqueduct / ravine / hollow / mine / bridge:
  - **The scene pass** p50 16.090 → 7.453, 20.187 → 6.777, 7.980 → 3.440, 27.833 → 10.240, 19.280 → 6.500 and 12.293 → 4.290 ms; `render_span` p50 21.917 → 13.270, 26.020 → 12.587, 13.727 → 9.147, 33.763 → 16.173, 25.107 → 12.303 and 18.057 → 10.010.
  - **The lit path,** by the gate's knock-out: 4.217 / 3.471 / 0.738 / 7.083 / 3.287 / 1.364 ms, under half its gate reading in every area (`perf-runs/715-gpu-pass/step-2/areas/gate.txt`). Of the gate's parts the post passes, unchanged at 5.43–5.56 ms, now lead every area but the hollow, where the lit path still does, and the backdrops and tile layers (2.36–2.87 ms) pass the lit path in the ravine and at the bridge.
  - **A light's `priority`** (`02de1761`, `D-296`) moved no scene pass: `unchanged` in all fourteen judged pairs of its sitting, `gpu`'s two among them (`perf-runs/20261122T043956Z-sitting/`).
  - **Landed in the second plan:** the fragment's range test, which stopped in this one at `gpu`'s `render_encode`.
- **After 7.15's second plan** (2026-11-23, `dc89562f`, `D-297`). Example 01 draws under `render.depth_sort = gpu_depth`, which draws `cpu_stable`'s picture: the six areas' stills are byte for byte the same at every step. Clearing / aqueduct / ravine / hollow / mine / bridge:
  - **The scene pass, a step at a time** (p50, each step's own sitting): the fragment's range test 7.457 → 5.630, 6.780 → 5.523, 3.440 → 3.250, 10.240 → 6.790, 6.503 → 5.340 and 4.290 → 3.983 ms (`4fa9ea27`); a quad's skip of the lights that cannot reach it, to 4.770 / 4.960 / 3.180 / 5.273 / 4.840 / 3.873 (`a0f4574c`); the depth sort with the backdrop's batches marked solid, to 3.820 / 3.570 / 2.160 / 3.757 / 3.293 / 2.770 (`3db5c6e8`); the strips clipped to the rows their sprites draw, to 3.433 / 3.270 / 2.093 / 3.740 / 3.233 / 2.477 (`dc89562f`).
  - **Against 0.118** at the plan's close (`perf-runs/20261123T024118Z-sitting/`): the scene pass p50 16.090 → 3.430, 20.190 → 3.270, 7.983 → 2.090, 27.830 → 3.740, 19.287 → 3.240 and 12.297 → 2.480 ms; `render_span` p50 21.920 → 9.213, 26.033 → 9.040, 13.740 → 7.820, 33.770 → 9.657, 25.120 → 9.003 and 18.060 → 8.190. The post passes, unchanged at 5.4–5.6 ms, are now the larger part of every area's `render_span`.
  - **The backdrops and tile layers** (G2), by the gate's formula on the measured tree, the scene pass with lit batches plain less the same with backdrops and tile layers out: 1.691 / 1.609 / 1.346 / 1.166 / 1.291 / 1.561 ms on the sort and 1.316 / 1.309 / 1.277 / 1.153 / 1.225 / 1.276 with the clip, against 2.664 / 2.873 / 2.358 / 2.629 / 2.777 / 2.507 at the gate: at or under half in four areas, 46 and 49 % down in the ravine and at the bridge (`perf-runs/715-gpu-pass-2/step-5/`, `step-6/`).
  - **The census behind the sort** (`perf-runs/20261122-gpu-pass-2-plan/census/`): one frame an area, rasterized offline from a dump of 01's batches, counted 3.39 / 3.70 / 3.16 / 3.43 / 3.74 / 3.44 views of backdrop and tile fragments drawn and 46.1 / 53.5 / 54.7 / 72.1 / 65.7 / 50.7 % of them behind a later fragment of a batch of two-level sprites, where wholly opaque sprites hide 0.0 / 0.0 / 0.0 / 48.2 / 2.0 / 4.8 %: 01's strips are two-level, every texel clear or opaque. Of what the test leaves, about one view is fragments that write and 0.79 / 0.67 / 0.37 / 0.01 / 0.29 / 0.69 views are clear texels of the strips' and clouds' quads. A view of fragments cost 0.74–0.79 ms before; under the depth path a written one costs about 0.9 ms a view, a clear texel's 0.6–0.67 and a hidden one's about 0.09 (the plan session's fit).
  - **The window** opens at 3840×2040 for the rows' 3840×2160 request on the reference desktop, and always has.

## Counter guards

Ordinary tests that pin what a frame does, so a change that adds work fails `just check` without a sitting (redo 7.3). `tungsten::testing::CounterGuard::measure` steps a headless harness through a warm-up and a window with `CountingAlloc` installed in the test binary and reads the window's allocations and bytes on the test's thread, `World::counters()`' archetype moves, spawns, despawns and events, and the most sprite batches and instances one frame extracted. The log console's feed stays detached while it counts, since in a test binary it carries every thread's records. `check` compares the reading with its own JSON pin, one file per guard; `TUNGSTEN_GUARDS_WRITE=1` rewrites that file. Pins are Linux's; elsewhere a guard prints its reading and passes.

| Guard | Test | Configuration | Asserted first |
| --- | --- | --- | --- |
| `physics` | `example-02-bench` `guards_physics` | `physics` at `min` | `Ball` count = `balls`; every frame's instances = the view's drawable `ViewSprite`s |
| `physics-sparse` | `guards_physics_sparse` | `physics` at `sparse-min` | `Ball` count = `bodies`; view instances |
| `ecs` | `guards_ecs` | `ecs` at `min` | entity count = `entities`; view instances |
| `churn` | `guards_churn` | `churn` at `min` | `Life` count = `population`; view instances (none at `view` 0) |
| `particles` | `guards_particles` | `particles` at `min` | emitters and animated sprites; instances = visible sprites by the default extract's cull |
| `gpu` | `guards_gpu` | `gpu` at `min` | field and glow sprites, lights, tilemap; instances = sprites plus visible tiles |
| `gpu-throughput` | `guards_gpu_throughput` | `gpu` at `throughput`, scale 0.01 | as `gpu` |
| `terrain` | `guards_terrain` | `terrain` at `min` | `Body` count = `bodies`, `Sensor` count = `sensors`, one map; the warm-up's last frame and the window's last break and refill `edits` cells each and trace 1 to 8 × `edits` chunks; every frame's instances = bodies in view plus visible art tiles |
| `friction` | `guards_friction` | `piles` at `friction-min` | `Body` count = `bodies`, one paddle, one map; each window frame no body asleep or out of the bin and a circle spinning; instances = bodies and paddles in view plus visible art tiles |
| `slopes` | `guards_slopes` | `piles` at `slopes-min` | `Body` count = `bodies`, no paddle, one map; each window frame no centre inside a slope cell's polygon and at least 60 returns over the last 60 frames; instances as `friction` |
| `blast` | `guards_blast` | `blast` at `min` | `Body` count = `bodies`, one map; before window frame 20 no body awake and no collision event; at 20 the blast reaching bodies and leaving no sleeper, then a blast every `period` frames; each window frame no body out of the bin; instances = bodies in view plus visible art tiles |
| `saves` | `guards_saves` | `saves` at `min` | records = `objects` + 1 + `spawns`; view instances |
| `spread` | `guards_spread` | `spread` at `min` | the receivers tagged and every one lit; each window frame every one burning, chemistry's queue at most one entry a receiver, nothing dropped; view instances |
| `rooms` | `guards_rooms` | `rooms` at `min` | `RigidBody` count = `bodies`, one map; each window frame `moves` bodies spawned again and `paints` cells written in place; instances = bodies in view plus visible art tiles |
| `generate` | `guards_generate` | `generate` at `min` | the template and the generator registered; each window frame `rooms` rooms generated and spawned, `rooms` × (`objects` + 1) entities, `rooms` removed; instances = bodies in view plus visible art tiles; after the frames, `the_generate_row_keeps_only_the_live_runs_rooms_and_areas` finds the template and the live run's rooms alone, the other prefix's areas neither loaded nor kept |
| `boot` | `guards_boot` | `boot` at `min`, its assets' load run by `LoadPlan::run_headless`, no frame stepped | the plan's units = `bench.units`; every sound the manifests name loaded, no sprite packed (the reading is the load's: its sprite batches read 0, and its bytes, which carry the checkout's canonical path, are not pinned) |
| `pit` | `example-01-platformer` `guards::the_full_pit_holds_its_counts` | `start_run`, 2,000 balls, 120 settle frames, `wake_foes`, a 60-frame window | the `pit` counts (headless 01 loads no sprite assets, so its batches read 0) |
| `idle-screen` | `tungsten` `screens::tests::an_idle_screen_lays_out_no_root` | a `Menu` screen pushed and past its entrance, then 60 frames without input | `UiHarness::layout_totals` unchanged over the 60, with no pin file: the zero is the assertion; a focus move then lays out no root and one label's new text exactly one, so the guard sees work (redo 7.17) |
| `views-closed` | `tungsten` `debug_views::tests::views_guard_closed` | a default app, every debug view off, a 30-frame `UiHarness` warm-up, a 60-frame window | nothing: the reading is what every game's frame pays for the views and the system timings they read (debug-pass) |
| `views-open` | `debug_views::tests::views_guard_open_idle` | the five views open at 1280×720 on 1.75, each refresh interval infinite, the console on 40 records of its own feed, a 300-frame `UiHarness` warm-up past the HUD graph's 240 samples, a 60-frame window | the five laid out; 60 more laid-out frames then print the roots and nodes they lay out (debug-pass) |

Each bench guard runs its row's warm-up and a 60-frame window at the configuration named, and asserts the row its configuration measures before any pin. A rise in a cost counter needs a fix, or its cause in the change's plan, or in its commit's one-line subject when the change has none. The template has none: `tungsten new` copies the template's tests into every game.

**GPU counters** stay local: `just perf counters [--write]` runs `gpu`, `gpu-throughput`, `template` and `e01-area-clearing` at the row's warm-up plus 30 frames and checks every window frame's `counters:` line against the row's `counters` pin in the registry, `{"each": n}` for a count every frame shares or `{"sum": n}` for the window's total. `draw_calls` and `upload_bytes` are the engine's own; glyphon's text work is `text_renders`.

## Levers for 9.2

<a id="engine-findings"></a><a id="open-proposals"></a>Every engine finding, open proposal and item left open by a pass in this file at `2464cbc9`, one line each: an open lever with its evidence, or `closed` with what closed it. The evidence is this file's history (`git show 2464cbc9:docs/perf/benchmarks.md`, the section named) and the decisions. 9.2 picks from the open ones.

- **Burst latch:** closed (`D-143`).
- **Tiles at `z_norm` 0:** closed (`D-297`): under `gpu_depth` a batch's depth is its place in the frame's list, so no depth reads a tile's `z_norm` and the frame's order draws a sprite over the tiles, as under `cpu_stable`.
- **Lit-plus-material warning:** closed (0.61, M43).
- **Text-cache RSS:** closed (`D-085`).
- **Default extract:** open in part. Rewritten (`D-086`), interned (`D-113`) and culled (`D-114`); it still leads `gpu-throughput` (12.15 of 13.68 ms `total` p50 on 2026-10-04), and the bound test costs rows with every sprite in view; Engine findings, `gpu`. About half of that `gpu-throughput` reading is a stall that follows the GPU's scene pass onto whichever stage writes memory under it, 5.5–6.7 ms being the extract's own (7.14's gate, the `gpu` section above): 7.14's second plan took the row loop (C6) on the headless control (the `gpu` section), bit for bit: an archetype with no override block and no flip takes a loop of its own, which keeps what a sprite ID and material resolve to while the next row names the same pair (`e2f0487c`: 5.795 → 3.547 ms on the control; `gpu-throughput` `stage.extract` p50 11.944 → 11.692 and 11.666 ms, `gpu` 0.870 → 0.816 and 0.866 → 0.800, `perf-runs/20261121T201656Z-sitting/`), and the frame after a one-batch frame writes no key a kept sprite (`a32172fe`, `D-294`: 3.548 → 3.317 ms; `gpu-throughput` 11.696 → 11.074 and 11.672 → 11.062, `perf-runs/20261121T210845Z-sitting/`). C6 ended `stopped` there, 0.413 ms over its target of 2.904: the control's profile reads 37.9 cycles and 160 instructions a sprite (66 and 250 at the gate) with no key store, lookup or hash left in the loop, the view test and its branch 58.9 % of the loop's samples (with skid toward it), `z_norm` and the instance's store 8.4 %, the order and its bits 7.9 %, the row's step and visibility 6.9 %, the transform's loads and the size 6.3 %, the sprite's load and the memo's compare 5.6 % (`perf-runs/714-cpu-pass-2/step-4/`). Left to take: a cheaper view test for a frame wholly in view (below), non-temporal or raw instance stores (`unsafe`, refused) and instances kept between frames while nothing moved, a design of its own. A windowed frame now reads the extract over the control's: at one pixel a sprite (`sprite_px=4,zoom=0.25`) `stage.extract` p50 reads 3.99, 4.01 and 4.04 ms by hand against the control's 3.317, where it read 4.89 and 4.93 against 5.8 before the two steps, and the row reads 11.97 → 11.09 and 11.91 → 11.05 ms against 0.118 in the pass's close sitting (`perf-runs/20261121T224015Z-sitting/`), so a row that times the extract needs a headless run (ROADMAP.md, Later). The stall itself is the machine's.
- **The instance pool's bins:** open. `InstancePool` bins an emptied vector by the floor of its capacity's power of two and takes by the ceiling of the count's, so a vector reserved to a frame's exact count sits a class under the one the next frame asks for and is never taken: 7.14's plain loop reserves its keys and not its records, whose vector reaches its size by doubling on a frame whose count jumps (`records.reserve(rows)` broke `single_batch_frames_match_reference_as_the_count_changes` in the prototype). A take that reads the class below when the capacity fits, or a reserve rounded up to the class's size, would let pass 1 reserve.
- **Surface clear and present blit:** closed for the blit (`D-087`); the clear in the first swapchain pass stays, which only `LoadOp::DontCare` (`unsafe`) removes; Engine findings. The frame's last full-screen stage writes and clears the swapchain, which costs it about 0.7–0.8 ms more at 2160p than an offscreen target, whichever pass it is: bloom's chain reads 4.48–4.59 ms with the vignette off, the scene pass 0.66–0.83 more with both off, and an empty scene's clear 0.53 against 0.25 (7.15's audit, 2026-11-21, by hand; `perf-runs/20261121-gpu-pass-plan/knock/`).
- **Tile collision proxies:** open in part. Kept across steps (`D-203`) and re-traced by 16 × 16 chunk after an edit (runtime-terrain); a frame with an edit still rebuilds the edited map's proxies and restages the pair grid's tiles, which led `terrain`'s 5.35 ms `physics_step` p50 (2026-10-30; 4.26 ms on 2026-11-11, once the support record is compared without a whole sort, `1517795a`); dropping the interior cells, which make no contact, is the measured next step (ROADMAP.md, Later).
- **Contact wakes rebuild the pair list:** closed (`D-234`): a woken body is repaired into the list; `e01-pit`'s total p95 −7.13 and −6.95 ms and p99 −14.8 and −13.9 ms in its sitting (`perf-runs/20261102T013307Z-sitting/`).
- **The pair grid's walk:** open in part. The build, the repair and the arrival pass take a query's cells run by run since the CPU stress pass (`55104ae7`: `physics` `physics_step` p50 −0.69 and −0.66 ms, `blast` −2.35 and −2.38, `perf-runs/20261111T035427Z-sitting/`), in `D-080`'s visit order; a point's and a ray's visitors keep the walk by entries. Before it the three's queries, their visits included, held 18.1 % of the 31.3 ms of `physics_step` in `e01-pit`'s 20 highest frames (`perf-runs/20261101-physics-tail/close/`); their share is not profiled since.
- **01's collision-event readers:** open in part. Each walks the step's collision events, and one event a pair a step (`D-270`) cut those fourfold: `e01-stress` `fire_spread` p50 13.01 → 5.00 ms, `body_impacts` 2.42 → 0.77, `iron_friction` 1.18 → 0.34 and `iron_crush` p95 4.44 → 1.31 (the CPU stress pass's close sitting, `perf-runs/20261111T115945Z-sitting/`). The readers are 01's, and 7.13 rewrites them.
- **`CollisionEvent`'s layout:** open. 72 bytes, public API (`D-229`'s speed and `Option<TileHit>`); in `e01-pit`'s 20 highest frames the pair loop's push, the drain and the queue's sends took 0.92, 0.92 and 0.63 ms of the 31.3 ms of `physics_step` (2026-11-01). Since the CPU stress pass the step hands its events over in one append (`EventQueue::append`, `a7a4c7ed`: `blast` `physics_step` p50 −1.75 ms in both pairs) and sends a quarter as many (`D-270`); the layout's cost at that count is not measured.
- **Bundle insert:** closed (`D-135`).
- **Archetype count:** open. `ecs` `update` p50 9.14 / 10.52 / 19.15 ms at 64 / 488 / 3,189 archetypes, consistent with each column's first cache lines missing; no candidate without `unsafe`; Engine findings.
- **String sprite IDs:** closed (`D-113`); animation IDs stay strings (below).
- **G1 kinematic bodies in a row:** open. The engine side landed (`D-154`); a row that uses them is a workload change.
- **G2 sensor bins in `physics`:** open. Sensors landed (`D-154`); a workload change.
- **G3 more than 16 lights:** open. `LIGHT_CAP` is 16.
- **G4 GPU particle simulation:** open.
- **T1 particle and tween timings:** closed (`D-133`).
- **T2 a `startup:` line:** closed (redo 7.9): the line, its `startup.<field>` metrics and the `boot` row that owns them.
- **M1 a counting allocator:** closed (redo 7.3: `CountingAlloc` in test binaries, the counter guards).
- **Below the calibration band:** open. `physics` (6.07 ms `total` p95 against 14.58 at calibration), `physics-sparse` (3.73 against 10.11), `churn` (4.06 against 11.3) and `particles` (4.92 on 2026-10-04 against 11.6) run under the 8–16 ms band; recalibrating is a workload change; Open proposals. `integrated`'s line is closed: the row is retired (redo 7.3).
- **RSS growth threshold:** open. Judge growth for `churn` only (32 KiB/s median, 16 KiB/s every run); Open proposals.
- **`ecs`'s per-run mode:** open. Seven system rows take a second value in some runs with the same digest; cause not looked for; `ecs`.
- **The scene pass's two values in the GPU run:** closed (7.15's gate, 2026-11-21). They followed the blocking readback's timing, and redo 7.3's ring reads timestamps without blocking: the five GPU runs of a `gpu` capture read the scene pass p50 6.94, 6.93, 6.94, 6.94 and 6.94 ms (`perf-runs/20261122T022830Z-gpu/`), and the twenty of 7.14's close sitting 6.93–6.96 (`perf-runs/20261121T224015Z-sitting/`).
- **`integrated`'s row note naming the old text cache:** closed (the row is retired, redo 7.3).
- **Compositor-bypass hint on X11:** open. KWin's acquire stall sets `gpu`'s tail at `min`; Session B's list.
- **Shape-run cache for text shaping:** open; Session B's list.
- **Frame cap overshoot:** open. 0.06 ms per frame at a cap of 60, each deadline counting from the frame's own start; a deadline grid would remove it; Session B's list.
- **The HUD's `fps` row:** closed (M57, `D-165`).
- **Interned animation IDs:** open in part; M33's list. `animation_tick` finds a row's clip once and keeps it while the next row names the same one (`4b09dd3b`: `e01-stress` `animation_tick` p50 1.04 → 0.46 ms, 12.4 ns a sprite in the kit's criterion row). 7.14's gate kept the rest as C3: `particles` `animate_sprites` paid a hash and a probe of the registry's string map a sprite through `AnimationState::advance`, 0.243 of the row's 4.517 ms mean `total` (5.37 %), its eight clips interleaving by row. Its second plan closed C3 by a clip's slot kept in `AnimationState` (`63c4fa78`): the registry keeps its clips in a list, a state keeps its clip's slot in the four bytes `frame_index` gave up as a `u32`, and a lookup takes the clip at that slot when its ID equals `animation_id`, compared a word at a time, else looks the name up and keeps the slot it finds: `animate_sprites` p50 0.232 → 0.102 and 0.236 → 0.104 ms against a target of 0.120 (`perf-runs/20261121T192806Z-sitting/`). Open past it: `animation_id` stays a `String` any caller may write, so a lookup still compares its bytes and a state holds a heap string; an interned ID, as sprites took (`D-113`), is a public type change.
- **`particles`' `unattributed` reading after `D-113`:** open, mechanism not established; M33's list.
- **A cheaper cull for all-in-view rows and a view test for `extract_mesh_particles`:** open; M33's list.
- **The arrival pass:** open. Its search and sort were cut bit for bit (`9b2c12f0`: `blast` `physics_step` p95 −3.31 and −3.13 ms); reading a listed body's candidates from the pair list instead of the grid then missed its target twice (`blast` p95 −0.11 and −0.05 ms bit for bit), a listed body's walk and cut costing about 0.5 µs by then. The variants that read faster changed motion, clamping a body against every pair partner in reach (ROADMAP.md, Later; `perf-runs/20261110-cpu-stress/step-6/`).
- **The pair list kept across frames:** open. On a blasted pile of 12,000, 66 of 112 steady frame starts could keep the list (59 %), under the CPU stress pass's gate of two in three: a budget covers one frame's travel at the speed it was built for, so a body over about 250 px/s outruns it at the next frame's start (the release test `a_blasted_pile_of_12_000_prints_the_frame_starts_that_could_keep_their_pairs`).
- **A worker pool for the step's pure stages:** open, the owner's decision. The arrival pass's neighbour search and the build's per-querier walks ran in chunks on a caller-owned rayon pool with the determinism test's pinned hash holding at 0, 2, 6 and 12 workers: at six workers `e01-stress` `total` p95 −6.9 and p99 −7.7 ms and `e01-pit` −8.6 and −11.1, measured before the CPU stress pass shrank both stages, so measured again before any decision (`perf-runs/20261110-cpu-stress/threads.txt`). Landing it supersedes `D-131`'s no-worker clause and `D-067`.
- **Chemistry's emission step:** closed for 7.14's C2, open past it. `spread` `system.chemistry` p95 10.15 → 5.11 ms after the CPU stress pass (`4d616236`: a cheaper hasher on the queue's maps, a receiver's slot kept through a pass's emissions, one tile check a pass), then 5.11 → 2.43 ms in 7.14 (`f2e4597b`, `c33f83f7`: the grid's cell directory, packed centres and squared test; a kept slot as a hint across runs, the cells' order kept, the take keyed to its cut-off age), bit for bit. What remains is the 142,000 reaches of the step where every receiver emits: the grid's scan and the merges. Spreading the emissions over steps moves the kit's pin; skipping a merge is not exact, sums running to `f32::MAX`.
- **The sensor pass and the query index:** open past 7.14's C1 (1.40 → 0.58 ms a frame of `e01-pit`, `f49e58dc`, `37d7137e`). The pass still tests each of a sensor's candidates, 12,000 under 01's pit zone, and a refresh still copies every proxy's entry and compares its box; an awake pile stages the bodies' grid whole each step (known issues, `refresh`). The pair grid as the queries' index would end the second grid, a repaired or oversize body being outside it today; a mask on 01's zones is data.
- **The query paths past 7.14's C4:** open (1.10 → 0.43 ms a frame of `e01-pit`, `ebc81a73`). Keyed gets and `has` hold 0.28 of it, 01's `FastBodies::has` and its event readers, and the column setup 0.12, a key scan an item an archetype: the setup through the slot table, or a part list kept between calls.
- **The save capture past the table:** closed for 7.14's C5, open past it. `saves` `system.save_snapshot` p50 17.87 → 12.9 ms by the capture's bit-exact costs (`1ed0aa58`), then 12.99 → 4.85 and 12.94 → 4.82 ms against a target of 8.934 by a prefab's runtime spawns saved as a table (`441f3440`, `D-295`, format 2; `perf-runs/20261121T222526Z-sitting/`). What remains is the named records' patches, the erased reads (a keyed `get` a component a spawn), the leaves' pushes, `to_vec`'s floats and the ID map. A table rests on its group's lowest-ID spawn: a spawn whose references or layout differ from that one's is written as a record, so a group whose lowest-ID spawn is the odd one (a field its form skips, a reference of its own) writes records alone, at format 1's cost; choosing the commonest shape needs a first walk that reads values. Cells equal in every row are still written a row, which keeps 01's balls off the table (S-35; ROADMAP.md, Later).
- **The statics staged a pair build, and the oversize collider:** open. Every pair build stages each entity static's whole cell span again, and a collider past the loader's 16,384 px half extent stages its whole cell area into every grid (known issues, `build_pairs` and staging): unchanged statics kept staged across builds, as `D-203` keeps the tiles. No row shows the cost, so 7.14's gate left both.
- **01's `ball_look` (S-90):** open, 01's own. 0.31 of `e01-pit`'s 19.9 ms mean `total` (1.6 %) and 1.34 ms of `e01-stress` (3.6 %) at 7.14's gate: a `powf` a hued ball, 0.13 ms in the pit, and the status reads, 1.06 ms under the storm; a tint table moves a test's pinned colours (known issues).
- **An idle tar pool's allocations in 01's pit (EV-41):** open. No profile of 7.14's gate puts it in the query paths; its fixture is the evaluation's workspace, so it needs a row of its own before any work.
- **The lit path past 7.15 (G1):** closed for 7.15, `done` on every kept row; open past it. The light list packs only the lights that can light the view (`928e5dae`: the lit path of 01's six areas at 2160p 12.858 / 16.899 / 5.269 / 24.691 / 16.080 / 9.378 → 4.217 / 3.471 / 0.738 / 7.083 / 3.287 / 1.364 ms), a lit fragment skips a light's term out of its reach (`4fa9ea27`) and a quad's vertices mark the lights that cannot reach it (`a0f4574c`): `gpu`'s lit path 1.738 → 1.133 → 0.832 ms against a target of 0.869. This line once said a list of lights "would skip only range tests"; a rejection costs 41 % of a term (the `gpu` section's split), and the mark takes most of them. What is left on `gpu`, split on the plan session's build with the mark in (`perf-runs/20261122-gpu-pass-2-plan/final-g1/`): 0.18 ms with no light, 0.25 of the terms' own arithmetic for the fragments in reach and 0.39 of reach arithmetic and mark tests for the lights the mark leaves. Levers, each exact: a cheaper rejection in the fragment, a squared distance against a squared radius under a slack before the square root and the division, worth at most the rejections the mark leaves, about 0.09 ms on `gpu`; the mark under MSAA, by a quad's box widened by a pixel's reach, which the light block would have to carry; a finer mark than a quad's, a light grid or lists a tile. A clear texel's early exit added 0.1–0.35 ms in the first plan's probe; under `gpu_depth` a solid lit batch's clear texels are discarded before the loop.
- **Overdraw past 7.15's depth sort (G2):** closed for 7.15 in four of 01's areas, `stopped` in the ravine and at the bridge; open past it. What is left is the frame's own (the `e01-areas` section's census): about one written fragment a pixel, the clear texels inside the strips' silhouette bands and the clouds, and the hidden fragments the depth test turns away at about a tenth of a drawn one's cost. Two levers change the picture or rest on a promise: quads trimmed to a sprite's drawn box, in the art or at decode, whose vertices snap to the sub-pixel grid apart from the whole quad's, so a nearest sample within about a thousandth of a texel's edge can change; and farther layers cut at a nearer strip's foot line, which covers the view below its strip only if adjoining copies meet without a gap, a promise a strip's fractional position does not make. Exact, and on no row 7.15 kept (`ROADMAP.md`'s Later): `Sprite` entities' batches marked solid, the engine's own backdrops clipped, the bench's generated sprites measured, an opaque sprite under a linear sampler counted solid, and `gpu_depth` as the default sort. Example 01's own backdrop cull is still worth 0.07–0.14 ms a frame under the depth test and stays; the depth target attached with no batch solid costs 0.03–0.05 ms of the scene pass.
- **The scene depth target under `cpu_stable`:** open. It is allocated whenever depth is enabled though only `gpu_depth` attaches it (33 MB at 4K); left unallocated it lengthened `boot`'s pack by 3.3 ms when 7.15 tried it, so the change left its step (known issues' render F11; the `boot` section).
- **`gpu`'s owned `render_encode`:** open, 9.1's with the budgets. The stage reads the scene pass inversely (the `gpu` section), so a change that shortens the row's GPU frame is judged on `total` and reads the stage `regressed`; owning `stage.total` there in the stage's place would end that reading, and an owned-metric change is a decision's.
- **Bloom and the post stack (G3):** open; every lever known changes the picture or needs `unsafe`, so 7.15 stopped the item at its gate (5.43–5.56 ms an area at 2160p, 5.34 with nothing drawn). A pyramid whose first level follows a 1080-line reference, so 2160p starts at a quarter: bloom 3.78 → 2.03 ms with a wider finest glow, the owner's review of the look. A trailing vignette folded into bloom's composite, or drawn as a blended triangle in the composite's pass: about 0.95 ms and one 8-bit rounding fewer. `Rg11b10Ufloat` for the pyramid: 3.78 → 3.66. The swapchain's clear (above). The first two together read about 2.8 ms against a target of about 2.75 (7.15's audit, 2026-11-21, by hand; `perf-runs/20261121-gpu-pass-plan/knock2/`).
- **Particle quads (G4):** open beside the GPU simulation above. On `particles` the scene pass follows the quads drawn, 42–48 ns a quad (1.997 ms for 41,443, 0.876 for 20,901 at `rate=0.5`; 7.15's gate), and the bench's soft dots reach their quads' rims, so 7.15 stopped the item there; 01's area rows read it under 0.01 ms and no row sprays. A quad trimmed at decode to its sprite's drawn box would cut the fill of sprites with clear margins (01's `ex10_mote` holds an 18 px dot).
- **The encode's instance copy (G5):** open. `gpu-throughput`'s `render_encode` is one copy of the frame's 19,200,128 instance bytes into the staging view: mean 1.597 of a 12.872 ms `total` at 7.15's gate, 11.48 % of the window's cycles under `record_main_draws` with 99.2 % of them libc's copy, and no fresh memory a frame (`perf-runs/715-gpu-pass/step-1/g5/`), so 7.15 stopped the item there. What would lower it crosses the seam (`D-018`): the extract writing the staging view itself, or instances kept between frames while nothing moved (the default extract's line above); a narrower instance is a public type and loses `z_norm`'s range.

## Adding a row

A row and its counter guard land with the system step that needs them (redo §5.8).

1. **Workload.** A bench benchmark under `examples/02_bench/src/<bench>.rs`, registered in `main.rs`, seeded from `seed`, with knobs, presets (`min` and `default` at least) and `bench:` counters that prove the workload; or a game row's script on hooks its game registers.
2. **Registry.** An entry in `scripts/perf-rows.json` at `workload_version` 1: package, folder, launch, resolution, warm-up, frames, repeats, `gpu`, owned metrics, guards. `just perf list` shows it; the bench's `every_registry_row_resolves_to_its_row` test checks a bench row's preset.
3. **Counter guard.** A test beside the row's game that builds it headless, asserts its workload, then checks a `CounterGuard` against its pin; write the pin with `TUNGSTEN_GUARDS_WRITE=1` and run it twice.
4. **Smoke rows** for a bench's `min` and `default` in `scripts/smoke-examples.sh`, with their expected runs in `scripts/test-smoke-examples.sh`.
5. **Capture.** A valid `just perf run <row> --repeat 3` with matching digests; a GPU row with a draw or upload change gets a `counters` pin (`just perf counters --write`).
6. **Docs.** A section here. A change to an existing row's work bumps its `workload_version`.
