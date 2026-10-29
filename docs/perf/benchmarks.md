# Benchmarks

The rows `just perf` measures (`D-078` as redo 7.3 amends it), each owning one cost, and the counter guards that `just check` runs. `scripts/perf-rows.json` is the one registry: each row's package and folder, its launch, resolution, warm-up, frames, repeats, GPU runs, owned metrics, guards, known-drift metrics, budgets and `workload_version`. [`profiling-workflow.md`](profiling-workflow.md) covers running them, captures, sittings and verdicts. Dated results live in the captures under `perf-runs/` and in the history of this file; the levers below point at them.

```bash
just perf list                                   # every row, its kind and repeats
just perf run ecs                                # one capture
just perf sitting --quick                        # the working tree against the last tag, CPU rows, informational
cargo run -p example-02-bench                    # physics at `default`, interactive; Tab cycles the five benchmarks
TUNGSTEN_BENCH=gpu TUNGSTEN_BENCH_PRESET=min cargo run -p example-02-bench
```

**Reference machine.** Ryzen 5 6600H, Radeon 660M (Mesa RADV `REMBRANDT`), Vulkan, `immediate` present mode with latency 1, governor `performance`, generic x86-64 release builds with frame pointers, every function and non-fallthrough block aligned to 64 bytes (`just perf`'s default flags). Its numbers are informational (`D-070`).

## Harness

- **Two kinds of row.** The five benchmarks of `examples/02_bench/` (package `example-02-bench`) run one per launch, selected by `TUNGSTEN_BENCH` and `TUNGSTEN_BENCH_PRESET` and captured under `TUNGSTEN_SMOKE_FRAMES`, which pins dt to 1/60 s. The game rows run a game's own build in script mode (`D-225`) on an inline script the registry holds: setup steps, then the warm-up and the window, ending on the row's asserts; script mode pins dt too.
- **The bench binary.** `main.rs` resolves the configuration (`knobs.rs`: preset, then scale, then overrides, then validation) and builds the app through `bench_app`, which the counter guards build alike. `TUNGSTEN_BENCH_DESCRIBE=1` prints every benchmark's knobs and presets as JSON and `=config` the resolved configuration, before a window opens. `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` enables overlays. In an interactive run Tab relaunches the binary on the next benchmark, keeping the preset when it has it.
- **Determinism.** Every random choice comes from `Pcg32::seeded(seed)` and dt is pinned, so one build replays one workload. No rendered text depends on wall-clock time. A capture's runs share a digest over the window's `physics:` and `bench:` lines.
- **Assets.** `gen.rs` generates every bench texture under `bench_` IDs through a `TextureSink`: the renderer uploads them, and the counter guards' headless sink hands out the harness's handles and uploads nothing, so both register the same sprites, pages and batch keys. The bench's only asset files are `examples/02_bench/assets/manifest.json` and the `bench_heavy` shader and material it lists, which `gpu` loads after the root manifest.
- **Lean view.** `view.rs` is a culled, unsorted extract over `Position` plus `ViewSprite`. `physics`, `ecs` and `churn` draw through it, so their render cost stays flat as they scale; `gpu` and `particles` use the engine's extracts.
- **Telemetry.** `counters.rs` logs `bench-config:` once and `bench:` counters every frame, on log target `bench`; a frame's `bench:` line prints in the next frame.
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

## `e01-pit`

Example 01's heaviest frame: the full ball pit under the rope bridge with six enemies awake about the player (`features/run/pit.rs`). The script: `start_run` at frame 2, `fill_pit` to 12,000 balls at 30, `wake_foes` at 630, then 180 warm-up frames (warm-up 810 in all) and 300 measured; it ends asserting the `pit` probe (`balls` = 12,000, `foes_alive` = 6, `foes_awake` ≥ 5). 1920×1080, CPU, five repeats.

- **Owned:** `total` p50/p95; `update`, `physics_step` and `extract` p50.
- **Guard:** the script's report.

## `template`

The template's sample game, the frame a new game starts from. The script: `start_run` at frame 2, then 120 warm-up frames (warm-up 122) and 300 measured; it ends asserting the `player` probe's health above 0, since the template has no state probe. 1920×1080, CPU, five repeats.

- **Owned:** `total` p50/p95; `update` and `extract` p50.
- **GPU counters:** pinned in `just perf counters`.

## `e01-areas`

Six rows of example 01 at 3840×2160, GPU runs only, three repeats: `e01-area-clearing`, `-aqueduct`, `-ravine`, `-hollow`, `-mine` and `-bridge`, the areas of M77's survey. The script: `start_run` at frame 2, then `place_player` from frame 30 through the last frame at a point standing in the area (`clearing`, `aqueduct_floor`, `rv_ledge_2`, `hollow_floor_1`, `cut_floor_1`, `troll_bridge` of `tools/level.json`), 60 warm-up frames (warm-up 90) and 300 measured, asserting the `area` probe names the area.

- **Owned:** the scene pass and `render_span` p50/p95; `post0_bloom_threshold`, `post0_bloom_composite` and `post1_vignette` p50.
- **GPU counters:** `e01-area-clearing` is pinned in `just perf counters`.

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
| `pit` | `example-01-platformer` `guards::the_full_pit_holds_its_counts` | `start_run`, 2,000 balls, 120 settle frames, `wake_foes`, a 60-frame window | the `pit` counts (headless 01 loads no sprite assets, so its batches read 0) |

Each bench guard runs its row's warm-up and a 60-frame window at the configuration named, and asserts the row its configuration measures before any pin. A rise in a cost counter needs a fix or its cause in the change's plan. The template has none: `tungsten new` copies the template's tests into every game.

**GPU counters** stay local: `just perf counters [--write]` runs `gpu`, `gpu-throughput`, `template` and `e01-area-clearing` at the row's warm-up plus 30 frames and checks every window frame's `counters:` line against the row's `counters` pin in the registry, `{"each": n}` for a count every frame shares or `{"sum": n}` for the window's total. `draw_calls` and `upload_bytes` are the engine's own; glyphon's text work is `text_renders`.

## Levers for 9.2

<a id="engine-findings"></a><a id="open-proposals"></a>Every engine finding, open proposal and item left open by a pass in this file at `2464cbc9`, one line each: an open lever with its evidence, or `closed` with what closed it. The evidence is this file's history (`git show 2464cbc9:docs/perf/benchmarks.md`, the section named) and the decisions. 9.2 picks from the open ones.

- **Burst latch:** closed (`D-143`).
- **Tiles at `z_norm` 0:** open. `extract_tilemaps` writes the nearest depth, so under `gpu_depth` the tilemap covers every sprite; Engine findings.
- **Lit-plus-material warning:** closed (0.61, M43).
- **Text-cache RSS:** closed (`D-085`).
- **Default extract:** open in part. Rewritten (`D-086`), interned (`D-113`) and culled (`D-114`); it still leads `gpu-throughput` (12.15 of 13.68 ms `total` p50 on 2026-10-04), and the bound test costs rows with every sprite in view; Engine findings, `gpu`.
- **Surface clear and present blit:** closed for the blit (`D-087`); the clear in the first swapchain pass stays, which only `LoadOp::DontCare` (`unsafe`) removes; Engine findings.
- **Tile collision proxies:** open. Rebuilt from a full-map scan every frame, 8,528 proxies and about 1.9 ms of `physics_step` in the retired `integrated` row; Engine findings.
- **Contact wakes rebuild the pair list:** open. Re-pairing a woken proxy would remove about 2.5 builds a frame where wakes are common, and needs its own decision (`D-081`); Engine findings.
- **Bundle insert:** closed (`D-135`).
- **Archetype count:** open. `ecs` `update` p50 9.14 / 10.52 / 19.15 ms at 64 / 488 / 3,189 archetypes, consistent with each column's first cache lines missing; no candidate without `unsafe`; Engine findings.
- **String sprite IDs:** closed (`D-113`); animation IDs stay strings (below).
- **G1 kinematic bodies in a row:** open. The engine side landed (`D-154`); a row that uses them is a workload change.
- **G2 sensor bins in `physics`:** open. Sensors landed (`D-154`); a workload change.
- **G3 more than 16 lights:** open. `LIGHT_CAP` is 16.
- **G4 GPU particle simulation:** open.
- **T1 particle and tween timings:** closed (`D-133`).
- **T2 a `startup:` line:** open. Window, renderer, manifest, user startup and audio times per capture.
- **M1 a counting allocator:** closed (redo 7.3: `CountingAlloc` in test binaries, the counter guards).
- **Below the calibration band:** open. `physics` (6.07 ms `total` p95 against 14.58 at calibration), `physics-sparse` (3.73 against 10.11), `churn` (4.06 against 11.3) and `particles` (4.92 on 2026-10-04 against 11.6) run under the 8–16 ms band; recalibrating is a workload change; Open proposals. `integrated`'s line is closed: the row is retired (redo 7.3).
- **RSS growth threshold:** open. Judge growth for `churn` only (32 KiB/s median, 16 KiB/s every run); Open proposals.
- **`ecs`'s per-run mode:** open. Seven system rows take a second value in some runs with the same digest; cause not looked for; `ecs`.
- **The scene pass's two values in the GPU run:** open, to recheck. They followed the blocking readback's timing; redo 7.3's ring reads timestamps without blocking, so 7.15 checks whether they remain; `gpu`.
- **`integrated`'s row note naming the old text cache:** closed (the row is retired, redo 7.3).
- **Compositor-bypass hint on X11:** open. KWin's acquire stall sets `gpu`'s tail at `min`; Session B's list.
- **Shape-run cache for text shaping:** open; Session B's list.
- **Frame cap overshoot:** open. 0.06 ms per frame at a cap of 60, each deadline counting from the frame's own start; a deadline grid would remove it; Session B's list.
- **The HUD's `fps` row:** closed (M57, `D-165`).
- **Interned animation IDs:** open. `AnimationState.animation_id` is a `String` looked up per entity per advance; M33's list.
- **`particles`' `unattributed` reading after `D-113`:** open, mechanism not established; M33's list.
- **A cheaper cull for all-in-view rows and a view test for `extract_mesh_particles`:** open; M33's list.

## Adding a row

A row and its counter guard land with the system step that needs them (redo §5.8).

1. **Workload.** A bench benchmark under `examples/02_bench/src/<bench>.rs`, registered in `main.rs`, seeded from `seed`, with knobs, presets (`min` and `default` at least) and `bench:` counters that prove the workload; or a game row's script on hooks its game registers.
2. **Registry.** An entry in `scripts/perf-rows.json` at `workload_version` 1: package, folder, launch, resolution, warm-up, frames, repeats, `gpu`, owned metrics, guards. `just perf list` shows it; the bench's `every_registry_row_resolves_to_its_row` test checks a bench row's preset.
3. **Counter guard.** A test beside the row's game that builds it headless, asserts its workload, then checks a `CounterGuard` against its pin; write the pin with `TUNGSTEN_GUARDS_WRITE=1` and run it twice.
4. **Smoke rows** for a bench's `min` and `default` in `scripts/smoke-examples.sh`, with their expected runs in `scripts/test-smoke-examples.sh`.
5. **Capture.** A valid `just perf run <row> --repeat 3` with matching digests; a GPU row with a draw or upload change gets a `counters` pin (`just perf counters --write`).
6. **Docs.** A section here. A change to an existing row's work bumps its `workload_version`.
