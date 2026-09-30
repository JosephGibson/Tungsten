# Benchmarks

`examples/02_bench/` (package `example-02-bench`) holds six scalable, deterministic benchmarks, each owning one bottleneck (`D-078`). One binary runs one benchmark per launch. This file describes what each benchmark measures, its knobs, guards and owned metrics, and the calibrated defaults on the reference machine. [`profiling-workflow.md`](profiling-workflow.md) covers configuration, captures, compare, capacity search and the capture rules.

```bash
cargo run -p example-02-bench                                       # physics at `default`, interactive
TUNGSTEN_BENCH=gpu TUNGSTEN_BENCH_PRESET=min cargo run -p example-02-bench
just perf describe gpu                                              # knobs, presets and tracked rows from the binary
```

**Reference machine.** Ryzen 5 6600H, Radeon 660M (Mesa RADV `REMBRANDT`), Vulkan, `immediate` present mode with latency 1, Linux 7.2.7, rustc 1.98.1, governor `performance`, generic x86-64 release builds with frame pointers (`just perf`'s default flags). Every number below comes from that machine, measured 2026-09-30, and is informational (`D-070`). Numbers from the retired perf scenes aren't comparable with this suite.

## Harness

- **One binary.** `main.rs` resolves the configuration (`knobs.rs`: preset, then scale, then overrides, then validation) and calls the benchmark's `configure`. Without `TUNGSTEN_BENCH` it runs `physics` at `default`. `TUNGSTEN_BENCH_DESCRIBE=1` prints every benchmark's schema as JSON and `=config` the resolved configuration, both before a window opens. `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` enables overlays for interactive use.
- **Determinism.** Every random choice comes from `Pcg32::seeded(seed)`, and captures run under `TUNGSTEN_SMOKE_FRAMES`, which pins `dt` to 1/60 s, so one build replays an identical workload. No rendered text depends on wall-clock time.
- **Assets.** `gen.rs` generates every texture (discs, soft dots, sphere and bevel normal maps, emissive masks, tiles, parallax strips, animation frames) under `bench_` IDs, so the suite ships no image files. Its only asset files are `examples/02_bench/assets/manifest.json` and the `bench_heavy` shader and material it lists. `gpu` loads that manifest after the root manifest; `integrated` loads the root manifest alone, for its fonts and the `damage_flash` material; the others load none. Manifest composition replaces the tilemap, animation and particle registries, so benchmarks register those in their startup hook.
- **Lean view.** `view.rs` is a culled, unsorted extract over `Position` plus a small `ViewSprite`. `physics`, `ecs` and `churn` draw through it, so their render cost stays flat as scale grows. `gpu`, `particles` and `integrated` use the engine extracts, whose cost they own or report.
- **Telemetry.** `counters.rs` logs `bench-config:` once (the resolved config as JSON, plus derived values) and `bench:` counters every frame, on log target `bench`. Frame N + 1's first system logs frame N's counters, so each `bench:` line follows its frame's `frame:`, `systems:`, `gpu_passes:` and `physics:` group; the last frame has none.
- **Engine hook.** `Bench::engine_config` runs before the window opens. `gpu` and `integrated` apply `aa` and `bloom_mips` there, except where `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_POST_AA` or `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` is set: the environment wins and a warning names the overridden knob. The runner clears those variables, so captures follow the knobs.
- **Workload version.** Each benchmark has a `workload_version`, logged in `bench-config` and compared as a hard field. A bench code change that alters the work bumps it; engine changes never do. Every benchmark is at version 1. Each section lists its rows' determinism digests at the default on the reference machine: a harness change that moves one changed the work, while an engine change may move them without a version bump.
- **Tests.** `src/tests/` covers knob resolution and validation and the pachinko geometry invariants; `tests/visual_regression.rs` is the `gpu` visual test.

## Ownership

Each benchmark is judged only on the costs it owns. Every other metric is reported and never produces a verdict against that benchmark.

| Cost | Owner, judged on | Also present in, reported only |
| --- | --- | --- |
| Narrow phase, contact build, solve, collision events, sleep bookkeeping | `physics`: `physics_step` p50/p95, `update` p95 | `integrated` |
| Proxy gather, broadphase build, pair query | `physics-sparse`: `physics_step` p50/p95 | `physics`, `integrated` |
| Query iteration, column access, random entity lookup | `ecs`: `update` p50/p95, the 14 system rows at p50 | all |
| `CommandBuffer` recording and flush, archetype moves, entity allocation and free | `churn`: `flush` p50/p95, the four churn system rows at p50 | `particles`, `integrated` |
| Particle emit, tick and count refresh; animation playback | `particles`: `unattributed` p50/p95 (the untimed particle stage), `animate_sprites` p50/p95 | `integrated` |
| Default extract, encode, text shaping, GPU passes | `gpu`: scene pass and `render_span` p50/p95, pass rows at p50, `extract` and `render_encode` p50/p95; `gpu-throughput`: `extract` and `render_encode` p50/p95 | `particles`, `integrated` |
| Cross-system effects: tile proxies × bodies, event → spawn cascades, batch fragmentation, full-frame composition | `integrated`: `total` p50/p95/p99, jitter (p99 − p50) | — |
| Acquire and present pacing | Nobody: environment, reported and never judged | all |

Peak RSS is reported, with its own verdict in compare, for every row. RSS growth is reported for every row and judged for none; see "Open proposals".

## `physics`

Two rows. `physics` (pachinko) owns the narrow phase, contact build, solve, collision events and sleep bookkeeping; `physics-sparse` owns proxy gather, broadphase build and pair query. Warm-up 120 frames, no GPU diagnostic run. Module: `physics.rs`.

**Pachinko.** Balls fall through a staggered peg field into bins and recirculate.

- Balls use the `radius_mix`: `mixed` is 3, 4.5, 6 and 9 px at 35/30/25/10% (mean area 82.7 px², largest diameter `d_max` 18 px), with restitution 0.3 and gravity 900 px/s².
- Pegs are static 5 px circles in staggered rows: horizontal pitch `p_x = max(64, ceil16(3·d_max + 2·r_p))` = 64 px, vertical pitch `0.75·p_x` = 48 px, odd rows offset by `p_x/2`. The in-row gap (3.0 `d_max`) and the diagonal gap (2.65 `d_max`) keep any ball from wedging.
- Top to bottom: an 80 px ceiling, a 128 px spawn band, 32 peg rows (1,536 px) and a 160 px bin zone, for an interior height of 1,824 px. Dividers stand every 128 px, each capped with a circle; each bin has a static floor tagged `BinFloor`.
- The board width is `balls · mean area / (fill · 1,824)`, rounded up to whole bins; the height stays fixed as `balls` scales. At the default: 1,024 × 1,824 px, 8 bins, 496 pegs (99 bumpers), actual fill 0.354. The camera fits the height (zoom 0.592); the culled view draws only visible balls, so render cost plateaus while physics grows.
- Nothing may sleep, so four mechanisms keep every slow streak under the engine's 0.5 s (`D-065`):
  1. Recirculation: a bin floor teleports a ball to the spawn band on its first floor contact. The consumer runs after `physics_step` and reads `EventQueue::<CollisionEvent>::iter_current()`.
  2. Bumpers: `bumpers` of the pegs set a touching ball's speed along the contact normal to at least `kick`.
  3. Anti-stall: a ball below 30 px/s for 0.25 s gets a seeded 200 px/s kick, and respawns at 0.4 s.
  4. The guard below.

**Sparse.** `bodies` circles of `radius` 2 px move at 600–1,400 px/s in random directions with no gravity and restitution 1, in an arena of area `bodies · π · radius² / density` (4,224 × 2,368 px at the default). Speeds are renormalized into `[speed_min, speed_max]` every frame. The camera zoom is 0.5, so about 6,660 bodies are visible whatever the arena size.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `mode` | `pachinko` | `pachinko`, `sparse` | | |
| `balls` | 8,000 | 200–400,000 | yes | Board width follows |
| `fill` | 0.36 | 0.05–0.40 | | Ball area ÷ board area |
| `radius_mix` | `mixed` | `uniform`, `mixed`, `wide` | | `uniform`: 4.5 px; `wide`: 2/4/8/16 px at 40/30/20/10% |
| `peg_radius` | 5 | 2–12 px | | |
| `bumpers` | 0.2 | 0–1 | | Share of pegs that kick |
| `kick` | 260 | 0–1,000 px/s | | Bumper exit speed |
| `gravity` | 900 | 100–3,000 px/s² | | |
| `restitution` | 0.3 | 0–1 | | Balls |
| `bodies` | 8,000 | 1,000–2,000,000 | yes | Sparse only; arena area follows |
| `radius` | 2 | 1–8 px | | Sparse only |
| `speed_min`, `speed_max` | 600, 1,400 | 50–8,000 px/s | | Sparse only |
| `density` | 0.01 | 0.001–0.05 | | Sparse only; body area ÷ arena area |
| `substeps` | 4 | 1–8 | | `PhysicsConfig::substeps` |
| `iterations` | 1 | 1–8 | | `PhysicsConfig::solver_iterations` |
| `cell` | 32 | 8–256 px | | `PhysicsConfig::broadphase_cell_size` |
| `sleep` | `on` | `on`, `off` | | Diagnostic; the guard requires zero sleepers either way |
| `render` | `culled` | `culled`, `none` | | `none` draws no bodies |
| `seed` | 1 | u64 | | Every benchmark has it |

- **Presets:** `min` (1,000 balls), `default`, `sparse-min` (2,000 bodies) and `sparse`.
- **Counters:** `balls`, `teleports`, `bumper_kicks`, `stall_kicks`, `respawns`, `events` and `visible` (pachinko); `bodies`, `events` and `visible` (sparse). The engine's `physics:` line adds proxies, dynamic, sleeping, pairs and contacts.
- **Guards:** `physics.sleeping <= 0` on both rows; `bench.teleports >= 1` on `physics`. At `min` the flow is about 3 teleports per frame, so single frames can see none and fail the teleport guard.
- **Owned:** `physics`: `physics_step` p50/p95 and `update` p95. `physics-sparse`: `physics_step` p50/p95. Both declare `physics_step` as the bottleneck; key knobs `balls` and `bodies`.

**Calibrated defaults** (medians of 3 runs):

| Row | Default change | World | `physics_step` p50 / p95 | `update` p95 | `total` p95 | Calibration check |
| --- | --- | --- | --- | --- | --- | --- |
| `physics` | `fill` 0.21 → 0.36; `balls` stays 8,000 | 1,024 × 1,824 px, 8 bins | 13.37 / 13.99 ms | 14.06 ms | 14.58 ms | 1.11 contacts per ball (≥ 1.0 ✓) |
| `physics-sparse` | `bodies` 50,000 → 8,000 | 4,224 × 2,368 px | 8.06 / 9.83 ms | 9.86 ms | 10.11 ms | 0.229 pairs and 0.118 contacts per proxy (≤ 0.5 and ≤ 0.2 ✓) |

- Pachinko at `fill` 0.21 gave 0.64 contacts per ball; 0.25, 0.30, 0.35 and 0.40 gave 0.74, 0.89, 0.99 and 1.11. Whole bins mean 1.0 needs 8 bins, so `fill` ≥ 0.3542. Stall kicks stay near 0.01 per frame and respawns at 0.
- Sparse at 50,000 bodies gave a `total` p95 of 78 ms; 6,000, 8,000, 10,000 and 12,000 bodies gave 7.8, 10.6, 13.9 and 15.7 ms. A frame-pointer profile at 8,000 splits `physics_step` into pair query (`build_pairs`) 54.5%, `speculative_pass` 29.9% (22% of it `SpatialGrid::query`), grid build 6.2%, narrow phase 1.4% and solver 0.7%: broadphase-bound, as intended.
- Peak RSS is about 132 MiB (pachinko) and 128 MiB (sparse). Pachinko RSS grows 2–8 KiB/s and sparse RSS not at all, reported only.

**Workload version history:** 1, the initial version (2026-09-30). Digests at the default: `physics` `c972b2818d486617`, `physics-sparse` `c95fadc938a3689e`.

## `ecs`

Owns steady-state query iteration. Warm-up 60 frames, no GPU diagnostic run. Modules: `ecs.rs`, `ecs/systems.rs`.

- **Components.** Up to 16 per entity: `Position` (the engine's), `Vel`, `Acc`, `Heading`, `Health`, `Regen`, `Cooldowns([f32; 4])`, `Brain`, `Team`, `Faction`, `Stats([f32; 8])`, `Tint`, `Phase`, `Age`, `Follow(Entity)` and `Bag([u16; 16])`. Zero-sized tags `T0`–`T7` fragment the archetypes.
- **Systems,** in run order, each a different query shape: `brain` (a state machine over `query3_mut`), `cooldowns` (branching timers), `buffs` (the `query2_opt2_mut` path), `regen` and `stats_decay` (sparse matches), `accelerate` (a dense write over the 80% holding `Acc`), `follow` (one `World::get::<Position>` per follower), `integrate`, `bounds_wrap` and `heading` (dense writes, `heading` via `atan2`), `tint` and `age_phase` (cheap ticks), `team_bags` and `faction_histogram` (read-only reductions). `dense` selects the 8 whole-population systems and `sparse` the 6 optional-component ones. A last system, `ecs_audit`, counts structural work.
- **Invariants.** Nothing structural happens after startup, and only the `view` sample draws.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `entities` | 250,000 | 1,000–4,000,000 | yes | |
| `fragmentation` | 8 | 1–256 | | Tag-component combinations |
| `optional` | `on` | `on`, `off` | | `on`: `Acc` 80%, `Regen` 40%, `Stats` 50%, `Bag` 25%; `off`: every entity carries all four |
| `followers` | 0.1 | 0–0.5 | | Share doing a random-access leader lookup |
| `systems` | `all` | `all`, `dense`, `sparse` | | |
| `view` | 4,096 | 0–65,536 | | Rendered sample, fixed across scale |

- **Presets:** `min` (1,000 entities) and `default`.
- **Counters:** `entities`, `archetypes` (distinct signatures spawned; 488 at the default), `lookups` (24,967 at the default), `structural` (commands queued during `update` plus any change in the live entity count) and `digest`, an FNV-1a hash of every position plus the two reductions. The hex digest stays out of the counter statistics and feeds the determinism check; computing it costs 0.39 ms per frame inside `update`.
- **Guards:** `bench.structural <= 0` and `bench.entities constant`.
- **Owned:** `update` p50/p95 plus each of the 14 system rows at p50. The bottleneck is `update` (any system); key knob `entities`.

**Calibrated default:** `entities` stays 250,000. `total` p95 per run 12.29, 12.61 and 12.23 ms; `update` p50 10.84 / p95 11.84 ms; `update` is 95.7% of `total` (≥ 70% ✓). Per-system p50 medians: `heading` 2.59, `brain` 2.22, `follow` 1.90, `buffs` 0.66, `age_phase` 0.48, `cooldowns` 0.45, `tint` 0.41, `integrate` 0.27, `accelerate` 0.26, `bounds_wrap` 0.26, `team_bags` 0.25, `faction_histogram` 0.25, `stats_decay` 0.17 and `regen` 0.09 ms. Peak RSS is about 163 MiB, with no RSS growth.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `c785506ced04de4c`.

## `churn`

Owns structural change: command recording, `World::flush`, archetype moves, and entity allocation and free. Warm-up 60 frames, no GPU diagnostic run. Module: `churn.rs`.

- **Population.** A steady population is replaced FIFO: every entity lives `1/turnover` frames (20 at the default), and the population is pre-aged, so churn is steady from the first frame. Frame f despawns spawn serials `[f·S, (f+1)·S)` and spawns `[P + f·S, P + (f+1)·S)`. Each spawn inserts `components` components one at a time, one archetype move each.
- **Status toggles.** Survivors whose serial is f modulo `groups` gain one of `statuses` status components at frame f and lose it at f + 1. Gains skip entities that expire at f + 1, so each frame's losses equal the previous frame's gains. `groups = round(2·(P − 2S) / (toggles·P))`, 36 at the default.
- **Modes.** `deferred` records everything through the `CommandBuffer` and measures it in `flush`; `immediate` makes the same calls on `World` inside the churn systems. Both find their targets with one scan over the lifetime component, and the systems run as `churn_scan`, `churn_despawn`, `churn_toggle`, `churn_spawn`.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `population` | 125,000 | 1,000–2,000,000 | yes | |
| `turnover` | 0.05 | 0–0.5 | | Share replaced per frame, FIFO |
| `toggles` | 0.05 | 0–0.5 | | Share gaining or losing a status per frame; a status lasts one frame |
| `components` | 6 | 2–12 | | Components per spawn, one archetype move each |
| `statuses` | 3 | 1–8 | | Distinct status components |
| `mode` | `deferred` | `deferred`, `immediate` | | |
| `view` | 0 | 0–16,384 | | Rendered sample |

- **Presets:** `min` (population 1,000) and `default`.
- **Counters:** `population`, `spawned`, `despawned`, `inserted`, `removed` and `commands`. At the default each frame spawns and despawns 6,250 entities and makes 3,125 gains and 3,125 losses: 40,625 inserts, 3,125 removals and 56,250 commands.
- **Guards:** `bench.population constant` and `bench.spawned == bench.despawned`.
- **Owned:** `flush` p50/p95 plus `churn_scan`, `churn_despawn`, `churn_toggle` and `churn_spawn` at p50. The bottleneck is `flush`; key knob `population`.

**Calibrated default:** `population` 100,000 → 125,000. `total` p95 per run 11.37, 11.28 and 10.95 ms; `flush` p50 8.74 / p95 10.01 ms; `flush` plus the churn systems are 96.8% of `total` (≥ 60% ✓). At 100,000 the p95 sat within 1 ms of the band floor; a sweep gave 9.01, 10.99, 14.37 and 16.81 ms at 100,000, 125,000, 150,000 and 175,000. Peak RSS is about 143 MiB.

Command-buffer overhead, means in ms from the median run of each mode:

| Mode | `flush` | `churn_scan` | `churn_despawn` | `churn_toggle` | `churn_spawn` | Structural total | `total` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `deferred` | 8.54 | 0.43 | 0.03 | 0.10 | 0.35 | 9.45 | 9.75 |
| `immediate` | 0 | 0.35 | 1.41 | 2.18 | 2.99 | 6.93 | 7.19 |

- The `CommandBuffer` costs 2.52 ms per frame, 36% over the direct calls or about 45 ns per command: a boxed command and setter per operation, plus replay. Peak RSS is 4.5 MiB higher deferred.
- Toggle moves are memory-latency bound. Spawns and despawns alone (`toggles=0`) cost 4.18 ms of `flush` for 50,000 commands, about 0.1 µs per move; the 6,250 toggle moves add 4.36 ms, about 0.70 µs each, because toggled rows are scattered through a large archetype.
- Swap-remove scrambles the archetype layout over time, so structural cost depends on access locality as much as on count: a change to which entities a system touches can move `flush` severalfold at the same command count.
- RSS growth is 0.0 KiB/s in both modes at the default: reported only.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `86014148b7a94681`.

## `gpu`

Owns the render path: the default extract, encode, text shaping and every GPU pass. Two rows: `gpu` (preset `default`) and `gpu-throughput` (preset `throughput`). Warm-up 60 frames; the GPU diagnostic run is on. Modules: `gpu.rs` and `gpu/scene.rs`, plus the manifest and `bench_heavy.wgsl` under `examples/02_bench/assets/`.

- **Sprite field.** `sprites` sprites over `z_layers` interleaved z layers. Weyl sequences assign the kinds so every layer interleaves them: `lit` orbs and crates with normal and emissive maps, `shader_share` sprites drawn with `bench_heavy`, and unlit textures (`textures` of them, alternating nearest and linear filtering). The field and the glow layer carry `ParallaxLayer` factor 0, so they stay on screen while the camera scrolls; nothing moves them after startup.
- **`bench_heavy`.** It keeps `damage_flash.wgsl`'s vertex inputs and bind layout and loops `material.i.x` times per fragment (capped at 4,096). `shader_iters` is written into the material's authored defaults, so every heavy sprite shares one uniform block.
- **Glow.** `glow` soft alpha-blended sprites of `glow_px` supply particle-like overdraw.
- **Lights.** `lights` point lights on seeded paths plus one directional light; `LIGHT_CAP` is 16.
- **Tilemap.** A render-only 2,048 × 512-tile map, `tile_layers` deep, scrolls under a scripted camera that ping-pongs at `scroll` px/s. The tilemap extract culls it; the tiles share one atlas page.
- **Text.** 100 sections of 40 characters, `text_change` of them rebuilt every frame from the frame index.
- **Post.** `post` selects the chain, with `aa` and `bloom_mips` on top.
- **Batches.** `bench-config` derives the expected batch count: at the default 56 sprite batches (8 layers × 7 keys), 1 glow batch and 1 per tile layer, 60 in all. The startup hook checks the sprite extract against it and logs the result.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `sprites` | 2,000 | 0–2,000,000 | yes | |
| `sprite_px` | 24 | 4–128 | | |
| `textures` | 4 | 1–8 | | Distinct unlit textures, alternating nearest and linear filtering |
| `z_layers` | 8 | 1–64 | | Interleaving; drives the batch count |
| `lit` | 0.5 | 0–1 | | Share with normal and emissive maps |
| `lights` | 16 | 0–16 | | Moving point lights, plus one directional (G3 for more) |
| `glow` | 20,000 | 0–500,000 | yes | Overdraw sprites |
| `glow_px` | 48 | 8–256 | | |
| `shader_share` | 0.1 | 0–1 | | Share drawn with `bench_heavy`; `lit + shader_share <= 1` |
| `shader_iters` | 32 | 0–512 | | Per-fragment loop count |
| `tile_layers` | 3 | 0–4 | | |
| `tile_px` | 16 | 8, 16, 32 | | |
| `scroll` | 600 | 0–4,000 px/s | | |
| `zoom` | 1.0 | 0.25–2 | | |
| `glyphs` | 4,000 | 0–50,000 | yes | 40 per section |
| `text_change` | 1.0 | 0–1 | | Share of sections rewritten per frame |
| `post` | `light` | `none`, `light`, `full` | | `light`: bloom, vignette. `full`: bloom, god rays, fog, color adjust, tonemap, chromatic aberration, film grain, vignette |
| `aa` | `smaa_high` | `off`, `smaa_high`, `msaa4` | | `TUNGSTEN_RENDER_MSAA` and `TUNGSTEN_RENDER_POST_AA` win when set |
| `bloom_mips` | 6 | 1–8 | | `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` wins when set |
| `resolution` | 1920x1080 | 1280x720, 1600x900, 1920x1080, 2560x1440, 3840x2160 | | |

- **Presets:**
  - `min`: 1,000 sprites, 200 glow sprites, 4 lights, one tile layer, 200 glyphs, `post=light`, `aa=off`. Smoke runs the M25 MSAA × depth-sort matrix here.
  - `default`.
  - `throughput`: 400,000 unlit 8 px sprites with one texture and one z layer; no glow, lights, shader, tiles, text, post or AA.
  - `visual`: 2,000 sprites over 3 z layers (half lit), `bench_heavy` at 8 iterations, 4 point lights, one tile layer, 64 static glyphs, bloom and vignette, AA off, 1280×720 and a fixed camera. `just visual` compares its frame 5 with `examples/02_bench/tests/fixtures/gpu-visual.png`, whose README records the reference machine and regeneration command.
- **Counters:** `sprites`, `glow`, `visible_tiles`, `glyphs_changed` and `lights`.
- **Guards:** none. Without timestamp queries the GPU rows are n/a and the capture stays valid.
- **Owned:**
  - `gpu`: `gpu` (the scene pass) and `render_span` at p50/p95; the passes `post0_bloom_threshold`, `post0_bloom_composite`, `post1_vignette`, `smaa_edges`, `smaa_blend_weights`, `smaa_neighborhood`, `text` and `present` at p50; `extract` and `render_encode` at p50/p95. The bloom mip passes are reported, not owned. Bottleneck `present` (the GPU wait shows as present time); key knob `sprites`.
  - `gpu-throughput`: `extract` and `render_encode` at p50/p95. Bottleneck `extract`; key knob `sprites`.
- **Row note (`gpu`):** `gpu` (the scene pass), `render_span` and the pass rows come from the GPU diagnostic run; without timestamp queries they are n/a and the capture stays valid.

**Calibrated defaults** (medians of 3 runs):

| Row | Default change | `total` p50 / p95 / p99 per run (ms) | Owned (ms) | Calibration check |
| --- | --- | --- | --- | --- |
| `gpu` | `sprites` 120,000 → 2,000; `post` `full` → `light` | 11.09/12.55/14.08, 11.10/12.77/13.95, 11.12/12.65/14.45 | scene pass p50 7.82 / p95 8.30; `render_span` 11.48 / 11.95; `extract` 1.95 / 2.28; `render_encode` 2.68 / 3.07 | GPU span 11.19–11.46 ms > `update` + `extract` + `render_encode` 4.74–4.79 ms (means) ✓ |
| `gpu-throughput` | none; not calibrated to the band | 25.62/26.22/27.20, 25.63/26.18/26.52, 25.57/26.38/26.58 | `extract` p50 23.99 / p95 24.45; `render_encode` 1.31 / 1.51 | — |

- The design defaults gave a `total` p95 of 161 ms (scene pass 152 ms, about 1.2 µs per sprite). With no sprites, the full post chain (5.7 ms of GPU time, 2.6 of it god rays), SMAA and the fixed layers still gave 14.7 ms. SMAA cost depends on content: `smaa_blend_weights` rose from 0.23 to 2.56 ms on bare tiles. With `post=light`, 0, 2,000, 4,000 and 8,000 sprites gave p95 10.38, 12.81, 15.47 and 20.37 ms.
- `gpu-throughput` spends about 60 ns per sprite in `extract`, mostly the sort.
- The scene pass is stable within a capture (runs within 2%) but moved about 12% between captures of one configuration, so compare can read the GPU rows `noisy`.
- Peak RSS is about 731 MiB for `gpu` (the text buffer cache; see "Engine findings") and 251 MiB for `gpu-throughput`.

Light sweep (`--sweep lights=0,4,8,16 --repeat 3`), medians in ms. At 2,000 sprites, 1,000 of them lit, lighting adds little:

| `lights` | Scene pass p50 / p95 | `render_span` p50 / p95 | `extract` p50 | `render_encode` p50 | `total` p95 |
| --- | --- | --- | --- | --- | --- |
| 0 | 6.54 / 6.68 | 10.20 / 10.36 | 1.93 | 2.93 | 11.89 |
| 4 | 6.53 / 7.14 | 10.21 / 10.80 | 1.93 | 2.86 | 12.40 |
| 8 | 7.39 / 7.70 | 11.04 / 11.36 | 1.94 | 2.69 | 12.44 |
| 16 | 6.89 / 8.14 | 10.55 / 11.78 | 1.93 | 2.66 | 12.52 |

**Workload version history:** 1, the initial version (2026-09-30). Digests at the default: `gpu` `f45f26ebdf706840`, `gpu-throughput` `121f8ef1503d8697`.

## `particles`

Owns particle emit, tick and count refresh, and animation playback. Warm-up 180 frames, which steady state needs with lifetimes up to 2.5 s; no GPU diagnostic run. Module: `particles.rs`.

- **Emitters.** `emitters` emitters move on Lissajous paths (`motion`) and draw on `configs` generated `ParticleConfig`s, assigned round-robin. The configs span every emission kind (continuous, burst, pulse), every velocity model (cone, radial, vector), gravity and drag, scale, color and alpha curves of 2–6 points, both blend modes and lifetimes of 0.3–2.5 s.
- **Emission.** Config i targets about 150 × `rate` live particles per emitter: continuous configs set `rate_hz` to the target over the mean lifetime, pulses fire every 0.2–0.35 s, and bursts re-arm every 0.4–0.7 s. The engine latches a burst after it fires (see "Engine findings"), so `rearm_bursts` clears the latch. Pulse timers and burst phases start staggered. `ParticleBudget::global_cap` is `budget`, and `bench-config` logs `live_model`: emitters × 150 × `rate`, capped by `budget` (42,000 at the default).
- **Animated sprites.** `animated` sprites play `clips` generated 8-frame clips at `anim_fps`. The clips share one atlas page, so all animated sprites form one batch key. Even clips loop; odd clips are one-shots that `animate_sprites` restarts at frame 0, counted as a frame change. `animate_sprites` walks `query2_mut::<AnimationState, Sprite>` and calls `advance`; the benchmark inserts its own `AnimationRegistry`, since it loads no manifest.
- **Render path.** Particles and animated sprites draw through the default extract. That cost belongs to `gpu` and is reported here.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `emitters` | 280 | 0–20,000 | yes | |
| `configs` | 12 | 1–64 | | Generated configs, assigned round-robin |
| `rate` | 1.0 | 0.1–10 | | Emission multiplier; about 150 live particles per emitter at 1 |
| `budget` | 100,000 | 1,000–5,000,000 | yes | `ParticleBudget::global_cap` |
| `animated` | 8,000 | 0–1,000,000 | yes | |
| `clips` | 8 | 1–32 | | 8-frame clips; odd clips are one-shots the bench restarts |
| `anim_fps` | 12 | 1–60 | | |
| `motion` | `on` | `on`, `off` | | Emitter movement |

- **Presets:** `min` (8 emitters, 500 animated sprites) and `default`.
- **Counters:** `live`, `emitters`, `animated` and `frame_changes`.
- **Guard:** `bench.live within ±10% of its median`. It checks that the live count is steady, not its level; the level shows in `live_model` and in compare's workload-drift check.
- **Owned:** `unattributed` p50/p95 and `animate_sprites` p50/p95. Bottleneck `unattributed`; key knobs `emitters` and `animated`.
- **Row note:** Without T1 the particle stage has no timing of its own: `unattributed` holds it (plus event flush), so the row owns `stage.unattributed` and `animate_sprites`.

**Calibrated default:** `emitters` 400 → 280 and `animated` 40,000 → 8,000. `total` p50/p95/p99 per run 11.02/11.64/12.18, 10.98/11.56/12.18 and 10.89/11.54/11.88 ms; `unattributed` p50 2.38 / p95 2.98 ms; `animate_sprites` p50 0.27 / p95 0.32 ms; `live` median 42,288 (largest deviation 1.9%). Peak RSS is about 142 MiB; RSS grows 4–13 KiB/s, reported only.

- **The share check fails by construction:** `unattributed` plus `animate_sprites` is 24.8–25.1% of `total` against a target of 50% ✗. At the design defaults `live` reached 60,416, but `total` p95 was 24.0 ms and the default extract took 15.71 of 23.03 ms: about 0.16 µs per string-ID sprite, against 0.06 µs in `gpu-throughput`'s one-texture field. Per frame, one live particle costs about 0.24 µs (0.066 of it unattributed) and one animated sprite about 0.2 µs (0.038 in `animate_sprites`), so no mix reaches 50%.
- At 8,000 animated sprites, 240, 280 and 320 emitters gave live medians of 36,258, 42,288 and 48,351 and p95 values of 9.78, 12.20 and 13.42 ms. The owner accepted 280 emitters (about 70% of the design's 60,000-particle target) and 8,000 animated sprites, mid-band.
- Capacity search finds the row `extract`-limited (✗ against the declared `unattributed`) at both budgets: the same finding.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `08a3c7c126a0d43a`.

## `integrated`

Owns the effects that only appear when systems interact, and is the one benchmark judged on total frame time and its tail. Warm-up 120 frames; the GPU diagnostic run is on. Modules: `integrated.rs` (knobs, config and registration), `integrated/level.rs` (the seeded level), `integrated/assets.rs` (generated textures, the level tilemap, walk clips, fire and spark configs), `integrated/scene.rs` (startup spawning), `integrated/runtime.rs` (per-walker arrays and the runtime resource) and `integrated/systems.rs` (gameplay systems in frame order).

- **Level** (seeded, 96 rows of 16 px tiles, `level_tiles` wide). The ground surface wanders between rows 80 and 90 in flat runs of 8–24 columns joined by single steps or staircases of 3–5 steps, over a three-tile collision crust; the rendered terrain fills to the bottom row. Full-height walls close both ends, and walls 1–2 columns wide and 3–6 tiles high stand on the ground every 48–96 columns. Three tiers at rows 72, 60 and 48 (±1) carry one-tile platforms 10–36 tiles long with gaps of 3–9. The tilemap holds the collision layer (the engine never draws it), the rendered terrain and two decoration layers; four parallax strips (factors 0.1–0.7, one atlas page) sit behind. At the default: 24,576 × 1,536 px, 8,528 tile proxies, 317 walkable spans and 81,776 px of walkable surface.
- **Walkers.** Dynamic 12 × 20 px AABBs with tile collision, drawing a 24 px lit walk clip (6 frames, 4 color variants, one lit atlas page with normal and emissive maps). The AI reads last frame's contacts: a wall turns a walker unless it is a one-tile step, which it hops; a ledge (no ground within two tiles ahead) turns it; a random hop comes every 1.5–4 s where ground 56 px ahead supports the landing. Touching down after 6 or more airborne frames sends a `SquashEvent` (`OnLand`) to the engine's squash systems.
- **Casters.** Every tenth walker (250 at the default) fires 3 px circle projectiles at 700 px/s, 8–29° above the horizontal, `fire_rate` times per second.
- **Hits.** A hit despawns the projectile, spawns a spark burst, knocks a struck crate and wakes it with `physics::wake`, adds 0.08 trauma when within 64 px of the view, and flashes a struck walker.
- **Flash.** The default extract draws a lit sprite without its material (lit wins, `D-061`, with a warning per sprite and frame). A struck walker therefore switches to an unlit copy of its clip (a second atlas page and clip set) drawn with `damage_flash`, an override block (color in `vec4[0]`) and a `UniformScalar` F0 tween from 1 to 0 over 0.3 s, tagged `flash`. `TweenComplete` arrives after the systems, so the next frame reads it in the previous window, restores the lit clip and removes the block. Each flashing walker has its own override hash and so its own batch: 17.5 at a time at the default.
- **Sparks.** Each hit spawns a fresh emitter (`Burst { count: 12, once: true }`) through the `CommandBuffer` and despawns it when its `ParticleSystemDrained` arrives, which sidesteps the burst latch.
- **Scenery.** Dynamic crates sit in piles of 10 and may sleep; hits wake them. Props are static, lit and unlit. Pickups bob through position tweens.
- **Torches.** Each carries a point light and a fire emitter. `ParticleBudget` is torches × 32 + 4,096 (13,696 at the default), so it never clips the scaled workload; the 300 fire emitters keep about 6,200 particles live. About 28 point lights reach the view; the extract keeps 16.
- **Text.** Three HUD lines plus `tags` name tags in `mono` on the walkers nearest the view center (name, hit points, x). All of them change every frame.
- **Post.** `game` is bloom (threshold 0.8, intensity 0.5), tonemap and vignette, plus SMAA High. `engine_config` applies `aa` and `bloom_mips` under the environment-first rule; `configure` sets the chain as a world resource.
- **Camera.** `CameraMode::Scripted`: `camera_script` writes the path's base position, ping-ponging at `camera_speed`, then `shake_tick` and `camera_update` add shake and clamp to the level. The path keeps 16 px from every edge, more than the 6 × 4 px shake.
- **Draw order.** The parallax strips have the lowest z, so the default extract emits their batches first, then `extract_tilemaps`' batches, then every other sprite. Tiles write `z_norm` 0, so the row needs the default `cpu_stable` depth sort.

The interaction costs it exposes, each visible through a counter or a stage row: tile proxies × dynamic bodies (the `physics:` line's `proxies`, which `tile_collision=off` isolates); collision event → burst → `CommandBuffer` spawn → flush cascades; batch fragmentation from lit, animated, material, parallax, particle and tile sprites interleaved by z; light culling with hundreds of off-screen lights; text, post and SMAA over the composed frame; several systems writing `Transform` in one frame.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `level_tiles` | 1,536 | 256–16,384 | | Level width in 16 px tiles; 96 tiles high |
| `actors` | 2,500 | 0–100,000 | yes | Walkers; every tenth is a caster |
| `crates` | 2,000 | 0–100,000 | yes | Dynamic crates in piles of 10; they may sleep |
| `props` | 6,000 | 0–200,000 | yes | Static lit and unlit decoration |
| `torches` | 300 | 0–10,000 | yes | A point light and a fire emitter each |
| `pickups` | 1,000 | 0–50,000 | yes | Bobbing through position tweens |
| `fire_rate` | 0.3 | 0–5 per s | | Per caster |
| `tags` | 64 | 0–512 | | Name tags rewritten every frame |
| `tile_collision` | `on` | `on`, `off` | | `off`: merged static boxes replace the per-tile proxies |
| `post` | `game` | `none`, `game`, `full` | | `full` adds god rays, fog, color adjust, chromatic aberration and film grain |
| `aa` | `smaa_high` | `off`, `smaa_high` | | `TUNGSTEN_RENDER_POST_AA` wins when set |
| `bloom_mips` | 6 | 1–8 | | `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` wins when set |
| `camera_speed` | 360 | 0–2,000 px/s | | Scripted camera; ping-pongs along the level |

- **Presets:** `min` (256 tiles, 50 actors, 50 crates, 100 props, 8 torches, 20 pickups, 4 tags) and `default`.
- **Counters,** in line order: `actors`, `projectiles` (live), `hits`, `particles` (live), `lights` (point lights reaching the view, before the extract keeps 16), `camera_x`, `view_out`, `flashing`, `landings`, `shots`, `turns` and `events` (collision events).
- **Guard:** `bench.view_out <= 0`. `view_out` is how far the final view, shake included, leaves the level, rounded up to whole pixels.
- **Owned:** `total` p50/p95/p99 and jitter (p99 − p50, judged with p99's threshold). No stage is owned. The declared bottleneck is `physics_step`, which calibration found limiting (6.66 of 12.03 ms, means) ahead of the default extract (2.60 ms). Key knobs `actors`, `crates`, `props`, `torches` and `pickups`.
- **Row note:** Judged on total frame time; `jitter` is p99 - p50 of `total`, judged with p99's threshold. No stage is owned: the declared bottleneck, physics_step, is the stage calibration found limiting (walkers and crates against the tile proxies), ahead of the default extract. Tiles draw at z_norm 0, so the row needs the default cpu_stable depth sort (under gpu_depth they cover every sprite). Without T1, particle and tween time lands in `unattributed`. Changing HUD and name-tag text grows RSS through the text buffer cache (360-frame TTL).

**Calibrated default:** `actors` 3,000 → 2,500; the other counts stay as designed. Target: a `total` p95 of 10–16 ms ✓.

| `total` p50 / p95 / p99 per run (ms) | Jitter per run (ms) |
| --- | --- |
| 11.97/12.37/12.52, 12.04/12.47/12.53, 12.40/12.90/13.29 | 0.55, 0.49, 0.89 |

The design defaults gave p95 13.71 ms. A single-run `actors` sweep, in ms:

| `actors` | `total` p50 / p95 / p99 | `physics_step` mean | `extract` mean | Turns per walker per s |
| --- | --- | --- | --- | --- |
| 1,500 | 9.61 / 9.99 / 10.42 | 4.72 | 2.41 | 1.4 |
| 2,000 | 10.96 / 11.36 / 11.64 | 5.75 | 2.56 | 1.8 |
| 2,500 | 12.01 / 12.38 / 12.53 | 6.57 | 2.63 | 2.1 |
| 3,000 | 13.35 / 13.91 / 14.77 | 7.76 | 2.72 | 2.4 |
| 3,500 | 14.63 / 15.10 / 15.64 | 8.71 | 2.85 | 2.6 |

- Each 500 walkers add about 1.3 ms of p95, nearly all in `physics_step`. 2,500 leaves 2.4 ms to the band's floor and 3.6 ms to its ceiling and cuts the jitter to about 0.5 ms, against 1.4 ms at 3,000.
- Stage means at the default: `update` 7.46 (`physics_step` 6.66, `animate_actors` 0.37, `collision_events` 0.16, `actor_ai` 0.11), `extract` 2.60, `render` 1.44 (`render_encode` 0.96, present wait 0.47), `unattributed` 0.37 and `flush` 0.17 ms. The frame is CPU-bound: the GPU diagnostic run puts `render_span` at 6.19 ms p50 (SMAA 2.56, scene 1.30, bloom 1.05 over all its passes, tonemap 0.34, present 0.32, vignette 0.26).
- Interaction counters, means per frame: 13,044 proxies (8,528 tile proxies plus 4,516 dynamic bodies), 951 sleeping crates, 6,677 pairs, 6,282 contacts and 23,110 collision events; 16 live projectiles, 1.26 shots, 1.23 hits and 17.5 flashing walkers; 6,203 live particles; 27.7 point lights in view (25–30); 23.9 landings and 88.2 turns.
- `tile_collision=off` spawns 328 merged static boxes (runs along rows, merged downward) in place of the 8,528 tile proxies: `physics_step` drops from 6.57 to 4.27 ms, p95 from 12.38 to 9.66 ms and events from 23,110 to 16,531, while turns stay at 88. The AI reads the level grid in both modes.
- At the 144 Hz budget capacity search finds the row present-bound (✗ against `physics_step`): scale moves the scene counts but not the level or the 1080p post chain, whose GPU span of about 6 ms sets a floor near 6.9 ms.
- Peak RSS is about 281 MiB and grows about 19.5 MiB/s from the name tags and HUD (see "Engine findings"), so it depends on capture length.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `9b2617e4c1ab23f7`.

## Engine findings

The suite exposes these engine costs and behaviors on purpose. Each is a finding for separate engine work, not something the suite fixes or works around silently.

- **Burst latch.** `EmissionKind::Burst` fires once whatever `once` says, because the `continuous_accum` latch in `particle_tick_system` never resets (`crates/tungsten/src/particles.rs`). `particles` re-arms bursts by clearing the latch; `integrated` spawns a fresh emitter per hit.
- **Tiles at `z_norm` 0.** `extract_tilemaps` writes the nearest depth, so under `TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth` the tilemap covers every sprite: the M25 `gpu_depth` smoke rows render only tiles and text, and `integrated` needs the default `cpu_stable`.
- **Lit-plus-material warning.** The default extract logs a warning once per sprite and frame when a lit sprite carries a material, so lit plus material costs a log call per sprite. `integrated` swaps a struck walker to an unlit clip to show its flash.
- **Text-cache RSS.** The text pipeline keeps every changed section's shaped buffer for 360 frames (`BUFFER_CACHE_TTL_FRAMES`, pruned every 120). With text changing every frame, RSS grows until the TTL saturates: about 139 MiB/s in `gpu` (peak near 731 MiB, against 171 MiB with 2 glyphs) and 19.5 MiB/s in `integrated`. Peak RSS on those rows depends on capture length.
- **Default extract.** It doesn't cull, sorts every sprite every frame and resolves string IDs: about 0.16 µs per sprite in `particles` and 0.06 µs in `gpu-throughput`'s one-texture field. It dominates the sprite-heavy rows.
- **Tile collision proxies** are rebuilt from a full-map scan every frame (`gather_tilemap_proxies`): 8,528 proxies in `integrated`, which cost about 2.3 ms of `physics_step` against merged boxes.
- **No bundle insert.** Spawning an entity with k components costs k boxed commands and k archetype moves.
- **String sprite IDs.** Each animation frame change clones a `String`, and the extract resolves IDs by string.

## Open proposals

Not approved; each benchmark works without them. Approving one is a separate decision.

| ID | Proposal | Would enable |
| --- | --- | --- |
| G1 | Kinematic bodies | Moving paddles and spinners in `physics`, moving platforms in `integrated` |
| G2 | Sensor colliders | Real bin sensors in `physics` instead of solid floors read through collision events |
| G3 | More than 16 lights | A `lights` sweep past 16 |
| G4 | GPU particle simulation | GPU-side particles |
| T1 | `particles_ms` and `tweens_ms` in `FrameTimings` and the `frame:` line | A timed particle stage for `particles` (instead of `unattributed`) and attribution in `integrated` |
| T2 | A `startup:` line with window, renderer, manifest, user startup and audio times | Load-time metrics in every capture and compare |
| M1 | A counting global allocator in `example-02-bench` only, behind a feature | Allocations per frame; it needs a `DECISIONS.md` entry, since its counters are process-global state |

**RSS growth threshold.** A5 defines none, so RSS growth is reported for every row, `churn` included, and judged for none. The open proposal judges it for `churn` only:

- a capture reads `leaking` when the median of its per-run slopes exceeds 32 KiB/s and every run exceeds 16 KiB/s;
- compare marks growth `regressed` when the candidate's median exceeds the baseline's by more than 32 KiB/s under the same all-runs rule;
- 32 KiB/s was about 5× the largest slope then observed (6.8 KiB/s, `churn` at 100,000) and 10× the one-page resolution of the 1.3 s fit a `churn` run allows; the all-runs rule keeps one 132 KiB heap extension inside a single run's fit from counting.

## Adding a benchmark

1. **Module.** `examples/02_bench/src/<bench>.rs` (split past about 600 lines into `src/<bench>/`), registered in `main.rs`, at `workload_version` 1. Seed every random choice from `seed`, and keep rendered text independent of wall-clock time.
2. **Knobs** in the benchmark's schema: type, default, range, whether it scales, a note. Mark the knobs that should follow `TUNGSTEN_BENCH_SCALE`.
3. **Presets:** `min` and `default` at least. Calibrate `default` so `total` p95 lands between 8 and 16 ms on the reference machine, or record why it doesn't.
4. **Counters** on the `bench:` line through `counters.rs`, including whatever proves the workload is what it claims.
5. **Row:** owned metrics with their statistics, guards (`physics_max`, `counter_min`, `counter_max`, `counter_const`, `counter_eq`, `counter_band`), `bottleneck`, `key_knobs` (at least one scaled for capacity search) and a `note` when a choice needs explaining. `describe` publishes all of it; the runner, suite and capacity search read it from there.
6. **Tests:** knob resolution in `src/tests/knobs.rs` covers every preset; add invariant tests only where the geometry or setup is easy to get wrong.
7. **Smoke rows** for `min` and `default` in the Benchmarks section of `scripts/smoke-examples.sh`, plus their expected runs in `scripts/test-smoke-examples.sh`.
8. **Captures:** a valid `just perf run <bench> --repeat 3` with matching digests, then `just perf capacity <bench>` at both budgets.
9. **Docs:** a section here, and its tracked row in [`profiling-workflow.md`](profiling-workflow.md#tracked-rows-suites-and-regression-policy). A change to an existing benchmark's work bumps its `workload_version` and adds a line to its history.
