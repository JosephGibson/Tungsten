# Engine and kit performance pass (`engine-perf-pass`) — draft

- **status:** draft
- **goal:** lower example 01's `total` p50, p95 and p99 under stress (`e01-pit`, `e01-stress`) and its CPU frame in ordinary play (`e01-play`'s `update`, `physics_step` and `extract`; its `total` is the GPU's frame on the reference machine) through engine and kit code, with every physics digest and every pixel kept: a worker pool for the physics step's pure stages, then the serial levers Phase B's reviewers confirmed, each judged by a sitting against `95939269`.
- **non-goals:** example 01's own systems (`fire_exposure`, `fire_spread`, `ball_look`) and any 01 change but a forced call-site port; any change of motion or pixels, new `unsafe`, or a third-party dependency (each measured and written under Proposals instead); a release cut; the levers under 0.05 ms that Follow-ups lists.
- **files to touch:**
  - Step 1 (landed, `95939269`): `scripts/perf-rows.json`, `scripts/perf.py`, `scripts/test-perf.py`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/testing/guard.rs`, `api/tungsten.txt`, `examples/01_platformer/tests/game/guards.rs`, `examples/01_platformer/tests/game/guards/play.json`, `docs/perf/benchmarks.md`, `docs/perf/profiling-workflow.md`, `.claude/skills/tungsten-perf/SKILL.md`.
  - Step 2: `crates/tungsten-core/src/physics/step/pool.rs` (new), `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/broadphase.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten-core/src/tests/physics/workers.rs` (new), `crates/tungsten-core/tests/integration/physics_determinism.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `api/tungsten-core.txt`, `api/tungsten.txt`, `scripts/perf.py`, `scripts/perf_stats.py`, `scripts/test-perf.py`, `docs/perf/profiling-workflow.md`, `docs/perf/benchmarks.md`, `docs/getting-started.md`, `AGENTS.md`, `DESIGN.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `ROADMAP.md`.
  - Step 3: `crates/tungsten-core/src/physics/step/arrival.rs`, `step/pool.rs`, `step/mod.rs`, `crates/tungsten-core/src/tests/physics/workers.rs`.
  - Step 4: `crates/tungsten-core/src/physics/step/mod.rs`, `step/pool.rs`, `step/solver.rs`, `crates/tungsten-core/src/tests/physics/workers.rs`.
  - Step 5: `crates/tungsten-core/src/physics/step/pairs.rs`, `step/mod.rs`, `step/sleep.rs`, `step/gather.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, and the guard pins a carry or sleep buffer moves (`examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/*.json`).
  - Step 6: `crates/tungsten-kit/src/damage/effects.rs`, `crates/tungsten-kit/src/overlap/impact.rs`, `crates/tungsten-kit/src/tests/effects.rs`, `crates/tungsten-kit/src/tests/impact.rs`, `crates/tungsten/src/tests/kit_recipes/`, `api/tungsten-kit.txt`, and the guard pins that count an effect instance's bytes (`examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/*.json`).
  - Step 7: `crates/tungsten-core/src/physics/step/sleep.rs`, `step/gather.rs`, `step/mod.rs`, `step/pairs.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, and the guard pins that count a kept buffer (`examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/*.json`).
  - Step 8: `crates/tungsten-core/src/physics/step/mod.rs`, `step/pairs.rs`, `step/sleep.rs`, `step/solver.rs`, `crates/tungsten-core/src/physics/broadphase.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/broadphase.rs`, and the guard pins that count a new list (`examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/*.json`).
  - Step 9: `crates/tungsten-core/src/ecs/archetype.rs`, `crates/tungsten-core/src/ecs/storage.rs`, `crates/tungsten-core/src/ecs/world.rs`, `crates/tungsten-core/src/tests/ecs/archetype.rs`, `crates/tungsten-core/src/tests/ecs/storage.rs`, `crates/tungsten-core/src/tests/ecs/world.rs`, `crates/tungsten/src/sprite_extract.rs`, `crates/tungsten/src/tests/sprite_extract.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, and the guard pins that count the extract's key buffers (`examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/*.json`, `crates/tungsten/src/tests/guards/*.json`).
  - Step 10: `crates/tungsten-render/src/post/bloom.rs`, `crates/tungsten-render/src/renderer.rs`, `crates/tungsten-render/src/sprite.rs`, `crates/tungsten-render/src/tests/`.
  - Step 11: this plan's Proposals only (scratch builds under `target/engine-perf/`).
  - Step 12: this plan (deleted), `ROADMAP.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`.
  - Every step: this plan's evidence log.
- **ordered steps:** (1) the `e01-play` row, landed; (2) the worker pool and the pair build's walks; (3) the arrival round's search on the pool; (4) the pair loop's narrow phase ahead, on the pool, and the one-worker sitting; (5) the warm-start carry remapped by index and the sleep table rebuilt by cursor; (6) the kit's counted effects by slot and the impact record's cursor; (7) the idle step: tile supports, sleep loops and tile tails; (8) the pair loop's serial part and the merged fill; (9) long archetype keys, a query's match and the extract's biased orders; (10) the renderer's encode; (11) the proposals measured; (12) close.
- **done-when:** every step's done-when passes or the step is recorded stopped, quoted in the evidence log; the close's full sitting against `95939269` reads no owned metric `regressed` without a fix or a known-issues row; `just check` passes and example 01 runs its three rows with their digests unchanged; the plan deleted with its citations and the item out of Now.

Item `engine-perf-pass` in [ROADMAP.md](../../ROADMAP.md), off the roadmap by the owner's request of 2026-11-23, as the CPU pass ran ahead of 7.13 (`D-269`, `D-270`). Written 2026-11-23 at `95939269`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- **Owner's decisions (2026-11-23).** A deterministic worker pool for the physics step's pure stages may land, bit-identical to the serial path at every worker count, standard library only, gated on a re-measured probe saving 2 ms of `total` p95 on both `e01-pit` and `e01-stress`. Only exact changes land (digests and stills equal, no new `unsafe`); the rest is measured into Proposals. Step 1's row is the base of every sitting. The budget is 40 % of the weekly meter from 2026-11-23T08:00Z, stopping at 35 %.
- **Where the time is** (Phase A, gated profiles at the base): `physics_step` is 86 % of `e01-pit`'s wave frames (36.0 ms), 78 % of its awake frames (18.7 ms) and 74 % of `e01-stress`'s 20 highest (37.1 ms). `e01-play`'s `total` (2.90 ms) is the GPU's frame, `render_span` 2.72 ms at 1080p, and its p95 and p99 are KWin's 60 Hz slice; its CPU work is `update` 0.88, `extract` 0.12 and 0.67 ms of encode and submit.
- **The gate passed** (timed probe, alone, three runs a count): six participants take 8.9 ms off `e01-pit`'s `total` p95 and 7.4 off `e01-stress`'s, every digest equal at 1, 2, 3, 4, 6, 8 and 12.
- **Stands on:** `D-080` (visit order), `D-081` (repair), `D-092` (arrival pass), `D-226` (sittings), `D-269` (carry). **Supersedes** `D-067` and `D-131`'s no-worker and rayon clauses; **amends** `D-220`'s thread count, `D-269` and `D-083`.
- **Evidence** is machine-local under `perf-runs/20261123-engine-perf/`: `README.md` (log), `phase-a/pack/` (profiles and folds), `agents/` (eight reviews, four verifications), `probe/` (the pool's probe and its timed readings), `tools/` (readers).
- **Later items expect** 7.16 to start from a tree whose rows' reference numbers and levers `docs/perf/benchmarks.md` states.

## 1. Audit

Read at `95939269`. Times are ms a frame from the gated profiles unless a row's metric is named.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Base means of five runs, `total` p50 / p95 / p99: `e01-pit` 22.54 / 35.92 / 40.36, `e01-stress` 36.51 / 43.63 / 46.24, `e01-play` 2.90 / 4.61 / 4.97, `blast` 20.62 / 31.99 / 36.31. `physics_step` p50: 18.23, 23.98, 0.31, 19.98. | `perf-runs/20261123T053827Z-e01-pit`, `…T054018Z-e01-stress`, `…T054234Z-e01-play`, `…T054426Z-blast` |
| A2 | The step is serial end to end (`D-067`); the build's candidate walks cost 4.3–5.3 ms of an awake pile's frame, the pair loop's margin and narrow phase 4.6–6.3, the arrival round's search 8.7 of a wave frame and 6.5 of `e01-stress`'s highest. | `agents/physics-pool.md`, `phase-a/pack/fold/pit-top20.substep.txt` |
| A3 | Those three stages read state nothing writes while they run: the walk reads `pair_flags`, `pair_bounds`, `layers` and the grid between staging and `sent.enter`; the narrow phase reads centre, velocity, shape, face mask and slope, none of which the pair loop writes; an arrival round writes `slot`, velocities and centres only after its search. | `step/pairs.rs:833-861`, `step/solver.rs:262-362`, `step/mod.rs:1393-1610`, `step/arrival.rs:441-598` |
| A4 | A script run skips the branch that starts the gamepad thread, so a pool started there would never start in a perf row. Guards drive `Harness` over `App::new` and count allocations per thread. | `crates/tungsten/src/app.rs:1197-1204`, `crates/tungsten/src/testing/guard.rs:20-22`, `:123` |
| A5 | In a frame whose proxies changed, `rekey` rebuilds one or two `ImpulseMap`s and the first substep seeds every contact through `ImpulseMap::get`: 0.83 + 0.93 + 0.18 ms of `e01-stress`'s steady frames, most of them. | `step/pairs.rs:487-522`, `:731-757`, `step/mod.rs:1415-1424`, `phase-a/pack/fold/stress-steady.physics.txt` |
| A6 | `SleepTable::rebuild` probes by key, 0.43 ms with `index_of` 0.25 in `e01-stress`'s steady frames. | `step/sleep.rs:201-215`, `:254-306` |
| A7 | A counted effect finds its entry by comparing heap IDs through `bcmp`: `effect_tick` p50 1.52 ms in `e01-stress`, 1,148 of 1,335 libc leaves under `Ticker::run` at the instance's ID load. | `crates/tungsten-kit/src/damage/effects.rs:3836`, `agents/verify-world.md` |
| A8 | `ImpactRecord::get` indexes a 20-byte slot by entity at random for every hitter: 41.84 % of the hitter loop in `e01-stress`. | `crates/tungsten-kit/src/overlap/impact.rs:858-862`, `:960-965` |
| A9 | An idle step (`e01-play`, 0.31 ms) spends 0.115 ms comparing about 10,150 tile supports that change only with the tile cache's version, 0.06 in sleep loops over tile proxies and 0.065 regathering tile tails. | `step/sleep.rs:107-146`, `step/gather.rs:1328-1349`, `phase-a/pack/fold/play-window.physics.txt` |
| A10 | The pair loop repeats a union per contact (0.48 ms awake), calls its seed through an out-of-line closure with a capture setup (0.17 + 0.30), and scans every contact for restitution (0.11 awake, 0.50 in the wave). | `step/sleep.rs:525-555`, `step/mod.rs:1415-1424`, `step/solver.rs:241` |
| A11 | A merged build restages about 91,000 tile refs of 114,000 every frame, `SpatialGrid::build` 0.47 and `insert` 0.29 ms. | `phase-a/pack/fold/pit-awake.physics.txt`, `agents/verify-physics.md` |
| A12 | An archetype whose key no shift separates falls back to a key scan on every `get`: 0.037 ms a frame of `e01-play`. A query clones an `Arc` per call (0.007 in play, 0.02 under stress). | `ecs/archetype.rs:61-80`, `:226-233`, `ecs/storage.rs:87-109` |
| A13 | The extract's radix sort already skips a byte no two orders differ in, but a sprite's order is its z with the sign bit flipped, so 01's z of −20 to 15 varies all four high bytes: six passes, `sort_keys` 0.13–0.15 of `stage_extract`'s 0.36–0.46 ms. | `crates/tungsten/src/sprite_extract.rs:112-114`, `:1217`, `:1395-1419` |
| A14 | Bloom wraps its 13 passes in encoder-level debug groups, each a carrier command buffer: 40 buffers before the acquire, 12 of them removable. Seven bloom targets are cleared and then fully overwritten. A sprite turn rebinds the instance buffer. | `crates/tungsten-render/src/post/bloom.rs:363-496`, `:678`, `:757`, `renderer.rs:1906-1908`, `sprite.rs:1399-1400` |
| A15 | The probe's forked code at one participant reads 1.0–1.8 ms slower at p50 than the serial code, and its serial code 0.0–0.6 ms slower than the base with its stage timers on. | `perf-runs/20261123-engine-perf/probe/timed-means.txt` |

## 2. Design

### 2.1 How a step is judged

- **The sitting.** `WGPU_BACKEND=vulkan just perf sitting --base 95939269 --cpu-only --wait-quiet 900`, alone on the machine, the full form (no `--cpu-only`) for Step 10 and the close. Exit 0 means no row reads `regressed`, `unresolved` or `invalid`.
- **Digests.** `python3 -B perf-runs/20261123-engine-perf/tools/judged.py <sitting> <out>` exits 0 only when every judged pair's digests match, on every row: the base's serial step against the candidate's, at the machine's automatic worker count.
- **A step's own move.** Every sitting has the same base, so a sitting's Δ is cumulative. Δ(sitting, row, metric, statistic) is the mean over its two judged pairs of candidate minus base, each side the mean over its valid runs of the run's statistic. A step's own move is Δ(its sitting) − Δ(the previous step's sitting). "Falls by at least T" holds when the own move is at most −T and each judged pair's candidate minus base lies below the previous sitting's Δ. `tools/step_delta.py <sitting> <previous sitting or -> <row> <metric>:<statistic> …` prints these; the evidence log quotes its lines. Step 2's previous Δ is 0.
- **Stills.** `python3 -B perf-runs/20261123-engine-perf/tools/stills.py` captures nine stills through a script's `capture` step (the six areas at window frame 200, `e01-pit` and `e01-stress` at frame 190, `e01-play` at frame 610) from the base's kept binary (`target/engine-perf/base-bin/`) and from the tree's aligned build, and compares the files' bytes; it prints `9 of 9 equal` and exits 0. The base against itself reads 9 of 9 (2026-11-23), so a still is reproducible.
- **Guards.** `just check` runs the counter guards serial, as every `Harness` test runs. A pin that moves needs its cause in the step's Moves line and the evidence log; `TUNGSTEN_GUARDS_WRITE=1` on that test, run twice.
- **The meter.** At each step's end `python3 -B perf-runs/713-chain/spend.py --since 2026-11-23T08:00:00Z`, its `chain:` line quoted in the log; at 35 % the run stops and the evidence log names what is left.

### 2.2 The worker pool

- **Shape.** `PhysicsWorkers` is a public resource in `tungsten_core::physics` holding a participant count, the calling thread included: `PhysicsWorkers::new(n)` (clamped to 1–16), `PhysicsWorkers::auto()` (half of `std::thread::available_parallelism`, at least 1, at most 6: the plateau measured on six cores and twelve threads) and `participants()`. No resource, or one participant, is today's serial step, function for function.
- **Who sets it.** `App::run` inserts it from `physics.workers` unless the game inserted one: 0, the default, is automatic; 1 is serial; n is n participants; above 64 is `ConfigError::InvalidValue`. `TUNGSTEN_PHYSICS_WORKERS` overrides the file, as the display and render keys' variables do. `App::new` inserts nothing, so `Harness` tests, the counter guards and every pin run serial unless a test inserts the resource. The insert sits outside the `script.is_none()` branch (A4).
- **Mechanism.** A pool private to `step/pool.rs`, kept in `StepExtra` behind one boxed option and started at the first fork: `participants − 1` named threads (`tungsten-physics-N`), each parked on its own `std::sync::mpsc` receiver. A fork moves the buffers its stage reads into one `ForkData`, shares it by `Arc`, sends each worker its own scratch, runs the caller's share, collects the scratches and moves the buffers back by `Arc::try_unwrap`. No `unsafe`, no dependency, no bound on a component or resource type. A worker that panics answers through a drop guard, and the step panics with its message; a thread that fails to spawn leaves the pool with fewer workers, down to the serial step. Dropping the pool closes the channels and joins the threads. The timed probe read a `std::thread::scope` fork and this pool the same (within 0.3 ms on every figure, §Proposals P1 for the spinning form); the pool is chosen because a fork's cost then does not depend on the platform's thread creation, which this machine cannot measure for Windows.
- **Splitting a stage.** Work is cut into fixed chunks claimed through one atomic counter; each participant writes into its own scratch with the chunk's index, and the caller merges by chunk index. The merged output is the serial loop's, entry for entry, whatever thread took a chunk.
- **Thresholds.** A stage forks only above a fixed size (the probe's: 1,024 querying bodies for the build, 4,096 pairs for the narrow phase, 256 listed bodies for an arrival round) and with two or more participants; below it the serial function runs. `e01-play` never forks. A crate-private constructor lets the lockstep tests fork at any size.
- **The three stages.**
  - *Build walks (Step 2).* Each querying body's candidate walk, with marks owned by the participant in place of the grid's (`SpatialGrid` and `PairGrid` gain crate-private shared walks; the public `query` is untouched). Pairs are appended per chunk and concatenated in chunk order before `sent.enter`.
  - *Arrival search (Step 3).* A round's listed bodies in chunks: each body's pairs as the serial loop builds them, except the search for a pair an earlier body of the round built, which the serial merge applies in list order.
  - *Narrow phase ahead (Step 4).* The margin and the contact of a block of pairs computed ahead into per-chunk arrays; the pair loop consumes them in pair order and does everything else as today.
- **Serial path.** The forked variants are separate `#[inline(never)]` functions; the serial functions keep their bodies. A sitting at `TUNGSTEN_PHYSICS_WORKERS=1` must read no owned metric `regressed` (A15): once after the pool's last step, whether Step 4 stays or not, and again at the close.
- **The runner.** A capture records `physics_workers_env` beside `wgpu_backend_env` (the variable's value, or `auto`), and `hard_mismatches` (`scripts/perf_stats.py`) refuses two captures whose values differ, a capture without the field reading `auto`; a test covers the mismatch.
- **Records.** One decision at Step 2's commit, under the next free ID: supersedes `D-067`, amends `D-131` (its no-worker clause and its rayon mechanism; no `Send` or `Sync` bound on components stands) and `D-220` (the thread count), and rewrites AGENTS.md's hard rule to name the step's pool beside the three background threads.

### 2.3 The serial levers

Each is exact by the argument its verifier checked (`agents/verify-physics.md`, `verify-world.md`, `verify-render.md`); estimates are the verifiers' corrected ones.

- **Carry by index (Step 5, 1.4 ms of `e01-stress`'s steady frames).** In a frame the proxies changed, map old proxy index to new through a dense table by entity index that checks the generation, and seed from the carry by index as an unchanged frame does. Entity keys are unique and carry bit 63; a pair with a tile side, whose keys can repeat, keeps the keyed fallback. A miss reads +0 both ways. Its decision amends `D-269`.
- **Sleep table by cursor (Step 5, 0.2).** `SleepTable::rebuild` walks the old and new key lists with a cursor and probes only on a miss; keys are unique, so a hit is the probe's entry.
- **Counted effects by slot (Step 6, 0.75 of `effect_tick`).** A counted instance carries its entry's slot and the list's token; `counts` checks the token and reads the slot, falling back to the ID compare. `PartialEq` and `Debug` are written by hand so a slot never enters equality.
- **Impact record cursor (Step 6, 0.10).** The hitter loop reads the record through a cursor that advances with the runner's archetype order; a hit needs the whole `Entity` and the step's own record, and a miss falls back to `get` without advancing.
- **The idle step (Step 7, 0.2 of `e01-play`'s `physics_step`).** Skip the tile supports' comparison while the tile cache's version holds; bound the sleep loops to entity proxies; keep the tile proxies' tails in place behind a scan that every tile velocity is +0; drop two fills every reader overwrites.
- **The pair loop's serial part (Step 8, 0.5 of an awake pile's frame).** Skip a union whose two bodies the build already joined (a byte per pair, cleared by every build and repair); inline the seed; walk restitution over a list of the contacts whose restitution is not 0 and whose approach is at most the negated threshold, both fixed when the contact is built, kept by every build and repair. The pass still tests each listed contact's impulse, which the solve changes (`solver.rs:241`).
- **The merged fill (Step 8, 0.4).** A merged build copies the tile refs from the kept tile grid's rows at the layout's offset in place of restaging them; the tile grid is built once a cache version; a layout the copy cannot serve takes today's path.
- **Lookups and the sort (Step 9, 0.04 of `e01-play`'s `update`, 0.06 of the stress rows' `extract`).** A multiplier in place of the slot table's shift, its search bounded by the key's length, in the same 128 slots (amends `D-083`); no `Arc` clone a query; the extract's orders biased by the frame's lowest z, so the high bytes stop varying and the sort's existing skip drops their passes (six passes to three for 01). A constant taken from every order keeps their order. An odd pass count trades the sort's two buffers, so the guards decide whether an allocation pin moves.
- **The encode (Step 10, 0.06 of `render` on the CPU-bound rows).** Bloom's debug groups inside its passes; `Load` in place of `Clear` on the seven offscreen targets a pass overwrites whole; one instance-buffer bind a sprite pass.

### 2.4 What `e01-play` can show

`e01-play`'s `total` waits on the GPU (acquire 1.16 ms of 2.90), so a CPU saving lengthens the wait and `total` holds. Its targets are `update`, `physics_step` and `extract` p50. What lowers its `total` changes pixels or needs a dependency: Proposals P3 to P5.

## 3. Steps

### Step 1: the `e01-play` row (landed)

- **Files:** as listed. **Commit:** `95939269` `test(bench): add the e01-play row, its guard and the script freeze`.
- **Done-when (passed):** `just check` → exit 0; `just perf run e01-play` → valid, one digest in five runs; the guard `play` holds its pin.

### Step 2: the worker pool and the pair build's walks

- **Files:** as listed for Step 2.
- **Change:** `PhysicsWorkers`, `physics.workers` and its variable, `App::run`'s insert; `step/pool.rs`; the build's forked walk and merge; the lockstep tests; the runner's `physics_workers_env`; the decision and its markers, AGENTS.md's rule, DESIGN.md's threading text, the config key in the getting-started guide, ROADMAP.md's Later line removed, benchmarks.md's lever line rewritten; `status: in progress`.
- **Done-when:**
  - `cargo test -p tungsten-core workers` → exit 0: a pile, a blast, a layered scene with one-way platforms, sensors and kinematic movers, and a churn of spawns and despawns each step bit-equal (every body's position, velocity and sleep state, every collision event) to the serial step at 2, 3, 4, 6 and 12 participants forced below the thresholds.
  - `cargo test -p tungsten-core --test integration physics_determinism` → exit 0, its pinned hash held with `PhysicsWorkers` at 2, 6 and 12.
  - `just check` → exit 0, no guard pin rewritten; `just api-check` → exit 0; `just repo-check` → exit 0; `just perf-test` → exit 0.
  - The sitting → exit 0; `judged.py` → exit 0; `stills.py --base-again` → `9 of 9 equal` (the control: the base against itself), then `stills.py` → `9 of 9 equal`.
  - `step_delta.py <sitting> - e01-pit system.physics_step:p50 e01-stress system.physics_step:p50 blast system.physics_step:p50` → each falls by at least 1.2 ms (probe: the walks 2.98 → 1.03 ms in `e01-pit`, 3.37 → 1.18 in `e01-stress`).
- **Commit:** `perf(physics): fork the pair build's walks on a worker pool`
- **Moves:** the thread rule (`D-067`, `D-131`, `D-220`, AGENTS.md), by the decision. No pin: the harness stays serial and the pool's state is one boxed option in `StepExtra`; a pin that reads `StepExtra`'s size moves by 8 bytes, named in the log.

### Step 3: the arrival round's search on the pool

- **Files:** as listed for Step 3.
- **Change:** the round's forked search and its serial merge; the tests' scenes gain a wake wave that lists more than 256 bodies.
- **Done-when:** `cargo test -p tungsten-core workers` → exit 0; `just check` → exit 0; the sitting → exit 0; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 2's> e01-pit stage.total:p95 e01-pit stage.total:p99 e01-stress stage.total:p95` → `e01-pit`'s p95 and p99 each fall by at least 3.0 ms and `e01-stress`'s p95 by at least 2.0 (probe: the search 2.58 → 0.95 ms a frame in `e01-pit`, in its wave frames; 2.81 → 1.07 in `e01-stress`).
- **Commit:** `perf(physics): fork an arrival round's search on the worker pool`
- **Moves:** none.

### Step 4: the pair loop's narrow phase ahead, and the one-worker sitting

- **Files:** as listed for Step 4.
- **Change:** the forked prepass and the pair loop's consuming variant; then the serial path's sitting.
- **Done-when:**
  - `cargo test -p tungsten-core workers` → exit 0; `just check` → exit 0; the sitting → exit 0; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`.
  - `step_delta.py <sitting> <Step 3's> e01-pit system.physics_step:p50 e01-stress system.physics_step:p50` → each falls by at least 0.7 ms (probe: 1.0–1.7 by subtraction). A miss after one revision reverts this step's commit; Steps 2 and 3 stand.
  - Whether this step's commit stays or is reverted, on the pool as it then stands: `TUNGSTEN_PHYSICS_WORKERS=1 WGPU_BACKEND=vulkan just perf sitting --base 95939269 --cpu-only --wait-quiet 900 --rows physics,physics-sparse,friction,slopes,terrain,rooms,blast,e01-pit,e01-stress,e01-play` → exit 0: the serial path reads no owned metric `regressed`. A `regressed` metric there is fixed before Step 5, or the pool's steps are reverted.
- **Commit:** `perf(physics): compute the pair loop's contacts ahead on the worker pool`
- **Moves:** none.

### Step 5: the carry remapped by index, the sleep table by cursor

- **Files:** as listed for Step 5.
- **Change:** two commits, the carry first. The reference model (`step_oracle.rs`) keeps its map by key and compares every warm start through a changed heap: a body leaving, one coming, an archetype move that reorders, a cut tile, a repeated tile key.
- **Done-when:** `cargo test -p tungsten-core physics` → exit 0; `just check` → exit 0 with any moved pin named; `just repo-check` → exit 0; the sitting → exit 0; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 4's> e01-stress stage.total:p50 e01-stress system.physics_step:p50` → each falls by at least 1.0 ms.
- **Commits:** `perf(physics): remap the warm-start carry by index when proxies change`; `perf(physics): rebuild the sleep table by cursor`
- **Moves:** `D-269`'s keyed frame, by the carry's decision; the allocation pins a remap table or a dropped map moves.

### Step 6: counted effects by slot, the impact record's cursor

- **Files:** as listed for Step 6.
- **Change:** two commits. `KIT_RECIPES_HASH` and every kit recipe's digest hold.
- **Done-when:** `cargo test -p tungsten-kit` → exit 0; `cargo test -p tungsten kit_recipes` → exit 0; `just check` → exit 0 with any moved pin named; the sitting → exit 0, `spread` among its rows; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 5's> e01-stress system.effect_tick:p50 e01-stress system.impact_damage:p50` → `effect_tick` falls by at least 0.5 ms and `impact_damage` by at least 0.05.
- **Commits:** `perf(kit): find a counted effect's entry by slot`; `perf(kit): read the impact record through a cursor`
- **Moves:** an effect instance's size, if the slot and token grow it: the pins that count its bytes.

### Step 7: the idle step

- **Files:** as listed for Step 7.
- **Change:** one commit a lever: the tile supports' skip, the sleep loops' bound, the tile tails kept, the two dropped fills.
- **Done-when:** `cargo test -p tungsten-core physics` → exit 0, with a test that edits a tile at runtime and one that reloads a map between steps, each bit-equal to a world that never skipped; `just check` → exit 0; the sitting → exit 0; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 6's> e01-play system.physics_step:p50 e01-play stage.update:p50` → `physics_step` falls by at least 0.12 ms and `update` by at least 0.10.
- **Commits:** `perf(physics): skip the tile supports while the tile cache holds`; `perf(physics): bound the sleep loops to entity proxies`; `perf(physics): keep the tile proxies' tails between steps`; `perf(physics): drop two pair fills every reader overwrites`
- **Moves:** the pins a kept buffer moves.

### Step 8: the pair loop's serial part and the merged fill

- **Files:** as listed for Step 8.
- **Change:** one commit a lever: the union skip, the inlined seed, the restitution list, then the merged fill from the kept tile grid. The fill's tests cover a body above the map's top row, a map edited at runtime and a world with two tilemaps.
- **Done-when:** `cargo test -p tungsten-core physics` → exit 0; `just check` → exit 0 with any moved pin named; the sitting → exit 0; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 7's> e01-pit system.physics_step:p50 blast system.physics_step:p50` → each falls by at least 0.4 ms. The merged fill is judged apart when the first three miss: it stays only if it keeps the target with them reverted.
- **Commits:** `perf(physics): skip a union the build already made`; `perf(physics): inline the pair loop's seed`; `perf(physics): walk restitution over a list`; `perf(physics): copy a merged build's tile refs from the tile grid`
- **Moves:** the pins a new per-pair byte or list moves; 02's guards whose worlds build merged with tiles.

### Step 9: long archetype keys, a query's match and the extract's sort

- **Files:** as listed for Step 9.
- **Change:** one commit a lever: the slot table's multiplier with its decision, the query's match, the biased orders.
- **Done-when:** `cargo test -p tungsten-core ecs` → exit 0; `just check` → exit 0 with any moved pin named; `just repo-check` → exit 0; the sitting → exit 0 (`ecs` and `churn` among its rows); `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 8's> e01-play stage.update:p50 e01-stress stage.extract:p50 e01-pit stage.extract:p50` → `update` falls by at least 0.03 ms and each `extract` by at least 0.04.
- **Commits:** `perf(ecs): separate long archetype keys by a multiplier`; `perf(ecs): match a query without cloning its bits`; `perf(extract): bias the sort's orders by the frame's lowest z`
- **Moves:** `D-083`'s shift and its fallback test, by the slot table's decision; an archetype's size must stay 264 bytes.

### Step 10: the renderer's encode

- **Files:** as listed for Step 10. Read `crates/tungsten-render/AGENTS.md` first.
- **Change:** one commit a lever: the debug groups, the `Load` targets, the single bind.
- **Done-when:** `just check` → exit 0; `just smoke` → exit 0; `TUNGSTEN_VISUAL_REGRESSION=1 just visual` → exit 0 with no reference image rewritten; `just perf counters` → exit 0, no pin moved; the full sitting → exit 0; `judged.py` → exit 0; `stills.py` → `9 of 9 equal`; `step_delta.py <sitting> <Step 9's> e01-pit stage.render:p50 e01-stress stage.render:p50` → each falls by at least 0.03 ms, and on no `e01-areas` row does `gpu.render_span` p50, `gpu.post0_bloom_threshold` p50 or `gpu.post0_bloom_composite` p50 read `regressed`.
- **Commits:** `perf(render): open bloom's debug groups inside its passes`; `perf(render): load the bloom targets a pass overwrites`; `perf(render): bind the sprite instance buffer once a pass`
- **Moves:** none: `counters::draw` counts draws, and no test pins a debug group.

### Step 11: the proposals measured

- **Files:** this plan's Proposals.
- **Change:** by hand, alone on the machine, in scratch exports under `target/engine-perf/`: P2 (`perf-runs/20261110-cpu-stress/step-6/revision.patch` ported onto the tree, `e01-pit`, `e01-stress` and `blast` three runs a side, the tests it fails named); P3 (`TUNGSTEN_RENDER_BLOOM_MAX_MIPS` at 4 and 3 against the default on two `e01-areas` rows: `gpu.render_span` p50, the sum of the `gpu.post0_bloom_*` passes' means, and the still's changed pixels counted); P4 (the vignette off, its pass's GPU time as the fold's bound); P5 (`e01-play` with KWin's compositing suspended for the run and resumed after it, p95 and p99). Each proposal gets its numbers and a draft decision.
- **Done-when:** every proposal in Proposals reads measured with its path, or estimated with its source; no file in the tree changed (`git status --short` → empty but this plan).
- **Commit:** `docs(plans): record the engine perf pass's measured proposals`
- **Moves:** none.

### Step 12: Close

- **Files:** as listed for Step 12.
- **Change:** the full sitting against `95939269`, the last step's when no code commit followed it; a `regressed` owned metric gets a fix or a known-issues row with the sitting's path; benchmarks.md's row sections and levers take the new reference numbers; each proposal and each open follow-up moves to benchmarks.md's levers or ROADMAP.md's Later; the plan and its citations deleted; the item out of Now.
- **Done-when:** `WGPU_BACKEND=vulkan just perf sitting --base 95939269 --wait-quiet 900` → exit 0, or each `regressed` metric fixed or recorded; Step 4's one-worker sitting command again, on the final tree → exit 0, or each `regressed` metric fixed or recorded; `just check` → exit 0; `just repo-check` → exit 0; example 01's three rows run with one digest each, equal to the base's.
- **Ends with:** the commits, the evidence, what the owner should try, the next prompt.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The automatic worker count on a machine unlike the reference one. | Half the logical CPUs, at least 1, at most 6; `physics.workers` overrides it. |
| Q2 | Whether workers spin between forks (P1). | Parked: the owner named scopes and parked threads, and spinning burns up to five cores between forks. |
| Q3 | A step's own move reads inside the noise of two sittings. | The step keeps its commits when its sitting reads nothing `regressed` and its tests hold, and the log records the target as not shown. |

## 5. Invariants

- **May move:** the thread rule (Step 2, by decision); `D-269`'s keyed frame (Step 5); `D-083`'s shift (Step 9); allocation and byte pins a step's Moves line names, their new values in the evidence log.
- **Must not move:** every row's digest; the determinism and containment hashes; `KIT_RECIPES_HASH`; every reference image and the nine stills; the GPU counter pins; an archetype's 264 bytes; the public behaviour of `SpatialGrid::query`, a repeated ID returned once included; the serial path's owned metrics at one worker.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it. Steps 3 and 4 depend on Step 2; no other step depends on another.
- A lever cannot stay exact (a digest, a hash or a still moves), or needs `unsafe`: it stops and goes to Proposals with what was measured.
- A target missed: one revision, then the lever's commit is reverted by a commit of its own, unless Q3's default applies.
- A sitting reads a row `unresolved` or `invalid`: it is taken once more for those rows; a second failure stops the step.
- The meter's `chain:` line reads 35 %: stop, record what is left.
- The pool's Step 2 misses its target after one revision: Steps 3 and 4 stop with it and the pool goes to Proposals.

## 7. Critique

gpt-6-astra through the critique skill, 2026-11-23, the working tree at `95939269` with this plan uncommitted, round 1 of 1: the owner's request names one round, so the changed checks below went unreviewed. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 409.3 s, tokens in 1262288 (cached 1162624), out 9920 (reasoning 4187), commands 33, output 354 KB (largest 46.4 KB), usage 5h 0% → 27%, week 0% → 4%`.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | HIGH: the restitution predicate includes `contact.impulse <= 0.0`, which the solve changes before restitution runs, so it cannot move whole to the build. | `step/solver.rs:241`; `agents/verify-physics.md`'s `REST-1` | Right | §2.3: the list keeps the contacts by restitution and approach only; the pass still tests the impulse. |
| 2 | HIGH: Step 9's skip of constant-byte radix passes already exists. | `crates/tungsten/src/sprite_extract.rs:1217`, `:1399`; `agents/verify-world.md`'s `SORT-1` | Right: the plan had lost the lever's bias | A13, §2.3 and Step 9 name the lever as the verifier checked it: orders biased by the frame's lowest z, the existing skip then dropping three passes. |
| 3 | MEDIUM: the only one-worker sitting sits in Step 4, which may be reverted, and later physics steps and the close run at automatic workers. | Step 4, §6, Step 12 | Right | The one-worker sitting runs whether Step 4 stays or not, and again at the close. |
| 4 | MEDIUM: `wgpu_backend_env` is recorded, not compared, so following it does not refuse two worker settings. | `scripts/perf.py:224`, `scripts/perf_stats.py:422-433` | Right | §2.2: `hard_mismatches` compares the field, with a test; `scripts/perf_stats.py` joins Step 2's files. |
| 5 | MEDIUM: no metric is named `gpu.post`. | `scripts/perf-rows.json`'s `e01-area-*` rows; a capture's `gpu.passes` keys | Right | Step 10 judges `gpu.render_span`, `gpu.post0_bloom_threshold` and `gpu.post0_bloom_composite`; Step 11 sums the `post0_bloom_*` passes. |
| 6 | MEDIUM: Q4's default let a still pass within `just visual`'s tolerance of 2. | `examples/02_bench/tests/visual_regression.rs:87` | Right; moot | Q4 is gone: the base against itself read 9 of 9 byte-equal, so the stills compare bytes only. |
| 7 | MEDIUM: no prescribed command ran the stills' base control. | `perf-runs/20261123-engine-perf/tools/stills.py` | Right | Step 2's done-when runs `stills.py --base-again` first; it read 9 of 9 on 2026-11-23. |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-23 | landed | `e01-play` `total` 2.90 / 4.61 / 4.97 ms, `update` p50 0.88, `physics_step` p50 0.31, `extract` p50 0.12; guard `play` 828,966 allocations | `95939269`, `perf-runs/20261123T054234Z-e01-play` |
| gate | 2026-11-23 | passed | six participants: `e01-pit` `total` p95 −8.9, p99 −10.2 ms; `e01-stress` p95 −7.4, p99 −9.5; digests equal at every count | `perf-runs/20261123-engine-perf/probe/timed-means.txt` |

## Follow-ups

Exact levers left out, each under the sitting's reach or the pass's budget (estimates from `agents/verify-*.md`):

- The walk's reference-cell dedupe and branchless acceptance (0.85 + 0.35 ms serial, about a third of it under the pool), with a per-proxy layer class (0.2); the dedupe needs a grid that proves its IDs distinct, since `SpatialGrid::query` returns a repeated ID once.
- The arrival pass's depth pass over a compact array (0.6 ms of a wave frame); the repair's walks, the fill, the staging and the repair's compaction on the pool (0.15–0.3 each); the pair loop consuming on the pool (0.6).
- Sensors by a kept grid and a cheaper zone sort (0.1 + 0.15); an incremental asleep build (0.2, the mean only).
- The kit's and frame loop's levers under 0.025 ms, by their names in the reviews: `KEY-1` (the save-order sorts over keys read once, twelve sort sites), `CHEM-1` (chemistry's rule snapshot reused), `STATS-1`, `IMPACT-2`, `ANIM-1`, `ANIM-2`, `CHAR-1`, `CONTACT-1`, `PART-1`, `TILE-1`, `ALLOC-1`, `RES-1`, `SCHED-1`, `QUERY-2`, `QUERY-3`; `ImpactRecord::keep`'s double write; `write_timing`'s formatting, which only a perf log pays.
- The typed save load and a capture that compares before it builds (0.55 + 0.35 ms a save frame, no percentile).
- The overlay in the last full-screen pass, the scene depth discarded, the lit fragment's squared-distance rejection, the frame-level debug groups (0.01 each or unmeasured).

## Proposals

Levers that change motion or pixels, need `unsafe` or a dependency, or trade something no row reads. None lands in this pass; each needs the owner's decision.

| # | Proposal | Numbers | State | Draft decision |
| --- | --- | --- | --- | --- |
| P1 | The pool's workers spin up to 3 ms for the next fork before they park. | Against the parked pool, `total` p50 / p95 / p99: `e01-pit` −0.24 / −0.56 / −0.86 ms, `e01-stress` −0.38 / −0.88 / −1.00, `blast` −0.51 / −0.84 / −1.17; up to five cores busy between forks. | Measured: `probe/timed-means.txt` | "The step's workers spin for `physics.worker_spin_ms` before parking; default 0." |
| P2 | An arrival pass that clamps a listed body against every pair partner in reach (ROADMAP.md, Later). | Step 11 | To measure on the pooled tree | Step 11 |
| P3 | A shorter bloom chain. | Step 11 | To measure | Step 11 |
| P4 | The vignette folded into bloom's composite. | Reviewer's estimate −0.25 ms of `render_span` at 1080p, −0.95 at 2160p; Step 11 bounds it. | To measure | Step 11 |
| P5 | A compositor-bypass hint on the window (`_NET_WM_BYPASS_COMPOSITOR`), which needs an X11 dependency or FFI. | Reviewer's estimate: `e01-play` `total` p95 4.59 → about 3.3 ms, p99 4.99 → about 3.5; Step 11 measures it. | To measure | Step 11 |
| P6 | Saves written off the frame, on a writer thread. | 7–16 ms a save frame in `e01-play` and 394 ms once, the first save's fsync; no percentile reads them. | Estimated: `phase-a/pack/rows/play.timeline.txt`, `agents/frame.md` | "Autosaves serialize on the frame and write on a fourth background thread." |
| P7 | The pair list kept across frames. | About 2.5 ms of an awake pile's serial frame, less under the pool; pair order is the solver's order, so motion changes. | Estimated: `agents/physics-pairs.md` | — |
| P8 | `CollisionEvent` at 48 bytes. | 0.05 ms; breaks the public struct. | Estimated: `agents/verify-physics.md` | — |
| P9 | One submit a frame when the acquire does not wait. | 0.025 ms; the frame's submit structure would depend on timing (amends `D-145`). | Estimated: `agents/verify-render.md` | — |
