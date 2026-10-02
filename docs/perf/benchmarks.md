# Benchmarks

`examples/02_bench/` (package `example-02-bench`) holds six scalable, deterministic benchmarks, each owning one bottleneck (`D-078`). One binary runs one benchmark per launch. This file describes what each benchmark measures, its knobs, guards and owned metrics, and the calibrated defaults on the reference machine. [`profiling-workflow.md`](profiling-workflow.md) covers configuration, captures, compare, capacity search and the capture rules.

```bash
cargo run -p example-02-bench                                       # physics at `default`, interactive
TUNGSTEN_BENCH=gpu TUNGSTEN_BENCH_PRESET=min cargo run -p example-02-bench
just perf describe gpu                                              # knobs, presets and tracked rows from the binary
```

**Reference machine.** Ryzen 5 6600H, Radeon 660M (Mesa RADV `REMBRANDT`), Vulkan, `immediate` present mode with latency 1, Linux 7.2.7, rustc 1.98.1, governor `performance`, generic x86-64 release builds with frame pointers (`just perf`'s default flags). Every number below comes from that machine and is informational (`D-070`). Numbers from the retired perf scenes aren't comparable with this suite.

**Dates.** The defaults were calibrated on 2026-09-30, and a number without a date is from that day. The physics and ECS performance pass of 2026-10-01 (`D-080`–`D-084`) then moved the `physics`, `physics-sparse`, `ecs`, `churn`, `particles` and `integrated` rows. A number marked 2026-10-01 is from that pass's final suite (5 runs a row, medians of the per-run values) or, for the `churn` mode table and the `integrated` `actors` sweep and `tile_collision` numbers, from knob captures of the same build that day (3 runs each). `just perf compare` works on per-run means, so the decisions and the changelog quote values up to 0.03 ms off the medians here. That pass did not move `gpu` or `gpu-throughput`. Session A of the render-path pass followed the same day (`D-085`) and moved `gpu` and `integrated`: a number marked "after `D-085`" is from that session's final suite, taken the same way. The extract rewrite (`D-086`) landed later that day and moved `gpu-throughput`, `particles`, `integrated` and the extract of `gpu`: a number marked "after `D-086`" is from the suite taken on that tree. The direct present path (`D-087`) followed and moved the GPU passes of `gpu` and `integrated`: "after `D-087`" marks that tree's suite. Session B of that pass closed on 2026-10-02 and moved no row: it added the `interval` field to the `frame:` line and took the pass's last captures, and a number marked 2026-10-02 is from its final suite (5 runs a row) or from the captures taken beside it.

**FPS.** An FPS in this file is 1000 / mean `total`. Since 2026-10-02 the `frame:` line also carries `interval`, the time between two frame starts ([`profiling-workflow.md`](profiling-workflow.md#telemetry-lines)), which is the frame's real period. Its mean is 0.05–0.13 ms above the mean `total` in the eight rows, so a row runs 0.5% (`gpu`: 89.7 FPS against 90.1) to 1.5% (`integrated`: 117.8 against 119.6) below the figure quoted here, and the empty `gpu` frame runs at 1,990 FPS where `total` gives 2,114.

For a specific workload, open its section: [`physics`](#physics), [`ecs`](#ecs), [`churn`](#churn), [`gpu`](#gpu), [`particles`](#particles), [`integrated`](#integrated). [Ownership](#ownership) and [open proposals](#open-proposals) apply across rows. Dated measurements below remain historical evidence; use a fresh comparable capture to assess the current tree.

## Harness

- **One binary.** `main.rs` resolves the configuration (`knobs.rs`: preset, then scale, then overrides, then validation) and calls the benchmark's `configure`. Without `TUNGSTEN_BENCH` it runs `physics` at `default`. `TUNGSTEN_BENCH_DESCRIBE=1` prints every benchmark's schema as JSON and `=config` the resolved configuration, both before a window opens. `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` enables overlays for interactive use.
- **Determinism.** Every random choice comes from `Pcg32::seeded(seed)`, and captures run under `TUNGSTEN_SMOKE_FRAMES`, which pins `dt` to 1/60 s, so one build replays an identical workload. No rendered text depends on wall-clock time.
- **Assets.** `gen.rs` generates every texture (discs, soft dots, sphere and bevel normal maps, emissive masks, tiles, parallax strips, animation frames) under `bench_` IDs, so the suite ships no image files. Its only asset files are `examples/02_bench/assets/manifest.json` and the `bench_heavy` shader and material it lists. `gpu` loads that manifest after the root manifest; `integrated` loads the root manifest alone, for its fonts and the `damage_flash` material; the others load none. Manifest composition replaces the tilemap, animation and particle registries, so benchmarks register those in their startup hook.
- **Lean view.** `view.rs` is a culled, unsorted extract over `Position` plus a small `ViewSprite`. `physics`, `ecs` and `churn` draw through it, so their render cost stays flat as scale grows. `gpu`, `particles` and `integrated` use the engine extracts, whose cost they own or report.
- **Telemetry.** `counters.rs` logs `bench-config:` once (the resolved config as JSON, plus derived values) and `bench:` counters every frame, on log target `bench`. Frame N + 1's first system logs frame N's counters, so each `bench:` line follows its frame's `frame:`, `systems:`, `gpu_passes:` and `physics:` group; the last frame has none.
- **Engine hook.** `Bench::engine_config` runs before the window opens. `gpu` and `integrated` apply `aa` and `bloom_mips` there, except where `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_POST_AA` or `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` is set: the environment wins and a warning names the overridden knob. The runner clears those variables, so captures follow the knobs.
- **Workload version.** Each benchmark has a `workload_version`, logged in `bench-config` and compared as a hard field. A bench code change that alters the work bumps it; engine changes never do. Every benchmark is at version 1. Each section lists its rows' determinism digests at the default on the reference machine: a harness change that moves one changed the work, while an engine change may move them without a version bump.
- **Tests.** `src/tests/` covers knob resolution and validation and the pachinko geometry invariants; `tests/visual_regression.rs` holds the `gpu` visual test and the test that the direct and the capture present paths draw the same image (`D-087`), both behind `TUNGSTEN_VISUAL_REGRESSION`.

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

**Calibrated defaults** (2026-09-30, medians of 3 runs):

| Row | Default change | World | `physics_step` p50 / p95 | `update` p95 | `total` p95 | Calibration check |
| --- | --- | --- | --- | --- | --- | --- |
| `physics` | `fill` 0.21 → 0.36; `balls` stays 8,000 | 1,024 × 1,824 px, 8 bins | 13.37 / 13.99 ms | 14.06 ms | 14.58 ms | 1.11 contacts per ball (≥ 1.0 ✓) |
| `physics-sparse` | `bodies` 50,000 → 8,000 | 4,224 × 2,368 px | 8.06 / 9.83 ms | 9.86 ms | 10.11 ms | 0.229 pairs and 0.118 contacts per proxy (≤ 0.5 and ≤ 0.2 ✓) |

- Pachinko at `fill` 0.21 gave 0.64 contacts per ball; 0.25, 0.30, 0.35 and 0.40 gave 0.74, 0.89, 0.99 and 1.11. Whole bins mean 1.0 needs 8 bins, so `fill` ≥ 0.3542. Stall kicks stay near 0.01 per frame and respawns at 0.
- Sparse at 50,000 bodies gave a `total` p95 of 78 ms; 6,000, 8,000, 10,000 and 12,000 bodies gave 7.8, 10.6, 13.9 and 15.7 ms. A frame-pointer profile at 8,000 split `physics_step` into pair query (`build_pairs`) 54.5%, `speculative_pass` 29.9% (22% of it `SpatialGrid::query`), grid build 6.2%, narrow phase 1.4% and solver 0.7%: broadphase-bound, as intended.
- Peak RSS was about 132 MiB (pachinko) and 128 MiB (sparse). Pachinko RSS grew 2–8 KiB/s and sparse RSS not at all, reported only.

**The same defaults on 2026-10-01**, after `D-080`–`D-082` (the final suite, medians of 5 runs):

| Row | `physics_step` p50 / p95 | `update` p95 | `total` p95 | `physics.pairs` (before) | Contacts | Peak RSS |
| --- | --- | --- | --- | --- | --- | --- |
| `physics` | 5.56 / 5.71 ms | 5.76 ms | 6.07 ms | 36,144 (15,150) | 8,846, 1.11 per ball | 131 MiB |
| `physics-sparse` | 3.42 / 3.47 ms | 3.51 ms | 3.73 ms | 15,298 (1,831) | 940, 0.117 per proxy | 128 MiB |

- No default changed and `workload_version` stays 1, so both rows now run below the 8–16 ms band they were calibrated to; see "Below the calibration band" under "Open proposals".
- The contact checks still hold. The pair check of `physics-sparse` no longer describes the list: `D-081` builds the pair list once for the whole frame's travel plus a margin instead of rebuilding it for the remainder at every substep, so the count logged at the last substep rose to 1.91 pairs per proxy with the same contacts. Compare flags `physics.pairs` as workload drift against captures from before `D-081`.
- What leads each row now (inclusive shares of all samples in a frame-pointer profile):
  - `physics` (`physics_step` 92.4%): the one pair build per frame 37.4% (its grid query 35.1%), narrow phase 13.0%, `solve_contacts` 8.8%, `repair_pairs` 7.4%, grid build and insert 4.2%, `apply_restitution` 2.6%, `ImpulseMap::get` 2.3%, `sleep_frame_end` 1.7%. A frame is one build plus 3.0 repairs.
  - `physics-sparse` (`physics_step` 89.4%): the pair build 38.1% (query 34.2%), the safety-net sweep 30.7% (`SpatialGrid::query` 23.7%, `cell_range` 9.8%: four long walls do not fill compact bounds, so the statics-only grid uses the hashed table), grid build and insert 7.0%, `repair_pairs` 4.4%, narrow phase 3.9%, solver 1.7%, `collect_tripped` 1.2%. Still broadphase-bound; a frame is one build plus 2.1 repairs.
- RSS growth, reported only, reads 0 KiB/s in all five pachinko runs. A sparse run is now about 1.5 s long, so one 1.4 MiB step of RSS inside the fitted half read as 1.2 and 2.6 MiB/s in two of the five runs; the other three read 0–6 KiB/s.

**Workload version history:** 1, the initial version (2026-09-30). Digests at the default: `physics` `c10885dddd6115a4`, `physics-sparse` `9df5ade5305646dc`. They changed on 2026-10-02 with `D-092`, the arrival pass, which clamps bodies in both worlds; before it they were `86ffcabcdb15eed1` and `5899f9c8a69d79b1`. Those dated from 2026-10-01 and `D-081`, an engine change that reorders the pair list and so the solver; before it they were `c972b2818d486617` and `c95fadc938a3689e`. `D-092` also costs the rows `physics_step` time, p50 5.61 → 6.08 ms in `physics` and 3.45 → 3.63 ms in `physics-sparse` (five runs a side); the dated numbers above are from before it.

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

**Calibrated default** (2026-09-30, 3 runs): `entities` stays 250,000. `total` p95 per run 12.29, 12.61 and 12.23 ms; `update` p50 10.84 / p95 11.84 ms; `update` is 95.7% of `total` (≥ 70% ✓). Per-system p50 medians: `heading` 2.59, `brain` 2.22, `follow` 1.90, `buffs` 0.66, `age_phase` 0.48, `cooldowns` 0.45, `tint` 0.41, `integrate` 0.27, `accelerate` 0.26, `bounds_wrap` 0.26, `team_bags` 0.25, `faction_histogram` 0.25, `stats_decay` 0.17 and `regen` 0.09 ms. Peak RSS is about 163 MiB, with no RSS growth.

**The same default on 2026-10-01**, after `D-083` (the final suite, medians of 5 runs): `total` p95 11.35 ms (per run 11.32–11.40); `update` p50 10.44 / p95 10.86 ms, 95.7% of `total`. Per-system p50: `heading` 2.53, `brain` 2.25, `follow` 1.77, `buffs` 0.65, `age_phase` 0.46, `cooldowns` 0.44, `tint` 0.39, `bounds_wrap` 0.26, `accelerate` 0.25, `team_bags` 0.25, `faction_histogram` 0.25, `integrate` 0.23, `stats_decay` 0.16 and `regen` 0.08 ms. Peak RSS and RSS growth are unchanged.

- **Accepted reading (`D-083`).** Against the baseline of the pass, `follow` p50 reads `improved` (1.95 → 1.74 ms, per-run means) and `update` p50 reads `noisy`: 10.74 → 10.46 ms, −0.28 with an interval of −0.37 to −0.20 against a threshold of 0.32. The gain is real and sits just under the 3% threshold, where compare can say neither `improved` nor `unchanged`; three clean captures read −0.26, −0.30 and −0.28. The owner accepted the reading on 2026-10-01. Ask a later check on this metric for "not `regressed`".
- `follow` is one `World::get` per follower behind two cache misses (the entity's metadata, then the leader's `Position`). An iteration is about 160 instructions, too long for a third lookup to overlap, so what the engine can change is the loads between the two misses (`D-083`'s slot table); its per-run p50 still spreads from 1.60 to 1.82 ms.
- **A per-run mode.** In some runs `stats_decay` reads 0.31–0.32 ms instead of 0.15–0.16, `regen` 0.02 ms more and `follow` about 0.2 ms less, with the same digest. It showed in 5 of the 40 runs of eight `ecs` captures on 2026-10-01, the untouched tree's included (two of the final suite's five runs: `stats_decay` 0.31, 0.32, 0.15, 0.15, 0.16 against `follow` 1.69, 1.60, 1.82, 1.81, 1.77), so it belongs to the run, not to a build. Its cause was not looked for. It widens compare's intervals on those three rows. `buffs` moves with it too, by 0.02–0.03 ms (0.65–0.67 against 0.62–0.66), and the mode's share is not steady: later that day it showed in 4 of 5 runs of two suites and in 16 of 30 runs of a 15-run pair. A suite that catches it in four runs against a baseline that caught it in one reads `buffs` `regressed` at 0.64 → 0.66 ms, a delta equal to the 0.02 ms floor; the 15-run pair reads it `unchanged` (`D-086`). On 2026-10-02 two suites of one tree in one sitting caught it in four and in two of five runs, and the first, against a suite of the same tree from 73 minutes earlier that held no such run, read `stats_decay` (0.15 → 0.28 ms) and `buffs` `regressed` and `follow` (1.82 → 1.63 ms) and `team_bags` `improved`. Three more rows move with the mode, each downward: `team_bags` (0.22 ms against 0.24–0.25), `accelerate` (0.23 against 0.25) and `integrate` (0.22 against 0.25). So the mode shifts seven of the 14 system rows, and a five-run suite can read any of them `regressed` or `improved` with nothing changed; `update` itself stayed `unchanged` (a mode run reads about 0.15 ms less).
- `brain` read 2.17 to 2.26 ms across the builds of the pass with no change to its code path: the small system rows move by 0.02–0.05 ms with how each row loop compiles around the inlined query setup, so judge a query change on all 14 rows.
- **Code placement moves these rows past the threshold.** Where the linker puts a system's unchanged instructions decides which of two values it reads: `brain` 2.17 or 2.26 ms and `bounds_wrap` 0.26 or 0.31–0.32 ms, by the start address modulo 64. On 2026-10-01 (`D-085`) changes in the render and app crates read `regressed` on each, and the tree without the change plus one function that no frame calls, sized to give the same alignment, reproduced both verdicts (15 runs a side). Before taking a `regressed` on a system row as the change's own, compare the addresses (`nm -C target/release/example-02-bench | rg 'systems::brain$'`) and, if they moved, A/B against such a padded build. `churn`'s `flush` moves the same way. `D-087` is the next instance: its build starts the text section 16 bytes earlier, `bounds_wrap` lands 32 bytes past a boundary and reads 0.27 → 0.31 ms, and a build with the same layout and the old frame reads the same.
- In the final profile (frame pointers, shares of all samples, `update` 87.2%) `heading` is 19.7% (`atan2` 15.0%) and `brain` 18.5%, both benchmark work the engine can't touch; `follow` is 16.5%, of which the lookup itself is 4.9% and the rest waits on the two loads; the query-setup scan (`Archetype::column_index`) is 1.1%.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `c785506ced04de4c`.

## `churn`

Owns structural change: command recording, `World::flush`, archetype moves, and entity allocation and free. Warm-up 60 frames, no GPU diagnostic run. Module: `churn.rs`.

- **Population.** A steady population is replaced FIFO: every entity lives `1/turnover` frames (20 at the default), and the population is pre-aged, so churn is steady from the first frame. Frame f despawns spawn serials `[f·S, (f+1)·S)` and spawns `[P + f·S, P + (f+1)·S)`. Each spawn inserts `components` components one at a time. In `immediate` mode that is one archetype move each; in `deferred` mode the flush has applied a spawn's inserts as one move since `D-084`. The binary's schema note still says "one archetype move each"; that describes `immediate` only.
- **Status toggles.** Survivors whose serial is f modulo `groups` gain one of `statuses` status components at frame f and lose it at f + 1. Gains skip entities that expire at f + 1, so each frame's losses equal the previous frame's gains. `groups = round(2·(P − 2S) / (toggles·P))`, 36 at the default.
- **Modes.** `deferred` records everything through the `CommandBuffer` and measures it in `flush`; `immediate` makes the same calls on `World` inside the churn systems. Both find their targets with one scan over the lifetime component, and the systems run as `churn_scan`, `churn_despawn`, `churn_toggle`, `churn_spawn`.

| Knob | Default | Range | Scales | Notes |
| --- | --- | --- | --- | --- |
| `population` | 125,000 | 1,000–2,000,000 | yes | |
| `turnover` | 0.05 | 0–0.5 | | Share replaced per frame, FIFO |
| `toggles` | 0.05 | 0–0.5 | | Share gaining or losing a status per frame; a status lasts one frame |
| `components` | 6 | 2–12 | | Components per spawn; deferred inserts move once per spawn, direct inserts move once each |
| `statuses` | 3 | 1–8 | | Distinct status components |
| `mode` | `deferred` | `deferred`, `immediate` | | |
| `view` | 0 | 0–16,384 | | Rendered sample |

- **Presets:** `min` (population 1,000) and `default`.
- **Counters:** `population`, `spawned`, `despawned`, `inserted`, `removed` and `commands`. At the default each frame spawns and despawns 6,250 entities and makes 3,125 gains and 3,125 losses: 40,625 inserts, 3,125 removals and 56,250 commands.
- **Guards:** `bench.population constant` and `bench.spawned == bench.despawned`.
- **Owned:** `flush` p50/p95 plus `churn_scan`, `churn_despawn`, `churn_toggle` and `churn_spawn` at p50. The bottleneck is `flush`; key knob `population`.

**Calibrated default** (2026-09-30, 3 runs): `population` 100,000 → 125,000. `total` p95 per run 11.37, 11.28 and 10.95 ms; `flush` p50 8.74 / p95 10.01 ms; `flush` plus the churn systems are 96.8% of `total` (≥ 60% ✓). At 100,000 the p95 sat within 1 ms of the band floor; a sweep gave 9.01, 10.99, 14.37 and 16.81 ms at 100,000, 125,000, 150,000 and 175,000. Peak RSS is about 143 MiB.

**The same default on 2026-10-01**, after `D-083` and `D-084` (the final suite, medians of 5 runs): `total` p95 4.06 ms (per run 4.03–4.13); `flush` p50 2.72 / p95 3.07 ms; `churn_scan` 0.38, `churn_spawn` 0.33, `churn_toggle` 0.05 and `churn_despawn` 0.01 ms at p50; `flush` plus the churn systems are 94.1% of `total` (means). Peak RSS is about 142 MiB. No default changed, so the row runs below the 8–16 ms band; see "Below the calibration band" under "Open proposals". The population sweep above was not repeated.

- A frame makes 208 `malloc` calls, where it made 293,960 before the pass (exact counts), and the flush's profile shows none.
- `flush` moves with code placement, like the `ecs` system rows: one build of 2026-10-01 read 2.53 ms p50 and three others 2.63–2.68 ms in the same sitting, with `World::flush_reusing` the same size in each and at another address. One of the three was the 2.53 ms tree plus a function that no frame calls, and it read `regressed` against it (`D-085`). With `D-087` the row reads 2.51 ms p50 and 2.78 ms p95 against 2.68 and 3.02: `World::flush_reusing` moved from 48 to 16 bytes past a boundary, which a build with that layout and the old frame reproduces (2.71 → 2.54 ms, 15 runs a side), and the direct present path itself adds 2.54 → 2.45 ms. The `interval` field of 2026-10-02 lengthened `App::window_event` and put `World::flush_reusing` on a 64-byte boundary: the row reads 2.50 ms p50 against 2.45 (`noisy`, +0.05 against a threshold of 0.074) and 2.79 ms p95 against 2.73 (`unchanged`), five runs a side.
- What leads the row now (frame-pointer profile, shares of all samples): `flush_reusing` 68.9%, of which `insert_run` is 39.8% (the toggles' row moves 24.4%, waiting on the moved row in the first column; the value writes 6.1%; the edge walk 5.9%), the status losses' `Archetypes::remove` 7.4%, and the despawns' `swap_remove_drop` on the first column 6.7% plus `World::despawn` 4.3%. The benchmark's own `churn_scan` and `churn_spawn` are 10.5% and 8.0%.

Command-buffer overhead (2026-10-01, 3 runs a mode), means in ms from the median run of each mode:

| Mode | `flush` | `churn_scan` | `churn_despawn` | `churn_toggle` | `churn_spawn` | Structural total | `total` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `deferred` | 2.75 | 0.38 | 0.01 | 0.04 | 0.33 | 3.52 | 3.74 |
| `immediate` | 0 | 0.34 | 0.63 | 1.09 | 1.22 | 3.29 | 3.51 |

- The `CommandBuffer` costs 0.23 ms per frame, 7% over the direct calls or about 4 ns per command. Before the pass it cost 2.52 ms, 36% or about 45 ns per command (structural totals of 9.45 and 6.93 ms): a boxed command and setter per operation, plus replay. The two modes no longer do the same moves: a deferred spawn moves its row once, an immediate one six times. Peak RSS is about 1 MiB higher deferred (4.5 MiB before).
- Toggle moves are memory-latency bound. Spawns and despawns alone (`toggles=0`) cost 0.89 ms of `flush` for 50,000 commands, about 0.07 µs for each of the 6,250 spawn moves and 6,250 despawns; the 6,250 toggle moves add 1.86 ms, about 0.30 µs each, because toggled rows are scattered through a large archetype. Before the pass those were 4.18 ms, and 4.36 ms or 0.70 µs per toggle move.
- Swap-remove scrambles the archetype layout over time, so structural cost depends on access locality as much as on count: a change to which entities a system touches can move `flush` severalfold at the same command count.
- RSS growth, reported only, reads 0.0 KiB/s in every `immediate` run and in two of the three `deferred` runs; in the third, one 1.3 MiB step of RSS inside the fitted half of a 1.4 s run read as 1.4 MiB/s.

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
  - `gpu`: `gpu` (the scene pass) and `render_span` at p50/p95; the passes `post0_bloom_threshold`, `post0_bloom_composite`, `post1_vignette`, `smaa_edges`, `smaa_blend_weights`, `smaa_neighborhood` and `text` at p50; `extract` and `render_encode` at p50/p95. The bloom mip passes are reported, not owned. Until `D-087` the row also owned the `present` pass, which a measured frame no longer has. Bottleneck `present`, the present wait (`render_acquire` plus `render_submit_present`): a GPU-bound frame waits in the acquire, 7.12 of `total`'s 11.04 ms at p50 after `D-085` and 7.53 of 10.72 ms after `D-087`, against 0.31–0.33 ms in submit and present. Key knob `sprites`.
  - `gpu-throughput`: `extract` and `render_encode` at p50/p95. Bottleneck `extract`; key knob `sprites`.
- **Row note (`gpu`):** `gpu` (the scene pass), `render_span` and the pass rows come from the GPU diagnostic run; without timestamp queries they are n/a and the capture stays valid.

**Calibrated defaults** (medians of 3 runs):

| Row | Default change | `total` p50 / p95 / p99 per run (ms) | Owned (ms) | Calibration check |
| --- | --- | --- | --- | --- |
| `gpu` | `sprites` 120,000 → 2,000; `post` `full` → `light` | 11.09/12.55/14.08, 11.10/12.77/13.95, 11.12/12.65/14.45 | scene pass p50 7.82 / p95 8.30; `render_span` 11.48 / 11.95; `extract` 1.95 / 2.28; `render_encode` 2.68 / 3.07 | GPU span 11.19–11.46 ms > `update` + `extract` + `render_encode` 4.74–4.79 ms (means) ✓ |
| `gpu-throughput` | none; not calibrated to the band | 25.62/26.22/27.20, 25.63/26.18/26.52, 25.57/26.38/26.58 | `extract` p50 23.99 / p95 24.45; `render_encode` 1.31 / 1.51 | — |

- The design defaults gave a `total` p95 of 161 ms (scene pass 152 ms, about 1.2 µs per sprite). With no sprites, the full post chain (5.7 ms of GPU time, 2.6 of it god rays), SMAA and the fixed layers still gave 14.7 ms. SMAA cost depends on content: `smaa_blend_weights` rose from 0.23 to 2.56 ms on bare tiles. With `post=light`, 0, 2,000, 4,000 and 8,000 sprites gave p95 10.38, 12.81, 15.47 and 20.37 ms.
- `gpu-throughput` spends about 60 ns per sprite in `extract`, mostly the sort.
- At calibration the scene pass was stable within a capture (runs within 2%) but moved about 12% between captures of one configuration, so compare could read the GPU rows `noisy`. See the scene-pass note below.
- At calibration peak RSS was about 731 MiB for `gpu` (the text buffer cache of that day; see "Engine findings") and 251 MiB for `gpu-throughput`.

**The same defaults after `D-085`** (2026-10-01, medians of 5 runs):

| Row | `total` p50 / p95 / p99 per run (ms) | Owned (ms) | Peak RSS |
| --- | --- | --- | --- |
| `gpu` | 11.04/12.37/12.95, 11.03/12.47/13.35, 11.02/12.43/12.79, 11.12/12.43/13.42, 11.04/12.49/13.22 | scene pass p50 6.90 / p95 8.43; `render_span` 10.56 / 12.09; `extract` 1.28 / 1.56; `render_encode` 2.19 / 2.53 | 169 MiB |
| `gpu-throughput` | 25.76/26.32/27.15, 25.90/26.51/27.13, 25.94/26.57/26.79, 25.86/26.40/26.84, 25.94/26.51/26.83 | `extract` p50 24.30 / p95 24.83; `render_encode` 1.29 / 1.44 | 240 MiB |

- **`gpu`.** Against that session's own baseline (5 runs a side, per-run means) `render_encode` reads `improved` at p50 2.72 → 2.18 ms and p95 3.15 → 2.53 ms, and `extract` at p50 2.00 → 1.27 ms and p95 2.35 → 1.54 ms. The extract's code did not change: the old text cache took fresh pages every frame, so the extract's vectors faulted on new memory (387 faults per frame, none now). The row waits on the GPU, so `total` p50 stayed at 11.04 ms and 88 FPS: `render_acquire` took up what the CPU stages gave back (p50 5.83 → 7.12 ms). The tail moved: p99 14.22 → 13.22 ms, jitter 3.22 → 2.18 ms, the largest frame 23.34 → 14.41 ms, and no frame above 1.5 × p50 in 300 or 900 frames, where the old cache gave 2 and 8 (a 41–43 ms frame every 120 frames once it was full).
- **Peak RSS** no longer follows capture length: 168.9 MiB at 300 frames and 168.7 MiB at 900, against 720.9 and 904.7 MiB, with no RSS growth.
- **Per frame** the `gpu` row creates no bind group and 5 buffers (31 and 38 before) and makes 1,928 `malloc` calls (4,709 before); exact counts on the capture binary.
- **Scene-pass note.** The two modes of the scene pass belonged to the old text path, and to the GPU diagnostic run only. On the builds before `D-085` the slower one (7.5–7.9 ms p50) never showed with `text_change` at 0.1 or below or with 400 glyphs or fewer (12 runs), showed in 9 of the 15 diagnostic runs of three suites at the default and in every run at 16,000 glyphs. With the new text path all 30 diagnostic runs of six suites read 6.87–6.92 ms, which is why the scene pass and `render_span` read `improved` at p50 against that session's baseline (7.49 → 6.90 and 11.13 → 10.56 ms). The timing runs never sat in the slower mode: their mean `total` is 11.39 ms before and 11.36 ms after, so no frame rate comes with it. How the text path slowed the diagnostic run's GPU passes was not established.
- **`gpu-throughput`** did not move: `extract` p50 24.00 → 24.27 ms and p95 24.47 → 24.80 ms read `unchanged`, `render_encode` `noisy` (p50 1.45 → 1.28 ms). Its extract is the "Default extract" finding below.

**After `D-086`** (2026-10-01, medians of 5 runs):

| Row | `total` p50 / p95 / p99 per run (ms) | Owned (ms) | Peak RSS |
| --- | --- | --- | --- |
| `gpu` | 11.00/12.42/13.38, 11.00/12.41/12.86, 11.04/12.50/13.18, 11.22/12.57/13.58, 10.98/12.40/13.24 | scene pass p50 6.87 / p95 8.38; `render_span` 10.52 / 12.04; `extract` 0.66 / 0.90; `render_encode` 2.16 / 2.42 | 170 MiB |
| `gpu-throughput` | 14.49/15.38/16.51, 15.03/15.70/15.89, 14.51/15.04/15.26, 14.44/14.87/15.20, 14.42/14.91/15.52 | `extract` p50 13.08 / p95 13.42; `render_encode` 1.14 / 1.53 | 226 MiB |

- **`gpu-throughput`.** Against the tree without the rewrite (5 runs a side, per-run means) `extract` reads `improved` at p50 24.27 → 13.06 ms and p95 24.80 → 13.47 ms: about 33 ns per sprite against 60, and 68.7 FPS against 38.6. A frame takes 0.17 ms of kernel time and 2 page faults where it took 8.81 ms and 1,610, and makes no `brk` call where it made 5 (exact counts). Peak RSS falls by 14.5 MiB. `render_encode` reads `noisy`: its p95 takes one of two values per run on one binary, about 1.45 or 2.7–2.9 ms, and the tree without the rewrite read `regressed` at p50 against its own suite of two hours earlier (1.28 → 1.44 ms), with three of its five runs at the higher one.
- **`gpu`.** `extract` reads `improved` at p50 1.27 → 0.66 ms and p95 1.54 → 0.87 ms. `total` p50 stays at 11.05 ms and 88 FPS: `render_acquire` took up the difference (p50 7.13 → 7.74 ms). With four times the tiles in view (`zoom=0.5`) `extract` p50 is 1.18 ms against 2.67.
- **The scene pass takes one of two values per frame in the diagnostic run.** At the default a frame's scene pass reads 6.8–7.0 or 7.9–8.6 ms, and at `zoom=0.5` about 3.1 or 4.2 ms. Which frames take the higher value follows the timing of the diagnostic frame (CPU work, then a blocking readback), not the scene, so a change that shortens a CPU stage moves the share. `D-086` took 1.5 ms out of the extract at `zoom=0.5`: the share of slow frames went from about 40% to about 70%, and the scene pass p50 read `regressed` at 3.16 → 4.06 ms (`render_span` 9.02 → 9.89 ms) with p95 `unchanged`. Without timestamp queries both builds ran at 96.4 FPS with 9.45 ms of GPU engine time per frame, and the timing runs' `total` stayed at 9.28 ms. At the default the same change took the share from about 40% to about 14%, and p50 stayed. The two values remain with desktop compositing suspended, and their cause is not established. Read a scene-pass or `render_span` verdict together with the per-frame values (`gpu_passes:` in `run-N/gpu.log`): equal values with a different share are the diagnostic run's timing, and no frame rate comes with them. The two modes of the scene-pass note above were such shares.

**After `D-087`** (2026-10-01, medians of 5 runs):

| Row | `total` p50 / p95 / p99 per run (ms) | Owned (ms) | Peak RSS |
| --- | --- | --- | --- |
| `gpu` | 10.71/12.19/12.93, 10.71/12.27/12.71, 10.72/12.23/12.61, 10.72/12.18/13.07, 10.75/12.18/12.74 | scene pass p50 6.89 / p95 8.64; `render_span` 10.39 / 12.16; `smaa_neighborhood` 0.79; `text` 0.06; `extract` 0.65 / 0.85; `render_encode` 2.11 / 2.38 | 169 MiB |
| `gpu-throughput` | 14.99/15.55/15.69, 15.72/16.73/16.84, 14.63/15.08/15.75, 14.74/15.20/15.58, 14.74/15.22/15.63 | `extract` p50 13.28 / p95 13.63; `render_encode` 1.13 / 1.51 | 227 MiB |

- **`gpu`.** The last full-screen stage, here SMAA's neighborhood pass, renders into the swapchain, and there is no present blit. The row's timing runs read `total` p50 11.05 → 10.72 ms against the tree before (5 runs a side, per-run means; `noisy`, the delta of 0.33 ms sits on the threshold of 0.331) and 90.2 FPS against 88.3; `render_acquire` p50 7.74 → 7.52 ms. GPU engine time per frame without timestamp queries is 10.60 ms against 10.93. A frame records 18 render passes and wgpu adds none, against 19 and 1.
- **Accepted regression (`D-087`).** `smaa_neighborhood` p50 reads `regressed`: 0.61 → 0.79 ms, +0.18 with an interval of +0.17 to +0.19 against a threshold of 0.02. The pass now writes the swapchain image (0.04 ms more than `PresentSource`) and carries the surface clear (0.14 ms), which wgpu's own pass spent outside every pass metric. `text` reads 0.04 → 0.06 ms (`noisy`: the delta equals the threshold) and the `present` pass, 0.32 ms, is gone, so the three sum to 0.97 → 0.85 ms and `render_span` p50 reads 10.52 → 10.39 ms (`unchanged`). The owner accepted the regression on 2026-10-01.
- **`gpu-throughput`** has neither post nor SMAA, so its scene pass writes the swapchain. Its owned stages read `unchanged` and `noisy`: `extract` p50 13.06 → 13.30 ms, +0.24 against a threshold of 0.39, and the row runs at 67.7 FPS against 68.7.
- **Per stage that writes the swapchain** (the empty frame with one thing added, tree before → after in one sitting, medians of 3 runs):

  | Frame | `total` p50 | FPS | `render_span` p50 | The writing pass |
  | --- | --- | --- | --- | --- |
  | Empty (`sprites=0,glow=0,lights=0,tile_layers=0,glyphs=0,post=none,aa=off`) | 0.82 → 0.43 ms | 1,101 → 2,074 | 0.41 → 0.15 ms | scene 0.06 → 0.13 ms |
  | Empty, `aa=msaa4` | 1.25 → 0.85 ms | 733 → 1,070 | 0.81 → 0.57 ms | scene (resolve) 0.48 → 0.56 ms |
  | Empty, `post=light` | 2.15 → 1.90 ms | 421 → 480 | 1.76 → 1.60 ms | vignette 0.25 → 0.45 ms |
  | Empty, `aa=smaa_high` | 2.04 → 1.78 ms | 448 → 516 | 1.64 → 1.51 ms | neighborhood 0.60 → 0.79 ms |

  The frame time falls by more than the span: the span gains the clear that wgpu's pass kept outside it. A pass that already cleared its offscreen target pays about 0.07 ms more for clearing the swapchain; a full-screen pass pays about 0.19 ms more.

**On 2026-10-02** (Session B's final suite, medians of 5 runs; neither row moved against the session's baseline):

- **`gpu`.** `total` p50 / p95 / p99 10.75 / 12.23 / 12.81 ms, 90.1 FPS and 89.7 by `interval`; scene pass p50 6.88 / p95 8.64 ms; `render_span` 10.39 / 12.16 ms; `extract` 0.65 / 0.89 ms; `render_encode` 2.12 / 2.40 ms; peak RSS 169 MiB. No frame above 1.5 × p50 in 300 or 900 frames, and the largest frame of the five runs is 13.99 ms. Capacity at 60 Hz is scale 1.61 (3,221 sprites, present-limited ✓); 144 Hz is out of reach (p95 8.36 ms with one sprite; 8.69 after `D-085`).
- **`gpu-throughput`.** `total` 14.68 / 15.17 / 15.61 ms, 67.9 FPS and 67.5 by `interval`; `extract` p50 13.28 / p95 13.64 ms; `render_encode` 1.19 / 1.50 ms, with one run of the five at its other value (p50 2.14 ms, p95 2.44). A frame takes 0.23 ms of kernel time and no page fault that grows with the frame count. Capacity is scale 1.04 at 60 Hz (417,708 sprites, `extract`-limited ✓; 0.57 before `D-086`) and 0.42 at 144 Hz (168,179 sprites; 0.31 before).
- **`gpu` at `min`** (hand-run, 3 runs): `total` p50 / p95 / p99 3.05 / 4.72 / 4.79 ms, 297.5 FPS and 293.9 by `interval`, against 3.35 / 5.07 / 5.23 ms and 269.1 FPS before `D-086` and `D-087`. The frames near 4.7 ms come once per display refresh and are the compositor's; they now sit above 1.5 × p50, so the row counts 61 spikes per 300 frames where it counted 24.

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

**Calibrated default** (2026-09-30, 3 runs): `emitters` 400 → 280 and `animated` 40,000 → 8,000. `total` p50/p95/p99 per run 11.02/11.64/12.18, 10.98/11.56/12.18 and 10.89/11.54/11.88 ms; `unattributed` p50 2.38 / p95 2.98 ms; `animate_sprites` p50 0.27 / p95 0.32 ms; `live` median 42,288 (largest deviation 1.9%). Peak RSS is about 142 MiB; RSS grows 4–13 KiB/s, reported only.

**The same default on 2026-10-01**, after `D-083` and `D-084` (the final suite, medians of 5 runs): `total` p50/p95/p99 10.99/11.62/12.03 ms; `unattributed` p50 2.11 / p95 2.66 ms; `animate_sprites` p50 0.30 / p95 0.38 ms; `flush` p50 0.40 ms and the default extract p50 7.68 ms, both reported only; `live` unchanged. Peak RSS is about 145 MiB; RSS grows 6–12 KiB/s. The row's `total` did not move (p95 11.65 → 11.63 ms against the baseline of the pass, `unchanged`): the engine stages got cheaper and the extract dearer by about the same amount.

- **Accepted regression (`D-084`).** Against the baseline of the pass (5 runs a side, per-run means) `animate_sprites` p95 reads `regressed`: 0.35 → 0.38 ms, +0.04 with an interval of +0.01 to +0.06 against a threshold of 0.02; its p50 reads `unchanged` (0.30). In the same compare `unattributed` p50 improved from 2.37 to 2.10 ms and `flush` from 0.70 to 0.40 ms. The mechanism was not found, and the owner accepted the regression on 2026-10-01; `D-084` holds the justification the regression policy asks for.
- **The extract in this row, reported only:** p50 7.03 → 7.68 ms in that compare (p95 7.71 → 8.39), which reads `regressed` and is judged nowhere, because `gpu` and `gpu-throughput` own the extract and read no `regressed` there. `integrated` shows the same in smaller: extract p50 2.61 → 2.80 ms, also `regressed` and also reported only.
- When they appeared: this row's two readings came with the insert runs of `D-084` (extract p50 about 7.05 → 7.52 ms) and the extract rose again with its typed queues (→ 7.68); `integrated`'s extract moved with the typed queues only (about 2.60 → 2.85). What was ruled out on this row: its allocation traffic is the same before and after the insert runs (5,398.6 and 5,399.4 `malloc` calls per frame, exact counts); the extract's collect loop is byte-identical and equally aligned in both builds; the sprites' asset-ID strings are equally scattered (0.2% of consecutive rows share a page); writing a run's values in column order changes nothing; disabling transparent huge pages changes nothing. The slower build runs fewer instructions but more cycles, with more L1 data misses (in the extract's sort and the particle tick) and more cycles in `malloc`, `free`, the string compare and the extract's batching loop. Without telemetry logging both builds take the same cycles. So the row's time follows the allocator's state under its own traffic (a `String` clone per animation frame change, a `String` per particle, the extract's per-frame buffers, the telemetry's formatting), and that state shifts when allocations are added or removed anywhere else. See "String sprite IDs" under "Engine findings".

**After `D-086`** (2026-10-01, medians of 5 runs): `total` p50/p95/p99 6.12/7.14/7.57 ms, 158.5 FPS against 89.7; `unattributed` p50 2.01 / p95 2.47 ms; `animate_sprites` p50 0.32 / p95 0.40 ms; `flush` p50 0.42 ms and the default extract p50 2.91 ms, both reported only; `live` unchanged. Peak RSS is about 136 MiB; RSS grows 8–19 KiB/s. `total` p95 now sits below the 8–16 ms band; see "Below the calibration band" under "Open proposals".

- **Accepted regression (`D-086`).** Against the tree without the rewrite (5 runs a side, per-run means) `animate_sprites` p95 reads `regressed`: 0.38 → 0.40 ms, +0.02 with an interval of +0.00 to +0.05 against a threshold of 0.02. With 15 runs a side in one sitting it reads 0.37 → 0.40 ms (+0.03, interval +0.02 to +0.04); p50 reads `noisy` (0.30 → 0.32). The old extract read every `Sprite` late in the frame and left the column in cache for this system, the first of the next frame; the new one reads it once, earlier. In the same compare `unattributed` reads `improved` at p50 2.10 → 2.01 ms and p95 2.69 → 2.47 ms, and the row's `total` p50 falls by 4.88 ms. The owner accepted the regression on 2026-10-01.
- **The extract in this row, reported only:** p50 7.76 → 2.95 ms in that compare, about 0.06 µs per sprite against 0.16. Jitter reads 1.46 ms against 1.12, and the spread is the extract's (p50 2.91, p95 3.89 ms). The largest frame of the five runs is 10.1 ms, where every run of the suite before held one of 12.1–15.8 ms.
- The owned share is 38.8% of `total` (means): still ✗ against the target of 50%, for the reason below.
- **Accepted regression (`D-087`).** With the direct present path `unattributed` p95 reads `regressed`: 2.47 → 2.71 ms, +0.24 with an interval of +0.19 to +0.29 against a threshold of 0.123 (5 runs a side). It is that path and not placement: a build with the same layout that still blits reads 2.49 ms, 15 runs a side. p50 reads `unchanged` (2.00 → 2.04 ms). The cost moves between stages again: `extract` p50 reads 2.94 → 2.82 ms and `animate_sprites` p50 0.32 → 0.30 ms, both `improved`, and `total` is `unchanged` (6.15 → 6.17 ms p50). After `D-087` (medians of 5 runs): `total` p50/p95/p99 6.10/7.03/7.48 ms, 160.9 FPS; `unattributed` p50 2.03 / p95 2.72 ms; `animate_sprites` p50 0.29 / p95 0.41 ms; extract p50 2.79 ms; jitter 1.29 ms. The mechanism was not established. The owner accepted the regression on 2026-10-01.
- **On 2026-10-02** (Session B's final suite, medians of 5 runs) the row reads the same: `total` p50/p95/p99 6.22/7.23/7.76 ms, 159.6 FPS and 157.8 by `interval`; `unattributed` p50 2.07 / p95 2.75 ms; `animate_sprites` p50 0.30 / p95 0.42 ms; extract p50 2.78 ms; jitter 1.48 ms. One run of the five holds two frames above 1.5 × p50 (10.17 ms at most).

- **The share check fails by construction:** `unattributed` plus `animate_sprites` is 24.8–25.1% of `total` against a target of 50% ✗ (22.5% on 2026-10-01). At the design defaults `live` reached 60,416, but `total` p95 was 24.0 ms and the default extract took 15.71 of 23.03 ms: about 0.16 µs per string-ID sprite, against 0.06 µs in `gpu-throughput`'s one-texture field. Per frame, one live particle costs about 0.24 µs (0.066 of it unattributed) and one animated sprite about 0.2 µs (0.038 in `animate_sprites`), so no mix reaches 50%.
- At 8,000 animated sprites, 240, 280 and 320 emitters gave live medians of 36,258, 42,288 and 48,351 and p95 values of 9.78, 12.20 and 13.42 ms. The owner accepted 280 emitters (about 70% of the design's 60,000-particle target) and 8,000 animated sprites, mid-band.
- Capacity search finds the row `extract`-limited (✗ against the declared `unattributed`) at both budgets: the same finding.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `08a3c7c126a0d43a`.

## `integrated`

Owns the effects that only appear when systems interact, and is the one benchmark judged on total frame time and its tail. Warm-up 120 frames; the GPU diagnostic run is on. Modules: `integrated.rs` (knobs, config and registration), `integrated/level.rs` (the seeded level), `integrated/assets.rs` (generated textures, the level tilemap, walk clips, fire and spark configs), `integrated/scene.rs` (startup spawning), `integrated/runtime.rs` (per-walker arrays and the runtime resource) and `integrated/systems.rs` (gameplay systems in frame order).

- **Level** (seeded, 96 rows of 16 px tiles, `level_tiles` wide). The ground surface wanders between rows 80 and 90 in flat runs of 8–24 columns joined by single steps or staircases of 3–5 steps, over a three-tile collision crust; the rendered terrain fills to the bottom row. Full-height walls close both ends, and walls 1–2 columns wide and 3–6 tiles high stand on the ground every 48–96 columns. Three tiers at rows 72, 60 and 48 (±1) carry one-tile platforms 10–36 tiles long with gaps of 3–9. The tilemap holds the collision layer (the engine never draws it), the rendered terrain and two decoration layers; four parallax strips (factors 0.1–0.7, one atlas page) sit behind. At the default: 24,576 × 1,536 px, 8,528 tile proxies, 317 walkable spans and 81,776 px of walkable surface.
- **Walkers.** Dynamic 12 × 20 px AABBs with tile collision, drawing a 24 px lit walk clip (6 frames, 4 color variants, one lit atlas page with normal and emissive maps). The AI reads last frame's contacts: a wall turns a walker unless it is a one-tile step, which it hops; a ledge (no ground within two tiles ahead) turns it; a random hop comes every 1.5–4 s where ground 56 px ahead supports the landing. Touching down after 6 or more airborne frames sends a `SquashEvent` (`OnLand`) to the engine's squash systems.
- **Casters.** Every tenth walker (250 at the default) fires 3 px circle projectiles at 700 px/s, 8–29° above the horizontal, `fire_rate` times per second.
- **Hits.** A hit despawns the projectile, spawns a spark burst, knocks a struck crate and wakes it with `physics::wake`, adds 0.08 trauma when within 64 px of the view, and flashes a struck walker.
- **Flash.** The default extract draws a lit sprite without its material (lit wins, `D-061`, with a warning per sprite and frame). A struck walker therefore switches to an unlit copy of its clip (a second atlas page and clip set) drawn with `damage_flash`, an override block (color in `vec4[0]`) and a `UniformScalar` F0 tween from 1 to 0 over 0.3 s, tagged `flash`. `TweenComplete` arrives after the systems, so the next frame reads it in the previous window, restores the lit clip and removes the block. Each flashing walker has its own override hash and so its own batch: about 17 at a time at the default.
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
- **Owned:** `total` p50/p95/p99 and jitter (p99 − p50, judged with p99's threshold). No stage is owned. The declared bottleneck is `physics_step`, which calibration found limiting (6.66 of 12.03 ms, means) ahead of the default extract (2.60 ms); on 2026-10-01 it is 5.08 of 10.43 ms, with the extract at 2.81, after `D-085` 5.10 of 9.55 ms, with the extract at 2.13, and after `D-086` 5.14 of 8.52 ms, with the extract at 0.98. Key knobs `actors`, `crates`, `props`, `torches` and `pickups`.
- **Row note:** Judged on total frame time; `jitter` is p99 - p50 of `total`, judged with p99's threshold. No stage is owned: the declared bottleneck, physics_step, is the stage calibration found limiting (walkers and crates against the tile proxies), ahead of the default extract. Tiles draw at z_norm 0, so the row needs the default cpu_stable depth sort (under gpu_depth they cover every sprite). Without T1, particle and tween time lands in `unattributed`. Changing HUD and name-tag text grows RSS through the text buffer cache (360-frame TTL). That last sentence is the binary's own text (`integrated.rs`) and stopped being true with `D-085`, which left the benchmark's sources alone: RSS no longer grows with the text.

**Calibrated default:** `actors` 3,000 → 2,500; the other counts stay as designed. Target: a `total` p95 of 10–16 ms ✓, each time it was measured until `D-086`. After `D-085` the median sat 0.14 ms above the band's floor; after `D-086` it is 9.24 ms, 0.76 ms below it, which the owner accepted (`D-086`; "Below the calibration band" under "Open proposals").

| Measured | `total` p50 / p95 / p99 per run (ms) | Jitter per run (ms) |
| --- | --- | --- |
| 2026-09-30, at calibration | 11.97/12.37/12.52, 12.04/12.47/12.53, 12.40/12.90/13.29 | 0.55, 0.49, 0.89 |
| 2026-10-01, after `D-080`–`D-084` (the final suite) | 10.53/11.01/11.26, 10.68/11.23/11.61, 10.55/11.02/11.26, 10.51/11.05/11.20, 10.47/11.07/11.32 | 0.73, 0.93, 0.71, 0.69, 0.85 |
| 2026-10-01, after `D-085` | 9.62/10.14/10.61, 9.59/10.11/10.47, 9.67/10.41/10.72, 9.72/10.62/11.10, 9.60/10.05/10.31 | 0.99, 0.88, 1.05, 1.38, 0.71 |
| 2026-10-01, after `D-086` | 8.65/9.29/9.86, 8.56/9.15/9.58, 8.54/9.24/9.53, 8.60/9.29/9.95, 8.57/9.01/9.38 | 1.21, 1.02, 0.99, 1.35, 0.81 |
| 2026-10-01, after `D-087` | 8.41/8.99/9.34, 8.51/9.05/9.37, 8.36/8.89/9.16, 8.47/9.00/9.45, 8.40/8.87/9.36 | 0.93, 0.86, 0.80, 0.98, 0.96 |
| 2026-10-02, Session B's final suite | 8.44/8.97/9.25, 8.40/8.99/9.44, 8.40/9.06/9.38, 8.39/9.04/9.34, 8.39/8.93/9.53 | 0.81, 1.04, 0.98, 0.95, 1.14 |

The design defaults gave p95 13.71 ms at calibration. The `actors` sweep on 2026-10-01 (`--sweep actors=1500,2000,2500,3000,3500 --repeat 3`), medians of 3 runs in ms:

| `actors` | `total` p50 / p95 / p99 | `physics_step` mean | `extract` mean | Turns per walker per s |
| --- | --- | --- | --- | --- |
| 1,500 | 8.43 / 9.01 / 9.40 | 3.58 | 2.52 | 1.4 |
| 2,000 | 9.49 / 10.03 / 10.45 | 4.36 | 2.68 | 1.8 |
| 2,500 | 10.61 / 11.30 / 11.70 | 5.15 | 2.87 | 2.1 |
| 3,000 | 11.66 / 12.29 / 12.66 | 5.86 | 2.99 | 2.4 |
| 3,500 | 12.58 / 13.34 / 13.77 | 6.55 | 3.03 | 2.6 |

- Each 500 walkers add about 1.1 ms of p95, about 0.75 ms of it in `physics_step`. The default sits 1.3 ms above the band's floor in the sweep (1.05 ms in the final suite) and 4.7 ms below its ceiling. Jitter reads about 1 ms at every sweep point and 0.73 ms in the final suite's five runs.
- At calibration the sweep was a single run per value: p95 9.99, 11.36, 12.38, 13.91 and 15.10 ms with `physics_step` means of 4.72, 5.75, 6.57, 7.76 and 8.71 ms. Each 500 walkers added about 1.3 ms of p95, nearly all in `physics_step`, and 2,500 was chosen because it left 2.4 ms to the band's floor and cut the jitter to about 0.5 ms, against 1.4 ms at 3,000.
- Stage means at the default (2026-10-01, the final suite): `update` 5.76 (`physics_step` 5.08, `animate_actors` 0.31, `collision_events` 0.16, `actor_ai` 0.10), `extract` 2.81, `render` 1.44 (`render_encode` 0.94, present wait 0.44), `unattributed` 0.32 and `flush` 0.09 ms. The frame is CPU-bound: the GPU diagnostic run puts `render_span` at 6.20 ms p50 (SMAA 2.56, scene 1.32, bloom 1.05 over all its passes, tonemap 0.33, present 0.32, vignette 0.26). At calibration `update` was 7.46 (`physics_step` 6.66), `extract` 2.60 and `flush` 0.17 ms; the extract's rise is the reported-only reading described under `particles`.
- Stage means after `D-085`: `update` 5.67 (`physics_step` 5.10, `animate_actors` 0.21, `collision_events` 0.15, `actor_ai` 0.09), `extract` 2.13, `render` 1.35 (`render_encode` 0.90, present wait 0.40), `unattributed` 0.33 and `flush` 0.07 ms: 9.55 ms in all, 104.7 FPS against 94.9. Against that session's own baseline (5 runs a side, per-run means) `total` reads `improved` at p50 10.59 → 9.64 ms and p95 11.25 → 10.27 ms, `noisy` at p99 (11.56 → 10.64 ms, Δ −0.92 against a threshold of 0.925) and `unchanged` in jitter (0.97 → 1.00 ms). About 0.9 ms came with the text change, in stages whose code it left alone: the old cache took fresh pages every frame, which slowed the extract (2.85 → 2.13 ms) and `animate_actors` (0.31 → 0.21 ms). Keeping the post chain's GPU objects between frames took another 0.1 ms off `render`. The diagnostic run's `render_span` reads 5.91 ms p50, with the scene pass at 1.02 ms against 1.35 (the scene-pass note under `gpu`).
- Stage means after `D-086`: `update` 5.71 (`physics_step` 5.14, `animate_actors` 0.21, `collision_events` 0.16, `actor_ai` 0.10), `extract` 0.98, `render` 1.42 (`render_encode` 0.96, present wait 0.44), `unattributed` 0.34 and `flush` 0.08 ms: 8.52 ms in all, 117.7 FPS against 104.7. Against the tree without the rewrite (5 runs a side, per-run means) `total` reads `improved` at p50 9.64 → 8.58 ms, p95 10.27 → 9.20 ms and p99 10.64 → 9.66 ms, and `unchanged` in jitter (1.00 → 1.08 ms). All of it is the extract, 2.13 → 0.96 ms at p50. `render_encode`, reported here and owned by the `gpu` rows, reads 0.90 → 0.93 ms at p50 and 1.00 → 1.10 ms at p95. The diagnostic run's `render_span` stays at 5.90 ms p50.
- After `D-087`: `update` 5.62 (`physics_step` 5.06), `extract` 0.96, `render` 1.37 (`render_encode` 0.94, present wait 0.42), `unattributed` 0.34 and `flush` 0.08 ms: 8.37 ms in all, 119.4 FPS. Against the tree before (5 runs a side, per-run means) `total` reads `unchanged` at p50 8.58 → 8.43 ms, p95 9.20 → 8.96 ms and p99 9.66 → 9.34 ms, and in jitter (1.08 → 0.91 ms). The frame is CPU-bound, so the blit it no longer pays shows as GPU time: 6.30 ms of engine time per frame against 6.70. In the diagnostic run `smaa_neighborhood` reads 0.62 → 0.80 ms and the `present` pass (0.32 ms) is gone; `render_span` p50 reads 5.90 → 5.81 ms, `unchanged`, because the span now contains the surface clear.
- Interaction counters, means per frame (2026-10-01; `D-081` changed the trajectories, so they differ a little from calibration's): 13,043 proxies (8,528 tile proxies plus 4,515 dynamic bodies), 929 sleeping crates, 7,034 pairs, 6,319 contacts and 23,263 collision events; 15 live projectiles, 1.26 shots, 1.21 hits and 16.8 flashing walkers; 6,204 live particles; 27.7 point lights in view (25–30); 23.7 landings and 88.6 turns.
- `tile_collision=off` spawns 328 merged static boxes (runs along rows, merged downward) in place of the 8,528 tile proxies. On 2026-10-01 (3 runs each, one sitting with the default): the `physics_step` mean drops from 5.17 to 3.29 ms, `total` p95 from 11.38 to 8.85 ms and events from 23,263 to 16,420, while turns stay at 88. At calibration the same switch read 6.57 to 4.27 ms. The AI reads the level grid in both modes.
- `physics_step` still rebuilds the pair list about 3.5 times per frame in this row, because most of its rebuilds follow a contact wake and `D-081` repairs only budget trips (1,003 of 1,403 builds in 400 frames). See "Contact wakes rebuild the pair list" under "Engine findings".
- At the 144 Hz budget capacity search found the row present-bound (✗ against `physics_step`): scale moves the scene counts but not the level or the 1080p post chain, whose GPU span of about 6 ms sets a floor near 6.9 ms. Searched again after `D-085`: scale 0.085 (212 actors) at 144 Hz, still present-bound (✗), and 1.53 (3,835 actors) at 60 Hz, `physics_step`-limited (✓); the same morning's build read 0.09 and 1.48. On 2026-10-02, with `D-086` and `D-087` in: 0.30 (743 actors) at 144 Hz, still present-bound (✗), and 1.76 (4,391 actors) at 60 Hz, `physics_step`-limited (✓).
- Until `D-085` peak RSS was about 283 MiB and grew about 24 MiB/s from the name tags and HUD (see "Engine findings"), so it depended on capture length; at calibration it grew 19.5 MiB/s, at a lower frame rate. Since `D-085` it is about 167 MiB, and RSS growth reads 0–0.3 MiB/s per run (reported only). After `D-086` it is about 170 MiB, because the extract keeps its buffers, and growth reads 0.2–2.3 MiB/s per run: the buffers step to a larger size inside the fitted half of a 3.6 s run. Over 900 frames two of three runs are flat after the first two seconds (0.03–0.04 MiB/s, peaks 172 and 168 MiB) and the third steps from 171 to 175 MiB.

**Workload version history:** 1, the initial version (2026-09-30). Digest at the default: `334b519e18b543d7`. It changed on 2026-10-02 with `D-092` (the arrival pass); before it the digest was `5f031f4d947964cb`, from 2026-10-01 and `D-081` (pair order), and before that `9b2617e4c1ab23f7`.

## Engine findings

The suite exposes these engine costs and behaviors on purpose. Each is a finding for separate engine work, not something the suite fixes or works around silently.

- **Burst latch.** `EmissionKind::Burst` fires once whatever `once` says, because the `continuous_accum` latch in `particle_tick_system` never resets (`crates/tungsten/src/particles.rs`). `particles` re-arms bursts by clearing the latch; `integrated` spawns a fresh emitter per hit.
- **Tiles at `z_norm` 0.** `extract_tilemaps` writes the nearest depth, so under `TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth` the tilemap covers every sprite: the M25 `gpu_depth` smoke rows render only tiles and text, and `integrated` needs the default `cpu_stable`.
- **Lit-plus-material warning.** The default extract logs a warning once per sprite and frame when a lit sprite carries a material, so lit plus material costs a log call per sprite. `integrated` swaps a struck walker to an unlit clip to show its flash.
- **Text-cache RSS (fixed, `D-085`).** Until 2026-10-01 the text pipeline kept every changed section's shaped buffer for 360 frames and pruned every 120. With text changing every frame, RSS grew until the TTL saturated: about 139 MiB/s in `gpu` (peak near 731 MiB at 300 frames and 905 MiB at 900, against 171 MiB with 2 glyphs) and 24 MiB/s in `integrated`. Once the cache was full the prune gave `gpu` a 41–43 ms frame every 120 frames, after two frames of 17–23 ms on the way there, and the fresh pages slowed whatever allocated beside the cache (the default extract in both rows). The layout cache now holds about three frames of sections and recycles its buffers: no growth, 169 MiB in `gpu` at either length and 167 MiB in `integrated`.
- **Default extract (rewritten, `D-086`).** It doesn't cull and resolves string IDs. Until `D-086` it also sorted every sprite as a 48-byte tuple every frame and allocated and freed its buffers every frame, about 0.16 µs per sprite in `particles` and 0.06 µs in `gpu-throughput`'s one-texture field: at 400,000 sprites glibc grew and trimmed the heap for the buffers each time, about 1,600 page faults and 8.8 ms of kernel time per frame in `gpu-throughput`, and now and then one frame's extract took 11–13 ms in `particles` where it took 7.8 (in 8 of 135 runs on 2026-10-01, on every tree measured). It now makes one pass into buffers the app keeps between frames, sorts 16-byte keys only when the query is not already in painter order and resolves IDs through a small cache, with the same batches and pixels: about 0.06 µs per sprite in `particles` and 0.03 µs in `gpu-throughput`. Measured on 2026-10-01 (medians): `gpu-throughput` `extract` p50 24.30 → 13.08 ms and 38.6 → 68.7 FPS, `particles` `total` p50 11.05 → 6.12 ms, `integrated` 9.62 → 8.57 ms. With it `particles`' owned `animate_sprites` p95 reads `regressed` (0.37 → 0.40 ms, 15 runs a side), because that system now reads the `Sprite` column cold; the owner accepted it. It still dominates `gpu-throughput` and is still the largest stage of `particles`; the ID string compare is the largest cost left in `gpu-throughput`'s extract.
- **Surface clear and present blit (resolved, `D-087`).** Until `D-087` every frame ended with a full-screen blit into the swapchain (0.32 ms of GPU time at 1080p), and wgpu first cleared the fresh surface texture in a pass of its own, because the blit's pass loaded it. Giving that pass a clear removed wgpu's pass and cost the same on RADV: the present pass read 0.32 → 0.46 ms and the empty frame's GPU time stayed at 0.58 ms (`D-085`), so it was not adopted. Now the last full-screen stage renders into the swapchain and clears it, and only a capture frame blits. The blit is saved in every row (GPU time per frame 10.93 → 10.60 ms in `gpu`, 6.70 → 6.30 ms in `integrated`, 1.45 → 0.72 ms in `physics`, 0.58 → 0.18 ms on the empty frame, which runs at 2,074 FPS against 1,101). The clear is not saved: it moved into the pass that first writes the swapchain, which is why `smaa_neighborhood` reads 0.18 ms more. Only `LoadOp::DontCare`, which needs an `unsafe` call, would remove it.
- **Tile collision proxies** are rebuilt from a full-map scan every frame (`gather_tilemap_proxies`): 8,528 proxies in `integrated`, which cost about 1.9 ms of `physics_step` against merged boxes (2026-10-01; 2.3 ms before the pass).
- **Contact wakes rebuild the pair list.** `D-081` repairs the pair list for a proxy that runs out of travel budget, but a contact wake still rebuilds it. `physics` and `physics-sparse` build once per frame; `integrated` builds about 3.5 times, 1,003 of its 1,403 builds in 400 frames following a wake, which is why pair repair left that row `unchanged`. Re-pairing the woken proxy would remove about 2.5 of those builds per frame and needs its own decision, because a woken body becomes an initiator and needs its sleeping and static neighbours.
- **No bundle insert.** Through the `CommandBuffer` a spawn's k inserts box nothing and move the row once (`D-084`). k direct `World::insert` calls still move it k times and k(k−1)/2 values, which is what `churn`'s `immediate` mode measures; there is no bundle-insert API.
- **Archetype count.** With the same 250,000 entities and the same work, `ecs` `update` p50 reads 9.14 ms in 64 archetypes, 10.52 in 488 (the default) and 19.15 in 3,189 (`fragmentation` 1, 8 and 64; 3 runs each on 2026-10-01). Per-archetype setup is the small part: a prototype that found a query's columns through the slot table instead of the key scan read 9.13, 10.37 and 18.74 ms, `unchanged` at the default in two captures, and was reverted (`D-083`). The rest is consistent with the first cache lines of every column of every archetype missing, since each column is its own allocation. Nothing reaches that cost yet; a software prefetch is not callable without `unsafe`.
- **String sprite IDs.** Each animation frame change clones a `String`, each particle carries one, and the extract resolves IDs by string. Besides their direct cost, they tie `particles`' `animate_sprites` and the default extract in `particles` and `integrated` to the allocator's state: a change elsewhere that adds or removes allocations moves those readings with no change to their code (`D-084`, and the `particles` section). Interned IDs would take the strings out of all three stages.

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

**Below the calibration band.** Neither the pass of 2026-10-01 (`D-080`–`D-084`) nor the extract rewrite of the same day (`D-086`) changed a default, so five rows now run under the `total` p95 band their defaults were calibrated to (8–16 ms; 10–16 ms for `integrated`):

| Row | `total` p95 at calibration | On 2026-10-01 | Knob that would bring it back |
| --- | --- | --- | --- |
| `physics` | 14.58 ms | 6.07 ms | `balls` |
| `physics-sparse` | 10.11 ms | 3.73 ms | `bodies` |
| `churn` | about 11.3 ms | 4.06 ms | `population` |
| `particles` | about 11.6 ms | 7.14 ms (after `D-086`) | `emitters`, `animated` |
| `integrated` | about 12.5 ms | 9.24 ms (after `D-086`) | `actors` |

`ecs` (11.35 ms) and `gpu` (12.42 ms) are still inside. The owner accepted the last two rows with `D-086`. The five rows stay valid and comparable as they are, and every guard passes. One practical effect: a `physics-sparse` or `churn` run now lasts about 1.5 s, so a single step of RSS inside the fitted half reads as MiB/s of growth (reported only). Recalibrating means new defaults, which is a change to the benchmark's work: a `workload_version` bump, a new baseline, and a history line in the row's section. That is a separate decision and is not approved.

**Left open by the pass of 2026-10-01.** None is approved or started:

- Re-pairing a woken proxy instead of rebuilding the pair list ("Contact wakes rebuild the pair list" above; `D-081`).
- Interned sprite IDs ("String sprite IDs" above), the lever for the accepted `animate_sprites` regression and the reported-only extract readings.
- The per-archetype cold start ("Archetype count" above): no candidate without `unsafe`.
- Recalibrating `physics`, `physics-sparse` and `churn` (the table above).
- The per-run mode of `ecs` (`stats_decay`, `regen`, `follow`, `buffs`), whose cause was not looked for.

**Left open by Session A of the render-path pass (2026-10-01, `D-085`).** None is approved:

- The default-extract rewrite was merged the same day (`D-086`; "Default extract" above). What it leaves open: interned sprite IDs for the string compare, extract culling, recalibrating `particles` and `integrated` (the table above), and the cause of the scene pass's two values in the GPU diagnostic run (`gpu`, "After `D-086`").
- The surface clear ("Surface clear and present blit" above): since `D-087` the pass that first writes the swapchain carries it. `LoadOp::DontCare` would remove it and would be the engine's first `unsafe` outside a test.
- The `integrated` row note in the binary, which still names the old text cache.

**Left open by Session B of the render-path pass (2026-10-02).** None is approved; the first three were gated steps of the now-archived `docs/plans/archive/gpu-perf-pass.md` that the owner left at `no`. Retirement of that execution plan does not approve these experiments:

- Extract culling: the default extract still emits every sprite, in view or not.
- A compositor-bypass hint on X11: the compositor's stall is what sets the tail of `gpu` at `min` (the frames near 4.7 ms above).
- The shape-run cache experiment for the text pipeline's shaping cost.
- The frame cap overshoots its period by 0.06 ms per frame (16.73 ms mean `interval` at a cap of 60, 59.8 FPS), because each deadline counts from the frame's own start; a deadline grid kept across frames would remove it.
- The HUD's `fps` row still divides by the CPU frame time; `FrameTimings::interval_ms` now holds the period it would need.

**RSS growth threshold.** The current capture contract defines none, so RSS growth is reported for every row, `churn` included, and judged for none. The open proposal judges it for `churn` only:

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
