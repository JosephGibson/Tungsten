# The chemistry shock pass (chemistry-shock-pass) — draft

- **status:** draft
- **goal:** lower `system.chemistry`'s p50, p95 and p99, and the frame's spike, while a chain reaction runs through example 01's burning ball pit: a new `e01-shock` row that reproduces the play-tested balloon, per-frame chemistry counters, and exact kit and engine fixes, each sat against the row's base; every digest and pixel unchanged.
- **non-goals:** anything that changes a chain's outcome (a cap per stimulus, a smaller run cap, a chain spread over more frames, merged blasts: measured on scratch and proposed, §2.4); the physics step's cost in the chain's frames (`physics_step` p95 about 34 ms, the pile shoved and thinned: a follow-up); 01's handlers and data unless a step's profile names them; a release cut.
- **files to touch:** Step 1: `examples/01_platformer/src/features/run/pit.rs`, `.../run/script.rs`, `.../elements/cascade.rs`, `scripts/perf-rows.json`, `docs/perf/benchmarks.md`. Step 2: `crates/tungsten-kit/src/chemistry/{stats.rs (new),mod.rs,outputs.rs,blast.rs}`, `crates/tungsten-kit/src/lib.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`, `crates/tungsten/src/app.rs`, `scripts/perf_stats.py`, `scripts/test-perf.py`, `docs/perf/profiling-workflow.md`, `api/tungsten-kit.txt`. Step 3: `crates/tungsten-kit/src/chemistry/{grid.rs,blast.rs}`, `crates/tungsten-kit/src/tests/chemistry.rs`. Step 4: `crates/tungsten-kit/src/chemistry/grid.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`. Step 5: `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/src/tests/assets/data.rs`, `crates/tungsten-kit/src/chemistry/rules.rs`, `crates/tungsten-kit/src/damage/kinds.rs`, `crates/tungsten-kit/src/foundation/tags.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`, `api/tungsten-core.txt`. Step 6 (conditional): `examples/01_platformer/src/features/elements/cascade.rs`. Step 7: `docs/perf/benchmarks.md`, `docs/known-issues.md`, `ROADMAP.md`, this plan (deleted).
- **ordered steps:** (1) the `e01-shock` row; (2) chemistry's per-frame counters, the base and its profile; (3) a stimulus-free blast gathers only the bodies with health; (4) the grid's save-order sort over keys read once; (5) a game's stimulus checked over its kinds' entries in place; (6) 01's bin by sorted cells, only if Step 5's profile names it; (7) the proposals on scratch, the close sitting and the records.
- **done-when:** every step's done-when passes, quoted in the evidence log; the close sitting reads `e01-shock`, `spread`, `e01-stress` and `e01-pit` `ok` against the base with digests equal; `just check` passes and the row's script runs its chain to its asserts; the plan deleted with its citations and the item out of Now.

Off-roadmap, on the owner's prompt of 2026-11-25 ("a targeted performance pass on the kit's chemistry"). Written 2026-11-25 at `8570ebf1`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md), the [tungsten-perf skill](../../.claude/skills/tungsten-perf/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs. Budget: about 20 % of the weekly meter, no new work past 15 % (`perf-runs/713-chain/spend.py --since 2026-11-25T02:30:00Z`).

## Context digest

Play-testing 01, a chain reaction through a large pile of burning balls made `system.chemistry` balloon. No row reproduces it: `spread` (02's bench) is the only row that owns `system.chemistry`, and 01's rows cast no `ice_spray`. Earlier chemistry perf work: `D-241`'s queue, `D-271`, 7.14's cell directory and kept receiver slot (`spread` p95 5.1 → 2.4 ms). This pass's probes (`perf-runs/20261125-chemistry-shock/README.md`, machine-local) ran a scratch build with timers in the kit's run and 01's handlers. They found:

- **The ice spray across a burning pile starts a steam chain**, not thermal shock. Cold on a burning small ball runs 01's `steam` handler, and its 96 px burst sends `cold` to every burning small ball in reach. The probe's chain took 11,990 balls in about 455 frames, 48 a run (01's cap).
- **Thermal shocks need heat on frozen balls.** The spray freezes only non-burning balls its seven rays meet, and the fire's spread never heats a frozen ball. So the row lands a missile on a patch the spray froze while the fire closed in: 5 frozen among 5,534 burning, 5 shocks.
- **The balloon is one call.** Both handlers call the kit's public `blast()`, which rebuilds chemistry's grid at every call: every `Tags` holder, about 9,500 burning balls, sorted by save order with a `Saved` lookup per comparison. 48 reactions took 91.4 ms; 48 grid builds took 87.7 ms of it.
- **A probe of Step 3's change** read `chemistry` p95 98.7 → 3.96 ms and `total` p95 138.3 → 48.6 over the chain, its window's `physics:` digest equal.

## 1. Audit

Read at `8570ebf1`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | 01's chemistry runs at cap 48, bound 16,384 and depth 255, not the kit's 256, 16,384 and 8 | `examples/01_platformer/src/features/elements/mod.rs:88`, `examples/01_platformer/src/game.rs:50` |
| A2 | `cast_at` lands no `ice_spray`; the spray is a cone of seven rays from its caster, each stopping at the first body | `examples/01_platformer/src/features/run/pit.rs:181`, `examples/01_platformer/src/features/toys/systems.rs:1173` |
| A3 | In a cold cone a burning small ball takes `cold` (its steam), any other ball a chill toward freezing | `examples/01_platformer/src/features/toys/systems.rs:1282`, `:1293` |
| A4 | The fire's spread lights no frozen ball, and sends no heat | `examples/01_platformer/src/features/elements/systems.rs:92` |
| A5 | A steam burst despawns its ball, sends `cold` to the burning small balls in reach and calls the kit's `blast()`; a thermal shock calls it too | `examples/01_platformer/src/features/elements/cascade.rs:719`, `:618` |
| A6 | The public `blast()` clears its grid at every call | `crates/tungsten-kit/src/chemistry/blast.rs:605` |
| A7 | A build walks every `Tags` holder, then the `Health` holders, and sorts them by `order.key`, a `Saved` lookup each time the sort compares two | `crates/tungsten-kit/src/chemistry/grid.rs:242`, `:267`; `crates/tungsten-core/src/save/id.rs:388` |
| A8 | A blast that sends no stimulus reads only `Health` holders from its gather: its hits need a live `Health`, its impulse has its own overlap and skips by `Health`, its stimuli come after a return | `crates/tungsten-kit/src/chemistry/blast.rs:792`, `:827`, `:838` |
| A9 | Every blast of 01's handlers, its missile and its lightning sends no stimulus | `examples/01_platformer/src/features/elements/cascade.rs:607`, `examples/01_platformer/src/features/toys/systems.rs:601`, `examples/01_platformer/src/features/lightning/mod.rs:279` |
| A10 | The public `stimulate()` checks its name each call: the damage types, then the declared tags, building a `MissingAsset` it drops when a tag passes; then every rule, by an ID lookup each | `crates/tungsten-kit/src/chemistry/rules.rs:107`, `:186`; `crates/tungsten-kit/src/damage/kinds.rs:164`; `crates/tungsten-kit/src/foundation/tags.rs:476`; `crates/tungsten-core/src/assets/data.rs:219` |
| A11 | A game's `stimulate` queues at depth 0, so 01's handler chains never deepen and the depth limit never cuts them | `crates/tungsten-kit/src/chemistry/mod.rs:69`, `:149` |
| A12 | 01's handlers bin the step's 12,000 bodies once a step into a `HashMap` of `Vec`s on the default hasher | `examples/01_platformer/src/features/elements/cascade.rs:248`, `:476` |

## 2. Design

### 2.1 The row

`e01-shock`, a scripted CPU row of 01 at 1920×1080, five repeats, warm-up 690 and 480 measured frames (691 to 1,170: the chain's frames). Its script, measured on the probe build:

- `start_run` at 2, `fill_pit` 12,000 at 30.
- The wizard held at (4,330, 3,400), spraying cold at (4,330, 3,600), frames 600 to 690, freezing an east patch.
- A fire missile at (3,300, 3,600) at 640: the fire runs east through the pile.
- Asserts at 690: `pit_shock` `/frozen` ≥ 3 and `pit_fire` `/burning` ≥ 5,000.
- At 696 a missile on the patch (4,325, 3,590): its blast's heat shocks the frozen balls, in the window.
- At 700 a missile at (3,840, 3,560); from 712 to the end the wizard at (3,840, 3,400) spraying cold at (3,840, 3,600): the steam chain.
- Asserts at the window's end: `pit_shock` `/shocks` ≥ 3, `/steams` ≥ 11,000, the `player` probe's health above 0.

01's side:

- `cast_at` gains `ice_spray`: the wizard's cold spray held toward the point for the step, free of mana, through `toys::spray`.
- A `pit_shock` probe gives `{frozen, shocks, steams}`. `frozen` counts the small balls carrying `status.frozen` now. `shocks` and `steams` are two unsaved counters on `Cascades` that the two handlers count: a run's, since `Cascades` is inserted at a run's start.
- Owned: `system.chemistry` and `system.chemistry_tick` p50/p95/p99, `stage.total` p95/p99. Guard: the script's report. It has no counter guard: as with `e01-stress`, its chain runs 480 frames of up to 100 ms, and the kit's tests and `pit`'s pins cover the code.

### 2.2 The counters

`ChemistryStats`, a kit resource `ChemistryPlugin` inserts, counts a frame's chemistry in place. The frame's first counted operation, a run or a blast (by `Time::frame`), zeroes it. A script's or a system's `blast()` before the run, or in a frame with no run, counts in that frame: script actions run before the fixed update (`crates/tungsten/src/app.rs:2401`). It holds:

- `runs`, `queued` (entries made between two runs' ends: the queue's growth plus the run's take), `taken`, `dropped`, `cut`, `pending` after the last run;
- `behind`, the runs the oldest taken entry waited; `depth`, the deepest entry taken;
- `blasts` and `gathered`, the receivers their gathers found, both blast paths;
- each handler's reactions and milliseconds, by name;
- milliseconds in the take, the rule matches, the outputs and the blasts.

Behind `TUNGSTEN_PERF_LOG` and the `kit` feature, the umbrella prints it as a `chemistry:` companion line after `physics:`. The line has zeros on a frame where nothing counted, and `handler.<name>=n handler.<name>.time=Xms` per handler. The kit's test counts a public blast before a run and in a frame with none. `perf_stats.py` reads it as `counter.chemistry.<name>`, outside the digest. Timers sit per run and per reaction, never per merge. The resource is filled in place, so a warm frame allocates nothing: `spread`'s and `pit`'s pins hold, since the handler list fills in warm-up.

### 2.3 The fixes (exact)

- **Step 3. A stimulus-free blast gathers from the bodies with health (A6, A8, A9).** `Grid` gains a build over the `Health` holders alone, `tagged` as each has `Tags`. `blast()` takes it for a blast that sends no stimulus, or in a World without chemistry. Both orders are save order with unique keys, so its targets are the full gather's `Health` holders in the same order. Their hits, crits, pushes and the impulse's skips are the same; the widening by the farthest surface still covers every holder within reach. `detonate`'s blasts keep the run's grid. A test-only switch keeps the full build, and a test compares the two over seeded worlds of tagged, health-only and mixed entities and stimulus-free blasts: health, the queued and landed hits and their order, the crit stream, positions and velocities. A negative control (the holders' build dropping tagged holders) must fail it.
- **Step 4. The grid's sort over keys read once (A7).** The build fills a kept `Vec` of `(order key, candidate)`, sorts it unstably (the keys are unique) and lays the candidates out from it. No allocation once warm. The test compares the order with `sort_by_key`'s in a saving World of saved and unsaved candidates.
- **Step 5. A game's stimulus checked over its kinds' entries in place (A10).** Core gains `DataRegistry::entries::<T>()`: `(id, &T)` in ID order, the order `ids` and `get` give. Three checks move to it:
  - the kit's `rules_of`, which `answers` reads;
  - the damage types' `entries`;
  - `check_declared`'s data tags.

  `check_stimulus` asks a predicate first and builds its error only when both kinds refuse. The answers stay the same, and core's test checks the new method against `ids` and `get`.
- **Step 6. 01's bin by sorted cells (A12), only if Step 5's profile puts `Bin::build` at 10 % or more of `chemistry`.** A kept `Vec` of `(cell, index)`, sorted, replaces the map. `near` sorts its finds by save ID, so any exact binning returns the same list; 01's cascade tests and a seeded comparison with the map check it.
- **The systems' map (`D-311`)** has no stage here whose rows are independent: the handlers write the World. No step uses it unless a step's profile names such a stage.

### 2.4 Proposals, measured on scratch

These change the chain's outcome, so none lands; each goes to ROADMAP's Later with its numbers. On the final code's export with 01's cap at 24, measure the chain's frames and `chemistry` and `total` p95, reported strictly as a smaller global cap. A cap per stimulus is not the same thing: the window's thermal shocks take `heat` entries and the steam `cold`, under one cap today (`crates/tungsten-kit/src/chemistry/queue.rs:700`). It needs an implementation to measure, so it is estimated from the counters' mix of handlers, as merged blasts are from `gathered` and `blasts`. ROADMAP's Later already holds the cap per stimulus (`D-271`).

### 2.5 Sittings

- The base is Step 2's commit (base `B`): the first commit with both the row and the counters, so both sides print `chemistry:`.
- Step 3 sits against `B`. Steps 4 to 6 each sit against the last kept commit before their own: its parent, or after a skip or a revert the commit that left the tree as it stands. The evidence names it. The base's chain p95 is about 98 ms, and its noise between sittings (about 0.5 ms) would swamp a sub-millisecond step's difference of two sittings.
- The close sits every row against `B`.
- **A stale base build:** `git archive` gives a base's files its commit's time, and cargo reuses a newer `target/perf/base-aligned` build without a warning (a known issue). Before each sitting whose base differs from the last sitting's, age the workspace fingerprints there (`fd -t f . $(fd -t d -d 1 '^(tungsten|example-0)' target/perf/base-aligned/release/.fingerprint) -x touch -d 2001-01-01 {}`). The sitting's log must then show the base tree's crates `Compiling`.
- **The chain's counts, exactly:** `perf-runs/20261125-chemistry-shock/tools/chem_counts.py <sitting>` hashes each run's `chemistry:` count fields over the window: `runs`, `queued`, `taken`, `dropped`, `cut`, `pending`, `behind`, `depth`, `blasts` and every handler's reactions. Timings and, from Step 3 on, `gathered` are left out. The digest hashes only `physics:` and `bench:` (`scripts/perf_stats.py:179`), so a change in what the queue takes could pass it unseen. Every judged pair's hashes must be equal.
- Steps commit on their untimed checks, then sit. Any check that fails after the commit (a floor, a digest, a count hash, another row's verdict) takes a `revert(<scope>):` commit. It restores the step's files from its parent with `git show <parent>:<path> > <path>`, and `git diff --quiet <parent> HEAD -- <the step's files>` must exit 0 before the next step takes its base. The step is marked skipped, and a missed floor is asked of the owner at the close (the owner's rule: keep a fix that clears its floor, revert one that does not).
- `pgrep -x nxcodec.bin` before each sitting: while the encoder runs, the timed checks wait and the steps go on.

## 3. Steps

### Step 1: The `e01-shock` row

- **Files:** `examples/01_platformer/src/features/run/pit.rs`, `examples/01_platformer/src/features/run/script.rs`, `examples/01_platformer/src/features/elements/cascade.rs`, `scripts/perf-rows.json`, `docs/perf/benchmarks.md`
- **Change:** §2.1: `cast_at`'s `ice_spray` and its doc, `pit::shock` and the `pit_shock` probe, the two counters on `Cascades` and their two increments, the registry row, and the row's section in the benchmarks.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0.
  - `cargo clippy -p example-01-platformer --all-targets -- -D warnings` → exit 0.
  - `cargo test -p example-01-platformer` → exit 0, its `pit` and `play` guards unchanged.
  - `just perf-test` → exit 0.
  - `WGPU_BACKEND=vulkan just perf run e01-shock --repeat 3` → exit 0, `valid: true`, one digest over the three runs, each run's report passing, `system.chemistry` p95 above 50 ms in the capture's README (the balloon).
- **Commit:** `perf(example-01): add the e01-shock row, a chain through the burning pit`
- **Moves:** a new row; no other row's work.

### Step 2: Chemistry's per-frame counters, the base and its profile

- **Files:** `crates/tungsten-kit/src/chemistry/stats.rs` (new), `crates/tungsten-kit/src/chemistry/mod.rs`, `crates/tungsten-kit/src/chemistry/outputs.rs`, `crates/tungsten-kit/src/chemistry/blast.rs`, `crates/tungsten-kit/src/lib.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`, `crates/tungsten/src/app.rs`, `scripts/perf_stats.py`, `scripts/test-perf.py`, `docs/perf/profiling-workflow.md`, `api/tungsten-kit.txt`
- **Change:** §2.2. A kit test runs a small chain and reads its counts: taken, queued, a handler's reactions, a blast's gathered, and a public blast counted before a run and in a frame with none. `perf_stats.py` gets a test of a `chemistry:` line. §2.5's `chem_counts.py` goes in the evidence folder.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0.
  - `just lint` → exit 0.
  - `cargo test -p tungsten-kit` → exit 0.
  - `cargo test -p example-02-bench guards` → exit 0, pins unchanged.
  - `cargo test -p example-01-platformer guards` → exit 0, pins unchanged.
  - `just perf-test` → exit 0.
  - `just api-check` → exit 0, after `just api` rewrote `api/tungsten-kit.txt`.
  - `WGPU_BACKEND=vulkan just perf run e01-shock` → valid, five runs. Its digest equals Step 1's capture's. Its `counter.chemistry.taken` p95 is 48, and `counter.chemistry.handler.steam` and `.handler.thermal_shock` are present.
  - The base profile: a `perf record -e cycles:u --user-callchains --call-graph fp` of the base binary on the row's script, folded by system and under `chemistry`, in the evidence folder. It names the costs; it has no pass or fail.
- **Commit:** `feat(kit): count chemistry's work a frame on the perf log`
- **Moves:** every chemistry row's log gains a `chemistry:` line; no digest, pin or owned metric moves.

### Step 3: A stimulus-free blast gathers only the bodies with health

- **Files:** `crates/tungsten-kit/src/chemistry/grid.rs`, `crates/tungsten-kit/src/chemistry/blast.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`
- **Change:** §2.3's first fix, its test and its negative control.
- **Done-when:**
  - fmt → exit 0; `cargo clippy -p tungsten-kit --all-targets -- -D warnings` → exit 0.
  - `cargo test -p tungsten-kit` → exit 0, the comparison test among them. The control run with the break applied (confirmed by `git diff --stat`) → that test fails.
  - `cargo test -p example-01-platformer` → exit 0.
  - `cargo test -p example-02-bench guards` → exit 0, pins unchanged.
  - `WGPU_BACKEND=vulkan just perf sitting --base B --rows e01-shock,spread --cpu-only` → `spread` `ok`. `e01-shock` reads `system.chemistry` p95 and p99 `improved` in both pairs, p95 at least 50 % below the base's, and `stage.total` p95 `improved`. Each judged pair's digests are equal.
  - `python3 -B perf-runs/20261125-chemistry-shock/tools/chem_counts.py <the sitting>` → every judged pair's count hashes equal, `gathered` left out (§2.5).
- **Commit:** `perf(kit): gather a stimulus-free blast from the bodies with health`
- **Moves:** `counter.chemistry.gathered` (the health holders alone).

### Step 4: The grid's sort over keys read once

- **Files:** `crates/tungsten-kit/src/chemistry/grid.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`
- **Change:** §2.3's second fix and its order test.
- **Done-when:**
  - fmt and the kit's clippy → exit 0.
  - `cargo test -p tungsten-kit` → exit 0.
  - `cargo test -p example-02-bench guards` → exit 0, pins unchanged.
  - `WGPU_BACKEND=vulkan just perf sitting --base <the last kept commit, §2.5> --rows e01-shock,spread --cpu-only` → both rows `ok`, digests equal. `e01-shock` `system.chemistry` p95 is lower by at least 0.2 ms in both pairs: the probe put the sort's lookups at about 15 % of the chain's chemistry after Step 3.
  - `chem_counts.py <the sitting>` → every judged pair's count hashes equal.
- **Commit:** `perf(kit): sort chemistry's grid by save order over keys read once`
- **Moves:** none.

### Step 5: A game's stimulus checked over its kinds' entries in place

- **Files:** `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/src/tests/assets/data.rs`, `crates/tungsten-kit/src/chemistry/rules.rs`, `crates/tungsten-kit/src/damage/kinds.rs`, `crates/tungsten-kit/src/foundation/tags.rs`, `crates/tungsten-kit/src/tests/chemistry.rs`, `api/tungsten-core.txt`
- **Change:** §2.3's third fix.
- **Done-when:**
  - fmt and `just lint` → exit 0.
  - `cargo test -p tungsten-core assets::data` → exit 0, with a nonzero count in `running N tests`.
  - `cargo test -p tungsten-kit` → exit 0.
  - `cargo test -p example-01-platformer` → exit 0.
  - `just api-check` → exit 0 after `just api`.
  - `WGPU_BACKEND=vulkan just perf sitting --base <the last kept commit, §2.5> --rows e01-shock --cpu-only` → `ok`, digests equal, `system.chemistry` p95 lower by at least 0.4 ms in both pairs: the checks were about 30 % of the chain's chemistry after Step 3.
  - `chem_counts.py <the sitting>` → every judged pair's count hashes equal.
  - The step's profile (as in Step 2) gives `Bin::build`'s share of `chemistry`, which decides Step 6.
- **Commit:** `perf(kit): check a game's stimulus over its kinds' entries in place`
- **Moves:** none.

### Step 6: 01's bin by sorted cells (conditional)

- **Files:** `examples/01_platformer/src/features/elements/cascade.rs`
- **Change:** §2.3's fourth fix, with a seeded `near` comparison in cascade.rs's own tests. It runs only when Step 5's profile puts `Bin::build` at 10 % or more of `chemistry`; else the step is skipped with the share quoted.
- **Done-when:**
  - fmt and `cargo clippy -p example-01-platformer --all-targets -- -D warnings` → exit 0.
  - `cargo test -p example-01-platformer` → exit 0.
  - `WGPU_BACKEND=vulkan just perf sitting --base <the last kept commit, §2.5> --rows e01-shock --cpu-only` → `ok`, digests equal, `system.chemistry` p95 lower by at least 0.15 ms in both pairs.
  - `chem_counts.py <the sitting>` → every judged pair's count hashes equal.
- **Commit:** `perf(example-01): bin the cascades' bodies by sorted cells`
- **Moves:** none.

### Step 7: Proposals, the close sitting and the records

- **Files:** `docs/perf/benchmarks.md`, `docs/known-issues.md`, `ROADMAP.md`, this plan (deleted)
- **Change:**
  - §2.4's scratch measurement, in the evidence folder.
  - The close sitting, and the before and after table of each owned stat.
  - Stills of the row at frames 700, 760, 900 and 1,100 (script `capture` steps), from the base's and the final commit's kept binaries, compared with `cmp`.
  - The records: the row's reference numbers and the levers' chemistry line in the benchmarks; stopped levers in known issues; the proposals in ROADMAP's Later.
  - The plan deleted, its citations with it, and the item out of Now.
- **Done-when:**
  - `WGPU_BACKEND=vulkan just perf sitting --base B --rows e01-shock,spread,e01-stress,e01-pit --cpu-only` → every row `ok`, each judged pair's digests equal.
  - `chem_counts.py <the sitting>` → every judged pair's count hashes equal, `gathered` left out.
  - Each still pair → `cmp` exit 0.
  - `just check` → exit 0.
  - `just repo-check` → exit 0.
  - The demo: `cd examples/01_platformer && cargo run --release -- run --script <the row's composed script>`, or the capture's own report, → the report passes with the chain's asserts.
- **Commit:** `docs(plans): close the chemistry shock pass`
- **Ends with:** the commits, the evidence, what the owner should try, the next prompt.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The play-tested chain: steam (cold on burning) or thermal shock (heat on frozen)? | The row runs both, as found. Steam is the balloon; both run through the same `blast()`. Told at the close. |
| Q2 | A floor missed narrowly by a real, exact gain | Revert by the owner's rule. Record the numbers and ask at the close. |
| Q3 | Does the `chemistry:` line need a decision? | No. The profiling workflow's telemetry section records it, as it records `physics:`; it constrains no later work. |
| Q4 | Step 6 below its 10 % gate | Skipped, its share quoted, and 01's bin goes to Later. |

## 5. Invariants

- **May move:** the new row's existence (Step 1); every chemistry row's log gains `chemistry:` (Step 2); `counter.chemistry.gathered` (Step 3); `api/tungsten-kit.txt` (Step 2) and `api/tungsten-core.txt` (Step 5).
- **Must not move:**
  - Every row's digest across Steps 2 to 6 against `B`.
  - The counter guards' pins.
  - 01's and 02's tests.
  - The stills at the close.
  - The chain's counts per frame, as `chem_counts.py` hashes them (§2.5), from Step 2's capture on.

## 6. Stop conditions

- A done-when check fails before the step's commit: restore the step's files from the last commit, mark the step skipped and continue with the steps that don't depend on it. Steps 4 to 6 depend on no other fix; the close needs Step 2.
- A check fails after the step's commit: §2.5's revert, from the step's parent, checked by `git diff --quiet`.
- A digest, a count hash or a still differs: the change is not exact; stop the step (after its commit, §2.5's revert).
- Disk under 10 GB free: delete `target/shock/` scratch, then `target/debug/incremental` with no cargo running.
- The week's spend at 15 % since the start: no new step; the close runs.

## 7. Critique

Round 1: gpt-6-astra at xhigh through `critique --raw --repo`, 2026-11-25, reading the uncommitted draft at `8570ebf1`. Footer: 296.1 s, tokens in 1,008,358 (cached 924,800), out 8,251, commands 19, usage 5h 1 % → 23 %, week 22 % → 25 %.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | Resetting the counters at chemistry's first run loses a script's or system's blasts earlier in the frame, and zeros on a frame with no run hide blasts there | `crates/tungsten/src/app.rs:2401` (script actions before `PreUpdate` and the fixed update); `examples/01_platformer/src/features/toys/systems.rs:603` (the missile's `blast()`) | Right | §2.2: the frame's first counted operation, a run or a blast, starts it. The kit's test counts a blast before a run and in a frame with none |
| 2 | Steps 4 to 6 name the step before as their base, which after a skip or a revert no longer isolates the step, or does not exist | §2.5 and §6 as written | Right | §2.5 and Steps 4 to 6: the last kept commit before the step, named in the evidence |
| 3 | The invariant on the chain's counts is checked by nothing: the digest hashes `physics:` and `bench:` alone, and the counters sit outside it | `scripts/perf_stats.py:179` | Right | §2.5's `chem_counts.py` hashes each run's count fields, timings and `gathered` out; Steps 3 to 6 and the close check every judged pair |
| 4 | A global cap of 24 is not a cap per stimulus: the window's shocks take `heat` and the steam `cold` under one cap | `examples/01_platformer/assets/data/interactions/thermal_shock.json`, `steam.json`; `crates/tungsten-kit/src/chemistry/queue.rs:700` | Right | §2.4: reported as a smaller global cap; the per-stimulus cap estimated from the handler mix, unmeasured |

Round 2: the same model at xhigh, the plan without §7 and the log (`--from "sed '/^## 7\./,/^## Fol/{/^## Fol/!d}' …"`) and a focus note naming round 1's four changes. Footer: 261.6 s, tokens in 781,597 (cached 707,968), out 7,041, commands 16, usage 5h 23 % → 41 %, week 25 % → 28 %.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | A check that fails after a step's commit, other than a floor, restores from the last commit, which holds the change, so a skipped step's code stays in the next base | §2.5 and §6 as written | Right | §2.5 and §6: every check failed after the commit takes the revert from the step's parent, `git diff --quiet <parent> HEAD -- <files>` before the next base |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| Plan probes | 2026-11-25 | measured | the chain at `8570ebf1` plus timers: `chemistry` p50 21.4, p95 98.7, p99 105.6 ms; `total` p95 138.3, p99 151.5; 48 builds 87.7 of a run's 91.4 ms; Step 3's probe p95 3.96, `total` p95 48.6, digest equal | `perf-runs/20261125-chemistry-shock/` |

## Follow-ups
