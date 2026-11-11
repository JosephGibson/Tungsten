# CPU stress pass (cpu-stress) — in progress

- **status:** in progress
- **goal:** lower `total` p95 and p99 in the frames that load many systems at once (01's full pit under blasts, a pile's wake, fire, impacts and enemies), measured on `e01-stress` and `e01-pit`, and take the cheap general speed-ups the measurement found on the other CPU rows; single-threaded, in `tungsten-core`, `tungsten-kit` and the umbrella crate
- **non-goals:** 7.14's list (it stays after 7.13; this pass only marks what it closes, §2.6); any change to example 01 beyond the hooks its row has; a worker thread in any landed change (`D-067`, `D-131`); collision once a frame, a staggered or partial island wake, the resting query index (C1), the extract (C6), the flush's batching and chemistry's emission phase, each dropped at the gate with its reason (§2.1)
- **files to touch:** by step, nothing outside these lists
  - Step 1: `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`
  - Step 2: `crates/tungsten-core/src/physics/broadphase.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/tests/physics/broadphase.rs`, `crates/tungsten-core/benches/physics_bench.rs`
  - Step 3: `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/solver.rs`, `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/benches/physics_bench.rs`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`
  - Step 4: `crates/tungsten-core/src/ecs/event_queue.rs`, `crates/tungsten-core/src/tests/ecs/event_queue.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/benches/ecs_bench.rs`, `api/tungsten-core.txt`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten/src/tests/guards/*.json`
  - Step 5: `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, `crates/tungsten-core/src/physics/events.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `docs/perf/benchmarks.md`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`
  - Step 6: `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, `docs/plans/phase-7.md`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten-core/tests/integration/physics_determinism.rs`, `crates/tungsten-core/tests/integration/physics_containment.rs`, `crates/tungsten/src/tests/kit_recipes/determinism.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`
  - Step 7: `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, `docs/plans/phase-7.md`, `docs/known-issues.md`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten-core/tests/integration/physics_determinism.rs`, `crates/tungsten-core/tests/integration/physics_containment.rs`, `crates/tungsten/src/tests/kit_recipes/determinism.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`
  - Step 8: `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `examples/02_bench/src/tests/guards/*.json`, `examples/01_platformer/tests/game/guards/pit.json`
  - Step 9: `crates/tungsten-core/src/ecs/resource.rs`, `crates/tungsten-core/src/ecs/mod.rs`, `crates/tungsten-core/src/ecs/world.rs`, `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/benches/ecs_bench.rs`
  - Step 10: `crates/tungsten-kit/src/damage/effects.rs`, `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-kit/src/tests/effects.rs`, `crates/tungsten-kit/benches/kit.rs`
  - Step 11: `crates/tungsten-kit/src/present/animation.rs`, `crates/tungsten-core/src/assets/animation.rs`, `crates/tungsten-core/src/tests/assets/animation.rs`, `crates/tungsten-kit/src/tests/animation.rs`, `crates/tungsten-kit/benches/kit.rs`, `api/tungsten-core.txt`
  - Step 12: `crates/tungsten-kit/src/chemistry/queue.rs`, `crates/tungsten-kit/src/chemistry/stimuli.rs`, `crates/tungsten-kit/src/chemistry/tiles.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`, `crates/tungsten-kit/benches/kit.rs`, `examples/02_bench/src/tests/guards/spread.json`
  - Step 13: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`
- **ordered steps:** (1) the arrival pass's search and sort, bit for bit; (2) the pair grid's walk by cell runs; (3) warm starts seeded by proxy index; (4) a step's events handed over in one append; (5) one collision event a pair a step; (6) the arrival pass's candidates from the pair list; (7) the pair list kept across frames; (8) the support record compared without a full sort; (9) `TypeId` maps on the pass-through hasher; (10) one definition resolve an effect instance; (11) `animation_tick`'s one clip resolve a row; (12) chemistry's queue index and tile gate; (13) close
- **done-when:** every step is done, dropped at its probe or stopped, each quoted in the evidence log; the close sitting shows `e01-stress`'s and `e01-pit`'s `total` p95 and p99 lower, in ms, with no other CPU row and no GPU row `regressed`; `just check`, `just physics-release`, `just perf-test` and `just repo-check` pass; the evidence sits under `perf-runs/20261110-cpu-stress/`; the plan deleted with its citations and the item out of Now

Item `cpu-stress` in [ROADMAP.md](../../ROADMAP.md), from the owner's request of 2026-11-10: an off-roadmap pass before 7.13. Written 2026-11-10 at `5809023b`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md), the [tungsten-perf skill](../../.claude/skills/tungsten-perf/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

The pass sits between 7.12 and 7.13 on the one track. Its plan session added two rows (`c5c69d1e`): `e01-stress`, 01's full pit under scripted blasts, lightning, pushes, iron and a black hole, and `blast`, the same pile in example 02 under the engine's own radial impulse, with a counter guard. It then took a baseline sitting against v0.116.0, scanned the fourteen physics commits since the tag, profiled the tail frames by state, ran four read-only agents and a scratch thread experiment. Everything measured is under `perf-runs/20261110-cpu-stress/` (machine-local; its README maps it).

Three things shape the steps. First, `physics_step` is 57 % of the stress row's 20 highest frames, and inside it the arrival pass (`D-092`) and the pair build (`D-075`, `D-081`) lead; 01's own event readers hold another 29 %, which only the engine's event volume can lower, since 7.13 rewrites those systems. Second, core hardening (7.21) cost the physics rows 4 to 8 % and `terrain` 36 % since the tag, by `D-251` and `D-254`. Third, the two leading stages run bit-exact on a caller-owned rayon pool and pay 7 to 11 ms on the tail at six workers, which is the owner's decision to take, not this pass's (§2.5).

Steps 1 to 4 and 8 to 12 change no behaviour. Step 5 changes what a `CollisionEvent` promises; steps 6 and 7 move the physics hashes, which Phase 7's §5 allows only by a recorded decision. 7.14 keeps its list: this pass marks what it closes there.

## 1. Audit

Read and measured at `c5c69d1e`. Profile numbers are ms a frame under the profiler (25 kHz, about 10 % over a capture's).

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Baseline: `e01-stress` `total` p50 55.20, p95 63.68, p99 67.59 ms; `blast` `physics_step` p50 26.86, p95 40.81, p99 46.51 ms; `e01-pit` `total` p50 34.74, p95 47.68, p99 53.27 ms (five runs each) | `perf-runs/20261110T211242Z-sitting/`, `baseline/sitting-table.txt` |
| A2 | Against v0.116.0, `physics_step` p50 reads `regressed` on `physics` (6.32 → 6.85 ms), `physics-sparse` (3.23 → 3.39), `terrain` (4.71 → 6.34), `friction` (6.73 → 7.13) and `slopes` (2.17 → 2.26); every other CPU row reads `ok` | the same sitting |
| A3 | `e01-pit` at the tag, from three valid runs (an encoder invalidated the capture): `total` p95 36.4 to 36.9, p99 40.8 to 41.1 ms. The tree's wave wakes 12,000 sleepers in 21 frames and twice in the window, the tag's in 29, once | `captures/e01-pit-A1-2/`, `baseline/e01-pit-split.txt` |
| A4 | `1427b84f` (`D-251`) brought `physics` 6.39 → 6.90 ms and `friction` 6.82 → 7.26: the build's per-candidate closure is no longer inlined. `faacb2d4` (`D-254`) brought `terrain` 4.71 → 6.39 and the pit's steeper wave. `513e78ff` (`D-261`) brought `physics` 6.90 → 7.03 | `scan.txt`; `profile/pit-top20.fold.txt` against `pit-base-top20.fold.txt` |
| A5 | `e01-stress`'s 20 highest frames, 72.1 ms: `physics_step` 41.3 (57.3 %), 01's `spread_fire` 12.84, `crush` 3.85, `flame_exposure` 3.19, `body_impacts` 2.80, `friction` 1.27, the kit's `animation_tick` 1.05, render 1.09, extract 0.60 | `profile/stress-top20.fold.txt` |
| A6 | Inside that `physics_step`: `arrival_pass` 9.38, `build_pairs` 7.62, `narrow_phase` 3.62, `solve_contacts` 3.38, `ImpulseMap::get` 2.94, the substep's own loop 2.56, `sensor_pass` 1.69, `refresh_pairs` 1.48, contact and event pushes 1.31, the event drain 1.17 and `EventQueue::send` 0.82, `sync_impulses` 0.59 | the same fold |
| A7 | By state, `physics_step` and its leaders: asleep 2.76 (`sensor_pass` 1.42); the wake's two frames 38.4 (`build_pairs` 8.60, arrival 5.53); the ten frames after 50.3 (arrival 15.61, build 8.14); steady awake 29.8 (build 6.92, solve 3.19, `ImpulseMap::get` 3.17, narrow 3.01, arrival 0.86) | `profile/stress-{asleep,wake,first,steady}.fold.txt` |
| A8 | `blast` agrees without a game beside it: its 20 highest 46.3 (arrival 14.18, build 7.17, narrow 4.10, solve 3.77, `ImpulseMap::get` 3.63); steady 26.2 (build 6.15, solve 3.46, `ImpulseMap::get` 3.26, pushes 1.39, drain 1.27, send 0.79). One impulse wakes the island whole: 0.7 ms to 25 in a frame. The step sends about 200,000 `CollisionEvent`s a frame, one a penetrating contact a substep | `profile/blast-*.fold.txt`; the row's `bench.events` |
| A9 | The arrival pass on the pit's 20 highest, 16.46: its own loop 5.06, the grid walk 2.73, `built_pair`'s search 2.04, `mover` 1.23, the stable sort and its scratch 1.08, `narrow_phase_at` 0.50. A listed body walks about 60 candidates for about 6 neighbours at 64 px cells | `profile/pit-top20.fold.txt`; `agents/physics.md` 1 |
| A10 | The pair list is rebuilt every frame (`pairs_invalidated = true`), and each build seeds every contact's warm start through a keyed map | `crates/tungsten-core/src/physics/step/mod.rs:1020`, `:1426-1430`; `step/pairs.rs:168-182` |
| A11 | `terrain`: sorting the support record is 29 % of its frame; any tile edit shifts the key sequence and sorts every 56-byte support | `profile/rows/row-terrain.data`; `crates/tungsten-core/src/physics/step/sleep.rs:329-406` |
| A12 | `spread`: `hash_one::<&TypeId>` 17.1 %, `DataRegistry::kind_for` 12.0 %, `effects::definition` 11.1 %, SipHash `write` 7.0 %; resources and registry kinds hash `TypeId` with SipHash though `TypeIdMap` exists; `chemistry` p95 10.16 ms, every receiver emitting on one frame in fifteen | `profile/rows/row-spread.data`; `crates/tungsten-core/src/ecs/resource.rs:6`, `ecs/archetype.rs:10`; `agents/kit.md` |
| A13 | `animation_tick` looks a clip up by string twice a sprite a frame, asleep or off screen: 0.69 of a 6.7 ms asleep frame | `crates/tungsten-kit/src/present/animation.rs:160-169`; `profile/stress-asleep.fold.txt` |
| A14 | 01's four event readers key their work by pair or by entity and read speeds from `PreviousVelocity`, not from the event's `speed`; `crush` and `ball_impacts` project that speed on each event's normal; the kit reads no `CollisionEvent` | `examples/01_platformer/src/features/elements/systems.rs:113`, `:538`, `:676`, `:1262`; `features/toys/systems.rs:880` |

## 2. Design

### 2.1 The gate

Ranked by share of `e01-stress`'s 20 highest frames (A5, A6), which hold its p95 and p99 frames. An item under 5 % is dropped unless it is a cheap general win; a kept item's target and probe are its step's.

| # | Item | ms | Share | Verdict |
| --- | --- | --- | --- | --- |
| G1 | 01's event readers (`spread_fire`, `crush`, `body_impacts`, `friction`), each walking every event | 20.76 | 28.8 % | kept as the engine's event volume: Step 5; 01 untouched |
| G2 | The arrival pass | 9.38 | 13.0 % (pit 30.0 %, `blast` 29.8 %) | kept: Steps 1 and 6 |
| G3 | The pair build | 7.62 | 10.6 % (steady 11.3 %, `physics` 29.5 %) | kept: Steps 2 and 7 |
| G4 | The narrow phase | 3.62 | 5.0 % | stopped, the work itself: a circle test a pair a substep; collision once a frame contradicts `D-063`, `D-064` and `D-075` (Follow-ups) |
| G5 | Keyed warm starts (`ImpulseMap::get`, `sync_impulses`) | 3.53 | 4.9 % (`blast` tail 9.1 %) | kept, cheap and bit-exact on every awake frame: Step 3 |
| G6 | `solve_contacts` | 3.38 | 4.7 % | dropped; Step 3's smaller contact is its only cut |
| G7 | Collision events in the step (pushes, drain, send) | 3.30 | 4.6 % (`blast` steady 13.2 % of the step) | kept, cheap and bit-exact: Step 4 |
| G8 | 01's `flame_exposure` | 3.19 | 4.4 % | 01's own: 7.13 |
| G9 | The substep's own loop | 2.56 | 3.6 % | dropped |
| G10 | `sensor_pass` and the query index's refresh | 1.69 | 2.3 % (51 % of the asleep step) | dropped, not cheap: C1 stays 7.14's, with A7 |
| G11 | `refresh_pairs`, the sleep passes | 2.48 | 3.4 % | dropped |
| G12 | `World::get` under 01's readers | about 1.2 | 1.7 % | dropped: its callers are 01's (C4) |
| G13 | `animation_tick` | 1.05 | 1.5 % (10 % asleep) | kept, cheap, every animated sprite: Step 11 |
| G14 | The island's wake in one frame | none | not a tail frame | dropped: a wake by reach waits on two known issues and every engine surveyed wakes an island whole (`agents/research.md`; Follow-ups) |

The other CPU rows (goal 2), from A2, A4, A11 and A12:

| Row | Item | Verdict |
| --- | --- | --- |
| `physics`, `physics-sparse`, `friction`, `slopes` | `D-251`'s closure, out of line since `1427b84f` | Step 2 |
| `terrain` | `D-254`'s support sort | Step 8 |
| `spread` | `TypeId` hashing; four definition resolves an instance; the queue's index and the tile probe | Steps 9, 10, 12; the emission's phase moves the kit's pin and behaviour: 7.14's C2 |
| `gpu-throughput`, `particles` | the extract (87 % and 33 %) | dropped: its split by pass is unmeasured; C6 stays 7.14's |
| `churn` | the flush's per-column moves | dropped: the bit-exact part is a few percent; C4 |
| `saves`, `rooms`, `ecs`, `boot`, `generate`, `template`, `gpu` | none found that is cheap | none |

### 2.2 How a step is judged

- **Probe first.** Each step lands its probe (a `#[cfg(test)]` counter with a test that prints it, or a criterion pair) and quotes its reading before the fix; steps 6 and 7 gate on it.
- **Step sitting.** With the step's change in the tree, uncommitted, and nothing beside it: `WGPU_BACKEND=vulkan just perf sitting --base HEAD --rows <the step's rows> --cpu-only --wait-quiet 600`. The owner's NoMachine client stays disconnected.
- **Target met** when `just perf compare` of the sitting's A1 and B1 captures and of its A2 and B2 each print a fall of at least the target on the step's metric, and the sitting reads no row `regressed`. A step that claims bit for bit also reads "digests match" on its physics rows in both compares.
- **Targets** are about 60 % of the profile's estimate, so a met target is a real effect and a missed one is a wrong mechanism.
- `blast` judges the physics steps: it replays 01's pile with no game system beside it, so a sitting takes about ten minutes. `e01-stress` judges steps 5 and 11; the close sits everything.
- New step state goes in the boxed `StepExtra`, never inline in `PhysicsBuffers`, whose size every guard's `allocated_bytes` holds.
- A guard pin or a pinned hash is in a step's list where the step may move it, and the step's Moves line says which may. In the steps, "the physics guards" are 02's `physics`, `physics-sparse`, `friction`, `slopes`, `terrain` and `blast` pins and 01's `pit.json`; "the hashes" are the determinism and containment tests' `EXPECTED` values and `KIT_RECIPES_HASH`.
- Before each step: `just preflight docs/plans/cpu-stress-pass.md --step N`; a hit outside the step's list is read before editing.

### 2.3 The fixes

- **Step 1** keeps every pair, clamp and proxy bit. `built_pair` reads a recorded range of each listed body's pairs instead of a binary search; `mover` becomes a bit set at the pair's build for the depth pass and the sort alone, which run before any velocity changes, while the backward sweep still recomputes it from the swept velocities; the stable sort becomes an unstable one on `(depth, target_inv's inverted bits, sequence)`, the same order without its scratch; the candidate cut reads a 24-byte array of start centre, half extents and velocity instead of the 80-byte `Proxy`.
- **Step 2** hands a query's visitor a cell's run of ids and loops inside it, for the build, the repair and the arrival pass, so the closure's inlining stops deciding the cost; the oversize loop moves to a `#[cold]` function. Order stays `D-080`'s.
- **Step 3**, while the body sequence and the tile version hold, groups last frame's contacts by lower proxy index and seeds each new contact from that short run; the keyed map stays for a sequence change. `ContactConstraint` then drops its 16-byte key, 80 → 48 bytes. Values stay `D-076`'s.
- **Step 4** adds `EventQueue::append(&mut Vec<T>)`, which swaps buffers when the queue's current one is empty and appends otherwise, and the step's five drains use it.
- **Step 5** sends a pair's `CollisionEvent` once a step, the first time either producer has one: the pair loop, in the first substep its contact penetrates, or the safety net's clamp (`step/sweep.rs`). The step keeps an emission record of its own, a bit a pair, reset at the step's start and kept through the step's builds and repairs whether or not the contact persists (the impulses forget an absent contact; this record does not). The event keeps that substep's normal, penetration, speed and tile. 01's readers key by pair (A14), but `crush` and `ball_impacts` project a speed on each event's normal, so a later, more head-on substep's reaction can be lost: the step measures what changes, and Q2 sets the bar.
- **Step 6**: a listed body whose new speed fits its remaining pair budget can reach only its pair partners, so it reads them from an adjacency built with the pair list, and only a body over budget walks the grid. Candidate order changes, so the hashes move.
- **Step 7** keeps the pair list while the gathered proxies match the last frame's: it invalidates on a body-sequence change, a tile version change, an island falling asleep, any proxy whose shape, offset or kind changed (an awake body's as a static's), a moved static, and a layers or config change; a written `Position` adds its jump to the body's travel. The side arrays a gather may leave empty (friction, gravity scales, spins) invalidate when one fills or empties, and every buffer a build sizes by the pair list (`pair_tangents` among them) is checked against it on a frame that skips the build. `D-234`'s woken path covers frame-start wakes.
- **Step 8** compares supports by key first, strips the common prefix and suffix before any sort, and takes tile changes from the cache's written cells where it can.
- **Step 9** puts `ResourceMap`, `DataRegistry::kinds_by_type` and `World::event_types` on `TypeIdMap`; none is iterated.
- **Step 10** resolves an effect's definition once an instance a tick, shared by `suppressed` and `tick_instance`, with kinds indexed by slot; the per-instance order stays.
- **Step 11** adds `AnimationState::advance_in(&mut self, dt_ms, &AnimationData)` and resolves a row's clip once, remembering the last `(id, clip)`.
- **Step 12** probes the queue's index once a candidate through a cheaper hasher or a per-run slot, and asks `has_any::<TilemapInstance>()` once a run before `cells_within`.

### 2.4 Hashes and decisions

- Steps 1 to 4 and 8 to 12 hold `physics_step_is_bit_identical_across_runs`'s `0x088e_c07a_73c1_b168`, the containment hash `0xaee2_72e0_1ffc_4e4c` and `KIT_RECIPES_HASH` `0xa2de_8a8e_2368_07d3`.
- Step 5 holds the three and records a decision amending `D-075`'s event clause (one event a pair a step).
- Steps 6 and 7 each move the determinism and containment hashes, and the kit's pin where a recipe's world runs them. Each records its decision (Step 6 amending `D-092`; Step 7 amending `D-075`, `D-076` and `D-081`) with the old and new hashes, and Step 6 amends Phase 7's §5 sentence on where the physics hashes may move, naming this pass and its decisions. The two-run assertion holds at every commit.
- Each decision takes the next free number at its commit and its index rows in the same change (the tungsten-decision skill; `cargo test -p tungsten-core --locked --test integration decision_index`).
- Step 3 adds an amending entry only if `D-076`'s Decision field names the keyed map as the carry.

### 2.5 The threads track

**What would block a worker pool later** (`agents/*.md`, their thread-readiness lists):

- **`World` is neither `Send` nor `Sync`:** resources are `Box<dyn Any>` without bounds (`ecs/resource.rs:6`); every query borrows `Archetypes::matches`, a `RefCell`, mutably (`ecs/storage.rs:23`, `:92`); systems are `Box<dyn FnMut(&mut World)>` (`schedule.rs:132`).
- **`Rc` and `RefCell` in resources and recorders:** `ExtractScratch` (`tungsten/src/sprite_extract.rs:288`), `SpatialIndex` (`physics/query.rs:284`), the kit's explain ring (`foundation/explain.rs:52`), chemistry's handlers and map names (`chemistry/outputs.rs:393`, `chemistry/tiles.rs:43`), the looks' shared state (`damage/effects.rs:4369`), `SaveRegistry` (`save/registry.rs:340`), `ComponentRegistry` (`component_registry.rs:263`), script hooks (`script.rs:1443`).
- **Order dependencies in the step:** `D-080`'s walk order fixes pair order; the pair loop writes wake flags later pairs read, mutates the union-find and pushes contacts and events in pair order (`step/mod.rs:1379-1410`, `:1528-1539`); Gauss-Seidel runs in contact order (`step/solver.rs:62-90`); the arrival sweeps are serial (`step/arrival.rs:365-456`). In the kit: effect and damage resolve, chemistry's take and its `f64` merges, cues and every `KitRng` draw.
- **Buffers shared across hot loops:** `SpatialGrid::for_each_in` takes `&mut self` for its dedupe marks and its lazy build (`broadphase.rs:188-205`, `:333-342`); one `candidates` vector serves the pair loop and the arrival pass (`step/mod.rs:1317`).
- **Pure per item today:** the narrow phase's geometry, the arrival pass's neighbour search, a build's walk per querier, `collect_tripped`, the integration loops; in the kit `animation_tick`'s loop, `tick_instance`, `physics_sync` and `physics_prev_snapshot`.

**The scratch experiment** (`threads/scratch.patch`, never committed): the arrival pass's neighbour search and the build's per-querier walks in chunks on a rayon `ThreadPool` the caller inserts as a resource, each thread with its own marks, results joined in list and proxy order. `physics_step_is_bit_identical_across_runs` reads its pinned hash at 0, 2, 6 and 12 workers, and each row's digest over its `physics:` lines is the same at every count. Two runs a count (`threads.txt`):

| Workers | `e01-stress` `total` p95 / p99 | `e01-pit` `total` p95 / p99 | `blast` `physics_step` p50 / p95 / p99 |
| --- | --- | --- | --- |
| none | 64.1 / 68.2 ms | 48.0 / 53.5 | 26.9 / 41.3 / 47.3 |
| 1 | 65.2 / 69.8 | 50.8 / 55.9 | 27.4 / 42.9 / 48.4 |
| 2 | 60.0 / 63.9 | 42.9 / 46.6 | 24.6 / 36.6 / 40.7 |
| 4 | 58.6 / 61.7 | 40.7 / 43.6 | 23.1 / 33.2 / 36.9 |
| 6 | 57.2 / 60.5 | 39.4 / 42.4 | 22.7 / 32.3 / 36.1 |
| 12 | 57.6 / 60.9 | 38.7 / 41.8 | 22.4 / 32.0 / 35.5 |

Threads pay: at six workers the stress row's p95 falls 6.9 ms and its p99 7.7, the pit's 8.6 and 11.1, from two stages alone; four workers take most of it and one costs 1 to 3 ms. Steps 1, 2, 6 and 7 shrink the same two stages, so the pool's gain after this pass is smaller and is measured again before any decision.

**For the owner:** landing it needs a decision that supersedes `D-131`'s no-worker clause and `D-067` with it, and amends `AGENTS.md`'s three-thread rule: a rayon `ThreadPool` owned by `App` (`D-131`'s own mechanism, `D-015` rule 3), handed to the physics step for its pure stages, results joined in the serial order so the hashes hold at any count. This pass records no such decision.

**No step adds a blocker:** no step adds an `Rc`, a `RefCell` or a `Cell`, query state inside a grid, or a write to shared state in a loop the list above calls pure. Step 2's run visitor takes the marks it needs as the walk does today.

### 2.6 What the pass closes in 7.14's list

At the close, by the close sitting: C3's K13 (`animation_tick` by name) when Step 11's target is met; C2 when `spread`'s `system.chemistry` p95 has fallen by half (Phase 7's Q4). C1, C4, C5 and C6 stay open, each with this pass's measurement added to its evidence cell. Nothing joins the list.

## 3. Steps

Each step: `just preflight`, the probe's reading, the change, the checks, the step sitting, then one commit of its own paths, pushed. A step's files are its line in the header's list. `just check` and `git diff --check` pass at every commit.

Order: the physics step's items by share (§2.1), each item's bit-exact fix before its hash-moving one; Step 5, the largest share, follows Step 3, whose index runs carry its flag; the other rows' items after them.

### Step 1: The arrival pass's search and sort, bit for bit

- **Probe:** counters beside `listed` and `clamps` for candidates walked, cut survivors, `built_pair` calls and pairs built; a test on a 600-body pile under a radial impulse prints them (expected: about ten candidates walked a pair built).
- **Change:** §2.3. The plan's `status` becomes `in progress` in this commit.
- **Done-when:** `just physics-release` → exit 0, its two hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0; sitting on `blast,physics,friction`: `blast` `system.physics_step` p95 and p99 each fall at least 2.5 ms in both pairs, digests match.
- **Commit:** `perf(physics): cut the arrival pass's search and sort costs`
- **Moves:** the physics guards' `allocations` and `allocated_bytes` may fall (the sort's scratch); nothing rises.

### Step 2: The pair grid's walk by cell runs

- **Probe:** criterion `spatial_grid_query_50k_dense` and a new `build_pairs_12k_pile` pair, saved as `before`; `nm -C` on the perf build lists `build_pairs::{closure#0}`.
- **Change:** §2.3.
- **Done-when:** `just physics-release` → exit 0, hashes unchanged; the criterion pair against `before` → both improve; sitting on `physics,physics-sparse,friction,slopes,blast`: `physics` `system.physics_step` p50 falls at least 0.35 ms and `blast`'s at least 0.8 ms in both pairs, digests match.
- **Commit:** `perf(physics): walk the pair grid by cell runs`
- **Moves:** none.

### Step 3: Warm starts seeded by proxy index

- **Probe:** the criterion `physics_step` group at 12,000 settled awake bodies, saved as `before`; the `reference_impulses` oracle extended to the index path, so every seeded impulse equals the keyed map's bit for bit.
- **Change:** §2.3; `DESIGN.md`'s sentence on the carry by key follows.
- **Done-when:** `just physics-release` → exit 0, hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0; sitting on `blast,physics,friction`: `blast` `system.physics_step` p50 and p95 each fall at least 1.5 ms in both pairs, digests match.
- **Commit:** `perf(physics): seed warm starts by proxy index`
- **Moves:** the physics guards' `allocated_bytes` may fall (a 48-byte contact); `D-076` by an amending entry only under §2.4's condition.

### Step 4: A step's events handed over in one append

- **Probe:** criterion `event_send_200k` against `event_append_200k` in `ecs_bench.rs`.
- **Change:** §2.3; `just api` regenerates `api/tungsten-core.txt`.
- **Done-when:** `just api-check` → exit 0; `just physics-release` → exit 0, hashes unchanged; sitting on `blast,physics`: `blast` `system.physics_step` p50 falls at least 1.2 ms in both pairs, digests match.
- **Commit:** `perf(ecs): hand a step's events to their queue in one append`
- **Moves:** any guard's `allocations` and `allocated_bytes` may move once (three buffers rotate); `events_sent` holds.

### Step 5: One collision event a pair a step

- **Probe:** a test counting distinct pairs among one step's events on a resting pile (expected: a quarter of the events); `blast.json`'s `events_sent` (271,607) before. Two tests land with the fix: a pair whose contact leaves and returns within a step sends one event, and so does a pair the safety net clamps and the pair loop then meets.
- **Change:** §2.3; the decision; `events.rs`'s and `DESIGN.md`'s sentences; `blast`'s events figure in `docs/perf/benchmarks.md`.
- **Done-when:** `cargo test -p example-01-platformer --locked` → exit 0 with no test of 01 edited; `cargo test -p tungsten --locked --lib kit_recipes::determinism` → exit 0, the pin unchanged; `just physics-release` → exit 0, hashes unchanged; `cargo test -p tungsten-core --locked --test integration decision_index` → exit 0; `pit.json`'s `spawned`, `despawned` and `archetype_moves` unchanged; sitting on `e01-stress,blast`: `e01-stress` `system.fire_spread` p50 and `stage.total` p95 each fall at least 5.0 ms in both pairs, the captures valid (their `pit` and `pit_fire` asserts pass), and the two sides' script reports, quoted in the evidence row, agree within 1 % on `pit`'s `balls` and `pit_fire`'s `burning`.
- **Commit:** `perf(physics)!: send one collision event a pair a step`
- **Moves:** the physics guards' `events_sent` and `allocated_bytes` fall; `blast`'s digest moves by its `events` count alone; `e01-stress`'s and `e01-pit`'s digests may move with 01's reactions; `D-075`'s event clause by the new decision.

### Step 6: The arrival pass's candidates from the pair list

- **Probe, and gate:** a counter of listed bodies whose new speed fits their remaining budget, on the Step 1 test's blast frames. Under half of the listed bodies: the step is dropped with the reading.
- **Change:** §2.3; the decision with the old and new hashes; Phase 7's §5 sentence.
- **Done-when:** `just physics-release` → exit 0 on the new `EXPECTED` values, containment's `escaped` 0 and every tunnelling case passing unedited; `cargo test -p tungsten-core --locked` → exit 0, no oracle entry added; `cargo test -p example-01-platformer --locked` → exit 0 with no test edited (its `ball_pit` checks among them); `cargo test -p tungsten --locked --lib kit_recipes::determinism` → exit 0; sitting on `blast,e01-pit,physics,friction,slopes`: `blast` `system.physics_step` p95 falls at least 2.0 ms in both pairs, `bench.escaped` 0, and `e01-pit`'s `stage.total` p95 and p99 rise in neither pair by more than 1 ms.
- **Commit:** `perf(physics): read the arrival pass's candidates from the pair list`
- **Moves:** the hashes; the physics guards; `D-092` by the new decision.

### Step 7: The pair list kept across frames

- **Probe, and gate:** a counter of the builds a frame start would skip, with travel carried, over `blast`'s steady frames in a release test at 12,000 bodies. Under two frames in three: the step is dropped with the reading.
- **Change:** §2.3; the decision with the old and new hashes; the known-issues row on statics staged at every build, which this changes. Tests land with it: friction switched on, then off, across frames that keep the list; an awake collider enlarged while its travel budget holds, which must meet its new neighbours; a body despawned and one spawned between frames.
- **Done-when:** as Step 6's first four checks; sitting on `blast,e01-pit,physics,physics-sparse,friction,slopes,terrain`: `blast` `system.physics_step` p50 falls at least 3.0 ms in both pairs, `bench.escaped` 0, and `e01-pit`'s `stage.total` p95 and p99 rise in neither pair by more than 1 ms.
- **Commit:** `perf(physics): keep the pair list across frames`
- **Moves:** the hashes; the physics guards; `D-075`, `D-076` and `D-081` by the new decision.

### Step 8: The support record compared without a full sort

- **Probe:** a counter of supports sorted a frame, in a test that edits one tile a frame under a sleeping pile (expected before: every support).
- **Change:** §2.3.
- **Done-when:** `just physics-release` → exit 0, hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0; sitting on `terrain,physics,blast`: `terrain` `system.physics_step` p50 falls at least 1.3 ms in both pairs, digests match.
- **Commit:** `perf(physics): compare the support record without sorting it whole`
- **Moves:** `terrain.json`'s and `pit.json`'s counts may fall; nothing rises.

### Step 9: `TypeId` maps on the pass-through hasher

- **Probe:** criterion `resource_get_x1000` and `data_ref_resolve_x1000`, saved as `before`.
- **Change:** §2.3.
- **Done-when:** the criterion pair against `before` → both improve; sitting on every CPU row (`--cpu-only`, no `--rows`): `spread` `system.effect_tick` p50 falls at least 0.5 ms in both pairs and no row reads `regressed`.
- **Commit:** `perf(core): hash TypeId keys through the pass-through hasher`
- **Moves:** none.

### Step 10: One definition resolve an effect instance

- **Probe:** a by-ID `effect_tick` row in `benches/kit.rs` (today's uses `DataRef::inline` and never reaches `kind_for`), saved as `before`.
- **Change:** §2.3.
- **Done-when:** `cargo test -p tungsten-kit --locked` → exit 0; `cargo test -p tungsten --locked --lib kit_recipes::determinism` → exit 0, the pin unchanged; sitting on `spread,e01-stress`: `spread` `system.effect_tick` p50 falls at least 0.5 ms in both pairs.
- **Commit:** `perf(kit): resolve an effect's definition once an instance`
- **Moves:** none.

### Step 11: `animation_tick`'s one clip resolve a row

- **Probe:** a new `animation_tick` pair in `benches/kit.rs` under `D-174`'s bound, saved as `before`.
- **Change:** §2.3; `just api` regenerates `api/tungsten-core.txt`.
- **Done-when:** `just api-check` → exit 0; `cargo test -p tungsten-kit --locked` → exit 0; sitting on `e01-stress,particles,template`: `e01-stress` `system.animation_tick` p50 falls at least 0.5 ms in both pairs.
- **Commit:** `perf(kit): resolve a sprite's clip once in animation_tick`
- **Moves:** none.

### Step 12: Chemistry's queue index and tile gate

- **Probe:** a `chemistry` emission row in `benches/kit.rs` at 12,000 receivers in lockstep, saved as `before`.
- **Change:** §2.3.
- **Done-when:** `cargo test -p tungsten-kit --locked` → exit 0; `cargo test -p tungsten --locked --lib kit_recipes::determinism` → exit 0, the pin unchanged; `cargo test -p example-02-bench --locked guards_spread` → exit 0; sitting on `spread`: `system.chemistry` p95 falls at least 3.0 ms in both pairs.
- **Commit:** `perf(kit): cut chemistry's queue probes and its tile lookup`
- **Moves:** `spread.json`'s `allocations` may fall.

### Step 13: Close

- **Change:** the close sitting, `WGPU_BACKEND=vulkan just perf sitting --base <the commit that added this plan> --wait-quiet 600`, every row, GPU runs included; §2.6's marks in Phase 7's §2.5; `docs/perf/benchmarks.md`'s levers and the new rows' reference numbers; the known-issues rows the steps changed; the dropped and stopped items to `ROADMAP.md`'s Later; the plan and its citations deleted; `cpu-stress` out of Now.
- **Done-when:** the close sitting → `e01-stress`'s and `e01-pit`'s `stage.total` p95 and p99 lower in both pairs, quoted in ms, no other row `regressed`; `just check`, `just physics-release`, `just perf-test` and `just repo-check` → exit 0; `just smoke` → passes; the demo is `e01-stress`'s captures, valid in the close sitting with their script reports passing.
- **Ends with:** the commits, the evidence, what the owner should try (01's pit under missiles before and after) and the next prompt (7.13's plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | A hash-moving step fails a test of 01 that pins exact motion and is no pile check | The step stops: 01 is not edited, and the fix is recorded for after 7.13 |
| Q2 | Step 5 changes what 01 does (a digest moves, every test passing) | Kept while `pit.json`'s `spawned`, `despawned` and `archetype_moves` hold and the two sides' `balls` and `burning` agree within 1 %; past that the step stops and the lever waits for 7.13's readers |
| Q3 | A step meets its target on `blast` and `e01-stress` does not move | Kept; the close judges the pass on `e01-stress` and `e01-pit` |
| Q4 | The pit's wave moves again under steps 6 or 7 and `e01-pit`'s p95 or p99 rises though its wake frames cost less | A rise over 1 ms in a pair of the step's sitting is a missed target: one revision, then the step is reverted. The close's rule stands as written; the evidence row quotes `e01-pit`'s frames split by state |
| Q5 | The run outgrows a session | The next session resumes at the first step without an evidence row |
| Q6 | The thread pool's decision | The owner's, after the pass, on a fresh measurement (§2.5) |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** the physics determinism and containment hashes and the kit's pin, in steps 6 and 7 alone, each with its decision and both values in the evidence log; the guards' counts as each step's Moves says, never a rise in a cost counter outside them; row digests after steps 5 to 7; `D-075`'s event clause (Step 5).
- **Must not move:** the determinism test's two-run assertion at every commit; containment (no escape) and every tunnelling case; 01's tests, unedited, its pile checks among them; `gpu-visual.png` and the post and transition regressions; the rows' `workload_version`s; example 01's source beyond what `c5c69d1e` added; `D-067` and `D-131`: no landed change spawns a worker.

## 6. Stop conditions

- A step misses its target: one revision, then its files are restored from the last commit, the step is marked stopped with both sittings' paths, and the pass carries on.
- A probe's gate fails (steps 6 and 7): the step is dropped with the reading.
- A done-when check fails, or a file outside the step's list is needed: the skill's restore, the step skipped with its reason, the steps that depend on it named (5 and 7 need 3; 6 needs 1).
- A sitting refuses to start (an encoder, a busy process): wait for quiet; never `--allow-background` for a judged sitting.
- The pass ends when every item is done, dropped or stopped, or a profile of the stress row's 20 highest frames shows the rest is the work itself.
- The close sitting misses the pass's done-when: the plan is not deleted and stays `in progress`, the evidence row names the percentile that missed with the frames split by state, and the owner decides.

## 7. Critique

gpt-6-astra through `critique --raw --repo`, 2026-11-10, round 1 at xhigh, on the tree at `5809023b` with this plan uncommitted and §7 and the log cut: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 309.9 s, tokens in 1021992 (cached 923008), out 9815 (reasoning 4705), commands 36, output 441.3 KB (largest 73.3 KB), usage 5h 0% → 24%, week 0% → 4%`. No second round: the owner's prompt set one round, though this one changed done-when checks; its changes add checks and narrow two claims.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | HIGH: a pair list kept across frames can leave `pair_tangents` empty when friction turns on between frames, and the pair loop then indexes it | `step/pairs.rs:484-487` sizes it at a build alone; `step/gather.rs:188-192` refills `frictions` each frame; `step/mod.rs:1344-1345` indexes it | Accepted | Step 7's contract invalidates when a side array fills or empties and checks every pair-sized buffer on a frame that skips the build; a friction on-and-off test lands with it |
| 2 | HIGH: Step 7's invalidators omit an awake body's collider change | `step/gather.rs:226` reads the collider afresh; `step/pairs.rs:527-528` trips on travel and speed alone | Accepted | any proxy whose shape, offset or kind changed invalidates; an enlarged-collider test lands with it |
| 3 | HIGH: a `mover` bit set at the pair's build is not bit-exact in the backward sweep | `step/arrival.rs:374-375` changes both velocities; `:403` recomputes the mover after them | Accepted | the bit serves the depth pass and the sort alone (§2.3) |
| 4 | MEDIUM: a flag carried as an impulse does not give one event a step, and the safety net sends events outside it | `step/mod.rs:1343` zeroes a pair's impulse when its contact goes; `step/sweep.rs:262-270` pushes a `CollisionEvent` | Accepted | Step 5 keeps its own emission record, shared by both producers; `step/sweep.rs` joins its files; two tests land with it |
| 5 | MEDIUM: 01's readers do not dedupe to the same reactions; the row's asserts cannot show a lost one | `elements/systems.rs:545` and `:695` project a speed on each event's normal; `scripts/perf-rows.json`'s four asserts | Accepted | §2.3 and A14 say so; Step 5 holds `pit.json`'s `spawned`, `despawned` and `archetype_moves` and compares both sides' `balls` and `burning`; Q2's default is that bar |
| 6 | MEDIUM: Q4 lets `e01-pit`'s percentiles rise where the close requires them lower | Q4 against Step 13 and the header's done-when | Accepted | steps 6 and 7 sit `e01-pit` and treat a rise over 1 ms as a miss; the close's rule stands, and §6 says what a missed close leaves |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-10 | done, after its one revision | Probe (`a_blasted_pile_prints_the_arrival_passes_counts`, twelve frames after the blast), the same before and after: listed 10,303, clamps 26,250, walked 951,224, past the cut 80,613, searches 30,931, pairs built 38,109, 25.0 walked a pair built (the plan expected about ten; a listed body walks 92 at 64 px cells). First sitting, §2.3 as written: `blast` `system.physics_step` p95 40.78 → 38.77 and 40.71 → 38.76 ms (−2.01, −1.95, missed), p99 46.36 → 43.58 and 46.46 → 43.64 (−2.78, −2.81). A profile of its 20 highest frames read why: the unstable sort on unique keys cost 1.33 ms where the stable one, on a few distinct keys, cost 0.49, and the cut's branch mispredicted on each candidate kept. Revision: a round over 128 pairs is put in order in two passes over its depth and mass classes (16 at most, else the unstable sort, which smaller rounds keep), and the cut marks 64 candidates at a time in a bit mask; tests hold every path to a stable sort's order. Revision sitting: p95 40.89 → 37.58 and 40.91 → 37.78 ms (−3.31, −3.13), p99 46.39 → 42.23 and 46.60 → 42.15 (−4.15, −4.45), `improved` in both pairs; `physics` and `friction` `unchanged`; "First-run digests match" on the three rows in both pairs; no row `regressed`. `just physics-release` → exit 0, 16 passed, both hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0, 1,747 passed; `just check` → exit 0. Pins: `blast.json` `allocations` 1,393 → 1,365 and `allocated_bytes` 6,705,420 → 6,505,864; `friction.json` 1,992 → 1,988 and 3,801,516 → 3,777,632; the other guards and `pit.json` unmoved. The arrival pair keeps its 28 bytes (at 32, `physics.json`'s `allocated_bytes` rose 512). | `perf-runs/20261111T005202Z-sitting/` (missed), `perf-runs/20261111T013009Z-sitting/`, `perf-runs/20261110-cpu-stress/step-1/` (the folds by function and by line) |
| 2 | 2026-11-10 | done | Probe: `nm -C` on the perf build listed `build_pairs::{closure#0}` out of line; after, it is gone. Criterion saved as `before`, then against it: `build_pairs_12k_pile/step` (new: 12,000 awake circles at rest, unpaired, 64 px cells) 2.044 → 1.611 ms, −21.2 %, "Performance has improved"; `spatial_grid_query_50k_dense/boxes` 34.85 → 29.79 ms, −14.5 %, improved, `points` 8.110 → 8.101 ms and `points_visit` 14.12 → 14.23 ms, each "Change within noise threshold". Judgment: counted as both improved; the bench's two point rows walk by entries as before (a build of the same walk read them between −0.2 and +4.5 %), and its gain is `query`'s walk by runs. Beyond §2.3, inside the step's files: a run's closure is forced inline (`#[inline(always)]` on the closure), and a run tells its repeated ids either id by id or by a copy with no branch, chosen by the cells a grid's entries cover on average (2.5 and up copy). The two disagree by row: every run copied read `physics` p50 −0.47 and `blast` −0.97 ms in single captures, none copied −0.33 and −2.23; the pile covers 1.7 to 1.9 cells an entry (2.8 after a blast), the pachinko board 2.8. Sitting: `physics` `system.physics_step` p50 6.906 → 6.220 and 6.882 → 6.220 ms (−0.69, −0.66; target 0.35), `blast` p50 26.83 → 24.48 and 26.90 → 24.52 (−2.35, −2.38; target 0.8), its p95 −2.21 and −2.30, p99 −2.40 and −2.51; `physics-sparse` p50 −0.19 and −0.18, `friction` −0.39 and −0.39, `slopes` −0.10 and −0.10, all `improved`; "First-run digests match" on the five rows in both pairs; no row `regressed`. `just physics-release` → exit 0, 16 passed, hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0, 1,749 passed; `just check` → exit 0, no pin moved. | `perf-runs/20261111T035427Z-sitting/`; the variants' single captures `perf-runs/20261111T02*` and `T03*-{physics,blast}/` |
| 3 | 2026-11-11 | done | Probe: criterion `physics_step/dense_pile_awake/12000` (the group's new count) 9.149 → 8.768 ms against `before`, −4.1 %, "Performance has improved" (3,000: −1.2 %). The reference model compares every warm start with a map by key, a seed's tangent now too; `seeds_by_proxy_index_equal_the_keyed_carry_through_every_change` runs it over a heap with friction on a tile floor while nothing changes (seeds by index), when a body leaves and one comes and when a tile is cut (each seeds by key once). `D-076`'s Decision field names the keyed map as the carry, so §2.4's condition held: `D-269` amends it and `D-229`'s tangent carry. Sitting: `blast` `system.physics_step` p50 24.47 → 22.05 and 24.59 → 22.05 ms (−2.42, −2.54; target 1.5), p95 35.47 → 33.04 and 35.64 → 33.05 (−2.42, −2.60), `improved` in both pairs, p99 −2.50 and −2.72; `physics` p50 +0.06 and +0.04, `friction` −0.06 and −0.05, `unchanged`; "First-run digests match" on the three rows in both pairs; no row `regressed`. `just physics-release` → exit 0, 16 passed, hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0; `cargo test -p tungsten-core --locked --test integration decision_index` → exit 0; `just check` → exit 0. No pin moved: the guards' windows grow no contact buffer. `ContactConstraint` is 48 bytes outside tests, where it keeps its key for the oracles (`step_oracle.rs` reads it and is outside the step's list). | `perf-runs/20261111T043155Z-sitting/` |
| 4 | 2026-11-11 | done | Probe: criterion `event_send_200k` 2.512 ms against `event_append_200k` 1.829 ms (each fills a step's buffer with 200,000 `CollisionEvent`s, hands it over and flushes; the fill is common to both). `EventQueue::append` trades buffers with an empty window, else copies; the step's five hand-overs go through it. Sitting: `blast` `system.physics_step` p50 22.05 → 20.31 and 21.98 → 20.23 ms (−1.75, −1.75; target 1.2), `improved`, p95 −1.82 and −1.79, p99 −1.86 and −1.79; `physics` p50 −0.05 and −0.03, `unchanged`; "First-run digests match" on both rows in both pairs; no row `regressed`. `just api-check` → exit 0 (`api/tungsten-core.txt` gains `EventQueue<T>::append`); `just physics-release` → exit 0, 16 passed, hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0; `just check` → exit 0. No pin moved, `events_sent` among them: a window's three buffers are full-grown by the guards' warm-ups either way. | `perf-runs/20261111T050224Z-sitting/` |
| 5 | 2026-11-11 | done | Probe (`a_resting_pile_prints_its_events_and_their_pairs`): 2,400 events of 600 pairs in one step before, 4.00 a pair, and 600 after; `blast.json`'s `events_sent` 271,607 → 68,838. The record is a flag a pair of the list and a set by proxies for the pairs with a static or tile side and for those that left the list; the two tests the plan names pass, the first through a rebuild and a repair, for a static and a dynamic partner. `D-270` amends `D-075`'s and `D-066`'s event clauses. Sitting: `e01-stress` `system.fire_spread` p50 13.11 → 5.03 and 13.04 → 5.03 ms (−8.08, −8.02; target 5.0), `stage.total` p95 58.50 → 46.04 and 58.31 → 46.02 (−12.45, −12.29; target 5.0), p99 61.78 → 48.82 and 61.53 → 48.70, p50 −11.69 and −11.58, `body_impacts` p50 2.48 → 0.79, `iron_crush` p95 4.43 → 1.32, `iron_friction` p50 1.23 → 0.34, all `improved` in both pairs; `e01-stress`'s digests match (`2b7220fee119525c`); its captures valid, every run's report passing, and the four captures' reports equal: `pit` `balls` 11,229 and `pit_fire` `burning` 10,472 on both sides (Q2's bar, 1 %: no difference). `blast` `physics_step` `unchanged` (p50 −0.29 and −0.33, p95 +0.22 and +0.24); its digest moves, by `events` alone (its `physics:` lines hash as the base's and its `bench:` lines too once `events=` is cut; `bench.events` p50 199,550 → 50,230); no row `regressed`. `cargo test -p example-01-platformer --locked` → exit 0, 444 passed, no test edited; `cargo test -p tungsten --locked --lib kit_recipes::determinism` → exit 0, the pin unchanged; `just physics-release` → exit 0, hashes unchanged; `cargo test -p tungsten-core --locked --test integration decision_index` → exit 0; `just check` → exit 0. Pins: `events_sent` falls on 02's `blast`, `friction` (155,823 → 40,787), `slopes` (34,488 → 9,959), `physics` (31,391 → 12,737), `terrain` (7,926 → 2,135) and `physics-sparse` (4,866 → 4,621); `allocated_bytes` falls on `physics` and `slopes`, `allocations` on `slopes`; nothing rises; `pit.json` whole, its `spawned`, `despawned` and `archetype_moves` among it. | `perf-runs/20261111T053805Z-sitting/` |
| 6 | 2026-11-11 | stopped: target missed twice, files restored | Gate: 9,422 of 10,303 listed bodies (91 %) within their pair budget on the Step 1 test's blast frames, so not dropped. First sitting, every proxy's partners taken from the pair list whenever one body is listed for 256 pairs and read in pair-list order: `blast` `system.physics_step` p95 31.42 → 29.76 and 31.23 → 29.69 ms (−1.66, −1.54; target 2.0, missed), `physics` p50 6.22 → 6.91 in both pairs, `regressed`; `e01-pit` `stage.total` p95 −4.81 and −4.49, p99 −3.82 and −3.64. That variant, and one that took the partners once a build with the repairs' pairs added (single captures: `blast` p95 −3.76, `e01-pit` p95 −8.76 and p99 −10.2 ms), changed motion and not only cost: a body within budget met every partner in reach, where its own walk meets those sharing a grid cell with its box. The containment hash moved (`0xaee2_72e0_1ffc_4e4c` → `0x1b7b_9927_7f88_d9db`, then `0x2e4e_4cbb_b050_9120`) and 01's `an_open_hole_sweeps_small_balls_from_its_core_a_few_a_step` failed (a ball left in reach: `[(400, 49), (49, 2), (2, 1), (1, 1), (1, 1)]`), Q1's stop. Revision: the partners read in the order the body's own walk meets them, less the ones it would not, the first round's bodies alone, a test check in the pass holding each to its walk: both physics hashes, the kit's pin, `pit.json` and 01's 444 tests hold unedited, and the sitting reads `blast` p50 19.82 → 19.13 and 19.80 → 19.15 ms (`improved`), p95 −0.11 and −0.05 (missed), p99 −0.61 and −0.58; `e01-pit` p95 +0.07 and +0.17, p99 −0.43 and −0.14; `physics`, `friction` and `slopes` `unchanged`, digests equal. The lookup is no longer where the tail's time is: after Steps 1 and 2 a listed body's walk and cut cost about 0.5 µs. Not run: the decision, Phase 7's §5 sentence. Step 7 does not depend on this one. | `perf-runs/20261111T064305Z-sitting/`, `perf-runs/20261111T073652Z-sitting/`; `perf-runs/20261110-cpu-stress/step-6/` (`revision.patch`, the folds); captures `perf-runs/20261111T071002Z-blast/`, `T071300Z-e01-pit/`, `T073110Z-blast/`, `T073414Z-e01-pit/` |
| 7 | 2026-11-11 | dropped at its gate | Probe (`a_blasted_pile_of_12_000_prints_the_frame_starts_that_could_keep_their_pairs`, release, 12,000 balls on the `blast` row's lattice in a bin of static boxes, asleep, then a blast every 20 frames for the row's 300): a shadow of each body's budget carries its travel across frames, and a frame start keeps the list when at most one awake body in four has outrun its budget. 66 of 112 steady frame starts keep it (59 %; the eight frames before each blast after the first), 170 of 300 in all; a 120-frame sample read 26 of 40. Under two in three, so the step is dropped: a budget covers one frame's travel at the speed it was built for, and a body over about 250 px/s outruns it at the next frame's start. The probe stays as a test; nothing else changed. Not run: the change, its three tests, the decision, the sitting. | `cargo test --release -p tungsten-core --locked --lib a_blasted_pile_of_12_000 -- --nocapture` |
| 8 | 2026-11-11 | done, after its one revision | Probe (`a_tile_cut_under_a_sleeping_pile_prints_the_supports_sorted`, 600 tile supports, one tile cut a frame far from a sleeping pile): supports sorted a frame 1,199, then 598, 597, … before; 1 a frame after. First sitting, the lists' shared starts and ends stripped and the stretch between sorted on both sides: `terrain` `system.physics_step` p50 6.46 → 8.65 and 6.45 → 8.63 ms, `regressed`: the row edits tiles all over its map, so the stretch was most of the list, sorted twice where the cached order sorted it once. Revision: the two lists walked side by side in proxy order, equal entries paired off, a look 64 entries ahead where they differ, and only the unpaired entries sorted and compared; pairing equal entries off both sides leaves the whole lists' difference whatever their order, and a test check in the function holds each result to the whole lists', sorted. Revision sitting: `terrain` p50 6.54 → 4.24 and 6.47 → 4.26 ms (−2.30, −2.21; target 1.3), p95 −2.35 and −2.18, `improved`; `physics` and `blast` `unchanged`; "First-run digests match" on the three rows in both pairs; no row `regressed`. `just physics-release` → exit 0, hashes unchanged; `cargo test -p tungsten-core --locked` → exit 0, 1,702 passed; `just check` → exit 0. Pins: `pit.json` `allocations` 59,980 → 59,979 and `allocated_bytes` 69,236,700 → 68,099,612; 02's guards unmoved; nothing rises. Not taken: tile changes from the cache's written cells (§2.3's "where it can"), which `step/gather.rs` holds, outside the step's list; the walk already leaves the sort to the changed entries. | `perf-runs/20261111T082432Z-sitting/` (regressed), `perf-runs/20261111T084440Z-sitting/` |
| 9 | 2026-11-11 | done | Probe: criterion `resource_get_x1000` 7.75 → 2.78 µs (−64 %) and `data_ref_resolve_x1000` 29.27 → 25.55 µs (−13 %) against `before`, each "Performance has improved". `ResourceMap` and `DataRegistry::kinds_by_type` are `TypeIdMap`s; `World::event_types` is a `HashSet` on the same pass-through hasher, since the workspace's clippy refuses a map of `()` (`zero_sized_map_values`); none is iterated. Sitting, every CPU row (19, 2,975 s): `spread` `system.effect_tick` p50 2.83 → 1.63 and 2.83 → 1.63 ms (−1.19, −1.20; target 0.5), p95 −1.34 and −1.39, `system.chemistry` p95 10.53 → 9.75 and 10.51 → 9.75, `stage.update` p95 −2.13 and −2.15, `improved`; every row `ok`, none `regressed`; `spread`'s, `e01-stress`'s, `e01-pit`'s, `ecs`'s, `churn`'s, `saves`', `rooms`' and `template`'s digests match. `just check` → exit 0; no pin moved. | `perf-runs/20261111T091417Z-sitting/` |

## Follow-ups

- Collision once a frame with the separation updated inside substeps (Box2D v3's Soft Step): the narrow phase and the events fourfold, against `D-063`, `D-064` and `D-075`; a plan of its own after 7.14.
- A wake by reach instead of by island, after the known issues on the absorbed hit and on sleepers left hanging are closed.
- The resting query index (C1), the extract's sort passes (C6), the flush's per-column batching (C4) and chemistry's emission phase (C2): 7.14's, each with this pass's measurement.
- The worker pool's decision (§2.5).
- An arrival pass that clamps a listed body against every pair partner in reach, not only those its own grid walk meets (Step 6's first variants): `e01-pit` `total` p95 −8.8 and p99 −10.2 ms in a single capture, by changed motion, with 01's black-hole sweep test failing; a `D-092` question for after 7.13, with `perf-runs/20261110-cpu-stress/step-6/`.
